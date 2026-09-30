//! `cockpit-git` 執行器（git-review task 2.2）測試專用假程式：不呼叫真正的 git 或
//! `wsl.exe`、也不經任何 shell，只依 argv 模擬逾時、輸出上限、以及各種 exit code／stderr
//! 情境。只給 `cockpit-git/tests/runner.rs` 用，不是給使用者跑的工具；只在 `test-support`
//! feature 下建置（見 `Cargo.toml`「[[bin]]」節），不會進一般 `cargo build -p cockpit-git`。
//!
//! 用法（argv\[1\] 選模式）：
//! - `sleep <ms>`：睡滿 `<ms>` 毫秒後以 0 結束，不輸出任何內容。
//! - `sleep-record-pid <pid_file> <ms>`：先把自己的 PID（十進位字串）寫進 `<pid_file>`
//!   （建立／截斷後立即 flush、關閉），再睡滿 `<ms>` 毫秒才以 0 結束——讓測試在執行器判定
//!   逾時之後去查這個 PID 是否已從行程表消失，證明子程序真的被終止（brief 驗收要求：
//!   「逾時後要確定子程序已被終止」）。
//! - `stdout-bytes <n>`：把 `n` 個 `b'a'` 位元組一次寫進 stdout、flush 後以 0 結束——用來測
//!   執行器的 stdout 上限與截斷（`n` 故意選得比要測的上限大一點，位元組數遠小於 OS pipe
//!   緩衝區，寫入不會被阻塞卡住）。
//! - `exit-with <code> <text>`：把 `<text>` 印一行到 stderr 後以 `<code>` 結束——用來測錯誤
//!   分類（dubious ownership、其他非零）；測試呼叫端直接照抄
//!   `docs/research/2026-09-28/git-review-probe.md` ⑦ 記錄的 stderr 原文。
//! - `stderr-then-stdout <stderr_bytes> <stdout_bytes>`：先把 `<stderr_bytes>` 個 `b'e'` 寫進 stderr
//!   （遠大於執行器保留的 8 KiB，模擬 git 大量 CRLF 警告），寫入失敗（讀端已關閉）就以 13 結束
//!   （模擬 git 收到 SIGPIPE 死掉）；成功才把 `<stdout_bytes>` 個 `b'a'` 寫進 stdout 並以 0 結束。
//!
//! 三個模式都只用 std 的同步 I/O：不啟動 tokio、不呼叫任何 shell（`cmd /c`／`powershell`／
//! `sh -c`），符合 brief「假程式...不依賴 shell」的要求。

use std::io::Write;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("sleep") => run_sleep(&args[1..]),
        Some("sleep-record-pid") => run_sleep_record_pid(&args[1..]),
        Some("stdout-bytes") => run_stdout_bytes(&args[1..]),
        Some("exit-with") => run_exit_with(&args[1..]),
        Some("stderr-then-stdout") => run_stderr_then_stdout(&args[1..]),
        other => {
            eprintln!("git_fake: unknown mode {other:?}, args={args:?}");
            std::process::exit(2);
        }
    }
}

fn run_sleep(rest: &[String]) {
    let ms: u64 = rest.first().and_then(|s| s.parse().ok()).unwrap_or(0);
    std::thread::sleep(std::time::Duration::from_millis(ms));
}

fn run_sleep_record_pid(rest: &[String]) {
    let Some(pid_file) = rest.first() else {
        eprintln!("git_fake sleep-record-pid: 缺少 <pid_file> 參數");
        std::process::exit(2);
    };
    let ms: u64 = rest.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);

    let pid = std::process::id();
    match std::fs::File::create(pid_file) {
        Ok(mut file) => {
            if write!(file, "{pid}").is_err() || file.flush().is_err() {
                eprintln!("git_fake sleep-record-pid: 寫入 {pid_file} 失敗");
                std::process::exit(2);
            }
        }
        Err(e) => {
            eprintln!("git_fake sleep-record-pid: 開啟 {pid_file} 失敗：{e}");
            std::process::exit(2);
        }
    }

    std::thread::sleep(std::time::Duration::from_millis(ms));
}

fn run_stdout_bytes(rest: &[String]) {
    let n: usize = rest.first().and_then(|s| s.parse().ok()).unwrap_or(0);
    let chunk = vec![b'a'; n];
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    let _ = handle.write_all(&chunk);
    let _ = handle.flush();
}

fn run_exit_with(rest: &[String]) {
    let code: i32 = rest.first().and_then(|s| s.parse().ok()).unwrap_or(1);
    let text = rest.get(1).map(String::as_str).unwrap_or("");
    eprintln!("{text}");
    std::process::exit(code);
}

fn run_stderr_then_stdout(rest: &[String]) {
    let stderr_n: usize = rest.first().and_then(|s| s.parse().ok()).unwrap_or(0);
    let stdout_n: usize = rest.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);

    let stderr = std::io::stderr();
    let mut handle = stderr.lock();
    let chunk = [b'e'; 4096];
    let mut left = stderr_n;
    while left > 0 {
        let n = left.min(chunk.len());
        if handle.write_all(&chunk[..n]).is_err() {
            // 讀端已關閉：模擬 git 收到 SIGPIPE 死掉，不再輸出 stdout。
            std::process::exit(13);
        }
        left -= n;
    }
    let _ = handle.flush();

    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    let _ = out.write_all(&vec![b'a'; stdout_n]);
    let _ = out.flush();
}
