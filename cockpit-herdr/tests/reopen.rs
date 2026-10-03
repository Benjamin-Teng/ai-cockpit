//! Task 2.6 驗收測試：pane 集合改變時重開 S 訂閱（200 ms 去抖動、先開新 S 再關舊 S）、
//! `snapshot()` 的協定版本警告、以及「只用唯讀 method」（spec `herdr-runtime-session`
//! 「pane 集合改變時重開狀態訂閱」「取得 snapshot 與版本警告」「只用唯讀 method」；
//! design D10、D12）。
//!
//! live-output task 3.4：「只用兩個 method」這個 Requirement 改名為「只用唯讀 method」
//! （多了 `pane.read`，但只在有人明確要求讀取 pane 輸出時才送出），本檔的 method 集合測試
//! 也延伸一個「另外讀取一次輸出 → 只多一筆 pane.read，沒有其他 method」的情境。
//!
//! 同 `tests/session.rs`：全部用真實 transport 的 `FakeHerdr`，所以一律 `#[tokio::test]`、
//! 不暫停時間；每個 `await` 都用 `tokio::time::timeout`／輪詢加逾時保護，卡住時以逾時失敗
//! 而不是掛死整個測試。

use std::collections::BTreeSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use cockpit_core::{
    AgentRuntime, Message, MessageCode, PaneId, RuntimeError, RuntimeEvent, RuntimeEvents,
    RuntimeId,
};
use cockpit_herdr::runtime::HerdrRuntime;
use herdr_client::connector::{ConnectError, Connector, NdjsonStream};
use herdr_client::testing::{FakeHerdr, FakeHerdrConfig, MethodResponse, Step, SubscribeMatcher};
use serde_json::{Value, json};

/// 每個 `await`／輪詢的保護逾時；本地全程都是程序內連線，正常情況遠低於這個值。
const TIMEOUT: Duration = Duration::from_secs(5);

/// 比 runtime 的 200 ms 去抖動明顯長：等過這段時間之後，「該重開的已經重開完」、
/// 「不該重開的也已經確定不會再重開」。
const AFTER_DEBOUNCE: Duration = Duration::from_millis(400);

/// L 腳本用的 `pane_created`：抄自 `herdr-client/tests/fixtures/events-lifecycle-p20.ndjson`
/// 的 `pane_created` 一行，只改 `pane_id`／`workspace_id`／`tab_id` 對齊本檔的 pane 命名。
fn pane_created(pane_id: &str) -> String {
    format!(
        r#"{{"data":{{"pane":{{"agent_status":"unknown","cwd":"/PROJECT_1","focused":false,"foreground_cwd":"/PROJECT_1","pane_id":"{pane_id}","revision":0,"scroll":{{"max_offset_from_bottom":0,"offset_from_bottom":0,"viewport_rows":39}},"tab_id":"wJ:t1","terminal_id":"term_65b59379e37dcc","workspace_id":"wJ"}},"type":"pane_created"}},"event":"pane_created"}}"#
    )
}

/// L 腳本用的 `pane_closed`：fixture 沒有這個事件，依 schema 的 `PaneClosedPayload`
/// （`pane_id`、`workspace_id` 兩個必填欄位）比照 `pane_created` 的信封組出來。
fn pane_closed(pane_id: &str) -> String {
    format!(
        r#"{{"data":{{"pane_id":"{pane_id}","type":"pane_closed","workspace_id":"wJ"}},"event":"pane_closed"}}"#
    )
}

/// S 腳本用的每 pane 事件：抄自 `herdr-client/tests/fixtures/events-status-p22.ndjson` 的一行，
/// 只改 `pane_id`／`workspace_id`。
fn status_changed(pane_id: &str) -> String {
    format!(
        r#"{{"data":{{"agent":"claude","agent_status":"done","pane_id":"{pane_id}","workspace_id":"wJ"}},"event":"pane.agent_status_changed"}}"#
    )
}

/// 組一個 `session.snapshot` 的成功 `result`（protocol 固定 22，也就是已測範圍內）。
fn snapshot_result(pane_ids: &[&str]) -> Value {
    snapshot_result_with_protocol(pane_ids, 22)
}

/// 組一個 `session.snapshot` 的成功 `result`，protocol 可指定（版本警告的測試用）。
fn snapshot_result_with_protocol(pane_ids: &[&str], protocol: u32) -> Value {
    let panes: Vec<Value> = pane_ids
        .iter()
        .enumerate()
        .map(|(i, id)| {
            json!({
                "pane_id": id,
                "workspace_id": "wJ",
                "tab_id": "wJ:t1",
                "agent": null,
                "agent_status": "unknown",
                "title": null,
                "terminal_title": null,
                "cwd": null,
                "label": null,
                "focused": i == 0,
                "revision": 1,
            })
        })
        .collect();
    json!({
        "type": "session_snapshot",
        "snapshot": {
            "version": "0.9.0",
            "protocol": protocol,
            "workspaces": [],
            "tabs": [],
            "panes": panes,
            "agents": [],
            "layouts": [],
        },
    })
}

/// 包住另一個 connector，讓第 n 次 `connect()`（0-based）先睡 `delays[n]` 再真的連線；
/// 超出 `delays` 長度的呼叫一律睡 `rest`。
///
/// 用途：把「新 S 正在建立」「snapshot 請求還在路上」這些平常只有幾微秒的窗口撐開到測試
/// 觀察得到（`reopen_keeps_old_s_open_until_new_one_started`、
/// `snapshot_response_older_than_l_event_does_not_revert_desired`、
/// `two_reopens_completing_out_of_order_keep_latest_set`、`reset_during_pending_reopen_aborts_it`）。
struct DelayedConnector {
    inner: Box<dyn Connector>,
    calls: AtomicUsize,
    delays: Vec<Duration>,
    rest: Duration,
}

impl DelayedConnector {
    fn new(inner: Box<dyn Connector>, delays: Vec<Duration>, rest: Duration) -> Self {
        Self {
            inner,
            calls: AtomicUsize::new(0),
            delays,
            rest,
        }
    }

    /// 前 `n` 次（建立事件流的 snapshot／L／S）不延遲，之後每次都延遲 `rest`。
    fn after(inner: Box<dyn Connector>, n: usize, rest: Duration) -> Self {
        Self::new(inner, vec![Duration::ZERO; n], rest)
    }
}

