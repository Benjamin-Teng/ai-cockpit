//! git 端點：狀態、refs、commit 清單、commit 詳情、變更檔案清單、共同祖先（git-review task 3.2；
//! spec `git-review`「git 端點的共同規則」「狀態端點」「refs 端點」「commit 清單端點」「commit
//! 詳情端點」「變更檔案清單端點」「共同祖先端點」；design D1、D2、D3、D4、D5、D6、D8）。
//!
//! **執行 git 只能經 [`cockpit_git::GitRunner::run`]**：本模組完全不直接建立子程序，
//! 能執行的查詢種類由 `cockpit-git` 的 sealed `GitQuery` 在編譯期封閉（design D1 Goal）。
//! `GitRunner` 全程序共用一個（[`crate::http::AppState::git_runner`]），讓「同時最多 4 支
//! git 子程序」的並行上限對全部請求生效。
//!
//! 每個端點的處理流程固定：**解析並驗證全部參數（純函式，不碰檔案系統與子程序）→
//! [`crate::files::authorize_root`]（允許清單）→ 根目錄 `is_git` 為假回 [`GitApiError::NotGit`]
//! → 依根目錄選 [`cockpit_git::GitTarget`]（[`cockpit_git::select_target`]）→ 需要「這個 hash／ref
//! 在這個 repo 裡存在」的語意驗證（[`verify_commit`]、`log` 的 `ref` 比對）→ 執行查詢**——參數格式
//! 驗證（含 `ChangedFiles::new` 的兩側組合檢查）永遠排在 `authorize_root` 之前：即使 `root_id`
//! 對應到一個不存在或不可用的根目錄，壞掉的參數也一律先回 400，不會啟動任何子程序，也不會因為
//! 檔案系統推算而洩漏根目錄是否存在（同 file-review task 3.2 的既有原則）。
//!
//! query string 一律從 [`axum::extract::OriginalUri`] 自行解析（[`parse_query_pairs`]），不用
//! axum 內建的 `Query` extractor——同 `crate::files` 模組文件提到的「axum 會先解碼」的坑：本模組
//! 需要保留重複參數（`log` 的多個 `ref`／`tip`）與逐位元組可控的 percent-decode 規則，重用
//! [`crate::files::percent_decode_utf8`]（file-review task 3.2 已驗證過的同一套解碼規則）。

use std::collections::HashSet;

use axum::extract::rejection::PathRejection;
use axum::extract::{OriginalUri, Path, State};
use axum::http::{StatusCode, Uri, header};
use axum::response::{IntoResponse, Response};
use cockpit_files::{
    FilesError, IconTheme, RelPath, Root, Viewer, classify_viewer_bytes, read_capped, resolve,
};
use cockpit_git::{
    Blob, BlobHead, BlobId, BlobSize, ChangedFile, ChangedFiles, ChangedFilesOutput, CommitInfo,
    CommitInfoOutput, FileDiff, FileDiffOutput, GitParseError, GitQuery, GitRunner, GitTarget,
    GraphCommit, GraphHalf, GraphRow, Log, LogRow, MergeBase, Oid, RefKind, Refs, RefsOutput,
    RepoPath, RunOutput, RunnerError, Side, Status, StatusGroup, StatusOutput, VerifyCommit,
    layout, select_target, untracked_file_diff,
};
use serde::Serialize;

use crate::files::{FileApiError, authorize_root, percent_decode_utf8, raw_content_type};
use crate::http::{AppState, coded_error_response, with_no_store_headers};

// ---------------------------------------------------------------------------
// 錯誤（spec「git 端點的共同規則」新增的代碼；file-review 已定義的代碼一律沿用
// [`FileApiError`]，經 [`GitEndpointError`] 合併成單一回應）
// ---------------------------------------------------------------------------

/// git 端點自己的錯誤（不含 `crate::files::authorize_root` 已覆蓋的
/// `bad_request`／`runtime_unknown`／`root_unavailable`——那些一律經 [`GitEndpointError::Api`]）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GitApiError {
    /// 本模組自己驗證出的格式錯誤（版本語法、`ref`／`tip` 同時出現、`offset`／`limit` 超界、
    /// 兩側組合不合法等）→ 400 `bad_request`。
    BadRequest,
    /// 根目錄 `is_git` 為假 → 409 `not_git`。
    NotGit,
    /// hash 不是這個 repo 的 commit（[`VerifyCommit`] 失敗）→ 404 `rev_unknown`。
    RevUnknown,
    /// 指定的 `ref` 不在目前 `refs` 清單中 → 404 `ref_unknown`。
    RefUnknown,
    /// 檔案在該版本不存在（`BlobSize`／`Blob` 的 `cat-file` 非零結束）→ 404 `not_found_in_rev`。
    NotFoundInRev,
    /// 兩個 commit 沒有共同祖先（`merge-base` exit 1）→ 409 `no_merge_base`。
    NoMergeBase,
    /// 對未合併（衝突中）的檔案請求 `INDEX→WORKTREE` diff（`FileDiff::parse` 回
    /// [`GitParseError::Unmerged`]：patch 是 `diff --cc`／`diff --combined` 三方格式）→ 409
    /// `unmerged_path`（目視驗收缺陷 V1、Ruling R11）。
    UnmergedPath,
    /// 查詢輸出超過上限且不可截斷 → 413 `too_large`。
    TooLarge,
    /// 找不到 `git` 或 `wsl.exe` → 503 `git_unavailable`。
    GitUnavailable,
    /// git 拒絕擁有者不符的 repo → 502 `git_untrusted`。
    GitUntrusted,
    /// 逾時（含排隊）→ 504 `git_timeout`。
    GitTimeout,
    /// git 其他非零結束，或輸出格式與預期不符（[`GitParseError`]）→ 502 `git_failed`。
    GitFailed,
    /// 不應該發生的內部狀態（例如允許清單回傳的根目錄路徑選不出執行目標）→ 500 `io_error`。
    Internal,
}

impl GitApiError {
    /// HTTP 狀態碼。
    pub fn status(self) -> StatusCode {
        match self {
            GitApiError::BadRequest => StatusCode::BAD_REQUEST,
            GitApiError::NotGit | GitApiError::NoMergeBase | GitApiError::UnmergedPath => {
                StatusCode::CONFLICT
            }
            GitApiError::RevUnknown | GitApiError::RefUnknown | GitApiError::NotFoundInRev => {
                StatusCode::NOT_FOUND
            }
            GitApiError::TooLarge => StatusCode::PAYLOAD_TOO_LARGE,
            GitApiError::GitUnavailable => StatusCode::SERVICE_UNAVAILABLE,
            GitApiError::GitUntrusted | GitApiError::GitFailed => StatusCode::BAD_GATEWAY,
            GitApiError::GitTimeout => StatusCode::GATEWAY_TIMEOUT,
            GitApiError::Internal => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    /// spec 的錯誤代碼。
    pub fn code(self) -> &'static str {
        match self {
            GitApiError::BadRequest => "bad_request",
            GitApiError::NotGit => "not_git",
            GitApiError::RevUnknown => "rev_unknown",
            GitApiError::RefUnknown => "ref_unknown",
            GitApiError::NotFoundInRev => "not_found_in_rev",
            GitApiError::NoMergeBase => "no_merge_base",
            GitApiError::UnmergedPath => "unmerged_path",
            GitApiError::TooLarge => "too_large",
            GitApiError::GitUnavailable => "git_unavailable",
            GitApiError::GitUntrusted => "git_untrusted",
            GitApiError::GitTimeout => "git_timeout",
            GitApiError::GitFailed => "git_failed",
            GitApiError::Internal => "io_error",
        }
    }

    /// 固定的中文原因；**不含** git 的 stderr 原文與請求路徑片段原文（spec 明文要求）。
    fn reason(self) -> &'static str {
        match self {
            GitApiError::BadRequest => "請求格式不正確",
            GitApiError::NotGit => "這個根目錄不是 git repo",
            GitApiError::RevUnknown => "這個雜湊不是這個 repo 的 commit",
            GitApiError::RefUnknown => "指定的 ref 不在目前的 refs 清單中",
            GitApiError::NotFoundInRev => "這個版本沒有這個檔案",
            GitApiError::NoMergeBase => "這兩個 commit 沒有共同祖先",
            GitApiError::UnmergedPath => "這個檔案還在合併衝突中，暫存區沒有單一版本可比較",
            GitApiError::TooLarge => "輸出超過大小上限",
            GitApiError::GitUnavailable => "找不到可用的 git",
            GitApiError::GitUntrusted => "git 拒絕讀取這個 repo（擁有者不符）",
            GitApiError::GitTimeout => "執行逾時",
            GitApiError::GitFailed => "執行 git 時發生錯誤",
            GitApiError::Internal => "處理請求時發生內部錯誤",
        }
    }
}

