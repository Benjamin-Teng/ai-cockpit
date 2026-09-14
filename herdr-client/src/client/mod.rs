//! `Client`：單次 request／response 往返與長連線事件訂閱（design §5.2、D3、D5、D6、D10、
//! D12；spec `herdr-request`、`herdr-event-subscription`）。
//!
//! `Client::request` 對每次呼叫開一條全新連線：送出一行 `{"id","method","params"}`、讀一行
//! 回應、解析後把連線 drop 掉（不重試、不設逾時——這些是連線迴圈〔change 1b〕的責任，見
//! design D10「Non-Goals」）。`Client::subscribe`（型別定義在 `subscribe` 子模組）流程類似，
//! 但讀到 `subscription_started` 後把連線包成 [`EventStream`] 交給呼叫端持續讀取事件，不會
//! 主動關閉連線。

mod error;
mod request;
mod subscribe;

pub use error::RequestError;
pub use request::{PaneReadRequest, Request, SessionSnapshotRequest};
pub use subscribe::{EventStream, IncomingEvent, StreamError};

// 全分支最終 review finding 2：`EventsSubscribeRequest` 是 crate-private（見
// `subscribe.rs` 的型別文件），不對外 re-export；只有這個模組（`Client::subscribe`）用
// 得到它，這裡用一個不公開的 `use` 帶進來。
use subscribe::EventsSubscribeRequest;

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::connector::{ConnectError, Connector};
use crate::types::{
    ErrorBody, EventsSubscribeParams, RequestEnvelope, ResponseEnvelope, Subscription,
};

/// 程序內遞增的 request id 計數器（design D10：十進位字串，唯一性只需在程序內成立）。
static NEXT_REQUEST_ID: AtomicU64 = AtomicU64::new(1);

fn next_request_id() -> String {
    NEXT_REQUEST_ID.fetch_add(1, Ordering::Relaxed).to_string()
}

/// 對單一 HERDR 端點的 client（design §5.2：`pub struct Client { connector: Arc<dyn
/// Connector> }`）。
pub struct Client {
    connector: Arc<dyn Connector>,
}

impl Client {
    /// 用一個 [`Connector`] 建立 `Client`；三種 transport（named pipe／unix socket／子程序）
    /// 對 `Client` 呈現同一種介面。
    #[must_use]
    pub fn new(connector: Arc<dyn Connector>) -> Self {
        Self { connector }
    }

    /// 送一次 request：開連線 → 送一行 → 讀一行 → 解析 → 連線在函式結束時 drop（design
    /// §5.2）。
    ///
    /// 錯誤分類見 [`RequestError`]；連線在讀到回應前中斷時，`Client` 自己把「乾淨 EOF」合成
    /// 成 `RequestError::Io(UnexpectedEof)`——4.1 實測 named pipe 上「收到 request 後不回就
    /// 關閉」對 transport 只是乾淨 EOF（`Ok(None)`），不會自己產生 I/O 錯誤。
    pub async fn request<R: Request>(&self, req: R) -> Result<R::Response, RequestError> {
        let id = next_request_id();
        let mut stream = self.connector.connect().await?;

        let envelope = RequestEnvelope {
            id: &id,
            method: R::METHOD,
            params: &req,
        };
        let line = serde_json::to_string(&envelope)
            .map_err(|e| RequestError::Protocol(format!("failed to serialize request: {e}")))?;
        if let Err(e) = stream.send_line(&line).await {
            // Codex task review fix round 1 finding 2：子程序橋接目標已經結束時，第一次
            // `send_line`（不只是第一次 `recv_line`）就可能撞上 `ConnectionRefused`
            // （`ChildStdioStream::send_line` 已經把「還沒收到任何一行、子程序已結束」時的
            // `BrokenPipe`／`ConnectionReset` 分類成這個 kind，見
            // `src/connector/child_stdio.rs`）；一定要跟第一次讀取共用同一套「NotFound／
            // ConnectionRefused → ServerNotRunning」判定，不能讓 `?` 把它直接吞成 `Io`。
            return Err(classify_first_io_error(e));
        }

        let response_line = match stream.recv_line().await {
            Ok(Some(line)) => line,
            Ok(None) => {
                // design D3 的判定只在「第一次讀取」發生時分類 ServerNotRunning；這裡是讀到
                // 完整回應之前的乾淨 EOF，屬於 spec「回應前連線中斷」情境（連線類錯誤，原因
                // 文字非空），不是「目標沒有 server」，所以合成 `Io`，不是 `Connect`。
                return Err(RequestError::Io(std::io::Error::new(
                    std::io::ErrorKind::UnexpectedEof,
                    "connection closed before response",
                )));
            }
            Err(e) => return Err(classify_first_io_error(e)),
        };

        parse_response::<R>(&id, &response_line)
    }

