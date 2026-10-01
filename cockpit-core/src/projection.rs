//! `RuntimeStore`＋`DomainState` → `ProjectedState` 的純函數投影（spec `state-projection`；設計
//! 文件 §6.4、design D7、D9、D12；change `pipeline-projection` design D2、D3）。綁定解析與
//! StageStatus 推導在投影時即時計算、不快取。不修改 `store`／`domain`；`version` 由呼叫端決定，本模組只原樣
//! 填入（1.6 的投影任務負責決定何時遞增）。所有時間欄位以 `chrono` 轉成 RFC 3339
//! （UTC、秒精度），格式固定 `YYYY-MM-DDTHH:MM:SSZ`。

use std::time::SystemTime;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::domain::binding::{BindingResolution, BindingSource, Override, resolve_binding};
use crate::domain::config::ProjectDef;
use crate::domain::ids::{ProjectId, TaskId, WorkstreamId};
use crate::domain::progress::{Mark, TaskProgress};
use crate::domain::state::DomainState;
use crate::domain::status::{StageStatus, derive_status};
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
    /// 所有 Project，依設定檔順序；沒有 Project 時為空陣列（spec 「Project 投影」）。
    pub projects: Vec<ProjectedProject>,
    /// 所有 runtime 的最近事件合併結果，最新在前，最多
    /// [`RECENT_EVENTS_CAPACITY`] 筆。
    pub recent_events: Vec<ProjectedEvent>,
}

impl ProjectedState {
    /// 比較兩份投影的實質內容，忽略 `version` 與 `generated_at`（design D9）：1.6 用它
    /// 決定要不要遞增 `version`、要不要廣播。
    pub fn content_eq(&self, other: &ProjectedState) -> bool {
        self.runtimes == other.runtimes
            && self.projects == other.projects
            && self.recent_events == other.recent_events
    }
}

/// 一個 Project 的投影（spec `state-projection` 「Project 投影」）。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectedProject {
    /// project id。
    pub id: ProjectId,
    /// 顯示名稱。
    pub name: String,
    /// Stage 的線性順序（設定順序）。
    pub stages: Vec<String>,
    /// 載入狀態檔時產生的 warning；沒有就是空陣列。
    pub warnings: Vec<String>,
    /// 這個 Project 的 workstream，依設定順序。
    pub workstreams: Vec<ProjectedWorkstream>,
    /// 這個 Project 的 task，依設定順序。
    pub tasks: Vec<ProjectedTask>,
}

/// 一條 Workstream 的投影。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectedWorkstream {
    /// workstream id。
    pub id: WorkstreamId,
    /// 顯示名稱。
    pub name: String,
    /// 這次投影即時解析出的綁定。
    pub binding: ProjectedBinding,
    /// 目前 task 的 id；沒有就序列化為 `null`，不省略（progress-model task 2.3）。
    #[serde(default)]
    pub active_task: Option<TaskId>,
    /// 綁定的 agent 正在工作或被擋住，但這條 workstream 沒有目前 task（agent 還沒宣告在做哪個
    /// task）。只在 `binding` 為 `bound`、`agent_status` 為 `working`／`blocked`、且 `active_task`
    /// 為 `None` 時為 `true`。
    #[serde(default)]
    pub activity_undeclared: bool,
}

