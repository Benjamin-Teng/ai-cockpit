//! 檔案根目錄與允許清單（file-review task 3.1；spec `file-review`「檔案根目錄與允許清單」「檔案端點的
//! 共同規則」「根目錄查詢端點」；design D1、D2、D10）。
//!
//! `cockpit-files` 只知道「給一個主機路徑，往上找 `.git`」（[`cockpit_files::find_root`]）；這裡負責它
//! 不知道的部分：pane → `cwd` → 主機路徑（WSL 轉換，[`wsl_host_path`]）、`root_id` 編解碼
//! （[`encode_root_id`]／[`decode_root_id`]）、允許清單（[`authorize_root`]），以及
//! `GET /api/runtimes/{runtime}/panes/{pane}/root` 的處理常式（路由註冊在 [`crate::http::router`]）。
//!
//! file-review task 3.2 再加上四組檔案端點（列目錄、中繼資料、Markdown 渲染、原始內容）的處理常式、
//! `cockpit_files::FilesError` 的狀態碼對應（[`files_error_response`]）、「在 VS Code 開啟」的網址
//! （[`vscode_uri`]），以及啟動時決定的 [`FileSettings`]（icon 對照表、原始內容上限）。相對路徑一律從
//! 原始請求 URI 切出（`parse_target`），不用 axum 已解碼的 `{*path}`。
//!
//! **允許清單是檔案端點唯一的授權來源**（design D2）：`root_id` 只是根目錄主機路徑的十六進位編碼，
//! 不是秘密也不是憑證——任何人都能自己編一個。[`authorize_root`] 在每次請求當下讀最新投影，只接受
//! 「該 runtime 某個 pane（含 exited）目前推算得出」的根目錄；不快取，pane 一消失同一個 `root_id`
//! 立刻變成 `root_unavailable`。
//!
//! 所有會碰檔案系統的步驟（`find_root` 逐層 `metadata`；WSL 是 `\\wsl.localhost` 的 9P，可能很慢）
//! 都包在 [`tokio::task::spawn_blocking`] 內，不卡 async 執行緒。

use std::collections::HashSet;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use axum::extract::rejection::PathRejection;
use axum::extract::{OriginalUri, Path as UrlPath, State};
use axum::http::{HeaderValue, StatusCode, Uri, header};
use axum::response::{IntoResponse, Response};
use cockpit_core::ProjectedState;
use cockpit_files::{
    FilesError, IconTheme, Kind, RelPath, Root, Viewer, file_meta, find_root, list_dir,
    read_capped, render_markdown, resolve,
};
use cockpit_herdr::HerdrEndpoint;
use serde::Serialize;

use crate::http::{AppState, coded_error_response, with_no_store_headers};

// ---------------------------------------------------------------------------
// 錯誤（spec「檔案端點的共同規則」的代碼表中，本 task 用到的部分）
// ---------------------------------------------------------------------------

/// 檔案端點在 `cockpit` 這層產生的錯誤；每個變體對應 spec 的一個 `code`。
///
/// 本體一律 `{"error": "<中文原因>", "code": "<代碼>"}`（[`IntoResponse`]），原因是固定字串，
/// **不含請求路徑的片段原文**。file-review task 3.2 的檔案端點遇到 `cockpit_files::FilesError`
/// 時另行對應（那些代碼不在這裡）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileApiError {
    /// 路徑片段不是合法 UTF-8、`root_id` 無法解碼 → 400 `bad_request`。
    BadRequest,
    /// runtime 不是設定中的 id → 404 `runtime_unknown`。
    RuntimeUnknown,
    /// pane 不在最新投影中 → 404 `pane_unknown`。
    PaneUnknown,
    /// pane 沒有根目錄（`cwd` 為 null、轉換不出主機路徑、或路徑不存在）→ 404 `no_root`。
    NoRoot,
    /// 根目錄不在允許清單 → 404 `root_unavailable`。
    RootUnavailable,
    /// method 不是 `GET` → 405 `method_not_allowed`。
    MethodNotAllowed,
    /// 檔案系統工作的 blocking task 本身失敗（panic／被取消）→ 500 `io_error`。
    Internal,
}

impl FileApiError {
    /// HTTP 狀態碼。
    pub fn status(self) -> StatusCode {
        match self {
            FileApiError::BadRequest => StatusCode::BAD_REQUEST,
            FileApiError::RuntimeUnknown
            | FileApiError::PaneUnknown
            | FileApiError::NoRoot
            | FileApiError::RootUnavailable => StatusCode::NOT_FOUND,
            FileApiError::MethodNotAllowed => StatusCode::METHOD_NOT_ALLOWED,
            FileApiError::Internal => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    /// spec 的錯誤代碼。
    pub fn code(self) -> &'static str {
        match self {
            FileApiError::BadRequest => "bad_request",
            FileApiError::RuntimeUnknown => "runtime_unknown",
            FileApiError::PaneUnknown => "pane_unknown",
            FileApiError::NoRoot => "no_root",
            FileApiError::RootUnavailable => "root_unavailable",
            FileApiError::MethodNotAllowed => "method_not_allowed",
            FileApiError::Internal => "io_error",
        }
    }

    /// 固定的中文原因（不夾帶任何請求內容）。
    fn reason(self) -> &'static str {
        match self {
            FileApiError::BadRequest => "請求格式不正確",
            FileApiError::RuntimeUnknown => "設定中沒有這個 runtime",
            FileApiError::PaneUnknown => "目前的畫面中沒有這個 pane",
            FileApiError::NoRoot => "這個 pane 沒有可用的檔案根目錄",
            FileApiError::RootUnavailable => "這個根目錄目前不可用",
            FileApiError::MethodNotAllowed => "這個端點只接受 GET",
            FileApiError::Internal => "讀取檔案系統時發生內部錯誤",
        }
    }
}

