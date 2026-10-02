//! desktop-launch-notify task 2.1 驗收測試：`--exit-when-idle` 的閒置監看（spec
//! `desktop-launch`「閒置自動結束」五個 scenario；design D6）。
//!
//! 監看本身（[`app::shutdown_signal`]）以 `tokio::time::pause`（`start_paused = true`）跑：
//! 測試裡的 `sleep` 會在所有 task 都閒下來時直接把時鐘撥到下一個計時器，60 秒、10 秒的正式值
//! 不必真的等。連線與請求以 [`ClientActivity`] 的公開方法模擬——`/ws` 與 `GET /`、
//! `GET /api/state` 的處理常式呼叫的正是同一組方法（真實連線的計數由本檔後半的整合測試驗）。
//!
//! 每個情境都把監看 future spawn 成獨立 task，再以「某個時間點 `is_finished()` 與否」斷言
//! 它何時完成；注入的 Ctrl-C 是一個測試手上握著傳送端的 oneshot，不送就永遠不會完成。

use std::net::SocketAddr;
use std::sync::atomic::Ordering;
use std::time::Duration;

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use cockpit::app::{self, IdlePolicy};
use cockpit::http::{self, AppState, ClientActivity};
use cockpit_core::{RuntimeStore, StoreHandle};
use futures_util::SinkExt;
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;
use tokio::time::{Instant, sleep, sleep_until, timeout};
use tokio_tungstenite::tungstenite::Message as WsMessage;
use tower::ServiceExt;

/// 正式值（spec：從未連線 60 秒、閒置 10 秒）。
fn production() -> IdlePolicy {
    IdlePolicy::default()
}

/// 建一個帶 `--exit-when-idle`（或不帶）的關閉訊號 task；回傳 task 與 Ctrl-C 的傳送端。
/// 監看的起算點（「開始監聽」）就是呼叫這個函式的當下。
fn spawn_signal(
    exit_when_idle: bool,
    activity: &ClientActivity,
) -> (JoinHandle<anyhow::Result<()>>, oneshot::Sender<()>) {
    let (ctrl_c_tx, ctrl_c_rx) = oneshot::channel::<()>();
    let ctrl_c = async move {
        // 傳送端被 drop 不算 Ctrl-C：維持「不送就永遠不完成」。
        if ctrl_c_rx.await.is_err() {
            std::future::pending::<()>().await;
        }
        Ok(())
    };
    let signal = app::shutdown_signal(exit_when_idle, activity.clone(), production(), ctrl_c);
    (tokio::spawn(signal), ctrl_c_tx)
}

/// 睡到「起算點＋`secs` 秒」。
async fn at(start: Instant, secs: f64) {
    sleep_until(start + Duration::from_secs_f64(secs)).await;
}

#[test]
fn production_policy_is_sixty_and_ten_seconds() {
    let policy = IdlePolicy::default();
    assert_eq!(policy.startup_grace, Duration::from_secs(60));
    assert_eq!(policy.idle_grace, Duration::from_secs(10));
}

/// Scenario「最後一個畫面關閉後結束」：連線關閉後 10 秒內沒有新連線、也沒有 `GET`，約 10 秒時
/// 開始關閉，關閉訊號回 `Ok`（與 Ctrl-C 同一條正常關閉路徑，結束碼 0）。
#[tokio::test(start_paused = true)]
async fn closes_ten_seconds_after_last_connection_closes() {
    let activity = ClientActivity::new();
    let start = Instant::now();
    let (signal, _ctrl_c) = spawn_signal(true, &activity);

    at(start, 1.0).await;
    let connection = activity.connect();
    at(start, 5.0).await;
    drop(connection);

    at(start, 14.9).await;
    assert!(!signal.is_finished(), "關閉後未滿 10 秒不該結束");
    at(start, 15.1).await;
    assert!(signal.is_finished(), "關閉後滿 10 秒應該開始關閉");
    signal
        .await
        .expect("監看 task 不該 panic")
        .expect("閒置結束應該回 Ok（正常關閉、結束碼 0）");
}

