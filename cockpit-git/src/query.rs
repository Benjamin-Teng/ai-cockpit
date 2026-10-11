//! 封閉的 git 查詢清單（git-review design D1、D4）。
//!
//! `GitQuery` 是 sealed trait：只有本 crate 內的型別能實作它，見 [`GitQuery`] 文件的
//! `compile_fail` doctest。task 2.1 只做「給定 [`GitTarget`] 產生完整 argv」這一步，
//! 不啟動子程序（task 2.2）、不解析輸出（task 2.3）——`Output` 先一律是 `()`，之後
//! task 2.3 會在既有的 impl 上加一個 `fn parse(stdout: &[u8]) -> Result<Self::Output, _>`
//! 之類的方法；因為 `GitQuery` 是 sealed trait，外部程式碼本來就不能自己實作它，之後幫
//! trait 加方法只影響本 crate 內的 10 個 impl，呼叫端（`argv`／`commands` 的用法）完全
//! 不受影響，所以這裡不先放 `unimplemented!()` 佔位方法。

use crate::{Oid, RepoPath, Side, target::DIFF_EXTRA};
use std::fmt;

/// 一個可以送給 git 的查詢：能提供給定目標下的完整 argv。
///
/// # Sealed
///
/// 只有本 crate 內的型別可以實作它（`private::Sealed`，`pub(crate)`，crate 外無法命名，
/// 寫法照 `herdr-client/src/client/request.rs`）。公開的查詢型別清單見 design D4：
/// [`Status`]、[`Refs`]、[`Log`]、[`CommitInfo`]、[`ChangedFiles`]、[`FileDiff`]、
/// [`MergeBase`]、[`BlobSize`]、[`Blob`]、[`VerifyCommit`]。如果外部型別能自己實作
/// `GitQuery`，就能繞過「查詢種類封閉」這條安全邊界，自訂任意子命令與引數交給
/// `GitRunner`（task 2.2）執行：
///
/// ```compile_fail,E0277
/// struct MyEvilQuery;
///
/// impl cockpit_git::GitQuery for MyEvilQuery {
///     type Output = ();
///
///     fn commands(&self, _target: &cockpit_git::GitTarget) -> Vec<Vec<String>> {
///         vec![vec!["push".to_string(), "--force".to_string()]]
///     }
/// }
/// ```
pub trait GitQuery: private::Sealed {
    /// 這個查詢解析後的輸出型別（task 2.3 補上解析器之前先一律是 `()`）。
    type Output;

    /// 給定執行目標，產生完整的 argv（含程式名）。一個查詢可能對應不止一次 git 呼叫
    /// （design D4：`Refs` 為 3 次、`ChangedFiles` 為 2 次），所以回傳值是「一批 argv」。
    fn commands(&self, target: &crate::GitTarget) -> Vec<Vec<String>>;

    /// 這個查詢每次呼叫的 stdout 上限（位元組，design D3：`Status` 4 MiB、`Log` 8 MiB、
    /// `ChangedFiles` 4 MiB、`FileDiff` 8 MiB、`Blob` 50 MiB，其餘 1 MiB）。task 2.2 的執行器
    /// 用這個值決定何時終止子程序；一個查詢若對應多次呼叫（`Refs`、`ChangedFiles`），上限對
    /// 每次呼叫各自套用，不是總和。
    fn stdout_cap(&self) -> usize;

    /// 超過 [`GitQuery::stdout_cap`] 時，執行器能不能回傳「已完整解析的前段＋
    /// `truncated: true`」而不是整個回 `TooLarge`（design D3：只有 [`Status`] 與
    /// [`ChangedFiles`] 是可截斷查詢，因為它們的輸出是 NUL 分隔的獨立紀錄，可以安全截斷在
    /// 最後一個完整紀錄；其餘查詢的單一輸出被截斷後不具備獨立可用的語意）。預設 `false`。
    fn truncatable(&self) -> bool {
        false
    }
}

/// [`GitQuery::stdout_cap`] 的位元組單位（design D3 的上限都以 MiB 表示）。
const ONE_MIB: usize = 1024 * 1024;
/// design D3：`Status` 的 stdout 上限。
const STATUS_STDOUT_CAP: usize = 4 * ONE_MIB;
/// design D3：`Log` 的 stdout 上限。
const LOG_STDOUT_CAP: usize = 8 * ONE_MIB;
/// design D3：`ChangedFiles` 的 stdout 上限（每次呼叫各自套用）。
const CHANGED_FILES_STDOUT_CAP: usize = 4 * ONE_MIB;
/// design D3：`FileDiff` 的 stdout 上限。
const FILE_DIFF_STDOUT_CAP: usize = 8 * ONE_MIB;
/// design D3：`Blob` 的 stdout 上限（同 5a 原始內容端點）。
const BLOB_STDOUT_CAP: usize = 50 * ONE_MIB;
/// design D4「BlobHead」（Ruling R10）：只取前 8192 位元組（同 5a 中繼資料端點判斷 `viewer`
/// 只讀取的位元組數），可截斷（讀滿即終止子程序，回傳前段）。
const BLOB_HEAD_STDOUT_CAP: usize = 8192;
/// design D3：「其餘」查詢（`Refs`、`CommitInfo`、`MergeBase`、`BlobSize`、`VerifyCommit`、
/// `BlobId`）的 stdout 上限。
const DEFAULT_STDOUT_CAP: usize = ONE_MIB;

pub(crate) mod private {
    pub trait Sealed {}
}

/// 查詢建構失敗的原因（例如 [`ChangedFiles`]／[`FileDiff`] 收到 design D5 不允許的兩側
/// 組合、或 [`BlobSize`]／[`Blob`] 收到 [`Side::Worktree`]／[`Side::Empty`]）。
///
/// 這是本 crate 內、建構查詢時的錯誤，**不是** D6 的 HTTP 錯誤代碼——HTTP 層（之後的
/// task）把它對應到 `bad_request` 之類的狀態碼與 `code`。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueryError {
    /// `from`／`to` 不是 design D5 允許的組合。
    UnsupportedSideCombination,
    /// 這個查詢的版本參數只接受 [`Side::Oid`] 或 [`Side::Index`]。
    UnsupportedSide,
}

impl fmt::Display for QueryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            QueryError::UnsupportedSideCombination => write!(f, "不支援的版本組合"),
            QueryError::UnsupportedSide => write!(f, "這個查詢不支援這種版本"),
        }
    }
}

impl std::error::Error for QueryError {}

fn strings(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| s.to_string()).collect()
}

// ---------------------------------------------------------------------------
// Status
// ---------------------------------------------------------------------------

/// `status --porcelain=v2 -z --branch --untracked-files=all`（design D4）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Status;

impl private::Sealed for Status {}

impl GitQuery for Status {
    type Output = crate::status::StatusOutput;

    fn commands(&self, target: &crate::GitTarget) -> Vec<Vec<String>> {
        let mut argv = target.base_argv();
        argv.extend(strings(&[
            "status",
            "--porcelain=v2",
            "-z",
            "--branch",
            "--untracked-files=all",
        ]));
        vec![argv]
    }

    fn stdout_cap(&self) -> usize {
        STATUS_STDOUT_CAP
    }

