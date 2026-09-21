//! `cockpit` example：用固定 fixture 起一個真的 dashboard server，之後每個推送間隔（預設 2 秒）輪替一個
//! pane 的 `agent_status`，供瀏覽器工具與人眼檢查（design D14；Task 3.5）。不需要 HERDR，
//! 也不需要真正的 runtime。
//!
//! ```text
//! cargo run -p cockpit --example ui_preview
//! ```
//!
//! 預設監聽 `127.0.0.1:7770`；可用環境變數 `COCKPIT_PREVIEW_LISTEN` 覆寫（例如
//! `COCKPIT_PREVIEW_LISTEN=127.0.0.1:8080`）。Ctrl-C 結束。
//!
//! 推送間隔預設 2 秒；`COCKPIT_PREVIEW_PUSH_MS=100` 切成每 100 ms 推送一份新 version 的模式
//! （spec `cockpit-dashboard`「頻繁重畫時按鈕仍有效」；task 5.3）。
//!
//! 寫入端點（`POST /api/projects/{project}/tasks/{task}/{op}`、`PUT`／`DELETE
//! /api/projects/{project}/workstreams/{workstream}/override`）在這裡**只記錄請求並回 204**，
//! 不改投影：每筆請求在 stdout 印一行 `write-request <METHOD> <PATH> <BODY>`，供瀏覽器驗收
//! 腳本（`docs/research/2026-09-16/actions-check.js`）比對畫面送出了什麼（task 5.3）。
//!
//! 要模擬慢回應或被拒絕時，設 `COCKPIT_PREVIEW_WRITE_RULES`：以 `;` 分隔的
//! `<PATH>=<延遲毫秒>:<狀態碼>`，例如
//! `COCKPIT_PREVIEW_WRITE_RULES=/api/projects/cockpit/tasks/be-1/fail=1500:409`。符合路徑的
//! 請求照樣先記錄，再等指定延遲、回指定狀態碼（非 2xx 附 `{"error": ...}` 本體）；其他路徑
//! 仍立即回 204（task 5.3 fix round 1）。
//!
//! ## 輸出讀取端點的假 `AgentRuntime`（live-output task 5.1）
//!
//! 掛的是與正式服務**相同**的路由與來源檢查（`cockpit::http::router` 的
//! `GET /api/runtimes/{runtime}/panes/{pane}/output`；design D9 第三點）——這裡不另外寫一份
//! handler。假 runtime 的 id 固定是 `win`（跟投影裡的 runtime 一致），能服務的 pane 來自
//! fixture（`wJ:p1`、`wJ:p2`、`wJ:p3`）。每次請求進到假 runtime 的 `read_output` 時，stdout
//! 印一行 `output-request <runtime> <pane>`（被來源檢查擋下的 403 請求不會進到這裡，自然不
//! 印）。
//!
//! 每個 pane 的行為（模式）預設：
//!
//! - `wJ:p1`：`ticker`——內容每秒多一行（`line 1`、`line 2`、……），立即回應。
//! - `wJ:p3`：`long`——一開始就有 300+ 行、`truncated=true`，內容仍每秒多一行。
//! - `wJ:p2`：`notfound`——一律 404（這個 pane 在 fixture 裡 `exited=true`，本來就不能從
//!   pane 列點選；要測「pane 已不存在」的 404 情境，直接對它的路徑發請求，或用
//!   `COCKPIT_PREVIEW_OUTPUT_MODES` 把 `notfound` 疊到一個可點選的 pane 上）。
//!
//! 用 `COCKPIT_PREVIEW_OUTPUT_MODES` 覆寫或新增：`;` 分隔的 `<pane>=<模式>` 規則（同
//! `COCKPIT_PREVIEW_WRITE_RULES` 的分隔慣例），例如
//! `COCKPIT_PREVIEW_OUTPUT_MODES=wJ:p1=delay:3000;wJ:p3=fail:2`。可用的模式：
//!
//! - `ticker`：同預設，內容每秒多一行、立即回應。
//! - `long`：同預設，一開始 300+ 行、`truncated=true`，之後仍每秒多一行。
//! - `delay:<毫秒>`：內容同 `ticker`（每秒多一行），但等待 `<毫秒>` 之後才回應——供「舊回應
//!   不蓋掉新選取」「請求不堆積」。
//! - `fail:<次數>`：前 `<次數>` 次請求回 503（`RuntimeError::Unavailable`），之後恢復成
//!   `ticker`——供「runtime 斷線後恢復」。
//! - `notfound`：一律 404（`RuntimeError::PaneNotFound`）。
//! - `html`：固定回一段含 `<script>window.pwned=1</script>` 與 `<b>x</b>` 的文字，內容不隨
//!   時間變化——供「內容不被當成 HTML」。
//!
//! 「pane 被關掉」情境（spec「失敗與消失的呈現」）：設
//! `COCKPIT_PREVIEW_VANISH_PANE=<pane>=<毫秒>`，該 pane 會在推送迴圈經過那麼久之後，從之後
//! 每一份推送的投影中被拿掉（例如 `COCKPIT_PREVIEW_VANISH_PANE=wJ:p1=5000`：5 秒後
//! `wJ:p1` 消失）。只支援一個 pane。