/// Scenario「重新整理不會被誤殺」：關閉 3 秒後又有新連線，後端持續執行；之後再降為 0 時重新
/// 計時（從第二次降為 0 起算 10 秒，不是從第一次）。
#[tokio::test(start_paused = true)]
async fn reconnect_within_grace_keeps_running_and_restarts_countdown() {
    let activity = ClientActivity::new();
    let start = Instant::now();
    let (signal, _ctrl_c) = spawn_signal(true, &activity);

    at(start, 1.0).await;
    let first = activity.connect();
    at(start, 5.0).await;
    drop(first);
    at(start, 8.0).await;
    let second = activity.connect();

    at(start, 200.0).await;
    assert!(!signal.is_finished(), "重新連上之後不該因閒置結束");

    drop(second);
    at(start, 209.9).await;
    assert!(!signal.is_finished(), "第二次降為 0 後應該重新計時");
    at(start, 210.1).await;
    assert!(signal.is_finished(), "第二次降為 0 滿 10 秒應該結束");
}

/// Scenario「啟動器偵測延長期限」：最後一個連線關閉 8 秒時收到 `GET /api/state`，之後 5 秒內
/// 有新連線——原本的期限（關閉後 10 秒）已過，但後端仍在執行。
#[tokio::test(start_paused = true)]
async fn api_state_request_extends_deadline_until_reconnect() {
    let activity = ClientActivity::new();
    let start = Instant::now();
    let (signal, _ctrl_c) = spawn_signal(true, &activity);

    at(start, 1.0).await;
    let first = activity.connect();
    at(start, 5.0).await;
    drop(first);
    at(start, 13.0).await;
    activity.record_request();

    at(start, 17.9).await;
    assert!(
        !signal.is_finished(),
        "GET 把期限延到 23 秒，原本 15 秒的期限已過也不該結束"
    );
    at(start, 18.0).await;
    let _second = activity.connect();

    at(start, 200.0).await;
    assert!(!signal.is_finished(), "請求後 5 秒內連上，後端應該持續執行");
}

/// 同上的反面：`GET` 之後沒有任何連線，期限是「最後一次 `GET` 後 10 秒」。
#[tokio::test(start_paused = true)]
async fn api_state_request_without_reconnect_closes_ten_seconds_after_request() {
    let activity = ClientActivity::new();
    let start = Instant::now();
    let (signal, _ctrl_c) = spawn_signal(true, &activity);

    at(start, 1.0).await;
    let first = activity.connect();
    at(start, 5.0).await;
    drop(first);
    at(start, 13.0).await;
    activity.record_request();

    at(start, 22.9).await;
    assert!(!signal.is_finished(), "最後一次 GET 後未滿 10 秒不該結束");
    at(start, 23.1).await;
    assert!(signal.is_finished(), "最後一次 GET 後滿 10 秒應該結束");
}

/// 連線期間或降為 0 之前的 `GET` 不延長期限（期限取「降為 0」與「之後最近一次 GET」較晚者）。
#[tokio::test(start_paused = true)]
async fn request_before_connection_closes_does_not_extend_deadline() {
    let activity = ClientActivity::new();
    let start = Instant::now();
    let (signal, _ctrl_c) = spawn_signal(true, &activity);

    at(start, 1.0).await;
    let first = activity.connect();
    at(start, 4.0).await;
    activity.record_request();
    at(start, 5.0).await;
    drop(first);

    at(start, 14.9).await;
    assert!(!signal.is_finished());
    at(start, 15.1).await;
    assert!(
        signal.is_finished(),
        "期限應該從降為 0 起算，不受之前的 GET 影響"
    );
}

