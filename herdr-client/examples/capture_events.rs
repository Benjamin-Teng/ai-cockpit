//! Task 5.1：對 Windows 端 HERDR（protocol 22）擷取 p22 事件 fixture 的小工具。
//!
//! **fix round 1 finding 1（Codex task review，high，已處理）**：原本用 `Client::subscribe`，
//! 收到每筆事件後從 `IncomingEvent` 取出 `event`／`data`、再用 `serde_json::to_string` 重建
//! 一行寫進 `*.raw.ndjson`，不是 wire 上原貌。改法：不透過 `Client`／`EventStream`，直接用
//! `Connector`／`NdjsonStream`：手動組 `RequestEnvelope { method: "events.subscribe", .. }`
//! 送出、`recv_line()` 讀到什麼字串就寫什麼字串（只另外解析一份副本印 stderr 摘要，解析
//! 失敗照樣把原始行寫進 raw 檔並印 `malformed`）。
//!
//! **fix round 2 finding 1（Codex task review，high）**：round 1 的擷取迴圈用
//! `tokio::time::timeout(remaining, stream.recv_line()).await` 直接包住 `recv_line()`；
//! 逾時觸發時這個 future 被取消——`recv_line()` 底層是 `AsyncBufReadExt::read_line`，取消
//! 可能讓「已經從底層 buffer `consume()` 掉、但還沒組成完整行交回呼叫端」的位元組憑空消失
//! （這些位元組不會留在 buffer 裡等下次讀，因為 `consume()` 已經呼叫過），跟「raw 擷取不能
//! 漏掉截止邊界最後一行」的目標衝突，round 1 報告寫的「不會漏記」講過頭了。
//!
//! 改法：每條連線一個獨立的 reader task，只做一件事——`loop { stream.recv_line().await }`
//! 讀到完整的一行才送進 `tokio::sync::mpsc` channel，从不半途取消 `recv_line()` 本身；主迴圈
//! （[`capture_with_deadline`]）只決定「還要不要繼續消耗這個 channel」，不會去取消
//! 正在進行中的讀取。到了 `--seconds` 指定的 deadline 後還有一段 `grace`（200ms）：這段時間
//! 內抵達 channel 的行照樣收下，grace 結束才真正停止、讓呼叫端 drop 連線（reader task 之後
//! 再送一次會發現 channel 已關閉，自然結束）。**保證的是**：換行在 deadline + grace 之前送進
//! channel 的行必定被收下；deadline + grace 那一刻仍在傳輸中、還沒組成完整行的半行本來就
//! 沒有機會被 reader task 送進 channel，不算「漏記」，是還沒發生的事。
//!
//! **fix round 3 finding 1（Codex task review，high）**：round 2 的 grace 迴圈直接
//! `tokio::select! { rx.recv() => ..., grace_sleep => break }`。`tokio::select!` 在多個分支
//! 同時 ready 時是**隨機**選一個（不是「先看哪個」），所以就算某一行早就送進 channel、
//! `rx.recv()` 已經 ready，只要那一輪 poll 時 `grace_sleep` 也剛好到期，`select!` 仍有機會選中
//! timer 分支直接 `break`——已經排隊的行因此可能被漏收（round 2 的 `FakeHerdr` 測試把事件
//! 放在 grace 中段、故意避開這個邊界，沒蓋到這個競態）。
//!
//! 改法：抽出 [`drain_until`]——不論這一輪 `select!` 選中 `rx.recv()` 還是計時器，只要選中
//! 計時器就先用 `rx.try_recv()`（不等待，只問「現在有沒有」）把當下已經排隊、還沒被處理的行
//! 全部收乾淨，才真正停止；[`capture_with_deadline`] 的 deadline 階段與 grace 階段都呼叫這個
//! 共用函式。另外，`capture_with_deadline` 保留 reader task 的 `JoinHandle`，回傳前明確
//! `abort()` 再 `.await` 它（`JoinError::is_cancelled()` 是預期結果，其他錯誤才印出來）——
//! 不再只靠「channel 被 drop 後 reader 下一次 `send` 失敗」這種間接、時間點不確定的終止方式，
//! 確保函式回傳的當下連線（`stream`）已經真正 drop，即使 reader task 當時正卡在
//! `recv_line().await`（例如對端只回完 `subscription_started` 就 `Step::Hold` 掛著）。
//!
//! **fix round 4 finding 1（Codex scoped re-review，high）**：round 3 的收尾只把非取消的
//! `JoinError`（也就是 reader task panic）印成一行 stderr，然後照樣回傳已經收到的 `lines`。
//! 對一個「擷取 fixture」的工具來說這是最糟的失敗模式：任何 `recv_line()` 實作 panic 都會
//! 讓事件集不完整，卻寫出一份看起來正常的 `*.raw.ndjson`，資料缺漏無聲無息被帶進 repo。
//!
//! 改法：[`capture_with_deadline`] 改回傳 `Result<Vec<String>, CaptureError>`——`is_cancelled()`
//! （我們自己 `abort()` 的結果）仍是預期值，其他 `JoinError` 一律轉成
//! [`CaptureError::ReaderFailed`]（帶著 panic 訊息），已經收到的半套資料不會被當成成功結果
//! 交出去。`main()` 兩條連線只要任一條回 `Err` 就印錯誤、**一份檔都不寫**、以非 0 結束。
//!
//! 核心邏輯抽成 [`capture_with_deadline`]（不做檔案 I/O，只吃一條 `NdjsonStream` 回傳
//! `Result<Vec<String>, CaptureError>`），下面 `mod tests` 用 `herdr_client::testing::FakeHerdr`
//! 與自製的 `NdjsonStream` 替身驗證：deadline／grace 到了之後、還在緩衝期內送達的事件仍會被
//! 收下、計時器與已排隊訊息同時 ready 時不會漏收、回傳前連線確實已經 drop、reader task panic
//! 時回 `Err` 而不是半套的成功結果。
//!
//! 用法：
//!
//! ```sh
//! cargo run -p herdr-client --example capture_events -- --seconds 30
//! cargo run -p herdr-client --example capture_events -- --seconds 60 --out target/capture
//! ```
//!
//! 環境變數：`HERDR_CLIENT_TEST_WIN_SOCKET`（Windows 端 HERDR API socket 路徑，省略則用
//! `default_socket_path_from_env()`）。
//!
//! 硬性限制：全程只送 `session.snapshot`／`events.subscribe`，不執行任何會改變 HERDR 狀態的
//! 指令（`herdr-client/README.md`「真機測試」節的限制對這個工具同樣成立）。
//!
//! `events-status-p22.raw.ndjson` 需要至少一個 pane 的 agent 狀態在擷取期間真的變化才會有
//! 內容（例如某個 agent pane 從 working 變 idle）；擷取期間都沒有變化的話，這個檔案會是
//! 空的——空檔案不是這個工具的錯誤，之後由 `docs/research/2026-09-13/deidentify-fixture.py`
//! 判斷「輸入是空的」時會回報。**`*.raw.ndjson` 是 wire 原貌；去識別化之後產生的
//! `tests/fixtures/events-*-p22.ndjson` 已經過 `deidentify-fixture.py` 重新序列化（`json.load`
//! 再 `json.dump`）——去識別化本身必然要解析並改欄位值，這一步的重新序列化是可接受的，
//! 跟這裡要避免的「還沒去識別化就先被序列化改樣貌」是兩回事。**

