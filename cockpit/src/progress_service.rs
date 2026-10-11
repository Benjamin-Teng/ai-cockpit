//! 進度寫入服務（design D3、D4；spec `pipeline-progress`「狀態檔格式與持久化」）。
//!
//! 所有會改 Domain 狀態的寫入（進度操作、設定／取消覆蓋、刪除失效覆蓋，以及 Repo Project 的加入、修改、
//! 移除、pane 歸類更新與消失 pane 的進度清除——repo-projects task 4.2）都在同一把
//! `tokio::sync::Mutex` 內依序完成：
//!
//! 1. 從 `StoreHandle` 讀目前的 `DomainState`；
//! 2. 在 `DomainState` 的 clone 上呼叫 core 的狀態轉移（`apply_progress`／`set_active`／
//!    `set_override`／`remove_override`，目前 task 的清除規則都在那裡）與 `validate_override`，
//!    被拒絕就回錯、不落檔；
//! 3. 新狀態與目前相同（例如標記已是 none 時 clear、取消不存在的覆蓋）→ 成功、不落檔、不通知；
//! 4. 沒有狀態檔路徑、或新舊狀態序列化成的狀態檔內容完全相同（例如只改了 pane 歸類，repo-projects
//!    task 4.1，design D3「何時寫檔」）→ 不落檔；否則 `spawn_blocking` 寫同目錄 `<檔名>.tmp` 再
//!    `std::fs::rename` 取代狀態檔（零設定模式先建立所在資料夾）；
//! 5. 落檔成功（或不需落檔）才 `StoreHandle::set_domain` 生效。
//!
//! 取消安全：整筆交易（取鎖 → 計算 → 落檔 → 生效）跑在服務自己 `tokio::spawn` 的 task 裡，
//! 由那個 task 持鎖到結束；呼叫端（例如客戶端斷線時被 axum drop 的 handler）只是在等它的
//! `JoinHandle`，被取消也不會中途放掉鎖、跳過 `set_domain`，後續寫入不會與仍在跑的落檔交錯。
//!
//! 例外是失效覆蓋的刪除（design D3）：落檔失敗只記 `tracing::error!`，記憶體照樣刪除——保留一筆
//! 已失效的覆蓋沒有意義，下一次任何成功寫入都會把正確狀態寫出。

use std::collections::BTreeMap;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use cockpit_core::{
    DomainState, Mark, Message, Observation, OpenSpecPhase, Override, PaneId, PaneRepo, PaneRepos,
    ProgressOp, ProjectDef, ProjectId, Rejection, RepoKey, RepoProjectDef, RuntimeId, StageEdit,
    StaleOverride, StoreHandle, TaskId, TaskProgress, WorkstreamId, apply_stage_edits,
    derive_repo_project_id, domain::binding::validate_override,
    domain::repo::REPO_PROJECT_NAME_MAX_CHARS, is_disallowed_label_char,
    normalize_repo_project_name, normalize_repo_project_stages, pane_item_id,
    repo_project_phases_valid,
};
use tokio::sync::{Mutex, mpsc};
use tokio::task::JoinHandle;

use crate::progress::{
    STATE_FILE_VERSION, StateFile, StateOverride, StateProject, StateRepoProject, StateRepoTask,
    StateSync, StateTask,
};

/// 寫入失敗的原因；變體對應 HTTP 狀態碼：`Unknown*`／`NoTaskForPane` → 404、`PaneNotBound` → 403、`Rejected`／`NotOverridable`／`AmbiguousTask` → 409、
/// `Persist`／`Internal` → 500。
#[derive(Debug, thiserror::Error)]
pub enum WriteError {
    /// project 不存在。
    #[error("project 不存在：{0}")]
    UnknownProject(ProjectId),
    /// project 存在，但其中沒有這個 task。
    #[error("task 不存在：{0}")]
    UnknownTask(TaskId),
    /// project 存在，但其中沒有這條 workstream。
    #[error("workstream 不存在：{0}")]
    UnknownWorkstream(WorkstreamId),
    /// 操作被 core 規則拒絕；`Display` 即 409 本體的原因字串。
    #[error(transparent)]
    Rejected(#[from] Rejection),
    /// 狀態檔寫入失敗；記憶體狀態維持操作前的值。
    #[error("寫入狀態檔失敗（{}）：{source}", path.display())]
    Persist {
        /// 狀態檔路徑。
        path: PathBuf,
        /// 底層 I/O 錯誤。
        #[source]
        source: std::io::Error,
    },
    /// 身分判定當時依據的覆蓋事實，在寫入鎖內重驗時已不成立（例如判定後使用者改綁或取消覆蓋）；
    /// HTTP 層對應 403 `pane_not_bound`。
    #[error("pane 已不再綁定到該 workstream")]
    PaneNotBound,
    /// 對 Repo Project 固定綁定到 pane 的工作線設定或取消覆蓋（repo-projects task 4.4）；固定 pane 不接受改綁。
    /// 在寫入鎖內以 Domain 當下的展開結果判定，不看可能落後的投影。HTTP 層對應 409。
    #[error("這條工作線固定綁定到 pane，不能改綁")]
    NotOverridable,
    /// 免帶 id 推進時，綁定到這個 pane 的工作線沒有任何候選 task（無綁定、沒有目前 task 且 task 數不是一張）。
    /// HTTP 層對應 404。
    #[error("這個 pane 沒有可推進的 task")]
    NoTaskForPane,
    /// 免帶 id 推進時，候選 task 有兩張以上，無法判斷要推進哪一張。HTTP 層對應 409。
    #[error("這個 pane 有多張可推進的 task，無法判斷要推進哪一張")]
    AmbiguousTask,
    /// 執行寫入交易的 task 異常結束（panic 或 runtime 關閉時被取消）。
    #[error("寫入任務異常結束：{0}")]
    Internal(#[source] tokio::task::JoinError),
}

impl WriteError {
    /// 穩定的 snake_case 代碼（ui-language design D4）：HTTP 錯誤本體的 `code`，前端依它查字典
    /// 翻譯；`Rejected` 沿用 [`Rejection::code`]。代碼集中定義在這裡，handler 不散寫字串。
    pub fn code(&self) -> &'static str {
        match self {
            Self::UnknownProject(_) => "unknown_project",
            Self::UnknownTask(_) => "unknown_task",
            Self::UnknownWorkstream(_) => "unknown_workstream",
            Self::Rejected(rejection) => rejection.code(),
            Self::Persist { .. } => "persist_failed",
            Self::PaneNotBound => "pane_not_bound",
            Self::NotOverridable => "not_overridable",
            Self::NoTaskForPane => "no_task_for_pane",
            Self::AmbiguousTask => "ambiguous_task",
            Self::Internal(_) => "internal_error",
        }
    }

    /// 與 [`WriteError::code`] 搭配的參數（名稱 → 字串值）；沒有參數回空。`persist_failed` 的
    /// `detail` 只有底層 I/O 原因，刻意**不含**狀態檔路徑（路徑可能帶使用者名稱）。
    pub fn params(&self) -> Vec<(&'static str, String)> {
        match self {
            Self::UnknownProject(id) => vec![("id", id.to_string())],
            Self::UnknownTask(id) => vec![("id", id.to_string())],
            Self::UnknownWorkstream(id) => vec![("id", id.to_string())],
            Self::Persist { source, .. } => vec![("detail", source.to_string())],
            Self::Rejected(_)
            | Self::PaneNotBound
            | Self::NotOverridable
            | Self::NoTaskForPane
            | Self::AmbiguousTask
            | Self::Internal(_) => Vec::new(),
        }
    }
}

/// Repo Project 管理操作（加入、修改、移除）失敗的原因（repo-projects task 4.2，design D6）。HTTP 對應
/// （task 4.4，`crate::http`）：`RepoNotDetected`／`UnknownProject` → 404、`RepoAlreadyAdded`／`NotRepoProject` → 409、
/// `InvalidName`／`InvalidStages` → 400、`Write` 沿用 [`WriteError`] 的對應（`persist_failed` → 500）。
/// 任何錯誤都不改記憶體狀態。
#[derive(Debug, thiserror::Error)]
pub enum RepoProjectError {
    /// `repo` 不是目前 `pane_repos` 中任何 pane 所屬的 repo（只能加入偵測到的 repo）。
    #[error("這個 repo 不在偵測到的清單中")]
    RepoNotDetected(RepoKey),
    /// 這個 repo 已被某個 Repo Project 加入（含被撞名隱藏的）。
    #[error("這個 repo 已經加入")]
    RepoAlreadyAdded(RepoKey),
    /// `pid` 不是任何 Repo Project 的 id，但是手寫 project 的 id。
    #[error("不是 Repo Project：{0}")]
    NotRepoProject(ProjectId),
    /// `pid` 既不是 Repo Project 也不是手寫 project 的 id。
    #[error("project 不存在：{0}")]
    UnknownProject(ProjectId),
    /// 名稱不符合規則（去除前後空白後 1～64 個字元、不含控制字元或不可見的格式字元）。
    #[error("名稱不合規則：去除前後空白後須為 1～64 個字元，且不含控制字元或不可見的格式字元")]
    InvalidName,
    /// stages 不符合規則（1～12 個、各 1～32 字元、不含控制字元或不可見的格式字元、互不相同；`from` 必須是現有 stage 且不重複引用）。
    #[error(
        "stages 不合規則：須為 1～12 個、各 1～32 個字元、不含控制字元或不可見的格式字元且互不相同；from 須是現有的 stage 且不重複引用"
    )]
    InvalidStages,
    /// 寫入交易本身失敗：狀態檔寫入失敗（`persist_failed`）或交易 task 異常結束（`internal_error`）。
    #[error(transparent)]
    Write(#[from] WriteError),
}

