//! RED→GREEN 驗收測試：連線驅動器的「訂閱先開、snapshot 前丟棄、可停止」（Task 1.7）
//! 與「Drift 立即重拿、定期重拿」（Task 1.8）。
//!
//! 全部在暫停時鐘底下跑（`#[tokio::test(start_paused = true)]`），每個可能卡住的
//! `await` 都包 `tokio::time::timeout`：暫停的時鐘在沒有任務可跑時會 auto-advance 到
//! 下一個 timer，逾時只消耗虛擬時間，不會真的等到實際時間流逝。
//!
//! 等待條件成立一律用 [`wait_until`]：先讓排程器跑幾圈（**不動時鐘**），條件還不成立
//! 才推進 1 ms 虛擬時間。順序很重要——先 yield 才推進，才不會在「事件已送進通道、
//! 驅動器還沒把它取出來」的空檔就被 auto-advance 帶過 snapshot 的延遲，測不到丟棄行為。

mod common;

use std::sync::Arc;
use std::time::{Duration, SystemTime};

use cockpit_core::driver::{Policy, run};
use cockpit_core::{
    AgentRuntime, AgentStatus, ConnectionState, RuntimeError, RuntimeEvent, RuntimeId,
    RuntimeSnapshot, RuntimeStore, StoreHandle,
};
use tokio::sync::oneshot;

use common::{
    Call, FakeRuntime, empty_focused, pane, pane_id, runtime_id, snapshot, tab, tab_id, workspace,
    workspace_id,
};

/// 給「應該會發生」的斷言一個寬裕但有限的虛擬時間上限；逾時代表實作有問題。
const EXPECT_TIMEOUT: Duration = Duration::from_secs(5);

/// [`wait_until`] 每輪推進的虛擬時間。
const STEP: Duration = Duration::from_millis(1);

/// [`wait_until`] 最多推進幾輪（= 2 s 虛擬時間）；超過就當作實作有問題。
const MAX_STEPS: usize = 2_000;

/// 暫停時鐘下等一個條件成立：每輪先讓排程器跑 8 圈（不動時鐘），條件還不成立才推進
/// [`STEP`] 虛擬時間。全程不會真的等待實際時間；超過 [`MAX_STEPS`] 就 panic，避免 hang。
async fn wait_until(mut cond: impl FnMut() -> bool, what: &str) {
    for _ in 0..MAX_STEPS {
        for _ in 0..8 {
            if cond() {
                return;
            }
            tokio::task::yield_now().await;
        }
        tokio::time::advance(STEP).await;
    }
    panic!("等不到：{what}");
}

/// 只讓排程器跑幾圈、完全不推進時鐘。用在「事件已經送進通道，要讓驅動器有機會把它
/// 取出來」這種不該讓時間前進的地方。
async fn spin(rounds: usize) {
    for _ in 0..rounds {
        tokio::task::yield_now().await;
    }
}

fn connection(store: &StoreHandle, id: &RuntimeId) -> ConnectionState {
    store.with_store(|s| s.connection(id).cloned().expect("runtime 應已登記"))
}

fn is_connected(store: &StoreHandle, id: &RuntimeId) -> bool {
    matches!(connection(store, id), ConnectionState::Connected { .. })
}

fn is_disconnected(store: &StoreHandle, id: &RuntimeId) -> bool {
    matches!(connection(store, id), ConnectionState::Disconnected { .. })
}

/// 登記好一個 runtime 的空狀態庫把手。
fn store_with(id: &RuntimeId) -> StoreHandle {
    let store = StoreHandle::new(RuntimeStore::new());
    store.register(id.clone(), "fake".to_string(), "test://fake".to_string());
    store
}

/// 一份含一個 workspace／tab／pane 的快照。
fn one_of_each() -> RuntimeSnapshot {
    snapshot(
        vec![workspace("wJ", 1)],
        vec![tab("wJ:t1", "wJ", 1)],
        vec![pane("wJ:p1", "wJ", "wJ:t1")],
        vec![],
        empty_focused(),
    )
}

