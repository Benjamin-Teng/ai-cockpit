//! Task 2.7 驗收測試：`cockpit_core::driver::run` ＋ `HerdrRuntime` ＋ `FakeHerdr` 跑完整
//! 迴圈（design D16；spec `runtime-driver` × `herdr-runtime-session`）。
//!
//! 驗證的是「驅動器的 scenario 在真的 HERDR 協定路徑上也成立」：連上 → 狀態庫等於 snapshot
//! → L 斷掉會斷線並以新的 seed 重連 → Drift 會重拿到不同內容 → S 探測失敗會退避重試。
//!
//! 全部用真實 transport 的 `FakeHerdr`（Windows named pipe／unix socket），所以一律不暫停
//! 時間、用真實的退避秒數；每個等待都是「輪詢加逾時」，卡住時以逾時失敗而不是掛死。
//! 用 `multi_thread` runtime：驅動器、runtime 的 reader task 與假 HERDR 的 handler 同時在跑，
//! 單執行緒 runtime 下測試本身的 `sleep` 會拖慢它們。
//!
//! **狀態庫的比對一律逐欄**（fix round 1 finding 1）：期望值在 [`expected_state`] 逐欄寫死
//! （不拿 `translate::snapshot` 當 oracle，理由見該函式），再與 `RuntimeState` 的
//! workspaces／tabs／panes／agents／focused／server_version／protocol 整個 `assert_eq!`，
//! 只把 `updated_at`（翻譯當下時間，本來就不可能相等）換成 store 裡的值。只比 id 的話，
//! label／focused／agent_status 這些欄位掉了或留著舊值都不會被抓到。
//!
//! **退避的驗證看狀態歷史而不是某個瞬間**（fix round 1 finding 2）：背景輪詢把每一次
//! `Disconnected { reason, retry_in }` 依序記錄下來，斷言「先 1 秒再 2 秒」；在固定時間點
//! 要求「現在恰好停在第二次斷線」的話，CI 過載時正確實作也會失敗。

use std::collections::{BTreeSet, HashMap};
use std::fmt::Display;
use std::hash::Hash;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};

use cockpit_core::driver::{self, Policy};
use cockpit_core::{
    Agent, AgentRuntime, AgentStatus, ConnectionState, Focused, Pane, PaneId, RuntimeId,
    RuntimeSnapshot, RuntimeStore, StoreHandle, Tab, TabId, Workspace, WorkspaceId,
};
use cockpit_herdr::runtime::HerdrRuntime;
use herdr_client::testing::{FakeHerdr, FakeHerdrConfig, MethodResponse, Step, SubscribeMatcher};
use serde_json::{Value, json};
use tokio::sync::oneshot;
use tokio::task::JoinHandle;

/// 一般等待的保護逾時（連線、斷線都在毫秒等級）。
const TIMEOUT: Duration = Duration::from_secs(5);

/// 等「退避之後重連」「退避序列走到第二筆」用的逾時：退避前兩筆是 1＋2 秒，再加上重連的
/// 時間，留足餘裕。
const RECONNECT_TIMEOUT: Duration = Duration::from_secs(8);

/// 輪詢間隔：比 L 腳本的 300 ms 延遲短一個數量級，不會錯過只存在 300 ms 的 `Connected`。
const POLL: Duration = Duration::from_millis(20);

/// `Subscription::all_lifecycle()` 的長度（L 這條連線應該訂的 24 種生命週期事件）。
const LIFECYCLE_SUBSCRIPTION_COUNT: usize = 24;

/// 本檔所有測試共用的驅動器參數：定期重拿 30 秒（測試期間不會到期），退避 1／2／4 秒。
fn policy() -> Policy {
    Policy {
        resnapshot: Duration::from_secs(30),
        backoff: vec![
            Duration::from_secs(1),
            Duration::from_secs(2),
            Duration::from_secs(4),
        ],
    }
}