impl RepoProjectError {
    /// 穩定的 snake_case 代碼（ui-language design D4）；`Write` 沿用 [`WriteError::code`]。
    pub fn code(&self) -> &'static str {
        match self {
            Self::RepoNotDetected(_) => "repo_not_detected",
            Self::RepoAlreadyAdded(_) => "repo_already_added",
            Self::NotRepoProject(_) => "not_repo_project",
            Self::UnknownProject(_) => "unknown_project",
            Self::InvalidName => "invalid_name",
            Self::InvalidStages => "invalid_stages",
            Self::Write(error) => error.code(),
        }
    }

    /// 與 [`RepoProjectError::code`] 搭配的參數；`Write` 沿用 [`WriteError::params`]。repo key 是主機路徑（可能帶
    /// 使用者名稱），刻意不放進參數。
    pub fn params(&self) -> Vec<(&'static str, String)> {
        match self {
            Self::NotRepoProject(id) | Self::UnknownProject(id) => vec![("id", id.to_string())],
            Self::Write(error) => error.params(),
            Self::RepoNotDetected(_)
            | Self::RepoAlreadyAdded(_)
            | Self::InvalidName
            | Self::InvalidStages => Vec::new(),
        }
    }
}

/// 加入 Repo Project 的請求（`POST /api/repo-projects` 的本體，design D6）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewRepoProject {
    /// 要加入的 repo；必須是目前 `pane_repos` 中某個 pane 所屬的 repo。
    pub repo: RepoKey,
    /// stage 清單（尚未去除前後空白）。
    pub stages: Vec<String>,
    /// 顯示名稱（尚未去除前後空白）；`None` 時用該 repo 的預設名稱。
    pub name: Option<String>,
    /// 與 `stages` 逐項對齊的階段字串（`plan`／`implement`／`review`／`complete` 或 `None`）；`None`（本體省略）等同
    /// 全部不對應（openspec-stage-sync task 4.3）。收字串而非 [`OpenSpecPhase`]，讓未知字串在服務內依既有順序
    /// （名稱 → stages → phases）判定，不在 HTTP 層提早回錯（openspec-stage-sync task 4.5）。
    pub phases: Option<Vec<Option<String>>>,
}

/// [`RepoProjectPatch`] 的一個 stage：同 [`StageEdit`]，但 `phase` 是尚未驗證的字串
/// （openspec-stage-sync task 4.5；理由同 [`NewRepoProject::phases`]）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StageEditInput {
    /// 修改後的 stage 名稱（尚未去除前後空白）。
    pub name: String,
    /// 它原本的 stage 名稱；`None` 表示新增的 stage。
    pub from: Option<String>,
    /// 修改後對應的 OpenSpec 階段字串；`None` 表示不對應（不沿用舊值）。
    pub phase: Option<String>,
}

/// 階段字串 → 階段：`None` 是不對應，不是 `plan`／`implement`／`review`／`complete` 之一的字串回
/// [`RepoProjectError::InvalidStages`]（design D10-1）。正式端點與 `ui_preview` 的假端點共用。
///
/// # Errors
///
/// 任一字串不是四個階段之一時回 [`RepoProjectError::InvalidStages`]。
#[doc(hidden)]
pub fn parse_phases(
    raw: &[Option<String>],
) -> Result<Vec<Option<OpenSpecPhase>>, RepoProjectError> {
    raw.iter()
        .map(|phase| parse_phase(phase.as_deref()))
        .collect()
}

fn parse_phase(raw: Option<&str>) -> Result<Option<OpenSpecPhase>, RepoProjectError> {
    raw.map(|text| OpenSpecPhase::parse(text).ok_or(RepoProjectError::InvalidStages))
        .transpose()
}

/// 修改 Repo Project 的請求（`PATCH /api/repo-projects/{pid}` 的本體，design D6）。兩個欄位都是 `None` 屬
/// `invalid_body`，由 HTTP 層擋；服務收到時只做 pid 查找、不改任何東西。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RepoProjectPatch {
    /// 新名稱（尚未去除前後空白）。
    pub name: Option<String>,
    /// 修改後完整的有序 stage 清單與 `from` 對應。
    pub stages: Option<Vec<StageEditInput>>,
}

/// 測試用：落檔 IO 的開始與結束時點，交給 [`WriteHook`]。
#[doc(hidden)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WriteStage {
    /// 即將在 blocking 執行緒上寫 `.tmp`。
    Start,
    /// `.tmp` 寫入與 `rename` 已結束（不論成功與否）。
    Finish,
}

/// 測試用鉤子：在 blocking 執行緒上於落檔 IO 前後被呼叫，可藉此卡住或觀察寫入。
#[doc(hidden)]
pub type WriteHook = Arc<dyn Fn(WriteStage) + Send + Sync>;

/// agent 端點判定 pane 身分時，那條 workstream 的綁定來源（review M1）。service 在寫入鎖內拿它
/// 比對 Domain 的覆蓋，確認判定依據沒有在「讀投影」與「取得鎖」之間被改綁推翻；只看 Domain
/// 狀態，不依賴 Runtime 層。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BindingBasis {
    /// 判定時綁定來源是自動解析：要求鎖內仍然沒有覆蓋。
    Auto,
    /// 判定時綁定來源是覆蓋：要求鎖內覆蓋仍等於這筆。
    Override(Override),
    /// 判定時綁定來源是 Repo Project 的固定 pane（repo-projects task 3.1）：要求鎖內該 workstream
    /// 仍是固定 pane 的工作線；**忽略覆蓋**——core 的解析對固定 pane 忽略殘留覆蓋、也不回報失效，
    /// 若沿用 `Auto` 的「沒有覆蓋」條件，一筆殘留覆蓋就會讓 agent 推進與宣告永遠被拒。
    Pinned,
}

impl BindingBasis {
    /// 鎖內重驗：`domain` 對 `(project, workstream)` 的覆蓋事實（`Pinned` 為「仍是固定 pane 的工作線」）
    /// 是否仍與判定依據一致。
    fn holds(&self, domain: &DomainState, project: &ProjectId, workstream: &WorkstreamId) -> bool {
        let current = domain
            .overrides
            .get(project)
            .and_then(|m| m.get(workstream));
        match self {
            Self::Auto => current.is_none(),
            Self::Override(expected) => current == Some(expected),
            Self::Pinned => domain
                .projects
                .iter()
                .find(|p| &p.id == project)
                .and_then(|p| p.workstreams.iter().find(|w| &w.id == workstream))
                .is_some_and(|w| w.pinned_pane.is_some()),
        }
    }
}

