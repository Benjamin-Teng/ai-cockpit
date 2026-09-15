//! `HerdrRuntime`：把 `herdr-client` 的兩種呼叫（`session.snapshot`、`events.subscribe`）
//! 包成 `cockpit-core` 的 `AgentRuntime`（spec `herdr-runtime-session`「建立事件流的順序」
//! 「合併事件流與結束」「pane 集合改變時重開狀態訂閱」「取得 snapshot 與版本警告」；
//! design D1、D2、D8、D10、D12）。
//!
//! `subscribe()` 內部依序完成：（`wsl` 型 runtime）探測發行版 → 短連線 `session.snapshot`
//! 取 pane id 清單（seed，不套用到狀態庫）→ 長連線 L 訂閱 24 種生命週期事件 → 長連線 S 對
//! seed 中每個 pane 各訂一筆 `pane.agent_status_changed`（seed 沒有 pane 時不開 S）→ 回傳
//! 合併後的事件流。任何一步失敗都回傳帶原因的錯誤，而且**先關掉這一輪已經開好的長連線**
//! 再回傳。
//!
//! 合併流靠 `RuntimeEvents::channel()`：L 與 S 各一個 reader task，把翻譯後的事件送進同一個
//! mpsc；任一條結束或 I/O 錯誤時，先送一個指名 `L`／`S` 的錯誤項，再 abort 另一條 reader
//! task（sender 全部 drop → `RuntimeEvents::next()` 在錯誤項之後回 `None`）。`RuntimeEvents`
//! 被 drop 時 `UnstartedEvents::start` 綁定的 `JoinHandle` 一併 abort，task 持有的
//! `EventStream`／連線隨之關閉（design D2）。
//!
//! pane 集合會變：L 看到 `pane_created`／`pane_closed`／`pane_moved`、或 `snapshot()` 算出
//! 集合與目前 S 的清單不同時，交給 [`StatusSubscription`] 這個 S 管理器——它以 200 ms 去抖動
//! 合併多次觸發，然後**先**用新清單開好新 S（`Client::subscribe` 讀到 `subscription_started`
//! 才返回）**再**關掉舊 S；新清單為空時只關舊 S；新 S 建立失敗時往合併流送一個錯誤項，
//! 讓驅動器整輪重來（design D10）。
//!
//! 「誰的 pane 集合比較新」一律由 `StatusSubscription::generation` 這個只增不減的版本號決定
//! （Codex review round 1）：`snapshot()` 的回應若比期間觀察到的 L 事件舊就不覆寫目標集合，
//! 重開 task 提交前也要確認自己開的訂閱沒有被更新的目標集合取代（過時就整條丟掉重排），
//! 而且提交前一律再確認「還是同一輪」——否則舊一輪的結果會寫進新一輪的狀態。

use std::collections::BTreeSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, SystemTime};

use async_trait::async_trait;
use cockpit_core::{
    AgentRuntime, RuntimeError, RuntimeEvent, RuntimeEvents, RuntimeId, RuntimeSnapshot,
};
use herdr_client::client::{Client, EventStream, IncomingEvent, SessionSnapshotRequest};
use herdr_client::connector::Connector;
use herdr_client::types::{
    EventKind, PaneClosedPayload, PaneMovedPayload, PanePayload, Subscription,
};
use tokio::sync::mpsc::{Sender, WeakSender};
use tokio::task::{AbortHandle, JoinHandle};

use crate::probe::DistroProber;
use crate::translate;

/// 重開 S 的去抖動間隔（spec：200 ms 內的多次觸發合併成一次）。
const REOPEN_DEBOUNCE: Duration = Duration::from_millis(200);

/// 已實測過的 HERDR 協定版本範圍（design D12、設計文件 §2.8：本機兩台真機分別是 20 與 22）。
/// 落在範圍外時 `snapshot()` 記 warn 並附警告字串，但照常回傳結果。
const TESTED_PROTOCOL_MIN: u32 = 20;
const TESTED_PROTOCOL_MAX: u32 = 22;

/// `wsl` 型 runtime 的探測設定（design D8）：建立事件流之前先確認這個發行版在運作中。
pub struct WslProbe {
    /// 要探測的 WSL 發行版名稱（例如 `"Ubuntu-24.04"`）。
    pub distro: String,
    /// 實際執行探測的探測器。
    pub prober: Box<dyn DistroProber>,
    /// 探測失敗時要附上的固定重試間隔，來自設定的 `wsl_probe_secs`（design D4）。
    ///
    /// **這個值才是 runtime 對外的保證**：`subscribe()` 會把探測器回傳的任何錯誤正規化成
    /// `RuntimeError::Unavailable`，`retry_after` 一律換成這個欄位——不論探測器回的是
    /// `Failed`（沒有間隔）還是帶了別的間隔的 `Unavailable`。探測器自帶的間隔（例如
    /// `WslProber::retry_after`）只是它自己的建議，runtime 不依賴它是對的。
    pub retry_after: Duration,
}

/// 一條合併事件流的收尾協調（Codex review round 1 finding 1）。
///
/// `closing` 是一次性旗標：L 與 S 幾乎同時結束（例如兩條連線同時 EOF）時，兩個 reader task
/// 會同時走到收尾，但 spec「合併事件流與結束」要求消費端**只看到一個**錯誤項——所以誰先
/// `compare_exchange(false, true)` 成功誰才送錯誤項並 abort 其他 task，輸的一方直接結束、
/// 什麼都不送（它手上的 sender 隨 task 結束而 drop）。重開 S 失敗時走的是同一個旗標。
///
/// `aborts` 是這條流的所有背景 task handle（L、S、重開之後的新 S、以及去抖動中的重開
/// task）；收尾的一方把它們全部 abort，讓 sender 全部消失，`RuntimeEvents::next()` 於是在
/// 錯誤項之後回 `None`。
#[derive(Default)]
struct Shutdown {
    closing: AtomicBool,
    aborts: Mutex<Vec<AbortHandle>>,
}