/// 組一個 `session.snapshot` 的成功 `result`：固定 1 個 workspace（`wJ`）、1 個 tab
/// （`wJ:t1`），pane 由呼叫端指定。欄位與 `herdr-client` 的 `SessionSnapshot` schema 對齊。
///
/// 每個 pane 的 `agent`／`agent_status`／`title`／`cwd`／`label`／`focused` 都刻意給不同的
/// 值（第一個 pane 跑著 `claude` 且 `working`、其餘 `done`），並附一筆 `agents`——逐欄比對
/// 才有東西可比，全部填 `null`／`unknown` 的 fixture 抓不出欄位掉落。
fn snapshot_result(pane_ids: &[&str]) -> Value {
    let panes: Vec<Value> = pane_ids
        .iter()
        .enumerate()
        .map(|(i, id)| {
            json!({
                "pane_id": id,
                "workspace_id": "wJ",
                "tab_id": "wJ:t1",
                "agent": if i == 0 { json!("claude") } else { json!(null) },
                "agent_status": if i == 0 { "working" } else { "done" },
                "title": format!("TITLE_{i}"),
                "terminal_title": format!("TERM_TITLE_{i}"),
                "cwd": format!("/PROJECT_{i}"),
                "label": format!("PANE_LABEL_{i}"),
                "focused": i == 0,
                "revision": 1 + i as u64,
            })
        })
        .collect();
    let agents: Vec<Value> = pane_ids
        .first()
        .map(|id| {
            json!({
                "agent": "claude",
                "pane_id": id,
                "workspace_id": "wJ",
                "tab_id": "wJ:t1",
                "agent_status": "working",
            })
        })
        .into_iter()
        .collect();
    let pane_count = pane_ids.len() as u32;
    json!({
        "type": "session_snapshot",
        "snapshot": {
            "version": "0.9.0",
            "protocol": 22,
            "workspaces": [{
                "workspace_id": "wJ",
                "label": "WORKSPACE_LABEL",
                "number": 3,
                "active_tab_id": "wJ:t1",
                "agent_status": "working",
                "focused": true,
                "pane_count": pane_count,
                "tab_count": 1,
            }],
            "tabs": [{
                "tab_id": "wJ:t1",
                "workspace_id": "wJ",
                "number": 5,
                "label": "TAB_LABEL",
                "agent_status": "working",
                "focused": true,
                "pane_count": pane_count,
            }],
            "panes": panes,
            "agents": agents,
            "layouts": [],
            "focused_workspace_id": "wJ",
            "focused_tab_id": "wJ:t1",
            "focused_pane_id": pane_ids.first().copied(),
        },
    })
}

/// [`snapshot_result`] 那份 fixture 進到狀態庫之後**應該**長成的樣子，逐欄寫死。
///
/// **刻意不呼叫 `cockpit_herdr::translate::snapshot` 來算期望值**（fix round 1 實測）：那樣
/// 等於拿受測程式當 oracle，兩邊會一起壞——把 `translate::pane` 的 `agent_status` 寫死成
/// `Idle` 的突變，用 translate 算期望值時測試照樣通過，寫死期望值才抓得到。代價是這裡重述
/// 了一份翻譯結果，但驗收測試本來就該釘住對外契約，而不是重新推導它。
///
/// `updated_at` 是翻譯當下的時間，填 `UNIX_EPOCH` 佔位，比對前換成 store 裡的值。
fn expected_state(pane_ids: &[&str]) -> RuntimeSnapshot {
    let panes: Vec<Pane> = pane_ids
        .iter()
        .enumerate()
        .map(|(i, id)| Pane {
            id: PaneId::new(*id),
            workspace_id: WorkspaceId::new("wJ"),
            tab_id: TabId::new("wJ:t1"),
            agent: (i == 0).then(|| "claude".to_string()),
            agent_status: if i == 0 {
                AgentStatus::Working
            } else {
                AgentStatus::Done
            },
            title: Some(format!("TITLE_{i}")),
            cwd: Some(format!("/PROJECT_{i}")),
            label: Some(format!("PANE_LABEL_{i}")),
            focused: i == 0,
            // HERDR snapshot 沒有 exited 欄位，翻譯固定 false（design D15）。
            exited: false,
            updated_at: SystemTime::UNIX_EPOCH,
        })
        .collect();
    let agents: Vec<Agent> = pane_ids
        .first()
        .map(|id| Agent {
            agent: "claude".to_string(),
            pane_id: PaneId::new(*id),
            workspace_id: WorkspaceId::new("wJ"),
            tab_id: TabId::new("wJ:t1"),
            agent_status: AgentStatus::Working,
        })
        .into_iter()
        .collect();

    RuntimeSnapshot {
        server_version: "0.9.0".to_string(),
        protocol: 22,
        workspaces: vec![Workspace {
            id: WorkspaceId::new("wJ"),
            label: Some("WORKSPACE_LABEL".to_string()),
            number: 3,
            agent_status: AgentStatus::Working,
            focused: true,
        }],
        // `cockpit_core::Tab` 沒有 label／pane_count 欄位，HERDR 的那兩個欄位會被丟掉。
        tabs: vec![Tab {
            id: TabId::new("wJ:t1"),
            workspace_id: WorkspaceId::new("wJ"),
            number: 5,
            agent_status: AgentStatus::Working,
            focused: true,
        }],
        panes,
        agents,
        focused: Focused {
            workspace_id: Some(WorkspaceId::new("wJ")),
            tab_id: Some(TabId::new("wJ:t1")),
            pane_id: pane_ids.first().map(|id| PaneId::new(*id)),
        },
        protocol_warning: None,
    }
}

