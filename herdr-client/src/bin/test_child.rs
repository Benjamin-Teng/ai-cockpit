//! `ChildStdioConnector` 整合測試用的假對端子程序（task 2.2、design Open Questions）。
//! 只給 `herdr-client/tests/transport.rs` 用，不是給使用者跑的工具。
//!
//! 用法（argv\[1\] 選模式）：
//! - `test_child echo`：先在 stderr 印一行 `noise`（驗證 stderr 不混入 stdout），之後每讀一行
//!   stdin 就在 stdout 回一行 `{"echo":"<原行>"}`，直到 stdin EOF。
//! - `test_child exit-immediately <code> <stderr文字>`：把 `<stderr文字>` 印到 stderr 後以
//!   `<code>` 結束，不讀 stdin（模擬橋接目標不存在、子程序立即結束的情境）。
//! - `test_child exit-after-sleep <code> <stderr文字> <sleep_ms>`：把 `<stderr文字>` 印到
//!   stderr 後睡 `<sleep_ms>` 毫秒才以 `<code>` 結束，不讀 stdin（Codex fix round 1 finding 2：
//!   驗證 stderr 內容在「寫入與行程結束之間有明顯間隔」的排程情境下依然不會遺失）。
//! - `test_child sleep <secs>`：睡滿 `<secs>` 秒才結束，用來測 `kill_on_drop`。
//! - `test_child relay <endpoint_path>`：task 4.4 的假對端（design Open Questions 裁決，見
//!   `.superpowers/sdd/tasks/progress.md` 的 ruling）。行為像 `nc -U`：連到假 HERDR
//!   （`herdr_client::testing::FakeHerdr`）的原生端點——`endpoint_path` 即
//!   `FakeHerdr::endpoint_path()` 回傳的原始路徑（Windows 不含 `\\.\pipe\` 前綴，由
//!   `NamedPipeConnector` 自己補上；unix 直接是 socket 檔案路徑，交給 `UnixSocketConnector`）
//!   ——之後 stdin 每一行轉送到 socket、socket 每一行寫到 stdout 並 flush。連線失敗時把原因印到
//!   stderr 後以非 0 結束；連線成功後，socket EOF 或 stdin EOF 任一者發生就以 0 結束程序。
//!   只給 `herdr-client/tests/child_bridge.rs` 用。
//! - `test_child relay-then-fail <endpoint_path> <n> [close_then_sleep_ms]`：全分支最終
//!   review finding 3 加的模式，模擬橋接目標（`wsl.exe`／`nc` 這類）在握手後才因故障非零
//!   退出——行為跟 `relay` 完全相同，但只轉送（socket → stdout，即假 HERDR 送給 client 那個
//!   方向）滿 `<n>` 行之後，就印一行 stderr 訊息、以非 0（`3`）結束程序，不繼續轉送（也不理會
//!   stdin）。
//!   給第 4 個參數 `close_then_sleep_ms`（Codex 最終 review 二次確認 finding 加）時，改成：
//!   印完 stderr 訊息後先明確關閉自己的 stdout（讓連線對端立刻看到 stdout EOF），再睡滿
//!   `close_then_sleep_ms` 毫秒才以 `3` 結束——模擬「橋接程序先關 stdout、經清理或排程延遲
//!   後才真正退出」這個會被誤判成正常 EOF 的競態窗口。省略這個參數維持舊行為（印完 stderr
//!   立刻結束，stdout 關閉與程序結束幾乎同時發生）。只給
//!   `herdr-client/tests/child_bridge.rs` 用。

use std::io::{self, BufRead, Write};

use herdr_client::connector::Connector;
#[cfg(windows)]
use herdr_client::connector::NamedPipeConnector;
#[cfg(unix)]
use herdr_client::connector::UnixSocketConnector;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("echo") => run_echo(),
        Some("exit-immediately") => run_exit_immediately(&args[1..]),
        Some("exit-after-sleep") => run_exit_after_sleep(&args[1..]),
        Some("sleep") => run_sleep(&args[1..]),
        Some("relay") => run_relay(&args[1..]),
        Some("relay-then-fail") => run_relay_then_fail(&args[1..]),
        other => {
            eprintln!("test_child: unknown mode {other:?}, args={args:?}");
            std::process::exit(2);
        }
    }
}

fn run_echo() {
    // 驗證 stderr 內容不會混入 ChildStdioConnector 讀到的 stdout 資料流。
    eprintln!("noise");

    let stdin = io::stdin();
    let mut stdout = io::stdout();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        let response = serde_json::json!({ "echo": line });
        if writeln!(stdout, "{response}").is_err() {
            break;
        }
        let _ = stdout.flush();
    }
}

fn run_exit_immediately(rest: &[String]) {
    let code: i32 = rest.first().and_then(|s| s.parse().ok()).unwrap_or(1);
    let message = rest.get(1).map(String::as_str).unwrap_or("");
    eprintln!("{message}");
    std::process::exit(code);
}

