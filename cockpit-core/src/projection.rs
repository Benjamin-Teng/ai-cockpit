//! `RuntimeStore` → `ProjectedState` 的純函數投影（spec `state-projection`；設計文件
//! §6.4、design D7、D9、D12）。不修改 `store`；`version` 由呼叫端決定，本模組只原樣
//! 填入（1.6 的投影任務負責決定何時遞增）。所有時間欄位以 `chrono` 轉成 RFC 3339
//! （UTC、秒精度），格式固定 `YYYY-MM-DDTHH:MM:SSZ`。

use std::time::SystemTime;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::store::{RECENT_EVENTS_CAPACITY, RuntimeState, RuntimeStore};
use crate::types::agent_status::AgentStatus;
use crate::types::connection::ConnectionState;
use crate::types::ids::{PaneId, RuntimeId, TabId, WorkspaceId};
use crate::types::model::{Focused, Pane, Tab, Workspace};

/// 送給畫面的完整狀態（設計文件 §6.4）。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectedState {
    /// 遞增版本號，由呼叫端（1.6 的投影任務）決定何時遞增；本函數只原樣填入。
    pub version: u64,
    /// 這份投影產生的時間（RFC 3339，UTC，秒精度）。
    pub generated_at: String,
    /// 所有 runtime，依登記順序（＝設定檔順序）。
    pub runtimes: Vec<ProjectedRuntime>,
    /// 所有 runtime 的最近事件合併結果，最新在前，最多
    /// [`RECENT_EVENTS_CAPACITY`] 筆。
    pub recent_events: Vec<ProjectedEvent>,
}

impl ProjectedState {
    /// 比較兩份投影的實質內容，忽略 `version` 與 `generated_at`（design D9）：1.6 用它
    /// 決定要不要遞增 `version`、要不要廣播。
    pub fn content_eq(&self, other: &ProjectedState) -> bool {
        self.runtimes == other.runtimes && self.recent_events == other.recent_events
    }
}

/// 單一 runtime 的投影。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectedRuntime {
    /// runtime 識別碼。
    pub id: RuntimeId,
    /// runtime 種類（例如 `"herdr"`），登記時提供，原樣輸出。
    pub kind: String,
    /// 連線端點，登記時提供，原樣輸出。
    pub endpoint: String,
    /// 與這個 runtime 之間的連線狀態。
    pub connection: ProjectedConnection,
    /// 目前使用者焦點在各層級的位置。
    pub focused: Focused,
    /// 這個 runtime 目前的所有 workspace，依 `number` 遞增排序。
    pub workspaces: Vec<ProjectedWorkspace>,
}

/// 連線狀態的投影：三態序列化為 `{"state": "connecting" | "connected" | "disconnected", ...}`
/// （design D12）。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "lowercase")]
pub enum ProjectedConnection {
    /// 正在嘗試建立連線，尚未成功過。
    Connecting,
    /// 連線中。
    Connected {
        /// 這次連線建立的時間。
        since: String,
        /// 對方回報的 server 版本字串。
        server_version: String,
        /// 這次連線使用的協定版本號。
        protocol: u32,
        /// 最近一次成功取得快照的時間。
        last_snapshot_at: String,
        /// 協定相容性警告；`None` 序列化為 `null`，不省略（design D12）。
        protocol_warning: Option<String>,
    },
    /// 已斷線。
    Disconnected {
        /// 斷線原因。
        reason: String,
        /// 距離下次重試還要等多久（秒）。
        retry_in_secs: u64,
    },
}

/// 一個 workspace 的投影。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectedWorkspace {
    /// workspace id。
    pub id: WorkspaceId,
    /// 顯示名稱；`None` 序列化為 `null`，不省略。
    pub label: Option<String>,
    /// 顯示用序號，排序依據。
    pub number: u32,
    /// 這個 workspace 底下彙總出的 agent 狀態。
    pub agent_status: AgentStatus,
    /// 是否為目前使用者焦點所在的 workspace。
    pub focused: bool,
    /// 這個 workspace 底下的 tab，依 `number` 遞增排序；父層（workspace）不存在的 tab
    /// 不會出現在這裡（防禦，design D7）。
    pub tabs: Vec<ProjectedTab>,
}