/// L 腳本用的 `pane_closed`（依 schema 的 `PaneClosedPayload`：`pane_id`、`workspace_id`）。
fn pane_closed(pane_id: &str) -> String {
    format!(
        r#"{{"data":{{"pane_id":"{pane_id}","type":"pane_closed","workspace_id":"wJ"}},"event":"pane_closed"}}"#
    )
}

/// 把一個以 id 當 key 的 map 換成字串集合（只在「還沒到齊」的輪詢條件裡用；正式斷言一律
/// 走 [`assert_store_matches`] 的逐欄比對）。
fn ids<K: Display + Eq + Hash, V>(map: &HashMap<K, V>) -> BTreeSet<String> {
    map.keys().map(ToString::to_string).collect()
}

/// 期望的字串集合（寫斷言用）。
fn expected(items: &[&str]) -> BTreeSet<String> {
    items.iter().map(ToString::to_string).collect()
}

/// 一個 `pane.agent_status_changed` 訂閱項的 JSON 形狀（`Subscription` 的 serde tag 是
/// `type`，見 `herdr_client::types::Subscription`）。
fn status_subscription(pane_id: &str) -> Value {
    json!({ "type": "pane.agent_status_changed", "pane_id": pane_id })
}

/// 把訂閱清單排序後回傳，好與期望清單比較（訂閱順序由 `BTreeSet` 決定，但不把順序寫死進
/// 斷言，只要求集合與重複次數完全一致）。
fn sorted(mut subscriptions: Vec<Value>) -> Vec<Value> {
    subscriptions.sort_by_key(ToString::to_string);
    subscriptions
}

/// 一組「假 HERDR ＋ HerdrRuntime ＋ 狀態庫 ＋ 驅動器」的完整迴圈。
struct Harness {
    fake: FakeHerdr,
    store: StoreHandle,
    id: RuntimeId,
    stop: Option<oneshot::Sender<()>>,
    driver: Option<JoinHandle<()>>,
}

impl Harness {
    /// 起假 HERDR、把 `HerdrRuntime` 登記進狀態庫，然後 spawn 驅動器。
    async fn start(config: FakeHerdrConfig) -> Self {
        let fake = FakeHerdr::start(config).await.expect("假 HERDR 應能啟動");
        let id = RuntimeId::new("herdr-loop");
        let runtime: Arc<dyn AgentRuntime> = Arc::new(HerdrRuntime::new(
            id.clone(),
            Arc::from(fake.connector()),
            None,
        ));
        let store = StoreHandle::new(RuntimeStore::new());
        store.register(
            id.clone(),
            "herdr".to_string(),
            fake.endpoint_path().display().to_string(),
        );
        let (stop, stop_rx) = oneshot::channel();
        let driver = tokio::spawn(driver::run(runtime, store.clone(), policy(), stop_rx));
        Self {
            fake,
            store,
            id,
            stop: Some(stop),
            driver: Some(driver),
        }
    }

