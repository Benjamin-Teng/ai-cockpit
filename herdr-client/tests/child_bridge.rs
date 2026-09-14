//! Task 4.4 驗收測試：子程序橋接接上 `Client`（`ChildStdioConnector` 經 `test_child relay`
//! 連到假 HERDR；design D3、D9；spec `herdr-transport`「子程序 stdio 橋接連線」、
//! `herdr-event-subscription`「子程序橋接下的結束」「事件即時送達（一秒內）」）。
//!
//! `relay <endpoint_path>` 是 `herdr-client-test-child` 的第五種模式（`src/bin/test_child.rs`），
//! 行為像 `nc -U`：連到假 HERDR 的原生端點，之後 stdin 每一行轉送到 socket、socket 每一行寫到
//! stdout 並 flush；socket EOF 或 stdin EOF 時結束程序（exit 0）。

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use async_trait::async_trait;

use herdr_client::client::{
    Client, IncomingEvent, RequestError, SessionSnapshotRequest, StreamError,
};
use herdr_client::connector::{ChildStdioConnector, ConnectError, Connector, NdjsonStream};
use herdr_client::testing::{FakeHerdr, FakeHerdrConfig, Step};
use herdr_client::types::{EventKind, Subscription};

mod common;
use common::load_json;

fn test_child_bin() -> &'static str {
    env!("CARGO_BIN_EXE_herdr-client-test-child")
}

// ---------------------------------------------------------------------------
// PID 擷取用的 `Connector` 包裝：在 `ChildStdioConnector::spawn_stream()`（取得具體型別）
// 之後、包成 `Box<dyn NdjsonStream>` 交給 `Client` 之前，先記下 relay 子程序的 pid，供測試在
// `EventStream` drop 之後輪詢確認它已經結束。
//
// 不用「作業系統層依 image name 計數前後差」（brief 建議的替代做法之一）：本 crate 的整合
// 測試（`tests/transport.rs`、`tests/request.rs` 等）也會平行啟動同名的
// `herdr-client-test-child` 子程序，全域計數在平行測試下不可靠（cargo test 預設多執行緒跑多個
// `#[tokio::test]`）；改成追蹤這次 spawn 實際拿到的 pid，逐一輪詢它是否還存在，不受其他測試
// 影響，也不需要碰 `src/client/subscribe.rs` 加測試專用存取器。
// ---------------------------------------------------------------------------

struct PidCapturingConnector {
    inner: ChildStdioConnector,
    pid: Arc<Mutex<Option<u32>>>,
}

#[async_trait]
impl Connector for PidCapturingConnector {
    async fn connect(&self) -> Result<Box<dyn NdjsonStream>, ConnectError> {
        let stream = self.inner.spawn_stream().await?;
        *self.pid.lock().expect("pid mutex poisoned") = stream.pid();
        Ok(Box::new(stream) as Box<dyn NdjsonStream>)
    }

    fn describe(&self) -> String {
        self.inner.describe()
    }
}

/// 存活探測的三態結果（fix round 1 finding 2）：「探測不出結果」（指令失敗、輸出格式看不懂、
/// 權限被拒等）跟「確定已結束」是兩件事，不能把前者悄悄併進後者——那樣探測邏輯本身壞掉時，
/// 測試會誤判成通過（就算 relay 真的變成孤兒程序也測不出來）。`ProbeError` 一律讓呼叫端
/// panic，不是靜默當成任何一種確定的結果。
#[derive(Debug)]
enum ProcessProbe {
    Alive,
    Exited,
    ProbeError(String),
}

