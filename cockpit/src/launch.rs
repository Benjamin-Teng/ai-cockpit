//! 桌面啟動器 `cockpit-launch` 的可測邏輯（desktop-launch-notify task 2.2；spec `desktop-launch`
//! 「啟動器」；design D1–D4）。
//!
//! 這裡只放純函式與一個最小的 HTTP 偵測函式；bin（`src/bin/cockpit-launch.rs`）只做 I/O 串接
//! （開子程序、輪詢、訊息框）。設定檔位置與監聽位址沿用 [`crate::config`] 的既有規則
//! （`parse_args`／`load`），不在這裡重寫。

use std::ffi::OsString;
use std::io::{ErrorKind, Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::config::{Args, ConfigSource};

/// 偵測的連線逾時（design D2）。
pub const PROBE_CONNECT_TIMEOUT: Duration = Duration::from_secs(1);
/// 偵測的讀取逾時：連上之後整段讀取的期限，與連線逾時合計不超過 3 秒（design D2）。
pub const PROBE_READ_TIMEOUT: Duration = Duration::from_secs(2);
/// 偵測回應本體的上限（design D2）。
pub const PROBE_MAX_BODY: usize = 8 * 1024 * 1024;
/// 錯誤訊息框附上 `cockpit.log` 的最後幾行（design D5）。
pub const LOG_TAIL_LINES: usize = 20;
/// 後端 log 檔名（design D3）。
pub const LOG_FILE_NAME: &str = "cockpit.log";

/// 由命令列、設定得出的啟動計畫：bin 照著做 I/O。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LaunchPlan {
    /// 監聽位址（偵測與開視窗都用它）。
    pub listen: SocketAddr,
    /// 瀏覽器要開的網址：`http://<監聽位址>/`。
    pub url: String,
    /// 背景啟動後端時的引數：`--config <絕對路徑>`（有設定檔時）與 `--exit-when-idle`。
    pub backend_args: Vec<OsString>,
    /// 後端 stdout／stderr 寫入的 log 檔。
    pub log_path: PathBuf,
}

/// 偵測 `GET /api/state` 的結果（spec「啟動器」第 3 步）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProbeOutcome {
    /// 回應 200 且本體是含 `version` 與 `runtimes` 的 JSON 物件：Cockpit 已在執行。
    Cockpit,
    /// 連得上，但回應不是 Cockpit（或根本不回 HTTP）：埠被占用；字串是原因。
    NotCockpit(String),
    /// 連不上：沒有程式在聽；字串是原因。
    Unreachable(String),
}

/// 解析啟動器的命令列：只接受 `--config <path>`／`--config=<path>`（spec「啟動器」第 1 步）。
///
/// # Errors
///
/// 其他任何引數（含 `--exit-when-idle`，那是後端的旗標）回傳中文說明。
pub fn parse_launch_args(argv: &[String]) -> Result<Args, String> {
    let args = crate::config::parse_args(argv).map_err(|error| format!("命令列錯誤：{error}"))?;
    if args.exit_when_idle {
        return Err(
            "命令列錯誤：啟動器只接受 --config <path>（--exit-when-idle 是後端的旗標）".to_string(),
        );
    }
    Ok(args)
}

/// 依設定來源與工作目錄組出啟動計畫（spec「啟動器」第 1、4 步）：設定解析沿用
/// [`crate::config::load`]。
///
/// # Errors
///
/// 設定錯誤或監聽埠為 0 時回傳中文說明。
pub fn plan(
    args: &Args,
    cwd: &Path,
    lookup_env: &dyn Fn(&str) -> Option<String>,
) -> Result<LaunchPlan, String> {
    let config =
        crate::config::load(args, cwd, lookup_env).map_err(|error| format!("設定錯誤：{error}"))?;
    let url = listen_url(config.server.listen)?;
    // `load` 已依 cwd 把相對路徑接成絕對路徑；再以 `std::path::absolute` 正規化掉 `.`
    // 之類的片段（不碰檔案系統），讓後端引數與 log 路徑都是乾淨的絕對路徑。
    let file = config_file(&config.source)
        .map(|path| std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf()));
    Ok(LaunchPlan {
        listen: config.server.listen,
        url,
        backend_args: backend_args(file.as_deref()),
        log_path: log_path(file.as_deref(), cwd),
    })
}

