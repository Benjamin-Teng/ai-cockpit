//! 本機預設 socket 路徑解析（spec「本機預設 socket 路徑解析」）。

use std::path::{Path, PathBuf};

/// HERDR 用來組設定目錄的固定資料夾名稱。真機 HERDR 的 `app_dir_name()`
/// （`src/config/io.rs:22-28`，clone 自 <https://github.com/herdrdev/herdr>，commit
/// `bafbc0949dd996cf7fd0848c8965e254348cc11e`）在 `cfg(debug_assertions)` 的 HERDR debug
/// build 回 `"herdr-dev"`，其餘（含一般發行版）回 `"herdr"`；本 crate 假設目標 HERDR 是非
/// debug build，固定用 `"herdr"`。若目標機器跑的是 HERDR 自己的 debug build，兩邊資料夾
/// 名稱對不上，不在本 change 處理範圍。
const HERDR_APP_DIR_NAME: &str = "herdr";

/// 純函數版本：依 HERDR 的規則解析預設 socket 路徑，環境變數與設定目錄以參數注入
/// （不在測試中改真實環境）。
///
/// 優先序：`HERDR_SOCKET_PATH` 優先，其次 `HERDR_SESSION`（對應
/// `<config_dir>/sessions/<name>/herdr.sock`），否則 `<config_dir>/herdr.sock`
/// （`docs/research/2026-09-13/herdr-source-findings.txt` §5 `active_api_socket_path()`）。
///
/// 注意：真機 HERDR（`session.rs` `active_name()`）會把字面值 `"default"` 或未通過
/// `validate_name` 的 session 名稱當成「沒有 session」處理；本函數不重現這個過濾（design 沒
/// 把它列進本 change 範圍）。呼叫端若真的傳入 `HERDR_SESSION=default`，這裡會回
/// `<config_dir>/sessions/default/herdr.sock`，與真機「視為未設定」的行為不同。
pub fn default_socket_path(
    lookup_env: impl Fn(&str) -> Option<String>,
    config_dir: &Path,
) -> PathBuf {
    if let Some(path) = lookup_env("HERDR_SOCKET_PATH") {
        return PathBuf::from(path);
    }
    if let Some(session) = lookup_env("HERDR_SESSION") {
        return config_dir.join("sessions").join(session).join("herdr.sock");
    }
    config_dir.join("herdr.sock")
}

/// 用真實環境變數解析預設 socket 路徑；設定目錄的規則對齊 HERDR 自己的
/// `config_dir()`／`platform_config_dir()`（task 2.3 查證結果，見 `herdr_config_dir` 的文件
/// 註解與來源引用，取代先前「待查證」的猜測）。
pub fn default_socket_path_from_env() -> PathBuf {
    let lookup_env = |key: &str| std::env::var(key).ok();
    let config_dir = herdr_config_dir(lookup_env);
    default_socket_path(lookup_env, &config_dir)
}

/// 依 HERDR 的規則解析設定目錄，環境變數以參數注入（測試用，不改真實環境）。與 HERDR
/// 原始碼（下方引用）逐行對齊：
///
/// 1. `XDG_CONFIG_HOME`——只要這個環境變數有設定就不分平台一律優先採用，接上
///    `herdr`（`src/config/io.rs:30-33`：`if let Ok(dir) = std::env::var("XDG_CONFIG_HOME")`）。
///    HERDR 原始碼沒有排除空字串：`XDG_CONFIG_HOME=""` 一樣算「有設定」，會拼出
///    `PathBuf::from("").join("herdr")`（相對路徑 `herdr`）這種邊界結果，這裡忠實重現。
/// 2. 否則依平台 fallback（`platform_config_dir()`，`src/config/io.rs:44-68`）：
///    - Windows：`APPDATA` → `USERPROFILE\AppData\Roaming` → `HOME`（接 `.config/herdr`，
///      HERDR 原始碼在 Windows 分支的 `HOME` fallback 仍然用 unix 風格的 `.config` 子目錄，
///      不是直接接 `herdr`）→ `std::env::temp_dir()`。
///    - unix：`HOME`（`.config/herdr`）→ `std::env::temp_dir()`。
///
/// 來源：clone 自 <https://github.com/herdrdev/herdr>，commit
/// `bafbc0949dd996cf7fd0848c8965e254348cc11e`（與
/// `docs/research/2026-09-13/herdr-source-findings.txt` 的查證同一個 commit）。跟 HERDR 原始碼
/// 一樣不會「找不到就放棄」：兩邊都查不到時退到系統暫存目錄，所以這裡回傳 `PathBuf`
/// 而非 `Option<PathBuf>`。
fn herdr_config_dir(lookup_env: impl Fn(&str) -> Option<String>) -> PathBuf {
    if let Some(dir) = lookup_env("XDG_CONFIG_HOME") {
        return PathBuf::from(dir).join(HERDR_APP_DIR_NAME);
    }
    platform_config_dir(&lookup_env)
}