/// 一筆 OpenSpec 偵測結果（openspec-stage-sync task 4.4；Task 4.3 Ruling）：哪個 pane、偵測當下它的歸類、偵測結果
/// （`None` 為無結果）。歸類用來在寫入鎖內確認 pane 從偵測到送出之間沒有改歸類。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpenSpecEntry {
    /// `(runtime id, pane id)`。
    pub pane: (RuntimeId, PaneId),
    /// 偵測當下這個 pane 的歸類（repo、worktree、根目錄）。
    pub location: PaneRepo,
    /// 偵測結果；`None` 為無結果。
    pub observation: Option<Observation>,
}

/// 狀態檔的落檔位置（repo-projects task 4.1，design D5）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StateFileTarget {
    /// 狀態檔路徑。
    pub path: PathBuf,
    /// 落檔前是否先建立所在資料夾：零設定模式（`%LOCALAPPDATA%\ai-cockpit`）為 `true`，資料夾在第一次
    /// 寫入時才建立；有設定檔時為 `false`，行為不變（資料夾不存在就是寫檔失敗）。
    pub create_parent_dir: bool,
}

struct Inner {
    handle: StoreHandle,
    /// `None`：沒有狀態檔（inline 設定、零設定模式找不到 `LOCALAPPDATA`），只更新記憶體（design D5）。
    target: Option<StateFileTarget>,
    /// 序列化所有寫入（design D4）；鎖內容為空，只用來排隊。
    lock: Mutex<()>,
    write_hook: Option<WriteHook>,
}

/// 進度寫入服務；`Clone` 便宜（內部 `Arc`），可以交給 HTTP handler 與失效覆蓋接收任務各持一份。
/// 一律建立（repo-projects task 4.1，design D5）：沒有狀態檔路徑時只更新記憶體。
#[derive(Clone)]
pub struct ProgressService(Arc<Inner>);

impl ProgressService {
    /// 建立寫入服務：`handle` 是要套用新狀態的控制點，`path` 是狀態檔路徑（所在資料夾必須已存在）。
    pub fn new(handle: StoreHandle, path: PathBuf) -> Self {
        Self::with_target(
            handle,
            Some(StateFileTarget {
                path,
                create_parent_dir: false,
            }),
        )
    }

    /// 沒有狀態檔的寫入服務：所有寫入只更新記憶體（design D5）。
    pub fn in_memory(handle: StoreHandle) -> Self {
        Self::with_target(handle, None)
    }

    /// 依落檔位置建立寫入服務；`target` 為 `None` 時同 [`ProgressService::in_memory`]。
    pub fn with_target(handle: StoreHandle, target: Option<StateFileTarget>) -> Self {
        Self::build(handle, target, None)
    }

    /// 測試用：同 [`ProgressService::new`]，另外在每次落檔 IO 前後呼叫 `hook`。
    #[doc(hidden)]
    pub fn with_write_hook(handle: StoreHandle, path: PathBuf, hook: WriteHook) -> Self {
        let target = StateFileTarget {
            path,
            create_parent_dir: false,
        };
        Self::build(handle, Some(target), Some(hook))
    }

    fn build(
        handle: StoreHandle,
        target: Option<StateFileTarget>,
        write_hook: Option<WriteHook>,
    ) -> Self {
        Self(Arc::new(Inner {
            handle,
            target,
            lock: Mutex::new(()),
            write_hook,
        }))
    }

    /// 換上新的 pane 歸類結果並重算實際生效的 project 清單（design D2、D3），再以記憶體中的最新偵測結果重套
    /// （openspec-stage-sync task 4.3）。依序：
    ///
    /// 1. 歸類改變的 pane（新舊歸類去掉 `default_name` 後不同——repo、worktree 標註或根目錄任一個變了，見
    ///    [`PaneRepo::same_location`]；或已不在新的歸類結果中）的偵測結果從 `openspec_obs` 移除：
    ///    偵測結果是對舊歸類的 worktree 查到的，不得套到新 repo 的卡片上，也不得被手動入口記成 `applied`
    ///    （task 4.3 review fix round 1）。新歸類的結果等下一輪偵測送來。
    /// 2. 換上歸類結果、重算展開。
    /// 3. [`DomainState::reapply_openspec_all`]：新展開的 task 若有屬於它目前歸類的偵測結果，可能被移動、建立同步狀態。
    ///
    /// 歸類結果、展開出的 workstream／task 與最新偵測結果都不寫入狀態檔；只有第 3 步真的改了某張 task 的 stage
    /// 或同步狀態時，序列化內容才會改變而落檔（design D3「何時寫檔」）。
    ///
    /// 不做消失 pane 的進度清除：清除條件不看歸類結果（design D4），由 resolver 每一輪另外呼叫
    /// [`ProgressService::clear_vanished_progress`]（repo-projects task 4.2）。兩者經同一把鎖依序執行。
    ///
    /// # Errors
    ///
    /// 第 3 步造成序列化內容改變且落檔失敗時回傳 [`WriteError::Persist`]；此時整個歸類更新都不生效（記憶體維持
    /// 呼叫前的值），resolver 不更新已送出的紀錄，下一輪重送。
    pub async fn set_pane_repos(&self, pane_repos: PaneRepos) -> Result<(), WriteError> {
        self.write(move |domain| {
            let mut new_domain = domain.clone();
            new_domain.openspec_obs.retain(|key, _| {
                match (domain.pane_repos.get(key), pane_repos.get(key)) {
                    (Some(old), Some(new)) => old.same_location(new),
                    _ => false,
                }
            });
            new_domain.pane_repos = pane_repos;
            new_domain.refresh_projects();
            new_domain.reapply_openspec_all();
            Ok(new_domain)
        })
        .await
    }

    /// 套用一輪 OpenSpec 偵測結果（openspec-stage-sync task 4.3；design D2、D3、D10-6）。`entries` 是**整張**對照表：
    /// 所有仍在 Repo Project 中的 pane → 偵測結果（`None` 為無結果）。一次寫入內完成：以這張表**取代**
    /// `openspec_obs`（值為 `None` 或不在表中的 pane 移除，投影以「沒有這筆」呈現 `sync: null`）。每筆附的歸類
    /// （偵測當下）在鎖內與 `pane_repos` 比對（[`PaneRepo::same_location`]），不符或 pane 已不在 `pane_repos` 中就當無
    /// 結果——偵測與送出之間 pane 可能改了歸類，舊 worktree 的結果不得套到新卡片上（task 4.3 Ruling，task 4.4）。再以
    /// [`DomainState::reapply_openspec_all`] 對每張展開的 Repo Project task 套用自動移動規則（判定依寫入鎖內的當下
    /// 狀態，不看投影）。
    ///
    /// 何時落檔：只有序列化後的狀態檔內容改變（某張 task 的 stage 或同步狀態變了）才寫檔；只改了最新偵測結果時只換上
    /// 記憶體中的狀態（投影可見），不寫檔；完全沒有改變時連記憶體都不換，投影 `version` 不遞增。
    ///
    /// # Errors
    ///
    /// 落檔失敗回 [`WriteError::Persist`]，記憶體（含 `openspec_obs`）維持呼叫前的值；呼叫端（task 4.4）不應把這一輪
    /// 記成「已送出」，下一輪要重送。
    pub async fn sync_openspec(&self, entries: Vec<OpenSpecEntry>) -> Result<(), WriteError> {
        self.write(move |domain| {
            let mut new_domain = domain.clone();
            new_domain.openspec_obs = entries
                .into_iter()
                .filter(|entry| {
                    domain
                        .pane_repos
                        .get(&entry.pane)
                        .is_some_and(|current| current.same_location(&entry.location))
                })
                .filter_map(|entry| entry.observation.map(|obs| (entry.pane, obs)))
                .collect();
            // 不變式：只留展開中 Repo Project task 的 pane（task 4.4 review fix round 2）。
            new_domain.retain_expanded_openspec_obs();
            new_domain.reapply_openspec_all();
            Ok(new_domain)
        })
        .await
    }

