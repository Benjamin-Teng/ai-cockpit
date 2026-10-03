//! `RuntimeStore`：純記憶體狀態庫，提供 `replace`（整份替換）與 `apply`（逐筆套用
//! `RuntimeEvent`）。不含鎖、不含 async（design D9）；1.6 會用 `Arc<Mutex<RuntimeStore>>`
//! 包起來給多個消費者共用，1.5 會從 `RuntimeState` 產生投影。

use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt;
use std::time::SystemTime;

use crate::message::Message;
use crate::types::agent_status::AgentStatus;
use crate::types::connection::ConnectionState;
use crate::types::events::{FocusChange, RuntimeEvent};
use crate::types::ids::{PaneId, RuntimeId, TabId, WorkspaceId};
use crate::types::model::{Agent, Focused, Pane, RuntimeSnapshot, Tab, Workspace};

/// 每個 runtime 保留的最近事件筆數上限（spec「最近事件」；design D9）。
pub const RECENT_EVENTS_CAPACITY: usize = 50;

/// 一筆已套用（或套用失敗）的事件紀錄，供 1.5 的投影組出 `recent_events`。
///
/// 刻意不放進 `RuntimeState`：放進去會讓 1.4「套用前後整份 `RuntimeState` 相等」的
/// 比較失敗（`recent` 每次都會多一筆、絕不可能與套用前相等）。改放在 `RuntimeStore`
/// 自己的另一個 map，`RuntimeState` 的 `PartialEq` 完全不受影響。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecentEvent {
    /// 記錄當下的時間（呼叫端傳給 `apply` 的 `now`）。
    pub at: SystemTime,
    /// 事件種類：見 [`event_kind`]（成功套用的事件多為 `RuntimeEvent` 變體名的
    /// snake_case，`AgentStatusChanged` 例外固定 `"pane.agent_status_changed"`、
    /// `Noted` 用它自己帶的字串）；套用失敗（回傳 `Err(Drift)`）一律是 [`DRIFT_KIND`]
    /// （不論觸發的是哪個變體）。
    pub kind: String,
    /// 事件影響到的 workspace；只有 workspace 類事件（與 `TabsReplaced`、部分
    /// `FocusChanged`）會填，其餘為 `None`（見 [`describe_event`]）。
    pub workspace_id: Option<WorkspaceId>,
    /// 事件影響到的 tab；只有 tab 類事件（與部分 `FocusChanged`）會填，其餘為 `None`。
    pub tab_id: Option<TabId>,
    /// 事件影響到的 pane；依事件種類而定，可能為 `None`。
    pub pane_id: Option<PaneId>,
    /// 額外說明文字；依事件種類而定，預設空字串。
    pub detail: String,
}

/// `describe_event` 的回傳值：套用前先從事件本身算出這筆事件成功時要記進 `recent`
/// 的內容，套用成功時原樣採用。
struct EventDescription {
    kind: String,
    workspace_id: Option<WorkspaceId>,
    tab_id: Option<TabId>,
    pane_id: Option<PaneId>,
    detail: String,
}

/// `AgentStatus` 的小寫字串表示，與它的 serde `rename_all = "lowercase"` 一致；供
/// `recent_events` 的 `detail` 用，不為此另外拉 `serde_json` 依賴。
fn agent_status_str(status: AgentStatus) -> &'static str {
    match status {
        AgentStatus::Idle => "idle",
        AgentStatus::Working => "working",
        AgentStatus::Blocked => "blocked",
        AgentStatus::Done => "done",
        AgentStatus::Unknown => "unknown",
    }
}

/// 套用失敗（回傳 `Err(Drift)`）時，`recent` 一律記這個 `kind`，不論觸發失敗的是哪個
/// `RuntimeEvent` 變體。
pub(crate) const DRIFT_KIND: &str = "drift";