use std::collections::{HashMap, HashSet};
use std::env;
use std::sync::Arc;
use std::sync::atomic::{AtomicU16, AtomicU32, Ordering};
use std::time::{Duration, SystemTime};

use anyhow::Context;
use async_trait::async_trait;
use axum::Router;
use axum::body::Bytes;
use axum::extract::State;
use axum::http::{Method, StatusCode, Uri, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{post, put};
use cockpit::http::{AppState, router};
use cockpit_core::{
    AgentRuntime, AgentStatus, OutputFormat, PaneId, PaneOutput, ProjectedState, RuntimeError,
    RuntimeEvents, RuntimeId, RuntimeSnapshot,
};
use tokio::net::TcpListener;
use tokio::sync::watch;
use tokio::time::{Instant, interval_at};

/// Task 3.5 手工改過的 fixture（`cockpit/tests/fixtures/projected-state.json`）：兩個
/// runtime（`win` connected、`wsl` disconnected 附原因），`win` 底下一個 workspace、一個
/// tab、三個 pane。跟 `cockpit/tests/fixture.rs` 的 `fixture_projected_state_deserializes`
/// 共用同一份檔案。
const FIXTURE: &str = include_str!("../tests/fixtures/projected-state.json");

const DEFAULT_LISTEN: &str = "127.0.0.1:7770";

const DEFAULT_PUSH_MS: u64 = 2000;

/// 假 `AgentRuntime` 的 id；必須跟 fixture 裡的 runtime id 一致（task 5.1 brief）。
const OUTPUT_RUNTIME_ID: &str = "win";

/// `html` 模式的固定內容（task 5.1 brief 逐字：含 `<script>` 與 `<b>` 字樣）。
const HTML_PROBE_TEXT: &str = "before\n<script>window.pwned=1</script>\n<b>x</b>\nafter";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let initial: ProjectedState =
        serde_json::from_str(FIXTURE).expect("fixture 應該能反序列化成 ProjectedState");
    // R14 要在 `initial` 被搬進 `Arc::new` 之前先蒐集 pane id 集合，晚一步就借不到了。
    let known_panes = known_pane_ids(&initial);

    let (tx, rx) = watch::channel(Arc::new(initial));

    let listen_addr =
        env::var("COCKPIT_PREVIEW_LISTEN").unwrap_or_else(|_| DEFAULT_LISTEN.to_string());
    let listener = TcpListener::bind(&listen_addr).await?;
    let actual_addr = listener.local_addr()?;

    let push_every = push_interval()?;
    let write_rules = Arc::new(write_rules()?);
    let output_modes =
        parse_output_modes(env::var("COCKPIT_PREVIEW_OUTPUT_MODES").ok().as_deref())?;
    let vanish = parse_vanish_pane(env::var("COCKPIT_PREVIEW_VANISH_PANE").ok().as_deref())?;
    // R14（live-output task 5.1 規格對照檢查的控制端追加）：這兩個環境變數指到 fixture
    // 投影裡不存在的 pane id 時，原本會靜默成功、對那個打錯的 pane id 套用預設行為——驗收
    // 腳本如果打錯 pane id，會在假 runtime 的預設行為下意外變成假綠。啟動時就擋下來，給
    // 清楚的錯誤訊息。
    validate_known_panes(&output_modes, &vanish, &known_panes)?;

    let fake_runtime = Arc::new(FakeOutputRuntime::new(OUTPUT_RUNTIME_ID, output_modes));
    let mut runtimes: HashMap<RuntimeId, Arc<dyn AgentRuntime>> = HashMap::new();
    runtimes.insert(RuntimeId::new(OUTPUT_RUNTIME_ID), fake_runtime);

    // brief（task 5.1）：`AppState::new` 的 port 初值是 0，`source_check` 會把帶明確埠號的
    // `Host`（例如瀏覽器與 curl 送來的 `Host: 127.0.0.1:<port>`）一律擋成 403（見
    // `AppState::new` 文件「fail-closed」那段）。這裡的 `listener` 已經在組 `AppState` 之前
    // 綁定好，所以直接把系統實際指派的埠寫進去即可，不需要像
    // `cockpit::app::run_with_shutdown` 那樣先組路由表、綁定後才回填。
    let app_state = AppState {
        state: rx,
        progress: None,
        port: Arc::new(AtomicU16::new(actual_addr.port())),
        runtimes: Arc::new(runtimes),
    };

    // 寫入路由放外層、其餘交給真正的 dashboard router 當 fallback：`Router::merge` 遇到同一
    // 路徑已有 POST／PUT／DELETE（http.rs 的正式寫入端點）會 panic，fallback 則只在外層沒有
    // 符合的路徑時才轉交。外層路徑符合但方法不符（例如 GET）由外層回 405，跟正式路由一致。
    let app = Router::new()
        .route(
            "/api/projects/{project}/tasks/{task}/{op}",
            post(record_write_request),
        )
        .route(
            "/api/projects/{project}/workstreams/{workstream}/override",
            put(record_write_request).delete(record_write_request),
        )
        .fallback_service(router(app_state))
        .with_state(write_rules);

    println!("ui_preview 監聽 http://{actual_addr}（Ctrl-C 結束）");
    println!(
        "推送間隔 {} ms；寫入請求只記錄、回 204",
        push_every.as_millis()
    );
    println!("試試：curl http://{actual_addr}/api/state");
    println!(
        "輸出讀取端點範例：curl -H \"Host: 127.0.0.1:{}\" http://{actual_addr}/api/runtimes/{OUTPUT_RUNTIME_ID}/panes/wJ:p1/output",
        actual_addr.port()
    );

    let cycle_task = tokio::spawn(push_loop(tx, push_every, vanish, Instant::now()));

    tokio::select! {
        result = axum::serve(listener, app) => {
            result?;
        }
        _ = tokio::signal::ctrl_c() => {
            println!("收到 Ctrl-C，結束 ui_preview");
        }
    }

    cycle_task.abort();
    Ok(())
}

