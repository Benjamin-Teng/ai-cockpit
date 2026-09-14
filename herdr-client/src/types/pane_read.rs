//! `pane.read` 的參數與結果（spec `herdr-observer-types`「pane.read 型別」；供 change 3
//! 使用，change 1 不對真機呼叫，合約測試只驗序列化，見 design D10）。

use serde::{Deserialize, Serialize};

/// `pane.read` 的 `source` 值域（設計文件 §2.6）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReadSource {
    Visible,
    Recent,
    RecentUnwrapped,
    Detection,
}

/// `pane.read` 的 `format` 值域：schema 的 `ReadFormat` 只接受 `"text"`／`"ansi"`
/// 兩種字串（`PaneReadParams.format` 與 `PaneReadResult.format` 共用同一個 `$ref`）。
/// 用封閉的 enum 而不是 `String`，讓「送出 schema 不接受的值」在編譯期就不可能發生——
/// fix round 1 / finding 1（high）：原本 `PaneReadParams.format: Option<String>` 讓
/// `Some("html".to_string())` 這種不合法的值也能通過型別檢查，只能靠合約測試在執行期
/// 抓到。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReadFormat {
    Text,
    Ansi,
}

/// `pane.read` 的請求參數。`format`、`lines`、`strip_ansi` 缺席時完全不送出這個欄位
/// （`skip_serializing_if`），避免送出 schema 不接受 `null` 的欄位——`format`
/// （`ReadFormat`，`"text"`／`"ansi"`）與 `strip_ansi`（`bool`）在 schema 裡都不是
/// nullable，只有 `lines` 允許 `null`；為了三個欄位規則一致、且「省略」永遠是合法的
/// JSON Schema 選填欄位表示法，三個欄位一律用「缺席」而不是「送 null」。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PaneReadParams {
    pub pane_id: String,
    pub source: ReadSource,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<ReadFormat>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lines: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strip_ansi: Option<bool>,
}

/// `pane.read` 的回應內容（`result.read`）；8 個欄位在 schema 裡皆為必填。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PaneReadResult {
    pub pane_id: String,
    pub workspace_id: String,
    pub tab_id: String,
    pub source: ReadSource,
    pub format: ReadFormat,
    pub text: String,
    pub revision: u64,
    pub truncated: bool,
}