fn run_exit_after_sleep(rest: &[String]) {
    let code: i32 = rest.first().and_then(|s| s.parse().ok()).unwrap_or(1);
    let message = rest.get(1).map(String::as_str).unwrap_or("");
    let sleep_ms: u64 = rest.get(2).and_then(|s| s.parse().ok()).unwrap_or(50);
    eprintln!("{message}");
    // `eprintln!` 走 `Stderr`，std 不緩衝，寫入時就已經送進管線，不需要額外 flush。
    std::thread::sleep(std::time::Duration::from_millis(sleep_ms));
    std::process::exit(code);
}

fn run_sleep(rest: &[String]) {
    let secs: u64 = rest.first().and_then(|s| s.parse().ok()).unwrap_or(1);
    std::thread::sleep(std::time::Duration::from_secs(secs));
}

/// `relay <endpoint_path>`：同步入口，包一個 `current_thread` tokio runtime——`main()`
/// 其餘模式都是同步 blocking I/O，不用為了這一個模式把整個 bin 改成 `#[tokio::main]`。
fn run_relay(rest: &[String]) {
    let Some(endpoint) = rest.first() else {
        eprintln!("test_child relay: 缺少 <endpoint_path> 參數");
        std::process::exit(2);
    };

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap_or_else(|e| {
            eprintln!("test_child relay: 建立 tokio runtime 失敗: {e}");
            std::process::exit(1);
        });

    let exit_code = runtime.block_on(relay_main(endpoint));
    std::process::exit(exit_code);
}

#[cfg(windows)]
fn relay_connector(endpoint: &str) -> Box<dyn Connector> {
    Box::new(NamedPipeConnector::new(endpoint))
}

#[cfg(unix)]
fn relay_connector(endpoint: &str) -> Box<dyn Connector> {
    Box::new(UnixSocketConnector::new(endpoint))
}

/// 連線 → 雙向轉送 → 任一端 EOF／錯誤就結束（行為像 `nc -U`）。
///
/// 用一個 `loop { tokio::select! { ... } }` 交替讀 stdin 行與 socket 行：`stream.recv_line()`
/// 是這個迴圈裡唯一會借用 `&mut stream` 的分支——`tokio::select!` 在每次迭代開頭建立所有分支
/// 的 future 後才開始輪詢，贏的分支執行完（丟棄其餘 future、包括那個借用）之後才輪到下一次
/// 迭代；「stdin 有一行」那個分支的 arm body 裡呼叫 `stream.send_line()` 時，上一輪
/// `recv_line()` 的借用早已結束，不會有兩個同時存在的 `&mut stream` 借用，不需要把
/// `NdjsonStream` 拆成讀寫兩半。
async fn relay_main(endpoint: &str) -> i32 {
    let connector = relay_connector(endpoint);
    let mut stream = match connector.connect().await {
        Ok(stream) => stream,
        Err(e) => {
            eprintln!("test_child relay: 連線到 {endpoint} 失敗: {e}");
            return 1;
        }
    };

    let stdin = tokio::io::BufReader::new(tokio::io::stdin());
    let mut stdin_lines = stdin.lines();
    let mut stdout = tokio::io::stdout();

    loop {
        tokio::select! {
            line = stdin_lines.next_line() => {
                match line {
                    Ok(Some(l)) => {
                        if stream.send_line(&l).await.is_err() {
                            break;
                        }
                    }
                    // stdin EOF 或讀取錯誤：結束程序（design：兩端任一 EOF 都結束）。
                    Ok(None) | Err(_) => break,
                }
            }
            recv = stream.recv_line() => {
                match recv {
                    Ok(Some(l)) => {
                        if stdout.write_all(l.as_bytes()).await.is_err()
                            || stdout.write_all(b"\n").await.is_err()
                            || stdout.flush().await.is_err()
                        {
                            break;
                        }
                    }
                    // socket EOF 或讀取錯誤：結束程序。
                    Ok(None) | Err(_) => break,
                }
            }
        }
    }

    0
}

/// `relay-then-fail <endpoint_path> <n> [close_then_sleep_ms]`：同步入口，跟 `run_relay`
/// 一樣包一個 `current_thread` tokio runtime。
fn run_relay_then_fail(rest: &[String]) {
    let Some(endpoint) = rest.first() else {
        eprintln!("test_child relay-then-fail: 缺少 <endpoint_path> 參數");
        std::process::exit(2);
    };
    let Some(forward_limit) = rest.get(1).and_then(|s| s.parse::<usize>().ok()) else {
        eprintln!("test_child relay-then-fail: 缺少或不合法的 <n> 參數");
        std::process::exit(2);
    };
    // 第 4 個參數選填：Codex 最終 review 二次確認 finding 加，見上方模組文件註解。
    let close_then_sleep_ms = rest.get(2).and_then(|s| s.parse::<u64>().ok());

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap_or_else(|e| {
            eprintln!("test_child relay-then-fail: 建立 tokio runtime 失敗: {e}");
            std::process::exit(1);
        });

    let exit_code = runtime.block_on(relay_then_fail_main(
        endpoint,
        forward_limit,
        close_then_sleep_ms,
    ));
    std::process::exit(exit_code);
}