/// 把一個 `RuntimeEvent` 對應到 `recent_events` 要用的 `kind` 字串；集中定義在這一處
/// （review finding 1：先前 `AgentStatusChanged` 與其他變體的字串各自散在
/// `describe_event` 裡分別組出來，容易漏改）。
///
/// 除了 `AgentStatusChanged` 之外，其餘變體都是變體名的 snake_case。
/// **`AgentStatusChanged` 固定輸出 `"pane.agent_status_changed"`**（帶 `pane.`
/// 前綴）：對齊設計文件 §6.4 與 Live-state Scenario B 驗收——這是目前唯一帶前綴的
/// kind，其餘 pane 類事件（`pane_upserted` 等）維持既有底線命名，不擴大修正範圍。
/// `Noted` 用它自己帶的字串；`Drift` 固定 [`DRIFT_KIND`]。
fn event_kind(event: &RuntimeEvent) -> &str {
    match event {
        RuntimeEvent::WorkspaceUpserted(_) => "workspace_upserted",
        RuntimeEvent::WorkspacesReplaced(_) => "workspaces_replaced",
        RuntimeEvent::WorkspaceRemoved(_) => "workspace_removed",
        RuntimeEvent::WorkspaceRelabeled { .. } => "workspace_relabeled",
        RuntimeEvent::TabUpserted(_) => "tab_upserted",
        RuntimeEvent::TabsReplaced { .. } => "tabs_replaced",
        RuntimeEvent::TabRemoved(_) => "tab_removed",
        RuntimeEvent::TabRelabeled { .. } => "tab_relabeled",
        RuntimeEvent::PaneUpserted(_) => "pane_upserted",
        RuntimeEvent::PaneMoved { .. } => "pane_moved",
        RuntimeEvent::PaneRemoved(_) => "pane_removed",
        RuntimeEvent::PaneExited(_) => "pane_exited",
        RuntimeEvent::AgentDetected { .. } => "agent_detected",
        RuntimeEvent::AgentStatusChanged { .. } => "pane.agent_status_changed",
        RuntimeEvent::FocusChanged(_) => "focus_changed",
        RuntimeEvent::Drift { .. } => DRIFT_KIND,
        RuntimeEvent::Noted { kind } => kind.as_str(),
    }
}

/// 在套用事件「之前」，依事件內容算出這筆事件成功時要記進 `recent` 的內容（design
/// D9；spec「最近事件」）。
///
/// **主體 id 每筆事件只填一個**（review finding 2）：pane 類事件（`PaneUpserted`／
/// `PaneMoved`／`PaneRemoved`／`PaneExited`／`AgentDetected`／`AgentStatusChanged`）只填
/// `pane_id`；tab 類事件（`TabUpserted`／`TabRemoved`／`TabRelabeled`）只填 `tab_id`，
/// `TabsReplaced` 沒有單一 tab 主體、改填 `workspace_id`；workspace 類事件
/// （`WorkspaceUpserted`／`WorkspaceRemoved`／`WorkspaceRelabeled`）只填
/// `workspace_id`；`FocusChanged` 只填它帶的最深一層（pane > tab > workspace）；
/// `Noted`／`Drift`／`WorkspacesReplaced` 不填任何 id。
///
/// 套用失敗（`apply` 回傳 `Err(Drift)`）時，呼叫端不會使用這裡算出的內容，一律改記
/// `kind: DRIFT_KIND`、`detail` 為 `Drift::reason`——所以這裡不用處理「事件其實不合法」
/// 的情況，只管在「假設它會成功」的前提下描述它。因為不需要查詢套用前的狀態（不再
/// 「補」父層 id），這是純函數，不是 `RuntimeState` 的方法。
fn describe_event(event: &RuntimeEvent) -> EventDescription {
    let kind = event_kind(event).to_string();
    match event {
        RuntimeEvent::WorkspaceUpserted(workspace) => EventDescription {
            kind,
            workspace_id: Some(workspace.id.clone()),
            tab_id: None,
            pane_id: None,
            detail: String::new(),
        },
        RuntimeEvent::WorkspacesReplaced(_) => EventDescription {
            kind,
            workspace_id: None,
            tab_id: None,
            pane_id: None,
            detail: String::new(),
        },
        RuntimeEvent::WorkspaceRemoved(id) => EventDescription {
            kind,
            workspace_id: Some(id.clone()),
            tab_id: None,
            pane_id: None,
            detail: String::new(),
        },
        RuntimeEvent::WorkspaceRelabeled { id, label } => EventDescription {
            kind,
            workspace_id: Some(id.clone()),
            tab_id: None,
            pane_id: None,
            detail: label.clone(),
        },
        RuntimeEvent::TabUpserted(tab) => EventDescription {
            kind,
            workspace_id: None,
            tab_id: Some(tab.id.clone()),
            pane_id: None,
            detail: String::new(),
        },
        RuntimeEvent::TabsReplaced { workspace_id, .. } => EventDescription {
            kind,
            workspace_id: Some(workspace_id.clone()),
            tab_id: None,
            pane_id: None,
            detail: String::new(),
        },
        RuntimeEvent::TabRemoved(id) => EventDescription {
            kind,
            workspace_id: None,
            tab_id: Some(id.clone()),
            pane_id: None,
            detail: String::new(),
        },
        RuntimeEvent::TabRelabeled { id, label } => EventDescription {
            kind,
            workspace_id: None,
            tab_id: Some(id.clone()),
            pane_id: None,
            detail: label.clone(),
        },
        RuntimeEvent::PaneUpserted(pane) => EventDescription {
            kind,
            workspace_id: None,
            tab_id: None,
            pane_id: Some(pane.id.clone()),
            detail: String::new(),
        },
        RuntimeEvent::PaneMoved { previous, pane } => EventDescription {
            kind,
            workspace_id: None,
            tab_id: None,
            pane_id: Some(pane.id.clone()),
            detail: format!("from {previous}"),
        },
        RuntimeEvent::PaneRemoved(id) => EventDescription {
            kind,
            workspace_id: None,
            tab_id: None,
            pane_id: Some(id.clone()),
            detail: String::new(),
        },
        RuntimeEvent::PaneExited(id) => EventDescription {
            kind,
            workspace_id: None,
            tab_id: None,
            pane_id: Some(id.clone()),
            detail: String::new(),
        },
        RuntimeEvent::AgentDetected { pane_id, agent } => EventDescription {
            kind,
            workspace_id: None,
            tab_id: None,
            pane_id: Some(pane_id.clone()),
            detail: agent.clone().unwrap_or_else(|| "released".to_string()),
        },
        RuntimeEvent::AgentStatusChanged {
            pane_id, status, ..
        } => EventDescription {
            kind,
            workspace_id: None,
            tab_id: None,
            pane_id: Some(pane_id.clone()),
            detail: agent_status_str(*status).to_string(),
        },
        RuntimeEvent::FocusChanged(change) => {
            // 只填最深一層：pane > tab > workspace（review finding 2）。
            let (workspace_id, tab_id, pane_id) = if change.pane_id.is_some() {
                (None, None, change.pane_id.clone())
            } else if change.tab_id.is_some() {
                (None, change.tab_id.clone(), None)
            } else {
                (change.workspace_id.clone(), None, None)
            };
            EventDescription {
                kind,
                workspace_id,
                tab_id,
                pane_id,
                detail: String::new(),
            }
        }
        RuntimeEvent::Drift { reason } => EventDescription {
            kind,
            workspace_id: None,
            tab_id: None,
            pane_id: None,
            detail: reason.clone(),
        },
        RuntimeEvent::Noted { .. } => EventDescription {
            kind,
            workspace_id: None,
            tab_id: None,
            pane_id: None,
            detail: String::new(),
        },
    }
}

