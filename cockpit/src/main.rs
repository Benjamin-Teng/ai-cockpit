//! Cockpit 服務進入點（design D16）：只負責初始化日誌、注入真值（命令列、工作目錄、
//! 環境變數），其餘全部交給 [`cockpit::app::run`]。
//!
//! 日誌用 `tracing-subscriber` 的 env filter（`RUST_LOG`，預設 `info`），只寫 stderr——
//! stdout 留給未來可能的管線輸出，也讓 `cargo run` 的日誌不會混進 HTTP 內容。stderr 是終端機
//! 時才輸出 ANSI 色碼；導向檔案（啟動器背景啟動時寫進 `cockpit.log`）時不帶色碼
//! （desktop-launch-notify task 2.1；design D3）。
//!
//! 回傳 `anyhow::Result<()>`：任何啟動失敗（設定檔不存在、驗證不過、port 被占用）都會
//! 由 Rust runtime 以 `Error: <訊息>` 印到 stderr 並讓行程非零結束。
//!
//! 不用 `#[tokio::main]`：它在 `main` 返回時 drop runtime，會無限期等卡住的 `spawn_blocking`（例如讀
//! `\\wsl.localhost` 卡在 9P）。改由 [`cockpit::app::block_on_bounded`] 建 runtime，跑完後最多再等
//! [`cockpit::app::RUNTIME_SHUTDOWN_TIMEOUT`]（openspec-stage-sync task 4.7）。

use std::io::IsTerminal;

use anyhow::Context;
use cockpit::app::{RUNTIME_SHUTDOWN_TIMEOUT, block_on_bounded};
use tracing_subscriber::EnvFilter;

fn main() -> anyhow::Result<()> {
    block_on_bounded(serve(), RUNTIME_SHUTDOWN_TIMEOUT).context("建立 tokio runtime 失敗")?
}

async fn serve() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_writer(std::io::stderr)
        .with_ansi(std::io::stderr().is_terminal())
        .init();

    let args = cockpit::config::parse_args(&std::env::args().skip(1).collect::<Vec<_>>())?;
    let cwd = std::env::current_dir().context("取得工作目錄失敗")?;

    cockpit::app::run(&args, &cwd, &|key| std::env::var(key).ok()).await
}
