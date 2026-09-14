//! Task 3.2 驗收測試：事件與訂閱型別（spec `herdr-event-subscription`）。
//!
//! fix round 1 / finding 3：`assert_known_lifecycle_event` 原本解析完就把結果丟掉，
//! 搭配 payload 型別上的 `#[serde(default)]`，Rust 欄位名拼錯或漏映射一樣能「解析成功」
//! 只是值不對，測試抓不到。真機 fixture 只出現 9 種事件名稱，`workspace_moved`、
//! `workspace_reordered`、`tab_renamed`、`tab_moved`、`pane_closed`、`pane_exited` 等分支
//! 完全沒被跑到，所以另外手寫「先過 schema `event`／`subscription_event` 根驗證、再逐欄
//! 斷言」的最小案例補上。
//!
//! fix round 2 / finding 3：改用 `common::assert_modeled_fields_match`——把解析結果重新
//! 序列化回 JSON，遞迴比對它每一個已建模欄位（含巢狀物件、陣列逐元素）與原始 JSON 的值，
//! 取代先前只挑 1–2 個欄位手寫 `assert_eq!` 的做法（那種寫法在 `pane_moved` 這種多欄位
//! payload 上尤其漏得多：只核對過 `previous_pane_id` 與新 `pane.pane_id`，
//! `previous_workspace_id`、`previous_tab_id`、`pane` 其餘欄位完全沒比對）。也為
//! `display_agent`、`title`、`state_labels` 各加一個「有值」案例（原本完全沒有這三個欄位
//! 有值的測試資料）。

use std::str::FromStr;

use herdr_client::types::{
    AgentStatus, EventEnvelope, EventKind, EventsSubscribeParams, PaneAgentDetectedPayload,
    PaneAgentStatusChangedPayload, PaneClosedPayload, PaneFocusedPayload, PaneMovedPayload,
    PanePayload, Subscription, SubscriptionEventKind, TabClosedPayload, TabFocusedPayload,
    TabMovedPayload, TabPayload, TabRenamedPayload, WorkspaceClosedPayload,
    WorkspaceFocusedPayload, WorkspacePayload, WorkspaceRenamedPayload, WorkspacesReplacedPayload,
};

mod common;
use common::{
    assert_modeled_fields_match, assert_required_fields_detected,
    assert_required_fields_detected_for_event_variant, assert_valid_as_event,
    assert_valid_as_subscription_event, fixture_path, read_lines,
};

/// 全分支最終 review finding 1：跟 `tests/types.rs` 的 `SCHEMA_FILES` 同樣的表格化前提——
/// p22、p20 各跑一次。
const SCHEMA_FILES: &[&str] = &["schema-p22.json", "schema-p20.json"];

// ---------------------------------------------------------------------------
// Fix round 3 / finding 3：`assert_modeled_fields_match` 現在預設「raw 缺這個 key 就失敗」，
// 只有列在 allow-list 裡的裸欄位名稱才放行——呼叫端要自己寫「這個型別真正選填的欄位叫
// 什麼名字」，不是從 parsed 的值反推，這樣才抓得到 serde rename 打錯字（見
// `assert_modeled_fields_match_catches_wrong_serde_rename`）。下面幾個常數對應各 payload
// 型別實際選填的欄位；欄位全部必填的型別（`WorkspaceInfo`、`TabInfo`
// 及包著它們、自己也沒有選填欄位的 payload）直接傳 `&[]`。
// ---------------------------------------------------------------------------

const PANE_INFO_OPTIONAL_FIELDS: &[&str] = &["agent", "title", "terminal_title", "cwd", "label"];
const PANE_MOVED_OPTIONAL_FIELDS: &[&str] = &[
    "created_workspace",
    "created_tab",
    "closed_workspace_id",
    "closed_tab_id",
    "agent",
    "title",
    "terminal_title",
    "cwd",
    "label",
];
const PANE_AGENT_DETECTED_OPTIONAL_FIELDS: &[&str] = &["agent", "released", "final_status"];
const PANE_AGENT_STATUS_CHANGED_OPTIONAL_FIELDS: &[&str] =
    &["agent", "display_agent", "title", "state_labels"];

fn schema_event_kinds() -> Vec<String> {
    let raw = std::fs::read_to_string(fixture_path("schema-p22.json")).expect("schema fixture");
    let doc: serde_json::Value = serde_json::from_str(&raw).unwrap();
    doc.pointer("/schemas/event/$defs/EventKind/enum")
        .and_then(serde_json::Value::as_array)
        .expect("EventKind enum in schema")
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect()
}

#[test]
fn subscription_serializes_dotted_type_only() {
    for sub in Subscription::all_lifecycle() {
        let value = serde_json::to_value(&sub).unwrap();
        let obj = value
            .as_object()
            .expect("subscription serializes to an object");
        assert_eq!(
            obj.len(),
            1,
            "{sub:?} must serialize to only a \"type\" field"
        );
        let ty = obj.get("type").and_then(serde_json::Value::as_str).unwrap();
        assert!(ty.contains('.'), "{ty} must be dotted");
    }
    assert_eq!(Subscription::all_lifecycle().len(), 24);
}

