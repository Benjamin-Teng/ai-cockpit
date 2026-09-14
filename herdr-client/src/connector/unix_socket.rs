//! unix domain socket 連線（spec「unix domain socket 連線」）。

use std::path::PathBuf;

use async_trait::async_trait;
use tokio::net::UnixStream;

use super::stream::SplitLineStream;
use super::{ConnectError, Connector, NdjsonStream};

/// 以 AF_UNIX socket 連上指定路徑的 HERDR（WSL 端預設
/// `/home/<user>/.config/herdr/herdr.sock`，設計文件 §2.1）。
pub struct UnixSocketConnector {
    path: PathBuf,
}

impl UnixSocketConnector {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }
}

#[async_trait]
impl Connector for UnixSocketConnector {
    async fn connect(&self) -> Result<Box<dyn NdjsonStream>, ConnectError> {
        let stream = UnixStream::connect(&self.path)
            .await
            .map_err(ConnectError::from_io)?;
        Ok(Box::new(SplitLineStream::new(stream)))
    }

    fn describe(&self) -> String {
        format!("unix-socket {}", self.path.display())
    }
}
