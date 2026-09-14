//! Task 4.1 驗收測試：假 HERDR（`herdr_client::testing::FakeHerdr`）。全部走真實 transport
//! （Windows named pipe／unix socket），不需要真機 HERDR（design D7）。

use herdr_client::connector::{ConnectError, NdjsonStream};
use herdr_client::testing::{FakeHerdr, FakeHerdrConfig, MethodResponse, Step, SubscribeMatcher};
use herdr_client::types::{
    EventsSubscribeParams, RequestEnvelope, ResponseEnvelope, SessionSnapshotResult, Subscription,
};

mod common;
use common::load_json;

async fn connect(fake: &FakeHerdr) -> Box<dyn NdjsonStream> {
    fake.connector().connect().await.expect("連線假 HERDR 失敗")
}

async fn send_request(
    stream: &mut Box<dyn NdjsonStream>,
    id: &str,
    method: &str,
    params: &serde_json::Value,
) {
    let request = RequestEnvelope { id, method, params };
    let line = serde_json::to_string(&request).expect("request 應可序列化");
    stream.send_line(&line).await.expect("送出 request 失敗");
}

async fn send_snapshot_request(stream: &mut Box<dyn NdjsonStream>, id: &str) {
    send_request(stream, id, "session.snapshot", &serde_json::json!({})).await;
}

async fn send_subscribe_request(
    stream: &mut Box<dyn NdjsonStream>,
    id: &str,
    subscriptions: Vec<Subscription>,
) {
    let params = EventsSubscribeParams { subscriptions };
    let params_value = serde_json::to_value(&params).expect("subscribe params 應可序列化");
    send_request(stream, id, "events.subscribe", &params_value).await;
}

async fn recv_required_line(stream: &mut Box<dyn NdjsonStream>) -> String {
    stream
        .recv_line()
        .await
        .expect("讀取失敗")
        .expect("不應為 EOF")
}

fn snapshot_result_fixture() -> serde_json::Value {
    let fixture = load_json("snapshot-p22.json");
    fixture
        .get("result")
        .cloned()
        .expect("snapshot-p22.json 應有 result 欄位")
}

// ---------------------------------------------------------------------------
// 驗收測試（brief 指定測試名）
// ---------------------------------------------------------------------------

#[tokio::test]
async fn fake_serves_snapshot_over_native_transport() {
    let fixture = load_json("snapshot-p22.json");
    let config = FakeHerdrConfig::new().with_snapshot_fixture_line(&fixture);
    let fake = FakeHerdr::start(config).await.expect("啟動假 HERDR 失敗");

    // fix round 1 finding 3：端點放在系統暫存目錄下，Windows 因此是含磁碟機代號與反斜線的
    // 絕對路徑，對照 design D7「名稱用暫存路徑加隨機後綴」與真機 HERDR pipe 名稱的形狀。
    #[cfg(windows)]
    assert!(
        fake.endpoint_path().to_string_lossy().contains(':'),
        "Windows 端點路徑應含磁碟機代號，實際: {}",
        fake.endpoint_path().display()
    );

    let mut stream = connect(&fake).await;
    send_snapshot_request(&mut stream, "1").await;
    let line = recv_required_line(&mut stream).await;

    let response: ResponseEnvelope = serde_json::from_str(&line).expect("回應應為合法 JSON");
    assert_eq!(response.id, "1");
    let result = response.result.expect("應有 result");
    let snapshot: SessionSnapshotResult =
        serde_json::from_value(result).expect("應可解析為 SessionSnapshotResult");
    assert_eq!(snapshot.snapshot.protocol, 22);
    assert_eq!(
        snapshot.snapshot.version,
        "0.9.0-preview.2026-09-08-62431dbd033b"
    );
}