    fn truncatable(&self) -> bool {
        true
    }
}

// ---------------------------------------------------------------------------
// Refs
// ---------------------------------------------------------------------------

/// design D4：`for-each-ref` 列 refs、`rev-parse --verify -q HEAD` 取 HEAD 的 hash、
/// `symbolic-ref -q HEAD` 取 HEAD 指向的完整 refname——三次獨立呼叫，沒有共用旗標可以
/// 合併成一次（`for-each-ref` 的 `--format` 拿不到「HEAD 是否為 symbolic ref」這件事）。
/// 三者都不含使用者可控字串（`refs/heads`、`refs/tags` 等是命名空間常數，`HEAD` 是
/// git 自己的名字），所以都不需要 `--end-of-options`。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Refs;

impl private::Sealed for Refs {}

/// `for-each-ref` 的 `--format`：refname、objectname、剝皮後的 objectname、objecttype、
/// 剝皮後的 objecttype、symref，以 NUL 分隔（design D4「附註 tag 以 `*objectname`（剝皮後的
/// 物件）為準」；ui-fixes task 3.1／design D4：型別欄位用來判斷是否為 commit）。
const REFS_FORMAT: &str = "--format=%(refname)%00%(objectname)%00%(*objectname)%00%(objecttype)%00%(*objecttype)%00%(symref)";

impl GitQuery for Refs {
    type Output = crate::refs::RefsOutput;

    fn commands(&self, target: &crate::GitTarget) -> Vec<Vec<String>> {
        let mut for_each_ref = target.base_argv();
        for_each_ref.extend(strings(&[
            "for-each-ref",
            REFS_FORMAT,
            "refs/heads",
            "refs/remotes",
            "refs/tags",
        ]));

        let mut rev_parse_head = target.base_argv();
        rev_parse_head.extend(strings(&["rev-parse", "--verify", "-q", "HEAD"]));

        let mut symbolic_ref_head = target.base_argv();
        symbolic_ref_head.extend(strings(&["symbolic-ref", "-q", "HEAD"]));

        vec![for_each_ref, rev_parse_head, symbolic_ref_head]
    }

    fn stdout_cap(&self) -> usize {
        DEFAULT_STDOUT_CAP
    }
}

// ---------------------------------------------------------------------------
// Log
// ---------------------------------------------------------------------------

/// `log --date-order -z --format=... -n <上限> --end-of-options <tips…>`（design D4）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Log {
    /// 起點 commit（design D5：由呼叫端從 `Refs` 或前端指定的 `tip` 解析出，逐字比對過）。
    pub tips: Vec<Oid>,
    /// `-n` 的值（`offset + limit`；spec 的 `offset`／`limit` 上限由 HTTP 層驗證）。
    pub limit: u32,
}

impl private::Sealed for Log {}

const LOG_FORMAT: &str = "--format=%H%x00%P%x00%an%x00%ae%x00%at%x00%s";

impl GitQuery for Log {
    type Output = crate::log::LogOutput;

    fn commands(&self, target: &crate::GitTarget) -> Vec<Vec<String>> {
        let mut argv = target.base_argv();
        argv.extend(strings(&["log", "--date-order", "-z", LOG_FORMAT, "-n"]));
        argv.push(self.limit.to_string());
        argv.push("--end-of-options".to_string());
        argv.extend(self.tips.iter().map(|o| o.as_str().to_string()));
        vec![argv]
    }

    fn stdout_cap(&self) -> usize {
        LOG_STDOUT_CAP
    }
}

// ---------------------------------------------------------------------------
// CommitInfo
// ---------------------------------------------------------------------------

/// `show -s -z --format=... --end-of-options <oid>`（design D4）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitInfo {
    pub oid: Oid,
}

impl private::Sealed for CommitInfo {}

const COMMIT_INFO_FORMAT: &str =
    "--format=%H%x00%P%x00%an%x00%ae%x00%at%x00%cn%x00%ce%x00%ct%x00%B";

impl GitQuery for CommitInfo {
    type Output = crate::commit_info::CommitInfoOutput;

    fn commands(&self, target: &crate::GitTarget) -> Vec<Vec<String>> {
        let mut argv = target.base_argv();
        argv.extend(strings(&[
            "show",
            "-s",
            "-z",
            COMMIT_INFO_FORMAT,
            "--end-of-options",
        ]));
        argv.push(self.oid.as_str().to_string());
        vec![argv]
    }

    fn stdout_cap(&self) -> usize {
        DEFAULT_STDOUT_CAP
    }
}

// ---------------------------------------------------------------------------
// 兩側組合 → diff／diff-tree 子命令（design D5、D4「ChangedFiles」；ChangedFiles 與
// FileDiff 共用同一套組合選擇邏輯）
// ---------------------------------------------------------------------------

/// 選出的 diff 家族子命令：名稱、子命令自己的旗標（`--cached`、`-r --root` 等，
/// 不含 diff 類共用的 [`DIFF_EXTRA]`）、版本引數（放在 `--end-of-options` 之後）。
struct DiffFamily {
    subcommand: &'static str,
    pre_rev_flags: &'static [&'static str],
    revs: Vec<String>,
}

/// design D5 允許的組合（`ChangedFiles`／`FileDiff` 共用）：
/// - `Index → Worktree`：`diff-files`（無版本引數，兩側都隱含）
/// - `Oid(a) → Worktree`：`diff-index <a>`（不加 `--cached`；合併衝突中的檔案，**僅
///   `FileDiff`**；未合併檔案的 `Index → Worktree` 會得到 `diff --cc` 三方格式，改以 HEAD 為
///   左側取代暫存區，Ruling R11——`ChangedFiles::new` 額外擋下這個組合，見該函式文件）
///
/// 工作區側一律用 plumbing（Ruling R13）：porcelain `git diff` 遇到 stat 變舊的檔案會刷新並
/// **重寫 `.git/index`**（取得 `index.lock`），`--no-optional-locks` 擋不住；而 diff 分頁每
/// 2 秒輪詢，會跟 agent 的 `git add`／`commit` 搶鎖。`diff-files`／`diff-index` 不刷新 stat，
/// 不寫 index。代價：它們預設輸出 raw（`FileDiff` 要補 `-p`），且「只 touch、內容沒變」的檔案
/// 會被 `--name-status` 誤報 `M`（`ChangedFiles` 的解析器負責濾掉，見 `changed_files.rs`）。
///
/// 其餘：
/// - `Oid(a) → Index`：`diff --cached <a>`（已暫存，與指定的 commit 比較）
/// - `Empty → Index`：`diff --cached`（repo 還沒有任何 commit，probe ⑥ 實測不需要
///   偵測「有沒有 HEAD」，git 自己拿空樹當基準）
/// - `Oid(a) → Oid(b)`：`diff <a> <b>`
/// - `Empty → Oid(b)`：`diff-tree -r --root <b>`（根 commit，probe ⑥ 實測輸出格式）
///
/// 其餘組合（含 `Empty → Worktree`：未追蹤檔案，design D7 不經 git；`Worktree → *`；
/// `Index → Index` 等）回 [`QueryError::UnsupportedSideCombination`]。
fn diff_family(from: &Side, to: &Side) -> Result<DiffFamily, QueryError> {
    match (from, to) {
        (Side::Index, Side::Worktree) => Ok(DiffFamily {
            subcommand: "diff-files",
            pre_rev_flags: &[],
            revs: Vec::new(),
        }),
        (Side::Oid(a), Side::Worktree) => Ok(DiffFamily {
            subcommand: "diff-index",
            pre_rev_flags: &[],
            revs: vec![a.as_str().to_string()],
        }),
        (Side::Oid(a), Side::Index) => Ok(DiffFamily {
            subcommand: "diff",
            pre_rev_flags: &["--cached"],
            revs: vec![a.as_str().to_string()],
        }),
        (Side::Empty, Side::Index) => Ok(DiffFamily {
            subcommand: "diff",
            pre_rev_flags: &["--cached"],
            revs: Vec::new(),
        }),
        (Side::Oid(a), Side::Oid(b)) => Ok(DiffFamily {
            subcommand: "diff",
            pre_rev_flags: &[],
            revs: vec![a.as_str().to_string(), b.as_str().to_string()],
        }),
        (Side::Empty, Side::Oid(b)) => Ok(DiffFamily {
            subcommand: "diff-tree",
            pre_rev_flags: &["-r", "--root"],
            revs: vec![b.as_str().to_string()],
        }),
        _ => Err(QueryError::UnsupportedSideCombination),
    }
}

