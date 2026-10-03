//! 自動更新的純邏輯（auto-update tasks 2.1–2.5；spec `auto-update`；design D3–D7）。
//!
//! 這裡沒有網路、沒有檔案 I/O、不讀系統時鐘、不讀真實環境變數：現在時間以參數傳入（Unix 秒），
//! 環境變數以注入的 `lookup_env` 提供（同 [`crate::launch::plan`]）。唯一碰串流的
//! [`copy_hashed`] 泛型於 `Read`／`Write`，由呼叫端給來源與目的地。網路、檔案、訊息框與 spawn
//! 都在啟動器 bin（`src/bin/cockpit-launch.rs`）。

use std::ffi::OsString;
use std::fmt;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// 本專案的 GitHub 查詢網址：最新正式 release（不含草稿與預發布；design D6）。
pub const LATEST_RELEASE_API_URL: &str =
    "https://api.github.com/repos/Benjamin-Teng/ai-cockpit/releases/latest";
/// 固定下載基底；完整網址為 `<基底>/v<版本>/<檔名>`（design D6）。
pub const DOWNLOAD_BASE_URL: &str = "https://github.com/Benjamin-Teng/ai-cockpit/releases/download";
/// 雜湊檔的資產名稱。
pub const SUMS_FILE_NAME: &str = "SHA256SUMS.txt";
/// 更新檢查紀錄檔名，放在 `cockpit.log` 所在目錄（design D5）。
pub const STATE_FILE_NAME: &str = "cockpit.update.json";
/// 兩次檢查之間至少相隔的秒數：24 小時（spec「何時檢查更新」）。
pub const CHECK_INTERVAL_SECS: u64 = 24 * 60 * 60;
/// 紀錄檔 `result` 的暫存值：發出查詢前先寫入，結果出來後覆寫（design D5）。
pub const RESULT_PENDING: &str = "pending";

/// 環境變數：非空即關閉更新檢查（spec「何時檢查更新」）。
pub const ENV_NO_UPDATE_CHECK: &str = "COCKPIT_NO_UPDATE_CHECK";
/// 環境變數（僅供自動驗收，design D7）：取代 [`LATEST_RELEASE_API_URL`]。
pub const ENV_API_URL: &str = "COCKPIT_UPDATE_API_URL";
/// 環境變數（僅供自動驗收，design D7）：取代 [`DOWNLOAD_BASE_URL`]。
pub const ENV_DOWNLOAD_BASE: &str = "COCKPIT_UPDATE_DOWNLOAD_BASE";
/// 環境變數（僅供自動驗收，design D7）：訊息框改寫檔時，詢問框的答案（`yes`／`no`，預設 `no`）。
pub const ENV_UPDATE_ANSWER: &str = "COCKPIT_LAUNCH_UPDATE_ANSWER";

/// `X.Y.Z` 三段十進位版本；衍生的排序即「主、次、修訂」依序比較。
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Version {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
}

impl Version {
    /// 嚴格解析 `X.Y.Z`：恰三段、每段非空且只含 ASCII 數字、無前導零（`0` 本身可）、不溢位。
    /// 不接受前導 `v`、後綴（`-rc.1`）、空白或第四段。
    #[must_use]
    pub fn parse(s: &str) -> Option<Version> {
        let mut parts = s.split('.');
        let major = parse_part(parts.next()?)?;
        let minor = parse_part(parts.next()?)?;
        let patch = parse_part(parts.next()?)?;
        if parts.next().is_some() {
            return None;
        }
        Some(Version {
            major,
            minor,
            patch,
        })
    }

    /// 解析 release 的 `tag_name`：必須是 `v` 加 `X.Y.Z`（spec「判定有無新版」）。
    #[must_use]
    pub fn parse_tag(tag: &str) -> Option<Version> {
        Version::parse(tag.strip_prefix('v')?)
    }
}

/// 版本的一段：非空、全為 ASCII 數字、除 `0` 本身外不以 `0` 開頭（`u64::from_str` 會放行 `+1`，
/// 所以先自己檢查字元），溢位時為 `None`。
fn parse_part(part: &str) -> Option<u64> {
    if part.is_empty()
        || !part.bytes().all(|b| b.is_ascii_digit())
        || (part.len() > 1 && part.starts_with('0'))
    {
        return None;
    }
    part.parse().ok()
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// 本執行檔（`cockpit` crate）的版本；`CARGO_PKG_VERSION` 不是 `X.Y.Z` 時為 `None`。
#[must_use]
pub fn current_version() -> Option<Version> {
    Version::parse(env!("CARGO_PKG_VERSION"))
}

/// 版本檢查的判定結果。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CheckOutcome {
    /// 有新版（已確認版本較大且兩個資產都在）。
    NewVersion(Version),
    /// 沒有新版；字串是寫進紀錄檔 `result` 的英文原因。連線失敗或逾時沒有 HTTP 狀態碼，
    /// 由呼叫端直接組 `NoUpdate("connection failed: …")`／`NoUpdate("timed out")`。
    NoUpdate(String),
}

impl CheckOutcome {
    /// 寫進紀錄檔 `result` 的字串。
    #[must_use]
    pub fn log_result(&self) -> String {
        match self {
            CheckOutcome::NewVersion(version) => format!("update available: {version}"),
            CheckOutcome::NoUpdate(reason) => reason.clone(),
        }
    }
}

/// 安裝檔的資產名稱：`ai-cockpit-<版本>-x64-setup.exe`。
#[must_use]
pub fn installer_file_name(version: Version) -> String {
    format!("ai-cockpit-{version}-x64-setup.exe")
}

/// 由 `releases/latest` 的 HTTP 狀態碼與本體，判定有無新版（spec「判定有無新版」）。
/// 只有 200、本體為 JSON、`tag_name` 為 `vX.Y.Z`、版本大於 `current`、資產清單含安裝檔與
/// [`SUMS_FILE_NAME`] 時才是 [`CheckOutcome::NewVersion`]；其餘一律 [`CheckOutcome::NoUpdate`]。
#[must_use]
pub fn judge_latest_release(status: u16, body: &[u8], current: Version) -> CheckOutcome {
    let no = |reason: String| CheckOutcome::NoUpdate(reason);
    match status {
        200 => {}
        404 => return no("HTTP 404 (no stable release yet)".to_string()),
        403 | 429 => return no(format!("HTTP {status} (rate limited or forbidden)")),
        _ => return no(format!("HTTP {status}")),
    }
    let Ok(json) = serde_json::from_slice::<serde_json::Value>(body) else {
        return no("response is not valid JSON".to_string());
    };
    let Some(tag) = json.get("tag_name").and_then(serde_json::Value::as_str) else {
        return no("missing tag_name".to_string());
    };
    let Some(latest) = Version::parse_tag(tag) else {
        return no(format!("tag_name {tag:?} is not vX.Y.Z"));
    };
    if latest <= current {
        return no(format!(
            "latest {latest} is not newer than current {current}"
        ));
    }
    let has_asset = |wanted: &str| {
        json.get("assets")
            .and_then(serde_json::Value::as_array)
            .is_some_and(|assets| {
                assets
                    .iter()
                    .any(|a| a.get("name").and_then(serde_json::Value::as_str) == Some(wanted))
            })
    };
    for wanted in [installer_file_name(latest), SUMS_FILE_NAME.to_string()] {
        if !has_asset(&wanted) {
            return no(format!("release {latest} is missing asset {wanted}"));
        }
    }
    CheckOutcome::NewVersion(latest)
}