/// 套用事件失敗、狀態庫與 runtime 之間認知不一致時回傳的錯誤。
///
/// 不是 panic：呼叫端（1.4 之後的驅動器）決定是否要重新拉一份 snapshot 校正。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Drift {
    /// 觸發 drift 的原因，供除錯與記錄；最近事件會把它放進 `detail`，前端依 `detail_msg` 翻譯。
    /// 必須由 [`Message`]（`Drift*NotFound`、`DriftRuntimeNotRegistered`、`EventPayloadUnparsable` 等）的
    /// `text()` 產生：新增原因種類時先加 `Message` 變體並補 `msg.<code>` 字典鍵（http 對帳測試會擋缺鍵）。
    pub reason: String,
}

impl fmt::Display for Drift {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.reason)
    }
}

/// 單一 runtime 的完整狀態：扁平 map 加連線狀態與焦點（design D9；設計文件 §6.3）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimeState {
    /// runtime 種類（例如 `"herdr"`）。
    pub kind: String,
    /// 連線端點，供顯示與除錯。
    pub endpoint: String,
    /// 最近一次 snapshot 回報的 server 版本；尚未有 snapshot 時為 `None`。
    pub server_version: Option<String>,
    /// 最近一次 snapshot 回報的協定版本；尚未有 snapshot 時為 `None`。
    pub protocol: Option<u32>,
    /// 這個 runtime 目前的所有 workspace。
    pub workspaces: HashMap<WorkspaceId, Workspace>,
    /// 這個 runtime 目前的所有 tab。
    pub tabs: HashMap<TabId, Tab>,
    /// 這個 runtime 目前的所有 pane。
    pub panes: HashMap<PaneId, Pane>,
    /// 每個 pane 進入狀態庫的序號（design D7），投影排序用；pane id 字串不適合當排序鍵。
    pub pane_seq: HashMap<PaneId, u64>,
    /// 下一個要分配的 pane 序號。
    pub next_seq: u64,
    /// 目前偵測到的 agent，每個 pane 至多一筆，以 pane id 為 key。
    pub agents: HashMap<PaneId, Agent>,
    /// 目前使用者焦點在各層級的位置。
    pub focused: Focused,
    /// 與這個 runtime 之間的連線狀態；剛登記時為 `Connecting`。
    pub connection: ConnectionState,
}

