//! HERDR `SessionSnapshot` → `cockpit-core` `RuntimeSnapshot` 的純函數翻譯（spec
//! `herdr-runtime-translation`「snapshot 翻譯」；design D15）。
//!
//! 只做欄位對應，不做 I/O、不判斷協定相容性——`RuntimeSnapshot.protocol_warning` 一律填
//! `None`，由呼叫端（`HerdrRuntime::snapshot()`，task 2.6）視協定版本另行填入（design D12）。
//! 每個逐型別 helper 都公開，task 2.2 的 `event()` 翻譯會重用它們。

use std::time::SystemTime;

use cockpit_core::{
    Agent, AgentStatus, FocusChange, Focused, Pane, PaneId, RuntimeEvent, RuntimeSnapshot, Tab,
    TabId, Workspace, WorkspaceId,
};
use herdr_client::client::IncomingEvent;
use herdr_client::types::{
    AgentInfo, EventKind, PaneAgentDetectedPayload, PaneAgentStatusChangedPayload,
    PaneClosedPayload, PaneFocusedPayload, PaneInfo, PaneMovedPayload, PanePayload,
    SessionSnapshot, SubscriptionEventKind, TabClosedPayload, TabFocusedPayload, TabInfo,
    TabMovedPayload, TabPayload, TabRenamedPayload, WorkspaceClosedPayload,
    WorkspaceFocusedPayload, WorkspaceInfo, WorkspacePayload, WorkspaceRenamedPayload,
    WorkspacesReplacedPayload,
};

/// 把 HERDR `SessionSnapshot` 翻成 `RuntimeSnapshot`。`now` 是這次翻譯發生的時間，填入每個
/// pane 的 `updated_at`——HERDR snapshot 沒有逐 pane 的更新時間欄位。`layouts` 不翻譯
/// （design 決議：change 1 不畫 pane 幾何配置）。
pub fn snapshot(src: &SessionSnapshot, now: SystemTime) -> RuntimeSnapshot {
    RuntimeSnapshot {
        server_version: src.version.clone(),
        protocol: src.protocol,
        workspaces: src.workspaces.iter().map(workspace).collect(),
        tabs: src.tabs.iter().map(tab).collect(),
        panes: src.panes.iter().map(|p| pane(p, now)).collect(),
        agents: src.agents.iter().filter_map(agent).collect(),
        focused: Focused {
            workspace_id: src.focused_workspace_id.clone().map(WorkspaceId::new),
            tab_id: src.focused_tab_id.clone().map(TabId::new),
            pane_id: src.focused_pane_id.clone().map(PaneId::new),
        },
        protocol_warning: None,
    }
}

/// 把 HERDR `WorkspaceInfo` 翻成 `Workspace`：`label` 空字串視為無標籤。
pub fn workspace(src: &WorkspaceInfo) -> Workspace {
    Workspace {
        id: WorkspaceId::new(src.workspace_id.clone()),
        label: label(&src.label),
        number: src.number,
        agent_status: status(src.agent_status),
        focused: src.focused,
    }
}

/// 把 HERDR `TabInfo` 翻成 `Tab`。`TabInfo.label`、`pane_count` 丟棄——`cockpit_core::Tab`
/// 沒有 label 欄位，投影的 tab 只顯示 number。
pub fn tab(src: &TabInfo) -> Tab {
    Tab {
        id: TabId::new(src.tab_id.clone()),
        workspace_id: WorkspaceId::new(src.workspace_id.clone()),
        number: src.number,
        agent_status: status(src.agent_status),
        focused: src.focused,
    }
}

/// 把 HERDR `PaneInfo` 翻成 `Pane`。`terminal_title`、`revision` 丟棄（observer 子集只用
/// `title`）；`exited` 固定 `false`——HERDR snapshot 沒有這個欄位（design D15）；`updated_at`
/// 用翻譯當下時間 `now`。`label` 已是 `Option<String>`，空字串同樣視為無標籤。
pub fn pane(src: &PaneInfo, now: SystemTime) -> Pane {
    Pane {
        id: PaneId::new(src.pane_id.clone()),
        workspace_id: WorkspaceId::new(src.workspace_id.clone()),
        tab_id: TabId::new(src.tab_id.clone()),
        agent: src.agent.clone(),
        agent_status: status(src.agent_status),
        title: src.title.clone(),
        cwd: src.cwd.clone(),
        label: src.label.as_deref().and_then(label),
        focused: src.focused,
        exited: false,
        updated_at: now,
    }
}

