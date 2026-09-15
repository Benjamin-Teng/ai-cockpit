//! Task 2.1／2.2 驗收測試：HERDR `SessionSnapshot` → `cockpit-core` `RuntimeSnapshot` 的
//! snapshot 翻譯（spec `herdr-runtime-translation`「snapshot 翻譯」；design D15）；task 2.2
//! 續在本檔加事件翻譯（`herdr-runtime-translation`「事件翻譯對照」；design §7.2、D3、D11）。

use std::collections::BTreeSet;
use std::time::SystemTime;

use cockpit_core::{AgentStatus, PaneId, RuntimeEvent, TabId, WorkspaceId};
use herdr_client::client::IncomingEvent;
use herdr_client::types::{EventKind, SessionSnapshot, SubscriptionEventKind};
use serde_json::{Value, json};

const P22: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../herdr-client/tests/fixtures/snapshot-p22.json"
);
const P20: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../herdr-client/tests/fixtures/snapshot-p20.json"
);

/// 讀取整個回應信封（`{"id":..,"result":{"type":"session_snapshot","snapshot":{...}}}`），
/// 取出 `result.snapshot` 解析成 `SessionSnapshot`。
fn load_snapshot(path: &str) -> SessionSnapshot {
    let raw = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("failed to read fixture {path}: {e}"));
    let envelope: serde_json::Value = serde_json::from_str(&raw)
        .unwrap_or_else(|e| panic!("fixture {path} is not valid JSON: {e}"));
    let snapshot_value = envelope["result"]["snapshot"].clone();
    serde_json::from_value(snapshot_value)
        .unwrap_or_else(|e| panic!("fixture {path}: snapshot does not match SessionSnapshot: {e}"))
}

/// 對一份 fixture 斷言：workspaces／tabs／panes 的筆數與 id 集合和來源相同、agents 筆數等於
/// 來源中 `agent` 非 `None` 的筆數、`protocol`／`server_version` 對應、每個 pane `exited ==
/// false` 且 `updated_at == now`、`focused` 三欄對應來源的 `focused_*_id`。
fn assert_translation_matches_source(src: &SessionSnapshot, expected_protocol: u32) {
    let now = SystemTime::now();
    let out = cockpit_herdr::translate::snapshot(src, now);

    assert_eq!(out.protocol, expected_protocol);
    assert_eq!(out.server_version, src.version);

    let src_workspace_ids: BTreeSet<String> = src
        .workspaces
        .iter()
        .map(|w| w.workspace_id.clone())
        .collect();
    let out_workspace_ids: BTreeSet<String> = out
        .workspaces
        .iter()
        .map(|w| w.id.as_str().to_string())
        .collect();
    assert_eq!(out.workspaces.len(), src.workspaces.len());
    assert_eq!(out_workspace_ids, src_workspace_ids);

    let src_tab_ids: BTreeSet<String> = src.tabs.iter().map(|t| t.tab_id.clone()).collect();
    let out_tab_ids: BTreeSet<String> =
        out.tabs.iter().map(|t| t.id.as_str().to_string()).collect();
    assert_eq!(out.tabs.len(), src.tabs.len());
    assert_eq!(out_tab_ids, src_tab_ids);

    let src_pane_ids: BTreeSet<String> = src.panes.iter().map(|p| p.pane_id.clone()).collect();
    let out_pane_ids: BTreeSet<String> = out
        .panes
        .iter()
        .map(|p| p.id.as_str().to_string())
        .collect();
    assert_eq!(out.panes.len(), src.panes.len());
    assert_eq!(out_pane_ids, src_pane_ids);

    let expected_agent_count = src.agents.iter().filter(|a| a.agent.is_some()).count();
    assert_eq!(out.agents.len(), expected_agent_count);

    for pane in &out.panes {
        assert!(!pane.exited, "pane {:?} should not be exited", pane.id);
        assert_eq!(
            pane.updated_at, now,
            "pane {:?} updated_at mismatch",
            pane.id
        );
    }

    assert_eq!(
        out.focused.workspace_id,
        src.focused_workspace_id.clone().map(WorkspaceId::new)
    );
    assert_eq!(
        out.focused.tab_id,
        src.focused_tab_id.clone().map(TabId::new)
    );
    assert_eq!(
        out.focused.pane_id,
        src.focused_pane_id.clone().map(PaneId::new)
    );
}

#[test]
fn snapshot_p22_fixture_translates_with_same_counts_and_ids() {
    let src = load_snapshot(P22);
    assert_eq!(src.protocol, 22);
    assert_translation_matches_source(&src, 22);
}

