//! Task 4.2 驗收測試：`Client::request`（design D3、D5、D10、D12；spec `herdr-request`）。
//! 全部走真實 transport（假 HERDR，見 `herdr_client::testing::FakeHerdr`），不需要真機
//! HERDR。

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use async_trait::async_trait;

use herdr_client::client::{Client, PaneReadRequest, RequestError, SessionSnapshotRequest};
use herdr_client::connector::{ChildStdioConnector, ConnectError, Connector, NdjsonStream};
use herdr_client::testing::{FakeHerdr, FakeHerdrConfig, MethodResponse};
use herdr_client::types::{PaneReadParams, ReadSource};

mod common;
use common::load_json;

fn snapshot_result_fixture() -> serde_json::Value {
    let fixture = load_json("snapshot-p22.json");
    fixture
        .get("result")
        .cloned()
        .expect("snapshot-p22.json 應有 result 欄位")
}

fn client_for(fake: &FakeHerdr) -> Client {
    Client::new(Arc::from(fake.connector()))
}

fn test_child_bin() -> &'static str {
    env!("CARGO_BIN_EXE_herdr-client-test-child")
}

// ---------------------------------------------------------------------------
// 最小 test double：讀出 Client 實際送出的 request id，用一個閉包產生回應行。
//
// `FakeHerdr` 現有的 `MethodResponse` 只覆蓋六種固定形狀（見 task 4.1 報告），對「一般
// method 收到帶自訂後綴 id 的 error」（D12）、「result 與 error 同時出現」、「兩者皆缺席」
// 這類 Codex task review fix round 1 finding 3 要的畸形回應矩陣沒有對應的變體。依 brief
// 指示「若假 HERDR 缺你需要的能力…用最小 workaround，不要改它」，這裡寫一個通用、可重用的
// `Connector`／`NdjsonStream` 雙面體，用閉包描述「拿到 request id 之後要回哪一行」，避免
// 每個畸形回應情境都各自重寫一份幾乎一樣的雙面體。
// ---------------------------------------------------------------------------

type RespondFn = Arc<dyn Fn(&str) -> String + Send + Sync>;

struct ScriptedResponseConnector {
    respond: RespondFn,
}

#[async_trait]
impl Connector for ScriptedResponseConnector {
    async fn connect(&self) -> Result<Box<dyn NdjsonStream>, ConnectError> {
        Ok(Box::new(ScriptedResponseStream {
            respond: self.respond.clone(),
            sent_id: None,
        }))
    }

    fn describe(&self) -> String {
        "scripted-response-test-double".to_string()
    }
}

struct ScriptedResponseStream {
    respond: RespondFn,
    sent_id: Option<String>,
}

#[async_trait]
impl NdjsonStream for ScriptedResponseStream {
    async fn send_line(&mut self, line: &str) -> std::io::Result<()> {
        let value: serde_json::Value = serde_json::from_str(line)
            .expect("Client 送出的 request 行應為合法 JSON（測試雙面體的前提）");
        self.sent_id = value.get("id").and_then(|v| v.as_str()).map(str::to_string);
        Ok(())
    }

    async fn recv_line(&mut self) -> std::io::Result<Option<String>> {
        let id = self
            .sent_id
            .clone()
            .expect("recv_line 前應該已經呼叫過 send_line");
        Ok(Some((self.respond)(&id)))
    }
}

/// 建一個永遠用 `respond(request_id)` 產生回應行的 `Client`。
fn scripted_client(respond: impl Fn(&str) -> String + Send + Sync + 'static) -> Client {
    Client::new(Arc::new(ScriptedResponseConnector {
        respond: Arc::new(respond),
    }))
}

// ---------------------------------------------------------------------------
// 驗收測試（brief 指定名稱）
// ---------------------------------------------------------------------------

#[tokio::test]
async fn request_snapshot_ok_and_connection_closed_after_one_line() {
    let fixture = load_json("snapshot-p22.json");
    let config = FakeHerdrConfig::new().with_snapshot_fixture_line(&fixture);
    let fake = FakeHerdr::start(config).await.expect("啟動假 HERDR 失敗");
    let client = client_for(&fake);

    let snapshot = client
        .request(SessionSnapshotRequest {})
        .await
        .expect("session.snapshot 應成功");

    assert_eq!(snapshot.snapshot.protocol, 22);
    assert_eq!(
        snapshot.snapshot.version,
        "0.9.0-preview.2026-09-08-62431dbd033b"
    );

    // 假 HERDR（task 4.1）在回完一次回應後就會關閉連線；這裡確認的是 Client 只送了一行
    // request、對面也只記到一條連線一行——「之後連線被關閉」由 4.1 已驗證的假 HERDR 行為
    // 保證（見 `fake_closes_after_single_response`），`Client::request` 本身也沒有保留這條
    // 連線（函式結束時 drop）。
    let received = fake.received();
    assert_eq!(received.len(), 1, "應該只建立一條連線");
    assert_eq!(received[0].len(), 1, "這條連線應該只送出一行 request");
}

