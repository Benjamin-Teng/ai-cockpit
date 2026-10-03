//! 桌面啟動器 `cockpit-launch`（desktop-launch-notify task 2.3；spec `desktop-launch`「啟動器」；
//! design D1、D3、D5）。
//!
//! 圖形子系統執行檔：啟動時不建立主控台視窗。依序做：解析引數與設定 → 選瀏覽器 → 偵測
//! Cockpit 是否已在執行 → （沒有就）檢查更新（auto-update tasks 3.1–3.3；安裝版才檢查，有新版且使用者
//! 同意時下載驗證後交棒給安裝檔並以 0 結束）→ 背景啟動同目錄的 `cockpit --exit-when-idle` 並輪詢就緒 →
//! 以 `--app=<網址>` 開瀏覽器後結束。任何失敗都以錯誤訊息框結束；環境變數
//! `COCKPIT_LAUNCH_DIALOG_FILE` 有值時不顯示訊息框，改把訊息附加寫入該檔（自動驗收用，design D5）。
//!
//! 訊息語言依 Windows 使用者介面語言（ui-language design D6）；環境變數 `COCKPIT_LAUNCH_LANG=en|zh`
//! 可強制指定，僅供自動驗收（無法在測試中切換系統語言）。
//!
//! 可測的邏輯都在 [`cockpit::launch`]，這裡只做 I/O 串接。

#![windows_subsystem = "windows"]

use std::process::ExitCode;

/// 非 Windows 平台：只印說明並以非 0 結束（spec「啟動器」）。
#[cfg(not(windows))]
fn main() -> ExitCode {
    eprintln!("cockpit-launch 僅支援 Windows；其他平台請直接執行 cockpit。");
    ExitCode::FAILURE
}

#[cfg(windows)]
fn main() -> ExitCode {
    let lang = windows_launch::ui_lang();
    match windows_launch::run(lang) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            windows_launch::show_error(&message);
            ExitCode::FAILURE
        }
    }
}

