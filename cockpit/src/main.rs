//! Cockpit 服務進入點（design D16）：只負責初始化日誌、注入真值（命令列、工作目錄、
//! 環境變數），其餘全部交給 [`cockpit::app::run`]。
//!
//! 日誌用 `tracing-subscriber` 的 env filter（`RUST_LOG`，預設 `info`），只寫 stderr——
//! stdout 留給未來可能的管線輸出，也讓 `cargo run` 的日誌不會混進 HTTP 內容。
//!
//! 回傳 `anyhow::Result<()>`：任何啟動失敗（設定檔不存在、驗證不過、port 被占用）都會
//! 由 Rust runtime 以 `Error: <訊息>` 印到 stderr 並讓行程非零結束。

use anyhow::Context;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_writer(std::io::stderr)
        .init();

    let args = cockpit::config::parse_args(&std::env::args().skip(1).collect::<Vec<_>>())?;
    let cwd = std::env::current_dir().context("取得工作目錄失敗")?;

    cockpit::app::run(&args, &cwd, &|key| std::env::var(key).ok()).await
}