impl RuntimeState {
    fn new(kind: String, endpoint: String) -> Self {
        Self {
            kind,
            endpoint,
            server_version: None,
            protocol: None,
            workspaces: HashMap::new(),
            tabs: HashMap::new(),
            panes: HashMap::new(),
            pane_seq: HashMap::new(),
            next_seq: 0,
            agents: HashMap::new(),
            focused: Focused {
                workspace_id: None,
                tab_id: None,
                pane_id: None,
            },
            connection: ConnectionState::Connecting,
        }
    }

    /// 分配下一個 pane 序號並遞增計數器。
    fn next_pane_seq(&mut self) -> u64 {
        let seq = self.next_seq;
        self.next_seq += 1;
        seq
    }

    /// 新增或覆蓋一個 pane：同 id 已有序號就沿用，否則分配新序號。
    fn insert_pane(&mut self, pane: Pane) {
        let seq = match self.pane_seq.get(&pane.id) {
            Some(existing) => *existing,
            None => self.next_pane_seq(),
        };
        self.pane_seq.insert(pane.id.clone(), seq);
        self.panes.insert(pane.id.clone(), pane);
    }

    /// 移除一個 pane 連同它的序號與 agent 紀錄。
    fn remove_pane_and_agent(&mut self, pane_id: &PaneId) {
        self.panes.remove(pane_id);
        self.pane_seq.remove(pane_id);
        self.agents.remove(pane_id);
    }

    /// 移除一個 tab，連帶移除其下所有 pane 與它們的 agent 紀錄（design D5）。
    ///
    /// 直接依 `pane.tab_id`／`agent.tab_id` 欄位過濾，不透過 tab→pane 的 parent map
    /// 走訪：即使狀態庫裡有 pane 缺對應 tab、或 agent 缺對應 pane（`replace` 不驗證
    /// 關聯完整性，理論上不該發生但要防禦），也能連帶清乾淨，不留 orphan（review
    /// finding 3）。
    fn remove_tab_cascade(&mut self, tab_id: &TabId) {
        self.tabs.remove(tab_id);
        self.panes.retain(|_, pane| pane.tab_id != *tab_id);
        let panes = &self.panes;
        self.pane_seq.retain(|id, _| panes.contains_key(id));
        self.agents.retain(|_, agent| agent.tab_id != *tab_id);
    }

    /// 移除一個 workspace，連帶移除其下所有 tab、pane、agent 紀錄（design D5）。
    ///
    /// 同 [`RuntimeState::remove_tab_cascade`]：直接依 `tab.workspace_id`／
    /// `pane.workspace_id`／`agent.workspace_id` 欄位過濾，不透過 workspace→tab→pane
    /// 的 parent map 走訪。
    fn remove_workspace_cascade(&mut self, workspace_id: &WorkspaceId) {
        self.workspaces.remove(workspace_id);
        self.tabs.retain(|_, tab| tab.workspace_id != *workspace_id);
        self.panes
            .retain(|_, pane| pane.workspace_id != *workspace_id);
        let panes = &self.panes;
        self.pane_seq.retain(|id, _| panes.contains_key(id));
        self.agents
            .retain(|_, agent| agent.workspace_id != *workspace_id);
    }

    /// 以一份 `RuntimeSnapshot` 整份取代 workspaces、tabs、panes、agents、focused、
    /// server 版本與 protocol；不動 `connection`、`kind`、`endpoint`。
    fn replace(&mut self, snapshot: RuntimeSnapshot) {
        self.workspaces.clear();
        self.tabs.clear();
        self.panes.clear();
        self.pane_seq.clear();
        self.agents.clear();
        self.next_seq = 0;

        for workspace in snapshot.workspaces {
            self.workspaces.insert(workspace.id.clone(), workspace);
        }
        for tab in snapshot.tabs {
            self.tabs.insert(tab.id.clone(), tab);
        }
        for pane in snapshot.panes {
            // 照 snapshot 陣列順序給號（design D7）。
            self.insert_pane(pane);
        }
        for agent in snapshot.agents {
            self.agents.insert(agent.pane_id.clone(), agent);
        }
        self.focused = snapshot.focused;
        self.server_version = Some(snapshot.server_version);
        self.protocol = Some(snapshot.protocol);
    }

