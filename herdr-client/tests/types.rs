//! Task 3.1 驗收測試：observer 子集型別（spec `herdr-observer-types`）。

use herdr_client::types::{
    AgentInfo, AgentStatus, ErrorBody, PaneInfo, ReadFormat, ResponseEnvelope, SessionSnapshot,
    SessionSnapshotResult, TabInfo, WorkspaceInfo,
};

mod common;
use common::{
    assert_modeled_fields_match, assert_required_fields_detected, schema_required_fields,
};

/// fix round 3 / finding 2：round 2 的表格化必填欄位測試只讀 `schema-p22.json`，沒有
/// 斷言守住「p20 的 required 陣列跟 p22 一樣」這個前提。現在對兩份 schema 都跑一次，另加
/// `p22_and_p20_required_fields_agree_for_modeled_types` 直接斷言兩份的 required 陣列
/// 相等，schema drift 時會印出差異、明確失敗，不會靜默漏掉。
const SCHEMA_FILES: &[&str] = &["schema-p22.json", "schema-p20.json"];

fn fixture(name: &str) -> serde_json::Value {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    let raw = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read fixture {}: {e}", path.display()));
    serde_json::from_str(&raw).unwrap_or_else(|e| panic!("fixture {name} is not valid JSON: {e}"))
}

/// 解析出 `ResponseEnvelope`，再把 `result` 解析成 `SessionSnapshotResult`，回傳兩者供斷言。
fn parse_snapshot_response(name: &str) -> (serde_json::Value, SessionSnapshotResult) {
    let raw = fixture(name);
    let envelope: ResponseEnvelope = serde_json::from_value(raw.clone())
        .unwrap_or_else(|e| panic!("{name}: failed to parse ResponseEnvelope: {e}"));
    let result_value = envelope
        .result
        .unwrap_or_else(|| panic!("{name}: ResponseEnvelope.result is None"));
    let result: SessionSnapshotResult = serde_json::from_value(result_value)
        .unwrap_or_else(|e| panic!("{name}: failed to parse SessionSnapshotResult: {e}"));
    (raw, result)
}

fn array_len(raw: &serde_json::Value, pointer: &str) -> usize {
    raw.pointer(pointer)
        .unwrap_or_else(|| panic!("fixture missing {pointer}"))
        .as_array()
        .unwrap_or_else(|| panic!("{pointer} is not an array"))
        .len()
}

#[test]
fn snapshot_fixture_p22_parses() {
    let (raw, result) = parse_snapshot_response("snapshot-p22.json");
    let snapshot = result.snapshot;

    assert_eq!(
        snapshot.workspaces.len(),
        array_len(&raw, "/result/snapshot/workspaces")
    );
    assert_eq!(
        snapshot.tabs.len(),
        array_len(&raw, "/result/snapshot/tabs")
    );
    assert_eq!(
        snapshot.panes.len(),
        array_len(&raw, "/result/snapshot/panes")
    );
    assert_eq!(
        snapshot.agents.len(),
        array_len(&raw, "/result/snapshot/agents")
    );
    assert_eq!(
        snapshot.version,
        raw.pointer("/result/snapshot/version")
            .and_then(serde_json::Value::as_str)
            .unwrap()
    );
    assert_eq!(
        snapshot.protocol,
        raw.pointer("/result/snapshot/protocol")
            .and_then(serde_json::Value::as_u64)
            .unwrap() as u32
    );

    // fix round 2 / finding 3：上面幾個斷言只挑了長度／version／protocol；這裡遞迴比對
    // `snapshot` 重新序列化後的每一個已建模欄位（含 workspaces/tabs/panes/agents 陣列裡
    // 每一筆的每一個欄位）與原始 fixture 的值，一次涵蓋全部。fix round 3：
    // `SNAPSHOT_OPTIONAL_FIELDS` 是這棵樹裡「合法選填、raw 缺席也不算錯」的裸欄位名稱，
    // WorkspaceInfo／TabInfo 全部欄位都必填，不在清單裡，缺席一律視為失敗。
    assert_modeled_fields_match(
        &raw["result"]["snapshot"],
        &snapshot,
        SNAPSHOT_OPTIONAL_FIELDS,
        "snapshot-p22.json snapshot (deep)",
    );
}

