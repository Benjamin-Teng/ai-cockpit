//! 程序生命週期（design D16）：載入設定 → 建 `StoreHandle` 與投影任務 → 每筆 runtime
//! spawn 驅動器 → 起 axum → 等 Ctrl-C → 停掉驅動器與投影任務並**等它們真的結束** →
//! 結束。
//!
//! `main` 只負責注入真值（`std::env::args`、`std::env::current_dir`、`std::env::var`）並
//! 初始化 `tracing-subscriber`；所有邏輯都在這裡，測試才打得到（同 `config::load` 把
//! `cwd`／`lookup_env` 當參數的理由）。
//!
//! shutdown signal 本身也是注入的（[`run_with_shutdown`]），`run` 注入的才是真的 Ctrl-C；
//! 測試靠它打得到 serve／shutdown 這段，也才驗得到「Ctrl-C 監聽註冊失敗要非零結束」。
//!
//! 「停止」的實作就是把 [`Components::stops`] 裡的 `oneshot::Sender` 全部 drop：
//! `cockpit_core::driver::run` 的每個 `select!` 都 `biased` 地先看 `stop`，收到訊號**或
//! sender 被 drop** 都會立刻釋放事件流並返回（spec `runtime-driver`「可停止」），所以
//! `main` 不需要自己追蹤子程序與連線——`RuntimeEvents` 的 `Drop` 會收（design D2）。
//!
//! 但「送出停止訊號」不等於「已經停了」：tokio 的 `JoinHandle` 被 drop 只是 detach。
//! 所以 [`shutdown_components`] 在送出訊號之後還要逐一 `await` 每個驅動器，並 abort ＋
//! `await` 那個永遠不會自己結束的投影任務；`run_with_shutdown` 的每一條回傳路徑（含
//! 錯誤路徑）都會先走完它（Codex 最終 review finding 2）。獨立執行檔隨後就銷毀 runtime
//! 所以看不出差別，但測試、嵌入與重複啟停會一路累積活著的 task。

use std::future::Future;
use std::net::SocketAddr;
use std::path::Path;
use std::time::Duration;

use anyhow::Context;
use axum::Router;
use cockpit_core::{Policy, RuntimeStore, StoreHandle, driver, spawn_projector};
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;

use crate::config::{Args, Config, ConfigSource, load};
use crate::http::{AppState, router};
use crate::runtimes;

/// 組裝好、還沒開 port 的程序內部件。
///
/// 測試拿它驗「路由看得到登記的 runtime」與「停止把手 drop 之後驅動器真的結束」，不必
/// 真的 bind 一個 port 或起一個子程序。
pub struct Components {
    /// 狀態庫的共享把手；投影任務與所有驅動器共用同一個。
    pub handle: StoreHandle,
    /// 每個驅動器一個停止把手，順序與設定檔的 `[[runtime]]` 一致。全部 drop（或送出
    /// `()`）就是「停止」。
    pub stops: Vec<oneshot::Sender<()>>,
    /// 每個驅動器的 task handle，順序同 [`Components::stops`]；[`shutdown_components`]
    /// 逐一 `await` 它們，確認驅動器真的結束（只 drop `JoinHandle` 等於 detach）。
    pub drivers: Vec<JoinHandle<()>>,
    /// 投影任務的 task handle。它的迴圈永遠不會自己結束，所以關機時由
    /// [`shutdown_components`] `abort()` 之後再 `await`。
    pub projector: JoinHandle<()>,
    /// 已經接上 [`Components::handle`] 訂閱端的完整路由表。
    pub router: Router,
}

/// 依設定組出狀態庫、投影任務、每筆 runtime 的驅動器與路由表。
///
/// 必須在 tokio runtime 內呼叫（會 `tokio::spawn` 投影任務與驅動器）。驅動器的
/// [`Policy`] 只把 `resnapshot` 換成 `config.polling.resnapshot_secs`，退避序列沿用
/// [`Policy::default`] 的 1／2／4／8／16／30 秒（spec `runtime-driver`）。
///
/// # Errors
///
/// 任何一筆 `[[runtime]]` 交給 `cockpit-herdr` 的工廠失敗（`command` 為空、平台不支援
/// socket 端點）就整個失敗，錯誤訊息附上是哪一筆。
pub fn build_components(config: &Config) -> anyhow::Result<Components> {
    let entries = runtimes::build(config).context("組裝 runtime 失敗")?;

    let handle = StoreHandle::new(RuntimeStore::new());
    for entry in &entries {
        tracing::info!(runtime = %entry.id, endpoint = %entry.endpoint, "登記 runtime");
        handle.register(entry.id.clone(), entry.kind.clone(), entry.endpoint.clone());
    }

    let projector = spawn_projector(handle.clone());

    let policy = Policy {
        resnapshot: Duration::from_secs(config.polling.resnapshot_secs),
        ..Policy::default()
    };
    let mut stops = Vec::with_capacity(entries.len());
    let mut drivers = Vec::with_capacity(entries.len());
    for entry in entries {
        let (stop_tx, stop_rx) = oneshot::channel();
        stops.push(stop_tx);
        drivers.push(tokio::spawn(driver::run(
            entry.runtime,
            handle.clone(),
            policy.clone(),
            stop_rx,
        )));
    }

    let router = router(AppState {
        state: handle.subscribe(),
    });

    Ok(Components {
        handle,
        stops,
        drivers,
        projector,
        router,
    })
}

