#![cfg(windows)]

//! 真機測試（task 5.1；design D1、D3、D4、D6、D9–D12；spec `herdr-request`、
//! `herdr-event-subscription`）：全部改寫成用 `herdr_client::client::Client`，不再手動組
//! named pipe／`wsl.exe -e nc` 的逐行讀寫（那是第 1 組 spike 時代、`Client` 還不存在時的
//! 寫法）。全部 `#[ignore]`，需要真機 HERDR 在跑才會執行；全程只送 `session.snapshot`／
//! `events.subscribe`，不使用 `herdr server stop` 或任何會終止 server 的操作。
//!
//! 舊檔案 `real_herdr_wsl.rs`（spike 2）、`real_herdr_subscriptions.rs`（spike 3）的測試已
//! 改寫、併入本檔後刪除；歷史紀錄見
//! `docs/research/2026-09-13/change-1a-spikes.md`（各節已加註「已於 task 5.1 改寫」）。
//!
//! 環境變數（省略則用下列預設值；詳見 `herdr-client/README.md`「真機測試」節）：
//!
//! | 環境變數 | 用途 | 省略時的預設 |
//! |---|---|---|
//! | `HERDR_CLIENT_TEST_WIN_SOCKET` | Windows 端 HERDR API socket 路徑 | `default_socket_path_from_env()` |
//! | `HERDR_CLIENT_TEST_WSL_DISTRO` | WSL 發行版名稱 | `Ubuntu-24.04` |
//! | `HERDR_CLIENT_TEST_WSL_SOCKET` | WSL 端 HERDR API unix socket 路徑 | 由 WSL 的 `$HOME` 推得：`<HOME>/.config/herdr/herdr.sock` |
//! | `HERDR_CLIENT_TEST_ALLOW_WSL_WRITES` | 是否允許在 WSL 端 workspace `wD` 建立/操作/關閉測試 tab | 未設定＝不允許（唯讀） |
//!
//! **預設唯讀、寫入需明確 opt-in**：`real_wsl_lifecycle_stream_via_child_stdio`（建立 tab 觸發
//! 事件那一段）與 `real_wsl_reopen_overlap_loses_nothing`（建立 tab、送 agent 狀態變化）需要
//! `HERDR_CLIENT_TEST_ALLOW_WSL_WRITES=1` 才會實際寫入；未設定時印出說明後直接算通過並跳過
//! （寫入動作只對 `HERDR_CLIENT_TEST_WSL_SOCKET` 指定的 WSL 端測試 server 執行，且只建立/清理
//! 自己的 tab，不動既有 tab／pane，見 `TabGuard`）。其餘測試全程唯讀，不受這個開關影響。

use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use herdr_client::client::{
    Client, EventStream, IncomingEvent, RequestError, SessionSnapshotRequest,
};
use herdr_client::connector::{
    ChildStdioConnector, Connector, NamedPipeConnector, default_socket_path_from_env,
};
use herdr_client::types::{AgentStatus, EventKind, Subscription, SubscriptionEventKind};

const WSL_WORKSPACE: &str = "wD";

// ---------------------------------------------------------------------------
// 環境變數解析與 client／connector 建構
// ---------------------------------------------------------------------------

fn win_socket_path() -> std::path::PathBuf {
    std::env::var("HERDR_CLIENT_TEST_WIN_SOCKET")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| default_socket_path_from_env())
}

fn wsl_distro() -> String {
    std::env::var("HERDR_CLIENT_TEST_WSL_DISTRO").unwrap_or_else(|_| "Ubuntu-24.04".to_string())
}