/// `COCKPIT_PREVIEW_PUSH_MS`（正整數毫秒）→ 推送間隔；未設定為 2 秒。
fn push_interval() -> anyhow::Result<Duration> {
    match env::var("COCKPIT_PREVIEW_PUSH_MS") {
        Err(_) => Ok(Duration::from_millis(DEFAULT_PUSH_MS)),
        Ok(raw) => {
            let ms: u64 = raw.parse().with_context(|| {
                format!("COCKPIT_PREVIEW_PUSH_MS 必須是正整數毫秒，實際 {raw:?}")
            })?;
            anyhow::ensure!(ms > 0, "COCKPIT_PREVIEW_PUSH_MS 必須大於 0");
            Ok(Duration::from_millis(ms))
        }
    }
}

/// `COCKPIT_PREVIEW_WRITE_RULES` 的一條規則：路徑完全相符時延遲 `delay` 後回 `status`。
struct WriteRule {
    path: String,
    delay: Duration,
    status: StatusCode,
}

/// 解析 `COCKPIT_PREVIEW_WRITE_RULES`（格式見檔頭）；未設定為空。
fn write_rules() -> anyhow::Result<Vec<WriteRule>> {
    let Ok(raw) = env::var("COCKPIT_PREVIEW_WRITE_RULES") else {
        return Ok(Vec::new());
    };
    raw.split(';')
        .filter(|entry| !entry.trim().is_empty())
        .map(|entry| {
            let (path, spec) = entry
                .trim()
                .rsplit_once('=')
                .with_context(|| format!("規則缺 `=`：{entry:?}"))?;
            let (delay_ms, status) = spec
                .split_once(':')
                .with_context(|| format!("規則缺 `:`：{entry:?}"))?;
            Ok(WriteRule {
                path: path.to_string(),
                delay: Duration::from_millis(
                    delay_ms
                        .parse()
                        .with_context(|| format!("延遲不是整數毫秒：{entry:?}"))?,
                ),
                status: StatusCode::from_u16(
                    status
                        .parse()
                        .with_context(|| format!("狀態碼不是整數：{entry:?}"))?,
                )
                .with_context(|| format!("狀態碼不合法：{entry:?}"))?,
            })
        })
        .collect()
}