impl IntoResponse for GitApiError {
    fn into_response(self) -> Response {
        coded_error_response(self.status(), self.code(), self.reason())
    }
}

/// git 端點處理過程中的錯誤：`crate::files` 的（[`FileApiError`]，來自 `authorize_root`）、本模組
/// 自己的（[`GitApiError`]），或 `cockpit_files` 的（[`FilesError`]——git-review task 3.3：`diff`
/// 的未追蹤檔案分支、`blob`／`meta`／`render` 皆重用 5a 的相對路徑界限與讀取，兩者的錯誤代碼與
/// 狀態碼直接沿用 [`crate::files::files_error_response`]，不重寫第二份對照表）。同
/// `crate::files::EndpointError` 的既有模式。
enum GitEndpointError {
    Api(FileApiError),
    Git(GitApiError),
    Files(FilesError),
}

impl From<FileApiError> for GitEndpointError {
    fn from(err: FileApiError) -> Self {
        GitEndpointError::Api(err)
    }
}

impl From<GitApiError> for GitEndpointError {
    fn from(err: GitApiError) -> Self {
        GitEndpointError::Git(err)
    }
}

impl From<FilesError> for GitEndpointError {
    fn from(err: FilesError) -> Self {
        GitEndpointError::Files(err)
    }
}

impl IntoResponse for GitEndpointError {
    fn into_response(self) -> Response {
        match self {
            GitEndpointError::Api(err) => err.into_response(),
            GitEndpointError::Git(err) => err.into_response(),
            GitEndpointError::Files(err) => crate::files::files_error_response(err),
        }
    }
}

// ---------------------------------------------------------------------------
// 共用：允許清單＋is_git＋執行目標、執行器呼叫、錯誤分類
// ---------------------------------------------------------------------------

/// [`RunnerError`] → [`GitApiError`]（design D6）：只涵蓋「不論哪個查詢都一樣」的分類；
/// `Failed` 在部分查詢（[`VerifyCommit`]、[`MergeBase`]）有更精確的解讀，由呼叫端另外處理。
fn classify_runner_error(err: &RunnerError) -> GitApiError {
    match err {
        RunnerError::Unavailable(_) => GitApiError::GitUnavailable,
        RunnerError::Untrusted { .. } => GitApiError::GitUntrusted,
        RunnerError::Timeout => GitApiError::GitTimeout,
        RunnerError::Failed { .. } => GitApiError::GitFailed,
        RunnerError::TooLarge => GitApiError::TooLarge,
    }
}

/// 允許清單＋`is_git`＋執行目標選擇（design D6 流程的中段）。呼叫端必須先完成所有參數格式驗證
/// 才呼叫這個函式——它會做 `find_root` 等檔案系統存取，但不啟動任何子程序；子程序只會在這個函式
/// 成功之後、呼叫端接著呼叫 [`run_query`] 時才會啟動。
async fn authorize_git_root(
    app: &AppState,
    runtime: &str,
    root_id: &str,
) -> Result<(Root, GitTarget), GitEndpointError> {
    let root = authorize_root(app, runtime, root_id).await?;
    if !root.is_git {
        return Err(GitApiError::NotGit.into());
    }
    let host_path = root.path.to_str().ok_or(GitApiError::Internal)?;
    let target = select_target(host_path).map_err(|_| GitApiError::Internal)?;
    Ok((root, target))
}

/// 執行一個查詢——正式流程下唯一啟動子程序的入口（[`GitRunner::run`] 的薄包裝，補上 design D6
/// 的錯誤分類）。只有「不論哪個查詢都一樣」的系統性錯誤會在這裡出現；查詢內個別呼叫的
/// `Failed`／`TooLarge` 留在 [`RunOutput::calls`] 裡，由呼叫端逐一檢視。
async fn run_query<Q: GitQuery>(
    runner: &GitRunner,
    query: &Q,
    target: &GitTarget,
) -> Result<RunOutput, GitApiError> {
    runner
        .run(query, target)
        .await
        .map_err(|err| classify_runner_error(&err))
}

/// [`GitParseError`] → [`GitApiError`]：git 的輸出格式與 design D4 預期不符，一律 502
/// `git_failed`（不是使用者輸入的問題，細節只記日誌，不進回應本體）。
fn map_parse<T>(result: Result<T, GitParseError>) -> Result<T, GitApiError> {
    result.map_err(|err| {
        tracing::warn!(error = %err, "git 輸出解析失敗");
        GitApiError::GitFailed
    })
}

/// [`FileDiff::parse`] 專用的錯誤對應：[`GitParseError::Unmerged`] 是 409 `unmerged_path`
/// （目視驗收缺陷 V1、Ruling R11），不是籠統的 502 `git_failed`；其餘變體沿用 [`map_parse`]
/// 的分類與日誌行為。
fn map_diff_parse(
    result: Result<FileDiffOutput, GitParseError>,
) -> Result<FileDiffOutput, GitApiError> {
    match result {
        Err(GitParseError::Unmerged) => Err(GitApiError::UnmergedPath),
        other => map_parse(other),
    }
}

/// 驗證一個 hash 是這個 repo 的 commit（design D4 `VerifyCommit`；spec 多處「hash 不是這個 repo
/// 的 commit → 404 rev_unknown」）。`-q` 讓「不是 commit」以單純非零結束、無 stderr 標記呈現，
/// 這裡把這個查詢的 `Failed` 解讀成 `rev_unknown`，其餘（`Unavailable`／`Untrusted`／`Timeout`／
/// `TooLarge`）沿用一般分類。
async fn verify_commit(
    runner: &GitRunner,
    target: &GitTarget,
    oid: &Oid,
) -> Result<(), GitApiError> {
    let run = run_query(runner, &VerifyCommit { oid: oid.clone() }, target).await?;
    match &run.calls[0] {
        Ok(_) => Ok(()),
        Err(RunnerError::Failed { .. }) => Err(GitApiError::RevUnknown),
        Err(other) => Err(classify_runner_error(other)),
    }
}

/// 執行 `merge-base` 並回傳共同祖先的 hash（design D4）：exit 1（無共同祖先）解讀成
/// [`GitApiError::NoMergeBase`]，其餘沿用一般分類。呼叫端必須先用 [`verify_commit`] 確認兩個 hash
/// 都是這個 repo 的 commit，才能把非零結束可靠地解讀成「沒有共同祖先」而不是「其中一個不存在」。
async fn merge_base_oid(
    runner: &GitRunner,
    target: &GitTarget,
    a: Oid,
    b: Oid,
) -> Result<String, GitApiError> {
    let run = run_query(runner, &MergeBase { a, b }, target).await?;
    match &run.calls[0] {
        Ok(outcome) => {
            let text = String::from_utf8_lossy(&outcome.stdout);
            let oid = text.trim();
            if oid.is_empty() {
                tracing::warn!("merge-base 成功結束卻沒有任何輸出");
                Err(GitApiError::GitFailed)
            } else {
                Ok(oid.to_string())
            }
        }
        Err(RunnerError::Failed { .. }) => Err(GitApiError::NoMergeBase),
        Err(other) => Err(classify_runner_error(other)),
    }
}

/// 取得目前的 refs（`status`／`log` 的預設起點與 `ref` 比對都要用到）。
async fn fetch_refs(runner: &GitRunner, target: &GitTarget) -> Result<RefsOutput, GitApiError> {
    let run = run_query(runner, &Refs, target).await?;
    map_parse(Refs.parse(&run.calls))
}

// ---------------------------------------------------------------------------
// query string 解析（design D6：一律從原始 URI 自行解析，同 5a 的坑）
// ---------------------------------------------------------------------------

/// 把 `?` 之後的原始 query string 拆成 `(key, value)` 對，逐一 percent-decode
/// （[`percent_decode_utf8`]，複用 file-review task 3.2 已驗證過的規則）。任何一段解不開
/// （`%` 後不是兩位十六進位、或解出來不是 UTF-8）一律 [`GitApiError::BadRequest`]——這是純函式，
/// 不碰檔案系統，讓格式驗證能排在 `authorize_root` 之前。
fn parse_query_pairs(query: &str) -> Result<Vec<(String, String)>, GitApiError> {
    let mut pairs = Vec::new();
    for segment in query.split('&') {
        if segment.is_empty() {
            continue;
        }
        let (k, v) = segment.split_once('=').unwrap_or((segment, ""));
        let key = percent_decode_utf8(k).ok_or(GitApiError::BadRequest)?;
        let value = percent_decode_utf8(v).ok_or(GitApiError::BadRequest)?;
        pairs.push((key, value));
    }
    Ok(pairs)
}