/// 一個 tab 的投影。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectedTab {
    /// tab id。
    pub id: TabId,
    /// 顯示用序號，排序依據。
    pub number: u32,
    /// 這個 tab 底下彙總出的 agent 狀態。
    pub agent_status: AgentStatus,
    /// 是否為目前使用者焦點所在的 tab。
    pub focused: bool,
    /// 這個 tab 底下的 pane，依進入狀態庫的先後排序；父層（tab）不存在的 pane 不會出現
    /// 在這裡（防禦，design D7）。
    pub panes: Vec<ProjectedPane>,
}

/// 一個 pane 的投影。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectedPane {
    /// pane id。
    pub id: PaneId,
    /// 目前偵測到在這個 pane 執行的 agent 名稱；`None` 序列化為 `null`，不省略。
    pub agent: Option<String>,
    /// 這個 pane 的 agent 狀態。
    pub agent_status: AgentStatus,
    /// pane 標題；`None` 序列化為 `null`，不省略。
    pub title: Option<String>,
    /// pane 目前的工作目錄；`None` 序列化為 `null`，不省略。
    pub cwd: Option<String>,
    /// 使用者設定的顯示名稱；`None` 序列化為 `null`，不省略。
    pub label: Option<String>,
    /// 是否為目前使用者焦點所在的 pane。
    pub focused: bool,
    /// pane 底下的行程是否已結束。
    pub exited: bool,
    /// 這筆資料最後一次更新的時間（RFC 3339）。
    pub updated_at: String,
}

/// 一筆最近事件的投影（設計文件 §6.4）：主體 id 欄位依事件種類，沒有的省略
/// （`skip_serializing_if`），不是 `null`。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectedEvent {
    /// 事件發生（記錄）的時間（RFC 3339）。
    pub at: String,
    /// 這筆事件來自哪個 runtime。
    pub runtime: RuntimeId,
    /// 事件種類。
    pub kind: String,
    /// 影響到的 workspace；沒有就省略這個欄位。
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub workspace_id: Option<WorkspaceId>,
    /// 影響到的 tab；沒有就省略這個欄位。
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub tab_id: Option<TabId>,
    /// 影響到的 pane；沒有就省略這個欄位。
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub pane_id: Option<PaneId>,
    /// 額外說明文字。
    pub detail: String,
}

/// 由狀態庫純函數產生一份投影：不修改 `store`；`version` 原樣填入；`now` 用來產生
/// `generated_at`。
pub fn project(store: &RuntimeStore, version: u64, now: SystemTime) -> ProjectedState {
    let runtime_ids = store.runtime_ids();

    let runtimes = runtime_ids
        .iter()
        .filter_map(|id| {
            // 父層（runtime）理論上一定登記過（來自 `runtime_ids()`），但仍照 D7 的
            // 防禦精神處理：找不到就跳過，不 panic。
            let state = store.state(id)?;
            Some(ProjectedRuntime {
                id: id.clone(),
                kind: state.kind.clone(),
                endpoint: state.endpoint.clone(),
                connection: project_connection(&state.connection),
                focused: state.focused.clone(),
                workspaces: project_workspaces(state),
            })
        })
        .collect();

    let recent_events = project_recent_events(store, &runtime_ids);

    ProjectedState {
        version,
        generated_at: to_rfc3339(now),
        runtimes,
        recent_events,
    }
}

/// 把所有 runtime 的最近事件合併：依 `at` 由新到舊，同時間依 runtime 登記順序，取前
/// [`RECENT_EVENTS_CAPACITY`] 筆。
fn project_recent_events(store: &RuntimeStore, runtime_ids: &[RuntimeId]) -> Vec<ProjectedEvent> {
    let mut merged: Vec<(SystemTime, usize, ProjectedEvent)> = Vec::new();
    for (order, id) in runtime_ids.iter().enumerate() {
        for recent in store.recent_events(id) {
            merged.push((
                recent.at,
                order,
                ProjectedEvent {
                    at: to_rfc3339(recent.at),
                    runtime: id.clone(),
                    kind: recent.kind.clone(),
                    workspace_id: recent.workspace_id.clone(),
                    tab_id: recent.tab_id.clone(),
                    pane_id: recent.pane_id.clone(),
                    detail: recent.detail.clone(),
                },
            ));
        }
    }
    // 新到舊：`at` 遞減；同時間依登記順序（`order` 遞增，先登記的排前面）。
    merged.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    merged.truncate(RECENT_EVENTS_CAPACITY);
    merged.into_iter().map(|(_, _, event)| event).collect()
}