/// GIVEN 假 runtime 記錄每次呼叫的順序 WHEN 驅動器啟動並進入 `Connected` THEN 呼叫順序
/// 為「建立事件流、取得 snapshot」，且狀態庫等於該 snapshot。
#[tokio::test(start_paused = true)]
async fn subscribes_before_snapshot() {
    let id = runtime_id("win");
    let fake = Arc::new(
        FakeRuntime::new("win").snapshot_responses(vec![(Ok(one_of_each()), Duration::ZERO)]),
    );
    let store = store_with(&id);

    let (stop_tx, stop_rx) = oneshot::channel();
    let runtime: Arc<dyn AgentRuntime> = fake.clone();
    let driver = tokio::spawn(run(runtime, store.clone(), Policy::default(), stop_rx));

    wait_until(|| is_connected(&store, &id), "驅動器進入 Connected").await;

    assert_eq!(
        fake.calls(),
        vec![Call::Subscribe, Call::Snapshot],
        "必須先建立事件流才取得 snapshot"
    );

    store.with_store(|s| {
        let state = s.state(&id).expect("runtime 應已登記");
        assert_eq!(state.workspaces.len(), 1, "狀態庫應等於快照的 workspace");
        assert!(state.workspaces.contains_key(&workspace_id("wJ")));
        assert_eq!(state.tabs.len(), 1, "狀態庫應等於快照的 tab");
        assert!(state.tabs.contains_key(&tab_id("wJ:t1")));
        assert_eq!(state.panes.len(), 1, "狀態庫應等於快照的 pane");
        assert!(state.panes.contains_key(&pane_id("wJ:p1")));
    });

    match connection(&store, &id) {
        ConnectionState::Connected {
            server_version,
            protocol,
            protocol_warning,
            ..
        } => {
            assert_eq!(server_version, "0.9.0", "server_version 應取自 snapshot");
            assert_eq!(protocol, 1, "protocol 應取自 snapshot");
            assert_eq!(protocol_warning, None);
        }
        other => panic!("應為 Connected，實際 {other:?}"),
    }

    drop(stop_tx);
    tokio::time::timeout(EXPECT_TIMEOUT, driver)
        .await
        .expect("停止後驅動器應在期限內結束")
        .expect("驅動器不應 panic");
}

/// GIVEN 假 runtime 在回應 snapshot 前先由事件流送出 3 筆指向不存在 pane 的事件
/// WHEN snapshot 完成 THEN snapshot 只被取得一次、狀態庫等於 snapshot 內容、最近事件為空。
#[tokio::test(start_paused = true)]
async fn events_before_authoritative_snapshot_are_dropped_and_not_recorded() {
    let id = runtime_id("win");
    let fake = Arc::new(
        FakeRuntime::new("win")
            .snapshot_responses(vec![(Ok(one_of_each()), Duration::from_millis(100))]),
    );
    let store = store_with(&id);

    let (stop_tx, stop_rx) = oneshot::channel();
    let runtime: Arc<dyn AgentRuntime> = fake.clone();
    let driver = tokio::spawn(run(runtime, store.clone(), Policy::default(), stop_rx));

    // 等到事件流已建立、snapshot 也已經呼叫（還卡在 100 ms 延遲裡）。這兩步不需要時間，
    // 所以 `wait_until` 會在第一輪 yield 就滿足，不會推進時鐘。
    wait_until(
        || fake.calls().len() == 2,
        "事件流建立與 snapshot 呼叫都已發生",
    )
    .await;
    assert!(
        !is_connected(&store, &id),
        "snapshot 還沒完成就不該 Connected"
    );

    // 指向不存在 pane 的事件：若被誤套用，`apply` 會回 `Err(Drift)` 並記一筆最近事件。
    for i in 0..3 {
        fake.push_event(Ok(RuntimeEvent::PaneRemoved(pane_id(&format!("ghost{i}")))));
    }
    // 不推進時鐘，只讓驅動器有機會把這 3 筆從通道取出（snapshot 仍未完成）。
    spin(20).await;

    tokio::time::advance(Duration::from_millis(100)).await;
    wait_until(|| is_connected(&store, &id), "驅動器進入 Connected").await;

    assert_eq!(
        fake.calls()
            .iter()
            .filter(|call| **call == Call::Snapshot)
            .count(),
        1,
        "snapshot 只該被取得一次"
    );

    store.with_store(|s| {
        let state = s.state(&id).expect("runtime 應已登記");
        assert_eq!(state.panes.len(), 1, "狀態庫應等於快照內容");
        assert!(state.panes.contains_key(&pane_id("wJ:p1")));
        assert_eq!(
            s.recent_events(&id).count(),
            0,
            "snapshot 前的事件不套用也不記入最近事件"
        );
    });

    drop(stop_tx);
    tokio::time::timeout(EXPECT_TIMEOUT, driver)
        .await
        .expect("停止後驅動器應在期限內結束")
        .expect("驅動器不應 panic");
}