impl Shutdown {
    /// 搶下「由我送出錯誤項並收尾」的資格；同一條流只有第一個呼叫者會拿到 `true`。
    fn claim(&self) -> bool {
        self.closing
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
    }

    /// 把本輪後來才 spawn 的 task（重開 task、重開後的新 S reader）登記進來，好讓收尾時
    /// 一併 abort。
    ///
    /// **已經開始收尾就不登記、直接 abort**：`abort_all()` 只 abort「當下已經在清單裡」的
    /// handle，晚一步登記的 task 不會有人再收拾它（會一直抓著連線不放）。`closing` 的讀取與
    /// 清單的寫入都在同一把鎖內，`abort_all()` 也持同一把鎖，所以不存在「檢查時還沒收尾、
    /// 登記完卻已經 abort 完」的窗口。持鎖期間沒有 await。
    fn register(&self, handle: AbortHandle) {
        let mut registry = self.aborts.lock().expect("Shutdown.aborts mutex poisoned");
        if self.closing.load(Ordering::SeqCst) {
            handle.abort();
        } else {
            registry.push(handle);
        }
    }

    /// abort 這條流的所有背景 task（含呼叫者自己那一條——此時已經跑完迴圈、後面沒有
    /// await 點，abort 自己不影響這次收尾，只是省去比對「哪一個 handle 是我」）。
    ///
    /// **先封閉再掃描**（Codex review round 1 finding 3）：在同一把鎖內先把 `closing` 設成
    /// `true`，再逐一 abort。只掃描而不封閉的話，掃描之後才 `register` 的 task（例如一個正
    /// 在提交的重開 task 剛 spawn 出來的新 S reader）會被放進一份沒有人會再掃第二次的清單，
    /// 從此逃過收尾——連線一直開著，還可能回寫已經是上一輪的 `StatusSubscription`。
    /// 封閉之後，晚到的 `register` 會在 `register` 裡當場 abort。
    ///
    /// 持鎖期間沒有 await：只是設一個旗標並逐一 abort，不會跨 await 持有 std Mutex。
    fn abort_all(&self) {
        let registry = self.aborts.lock().expect("Shutdown.aborts mutex poisoned");
        self.closing.store(true, Ordering::SeqCst);
        for handle in registry.iter() {
            handle.abort();
        }
    }
}

/// 掛在 L reader task 上的收尾 guard：L task 被 drop（`RuntimeEvents` 被 drop 時
/// `AbortOnDrop` 會 abort 它）時，把本輪**所有**背景 task 一併 abort。
///
/// 為什麼需要它：`UnstartedEvents::start` 只收一次 `JoinHandle`，重開之後才 spawn 的新 S
/// reader 與去抖動中的重開 task 不在那份清單裡，光靠 `AbortOnDrop` 收不掉（design D10 的
/// 「S 管理器要自己持有後續新 S 的 handle，並在本輪被釋放時 abort」）。L 一定活到本輪結束，
/// 所以把 guard 掛在 L 上最省事。
struct AbortAllOnDrop(Arc<Shutdown>);

impl Drop for AbortAllOnDrop {
    fn drop(&mut self) {
        self.0.abort_all();
    }
}

/// 一輪事件流共用的東西，給「後來才 spawn 的 task」用（design D10）。
///
/// `tx` 刻意是 [`WeakSender`]：S 管理器住在 `HerdrRuntime` 裡、活得比一輪事件流久，如果它
/// 抓著一份強 sender，L 與 S 都結束之後 `RuntimeEvents::next()` 也永遠等不到 `None`
/// （channel 還有 sender 活著）。重開時才 `upgrade()`——拿不到就代表本輪已經結束，什麼都
/// 不用做。
struct Session {
    runtime_id: RuntimeId,
    client: Arc<Client>,
    tx: WeakSender<Result<RuntimeEvent, RuntimeError>>,
    shutdown: Arc<Shutdown>,
}

/// S（每 pane 的 `pane.agent_status_changed`）訂閱的管理器（design D10）。
///
/// 住在 `HerdrRuntime` 內、跨重連共用同一個實例，所以每次 `subscribe()` 開頭要先
/// [`StatusSubscription::reset`]。
///
/// - `current`：目前這條 S 連線實際訂閱的 pane 清單。
/// - `desired`：最新的目標集合（L 的 pane 事件與 `snapshot()` 都只更新它）。
/// - `handle`：目前 S reader task 的 `AbortHandle`。
/// - `reopen_scheduled`：已經有一個重開在跑（去抖動中、或正在開新 S），多餘的觸發只更新
///   `desired`；旗標要等那次重開**提交或放棄之後**才清掉。
/// - `generation`：`desired` 的版本號，只增不減（跨 `reset()` 也不歸零）。每次 `desired` 被
///   動到就 +1，讓「拿著舊集合的非同步工作」能認出自己已經過時——`snapshot()` 用它判斷
///   回應是不是比 L 事件舊（finding 1），重開 task 用它判斷自己開好的訂閱是不是已經被更新
///   的目標集合取代（finding 2）。
/// - `session`：本輪事件流共用的東西；`None` 代表目前沒有活著的事件流（例如只呼叫過
///   `snapshot()`），這時不排程重開。重開 task 用 `Arc::ptr_eq` 比對它，確認自己還屬於
///   目前這一輪。
#[derive(Default)]
struct StatusSubscription {
    current: BTreeSet<String>,
    desired: BTreeSet<String>,
    handle: Option<AbortHandle>,
    reopen_scheduled: bool,
    generation: u64,
    session: Option<Arc<Session>>,
}