/// Scenario「從未有人連上」：60 秒內沒有任何 `/ws` 連線就結束，期間的 `GET /api/state` 不延長。
#[tokio::test(start_paused = true)]
async fn never_connected_closes_after_sixty_seconds_despite_requests() {
    let activity = ClientActivity::new();
    let start = Instant::now();
    let (signal, _ctrl_c) = spawn_signal(true, &activity);

    for secs in [10.0, 30.0, 55.0, 59.0] {
        at(start, secs).await;
        activity.record_request();
    }

    at(start, 59.9).await;
    assert!(!signal.is_finished(), "開始監聽後未滿 60 秒不該結束");
    at(start, 60.1).await;
    assert!(signal.is_finished(), "60 秒內從未連線應該結束，GET 不延長");
    signal
        .await
        .expect("監看 task 不該 panic")
        .expect("從未連線結束也應該回 Ok");
}

/// design D6：監看端每次收到通知就重新評估，不等「值大於 0」。同一個時間點內 0→1→0（監看端
/// 來不及看到 1）也要算「曾經連線、剛降為 0」：改走 10 秒的閒置期限，不是 60 秒的啟動期限。
#[tokio::test(start_paused = true)]
async fn zero_one_zero_blip_counts_as_connection_and_restarts_countdown() {
    let activity = ClientActivity::new();
    let start = Instant::now();
    let (signal, _ctrl_c) = spawn_signal(true, &activity);

    at(start, 30.0).await;
    // 中間沒有任何 await：監看端只會看到最終值 0 與一次（或多次）變更通知。
    drop(activity.connect());

    at(start, 39.9).await;
    assert!(!signal.is_finished());
    at(start, 40.1).await;
    assert!(
        signal.is_finished(),
        "0→1→0 之後應該以閒置期限（10 秒）結束，不是等到 60 秒"
    );
}

/// 計時中的 0→1→0 也要取消並重新計時：第一次降為 0 在 5 秒，12 秒時一次極短連線，期限應該
/// 變成 22 秒，不是原本的 15 秒。
#[tokio::test(start_paused = true)]
async fn blip_during_countdown_restarts_it() {
    let activity = ClientActivity::new();
    let start = Instant::now();
    let (signal, _ctrl_c) = spawn_signal(true, &activity);

    at(start, 1.0).await;
    let first = activity.connect();
    at(start, 5.0).await;
    drop(first);
    at(start, 12.0).await;
    drop(activity.connect());

    at(start, 21.9).await;
    assert!(!signal.is_finished(), "短暫連線應該重新計時");
    at(start, 22.1).await;
    assert!(signal.is_finished());
}

/// 一直連著就不結束（連線數大於 0 時沒有任何期限）。
#[tokio::test(start_paused = true)]
async fn open_connection_never_times_out() {
    let activity = ClientActivity::new();
    let start = Instant::now();
    let (signal, _ctrl_c) = spawn_signal(true, &activity);

    at(start, 1.0).await;
    let _connection = activity.connect();
    at(start, 3600.0).await;
    assert!(!signal.is_finished());
}

/// Scenario「沒有旗標時不結束」：唯一的連線關閉並經過 60 秒（以及從未連線超過 60 秒）都不結束；
/// Ctrl-C 照常有效。
#[tokio::test(start_paused = true)]
async fn without_flag_never_closes_on_idle_but_ctrl_c_still_works() {
    let activity = ClientActivity::new();
    let start = Instant::now();
    let (signal, ctrl_c) = spawn_signal(false, &activity);

    at(start, 1.0).await;
    let connection = activity.connect();
    at(start, 5.0).await;
    drop(connection);

    at(start, 300.0).await;
    assert!(!signal.is_finished(), "不帶旗標時不因連線數結束");

    ctrl_c.send(()).expect("監看 task 應該還在等 Ctrl-C");
    let result = timeout(Duration::from_secs(1), signal)
        .await
        .expect("送出 Ctrl-C 後應該立刻完成")
        .expect("監看 task 不該 panic");
    result.expect("Ctrl-C 應該回 Ok");
}