#[async_trait]
impl Connector for DelayedConnector {
    async fn connect(&self) -> Result<Box<dyn NdjsonStream>, ConnectError> {
        let call = self.calls.fetch_add(1, AtomicOrdering::SeqCst);
        let delay = self.delays.get(call).copied().unwrap_or(self.rest);
        if !delay.is_zero() {
            tokio::time::sleep(delay).await;
        }
        self.inner.connect().await
    }

    fn describe(&self) -> String {
        self.inner.describe()
    }
}

/// 指向假 HERDR 的 `HerdrRuntime`（`wsl` 為 `None`，也就是不探測的 `win` 型 runtime）。
fn runtime(fake: &FakeHerdr) -> HerdrRuntime {
    HerdrRuntime::new(RuntimeId::new("win"), Arc::from(fake.connector()), None)
}

/// 解析某條連線收到的第一行 request。
fn request_line(fake: &FakeHerdr, index: usize) -> Value {
    let received = fake.received();
    let lines = received
        .get(index)
        .unwrap_or_else(|| panic!("假 HERDR 應有第 {index} 條連線，實際: {received:?}"));
    let line = lines
        .first()
        .unwrap_or_else(|| panic!("第 {index} 條連線應收到一行 request，實際: {lines:?}"));
    serde_json::from_str(line)
        .unwrap_or_else(|e| panic!("第 {index} 條連線收到的行不是合法 JSON: {e}（{line}）"))
}

/// 取某條 `events.subscribe` request 的每 pane 訂閱 pane id 集合（順序不保證，用集合比對）。
fn subscribed_pane_ids(fake: &FakeHerdr, index: usize) -> BTreeSet<String> {
    let request = request_line(fake, index);
    assert_eq!(
        request["method"], "events.subscribe",
        "第 {index} 條連線應該是 events.subscribe，實際: {request}"
    );
    request["params"]["subscriptions"]
        .as_array()
        .unwrap_or_else(|| panic!("events.subscribe 應帶 subscriptions 陣列，實際: {request}"))
        .iter()
        .map(|s| {
            assert_eq!(
                s["type"], "pane.agent_status_changed",
                "S 的訂閱清單應全部是 pane.agent_status_changed，實際: {s}"
            );
            s["pane_id"]
                .as_str()
                .unwrap_or_else(|| panic!("每 pane 訂閱應帶 pane_id，實際: {s}"))
                .to_string()
        })
        .collect()
}

/// 把 `&[&str]` 轉成集合，供斷言比對。
fn ids(pane_ids: &[&str]) -> BTreeSet<String> {
    pane_ids.iter().map(|id| (*id).to_string()).collect()
}

/// 假 HERDR 目前為止收到的所有 request 用過哪些不重複的 method（spec「只用唯讀 method」）。
fn methods_used(fake: &FakeHerdr) -> BTreeSet<String> {
    fake.received()
        .iter()
        .flatten()
        .map(|line| {
            let request: Value = serde_json::from_str(line)
                .unwrap_or_else(|e| panic!("假 HERDR 收到的行不是合法 JSON: {e}（{line}）"));
            request["method"]
                .as_str()
                .unwrap_or_else(|| panic!("每一行 request 都應有 method，實際: {request}"))
                .to_string()
        })
        .collect()
}

/// 假 HERDR 目前為止收到的所有 request 中，`method` 為 `target` 的行數（累計，跨連線）。
fn method_call_count(fake: &FakeHerdr, target: &str) -> usize {
    fake.received()
        .iter()
        .flatten()
        .filter(|line| {
            let request: Value = serde_json::from_str(line)
                .unwrap_or_else(|e| panic!("假 HERDR 收到的行不是合法 JSON: {e}（{line}）"));
            request["method"] == target
        })
        .count()
}

/// 組一筆 `pane.read` 成功回應的 `result`（含外層 `"type"` 標籤，見
/// `herdr-client/tests/fixtures/pane-read-p20.json` 的 `result` 形狀，抄自
/// `tests/read_output.rs` 的 `pane_read_result`）。
fn pane_read_result(pane_id: &str, text: &str, truncated: bool) -> Value {
    json!({
        "type": "pane_read",
        "read": {
            "pane_id": pane_id,
            "workspace_id": "wJ",
            "tab_id": "wJ:t1",
            "source": "recent",
            "format": "text",
            "text": text,
            "revision": 0,
            "truncated": truncated,
        },
    })
}