    /// 建立一條長連線事件訂閱（design §5.2、D6、D10、D12；spec `herdr-event-subscription`
    /// 「訂閱建立」）：開連線 → 送 `events.subscribe` 一行 → 讀一行 → 解析——收到 `error`
    /// 回應時整個訂閱失敗，回 `RequestError::Remote`（含 D12：探測失敗的 `error` 回應 `id`
    /// 帶 `:sub:<n>:probe` 後綴，仍視為相符）；`result` 不是 `subscription_started` 型別
    /// → `Protocol`；成功才把連線交給呼叫端包成 [`EventStream`]。跟 `request` 一樣不設逾時、
    /// 不重試（design D10 Non-Goals：連線迴圈〔change 1b〕的責任）。
    ///
    /// # 只能走這個方法，不能繞去 `Client::request`（全分支最終 review finding 2）
    ///
    /// `events.subscribe` 內部用的 request 型別（`EventsSubscribeRequest`）是
    /// crate-private、不對外公開，也不從 `client` 模組 re-export：`Client::request` 收到
    /// 回應後語意是「解析完就丟棄連線」，跟 `subscribe` 「收到 `subscription_started` 後把
    /// 連線包成 `EventStream` 持續讀取」的語意互不相容——如果外部呼叫端能命名這個型別，就能
    /// 合法寫出 `client.request(EventsSubscribeRequest(..))`，通過編譯、也會在收到
    /// `subscription_started` 後立刻把連線丟掉並回傳「成功」，卻從沒真正建立事件串流。下面
    /// 這段示範外部呼叫端連命名這個型別都做不到（`herdr_client::client` 沒有
    /// `EventsSubscribeRequest` 這個公開路徑；實測錯誤碼是 `E0603`：tuple struct
    /// constructor 是 private——附註同 `client::request::Request` 文件註解：fence 上的
    /// `,E0603` 只是給人看的標記，rustdoc 的 `compile_fail` 不會真的核對錯誤碼，真正驗證
    /// 過的是這段文件本身）：
    ///
    /// ```compile_fail,E0603
    /// # async fn f(client: &herdr_client::client::Client) {
    /// let req = herdr_client::client::EventsSubscribeRequest(Default::default());
    /// client.request(req).await.unwrap();
    /// # }
    /// ```
    pub async fn subscribe(&self, subs: &[Subscription]) -> Result<EventStream, RequestError> {
        let id = next_request_id();
        let mut stream = self.connector.connect().await?;

        let req = EventsSubscribeRequest(EventsSubscribeParams {
            subscriptions: subs.to_vec(),
        });
        let envelope = RequestEnvelope {
            id: &id,
            method: EventsSubscribeRequest::METHOD,
            params: &req,
        };
        let line = serde_json::to_string(&envelope)
            .map_err(|e| RequestError::Protocol(format!("failed to serialize request: {e}")))?;
        if let Err(e) = stream.send_line(&line).await {
            return Err(classify_first_io_error(e));
        }

        let response_line = match stream.recv_line().await {
            Ok(Some(line)) => line,
            Ok(None) => {
                // design D6／spec「訂閱建立」：還沒讀到 `subscription_started` 就 EOF，屬於
                // 「連線類錯誤，原因文字非空」，跟 `request` 的「回應前連線中斷」同一種合成
                // 方式（見上面 `request` 的對應分支）。
                return Err(RequestError::Io(std::io::Error::new(
                    std::io::ErrorKind::UnexpectedEof,
                    "connection closed before subscription_started",
                )));
            }
            Err(e) => return Err(classify_first_io_error(e)),
        };

        parse_response::<EventsSubscribeRequest>(&id, &response_line)?;

        Ok(EventStream::new(stream))
    }
}

/// design D3：子程序型連線在還沒讀到任何一行、子程序已結束時，`send_line`／`recv_line`
/// 會回一個 `NotFound`／`ConnectionRefused` 類的 I/O 錯誤（見 `ConnectError::from_io` 與
/// `ChildStdioStream` 的文件註解）；`Client` 在第一次送出或讀取遇到這兩種 kind 時，把它
/// 對應成 `RequestError::Connect(ConnectError::ServerNotRunning)`——named pipe／unix socket
/// 兩種 transport 通常在 `connect()` 當下就已經回這個錯誤（走 `?` 那條路徑），這裡涵蓋的是
/// 子程序橋接「spawn 成功但子程序立刻結束」的情境，而且子程序結束後第一個撞上的可能是
/// `send_line`，不保證一定是 `recv_line`（Codex task review fix round 1 finding 2），所以
/// 兩個呼叫點共用同一個分類函式。其餘 kind 一律歸 `Io`。
fn classify_first_io_error(e: std::io::Error) -> RequestError {
    match e.kind() {
        std::io::ErrorKind::NotFound | std::io::ErrorKind::ConnectionRefused => {
            RequestError::Connect(ConnectError::ServerNotRunning {
                detail: e.to_string(),
            })
        }
        _ => RequestError::Io(e),
    }
}