/// GIVEN 驅動器 `Connected` WHEN 發出停止 THEN 假 runtime 觀察到事件流被釋放，
/// 之後不再有任何呼叫。
#[tokio::test(start_paused = true)]
async fn cancel_releases_stream_and_stops_calls() {
    let id = runtime_id("win");
    let fake = Arc::new(
        FakeRuntime::new("win").snapshot_responses(vec![(Ok(one_of_each()), Duration::ZERO)]),
    );
    let store = store_with(&id);

    let (stop_tx, stop_rx) = oneshot::channel();
    let runtime: Arc<dyn AgentRuntime> = fake.clone();
    let driver = tokio::spawn(run(runtime, store.clone(), Policy::default(), stop_rx));

    wait_until(|| is_connected(&store, &id), "驅動器進入 Connected").await;
    let calls_before = fake.calls().len();
    assert!(!fake.stream_released(0), "還沒停止，事件流不該被釋放");

    // sender 被 drop 也算停止訊號。
    drop(stop_tx);

    tokio::time::timeout(EXPECT_TIMEOUT, driver)
        .await
        .expect("停止後驅動器應在期限內結束")
        .expect("驅動器不應 panic");

    // `JoinHandle::abort()` 是非同步的：驅動器 drop 掉 `RuntimeEvents` 之後，被 abort
    // 的 reader task 還要等排程器跑一輪才會真的被丟掉、guard 才會翻旗標。
    let mut released = false;
    for _ in 0..100 {
        if fake.stream_released(0) {
            released = true;
            break;
        }
        tokio::task::yield_now().await;
    }
    assert!(released, "停止後假 runtime 應觀察到事件流被釋放");

    // 再放 5 秒虛擬時間，確認沒有任何重連或重拿。
    tokio::time::advance(Duration::from_secs(5)).await;
    spin(20).await;
    assert_eq!(
        fake.calls().len(),
        calls_before,
        "停止後不該再對 runtime 有任何呼叫"
    );
}

// ---------------------------------------------------------------------------
// Task 1.8：Drift 立即重拿與定期重拿
// ---------------------------------------------------------------------------

/// 另一份與 [`one_of_each`] 主體完全不同（wK 系列）的快照：用來分辨「狀態庫真的整份
/// 換成新 snapshot」，而不是原地沒動。
fn other_of_each() -> RuntimeSnapshot {
    snapshot(
        vec![workspace("wK", 2)],
        vec![tab("wK:t1", "wK", 1)],
        vec![pane("wK:p1", "wK", "wK:t1")],
        vec![],
        empty_focused(),
    )
}

/// 目前為止 `snapshot()` 被呼叫過幾次。
fn snapshot_calls(fake: &FakeRuntime) -> usize {
    fake.calls()
        .iter()
        .filter(|call| **call == Call::Snapshot)
        .count()
}

/// 狀態庫裡有沒有這個 pane。
fn has_pane(store: &StoreHandle, id: &RuntimeId, pane: &str) -> bool {
    store.with_store(|s| {
        s.state(id)
            .expect("runtime 應已登記")
            .panes
            .contains_key(&pane_id(pane))
    })
}

/// 狀態庫裡某個 pane 目前的 agent 狀態。
fn pane_status(store: &StoreHandle, id: &RuntimeId, pane: &str) -> AgentStatus {
    store.with_store(|s| {
        s.state(id)
            .expect("runtime 應已登記")
            .panes
            .get(&pane_id(pane))
            .expect("pane 應存在")
            .agent_status
    })
}

/// 一筆一定會造成 Drift 的事件：移除一個不存在的 pane。
fn drift_event(ghost: &str) -> RuntimeEvent {
    RuntimeEvent::PaneRemoved(pane_id(ghost))
}

/// 從 `Connected` 取出 `since` 與 `last_snapshot_at`。
fn connected_times(store: &StoreHandle, id: &RuntimeId) -> (SystemTime, SystemTime) {
    match connection(store, id) {
        ConnectionState::Connected {
            since,
            last_snapshot_at,
            ..
        } => (since, last_snapshot_at),
        other => panic!("應為 Connected，實際 {other:?}"),
    }
}