#[test]
fn per_pane_subscription_has_pane_id_and_no_filter() {
    let sub = Subscription::PaneAgentStatusChanged {
        pane_id: "wJ:p1".to_string(),
    };
    let value = serde_json::to_value(&sub).unwrap();
    let obj = value.as_object().unwrap();
    assert_eq!(
        obj.get("type").and_then(serde_json::Value::as_str),
        Some("pane.agent_status_changed")
    );
    assert_eq!(
        obj.get("pane_id").and_then(serde_json::Value::as_str),
        Some("wJ:p1")
    );
    assert!(
        !obj.contains_key("agent_status"),
        "per-pane subscription must not carry an agent_status filter"
    );
    assert_eq!(obj.len(), 2, "only \"type\" and \"pane_id\" expected");
}

#[test]
fn event_kind_roundtrips_all_26() {
    let schema_kinds = schema_event_kinds();
    assert_eq!(schema_kinds.len(), 26);

    let all = [
        EventKind::WorkspaceCreated,
        EventKind::WorkspaceUpdated,
        EventKind::WorkspaceMetadataUpdated,
        EventKind::WorkspaceClosed,
        EventKind::WorkspaceRenamed,
        EventKind::WorkspaceMoved,
        EventKind::WorkspaceReordered,
        EventKind::WorkspaceFocused,
        EventKind::WorktreeCreated,
        EventKind::WorktreeOpened,
        EventKind::WorktreeRemoved,
        EventKind::TabCreated,
        EventKind::TabClosed,
        EventKind::TabRenamed,
        EventKind::TabMoved,
        EventKind::TabFocused,
        EventKind::PaneCreated,
        EventKind::PaneClosed,
        EventKind::PaneUpdated,
        EventKind::PaneFocused,
        EventKind::PaneMoved,
        EventKind::PaneOutputChanged,
        EventKind::PaneExited,
        EventKind::PaneAgentDetected,
        EventKind::PaneAgentStatusChanged,
        EventKind::LayoutUpdated,
    ];
    assert_eq!(all.len(), 26);

    let mut as_str_values: Vec<&str> = all.iter().map(|k| k.as_str()).collect();
    as_str_values.sort_unstable();
    let mut schema_values: Vec<&str> = schema_kinds.iter().map(String::as_str).collect();
    schema_values.sort_unstable();
    assert_eq!(as_str_values, schema_values);

    for kind in all {
        let round_tripped = EventKind::from_str(kind.as_str()).unwrap();
        assert_eq!(round_tripped, kind);
    }

    assert!(EventKind::from_str("pane_teleported").is_err());
}

/// 依 `event` 選對應的 payload 型別解析 `data`，再用 `assert_modeled_fields_match` 遞迴
/// 比對解析結果重新序列化後的每一個已建模欄位與原始 `data` 的值（含巢狀物件、陣列逐
/// 元素）——不是解析完就丟掉，也不是只挑 1–2 個欄位手寫比對。沒有型別的
/// （`worktree_*`、`layout_updated`、`pane_output_changed`）只確認 `EventKind::from_str`
/// 成功。
fn assert_known_lifecycle_event(event: &str, data: &serde_json::Value) {
    match event {
        "workspace_created" | "workspace_updated" | "workspace_metadata_updated" => {
            let payload = serde_json::from_value::<WorkspacePayload>(data.clone())
                .unwrap_or_else(|e| panic!("{event}: {e}"));
            assert_modeled_fields_match(data, &payload, &[], event);
        }
        "workspace_closed" => {
            let payload = serde_json::from_value::<WorkspaceClosedPayload>(data.clone())
                .unwrap_or_else(|e| panic!("{event}: {e}"));
            assert_modeled_fields_match(data, &payload, &["workspace"], event);
        }
        "workspace_renamed" => {
            let payload = serde_json::from_value::<WorkspaceRenamedPayload>(data.clone())
                .unwrap_or_else(|e| panic!("{event}: {e}"));
            assert_modeled_fields_match(data, &payload, &[], event);
        }
        "workspace_moved" | "workspace_reordered" => {
            let payload = serde_json::from_value::<WorkspacesReplacedPayload>(data.clone())
                .unwrap_or_else(|e| panic!("{event}: {e}"));
            assert_modeled_fields_match(data, &payload, &[], event);
        }
        "workspace_focused" => {
            let payload = serde_json::from_value::<WorkspaceFocusedPayload>(data.clone())
                .unwrap_or_else(|e| panic!("{event}: {e}"));
            assert_modeled_fields_match(data, &payload, &[], event);
        }
        "tab_created" => {
            let payload = serde_json::from_value::<TabPayload>(data.clone())
                .unwrap_or_else(|e| panic!("{event}: {e}"));
            assert_modeled_fields_match(data, &payload, &[], event);
        }
        "tab_closed" => {
            let payload = serde_json::from_value::<TabClosedPayload>(data.clone())
                .unwrap_or_else(|e| panic!("{event}: {e}"));
            assert_modeled_fields_match(data, &payload, &[], event);
        }
        "tab_renamed" => {
            let payload = serde_json::from_value::<TabRenamedPayload>(data.clone())
                .unwrap_or_else(|e| panic!("{event}: {e}"));
            assert_modeled_fields_match(data, &payload, &[], event);
        }
        "tab_moved" => {
            let payload = serde_json::from_value::<TabMovedPayload>(data.clone())
                .unwrap_or_else(|e| panic!("{event}: {e}"));
            assert_modeled_fields_match(data, &payload, &[], event);
        }
        "tab_focused" => {
            let payload = serde_json::from_value::<TabFocusedPayload>(data.clone())
                .unwrap_or_else(|e| panic!("{event}: {e}"));
            assert_modeled_fields_match(data, &payload, &[], event);
        }
        "pane_created" | "pane_updated" => {
            let payload = serde_json::from_value::<PanePayload>(data.clone())
                .unwrap_or_else(|e| panic!("{event}: {e}"));
            assert_modeled_fields_match(data, &payload, PANE_INFO_OPTIONAL_FIELDS, event);
        }
        "pane_closed" | "pane_exited" => {
            let payload = serde_json::from_value::<PaneClosedPayload>(data.clone())
                .unwrap_or_else(|e| panic!("{event}: {e}"));
            assert_modeled_fields_match(data, &payload, &[], event);
        }
        "pane_focused" => {
            let payload = serde_json::from_value::<PaneFocusedPayload>(data.clone())
                .unwrap_or_else(|e| panic!("{event}: {e}"));
            assert_modeled_fields_match(data, &payload, &[], event);
        }
        "pane_moved" => {
            let payload = serde_json::from_value::<PaneMovedPayload>(data.clone())
                .unwrap_or_else(|e| panic!("{event}: {e}"));
            assert_modeled_fields_match(data, &payload, PANE_MOVED_OPTIONAL_FIELDS, event);
        }
        "pane_agent_detected" => {
            let payload = serde_json::from_value::<PaneAgentDetectedPayload>(data.clone())
                .unwrap_or_else(|e| panic!("{event}: {e}"));
            assert_modeled_fields_match(data, &payload, PANE_AGENT_DETECTED_OPTIONAL_FIELDS, event);
        }
        "pane_agent_status_changed" => {
            let payload = serde_json::from_value::<PaneAgentStatusChangedPayload>(data.clone())
                .unwrap_or_else(|e| panic!("{event}: {e}"));
            assert_modeled_fields_match(
                data,
                &payload,
                PANE_AGENT_STATUS_CHANGED_OPTIONAL_FIELDS,
                event,
            );
        }
        // 不提供型別的事件：只確認名稱在 26 種已知集合內。
        "worktree_created"
        | "worktree_opened"
        | "worktree_removed"
        | "layout_updated"
        | "pane_output_changed" => {
            EventKind::from_str(event)
                .unwrap_or_else(|_| panic!("{event} should be a known EventKind"));
        }
        other => panic!("unexpected lifecycle event name in fixture: {other}"),
    }
}

