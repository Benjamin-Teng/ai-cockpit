//! Task 4.2／4.3 驗收測試：`cockpit::http::router` 的輸出讀取端點與來源檢查 middleware（spec
//! `live-output`「輸出讀取端點」「輸出端點只接受本機同源請求」；design D2、D4、D6、D7）。
//!
//! 同 `cockpit/tests/pipeline_api.rs` 用 `tower::ServiceExt::oneshot` 打 `router`，不開真正的
//! port；`AppState.port` 停在 `build()` 給的值（0），來源檢查用這個值比對 `Host`（design D6
//! 「測試用 port 0 也正確」）。
//!
//! `cockpit` 的測試拿不到 `cockpit-core/tests/common` 的 `FakeRuntime`（那份是 `cockpit-core`
//! 專用的整合測試鷹架），這裡自備一個腳本化的假 `AgentRuntime`（`FakeOutputRuntime`）：依 pane
//! id 查表決定 `read_output` 要回什麼、記下最後一次收到的 `max_lines`，並計數被呼叫的次數
//! （brief 精確值：「不認識的 runtime」與兩個 403 情境都要斷言讀取次數為 0）。實作用
//! `#[async_trait::async_trait]`（live-output fix round 1 R11：`cockpit/Cargo.toml` 的
//! `[dev-dependencies]` 已加 `async-trait`，與 `cockpit-core`／`cockpit-herdr` 同版本，取代
//! fix round 1 之前手寫的 async-trait 展開簽章）。
//!
//! `FakeOutputRuntime` 刻意不用任何鎖把不同 pane 的讀取串起來——那是 `HerdrRuntime` 自己的事
//! （design D5），這裡要驗的是 handler 層「逾時不卡住其他請求」，用全域鎖反而會讓那個測試
//! 失去意義。

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU16, AtomicU32, AtomicUsize, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use axum::body::Body;
use axum::http::{HeaderMap, Request, StatusCode};
use cockpit::http::{self, AppState};
use cockpit_core::{
    AgentRuntime, AnsiColor, OutputSegment, PaneId, PaneOutput, RuntimeError, RuntimeEvents,
    RuntimeId, RuntimeSnapshot, RuntimeStore, SegmentStyle, StoreHandle,
};
use http_body_util::BodyExt;
use serde_json::Value;
use tokio::time::Instant;
use tower::ServiceExt;

const RUNTIME: &str = "win";

/// `FakeOutputRuntime::read_output` 依 pane id 查表決定要回什麼；沒設定過的 pane 一律回
/// `PaneNotFound`。
#[derive(Clone)]
enum Behavior {
    Success {
        text: String,
        truncated: bool,
    },
    /// 帶樣式片段的輸出（live-output-color task 4.1）：用 `PaneOutput::from_segments` 組出。
    Styled {
        segments: Vec<OutputSegment>,
        truncated: bool,
    },
    Failed(String),
    Unavailable {
        reason: String,
    },
    /// 永不回應：模擬 runtime 讀取遲遲不回應（spec 情境「讀取逾時」）。
    Pending,
}

struct FakeOutputRuntime {
    id: RuntimeId,
    behaviors: HashMap<PaneId, Behavior>,
    calls: AtomicUsize,
    /// 最後一次 `read_output` 收到的 `max_lines`（finding 1：驗 handler 真的把
    /// `OUTPUT_MAX_LINES` 傳下去，不是被忽略的死參數）。`0` 代表還沒被呼叫過（合法呼叫的
    /// `max_lines` 恆為正數，不會混淆）。
    last_max_lines: AtomicU32,
}

impl FakeOutputRuntime {
    fn new(id: &str) -> Self {
        Self {
            id: RuntimeId::new(id),
            behaviors: HashMap::new(),
            calls: AtomicUsize::new(0),
            last_max_lines: AtomicU32::new(0),
        }
    }

    fn with_behavior(mut self, pane: &str, behavior: Behavior) -> Self {
        self.behaviors.insert(PaneId::new(pane), behavior);
        self
    }

    /// 目前為止 `read_output` 被呼叫的次數。
    fn call_count(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }

    /// 最後一次 `read_output` 收到的 `max_lines`；從未被呼叫過回 `None`。
    fn last_max_lines(&self) -> Option<u32> {
        match self.last_max_lines.load(Ordering::SeqCst) {
            0 => None,
            n => Some(n),
        }
    }
}

#[async_trait]
impl AgentRuntime for FakeOutputRuntime {
    fn id(&self) -> &RuntimeId {
        &self.id
    }

