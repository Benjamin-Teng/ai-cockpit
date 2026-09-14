//! Task 3.3 驗收測試：把序列化的 request 與解析用的 fixture 拿去對 HERDR 官方 schema 驗證
//! （spec `herdr-observer-types`「兩個 protocol 版本的合約」；design D8）。
//!
//! schema 檔頂層是 `{ "$schema", "protocol", "schema_version", "schemas": { request,
//! success_response, error_response, event, subscription_event }, "title" }`，內部 `$ref`
//! 一律是文件絕對指標（例如 `#/schemas/success_response/$defs/SessionSnapshot`），所以不能把
//! `schemas.request` 單獨抽出來編譯。做法（D8）：複製整份文件，在頂層插入
//! `"$ref": "#/schemas/<root>"` 來選根，再整份餵給 `jsonschema::draft202012::new`——`$ref`
//! 與同一份文件裡其他 `$ref` 一樣，都相對這份文件的（合成）base URI 解析，所以巢狀引用不受
//! 影響。這個做法確實可行，兩份 schema 都能這樣編譯出 5 種根 validator。

use std::str::FromStr;

use herdr_client::types::{
    EventEnvelope, EventsSubscribeParams, PaneReadParams, ReadFormat, ReadSource, RequestEnvelope,
    Subscription,
};

mod common;
use common::{assert_valid, load_json, read_lines, schemas, validator_for_root};

fn sample_requests() -> Vec<(&'static str, serde_json::Value)> {
    let snapshot_req = RequestEnvelope {
        id: "1",
        method: "session.snapshot",
        params: &serde_json::json!({}),
    };

    let pane_read_params = PaneReadParams {
        pane_id: "wJ:p1".to_string(),
        source: ReadSource::Recent,
        format: None,
        lines: Some(200),
        strip_ansi: None,
    };
    let pane_read_req = RequestEnvelope {
        id: "2",
        method: "pane.read",
        params: &pane_read_params,
    };

    // fix round 1 / finding 1：至少一筆 `format` 有實際值（`ReadFormat::Text`）的
    // `pane.read` request，確認它序列化成 `"format":"text"` 且通過兩份 schema。
    let pane_read_with_format_params = PaneReadParams {
        pane_id: "wJ:p2".to_string(),
        source: ReadSource::Visible,
        format: Some(ReadFormat::Text),
        lines: None,
        strip_ansi: Some(true),
    };
    let pane_read_with_format_req = RequestEnvelope {
        id: "4",
        method: "pane.read",
        params: &pane_read_with_format_params,
    };

    let mut subscriptions = Subscription::all_lifecycle();
    assert_eq!(subscriptions.len(), 24);
    for pane_id in ["wJ:p1", "wJ:p2", "wK:p1"] {
        subscriptions.push(Subscription::PaneAgentStatusChanged {
            pane_id: pane_id.to_string(),
        });
    }
    let subscribe_params = EventsSubscribeParams { subscriptions };
    let subscribe_req = RequestEnvelope {
        id: "3",
        method: "events.subscribe",
        params: &subscribe_params,
    };

    vec![
        (
            "session.snapshot",
            serde_json::to_value(&snapshot_req).unwrap(),
        ),
        ("pane.read", serde_json::to_value(&pane_read_req).unwrap()),
        (
            "pane.read (format=text)",
            serde_json::to_value(&pane_read_with_format_req).unwrap(),
        ),
        (
            "events.subscribe",
            serde_json::to_value(&subscribe_req).unwrap(),
        ),
    ]
}

#[test]
fn requests_validate_against_both_schemas() {
    let schemas = schemas();
    let p22_request = validator_for_root(&schemas.p22, "request");
    let p20_request = validator_for_root(&schemas.p20, "request");

    for (label, request) in sample_requests() {
        assert_valid(&p22_request, &request, &format!("{label} (protocol 22)"));
        assert_valid(&p20_request, &request, &format!("{label} (protocol 20)"));
    }
}

#[test]
fn pane_read_format_serializes_as_known_string() {
    // fix round 1 / finding 1：`ReadFormat::Text` 必須逐字序列化成 schema 認得的
    // `"text"`，不是任意字串；`ReadFormat` 只有 `Text`／`Ansi` 兩個變體，其他字串
    // （例如 "html"）在編譯期就不可能被指定進 `PaneReadParams.format`。
    let value = serde_json::to_value(ReadFormat::Text).unwrap();
    assert_eq!(value, serde_json::Value::String("text".to_string()));
}

#[test]
fn snapshot_fixtures_validate_as_success_response() {
    let schemas = schemas();
    let p22_response = validator_for_root(&schemas.p22, "success_response");
    let p20_response = validator_for_root(&schemas.p20, "success_response");

    let snapshot_p22 = load_json("snapshot-p22.json");
    assert_valid(&p22_response, &snapshot_p22, "snapshot-p22.json");

    let snapshot_p20 = load_json("snapshot-p20.json");
    assert_valid(&p20_response, &snapshot_p20, "snapshot-p20.json");
}