fn get_param<'a>(pairs: &'a [(String, String)], key: &str) -> Option<&'a str> {
    pairs
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.as_str())
}

fn get_all_params<'a>(pairs: &'a [(String, String)], key: &str) -> Vec<&'a str> {
    pairs
        .iter()
        .filter(|(k, _)| k == key)
        .map(|(_, v)| v.as_str())
        .collect()
}

fn parse_u32_param(
    pairs: &[(String, String)],
    key: &str,
    default: u32,
) -> Result<u32, GitApiError> {
    match get_param(pairs, key) {
        None => Ok(default),
        Some(raw) => raw.parse::<u32>().map_err(|_| GitApiError::BadRequest),
    }
}

/// spec「git 端點的共同規則」的「版本」格式：40／64 個小寫十六進位字元的 commit hash、`INDEX`、
/// `WORKTREE`、`EMPTY`；其他值（含 `HEAD`、分支名稱、`HEAD~1` 這類版本語法）一律 400
/// `bad_request`（design D5：使用者可控字串永遠不會以版本語法交給 git 解讀）。
fn parse_side(raw: &str) -> Result<Side, GitApiError> {
    match raw {
        "INDEX" => Ok(Side::Index),
        "WORKTREE" => Ok(Side::Worktree),
        "EMPTY" => Ok(Side::Empty),
        _ => Oid::parse(raw)
            .map(Side::Oid)
            .map_err(|_| GitApiError::BadRequest),
    }
}

/// git 的路徑一律以 `/` 分隔（即使 repo 在 Windows 上）；icon 只需要最後一段檔名。
fn basename(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

fn json_ok<T: Serialize>(body: &T) -> Response {
    let body =
        serde_json::to_string(body).expect("git 端點回應只含字串、數字與布林，序列化不會失敗");
    with_no_store_headers(
        (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "application/json")],
            body,
        )
            .into_response(),
    )
}

// ---------------------------------------------------------------------------
// GET /api/git/{runtime}/{root_id}/status（spec「狀態端點」）
// ---------------------------------------------------------------------------

pub(crate) async fn status(
    State(app): State<AppState>,
    path: Result<Path<(String, String)>, PathRejection>,
) -> Response {
    let Ok(Path((runtime, root_id))) = path else {
        return GitApiError::BadRequest.into_response();
    };
    match status_inner(app, &runtime, &root_id).await {
        Ok(response) => response,
        Err(err) => err.into_response(),
    }
}

async fn status_inner(
    app: AppState,
    runtime: &str,
    root_id: &str,
) -> Result<Response, GitEndpointError> {
    let (_root, target) = authorize_git_root(&app, runtime, root_id).await?;
    let run = run_query(&app.git_runner, &Status, &target).await?;
    let output = map_parse(Status.parse(&run.calls))?;
    Ok(status_response(&output, &app.files.icons))
}

#[derive(Serialize)]
struct StatusBranchBody {
    head: Option<String>,
    oid: Option<String>,
    upstream: Option<String>,
    ahead: Option<u32>,
    behind: Option<u32>,
}

#[derive(Serialize)]
struct StatusEntryBody {
    path: String,
    old_path: Option<String>,
    group: &'static str,
    status: String,
    icon: String,
}

#[derive(Serialize)]
struct StatusBody {
    branch: StatusBranchBody,
    entries: Vec<StatusEntryBody>,
    truncated: bool,
    skipped: u32,
}

fn status_group_str(group: StatusGroup) -> &'static str {
    match group {
        StatusGroup::Conflict => "conflict",
        StatusGroup::Staged => "staged",
        StatusGroup::Unstaged => "unstaged",
        StatusGroup::Untracked => "untracked",
    }
}

fn status_response(output: &StatusOutput, icons: &IconTheme) -> Response {
    let branch = StatusBranchBody {
        head: output.branch.head.clone(),
        oid: output.branch.oid.clone(),
        upstream: output.branch.upstream.clone(),
        ahead: output.branch.ahead,
        behind: output.branch.behind,
    };
    let entries = output
        .entries
        .iter()
        .map(|entry| StatusEntryBody {
            path: entry.path.clone(),
            old_path: entry.old_path.clone(),
            group: status_group_str(entry.group),
            status: entry.status.to_string(),
            icon: icons.file_icon(basename(&entry.path)).to_string(),
        })
        .collect();
    json_ok(&StatusBody {
        branch,
        entries,
        truncated: output.truncated,
        skipped: output.skipped,
    })
}

// ---------------------------------------------------------------------------
// GET /api/git/{runtime}/{root_id}/refs（spec「refs 端點」）
// ---------------------------------------------------------------------------

pub(crate) async fn refs(
    State(app): State<AppState>,
    path: Result<Path<(String, String)>, PathRejection>,
) -> Response {
    let Ok(Path((runtime, root_id))) = path else {
        return GitApiError::BadRequest.into_response();
    };
    match refs_inner(app, &runtime, &root_id).await {
        Ok(response) => response,
        Err(err) => err.into_response(),
    }
}

async fn refs_inner(
    app: AppState,
    runtime: &str,
    root_id: &str,
) -> Result<Response, GitEndpointError> {
    let (_root, target) = authorize_git_root(&app, runtime, root_id).await?;
    let output = fetch_refs(&app.git_runner, &target).await?;
    Ok(refs_response(output))
}

#[derive(Serialize)]
struct RefsHeadBody {
    oid: Option<String>,
    #[serde(rename = "ref")]
    git_ref: Option<String>,
}

#[derive(Serialize)]
struct RefEntryBody {
    name: String,
    short: String,
    kind: &'static str,
    oid: String,
    commit: bool,
}

#[derive(Serialize)]
struct RefsBody {
    head: RefsHeadBody,
    refs: Vec<RefEntryBody>,
}

fn ref_kind_str(kind: RefKind) -> &'static str {
    match kind {
        RefKind::Branch => "branch",
        RefKind::Remote => "remote",
        RefKind::Tag => "tag",
    }
}

fn refs_response(output: RefsOutput) -> Response {
    let head = RefsHeadBody {
        oid: output.head.oid,
        git_ref: output.head.git_ref,
    };
    let refs = output
        .refs
        .into_iter()
        .map(|entry| RefEntryBody {
            name: entry.name,
            short: entry.short,
            kind: ref_kind_str(entry.kind),
            oid: entry.oid,
            commit: entry.commit,
        })
        .collect();
    json_ok(&RefsBody { head, refs })
}

// ---------------------------------------------------------------------------
// GET /api/git/{runtime}/{root_id}/log（spec「commit 清單端點」；design D8）
// ---------------------------------------------------------------------------

/// spec：`limit` 預設 200、最大 200。
const LOG_DEFAULT_LIMIT: u32 = 200;
const LOG_MAX_LIMIT: u32 = 200;
/// spec／design：`offset + limit` 不得超過 5000；Graph 的整體上限也是 5000。
const LOG_MAX_TOTAL: u32 = 5000;

pub(crate) async fn log(
    State(app): State<AppState>,
    path: Result<Path<(String, String)>, PathRejection>,
    OriginalUri(uri): OriginalUri,
) -> Response {
    let Ok(Path((runtime, root_id))) = path else {
        return GitApiError::BadRequest.into_response();
    };
    match log_inner(app, &runtime, &root_id, &uri).await {
        Ok(response) => response,
        Err(err) => err.into_response(),
    }
}

