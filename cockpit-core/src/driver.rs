//! 連線驅動器：把一個 [`AgentRuntime`] 與狀態庫接起來，負責與 runtime 種類無關的
//! 連線生命週期（design D1）。
//!
//! 完整的狀態流程（1.7 的連線生命週期 + 1.8 的重拿 + 1.9 的退避）：
//!
//! ```text
//! Connecting → subscribe() → 同時等 snapshot() 並丟棄期間到達的事件
//!            → replace() → Connected（退避序列歸零）→ 逐筆套用事件
//!                          ├─ apply 回 Err(Drift) → 立即重拿（進行中就不重複）
//!                          └─ 距上次成功 snapshot 滿 policy.resnapshot → 定期重拿
//!            → 事件流結束／錯誤／建立失敗／snapshot 失敗（含重拿失敗）
//!            → 釋放事件流 → Disconnected { reason, retry_in } → 等待 → Connecting
//! ```
//!
//! `retry_in` 的算法（spec「斷線、退避與固定間隔重試」、design D4）：錯誤自帶固定重試
//! 間隔（`RuntimeError::Unavailable` 的 `retry_after`，探測類錯誤用）就照它等，且**不**
//! 推進退避序列；否則取 `policy.backoff` 的下一項（用完最後一項就一直重複它）。三種
//! 斷線來源（建立事件流失敗、取得 snapshot 失敗、事件流結束或回錯誤）走的是同一套規則。
//!
//! 只透過 [`StoreHandle`] 的包裝（`replace`／`apply`／`set_connection`）寫狀態庫
//! （design D9），不直接碰 `RuntimeStore`。

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use tokio::sync::oneshot;
use tokio::time::Instant;

use crate::handle::StoreHandle;
use crate::runtime::{AgentRuntime, RuntimeError};
use crate::types::connection::ConnectionState;
use crate::types::ids::RuntimeId;
use crate::types::model::RuntimeSnapshot;

/// 事件流自然結束（`next()` 回 `None`）時寫進 `Disconnected.reason` 的原因；
/// 其餘情況一律用 `RuntimeError` 自己的 `Display`。
const STREAM_ENDED: &str = "事件流結束";

/// `Policy.backoff` 是空序列時的保底重試間隔。
///
/// 防禦用：`Policy::default()` 一定有六項，但 `backoff` 是公開欄位，呼叫端塞得進空
/// `Vec`。與其在那種情況下忙碌重連（等 0 秒），不如退回 1 秒（＝正常序列的第一項）。
const EMPTY_BACKOFF_RETRY: Duration = Duration::from_secs(1);

/// 驅動器的時間參數。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Policy {
    /// `Connected` 期間距離上一次成功取得 snapshot 多久之後要再重拿一次。
    pub resnapshot: Duration,
    /// 斷線重連的退避序列；用完最後一項就一直重複它。空序列退回
    /// [`EMPTY_BACKOFF_RETRY`]。
    pub backoff: Vec<Duration>,
}

impl Default for Policy {
    /// 定期重拿 30 秒；退避 1、2、4、8、16、30 秒（spec `runtime-driver`）。
    fn default() -> Self {
        Self {
            resnapshot: Duration::from_secs(30),
            backoff: [1, 2, 4, 8, 16, 30]
                .into_iter()
                .map(Duration::from_secs)
                .collect(),
        }
    }
}

/// 一個進行中的 `snapshot()` 呼叫。
///
/// `AgentRuntime` 是 `#[async_trait]` trait，`snapshot()` 本來就回傳 boxed future，
/// 這裡只是把那個型別命名下來，好放進 `Option` 跨迴圈保存——「重拿進行中不重複觸發」
/// 就是靠這個 `Option` 是不是 `Some`（design：有進行中的就不排隊）。
type PendingSnapshot<'a> =
    Pin<Box<dyn Future<Output = Result<RuntimeSnapshot, RuntimeError>> + Send + 'a>>;

/// 一輪連線（`subscribe` 到事件流結束）的結果。
enum Outcome {
    /// 收到停止指令：事件流已釋放，`run` 應直接返回，不改連線狀態。
    Stopped,
    /// 這輪連線結束了。
    Disconnected {
        /// 要寫進 `Disconnected.reason` 的原因。
        reason: String,
        /// 錯誤自帶的固定重試間隔（只有 `RuntimeError::Unavailable` 有）。
        /// `Some` 就照它等且不推進退避序列，`None` 走退避序列（design D4）。
        retry_after: Option<Duration>,
    },
}

impl Outcome {
    /// 由一個 `RuntimeError` 組出斷線結果：原因取它的 `Display`（`Unavailable` → reason、
    /// `Failed` → 字串），固定重試間隔取 [`RuntimeError::retry_after`]。
    fn from_error(err: RuntimeError) -> Self {
        Outcome::Disconnected {
            reason: err.to_string(),
            retry_after: err.retry_after(),
        }
    }