#[test]
fn event_fixtures_parse_with_matching_payload_types() {
    let mut lifecycle_events_seen = 0usize;
    for line in read_lines("events-lifecycle-p20.ndjson") {
        let envelope: EventEnvelope = serde_json::from_str(&line)
            .unwrap_or_else(|e| panic!("bad line in events-lifecycle-p20.ndjson: {e}\n{line}"));
        assert!(
            EventKind::from_str(&envelope.event).is_ok(),
            "{} is not one of the 26 known EventKind names",
            envelope.event
        );
        assert_known_lifecycle_event(&envelope.event, &envelope.data);
        lifecycle_events_seen += 1;
    }
    assert!(lifecycle_events_seen > 0, "fixture must not be empty");

    let mut status_events_seen = 0usize;
    for line in read_lines("events-status-p20.ndjson") {
        let envelope: EventEnvelope = serde_json::from_str(&line)
            .unwrap_or_else(|e| panic!("bad line in events-status-p20.ndjson: {e}\n{line}"));
        assert_eq!(
            SubscriptionEventKind::from_str(&envelope.event),
            Ok(SubscriptionEventKind::PaneAgentStatusChanged)
        );
        let payload =
            serde_json::from_value::<PaneAgentStatusChangedPayload>(envelope.data.clone())
                .unwrap_or_else(|e| panic!("pane.agent_status_changed payload: {e}"));
        // fix round 2 / finding 3：原本只比較 `agent.is_some()`，沒比真值；現在遞迴比對
        // 全部已建模欄位（`pane_id`、`workspace_id`、`agent_status` 的真值、`agent` 有值時
        // 的真值）。events-status-p20.ndjson 最後一行沒有 "agent" 鍵（真機實測，見 fixture
        // 本身）——`agent` 在 allow-list 裡，這種缺席算合法。
        assert_modeled_fields_match(
            &envelope.data,
            &payload,
            PANE_AGENT_STATUS_CHANGED_OPTIONAL_FIELDS,
            "pane.agent_status_changed",
        );
        status_events_seen += 1;
    }
    assert!(status_events_seen > 0, "fixture must not be empty");
}

// ---------------------------------------------------------------------------
// task 5.1：p22（Windows 端真機，`herdr-client/examples/capture_events.rs` 擷取）版本的事件
// fixture。
//
// fix round 1 finding 2（Codex task review，medium）：lifecycle 那份已經交付、進了 repo，
// 原本用 `Path::exists` 悄悄跳過，缺檔（漏加、誤刪、沒打包）時 CI 仍然全綠——改成無條件
// `read_lines`，缺檔就直接讓測試失敗。`events-status-p22.ndjson`（2 行
// `pane.agent_status_changed`：`wJ:p1` 從 `done` 變 `working`）在 fix round 1 時還沒擷取到
// 內容，先用 `#[ignore]` 佔位；fix round 2 擷取到之後改成同樣無條件執行。
// ---------------------------------------------------------------------------

#[test]
fn p22_lifecycle_event_fixture_parses_with_matching_payload_types() {
    let mut lifecycle_events_seen = 0usize;
    for line in read_lines("events-lifecycle-p22.ndjson") {
        let envelope: EventEnvelope = serde_json::from_str(&line)
            .unwrap_or_else(|e| panic!("bad line in events-lifecycle-p22.ndjson: {e}\n{line}"));
        assert!(
            EventKind::from_str(&envelope.event).is_ok(),
            "{} is not one of the 26 known EventKind names",
            envelope.event
        );
        assert_known_lifecycle_event(&envelope.event, &envelope.data);
        lifecycle_events_seen += 1;
    }
    assert!(lifecycle_events_seen > 0, "fixture must not be empty");
}