/// `main` 的全部邏輯：載入設定、組裝、開 port、服務到 Ctrl-C，然後停掉驅動器。
///
/// `cwd` 與 `lookup_env` 由呼叫端注入（`main` 傳真值，測試傳固定值），同 `config::load`。
///
/// # Errors
///
/// 設定載入或驗證失敗（含 `--config` 指的檔案不存在，錯誤訊息帶解析後的完整路徑）、
/// runtime 組裝失敗、`server.listen` 綁不上（port 被占用等；訊息帶位址與原因，**不**自動
/// 換 port）、或 axum 服務期間出錯時回傳 `Err`；`main` 直接把它往外丟，行程非零結束。
pub async fn run(
    args: &Args,
    cwd: &Path,
    lookup_env: &dyn Fn(&str) -> Option<String>,
) -> anyhow::Result<()> {
    let config = load(args, cwd, lookup_env)?;
    tracing::info!(
        source = %describe_source(&config.source),
        listen = %config.server.listen,
        runtimes = config.runtimes.len(),
        "載入設定"
    );

    let components = build_components(&config)?;

    let listen = config.server.listen;
    let listener = bind(listen).await?;
    let local_addr = listener.local_addr().unwrap_or(listen);
    tracing::info!("dashboard 已啟動：http://{local_addr}/（Ctrl-C 結束）");

    run_with_shutdown(components, listener, ctrl_c()).await
}

/// 收尾時最多等一個驅動器「自己結束」多久（每個各算一次）。逾時就記 warn、abort 它，
/// 再等它真的被取消，不讓關機卡死、也不讓 future 留到回傳之後才被 drop。
pub const DRIVER_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(10);

/// 跑 HTTP 服務直到 `shutdown` 完成，然後停掉驅動器與投影任務。shutdown signal 由呼叫端
/// 注入，測試才打得到這段（`run` 注入的是 [`ctrl_c`]）。
///
/// **只有 `shutdown` 回 `Ok(())` 才算「使用者真的要結束」**：那時才印 `bye`、回 `Ok`。
/// 回 `Err`（例如 Ctrl-C handler 根本註冊不起來）代表「監聽壞了」而不是「收到了 Ctrl-C」：
/// 照樣停止服務，但把錯誤往外帶，行程非零結束、也不印 `bye`（review round 1 finding：
/// 原本 `tokio::signal::ctrl_c().await.ok()` 會把註冊失敗吞掉，shutdown future 立刻完成，
/// 啟動故障看起來就像一次乾淨的關機）。
///
/// **收尾不分成敗**（Codex 最終 review finding 2）：不論是正常關機還是上面任何一條錯誤
/// 路徑，回傳之前一律走完 [`shutdown_components`]。回傳時驅動器與投影任務都已經真的結束
/// ——否則在長生命週期的 runtime（測試、嵌入、重複啟停）裡，它們會連同事件流、連線與
/// 子程序一起留下來。
///
/// # Errors
///
/// axum 服務期間出錯、`shutdown` 回 `Err`、或 `shutdown` 的結果收不到時回傳 `Err`。
pub async fn run_with_shutdown(
    components: Components,
    listener: TcpListener,
    shutdown: impl Future<Output = anyhow::Result<()>> + Send + 'static,
) -> anyhow::Result<()> {
    let Components {
        stops,
        drivers,
        projector,
        router,
        handle: _handle,
    } = components;

    // `with_graceful_shutdown` 只吃 `Future<Output = ()>`，所以把 signal 的結果用
    // oneshot 帶出來，等 serve 收工之後再判。
    let (outcome_tx, outcome_rx) = oneshot::channel();
    let served = axum::serve(listener, router)
        .with_graceful_shutdown(async move {
            let outcome = shutdown.await;
            outcome_tx.send(outcome).ok();
        })
        .await
        .context("HTTP 伺服器異常結束");

    // 先把成敗算出來、**不要**在這裡 `?`：每一條 return 都得先經過下面那段 cleanup。
    let outcome = match served {
        Err(error) => Err(error),
        Ok(()) => match outcome_rx.await {
            Ok(result) => result,
            // `axum::serve` 只在 shutdown future 完成之後才回 `Ok`，所以收不到結果代表那個
            // future 被丟掉了。當成錯誤處理，不要靜悄悄地假裝是一次正常關機。
            Err(_) => Err(anyhow::anyhow!(
                "HTTP 伺服器結束了，但沒有收到 shutdown signal 的結果"
            )),
        },
    };

    shutdown_components(stops, drivers, projector, DRIVER_SHUTDOWN_TIMEOUT).await;

    outcome?;
    tracing::info!("bye");
    Ok(())
}