    /// 目前的連線狀態（複製一份出來，不抓著鎖）。
    fn connection(&self) -> ConnectionState {
        self.store
            .with_store(|store| store.connection(&self.id).cloned())
            .expect("runtime 應已登記")
    }

    /// 狀態庫裡目前的 pane id 集合（輪詢條件用）。
    fn pane_ids(&self) -> BTreeSet<String> {
        self.store
            .with_store(|store| ids(&store.state(&self.id).expect("runtime 應已登記").panes))
    }

    /// 假 HERDR 各條連線收到的第一行 request（依連線建立順序）。
    fn requests(&self) -> Vec<Value> {
        self.fake
            .received()
            .iter()
            .enumerate()
            .map(|(i, lines)| {
                let line = lines
                    .first()
                    .unwrap_or_else(|| panic!("第 {i} 條連線應收到一行 request，實際: {lines:?}"));
                serde_json::from_str(line)
                    .unwrap_or_else(|e| panic!("第 {i} 條連線收到的行不是合法 JSON: {e}（{line}）"))
            })
            .collect()
    }

    /// 各條連線的 method 名稱，依連線建立順序。
    fn methods(&self) -> Vec<String> {
        self.requests()
            .iter()
            .map(|request| {
                request["method"]
                    .as_str()
                    .unwrap_or_else(|| panic!("request 應有 method 欄位，實際: {request}"))
                    .to_string()
            })
            .collect()
    }

    /// `session.snapshot` 總共被呼叫幾次。
    fn snapshot_calls(&self) -> usize {
        self.methods()
            .iter()
            .filter(|method| *method == "session.snapshot")
            .count()
    }

    /// 第 `index` 條連線的 `events.subscribe` 訂閱清單（原樣，未排序）；不是
    /// `events.subscribe` 就 panic。
    fn subscriptions(&self, index: usize) -> Vec<Value> {
        let requests = self.requests();
        let request = requests
            .get(index)
            .unwrap_or_else(|| panic!("應有第 {index} 條連線，實際只有 {} 條", requests.len()));
        assert_eq!(
            request["method"], "events.subscribe",
            "第 {index} 條連線應該是 events.subscribe，實際: {request}"
        );
        request["params"]["subscriptions"]
            .as_array()
            .unwrap_or_else(|| panic!("events.subscribe 應帶 subscriptions 陣列，實際: {request}"))
            .clone()
    }

    /// 送出停止指令並等驅動器收工（順便驗證停止指令真的有效）。
    async fn shutdown(mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(driver) = self.driver.take() {
            tokio::time::timeout(TIMEOUT, driver)
                .await
                .expect("驅動器應在收到停止指令後結束")
                .expect("驅動器 task 不應 panic");
        }
    }
}

impl Drop for Harness {
    /// 測試失敗（panic）時 `shutdown()` 不會被呼叫，這裡兜底收掉驅動器，免得它在背景繼續
    /// 重連、干擾同一個 test binary 裡的其他測試。
    fn drop(&mut self) {
        if let Some(driver) = self.driver.take() {
            driver.abort();
        }
    }
}

