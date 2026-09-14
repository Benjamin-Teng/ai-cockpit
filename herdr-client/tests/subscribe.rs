//! Task 4.3 驗收測試：`Client::subscribe`、`EventStream`、`IncomingEvent`（design D4、D6、
//! D10、D12；spec `herdr-event-subscription`）。全部走真實 transport（假 HERDR，見
//! `herdr_client::testing::FakeHerdr`），不需要真機 HERDR。

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;

use herdr_client::client::{Client, IncomingEvent, RequestError, StreamError};
use herdr_client::connector::{ConnectError, Connector, NdjsonStream};
use herdr_client::testing::{FakeHerdr, FakeHerdrConfig, Step, SubscribeMatcher};
use herdr_client::types::{EventKind, Subscription, SubscriptionEventKind};

mod common;

fn client_for(fake: &FakeHerdr) -> Client {
    Client::new(Arc::from(fake.connector()))
}

/// 取出一條連線收到的第一行 request 裡 `params.subscriptions` 陣列。
fn subscriptions_from_request_line(line: &str) -> Vec<serde_json::Value> {
    let value: serde_json::Value = serde_json::from_str(line).expect("request 行應為合法 JSON");
    value
        .get("params")
        .and_then(|p| p.get("subscriptions"))
        .and_then(serde_json::Value::as_array)
        .expect("params.subscriptions 應為陣列")
        .clone()
}

// ---------------------------------------------------------------------------
// 訂閱建立（spec「訂閱建立」）
// ---------------------------------------------------------------------------

#[tokio::test]
async fn subscribe_lifecycle_24_sends_expected_types() {
    let fake = FakeHerdr::start(FakeHerdrConfig::new())
        .await
        .expect("啟動假 HERDR 失敗");
    let client = client_for(&fake);

    let stream = client
        .subscribe(&Subscription::all_lifecycle())
        .await
        .expect("24 種生命週期訂閱應成功");
    drop(stream);

    let received = fake.received();
    assert_eq!(received.len(), 1, "應該只建立一條連線");
    let subscriptions = subscriptions_from_request_line(&received[0][0]);
    assert_eq!(subscriptions.len(), 24, "應該送出 24 筆訂閱");

    for sub in &subscriptions {
        let obj = sub.as_object().expect("訂閱項目應為物件");
        assert_eq!(obj.len(), 1, "無參數訂閱應只有 type 欄位: {sub}");
    }

    let expected_types: HashSet<&str> = [
        "workspace.created",
        "workspace.updated",
        "workspace.metadata_updated",
        "workspace.renamed",
        "workspace.moved",
        "workspace.reordered",
        "workspace.closed",
        "workspace.focused",
        "worktree.created",
        "worktree.opened",
        "worktree.removed",
        "tab.created",
        "tab.closed",
        "tab.focused",
        "tab.renamed",
        "tab.moved",
        "pane.created",
        "pane.closed",
        "pane.updated",
        "pane.focused",
        "pane.moved",
        "pane.exited",
        "pane.agent_detected",
        "layout.updated",
    ]
    .into_iter()
    .collect();
    assert_eq!(expected_types.len(), 24);
    let actual_types: HashSet<&str> = subscriptions
        .iter()
        .map(|s| {
            s.get("type")
                .and_then(serde_json::Value::as_str)
                .expect("每筆訂閱都應有 type")
        })
        .collect();
    assert_eq!(
        actual_types, expected_types,
        "24 筆訂閱的 type 應恰好是這 24 種點號命名，且不重複"
    );
}

#[tokio::test]
async fn subscribe_per_pane_n_sends_pane_ids() {
    let fake = FakeHerdr::start(FakeHerdrConfig::new())
        .await
        .expect("啟動假 HERDR 失敗");
    let client = client_for(&fake);

    let pane_ids = ["wD:p1", "wD:p2", "wD:p3"];
    let subs: Vec<Subscription> = pane_ids
        .iter()
        .map(|id| Subscription::PaneAgentStatusChanged {
            pane_id: (*id).to_string(),
        })
        .collect();

    let stream = client
        .subscribe(&subs)
        .await
        .expect("每 pane 的 agent 狀態訂閱應成功");
    drop(stream);

    let received = fake.received();
    assert_eq!(received.len(), 1, "應該只建立一條連線");
    let subscriptions = subscriptions_from_request_line(&received[0][0]);
    assert_eq!(subscriptions.len(), pane_ids.len(), "應該送出 N 筆訂閱");

    let actual_pane_ids: Vec<&str> = subscriptions
        .iter()
        .map(|s| {
            assert_eq!(
                s.get("type").and_then(serde_json::Value::as_str),
                Some("pane.agent_status_changed")
            );
            assert!(
                s.get("agent_status").is_none(),
                "不應含 agent_status 過濾欄位: {s}"
            );
            s.get("pane_id")
                .and_then(serde_json::Value::as_str)
                .expect("應有 pane_id")
        })
        .collect();
    assert_eq!(actual_pane_ids, pane_ids);
}