/// 停掉驅動器與投影任務，**等它們真的結束**才返回（Codex 最終 review finding 2）。
///
/// [`run_with_shutdown`] 的收尾步驟，每一條回傳路徑（含錯誤路徑）都會走完它。設成
/// `pub` 只為了兩件事：測試打得到這一段（否則只能透過 `run_with_shutdown` 間接測，
/// 逾時分支就得真的等 10 秒），以及把 `cockpit` 當函式庫嵌進別的程序的人能自訂
/// `driver_timeout`。
///
/// - 「停止」就是把 [`Components::stops`] 全部 drop：`cockpit_core::driver::run` 的每個
///   `select!` 都 `biased` 地先看 `stop`，sender 被 drop 就立刻釋放事件流（連同連線與
///   子程序）並返回。
/// - 然後逐一 `await` 每個驅動器的 `JoinHandle`。只 drop `JoinHandle` 沒有用——tokio 的
///   `JoinHandle` drop 只是 detach，task 照樣在背景跑。單一驅動器超過 `driver_timeout`
///   沒結束就記 warn、`abort()` 它，**然後再 `await` 一次**：`abort()` 只是一個非同步的
///   取消請求，送出去不代表 task 已經結束，future（連同它持有的連線與子程序）要等執行器
///   再排程到它才會被 drop（Codex scoped re-review，fix round 2）。被自己 abort 掉
///   （`JoinError::is_cancelled()`）是預期中的結束，其餘 `JoinError`（panic）記 `warn`，
///   都不讓關機失敗。
/// - 投影任務的迴圈永遠不會自己結束（它等的是 dirty 通知），所以只能 `abort()` 再
///   `await`；`JoinError::is_cancelled()` 同樣是預期中的正常結束。
///
/// `run_with_shutdown` 傳的 `driver_timeout` 是 [`DRIVER_SHUTDOWN_TIMEOUT`]，每個驅動器
/// 各算一次。
pub async fn shutdown_components(
    stops: Vec<oneshot::Sender<()>>,
    drivers: Vec<JoinHandle<()>>,
    projector: JoinHandle<()>,
    driver_timeout: Duration,
) {
    drop(stops);

    for mut driver in drivers {
        // `&mut driver` 而不是 `driver`：`JoinHandle` 是 `Unpin`，`&mut` 就能當 future 用，
        // 逾時之後 handle 還在我們手上（`timeout(_, driver)` 會把它吃掉，就再也 await 不
        // 到了）。
        let joined = match tokio::time::timeout(driver_timeout, &mut driver).await {
            Ok(joined) => joined,
            Err(_) => {
                tracing::warn!(
                    timeout_secs = driver_timeout.as_secs(),
                    "驅動器在逾時之內沒有結束，改為 abort 並等它真的被取消"
                );
                // `abort()` 只是一個**非同步的取消請求**：送出去不代表 task 已經結束，
                // future（連同它持有的事件流、連線與子程序）要等執行器再排程到它才會被
                // drop。所以這裡必須再 `await` 一次，回傳時才真的收乾淨
                // （Codex scoped re-review，fix round 2）。
                driver.abort();
                driver.await
            }
        };
        match joined {
            Ok(()) => {}
            // 被我們自己 abort 掉是預期中的結束方式，不是異常。
            Err(error) if error.is_cancelled() => {}
            Err(error) => tracing::warn!(error = %error, "驅動器沒有乾淨結束"),
        }
    }

    projector.abort();
    if let Err(error) = projector.await
        && !error.is_cancelled()
    {
        tracing::warn!(error = %error, "投影任務沒有乾淨結束");
    }
}

/// `run` 用的預設 shutdown signal：等 Ctrl-C。
///
/// 註冊失敗（handler 建不起來）是**錯誤**，不是「收到了 Ctrl-C」——所以這裡用
/// `Context` 往外帶，不 `.ok()`。
async fn ctrl_c() -> anyhow::Result<()> {
    tokio::signal::ctrl_c()
        .await
        .context("無法註冊 Ctrl-C 監聽")
}

/// 綁定監聽位址；失敗時把位址寫進 context 訊息，底層原因留在 `anyhow` 的 source chain
/// （`{:#}` 印成「無法綁定 <位址>: <原因>」，`main` 回 `Err` 時的 `{:?}` 印成
/// 「無法綁定 <位址>」加一段 `Caused by:`）——位址與原因都會出現，且不自動換 port
/// （design D16 Risks）。
async fn bind(listen: SocketAddr) -> anyhow::Result<TcpListener> {
    TcpListener::bind(listen)
        .await
        .with_context(|| format!("無法綁定 {listen}"))
}

/// 設定來源的人話描述，只給日誌用。
fn describe_source(source: &ConfigSource) -> String {
    match source {
        ConfigSource::Explicit(path) => format!("--config {}", path.display()),
        ConfigSource::Cwd(path) => format!("工作目錄的 {}", path.display()),
        ConfigSource::ZeroConfig => "零設定模式".to_string(),
        ConfigSource::Inline => "呼叫端直接提供".to_string(),
    }
}