/// GIVEN 驅動器已 `Connected` WHEN 事件流送出一筆會造成 Drift 的事件 THEN 假 runtime
/// 觀察到 snapshot 又被取得一次，狀態庫等於新 snapshot（`since` 不變、
/// `last_snapshot_at` 更新）。
#[tokio::test(start_paused = true)]
async fn drift_triggers_one_resnapshot() {
    let id = runtime_id("win");
    let fake = Arc::new(FakeRuntime::new("win").snapshot_responses(vec![
        (Ok(one_of_each()), Duration::ZERO),
        (Ok(other_of_each()), Duration::ZERO),
    ]));
    let store = store_with(&id);

    let (stop_tx, stop_rx) = oneshot::channel();
    let runtime: Arc<dyn AgentRuntime> = fake.clone();
    let driver = tokio::spawn(run(runtime, store.clone(), Policy::default(), stop_rx));

    wait_until(|| is_connected(&store, &id), "驅動器進入 Connected").await;
    assert_eq!(snapshot_calls(&fake), 1, "初次連線只該取得一次 snapshot");
    let (since_before, last_snapshot_before) = connected_times(&store, &id);

    fake.push_event(Ok(drift_event("ghost")));
    wait_until(
        || has_pane(&store, &id, "wK:p1"),
        "Drift 觸發的重拿完成、狀態庫換成新 snapshot",
    )
    .await;

    assert_eq!(snapshot_calls(&fake), 2, "Drift 應剛好多取得一次 snapshot");
    store.with_store(|s| {
        let state = s.state(&id).expect("runtime 應已登記");
        assert_eq!(state.panes.len(), 1, "狀態庫應整份換成新 snapshot");
        assert!(
            !state.panes.contains_key(&pane_id("wJ:p1")),
            "舊 snapshot 的 pane 不該留著"
        );
        assert!(
            state.workspaces.contains_key(&workspace_id("wK")),
            "狀態庫應等於新 snapshot"
        );
    });

    let (since_after, last_snapshot_after) = connected_times(&store, &id);
    assert_eq!(
        since_after, since_before,
        "重拿不是重新連線，since 不該被改掉"
    );
    assert!(
        last_snapshot_after > last_snapshot_before,
        "重拿成功應更新 last_snapshot_at"
    );

    drop(stop_tx);
    tokio::time::timeout(EXPECT_TIMEOUT, driver)
        .await
        .expect("停止後驅動器應在期限內結束")
        .expect("驅動器不應 panic");
}

/// GIVEN 假 runtime 讓第二次 snapshot 回應延遲 100 ms WHEN 延遲期間連續送出 3 筆造成
/// Drift 的事件 THEN 只多取得一次 snapshot。
#[tokio::test(start_paused = true)]
async fn drifts_during_pending_resnapshot_coalesce() {
    let id = runtime_id("win");
    let fake = Arc::new(FakeRuntime::new("win").snapshot_responses(vec![
        (Ok(one_of_each()), Duration::ZERO),
        (Ok(other_of_each()), Duration::from_millis(100)),
    ]));
    let store = store_with(&id);

    let (stop_tx, stop_rx) = oneshot::channel();
    let runtime: Arc<dyn AgentRuntime> = fake.clone();
    let driver = tokio::spawn(run(runtime, store.clone(), Policy::default(), stop_rx));

    wait_until(|| is_connected(&store, &id), "驅動器進入 Connected").await;
    assert_eq!(snapshot_calls(&fake), 1, "初次連線只該取得一次 snapshot");

    // 三筆 Drift，每筆之間先讓驅動器有機會取走（spin）再推進 10 ms——加起來 30 ms，
    // 遠小於第二次 snapshot 的 100 ms 延遲，所以三筆都落在「重拿進行中」的視窗裡。
    for i in 0..3 {
        fake.push_event(Ok(drift_event(&format!("ghost{i}"))));
        spin(20).await;
        tokio::time::advance(Duration::from_millis(10)).await;
    }
    assert_eq!(
        snapshot_calls(&fake),
        2,
        "重拿進行中再遇到 Drift 不該重複觸發"
    );

    // 推進到第二次 snapshot 的回應完成。
    tokio::time::advance(Duration::from_millis(100)).await;
    wait_until(
        || has_pane(&store, &id, "wK:p1"),
        "進行中的重拿完成、狀態庫換成新 snapshot",
    )
    .await;

    assert_eq!(
        snapshot_calls(&fake),
        2,
        "3 筆 Drift 合併後 snapshot 總共只該被取得 2 次"
    );

    drop(stop_tx);
    tokio::time::timeout(EXPECT_TIMEOUT, driver)
        .await
        .expect("停止後驅動器應在期限內結束")
        .expect("驅動器不應 panic");
}