impl IntoResponse for FileApiError {
    fn into_response(self) -> Response {
        coded_error_response(self.status(), self.code(), self.reason())
    }
}

// ---------------------------------------------------------------------------
// cwd → 主機路徑
// ---------------------------------------------------------------------------

/// 一個 runtime 回報的 `cwd` 怎麼變成服務所在主機可讀的路徑（spec「檔案根目錄與允許清單」）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PathMapping {
    /// 原樣當主機路徑（非 WSL 的 runtime）。
    Native,
    /// WSL runtime：POSIX 路徑經 [`wsl_host_path`] 轉成 `\\wsl.localhost\<distro>\...`。
    Wsl {
        /// 設定裡 `wsl = { distro, ... }` 的 distro。
        distro: String,
    },
}

impl PathMapping {
    /// 依 runtime 的端點設定決定對應方式：只有 `wsl = { distro, socket }` 是 [`PathMapping::Wsl`]，
    /// 其餘（`socket`、`command`、預設）都是 [`PathMapping::Native`]。
    ///
    /// `command` 端點就算實際上橋接進 WSL，這裡也無從得知 distro，視為 `Native`——其 POSIX `cwd`
    /// 在 Windows 上不是絕對路徑，[`PathMapping::host_path`] 會回 `None`（沒有根目錄；fail-closed）。
    pub fn from_endpoint(endpoint: &HerdrEndpoint) -> Self {
        match endpoint {
            HerdrEndpoint::Wsl { distro, .. } => PathMapping::Wsl {
                distro: distro.clone(),
            },
            HerdrEndpoint::Socket(_) | HerdrEndpoint::Command(_) | HerdrEndpoint::Default => {
                PathMapping::Native
            }
        }
    }

    /// 把 pane 回報的 `cwd` 轉成主機路徑；轉不出來回 `None`（＝該 pane 沒有根目錄）。
    ///
    /// `Native` 雖然是「原樣使用」，仍要求是絕對路徑、且不含 `..` 成分：相對路徑會被解讀成相對於
    /// cockpit 行程自己的工作目錄，跟 pane 無關；`..` 會讓同一個目錄有不同字面（`root_id` 不穩定）
    /// 並讓 `find_root` 的逐層 `parent()` 走到跟實際位置不一致的地方。HERDR 回報的 `cwd` 正常不會有
    /// 這兩種形狀，擋掉只是不讓怪輸入造出怪路徑。
    pub fn host_path(&self, cwd: &str) -> Option<PathBuf> {
        match self {
            PathMapping::Native => {
                let path = Path::new(cwd);
                let has_parent_dir = path
                    .components()
                    .any(|component| matches!(component, Component::ParentDir));
                (path.is_absolute() && !has_parent_dir).then(|| path.to_path_buf())
            }
            PathMapping::Wsl { distro } => wsl_host_path(distro, cwd),
        }
    }
}

