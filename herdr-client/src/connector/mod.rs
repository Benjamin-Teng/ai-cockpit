//! Transport 層：建立一條 NDJSON 連線的三種方式與連線失敗分類（spec `herdr-transport`）。
//! 內容由 task 2.1–2.3 填入。

mod child_stdio;
mod default_path;
#[cfg(windows)]
mod named_pipe;
// task 4.1：放寬成 `pub(crate)`，讓 `testing` 模組重用 `read_one_line`／`write_line`
// （兩者本來就是 `pub(crate)`，這裡只是讓模組本身在 crate 內可見），不重寫第二份逐行
// 讀寫邏輯。`SplitLineStream` 本身沒被 `testing` 用到（因為 server 端保留單一 `T` 值、
// 靠 `BufReader<T>` 對已實作 `AsyncWrite` 的 `T` 轉發寫入，見 `testing::connection`）。
pub(crate) mod stream;
#[cfg(unix)]
mod unix_socket;

pub use child_stdio::{ChildStdioConnector, ChildStdioStream};
pub use default_path::{default_socket_path, default_socket_path_from_env};
#[cfg(windows)]
pub use named_pipe::NamedPipeConnector;
#[cfg(unix)]
pub use unix_socket::UnixSocketConnector;

use async_trait::async_trait;

/// 一條已建立的 NDJSON 連線：送一行、讀一行（spec「每次呼叫建立一條全新的 NDJSON 連線」）。
#[async_trait]
pub trait NdjsonStream: Send {
    /// 送一行（不含換行，實作補 `\n` 並 flush）。
    async fn send_line(&mut self, line: &str) -> std::io::Result<()>;
    /// 讀一行（去掉尾端換行）；乾淨 EOF 回 `Ok(None)`。
    async fn recv_line(&mut self) -> std::io::Result<Option<String>>;
}

/// 建立一條新 NDJSON 連線的方式；每呼叫一次 `connect` 開一條全新連線，不在同一條連線上
/// 多工多個 method（設計文件 §5.1）。
#[async_trait]
pub trait Connector: Send + Sync {
    async fn connect(&self) -> Result<Box<dyn NdjsonStream>, ConnectError>;
    /// 給人看的描述字串，指出 transport 種類與目標（路徑或指令列），供畫面與日誌顯示。
    fn describe(&self) -> String;
}

/// 連線失敗的三種分類，附可讀的原因文字（spec「連線失敗分類」需求）。
#[derive(Debug, thiserror::Error)]
pub enum ConnectError {
    #[error("HERDR server not running: {detail}")]
    ServerNotRunning { detail: String },
    #[error("failed to spawn bridge process: {detail}")]
    Spawn { detail: String },
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

impl ConnectError {
    /// 依 `io::ErrorKind` 分類：`NotFound`／`ConnectionRefused` 代表目標不存在或拒絕連線
    /// （HERDR 自己也是用這兩種 kind 判定 server 未啟動，見設計文件 §2.2），其餘歸 `Io`。
    pub fn from_io(e: std::io::Error) -> Self {
        match e.kind() {
            std::io::ErrorKind::NotFound | std::io::ErrorKind::ConnectionRefused => {
                ConnectError::ServerNotRunning {
                    detail: e.to_string(),
                }
            }
            _ => ConnectError::Io(e),
        }
    }
}
