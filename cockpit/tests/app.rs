//! Task 3.8 驗收測試：`cockpit::app` 的程序組裝與停止（design D16；spec `cockpit-config`
//! 「設定檔位置與零設定模式」的 `--config` 不存在情境、`runtime-driver`「可停止」）。
//!
//! `app::run` 是 `main` 的全部邏輯（`main` 只負責注入真值），所以「`--config` 指到不存在
//! 的檔案要非零結束且訊息含路徑」可以在程序內驗，不必真的 spawn 一個 `cockpit.exe`。
//! 組裝出來的部分（狀態庫、投影任務、驅動器、路由）由 `app::build_components` 回傳，
//! 測試用它驗「路由看得到登記的 runtime」與「停止把手 drop 之後驅動器真的結束」——都不
//! 開 port、也不碰真的 HERDR（端點故意指到一個不存在的指令）。

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU16, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use axum::body::Body;
use axum::http::{Request, StatusCode};
use cockpit::app::{self, Components};
use cockpit::config::{Args, Config, ConfigSource, PollingConfig, RuntimeConfig, ServerConfig};
use cockpit::http::{AppState, router};
use cockpit::progress_service::ProgressService;
use cockpit_core::{
    AgentStatus, BindingSource, ConnectionState, DomainState, Focused, Mark, Override, Pane,
    PaneId, ProgressOp, ProjectDef, ProjectId, ProjectedBinding, RuntimeId, RuntimeSnapshot,
    RuntimeStore, StoreHandle, TabId, TaskDef, TaskId, WorkspaceId, WorkstreamDef, WorkstreamId,
    spawn_projector,
};
use cockpit_herdr::HerdrEndpoint;
use http_body_util::BodyExt;
use tokio::sync::oneshot;
use tower::ServiceExt;

/// 一定連不上、也一定不會碰到真 HERDR 的設定：端點是一個不存在的指令，驅動器會在
/// `subscribe()` 就失敗，連線狀態停在 `connecting`／`disconnected` 之間輪替。
fn unreachable_config() -> Config {
    Config {
        server: ServerConfig {
            listen: "127.0.0.1:0".parse().expect("測試位址應可解析"),
        },
        polling: PollingConfig {
            resnapshot_secs: 30,
            wsl_probe_secs: 60,
        },
        runtimes: vec![RuntimeConfig {
            id: "local".to_string(),
            kind: "herdr".to_string(),
            endpoint: HerdrEndpoint::Command(vec!["cockpit-test-no-such-command".to_string()]),
        }],
        // task 3.1 新增：這裡不測 project／狀態檔，維持空清單、沒有狀態檔路徑。
        projects: Vec::new(),
        state_path: None,
        source: ConfigSource::Inline,
    }
}

/// 打 `/api/state` 拿目前這份投影（順便斷言路由回 200 與合法 JSON）。
async fn fetch_state(router: &axum::Router) -> serde_json::Value {
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/state")
                .body(Body::empty())
                .expect("請求應該組得起來"),
        )
        .await
        .expect("/api/state 應該有回應");
    assert_eq!(response.status(), StatusCode::OK, "/api/state 應該回 200");

    let body = response
        .into_body()
        .collect()
        .await
        .expect("body 收集不應該失敗")
        .to_bytes();
    serde_json::from_slice(&body).expect("/api/state 應該回合法 JSON")
}

