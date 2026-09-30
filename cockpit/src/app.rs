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
//! 所以 [`shutdown_components`] 在送出訊號之後還要 `await` 每個驅動器（共用一個總期限，
//! 逾時 abort 後的 `await` 另有 1 秒上限，design D11），並 abort ＋ `await` 那個永遠不會
//! 自己結束的投影任務；`run_with_shutdown` 的每一條回傳路徑（含
//! 錯誤路徑）都會先走完它（Codex 最終 review finding 2）。獨立執行檔隨後就銷毀 runtime
//! 所以看不出差別，但測試、嵌入與重複啟停會一路累積活著的 task。
//!
//! 有 project 時（task 3.4）：啟動先讀狀態檔（[`progress::load_progress`]）算出初始
//! `DomainState`，`StoreHandle` 帶著它建立；再建 [`ProgressService`] 與投影任務之間的
//! stale override channel（[`spawn_projector_with_stale_sink`]），並起一個背景任務持續把
//! 投影判定失效的覆蓋交給寫入服務刪除（design D3）。沒有 project 時完全不碰狀態檔，行為
//! 與 1b 相同（design Migration Plan）——`Components::progress_service` 就是 `None`。
//!
//! 停止時，這個背景任務跟驅動器、投影任務一樣要「等它真的結束」：投影任務被
//! [`shutdown_components`] abort＋await 之後，它持有的 stale channel 傳送端才會真的被
//! drop，接收端的 `rx.recv()` 才會收到 `None` 讓迴圈自然結束——所以要排在
//! [`shutdown_components`] **之後**才 await，不能提前。一旦傳送端真的掉了這個迴圈幾乎立刻
//! 結束；仍套 1 秒上限，防投影任務沒收掉時卡死（design D11）。

use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU16, Ordering};
use std::time::Duration;

use anyhow::Context;
use axum::Router;
use cockpit_core::{
    AgentRuntime, DomainState, Policy, RuntimeId, RuntimeStore, StoreHandle, driver,
    spawn_projector, spawn_projector_with_stale_sink,
};
use tokio::net::TcpListener;
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;