use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

use herdr_client::connector::NdjsonStream;

/// deadline 之後還願意多等的時間（fix round 2 finding 1）：deadline 到達不代表立刻切斷，
/// 這段時間內抵達 channel 的行照樣收下，才真正結束並讓呼叫端 drop 連線。
const CAPTURE_GRACE: Duration = Duration::from_millis(200);

/// [`capture_with_deadline`] 的失敗原因（fix round 4 finding 1）。
///
/// 這個工具的產出要拿去當 fixture，所以「事件集不完整」必須是呼叫端看得見的失敗，不能只印
/// 一行 stderr 就把半套資料當成成功結果交出去。
#[derive(Debug)]
enum CaptureError {
    /// reader task 沒有正常結束——panic，或任何非「我們主動 `abort()`」造成的 `JoinError`。
    /// 內含的字串是 panic 訊息（或 `JoinError` 的描述），可以直接印給使用者看。
    ReaderFailed(String),
}

impl std::fmt::Display for CaptureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ReaderFailed(detail) => {
                write!(f, "reader task 非預期結束，擷取到的事件集不完整: {detail}")
            }
        }
    }
}

impl std::error::Error for CaptureError {}

/// 把非取消的 `JoinError` 轉成人看得懂的字串：panic payload 是 `&'static str`（`panic!` 帶
/// 字面值）或 `String`（`panic!` 帶格式化參數）時取出原訊息，其他型別就標記為取不出。
fn describe_join_error(error: tokio::task::JoinError) -> String {
    if !error.is_panic() {
        return format!("非 panic 的 JoinError: {error}");
    }
    let payload = error.into_panic();
    if let Some(message) = payload.downcast_ref::<&'static str>() {
        format!("panic: {message}")
    } else if let Some(message) = payload.downcast_ref::<String>() {
        format!("panic: {message}")
    } else {
        "panic（payload 不是字串，取不出訊息）".to_string()
    }
}

struct Args {
    seconds: u64,
    out_dir: PathBuf,
}

fn print_usage() {
    eprintln!("用法: capture_events [--seconds N] [--out <dir>]");
    eprintln!();
    eprintln!("  --seconds N   擷取秒數（預設 60）");
    eprintln!("  --out <dir>   輸出目錄（預設 <workspace>/target/capture）");
    eprintln!("  -h, --help    印出本說明並離開，不連線");
    eprintln!();
    eprintln!("環境變數:");
    eprintln!(
        "  HERDR_CLIENT_TEST_WIN_SOCKET（Windows 端 HERDR API socket 路徑，\
         預設 default_socket_path_from_env()）"
    );
}