/// 帶旗標時 Ctrl-C 與閒置監看以 select 合併：Ctrl-C 先到就照常結束。
#[tokio::test(start_paused = true)]
async fn with_flag_ctrl_c_still_works() {
    let activity = ClientActivity::new();
    let (signal, ctrl_c) = spawn_signal(true, &activity);

    sleep(Duration::from_secs(3)).await;
    assert!(!signal.is_finished());
    ctrl_c.send(()).expect("監看 task 應該還在等 Ctrl-C");
    timeout(Duration::from_secs(1), signal)
        .await
        .expect("送出 Ctrl-C 後應該立刻完成")
        .expect("監看 task 不該 panic")
        .expect("Ctrl-C 應該回 Ok");
}

/// Ctrl-C 監聽註冊失敗（`Err`）時照樣往外帶，不被閒置監看吞掉。
#[tokio::test(start_paused = true)]
async fn with_flag_ctrl_c_error_is_propagated() {
    let activity = ClientActivity::new();
    let signal = app::shutdown_signal(true, activity, production(), async {
        Err(anyhow::anyhow!("boom"))
    });
    let error = signal.await.expect_err("Ctrl-C 回 Err 時應該往外帶");
    assert!(format!("{error:#}").contains("boom"));
}

// ---------------------------------------------------------------------------
// 整合測試：真實 `/ws` 連線的計數與 `GET` 的活動時間（真的開 port，不暫停時鐘）
// ---------------------------------------------------------------------------

const IO_TIMEOUT: Duration = Duration::from_secs(5);

/// 起一個真的監聽 `127.0.0.1:0` 的 server；回傳位址、可另外打 `oneshot` 的同一份路由表、以及
/// 路由表內 `AppState::activity` 的 clone。
async fn serve() -> (SocketAddr, axum::Router, ClientActivity, StoreHandle) {
    let handle = StoreHandle::new(RuntimeStore::new());
    let state = AppState::new(handle.subscribe());
    let activity = state.activity.clone();
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind 127.0.0.1:0 不應該失敗");
    let addr = listener.local_addr().expect("local_addr 不應該失敗");
    // `/ws`、`/api/state` 套來源檢查（ws-source-check）：回填實際監聽埠，同 `app::run_with_shutdown`。
    state.port.store(addr.port(), Ordering::Relaxed);
    let router = http::router(state);
    let served = router.clone();
    tokio::spawn(async move {
        axum::serve(listener, served)
            .await
            .expect("axum::serve 不應該失敗");
    });
    (addr, router, activity, handle)
}

/// 不開 port 的版本（供暫停時鐘的測試用）：`AppState::port` 直接設成假想的監聽位址，請求以
/// `oneshot` 打進路由表；回傳值同 [`serve`]。
fn serve_in_memory() -> (SocketAddr, axum::Router, ClientActivity, StoreHandle) {
    let handle = StoreHandle::new(RuntimeStore::new());
    let state = AppState::new(handle.subscribe());
    let activity = state.activity.clone();
    let addr: SocketAddr = "127.0.0.1:7770".parse().expect("位址應該合法");
    state.port.store(addr.port(), Ordering::Relaxed);
    (addr, http::router(state), activity, handle)
}

async fn wait_for_connections(activity: &ClientActivity, expected: usize) {
    let mut connections = activity.connections();
    timeout(IO_TIMEOUT, connections.wait_for(|count| *count == expected))
        .await
        .unwrap_or_else(|_| panic!("等連線數變成 {expected} 逾時"))
        .expect("連線數的傳送端不該消失");
}

/// 打一個請求進 `router`，帶 `Host: <addr>`（`serve` 已把實際監聽埠回填進 `AppState::port`，
/// 來源檢查才會放行）。
async fn request(router: &axum::Router, addr: SocketAddr, method: Method, uri: &str) -> StatusCode {
    request_with_headers(router, method, uri, &[("host", &addr.to_string())]).await
}

