//! Task 2.5 驗收測試：`HerdrRuntime` 建立事件流的順序（探測 → seed → L → S）與合併流
//! （spec `herdr-runtime-session`「建立事件流的順序」「合併事件流與結束」；design D1、D2、
//! D10）。
//!
//! 全部用真實 transport 的 `FakeHerdr`（Windows named pipe／unix socket），所以一律
//! `#[tokio::test]`、不暫停時間；每個 `await` 都用 `tokio::time::timeout` 包住，卡住時以
//! 逾時失敗而不是掛死整個測試。

mod common;

use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::{Duration, Instant};

use cockpit_core::{
    AgentRuntime, AgentStatus, PaneId, RuntimeError, RuntimeEvent, RuntimeEvents, RuntimeId,
};
use cockpit_herdr::probe::DistroProber;
use cockpit_herdr::runtime::{HerdrRuntime, WslProbe};
use herdr_client::testing::{FakeHerdr, FakeHerdrConfig, MethodResponse, Step, SubscribeMatcher};
use serde_json::{Value, json};

use common::FakeProber;

/// 每個 `await` 的保護逾時；本地全程都是程序內連線，正常情況遠低於這個值。
const TIMEOUT: Duration = Duration::from_secs(5);

/// L 腳本用的生命週期事件：抄自 `herdr-client/tests/fixtures/events-lifecycle-p20.ndjson`
/// 的 `tab_created` 一行，只改 `tab_id`／`number` 以便斷言是這一筆。
const TAB_CREATED: &str = r#"{"data":{"tab":{"agent_status":"unknown","focused":false,"label":"LABEL_1","number":9,"pane_count":1,"tab_id":"wD:t9","workspace_id":"wD"},"type":"tab_created"},"event":"tab_created"}"#;

/// S 腳本用的每 pane 事件：抄自 `herdr-client/tests/fixtures/events-status-p22.ndjson` 的一行，
/// 只改 `pane_id`／`workspace_id` 對齊本檔 seed 的 pane。
const STATUS_CHANGED: &str = r#"{"data":{"agent":"claude","agent_status":"done","pane_id":"wD:p1","workspace_id":"wD"},"event":"pane.agent_status_changed"}"#;