    /// 清除消失 pane 的 Repo Project 進度（design D4；spec `repo-projects`「Repo Project 進度的保存與清除」，
    /// repo-projects task 4.2）。在寫入鎖內讀 core `RuntimeStore` **當下**的連線狀態與 pane 樹判定（不讀投影）：
    /// runtime 已連線且 pane 樹中沒有該 pane id 才清；已 exited 但仍在樹中、未連線、尚未連上都保留
    /// （[`DomainState::clear_vanished_repo_progress`]）。有清掉才落檔；resolver 每一輪 repo 判定結束時呼叫一次。
    ///
    /// # Errors
    ///
    /// 有清掉東西但落檔失敗時回傳 [`WriteError::Persist`]，記憶體不變（下一輪會再試）。
    pub async fn clear_vanished_progress(&self) -> Result<(), WriteError> {
        let handle = self.0.handle.clone();
        self.write(move |domain| {
            let mut new_domain = domain.clone();
            let cleared = handle.with_store(|store| new_domain.clear_vanished_repo_progress(store));
            Ok(if cleared { new_domain } else { domain.clone() })
        })
        .await
    }

    /// 加入 Repo Project（design D6；spec `repo-projects`「加入 Repo Project」「Repo Project 的 id 產生」，
    /// repo-projects task 4.2），回傳新 id。依序檢查：名稱（有給時）與 stages 的格式 → `repo` 是否已被加入 →
    /// `repo` 是否為目前 `pane_repos` 中有的 key。名稱未給時用該 repo 的預設名稱（不合名稱規則時去掉控制字元、截到
    /// 64 字元，空的話用 `repo`）。id 與所有現有 project id（手寫與 Repo，含被撞名隱藏的）去重。新 project 的 task
    /// 沒有進度紀錄，即第一個 stage、標記 none。`phases` 省略時全部不對應；給了但與 stages 長度不同或非 `None` 值重複時回
    /// [`RepoProjectError::InvalidStages`]。同 id 的殘留進度與同步狀態一併清掉；展開後以記憶體中的最新偵測結果重套
    /// （openspec-stage-sync task 4.3）。
    ///
    /// # Errors
    ///
    /// [`RepoProjectError::InvalidName`]／[`RepoProjectError::InvalidStages`]／[`RepoProjectError::RepoAlreadyAdded`]／
    /// [`RepoProjectError::RepoNotDetected`]，或落檔失敗（[`RepoProjectError::Write`]）；任何錯誤都不改記憶體。
    pub async fn add_repo_project(
        &self,
        request: NewRepoProject,
    ) -> Result<ProjectId, RepoProjectError> {
        self.transact(move |domain| {
            let name = request
                .name
                .as_deref()
                .map(|raw| normalize_repo_project_name(raw).ok_or(RepoProjectError::InvalidName))
                .transpose()?;
            let stages = normalize_repo_project_stages(&request.stages)
                .ok_or(RepoProjectError::InvalidStages)?;
            // 省略 `phases` 等同全部不對應；有給時字串須合法、與 stages 對齊、非 `None` 值不重複
            // （openspec-stage-sync task 4.3、4.5）。排在名稱與 stages 之後。
            let phases = match request.phases.as_deref() {
                Some(raw) => parse_phases(raw)?,
                None => vec![None; stages.len()],
            };
            if !repo_project_phases_valid(&stages, &phases) {
                return Err(RepoProjectError::InvalidStages);
            }
            if domain.repo_projects.iter().any(|d| d.repo == request.repo) {
                return Err(RepoProjectError::RepoAlreadyAdded(request.repo));
            }
            let Some(detected) = domain.pane_repos.values().find(|p| p.repo == request.repo) else {
                return Err(RepoProjectError::RepoNotDetected(request.repo));
            };
            let name = name.unwrap_or_else(|| default_project_name(&detected.default_name));
            let id = derive_repo_project_id(&name, |candidate| {
                domain.projects.iter().any(|p| p.id.as_str() == candidate)
                    || domain
                        .repo_projects
                        .iter()
                        .any(|p| p.id.as_str() == candidate)
            });

            let mut new_domain = domain.clone();
            new_domain.repo_progress.remove(&id);
            // 殘留進度清掉時同步狀態也要清，否則留下沒有進度項目的 `sync`（openspec-stage-sync task 4.3）。
            new_domain.clear_repo_project_sync(&id);
            new_domain.repo_projects.push(RepoProjectDef {
                id: id.clone(),
                name,
                repo: request.repo,
                phases,
                stages,
            });
            new_domain.refresh_projects();
            new_domain.reapply_openspec_all();
            Ok((new_domain, id))
        })
        .await
    }

    /// 修改 Repo Project 的名稱與（或）stages（design D6；spec `repo-projects`「修改 Repo Project 名稱與 stages」，
    /// repo-projects task 4.2）。`pid` 只查 Repo Project 定義（含被撞名隱藏的）。兩者都給時一起生效或一起被拒；
    /// id 不變。stages 變更時每張 task（全部 `repo_progress`，不只目前展開的）依
    /// [`cockpit_core::StageRemap::stage_for`] 換到新 stage，標記保留。各列的 `phase` 決定新的階段對應；對應依
    /// [`cockpit_core::StageRemap::phases_changed`]（以覆寫前的定義比較）真的改變時，該 project 自動模式的 task 清掉
    /// `applied`，並在同一次寫入內依新對應重套最新偵測結果（openspec-stage-sync task 4.3，design D3、D10-2）。
    ///
    /// # Errors
    ///
    /// 依序檢查：[`RepoProjectError::NotRepoProject`]／[`RepoProjectError::UnknownProject`]、
    /// [`RepoProjectError::InvalidName`]、[`RepoProjectError::InvalidStages`]（含 `from` 不存在或重複引用、階段字串不合法或重複），
    /// 或落檔失敗；
    /// 任何錯誤都不改記憶體。
    pub async fn update_repo_project(
        &self,
        project: &ProjectId,
        patch: RepoProjectPatch,
    ) -> Result<(), RepoProjectError> {
        let project = project.clone();
        self.transact(move |domain| {
            // 先判定 pid、再驗內容，與移除一致（repo-projects task 4.6 Minor 3）。
            let index = find_repo_project(domain, &project)?;
            let name = patch
                .name
                .as_deref()
                .map(|raw| normalize_repo_project_name(raw).ok_or(RepoProjectError::InvalidName))
                .transpose()?;
            // 排在名稱之後：先轉階段字串（未知字串 → `InvalidStages`），再套用 stage 清單（openspec-stage-sync task 4.5）。
            let remap = patch
                .stages
                .as_deref()
                .map(|inputs| {
                    let edits = inputs
                        .iter()
                        .map(|input| {
                            Ok(StageEdit {
                                name: input.name.clone(),
                                from: input.from.clone(),
                                phase: parse_phase(input.phase.as_deref())?,
                            })
                        })
                        .collect::<Result<Vec<_>, RepoProjectError>>()?;
                    apply_stage_edits(&domain.repo_projects[index].stages, &edits)
                        .ok_or(RepoProjectError::InvalidStages)
                })
                .transpose()?;

            let mut new_domain = domain.clone();
            let def = &mut new_domain.repo_projects[index];
            if let Some(name) = name {
                def.name = name;
            }
            if let Some(remap) = remap {
                let old_first = def.stages.first().cloned().unwrap_or_default();
                let repo = def.repo.clone();
                // 判斷階段對應是否改變要用**覆寫前**的定義（openspec-stage-sync task 4.3，design D10-2）。
                let mapping_changed = remap.phases_changed(&def.stages, &def.phases);
                def.stages = remap.stages().to_vec();
                def.phases = remap.phases().to_vec();
                if mapping_changed {
                    // 自動的 task 清掉 `applied`，下面的全表重套依新對應重新套用；手動的 task 不動（design D3）。
                    new_domain.reset_auto_applied(&project);
                }
                // 沒有進度紀錄的 task 在**舊**的第一個 stage（投影退回 `TaskProgress::initial`，取的是當下定義的
                // 第一個 stage）。舊的第一個 stage 對應到的不是新的第一個 stage 時（重新排序、在前面插入），先替這個
                // repo 目前歸類到的每個 pane 補上 `(舊的第一個 stage, none)`，再與既有紀錄一起套用對應，否則它們會被
                // 靜默移到新的第一個 stage（fix round 1 Critical 1）。依 `pane_repos` 補而不是依展開結果，被撞名隱藏的
                // Repo Project 才同樣正確。
                let table = new_domain.repo_progress.entry(project.clone()).or_default();
                if remap.stage_for(&old_first) != remap.first_stage() {
                    for (runtime, pane) in new_domain
                        .pane_repos
                        .iter()
                        .filter(|(_, classified)| classified.repo == repo)
                        .map(|(key, _)| key)
                    {
                        table
                            .entry(TaskId::new(pane_item_id(runtime, pane)))
                            .or_insert_with(|| TaskProgress {
                                stage: old_first.clone(),
                                mark: Mark::None,
                            });
                    }
                }
                for progress in table.values_mut() {
                    progress.stage = remap.stage_for(&progress.stage).to_string();
                }
                if table.is_empty() {
                    new_domain.repo_progress.remove(&project);
                }
                // 載入時的 stage 退回警告在 stages 改過之後已過時，清掉（repo-projects task 4.6 Minor 2）；被撞名隱藏時
                // `warnings[pid]` 屬於同 id 的手寫 project，不動。
                if !hidden_by_config_project(&new_domain, &project)
                    && let Some(list) = new_domain.warnings.get_mut(&project)
                {
                    list.retain(|text| {
                        !matches!(Message::classify(text), Message::TaskStageReset { .. })
                    });
                    if list.is_empty() {
                        new_domain.warnings.remove(&project);
                    }
                }
            }
            new_domain.refresh_projects();
            new_domain.reapply_openspec_all();
            Ok((new_domain, ()))
        })
        .await
    }

