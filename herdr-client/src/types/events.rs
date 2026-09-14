//! 事件與訂閱型別（spec `herdr-event-subscription`；design D4；設計文件 §2.3、§5.2）。
//!
//! `EventKind`（26 種，底線命名）與 `SubscriptionEventKind`（3 種，點號命名）是「已知名稱
//! 集合」判定的依據：`FromStr` 成功代表落在對應的集合裡，失敗（`Err`）交給呼叫端歸類為
//! 未知事件（design D4）。兩者都不放 `#[serde(other)]`——未知名稱不該被這兩個型別默默吃掉，
//! 而是由呼叫端（task 4.3 的 `IncomingEvent::Unknown`）保留原始字串。

use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use super::agent_status::AgentStatus;
use super::snapshot::{PaneInfo, TabInfo, WorkspaceInfo};

/// 解析 `EventKind`／`SubscriptionEventKind` 失敗時的錯誤，保留原始字串供呼叫端記錄。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownEventName(pub String);

impl fmt::Display for UnknownEventName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unknown event name: \"{}\"", self.0)
    }
}

impl std::error::Error for UnknownEventName {}

/// 生命週期事件的種類：`event` 欄位為底線命名時的 26 種值（設計文件 §2.3）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    WorkspaceCreated,
    WorkspaceUpdated,
    WorkspaceMetadataUpdated,
    WorkspaceClosed,
    WorkspaceRenamed,
    WorkspaceMoved,
    WorkspaceReordered,
    WorkspaceFocused,
    WorktreeCreated,
    WorktreeOpened,
    WorktreeRemoved,
    TabCreated,
    TabClosed,
    TabRenamed,
    TabMoved,
    TabFocused,
    PaneCreated,
    PaneClosed,
    PaneUpdated,
    PaneFocused,
    PaneMoved,
    PaneOutputChanged,
    PaneExited,
    PaneAgentDetected,
    PaneAgentStatusChanged,
    LayoutUpdated,
}

impl EventKind {
    /// 底線命名的原始字串，逐字對照 schema 的 `EventKind` enum。
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            EventKind::WorkspaceCreated => "workspace_created",
            EventKind::WorkspaceUpdated => "workspace_updated",
            EventKind::WorkspaceMetadataUpdated => "workspace_metadata_updated",
            EventKind::WorkspaceClosed => "workspace_closed",
            EventKind::WorkspaceRenamed => "workspace_renamed",
            EventKind::WorkspaceMoved => "workspace_moved",
            EventKind::WorkspaceReordered => "workspace_reordered",
            EventKind::WorkspaceFocused => "workspace_focused",
            EventKind::WorktreeCreated => "worktree_created",
            EventKind::WorktreeOpened => "worktree_opened",
            EventKind::WorktreeRemoved => "worktree_removed",
            EventKind::TabCreated => "tab_created",
            EventKind::TabClosed => "tab_closed",
            EventKind::TabRenamed => "tab_renamed",
            EventKind::TabMoved => "tab_moved",
            EventKind::TabFocused => "tab_focused",
            EventKind::PaneCreated => "pane_created",
            EventKind::PaneClosed => "pane_closed",
            EventKind::PaneUpdated => "pane_updated",
            EventKind::PaneFocused => "pane_focused",
            EventKind::PaneMoved => "pane_moved",
            EventKind::PaneOutputChanged => "pane_output_changed",
            EventKind::PaneExited => "pane_exited",
            EventKind::PaneAgentDetected => "pane_agent_detected",
            EventKind::PaneAgentStatusChanged => "pane_agent_status_changed",
            EventKind::LayoutUpdated => "layout_updated",
        }
    }
}