/// WSL 的 POSIX 路徑 → Windows 主機可讀的 `\\wsl.localhost\<distro>\...`（spec「WSL 路徑轉換」；
/// design D2）。純函式，不碰檔案系統。
///
/// - 只接受以 `/` 開頭的路徑，否則 `None`。
/// - 空段（`//`、結尾 `/`）略過：POSIX 上 `/a//b/` 就是 `/a/b`，略過讓同一個目錄只有一種字面，
///   `root_id` 才穩定。根 `/` 本身 → `\\wsl.localhost\<distro>\`（distro 分享的根）。
/// - `.`、`..` 段 → `None`：不做字面正規化（`..` 在有符號連結時字面折疊是錯的），HERDR 回報的
///   `cwd` 本來就不該有這兩種段。
/// - 段內含 Windows 路徑無法表示或會被另作解讀的字元 → `None`：`\`（會被 Windows 當分隔符，把一個
///   Linux 檔名拆成兩層）、`/`、`:`（NTFS 替代資料流語法）、`*` `?` `"` `<` `>` `|`、控制字元
///   U+0000–U+001F（含 NUL）。
/// - 段以 `.` 或空白結尾 → `None`：Win32 會默默去掉結尾的點與空白，`repo.` 會被解讀成另一個目錄
///   `repo`。
/// - `distro` 套用同一套段規則（空字串、含 `\`／`/` 等一律 `None`），不讓設定值造出別的 UNC 路徑。
///
/// 被拒的輸入代表「這個 pane 沒有根目錄」，不是錯誤；Linux 上合法但 Windows 無法表示的目錄本來就
/// 無法經 `\\wsl.localhost` 可靠讀取。
pub fn wsl_host_path(distro: &str, posix: &str) -> Option<PathBuf> {
    if !is_plain_segment(distro) {
        return None;
    }
    let rest = posix.strip_prefix('/')?;
    let mut host = format!(r"\\wsl.localhost\{distro}\");
    let mut first = true;
    for segment in rest.split('/') {
        if segment.is_empty() {
            continue;
        }
        if !is_plain_segment(segment) {
            return None;
        }
        if !first {
            host.push('\\');
        }
        host.push_str(segment);
        first = false;
    }
    Some(PathBuf::from(host))
}

/// 一段路徑名稱能不能原封不動放進 Windows 路徑（見 [`wsl_host_path`] 的規則）。
fn is_plain_segment(segment: &str) -> bool {
    const FORBIDDEN: &[char] = &['\\', '/', ':', '*', '?', '"', '<', '>', '|'];
    !segment.is_empty()
        && segment != "."
        && segment != ".."
        && !segment.ends_with('.')
        && !segment.ends_with(' ')
        && !segment
            .chars()
            .any(|c| FORBIDDEN.contains(&c) || u32::from(c) < 0x20)
}

// ---------------------------------------------------------------------------
// root_id（design D2）
// ---------------------------------------------------------------------------

/// `root_id` ＝ 根目錄主機路徑（[`Root::path`]，未 canonicalize）的 UTF-8 位元組之小寫十六進位。
/// 路徑不是合法 UTF-8 時回 `None`（無法給出穩定的 id，視為沒有根目錄）。
pub fn encode_root_id(path: &Path) -> Option<String> {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let text = path.to_str()?;
    let mut id = String::with_capacity(text.len() * 2);
    for byte in text.bytes() {
        id.push(char::from(HEX[usize::from(byte >> 4)]));
        id.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    Some(id)
}

/// [`encode_root_id`] 的反向。只接受 [`encode_root_id`] 產生得出的形狀：非空、偶數長度、只含
/// `0-9a-f`（大寫不收——同一個根目錄只有一種 id），解出的位元組必須是合法 UTF-8；否則
/// [`FileApiError::BadRequest`]。解得出來不代表可用，授權一律看 [`authorize_root`]。
///
/// # Errors
///
/// 不是合法的 `root_id` 時回 [`FileApiError::BadRequest`]。
pub fn decode_root_id(id: &str) -> Result<PathBuf, FileApiError> {
    if id.is_empty() || !id.len().is_multiple_of(2) {
        return Err(FileApiError::BadRequest);
    }
    let bytes = id
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| Some(hex_digit(pair[0])? << 4 | hex_digit(pair[1])?))
        .collect::<Option<Vec<u8>>>()
        .ok_or(FileApiError::BadRequest)?;
    let text = String::from_utf8(bytes).map_err(|_| FileApiError::BadRequest)?;
    Ok(PathBuf::from(text))
}

fn hex_digit(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// 投影查詢與根目錄推算
// ---------------------------------------------------------------------------

/// 在投影中找 `runtime` 底下的 `pane`：找不到回 `None`；找到回 `Some(cwd)`（`cwd` 可能是 `None`）。
fn find_pane_cwd(state: &ProjectedState, runtime: &str, pane: &str) -> Option<Option<String>> {
    state
        .runtimes
        .iter()
        .filter(|rt| rt.id.as_str() == runtime)
        .flat_map(|rt| &rt.workspaces)
        .flat_map(|ws| &ws.tabs)
        .flat_map(|tab| &tab.panes)
        .find(|p| p.id.as_str() == pane)
        .map(|p| p.cwd.clone())
}

/// 投影中 `runtime` 底下所有 pane（含 exited）的 `cwd`，去重。
fn runtime_cwds(state: &ProjectedState, runtime: &str) -> Vec<String> {
    let mut seen = HashSet::new();
    state
        .runtimes
        .iter()
        .filter(|rt| rt.id.as_str() == runtime)
        .flat_map(|rt| &rt.workspaces)
        .flat_map(|ws| &ws.tabs)
        .flat_map(|tab| &tab.panes)
        .filter_map(|p| p.cwd.clone())
        .filter(|cwd| seen.insert(cwd.clone()))
        .collect()
}

/// 一個 `cwd` 的根目錄（會碰檔案系統，呼叫端要在 blocking 執行緒上）。
fn root_of(mapping: &PathMapping, cwd: &str) -> Option<Root> {
    find_root(&mapping.host_path(cwd)?)
}

/// 允許清單驗證（design D2）：`(runtime, root_id)` 是否對應到「最新投影中該 runtime 某個 pane（含
/// exited）推算得出的根目錄」。是就回那個 [`Root`]；file-review task 3.2 的每個檔案端點在碰任何
/// 檔案之前都要先過這一關，並只在回傳的 `Root::path` 之內操作。
///
/// 順序：`root_id` 解碼（400 `bad_request`）→ runtime 是否為設定中的 id（404 `runtime_unknown`，
/// 依 [`AppState::path_mappings`]）→ 當下 `borrow()` 最新投影、取出該 runtime 所有 pane 的 `cwd`
/// （去重）→ 在 blocking 執行緒逐一推算根目錄，與解出的路徑**逐位元組相等**即停 → 全部不符 404
/// `root_unavailable`。不快取。
///
/// 推算前先以 `Path::starts_with` 篩掉「主機路徑不在這個根目錄底下」的 pane：推算出的根目錄一定是
/// 起點本身或其字面上層，所以篩掉的不可能相符，只是少碰檔案系統（WSL 的 UNC 很慢）。字面前綴相符
/// 不代表授權——之後仍要 `find_root` 的結果恰好等於它（上層目錄、子目錄都不算）。
///
/// # Errors
///
/// 見上；blocking task 失敗時回 [`FileApiError::Internal`]。
///
/// # 用法（file-review task 3.2）
///
/// ```ignore
/// let root = match authorize_root(&app, &runtime, &root_id).await {
///     Ok(root) => root,
///     Err(err) => return err.into_response(),
/// };
/// // 之後只在 root.path 之內解析相對路徑（cockpit_files::resolve）……
/// ```
pub async fn authorize_root(
    app: &AppState,
    runtime: &str,
    root_id: &str,
) -> Result<Root, FileApiError> {
    let wanted = decode_root_id(root_id)?;
    let mapping = app
        .path_mappings
        .get(&cockpit_core::RuntimeId::new(runtime))
        .cloned()
        .ok_or(FileApiError::RuntimeUnknown)?;
    // `borrow()` 的 guard 不能跨 await，先把需要的 cwd 複製出來。
    let cwds = runtime_cwds(&app.state.borrow(), runtime);

    tokio::task::spawn_blocking(move || {
        cwds.iter().find_map(|cwd| {
            let host = mapping.host_path(cwd)?;
            if !host.starts_with(&wanted) {
                return None;
            }
            let root = find_root(&host)?;
            (root.path.as_os_str() == wanted.as_os_str()).then_some(root)
        })
    })
    .await
    .map_err(|_| FileApiError::Internal)?
    .ok_or(FileApiError::RootUnavailable)
}

// ---------------------------------------------------------------------------
// GET /api/runtimes/{runtime}/panes/{pane}/root（spec「根目錄查詢端點」）
// ---------------------------------------------------------------------------

/// 根目錄查詢端點的處理常式（路由與 405／`source_check` 的掛法見 [`crate::http::router`]）。
///
/// `path` 收 `Result<.., PathRejection>`：路徑片段 percent-decode 後不是合法 UTF-8 時自己回 400
/// `bad_request`（同輸出端點的理由：axum 預設的 rejection 是純文字、沒有兩個安全標頭）。
pub(crate) async fn pane_root(
    State(app): State<AppState>,
    path: Result<UrlPath<(String, String)>, PathRejection>,
) -> Response {
    let Ok(UrlPath((runtime, pane))) = path else {
        return FileApiError::BadRequest.into_response();
    };
    match resolve_pane_root(&app, &runtime, &pane).await {
        Ok(found) => found.into_response(),
        Err(err) => err.into_response(),
    }
}

/// 200 本體（spec 逐字欄位名）。
#[derive(Serialize)]
struct PaneRootBody {
    runtime: String,
    pane_id: String,
    root_id: String,
    root_path: String,
    cwd_path: String,
    name: String,
    is_git: bool,
}

impl IntoResponse for PaneRootBody {
    fn into_response(self) -> Response {
        let body = serde_json::to_string(&self).expect("只含字串與布林，序列化不會失敗");
        with_no_store_headers(
            (
                StatusCode::OK,
                [(header::CONTENT_TYPE, "application/json")],
                body,
            )
                .into_response(),
        )
    }
}

/// pane → 根目錄：runtime 不在設定 → `runtime_unknown`；pane 不在最新投影 → `pane_unknown`；
/// `cwd` 為 null、轉不出主機路徑、路徑不存在、或根目錄路徑不是 UTF-8 → `no_root`。
async fn resolve_pane_root(
    app: &AppState,
    runtime: &str,
    pane: &str,
) -> Result<PaneRootBody, FileApiError> {
    let mapping = app
        .path_mappings
        .get(&cockpit_core::RuntimeId::new(runtime))
        .cloned()
        .ok_or(FileApiError::RuntimeUnknown)?;
    let cwd = find_pane_cwd(&app.state.borrow(), runtime, pane)
        .ok_or(FileApiError::PaneUnknown)?
        .ok_or(FileApiError::NoRoot)?;

    let cwd_for_fs = cwd.clone();
    let root = tokio::task::spawn_blocking(move || root_of(&mapping, &cwd_for_fs))
        .await
        .map_err(|_| FileApiError::Internal)?
        .ok_or(FileApiError::NoRoot)?;

    let root_id = encode_root_id(&root.path).ok_or(FileApiError::NoRoot)?;
    let root_path = root.path.to_str().ok_or(FileApiError::NoRoot)?.to_owned();
    // 磁碟根（`D:\`）與 WSL distro 根沒有最後一段名稱，退回整個路徑字串。
    let name = root
        .path
        .file_name()
        .and_then(|n| n.to_str())
        .map_or_else(|| root_path.clone(), str::to_owned);

    Ok(PaneRootBody {
        runtime: runtime.to_owned(),
        pane_id: pane.to_owned(),
        root_id,
        root_path,
        cwd_path: cwd,
        name,
        is_git: root.is_git,
    })
}

/// 檔案端點的 405（`POST` 等經 `.fallback(...)`、`HEAD` 經 `.head(...)`；掛法見
/// [`crate::http::router`]）。`HEAD` 的本體由 axum 依 method 自動清空。
pub(crate) async fn method_not_allowed() -> Response {
    FileApiError::MethodNotAllowed.into_response()
}

// ---------------------------------------------------------------------------
// 檔案端點：列目錄、中繼資料、Markdown 渲染、原始內容（file-review task 3.2；spec「檔案端點的共同
// 規則」「列目錄端點」「中繼資料端點」「Markdown 渲染端點」「原始內容端點」「在 VS Code 開啟」「檔案
// icon」；design D3、D5、D9、D10）
// ---------------------------------------------------------------------------

/// 原始內容端點的大小上限（spec「原始內容端點」：超過 50 MiB 回 413 `too_large`）。正式值；測試可經
/// [`FileSettings::raw_limit`] 注入較小的上限，不必在 repo 放大檔。
pub const RAW_SIZE_LIMIT: u64 = 50 * 1024 * 1024;

/// 檔案端點在服務啟動時就決定、之後不變的設定（掛在 [`AppState::files`]）。
#[derive(Debug, Clone)]
pub struct FileSettings {
    /// 已解析的 icon 對照表；回應裡的 `icon`／`icon_open` 是它查出的 SVG 檔名。
    pub icons: IconTheme,
    /// 原始內容端點的大小上限；正式值 [`RAW_SIZE_LIMIT`]。
    pub raw_limit: u64,
}

impl FileSettings {
    /// 正式設定：解析內嵌的 `material-icons.json`，上限 [`RAW_SIZE_LIMIT`]。
    ///
    /// `material-icons.json` 的位元組來自 [`crate::vendor::VENDOR_DIR`]（file-review task 3.3；
    /// design D9）——不再另用 `include_bytes!` 內嵌第二份，同一份 ~450 KiB 的檔案只在執行檔裡出現
    /// 一次；`/vendor/material-icons/material-icons.json` 這個網址與這裡讀到的是同一份資料。
    ///
    /// # Panics
    ///
    /// 內嵌目錄裡沒有這個檔案、或對照表解析失敗時 panic——兩者都是建置時就決定的資料壞掉
    /// （程式錯誤或 vendor 佈局被改壞），不是執行期狀況；
    /// `cockpit/tests/files_endpoint.rs::embedded_icon_theme_parses_and_raw_limit_is_50_mib`
    /// 證明實檔可解析。
    pub fn embedded() -> Self {
        let bytes = crate::vendor::VENDOR_DIR
            .get_file("material-icons/material-icons.json")
            .expect("內嵌目錄必須含 material-icons/material-icons.json（見 vendor/README.md 佈局）")
            .contents();
        Self {
            icons: IconTheme::from_json(bytes)
                .expect("內嵌的 material-icons.json 必須可解析（建置期資料）"),
            raw_limit: RAW_SIZE_LIMIT,
        }
    }
}

/// `cockpit_files::FilesError` → HTTP 回應（spec「檔案端點的共同規則」代碼表）：bad_request 400、
/// path_outside_root 403、not_found 404、wrong_kind 400、too_large 413、not_markdown 415、
/// io_error 500。本體 `{"error": "<固定中文>", "code": "<代碼>"}`：**不含請求路徑片段，也不含 I/O
/// 錯誤訊息原文**（作業系統的錯誤訊息可能帶出主機路徑）；I/O 錯誤只在服務端日誌記種類。
pub fn files_error_response(err: FilesError) -> Response {
    let (status, reason) = match &err {
        FilesError::BadRequest => (StatusCode::BAD_REQUEST, "請求格式不正確"),
        FilesError::PathOutsideRoot => (StatusCode::FORBIDDEN, "目標不在根目錄之內"),
        FilesError::NotFound => (StatusCode::NOT_FOUND, "找不到這個檔案或資料夾"),
        FilesError::WrongKind => (StatusCode::BAD_REQUEST, "目標種類不符"),
        FilesError::TooLarge => (StatusCode::PAYLOAD_TOO_LARGE, "檔案超過大小上限"),
        FilesError::NotMarkdown => (StatusCode::UNSUPPORTED_MEDIA_TYPE, "這個檔案不是 Markdown"),
        FilesError::Io(io) => {
            tracing::warn!(kind = ?io.kind(), "檔案端點讀取時發生 I/O 錯誤");
            (StatusCode::INTERNAL_SERVER_ERROR, "讀取檔案時發生錯誤")
        }
    };
    coded_error_response(status, err.code(), reason)
}

/// 檔案端點處理過程中的錯誤：`cockpit` 這層的（[`FileApiError`]）或 `cockpit-files` 的。
enum EndpointError {
    Api(FileApiError),
    Files(FilesError),
}

impl From<FileApiError> for EndpointError {
    fn from(err: FileApiError) -> Self {
        EndpointError::Api(err)
    }
}

impl From<FilesError> for EndpointError {
    fn from(err: FilesError) -> Self {
        EndpointError::Files(err)
    }
}

impl IntoResponse for EndpointError {
    fn into_response(self) -> Response {
        match self {
            EndpointError::Api(err) => err.into_response(),
            EndpointError::Files(err) => files_error_response(err),
        }
    }
}

/// 四組檔案端點。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FileOp {
    List,
    Meta,
    Render,
    Raw,
}

impl FileOp {
    /// 路由上 `{root_id}` 之後那一段的字面。
    fn segment(self) -> &'static str {
        match self {
            FileOp::List => "list",
            FileOp::Meta => "meta",
            FileOp::Render => "render",
            FileOp::Raw => "raw",
        }
    }
}