/// 等到假 HERDR accept 到第 `count` 條連線；逾時即失敗。
async fn wait_for_connections(fake: &FakeHerdr, count: usize, what: &str) {
    let deadline = Instant::now() + TIMEOUT;
    loop {
        let received = fake.received();
        if received.len() >= count {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "{what}：應在 {TIMEOUT:?} 內看到第 {count} 條連線，實際: {received:?}"
        );
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
}

/// 等到假 HERDR 觀察到第 `index` 條連線關閉；逾時即失敗。
async fn wait_closed(fake: &FakeHerdr, index: usize, what: &str) {
    let deadline = Instant::now() + TIMEOUT;
    loop {
        let closed = fake.closed_connections();
        if closed.get(index).copied().unwrap_or(false) {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "{what}（連線 #{index}）應在 {TIMEOUT:?} 內被關閉，實際各連線關閉狀態: {closed:?}"
        );
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
}

/// 取下一筆事件，逾時即失敗。
async fn next_item(events: &mut RuntimeEvents) -> Option<Result<RuntimeEvent, RuntimeError>> {
    tokio::time::timeout(TIMEOUT, events.next())
        .await
        .expect("讀取事件流不應逾時")
}

/// spec「新 pane 加入訂閱」：S 訂閱 `wJ:p1`，L 推 `pane_created`（`wJ:p2`）→ 假 HERDR 收到
/// 新的 `events.subscribe`（含兩個 pane），且舊 S 在新 S 那條連線出現之後才被關閉。
#[tokio::test]
async fn pane_created_reopens_s_with_new_list_and_closes_old_after_started() {
    let config = FakeHerdrConfig::new()
        .with_snapshot_result(snapshot_result(&["wJ:p1"]))
        .with_subscribe_rule(
            SubscribeMatcher::LifecycleOnly,
            vec![Step::Event(pane_created("wJ:p2")), Step::Hold],
        )
        .with_subscribe_rule(SubscribeMatcher::PerPane, vec![Step::Hold]);
    let fake = FakeHerdr::start(config).await.expect("啟動假 HERDR 失敗");
    let runtime = runtime(&fake);

    let _events = tokio::time::timeout(TIMEOUT, runtime.subscribe())
        .await
        .expect("建立事件流不應逾時")
        .expect("建立事件流應成功");

    assert_eq!(
        fake.received().len(),
        3,
        "一開始只該有 snapshot／L／S 三條連線，實際: {:?}",
        fake.received()
    );
    assert!(
        !fake.closed_connections()[2],
        "去抖動還沒到期之前，舊 S 不該被關閉，實際: {:?}",
        fake.closed_connections()
    );

    // 新 S 那條連線出現之前，舊 S 必須一直開著（spec：收到 `subscription_started` 才關舊 S）。
    let deadline = Instant::now() + TIMEOUT;
    loop {
        if fake.received().len() >= 4 {
            break;
        }
        assert!(
            !fake.closed_connections()[2],
            "舊 S 不該在新 S 的連線出現之前就被關閉，實際各連線關閉狀態: {:?}",
            fake.closed_connections()
        );
        assert!(
            Instant::now() < deadline,
            "應在 {TIMEOUT:?} 內看到重開的 S 連線，實際: {:?}",
            fake.received()
        );
        tokio::time::sleep(Duration::from_millis(1)).await;
    }

    assert_eq!(
        subscribed_pane_ids(&fake, 3),
        ids(&["wJ:p1", "wJ:p2"]),
        "重開的 S 應同時訂閱舊 pane 與新 pane"
    );
    wait_closed(&fake, 2, "重開之後舊 S").await;
}

/// 同一個 spec 情境的「重疊」那一半：`pane_created_reopens_s_with_new_list_and_closes_old_after_started`
/// 只看得到「舊 S 最後有被關掉」，看不到「**新 S 開好之前**舊 S 一直開著」——正確實作與
/// 「先關舊的再開新的」之間只差幾微秒，測試輪詢不到（實測突變 M1b 殺不掉那個測試）。
///
/// 這裡用一個會拖慢連線建立的 connector 把那段窗口撐開：重開那一次 `connect()` 先睡
/// 400 ms 才真的連上假 HERDR，於是「去抖動到期（200 ms）」與「新 S 連上（約 600 ms）」之間
/// 有一段可以觀察的時間——spec 要求這段期間舊 S 必須還開著（design D10、spike 3：重疊才不
/// 丟事件）。
#[tokio::test]
async fn reopen_keeps_old_s_open_until_new_one_started() {
    let config = FakeHerdrConfig::new()
        .with_snapshot_result(snapshot_result(&["wJ:p1"]))
        .with_subscribe_rule(
            SubscribeMatcher::LifecycleOnly,
            vec![Step::Event(pane_created("wJ:p2")), Step::Hold],
        )
        .with_subscribe_rule(SubscribeMatcher::PerPane, vec![Step::Hold]);
    let fake = FakeHerdr::start(config).await.expect("啟動假 HERDR 失敗");
    // 前 3 次連線（snapshot／L／S）照常，第 4 次（重開的 S）才拖慢。
    let connector = Arc::new(DelayedConnector::after(
        fake.connector(),
        3,
        Duration::from_millis(400),
    ));
    let runtime = HerdrRuntime::new(RuntimeId::new("win"), connector, None);

    let _events = tokio::time::timeout(TIMEOUT, runtime.subscribe())
        .await
        .expect("建立事件流不應逾時")
        .expect("建立事件流應成功");

    // 去抖動（200 ms）已經到期、重開正卡在 connect 的 400 ms 裡：新 S 還沒連上，舊 S 必須
    // 還開著。
    tokio::time::sleep(Duration::from_millis(350)).await;
    assert_eq!(
        fake.received().len(),
        3,
        "此時重開的連線還沒建立（connect 還在等），實際: {:?}",
        fake.received()
    );
    assert!(
        !fake.closed_connections()[2],
        "新 S 還沒收到 subscription_started 之前，舊 S 必須還開著，實際各連線關閉狀態: {:?}",
        fake.closed_connections()
    );

    wait_for_connections(&fake, 4, "重開的 S 連上之後").await;
    assert_eq!(
        subscribed_pane_ids(&fake, 3),
        ids(&["wJ:p1", "wJ:p2"]),
        "重開的 S 應同時訂閱舊 pane 與新 pane"
    );
    wait_closed(&fake, 2, "新 S 開好之後舊 S").await;
}

/// spec「多次觸發合併」：L 連推 3 筆 `pane_created`（遠小於 100 ms 的間隔）→ 只多開一條 S
/// 連線，清單含全部 4 個 pane。
#[tokio::test]
async fn three_pane_events_within_100ms_reopen_once() {
    let config = FakeHerdrConfig::new()
        .with_snapshot_result(snapshot_result(&["wJ:p1"]))
        .with_subscribe_rule(
            SubscribeMatcher::LifecycleOnly,
            vec![
                Step::Event(pane_created("wJ:p2")),
                Step::Event(pane_created("wJ:p3")),
                Step::Event(pane_created("wJ:p4")),
                Step::Hold,
            ],
        )
        .with_subscribe_rule(SubscribeMatcher::PerPane, vec![Step::Hold]);
    let fake = FakeHerdr::start(config).await.expect("啟動假 HERDR 失敗");
    let runtime = runtime(&fake);

    let _events = tokio::time::timeout(TIMEOUT, runtime.subscribe())
        .await
        .expect("建立事件流不應逾時")
        .expect("建立事件流應成功");

    wait_for_connections(&fake, 4, "3 筆 pane_created 之後").await;
    tokio::time::sleep(AFTER_DEBOUNCE).await;

    assert_eq!(
        fake.received().len(),
        4,
        "100 ms 內的 3 次觸發只該合併成一次重開（總共 4 條連線），實際: {:?}",
        fake.received()
    );
    assert_eq!(
        subscribed_pane_ids(&fake, 3),
        ids(&["wJ:p1", "wJ:p2", "wJ:p3", "wJ:p4"]),
        "合併後的 S 清單應含 seed 的 pane 與 3 個新 pane"
    );
}

/// spec「snapshot 發現集合不同」：S 訂閱 `wJ:p1`，之後取得的 snapshot 只有 `wJ:p3`
/// → 重開 S、清單為 `wJ:p3`，舊 S 關閉。
#[tokio::test]
async fn snapshot_with_different_pane_set_reopens() {
    let config = FakeHerdrConfig::new()
        .with_method_responses(
            "session.snapshot",
            vec![
                MethodResponse::Success(snapshot_result(&["wJ:p1"])),
                MethodResponse::Success(snapshot_result(&["wJ:p3"])),
            ],
        )
        .with_subscribe_rule(SubscribeMatcher::LifecycleOnly, vec![Step::Hold])
        .with_subscribe_rule(SubscribeMatcher::PerPane, vec![Step::Hold]);
    let fake = FakeHerdr::start(config).await.expect("啟動假 HERDR 失敗");
    let runtime = runtime(&fake);

    let _events = tokio::time::timeout(TIMEOUT, runtime.subscribe())
        .await
        .expect("建立事件流不應逾時")
        .expect("建立事件流應成功");
    assert_eq!(
        subscribed_pane_ids(&fake, 2),
        ids(&["wJ:p1"]),
        "seed 的 S 應只訂閱 wJ:p1"
    );

    let snapshot = tokio::time::timeout(TIMEOUT, runtime.snapshot())
        .await
        .expect("取 snapshot 不應逾時")
        .expect("取 snapshot 應成功");
    let panes: BTreeSet<String> = snapshot
        .panes
        .iter()
        .map(|pane| pane.id.as_str().to_string())
        .collect();
    assert_eq!(panes, ids(&["wJ:p3"]), "第二次 snapshot 應只有 wJ:p3");

    // 第 4 條是剛才 snapshot 的短連線，第 5 條才是重開的 S。
    wait_for_connections(&fake, 5, "snapshot 發現集合不同之後").await;
    assert_eq!(
        subscribed_pane_ids(&fake, 4),
        ids(&["wJ:p3"]),
        "重開的 S 應只訂閱 snapshot 裡的 wJ:p3"
    );
    wait_closed(&fake, 2, "snapshot 觸發重開之後舊 S").await;
}

/// design D10 的收尾要求：重開之後才 spawn 的新 S reader 不在 `UnstartedEvents::start` 收下
/// 的那份 `JoinHandle` 清單裡，所以要另外保證「`RuntimeEvents` 被釋放 → 連重開的 S 也一起
/// 關掉」，不然每重開一次就漏一條連線。
#[tokio::test]
async fn dropping_stream_closes_reopened_s() {
    let config = FakeHerdrConfig::new()
        .with_snapshot_result(snapshot_result(&["wJ:p1"]))
        .with_subscribe_rule(
            SubscribeMatcher::LifecycleOnly,
            vec![Step::Event(pane_created("wJ:p2")), Step::Hold],
        )
        .with_subscribe_rule(SubscribeMatcher::PerPane, vec![Step::Hold]);
    let fake = FakeHerdr::start(config).await.expect("啟動假 HERDR 失敗");
    let runtime = runtime(&fake);

    let events = tokio::time::timeout(TIMEOUT, runtime.subscribe())
        .await
        .expect("建立事件流不應逾時")
        .expect("建立事件流應成功");
    wait_for_connections(&fake, 4, "重開之後").await;
    wait_closed(&fake, 2, "重開之後舊 S").await;

    drop(events);

    wait_closed(&fake, 1, "釋放事件流後 L").await;
    wait_closed(&fake, 3, "釋放事件流後重開的 S").await;
}

/// spec「新清單為空」：L 推 `pane_closed`（唯一的 pane）→ 只關舊 S、不開新 S。
#[tokio::test]
async fn empty_new_list_only_closes_old_s() {
    let config = FakeHerdrConfig::new()
        .with_snapshot_result(snapshot_result(&["wJ:p1"]))
        .with_subscribe_rule(
            SubscribeMatcher::LifecycleOnly,
            vec![Step::Event(pane_closed("wJ:p1")), Step::Hold],
        )
        .with_subscribe_rule(SubscribeMatcher::PerPane, vec![Step::Hold]);
    let fake = FakeHerdr::start(config).await.expect("啟動假 HERDR 失敗");
    let runtime = runtime(&fake);

    let _events = tokio::time::timeout(TIMEOUT, runtime.subscribe())
        .await
        .expect("建立事件流不應逾時")
        .expect("建立事件流應成功");

    wait_closed(&fake, 2, "最後一個 pane 關閉之後舊 S").await;
    tokio::time::sleep(AFTER_DEBOUNCE).await;
    assert_eq!(
        fake.received().len(),
        3,
        "新清單為空時不該再開 S 連線，實際: {:?}",
        fake.received()
    );
}

/// spec「新 S 建立失敗」：新 pane 的訂閱探測失敗 → 合併流送出帶原因的錯誤項後結束。
#[tokio::test]
async fn reopen_failure_ends_stream_with_error() {
    let config = FakeHerdrConfig::new()
        .with_snapshot_result(snapshot_result(&["wJ:p1"]))
        .with_subscribe_rule(
            SubscribeMatcher::LifecycleOnly,
            vec![Step::Event(pane_created("wJ:p2")), Step::Hold],
        )
        .with_subscribe_rule(SubscribeMatcher::PerPane, vec![Step::Hold])
        .with_failing_probe_pane_ids(["wJ:p2"]);
    let fake = FakeHerdr::start(config).await.expect("啟動假 HERDR 失敗");
    let runtime = runtime(&fake);

    let mut events = tokio::time::timeout(TIMEOUT, runtime.subscribe())
        .await
        .expect("建立事件流不應逾時")
        .expect("建立事件流應成功");

    // 先收到 `pane_created` 翻成的事件，再收到重開失敗的錯誤項。
    let mut error = None;
    for i in 0..4 {
        let item = next_item(&mut events)
            .await
            .unwrap_or_else(|| panic!("第 {i} 筆之前事件流不該結束（還沒看到錯誤項）"));
        match item {
            Ok(RuntimeEvent::PaneUpserted(pane)) => assert_eq!(pane.id.as_str(), "wJ:p2"),
            Ok(other) => panic!("只該收到 pane_created 翻成的事件與錯誤項，實際: {other:?}"),
            Err(e) => {
                error = Some(e);
                break;
            }
        }
    }
    let error = error.expect("重開失敗時事件流應送出一個錯誤項");
    assert!(
        error.to_string().contains("pane_not_found"),
        "錯誤原因應含 pane_not_found，實際: {error}"
    );

    assert!(
        next_item(&mut events).await.is_none(),
        "錯誤項之後事件流應結束"
    );
}

/// spec「版本落差」：`protocol` 為 23 → `snapshot()` 仍成功，並附含 `23` 的警告字串。
#[tokio::test]
async fn protocol_23_sets_warning_and_still_succeeds() {
    let config =
        FakeHerdrConfig::new().with_snapshot_result(snapshot_result_with_protocol(&["wJ:p1"], 23));
    let fake = FakeHerdr::start(config).await.expect("啟動假 HERDR 失敗");
    let runtime = runtime(&fake);

    let snapshot = tokio::time::timeout(TIMEOUT, runtime.snapshot())
        .await
        .expect("取 snapshot 不應逾時")
        .expect("protocol 落在已測範圍外仍應成功");

    assert_eq!(snapshot.protocol, 23);
    let warning = snapshot
        .protocol_warning
        .expect("protocol 23 應附協定版本警告");
    assert!(
        warning.contains("23"),
        "警告字串應含實際的 protocol 版本 23，實際: {warning}"
    );
    // ui-language task 3.2：同一則警告歸 `protocol_untested`，原文照舊。
    assert_eq!(warning, "HERDR protocol 23 不在已測範圍 20..=22");
    assert_eq!(
        Message::classify(&warning).msg(),
        MessageCode {
            code: "protocol_untested".to_string(),
            params: [
                ("protocol".to_string(), "23".to_string()),
                ("tested".to_string(), "20..=22".to_string()),
            ]
            .into(),
        }
    );
}

/// ui-language task 3.2：`snapshot()` 失敗的原因歸 `snapshot_failed`，`detail` 是 herdr-client 原文
/// （英文 thiserror 文字），原文欄位仍帶繁中前綴。
#[tokio::test]
async fn snapshot_failure_reason_is_classified_snapshot_failed() {
    let config = FakeHerdrConfig::new().with_method_response(
        "session.snapshot",
        MethodResponse::RemoteError {
            code: "internal_error".to_string(),
            message: "boom".to_string(),
        },
    );
    let fake = FakeHerdr::start(config).await.expect("啟動假 HERDR 失敗");
    let runtime = runtime(&fake);

    let err = tokio::time::timeout(TIMEOUT, runtime.snapshot())
        .await
        .expect("取 snapshot 不應逾時")
        .expect_err("session.snapshot 回錯誤時 snapshot() 應失敗");
    let reason = err.to_string();
    assert!(reason.starts_with("snapshot 失敗："), "原文前綴：{reason}");
    let msg = Message::classify(&reason).msg();
    assert_eq!(msg.code, "snapshot_failed");
    assert_eq!(
        format!("snapshot 失敗：{}", msg.params["detail"]),
        reason,
        "detail 加前綴應還原原文"
    );
    assert!(msg.params["detail"].contains("boom"), "{msg:?}");
}

/// spec「已測版本無警告」：`protocol` 為 22 → 沒有警告。
#[tokio::test]
async fn protocol_22_has_no_warning() {
    let config =
        FakeHerdrConfig::new().with_snapshot_result(snapshot_result_with_protocol(&["wJ:p1"], 22));
    let fake = FakeHerdr::start(config).await.expect("啟動假 HERDR 失敗");
    let runtime = runtime(&fake);

    let snapshot = tokio::time::timeout(TIMEOUT, runtime.snapshot())
        .await
        .expect("取 snapshot 不應逾時")
        .expect("取 snapshot 應成功");

    assert_eq!(snapshot.protocol, 22);
    assert_eq!(
        snapshot.protocol_warning, None,
        "protocol 22 在已測範圍內，不該有警告"
    );
}

// ---------------------------------------------------------------------------
// Fix round 1（Codex review 三項 findings）的回歸測試。
// ---------------------------------------------------------------------------

/// Finding 1 [high]：`snapshot()` 不得撤銷期間觀察到的 L 觸發。
///
/// 順序刻意排成「snapshot 請求送出 → L 推 `pane_created`（`wJ:p2`）→ 比較舊的 snapshot
/// 回應（只有 `wJ:p1`）才回來」：舊實作會無條件把 `desired` 改回 `{wJ:p1}`，去抖動 task 醒來
/// 看到與 `current` 相同就直接返回，`wJ:p2` 於是一路沒有 S 訂閱（要等下一個事件或下一次定期
/// snapshot 才補得回來）。
#[tokio::test]
async fn snapshot_response_older_than_l_event_does_not_revert_desired() {
    let config = FakeHerdrConfig::new()
        .with_snapshot_result(snapshot_result(&["wJ:p1"]))
        .with_subscribe_rule(
            SubscribeMatcher::LifecycleOnly,
            // t≈100 ms 推事件：夾在 snapshot 請求送出（t≈0）與它的回應（t≈200 ms）之間。
            vec![
                Step::Delay(Duration::from_millis(100)),
                Step::Event(pane_created("wJ:p2")),
                Step::Hold,
            ],
        )
        .with_subscribe_rule(SubscribeMatcher::PerPane, vec![Step::Hold]);
    let fake = FakeHerdr::start(config).await.expect("啟動假 HERDR 失敗");
    // 建立事件流的 3 條連線照常；之後每條（snapshot 的短連線、重開的 S）都慢 200 ms。
    let connector = Arc::new(DelayedConnector::after(
        fake.connector(),
        3,
        Duration::from_millis(200),
    ));
    let runtime = HerdrRuntime::new(RuntimeId::new("win"), connector, None);

    let _events = tokio::time::timeout(TIMEOUT, runtime.subscribe())
        .await
        .expect("建立事件流不應逾時")
        .expect("建立事件流應成功");

    let snapshot = tokio::time::timeout(TIMEOUT, runtime.snapshot())
        .await
        .expect("取 snapshot 不應逾時")
        .expect("取 snapshot 應成功");
    let panes: BTreeSet<String> = snapshot
        .panes
        .iter()
        .map(|pane| pane.id.as_str().to_string())
        .collect();
    assert_eq!(
        panes,
        ids(&["wJ:p1"]),
        "這份 snapshot 就是要比 L 事件舊（還沒有 wJ:p2）"
    );

    // 第 4 條是 snapshot 的短連線，第 5 條是重開的 S：清單必須仍含事件帶來的 wJ:p2。
    wait_for_connections(&fake, 5, "比事件舊的 snapshot 回來之後").await;
    assert_eq!(
        subscribed_pane_ids(&fake, 4),
        ids(&["wJ:p1", "wJ:p2"]),
        "較舊的 snapshot 不該把事件算出的目標集合改回去"
    );
    wait_closed(&fake, 2, "重開之後舊 S").await;
}

/// Finding 2 [high]：重開期間目標集合又變時，先開好的那條（較舊的集合）不得提交。
///
/// 第一次重開的 `connect()` 刻意慢 600 ms，期間 L 再推一個 pane：正確行為是那條剛開好的
/// 舊集合訂閱**整條丟掉**（連 reader 都不 spawn，所以它腳本裡的事件不會進合併流），然後用
/// 最新集合重開一次。舊實作會無條件 `replace` handle、把舊集合寫進 `current`。
#[tokio::test]
async fn two_reopens_completing_out_of_order_keep_latest_set() {
    let config = FakeHerdrConfig::new()
        .with_snapshot_result(snapshot_result(&["wJ:p1"]))
        .with_subscribe_rule(
            SubscribeMatcher::LifecycleOnly,
            vec![
                Step::Event(pane_created("wJ:p2")),
                Step::Delay(Duration::from_millis(250)),
                Step::Event(pane_created("wJ:p3")),
                Step::Hold,
            ],
        )
        // 最新集合（含 wJ:p3）那條只是掛著；比它舊的那條（含 wJ:p2、不含 wJ:p3）會推一筆狀態
        // 事件——那筆事件出現在合併流裡，就代表舊訂閱被錯誤地採用了。
        .with_subscribe_rule(
            SubscribeMatcher::ContainsPaneId("wJ:p3".to_string()),
            vec![Step::Hold],
        )
        .with_subscribe_rule(
            SubscribeMatcher::ContainsPaneId("wJ:p2".to_string()),
            vec![Step::Event(status_changed("wJ:p2")), Step::Hold],
        )
        .with_subscribe_rule(SubscribeMatcher::PerPane, vec![Step::Hold]);
    let fake = FakeHerdr::start(config).await.expect("啟動假 HERDR 失敗");
    // 第 4 次 connect（第一次重開）慢 600 ms，之後的都不慢。
    let connector = Arc::new(DelayedConnector::new(
        fake.connector(),
        vec![
            Duration::ZERO,
            Duration::ZERO,
            Duration::ZERO,
            Duration::from_millis(600),
        ],
        Duration::ZERO,
    ));
    let runtime = HerdrRuntime::new(RuntimeId::new("win"), connector, None);

    let mut events = tokio::time::timeout(TIMEOUT, runtime.subscribe())
        .await
        .expect("建立事件流不應逾時")
        .expect("建立事件流應成功");

    wait_for_connections(&fake, 5, "較舊的重開被丟掉、改用最新集合重開之後").await;
    assert_eq!(
        subscribed_pane_ids(&fake, 3),
        ids(&["wJ:p1", "wJ:p2"]),
        "第 4 條是慢的那次重開（集合還沒有 wJ:p3）"
    );
    assert_eq!(
        subscribed_pane_ids(&fake, 4),
        ids(&["wJ:p1", "wJ:p2", "wJ:p3"]),
        "最後生效的 S 應該是最新的集合"
    );
    wait_closed(&fake, 3, "被丟掉的那次重開連線").await;
    wait_closed(&fake, 2, "最初的 S").await;

    // 讓所有已經在路上的東西都落地，再確認最新的 S 沒有被較舊的結果關掉。
    tokio::time::sleep(AFTER_DEBOUNCE).await;
    assert!(
        !fake.closed_connections()[4],
        "最新的 S 不該被較舊的重開結果關掉，實際各連線關閉狀態: {:?}",
        fake.closed_connections()
    );
    assert_eq!(
        fake.received().len(),
        5,
        "不該再多開 S 連線，實際: {:?}",
        fake.received()
    );

    // 被丟掉的那條訂閱腳本裡的狀態事件不該進合併流（它連 reader 都不該有）。
    while let Ok(Some(item)) = tokio::time::timeout(Duration::from_millis(200), events.next()).await
    {
        match item.expect("這一段不該有錯誤項") {
            RuntimeEvent::PaneUpserted(_) => {}
            other => panic!("被丟掉的訂閱不該有 reader，卻收到它推的事件: {other:?}"),
        }
    }
}

/// Finding 3 [medium]：重連（`reset()`）必須收掉上一輪還在飛的重開，結果不得落到新一輪。
///
/// 第一輪的重開 `connect()` 刻意慢 800 ms，期間就開第二輪：正確行為是 `reset()` 把上一輪
/// `Shutdown` 裡的 task 全部 abort（那次重開連線根本不會建立）。
///
/// **刻意不 drop 第一輪的 `RuntimeEvents`**：drop 會讓 L task 的 `AbortAllOnDrop` 也把那次
/// 重開收掉，變成兩道保險一起生效、測不出 `reset()` 自己有沒有做事（實測「`reset()` 不
/// abort 上一輪」這個突變在 drop 版本下殺不掉）。
#[tokio::test]
async fn reset_during_pending_reopen_aborts_it() {
    let config = FakeHerdrConfig::new()
        .with_method_responses(
            "session.snapshot",
            vec![
                // 第一輪 seed：只有 wJ:p1，所以 L 推的 pane_created 會觸發重開。
                MethodResponse::Success(snapshot_result(&["wJ:p1"])),
                // 第二輪 seed：已經含 wJ:p2，第二輪自己的 pane_created 不會再開新連線。
                MethodResponse::Success(snapshot_result(&["wJ:p1", "wJ:p2"])),
            ],
        )
        .with_subscribe_rule(
            SubscribeMatcher::LifecycleOnly,
            vec![Step::Event(pane_created("wJ:p2")), Step::Hold],
        )
        .with_subscribe_rule(SubscribeMatcher::PerPane, vec![Step::Hold]);
    let fake = FakeHerdr::start(config).await.expect("啟動假 HERDR 失敗");
    // 第 4 次 connect（第一輪的重開）慢 800 ms；第二輪的三條連線不慢。
    let connector = Arc::new(DelayedConnector::new(
        fake.connector(),
        vec![
            Duration::ZERO,
            Duration::ZERO,
            Duration::ZERO,
            Duration::from_millis(800),
        ],
        Duration::ZERO,
    ));
    let runtime = HerdrRuntime::new(RuntimeId::new("win"), connector, None);

    let _first = tokio::time::timeout(TIMEOUT, runtime.subscribe())
        .await
        .expect("建立第一輪事件流不應逾時")
        .expect("建立第一輪事件流應成功");

    // 等去抖動到期、第一輪的重開卡進那 800 ms 的 connect 裡，再開第二輪。
    tokio::time::sleep(Duration::from_millis(250)).await;
    let _second = tokio::time::timeout(TIMEOUT, runtime.subscribe())
        .await
        .expect("建立第二輪事件流不應逾時")
        .expect("建立第二輪事件流應成功");

    wait_closed(&fake, 1, "重連之後第一輪的 L").await;
    wait_closed(&fake, 2, "重連之後第一輪的 S").await;

    // 撐過第一輪重開原本會連上的時間點（t≈1000 ms），確認它真的沒有連上。
    tokio::time::sleep(Duration::from_millis(1000)).await;
    assert_eq!(
        fake.received().len(),
        6,
        "只該有兩輪各自的 3 條連線；第一輪還在飛的重開不該連上，實際: {:?}",
        fake.received()
    );
    assert_eq!(
        subscribed_pane_ids(&fake, 5),
        ids(&["wJ:p1", "wJ:p2"]),
        "第 6 條應該是第二輪自己的 S"
    );
    assert!(
        !fake.closed_connections()[5],
        "第二輪的 S 不該被上一輪的殘留收尾關掉，實際各連線關閉狀態: {:?}",
        fake.closed_connections()
    );
}

/// spec「假 HERDR 收到的 method 集合」：完整跑過建立事件流、取 snapshot、重開狀態訂閱之後，
/// 假 HERDR 收到的 method 只有 `session.snapshot` 與 `events.subscribe`。
#[tokio::test]
async fn fake_only_receives_snapshot_and_subscribe_methods() {
    let config = FakeHerdrConfig::new()
        .with_snapshot_result(snapshot_result(&["wJ:p1"]))
        .with_subscribe_rule(
            SubscribeMatcher::LifecycleOnly,
            // 先讓 `snapshot()` 跑完（它會把 snapshot 的 pane 集合設成目標集合）再推
            // `pane_created`，才是「subscribe → snapshot → 重開」這個情境的順序。
            vec![
                Step::Delay(Duration::from_millis(300)),
                Step::Event(pane_created("wJ:p2")),
                Step::Hold,
            ],
        )
        .with_subscribe_rule(SubscribeMatcher::PerPane, vec![Step::Hold]);
    let fake = FakeHerdr::start(config).await.expect("啟動假 HERDR 失敗");
    let runtime = runtime(&fake);

    let _events = tokio::time::timeout(TIMEOUT, runtime.subscribe())
        .await
        .expect("建立事件流不應逾時")
        .expect("建立事件流應成功");
    tokio::time::timeout(TIMEOUT, runtime.snapshot())
        .await
        .expect("取 snapshot 不應逾時")
        .expect("取 snapshot 應成功");
    // 建立事件流 3 條 + snapshot 1 條 + 重開的 S 1 條。
    wait_for_connections(&fake, 5, "跑完建立事件流、取 snapshot、重開 S 之後").await;

    assert_eq!(
        methods_used(&fake),
        ids(&["events.subscribe", "session.snapshot"]),
        "不呼叫 read_output 時，只該用 session.snapshot 與 events.subscribe 兩個 method"
    );
}

/// live-output task 3.4：spec「只用唯讀 method」情境「讀取輸出只多一種 method」——在
/// 「假 HERDR 收到的 method 集合」那個情境（建立事件流、取 snapshot、重開狀態訂閱）之外，
/// 另外讀取一次 pane 輸出 → 只多收到恰好一筆 `pane.read`，沒有其他 method。
#[tokio::test]
async fn read_output_adds_exactly_one_pane_read_method() {
    let config = FakeHerdrConfig::new()
        .with_snapshot_result(snapshot_result(&["wJ:p1"]))
        .with_subscribe_rule(
            SubscribeMatcher::LifecycleOnly,
            vec![
                Step::Delay(Duration::from_millis(300)),
                Step::Event(pane_created("wJ:p2")),
                Step::Hold,
            ],
        )
        .with_subscribe_rule(SubscribeMatcher::PerPane, vec![Step::Hold])
        .with_method_response(
            "pane.read",
            MethodResponse::Success(pane_read_result("wJ:p1", "irrelevant", false)),
        );
    let fake = FakeHerdr::start(config).await.expect("啟動假 HERDR 失敗");
    let runtime = runtime(&fake);

    let _events = tokio::time::timeout(TIMEOUT, runtime.subscribe())
        .await
        .expect("建立事件流不應逾時")
        .expect("建立事件流應成功");
    tokio::time::timeout(TIMEOUT, runtime.snapshot())
        .await
        .expect("取 snapshot 不應逾時")
        .expect("取 snapshot 應成功");
    // 建立事件流 3 條 + snapshot 1 條 + 重開的 S 1 條。
    wait_for_connections(&fake, 5, "跑完建立事件流、取 snapshot、重開 S 之後").await;

    assert_eq!(
        methods_used(&fake),
        ids(&["events.subscribe", "session.snapshot"]),
        "讀取輸出之前不該多出任何 method"
    );

    tokio::time::timeout(TIMEOUT, runtime.read_output(&PaneId::new("wJ:p1"), 200))
        .await
        .expect("read_output 不應逾時")
        .expect("read_output 應成功");

    assert_eq!(
        method_call_count(&fake, "pane.read"),
        1,
        "另外讀取一次 pane 輸出應只多收到恰好一筆 pane.read"
    );
    assert_eq!(
        methods_used(&fake),
        ids(&["events.subscribe", "pane.read", "session.snapshot"]),
        "讀取輸出之後應多一種 pane.read method，沒有其他 method"
    );
}

/// Codex 最終 review finding 1 的整合層非退化測試：初始 S 的 `AbortHandle` 必須真的裝進
/// S 管理器，重開提交時的 `state.handle.replace(new)` 才有東西可以 abort。
///
/// 裝不進去（或被較新一代的 handle 蓋掉再被丟棄）時，初始那條 S reader 還活著：它與新 S
/// **同時**在送 `pane.agent_status_changed`，消費端就會看到重複的狀態事件。這裡讓初始 S
/// 在重開早就提交之後（700 ms）才推一筆只有它訂得到的事件（`wJ:p1`；重開後的 S 走另一條
/// 規則、什麼都不推），如果那筆事件跑得出來，就代表初始 S 沒有被收掉。
///
/// 與 `pane_created_reopens_s_with_new_list_and_closes_old_after_started` 的分工：那條看的
/// 是「舊 S 的**連線**最後有被關」，這條看的是「舊 S 不會再送事件進合併流」。
#[tokio::test]
async fn initial_s_stops_emitting_after_reopen_takes_over() {
    let config = FakeHerdrConfig::new()
        .with_snapshot_result(snapshot_result(&["wJ:p1"]))
        .with_subscribe_rule(
            SubscribeMatcher::LifecycleOnly,
            vec![Step::Event(pane_created("wJ:p2")), Step::Hold],
        )
        // 規則採「第一個符合的」，所以重開後的 S（清單含 wJ:p2）這條要排在通用的 PerPane
        // 之前；它什麼都不推，這樣合併流裡任何一筆狀態事件都只可能來自初始 S。
        .with_subscribe_rule(
            SubscribeMatcher::ContainsPaneId("wJ:p2".to_string()),
            vec![Step::Hold],
        )
        // 初始 S（只訂 wJ:p1）：等到重開早就提交（200 ms 去抖動）之後才推一筆事件。
        .with_subscribe_rule(
            SubscribeMatcher::PerPane,
            vec![
                Step::Delay(Duration::from_millis(700)),
                Step::Event(status_changed("wJ:p1")),
                Step::Hold,
            ],
        );
    let fake = FakeHerdr::start(config).await.expect("啟動假 HERDR 失敗");
    let runtime = runtime(&fake);

    let mut events = tokio::time::timeout(TIMEOUT, runtime.subscribe())
        .await
        .expect("建立事件流不應逾時")
        .expect("建立事件流應成功");

    // L 推的 `pane_created` 會翻成一筆 `PaneUpserted`，那是這條流唯一該有的事件。
    match next_item(&mut events).await {
        Some(Ok(RuntimeEvent::PaneUpserted(pane))) => {
            assert_eq!(pane.id.as_str(), "wJ:p2", "第一筆應該是新 pane 的 upsert");
        }
        other => panic!("第一筆應該是 pane_created 翻出來的 PaneUpserted，實際: {other:?}"),
    }

    wait_for_connections(&fake, 4, "重開的 S 連上之後").await;
    assert_eq!(
        subscribed_pane_ids(&fake, 3),
        ids(&["wJ:p1", "wJ:p2"]),
        "重開的 S 應同時訂閱舊 pane 與新 pane"
    );

    // 初始 S 要到 700 ms 才推事件；等過那個時間點，合併流必須一直是空的。
    let extra = tokio::time::timeout(Duration::from_millis(1200), events.next()).await;
    assert!(
        extra.is_err(),
        "重開接手之後初始 S 不該再送任何東西進合併流，實際收到: {extra:?}"
    );
}

/// task 6.3（design D11 1b deferred）：`Shutdown.aborts` 在登記新 handle 時要修剪已結束的
/// handle（`retain(|h| !h.is_finished())`），登記數才不會隨重開次數無限增長。
///
/// 「常數上限」：穩態下任一時刻最多同時活著的 task 是 L（1，全程不動）、目前生效的 S
/// reader（1）、正在跑的去抖動／重開 task（1）——`register()` 每次呼叫都先修剪才 push，所以
/// 登記數應該穩定貼著這個量級，跟「已經重開了幾次」無關。抓 8 當上限：比穩態量級（約 3）
/// 寬裕一截以吸收「abort 已呼叫但 task 還沒真的跑完」的短暫窗口，同時遠低於「修剪失效」時
/// 20 次重開會累積的登記數（每次重開兩筆 register，20 次會到 40 上下）——上限訂在兩者之間，
/// 才驗得出這條差異。
#[tokio::test]
async fn aborts_registry_does_not_grow_unbounded() {
    const REOPEN_COUNT: usize = 20;
    /// 見上方函式說明：遠低於「不修剪」的量級（約 2×`REOPEN_COUNT`），且比穩態量級寬裕。
    const ABORTS_UPPER_BOUND: usize = 8;
    /// 每次觸發之間的間隔：比去抖動（200 ms）與 `AFTER_DEBOUNCE`（400 ms）都寬裕，確保
    /// 20 次觸發各自獨立重開，不會被去抖動合併成更少次。
    const TOGGLE_GAP: Duration = Duration::from_millis(450);

    // L 腳本：交替推 `pane_created("wJ:p2")`／`pane_closed("wJ:p2")`，兩兩之間留
    // `TOGGLE_GAP`；seed 只有 `wJ:p1`，所以目標集合每次都在 {p1} 與 {p1,p2} 之間切換，
    // 20 次切換各自都是非空集合、各自都會真的重開一次 S。
    let mut lifecycle_steps = Vec::with_capacity(REOPEN_COUNT * 2 + 1);
    for i in 0..REOPEN_COUNT {
        lifecycle_steps.push(Step::Event(if i % 2 == 0 {
            pane_created("wJ:p2")
        } else {
            pane_closed("wJ:p2")
        }));
        lifecycle_steps.push(Step::Delay(TOGGLE_GAP));
    }
    lifecycle_steps.push(Step::Hold);

    let config = FakeHerdrConfig::new()
        .with_snapshot_result(snapshot_result(&["wJ:p1"]))
        .with_subscribe_rule(SubscribeMatcher::LifecycleOnly, lifecycle_steps)
        .with_subscribe_rule(SubscribeMatcher::PerPane, vec![Step::Hold]);
    let fake = FakeHerdr::start(config).await.expect("啟動假 HERDR 失敗");
    let runtime = runtime(&fake);

    let _events = tokio::time::timeout(TIMEOUT, runtime.subscribe())
        .await
        .expect("建立事件流不應逾時")
        .expect("建立事件流應成功");

    // 建立事件流 3 條（snapshot／L／S）+ 每次重開各多開一條 S = 3 + REOPEN_COUNT 條。
    // 20 次觸發跨度約 `REOPEN_COUNT * TOGGLE_GAP`（9 s），比共用的 `wait_for_connections`
    // 用的 `TIMEOUT`（5 s）長，這裡另外訂一個寬裕的期限。
    let want = 3 + REOPEN_COUNT;
    let deadline = Instant::now() + TOGGLE_GAP * (REOPEN_COUNT as u32) + TIMEOUT;
    loop {
        let received = fake.received();
        if received.len() >= want {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "20 次交替 pane_created／pane_closed 各自重開之後：應看到第 {want} 條連線，實際: {received:?}"
        );
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    // 最後一次重開落定（新 S 收到 `subscription_started`、舊 S 已關）之後再讀登記數，
    // 避免讀到「新 S 已連上但舊 handle 還沒被下一次 register 修剪掉」的過渡瞬間。
    tokio::time::sleep(AFTER_DEBOUNCE).await;

    let len = runtime.aborts_registry_len();
    assert!(
        len <= ABORTS_UPPER_BOUND,
        "重開 {REOPEN_COUNT} 次後 Shutdown.aborts 登記數應 <= {ABORTS_UPPER_BOUND}，實際: {len}"
    );
}