impl FromStr for EventKind {
    type Err = UnknownEventName;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "workspace_created" => EventKind::WorkspaceCreated,
            "workspace_updated" => EventKind::WorkspaceUpdated,
            "workspace_metadata_updated" => EventKind::WorkspaceMetadataUpdated,
            "workspace_closed" => EventKind::WorkspaceClosed,
            "workspace_renamed" => EventKind::WorkspaceRenamed,
            "workspace_moved" => EventKind::WorkspaceMoved,
            "workspace_reordered" => EventKind::WorkspaceReordered,
            "workspace_focused" => EventKind::WorkspaceFocused,
            "worktree_created" => EventKind::WorktreeCreated,
            "worktree_opened" => EventKind::WorktreeOpened,
            "worktree_removed" => EventKind::WorktreeRemoved,
            "tab_created" => EventKind::TabCreated,
            "tab_closed" => EventKind::TabClosed,
            "tab_renamed" => EventKind::TabRenamed,
            "tab_moved" => EventKind::TabMoved,
            "tab_focused" => EventKind::TabFocused,
            "pane_created" => EventKind::PaneCreated,
            "pane_closed" => EventKind::PaneClosed,
            "pane_updated" => EventKind::PaneUpdated,
            "pane_focused" => EventKind::PaneFocused,
            "pane_moved" => EventKind::PaneMoved,
            "pane_output_changed" => EventKind::PaneOutputChanged,
            "pane_exited" => EventKind::PaneExited,
            "pane_agent_detected" => EventKind::PaneAgentDetected,
            "pane_agent_status_changed" => EventKind::PaneAgentStatusChanged,
            "layout_updated" => EventKind::LayoutUpdated,
            other => return Err(UnknownEventName(other.to_string())),
        })
    }
}

/// 每 pane 訂閱推送的事件種類：`event` 欄位為點號命名時的 3 種值。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SubscriptionEventKind {
    #[serde(rename = "pane.output_matched")]
    PaneOutputMatched,
    #[serde(rename = "pane.agent_status_changed")]
    PaneAgentStatusChanged,
    #[serde(rename = "pane.scroll_changed")]
    PaneScrollChanged,
}

impl SubscriptionEventKind {
    /// 點號命名的原始字串，逐字對照 schema 的 `SubscriptionEventKind` enum。
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            SubscriptionEventKind::PaneOutputMatched => "pane.output_matched",
            SubscriptionEventKind::PaneAgentStatusChanged => "pane.agent_status_changed",
            SubscriptionEventKind::PaneScrollChanged => "pane.scroll_changed",
        }
    }
}

impl FromStr for SubscriptionEventKind {
    type Err = UnknownEventName;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "pane.output_matched" => SubscriptionEventKind::PaneOutputMatched,
            "pane.agent_status_changed" => SubscriptionEventKind::PaneAgentStatusChanged,
            "pane.scroll_changed" => SubscriptionEventKind::PaneScrollChanged,
            other => return Err(UnknownEventName(other.to_string())),
        })
    }
}

/// `events.subscribe` 的一筆訂閱項目。24 個無參數的生命週期訂閱序列化為只有 `type` 欄位的
/// 物件（例如 `{"type":"workspace.created"}`）；每 pane 的 agent 狀態訂閱另外帶 `pane_id`、
/// 不含 `agent_status` 過濾欄位（觀察子集不用 server 端過濾）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Subscription {
    #[serde(rename = "workspace.created")]
    WorkspaceCreated,
    #[serde(rename = "workspace.updated")]
    WorkspaceUpdated,
    #[serde(rename = "workspace.metadata_updated")]
    WorkspaceMetadataUpdated,
    #[serde(rename = "workspace.renamed")]
    WorkspaceRenamed,
    #[serde(rename = "workspace.moved")]
    WorkspaceMoved,
    #[serde(rename = "workspace.reordered")]
    WorkspaceReordered,
    #[serde(rename = "workspace.closed")]
    WorkspaceClosed,
    #[serde(rename = "workspace.focused")]
    WorkspaceFocused,
    #[serde(rename = "worktree.created")]
    WorktreeCreated,
    #[serde(rename = "worktree.opened")]
    WorktreeOpened,
    #[serde(rename = "worktree.removed")]
    WorktreeRemoved,
    #[serde(rename = "tab.created")]
    TabCreated,
    #[serde(rename = "tab.closed")]
    TabClosed,
    #[serde(rename = "tab.focused")]
    TabFocused,
    #[serde(rename = "tab.renamed")]
    TabRenamed,
    #[serde(rename = "tab.moved")]
    TabMoved,
    #[serde(rename = "pane.created")]
    PaneCreated,
    #[serde(rename = "pane.closed")]
    PaneClosed,
    #[serde(rename = "pane.updated")]
    PaneUpdated,
    #[serde(rename = "pane.focused")]
    PaneFocused,
    #[serde(rename = "pane.moved")]
    PaneMoved,
    #[serde(rename = "pane.exited")]
    PaneExited,
    #[serde(rename = "pane.agent_detected")]
    PaneAgentDetected,
    #[serde(rename = "layout.updated")]
    LayoutUpdated,
    #[serde(rename = "pane.agent_status_changed")]
    PaneAgentStatusChanged { pane_id: String },
}

