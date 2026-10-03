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

/// 啟動器訊息框的語言（ui-language design D6）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LaunchLang {
    /// 繁體中文（簡體中文系統也用繁中，spec「桌面啟動器訊息框語言」）。
    Zh,
    /// 英文（其餘一律）。
    En,
}

/// 完整 LANGID → 訊息語言（純函式）：中文的六個代碼 `0x0404`（台灣）、`0x0C04`（香港）、`0x1404`
/// （澳門）、`0x7C04`（繁體）、`0x0804`（中國）、`0x0004`（簡體）為 [`LaunchLang::Zh`]，其他一律
/// [`LaunchLang::En`]（含新加坡中文 `0x1004`，不在 spec 列的代碼內）。
#[must_use]
pub fn launch_lang_from_langid(langid: u16) -> LaunchLang {
    match langid {
        0x0404 | 0x0C04 | 0x1404 | 0x7C04 | 0x0804 | 0x0004 => LaunchLang::Zh,
        _ => LaunchLang::En,
    }
}

/// 環境變數 `COCKPIT_LAUNCH_LANG`（僅供自動驗收，不對使用者公開）的值 → 語言：`en`／`zh`
/// （不分大小寫、忽略前後空白），其他值為 `None`（改用系統語言）。
#[must_use]
pub fn parse_lang_override(value: &str) -> Option<LaunchLang> {
    match value.trim().to_ascii_lowercase().as_str() {
        "en" => Some(LaunchLang::En),
        "zh" => Some(LaunchLang::Zh),
        _ => None,
    }
}

/// 系統的使用者介面語言（Windows：`GetUserDefaultUILanguage`）；非 Windows 建置固定英文。
#[must_use]
pub fn system_launch_lang() -> LaunchLang {
    #[cfg(windows)]
    {
        #[link(name = "kernel32")]
        unsafe extern "system" {
            /// Win32 `GetUserDefaultUILanguage`（kernel32.dll）：`LANGID GetUserDefaultUILanguage(void)`。
            /// 同 `cockpit-launch` 的 `MessageBoxW`，只宣告這一個函式，不加 `windows-sys` 依賴（design D6）。
            fn GetUserDefaultUILanguage() -> u16;
        }
        // SAFETY：無參數、無指標的純查詢函式，任何時刻呼叫都安全。
        launch_lang_from_langid(unsafe { GetUserDefaultUILanguage() })
    }
    #[cfg(not(windows))]
    {
        LaunchLang::En
    }
}