#[tokio::test]
async fn request_ids_are_unique() {
    let config = FakeHerdrConfig::new().with_snapshot_result(snapshot_result_fixture());
    let fake = FakeHerdr::start(config).await.expect("啟動假 HERDR 失敗");
    let client = client_for(&fake);

    client
        .request(SessionSnapshotRequest {})
        .await
        .expect("第一次 session.snapshot 應成功");
    client
        .request(SessionSnapshotRequest {})
        .await
        .expect("第二次 session.snapshot 應成功");

    let received = fake.received();
    assert_eq!(received.len(), 2, "應該建立兩條連線");

    let first: serde_json::Value =
        serde_json::from_str(&received[0][0]).expect("記錄的行應為合法 JSON");
    let second: serde_json::Value =
        serde_json::from_str(&received[1][0]).expect("記錄的行應為合法 JSON");
    let first_id = first.get("id").and_then(|v| v.as_str());
    let second_id = second.get("id").and_then(|v| v.as_str());
    assert!(first_id.is_some() && second_id.is_some());
    assert_ne!(first_id, second_id, "兩個 request 的 id 不應重複");
}

#[tokio::test]
async fn request_remote_error_keeps_code_and_message() {
    let config = FakeHerdrConfig::new().with_method_response(
        "session.snapshot",
        MethodResponse::RemoteError {
            code: "x".to_string(),
            message: "y".to_string(),
        },
    );
    let fake = FakeHerdr::start(config).await.expect("啟動假 HERDR 失敗");
    let client = client_for(&fake);

    let err = client
        .request(SessionSnapshotRequest {})
        .await
        .expect_err("遠端錯誤應該回傳 Err");

    match err {
        RequestError::Remote {
            code,
            message,
            response_id,
        } => {
            assert_eq!(code, "x");
            assert_eq!(message, "y");
            assert!(!response_id.is_empty(), "response_id 不應為空");
        }
        other => panic!("預期 RequestError::Remote，實際: {other:?}"),
    }
}

#[tokio::test]
async fn request_id_mismatch_is_protocol() {
    let config = FakeHerdrConfig::new().with_method_response(
        "session.snapshot",
        MethodResponse::WrongId(snapshot_result_fixture()),
    );
    let fake = FakeHerdr::start(config).await.expect("啟動假 HERDR 失敗");
    let client = client_for(&fake);

    let err = client
        .request(SessionSnapshotRequest {})
        .await
        .expect_err("id 不符應該回傳 Err");

    assert!(
        matches!(err, RequestError::Protocol(_)),
        "預期 RequestError::Protocol，實際: {err:?}"
    );
}

#[tokio::test]
async fn request_non_json_is_protocol() {
    let config = FakeHerdrConfig::new().with_method_response(
        "session.snapshot",
        MethodResponse::NonJson("not json at all".to_string()),
    );
    let fake = FakeHerdr::start(config).await.expect("啟動假 HERDR 失敗");
    let client = client_for(&fake);

    let err = client
        .request(SessionSnapshotRequest {})
        .await
        .expect_err("非 JSON 回應應該回傳 Err");

    assert!(
        matches!(err, RequestError::Protocol(_)),
        "預期 RequestError::Protocol，實際: {err:?}"
    );
}

#[tokio::test]
async fn request_wrong_result_type_is_protocol() {
    let config =
        FakeHerdrConfig::new().with_method_response("session.snapshot", MethodResponse::PongResult);
    let fake = FakeHerdr::start(config).await.expect("啟動假 HERDR 失敗");
    let client = client_for(&fake);

    let err = client
        .request(SessionSnapshotRequest {})
        .await
        .expect_err("result 形狀不符應該回傳 Err");

    assert!(
        matches!(err, RequestError::Protocol(_)),
        "預期 RequestError::Protocol，實際: {err:?}"
    );
}

