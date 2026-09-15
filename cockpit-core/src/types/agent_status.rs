//! Agent 在某個 pane 裡的狀態機。刻意不提供任何「完成」語意或判斷方法：狀態庫與 UI
//! 要自己決定怎麼呈現 `Done`，不能把它當成任務成功與否的訊號。

use serde::{Deserialize, Serialize};

/// Agent 狀態，序列化為小寫字串（`"idle"`／`"working"`／`"blocked"`／`"done"`／`"unknown"`）。
///
/// 只有這五個值，沒有任何「完成」（completed／finished）變體，也不提供
/// `is_done`／`is_complete` 之類的判斷方法——避免呼叫端誤把 `Done` 當成任務成功。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentStatus {
    /// 沒有在跑任何東西，也還沒被回報過任何狀態變化。
    Idle,
    /// 正在執行任務。
    Working,
    /// 卡住、需要人或其他 agent 介入才能繼續。
    Blocked,
    /// 已 idle 且尚未被看過，不是任務完成。
    Done,
    /// 目前無法判斷狀態（例如 pane 剛出現、還沒收到第一筆狀態）。
    Unknown,
}