#[test]
fn p22_status_event_fixture_parses_with_matching_payload_type() {
    let mut status_events_seen = 0usize;
    for line in read_lines("events-status-p22.ndjson") {
        let envelope: EventEnvelope = serde_json::from_str(&line)
            .unwrap_or_else(|e| panic!("bad line in events-status-p22.ndjson: {e}\n{line}"));
        assert_eq!(
            SubscriptionEventKind::from_str(&envelope.event),
            Ok(SubscriptionEventKind::PaneAgentStatusChanged)
        );
        let payload =
            serde_json::from_value::<PaneAgentStatusChangedPayload>(envelope.data.clone())
                .unwrap_or_else(|e| panic!("pane.agent_status_changed payload: {e}"));
        assert_modeled_fields_match(
            &envelope.data,
            &payload,
            PANE_AGENT_STATUS_CHANGED_OPTIONAL_FIELDS,
            "pane.agent_status_changed",
        );
        status_events_seen += 1;
    }
    assert!(status_events_seen > 0, "fixture must not be empty");
}

#[test]
fn pane_moved_keeps_both_ids() {
    let data = serde_json::json!({
        "type": "pane_moved",
        "previous_pane_id": "wJ:p1",
        "previous_workspace_id": "wJ",
        "previous_tab_id": "wJ:t1",
        "pane": {
            "pane_id": "wK:p3",
            "workspace_id": "wK",
            "tab_id": "wK:t1",
            "agent_status": "unknown",
            "focused": false,
            "revision": 1
        }
    });

    let payload: PaneMovedPayload = serde_json::from_value(data).unwrap();
    assert_eq!(payload.previous_pane_id, "wJ:p1");
    assert_eq!(payload.pane.pane_id, "wK:p3");
    assert_ne!(payload.previous_pane_id, payload.pane.pane_id);
}

#[test]
fn pane_moved_optional_fields_default_to_none() {
    let data = serde_json::json!({
        "type": "pane_moved",
        "previous_pane_id": "wJ:p1",
        "previous_workspace_id": "wJ",
        "previous_tab_id": "wJ:t1",
        "pane": {
            "pane_id": "wJ:p2",
            "workspace_id": "wJ",
            "tab_id": "wJ:t1",
            "agent_status": "unknown",
            "focused": false,
            "revision": 1
        }
    });

    let payload: PaneMovedPayload = serde_json::from_value(data).unwrap();
    assert!(payload.created_workspace.is_none());
    assert!(payload.created_tab.is_none());
    assert!(payload.closed_workspace_id.is_none());
    assert!(payload.closed_tab_id.is_none());
}

#[test]
fn pane_agent_detected_released_with_null_agent() {
    let data = serde_json::json!({
        "type": "pane_agent_detected",
        "pane_id": "wJ:p1",
        "workspace_id": "wJ",
        "agent": null,
        "released": true
    });

    let payload: PaneAgentDetectedPayload = serde_json::from_value(data).unwrap();
    assert!(payload.agent.is_none());
    assert!(payload.released);
    assert_eq!(payload.final_status, None::<AgentStatus>);
}

#[test]
fn events_subscribe_params_serializes_lifecycle_and_per_pane_together() {
    let mut subscriptions = Subscription::all_lifecycle();
    subscriptions.push(Subscription::PaneAgentStatusChanged {
        pane_id: "wJ:p1".to_string(),
    });
    let params = EventsSubscribeParams { subscriptions };
    let value = serde_json::to_value(&params).unwrap();
    let list = value["subscriptions"].as_array().unwrap();
    assert_eq!(list.len(), 25);
}

// ---------------------------------------------------------------------------
// Fix round 1 / finding 3（medium）：真機 fixture 只出現 9 種事件名稱，下面幾種在
// `event_fixtures_parse_with_matching_payload_types` 完全沒被跑到。手寫最小案例，先過
// `event` 根的 schema 驗證（`common::assert_valid_as_event`，跟 contract.rs 共用同一顆
// validator），再解析成 payload 逐欄斷言，補齊涵蓋範圍。
// ---------------------------------------------------------------------------

fn valid_workspace_info_json(id: &str) -> serde_json::Value {
    serde_json::json!({
        "workspace_id": id,
        "number": 9,
        "label": "SYNTHETIC_WS",
        "focused": false,
        "pane_count": 0,
        "tab_count": 0,
        "active_tab_id": format!("{id}:t1"),
        "agent_status": "idle"
    })
}

fn valid_tab_info_json(id: &str, workspace_id: &str) -> serde_json::Value {
    serde_json::json!({
        "tab_id": id,
        "workspace_id": workspace_id,
        "number": 1,
        "label": "SYNTHETIC_TAB",
        "agent_status": "idle",
        "focused": false,
        "pane_count": 0
    })
}

fn valid_pane_info_json(id: &str, workspace_id: &str, tab_id: &str) -> serde_json::Value {
    // `terminal_id` 是 schema 的 `PaneInfo` 必填欄位，我們的 `PaneInfo` 型別刻意不建模它
    // （不在 observer 子集），但拿去做「這個 payload 通過 schema」的合成 fixture 時仍要帶。
    serde_json::json!({
        "pane_id": id,
        "terminal_id": format!("term_synthetic_{id}"),
        "workspace_id": workspace_id,
        "tab_id": tab_id,
        "agent_status": "idle",
        "focused": false,
        "revision": 1
    })
}