fn default_out_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("herdr-client 應有上層目錄")
        .join("target")
        .join("capture")
}

/// 在讀任何環境變數或連線之前完整解析 argv（同 `spike4_no_window.rs` 的教訓：未知參數
/// 一律視為錯誤，不悄悄落入預設值）。
fn parse_args() -> Args {
    let mut seconds = 60u64;
    let mut out_dir: Option<PathBuf> = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => {
                print_usage();
                std::process::exit(0);
            }
            "--seconds" => {
                let value = args.next().unwrap_or_else(|| {
                    eprintln!("--seconds 需要一個數值參數");
                    print_usage();
                    std::process::exit(2);
                });
                seconds = value.parse().unwrap_or_else(|e| {
                    eprintln!("--seconds 參數不是合法整數: {e}");
                    std::process::exit(2);
                });
            }
            "--out" => {
                let value = args.next().unwrap_or_else(|| {
                    eprintln!("--out 需要一個路徑參數");
                    print_usage();
                    std::process::exit(2);
                });
                out_dir = Some(PathBuf::from(value));
            }
            other => {
                eprintln!("unknown argument: {other}");
                print_usage();
                std::process::exit(2);
            }
        }
    }
    Args {
        seconds,
        out_dir: out_dir.unwrap_or_else(default_out_dir),
    }
}

/// 解析一份「副本」印 stderr 摘要；解析失敗一樣照常收下這一行（不影響擷取結果，只是摘要
/// 印不出事件名稱）。
fn print_summary(label: &str, line: &str) {
    match serde_json::from_str::<serde_json::Value>(line) {
        Ok(value) => {
            let event_name = value.get("event").and_then(serde_json::Value::as_str);
            eprintln!(
                "[{label}] {}: {line}",
                event_name.unwrap_or("<no event field>")
            );
        }
        Err(e) => {
            eprintln!("[{label}] malformed（{e}）: {line}");
        }
    }
}

/// fix round 3 finding 1 的核心修正：不論 `select!` 這一輪選中 `rx.recv()` 還是 `timer`，
/// 只要選中 `timer` 就先用 `rx.try_recv()`（不等待）把當下已經排隊、還沒被處理的行收乾淨，
/// 才真正停止——`tokio::select!` 在多個分支同時 ready 時是隨機選的，只在 timer 分支寫
/// `break` 會讓「已經送進 channel、只是這一輪運氣不好沒被選中」的行留在原地沒收。
///
/// 回傳 `true` 代表是因為 channel 關閉（reader task 已經自然結束：EOF／錯誤／送不出去）而
/// 停止，不是 timer 到期；呼叫端用這個判斷「channel 都已經關了，還要不要進入下一階段」。
///
/// 泛型接受任何 `Future<Output = ()>` 而不是寫死 `tokio::time::Sleep`，方便測試直接餵一個
/// 建立當下就已經到期的計時器，不需要仰賴不確定的即時排程去重現「timer 與已排隊訊息同時
/// ready」這個競態窗口。
async fn drain_until<F>(
    rx: &mut tokio::sync::mpsc::UnboundedReceiver<String>,
    mut timer: std::pin::Pin<&mut F>,
    label: &str,
    lines: &mut Vec<String>,
) -> bool
where
    F: std::future::Future<Output = ()>,
{
    loop {
        tokio::select! {
            maybe_line = rx.recv() => {
                match maybe_line {
                    Some(line) => {
                        print_summary(label, &line);
                        lines.push(line);
                    }
                    None => return true,
                }
            }
            () = &mut timer => break,
        }
    }
    while let Ok(line) = rx.try_recv() {
        print_summary(label, &line);
        lines.push(line);
    }
    false
}

