#![cfg(windows)]
//! Task 3.8 真機測試骨架（design D14；spec `cockpit-dashboard` Scenario A 的 Windows 側）。
//!
//! 全部 `#[ignore]`：需要 Windows 端 HERDR 正在跑（`herdr status server` 顯示
//! `status: running`），一般 `cargo test` 不會執行到。手動跑法：
//!
//! ```text
//! cargo test -p cockpit --test real_attach -- --ignored --test-threads=1
//! ```
//!
//! 全程唯讀：只用 `session.snapshot` 與 `events.subscribe`（驅動器自己發），外加執行
//! `herdr api snapshot` 讀一份對照。**絕不**執行 `herdr server stop`（會殺掉所有 pane，
//! 設計文件 §10.1 的真機測試禁令）。
//!
//! 起法與 `main` 相同：`config::load` 的零設定模式 → `app::build_components` → 真的綁
//! `127.0.0.1:0` 跑 `axum::serve`，再用最小的 HTTP/1.1 客戶端打 `/api/state`。

use std::net::SocketAddr;
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

use cockpit::app::{self, Components};
use cockpit::config::{self, Args, Config};
use serde_json::Value;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

/// 等待連線變成 `connected` 的上限。
const CONNECT_TIMEOUT: Duration = Duration::from_secs(20);
/// 輪詢 `/api/state` 的間隔。
const POLL_INTERVAL: Duration = Duration::from_millis(200);

#[tokio::test]
#[ignore = "需要 Windows 端 HERDR 正在跑（herdr status server）"]
async fn real_zero_config_connects_and_pane_count_matches_herdr_snapshot() {
    let config = zero_config();
    assert_eq!(
        config.runtimes.len(),
        1,
        "零設定模式應該只有一筆自動找本機的 runtime"
    );
    assert_eq!(
        config.runtimes[0].id, "local",
        "零設定的 runtime id 是 local"
    );

    let Components { stops, router, .. } =
        app::build_components(&config).expect("零設定應該組得起來");

    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind 127.0.0.1:0 不應該失敗");
    let addr = listener.local_addr().expect("local_addr 不應該失敗");
    let server = tokio::spawn(async move {
        axum::serve(listener, router).await.ok();
    });

    let state = wait_until_connected(addr).await;
    let cockpit_panes = count_panes(&state);
    let herdr_panes = herdr_snapshot_pane_count();

    assert_eq!(
        cockpit_panes, herdr_panes,
        "cockpit 的 /api/state 與 herdr api snapshot 的 pane 數應該一致；\
         如果測試期間剛好有人開關 pane，重跑一次。/api/state = {state}"
    );

    drop(stops);
    server.abort();
}

/// 零設定模式：在一個保證沒有 `cockpit.toml` 的暫存目錄上跑 `config::load`（`--config`
/// 不給），走的就是 `main` 的第三順位來源。
fn zero_config() -> Config {
    let dir: PathBuf = std::env::temp_dir().join(format!(
        "cockpit-real-attach-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default()
    ));
    std::fs::create_dir_all(&dir).expect("暫存目錄應該建得起來");
    let config = config::load(&Args::default(), &dir, &|key| std::env::var(key).ok())
        .expect("零設定模式不應該失敗");
    std::fs::remove_dir_all(&dir).ok();
    config
}

/// 輪詢 `/api/state` 直到 `runtimes[0].connection.state == "connected"`；逾時就 panic 並
/// 附上最後看到的內容（最可能的原因是 Windows 端 HERDR 沒在跑）。
async fn wait_until_connected(addr: SocketAddr) -> Value {
    let deadline = tokio::time::Instant::now() + CONNECT_TIMEOUT;
    loop {
        let last = get_api_state(addr).await;
        if last["runtimes"][0]["connection"]["state"] == "connected" {
            return last;
        }
        if tokio::time::Instant::now() >= deadline {
            panic!(
                "等了 {} 秒 runtime 還沒 connected：先確認 `herdr status server` 顯示 \
                 running。最後一次 /api/state = {last}",
                CONNECT_TIMEOUT.as_secs()
            );
        }
        tokio::time::sleep(POLL_INTERVAL).await;
    }
}

/// 最小的 HTTP/1.1 GET：`Connection: close` 讓 server 回完就關，`read_to_end` 自然結束。
async fn get_api_state(addr: SocketAddr) -> Value {
    let mut stream = TcpStream::connect(addr)
        .await
        .expect("連得上自己剛開的 server");
    let request = format!("GET /api/state HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n");
    stream
        .write_all(request.as_bytes())
        .await
        .expect("送出請求不應該失敗");
    let mut raw = Vec::new();
    stream
        .read_to_end(&mut raw)
        .await
        .expect("讀取回應不應該失敗");
    let text = String::from_utf8(raw).expect("回應應該是 UTF-8");
    let (head, body) = text
        .split_once("\r\n\r\n")
        .expect("回應應該有 header 與 body 的分隔");
    assert!(
        head.starts_with("HTTP/1.1 200"),
        "/api/state 應該回 200，實際是：{head}"
    );
    serde_json::from_str(body).expect("/api/state 應該回合法 JSON")
}

/// `/api/state` 投影裡的 pane 總數（所有 workspace 的所有 tab）。
fn count_panes(state: &Value) -> usize {
    state["runtimes"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default()
        .iter()
        .flat_map(|runtime| {
            runtime["workspaces"]
                .as_array()
                .map(Vec::as_slice)
                .unwrap_or_default()
        })
        .flat_map(|workspace| {
            workspace["tabs"]
                .as_array()
                .map(Vec::as_slice)
                .unwrap_or_default()
        })
        .map(|tab| tab["panes"].as_array().map(Vec::len).unwrap_or_default())
        .sum()
}

/// 跑 `herdr api snapshot`（查證 2026-09-15：`herdr 0.9.0-preview`，這個子命令沒有任何
/// 旗標，輸出本來就是一行 JSON `{"id":..., "result":{"snapshot":{...}}}`），回傳
/// `result.snapshot.panes` 的筆數。
fn herdr_snapshot_pane_count() -> usize {
    let output = Command::new("herdr")
        .args(["api", "snapshot"])
        .output()
        .unwrap_or_else(|err| panic!("執行 `herdr api snapshot` 失敗（herdr 不在 PATH？）：{err}"));
    assert!(
        output.status.success(),
        "`herdr api snapshot` 非零結束（HERDR 沒在跑？先跑 `herdr status server`）：{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("herdr 的輸出應該是 UTF-8");
    let parsed: Value =
        serde_json::from_str(stdout.trim()).expect("`herdr api snapshot` 應該輸出一行 JSON");
    let panes = parsed
        .pointer("/result/snapshot/panes")
        .or_else(|| parsed.pointer("/snapshot/panes"))
        .unwrap_or_else(|| panic!("`herdr api snapshot` 的輸出找不到 snapshot.panes：{parsed}"));
    panes
        .as_array()
        .unwrap_or_else(|| panic!("snapshot.panes 應該是陣列：{panes}"))
        .len()
}