    async fn snapshot(&self) -> Result<RuntimeSnapshot, RuntimeError> {
        unimplemented!("output_endpoint 測試只驗 read_output，不需要 snapshot")
    }

    async fn subscribe(&self) -> Result<RuntimeEvents, RuntimeError> {
        unimplemented!("output_endpoint 測試只驗 read_output，不需要 subscribe")
    }

    async fn read_output(&self, pane: &PaneId, max_lines: u32) -> Result<PaneOutput, RuntimeError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.last_max_lines.store(max_lines, Ordering::SeqCst);
        match self.behaviors.get(pane) {
            Some(Behavior::Success { text, truncated }) => {
                Ok(PaneOutput::plain(text.clone(), *truncated))
            }
            Some(Behavior::Styled {
                segments,
                truncated,
            }) => Ok(PaneOutput::from_segments(segments.clone(), *truncated)),
            None => Err(RuntimeError::PaneNotFound {
                pane_id: pane.clone(),
            }),
            Some(Behavior::Failed(message)) => Err(RuntimeError::Failed(message.clone())),
            Some(Behavior::Unavailable { reason }) => Err(RuntimeError::Unavailable {
                reason: reason.clone(),
                retry_after: Duration::from_secs(1),
            }),
            Some(Behavior::Pending) => std::future::pending().await,
        }
    }
}

/// 組一份只含一個 runtime（`win`）的 `AppState`；沒有寫入服務（輸出端點不需要）。
fn build(fake: Arc<FakeOutputRuntime>) -> AppState {
    let handle = StoreHandle::new(RuntimeStore::new());
    let mut runtimes: HashMap<RuntimeId, Arc<dyn AgentRuntime>> = HashMap::new();
    runtimes.insert(RuntimeId::new(RUNTIME), fake);
    AppState {
        state: handle.subscribe(),
        progress: None,
        port: Arc::new(AtomicU16::new(0)),
        runtimes: Arc::new(runtimes),
        path_mappings: Arc::new(HashMap::new()),
        files: Arc::new(cockpit::files::FileSettings::embedded()),
        git_runner: Arc::new(cockpit_git::GitRunner::new()),
        activity: cockpit::http::ClientActivity::new(),
    }
}

fn output_uri(runtime: &str, pane: &str) -> String {
    format!("/api/runtimes/{runtime}/panes/{pane}/output")
}

/// 送一個請求進 `router`，固定帶 `Host: 127.0.0.1:0`（同 `pipeline_api.rs::send`；`build()`
/// 組出的 `AppState.port` 初值是 0）。
async fn send(
    router: &axum::Router,
    method: &str,
    uri: &str,
) -> (StatusCode, Value, Option<String>, HeaderMap) {
    send_with_headers(router, method, uri, &[("host", "127.0.0.1:0")]).await
}

/// 同 [`send`]，但自己指定完整的標頭清單（來源檢查測試需要送出跟預設不同、甚至不合法的
/// `Host`／`Origin`）。
async fn send_with_headers(
    router: &axum::Router,
    method: &str,
    uri: &str,
    headers: &[(&str, &str)],
) -> (StatusCode, Value, Option<String>, HeaderMap) {
    let mut builder = Request::builder().method(method).uri(uri);
    for (name, value) in headers {
        builder = builder.header(*name, *value);
    }
    let request = builder.body(Body::empty()).expect("request 建構不應該失敗");
    let response = router
        .clone()
        .oneshot(request)
        .await
        .expect("oneshot 呼叫不應該失敗");
    let status = response.status();
    let response_headers = response.headers().clone();
    let content_type = response_headers
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("body 收集不應該失敗")
        .to_bytes();
    let value = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or(Value::Null)
    };
    (status, value, content_type, response_headers)
}

/// 錯誤回應共通斷言：`error` 欄位非空（同 `pipeline_api.rs::assert_error_response` 的一部分，
/// 這裡不強制 `Content-Type`——呼叫端已個別斷言過）。
fn assert_error_body(body: &Value) {
    assert!(
        body["error"].as_str().is_some_and(|s| !s.is_empty()),
        "錯誤回應應該有非空的 error 欄位，實際：{body:?}"
    );
}

/// R12：這個端點的**所有**回應都要帶 `Cache-Control: no-store` 與
/// `X-Content-Type-Options: nosniff`（不只 200）。
fn assert_no_store_and_nosniff(headers: &HeaderMap) {
    assert_eq!(
        headers.get("cache-control").and_then(|v| v.to_str().ok()),
        Some("no-store"),
        "回應應該帶 Cache-Control: no-store，實際標頭：{headers:?}"
    );
    assert_eq!(
        headers
            .get("x-content-type-options")
            .and_then(|v| v.to_str().ok()),
        Some("nosniff"),
        "回應應該帶 X-Content-Type-Options: nosniff，實際標頭：{headers:?}"
    );
}