#[test]
fn synthetic_workspace_created_is_schema_valid_and_maps_workspace() {
    let event = serde_json::json!({
        "event": "workspace_created",
        "data": {
            "type": "workspace_created",
            "workspace": valid_workspace_info_json("wZ")
        }
    });
    assert_valid_as_event(&event, "synthetic workspace_created");

    let payload: WorkspacePayload = serde_json::from_value(event["data"].clone()).unwrap();
    assert_eq!(payload.workspace.workspace_id, "wZ");
    assert_eq!(payload.workspace.label, "SYNTHETIC_WS");
    assert_eq!(payload.workspace.agent_status, AgentStatus::Idle);
}

#[test]
fn synthetic_workspace_closed_is_schema_valid_and_maps_id() {
    let event = serde_json::json!({
        "event": "workspace_closed",
        "data": { "type": "workspace_closed", "workspace_id": "wZ" }
    });
    assert_valid_as_event(&event, "synthetic workspace_closed");

    let payload: WorkspaceClosedPayload = serde_json::from_value(event["data"].clone()).unwrap();
    assert_eq!(payload.workspace_id, "wZ");
    assert!(payload.workspace.is_none());
}

#[test]
fn synthetic_workspace_renamed_is_schema_valid_and_maps_fields() {
    let event = serde_json::json!({
        "event": "workspace_renamed",
        "data": { "type": "workspace_renamed", "workspace_id": "wZ", "label": "NEW_LABEL" }
    });
    assert_valid_as_event(&event, "synthetic workspace_renamed");

    let payload: WorkspaceRenamedPayload = serde_json::from_value(event["data"].clone()).unwrap();
    assert_eq!(payload.workspace_id, "wZ");
    assert_eq!(payload.label, "NEW_LABEL");
}

#[test]
fn synthetic_workspace_moved_and_reordered_are_schema_valid_and_map_workspaces() {
    for event_name in ["workspace_moved", "workspace_reordered"] {
        let data = if event_name == "workspace_moved" {
            serde_json::json!({
                "type": event_name,
                "workspace_id": "wZ",
                "insert_index": 0,
                "workspaces": [valid_workspace_info_json("wZ")]
            })
        } else {
            serde_json::json!({
                "type": event_name,
                "workspace_ids": ["wZ"],
                "workspaces": [valid_workspace_info_json("wZ")]
            })
        };
        let event = serde_json::json!({ "event": event_name, "data": data });
        assert_valid_as_event(&event, &format!("synthetic {event_name}"));

        let payload: WorkspacesReplacedPayload =
            serde_json::from_value(event["data"].clone()).unwrap();
        assert_eq!(payload.workspaces.len(), 1);
        assert_eq!(payload.workspaces[0].workspace_id, "wZ");
    }
}

#[test]
fn synthetic_tab_renamed_is_schema_valid_and_maps_fields() {
    let event = serde_json::json!({
        "event": "tab_renamed",
        "data": {
            "type": "tab_renamed",
            "tab_id": "wZ:t1",
            "workspace_id": "wZ",
            "label": "NEW_TAB_LABEL"
        }
    });
    assert_valid_as_event(&event, "synthetic tab_renamed");

    let payload: TabRenamedPayload = serde_json::from_value(event["data"].clone()).unwrap();
    assert_eq!(payload.tab_id, "wZ:t1");
    assert_eq!(payload.workspace_id, "wZ");
    assert_eq!(payload.label, "NEW_TAB_LABEL");
}

#[test]
fn synthetic_tab_moved_is_schema_valid_and_maps_fields() {
    let event = serde_json::json!({
        "event": "tab_moved",
        "data": {
            "type": "tab_moved",
            "tab_id": "wZ:t1",
            "workspace_id": "wZ",
            "insert_index": 2,
            "tabs": [valid_tab_info_json("wZ:t1", "wZ")]
        }
    });
    assert_valid_as_event(&event, "synthetic tab_moved");

    let payload: TabMovedPayload = serde_json::from_value(event["data"].clone()).unwrap();
    assert_eq!(payload.tab_id, "wZ:t1");
    assert_eq!(payload.workspace_id, "wZ");
    assert_eq!(payload.insert_index, 2);
    assert_eq!(payload.tabs.len(), 1);
    assert_eq!(payload.tabs[0].tab_id, "wZ:t1");
}

#[test]
fn synthetic_pane_closed_and_exited_are_schema_valid_and_map_ids() {
    for event_name in ["pane_closed", "pane_exited"] {
        let event = serde_json::json!({
            "event": event_name,
            "data": { "type": event_name, "pane_id": "wZ:p1", "workspace_id": "wZ" }
        });
        assert_valid_as_event(&event, &format!("synthetic {event_name}"));

        let payload: PaneClosedPayload = serde_json::from_value(event["data"].clone()).unwrap();
        assert_eq!(payload.pane_id, "wZ:p1");
        assert_eq!(payload.workspace_id, "wZ");
    }
}

