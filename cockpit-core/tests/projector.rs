//! RED→GREEN 驗收測試（Task 1.6）：`StoreHandle` 與投影任務的合併廣播、version 只在
//! 內容改變時遞增、新訂閱者立即取得現況。全部在 `tokio::time::pause()` 底下跑
//! （`#[tokio::test(start_paused = true)]`）。
//!
//! 計時策略：`tokio::time::advance()` 本身只是「把時鐘撥快＋yield_now 一次」，不保證
//! 被喚醒的任務已經跑完（見 tokio 原始碼 `time::clock::advance`）。要確定投影任務真的
//! 跑到「送出」或「判定不送」的終點，一律改用 `rx.changed()` 搭配
//! `tokio::time::timeout`：暫停的時鐘沒有其他任務可跑時會 auto-advance 到下一個
//! timer，逾時本身就是「這段時間內沒有廣播」的證據，不會真的等到真實時間流逝。
//!
//! `ten_changes_within_10ms_yield_at_most_two_broadcasts` 裡的 10 筆事件改用
//! `tokio::task::yield_now()` 交錯（不是 `tokio::time::advance`）：手動 `advance`
//! 交錯過會被 auto-advance 提早帶到 50 ms 合併視窗之後（實測過，即使實作正確也會讓
//! 10 筆各自形成一次廣播），`yield_now()` 只讓排程器跑一輪、不動時鐘，才能讓事件
//! 真的散在「投影任務已經進入合併 sleep」的期間，測到真正的合併行為。

mod common;

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, SystemTime};

use cockpit_core::{ConnectionState, RuntimeEvent, RuntimeStore, StoreHandle, spawn_projector};

use common::{empty_focused, pane, pane_id, runtime_id, snapshot, tab, workspace, workspace_id};

/// 給「應該收到廣播」的斷言一個寬裕但有限的虛擬時間上限，逾時代表實作有問題（例如
/// dirty 沒被 notify、或投影任務卡住），而不是真的等 5 秒。
const EXPECT_BROADCAST_TIMEOUT: Duration = Duration::from_secs(5);

/// GIVEN 已產生某個 version 的投影 WHEN 狀態庫沒有任何變動又觸發一次投影 THEN version
/// 不變，觀察者沒有收到新的一份。
#[tokio::test(start_paused = true)]
async fn unchanged_store_keeps_version_and_does_not_broadcast() {
    let store = RuntimeStore::new();
    let handle = StoreHandle::new(store);
    let mut rx = handle.subscribe();
    assert_eq!(
        handle.current().version,
        1,
        "空狀態庫的初始投影是 version 1"
    );

    let win = runtime_id("win");
    // register 一個 runtime 後才起投影任務：從空到有 runtime 是這個測試的第一筆變動。
    handle.register(win.clone(), "herdr".to_string(), "tcp://win".to_string());

    let projector = spawn_projector(handle.clone());

    tokio::time::timeout(EXPECT_BROADCAST_TIMEOUT, rx.changed())
        .await
        .expect("應在合理時間內收到廣播")
        .expect("channel 應仍存活");
    assert_eq!(
        rx.borrow().version,
        2,
        "從空到有 runtime 應該讓 version 變成 2"
    );

    // 對同一個 runtime 呼叫一個不改內容的操作：連線狀態設回它原本就有的 Connecting。
    handle
        .set_connection(&win, ConnectionState::Connecting)
        .expect("set_connection 應成功");

    // 給投影任務充分機會跑完這一輪（含 50 ms 合併視窗）：真的廣播了會在期限內 resolve
    // Ok；沒有廣播（預期行為）就會逾時——逾時本身就是「沒有廣播」的證據。
    let result = tokio::time::timeout(Duration::from_millis(500), rx.changed()).await;
    assert!(result.is_err(), "內容沒變不該廣播");
    assert_eq!(handle.current().version, 2, "version 應維持不變");

    projector.abort();
}