#[tokio::test]
async fn fake_records_received_lines() {
    let config = FakeHerdrConfig::new().with_snapshot_result(snapshot_result_fixture());
    let fake = FakeHerdr::start(config).await.expect("啟動假 HERDR 失敗");

    let mut first = connect(&fake).await;
    send_snapshot_request(&mut first, "1").await;
    let _ = recv_required_line(&mut first).await;

    let mut second = connect(&fake).await;
    send_snapshot_request(&mut second, "2").await;
    let _ = recv_required_line(&mut second).await;

    let received = fake.received();
    assert_eq!(received.len(), 2, "應該記錄兩條連線");
    assert_eq!(received[0].len(), 1, "第一條連線只收到一行");
    assert_eq!(received[1].len(), 1, "第二條連線只收到一行");

    let first_request: serde_json::Value =
        serde_json::from_str(&received[0][0]).expect("記錄的行應為合法 JSON");
    let second_request: serde_json::Value =
        serde_json::from_str(&received[1][0]).expect("記錄的行應為合法 JSON");
    assert_eq!(first_request.get("id").and_then(|v| v.as_str()), Some("1"));
    assert_eq!(second_request.get("id").and_then(|v| v.as_str()), Some("2"));
    assert_ne!(
        first_request.get("id"),
        second_request.get("id"),
        "兩個 request 的 id 不應重複"
    );
}

#[tokio::test]
async fn fake_closes_after_single_response() {
    let config = FakeHerdrConfig::new().with_snapshot_result(snapshot_result_fixture());
    let fake = FakeHerdr::start(config).await.expect("啟動假 HERDR 失敗");

    let mut stream = connect(&fake).await;
    send_snapshot_request(&mut stream, "1").await;
    let _ = recv_required_line(&mut stream).await;

    let after = stream.recv_line().await;
    assert!(
        matches!(after, Ok(None)),
        "回應後連線應該正常關閉（乾淨 EOF），實際: {after:?}"
    );
}

// ---------------------------------------------------------------------------
// 一般 request 的回應覆寫：WrongId、NonJson、PongResult、CloseBeforeReply、RemoteError
// ---------------------------------------------------------------------------

#[tokio::test]
async fn generic_request_wrong_id_is_configurable() {
    let config = FakeHerdrConfig::new().with_method_response(
        "session.snapshot",
        MethodResponse::WrongId(snapshot_result_fixture()),
    );
    let fake = FakeHerdr::start(config).await.expect("啟動假 HERDR 失敗");

    let mut stream = connect(&fake).await;
    send_snapshot_request(&mut stream, "42").await;
    let line = recv_required_line(&mut stream).await;
    let response: ResponseEnvelope = serde_json::from_str(&line).expect("回應應為合法 JSON");

    assert_ne!(response.id, "42", "id 應刻意與 request 不同");
    assert!(response.result.is_some(), "result 形狀本身應該合法");
}

#[tokio::test]
async fn generic_request_non_json_is_configurable() {
    let config = FakeHerdrConfig::new().with_method_response(
        "session.snapshot",
        MethodResponse::NonJson("not json at all".to_string()),
    );
    let fake = FakeHerdr::start(config).await.expect("啟動假 HERDR 失敗");

    let mut stream = connect(&fake).await;
    send_snapshot_request(&mut stream, "1").await;
    let line = recv_required_line(&mut stream).await;

    assert_eq!(line, "not json at all");
    let parsed: Result<serde_json::Value, _> = serde_json::from_str(&line);
    assert!(parsed.is_err(), "這行不應該是合法 JSON");
}

#[tokio::test]
async fn generic_request_pong_result_is_configurable() {
    let config =
        FakeHerdrConfig::new().with_method_response("session.snapshot", MethodResponse::PongResult);
    let fake = FakeHerdr::start(config).await.expect("啟動假 HERDR 失敗");

    let mut stream = connect(&fake).await;
    send_snapshot_request(&mut stream, "1").await;
    let line = recv_required_line(&mut stream).await;
    let response: ResponseEnvelope = serde_json::from_str(&line).expect("回應應為合法 JSON");

    assert_eq!(response.id, "1");
    let result = response.result.expect("應有 result");
    assert_eq!(result.get("type").and_then(|v| v.as_str()), Some("pong"));
    let parsed: Result<SessionSnapshotResult, _> = serde_json::from_value(result);
    assert!(
        parsed.is_err(),
        "type 為 pong 不應該能解析成 SessionSnapshotResult"
    );
}

