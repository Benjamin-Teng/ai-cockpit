//! `ProjectedState` 純函數與最近事件的驗收測試（Task 1.5；spec `state-projection`）。

mod common;

use std::time::{Duration, SystemTime};

use cockpit_core::{
    AgentStatus, ConnectionState, DomainState, Focused, ProjectedConnection, RuntimeEvent,
    RuntimeStore, project,
};
use serde_json::json;

use common::{agent, pane, pane_id, runtime_id, snapshot, tab, tab_id, workspace, workspace_id};

fn epoch_secs(secs: u64) -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(secs)
}

#[test]
fn nested_projection_matches_design_json_shape() {
    let mut store = RuntimeStore::new();
    let win = runtime_id("win");
    store.register(
        win.clone(),
        "herdr".to_string(),
        "named-pipe C:\\Users\\test\\herdr.sock".to_string(),
    );

    let mut ws = workspace("wJ", 2);
    ws.label = Some("ai-cockpit".to_string());
    ws.agent_status = AgentStatus::Working;
    ws.focused = true;

    let mut t1 = tab("wJ:t1", "wJ", 1);
    t1.agent_status = AgentStatus::Working;
    t1.focused = true;

    let mut p1 = pane("wJ:p1", "wJ", "wJ:t1");
    p1.agent = Some("claude".to_string());
    p1.agent_status = AgentStatus::Working;
    p1.title = Some("claude - working".to_string());
    p1.cwd = Some("D:\\projects\\ai-cockpit".to_string());
    p1.focused = true;
    p1.updated_at = epoch_secs(3000);

    let focused = Focused {
        workspace_id: Some(workspace_id("wJ")),
        tab_id: Some(tab_id("wJ:t1")),
        pane_id: Some(pane_id("wJ:p1")),
    };

    store
        .replace(
            &win,
            snapshot(vec![ws], vec![t1], vec![p1], vec![], focused),
        )
        .expect("replace 應成功");

    store
        .set_connection(
            &win,
            ConnectionState::Connected {
                since: epoch_secs(1000),
                server_version: "0.9.0-preview.1".to_string(),
                protocol: 22,
                last_snapshot_at: epoch_secs(2000),
                protocol_warning: None,
            },
        )
        .expect("set_connection 應成功");

    let projected = project(&store, &DomainState::default(), 42, epoch_secs(4000));
    let actual = serde_json::to_value(&projected).expect("序列化應成功");

    let expected = json!({
        "version": 42,
        "generated_at": "1970-01-01T01:06:40Z",
        "runtimes": [
            {
                "id": "win",
                "kind": "herdr",
                "endpoint": "named-pipe C:\\Users\\test\\herdr.sock",
                "connection": {
                    "state": "connected",
                    "since": "1970-01-01T00:16:40Z",
                    "server_version": "0.9.0-preview.1",
                    "protocol": 22,
                    "last_snapshot_at": "1970-01-01T00:33:20Z",
                    "protocol_warning": null
                },
                "focused": {
                    "workspace_id": "wJ",
                    "tab_id": "wJ:t1",
                    "pane_id": "wJ:p1"
                },
                "workspaces": [
                    {
                        "id": "wJ",
                        "label": "ai-cockpit",
                        "number": 2,
                        "agent_status": "working",
                        "focused": true,
                        "tabs": [
                            {
                                "id": "wJ:t1",
                                "number": 1,
                                "agent_status": "working",
                                "focused": true,
                                "panes": [
                                    {
                                        "id": "wJ:p1",
                                        "agent": "claude",
                                        "agent_status": "working",
                                        "title": "claude - working",
                                        "cwd": "D:\\projects\\ai-cockpit",
                                        "label": null,
                                        "focused": true,
                                        "exited": false,
                                        "updated_at": "1970-01-01T00:50:00Z"
                                    }
                                ]
                            }
                        ]
                    }
                ]
            }
        ],
        // change `pipeline-projection`：沒有 Project 時 `projects` 為空陣列（spec 「投影形狀」）。
        "projects": [],
        "recent_events": []
    });

    assert_eq!(actual, expected);
}