#[tokio::test]
async fn request_closed_before_reply_is_connect_error_with_reason() {
    let config = FakeHerdrConfig::new()
        .with_method_response("session.snapshot", MethodResponse::CloseBeforeReply);
    let fake = FakeHerdr::start(config).await.expect("啟動假 HERDR 失敗");
    let client = client_for(&fake);

    let err = client
        .request(SessionSnapshotRequest {})
        .await
        .expect_err("回應前連線中斷應該回傳 Err");

    // 指揮官的補充決定：named pipe 上「收到後不回就關閉」對 client 只是乾淨 EOF
    // （`Ok(None)`），transport 不會自己報錯；spec 要求的「連線類錯誤，原因文字非空」由
    // `Client::request` 自己合成 `RequestError::Io(UnexpectedEof, "...")`——不是
    // `RequestError::Connect`（那一類專屬於「目標根本沒有 server」，design D3）。
    match err {
        RequestError::Io(io_err) => {
            assert_eq!(io_err.kind(), std::io::ErrorKind::UnexpectedEof);
            assert!(!io_err.to_string().is_empty(), "原因文字不應為空");
        }
        other => panic!("預期 RequestError::Io(UnexpectedEof)，實際: {other:?}"),
    }
}

#[tokio::test]
async fn request_server_not_running() {
    // 直接指向一個從來沒有任何 server 監聽過的端點路徑，對應 spec「連線目標沒有 server」。
    //
    // 一開始嘗試的寫法是「啟動假 HERDR 後立刻 drop，再用它的 connector 連」（沿用 4.1 報告
    // 的 `new_connection_after_drop_is_server_not_running`），但實測會間歇性地得到
    // `RequestError::Io(BrokenPipe)`：`FakeHerdr::drop` 的 `listener_task.abort()`
    // 是延後生效的（tokio 要等到該 task 下一次被排程才真的取消、真的釋放底層 pipe
    // instance），在這個「已經 drop、但 OS 端 pipe instance 還沒真的被收走」的極短窗口內，
    // `connect()` 可能短暫成功，然後 `send_line()` 才因為對面正在被拆除而回 `BrokenPipe`
    // ——這是「曾經有 server、剛被關掉」的競態，語意上更接近「連線中斷」而不是「連線目標從來
    // 沒有 server」，`RequestError::Io` 是對這個窄窗口更準確的分類，不該被塞進
    // `ServerNotRunning`。改成指向一個從一開始就不存在任何 server 的路徑，直接、確定地測
    // spec 這個 Scenario，不用重試迴圈也不會有競態。
    let path = std::env::temp_dir()
        .join("herdr-client-request-test")
        .join(format!(
            "nonexistent-{}-{}.sock",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock 應晚於 UNIX_EPOCH")
                .as_nanos()
        ));

    #[cfg(windows)]
    let connector: Arc<dyn Connector> =
        Arc::new(herdr_client::connector::NamedPipeConnector::new(path));
    #[cfg(unix)]
    let connector: Arc<dyn Connector> =
        Arc::new(herdr_client::connector::UnixSocketConnector::new(path));

    let client = Client::new(connector);
    let err = client
        .request(SessionSnapshotRequest {})
        .await
        .expect_err("目標沒有 server 應該回傳 Err");

    assert!(
        matches!(
            err,
            RequestError::Connect(ConnectError::ServerNotRunning { .. })
        ),
        "預期 RequestError::Connect(ServerNotRunning)，實際: {err:?}"
    );
}

// ---------------------------------------------------------------------------
// 指揮官加碼的兩個測試
// ---------------------------------------------------------------------------