/// 從**原始**請求路徑（仍為 percent-encoded）切出 `(runtime, root_id, 相對路徑)`（控制端裁決 R3）。
///
/// 不用 axum `Path` 解碼過的 `{*path}`：那個值已把 `%2F` 解成 `/`，`a%2Fb` 會變成兩段而漏過
/// [`RelPath::parse`] 的「片段含 `/`」規則。這裡以 `/` 切原始路徑——與路由比對的切法相同（axum
/// 也在原始路徑上比對）——再把相對路徑原字串交給 [`RelPath::parse`] 逐段解碼與檢查。
///
/// - `runtime` 段自行 percent-decode（`%` 後不是兩位十六進位、或解出來不是 UTF-8 → 400）；它只拿來
///   查 [`AppState::path_mappings`]，不碰檔案系統。
/// - `root_id` 段原樣交給 [`decode_root_id`]（只收小寫十六進位，任何 `%` 都會被拒）。
/// - 相對路徑：沒有（`.../list`）＝根目錄本身；有 `/` 卻是空字串（`.../list/`）→ 400（空片段）。
///
/// 這一步完全不碰檔案系統，所以壞的相對路徑在任何檔案存取（含允許清單推算）之前就被擋下。
fn parse_target(path: &str, op: FileOp) -> Result<(String, &str, RelPath), EndpointError> {
    let rest = path
        .strip_prefix("/api/files/")
        .ok_or(FileApiError::BadRequest)?;
    let mut parts = rest.splitn(4, '/');
    let runtime_raw = parts.next().ok_or(FileApiError::BadRequest)?;
    let root_id = parts.next().ok_or(FileApiError::BadRequest)?;
    if parts.next() != Some(op.segment()) {
        return Err(FileApiError::BadRequest.into());
    }
    let rel_raw = match parts.next() {
        None => "",
        Some("") => return Err(FilesError::BadRequest.into()),
        Some(raw) => raw,
    };
    let runtime = percent_decode_utf8(runtime_raw).ok_or(FileApiError::BadRequest)?;
    let rel = RelPath::parse(rel_raw)?;
    Ok((runtime, root_id, rel))
}

