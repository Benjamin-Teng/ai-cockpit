//! Task 3.1／3.2 驗收測試：`HerdrRuntime::read_output` 送出的參數與回應是否原樣交回，以及
//! `pane_not_found`／其他錯誤碼／連不上／回應無法解析的錯誤對應
//! （spec `herdr-runtime-session`「讀取 pane 輸出」的情境「送出的參數」「回應原樣交回」
//! 「pane 不存在」「其他錯誤碼」「連不上」；design D2、D4）。
//!
//! task 3.1 只驗成功路徑；本檔其餘測試（task 3.2）驗錯誤對應：全部用真實 transport（假
//! HERDR，或指向一個從一開始就不存在任何 server 的路徑），一律 `#[tokio::test]`、走真實
//! 時間，不需要真機 HERDR。

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use cockpit_core::{AgentRuntime, OutputFormat, PaneId, RuntimeError, RuntimeId};
use cockpit_herdr::probe::DistroProber;
use cockpit_herdr::runtime::{HerdrRuntime, WslProbe};
use herdr_client::connector::Connector;
use herdr_client::testing::{FakeHerdr, FakeHerdrConfig, MethodResponse};
use serde_json::{Value, json};

/// 指向假 HERDR 的 `HerdrRuntime`（`wsl` 為 `None`，不探測的 `win` 型 runtime）。
fn runtime(fake: &FakeHerdr) -> HerdrRuntime {
    HerdrRuntime::new(RuntimeId::new("win"), Arc::from(fake.connector()), None)
}

/// 組一筆 `pane.read` 成功回應的 `result`（含外層 `"type"` 標籤，見
/// `herdr-client/tests/fixtures/pane-read-p20.json` 的 `result` 形狀）。
fn pane_read_result(pane_id: &str, text: &str, truncated: bool) -> Value {
    json!({
        "type": "pane_read",
        "read": {
            "pane_id": pane_id,
            "workspace_id": "w1",
            "tab_id": "w1:t1",
            "source": "recent",
            "format": "text",
            "text": text,
            "revision": 0,
            "truncated": truncated,
        },
    })
}

/// 解析假 HERDR 收到的唯一一條連線的唯一一行 request。
fn sole_request(fake: &FakeHerdr) -> Value {
    let received = fake.received();
    assert_eq!(received.len(), 1, "應該只建立一條連線，實際: {received:?}");
    let lines = &received[0];
    assert_eq!(
        lines.len(),
        1,
        "這條連線應該只送出一行 request，實際: {lines:?}"
    );
    serde_json::from_str(&lines[0])
        .unwrap_or_else(|e| panic!("記錄的行應為合法 JSON: {e}（{}）", lines[0]))
}

/// spec「送出的參數」：以行數上限 200 讀取 pane `w1:p1` 的輸出 → 假 HERDR 收到 `pane.read`，
/// params 為 `pane_id`＝`w1:p1`、`source`＝`recent`、`format`＝`text`、`lines`＝200，
/// 沒有 `strip_ansi` 鍵。
#[tokio::test]
async fn sends_fixed_params_with_max_lines() {
    let config = FakeHerdrConfig::new().with_method_response(
        "pane.read",
        MethodResponse::Success(pane_read_result("w1:p1", "irrelevant", false)),
    );
    let fake = FakeHerdr::start(config).await.expect("啟動假 HERDR 失敗");
    let runtime = runtime(&fake);

    runtime
        .read_output(&PaneId::new("w1:p1"), 200)
        .await
        .expect("read_output 應成功");

    let request = sole_request(&fake);
    assert_eq!(
        request["method"], "pane.read",
        "method 應為 pane.read，實際: {request}"
    );
    let params = &request["params"];
    assert_eq!(params["pane_id"], "w1:p1");
    assert_eq!(params["source"], "recent");
    assert_eq!(params["format"], "text");
    assert_eq!(params["lines"], 200);
    assert!(
        params.get("strip_ansi").is_none(),
        "params 不該帶 strip_ansi 鍵，實際: {params}"
    );
}

/// spec「回應原樣交回」：假 HERDR 回應 `text` 為 `a\nb`、`truncated` 為 `true`、`revision`
/// 為 0 → 得到文字 `a\nb`、`truncated` 為 `true`、格式為純文字。
#[tokio::test]
async fn returns_response_text_and_truncated_as_is() {
    let config = FakeHerdrConfig::new().with_method_response(
        "pane.read",
        MethodResponse::Success(pane_read_result("w1:p1", "a\nb", true)),
    );
    let fake = FakeHerdr::start(config).await.expect("啟動假 HERDR 失敗");
    let runtime = runtime(&fake);

    let output = runtime
        .read_output(&PaneId::new("w1:p1"), 200)
        .await
        .expect("read_output 應成功");

    assert_eq!(output.text, "a\nb");
    assert!(output.truncated, "truncated 應原樣交回 true");
    assert_eq!(output.format, OutputFormat::Text);
}