/// 綁定解析結果的投影：序列化為 `{"state": "none" | "runtime_disconnected" | "bound" |
/// "unbound" | "ambiguous", ...}`（spec 「Project 投影」）。`bound` 另補上綁定 pane 目前的
/// `agent`／`agent_status`。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum ProjectedBinding {
    /// 沒有 binding，也沒有覆蓋。
    None,
    /// 要使用的 runtime 目前不是 `connected`。
    RuntimeDisconnected {
        /// 未連線的 runtime id。
        runtime: RuntimeId,
        /// 這個斷線的綁定來自自動解析（`auto`）或畫面覆蓋（`override`），與 `bound` 的
        /// `source` 同義（ui-fixes task 2.1）。
        source: BindingSource,
    },
    /// 恰好解析到一個 pane。
    Bound {
        /// 綁定所在的 runtime id。
        runtime: RuntimeId,
        /// 綁定的 pane id。
        pane_id: PaneId,
        /// 自動解析（`auto`）或畫面覆蓋（`override`）。
        source: BindingSource,
        /// 綁定 pane 目前的 agent 名稱；`None` 序列化為 `null`，不省略。
        agent: Option<String>,
        /// 綁定 pane 目前的 agent 狀態。
        agent_status: AgentStatus,
    },
    /// 自動解析的候選為 0 個。
    Unbound {
        /// 嘗試解析的 runtime id。
        runtime: RuntimeId,
    },
    /// 自動解析的候選超過 1 個。
    Ambiguous {
        /// 嘗試解析的 runtime id。
        runtime: RuntimeId,
        /// 候選 pane id，依狀態庫順序。
        candidates: Vec<PaneId>,
    },
}

/// 一個 Task 的投影。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectedTask {
    /// task id。
    pub id: TaskId,
    /// 顯示標題。
    pub title: String,
    /// 所屬 workstream id。
    pub workstream: WorkstreamId,
    /// 目前所在的 Stage。
    pub stage: String,
    /// 目前的人工標記。
    pub mark: Mark,
    /// 這次投影即時推導出的 StageStatus。
    pub status: StageStatus,
    /// 依賴的 task id。
    pub depends_on: Vec<TaskId>,
}

/// 一筆失效的覆蓋（design D3）：覆蓋的 runtime 已 `connected`，但 pane 不存在或已 `exited`。
/// 投影已視同覆蓋不存在；刪除交給接收端（寫入服務）非同步完成。接收端刪除前應確認該
/// workstream 目前的覆蓋仍等於 `override_`——同一筆失效可能在刪除完成前被再送一次，期間也
/// 可能被使用者換成新的覆蓋。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StaleOverride {
    /// 覆蓋所在的 project。
    pub project: ProjectId,
    /// 被覆蓋的 workstream。
    pub workstream: WorkstreamId,
    /// 判定失效當下的覆蓋內容。
    pub override_: Override,
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

/// 由狀態庫與 Domain 狀態純函數產生一份投影：不修改輸入；`version` 原樣填入；`now` 用來產生
/// `generated_at`。需要失效覆蓋清單時用 [`project_with_stale`]。
pub fn project(
    store: &RuntimeStore,
    domain: &DomainState,
    version: u64,
    now: SystemTime,
) -> ProjectedState {
    project_with_stale(store, domain, version, now).0
}

/// 同 [`project`]，另回傳這次解析判定失效的覆蓋（design D3，依 project、workstream 設定順序）；
/// 投影任務把非空清單送給寫入服務。
pub fn project_with_stale(
    store: &RuntimeStore,
    domain: &DomainState,
    version: u64,
    now: SystemTime,
) -> (ProjectedState, Vec<StaleOverride>) {
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

    let mut stale = Vec::new();
    let projects = domain
        .projects
        .iter()
        .map(|def| project_project(store, domain, def, &mut stale))
        .collect();

    let state = ProjectedState {
        version,
        generated_at: to_rfc3339(now),
        runtimes,
        projects,
        recent_events,
    };
    (state, stale)
}

