//! `cockpit` example：用固定 fixture 起一個真的 dashboard server，之後每 2 秒輪替一個
//! pane 的 `agent_status`，供瀏覽器工具與人眼檢查（design D14；Task 3.5）。不需要 HERDR，
//! 也不需要真正的 runtime。
//!
//! ```text
//! cargo run -p cockpit --example ui_preview
//! ```
//!
//! 預設監聽 `127.0.0.1:7770`；可用環境變數 `COCKPIT_PREVIEW_LISTEN` 覆寫（例如
//! `COCKPIT_PREVIEW_LISTEN=127.0.0.1:8080`）。Ctrl-C 結束。

use std::env;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

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

    let app = router(AppState { state: rx });

    println!("ui_preview 監聽 http://{actual_addr}（Ctrl-C 結束）");
    println!("試試：curl http://{actual_addr}/api/state");

    let cycle_task = tokio::spawn(cycle_first_pane_status(tx));

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

/// 每 2 秒把第一個 runtime、第一個 workspace、第一個 tab、第一個 pane 的 `agent_status`
/// 在 working／idle／blocked 之間輪替，`version` 遞增、`generated_at` 更新為現在時間，讓
/// 連著的瀏覽器（`/ws`）與下一次 `/api/state` 都看得到變化（design D14）。
async fn cycle_first_pane_status(tx: watch::Sender<Arc<ProjectedState>>) {
    const CYCLE: [AgentStatus; 3] = [
        AgentStatus::Working,
        AgentStatus::Idle,
        AgentStatus::Blocked,
    ];
    let mut index = 0usize;
    // `tokio::time::interval` 的第一次 `tick()` 會立即完成（design 沒特別要求，但
    // fix round 1 finding：這會讓背景 task 一啟動就把 fixture 的 version/狀態改掉，
    // 使剛啟動的 `/api/state` 看不到 fixture 原始值）。改用 `interval_at` 把第一個
    // tick 排在「現在 + 2 秒」，讓啟動當下到第一次真的輪替之間有完整的 2 秒空窗，
    // `/api/state` 才能如預期在這段時間內看到 fixture 原封不動的內容。
    let mut ticker = interval_at(
        Instant::now() + Duration::from_secs(2),
        Duration::from_secs(2),
    );

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