#[test]
fn disconnected_connection_carries_reason_and_retry() {
    let mut store = RuntimeStore::new();
    let wsl = runtime_id("wsl");
    store.register(wsl.clone(), "herdr".to_string(), "tcp://wsl".to_string());

    store
        .set_connection(
            &wsl,
            ConnectionState::Disconnected {
                reason: "r".to_string(),
                retry_in: Duration::from_secs(60),
            },
        )
        .expect("set_connection 應成功");

    let projected = project(&store, &DomainState::default(), 1, epoch_secs(0));

    assert_eq!(projected.runtimes.len(), 1);
    assert_eq!(
        projected.runtimes[0].connection,
        ProjectedConnection::Disconnected {
            reason: "r".to_string(),
            retry_in_secs: 60,
        }
    );

    let actual = serde_json::to_value(&projected.runtimes[0].connection).expect("序列化應成功");
    let expected = json!({
        "state": "disconnected",
        "reason": "r",
        "retry_in_secs": 60
    });
    assert_eq!(actual, expected);
}

#[test]
fn timestamps_are_rfc3339() {
    let mut store = RuntimeStore::new();
    let win = runtime_id("win");
    store.register(win.clone(), "herdr".to_string(), "tcp://win".to_string());

    let mut p1 = pane("wJ:p1", "wJ", "wJ:t1");
    p1.updated_at = epoch_secs(12345);

    store
        .replace(
            &win,
            snapshot(
                vec![workspace("wJ", 1)],
                vec![tab("wJ:t1", "wJ", 1)],
                vec![p1],
                vec![],
                Focused {
                    workspace_id: None,
                    tab_id: None,
                    pane_id: None,
                },
            ),
        )
        .expect("replace 應成功");

    store
        .set_connection(
            &win,
            ConnectionState::Connected {
                since: epoch_secs(100),
                server_version: "0.9.0".to_string(),
                protocol: 1,
                last_snapshot_at: epoch_secs(200),
                protocol_warning: None,
            },
        )
        .expect("set_connection 應成功");

    let projected = project(&store, &DomainState::default(), 1, epoch_secs(999999));

    assert_eq!(projected.generated_at, "1970-01-12T13:46:39Z");

    let pane_json = &projected.runtimes[0].workspaces[0].tabs[0].panes[0];
    assert_eq!(pane_json.updated_at, "1970-01-01T03:25:45Z");

    match &projected.runtimes[0].connection {
        ProjectedConnection::Connected {
            since,
            last_snapshot_at,
            ..
        } => {
            assert_eq!(since, "1970-01-01T00:01:40Z");
            assert_eq!(last_snapshot_at, "1970-01-01T00:03:20Z");
        }
        other => panic!("預期 Connected，實際是 {other:?}"),
    }

    // 秒精度、固定格式，一律以 Z 結尾、共 20 個字元。
    assert_eq!(projected.generated_at.len(), 20);
    assert!(projected.generated_at.ends_with('Z'));
}