use crate::config::{Args, Config, ConfigSource, load};
use crate::files::PathMapping;
use crate::http::{AppState, router};
use crate::progress;
use crate::progress_service::ProgressService;
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
    /// 在共用期限內 `await` 它們，確認驅動器真的結束（只 drop `JoinHandle` 等於 detach）。
    pub drivers: Vec<JoinHandle<()>>,
    /// 投影任務的 task handle。它的迴圈永遠不會自己結束，所以關機時由
    /// [`shutdown_components`] `abort()` 之後再 `await`。
    pub projector: JoinHandle<()>,
    /// 已經接上 [`Components::handle`] 訂閱端的完整路由表。
    pub router: Router,
    /// 進度寫入服務；設定裡有 project 才建立，否則是 `None`（design Migration Plan：沒有
    /// project 時不讀不寫狀態檔）。這裡只負責讓它在程序內可取得——接進 HTTP 寫入路由是
    /// task 4.1 的事，這裡不建路由。
    pub progress_service: Option<ProgressService>,
    /// 持續把投影判定失效的覆蓋交給 [`Components::progress_service`] 刪除的背景任務；跟
    /// `progress_service` 同時有或同時沒有。[`run_with_shutdown`] 在
    /// [`shutdown_components`] 之後另外等它結束。
    pub stale_remover: Option<JoinHandle<()>>,
    /// 與 [`Components::router`] 內 `AppState::port` 共用同一個 `Arc`（task 4.1；design
    /// D6）：[`router`] 在監聽埠確定之前就已經組好，[`run_with_shutdown`] 開始
    /// `axum::serve` 之前透過這個把手把傳入 `listener` 的 `TcpListener::local_addr()`
    /// 實際埠寫回去，所有已經拿到 `AppState` clone 的請求都讀得到更新後的值。初值是設定
    /// 裡寫的埠（`listen = "127.0.0.1:0"` 時初值就是 0）。**這個回填只發生在
    /// `run_with_shutdown` 裡**——不透過它、只呼叫 `build_components` 的呼叫端（例如只用
    /// `Components::router` 打 `tower::oneshot` 的測試）不會有真正的監聽埠，`port` 會維持
    /// 初值（Codex fix round 1 finding 1：先前誤放在 `run()`，直接呼叫
    /// `run_with_shutdown` 的呼叫端會讀到永遠是 0／設定值的埠，4.2 的 Host 檢查會因此擋掉
    /// 合法請求）。
    pub port: Arc<AtomicU16>,
    /// 與 [`Components::router`] 內 `AppState::runtimes` 共用同一個 `Arc`（live-output task
    /// 4.1；design D2 第三點），同 [`Components::port`] 的既有模式：暴露出來只為了讓測試不
    /// 開 port 也驗得到「路由看得到的 runtime 對照表」。表只在這裡建立一次、之後不再變動；
    /// 這份 `Arc` 與交給 `driver::run` 的那份各自獨立，額外的擁有者不影響驅動器的停止流程
    /// （見 [`shutdown_all`] 文件——停止靠的是 [`Components::stops`] 與 `await`
    /// [`Components::drivers`]，跟這張表的 `Arc` 引用計數無關）。
    pub runtimes: Arc<HashMap<RuntimeId, Arc<dyn AgentRuntime>>>,
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

    let has_project = !config.projects.is_empty();
    let domain = load_domain(config)?;

    // 先把全部 runtime 登記進 `RuntimeStore`，才用它建 `StoreHandle`：`new_with_domain`
    // 拿建構當下的 store 內容算出 version 1，當 watch 頻道的初值。順序反過來（先建空
    // store 的 handle 再逐筆 `handle.register`）的話，`build_components` 一返回，
    // `handle.current()`／新訂閱者立刻拿到的還是那份「version 1、沒有任何 runtime」的
    // 投影——要等投影任務下一輪（dirty 通知＋50 ms 合併窗）才會追上，這段期間讀到的
    // Project 投影裡，剛從狀態檔復原的覆蓋會因為找不到對應 runtime 而被判成不可用
    // （Codex fix round 1 finding 2）。
    let mut store = RuntimeStore::new();
    for entry in &entries {
        tracing::info!(runtime = %entry.id, endpoint = %entry.endpoint, "登記 runtime");
        store.register(entry.id.clone(), entry.kind.clone(), entry.endpoint.clone());
    }
    let handle = StoreHandle::new_with_domain(store, domain);

    // 沒有 project 就不建寫入服務、不接 stale channel，行為與 1b 相同（design Migration
    // Plan）——即使設定裡仍給了 `[state] path`，也不讀不寫。判斷依 project 是否為空，不
    // 依路徑是否存在：`load_domain` 已經在有 project 卻沒有路徑時回錯，這裡才能放心
    // `expect` 有路徑。
    let (progress_service, stale_remover, projector) = if has_project {
        let path = config
            .state_path
            .clone()
            .expect("load_domain 成功時，有 project 必有狀態檔路徑");
        let service = ProgressService::new(handle.clone(), path);
        let (stale_tx, stale_rx) = mpsc::unbounded_channel();
        let projector = spawn_projector_with_stale_sink(handle.clone(), stale_tx);
        let stale_remover = service.spawn_stale_remover(stale_rx);
        (Some(service), Some(stale_remover), projector)
    } else {
        (None, None, spawn_projector(handle.clone()))
    };

    let policy = Policy {
        resnapshot: Duration::from_secs(config.polling.resnapshot_secs),
        ..Policy::default()
    };
    let mut stops = Vec::with_capacity(entries.len());
    let mut drivers = Vec::with_capacity(entries.len());
    // clone 一份 `Arc<dyn AgentRuntime>` 進表，原本那份照舊交給 `driver::run`（design D2
    // 第三點）：表建好後不再變動，`AppState::runtimes` 與這裡的 `runtimes` 共用同一個
    // `Arc<HashMap<..>>`，跟 `port` 的既有模式一致。
    let mut runtimes = HashMap::with_capacity(entries.len());
    for entry in entries {
        runtimes.insert(entry.id.clone(), Arc::clone(&entry.runtime));
        let (stop_tx, stop_rx) = oneshot::channel();
        stops.push(stop_tx);
        drivers.push(tokio::spawn(driver::run(
            entry.runtime,
            handle.clone(),
            policy.clone(),
            stop_rx,
        )));
    }
    let runtimes = Arc::new(runtimes);

    // 初值是設定裡寫的埠；`listen = "127.0.0.1:0"` 時要等 `run` 真的 `bind` 之後才知道實際
    // 拿到哪個埠（design D6）。`router` 在這裡就已經組好，所以兩邊共用同一個 `Arc`——`run`
    // 綁定成功後改的是這個 `Arc` 指到的內容，不是重新組一次路由表。
    let port = Arc::new(AtomicU16::new(config.server.listen.port()));
    let router = router(AppState {
        state: handle.subscribe(),
        progress: progress_service.clone(),
        port: Arc::clone(&port),
        runtimes: Arc::clone(&runtimes),
        path_mappings: Arc::new(path_mappings(config)),
        files: Arc::new(crate::files::FileSettings::embedded()),
        git_runner: Arc::new(cockpit_git::GitRunner::new()),
    });

    Ok(Components {
        handle,
        stops,
        drivers,
        projector,
        router,
        progress_service,
        stale_remover,
        port,
        runtimes,
    })
}