#[test]
fn snapshot_p20_fixture_translates() {
    let src = load_snapshot(P20);
    assert_eq!(src.protocol, 20);
    assert_translation_matches_source(&src, 20);
}

#[test]
fn unknown_agent_status_string_maps_to_unknown() {
    let raw = serde_json::json!({
        "version": "0.0.0-test",
        "protocol": 99,
        "focused_workspace_id": "w1",
        "focused_tab_id": "w1:t1",
        "focused_pane_id": "w1:p1",
        "workspaces": [
            {
                "workspace_id": "w1",
                "label": "LABEL",
                "number": 1,
                "active_tab_id": "w1:t1",
                "agent_status": "idle",
                "focused": true,
                "pane_count": 1,
                "tab_count": 1
            }
        ],
        "tabs": [
            {
                "tab_id": "w1:t1",
                "workspace_id": "w1",
                "number": 1,
                "label": "LABEL",
                "agent_status": "idle",
                "focused": true,
                "pane_count": 1
            }
        ],
        "panes": [
            {
                "pane_id": "w1:p1",
                "workspace_id": "w1",
                "tab_id": "w1:t1",
                "agent_status": "teleporting",
                "focused": true,
                "revision": 1
            }
        ],
        "agents": [],
        "layouts": []
    });

    let src: SessionSnapshot = serde_json::from_value(raw)
        .unwrap_or_else(|e| panic!("minimal snapshot should parse: {e}"));

    let now = SystemTime::now();
    let out = cockpit_herdr::translate::snapshot(&src, now);

    let pane = out
        .panes
        .iter()
        .find(|p| p.id == PaneId::new("w1:p1"))
        .expect("pane w1:p1 should be present");
    assert_eq!(pane.agent_status, AgentStatus::Unknown);
    assert!(!pane.exited);
    assert_eq!(pane.updated_at, now);
    assert_eq!(pane.workspace_id, WorkspaceId::new("w1"));
    assert_eq!(pane.tab_id, TabId::new("w1:t1"));
}

// ---------------------------------------------------------------------------
// Task 2.2：事件翻譯（`translate::event`）驗收測試。
// ---------------------------------------------------------------------------

const EVENTS_LIFECYCLE_P22: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../herdr-client/tests/fixtures/events-lifecycle-p22.ndjson"
);
const EVENTS_LIFECYCLE_P20: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../herdr-client/tests/fixtures/events-lifecycle-p20.ndjson"
);
const EVENTS_STATUS_P22: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../herdr-client/tests/fixtures/events-status-p22.ndjson"
);
const EVENTS_STATUS_P20: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../herdr-client/tests/fixtures/events-status-p20.ndjson"
);

/// 依事件名稱字串建構 [`IncomingEvent`]：含 `.` 的名稱先試
/// [`SubscriptionEventKind`]（建 `PerPane`），否則試 [`EventKind`]（建 `Lifecycle`），兩者都
/// 解析失敗就落到 `Unknown`（同 `herdr_client::client::subscribe::classify` 的分類語意，只是
/// 測試端自己重建一份，因為生產程式碼的 `classify`／`parse_event_line` 是 crate-private）。
fn incoming(event: &str, data: Value) -> IncomingEvent {
    if event.contains('.') {
        if let Ok(kind) = serde_json::from_value::<SubscriptionEventKind>(json!(event)) {
            return IncomingEvent::PerPane(kind, data);
        }
    } else if let Ok(kind) = serde_json::from_value::<EventKind>(json!(event)) {
        return IncomingEvent::Lifecycle(kind, data);
    }
    IncomingEvent::Unknown {
        event: event.to_string(),
        data,
    }
}