/// 寫入端點的替身：記錄請求（stdout 一行 `write-request <METHOD> <PATH> <BODY>`），不改投影
/// ——畫面收到的新投影仍只來自推送迴圈（task 5.3）。預設立即回 204；路徑符合
/// `COCKPIT_PREVIEW_WRITE_RULES` 時延遲後回指定狀態碼（fix round 1）。
async fn record_write_request(
    State(rules): State<Arc<Vec<WriteRule>>>,
    method: Method,
    uri: Uri,
    body: Bytes,
) -> Response {
    println!(
        "write-request {method} {} {}",
        uri.path(),
        String::from_utf8_lossy(&body)
    );
    let Some(rule) = rules.iter().find(|rule| rule.path == uri.path()) else {
        return StatusCode::NO_CONTENT.into_response();
    };
    tokio::time::sleep(rule.delay).await;
    if rule.status.is_success() {
        return rule.status.into_response();
    }
    let body =
        serde_json::json!({ "error": format!("ui_preview 模擬回應 {}", rule.status.as_u16()) })
            .to_string();
    (
        rule.status,
        [(header::CONTENT_TYPE, "application/json")],
        body,
    )
        .into_response()
}

// ---------------------------------------------------------------------------
// live-output task 5.1：輸出讀取端點的假 AgentRuntime
// ---------------------------------------------------------------------------

/// 單一 pane 的腳本化輸出行為（brief 逐字列出的六種）。
#[derive(Debug)]
enum OutputMode {
    /// 內容隨時間增加：起始行數 `start_lines`（`ticker` 為 0、`long` 為 300），之後每秒多一
    /// 行；回應前先等待 `delay`（`ticker`／`long` 為 0，`delay:<ms>` 用指定值）。行內容固定
    /// 為 `line <N>`，讓驗收腳本能斷言「N 秒內出現新行」。
    Growing { start_lines: u64, delay: Duration },
    /// 前 `remaining` 次呼叫回 `RuntimeError::Unavailable`（→ 503），之後恢復成起始行數 0、
    /// 無延遲的 `Growing`。用 `AtomicU32` 是因為 `AgentRuntime::read_output` 只拿 `&self`。
    FailThenRecover { remaining: AtomicU32 },
    /// 一律 `RuntimeError::PaneNotFound`（→ 404）。
    NotFound,
    /// 固定回 [`HTML_PROBE_TEXT`]，內容不隨時間變化。
    Html,
}

/// 假 `AgentRuntime`：`snapshot`／`subscribe` 不會被呼叫到（ui_preview 不經
/// `cockpit_core::driver::run`），`read_output` 依 pane id 查 `panes` 決定行為。
struct FakeOutputRuntime {
    id: RuntimeId,
    /// 供 [`OutputMode::Growing`] 算「已經過幾秒」的起點；整個 runtime 只有一個，讓不同 pane
    /// 的內容成長速度可比較（例如 `ticker` 與 `long` 在同一秒數下的行數差固定是
    /// `start_lines`）。
    started: Instant,
    panes: HashMap<PaneId, OutputMode>,
}

impl FakeOutputRuntime {
    fn new(id: &str, panes: HashMap<PaneId, OutputMode>) -> Self {
        Self {
            id: RuntimeId::new(id),
            started: Instant::now(),
            panes,
        }
    }
}

#[async_trait]
impl AgentRuntime for FakeOutputRuntime {
    fn id(&self) -> &RuntimeId {
        &self.id
    }

    async fn snapshot(&self) -> Result<RuntimeSnapshot, RuntimeError> {
        unimplemented!("ui_preview 不經 driver，這個假 runtime 只服務輸出讀取端點")
    }

    async fn subscribe(&self) -> Result<RuntimeEvents, RuntimeError> {
        unimplemented!("ui_preview 不經 driver，這個假 runtime 只服務輸出讀取端點")
    }

    async fn read_output(&self, pane: &PaneId, max_lines: u32) -> Result<PaneOutput, RuntimeError> {
        // brief：每次請求在 stdout 印一行；被 source_check 擋下的 403 請求不會走到這裡，自然
        // 不印。沒設定過模式的 pane（包含真的不存在的 pane id）視同 PaneNotFound。
        println!("output-request {} {}", self.id, pane);
        let Some(mode) = self.panes.get(pane) else {
            return Err(RuntimeError::PaneNotFound {
                pane_id: pane.clone(),
            });
        };
        match mode {
            OutputMode::NotFound => Err(RuntimeError::PaneNotFound {
                pane_id: pane.clone(),
            }),
            OutputMode::Html => Ok(PaneOutput {
                format: OutputFormat::Text,
                text: HTML_PROBE_TEXT.to_string(),
                truncated: false,
            }),
            OutputMode::Growing { start_lines, delay } => {
                if !delay.is_zero() {
                    tokio::time::sleep(*delay).await;
                }
                Ok(growing_output(*start_lines, self.started, max_lines))
            }
            OutputMode::FailThenRecover { remaining } => {
                let previous = remaining
                    .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |r| {
                        Some(r.saturating_sub(1))
                    })
                    .expect("closure 一律回傳 Some，fetch_update 不會失敗");
                if previous > 0 {
                    return Err(RuntimeError::Unavailable {
                        reason: format!("ui_preview 模擬斷線（還剩 {previous} 次恢復前）"),
                        retry_after: Duration::from_millis(500),
                    });
                }
                Ok(growing_output(0, self.started, max_lines))
            }
        }
    }
}

