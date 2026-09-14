//! `RequestError`：`Client::request` 往返可能失敗的四種分類（design D3、D5、D12；spec
//! `herdr-request`「回應對應」）。
//!
//! 四類對應四種不同的失敗原因：
//! - [`RequestError::Connect`]：連線本身失敗（含子程序型連線在第一次讀取遇到
//!   `NotFound`／`ConnectionRefused` 時對應的 `ConnectError::ServerNotRunning`，design D3）。
//! - [`RequestError::Remote`]：HERDR 明確回了 `error` 物件，且 `id` 相符（design D12：相符
//!   包含「等於 request id」與「以 `<request id>:` 開頭」兩種情況）。
//! - [`RequestError::Protocol`]：對方沒照協定講話——整行非 JSON、成功回應的 `id` 不符、
//!   `error` 回應的 `id` 也不符、或 `result` 形狀不符（design D5）。
//! - [`RequestError::Io`]：I/O 錯誤，含「還沒讀到完整回應就連線中斷」時由 `Client` 自己合成
//!   的 `UnexpectedEof`（4.1 實測：named pipe 上「收到 request 後不回就關閉」對 client 只是
//!   乾淨 EOF，transport 不會自己報錯，見 `Client::request` 的實作）。

use crate::connector::ConnectError;

/// `Client::request` 的錯誤型別。
#[derive(Debug, thiserror::Error)]
pub enum RequestError {
    /// 連線失敗；原因見 [`ConnectError`]。
    #[error("connect error: {0}")]
    Connect(#[from] ConnectError),

    /// 遠端明確回覆的 `error` 物件（`code`／`message` 原樣保留）；`response_id` 保留原始
    /// 回應 `id`（含 D12 探測失敗時的 `:sub:<n>:probe` 後綴）——全分支最終 review finding 5：
    /// 原本 `parse_response` 建立這個變體時只留下 `code`／`message`，直接丟掉
    /// `response.id`，探測失敗時「是哪一筆訂閱探測失敗」這個序號資訊沒有保留到錯誤原因裡。
    #[error("remote error {code}: {message} (response id {response_id})")]
    Remote {
        code: String,
        message: String,
        response_id: String,
    },

    /// 對方沒照協定講話：整行非 JSON、`id` 不符、或 `result` 形狀不符。
    #[error("protocol error: {0}")]
    Protocol(String),

    /// I/O 錯誤（含連線中斷後合成的 `UnexpectedEof`）。
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}