/// 把 `translate::event()` 的回傳值化簡成一個種類字串，方便 table-driven 測試斷言「翻成了
/// 哪一種 `RuntimeEvent`」而不必逐欄核對內容。
fn kind_name(event: &Option<RuntimeEvent>) -> &'static str {
    match event {
        None => "None",
        Some(RuntimeEvent::WorkspaceUpserted(_)) => "WorkspaceUpserted",
        Some(RuntimeEvent::WorkspacesReplaced(_)) => "WorkspacesReplaced",
        Some(RuntimeEvent::WorkspaceRemoved(_)) => "WorkspaceRemoved",
        Some(RuntimeEvent::WorkspaceRelabeled { .. }) => "WorkspaceRelabeled",
        Some(RuntimeEvent::TabUpserted(_)) => "TabUpserted",
        Some(RuntimeEvent::TabsReplaced { .. }) => "TabsReplaced",
        Some(RuntimeEvent::TabRemoved(_)) => "TabRemoved",
        Some(RuntimeEvent::TabRelabeled { .. }) => "TabRelabeled",
        Some(RuntimeEvent::PaneUpserted(_)) => "PaneUpserted",
        Some(RuntimeEvent::PaneMoved { .. }) => "PaneMoved",
        Some(RuntimeEvent::PaneRemoved(_)) => "PaneRemoved",
        Some(RuntimeEvent::PaneExited(_)) => "PaneExited",
        Some(RuntimeEvent::AgentDetected { .. }) => "AgentDetected",
        Some(RuntimeEvent::AgentStatusChanged { .. }) => "AgentStatusChanged",
        Some(RuntimeEvent::FocusChanged(_)) => "FocusChanged",
        Some(RuntimeEvent::Drift { .. }) => "Drift",
        Some(RuntimeEvent::Noted { .. }) => "Noted",
    }
}

/// 對照表：事件名稱字串 → 預期的 `kind_name()`（`"None"` 代表不產生事件）。涵蓋全部 26 種
/// `EventKind` ＋ 3 種 `SubscriptionEventKind`，供 table-driven 測試與 fixture 測試共用。
fn expected_kind_for_event_name(event: &str) -> &'static str {
    match event {
        "workspace_created" | "workspace_updated" | "workspace_metadata_updated" => {
            "WorkspaceUpserted"
        }
        "workspace_closed" => "WorkspaceRemoved",
        "workspace_renamed" => "WorkspaceRelabeled",
        "workspace_moved" | "workspace_reordered" => "WorkspacesReplaced",
        "workspace_focused" => "FocusChanged",
        "worktree_created" | "worktree_opened" | "worktree_removed" => "Noted",
        "tab_created" => "TabUpserted",
        "tab_closed" => "TabRemoved",
        "tab_renamed" => "TabRelabeled",
        "tab_moved" => "TabsReplaced",
        "tab_focused" => "FocusChanged",
        "pane_created" | "pane_updated" => "PaneUpserted",
        "pane_closed" => "PaneRemoved",
        "pane_focused" => "FocusChanged",
        "pane_moved" => "PaneMoved",
        "pane_output_changed" => "None",
        "pane_exited" => "PaneExited",
        "pane_agent_detected" => "AgentDetected",
        "pane_agent_status_changed" => "None",
        "layout_updated" => "Noted",
        "pane.output_matched" => "None",
        "pane.agent_status_changed" => "AgentStatusChanged",
        "pane.scroll_changed" => "None",
        other => panic!("expected_kind_for_event_name: 沒有這個事件名稱的對照：{other}"),
    }
}