    /// 移除 Repo Project（spec `repo-projects`「移除 Repo Project」，repo-projects task 4.2）：定義與它的
    /// `repo_progress` 與同步狀態（`repo_sync`，openspec-stage-sync task 4.3）一併移除；`pid` 查找規則同 [`ProgressService::update_repo_project`]。只清自己的
    /// `repo_progress`：被撞名隱藏時，同 id 手寫 project 的進度、覆蓋與目前 task 都不動；沒被隱藏時連同記憶體中殘留的
    /// `overrides[pid]`／`active[pid]` 一併清掉。
    ///
    /// # Errors
    ///
    /// [`RepoProjectError::NotRepoProject`]／[`RepoProjectError::UnknownProject`]，或落檔失敗；任何錯誤都不改記憶體。
    pub async fn remove_repo_project(&self, project: &ProjectId) -> Result<(), RepoProjectError> {
        let project = project.clone();
        self.transact(move |domain| {
            let index = find_repo_project(domain, &project)?;
            let mut new_domain = domain.clone();
            new_domain.repo_projects.remove(index);
            new_domain.repo_progress.remove(&project);
            // 同步狀態只屬於 Repo Project，撞名隱藏與否都一併清掉（openspec-stage-sync task 4.3）。
            new_domain.clear_repo_project_sync(&project);
            // 沒被撞名隱藏時，`overrides[pid]`／`active[pid]`／`warnings[pid]` 若有殘留也屬於這個 Repo Project，一併
            // 清掉（fix round 1 Minor 2；warnings 為 task 4.6 Minor 2：同 repo、同名再加入會拿到同一個 id）；被隱藏時
            // 屬於同 id 的手寫 project，不動。
            if !hidden_by_config_project(&new_domain, &project) {
                new_domain.overrides.remove(&project);
                new_domain.active.remove(&project);
                new_domain.warnings.remove(&project);
            }
            new_domain.refresh_projects();
            new_domain.reapply_openspec_all();
            Ok((new_domain, ()))
        })
        .await
    }

    /// 對一個 task 套用進度操作（spec `pipeline-progress`「進度寫入端點」）。Repo Project task 的推進／退回成功時，在同一次
    /// 寫入內標為手動（[`DomainState::mark_manual`]）；清除標記成功時，在同一次寫入內重套最新偵測結果；標 Completed／
    /// Failed 不改同步狀態（openspec-stage-sync task 4.3，design D3）。
    ///
    /// # Errors
    ///
    /// project／task 不存在、操作被拒絕、或狀態檔寫入失敗時回傳 [`WriteError`]；任何錯誤都不改
    /// 記憶體狀態。
    pub async fn apply_progress(
        &self,
        project: &ProjectId,
        task: &TaskId,
        op: ProgressOp,
    ) -> Result<(), WriteError> {
        let project = project.clone();
        let task = task.clone();
        self.write(move |domain| {
            let mut new_domain = domain.clone();
            let ensured = ensure_task(&mut new_domain, &project, &task)?;
            new_domain.apply_progress(&project, &task, op)?;
            match op {
                // 人工推進／退回成功後在同一次寫入內標手動，只落檔一次（openspec-stage-sync task 4.3，design D3）；
                // 被拒絕時上面已回錯，不會走到這裡。手寫 project 的 task 是 no-op。
                ProgressOp::Advance | ProgressOp::Retreat => {
                    new_domain.mark_manual(&project, &task);
                }
                // 清除標記後，標記期間被擋下的最新偵測結果在同一次寫入內套用（Task 3.2 Ruling：背景工作去重，
                // 偵測不變時不會再送下一輪）。`mode` 只有真的套用時才變自動（design D3 第 4 條）。
                ProgressOp::Clear => {
                    new_domain.reapply_openspec_all();
                }
                ProgressOp::Complete | ProgressOp::Fail => {}
            }
            ensured.finish(&mut new_domain);
            Ok(new_domain)
        })
        .await
    }

    /// 宣告目前 task：把 `task` 設為其所屬 workstream 的目前 task（spec `agent-reporting`「宣告目前
    /// task」；progress-model task 3.2，design D3）。重複宣告同一個 task 時狀態不變，成功、不落檔。
    /// 「這個 pane 是否綁定到該 workstream」的身分判定在 HTTP 層（task 3.4），這裡只在鎖內以
    /// `basis` 重驗判定依據的覆蓋事實（review M1），不符回 [`WriteError::PaneNotBound`]。
    ///
    /// # Errors
    ///
    /// project／task 不存在（[`WriteError::UnknownProject`]／[`WriteError::UnknownTask`]）、task 已有
    /// 標記（[`WriteError::Rejected`]）、或狀態檔寫入失敗時回傳 [`WriteError`]；任何錯誤都不改記憶體。
    pub async fn declare_active(
        &self,
        project: &ProjectId,
        task: &TaskId,
        basis: &BindingBasis,
    ) -> Result<(), WriteError> {
        let project = project.clone();
        let task = task.clone();
        let basis = basis.clone();
        self.write(move |domain| {
            let mut new_domain = domain.clone();
            let ensured = ensure_task(&mut new_domain, &project, &task)?;
            if !basis.holds(domain, &project, &ensured.workstream) {
                return Err(WriteError::PaneNotBound);
            }
            new_domain.set_active(&project, &ensured.workstream, &task)?;
            ensured.finish(&mut new_domain);
            Ok(new_domain)
        })
        .await
    }

    /// agent 推進：同一筆交易先推進 `task` 的 stage（Repo Project task 同時標為手動，openspec-stage-sync task 4.3）、
    /// 再把它設為所屬 workstream 的目前 task（spec
    /// `agent-reporting`「agent 推進」；progress-model task 3.2）。兩步在同一把鎖內、只落檔一次；
    /// 任一步被拒絕整筆不生效。身分判定與 `basis` 重驗同 [`ProgressService::declare_active`]。
    ///
    /// # Errors
    ///
    /// 同 [`ProgressService::declare_active`]；推進規則被拒絕（最後一站、已有標記）也走
    /// [`WriteError::Rejected`]。
    pub async fn agent_advance(
        &self,
        project: &ProjectId,
        task: &TaskId,
        basis: &BindingBasis,
    ) -> Result<(), WriteError> {
        let project = project.clone();
        let task = task.clone();
        let basis = basis.clone();
        self.write(move |domain| {
            let mut new_domain = domain.clone();
            let ensured = ensure_task(&mut new_domain, &project, &task)?;
            if !basis.holds(domain, &project, &ensured.workstream) {
                return Err(WriteError::PaneNotBound);
            }
            new_domain.apply_progress(&project, &task, ProgressOp::Advance)?;
            // agent 推進算手動入口（openspec-stage-sync task 4.3，design D3）：同一次寫入內標記。
            new_domain.mark_manual(&project, &task);
            new_domain.set_active(&project, &ensured.workstream, &task)?;
            ensured.finish(&mut new_domain);
            Ok(new_domain)
        })
        .await
    }