fn success_behavior(text: &str) -> Behavior {
    Behavior::Success {
        text: text.to_string(),
        truncated: false,
    }
}

/// 不帶任何樣式的片段（所有樣式欄位為預設值）。
fn plain_segment(text: &str) -> OutputSegment {
    OutputSegment {
        text: text.to_string(),
        style: SegmentStyle::default(),
    }
}

/// `segments` 的共通不變量（spec「輸出讀取端點」）：片段串接等於 `text`、每個片段的 `text`
/// 非空、相鄰片段的樣式不同（以去掉 `text` 的 JSON 鍵值比較）。
fn assert_segments_invariants(body: &Value) {
    let segments = body["segments"].as_array().expect("segments 應該是陣列");
    let joined: String = segments
        .iter()
        .map(|seg| seg["text"].as_str().expect("每個片段都有字串 text"))
        .collect();
    assert_eq!(
        Value::String(joined),
        body["text"],
        "segments 串接應該等於 text"
    );
    for seg in segments {
        assert!(
            seg["text"].as_str().is_some_and(|t| !t.is_empty()),
            "每個片段的 text 都不得為空，實際：{seg:?}"
        );
    }
    let style_of = |seg: &Value| {
        let mut map = seg.as_object().expect("片段是物件").clone();
        map.remove("text");
        map
    };
    for pair in segments.windows(2) {
        assert_ne!(
            style_of(&pair[0]),
            style_of(&pair[1]),
            "相鄰片段的樣式必須不同（否則應合併），實際：{pair:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// spec `live-output`「輸出讀取端點」
// ---------------------------------------------------------------------------

/// Scenario「讀到輸出」（live-output-color task 4.1）：三行、第二行是紅色 `error`；回應帶
/// `segments`，串接等於 `text`，含 `error` 的片段 `fg` 為 `red`。
#[tokio::test]
async fn read_output_returns_segments_with_styled_line() {
    let red = SegmentStyle {
        fg: Some(AnsiColor::Red),
        ..SegmentStyle::default()
    };
    let fake = Arc::new(FakeOutputRuntime::new(RUNTIME).with_behavior(
        "w1:p1",
        Behavior::Styled {
            segments: vec![
                plain_segment("line1\n"),
                OutputSegment {
                    text: "error".to_string(),
                    style: red,
                },
                plain_segment("\nline3"),
            ],
            truncated: false,
        },
    ));
    let router = http::router(build(fake.clone()));

    let (status, body, content_type, headers) =
        send(&router, "GET", &output_uri(RUNTIME, "w1:p1")).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(content_type.as_deref(), Some("application/json"));
    assert_eq!(body["format"], "text");
    assert_eq!(body["text"], "line1\nerror\nline3");
    assert_eq!(body["truncated"], false);
    assert_no_store_and_nosniff(&headers);
    assert_segments_invariants(&body);
    let error_segment = body["segments"]
        .as_array()
        .expect("segments 應該是陣列")
        .iter()
        .find(|seg| seg["text"].as_str().is_some_and(|t| t.contains("error")))
        .expect("應該有含 error 的片段");
    assert_eq!(error_segment["fg"], "red");
    assert_eq!(fake.call_count(), 1);
}

/// Scenario「沒有樣式的輸出」（live-output-color task 4.1）：單行 `hello` 沒有任何樣式，
/// `segments` 恰為 `[{"text":"hello"}]`，不含任何樣式鍵。
#[tokio::test]
async fn read_output_without_style_has_single_unstyled_segment() {
    let fake =
        Arc::new(FakeOutputRuntime::new(RUNTIME).with_behavior("w1:p1", success_behavior("hello")));
    let router = http::router(build(fake));

    let (status, body, _, _) = send(&router, "GET", &output_uri(RUNTIME, "w1:p1")).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["text"], "hello");
    assert_eq!(body["segments"], serde_json::json!([{"text": "hello"}]));
    assert_segments_invariants(&body);
}

/// spec「輸出讀取端點」：`text` 為空字串時 `segments` 為空陣列（端點層；core 層的 `plain("")`／
/// `from_segments` 空輸入另有測試）。live-output-color 4.3 審查 F5。
#[tokio::test]
async fn read_output_empty_text_has_empty_segments_array() {
    let fake =
        Arc::new(FakeOutputRuntime::new(RUNTIME).with_behavior("w1:p1", success_behavior("")));
    let router = http::router(build(fake));

    let (status, body, _, _) = send(&router, "GET", &output_uri(RUNTIME, "w1:p1")).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["text"], "");
    assert_eq!(body["segments"], serde_json::json!([]));
}

/// spec「輸出讀取端點」：200 回應的頂層鍵恰為 `runtime`、`pane_id`、`format`、`text`、`segments`、
/// `truncated`，日後誤加欄位會在這裡被發現。live-output-color 4.3 審查 F5。
#[tokio::test]
async fn read_output_body_has_exactly_the_six_documented_keys() {
    let fake =
        Arc::new(FakeOutputRuntime::new(RUNTIME).with_behavior("w1:p1", success_behavior("hello")));
    let router = http::router(build(fake));

    let (status, body, _, _) = send(&router, "GET", &output_uri(RUNTIME, "w1:p1")).await;

    assert_eq!(status, StatusCode::OK);
    let mut keys: Vec<&str> = body
        .as_object()
        .expect("本體應該是 JSON 物件")
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    let mut expected = vec![
        "runtime",
        "pane_id",
        "format",
        "text",
        "segments",
        "truncated",
    ];
    expected.sort_unstable();
    assert_eq!(keys, expected);
}

/// Scenario「讀到輸出」。
#[tokio::test]
async fn read_output_returns_200_with_headers() {
    let fake = Arc::new(
        FakeOutputRuntime::new(RUNTIME)
            .with_behavior("w1:p1", success_behavior("line1\nline2\nline3")),
    );
    let router = http::router(build(fake.clone()));

    let (status, body, content_type, headers) =
        send(&router, "GET", &output_uri(RUNTIME, "w1:p1")).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(content_type.as_deref(), Some("application/json"));
    assert_eq!(body["runtime"], RUNTIME);
    assert_eq!(body["pane_id"], "w1:p1");
    assert_eq!(body["format"], "text");
    assert_eq!(body["text"], "line1\nline2\nline3");
    assert_eq!(body["truncated"], false);
    assert_no_store_and_nosniff(&headers);
    assert_eq!(fake.call_count(), 1);
    // finding 1：釘住「以常數 200 行呼叫 read_output」，不是隨便一個數字。
    assert_eq!(
        fake.last_max_lines(),
        Some(200),
        "應該以 design D4 的常數 200 行呼叫 read_output"
    );
}

/// Scenario「還有更早的輸出」：`truncated` 原樣透傳為 `true`。
#[tokio::test]
async fn read_output_truncated_passthrough() {
    let fake = Arc::new(FakeOutputRuntime::new(RUNTIME).with_behavior(
        "w1:p1",
        Behavior::Success {
            text: "很長的輸出".to_string(),
            truncated: true,
        },
    ));
    let router = http::router(build(fake));

    let (status, body, _, _) = send(&router, "GET", &output_uri(RUNTIME, "w1:p1")).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["truncated"], true);
}

