//! 測試資料 builder 與 `FakeRuntime`，Task 1.3–1.9（`cockpit-core/tests/*.rs`）共用。
//!
//! `#[allow(dead_code)]`：不同 task 只用得到其中一部分 builder，其餘留給之後的 task 用。

#![allow(dead_code)]

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use cockpit_core::{
    Agent, AgentRuntime, AgentStatus, Focused, Pane, PaneId, PaneOutput, RuntimeError,
    RuntimeEvent, RuntimeEvents, RuntimeId, RuntimeSnapshot, Tab, TabId, Workspace, WorkspaceId,
};
use tokio::sync::mpsc;
use tokio::time::Instant;

pub fn runtime_id(id: &str) -> RuntimeId {
    RuntimeId::new(id)
}

pub fn workspace_id(id: &str) -> WorkspaceId {
    WorkspaceId::new(id)
}

pub fn tab_id(id: &str) -> TabId {
    TabId::new(id)
}

pub fn pane_id(id: &str) -> PaneId {
    PaneId::new(id)
}

/// 建一個 workspace；`label`、`focused` 一律用預設值，需要不同值時直接改回傳值的欄位。
pub fn workspace(id: &str, number: u32) -> Workspace {
    Workspace {
        id: workspace_id(id),
        label: None,
        number,
        agent_status: AgentStatus::Idle,
        focused: false,
    }
}

/// 建一個 tab，隸屬於 `workspace`。
pub fn tab(id: &str, workspace: &str, number: u32) -> Tab {
    Tab {
        id: tab_id(id),
        workspace_id: workspace_id(workspace),
        number,
        agent_status: AgentStatus::Idle,
        focused: false,
    }
}

/// 建一個 pane，隸屬於 `workspace` 底下的 `tab`。
pub fn pane(id: &str, workspace: &str, tab: &str) -> Pane {
    Pane {
        id: pane_id(id),
        workspace_id: workspace_id(workspace),
        tab_id: tab_id(tab),
        agent: None,
        agent_status: AgentStatus::Idle,
        title: None,
        cwd: None,
        label: None,
        focused: false,
        exited: false,
        updated_at: SystemTime::UNIX_EPOCH,
    }
}

/// 建一筆 agent 紀錄。
pub fn agent(pane: &str, workspace: &str, tab: &str, name: &str, status: AgentStatus) -> Agent {
    Agent {
        agent: name.to_string(),
        pane_id: pane_id(pane),
        workspace_id: workspace_id(workspace),
        tab_id: tab_id(tab),
        agent_status: status,
    }
}

/// 三層都沒有焦點的 `Focused`。
pub fn empty_focused() -> Focused {
    Focused {
        workspace_id: None,
        tab_id: None,
        pane_id: None,
    }
}

/// 建一份 `RuntimeSnapshot`；`server_version`／`protocol` 用固定測試值，
/// `protocol_warning` 固定 `None`（1.4 之後要測警告時再另加 builder）。
pub fn snapshot(
    workspaces: Vec<Workspace>,
    tabs: Vec<Tab>,
    panes: Vec<Pane>,
    agents: Vec<Agent>,
    focused: Focused,
) -> RuntimeSnapshot {
    RuntimeSnapshot {
        server_version: "0.9.0".to_string(),
        protocol: 1,
        workspaces,
        tabs,
        panes,
        agents,
        focused,
        protocol_warning: None,
    }
}

// ---------------------------------------------------------------------------
// FakeRuntime（Task 1.7 建立，1.8／1.9 沿用）
// ---------------------------------------------------------------------------

/// 假 runtime 收到過的一次呼叫。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Call {
    /// [`AgentRuntime::subscribe`]。
    Subscribe,
    /// [`AgentRuntime::snapshot`]。
    Snapshot,
}

/// 一次呼叫連同它發生的時間（虛擬時鐘）；1.9 量退避間隔用。
#[derive(Clone, Copy, Debug)]
pub struct CallRecord {
    /// 呼叫種類。
    pub call: Call,
    /// 呼叫發生的時間。
    pub at: Instant,
}

/// 一筆 snapshot 回應：要回什麼，以及這次呼叫要花多久才回。
pub type SnapshotResponse = (Result<RuntimeSnapshot, RuntimeError>, Duration);

/// 事件流生產端的 sender（[`RuntimeEvents::channel`] 的那一半）。
type EventSender = mpsc::Sender<Result<RuntimeEvent, RuntimeError>>;

/// 依序回應的佇列：用完最後一筆之後就一直重複最後一筆。
struct ResponseQueue<T> {
    items: Vec<T>,
    next: usize,
}

impl<T> ResponseQueue<T> {
    fn new(items: Vec<T>) -> Self {
        Self { items, next: 0 }
    }