/// 查詢請求的 `Accept` 值（design D6）。
pub const API_ACCEPT: &str = "application/vnd.github+json";

/// 查詢請求的 `User-Agent`：`ai-cockpit/<目前版本>`（GitHub 要求必帶；design D6）。
#[must_use]
pub fn user_agent() -> String {
    format!("ai-cockpit/{}", env!("CARGO_PKG_VERSION"))
}

/// 讀環境變數，未設定與空字串都視為沒有。
fn non_empty_env(lookup_env: &dyn Fn(&str) -> Option<String>, key: &str) -> Option<String> {
    lookup_env(key).filter(|value| !value.is_empty())
}

/// 是否關閉更新檢查：`COCKPIT_NO_UPDATE_CHECK` 非空即關閉（未設定或空字串為否）。
#[must_use]
pub fn update_check_disabled(lookup_env: &dyn Fn(&str) -> Option<String>) -> bool {
    non_empty_env(lookup_env, ENV_NO_UPDATE_CHECK).is_some()
}

/// 自動驗收模式（訊息框改寫檔）下詢問框的答案：`COCKPIT_LAUNCH_UPDATE_ANSWER` 為 `yes`
/// （不分大小寫、忽略前後空白）才是 `true`，其他（含未設定）一律 `false`（design D7）。
#[must_use]
pub fn update_answer_is_yes(lookup_env: &dyn Fn(&str) -> Option<String>) -> bool {
    lookup_env(ENV_UPDATE_ANSWER).is_some_and(|value| value.trim().eq_ignore_ascii_case("yes"))
}

/// 查詢網址：`COCKPIT_UPDATE_API_URL` 非空時取其值，否則 [`LATEST_RELEASE_API_URL`]（design D7）。
#[must_use]
pub fn latest_release_url(lookup_env: &dyn Fn(&str) -> Option<String>) -> String {
    non_empty_env(lookup_env, ENV_API_URL).unwrap_or_else(|| LATEST_RELEASE_API_URL.to_string())
}

/// 下載基底：`COCKPIT_UPDATE_DOWNLOAD_BASE` 非空時取其值（去掉結尾的 `/`），否則
/// [`DOWNLOAD_BASE_URL`]（design D7）。
#[must_use]
pub fn download_base(lookup_env: &dyn Fn(&str) -> Option<String>) -> String {
    non_empty_env(lookup_env, ENV_DOWNLOAD_BASE)
        .map(|base| base.trim_end_matches('/').to_string())
        .unwrap_or_else(|| DOWNLOAD_BASE_URL.to_string())
}

/// 是否只允許 https：`COCKPIT_UPDATE_API_URL`、`COCKPIT_UPDATE_DOWNLOAD_BASE` 都沒有設定（或為空）時為真；
/// 任一有值時為假，讓自動驗收能用本機 `http://` 假伺服器（design D7）。
#[must_use]
pub fn https_only(lookup_env: &dyn Fn(&str) -> Option<String>) -> bool {
    non_empty_env(lookup_env, ENV_API_URL).is_none()
        && non_empty_env(lookup_env, ENV_DOWNLOAD_BASE).is_none()
}

/// 啟動器查詢與下載用的 ureq agent（只建立設定，不連線；design D3、D6）：整體逾時（連線到讀完本體）、
/// `User-Agent: ai-cockpit/<版本>`，`https_only` 為真時連轉址在內都只允許 `https://`（見 [`https_only`]）。
/// 其餘沿用預設：`http_status_as_error` 為真（4xx／5xx 成為 `Err(StatusCode)`）、跟隨最多 10 次轉址、
/// 讀 `HTTPS_PROXY` 等代理環境變數。
#[must_use]
pub fn http_agent(timeout: Duration, https_only: bool) -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(timeout))
        .user_agent(user_agent())
        .https_only(https_only)
        .build()
        .into()
}

/// 固定下載網址 `<基底>/v<版本>/<檔名>`（design D6）；不使用 API 回應中的任何網址。
#[must_use]
pub fn download_url(base: &str, version: Version, file_name: &str) -> String {
    format!("{base}/v{version}/{file_name}")
}

/// 解析 `SHA256SUMS.txt` 失敗的原因。
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum SumsError {
    /// 檔案不是 release.yml 寫出的格式（第 `line` 行，從 1 起算；BOM 與 CR 算在出現的那一行）。
    #[error("{SUMS_FILE_NAME} line {line} is malformed")]
    Malformed { line: usize },
    /// 沒有檔名恰為 `file` 的那一行。
    #[error("{SUMS_FILE_NAME} has no entry for {file}")]
    NotFound { file: String },
    /// 有不只一行檔名恰為 `file`。
    #[error("{SUMS_FILE_NAME} has more than one entry for {file}")]
    Duplicate { file: String },
}

/// 從 `SHA256SUMS.txt` 內容找出檔名恰為 `file_name` 那一行的雜湊，回傳小寫 64 位十六進位。
///
/// 只有「檔名欄是 `file_name`」的行必須是 release.yml 寫出的格式（`<64 位十六進位>` 兩個空白
/// `<檔名>`，無 BOM、無 CR、檔名前後無空白）；格式錯就是 [`SumsError::Malformed`]。判斷一行的
/// 檔名欄時從寬：第一段空白之後、去掉可有可無的 `*` 與結尾空白（含 CR）的部分。其他行能嚴格
/// 解析就參與同名重複檢查（實際上不同名，不影響結果），不能解析（空行、垃圾行、BOM、CRLF）就略過。
///
/// # Errors
/// 目標行格式錯誤、找不到、同名多行（見 [`SumsError`]）。
pub fn find_expected_sha256(sums: &str, file_name: &str) -> Result<String, SumsError> {
    let mut found: Option<&str> = None;
    for (index, line) in sums.split('\n').enumerate() {
        let strict = parse_sums_line(line);
        let is_target = strict.is_some_and(|(_, name)| name == file_name)
            || loose_name_field(line) == Some(file_name);
        if !is_target {
            continue;
        }
        let Some((hash, _)) = strict.filter(|(_, name)| *name == file_name) else {
            return Err(SumsError::Malformed { line: index + 1 });
        };
        if found.is_some() {
            return Err(SumsError::Duplicate {
                file: file_name.to_string(),
            });
        }
        found = Some(hash);
    }
    found
        .map(str::to_ascii_lowercase)
        .ok_or_else(|| SumsError::NotFound {
            file: file_name.to_string(),
        })
}