/// 監聽位址 → 瀏覽器網址 `http://<監聽位址>/`。
///
/// # Errors
///
/// 埠為 0（系統隨機挑埠，啟動器無從得知實際埠）時回傳中文說明。
pub fn listen_url(listen: SocketAddr) -> Result<String, String> {
    if listen.port() == 0 {
        return Err(format!(
            "設定錯誤：server.listen 的埠為 0（{listen}），啟動器無法得知實際埠，請指定固定埠"
        ));
    }
    Ok(format!("http://{listen}/"))
}

/// 設定來源中的設定檔路徑；零設定與 inline 沒有檔案。
#[must_use]
pub fn config_file(source: &ConfigSource) -> Option<&Path> {
    match source {
        ConfigSource::Explicit(path) | ConfigSource::Cwd(path) => Some(path),
        ConfigSource::ZeroConfig | ConfigSource::Inline => None,
    }
}

/// 後端引數：`--config <絕對路徑>`（有設定檔時）＋ `--exit-when-idle`（design D3）。
#[must_use]
pub fn backend_args(config_file: Option<&Path>) -> Vec<OsString> {
    let mut args = Vec::with_capacity(3);
    if let Some(path) = config_file {
        args.push(OsString::from("--config"));
        args.push(path.as_os_str().to_os_string());
    }
    args.push(OsString::from("--exit-when-idle"));
    args
}

/// log 路徑：有設定檔時在其所在目錄，否則在工作目錄（design D3）。
#[must_use]
pub fn log_path(config_file: Option<&Path>, cwd: &Path) -> PathBuf {
    let dir = config_file
        .and_then(Path::parent)
        .filter(|dir| !dir.as_os_str().is_empty())
        .unwrap_or(cwd);
    dir.join(LOG_FILE_NAME)
}

/// 後端執行檔：啟動器同目錄的 `cockpit`（Windows 上為 `cockpit.exe`）。
#[must_use]
pub fn backend_exe(launcher_exe: &Path) -> PathBuf {
    let name = format!("cockpit{}", std::env::consts::EXE_SUFFIX);
    match launcher_exe.parent() {
        Some(dir) => dir.join(name),
        None => PathBuf::from(name),
    }
}

/// 瀏覽器候選清單，依優先順序（design D4）：`COCKPIT_BROWSER` → Chrome（`%ProgramFiles%`、
/// `%ProgramFiles(x86)%`、`%LOCALAPPDATA%`）→ Edge（`%ProgramFiles(x86)%`、`%ProgramFiles%`）。
/// 環境變數沒設或為空的位置略過。
#[must_use]
pub fn browser_candidates(lookup_env: &dyn Fn(&str) -> Option<String>) -> Vec<PathBuf> {
    const CHROME: &str = r"Google\Chrome\Application\chrome.exe";
    const EDGE: &str = r"Microsoft\Edge\Application\msedge.exe";
    let var = |key: &str| lookup_env(key).filter(|value| !value.is_empty());
    let mut candidates = Vec::new();
    if let Some(custom) = var("COCKPIT_BROWSER") {
        candidates.push(PathBuf::from(custom));
    }
    for (key, relative) in [
        ("ProgramFiles", CHROME),
        ("ProgramFiles(x86)", CHROME),
        ("LOCALAPPDATA", CHROME),
        ("ProgramFiles(x86)", EDGE),
        ("ProgramFiles", EDGE),
    ] {
        if let Some(base) = var(key) {
            candidates.push(Path::new(&base).join(relative));
        }
    }
    candidates
}

/// 候選清單中第一個存在的執行檔（spec「啟動器」第 2 步）；都不存在回 `None`。
#[must_use]
pub fn select_browser(
    lookup_env: &dyn Fn(&str) -> Option<String>,
    exists: &dyn Fn(&Path) -> bool,
) -> Option<PathBuf> {
    browser_candidates(lookup_env)
        .into_iter()
        .find(|candidate| exists(candidate))
}

