//! 桌面啟動器 `cockpit-launch`（desktop-launch-notify task 2.3；spec `desktop-launch`「啟動器」；
//! design D1、D3、D5）。
//!
//! 圖形子系統執行檔：啟動時不建立主控台視窗。依序做：解析引數與設定 → 選瀏覽器 → 偵測
//! Cockpit 是否已在執行 → （沒有就）背景啟動同目錄的 `cockpit --exit-when-idle` 並輪詢就緒 →
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
    use std::os::windows::process::CommandExt;
    use std::path::Path;
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    use cockpit::launch::{self, LaunchLang, LaunchPlan, LaunchText, ProbeOutcome, text};

    /// Windows `CREATE_NO_WINDOW`：後端不建立主控台視窗；不與 `DETACHED_PROCESS` 併用，孫程序
    /// （`wsl.exe`、git）因而繼承這個隱藏主控台、不閃窗（design D3；同 `cockpit-git/src/runner.rs`）。
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
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

        // 第 3、4 步：偵測，必要時背景啟動並等待就緒。
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
            ProbeOutcome::Unreachable(_) => start_backend_and_wait(&plan, lang)?,
        }

        // 第 5 步：開視窗，不等待瀏覽器結束。
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

    /// 第 4 步：背景啟動同目錄的 `cockpit`，每 200 毫秒偵測一次、最多 15 秒。
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

    /// 顯示錯誤訊息框；`COCKPIT_LAUNCH_DIALOG_FILE` 有值時改以 UTF-8 附加寫入該檔（design D5）。
    /// 寫檔失敗時不改顯示訊息框：這是自動驗收的入口，跳出會阻塞的訊息框反而讓驗收卡住。
    pub fn show_error(message: &str) {
        if let Some(path) = std::env::var_os(DIALOG_FILE_ENV).filter(|value| !value.is_empty()) {
            if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
                let _ = write!(file, "{message}\n\n");
            }
            return;
        }
        message_box(DIALOG_TITLE, message);
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

    const MB_OK: u32 = 0x0000_0000;
    const MB_ICONERROR: u32 = 0x0000_0010;

    fn message_box(title: &str, message: &str) {
        let wide = |text: &str| -> Vec<u16> { text.encode_utf16().chain(Some(0)).collect() };
        let text = wide(message);
        let caption = wide(title);
        // SAFETY：兩個指標都指向以 NUL 結尾、在呼叫期間存活的 UTF-16 緩衝區；hwnd 為 null
        // 表示沒有父視窗，是 MessageBoxW 允許的值。
        unsafe {
            MessageBoxW(
                std::ptr::null_mut(),
                text.as_ptr(),
                caption.as_ptr(),
                MB_OK | MB_ICONERROR,
            );
        }
    }
}