/// [`OutputMode::Growing`] 的內容產生：`started` 起算的秒數決定目前總行數
/// （`start_lines + elapsed_secs + 1`，讓 `elapsed == 0` 時就已經有第一行），回應只取最後
/// `max_lines` 行，`truncated` 為總行數是否超過 `max_lines`。
fn growing_output(start_lines: u64, started: Instant, max_lines: u32) -> PaneOutput {
    let elapsed_secs = started.elapsed().as_secs();
    let total_lines = start_lines + elapsed_secs + 1;
    let max_lines = u64::from(max_lines);
    let first_line = total_lines.saturating_sub(max_lines) + 1;
    let text = (first_line..=total_lines)
        .map(|n| format!("line {n}"))
        .collect::<Vec<_>>()
        .join("\n");
    PaneOutput {
        format: OutputFormat::Text,
        text,
        truncated: total_lines > max_lines,
    }
}

/// 沒有 `COCKPIT_PREVIEW_OUTPUT_MODES` 覆寫時的預設分配（檔頭文件節錄）：`wJ:p1` 為
/// `ticker`、`wJ:p3` 為 `long`、`wJ:p2`（`exited`）為 `notfound`。
fn default_output_modes() -> HashMap<PaneId, OutputMode> {
    let mut modes = HashMap::new();
    modes.insert(
        PaneId::new("wJ:p1"),
        OutputMode::Growing {
            start_lines: 0,
            delay: Duration::ZERO,
        },
    );
    modes.insert(
        PaneId::new("wJ:p3"),
        OutputMode::Growing {
            start_lines: 300,
            delay: Duration::ZERO,
        },
    );
    modes.insert(PaneId::new("wJ:p2"), OutputMode::NotFound);
    modes
}

/// 解析 `COCKPIT_PREVIEW_OUTPUT_MODES`（格式見檔頭）：以 [`default_output_modes`] 為底，逐條
/// 規則覆寫或新增。未設定時原樣回傳預設值。
fn parse_output_modes(raw: Option<&str>) -> anyhow::Result<HashMap<PaneId, OutputMode>> {
    let mut modes = default_output_modes();
    let Some(raw) = raw else {
        return Ok(modes);
    };
    for entry in raw
        .split(';')
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
    {
        let (pane, mode_spec) = entry
            .split_once('=')
            .with_context(|| format!("COCKPIT_PREVIEW_OUTPUT_MODES 規則缺 `=`：{entry:?}"))?;
        let mode = parse_output_mode(mode_spec)
            .with_context(|| format!("COCKPIT_PREVIEW_OUTPUT_MODES 規則不合法：{entry:?}"))?;
        modes.insert(PaneId::new(pane), mode);
    }
    Ok(modes)
}

/// 解析單一模式字串（`<模式>` 或 `<模式>:<參數>`），[`parse_output_modes`] 拆出 `<pane>=` 之後
/// 剩下的部分。
fn parse_output_mode(spec: &str) -> anyhow::Result<OutputMode> {
    match spec.split_once(':') {
        Some(("delay", ms)) => Ok(OutputMode::Growing {
            start_lines: 0,
            delay: Duration::from_millis(
                ms.parse()
                    .with_context(|| format!("delay 的毫秒數不是整數：{ms:?}"))?,
            ),
        }),
        Some(("fail", count)) => Ok(OutputMode::FailThenRecover {
            remaining: AtomicU32::new(
                count
                    .parse()
                    .with_context(|| format!("fail 的次數不是整數：{count:?}"))?,
            ),
        }),
        Some((other, _)) => anyhow::bail!("不認識的模式：{other:?}"),
        None => match spec {
            "ticker" => Ok(OutputMode::Growing {
                start_lines: 0,
                delay: Duration::ZERO,
            }),
            "long" => Ok(OutputMode::Growing {
                start_lines: 300,
                delay: Duration::ZERO,
            }),
            "notfound" => Ok(OutputMode::NotFound),
            "html" => Ok(OutputMode::Html),
            other => anyhow::bail!("不認識的模式：{other:?}"),
        },
    }
}

