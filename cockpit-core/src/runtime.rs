//! `AgentRuntime` 抽象與圍繞它的錯誤、事件流型別：驅動器與狀態庫只透過這個抽象與
//! runtime 互動，看不到 runtime 內部有幾條連線（例如 HERDR 的 gRPC 連線與子程序）。

use std::time::Duration;

use thiserror::Error;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::types::{RuntimeEvent, RuntimeId, RuntimeSnapshot};

/// 呼叫 `AgentRuntime` 時可能發生的錯誤。
///
/// `Display` 的輸出直接就是可以拿來當 `ConnectionState::Disconnected.reason` 用的字串。
#[derive(Debug, Error)]
pub enum RuntimeError {
    /// runtime 目前無法使用，但附上建議的重試間隔（例如 HERDR 還沒啟動、正在重連）。
    #[error("{reason}")]
    Unavailable {
        /// 無法使用的原因，供顯示與記錄。
        reason: String,
        /// 建議在這麼久之後重試。
        retry_after: Duration,
    },
    /// 呼叫本身失敗，沒有固定的重試間隔建議。
    #[error("{0}")]
    Failed(String),
}

impl RuntimeError {
    /// 取得建議的重試間隔；只有 `Unavailable` 會回傳 `Some`，`Failed` 一律 `None`。
    pub fn retry_after(&self) -> Option<Duration> {
        match self {
            RuntimeError::Unavailable { retry_after, .. } => Some(*retry_after),
            RuntimeError::Failed(_) => None,
        }
    }
}

/// 持有一組背景 task 的 `JoinHandle`，drop 時把它們全部 `abort()`。
///
/// 用來保證「釋放事件流即關閉連線與子程序」：使用者只要 drop `RuntimeEvents`，
/// 不需要自己記得收拾背景任務。
struct AbortOnDrop(Vec<JoinHandle<()>>);

impl Drop for AbortOnDrop {
    fn drop(&mut self) {
        for handle in &self.0 {
            handle.abort();
        }
    }
}

/// 一條已合併的 runtime 事件流；每個項目是一個 `RuntimeEvent` 或是帶原因的 `RuntimeError`。
///
/// 只提供 `next()`，不實作 `futures::Stream`（`cockpit-core` 不引入 `futures`）。
///
/// 唯一的建構方式是 [`RuntimeEvents::channel`]：容量固定為 [`RuntimeEvents::CAPACITY`]，
/// 呼叫端無法用任意容量的 receiver 拼出一個 `RuntimeEvents`。用法：
///
/// ```ignore
/// let (tx, unstarted) = RuntimeEvents::channel();
/// let task = tokio::spawn(async move { /* 用 tx 送事件 */ });
/// let events = unstarted.start(vec![task]);
/// ```
pub struct RuntimeEvents {
    rx: mpsc::Receiver<Result<RuntimeEvent, RuntimeError>>,
    _guard: AbortOnDrop,
}

impl RuntimeEvents {
    /// 事件通道的容量；`channel()` 建出來的通道一律是這個容量。
    pub const CAPACITY: usize = 1024;

    /// 建立一組容量固定為 `CAPACITY` 的事件通道。呼叫端先用回傳的 `Sender`
    /// spawn 產生事件的背景 task，等拿到這些 task 的 `JoinHandle` 之後，
    /// 再呼叫 [`UnstartedEvents::start`] 組出真正的 `RuntimeEvents`。
    pub fn channel() -> (
        mpsc::Sender<Result<RuntimeEvent, RuntimeError>>,
        UnstartedEvents,
    ) {
        let (tx, rx) = mpsc::channel(Self::CAPACITY);
        (tx, UnstartedEvents { rx })
    }

    /// 取得下一筆事件；事件流關閉時回傳 `None`。
    pub async fn next(&mut self) -> Option<Result<RuntimeEvent, RuntimeError>> {
        self.rx.recv().await
    }
}

/// [`RuntimeEvents::channel`] 回傳的另一半：已經拿到固定容量的 receiver，但還沒有
/// 綁定產生事件的背景 task。呼叫 [`UnstartedEvents::start`] 把這些 task 的
/// `JoinHandle` 交給它，換回可以使用的 `RuntimeEvents`。
pub struct UnstartedEvents {
    rx: mpsc::Receiver<Result<RuntimeEvent, RuntimeError>>,
}

impl UnstartedEvents {
    /// 綁定產生事件的背景 task，組出完整的 `RuntimeEvents`；這些 task 會在
    /// `RuntimeEvents` 被 drop 時一併 `abort()`。
    pub fn start(self, tasks: Vec<JoinHandle<()>>) -> RuntimeEvents {
        RuntimeEvents {
            rx: self.rx,
            _guard: AbortOnDrop(tasks),
        }
    }
}

/// 一個可以被驅動器與狀態庫消費的 agent runtime 抽象（例如 HERDR）。
///
/// 實作者要能回報自己的 `RuntimeId`、提供一份 `RuntimeSnapshot`，以及建立一條
/// 已合併的事件流。
#[async_trait::async_trait]
pub trait AgentRuntime: Send + Sync {
    /// 這個 runtime 的識別碼。
    fn id(&self) -> &RuntimeId;

    /// 取得目前的完整狀態快照。
    async fn snapshot(&self) -> Result<RuntimeSnapshot, RuntimeError>;

    /// 建立一條已合併的事件流，供狀態庫持續套用增量更新。
    async fn subscribe(&self) -> Result<RuntimeEvents, RuntimeError>;
}