impl StatusSubscription {
    /// 清掉上一輪的狀態，並收拾上一輪留下的背景 task：abort 目前的 S reader，以及上一輪
    /// `Shutdown` 清單裡的所有 task（L、重開後的新 S、去抖動中的重開 task）；連線隨 task
    /// 被 drop 而關閉。
    ///
    /// `generation` 一併 +1（不歸零）：跨在這次 reset 兩側的 `snapshot()` 必須看得出「期間
    /// 整個換了一輪」，不能把上一輪的 pane 集合寫進新一輪的目標。
    fn reset(&mut self) {
        self.current.clear();
        self.desired.clear();
        self.reopen_scheduled = false;
        self.generation += 1;
        if let Some(handle) = self.handle.take() {
            handle.abort();
        }
        if let Some(session) = self.session.take() {
            session.shutdown.abort_all();
        }
    }

    /// 這個管理器目前這一輪是不是 `session` 這一輪（重開 task 提交前必須確認）。
    fn is_current_round(&self, session: &Arc<Session>) -> bool {
        self.session
            .as_ref()
            .is_some_and(|current| Arc::ptr_eq(current, session))
    }
}

fn lock_status(status: &Arc<Mutex<StatusSubscription>>) -> MutexGuard<'_, StatusSubscription> {
    status.lock().expect("HerdrRuntime.status mutex poisoned")
}

/// 一個接到 HERDR 端點的 `AgentRuntime`。
pub struct HerdrRuntime {
    id: RuntimeId,
    /// `Arc` 而不是 `Client`：重開 task 要在 `subscribe()` 早就返回之後自己發
    /// `events.subscribe`，需要一份能帶進 task 的 client（`Client` 本身沒有 `Clone`）。
    client: Arc<Client>,
    wsl: Option<WslProbe>,
    status: Arc<Mutex<StatusSubscription>>,
}

impl HerdrRuntime {
    /// 建立一個 HERDR runtime。`wsl` 為 `Some` 時是 `wsl` 型 runtime，`subscribe()` 之前會
    /// 先探測發行版（`snapshot()` 不探測，design D8）；`None` 則是不需要探測的 `win` 型。
    #[must_use]
    pub fn new(id: RuntimeId, connector: Arc<dyn Connector>, wsl: Option<WslProbe>) -> Self {
        Self {
            id,
            client: Arc::new(Client::new(connector)),
            wsl,
            status: Arc::new(Mutex::new(StatusSubscription::default())),
        }
    }

    /// 這個 runtime 要探測的 WSL 發行版名稱；`None` 代表不是 `wsl` 型（沒有探測器）。
    /// 供工廠的驗收測試與日誌辨認端點種類用。
    #[must_use]
    pub fn wsl_distro(&self) -> Option<&str> {
        self.wsl.as_ref().map(|wsl| wsl.distro.as_str())
    }

    /// 這個 runtime 的 WSL 探測失敗重試間隔；`None` 代表不是 `wsl` 型（沒有探測器）。
    /// 供組裝層（`cockpit::runtimes`）與測試觀察 `wsl_probe_secs` 是否確實轉傳到這裡，
    /// 不影響 `subscribe()` 本身的行為（探測失敗時實際用的仍是 `WslProbe.retry_after`）。
    #[must_use]
    pub fn wsl_retry_after(&self) -> Option<Duration> {
        self.wsl.as_ref().map(|wsl| wsl.retry_after)
    }

    fn status(&self) -> MutexGuard<'_, StatusSubscription> {
        lock_status(&self.status)
    }
}

#[async_trait]
impl AgentRuntime for HerdrRuntime {
    fn id(&self) -> &RuntimeId {
        &self.id
    }

    /// 取一份完整快照：短連線 `session.snapshot` → 翻譯 → 協定版本判讀 → 把 pane 集合交給
    /// S 管理器。不探測（design D8：連線活著本身就代表虛擬機在跑）。
    ///
    /// `protocol` 不在 20..=22（已實測過的範圍）時記一筆 `warn` 並填 `protocol_warning`
    /// （含實際版本號），但**照常回傳快照**（spec「取得 snapshot 與版本警告」、design D12）。
    ///
    /// **snapshot 不會撤銷期間觀察到的 L 觸發**（Codex review round 1 finding 1）：送出請求
    /// 前先記下 `desired` 的版本號，回應回來時若版本號變了（期間 L 看到 pane 增減，或整個
    /// 換了一輪），代表這份 snapshot 比事件舊——**不覆寫** `desired`，只在集合仍與 `current`
    /// 不同時排程重開。否則那個新 pane 會一路沒有 S 訂閱，要等下一次事件或下一次定期
    /// snapshot 才補得回來。
    async fn snapshot(&self) -> Result<RuntimeSnapshot, RuntimeError> {
        let generation_before = self.status().generation;
        let result = self
            .client
            .request(SessionSnapshotRequest {})
            .await
            .map_err(|e| RuntimeError::Failed(format!("snapshot 失敗：{e}")))?;
        let mut snapshot = translate::snapshot(&result.snapshot, SystemTime::now());

        if snapshot.protocol < TESTED_PROTOCOL_MIN || snapshot.protocol > TESTED_PROTOCOL_MAX {
            tracing::warn!(
                runtime = %self.id,
                protocol = snapshot.protocol,
                "HERDR protocol 不在已測範圍 {TESTED_PROTOCOL_MIN}..={TESTED_PROTOCOL_MAX}"
            );
            snapshot.protocol_warning = Some(format!(
                "HERDR protocol {} 不在已測範圍 {TESTED_PROTOCOL_MIN}..={TESTED_PROTOCOL_MAX}",
                snapshot.protocol
            ));
        }

        // 交給 S 管理器：集合與目前 S 的清單不同就排程重開（spec「snapshot 發現集合不同」）。
        // 沒有活著的事件流（`session` 為 `None`）時什麼都不做。持鎖期間沒有 await。
        let panes: BTreeSet<String> = result
            .snapshot
            .panes
            .iter()
            .map(|pane| pane.pane_id.clone())
            .collect();
        {
            let mut state = self.status();
            if state.session.is_some() {
                if state.generation == generation_before {
                    // 期間沒有任何人動過目標集合：這份 snapshot 就是最新的權威。
                    if state.desired != panes {
                        state.desired = panes;
                        state.generation += 1;
                    }
                } else {
                    tracing::debug!(
                        runtime = %self.id,
                        "snapshot 回應比期間觀察到的 pane 事件舊，保留事件算出的目標集合"
                    );
                }
                if state.desired != state.current {
                    schedule_reopen(&self.status, &mut state);
                }
            }
        }

        Ok(snapshot)
    }

