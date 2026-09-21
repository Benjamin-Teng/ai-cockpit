//! Task 2.2 驗收測試：`AgentRuntime::read_output`（spec `runtime-model`「假 runtime
//! 回應輸出」「讀取輸出不動狀態庫」）。

mod common;

use std::time::{Duration, SystemTime};

use cockpit_core::{
    AgentRuntime, OutputFormat, RuntimeError, RuntimeEvent, RuntimeStore, StoreHandle,
    spawn_projector,
};

use common::{FakeRuntime, empty_focused, pane, pane_id, runtime_id, snapshot, tab, workspace};

/// GIVEN 一個對 pane `p1` 回應三行文字、對其他 pane 回應「pane 不存在」的假 runtime
/// WHEN 分別讀取 `p1` 與 `p9` 的輸出
/// THEN 前者得到那三行、格式為純文字；後者得到可與其他失敗區分的「pane 不存在」錯誤。
#[tokio::test]
async fn fake_runtime_read_output() {
    let target = pane_id("p1");
    let fake = FakeRuntime::new("win").pane_output(target.clone(), "one\ntwo\nthree");

    let output = fake
        .read_output(&target, 200)
        .await
        .expect("讀取已腳本化的 pane 應成功");
    assert_eq!(output.format, OutputFormat::Text);
    assert_eq!(output.text, "one\ntwo\nthree");
    assert!(!output.truncated, "沒有設定過 truncated，應為 false");

    let missing = pane_id("p9");
    let err = fake
        .read_output(&missing, 200)
        .await
        .expect_err("讀取沒設定過的 pane 應失敗");
    match err {
        RuntimeError::PaneNotFound { pane_id } => {
            assert_eq!(pane_id, missing, "錯誤應帶著呼叫時傳入的 pane id");
        }
        other => panic!("預期 PaneNotFound，實際: {other:?}"),
    }
}

/// GIVEN 狀態庫已有某 runtime 的內容，投影 version 為 5
/// WHEN 讀取該 runtime 任一 pane 的輸出十次
/// THEN 投影 version 仍為 5。
///
/// `read_output` 是 `AgentRuntime` 上的方法，不經過 `StoreHandle`（design D2）；這裡實際
/// 建一份 `StoreHandle`＋投影任務、餵到 version 5，再對假 runtime 讀十次，確認投影完全沒被
/// 觸發（`rx.changed()` 在期限內逾時＝沒有廣播）。
#[tokio::test(start_paused = true)]
async fn read_output_does_not_bump_projection_version() {
    let mut store = RuntimeStore::new();
    let win = runtime_id("win");
    store.register(win.clone(), "fake".to_string(), "test://fake".to_string());
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
    let projector = spawn_projector(handle.clone());

    // 把投影餵到 version 5：初始是 1，四次 apply 各 +1（合併視窗之間讓排程器跑到廣播
    // 落地，避免多筆事件被合併視窗吃掉而少於預期次數）。
    for i in 0..4u32 {
        handle
            .apply(
                &win,
                RuntimeEvent::PaneUpserted(pane(&format!("wJ:p2{i}"), "wJ", "wJ:t1")),
                SystemTime::now(),
            )
            .expect("apply 應成功");
        tokio::time::timeout(Duration::from_secs(5), rx.changed())
            .await
            .expect("應在合理時間內收到廣播")
            .expect("channel 應仍存活");
    }
    let before_version = handle.current().version;
    assert_eq!(before_version, 5, "四次內容改變之後投影 version 應為 5");

    let fake = FakeRuntime::new("win").pane_output(pane_id("wJ:p1"), "hello");
    for _ in 0..10 {
        fake.read_output(&pane_id("wJ:p1"), 200)
            .await
            .expect("read_output 應成功");
    }

    // 給投影任務充分機會（若真的被 dirty 通知，合併視窗只有 50 ms）：逾時本身就是
    // 「沒有廣播」的證據。
    let result = tokio::time::timeout(Duration::from_millis(500), rx.changed()).await;
    assert!(result.is_err(), "讀取輸出不該觸發任何投影廣播");
    assert_eq!(
        handle.current().version,
        before_version,
        "投影 version 不該因為讀取輸出而改變"
    );

    projector.abort();
}