    /// 逐筆套用一個 `RuntimeEvent`；每個變體都有分支，沒有 `_ =>` 萬用分支。
    fn apply(&mut self, event: RuntimeEvent, now: SystemTime) -> Result<(), Drift> {
        match event {
            RuntimeEvent::WorkspaceUpserted(workspace) => {
                self.workspaces.insert(workspace.id.clone(), workspace);
                Ok(())
            }
            RuntimeEvent::WorkspacesReplaced(workspaces) => {
                // keep set 來自「新的」workspace 清單，不是舊 map 減新 map算出來的
                // removed 清單：這樣即使 pane／agent 指向的 workspace 在舊 map 裡本來
                // 就不存在（父層原本就缺失的 orphan），只要它不在新清單裡，一樣會被
                // 這裡的 retain 濾掉，不會因為「沒出現在 removed 裡」而殘留
                // （review finding 3 未修完的那一半）。
                let keep: HashSet<WorkspaceId> = workspaces.iter().map(|w| w.id.clone()).collect();
                self.workspaces = workspaces
                    .into_iter()
                    .map(|workspace| (workspace.id.clone(), workspace))
                    .collect();
                self.tabs.retain(|_, tab| keep.contains(&tab.workspace_id));
                self.panes
                    .retain(|_, pane| keep.contains(&pane.workspace_id));
                let panes = &self.panes;
                self.pane_seq.retain(|id, _| panes.contains_key(id));
                self.agents
                    .retain(|_, agent| keep.contains(&agent.workspace_id));
                Ok(())
            }
            RuntimeEvent::WorkspaceRemoved(id) => {
                if !self.workspaces.contains_key(&id) {
                    return Err(Drift {
                        reason: Message::DriftWorkspaceNotFound { id: id.to_string() }.text(),
                    });
                }
                self.remove_workspace_cascade(&id);
                Ok(())
            }
            RuntimeEvent::WorkspaceRelabeled { id, label } => {
                let workspace = self.workspaces.get_mut(&id).ok_or_else(|| Drift {
                    reason: Message::DriftWorkspaceNotFound { id: id.to_string() }.text(),
                })?;
                workspace.label = if label.is_empty() { None } else { Some(label) };
                Ok(())
            }
            RuntimeEvent::TabUpserted(tab) => {
                let workspace_id = &tab.workspace_id;
                if !self.workspaces.contains_key(workspace_id) {
                    return Err(Drift {
                        reason: Message::DriftWorkspaceNotFound {
                            id: workspace_id.to_string(),
                        }
                        .text(),
                    });
                }
                self.tabs.insert(tab.id.clone(), tab);
                Ok(())
            }
            RuntimeEvent::TabsReplaced { workspace_id, tabs } => {
                if !self.workspaces.contains_key(&workspace_id) {
                    return Err(Drift {
                        reason: Message::DriftWorkspaceNotFound {
                            id: workspace_id.to_string(),
                        }
                        .text(),
                    });
                }
                // 同 `WorkspacesReplaced`：keep set 來自「新的」tab 清單，不是舊 map
                // 減新 map 算出來的 removed 清單，才能濾掉「tab_id 在 tabs map 裡本來
                // 就不存在」的 orphan pane／agent（review finding 3）。屬於其他
                // workspace 的物件不受影響。
                let keep: HashSet<TabId> = tabs.iter().map(|t| t.id.clone()).collect();
                self.tabs
                    .retain(|id, tab| tab.workspace_id != workspace_id || keep.contains(id));
                for tab in tabs {
                    self.tabs.insert(tab.id.clone(), tab);
                }
                self.panes.retain(|_, pane| {
                    pane.workspace_id != workspace_id || keep.contains(&pane.tab_id)
                });
                let panes = &self.panes;
                self.pane_seq.retain(|id, _| panes.contains_key(id));
                self.agents.retain(|_, agent| {
                    agent.workspace_id != workspace_id || keep.contains(&agent.tab_id)
                });
                Ok(())
            }
            RuntimeEvent::TabRemoved(id) => {
                if !self.tabs.contains_key(&id) {
                    return Err(Drift {
                        reason: Message::DriftTabNotFound { id: id.to_string() }.text(),
                    });
                }
                self.remove_tab_cascade(&id);
                Ok(())
            }
            RuntimeEvent::TabRelabeled { id, label: _ } => {
                // `Tab`（task 1.2 型別）目前沒有 label 欄位，只驗存在性、不改內容。
                if !self.tabs.contains_key(&id) {
                    return Err(Drift {
                        reason: Message::DriftTabNotFound { id: id.to_string() }.text(),
                    });
                }
                Ok(())
            }
            RuntimeEvent::PaneUpserted(mut pane) => {
                let workspace_id = &pane.workspace_id;
                if !self.workspaces.contains_key(workspace_id) {
                    return Err(Drift {
                        reason: Message::DriftWorkspaceNotFound {
                            id: workspace_id.to_string(),
                        }
                        .text(),
                    });
                }
                let tab_id = &pane.tab_id;
                if !self.tabs.contains_key(tab_id) {
                    return Err(Drift {
                        reason: Message::DriftTabNotFound {
                            id: tab_id.to_string(),
                        }
                        .text(),
                    });
                }
                // pane 被這筆事件改動，updated_at 一律覆蓋成 apply 傳入的 now，
                // 不用事件內帶來的值（review finding 1）。
                pane.updated_at = now;
                self.insert_pane(pane);
                Ok(())
            }
            RuntimeEvent::PaneMoved { previous, mut pane } => {
                if !self.panes.contains_key(&previous) {
                    return Err(Drift {
                        reason: Message::DriftPaneNotFound {
                            id: previous.to_string(),
                        }
                        .text(),
                    });
                }
                let workspace_id = &pane.workspace_id;
                if !self.workspaces.contains_key(workspace_id) {
                    return Err(Drift {
                        reason: Message::DriftWorkspaceNotFound {
                            id: workspace_id.to_string(),
                        }
                        .text(),
                    });
                }
                let tab_id = &pane.tab_id;
                if !self.tabs.contains_key(tab_id) {
                    return Err(Drift {
                        reason: Message::DriftTabNotFound {
                            id: tab_id.to_string(),
                        }
                        .text(),
                    });
                }

                // 同上：搬移也是一種 pane 改動，updated_at 覆蓋成 now（review finding 1）。
                pane.updated_at = now;
                self.panes.remove(&previous);
                self.pane_seq.remove(&previous);
                let new_id = pane.id.clone();
                let seq = self.next_pane_seq();
                self.pane_seq.insert(new_id.clone(), seq);
                if let Some(mut moved_agent) = self.agents.remove(&previous) {
                    moved_agent.pane_id = new_id.clone();
                    moved_agent.workspace_id = pane.workspace_id.clone();
                    moved_agent.tab_id = pane.tab_id.clone();
                    self.agents.insert(new_id.clone(), moved_agent);
                }
                self.panes.insert(new_id, pane);
                Ok(())
            }
            RuntimeEvent::PaneRemoved(id) => {
                if !self.panes.contains_key(&id) {
                    return Err(Drift {
                        reason: Message::DriftPaneNotFound { id: id.to_string() }.text(),
                    });
                }
                self.remove_pane_and_agent(&id);
                Ok(())
            }
            RuntimeEvent::PaneExited(id) => {
                let pane = self.panes.get_mut(&id).ok_or_else(|| Drift {
                    reason: Message::DriftPaneNotFound { id: id.to_string() }.text(),
                })?;
                pane.exited = true;
                pane.updated_at = now;
                Ok(())
            }
            RuntimeEvent::AgentDetected { pane_id, agent } => {
                let Some(pane) = self.panes.get_mut(&pane_id) else {
                    return Err(Drift {
                        reason: Message::DriftPaneNotFound {
                            id: pane_id.to_string(),
                        }
                        .text(),
                    });
                };
                match agent {
                    Some(name) => {
                        pane.agent = Some(name.clone());
                        pane.updated_at = now;
                        let agent_status = pane.agent_status;
                        let workspace_id = pane.workspace_id.clone();
                        let tab_id = pane.tab_id.clone();
                        self.agents.insert(
                            pane_id.clone(),
                            Agent {
                                agent: name,
                                pane_id,
                                workspace_id,
                                tab_id,
                                agent_status,
                            },
                        );
                    }
                    None => {
                        pane.agent = None;
                        pane.updated_at = now;
                        self.agents.remove(&pane_id);
                    }
                }
                Ok(())
            }
            RuntimeEvent::AgentStatusChanged {
                pane_id,
                status,
                title,
                agent,
            } => {
                let Some(pane) = self.panes.get_mut(&pane_id) else {
                    return Err(Drift {
                        reason: Message::DriftPaneNotFound {
                            id: pane_id.to_string(),
                        }
                        .text(),
                    });
                };
                pane.agent_status = status;
                if let Some(title) = title {
                    pane.title = Some(title);
                }
                if let Some(agent_name) = &agent {
                    pane.agent = Some(agent_name.clone());
                }
                pane.updated_at = now;
                let workspace_id = pane.workspace_id.clone();
                let tab_id = pane.tab_id.clone();
                // `pane.agent` 這時已經依事件更新過（事件有給就覆蓋、沒給就維持原值），
                // 所以拿它當「紀錄不存在時要用哪個 agent 名稱」的唯一依據，等同
                // 「事件的 agent，否則 pane 目前的 agent」（review finding 2）。
                let fallback_agent_name = pane.agent.clone();

                if let Some(record) = self.agents.get_mut(&pane_id) {
                    record.agent_status = status;
                    if let Some(agent_name) = agent {
                        record.agent = agent_name;
                    }
                } else if let Some(agent_name) = fallback_agent_name {
                    // 紀錄不存在，但 pane 本來就有 agent 名稱（可能來自事件、也可能是
                    // pane 上原有的）：補建一筆紀錄，不讓 pane 有 agent、agents map
                    // 卻查不到的不一致狀態。兩者都沒有名稱時，沒有紀錄可建，只更新
                    // pane（這是唯一「無紀錄可建」的情況）。
                    self.agents.insert(
                        pane_id.clone(),
                        Agent {
                            agent: agent_name,
                            pane_id,
                            workspace_id,
                            tab_id,
                            agent_status: status,
                        },
                    );
                }
                Ok(())
            }
            RuntimeEvent::FocusChanged(change) => self.apply_focus_changed(change, now),
            RuntimeEvent::Drift { reason } => Err(Drift { reason }),
            RuntimeEvent::Noted { .. } => Ok(()),
        }
    }