#[tokio::test]
async fn generic_request_remote_error_is_configurable() {
    let config = FakeHerdrConfig::new().with_method_response(
        "session.snapshot",
        MethodResponse::RemoteError {
            code: "x".to_string(),
            message: "y".to_string(),
        },
    );
    let fake = FakeHerdr::start(config).await.expect("啟動假 HERDR 失敗");

    let mut stream = connect(&fake).await;
    send_snapshot_request(&mut stream, "1").await;
    let line = recv_required_line(&mut stream).await;
    let response: ResponseEnvelope = serde_json::from_str(&line).expect("回應應為合法 JSON");

    assert_eq!(response.id, "1");
    let error = response.error.expect("應有 error");
    assert_eq!(error.code, "x");
    assert_eq!(error.message, "y");
}

#[tokio::test]
async fn generic_request_close_before_reply_is_configurable() {
    let config = FakeHerdrConfig::new()
        .with_method_response("session.snapshot", MethodResponse::CloseBeforeReply);
    let fake = FakeHerdr::start(config).await.expect("啟動假 HERDR 失敗");

    let mut stream = connect(&fake).await;
    send_snapshot_request(&mut stream, "1").await;

    // 假 HERDR 收到 request 後直接關閉、不回應：實測（見 task 4.1 報告）named pipe 在這種
    // 「從沒寫過任何東西就關閉」的情況下，tokio 把底層的管道錯誤正規化成乾淨 EOF
    // （`Ok(None)`），不是 I/O 錯誤；「連線類錯誤」是 task 4.2 的 `Client` 依「還沒讀到
    // 完整回應就 EOF」自己合成，不是 transport 層的原始行為。
    let outcome = stream.recv_line().await;
    assert!(
        matches!(outcome, Ok(None)),
        "應為乾淨 EOF（沒有任何回應行），實際: {outcome:?}"
    );
}

// ---------------------------------------------------------------------------
// events.subscribe：探測失敗、腳本各步驟
// ---------------------------------------------------------------------------

#[tokio::test]
async fn subscribe_probe_failure_reports_pane_not_found_with_id_suffix() {
    let config = FakeHerdrConfig::new().with_failing_probe_pane_ids(["wD:pS"]);
    let fake = FakeHerdr::start(config).await.expect("啟動假 HERDR 失敗");

    let mut stream = connect(&fake).await;
    send_subscribe_request(
        &mut stream,
        "7",
        vec![Subscription::PaneAgentStatusChanged {
            pane_id: "wD:pS".to_string(),
        }],
    )
    .await;
    let line = recv_required_line(&mut stream).await;
    let response: ResponseEnvelope = serde_json::from_str(&line).expect("回應應為合法 JSON");

    assert_eq!(response.id, "7:sub:1:probe");
    let error = response.error.expect("探測失敗應該回 error");
    assert_eq!(error.code, "pane_not_found");
    assert!(
        error.message.contains("wD:pS"),
        "訊息應含 pane id: {error:?}"
    );

    let after = stream.recv_line().await;
    assert!(
        matches!(after, Ok(None)),
        "探測失敗後不應該建立串流，連線應該關閉，實際: {after:?}"
    );
}

#[tokio::test]
async fn subscribe_script_delivers_event() {
    let event_line = r#"{"event":"pane_created","data":{"pane_id":"wD:p1"}}"#;
    let config = FakeHerdrConfig::new()
        .with_subscribe_script(vec![Step::Event(event_line.to_string()), Step::Hold]);
    let fake = FakeHerdr::start(config).await.expect("啟動假 HERDR 失敗");

    let mut stream = connect(&fake).await;
    send_subscribe_request(&mut stream, "1", vec![Subscription::PaneCreated]).await;

    let started_line = recv_required_line(&mut stream).await;
    let started: ResponseEnvelope = serde_json::from_str(&started_line).expect("合法 JSON");
    assert_eq!(started.id, "1");
    let result = started.result.expect("應有 subscription_started result");
    assert_eq!(
        result.get("type").and_then(|v| v.as_str()),
        Some("subscription_started")
    );

    let event = recv_required_line(&mut stream).await;
    assert_eq!(event, event_line);
}

#[tokio::test]
async fn subscribe_script_malformed_line_is_delivered_raw() {
    let malformed = "this is not json";
    let config = FakeHerdrConfig::new()
        .with_subscribe_script(vec![Step::Malformed(malformed.to_string()), Step::Close]);
    let fake = FakeHerdr::start(config).await.expect("啟動假 HERDR 失敗");

    let mut stream = connect(&fake).await;
    send_subscribe_request(&mut stream, "1", vec![Subscription::PaneCreated]).await;
    let _started = recv_required_line(&mut stream).await;

    let line = recv_required_line(&mut stream).await;
    assert_eq!(line, malformed);
}