/// 逐欄斷言狀態庫等於 `result` 這份 snapshot 翻譯後的內容。
///
/// `Pane::updated_at` 是翻譯當下的時間，不可能與期望值相等，所以比對前先換成 store 裡的
/// 值——其餘每一個欄位（含 `label`、`focused`、`agent_status`、`title`、`cwd`、`exited`）
/// 都真的比。
fn assert_store_matches(harness: &Harness, pane_ids: &[&str], what: &str) {
    let snapshot = expected_state(pane_ids);
    harness.store.with_store(|store| {
        let state = store.state(&harness.id).expect("runtime 應已登記");

        assert_eq!(
            state.server_version.as_deref(),
            Some(snapshot.server_version.as_str()),
            "{what}：server_version 應取自 snapshot"
        );
        assert_eq!(
            state.protocol,
            Some(snapshot.protocol),
            "{what}：protocol 應取自 snapshot"
        );
        assert_eq!(
            state.focused, snapshot.focused,
            "{what}：焦點應取自 snapshot"
        );

        let workspaces: HashMap<WorkspaceId, Workspace> = snapshot
            .workspaces
            .iter()
            .map(|workspace| (workspace.id.clone(), workspace.clone()))
            .collect();
        assert_eq!(
            state.workspaces, workspaces,
            "{what}：workspaces 應逐欄相等"
        );

        let tabs: HashMap<TabId, Tab> = snapshot
            .tabs
            .iter()
            .map(|tab| (tab.id.clone(), tab.clone()))
            .collect();
        assert_eq!(state.tabs, tabs, "{what}：tabs 應逐欄相等");

        let panes: HashMap<PaneId, Pane> = snapshot
            .panes
            .iter()
            .map(|pane| {
                let mut pane = pane.clone();
                // `updated_at` 是翻譯當下時間，本來就不會相等：換成 store 裡的值，其餘欄位
                // 照比。store 缺這個 pane 的話直接在這裡失敗，訊息比整包 map 的 diff 好讀。
                let actual = state.panes.get(&pane.id).unwrap_or_else(|| {
                    panic!(
                        "{what}：狀態庫應有 pane {}，實際只有 {:?}",
                        pane.id,
                        ids(&state.panes)
                    )
                });
                pane.updated_at = actual.updated_at;
                (pane.id.clone(), pane)
            })
            .collect();
        assert_eq!(state.panes, panes, "{what}：panes 應逐欄相等");

        let agents: HashMap<PaneId, Agent> = snapshot
            .agents
            .iter()
            .map(|agent| (agent.pane_id.clone(), agent.clone()))
            .collect();
        assert_eq!(state.agents, agents, "{what}：agents 應逐欄相等");
    });
}

/// 輪詢等到 `cond` 成立；逾時即失敗，錯誤訊息帶上連線狀態與目前收到的 method 清單。
async fn wait_until(
    harness: &Harness,
    what: &str,
    timeout: Duration,
    mut cond: impl FnMut(&Harness) -> bool,
) {
    let deadline = Instant::now() + timeout;
    loop {
        if cond(harness) {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "{what}——{timeout:?} 內未成立；連線狀態: {:?}，收到的 method: {:?}",
            harness.connection(),
            harness.methods()
        );
        tokio::time::sleep(POLL).await;
    }
}

/// 背景記錄「每一次斷線」的狀態歷史（fix round 1 finding 2）。
///
/// 每 [`POLL`] 讀一次連線狀態，看到 `Disconnected` 就把 `(reason, retry_in)` 記下來，
/// 與上一筆完全相同時不重複記（同一次斷線會被輪詢看到很多次）。斷言因此針對「依序發生過
/// 哪些斷線」，而不是「某個時間點恰好停在哪個狀態」——後者在 CI 過載時正確實作也會失敗。
struct DisconnectLog {
    entries: Arc<Mutex<Vec<(String, Duration)>>>,
    task: JoinHandle<()>,
}

impl DisconnectLog {
    fn start(harness: &Harness) -> Self {
        let entries: Arc<Mutex<Vec<(String, Duration)>>> = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&entries);
        let store = harness.store.clone();
        let id = harness.id.clone();
        let task = tokio::spawn(async move {
            loop {
                let current = store.with_store(|store| store.connection(&id).cloned());
                if let Some(ConnectionState::Disconnected { reason, retry_in }) = current {
                    let entry = (reason, retry_in);
                    let mut log = sink.lock().expect("DisconnectLog mutex poisoned");
                    if log.last() != Some(&entry) {
                        log.push(entry);
                    }
                }
                tokio::time::sleep(POLL).await;
            }
        });
        Self { entries, task }
    }

    fn entries(&self) -> Vec<(String, Duration)> {
        self.entries
            .lock()
            .expect("DisconnectLog mutex poisoned")
            .clone()
    }

    /// 等到記錄到至少 `count` 次斷線；逾時即失敗。
    async fn wait_for(&self, count: usize, timeout: Duration) -> Vec<(String, Duration)> {
        let deadline = Instant::now() + timeout;
        loop {
            let entries = self.entries();
            if entries.len() >= count {
                return entries;
            }
            assert!(
                Instant::now() < deadline,
                "{timeout:?} 內應記錄到至少 {count} 次斷線，實際: {entries:?}"
            );
            tokio::time::sleep(POLL).await;
        }
    }
}