    /// `FocusChanged`：先驗證事件帶的每個層級都存在（任一不存在就整個事件回 Drift、
    /// 不動任何狀態），全部存在才套用，讓同層只有一個物件 `focused` 為 true。
    ///
    /// `Pane.updated_at` 只在該 pane 的 `focused` 值**實際改變**時更新為 `now`（舊焦點
    /// pane 從 true 變 false、新焦點 pane 從 false 變 true）；沒被這次事件動到的 pane
    /// 不更新 `updated_at`（review finding 1）。`Workspace`／`Tab` 沒有 `updated_at`
    /// 欄位，不受影響。
    fn apply_focus_changed(&mut self, change: FocusChange, now: SystemTime) -> Result<(), Drift> {
        if let Some(id) = &change.workspace_id
            && !self.workspaces.contains_key(id)
        {
            return Err(Drift {
                reason: Message::DriftWorkspaceNotFound { id: id.to_string() }.text(),
            });
        }
        if let Some(id) = &change.tab_id
            && !self.tabs.contains_key(id)
        {
            return Err(Drift {
                reason: Message::DriftTabNotFound { id: id.to_string() }.text(),
            });
        }
        if let Some(id) = &change.pane_id
            && !self.panes.contains_key(id)
        {
            return Err(Drift {
                reason: Message::DriftPaneNotFound { id: id.to_string() }.text(),
            });
        }

        if let Some(id) = change.workspace_id {
            for workspace in self.workspaces.values_mut() {
                workspace.focused = workspace.id == id;
            }
            self.focused.workspace_id = Some(id);
        }
        if let Some(id) = change.tab_id {
            for tab in self.tabs.values_mut() {
                tab.focused = tab.id == id;
            }
            self.focused.tab_id = Some(id);
        }
        if let Some(id) = change.pane_id {
            for pane in self.panes.values_mut() {
                let should_focus = pane.id == id;
                if pane.focused != should_focus {
                    pane.focused = should_focus;
                    pane.updated_at = now;
                }
            }
            self.focused.pane_id = Some(id);
        }
        Ok(())
    }
}

