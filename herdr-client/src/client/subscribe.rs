//! 長連線事件訂閱：`Client::subscribe`（定義在 `client/mod.rs`，重用本檔的型別）、
//! `EventStream`、`IncomingEvent`、`StreamError`（design D4、D6、D10、D12；spec
//! `herdr-event-subscription`）。

use std::str::FromStr;

use serde::Serialize;

use crate::connector::NdjsonStream;
use crate::types::{EventKind, EventsSubscribeParams, SubscriptionEventKind, SubscriptionStarted};

use super::request::Request;
use super::request::private::Sealed;

/// `events.subscribe` 的 request 型別：`params` 序列化為 [`EventsSubscribeParams`]
/// （design §5.2）；跟 `PaneReadRequest` 同樣是單一欄位的 newtype，序列化結果與直接序列化
/// 內層 `EventsSubscribeParams` 完全相同。
///
/// 全分支最終 review finding 2：`pub(crate)`，不對外公開，也不從 `client` 模組
/// re-export——`events.subscribe` 收到 `subscription_started` 後語意是「把連線包成
/// `EventStream` 交給呼叫端持續讀取事件」，跟 `Request::METHOD` 搭配 `Client::request` 的
/// 「讀到回應就丟棄連線」語意不相容。這個型別過去公開實作了 `Request`，外部呼叫端因此能合法
/// 寫出 `client.request(EventsSubscribeRequest(..))`：會通過編譯、也會在收到
/// `subscription_started` 後照 `request()` 的語意立即丟棄連線並回傳成功——呼叫端拿到一個
/// 「成功」但其實從沒真正建立事件串流的結果，是個容易誤用的陷阱。改成 crate-private 之後，
/// 只有 `Client::subscribe` 能建構、使用它；`client::subscribe` 模組本身的 `impl Request`
/// 仍然合法（sealed trait 的邊界見 `request::private::Sealed`），只是外部無法命名這個型別，
/// 也就無法自己呼叫 `client.request(EventsSubscribeRequest(..))`（`compile_fail` 示範見
/// `Client::subscribe` 的文件註解）。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub(crate) struct EventsSubscribeRequest(pub(crate) EventsSubscribeParams);

impl Sealed for EventsSubscribeRequest {}

impl Request for EventsSubscribeRequest {
    const METHOD: &'static str = "events.subscribe";
    type Response = SubscriptionStarted;
}

/// 分軌後的一筆事件（design D4：以「已知名稱集合」判定，未知名稱保留原字串）。
///
/// 分類順序：先試 [`EventKind::from_str`]（26 種底線命名 → `Lifecycle`），再試
/// [`SubscriptionEventKind::from_str`]（3 種點號命名 → `PerPane`），兩者都失敗 → `Unknown`
/// （保留原始 `event` 字串，供上層記錄；spec「事件分軌」要求未知事件不得中斷串流，且不能
/// 被悄悄吞掉字串）。三個變體都原樣保留 `data`，payload 型別的挑選是上層（change 1b）的事。
#[derive(Debug, Clone, PartialEq)]
pub enum IncomingEvent {
    /// 生命週期事件：`event` 為 26 種底線命名之一。
    Lifecycle(EventKind, serde_json::Value),
    /// 每 pane 事件：`event` 為 3 種點號命名之一。
    PerPane(SubscriptionEventKind, serde_json::Value),
    /// 未知事件名稱：保留原始字串，不中斷串流。
    Unknown {
        event: String,
        data: serde_json::Value,
    },
}