/// 同 [`request`]，但自己指定完整標頭清單（測試被拒絕的來源）。
async fn request_with_headers(
    router: &axum::Router,
    method: Method,
    uri: &str,
    headers: &[(&str, &str)],
) -> StatusCode {
    let mut builder = Request::builder().method(method).uri(uri);
    for (name, value) in headers {
        builder = builder.header(*name, *value);
    }
    router
        .clone()
        .oneshot(builder.body(Body::empty()).expect("請求應該組得起來"))
        .await
        .expect("路由應該有回應")
        .status()
}

/// 連上 `/ws` 時計數為 1、正常關閉後回 0；`GET /api/state` 不計入連線數但更新活動時間。
#[tokio::test]
async fn ws_connection_is_counted_and_api_state_only_updates_activity() {
    let (addr, router, activity, _handle) = serve().await;
    assert_eq!(*activity.connections().borrow(), 0);
    assert_eq!(*activity.last_request().borrow(), None);

    let (mut ws, _response) = timeout(
        IO_TIMEOUT,
        tokio_tungstenite::connect_async(format!("ws://{addr}/ws")),
    )
    .await
    .expect("WebSocket 連線逾時")
    .expect("WebSocket 連線不應該失敗");
    wait_for_connections(&activity, 1).await;

    let before = Instant::now();
    assert_eq!(
        request(&router, addr, Method::GET, "/api/state").await,
        StatusCode::OK
    );
    assert_eq!(
        *activity.connections().borrow(),
        1,
        "/api/state 不計入連線數"
    );
    let recorded = activity
        .last_request()
        .borrow()
        .expect("GET /api/state 應該更新活動時間");
    assert!(recorded >= before, "活動時間應該是這次請求的時間");

    ws.send(WsMessage::Close(None))
        .await
        .expect("送出 close 不應該失敗");
    wait_for_connections(&activity, 0).await;
}

/// 客戶端不送 close、直接斷掉 TCP，計數同樣回 0（drop guard 涵蓋每一條結束路徑）。
#[tokio::test]
async fn ws_counter_returns_to_zero_when_client_drops_without_close() {
    let (addr, _router, activity, _handle) = serve().await;

    let mut sockets = Vec::new();
    for _ in 0..2 {
        let (ws, _response) = timeout(
            IO_TIMEOUT,
            tokio_tungstenite::connect_async(format!("ws://{addr}/ws")),
        )
        .await
        .expect("WebSocket 連線逾時")
        .expect("WebSocket 連線不應該失敗");
        sockets.push(ws);
    }
    wait_for_connections(&activity, 2).await;

    drop(sockets.pop());
    wait_for_connections(&activity, 1).await;
    drop(sockets.pop());
    wait_for_connections(&activity, 0).await;
}

/// `GET /` 也更新活動時間；其他請求（靜態資源、`HEAD /`）不影響。
#[tokio::test]
async fn only_get_index_and_get_api_state_update_activity() {
    let (addr, router, activity, _handle) = serve().await;

    assert_eq!(
        request(&router, addr, Method::GET, "/app/style.css").await,
        StatusCode::OK
    );
    assert_eq!(
        request(&router, addr, Method::GET, "/manifest.webmanifest").await,
        StatusCode::OK
    );
    assert_eq!(
        request(&router, addr, Method::HEAD, "/").await,
        StatusCode::OK
    );
    assert_eq!(
        request(&router, addr, Method::HEAD, "/api/state").await,
        StatusCode::OK
    );
    assert_eq!(
        *activity.last_request().borrow(),
        None,
        "GET / 與 GET /api/state 以外的請求不該更新活動時間"
    );

    let before = Instant::now();
    assert_eq!(
        request(&router, addr, Method::GET, "/").await,
        StatusCode::OK
    );
    let recorded = activity
        .last_request()
        .borrow()
        .expect("GET / 應該更新活動時間");
    assert!(recorded >= before);
    assert_eq!(*activity.connections().borrow(), 0, "GET / 不計入連線數");
}

// ---------------------------------------------------------------------------
// ws-source-check：被來源檢查拒絕的 `/ws`、`/api/state` 請求不算活動
// ---------------------------------------------------------------------------