async fn log_inner(
    app: AppState,
    runtime: &str,
    root_id: &str,
    uri: &Uri,
) -> Result<Response, GitEndpointError> {
    let pairs = parse_query_pairs(uri.query().unwrap_or(""))?;

    let ref_params = get_all_params(&pairs, "ref");
    let tip_params = get_all_params(&pairs, "tip");
    if !ref_params.is_empty() && !tip_params.is_empty() {
        return Err(GitApiError::BadRequest.into());
    }

    let offset = parse_u32_param(&pairs, "offset", 0)?;
    let limit = parse_u32_param(&pairs, "limit", LOG_DEFAULT_LIMIT)?;
    if limit > LOG_MAX_LIMIT {
        return Err(GitApiError::BadRequest.into());
    }
    let want = offset.checked_add(limit).ok_or(GitApiError::BadRequest)?;
    if want > LOG_MAX_TOTAL {
        return Err(GitApiError::BadRequest.into());
    }

    // 純格式驗證（不碰檔案系統）：tip 的 hash 格式在 authorize_root 之前就要驗完。
    let explicit_tips: Option<Vec<Oid>> = if tip_params.is_empty() {
        None
    } else {
        Some(
            tip_params
                .iter()
                .map(|raw| Oid::parse(raw).map_err(|_| GitApiError::BadRequest))
                .collect::<Result<_, _>>()?,
        )
    };
    let ref_names: Vec<String> = ref_params.iter().map(|s| (*s).to_string()).collect();

    let (_root, target) = authorize_git_root(&app, runtime, root_id).await?;

    let tips: Vec<Oid> = if let Some(tips) = explicit_tips {
        tips
    } else if !ref_names.is_empty() {
        let refs_output = fetch_refs(&app.git_runner, &target).await?;
        let mut tips = Vec::with_capacity(ref_names.len());
        for name in &ref_names {
            match refs_output.refs.iter().find(|r| &r.name == name) {
                Some(entry) => {
                    tips.push(Oid::parse(&entry.oid).map_err(|_| GitApiError::Internal)?)
                }
                None => return Err(GitApiError::RefUnknown.into()),
            }
        }
        tips
    } else {
        let refs_output = fetch_refs(&app.git_runner, &target).await?;
        let mut seen = HashSet::new();
        let mut tips = Vec::new();
        for entry in &refs_output.refs {
            // ui-fixes task 3.1：剝開後不是 commit 的 ref（例如指向 tree 的 tag）不當起點；
            // 它仍在 refs 清單中。排除不是為了避免 `git log` 失敗（git 對 tree／blob 起點直接
            // 忽略），而是讓前端分支變更偵測（`computeExpectedTips`）與後端起點規則一致，
            // 見 design D4。
            if !entry.commit {
                continue;
            }
            if seen.insert(entry.oid.clone()) {
                tips.push(Oid::parse(&entry.oid).map_err(|_| GitApiError::Internal)?);
            }
        }
        if let Some(head_oid) = &refs_output.head.oid
            && seen.insert(head_oid.clone())
        {
            tips.push(Oid::parse(head_oid).map_err(|_| GitApiError::Internal)?);
        }
        tips
    };

    // 沒有任何起點（空 repo、也沒有任何 ref）：不必啟動任何子程序，直接回空清單。
    if tips.is_empty() {
        return Ok(log_response(&[], &[], &[], false));
    }

    // design D8：以 `offset+limit` 筆重算整份排版；多取 1 筆（未達 5000 上限時）判斷 `has_more`。
    let query_limit = if want < LOG_MAX_TOTAL { want + 1 } else { want };
    let log_query = Log {
        tips: tips.clone(),
        limit: query_limit,
    };
    let run = run_query(&app.git_runner, &log_query, &target).await?;
    let output = match log_query.parse(&run.calls) {
        Ok(output) => output,
        Err(parse_err) => {
            // fix round 1（控制端裁決）：正常路徑不逐一驗證 tips（避免每批都多跑 N 次
            // git）；只有 `Log` 這次查詢本身失敗時，才對每個 tip 補跑 `VerifyCommit`，藉此
            // 把「tip 不是這個 repo 的 commit」從籠統的 `git_failed` 細分成 spec 共同規則要求
            // 的 404 `rev_unknown`；全部 tip 都存在時维持原本的 `git_failed` 分類。
            tracing::warn!(error = %parse_err, "git 輸出解析失敗");
            for tip in &tips {
                verify_commit(&app.git_runner, &target, tip).await?;
            }
            return Err(GitApiError::GitFailed.into());
        }
    };

    let commits: Vec<GraphCommit<'_>> = output
        .rows
        .iter()
        .map(|row| GraphCommit {
            oid: &row.oid,
            parents: &row.parents,
        })
        .collect();
    let graph_rows = layout(&commits);

    let total = output.rows.len() as u32;
    let has_more = if want < LOG_MAX_TOTAL {
        total > want
    } else {
        false
    };
    let end = want.min(total) as usize;
    let start = (offset as usize).min(end);

    let tip_strings: Vec<String> = tips.iter().map(|oid| oid.as_str().to_string()).collect();
    Ok(log_response(
        &tip_strings,
        &output.rows[start..end],
        &graph_rows[start..end],
        has_more,
    ))
}

#[derive(Serialize)]
struct GraphLineBody {
    from: usize,
    to: usize,
    half: &'static str,
    color: u8,
}

#[derive(Serialize)]
struct GraphBody {
    col: usize,
    color: u8,
    lines: Vec<GraphLineBody>,
}

#[derive(Serialize)]
struct LogRowBody {
    oid: String,
    parents: Vec<String>,
    author: String,
    email: String,
    time: i64,
    subject: String,
    graph: GraphBody,
}

#[derive(Serialize)]
struct LogBody {
    tips: Vec<String>,
    rows: Vec<LogRowBody>,
    has_more: bool,
}

fn graph_half_str(half: GraphHalf) -> &'static str {
    match half {
        GraphHalf::Top => "top",
        GraphHalf::Bottom => "bottom",
    }
}

fn log_response(
    tips: &[String],
    rows: &[LogRow],
    graph_rows: &[GraphRow],
    has_more: bool,
) -> Response {
    let rows = rows
        .iter()
        .zip(graph_rows)
        .map(|(row, graph)| LogRowBody {
            oid: row.oid.clone(),
            parents: row.parents.clone(),
            author: row.author.clone(),
            email: row.email.clone(),
            time: row.time,
            subject: row.subject.clone(),
            graph: GraphBody {
                col: graph.col,
                color: graph.color,
                lines: graph
                    .lines
                    .iter()
                    .map(|line| GraphLineBody {
                        from: line.from,
                        to: line.to,
                        half: graph_half_str(line.half),
                        color: line.color,
                    })
                    .collect(),
            },
        })
        .collect();
    json_ok(&LogBody {
        tips: tips.to_vec(),
        rows,
        has_more,
    })
}

// ---------------------------------------------------------------------------
// GET /api/git/{runtime}/{root_id}/commit/{hash}（spec「commit 詳情端點」）
// ---------------------------------------------------------------------------

pub(crate) async fn commit(
    State(app): State<AppState>,
    path: Result<Path<(String, String, String)>, PathRejection>,
) -> Response {
    let Ok(Path((runtime, root_id, hash))) = path else {
        return GitApiError::BadRequest.into_response();
    };
    match commit_inner(app, &runtime, &root_id, &hash).await {
        Ok(response) => response,
        Err(err) => err.into_response(),
    }
}

async fn commit_inner(
    app: AppState,
    runtime: &str,
    root_id: &str,
    hash: &str,
) -> Result<Response, GitEndpointError> {
    let oid = Oid::parse(hash).map_err(|_| GitApiError::BadRequest)?;

    let (_root, target) = authorize_git_root(&app, runtime, root_id).await?;

    verify_commit(&app.git_runner, &target, &oid).await?;

    let info_query = CommitInfo { oid: oid.clone() };
    let run = run_query(&app.git_runner, &info_query, &target).await?;
    let info = map_parse(info_query.parse(&run.calls))?;

    // spec「commit 詳情端點」：`compared_to` 為第一個 parent 的 hash；沒有 parent（根 commit）
    // 時為 null，代表與空內容比較——merge commit 也一律與第一個 parent 比較。
    let compared_to = info.parents.first().cloned();
    let from_side = match &compared_to {
        Some(parent) => {
            let parent_oid = Oid::parse(parent).map_err(|_| GitApiError::Internal)?;
            Side::Oid(parent_oid)
        }
        None => Side::Empty,
    };
    // `Empty→Oid` 與 `Oid→Oid` 都是 design D5 允許的組合，這裡的建構不會失敗；`Internal` 只是
    // 防禦不應該發生的情況。
    let changed_query =
        ChangedFiles::new(from_side, Side::Oid(oid)).map_err(|_| GitApiError::Internal)?;
    let changed_run = run_query(&app.git_runner, &changed_query, &target).await?;
    let changed = map_parse(changed_query.parse(&changed_run.calls))?;

    Ok(commit_response(
        &info,
        compared_to,
        &changed,
        &app.files.icons,
    ))
}