/// 輪詢 `/api/state` 直到投影裡出現 runtime（投影任務有 50 ms 合併窗）；5 秒還沒出現
/// 就回最後一次的結果，讓後面的斷言把實際內容印出來。
async fn poll_state_until_runtime_appears(router: &axum::Router) -> serde_json::Value {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        let state = fetch_state(router).await;
        let empty = state["runtimes"]
            .as_array()
            .is_none_or(|runtimes| runtimes.is_empty());
        if !empty || tokio::time::Instant::now() >= deadline {
            return state;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

#[tokio::test]
async fn missing_config_path_exits_with_error_naming_path() {
    let cwd = std::env::temp_dir();
    let args = Args {
        config: Some(PathBuf::from("missing.toml")),
        exit_when_idle: false,
    };

    let error = app::run(&args, &cwd, &|_| None)
        .await
        .expect_err("--config 指到不存在的檔案時 app::run 應該回 Err");

    let rendered = format!("{error:#}");
    assert!(
        rendered.contains("missing.toml"),
        "錯誤訊息應該指出是哪個路徑找不到，實際是：{rendered}"
    );
}

#[tokio::test]
async fn app_serves_state_and_shuts_down_on_stop() {
    let config = unreachable_config();
    let Components {
        router,
        stops,
        drivers,
        ..
    } = app::build_components(&config).expect("組裝應該成功");

    // 投影任務把變動合併 50 ms 才廣播（design D9），所以剛組裝完的第一份投影還是空的；
    // 輪詢到登記的 runtime 出現為止，逾時就讓測試失敗。
    let state = poll_state_until_runtime_appears(&router).await;

    let runtimes = state["runtimes"]
        .as_array()
        .expect("投影應該有 runtimes 陣列");
    assert_eq!(
        runtimes.len(),
        1,
        "設定裡只有一筆 runtime，投影也應該只有一筆"
    );
    assert_eq!(runtimes[0]["id"], "local", "runtime id 應該原樣來自設定");
    assert_eq!(
        runtimes[0]["kind"], "herdr",
        "runtime kind 應該原樣來自設定"
    );
    let connection = runtimes[0]["connection"]["state"]
        .as_str()
        .expect("connection.state 應該是字串");
    assert!(
        matches!(connection, "connecting" | "disconnected"),
        "連不上的端點只會停在 connecting 或 disconnected，實際是：{connection}"
    );

    // 「可停止」：Ctrl-C 之後 `main` 就是把這些 sender drop 掉，驅動器要自己結束。
    assert_eq!(stops.len(), 1, "每筆 runtime 應該有一個停止把手");
    drop(stops);
    for driver in drivers {
        tokio::time::timeout(Duration::from_secs(5), driver)
            .await
            .expect("停止把手 drop 後驅動器應該在 5 秒內結束")
            .expect("驅動器 task 不應該 panic");
    }
}

/// live-output task 4.1（design D2 第三點）：`Components::runtimes`（與路由表內
/// `AppState::runtimes` 共用同一個 `Arc`，同 `Components::port` 的既有模式）要含設定裡
/// 每一筆 `[[runtime]]` 的 id，且筆數相等——不多也不少。
#[tokio::test]
async fn app_state_holds_every_configured_runtime() {
    let mut config = unreachable_config();
    config.runtimes.push(RuntimeConfig {
        id: "local2".to_string(),
        kind: "herdr".to_string(),
        endpoint: HerdrEndpoint::Command(vec!["cockpit-test-no-such-command-2".to_string()]),
    });

    let components = app::build_components(&config).expect("組裝應該成功");

    let configured_ids: HashSet<RuntimeId> = config
        .runtimes
        .iter()
        .map(|runtime| RuntimeId::new(runtime.id.clone()))
        .collect();
    let table_ids: HashSet<RuntimeId> = components.runtimes.keys().cloned().collect();

    assert_eq!(
        table_ids, configured_ids,
        "runtimes 表應該恰好含設定裡每一筆 runtime id"
    );
    assert_eq!(
        components.runtimes.len(),
        config.runtimes.len(),
        "runtimes 表筆數應該等於設定裡的 runtime 筆數"
    );
}

/// repo 根的設定範例，內嵌進測試執行檔（不在執行期讀相對路徑）。
const EXAMPLE_CONFIG: &str = include_str!("../../cockpit.example.toml");

/// README 叫使用者 `cp cockpit.example.toml cockpit.toml`，所以範例必須真的解析得過；
/// 驗證規則改動時這個測試會先壞，而不是等使用者複製了才發現。
#[test]
fn example_config_parses_and_keeps_user_placeholder() {
    let config = cockpit::config::parse_toml(EXAMPLE_CONFIG)
        .expect("cockpit.example.toml 應該解析並驗證通過");

    assert_eq!(
        config.server.listen.to_string(),
        "127.0.0.1:7770",
        "範例的 listen 應該是預設的 loopback 位址"
    );
    let ids: Vec<&str> = config
        .runtimes
        .iter()
        .map(|runtime| runtime.id.as_str())
        .collect();
    assert_eq!(
        ids,
        ["win", "wsl"],
        "範例應該示範 Windows 與 WSL 兩筆 runtime"
    );

    assert!(
        EXAMPLE_CONFIG.contains("<user>"),
        "範例裡的路徑要用 <user> 佔位，不能留真實使用者名稱"
    );
}

// ---------------------------------------------------------------------------
// review round 1 finding：Ctrl-C 監聽註冊失敗被 `.ok()` 吞掉，會被誤判成正常關機。
// shutdown signal 改成可注入，下面兩個測試才打得到 serve／shutdown 這段。
// ---------------------------------------------------------------------------

/// 起一個真的監聽 `127.0.0.1:0` 的 listener，回傳它與位址。
async fn bind_ephemeral() -> (tokio::net::TcpListener, std::net::SocketAddr) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind 127.0.0.1:0 不應該失敗");
    let addr = listener.local_addr().expect("local_addr 不應該失敗");
    (listener, addr)
}

/// 最小的 HTTP/1.1 GET `/api/state`：`Connection: close` 讓 server 回完就關。
/// 這裡不能用 `tower::oneshot`——`router` 已經交給 `run_with_shutdown` 了，要驗的正是
/// 那個真的在跑的 server。
async fn http_get_state(addr: std::net::SocketAddr) -> serde_json::Value {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let mut stream = tokio::net::TcpStream::connect(addr)
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

/// 第一筆 runtime 目前的 `connection`：回傳 `(state, retry_in_secs)`，`retry_in_secs` 只在
/// `disconnected` 時有值。
fn connection_of(state: &serde_json::Value) -> (String, Option<u64>) {
    let connection = &state["runtimes"][0]["connection"];
    (
        connection["state"]
            .as_str()
            .unwrap_or("<缺欄位>")
            .to_string(),
        connection["retry_in_secs"].as_u64(),
    )
}

/// 輪詢 `/api/state` 直到第一筆 runtime 斷線，回傳它當下的 `retry_in_secs`。
async fn wait_for_disconnected(addr: std::net::SocketAddr) -> u64 {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    let mut last = String::new();
    while tokio::time::Instant::now() < deadline {
        let (state, retry) = connection_of(&http_get_state(addr).await);
        if let Some(retry) = retry {
            return retry;
        }
        last = state;
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("等不到 runtime 斷線（最後看到 {last}）：驅動器可能根本沒跑完第一輪");
}

/// 輪詢 `/api/state` 直到退避秒數從 `from` 變掉。
///
/// 為什麼不看 `connection.state` 從 disconnected 變回 connecting：`connecting` 只存在
/// 幾微秒（`subscribe()` 立刻就失敗），會被投影任務的 50 ms 合併窗吃掉，畫面上永遠看不
/// 到。退避序列（1、2、4、8、16、30 秒）每跑完一輪就往前一項，那是驅動器確實還在重試、
/// 也就是停止把手還沒 drop 的可觀察證據。
async fn wait_until_retry_changes(addr: std::net::SocketAddr, from: u64) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    while tokio::time::Instant::now() < deadline {
        tokio::time::sleep(Duration::from_millis(50)).await;
        let (_, retry) = connection_of(&http_get_state(addr).await);
        if retry.is_some_and(|retry| retry != from) {
            return;
        }
    }
    panic!("退避秒數卡在 {from} 秒不動：驅動器已經被停掉了，停止把手不該在這個時間點 drop");
}

#[tokio::test]
async fn shutdown_signal_error_makes_run_fail() {
    let components = app::build_components(&unreachable_config()).expect("組裝應該成功");
    let (listener, _addr) = bind_ephemeral().await;

    let error =
        app::run_with_shutdown(components, listener, async { Err(anyhow::anyhow!("boom")) })
            .await
            .expect_err("shutdown signal 回 Err 時 run_with_shutdown 應該回 Err，不能當成正常關機");

    let rendered = format!("{error:#}");
    assert!(
        rendered.contains("boom"),
        "錯誤應該把 shutdown signal 的原因帶出來，實際是：{rendered}"
    );
}

#[tokio::test]
async fn stop_senders_dropped_only_after_signal() {
    let components = app::build_components(&unreachable_config()).expect("組裝應該成功");
    let (listener, addr) = bind_ephemeral().await;

    let (signal_tx, signal_rx) = tokio::sync::oneshot::channel::<()>();
    let run = tokio::spawn(app::run_with_shutdown(components, listener, async move {
        signal_rx.await.expect("測試會送出 signal");
        Ok(())
    }));

    // signal 還沒送：server 正在服務（`/api/state` 打得通），而且驅動器還活著——
    // 連不上的端點會一輪一輪重試，退避秒數會沿著 1、2、4… 往前走。停止把手若在進
    // serve 之前就被 drop，驅動器會停在原地，退避秒數從此不動。
    //
    // 不能只看 version 遞增：剛啟動時本來就有一批還沒廣播的變動（投影任務有 50 ms
    // 合併窗），那個遞增就算驅動器早就死了也會發生。
    let retry = wait_for_disconnected(addr).await;
    wait_until_retry_changes(addr, retry).await;
    assert!(
        !run.is_finished(),
        "還沒送出 signal，run_with_shutdown 不該結束"
    );

    signal_tx
        .send(())
        .expect("shutdown future 應該還在等 signal");
    tokio::time::timeout(Duration::from_secs(5), run)
        .await
        .expect("送出 signal 後 run_with_shutdown 應該在 5 秒內結束")
        .expect("run task 不應該 panic")
        .expect("真的收到 signal 時應該回 Ok");
}

/// Codex fix round 1 finding 1：`AppState::port` 只在 `run_with_shutdown` 裡回填，不是
/// `run`——`listen = "127.0.0.1:0"` 時，`Components::port`（與路由表內 `AppState::port`
/// 共用同一個 `Arc`）要在服務開始 `serve` 之前就被寫成 `TcpListener::local_addr()` 實際
/// 拿到的埠，不能停在設定值 0。
///
/// 在 `build_components` 之後、傳進 `run_with_shutdown` 之前先 `Arc::clone`
/// `components.port`：這就是路由表內 `AppState` 會讀到的同一個 `Arc`，不需要另外開
/// port 打 HTTP 才能驗（4.2 的來源檢查 middleware 才會真的讀它）。
#[tokio::test]
async fn run_with_shutdown_backfills_port_zero_with_actual_port() {
    let components = app::build_components(&unreachable_config()).expect("組裝應該成功");
    let port_handle = Arc::clone(&components.port);
    assert_eq!(
        port_handle.load(Ordering::Relaxed),
        0,
        "還沒開始服務前，port 應該還是設定值（unreachable_config 的 listen 是 \"127.0.0.1:0\"）"
    );

    let (listener, addr) = bind_ephemeral().await;
    assert_ne!(addr.port(), 0, "bind 127.0.0.1:0 之後，OS 指派的埠不會是 0");

    let (signal_tx, signal_rx) = tokio::sync::oneshot::channel::<()>();
    let run = tokio::spawn(app::run_with_shutdown(components, listener, async move {
        signal_rx.await.expect("測試會送出 signal");
        Ok(())
    }));

    // `port` 在開始 serve 之前就會被寫回，不需要等任何請求；輪詢比固定 sleep 穩，避免
    // 時序偶發失敗，逾時就讓斷言把最後讀到的值印出來。
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        let current = port_handle.load(Ordering::Relaxed);
        if current == addr.port() {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "port 應該在服務開始前就回填成實際監聽埠 {}，實際仍是 {current}",
            addr.port()
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    signal_tx
        .send(())
        .expect("shutdown future 應該還在等 signal");
    tokio::time::timeout(Duration::from_secs(5), run)
        .await
        .expect("送出 signal 後 run_with_shutdown 應該在 5 秒內結束")
        .expect("run task 不應該 panic")
        .expect("真的收到 signal 時應該回 Ok");
}

/// desktop-launch-notify task 2.1（spec「閒置自動結束」：走與 Ctrl-C 相同的正常關閉流程、
/// 結束碼 0）：`build_components` 的路由表與 `Components::activity` 是同一份計數，閒置監看
/// 接在 `run_with_shutdown` 上，最後一個 `/ws` 關閉後經過閒置期限就回 `Ok`。期限注入短值，
/// 真的連線、真的計時（不暫停時鐘）。
#[tokio::test]
async fn exit_when_idle_shuts_down_normally_after_last_ws_closes() {
    use futures_util::SinkExt;

    let components = app::build_components(&unreachable_config()).expect("組裝應該成功");
    let (listener, addr) = bind_ephemeral().await;
    let policy = app::IdlePolicy {
        startup_grace: Duration::from_secs(30),
        idle_grace: Duration::from_millis(300),
    };
    let signal = app::shutdown_signal(
        true,
        components.activity.clone(),
        policy,
        std::future::pending::<anyhow::Result<()>>(),
    );
    let run = tokio::spawn(app::run_with_shutdown(components, listener, signal));

    let (mut ws, _response) = tokio::time::timeout(
        Duration::from_secs(5),
        tokio_tungstenite::connect_async(format!("ws://{addr}/ws")),
    )
    .await
    .expect("WebSocket 連線逾時")
    .expect("WebSocket 連線不應該失敗");
    // 連著的期間（遠超過閒置期限）不結束。
    tokio::time::sleep(Duration::from_secs(1)).await;
    assert!(!run.is_finished(), "還有 /ws 連線時不該結束");

    ws.send(tokio_tungstenite::tungstenite::Message::Close(None))
        .await
        .expect("送出 close 不應該失敗");
    tokio::time::timeout(Duration::from_secs(15), run)
        .await
        .expect("最後一個連線關閉後應該在閒置期限＋收尾時間內結束")
        .expect("run task 不應該 panic")
        .expect("閒置結束是正常關閉，應該回 Ok");
}

// ---------------------------------------------------------------------------
// Codex 最終 review finding 2：`run_with_shutdown` 回傳時沒等驅動器與投影任務收乾淨。
//
// `drop(stops)` 之後解構出來的 `drivers`／`projector` 只是被 drop——tokio 的
// `JoinHandle` drop 只是 detach。驅動器收到停止訊號後會自己結束，但函式不等它們；
// `spawn_projector` 的迴圈根本不會結束，還自己持有一份 `StoreHandle`，在長生命週期
// runtime（測試、嵌入、重複啟停）裡就永遠活著。獨立執行檔隨後銷毀 runtime 所以看不出來。
// ---------------------------------------------------------------------------

/// 組一份 `Components`，`drivers` 換成測試自己控制的一個 task。
///
/// **為什麼不用真的 `driver::run`**：真驅動器收到停止訊號後幾乎是立刻返回，那段窗口短
/// 到「函式有等它」與「函式沒等、但它剛好也結束了」分不出來——實測把 await 拿掉，用真
/// 驅動器的版本照樣會通過（`projector.abort().await` 那個 await 點就足以讓它跑完）。
/// 要驗的性質是「`run_with_shutdown` 會 await `Components::drivers` 裡的每一個
/// `JoinHandle`」，所以放一個「停止訊號之後還要花 300 ms 才結束」的 task：只 drop
/// `JoinHandle`（detach）的話，函式會在那 300 ms 之前就回傳，旗標就還是 `false`。
///
/// 「真驅動器收到停止把手 drop 之後會自己結束」由
/// `app_serves_state_and_shuts_down_on_stop` 負責，這裡不重複。
///
/// 回傳值：`Components`、狀態庫把手的 clone、以及「那個 driver task 已經跑完」的旗標。
fn components_with_slow_driver() -> (Components, StoreHandle, Arc<AtomicBool>) {
    let handle = StoreHandle::new(RuntimeStore::new());
    let projector = spawn_projector(handle.clone());
    let router = router(AppState::new(handle.subscribe()));

    let finished = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&finished);
    let (stop_tx, stop_rx) = oneshot::channel::<()>();
    let driver = tokio::spawn(async move {
        // 停止把手被 drop（或送出訊號）就開始收尾；`DRIVER_TEARDOWN` 代表真驅動器釋放
        // 事件流、連線與子程序要花的時間。
        let _ = stop_rx.await;
        tokio::time::sleep(DRIVER_TEARDOWN).await;
        flag.store(true, Ordering::SeqCst);
    });

    (
        Components {
            handle: handle.clone(),
            stops: vec![stop_tx],
            drivers: vec![driver],
            projector,
            router,
            progress_service: None,
            stale_remover: None,
            port: Arc::new(AtomicU16::new(0)),
            runtimes: Arc::new(HashMap::new()),
            activity: cockpit::http::ClientActivity::new(),
        },
        handle,
        finished,
    )
}

/// 假驅動器收到停止訊號之後還要花多久才結束。比排程一輪長得多，短到不拖慢測試。
const DRIVER_TEARDOWN: Duration = Duration::from_millis(300);

/// (a) `run_with_shutdown` 回傳的**當下**，`Components::drivers` 裡的每個 task 都必須
/// 已經跑完。
///
/// 只 drop `JoinHandle` 的話那只是 detach：函式先回傳，驅動器還在後面慢慢收尾，事件流、
/// 連線與子程序在那段期間仍然開著。斷言不加 sleep，就是要它是「回傳前已經等過」。
#[tokio::test]
async fn run_with_shutdown_waits_for_every_driver_task_to_finish() {
    let (components, _handle, finished) = components_with_slow_driver();
    assert!(
        !finished.load(Ordering::SeqCst),
        "還沒開始關機，driver task 不該已經結束"
    );

    let (listener, _addr) = bind_ephemeral().await;
    tokio::time::timeout(
        Duration::from_secs(5),
        app::run_with_shutdown(components, listener, async { Ok(()) }),
    )
    .await
    .expect("run_with_shutdown 應該在 5 秒內結束")
    .expect("shutdown signal 回 Ok 時 run_with_shutdown 應該回 Ok");

    assert!(
        finished.load(Ordering::SeqCst),
        "run_with_shutdown 回傳時每個 driver task 都應該已經跑完（不是回傳後才慢慢收）"
    );
}

/// (a') 錯誤路徑也要走同一段收尾：`shutdown` 回 `Err` 時照樣要等驅動器結束，錯誤才往外帶。
#[tokio::test]
async fn run_with_shutdown_waits_for_drivers_even_when_shutdown_fails() {
    let (components, _handle, finished) = components_with_slow_driver();
    let (listener, _addr) = bind_ephemeral().await;

    let error = tokio::time::timeout(
        Duration::from_secs(5),
        app::run_with_shutdown(components, listener, async { Err(anyhow::anyhow!("boom")) }),
    )
    .await
    .expect("run_with_shutdown 應該在 5 秒內結束")
    .expect_err("shutdown signal 回 Err 時應該回 Err");

    assert!(
        finished.load(Ordering::SeqCst),
        "錯誤路徑也要等驅動器收乾淨再回傳，實際錯誤：{error:#}"
    );
}

/// (b) `run_with_shutdown` 回傳之後，投影任務必須已經結束。
///
/// 投影任務的迴圈永遠不會自己結束，又自己抓著一份 `StoreHandle`；沒人 abort 它的話，
/// 在同一個 runtime 裡重複啟停（測試、嵌入）就會留下一堆永遠活著的 task。觀察方式：
/// run 回傳之後動一下狀態庫，投影版本號必須**不再**變化——只有投影任務會推進它。
#[tokio::test]
async fn run_with_shutdown_stops_the_projector_task() {
    let components = app::build_components(&unreachable_config()).expect("組裝應該成功");
    let handle = components.handle.clone();
    let (listener, _addr) = bind_ephemeral().await;

    tokio::time::timeout(
        Duration::from_secs(5),
        app::run_with_shutdown(components, listener, async { Ok(()) }),
    )
    .await
    .expect("run_with_shutdown 應該在 5 秒內結束")
    .expect("shutdown signal 回 Ok 時 run_with_shutdown 應該回 Ok");

    let before = handle.current().version;
    // 這筆登記一定會改變投影內容（多一個 runtime），投影任務還活著就會廣播新版本。
    handle.register(
        RuntimeId::new("registered-after-shutdown"),
        "herdr".to_string(),
        "test://late".to_string(),
    );
    // 投影任務的合併窗是 50 ms，200 ms 足夠讓它跑完一輪。
    tokio::time::sleep(Duration::from_millis(200)).await;

    assert_eq!(
        handle.current().version,
        before,
        "run_with_shutdown 回傳之後投影任務就不該再產生新版本（它應該已經被收掉）"
    );
}

// ---------------------------------------------------------------------------
// Codex scoped re-review（fix round 2）：`shutdown_components` 的逾時分支只送出
// `abort()` 就往下走。tokio 的 abort 是**非同步的取消請求**——它只是標記 task 要被取消，
// 真正的 drop 要等執行器再排程到那個 task。所以「逾時 → abort → 回傳」的路徑上，driver
// 的 future（連同它持有的 HERDR 連線與子程序）可能在 `run_with_shutdown` 已經回傳之後
// 才被釋放，違反「回傳前真的收乾淨」。
// ---------------------------------------------------------------------------

/// 被 drop 時翻旗標。掛在 driver task 的 future 裡，測試就看得到「這個 future 真的被
/// drop 了」，而不只是「abort 已經送出去了」。
struct DropFlag(Arc<AtomicBool>);

impl Drop for DropFlag {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

/// 逾時分支也必須等 driver 的 future 真的被 drop 才返回。
///
/// 場景：一個**不理會停止訊號**的 driver task（`std::future::pending()`），逾時設 100 ms。
/// `shutdown_components` 回傳的當下，它持有的 `DropFlag` 必須已經被 drop。
///
/// 只送 `abort()` 就往下走的話，回傳時 task 還沒被執行器收掉，旗標還是 `false`。
///
/// **投影任務故意傳一個「已經跑完」的 handle**：`shutdown_components` 收尾投影任務時的
/// `projector.await` 是一個 await 點，執行器在那裡就會順手把剛被 abort 的 driver 收掉，
/// 於是「有等 driver」與「沒等、但投影那邊的 await 剛好幫忙收了」分不出來（實測未修正
/// 的版本會因此通過）。換成已經完成的 handle，`await` 直接 Ready、不讓出執行權，測到的
/// 才是 driver 分支自己的行為。
#[tokio::test]
async fn shutdown_components_waits_for_aborted_driver_to_actually_drop() {
    // 已經跑完的「投影任務」：`await` 它不會讓出執行權。
    let projector = tokio::spawn(async {});
    while !projector.is_finished() {
        tokio::task::yield_now().await;
    }

    let dropped = Arc::new(AtomicBool::new(false));
    let guard = DropFlag(Arc::clone(&dropped));
    let (stop_tx, _stop_rx) = oneshot::channel::<()>();
    let stuck = tokio::spawn(async move {
        let _guard = guard;
        // 完全不看停止訊號：只有 abort 收得掉它。
        std::future::pending::<()>().await;
    });

    let timeout = Duration::from_millis(100);
    let started = tokio::time::Instant::now();
    tokio::time::timeout(
        Duration::from_secs(5),
        app::shutdown_components(vec![stop_tx], vec![stuck], projector, timeout),
    )
    .await
    .expect("shutdown_components 不該卡死");
    let elapsed = started.elapsed();

    assert!(
        dropped.load(Ordering::SeqCst),
        "shutdown_components 回傳時，被 abort 的 driver future 必須已經被 drop（abort 只是\
         非同步的取消請求，送出去不等於已經結束）"
    );
    assert!(
        elapsed >= timeout,
        "應該先等滿逾時才 abort，實際只花了 {elapsed:?}"
    );
    assert!(
        elapsed < Duration::from_secs(2),
        "等滿逾時之後就該收工，不該再拖，實際花了 {elapsed:?}"
    );
}

// ---------------------------------------------------------------------------
// Task 3.4：`app.rs` 組裝進度寫入服務（design D1／D3／D5；spec `pipeline-progress`
// 「狀態檔載入與容錯」「狀態檔格式與持久化」）。
//
// 這三個測試都直接建構 `Config`（不透過 `config::load`），`runtimes` 刻意留空：這個 change
// 的重點是「有 project 時該不該讀／建狀態檔、寫入服務該不該接上」，不是 runtime 驅動器
// 本身（那些已經在上面的測試涵蓋）。留空 `runtimes` 讓 `build_components` 不 spawn 任何
// 驅動器，才能用 `Components::handle` 手動模擬一個已連線、有 pane 的 runtime，不必擔心
// 跟真的驅動器的重試迴圈搶著寫 `RuntimeStore`。
// ---------------------------------------------------------------------------

/// 每個測試專用的暫存目錄；沿用 `cockpit/tests/config.rs`、`cockpit/tests/progress_service.rs`
/// 自製的 `TempDir`（task 3.4 brief 約束：不加 `tempfile`）。
struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(tag: &str) -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock 應晚於 UNIX_EPOCH")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "cockpit-app-test-{tag}-{}-{nanos}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("建立測試暫存目錄");
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }

    /// 寫一個檔案到暫存目錄底下，回傳完整路徑（同 `cockpit/tests/config.rs` 的
    /// `TempDir::write`）。fix round 1 finding 1 的 `run()` 級測試需要一份真的
    /// `cockpit.toml`——`app::run` 只吃 `Args`／`cwd`，沒有直接建構 `Config` 的入口。
    fn write(&self, name: &str, contents: &str) -> PathBuf {
        let file = self.path.join(name);
        fs::write(&file, contents).expect("寫入測試設定檔");
        file
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

/// 一個 project `p`（stages `Spec`→`Build`）、一條 workstream `be`（沒有設定檔 `binding`——
/// 覆蓋是獨立於 `WorkstreamDef::binding` 的機制，不需要它）、一個 task `t1`。
fn sample_project() -> ProjectDef {
    ProjectDef {
        id: ProjectId::new("p"),
        name: "p".to_string(),
        stages: vec!["Spec".to_string(), "Build".to_string()],
        workstreams: vec![WorkstreamDef {
            id: WorkstreamId::new("be"),
            name: "be".to_string(),
            binding: None,
        }],
        tasks: vec![TaskDef {
            id: TaskId::new("t1"),
            title: "t1".to_string(),
            workstream: WorkstreamId::new("be"),
            stage: "Spec".to_string(),
            depends_on: Vec::new(),
        }],
    }
}

/// 直接建構 `Config`（不經 TOML／`config::load`）：`runtimes` 留空，其餘欄位固定值，
/// `projects`／`state_path` 由呼叫端決定。
fn config_with(projects: Vec<ProjectDef>, state_path: Option<PathBuf>) -> Config {
    Config {
        server: ServerConfig {
            listen: "127.0.0.1:0".parse().expect("測試位址應可解析"),
        },
        polling: PollingConfig {
            resnapshot_secs: 30,
            wsl_probe_secs: 60,
        },
        runtimes: Vec::new(),
        projects,
        state_path,
        source: ConfigSource::Inline,
    }
}

/// 同 [`config_with`]，但帶一筆 `local` runtime（端點連不上、也一定不會碰到真 HERDR，同
/// [`unreachable_config`]）——`restart_keeps_progress_and_override` 需要覆蓋的 runtime
/// 出現在 `config.runtimes` 才會被狀態檔載入視為已知（design D5）。
fn config_with_runtime(projects: Vec<ProjectDef>, state_path: Option<PathBuf>) -> Config {
    Config {
        runtimes: vec![RuntimeConfig {
            id: "local".to_string(),
            kind: "herdr".to_string(),
            endpoint: HerdrEndpoint::Command(vec!["cockpit-test-no-such-command".to_string()]),
        }],
        ..config_with(projects, state_path)
    }
}

#[tokio::test]
async fn corrupt_state_file_fails_startup() {
    let dir = TempDir::new("corrupt-state");
    let state_path = dir.path().join("cockpit.state.json");
    fs::write(&state_path, "{not json").expect("寫入損毀狀態檔");

    let config = config_with(vec![sample_project()], Some(state_path.clone()));

    // 不用 `Result::expect_err`：它要求 `Ok` 分支的型別也實作 `Debug`，但
    // `Components` 含 `axum::Router`／`JoinHandle` 等不需要（也不打算）讓它 `Debug` 的欄位。
    let error = match app::build_components(&config) {
        Ok(_) => panic!("狀態檔損毀時組裝應該失敗"),
        Err(error) => error,
    };

    let rendered = format!("{error:#}");
    assert!(
        rendered.contains(&state_path.display().to_string()),
        "錯誤訊息應該含狀態檔路徑，實際是：{rendered}"
    );
}

#[tokio::test]
async fn no_projects_creates_no_state_file() {
    let dir = TempDir::new("no-projects");
    // 刻意仍然給一個路徑（模擬使用者寫了 `[state] path` 卻沒有任何 `[[project]]`）：沒有
    // project 時不該讀也不該建狀態檔（design Migration Plan），即使設定裡指定了路徑。
    let state_path = dir.path().join("cockpit.state.json");

    let config = config_with(Vec::new(), Some(state_path.clone()));

    let components = app::build_components(&config).expect("沒有 project 應該組裝成功");

    assert!(
        components.progress_service.is_none(),
        "沒有 project 不該建立寫入服務"
    );
    assert!(
        !state_path.exists(),
        "沒有 project 不該建立狀態檔，即使設定裡指定了路徑"
    );
}

/// 讓 `runtime` 在 `handle` 裡登記、`connected`，且有一個未 exited 的 pane `pane_id`——
/// 讓 `progress_service::set_override` 的 `validate_override` 通過。
fn register_connected_pane(handle: &StoreHandle, runtime: &RuntimeId, pane_id: &str) {
    handle.register(runtime.clone(), "herdr".to_string(), "test".to_string());
    handle
        .replace(
            runtime,
            RuntimeSnapshot {
                server_version: "test".to_string(),
                protocol: 1,
                workspaces: Vec::new(),
                tabs: Vec::new(),
                panes: vec![Pane {
                    id: PaneId::new(pane_id),
                    workspace_id: WorkspaceId::new("w1"),
                    tab_id: TabId::new("t1"),
                    agent: None,
                    agent_status: AgentStatus::Idle,
                    title: None,
                    cwd: None,
                    label: None,
                    focused: false,
                    exited: false,
                    updated_at: SystemTime::UNIX_EPOCH,
                }],
                agents: Vec::new(),
                focused: Focused {
                    workspace_id: None,
                    tab_id: None,
                    pane_id: None,
                },
                protocol_warning: None,
            },
        )
        .expect("runtime 剛登記，replace 不應該失敗");
    handle
        .set_connection(
            runtime,
            ConnectionState::Connected {
                since: SystemTime::UNIX_EPOCH,
                server_version: "test".to_string(),
                protocol: 1,
                last_snapshot_at: SystemTime::UNIX_EPOCH,
                protocol_warning: None,
            },
        )
        .expect("runtime 剛登記，set_connection 不應該失敗");
}

#[tokio::test]
async fn restart_keeps_progress_and_override() {
    let dir = TempDir::new("restart");
    let state_path = dir.path().join("cockpit.state.json");

    let project = ProjectId::new("p");
    let task = TaskId::new("t1");
    let workstream = WorkstreamId::new("be");
    let runtime = RuntimeId::new("local");

    // 「上一次啟動」：用一個獨立、完全不接驅動器的 `StoreHandle`／`ProgressService`
    // （同一個狀態檔路徑）落一份檔案，模擬使用者操作後關機。`ProgressService` 本身怎麼把
    // 進度與覆蓋寫進狀態檔已經是 task 3.3 `progress_service` 測試的範圍；這裡只需要一份
    // 真的落在磁碟上、內容已知的狀態檔，且不想讓 `runtimes::build` 產生的驅動器（見本檔
    // 模組文件）跟這裡手動塞的 `RuntimeStore` 內容賽跑。
    let writer_handle = StoreHandle::new_with_domain(
        RuntimeStore::new(),
        DomainState::from_projects(vec![sample_project()]),
    );
    register_connected_pane(&writer_handle, &runtime, "p1");
    let writer = ProgressService::new(writer_handle, state_path.clone());
    writer
        .apply_progress(&project, &task, ProgressOp::Advance)
        .await
        .expect("推進應該成功");
    writer
        .apply_progress(&project, &task, ProgressOp::Complete)
        .await
        .expect("標 Completed 應該成功");
    writer
        .set_override(
            &project,
            &workstream,
            Override {
                runtime: runtime.clone(),
                pane_id: PaneId::new("p1"),
            },
        )
        .await
        .expect("設定覆蓋應該成功");

    // 「重啟」：組一份含 `local` runtime 的設定（覆蓋的 runtime 要在 `config.runtimes` 裡
    // 才會被狀態檔載入視為已知——design D5）指向同一個狀態檔，呼叫真正要驗的
    // `app::build_components`。`build_components` 本身是同步函式，這裡跟下面讀
    // `handle.current()` 之間不會讓出執行權給剛 spawn 的驅動器，讀到的必定是狀態檔載入＋
    // runtime 登記當下的結果，不受驅動器後續連線行為影響。
    let config = config_with_runtime(vec![sample_project()], Some(state_path.clone()));
    let restarted = app::build_components(&config).expect("重啟後組裝應該成功");

    // fix round 1 finding 2：`build_components` 一返回，`handle.current()` 就該已經看得到
    // 剛登記的 runtime——如果 `StoreHandle::new_with_domain` 是用空 `RuntimeStore` 算出
    // version 1（登記動作在那之後才發生），這裡讀到的會是那份「還沒有任何 runtime」的初值，
    // 要等投影任務下一輪（dirty 通知＋50 ms 合併窗）才追上。這裡刻意在 `build_components`
    // 之後不 `await` 任何東西就立刻讀，才測得到這個窗口。
    let projection = restarted.handle.current();
    assert_eq!(
        projection.runtimes.len(),
        1,
        "build_components 一返回，投影就該看得到剛登記的 runtime，不是空 store 算出的初值"
    );
    assert_eq!(projection.runtimes[0].id, runtime);

    let projected_project = projection
        .projects
        .iter()
        .find(|p| p.id == project)
        .expect("投影應該有 project p");
    let projected_task = projected_project
        .tasks
        .iter()
        .find(|t| t.id == task)
        .expect("投影應該有 task t1");
    assert_eq!(
        projected_task.stage, "Build",
        "重啟後投影中 t1 應該還在 Build"
    );
    assert_eq!(
        projected_task.mark,
        Mark::Completed,
        "重啟後投影中 t1 應該還是 Completed"
    );

    let projected_workstream = projected_project
        .workstreams
        .iter()
        .find(|w| w.id == workstream)
        .expect("投影應該有 workstream be");
    // `be` 在設定裡沒有靜態 `binding`（`sample_project`），所以「投影看到 override 的
    // runtime」跟「覆蓋根本沒被讀回來（呈現 `ProjectedBinding::None`）」是兩種不同結果：
    // `runtime` 這時還沒真的連上（端點是刻意連不上的指令，見 `config_with_runtime`），
    // 覆蓋本身依規則保留、解析成 `runtime_disconnected`（spec「斷線期間保留覆蓋」）——不是
    // `none`，才證明覆蓋真的被狀態檔載入讀回來了。
    match &projected_workstream.binding {
        ProjectedBinding::RuntimeDisconnected {
            runtime: bound_runtime,
            source,
        } => {
            assert_eq!(*bound_runtime, runtime);
            assert_eq!(
                *source,
                BindingSource::Override,
                "be 沒有靜態 binding，斷線的綁定來自覆蓋"
            );
        }
        other => panic!("重啟後 be 的覆蓋應該保留、解析成 runtime_disconnected，實際是：{other:?}"),
    }

    // 最後解析實際落在磁碟上的狀態檔內容，確認不只是記憶體裡看起來對。
    let raw_state = fs::read_to_string(&state_path).expect("重啟後狀態檔應該存在");
    let state_json: serde_json::Value =
        serde_json::from_str(&raw_state).expect("狀態檔應該是合法 JSON");
    // 狀態檔改為 v2（pipeline-progress「狀態檔格式與持久化」：系統寫出的狀態檔一律為 version 2；progress-model task 3.1）。
    assert_eq!(state_json["version"], 2);
    assert_eq!(state_json["projects"]["p"]["tasks"]["t1"]["stage"], "Build");
    assert_eq!(
        state_json["projects"]["p"]["tasks"]["t1"]["mark"],
        "completed"
    );
    assert_eq!(
        state_json["projects"]["p"]["overrides"]["be"]["runtime"],
        "local"
    );
    assert_eq!(
        state_json["projects"]["p"]["overrides"]["be"]["pane_id"],
        "p1"
    );
}

// ---------------------------------------------------------------------------
// Fix round 1（task 3.4）finding 1：`build_components` 在 `bind` listener 之前就 spawn
// 投影任務與（有 project 時）寫入服務的 stale-remover；`run()` 原本 `bind(listen).await?`
// 一失敗，剛建好的 `Components` 就被 `?` 整包 drop 掉——`JoinHandle` drop 只是 detach，
// 投影任務會變成永遠不會自己結束的孤兒 task，它持有的 stale channel 傳送端也不會被
// drop，stale-remover 的 `rx.recv()` 永遠等不到 `None`。修法：`run()` 的 `bind` 失敗分支
// 改呼叫新增的 `app::shutdown_all`（`run_with_shutdown` 也改用同一個函式），bind 失敗前
// 一樣先把背景 task 收乾淨才把錯誤往外丟。
//
// 下面兩個測試分兩層驗證：
// - `shutdown_all_finishes_projector_and_stale_remover`：白箱直接測 `shutdown_all` 本身
//   真的把投影任務跟 stale-remover 都收乾淨——用 `JoinHandle::abort_handle()`（可以
//   `Clone`，`is_finished()` 不消耗 handle）在交給 `shutdown_all`之前先留一份，`shutdown_all`
//   回傳後檢查兩者都回報已結束。這是最直接、不受時序影響的證據。
// - `run_cleans_up_background_tasks_when_bind_fails`：黑箱測 `app::run` 真的把這條路徑接
//   起來——先佔住一個埠，指向同一個埠啟動一份帶 project 的設定，斷言 `run` 回傳的錯誤含
//   綁不上的位址（`bind` 確實失敗，觸發的是要修的那個分支，不是別的錯誤）。
// ---------------------------------------------------------------------------

#[tokio::test]
async fn shutdown_all_finishes_projector_and_stale_remover() {
    let dir = TempDir::new("shutdown-all");
    let state_path = dir.path().join("cockpit.state.json");
    // `runtimes` 留空（同本檔其餘 task 3.4 測試）：這裡要驗的是投影任務／stale-remover，
    // 跟驅動器無關，留空讓 `components.drivers`／`components.stops` 都是空的，不必等任何
    // 真驅動器收尾，斷言不受時序影響。
    let config = config_with(vec![sample_project()], Some(state_path));

    let components = app::build_components(&config).expect("組裝應該成功");
    let projector_handle = components.projector.abort_handle();
    let stale_remover_handle = components
        .stale_remover
        .as_ref()
        .expect("有 project 應該有 stale-remover")
        .abort_handle();

    assert!(
        !projector_handle.is_finished(),
        "剛組裝完，投影任務不該已經結束"
    );
    assert!(
        !stale_remover_handle.is_finished(),
        "剛組裝完，stale-remover 不該已經結束"
    );

    // 模擬 `run()` 的 bind 失敗分支：`Components` 已經建好卻不能就這樣被 drop 掉，
    // 要走跟 `run_with_shutdown` 一樣的收尾。
    app::shutdown_all(
        components.stops,
        components.drivers,
        components.projector,
        components.stale_remover,
        Duration::from_secs(5),
    )
    .await;

    assert!(
        projector_handle.is_finished(),
        "shutdown_all 回傳時投影任務必須已經結束"
    );
    assert!(
        stale_remover_handle.is_finished(),
        "shutdown_all 回傳時 stale-remover 必須已經結束（不能變成孤兒 task）"
    );
}

#[tokio::test]
async fn run_cleans_up_background_tasks_when_bind_fails() {
    let dir = TempDir::new("bind-failure-run");

    // 佔住一個 loopback port，讓等一下 `app::run` 內部的 `bind` 真的失敗。
    let occupied = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind 127.0.0.1:0 不應該失敗");
    let addr = occupied.local_addr().expect("local_addr 不應該失敗");

    // 一份含 project 的真設定（`app::run` 只吃 `Args`／`cwd`，得走真的 `cockpit.toml`）：
    // 有 project 才會建立寫入服務與 stale-remover，才測得到 finding 1 要修的那個分支。
    dir.write(
        "cockpit.toml",
        &format!(
            r#"
[server]
listen = "{addr}"

[[project]]
id = "p"
stages = ["Spec", "Build"]

[[project.workstream]]
id = "be"

[[project.task]]
id = "t1"
workstream = "be"
"#
        ),
    );

    let args = Args {
        config: None,
        exit_when_idle: false,
    };
    let error = app::run(&args, dir.path(), &|_| None)
        .await
        .expect_err("位址已被佔用時 app::run 應該回 Err");

    let rendered = format!("{error:#}");
    assert!(
        rendered.contains(&addr.to_string()),
        "錯誤應該指出綁不上的位址（證明真的走到 bind 失敗這條分支，不是別的錯誤），\
         實際是：{rendered}"
    );

    drop(occupied);
}

// ---------------------------------------------------------------------------
// Task 6.4（design D11，1b deferred）：`shutdown_components` 的逾時改成所有驅動器共用一個
// 總期限（不再逐一各等一次，最壞 10 s×N）；逾時 `abort()` 之後的 `await` 另設 1 秒上限，
// 超過就記 warn 並放手，不讓關機卡死。另補 `run_with_shutdown` 收不到 shutdown 結果的
// bail 分支。
// ---------------------------------------------------------------------------

/// 一個完全不理會停止訊號、只有 abort 收得掉的假驅動器。
fn spawn_stuck_driver() -> tokio::task::JoinHandle<()> {
    tokio::spawn(std::future::pending::<()>())
}

/// 3 個卡住的驅動器：總耗時應該 ≈ 一個期限，而不是三倍（暫停時鐘，不真的等）。
#[tokio::test(start_paused = true)]
async fn shutdown_deadline_is_shared_across_drivers() {
    let projector = tokio::spawn(std::future::pending::<()>());
    let mut stops = Vec::new();
    let mut drivers = Vec::new();
    for _ in 0..3 {
        let (stop_tx, _stop_rx) = oneshot::channel::<()>();
        stops.push(stop_tx);
        drivers.push(spawn_stuck_driver());
    }
    let aborts: Vec<_> = drivers.iter().map(|d| d.abort_handle()).collect();

    let deadline = Duration::from_secs(10);
    let started = tokio::time::Instant::now();
    app::shutdown_components(stops, drivers, projector, deadline).await;
    let elapsed = started.elapsed();

    assert!(
        elapsed >= deadline,
        "應該先等滿總期限才 abort，實際只花了 {elapsed:?}"
    );
    assert!(
        elapsed < deadline * 2,
        "3 個驅動器應該共用同一個期限（≈ {deadline:?}），不是各等一次，實際花了 {elapsed:?}"
    );
    assert!(
        aborts.iter().all(tokio::task::AbortHandle::is_finished),
        "逾時的驅動器都應該已經被 abort 並收掉"
    );
}

/// abort 之後的 `await` 最多等 1 秒：遇到 abort 收不掉的 task（`spawn_blocking` 正在跑的
/// 同步工作，abort 對它無效）就記 warn 放手，`shutdown_components` 仍然要返回。
///
/// 這裡用真實時間：`spawn_blocking` 執行中時 tokio 的暫停時鐘不會自動推進，暫停時鐘反而
/// 會卡住。總耗時 ≈ 驅動器期限 50 ms ＋ abort 後上限 1 s。
#[tokio::test]
async fn abort_await_is_bounded() {
    let projector = tokio::spawn(async {});
    let (release_tx, release_rx) = std::sync::mpsc::channel::<()>();
    let unabortable = tokio::task::spawn_blocking(move || {
        // 等到測試放行（或傳送端被 drop）才結束；abort 對執行中的 blocking task 無效。
        let _ = release_rx.recv();
    });
    let blocking_abort = unabortable.abort_handle();
    let (stop_tx, _stop_rx) = oneshot::channel::<()>();

    let driver_timeout = Duration::from_millis(50);
    let started = std::time::Instant::now();
    let finished_in_time = tokio::time::timeout(
        Duration::from_secs(5),
        app::shutdown_components(vec![stop_tx], vec![unabortable], projector, driver_timeout),
    )
    .await;
    let elapsed = started.elapsed();

    // 先放行 blocking task，免得 runtime 收尾時卡在它上面。
    release_tx.send(()).ok();

    assert!(
        finished_in_time.is_ok(),
        "abort 收不掉的 task 不該讓 shutdown_components 卡死（abort 後 await 應有 1 秒上限）"
    );
    assert!(
        elapsed >= driver_timeout + Duration::from_secs(1),
        "應該先等滿期限、abort 後再等滿 1 秒上限才放手，實際只花了 {elapsed:?}"
    );
    assert!(
        elapsed < Duration::from_secs(3),
        "abort 後的 await 上限是 1 秒，實際花了 {elapsed:?}"
    );
    assert!(
        !blocking_abort.is_finished(),
        "前提檢查：那個 blocking task 在放行前確實收不掉（否則這個測試沒測到上限）"
    );
}

/// shutdown future 本身 panic 時（axum 把它放在獨立 task 裡跑，panic 會讓 serve 照樣
/// `Ok` 收工，但結果永遠送不出來）：`run_with_shutdown` 要走「收不到 shutdown 結果」
/// 的 bail 分支回 `Err`，而且回傳前一樣要把驅動器收乾淨。
#[tokio::test]
async fn run_with_shutdown_bail_branch() {
    async fn panicking_shutdown() -> anyhow::Result<()> {
        panic!("故意讓 shutdown future panic，結果送不出來");
    }

    let (components, _handle, finished) = components_with_slow_driver();
    let (listener, _addr) = bind_ephemeral().await;

    let error = tokio::time::timeout(
        Duration::from_secs(5),
        app::run_with_shutdown(components, listener, panicking_shutdown()),
    )
    .await
    .expect("run_with_shutdown 應該在 5 秒內結束")
    .expect_err("收不到 shutdown 結果時應該回 Err，不能假裝正常關機");

    assert!(
        format!("{error:#}").contains("沒有收到 shutdown signal 的結果"),
        "應該是 bail 分支的錯誤，實際是：{error:#}"
    );
    assert!(
        finished.load(Ordering::SeqCst),
        "bail 分支也要等驅動器收乾淨再回傳"
    );
}