/// 開視窗的瀏覽器引數：`--app=<網址>`（design D4）。
#[must_use]
pub fn browser_args(url: &str) -> Vec<OsString> {
    vec![OsString::from(format!("--app={url}"))]
}

/// 判斷一段完整的 HTTP 回應（標頭＋本體）是不是 Cockpit 的 `/api/state`（design D2）：
/// 狀態 200，本體（依 `Content-Length` 截取）是含 `version` 與 `runtimes` 的 JSON 物件。
/// 只會回 [`ProbeOutcome::Cockpit`] 或 [`ProbeOutcome::NotCockpit`]。
#[must_use]
pub fn classify_response(raw: &[u8]) -> ProbeOutcome {
    let not = |reason: &str| ProbeOutcome::NotCockpit(reason.to_string());
    let Some(header_end) = find_header_end(raw) else {
        return not("回應不是 HTTP");
    };
    let Ok(head) = std::str::from_utf8(&raw[..header_end]) else {
        return not("回應不是 HTTP");
    };
    let mut lines = head.split("\r\n");
    let mut status = lines.next().unwrap_or("").splitn(3, ' ');
    if !status.next().unwrap_or("").starts_with("HTTP/1.") {
        return not("回應不是 HTTP");
    }
    let code = status.next().unwrap_or("");
    if code != "200" {
        return ProbeOutcome::NotCockpit(format!("HTTP 狀態 {code}"));
    }
    let mut content_length = None;
    for line in lines {
        if let Some((name, value)) = line.split_once(':')
            && name.trim().eq_ignore_ascii_case("content-length")
        {
            match value.trim().parse::<usize>() {
                Ok(length) => content_length = Some(length),
                Err(_) => return not("Content-Length 不合法"),
            }
        }
    }
    let mut body = &raw[header_end + 4..];
    if let Some(length) = content_length {
        if body.len() < length {
            return not("回應本體不完整");
        }
        body = &body[..length];
    }
    match serde_json::from_slice::<serde_json::Value>(body) {
        Ok(serde_json::Value::Object(map))
            if map.contains_key("version") && map.contains_key("runtimes") =>
        {
            ProbeOutcome::Cockpit
        }
        _ => not("回應不是 Cockpit 的狀態"),
    }
}

/// 對 `http://<listen>/api/state` 發 `GET`（design D2）：`connect_timeout` 1 秒、
/// `HTTP/1.0`＋`Connection: close`、讀到 EOF 或達 `Content-Length`，讀取總期限 2 秒、
/// 本體最多 8 MiB。
#[must_use]
pub fn probe(listen: SocketAddr) -> ProbeOutcome {
    let mut stream = match TcpStream::connect_timeout(&listen, PROBE_CONNECT_TIMEOUT) {
        Ok(stream) => stream,
        Err(error) => return ProbeOutcome::Unreachable(format!("無法連線：{error}")),
    };
    let deadline = Instant::now() + PROBE_READ_TIMEOUT;
    let request = format!(
        "GET /api/state HTTP/1.0\r\nHost: {listen}\r\nAccept: application/json\r\nConnection: close\r\n\r\n"
    );
    let _ = stream.set_write_timeout(Some(PROBE_READ_TIMEOUT));
    if let Err(error) = stream.write_all(request.as_bytes()) {
        return ProbeOutcome::NotCockpit(format!("送出請求失敗：{error}"));
    }
    let mut buf = Vec::new();
    let mut chunk = vec![0u8; 64 * 1024];
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return ProbeOutcome::NotCockpit("讀取回應逾時".to_string());
        }
        if let Err(error) = stream.set_read_timeout(Some(remaining)) {
            return ProbeOutcome::NotCockpit(format!("設定讀取逾時失敗：{error}"));
        }
        match stream.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => {
                buf.extend_from_slice(&chunk[..n]);
                match response_progress(&buf) {
                    Progress::Complete => break,
                    Progress::TooLarge => {
                        return ProbeOutcome::NotCockpit("回應過大".to_string());
                    }
                    Progress::Incomplete => {}
                }
            }
            Err(error) if error.kind() == ErrorKind::Interrupted => {}
            Err(error) if matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {
                return ProbeOutcome::NotCockpit("讀取回應逾時".to_string());
            }
            Err(error) => return ProbeOutcome::NotCockpit(format!("讀取回應失敗：{error}")),
        }
    }
    classify_response(&buf)
}