    /// 免帶 id 的 agent 推進（spec `agent-reporting`「免帶 id 推進」；design D7；repo-projects task 4.4）。
    /// `bound` 是 HTTP 層以（可能落後的）投影判定出「綁定到這個 pane」的工作線與判定依據；**候選 task 在寫入鎖內
    /// 依 Domain 選**，不信任投影（專案 memory `check-against-lagging-projection-misses-fresh-writes`：agent 剛
    /// 宣告完目前 task、投影還沒更新時，用投影選會推進到舊的那張並覆寫剛宣告的）。每條工作線：Domain 有目前 task
    /// 就取它，否則生效 def 中屬於它的 task 恰一張才取。候選恰一張時，先以 `basis` 重驗、再同
    /// [`ProgressService::agent_advance`] 推進並設為目前 task。
    ///
    /// # Errors
    ///
    /// 候選零張 [`WriteError::NoTaskForPane`]、兩張以上 [`WriteError::AmbiguousTask`]（狀態都不變）；其餘同
    /// [`ProgressService::agent_advance`]。
    pub async fn agent_advance_for_pane(
        &self,
        bound: Vec<(ProjectId, WorkstreamId, BindingBasis)>,
    ) -> Result<(), WriteError> {
        self.write(move |domain| {
            let mut candidates = Vec::new();
            for (project, workstream, basis) in &bound {
                let Some(def) = domain.projects.iter().find(|p| &p.id == project) else {
                    continue;
                };
                let mut own = def.tasks.iter().filter(|t| &t.workstream == workstream);
                let chosen = match domain.active_task(project, workstream) {
                    Some(active) => own.find(|t| &t.id == active),
                    None => match (own.next(), own.next()) {
                        (Some(only), None) => Some(only),
                        _ => None,
                    },
                };
                if let Some(task) = chosen {
                    candidates.push((project, workstream, &task.id, basis));
                }
            }
            let [(project, workstream, task, basis)] = candidates.as_slice() else {
                return Err(if candidates.is_empty() {
                    WriteError::NoTaskForPane
                } else {
                    WriteError::AmbiguousTask
                });
            };
            if !basis.holds(domain, project, workstream) {
                return Err(WriteError::PaneNotBound);
            }
            let mut new_domain = domain.clone();
            let ensured = ensure_task(&mut new_domain, project, task)?;
            new_domain.apply_progress(project, task, ProgressOp::Advance)?;
            // 鎖內選出的 task 就地標手動，不在鎖外重選（openspec-stage-sync task 4.3，design D3）。
            new_domain.mark_manual(project, task);
            new_domain.set_active(project, workstream, task)?;
            ensured.finish(&mut new_domain);
            Ok(new_domain)
        })
        .await
    }

    /// 設定一條 workstream 的畫面覆蓋（spec `pipeline-progress`「綁定覆蓋端點」`PUT`）。
    ///
    /// # Errors
    ///
    /// project／workstream 不存在、workstream 是 Repo Project 的固定 pane 工作線
    /// （[`WriteError::NotOverridable`]）、`validate_override` 拒絕、或狀態檔寫入失敗時回傳
    /// [`WriteError`]；任何錯誤都不改記憶體狀態。
    pub async fn set_override(
        &self,
        project: &ProjectId,
        workstream: &WorkstreamId,
        override_: Override,
    ) -> Result<(), WriteError> {
        let handle = self.0.handle.clone();
        let project = project.clone();
        let workstream = workstream.clone();
        self.write(move |domain| {
            ensure_workstream(domain, &project, &workstream)?;
            ensure_overridable(domain, &project, &workstream)?;
            handle.with_store(|store| validate_override(&override_, store))?;

            let mut new_domain = domain.clone();
            new_domain.set_override(&project, &workstream, override_);
            Ok(new_domain)
        })
        .await
    }

    /// 取消一條 workstream 的畫面覆蓋（`DELETE`）；覆蓋本來就不存在時成功且不落檔。
    ///
    /// # Errors
    ///
    /// project／workstream 不存在、workstream 是 Repo Project 的固定 pane 工作線
    /// （[`WriteError::NotOverridable`]）、或狀態檔寫入失敗時回傳 [`WriteError`]。
    pub async fn clear_override(
        &self,
        project: &ProjectId,
        workstream: &WorkstreamId,
    ) -> Result<(), WriteError> {
        let project = project.clone();
        let workstream = workstream.clone();
        self.write(move |domain| {
            ensure_workstream(domain, &project, &workstream)?;
            ensure_overridable(domain, &project, &workstream)?;
            let mut new_domain = domain.clone();
            new_domain.remove_override(&project, &workstream);
            Ok(new_domain)
        })
        .await
    }

    /// 刪除投影判定失效的覆蓋（design D3）。在鎖內逐筆確認目前覆蓋**仍等於**通知中的那筆才刪
    /// （不等代表使用者已改綁，略過）；有任何刪除才落檔。落檔失敗記 `tracing::error!`，記憶體
    /// 照樣刪除。整筆交易同樣跑在服務自己的 task 裡（取消安全，見模組文件）。
    pub async fn remove_stale(&self, stale: Vec<StaleOverride>) {
        let inner = Arc::clone(&self.0);
        let transaction = tokio::spawn(async move {
            let _guard = inner.lock.lock().await;
            let current = inner.handle.with_domain(Clone::clone);

            let mut new_domain = current.clone();
            for item in &stale {
                let still_same = new_domain
                    .overrides
                    .get(&item.project)
                    .and_then(|m| m.get(&item.workstream))
                    == Some(&item.override_);
                if still_same {
                    new_domain.remove_override(&item.project, &item.workstream);
                }
            }
            if new_domain == current {
                return;
            }

            if let Err(error) = inner.persist_if_changed(&current, &new_domain).await {
                tracing::error!(%error, "刪除失效覆蓋後寫入狀態檔失敗；記憶體中照樣刪除");
            }
            inner.handle.set_domain(new_domain);
        });
        if let Err(error) = transaction.await {
            tracing::error!(%error, "刪除失效覆蓋的寫入任務異常結束");
        }
    }

    /// 起一個任務持續接收投影送來的失效覆蓋清單並逐批 [`ProgressService::remove_stale`]；
    /// 送端關閉時結束。
    pub fn spawn_stale_remover(
        &self,
        mut rx: mpsc::UnboundedReceiver<Vec<StaleOverride>>,
    ) -> JoinHandle<()> {
        let service = self.clone();
        tokio::spawn(async move {
            while let Some(batch) = rx.recv().await {
                service.remove_stale(batch).await;
            }
        })
    }

    /// 共用寫入路徑：在服務自己的 task 裡「鎖 → 算新狀態（錯誤即回）→ 沒變就結束 → 落檔 →
    /// 成功才生效」；呼叫端只等 `JoinHandle`，被取消也不影響交易完整跑完。
    async fn write(
        &self,
        compute: impl FnOnce(&DomainState) -> Result<DomainState, WriteError> + Send + 'static,
    ) -> Result<(), WriteError> {
        self.transact(move |domain| compute(domain).map(|new_domain| (new_domain, ())))
            .await
    }

    /// [`ProgressService::write`] 的一般形：`compute` 另外回傳一個結果值（例如加入時的新 id），錯誤型別可以是任何
    /// 能由 [`WriteError`] 轉來的型別（repo-projects task 4.2 的 [`RepoProjectError`]）。新狀態與目前相同時不落檔、
    /// 不生效，結果值照樣回傳。
    async fn transact<T, E>(
        &self,
        compute: impl FnOnce(&DomainState) -> Result<(DomainState, T), E> + Send + 'static,
    ) -> Result<T, E>
    where
        T: Send + 'static,
        E: From<WriteError> + Send + 'static,
    {
        let inner = Arc::clone(&self.0);
        tokio::spawn(async move {
            let _guard = inner.lock.lock().await;
            let current = inner.handle.with_domain(Clone::clone);
            let (new_domain, output) = compute(&current)?;
            debug_assert!(
                repo_sync_backed_by_progress(&new_domain),
                "有同步狀態的 task 一定有進度項目（openspec-stage-sync R1）"
            );
            if new_domain != current {
                inner.persist_if_changed(&current, &new_domain).await?;
                inner.handle.set_domain(new_domain);
            }
            Ok(output)
        })
        .await
        .unwrap_or_else(|join_error| Err(E::from(WriteError::Internal(join_error))))
    }
}