fn project_connection(connection: &ConnectionState) -> ProjectedConnection {
    match connection {
        ConnectionState::Connecting => ProjectedConnection::Connecting,
        ConnectionState::Connected {
            since,
            server_version,
            protocol,
            last_snapshot_at,
            protocol_warning,
        } => ProjectedConnection::Connected {
            since: to_rfc3339(*since),
            server_version: server_version.clone(),
            protocol: *protocol,
            last_snapshot_at: to_rfc3339(*last_snapshot_at),
            protocol_warning: protocol_warning.clone(),
        },
        ConnectionState::Disconnected { reason, retry_in } => ProjectedConnection::Disconnected {
            reason: reason.clone(),
            retry_in_secs: retry_in.as_secs(),
        },
    }
}

/// workspace 依 `number` 遞增排序，同號依 id 字串排序（`WorkspaceId` 的 `Ord` 就是內部
/// 字串的順序）。父層（runtime）本身不存在的 workspace 不會走到這裡（呼叫端已經用
/// `runtime_ids()`／`state()` 篩過）。
fn project_workspaces(state: &RuntimeState) -> Vec<ProjectedWorkspace> {
    let mut workspaces: Vec<&Workspace> = state.workspaces.values().collect();
    workspaces.sort_by(|a, b| a.number.cmp(&b.number).then_with(|| a.id.cmp(&b.id)));

    workspaces
        .into_iter()
        .map(|workspace| ProjectedWorkspace {
            id: workspace.id.clone(),
            label: workspace.label.clone(),
            number: workspace.number,
            agent_status: workspace.agent_status,
            focused: workspace.focused,
            tabs: project_tabs(state, &workspace.id),
        })
        .collect()
}

/// 只取 `workspace_id` 屬於這個 workspace 的 tab（父層不存在的 tab 因此不會出現在任何
/// workspace 底下，等於「不投影」，design D7）；依 `number` 遞增排序，同號依 id 字串。
fn project_tabs(state: &RuntimeState, workspace_id: &WorkspaceId) -> Vec<ProjectedTab> {
    let mut tabs: Vec<&Tab> = state
        .tabs
        .values()
        .filter(|tab| tab.workspace_id == *workspace_id)
        .collect();
    tabs.sort_by(|a, b| a.number.cmp(&b.number).then_with(|| a.id.cmp(&b.id)));

    tabs.into_iter()
        .map(|tab| ProjectedTab {
            id: tab.id.clone(),
            number: tab.number,
            agent_status: tab.agent_status,
            focused: tab.focused,
            panes: project_panes(state, workspace_id, &tab.id),
        })
        .collect()
}

/// 只取同時屬於這個 workspace 與 tab 的 pane（孤兒 pane——父層 tab／workspace 不存在
/// ——因此不會出現在任何 tab 底下，design D7）；依 `pane_seq`（進入狀態庫的先後）遞增
/// 排序。
fn project_panes(
    state: &RuntimeState,
    workspace_id: &WorkspaceId,
    tab_id: &TabId,
) -> Vec<ProjectedPane> {
    let mut panes: Vec<&Pane> = state
        .panes
        .values()
        .filter(|pane| pane.workspace_id == *workspace_id && pane.tab_id == *tab_id)
        .collect();
    panes.sort_by_key(|pane| state.pane_seq.get(&pane.id).copied().unwrap_or(u64::MAX));

    panes
        .into_iter()
        .map(|pane| ProjectedPane {
            id: pane.id.clone(),
            agent: pane.agent.clone(),
            agent_status: pane.agent_status,
            title: pane.title.clone(),
            cwd: pane.cwd.clone(),
            label: pane.label.clone(),
            focused: pane.focused,
            exited: pane.exited,
            updated_at: to_rfc3339(pane.updated_at),
        })
        .collect()
}

/// `SystemTime` → RFC 3339（UTC，秒精度，固定 `YYYY-MM-DDTHH:MM:SSZ`）。
fn to_rfc3339(time: SystemTime) -> String {
    DateTime::<Utc>::from(time)
        .format("%Y-%m-%dT%H:%M:%SZ")
        .to_string()
}