#[test]
fn ordering_by_number_and_pane_sequence() {
    let mut store = RuntimeStore::new();
    let win = runtime_id("win");
    store.register(win.clone(), "herdr".to_string(), "tcp://win".to_string());

    // workspace：number 相同時依 id 字串排序；number 不同時依 number 遞增，
    // 刻意讓建立順序與最終排序相反來驗證真的有排序、不是巧合。
    let ws_high_number = workspace("wZ", 2);
    let ws_low_number = workspace("wA", 1);
    // 兩個 number 都是 3，id 字串 "tie-a" < "tie-b"，應排 tie-a 在前。
    let ws_tie_a = workspace("tie-a", 3);
    let ws_tie_b = workspace("tie-b", 3);

    let t_high = tab("wA:t2", "wA", 2);
    let t_low = tab("wA:t1", "wA", 1);

    // pane：陣列順序放 "wA:p_b" 在前、"wA:p_a" 在後，id 字母序卻相反，用來驗證排序依
    // pane_seq（進入狀態庫的先後＝陣列順序），不是 id 字串。
    let pane_entered_first = pane("wA:p_b", "wA", "wA:t1");
    let pane_entered_second = pane("wA:p_a", "wA", "wA:t1");

    store
        .replace(
            &win,
            snapshot(
                vec![ws_high_number, ws_low_number, ws_tie_a, ws_tie_b],
                vec![t_high, t_low],
                vec![pane_entered_first, pane_entered_second],
                vec![],
                Focused {
                    workspace_id: None,
                    tab_id: None,
                    pane_id: None,
                },
            ),
        )
        .expect("replace 應成功");

    let projected = project(&store, &DomainState::default(), 1, epoch_secs(0));
    let runtime = &projected.runtimes[0];

    let workspace_ids: Vec<&str> = runtime.workspaces.iter().map(|w| w.id.as_str()).collect();
    assert_eq!(workspace_ids, vec!["wA", "wZ", "tie-a", "tie-b"]);

    let wa = runtime
        .workspaces
        .iter()
        .find(|w| w.id.as_str() == "wA")
        .expect("wA 應存在");
    let tab_ids: Vec<&str> = wa.tabs.iter().map(|t| t.id.as_str()).collect();
    assert_eq!(tab_ids, vec!["wA:t1", "wA:t2"]);

    let t1 = wa
        .tabs
        .iter()
        .find(|t| t.id.as_str() == "wA:t1")
        .expect("wA:t1 應存在");
    // 陣列順序是 [p_b, p_a]：p_b 先進狀態庫拿到較小的 pane_seq，應排在前面，
    // 即使 id 字串「p_a」比「p_b」小。
    let pane_ids: Vec<&str> = t1.panes.iter().map(|p| p.id.as_str()).collect();
    assert_eq!(pane_ids, vec!["wA:p_b", "wA:p_a"]);
}

#[test]
fn orphan_pane_not_projected() {
    let mut store = RuntimeStore::new();
    let win = runtime_id("win");
    store.register(win.clone(), "herdr".to_string(), "tcp://win".to_string());

    // 一個合法 tab／pane 當對照組：如果實作把「父層不存在」的檢查整個拿掉、改成只看
    // workspace_id（例如 `project_panes` 漏掉 `pane.tab_id == tab.id` 這個條件），
    // 孤兒 pane 就會「混」進這個合法 tab 底下，而不是單純從空清單裡消失——後者即使
    // filter 條件下錯也可能巧合通過（Fix round 1 突變證據 M5 發現的落差，原本版本
    // 沒有合法 tab，任何「沒 tab 就什麼都不投影」的實作都會矇混過關）。
    let legit = pane("wJ:legit", "wJ", "wJ:t1");
    // `replace` 不驗證關聯完整性：故意塞一個 tab_id 不存在於 tabs 清單的 pane，
    // 模擬「父層不在狀態庫」的孤兒 pane。
    let orphan = pane("wJ:orphan", "wJ", "wJ:missing-tab");

    store
        .replace(
            &win,
            snapshot(
                vec![workspace("wJ", 1)],
                vec![tab("wJ:t1", "wJ", 1)],
                vec![legit, orphan],
                vec![],
                Focused {
                    workspace_id: None,
                    tab_id: None,
                    pane_id: None,
                },
            ),
        )
        .expect("replace 應成功");

    let projected = project(&store, &DomainState::default(), 1, epoch_secs(0));
    let runtime = &projected.runtimes[0];

    assert_eq!(runtime.workspaces.len(), 1);
    assert_eq!(runtime.workspaces[0].id.as_str(), "wJ");
    assert_eq!(runtime.workspaces[0].tabs.len(), 1, "只有一個合法 tab");

    let pane_ids: Vec<&str> = runtime.workspaces[0].tabs[0]
        .panes
        .iter()
        .map(|p| p.id.as_str())
        .collect();
    assert_eq!(
        pane_ids,
        vec!["wJ:legit"],
        "孤兒 pane 不該混進合法 tab 的 pane 清單"
    );

    let json_text = serde_json::to_string(&projected).expect("序列化應成功");
    assert!(
        !json_text.contains("wJ:orphan"),
        "孤兒 pane 不該出現在投影 JSON 的任何角落"
    );
}

