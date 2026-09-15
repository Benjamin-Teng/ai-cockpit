//! 狀態庫的靜態模型：`RuntimeSnapshot` 是某個時間點的完整快照，`RuntimeEvent`（見
//! `events.rs`）則是之後的增量變化。HERDR snapshot 的 layouts 陣列刻意不建模：
//! change 1 不畫 pane 幾何配置。

use std::time::SystemTime;

use serde::{Deserialize, Serialize};

use crate::types::agent_status::AgentStatus;
use crate::types::ids::{PaneId, TabId, WorkspaceId};

/// 一個 workspace（HERDR 的頂層分頁群組）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Workspace {
    /// 此 runtime 內的 workspace id。
    pub id: WorkspaceId,
    /// 使用者設定的顯示名稱；`None` 表示尚未命名。
    pub label: Option<String>,
    /// 顯示用的序號（例如 UI 上的 `1`、`2`）。
    pub number: u32,
    /// 這個 workspace 底下彙總出的 agent 狀態。
    pub agent_status: AgentStatus,
    /// 是否為目前使用者焦點所在的 workspace。
    pub focused: bool,
}

/// 一個 tab，隸屬於某個 workspace。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tab {
    /// 此 runtime 內的 tab id。
    pub id: TabId,
    /// 所屬 workspace 的 id。
    pub workspace_id: WorkspaceId,
    /// 顯示用的序號。
    pub number: u32,
    /// 這個 tab 底下彙總出的 agent 狀態。
    pub agent_status: AgentStatus,
    /// 是否為目前使用者焦點所在的 tab。
    pub focused: bool,
}

/// 一個 pane，隸屬於某個 tab；可能正在跑一個 agent，也可能沒有。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pane {
    /// 此 runtime 內的 pane id。
    pub id: PaneId,
    /// 所屬 workspace 的 id。
    pub workspace_id: WorkspaceId,
    /// 所屬 tab 的 id。
    pub tab_id: TabId,
    /// 目前偵測到在這個 pane 執行的 agent 名稱；`None` 表示沒有偵測到 agent。
    pub agent: Option<String>,
    /// 這個 pane 的 agent 狀態。
    pub agent_status: AgentStatus,
    /// pane 標題（通常來自終端機的視窗標題）。
    pub title: Option<String>,
    /// pane 目前的工作目錄。
    pub cwd: Option<String>,
    /// 使用者設定的顯示名稱。
    pub label: Option<String>,
    /// 是否為目前使用者焦點所在的 pane。
    pub focused: bool,
    /// pane 底下的行程是否已結束。
    pub exited: bool,
    /// 這筆資料最後一次更新的時間。
    pub updated_at: SystemTime,
}

/// 一個目前偵測到的 agent 及其所在位置，供狀態庫依 agent 聚合查詢。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Agent {
    /// agent 名稱（例如 `"claude"`、`"codex"`）。
    pub agent: String,
    /// 這個 agent 執行所在的 pane id。
    pub pane_id: PaneId,
    /// 這個 agent 執行所在的 workspace id。
    pub workspace_id: WorkspaceId,
    /// 這個 agent 執行所在的 tab id。
    pub tab_id: TabId,
    /// 這個 agent 目前的狀態。
    pub agent_status: AgentStatus,
}

/// 目前使用者焦點在各層級的位置；`None` 表示該層目前沒有焦點。
///
/// 直接被 1.5 的 `ProjectedRuntime` 重用，因此也 derive `Serialize`／`Deserialize`
/// （欄位名與投影 JSON 的 `focused` 一致，`None` 序列化為 `null`）。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Focused {
    /// 目前有焦點的 workspace；`None` 表示沒有任何 workspace 有焦點。
    pub workspace_id: Option<WorkspaceId>,
    /// 目前有焦點的 tab；`None` 表示沒有任何 tab 有焦點。
    pub tab_id: Option<TabId>,
    /// 目前有焦點的 pane；`None` 表示沒有任何 pane 有焦點。
    pub pane_id: Option<PaneId>,
}

/// 某個 runtime 在某個時間點的完整狀態快照，用來初始化或重建狀態庫。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimeSnapshot {
    /// HERDR server 回報的版本字串。
    pub server_version: String,
    /// HERDR 通訊協定版本號。
    pub protocol: u32,
    /// 快照當下的所有 workspace。
    pub workspaces: Vec<Workspace>,
    /// 快照當下的所有 tab。
    pub tabs: Vec<Tab>,
    /// 快照當下的所有 pane。
    pub panes: Vec<Pane>,
    /// 快照當下偵測到的所有 agent。
    pub agents: Vec<Agent>,
    /// 快照當下的焦點位置。
    pub focused: Focused,
    /// 協定相容性警告（例如 server 版本比預期新／舊）；`None` 表示沒有警告。
    pub protocol_warning: Option<String>,
}
