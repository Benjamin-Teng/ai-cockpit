//! 測試用假安裝檔／假瀏覽器（auto-update tasks 3.1–3.3）：只給 `cockpit/tests/launch_update.rs` 用，
//! 不進正式安裝包（打包只收 `cockpit.exe` 與 `cockpit-launch.exe`）。`cargo test` 會順便建置 examples，
//! 測試從 `target/<profile>/examples/` 取用本執行檔。
//!
//! 行為：
//!
//! 1. 把自己收到的引數（`std::env::args_os` 解析後，每行一個）寫到
//!    `$COCKPIT_TEST_FAKE_RECORD_DIR/<執行檔主檔名>.args`；Windows 上另寫原始命令列
//!    （`GetCommandLineW`）到 `<主檔名>.cmdline`。先寫暫存檔再改名，讀的一方不會讀到半份。
//! 2. 睡 `COCKPIT_TEST_FAKE_SLEEP_MS` 毫秒（未設為 0）。
//! 3. 寫 `<主檔名>.done`，以 0 結束。
//!
//! 沒設 `COCKPIT_TEST_FAKE_RECORD_DIR` 時什麼都不寫，立即結束。

use std::path::Path;
use std::time::Duration;

fn main() {
    let Some(dir) = std::env::var_os("COCKPIT_TEST_FAKE_RECORD_DIR") else {
        return;
    };
    let dir = Path::new(&dir);
    let exe = std::env::current_exe().expect("current_exe");
    let stem = exe
        .file_stem()
        .expect("執行檔有主檔名")
        .to_string_lossy()
        .into_owned();

    let args: Vec<String> = std::env::args_os()
        .skip(1)
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();
    write_atomically(dir, &format!("{stem}.args"), &args.join("\n"));
    if let Some(cmdline) = raw_command_line() {
        write_atomically(dir, &format!("{stem}.cmdline"), &cmdline);
    }

    let sleep_ms = std::env::var("COCKPIT_TEST_FAKE_SLEEP_MS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(0);
    std::thread::sleep(Duration::from_millis(sleep_ms));
    write_atomically(dir, &format!("{stem}.done"), "");
}

fn write_atomically(dir: &Path, name: &str, content: &str) {
    let tmp = dir.join(format!("{name}.tmp"));
    std::fs::write(&tmp, content).expect("寫入紀錄暫存檔");
    std::fs::rename(&tmp, dir.join(name)).expect("紀錄檔改名");
}

#[cfg(windows)]
fn raw_command_line() -> Option<String> {
    #[link(name = "kernel32")]
    unsafe extern "system" {
        /// Win32 `GetCommandLineW`（kernel32.dll）：`LPWSTR GetCommandLineW(void)`。
        fn GetCommandLineW() -> *const u16;
    }
    // SAFETY：GetCommandLineW 回傳程序存活期間有效、以 NUL 結尾的 UTF-16 字串；只讀到 NUL 為止。
    unsafe {
        let ptr = GetCommandLineW();
        if ptr.is_null() {
            return None;
        }
        let mut len = 0;
        while *ptr.add(len) != 0 {
            len += 1;
        }
        Some(String::from_utf16_lossy(std::slice::from_raw_parts(
            ptr, len,
        )))
    }
}

#[cfg(not(windows))]
fn raw_command_line() -> Option<String> {
    None
}