/// 不用 `timeout` 直接包住 `recv_line()`（fix round 2 finding 1：會取消非 cancellation-safe
/// 的逐行讀取，可能遺失半行狀態）。改成獨立 reader task 只管把 `recv_line()` 讀到的完整行
/// 送進 channel，`.await` 從不被取消；這個函式只決定「還要不要繼續收」：`deadline` 之前收到
/// 就收，`deadline` 到了之後還有 `grace` 的緩衝期，grace 結束才真正停止。
///
/// 兩個階段都呼叫 [`drain_until`]（fix round 3 finding 1）：deadline 階段如果因為 channel
/// 關閉而結束就直接回傳，不再進入 grace 階段（連線都已經沒了，等不到更多東西）；否則進入
/// grace 階段。結束前明確 `abort()` reader task 的 `JoinHandle` 再 `.await` 它，確保函式
/// 回傳的當下 `stream` 已經真正 drop（不論 reader task 那時是自然結束、還是正卡在
/// `recv_line().await` 被強制中止）。
///
/// 不做任何檔案 I/O，只回傳收到的原始行（未經解析、未經重新序列化），供呼叫端決定要不要
/// 寫檔——這個切割讓下面 `mod tests` 可以直接餵一條 `FakeHerdr` 連線驗證行為，不需要真的碰
/// 檔案系統。
///
/// fix round 4 finding 1：reader task 若 panic，回傳 [`CaptureError::ReaderFailed`] 而不是
/// 已經收到的那幾行——事件集既然不完整，呼叫端就不該拿到一個「成功」的 `Vec<String>`。
async fn capture_with_deadline(
    mut stream: Box<dyn NdjsonStream>,
    label: &str,
    deadline: tokio::time::Instant,
    grace: Duration,
) -> Result<Vec<String>, CaptureError> {
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    let reader_label = label.to_string();
    let reader_handle = tokio::spawn(async move {
        loop {
            match stream.recv_line().await {
                Ok(Some(line)) => {
                    if tx.send(line).is_err() {
                        break; // 呼叫端已經不再收，channel 關閉。
                    }
                }
                Ok(None) => {
                    eprintln!("[{reader_label}] 連線已結束（EOF）");
                    break;
                }
                Err(e) => {
                    eprintln!("[{reader_label}] 讀取失敗: {e}");
                    break;
                }
            }
        }
        // `stream` 隨這個 task 結束一起 drop，連線終止。
    });

    let mut lines = Vec::new();

    let sleep_until_deadline = tokio::time::sleep_until(deadline);
    tokio::pin!(sleep_until_deadline);
    let channel_closed =
        drain_until(&mut rx, sleep_until_deadline.as_mut(), label, &mut lines).await;

    if !channel_closed {
        // deadline 已到：再給 grace 的緩衝期，把 deadline 前後、已經送進 channel 的完整行
        // 收下；這一刻仍在傳輸中、還沒組成完整行的半行本來就還沒進 channel，不算漏記。
        let grace_sleep = tokio::time::sleep(grace);
        tokio::pin!(grace_sleep);
        drain_until(&mut rx, grace_sleep.as_mut(), label, &mut lines).await;
    }

    // fix round 3 finding 1(b)：明確終止 reader task 並等它結束，確認連線在這個函式回傳前
    // 一定已經 drop。已經自然結束的 task 被 `abort()` 是無效操作（tokio 文件：對已完成的
    // task 呼叫 `abort()` 什麼都不做），`.await` 照樣拿得到它原本的結果——包含 panic。
    //
    // fix round 4 finding 1：`is_cancelled()`（我們自己 abort 的）才是預期結果；其他
    // `JoinError` 代表 reader task panic 了，收到的事件集必然不完整，一律回 `Err` 讓呼叫端
    // 決定怎麼辦，不能印一行 stderr 之後照樣把 `lines` 當成功結果交出去。
    reader_handle.abort();
    if let Err(e) = reader_handle.await
        && !e.is_cancelled()
    {
        let detail = describe_join_error(e);
        eprintln!("[{label}] reader task 非預期結束: {detail}");
        return Err(CaptureError::ReaderFailed(detail));
    }

    Ok(lines)
}

/// 擷取完成後才把收到的原始行整批寫進 `out_path`（每行逐字寫入，不重新解析／序列化）。
/// 事件量在這個工具的使用情境下（幾十秒到幾分鐘、至多幾百筆）很小，整批寫入不是問題；
/// 這個切割讓 [`capture_with_deadline`] 保持不碰檔案系統、方便測試。
///
/// fix round 4 finding 1：只有在**每一條**連線都擷取成功之後才會呼叫到這裡——任何一條回
/// `Err` 就完全不寫檔，不留下看起來正常、其實少了事件的 raw fixture。
fn write_lines(lines: &[String], out_path: &Path) -> usize {
    let file = std::fs::File::create(out_path)
        .unwrap_or_else(|e| panic!("建立 {} 失敗: {e}", out_path.display()));
    let mut writer = std::io::BufWriter::new(file);
    for line in lines {
        writeln!(writer, "{line}")
            .unwrap_or_else(|e| panic!("寫入 {} 失敗: {e}", out_path.display()));
    }
    writer
        .flush()
        .unwrap_or_else(|e| panic!("flush {} 失敗: {e}", out_path.display()));

    lines.len()
}

#[cfg(not(windows))]
fn main() {
    eprintln!(
        "capture_events 只對 Windows 端 HERDR（named pipe）擷取 p22 事件，本平台無此概念，略過。"
    );
}