    /// 事件流自然結束（`next()` 回 `None`）：沒有錯誤物件，原因固定、走退避序列。
    fn stream_ended() -> Self {
        Outcome::Disconnected {
            reason: STREAM_ENDED.to_string(),
            retry_after: None,
        }
    }
}

/// 退避序列的第 `index` 項：超出長度就一直用最後一項；空序列退回
/// [`EMPTY_BACKOFF_RETRY`]。
fn backoff_at(policy: &Policy, index: usize) -> Duration {
    match policy.backoff.len() {
        0 => EMPTY_BACKOFF_RETRY,
        len => policy.backoff[index.min(len - 1)],
    }
}

/// 驅動一個 runtime 的連線直到收到停止指令。
///
/// `runtime` 必須已經由呼叫端 [`StoreHandle::register`] 登記過；`run` 不隱式登記
/// （未登記時每次寫入都會拿到 `Drift`，只會留下 `tracing::warn!`）。
///
/// `stop` 收到訊號**或它的 sender 被 drop**都算停止：立刻釋放目前的事件流並返回，
/// 不改動連線狀態（停止是呼叫端的決定，不是斷線）。
pub async fn run(
    runtime: Arc<dyn AgentRuntime>,
    store: StoreHandle,
    policy: Policy,
    stop: oneshot::Receiver<()>,
) {
    let id = runtime.id().clone();
    let mut stop = stop;
    // 退避序列目前走到第幾項：一般錯誤每次 +1，成功進入 `Connected` 由
    // `connect_and_serve` 歸零，`Unavailable` 的固定間隔完全不動它（design D4）。
    let mut backoff_index: usize = 0;

    loop {
        set_connection(&store, &id, ConnectionState::Connecting);

        let (reason, retry_after) = match connect_and_serve(
            runtime.as_ref(),
            &store,
            &id,
            &policy,
            &mut stop,
            &mut backoff_index,
        )
        .await
        {
            Outcome::Stopped => return,
            Outcome::Disconnected {
                reason,
                retry_after,
            } => (reason, retry_after),
        };

        // 走到這裡 `connect_and_serve` 已經返回，它持有的 `RuntimeEvents` 也已經 drop
        // ——事件流在寫入 `Disconnected` 之前就釋放了。
        let retry_in = match retry_after {
            // 探測類錯誤自帶固定重試間隔：照它等，退避序列原地不動。
            Some(fixed) => fixed,
            None => {
                let wait = backoff_at(&policy, backoff_index);
                backoff_index = backoff_index.saturating_add(1);
                wait
            }
        };
        set_connection(
            &store,
            &id,
            ConnectionState::Disconnected { reason, retry_in },
        );

        tokio::select! {
            biased;
            _ = &mut stop => return,
            _ = tokio::time::sleep(retry_in) => {}
        }
    }
}