/// GIVEN 假 runtime 讓 Drift 觸發的 snapshot 回應延遲 WHEN 延遲期間送出一筆合法的
/// `AgentStatusChanged` THEN 替換前狀態庫已反映該事件；替換完成後狀態庫等於新 snapshot。
#[tokio::test(start_paused = true)]
async fn events_during_resnapshot_are_applied_then_overwritten_by_snapshot() {
    let id = runtime_id("win");
    // 兩份快照的 wJ:p1 都是 Idle：中途那筆事件把它改成 Working，替換完成後又回到 Idle。
    let fake = Arc::new(FakeRuntime::new("win").snapshot_responses(vec![
        (Ok(one_of_each()), Duration::ZERO),
        (Ok(one_of_each()), Duration::from_millis(100)),
    ]));
    let store = store_with(&id);

    let (stop_tx, stop_rx) = oneshot::channel();
    let runtime: Arc<dyn AgentRuntime> = fake.clone();
    let driver = tokio::spawn(run(runtime, store.clone(), Policy::default(), stop_rx));

    wait_until(|| is_connected(&store, &id), "驅動器進入 Connected").await;
    assert_eq!(
        pane_status(&store, &id, "wJ:p1"),
        AgentStatus::Idle,
        "初始 snapshot 的 pane 應為 Idle"
    );

    // 先讓 Drift 觸發重拿（第二次 snapshot 卡在 100 ms 延遲裡）。
    fake.push_event(Ok(drift_event("ghost")));
    spin(20).await;
    assert_eq!(snapshot_calls(&fake), 2, "Drift 應已觸發重拿");

    // 重拿進行中到達的合法事件：完全不推進時鐘，確保它落在延遲視窗內。
    fake.push_event(Ok(RuntimeEvent::AgentStatusChanged {
        pane_id: pane_id("wJ:p1"),
        status: AgentStatus::Working,
        title: None,
        agent: None,
    }));
    spin(20).await;
    assert_eq!(
        pane_status(&store, &id, "wJ:p1"),
        AgentStatus::Working,
        "重拿進行中到達的事件要照常套用（替換前已反映）"
    );

    tokio::time::advance(Duration::from_millis(100)).await;
    wait_until(
        || pane_status(&store, &id, "wJ:p1") == AgentStatus::Idle,
        "重拿完成後狀態庫以 snapshot 為準",
    )
    .await;
    assert_eq!(snapshot_calls(&fake), 2, "只該多取得一次 snapshot");

    drop(stop_tx);
    tokio::time::timeout(EXPECT_TIMEOUT, driver)
        .await
        .expect("停止後驅動器應在期限內結束")
        .expect("驅動器不應 panic");
}

/// GIVEN resnapshot 間隔 30 秒、驅動器 `Connected`、時間可控 WHEN 第 20 秒因 Drift 重拿
/// 一次 THEN 第 30 秒沒有再取得 snapshot，到第 50 秒才再取得一次。
#[tokio::test(start_paused = true)]
async fn periodic_resnapshot_fires_and_resets_after_drift() {
    let id = runtime_id("win");
    // 佇列只放一筆：`FakeRuntime` 用完最後一筆之後就一直重複它（每次都回同一份 A）。
    let fake = Arc::new(
        FakeRuntime::new("win").snapshot_responses(vec![(Ok(one_of_each()), Duration::ZERO)]),
    );
    let store = store_with(&id);
    let policy = Policy {
        resnapshot: Duration::from_secs(30),
        ..Policy::default()
    };

    let (stop_tx, stop_rx) = oneshot::channel();
    let runtime: Arc<dyn AgentRuntime> = fake.clone();
    let driver = tokio::spawn(run(runtime, store.clone(), policy, stop_rx));

    wait_until(|| is_connected(&store, &id), "驅動器進入 Connected").await;
    assert_eq!(snapshot_calls(&fake), 1, "初次連線只該取得一次 snapshot");

    // t = 20 s：還沒到期。
    tokio::time::advance(Duration::from_secs(20)).await;
    spin(20).await;
    assert_eq!(snapshot_calls(&fake), 1, "第 20 秒還沒到定期重拿的時間");

    // t = 20 s：Drift 重拿一次，順帶重設計時器。
    fake.push_event(Ok(drift_event("ghost")));
    spin(20).await;
    assert_eq!(snapshot_calls(&fake), 2, "Drift 應觸發一次重拿");

    // t = 30 s：原本的計時器若沒被重設，這裡就會多出一次。
    tokio::time::advance(Duration::from_secs(10)).await;
    spin(20).await;
    assert_eq!(
        snapshot_calls(&fake),
        2,
        "Drift 重拿已重設計時器，第 30 秒不該再取得 snapshot"
    );

    // t = 50 s：距離上次成功重拿滿 30 秒。
    tokio::time::advance(Duration::from_secs(20)).await;
    spin(20).await;
    assert_eq!(snapshot_calls(&fake), 3, "第 50 秒應定期重拿一次");

    drop(stop_tx);
    tokio::time::timeout(EXPECT_TIMEOUT, driver)
        .await
        .expect("停止後驅動器應在期限內結束")
        .expect("驅動器不應 panic");
}