    /// 建立合併事件流（spec「建立事件流的順序」）：重置 S 管理器 → 探測 → seed → L → S →
    /// 合併流。失敗時已開的長連線在回傳前關閉。
    async fn subscribe(&self) -> Result<RuntimeEvents, RuntimeError> {
        // (a) 先重置 S 管理器：清掉上一輪的清單、abort 上一輪留下的所有 task。
        self.status().reset();

        // (b) `wsl` 型 runtime 先探測；失敗直接回傳，不開任何連線（spec「WSL 探測」）。
        // 探測器回什麼錯誤都正規化成 `Unavailable`，並換成本 runtime 設定的固定重試間隔
        // （`WslProbe::retry_after`，來自 `wsl_probe_secs`）：重試節奏是 runtime 的責任，
        // 不能交給任意 `DistroProber` 實作去保證（Codex review round 1 finding 2）。
        if let Some(wsl) = &self.wsl
            && let Err(e) = wsl.prober.probe(&wsl.distro).await
        {
            return Err(RuntimeError::Unavailable {
                reason: e.to_string(),
                retry_after: wsl.retry_after,
            });
        }

        // (c) seed：短連線取 pane id 清單，**不**套用到狀態庫。
        let seed = self
            .client
            .request(SessionSnapshotRequest {})
            .await
            .map_err(|e| RuntimeError::Failed(format!("seed snapshot 失敗：{e}")))?;
        let pane_ids: BTreeSet<String> = seed
            .snapshot
            .panes
            .iter()
            .map(|pane| pane.pane_id.clone())
            .collect();

        // (d) L：24 種生命週期事件的長連線。
        let lifecycle = self
            .client
            .subscribe(&Subscription::all_lifecycle())
            .await
            .map_err(|e| RuntimeError::Failed(format!("L 訂閱建立失敗：{e}")))?;

        // (e) S：seed 有 pane 才開；失敗時先關掉 L 再回傳（spec「任一步失敗……已開的連線
        // 關閉」）。
        let status_stream = if pane_ids.is_empty() {
            None
        } else {
            match self.client.subscribe(&pane_subscriptions(&pane_ids)).await {
                Ok(stream) => Some(stream),
                Err(e) => {
                    drop(lifecycle);
                    return Err(RuntimeError::Failed(format!("S 訂閱建立失敗：{e}")));
                }
            }
        };

        // (f) 合併流：L／S 各一個 reader task 送進同一個 mpsc（design D2）。
        let (tx, unstarted) = RuntimeEvents::channel();
        let shutdown = Arc::new(Shutdown::default());
        let session = Arc::new(Session {
            runtime_id: self.id.clone(),
            client: Arc::clone(&self.client),
            tx: tx.downgrade(),
            shutdown: Arc::clone(&shutdown),
        });

        // **先**把本輪的 session 交給 S 管理器再 spawn reader：L reader 一開跑就可能收到
        // `pane_created`，那時 `session` 必須已經在位，否則這次觸發會被當成「沒有活著的
        // 事件流」而靜默丟掉（多執行緒 runtime 下 spawn 出去的 task 可能立刻在別的 worker
        // 上跑起來）。`handle` 等 spawn 完才填得進去，重開時用 `replace` 覆蓋。
        {
            let mut state = self.status();
            state.current = pane_ids.clone();
            state.desired = pane_ids;
            state.reopen_scheduled = false;
            state.session = Some(Arc::clone(&session));
        }

        let mut tasks: Vec<JoinHandle<()>> = Vec::new();
        let status_handle;
        {
            // 先上鎖再 spawn：reader task 結束時要 abort「另一條」，必須確定清單已經登記完整
            // 才動手。兩個 `AbortHandle` 都 push 完才解鎖，先跑完的一方會在 abort 前擋在這把
            // 鎖上——這段持鎖期間沒有任何 `await`，不會 deadlock。
            let mut registry = shutdown
                .aborts
                .lock()
                .expect("Shutdown.aborts mutex poisoned");

            let lifecycle_task = tokio::spawn(lifecycle_loop(
                lifecycle,
                tx.clone(),
                Arc::clone(&shutdown),
                self.id.clone(),
                Arc::clone(&self.status),
            ));
            registry.push(lifecycle_task.abort_handle());
            tasks.push(lifecycle_task);

            status_handle = status_stream.map(|stream| {
                let status_task = tokio::spawn(read_loop(
                    stream,
                    tx.clone(),
                    "S",
                    Arc::clone(&shutdown),
                    self.id.clone(),
                    None,
                ));
                let handle = status_task.abort_handle();
                registry.push(status_task.abort_handle());
                tasks.push(status_task);
                handle
            });
        }
        // 只有 reader task 手上的 clone 能送事件；這裡這一份要丟掉，否則兩條 reader task 都
        // 結束之後 `RuntimeEvents::next()` 也等不到 `None`。
        drop(tx);

        if let Some(initial) = status_handle {
            let stale = {
                let mut state = self.status();
                install_initial_handle(&mut state, &session, initial)
            };
            if let Some(stale) = stale {
                stale.abort();
            }
        }

        Ok(unstarted.start(tasks))
    }
}