/// spec「pane 不存在」：假 HERDR 對 `pane.read` 回 `error` 物件、`code` 為 `pane_not_found`
/// → `read_output` 回 `RuntimeError::PaneNotFound`，`pane_id` 就是呼叫時傳入的那個。
#[tokio::test]
async fn pane_not_found_maps_to_pane_not_found() {
    let config = FakeHerdrConfig::new().with_method_response(
        "pane.read",
        MethodResponse::RemoteError {
            code: "pane_not_found".to_string(),
            message: "pane w1:p1 not found".to_string(),
        },
    );
    let fake = FakeHerdr::start(config).await.expect("啟動假 HERDR 失敗");
    let runtime = runtime(&fake);

    let err = runtime
        .read_output(&PaneId::new("w1:p1"), 200)
        .await
        .expect_err("code 為 pane_not_found 時 read_output 應該失敗");

    match err {
        RuntimeError::PaneNotFound { pane_id } => {
            assert_eq!(pane_id, PaneId::new("w1:p1"), "應保留呼叫時傳入的 pane id");
        }
        other => panic!("預期 RuntimeError::PaneNotFound，實際: {other:?}"),
    }
}

/// spec「其他錯誤碼」：假 HERDR 對 `pane.read` 回 `error` 物件、`code` 不是 `pane_not_found`
/// （這裡用 `internal_error`）→ `read_output` 回 `RuntimeError::Failed`，原因字串含該錯誤碼，
/// 不得誤報成 `PaneNotFound`。
#[tokio::test]
async fn other_remote_error_is_failed_with_code() {
    let config = FakeHerdrConfig::new().with_method_response(
        "pane.read",
        MethodResponse::RemoteError {
            code: "internal_error".to_string(),
            message: "something went wrong".to_string(),
        },
    );
    let fake = FakeHerdr::start(config).await.expect("啟動假 HERDR 失敗");
    let runtime = runtime(&fake);

    let err = runtime
        .read_output(&PaneId::new("w1:p1"), 200)
        .await
        .expect_err("其他錯誤碼時 read_output 應該失敗");

    match err {
        RuntimeError::Failed(reason) => {
            assert!(
                reason.contains("internal_error"),
                "原因字串應含錯誤碼 internal_error，實際: {reason}"
            );
        }
        other => panic!("預期 RuntimeError::Failed，實際: {other:?}"),
    }
}

/// spec「回應無法解析」：假 HERDR 對 `pane.read` 回一行不是合法 JSON 的內容 → `read_output`
/// 回 `RuntimeError::Failed`，不得誤報成 `PaneNotFound`。
#[tokio::test]
async fn unparseable_response_is_failed() {
    let config = FakeHerdrConfig::new().with_method_response(
        "pane.read",
        MethodResponse::NonJson("not json at all".to_string()),
    );
    let fake = FakeHerdr::start(config).await.expect("啟動假 HERDR 失敗");
    let runtime = runtime(&fake);

    let err = runtime
        .read_output(&PaneId::new("w1:p1"), 200)
        .await
        .expect_err("回應不是合法 JSON 時 read_output 應該失敗");

    assert!(
        matches!(err, RuntimeError::Failed(_)),
        "預期 RuntimeError::Failed，實際: {err:?}"
    );
}

/// spec「連不上」：HERDR 端點從一開始就不存在任何 server → `read_output` 回
/// `RuntimeError::Failed`，不得是 `PaneNotFound`，也不是 `Unavailable`（讀取輸出不做 WSL
/// 探測、不重試，design D4）。做法同 `herdr-client/tests/request.rs` 的
/// `request_server_not_running`：指向一個從一開始就不存在任何 server 的路徑，不用重試迴圈
/// 也不會有競態。
#[tokio::test]
async fn unreachable_endpoint_is_failed() {
    let path = std::env::temp_dir()
        .join("cockpit-herdr-read-output-test")
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

    let runtime = HerdrRuntime::new(RuntimeId::new("win"), connector, None);

    let err = runtime
        .read_output(&PaneId::new("w1:p1"), 200)
        .await
        .expect_err("端點不存在時 read_output 應該失敗");

    assert!(
        matches!(err, RuntimeError::Failed(_)),
        "預期 RuntimeError::Failed，實際: {err:?}"
    );
}