#[test]
fn event_fixtures_validate_as_event_or_subscription_event() {
    let schemas = schemas();
    let p22_event = validator_for_root(&schemas.p22, "event");
    let p20_event = validator_for_root(&schemas.p20, "event");
    let p22_subscription_event = validator_for_root(&schemas.p22, "subscription_event");
    let p20_subscription_event = validator_for_root(&schemas.p20, "subscription_event");

    let lifecycle_lines = read_lines("events-lifecycle-p20.ndjson");
    assert!(!lifecycle_lines.is_empty());
    for (i, line) in lifecycle_lines.iter().enumerate() {
        let instance: serde_json::Value = serde_json::from_str(line)
            .unwrap_or_else(|e| panic!("events-lifecycle-p20.ndjson line {i}: {e}"));
        assert_valid(
            &p22_event,
            &instance,
            &format!("events-lifecycle-p20.ndjson line {i} (protocol 22 event)"),
        );
        assert_valid(
            &p20_event,
            &instance,
            &format!("events-lifecycle-p20.ndjson line {i} (protocol 20 event)"),
        );
    }

    let status_lines = read_lines("events-status-p20.ndjson");
    assert!(!status_lines.is_empty());
    for (i, line) in status_lines.iter().enumerate() {
        let instance: serde_json::Value = serde_json::from_str(line)
            .unwrap_or_else(|e| panic!("events-status-p20.ndjson line {i}: {e}"));
        assert_valid(
            &p22_subscription_event,
            &instance,
            &format!("events-status-p20.ndjson line {i} (protocol 22 subscription_event)"),
        );
        assert_valid(
            &p20_subscription_event,
            &instance,
            &format!("events-status-p20.ndjson line {i} (protocol 20 subscription_event)"),
        );
    }

    // task 5.1 fix round 1 finding 2（Codex task review，medium）：這份 p22（Windows 端真機，
    // `herdr-client/examples/capture_events.rs` 擷取）生命週期事件 fixture 已經交付、進了
    // repo，改成無條件 `read_lines`——缺檔（漏加、誤刪、沒打包）就直接讓測試失敗，不再用
    // `Path::exists` 悄悄跳過（原本那樣寫，CI 在 fixture 消失時仍然全綠，等於白測）。
    let p22_lifecycle_lines = read_lines("events-lifecycle-p22.ndjson");
    assert!(!p22_lifecycle_lines.is_empty());
    for (i, line) in p22_lifecycle_lines.iter().enumerate() {
        let instance: serde_json::Value = serde_json::from_str(line)
            .unwrap_or_else(|e| panic!("events-lifecycle-p22.ndjson line {i}: {e}"));
        assert_valid(
            &p22_event,
            &instance,
            &format!("events-lifecycle-p22.ndjson line {i} (protocol 22 event)"),
        );
        assert_valid(
            &p20_event,
            &instance,
            &format!("events-lifecycle-p22.ndjson line {i} (protocol 20 event)"),
        );
    }
}

/// `events-status-p22.ndjson`（Windows 端真機，`herdr-client/examples/capture_events.rs`
/// 擷取，2 行 `pane.agent_status_changed`：`wJ:p1` 從 `done` 變 `working`）：task 5.1 fix
/// round 1 finding 2 時這份 fixture 還沒擷取到內容，先用 `#[ignore]` 佔位；擷取到之後
/// （fix round 2）比照 lifecycle 那段改成無條件執行，缺檔就直接讓測試失敗。
#[test]
fn event_status_p22_fixture_validates_as_subscription_event() {
    let schemas = schemas();
    let p22_subscription_event = validator_for_root(&schemas.p22, "subscription_event");
    let p20_subscription_event = validator_for_root(&schemas.p20, "subscription_event");

    let lines = read_lines("events-status-p22.ndjson");
    assert!(!lines.is_empty());
    for (i, line) in lines.iter().enumerate() {
        let instance: serde_json::Value = serde_json::from_str(line)
            .unwrap_or_else(|e| panic!("events-status-p22.ndjson line {i}: {e}"));
        assert_valid(
            &p22_subscription_event,
            &instance,
            &format!("events-status-p22.ndjson line {i} (protocol 22 subscription_event)"),
        );
        assert_valid(
            &p20_subscription_event,
            &instance,
            &format!("events-status-p22.ndjson line {i} (protocol 20 subscription_event)"),
        );
    }
}

#[test]
fn contract_rejects_invalid_request() {
    let schemas = schemas();
    let p22_request = validator_for_root(&schemas.p22, "request");

    let bad_request = serde_json::json!({
        "id": "1",
        "method": "session.snapshot_typo",
        "params": {}
    });
    assert!(
        p22_request.validate(&bad_request).is_err(),
        "a misspelled method must not validate as a request"
    );

    // 順手確認我們自己序列化出來的合法 request 確實會過（避免上面那個 assert 只是因為
    // validator 本身壞掉、永遠回 Err）。
    let good_request = serde_json::json!({
        "id": "1",
        "method": "session.snapshot",
        "params": {}
    });
    assert!(p22_request.validate(&good_request).is_ok());
}

#[test]
fn event_envelope_and_subscription_event_kind_agree_on_status_fixture() {
    // 附帶檢查：events-status-p20.ndjson 的 event 名稱都在 `SubscriptionEventKind` 內、
    // 都可以先解析成 `EventEnvelope`，銜接 contract 測試與 events.rs 的分軌測試。
    for line in read_lines("events-status-p20.ndjson") {
        let envelope: EventEnvelope = serde_json::from_str(&line).unwrap();
        assert!(herdr_client::types::SubscriptionEventKind::from_str(&envelope.event).is_ok());
    }
}