#[derive(Serialize)]
struct CommitPersonBody {
    name: String,
    email: String,
    time: i64,
}

#[derive(Serialize)]
struct CommitBody {
    oid: String,
    parents: Vec<String>,
    author: CommitPersonBody,
    committer: CommitPersonBody,
    message: String,
    compared_to: Option<String>,
    files: Vec<ChangedFileBody>,
    truncated: bool,
    skipped: u32,
}

fn commit_response(
    info: &CommitInfoOutput,
    compared_to: Option<String>,
    changed: &ChangedFilesOutput,
    icons: &IconTheme,
) -> Response {
    json_ok(&CommitBody {
        oid: info.oid.clone(),
        parents: info.parents.clone(),
        author: CommitPersonBody {
            name: info.author.name.clone(),
            email: info.author.email.clone(),
            time: info.author.time,
        },
        committer: CommitPersonBody {
            name: info.committer.name.clone(),
            email: info.committer.email.clone(),
            time: info.committer.time,
        },
        message: info.message.clone(),
        compared_to,
        files: changed_file_bodies(&changed.files, icons),
        truncated: changed.truncated,
        skipped: changed.skipped,
    })
}

// ---------------------------------------------------------------------------
// GET /api/git/{runtime}/{root_id}/changes（spec「變更檔案清單端點」）
// ---------------------------------------------------------------------------

pub(crate) async fn changes(
    State(app): State<AppState>,
    path: Result<Path<(String, String)>, PathRejection>,
    OriginalUri(uri): OriginalUri,
) -> Response {
    let Ok(Path((runtime, root_id))) = path else {
        return GitApiError::BadRequest.into_response();
    };
    match changes_inner(app, &runtime, &root_id, &uri).await {
        Ok(response) => response,
        Err(err) => err.into_response(),
    }
}

async fn changes_inner(
    app: AppState,
    runtime: &str,
    root_id: &str,
    uri: &Uri,
) -> Result<Response, GitEndpointError> {
    let pairs = parse_query_pairs(uri.query().unwrap_or(""))?;
    let from_raw = get_param(&pairs, "from").ok_or(GitApiError::BadRequest)?;
    let to_raw = get_param(&pairs, "to").ok_or(GitApiError::BadRequest)?;
    let from = parse_side(from_raw)?;
    let to = parse_side(to_raw)?;
    // design D5 允許的組合就是 spec「變更檔案清單端點」允許的組合，一對一——不允許的組合在這裡
    // 就被拒絕，不必另外重寫一次規則。純函式，排在 authorize_root 之前。
    let query = ChangedFiles::new(from.clone(), to.clone()).map_err(|_| GitApiError::BadRequest)?;

    let (_root, target) = authorize_git_root(&app, runtime, root_id).await?;

    for side in [&from, &to] {
        if let Side::Oid(oid) = side {
            verify_commit(&app.git_runner, &target, oid).await?;
        }
    }

    let run = run_query(&app.git_runner, &query, &target).await?;
    let output = map_parse(query.parse(&run.calls))?;
    Ok(changed_files_response(
        &output.files,
        output.truncated,
        output.skipped,
        &app.files.icons,
    ))
}

#[derive(Serialize)]
struct ChangedFileBody {
    status: String,
    path: String,
    old_path: Option<String>,
    additions: Option<u64>,
    deletions: Option<u64>,
    icon: String,
}

fn changed_file_bodies(files: &[ChangedFile], icons: &IconTheme) -> Vec<ChangedFileBody> {
    files
        .iter()
        .map(|file| ChangedFileBody {
            status: file.status.to_string(),
            path: file.path.clone(),
            old_path: file.old_path.clone(),
            additions: file.additions,
            deletions: file.deletions,
            icon: icons.file_icon(basename(&file.path)).to_string(),
        })
        .collect()
}

#[derive(Serialize)]
struct ChangedFilesBody {
    files: Vec<ChangedFileBody>,
    truncated: bool,
    skipped: u32,
}

fn changed_files_response(
    files: &[ChangedFile],
    truncated: bool,
    skipped: u32,
    icons: &IconTheme,
) -> Response {
    json_ok(&ChangedFilesBody {
        files: changed_file_bodies(files, icons),
        truncated,
        skipped,
    })
}

// ---------------------------------------------------------------------------
// GET /api/git/{runtime}/{root_id}/merge-base（spec「共同祖先端點」）
// ---------------------------------------------------------------------------

pub(crate) async fn merge_base(
    State(app): State<AppState>,
    path: Result<Path<(String, String)>, PathRejection>,
    OriginalUri(uri): OriginalUri,
) -> Response {
    let Ok(Path((runtime, root_id))) = path else {
        return GitApiError::BadRequest.into_response();
    };
    match merge_base_inner(app, &runtime, &root_id, &uri).await {
        Ok(response) => response,
        Err(err) => err.into_response(),
    }
}

async fn merge_base_inner(
    app: AppState,
    runtime: &str,
    root_id: &str,
    uri: &Uri,
) -> Result<Response, GitEndpointError> {
    let pairs = parse_query_pairs(uri.query().unwrap_or(""))?;
    let a_raw = get_param(&pairs, "a").ok_or(GitApiError::BadRequest)?;
    let b_raw = get_param(&pairs, "b").ok_or(GitApiError::BadRequest)?;
    let a = Oid::parse(a_raw).map_err(|_| GitApiError::BadRequest)?;
    let b = Oid::parse(b_raw).map_err(|_| GitApiError::BadRequest)?;

    let (_root, target) = authorize_git_root(&app, runtime, root_id).await?;

    verify_commit(&app.git_runner, &target, &a).await?;
    verify_commit(&app.git_runner, &target, &b).await?;

    let oid = merge_base_oid(&app.git_runner, &target, a, b).await?;
    Ok(json_ok(&MergeBaseBody { oid }))
}

#[derive(Serialize)]
struct MergeBaseBody {
    oid: String,
}

// ---------------------------------------------------------------------------
// 共用：某版本內容（`meta`／`blob`／`render`，以及 `diff` 走 git 的分支都要用到）
// ---------------------------------------------------------------------------

/// design D3：`Blob` 先以 `BlobSize` 確認大小，超過上限直接 413，不啟動讀取。這個上限用於
/// `meta`／`blob`（spec「某版本的檔案內容端點」規則同 file-review「原始內容端點」：50 MiB）；
/// `render` 另有更嚴格的 2 MiB（[`GIT_RENDER_SIZE_LIMIT`]，同 file-review「Markdown 渲染端點」）。
const GIT_BLOB_SIZE_LIMIT: u64 = 50 * 1024 * 1024;
/// spec「Markdown 渲染端點」：超過 2 MiB 回 413（`render` 端點重用同樣的上限）。
const GIT_RENDER_SIZE_LIMIT: u64 = 2 * 1024 * 1024;
/// design D7：未追蹤檔案（`diff` 的 `EMPTY→WORKTREE`）的讀取上限。
const UNTRACKED_DIFF_LIMIT: u64 = 8 * 1024 * 1024;

/// `cat-file -s <rev>:<path>`（design D4「BlobSize」）：檔案在該版本不存在（非零結束）→
/// [`GitApiError::NotFoundInRev`]；成功時把 stdout 剖析成位元組數。
async fn blob_size(
    runner: &GitRunner,
    target: &GitTarget,
    rev: &Side,
    path: &RepoPath,
) -> Result<u64, GitApiError> {
    let query = BlobSize::new(rev.clone(), path.clone()).map_err(|_| GitApiError::Internal)?;
    let run = run_query(runner, &query, target).await?;
    match &run.calls[0] {
        Ok(outcome) => {
            let text = String::from_utf8_lossy(&outcome.stdout);
            text.trim().parse::<u64>().map_err(|_| {
                tracing::warn!(output = %text, "cat-file -s 輸出不是數字");
                GitApiError::GitFailed
            })
        }
        Err(RunnerError::Failed { .. }) => Err(GitApiError::NotFoundInRev),
        Err(other) => Err(classify_runner_error(other)),
    }
}

/// `cat-file blob <rev>:<path>`（design D4「Blob」）：檔案在該版本不存在（非零結束）→
/// [`GitApiError::NotFoundInRev`]；呼叫端必須先用 [`blob_size`] 確認大小在允許範圍內
/// （design D3：`Blob` 先以 `BlobSize` 確認大小，超過上限不啟動讀取）。
async fn fetch_blob(
    runner: &GitRunner,
    target: &GitTarget,
    rev: &Side,
    path: &RepoPath,
) -> Result<Vec<u8>, GitApiError> {
    let query = Blob::new(rev.clone(), path.clone()).map_err(|_| GitApiError::Internal)?;
    let run = run_query(runner, &query, target).await?;
    match &run.calls[0] {
        Ok(outcome) => Ok(outcome.stdout.clone()),
        Err(RunnerError::Failed { .. }) => Err(GitApiError::NotFoundInRev),
        Err(other) => Err(classify_runner_error(other)),
    }
}