/// GIVEN 驅動器一直連不上（`subscribe` 永遠失敗）WHEN 經過 35 秒 THEN 這段期間完全沒有
/// 取得 snapshot，連線狀態仍是 `Disconnected`。
#[tokio::test(start_paused = true)]
async fn no_resnapshot_while_disconnected() {
    let id = runtime_id("win");
    let fake = Arc::new(
        FakeRuntime::new("win")
            .snapshot_responses(vec![(Ok(one_of_each()), Duration::ZERO)])
            .subscribe_responses(vec![Err(RuntimeError::Failed("連不上".to_string()))]),
    );
    let store = store_with(&id);

    let (stop_tx, stop_rx) = oneshot::channel();
    let runtime: Arc<dyn AgentRuntime> = fake.clone();
    let driver = tokio::spawn(run(runtime, store.clone(), Policy::default(), stop_rx));

    wait_until(|| is_disconnected(&store, &id), "驅動器進入 Disconnected").await;

    // 超過一個 resnapshot 間隔（30 s）的等待：斷線期間不該有任何定期重拿。
    tokio::time::advance(Duration::from_secs(35)).await;
    spin(20).await;

    assert_eq!(
        snapshot_calls(&fake),
        0,
        "連都沒連上，斷線期間不該取得任何 snapshot"
    );
    wait_until(|| is_disconnected(&store, &id), "驅動器仍為 Disconnected").await;

    drop(stop_tx);
    tokio::time::timeout(EXPECT_TIMEOUT, driver)
        .await
        .expect("停止後驅動器應在期限內結束")
        .expect("驅動器不應 panic");
}

// ---------------------------------------------------------------------------
// Task 1.9：斷線、退避與固定間隔重試
// ---------------------------------------------------------------------------

/// 目前為止 `subscribe()` 被呼叫過幾次（一次呼叫 = 一輪連線嘗試）。
fn subscribe_calls(fake: &FakeRuntime) -> usize {
    fake.calls()
        .iter()
        .filter(|call| **call == Call::Subscribe)
        .count()
}

/// 取出目前 `Disconnected` 的原因與重試秒數；不是 `Disconnected` 就 panic。
fn disconnected_parts(store: &StoreHandle, id: &RuntimeId) -> (String, Duration) {
    match connection(store, id) {
        ConnectionState::Disconnected { reason, retry_in } => (reason, retry_in),
        other => panic!("應為 Disconnected，實際 {other:?}"),
    }
}

/// 等第 `round` 輪連線嘗試失敗後寫進 `Disconnected` 的原因與重試秒數。
///
/// 連線失敗本身不花虛擬時間，所以 `wait_until`（先 yield 才推進時鐘）在這裡不會動到
/// 時鐘：讀到的就是驅動器這一輪算出來的值。呼叫端接著自己 `advance(retry_in)` 進下一
/// 輪——**不能一次 advance 一大段**，暫停時鐘下那只會把驅動器喚醒一次，數不出輪數。
async fn disconnected_after_attempt(
    store: &StoreHandle,
    id: &RuntimeId,
    fake: &FakeRuntime,
    round: usize,
) -> (String, Duration) {
    wait_until(
        || subscribe_calls(fake) >= round && is_disconnected(store, id),
        &format!("第 {round} 輪連線嘗試失敗後進入 Disconnected"),
    )
    .await;
    disconnected_parts(store, id)
}

/// 秒數陣列轉 `Vec<Duration>`，好跟收集到的重試秒數直接比。
fn secs(values: &[u64]) -> Vec<Duration> {
    values.iter().copied().map(Duration::from_secs).collect()
}

