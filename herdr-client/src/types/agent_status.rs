//! `AgentStatus`：HERDR 回報的 agent 狀態（spec `herdr-observer-types`
//! 「AgentStatus 五值與未知值」）。

use serde::{Deserialize, Serialize};

/// HERDR 的 agent 狀態五值：`idle`、`working`、`blocked`、`done`、`unknown`。
///
/// **`Done` 只代表「已 idle 且尚未被看過」，不是任務完成**（設計文件 §2.4、
/// `CONTEXT.md`）。本型別刻意不提供任何把 `Done`（或其他值）轉成「已完成」的判斷方法或
/// 欄位；上層若要衍生完成語意，必須在 Domain 層自訂規則，不能依賴這個型別。
///
/// 任何無法辨識的字串一律解析為 `Unknown`，不會讓解析失敗（向前相容未知的 HERDR 版本）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum AgentStatus {
    Idle,
    Working,
    Blocked,
    Done,
    #[default]
    #[serde(other)]
    Unknown,
}