impl Inner {
    /// 決定要不要落檔並寫出（design D3「何時寫檔」、D5）：沒有狀態檔路徑時不寫；`new` 序列化後的狀態檔內容與
    /// `current` 的完全相同時不寫（pane 歸類、展開結果、載入警告都不進檔案，只改它們不會落檔，舊的 v2 檔也不會
    /// 在沒有任何操作下被升成 v3）；否則在 blocking 執行緒上寫成狀態檔（`.tmp` 再 `rename`）。
    async fn persist_if_changed(
        &self,
        current: &DomainState,
        new: &DomainState,
    ) -> Result<(), WriteError> {
        let Some(target) = &self.target else {
            return Ok(());
        };
        let json = serialize_state_file(new);
        if json == serialize_state_file(current) {
            return Ok(());
        }
        let path = target.path.clone();
        let create_parent_dir = target.create_parent_dir;
        let hook = self.write_hook.clone();
        let result = tokio::task::spawn_blocking({
            let path = path.clone();
            move || {
                if let Some(hook) = &hook {
                    hook(WriteStage::Start);
                }
                let result = create_parent_if_needed(&path, create_parent_dir)
                    .and_then(|()| write_atomically(&path, &json));
                if let Some(hook) = &hook {
                    hook(WriteStage::Finish);
                }
                result
            }
        })
        .await
        .unwrap_or_else(|join_error| Err(std::io::Error::other(join_error)));
        result.map_err(|source| WriteError::Persist { path, source })
    }
}

/// 不變式 R1（openspec-stage-sync task 4.1／4.3，design D2）：`repo_sync` 的每一筆在 `repo_progress` 都有對應的進度
/// 項目。狀態檔把 `sync` 放在 task 進度底下，沒有進度的 `sync` 寫不出去、會靜默消失。
fn repo_sync_backed_by_progress(domain: &DomainState) -> bool {
    domain.repo_sync.iter().all(|(project, table)| {
        table.keys().all(|task| {
            domain
                .repo_progress
                .get(project)
                .is_some_and(|progress| progress.contains_key(task))
        })
    })
}

fn find_project<'a>(
    domain: &'a DomainState,
    project: &ProjectId,
) -> Result<&'a cockpit_core::ProjectDef, WriteError> {
    domain
        .projects
        .iter()
        .find(|p| &p.id == project)
        .ok_or_else(|| WriteError::UnknownProject(project.clone()))
}

fn ensure_workstream(
    domain: &DomainState,
    project: &ProjectId,
    workstream: &WorkstreamId,
) -> Result<(), WriteError> {
    let def = find_project(domain, project)?;
    if def.workstreams.iter().any(|w| &w.id == workstream) {
        Ok(())
    } else {
        Err(WriteError::UnknownWorkstream(workstream.clone()))
    }
}

/// 確認 workstream 可以設定或取消覆蓋：Repo Project 固定綁定到 pane 的工作線（`pinned_pane` 有值）不接受改綁，
/// 回 [`WriteError::NotOverridable`]（repo-projects task 4.4）。呼叫前須已通過 [`ensure_workstream`]。
fn ensure_overridable(
    domain: &DomainState,
    project: &ProjectId,
    workstream: &WorkstreamId,
) -> Result<(), WriteError> {
    let pinned = find_project(domain, project)?
        .workstreams
        .iter()
        .any(|w| &w.id == workstream && w.pinned_pane.is_some());
    if pinned {
        Err(WriteError::NotOverridable)
    } else {
        Ok(())
    }
}

/// [`ensure_task`] 的結果：task 所屬的 workstream，以及（有補時）為了前置條件補上的初始進度。
struct EnsuredTask {
    def: ProjectDef,
    task: TaskId,
    workstream: WorkstreamId,
    /// 進度表原本沒有這個 task 時補上的初始進度；原本就有時為 `None`。
    inserted: Option<TaskProgress>,
    /// 這個 project 的進度表是否也是剛補的（原本整張表都不存在）。
    created_table: bool,
}

impl EnsuredTask {
    /// 操作結束後撤回沒被改動的補值（[`DomainState::drop_untouched_initial`]），讓空操作的結果與操作前完全相同
    /// （repo-projects task 4.2：對沒有紀錄的 Repo Project task 宣告或清除標記不落檔）。
    fn finish(&self, domain: &mut DomainState) {
        if let Some(initial) = &self.inserted {
            domain.drop_untouched_initial(&self.def, &self.task, initial, self.created_table);
        }
    }
}

/// 確認 project 與 task 存在並回傳 task 所屬的 workstream；生效 def 對應的進度表（依種類分流，
/// [`DomainState::progress_for_mut`]，repo-projects task 4.2）缺這個 task 的項目時補初始進度，滿足
/// `DomainState::apply_progress` 的前置條件。呼叫端在操作後呼叫 [`EnsuredTask::finish`]。
fn ensure_task(
    domain: &mut DomainState,
    project: &ProjectId,
    task: &TaskId,
) -> Result<EnsuredTask, WriteError> {
    let def = find_project(domain, project)?.clone();
    let task_def = def
        .tasks
        .iter()
        .find(|t| &t.id == task)
        .ok_or_else(|| WriteError::UnknownTask(task.clone()))?;
    let workstream = task_def.workstream.clone();
    let initial = TaskProgress::initial(task_def);
    let created_table = domain.progress_for(&def).is_none();
    let table = domain.progress_for_mut(&def);
    let inserted = if table.contains_key(task) {
        None
    } else {
        table.insert(task.clone(), initial.clone());
        Some(initial)
    };
    Ok(EnsuredTask {
        def,
        task: task.clone(),
        workstream,
        inserted,
        created_table,
    })
}

/// `pid` 在 `repo_projects` 中的位置（只查 Repo Project 定義，含被撞名隱藏的；design D6）。不是 Repo Project
/// 時：是手寫 project 的 id → [`RepoProjectError::NotRepoProject`]，否則 [`RepoProjectError::UnknownProject`]。
fn find_repo_project(domain: &DomainState, project: &ProjectId) -> Result<usize, RepoProjectError> {
    if let Some(index) = domain.repo_projects.iter().position(|d| &d.id == project) {
        return Ok(index);
    }
    if domain
        .projects
        .iter()
        .any(|p| &p.id == project && p.repo.is_none())
    {
        Err(RepoProjectError::NotRepoProject(project.clone()))
    } else {
        Err(RepoProjectError::UnknownProject(project.clone()))
    }
}

/// `pid` 是否被同 id 的手寫 project 隱藏（撞名，design D3）：是的話 `overrides`／`active`／`warnings` 中這個 id 的
/// 項目都屬於手寫 project。
fn hidden_by_config_project(domain: &DomainState, project: &ProjectId) -> bool {
    domain
        .projects
        .iter()
        .any(|p| &p.id == project && p.repo.is_none())
}

/// 加入時沒給名稱所用的名稱：repo 的預設名稱（design D1）符合名稱規則就照用；否則（例如資料夾名超過 64 字元）
/// 去掉控制字元與零寬／雙向格式字元（[`is_disallowed_label_char`]）、去除前後空白、截到 64 字元，空的話用
/// `repo`。預設名稱不是使用者輸入，不因此回 `invalid_name`。
fn default_project_name(default_name: &str) -> String {
    if let Some(name) = normalize_repo_project_name(default_name) {
        return name;
    }
    let cleaned: String = default_name
        .chars()
        .filter(|c| !is_disallowed_label_char(*c))
        .collect();
    let truncated: String = cleaned
        .trim()
        .chars()
        .take(REPO_PROJECT_NAME_MAX_CHARS)
        .collect();
    normalize_repo_project_name(&truncated).unwrap_or_else(|| "repo".to_string())
}