/// [`EventStream::next`] 的錯誤（design D6）：目前只有 I/O 錯誤一種——壞行（非 JSON、缺
/// `event`／`data` 欄位）不算錯誤，只記警告後跳過（spec「壞行與結束」）。`Display`
/// 含原因文字，供 change 1b 填 `ConnectionState::Disconnected { reason }`（設計文件 §9
/// 「任何連線錯誤都顯示原因，不吞掉」）。
#[derive(Debug, thiserror::Error)]
pub enum StreamError {
    /// 連線發生 I/O 錯誤（原因見底層 `std::io::Error`）。
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

/// `events.subscribe` 建立成功後的事件串流（design D6）。
///
/// `next()` 的結束語意：正常 EOF → `None`；I/O 錯誤 → 先回一次 `Some(Err(..))`，之後一律
/// `None`（不會重複回報同一個錯誤，也不會在錯誤之後再嘗試讀取底層連線）。不設逾時、不重連
/// （design D10 Non-Goals：這些是連線迴圈〔change 1b〕的責任）。刻意不實作 `futures::Stream`
/// trait，只提供這一個 inherent async 方法，避免為此另外引入 `futures` 依賴。
pub struct EventStream {
    stream: Box<dyn NdjsonStream>,
    ended: bool,
}

impl std::fmt::Debug for EventStream {
    /// 手寫 `Debug`：`Box<dyn NdjsonStream>` 沒有、也不需要實作 `Debug`；只印 `ended`，讓
    /// `EventStream` 能出現在 `Result::expect_err` 這類要求 `T: Debug` 的斷言裡（測試常用）。
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EventStream")
            .field("ended", &self.ended)
            .finish_non_exhaustive()
    }
}

impl EventStream {
    pub(super) fn new(stream: Box<dyn NdjsonStream>) -> Self {
        Self {
            stream,
            ended: false,
        }
    }

    /// 讀下一個事件。壞行（非 JSON、或缺 `event`／`data` 欄位）記 `tracing::warn!` 後跳過、
    /// 繼續讀下一行，不算一次呼叫結果（spec「壞行與結束」：壞行不得中斷串流）。
    pub async fn next(&mut self) -> Option<Result<IncomingEvent, StreamError>> {
        if self.ended {
            return None;
        }
        loop {
            match self.stream.recv_line().await {
                Ok(None) => {
                    self.ended = true;
                    return None;
                }
                Err(e) => {
                    self.ended = true;
                    return Some(Err(StreamError::Io(e)));
                }
                Ok(Some(line)) => match parse_event_line(&line) {
                    Ok(event) => return Some(Ok(event)),
                    Err(reason) => {
                        tracing::warn!(
                            target: "herdr_client::client::subscribe",
                            line = %line,
                            reason = %reason,
                            "跳過無法解析的事件行（design D6：壞行不中斷串流）"
                        );
                    }
                },
            }
        }
    }
}

/// 解析一行事件，回傳分軌後的 [`IncomingEvent`]，或壞行的原因文字。
///
/// 刻意不透過 [`crate::types::EventEnvelope`] 的 `Deserialize` 做這個判斷——它整個容器標了
/// `#[serde(default)]`，缺 `event`／`data` 任一欄位時會被靜默補成空字串／`null`，
/// 偵測不到「缺欄位」這個壞行情境（design D13 對「必填欄位缺席該失敗、不該被 default 蓋過去」
/// 的同類考量）。這裡改為手動檢查：整行必須是 JSON 物件，`event` 必須存在且為字串，`data`
/// 必須存在且是 JSON 物件（Codex task review fix round 1 finding 1：兩份合約 schema 的
/// `event`／`subscription_event` 根都把 `data` 指向 `EventData`／`SubscriptionEventData`，
/// 兩者都是 `oneOf`、每個分支都標 `"type": "object"`，見 `tests/fixtures/schema-p22.json`／
/// `schema-p20.json`——`null`、陣列、純量值都不是合法 payload，只檢查鍵是否存在會讓格式損壞
/// 的已知事件被當成真事件，延後到上層 payload 解析才失敗，破壞壞行隔離語意）。
fn parse_event_line(line: &str) -> Result<IncomingEvent, String> {
    let value: serde_json::Value =
        serde_json::from_str(line).map_err(|e| format!("not valid JSON: {e}"))?;
    let obj = value
        .as_object()
        .ok_or_else(|| "not a JSON object".to_string())?;
    let event = obj
        .get("event")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "missing or non-string \"event\" field".to_string())?
        .to_string();
    let data = obj
        .get("data")
        .ok_or_else(|| "missing \"data\" field".to_string())?;
    if !data.is_object() {
        return Err(format!("\"data\" field must be a JSON object, got: {data}"));
    }
    Ok(classify(event, data.clone()))
}