/// brief 精確值（非 spec 逐字情境，design D6「pane id 含冒號」）：路徑上的 pane id 以
/// `%3A` 編碼形式送出時，axum 的 `Path` 要能正確解碼回原本含冒號的 pane id，回應的
/// `pane_id` 欄位也要是解碼後的值（跟前端 `encodeURIComponent` 組出的路徑一致）。
#[tokio::test]
async fn read_output_pane_id_with_percent_encoded_colon_decodes() {
    let fake =
        Arc::new(FakeOutputRuntime::new(RUNTIME).with_behavior("w1:p1", success_behavior("hi")));
    let router = http::router(build(fake.clone()));

    let (status, body, _, _) = send(
        &router,
        "GET",
        &format!("/api/runtimes/{RUNTIME}/panes/w1%3Ap1/output"),
    )
    .await;

    assert_eq!(status, StatusCode::OK, "%3A 應該被解碼成冒號，找得到 pane");
    assert_eq!(body["pane_id"], "w1:p1");
    assert_eq!(fake.call_count(), 1);
}

/// Scenario「不認識的 runtime」：404，且沒有對任何 runtime 發出讀取。
#[tokio::test]
async fn read_output_unknown_runtime_404_no_read() {
    let fake =
        Arc::new(FakeOutputRuntime::new(RUNTIME).with_behavior("w1:p1", success_behavior("hi")));
    let router = http::router(build(fake.clone()));

    let (status, body, content_type, headers) =
        send(&router, "GET", &output_uri("nope", "w1:p1")).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(content_type.as_deref(), Some("application/json"));
    assert_error_body(&body);
    assert_no_store_and_nosniff(&headers);
    assert_eq!(fake.call_count(), 0, "不認識的 runtime 不該發出讀取");
}

