//! ui-language task 3.2（design D4）：訊息代碼目錄。每則訊息的繁中原文與 `{code, params}` 由
//! 同一個 `Message` 建構；`classify` 是 `text` 的反向，目錄內每一則都必須來回一致（防止兩邊漂移）。

use std::collections::BTreeMap;

use cockpit_core::{Message, MessageCode};
use serde_json::json;

fn params(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

fn s(v: &str) -> String {
    v.to_string()
}

/// （訊息, 繁中原文, 代碼, 參數）。
type Sample = (
    Message,
    &'static str,
    &'static str,
    Vec<(&'static str, &'static str)>,
);

/// 目錄內每一個代碼各一個樣本：（訊息, 繁中原文, 代碼, 參數）。
fn catalog() -> Vec<Sample> {
    vec![
        (
            Message::WslDistroNotRunning {
                distro: s("Ubuntu-24.04"),
            },
            "WSL 發行版 Ubuntu-24.04 未啟動",
            "wsl_distro_not_running",
            vec![("distro", "Ubuntu-24.04")],
        ),
        (
            Message::WslProbeFailed {
                detail: s("拒絕存取"),
            },
            "WSL 探測失敗：拒絕存取",
            "wsl_probe_failed",
            vec![("detail", "拒絕存取")],
        ),
        (
            Message::SnapshotFailed {
                detail: s("connection refused"),
            },
            "snapshot 失敗：connection refused",
            "snapshot_failed",
            vec![("detail", "connection refused")],
        ),
        (
            Message::SeedSnapshotFailed { detail: s("x") },
            "seed snapshot 失敗：x",
            "seed_snapshot_failed",
            vec![("detail", "x")],
        ),
        (
            Message::LifecycleSubscribeFailed { detail: s("x") },
            "L 訂閱建立失敗：x",
            "lifecycle_subscribe_failed",
            vec![("detail", "x")],
        ),
        (
            Message::StatusSubscribeFailed { detail: s("x") },
            "S 訂閱建立失敗：x",
            "status_subscribe_failed",
            vec![("detail", "x")],
        ),
        (
            Message::StatusResubscribeFailed { detail: s("x") },
            "S 重開失敗：x",
            "status_resubscribe_failed",
            vec![("detail", "x")],
        ),
        (
            Message::ProtocolUntested {
                protocol: s("23"),
                tested: s("20..=22"),
            },
            "HERDR protocol 23 不在已測範圍 20..=22",
            "protocol_untested",
            vec![("protocol", "23"), ("tested", "20..=22")],
        ),
        (
            Message::EventStreamEnded,
            "事件流結束",
            "event_stream_ended",
            vec![],
        ),
        (
            Message::EventConnectionError {
                label: s("L"),
                detail: s("broken pipe"),
            },
            "L 連線錯誤：broken pipe",
            "event_connection_error",
            vec![("label", "L"), ("detail", "broken pipe")],
        ),
        (
            Message::EventConnectionEnded { label: s("S") },
            "S 連線結束",
            "event_connection_ended",
            vec![("label", "S")],
        ),
        (
            Message::TaskStageReset {
                task: s("docs-1"),
                stage: s("Draft"),
                start: s("Spec"),
            },
            "task docs-1 的 stage「Draft」已不在 pipeline 的 stages 中，已退回起始 stage「Spec」",
            "task_stage_reset",
            vec![("task", "docs-1"), ("stage", "Draft"), ("start", "Spec")],
        ),
        (
            Message::DriftWorkspaceNotFound { id: s("wX") },
            "workspace wX 不存在",
            "drift_workspace_not_found",
            vec![("id", "wX")],
        ),
        (
            Message::DriftTabNotFound { id: s("wX:t9") },
            "tab wX:t9 不存在",
            "drift_tab_not_found",
            vec![("id", "wX:t9")],
        ),
        (
            Message::DriftPaneNotFound { id: s("wX:p9") },
            "pane wX:p9 不存在",
            "drift_pane_not_found",
            vec![("id", "wX:p9")],
        ),
        (
            Message::DriftRuntimeNotRegistered { id: s("win") },
            "runtime win 未登記",
            "drift_runtime_not_registered",
            vec![("id", "win")],
        ),
        (
            Message::EventPayloadUnparsable {
                event: s("pane_created"),
                detail: s("missing field `pane_id`"),
            },
            "pane_created payload 無法解析：missing field `pane_id`",
            "event_payload_unparsable",
            vec![
                ("event", "pane_created"),
                ("detail", "missing field `pane_id`"),
            ],
        ),
        (
            Message::Raw {
                text: s("os error 10061"),
            },
            "os error 10061",
            "raw",
            vec![("text", "os error 10061")],
        ),
    ]
}

/// 編譯期窮舉檢查：新增 `Message` 變體時這個 `match`（沒有萬用分支）不完整而編譯失敗，提醒補 `catalog()`；
/// 數量斷言則擋住「變體有列進 match、卻忘了補樣本」。對照啟動器的 `text_table_covers_every_variant`。
#[test]
fn catalog_covers_every_variant() {
    use Message::*;
    let table = catalog();
    for (variant, _, code, _) in &table {
        match variant {
            WslDistroNotRunning { .. }
            | WslProbeFailed { .. }
            | SnapshotFailed { .. }
            | SeedSnapshotFailed { .. }
            | LifecycleSubscribeFailed { .. }
            | StatusSubscribeFailed { .. }
            | StatusResubscribeFailed { .. }
            | ProtocolUntested { .. }
            | EventStreamEnded
            | EventConnectionError { .. }
            | EventConnectionEnded { .. }
            | TaskStageReset { .. }
            | DriftWorkspaceNotFound { .. }
            | DriftTabNotFound { .. }
            | DriftPaneNotFound { .. }
            | DriftRuntimeNotRegistered { .. }
            | EventPayloadUnparsable { .. }
            | Raw { .. } => {}
        }
        // 每個樣本的代碼唯一（一個變體一個代碼）。
        assert_eq!(
            table.iter().filter(|(_, _, c, _)| c == code).count(),
            1,
            "{code} 在目錄裡只能有一筆"
        );
    }
    // 18 個變體各一筆；數字不一致代表上面的 match 與樣本表對不上。
    assert_eq!(table.len(), 18);
}

#[test]
fn every_message_has_exact_text_code_and_params() {
    for (message, text, code, expected_params) in catalog() {
        assert_eq!(message.text(), text, "{code} 原文");
        assert_eq!(
            message.msg(),
            MessageCode {
                code: code.to_string(),
                params: params(&expected_params),
            },
            "{code} 代碼與參數"
        );
    }
}

#[test]
fn classify_is_the_inverse_of_text_for_every_message() {
    for (message, text, code, _) in catalog() {
        assert_eq!(Message::classify(text), message, "{code} 應該能從原文還原");
        assert_eq!(Message::classify(&message.text()), message);
    }
}

#[test]
fn unclassifiable_text_is_raw_with_text_param() {
    // herdr-client 的英文 thiserror 文字原樣到達 reason。
    let text = "failed to connect to the named pipe: The system cannot find the file specified.";
    let message = Message::classify(text);
    assert_eq!(message, Message::Raw { text: s(text) });
    assert_eq!(message.text(), text);
    assert_eq!(
        message.msg(),
        MessageCode {
            code: "raw".to_string(),
            params: params(&[("text", text)]),
        }
    );
    // 空字串也歸 raw，不 panic。
    assert_eq!(Message::classify(""), Message::Raw { text: s("") });
}

#[test]
fn classify_does_not_swallow_near_misses() {
    // 前綴相同但結構不符：不能硬湊成已知代碼。
    for text in [
        "WSL 發行版 Ubuntu-24.04",
        "HERDR protocol 23 不在已測範圍",
        "task t1 的 stage「A」已不在 pipeline 的 stages 中",
        "事件流結束了",
        // id 不能是空的或含空白（否則任意句子都會被當成「X 不存在」）。
        "workspace  不存在",
        "pane 不存在",
        "runtime 未登記",
        "tab a b 不存在",
        "payload 無法解析：x",
        "兩 個 payload 無法解析：x",
    ] {
        assert_eq!(
            Message::classify(text),
            Message::Raw { text: s(text) },
            "{text}"
        );
    }
}

#[test]
fn message_code_serializes_as_code_and_params() {
    let code = Message::TaskStageReset {
        task: s("t"),
        stage: s("a"),
        start: s("b"),
    }
    .msg();
    assert_eq!(
        serde_json::to_value(&code).unwrap(),
        json!({"code": "task_stage_reset", "params": {"task": "t", "stage": "a", "start": "b"}})
    );
    // 無參數時 params 仍是空物件（前端不必判斷缺欄位）。
    assert_eq!(
        serde_json::to_value(Message::EventStreamEnded.msg()).unwrap(),
        json!({"code": "event_stream_ended", "params": {}})
    );
    // 可還原。
    let back: MessageCode = serde_json::from_value(json!({"code": "x"})).unwrap();
    assert_eq!(back.code, "x");
    assert!(back.params.is_empty());
}