/// 檔案端點的路徑對應表（file-review task 3.1；design D2）：設定裡每一筆 `[[runtime]]` 各一筆，
/// 依端點決定 `cwd` 怎麼轉成主機路徑（[`PathMapping::from_endpoint`]）。
fn path_mappings(config: &Config) -> HashMap<RuntimeId, PathMapping> {
    config
        .runtimes
        .iter()
        .map(|runtime| {
            (
                RuntimeId::new(runtime.id.clone()),
                PathMapping::from_endpoint(&runtime.endpoint),
            )
        })
        .collect()
}

/// 啟動時算出初始 `DomainState`（task 3.4）：`config.projects` 為空就是沒有任何 project，
/// 不讀狀態檔、回傳空的 `DomainState`（design Migration Plan，行為與 1b 相同）；否則讀
/// `config.state_path` 指到的狀態檔並套用到 `config.projects` 上（design D5，容錯規則見
/// `progress` 模組文件）。
///
/// `config.state_path` 在有 project 時理論上必為 `Some`（`config::resolve_state_path`：只有
/// `projects` 為空，或設定來源沒有檔案可依附時才會是 `None`；`cockpit::config::load` 的
/// 三種真實來源——`--config`、工作目錄的 `cockpit.toml`、零設定——當中零設定不可能帶
/// project）。萬一違反這個前提（例如直接建構 `Config` 的測試），視為不應該發生的內部矛盾，
/// 回傳錯誤而不是靜默略過使用者的進度。
///
/// # Errors
///
/// 有 project 但沒有狀態檔路徑（見上），或狀態檔存在但無法解析、`version` 不支援時回傳
/// `Err`；[`progress::ProgressError`] 的訊息本身已含狀態檔路徑。
fn load_domain(config: &Config) -> anyhow::Result<DomainState> {
    if config.projects.is_empty() {
        return Ok(DomainState::default());
    }

    let path = config.state_path.as_deref().ok_or_else(|| {
        anyhow::anyhow!("設定含 project 卻沒有解出狀態檔路徑（不應該發生，請回報 bug）")
    })?;
    let known_runtime_ids: HashSet<&str> = config
        .runtimes
        .iter()
        .map(|runtime| runtime.id.as_str())
        .collect();
    let domain = progress::load_progress(path, config.projects.clone(), &known_runtime_ids)?;
    Ok(domain)
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
    let listener = match bind(listen).await {
        Ok(listener) => listener,
        Err(error) => {
            // `components` 已經建好：驅動器、投影任務，有 project 時還有寫入服務的
            // stale-remover 背景任務都已經在跑。這裡不能讓 `components` 就這樣被 `?`
            // drop 掉——`JoinHandle` drop 只是 detach，它們會變成永遠不會自己結束的孤兒
            // task（尤其是 stale-remover：projector 持有的 stale channel 傳送端還活著，
            // `rx.recv()` 永遠等不到 `None`）。跟 `run_with_shutdown` 共用同一套收尾
            // （Codex fix round 1 finding 1），收乾淨才把錯誤往外丟。
            shutdown_all(
                components.stops,
                components.drivers,
                components.projector,
                components.stale_remover,
                DRIVER_SHUTDOWN_TIMEOUT,
            )
            .await;
            return Err(error);
        }
    };
    // 這裡的 `local_addr` 只給日誌用；真正寫回 `AppState::port` 的地方是
    // `run_with_shutdown`（見該函式與 `Components::port` 文件），兩邊呼叫 `local_addr()`
    // 是刻意的小重複，不是遺漏——`run_with_shutdown` 才是唯一權威的回填點，也是唯一能保護
    // 到「直接呼叫 `run_with_shutdown`（不經 `run`）」呼叫端的地方。
    let local_addr = listener.local_addr().unwrap_or(listen);
    tracing::info!("dashboard 已啟動：http://{local_addr}/（Ctrl-C 結束）");

    run_with_shutdown(components, listener, ctrl_c()).await
}