// ---------------------------------------------------------------------------
// ChangedFiles
// ---------------------------------------------------------------------------

/// design D4「ChangedFiles」：task 1.2 實測 `--name-status` 與 `--numstat` 不能同一次
/// 輸出（`git-review-probe.md` ⑤），所以固定分兩次呼叫，用同一個查詢型別的
/// [`GitQuery::commands`] 回傳兩組 argv（而不是拆成兩個查詢型別）——這樣呼叫端
/// （task 2.2 的執行器）不需要知道「這個查詢背後其實是兩次呼叫」以外的任何細節，
/// task 2.3 的解析器再以路徑合併兩次呼叫的結果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangedFiles {
    from: Side,
    to: Side,
    family_subcommand: &'static str,
    family_pre_rev_flags: &'static [&'static str],
    family_revs: Vec<String>,
}

impl private::Sealed for ChangedFiles {}

impl ChangedFiles {
    /// 依 design D5 驗證 `from`→`to` 組合；不允許的組合回
    /// [`QueryError::UnsupportedSideCombination`]。`Oid → Worktree`（合併衝突中的檔案）雖然
    /// `diff_family` 認得，但那個組合僅供 [`FileDiff`] 使用（spec「變更檔案清單端點」的允許
    /// 組合清單不含它；衝突組只開單檔 diff，不會列出檔案清單），這裡額外擋下，不能只靠
    /// `diff_family` 共用的選擇邏輯。
    pub fn new(from: Side, to: Side) -> Result<ChangedFiles, QueryError> {
        if matches!((&from, &to), (Side::Oid(_), Side::Worktree)) {
            return Err(QueryError::UnsupportedSideCombination);
        }
        let family = diff_family(&from, &to)?;
        Ok(ChangedFiles {
            from,
            to,
            family_subcommand: family.subcommand,
            family_pre_rev_flags: family.pre_rev_flags,
            family_revs: family.revs,
        })
    }

    pub fn from(&self) -> &Side {
        &self.from
    }

    pub fn to(&self) -> &Side {
        &self.to
    }

    /// 底層用的 diff 家族子命令（`"diff"`、`"diff-tree"`、`"diff-files"` 或 `"diff-index"`）。`pub(crate)` 供 task 2.3 的
    /// 解析器（`changed_files.rs`）判斷要不要跳過 `diff-tree -r --root` 多印的第一行
    /// commit hash（`git-review-probe.md` ⑥），以及 `diff-files` 要不要濾掉 stat-dirty 假陽性
    /// （Ruling R13）；不對外公開，呼叫端不需要、也不應該知道
    /// 底層用的是哪個 git 子命令。
    pub(crate) fn family_subcommand(&self) -> &'static str {
        self.family_subcommand
    }

    fn one_call(&self, target: &crate::GitTarget, mode_flag: &str) -> Vec<String> {
        let mut argv = target.base_argv();
        argv.push(self.family_subcommand.to_string());
        argv.extend(DIFF_EXTRA.iter().map(|s| s.to_string()));
        argv.extend(self.family_pre_rev_flags.iter().map(|s| s.to_string()));
        argv.extend(strings(&["-z", "-M", mode_flag]));
        if !self.family_revs.is_empty() {
            argv.push("--end-of-options".to_string());
            argv.extend(self.family_revs.clone());
        }
        argv
    }
}

impl GitQuery for ChangedFiles {
    type Output = crate::changed_files::ChangedFilesOutput;

    fn commands(&self, target: &crate::GitTarget) -> Vec<Vec<String>> {
        vec![
            self.one_call(target, "--name-status"),
            self.one_call(target, "--numstat"),
        ]
    }

    fn stdout_cap(&self) -> usize {
        CHANGED_FILES_STDOUT_CAP
    }

    fn truncatable(&self) -> bool {
        true
    }
}

// ---------------------------------------------------------------------------
// FileDiff
// ---------------------------------------------------------------------------

/// design D4「FileDiff」：同 [`ChangedFiles`] 的兩側組合選擇，但輸出 patch
/// （`-M -U3 -- <old_path> <path>`）而不是 `--name-status`／`--numstat`，所以只需要
/// 一次呼叫。`Side::Empty → Side::Worktree`（未追蹤檔案）不在允許組合內——design D7：
/// 那種情況不經 git，由呼叫端直接讀檔案，不會建構這個查詢型別。
///
/// plumbing 子命令（`Empty → Oid` 的根 commit 用 `diff-tree`；工作區側用 `diff-files`／
/// `diff-index`，Ruling R13）預設輸出與 `diff` 不同：不加 `-p` 時是 `--raw` 紀錄列表，不是
/// patch；[`ChangedFiles`] 用的就是這個預設格式，但 `FileDiff` 需要 patch，所以對非 `diff` 的
/// 子命令額外加 `-p`（`git diff` 則本來就預設輸出 patch，不需要加）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileDiff {
    from: Side,
    to: Side,
    path: RepoPath,
    old_path: Option<RepoPath>,
    family_subcommand: &'static str,
    family_pre_rev_flags: &'static [&'static str],
    family_revs: Vec<String>,
}

impl private::Sealed for FileDiff {}

impl FileDiff {
    /// `old_path`：改名時的原路徑（放在 `path` 之前，`git diff -M` 的慣例：先舊路徑、
    /// 後新路徑）。
    pub fn new(
        from: Side,
        to: Side,
        path: RepoPath,
        old_path: Option<RepoPath>,
    ) -> Result<FileDiff, QueryError> {
        let family = diff_family(&from, &to)?;
        Ok(FileDiff {
            from,
            to,
            path,
            old_path,
            family_subcommand: family.subcommand,
            family_pre_rev_flags: family.pre_rev_flags,
            family_revs: family.revs,
        })
    }

