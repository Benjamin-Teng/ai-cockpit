//! RED→GREEN 驗收測試（Task 2.4，change `pipeline-projection`）：`derive_status` 的推導優先序
//! （spec `pipeline-domain` 「StageStatus 推導」全部情境；design D8：標記 → 依賴 Pending →
//! 綁定 agent 狀態 → Ready）。
//!
//! `derive_status` 不吃 Task 的 Stage（design D8：Stage 只是線性順序，不影響 StageStatus 怎麼
//! 推導），所以底下的測試都不建 `TaskDef`／`ProjectDef`，只準備 `Mark`、依賴的 `Mark` 清單、
//! `BindingResolution`（部分測試以真的 `RuntimeStore` ＋ `resolve_binding` 產生，貼近 design D10
//! 「Scenario C、D 的判定」要求）、以及綁定 pane 的 `AgentStatus`。

mod common;

use std::time::Duration;

use cockpit_core::domain::binding::resolve_binding;
use cockpit_core::domain::{
    BindingResolution, BindingSpec, Mark, StageStatus, WorkstreamDef, WorkstreamId, derive_status,
};
use cockpit_core::{AgentStatus, ConnectionState, PaneId, RuntimeStore};

use common::{empty_focused, pane, runtime_id, snapshot, tab, workspace};

// ---------------------------------------------------------------------------
// 本檔自己的 fixture helper（比照 `domain_binding.rs` 的做法：每個整合測試檔各自局部定義，
// 不透過 `common` 共用——`common/mod.rs` 是給 Task 1.x 的 Runtime 層測試鷹架用的）。
// ---------------------------------------------------------------------------

fn labeled_workspace(id: &str, number: u32) -> cockpit_core::Workspace {
    let mut ws = workspace(id, number);
    ws.label = Some(id.to_string());
    ws
}

fn connected_state() -> ConnectionState {
    ConnectionState::Connected {
        since: std::time::SystemTime::UNIX_EPOCH,
        server_version: "0.9.0".to_string(),
        protocol: 1,
        last_snapshot_at: std::time::SystemTime::UNIX_EPOCH,
        protocol_warning: None,
    }
}

/// 建一個登記且 `connected` 的 runtime，`panes` 依傳入順序決定狀態庫的 pane 序號。
fn connected_store(
    runtime: &str,
    workspaces: Vec<cockpit_core::Workspace>,
    tabs: Vec<cockpit_core::Tab>,
    panes: Vec<cockpit_core::Pane>,
) -> RuntimeStore {
    let mut store = RuntimeStore::new();
    let id = runtime_id(runtime);
    store.register(id.clone(), "herdr".to_string(), "endpoint".to_string());
    store
        .set_connection(&id, connected_state())
        .expect("剛登記過");
    store
        .replace(
            &id,
            snapshot(workspaces, tabs, panes, Vec::new(), empty_focused()),
        )
        .expect("剛登記過");
    store
}

fn binding_spec(runtime: &str, workspace: &str) -> BindingSpec {
    BindingSpec {
        runtime: runtime_id(runtime),
        workspace: workspace.to_string(),
        pane_label: None,
        cwd: None,
        agent: None,
    }
}

fn workstream_with(id: &str, binding: Option<BindingSpec>) -> WorkstreamDef {
    WorkstreamDef {
        id: WorkstreamId::new(id),
        name: id.to_string(),
        binding,
    }
}