#[tokio::test]
async fn subscribe_script_delay_then_event() {
    let event_line = r#"{"event":"pane_created","data":{"pane_id":"wD:p1"}}"#;
    let config = FakeHerdrConfig::new().with_subscribe_script(vec![
        Step::Delay(std::time::Duration::from_millis(50)),
        Step::Event(event_line.to_string()),
        Step::Close,
    ]);
    let fake = FakeHerdr::start(config).await.expect("啟動假 HERDR 失敗");

    let mut stream = connect(&fake).await;
    send_subscribe_request(&mut stream, "1", vec![Subscription::PaneCreated]).await;
    let _started = recv_required_line(&mut stream).await;

    let start = std::time::Instant::now();
    let event = recv_required_line(&mut stream).await;
    assert_eq!(event, event_line);
    assert!(
        start.elapsed() >= std::time::Duration::from_millis(40),
        "Delay 應該讓事件延後送達，實際: {:?}",
        start.elapsed()
    );
}

#[tokio::test]
async fn subscribe_script_close_ends_stream() {
    let config = FakeHerdrConfig::new().with_subscribe_script(vec![Step::Close]);
    let fake = FakeHerdr::start(config).await.expect("啟動假 HERDR 失敗");

    let mut stream = connect(&fake).await;
    send_subscribe_request(&mut stream, "1", vec![Subscription::PaneCreated]).await;
    let _started = recv_required_line(&mut stream).await;

    let after = stream.recv_line().await;
    assert!(
        matches!(after, Ok(None)),
        "Close 之後應該是乾淨 EOF，實際: {after:?}"
    );
}

#[tokio::test]
async fn subscribe_script_hold_keeps_connection_open_until_fake_dropped() {
    let config = FakeHerdrConfig::new().with_subscribe_script(vec![Step::Hold]);
    let fake = FakeHerdr::start(config).await.expect("啟動假 HERDR 失敗");

    let mut stream = connect(&fake).await;
    send_subscribe_request(&mut stream, "1", vec![Subscription::PaneCreated]).await;
    let _started = recv_required_line(&mut stream).await;

    let still_open =
        tokio::time::timeout(std::time::Duration::from_millis(200), stream.recv_line()).await;
    assert!(
        still_open.is_err(),
        "Hold 期間連線應該保持開著，不應該收到任何行或 EOF，實際: {still_open:?}"
    );

    drop(fake);

    // fix round 1 finding 2：`FakeHerdr::drop` 對所有 handler task 呼叫 `abort_all()`，
    // 不再靠協作式訊號，1 秒內就應該結束（原本用 3 秒是保守值，現在收緊成 finding 2 要求的
    // 「1 秒內」）。
    let after = tokio::time::timeout(std::time::Duration::from_secs(1), stream.recv_line())
        .await
        .expect("fake 被 drop 後，Hold 中的連線應該在 1 秒內結束");
    assert!(
        matches!(after, Ok(None)) || after.is_err(),
        "fake 被 drop 後連線應該結束（EOF 或 I/O 錯誤皆可），實際: {after:?}"
    );
}

#[cfg(windows)]
#[tokio::test]
async fn subscribe_script_abort_yields_io_error() {
    let config = FakeHerdrConfig::new().with_subscribe_script(vec![Step::Abort]);
    let fake = FakeHerdr::start(config).await.expect("啟動假 HERDR 失敗");

    let mut stream = connect(&fake).await;
    send_subscribe_request(&mut stream, "1", vec![Subscription::PaneCreated]).await;
    let _started = recv_required_line(&mut stream).await;

    let after = tokio::time::timeout(std::time::Duration::from_secs(3), stream.recv_line())
        .await
        .expect("Abort 後連線應該在時限內結束（不應該卡住）");
    // 實測（見 task 4.1 報告）：`NamedPipeServer::disconnect()` 讓 client 端的下一次讀取拿到
    // I/O 錯誤（Windows `ERROR_NO_DATA`／233），不是乾淨 EOF——與 `Step::Close`
    // （`subscribe_script_close_ends_stream`，`Ok(None)`）明確不同，滿足 4.3「I/O 錯誤」
    // Scenario 需要「連線因 I/O 錯誤中斷」可被觀察到的前提。
    assert!(
        after.is_err(),
        "Abort 應該讓 client 端看到 I/O 錯誤而非乾淨 EOF，實際: {after:?}"
    );
}