#[tokio::test]
async fn subscribe_missing_pane_is_remote_and_no_stream() {
    let config = FakeHerdrConfig::new().with_failing_probe_pane_ids(["wD:pMissing"]);
    let fake = FakeHerdr::start(config).await.expect("啟動假 HERDR 失敗");
    let client = client_for(&fake);

    let subs = vec![Subscription::PaneAgentStatusChanged {
        pane_id: "wD:pMissing".to_string(),
    }];
    let err = client
        .subscribe(&subs)
        .await
        .expect_err("探測失敗的訂閱應該回傳 Err，不建立串流");

    // 全分支最終 review finding 5：唯一一筆訂閱探測失敗，序號固定是 1。
    let rendered = err.to_string();
    assert!(
        rendered.contains(":sub:1:probe"),
        "to_string() 應含 probe 後綴 id，實際: {rendered}"
    );

    match err {
        RequestError::Remote {
            code,
            message,
            response_id,
        } => {
            assert_eq!(code, "pane_not_found");
            assert!(
                message.contains("wD:pMissing"),
                "訊息應含 pane id: {message}"
            );
            assert!(
                response_id.contains(":sub:1:probe"),
                "response_id 應含 probe 後綴，實際: {response_id}"
            );
        }
        other => panic!("預期 RequestError::Remote，實際: {other:?}"),
    }
}

/// design D12：好的訂閱排第一筆、失敗的排第二筆——假 HERDR 回的 error id 會是
/// `<request id>:sub:2:probe`，驗證 client 端「以 `<request id>:` 開頭」的比對不是只認
/// `subscribe_missing_pane_is_remote_and_no_stream` 那種序號固定是 1 的情境。
#[tokio::test]
async fn subscribe_probe_failure_with_id_suffix_is_remote() {
    let config = FakeHerdrConfig::new().with_failing_probe_pane_ids(["wD:pBad"]);
    let fake = FakeHerdr::start(config).await.expect("啟動假 HERDR 失敗");
    let client = client_for(&fake);

    let subs = vec![
        Subscription::PaneAgentStatusChanged {
            pane_id: "wD:pGood".to_string(),
        },
        Subscription::PaneAgentStatusChanged {
            pane_id: "wD:pBad".to_string(),
        },
    ];
    let err = client
        .subscribe(&subs)
        .await
        .expect_err("探測失敗應該回傳 Err");

    // 全分支最終 review finding 5：好的訂閱排第一筆、失敗的排第二筆，序號應為 2，且這個
    // 序號要出現在 `to_string()` 裡（不是只留在 `response_id` 欄位裡沒被 Display 用到）。
    let rendered = err.to_string();
    assert!(
        rendered.contains(":sub:2:probe"),
        "to_string() 應含 probe 後綴 id，實際: {rendered}"
    );

    match err {
        RequestError::Remote {
            code,
            message,
            response_id,
        } => {
            assert_eq!(code, "pane_not_found");
            assert!(message.contains("wD:pBad"), "訊息應含 pane id: {message}");
            assert!(
                response_id.contains(":sub:2:probe"),
                "response_id 應含 probe 後綴，實際: {response_id}"
            );
        }
        other => panic!("預期 RequestError::Remote（不是 Protocol），實際: {other:?}"),
    }
}