/// 從一個已解析的 `BindingResolution` 查出綁定 pane 目前的 `AgentStatus`；不是 `Bound` 一律
/// `None`（`derive_status` 在那些情況本來就不看這個值，呼叫端傳什麼都不影響結果）。
fn bound_agent_status(store: &RuntimeStore, resolution: &BindingResolution) -> Option<AgentStatus> {
    match resolution {
        BindingResolution::Bound {
            runtime, pane_id, ..
        } => store
            .state(runtime)
            .and_then(|state| state.panes.get(pane_id))
            .map(|pane| pane.agent_status),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Scenario C／D：以真的 RuntimeStore + resolve_binding 產生 BindingResolution（design D10）。
// ---------------------------------------------------------------------------

/// Scenario C（spec）：task `A` 在 `Implement`、標記 `none`、無依賴，所屬 workstream 已綁定到
/// runtime `win`（`connected`）的 pane `wJ:p1`；`wJ:p1` 的 `agent_status` 變成 `working` 時，
/// `A` 的 StageStatus 為 `running`。
#[test]
fn scenario_c_running_in_implement() {
    let mut bound_pane = pane("wJ:p1", "w1", "t1");
    bound_pane.agent_status = AgentStatus::Working;
    let store = connected_store(
        "win",
        vec![labeled_workspace("w1", 1)],
        vec![tab("t1", "w1", 1)],
        vec![bound_pane],
    );
    let ws = workstream_with("implement", Some(binding_spec("win", "w1")));
    let (resolution, stale) = resolve_binding(&ws, None, &store);
    assert!(!stale);
    assert!(matches!(resolution, BindingResolution::Bound { .. }));

    let agent_status = bound_agent_status(&store, &resolution);
    let status = derive_status(Mark::None, &[], &resolution, agent_status, true);

    assert_eq!(status, StageStatus::Running);
}

/// spec 「綁定 agent blocked」：同 Scenario C 的綁定，`agent_status` 變成 `blocked` 時為 `blocked`。
#[test]
fn bound_agent_blocked_is_blocked() {
    let mut bound_pane = pane("wJ:p1", "w1", "t1");
    bound_pane.agent_status = AgentStatus::Blocked;
    let store = connected_store(
        "win",
        vec![labeled_workspace("w1", 1)],
        vec![tab("t1", "w1", 1)],
        vec![bound_pane],
    );
    let ws = workstream_with("implement", Some(binding_spec("win", "w1")));
    let (resolution, _) = resolve_binding(&ws, None, &store);
    let agent_status = bound_agent_status(&store, &resolution);

    assert_eq!(
        derive_status(Mark::None, &[], &resolution, agent_status, true),
        StageStatus::Blocked
    );
}

/// spec 「done 不是 Completed」：同 Scenario C 的綁定，`agent_status` 變成 `done` 時為 `ready`，
/// 不是 `completed`——`StageStatus::Completed` 只能來自 `Mark::Completed`。
#[test]
fn done_is_not_completed() {
    let mut bound_pane = pane("wJ:p1", "w1", "t1");
    bound_pane.agent_status = AgentStatus::Done;
    let store = connected_store(
        "win",
        vec![labeled_workspace("w1", 1)],
        vec![tab("t1", "w1", 1)],
        vec![bound_pane],
    );
    let ws = workstream_with("implement", Some(binding_spec("win", "w1")));
    let (resolution, _) = resolve_binding(&ws, None, &store);
    let agent_status = bound_agent_status(&store, &resolution);

    assert_eq!(
        derive_status(Mark::None, &[], &resolution, agent_status, true),
        StageStatus::Ready
    );
}

/// idle／unknown 也落在「其他」分支，同樣是 `ready`（spec 「(3)…其他（idle、done、unknown、
/// pane 沒有 agent）→ ready」）。
#[test]
fn idle_and_unknown_agent_status_are_ready() {
    for status in [AgentStatus::Idle, AgentStatus::Unknown] {
        let mut bound_pane = pane("wJ:p1", "w1", "t1");
        bound_pane.agent_status = status;
        let store = connected_store(
            "win",
            vec![labeled_workspace("w1", 1)],
            vec![tab("t1", "w1", 1)],
            vec![bound_pane],
        );
        let ws = workstream_with("implement", Some(binding_spec("win", "w1")));
        let (resolution, _) = resolve_binding(&ws, None, &store);
        let agent_status = bound_agent_status(&store, &resolution);

        assert_eq!(
            derive_status(Mark::None, &[], &resolution, agent_status, true),
            StageStatus::Ready,
            "agent_status={status:?}"
        );
    }
}

/// spec 「runtime 斷線不沿用舊狀態」：`A` 綁定的 pane 最後已知為 `working`，該 runtime 變成
/// `disconnected` 後 `A` 為 `ready`——測試名沿用 spec 情境用語。
#[test]
fn disconnected_is_ready() {
    let mut bound_pane = pane("wJ:p1", "w1", "t1");
    bound_pane.agent_status = AgentStatus::Working;
    let mut store = connected_store(
        "win",
        vec![labeled_workspace("w1", 1)],
        vec![tab("t1", "w1", 1)],
        vec![bound_pane],
    );
    let ws = workstream_with("implement", Some(binding_spec("win", "w1")));

    // 前置條件：斷線前先確認真的解析為 Bound、working（否則接下來驗的不是「不沿用舊狀態」）。
    let (resolution_before, _) = resolve_binding(&ws, None, &store);
    let agent_status_before = bound_agent_status(&store, &resolution_before);
    assert_eq!(
        derive_status(
            Mark::None,
            &[],
            &resolution_before,
            agent_status_before,
            true
        ),
        StageStatus::Running,
        "前置條件：斷線前應先是 running"
    );

    let runtime = runtime_id("win");
    store
        .set_connection(
            &runtime,
            ConnectionState::Disconnected {
                reason: "read timeout".to_string(),
                retry_in: Duration::from_secs(1),
            },
        )
        .expect("剛登記過");

    let (resolution_after, stale) = resolve_binding(&ws, None, &store);
    assert!(!stale);
    assert!(matches!(
        resolution_after,
        BindingResolution::RuntimeDisconnected { .. }
    ));
    let agent_status_after = bound_agent_status(&store, &resolution_after);

    assert_eq!(
        derive_status(Mark::None, &[], &resolution_after, agent_status_after, true),
        StageStatus::Ready
    );
}

/// spec 「依賴未完成為 Pending」：task `B` 的 `depends_on = ["A"]`，`A` 標記 `none`；`B` 所屬
/// workstream 綁定的 pane 為 `working`——`B` 為 `pending`；把 `A` 標 Completed 後 `B` 變成
/// `running`。
#[test]
fn dependency_incomplete_is_pending_then_running_once_completed() {
    let mut bound_pane = pane("wJ:p1", "w1", "t1");
    bound_pane.agent_status = AgentStatus::Working;
    let store = connected_store(
        "win",
        vec![labeled_workspace("w1", 1)],
        vec![tab("t1", "w1", 1)],
        vec![bound_pane],
    );
    let ws = workstream_with("implement", Some(binding_spec("win", "w1")));
    let (resolution, _) = resolve_binding(&ws, None, &store);
    let agent_status = bound_agent_status(&store, &resolution);

    assert_eq!(
        derive_status(Mark::None, &[Mark::None], &resolution, agent_status, true),
        StageStatus::Pending,
        "依賴 A 尚未 Completed，B 應為 pending"
    );
    assert_eq!(
        derive_status(
            Mark::None,
            &[Mark::Completed],
            &resolution,
            agent_status,
            true
        ),
        StageStatus::Running,
        "依賴 A 已 Completed，B 應照綁定 agent 狀態推導成 running"
    );
}

/// spec 「標記優先於依賴與 agent」：`B` 依賴未完成的 `A`，`B` 綁定的 pane 為 `working`；`B`
/// 被標 Failed 後為 `failed`（標記在優先序最前面）。
#[test]
fn mark_failed_overrides_dependency_and_agent() {
    let mut bound_pane = pane("wJ:p1", "w1", "t1");
    bound_pane.agent_status = AgentStatus::Working;
    let store = connected_store(
        "win",
        vec![labeled_workspace("w1", 1)],
        vec![tab("t1", "w1", 1)],
        vec![bound_pane],
    );
    let ws = workstream_with("implement", Some(binding_spec("win", "w1")));
    let (resolution, _) = resolve_binding(&ws, None, &store);
    let agent_status = bound_agent_status(&store, &resolution);

    assert_eq!(
        derive_status(Mark::Failed, &[Mark::None], &resolution, agent_status, true),
        StageStatus::Failed
    );
}

/// Scenario D（spec）：workstream `backend`、`frontend`、`tests` 分別綁定三個不同 pane；三個
/// pane 同時為 `working` 時，對應的三個 task 皆為 `running`（各自的 `BindingResolution`
/// 互不影響——這裡不驗「各自的目前 Stage」，因為 `derive_status` 不吃 Stage，那部分屬於
/// `TaskProgress` 本身，不是本 task 的推導規則）。
#[test]
fn scenario_d_parallel_workstreams() {
    let mut backend_pane = pane("p-be", "backend", "t-be");
    backend_pane.agent_status = AgentStatus::Working;
    let mut frontend_pane = pane("p-fe", "frontend", "t-fe");
    frontend_pane.agent_status = AgentStatus::Working;
    let mut tests_pane = pane("p-qa", "tests", "t-qa");
    tests_pane.agent_status = AgentStatus::Working;

    let store = connected_store(
        "win",
        vec![
            labeled_workspace("backend", 1),
            labeled_workspace("frontend", 2),
            labeled_workspace("tests", 3),
        ],
        vec![
            tab("t-be", "backend", 1),
            tab("t-fe", "frontend", 1),
            tab("t-qa", "tests", 1),
        ],
        vec![backend_pane, frontend_pane, tests_pane],
    );

    let backend_ws = workstream_with("backend", Some(binding_spec("win", "backend")));
    let frontend_ws = workstream_with("frontend", Some(binding_spec("win", "frontend")));
    let tests_ws = workstream_with("tests", Some(binding_spec("win", "tests")));

    for ws in [&backend_ws, &frontend_ws, &tests_ws] {
        let (resolution, stale) = resolve_binding(ws, None, &store);
        assert!(!stale);
        let agent_status = bound_agent_status(&store, &resolution);
        assert_eq!(
            derive_status(Mark::None, &[], &resolution, agent_status, true),
            StageStatus::Running,
            "workstream {} 應為 running",
            ws.id
        );
    }
}

/// spec 「同一 workstream 多個 Task」：workstream `backend` 有 task `x`（標記 `none`）與 `y`
/// （標記 `completed`），綁定 pane 為 `working`；`x` 為 `running`、`y` 為 `completed`——同一個
/// `BindingResolution`（與同一次查出的 `agent_status`）餵給兩個 Task 各自推導，互不影響。
#[test]
fn same_workstream_multiple_tasks() {
    let mut bound_pane = pane("p-be", "backend", "t-be");
    bound_pane.agent_status = AgentStatus::Working;
    let store = connected_store(
        "win",
        vec![labeled_workspace("backend", 1)],
        vec![tab("t-be", "backend", 1)],
        vec![bound_pane],
    );
    let ws = workstream_with("backend", Some(binding_spec("win", "backend")));
    let (resolution, _) = resolve_binding(&ws, None, &store);
    let agent_status = bound_agent_status(&store, &resolution);

    let status_x = derive_status(Mark::None, &[], &resolution, agent_status, true);
    let status_y = derive_status(Mark::Completed, &[], &resolution, agent_status, true);
    let status_z = derive_status(Mark::None, &[], &resolution, agent_status, false);

    assert_eq!(status_x, StageStatus::Running, "x 標記 none 應為 running");
    assert_eq!(
        status_z,
        StageStatus::Ready,
        "z 不是目前 task，即使綁定 pane working 也應為 ready"
    );
    assert_eq!(
        status_y,
        StageStatus::Completed,
        "y 標記 completed 應為 completed"
    );
}

// ---------------------------------------------------------------------------
// 其餘 BindingResolution 種類（沒有 binding／未綁定／歧義）：一律 Ready（spec 「(4) 其餘情況」）。
// 這幾個不需要真的 RuntimeStore 查詢，直接構造 BindingResolution 即可——derive_status 是純函數，
// 不在乎 BindingResolution 從哪裡來。
// ---------------------------------------------------------------------------

#[test]
fn no_binding_unbound_and_ambiguous_are_ready() {
    let cases = [
        BindingResolution::None,
        BindingResolution::Unbound {
            runtime: runtime_id("win"),
        },
        BindingResolution::Ambiguous {
            runtime: runtime_id("win"),
            candidates: vec![PaneId::new("p1"), PaneId::new("p2")],
        },
    ];

    for resolution in &cases {
        assert_eq!(
            derive_status(
                Mark::None,
                &[],
                resolution,
                Some(AgentStatus::Working),
                true
            ),
            StageStatus::Ready,
            "resolution={resolution:?}"
        );
    }
}

/// spec 「沒有目前 task 時不猜」：沒有目前 task（`is_active = false`）時，綁定 pane 為
/// `working` 或 `blocked`，標記 none 的 task 一律 `ready`；標記與依賴優先序不變。
#[test]
fn no_active_task_is_not_guessed() {
    let ws = workstream_with("backend", Some(binding_spec("win", "backend")));
    for status in [AgentStatus::Working, AgentStatus::Blocked] {
        let mut bound_pane = pane("p-be", "backend", "t-be");
        bound_pane.agent_status = status;
        let store = connected_store(
            "win",
            vec![labeled_workspace("backend", 1)],
            vec![tab("t-be", "backend", 1)],
            vec![bound_pane],
        );
        let (resolution, _) = resolve_binding(&ws, None, &store);
        let agent_status = bound_agent_status(&store, &resolution);
        assert_eq!(
            derive_status(Mark::None, &[], &resolution, agent_status, false),
            StageStatus::Ready,
            "x（status={status:?}）"
        );
        assert_eq!(
            derive_status(Mark::None, &[Mark::None], &resolution, agent_status, false),
            StageStatus::Pending
        );
        assert_eq!(
            derive_status(Mark::Failed, &[], &resolution, agent_status, false),
            StageStatus::Failed
        );
    }
}