/// 用 `tasklist` 依確切 pid 過濾（`/FO CSV`）：存活時第二欄是被雙引號包住的 pid 數字；
/// 不比對「查無工作」這類會依系統語系而異的訊息文字，改用指令的 exit status 分辨「查無此
/// pid（exit 0、輸出裡沒有這個 pid）」與「指令本身執行失敗（非 0 exit status，例如 filter
/// 語法錯誤、系統管制）」——後者是 `ProbeError`，不是 `Exited`（fix round 1 finding 2：
/// 舊版只看 `stdout.contains(pid)`，指令失敗時 stdout 一樣不含 pid，會被誤判成「已結束」）。
#[cfg(windows)]
fn probe_process(pid: u32) -> ProcessProbe {
    let output = match std::process::Command::new("tasklist")
        .args(["/FI", &format!("PID eq {pid}"), "/FO", "CSV", "/NH"])
        .output()
    {
        Ok(output) => output,
        Err(e) => return ProcessProbe::ProbeError(format!("tasklist 無法執行: {e}")),
    };
    if !output.status.success() {
        return ProcessProbe::ProbeError(format!(
            "tasklist 執行失敗（exit status {:?}）: stderr={}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    if stdout.contains(&format!("\"{pid}\"")) {
        ProcessProbe::Alive
    } else {
        ProcessProbe::Exited
    }
}

/// 讀 `/proc/<pid>/stat` 的狀態欄位：zombie（`Z`）視為已結束——與
/// `ChildStdioStream::wait_for_exit` 文件註解記載的同一個顧慮一致（`kill -0` 會把 zombie
/// 誤判成存活）；檔案不存在（`NotFound`）代表程序已結束且已被回收；其餘 I/O 錯誤（權限被拒
/// 等）或格式看不懂都回 `ProbeError`，不是 `Exited`（fix round 1 finding 2：舊版任何錯誤都
/// `unwrap_or(false)` 當成「不存活」）。只限定 Linux（含 WSL2）：`/proc` 是 Linux 專屬的
/// 虛擬檔案系統，macOS／BSD 這類其他 unix 平台沒有，見下方 `probe_process` 的
/// `not(target_os = "linux")` 分支。
#[cfg(target_os = "linux")]
fn probe_process(pid: u32) -> ProcessProbe {
    match std::fs::read_to_string(format!("/proc/{pid}/stat")) {
        Ok(stat) => {
            let state = stat
                .rsplit(')')
                .next()
                .and_then(|rest| rest.split_whitespace().next());
            match state {
                Some("Z") => ProcessProbe::Exited,
                Some(_) => ProcessProbe::Alive,
                None => ProcessProbe::ProbeError(format!(
                    "/proc/{pid}/stat 格式無法解析，讀到: {stat:?}"
                )),
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => ProcessProbe::Exited,
        Err(e) => ProcessProbe::ProbeError(format!("讀取 /proc/{pid}/stat 失敗: {e}")),
    }
}

/// 本測試的存活探測只實作了 Windows 與 Linux（含 WSL2，本 crate 實際支援的兩個平台，見
/// `AGENTS.md`／design D11）；macOS、BSD 這類其他 unix 平台沒有 `/proc`，不假裝能探測，
/// 一律回 `ProbeError`（不是靜默當成任何一種確定的結果——見 `ProcessProbe` 的文件註解）。
#[cfg(all(unix, not(target_os = "linux")))]
fn probe_process(_pid: u32) -> ProcessProbe {
    ProcessProbe::ProbeError(
        "存活探測未支援這個平台（只實作了 Windows 與 Linux／WSL2）".to_string(),
    )
}

async fn wait_until_process_exits(pid: u32, timeout: Duration) {
    let deadline = Instant::now() + timeout;
    loop {
        match probe_process(pid) {
            ProcessProbe::Exited => return,
            ProcessProbe::Alive => {}
            ProcessProbe::ProbeError(reason) => {
                panic!("存活探測失敗（pid={pid}），無法確認 relay 子程序是否已結束: {reason}");
            }
        }
        if Instant::now() >= deadline {
            panic!("relay 子程序（pid={pid}）應在 {timeout:?} 內結束，但仍存活");
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

// ---------------------------------------------------------------------------
// 驗收測試（brief 指定名稱）
// ---------------------------------------------------------------------------

#[tokio::test]
async fn child_bridge_request_roundtrip() {
    let fixture = load_json("snapshot-p22.json");
    let config = FakeHerdrConfig::new().with_snapshot_fixture_line(&fixture);
    let fake = FakeHerdr::start(config).await.expect("啟動假 HERDR 失敗");
    let endpoint = fake.endpoint_path().display().to_string();

    let connector = ChildStdioConnector::new(test_child_bin(), ["relay", &endpoint]);
    let client = Client::new(Arc::new(connector));

    let snapshot = client
        .request(SessionSnapshotRequest {})
        .await
        .expect("經子程序橋接的 session.snapshot 應成功");

    assert_eq!(snapshot.snapshot.protocol, 22);
    assert_eq!(snapshot.snapshot.workspaces.len(), 3, "workspaces 陣列長度");
    assert_eq!(snapshot.snapshot.tabs.len(), 3, "tabs 陣列長度");
    assert_eq!(snapshot.snapshot.panes.len(), 6, "panes 陣列長度");
    assert_eq!(snapshot.snapshot.agents.len(), 2, "agents 陣列長度");
}

#[tokio::test]
async fn child_bridge_stream_ends_and_child_exits() {
    let event_line = r#"{"event":"pane_created","data":{"pane_id":"wD:p1"}}"#;
    let config = FakeHerdrConfig::new()
        .with_subscribe_script(vec![Step::Event(event_line.to_string()), Step::Close]);
    let fake = FakeHerdr::start(config).await.expect("啟動假 HERDR 失敗");
    let endpoint = fake.endpoint_path().display().to_string();

    let pid_slot: Arc<Mutex<Option<u32>>> = Arc::new(Mutex::new(None));
    let connector = PidCapturingConnector {
        inner: ChildStdioConnector::new(test_child_bin(), ["relay", &endpoint]),
        pid: pid_slot.clone(),
    };
    let client = Client::new(Arc::new(connector));

    let mut stream = client
        .subscribe(&Subscription::all_lifecycle())
        .await
        .expect("經子程序橋接的訂閱應成功");

    let pid = pid_slot
        .lock()
        .expect("pid mutex poisoned")
        .expect("connect() 應已捕捉到 relay 子程序 pid");
    // 腳本在 `Event` 後緊接著就是 `Close`（沒有 `Delay`），fake HERDR 可能在測試程式跑到這裡
    // 之前就已經送完事件、關閉連線——relay 因此可能已經自然結束，不斷言這個時間點它一定還
    // 存活；真正要證明的是「stream 結束並 drop 之後，relay 最終確實會結束」，見下方
    // `wait_until_process_exits`。

    let first = stream
        .next()
        .await
        .expect("應收到假 HERDR 送出的事件")
        .expect("不應是錯誤");
    assert!(
        matches!(first, IncomingEvent::Lifecycle(EventKind::PaneCreated, _)),
        "實際: {first:?}"
    );

    let second = stream.next().await;
    assert!(
        second.is_none(),
        "Step::Close 後應正常 EOF 結束，實際: {second:?}"
    );

    drop(stream);

    wait_until_process_exits(pid, Duration::from_secs(3)).await;
}

/// 逾時上限：規格是「事件即時送達（一秒內）」，量測本身也要在約一秒後就明確失敗，不能無限期
/// 卡住（fix round 1 finding 1）。留一點緩衝（1.5x）避免在忙碌機器上把量測誤差當成逾時。
const EVENT_ARRIVAL_TIMEOUT: Duration = Duration::from_millis(1_500);

#[tokio::test]
async fn child_bridge_event_arrives_within_1s() {
    let event_line = r#"{"event":"pane_created","data":{"pane_id":"wD:p1"}}"#;
    let config = FakeHerdrConfig::new()
        .with_subscribe_script(vec![Step::Event(event_line.to_string()), Step::Hold]);
    let fake = FakeHerdr::start(config).await.expect("啟動假 HERDR 失敗");
    let endpoint = fake.endpoint_path().display().to_string();

    let connector = ChildStdioConnector::new(test_child_bin(), ["relay", &endpoint]);
    let client = Client::new(Arc::new(connector));

    let mut stream = client
        .subscribe(&Subscription::all_lifecycle())
        .await
        .expect("經子程序橋接的訂閱應成功");

    // fix round 1 finding 1：計時起點改成 subscribe() 回傳之後——這時 relay 已經啟動、
    // subscription_started 也已經收到，量到的只剩「假 HERDR 推送 → relay → client」這段，
    // 不會混入子程序啟動與訂閱 handshake 的時間。
    let started = Instant::now();
    let outcome = tokio::time::timeout(EVENT_ARRIVAL_TIMEOUT, stream.next())
        .await
        .unwrap_or_else(|_| {
            panic!(
                "應在 {EVENT_ARRIVAL_TIMEOUT:?} 內量到 next() 的結果，逾時（緩衝回歸會讓 \
                 next() 永遠不返回，不應該讓整個 cargo test 卡住）"
            )
        });
    let elapsed = started.elapsed();
    let event = outcome.expect("應收到事件").expect("不應是錯誤");
    assert!(
        matches!(event, IncomingEvent::Lifecycle(EventKind::PaneCreated, _)),
        "實際: {event:?}"
    );

    println!("child_bridge_event_arrives_within_1s: elapsed={elapsed:?}");
    assert!(
        elapsed < Duration::from_secs(1),
        "事件應在 1 秒內送達，實際: {elapsed:?}"
    );
}

/// 加碼：`Step::Delay(200ms)` 後才 `Event`，證明事件是即時轉送而不是緩衝到腳本跑完／EOF
/// 才一次送出（若被緩衝，`elapsed` 會遠小於 200ms，或因為卡在 `Hold` 而逾時——見下方
/// `tokio::time::timeout`）。
#[tokio::test]
async fn child_bridge_delayed_event_arrives_within_1s() {
    let event_line = r#"{"event":"pane_created","data":{"pane_id":"wD:p1"}}"#;
    let config = FakeHerdrConfig::new().with_subscribe_script(vec![
        Step::Delay(Duration::from_millis(200)),
        Step::Event(event_line.to_string()),
        Step::Hold,
    ]);
    let fake = FakeHerdr::start(config).await.expect("啟動假 HERDR 失敗");
    let endpoint = fake.endpoint_path().display().to_string();

    let connector = ChildStdioConnector::new(test_child_bin(), ["relay", &endpoint]);
    let client = Client::new(Arc::new(connector));

    let mut stream = client
        .subscribe(&Subscription::all_lifecycle())
        .await
        .expect("經子程序橋接的訂閱應成功");

    // fix round 1 finding 1：計時起點改成 subscribe() 回傳之後（理由同上）。
    let started = Instant::now();
    let outcome = tokio::time::timeout(EVENT_ARRIVAL_TIMEOUT, stream.next())
        .await
        .unwrap_or_else(|_| {
            panic!(
                "應在 {EVENT_ARRIVAL_TIMEOUT:?} 內量到 next() 的結果，逾時（緩衝回歸會讓 \
                 next() 永遠不返回，不應該讓整個 cargo test 卡住）"
            )
        });
    let elapsed = started.elapsed();
    let event = outcome.expect("應收到事件").expect("不應是錯誤");
    assert!(
        matches!(event, IncomingEvent::Lifecycle(EventKind::PaneCreated, _)),
        "實際: {event:?}"
    );

    println!("child_bridge_delayed_event_arrives_within_1s: elapsed={elapsed:?}");
    assert!(
        elapsed >= Duration::from_millis(180),
        "事件不應在 Delay 步驟跑完前就送達（證明不是緩衝到 EOF 才一次送出），實際: {elapsed:?}"
    );
    assert!(
        elapsed < Duration::from_secs(1),
        "事件應在 1 秒內送達，實際: {elapsed:?}"
    );
}

/// 全分支最終 review finding 3：橋接目標在握手後（已經收到過至少一行，例如
/// `subscription_started`）才因故障非零退出時，`EventStream::next()` 應該回報
/// `StreamError::Io`（含結束狀態與 stderr 內容），而不是被誤判成對面正常關閉連線
/// （`Ok(None)`／`next()` 回 `None`）。`relay-then-fail` 轉送 `subscription_started` 與
/// 1 筆事件（共 2 行）後印 stderr、以非 0（`3`）結束，模擬這個情境。
#[tokio::test]
async fn child_bridge_nonzero_exit_after_stream_is_reported_as_io_error() {
    let event_line = r#"{"event":"pane_created","data":{"pane_id":"wD:p1"}}"#;
    let config = FakeHerdrConfig::new()
        .with_subscribe_script(vec![Step::Event(event_line.to_string()), Step::Hold]);
    let fake = FakeHerdr::start(config).await.expect("啟動假 HERDR 失敗");
    let endpoint = fake.endpoint_path().display().to_string();

    let connector = ChildStdioConnector::new(test_child_bin(), ["relay-then-fail", &endpoint, "2"]);
    let client = Client::new(Arc::new(connector));

    let mut stream = client
        .subscribe(&Subscription::all_lifecycle())
        .await
        .expect(
            "經子程序橋接的訂閱應成功（relay-then-fail 在轉送 subscription_started 之前不會失敗）",
        );

    let first = stream
        .next()
        .await
        .expect("relay-then-fail 應先轉送 1 筆事件才失敗")
        .expect("第一筆不應是錯誤");
    assert!(
        matches!(first, IncomingEvent::Lifecycle(EventKind::PaneCreated, _)),
        "實際: {first:?}"
    );

    let second = tokio::time::timeout(Duration::from_secs(3), stream.next())
        .await
        .expect("relay 非零退出後應在時限內回報錯誤（不應該卡住）");
    match second {
        Some(Err(StreamError::Io(io_err))) => {
            let rendered = io_err.to_string();
            assert!(
                rendered.contains("bridge exited"),
                "原因文字應含結束狀態說明，實際: {rendered}"
            );
            assert!(
                rendered.contains("simulating bridge crash"),
                "原因文字應含子程序 stderr 內容，實際: {rendered}"
            );
        }
        other => panic!("預期 Some(Err(StreamError::Io(_)))，實際: {other:?}"),
    }

    let third = stream.next().await;
    assert!(
        third.is_none(),
        "回報錯誤之後再呼叫應該回 None（design D6），實際: {third:?}"
    );
}

/// Codex 最終 review 二次確認 finding：橋接目標先關閉 stdout、經清理或排程延遲後才以非零
/// 狀態退出時，`recv_line` 不能因為在 `POST_STREAM_EXIT_WAIT`（500ms）內等不到結束狀態，就
/// 把這個窗口誤判成正常 EOF——`relay-then-fail` 帶第 4 個參數（`close_then_sleep_ms`
/// =800，見 `src/bin/test_child.rs`）在轉送 2 行後先明確關閉自己的 stdout、睡滿 800ms（遠
/// 超過 500ms 的等待預算）才以 exit 3 結束並印 stderr，模擬這個競態窗口。指揮官裁決允許
/// 訊息含「did not exit」（逾時仍未結束那支路徑，本測試因為 800ms > 500ms 幾乎必定走這支）
/// 或含結束狀態說明（`bridge exited`，理論上等得到狀態時的路徑）兩者擇一，只要求是
/// `Err` 且原因文字非空。
#[tokio::test]
async fn child_bridge_delayed_nonzero_exit_is_reported() {
    let event_line = r#"{"event":"pane_created","data":{"pane_id":"wD:p1"}}"#;
    let config = FakeHerdrConfig::new()
        .with_subscribe_script(vec![Step::Event(event_line.to_string()), Step::Hold]);
    let fake = FakeHerdr::start(config).await.expect("啟動假 HERDR 失敗");
    let endpoint = fake.endpoint_path().display().to_string();

    let connector =
        ChildStdioConnector::new(test_child_bin(), ["relay-then-fail", &endpoint, "2", "800"]);
    let client = Client::new(Arc::new(connector));

    let mut stream = client
        .subscribe(&Subscription::all_lifecycle())
        .await
        .expect(
            "經子程序橋接的訂閱應成功（relay-then-fail 在轉送 subscription_started 之前不會失敗）",
        );

    let first = stream
        .next()
        .await
        .expect("relay-then-fail 應先轉送 1 筆事件才失敗")
        .expect("第一筆不應是錯誤");
    assert!(
        matches!(first, IncomingEvent::Lifecycle(EventKind::PaneCreated, _)),
        "實際: {first:?}"
    );

    let second = tokio::time::timeout(Duration::from_secs(3), stream.next())
        .await
        .expect("先關 stdout、延遲非零退出後應在時限內回報錯誤（不應該卡住）");
    match second {
        Some(Err(StreamError::Io(io_err))) => {
            let rendered = io_err.to_string();
            println!("child_bridge_delayed_nonzero_exit_is_reported: 實際錯誤訊息={rendered}");
            assert!(!rendered.is_empty(), "應有非空的錯誤原因文字");
            assert!(
                rendered.contains("did not exit") || rendered.contains("bridge exited"),
                "原因文字應說明逾時未結束或結束狀態，實際: {rendered}"
            );
        }
        other => panic!(
            "先關 stdout、子程序延遲非零退出時應回報 Some(Err(StreamError::Io(_)))，不應誤判成 \
             正常 EOF，實際: {other:?}"
        ),
    }
}

/// Codex 最終 review 二次確認 finding：上面那條測試證明「有延遲的非零退出」不再被誤判成
/// 正常 EOF；這條測試量測「反面」——正常 exit 0 路徑不應該因為改用有限等待就意外多付出
/// `POST_STREAM_EXIT_WAIT`（500ms）的全額延遲。腳本在 `Event` 後緊接 `Close`（沒有
/// `Delay`），計時起點放在收到事件之後——這時 fake 端幾乎已經執行到 `Close`（或即將執行），
/// 量到的主要是「relay 正常退出（exit 0，沒有任何人為 sleep）→ client 端看到 stdout
/// EOF」這段延遲；若誤付了 500ms 全額等待，會遠超過這裡的 200ms 門檻。
#[tokio::test]
async fn child_bridge_clean_exit_eof_is_prompt() {
    let event_line = r#"{"event":"pane_created","data":{"pane_id":"wD:p1"}}"#;
    let config = FakeHerdrConfig::new()
        .with_subscribe_script(vec![Step::Event(event_line.to_string()), Step::Close]);
    let fake = FakeHerdr::start(config).await.expect("啟動假 HERDR 失敗");
    let endpoint = fake.endpoint_path().display().to_string();

    let connector = ChildStdioConnector::new(test_child_bin(), ["relay", &endpoint]);
    let client = Client::new(Arc::new(connector));

    let mut stream = client
        .subscribe(&Subscription::all_lifecycle())
        .await
        .expect("經子程序橋接的訂閱應成功");

    let first = stream
        .next()
        .await
        .expect("應收到假 HERDR 送出的事件")
        .expect("不應是錯誤");
    assert!(
        matches!(first, IncomingEvent::Lifecycle(EventKind::PaneCreated, _)),
        "實際: {first:?}"
    );

    let started = Instant::now();
    let second = stream.next().await;
    let elapsed = started.elapsed();

    assert!(
        second.is_none(),
        "Step::Close 後應正常 EOF 結束，實際: {second:?}"
    );
    println!("child_bridge_clean_exit_eof_is_prompt: elapsed={elapsed:?}");
    assert!(
        elapsed < Duration::from_millis(200),
        "exit 0 不應該付出 POST_STREAM_EXIT_WAIT（500ms）的全額等待，實際: {elapsed:?}"
    );
}

#[tokio::test]
async fn relay_exits_when_endpoint_missing() {
    let missing_endpoint = std::env::temp_dir()
        .join("herdr-client-child-bridge-test")
        .join(format!(
            "missing-{}-{}.sock",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock 應晚於 UNIX_EPOCH")
                .as_nanos()
        ))
        .display()
        .to_string();

    let connector = ChildStdioConnector::new(test_child_bin(), ["relay", &missing_endpoint]);
    let client = Client::new(Arc::new(connector));

    let err = client
        .request(SessionSnapshotRequest {})
        .await
        .expect_err("endpoint 不存在時應該回傳 Err");

    match err {
        RequestError::Connect(ConnectError::ServerNotRunning { detail }) => {
            assert!(
                detail.contains(&missing_endpoint),
                "detail 應含 relay 印出、附端點路徑的原因，實際: {detail}"
            );
        }
        other => panic!("預期 RequestError::Connect(ServerNotRunning)，實際: {other:?}"),
    }
}