#[test]
fn pane_moved_optional_fields_retain_value_when_present() {
    let created_workspace = valid_workspace_info_json("wNew");
    let created_tab = valid_tab_info_json("wNew:t1", "wNew");
    let event = serde_json::json!({
        "event": "pane_moved",
        "data": {
            "type": "pane_moved",
            "previous_pane_id": "wOld:p1",
            "previous_workspace_id": "wOld",
            "previous_tab_id": "wOld:t1",
            "pane": valid_pane_info_json("wNew:p1", "wNew", "wNew:t1"),
            "created_workspace": created_workspace,
            "created_tab": created_tab,
            "closed_workspace_id": "wOld",
            "closed_tab_id": "wOld:t1"
        }
    });
    assert_valid_as_event(&event, "synthetic pane_moved with optional fields present");

    let payload: PaneMovedPayload = serde_json::from_value(event["data"].clone()).unwrap();
    // fix round 2 / finding 3：原本只比對 `previous_pane_id` 與新 `pane.pane_id`——
    // `previous_workspace_id`、`previous_tab_id`、`pane` 其餘欄位（`workspace_id`、
    // `tab_id`、`agent_status`、`focused`、`revision`）、`created_workspace`、
    // `created_tab` 的完整內容都沒被驗過。deep match 一次涵蓋全部已建模欄位。
    assert_modeled_fields_match(
        &event["data"],
        &payload,
        PANE_MOVED_OPTIONAL_FIELDS,
        "pane_moved with optional fields present",
    );
}

#[test]
fn pane_agent_detected_final_status_retained_when_present() {
    let event = serde_json::json!({
        "event": "pane_agent_detected",
        "data": {
            "type": "pane_agent_detected",
            "pane_id": "wZ:p1",
            "workspace_id": "wZ",
            "agent": "claude",
            "released": false,
            "final_status": "blocked"
        }
    });
    assert_valid_as_event(
        &event,
        "synthetic pane_agent_detected with final_status present",
    );

    let payload: PaneAgentDetectedPayload = serde_json::from_value(event["data"].clone()).unwrap();
    assert_modeled_fields_match(
        &event["data"],
        &payload,
        PANE_AGENT_DETECTED_OPTIONAL_FIELDS,
        "pane_agent_detected with final_status present",
    );
}

#[test]
fn synthetic_pane_agent_status_changed_lifecycle_shape_is_schema_valid() {
    // `pane_agent_status_changed`（底線命名）是 `EventKind` 的 26 種之一，但真機 fixture
    // 只在每 pane 訂閱（點號命名）收過它，這裡另外確認生命週期形狀本身也合法、payload
    // 型別可以共用。
    let event = serde_json::json!({
        "event": "pane_agent_status_changed",
        "data": {
            "type": "pane_agent_status_changed",
            "pane_id": "wZ:p1",
            "workspace_id": "wZ",
            "agent_status": "working"
        }
    });
    assert_valid_as_event(
        &event,
        "synthetic pane_agent_status_changed (lifecycle shape)",
    );

    let payload: PaneAgentStatusChangedPayload =
        serde_json::from_value(event["data"].clone()).unwrap();
    assert_modeled_fields_match(
        &event["data"],
        &payload,
        PANE_AGENT_STATUS_CHANGED_OPTIONAL_FIELDS,
        "pane_agent_status_changed (lifecycle shape)",
    );
}

#[test]
fn synthetic_pane_agent_status_changed_per_pane_with_display_fields_present() {
    // fix round 2 / finding 3：`display_agent`、`title`、`state_labels` 原本完全沒有「有值」
    // 的測試案例（真機 fixture 的 4 行都沒帶這三個欄位）。這裡用每 pane（點號命名、無
    // "type" 欄位）的實際形狀，先過 `subscription_event` 根驗證，再逐欄比對。
    let event = serde_json::json!({
        "event": "pane.agent_status_changed",
        "data": {
            "pane_id": "wZ:p1",
            "workspace_id": "wZ",
            "agent_status": "blocked",
            "agent": "claude",
            "display_agent": "Claude Code",
            "title": "waiting for approval",
            "state_labels": { "mode": "review" }
        }
    });
    assert_valid_as_subscription_event(
        &event,
        "synthetic pane.agent_status_changed with display fields present",
    );

    let payload: PaneAgentStatusChangedPayload =
        serde_json::from_value(event["data"].clone()).unwrap();
    // 這個案例每個欄位都給了值，allow-list 傳空陣列——正好額外驗證：
    // `display_agent`／`title`／`state_labels` 都不在任何 allow-list 常數裡，全靠這裡的
    // "raw 有 key" 分支比對到真值。
    assert_modeled_fields_match(
        &event["data"],
        &payload,
        &[],
        "pane.agent_status_changed with display fields present",
    );
}

// ---------------------------------------------------------------------------
// Fix round 3 / finding 3：負向測試，證明 `assert_modeled_fields_match` 真的會抓到 serde
// rename 打錯字——不是只靠人眼審查 allow-list 寫得對不對。
// ---------------------------------------------------------------------------

/// 只給這個負向測試用的 test-only struct：故意把欄位 rename 成錯字 `display_agnet`
/// （少一個 e，模擬打錯字），示範就算欄位本身的值是對的，序列化出來的 key 名稱錯了，
/// helper 也要能抓到。
#[derive(serde::Serialize)]
struct PayloadWithWrongRename {
    #[serde(rename = "display_agnet")]
    display_agent: Option<String>,
}

#[test]
#[should_panic(expected = "raw has no key")]
fn assert_modeled_fields_match_catches_wrong_serde_rename() {
    // `raw` 用的是正確欄位名稱 `display_agent`，跟真機／schema 一致。
    let raw = serde_json::json!({ "display_agent": "Claude Code" });
    let parsed = PayloadWithWrongRename {
        display_agent: Some("Claude Code".to_string()),
    };
    // allow-list 刻意只寫「正確」的欄位名稱 `display_agent`——這是呼叫端獨立於程式碼、自己
    // 認定的「這個型別選填欄位叫什麼名字」，不是從 `parsed` 反推出來的。`parsed` 因為
    // rename 打錯字，序列化出來的 key 是 `display_agnet`（打錯字那個），既不在 `raw`
    // 裡、也不在 allow-list 裡，helper 必須 panic。
    assert_modeled_fields_match(&raw, &parsed, &["display_agent"], "wrong rename regression");
}