/// 同 [`fetch_blob`]，但用 `BlobHead`（design D4「BlobHead」；Ruling R10，git-review task 3.3
/// fix round 2）：只取前 8192 位元組（截斷是正常結果，不是錯誤——`BlobHead` 標成
/// `truncatable`，執行器讀滿即終止子程序並回傳前段，`run.calls[0]` 仍是 `Ok`）。`meta` 端點用
/// 它取得 viewer 分類所需的內容片段，不必像 `blob` 端點那樣讀取整份內容。
async fn fetch_blob_head(
    runner: &GitRunner,
    target: &GitTarget,
    rev: &Side,
    path: &RepoPath,
) -> Result<Vec<u8>, GitApiError> {
    let query = BlobHead::new(rev.clone(), path.clone()).map_err(|_| GitApiError::Internal)?;
    let run = run_query(runner, &query, target).await?;
    match &run.calls[0] {
        Ok(outcome) => Ok(outcome.stdout.clone()),
        Err(RunnerError::Failed { .. }) => Err(GitApiError::NotFoundInRev),
        Err(other) => Err(classify_runner_error(other)),
    }
}

/// `rev-parse --verify -q --end-of-options <rev>:<path>`（design D4「BlobId」；Ruling R8，
/// git-review task 3.3 fix round 1）：檔案內容的物件 hash，**不讀取內容**。`meta` 端點用它取代
/// 先前「讀整份 `Blob` 內容算雜湊」的做法——暫存區版本的分頁每 2 秒輪詢 `meta`，先前的做法
/// 每次都要整份讀取最多 50 MiB 才能知道內容有沒有變，且算出來的值也不是 spec 要求的「物件
/// hash」。檔案在該版本不存在（非零結束）→ [`GitApiError::NotFoundInRev`]（同 [`blob_size`]）。
async fn blob_id(
    runner: &GitRunner,
    target: &GitTarget,
    rev: &Side,
    path: &RepoPath,
) -> Result<String, GitApiError> {
    let query = BlobId::new(rev.clone(), path.clone()).map_err(|_| GitApiError::Internal)?;
    let run = run_query(runner, &query, target).await?;
    match &run.calls[0] {
        Ok(_) => map_parse(BlobId::parse(&run.calls)),
        Err(RunnerError::Failed { .. }) => Err(GitApiError::NotFoundInRev),
        Err(other) => Err(classify_runner_error(other)),
    }
}

/// 一個 [`RelPath`]（5a 片段規則，已通過檢查）轉成 [`RepoPath`]（`cockpit-git` 自己重複的同一套
/// 片段規則，design D1：`cockpit-git` 不依賴 `cockpit-files`，縱深防禦第二關）。兩邊的
/// `check_segment` 規則逐條相同，`RelPath` 的每個片段已經不含 `/`，所以拼回 `/` 分隔字串再交給
/// `RepoPath::parse` 理論上不會失敗；萬一失敗（不應該發生）由呼叫端當成內部錯誤處理，不是使用者
/// 輸入的問題。
fn to_repo_path(rel: &RelPath) -> Result<RepoPath, cockpit_git::RepoPathError> {
    RepoPath::parse(&rel.segments().join("/"))
}

/// `rel` 的檔名（最後一段）；`rel` 由 [`parse_diff_rel_path`]／[`parse_rev_target`] 保證非空。
fn rel_basename(rel: &RelPath) -> &str {
    rel.segments().last().map_or("", String::as_str)
}

// ---------------------------------------------------------------------------
// GET /api/git/{runtime}/{root_id}/diff（spec「單檔 diff 端點」；design D7）
// ---------------------------------------------------------------------------

pub(crate) async fn diff(
    State(app): State<AppState>,
    path: Result<Path<(String, String)>, PathRejection>,
    OriginalUri(uri): OriginalUri,
) -> Response {
    let Ok(Path((runtime, root_id))) = path else {
        return GitApiError::BadRequest.into_response();
    };
    match diff_inner(app, &runtime, &root_id, &uri).await {
        Ok(response) => response,
        Err(err) => err.into_response(),
    }
}

/// `path`／`old_path` 從原始 query string 取「仍為 percent-encoded」的原始值（不經
/// [`parse_query_pairs`] 的通用解碼）：那套解碼會先把整個值解開（含 `%2F` → `/`）才看得到
/// 片段，等於讓使用者能用 `%2F` 偽裝出片段分隔字元、繞過 [`RelPath::parse`] 的「片段含 `/`」
/// 規則；這裡改用不解碼的原始切法取值，把「解碼」與「逐段檢查」兩件事一次交給
/// `RelPath::parse` 做（同 5a 的坑：一次 percent-decode，不能先解碼整段字串）。
fn get_raw_query_param<'a>(query: &'a str, key: &str) -> Option<&'a str> {
    query.split('&').find_map(|segment| {
        let (k, v) = segment.split_once('=').unwrap_or((segment, ""));
        (k == key).then_some(v)
    })
}

/// `path`／`old_path` 共用：解析＋非空檢查（diff 的路徑一定指向一個檔案，不能是根目錄本身）。
fn parse_diff_rel_path(raw: &str) -> Result<RelPath, GitEndpointError> {
    let rel = RelPath::parse(raw)?;
    if rel.segments().is_empty() {
        return Err(GitApiError::BadRequest.into());
    }
    Ok(rel)
}

async fn diff_inner(
    app: AppState,
    runtime: &str,
    root_id: &str,
    uri: &Uri,
) -> Result<Response, GitEndpointError> {
    let query_str = uri.query().unwrap_or("");
    let pairs = parse_query_pairs(query_str)?;
    let from_raw = get_param(&pairs, "from").ok_or(GitApiError::BadRequest)?;
    let to_raw = get_param(&pairs, "to").ok_or(GitApiError::BadRequest)?;
    let from = parse_side(from_raw)?;
    let to = parse_side(to_raw)?;

    let path_raw = get_raw_query_param(query_str, "path").ok_or(GitApiError::BadRequest)?;
    let rel = parse_diff_rel_path(path_raw)?;
    let old_rel = get_raw_query_param(query_str, "old_path")
        .map(parse_diff_rel_path)
        .transpose()?;

    // design D7：`EMPTY→WORKTREE`（未追蹤檔案）不經 git，直接讀檔——這個組合沒有「改名」的
    // 語意（未追蹤檔案不會有 old_path），送了就視為請求格式錯誤。
    if matches!((&from, &to), (Side::Empty, Side::Worktree)) {
        if old_rel.is_some() {
            return Err(GitApiError::BadRequest.into());
        }
        let (root, _target) = authorize_git_root(&app, runtime, root_id).await?;
        let resolved = resolve(&root.path, &rel)?;
        let bytes = read_capped(&resolved, UNTRACKED_DIFF_LIMIT)?;
        let output = untracked_file_diff(&bytes).map_err(|_| GitApiError::TooLarge)?;
        return Ok(diff_response(&output));
    }

    // 其餘組合：純函式建構查詢（design D5 允許的組合），在 authorize_root 之前就能判斷組合
    // 是否合法，不啟動任何子程序（同 `changes_inner` 的既有模式）。
    let repo_path = to_repo_path(&rel).map_err(|_| GitApiError::Internal)?;
    let old_repo_path = old_rel
        .as_ref()
        .map(to_repo_path)
        .transpose()
        .map_err(|_| GitApiError::Internal)?;
    let file_diff_query = FileDiff::new(from.clone(), to.clone(), repo_path, old_repo_path)
        .map_err(|_| GitApiError::BadRequest)?;

    let (_root, target) = authorize_git_root(&app, runtime, root_id).await?;
    for side in [&from, &to] {
        if let Side::Oid(oid) = side {
            verify_commit(&app.git_runner, &target, oid).await?;
        }
    }
    let run = run_query(&app.git_runner, &file_diff_query, &target).await?;
    let output = map_diff_parse(file_diff_query.parse(&run.calls))?;
    Ok(diff_response(&output))
}

#[derive(Serialize)]
struct DiffLineBody {
    line: u32,
    text: String,
}