// ---------------------------------------------------------------------------
// live-output task 5.1：pane 消失（spec「失敗與消失的呈現」Scenario「pane 被關掉」）
// ---------------------------------------------------------------------------

/// `COCKPIT_PREVIEW_VANISH_PANE` 的設定：`pane` 在推送迴圈經過 `after` 之後，從之後每一份
/// 推送的投影中被拿掉。
#[derive(Debug)]
struct VanishConfig {
    pane: PaneId,
    after: Duration,
}

/// 解析 `COCKPIT_PREVIEW_VANISH_PANE=<pane>=<毫秒>`（例如
/// `COCKPIT_PREVIEW_VANISH_PANE=wJ:p1=5000`）；未設定為 `None`。`pane` id 本身可能含冒號
/// （fixture 的 pane id 都是 `wJ:p1` 這種形式），所以用第一個 `=` 切，不能用 `:` 切。
fn parse_vanish_pane(raw: Option<&str>) -> anyhow::Result<Option<VanishConfig>> {
    let Some(raw) = raw else {
        return Ok(None);
    };
    let raw = raw.trim();
    if raw.is_empty() {
        return Ok(None);
    }
    let (pane, after_ms) = raw
        .split_once('=')
        .with_context(|| format!("COCKPIT_PREVIEW_VANISH_PANE 缺 `=`：{raw:?}"))?;
    let after = Duration::from_millis(
        after_ms
            .parse()
            .with_context(|| format!("COCKPIT_PREVIEW_VANISH_PANE 的毫秒數不是整數：{raw:?}"))?,
    );
    Ok(Some(VanishConfig {
        pane: PaneId::new(pane),
        after,
    }))
}

/// 蒐集 `state` 裡所有 runtime／workspace／tab 底下出現過的 pane id（R14：驗證
/// `COCKPIT_PREVIEW_OUTPUT_MODES`／`COCKPIT_PREVIEW_VANISH_PANE` 有沒有指到打錯的 pane id）。
fn known_pane_ids(state: &ProjectedState) -> HashSet<PaneId> {
    state
        .runtimes
        .iter()
        .flat_map(|runtime| &runtime.workspaces)
        .flat_map(|workspace| &workspace.tabs)
        .flat_map(|tab| &tab.panes)
        .map(|pane| pane.id.clone())
        .collect()
}

/// R14：`output_modes`（`COCKPIT_PREVIEW_OUTPUT_MODES` 解析結果，含預設值）與 `vanish`
/// （`COCKPIT_PREVIEW_VANISH_PANE` 解析結果）提到的每個 pane id 都必須在 `known` 之中，否則
/// 回清楚的錯誤（列出打錯的 pane id）。預設值本身一定在 fixture 裡，只有使用者自己疊加的
/// 規則才可能打錯。
fn validate_known_panes(
    output_modes: &HashMap<PaneId, OutputMode>,
    vanish: &Option<VanishConfig>,
    known: &HashSet<PaneId>,
) -> anyhow::Result<()> {
    let mut unknown: Vec<&PaneId> = output_modes
        .keys()
        .filter(|pane| !known.contains(*pane))
        .collect();
    unknown.sort();
    anyhow::ensure!(
        unknown.is_empty(),
        "COCKPIT_PREVIEW_OUTPUT_MODES 指到 fixture 投影裡不存在的 pane id：{}",
        unknown
            .iter()
            .map(|pane| pane.to_string())
            .collect::<Vec<_>>()
            .join(", ")
    );
    if let Some(vanish) = vanish {
        anyhow::ensure!(
            known.contains(&vanish.pane),
            "COCKPIT_PREVIEW_VANISH_PANE 指到 fixture 投影裡不存在的 pane id：{}",
            vanish.pane
        );
    }
    Ok(())
}

/// 把 `target` 從 `state` 所有 runtime／workspace／tab 的 pane 清單中移除（[`VanishConfig`]
/// 用：模擬 pane 被關掉後從投影消失）。
fn remove_pane(state: &mut ProjectedState, target: &PaneId) {
    for runtime in &mut state.runtimes {
        for workspace in &mut runtime.workspaces {
            for tab in &mut workspace.tabs {
                tab.panes.retain(|pane| &pane.id != target);
            }
        }
    }
}