#[test]
fn recent_events_keeps_latest_fifty() {
    let mut store = RuntimeStore::new();
    let win = runtime_id("win");
    store.register(win.clone(), "herdr".to_string(), "tcp://win".to_string());

    for i in 0..60u64 {
        store
            .apply(
                &win,
                RuntimeEvent::Noted {
                    kind: format!("evt_{i}"),
                },
                epoch_secs(i),
            )
            .expect("Noted 一律成功");
    }

    let projected = project(&store, &DomainState::default(), 1, epoch_secs(1000));
    assert_eq!(projected.recent_events.len(), 50);

    // 最新在前：最後一筆（evt_59）應排最前面，最早留下的是 evt_10（60 筆丟最舊 10 筆）。
    assert_eq!(projected.recent_events[0].kind, "evt_59");
    assert_eq!(projected.recent_events[49].kind, "evt_10");

    let kinds: Vec<&str> = projected
        .recent_events
        .iter()
        .map(|e| e.kind.as_str())
        .collect();
    assert!(!kinds.contains(&"evt_9"), "evt_0..=evt_9 應已被丟棄");
}

#[test]
fn noted_only_enters_recent_events() {
    let mut store = RuntimeStore::new();
    let win = runtime_id("win");
    store.register(win.clone(), "herdr".to_string(), "tcp://win".to_string());

    store
        .replace(
            &win,
            snapshot(
                vec![workspace("wJ", 1)],
                vec![tab("wJ:t1", "wJ", 1)],
                vec![pane("wJ:p1", "wJ", "wJ:t1")],
                vec![agent("wJ:p1", "wJ", "wJ:t1", "claude", AgentStatus::Idle)],
                Focused {
                    workspace_id: None,
                    tab_id: None,
                    pane_id: None,
                },
            ),
        )
        .expect("replace 應成功");

    let before = store.state(&win).cloned().expect("win 應已登記");

    store
        .apply(
            &win,
            RuntimeEvent::Noted {
                kind: "layout_updated".to_string(),
            },
            epoch_secs(10),
        )
        .expect("Noted 一律成功");

    assert_eq!(
        store.state(&win).expect("win 應已登記"),
        &before,
        "Noted 不該改變狀態庫內容"
    );

    let projected = project(&store, &DomainState::default(), 1, epoch_secs(20));
    assert_eq!(projected.recent_events.len(), 1);
    let event = &projected.recent_events[0];
    assert_eq!(event.kind, "layout_updated");
    assert_eq!(event.detail, "");
    assert_eq!(event.workspace_id, None);
    assert_eq!(event.tab_id, None);
    assert_eq!(event.pane_id, None);

    let json_text = serde_json::to_string(event).expect("序列化應成功");
    assert!(
        !json_text.contains("workspace_id")
            && !json_text.contains("tab_id")
            && !json_text.contains("pane_id"),
        "沒有主體的事件（Noted）不該序列化出 id 欄位"
    );
}