/// WSL 端 socket 路徑；整個測試 binary 只解析一次（`OnceLock` 快取，成功或失敗都記住），
/// 避免每次呼叫都同步 spawn 一次 `wsl.exe`。不會 panic：取不到回 `None`，供 `Drop` 路徑使用
/// （測試正在 unwinding 時再 panic 會讓整個測試 binary abort）。
fn wsl_socket_opt() -> Option<&'static str> {
    static WSL_SOCKET: OnceLock<Option<String>> = OnceLock::new();
    WSL_SOCKET
        .get_or_init(|| {
            if let Ok(socket) = std::env::var("HERDR_CLIENT_TEST_WSL_SOCKET") {
                return Some(socket);
            }
            // 沒指定就問 WSL 的 $HOME（--exec 不經 shell），預設 socket 在其 .config/herdr 下。
            let output = std::process::Command::new("wsl.exe")
                .args(["-d", &wsl_distro(), "--exec", "printenv", "HOME"])
                .output();
            output
                .ok()
                .filter(|output| output.status.success())
                .and_then(|output| String::from_utf8(output.stdout).ok())
                .map(|stdout| stdout.trim().to_string())
                .filter(|home| !home.is_empty())
                .map(|home| format!("{home}/.config/herdr/herdr.sock"))
        })
        .as_deref()
}

/// 測試主體用：取不到就 panic 並給出可行動的訊息。`Drop` 路徑一律改用 [`wsl_socket_opt`]。
fn wsl_socket() -> String {
    wsl_socket_opt()
        .expect(
            "無法取得 WSL 的 $HOME；請設定 HERDR_CLIENT_TEST_WSL_SOCKET 指定 WSL 端 socket 路徑",
        )
        .to_string()
}

/// 是否明確 opt-in 允許本檔案內會操作 WSL 端 HERDR 測試 tab 的測試執行；預設不允許（唯讀）。
fn allow_wsl_writes() -> bool {
    std::env::var("HERDR_CLIENT_TEST_ALLOW_WSL_WRITES").as_deref() == Ok("1")
}

fn win_client() -> Client {
    Client::new(Arc::new(NamedPipeConnector::new(win_socket_path())))
}

/// `wsl.exe -d <distro> -e nc -U <socket>`（design D1、D3：子程序 stdio 橋接），子程序參數
/// 需要 owned `String`（`distro`／`socket` 是執行期組出來的值，不是 `&'static str`）。
fn wsl_bridge_connector(distro: &str, socket: &str) -> ChildStdioConnector {
    ChildStdioConnector::new(
        "wsl.exe",
        vec![
            "-d".to_string(),
            distro.to_string(),
            "-e".to_string(),
            "nc".to_string(),
            "-U".to_string(),
            socket.to_string(),
        ],
    )
}

fn wsl_client() -> Client {
    Client::new(Arc::new(wsl_bridge_connector(&wsl_distro(), &wsl_socket())))
}

/// 對 WSL 端送一次「非 observer 子集」的 JSON-RPC 呼叫（`tab.create`／`tab.close`／
/// `pane.report_agent`／`pane.clear_agent_authority`／`tab.list`）：`Client::request` 的
/// `Request` trait 是 sealed（design D10「只提供 observer 子集」），這幾個方法本來就不該、
/// 也不能透過它送出。這裡直接重用 crate 自己的 `ChildStdioConnector` 開一條連線、手動送一行
/// request、讀一行回應——只在測試裡準備／清理環境，不是新增一條給 crate 外部使用的旁路。
async fn wsl_raw_call_to(
    distro: &str,
    socket: &str,
    id: &str,
    method: &str,
    params: Value,
) -> Value {
    let connector = wsl_bridge_connector(distro, socket);
    let mut stream = connector
        .connect()
        .await
        .unwrap_or_else(|e| panic!("開 WSL 連線失敗（{method}）: {e}"));
    let line = serde_json::to_string(&json!({"id": id, "method": method, "params": params}))
        .expect("Value 序列化不會失敗");
    stream
        .send_line(&line)
        .await
        .unwrap_or_else(|e| panic!("送出 {method} 失敗: {e}"));
    let response_line = stream
        .recv_line()
        .await
        .unwrap_or_else(|e| panic!("讀取 {method} 回應失敗: {e}"))
        .unwrap_or_else(|| panic!("連線在收到 {method} 回應前就 EOF"));
    serde_json::from_str(&response_line)
        .unwrap_or_else(|e| panic!("{method} 回應不是合法 JSON: {e}\n{response_line}"))
}

async fn wsl_raw_call(id: &str, method: &str, params: Value) -> Value {
    wsl_raw_call_to(&wsl_distro(), &wsl_socket(), id, method, params).await
}