/// GIVEN 假 runtime 連續讓建立事件流失敗 6 次、時間可控 WHEN 觀察每次 `Disconnected` 的
/// 重試秒數 THEN 依序為 1、2、4、8、16、30；再多跑一次仍是 30。
#[tokio::test(start_paused = true)]
async fn backoff_sequence_1_2_4_8_16_30() {
    let id = runtime_id("win");
    let fake = Arc::new(
        FakeRuntime::new("win")
            .subscribe_responses(vec![Err(RuntimeError::Failed("boom".to_string()))]),
    );
    let store = store_with(&id);

    let (stop_tx, stop_rx) = oneshot::channel();
    let runtime: Arc<dyn AgentRuntime> = fake.clone();
    let driver = tokio::spawn(run(runtime, store.clone(), Policy::default(), stop_rx));

    let mut seen = Vec::new();
    for round in 1..=6 {
        let (_, retry_in) = disconnected_after_attempt(&store, &id, &fake, round).await;
        seen.push(retry_in);
        tokio::time::advance(retry_in).await;
    }
    assert_eq!(
        seen,
        secs(&[1, 2, 4, 8, 16, 30]),
        "退避序列應依序為 1、2、4、8、16、30 秒"
    );

    let (_, seventh) = disconnected_after_attempt(&store, &id, &fake, 7).await;
    assert_eq!(
        seventh,
        Duration::from_secs(30),
        "序列用完最後一項後應一直重複它"
    );

    drop(stop_tx);
    tokio::time::timeout(EXPECT_TIMEOUT, driver)
        .await
        .expect("停止後驅動器應在期限內結束")
        .expect("驅動器不應 panic");
}

/// GIVEN 已失敗 3 次（1、2、4 秒）後第 4 次成功 `Connected`，之後事件流結束
/// WHEN 下一次 `Disconnected` THEN 重試秒數回到 1。
#[tokio::test(start_paused = true)]
async fn backoff_resets_after_connected() {
    let id = runtime_id("win");
    let fake = Arc::new(
        FakeRuntime::new("win")
            .snapshot_responses(vec![(Ok(one_of_each()), Duration::ZERO)])
            .subscribe_responses(vec![
                Err(RuntimeError::Failed("boom".to_string())),
                Err(RuntimeError::Failed("boom".to_string())),
                Err(RuntimeError::Failed("boom".to_string())),
                Ok(()),
            ]),
    );
    let store = store_with(&id);

    let (stop_tx, stop_rx) = oneshot::channel();
    let runtime: Arc<dyn AgentRuntime> = fake.clone();
    let driver = tokio::spawn(run(runtime, store.clone(), Policy::default(), stop_rx));

    let mut seen = Vec::new();
    for round in 1..=3 {
        let (_, retry_in) = disconnected_after_attempt(&store, &id, &fake, round).await;
        seen.push(retry_in);
        tokio::time::advance(retry_in).await;
    }
    assert_eq!(seen, secs(&[1, 2, 4]), "前三次失敗應照退避序列");

    wait_until(|| is_connected(&store, &id), "第 4 輪連線成功").await;

    fake.end_stream();
    wait_until(
        || is_disconnected(&store, &id),
        "事件流結束後進入 Disconnected",
    )
    .await;

    let (reason, retry_in) = disconnected_parts(&store, &id);
    assert_eq!(
        retry_in,
        Duration::from_secs(1),
        "成功進入 Connected 之後退避序列應歸零"
    );
    assert_eq!(reason, "事件流結束", "事件流正常結束用固定描述");

    drop(stop_tx);
    tokio::time::timeout(EXPECT_TIMEOUT, driver)
        .await
        .expect("停止後驅動器應在期限內結束")
        .expect("驅動器不應 panic");
}

/// GIVEN 假 runtime 連續回「附固定間隔 60 秒」的探測類錯誤 3 次，接著回一般錯誤
/// WHEN 觀察四次 `Disconnected` THEN 前三次重試秒數都是 60（原因為該錯誤的說明），
/// 第四次是 1（固定間隔沒有推進退避序列）。
#[tokio::test(start_paused = true)]
async fn unavailable_uses_fixed_interval_without_advancing_backoff() {
    const NOT_RUNNING: &str = "WSL 發行版 Ubuntu-24.04 未啟動";
    let unavailable = || RuntimeError::Unavailable {
        reason: NOT_RUNNING.to_string(),
        retry_after: Duration::from_secs(60),
    };

    let id = runtime_id("win");
    let fake = Arc::new(FakeRuntime::new("win").subscribe_responses(vec![
        Err(unavailable()),
        Err(unavailable()),
        Err(unavailable()),
        Err(RuntimeError::Failed("x".to_string())),
    ]));
    let store = store_with(&id);

    let (stop_tx, stop_rx) = oneshot::channel();
    let runtime: Arc<dyn AgentRuntime> = fake.clone();
    let driver = tokio::spawn(run(runtime, store.clone(), Policy::default(), stop_rx));

    let mut seen = Vec::new();
    for round in 1..=4 {
        let (reason, retry_in) = disconnected_after_attempt(&store, &id, &fake, round).await;
        if round <= 3 {
            assert_eq!(
                reason, NOT_RUNNING,
                "探測類錯誤的說明應原樣寫進 Disconnected.reason"
            );
        }
        seen.push(retry_in);
        tokio::time::advance(retry_in).await;
    }

    assert_eq!(
        seen,
        secs(&[60, 60, 60, 1]),
        "固定間隔照等且不推進退避，之後的一般錯誤仍從 1 秒起算"
    );

    drop(stop_tx);
    tokio::time::timeout(EXPECT_TIMEOUT, driver)
        .await
        .expect("停止後驅動器應在期限內結束")
        .expect("驅動器不應 panic");
}