/// fix round 1 finding 1：AF_UNIX 沒有 RST 語意、tokio 的 `UnixStream` 也沒公開
/// `SO_LINGER`（查證見 `connection.rs` 的 `BestEffortAbort` 文件註解），`Step::Abort` 在
/// unix 上降級為與 `Step::Close` 相同的乾淨 EOF——這是平台限制，這個測試把它當成明確的
/// 規格記錄下來，而不是放著一個在 unix 上會失敗的斷言。
#[cfg(unix)]
#[tokio::test]
async fn subscribe_script_abort_degrades_to_eof() {
    let config = FakeHerdrConfig::new().with_subscribe_script(vec![Step::Abort]);
    let fake = FakeHerdr::start(config).await.expect("啟動假 HERDR 失敗");

    let mut stream = connect(&fake).await;
    send_subscribe_request(&mut stream, "1", vec![Subscription::PaneCreated]).await;
    let _started = recv_required_line(&mut stream).await;

    let after = tokio::time::timeout(std::time::Duration::from_secs(3), stream.recv_line())
        .await
        .expect("Abort 後連線應該在時限內結束（不應該卡住）");
    assert!(
        matches!(after, Ok(None)),
        "unix 上 Abort 應該降級為乾淨 EOF（平台限制），實際: {after:?}"
    );
}

#[tokio::test]
async fn two_subscribe_connections_are_independent() {
    // fix round 1 finding 4：兩條連線依訂閱內容分派到不同規則，各自收到不同的事件內容，
    // 而不只是「兩個 socket 沒互相關閉」。
    let lifecycle_event = r#"{"event":"pane_created","data":{"pane_id":"wD:p1"}}"#;
    let per_pane_event = r#"{"event":"pane.agent_status_changed","data":{"pane_id":"wD:p1","agent_status":"working"}}"#;
    let config = FakeHerdrConfig::new()
        .with_subscribe_rule(
            SubscribeMatcher::LifecycleOnly,
            vec![Step::Event(lifecycle_event.to_string()), Step::Hold],
        )
        .with_subscribe_rule(
            SubscribeMatcher::PerPane,
            vec![Step::Event(per_pane_event.to_string()), Step::Hold],
        );
    let fake = FakeHerdr::start(config).await.expect("啟動假 HERDR 失敗");

    let mut lifecycle = connect(&fake).await;
    send_subscribe_request(&mut lifecycle, "1", vec![Subscription::PaneCreated]).await;
    let _ = recv_required_line(&mut lifecycle).await; // subscription_started
    let lifecycle_received = recv_required_line(&mut lifecycle).await;

    let mut per_pane = connect(&fake).await;
    send_subscribe_request(
        &mut per_pane,
        "2",
        vec![Subscription::PaneAgentStatusChanged {
            pane_id: "wD:p1".to_string(),
        }],
    )
    .await;
    let _ = recv_required_line(&mut per_pane).await; // subscription_started
    let per_pane_received = recv_required_line(&mut per_pane).await;

    assert_eq!(
        lifecycle_received, lifecycle_event,
        "lifecycle 連線應收到 lifecycle 腳本"
    );
    assert_eq!(
        per_pane_received, per_pane_event,
        "per-pane 連線應收到 per-pane 腳本"
    );
    assert_ne!(
        lifecycle_received, per_pane_received,
        "兩條連線應該依訂閱內容收到不同事件，不是同一份腳本"
    );

    drop(lifecycle);

    // 關閉其中一條之後，另一條仍應該正常（這裡用 Hold，所以只驗證它還沒被關閉：
    // 短暫等待不會出現 EOF 或錯誤）。
    let still_open =
        tokio::time::timeout(std::time::Duration::from_millis(150), per_pane.recv_line()).await;
    assert!(
        still_open.is_err(),
        "關閉另一條連線不應該影響這條，實際: {still_open:?}"
    );
}

// ---------------------------------------------------------------------------
// fix round 1 finding 2：drop 安全性——不只 Hold，任何卡住的 handler 都要在 drop 後結束。
// ---------------------------------------------------------------------------