/// 讀取 workspace `wD` 目前所有 tab 的 id 集合（`tab.list`，唯讀）。
async fn wsl_list_tab_ids() -> HashSet<String> {
    let resp = wsl_raw_call("tl", "tab.list", json!({"workspace_id": WSL_WORKSPACE})).await;
    resp["result"]["tabs"]
        .as_array()
        .expect("tab.list 回應應含 result.tabs")
        .iter()
        .filter_map(|t| t["tab_id"].as_str().map(str::to_string))
        .collect()
}

async fn wsl_report_agent(pane_id: &str, state: &str) {
    let resp = wsl_raw_call(
        "rep",
        "pane.report_agent",
        json!({"pane_id": pane_id, "source": "real5_1", "agent": "claude", "state": state}),
    )
    .await;
    assert_eq!(
        resp["result"]["type"].as_str(),
        Some("ok"),
        "pane.report_agent({state}) 應回 ok：{resp:?}"
    );
}

async fn wsl_release_agent(pane_id: &str) {
    let resp = wsl_raw_call(
        "rel",
        "pane.clear_agent_authority",
        json!({"pane_id": pane_id, "source": "real5_1"}),
    )
    .await;
    assert_eq!(
        resp["result"]["type"].as_str(),
        Some("ok"),
        "pane.clear_agent_authority 應回 ok：{resp:?}"
    );
}

async fn wsl_close_tab(tab_id: &str) {
    let resp = wsl_raw_call("tclose", "tab.close", json!({"tab_id": tab_id})).await;
    assert_eq!(
        resp["result"]["type"].as_str(),
        Some("ok"),
        "tab.close 應回 ok：{resp:?}"
    );
}

// ---------------------------------------------------------------------------
// TabGuard：建立前先記下 tab id 集合、建立 guard，才呼叫 `tab.create`（沿用 spike 2／3 的
// 教訓，見 `docs/research/2026-09-13/change-1a-spikes.md`）——即使 `tab.create` 已在 server
// 端成功但呼叫端解析輸出時 panic，guard 也已存在，Drop 時能靠「建立前的 id 集合」與 label
// 對 `tab.list` 做 diff 找到並清理殘留 tab，不需要事先知道確切 tab id。
// ---------------------------------------------------------------------------

struct TabGuard {
    label: String,
    before_ids: HashSet<String>,
    disarmed: bool,
}

impl TabGuard {
    fn new(label: String, before_ids: HashSet<String>) -> Self {
        Self {
            label,
            before_ids,
            disarmed: false,
        }
    }

    /// 正常路徑已經呼叫 `tab.close` 成功後呼叫，避免 Drop 重複清理。
    fn disarm(&mut self) {
        self.disarmed = true;
    }
}

impl Drop for TabGuard {
    /// Drop 不能 `.await`、也不能在既有 tokio runtime 內再 `block_on`，所以另開一條 std
    /// thread 跑一個獨立的迷你 runtime（沿用 spike 3 的做法）。
    fn drop(&mut self) {
        if self.disarmed {
            return;
        }
        eprintln!(
            "TabGuard: 測試提前結束，依 label={} 與建立前的 tab id 集合 diff 清理殘留 tab/pane",
            self.label
        );
        let label = self.label.clone();
        let before_ids = self.before_ids.clone();
        let distro = wsl_distro();
        // Drop 可能發生在 unwinding 中，這裡不可 panic：取不到 socket 就略過清理。
        let Some(socket) = wsl_socket_opt().map(str::to_string) else {
            eprintln!("TabGuard: 無法取得 WSL socket 路徑，略過清理");
            return;
        };
        let joined = std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("建立清理用 runtime 失敗");
            rt.block_on(async {
                let tabs_resp = wsl_raw_call_to(
                    &distro,
                    &socket,
                    "guard-list",
                    "tab.list",
                    json!({"workspace_id": WSL_WORKSPACE}),
                )
                .await;
                let Some(tabs) = tabs_resp["result"]["tabs"].as_array() else {
                    eprintln!("TabGuard: tab.list 回應缺少 result.tabs，無法清理");
                    return;
                };
                for tab in tabs {
                    let (Some(tab_id), Some(tab_label)) =
                        (tab["tab_id"].as_str(), tab["label"].as_str())
                    else {
                        continue;
                    };
                    if tab_label != label || before_ids.contains(tab_id) {
                        continue;
                    }
                    eprintln!(
                        "TabGuard: 找到殘留 tab {tab_id}（label={tab_label}），清理其 pane 並關閉"
                    );
                    let snap = wsl_raw_call_to(
                        &distro,
                        &socket,
                        "guard-snap",
                        "session.snapshot",
                        json!({}),
                    )
                    .await;
                    if let Some(panes) = snap["result"]["snapshot"]["panes"].as_array() {
                        for pane in panes {
                            if pane["tab_id"].as_str() != Some(tab_id) {
                                continue;
                            }
                            if let Some(pane_id) = pane["pane_id"].as_str() {
                                let _ = wsl_raw_call_to(
                                    &distro,
                                    &socket,
                                    "guard-rel",
                                    "pane.clear_agent_authority",
                                    json!({"pane_id": pane_id, "source": "real5_1"}),
                                )
                                .await;
                            }
                        }
                    }
                    let _ = wsl_raw_call_to(
                        &distro,
                        &socket,
                        "guard-close",
                        "tab.close",
                        json!({"tab_id": tab_id}),
                    )
                    .await;
                }
            });
        })
        .join();
        if joined.is_err() {
            eprintln!("TabGuard: 清理 thread panic");
        }
    }
}