/// 每個推送間隔把第一個 runtime、第一個 workspace、第一個 tab、第一個 pane 的 `agent_status`
/// 在 working／idle／blocked 之間輪替，`version` 遞增、`generated_at` 更新為現在時間，讓
/// 連著的瀏覽器（`/ws`）與下一次 `/api/state` 都看得到變化（design D14）。`vanish` 有設定時，
/// 經過指定時間後把該 pane 從投影拿掉一次（task 5.1）。
async fn push_loop(
    tx: watch::Sender<Arc<ProjectedState>>,
    every: Duration,
    vanish: Option<VanishConfig>,
    started: Instant,
) {
    const CYCLE: [AgentStatus; 3] = [
        AgentStatus::Working,
        AgentStatus::Idle,
        AgentStatus::Blocked,
    ];
    let mut index = 0usize;
    let mut vanished = false;
    // `tokio::time::interval` 的第一次 `tick()` 會立即完成（design 沒特別要求，但
    // fix round 1 finding：這會讓背景 task 一啟動就把 fixture 的 version/狀態改掉，
    // 使剛啟動的 `/api/state` 看不到 fixture 原始值）。改用 `interval_at` 把第一個
    // tick 排在「現在 + 一個間隔」，讓啟動當下到第一次真的輪替之間有完整的一個間隔空窗，
    // `/api/state` 才能如預期在這段時間內看到 fixture 原封不動的內容。
    let mut ticker = interval_at(Instant::now() + every, every);

    loop {
        ticker.tick().await;
        let mut next = (**tx.borrow()).clone();

        if let Some(vanish) = &vanish
            && !vanished
            && started.elapsed() >= vanish.after
        {
            remove_pane(&mut next, &vanish.pane);
            vanished = true;
        }

        index = (index + 1) % CYCLE.len();

        let updated = next
            .runtimes
            .first_mut()
            .and_then(|runtime| runtime.workspaces.first_mut())
            .and_then(|workspace| workspace.tabs.first_mut())
            .and_then(|tab| tab.panes.first_mut());

        let Some(pane) = updated else {
            // fixture 形狀跑掉（例如被改成沒有任何 pane）：沒東西可輪替，結束這個任務，
            // 但不影響 server 本身繼續服務目前這一份投影。
            break;
        };

        pane.agent_status = CYCLE[index];
        next.version += 1;
        next.generated_at = to_rfc3339(SystemTime::now());

        if tx.send(Arc::new(next)).is_err() {
            // 沒有任何 receiver 了（server 已經停了），沒必要再繼續輪替。
            break;
        }
    }
}