    /// 取下一筆；空佇列回 `None`，否則索引夾在最後一筆（重複最後一筆）。
    fn next_item(&mut self) -> Option<&T> {
        if self.items.is_empty() {
            return None;
        }
        let index = self.next.min(self.items.len() - 1);
        self.next += 1;
        Some(&self.items[index])
    }
}

/// `RuntimeError` 沒有 `Clone`（它是錯誤型別，本來就不該隨便複製），但回應佇列要能
/// 重複回同一筆，所以在測試鷹架裡自己複製一份。
fn clone_error(err: &RuntimeError) -> RuntimeError {
    match err {
        RuntimeError::Unavailable {
            reason,
            retry_after,
        } => RuntimeError::Unavailable {
            reason: reason.clone(),
            retry_after: *retry_after,
        },
        RuntimeError::Failed(message) => RuntimeError::Failed(message.clone()),
        RuntimeError::PaneNotFound { pane_id } => RuntimeError::PaneNotFound {
            pane_id: pane_id.clone(),
        },
    }
}

fn clone_snapshot_response(response: &SnapshotResponse) -> SnapshotResponse {
    let (result, delay) = response;
    let cloned = match result {
        Ok(snapshot) => Ok(snapshot.clone()),
        Err(err) => Err(clone_error(err)),
    };
    (cloned, *delay)
}

/// 被 drop 時翻旗標。放在事件流的 reader task 裡：`RuntimeEvents` 一被釋放就會
/// `abort()` 這個 task，task 的狀態（含這個 guard）被丟掉時旗標就翻起來，
/// [`FakeRuntime::stream_released`] 因此看得到「事件流被釋放」。
struct ReleaseGuard(Arc<AtomicBool>);

impl Drop for ReleaseGuard {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

/// 可控的假 [`AgentRuntime`]：記錄呼叫順序與時間、可設定（含延遲的）snapshot 回應
/// 佇列與 subscribe 成敗佇列、可從測試端注入事件或關閉事件流、可觀察事件流有沒有
/// 被釋放。
///
/// 用法：`Arc::new(FakeRuntime::new("win").snapshot_responses(..))`，再以
/// `let runtime: Arc<dyn AgentRuntime> = fake.clone();` 交給驅動器。
pub struct FakeRuntime {
    id: RuntimeId,
    snapshots: Mutex<ResponseQueue<SnapshotResponse>>,
    subscribes: Mutex<ResponseQueue<Result<(), RuntimeError>>>,
    calls: Mutex<Vec<CallRecord>>,
    /// 每條建立過的事件流的 sender，依建立順序；`None` 表示已被 [`FakeRuntime::end_stream`]
    /// 關掉（保留位置，讓索引與 `released` 對齊）。
    senders: Mutex<Vec<Option<EventSender>>>,
    /// 每條建立過的事件流的釋放旗標，依建立順序。
    released: Mutex<Vec<Arc<AtomicBool>>>,
    /// `read_output` 的腳本化回應：指定的 pane 回指定文字，其餘回 `PaneNotFound`
    /// （task 2.2，spec `runtime-model`「假 runtime 回應輸出」）。
    outputs: Mutex<HashMap<PaneId, String>>,
}

impl FakeRuntime {
    /// 建一個假 runtime：預設 subscribe 永遠成功、snapshot 尚未設定（呼叫會拿到
    /// `Failed`，藉此逼測試自己講清楚要回什麼）。
    pub fn new(id: &str) -> Self {
        Self {
            id: RuntimeId::new(id),
            snapshots: Mutex::new(ResponseQueue::new(Vec::new())),
            subscribes: Mutex::new(ResponseQueue::new(vec![Ok(())])),
            calls: Mutex::new(Vec::new()),
            senders: Mutex::new(Vec::new()),
            released: Mutex::new(Vec::new()),
            outputs: Mutex::new(HashMap::new()),
        }
    }

    /// 幫某個 pane 設定 `read_output` 要回的文字；沒有設定過的 pane 一律回
    /// `RuntimeError::PaneNotFound`。
    pub fn pane_output(mut self, pane: PaneId, text: impl Into<String>) -> Self {
        self.outputs.get_mut().unwrap().insert(pane, text.into());
        self
    }

    /// 設定 snapshot 回應佇列：每筆是「回應 + 這次呼叫要花多久」，依序取用，
    /// 用完最後一筆就一直重複最後一筆。
    pub fn snapshot_responses(mut self, responses: Vec<SnapshotResponse>) -> Self {
        *self.snapshots.get_mut().unwrap() = ResponseQueue::new(responses);
        self
    }

    /// 設定 subscribe 回應佇列：`Ok(())` 表示建流成功（會真的建一條可注入事件的流），
    /// `Err` 表示建流失敗；依序取用，用完最後一筆就一直重複最後一筆。
    pub fn subscribe_responses(mut self, responses: Vec<Result<(), RuntimeError>>) -> Self {
        *self.subscribes.get_mut().unwrap() = ResponseQueue::new(responses);
        self
    }