/// 跑一輪連線：建立事件流 → 取得 snapshot（期間丟棄事件）→ `Connected` → 套用事件
/// 並在 Drift／到期時重拿 snapshot，直到收到停止指令或這條事件流結束。
///
/// 返回時 `events` 一定已經被 drop（`RuntimeEvents` 的 `Drop` 會 abort runtime 那側的
/// 背景任務），所以呼叫端不需要自己收拾。
///
/// `backoff_index` 由呼叫端持有；這裡只在成功進入 `Connected` 時把它歸零。
async fn connect_and_serve(
    runtime: &dyn AgentRuntime,
    store: &StoreHandle,
    id: &RuntimeId,
    policy: &Policy,
    stop: &mut oneshot::Receiver<()>,
    backoff_index: &mut usize,
) -> Outcome {
    let mut events = tokio::select! {
        biased;
        _ = &mut *stop => return Outcome::Stopped,
        result = runtime.subscribe() => match result {
            Ok(events) => events,
            Err(err) => return Outcome::from_error(err),
        },
    };

    // 事件流已經開著，現在才去要 snapshot：兩者同時等（`biased` 讓停止最優先、
    // snapshot 次之），snapshot 完成前到達的事件只計數，不套用、不記入最近事件
    // （spec「訂閱先開、snapshot 整份替換、之前的事件丟棄」）。
    let snapshot_fut = runtime.snapshot();
    tokio::pin!(snapshot_fut);
    let mut discarded: u64 = 0;
    let snapshot = loop {
        tokio::select! {
            biased;
            _ = &mut *stop => return Outcome::Stopped,
            result = &mut snapshot_fut => match result {
                Ok(snapshot) => break snapshot,
                Err(err) => return Outcome::from_error(err),
            },
            item = events.next() => match item {
                Some(Ok(_)) => discarded += 1,
                Some(Err(err)) => return Outcome::from_error(err),
                None => return Outcome::stream_ended(),
            },
        }
    };
    tracing::debug!(runtime = %id, discarded, "snapshot 前丟棄的事件數");

    // 這輪連線建立的時間：之後每次重拿都只更新 `last_snapshot_at`，`since` 一路沿用
    // 這個值（重拿不是重新連線）。
    let since = SystemTime::now();
    install_snapshot(store, id, snapshot, since);
    // 真的連上了：退避序列歸零，下一次斷線從序列的第一項重新起算（spec「成功後歸零」）。
    // 之後每次重拿都不會再動它——`Connected` 期間根本不讀退避。
    *backoff_index = 0;

    // 進行中的重拿（`None` = 沒有）與下一次定期重拿的時刻。兩者都跨迴圈保存：
    // `pending` 不能每輪重建（重建等於每輪都發一次新請求），`deadline` 是絕對時刻，
    // 只在成功取得 snapshot 時才往後推。
    let mut pending: Option<PendingSnapshot<'_>> = None;
    let mut deadline = Instant::now() + policy.resnapshot;

    loop {
        tokio::select! {
            biased;
            _ = &mut *stop => return Outcome::Stopped,
            // 進行中的重拿優先於計時器與事件：拿到結果就整份替換，並把定期重拿的
            // 計時器往後推（design：任何一次成功的 snapshot 都重設計時器）。
            result = await_pending(pending.as_mut()) => {
                pending = None;
                match result {
                    Ok(snapshot) => {
                        install_snapshot(store, id, snapshot, since);
                        deadline = Instant::now() + policy.resnapshot;
                    }
                    // 重拿失敗視同斷線（design）：交給呼叫端走 `Disconnected` 路徑。
                    Err(err) => return Outcome::from_error(err),
                }
            }
            // 到期就重拿；已經有進行中的就完全不排這個分支（否則 deadline 已過、
            // `sleep_until` 每輪立刻完成，會變成忙碌迴圈）。
            _ = tokio::time::sleep_until(deadline), if pending.is_none() => {
                pending = Some(runtime.snapshot());
            }
            item = events.next() => match item {
                Some(Ok(event)) => {
                    if let Err(drift) = store.apply(id, event, SystemTime::now()) {
                        // Drift 立即重拿；已經有進行中的就合併（不排隊、不重複觸發）。
                        // drift 本身已經由 `apply` 記進最近事件。
                        tracing::warn!(runtime = %id, %drift, "套用事件時偵測到 drift，重新取得 snapshot");
                        if pending.is_none() {
                            pending = Some(runtime.snapshot());
                        }
                    }
                }
                Some(Err(err)) => return Outcome::from_error(err),
                None => return Outcome::stream_ended(),
            },
        }
    }
}

/// 等一個「可能不存在」的重拿：`None` 時回傳一個永不完成的 future，讓 `select!` 這個
/// 分支等同不存在（不必用 `if` 前置條件，也就不會有「沒重拿時分支立刻完成」的忙碌迴圈）。
async fn await_pending(
    pending: Option<&mut PendingSnapshot<'_>>,
) -> Result<RuntimeSnapshot, RuntimeError> {
    match pending {
        Some(future) => future.await,
        None => std::future::pending().await,
    }
}

/// 整份替換狀態庫並寫回 `Connected`：初次連上與之後每一次重拿都走這裡。
///
/// `since` 由呼叫端保存（這輪連線建立的時間，重拿不改它）；`server_version`／
/// `protocol`／`protocol_warning` 一律取新 snapshot 的值，`last_snapshot_at` 更新為此刻。
fn install_snapshot(
    store: &StoreHandle,
    id: &RuntimeId,
    snapshot: RuntimeSnapshot,
    since: SystemTime,
) {
    // `replace` 會吃掉 snapshot，先把要寫進 `Connected` 的欄位留下來。
    let server_version = snapshot.server_version.clone();
    let protocol = snapshot.protocol;
    let protocol_warning = snapshot.protocol_warning.clone();
    if let Err(drift) = store.replace(id, snapshot) {
        tracing::warn!(runtime = %id, %drift, "整份替換狀態庫失敗");
    }
    set_connection(
        store,
        id,
        ConnectionState::Connected {
            since,
            server_version,
            protocol,
            last_snapshot_at: SystemTime::now(),
            protocol_warning,
        },
    );
}

/// 寫連線狀態；runtime 沒登記時只留下警告，不中斷驅動器。
fn set_connection(store: &StoreHandle, id: &RuntimeId, state: ConnectionState) {
    if let Err(drift) = store.set_connection(id, state) {
        tracing::warn!(runtime = %id, %drift, "設定連線狀態失敗");
    }
}