/// Scenario「pane 不存在」。
#[tokio::test]
async fn read_output_pane_not_found_404() {
    let fake = Arc::new(FakeOutputRuntime::new(RUNTIME));
    let router = http::router(build(fake));

    let (status, body, content_type, headers) =
        send(&router, "GET", &output_uri(RUNTIME, "w1:p99")).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(content_type.as_deref(), Some("application/json"));
    assert_error_body(&body);
    assert_no_store_and_nosniff(&headers);
}

/// Scenario「錯誤回應也不可快取」（spec 逐字情境；R12 新增）：以「pane 不存在」→ 404 為代表
/// 案例，其餘狀態碼（403／405／503／504）另外各自在對應測試裡斷言過（見
/// `assert_no_store_and_nosniff` 的呼叫點），這裡只需釘住 spec 給的那個具體情境
/// （runtime `win` 沒有 pane `w1:p99`，請求得到 404）。
#[tokio::test]
async fn read_output_error_responses_are_not_cached() {
    let fake = Arc::new(FakeOutputRuntime::new(RUNTIME));
    let router = http::router(build(fake));

    let (status, _, _, headers) = send(&router, "GET", &output_uri(RUNTIME, "w1:p99")).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_no_store_and_nosniff(&headers);
}

/// Scenario「runtime 斷線」：`Unavailable` → 503。
#[tokio::test]
async fn read_output_unavailable_503() {
    let fake = Arc::new(FakeOutputRuntime::new(RUNTIME).with_behavior(
        "w1:p1",
        Behavior::Unavailable {
            reason: "連不上 wsl".to_string(),
        },
    ));
    let router = http::router(build(fake));

    let (status, body, content_type, headers) =
        send(&router, "GET", &output_uri(RUNTIME, "w1:p1")).await;

    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(content_type.as_deref(), Some("application/json"));
    assert_eq!(body["error"], "連不上 wsl");
    assert_no_store_and_nosniff(&headers);
}

/// design D6 錯誤對應表另一半：`Failed` 同樣 → 503（不是 spec 逐字情境，但同一張表要求的
/// 分支；缺這個測試代表 `RuntimeError::Failed` 這條路徑完全沒被驗過）。
#[tokio::test]
async fn read_output_failed_503() {
    let fake = Arc::new(
        FakeOutputRuntime::new(RUNTIME)
            .with_behavior("w1:p1", Behavior::Failed("回應無法解析".to_string())),
    );
    let router = http::router(build(fake));

    let (status, body, content_type, headers) =
        send(&router, "GET", &output_uri(RUNTIME, "w1:p1")).await;

    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(content_type.as_deref(), Some("application/json"));
    assert_eq!(body["error"], "回應無法解析");
    assert_no_store_and_nosniff(&headers);
}