/// 所有 runtime 的狀態庫：純記憶體結構，不含鎖、不含 async（design D9）。
#[derive(Debug, Default)]
pub struct RuntimeStore {
    /// 登記順序，供 [`RuntimeStore::runtime_ids`] 與之後的投影排序用。
    order: Vec<RuntimeId>,
    states: HashMap<RuntimeId, RuntimeState>,
    /// 每個 runtime 最近 [`RECENT_EVENTS_CAPACITY`] 筆事件紀錄；刻意不放在
    /// `RuntimeState` 內（見 [`RecentEvent`] 的說明）。`replace` 不清也不寫這裡。
    recent: HashMap<RuntimeId, VecDeque<RecentEvent>>,
}

impl RuntimeStore {
    /// 建立一個空的狀態庫。
    pub fn new() -> Self {
        Self::default()
    }

    /// 登記一個 runtime；重複登記只更新 `kind`／`endpoint`，不清除既有狀態、不影響
    /// 登記順序。剛登記的 runtime 連線狀態為 `Connecting`。
    pub fn register(&mut self, id: RuntimeId, kind: String, endpoint: String) {
        if let Some(state) = self.states.get_mut(&id) {
            state.kind = kind;
            state.endpoint = endpoint;
            return;
        }
        self.order.push(id.clone());
        self.states.insert(id, RuntimeState::new(kind, endpoint));
    }

