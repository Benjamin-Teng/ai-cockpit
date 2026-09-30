//! `cockpit-git` 解析器共用的錯誤型別（git-review task 2.3）。
//!
//! 這是本 crate 內、解析 git 輸出時的錯誤，**不是** design D6 的 HTTP 錯誤代碼——HTTP 層
//! （之後的 task）決定怎麼把它對應到狀態碼與 `code`（多半是 502 `git_failed`：會走到這裡
//! 代表 git 的輸出格式與預期不符，不是使用者輸入的問題——那些在啟動子程序之前已被
//! [`crate::QueryError`] 或 HTTP 層的參數驗證擋下）。

use std::fmt;

/// 解析一個 [`crate::GitQuery`] 的執行結果（[`crate::RunOutput::calls`]）失敗的原因。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GitParseError {
    /// `calls` 的長度與這個查詢預期的呼叫次數不同。理論上不會發生（`GitQuery::commands`
    /// 保證每次呼叫的次數固定），是型別安全的防禦分支。
    UnexpectedCallCount,
    /// 某次呼叫的結果不是這個查詢已知的合法失敗模式（例如 `Refs` 的 `rev-parse`／
    /// `symbolic-ref` 兩次呼叫以外的呼叫卻收到非零結束、或收到 `Unavailable`／
    /// `Untrusted`／`Timeout`／`TooLarge` 這些不該走到解析階段的錯誤）。內含
    /// [`crate::RunnerError`] 的 `Display` 文字，只供日誌用；不得放進 HTTP 回應本體
    /// （design D6：錯誤本體不得含 git 的 stderr 原文）。
    UnexpectedCallOutcome(String),
    /// git 輸出的位元組格式與 design D4 預期不符（例如欄位數不對），且不是因為輸出被截斷
    /// 造成——brief：「格式破損要回明確的解析錯誤，不得 panic」。
    MalformedOutput(String),
    /// 未合併（衝突中）的檔案：patch 以 `diff --cc`／`diff --combined`（三方合併格式）開頭——
    /// 不是格式破損，是 git 對「暫存區有多個版本」的檔案送 `INDEX→WORKTREE` 時的合法輸出
    /// （git-review 目視驗收缺陷 V1、Ruling R11）。與 [`GitParseError::MalformedOutput`] 分開，
    /// 讓呼叫端（`cockpit`）能對應到 409 `unmerged_path`，不是籠統的 502 `git_failed`。
    Unmerged,
}

impl fmt::Display for GitParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GitParseError::UnexpectedCallCount => write!(f, "呼叫次數與查詢預期不符"),
            GitParseError::UnexpectedCallOutcome(msg) => {
                write!(f, "呼叫結果不是這個查詢已知的合法失敗模式：{msg}")
            }
            GitParseError::MalformedOutput(msg) => write!(f, "git 輸出格式不符預期：{msg}"),
            GitParseError::Unmerged => write!(f, "檔案未合併（衝突中），暫存區沒有單一版本"),
        }
    }
}

impl std::error::Error for GitParseError {}