fn diff_line_body(line: &cockpit_git::DiffLine) -> DiffLineBody {
    DiffLineBody {
        line: line.line,
        text: line.text.clone(),
    }
}

/// spec「單檔 diff 端點」`rows` 的一列：內部標籤式 enum（`#[serde(tag = "kind")]`）讓每個變體
/// 序列化成扁平的 `{"kind": "...", ...該變體自己的欄位}`，逐字對應 spec 的形狀。
#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum DiffRowBody {
    Context {
        left: DiffLineBody,
        right: DiffLineBody,
    },
    Delete {
        left: DiffLineBody,
    },
    Add {
        right: DiffLineBody,
    },
    Change {
        left: DiffLineBody,
        right: DiffLineBody,
    },
    Gap {
        lines: u32,
    },
}

fn diff_row_body(row: &cockpit_git::DiffRow) -> DiffRowBody {
    match row {
        cockpit_git::DiffRow::Context { left, right } => DiffRowBody::Context {
            left: diff_line_body(left),
            right: diff_line_body(right),
        },
        cockpit_git::DiffRow::Delete { left } => DiffRowBody::Delete {
            left: diff_line_body(left),
        },
        cockpit_git::DiffRow::Add { right } => DiffRowBody::Add {
            right: diff_line_body(right),
        },
        cockpit_git::DiffRow::Change { left, right } => DiffRowBody::Change {
            left: diff_line_body(left),
            right: diff_line_body(right),
        },
        cockpit_git::DiffRow::Gap { lines } => DiffRowBody::Gap { lines: *lines },
    }
}

#[derive(Serialize)]
struct DiffBody {
    version: String,
    binary: bool,
    mode_only: bool,
    submodule: bool,
    rows: Vec<DiffRowBody>,
}

fn diff_response(output: &FileDiffOutput) -> Response {
    json_ok(&DiffBody {
        version: output.version.clone(),
        binary: output.binary,
        mode_only: output.mode_only,
        submodule: output.submodule,
        rows: output.rows.iter().map(diff_row_body).collect(),
    })
}

// ---------------------------------------------------------------------------
// GET /api/git/{runtime}/{root_id}/meta|blob|render/{rev}/{相對路徑}
// （spec「某版本的檔案內容端點」；design D6：重用 5a 的回應組裝、viewer 分類、Markdown 渲染）
// ---------------------------------------------------------------------------

/// 三個端點共用的路由段字面值與流程分派。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RevFileOp {
    Meta,
    Blob,
    Render,
}

impl RevFileOp {
    fn segment(self) -> &'static str {
        match self {
            RevFileOp::Meta => "meta",
            RevFileOp::Blob => "blob",
            RevFileOp::Render => "render",
        }
    }
}

/// `<rev>`：只接受 commit hash 或 `INDEX`（spec「某版本的檔案內容端點」明文；不像 `diff` 的
/// 「版本」還允許 `WORKTREE`／`EMPTY`——這三個端點不讀取工作區）。
fn parse_rev(raw: &str) -> Result<Side, GitApiError> {
    match raw {
        "INDEX" => Ok(Side::Index),
        _ => Oid::parse(raw)
            .map(Side::Oid)
            .map_err(|_| GitApiError::BadRequest),
    }
}

/// 從**原始**請求路徑（仍為 percent-encoded）切出 `(runtime, root_id, rev 原始字面, 相對路徑)`
/// （同 `crate::files::parse_target` 的理由：不用 axum 解碼過的 `{*path}`，`%2F` 才不會被誤判成
/// 路徑分隔符）。
fn parse_rev_target(
    path: &str,
    op: RevFileOp,
) -> Result<(String, &str, &str, RelPath), GitEndpointError> {
    let rest = path
        .strip_prefix("/api/git/")
        .ok_or(GitApiError::BadRequest)?;
    let mut parts = rest.splitn(5, '/');
    let runtime_raw = parts.next().ok_or(GitApiError::BadRequest)?;
    let root_id = parts.next().ok_or(GitApiError::BadRequest)?;
    if parts.next() != Some(op.segment()) {
        return Err(GitApiError::BadRequest.into());
    }
    let rev_raw = parts.next().ok_or(GitApiError::BadRequest)?;
    let rel_raw = match parts.next() {
        None | Some("") => return Err(GitApiError::BadRequest.into()),
        Some(raw) => raw,
    };
    let runtime = percent_decode_utf8(runtime_raw).ok_or(GitApiError::BadRequest)?;
    let rel = parse_diff_rel_path(rel_raw)?;
    Ok((runtime, root_id, rev_raw, rel))
}

pub(crate) async fn meta(State(app): State<AppState>, OriginalUri(uri): OriginalUri) -> Response {
    match rev_endpoint_inner(app, &uri, RevFileOp::Meta).await {
        Ok(response) => response,
        Err(err) => err.into_response(),
    }
}

pub(crate) async fn blob(State(app): State<AppState>, OriginalUri(uri): OriginalUri) -> Response {
    match rev_endpoint_inner(app, &uri, RevFileOp::Blob).await {
        Ok(response) => response,
        Err(err) => err.into_response(),
    }
}

pub(crate) async fn render(State(app): State<AppState>, OriginalUri(uri): OriginalUri) -> Response {
    match rev_endpoint_inner(app, &uri, RevFileOp::Render).await {
        Ok(response) => response,
        Err(err) => err.into_response(),
    }
}

async fn rev_endpoint_inner(
    app: AppState,
    uri: &Uri,
    op: RevFileOp,
) -> Result<Response, GitEndpointError> {
    let (runtime, root_id, rev_raw, rel) = parse_rev_target(uri.path(), op)?;
    let rev = parse_rev(rev_raw)?;

    let (_root, target) = authorize_git_root(&app, &runtime, root_id).await?;
    if let Side::Oid(oid) = &rev {
        verify_commit(&app.git_runner, &target, oid).await?;
    }

    let repo_path = to_repo_path(&rel).map_err(|_| GitApiError::Internal)?;
    let name = rel_basename(&rel).to_string();

    match op {
        RevFileOp::Meta => meta_endpoint(&app, &target, &rev, &repo_path, &name).await,
        RevFileOp::Blob => blob_endpoint(&app, &target, &rev, &repo_path, &name).await,
        RevFileOp::Render => render_endpoint(&app, &target, &rev, &repo_path, &name).await,
    }
}

/// `meta`（Ruling R8，git-review task 3.3 fix round 1）：`BlobSize` 取得 `size`（同時是存在性
/// 檢查，`not_found_in_rev`）＋`BlobId` 取得物件 hash（`blob` 欄位）——**不讀取內容**，不像
/// `blob`／`render` 需要真正的位元組才能運作。沒有 50 MiB 上限：這個上限原本是「讀取內容」的
/// 保護，`meta` 不再讀取內容就不需要它。
async fn meta_endpoint(
    app: &AppState,
    target: &GitTarget,
    rev: &Side,
    repo_path: &RepoPath,
    name: &str,
) -> Result<Response, GitEndpointError> {
    let size = blob_size(&app.git_runner, target, rev, repo_path).await?;
    let blob = blob_id(&app.git_runner, target, rev, repo_path).await?;
    let head = fetch_blob_head(&app.git_runner, target, rev, repo_path).await?;
    Ok(meta_rev_response(name, size, blob, &head, &app.files.icons))
}

/// `blob`：存在性＋50 MiB 上限檢查（design D3）後才真正讀取內容。
async fn blob_endpoint(
    app: &AppState,
    target: &GitTarget,
    rev: &Side,
    repo_path: &RepoPath,
    name: &str,
) -> Result<Response, GitEndpointError> {
    let size = blob_size(&app.git_runner, target, rev, repo_path).await?;
    if size > GIT_BLOB_SIZE_LIMIT {
        return Err(GitApiError::TooLarge.into());
    }
    let bytes = fetch_blob(&app.git_runner, target, rev, repo_path).await?;
    Ok(blob_rev_response(bytes, name))
}