/// 一段 URL 路徑片段的 percent-decode；`%` 後不是兩位十六進位（大小寫皆可）或結果不是 UTF-8 → `None`。
fn percent_decode_utf8(raw: &str) -> Option<String> {
    let bytes = raw.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hi = hex_digit_any_case(*bytes.get(i + 1)?)?;
            let lo = hex_digit_any_case(*bytes.get(i + 2)?)?;
            out.push(hi << 4 | lo);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

fn hex_digit_any_case(b: u8) -> Option<u8> {
    hex_digit(b.to_ascii_lowercase())
}

/// 四組端點共用的流程：解析原始路徑（400）→ 允許清單（[`authorize_root`]）→ 在 blocking 執行緒上做
/// 該端點的檔案系統工作（R15）。渲染與原始內容的**每個**由這裡產生的回應（含錯誤）都帶
/// `Content-Security-Policy: sandbox`；兩個安全標頭由各回應建構處（[`with_no_store_headers`]／
/// [`coded_error_response`]）負責。
async fn serve(app: AppState, uri: &Uri, op: FileOp) -> Response {
    let mut response = match serve_inner(app, uri, op).await {
        Ok(response) => response,
        Err(err) => err.into_response(),
    };
    if matches!(op, FileOp::Render | FileOp::Raw) {
        response.headers_mut().insert(
            header::CONTENT_SECURITY_POLICY,
            HeaderValue::from_static("sandbox"),
        );
    }
    response
}

async fn serve_inner(app: AppState, uri: &Uri, op: FileOp) -> Result<Response, EndpointError> {
    let (runtime, root_id, rel) = parse_target(uri.path(), op)?;
    let root = authorize_root(&app, &runtime, root_id).await?;
    let mapping = app
        .path_mappings
        .get(&cockpit_core::RuntimeId::new(runtime.as_str()))
        .cloned()
        .ok_or(FileApiError::RuntimeUnknown)?;
    let settings = Arc::clone(&app.files);
    let response = tokio::task::spawn_blocking(move || match op {
        FileOp::List => list_response(&root, &rel, &settings),
        FileOp::Meta => meta_response(&root, &rel, &mapping, &settings),
        FileOp::Render => render_response(&root, &rel),
        FileOp::Raw => raw_response(&root, &rel, settings.raw_limit),
    })
    .await
    .map_err(|_| FileApiError::Internal)??;
    Ok(response)
}

/// 200 JSON 回應，帶兩個安全標頭。
fn json_ok<T: Serialize>(body: &T) -> Response {
    let body =
        serde_json::to_string(body).expect("檔案端點回應只含字串、數字與布林，序列化不會失敗");
    with_no_store_headers(
        (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "application/json")],
            body,
        )
            .into_response(),
    )
}

