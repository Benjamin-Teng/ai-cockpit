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

use std::env;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use anyhow::Context;
use axum::Router;
use axum::body::Bytes;
use axum::extract::State;
use axum::http::{Method, StatusCode, Uri, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{post, put};
use cockpit::http::{AppState, router};
use cockpit_core::{AgentStatus, ProjectedState};
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

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let initial: ProjectedState =
        serde_json::from_str(FIXTURE).expect("fixture 應該能反序列化成 ProjectedState");

    let (tx, rx) = watch::channel(Arc::new(initial));

    let listen_addr =
        env::var("COCKPIT_PREVIEW_LISTEN").unwrap_or_else(|_| DEFAULT_LISTEN.to_string());
    let listener = TcpListener::bind(&listen_addr).await?;
    let actual_addr = listener.local_addr()?;

    let push_every = push_interval()?;
    let write_rules = Arc::new(write_rules()?);

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
        .fallback_service(router(AppState::new(rx)))
        .with_state(write_rules);

    println!("ui_preview 監聽 http://{actual_addr}（Ctrl-C 結束）");
    println!(
        "推送間隔 {} ms；寫入請求只記錄、回 204",
        push_every.as_millis()
    );
    println!("試試：curl http://{actual_addr}/api/state");

    let cycle_task = tokio::spawn(cycle_first_pane_status(tx, push_every));

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

/// 每個推送間隔把第一個 runtime、第一個 workspace、第一個 tab、第一個 pane 的 `agent_status`
/// 在 working／idle／blocked 之間輪替，`version` 遞增、`generated_at` 更新為現在時間，讓
/// 連著的瀏覽器（`/ws`）與下一次 `/api/state` 都看得到變化（design D14）。
async fn cycle_first_pane_status(tx: watch::Sender<Arc<ProjectedState>>, every: Duration) {
    const CYCLE: [AgentStatus; 3] = [
        AgentStatus::Working,
        AgentStatus::Idle,
        AgentStatus::Blocked,
    ];
    let mut index = 0usize;
    // `tokio::time::interval` 的第一次 `tick()` 會立即完成（design 沒特別要求，但
    // fix round 1 finding：這會讓背景 task 一啟動就把 fixture 的 version/狀態改掉，
    // 使剛啟動的 `/api/state` 看不到 fixture 原始值）。改用 `interval_at` 把第一個
    // tick 排在「現在 + 一個間隔」，讓啟動當下到第一次真的輪替之間有完整的一個間隔空窗，
    // `/api/state` 才能如預期在這段時間內看到 fixture 原封不動的內容。
    let mut ticker = interval_at(Instant::now() + every, every);

    loop {
        ticker.tick().await;
        let mut next = (**tx.borrow()).clone();
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