/// `render`：存在性檢查（`blob_size`）→ 副檔名必須是 markdown（`not_markdown`）→ 2 MiB 上限
/// （同 file-review「Markdown 渲染端點」，比 `blob` 的 50 MiB 更嚴格）→ 讀取內容。順序同 5a
/// 的 `render_markdown`（`file_meta` 先確認存在，再看 viewer，再看大小）。
async fn render_endpoint(
    app: &AppState,
    target: &GitTarget,
    rev: &Side,
    repo_path: &RepoPath,
    name: &str,
) -> Result<Response, GitEndpointError> {
    let size = blob_size(&app.git_runner, target, rev, repo_path).await?;
    // 只是「副檔名看起來是不是 markdown」的早退判斷（避免對明顯不是 markdown 的檔案多跑一次
    // git 呼叫），不是完整的 viewer 分類——完整規則（含內容判斷的退回邏輯）只在
    // `cockpit_files::classify_viewer_bytes` 定義一次，`meta` 端點呼叫它；這裡的副檔名比對
    // 與它的副檔名優先順序一致（`.md`／`.markdown` 才可能是 markdown，其餘分支不影響這個判斷）。
    let is_markdown_ext = matches!(
        std::path::Path::new(name)
            .extension()
            .and_then(|ext| ext.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some("md" | "markdown")
    );
    if !is_markdown_ext {
        return Err(FilesError::NotMarkdown.into());
    }
    if size > GIT_RENDER_SIZE_LIMIT {
        return Err(GitApiError::TooLarge.into());
    }
    let bytes = fetch_blob(&app.git_runner, target, rev, repo_path).await?;
    Ok(render_rev_response(&bytes))
}

#[derive(Serialize)]
struct MetaRevBody<'a> {
    size: u64,
    viewer: Viewer,
    icon: &'a str,
    blob: String,
}

fn meta_rev_response(
    name: &str,
    size: u64,
    blob: String,
    head: &[u8],
    icons: &IconTheme,
) -> Response {
    json_ok(&MetaRevBody {
        size,
        viewer: classify_viewer_bytes(name, head),
        icon: icons.file_icon(name),
        blob,
    })
}

/// 加上 `Content-Security-Policy: sandbox`（spec「某版本的檔案內容端點」：`blob` 的規則同
/// file-review「原始內容端點」；`render` 的規則同「Markdown 渲染端點」，兩者都要求這個標頭）。
fn with_sandbox_csp(mut response: Response) -> Response {
    response.headers_mut().insert(
        header::CONTENT_SECURITY_POLICY,
        axum::http::HeaderValue::from_static("sandbox"),
    );
    response
}

fn blob_rev_response(bytes: Vec<u8>, name: &str) -> Response {
    let viewer = classify_viewer_bytes(name, &bytes);
    let content_type = raw_content_type(std::path::Path::new(name), viewer, &bytes);
    with_sandbox_csp(with_no_store_headers(
        (
            StatusCode::OK,
            [(header::CONTENT_TYPE, content_type)],
            bytes,
        )
            .into_response(),
    ))
}

fn render_rev_response(bytes: &[u8]) -> Response {
    let src = String::from_utf8_lossy(bytes);
    let html = cockpit_files::render_markdown_str(&src);
    with_sandbox_csp(with_no_store_headers(
        (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
            html,
        )
            .into_response(),
    ))
}

// ---------------------------------------------------------------------------
// 單元測試（純函式；不需要子程序）
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::{
        GitApiError, RunnerError, Side, basename, classify_runner_error, graph_half_str,
        parse_query_pairs, parse_side, ref_kind_str, status_group_str,
    };
    use cockpit_git::{GraphHalf, Oid, RefKind, StatusGroup};

    /// design D6：`RunnerError` → [`GitApiError`] 的分類表。
    #[test]
    fn classify_runner_error_matches_design_d6_table() {
        assert_eq!(
            classify_runner_error(&RunnerError::Unavailable(std::io::Error::other(
                "找不到 git"
            ))),
            GitApiError::GitUnavailable
        );
        assert_eq!(
            classify_runner_error(&RunnerError::Untrusted {
                stderr_tail: Vec::new()
            }),
            GitApiError::GitUntrusted,
            "擁有者不符不繞過：RunnerError::Untrusted 必須分類成 git_untrusted，不得改分類成其他代碼"
        );
        assert_eq!(
            classify_runner_error(&RunnerError::Timeout),
            GitApiError::GitTimeout
        );
        assert_eq!(
            classify_runner_error(&RunnerError::Failed {
                exit_code: Some(1),
                stderr_tail: Vec::new()
            }),
            GitApiError::GitFailed
        );
        assert_eq!(
            classify_runner_error(&RunnerError::TooLarge),
            GitApiError::TooLarge
        );
    }

    /// spec「git 端點的共同規則」的代碼與狀態碼對應表。
    #[test]
    fn git_api_error_status_and_code_match_spec() {
        let cases = [
            (GitApiError::BadRequest, 400, "bad_request"),
            (GitApiError::NotGit, 409, "not_git"),
            (GitApiError::RevUnknown, 404, "rev_unknown"),
            (GitApiError::RefUnknown, 404, "ref_unknown"),
            (GitApiError::NoMergeBase, 409, "no_merge_base"),
            (GitApiError::UnmergedPath, 409, "unmerged_path"),
            (GitApiError::TooLarge, 413, "too_large"),
            (GitApiError::GitUnavailable, 503, "git_unavailable"),
            (GitApiError::GitUntrusted, 502, "git_untrusted"),
            (GitApiError::GitTimeout, 504, "git_timeout"),
            (GitApiError::GitFailed, 502, "git_failed"),
            (GitApiError::Internal, 500, "io_error"),
        ];
        for (err, status, code) in cases {
            assert_eq!(err.status().as_u16(), status, "{err:?}");
            assert_eq!(err.code(), code, "{err:?}");
        }
    }

    /// spec「git 端點的共同規則」的「版本」格式：合法字面值與 hash 通過，其餘（含 Scenario
    /// 「版本語法被拒」列出的每個值）一律 400。
    #[test]
    fn parse_side_accepts_literals_and_hashes_rejects_everything_else() {
        assert_eq!(parse_side("INDEX"), Ok(Side::Index));
        assert_eq!(parse_side("WORKTREE"), Ok(Side::Worktree));
        assert_eq!(parse_side("EMPTY"), Ok(Side::Empty));
        let hash40 = "a".repeat(40);
        assert_eq!(
            parse_side(&hash40),
            Ok(Side::Oid(Oid::parse(&hash40).expect("測試 hash 應合法")))
        );
        let hash64 = "b".repeat(64);
        assert_eq!(
            parse_side(&hash64),
            Ok(Side::Oid(Oid::parse(&hash64).expect("測試 hash 應合法")))
        );

        let upper40 = "A".repeat(40);
        let short = "a".repeat(39);
        for bad in [
            "HEAD",
            "main",
            "HEAD~1",
            "-p",
            ":/fix",
            upper40.as_str(),
            short.as_str(),
        ] {
            assert_eq!(parse_side(bad), Err(GitApiError::BadRequest), "{bad:?}");
        }
    }

    #[test]
    fn parse_query_pairs_decodes_and_preserves_repeated_keys() {
        let pairs =
            parse_query_pairs("ref=refs%2Fheads%2Fmain&ref=refs/heads/feat&offset=10").unwrap();
        assert_eq!(
            pairs,
            vec![
                ("ref".to_string(), "refs/heads/main".to_string()),
                ("ref".to_string(), "refs/heads/feat".to_string()),
                ("offset".to_string(), "10".to_string()),
            ]
        );
    }

    #[test]
    fn parse_query_pairs_empty_string_is_no_pairs() {
        assert_eq!(parse_query_pairs("").unwrap(), Vec::new());
    }

    #[test]
    fn parse_query_pairs_rejects_bad_percent_escape() {
        assert_eq!(parse_query_pairs("from=%zz"), Err(GitApiError::BadRequest));
    }

    #[test]
    fn basename_takes_last_forward_slash_segment_even_on_windows_repos() {
        assert_eq!(basename("a/b/c.rs"), "c.rs");
        assert_eq!(basename("c.rs"), "c.rs");
    }

    #[test]
    fn display_string_helpers_match_spec_vocabulary() {
        assert_eq!(status_group_str(StatusGroup::Conflict), "conflict");
        assert_eq!(status_group_str(StatusGroup::Staged), "staged");
        assert_eq!(status_group_str(StatusGroup::Unstaged), "unstaged");
        assert_eq!(status_group_str(StatusGroup::Untracked), "untracked");
        assert_eq!(ref_kind_str(RefKind::Branch), "branch");
        assert_eq!(ref_kind_str(RefKind::Remote), "remote");
        assert_eq!(ref_kind_str(RefKind::Tag), "tag");
        assert_eq!(graph_half_str(GraphHalf::Top), "top");
        assert_eq!(graph_half_str(GraphHalf::Bottom), "bottom");
    }
}