    pub fn from(&self) -> &Side {
        &self.from
    }

    pub fn to(&self) -> &Side {
        &self.to
    }
}

impl GitQuery for FileDiff {
    type Output = crate::file_diff::FileDiffOutput;

    fn commands(&self, target: &crate::GitTarget) -> Vec<Vec<String>> {
        let mut argv = target.base_argv();
        argv.push(self.family_subcommand.to_string());
        argv.extend(DIFF_EXTRA.iter().map(|s| s.to_string()));
        argv.extend(self.family_pre_rev_flags.iter().map(|s| s.to_string()));
        if self.family_subcommand != "diff" {
            // diff-tree／diff-files／diff-index 預設是 raw 格式，要 -p 才會輸出 patch（見型別文件）。
            argv.push("-p".to_string());
        }
        argv.extend(strings(&["-M", "-U3"]));
        if !self.family_revs.is_empty() {
            argv.push("--end-of-options".to_string());
            argv.extend(self.family_revs.clone());
        }
        argv.push("--".to_string());
        if let Some(old_path) = &self.old_path {
            argv.push(old_path.as_git_str());
        }
        argv.push(self.path.as_git_str());
        vec![argv]
    }

    fn stdout_cap(&self) -> usize {
        FILE_DIFF_STDOUT_CAP
    }
}

// ---------------------------------------------------------------------------
// MergeBase
// ---------------------------------------------------------------------------

/// `merge-base --end-of-options <a> <b>`（design D4）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergeBase {
    pub a: Oid,
    pub b: Oid,
}

impl private::Sealed for MergeBase {}

impl GitQuery for MergeBase {
    type Output = ();

    fn commands(&self, target: &crate::GitTarget) -> Vec<Vec<String>> {
        let mut argv = target.base_argv();
        argv.push("merge-base".to_string());
        argv.push("--end-of-options".to_string());
        argv.push(self.a.as_str().to_string());
        argv.push(self.b.as_str().to_string());
        vec![argv]
    }

    fn stdout_cap(&self) -> usize {
        DEFAULT_STDOUT_CAP
    }
}

// ---------------------------------------------------------------------------
// BlobSize／Blob
// ---------------------------------------------------------------------------

/// `BlobSize`／`Blob` 的 `<rev>:<path>`：`rev` 只接受 [`Side::Oid`] 或 [`Side::Index`]
/// （design D4「暫存區為 `:<path>`」），其餘變體回 [`QueryError::UnsupportedSide`]。
fn rev_path_token(rev: &Side, path: &RepoPath) -> Result<String, QueryError> {
    let rev_prefix = match rev {
        Side::Oid(oid) => oid.as_str().to_string(),
        Side::Index => String::new(),
        Side::Worktree | Side::Empty => return Err(QueryError::UnsupportedSide),
    };
    Ok(format!("{rev_prefix}:{}", path.as_git_str()))
}

/// `cat-file -s <oid-or-index>:<path>`（design D4）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlobSize {
    token: String,
}

impl private::Sealed for BlobSize {}

impl BlobSize {
    pub fn new(rev: Side, path: RepoPath) -> Result<BlobSize, QueryError> {
        Ok(BlobSize {
            token: rev_path_token(&rev, &path)?,
        })
    }
}

impl GitQuery for BlobSize {
    type Output = ();

    fn commands(&self, target: &crate::GitTarget) -> Vec<Vec<String>> {
        let mut argv = target.base_argv();
        argv.push("cat-file".to_string());
        argv.push("-s".to_string());
        argv.push(self.token.clone());
        vec![argv]
    }

    fn stdout_cap(&self) -> usize {
        DEFAULT_STDOUT_CAP
    }
}

/// `cat-file blob <oid-or-index>:<path>`（design D4）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Blob {
    token: String,
}

impl private::Sealed for Blob {}

impl Blob {
    pub fn new(rev: Side, path: RepoPath) -> Result<Blob, QueryError> {
        Ok(Blob {
            token: rev_path_token(&rev, &path)?,
        })
    }
}

impl GitQuery for Blob {
    type Output = ();

    fn commands(&self, target: &crate::GitTarget) -> Vec<Vec<String>> {
        let mut argv = target.base_argv();
        argv.push("cat-file".to_string());
        argv.push("blob".to_string());
        argv.push(self.token.clone());
        vec![argv]
    }

    fn stdout_cap(&self) -> usize {
        BLOB_STDOUT_CAP
    }
}

// ---------------------------------------------------------------------------
// BlobHead（git-review task 3.3 fix round 2；Ruling R10）
// ---------------------------------------------------------------------------

/// `cat-file blob <oid-or-index>:<path>`——argv 與 [`Blob`] 完全相同，只有 `stdout_cap`／
/// `truncatable` 不同（design D4「BlobHead」；Ruling R10）：只取前 8192 位元組（同 5a 中繼
/// 資料端點判斷 `viewer` 只讀取的位元組數），讀滿即由執行器終止子程序、回傳前段
/// （`truncated: true`）。`meta` 端點用它依 5a 的規則分類 `viewer`，不必像 `Blob` 那樣讀取
/// 整份內容（可能到 50 MiB）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlobHead {
    token: String,
}

impl private::Sealed for BlobHead {}

impl BlobHead {
    pub fn new(rev: Side, path: RepoPath) -> Result<BlobHead, QueryError> {
        Ok(BlobHead {
            token: rev_path_token(&rev, &path)?,
        })
    }
}

impl GitQuery for BlobHead {
    type Output = ();

    fn commands(&self, target: &crate::GitTarget) -> Vec<Vec<String>> {
        let mut argv = target.base_argv();
        argv.push("cat-file".to_string());
        argv.push("blob".to_string());
        argv.push(self.token.clone());
        vec![argv]
    }

    fn stdout_cap(&self) -> usize {
        BLOB_HEAD_STDOUT_CAP
    }

    fn truncatable(&self) -> bool {
        true
    }
}

// ---------------------------------------------------------------------------
// BlobId（git-review task 3.3 fix round 1；Ruling R8）
// ---------------------------------------------------------------------------

/// `rev-parse --verify -q --end-of-options <oid-or-index>:<path>`（design D4「BlobId」；
/// Ruling R8）：只取得檔案內容的物件 hash，不讀取內容本身。
///
/// 取代原先「`meta` 端點讀整份 `Blob` 內容算雜湊」的做法——暫存區版本的分頁每 2 秒輪詢
/// `meta`，先前的做法每次都要整份讀取最多 50 MiB 才能知道內容有沒有變，且算出來的值也不是
/// spec 要求的「物件 hash」（只是自訂內容雜湊）。`rev-parse` 直接查 tree entry 記錄的物件 id，
/// 不需要讀取／雜湊實際內容，同時滿足「不讀取 blob 內容」與「回傳真正的物件 hash」兩個要求。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlobId {
    token: String,
}

impl private::Sealed for BlobId {}

impl BlobId {
    pub fn new(rev: Side, path: RepoPath) -> Result<BlobId, QueryError> {
        Ok(BlobId {
            token: rev_path_token(&rev, &path)?,
        })
    }
}

impl GitQuery for BlobId {
    type Output = String;