/// 一個 Project：每條 workstream 解析一次綁定（同 workstream 的多個 task 共用），再逐 task
/// 推導 StageStatus；失效覆蓋附加到 `stale`。
fn project_project(
    store: &RuntimeStore,
    domain: &DomainState,
    def: &ProjectDef,
    stale: &mut Vec<StaleOverride>,
) -> ProjectedProject {
    let overrides = domain.overrides.get(&def.id);
    let progress = domain.progress.get(&def.id);
    let progress_of = |task_id: &TaskId| progress.and_then(|map| map.get(task_id));

    // 與 `def.workstreams` 同序：(解析結果, 綁定的 pane)。
    let resolved: Vec<(BindingResolution, Option<&Pane>)> = def
        .workstreams
        .iter()
        .map(|ws| {
            let override_ = overrides.and_then(|map| map.get(&ws.id));
            let (resolution, is_stale) = resolve_binding(ws, override_, store);
            if let (true, Some(over)) = (is_stale, override_) {
                stale.push(StaleOverride {
                    project: def.id.clone(),
                    workstream: ws.id.clone(),
                    override_: over.clone(),
                });
            }
            let pane = bound_pane(store, &resolution);
            (resolution, pane)
        })
        .collect();

    let workstreams = def
        .workstreams
        .iter()
        .zip(&resolved)
        .map(|(ws, (resolution, pane))| {
            let active_task = domain.active_task(&def.id, &ws.id).cloned();
            let busy = pane.is_some_and(|p| {
                matches!(p.agent_status, AgentStatus::Working | AgentStatus::Blocked)
            });
            ProjectedWorkstream {
                id: ws.id.clone(),
                name: ws.name.clone(),
                binding: project_binding(resolution, *pane),
                activity_undeclared: busy && active_task.is_none(),
                active_task,
            }
        })
        .collect();

    let tasks = def
        .tasks
        .iter()
        .map(|task| {
            let current = progress_of(&task.id)
                .cloned()
                .unwrap_or_else(|| TaskProgress::initial(task));
            // 依賴的 task 沒有進度紀錄（設定驗證理論上已擋掉未知 id）視為 `Mark::None`。
            let dependency_marks: Vec<Mark> = task
                .depends_on
                .iter()
                .map(|dep| progress_of(dep).map_or(Mark::None, |p| p.mark))
                .collect();
            // 找不到所屬 workstream（設定驗證理論上已擋掉）視為沒有綁定。
            let (resolution, pane) = def
                .workstreams
                .iter()
                .position(|ws| ws.id == task.workstream)
                .map_or((&BindingResolution::None, None), |i| {
                    (&resolved[i].0, resolved[i].1)
                });
            let status = derive_status(
                current.mark,
                &dependency_marks,
                resolution,
                pane.map(|p| p.agent_status),
                domain.active_task(&def.id, &task.workstream) == Some(&task.id),
            );
            ProjectedTask {
                id: task.id.clone(),
                title: task.title.clone(),
                workstream: task.workstream.clone(),
                stage: current.stage,
                mark: current.mark,
                status,
                depends_on: task.depends_on.clone(),
            }
        })
        .collect();

    ProjectedProject {
        id: def.id.clone(),
        name: def.name.clone(),
        stages: def.stages.clone(),
        warnings: domain.warnings.get(&def.id).cloned().unwrap_or_default(),
        workstreams,
        tasks,
    }
}

/// `Bound` 時從狀態庫取出綁定的 pane；其餘結果為 `None`。
fn bound_pane<'a>(store: &'a RuntimeStore, resolution: &BindingResolution) -> Option<&'a Pane> {
    match resolution {
        BindingResolution::Bound {
            runtime, pane_id, ..
        } => store
            .state(runtime)
            .and_then(|state| state.panes.get(pane_id)),
        _ => None,
    }
}

fn project_binding(resolution: &BindingResolution, pane: Option<&Pane>) -> ProjectedBinding {
    match resolution {
        BindingResolution::None => ProjectedBinding::None,
        BindingResolution::RuntimeDisconnected { runtime, source } => {
            ProjectedBinding::RuntimeDisconnected {
                runtime: runtime.clone(),
                source: *source,
            }
        }
        BindingResolution::Bound {
            runtime,
            pane_id,
            source,
        } => ProjectedBinding::Bound {
            runtime: runtime.clone(),
            pane_id: pane_id.clone(),
            source: *source,
            // `resolve_binding` 只在 pane 存在時回 `Bound`，找不到 pane 屬防禦分支。
            agent: pane.and_then(|p| p.agent.clone()),
            agent_status: pane.map_or(AgentStatus::Unknown, |p| p.agent_status),
        },
        BindingResolution::Unbound { runtime } => ProjectedBinding::Unbound {
            runtime: runtime.clone(),
        },
        BindingResolution::Ambiguous {
            runtime,
            candidates,
        } => ProjectedBinding::Ambiguous {
            runtime: runtime.clone(),
            candidates: candidates.clone(),
        },
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