#[test]
fn every_event_kind_maps_per_table() {
    let now = SystemTime::now();

    let workspace_info = json!({
        "workspace_id": "w1", "label": "L", "number": 1, "active_tab_id": "w1:t1",
        "agent_status": "idle", "focused": false, "pane_count": 1, "tab_count": 1
    });
    let tab_info = json!({
        "tab_id": "w1:t1", "workspace_id": "w1", "number": 1, "label": "L",
        "agent_status": "idle", "focused": false, "pane_count": 1
    });
    let pane_info = json!({
        "pane_id": "w1:p1", "workspace_id": "w1", "tab_id": "w1:t1",
        "agent_status": "idle", "focused": false, "revision": 1
    });

    let cases: Vec<(&str, Value)> = vec![
        ("workspace_created", json!({"workspace": workspace_info})),
        ("workspace_updated", json!({"workspace": workspace_info})),
        (
            "workspace_metadata_updated",
            json!({"workspace": workspace_info}),
        ),
        ("workspace_closed", json!({"workspace_id": "w1"})),
        (
            "workspace_renamed",
            json!({"workspace_id": "w1", "label": "NEW"}),
        ),
        ("workspace_moved", json!({"workspaces": []})),
        ("workspace_reordered", json!({"workspaces": []})),
        ("workspace_focused", json!({"workspace_id": "w1"})),
        ("worktree_created", json!({})),
        ("worktree_opened", json!({})),
        ("worktree_removed", json!({})),
        ("tab_created", json!({"tab": tab_info})),
        (
            "tab_closed",
            json!({"tab_id": "w1:t1", "workspace_id": "w1"}),
        ),
        (
            "tab_renamed",
            json!({"tab_id": "w1:t1", "workspace_id": "w1", "label": "NEW"}),
        ),
        (
            "tab_moved",
            json!({"tab_id": "w1:t1", "workspace_id": "w1", "insert_index": 0, "tabs": []}),
        ),
        (
            "tab_focused",
            json!({"tab_id": "w1:t1", "workspace_id": "w1"}),
        ),
        ("pane_created", json!({"pane": pane_info})),
        (
            "pane_closed",
            json!({"pane_id": "w1:p1", "workspace_id": "w1"}),
        ),
        ("pane_updated", json!({"pane": pane_info})),
        (
            "pane_focused",
            json!({"pane_id": "w1:p1", "workspace_id": "w1"}),
        ),
        (
            "pane_moved",
            json!({
                "previous_pane_id": "w1:p0",
                "previous_workspace_id": "w1",
                "previous_tab_id": "w1:t1",
                "pane": pane_info,
            }),
        ),
        ("pane_output_changed", json!({})),
        (
            "pane_exited",
            json!({"pane_id": "w1:p1", "workspace_id": "w1"}),
        ),
        (
            "pane_agent_detected",
            json!({"pane_id": "w1:p1", "workspace_id": "w1"}),
        ),
        ("pane_agent_status_changed", json!({})),
        ("layout_updated", json!({})),
        ("pane.output_matched", json!({})),
        (
            "pane.agent_status_changed",
            json!({"pane_id": "w1:p1", "workspace_id": "w1", "agent_status": "idle"}),
        ),
        ("pane.scroll_changed", json!({})),
    ];

    assert_eq!(
        cases.len(),
        29,
        "table 必須涵蓋 26 種 EventKind ＋ 3 種 SubscriptionEventKind"
    );

    for (name, data) in cases {
        let src = incoming(name, data.clone());
        let result = cockpit_herdr::translate::event(&src, now);
        let expected = expected_kind_for_event_name(name);
        assert_eq!(
            kind_name(&result),
            expected,
            "event {name} 預期種類 {expected}，實際 {result:?}（data={data})"
        );
    }
}

#[test]
fn every_line_of_four_event_fixtures_translates_without_drift() {
    let now = SystemTime::now();
    let fixtures = [
        EVENTS_LIFECYCLE_P22,
        EVENTS_LIFECYCLE_P20,
        EVENTS_STATUS_P22,
        EVENTS_STATUS_P20,
    ];
    let mut checked = 0usize;

    for path in fixtures {
        let raw = std::fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("failed to read fixture {path}: {e}"));
        for (line_no, line) in raw.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let value: Value = serde_json::from_str(line)
                .unwrap_or_else(|e| panic!("{path}:{}: not valid JSON: {e}", line_no + 1));
            let event_name = value["event"]
                .as_str()
                .unwrap_or_else(|| panic!("{path}:{}: missing \"event\" field", line_no + 1))
                .to_string();
            let data = value["data"].clone();

            let src = incoming(&event_name, data);
            let result = cockpit_herdr::translate::event(&src, now);

            assert!(
                !matches!(result, Some(RuntimeEvent::Drift { .. })),
                "{path}:{}: event {event_name} 產生了 Drift：{result:?}",
                line_no + 1
            );

            let expected = expected_kind_for_event_name(&event_name);
            assert_eq!(
                kind_name(&result),
                expected,
                "{path}:{}: event {event_name} 預期種類 {expected}，實際 {result:?}",
                line_no + 1
            );
            checked += 1;
        }
    }

    assert!(checked > 0, "四個 fixture 應該至少讀到一行");
}

#[test]
fn pane_moved_keeps_previous_and_new_ids() {
    let now = SystemTime::now();
    let pane_info = json!({
        "pane_id": "wK:p7", "workspace_id": "wK", "tab_id": "wK:t1",
        "agent_status": "idle", "focused": false, "revision": 1
    });
    let data = json!({
        "previous_pane_id": "wJ:p1",
        "previous_workspace_id": "wJ",
        "previous_tab_id": "wJ:t1",
        "pane": pane_info,
    });

    let src = incoming("pane_moved", data);
    let result = cockpit_herdr::translate::event(&src, now).expect("pane_moved 應翻成事件");

    match result {
        RuntimeEvent::PaneMoved { previous, pane } => {
            assert_eq!(previous, PaneId::new("wJ:p1"), "應保留搬移前的舊 pane id");
            assert_eq!(pane.id, PaneId::new("wK:p7"), "應是搬移後的新 pane id");
        }
        other => panic!("expected PaneMoved, got {other:?}"),
    }
}