impl Drop for DisconnectLog {
    fn drop(&mut self) {
        self.task.abort();
    }
}

/// 完整週期：seed → L → S → 權威 snapshot → `Connected`，狀態庫逐欄等於 snapshot。
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn full_cycle_reaches_connected_with_store_equal_to_snapshot() {
    let fixture = snapshot_result(&["wJ:p1", "wJ:p2"]);
    let harness = Harness::start(
        FakeHerdrConfig::new()
            .with_snapshot_result(fixture.clone())
            .with_subscribe_rule(SubscribeMatcher::LifecycleOnly, vec![Step::Hold])
            .with_subscribe_rule(SubscribeMatcher::PerPane, vec![Step::Hold]),
    )
    .await;

    wait_until(&harness, "連線應進入 Connected", TIMEOUT, |h| {
        matches!(h.connection(), ConnectionState::Connected { .. })
    })
    .await;

    assert_store_matches(&harness, &["wJ:p1", "wJ:p2"], "完整週期");

    let methods = harness.methods();
    assert_eq!(
        methods,
        vec![
            "session.snapshot".to_string(),
            "events.subscribe".to_string(),
            "events.subscribe".to_string(),
            "session.snapshot".to_string(),
        ],
        "一輪完整週期應該只有 seed snapshot、L、S、權威 snapshot 四條連線"
    );

    // L：24 種生命週期訂閱，一筆每 pane 訂閱都不能有。
    let lifecycle = harness.subscriptions(1);
    assert_eq!(
        lifecycle.len(),
        LIFECYCLE_SUBSCRIPTION_COUNT,
        "第二條連線應該是 L（24 種生命週期訂閱），實際: {lifecycle:?}"
    );
    assert!(
        lifecycle
            .iter()
            .all(|sub| sub["type"] != "pane.agent_status_changed"),
        "L 不該含每 pane 訂閱，實際: {lifecycle:?}"
    );

    // S：**精確等於** seed 的兩個 pane，沒有漏、沒有重複、沒有混進別種訂閱。
    assert_eq!(
        sorted(harness.subscriptions(2)),
        sorted(vec![
            status_subscription("wJ:p1"),
            status_subscription("wJ:p2"),
        ]),
        "第三條連線應該是 S，且訂閱清單精確等於 seed 的 pane 集合"
    );

    harness.shutdown().await;
}

/// L 連線被關掉：驅動器寫 `Disconnected`（原因指名 L），退避之後整輪重來、重新 seed。
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn l_close_disconnects_then_reconnects_with_fresh_seed() {
    let harness = Harness::start(
        FakeHerdrConfig::new()
            .with_snapshot_result(snapshot_result(&["wJ:p1", "wJ:p2"]))
            .with_subscribe_rule(
                SubscribeMatcher::LifecycleOnly,
                vec![Step::Delay(Duration::from_millis(300)), Step::Close],
            )
            .with_subscribe_rule(SubscribeMatcher::PerPane, vec![Step::Hold]),
    )
    .await;
    let log = DisconnectLog::start(&harness);

    wait_until(&harness, "第一輪應先連上", TIMEOUT, |h| {
        matches!(h.connection(), ConnectionState::Connected { .. })
    })
    .await;

    let entries = log.wait_for(1, TIMEOUT).await;
    let (reason, retry_in) = entries[0].clone();
    assert!(
        reason.contains('L'),
        "斷線原因應指名是 L 這條連線結束，實際: {reason:?}"
    );
    // ui-language task 3.2：同一則原因歸 `event_connection_ended`，label 為 L。
    let msg = cockpit_core::Message::classify(&reason).msg();
    assert_eq!(msg.code, "event_connection_ended", "{reason:?}");
    assert_eq!(msg.params["label"], "L");
    assert_eq!(
        retry_in,
        Duration::from_secs(1),
        "第一次斷線應採用退避序列第一筆"
    );

    wait_until(
        &harness,
        "退避之後應重新連上",
        RECONNECT_TIMEOUT,
        |h| h.methods().len() >= 8,
    )
    .await;

    let methods = harness.methods();
    assert_eq!(
        &methods[4..8],
        [
            "session.snapshot".to_string(),
            "events.subscribe".to_string(),
            "events.subscribe".to_string(),
            "session.snapshot".to_string(),
        ],
        "重連應該整輪重來：重新 seed → L → S → 權威 snapshot，實際: {methods:?}"
    );

    harness.shutdown().await;
}