/// 文字的最後 `max_lines` 行（以 `\n` 分行、去掉 `\r`，忽略結尾的空行）。
#[must_use]
pub fn log_tail(text: &str, max_lines: usize) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let end = lines
        .iter()
        .rposition(|line| !line.trim().is_empty())
        .map_or(0, |index| index + 1);
    let start = end.saturating_sub(max_lines);
    lines[start..end].join("\n")
}

/// 後端啟動失敗的訊息（spec「啟動器」第 4 步）：原因、`cockpit.log` 完整路徑、最後 20 行。
/// `log_text` 為 `None` 表示讀不到 log。
#[must_use]
pub fn backend_failure_message(reason: &str, log_path: &Path, log_text: Option<&str>) -> String {
    let mut message = format!("{reason}\n\n記錄檔：{}\n", log_path.display());
    match log_text.map(|text| log_tail(text, LOG_TAIL_LINES)) {
        None => message.push_str("（無法讀取記錄檔）"),
        Some(tail) if tail.is_empty() => message.push_str("（記錄檔是空的）"),
        Some(tail) => {
            message.push_str(&format!("最後 {LOG_TAIL_LINES} 行：\n{tail}"));
        }
    }
    message
}

/// 回應標頭的上限：超過還找不到標頭結尾就不是 Cockpit。
const PROBE_MAX_HEADER: usize = 64 * 1024;

/// `\r\n\r\n` 的位置（標頭結尾）。
fn find_header_end(raw: &[u8]) -> Option<usize> {
    raw.windows(4).position(|window| window == b"\r\n\r\n")
}

/// 讀取進度：決定 [`probe`] 要不要繼續讀。
enum Progress {
    /// 標頭與 `Content-Length` 指定的本體都到齊。
    Complete,
    /// 標頭或本體超過上限。
    TooLarge,
    /// 還要再讀（沒有 `Content-Length` 時一路讀到連線關閉）。
    Incomplete,
}

