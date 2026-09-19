//! `StoreHandle`：`RuntimeStore` 與 `DomainState` 的共享控制點，加一個 dirty `Notify` 與
//! `watch::Sender<Arc<ProjectedState>>`（design D9；spec `state-projection`；change
//! `pipeline-projection` design D2：兩層放在同一把鎖內，共用一個 version 序列與合併視窗）。驅動器
//! （1.7）只透過這裡的包裝寫入，不直接碰 `Mutex`；HTTP／WebSocket（3.3／3.4）只透過
//! [`StoreHandle::subscribe`] 讀。
//!
//! 臨界區（`store.lock()` 期間）只做純記憶體操作、不跨 `await`：用
//! `std::sync::Mutex` 而不是 tokio 的（design D9）。`lock().unwrap()`：poison 代表另一個
//! 持鎖者 panic 了，屬於 bug，不是可恢復的執行期狀況，讓它繼續 panic 比靜默吞掉更安全。

use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use tokio::sync::{Notify, mpsc, watch};
use tokio::task::JoinHandle;

use crate::domain::state::DomainState;
use crate::projection::{ProjectedState, StaleOverride, project, project_with_stale};
use crate::store::{Drift, RuntimeStore};
use crate::types::connection::ConnectionState;
use crate::types::events::RuntimeEvent;
use crate::types::ids::RuntimeId;
use crate::types::model::RuntimeSnapshot;

/// 投影任務把 50 ms 內的多次狀態變動合併成一次投影與廣播（spec「合併廣播」）。
const COALESCE_WINDOW: Duration = Duration::from_millis(50);

/// 同一把鎖保護的兩層狀態：投影在鎖內一次讀兩者，不會看到兩層不一致的瞬間（design D2）。
struct Layers {
    runtime: RuntimeStore,
    domain: DomainState,
}

struct Inner {
    store: Mutex<Layers>,
    dirty: Notify,
    tx: watch::Sender<Arc<ProjectedState>>,
}

/// `RuntimeStore` 的共享把手：`Clone` 便宜（內部 `Arc`），可以自由複製給驅動器、投影
/// 任務、測試各自持有。
#[derive(Clone)]
pub struct StoreHandle(Arc<Inner>);

impl StoreHandle {
    /// 用一份既有的 `RuntimeStore` 建立控制點：立刻算出第一份投影（version 1）當
    /// `watch` 頻道的初值，讓「新觀察者立即取得現況」從一開始就成立，不必等第一次
    /// dirty 通知。Domain 狀態為空（沒有任何 Project）。
    pub fn new(store: RuntimeStore) -> Self {
        Self::new_with_domain(store, DomainState::default())
    }

    /// 同 [`StoreHandle::new`]，但一開始就帶入 Domain 狀態：初始投影（version 1）已包含
    /// `projects`。
    pub fn new_with_domain(store: RuntimeStore, domain: DomainState) -> Self {
        let initial = project(&store, &domain, 1, SystemTime::now());
        let (tx, _rx) = watch::channel(Arc::new(initial));
        Self(Arc::new(Inner {
            store: Mutex::new(Layers {
                runtime: store,
                domain,
            }),
            dirty: Notify::new(),
            tx,
        }))
    }

    /// 訂閱投影廣播；新建立的 receiver 立刻能 `borrow()` 到目前這一份，不需要 await。
    pub fn subscribe(&self) -> watch::Receiver<Arc<ProjectedState>> {
        self.0.tx.subscribe()
    }

    /// 目前這一份投影（`tx.borrow()` 的複製，`Arc` clone 很便宜）。
    pub fn current(&self) -> Arc<ProjectedState> {
        self.0.tx.borrow().clone()
    }

    /// 登記一個 runtime（包裝 [`RuntimeStore::register`]）：鎖 → 呼叫 → 解鎖 → 通知投影
    /// 任務有髒資料。
    pub fn register(&self, id: RuntimeId, kind: String, endpoint: String) {
        {
            let mut store = self.0.store.lock().unwrap();
            store.runtime.register(id, kind, endpoint);
        }
        self.0.dirty.notify_one();
    }

    /// 整份取代一個 runtime 的狀態（包裝 [`RuntimeStore::replace`]）。失敗（`Drift`）也會
    /// 通知投影任務——drift 本身會被記進最近事件，屬於「內容變了」。
    pub fn replace(&self, id: &RuntimeId, snapshot: RuntimeSnapshot) -> Result<(), Drift> {
        let result = {
            let mut store = self.0.store.lock().unwrap();
            store.runtime.replace(id, snapshot)
        };
        self.0.dirty.notify_one();
        result
    }

    /// 逐筆套用一個事件（包裝 [`RuntimeStore::apply`]）。
    pub fn apply(&self, id: &RuntimeId, event: RuntimeEvent, now: SystemTime) -> Result<(), Drift> {
        let result = {
            let mut store = self.0.store.lock().unwrap();
            store.runtime.apply(id, event, now)
        };
        self.0.dirty.notify_one();
        result
    }

    /// 設定連線狀態（包裝 [`RuntimeStore::set_connection`]）。
    pub fn set_connection(&self, id: &RuntimeId, state: ConnectionState) -> Result<(), Drift> {
        let result = {
            let mut store = self.0.store.lock().unwrap();
            store.runtime.set_connection(id, state)
        };
        self.0.dirty.notify_one();
        result
    }

