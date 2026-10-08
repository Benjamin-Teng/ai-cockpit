//! `cockpit-git`：以唯讀方式執行 git 的封閉查詢層（git-review design D1）。
//!
//! 只放安全邊界會用到的邏輯：能執行的 git 查詢種類在本 crate 內封閉（[`GitQuery`]，
//! sealed trait，crate 外無法新增查詢種類）、執行目標的選擇（[`GitTarget`]、
//! [`select_target`]）、版本與路徑的值型別（[`Oid`]、[`Side`]、[`RepoPath`]、
//! [`RefName`]）、執行器（[`GitRunner`]：並行上限、逾時、輸出上限、錯誤分類，design D3）。
//! 不知道 HTTP、HERDR、投影與 runtime 設定；也不依賴
//! `cockpit-files`、`cockpit-core`、`cockpit-herdr`、`herdr-client`（design D1：
//! `cockpit-git` 不依賴它們，它們也不依賴 `cockpit-git`）。
//!
//! git-review task 2.1 只做型別、argv 組裝與目標選擇的純函式；task 2.2 在其上加執行器
//! （[`GitRunner`]／[`QueryPlan`]），仍不解析 git 輸出。task 2.3 幫 [`Status`]、[`Refs`]、
//! [`Log`]、[`CommitInfo`]、[`ChangedFiles`] 補上真正的 `Output` 型別與解析器（各自的 `parse`
//! inherent 方法，實作分別在 `status.rs`、`refs.rs`、`log.rs`、`commit_info.rs`、
//! `changed_files.rs`）：純函式，輸入是 [`RunOutput::calls`]，不啟動 git，不需要子程序也能
//! 測試。task 2.4（本次）幫 [`FileDiff`] 補上同樣模式的 `Output`／`parse`（`file_diff.rs`：
//! unified patch 解析、左右並排對齊、`gap`、`version`），並提供未追蹤檔案用的純函式
//! [`untracked_file_diff`]（design D7：`EMPTY→WORKTREE` 不經 git，讀檔上限由 `cockpit`
//! 負責，這裡只處理位元組）。`MergeBase`／`BlobSize`／`Blob`／`VerifyCommit`（後續 task）的
//! `Output` 仍是 `()`。git-review task 3.3 fix round 1（Ruling R8）新增 [`BlobId`]（`Output =
//! String`，`meta` 端點用它取得物件 hash 而不必讀取內容），解析器在 `blob_id.rs`；fix round 2
//! （Ruling R10）再新增 [`BlobHead`]（argv 同 [`Blob`]，`stdout_cap` 為 8192 且可截斷，供
//! `meta` 依 5a 規則以前 8192 位元組分類 `viewer`）。repo-projects task 2.1 新增
//! [`RepoIdentity`]（`Output = Option<RepoIdentityOutput>`：判定目錄屬於哪個 git repo，回傳
//! git 的三個原始路徑字串，`None` 代表「不是 repo」；解析器與判別規則在 `repo_identity.rs`）。

mod blob_id;
mod changed_files;
mod commit_info;
mod file_diff;
mod graph;
mod log;
mod oid;
mod parse_error;
mod parse_support;
mod query;
mod ref_name;
mod refs;
mod repo_identity;
mod repo_path;
mod runner;
mod side;
mod status;
mod target;

pub use changed_files::{ChangedFile, ChangedFilesOutput};
pub use commit_info::{CommitInfoOutput, CommitPerson};
pub use file_diff::{DiffLine, DiffRow, FileDiffOutput, UntrackedDiffError, untracked_file_diff};
pub use graph::{GraphCommit, GraphHalf, GraphLine, GraphRow, layout};
pub use log::{LogOutput, LogRow};
pub use oid::{Oid, OidError};
pub use parse_error::GitParseError;
pub use query::{
    Blob, BlobHead, BlobId, BlobSize, ChangedFiles, CommitInfo, FileDiff, GitQuery, Log, MergeBase,
    QueryError, Refs, RepoIdentity, Status, VerifyCommit,
};
pub use ref_name::{RefName, RefNameError};
pub use refs::{RefEntry, RefKind, RefsHead, RefsOutput};
pub use repo_identity::RepoIdentityOutput;
pub use repo_path::{RepoPath, RepoPathError};
pub use runner::{CallOutcome, GitRunner, QueryPlan, RunOutput, RunnerError};
pub use side::Side;
pub use status::{StatusBranch, StatusEntry, StatusGroup, StatusOutput};
pub use target::{GitTarget, SelectTargetError, select_target};