/// 收尾時**所有驅動器共用**的「自己結束」總期限（design D11）：從送出停止訊號起算，期限
/// 到了還沒結束的驅動器一起 abort。不是每個驅動器各算一次——那樣最壞是 10 s×N。
pub const DRIVER_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(10);

/// `abort()` 之後（以及收投影任務、stale-remover 時）最多再等多久（design D11）。abort 只是
/// 非同步的取消請求，正常情況下幾乎立刻完成；收不掉的（例如卡在同步工作裡）就記 warn 並
/// 放手，不讓關機卡死。
pub const ABORT_AWAIT_TIMEOUT: Duration = Duration::from_secs(1);

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
/// **`AppState::port` 的唯一回填點**（Codex fix round 1 finding 1）：開始 `axum::serve`
/// 之前，用傳入的 `listener`（呼叫端已經 bind 好）呼叫 `local_addr()`，把實際監聽埠寫進
/// [`Components::port`] 共用的 `Arc`——`listen = "127.0.0.1:0"` 時這才是作業系統指派的
/// 埠，不是設定值。這裡是唯一權威的回填點：之前誤放在 [`run`] 裡，任何不經 `run`、直接
/// 呼叫這個函式的呼叫端（測試、把 `cockpit` 當函式庫嵌入的呼叫端）都不會補上，
/// `AppState::port` 會停在設定值／0，4.2 的 Host 來源檢查會因此擋掉合法請求。
/// `local_addr()` 理論上不會在一個已經 bind 成功的 listener 上失敗，但仍當成錯誤處理
/// （而不是 `expect`）：失敗時走跟其餘啟動失敗路徑相同的 [`shutdown_all`] 收尾，不留孤兒
/// task，再把錯誤往外帶。
///
/// # Errors
///
/// `listener.local_addr()` 失敗、axum 服務期間出錯、`shutdown` 回 `Err`、或 `shutdown`
/// 的結果收不到時回傳 `Err`。
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
        progress_service: _progress_service,
        stale_remover,
        port,
        runtimes: _runtimes,
    } = components;

    let local_addr = match listener.local_addr().context("無法取得監聽位址") {
        Ok(addr) => addr,
        Err(error) => {
            shutdown_all(
                stops,
                drivers,
                projector,
                stale_remover,
                DRIVER_SHUTDOWN_TIMEOUT,
            )
            .await;
            return Err(error);
        }
    };
    port.store(local_addr.port(), Ordering::Relaxed);

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

    shutdown_all(
        stops,
        drivers,
        projector,
        stale_remover,
        DRIVER_SHUTDOWN_TIMEOUT,
    )
    .await;

    outcome?;
    tracing::info!("bye");
    Ok(())
}