/// 列目錄（spec「列目錄端點」）：`{"entries": [{name, kind, icon, icon_open}], "omitted", "skipped"}`。
fn list_response(
    root: &Root,
    rel: &RelPath,
    settings: &FileSettings,
) -> Result<Response, FilesError> {
    #[derive(Serialize)]
    struct EntryBody<'a> {
        name: &'a str,
        kind: Kind,
        icon: &'a str,
        /// 資料夾展開時的 icon；檔案為 null（欄位一定出現）。
        icon_open: Option<&'a str>,
    }
    #[derive(Serialize)]
    struct ListBody<'a> {
        entries: Vec<EntryBody<'a>>,
        omitted: usize,
        skipped: usize,
    }

    let listing = list_dir(&root.path, rel)?;
    let entries = listing
        .entries
        .iter()
        .map(|entry| {
            let (icon, icon_open) = match entry.kind {
                Kind::Dir => {
                    let (collapsed, expanded) = settings.icons.folder_icon(&entry.name);
                    (collapsed, Some(expanded))
                }
                Kind::File => (settings.icons.file_icon(&entry.name), None),
            };
            EntryBody {
                name: &entry.name,
                kind: entry.kind,
                icon,
                icon_open,
            }
        })
        .collect();
    Ok(json_ok(&ListBody {
        entries,
        omitted: listing.omitted,
        skipped: listing.skipped,
    }))
}