#[tokio::test]
async fn two_streams_are_independent() {
    let lifecycle_event = r#"{"event":"pane_created","data":{"pane_id":"wD:p1"}}"#;
    let per_pane_event_1 = r#"{"event":"pane.agent_status_changed","data":{"pane_id":"wD:p1","agent_status":"working"}}"#;
    let per_pane_event_2 =
        r#"{"event":"pane.agent_status_changed","data":{"pane_id":"wD:p1","agent_status":"idle"}}"#;

    let config = FakeHerdrConfig::new()
        .with_subscribe_rule(
            SubscribeMatcher::LifecycleOnly,
            vec![Step::Event(lifecycle_event.to_string()), Step::Hold],
        )
        .with_subscribe_rule(
            SubscribeMatcher::PerPane,
            vec![
                Step::Event(per_pane_event_1.to_string()),
                Step::Delay(Duration::from_millis(30)),
                Step::Event(per_pane_event_2.to_string()),
                Step::Hold,
            ],
        );
    let fake = FakeHerdr::start(config).await.expect("啟動假 HERDR 失敗");
    let client = client_for(&fake);

    let mut lifecycle_stream = client
        .subscribe(&Subscription::all_lifecycle())
        .await
        .expect("lifecycle 訂閱應成功");
    let mut per_pane_stream = client
        .subscribe(&[Subscription::PaneAgentStatusChanged {
            pane_id: "wD:p1".to_string(),
        }])
        .await
        .expect("per-pane 訂閱應成功");

    let lifecycle_first = lifecycle_stream
        .next()
        .await
        .expect("lifecycle 串流應收到事件")
        .expect("不應是錯誤");
    assert!(
        matches!(
            lifecycle_first,
            IncomingEvent::Lifecycle(EventKind::PaneCreated, _)
        ),
        "實際: {lifecycle_first:?}"
    );

    let per_pane_first = per_pane_stream
        .next()
        .await
        .expect("per-pane 串流應收到事件")
        .expect("不應是錯誤");
    match per_pane_first {
        IncomingEvent::PerPane(SubscriptionEventKind::PaneAgentStatusChanged, data) => {
            assert_eq!(data["agent_status"], "working");
        }
        other => panic!("預期 PerPane，實際: {other:?}"),
    }

    // 關閉 lifecycle 那條連線，per-pane 那條應該不受影響、繼續收得到後續事件。
    drop(lifecycle_stream);

    let per_pane_second = per_pane_stream
        .next()
        .await
        .expect("關閉另一條連線後，這條串流仍應收到事件")
        .expect("不應是錯誤");
    match per_pane_second {
        IncomingEvent::PerPane(SubscriptionEventKind::PaneAgentStatusChanged, data) => {
            assert_eq!(data["agent_status"], "idle");
        }
        other => panic!("預期 PerPane，實際: {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// 事件分軌（spec「事件分軌」）
// ---------------------------------------------------------------------------

#[tokio::test]
async fn events_route_to_lifecycle_per_pane_unknown() {
    let unknown_line = r#"{"event":"pane_teleported","data":{"pane_id":"wD:p1"}}"#;
    let lifecycle_line = r#"{"event":"pane_created","data":{"pane_id":"wD:p1"}}"#;
    let per_pane_line = r#"{"event":"pane.agent_status_changed","data":{"pane_id":"wD:p1"}}"#;

    let config = FakeHerdrConfig::new().with_subscribe_script(vec![
        Step::Event(unknown_line.to_string()),
        Step::Event(lifecycle_line.to_string()),
        Step::Event(per_pane_line.to_string()),
        Step::Close,
    ]);
    let fake = FakeHerdr::start(config).await.expect("啟動假 HERDR 失敗");
    let client = client_for(&fake);

    let mut stream = client
        .subscribe(&Subscription::all_lifecycle())
        .await
        .expect("訂閱應成功");

    let first = stream
        .next()
        .await
        .expect("應收到未知事件")
        .expect("不應是錯誤");
    match first {
        IncomingEvent::Unknown { event, data } => {
            assert_eq!(event, "pane_teleported");
            assert_eq!(data["pane_id"], "wD:p1");
        }
        other => panic!("預期 Unknown，實際: {other:?}"),
    }

    let second = stream
        .next()
        .await
        .expect("未知事件後應該繼續正常產生已知事件")
        .expect("不應是錯誤");
    assert!(
        matches!(second, IncomingEvent::Lifecycle(EventKind::PaneCreated, _)),
        "實際: {second:?}"
    );

    let third = stream
        .next()
        .await
        .expect("應收到每 pane 事件")
        .expect("不應是錯誤");
    assert!(
        matches!(
            third,
            IncomingEvent::PerPane(SubscriptionEventKind::PaneAgentStatusChanged, _)
        ),
        "實際: {third:?}"
    );

    assert!(stream.next().await.is_none(), "三筆事件後應該正常 EOF 結束");
}

// ---------------------------------------------------------------------------
// 壞行與結束（spec「壞行與結束」）
// ---------------------------------------------------------------------------

#[tokio::test]
async fn malformed_lines_are_skipped_with_warn() {
    let valid_line = r#"{"event":"pane_created","data":{"pane_id":"wD:p1"}}"#;
    let config = FakeHerdrConfig::new().with_subscribe_script(vec![
        Step::Malformed("not json at all".to_string()),
        // 缺 "event" 欄位（container 級 #[serde(default)] 不能讓這種壞行悄悄過關）。
        Step::Malformed(r#"{"data":{"pane_id":"wD:p1"}}"#.to_string()),
        Step::Event(valid_line.to_string()),
        Step::Close,
    ]);
    let fake = FakeHerdr::start(config).await.expect("啟動假 HERDR 失敗");
    let client = client_for(&fake);

    let mut stream = client
        .subscribe(&Subscription::all_lifecycle())
        .await
        .expect("訂閱應成功");

    let event = stream
        .next()
        .await
        .expect("應跳過兩行壞行、收到合法事件")
        .expect("不應是錯誤（壞行只記警告，不產生 Err 項）");
    assert!(
        matches!(event, IncomingEvent::Lifecycle(EventKind::PaneCreated, _)),
        "實際: {event:?}"
    );

    assert!(
        stream.next().await.is_none(),
        "壞行不應該再產生任何項目，合法事件後應該是正常 EOF 結束"
    );
}

/// Codex task review fix round 1 finding 1：兩份合約 schema 的 `event`／`subscription_event`
/// 根都把 `data` 指向恰好是「`oneOf` 且每個分支都是 `type: object`」的定義（見
/// `tests/fixtures/schema-p22.json`／`schema-p20.json` 的 `EventData`／
/// `SubscriptionEventData`）——`data` 是 `null`、陣列或純量值都違反合約，必須跟缺欄位一樣
/// 記警告後跳過，不能被當成合法事件延後到上層 payload 解析才失敗。每個案例都用「壞行後接
/// 一筆合法事件」的形式，斷言 `next()` 只產生合法那一筆。
async fn assert_bad_event_line_is_skipped(bad_line: &str) {
    let valid_line = r#"{"event":"pane_created","data":{"pane_id":"wD:p1"}}"#;
    let config = FakeHerdrConfig::new().with_subscribe_script(vec![
        Step::Malformed(bad_line.to_string()),
        Step::Event(valid_line.to_string()),
        Step::Close,
    ]);
    let fake = FakeHerdr::start(config).await.expect("啟動假 HERDR 失敗");
    let client = client_for(&fake);

    let mut stream = client
        .subscribe(&Subscription::all_lifecycle())
        .await
        .expect("訂閱應成功");

    let event = stream
        .next()
        .await
        .unwrap_or_else(|| panic!("應跳過壞行、收到合法事件（壞行: {bad_line}）"))
        .unwrap_or_else(|e| {
            panic!("不應是錯誤（壞行只記警告，不產生 Err 項），實際: {e}（壞行: {bad_line}）")
        });
    assert!(
        matches!(event, IncomingEvent::Lifecycle(EventKind::PaneCreated, _)),
        "實際: {event:?}（壞行: {bad_line}）"
    );

    assert!(
        stream.next().await.is_none(),
        "壞行不應該再產生任何項目，合法事件後應該是正常 EOF 結束（壞行: {bad_line}）"
    );
}

#[tokio::test]
async fn malformed_data_null_is_skipped() {
    assert_bad_event_line_is_skipped(r#"{"event":"pane_created","data":null}"#).await;
}

#[tokio::test]
async fn malformed_data_array_is_skipped() {
    assert_bad_event_line_is_skipped(r#"{"event":"pane_created","data":[1,2,3]}"#).await;
}

#[tokio::test]
async fn malformed_data_number_is_skipped() {
    assert_bad_event_line_is_skipped(r#"{"event":"pane_created","data":42}"#).await;
}

#[tokio::test]
async fn malformed_data_string_is_skipped() {
    assert_bad_event_line_is_skipped(r#"{"event":"pane_created","data":"oops"}"#).await;
}

#[tokio::test]
async fn malformed_event_non_string_is_skipped() {
    assert_bad_event_line_is_skipped(r#"{"event":42,"data":{"pane_id":"wD:p1"}}"#).await;
}

/// design D4：分軌是查「已知名稱集合」，不是查有沒有點號——含點號但不在
/// `SubscriptionEventKind` 3 種已知值內的名稱，一樣要落進 `Unknown`。跟
/// `unknown_event_name_without_dot_is_unknown` 互補，各自守住分軌邏輯的兩側。
#[tokio::test]
async fn unknown_event_name_with_dot_is_unknown() {
    let config = FakeHerdrConfig::new().with_subscribe_script(vec![
        Step::Event(r#"{"event":"pane.teleported","data":{"pane_id":"wD:p1"}}"#.to_string()),
        Step::Close,
    ]);
    let fake = FakeHerdr::start(config).await.expect("啟動假 HERDR 失敗");
    let client = client_for(&fake);

    let mut stream = client
        .subscribe(&Subscription::all_lifecycle())
        .await
        .expect("訂閱應成功");

    let event = stream
        .next()
        .await
        .expect("應收到事件")
        .expect("不應是錯誤");
    match event {
        IncomingEvent::Unknown { event, data } => {
            assert_eq!(event, "pane.teleported");
            assert_eq!(data["pane_id"], "wD:p1");
        }
        other => panic!("預期 Unknown，實際: {other:?}"),
    }
}

#[tokio::test]
async fn unknown_event_name_without_dot_is_unknown() {
    let config = FakeHerdrConfig::new().with_subscribe_script(vec![
        Step::Event(r#"{"event":"pane_teleported","data":{"pane_id":"wD:p1"}}"#.to_string()),
        Step::Close,
    ]);
    let fake = FakeHerdr::start(config).await.expect("啟動假 HERDR 失敗");
    let client = client_for(&fake);

    let mut stream = client
        .subscribe(&Subscription::all_lifecycle())
        .await
        .expect("訂閱應成功");

    let event = stream
        .next()
        .await
        .expect("應收到事件")
        .expect("不應是錯誤");
    match event {
        IncomingEvent::Unknown { event, data } => {
            assert_eq!(event, "pane_teleported");
            assert_eq!(data["pane_id"], "wD:p1");
        }
        other => panic!("預期 Unknown，實際: {other:?}"),
    }
}

#[tokio::test]
async fn eof_ends_stream_without_error() {
    let config = FakeHerdrConfig::new().with_subscribe_script(vec![Step::Close]);
    let fake = FakeHerdr::start(config).await.expect("啟動假 HERDR 失敗");
    let client = client_for(&fake);

    let mut stream = client
        .subscribe(&Subscription::all_lifecycle())
        .await
        .expect("訂閱應成功");

    let outcome = stream.next().await;
    assert!(
        outcome.is_none(),
        "正常 EOF 應該回 None，不應有錯誤項，實際: {outcome:?}"
    );
}

#[tokio::test]
async fn stream_after_end_keeps_returning_none() {
    let config = FakeHerdrConfig::new().with_subscribe_script(vec![Step::Close]);
    let fake = FakeHerdr::start(config).await.expect("啟動假 HERDR 失敗");
    let client = client_for(&fake);

    let mut stream = client
        .subscribe(&Subscription::all_lifecycle())
        .await
        .expect("訂閱應成功");

    assert!(stream.next().await.is_none(), "第一次讀到 EOF 應該回 None");
    assert!(
        stream.next().await.is_none(),
        "結束後再呼叫仍應該回 None（design D6）"
    );
    assert!(stream.next().await.is_none(), "第三次仍應該是 None");
}

#[cfg(windows)]
#[tokio::test]
async fn io_error_is_reported_then_stream_ends() {
    let config = FakeHerdrConfig::new().with_subscribe_script(vec![Step::Abort]);
    let fake = FakeHerdr::start(config).await.expect("啟動假 HERDR 失敗");
    let client = client_for(&fake);

    let mut stream = client
        .subscribe(&Subscription::all_lifecycle())
        .await
        .expect("訂閱應成功");

    let first = tokio::time::timeout(Duration::from_secs(3), stream.next())
        .await
        .expect("Abort 後應該在時限內有結果（不應該卡住）");
    match first {
        Some(Err(StreamError::Io(io_err))) => {
            assert!(!io_err.to_string().is_empty(), "原因文字不應為空");
        }
        other => panic!("預期 Some(Err(StreamError::Io(_)))，實際: {other:?}"),
    }

    let second = tokio::time::timeout(Duration::from_secs(3), stream.next())
        .await
        .expect("回報錯誤之後應該在時限內結束");
    assert!(
        second.is_none(),
        "回報錯誤之後再呼叫應該回 None，實際: {second:?}"
    );
}

/// unix 上 `Step::Abort` 降級為與 `Step::Close` 相同的乾淨 EOF（平台限制，見
/// `testing::connection` 的 `BestEffortAbort` 文件註解）；這裡把它當成明確記錄下來的行為，
/// 而不是放著一個在 unix 上會失敗的斷言。
#[cfg(unix)]
#[tokio::test]
async fn io_error_degrades_to_eof_on_unix() {
    let config = FakeHerdrConfig::new().with_subscribe_script(vec![Step::Abort]);
    let fake = FakeHerdr::start(config).await.expect("啟動假 HERDR 失敗");
    let client = client_for(&fake);

    let mut stream = client
        .subscribe(&Subscription::all_lifecycle())
        .await
        .expect("訂閱應成功");

    let outcome = tokio::time::timeout(Duration::from_secs(3), stream.next())
        .await
        .expect("Abort 後應該在時限內有結果（不應該卡住）");
    assert!(
        outcome.is_none(),
        "unix 上 Abort 應該降級為乾淨 EOF，實際: {outcome:?}"
    );
}

/// 還沒讀到 `subscription_started` 就 EOF：`FakeHerdr` 的 `events.subscribe` 路徑一定會先回
/// `subscription_started` 或探測失敗的 `error`（見 `testing::connection::handle_subscribe`），
/// 沒有「收到 request 後直接關閉、完全不回應」這個能力（`MethodResponse::CloseBeforeReply`
/// 只涵蓋一般 method）。這裡用一個最小的 `Connector`／`NdjsonStream` test double 模擬這個情境，
/// 比照 `tests/request.rs` 既有的 test double 手法（brief：假 HERDR 缺能力時用最小
/// workaround，不改假 HERDR 本身）。
struct CloseBeforeStartedConnector;

#[async_trait]
impl Connector for CloseBeforeStartedConnector {
    async fn connect(&self) -> Result<Box<dyn NdjsonStream>, ConnectError> {
        Ok(Box::new(CloseBeforeStartedStream))
    }

    fn describe(&self) -> String {
        "close-before-started-test-double".to_string()
    }
}

struct CloseBeforeStartedStream;

#[async_trait]
impl NdjsonStream for CloseBeforeStartedStream {
    async fn send_line(&mut self, _line: &str) -> std::io::Result<()> {
        Ok(())
    }

    async fn recv_line(&mut self) -> std::io::Result<Option<String>> {
        Ok(None)
    }
}

#[tokio::test]
async fn subscribe_closed_before_started_is_io_error() {
    let client = Client::new(Arc::new(CloseBeforeStartedConnector));

    let err = client
        .subscribe(&Subscription::all_lifecycle())
        .await
        .expect_err("還沒讀到 subscription_started 就 EOF 應該回傳 Err");

    match err {
        RequestError::Io(io_err) => {
            assert_eq!(io_err.kind(), std::io::ErrorKind::UnexpectedEof);
            assert!(
                io_err.to_string().contains("subscription_started"),
                "原因文字應提到 subscription_started，實際: {io_err}"
            );
        }
        other => panic!("預期 RequestError::Io(UnexpectedEof)，實際: {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// 指揮官加碼：request 行同時通過兩份 schema fixture 的 request 根 schema。
// ---------------------------------------------------------------------------

#[tokio::test]
async fn subscribe_request_line_validates_against_both_schemas() {
    let fake = FakeHerdr::start(FakeHerdrConfig::new())
        .await
        .expect("啟動假 HERDR 失敗");
    let client = client_for(&fake);

    let _lifecycle_stream = client
        .subscribe(&Subscription::all_lifecycle())
        .await
        .expect("lifecycle 訂閱應成功");
    let _per_pane_stream = client
        .subscribe(&[Subscription::PaneAgentStatusChanged {
            pane_id: "wD:p1".to_string(),
        }])
        .await
        .expect("per-pane 訂閱應成功");

    let received = fake.received();
    assert_eq!(received.len(), 2, "應該各建立一條連線");

    let schemas = common::schemas();
    let p22_request = common::validator_for_root(&schemas.p22, "request");
    let p20_request = common::validator_for_root(&schemas.p20, "request");

    for (label, line) in [
        ("lifecycle events.subscribe", &received[0][0]),
        ("per-pane events.subscribe", &received[1][0]),
    ] {
        let instance: serde_json::Value =
            serde_json::from_str(line).expect("request 行應為合法 JSON");
        common::assert_valid(
            &p22_request,
            &instance,
            &format!("{label} (protocol 22 request)"),
        );
        common::assert_valid(
            &p20_request,
            &instance,
            &format!("{label} (protocol 20 request)"),
        );
    }
}