#[tokio::test]
async fn handler_blocked_on_initial_read_is_terminated_when_fake_dropped() {
    // 連上但從不送任何一行：handler 卡在讀第一行的 await 上，不是 Hold 分支（那時腳本根本
    // 還沒開始跑）。
    let fake = FakeHerdr::start(FakeHerdrConfig::new())
        .await
        .expect("啟動假 HERDR 失敗");
    let mut stream = connect(&fake).await;

    drop(fake);

    let after = tokio::time::timeout(std::time::Duration::from_secs(1), stream.recv_line())
        .await
        .expect("卡在讀第一行的連線也應該在 fake 被 drop 後 1 秒內結束");
    assert!(
        matches!(after, Ok(None)) || after.is_err(),
        "應該結束（EOF 或 I/O 錯誤皆可），實際: {after:?}"
    );
}

#[tokio::test]
async fn handler_blocked_on_delay_is_terminated_when_fake_dropped() {
    // 腳本卡在一個很長的 Delay：handler 卡在 `tokio::time::sleep` 的 await 上。
    let config = FakeHerdrConfig::new().with_subscribe_script(vec![
        Step::Delay(std::time::Duration::from_secs(30)),
        Step::Event(r#"{"event":"pane_created","data":{}}"#.to_string()),
    ]);
    let fake = FakeHerdr::start(config).await.expect("啟動假 HERDR 失敗");

    let mut stream = connect(&fake).await;
    send_subscribe_request(&mut stream, "1", vec![Subscription::PaneCreated]).await;
    let _started = recv_required_line(&mut stream).await;

    drop(fake);

    let after = tokio::time::timeout(std::time::Duration::from_secs(1), stream.recv_line())
        .await
        .expect("卡在 Delay 的連線也應該在 fake 被 drop 後 1 秒內結束，不應該等滿 30 秒");
    assert!(
        matches!(after, Ok(None)) || after.is_err(),
        "應該結束（EOF 或 I/O 錯誤皆可），實際: {after:?}"
    );
}

#[tokio::test]
async fn new_connection_after_drop_is_server_not_running() {
    let fake = FakeHerdr::start(FakeHerdrConfig::new())
        .await
        .expect("啟動假 HERDR 失敗");
    let connector = fake.connector();
    drop(fake);

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(1);
    loop {
        match connector.connect().await {
            Err(ConnectError::ServerNotRunning { .. }) => break,
            Err(other) => panic!("預期 ServerNotRunning，實際: {other:?}"),
            Ok(_) => {
                assert!(
                    std::time::Instant::now() < deadline,
                    "超過 1 秒仍然連線成功：listener 似乎沒有真的停止接受新連線"
                );
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        }
    }
}

#[cfg(unix)]
#[tokio::test]
async fn unix_socket_file_removed_after_drop() {
    let fake = FakeHerdr::start(FakeHerdrConfig::new())
        .await
        .expect("啟動假 HERDR 失敗");
    let path = fake.endpoint_path().to_path_buf();
    assert!(path.exists(), "啟動後 socket 檔案應該存在: {path:?}");

    drop(fake);

    assert!(!path.exists(), "drop 後 socket 檔案應該被刪除: {path:?}");
}

// ---------------------------------------------------------------------------
// fix round 2 finding 2：多執行緒下 accept 與 drop 交錯——剛被接受、還沒註冊進 handler
// 集合的連線，也必須在 drop 之後有明確結果（不能永遠卡住）。
// ---------------------------------------------------------------------------

/// 腳本只有 `Step::Hold`：如果「剛接受、還沒註冊」的 handler 沒被 `abort_all()` 涵蓋到，
/// 它會照常回 `subscription_started`（第一次讀取成功），接著跑進 `Step::Hold` 的
/// `pending().await` 然後永遠掛著——**第二次讀取**（拿不到事件、也等不到連線結束）會卡住，
/// 逾時就是這條測試要抓的訊號。第一次讀取失敗（送出失敗、對面提早丟棄連線等）視為合理的
/// 「連線在關閉窗口中被放棄」，不強求一定要收到 `subscription_started`。反覆 50 次讓
/// connect／accept／spawn 與 drop 儘量交錯（multi-thread runtime，`connect` 丟到另一個
/// task 上跑，不等它就馬上 drop）。
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn drop_races_with_newly_accepted_handlers_are_still_terminated() {
    for i in 0..50 {
        let config = FakeHerdrConfig::new().with_subscribe_script(vec![Step::Hold]);
        let fake = FakeHerdr::start(config)
            .await
            .unwrap_or_else(|e| panic!("第 {i} 次啟動假 HERDR 失敗: {e}"));
        let connector = fake.connector();

        // 直接在主流程 `.await` connect（不丟到背景 task 裡跟 drop 賽跑）：client 端
        // connect 成功，代表 server 端的 accept 一定已經完成——這時 listener task 正要
        // 或已經在把這條連線登記進 handler 集合。緊接著同步呼叫 `drop`，儘量讓它落在
        // 「已接受、尚未登記完成」這個窗口內，而不是靠背景 task 的排程時機碰運氣。
        let Ok(mut stream) = connector.connect().await else {
            drop(fake);
            continue; // 連不上也是合理結果（例如上一輪殘留的收尾時間），跳過這次。
        };

        drop(fake);

        let params = EventsSubscribeParams {
            subscriptions: vec![Subscription::PaneCreated],
        };
        let request = RequestEnvelope {
            id: "1",
            method: "events.subscribe",
            params: &params,
        };
        let line = serde_json::to_string(&request).expect("request 應可序列化");
        if stream.send_line(&line).await.is_err() {
            continue; // 合理結果：連線在窗口內被放棄，寫入失敗。
        }

        // 第一次讀取：可能是 subscription_started（handler 真的被跑到並回應了），也可能
        // 因為連線在這之間被拆除而是 EOF／錯誤——兩者都合理，只要求「有結果、不卡住」。
        let first =
            tokio::time::timeout(std::time::Duration::from_secs(1), stream.recv_line()).await;
        assert!(
            first.is_ok(),
            "第 {i} 次：第一次讀取應該在 1 秒內有結果，實際逾時"
        );

        // 只有真的收到一行（代表 handler 跑起來、回了 subscription_started）才需要繼續逼問
        // 下一次讀取；這一步如果是「漏網、沒被 abort」的 handler，會卡在 Step::Hold 永遠不
        // 回應，逾時就是抓到問題。
        if matches!(first, Ok(Ok(Some(_)))) {
            let second =
                tokio::time::timeout(std::time::Duration::from_secs(1), stream.recv_line()).await;
            assert!(
                second.is_ok(),
                "第 {i} 次：收到 subscription_started 後，後續讀取也應該在 1 秒內有結果——\
                 卡住代表有漏網、沒被 abort_all() 收到的 handler 卡在 Step::Hold"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Codex scoped re-review round 1：SubscribeMatcher「第一個符合」的 precedence 測試。
// ---------------------------------------------------------------------------

#[tokio::test]
async fn subscribe_rule_precedence_uses_first_matching_rule() {
    // 兩條規則都會命中同一份「per-pane」訂閱清單：PerPane 先加入、Any 後加入且必定命中；
    // 斷言生效的是先加入的 PerPane，不是後面也符合的 Any。
    let first_event = r#"{"event":"pane.agent_status_changed","data":{"pane_id":"wD:p1","agent_status":"working"}}"#;
    let second_event = r#"{"event":"pane_created","data":{}}"#;
    let config = FakeHerdrConfig::new()
        .with_subscribe_rule(
            SubscribeMatcher::PerPane,
            vec![Step::Event(first_event.to_string()), Step::Hold],
        )
        .with_subscribe_rule(
            SubscribeMatcher::Any,
            vec![Step::Event(second_event.to_string()), Step::Hold],
        );
    let fake = FakeHerdr::start(config).await.expect("啟動假 HERDR 失敗");

    let mut stream = connect(&fake).await;
    send_subscribe_request(
        &mut stream,
        "1",
        vec![Subscription::PaneAgentStatusChanged {
            pane_id: "wD:p1".to_string(),
        }],
    )
    .await;
    let _ = recv_required_line(&mut stream).await; // subscription_started
    let received = recv_required_line(&mut stream).await;

    assert_eq!(
        received, first_event,
        "兩條規則都符合時，應該套用先加入的 PerPane，不是後面也符合的 Any"
    );
}
