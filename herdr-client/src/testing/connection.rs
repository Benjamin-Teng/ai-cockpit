//! 單條連線的收發邏輯：讀第一行決定 method，依 `FakeHerdrConfig` 產生回應（design D7、D12；
//! spec `herdr-request`「回應對應」、`herdr-event-subscription`「訂閱建立」「壞行與結束」）。

use std::collections::HashSet;
use std::sync::Arc;

use serde::Deserialize;
use tokio::io::{AsyncRead, AsyncWrite, BufReader};

use crate::connector::stream::{read_one_line, write_line};
use crate::types::{EventsSubscribeParams, Subscription};

use super::config::{FakeHerdrConfig, MethodResponse};
use super::listener::SharedState;
use super::script::Step;

/// `Step::Abort` 時嘗試讓對端看到「非正常中斷」而非乾淨 EOF；各 transport 各自決定用哪個
/// 底層呼叫，做不到時就是單純 drop。
///
/// fix round 1 finding 1：Windows 用 `NamedPipeServer::disconnect()` 實測確實讓對端下一次
/// `recv_line()` 拿到 I/O 錯誤（見 task 4.1 報告、`subscribe_script_abort_yields_io_error`）。
/// unix 沒有對應做法——已查證兩個方向都不成立：
/// 1. tokio 的 `UnixStream`（以及底層 `std::os::unix::net::UnixStream`）沒有公開
///    `set_linger`／`SO_LINGER`；要用就得繞去原始 fd 手動 `setsockopt`，等於為了一個
///    test-only 的邊界行為引入 unsafe FFI 或新依賴（`libc` crate），不符合 KISS。
/// 2. Linux kernel selftests（`af_unix`，2025 年新增，見
///    <https://lkml.iu.edu/2511.1/07310.html>）記載的行為是：SOCK_STREAM 的 AF_UNIX 連線在
///    「關閉時自己那端還有未讀資料」才會讓對端看到 `ECONNRESET`，正常關閉（沒有未讀資料）
///    一律是 EOF。這個條件要靠「client 端刻意留一段假 HERDR 永遠不會再讀的位元組」才能觸發，
///    不是 `Step::Abort` 能單方面在 server 端做到的事（`events.subscribe` 連線的協定本來就
///    只有 client 送一行、之後只由 server 單向推事件，沒有「client 還有東西沒被讀」這種
///    情境）。
///
/// 因此 unix 上 `Step::Abort` 降級為與 `Step::Close` 相同的乾淨 EOF（平台限制，非本 crate
/// 能力不足）；`#[cfg(unix)] subscribe_script_abort_degrades_to_eof` 明確驗證並記錄這件事。
pub(super) trait BestEffortAbort {
    fn best_effort_abort(&self);
}

#[cfg(windows)]
impl BestEffortAbort for tokio::net::windows::named_pipe::NamedPipeServer {
    fn best_effort_abort(&self) {
        let _ = self.disconnect();
    }
}

#[cfg(unix)]
impl BestEffortAbort for tokio::net::UnixStream {
    fn best_effort_abort(&self) {
        // 平台限制，做不到「非乾淨中斷」；見上方 trait 文件註解的查證來源。直接 drop。
    }
}

#[derive(Debug, Deserialize)]
struct IncomingRequest {
    id: String,
    method: String,
    #[serde(default)]
    params: serde_json::Value,
}

/// 這條連線結束時（handler 正常返回，或被 `FakeHerdr::drop` 的 `abort_all()` 取消而讓整個
/// future 被 drop）在 `SharedState` 記下「已關閉」，供 `FakeHerdr::closed_connections` 觀察。
///
/// 用 drop guard 而不是在函式結尾寫一行：被 abort 的 task 不會跑到函式結尾，只會被 drop，
/// 兩種結束方式都要能標記到。
struct CloseGuard {
    shared: Arc<SharedState>,
    conn_index: usize,
}

impl Drop for CloseGuard {
    fn drop(&mut self) {
        self.shared.mark_closed(self.conn_index);
    }
}

/// 處理一條連線：讀第一行決定 method，記錄下來，再依 method 分派。
///
/// fix round 1 finding 2：不再接收協作式的 shutdown 訊號——`FakeHerdr::drop` 改成直接
/// `abort_all()` 這個 task（透過 listener 收集的 `JoinSet`），不論卡在讀第一行、
/// `Step::Delay` 的 sleep、還是 `Step::Hold` 的等待，都會在下一個排程點被強制取消並 drop 掉
/// 底層連線，不需要這個函式自己配合檢查訊號。
pub(super) async fn handle_connection<T>(
    mut io: BufReader<T>,
    config: Arc<FakeHerdrConfig>,
    shared: Arc<SharedState>,
    conn_index: usize,
) where
    T: AsyncRead + AsyncWrite + Unpin + BestEffortAbort + Send + 'static,
{
    let _close_guard = CloseGuard {
        shared: shared.clone(),
        conn_index,
    };

    let Ok(Some(first_line)) = read_one_line(&mut io).await else {
        return;
    };
    shared.record_line(conn_index, first_line.clone());

    let Ok(request) = serde_json::from_str::<IncomingRequest>(&first_line) else {
        return;
    };

    // live-output task 3.3：從收到這行到這次呼叫結束（回完應、關閉連線，或 handler 被
    // abort）為止，整段都計入這個 method 的並發數；`_concurrency_guard` drop 時自動 -1。
    let _concurrency_guard = shared.note_call_start(&request.method);

    if request.method == "events.subscribe" {
        handle_subscribe(&mut io, &request, &config).await;
    } else {
        handle_generic_request(&mut io, &request, &config).await;
    }
}