/// `DomainState` → 狀態檔形狀（design D5）：`projects` 區段只放手寫 project（`repo` 為 `None`），每個都寫出
/// 全部 task，沒有覆蓋的 project 寫 `{}`；`repo_projects` 區段依 `repo_projects` 定義與 `repo_progress` 對照表
/// 全部寫出，不以目前展開出來的 task 過濾（design D4；含因 id 撞名而沒展開的 Repo Project）。
fn to_state_file(domain: &DomainState) -> StateFile {
    let projects = domain
        .projects
        .iter()
        .filter(|def| def.repo.is_none())
        .map(|def| {
            let progress = domain.progress_for(def);
            let tasks = def
                .tasks
                .iter()
                .map(|task| {
                    let current = progress
                        .and_then(|m| m.get(&task.id))
                        .cloned()
                        .unwrap_or_else(|| TaskProgress::initial(task));
                    (
                        task.id.as_str().to_string(),
                        StateTask {
                            stage: current.stage,
                            mark: current.mark,
                        },
                    )
                })
                .collect();
            let overrides = domain
                .overrides
                .get(&def.id)
                .map(|m| {
                    m.iter()
                        .map(|(ws, o)| {
                            (
                                ws.as_str().to_string(),
                                StateOverride {
                                    runtime: o.runtime.as_str().to_string(),
                                    pane_id: o.pane_id.as_str().to_string(),
                                },
                            )
                        })
                        .collect()
                })
                .unwrap_or_default();
            let active = domain
                .active
                .get(&def.id)
                .map(|m| {
                    m.iter()
                        .map(|(ws, task)| (ws.as_str().to_string(), task.as_str().to_string()))
                        .collect()
                })
                .unwrap_or_default();
            (
                def.id.as_str().to_string(),
                StateProject {
                    tasks,
                    overrides,
                    active: Some(Some(active)),
                },
            )
        })
        .collect::<BTreeMap<_, _>>();

    let repo_projects = domain
        .repo_projects
        .iter()
        .map(|def| {
            let tasks = domain
                .repo_progress
                .get(&def.id)
                .map(|m| {
                    m.iter()
                        .map(|(task, progress)| {
                            (
                                task.as_str().to_string(),
                                StateRepoTask {
                                    stage: progress.stage.clone(),
                                    mark: progress.mark,
                                    sync: domain
                                        .repo_sync
                                        .get(&def.id)
                                        .and_then(|m| m.get(task))
                                        .map(StateSync::from),
                                },
                            )
                        })
                        .collect()
                })
                .unwrap_or_default();
            (
                def.id.as_str().to_string(),
                StateRepoProject {
                    name: def.name.clone(),
                    repo: def.repo.as_str().to_string(),
                    stages: def.stages.clone(),
                    phases: Some(def.phases.clone()),
                    tasks,
                },
            )
        })
        .collect::<BTreeMap<_, _>>();

    StateFile {
        version: STATE_FILE_VERSION,
        projects,
        repo_projects: Some(Some(repo_projects)),
    }
}

/// `DomainState` 序列化後的狀態檔位元組；同一份內容永遠得到同樣的位元組（所有 map 都是 `BTreeMap`）。
fn serialize_state_file(domain: &DomainState) -> Vec<u8> {
    serde_json::to_vec_pretty(&to_state_file(domain))
        .expect("StateFile 只含字串與列舉，序列化不會失敗")
}

/// 零設定模式在第一次寫入時建立狀態檔所在資料夾（design D5）；`create` 為 `false` 時什麼都不做。
fn create_parent_if_needed(path: &Path, create: bool) -> std::io::Result<()> {
    match path.parent() {
        Some(parent) if create && !parent.as_os_str().is_empty() => std::fs::create_dir_all(parent),
        _ => Ok(()),
    }
}

/// 寫同目錄 `<檔名>.tmp`（含 `sync_all`）再 `rename` 取代目標；rename 失敗時盡力刪掉暫存檔。
fn write_atomically(path: &Path, contents: &[u8]) -> std::io::Result<()> {
    let mut tmp_name = path
        .file_name()
        .ok_or_else(|| std::io::Error::other("狀態檔路徑沒有檔名"))?
        .to_os_string();
    tmp_name.push(".tmp");
    let tmp_path = path.with_file_name(tmp_name);

    {
        let mut file = std::fs::File::create(&tmp_path)?;
        file.write_all(contents)?;
        file.sync_all()?;
    }
    if let Err(error) = std::fs::rename(&tmp_path, path) {
        let _ = std::fs::remove_file(&tmp_path);
        return Err(error);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use cockpit_core::{
        DomainState, Override, PaneId, PaneRepo, PaneRepos, ProjectDef, ProjectId, RepoKey,
        RepoProjectDef, RuntimeId, WorkstreamDef, WorkstreamId,
    };

    use super::BindingBasis;

    /// 手寫 project `h`（workstream `w`，沒有固定 pane）＋展開的 Repo Project `app`（工作線
    /// `local~wJ:p1` 固定綁定 `wJ:p1`）；兩條 workstream 都有一筆殘留覆蓋。
    fn domain_with_stale_overrides() -> DomainState {
        let config = ProjectDef {
            id: ProjectId::new("h"),
            name: "h".to_string(),
            stages: vec!["A".to_string()],
            workstreams: vec![WorkstreamDef {
                id: WorkstreamId::new("w"),
                name: "w".to_string(),
                binding: None,
                pinned_pane: None,
            }],
            tasks: Vec::new(),
            repo: None,
        };
        let mut domain = DomainState::from_projects(vec![config]);
        domain.repo_projects = vec![RepoProjectDef {
            id: ProjectId::new("app"),
            name: "app".to_string(),
            repo: RepoKey::new(r"d:\work\app\.git"),
            stages: vec!["Plan".to_string()],
            phases: vec![None],
        }];
        let mut pane_repos = PaneRepos::new();
        pane_repos.insert(
            (RuntimeId::new("local"), PaneId::new("wJ:p1")),
            PaneRepo {
                repo: RepoKey::new(r"d:\work\app\.git"),
                default_name: "app".to_string(),
                worktree: None,
                root: None,
            },
        );
        domain.pane_repos = pane_repos;
        domain.refresh_projects();
        let stale = Override {
            runtime: RuntimeId::new("local"),
            pane_id: PaneId::new("wJ:p2"),
        };
        domain
            .overrides
            .entry(ProjectId::new("app"))
            .or_default()
            .insert(WorkstreamId::new("local~wJ:p1"), stale.clone());
        domain
            .overrides
            .entry(ProjectId::new("h"))
            .or_default()
            .insert(WorkstreamId::new("w"), stale);
        domain
    }

    /// 固定 pane 的 workstream 有殘留覆蓋時，基於 `Pinned` 的重驗仍成立（core 的解析忽略覆蓋、
    /// 也不回報失效，覆蓋不會被清掉）；`Auto` 在同樣情況下不成立（review fix round 1）。
    #[test]
    fn pinned_basis_holds_despite_a_stale_override() {
        let domain = domain_with_stale_overrides();
        let app = ProjectId::new("app");
        let ws = WorkstreamId::new("local~wJ:p1");
        assert!(BindingBasis::Pinned.holds(&domain, &app, &ws));
        assert!(!BindingBasis::Auto.holds(&domain, &app, &ws));
    }

    /// `Pinned` 只對固定 pane 的 workstream 成立：手寫 workstream、不存在的 workstream 都不成立
    /// （例如判定後 Repo Project 被同 id 的手寫 project 取代）。
    #[test]
    fn pinned_basis_fails_for_workstreams_without_a_pinned_pane() {
        let domain = domain_with_stale_overrides();
        assert!(!BindingBasis::Pinned.holds(
            &domain,
            &ProjectId::new("h"),
            &WorkstreamId::new("w")
        ));
        assert!(!BindingBasis::Pinned.holds(
            &domain,
            &ProjectId::new("app"),
            &WorkstreamId::new("local~wJ:p9")
        ));
    }
}