/// 把 `subscribe()` 開好的**初始** S reader 的 `AbortHandle` 安裝進 S 管理器
/// （Codex 最終 review finding 1）。
///
/// 回傳「呼叫端應該 abort 掉的那一條」：`None` 代表安裝成功，`Some(handle)` 代表這次
/// 不安裝、那條 reader 該被收掉。呼叫端必須已經持有管理器的鎖，安裝的決策才會與重開
/// task 的提交序列化。
///
/// 為什麼不能無條件寫回：`subscribe()` 是先 spawn L 與初始 S reader、**最後**才安裝
/// handle。L 一開跑就可能收到 `pane_created` → `schedule_reopen` → 200 ms 去抖動 →
/// `reopen_after_debounce` 用 `state.handle.replace(new)` 提交新一代 S。若 `subscribe()`
/// 這個 task 在 spawn 之後、安裝之前被排程延遲超過 200 ms，無條件寫回就會用初始 S 的
/// handle 蓋掉新一代的：新一代那條 reader 還在跑（它登記在 `Shutdown` registry 裡）卻
/// 失去了被 `replace()` abort 的路徑，於是兩條 S 同時送 `pane.agent_status_changed`
/// （重複事件、投影 version 與 recent_events 噪音），而之後被 abort 的反倒是初始那條。
///
/// 規則：
///
/// - 不是本輪（期間整個 `reset()` 過）→ 不碰新一輪的狀態，把初始 S 交回去 abort。
/// - `handle` 已經是 `Some` → 那是較新一代提交的，不覆寫，把初始 S 交回去 abort。
/// - `handle` 仍是 `None` → 安裝。即使期間 `generation` 變過也安全：去抖動 task 醒來後
///   會依最新的 `desired` 重開，並 `replace` 掉這次安裝的 handle（連帶 abort 它）。
fn install_initial_handle(
    state: &mut StatusSubscription,
    session: &Arc<Session>,
    initial: AbortHandle,
) -> Option<AbortHandle> {
    if !state.is_current_round(session) || state.handle.is_some() {
        return Some(initial);
    }
    state.handle = Some(initial);
    None
}

/// 把 pane id 集合換成 S 的訂閱清單。
fn pane_subscriptions(pane_ids: &BTreeSet<String>) -> Vec<Subscription> {
    pane_ids
        .iter()
        .map(|pane_id| Subscription::PaneAgentStatusChanged {
            pane_id: pane_id.clone(),
        })
        .collect()
}

/// L 這條連線的 reader task：本體是共用的 [`read_loop`]，額外掛一個 [`AbortAllOnDrop`]，
/// 讓「事件流被釋放 → L 被 abort」連帶收掉本輪後來才 spawn 的 task（見 `AbortAllOnDrop`）。
async fn lifecycle_loop(
    stream: EventStream,
    tx: Sender<Result<RuntimeEvent, RuntimeError>>,
    shutdown: Arc<Shutdown>,
    runtime_id: RuntimeId,
    status: Arc<Mutex<StatusSubscription>>,
) {
    let _guard = AbortAllOnDrop(Arc::clone(&shutdown));
    read_loop(stream, tx, "L", shutdown, runtime_id, Some(status)).await;
}

/// 一條長連線的 reader task：翻譯後送進共用的 mpsc；連線結束或 I/O 錯誤時送一個指名是
/// `L` 還是 `S` 的錯誤項（spec「合併事件流與結束」），然後結束。
///
/// `status` 只有 L 會帶（`Some`）：翻譯**之前**先看這筆事件是不是改變了 pane 集合
/// （`pane_created`／`pane_closed`／`pane_moved`），是的話更新 S 管理器的目標集合並排程一次
/// 重開（design D10：pane id 在翻譯前就能從 payload 拿到）。
///
/// **錯誤項只會有一個**：收尾前先向共用的 [`Shutdown`] 搶資格（一次性旗標），搶輸的一方
/// 直接結束、不送任何東西——L 與 S 幾乎同時 EOF 時，消費端因此仍然只看到一個錯誤項
/// （Codex review round 1 finding 1）。搶贏的一方送完錯誤項後 abort 所有 task，
/// sender 全部消失，`RuntimeEvents::next()` 於是在錯誤項之後回 `None`。
///
/// 翻譯成 `None` 的事件（未知事件名稱、change 1 不訂閱的每 pane 事件）直接略過，不中斷流；
/// 壞行由 `herdr-client` 自己跳過，這裡看不到。
async fn read_loop(
    mut stream: EventStream,
    tx: Sender<Result<RuntimeEvent, RuntimeError>>,
    label: &'static str,
    shutdown: Arc<Shutdown>,
    runtime_id: RuntimeId,
    status: Option<Arc<Mutex<StatusSubscription>>>,
) {
    let reason = loop {
        match stream.next().await {
            Some(Ok(event)) => {
                if let Some(status) = status.as_ref() {
                    note_pane_membership(status, &event);
                }
                if let Some(translated) = translate::event(&event, SystemTime::now())
                    && tx.send(Ok(translated)).await.is_err()
                {
                    // 接收端已經不在了（`RuntimeEvents` 被 drop，它的 `AbortOnDrop` 正在
                    // 收拾所有 reader task）：沒有人要收錯誤項，直接收工。
                    return;
                }
            }
            Some(Err(e)) => {
                tracing::warn!(
                    runtime = %runtime_id,
                    connection = label,
                    error = %e,
                    "事件連線發生 I/O 錯誤，事件流結束"
                );
                break format!("{label} 連線錯誤：{e}");
            }
            None => {
                tracing::warn!(
                    runtime = %runtime_id,
                    connection = label,
                    "事件連線結束，事件流結束"
                );
                break format!("{label} 連線結束");
            }
        }
    };

    if shutdown.claim() {
        let _ = tx.send(Err(RuntimeError::Failed(reason))).await;
        shutdown.abort_all();
    }
}