/// 明確關閉目前程序的 stdout（管線寫入端），讓 `ChildStdioStream` 那端的讀取立刻看到 EOF，
/// 而這個程序本身繼續跑（Codex 最終 review 二次確認 finding：模擬「橋接程序先關 stdout、
/// 經清理或排程延遲後才真正退出」這個競態窗口）。
///
/// std 的 `Stdout` 型別故意讓 drop 不去關底層 fd／handle（避免一般程式意外把行程共用的
/// stdout 關掉），所以要真的關閉，得自己用 `FromRawFd`／`FromRawHandle` 接手底層 fd／
/// handle 再馬上 drop——只用 std 本身的介面，不需要 `libc` 這類額外依賴（KISS）。呼叫這個
/// 函式之後不能再嘗試寫 stdout（會失敗），呼叫端需自行保證。
fn close_stdout() {
    #[cfg(unix)]
    {
        use std::os::unix::io::{AsRawFd, FromRawFd};
        let fd = io::stdout().as_raw_fd();
        // SAFETY: `fd` 是目前行程 stdout 的合法、開啟中的檔案描述符（來自
        // `io::stdout().as_raw_fd()`）；接手成 `File` 後立刻 drop，讓它的解構子關閉這個 fd，
        // 之後不再使用這個 fd 或原本的 `io::stdout()` 寫入。
        drop(unsafe { std::fs::File::from_raw_fd(fd) });
    }
    #[cfg(windows)]
    {
        use std::os::windows::io::{AsRawHandle, FromRawHandle};
        let handle = io::stdout().as_raw_handle();
        // SAFETY: `handle` 是目前行程 stdout 的合法、開啟中的 handle（來自
        // `io::stdout().as_raw_handle()`）；接手成 `File` 後立刻 drop，讓它的解構子關閉這個
        // handle，之後不再使用這個 handle 或原本的 `io::stdout()` 寫入。
        drop(unsafe { std::fs::File::from_raw_handle(handle) });
    }
}

/// 全分支最終 review finding 3：跟 `relay_main` 幾乎一樣，差別只在轉送方向
/// socket → stdout（假 HERDR 送給 client 的那個方向，`subscription_started` 與後續事件都走
/// 這裡）滿 `forward_limit` 行之後，不繼續迴圈——印一行 stderr 訊息、回傳非 0 結束碼，模擬
/// 橋接目標在握手後才因故障退出（`ChildStdioStream::recv_line` 這時應該把這個非零退出＋
/// stderr 內容包成一個 I/O 錯誤，而不是靜默地當成對面正常關閉連線）。
///
/// `close_then_sleep_ms` 為 `Some` 時（Codex 最終 review 二次確認 finding），印完 stderr
/// 訊息後先呼叫 `close_stdout()` 明確關閉 stdout，再睡滿這麼多毫秒才回傳 3——模擬「stdout
/// 先關、程序才因清理或排程延遲晚一點才真正退出」；`None` 維持舊行為（印完立刻回傳，stdout
/// 關閉與程序結束幾乎同時發生）。
async fn relay_then_fail_main(
    endpoint: &str,
    forward_limit: usize,
    close_then_sleep_ms: Option<u64>,
) -> i32 {
    let connector = relay_connector(endpoint);
    let mut stream = match connector.connect().await {
        Ok(stream) => stream,
        Err(e) => {
            eprintln!("test_child relay-then-fail: 連線到 {endpoint} 失敗: {e}");
            return 1;
        }
    };

    let stdin = tokio::io::BufReader::new(tokio::io::stdin());
    let mut stdin_lines = stdin.lines();
    let mut stdout = tokio::io::stdout();
    let mut forwarded = 0usize;

    loop {
        tokio::select! {
            line = stdin_lines.next_line() => {
                match line {
                    Ok(Some(l)) => {
                        if stream.send_line(&l).await.is_err() {
                            break;
                        }
                    }
                    Ok(None) | Err(_) => break,
                }
            }
            recv = stream.recv_line() => {
                match recv {
                    Ok(Some(l)) => {
                        if stdout.write_all(l.as_bytes()).await.is_err()
                            || stdout.write_all(b"\n").await.is_err()
                            || stdout.flush().await.is_err()
                        {
                            break;
                        }
                        forwarded += 1;
                        if forwarded >= forward_limit {
                            eprintln!(
                                "test_child relay-then-fail: forwarded {forwarded} line(s), simulating bridge crash"
                            );
                            if let Some(sleep_ms) = close_then_sleep_ms {
                                close_stdout();
                                tokio::time::sleep(std::time::Duration::from_millis(sleep_ms))
                                    .await;
                            }
                            return 3;
                        }
                    }
                    Ok(None) | Err(_) => break,
                }
            }
        }
    }

    0
}