// ---------------------------------------------------------------------------
// 全分支最終 review finding 1：所有事件 payload 型別都在容器層開 `#[serde(default)]`，
// 缺必填欄位（例如 `pane_moved` 缺 `previous_pane_id`／`pane`、`pane.agent_status_changed`
// 缺 `agent_status`）會被靜默補成空字串／`Default` 值，而不是解析失敗（違反 design D13）。
// 比照 `tests/types.rs`（`WorkspaceInfo`／`TabInfo`／`PaneInfo` 等）的表格化做法：「已建模
// 欄位」交「schema 必填欄位」的交集逐一從合法 JSON 刪除，斷言解析失敗；差集逐一刪除，斷言
// 仍解析成功。p22、p20 schema 對這些事件 payload 的 `required` 陣列相同（已用
// `docs/research` 的查證腳本核對），但仍各自表格化跑一次，跟 `tests/types.rs` 的作法一致。
// ---------------------------------------------------------------------------

#[test]
fn workspace_payload_required_fields_are_all_enforced() {
    let valid = serde_json::json!({
        "type": "workspace_created",
        "workspace": valid_workspace_info_json("wZ")
    });
    for schema_file in SCHEMA_FILES {
        assert_required_fields_detected_for_event_variant::<WorkspacePayload>(
            schema_file,
            "event",
            "EventData",
            "workspace_created",
            valid.clone(),
        );
    }
}

#[test]
fn workspace_closed_payload_required_fields_are_all_enforced() {
    let valid = serde_json::json!({
        "type": "workspace_closed",
        "workspace_id": "wZ",
        "workspace": valid_workspace_info_json("wZ")
    });
    for schema_file in SCHEMA_FILES {
        assert_required_fields_detected_for_event_variant::<WorkspaceClosedPayload>(
            schema_file,
            "event",
            "EventData",
            "workspace_closed",
            valid.clone(),
        );
    }
}

#[test]
fn workspace_renamed_payload_required_fields_are_all_enforced() {
    let valid = serde_json::json!({
        "type": "workspace_renamed",
        "workspace_id": "wZ",
        "label": "L"
    });
    for schema_file in SCHEMA_FILES {
        assert_required_fields_detected_for_event_variant::<WorkspaceRenamedPayload>(
            schema_file,
            "event",
            "EventData",
            "workspace_renamed",
            valid.clone(),
        );
    }
}

#[test]
fn workspaces_replaced_payload_required_fields_are_all_enforced() {
    for type_const in ["workspace_moved", "workspace_reordered"] {
        let valid = if type_const == "workspace_moved" {
            serde_json::json!({
                "type": type_const,
                "workspace_id": "wZ",
                "insert_index": 0,
                "workspaces": [valid_workspace_info_json("wZ")]
            })
        } else {
            serde_json::json!({
                "type": type_const,
                "workspace_ids": ["wZ"],
                "workspaces": [valid_workspace_info_json("wZ")]
            })
        };
        for schema_file in SCHEMA_FILES {
            assert_required_fields_detected_for_event_variant::<WorkspacesReplacedPayload>(
                schema_file,
                "event",
                "EventData",
                type_const,
                valid.clone(),
            );
        }
    }
}

#[test]
fn workspace_focused_payload_required_fields_are_all_enforced() {
    let valid = serde_json::json!({
        "type": "workspace_focused",
        "workspace_id": "wZ"
    });
    for schema_file in SCHEMA_FILES {
        assert_required_fields_detected_for_event_variant::<WorkspaceFocusedPayload>(
            schema_file,
            "event",
            "EventData",
            "workspace_focused",
            valid.clone(),
        );
    }
}

#[test]
fn tab_payload_required_fields_are_all_enforced() {
    for type_const in ["tab_created"] {
        let valid = serde_json::json!({
            "type": type_const,
            "tab": valid_tab_info_json("wZ:t1", "wZ")
        });
        for schema_file in SCHEMA_FILES {
            assert_required_fields_detected_for_event_variant::<TabPayload>(
                schema_file,
                "event",
                "EventData",
                type_const,
                valid.clone(),
            );
        }
    }
}

#[test]
fn tab_closed_payload_required_fields_are_all_enforced() {
    let valid = serde_json::json!({
        "type": "tab_closed",
        "tab_id": "wZ:t1",
        "workspace_id": "wZ"
    });
    for schema_file in SCHEMA_FILES {
        assert_required_fields_detected_for_event_variant::<TabClosedPayload>(
            schema_file,
            "event",
            "EventData",
            "tab_closed",
            valid.clone(),
        );
    }
}

#[test]
fn tab_renamed_payload_required_fields_are_all_enforced() {
    let valid = serde_json::json!({
        "type": "tab_renamed",
        "tab_id": "wZ:t1",
        "workspace_id": "wZ",
        "label": "L"
    });
    for schema_file in SCHEMA_FILES {
        assert_required_fields_detected_for_event_variant::<TabRenamedPayload>(
            schema_file,
            "event",
            "EventData",
            "tab_renamed",
            valid.clone(),
        );
    }
}