/// 從寬取一行的檔名欄：第一段空白（空格或 tab）之後的內容，去掉前導空白、一個可有可無的 `*`
/// （二進位標記）與結尾空白（含 CR）。沒有空白分隔時為 `None`。只用來判斷「這行是不是在講
/// 目標檔」，以便目標行格式錯時報錯而不是略過。
fn loose_name_field(line: &str) -> Option<&str> {
    let (_, rest) = line.split_once([' ', '\t'])?;
    let rest = rest.trim_start_matches([' ', '\t']);
    let rest = rest.strip_prefix('*').unwrap_or(rest);
    Some(rest.trim_end())
}

/// 一行 `<64 位十六進位>` 兩個空白 `<檔名>`；檔名非空、前後不得有空白字元（含 CR）。
fn parse_sums_line(line: &str) -> Option<(&str, &str)> {
    let hash = line.get(..64)?;
    let name = line.get(64..)?.strip_prefix("  ")?;
    let name_ok = !name.is_empty() && name.trim() == name;
    (hash.bytes().all(|b| b.is_ascii_hexdigit()) && name_ok).then_some((hash, name))
}

/// 兩個十六進位雜湊是否相等（不分大小寫）。
#[must_use]
pub fn sha256_matches(expected_hex: &str, actual_hex: &str) -> bool {
    expected_hex.eq_ignore_ascii_case(actual_hex)
}

/// 位元組轉小寫十六進位字串（把 `sha2` 算出的摘要轉成可比對的字串）。
#[must_use]
pub fn hex_lower(bytes: &[u8]) -> String {
    use fmt::Write as _;
    bytes
        .iter()
        .fold(String::with_capacity(bytes.len() * 2), |mut out, byte| {
            let _ = write!(out, "{byte:02x}");
            out
        })
}

/// 更新檢查紀錄 `cockpit.update.json` 的內容（design D5）。
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CheckRecord {
    /// 最近一次發出查詢的時間（Unix 秒）。
    pub checked_at: u64,
    /// 該次的結果與原因（給排查用的英文短句）；舊檔缺這欄時為空字串。
    #[serde(default)]
    pub result: String,
}

/// 解析紀錄檔內容；不是合法 JSON、缺 `checked_at` 或型別不對時為 `None`（讀不懂）。
#[must_use]
pub fn parse_record(text: &str) -> Option<CheckRecord> {
    serde_json::from_str(text).ok()
}

/// 序列化成紀錄檔內容（單行 JSON）。
#[must_use]
pub fn render_record(record: &CheckRecord) -> String {
    // 只有一個整數與一個字串，序列化不會失敗。
    serde_json::to_string(record).expect("CheckRecord 序列化不會失敗")
}

/// 距上次檢查是否已滿 [`CHECK_INTERVAL_SECS`]：沒有紀錄（不存在或讀不懂）、紀錄時間在未來
/// （時鐘被調回）、或相隔不少於 24 小時皆為 `true`（spec「何時檢查更新」）。
#[must_use]
pub fn is_check_due(record: Option<&CheckRecord>, now: u64) -> bool {
    match record {
        None => true,
        // 時間在未來：時鐘被調回過，視為已滿，免得之後永遠不檢查。
        Some(record) if record.checked_at > now => true,
        Some(record) => now - record.checked_at >= CHECK_INTERVAL_SECS,
    }
}

// ---- 啟動器 I/O 用的常數與輔助（auto-update tasks 3.1–3.3；design D2、D6、D8） ----

/// 查詢 `releases/latest` 的整體逾時（連線、送出、讀取回應；spec「判定有無新版」）。
pub const QUERY_TIMEOUT: Duration = Duration::from_secs(3);
/// 下載 `SHA256SUMS.txt` 的整體逾時（design D6）。
pub const SUMS_TIMEOUT: Duration = Duration::from_secs(15);
/// 下載安裝檔的整體逾時（design D6）。
pub const INSTALLER_TIMEOUT: Duration = Duration::from_secs(300);
/// 查詢回應本體上限 1 MB（控制端裁決；GitHub 的 latest release JSON 通常只有幾 KB）。
pub const MAX_QUERY_BODY_BYTES: u64 = 1024 * 1024;
/// `SHA256SUMS.txt` 上限 64 KB（控制端裁決；實際只有三行）。
pub const MAX_SUMS_BYTES: u64 = 64 * 1024;
/// 安裝檔上限 200 MB（spec「下載與驗證」；以 MiB 計）。
pub const MAX_INSTALLER_BYTES: u64 = 200 * 1024 * 1024;
/// 使用者暫存資料夾下的 Cockpit 專用資料夾；每個啟動器程序在其中用自己的子資料夾（[`work_dir`]，design D6）。
pub const TEMP_DIR_NAME: &str = "ai-cockpit-update";
/// Inno Setup 放在程式目錄的解除安裝程式；存在即「由安裝檔安裝」（design D2）。
pub const UNINSTALLER_FILE_NAME: &str = "unins000.exe";
/// 交棒時安裝檔 `/LOG=` 的檔名，放在 [`TEMP_DIR_NAME`] 資料夾（design D8）。
pub const SETUP_LOG_FILE_NAME: &str = "setup.log";

/// 啟動器同目錄的解除安裝程式路徑；`launcher_exe` 沒有上層目錄時為 `None`。
#[must_use]
pub fn uninstaller_path(launcher_exe: &Path) -> Option<PathBuf> {
    launcher_exe
        .parent()
        .filter(|dir| !dir.as_os_str().is_empty())
        .map(|dir| dir.join(UNINSTALLER_FILE_NAME))
}

/// 更新檢查紀錄的路徑：`cockpit.log` 的同一目錄（design D5）。
#[must_use]
pub fn state_path(log_path: &Path) -> PathBuf {
    match log_path.parent() {
        Some(dir) => dir.join(STATE_FILE_NAME),
        None => PathBuf::from(STATE_FILE_NAME),
    }
}

/// 程序 `pid` 下載與交棒用的暫存子資料夾 `<temp_dir>\ai-cockpit-update\<pid>\`（design D6）：
/// 同時開兩個啟動器時各用各的，不會清掉或覆寫對方正在下載的安裝檔。
#[must_use]
pub fn work_dir(temp_dir: &Path, pid: u32) -> PathBuf {
    temp_dir.join(TEMP_DIR_NAME).join(pid.to_string())
}

/// 交棒給安裝檔的引數（design D8 逐字）：`/SILENT /SUPPRESSMSGBOXES /NORESTART /NOCANCEL
/// /COCKPITUPDATE=1 /LOG=<work_dir>\setup.log`。`/LOG=` 與路徑是同一個引數，交給
/// `Command::arg` 時若路徑含空白，Rust 會把整個引數加上引號。
#[must_use]
pub fn installer_args(work_dir: &Path) -> Vec<OsString> {
    let mut log = OsString::from("/LOG=");
    log.push(work_dir.join(SETUP_LOG_FILE_NAME));
    let mut args: Vec<OsString> = [
        "/SILENT",
        "/SUPPRESSMSGBOXES",
        "/NORESTART",
        "/NOCANCEL",
        "/COCKPITUPDATE=1",
    ]
    .into_iter()
    .map(OsString::from)
    .collect();
    args.push(log);
    args
}