/// Drift：L 推一筆不存在 pane 的 `pane_closed` → 狀態庫回 Drift → 驅動器立刻重拿，拿到
/// 回應佇列的第三筆（內容與前兩筆不同），且狀態庫逐欄換成那一份。
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn drift_event_triggers_resnapshot_with_second_response() {
    let first = snapshot_result(&["wJ:p1", "wJ:p2"]);
    let second = snapshot_result(&["wJ:p1"]);
    let harness = Harness::start(
        FakeHerdrConfig::new()
            .with_method_responses(
                "session.snapshot",
                vec![
                    MethodResponse::Success(first.clone()),
                    MethodResponse::Success(first.clone()),
                    MethodResponse::Success(second.clone()),
                ],
            )
            .with_subscribe_rule(
                SubscribeMatcher::LifecycleOnly,
                vec![
                    Step::Delay(Duration::from_millis(300)),
                    Step::Event(pane_closed("wJ:p9")),
                    Step::Hold,
                ],
            )
            .with_subscribe_rule(SubscribeMatcher::PerPane, vec![Step::Hold]),
    )
    .await;

    wait_until(&harness, "連線應先進入 Connected", TIMEOUT, |h| {
        matches!(h.connection(), ConnectionState::Connected { .. })
    })
    .await;
    assert_store_matches(&harness, &["wJ:p1", "wJ:p2"], "重拿之前");

    wait_until(
        &harness,
        "Drift 之後狀態庫應換成第三筆回應",
        TIMEOUT,
        |h| h.pane_ids() == expected(&["wJ:p1"]),
    )
    .await;
    assert_store_matches(&harness, &["wJ:p1"], "Drift 重拿之後");

    assert_eq!(
        harness.snapshot_calls(),
        3,
        "session.snapshot 應該被呼叫三次（seed、權威、Drift 重拿），實際 method: {:?}",
        harness.methods()
    );
    assert!(
        matches!(harness.connection(), ConnectionState::Connected { .. }),
        "重拿成功不該斷線，實際: {:?}",
        harness.connection()
    );

    harness.shutdown().await;
}

/// S 訂閱探測失敗：驅動器斷線（原因帶 `pane_not_found`）並依退避序列重試——看的是斷線
/// 歷史「先 1 秒再 2 秒」，不是某個時間點的瞬間狀態。
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn s_probe_failure_backs_off_and_retries() {
    let harness = Harness::start(
        FakeHerdrConfig::new()
            .with_snapshot_result(snapshot_result(&["wJ:p1", "wJ:p2"]))
            .with_subscribe_rule(SubscribeMatcher::LifecycleOnly, vec![Step::Hold])
            .with_failing_probe_pane_ids(["wJ:p1"]),
    )
    .await;
    let log = DisconnectLog::start(&harness);

    let entries = log.wait_for(2, RECONNECT_TIMEOUT).await;

    for (index, (reason, _)) in entries.iter().take(2).enumerate() {
        assert!(
            reason.contains("pane_not_found"),
            "第 {} 次斷線的原因應帶假 HERDR 回的探測錯誤碼，實際: {reason:?}",
            index + 1
        );
    }
    let retry_ins: Vec<Duration> = entries.iter().take(2).map(|(_, retry)| *retry).collect();
    assert_eq!(
        retry_ins,
        vec![Duration::from_secs(1), Duration::from_secs(2)],
        "斷線歷史應依序採用退避序列的第一、第二筆，實際全部紀錄: {entries:?}"
    );

    assert!(
        harness.snapshot_calls() >= 2,
        "兩次斷線代表重試真的發生過（至少兩次 seed snapshot），實際 method: {:?}",
        harness.methods()
    );

    harness.shutdown().await;
}