#[cfg(windows)]
fn platform_config_dir(lookup_env: &impl Fn(&str) -> Option<String>) -> PathBuf {
    if let Some(dir) = lookup_env("APPDATA") {
        return PathBuf::from(dir).join(HERDR_APP_DIR_NAME);
    }
    if let Some(profile) = lookup_env("USERPROFILE") {
        return PathBuf::from(profile)
            .join("AppData")
            .join("Roaming")
            .join(HERDR_APP_DIR_NAME);
    }
    if let Some(home) = lookup_env("HOME") {
        return PathBuf::from(home).join(".config").join(HERDR_APP_DIR_NAME);
    }
    std::env::temp_dir().join(HERDR_APP_DIR_NAME)
}

#[cfg(unix)]
fn platform_config_dir(lookup_env: &impl Fn(&str) -> Option<String>) -> PathBuf {
    if let Some(home) = lookup_env("HOME") {
        PathBuf::from(home).join(".config").join(HERDR_APP_DIR_NAME)
    } else {
        std::env::temp_dir().join(HERDR_APP_DIR_NAME)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env(pairs: &'static [(&'static str, &'static str)]) -> impl Fn(&str) -> Option<String> {
        move |key| {
            pairs
                .iter()
                .find(|&&(k, _)| k == key)
                .map(|&(_, v)| v.to_string())
        }
    }

    #[test]
    fn default_path_without_env() {
        let config_dir = PathBuf::from("/config/herdr");
        let result = default_socket_path(env(&[]), &config_dir);
        assert_eq!(result, config_dir.join("herdr.sock"));
    }

    #[test]
    fn socket_path_env_wins() {
        let config_dir = PathBuf::from("/config/herdr");
        let result = default_socket_path(
            env(&[
                ("HERDR_SOCKET_PATH", "/custom/herdr.sock"),
                ("HERDR_SESSION", "dev"),
            ]),
            &config_dir,
        );
        assert_eq!(result, PathBuf::from("/custom/herdr.sock"));
    }

    #[test]
    fn session_env_maps_to_sessions_dir() {
        let config_dir = PathBuf::from("/config/herdr");
        let result = default_socket_path(env(&[("HERDR_SESSION", "dev")]), &config_dir);
        assert_eq!(
            result,
            config_dir.join("sessions").join("dev").join("herdr.sock")
        );
    }

    // -- task 2.3 fix round 1、finding 4：config_dir 查證後的行為驗證 -----------------

    #[test]
    fn herdr_config_dir_xdg_config_home_wins_over_platform_default() {
        let result = herdr_config_dir(env(&[
            ("XDG_CONFIG_HOME", "/custom/xdg"),
            ("HOME", "/home/someone"),
            ("APPDATA", r"C:\Users\someone\AppData\Roaming"),
        ]));
        assert_eq!(result, PathBuf::from("/custom/xdg").join("herdr"));
    }

    /// HERDR 原始碼用 `std::env::var("XDG_CONFIG_HOME")` 判斷「有沒有設定」，`Ok("")` 也算
    /// 有設定；即使結果是相對路徑 `herdr` 也照樣採用，不像本檔 fix 前的版本會排除空字串。
    #[test]
    fn herdr_config_dir_xdg_config_home_wins_even_when_empty() {
        let result = herdr_config_dir(env(&[("XDG_CONFIG_HOME", "")]));
        assert_eq!(result, PathBuf::from("").join("herdr"));
    }

    #[cfg(windows)]
    #[test]
    fn herdr_config_dir_windows_prefers_appdata() {
        let result = herdr_config_dir(env(&[
            ("APPDATA", r"C:\Users\test\AppData\Roaming"),
            ("USERPROFILE", r"C:\Users\test"),
        ]));
        assert_eq!(
            result,
            PathBuf::from(r"C:\Users\test\AppData\Roaming").join("herdr")
        );
    }

    #[cfg(windows)]
    #[test]
    fn herdr_config_dir_windows_falls_back_to_userprofile_then_home() {
        let via_userprofile = herdr_config_dir(env(&[("USERPROFILE", r"C:\Users\test")]));
        assert_eq!(
            via_userprofile,
            PathBuf::from(r"C:\Users\test")
                .join("AppData")
                .join("Roaming")
                .join("herdr")
        );

        let via_home = herdr_config_dir(env(&[("HOME", "/home/test")]));
        assert_eq!(
            via_home,
            PathBuf::from("/home/test").join(".config").join("herdr")
        );
    }

    #[cfg(unix)]
    #[test]
    fn herdr_config_dir_unix_uses_home_dot_config() {
        let result = herdr_config_dir(env(&[("HOME", "/home/test")]));
        assert_eq!(
            result,
            PathBuf::from("/home/test").join(".config").join("herdr")
        );
    }
}