    fn commands(&self, target: &crate::GitTarget) -> Vec<Vec<String>> {
        let mut argv = target.base_argv();
        argv.extend(strings(&[
            "rev-parse",
            "--verify",
            "-q",
            "--end-of-options",
        ]));
        argv.push(self.token.clone());
        vec![argv]
    }

    fn stdout_cap(&self) -> usize {
        DEFAULT_STDOUT_CAP
    }
}

// ---------------------------------------------------------------------------
// VerifyCommit
// ---------------------------------------------------------------------------

/// `rev-parse --verify -q --end-of-options <oid>^{commit}`（design D4）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifyCommit {
    pub oid: Oid,
}

impl private::Sealed for VerifyCommit {}

impl GitQuery for VerifyCommit {
    type Output = ();

    fn commands(&self, target: &crate::GitTarget) -> Vec<Vec<String>> {
        let mut argv = target.base_argv();
        argv.extend(strings(&[
            "rev-parse",
            "--verify",
            "-q",
            "--end-of-options",
        ]));
        argv.push(format!("{}^{{commit}}", self.oid.as_str()));
        vec![argv]
    }

    fn stdout_cap(&self) -> usize {
        DEFAULT_STDOUT_CAP
    }
}

// ---------------------------------------------------------------------------
// RepoIdentity（repo-projects task 2.1；design D1）
// ---------------------------------------------------------------------------

/// `rev-parse --path-format=absolute --git-common-dir --git-dir --show-toplevel`：判定
/// 目標目錄屬於哪個 git repo（repo-projects design D1）。成功時 stdout 三行，依引數順序：
/// 共同 `.git` 目錄、該工作樹自己的 git 目錄、工作樹根目錄。
///
/// `--path-format=absolute` 必須放在三個路徑旗標**之前**（它只影響其後的引數；git 官方文件
/// `git-rev-parse` 的 `--path-format` 節），否則子目錄會得到相對路徑。引數都不含使用者可控
/// 字串（目標目錄經 `-C` 傳入），所以不需要 `--end-of-options`。解析與「不是 repo」的判別見
/// `repo_identity.rs`。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RepoIdentity;

impl private::Sealed for RepoIdentity {}

impl GitQuery for RepoIdentity {
    type Output = Option<crate::repo_identity::RepoIdentityOutput>;

    fn commands(&self, target: &crate::GitTarget) -> Vec<Vec<String>> {
        let mut argv = target.base_argv();
        argv.extend(strings(&[
            "rev-parse",
            "--path-format=absolute",
            "--git-common-dir",
            "--git-dir",
            "--show-toplevel",
        ]));
        vec![argv]
    }

    fn stdout_cap(&self) -> usize {
        DEFAULT_STDOUT_CAP
    }
}

// ---------------------------------------------------------------------------
// CurrentBranch（openspec-stage-sync task 2.1；design D7）
// ---------------------------------------------------------------------------

/// `symbolic-ref -q HEAD`：HEAD 指向的完整 refname（design D7）。
///
/// **不用 `--short`**：存在與分支同名的 tag 時，`--short` 會把 `refs/heads/x` 縮成帶歧義的
/// `heads/x`，拿到錯的分支名；改由 [`CurrentBranch::parse`] 自行去掉 `refs/heads/` 前綴。
/// `-q`：HEAD 是 detached（不是符號參照）時不印錯誤訊息、安靜地以 exit 1 結束（git 官方文件
/// `git-symbolic-ref`），解析器據此回 `None`，不當成錯誤。引數都是常數，不需要
/// `--end-of-options`。此查詢只供 OpenSpec 進度偵測內部使用，不對應任何 HTTP 端點。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CurrentBranch;

impl private::Sealed for CurrentBranch {}

impl GitQuery for CurrentBranch {
    type Output = Option<String>;

    fn commands(&self, target: &crate::GitTarget) -> Vec<Vec<String>> {
        let mut argv = target.base_argv();
        argv.extend(strings(&["symbolic-ref", "-q", "HEAD"]));
        vec![argv]
    }

    fn stdout_cap(&self) -> usize {
        DEFAULT_STDOUT_CAP
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Blob, BlobHead, BlobId, BlobSize, ChangedFiles, CommitInfo, CurrentBranch, FileDiff,
        GitQuery, Log, MergeBase, QueryError, Refs, RepoIdentity, Status, VerifyCommit,
    };
    use crate::{GitTarget, Oid, RepoPath, Side};

    fn native() -> GitTarget {
        GitTarget::Native {
            path: r"C:\repo".to_string(),
        }
    }

    fn wsl() -> GitTarget {
        GitTarget::Wsl {
            distro: "Ubuntu-24.04".to_string(),
            posix: "/home/me/repo".to_string(),
        }
    }

    fn oid(byte: char) -> Oid {
        Oid::parse(&std::iter::repeat_n(byte, 40).collect::<String>()).expect("測試 oid 應合法")
    }

    /// 共同前綴：`git -C <path> <固定前綴>`／`wsl.exe -d <distro> --exec env LC_ALL=C git -C <posix> <固定前綴>`。
    fn assert_common_prefix(argv: &[String], target: &GitTarget) {
        let expected = target.base_argv();
        assert_eq!(&argv[..expected.len()], expected.as_slice());
    }

    #[test]
    fn status_argv_is_single_call_with_no_end_of_options() {
        for target in [native(), wsl()] {
            let mut calls = Status.commands(&target);
            assert_eq!(calls.len(), 1);
            let argv = calls.remove(0);
            assert_common_prefix(&argv, &target);
            let tail = &argv[target.base_argv().len()..];
            assert_eq!(
                tail,
                [
                    "status",
                    "--porcelain=v2",
                    "-z",
                    "--branch",
                    "--untracked-files=all"
                ]
            );
            assert!(!argv.contains(&"--end-of-options".to_string()));
        }
    }

    /// repo-projects task 2.1（design D1）：單次呼叫，`--path-format=absolute` 在三個路徑旗標
    /// 之前，順序固定為 common-dir、git-dir、show-toplevel（輸出三行依此順序）。
    #[test]
    fn repo_identity_argv_is_single_call_with_path_format_before_path_flags() {
        for target in [native(), wsl()] {
            let mut calls = RepoIdentity.commands(&target);
            assert_eq!(calls.len(), 1);
            let argv = calls.remove(0);
            assert_common_prefix(&argv, &target);
            assert_eq!(
                &argv[target.base_argv().len()..],
                [
                    "rev-parse",
                    "--path-format=absolute",
                    "--git-common-dir",
                    "--git-dir",
                    "--show-toplevel"
                ]
            );
            assert!(!argv.contains(&"--end-of-options".to_string()));
        }
    }

    /// openspec-stage-sync task 2.1（design D7）：單次呼叫 `symbolic-ref -q HEAD`，不用
    /// `--short`（同名 tag 時會縮成帶歧義的 `heads/x`），也不需要 `--end-of-options`。
    #[test]
    fn current_branch_argv_is_single_symbolic_ref_without_short() {
        for target in [native(), wsl()] {
            let mut calls = CurrentBranch.commands(&target);
            assert_eq!(calls.len(), 1);
            let argv = calls.remove(0);
            assert_common_prefix(&argv, &target);
            assert_eq!(
                &argv[target.base_argv().len()..],
                ["symbolic-ref", "-q", "HEAD"]
            );
            assert!(!argv.contains(&"--short".to_string()));
            assert!(!argv.contains(&"--end-of-options".to_string()));
        }
    }