/// Scenario「讀取逾時」：5 秒後 504，且服務不因此卡住其他請求（對另一個 pane 的請求仍能
/// 正常完成）。假 runtime 不經真實 transport，`start_paused` 安全（design D9）。
///
/// finding 4：不只驗狀態碼，還要釘住「確實是常數 `OUTPUT_READ_TIMEOUT`（5 秒）」與
/// 「另一個請求是在卡住的請求仍進行中時完成的」，而不是隨便什麼時長都能通過：
/// - 送第二個請求之前先斷言 `call_count() == 1`（卡住的請求已經真的呼叫過 `read_output`，
///   卡在 pending 上）且 `stuck_task` 尚未完成；
/// - 第二個請求完成後，`stuck_task` 仍應該尚未完成（證明它不是「剛好也秒回」）；
/// - `stuck_task` 最終完成時量測從送出到完成的總耗時，斷言落在 `[5s, 6s)`——常數被改成
///   50 ms 或 50 s 都會讓這個區間斷言失敗。
#[tokio::test(start_paused = true)]
async fn read_output_timeout_504_does_not_block_other_request() {
    let fake = Arc::new(
        FakeOutputRuntime::new(RUNTIME)
            .with_behavior("stuck", Behavior::Pending)
            .with_behavior("ok", success_behavior("hi")),
    );
    let router = http::router(build(fake.clone()));

    let start = Instant::now();
    let stuck_router = router.clone();
    let stuck_task =
        tokio::spawn(
            async move { send(&stuck_router, "GET", &output_uri(RUNTIME, "stuck")).await },
        );
    // 讓 stuck_task 真的排進去、發出請求、卡在 read_output 的 pending 上，再驗另一個請求
    // 不受它影響。
    tokio::task::yield_now().await;
    assert_eq!(
        fake.call_count(),
        1,
        "stuck 請求應該已經真的呼叫過 read_output（卡在 pending 上）"
    );
    assert!(
        !stuck_task.is_finished(),
        "前提檢查：stuck 請求此時不該已經完成"
    );

    let (ok_status, ok_body, _, ok_headers) =
        send(&router, "GET", &output_uri(RUNTIME, "ok")).await;
    assert_eq!(
        ok_status,
        StatusCode::OK,
        "逾時中的另一個 pane 請求不應被卡住"
    );
    assert_eq!(ok_body["text"], "hi");
    assert_no_store_and_nosniff(&ok_headers);
    assert!(
        !stuck_task.is_finished(),
        "另一個請求完成時，stuck 請求應該仍在進行中——不是剛好也秒回"
    );

    let (stuck_status, stuck_body, _, stuck_headers) = stuck_task.await.expect("task 不應該 panic");
    let elapsed = start.elapsed();
    assert_eq!(stuck_status, StatusCode::GATEWAY_TIMEOUT);
    assert_error_body(&stuck_body);
    assert_no_store_and_nosniff(&stuck_headers);
    assert!(
        elapsed >= Duration::from_secs(5),
        "應該至少等滿 5 秒的 OUTPUT_READ_TIMEOUT 常數，實際只等了 {elapsed:?}"
    );
    assert!(
        elapsed < Duration::from_secs(6),
        "不應該遠超過 5 秒常數（否則代表常數被改掉還是通過），實際等了 {elapsed:?}"
    );

    assert_eq!(fake.call_count(), 2);
}

/// Scenario「不接受其他 method」：405，本體為 `{"error": "<原因>"}`，沒有對 runtime 發出
/// 讀取，且同樣帶兩個安全標頭（R12）。
#[tokio::test]
async fn read_output_wrong_method_405() {
    let fake =
        Arc::new(FakeOutputRuntime::new(RUNTIME).with_behavior("w1:p1", success_behavior("hi")));
    let router = http::router(build(fake.clone()));

    let (status, body, content_type, headers) =
        send(&router, "POST", &output_uri(RUNTIME, "w1:p1")).await;

    assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(content_type.as_deref(), Some("application/json"));
    assert_error_body(&body);
    assert_no_store_and_nosniff(&headers);
    assert_eq!(fake.call_count(), 0, "被拒的 method 不該發出讀取");
}

/// finding B（Codex 複審 round 2；spec 情境「不接受其他 method」逐字要求 `GET` 以外的
/// method 含 `HEAD` 一律 405、不得對 runtime 發出讀取——「一律」代表不論 `Host`／`Origin`
/// 是否合法，跟 `POST` 等其餘 method 同一套優先序，不能被 `source_check` 先攔成 403）：
/// `HEAD` 的 `.head(output_method_not_allowed)` 改掛在 `.route_layer(source_check_layer)`
/// **之後**（`router()`）——axum 的 `MethodRouter::route_layer` 只包它被呼叫**當下**已註冊
/// 的方法插槽，在它之後才註冊的 `head` 插槽不會被那層包住，因此不經 `source_check`，跟
/// `.fallback(...)` 接住的 `POST` 等其餘 method 是同一套優先序。三種案例都要 405、
/// `call_count()==0`、兩個安全標頭：合法 `Host`、不合法 `Host`（`evil.example`）、合法
/// `Host` 但跨站 `Origin`。`HEAD` 依 HTTP 沒有本體：不論命中哪個 handler，axum 都會在
/// top-level Route 依請求方法自動清空本體（見 axum 0.8.9
/// `routing::route::RouteFuture::poll` 對 `Method::HEAD` 的處理），跟這裡回 405 而不是
/// 200 無關。
#[tokio::test]
async fn read_output_head_always_405_regardless_of_host_or_origin() {
    let fake =
        Arc::new(FakeOutputRuntime::new(RUNTIME).with_behavior("w1:p1", success_behavior("hi")));
    let router = http::router(build(fake.clone()));

    // 案例 1：合法 Host。
    let (status, body, content_type, headers) =
        send(&router, "HEAD", &output_uri(RUNTIME, "w1:p1")).await;
    assert_eq!(
        status,
        StatusCode::METHOD_NOT_ALLOWED,
        "合法 Host 的 HEAD 也該回 405，不進 handler"
    );
    assert_eq!(
        content_type.as_deref(),
        Some("application/json"),
        "405 handler 本來就設 JSON content-type，即使 HEAD 本體會被清空"
    );
    assert_eq!(
        body,
        Value::Null,
        "HEAD 回應不該有本體（axum 在 top-level Route 依方法自動清空）"
    );
    assert_no_store_and_nosniff(&headers);

    // 案例 2：不合法 Host（DNS rebinding）——一樣是 405，不是 403；不進 source_check。
    let (status, _, _, headers) = send_with_headers(
        &router,
        "HEAD",
        &output_uri(RUNTIME, "w1:p1"),
        &[("host", "evil.example:0")],
    )
    .await;
    assert_eq!(
        status,
        StatusCode::METHOD_NOT_ALLOWED,
        "Host 不合法的 HEAD 也該回 405（跟 POST 同一套優先序，不被 source_check 攔成 403）"
    );
    assert_no_store_and_nosniff(&headers);

    // 案例 3：合法 Host、跨站 Origin——同樣是 405，不是 403。
    let (status, _, _, headers) = send_with_headers(
        &router,
        "HEAD",
        &output_uri(RUNTIME, "w1:p1"),
        &[("host", "127.0.0.1:0"), ("origin", "https://evil.example")],
    )
    .await;
    assert_eq!(
        status,
        StatusCode::METHOD_NOT_ALLOWED,
        "跨站 Origin 的 HEAD 也該回 405，不是 403"
    );
    assert_no_store_and_nosniff(&headers);

    assert_eq!(
        fake.call_count(),
        0,
        "三種情境下 HEAD 都不該對 runtime 發出讀取"
    );
}