/// GIVEN 驅動器 `Connected` WHEN 事件流回傳原因含 `L closed` 的錯誤項 THEN 進入
/// `Disconnected`、原因含 `L closed`，且事件流已釋放。
#[tokio::test(start_paused = true)]
async fn stream_error_reason_appears_in_disconnected() {
    let id = runtime_id("win");
    let fake = Arc::new(
        FakeRuntime::new("win").snapshot_responses(vec![(Ok(one_of_each()), Duration::ZERO)]),
    );
    let store = store_with(&id);

    let (stop_tx, stop_rx) = oneshot::channel();
    let runtime: Arc<dyn AgentRuntime> = fake.clone();
    let driver = tokio::spawn(run(runtime, store.clone(), Policy::default(), stop_rx));

    wait_until(|| is_connected(&store, &id), "驅動器進入 Connected").await;

    fake.push_event(Err(RuntimeError::Failed("L closed".to_string())));
    // 不推進時鐘，只讓驅動器把這筆錯誤取出來。
    spin(20).await;

    let (reason, retry_in) = disconnected_parts(&store, &id);
    assert!(
        reason.contains("L closed"),
        "斷線原因應含事件流錯誤的字串，實際為 {reason}"
    );
    assert_eq!(
        retry_in,
        Duration::from_secs(1),
        "連上過之後的第一次斷線從 1 秒起算"
    );
    wait_until(|| fake.stream_released(0), "事件流已釋放").await;

    drop(stop_tx);
    tokio::time::timeout(EXPECT_TIMEOUT, driver)
        .await
        .expect("停止後驅動器應在期限內結束")
        .expect("驅動器不應 panic");
}

/// GIVEN 事件流建立成功 WHEN `snapshot()` 回 `Failed("boom")` THEN 進入 `Disconnected`、
/// 原因含 `boom`、事件流已釋放，之後依退避重來（再建立一次事件流）。
#[tokio::test(start_paused = true)]
async fn snapshot_failure_is_disconnected() {
    let id = runtime_id("win");
    let fake = Arc::new(
        FakeRuntime::new("win")
            .subscribe_responses(vec![Ok(())])
            .snapshot_responses(vec![(
                Err(RuntimeError::Failed("boom".to_string())),
                Duration::ZERO,
            )]),
    );
    let store = store_with(&id);

    let (stop_tx, stop_rx) = oneshot::channel();
    let runtime: Arc<dyn AgentRuntime> = fake.clone();
    let driver = tokio::spawn(run(runtime, store.clone(), Policy::default(), stop_rx));

    wait_until(
        || is_disconnected(&store, &id),
        "snapshot 失敗後進入 Disconnected",
    )
    .await;

    let (reason, retry_in) = disconnected_parts(&store, &id);
    assert!(
        reason.contains("boom"),
        "斷線原因應含 snapshot 的錯誤字串，實際為 {reason}"
    );
    assert_eq!(
        retry_in,
        Duration::from_secs(1),
        "snapshot 失敗也走一般退避序列"
    );
    wait_until(|| fake.stream_released(0), "事件流已釋放").await;

    tokio::time::advance(Duration::from_secs(1)).await;
    wait_until(
        || subscribe_calls(&fake) >= 2,
        "等滿重試秒數後再建立一次事件流",
    )
    .await;
    let calls = fake.calls();
    assert_eq!(
        calls[..3],
        [Call::Subscribe, Call::Snapshot, Call::Subscribe],
        "依退避重來時應重新建立事件流"
    );

    drop(stop_tx);
    tokio::time::timeout(EXPECT_TIMEOUT, driver)
        .await
        .expect("停止後驅動器應在期限內結束")
        .expect("驅動器不應 panic");
}