    #[test]
    fn refs_argv_is_three_calls() {
        for target in [native(), wsl()] {
            let calls = Refs.commands(&target);
            assert_eq!(calls.len(), 3);
            let prefix_len = target.base_argv().len();

            assert_eq!(
                &calls[0][prefix_len..],
                [
                    "for-each-ref",
                    "--format=%(refname)%00%(objectname)%00%(*objectname)%00%(objecttype)%00%(*objecttype)%00%(symref)",
                    "refs/heads",
                    "refs/remotes",
                    "refs/tags",
                ]
            );
            assert_eq!(
                &calls[1][prefix_len..],
                ["rev-parse", "--verify", "-q", "HEAD"]
            );
            assert_eq!(&calls[2][prefix_len..], ["symbolic-ref", "-q", "HEAD"]);
        }
    }

    #[test]
    fn log_argv_places_end_of_options_after_git_flags_and_before_tips() {
        for target in [native(), wsl()] {
            let query = Log {
                tips: vec![oid('a'), oid('b')],
                limit: 200,
            };
            let mut calls = query.commands(&target);
            assert_eq!(calls.len(), 1);
            let argv = calls.remove(0);
            let prefix_len = target.base_argv().len();
            assert_eq!(
                &argv[prefix_len..],
                [
                    "log",
                    "--date-order",
                    "-z",
                    "--format=%H%x00%P%x00%an%x00%ae%x00%at%x00%s",
                    "-n",
                    "200",
                    "--end-of-options",
                    oid('a').as_str(),
                    oid('b').as_str(),
                ]
            );
        }
    }

    #[test]
    fn commit_info_argv() {
        let query = CommitInfo { oid: oid('c') };
        let target = native();
        let mut calls = query.commands(&target);
        let argv = calls.remove(0);
        let prefix_len = target.base_argv().len();
        assert_eq!(
            &argv[prefix_len..],
            [
                "show",
                "-s",
                "-z",
                "--format=%H%x00%P%x00%an%x00%ae%x00%at%x00%cn%x00%ce%x00%ct%x00%B",
                "--end-of-options",
                oid('c').as_str(),
            ]
        );
    }

    #[test]
    fn merge_base_argv() {
        let query = MergeBase {
            a: oid('a'),
            b: oid('b'),
        };
        let target = native();
        let argv = query.commands(&target).remove(0);
        let prefix_len = target.base_argv().len();
        assert_eq!(
            &argv[prefix_len..],
            [
                "merge-base",
                "--end-of-options",
                oid('a').as_str(),
                oid('b').as_str()
            ]
        );
    }

    #[test]
    fn verify_commit_argv_appends_commit_suffix_to_single_token() {
        let query = VerifyCommit { oid: oid('a') };
        let target = native();
        let argv = query.commands(&target).remove(0);
        let prefix_len = target.base_argv().len();
        assert_eq!(
            &argv[prefix_len..],
            [
                "rev-parse",
                "--verify",
                "-q",
                "--end-of-options",
                &format!("{}^{{commit}}", oid('a'))
            ]
        );
    }

    #[test]
    fn blob_size_uses_oid_colon_path_token() {
        let query = BlobSize::new(Side::Oid(oid('a')), RepoPath::parse("docs/a.md").unwrap())
            .expect("Oid 應合法");
        let target = native();
        let argv = query.commands(&target).remove(0);
        let prefix_len = target.base_argv().len();
        assert_eq!(
            &argv[prefix_len..],
            ["cat-file", "-s", &format!("{}:docs/a.md", oid('a'))]
        );
        assert!(!argv.contains(&"--end-of-options".to_string()));
    }

    #[test]
    fn blob_uses_index_colon_path_token_when_rev_is_index() {
        let query =
            Blob::new(Side::Index, RepoPath::parse("docs/a.md").unwrap()).expect("Index 應合法");
        let target = native();
        let argv = query.commands(&target).remove(0);
        let prefix_len = target.base_argv().len();
        assert_eq!(&argv[prefix_len..], ["cat-file", "blob", ":docs/a.md"]);
    }

    #[test]
    fn blob_rejects_worktree_and_empty_rev() {
        let path = RepoPath::parse("docs/a.md").unwrap();
        assert_eq!(
            Blob::new(Side::Worktree, path.clone()),
            Err(QueryError::UnsupportedSide)
        );
        assert_eq!(
            Blob::new(Side::Empty, path),
            Err(QueryError::UnsupportedSide)
        );
    }

    /// git-review task 3.3 fix round 2（Ruling R10）：`BlobHead` 的 argv 與 `Blob` 逐字相同
    /// （commit hash 版本）。
    #[test]
    fn blob_head_argv_matches_blob_for_oid() {
        let path = RepoPath::parse("docs/a.md").unwrap();
        let blob_head = BlobHead::new(Side::Oid(oid('a')), path.clone()).expect("Oid 應合法");
        let blob = Blob::new(Side::Oid(oid('a')), path).expect("Oid 應合法");
        let target = native();
        assert_eq!(
            blob_head.commands(&target),
            blob.commands(&target),
            "BlobHead 與 Blob 的 argv 應逐字相同"
        );
    }

    /// 暫存區版本：token 前綴為空（`:docs/a.md`），argv 與 `Blob` 相同。
    #[test]
    fn blob_head_uses_index_colon_path_token_when_rev_is_index() {
        let query = BlobHead::new(Side::Index, RepoPath::parse("docs/a.md").unwrap())
            .expect("Index 應合法");
        let target = native();
        let argv = query.commands(&target).remove(0);
        let prefix_len = target.base_argv().len();
        assert_eq!(&argv[prefix_len..], ["cat-file", "blob", ":docs/a.md"]);
    }

    #[test]
    fn blob_head_rejects_worktree_and_empty_rev() {
        let path = RepoPath::parse("docs/a.md").unwrap();
        assert_eq!(
            BlobHead::new(Side::Worktree, path.clone()),
            Err(QueryError::UnsupportedSide)
        );
        assert_eq!(
            BlobHead::new(Side::Empty, path),
            Err(QueryError::UnsupportedSide)
        );
    }

    /// git-review task 3.3 fix round 2（Ruling R10）：`BlobHead` 的 `stdout_cap` 為 8192、
    /// `truncatable` 為 true——跟 `Blob`（50 MiB、不可截斷）刻意不同，這是它存在的理由。
    #[test]
    fn blob_head_stdout_cap_is_8192_and_truncatable() {
        let query = BlobHead::new(Side::Index, RepoPath::parse("a.rs").unwrap()).unwrap();
        assert_eq!(query.stdout_cap(), 8192);
        assert!(query.truncatable());
    }