/// 把 HERDR `AgentInfo` 翻成 `Agent`。`AgentInfo.agent` 為 `None` 表示這個 pane 目前沒有偵測
/// 到 agent；`cockpit_core::Agent.agent` 是必填 `String`，沒有東西可對應，回傳 `None` 並記錄
/// 一筆 debug log（呼叫端應該用 `filter_map` 略過這種情況）。
pub fn agent(src: &AgentInfo) -> Option<Agent> {
    let Some(name) = src.agent.clone() else {
        tracing::debug!(
            pane_id = %src.pane_id,
            workspace_id = %src.workspace_id,
            tab_id = %src.tab_id,
            "HERDR AgentInfo.agent 為 None，略過此筆 agent"
        );
        return None;
    };

    Some(Agent {
        agent: name,
        pane_id: PaneId::new(src.pane_id.clone()),
        workspace_id: WorkspaceId::new(src.workspace_id.clone()),
        tab_id: TabId::new(src.tab_id.clone()),
        agent_status: status(src.agent_status),
    })
}

/// 把 HERDR `AgentStatus` 逐值對應到 `cockpit_core::AgentStatus`。窮舉 `match`（不用 `_`）：
/// HERDR 未來新增變體時這裡會編譯失敗，逼著補上對應，而不是靜默落到某個既有值。
pub fn status(src: herdr_client::types::AgentStatus) -> AgentStatus {
    match src {
        herdr_client::types::AgentStatus::Idle => AgentStatus::Idle,
        herdr_client::types::AgentStatus::Working => AgentStatus::Working,
        herdr_client::types::AgentStatus::Blocked => AgentStatus::Blocked,
        herdr_client::types::AgentStatus::Done => AgentStatus::Done,
        herdr_client::types::AgentStatus::Unknown => AgentStatus::Unknown,
    }
}