#[test]
fn pane_agent_detected_released_clears_agent() {
    let now = SystemTime::now();
    let data = json!({
        "pane_id": "w1:p1",
        "workspace_id": "w1",
        "agent": "claude",
        "released": true,
    });

    let src = incoming("pane_agent_detected", data);
    let result = cockpit_herdr::translate::event(&src, now).expect("應翻成事件");

    match result {
        RuntimeEvent::AgentDetected { pane_id, agent } => {
            assert_eq!(pane_id, PaneId::new("w1:p1"));
            assert_eq!(agent, None, "released 為 true 時 agent 應清空");
        }
        other => panic!("expected AgentDetected, got {other:?}"),
    }
}

#[test]
fn agent_status_changed_done_maps_to_done_only() {
    let now = SystemTime::now();
    let data = json!({
        "pane_id": "w1:p1",
        "workspace_id": "w1",
        "agent_status": "done",
        "title": "x",
    });

    let src = incoming("pane.agent_status_changed", data);
    let result = cockpit_herdr::translate::event(&src, now).expect("應翻成事件");

    // AgentStatus::Done 只代表「已 idle 且尚未被看過」，不是任務完成（herdr-client
    // agent_status.rs 的文件註解）；`RuntimeEvent::AgentStatusChanged` 只有這四個欄位
    // （pane_id/status/title/agent），沒有任何額外的「完成」語意欄位——下面窮舉解構就是這份
    // 佐證：多一個欄位這裡就編譯不過，少一個欄位這裡也編譯不過。
    match result {
        RuntimeEvent::AgentStatusChanged {
            pane_id,
            status,
            title,
            agent,
        } => {
            assert_eq!(pane_id, PaneId::new("w1:p1"));
            assert_eq!(status, AgentStatus::Done);
            assert_eq!(title, Some("x".to_string()));
            assert_eq!(agent, None);
        }
        other => panic!("expected AgentStatusChanged, got {other:?}"),
    }
}

#[test]
fn missing_payload_field_is_drift_with_event_name() {
    let now = SystemTime::now();
    let src = incoming("pane_created", json!({}));
    let result =
        cockpit_herdr::translate::event(&src, now).expect("payload 解析失敗仍應是 Some(Drift)");

    match result {
        RuntimeEvent::Drift { reason } => {
            assert!(
                reason.contains("pane_created"),
                "reason 應含事件名稱，得到：{reason}"
            );
        }
        other => panic!("expected Drift, got {other:?}"),
    }
}

#[test]
fn unknown_event_name_yields_none() {
    let now = SystemTime::now();
    let src = incoming("pane_teleported", json!({}));
    let result = cockpit_herdr::translate::event(&src, now);
    assert_eq!(result, None);
}

#[test]
fn focused_events_are_partial() {
    let now = SystemTime::now();

    let workspace_focused = incoming("workspace_focused", json!({"workspace_id": "w1"}));
    match cockpit_herdr::translate::event(&workspace_focused, now).expect("應翻成事件") {
        RuntimeEvent::FocusChanged(change) => {
            assert_eq!(change.workspace_id, Some(WorkspaceId::new("w1")));
            assert_eq!(change.tab_id, None);
            assert_eq!(change.pane_id, None);
        }
        other => panic!("expected FocusChanged, got {other:?}"),
    }

    let tab_focused = incoming(
        "tab_focused",
        json!({"tab_id": "w1:t1", "workspace_id": "w1"}),
    );
    match cockpit_herdr::translate::event(&tab_focused, now).expect("應翻成事件") {
        RuntimeEvent::FocusChanged(change) => {
            assert_eq!(change.workspace_id, None);
            assert_eq!(change.tab_id, Some(TabId::new("w1:t1")));
            assert_eq!(change.pane_id, None);
        }
        other => panic!("expected FocusChanged, got {other:?}"),
    }

    let pane_focused = incoming(
        "pane_focused",
        json!({"pane_id": "w1:p1", "workspace_id": "w1"}),
    );
    match cockpit_herdr::translate::event(&pane_focused, now).expect("應翻成事件") {
        RuntimeEvent::FocusChanged(change) => {
            assert_eq!(change.workspace_id, None);
            assert_eq!(change.tab_id, None);
            assert_eq!(change.pane_id, Some(PaneId::new("w1:p1")));
        }
        other => panic!("expected FocusChanged, got {other:?}"),
    }
}