/// 一筆生命週期事件對 pane 集合的影響（design D10：只有這三種事件會改變集合）。
enum PaneMembershipChange {
    Added(String),
    Removed(String),
    Moved { previous: String, current: String },
}

/// 看一筆 L 的事件有沒有改變 pane 集合；有的話更新 S 管理器的 `desired` 並排程一次重開。
///
/// payload 解析失敗時什麼都不做（這筆事件會由 `translate::event` 翻成 `Drift`，由驅動器
/// 決定要不要重拿 snapshot；重拿的 `snapshot()` 會再把正確的集合交給管理器）。
fn note_pane_membership(status: &Arc<Mutex<StatusSubscription>>, event: &IncomingEvent) {
    let IncomingEvent::Lifecycle(kind, data) = event else {
        return;
    };
    // 只有這三種事件會改變 pane 集合；其餘 23 種生命週期事件與每 pane 事件都不影響。
    let change = match kind {
        EventKind::PaneCreated => serde_json::from_value::<PanePayload>(data.clone())
            .ok()
            .map(|payload| PaneMembershipChange::Added(payload.pane.pane_id)),
        EventKind::PaneClosed => serde_json::from_value::<PaneClosedPayload>(data.clone())
            .ok()
            .map(|payload| PaneMembershipChange::Removed(payload.pane_id)),
        EventKind::PaneMoved => serde_json::from_value::<PaneMovedPayload>(data.clone())
            .ok()
            .map(|payload| PaneMembershipChange::Moved {
                previous: payload.previous_pane_id,
                current: payload.pane.pane_id,
            }),
        _ => None,
    };
    let Some(change) = change else {
        return;
    };

    let mut state = lock_status(status);
    match change {
        PaneMembershipChange::Added(pane_id) => {
            state.desired.insert(pane_id);
        }
        PaneMembershipChange::Removed(pane_id) => {
            state.desired.remove(&pane_id);
        }
        PaneMembershipChange::Moved { previous, current } => {
            state.desired.remove(&previous);
            state.desired.insert(current);
        }
    }
    // 目標集合換版：正在飛的 `snapshot()` 回應與重開 task 都得知道自己拿的是舊的
    // （Codex review round 1 findings 1、2）。即使集合內容沒變（例如重複的
    // `pane_created`）也照樣 +1——寧可讓那些非同步工作重算一次，也不要漏掉一次更新。
    state.generation += 1;
    schedule_reopen(status, &mut state);
}

/// 排程一次去抖動的重開（spec：200 ms 內的多次觸發合併成一次）。
///
/// 呼叫端必須已經持有管理器的鎖（`state`），這裡不再自己上鎖——避免「放掉鎖、再上鎖」之間
/// 多一個觸發插隊。已經有重開在跑時什麼都不做（那個 task 會在醒來時讀到最新的 `desired`，
/// 而且它在提交／放棄之後會自己補排一次）；沒有活著的事件流（`session` 為 `None`）時也不
/// 排程。
///
/// `reopen_scheduled` 涵蓋「去抖動 + 正在開新 S」整段（Codex review round 1 finding 2），所以
/// 同一輪同一時間最多只有一個重開在跑——兩個重開並行、慢的那個用舊集合蓋掉快的那個，這種
/// 交錯從結構上就不會發生。
///
/// 持鎖期間沒有 await：`tokio::spawn` 與 `Shutdown::register` 都是同步呼叫。
fn schedule_reopen(status: &Arc<Mutex<StatusSubscription>>, state: &mut StatusSubscription) {
    if state.reopen_scheduled {
        return;
    }
    let Some(session) = state.session.clone() else {
        return;
    };
    state.reopen_scheduled = true;
    let task = tokio::spawn(reopen_after_debounce(
        Arc::clone(status),
        Arc::clone(&session),
    ));
    session.shutdown.register(task.abort_handle());
}