#[tokio::test]
async fn request_line_validates_against_both_schemas() {
    let config = FakeHerdrConfig::new().with_snapshot_result(snapshot_result_fixture());
    let fake = FakeHerdr::start(config).await.expect("啟動假 HERDR 失敗");
    let client = client_for(&fake);

    client
        .request(SessionSnapshotRequest {})
        .await
        .expect("session.snapshot 應成功");

    // `pane.read` 沒有替它設定回應：假 HERDR 對沒設定的一般 method 預設等同
    // `CloseBeforeReply`（task 4.1 報告 concern 2），這裡只在乎假 HERDR 記錄到的 request
    // 行本身是否合法，不在乎回應——所以忽略這次呼叫的結果。
    let pane_read_params = PaneReadParams {
        pane_id: "wD:p1".to_string(),
        source: ReadSource::Visible,
        format: None,
        lines: None,
        strip_ansi: None,
    };
    let _ = client.request(PaneReadRequest(pane_read_params)).await;

    let received = fake.received();
    assert_eq!(received.len(), 2, "應該各建立一條連線");

    let schemas = common::schemas();
    let p22_request = common::validator_for_root(&schemas.p22, "request");
    let p20_request = common::validator_for_root(&schemas.p20, "request");

    for (label, line) in [
        ("session.snapshot", &received[0][0]),
        ("pane.read", &received[1][0]),
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

/// design D12：`error` 回應的 `id` 以 `"<request id>:"` 開頭時仍視為相符、回 `Remote`。
#[tokio::test]
async fn error_id_with_probe_suffix_is_remote() {
    let client = scripted_client(|id| {
        serde_json::json!({
            "id": format!("{id}:sub:1:probe"),
            "error": { "code": "pane_not_found", "message": "pane wD:pS not found" },
        })
        .to_string()
    });

    let err = client
        .request(SessionSnapshotRequest {})
        .await
        .expect_err("帶後綴 id 的 error 回應仍應回傳 Err（Remote，不是 Protocol）");

    // 全分支最終 review finding 5：`RequestError::Remote` 現在保留 `response_id`（含
    // D12 探測失敗的 `:sub:<n>:probe` 後綴），`Display` 把它排進錯誤訊息——序號資訊不該只
    // 活在 `response.id` 裡、解析完就被丟掉。
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
            assert!(message.contains("wD:pS"));
            assert!(
                response_id.contains(":sub:1:probe"),
                "response_id 應含 probe 後綴，實際: {response_id}"
            );
        }
        other => panic!("預期 RequestError::Remote，實際: {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Codex task review fix round 1 finding 3：畸形回應的完整分類矩陣。
// ---------------------------------------------------------------------------

/// `result` 與 `error` 同時出現：兩者「恰有一個」的前提被違反，不能只看 `error` 就直接判
/// `Remote`（原本的實作會這樣做，忽略掉同時存在的 `result`）。
#[tokio::test]
async fn request_response_with_both_result_and_error_is_protocol() {
    let client = scripted_client(|id| {
        serde_json::json!({
            "id": id,
            "result": { "type": "session_snapshot", "snapshot": {} },
            "error": { "code": "x", "message": "y" },
        })
        .to_string()
    });

    let err = client
        .request(SessionSnapshotRequest {})
        .await
        .expect_err("result 與 error 同時出現應該回傳 Err");

    assert!(
        matches!(err, RequestError::Protocol(_)),
        "預期 RequestError::Protocol，實際: {err:?}"
    );
}

/// `result` 與 `error` 皆缺席。
#[tokio::test]
async fn request_response_with_neither_result_nor_error_is_protocol() {
    let client = scripted_client(|id| serde_json::json!({ "id": id }).to_string());

    let err = client
        .request(SessionSnapshotRequest {})
        .await
        .expect_err("result 與 error 皆缺席應該回傳 Err");

    assert!(
        matches!(err, RequestError::Protocol(_)),
        "預期 RequestError::Protocol，實際: {err:?}"
    );
}

/// `error` 物件缺 `code`（schema 必填，design D13）：`ErrorBody` 反序列化直接失敗，不應該
/// 被容器層 `#[serde(default)]` 悄悄補成空字串再誤判成一個「code 是空字串」的 `Remote`。
#[tokio::test]
async fn request_response_error_missing_code_is_protocol() {
    let client = scripted_client(|id| {
        serde_json::json!({
            "id": id,
            "error": { "message": "y" },
        })
        .to_string()
    });

    let err = client
        .request(SessionSnapshotRequest {})
        .await
        .expect_err("error 缺 code 應該回傳 Err");

    assert!(
        matches!(err, RequestError::Protocol(_)),
        "預期 RequestError::Protocol，實際: {err:?}"
    );
}

/// `error` 的 `id` 既不等於 request id、也不是 `"<request id>:"` 開頭——與
/// `error_id_with_probe_suffix_is_remote`（相符）互補，驗證「不相符」這一側同樣被歸類正確。
#[tokio::test]
async fn request_response_error_with_unrelated_id_is_protocol() {
    let client = scripted_client(|_id| {
        serde_json::json!({
            "id": "completely-different-id",
            "error": { "code": "x", "message": "y" },
        })
        .to_string()
    });

    let err = client
        .request(SessionSnapshotRequest {})
        .await
        .expect_err("error 的 id 不相符應該回傳 Err");

    assert!(
        matches!(err, RequestError::Protocol(_)),
        "預期 RequestError::Protocol，實際: {err:?}"
    );
}

// ---------------------------------------------------------------------------
// Codex task review fix round 1 finding 4：成功案例要證明 Client 真的釋放了連線，不能只靠
// 假 HERDR 自己主動關閉來間接推論。
// ---------------------------------------------------------------------------

struct DropFlagConnector {
    dropped: Arc<AtomicBool>,
}

#[async_trait]
impl Connector for DropFlagConnector {
    async fn connect(&self) -> Result<Box<dyn NdjsonStream>, ConnectError> {
        Ok(Box::new(DropFlagStream {
            sent_id: None,
            dropped: self.dropped.clone(),
        }))
    }

    fn describe(&self) -> String {
        "drop-flag-test-double".to_string()
    }
}

struct DropFlagStream {
    sent_id: Option<String>,
    dropped: Arc<AtomicBool>,
}

impl Drop for DropFlagStream {
    fn drop(&mut self) {
        self.dropped.store(true, Ordering::SeqCst);
    }
}

#[async_trait]
impl NdjsonStream for DropFlagStream {
    async fn send_line(&mut self, line: &str) -> std::io::Result<()> {
        let value: serde_json::Value = serde_json::from_str(line)
            .expect("Client 送出的 request 行應為合法 JSON（測試雙面體的前提）");
        self.sent_id = value.get("id").and_then(|v| v.as_str()).map(str::to_string);
        Ok(())
    }

    async fn recv_line(&mut self) -> std::io::Result<Option<String>> {
        let id = self
            .sent_id
            .clone()
            .expect("recv_line 前應該已經呼叫過 send_line");
        let line = serde_json::json!({ "id": id, "result": snapshot_result_fixture() }).to_string();
        Ok(Some(line))
    }
}

#[tokio::test]
async fn request_releases_connection_after_response() {
    let dropped = Arc::new(AtomicBool::new(false));
    let client = Client::new(Arc::new(DropFlagConnector {
        dropped: dropped.clone(),
    }));

    client
        .request(SessionSnapshotRequest {})
        .await
        .expect("session.snapshot 應成功");

    assert!(
        dropped.load(Ordering::SeqCst),
        "Client::request 回傳後應該已經釋放（drop）自己開的連線"
    );
}

// ---------------------------------------------------------------------------
// Codex task review fix round 1 finding 2：初次送出（不只是初次讀取）遇到
// NotFound／ConnectionRefused 也要分類成 Connect(ServerNotRunning)。
// ---------------------------------------------------------------------------

/// 包一層 `ChildStdioConnector`：`connect()` 時先用 `spawn_stream()`＋`wait_for_exit()`
/// （`test-support` feature 下的測試專用方法，見 `tests/transport.rs` 的
/// `child_exits_immediately_is_server_not_running`）確定子程序「已經」結束，才把
/// `ChildStdioStream` 交給 `Client`。這樣 `Client::request` 接手後的第一個 `send_line`
/// 保證會撞上已關閉的 stdin（`ChildStdioStream::send_line` 已經會把這個情境分類成
/// `ConnectionRefused`，見 `src/connector/child_stdio.rs`），把「子程序立即結束」這個
/// design D3 情境變成決定性、不用賭時序的測試，而不是像 `ChildStdioConnector::connect()`
/// 那樣「只 spawn 不等待」而可能因為排程時機不同，讓第一次 `send_line` 剛好還沒撞上。
struct ExitedChildBridgeConnector {
    inner: ChildStdioConnector,
}

#[async_trait]
impl Connector for ExitedChildBridgeConnector {
    async fn connect(&self) -> Result<Box<dyn NdjsonStream>, ConnectError> {
        let mut stream = self.inner.spawn_stream().await?;
        stream
            .wait_for_exit(Duration::from_secs(3))
            .await
            .expect("子程序應該在 3 秒內結束（exit-immediately）");
        Ok(Box::new(stream) as Box<dyn NdjsonStream>)
    }

    fn describe(&self) -> String {
        self.inner.describe()
    }
}

#[tokio::test]
async fn request_via_child_bridge_that_exits_is_server_not_running() {
    let connector = ExitedChildBridgeConnector {
        inner: ChildStdioConnector::new(
            test_child_bin(),
            ["exit-immediately", "1", "boom: target not found"],
        ),
    };
    let client = Client::new(Arc::new(connector));

    let err = client
        .request(SessionSnapshotRequest {})
        .await
        .expect_err("橋接目標子程序已經結束應該回傳 Err");

    match err {
        RequestError::Connect(ConnectError::ServerNotRunning { detail }) => {
            assert!(
                detail.contains("boom: target not found"),
                "detail 應含子程序 stderr 內容，實際: {detail}"
            );
        }
        other => panic!("預期 RequestError::Connect(ServerNotRunning)，實際: {other:?}"),
    }
}
