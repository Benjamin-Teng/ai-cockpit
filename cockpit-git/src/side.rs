//! 版本（design D5：「前端送來的版本（side）只有四種」）。

use crate::Oid;

/// 一次比較或一個「版本檔案內容」查詢裡，其中一側指的版本。
///
/// design D5：使用者可控的字串永遠不會以「版本語法」交給 git 解讀——`HEAD`、分支名稱、
/// `HEAD~1` 這類語法都不是合法的 `Side`；`HEAD` 與分支名稱由呼叫端先換成 [`Oid`] 再建構。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Side {
    /// 一個已驗證、且已確認是這個 repo 裡 commit 的 hash（`VerifyCommit`）。
    Oid(Oid),
    /// 暫存區。
    Index,
    /// 工作區。
    Worktree,
    /// 空內容（例如根 commit 與它「之前」比較、或還沒有任何 commit 的 repo）。
    Empty,
}