/// [`copy_hashed`] 的結果。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Copied {
    /// 寫入的位元組數。
    pub bytes: u64,
    /// 寫入內容的 SHA-256（小寫十六進位）。
    pub sha256: String,
}

/// [`copy_hashed`] 失敗的原因；`Display` 為英文短句，可當訊息的 `detail`。
#[derive(Debug, thiserror::Error)]
pub enum CopyError {
    /// 來源超過上限；超過的部分沒有寫入。
    #[error("the file is larger than {max} bytes")]
    TooLarge { max: u64 },
    /// 讀取來源失敗（下載中斷、逾時）。
    #[error("{0}")]
    Read(std::io::Error),
    /// 寫入目的地失敗。
    #[error("{0}")]
    Write(std::io::Error),
}

/// 把 `reader` 串流寫進 `writer`，同時計算 SHA-256 與位元組數；累計超過 `max_bytes` 即中止
/// （剛好等於上限可以）。泛型於 `Read`／`Write`，啟動器以下載串流與檔案呼叫，測試以記憶體呼叫。
///
/// # Errors
/// 超過上限、讀取失敗、寫入失敗（見 [`CopyError`]）。
pub fn copy_hashed<R: Read, W: Write>(
    mut reader: R,
    mut writer: W,
    max_bytes: u64,
) -> Result<Copied, CopyError> {
    use sha2::Digest as _;
    let mut hasher = sha2::Sha256::new();
    let mut total: u64 = 0;
    let mut buffer = vec![0u8; 64 * 1024];
    loop {
        let n = match reader.read(&mut buffer) {
            Ok(0) => break,
            Ok(n) => n,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(CopyError::Read(error)),
        };
        total = total.saturating_add(n as u64);
        if total > max_bytes {
            return Err(CopyError::TooLarge { max: max_bytes });
        }
        let chunk = &buffer[..n];
        hasher.update(chunk);
        writer.write_all(chunk).map_err(CopyError::Write)?;
    }
    writer.flush().map_err(CopyError::Write)?;
    Ok(Copied {
        bytes: total,
        sha256: hex_lower(&hasher.finalize()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(major: u64, minor: u64, patch: u64) -> Version {
        Version {
            major,
            minor,
            patch,
        }
    }

    // ---- 2.1 版本解析與比較 ----

    #[test]
    fn parse_accepts_three_decimal_parts() {
        assert_eq!(Version::parse("0.1.0"), Some(v(0, 1, 0)));
        assert_eq!(Version::parse("12.345.6789"), Some(v(12, 345, 6789)));
        assert_eq!(Version::parse("0.0.0"), Some(v(0, 0, 0)));
    }

    #[test]
    fn parse_rejects_everything_but_plain_x_y_z() {
        for bad in [
            "",
            "0.2",
            "0.2.0.1",
            "0.2.0-rc.1",
            "0.2.0+build",
            "v0.2.0",
            " 0.2.0",
            "0.2.0 ",
            "0.2.0\n",
            "a.b.c",
            "0.x.0",
            "0..0",
            ".1.2",
            "1.2.",
            "-1.2.3",
            "+1.2.3",
            "1.2.\u{ff13}",
            "01.2.3",
            "1.02.3",
            "1.2.03",
            "99999999999999999999.0.0",
        ] {
            assert_eq!(Version::parse(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn parse_tag_requires_one_leading_v() {
        assert_eq!(Version::parse_tag("v0.2.0"), Some(v(0, 2, 0)));
        assert_eq!(Version::parse_tag("v10.0.1"), Some(v(10, 0, 1)));
        for bad in [
            "",
            "v",
            "0.2.0",
            "vv0.2.0",
            "V0.2.0",
            "v0.2.0-rc.1",
            "v0.2",
            "v0.2.0.1",
            "v-1.0.0",
            "vx.y.z",
            "v 0.2.0",
        ] {
            assert_eq!(Version::parse_tag(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn ordering_compares_major_then_minor_then_patch() {
        let p = |s: &str| Version::parse_tag(s).unwrap();
        assert!(p("v0.2.0") > Version::parse("0.1.1").unwrap());
        assert!(p("v0.1.10") > Version::parse("0.1.9").unwrap());
        assert!(p("v1.0.0") > Version::parse("0.99.99").unwrap());
        assert_eq!(p("v0.2.0"), Version::parse("0.2.0").unwrap());
        assert!(p("v0.1.0") < Version::parse("0.1.1").unwrap());
        assert!(p("v0.9.0") < Version::parse("0.10.0").unwrap());
    }

    #[test]
    fn display_round_trips() {
        assert_eq!(v(0, 10, 3).to_string(), "0.10.3");
        assert_eq!(Version::parse(&v(7, 8, 9).to_string()), Some(v(7, 8, 9)));
    }

    #[test]
    fn current_version_matches_cargo_package_version() {
        let cur = current_version().expect("CARGO_PKG_VERSION 必須是 X.Y.Z");
        assert_eq!(cur.to_string(), env!("CARGO_PKG_VERSION"));
    }

    // ---- 2.2 latest release JSON 判定 ----

    fn release_json(tag: &str, assets: &[&str]) -> String {
        let assets: Vec<String> = assets
            .iter()
            .map(|n| {
                format!(r#"{{"name":"{n}","browser_download_url":"https://example.invalid/{n}"}}"#)
            })
            .collect();
        format!(
            r#"{{"tag_name":"{tag}","prerelease":false,"assets":[{}]}}"#,
            assets.join(",")
        )
    }

    fn judge(status: u16, body: &str, current: Version) -> CheckOutcome {
        judge_latest_release(status, body.as_bytes(), current)
    }

    fn no_update_reason(outcome: CheckOutcome) -> String {
        match outcome {
            CheckOutcome::NoUpdate(reason) => reason,
            other => panic!("預期沒有新版，實際 {other:?}"),
        }
    }

    const FULL_020: [&str; 3] = [
        "ai-cockpit-0.2.0-x64-setup.exe",
        "ai-cockpit-0.2.0-x64.zip",
        "SHA256SUMS.txt",
    ];

    #[test]
    fn judge_200_with_newer_version_and_both_assets_is_new_version() {
        let body = release_json("v0.2.0", &FULL_020);
        let outcome = judge(200, &body, v(0, 1, 1));
        assert_eq!(outcome, CheckOutcome::NewVersion(v(0, 2, 0)));
        assert_eq!(outcome.log_result(), "update available: 0.2.0");
    }

    #[test]
    fn judge_ignores_extra_fields_and_asset_order() {
        let body = r#"{"id":1,"tag_name":"v0.1.10","name":"x","assets":[
            {"name":"SHA256SUMS.txt","size":3},
            {"name":"ai-cockpit-0.1.10-x64-setup.exe","size":9}],"body":"notes"}"#;
        assert_eq!(
            judge(200, body, v(0, 1, 9)),
            CheckOutcome::NewVersion(v(0, 1, 10))
        );
    }

    #[test]
    fn judge_non_200_statuses_are_no_update_with_reason() {
        let good_body = release_json("v0.2.0", &FULL_020);
        for (status, expect) in [
            (404u16, "HTTP 404"),
            (403, "HTTP 403"),
            (429, "HTTP 429"),
            (500, "HTTP 500"),
            (301, "HTTP 301"),
            (204, "HTTP 204"),
        ] {
            // 本體即使是合法的新版 JSON，非 200 也不算數。
            let reason = no_update_reason(judge(status, &good_body, v(0, 1, 1)));
            assert!(reason.contains(expect), "{status}: {reason}");
            let reason = no_update_reason(judge(status, "", v(0, 1, 1)));
            assert!(reason.contains(expect), "{status}: {reason}");
        }
        assert!(no_update_reason(judge(404, "", v(0, 1, 1))).contains("no stable release"));
        assert!(no_update_reason(judge(403, "", v(0, 1, 1))).contains("rate limit"));
        assert!(no_update_reason(judge(429, "", v(0, 1, 1))).contains("rate limit"));
    }

    #[test]
    fn judge_bad_body_is_no_update() {
        let cur = v(0, 1, 1);
        assert!(no_update_reason(judge(200, "not json", cur)).contains("JSON"));
        assert!(no_update_reason(judge(200, "", cur)).contains("JSON"));
        assert!(
            no_update_reason(judge_latest_release(200, &[0xff, 0xfe, 0x00], cur)).contains("JSON")
        );
        // JSON 但不是物件。
        assert!(no_update_reason(judge(200, "[]", cur)).contains("tag_name"));
        // 缺 tag_name 或型別不對。
        assert!(no_update_reason(judge(200, r#"{"assets":[]}"#, cur)).contains("tag_name"));
        assert!(
            no_update_reason(judge(200, r#"{"tag_name":5,"assets":[]}"#, cur)).contains("tag_name")
        );
        assert!(no_update_reason(judge(200, r#"{"tag_name":null}"#, cur)).contains("tag_name"));
    }

    #[test]
    fn judge_unparsable_tag_is_no_update() {
        for tag in ["v0.2.0-rc.1", "0.2.0", "v0.2", "v0.2.0.1", "", "latest"] {
            let body = release_json(tag, &["ai-cockpit-0.2.0-x64-setup.exe", "SHA256SUMS.txt"]);
            let reason = no_update_reason(judge(200, &body, v(0, 1, 1)));
            assert!(reason.contains("tag_name"), "{tag}: {reason}");
        }
    }

    #[test]
    fn judge_version_not_greater_is_no_update() {
        let body = release_json("v0.2.0", &FULL_020);
        for current in [v(0, 2, 0), v(0, 2, 1), v(0, 10, 0), v(1, 0, 0)] {
            let reason = no_update_reason(judge(200, &body, current));
            assert!(reason.contains("not newer"), "{current}: {reason}");
            assert!(reason.contains("0.2.0"), "{reason}");
        }
    }

    #[test]
    fn judge_missing_any_required_asset_is_no_update() {
        let cur = v(0, 1, 1);
        // 缺安裝檔。
        let body = release_json("v0.2.0", &["ai-cockpit-0.2.0-x64.zip", "SHA256SUMS.txt"]);
        let reason = no_update_reason(judge(200, &body, cur));
        assert!(
            reason.contains("ai-cockpit-0.2.0-x64-setup.exe"),
            "{reason}"
        );
        // 缺雜湊檔。
        let body = release_json("v0.2.0", &["ai-cockpit-0.2.0-x64-setup.exe"]);
        let reason = no_update_reason(judge(200, &body, cur));
        assert!(reason.contains("SHA256SUMS.txt"), "{reason}");
        // 安裝檔名的版本不符（資產是舊版的）。
        let body = release_json(
            "v0.2.0",
            &["ai-cockpit-0.1.1-x64-setup.exe", "SHA256SUMS.txt"],
        );
        assert!(matches!(judge(200, &body, cur), CheckOutcome::NoUpdate(_)));
        // 沒有 assets 欄位、assets 不是陣列、項目沒有 name、清單為空。
        for body in [
            r#"{"tag_name":"v0.2.0"}"#,
            r#"{"tag_name":"v0.2.0","assets":"x"}"#,
            r#"{"tag_name":"v0.2.0","assets":[{"size":1},5,null]}"#,
            r#"{"tag_name":"v0.2.0","assets":[]}"#,
        ] {
            assert!(
                matches!(judge(200, body, cur), CheckOutcome::NoUpdate(_)),
                "{body}"
            );
        }
    }

    #[test]
    fn no_update_log_result_is_the_reason() {
        assert_eq!(
            CheckOutcome::NoUpdate("timed out".to_string()).log_result(),
            "timed out"
        );
    }

    #[test]
    fn installer_file_name_follows_release_contract() {
        assert_eq!(
            installer_file_name(v(0, 2, 0)),
            "ai-cockpit-0.2.0-x64-setup.exe"
        );
    }

    // ---- 2.3 SHA256SUMS.txt 解析與比對 ----

    const HASH_A: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    const HASH_B: &str = "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210";
    const SETUP: &str = "ai-cockpit-0.2.0-x64-setup.exe";

    /// release.yml「Checksums」步驟實際寫出的格式：依檔名排序、小寫 hex、兩個空白、LF、結尾換行。
    fn release_sums() -> String {
        format!("{HASH_A}  {SETUP}\n{HASH_B}  ai-cockpit-0.2.0-x64.zip\n")
    }

    #[test]
    fn sums_finds_hash_in_release_yml_format() {
        let sums = release_sums();
        assert_eq!(find_expected_sha256(&sums, SETUP).as_deref(), Ok(HASH_A));
        assert_eq!(
            find_expected_sha256(&sums, "ai-cockpit-0.2.0-x64.zip").as_deref(),
            Ok(HASH_B)
        );
    }

    #[test]
    fn sums_accepts_missing_final_newline_and_normalizes_uppercase_to_lower() {
        let upper = HASH_A.to_uppercase();
        let sums = format!("{upper}  {SETUP}");
        assert_eq!(find_expected_sha256(&sums, SETUP).as_deref(), Ok(HASH_A));
    }

    #[test]
    fn sums_filename_with_inner_spaces_is_compared_exactly() {
        let sums = format!("{HASH_A}  my file.exe\n");
        assert_eq!(
            find_expected_sha256(&sums, "my file.exe").as_deref(),
            Ok(HASH_A)
        );
    }

    #[test]
    fn sums_not_found_cases() {
        // 空檔、只有別的檔名、檔名只是子字串（前綴／後綴）、大小寫不同。
        for sums in [
            String::new(),
            format!("{HASH_A}  other.exe\n"),
            format!("{HASH_A}  {SETUP}.sig\n"),
            format!("{HASH_A}  x{SETUP}\n"),
            format!("{HASH_A}  AI-COCKPIT-0.2.0-X64-SETUP.EXE\n"),
        ] {
            assert_eq!(
                find_expected_sha256(&sums, SETUP),
                Err(SumsError::NotFound {
                    file: SETUP.to_string()
                }),
                "{sums:?}"
            );
        }
    }

    #[test]
    fn sums_duplicate_entries_are_an_error() {
        let sums = format!("{HASH_A}  {SETUP}\n{HASH_B}  {SETUP}\n");
        assert_eq!(
            find_expected_sha256(&sums, SETUP),
            Err(SumsError::Duplicate {
                file: SETUP.to_string()
            })
        );
        // 同雜湊重複也算。
        let sums = format!("{HASH_A}  {SETUP}\n{HASH_A}  {SETUP}\n");
        assert!(matches!(
            find_expected_sha256(&sums, SETUP),
            Err(SumsError::Duplicate { .. })
        ));
    }

    /// 檔名欄恰為目標檔名的那一行必須完全合格；格式錯就是 `Malformed`（含行號）。
    #[test]
    fn sums_malformed_target_line_is_an_error_with_line_number() {
        let short_hash = format!("{}  {SETUP}\n", &HASH_A[..63]);
        let non_hex = format!("{}g  {SETUP}\n", &HASH_A[..63]);
        for (sums, line) in [
            // BOM 只會出現在第一行；第一行就是目標行時失敗。
            (format!("\u{feff}{HASH_A}  {SETUP}\n"), 1),
            (format!("{HASH_A}  {SETUP}\r\n"), 1),
            (format!("{HASH_A}   {SETUP}\n"), 1),
            (format!("{HASH_A} {SETUP}\n"), 1),
            (format!("{HASH_A}\t{SETUP}\n"), 1),
            (format!("{HASH_A} *{SETUP}\n"), 1),
            (format!("{HASH_A}  *{SETUP}\n"), 1),
            (short_hash, 1),
            (format!("{HASH_A}0  {SETUP}\n"), 1),
            (non_hex, 1),
            (format!("{HASH_A}  {SETUP} \n"), 1),
            (format!("{HASH_A}  {SETUP}\t\n"), 1),
            (format!("{HASH_B}  a.zip\n{HASH_A}  {SETUP}\r\n"), 2),
            (format!("{HASH_B}  a.zip\n\n{HASH_A} {SETUP}"), 3),
        ] {
            assert_eq!(
                find_expected_sha256(&sums, SETUP),
                Err(SumsError::Malformed { line }),
                "{sums:?}"
            );
        }
    }

    /// 其他行格式錯（含 BOM、CRLF、空行、垃圾行、只有雜湊）一律略過，不影響目標行。
    #[test]
    fn sums_malformed_other_lines_are_skipped() {
        for sums in [
            format!("{HASH_A}  {SETUP}\n\n"),
            format!("{HASH_B}  a.zip\n\n{HASH_A}  {SETUP}\n"),
            format!("{HASH_B}  a.zip\ngarbage\n{HASH_A}  {SETUP}\n"),
            format!("\u{feff}{HASH_B}  a.zip\n{HASH_A}  {SETUP}\n"),
            format!("{HASH_B}  a.zip\r\n{HASH_A}  {SETUP}\n"),
            format!("{HASH_B} *a.zip\n{HASH_A}  {SETUP}"),
            format!("{HASH_A}  {SETUP}\n{HASH_B}\n"),
            format!("{}  a.zip\n{HASH_A}  {SETUP}\n", &HASH_B[..10]),
            format!("\n\n{HASH_A}  {SETUP}\n\n"),
        ] {
            assert_eq!(
                find_expected_sha256(&sums, SETUP).as_deref(),
                Ok(HASH_A),
                "{sums:?}"
            );
        }
    }

    /// 只有格式錯的其他行、或空行，等於沒有目標行。
    #[test]
    fn sums_only_unparsable_lines_is_not_found() {
        for sums in [
            "\n".to_string(),
            "\n\n".to_string(),
            "garbage\n".to_string(),
            format!("{HASH_A}  \n"),
            format!("\u{feff}{HASH_A}  a.zip\n"),
        ] {
            assert_eq!(
                find_expected_sha256(&sums, SETUP),
                Err(SumsError::NotFound {
                    file: SETUP.to_string()
                }),
                "{sums:?}"
            );
        }
    }

    /// 能解析的同名行參與重複檢查：一行合格、一行格式錯但檔名是目標 → 以格式錯失敗。
    #[test]
    fn sums_duplicate_target_line_with_bad_format_is_malformed() {
        let sums = format!("{HASH_A}  {SETUP}\n{HASH_B}   {SETUP}\n");
        assert_eq!(
            find_expected_sha256(&sums, SETUP),
            Err(SumsError::Malformed { line: 2 })
        );
    }

    #[test]
    fn sums_error_messages_are_readable() {
        assert_eq!(
            SumsError::Malformed { line: 2 }.to_string(),
            "SHA256SUMS.txt line 2 is malformed"
        );
        assert_eq!(
            SumsError::NotFound {
                file: "a.exe".into()
            }
            .to_string(),
            "SHA256SUMS.txt has no entry for a.exe"
        );
        assert_eq!(
            SumsError::Duplicate {
                file: "a.exe".into()
            }
            .to_string(),
            "SHA256SUMS.txt has more than one entry for a.exe"
        );
    }

    #[test]
    fn sha256_matches_ignores_case_only() {
        assert!(sha256_matches(HASH_A, HASH_A));
        assert!(sha256_matches(HASH_A, &HASH_A.to_uppercase()));
        assert!(sha256_matches(&HASH_A.to_uppercase(), HASH_A));
        assert!(!sha256_matches(HASH_A, HASH_B));
        assert!(!sha256_matches(HASH_A, &HASH_A[..63]));
        assert!(!sha256_matches(HASH_A, &format!("{HASH_A} ")));
        assert!(!sha256_matches("", HASH_A));
    }

    #[test]
    fn hex_lower_encodes_bytes_and_matches_sha2() {
        use sha2::{Digest, Sha256};
        assert_eq!(hex_lower(&[]), "");
        assert_eq!(hex_lower(&[0x00, 0x0f, 0xa0, 0xff]), "000fa0ff");
        // SHA-256("abc") 的標準測試向量（FIPS 180-2）。
        assert_eq!(
            hex_lower(&Sha256::digest(b"abc")),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    // ---- 2.4 更新檢查紀錄與節流 ----

    #[test]
    fn record_round_trips_through_json() {
        let record = CheckRecord {
            checked_at: 1_790_000_000,
            result: "update available: 0.2.0".to_string(),
        };
        let text = render_record(&record);
        assert_eq!(
            text,
            r#"{"checked_at":1790000000,"result":"update available: 0.2.0"}"#
        );
        assert_eq!(parse_record(&text), Some(record));
    }

    #[test]
    fn record_parse_tolerates_extra_fields_and_missing_result_but_not_junk() {
        assert_eq!(
            parse_record(r#"{"checked_at":5,"result":"x","extra":[1]}"#),
            Some(CheckRecord {
                checked_at: 5,
                result: "x".to_string()
            })
        );
        assert_eq!(
            parse_record(r#"{"checked_at":5}"#),
            Some(CheckRecord {
                checked_at: 5,
                result: String::new()
            })
        );
        for bad in [
            "",
            "not json",
            "{}",
            "[]",
            r#"{"result":"x"}"#,
            r#"{"checked_at":"5"}"#,
            r#"{"checked_at":-1}"#,
            r#"{"checked_at":1.5}"#,
            r#"{"checked_at":null}"#,
            r#"{"checked_at":5,"result":7}"#,
        ] {
            assert_eq!(parse_record(bad), None, "{bad:?}");
        }
    }

    fn rec(checked_at: u64) -> CheckRecord {
        CheckRecord {
            checked_at,
            result: String::new(),
        }
    }

    #[test]
    fn check_is_due_when_record_missing_or_unreadable() {
        assert!(is_check_due(None, 1_000_000));
        assert!(is_check_due(parse_record("junk").as_ref(), 1_000_000));
    }

    #[test]
    fn check_due_boundaries_at_exactly_24_hours() {
        let checked_at = 1_000_000;
        // 剛好 24 小時：已滿；差 1 秒：未滿；同一秒：未滿。
        assert!(is_check_due(
            Some(&rec(checked_at)),
            checked_at + CHECK_INTERVAL_SECS
        ));
        assert!(!is_check_due(
            Some(&rec(checked_at)),
            checked_at + CHECK_INTERVAL_SECS - 1
        ));
        assert!(!is_check_due(Some(&rec(checked_at)), checked_at));
        assert!(is_check_due(
            Some(&rec(checked_at)),
            checked_at + CHECK_INTERVAL_SECS + 1
        ));
        assert_eq!(CHECK_INTERVAL_SECS, 86_400);
    }

    #[test]
    fn check_is_due_when_recorded_time_is_in_the_future() {
        assert!(is_check_due(Some(&rec(2_000_000)), 1_000_000));
        // 未來只差 1 秒也算（不能被「差一點」擋住）。
        assert!(is_check_due(Some(&rec(1_000_001)), 1_000_000));
        // 極端值不 panic。
        assert!(is_check_due(Some(&rec(u64::MAX)), 0));
        assert!(!is_check_due(Some(&rec(0)), 1));
    }

    // ---- 2.5 網址組裝與測試覆寫 ----

    fn env_of<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
        move |key| {
            pairs
                .iter()
                .find(|(k, _)| *k == key)
                .map(|(_, v)| (*v).to_string())
        }
    }

    #[test]
    fn default_urls_are_https_and_point_at_this_repo() {
        let none = env_of(&[]);
        let api = latest_release_url(&none);
        assert_eq!(
            api,
            "https://api.github.com/repos/Benjamin-Teng/ai-cockpit/releases/latest"
        );
        let base = download_base(&none);
        assert_eq!(
            base,
            "https://github.com/Benjamin-Teng/ai-cockpit/releases/download"
        );
        assert!(api.starts_with("https://") && base.starts_with("https://"));
        assert_eq!(
            download_url(&base, v(0, 2, 0), "SHA256SUMS.txt"),
            "https://github.com/Benjamin-Teng/ai-cockpit/releases/download/v0.2.0/SHA256SUMS.txt"
        );
        assert_eq!(
            download_url(&base, v(0, 2, 0), &installer_file_name(v(0, 2, 0))),
            "https://github.com/Benjamin-Teng/ai-cockpit/releases/download/v0.2.0/ai-cockpit-0.2.0-x64-setup.exe"
        );
    }

    #[test]
    fn empty_override_values_count_as_unset() {
        let env = env_of(&[(ENV_API_URL, ""), (ENV_DOWNLOAD_BASE, "")]);
        assert_eq!(latest_release_url(&env), LATEST_RELEASE_API_URL);
        assert_eq!(download_base(&env), DOWNLOAD_BASE_URL);
    }

    #[test]
    fn overrides_replace_urls_and_allow_http() {
        let env = env_of(&[
            (ENV_API_URL, "http://127.0.0.1:8123/latest"),
            (ENV_DOWNLOAD_BASE, "http://127.0.0.1:8123/dl/"),
        ]);
        assert_eq!(latest_release_url(&env), "http://127.0.0.1:8123/latest");
        let base = download_base(&env);
        // 結尾的 `/` 去掉，避免組出 `//v0.2.0`。
        assert_eq!(base, "http://127.0.0.1:8123/dl");
        assert_eq!(
            download_url(&base, v(0, 2, 0), "a.exe"),
            "http://127.0.0.1:8123/dl/v0.2.0/a.exe"
        );
    }

    #[test]
    fn overrides_are_independent_of_each_other() {
        let env = env_of(&[(ENV_API_URL, "http://x/api")]);
        assert_eq!(latest_release_url(&env), "http://x/api");
        assert_eq!(download_base(&env), DOWNLOAD_BASE_URL);
    }

    #[test]
    fn env_var_and_file_names_are_the_documented_ones() {
        assert_eq!(ENV_NO_UPDATE_CHECK, "COCKPIT_NO_UPDATE_CHECK");
        assert_eq!(ENV_API_URL, "COCKPIT_UPDATE_API_URL");
        assert_eq!(ENV_DOWNLOAD_BASE, "COCKPIT_UPDATE_DOWNLOAD_BASE");
        assert_eq!(ENV_UPDATE_ANSWER, "COCKPIT_LAUNCH_UPDATE_ANSWER");
        assert_eq!(STATE_FILE_NAME, "cockpit.update.json");
        assert_eq!(SUMS_FILE_NAME, "SHA256SUMS.txt");
    }

    #[test]
    fn update_check_disabled_only_when_non_empty() {
        assert!(!update_check_disabled(&env_of(&[])));
        assert!(!update_check_disabled(&env_of(&[(
            ENV_NO_UPDATE_CHECK,
            ""
        )])));
        assert!(update_check_disabled(&env_of(&[(
            ENV_NO_UPDATE_CHECK,
            "1"
        )])));
        assert!(update_check_disabled(&env_of(&[(
            ENV_NO_UPDATE_CHECK,
            "0"
        )])));
        assert!(update_check_disabled(&env_of(&[(
            ENV_NO_UPDATE_CHECK,
            "yes"
        )])));
    }

    #[test]
    fn update_answer_is_yes_only_for_yes() {
        assert!(!update_answer_is_yes(&env_of(&[])));
        for no in ["", "no", "No", "y", "true", "1", "yes please"] {
            assert!(
                !update_answer_is_yes(&env_of(&[(ENV_UPDATE_ANSWER, no)])),
                "{no:?}"
            );
        }
        for yes in ["yes", "YES", "Yes", " yes "] {
            assert!(
                update_answer_is_yes(&env_of(&[(ENV_UPDATE_ANSWER, yes)])),
                "{yes:?}"
            );
        }
    }

    #[test]
    fn user_agent_names_the_product_and_current_version() {
        assert_eq!(
            user_agent(),
            format!("ai-cockpit/{}", env!("CARGO_PKG_VERSION"))
        );
        assert_eq!(API_ACCEPT, "application/vnd.github+json");
    }

    // ---- 3.x 啟動器 I/O 用的路徑、參數與上限 ----

    #[test]
    fn limits_and_timeouts_follow_design_d6() {
        assert_eq!(QUERY_TIMEOUT, Duration::from_secs(3));
        assert_eq!(SUMS_TIMEOUT, Duration::from_secs(15));
        assert_eq!(INSTALLER_TIMEOUT, Duration::from_secs(300));
        assert_eq!(MAX_QUERY_BODY_BYTES, 1024 * 1024);
        assert_eq!(MAX_SUMS_BYTES, 64 * 1024);
        assert_eq!(MAX_INSTALLER_BYTES, 200 * 1024 * 1024);
        assert_eq!(TEMP_DIR_NAME, "ai-cockpit-update");
        assert_eq!(UNINSTALLER_FILE_NAME, "unins000.exe");
    }

    #[test]
    fn uninstaller_is_next_to_the_launcher() {
        let launcher = Path::new(r"C:\Apps\Cockpit\cockpit-launch.exe");
        assert_eq!(
            uninstaller_path(launcher),
            Some(PathBuf::from(r"C:\Apps\Cockpit\unins000.exe"))
        );
        assert_eq!(uninstaller_path(Path::new("")), None);
    }

    #[test]
    fn state_file_sits_next_to_cockpit_log() {
        assert_eq!(
            state_path(Path::new(r"C:\Data\cockpit.log")),
            PathBuf::from(r"C:\Data\cockpit.update.json")
        );
        assert_eq!(
            state_path(Path::new("cockpit.log")),
            PathBuf::from("cockpit.update.json")
        );
    }

    #[test]
    fn https_only_unless_a_url_override_is_set() {
        let none = |_: &str| None;
        assert!(https_only(&none), "沒有覆寫：只允許 https");
        let api = |key: &str| (key == ENV_API_URL).then(|| "http://127.0.0.1:1/x".to_string());
        assert!(!https_only(&api), "覆寫查詢網址：允許 http（測試用）");
        let base = |key: &str| (key == ENV_DOWNLOAD_BASE).then(|| "http://127.0.0.1:1".to_string());
        assert!(!https_only(&base), "覆寫下載基底：允許 http（測試用）");
        let empty = |key: &str| (key == ENV_API_URL || key == ENV_DOWNLOAD_BASE).then(String::new);
        assert!(https_only(&empty), "空字串等於沒設定");
    }

    /// `https_only` 為真的 agent 對 `http://` 不連線就拒絕；為假的 agent 照常連到本機假伺服器。
    #[test]
    fn https_only_agent_refuses_http_and_test_agent_allows_it() {
        use std::io::{Read as _, Write as _};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/", listener.local_addr().unwrap());
        // 只接受一次連線：若 https_only 的 agent 也連了線，第二個請求會等不到回應而逾時失敗。
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0u8; 4096];
            let _ = stream.read(&mut request);
            stream
                .write_all(
                    b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                )
                .unwrap();
        });

        let error = http_agent(Duration::from_secs(3), true)
            .get(&url)
            .call()
            .unwrap_err();
        assert!(matches!(error, ureq::Error::RequireHttpsOnly(_)), "{error}");

        // 不經使用者環境的代理變數，直接連本機。
        let response = http_agent(Duration::from_secs(3), false)
            .get(&url)
            .config()
            .proxy(None)
            .build()
            .call()
            .unwrap();
        assert_eq!(response.status().as_u16(), 204);
        server.join().unwrap();
    }

    #[test]
    fn work_dir_is_one_subfolder_per_process() {
        assert_eq!(
            work_dir(Path::new(r"C:\Temp"), 4242),
            PathBuf::from(r"C:\Temp\ai-cockpit-update\4242")
        );
    }

    #[test]
    fn installer_args_are_design_d8_verbatim() {
        let args: Vec<String> = installer_args(Path::new(r"C:\Temp\ai-cockpit-update\4242"))
            .into_iter()
            .map(|arg| arg.into_string().unwrap())
            .collect();
        assert_eq!(
            args,
            [
                "/SILENT",
                "/SUPPRESSMSGBOXES",
                "/NORESTART",
                "/NOCANCEL",
                "/COCKPITUPDATE=1",
                r"/LOG=C:\Temp\ai-cockpit-update\4242\setup.log",
            ]
        );
    }

    #[test]
    fn copy_hashed_copies_bytes_and_hashes_them() {
        use sha2::Digest;
        let data = b"abc".repeat(10_000);
        let mut out = Vec::new();
        let copied = copy_hashed(&data[..], &mut out, 1_000_000).unwrap();
        assert_eq!(out, data);
        assert_eq!(copied.bytes, data.len() as u64);
        assert_eq!(copied.sha256, hex_lower(&sha2::Sha256::digest(&data)));
    }

    #[test]
    fn copy_hashed_allows_exactly_the_limit_and_rejects_one_more() {
        let mut out = Vec::new();
        let copied = copy_hashed(&[7u8; 100][..], &mut out, 100).unwrap();
        assert_eq!(copied.bytes, 100);
        let mut out = Vec::new();
        let error = copy_hashed(&[7u8; 101][..], &mut out, 100).unwrap_err();
        assert!(
            matches!(error, CopyError::TooLarge { max: 100 }),
            "{error:?}"
        );
        assert!(out.len() <= 100, "超過上限後不得再寫入：{}", out.len());
        assert_eq!(error.to_string(), "the file is larger than 100 bytes");
    }

    #[test]
    fn copy_hashed_reports_read_and_write_errors_separately() {
        struct FailingReader;
        impl std::io::Read for FailingReader {
            fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
                Err(std::io::Error::other("connection reset"))
            }
        }
        struct FailingWriter;
        impl std::io::Write for FailingWriter {
            fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
                Err(std::io::Error::other("disk full"))
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let error = copy_hashed(FailingReader, &mut Vec::new(), 100).unwrap_err();
        assert!(matches!(error, CopyError::Read(_)), "{error:?}");
        let error = copy_hashed(&b"x"[..], FailingWriter, 100).unwrap_err();
        assert!(matches!(error, CopyError::Write(_)), "{error:?}");
    }
}