impl Subscription {
    /// 24 種無參數的生命週期訂閱（不含每 pane 的 `PaneAgentStatusChanged`）。
    #[must_use]
    pub fn all_lifecycle() -> Vec<Subscription> {
        vec![
            Subscription::WorkspaceCreated,
            Subscription::WorkspaceUpdated,
            Subscription::WorkspaceMetadataUpdated,
            Subscription::WorkspaceRenamed,
            Subscription::WorkspaceMoved,
            Subscription::WorkspaceReordered,
            Subscription::WorkspaceClosed,
            Subscription::WorkspaceFocused,
            Subscription::WorktreeCreated,
            Subscription::WorktreeOpened,
            Subscription::WorktreeRemoved,
            Subscription::TabCreated,
            Subscription::TabClosed,
            Subscription::TabFocused,
            Subscription::TabRenamed,
            Subscription::TabMoved,
            Subscription::PaneCreated,
            Subscription::PaneClosed,
            Subscription::PaneUpdated,
            Subscription::PaneFocused,
            Subscription::PaneMoved,
            Subscription::PaneExited,
            Subscription::PaneAgentDetected,
            Subscription::LayoutUpdated,
        ]
    }
}

/// `events.subscribe` 的參數。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct EventsSubscribeParams {
    pub subscriptions: Vec<Subscription>,
}

// ---------------------------------------------------------------------------
// 事件 payload 型別（觀察子集；設計文件 §2.3「Payload 完整性」表、
// spec `herdr-observer-types`「事件 payload 型別」）。
// ---------------------------------------------------------------------------

/// `workspace_created`、`workspace_updated`、`workspace_metadata_updated` 的 payload：
/// `data` 把完整的 `WorkspaceInfo` 包在 `workspace` 欄位裡（非攤平；已用 schema 與真機
/// fixture 核對）。
///
/// 全分支最終 review finding 1：不在容器層加 `#[serde(default)]`——`workspace` 是 schema
/// 必填欄位，缺席時應該讓 `serde_json::from_value` 直接失敗，不能被靜默補成
/// `WorkspaceInfo::default()`（design D13；跟 `src/types/snapshot.rs` 對 `WorkspaceInfo`
/// 等型別的既有作法一致）。本檔其餘 payload 型別都比照同一套規則：容器層不加
/// `#[serde(default)]`，只在 schema 真正非必填的欄位上加欄位級 `#[serde(default)]`。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct WorkspacePayload {
    pub workspace: WorkspaceInfo,
}

/// `tab_created` 的 payload：完整 `TabInfo` 包在 `tab` 欄位裡。`tab` 是 schema 必填欄位。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct TabPayload {
    pub tab: TabInfo,
}

/// `pane_created`、`pane_updated` 的 payload：完整 `PaneInfo` 包在 `pane` 欄位裡。`pane`
/// 是 schema 必填欄位。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct PanePayload {
    pub pane: PaneInfo,
}

/// `workspace_moved`、`workspace_reordered` 的 payload：整份順序後的 `workspaces` 陣列。
/// `workspaces` 在兩個事件的 schema 裡都是必填欄位。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct WorkspacesReplacedPayload {
    pub workspaces: Vec<WorkspaceInfo>,
}

/// `tab_moved` 的 payload。四個欄位在 schema 裡都是必填欄位。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct TabMovedPayload {
    pub tab_id: String,
    pub workspace_id: String,
    pub insert_index: u32,
    pub tabs: Vec<TabInfo>,
}

