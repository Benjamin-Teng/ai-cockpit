//! Request／response／event 的外層信封，供第 4 組 `Client` 使用（design D5、D8；設計文件
//! §5.2）。
//!
//! `ResponseEnvelope.result` 刻意留在 `serde_json::Value`：三種已知的 `result` 形狀
//! （`SessionSnapshotResult`、`PaneReadResultEnvelope`、`SubscriptionStarted`）各自在
//! `Deserialize` 裡檢查 `type` 欄位，`type` 不符時回傳 `Err`——這是 spec「result 形狀不符 →
//! Protocol」的基礎：呼叫端先解析出 `Value`，再依預期的 method 挑一種 result 型別去解析，
//! 失敗就對應 `RequestError::Protocol`（task 4.2，不在本 change 實作）。

use serde::de::Error as DeError;
use serde::{Deserialize, Deserializer, Serialize};

use super::pane_read::PaneReadResult;
use super::snapshot::SessionSnapshot;

/// 送出的一行 request：`{"id","method","params"}`。
#[derive(Debug, Clone, Serialize)]
pub struct RequestEnvelope<'a, P: Serialize> {
    pub id: &'a str,
    pub method: &'a str,
    pub params: &'a P,
}

/// 收到的一行 response。成功回應只有 `result`、失敗回應只有 `error`；兩者都可能缺席
/// （例如格式本身就不對），交給呼叫端判斷。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ResponseEnvelope {
    pub id: String,
    pub result: Option<serde_json::Value>,
    pub error: Option<ErrorBody>,
}

/// `error` 回應的內容。`code`／`message` 是 schema 必填欄位（`error_response.$defs.ErrorBody`），
/// 依 design D13 不加容器層 `#[serde(default)]`——缺任一欄位時反序列化直接失敗，而不是被
/// 悄悄補成空字串。`Client::request` 依賴這件事：`error` 物件缺 `code`／`message` 時，整個
/// `ResponseEnvelope` 解析失敗，經 `parse_response` 歸類成 `RequestError::Protocol`（對方沒
/// 照協定講話），不會被誤判成一個 `code`／`message` 皆空字串的 `Remote` 錯誤（Codex task
/// review fix round 1 finding 3）。`Default` 仍保留（不是靠它決定要不要補預設值，只是型別
/// 本身要有預設建構子，供 `tests/common/mod.rs` 的 `assert_required_fields_detected` 這類
/// 表格化測試使用）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ErrorBody {
    pub code: String,
    pub message: String,
}

/// 收到的一行事件：`{"event","data"}`。`event` 保留原始字串、`data` 保留原始 JSON——
/// 分軌（生命週期／每 pane／未知）與挑 payload 型別是呼叫端的事（design D4，task 4.3）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct EventEnvelope {
    pub event: String,
    pub data: serde_json::Value,
}

/// 依 `expected` 檢查 JSON 物件的 `"type"` 欄位，成功時把整個物件（含 `"type"`）交給 `T`
/// 的 `Deserialize` 實作。`T` 不需要宣告 `type` 欄位——不用 `deny_unknown_fields`，多出來的
/// `"type"` 會被忽略。
fn deserialize_tagged<'de, D, T>(deserializer: D, expected: &'static str) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    let tag = value
        .get("type")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| DeError::custom("missing or non-string \"type\" field"))?;
    if tag != expected {
        return Err(DeError::custom(format!(
            "expected result type \"{expected}\", got \"{tag}\""
        )));
    }
    T::deserialize(value).map_err(DeError::custom)
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
struct SessionSnapshotShape {
    snapshot: SessionSnapshot,
}

/// `session.snapshot` 成功回應的 `result`（`type` 必須是 `"session_snapshot"`）。
#[derive(Debug, Clone, PartialEq)]
pub struct SessionSnapshotResult {
    pub snapshot: SessionSnapshot,
}

impl<'de> Deserialize<'de> for SessionSnapshotResult {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let shape: SessionSnapshotShape = deserialize_tagged(deserializer, "session_snapshot")?;
        Ok(SessionSnapshotResult {
            snapshot: shape.snapshot,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
struct PaneReadResultShape {
    read: PaneReadResult,
}

/// `pane.read` 成功回應的 `result`（`type` 必須是 `"pane_read"`）。
#[derive(Debug, Clone, PartialEq)]
pub struct PaneReadResultEnvelope {
    pub read: PaneReadResult,
}

impl<'de> Deserialize<'de> for PaneReadResultEnvelope {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let shape: PaneReadResultShape = deserialize_tagged(deserializer, "pane_read")?;
        Ok(PaneReadResultEnvelope { read: shape.read })
    }
}

/// `events.subscribe` 成功回應的 `result`（`type` 必須是 `"subscription_started"`；沒有其他
/// 欄位）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SubscriptionStarted;

impl<'de> Deserialize<'de> for SubscriptionStarted {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Empty {}
        deserialize_tagged::<D, Empty>(deserializer, "subscription_started")?;
        Ok(SubscriptionStarted)
    }
}