    /// 唯讀存取底層 `RuntimeStore`（驅動器讀取現況、測試組裝斷言用）；閉包執行期間持有
    /// 鎖，不得在裡面 `await`。
    pub fn with_store<R>(&self, f: impl FnOnce(&RuntimeStore) -> R) -> R {
        let store = self.0.store.lock().unwrap();
        f(&store.runtime)
    }

    /// 唯讀存取目前的 Domain 狀態（寫入服務讀現況再算新狀態用）；閉包執行期間持有鎖，不得
    /// 在裡面 `await`。
    pub fn with_domain<R>(&self, f: impl FnOnce(&DomainState) -> R) -> R {
        let store = self.0.store.lock().unwrap();
        f(&store.domain)
    }

    /// 整份取代 Domain 狀態並通知投影任務（design D2）。不在這裡比較新舊是否相等：與現況
    /// 相同時投影內容也相同，投影任務的 [`ProjectedState::content_eq`] 會判定不遞增 version、
    /// 不廣播（spec 「無效操作不遞增」）。
    pub fn set_domain(&self, domain: DomainState) {
        {
            let mut store = self.0.store.lock().unwrap();
            store.domain = domain;
        }
        self.0.dirty.notify_one();
    }
}

/// 起一個投影任務：等 dirty 通知 → 睡 [`COALESCE_WINDOW`]（合併這段期間的多次通知）→
/// 鎖 store 算一份新投影 → 與目前廣播的那份比較內容（忽略 `version`／`generated_at`，
/// [`ProjectedState::content_eq`]）→ 相等就什麼都不做，不等就送出整份新投影（design
/// D9；spec「version 只在內容改變時遞增」「合併廣播」）。
///
/// `next_version` 取自「目前廣播版本 + 1」，不另外用一個變數追蹤——內容沒變時就不會
/// 用到它、不會被浪費掉；內容有變時它就是真正要採用的新版本號。
pub fn spawn_projector(handle: StoreHandle) -> JoinHandle<()> {
    spawn_projector_inner(handle, None)
}

/// 同 [`spawn_projector`]，另外把失效覆蓋送到 `stale_tx`（design D3；接收端是寫入服務）。
///
/// - 用 `UnboundedSender`：送出不會因容量而丟失，也不會讓投影等寫入服務落檔。
/// - 每筆失效覆蓋只送一次：投影任務記住「已送出」的集合，每次投影只送集合裡沒有的新項目；
///   某筆已不在這次的失效清單（覆蓋被刪、被換掉、或 pane 回來了）就從集合移除，之後若再次
///   失效會再送。所以 channel 不會被同一筆重複塞滿，也不需要接收端回 ack。
/// - 接收端已關閉：`tracing::error!` 只記第一次（之後每次投影都會失敗，刷 log 沒有資訊量），
///   送不出去的項目不加入集合；投影與廣播照常進行、不 panic。
pub fn spawn_projector_with_stale_sink(
    handle: StoreHandle,
    stale_tx: mpsc::UnboundedSender<Vec<StaleOverride>>,
) -> JoinHandle<()> {
    spawn_projector_inner(handle, Some(stale_tx))
}

fn spawn_projector_inner(
    handle: StoreHandle,
    stale_tx: Option<mpsc::UnboundedSender<Vec<StaleOverride>>>,
) -> JoinHandle<()> {
    tokio::task::spawn(async move {
        // 已送出、且到上一次投影為止仍失效的覆蓋（清單很短，線性比對即可）。
        let mut sent_stale: Vec<StaleOverride> = Vec::new();
        let mut closed_logged = false;
        loop {
            handle.0.dirty.notified().await;
            tokio::time::sleep(COALESCE_WINDOW).await;

            let current = handle.current();
            let next_version = current.version + 1;
            let now = SystemTime::now();
            let (candidate, stale) = {
                let store = handle.0.store.lock().unwrap();
                project_with_stale(&store.runtime, &store.domain, next_version, now)
            };

            if !current.content_eq(&candidate) {
                // 用 `send_replace` 而不是 `send`：零 receiver 時 `send` 回傳 `Err` 且
                // 不會更新頻道保留的值，忽略掉這個錯誤會讓變動整個遺失——WebSocket 全部
                // 斷線期間狀態若繼續變動，`current()` 會卡在舊版本，之後新訂閱者
                // `subscribe()` 也會立即拿到陳舊投影，違反「新加入的觀察者立即取得目前
                // 的一份」（review finding）。`send_replace` 不管有沒有 receiver 都會
                // 更新保留值，所以 `current()`／`subscribe().borrow()` 一定反映最新內容。
                handle.0.tx.send_replace(Arc::new(candidate));
            }

            if let Some(tx) = &stale_tx {
                sent_stale.retain(|item| stale.contains(item));
                let fresh: Vec<StaleOverride> = stale
                    .into_iter()
                    .filter(|item| !sent_stale.contains(item))
                    .collect();
                if !fresh.is_empty() {
                    match tx.send(fresh.clone()) {
                        Ok(()) => sent_stale.extend(fresh),
                        Err(_) if !closed_logged => {
                            closed_logged = true;
                            tracing::error!(
                                count = fresh.len(),
                                "失效覆蓋的接收端已關閉，無法交付刪除；之後不再重複記錄"
                            );
                        }
                        Err(_) => {}
                    }
                }
            }
        }
    })
}