#[test]
fn snapshot_fixture_p20_parses() {
    let (raw, result) = parse_snapshot_response("snapshot-p20.json");
    let snapshot = result.snapshot;

    assert_eq!(
        snapshot.workspaces.len(),
        array_len(&raw, "/result/snapshot/workspaces")
    );
    assert_eq!(
        snapshot.tabs.len(),
        array_len(&raw, "/result/snapshot/tabs")
    );
    assert_eq!(
        snapshot.panes.len(),
        array_len(&raw, "/result/snapshot/panes")
    );
    assert_eq!(
        snapshot.agents.len(),
        array_len(&raw, "/result/snapshot/agents")
    );
    assert_eq!(
        snapshot.version,
        raw.pointer("/result/snapshot/version")
            .and_then(serde_json::Value::as_str)
            .unwrap()
    );
    assert_eq!(
        snapshot.protocol,
        raw.pointer("/result/snapshot/protocol")
            .and_then(serde_json::Value::as_u64)
            .unwrap() as u32
    );

    assert_modeled_fields_match(
        &raw["result"]["snapshot"],
        &snapshot,
        SNAPSHOT_OPTIONAL_FIELDS,
        "snapshot-p20.json snapshot (deep)",
    );
}

/// fix round 3 / finding 3：`assert_modeled_fields_match` 現在預設「raw 缺這個 key 就失敗」，
/// 只有列在這裡的裸欄位名稱才放行。`SessionSnapshot` 樹裡真正選填的只有這些——巢狀
/// `PaneInfo`／`AgentInfo` 的選填欄位（`agent`／`title`／`terminal_title`／`cwd`／`label`）
/// 與頂層三個 `focused_*`；`WorkspaceInfo`／`TabInfo` 沒有選填欄位，不在清單裡，任何位置
/// 缺席都會讓比對失敗。
const SNAPSHOT_OPTIONAL_FIELDS: &[&str] = &[
    "agent",
    "title",
    "terminal_title",
    "cwd",
    "label",
    "focused_workspace_id",
    "focused_tab_id",
    "focused_pane_id",
];

/// `PaneInfo` 單獨核對時的選填欄位清單（不含 `SessionSnapshot` 才有的 `focused_*`）。
const PANE_INFO_OPTIONAL_FIELDS: &[&str] = &["agent", "title", "terminal_title", "cwd", "label"];

/// `AgentInfo` 單獨核對時的選填欄位清單。
const AGENT_INFO_OPTIONAL_FIELDS: &[&str] = &["agent"];

#[test]
fn unknown_fields_ignored_and_missing_optional_is_none() {
    let value = serde_json::json!({
        "pane_id": "wJ:p1",
        "workspace_id": "wJ",
        "tab_id": "wJ:t1",
        "agent": "claude",
        "agent_status": "working",
        "focused": true,
        "revision": 42,
        "future_field": "should be ignored",
        // "cwd" 刻意缺席
    });

    let pane: PaneInfo =
        serde_json::from_value(value).expect("unknown field must not fail parsing");

    assert_eq!(pane.pane_id, "wJ:p1");
    assert_eq!(pane.agent.as_deref(), Some("claude"));
    assert_eq!(pane.agent_status, AgentStatus::Working);
    assert!(pane.cwd.is_none());
}

#[test]
fn agent_status_unknown_catch_all() {
    let status: AgentStatus =
        serde_json::from_value(serde_json::Value::String("meditating".to_string()))
            .expect("unrecognized value must not fail parsing");
    assert_eq!(status, AgentStatus::Unknown);
}

#[test]
fn agent_status_done_parses_as_done() {
    let status: AgentStatus =
        serde_json::from_value(serde_json::Value::String("done".to_string())).unwrap();
    assert_eq!(status, AgentStatus::Done);
}