/// Fix round 1 finding 1／2：kind 集中定義、主體 id 每筆只填一個。對 workspace／tab／
/// pane（用 `AgentStatusChanged`，這是唯一 kind 帶 `pane.` 前綴的事件）／`Noted`／
/// `Drift` 各套一筆，逐筆用 `serde_json::Value` 整份比對，確認 `kind` 逐字正確、
/// 只有一個主體 id key、其餘 id key 完全不存在（`skip_serializing_if` 生效）。
#[test]
fn recent_event_json_shape_per_event_level() {
    let mut store = RuntimeStore::new();
    let win = runtime_id("win");
    store.register(win.clone(), "herdr".to_string(), "tcp://win".to_string());

    store
        .replace(
            &win,
            snapshot(
                vec![workspace("wJ", 1)],
                vec![tab("wJ:t1", "wJ", 1)],
                vec![pane("wJ:p1", "wJ", "wJ:t1")],
                vec![],
                Focused {
                    workspace_id: None,
                    tab_id: None,
                    pane_id: None,
                },
            ),
        )
        .expect("replace 應成功");

    // workspace 事件。
    store
        .apply(
            &win,
            RuntimeEvent::WorkspaceRelabeled {
                id: workspace_id("wJ"),
                label: "new-label".to_string(),
            },
            epoch_secs(1),
        )
        .expect("WorkspaceRelabeled 應成功");

    // tab 事件。
    store
        .apply(
            &win,
            RuntimeEvent::TabRelabeled {
                id: tab_id("wJ:t1"),
                label: "new-tab-label".to_string(),
            },
            epoch_secs(2),
        )
        .expect("TabRelabeled 應成功");

    // pane 事件：用 AgentStatusChanged，是唯一 kind 帶 `pane.` 前綴的事件。
    store
        .apply(
            &win,
            RuntimeEvent::AgentStatusChanged {
                pane_id: pane_id("wJ:p1"),
                status: AgentStatus::Working,
                title: None,
                agent: None,
            },
            epoch_secs(3),
        )
        .expect("AgentStatusChanged 應成功");

    // Noted：不改狀態，只進紀錄。
    store
        .apply(
            &win,
            RuntimeEvent::Noted {
                kind: "custom_kind".to_string(),
            },
            epoch_secs(4),
        )
        .expect("Noted 一律成功");

    // Drift：故意失敗，驗證失敗路徑的 kind／detail。
    store
        .apply(
            &win,
            RuntimeEvent::Drift {
                reason: "manual drift".to_string(),
            },
            epoch_secs(5),
        )
        .expect_err("Drift 事件一律回傳 Err");

    let projected = project(&store, &DomainState::default(), 1, epoch_secs(10));
    assert_eq!(projected.recent_events.len(), 5, "五筆事件各記一筆");

    // 最新在前：drift(5) → noted(4) → pane(3) → tab(2) → workspace(1)。
    let actual: Vec<serde_json::Value> = projected
        .recent_events
        .iter()
        .map(|event| serde_json::to_value(event).expect("序列化應成功"))
        .collect();

    let expected = vec![
        json!({
            "at": "1970-01-01T00:00:05Z",
            "runtime": "win",
            "kind": "drift",
            "detail": "manual drift"
        }),
        json!({
            "at": "1970-01-01T00:00:04Z",
            "runtime": "win",
            "kind": "custom_kind",
            "detail": ""
        }),
        json!({
            "at": "1970-01-01T00:00:03Z",
            "runtime": "win",
            "kind": "pane.agent_status_changed",
            "pane_id": "wJ:p1",
            "detail": "working"
        }),
        json!({
            "at": "1970-01-01T00:00:02Z",
            "runtime": "win",
            "kind": "tab_relabeled",
            "tab_id": "wJ:t1",
            "detail": "new-tab-label"
        }),
        json!({
            "at": "1970-01-01T00:00:01Z",
            "runtime": "win",
            "kind": "workspace_relabeled",
            "workspace_id": "wJ",
            "detail": "new-label"
        }),
    ];

    assert_eq!(actual, expected);
}

/// design D9：`ProjectedState::content_eq` 要忽略 `version` 與 `generated_at`，
/// 只比較 `runtimes`／`recent_events`（1.6 用它判斷是否要遞增 version、要不要廣播）。
/// 這個行為在 Task 1.5 原本的七個測試裡完全沒有測到——Fix round 1 的突變證據（M8：
/// 把 `content_eq` 改成也比較 `generated_at`）跑過全部既有測試都不會失敗，證實了這個
/// 缺口，所以在這裡補一個專門測試。
#[test]
fn content_eq_ignores_version_and_generated_at() {
    let mut store = RuntimeStore::new();
    let win = runtime_id("win");
    store.register(win.clone(), "herdr".to_string(), "tcp://win".to_string());
    store
        .replace(
            &win,
            snapshot(
                vec![workspace("wJ", 1)],
                vec![],
                vec![],
                vec![],
                Focused {
                    workspace_id: None,
                    tab_id: None,
                    pane_id: None,
                },
            ),
        )
        .expect("replace 應成功");

    let a = project(&store, &DomainState::default(), 1, epoch_secs(100));
    let b = project(&store, &DomainState::default(), 2, epoch_secs(200));
    assert_ne!(a.version, b.version);
    assert_ne!(a.generated_at, b.generated_at);
    assert!(
        a.content_eq(&b),
        "只有 version／generated_at 不同時，content_eq 應視為內容相同"
    );

    store
        .apply(
            &win,
            RuntimeEvent::WorkspaceRelabeled {
                id: workspace_id("wJ"),
                label: "changed".to_string(),
            },
            epoch_secs(300),
        )
        .expect("WorkspaceRelabeled 應成功");
    let c = project(&store, &DomainState::default(), 2, epoch_secs(200));

    assert!(
        !b.content_eq(&c),
        "recent_events 內容不同時，content_eq 應視為不同"
    );
}