// ---------------------------------------------------------------------------
// 事件收集 helper
// ---------------------------------------------------------------------------

/// 背景收集一條 `EventStream` 收到的所有事件，連同相對測試起點的時間戳；`stop()` 前一直收，
/// 讓多條訂閱可以獨立控制何時停止觀察（例如 reopen overlap 測試裡 S1 比 S2 早關）。crate 的
/// dev-dependency 沒開 tokio `sync` feature 給非 test-support 的正常建置用，沿用 spike 3 的
/// 做法：停止訊號用 `AtomicBool` 加每輪 100ms 的 `tokio::time::timeout` 輪詢，不用
/// `oneshot`／`select!`；共享緩衝用 `std::sync::Mutex`（臨界區只有一次 `push`，不跨 `.await`）。
struct EventCollector {
    events: Arc<Mutex<Vec<(Duration, IncomingEvent)>>>,
    stop: Arc<AtomicBool>,
    handle: tokio::task::JoinHandle<()>,
}

impl EventCollector {
    fn spawn(mut stream: EventStream, start: Instant) -> Self {
        let events = Arc::new(Mutex::new(Vec::new()));
        let events_task = Arc::clone(&events);
        let stop = Arc::new(AtomicBool::new(false));
        let stop_task = Arc::clone(&stop);
        let handle = tokio::spawn(async move {
            loop {
                if stop_task.load(Ordering::Relaxed) {
                    break;
                }
                match tokio::time::timeout(Duration::from_millis(100), stream.next()).await {
                    Ok(Some(Ok(event))) => {
                        events_task
                            .lock()
                            .expect("mutex 中毒")
                            .push((start.elapsed(), event));
                    }
                    Ok(Some(Err(e))) => {
                        eprintln!("EventCollector: 串流回錯誤，停止收集: {e}");
                        break;
                    }
                    Ok(None) => break,
                    Err(_elapsed) => continue,
                }
            }
        });
        Self {
            events,
            stop,
            handle,
        }
    }

    async fn stop(self) -> Vec<(Duration, IncomingEvent)> {
        self.stop.store(true, Ordering::Relaxed);
        let _ = self.handle.await;
        Arc::try_unwrap(self.events)
            .unwrap_or_else(|arc| {
                panic!(
                    "EventCollector 收工時仍有其他 Arc 持有者，共 {} 個",
                    Arc::strong_count(&arc)
                )
            })
            .into_inner()
            .expect("mutex 中毒")
    }
}