/// `SystemTime` → RFC 3339（UTC，秒精度，`YYYY-MM-DDTHH:MM:SSZ`），格式對齊
/// `cockpit-core::projection` 內部的同名私有函數。這裡自帶一份最小實作而不引入 `chrono`
/// ——`cockpit/Cargo.toml` 沒有這個依賴，本 task 不改 `Cargo.toml`。
fn to_rfc3339(time: SystemTime) -> String {
    let secs = time
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let days = secs.div_euclid(86_400);
    let secs_of_day = secs.rem_euclid(86_400);
    let hour = secs_of_day / 3600;
    let minute = (secs_of_day % 3600) / 60;
    let second = secs_of_day % 60;
    let (year, month, day) = civil_from_days(days);
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

/// Howard Hinnant 的 `civil_from_days` 演算法（公開演算法，非本專案原創）：把「自
/// 1970-01-01 起的天數」換算成公曆年月日。
fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if month <= 2 { y + 1 } else { y };
    (year, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `parse_output_mode`／`parse_output_modes` 覆蓋 brief 的六種模式與「規則不合法」路徑。

    #[test]
    fn parse_output_modes_none_returns_defaults() {
        let modes = parse_output_modes(None).expect("未設定應該成功回傳預設值");
        assert_eq!(modes.len(), 3);
        assert!(matches!(
            modes.get(&PaneId::new("wJ:p1")),
            Some(OutputMode::Growing {
                start_lines: 0,
                delay
            }) if delay.is_zero()
        ));
        assert!(matches!(
            modes.get(&PaneId::new("wJ:p3")),
            Some(OutputMode::Growing {
                start_lines: 300,
                delay
            }) if delay.is_zero()
        ));
        assert!(matches!(
            modes.get(&PaneId::new("wJ:p2")),
            Some(OutputMode::NotFound)
        ));
    }

    #[test]
    fn parse_output_modes_overrides_and_adds() {
        let modes = parse_output_modes(Some("wJ:p1=delay:3000;wJ:p3=fail:2;wJ:p2=html"))
            .expect("合法規則應該解析成功");
        assert!(matches!(
            modes.get(&PaneId::new("wJ:p1")),
            Some(OutputMode::Growing { start_lines: 0, delay })
            if *delay == Duration::from_millis(3000)
        ));
        match modes.get(&PaneId::new("wJ:p3")) {
            Some(OutputMode::FailThenRecover { remaining }) => {
                assert_eq!(remaining.load(Ordering::SeqCst), 2);
            }
            other => panic!("預期 FailThenRecover，實際：{other:?}"),
        }
        assert!(matches!(
            modes.get(&PaneId::new("wJ:p2")),
            Some(OutputMode::Html)
        ));
    }

    #[test]
    fn parse_output_modes_rejects_unknown_mode() {
        let error = parse_output_modes(Some("wJ:p1=bogus")).expect_err("不認識的模式應該回錯");
        assert!(format!("{error:#}").contains("不認識的模式"));
    }

    #[test]
    fn parse_output_modes_rejects_missing_equals() {
        let error = parse_output_modes(Some("wJ:p1")).expect_err("缺 `=` 應該回錯");
        assert!(format!("{error:#}").contains("缺"));
    }

    #[test]
    fn parse_vanish_pane_none_when_unset() {
        assert!(parse_vanish_pane(None).expect("None 應該成功").is_none());
    }

    #[test]
    fn parse_vanish_pane_parses_colon_pane_id() {
        let config = parse_vanish_pane(Some("wJ:p1=5000"))
            .expect("合法設定應該解析成功")
            .expect("應該回傳 Some");
        assert_eq!(config.pane, PaneId::new("wJ:p1"));
        assert_eq!(config.after, Duration::from_millis(5000));
    }

    #[test]
    fn parse_vanish_pane_rejects_non_integer_ms() {
        let error = parse_vanish_pane(Some("wJ:p1=soon")).expect_err("非整數毫秒應該回錯");
        assert!(format!("{error:#}").contains("不是整數"));
    }

    /// R14 補的測試（brief 逐字要求的四種）：`delay:<非數字>`、`fail:<非數字>`、
    /// `VANISH_PANE` 缺 `=`，以及「兩個環境變數指到 fixture 裡不存在的 pane id」。

    #[test]
    fn parse_output_mode_rejects_non_integer_delay() {
        let error = parse_output_modes(Some("wJ:p1=delay:abc")).expect_err("delay 非數字應該回錯");
        assert!(format!("{error:#}").contains("不是整數"));
    }

    #[test]
    fn parse_output_mode_rejects_non_integer_fail_count() {
        let error = parse_output_modes(Some("wJ:p1=fail:abc")).expect_err("fail 非數字應該回錯");
        assert!(format!("{error:#}").contains("不是整數"));
    }

    #[test]
    fn parse_vanish_pane_rejects_missing_equals() {
        let error = parse_vanish_pane(Some("wJ:p1")).expect_err("缺 `=` 應該回錯");
        assert!(format!("{error:#}").contains("缺"));
    }

    fn known_ids(ids: &[&str]) -> HashSet<PaneId> {
        ids.iter().map(|id| PaneId::new(*id)).collect()
    }

    #[test]
    fn validate_known_panes_rejects_unknown_output_mode_pane() {
        let known = known_ids(&["wJ:p1", "wJ:p2", "wJ:p3"]);
        let modes = parse_output_modes(Some("wJ:p9=ticker")).expect("合法規則應該解析成功");
        let error = validate_known_panes(&modes, &None, &known)
            .expect_err("COCKPIT_PREVIEW_OUTPUT_MODES 指到不存在的 pane id 應該回錯");
        let message = format!("{error:#}");
        assert!(
            message.contains("wJ:p9"),
            "訊息應該點名打錯的 pane id：{message}"
        );
        assert!(
            message.contains("COCKPIT_PREVIEW_OUTPUT_MODES"),
            "訊息應該點名是哪個環境變數：{message}"
        );
    }

    #[test]
    fn validate_known_panes_rejects_unknown_vanish_pane() {
        let known = known_ids(&["wJ:p1", "wJ:p2", "wJ:p3"]);
        let modes = default_output_modes();
        let vanish = parse_vanish_pane(Some("wJ:p9=5000")).expect("合法設定應該解析成功");
        let error = validate_known_panes(&modes, &vanish, &known)
            .expect_err("COCKPIT_PREVIEW_VANISH_PANE 指到不存在的 pane id 應該回錯");
        let message = format!("{error:#}");
        assert!(
            message.contains("wJ:p9"),
            "訊息應該點名打錯的 pane id：{message}"
        );
        assert!(
            message.contains("COCKPIT_PREVIEW_VANISH_PANE"),
            "訊息應該點名是哪個環境變數：{message}"
        );
    }

    #[test]
    fn validate_known_panes_accepts_defaults() {
        let known = known_ids(&["wJ:p1", "wJ:p2", "wJ:p3"]);
        let modes = default_output_modes();
        assert!(validate_known_panes(&modes, &None, &known).is_ok());
    }
}