/// 組一個 `session.snapshot` 的成功 `result`：只有 pane 清單有內容（seed 只取 pane id）。
fn snapshot_result(pane_ids: &[&str]) -> Value {
    let panes: Vec<Value> = pane_ids
        .iter()
        .enumerate()
        .map(|(i, id)| {
            json!({
                "pane_id": id,
                "workspace_id": "wD",
                "tab_id": "wD:t1",
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
            "protocol": 22,
            "workspaces": [],
            "tabs": [],
            "panes": panes,
            "agents": [],
            "layouts": [],
        },
    })
}

/// L／S 都掛著不動的假 HERDR 設定（探測相關的測試只在意「有沒有開連線」）。
fn hold_config(pane_ids: &[&str]) -> FakeHerdrConfig {
    FakeHerdrConfig::new()
        .with_snapshot_result(snapshot_result(pane_ids))
        .with_subscribe_rule(SubscribeMatcher::LifecycleOnly, vec![Step::Hold])
        .with_subscribe_rule(SubscribeMatcher::PerPane, vec![Step::Hold])
}

/// 指向假 HERDR 的 `HerdrRuntime`（`wsl` 為 `None`，也就是不探測的 `win` 型 runtime）。
fn runtime(fake: &FakeHerdr) -> HerdrRuntime {
    HerdrRuntime::new(RuntimeId::new("win"), Arc::from(fake.connector()), None)
}

/// 組一筆 `pane.read` 成功回應的 `result`（含外層 `"type"` 標籤，抄自 `tests/read_output.rs`
/// 的 `pane_read_result`，供 live-output task 3.4「不干擾事件流」測試用）。
fn pane_read_result(pane_id: &str, text: &str, truncated: bool) -> Value {
    json!({
        "type": "pane_read",
        "read": {
            "pane_id": pane_id,
            "workspace_id": "wD",
            "tab_id": "wD:t1",
            "source": "recent",
            "format": "text",
            "text": text,
            "revision": 0,
            "truncated": truncated,
        },
    })
}

/// 假 HERDR 目前為止收到的所有 request 中，`method` 為 `events.subscribe` 的行數（累計、
/// 跨連線）；供判斷「有沒有重開訂閱」（重開會多開一條新的 `events.subscribe` 連線）。
fn events_subscribe_count(fake: &FakeHerdr) -> usize {
    fake.received()
        .iter()
        .flatten()
        .filter(|line| {
            let request: Value = serde_json::from_str(line)
                .unwrap_or_else(|e| panic!("假 HERDR 收到的行不是合法 JSON: {e}（{line}）"));
            request["method"] == "events.subscribe"
        })
        .count()
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

/// 取某條 `events.subscribe` request 的訂閱清單。
fn subscriptions_of(request: &Value) -> Vec<Value> {
    assert_eq!(
        request["method"], "events.subscribe",
        "這條連線應該是 events.subscribe，實際: {request}"
    );
    request["params"]["subscriptions"]
        .as_array()
        .unwrap_or_else(|| panic!("events.subscribe 應帶 subscriptions 陣列，實際: {request}"))
        .clone()
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
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

/// `RuntimeEvents` 沒有實作 `Debug`（`cockpit-core` 的既有設計），不能用 `Result::expect_err`；
/// 這裡自己 match 出錯誤，成功時直接 panic。
fn expect_failure(outcome: Result<RuntimeEvents, RuntimeError>, what: &str) -> RuntimeError {
    match outcome {
        Ok(_) => panic!("{what}"),
        Err(error) => error,
    }
}

/// 取下一筆事件，逾時即失敗。
async fn next_item(events: &mut RuntimeEvents) -> Option<Result<RuntimeEvent, RuntimeError>> {
    tokio::time::timeout(TIMEOUT, events.next())
        .await
        .expect("讀取事件流不應逾時")
}

/// spec「連線順序」：3 個 pane 的 seed → 依序 `session.snapshot`、24 種生命週期訂閱、
/// 3 筆 `pane.agent_status_changed` 訂閱。
#[tokio::test]
async fn connection_order_is_snapshot_then_l_then_s() {
    let config = FakeHerdrConfig::new()
        .with_snapshot_result(snapshot_result(&["wD:p1", "wD:p2", "wD:p3"]))
        .with_subscribe_rule(SubscribeMatcher::LifecycleOnly, vec![Step::Hold])
        .with_subscribe_rule(SubscribeMatcher::PerPane, vec![Step::Hold]);
    let fake = FakeHerdr::start(config).await.expect("啟動假 HERDR 失敗");
    let runtime = runtime(&fake);

    let events = tokio::time::timeout(TIMEOUT, runtime.subscribe())
        .await
        .expect("建立事件流不應逾時")
        .expect("建立事件流應成功");

    assert_eq!(
        fake.received().len(),
        3,
        "應依序開三條連線（snapshot／L／S），實際: {:?}",
        fake.received()
    );

    let seed = request_line(&fake, 0);
    assert_eq!(
        seed["method"], "session.snapshot",
        "第 1 條連線應是 session.snapshot，實際: {seed}"
    );

    let lifecycle = subscriptions_of(&request_line(&fake, 1));
    assert_eq!(
        lifecycle.len(),
        24,
        "第 2 條連線（L）應訂閱 24 種生命週期事件，實際: {lifecycle:?}"
    );
    assert!(
        lifecycle
            .iter()
            .all(|s| s["type"] != "pane.agent_status_changed"),
        "L 的訂閱清單不該含 pane.agent_status_changed，實際: {lifecycle:?}"
    );

    let status = subscriptions_of(&request_line(&fake, 2));
    assert_eq!(
        status.len(),
        3,
        "第 3 條連線（S）應對 seed 的 3 個 pane 各訂一筆，實際: {status:?}"
    );
    assert!(
        status
            .iter()
            .all(|s| s["type"] == "pane.agent_status_changed"),
        "S 的訂閱清單應全部是 pane.agent_status_changed，實際: {status:?}"
    );
    let pane_ids: BTreeSet<String> = status
        .iter()
        .map(|s| {
            s["pane_id"]
                .as_str()
                .unwrap_or_else(|| panic!("每 pane 訂閱應帶 pane_id，實際: {s}"))
                .to_string()
        })
        .collect();
    assert_eq!(
        pane_ids,
        ["wD:p1", "wD:p2", "wD:p3"]
            .into_iter()
            .map(str::to_string)
            .collect::<BTreeSet<_>>()
    );

    drop(events);
}

/// spec「seed 沒有 pane」：只開 L、不開 S，事件流照樣可用。
#[tokio::test]
async fn seed_without_panes_opens_only_l() {
    let config = FakeHerdrConfig::new()
        .with_snapshot_result(snapshot_result(&[]))
        .with_subscribe_rule(
            SubscribeMatcher::LifecycleOnly,
            vec![Step::Event(TAB_CREATED.to_string()), Step::Hold],
        )
        .with_subscribe_rule(SubscribeMatcher::PerPane, vec![Step::Hold]);
    let fake = FakeHerdr::start(config).await.expect("啟動假 HERDR 失敗");
    let runtime = runtime(&fake);

    let mut events = tokio::time::timeout(TIMEOUT, runtime.subscribe())
        .await
        .expect("建立事件流不應逾時")
        .expect("建立事件流應成功");

    assert_eq!(
        fake.received().len(),
        2,
        "seed 沒有 pane 時只該開 snapshot 與 L 兩條連線，實際: {:?}",
        fake.received()
    );

    let item = next_item(&mut events)
        .await
        .expect("事件流應有一筆事件")
        .expect("這一筆應是事件而不是錯誤");
    match item {
        RuntimeEvent::TabUpserted(tab) => assert_eq!(tab.id.as_str(), "wD:t9"),
        other => panic!("應收到 L 推的 TabUpserted，實際: {other:?}"),
    }
}

/// spec「狀態訂閱探測失敗」：S 的探測失敗 → 建立失敗、原因含 `pane_not_found`、L 連線關閉。
#[tokio::test]
async fn probe_failure_on_seed_pane_fails_subscribe_and_closes_l() {
    let config = FakeHerdrConfig::new()
        .with_snapshot_result(snapshot_result(&["wD:p1", "wD:p2", "wD:p3"]))
        .with_subscribe_rule(SubscribeMatcher::LifecycleOnly, vec![Step::Hold])
        .with_subscribe_rule(SubscribeMatcher::PerPane, vec![Step::Hold])
        .with_failing_probe_pane_ids(["wD:p2"]);
    let fake = FakeHerdr::start(config).await.expect("啟動假 HERDR 失敗");
    let runtime = runtime(&fake);

    let outcome = tokio::time::timeout(TIMEOUT, runtime.subscribe())
        .await
        .expect("建立事件流不應逾時");
    let error = expect_failure(outcome, "S 的探測失敗時建立事件流應失敗");
    assert!(
        error.to_string().contains("pane_not_found"),
        "失敗原因應含 pane_not_found，實際: {error}"
    );

    wait_closed(&fake, 1, "S 建立失敗後 L").await;
}

/// spec「兩條連線的事件都到達」：L 與 S 各推一筆，合併成同一條流（跨連線順序不保證，
/// 收齊兩筆後比對集合）。
#[tokio::test]
async fn events_from_l_and_s_arrive_in_one_stream() {
    let config = FakeHerdrConfig::new()
        .with_snapshot_result(snapshot_result(&["wD:p1"]))
        .with_subscribe_rule(
            SubscribeMatcher::LifecycleOnly,
            vec![Step::Event(TAB_CREATED.to_string()), Step::Hold],
        )
        .with_subscribe_rule(
            SubscribeMatcher::PerPane,
            vec![Step::Event(STATUS_CHANGED.to_string()), Step::Hold],
        );
    let fake = FakeHerdr::start(config).await.expect("啟動假 HERDR 失敗");
    let runtime = runtime(&fake);

    let mut events = tokio::time::timeout(TIMEOUT, runtime.subscribe())
        .await
        .expect("建立事件流不應逾時")
        .expect("建立事件流應成功");

    let mut saw_tab = false;
    let mut saw_status = false;
    for i in 0..2 {
        let item = next_item(&mut events)
            .await
            .unwrap_or_else(|| panic!("第 {i} 筆應有內容，事件流不該提前結束"))
            .unwrap_or_else(|e| panic!("第 {i} 筆應是事件而不是錯誤: {e}"));
        match item {
            RuntimeEvent::TabUpserted(tab) => {
                assert_eq!(tab.id.as_str(), "wD:t9");
                saw_tab = true;
            }
            RuntimeEvent::AgentStatusChanged {
                pane_id,
                status,
                agent,
                ..
            } => {
                assert_eq!(pane_id.as_str(), "wD:p1");
                assert_eq!(status, AgentStatus::Done);
                assert_eq!(agent.as_deref(), Some("claude"));
                saw_status = true;
            }
            other => {
                panic!("只該收到 L 的 TabUpserted 與 S 的 AgentStatusChanged，實際: {other:?}")
            }
        }
    }
    assert!(saw_tab, "應收到 L 推的 TabUpserted");
    assert!(saw_status, "應收到 S 推的 AgentStatusChanged");
}

/// spec「L 關閉」：推一筆事件後關閉 → 事件、原因含 `L` 的錯誤項、然後流結束。
#[tokio::test]
async fn l_close_yields_error_naming_l_then_ends() {
    let config = FakeHerdrConfig::new()
        .with_snapshot_result(snapshot_result(&["wD:p1"]))
        .with_subscribe_rule(
            SubscribeMatcher::LifecycleOnly,
            vec![Step::Event(TAB_CREATED.to_string()), Step::Close],
        )
        .with_subscribe_rule(SubscribeMatcher::PerPane, vec![Step::Hold]);
    let fake = FakeHerdr::start(config).await.expect("啟動假 HERDR 失敗");
    let runtime = runtime(&fake);

    let mut events = tokio::time::timeout(TIMEOUT, runtime.subscribe())
        .await
        .expect("建立事件流不應逾時")
        .expect("建立事件流應成功");

    let first = next_item(&mut events)
        .await
        .expect("第 1 筆應有內容")
        .expect("第 1 筆應是事件");
    match first {
        RuntimeEvent::TabUpserted(tab) => assert_eq!(tab.id.as_str(), "wD:t9"),
        other => panic!("第 1 筆應是 L 推的 TabUpserted，實際: {other:?}"),
    }

    let second = next_item(&mut events)
        .await
        .expect("第 2 筆應有內容")
        .expect_err("第 2 筆應是錯誤項");
    assert!(
        second.to_string().contains('L'),
        "錯誤原因應指出是 L 這條連線，實際: {second}"
    );

    assert!(
        next_item(&mut events).await.is_none(),
        "錯誤項之後事件流應結束"
    );
}

/// spec「釋放事件流」：drop 之後 L 與 S 兩條連線都被關閉。
#[tokio::test]
async fn dropping_stream_closes_both_connections() {
    let config = FakeHerdrConfig::new()
        .with_snapshot_result(snapshot_result(&["wD:p1", "wD:p2"]))
        .with_subscribe_rule(SubscribeMatcher::LifecycleOnly, vec![Step::Hold])
        .with_subscribe_rule(SubscribeMatcher::PerPane, vec![Step::Hold]);
    let fake = FakeHerdr::start(config).await.expect("啟動假 HERDR 失敗");
    let runtime = runtime(&fake);

    let events = tokio::time::timeout(TIMEOUT, runtime.subscribe())
        .await
        .expect("建立事件流不應逾時")
        .expect("建立事件流應成功");
    assert_eq!(fake.received().len(), 3, "應開了 snapshot／L／S 三條連線");

    drop(events);

    wait_closed(&fake, 1, "釋放事件流後 L").await;
    wait_closed(&fake, 2, "釋放事件流後 S").await;
}

/// spec「WSL 探測」：探測失敗 → `Unavailable`（附固定重試間隔），且不開任何連線。
#[tokio::test]
async fn prober_unavailable_short_circuits_before_any_connection() {
    let config = FakeHerdrConfig::new()
        .with_snapshot_result(snapshot_result(&["wD:p1"]))
        .with_subscribe_rule(SubscribeMatcher::LifecycleOnly, vec![Step::Hold])
        .with_subscribe_rule(SubscribeMatcher::PerPane, vec![Step::Hold]);
    let fake = FakeHerdr::start(config).await.expect("啟動假 HERDR 失敗");
    let retry_after = Duration::from_secs(60);
    let runtime = HerdrRuntime::new(
        RuntimeId::new("wsl"),
        Arc::from(fake.connector()),
        Some(WslProbe {
            distro: "Ubuntu-24.04".to_string(),
            // 探測器刻意帶另一個間隔（5 秒），用來證明 60 秒是來自 `WslProbe::retry_after`
            // 而不是探測器自帶的值。
            prober: Box::new(FakeProber::unavailable(
                "WSL 發行版 Ubuntu-24.04 未啟動",
                Duration::from_secs(5),
            )),
            retry_after,
        }),
    );

    let outcome = tokio::time::timeout(TIMEOUT, runtime.subscribe())
        .await
        .expect("建立事件流不應逾時");
    let error = expect_failure(outcome, "探測失敗時建立事件流應失敗");
    assert!(
        matches!(error, RuntimeError::Unavailable { .. }),
        "探測失敗應回 Unavailable，實際: {error:?}"
    );
    assert_eq!(
        error.retry_after(),
        Some(retry_after),
        "Unavailable 應附 WslProbe::retry_after 這個 runtime 設定的固定間隔（不是探測器自帶的 5 秒）"
    );
    assert!(
        fake.received().is_empty(),
        "探測失敗時不該開任何連線，實際: {:?}",
        fake.received()
    );
}

/// Codex review round 1 finding 1：L 與 S 幾乎同時結束時，消費端只該看到**一個**錯誤項
/// （spec「合併事件流與結束」：送出一個附原因的錯誤項後結束）。競態不是每次都會發生，
/// 所以重複 20 輪。
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn simultaneous_l_and_s_close_yield_exactly_one_error() {
    for round in 0..20 {
        let config = FakeHerdrConfig::new()
            .with_snapshot_result(snapshot_result(&["wD:p1"]))
            .with_subscribe_rule(SubscribeMatcher::LifecycleOnly, vec![Step::Close])
            .with_subscribe_rule(SubscribeMatcher::PerPane, vec![Step::Close]);
        let fake = FakeHerdr::start(config)
            .await
            .unwrap_or_else(|e| panic!("第 {round} 輪：啟動假 HERDR 失敗: {e}"));
        let runtime = runtime(&fake);

        let mut events = tokio::time::timeout(TIMEOUT, runtime.subscribe())
            .await
            .expect("建立事件流不應逾時")
            .unwrap_or_else(|e| panic!("第 {round} 輪：建立事件流應成功: {e}"));

        let first = next_item(&mut events)
            .await
            .unwrap_or_else(|| panic!("第 {round} 輪：兩條連線都關閉時應先收到一個錯誤項"));
        assert!(
            first.is_err(),
            "第 {round} 輪：第一筆應是錯誤項，實際: {first:?}"
        );

        let rest = next_item(&mut events).await;
        assert!(
            rest.is_none(),
            "第 {round} 輪：一個錯誤項之後事件流就該結束，卻又收到一筆: {rest:?}"
        );
    }
}

/// Codex review round 1 finding 2：探測失敗的重試間隔一律用 runtime 設定的
/// `WslProbe::retry_after`（來自 `wsl_probe_secs`），不論探測器回的是 `Failed`（沒有間隔）
/// 還是帶了別的間隔的 `Unavailable`。
#[tokio::test]
async fn probe_failure_uses_runtime_retry_interval_regardless_of_prober_error() {
    let runtime_interval = Duration::from_secs(60);
    let cases: Vec<(&str, Box<dyn DistroProber>, &str)> = vec![
        (
            "探測器回 Failed（完全沒有重試間隔）",
            Box::new(FakeProber::sequence(vec![Err(RuntimeError::Failed(
                "boom".to_string(),
            ))])),
            "boom",
        ),
        (
            "探測器回 Unavailable 但帶別的間隔（5 秒）",
            Box::new(FakeProber::unavailable(
                "WSL 發行版 Ubuntu-24.04 未啟動",
                Duration::from_secs(5),
            )),
            "WSL 發行版 Ubuntu-24.04 未啟動",
        ),
    ];

    for (label, prober, expected_reason) in cases {
        let fake = FakeHerdr::start(hold_config(&["wD:p1"]))
            .await
            .expect("啟動假 HERDR 失敗");
        let runtime = HerdrRuntime::new(
            RuntimeId::new("wsl"),
            Arc::from(fake.connector()),
            Some(WslProbe {
                distro: "Ubuntu-24.04".to_string(),
                prober,
                retry_after: runtime_interval,
            }),
        );

        let outcome = tokio::time::timeout(TIMEOUT, runtime.subscribe())
            .await
            .expect("建立事件流不應逾時");
        let error = expect_failure(outcome, "探測失敗時建立事件流應失敗");

        assert!(
            matches!(error, RuntimeError::Unavailable { .. }),
            "{label}：探測失敗一律應正規化成 Unavailable，實際: {error:?}"
        );
        assert_eq!(
            error.retry_after(),
            Some(runtime_interval),
            "{label}：重試間隔應是 runtime 設定的 {runtime_interval:?}"
        );
        assert!(
            error.to_string().contains(expected_reason),
            "{label}：原因應保留探測器回報的字串 {expected_reason:?}，實際: {error}"
        );
        assert!(
            fake.received().is_empty(),
            "{label}：探測失敗時不該開任何連線，實際: {:?}",
            fake.received()
        );
    }
}

/// spec「讀取 pane 輸出」情境「不干擾事件流」（live-output task 3.4）：事件流已建立並持續
/// 收到事件，期間讀取輸出 10 次 → 事件流沒有中斷、沒有重開訂閱（以假 HERDR 觀察到的
/// `events.subscribe` 次數不增加來判定）。L 每筆事件之間隔 20 ms，穿插在 10 次讀取輸出之間，
/// 藉此證明兩者真的同時在跑，不是讀取輸出把事件流卡住了。
#[tokio::test]
async fn read_output_ten_times_does_not_disturb_event_stream() {
    let mut lifecycle_steps = Vec::new();
    for _ in 0..10 {
        lifecycle_steps.push(Step::Event(TAB_CREATED.to_string()));
        lifecycle_steps.push(Step::Delay(Duration::from_millis(20)));
    }
    lifecycle_steps.push(Step::Hold);

    let config = FakeHerdrConfig::new()
        .with_snapshot_result(snapshot_result(&["wD:p1"]))
        .with_subscribe_rule(SubscribeMatcher::LifecycleOnly, lifecycle_steps)
        .with_subscribe_rule(SubscribeMatcher::PerPane, vec![Step::Hold])
        .with_method_response(
            "pane.read",
            MethodResponse::Success(pane_read_result("wD:p1", "irrelevant", false)),
        );
    let fake = FakeHerdr::start(config).await.expect("啟動假 HERDR 失敗");
    let runtime = runtime(&fake);
    let pane = PaneId::new("wD:p1");

    let mut events = tokio::time::timeout(TIMEOUT, runtime.subscribe())
        .await
        .expect("建立事件流不應逾時")
        .expect("建立事件流應成功");

    let subscribe_count_before = events_subscribe_count(&fake);
    assert_eq!(
        subscribe_count_before, 2,
        "建立事件流時應正好各開一次 L、S 的 events.subscribe"
    );

    for i in 0..10 {
        let item = next_item(&mut events)
            .await
            .unwrap_or_else(|| panic!("第 {i} 筆事件不該讓事件流提前結束"))
            .unwrap_or_else(|e| panic!("第 {i} 筆應是事件而不是錯誤: {e}"));
        match item {
            RuntimeEvent::TabUpserted(tab) => assert_eq!(tab.id.as_str(), "wD:t9"),
            other => panic!("第 {i} 筆應是 L 推的 TabUpserted，實際: {other:?}"),
        }

        tokio::time::timeout(TIMEOUT, runtime.read_output(&pane, 200))
            .await
            .expect("read_output 不應逾時")
            .unwrap_or_else(|e| panic!("第 {i} 次讀取輸出應成功: {e}"));
    }

    assert_eq!(
        events_subscribe_count(&fake),
        subscribe_count_before,
        "期間讀取輸出 10 次，events.subscribe 的次數不該增加（沒有重開訂閱）"
    );
}
