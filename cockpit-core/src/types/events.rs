//! Runtime 事件流的型別。`RuntimeEvent` 是狀態庫用來把 `RuntimeSnapshot` 增量更新到最新的
//! 單位；每個變體的完整套用語意（例如「逐筆套用」「取代整組」）在 1.3 的
//! `runtime-model` spec 裡定義，這裡只註記一句話對照。

use crate::types::agent_status::AgentStatus;
use crate::types::ids::{PaneId, TabId, WorkspaceId};
use crate::types::model::{Pane, Tab, Workspace};

/// 焦點的增量變化：每個欄位 `None` 表示「該層焦點不變」，跟 `Focused`（`None` 表示
/// 「該層沒有焦點」）語意不同，因此不共用同一個型別。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FocusChange {
    /// 新的 workspace 焦點；`None` 表示 workspace 層焦點不變。
    pub workspace_id: Option<WorkspaceId>,
    /// 新的 tab 焦點；`None` 表示 tab 層焦點不變。
    pub tab_id: Option<TabId>,
    /// 新的 pane 焦點；`None` 表示 pane 層焦點不變。
    pub pane_id: Option<PaneId>,
}

/// 狀態庫要套用到快照上的一筆增量事件。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RuntimeEvent {
    /// 新增或更新一個 workspace：狀態庫依 id 做 upsert。
    WorkspaceUpserted(Workspace),
    /// 整批取代目前的 workspace 清單。
    WorkspacesReplaced(Vec<Workspace>),
    /// 移除一個 workspace 及其底下的 tab／pane。
    WorkspaceRemoved(WorkspaceId),
    /// workspace 的顯示名稱被改掉。
    WorkspaceRelabeled {
        /// 被改名的 workspace。
        id: WorkspaceId,
        /// 新的顯示名稱。
        label: String,
    },
    /// 新增或更新一個 tab：狀態庫依 id 做 upsert。
    TabUpserted(Tab),
    /// 整批取代某個 workspace 底下的 tab 清單。
    TabsReplaced {
        /// 這批 tab 所屬的 workspace。
        workspace_id: WorkspaceId,
        /// 取代後的完整 tab 清單。
        tabs: Vec<Tab>,
    },
    /// 移除一個 tab 及其底下的 pane。
    TabRemoved(TabId),
    /// tab 的顯示名稱被改掉。
    TabRelabeled {
        /// 被改名的 tab。
        id: TabId,
        /// 新的顯示名稱。
        label: String,
    },
    /// 新增或更新一個 pane：狀態庫依 id 做 upsert。
    PaneUpserted(Pane),
    /// 一個 pane 從舊 id 搬到新狀態（例如搬到別的 tab）；狀態庫要先移除舊的再放新的。
    PaneMoved {
        /// 搬移前的 pane id。
        previous: PaneId,
        /// 搬移後的完整 pane 狀態。
        pane: Pane,
    },
    /// 移除一個 pane。
    PaneRemoved(PaneId),
    /// pane 底下的行程結束（pane 本身可能還留著，只是標記為已結束）。
    PaneExited(PaneId),
    /// 在某個 pane 偵測到 agent 出現或釋放；`agent` 為 `None` 表示釋放。
    AgentDetected {
        /// 偵測到變化的 pane。
        pane_id: PaneId,
        /// 偵測到的 agent 名稱；`None` 表示這個 pane 不再有 agent。
        agent: Option<String>,
    },
    /// 某個 pane 的 agent 狀態改變。
    AgentStatusChanged {
        /// 狀態改變的 pane。
        pane_id: PaneId,
        /// 新的 agent 狀態。
        status: AgentStatus,
        /// 當下的 pane 標題（用於狀態庫同步更新顯示用資訊）。
        title: Option<String>,
        /// 當下偵測到的 agent 名稱。
        agent: Option<String>,
    },
    /// 使用者焦點改變。
    FocusChanged(FocusChange),
    /// 狀態庫偵測到自己與 runtime 的認知不一致，需要重新拉快照校正；細節見 §6.3。
    Drift {
        /// 觸發 drift 的原因，供除錯與記錄。
        reason: String,
    },
    /// 僅供記錄的雜項通知，不改變任何狀態，只會被收進 recent events。
    Noted {
        /// 通知的分類。
        kind: String,
    },
}