/// 停掉驅動器與投影任務，**等它們真的結束**才返回（Codex 最終 review finding 2），但每一段
/// 等待都有上限，不讓關機卡死（design D11）。
///
/// [`run_with_shutdown`] 的收尾步驟，每一條回傳路徑（含錯誤路徑）都會走完它。設成
/// `pub` 只為了兩件事：測試打得到這一段（否則只能透過 `run_with_shutdown` 間接測，
/// 逾時分支就得真的等 10 秒），以及把 `cockpit` 當函式庫嵌進別的程序的人能自訂
/// `driver_timeout`。
///
/// - 「停止」就是把 [`Components::stops`] 全部 drop：`cockpit_core::driver::run` 的每個
///   `select!` 都 `biased` 地先看 `stop`，sender 被 drop 就立刻釋放事件流（連同連線與
///   子程序）並返回。
/// - 然後依序 `await` 每個驅動器的 `JoinHandle`，但**所有驅動器共用同一個期限**
///   （`driver_timeout`，從這裡起算）：最壞總耗時 ≈ 一個期限，不是期限×驅動器數。只 drop
///   `JoinHandle` 沒有用——tokio 的 `JoinHandle` drop 只是 detach，task 照樣在背景跑。
/// - 期限到了還沒結束的驅動器記 warn、全部 `abort()`，**然後再 `await`**：`abort()` 只是
///   一個非同步的取消請求，送出去不代表 task 已經結束，future（連同它持有的連線與子程序）
///   要等執行器再排程到它才會被 drop（Codex scoped re-review，fix round 2）。這段 `await`
///   另有 [`ABORT_AWAIT_TIMEOUT`] 的共用上限；超過就記 warn 並放手（drop handle），不讓一個
///   收不掉的 task 把關機卡死。被自己 abort 掉（`JoinError::is_cancelled()`）是預期中的
///   結束，其餘 `JoinError`（panic）記 `warn`，都不讓關機失敗。
/// - 投影任務的迴圈永遠不會自己結束（它等的是 dirty 通知），所以只能 `abort()` 再
///   `await`，同樣以 [`ABORT_AWAIT_TIMEOUT`] 為上限。
pub async fn shutdown_components(
    stops: Vec<oneshot::Sender<()>>,
    drivers: Vec<JoinHandle<()>>,
    projector: JoinHandle<()>,
    driver_timeout: Duration,
) {
    drop(stops);

    let deadline = tokio::time::Instant::now() + driver_timeout;
    let mut overdue = Vec::new();
    for mut driver in drivers {
        // `&mut driver` 而不是 `driver`：`JoinHandle` 是 `Unpin`，`&mut` 就能當 future 用，
        // 逾時之後 handle 還在我們手上（`timeout_at(_, driver)` 會把它吃掉，就再也 await
        // 不到了）。期限過了之後 `timeout_at` 仍會先 poll 一次 handle，已經結束的驅動器照樣
        // 算正常結束。
        match tokio::time::timeout_at(deadline, &mut driver).await {
            Ok(joined) => log_join("驅動器沒有乾淨結束", joined),
            Err(_) => overdue.push(driver),
        }
    }

    if !overdue.is_empty() {
        tracing::warn!(
            timeout_secs = driver_timeout.as_secs(),
            drivers = overdue.len(),
            "驅動器在共用期限之內沒有結束，改為 abort 並等它們真的被取消"
        );
        // 先全部送出 abort，讓它們同時被取消，再共用一個 await 上限逐一等。
        for driver in &overdue {
            driver.abort();
        }
        let abort_deadline = tokio::time::Instant::now() + ABORT_AWAIT_TIMEOUT;
        for mut driver in overdue {
            match tokio::time::timeout_at(abort_deadline, &mut driver).await {
                Ok(joined) => log_join("驅動器沒有乾淨結束", joined),
                Err(_) => tracing::warn!(
                    timeout_secs = ABORT_AWAIT_TIMEOUT.as_secs(),
                    "驅動器 abort 之後仍未結束，放手不再等"
                ),
            }
        }
    }

    projector.abort();
    await_bounded(projector, "投影任務").await;
}

