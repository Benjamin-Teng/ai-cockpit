//! 多個解析器共用的小工具（git-review task 2.3）：`Status`、`Log`、`CommitInfo` 都只有一次
//! 呼叫，這裡提供共用的「取出唯一呼叫結果」邏輯，避免三處重複同一段防禦性檢查。

use crate::parse_error::GitParseError;
use crate::runner::{CallOutcome, RunnerError};

/// 取出單一呼叫查詢（`Status`、`Log`、`CommitInfo`）的唯一呼叫結果。呼叫次數不是 1
/// （理論上不會發生，`GitQuery::commands` 保證）或該次呼叫失敗時回傳明確的解析錯誤，
/// 不 panic。
pub(crate) fn single_call(
    calls: &[Result<CallOutcome, RunnerError>],
) -> Result<&CallOutcome, GitParseError> {
    if calls.len() != 1 {
        return Err(GitParseError::UnexpectedCallCount);
    }
    calls[0]
        .as_ref()
        .map_err(|err| GitParseError::UnexpectedCallOutcome(err.to_string()))
}