#[cfg(windows)]
#[tokio::main]
async fn main() {
    use herdr_client::connector::{Connector, NamedPipeConnector, default_socket_path_from_env};
    use herdr_client::types::{
        EventsSubscribeParams, RequestEnvelope, ResponseEnvelope, Subscription,
    };

    fn win_socket_path() -> PathBuf {
        std::env::var("HERDR_CLIENT_TEST_WIN_SOCKET")
            .map(PathBuf::from)
            .unwrap_or_else(|_| default_socket_path_from_env())
    }

    /// 開一條全新連線、送一次 `session.snapshot`、回傳解析後的完整回應（`Value`）。只在
    /// 啟動時查一次全部 pane id，供 S 訂閱使用；跟 L／S 訂閱連線各自獨立，用完即 drop。
    async fn request_snapshot(connector: &NamedPipeConnector) -> serde_json::Value {
        let mut stream = connector
            .connect()
            .await
            .unwrap_or_else(|e| panic!("session.snapshot 連線失敗: {e}"));
        let envelope = RequestEnvelope {
            id: "snap",
            method: "session.snapshot",
            params: &serde_json::json!({}),
        };
        let line = serde_json::to_string(&envelope).expect("Value 序列化不會失敗");
        stream
            .send_line(&line)
            .await
            .unwrap_or_else(|e| panic!("送出 session.snapshot 失敗: {e}"));
        let response_line = stream
            .recv_line()
            .await
            .unwrap_or_else(|e| panic!("讀取 session.snapshot 回應失敗: {e}"))
            .unwrap_or_else(|| panic!("連線在收到 session.snapshot 回應前就 EOF"));
        serde_json::from_str(&response_line)
            .unwrap_or_else(|e| panic!("session.snapshot 回應不是合法 JSON: {e}\n{response_line}"))
    }

    /// 開一條全新連線、送 `events.subscribe`、讀第一行確認是 `subscription_started`（解析
    /// 一份副本，不影響呼叫端拿到的連線本身），回傳仍開著的連線給呼叫端逐字讀事件行。
    async fn open_subscription(
        connector: &NamedPipeConnector,
        request_id: &str,
        subscriptions: Vec<Subscription>,
    ) -> Box<dyn NdjsonStream> {
        let mut stream = connector
            .connect()
            .await
            .unwrap_or_else(|e| panic!("[{request_id}] 連線失敗: {e}"));
        let params = EventsSubscribeParams { subscriptions };
        let envelope = RequestEnvelope {
            id: request_id,
            method: "events.subscribe",
            params: &params,
        };
        let line = serde_json::to_string(&envelope).expect("Value 序列化不會失敗");
        stream
            .send_line(&line)
            .await
            .unwrap_or_else(|e| panic!("[{request_id}] 送出 events.subscribe 失敗: {e}"));
        let response_line = stream
            .recv_line()
            .await
            .unwrap_or_else(|e| panic!("[{request_id}] 讀取 subscription 回應失敗: {e}"))
            .unwrap_or_else(|| panic!("[{request_id}] 連線在收到 subscription_started 前就 EOF"));
        let response: ResponseEnvelope = serde_json::from_str(&response_line).unwrap_or_else(|e| {
            panic!("[{request_id}] subscription 回應不是合法回應信封: {e}\n{response_line}")
        });
        let is_started = response
            .result
            .as_ref()
            .and_then(|r| r.get("type"))
            .and_then(serde_json::Value::as_str)
            == Some("subscription_started");
        if !is_started {
            panic!("[{request_id}] 訂閱未成功: {response_line}");
        }
        stream
    }

    let args = parse_args();
    let connector = NamedPipeConnector::new(win_socket_path());

    let snapshot = request_snapshot(&connector).await;
    let pane_ids: Vec<String> = snapshot["result"]["snapshot"]["panes"]
        .as_array()
        .unwrap_or_else(|| panic!("snapshot.panes 應為陣列，實際: {snapshot}"))
        .iter()
        .map(|p| {
            p["pane_id"]
                .as_str()
                .unwrap_or_else(|| panic!("pane 應有 pane_id，實際: {p}"))
                .to_string()
        })
        .collect();
    eprintln!("目前 pane 數: {}（S 訂閱涵蓋全部）", pane_ids.len());

    let l_stream = open_subscription(&connector, "L", Subscription::all_lifecycle()).await;
    let status_subs: Vec<Subscription> = pane_ids
        .iter()
        .map(|id| Subscription::PaneAgentStatusChanged {
            pane_id: id.clone(),
        })
        .collect();
    let s_stream = open_subscription(&connector, "S", status_subs).await;

    std::fs::create_dir_all(&args.out_dir)
        .unwrap_or_else(|e| panic!("建立輸出目錄 {} 失敗: {e}", args.out_dir.display()));
    let lifecycle_path = args.out_dir.join("events-lifecycle-p22.raw.ndjson");
    let status_path = args.out_dir.join("events-status-p22.raw.ndjson");

    eprintln!(
        "擷取 {} 秒（另加 {}ms grace），輸出目錄: {}",
        args.seconds,
        CAPTURE_GRACE.as_millis(),
        args.out_dir.display()
    );

    // L、S 共用同一個 deadline（各自的 connector 拿到的時間點幾乎相同，這裡明確共用一個
    // `Instant`，避免兩條各自算一次造成的些微時間差）。
    let deadline = tokio::time::Instant::now() + Duration::from_secs(args.seconds);
    let (l_result, s_result) = tokio::join!(
        capture_with_deadline(l_stream, "L", deadline, CAPTURE_GRACE),
        capture_with_deadline(s_stream, "S", deadline, CAPTURE_GRACE),
    );

    // fix round 4 finding 1：任一條連線的 reader task 非預期結束＝事件集不完整。印出失敗原因、
    // 一份 `*.raw.ndjson` 都不寫、以非 0 結束——這個工具的產出是要拿去當 fixture 的，把半套
    // 資料寫成看起來正常的檔案，比直接失敗糟糕得多。
    let (l_lines, s_lines) = match (l_result, s_result) {
        (Ok(l_lines), Ok(s_lines)) => (l_lines, s_lines),
        (l_result, s_result) => {
            for (label, result) in [("L", l_result), ("S", s_result)] {
                if let Err(e) = result {
                    eprintln!("[{label}] 擷取失敗: {e}");
                }
            }
            eprintln!(
                "擷取失敗，兩份 *.raw.ndjson 都不寫入（不把不完整的事件集當成成功的擷取結果）。"
            );
            std::process::exit(1);
        }
    };

    let l_count = write_lines(&l_lines, &lifecycle_path);
    let s_count = write_lines(&s_lines, &status_path);

    eprintln!("完成。L 收到 {l_count} 筆 → {}", lifecycle_path.display());
    eprintln!("完成。S 收到 {s_count} 筆 → {}", status_path.display());
    if s_count == 0 {
        eprintln!(
            "S 是空的：擷取期間沒有任何 pane 的 agent 狀態變化，這不是錯誤——需要真的有一個\
             agent pane 在這段時間內變化狀態（例如 working → idle）才會有內容。"
        );
    }
}