    /// git-review task 3.3 fix round 1（Ruling R8）：`BlobId` 的 argv——commit hash 版本
    /// 帶完整 oid，`--verify -q --end-of-options` 放在 token 之前（同 `VerifyCommit` 的旗標
    /// 順序），不含 `^{commit}` 後綴（那是 `VerifyCommit` 專屬，`BlobId` 要驗的是
    /// `<rev>:<path>` 這個 blob token，不是 commit）。
    #[test]
    fn blob_id_uses_oid_colon_path_token() {
        let query = BlobId::new(Side::Oid(oid('a')), RepoPath::parse("docs/a.md").unwrap())
            .expect("Oid 應合法");
        let target = native();
        let argv = query.commands(&target).remove(0);
        let prefix_len = target.base_argv().len();
        assert_eq!(
            &argv[prefix_len..],
            [
                "rev-parse",
                "--verify",
                "-q",
                "--end-of-options",
                &format!("{}:docs/a.md", oid('a'))
            ]
        );
    }

    /// 暫存區版本：token 前綴為空（`:docs/a.md`），同 `BlobSize`／`Blob` 的既有規則。
    #[test]
    fn blob_id_uses_index_colon_path_token_when_rev_is_index() {
        let query =
            BlobId::new(Side::Index, RepoPath::parse("docs/a.md").unwrap()).expect("Index 應合法");
        let target = native();
        let argv = query.commands(&target).remove(0);
        let prefix_len = target.base_argv().len();
        assert_eq!(
            &argv[prefix_len..],
            [
                "rev-parse",
                "--verify",
                "-q",
                "--end-of-options",
                ":docs/a.md"
            ]
        );
    }

    #[test]
    fn blob_id_rejects_worktree_and_empty_rev() {
        let path = RepoPath::parse("docs/a.md").unwrap();
        assert_eq!(
            BlobId::new(Side::Worktree, path.clone()),
            Err(QueryError::UnsupportedSide)
        );
        assert_eq!(
            BlobId::new(Side::Empty, path),
            Err(QueryError::UnsupportedSide)
        );
    }

    /// WSL 目標下同樣使用 `wsl.exe --exec` 前綴（同其餘查詢的既有規則，這裡只驗證
    /// `BlobId` 也走同一條路，不是另開一條組裝邏輯）。
    #[test]
    fn blob_id_wsl_target_uses_exec_env_git_and_posix_path() {
        let query = BlobId::new(Side::Oid(oid('a')), RepoPath::parse("a.md").unwrap()).unwrap();
        let target = wsl();
        let argv = query.commands(&target);
        assert_eq!(argv.len(), 1);
        assert_eq!(
            &argv[0][..9],
            [
                "wsl.exe",
                "-d",
                "Ubuntu-24.04",
                "--exec",
                "env",
                "LC_ALL=C",
                "git",
                "-C",
                "/home/me/repo",
            ]
        );
    }

    #[test]
    fn changed_files_index_to_worktree_uses_diff_files_plumbing_with_no_revs() {
        let query = ChangedFiles::new(Side::Index, Side::Worktree).expect("應合法組合");
        let target = native();
        let calls = query.commands(&target);
        assert_eq!(calls.len(), 2);
        let prefix_len = target.base_argv().len();
        assert_eq!(
            &calls[0][prefix_len..],
            [
                "diff-files",
                "--no-ext-diff",
                "--no-textconv",
                "--no-color",
                "-z",
                "-M",
                "--name-status"
            ]
        );
        assert_eq!(
            &calls[1][prefix_len..],
            [
                "diff-files",
                "--no-ext-diff",
                "--no-textconv",
                "--no-color",
                "-z",
                "-M",
                "--numstat"
            ]
        );
        assert!(!calls[0].contains(&"--end-of-options".to_string()));
    }

    #[test]
    fn changed_files_oid_to_index_uses_cached_with_explicit_oid() {
        let query = ChangedFiles::new(Side::Oid(oid('a')), Side::Index).expect("應合法組合");
        let target = native();
        let calls = query.commands(&target);
        let prefix_len = target.base_argv().len();
        assert_eq!(
            &calls[0][prefix_len..],
            [
                "diff",
                "--no-ext-diff",
                "--no-textconv",
                "--no-color",
                "--cached",
                "-z",
                "-M",
                "--name-status",
                "--end-of-options",
                oid('a').as_str(),
            ]
        );
    }

    #[test]
    fn changed_files_empty_to_index_uses_cached_without_oid() {
        let query =
            ChangedFiles::new(Side::Empty, Side::Index).expect("應合法組合（無 commit 的 repo）");
        let target = native();
        let calls = query.commands(&target);
        let prefix_len = target.base_argv().len();
        assert_eq!(
            &calls[0][prefix_len..],
            [
                "diff",
                "--no-ext-diff",
                "--no-textconv",
                "--no-color",
                "--cached",
                "-z",
                "-M",
                "--name-status"
            ]
        );
        assert!(!calls[0].contains(&"--end-of-options".to_string()));
    }

    #[test]
    fn changed_files_oid_to_oid_uses_plain_diff_with_both_oids() {
        let query =
            ChangedFiles::new(Side::Oid(oid('a')), Side::Oid(oid('b'))).expect("應合法組合");
        let target = native();
        let calls = query.commands(&target);
        let prefix_len = target.base_argv().len();
        assert_eq!(
            &calls[0][prefix_len..],
            [
                "diff",
                "--no-ext-diff",
                "--no-textconv",
                "--no-color",
                "-z",
                "-M",
                "--name-status",
                "--end-of-options",
                oid('a').as_str(),
                oid('b').as_str(),
            ]
        );
    }

    #[test]
    fn changed_files_empty_to_oid_uses_diff_tree_root() {
        let query =
            ChangedFiles::new(Side::Empty, Side::Oid(oid('b'))).expect("應合法組合（根 commit）");
        let target = native();
        let calls = query.commands(&target);
        let prefix_len = target.base_argv().len();
        assert_eq!(
            &calls[0][prefix_len..],
            [
                "diff-tree",
                "--no-ext-diff",
                "--no-textconv",
                "--no-color",
                "-r",
                "--root",
                "-z",
                "-M",
                "--name-status",
                "--end-of-options",
                oid('b').as_str(),
            ]
        );
    }

    #[test]
    fn changed_files_rejects_unsupported_combinations() {
        for (from, to) in [
            (Side::Worktree, Side::Index),
            (Side::Empty, Side::Worktree),
            (Side::Index, Side::Oid(oid('a'))),
            (Side::Worktree, Side::Worktree),
        ] {
            assert_eq!(
                ChangedFiles::new(from, to),
                Err(QueryError::UnsupportedSideCombination)
            );
        }
    }

    /// 目視驗收缺陷 V1、Ruling R11：`Oid → Worktree`（合併衝突中的檔案）僅供 `FileDiff` 使用，
    /// `ChangedFiles`（「變更檔案清單端點」）不允許這個組合，即使 `diff_family` 認得它。
    #[test]
    fn changed_files_rejects_oid_to_worktree_even_though_file_diff_allows_it() {
        assert_eq!(
            ChangedFiles::new(Side::Oid(oid('a')), Side::Worktree),
            Err(QueryError::UnsupportedSideCombination)
        );
    }

