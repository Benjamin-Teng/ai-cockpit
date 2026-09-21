//! RED→GREEN 驗收測試（Task 1.2、live-output task 2.1）：AgentStatus 序列化、窮舉、
//! RuntimeError、RuntimeEvents drop、PaneOutput／OutputFormat 序列化。

use cockpit_core::{AgentStatus, OutputFormat, PaneId, PaneOutput, RuntimeError, RuntimeEvents};

#[test]
fn agent_status_serializes_lowercase() {
    let cases: [(AgentStatus, &str); 5] = [
        (AgentStatus::Idle, "idle"),
        (AgentStatus::Working, "working"),
        (AgentStatus::Blocked, "blocked"),
        (AgentStatus::Done, "done"),
        (AgentStatus::Unknown, "unknown"),
    ];

    for (status, expected) in cases {
        let json = serde_json::to_string(&status).expect("serialize AgentStatus");
        assert_eq!(json, format!("\"{expected}\""));

        let round_tripped: AgentStatus =
            serde_json::from_str(&json).expect("deserialize AgentStatus");
        assert_eq!(round_tripped, status);
    }
}

#[test]
fn agent_status_has_exactly_five_values_and_no_completed() {
    let all = [
        AgentStatus::Idle,
        AgentStatus::Working,
        AgentStatus::Blocked,
        AgentStatus::Done,
        AgentStatus::Unknown,
    ];

    let mut names = std::collections::BTreeSet::new();
    for status in all {
        // 沒有 `_` 分支：未來多加變體會讓這裡編譯失敗，逼著回來更新窮舉。
        let name = match status {
            AgentStatus::Idle => "idle",
            AgentStatus::Working => "working",
            AgentStatus::Blocked => "blocked",
            AgentStatus::Done => "done",
            AgentStatus::Unknown => "unknown",
        };
        names.insert(name.to_string());
    }

    assert_eq!(names.len(), 5);
    assert!(!names.contains("completed"));
    assert!(!names.contains("finished"));
}

#[test]
fn runtime_error_retry_after_only_for_unavailable() {
    use std::time::Duration;

    let unavailable = RuntimeError::Unavailable {
        reason: "herdr 尚未啟動".to_string(),
        retry_after: Duration::from_secs(5),
    };
    assert_eq!(unavailable.retry_after(), Some(Duration::from_secs(5)));
    assert_eq!(unavailable.to_string(), "herdr 尚未啟動");

    let failed = RuntimeError::Failed("連線中斷：peer reset".to_string());
    assert_eq!(failed.retry_after(), None);
    assert_eq!(failed.to_string(), "連線中斷：peer reset");
}

#[test]
fn pane_output_serializes_format_as_text() {
    let output = PaneOutput {
        format: OutputFormat::Text,
        text: "line 1\nline 2".to_string(),
        truncated: false,
    };

    let json = serde_json::to_value(&output).expect("serialize PaneOutput");
    assert_eq!(json["format"], "text");
    assert_eq!(json["text"], "line 1\nline 2");
    assert_eq!(json["truncated"], false);
}

#[test]
fn pane_not_found_has_no_retry_after() {
    let pane_id = PaneId::new("p9");
    let error = RuntimeError::PaneNotFound {
        pane_id: pane_id.clone(),
    };

    assert_eq!(error.retry_after(), None);
    assert!(error.to_string().contains(pane_id.as_str()));
}

#[tokio::test]
async fn runtime_events_drop_aborts_reader_task() {
    use tokio::sync::oneshot;

    struct DropGuard(Option<oneshot::Sender<()>>);

    impl Drop for DropGuard {
        fn drop(&mut self) {
            if let Some(tx) = self.0.take() {
                let _ = tx.send(());
            }
        }
    }

    let (_tx, unstarted) = RuntimeEvents::channel();
    let (dropped_tx, dropped_rx) = oneshot::channel::<()>();

    let task = tokio::spawn(async move {
        let _guard = DropGuard(Some(dropped_tx));
        std::future::pending::<()>().await;
    });

    let events = unstarted.start(vec![task]);
    drop(events);

    let result = tokio::time::timeout(std::time::Duration::from_secs(2), dropped_rx).await;
    assert!(
        result.is_ok(),
        "RuntimeEvents 被 drop 後應該 abort reader task，讓其 guard 被 drop"
    );
}

#[test]
fn runtime_events_channel_has_fixed_capacity() {
    let (tx, _unstarted) = RuntimeEvents::channel();
    assert_eq!(tx.max_capacity(), RuntimeEvents::CAPACITY);
}