async fn handle_generic_request<T>(
    io: &mut BufReader<T>,
    request: &IncomingRequest,
    config: &FakeHerdrConfig,
) where
    T: AsyncRead + AsyncWrite + Unpin + BestEffortAbort,
{
    // live-output task 3.3：`Delayed` 先睡再攤平成內層的 `MethodResponse`（可巢狀），睡眠期間
    // 這條連線仍計入呼叫端登記的並發數（`handle_connection` 的 `_concurrency_guard`）。
    let mut response = config.response_for(&request.method).cloned();
    while let Some(MethodResponse::Delayed(duration, inner)) = response {
        tokio::time::sleep(duration).await;
        response = Some(*inner);
    }

    match response {
        Some(MethodResponse::Success(result)) => {
            let line = serde_json::json!({ "id": request.id, "result": result }).to_string();
            let _ = write_line(io, &line).await;
        }
        Some(MethodResponse::RemoteError { code, message }) => {
            let line = serde_json::json!({
                "id": request.id,
                "error": { "code": code, "message": message },
            })
            .to_string();
            let _ = write_line(io, &line).await;
        }
        Some(MethodResponse::WrongId(result)) => {
            let line = serde_json::json!({
                "id": format!("{}-wrong", request.id),
                "result": result,
            })
            .to_string();
            let _ = write_line(io, &line).await;
        }
        Some(MethodResponse::NonJson(text)) => {
            let _ = write_line(io, &text).await;
        }
        Some(MethodResponse::PongResult) => {
            let line = serde_json::json!({
                "id": request.id,
                "result": { "type": "pong" },
            })
            .to_string();
            let _ = write_line(io, &line).await;
        }
        Some(MethodResponse::CloseBeforeReply) | None => {
            // 直接關閉連線、不回應。`None`（呼叫端沒替這個 method 設定回應）與明確要求的
            // `CloseBeforeReply` 行為相同——測試理當先設定好要用到的 method。
        }
        Some(MethodResponse::Delayed(..)) => unreachable!(
            "上面的 while 迴圈已經把所有 Delayed 攤平，這裡不會再遇到 Delayed 這個 variant"
        ),
    }
}

async fn handle_subscribe<T>(
    io: &mut BufReader<T>,
    request: &IncomingRequest,
    config: &FakeHerdrConfig,
) where
    T: AsyncRead + AsyncWrite + Unpin + BestEffortAbort,
{
    let Ok(params) = serde_json::from_value::<EventsSubscribeParams>(request.params.clone()) else {
        return;
    };

    if let Some((position, pane_id)) =
        find_failing_probe(&params.subscriptions, config.failing_probe_pane_ids())
    {
        let line = serde_json::json!({
            "id": format!("{}:sub:{position}:probe", request.id),
            "error": {
                "code": "pane_not_found",
                "message": format!("pane {pane_id} not found"),
            },
        })
        .to_string();
        let _ = write_line(io, &line).await;
        return;
    }

    let started =
        serde_json::json!({ "id": request.id, "result": { "type": "subscription_started" } })
            .to_string();
    if write_line(io, &started).await.is_err() {
        return;
    }

    // fix round 1 finding 4：依訂閱內容挑第一個符合的規則，讓不同連線可以重播不同腳本
    // （design、spec「兩條訂閱連線同時存在」）。
    for step in config.subscribe_steps_for(&params.subscriptions) {
        match step {
            Step::Event(line) | Step::Malformed(line) => {
                if write_line(io, line).await.is_err() {
                    return;
                }
            }
            Step::Delay(duration) => tokio::time::sleep(*duration).await,
            Step::Close => return,
            Step::Abort => {
                io.get_ref().best_effort_abort();
                return;
            }
            Step::Hold => {
                // fix round 1 finding 2：不再自己等待協作式的 shutdown 訊號；
                // `FakeHerdr::drop` 的 `abort_all()` 會強制終止這個 task。
                //
                // 這裡等的是「對端把連線關掉」而不是 `std::future::pending()`：訂閱連線上
                // client 送完 `events.subscribe` 之後不會再送任何一行，所以這個讀取迴圈實際
                // 上就是掛著等對端關閉——差別在於對端關閉時 handler 會真的結束，
                // `CloseGuard` 才能把這條連線標記成已關閉（`FakeHerdr::closed_connections`）。
                // 對端一直開著時行為與過去相同：永遠不返回，直到 task 被 abort。
                while let Ok(Some(_)) = read_one_line(io).await {}
                return;
            }
        }
    }
    // 腳本跑完沒有明確的結束步驟：以正常關閉收尾（等同 `Step::Close`）。
}

/// 依訂閱清單找第一個落在 `failing` 集合裡的每 pane 訂閱，回傳它在清單中的 1-based 位置與
/// pane id（design D12：`error` 回應 `id` 帶 `:sub:<序號>:probe`，序號是清單中第幾筆）。
fn find_failing_probe<'a>(
    subscriptions: &'a [Subscription],
    failing: &HashSet<String>,
) -> Option<(usize, &'a str)> {
    subscriptions
        .iter()
        .enumerate()
        .find_map(|(i, sub)| match sub {
            Subscription::PaneAgentStatusChanged { pane_id } if failing.contains(pane_id) => {
                Some((i + 1, pane_id.as_str()))
            }
            _ => None,
        })
}