/// design D5／D12：解析一行回應。
///
/// - 整行不能反序列化成 `ResponseEnvelope`（非合法 JSON、或 `error` 物件缺 `code`／
///   `message` 這類必填欄位，design D13）→ `Protocol`。
/// - `result` 與 `error` 必須「恰有一個」是 `Some`（Codex task review fix round 1
///   finding 3）：兩者同時出現、或兩者都缺席，都代表對方沒照協定講話 → `Protocol`。
/// - 只有 `error`：`id` 等於 request id、或以 `"<request id>:"` 開頭（D12：HERDR 對訂閱
///   探測失敗會這樣改寫）→ `Remote`；否則 → `Protocol`（id 對不上，沒有 code／message
///   可信，不硬填成 Remote，D5）。
/// - 只有 `result`：`id` 必須完全相等，否則 `Protocol`；`R::Response` 反序列化失敗
///   （含 `type` 標籤不符）→ `Protocol`。
fn parse_response<R: Request>(request_id: &str, line: &str) -> Result<R::Response, RequestError> {
    let response: ResponseEnvelope = serde_json::from_str(line).map_err(|e| {
        RequestError::Protocol(format!(
            "response line does not parse as a response envelope: {e}"
        ))
    })?;

    match (response.result, response.error) {
        (Some(_), Some(_)) => Err(RequestError::Protocol(
            "response has both \"result\" and \"error\"".to_string(),
        )),
        (None, None) => Err(RequestError::Protocol(
            "response has neither \"result\" nor \"error\"".to_string(),
        )),
        (None, Some(ErrorBody { code, message })) => {
            if response_id_matches(&response.id, request_id) {
                Err(RequestError::Remote {
                    code,
                    message,
                    response_id: response.id,
                })
            } else {
                Err(RequestError::Protocol(format!(
                    "error response id {:?} does not match request id {request_id:?}",
                    response.id
                )))
            }
        }
        (Some(result), None) => {
            if response.id != request_id {
                return Err(RequestError::Protocol(format!(
                    "response id {:?} does not match request id {request_id:?}",
                    response.id
                )));
            }
            serde_json::from_value(result).map_err(|e| {
                RequestError::Protocol(format!("result does not match expected shape: {e}"))
            })
        }
    }
}

/// design D12：`error` 回應的 `id` 只要等於 `request_id`、或以 `"<request_id>:"` 開頭，就視為
/// 相符（HERDR 對訂閱探測失敗會回 `<request id>:sub:<序號>:probe`）。
fn response_id_matches(response_id: &str, request_id: &str) -> bool {
    response_id == request_id || response_id.starts_with(&format!("{request_id}:"))
}

/// Codex task review fix round 2 finding 1：`request::private::Sealed` 一定要能被 `client`
/// 底下的**手足**模組實作，不能只對 `request` 自己與它的子孫可見——4.3 的 `events.subscribe`
/// 型別預期會放在像 `client::subscribe` 這樣的手足模組裡，跟 `client::request` 同一層、都是
/// `client` 的子模組，不是 `client::request` 的子孫。這個測試模組本身就是刻意放在
/// `client`（本檔）底下、與 `request` 同層的手足模組，模擬 4.3 的情境：如果它能編譯，代表
/// sealing 邊界正確地是「crate 內任何地方都能實作，crate 外都不能」，而不是「只有
/// `request` 自己的子孫能實作」。
#[cfg(test)]
mod seal_sibling_test {
    use super::request::Request;
    use super::request::private::Sealed;

    /// 模擬 4.3 放在手足模組（例如 `client::subscribe`）的 request 型別。
    #[derive(Debug, Clone, Copy, serde::Serialize)]
    struct SiblingModuleRequest;

    impl Sealed for SiblingModuleRequest {}

    impl Request for SiblingModuleRequest {
        const METHOD: &'static str = "sibling.probe";
        type Response = serde_json::Value;
    }

    #[test]
    fn sibling_module_can_implement_sealed_request() {
        assert_eq!(SiblingModuleRequest::METHOD, "sibling.probe");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn response_id_matches_exact() {
        assert!(response_id_matches("7", "7"));
    }

    #[test]
    fn response_id_matches_probe_suffix() {
        assert!(response_id_matches("7:sub:1:probe", "7"));
    }

    #[test]
    fn response_id_does_not_match_unrelated_id() {
        assert!(!response_id_matches("70", "7"));
        assert!(!response_id_matches("8", "7"));
    }

    #[test]
    fn classify_first_io_error_maps_not_found_and_connection_refused_to_server_not_running() {
        for kind in [
            std::io::ErrorKind::NotFound,
            std::io::ErrorKind::ConnectionRefused,
        ] {
            let err = classify_first_io_error(std::io::Error::new(kind, "boom"));
            assert!(
                matches!(
                    err,
                    RequestError::Connect(ConnectError::ServerNotRunning { .. })
                ),
                "kind {kind:?} 應對應 ServerNotRunning，實際: {err:?}"
            );
        }
    }

    #[test]
    fn classify_first_io_error_maps_other_kinds_to_io() {
        let err =
            classify_first_io_error(std::io::Error::new(std::io::ErrorKind::TimedOut, "boom"));
        assert!(
            matches!(err, RequestError::Io(_)),
            "其他 kind 應對應 Io，實際: {err:?}"
        );
    }
}