/// 在 [`ABORT_AWAIT_TIMEOUT`] 之內等一個 task 結束；超過就記 warn 並放手（drop handle）。
async fn await_bounded(mut task: JoinHandle<()>, what: &'static str) {
    match tokio::time::timeout(ABORT_AWAIT_TIMEOUT, &mut task).await {
        Ok(joined) => log_join(what, joined),
        Err(_) => tracing::warn!(
            task = what,
            timeout_secs = ABORT_AWAIT_TIMEOUT.as_secs(),
            "task 在上限之內沒有結束，放手不再等"
        ),
    }
}

/// `JoinHandle` 的結果：正常結束或被我們自己 abort 掉（`is_cancelled`）都是預期中的，
/// 其餘（panic）記 warn，不讓關機失敗。
fn log_join(what: &'static str, joined: Result<(), tokio::task::JoinError>) {
    match joined {
        Ok(()) => {}
        Err(error) if error.is_cancelled() => {}
        Err(error) => tracing::warn!(task = what, error = %error, "task 沒有乾淨結束"),
    }
}

/// [`shutdown_components`] 之外，再收掉（若有）寫入服務的失效覆蓋接收背景任務
/// （task 3.4；Codex fix round 1 finding 1）：[`run_with_shutdown`] 的正常收尾與 [`run`]
/// 的 bind 失敗路徑共用同一套邏輯，避免 `Components` 建好之後、還沒進到
/// `run_with_shutdown` 就被提早 `?` 丟掉——那樣 `stale_remover` 會變成孤兒 task（投影
/// 任務持有的 stale channel 傳送端還活著，它的 `rx.recv()` 永遠收不到 `None`）。
///
/// 順序刻意排在 [`shutdown_components`] **之後**：投影任務被 abort＋await 之後，它持有
/// 的 stale channel 傳送端才會真的被 drop，接收端的迴圈才會在下一次 `rx.recv()` 收到
/// `None` 後立刻返回。正常情況這裡幾乎不用等；但萬一投影任務在 [`ABORT_AWAIT_TIMEOUT`]
/// 之內沒收掉（傳送端仍活著），這個迴圈就永遠等不到 `None`——所以同樣以
/// [`ABORT_AWAIT_TIMEOUT`] 為上限（design D11），逾時就 abort 它、再給一次上限，不讓
/// 關機卡死。
pub async fn shutdown_all(
    stops: Vec<oneshot::Sender<()>>,
    drivers: Vec<JoinHandle<()>>,
    projector: JoinHandle<()>,
    stale_remover: Option<JoinHandle<()>>,
    driver_timeout: Duration,
) {
    shutdown_components(stops, drivers, projector, driver_timeout).await;

    if let Some(mut stale_remover) = stale_remover {
        match tokio::time::timeout(ABORT_AWAIT_TIMEOUT, &mut stale_remover).await {
            Ok(joined) => log_join("寫入服務的失效覆蓋接收任務", joined),
            Err(_) => {
                tracing::warn!(
                    timeout_secs = ABORT_AWAIT_TIMEOUT.as_secs(),
                    "寫入服務的失效覆蓋接收任務沒有自己結束，改為 abort"
                );
                stale_remover.abort();
                await_bounded(stale_remover, "寫入服務的失效覆蓋接收任務").await;
            }
        }
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