/// finding 7（fix round 1；fix round 2 收緊）：怪異的 pane id 不得讓 server panic——狀態碼
/// 落在 4xx 即可，不能是 5xx。`a%2Fb`（編碼斜線）與空 pane 段（`//output`）都實測確實命中
/// 這條路由（fix round 2 修正 fix round 1 的錯誤假設：matchit 0.8.4 把空字串也接受成一個
/// 合法的路徑片段值，不是「路由不匹配」），解碼後查表查無此 pane，兩者都在 handler 內變成
/// `PaneNotFound` 404，都屬於「這個端點的回應」，收緊到都驗 JSON error 本體與兩個安全標頭
/// ——同 `read_output_pane_not_found_404`。
#[tokio::test]
async fn read_output_weird_pane_id_does_not_panic() {
    let fake =
        Arc::new(FakeOutputRuntime::new(RUNTIME).with_behavior("w1:p1", success_behavior("hi")));
    let router = http::router(build(fake.clone()));

    for (uri, label) in [
        (
            format!("/api/runtimes/{RUNTIME}/panes/a%2Fb/output"),
            "編碼斜線",
        ),
        (format!("/api/runtimes/{RUNTIME}/panes//output"), "空"),
    ] {
        let (status, body, content_type, headers) = send(&router, "GET", &uri).await;
        assert_eq!(
            status,
            StatusCode::NOT_FOUND,
            "{label} pane id（{uri}）解碼後查無此 pane，應該回 404，不能是 5xx"
        );
        assert_eq!(content_type.as_deref(), Some("application/json"));
        assert_error_body(&body);
        assert_no_store_and_nosniff(&headers);
    }

    assert_eq!(
        fake.call_count(),
        2,
        "兩種怪異 pane id 都真的進了 handler、各呼叫過一次 read_output"
    );
}