/// GIVEN 目前投影 WHEN 套用一筆改變 pane 狀態的事件 THEN 觀察者收到 version 恰好 +1 的
/// 完整投影，內容反映這筆變動。
#[tokio::test(start_paused = true)]
async fn changed_store_bumps_version_once() {
    let mut store = RuntimeStore::new();
    let win = runtime_id("win");
    store.register(win.clone(), "herdr".to_string(), "tcp://win".to_string());
    store
        .replace(
            &win,
            snapshot(
                vec![workspace("wJ", 1)],
                vec![tab("wJ:t1", "wJ", 1)],
                vec![pane("wJ:p1", "wJ", "wJ:t1")],
                vec![],
                empty_focused(),
            ),
        )
        .expect("replace 應成功");

    let handle = StoreHandle::new(store);
    let mut rx = handle.subscribe();
    let before_version = handle.current().version;

    let projector = spawn_projector(handle.clone());

    handle
        .apply(
            &win,
            RuntimeEvent::PaneExited(pane_id("wJ:p1")),
            SystemTime::now(),
        )
        .expect("apply 應成功");

    tokio::time::timeout(EXPECT_BROADCAST_TIMEOUT, rx.changed())
        .await
        .expect("應在合理時間內收到廣播")
        .expect("channel 應仍存活");

    let after = rx.borrow().clone();
    assert_eq!(
        after.version,
        before_version + 1,
        "只有一次變動，version 應剛好 +1"
    );
    let pane = &after.runtimes[0].workspaces[0].tabs[0].panes[0];
    assert!(pane.exited, "投影應反映 PaneExited");

    projector.abort();
}

/// GIVEN 時間可控 WHEN 在 10 ms 內套用 10 筆事件 THEN 觀察者最多收到 2 份廣播，最後一份
/// 反映全部 10 筆。
#[tokio::test(start_paused = true)]
async fn ten_changes_within_10ms_yield_at_most_two_broadcasts() {
    let mut store = RuntimeStore::new();
    let win = runtime_id("win");
    store.register(win.clone(), "herdr".to_string(), "tcp://win".to_string());
    store
        .replace(
            &win,
            snapshot(
                vec![workspace("wJ", 1)],
                vec![tab("wJ:t1", "wJ", 1)],
                vec![],
                vec![],
                empty_focused(),
            ),
        )
        .expect("replace 應成功");

    let handle = StoreHandle::new(store);

    // 用一個一直在背景跑的計數任務即時數廣播次數：`watch` 頻道只留「最新一份」，如果
    // 只在 10 筆事件都套用完之後才去看 receiver，中途（合併視窗失效時）發生的好幾次
    // `send` 會被頻道本身的「只留最新值」語意悄悄合併掉，量不出真正的送出次數。背景
    // 任務跟主測試的 `advance(1ms)` 交錯排程，才能在每次 `send` 發生的當下就數到。
    let mut count_rx = handle.subscribe();
    let broadcasts = Arc::new(AtomicUsize::new(0));
    let broadcasts_for_task = broadcasts.clone();
    let counting_task = tokio::spawn(async move {
        while count_rx.changed().await.is_ok() {
            broadcasts_for_task.fetch_add(1, Ordering::SeqCst);
        }
    });

    let mut settle_rx = handle.subscribe();
    let projector = spawn_projector(handle.clone());

    // 10 筆事件中間穿插 `yield_now()`（不是 `tokio::time::advance`）：純粹讓排程器有
    // 機會跑一輪，不會推進虛擬時鐘，所以不會誤觸發 auto-advance 把投影任務的 50 ms
    // sleep 提前跑完（實測過：改成 `tokio::time::advance(1ms)` 交錯，即使實作正確，
    // 時鐘還是會被 auto-advance 提早帶到 50 ms 之後，讓 10 筆各自形成一次廣播，反而
    // 測不出合併視窗真正的行為）。這樣能讓投影任務在套用到一半時就已經進入
    // `sleep(50ms)` 而不是等 10 筆全套用完才第一次被排程，才是「事件散在合併視窗
    // 期間」的真實情境。
    for i in 1..=10u32 {
        let pane_id_str = format!("wJ:p{i}");
        handle
            .apply(
                &win,
                RuntimeEvent::PaneUpserted(pane(&pane_id_str, "wJ", "wJ:t1")),
                SystemTime::now(),
            )
            .expect("apply 應成功");
        tokio::task::yield_now().await;
    }

    // 讓合併視窗（含可能的第二輪空轉）跑完，再讓計數任務有機會把它看到的變化都處理掉。
    let _ = tokio::time::timeout(Duration::from_millis(500), settle_rx.changed()).await;
    for _ in 0..20 {
        tokio::task::yield_now().await;
    }

    let final_state = handle.current();
    let panes = &final_state.runtimes[0].workspaces[0].tabs[0].panes;
    assert_eq!(panes.len(), 10, "最後一份應反映全部 10 筆變動");

    let count = broadcasts.load(Ordering::SeqCst);
    assert!(
        count <= 2,
        "10 ms 內 10 筆事件最多只該廣播 2 次，實際 {count}"
    );

    projector.abort();
    counting_task.abort();
}