    /// 把一筆事件（或事件流錯誤）送進**最新**一條事件流。
    pub fn push_event(&self, item: Result<RuntimeEvent, RuntimeError>) {
        let senders = self.senders.lock().unwrap();
        let sender = senders
            .last()
            .and_then(|slot| slot.as_ref())
            .expect("還沒建立事件流，或最新一條已經被 end_stream 關掉");
        if sender.try_send(item).is_err() {
            panic!("事件送不進通道（滿了或對端已釋放）");
        }
    }

    /// 關掉最新一條事件流的 sender，讓消費端的 `next()` 回 `None`。
    pub fn end_stream(&self) {
        let mut senders = self.senders.lock().unwrap();
        if let Some(slot) = senders.last_mut() {
            *slot = None;
        }
    }

    /// 第 `n` 條事件流（依建立順序，從 0 起算）是否已被消費端釋放。
    ///
    /// 注意 `JoinHandle::abort()` 是非同步的：`RuntimeEvents` 被 drop 之後，還要讓
    /// 排程器跑一輪，被 abort 的 reader task 才會真的被丟掉、旗標才會翻。
    pub fn stream_released(&self, n: usize) -> bool {
        self.released
            .lock()
            .unwrap()
            .get(n)
            .is_some_and(|flag| flag.load(Ordering::SeqCst))
    }

    /// 目前為止建立過幾條事件流。
    pub fn stream_count(&self) -> usize {
        self.released.lock().unwrap().len()
    }

    /// 依序回傳收到過的呼叫種類。
    pub fn calls(&self) -> Vec<Call> {
        self.calls
            .lock()
            .unwrap()
            .iter()
            .map(|record| record.call)
            .collect()
    }

    /// 依序回傳收到過的呼叫連同發生時間（1.9 量退避間隔用）。
    pub fn call_log(&self) -> Vec<CallRecord> {
        self.calls.lock().unwrap().clone()
    }

    fn record(&self, call: Call) {
        self.calls.lock().unwrap().push(CallRecord {
            call,
            at: Instant::now(),
        });
    }

    /// 取下一筆 snapshot 回應；沒設定過就回一個講清楚原因的 `Failed`。
    fn next_snapshot(&self) -> SnapshotResponse {
        let mut queue = self.snapshots.lock().unwrap();
        match queue.next_item() {
            Some(response) => clone_snapshot_response(response),
            None => (
                Err(RuntimeError::Failed(
                    "FakeRuntime 沒有設定 snapshot 回應".to_string(),
                )),
                Duration::ZERO,
            ),
        }
    }

    /// 取下一筆 subscribe 回應；佇列空了就當成功。
    fn next_subscribe(&self) -> Result<(), RuntimeError> {
        let mut queue = self.subscribes.lock().unwrap();
        match queue.next_item() {
            Some(Ok(())) | None => Ok(()),
            Some(Err(err)) => Err(clone_error(err)),
        }
    }
}

#[async_trait::async_trait]
impl AgentRuntime for FakeRuntime {
    fn id(&self) -> &RuntimeId {
        &self.id
    }

    async fn snapshot(&self) -> Result<RuntimeSnapshot, RuntimeError> {
        // 先記呼叫再睡：呼叫順序反映的是「什麼時候發出這個請求」，不是什麼時候回應。
        self.record(Call::Snapshot);
        let (result, delay) = self.next_snapshot();
        if !delay.is_zero() {
            tokio::time::sleep(delay).await;
        }
        result
    }

    async fn subscribe(&self) -> Result<RuntimeEvents, RuntimeError> {
        self.record(Call::Subscribe);
        self.next_subscribe()?;

        let (tx, unstarted) = RuntimeEvents::channel();
        let flag = Arc::new(AtomicBool::new(false));
        let guard = ReleaseGuard(flag.clone());
        // reader task 什麼事都不做，只負責被 abort 時讓 guard 翻旗標——真正的 runtime
        // 在這裡會是 gRPC 讀取迴圈，abort 它就等於關掉那條連線。
        let task = tokio::spawn(async move {
            let _guard = guard;
            std::future::pending::<()>().await;
        });

        self.senders.lock().unwrap().push(Some(tx));
        self.released.lock().unwrap().push(flag);
        Ok(unstarted.start(vec![task]))
    }

    /// 腳本化回應：`pane_output` 設定過的 pane 回那份文字（格式固定 `Text`、`truncated`
    /// 固定 `false`），其餘一律回 `PaneNotFound`。純記憶體查表，不 `await`。
    async fn read_output(
        &self,
        pane: &PaneId,
        _max_lines: u32,
    ) -> Result<PaneOutput, RuntimeError> {
        match self.outputs.lock().unwrap().get(pane) {
            Some(text) => Ok(PaneOutput::plain(text.clone(), false)),
            None => Err(RuntimeError::PaneNotFound {
                pane_id: pane.clone(),
            }),
        }
    }
}