    /// 依登記順序回傳所有 runtime id。
    pub fn runtime_ids(&self) -> Vec<RuntimeId> {
        self.order.clone()
    }

    /// 讀取某個 runtime 目前的狀態；未登記回傳 `None`。
    pub fn state(&self, id: &RuntimeId) -> Option<&RuntimeState> {
        self.states.get(id)
    }

    /// 以一份 `RuntimeSnapshot` 整份取代該 runtime 的狀態；對未登記的 runtime 回傳
    /// `Err(Drift)`，不 panic、不隱式登記。
    pub fn replace(&mut self, id: &RuntimeId, snapshot: RuntimeSnapshot) -> Result<(), Drift> {
        let state = self.states.get_mut(id).ok_or_else(|| Drift {
            reason: Message::DriftRuntimeNotRegistered { id: id.to_string() }.text(),
        })?;
        state.replace(snapshot);
        Ok(())
    }

    /// 逐筆套用一個 `RuntimeEvent`；對未登記的 runtime 回傳 `Err(Drift)`，不 panic、
    /// 不隱式登記。
    ///
    /// 成功套用（含 `Noted`）記一筆最近事件；回傳 `Err(Drift)` 時（不論是哪個變體
    /// 觸發的）也記一筆，`kind` 固定 [`DRIFT_KIND`]、`detail` 為 `reason`，讓畫面看得到
    /// 重拿快照的原因（design D9）。
    pub fn apply(
        &mut self,
        id: &RuntimeId,
        event: RuntimeEvent,
        now: SystemTime,
    ) -> Result<(), Drift> {
        let state = self.states.get_mut(id).ok_or_else(|| Drift {
            reason: Message::DriftRuntimeNotRegistered { id: id.to_string() }.text(),
        })?;
        let description = describe_event(&event);
        let result = state.apply(event, now);
        let recorded = match &result {
            Ok(()) => RecentEvent {
                at: now,
                kind: description.kind,
                workspace_id: description.workspace_id,
                tab_id: description.tab_id,
                pane_id: description.pane_id,
                detail: description.detail,
            },
            Err(drift) => RecentEvent {
                at: now,
                kind: DRIFT_KIND.to_string(),
                workspace_id: None,
                tab_id: None,
                pane_id: None,
                detail: drift.reason.clone(),
            },
        };
        self.push_recent(id, recorded);
        result
    }

    /// 把一筆紀錄追加到某個 runtime 的 ring buffer，超過
    /// [`RECENT_EVENTS_CAPACITY`] 就丟最舊的一筆。
    fn push_recent(&mut self, id: &RuntimeId, recorded: RecentEvent) {
        let queue = self.recent.entry(id.clone()).or_default();
        queue.push_back(recorded);
        while queue.len() > RECENT_EVENTS_CAPACITY {
            queue.pop_front();
        }
    }

    /// 依記錄先後（最舊到最新）讀取某個 runtime 的最近事件；未登記或尚無紀錄回傳空。
    pub fn recent_events(&self, id: &RuntimeId) -> impl Iterator<Item = &RecentEvent> {
        self.recent
            .get(id)
            .into_iter()
            .flat_map(|queue| queue.iter())
    }

    /// 設定某個 runtime 的連線狀態；只改 `connection`，不動 `kind`／`endpoint`／
    /// workspaces 等其他欄位。對未登記的 runtime 回傳 `Err(Drift)`，不 panic、不隱式
    /// 登記，行為與 `replace`／`apply` 一致。
    pub fn set_connection(&mut self, id: &RuntimeId, state: ConnectionState) -> Result<(), Drift> {
        let runtime_state = self.states.get_mut(id).ok_or_else(|| Drift {
            reason: Message::DriftRuntimeNotRegistered { id: id.to_string() }.text(),
        })?;
        runtime_state.connection = state;
        Ok(())
    }

    /// 讀取某個 runtime 目前的連線狀態；未登記回傳 `None`。
    pub fn connection(&self, id: &RuntimeId) -> Option<&ConnectionState> {
        self.states.get(id).map(|state| &state.connection)
    }
}