/// 去抖動之後真正重開 S（spec「pane 集合改變時重開狀態訂閱」；design D10）。
///
/// 1. 睡 200 ms，讓這段期間的其他觸發併進同一次重開；
/// 2. 持鎖讀最新的 `desired` 與它的 `generation`——不是本輪就直接收工；與 `current` 相同就
///    不用動；為空就只 abort 舊 S、不開新 S（這三種情況都在這裡清掉 `reopen_scheduled`）；
/// 3. 否則先開新 S（`Client::subscribe` 讀到 `subscription_started` 才返回）；
/// 4. **提交前再持鎖確認兩件事**（Codex review round 1 findings 2、3）：還是本輪
///    （`Arc::ptr_eq` 比對 session），而且 `generation` 沒變（期間沒有更新的目標集合）。
///    過時就把剛開好的連線整個丟掉（不 spawn reader、不換 handle、不寫 `current`），清掉
///    旗標之後再排一次——讓新的目標集合用新的一次重開來套，而不是讓舊結果蓋上去；
/// 5. 通過檢查才 spawn 新 reader、換 handle、寫 `current`，**之後**才 abort 舊 S（spike 3
///    驗證重疊不丟事件）；提交完若 `desired` 又跟 `current` 不同（期間集合再度改變）就補排
///    一次；
/// 6. 新 S 建立失敗：不在原地重試，走一次性旗標往合併流送一個錯誤項並收尾，讓驅動器整輪
///    重來。
async fn reopen_after_debounce(status: Arc<Mutex<StatusSubscription>>, session: Arc<Session>) {
    tokio::time::sleep(REOPEN_DEBOUNCE).await;

    let (desired, generation) = {
        let mut state = lock_status(&status);
        if !state.is_current_round(&session) {
            // 已經換過一輪（`reset()`）：這一輪的旗標與 handle 都不是我能動的。
            return;
        }
        if state.desired == state.current {
            state.reopen_scheduled = false;
            return;
        }
        if state.desired.is_empty() {
            if let Some(handle) = state.handle.take() {
                handle.abort();
            }
            state.current.clear();
            state.reopen_scheduled = false;
            tracing::debug!(
                runtime = %session.runtime_id,
                "新的 pane 集合是空的，只關掉舊的狀態訂閱"
            );
            return;
        }
        (state.desired.clone(), state.generation)
    };

    match session
        .client
        .subscribe(&pane_subscriptions(&desired))
        .await
    {
        Ok(stream) => {
            let Some(tx) = session.tx.upgrade() else {
                // 本輪事件流已經結束（沒有任何強 sender 了），這條新連線隨 `stream` 一起丟掉。
                return;
            };

            let previous = {
                let mut state = lock_status(&status);
                if !state.is_current_round(&session) {
                    // 期間整個換了一輪：這條新連線隨 `stream` 一起丟掉，也不要碰新一輪的狀態。
                    return;
                }
                if state.generation != generation {
                    // 期間目標集合又變了：我手上這條訂閱已經過時，整條丟掉並重排一次。
                    tracing::debug!(
                        runtime = %session.runtime_id,
                        "重開期間目標集合又變了，丟掉這次剛開好的訂閱並重排"
                    );
                    state.reopen_scheduled = false;
                    schedule_reopen(&status, &mut state);
                    return;
                }

                let task = tokio::spawn(read_loop(
                    stream,
                    tx,
                    "S",
                    Arc::clone(&session.shutdown),
                    session.runtime_id.clone(),
                    None,
                ));
                session.shutdown.register(task.abort_handle());
                let previous = state.handle.replace(task.abort_handle());
                state.current = desired;
                state.reopen_scheduled = false;
                if state.desired != state.current {
                    schedule_reopen(&status, &mut state);
                }
                previous
            };
            // 新 S 已經收到 `subscription_started`、reader 也開始跑了，這時才關舊的。
            if let Some(previous) = previous {
                previous.abort();
            }
        }
        Err(e) => {
            tracing::warn!(
                runtime = %session.runtime_id,
                error = %e,
                "重開狀態訂閱失敗，事件流結束"
            );
            if session.shutdown.claim() {
                if let Some(tx) = session.tx.upgrade() {
                    let _ = tx
                        .send(Err(RuntimeError::Failed(format!("S 重開失敗：{e}"))))
                        .await;
                }
                session.shutdown.abort_all();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use herdr_client::connector::{ConnectError, NdjsonStream};

    use super::*;

    /// Codex review round 1 finding 3：`abort_all()` 掃描之後才 `register` 的 task 也必須被
    /// 收掉。這條窗口在整合測試裡撞不到（要剛好卡在「一個正在提交的重開 task 已經被掃描
    /// 過、但還沒 spawn 出新 reader」那幾行同步程式碼之間，中間沒有 await 點可以介入），
    /// 所以直接對 `Shutdown` 這個型別驗它的封閉性質。
    #[tokio::test]
    async fn abort_all_closes_registry_so_late_handles_are_aborted() {
        let shutdown = Shutdown::default();
        shutdown.abort_all();

        let late = tokio::spawn(std::future::pending::<()>());
        shutdown.register(late.abort_handle());

        let outcome = tokio::time::timeout(Duration::from_secs(5), late)
            .await
            .expect("收尾之後才登記的 task 應該當場被 abort，不該一直跑下去");
        assert!(
            outcome
                .expect_err("這個 task 只會以被取消的形式結束")
                .is_cancelled(),
            "晚到的 task 應該是被 abort 掉的"
        );
    }

    // -----------------------------------------------------------------------
    // Codex 最終 review finding 1：`subscribe()` 最後安裝初始 S handle 時，可能蓋掉
    // 一個「較新一代已經提交」的 handle。
    //
    // 為什麼用單元測試而不是整合測試：這條競態要求「L 推的 pane 事件 → 200 ms 去抖動 →
    // 重開提交」整段跑完，都還早於 `subscribe()` 那幾行同步程式碼跑到安裝那一步。
    // `subscribe()` 是先 spawn reader 再安裝，中間沒有 await 點可以從外部插隊；要撐開它
    // 只能在產品程式碼裡埋測試專用的 hook，而專案規則禁止把測試專用程式碼放進產品型別。
    // 所以把「安裝」這個決策抽成純函數 [`install_initial_handle`]，直接構造
    // `StatusSubscription` 驗三種情形。
    // -----------------------------------------------------------------------

    /// 只為了組出一個 `Session` 的假 connector：`Client` 需要一個 connector，但這幾個
    /// 單元測試完全不連線，所以 `connect()` 一律失敗。
    struct NeverConnects;

    #[async_trait]
    impl Connector for NeverConnects {
        async fn connect(&self) -> Result<Box<dyn NdjsonStream>, ConnectError> {
            Err(ConnectError::ServerNotRunning {
                detail: "單元測試用的 connector，不會真的連線".to_string(),
            })
        }

        fn describe(&self) -> String {
            "never-connects".to_string()
        }
    }

    /// 組一個「本輪」的 `Session`，連同必須一起活著的強 sender（`Session.tx` 是
    /// `WeakSender`，強 sender 一被 drop 就 upgrade 不回來了）。
    fn test_session() -> (
        Arc<Session>,
        Sender<Result<RuntimeEvent, RuntimeError>>,
        Arc<Shutdown>,
    ) {
        let (tx, _unstarted) = RuntimeEvents::channel();
        let shutdown = Arc::new(Shutdown::default());
        let session = Arc::new(Session {
            runtime_id: RuntimeId::new("win"),
            client: Arc::new(Client::new(Arc::new(NeverConnects))),
            tx: tx.downgrade(),
            shutdown: Arc::clone(&shutdown),
        });
        (session, tx, shutdown)
    }

    /// 一條「永遠不會自己結束的 S reader」：回傳 `JoinHandle`（測試用來確認它是不是被
    /// abort 了）與對應的 `AbortHandle`（交給受測函數）。
    ///
    /// 不用 `AbortHandle::id()` 比對身分：`tokio::task::Id` 目前仍在 `tokio_unstable`
    /// 之後。改成「abort 某一個 handle，再看哪一條 task 被取消」來辨認身分。
    fn dummy_reader() -> (JoinHandle<()>, AbortHandle) {
        let task = tokio::spawn(std::future::pending::<()>());
        let handle = task.abort_handle();
        (task, handle)
    }

    /// 等一條 task 結束並斷言它是被 abort 掉的。
    async fn assert_cancelled(task: JoinHandle<()>, what: &str) {
        let outcome = tokio::time::timeout(Duration::from_secs(5), task)
            .await
            .unwrap_or_else(|_| panic!("{what} 應該在 5 秒內被取消，實際還在跑"));
        assert!(
            outcome
                .expect_err("這個 task 只會以被取消的形式結束")
                .is_cancelled(),
            "{what} 應該是被 abort 掉的"
        );
    }

    /// (a) 正常情形：還沒有任何 S handle → 安裝初始 handle，沒有東西要 abort。
    #[tokio::test]
    async fn install_initial_handle_installs_when_slot_is_empty() {
        let (session, _tx, _shutdown) = test_session();
        let mut state = StatusSubscription {
            session: Some(Arc::clone(&session)),
            ..StatusSubscription::default()
        };
        let (initial_task, initial) = dummy_reader();

        let stale = install_initial_handle(&mut state, &session, initial);

        assert!(
            stale.is_none(),
            "沒有較新一代的 handle 時不該有東西要 abort"
        );
        // 管理器裡那一條就是初始 S：abort 它，被取消的必須是初始 S 那條 task。
        state
            .handle
            .take()
            .expect("初始 S 的 handle 應該被安裝進管理器")
            .abort();
        assert_cancelled(initial_task, "管理器裡安裝的那條 reader").await;
    }

    /// (b) 競態情形：`subscribe()` 還沒安裝，較新一代的重開就已經提交了 handle
    /// → **不覆寫**，改成 abort 自己這條初始 S reader。
    ///
    /// 覆寫的話，新一代那條 S reader 還在跑（它登記在 `Shutdown` registry 裡）卻失去了被
    /// `replace()` abort 的路徑，於是兩條 S 同時送 `pane.agent_status_changed`。
    #[tokio::test]
    async fn install_initial_handle_does_not_overwrite_newer_generation() {
        let (session, _tx, _shutdown) = test_session();
        let (newer_task, newer) = dummy_reader();
        let mut state = StatusSubscription {
            session: Some(Arc::clone(&session)),
            handle: Some(newer),
            ..StatusSubscription::default()
        };
        let (initial_task, initial) = dummy_reader();

        let stale = install_initial_handle(&mut state, &session, initial);

        // 交回來要 abort 的必須是初始 S 那一條。
        stale
            .expect("較新一代已經提交時，初始 S 那一條要交回來 abort")
            .abort();
        assert_cancelled(initial_task, "初始 S 那條 reader").await;
        // 較新一代那條必須完好無損地還留在管理器裡（沒被蓋掉、也沒被連帶 abort）。
        assert!(
            !newer_task.is_finished(),
            "較新一代的 S reader 不該被收掉，它才是目前該送事件的那一條"
        );
        state
            .handle
            .take()
            .expect("管理器裡的 handle 不該被清掉")
            .abort();
        assert_cancelled(newer_task, "管理器裡留下的那條 reader（應是較新一代）").await;
    }

    /// (c) 整輪已經換掉（期間有人呼叫過 `subscribe()`／`reset()`）→ 不碰新一輪的狀態，
    /// 直接 abort 自己這條初始 S reader。
    #[tokio::test]
    async fn install_initial_handle_skips_when_round_changed() {
        let (session, _tx, _shutdown) = test_session();
        let (other_session, _other_tx, _other_shutdown) = test_session();
        let mut state = StatusSubscription {
            session: Some(Arc::clone(&other_session)),
            ..StatusSubscription::default()
        };
        let (initial_task, initial) = dummy_reader();

        let stale = install_initial_handle(&mut state, &session, initial);

        stale
            .expect("不是本輪時應該把初始 S 那一條交回去 abort")
            .abort();
        assert_cancelled(initial_task, "不是本輪時的初始 S reader").await;
        assert!(
            state.handle.is_none(),
            "不是本輪時不得碰新一輪的 handle 欄位"
        );
    }
}