/// 中繼資料（spec「中繼資料端點」）：`{size, modified_ms, viewer, icon, vscode_uri}`。`icon` 依請求的
/// 檔名（相對路徑最後一段）查；`vscode_uri` 見 [`vscode_uri`]，不適用時為 null。
fn meta_response(
    root: &Root,
    rel: &RelPath,
    mapping: &PathMapping,
    settings: &FileSettings,
) -> Result<Response, FilesError> {
    #[derive(Serialize)]
    struct MetaBody<'a> {
        size: u64,
        modified_ms: i64,
        viewer: Viewer,
        icon: &'a str,
        vscode_uri: Option<String>,
    }

    let meta = file_meta(&root.path, rel)?;
    let name = rel.segments().last().map_or("", String::as_str);
    Ok(json_ok(&MetaBody {
        size: meta.size,
        modified_ms: meta.modified_ms,
        viewer: meta.viewer,
        icon: settings.icons.file_icon(name),
        vscode_uri: vscode_uri(mapping, &root.path, rel.segments()),
    }))
}

/// Markdown 渲染（spec「Markdown 渲染端點」；design D5）：`text/html; charset=utf-8` 的 HTML 片段。
/// 種類、2 MiB 上限與清洗都由 [`render_markdown`] 負責；CSP 由 [`serve`] 補上。
fn render_response(root: &Root, rel: &RelPath) -> Result<Response, FilesError> {
    let html = render_markdown(&root.path, rel)?;
    Ok(with_no_store_headers(
        (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
            html,
        )
            .into_response(),
    ))
}

/// 原始內容（spec「原始內容端點」）：檔案原始位元組，`Content-Type` 見 [`raw_content_type`]。
///
/// 先用 [`file_meta`] 取得界限檢查、種類（資料夾 → `wrong_kind`）與 viewer 分類（「其他 viewer 為
/// text 的檔案」要用它），`size` 超過上限直接 413 只是省下讀取的快速路徑；**真正把關的是
/// [`read_capped`]**：以實際讀到的位元組數判斷，不信任先前看到的 `size`（檔案可能在兩者之間變大）。
fn raw_response(root: &Root, rel: &RelPath, limit: u64) -> Result<Response, FilesError> {
    let meta = file_meta(&root.path, rel)?;
    if meta.size > limit {
        return Err(FilesError::TooLarge);
    }
    let path = resolve(&root.path, rel)?;
    let bytes = read_capped(&path, limit)?;
    let content_type = raw_content_type(&path, meta.viewer, &bytes);
    Ok(with_no_store_headers(
        (
            StatusCode::OK,
            [(header::CONTENT_TYPE, content_type)],
            bytes,
        )
            .into_response(),
    ))
}

const TEXT_PLAIN_UTF8: &str = "text/plain; charset=utf-8";

/// spec「原始內容端點」的 content-type 對照表。副檔名不分大小寫、只看最後一段，取自**解析後的實體
/// 路徑**——與 [`file_meta`] 的 `viewer` 分類看同一個名字（8.3 短名稱或根內連結時，請求的字面可能
/// 不同）。表內沒有的副檔名：`viewer` 為 text → `text/plain; charset=utf-8`，其餘
/// `application/octet-stream`。
///
/// `.html`／`.htm` 依 `bytes`（**實際要回傳的整份位元組**，不是中繼資料只看的前 8192 位元組）決定：
/// 合法 UTF-8 → 帶 `charset=utf-8`，否則不帶 charset，交給檔內 `<meta charset>` 或 BOM（design D1、D2）。
/// 沒帶 charset 又沒宣告時，瀏覽器會用系統舊編碼（繁中 Windows 為 Big5）解碼 UTF-8 而成亂碼。
fn raw_content_type(path: &Path, viewer: Viewer, bytes: &[u8]) -> &'static str {
    let ext = path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(str::to_ascii_lowercase);
    match ext.as_deref() {
        Some("html" | "htm") if std::str::from_utf8(bytes).is_ok() => "text/html; charset=utf-8",
        Some("html" | "htm") => "text/html",
        Some("pdf") => "application/pdf",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("css") => "text/css",
        Some("md" | "markdown" | "txt") => TEXT_PLAIN_UTF8,
        _ if viewer == Viewer::Text => TEXT_PLAIN_UTF8,
        _ => "application/octet-stream",
    }
}