/// GIVEN 目前投影已到達某個 version WHEN 新觀察者加入 THEN 立即取得該 version 的整份，
/// 不需要 await。
#[tokio::test(start_paused = true)]
async fn new_subscriber_gets_current_state_immediately() {
    let mut store = RuntimeStore::new();
    let win = runtime_id("win");
    store.register(win.clone(), "herdr".to_string(), "tcp://win".to_string());
    store
        .replace(
            &win,
            snapshot(
                vec![workspace("wJ", 1)],
                vec![],
                vec![],
                vec![],
                empty_focused(),
            ),
        )
        .expect("replace 應成功");

    let handle = StoreHandle::new(store);
    let mut rx0 = handle.subscribe();
    let projector = spawn_projector(handle.clone());

    handle
        .apply(
            &win,
            RuntimeEvent::WorkspaceRelabeled {
                id: workspace_id("wJ"),
                label: "hello".to_string(),
            },
            SystemTime::now(),
        )
        .expect("apply 應成功");

    tokio::time::timeout(EXPECT_BROADCAST_TIMEOUT, rx0.changed())
        .await
        .expect("應在合理時間內收到廣播")
        .expect("channel 應仍存活");

    let expected = handle.current();
    assert!(expected.version > 1, "應該已經廣播過至少一次");

    // 新訂閱者：只呼叫 subscribe()／borrow()，完全不 await。
    let new_rx = handle.subscribe();
    let snapshot_now = new_rx.borrow().clone();
    assert_eq!(snapshot_now.version, expected.version);
    assert_eq!(snapshot_now, expected);

    projector.abort();
}

/// GIVEN 目前沒有任何 receiver 存活（`StoreHandle::new` 內部的初始 receiver 已經在建構
/// 時就被丟棄，呼叫端也還沒 `subscribe()`）WHEN 狀態庫在這段期間發生變動 THEN 投影任務
/// 保留的最新值仍要更新；之後才 `subscribe()` 的新觀察者要立即拿到反映這些變動的最新
/// 投影，而不是陳舊的初值（review finding：`watch::Sender::send` 在零 receiver 時回傳
/// `Err` 且不更新頻道保留的值，被忽略掉的話會讓變動整個遺失）。
#[tokio::test(start_paused = true)]
async fn projection_survives_zero_receivers() {
    let store = RuntimeStore::new();
    let handle = StoreHandle::new(store);
    let initial_version = handle.current().version;

    let projector = spawn_projector(handle.clone());

    // 刻意不持有任何 receiver（`StoreHandle::new` 內部的初始 receiver 已經隨函式結束被
    // 丟棄，這裡也不呼叫 `subscribe()`），模擬「WebSocket 全部斷線期間狀態仍在變動」。
    let win = runtime_id("win");
    handle.register(win.clone(), "herdr".to_string(), "tcp://win".to_string());
    handle
        .apply(
            &win,
            RuntimeEvent::WorkspaceUpserted(workspace("wJ", 1)),
            SystemTime::now(),
        )
        .expect("apply 應成功");

    // 沒有任何 receiver 可以 `.changed()`，所以不能用其他測試那種「await changed()」的
    // 同步手法；改用 `sleep`（tokio 官方建議：確定所有更早註冊的 timer 依序觸發完，見
    // 檔頭說明）讓投影任務在零 receiver 期間把這一輪（含可能的第二輪空轉）跑完。
    tokio::time::sleep(Duration::from_millis(500)).await;

    // 這時才第一次 subscribe：應該立刻看到反映上面兩筆變動的最新投影，不是陳舊初值。
    let new_rx = handle.subscribe();
    let snapshot_now = new_rx.borrow().clone();
    assert!(
        snapshot_now.version > initial_version,
        "零 receiver 期間的變動不該被丟棄，version 應該前進"
    );
    assert_eq!(snapshot_now.runtimes[0].id, win);
    assert_eq!(
        snapshot_now.runtimes[0].workspaces[0].id,
        workspace_id("wJ")
    );

    let current = handle.current();
    assert_eq!(current, snapshot_now, "current() 應與新訂閱者看到的一致");

    projector.abort();
}