#[test]
fn result_type_mismatch_fails() {
    let mut raw = fixture("snapshot-p22.json");
    raw["result"]["type"] = serde_json::Value::String("pong".to_string());

    let envelope: ResponseEnvelope = serde_json::from_value(raw).unwrap();
    let result_value = envelope.result.unwrap();

    let parsed = serde_json::from_value::<SessionSnapshotResult>(result_value);
    assert!(
        parsed.is_err(),
        "SessionSnapshotResult must reject a result whose \"type\" is not \"session_snapshot\""
    );
}

// ---------------------------------------------------------------------------
// Fix round 1 / finding 1（high）：`format` 必須是封閉的 `ReadFormat`，不能是任意 `String`。
// ---------------------------------------------------------------------------

#[test]
fn read_format_round_trips_known_values() {
    let text: ReadFormat =
        serde_json::from_value(serde_json::Value::String("text".to_string())).unwrap();
    assert_eq!(text, ReadFormat::Text);
    let ansi: ReadFormat =
        serde_json::from_value(serde_json::Value::String("ansi".to_string())).unwrap();
    assert_eq!(ansi, ReadFormat::Ansi);
}

#[test]
fn read_format_rejects_unknown_value() {
    let parsed =
        serde_json::from_value::<ReadFormat>(serde_json::Value::String("html".to_string()));
    assert!(
        parsed.is_err(),
        "ReadFormat must only accept \"text\" or \"ansi\" (schema ReadFormat enum)"
    );
}

// ---------------------------------------------------------------------------
// Fix round 1 / finding 2（medium）：WorkspaceInfo／TabInfo／PaneInfo／AgentInfo 逐欄核對
// fixture 的值，並證明缺少 schema 必填欄位時解析會失敗（不再被容器層 `#[serde(default)]`
// 靜默補成空字串／0／Unknown）。
// ---------------------------------------------------------------------------

/// 在 fixture 的陣列裡用 id 欄位找出一筆物件，找不到就直接讓測試失敗並附上可查的訊息。
fn find_by_id<'a>(array: &'a serde_json::Value, id_field: &str, id: &str) -> &'a serde_json::Value {
    array
        .as_array()
        .unwrap_or_else(|| panic!("{id_field} array missing or not an array"))
        .iter()
        .find(|item| item.get(id_field).and_then(serde_json::Value::as_str) == Some(id))
        .unwrap_or_else(|| panic!("no item with {id_field} == {id:?}"))
}

#[test]
fn workspace_info_maps_all_fields_from_fixture_p22() {
    let raw = fixture("snapshot-p22.json");
    let ws_json = find_by_id(
        &raw["result"]["snapshot"]["workspaces"],
        "workspace_id",
        "wJ",
    );
    let ws: WorkspaceInfo = serde_json::from_value(ws_json.clone()).unwrap();
    assert_modeled_fields_match(ws_json, &ws, &[], "workspace wJ (p22)");
}

#[test]
fn workspace_info_maps_all_fields_from_fixture_p20() {
    let raw = fixture("snapshot-p20.json");
    let ws_json = find_by_id(
        &raw["result"]["snapshot"]["workspaces"],
        "workspace_id",
        "wD",
    );
    let ws: WorkspaceInfo = serde_json::from_value(ws_json.clone()).unwrap();
    assert_modeled_fields_match(ws_json, &ws, &[], "workspace wD (p20)");
}

#[test]
fn tab_info_maps_all_fields_from_fixture_p22() {
    let raw = fixture("snapshot-p22.json");
    let tab_json = find_by_id(&raw["result"]["snapshot"]["tabs"], "tab_id", "wJ:t1");
    let tab: TabInfo = serde_json::from_value(tab_json.clone()).unwrap();
    assert_modeled_fields_match(tab_json, &tab, &[], "tab wJ:t1 (p22)");
}