/// 依目前讀到的位元組判斷 [`Progress`]。
fn response_progress(buf: &[u8]) -> Progress {
    let Some(header_end) = find_header_end(buf) else {
        return if buf.len() > PROBE_MAX_HEADER {
            Progress::TooLarge
        } else {
            Progress::Incomplete
        };
    };
    let body_len = buf.len() - (header_end + 4);
    let content_length = std::str::from_utf8(&buf[..header_end])
        .ok()
        .and_then(|head| {
            head.split("\r\n").skip(1).find_map(|line| {
                let (name, value) = line.split_once(':')?;
                if name.trim().eq_ignore_ascii_case("content-length") {
                    value.trim().parse::<usize>().ok()
                } else {
                    None
                }
            })
        });
    match content_length {
        Some(length) if length > PROBE_MAX_BODY => Progress::TooLarge,
        Some(length) if body_len >= length => Progress::Complete,
        _ if body_len > PROBE_MAX_BODY => Progress::TooLarge,
        _ => Progress::Incomplete,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    use super::*;

    fn argv(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| (*s).to_string()).collect()
    }

    fn env_from(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let map: HashMap<String, String> = pairs
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        move |key| map.get(key).cloned()
    }

    /// 測試用的獨立暫存目錄（不在 repo 內）。
    fn temp_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("cockpit-launch-test-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    // ---- 引數解析 ----

    #[test]
    fn args_accept_config_both_forms_and_nothing() {
        assert_eq!(parse_launch_args(&[]).unwrap(), Args::default());
        assert_eq!(
            parse_launch_args(&argv(&["--config", "a.toml"]))
                .unwrap()
                .config,
            Some(PathBuf::from("a.toml"))
        );
        assert_eq!(
            parse_launch_args(&argv(&["--config=b.toml"]))
                .unwrap()
                .config,
            Some(PathBuf::from("b.toml"))
        );
    }

    #[test]
    fn args_reject_other_flags_including_exit_when_idle() {
        assert!(parse_launch_args(&argv(&["--exit-when-idle"])).is_err());
        assert!(parse_launch_args(&argv(&["--help"])).is_err());
        assert!(parse_launch_args(&argv(&["--config"])).is_err());
        assert!(parse_launch_args(&argv(&["--config", "a.toml", "extra"])).is_err());
    }

    // ---- 監聽位址 → 網址 ----

    #[test]
    fn url_from_listen_and_port_zero_rejected() {
        assert_eq!(
            listen_url("127.0.0.1:7770".parse().unwrap()).unwrap(),
            "http://127.0.0.1:7770/"
        );
        assert_eq!(
            listen_url("[::1]:7791".parse().unwrap()).unwrap(),
            "http://[::1]:7791/"
        );
        assert!(listen_url("127.0.0.1:0".parse().unwrap()).is_err());
    }

    // ---- 後端引數與 log 路徑 ----

    #[test]
    fn backend_args_with_and_without_config() {
        let cfg = Path::new(r"D:\work\cockpit.toml");
        assert_eq!(
            backend_args(Some(cfg)),
            vec![
                OsString::from("--config"),
                OsString::from(r"D:\work\cockpit.toml"),
                OsString::from("--exit-when-idle"),
            ]
        );
        assert_eq!(backend_args(None), vec![OsString::from("--exit-when-idle")]);
    }

    // 路徑以 `join` 組出、不寫死以 `\` 分隔的字面值（修正波 2.6 M8）：Unix 的 `Path` 不把 `\` 當分隔
    // 符，寫死 Windows 路徑的斷言在非 Windows 平台會紅。
    #[test]
    fn log_path_next_to_config_or_in_cwd() {
        let base = std::env::temp_dir();
        let work = base.join("work");
        let cwd = base.join("elsewhere");
        assert_eq!(
            log_path(Some(&work.join("cockpit.toml")), &cwd),
            work.join("cockpit.log")
        );
        assert_eq!(log_path(None, &cwd), cwd.join("cockpit.log"));
    }

    #[test]
    fn config_file_only_for_file_sources() {
        let p = PathBuf::from(r"D:\work\cockpit.toml");
        assert_eq!(
            config_file(&ConfigSource::Explicit(p.clone())),
            Some(p.as_path())
        );
        assert_eq!(
            config_file(&ConfigSource::Cwd(p.clone())),
            Some(p.as_path())
        );
        assert_eq!(config_file(&ConfigSource::ZeroConfig), None);
        assert_eq!(config_file(&ConfigSource::Inline), None);
    }

    #[test]
    fn backend_exe_is_sibling_cockpit() {
        let suffix = std::env::consts::EXE_SUFFIX;
        let dir = std::env::temp_dir().join("bin");
        let exe = backend_exe(&dir.join(format!("cockpit-launch{suffix}")));
        assert_eq!(exe, dir.join(format!("cockpit{suffix}")));
    }

    #[test]
    fn plan_with_relative_config_gives_absolute_path_and_log_next_to_it() {
        let dir = temp_dir("plan-rel");
        std::fs::write(
            dir.join("cockpit.toml"),
            "[server]\nlisten = \"127.0.0.1:7791\"\n",
        )
        .unwrap();
        let args = parse_launch_args(&argv(&["--config", "cockpit.toml"])).unwrap();
        let plan = plan(&args, &dir, &env_from(&[])).unwrap();
        let cfg = dir.join("cockpit.toml");
        assert!(cfg.is_absolute());
        assert_eq!(plan.listen, "127.0.0.1:7791".parse().unwrap());
        assert_eq!(plan.url, "http://127.0.0.1:7791/");
        assert_eq!(
            plan.backend_args,
            vec![
                OsString::from("--config"),
                cfg.clone().into_os_string(),
                OsString::from("--exit-when-idle"),
            ]
        );
        assert_eq!(plan.log_path, dir.join("cockpit.log"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn plan_uses_cwd_cockpit_toml_when_no_flag() {
        let dir = temp_dir("plan-cwd");
        std::fs::write(
            dir.join("cockpit.toml"),
            "[server]\nlisten = \"127.0.0.1:7792\"\n",
        )
        .unwrap();
        let plan = plan(&Args::default(), &dir, &env_from(&[])).unwrap();
        assert_eq!(plan.url, "http://127.0.0.1:7792/");
        assert_eq!(plan.backend_args[0], OsString::from("--config"));
        assert_eq!(
            plan.backend_args[1],
            dir.join("cockpit.toml").into_os_string()
        );
        assert_eq!(plan.log_path, dir.join("cockpit.log"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn plan_zero_config_logs_in_cwd_without_config_arg() {
        let dir = temp_dir("plan-zero");
        let plan = plan(&Args::default(), &dir, &env_from(&[])).unwrap();
        assert_eq!(plan.url, "http://127.0.0.1:7770/");
        assert_eq!(plan.backend_args, vec![OsString::from("--exit-when-idle")]);
        assert_eq!(plan.log_path, dir.join("cockpit.log"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn plan_errors_on_bad_config_and_port_zero() {
        let dir = temp_dir("plan-bad");
        let missing = parse_launch_args(&argv(&["--config", "nope.toml"])).unwrap();
        assert!(plan(&missing, &dir, &env_from(&[])).is_err());

        std::fs::write(dir.join("bad.toml"), "this is = = not toml").unwrap();
        let bad = parse_launch_args(&argv(&["--config", "bad.toml"])).unwrap();
        assert!(plan(&bad, &dir, &env_from(&[])).is_err());

        std::fs::write(
            dir.join("zero.toml"),
            "[server]\nlisten = \"127.0.0.1:0\"\n",
        )
        .unwrap();
        let zero = parse_launch_args(&argv(&["--config", "zero.toml"])).unwrap();
        let err = plan(&zero, &dir, &env_from(&[])).unwrap_err();
        assert!(err.contains('0'), "{err}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    // ---- 瀏覽器 ----

    fn standard_env() -> impl Fn(&str) -> Option<String> {
        env_from(&[
            ("ProgramFiles", r"C:\Program Files"),
            ("ProgramFiles(x86)", r"C:\Program Files (x86)"),
            ("LOCALAPPDATA", r"C:\Users\u\AppData\Local"),
        ])
    }

    #[test]
    fn candidates_in_documented_order() {
        let env = env_from(&[
            ("COCKPIT_BROWSER", r"E:\b\browser.exe"),
            ("ProgramFiles", r"C:\Program Files"),
            ("ProgramFiles(x86)", r"C:\Program Files (x86)"),
            ("LOCALAPPDATA", r"C:\Users\u\AppData\Local"),
        ]);
        let chrome = Path::new(r"Google\Chrome\Application\chrome.exe");
        let edge = Path::new(r"Microsoft\Edge\Application\msedge.exe");
        assert_eq!(
            browser_candidates(&env),
            vec![
                PathBuf::from(r"E:\b\browser.exe"),
                Path::new(r"C:\Program Files").join(chrome),
                Path::new(r"C:\Program Files (x86)").join(chrome),
                Path::new(r"C:\Users\u\AppData\Local").join(chrome),
                Path::new(r"C:\Program Files (x86)").join(edge),
                Path::new(r"C:\Program Files").join(edge),
            ]
        );
    }

    #[test]
    fn candidates_skip_unset_or_empty_env() {
        let env = env_from(&[("COCKPIT_BROWSER", ""), ("ProgramFiles", r"C:\PF")]);
        assert_eq!(
            browser_candidates(&env),
            vec![
                Path::new(r"C:\PF").join(r"Google\Chrome\Application\chrome.exe"),
                Path::new(r"C:\PF").join(r"Microsoft\Edge\Application\msedge.exe"),
            ]
        );
    }

    #[test]
    fn spec_scenario_edge_when_chrome_missing() {
        // spec「瀏覽器選擇順序」：沒設 COCKPIT_BROWSER、Chrome 未安裝、Edge 已安裝 → Edge。
        let env = standard_env();
        let edge =
            Path::new(r"C:\Program Files (x86)").join(r"Microsoft\Edge\Application\msedge.exe");
        let edge_for_exists = edge.clone();
        let picked = select_browser(&env, &move |p: &Path| p == edge_for_exists);
        assert_eq!(picked, Some(edge));
    }

    #[test]
    fn chrome_preferred_over_edge() {
        let env = standard_env();
        let chrome =
            Path::new(r"C:\Users\u\AppData\Local").join(r"Google\Chrome\Application\chrome.exe");
        // 前綴用 `standard_env` 的 LOCALAPPDATA 整段（修正波 2.6 M8）：只比 `C:\Users` 時，Unix 的
        // `Path::starts_with` 以元件比對，而 `C:\Users\u\AppData\Local` 在 Unix 上整段是一個元件，比不到。
        let local_app_data = Path::new(r"C:\Users\u\AppData\Local");
        let picked = select_browser(&env, &|p: &Path| {
            p.ends_with(r"Google\Chrome\Application\chrome.exe") && p.starts_with(local_app_data)
                || p.ends_with(r"Microsoft\Edge\Application\msedge.exe")
        });
        assert_eq!(picked, Some(chrome));
    }

    #[test]
    fn cockpit_browser_used_first_when_exists_and_skipped_when_missing() {
        let env = env_from(&[
            ("COCKPIT_BROWSER", r"E:\b\browser.exe"),
            ("ProgramFiles", r"C:\Program Files"),
        ]);
        let chrome = Path::new(r"C:\Program Files").join(r"Google\Chrome\Application\chrome.exe");
        // 存在 → 用它。
        assert_eq!(
            select_browser(&env, &|_p: &Path| true),
            Some(PathBuf::from(r"E:\b\browser.exe"))
        );
        // 不存在 → 略過，落到 Chrome。
        let chrome_for_exists = chrome.clone();
        assert_eq!(
            select_browser(&env, &move |p: &Path| p == chrome_for_exists),
            Some(chrome)
        );
    }

    #[test]
    fn no_browser_found() {
        let env = env_from(&[
            ("COCKPIT_BROWSER", r"E:\nope\browser.exe"),
            ("ProgramFiles", r"C:\Program Files"),
            ("ProgramFiles(x86)", r"C:\Program Files (x86)"),
            ("LOCALAPPDATA", r"C:\Users\u\AppData\Local"),
        ]);
        assert_eq!(select_browser(&env, &|_p: &Path| false), None);
    }

    #[test]
    fn browser_opens_app_mode() {
        assert_eq!(
            browser_args("http://127.0.0.1:7770/"),
            vec![OsString::from("--app=http://127.0.0.1:7770/")]
        );
    }

    // ---- 回應判斷 ----

    fn response(status: &str, headers: &str, body: &str) -> Vec<u8> {
        format!("HTTP/1.1 {status}\r\n{headers}\r\n{body}").into_bytes()
    }

    #[test]
    fn cockpit_state_response_recognized() {
        let body = r#"{"version":3,"runtimes":[],"projects":[]}"#;
        let raw = response(
            "200 OK",
            &format!(
                "content-type: application/json\r\ncontent-length: {}\r\n",
                body.len()
            ),
            body,
        );
        assert_eq!(classify_response(&raw), ProbeOutcome::Cockpit);
        // 沒有 Content-Length（讀到 EOF）也可以。
        let raw = response("200 OK", "", body);
        assert_eq!(classify_response(&raw), ProbeOutcome::Cockpit);
    }

    #[test]
    fn non_cockpit_responses_are_occupied() {
        let cases: Vec<Vec<u8>> = vec![
            // 一般網頁。
            response("200 OK", "content-length: 5\r\n", "hello"),
            // JSON 但缺欄位。
            response("200 OK", "", r#"{"version":1}"#),
            response("200 OK", "", r#"{"runtimes":[]}"#),
            // JSON 但不是物件。
            response("200 OK", "", r#"[{"version":1,"runtimes":[]}]"#),
            // 狀態不是 200。
            response("404 Not Found", "", r#"{"version":1,"runtimes":[]}"#),
            // 本體比 Content-Length 短（被截斷）。
            response(
                "200 OK",
                "content-length: 100\r\n",
                r#"{"version":1,"runtimes":[]}"#,
            ),
            // 根本不是 HTTP。
            b"SSH-2.0-OpenSSH_9.0\r\n".to_vec(),
            Vec::new(),
        ];
        for raw in cases {
            assert!(
                matches!(classify_response(&raw), ProbeOutcome::NotCockpit(_)),
                "{:?}",
                String::from_utf8_lossy(&raw)
            );
        }
    }

    #[test]
    fn content_length_limits_body() {
        let body = r#"{"version":1,"runtimes":[]}"#;
        let raw = response(
            "200 OK",
            &format!("Content-Length: {}\r\n", body.len()),
            &format!("{body}garbage-after"),
        );
        assert_eq!(classify_response(&raw), ProbeOutcome::Cockpit);
    }

    // ---- 偵測（真的 TCP） ----

    fn serve_once(reply: Vec<u8>) -> (SocketAddr, std::thread::JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let handle = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buf = [0u8; 4096];
            let n = stream.read(&mut buf).unwrap();
            stream.write_all(&reply).unwrap();
            String::from_utf8_lossy(&buf[..n]).into_owned()
        });
        (addr, handle)
    }

    #[test]
    fn probe_recognizes_cockpit_and_sends_http10_close_with_host() {
        let body = r#"{"version":1,"runtimes":[]}"#;
        let (addr, handle) = serve_once(response(
            "200 OK",
            &format!("content-length: {}\r\n", body.len()),
            body,
        ));
        assert_eq!(probe(addr), ProbeOutcome::Cockpit);
        let request = handle.join().unwrap();
        assert!(
            request.starts_with("GET /api/state HTTP/1.0\r\n"),
            "{request}"
        );
        assert!(request.contains(&format!("Host: {addr}\r\n")), "{request}");
        assert!(request.contains("Connection: close\r\n"), "{request}");
    }

    #[test]
    fn probe_other_http_service_is_not_cockpit() {
        let (addr, handle) = serve_once(response("200 OK", "", "<html>hi</html>"));
        assert!(matches!(probe(addr), ProbeOutcome::NotCockpit(_)));
        handle.join().unwrap();
    }

    #[test]
    fn probe_nothing_listening_is_unreachable() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        drop(listener);
        assert!(matches!(probe(addr), ProbeOutcome::Unreachable(_)));
    }

    #[test]
    fn probe_silent_server_times_out_as_not_cockpit_within_budget() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let handle = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            std::thread::sleep(Duration::from_secs(4));
            drop(stream);
        });
        let started = std::time::Instant::now();
        assert!(matches!(probe(addr), ProbeOutcome::NotCockpit(_)));
        assert!(
            started.elapsed() < Duration::from_secs(3),
            "{:?}",
            started.elapsed()
        );
        handle.join().unwrap();
    }

    // ---- log 尾段與訊息 ----

    #[test]
    fn log_tail_takes_last_lines() {
        let text: String = (1..=30).map(|i| format!("line {i}\r\n")).collect();
        let tail = log_tail(&text, 20);
        let lines: Vec<&str> = tail.lines().collect();
        assert_eq!(lines.len(), 20);
        assert_eq!(lines[0], "line 11");
        assert_eq!(lines[19], "line 30");
        assert!(!tail.contains('\r'));
        assert_eq!(log_tail("a\nb\n\n\n", 20), "a\nb");
        assert_eq!(log_tail("", 20), "");
    }

    #[test]
    fn failure_message_has_reason_path_and_tail() {
        let text: String = (1..=25).map(|i| format!("row {i}\n")).collect();
        let msg = backend_failure_message(
            "後端在就緒前結束",
            Path::new(r"D:\work\cockpit.log"),
            Some(&text),
        );
        assert!(msg.contains("後端在就緒前結束"));
        assert!(msg.contains(r"D:\work\cockpit.log"));
        assert!(msg.contains("row 25"));
        assert!(msg.contains("row 6"));
        assert!(!msg.contains("row 5\n"));
        let msg = backend_failure_message("x", Path::new(r"D:\w\cockpit.log"), None);
        assert!(msg.contains(r"D:\w\cockpit.log"));
    }
}