/// `GET /api/files/{runtime}/{root_id}/list` 與 `.../list/{*path}`。
pub(crate) async fn list(State(app): State<AppState>, OriginalUri(uri): OriginalUri) -> Response {
    serve(app, &uri, FileOp::List).await
}

/// `GET /api/files/{runtime}/{root_id}/meta/{*path}`。
pub(crate) async fn meta(State(app): State<AppState>, OriginalUri(uri): OriginalUri) -> Response {
    serve(app, &uri, FileOp::Meta).await
}

/// `GET /api/files/{runtime}/{root_id}/render/{*path}`。
pub(crate) async fn render(State(app): State<AppState>, OriginalUri(uri): OriginalUri) -> Response {
    serve(app, &uri, FileOp::Render).await
}

/// `GET /api/files/{runtime}/{root_id}/raw/{*path}`。
pub(crate) async fn raw(State(app): State<AppState>, OriginalUri(uri): OriginalUri) -> Response {
    serve(app, &uri, FileOp::Raw).await
}

// ---------------------------------------------------------------------------
// 在 VS Code 開啟（spec「在 VS Code 開啟」）
// ---------------------------------------------------------------------------

/// 產生「在 VS Code 開啟」的網址；無法產生時回 `None`（中繼資料回應的 `vscode_uri` 為 null）。純函式，
/// 不碰檔案系統。`root` 是允許清單回傳的根目錄主機路徑（[`Root::path`]，邏輯路徑、未 canonicalize），
/// `rel` 是已解碼的相對路徑片段。
///
/// - 非 WSL runtime（[`PathMapping::Native`]）：`root` 必須以磁碟代號開頭（`X:\` 或 `X:/`）；結果為
///   `vscode://file/X:/<段>/<段>...`，根目錄各段（`\`、`/` 皆為分隔）與相對路徑各段逐段編碼，保留
///   `/` 與磁碟代號的 `:`。
/// - WSL runtime（[`PathMapping::Wsl`]）：`root` 必須是 `\\wsl.localhost\<該 runtime 的 distro>\...`
///   （[`wsl_host_path`] 的產物），還原成 POSIX 路徑後為
///   `vscode://vscode-remote/wsl+<distro><POSIX 路徑>:1`，distro 與各段都編碼；結尾 `:1` 不可省略
///   （spec：否則 VS Code 把遠端路徑當成資料夾開啟）。
/// - 其餘一律 `None`，規則是「只為我們確定 VS Code 認得的兩種形狀產生網址」：非 WSL runtime 的 UNC
///   根目錄（`\\server\share`，含 `\\wsl.localhost` 本身——那種路徑只有標為 WSL 的 runtime 才知道 distro
///   的語意）、`\\?\` 前綴、非絕對路徑；WSL runtime 的根目錄不是它自己 distro 的 `\\wsl.localhost` 路徑
///   （`\\wsl$\...` 也不收，[`wsl_host_path`] 不會產生它）；`rel` 為空（根目錄本身不是檔案）。
///
/// 編碼：RFC 3986 unreserved（`A-Za-z0-9-._~`）以外的每個位元組都編成 `%XX`（大寫十六進位），非 ASCII
/// 字元逐一編其 UTF-8 位元組。不加 crate（控制端裁決 R5）。
pub fn vscode_uri(mapping: &PathMapping, root: &Path, rel: &[String]) -> Option<String> {
    if rel.is_empty() {
        return None;
    }
    let root = root.to_str()?;
    let rel = rel.iter().map(String::as_str);
    match mapping {
        PathMapping::Native => {
            let bytes = root.as_bytes();
            let is_drive_absolute = bytes.len() >= 3
                && bytes[0].is_ascii_alphabetic()
                && bytes[1] == b':'
                && matches!(bytes[2], b'\\' | b'/');
            if !is_drive_absolute {
                return None;
            }
            let mut uri = format!("vscode://file/{}", &root[..2]);
            let segments = root[3..]
                .split(['\\', '/'])
                .filter(|segment| !segment.is_empty());
            for segment in segments.chain(rel) {
                uri.push('/');
                push_percent_encoded(&mut uri, segment);
            }
            Some(uri)
        }
        PathMapping::Wsl { distro } => {
            let rest = root.strip_prefix(r"\\wsl.localhost\")?;
            let (root_distro, posix) = rest.split_once('\\').unwrap_or((rest, ""));
            if root_distro != distro {
                return None;
            }
            let mut uri = String::from("vscode://vscode-remote/wsl+");
            push_percent_encoded(&mut uri, distro);
            let segments = posix.split('\\').filter(|segment| !segment.is_empty());
            for segment in segments.chain(rel) {
                uri.push('/');
                push_percent_encoded(&mut uri, segment);
            }
            uri.push_str(":1");
            Some(uri)
        }
    }
}

/// 把 `segment` 的每個位元組附加到 `out`：RFC 3986 unreserved 原樣，其餘 `%XX`（大寫）。
fn push_percent_encoded(out: &mut String, segment: &str) {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    for byte in segment.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            out.push(char::from(byte));
        } else {
            out.push('%');
            out.push(char::from(HEX[usize::from(byte >> 4)]));
            out.push(char::from(HEX[usize::from(byte & 0x0f)]));
        }
    }
}
