//! RED→GREEN 驗收測試（Task 2.3，change `pipeline-projection`）：`resolve_binding`／
//! `validate_override`，涵蓋 spec `runtime-binding` 全部情境（design D1、D3、D7）。

mod common;

use std::time::{Duration, SystemTime};

use cockpit_core::domain::binding::{resolve_binding, validate_override};
use cockpit_core::{
    BindingResolution, BindingSource, BindingSpec, ConnectionState, Override, PaneId, Rejection,
    RuntimeStore, WorkstreamDef, WorkstreamId,
};

use common::{empty_focused, pane, runtime_id, snapshot, tab, workspace};

/// 建一個 workspace，`label` 指定為 `label`（不必等於 `id`）——測 workspace `label`
/// 大小寫敏感時，需要 id 與 label 分開控制。
fn workspace_with_label(id: &str, number: u32, label: &str) -> cockpit_core::Workspace {
    let mut ws = workspace(id, number);
    ws.label = Some(label.to_string());
    ws
}

/// `common::workspace` 的 `label` 預設 `None`；binding 的 workspace 篩選比對 `label`，
/// 所以測試裡要用到自動解析的 workspace 一律用這個 builder，`label` 等於 `id`。
fn labeled_workspace(id: &str, number: u32) -> cockpit_core::Workspace {
    workspace_with_label(id, number, id)
}