/// Codex adversarial review finding（fix round 2，medium）：`read_pane_output` 直接用
/// `Path<(String, String)>`，路徑片段含無效 UTF-8 位元組時，axum 的 `Path` extractor 在進入
/// handler **之前**就會產生一個 rejection 回應——這個回應不經過 [`error_response`]，缺兩個
/// 安全標頭、本體也不是 `{"error": ...}`。分別對 pane 段與 runtime 段送 `%FF`（單一位元組
/// `0xFF` 不是任何合法 UTF-8 序列的開頭，percent-decode 後必定觸發 rejection）。
#[tokio::test]
async fn read_output_invalid_utf8_path_is_json_error_with_security_headers() {
    let fake =
        Arc::new(FakeOutputRuntime::new(RUNTIME).with_behavior("w1:p1", success_behavior("hi")));
    let router = http::router(build(fake.clone()));

    for uri in [
        format!("/api/runtimes/{RUNTIME}/panes/%FF/output"),
        "/api/runtimes/%FF/panes/w1:p1/output".to_string(),
    ] {
        let (status, body, content_type, headers) = send(&router, "GET", &uri).await;
        // 固定為 400（實測 axum 0.8.9 的 `PathRejection::FailedToDeserializePathParams`
        // 本身就是 400；我們的 rejection handler 沿用同一個狀態碼，不改成 404 混淆「查無
        // 資源」與「路徑格式錯誤」這兩種不同語意）。
        assert_eq!(
            status,
            StatusCode::BAD_REQUEST,
            "無效 UTF-8 路徑（{uri}）應該回 400，實際：{status}"
        );
        assert_eq!(
            content_type.as_deref(),
            Some("application/json"),
            "無效 UTF-8 路徑（{uri}）的回應應該是 JSON 錯誤本體，實際 content-type：{content_type:?}"
        );
        assert_error_body(&body);
        assert_no_store_and_nosniff(&headers);
    }

    assert_eq!(
        fake.call_count(),
        0,
        "無效 UTF-8 路徑不該對任何 runtime 發出讀取"
    );
}

// ---------------------------------------------------------------------------
// spec `live-output`「輸出端點只接受本機同源請求」
// ---------------------------------------------------------------------------

/// Scenario「DNS rebinding 被拒」。
#[tokio::test]
async fn read_output_dns_rebinding_403_no_read() {
    let fake =
        Arc::new(FakeOutputRuntime::new(RUNTIME).with_behavior("w1:p1", success_behavior("hi")));
    let router = http::router(build(fake.clone()));

    let (status, body, content_type, headers) = send_with_headers(
        &router,
        "GET",
        &output_uri(RUNTIME, "w1:p1"),
        &[("host", "evil.example:0")],
    )
    .await;

    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(content_type.as_deref(), Some("application/json"));
    assert_error_body(&body);
    assert_no_store_and_nosniff(&headers);
    assert_eq!(fake.call_count(), 0);
}

/// Scenario「跨站請求被拒」。
#[tokio::test]
async fn read_output_cross_site_origin_403_no_read() {
    let fake =
        Arc::new(FakeOutputRuntime::new(RUNTIME).with_behavior("w1:p1", success_behavior("hi")));
    let router = http::router(build(fake.clone()));

    let (status, body, content_type, headers) = send_with_headers(
        &router,
        "GET",
        &output_uri(RUNTIME, "w1:p1"),
        &[("host", "127.0.0.1:0"), ("origin", "https://evil.example")],
    )
    .await;

    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(content_type.as_deref(), Some("application/json"));
    assert_error_body(&body);
    assert_no_store_and_nosniff(&headers);
    assert_eq!(fake.call_count(), 0);
}

/// Scenario「自家頁面與命令列可用」：兩種合法組合都回 200。
#[tokio::test]
async fn read_output_same_origin_and_cli_accepted() {
    let fake =
        Arc::new(FakeOutputRuntime::new(RUNTIME).with_behavior("w1:p1", success_behavior("hi")));
    let router = http::router(build(fake.clone()));

    let (status, _, _, _) = send_with_headers(
        &router,
        "GET",
        &output_uri(RUNTIME, "w1:p1"),
        &[("host", "127.0.0.1:0")],
    )
    .await;
    assert_eq!(status, StatusCode::OK, "命令列（只有 Host）應該被接受");

    let (status, _, _, _) = send_with_headers(
        &router,
        "GET",
        &output_uri(RUNTIME, "w1:p1"),
        &[("host", "localhost:0"), ("origin", "http://localhost:0")],
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "自家頁面（Host＋相符 Origin）應該被接受"
    );

    assert_eq!(fake.call_count(), 2);
}

/// 額外情境（brief 要求）：`Host` 標頭重複出現一律 403，不看第一個值。
#[tokio::test]
async fn read_output_duplicate_host_403() {
    let fake =
        Arc::new(FakeOutputRuntime::new(RUNTIME).with_behavior("w1:p1", success_behavior("hi")));
    let router = http::router(build(fake.clone()));

    let (status, body, content_type, headers) = send_with_headers(
        &router,
        "GET",
        &output_uri(RUNTIME, "w1:p1"),
        &[("host", "127.0.0.1:0"), ("host", "evil.example:0")],
    )
    .await;

    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(content_type.as_deref(), Some("application/json"));
    assert_error_body(&body);
    assert_no_store_and_nosniff(&headers);
    assert_eq!(fake.call_count(), 0);
}