/// fix round 2／3：`capture_with_deadline`／`drain_until` 是這兩輪修正要驗證的核心，用
/// `herdr_client::testing::FakeHerdr` 控制時序——不需要真機、也不需要 Windows named pipe。
/// 由 `[[example]] test = true`（見 `Cargo.toml`）讓 `cargo test -p herdr-client` 建置並執行
/// 這個測試（已實測確認：見 task 5.1 report「Fix round 2」的 `cargo test` 輸出證據）。
#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    use herdr_client::testing::{FakeHerdr, FakeHerdrConfig, Step};

    use super::*;

    async fn open_test_subscription(fake: &FakeHerdr) -> Box<dyn NdjsonStream> {
        let mut stream = fake.connector().connect().await.expect("連線假 HERDR 失敗");
        let line = serde_json::json!({
            "id": "t",
            "method": "events.subscribe",
            "params": {},
        })
        .to_string();
        stream
            .send_line(&line)
            .await
            .expect("送出 events.subscribe 失敗");
        let started = stream
            .recv_line()
            .await
            .expect("讀取 subscription 回應失敗")
            .expect("連線在收到 subscription_started 前就 EOF");
        assert!(
            started.contains("subscription_started"),
            "應收到 subscription_started，實際: {started}"
        );
        stream
    }

    /// deadline 到了之後、grace 結束之前送達的事件仍會被收下（fix round 2 finding 1 要保住
    /// 的行為）：`Step::Delay` 刻意設在 deadline 之後、grace 結束之前，驗證第二筆事件不會因為
    /// 「deadline 一到就直接砍斷」而遺失。
    #[tokio::test]
    async fn capture_with_deadline_drains_events_that_arrive_within_grace() {
        let deadline_budget = Duration::from_millis(50);
        let grace = Duration::from_millis(300);
        // delay 選在 deadline 之後、grace 結束之前，留出充分餘裕避免測試在慢機器上偶發失敗。
        let delay_after_first_event = Duration::from_millis(150);
        assert!(delay_after_first_event > deadline_budget);
        assert!(delay_after_first_event < grace);

        let config = FakeHerdrConfig::new().with_subscribe_script(vec![
            Step::Event(r#"{"event":"first","data":{}}"#.to_string()),
            Step::Delay(delay_after_first_event),
            Step::Event(r#"{"event":"second","data":{}}"#.to_string()),
            Step::Hold,
        ]);
        let fake = FakeHerdr::start(config).await.expect("啟動假 HERDR 失敗");
        let stream = open_test_subscription(&fake).await;

        let deadline = tokio::time::Instant::now() + deadline_budget;
        let lines = capture_with_deadline(stream, "T", deadline, grace)
            .await
            .expect("reader task 正常結束時不應回 Err");

        assert_eq!(
            lines.len(),
            2,
            "deadline 之後、grace 結束之前送達的第二筆事件也應該被收下，實際收到: {lines:?}"
        );
        assert!(lines[0].contains("\"first\""), "第一筆: {}", lines[0]);
        assert!(lines[1].contains("\"second\""), "第二筆: {}", lines[1]);
    }

    /// grace 結束之後才送達的事件不會被收下——不是「永遠等下去」，grace 仍然是個真正的
    /// 上限。
    #[tokio::test]
    async fn capture_with_deadline_does_not_wait_past_grace() {
        let deadline_budget = Duration::from_millis(30);
        let grace = Duration::from_millis(100);
        let delay_after_first_event = Duration::from_millis(300); // 遠超過 deadline + grace。

        let config = FakeHerdrConfig::new().with_subscribe_script(vec![
            Step::Event(r#"{"event":"first","data":{}}"#.to_string()),
            Step::Delay(delay_after_first_event),
            Step::Event(r#"{"event":"second","data":{}}"#.to_string()),
            Step::Hold,
        ]);
        let fake = FakeHerdr::start(config).await.expect("啟動假 HERDR 失敗");
        let stream = open_test_subscription(&fake).await;

        let deadline = tokio::time::Instant::now() + deadline_budget;
        let started_at = tokio::time::Instant::now();
        let lines = capture_with_deadline(stream, "T", deadline, grace)
            .await
            .expect("reader task 正常結束時不應回 Err");
        let elapsed = started_at.elapsed();

        assert_eq!(
            lines,
            vec![r#"{"event":"first","data":{}}"#.to_string()],
            "grace 結束後才送達的第二筆事件不應該被收下"
        );
        assert!(
            elapsed < delay_after_first_event,
            "不應該一路等到遠超過 grace 之後才送達的事件，實際等了 {elapsed:?}"
        );
    }

    /// fix round 3 finding 1：直接測 [`drain_until`]，不透過真的連線或即時排程。
    ///
    /// 一開始試過 `tokio::time::sleep(Duration::ZERO)` 當計時器，發現靠不住：實測（見
    /// task 5.1 report「Fix round 3」）顯示這種計時器不保證在**第一次** poll 就回
    /// `Ready`——它似乎要等 tokio 的時間驅動器跑過一輪才會被標記到期，這段時間內
    /// `rx.recv()`（緩衝訊息本來就同步 ready）已經先贏了好幾輪，這個測試不管有沒有
    /// 修好都會通過，測不出東西。
    ///
    /// 改用 `std::future::ready(())`：這個 future 保證「第一次被 poll 到」那一刻必定是
    /// `Ready`，不靠任何時間驅動細節。搭配一次送進 64 筆訊息，讓「`tokio::select!` 連續
    /// 64 輪都隨機選中 `rx.recv()`、始終沒輪到 poll 這個計時器分支」的機率趨近於零
    /// （2^-64）——這個計時器遲早會被 `select!` 選中，而它一旦被選中就必定是它第一次被
    /// poll、也必定是 `Ready`（`select!` 不會提前把它跟 `rx.recv()` 那樣先摸過一輪
    /// `Pending` 才輪到，一輪只會恰好 poll 到讓那一輪勝出的那個分支，見下面 `Ready`
    /// 不會被 poll 兩次的推理），逼近確定性地重現 finding 1 描述的競態：那個當下 channel
    /// 裡幾乎一定還有訊息排隊，驗證 `drain_until` 仍然會用 `try_recv()` 把剩下的收乾淨，
    /// 不會因為 `select!` 選中計時器分支就漏掉。
    ///
    /// （`std::future::Ready<T>` 被 poll 兩次會 panic——但這裡不會發生：`tokio::select!`
    /// 每一輪只會照隨機順序逐一 poll 分支直到找到第一個 `Ready` 就停手，不會繼續 poll
    /// 排在它後面的分支；我們的計時器只要曾經被 poll 到就一定 `Ready` 並讓 `drain_until`
    /// 立刻 `break`、不再回頭，所以它一輩子最多只會被 poll 一次。）
    #[tokio::test]
    async fn drain_until_collects_messages_already_queued_when_timer_already_elapsed() {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();
        let expected: Vec<String> = (0..64).map(|i| format!("msg-{i}")).collect();
        for msg in &expected {
            tx.send(msg.clone()).expect("送出不應失敗（receiver 還在）");
        }

        let mut lines = Vec::new();
        let mut timer_ready = std::future::ready(());
        let timer = std::pin::Pin::new(&mut timer_ready);
        let channel_closed = drain_until(&mut rx, timer, "T", &mut lines).await;

        assert!(!channel_closed, "channel 沒有關閉，應該是計時器分支結束的");
        assert_eq!(
            lines, expected,
            "計時器到期當下已經排隊的訊息都應該被收下，不能因為 select! 隨機選中計時器分支\
             就漏掉"
        );
    }

    /// 測試用的 `NdjsonStream` 包裝，drop 時把旗標設成 `true`——用來驗證
    /// [`capture_with_deadline`] 回傳前，內部連線是否真的已經終止（fix round 3 finding 1(b)）。
    struct DropFlagStream {
        inner: Box<dyn NdjsonStream>,
        dropped: Arc<AtomicBool>,
    }

    impl Drop for DropFlagStream {
        fn drop(&mut self) {
            self.dropped.store(true, Ordering::SeqCst);
        }
    }

    #[async_trait::async_trait]
    impl NdjsonStream for DropFlagStream {
        async fn send_line(&mut self, line: &str) -> std::io::Result<()> {
            self.inner.send_line(line).await
        }

        async fn recv_line(&mut self) -> std::io::Result<Option<String>> {
            self.inner.recv_line().await
        }
    }

    /// fix round 3 finding 1(b)：`capture_with_deadline` 回傳前，reader task 必須已經結束、
    /// 連線（`stream`）必須已經 drop——不能只靠「channel 被 drop 後下一次 send 失敗」這種
    /// 間接、時間點不確定的終止方式。這裡故意用 `Step::Hold`（對端只回
    /// `subscription_started` 後就掛著，不再送任何東西），逼 reader task 卡在
    /// `recv_line().await` 裡；`capture_with_deadline` 仍應該在 deadline + grace 後準時
    /// 回傳，並確保連線已經 drop（否則測試會因為 `dropped` 還是 `false` 而失敗）。
    #[tokio::test]
    async fn capture_with_deadline_drops_stream_before_returning() {
        let config = FakeHerdrConfig::new().with_subscribe_script(vec![Step::Hold]);
        let fake = FakeHerdr::start(config).await.expect("啟動假 HERDR 失敗");
        let stream = open_test_subscription(&fake).await;

        let dropped = Arc::new(AtomicBool::new(false));
        let wrapped: Box<dyn NdjsonStream> = Box::new(DropFlagStream {
            inner: stream,
            dropped: Arc::clone(&dropped),
        });

        let deadline = tokio::time::Instant::now() + Duration::from_millis(20);
        let lines = capture_with_deadline(wrapped, "T", deadline, Duration::from_millis(50))
            .await
            .expect("reader task 正常結束時不應回 Err");

        assert!(lines.is_empty(), "Hold 腳本不會再送任何事件");
        assert!(
            dropped.load(Ordering::SeqCst),
            "capture_with_deadline 回傳時連線應該已經被 drop（reader task 已終止）"
        );
    }

    /// 只在測試裡用的 `NdjsonStream` 替身：第一次 `recv_line()` 正常回一行，第二次 panic，
    /// 模擬「reader task 半途 panic」——已經收到第一行、但事件集其實不完整。`emitted` 讓
    /// 測試可以確認 panic 之前真的有交出過一行（否則就不是「半套資料」情境了）。
    struct PanickingStream {
        calls: usize,
        emitted: Arc<AtomicUsize>,
    }

    #[async_trait::async_trait]
    impl NdjsonStream for PanickingStream {
        async fn send_line(&mut self, _line: &str) -> std::io::Result<()> {
            Ok(())
        }

        async fn recv_line(&mut self) -> std::io::Result<Option<String>> {
            self.calls += 1;
            if self.calls == 1 {
                self.emitted.fetch_add(1, Ordering::SeqCst);
                Ok(Some(r#"{"event":"first","data":{}}"#.to_string()))
            } else {
                panic!("測試用 reader panic（第 2 次 recv_line）");
            }
        }
    }

    /// fix round 4 finding 1：reader task panic 之後，已經收到的半套資料不可以被當成成功的
    /// 擷取結果回傳給呼叫端——必須回 `Err`，呼叫端才有機會不寫檔、以非 0 結束。
    ///
    /// 這個替身在**第二次** `recv_line()` 才 panic，所以第一行確實已經進了 channel：測試要
    /// 證明的正是「已經收到一行」也不能讓這次擷取被當成成功。
    #[tokio::test]
    async fn capture_with_deadline_reports_reader_panic_instead_of_partial_success() {
        let emitted = Arc::new(AtomicUsize::new(0));
        let stream: Box<dyn NdjsonStream> = Box::new(PanickingStream {
            calls: 0,
            emitted: Arc::clone(&emitted),
        });
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        let result = capture_with_deadline(stream, "T", deadline, Duration::from_millis(50)).await;

        // 先確認這次真的是「已經收到一行才 panic」的半套情境，而不是第一次呼叫就炸掉、
        // 根本沒有資料可以被誤當成功的退化案例（`capture_with_deadline` 已經 await 過
        // reader task，這個計數的寫入必定 happens-before 這裡的讀取）。
        assert_eq!(
            emitted.load(Ordering::SeqCst),
            1,
            "替身應該在 panic 之前先交出過一行"
        );

        let error = result.expect_err("reader task panic 不該被當成成功的擷取結果");
        let CaptureError::ReaderFailed(detail) = &error;
        assert!(
            detail.contains("測試用 reader panic"),
            "錯誤訊息應帶著原本的 panic 訊息，實際: {detail}"
        );
        assert!(
            error.to_string().contains("事件集不完整"),
            "Display 應說清楚失敗的後果，實際: {error}"
        );
    }
}