/// 訊息框的所有文字（design D6 的中央對照表）：變體帶動態參數（路徑、系統錯誤原文等照原文代入），
/// [`text`] 依語言轉成字串。繁中文字與 ui-language 之前逐字相同。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LaunchText<'a> {
    /// 命令列參數不是合法 Unicode；`arg` 是參數的 `{:?}` 表示。
    ArgNotUnicode { arg: &'a str },
    /// 命令列解析錯誤。
    CommandLineError { detail: &'a str },
    /// 傳入了後端旗標 `--exit-when-idle`。
    LauncherOnlyConfig,
    /// 設定檔錯誤。
    ConfigError { detail: &'a str },
    /// `server.listen` 的埠為 0。
    ListenPortZero { listen: &'a str },
    /// 偵測原因：回應不是 HTTP。
    ProbeNotHttp,
    /// 偵測原因：HTTP 狀態不是 200。
    ProbeHttpStatus { code: &'a str },
    /// 偵測原因：`Content-Length` 不合法。
    ProbeBadContentLength,
    /// 偵測原因：回應本體比 `Content-Length` 短。
    ProbeBodyIncomplete,
    /// 偵測原因：回應不是 Cockpit 的 `/api/state`。
    ProbeNotCockpitState,
    /// 偵測原因：連線失敗。
    ProbeConnectFailed { error: &'a str },
    /// 偵測原因：送出請求失敗。
    ProbeSendFailed { error: &'a str },
    /// 偵測原因：讀取逾時。
    ProbeReadTimeout,
    /// 偵測原因：設定讀取逾時失敗。
    ProbeSetTimeoutFailed { error: &'a str },
    /// 偵測原因：回應過大。
    ProbeTooLarge,
    /// 偵測原因：讀取回應失敗。
    ProbeReadFailed { error: &'a str },
    /// 後端失敗訊息的「記錄檔：<路徑>」行。
    LogPathLine { path: &'a str },
    /// 讀不到記錄檔。
    LogUnreadable,
    /// 記錄檔是空的。
    LogEmpty,
    /// 「最後 N 行：」標題。
    LogTailHeader { lines: usize },
    /// 取得工作目錄失敗。
    WorkingDirFailed { detail: &'a str },
    /// 找不到瀏覽器；`searched` 是已排好版的位置清單。
    NoBrowser { searched: &'a str },
    /// 監聽埠被其他程式占用。
    PortOccupied { listen: &'a str, reason: &'a str },
    /// 無法啟動瀏覽器。
    BrowserSpawnFailed { browser: &'a str, detail: &'a str },
    /// 取得啟動器自己的路徑失敗。
    LauncherPathFailed { detail: &'a str },
    /// 無法建立記錄檔。
    LogCreateFailed { path: &'a str, detail: &'a str },
    /// 無法啟動後端。
    BackendSpawnFailed { exe: &'a str, detail: &'a str },
    /// 後端在就緒前結束；`status` 是結束狀態的顯示字串。
    BackendExitedEarly { status: &'a str },
    /// 無法取得後端狀態。
    BackendStatusFailed { detail: &'a str },
    /// 後端在逾時內沒有就緒。
    BackendNotReady { secs: u64, url: &'a str },
}

/// 依語言產生訊息文字；動態參數照原文代入。
#[must_use]
#[allow(clippy::too_many_lines)] // 中央對照表：一個變體一個分支，拆開反而難對照。
pub fn text(lang: LaunchLang, text: LaunchText<'_>) -> String {
    use LaunchLang::{En, Zh};
    use LaunchText::*;
    match (text, lang) {
        (ArgNotUnicode { arg }, Zh) => format!("命令列錯誤：參數含有無法解讀的字元：{arg}"),
        (ArgNotUnicode { arg }, En) => {
            format!(
                "Command-line error: an argument contains characters that cannot be read: {arg}"
            )
        }
        (CommandLineError { detail }, Zh) => format!("命令列錯誤：{detail}"),
        (CommandLineError { detail }, En) => format!("Command-line error: {detail}"),
        (LauncherOnlyConfig, Zh) => {
            "命令列錯誤：啟動器只接受 --config <path>（--exit-when-idle 是後端的旗標）".to_string()
        }
        (LauncherOnlyConfig, En) => "Command-line error: the launcher only accepts --config \
             <path> (--exit-when-idle is a backend flag)"
            .to_string(),
        (ConfigError { detail }, Zh) => format!("設定錯誤：{detail}"),
        (ConfigError { detail }, En) => format!("Configuration error: {detail}"),
        (ListenPortZero { listen }, Zh) => format!(
            "設定錯誤：server.listen 的埠為 0（{listen}），啟動器無法得知實際埠，請指定固定埠"
        ),
        (ListenPortZero { listen }, En) => format!(
            "Configuration error: the port in server.listen is 0 ({listen}), so the launcher \
             cannot tell which port will be used. Please set a fixed port."
        ),
        (ProbeNotHttp, Zh) => "回應不是 HTTP".to_string(),
        (ProbeNotHttp, En) => "response is not HTTP".to_string(),
        (ProbeHttpStatus { code }, Zh) => format!("HTTP 狀態 {code}"),
        (ProbeHttpStatus { code }, En) => format!("HTTP status {code}"),
        (ProbeBadContentLength, Zh) => "Content-Length 不合法".to_string(),
        (ProbeBadContentLength, En) => "invalid Content-Length".to_string(),
        (ProbeBodyIncomplete, Zh) => "回應本體不完整".to_string(),
        (ProbeBodyIncomplete, En) => "response body is incomplete".to_string(),
        (ProbeNotCockpitState, Zh) => "回應不是 Cockpit 的狀態".to_string(),
        (ProbeNotCockpitState, En) => "response is not a Cockpit state".to_string(),
        (ProbeConnectFailed { error }, Zh) => format!("無法連線：{error}"),
        (ProbeConnectFailed { error }, En) => format!("cannot connect: {error}"),
        (ProbeSendFailed { error }, Zh) => format!("送出請求失敗：{error}"),
        (ProbeSendFailed { error }, En) => format!("failed to send the request: {error}"),
        (ProbeReadTimeout, Zh) => "讀取回應逾時".to_string(),
        (ProbeReadTimeout, En) => "timed out reading the response".to_string(),
        (ProbeSetTimeoutFailed { error }, Zh) => format!("設定讀取逾時失敗：{error}"),
        (ProbeSetTimeoutFailed { error }, En) => {
            format!("failed to set the read timeout: {error}")
        }
        (ProbeTooLarge, Zh) => "回應過大".to_string(),
        (ProbeTooLarge, En) => "response is too large".to_string(),
        (ProbeReadFailed { error }, Zh) => format!("讀取回應失敗：{error}"),
        (ProbeReadFailed { error }, En) => format!("failed to read the response: {error}"),
        (LogPathLine { path }, Zh) => format!("記錄檔：{path}"),
        (LogPathLine { path }, En) => format!("Log file: {path}"),
        (LogUnreadable, Zh) => "（無法讀取記錄檔）".to_string(),
        (LogUnreadable, En) => "(Could not read the log file)".to_string(),
        (LogEmpty, Zh) => "（記錄檔是空的）".to_string(),
        (LogEmpty, En) => "(The log file is empty)".to_string(),
        (LogTailHeader { lines }, Zh) => format!("最後 {lines} 行："),
        (LogTailHeader { lines }, En) => format!("Last {lines} lines:"),
        (WorkingDirFailed { detail }, Zh) => format!("取得工作目錄失敗：{detail}"),
        (WorkingDirFailed { detail }, En) => {
            format!("Failed to get the working directory: {detail}")
        }
        (NoBrowser { searched }, Zh) => format!(
            "找不到可用的瀏覽器（Google Chrome 或 Microsoft Edge）。\n\n\
             請安裝其中之一，或以環境變數 COCKPIT_BROWSER 指定瀏覽器執行檔的完整路徑。\n\n\
             找過的位置：\n{searched}"
        ),
        (NoBrowser { searched }, En) => format!(
            "No usable browser was found (Google Chrome or Microsoft Edge).\n\n\
             Install one of them, or set the COCKPIT_BROWSER environment variable to the full \
             path of a browser executable.\n\n\
             Locations searched:\n{searched}"
        ),
        (PortOccupied { listen, reason }, Zh) => format!(
            "{listen} 已被其他程式占用（{reason}），無法啟動 Cockpit。\n\n\
             請關閉占用該埠的程式，或在設定檔的 server.listen 改用其他埠。"
        ),
        (PortOccupied { listen, reason }, En) => format!(
            "{listen} is already in use by another program ({reason}), so Cockpit cannot \
             start.\n\n\
             Close the program using that port, or set a different port in server.listen in \
             the config file."
        ),
        (BrowserSpawnFailed { browser, detail }, Zh) => {
            format!("無法啟動瀏覽器 {browser}：{detail}")
        }
        (BrowserSpawnFailed { browser, detail }, En) => {
            format!("Failed to start the browser {browser}: {detail}")
        }
        (LauncherPathFailed { detail }, Zh) => format!("取得啟動器路徑失敗：{detail}"),
        (LauncherPathFailed { detail }, En) => {
            format!("Failed to get the launcher path: {detail}")
        }
        (LogCreateFailed { path, detail }, Zh) => format!("無法建立記錄檔 {path}：{detail}"),
        (LogCreateFailed { path, detail }, En) => {
            format!("Failed to create the log file {path}: {detail}")
        }
        (BackendSpawnFailed { exe, detail }, Zh) => {
            format!("無法啟動 Cockpit 後端 {exe}：{detail}")
        }
        (BackendSpawnFailed { exe, detail }, En) => {
            format!("Failed to start the Cockpit backend {exe}: {detail}")
        }
        (BackendExitedEarly { status }, Zh) => format!("Cockpit 後端在就緒前結束（{status}）。"),
        (BackendExitedEarly { status }, En) => {
            format!("The Cockpit backend exited before it was ready ({status}).")
        }
        (BackendStatusFailed { detail }, Zh) => format!("無法取得 Cockpit 後端的狀態：{detail}"),
        (BackendStatusFailed { detail }, En) => {
            format!("Failed to get the status of the Cockpit backend: {detail}")
        }
        (BackendNotReady { secs, url }, Zh) => {
            format!("Cockpit 後端在 {secs} 秒內沒有就緒（{url}）。")
        }
        (BackendNotReady { secs, url }, En) => {
            format!("The Cockpit backend was not ready within {secs} seconds ({url}).")
        }
    }
}

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
/// 其他任何引數（含 `--exit-when-idle`，那是後端的旗標）回傳 `lang` 語言的說明。
pub fn parse_launch_args(argv: &[String], lang: LaunchLang) -> Result<Args, String> {
    let args = crate::config::parse_args(argv).map_err(|error| {
        text(
            lang,
            LaunchText::CommandLineError {
                detail: &error.to_string(),
            },
        )
    })?;
    if args.exit_when_idle {
        return Err(text(lang, LaunchText::LauncherOnlyConfig));
    }
    Ok(args)
}

/// 依設定來源與工作目錄組出啟動計畫（spec「啟動器」第 1、4 步）：設定解析沿用
/// [`crate::config::load`]。
///
/// # Errors
///
/// 設定錯誤或監聽埠為 0 時回傳 `lang` 語言的說明。
pub fn plan(
    args: &Args,
    cwd: &Path,
    lookup_env: &dyn Fn(&str) -> Option<String>,
    lang: LaunchLang,
) -> Result<LaunchPlan, String> {
    let config = crate::config::load(args, cwd, lookup_env).map_err(|error| {
        text(
            lang,
            LaunchText::ConfigError {
                detail: &error.to_string(),
            },
        )
    })?;
    let url = listen_url(config.server.listen, lang)?;
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
/// 埠為 0（系統隨機挑埠，啟動器無從得知實際埠）時回傳 `lang` 語言的說明。
pub fn listen_url(listen: SocketAddr, lang: LaunchLang) -> Result<String, String> {
    if listen.port() == 0 {
        return Err(text(
            lang,
            LaunchText::ListenPortZero {
                listen: &listen.to_string(),
            },
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
pub fn classify_response(raw: &[u8], lang: LaunchLang) -> ProbeOutcome {
    let not = |reason: LaunchText<'_>| ProbeOutcome::NotCockpit(text(lang, reason));
    let Some(header_end) = find_header_end(raw) else {
        return not(LaunchText::ProbeNotHttp);
    };
    let Ok(head) = std::str::from_utf8(&raw[..header_end]) else {
        return not(LaunchText::ProbeNotHttp);
    };
    let mut lines = head.split("\r\n");
    let mut status = lines.next().unwrap_or("").splitn(3, ' ');
    if !status.next().unwrap_or("").starts_with("HTTP/1.") {
        return not(LaunchText::ProbeNotHttp);
    }
    let code = status.next().unwrap_or("");
    if code != "200" {
        return not(LaunchText::ProbeHttpStatus { code });
    }
    let mut content_length = None;
    for line in lines {
        if let Some((name, value)) = line.split_once(':')
            && name.trim().eq_ignore_ascii_case("content-length")
        {
            match value.trim().parse::<usize>() {
                Ok(length) => content_length = Some(length),
                Err(_) => return not(LaunchText::ProbeBadContentLength),
            }
        }
    }
    let mut body = &raw[header_end + 4..];
    if let Some(length) = content_length {
        if body.len() < length {
            return not(LaunchText::ProbeBodyIncomplete);
        }
        body = &body[..length];
    }
    match serde_json::from_slice::<serde_json::Value>(body) {
        Ok(serde_json::Value::Object(map))
            if map.contains_key("version") && map.contains_key("runtimes") =>
        {
            ProbeOutcome::Cockpit
        }
        _ => not(LaunchText::ProbeNotCockpitState),
    }
}

/// 對 `http://<listen>/api/state` 發 `GET`（design D2）：`connect_timeout` 1 秒、
/// `HTTP/1.0`＋`Connection: close`、讀到 EOF 或達 `Content-Length`，讀取總期限 2 秒、
/// 本體最多 8 MiB。
#[must_use]
pub fn probe(listen: SocketAddr, lang: LaunchLang) -> ProbeOutcome {
    let not = |reason: LaunchText<'_>| ProbeOutcome::NotCockpit(text(lang, reason));
    let mut stream = match TcpStream::connect_timeout(&listen, PROBE_CONNECT_TIMEOUT) {
        Ok(stream) => stream,
        Err(error) => {
            return ProbeOutcome::Unreachable(text(
                lang,
                LaunchText::ProbeConnectFailed {
                    error: &error.to_string(),
                },
            ));
        }
    };
    let deadline = Instant::now() + PROBE_READ_TIMEOUT;
    let request = format!(
        "GET /api/state HTTP/1.0\r\nHost: {listen}\r\nAccept: application/json\r\nConnection: close\r\n\r\n"
    );
    let _ = stream.set_write_timeout(Some(PROBE_READ_TIMEOUT));
    if let Err(error) = stream.write_all(request.as_bytes()) {
        return not(LaunchText::ProbeSendFailed {
            error: &error.to_string(),
        });
    }
    let mut buf = Vec::new();
    let mut chunk = vec![0u8; 64 * 1024];
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return not(LaunchText::ProbeReadTimeout);
        }
        if let Err(error) = stream.set_read_timeout(Some(remaining)) {
            return not(LaunchText::ProbeSetTimeoutFailed {
                error: &error.to_string(),
            });
        }
        match stream.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => {
                buf.extend_from_slice(&chunk[..n]);
                match response_progress(&buf) {
                    Progress::Complete => break,
                    Progress::TooLarge => {
                        return not(LaunchText::ProbeTooLarge);
                    }
                    Progress::Incomplete => {}
                }
            }
            Err(error) if error.kind() == ErrorKind::Interrupted => {}
            Err(error) if matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {
                return not(LaunchText::ProbeReadTimeout);
            }
            Err(error) => {
                return not(LaunchText::ProbeReadFailed {
                    error: &error.to_string(),
                });
            }
        }
    }
    classify_response(&buf, lang)
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
pub fn backend_failure_message(
    reason: &str,
    log_path: &Path,
    log_text: Option<&str>,
    lang: LaunchLang,
) -> String {
    let path_line = text(
        lang,
        LaunchText::LogPathLine {
            path: &log_path.display().to_string(),
        },
    );
    let mut message = format!("{reason}\n\n{path_line}\n");
    match log_text.map(|log| log_tail(log, LOG_TAIL_LINES)) {
        None => message.push_str(&text(lang, LaunchText::LogUnreadable)),
        Some(tail) if tail.is_empty() => message.push_str(&text(lang, LaunchText::LogEmpty)),
        Some(tail) => {
            let header = text(
                lang,
                LaunchText::LogTailHeader {
                    lines: LOG_TAIL_LINES,
                },
            );
            message.push_str(&format!("{header}\n{tail}"));
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

    // 既有測試一律以 `Zh` 斷言原本的中文內容（ui-language task 4.1）：下列包裝函式遮蔽 glob 匯入的同名
    // 公開函式，固定傳入 `Zh`，其餘既有測試維持原樣、不必逐一改簽章。
    const ZH: LaunchLang = LaunchLang::Zh;

    fn parse_launch_args(argv: &[String]) -> Result<Args, String> {
        super::parse_launch_args(argv, ZH)
    }

    fn plan(
        args: &Args,
        cwd: &Path,
        lookup_env: &dyn Fn(&str) -> Option<String>,
    ) -> Result<LaunchPlan, String> {
        super::plan(args, cwd, lookup_env, ZH)
    }

    fn listen_url(listen: SocketAddr) -> Result<String, String> {
        super::listen_url(listen, ZH)
    }

    fn classify_response(raw: &[u8]) -> ProbeOutcome {
        super::classify_response(raw, ZH)
    }

    fn probe(listen: SocketAddr) -> ProbeOutcome {
        super::probe(listen, ZH)
    }

    fn backend_failure_message(reason: &str, log_path: &Path, log_text: Option<&str>) -> String {
        super::backend_failure_message(reason, log_path, log_text, ZH)
    }

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

    // ---- 訊息語言（ui-language task 4.1；design D6） ----

    #[test]
    fn langid_chinese_variants_are_zh() {
        for (langid, name) in [
            (0x0404u16, "zh-TW"),
            (0x0C04, "zh-HK"),
            (0x1404, "zh-MO"),
            (0x7C04, "zh-Hant"),
            (0x0804, "zh-CN"),
            (0x0004, "zh-Hans"),
        ] {
            assert_eq!(launch_lang_from_langid(langid), LaunchLang::Zh, "{name}");
        }
    }

    #[test]
    fn langid_other_languages_are_en() {
        for (langid, name) in [
            (0x0409u16, "en-US"),
            (0x0809, "en-GB"),
            (0x0411, "ja-JP"),
            // 新加坡中文（0x1004）不在 spec 列的六個代碼內，歸英文。
            (0x1004, "zh-SG"),
            (0x0000, "未指定"),
        ] {
            assert_eq!(launch_lang_from_langid(langid), LaunchLang::En, "{name}");
        }
    }

    #[test]
    fn lang_override_accepts_only_en_and_zh() {
        assert_eq!(parse_lang_override("en"), Some(LaunchLang::En));
        assert_eq!(parse_lang_override("EN"), Some(LaunchLang::En));
        assert_eq!(parse_lang_override("zh"), Some(LaunchLang::Zh));
        assert_eq!(parse_lang_override(" zh "), Some(LaunchLang::Zh));
        assert_eq!(parse_lang_override(""), None);
        assert_eq!(parse_lang_override("ja"), None);
    }

    /// 每個 `LaunchText` 變體一筆：（變體，既有的中文原文，英文）。`exhaustive` 的 `match` 不含
    /// 萬用分支，新增變體卻沒補進 `exhaustive` 時編譯就失敗，提醒同步補這張表。
    fn text_table() -> Vec<(LaunchText<'static>, &'static str, &'static str)> {
        use LaunchText::*;
        vec![
            (
                ArgNotUnicode { arg: "\"x\"" },
                "命令列錯誤：參數含有無法解讀的字元：\"x\"",
                "Command-line error: an argument contains characters that cannot be read: \"x\"",
            ),
            (
                CommandLineError { detail: "bad" },
                "命令列錯誤：bad",
                "Command-line error: bad",
            ),
            (
                LauncherOnlyConfig,
                "命令列錯誤：啟動器只接受 --config <path>（--exit-when-idle 是後端的旗標）",
                "Command-line error: the launcher only accepts --config <path> (--exit-when-idle is a backend flag)",
            ),
            (
                ConfigError { detail: "bad" },
                "設定錯誤：bad",
                "Configuration error: bad",
            ),
            (
                ListenPortZero {
                    listen: "127.0.0.1:0",
                },
                "設定錯誤：server.listen 的埠為 0（127.0.0.1:0），啟動器無法得知實際埠，請指定固定埠",
                "Configuration error: the port in server.listen is 0 (127.0.0.1:0), so the launcher cannot tell which port will be used. Please set a fixed port.",
            ),
            (ProbeNotHttp, "回應不是 HTTP", "response is not HTTP"),
            (
                ProbeHttpStatus { code: "404" },
                "HTTP 狀態 404",
                "HTTP status 404",
            ),
            (
                ProbeBadContentLength,
                "Content-Length 不合法",
                "invalid Content-Length",
            ),
            (
                ProbeBodyIncomplete,
                "回應本體不完整",
                "response body is incomplete",
            ),
            (
                ProbeNotCockpitState,
                "回應不是 Cockpit 的狀態",
                "response is not a Cockpit state",
            ),
            (
                ProbeConnectFailed { error: "e" },
                "無法連線：e",
                "cannot connect: e",
            ),
            (
                ProbeSendFailed { error: "e" },
                "送出請求失敗：e",
                "failed to send the request: e",
            ),
            (
                ProbeReadTimeout,
                "讀取回應逾時",
                "timed out reading the response",
            ),
            (
                ProbeSetTimeoutFailed { error: "e" },
                "設定讀取逾時失敗：e",
                "failed to set the read timeout: e",
            ),
            (ProbeTooLarge, "回應過大", "response is too large"),
            (
                ProbeReadFailed { error: "e" },
                "讀取回應失敗：e",
                "failed to read the response: e",
            ),
            (
                LogPathLine {
                    path: "D:/w/cockpit.log",
                },
                "記錄檔：D:/w/cockpit.log",
                "Log file: D:/w/cockpit.log",
            ),
            (
                LogUnreadable,
                "（無法讀取記錄檔）",
                "(Could not read the log file)",
            ),
            (LogEmpty, "（記錄檔是空的）", "(The log file is empty)"),
            (
                LogTailHeader { lines: 20 },
                "最後 20 行：",
                "Last 20 lines:",
            ),
            (
                WorkingDirFailed { detail: "d" },
                "取得工作目錄失敗：d",
                "Failed to get the working directory: d",
            ),
            (
                NoBrowser { searched: "  C:/a" },
                "找不到可用的瀏覽器（Google Chrome 或 Microsoft Edge）。\n\n\
                 請安裝其中之一，或以環境變數 COCKPIT_BROWSER 指定瀏覽器執行檔的完整路徑。\n\n\
                 找過的位置：\n  C:/a",
                "No usable browser was found (Google Chrome or Microsoft Edge).\n\n\
                 Install one of them, or set the COCKPIT_BROWSER environment variable to the full path of a browser executable.\n\n\
                 Locations searched:\n  C:/a",
            ),
            (
                PortOccupied {
                    listen: "127.0.0.1:7770",
                    reason: "r",
                },
                "127.0.0.1:7770 已被其他程式占用（r），無法啟動 Cockpit。\n\n\
                 請關閉占用該埠的程式，或在設定檔的 server.listen 改用其他埠。",
                "127.0.0.1:7770 is already in use by another program (r), so Cockpit cannot start.\n\n\
                 Close the program using that port, or set a different port in server.listen in the config file.",
            ),
            (
                BrowserSpawnFailed {
                    browser: "b.exe",
                    detail: "d",
                },
                "無法啟動瀏覽器 b.exe：d",
                "Failed to start the browser b.exe: d",
            ),
            (
                LauncherPathFailed { detail: "d" },
                "取得啟動器路徑失敗：d",
                "Failed to get the launcher path: d",
            ),
            (
                LogCreateFailed {
                    path: "p.log",
                    detail: "d",
                },
                "無法建立記錄檔 p.log：d",
                "Failed to create the log file p.log: d",
            ),
            (
                BackendSpawnFailed {
                    exe: "c.exe",
                    detail: "d",
                },
                "無法啟動 Cockpit 後端 c.exe：d",
                "Failed to start the Cockpit backend c.exe: d",
            ),
            (
                BackendExitedEarly {
                    status: "exit code: 1",
                },
                "Cockpit 後端在就緒前結束（exit code: 1）。",
                "The Cockpit backend exited before it was ready (exit code: 1).",
            ),
            (
                BackendStatusFailed { detail: "d" },
                "無法取得 Cockpit 後端的狀態：d",
                "Failed to get the status of the Cockpit backend: d",
            ),
            (
                BackendNotReady {
                    secs: 15,
                    url: "http://127.0.0.1:7770/",
                },
                "Cockpit 後端在 15 秒內沒有就緒（http://127.0.0.1:7770/）。",
                "The Cockpit backend was not ready within 15 seconds (http://127.0.0.1:7770/).",
            ),
        ]
    }

    /// 編譯期窮舉檢查：新增 `LaunchText` 變體時這個 `match` 不完整而編譯失敗，提醒補 `text_table`。
    #[test]
    fn text_table_covers_every_variant() {
        use LaunchText::*;
        let table = text_table();
        for (variant, _, _) in &table {
            match variant {
                ArgNotUnicode { .. }
                | CommandLineError { .. }
                | LauncherOnlyConfig
                | ConfigError { .. }
                | ListenPortZero { .. }
                | ProbeNotHttp
                | ProbeHttpStatus { .. }
                | ProbeBadContentLength
                | ProbeBodyIncomplete
                | ProbeNotCockpitState
                | ProbeConnectFailed { .. }
                | ProbeSendFailed { .. }
                | ProbeReadTimeout
                | ProbeSetTimeoutFailed { .. }
                | ProbeTooLarge
                | ProbeReadFailed { .. }
                | LogPathLine { .. }
                | LogUnreadable
                | LogEmpty
                | LogTailHeader { .. }
                | WorkingDirFailed { .. }
                | NoBrowser { .. }
                | PortOccupied { .. }
                | BrowserSpawnFailed { .. }
                | LauncherPathFailed { .. }
                | LogCreateFailed { .. }
                | BackendSpawnFailed { .. }
                | BackendExitedEarly { .. }
                | BackendStatusFailed { .. }
                | BackendNotReady { .. } => {}
            }
        }
        // 30 個變體各一筆；數字不一致代表上面的 match 與表對不上。
        assert_eq!(table.len(), 30);
    }

    fn has_cjk(text: &str) -> bool {
        text.chars().any(|c| {
            matches!(c as u32,
                0x2E80..=0x9FFF | 0xAC00..=0xD7AF | 0xF900..=0xFAFF | 0xFE30..=0xFE4F | 0xFF00..=0xFFEF)
        })
    }

    #[test]
    fn every_text_zh_is_byte_identical_to_the_old_message() {
        for (variant, zh, _) in text_table() {
            assert_eq!(text(LaunchLang::Zh, variant), zh, "{variant:?}");
        }
    }

    #[test]
    fn every_text_en_is_exact_non_empty_and_has_no_cjk() {
        for (variant, _, en) in text_table() {
            let rendered = text(LaunchLang::En, variant);
            assert_eq!(rendered, en, "{variant:?}");
            assert!(!rendered.is_empty(), "{variant:?}");
            assert!(!has_cjk(&rendered), "{variant:?}: {rendered}");
        }
    }

    #[test]
    fn english_probe_reasons_and_failure_message_follow_the_language() {
        assert_eq!(
            super::classify_response(b"SSH-2.0\r\n", LaunchLang::En),
            ProbeOutcome::NotCockpit("response is not HTTP".to_string())
        );
        assert_eq!(
            super::classify_response(b"HTTP/1.1 404 Not Found\r\n\r\n", LaunchLang::En),
            ProbeOutcome::NotCockpit("HTTP status 404".to_string())
        );
        // 失敗訊息：標籤翻成英文，記錄檔路徑與內容照原文（內容含中文也不動）。
        let log_path = Path::new("work").join("cockpit.log");
        let msg = super::backend_failure_message(
            "reason",
            &log_path,
            Some("後端錯誤\nline 2\n"),
            LaunchLang::En,
        );
        assert_eq!(
            msg,
            format!(
                "reason\n\nLog file: {}\nLast 20 lines:\n後端錯誤\nline 2",
                log_path.display()
            )
        );
        let msg = super::backend_failure_message("r", Path::new("p.log"), Some(""), LaunchLang::En);
        assert!(msg.ends_with("(The log file is empty)"), "{msg}");
        let msg = super::backend_failure_message("r", Path::new("p.log"), None, LaunchLang::En);
        assert!(msg.ends_with("(Could not read the log file)"), "{msg}");
    }

    #[test]
    fn english_argument_and_config_errors() {
        let err =
            super::parse_launch_args(&argv(&["--exit-when-idle"]), LaunchLang::En).unwrap_err();
        assert!(err.starts_with("Command-line error:"), "{err}");
        let err = super::listen_url("127.0.0.1:0".parse().unwrap(), LaunchLang::En).unwrap_err();
        assert!(err.starts_with("Configuration error:"), "{err}");
    }

    #[test]
    fn system_lang_is_fixed_en_off_windows() {
        let lang = system_launch_lang();
        if cfg!(not(windows)) {
            assert_eq!(lang, LaunchLang::En);
        }
    }
}
