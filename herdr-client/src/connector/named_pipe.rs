//! Windows named pipe 連線（spec「Windows named pipe 連線」；design D9）。

use std::path::PathBuf;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use tokio::net::windows::named_pipe::ClientOptions;
use tokio::time::sleep;

use super::stream::SplitLineStream;
use super::{ConnectError, Connector, NdjsonStream};

/// `ERROR_PIPE_BUSY`：Windows named pipe 全部 instance 忙碌時 `CreateFile` 回的錯誤碼
/// （design D9；不為了這一個常數引入 `windows-sys`）。
const ERROR_PIPE_BUSY: i32 = 231;

/// pipe instance 忙碌時的重試上限與間隔（design D9：依 tokio 文件建議每 50ms 重試一次，
/// 上限 1 秒）。
const RETRY_BUDGET: Duration = Duration::from_secs(1);
const RETRY_INTERVAL: Duration = Duration::from_millis(50);

/// 以 Windows named pipe 連上 HERDR。pipe 名稱為 `\\.\pipe\` 接上 `path` 全文
/// （含磁碟機冒號與反斜線，例如 `\\.\pipe\C:\Users\<user>\AppData\Roaming\herdr\herdr.sock`），
/// 不解析 `herdr.sock` 檔案內容（其內容是 server 自用標記，非 socket 路徑本身）。
pub struct NamedPipeConnector {
    path: PathBuf,
}

impl NamedPipeConnector {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    fn pipe_name(&self) -> String {
        format!(r"\\.\pipe\{}", self.path.display())
    }
}

#[async_trait]
impl Connector for NamedPipeConnector {
    async fn connect(&self) -> Result<Box<dyn NdjsonStream>, ConnectError> {
        let name = self.pipe_name();
        let deadline = Instant::now() + RETRY_BUDGET;
        loop {
            match ClientOptions::new().open(&name) {
                Ok(client) => return Ok(Box::new(SplitLineStream::new(client))),
                Err(e) if e.raw_os_error() == Some(ERROR_PIPE_BUSY) => {
                    if Instant::now() >= deadline {
                        return Err(ConnectError::from_io(e));
                    }
                    sleep(RETRY_INTERVAL).await;
                }
                Err(e) => return Err(ConnectError::from_io(e)),
            }
        }
    }

    fn describe(&self) -> String {
        format!("named-pipe {}", self.path.display())
    }
}