/// design D4：先試 `EventKind`，再試 `SubscriptionEventKind`，都失敗則保留原名進 `Unknown`。
fn classify(event: String, data: serde_json::Value) -> IncomingEvent {
    if let Ok(kind) = EventKind::from_str(&event) {
        return IncomingEvent::Lifecycle(kind, data);
    }
    if let Ok(kind) = SubscriptionEventKind::from_str(&event) {
        return IncomingEvent::PerPane(kind, data);
    }
    IncomingEvent::Unknown { event, data }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_event_line_classifies_lifecycle() {
        let event = parse_event_line(r#"{"event":"pane_created","data":{"pane_id":"wD:p1"}}"#)
            .expect("應可解析");
        assert_eq!(
            event,
            IncomingEvent::Lifecycle(
                EventKind::PaneCreated,
                serde_json::json!({"pane_id":"wD:p1"})
            )
        );
    }

    #[test]
    fn parse_event_line_classifies_per_pane() {
        let event =
            parse_event_line(r#"{"event":"pane.agent_status_changed","data":{"pane_id":"wD:p1"}}"#)
                .expect("應可解析");
        assert_eq!(
            event,
            IncomingEvent::PerPane(
                SubscriptionEventKind::PaneAgentStatusChanged,
                serde_json::json!({"pane_id":"wD:p1"})
            )
        );
    }

    #[test]
    fn parse_event_line_classifies_unknown_and_keeps_name() {
        let event = parse_event_line(r#"{"event":"pane_teleported","data":{}}"#)
            .expect("應可解析（未知事件本身不是壞行）");
        assert_eq!(
            event,
            IncomingEvent::Unknown {
                event: "pane_teleported".to_string(),
                data: serde_json::json!({}),
            }
        );
    }

    #[test]
    fn parse_event_line_rejects_non_json() {
        assert!(parse_event_line("not json at all").is_err());
    }

    #[test]
    fn parse_event_line_rejects_missing_event_field() {
        assert!(parse_event_line(r#"{"data":{}}"#).is_err());
    }

    #[test]
    fn parse_event_line_rejects_missing_data_field() {
        assert!(parse_event_line(r#"{"event":"pane_created"}"#).is_err());
    }

    /// Codex task review fix round 1 finding 1：兩份合約 schema 的 `event`／`subscription_event`
    /// 根都把 `data` 指向 `EventData`／`SubscriptionEventData`，兩者都是 `oneOf` 且每個分支
    /// `"type": "object"`（見 `tests/fixtures/schema-p22.json`／`schema-p20.json`）——`data`
    /// 只檢查鍵是否存在，不檢查型別，會讓 `null`、陣列、純量值被誤判成合法事件。
    #[test]
    fn parse_event_line_rejects_null_data() {
        assert!(parse_event_line(r#"{"event":"pane_created","data":null}"#).is_err());
    }

    #[test]
    fn parse_event_line_rejects_array_data() {
        assert!(parse_event_line(r#"{"event":"pane_created","data":[1,2,3]}"#).is_err());
    }

    #[test]
    fn parse_event_line_rejects_number_data() {
        assert!(parse_event_line(r#"{"event":"pane_created","data":42}"#).is_err());
    }

    #[test]
    fn parse_event_line_rejects_string_data() {
        assert!(parse_event_line(r#"{"event":"pane_created","data":"oops"}"#).is_err());
    }

    #[test]
    fn parse_event_line_rejects_non_string_event() {
        assert!(parse_event_line(r#"{"event":42,"data":{"pane_id":"wD:p1"}}"#).is_err());
    }

    /// design D4：名稱含點號但不在 `SubscriptionEventKind` 的 3 種已知值內，一樣要落進
    /// `Unknown`，不能因為含點號就被誤判成每 pane 事件（分軌是查已知名稱集合，不是查有沒有
    /// 點號）。
    #[test]
    fn parse_event_line_classifies_unknown_with_dot_in_name() {
        let event = parse_event_line(r#"{"event":"pane.teleported","data":{}}"#)
            .expect("應可解析（未知事件本身不是壞行）");
        assert_eq!(
            event,
            IncomingEvent::Unknown {
                event: "pane.teleported".to_string(),
                data: serde_json::json!({}),
            }
        );
    }
}