#[test]
fn tab_info_maps_all_fields_from_fixture_p20() {
    let raw = fixture("snapshot-p20.json");
    let tab_json = find_by_id(&raw["result"]["snapshot"]["tabs"], "tab_id", "wD:t1");
    let tab: TabInfo = serde_json::from_value(tab_json.clone()).unwrap();
    assert_modeled_fields_match(tab_json, &tab, &[], "tab wD:t1 (p20)");
}

#[test]
fn pane_info_maps_all_fields_from_fixture_p22() {
    // wJ:p1 同時涵蓋「有值」（agent、cwd、terminal_title）與「schema 允許但這筆缺席」
    // （title、label）兩種情況；`assert_modeled_fields_match` 逐欄比對，兩種情況都會被
    // 驗到（後者：解析後是 `None`／序列化為 `null`，原始 JSON 也沒有這個 key）。
    let raw = fixture("snapshot-p22.json");
    let pane_json = find_by_id(&raw["result"]["snapshot"]["panes"], "pane_id", "wJ:p1");
    let pane: PaneInfo = serde_json::from_value(pane_json.clone()).unwrap();
    assert_modeled_fields_match(
        pane_json,
        &pane,
        PANE_INFO_OPTIONAL_FIELDS,
        "pane wJ:p1 (p22)",
    );
}

#[test]
fn pane_info_maps_all_fields_from_fixture_p20() {
    // wD:p8 涵蓋 label／terminal_title／cwd 皆有值、但沒有 agent 的情況（p20 fixture
    // 完全沒有 agent）。
    let raw = fixture("snapshot-p20.json");
    let pane_json = find_by_id(&raw["result"]["snapshot"]["panes"], "pane_id", "wD:p8");
    let pane: PaneInfo = serde_json::from_value(pane_json.clone()).unwrap();
    assert_modeled_fields_match(
        pane_json,
        &pane,
        PANE_INFO_OPTIONAL_FIELDS,
        "pane wD:p8 (p20)",
    );
}

#[test]
fn agent_info_maps_all_fields_from_fixture_p22() {
    // p20 fixture 的 agents 陣列是空的（`/result/snapshot/agents` == []，已由
    // `snapshot_fixture_p20_parses` 的陣列長度斷言與 deep match 覆蓋），沒有實例可核對
    // 欄位，所以 AgentInfo 的逐欄核對只在 p22 做。
    let raw = fixture("snapshot-p22.json");
    let agent_json = find_by_id(&raw["result"]["snapshot"]["agents"], "pane_id", "wJ:p1");
    let agent: AgentInfo = serde_json::from_value(agent_json.clone()).unwrap();
    assert_modeled_fields_match(
        agent_json,
        &agent,
        AGENT_INFO_OPTIONAL_FIELDS,
        "agent wJ:p1 (p22)",
    );
}

// ---------------------------------------------------------------------------
// Fix round 2 / finding 2（2b／2c）：表格化的必填欄位測試——「已建模欄位」由
// `T::default()` 的 JSON key 自動取得，「schema 必填欄位」讀 schema 的 `required` 陣列，
// 交集逐一從合法物件刪除後斷言解析失敗；差集（已建模但非必填）逐一刪除後斷言仍成功。
// 不為了偵測缺席而多建模 schema 有、我們用不到的欄位（例如 `PaneInfo`／`AgentInfo` 的
// `terminal_id`）——design D13 的適用範圍就是「已建模」的必填欄位（指揮官裁決 2a）。
// ---------------------------------------------------------------------------

#[test]
fn workspace_info_required_fields_are_all_enforced() {
    let valid = serde_json::json!({
        "workspace_id": "wJ", "label": "LABEL", "number": 1, "active_tab_id": "wJ:t1",
        "agent_status": "idle", "focused": false, "pane_count": 1, "tab_count": 1
    });
    for schema_file in SCHEMA_FILES {
        assert_required_fields_detected::<WorkspaceInfo>(
            schema_file,
            "success_response",
            "WorkspaceInfo",
            valid.clone(),
        );
    }
}