/// `workspace_closed` 的 payload：`workspace_id` 是 schema 必填欄位，`workspace` 是選填的
/// 舊 `WorkspaceInfo`（缺席時為 `None`）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct WorkspaceClosedPayload {
    pub workspace_id: String,
    #[serde(default)]
    pub workspace: Option<WorkspaceInfo>,
}

/// `workspace_renamed` 的 payload。兩個欄位在 schema 裡都是必填欄位。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct WorkspaceRenamedPayload {
    pub workspace_id: String,
    pub label: String,
}

/// `tab_closed` 的 payload。兩個欄位在 schema 裡都是必填欄位。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct TabClosedPayload {
    pub tab_id: String,
    pub workspace_id: String,
}

/// `tab_renamed` 的 payload。三個欄位在 schema 裡都是必填欄位。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct TabRenamedPayload {
    pub tab_id: String,
    pub workspace_id: String,
    pub label: String,
}

/// `pane_closed`、`pane_exited` 共用的 payload。兩個欄位在 schema 裡都是必填欄位。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct PaneClosedPayload {
    pub pane_id: String,
    pub workspace_id: String,
}

/// `workspace_focused` 的 payload。`workspace_id` 是 schema 必填欄位。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct WorkspaceFocusedPayload {
    pub workspace_id: String,
}

/// `tab_focused` 的 payload。兩個欄位在 schema 裡都是必填欄位。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct TabFocusedPayload {
    pub tab_id: String,
    pub workspace_id: String,
}

/// `pane_focused` 的 payload。兩個欄位在 schema 裡都是必填欄位。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct PaneFocusedPayload {
    pub pane_id: String,
    pub workspace_id: String,
}

/// `pane_moved` 的 payload：pane id 在移動後會變，`previous_*` 三個欄位保留舊 id、`pane`
/// 是移動後的完整 `PaneInfo`（新 id 在 `pane.pane_id`）——這四個欄位是 schema 必填欄位。
/// `created_workspace`、`created_tab`、`closed_workspace_id`、`closed_tab_id` 是 schema 有
/// 但設計文件 §2.3 payload 表沒列的選填欄位（design Risks），移動跨 workspace／tab 邊界時
/// 才會出現，缺席時為 `None`。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct PaneMovedPayload {
    pub previous_pane_id: String,
    pub previous_workspace_id: String,
    pub previous_tab_id: String,
    pub pane: PaneInfo,
    #[serde(default)]
    pub created_workspace: Option<WorkspaceInfo>,
    #[serde(default)]
    pub created_tab: Option<TabInfo>,
    #[serde(default)]
    pub closed_workspace_id: Option<String>,
    #[serde(default)]
    pub closed_tab_id: Option<String>,
}

/// `pane_agent_detected` 的 payload：`pane_id`、`workspace_id` 是 schema 必填欄位。`agent`
/// 為 `null`、`released` 為 `true` 代表 agent 離開該 pane，兩者皆為 schema 選填欄位；
/// `final_status` 是 schema 有但設計文件 §2.3 payload 表沒列的選填欄位。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct PaneAgentDetectedPayload {
    pub pane_id: String,
    pub workspace_id: String,
    #[serde(default)]
    pub agent: Option<String>,
    #[serde(default)]
    pub released: bool,
    #[serde(default)]
    pub final_status: Option<AgentStatus>,
}

/// `pane.agent_status_changed` 的 payload；生命週期版（`pane_agent_status_changed`，本
/// change 不會實際收到，見設計文件 §2.3）與每 pane 版兩軌欄位相同，共用這個型別。
/// `pane_id`、`workspace_id`、`agent_status` 是兩個 schema 變體共同的必填欄位；`agent`、
/// `display_agent`、`title`、`state_labels` 皆為選填。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct PaneAgentStatusChangedPayload {
    pub pane_id: String,
    pub workspace_id: String,
    pub agent_status: AgentStatus,
    #[serde(default)]
    pub agent: Option<String>,
    #[serde(default)]
    pub display_agent: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub state_labels: Option<HashMap<String, String>>,
}