#[cfg(windows)]
mod windows_launch {
    use std::fs::{File, OpenOptions};
    use std::io::Write;
    use std::os::windows::fs::OpenOptionsExt;
    use std::os::windows::process::CommandExt;
    use std::path::{Path, PathBuf};
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant, SystemTime};

    use cockpit::launch::{self, LaunchLang, LaunchPlan, LaunchText, ProbeOutcome, text};
    use cockpit::update::{self, CheckOutcome, CheckRecord, CopyError, Version};

    /// Windows `CREATE_NO_WINDOW`：後端不建立主控台視窗；不與 `DETACHED_PROCESS` 併用，孫程序
    /// （`wsl.exe`、git）因而繼承這個隱藏主控台、不閃窗（design D3；同 `cockpit-git/src/runner.rs`）。
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    /// Win32 `CreateFileW` 的 `dwShareMode`，查證自 Microsoft Learn「CreateFileW function (fileapi.h)」
    /// <https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-createfilew>（2026-10-03）：
    /// `FILE_SHARE_READ` 0x00000001、`FILE_SHARE_WRITE` 0x00000002；刻意不含 `FILE_SHARE_DELETE`
    /// （0x00000004，沒有它時「no process can open the file or device if it requests delete access」）。
    const FILE_SHARE_READ: u32 = 0x0000_0001;
    const FILE_SHARE_WRITE: u32 = 0x0000_0002;
    /// 就緒輪詢間隔（spec「啟動器」第 4 步）。
    const READY_POLL_INTERVAL: Duration = Duration::from_millis(200);
    /// 就緒輪詢上限（spec「啟動器」第 4 步）。
    const READY_TIMEOUT: Duration = Duration::from_secs(15);
    /// 測試入口：有值時訊息改寫入此檔（design D5）。
    const DIALOG_FILE_ENV: &str = "COCKPIT_LAUNCH_DIALOG_FILE";
    /// 訊息框標題（不隨語言變，spec「桌面啟動器訊息框語言」）。
    const DIALOG_TITLE: &str = "AI Agent Cockpit";
    /// 測試入口：`en`／`zh` 強制指定訊息語言（僅供自動驗收；不設時依系統語言）。
    const LANG_ENV: &str = "COCKPIT_LAUNCH_LANG";

    /// 訊息語言：`COCKPIT_LAUNCH_LANG` 有合法值時用它，否則依 Windows 使用者介面語言。
    pub fn ui_lang() -> LaunchLang {
        std::env::var(LANG_ENV)
            .ok()
            .and_then(|value| launch::parse_lang_override(&value))
            .unwrap_or_else(launch::system_launch_lang)
    }

    /// 啟動器本體；`Err` 是要顯示在訊息框的完整訊息（`lang` 語言）。
    pub fn run(lang: LaunchLang) -> Result<(), String> {
        stop_inheriting_std_handles();

        // 第 1 步：引數與設定。
        let argv = std::env::args_os()
            .skip(1)
            .map(|arg| {
                arg.into_string().map_err(|arg| {
                    text(
                        lang,
                        LaunchText::ArgNotUnicode {
                            arg: &format!("{arg:?}"),
                        },
                    )
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let args = launch::parse_launch_args(&argv, lang)?;
        let cwd = std::env::current_dir().map_err(|error| {
            text(
                lang,
                LaunchText::WorkingDirFailed {
                    detail: &error.to_string(),
                },
            )
        })?;
        let lookup_env = |key: &str| std::env::var(key).ok();
        let plan = launch::plan(&args, &cwd, &lookup_env, lang)?;

        // 第 2 步：瀏覽器（找不到時尚未啟動任何後端）。
        let browser = launch::select_browser(&lookup_env, &|path: &Path| path.is_file())
            .ok_or_else(|| no_browser_message(&lookup_env, lang))?;

        // 第 3～5 步：偵測；沒在執行時先檢查更新，再背景啟動並等待就緒。
        match launch::probe(plan.listen, lang) {
            ProbeOutcome::Cockpit => {}
            ProbeOutcome::NotCockpit(reason) => {
                return Err(text(
                    lang,
                    LaunchText::PortOccupied {
                        listen: &plan.listen.to_string(),
                        reason: &reason,
                    },
                ));
            }
            ProbeOutcome::Unreachable(_) => {
                // 第 4 步（auto-update）：交棒給安裝檔時在此以 0 結束，不啟動後端也不開瀏覽器。
                if check_and_update(&plan, lang) == UpdateStep::HandedOff {
                    return Ok(());
                }
                start_backend_and_wait(&plan, lang)?;
            }
        }

        // 第 6 步：開視窗，不等待瀏覽器結束。
        Command::new(&browser)
            .args(launch::browser_args(&plan.url))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|error| {
                text(
                    lang,
                    LaunchText::BrowserSpawnFailed {
                        browser: &browser.display().to_string(),
                        detail: &error.to_string(),
                    },
                )
            })?;
        Ok(())
    }

    /// 清掉啟動器自己標準輸入／輸出／錯誤 handle 的「可繼承」旗標（修正波 2.6）。
    ///
    /// Rust std 在 Windows 以 `bInheritHandles = TRUE` 建立子程序，啟動器所有可繼承的 handle 都會被
    /// 子程序繼承——包括呼叫端交給啟動器的 stdout／stderr pipe。後端會活到閒置結束，呼叫端若擷取
    /// 啟動器的輸出並等 EOF（Node `spawnSync`、`$(...)`、`| tee`），就要等到後端結束才返回。清掉旗標後，
    /// 後端只拿到 `Command` 明確指定的 handle（stdin 為空、stdout／stderr 為 `cockpit.log`），瀏覽器
    /// 同理（三者都是 `Stdio::null()`）。從 Explorer 捷徑啟動時圖形子系統程序沒有標準 handle
    /// （`GetStdHandle` 回 null），這裡什麼都不做。只影響繼承，不影響啟動器自己使用這些 handle。
    fn stop_inheriting_std_handles() {
        use std::os::windows::io::AsRawHandle;

        const HANDLE_FLAG_INHERIT: u32 = 0x0000_0001;
        let handles = [
            std::io::stdin().as_raw_handle(),
            std::io::stdout().as_raw_handle(),
            std::io::stderr().as_raw_handle(),
        ];
        for handle in handles {
            // null：沒有這個標準 handle；-1：INVALID_HANDLE_VALUE。
            if handle.is_null() || handle as isize == -1 {
                continue;
            }
            // SAFETY：handle 來自 GetStdHandle，是這個程序自己的有效 handle（上面已排除 null 與
            // INVALID_HANDLE_VALUE）；SetHandleInformation 只改旗標，失敗時回 0、不影響其他狀態，
            // 失敗頂多退回原本的繼承行為，所以忽略回傳值。
            unsafe {
                SetHandleInformation(handle, HANDLE_FLAG_INHERIT, 0);
            }
        }
    }

    /// 第 5 步：背景啟動同目錄的 `cockpit`，每 200 毫秒偵測一次、最多 15 秒。
    fn start_backend_and_wait(plan: &LaunchPlan, lang: LaunchLang) -> Result<(), String> {
        let launcher = std::env::current_exe().map_err(|error| {
            text(
                lang,
                LaunchText::LauncherPathFailed {
                    detail: &error.to_string(),
                },
            )
        })?;
        let exe = launch::backend_exe(&launcher);
        let log_create_failed = |error: std::io::Error| {
            text(
                lang,
                LaunchText::LogCreateFailed {
                    path: &plan.log_path.display().to_string(),
                    detail: &error.to_string(),
                },
            )
        };
        let log = File::create(&plan.log_path).map_err(log_create_failed)?;
        let log_err = log.try_clone().map_err(log_create_failed)?;
        let mut child = Command::new(&exe)
            .args(&plan.backend_args)
            .stdin(Stdio::null())
            .stdout(log)
            .stderr(log_err)
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
            .map_err(|error| {
                text(
                    lang,
                    LaunchText::BackendSpawnFailed {
                        exe: &exe.display().to_string(),
                        detail: &error.to_string(),
                    },
                )
            })?;

        let deadline = Instant::now() + READY_TIMEOUT;
        loop {
            match child.try_wait() {
                Ok(Some(status)) => {
                    // 提早結束：先再偵測一次——同時點兩次捷徑時，另一個啟動器的後端可能已就緒，
                    // 這個後端只是綁定失敗（spec「同時點兩次捷徑」）。
                    if launch::probe(plan.listen, lang) == ProbeOutcome::Cockpit {
                        return Ok(());
                    }
                    let reason = text(
                        lang,
                        LaunchText::BackendExitedEarly {
                            status: &status.to_string(),
                        },
                    );
                    return Err(failure(plan, &reason, lang));
                }
                Ok(None) => {}
                Err(error) => {
                    let reason = text(
                        lang,
                        LaunchText::BackendStatusFailed {
                            detail: &error.to_string(),
                        },
                    );
                    return Err(failure(plan, &reason, lang));
                }
            }
            if launch::probe(plan.listen, lang) == ProbeOutcome::Cockpit {
                return Ok(());
            }
            if Instant::now() >= deadline {
                // 15 秒仍未就緒：結束這個後端，不留下沒有視窗也不會被使用的程序
                // （它若卡在開始監聽之前，閒置計時不會啟動，就不會自行結束）。
                let _ = child.kill();
                let _ = child.wait();
                let reason = text(
                    lang,
                    LaunchText::BackendNotReady {
                        secs: READY_TIMEOUT.as_secs(),
                        url: &plan.url,
                    },
                );
                return Err(failure(plan, &reason, lang));
            }
            std::thread::sleep(READY_POLL_INTERVAL);
        }
    }

    /// 後端失敗的訊息：原因、`cockpit.log` 完整路徑與最後 20 行。
    fn failure(plan: &LaunchPlan, reason: &str, lang: LaunchLang) -> String {
        let text = std::fs::read(&plan.log_path)
            .ok()
            .map(|bytes| String::from_utf8_lossy(&bytes).into_owned());
        launch::backend_failure_message(reason, &plan.log_path, text.as_deref(), lang)
    }

    /// 找不到瀏覽器的訊息，列出找過的位置。
    fn no_browser_message(lookup_env: &dyn Fn(&str) -> Option<String>, lang: LaunchLang) -> String {
        let searched: Vec<String> = launch::browser_candidates(lookup_env)
            .iter()
            .map(|path| format!("  {}", path.display()))
            .collect();
        text(
            lang,
            LaunchText::NoBrowser {
                searched: &searched.join("\n"),
            },
        )
    }

    /// 更新流程（第 4 步）的去向。
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum UpdateStep {
        /// 照常背景啟動後端（不需檢查、沒有新版、選稍後、下載／驗證／啟動安裝檔失敗）。
        Continue,
        /// 已啟動安裝檔：啟動器以 0 結束，不啟動後端也不開瀏覽器。
        HandedOff,
    }

    /// 第 4 步（spec `auto-update`）：判定要不要檢查、查詢、詢問、下載驗證、交棒。任何失敗都
    /// 回 [`UpdateStep::Continue`]；需要讓使用者知道的失敗（下載後的步驟）先以錯誤訊息框說明。
    fn check_and_update(plan: &LaunchPlan, lang: LaunchLang) -> UpdateStep {
        let lookup_env = |key: &str| std::env::var(key).ok();

        // 由安裝檔安裝（design D2）：啟動器同目錄有 unins000.exe。
        let installed = std::env::current_exe()
            .ok()
            .and_then(|exe| update::uninstaller_path(&exe))
            .is_some_and(|path| path.is_file());
        if !installed {
            return UpdateStep::Continue;
        }
        // 清掉其他程序留下的暫存子資料夾（design D6）；不受節流與關閉檢查影響，所以更新後由安裝檔
        // 重新啟動的那次（24 小時內、不會再查詢）就會清掉上次下載的安裝檔。
        let temp_dir = std::env::temp_dir();
        remove_stale_work_dirs(&temp_dir, std::process::id());
        if update::update_check_disabled(&lookup_env) {
            return UpdateStep::Continue;
        }

        // 24 小時節流（design D5）。
        let state = update::state_path(&plan.log_path);
        let now = unix_now();
        let record = std::fs::read_to_string(&state)
            .ok()
            .as_deref()
            .and_then(update::parse_record);
        if !update::is_check_due(record.as_ref(), now) {
            return UpdateStep::Continue;
        }
        let Some(current) = update::current_version() else {
            // 理論上不會發生（crate 版本即 X.Y.Z）；略過檢查，原因留在紀錄供排查。
            write_record(&state, now, "current version is not X.Y.Z; check skipped");
            return UpdateStep::Continue;
        };

        // 發出查詢前先寫下時間（之後不論結果如何都算一次檢查），結果出來再覆寫原因。
        write_record(&state, now, update::RESULT_PENDING);
        let outcome = query_latest(&update::latest_release_url(&lookup_env), current);
        write_record(&state, now, &outcome.log_result());
        let CheckOutcome::NewVersion(latest) = outcome else {
            return UpdateStep::Continue;
        };

        let new_version = latest.to_string();
        let current_version = current.to_string();
        let prompt = text(
            lang,
            LaunchText::UpdatePrompt {
                new_version: &new_version,
                current_version: &current_version,
            },
        );
        if !ask_yes_no(&prompt) {
            return UpdateStep::Continue;
        }

        let work_dir = update::work_dir(&temp_dir, std::process::id());
        let installer = match download_and_verify(
            &update::download_base(&lookup_env),
            latest,
            current,
            &work_dir,
            lang,
        ) {
            Ok(installer) => installer,
            Err(message) => {
                show_error(&message);
                return UpdateStep::Continue;
            }
        };

        // 交棒（design D8）：三個標準串流皆 null（啟動器開頭已清掉自身 handle 的繼承旗標），不等待。
        match Command::new(&installer)
            .args(update::installer_args(&work_dir))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        {
            Ok(_) => UpdateStep::HandedOff,
            Err(error) => {
                let _ = std::fs::remove_file(&installer);
                show_error(&text(
                    lang,
                    LaunchText::UpdateLaunchFailed {
                        new_version: &new_version,
                        current_version: &current_version,
                        detail: &error.to_string(),
                    },
                ));
                UpdateStep::Continue
            }
        }
    }

    /// 現在時間（Unix 秒）；系統時鐘早於 1970 年時為 0（之後的紀錄會被當成「在未來」而重新檢查）。
    fn unix_now() -> u64 {
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_secs())
    }

    /// 寫更新檢查紀錄；寫入失敗不影響啟動（spec「何時檢查更新」）。
    fn write_record(path: &Path, checked_at: u64, result: &str) {
        let record = CheckRecord {
            checked_at,
            result: result.to_string(),
        };
        let _ = std::fs::write(path, update::render_record(&record));
    }

    /// ureq 的 agent（見 [`update::http_agent`]）：沒有網址覆寫變數時只允許 https（design D7）。
    fn http_agent(timeout: Duration) -> ureq::Agent {
        let https_only = update::https_only(&|key: &str| std::env::var(key).ok());
        update::http_agent(timeout, https_only)
    }

    /// 查詢最新正式版（spec「判定有無新版」）。沒有狀態碼的失敗（連不上、逾時、TLS）在這裡組原因；
    /// 有狀態碼的交給 [`update::judge_latest_release`]。
    fn query_latest(url: &str, current: Version) -> CheckOutcome {
        let response = http_agent(update::QUERY_TIMEOUT)
            .get(url)
            .header("Accept", update::API_ACCEPT)
            .call();
        let mut response = match response {
            Ok(response) => response,
            Err(ureq::Error::StatusCode(status)) => {
                return update::judge_latest_release(status, b"", current);
            }
            Err(error) => return CheckOutcome::NoUpdate(network_failure(&error)),
        };
        let status = response.status().as_u16();
        // ureq 的 limit 在讀滿上限後的下一次讀取就報錯，所以實際可接受的本體比上限少 1 位元組。
        match response
            .body_mut()
            .with_config()
            .limit(update::MAX_QUERY_BODY_BYTES)
            .read_to_vec()
        {
            Ok(body) => update::judge_latest_release(status, &body, current),
            Err(ureq::Error::BodyExceedsLimit(_)) => {
                CheckOutcome::NoUpdate("response too large".to_string())
            }
            Err(error) => CheckOutcome::NoUpdate(network_failure(&error)),
        }
    }

    /// 查詢失敗（沒有狀態碼）的紀錄原因。
    fn network_failure(error: &ureq::Error) -> String {
        match error {
            ureq::Error::Timeout(_) => "timed out".to_string(),
            other => format!("connection failed: {other}"),
        }
    }

    /// 下載失敗訊息的 `detail`（英文短句）。
    fn download_failure(error: &ureq::Error) -> String {
        match error {
            ureq::Error::StatusCode(status) => format!("HTTP {status}"),
            ureq::Error::Timeout(_) => "timed out".to_string(),
            ureq::Error::BodyExceedsLimit(max) => format!("the file is larger than {max} bytes"),
            other => other.to_string(),
        }
    }

    /// 下載 `SHA256SUMS.txt` 與安裝檔並比對雜湊（spec「下載與驗證」）；成功時回傳安裝檔路徑。
    /// `Err` 是要顯示的完整訊息；失敗時已刪除下載的安裝檔。
    fn download_and_verify(
        base: &str,
        latest: Version,
        current: Version,
        work_dir: &Path,
        lang: LaunchLang,
    ) -> Result<PathBuf, String> {
        let new_version = latest.to_string();
        let current_version = current.to_string();
        let download_failed = |detail: &str| {
            text(
                lang,
                LaunchText::UpdateDownloadFailed {
                    new_version: &new_version,
                    current_version: &current_version,
                    detail,
                },
            )
        };

        reset_dir(work_dir).map_err(|error| {
            download_failed(&format!("cannot prepare {}: {error}", work_dir.display()))
        })?;

        let sums = download_sums(&update::download_url(base, latest, update::SUMS_FILE_NAME))
            .map_err(|detail| download_failed(&detail))?;
        let installer_name = update::installer_file_name(latest);
        let expected = update::find_expected_sha256(&sums, &installer_name).map_err(|error| {
            text(
                lang,
                LaunchText::UpdateChecksumFailed {
                    new_version: &new_version,
                    current_version: &current_version,
                    detail: &error.to_string(),
                },
            )
        })?;

        let installer = work_dir.join(&installer_name);
        let installer_url = update::download_url(base, latest, &installer_name);
        let actual = match download_installer(&installer_url, &installer) {
            Ok(actual) => actual,
            Err(detail) => {
                let _ = std::fs::remove_file(&installer);
                return Err(download_failed(&detail));
            }
        };
        if !update::sha256_matches(&expected, &actual) {
            let _ = std::fs::remove_file(&installer);
            return Err(text(
                lang,
                LaunchText::UpdateHashMismatch {
                    new_version: &new_version,
                    current_version: &current_version,
                },
            ));
        }
        Ok(installer)
    }

    /// 刪除 `%TEMP%\ai-cockpit-update\` 下名稱不是 `own_pid` 的子資料夾（design D6）。被占用而刪不掉的
    /// 檔案（另一個啟動器正在下載的安裝檔——以不含 `FILE_SHARE_DELETE` 的方式開著，見 [`download_installer`]——
    /// 或正在執行的安裝檔）連同所在資料夾留著，不報錯，下次啟動再試；資料夾以外的項目不動。
    fn remove_stale_work_dirs(temp_dir: &Path, own_pid: u32) {
        let own = own_pid.to_string();
        let Ok(entries) = std::fs::read_dir(temp_dir.join(update::TEMP_DIR_NAME)) else {
            return;
        };
        for entry in entries.flatten() {
            let is_dir = entry.file_type().is_ok_and(|kind| kind.is_dir());
            if is_dir && entry.file_name() != own.as_str() {
                let _ = std::fs::remove_dir_all(entry.path());
            }
        }
    }

    /// 清空並重建這個程序的暫存子資料夾（design D6；同 pid 的舊資料夾屬於已結束的程序）。
    fn reset_dir(dir: &Path) -> std::io::Result<()> {
        match std::fs::remove_dir_all(dir) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        std::fs::create_dir_all(dir)
    }

    /// 下載 `SHA256SUMS.txt`（15 秒、64 KB）；`Err` 是失敗訊息的 `detail`。
    fn download_sums(url: &str) -> Result<String, String> {
        let mut response = http_agent(update::SUMS_TIMEOUT)
            .get(url)
            .call()
            .map_err(|error| download_failure(&error))?;
        let bytes = response
            .body_mut()
            .with_config()
            .limit(update::MAX_SUMS_BYTES)
            .read_to_vec()
            .map_err(|error| download_failure(&error))?;
        // 非 UTF-8 的位元組換成 U+FFFD：只會讓那一行解析失敗（目標行則報格式錯），不會誤判。
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }

    /// 串流下載安裝檔到 `dest`（300 秒、200 MB），回傳其 SHA-256；`Err` 是失敗訊息的 `detail`
    /// （呼叫端負責刪檔）。檔案在回傳前關閉，之後才能執行。
    fn download_installer(url: &str, dest: &Path) -> Result<String, String> {
        let response = http_agent(update::INSTALLER_TIMEOUT)
            .get(url)
            .call()
            .map_err(|error| download_failure(&error))?;
        // 下載期間不給別的程序刪除權（不含 FILE_SHARE_DELETE）：另一個啟動器開始時清理舊子資料夾
        // （remove_stale_work_dirs）刪不掉這個檔，就不會清掉下載到一半的安裝檔。
        let file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
            .open(dest)
            .map_err(|error| format!("cannot create {}: {error}", dest.display()))?;
        let reader = response.into_body().into_reader();
        match update::copy_hashed(reader, file, update::MAX_INSTALLER_BYTES) {
            Ok(copied) => Ok(copied.sha256),
            // 讀取錯誤可能包著 ureq 的錯誤（例如逾時），還原後再組原因。
            Err(CopyError::Read(error)) => Err(download_failure(&ureq::Error::from(error))),
            Err(error @ (CopyError::TooLarge { .. } | CopyError::Write(_))) => {
                Err(error.to_string())
            }
        }
    }

    /// 測試入口：`COCKPIT_LAUNCH_DIALOG_FILE` 有值時的路徑（design D5）。
    fn dialog_file() -> Option<std::ffi::OsString> {
        std::env::var_os(DIALOG_FILE_ENV).filter(|value| !value.is_empty())
    }

    /// 把訊息以 UTF-8 附加寫入對話框檔。寫檔失敗時不改顯示訊息框：這是自動驗收的入口，
    /// 跳出會阻塞的訊息框反而讓驗收卡住。
    fn append_dialog_file(path: &std::ffi::OsStr, message: &str) {
        if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
            let _ = write!(file, "{message}\n\n");
        }
    }

    /// 是／否詢問框，`IDYES` 才算同意（spec「詢問使用者」；關閉訊息框等同「否」）。
    /// `COCKPIT_LAUNCH_DIALOG_FILE` 有值時改寫檔，答案取 `COCKPIT_LAUNCH_UPDATE_ANSWER`
    /// （預設「否」，design D7）。
    fn ask_yes_no(message: &str) -> bool {
        if let Some(path) = dialog_file() {
            append_dialog_file(&path, message);
            return update::update_answer_is_yes(&|key: &str| std::env::var(key).ok());
        }
        // 詢問在查詢（最多 3 秒）之後才出現，使用者可能已切到別的視窗；啟動器沒有自己的視窗，
        // 沒有 MB_SETFOREGROUND 時訊息框可能落在其他視窗後面，使用者以為沒反應。
        message_box(
            DIALOG_TITLE,
            message,
            MB_YESNO | MB_ICONQUESTION | MB_SETFOREGROUND,
        ) == IDYES
    }

    /// 顯示錯誤訊息框；`COCKPIT_LAUNCH_DIALOG_FILE` 有值時改以 UTF-8 附加寫入該檔（design D5）。
    /// 同樣帶 `MB_SETFOREGROUND`：更新下載失敗的訊息可能在下載數分鐘後才出現，而且訊息框關掉前啟動器
    /// 不會繼續啟動後端——落在其他視窗後面，使用者會以為 Cockpit 沒開。
    pub fn show_error(message: &str) {
        if let Some(path) = dialog_file() {
            append_dialog_file(&path, message);
            return;
        }
        message_box(
            DIALOG_TITLE,
            message,
            MB_OK | MB_ICONERROR | MB_SETFOREGROUND,
        );
    }

    #[link(name = "user32")]
    unsafe extern "system" {
        /// Win32 `MessageBoxW`（user32.dll）。只用這一個函式，不為此加 `windows-sys` 依賴（design D5）。
        fn MessageBoxW(
            hwnd: *mut core::ffi::c_void,
            text: *const u16,
            caption: *const u16,
            utype: u32,
        ) -> i32;
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        /// Win32 `SetHandleInformation`（kernel32.dll）：`BOOL SetHandleInformation(HANDLE, DWORD, DWORD)`。
        /// 同 `MessageBoxW`，只用這一個函式，不加 `windows-sys` 依賴。
        fn SetHandleInformation(handle: *mut core::ffi::c_void, mask: u32, flags: u32) -> i32;
    }

    // 以下常數值查證自 Microsoft Learn「MessageBoxW function (winuser.h)」
    // <https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-messageboxw>
    // （2026-10-03）：MB_OK 0x00000000L、MB_YESNO 0x00000004L、MB_ICONERROR 0x00000010L、
    // MB_ICONQUESTION 0x00000020L、MB_SETFOREGROUND 0x00010000L（「The message box becomes the foreground
    // window. Internally, the system calls the SetForegroundWindow function for the message box.」；
    // 系統不允許時「Windows flashes the taskbar button of the window」，見 SetForegroundWindow 的 Remarks
    // <https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setforegroundwindow>）；
    // 回傳值 IDYES 為 6。
    const MB_OK: u32 = 0x0000_0000;
    const MB_YESNO: u32 = 0x0000_0004;
    const MB_ICONERROR: u32 = 0x0000_0010;
    const MB_ICONQUESTION: u32 = 0x0000_0020;
    const MB_SETFOREGROUND: u32 = 0x0001_0000;
    const IDYES: i32 = 6;

    /// 顯示訊息框並回傳按下的按鈕（`MessageBoxW` 的回傳值；失敗時為 0）。
    fn message_box(title: &str, message: &str, utype: u32) -> i32 {
        let wide = |text: &str| -> Vec<u16> { text.encode_utf16().chain(Some(0)).collect() };
        let text = wide(message);
        let caption = wide(title);
        // SAFETY：兩個指標都指向以 NUL 結尾、在呼叫期間存活的 UTF-16 緩衝區；hwnd 為 null
        // 表示沒有父視窗，是 MessageBoxW 允許的值。
        unsafe { MessageBoxW(std::ptr::null_mut(), text.as_ptr(), caption.as_ptr(), utype) }
    }
}