#[test]
fn tab_info_required_fields_are_all_enforced() {
    let valid = serde_json::json!({
        "tab_id": "wJ:t1", "workspace_id": "wJ", "number": 1, "label": "LABEL",
        "agent_status": "idle", "focused": false, "pane_count": 1
    });
    for schema_file in SCHEMA_FILES {
        assert_required_fields_detected::<TabInfo>(
            schema_file,
            "success_response",
            "TabInfo",
            valid.clone(),
        );
    }
}

#[test]
fn pane_info_required_fields_are_all_enforced() {
    let valid = serde_json::json!({
        "pane_id": "wJ:p1", "workspace_id": "wJ", "tab_id": "wJ:t1",
        "agent": "claude", "agent_status": "idle", "title": "T", "terminal_title": "TT",
        "cwd": "/x", "label": "L", "focused": false, "revision": 1
    });
    for schema_file in SCHEMA_FILES {
        assert_required_fields_detected::<PaneInfo>(
            schema_file,
            "success_response",
            "PaneInfo",
            valid.clone(),
        );
    }
}

#[test]
fn agent_info_required_fields_are_all_enforced() {
    let valid = serde_json::json!({
        "agent": "claude", "pane_id": "wJ:p1", "workspace_id": "wJ", "tab_id": "wJ:t1",
        "agent_status": "idle"
    });
    for schema_file in SCHEMA_FILES {
        assert_required_fields_detected::<AgentInfo>(
            schema_file,
            "success_response",
            "AgentInfo",
            valid.clone(),
        );
    }
}

/// Codex task review（task 4.2）fix round 1 finding 3：`ErrorBody` 拿掉容器層
/// `#[serde(default)]` 後（design D13），`code`／`message` 兩個 schema 必填欄位缺席時應該
/// 反序列化失敗，不是被悄悄補成空字串。
#[test]
fn error_body_required_fields_are_all_enforced() {
    let valid = serde_json::json!({ "code": "pane_not_found", "message": "pane wD:pS not found" });
    for schema_file in SCHEMA_FILES {
        assert_required_fields_detected::<ErrorBody>(
            schema_file,
            "error_response",
            "ErrorBody",
            valid.clone(),
        );
    }
}

#[test]
fn session_snapshot_required_fields_are_all_enforced() {
    // fix round 2 / finding 2（2c）：`SessionSnapshot` 原本容器層也有 `#[serde(default)]`，
    // 跟四個 Info 型別在 fix round 1 的修法不一致；已改成只在三個 `focused_*`
    // （schema 非必填、nullable）欄位上加逐欄 default。
    let valid = serde_json::json!({
        "version": "0.9.0", "protocol": 22,
        "workspaces": [], "tabs": [], "panes": [], "agents": [], "layouts": [],
        "focused_workspace_id": "wJ", "focused_tab_id": "wJ:t1", "focused_pane_id": "wJ:p1"
    });
    for schema_file in SCHEMA_FILES {
        assert_required_fields_detected::<SessionSnapshot>(
            schema_file,
            "success_response",
            "SessionSnapshot",
            valid.clone(),
        );
    }
}

#[test]
fn p22_and_p20_required_fields_agree_for_modeled_types() {
    // fix round 3 / finding 2：明確斷言兩份 schema 對這五個型別的 `required` 陣列相等；
    // 這是 5 個表格化測試「只依 p22 就代表 p20 也對」這個前提的守門測試，schema drift 時
    // 會在這裡先炸、印出差異，而不是靜默地讓表格化測試繼續各自對各自的 schema 全綠。
    for defs_name in [
        "WorkspaceInfo",
        "TabInfo",
        "PaneInfo",
        "AgentInfo",
        "SessionSnapshot",
    ] {
        let mut p22 = schema_required_fields("schema-p22.json", "success_response", defs_name);
        let mut p20 = schema_required_fields("schema-p20.json", "success_response", defs_name);
        p22.sort();
        p20.sort();
        assert_eq!(
            p22, p20,
            "{defs_name}: required array differs between protocol 22 and protocol 20 schemas"
        );
    }
}