    #[test]
    fn file_diff_index_to_worktree_has_trailing_dashdash_and_path() {
        let query = FileDiff::new(
            Side::Index,
            Side::Worktree,
            RepoPath::parse("a.rs").unwrap(),
            None,
        )
        .expect("應合法組合");
        let target = native();
        let argv = query.commands(&target).remove(0);
        let prefix_len = target.base_argv().len();
        assert_eq!(
            &argv[prefix_len..],
            [
                "diff-files",
                "--no-ext-diff",
                "--no-textconv",
                "--no-color",
                "-p",
                "-M",
                "-U3",
                "--",
                "a.rs"
            ]
        );
    }

    /// 目視驗收缺陷 V1、Ruling R11：`Oid → Worktree`（合併衝突中的檔案）——固定前綴＋diff 類
    /// 旗標＋`diff -M -U3 --end-of-options <oid> -- <path>`（brief V1.1 指定的 argv 形狀）。
    #[test]
    fn file_diff_oid_to_worktree_has_end_of_options_oid_before_dashdash() {
        let query = FileDiff::new(
            Side::Oid(oid('a')),
            Side::Worktree,
            RepoPath::parse("c.txt").unwrap(),
            None,
        )
        .expect("Oid → Worktree 應為合法組合（Ruling R11）");
        let target = native();
        let argv = query.commands(&target).remove(0);
        let prefix_len = target.base_argv().len();
        assert_eq!(
            &argv[prefix_len..],
            [
                "diff-index",
                "--no-ext-diff",
                "--no-textconv",
                "--no-color",
                "-p",
                "-M",
                "-U3",
                "--end-of-options",
                oid('a').as_str(),
                "--",
                "c.txt"
            ]
        );
    }

    #[test]
    fn file_diff_with_rename_puts_old_path_before_path_after_dashdash() {
        let query = FileDiff::new(
            Side::Oid(oid('a')),
            Side::Oid(oid('b')),
            RepoPath::parse("new.rs").unwrap(),
            Some(RepoPath::parse("old.rs").unwrap()),
        )
        .expect("應合法組合");
        let target = native();
        let argv = query.commands(&target).remove(0);
        let prefix_len = target.base_argv().len();
        assert_eq!(
            &argv[prefix_len..],
            [
                "diff",
                "--no-ext-diff",
                "--no-textconv",
                "--no-color",
                "-M",
                "-U3",
                "--end-of-options",
                oid('a').as_str(),
                oid('b').as_str(),
                "--",
                "old.rs",
                "new.rs",
            ]
        );
    }

    #[test]
    fn file_diff_root_commit_uses_diff_tree_with_patch_flag() {
        let query = FileDiff::new(
            Side::Empty,
            Side::Oid(oid('b')),
            RepoPath::parse("a.rs").unwrap(),
            None,
        )
        .expect("應合法組合（根 commit）");
        let target = native();
        let argv = query.commands(&target).remove(0);
        let prefix_len = target.base_argv().len();
        assert_eq!(
            &argv[prefix_len..],
            [
                "diff-tree",
                "--no-ext-diff",
                "--no-textconv",
                "--no-color",
                "-r",
                "--root",
                "-p",
                "-M",
                "-U3",
                "--end-of-options",
                oid('b').as_str(),
                "--",
                "a.rs",
            ]
        );
    }

    #[test]
    fn file_diff_rejects_empty_to_worktree_because_it_does_not_go_through_git() {
        // design D7：未追蹤檔案不經 git，呼叫端不應該建構這個查詢；這裡確認建構期就擋下。
        assert_eq!(
            FileDiff::new(
                Side::Empty,
                Side::Worktree,
                RepoPath::parse("new.md").unwrap(),
                None
            ),
            Err(QueryError::UnsupportedSideCombination)
        );
    }

    /// design D3 的每查詢 stdout 上限與可截斷性（task 2.2）：純數值比對，不需要子程序。
    #[test]
    fn stdout_cap_and_truncatable_match_design_d3() {
        const ONE_MIB: usize = 1024 * 1024;

        assert_eq!(Status.stdout_cap(), 4 * ONE_MIB);
        assert!(Status.truncatable());

        assert_eq!(Refs.stdout_cap(), ONE_MIB);
        assert!(!Refs.truncatable());

        let log = Log {
            tips: vec![oid('a')],
            limit: 1,
        };
        assert_eq!(log.stdout_cap(), 8 * ONE_MIB);
        assert!(!log.truncatable());

        let commit_info = CommitInfo { oid: oid('a') };
        assert_eq!(commit_info.stdout_cap(), ONE_MIB);
        assert!(!commit_info.truncatable());

        let changed_files = ChangedFiles::new(Side::Index, Side::Worktree).unwrap();
        assert_eq!(changed_files.stdout_cap(), 4 * ONE_MIB);
        assert!(changed_files.truncatable());

        let file_diff = FileDiff::new(
            Side::Index,
            Side::Worktree,
            RepoPath::parse("a.rs").unwrap(),
            None,
        )
        .unwrap();
        assert_eq!(file_diff.stdout_cap(), 8 * ONE_MIB);
        assert!(!file_diff.truncatable());

        let merge_base = MergeBase {
            a: oid('a'),
            b: oid('b'),
        };
        assert_eq!(merge_base.stdout_cap(), ONE_MIB);
        assert!(!merge_base.truncatable());

        let blob_size = BlobSize::new(Side::Index, RepoPath::parse("a.rs").unwrap()).unwrap();
        assert_eq!(blob_size.stdout_cap(), ONE_MIB);
        assert!(!blob_size.truncatable());

        let blob = Blob::new(Side::Index, RepoPath::parse("a.rs").unwrap()).unwrap();
        assert_eq!(blob.stdout_cap(), 50 * ONE_MIB);
        assert!(!blob.truncatable());

        let verify_commit = VerifyCommit { oid: oid('a') };
        assert_eq!(verify_commit.stdout_cap(), ONE_MIB);
        assert!(!verify_commit.truncatable());

        let blob_id = BlobId::new(Side::Index, RepoPath::parse("a.rs").unwrap()).unwrap();
        assert_eq!(blob_id.stdout_cap(), ONE_MIB);
        assert!(!blob_id.truncatable());

        let blob_head = BlobHead::new(Side::Index, RepoPath::parse("a.rs").unwrap()).unwrap();
        assert_eq!(blob_head.stdout_cap(), 8192);
        assert!(blob_head.truncatable());

        assert_eq!(RepoIdentity.stdout_cap(), ONE_MIB);
        assert!(!RepoIdentity.truncatable());

        assert_eq!(CurrentBranch.stdout_cap(), ONE_MIB);
        assert!(!CurrentBranch.truncatable());
    }

    #[test]
    fn wsl_target_argv_uses_exec_env_git_and_posix_path_everywhere() {
        let target = wsl();
        let argv = Status.commands(&target).remove(0);
        assert_eq!(
            &argv[..9],
            [
                "wsl.exe",
                "-d",
                "Ubuntu-24.04",
                "--exec",
                "env",
                "LC_ALL=C",
                "git",
                "-C",
                "/home/me/repo",
            ]
        );
        assert!(!argv.contains(&"--cd".to_string()));
    }
}