#[test]
fn tab_moved_payload_required_fields_are_all_enforced() {
    let valid = serde_json::json!({
        "type": "tab_moved",
        "tab_id": "wZ:t1",
        "workspace_id": "wZ",
        "insert_index": 2,
        "tabs": [valid_tab_info_json("wZ:t1", "wZ")]
    });
    for schema_file in SCHEMA_FILES {
        assert_required_fields_detected_for_event_variant::<TabMovedPayload>(
            schema_file,
            "event",
            "EventData",
            "tab_moved",
            valid.clone(),
        );
    }
}

#[test]
fn tab_focused_payload_required_fields_are_all_enforced() {
    let valid = serde_json::json!({
        "type": "tab_focused",
        "tab_id": "wZ:t1",
        "workspace_id": "wZ"
    });
    for schema_file in SCHEMA_FILES {
        assert_required_fields_detected_for_event_variant::<TabFocusedPayload>(
            schema_file,
            "event",
            "EventData",
            "tab_focused",
            valid.clone(),
        );
    }
}

#[test]
fn pane_payload_required_fields_are_all_enforced() {
    for type_const in ["pane_created", "pane_updated"] {
        let valid = serde_json::json!({
            "type": type_const,
            "pane": valid_pane_info_json("wZ:p1", "wZ", "wZ:t1")
        });
        for schema_file in SCHEMA_FILES {
            assert_required_fields_detected_for_event_variant::<PanePayload>(
                schema_file,
                "event",
                "EventData",
                type_const,
                valid.clone(),
            );
        }
    }
}

#[test]
fn pane_closed_payload_required_fields_are_all_enforced() {
    for type_const in ["pane_closed", "pane_exited"] {
        let valid = serde_json::json!({
            "type": type_const,
            "pane_id": "wZ:p1",
            "workspace_id": "wZ"
        });
        for schema_file in SCHEMA_FILES {
            assert_required_fields_detected_for_event_variant::<PaneClosedPayload>(
                schema_file,
                "event",
                "EventData",
                type_const,
                valid.clone(),
            );
        }
    }
}

#[test]
fn pane_focused_payload_required_fields_are_all_enforced() {
    let valid = serde_json::json!({
        "type": "pane_focused",
        "pane_id": "wZ:p1",
        "workspace_id": "wZ"
    });
    for schema_file in SCHEMA_FILES {
        assert_required_fields_detected_for_event_variant::<PaneFocusedPayload>(
            schema_file,
            "event",
            "EventData",
            "pane_focused",
            valid.clone(),
        );
    }
}

#[test]
fn pane_moved_payload_required_fields_are_all_enforced() {
    let valid = serde_json::json!({
        "type": "pane_moved",
        "previous_pane_id": "wJ:p1",
        "previous_workspace_id": "wJ",
        "previous_tab_id": "wJ:t1",
        "pane": valid_pane_info_json("wK:p1", "wK", "wK:t1"),
        "created_workspace": valid_workspace_info_json("wK"),
        "created_tab": valid_tab_info_json("wK:t1", "wK"),
        "closed_workspace_id": "wJ",
        "closed_tab_id": "wJ:t1"
    });
    for schema_file in SCHEMA_FILES {
        assert_required_fields_detected_for_event_variant::<PaneMovedPayload>(
            schema_file,
            "event",
            "EventData",
            "pane_moved",
            valid.clone(),
        );
    }
}

#[test]
fn pane_agent_detected_payload_required_fields_are_all_enforced() {
    let valid = serde_json::json!({
        "type": "pane_agent_detected",
        "pane_id": "wZ:p1",
        "workspace_id": "wZ",
        "agent": "claude",
        "released": false,
        "final_status": "blocked"
    });
    for schema_file in SCHEMA_FILES {
        assert_required_fields_detected_for_event_variant::<PaneAgentDetectedPayload>(
            schema_file,
            "event",
            "EventData",
            "pane_agent_detected",
            valid.clone(),
        );
    }
}

#[test]
fn pane_agent_status_changed_payload_required_fields_are_all_enforced_lifecycle() {
    let valid = serde_json::json!({
        "type": "pane_agent_status_changed",
        "pane_id": "wZ:p1",
        "workspace_id": "wZ",
        "agent_status": "working",
        "agent": "claude",
        "display_agent": "Claude Code",
        "title": "waiting",
        "state_labels": { "mode": "review" }
    });
    for schema_file in SCHEMA_FILES {
        assert_required_fields_detected_for_event_variant::<PaneAgentStatusChangedPayload>(
            schema_file,
            "event",
            "EventData",
            "pane_agent_status_changed",
            valid.clone(),
        );
    }
}

/// 每 pane 訂閱推送形狀（`schemas.subscription_event.$defs.PaneAgentStatusChangedEvent`，沒有
/// `oneOf`／`type` 判別式，`schema_required_fields` 直接讀就夠，不需要
/// `_for_event_variant` 那套）。
#[test]
fn pane_agent_status_changed_payload_required_fields_are_all_enforced_per_pane() {
    let valid = serde_json::json!({
        "pane_id": "wZ:p1",
        "workspace_id": "wZ",
        "agent_status": "working",
        "agent": "claude",
        "display_agent": "Claude Code",
        "title": "waiting",
        "state_labels": { "mode": "review" }
    });
    for schema_file in SCHEMA_FILES {
        assert_required_fields_detected::<PaneAgentStatusChangedPayload>(
            schema_file,
            "subscription_event",
            "PaneAgentStatusChangedEvent",
            valid.clone(),
        );
    }
}
