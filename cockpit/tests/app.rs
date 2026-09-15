//! Task 3.8 驗收測試：`cockpit::app` 的程序組裝與停止（design D16；spec `cockpit-config`
//! 「設定檔位置與零設定模式」的 `--config` 不存在情境、`runtime-driver`「可停止」）。
//!
//! `app::run` 是 `main` 的全部邏輯（`main` 只負責注入真值），所以「`--config` 指到不存在
//! 的檔案要非零結束且訊息含路徑」可以在程序內驗，不必真的 spawn 一個 `cockpit.exe`。
//! 組裝出來的部分（狀態庫、投影任務、驅動器、路由）由 `app::build_components` 回傳，
//! 測試用它驗「路由看得到登記的 runtime」與「停止把手 drop 之後驅動器真的結束」——都不
//! 開 port、也不碰真的 HERDR（端點故意指到一個不存在的指令）。

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use cockpit::app::{self, Components};
use cockpit::config::{Args, Config, ConfigSource, PollingConfig, RuntimeConfig, ServerConfig};
use cockpit::http::{AppState, router};
use cockpit_core::{RuntimeId, RuntimeStore, StoreHandle, spawn_projector};
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
    let router = router(AppState {
        state: handle.subscribe(),
    });

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