/// Scenario「被拒的 /ws 不讓後端保持執行」（時間軸部分，暫停時鐘）：唯一連線在 5 秒關閉後，
/// 8 秒時被拒的 `GET /api/state`（外站 `Host`）不得延長期限（否則會拖到 18 秒）；`/ws` 升級請求
/// （外站 `Origin`）也不得計為連線。後端照常在 15 秒結束。
#[tokio::test(start_paused = true)]
async fn rejected_requests_do_not_extend_idle_deadline() {
    let (addr, router, activity, _handle) = serve_in_memory();
    let start = Instant::now();
    let (signal, _ctrl_c) = spawn_signal(true, &activity);

    at(start, 1.0).await;
    let first = activity.connect();
    at(start, 5.0).await;
    drop(first);

    at(start, 8.0).await;
    let port = addr.port();
    let evil_host = format!("evil.example:{port}");
    let own_host = format!("127.0.0.1:{port}");
    assert_eq!(
        request_with_headers(&router, Method::GET, "/api/state", &[("host", &evil_host)]).await,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        request_with_headers(
            &router,
            Method::GET,
            "/api/state",
            &[("host", &own_host), ("origin", "https://evil.example")]
        )
        .await,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        request_with_headers(
            &router,
            Method::GET,
            "/ws",
            &[
                ("host", &own_host),
                ("origin", "https://evil.example"),
                ("connection", "upgrade"),
                ("upgrade", "websocket"),
                ("sec-websocket-version", "13"),
                ("sec-websocket-key", "dGhlIHNhbXBsZSBub25jZQ=="),
            ]
        )
        .await,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        *activity.last_request().borrow(),
        None,
        "被拒的 /api/state 不該記錄活動時間"
    );
    assert_eq!(*activity.connections().borrow(), 0, "被拒的 /ws 不算連線");

    at(start, 14.9).await;
    assert!(!signal.is_finished());
    at(start, 15.1).await;
    assert!(
        signal.is_finished(),
        "被拒的請求不延長期限，關閉後滿 10 秒應該結束"
    );
}

/// 同上的真實連線版本（真的開 port）：外站 `Origin` 的 `/ws` 升級被拒，連線數始終為 0（也沒有
/// 0→1→0 的變更通知）；之後同源連線照常計數。
#[tokio::test]
async fn rejected_ws_upgrade_never_counts_as_connection() {
    use tokio_tungstenite::tungstenite::client::IntoClientRequest;

    let (addr, _router, activity, _handle) = serve().await;
    let mut connections = activity.connections();
    connections.mark_unchanged();

    let mut evil = format!("ws://{addr}/ws")
        .into_client_request()
        .expect("升級請求應該組得起來");
    evil.headers_mut().insert(
        "origin",
        "https://evil.example".parse().expect("合法標頭值"),
    );
    let result = timeout(IO_TIMEOUT, tokio_tungstenite::connect_async(evil))
        .await
        .expect("WebSocket 連線逾時");
    match result {
        Err(tokio_tungstenite::tungstenite::Error::Http(response)) => {
            assert_eq!(response.status(), StatusCode::FORBIDDEN);
        }
        other => panic!("外站 Origin 的升級應該得到 403，實際：{other:?}"),
    }
    // 給伺服器一點時間（若錯誤地升級了，計數會在這段時間內動起來）。
    sleep(Duration::from_millis(200)).await;
    assert!(
        !connections.has_changed().expect("連線數的傳送端不該消失"),
        "被拒的 /ws 不該讓連線數有任何變動"
    );
    assert_eq!(*activity.connections().borrow(), 0);

    let (_ws, _response) = timeout(
        IO_TIMEOUT,
        tokio_tungstenite::connect_async(format!("ws://{addr}/ws")),
    )
    .await
    .expect("WebSocket 連線逾時")
    .expect("同源（不帶 Origin）連線應該成功");
    wait_for_connections(&activity, 1).await;
}