fn connected_state() -> ConnectionState {
    ConnectionState::Connected {
        since: SystemTime::UNIX_EPOCH,
        server_version: "0.9.0".to_string(),
        protocol: 1,
        last_snapshot_at: SystemTime::UNIX_EPOCH,
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

/// 建一個登記但未 `connected`（`Connecting`）的 runtime，狀態庫空的。
fn connecting_store(runtime: &str) -> RuntimeStore {
    let mut store = RuntimeStore::new();
    store.register(
        runtime_id(runtime),
        "herdr".to_string(),
        "endpoint".to_string(),
    );
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

fn workstream_with(binding: Option<BindingSpec>) -> WorkstreamDef {
    WorkstreamDef {
        id: WorkstreamId::new("be"),
        name: "be".to_string(),
        binding,
    }
}

// ---------------------------------------------------------------------------
// resolve_binding：以穩定特徵自動解析
// ---------------------------------------------------------------------------

#[test]
fn resolves_exactly_one_candidate_as_bound_auto() {
    let store = connected_store(
        "win",
        vec![labeled_workspace("w1", 1)],
        vec![tab("t1", "w1", 1)],
        vec![pane("p1", "w1", "t1")],
    );
    let ws = workstream_with(Some(binding_spec("win", "w1")));
    let (resolution, stale) = resolve_binding(&ws, None, &store);

    assert!(!stale);
    match resolution {
        BindingResolution::Bound {
            runtime,
            pane_id,
            source,
        } => {
            assert_eq!(runtime.as_str(), "win");
            assert_eq!(pane_id.as_str(), "p1");
            assert_eq!(source, BindingSource::Auto);
        }
        other => panic!("expected Bound, got {other:?}"),
    }
}

#[test]
fn pane_reappearing_under_new_id_still_resolves_by_label() {
    let binding = {
        let mut spec = binding_spec("wsl", "ai-cockpit");
        spec.pane_label = Some("backend".to_string());
        spec
    };
    let ws = workstream_with(Some(binding));

    // 一開始 pane id 是 w1:p2。
    let mut original_pane = pane("w1:p2", "ai-cockpit", "t1");
    original_pane.label = Some("backend".to_string());
    let store_before = connected_store(
        "wsl",
        vec![labeled_workspace("ai-cockpit", 1)],
        vec![tab("t1", "ai-cockpit", 1)],
        vec![original_pane],
    );
    let (resolution_before, _) = resolve_binding(&ws, None, &store_before);
    match resolution_before {
        BindingResolution::Bound { pane_id, .. } => assert_eq!(pane_id.as_str(), "w1:p2"),
        other => panic!("expected Bound, got {other:?}"),
    }

    // pane 被搬移，狀態庫中改以 id w1:p5 出現，label 仍是 backend（spec「pane 換 id 後重新對上」）。
    let mut moved_pane = pane("w1:p5", "ai-cockpit", "t1");
    moved_pane.label = Some("backend".to_string());
    let store_after = connected_store(
        "wsl",
        vec![labeled_workspace("ai-cockpit", 1)],
        vec![tab("t1", "ai-cockpit", 1)],
        vec![moved_pane],
    );
    let (resolution_after, stale_after) = resolve_binding(&ws, None, &store_after);
    assert!(!stale_after);
    match resolution_after {
        BindingResolution::Bound {
            pane_id, source, ..
        } => {
            assert_eq!(pane_id.as_str(), "w1:p5");
            assert_eq!(source, BindingSource::Auto);
        }
        other => panic!("expected Bound, got {other:?}"),
    }
}

#[test]
fn cwd_segment_match_excludes_similarly_named_sibling() {
    let binding = {
        let mut spec = binding_spec("win", "ws");
        spec.cwd = Some("worktrees/backend".to_string());
        spec
    };
    let ws = workstream_with(Some(binding));

    let mut unix_match = pane("p1", "ws", "t1");
    unix_match.cwd = Some("/home/u/proj/worktrees/backend/src".to_string());
    let mut windows_match = pane("p2", "ws", "t1");
    windows_match.cwd = Some("D:\\proj\\worktrees\\backend".to_string());

    let store = connected_store(
        "win",
        vec![labeled_workspace("ws", 1)],
        vec![tab("t1", "ws", 1)],
        vec![unix_match, windows_match],
    );
    let (resolution, stale) = resolve_binding(&ws, None, &store);
    assert!(!stale);
    match resolution {
        BindingResolution::Ambiguous { candidates, .. } => {
            let ids: Vec<&str> = candidates.iter().map(|id| id.as_str()).collect();
            assert_eq!(ids, vec!["p1", "p2"]);
        }
        other => panic!("expected Ambiguous (兩個都通過 cwd 篩選), got {other:?}"),
    }

    // 第三個 cwd 帶 `backend-old`，不該通過片段比對。
    let mut sibling = pane("p3", "ws", "t1");
    sibling.cwd = Some("/home/u/proj/worktrees/backend-old".to_string());
    let store_sibling_only = connected_store(
        "win",
        vec![labeled_workspace("ws", 1)],
        vec![tab("t1", "ws", 1)],
        vec![sibling],
    );
    let (resolution_sibling, _) = resolve_binding(&ws, None, &store_sibling_only);
    match resolution_sibling {
        BindingResolution::Unbound { .. } => {}
        other => panic!("expected Unbound (backend-old 不該匹配), got {other:?}"),
    }
}

#[test]
fn exited_pane_is_not_a_candidate() {
    let ws = workstream_with(Some(binding_spec("win", "ws")));
    let mut exited_pane = pane("p1", "ws", "t1");
    exited_pane.exited = true;
    let store = connected_store(
        "win",
        vec![labeled_workspace("ws", 1)],
        vec![tab("t1", "ws", 1)],
        vec![exited_pane],
    );
    let (resolution, _) = resolve_binding(&ws, None, &store);
    match resolution {
        BindingResolution::Unbound { .. } => {}
        other => panic!("expected Unbound (唯一候選已 exited), got {other:?}"),
    }
}

#[test]
fn multiple_candidates_are_ambiguous_in_store_order() {
    let ws = workstream_with(Some(binding_spec("win", "ws")));
    // 插入順序 p2 先於 p1：候選須依狀態庫順序（插入序），不是依 id 字典序。
    let store = connected_store(
        "win",
        vec![labeled_workspace("ws", 1)],
        vec![tab("t1", "ws", 1)],
        vec![pane("p2", "ws", "t1"), pane("p1", "ws", "t1")],
    );
    let (resolution, _) = resolve_binding(&ws, None, &store);
    match resolution {
        BindingResolution::Ambiguous { candidates, .. } => {
            let ids: Vec<&str> = candidates.iter().map(|id| id.as_str()).collect();
            assert_eq!(ids, vec!["p2", "p1"]);
        }
        other => panic!("expected Ambiguous, got {other:?}"),
    }
}

#[test]
fn no_matching_workspace_label_is_unbound() {
    let ws = workstream_with(Some(binding_spec("win", "does-not-exist")));
    let store = connected_store(
        "win",
        vec![labeled_workspace("ws", 1)],
        vec![tab("t1", "ws", 1)],
        vec![pane("p1", "ws", "t1")],
    );
    let (resolution, _) = resolve_binding(&ws, None, &store);
    match resolution {
        BindingResolution::Unbound { runtime } => assert_eq!(runtime.as_str(), "win"),
        other => panic!("expected Unbound, got {other:?}"),
    }
}

#[test]
fn candidate_filters_are_case_sensitive_and_reject_missing_or_wrong_agent() {
    struct Case {
        name: &'static str,
        workspace_label: &'static str,
        pane_label: &'static str,
        agent: Option<&'static str>,
        expect_bound: bool,
    }

    // binding 固定要求 workspace label "ws"、pane_label "backend"、agent "claude"；
    // 每個案例只有唯一一個候選 pane，直接斷言 Bound／Unbound，才能精確定位是哪個欄位
    // 的篩選出問題（design D7：完全相等、區分大小寫）。
    let cases = [
        Case {
            name: "workspace／pane_label／agent 大小寫全部完全相符",
            workspace_label: "ws",
            pane_label: "backend",
            agent: Some("claude"),
            expect_bound: true,
        },
        Case {
            name: "workspace label 大小寫不符",
            workspace_label: "WS",
            pane_label: "backend",
            agent: Some("claude"),
            expect_bound: false,
        },
        Case {
            name: "pane_label 大小寫不符",
            workspace_label: "ws",
            pane_label: "Backend",
            agent: Some("claude"),
            expect_bound: false,
        },
        Case {
            name: "agent 大小寫不符",
            workspace_label: "ws",
            pane_label: "backend",
            agent: Some("Claude"),
            expect_bound: false,
        },
        Case {
            name: "agent 缺失",
            workspace_label: "ws",
            pane_label: "backend",
            agent: None,
            expect_bound: false,
        },
        Case {
            name: "agent 值不符（非大小寫差異）",
            workspace_label: "ws",
            pane_label: "backend",
            agent: Some("codex"),
            expect_bound: false,
        },
    ];

    let binding = {
        let mut spec = binding_spec("win", "ws");
        spec.pane_label = Some("backend".to_string());
        spec.agent = Some("claude".to_string());
        spec
    };
    let ws = workstream_with(Some(binding));

    for case in cases {
        let mut candidate = pane("p1", "workspace", "t1");
        candidate.label = Some(case.pane_label.to_string());
        candidate.agent = case.agent.map(str::to_string);

        let store = connected_store(
            "win",
            vec![workspace_with_label("workspace", 1, case.workspace_label)],
            vec![tab("t1", "workspace", 1)],
            vec![candidate],
        );

        let (resolution, stale) = resolve_binding(&ws, None, &store);
        assert!(!stale, "case 「{}」：stale 不該為真", case.name);
        if case.expect_bound {
            match resolution {
                BindingResolution::Bound {
                    runtime,
                    pane_id,
                    source,
                } => {
                    assert_eq!(runtime.as_str(), "win", "case 「{}」", case.name);
                    assert_eq!(pane_id.as_str(), "p1", "case 「{}」", case.name);
                    assert_eq!(source, BindingSource::Auto, "case 「{}」", case.name);
                }
                other => panic!("case 「{}」：expected Bound, got {other:?}", case.name),
            }
        } else {
            match resolution {
                BindingResolution::Unbound { runtime } => {
                    assert_eq!(runtime.as_str(), "win", "case 「{}」", case.name);
                }
                other => panic!("case 「{}」：expected Unbound, got {other:?}", case.name),
            }
        }
    }
}

#[test]
fn disconnected_runtime_is_runtime_disconnected_and_does_not_reuse_previous_result() {
    let ws = workstream_with(Some(binding_spec("wsl", "ws")));
    let mut store = connected_store(
        "wsl",
        vec![labeled_workspace("ws", 1)],
        vec![tab("t1", "ws", 1)],
        vec![pane("p1", "ws", "t1")],
    );

    // 先確認真的解析到 Bound——「不沿用上次結果」才有意義可驗。
    let (resolution, stale) = resolve_binding(&ws, None, &store);
    assert!(!stale);
    assert!(
        matches!(resolution, BindingResolution::Bound { .. }),
        "前置條件：斷線前應先解析為 Bound，否則接下來驗的不是「不沿用上次結果」"
    );

    // 斷線：沿用 `RuntimeStore` 既有的 `set_connection` 語意改連線狀態，不動狀態庫裡的 pane
    // （驗證的是「runtime 未連線就不評估候選」，不是「候選碰巧消失」）。
    let runtime = runtime_id("wsl");
    store
        .set_connection(
            &runtime,
            ConnectionState::Disconnected {
                reason: "read timeout".to_string(),
                retry_in: Duration::from_secs(1),
            },
        )
        .expect("剛登記過");

    // pane 仍在狀態庫裡：斷線不是因為候選消失。
    assert!(
        store
            .state(&runtime)
            .expect("剛登記過")
            .panes
            .contains_key(&PaneId::new("p1")),
        "pane 應仍留在狀態庫，斷線判定不該依賴候選是否還在"
    );

    let (resolution, stale) = resolve_binding(&ws, None, &store);
    assert!(!stale);
    assert_eq!(
        resolution,
        BindingResolution::RuntimeDisconnected {
            runtime: runtime_id("wsl"),
            // 沒有覆蓋、由 `workstream.binding` 自動解析而來（spec「runtime 斷線」）。
            source: BindingSource::Auto,
        }
    );
}

#[test]
fn no_binding_and_no_override_resolves_to_none() {
    let ws = workstream_with(None);
    let store = RuntimeStore::new();
    let (resolution, stale) = resolve_binding(&ws, None, &store);
    assert!(!stale);
    assert_eq!(resolution, BindingResolution::None);
}

// ---------------------------------------------------------------------------
// resolve_binding：畫面覆蓋
// ---------------------------------------------------------------------------

#[test]
fn override_resolves_ambiguous_binding_to_the_picked_pane() {
    let ws = workstream_with(Some(binding_spec("win", "ws")));
    let store = connected_store(
        "win",
        vec![labeled_workspace("ws", 1)],
        vec![tab("t1", "ws", 1)],
        vec![pane("p1", "ws", "t1"), pane("p2", "ws", "t1")],
    );
    // 先確認自動解析真的是 Ambiguous（覆蓋要蓋掉這個結果）。
    let (auto_resolution, _) = resolve_binding(&ws, None, &store);
    assert!(matches!(
        auto_resolution,
        BindingResolution::Ambiguous { .. }
    ));

    let over = Override {
        runtime: runtime_id("win"),
        pane_id: cockpit_core::PaneId::new("p2"),
    };
    let (resolution, stale) = resolve_binding(&ws, Some(&over), &store);
    assert!(!stale);
    match resolution {
        BindingResolution::Bound {
            runtime,
            pane_id,
            source,
        } => {
            assert_eq!(runtime.as_str(), "win");
            assert_eq!(pane_id.as_str(), "p2");
            assert_eq!(source, BindingSource::Override);
        }
        other => panic!("expected Bound(Override), got {other:?}"),
    }
}

#[test]
fn override_pane_disappearing_falls_back_to_auto_and_is_marked_stale() {
    // workstream 沒有 binding：覆蓋失效後應回到 None，且 stale 旗標為真。
    let ws = workstream_with(None);
    let store = connected_store(
        "win",
        vec![workspace("ws", 1)],
        vec![tab("t1", "ws", 1)],
        vec![],
    );
    let over = Override {
        runtime: runtime_id("win"),
        pane_id: cockpit_core::PaneId::new("gone"),
    };
    let (resolution, stale) = resolve_binding(&ws, Some(&over), &store);
    assert!(stale, "runtime connected 但 pane 不存在，覆蓋應標記為失效");
    assert_eq!(resolution, BindingResolution::None);
}

#[test]
fn override_pane_exited_falls_back_to_auto_and_is_marked_stale() {
    let ws = workstream_with(Some(binding_spec("win", "ws")));
    let mut exited = pane("p9", "ws", "t1");
    exited.exited = true;
    let other = pane("p1", "ws", "t1");
    let store = connected_store(
        "win",
        vec![labeled_workspace("ws", 1)],
        vec![tab("t1", "ws", 1)],
        vec![exited, other],
    );
    let over = Override {
        runtime: runtime_id("win"),
        pane_id: cockpit_core::PaneId::new("p9"),
    };
    let (resolution, stale) = resolve_binding(&ws, Some(&over), &store);
    assert!(stale, "覆蓋指向的 pane 已 exited，應標記為失效");
    match resolution {
        BindingResolution::Bound {
            pane_id, source, ..
        } => {
            assert_eq!(pane_id.as_str(), "p1");
            assert_eq!(source, BindingSource::Auto);
        }
        other => panic!("expected Bound(Auto)（回到自動解析）, got {other:?}"),
    }
}

#[test]
fn override_is_preserved_while_runtime_disconnected_then_bound_after_reconnect() {
    let ws = workstream_with(None);
    let over = Override {
        runtime: runtime_id("wsl"),
        pane_id: cockpit_core::PaneId::new("w1:p1"),
    };

    // 斷線期間：覆蓋保留（不標記失效），結果為 RuntimeDisconnected。
    let disconnected_store = connecting_store("wsl");
    let (resolution, stale) = resolve_binding(&ws, Some(&over), &disconnected_store);
    assert!(!stale, "斷線期間覆蓋不應被視為失效");
    match resolution {
        BindingResolution::RuntimeDisconnected { runtime, source } => {
            assert_eq!(runtime.as_str(), "wsl");
            // 覆蓋造成的斷線帶出覆蓋來源（spec「覆蓋造成的斷線帶出覆蓋來源」）。
            assert_eq!(source, BindingSource::Override);
        }
        other => panic!("expected RuntimeDisconnected, got {other:?}"),
    }

    // 重連後 snapshot 中仍有未 exited 的 pane：回到 Bound(Override)。
    let reconnected_store = connected_store(
        "wsl",
        vec![labeled_workspace("ws", 1)],
        vec![tab("t1", "ws", 1)],
        vec![pane("w1:p1", "ws", "t1")],
    );
    let (resolution, stale) = resolve_binding(&ws, Some(&over), &reconnected_store);
    assert!(!stale);
    match resolution {
        BindingResolution::Bound {
            pane_id, source, ..
        } => {
            assert_eq!(pane_id.as_str(), "w1:p1");
            assert_eq!(source, BindingSource::Override);
        }
        other => panic!("expected Bound(Override), got {other:?}"),
    }
}

/// ui-fixes 修正波 1 B-M2（回歸測試，現行行為已正確，一開始即綠）：覆蓋指向的 runtime 連線中但
/// pane 已不存在（覆蓋失效），退回自動解析；自動綁定的是另一個已斷線的 runtime，結果為
/// `RuntimeDisconnected { source: Auto }`，且 stale 旗標仍為真。
#[test]
fn stale_override_falls_back_to_auto_binding_on_a_disconnected_runtime() {
    // 覆蓋用的 runtime「win」連線中、pane 不存在；自動綁定的 runtime「wsl」只登記、未連線。
    let mut store = connected_store(
        "win",
        vec![labeled_workspace("ws", 1)],
        vec![tab("t1", "ws", 1)],
        vec![],
    );
    store.register(
        runtime_id("wsl"),
        "herdr".to_string(),
        "endpoint".to_string(),
    );
    let ws = workstream_with(Some(binding_spec("wsl", "ws")));
    let over = Override {
        runtime: runtime_id("win"),
        pane_id: PaneId::new("gone"),
    };

    let (resolution, stale) = resolve_binding(&ws, Some(&over), &store);
    assert!(
        stale,
        "覆蓋的 runtime 連線中但 pane 不存在，覆蓋應標記為失效"
    );
    assert_eq!(
        resolution,
        BindingResolution::RuntimeDisconnected {
            runtime: runtime_id("wsl"),
            source: BindingSource::Auto,
        }
    );
}

#[test]
fn cancelling_override_falls_back_to_auto_resolution() {
    let ws = workstream_with(Some(binding_spec("win", "does-not-exist")));
    let store = connected_store(
        "win",
        vec![labeled_workspace("ws", 1)],
        vec![tab("t1", "ws", 1)],
        vec![pane("p1", "ws", "t1")],
    );
    // 取消覆蓋＝呼叫端傳 None；自動解析對到 0 個候選（workspace label 不對）。
    let (resolution, stale) = resolve_binding(&ws, None, &store);
    assert!(!stale);
    assert!(matches!(resolution, BindingResolution::Unbound { .. }));
}

// ---------------------------------------------------------------------------
// validate_override：四種拒絕
// ---------------------------------------------------------------------------

#[test]
fn validate_override_rejects_unregistered_runtime() {
    let store = RuntimeStore::new();
    let over = Override {
        runtime: runtime_id("win"),
        pane_id: cockpit_core::PaneId::new("p1"),
    };
    assert_eq!(
        validate_override(&over, &store),
        Err(Rejection::RuntimeNotRegistered)
    );
}

#[test]
fn validate_override_rejects_disconnected_runtime() {
    let store = connecting_store("win");
    let over = Override {
        runtime: runtime_id("win"),
        pane_id: cockpit_core::PaneId::new("p1"),
    };
    assert_eq!(
        validate_override(&over, &store),
        Err(Rejection::RuntimeNotConnected)
    );
}

#[test]
fn validate_override_rejects_missing_pane() {
    let store = connected_store(
        "win",
        vec![workspace("ws", 1)],
        vec![tab("t1", "ws", 1)],
        vec![],
    );
    let over = Override {
        runtime: runtime_id("win"),
        pane_id: cockpit_core::PaneId::new("wJ:p9"),
    };
    assert_eq!(
        validate_override(&over, &store),
        Err(Rejection::PaneNotFound)
    );
}

#[test]
fn validate_override_rejects_exited_pane() {
    let mut exited = pane("p1", "ws", "t1");
    exited.exited = true;
    let store = connected_store(
        "win",
        vec![workspace("ws", 1)],
        vec![tab("t1", "ws", 1)],
        vec![exited],
    );
    let over = Override {
        runtime: runtime_id("win"),
        pane_id: cockpit_core::PaneId::new("p1"),
    };
    assert_eq!(validate_override(&over, &store), Err(Rejection::PaneExited));
}

#[test]
fn validate_override_accepts_connected_runtime_with_live_pane() {
    let store = connected_store(
        "win",
        vec![workspace("ws", 1)],
        vec![tab("t1", "ws", 1)],
        vec![pane("p1", "ws", "t1")],
    );
    let over = Override {
        runtime: runtime_id("win"),
        pane_id: cockpit_core::PaneId::new("p1"),
    };
    assert_eq!(validate_override(&over, &store), Ok(()));
}