/// live-output task 3.3：同一個 `HerdrRuntime` 的 `read_output` 彼此排隊，不並發
/// （spec「同 runtime 不並發」；design D5：`HerdrRuntime` 持一把只序列化輸出讀取彼此的
/// `tokio::sync::Mutex`）。4 筆並發、假 HERDR 對 `pane.read` 每筆延遲 200 ms 才回、全部成功、
/// 假 HERDR 觀察到的同時進行中 `pane.read` 連線數最大值為 1。真實時間、多執行緒 runtime，
/// 不用 `start_paused`（brief 精確值）。
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn read_output_is_serialized_per_runtime() {
    let config = FakeHerdrConfig::new().with_method_response(
        "pane.read",
        MethodResponse::Delayed(
            std::time::Duration::from_millis(200),
            Box::new(MethodResponse::Success(pane_read_result(
                "w1:p1", "line", false,
            ))),
        ),
    );
    let fake = FakeHerdr::start(config).await.expect("啟動假 HERDR 失敗");
    let runtime = runtime(&fake);
    let pane = PaneId::new("w1:p1");

    let (r1, r2, r3, r4) = tokio::join!(
        runtime.read_output(&pane, 200),
        runtime.read_output(&pane, 200),
        runtime.read_output(&pane, 200),
        runtime.read_output(&pane, 200),
    );

    for (i, result) in [r1, r2, r3, r4].into_iter().enumerate() {
        result.unwrap_or_else(|e| panic!("第 {i} 筆 read_output 應該成功，實際: {e:?}"));
    }
    assert_eq!(
        fake.max_concurrent_calls("pane.read"),
        1,
        "同一個 runtime 的 read_output 應該排隊、不並發，觀察到的最大並發應為 1"
    );
}

/// 一律回失敗、且會計數呼叫次數的假 `DistroProber`（live-output task 3.4 控制端追加
/// Ruling R9）。刻意一律失敗：若日後有人在 `read_output` 裡加了 WSL 探測，`read_output`
/// 會因為探測失敗而回錯，讓 `read_output_does_not_probe_wsl` 明確轉紅，而不是悄悄探測成功、
/// 測試看不出來。計數用獨立的 `Arc<AtomicUsize>`（而不是 `FakeProber::calls()`），因為
/// `prober` 建構後就被 `Box` 進 `WslProbe`、所有權轉移，測試斷言時已經拿不到它本身。
struct AlwaysFailingCountingProber {
    calls: Arc<AtomicUsize>,
}

#[async_trait]
impl DistroProber for AlwaysFailingCountingProber {
    async fn probe(&self, _distro: &str) -> Result<(), RuntimeError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Err(RuntimeError::Unavailable {
            reason: "AlwaysFailingCountingProber 一律回失敗".to_string(),
            retry_after: Duration::from_secs(60),
        })
    }
}

/// spec「讀取 pane 輸出」本文「讀取輸出不做 WSL 探測、不自行重試」（live-output task 3.4
/// 控制端追加 Ruling R9）：`wsl` 型 runtime 的 `read_output` 不該呼叫 prober——即使 prober
/// 一律回失敗，`read_output` 仍應直接對 HERDR 送 `pane.read` 並成功，探測器呼叫次數為 0。
#[tokio::test]
async fn read_output_does_not_probe_wsl() {
    let config = FakeHerdrConfig::new().with_method_response(
        "pane.read",
        MethodResponse::Success(pane_read_result("w1:p1", "irrelevant", false)),
    );
    let fake = FakeHerdr::start(config).await.expect("啟動假 HERDR 失敗");
    let probe_calls = Arc::new(AtomicUsize::new(0));
    let runtime = HerdrRuntime::new(
        RuntimeId::new("wsl"),
        Arc::from(fake.connector()),
        Some(WslProbe {
            distro: "Ubuntu-24.04".to_string(),
            prober: Box::new(AlwaysFailingCountingProber {
                calls: Arc::clone(&probe_calls),
            }),
            retry_after: Duration::from_secs(60),
        }),
    );

    let output = runtime
        .read_output(&PaneId::new("w1:p1"), 200)
        .await
        .expect("read_output 不做 WSL 探測，即使探測器一律失敗也該成功取得文字");

    assert_eq!(output.text, "irrelevant");
    assert_eq!(
        probe_calls.load(Ordering::SeqCst),
        0,
        "read_output 不該呼叫 WSL 探測器"
    );
}
