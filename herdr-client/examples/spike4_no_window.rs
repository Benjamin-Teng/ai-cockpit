//! Spike 4（design D2、D9；ADR-0002）：驗證從 HERDR pane 內啟動子程序時，加
//! `CREATE_NO_WINDOW`（`0x0800_0000`）是否仍會彈出額外視窗。橋接指令沿用 spike 2 查證過的
//! `wsl.exe -d <distro> -e nc -U <socket>`，完成一次 `session.snapshot`。
//!
//! 用法：
//!   cargo run -p herdr-client --example spike4_no_window                 # 加 CREATE_NO_WINDOW
//!   cargo run -p herdr-client --example spike4_no_window -- --with-window  # 對照組，不加旗標
//!
//! 環境變數：
//!   HERDR_CLIENT_TEST_WSL_DISTRO（預設 `Ubuntu-24.04`）
//!   HERDR_CLIENT_TEST_WSL_SOCKET（預設 `/home/<user>/.config/herdr/herdr.sock`）
//!
//! 硬性限制：全程只送 `session.snapshot`；不得執行 `herdr server stop`；不得動 Windows 端 HERDR。

#[cfg(not(windows))]
fn main() {
    eprintln!(
        "spike4_no_window 驗證的是 Windows 專屬的 CREATE_NO_WINDOW 旗標（design D9），\
         本平台無此概念，略過。"
    );
}

#[cfg(windows)]
#[tokio::main]
async fn main() -> std::io::Result<()> {
    use std::process::Stdio;

    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    use tokio::process::Command;

    /// `CREATE_NO_WINDOW`（Windows process creation flag）。不與 `DETACHED_PROCESS` 併用
    /// （ADR-0002 決定）。
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    fn print_usage() {
        eprintln!("用法: spike4_no_window [--with-window]");
        eprintln!();
        eprintln!("  （無參數）     加 CREATE_NO_WINDOW 旗標啟動 wsl.exe 子程序（預設模式）");
        eprintln!("  --with-window  對照組，不加 CREATE_NO_WINDOW");
        eprintln!("  -h, --help     印出本說明並離開，不連線");
        eprintln!();
        eprintln!("環境變數:");
        eprintln!("  HERDR_CLIENT_TEST_WSL_DISTRO（預設 Ubuntu-24.04）");
        eprintln!("  HERDR_CLIENT_TEST_WSL_SOCKET（預設 /home/<user>/.config/herdr/herdr.sock）");
    }

    /// 在讀任何環境變數或啟動子程序之前完整解析 argv：只接受 `--with-window`；
    /// `-h`／`--help` 印用法後直接離開（exit 0），不連線；其他任何參數視為錯誤
    /// （exit 2），避免像 Codex review 指出的那樣，`--help` 之類的未知參數被
    /// `.any(|arg| arg == "--with-window")` 忽略、落入預設模式而直接連線。
    fn parse_with_window_flag() -> bool {
        let mut with_window = false;
        for arg in std::env::args().skip(1) {
            match arg.as_str() {
                "-h" | "--help" => {
                    print_usage();
                    std::process::exit(0);
                }
                "--with-window" => with_window = true,
                other => {
                    eprintln!("unknown argument: {other}");
                    print_usage();
                    std::process::exit(2);
                }
            }
        }
        with_window
    }

    let with_window = parse_with_window_flag();
    let distro = std::env::var("HERDR_CLIENT_TEST_WSL_DISTRO")
        .unwrap_or_else(|_| "Ubuntu-24.04".to_string());
    let socket = std::env::var("HERDR_CLIENT_TEST_WSL_SOCKET")
        .unwrap_or_else(|_| "/home/<user>/.config/herdr/herdr.sock".to_string());

    eprintln!("本程式 pid: {}", std::process::id());
    eprintln!(
        "模式: {}",
        if with_window {
            "對照組（不加 CREATE_NO_WINDOW）"
        } else {
            "CREATE_NO_WINDOW"
        }
    );
    eprintln!("distro: {distro}");
    eprintln!("socket: {socket}");

    let mut command = Command::new("wsl.exe");
    command
        .args(["-d", &distro, "-e", "nc", "-U", &socket])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    if !with_window {
        command.creation_flags(CREATE_NO_WINDOW);
    }

    let mut child = command.spawn().expect("啟動 wsl.exe 子程序失敗");
    eprintln!("子程序 pid: {:?}", child.id());

    let mut stdin = child.stdin.take().expect("子程序缺少 stdin handle");
    let stdout = child.stdout.take().expect("子程序缺少 stdout handle");

    let request = "{\"id\":\"1\",\"method\":\"session.snapshot\",\"params\":{}}\n";
    stdin.write_all(request.as_bytes()).await?;
    stdin.flush().await?;

    let mut reader = BufReader::new(stdout);
    let mut line = String::new();
    reader.read_line(&mut line).await?;
    eprintln!("回應 bytes: {}", line.len());

    let parsed: serde_json::Value =
        serde_json::from_str(line.trim_end()).expect("回應不是合法 JSON");
    let snapshot = parsed
        .get("result")
        .and_then(|result| result.get("snapshot"));
    eprintln!(
        "protocol: {:?}",
        snapshot.and_then(|snapshot| snapshot.get("protocol"))
    );
    eprintln!(
        "version: {:?}",
        snapshot.and_then(|snapshot| snapshot.get("version"))
    );

    drop(stdin);
    let status = child.wait().await?;
    eprintln!("子程序 exit status: {status}");

    Ok(())
}