/// 從訂閱連線持續讀事件，直到某個生命週期事件滿足 `matches`，回傳它的 `(EventKind, data)`。
/// `budget` 內沒等到就 panic（逾時）。跟 `EventCollector` 不同：這個是「等到特定事件就提早
/// 結束」，不需要背景 task 與停止旗標。
async fn wait_for_lifecycle_event(
    stream: &mut EventStream,
    budget: Duration,
    matches: impl Fn(EventKind, &Value) -> bool,
) -> (EventKind, Value) {
    tokio::time::timeout(budget, async {
        loop {
            match stream.next().await {
                Some(Ok(IncomingEvent::Lifecycle(kind, data))) if matches(kind, &data) => {
                    return (kind, data);
                }
                Some(Ok(_other)) => continue,
                Some(Err(e)) => panic!("讀取事件失敗: {e}"),
                None => panic!("訂閱連線在等待事件時提前 EOF"),
            }
        }
    })
    .await
    .expect("等待事件逾時")
}

/// 從一批已收集的事件裡取出某個 pane 的 `pane.agent_status_changed` 狀態序列（依收到順序）。
fn extract_status_sequence(
    events: &[(Duration, IncomingEvent)],
    pane_id: &str,
) -> Vec<AgentStatus> {
    events
        .iter()
        .filter_map(|(_, event)| match event {
            IncomingEvent::PerPane(SubscriptionEventKind::PaneAgentStatusChanged, data)
                if data.get("pane_id").and_then(Value::as_str) == Some(pane_id) =>
            {
                data.get("agent_status")
                    .cloned()
                    .and_then(|v| serde_json::from_value::<AgentStatus>(v).ok())
            }
            _ => None,
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Windows 端測試（`NamedPipeConnector`）
// ---------------------------------------------------------------------------

/// 對應 brief：`NamedPipeConnector` → `Client::request`（改寫自 spike 1
/// `spike1_named_pipe_session_snapshot`）。
#[tokio::test]
#[ignore = "需要 Windows 端 HERDR server 在跑；全程唯讀（design D11）"]
async fn real_win_snapshot() {
    let client = win_client();
    let result = client
        .request(SessionSnapshotRequest {})
        .await
        .expect("session.snapshot 應成功");

    eprintln!("version: {}", result.snapshot.version);
    eprintln!("protocol: {}", result.snapshot.protocol);
    eprintln!("workspaces: {}", result.snapshot.workspaces.len());
    eprintln!("tabs: {}", result.snapshot.tabs.len());
    eprintln!("panes: {}", result.snapshot.panes.len());
    eprintln!("agents: {}", result.snapshot.agents.len());
    assert!(
        result.snapshot.protocol >= 20,
        "Windows 端 HERDR 0.9.0-preview 的 protocol 應 >= 20，實際: {}",
        result.snapshot.protocol
    );
}

/// 對應 brief：L（24 種）與 S（對 snapshot 全部 pane）各收 5 秒，只讀（改寫自 spike 3
/// `spike3_windows_readonly_checks` 的 (a) 段）。
#[tokio::test]
#[ignore = "需要 Windows 端 HERDR server 在跑；全程唯讀，不觸發任何事件（design D11）"]
async fn real_win_lifecycle_and_status_streams() {
    let client = win_client();
    let snapshot = client
        .request(SessionSnapshotRequest {})
        .await
        .expect("session.snapshot 應成功");
    let pane_ids: Vec<String> = snapshot
        .snapshot
        .panes
        .iter()
        .map(|p| p.pane_id.clone())
        .collect();
    eprintln!("Windows 端目前 pane 數: {}", pane_ids.len());
    assert!(!pane_ids.is_empty(), "唯讀檢查需要至少一個既有 pane");

    let start = Instant::now();
    let l_stream = client
        .subscribe(&Subscription::all_lifecycle())
        .await
        .expect("L 訂閱應成功");
    let l_collector = EventCollector::spawn(l_stream, start);

    let status_subs: Vec<Subscription> = pane_ids
        .iter()
        .map(|id| Subscription::PaneAgentStatusChanged {
            pane_id: id.clone(),
        })
        .collect();
    let s_stream = client.subscribe(&status_subs).await.expect("S 訂閱應成功");
    let s_collector = EventCollector::spawn(s_stream, start);

    tokio::time::sleep(Duration::from_secs(5)).await;

    let l_events = l_collector.stop().await;
    let s_events = s_collector.stop().await;

    eprintln!("5 秒內 L 收到 {} 筆", l_events.len());
    for (elapsed, event) in &l_events {
        eprintln!("  [{elapsed:?}] L: {event:?}");
    }
    eprintln!("5 秒內 S 收到 {} 筆", s_events.len());
    for (elapsed, event) in &s_events {
        eprintln!("  [{elapsed:?}] S: {event:?}");
    }
}

/// 對應 spike 3 保留的價值（改名自 `spike3_windows_readonly_checks` 的 (b) 段）：不存在的
/// pane 讓整個 `events.subscribe` request 回 `error`，`Client::subscribe` 對映成
/// `RequestError::Remote`（design D12）。
#[tokio::test]
#[ignore = "需要 Windows 端 HERDR server 在跑；全程唯讀（design D11）"]
async fn real_win_missing_pane_is_remote() {
    let client = win_client();
    let snapshot = client
        .request(SessionSnapshotRequest {})
        .await
        .expect("session.snapshot 應成功");
    let real_pane = snapshot
        .snapshot
        .panes
        .first()
        .expect("唯讀檢查需要至少一個既有 pane")
        .pane_id
        .clone();

    let subs = vec![
        Subscription::PaneAgentStatusChanged { pane_id: real_pane },
        Subscription::PaneAgentStatusChanged {
            pane_id: "wZ:p999".to_string(),
        },
    ];
    let err = client
        .subscribe(&subs)
        .await
        .expect_err("不存在 pane 應讓整個 request 回 error");
    eprintln!("錯誤: {err}");
    assert!(
        matches!(err, RequestError::Remote { .. }),
        "應為 RequestError::Remote，實際: {err:?}"
    );
}

// ---------------------------------------------------------------------------
// WSL 端測試（`ChildStdioConnector` 橋接 `wsl.exe -e nc -U`）
// ---------------------------------------------------------------------------

/// 對應 brief：`ChildStdioConnector`（`wsl.exe -d <distro> -e nc -U <socket>`）→
/// `Client::request`（改寫自 spike 2 `spike2_wsl_nc_snapshot`）。
#[tokio::test]
#[ignore = "需要 WSL 端 HERDR headless server 正在跑（design D11）"]
async fn real_wsl_snapshot_via_child_stdio() {
    let client = wsl_client();
    let result = client
        .request(SessionSnapshotRequest {})
        .await
        .expect("WSL session.snapshot 應成功");

    eprintln!("protocol: {}", result.snapshot.protocol);
    eprintln!("panes: {}", result.snapshot.panes.len());
    assert_eq!(
        result.snapshot.protocol, 20,
        "WSL 端 HERDR 0.8.2 的 protocol 應為 20"
    );
}

/// 對應 brief：同法（`ChildStdioConnector`）`subscribe` L（24 種）；需要 opt-in 才建立測試
/// tab 觸發 `tab_created`／`tab_closed` 事件驗證確實收得到，無 opt-in 則只驗訂閱建立成功
/// （改寫自 spike 2 `spike2_wsl_nc_subscribe_latency`，去掉延遲量測——那是 spike 階段的探索
/// 問題，`Client::subscribe` 本身不做逾時／延遲保證，design D10 Non-Goals）。
#[tokio::test]
#[ignore = "需要 WSL 端 HERDR headless server 正在跑；寫入部分需 opt-in（design D11）"]
async fn real_wsl_lifecycle_stream_via_child_stdio() {
    let client = wsl_client();
    let mut stream = client
        .subscribe(&Subscription::all_lifecycle())
        .await
        .expect("L 訂閱應成功（ChildStdioConnector 橋接）");
    eprintln!("L 訂閱建立成功");

    if !allow_wsl_writes() {
        eprintln!(
            "略過建立測試 tab 觸發事件：需設定環境變數 HERDR_CLIENT_TEST_ALLOW_WSL_WRITES=1 \
             才會在 workspace wD 建立/關閉一個測試 tab 來驗證事件確實送達。"
        );
        return;
    }

    let before_ids = wsl_list_tab_ids().await;
    let mut guard = TabGuard::new("real5_1".to_string(), before_ids);

    let create_resp = wsl_raw_call(
        "tc",
        "tab.create",
        json!({"workspace_id": WSL_WORKSPACE, "label": "real5_1", "focus": false}),
    )
    .await;
    let tab_id = create_resp["result"]["tab"]["tab_id"]
        .as_str()
        .unwrap_or_else(|| panic!("tab.create 回應應含 result.tab.tab_id：{create_resp:?}"))
        .to_string();
    eprintln!("建立測試 tab: {tab_id}");

    let (kind, data) =
        wait_for_lifecycle_event(&mut stream, Duration::from_secs(5), |kind, data| {
            kind == EventKind::TabCreated
                && data
                    .get("tab")
                    .and_then(|t| t.get("tab_id"))
                    .and_then(Value::as_str)
                    == Some(tab_id.as_str())
        })
        .await;
    eprintln!("收到 {}: {data}", kind.as_str());

    wsl_close_tab(&tab_id).await;
    guard.disarm();

    let (kind, data) =
        wait_for_lifecycle_event(&mut stream, Duration::from_secs(5), |kind, data| {
            kind == EventKind::TabClosed
                && data.get("tab_id").and_then(Value::as_str) == Some(&tab_id)
        })
        .await;
    eprintln!("收到 {}: {data}", kind.as_str());
}

/// 對應 spike 3 保留的價值（改名自 `spike3_wsl_missing_pane_fails_whole_request`）：不存在的
/// pane 讓整個 request 回 error。改成先用 `session.snapshot` 動態查一個既有 pane（不再寫死
/// `"wD:p1"`），全程唯讀，不需要 opt-in。
#[tokio::test]
#[ignore = "需要 WSL 端 HERDR headless server 正在跑；全程唯讀（design D11）"]
async fn real_wsl_missing_pane_fails_whole_request() {
    let client = wsl_client();
    let snapshot = client
        .request(SessionSnapshotRequest {})
        .await
        .expect("session.snapshot 應成功");
    let real_pane = snapshot
        .snapshot
        .panes
        .iter()
        .find(|p| p.workspace_id == WSL_WORKSPACE)
        .unwrap_or_else(|| panic!("workspace {WSL_WORKSPACE} 應至少有一個既有 pane"))
        .pane_id
        .clone();

    let subs = vec![
        Subscription::PaneAgentStatusChanged { pane_id: real_pane },
        Subscription::PaneAgentStatusChanged {
            pane_id: "wZ:p999".to_string(),
        },
    ];
    let err = client
        .subscribe(&subs)
        .await
        .expect_err("不存在 pane 應讓整個 request 回 error");
    eprintln!("錯誤: {err}");
    assert!(
        matches!(err, RequestError::Remote { .. }),
        "應為 RequestError::Remote，實際: {err:?}"
    );
}

/// 對應 spike 3 保留的價值（改名自 `spike3_wsl_reopen_overlap_loses_nothing`）：重開 S 訂閱的
/// 重疊期間不會丟事件——S1、S2 同時開著時觸發一次狀態變化兩邊都該收到，關掉 S1 後 S2 仍完整
/// 收到後續序列。需要在 `wD` 建立/操作/關閉一個測試 tab，需 opt-in。
#[tokio::test]
#[ignore = "需要 WSL 端 HERDR headless server 正在跑，會在 wD 建立/刪除自己的 tab；需 opt-in（design D11）"]
async fn real_wsl_reopen_overlap_loses_nothing() {
    if !allow_wsl_writes() {
        eprintln!(
            "略過：此測試會對 WSL 端 HERDR 寫入（建立/操作/關閉一個測試 tab）。\
             需設定環境變數 HERDR_CLIENT_TEST_ALLOW_WSL_WRITES=1 才會執行。"
        );
        return;
    }

    let start = Instant::now();
    let before_ids = wsl_list_tab_ids().await;
    let mut guard = TabGuard::new("real5_1b".to_string(), before_ids);

    let create_resp = wsl_raw_call(
        "tc2",
        "tab.create",
        json!({"workspace_id": WSL_WORKSPACE, "label": "real5_1b", "focus": false}),
    )
    .await;
    let tab_id = create_resp["result"]["tab"]["tab_id"]
        .as_str()
        .unwrap_or_else(|| panic!("tab.create 回應應含 result.tab.tab_id：{create_resp:?}"))
        .to_string();
    let pane_id = create_resp["result"]["root_pane"]["pane_id"]
        .as_str()
        .unwrap_or_else(|| panic!("tab.create 回應應含 result.root_pane.pane_id：{create_resp:?}"))
        .to_string();
    eprintln!("建立 tab: {tab_id}, pane = {pane_id}");

    let client = wsl_client();
    let subs = vec![Subscription::PaneAgentStatusChanged {
        pane_id: pane_id.clone(),
    }];

    let s1 = client.subscribe(&subs).await.expect("S1 訂閱應成功");
    eprintln!("[{:?}] S1 started", start.elapsed());
    let s1_collector = EventCollector::spawn(s1, start);

    let s2 = client.subscribe(&subs).await.expect("S2 訂閱應成功");
    eprintln!("[{:?}] S2 started", start.elapsed());
    let s2_collector = EventCollector::spawn(s2, start);

    // 重疊期：S1、S2 都還開著時觸發一次狀態變化，兩邊都應收到。
    wsl_report_agent(&pane_id, "working").await;
    tokio::time::sleep(Duration::from_millis(400)).await;

    let s1_overlap = s1_collector.stop().await; // 關 S1，模擬重開時關掉舊訂閱。
    eprintln!(
        "[{:?}] S1 已關閉，重疊期收到 {} 筆",
        start.elapsed(),
        s1_overlap.len()
    );

    for state in ["blocked", "idle", "working"] {
        wsl_report_agent(&pane_id, state).await;
        tokio::time::sleep(Duration::from_millis(300)).await;
    }
    tokio::time::sleep(Duration::from_millis(300)).await;

    // 先停 S2、驗證完整序列，再做 release-agent／關 tab——release-agent 本身會再觸發一筆
    // `agent_status: "unknown"`，不屬於本測試要驗的四段序列（沿用 spike 3 的觀察）。
    let s2_all = s2_collector.stop().await;

    let s1_status = extract_status_sequence(&s1_overlap, &pane_id);
    eprintln!("S1 重疊期狀態序列: {s1_status:?}");
    assert!(
        s1_status.contains(&AgentStatus::Working),
        "重疊期間 S1 應收到 working，實際 {s1_status:?}"
    );

    let s2_status = extract_status_sequence(&s2_all, &pane_id);
    eprintln!("S2 完整狀態序列: {s2_status:?}");
    // 實測發現（task 5.1）：對 `pane.report_agent` 送 `state: "idle"`，WSL 端 HERDR 回報的
    // `agent_status` 會在同一支測試的不同次執行間在 `idle`／`done` 之間跳動——這正好對應
    // `AgentStatus::Done` 的文件註解「已 idle 且尚未被看過」：headless 測試 server 沒有真正
    // 的 UI 去「看過」這個 pane，`idle` 是否被歸類成 `done` 由 server 內部一個我們從橋接端
    // 觀察不到的狀態決定，不是這個 client 或這個測試能控制的。第 4 個元素固定是 `working`
    // （不受這個模糊地帶影響），第 3 個元素放寬成兩者皆可接受，不假裝這裡有一個穩定的 1:1
    // 映射。
    assert_eq!(s2_status.len(), 4, "S2 應收到完整 4 筆，實際 {s2_status:?}");
    assert_eq!(
        s2_status[0],
        AgentStatus::Working,
        "S2 第 1 筆應為 working，實際 {s2_status:?}"
    );
    assert_eq!(
        s2_status[1],
        AgentStatus::Blocked,
        "S2 第 2 筆應為 blocked，實際 {s2_status:?}"
    );
    assert!(
        matches!(s2_status[2], AgentStatus::Idle | AgentStatus::Done),
        "S2 第 3 筆（原始 report 為 idle）應為 idle 或 done，實際 {s2_status:?}"
    );
    assert_eq!(
        s2_status[3],
        AgentStatus::Working,
        "S2 第 4 筆（S1 關閉後）應為 working，證明重開重疊期間無缺口，實際 {s2_status:?}"
    );

    wsl_release_agent(&pane_id).await;
    wsl_close_tab(&tab_id).await;
    guard.disarm();
}