/// 把 HERDR 的字串標籤正規化：空字串視為「無標籤」，否則原樣包成 `Some`。
pub fn label(value: &str) -> Option<String> {
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

// ---------------------------------------------------------------------------
// Task 2.2：事件翻譯（spec「事件翻譯對照」；設計文件 §7.2、design D3、D11）。
// ---------------------------------------------------------------------------

/// 把 HERDR `IncomingEvent` 翻成零或一個 `RuntimeEvent`（design D3：純函數，無狀態）。
///
/// `now` 只在需要組出完整 `Pane`（`pane_created`／`pane_updated`／`pane_moved`）時用得到，
/// 填進 `Pane.updated_at`——同 [`snapshot`] 的理由，HERDR 事件 payload 沒有逐 pane 的更新
/// 時間欄位。
///
/// 回傳規則（對照表見 spec「事件翻譯對照」）：
/// - 對照表列出的事件：payload 可解析 → `Some(對應事件)`；payload 解析失敗 →
///   `Some(RuntimeEvent::Drift { reason })`，`reason` 含事件名稱原字串。
/// - `layout_updated`、三種 `worktree_*`：不解析 payload，一律 `Some(RuntimeEvent::Noted {
///   kind })`，`kind` 是事件名稱原字串。
/// - 生命週期版 `pane_agent_status_changed`、`pane_output_changed`（沒有對應的無參數訂閱，
///   正常收不到，見設計文件 §2.3）、每 pane 版的 `pane.output_matched`／`pane.scroll_changed`
///   （change 1 不訂閱）、`IncomingEvent::Unknown`（未知事件名稱）：`None`，並記一筆
///   `tracing::debug!`。
///
/// 兩個 `match`（`EventKind`、`SubscriptionEventKind`）都窮舉、不用 `_`：HERDR 未來新增事件
/// 種類時這裡會編譯失敗，逼著明確決定新事件的翻譯規則，而不是被萬用分支靜默吃掉。
#[must_use]
pub fn event(src: &IncomingEvent, now: SystemTime) -> Option<RuntimeEvent> {
    match src {
        IncomingEvent::Lifecycle(kind, data) => lifecycle_event(*kind, data, now),
        IncomingEvent::PerPane(kind, data) => per_pane_event(*kind, data),
        IncomingEvent::Unknown { event, data: _ } => {
            tracing::debug!(event = %event, "未知事件名稱，略過翻譯");
            None
        }
    }
}

/// 把 `data` 解析成 `$ty`，成功就用 `$p` 綁定後求值 `$body`；失敗就組成 `Drift`，原因含事件
/// 名稱原字串與底層 serde 錯誤訊息。兩種結果都是 `Some`——payload 解析失敗不等於「沒有事件」，
/// 而是一筆需要呈報的 drift（design D3）。
///
/// 寫成 macro 而非泛型函式：`cockpit-herdr` 只直接依賴 `serde_json`，不直接依賴 `serde`
/// （Cargo.toml 不改），沒辦法在函式簽章上寫 `T: serde::de::DeserializeOwned` 這個 bound
/// （`serde` 對這個 crate 只是遞移依賴，命名該 crate 會編譯失敗）；macro 展開後在呼叫端
/// （已直接依賴 `serde_json` 且透過 `herdr_client` 間接使用到已經 derive `Deserialize` 的
/// payload 型別）直接呼叫 `serde_json::from_value::<具體型別>`，不需要在這裡另外命名任何
/// trait。
macro_rules! parse_or_drift {
    ($event_name:expr, $data:expr, |$p:ident : $ty:ty| $body:expr) => {
        match serde_json::from_value::<$ty>($data.clone()) {
            Ok($p) => Some($body),
            Err(err) => Some(RuntimeEvent::Drift {
                reason: format!("{} payload 無法解析：{}", $event_name, err),
            }),
        }
    };
}

/// 生命週期事件（26 種底線命名 `EventKind`）的翻譯，`event()` 的 `Lifecycle` 分支委派到此。
fn lifecycle_event(
    kind: EventKind,
    data: &serde_json::Value,
    now: SystemTime,
) -> Option<RuntimeEvent> {
    let name = kind.as_str();
    match kind {
        EventKind::WorkspaceCreated
        | EventKind::WorkspaceUpdated
        | EventKind::WorkspaceMetadataUpdated => {
            parse_or_drift!(name, data, |p: WorkspacePayload| {
                RuntimeEvent::WorkspaceUpserted(workspace(&p.workspace))
            })
        }
        EventKind::WorkspaceClosed => parse_or_drift!(name, data, |p: WorkspaceClosedPayload| {
            RuntimeEvent::WorkspaceRemoved(WorkspaceId::new(p.workspace_id))
        }),
        EventKind::WorkspaceRenamed => {
            parse_or_drift!(name, data, |p: WorkspaceRenamedPayload| {
                RuntimeEvent::WorkspaceRelabeled {
                    id: WorkspaceId::new(p.workspace_id),
                    label: p.label,
                }
            })
        }
        EventKind::WorkspaceMoved | EventKind::WorkspaceReordered => {
            parse_or_drift!(name, data, |p: WorkspacesReplacedPayload| {
                RuntimeEvent::WorkspacesReplaced(p.workspaces.iter().map(workspace).collect())
            })
        }
        EventKind::WorkspaceFocused => {
            parse_or_drift!(name, data, |p: WorkspaceFocusedPayload| {
                RuntimeEvent::FocusChanged(FocusChange {
                    workspace_id: Some(WorkspaceId::new(p.workspace_id)),
                    tab_id: None,
                    pane_id: None,
                })
            })
        }
        EventKind::WorktreeCreated | EventKind::WorktreeOpened | EventKind::WorktreeRemoved => {
            Some(RuntimeEvent::Noted {
                kind: name.to_string(),
            })
        }
        EventKind::TabCreated => parse_or_drift!(name, data, |p: TabPayload| {
            RuntimeEvent::TabUpserted(tab(&p.tab))
        }),
        EventKind::TabClosed => parse_or_drift!(name, data, |p: TabClosedPayload| {
            RuntimeEvent::TabRemoved(TabId::new(p.tab_id))
        }),
        EventKind::TabRenamed => parse_or_drift!(name, data, |p: TabRenamedPayload| {
            RuntimeEvent::TabRelabeled {
                id: TabId::new(p.tab_id),
                label: p.label,
            }
        }),
        EventKind::TabMoved => parse_or_drift!(name, data, |p: TabMovedPayload| {
            RuntimeEvent::TabsReplaced {
                workspace_id: WorkspaceId::new(p.workspace_id),
                tabs: p.tabs.iter().map(tab).collect(),
            }
        }),
        EventKind::TabFocused => parse_or_drift!(name, data, |p: TabFocusedPayload| {
            RuntimeEvent::FocusChanged(FocusChange {
                workspace_id: None,
                tab_id: Some(TabId::new(p.tab_id)),
                pane_id: None,
            })
        }),
        EventKind::PaneCreated | EventKind::PaneUpdated => {
            parse_or_drift!(name, data, |p: PanePayload| {
                RuntimeEvent::PaneUpserted(pane(&p.pane, now))
            })
        }
        EventKind::PaneClosed => parse_or_drift!(name, data, |p: PaneClosedPayload| {
            RuntimeEvent::PaneRemoved(PaneId::new(p.pane_id))
        }),
        EventKind::PaneFocused => parse_or_drift!(name, data, |p: PaneFocusedPayload| {
            RuntimeEvent::FocusChanged(FocusChange {
                workspace_id: None,
                tab_id: None,
                pane_id: Some(PaneId::new(p.pane_id)),
            })
        }),
        EventKind::PaneMoved => parse_or_drift!(name, data, |p: PaneMovedPayload| {
            RuntimeEvent::PaneMoved {
                previous: PaneId::new(p.previous_pane_id),
                pane: pane(&p.pane, now),
            }
        }),
        EventKind::PaneExited => parse_or_drift!(name, data, |p: PaneClosedPayload| {
            RuntimeEvent::PaneExited(PaneId::new(p.pane_id))
        }),
        EventKind::PaneAgentDetected => {
            parse_or_drift!(name, data, |p: PaneAgentDetectedPayload| {
                // design D11：`released == true` 或 `agent` 本來就是 `None` → 無 agent；
                // `final_status` 不使用。
                let agent = if p.released { None } else { p.agent };
                RuntimeEvent::AgentDetected {
                    pane_id: PaneId::new(p.pane_id),
                    agent,
                }
            })
        }
        EventKind::PaneOutputChanged | EventKind::PaneAgentStatusChanged => {
            // 生命週期版：沒有對應的無參數訂閱，設計文件 §2.3 記錄正常情況下收不到；出現時只
            // 記 debug、不產生事件（每 pane 版才是這兩個變體真正的翻譯來源）。
            tracing::debug!(
                event = name,
                "生命週期版事件沒有對應的無參數訂閱，正常收不到，略過翻譯"
            );
            None
        }
        EventKind::LayoutUpdated => Some(RuntimeEvent::Noted {
            kind: name.to_string(),
        }),
    }
}

/// 每 pane 事件（3 種點號命名 `SubscriptionEventKind`）的翻譯，`event()` 的 `PerPane` 分支
/// 委派到此。
fn per_pane_event(kind: SubscriptionEventKind, data: &serde_json::Value) -> Option<RuntimeEvent> {
    let name = kind.as_str();
    match kind {
        SubscriptionEventKind::PaneAgentStatusChanged => {
            parse_or_drift!(name, data, |p: PaneAgentStatusChangedPayload| {
                RuntimeEvent::AgentStatusChanged {
                    pane_id: PaneId::new(p.pane_id),
                    status: status(p.agent_status),
                    title: p.title,
                    agent: p.agent,
                }
            })
        }
        SubscriptionEventKind::PaneOutputMatched | SubscriptionEventKind::PaneScrollChanged => {
            // change 1 不訂閱這兩種（見 `Subscription::all_lifecycle` 與訂閱建置邏輯）；正常
            // 情況下收不到，出現時只記 debug、不產生事件。
            tracing::debug!(event = name, "不訂閱的每 pane 事件，略過翻譯");
            None
        }
    }
}
