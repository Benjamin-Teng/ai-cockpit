//! 狀態檔載入（design D1／D5；spec `pipeline-progress`「狀態檔載入與容錯」）。
//!
//! 只做讀取與容錯，把設定檔解出的 `Vec<ProjectDef>`（task 3.1 的 `config::load`）與（若有）既有
//! 狀態檔內容合併成一份 `cockpit_core::DomainState`。寫入（`.tmp` 再 `rename`、序列化）是
//! task 3.3 的職責；這裡另外定義「狀態檔 JSON ↔ 型別」的 serde 結構（[`StateFile`] 等），供 3.3
//! 直接重用序列化寫出，避免兩邊各自定義一份容易漂移的 JSON 形狀。
//!
//! 容錯規則（design D5）：
//! - 狀態檔不存在：所有 task 用初始進度、沒有覆蓋，**不建立檔案**。
//! - 檔案無法解析為狀態檔形狀、或 `version` 不是 1～4：視為損毀，回傳 [`ProgressError`]（呼叫端
//!   應視為啟動失敗；這是使用者手改檔案才會發生的情況，靜默丟棄會吞掉進度）。
//! - 狀態檔中的 project／task／workstream 在設定檔不存在，或覆蓋的 `runtime` 不是設定檔中的
//!   runtime：忽略該筆並以 `tracing::warn!` 記錄一則操作記錄，不進入回傳的 `DomainState`
//!   （下次寫入自然不再寫出）。
//! - task 的 `stage` 不在所屬 Project 的 `stages`：改用該 Project 設定檔的起始 stage、保留
//!   標記，並在回傳值的 `warnings[project_id]` 加入一則含 task id 與原 stage 值的訊息——這則要
//!   顯示給使用者（投影 `warnings`），跟前一條「忽略並 warn」的操作記錄不同層級。
//! - 設定檔中有、狀態檔中沒有的 task：用初始進度。
//! - v1 舊檔沒有 `active`（有就是損毀，含 `null`）、v2 每個 project 都必須有 `active` 物件（缺或 `null` 就是損毀）；
//!   `active` 的無效項目（workstream／task 不存在、task 不屬於該 workstream、載入後標記不是
//!   none）忽略並 warn（progress-model task 3.1，design D5）。
//! - v3（repo-projects task 4.1，design D5）：`projects` 同 v2，另有必填的 `repo_projects`（可為空物件）；v1、v2
//!   出現 `repo_projects`（含 `null`）即損毀。Repo Project 的 id／名稱／stages 不合 D6 規則、或兩個 Repo Project
//!   的 `repo` 相同即損毀；task 的 stage 不在 stages → 第一個 stage＋warning；task id 的 runtime（最後一個 `~`
//!   之前）不是設定中的 runtime → 忽略並 warn。Repo Project 的 task 全部讀入，不經 [`resolve_tasks`]（D4）。
//! - v4（openspec-stage-sync task 4.1，design D4、D10-3）：每個 Repo Project 另有必填的 `phases`（與 `stages` 對齊，
//!   長度不符或非 `null` 值重複即損毀），Repo Project 的 task 可帶 `sync`。v1～v3 出現 `phases` 或 `sync`（含 `null`）、
//!   手寫 `projects` 底下的 task 帶 `sync` 都是損毀。讀 v1～v3 時依預設站名一次性補 `phases`（[`default_phases`]），
//!   v4 的 `null` 一律尊重。

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use cockpit_core::{
    DomainState, Mark, Message, Observation, OpenSpecPhase, Override, PaneId, ProjectDef,
    ProjectId, RepoKey, RepoProjectDef, RuntimeId, SyncMode, TaskId, TaskProgress, TaskSync,
    WorkstreamId, is_valid_repo_project_id, normalize_repo_project_name,
    normalize_repo_project_stages, repo_project_phases_valid, split_pane_item_id,
};
use serde::{Deserialize, Serialize};

/// 狀態檔寫出時的 `version`（Repo Project 加 `phases`、task 加 `sync` 的 v4；openspec-stage-sync task 4.1，design D4）。
pub const STATE_FILE_VERSION: u64 = 4;

/// 加上 `repo_projects`、沒有 `phases` 與 `sync` 的舊版（repo-projects task 4.1，design D5）。
const V3_STATE_FILE_VERSION: u64 = 3;

/// 含目前 task、沒有 `repo_projects` 的舊版（progress-model task 3.1）。
const V2_STATE_FILE_VERSION: u64 = 2;

/// 仍可讀取的舊版 `version`：沒有 `active` 欄位，載入後所有 workstream 沒有目前 task。
const LEGACY_STATE_FILE_VERSION: u64 = 1;

/// 狀態檔載入失敗的原因；訊息一律含狀態檔路徑（spec `pipeline-progress`「狀態檔載入與容錯」：
/// 「啟動失敗，訊息含狀態檔路徑與原因」）。
#[derive(Debug, thiserror::Error)]
pub enum ProgressError {
    /// 檔案存在但讀取失敗（權限、I/O 等）；不含 `NotFound`——那視為「還沒有狀態檔」，不是錯誤。
    #[error("讀取狀態檔失敗（{}）：{source}", path.display())]
    Read {
        /// 讀取失敗的路徑。
        path: PathBuf,
        /// 底層 I/O 錯誤。
        #[source]
        source: std::io::Error,
    },
    /// JSON 語法錯誤，或形狀對不上（含 `mark` 不是 `none`／`completed`／`failed` 這類欄位值不
    /// 合法——同樣視為損毀，design D5）。
    #[error("解析狀態檔失敗（{}）：{message}", path.display())]
    Parse {
        /// 出錯的檔案路徑。
        path: PathBuf,
        /// 底層解析器的錯誤訊息。
        message: String,
    },
    /// `version` 不是 1 到 [`STATE_FILE_VERSION`]。
    #[error("狀態檔版本不支援（{}）：期望 1 到 {STATE_FILE_VERSION}，收到 {version}", path.display())]
    UnsupportedVersion {
        /// 出錯的檔案路徑。
        path: PathBuf,
        /// 狀態檔實際的 `version` 值。
        version: u64,
    },
}

/// 狀態檔完整形狀（spec `pipeline-progress`「狀態檔格式與持久化」）：
/// `{"version": 1, "projects": {"<pid>": {"tasks": {...}, "overrides": {...}}}}`。
/// `deny_unknown_fields`：狀態檔只由這個程式自己寫出，手改壞掉的欄位一律視為損毀（同 D5）。
///
/// `projects`／[`StateProject::tasks`]／[`StateProject::overrides`] 三個欄位刻意**不**
/// `#[serde(default)]`：這三個欄位在正常寫出的狀態檔裡永遠存在（即使值是空物件 `{}`），
/// 缺欄位只會是手改或半份寫壞的檔案，必須當成「無法解析為上述形狀」讓啟動失敗，不能悄悄
/// 補成空集合——那等同把已存在的進度／覆蓋靜默重設，下次寫入就永久遺失（Codex fix round 1
/// finding，對照 spec `pipeline-progress`「檔案無法解析為上述形狀…→ 啟動失敗」）。空集合
/// 本身仍要能通過驗證，只是要在 JSON 裡明確寫成 `{}`，不能整個欄位不見。
///
/// 三個 map 欄位都用 `BTreeMap`（不是 `HashMap`）：序列化時依 key 字典序輸出，同一份
/// `DomainState` 內容不論寫幾次、也不論當次 `HashMap` 雜湊種子為何，位元組都完全相同
/// （M4 fix；`HashMap` 的雜湊種子在同一程式內每次建置都會變，寫出的狀態檔 key 順序會跟著變，
/// 讓部署／備份拿雜湊比對「內容有沒有變」全部失真）。
#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct StateFile {
    pub(crate) version: u64,
    pub(crate) projects: BTreeMap<String, StateProject>,
    /// 畫面加入的 Repo Project（v3；repo-projects task 4.1，design D5）。雙層 `Option` 的理由同
    /// [`StateProject::active`]：缺席＝`None`（v1、v2 必須如此）、`null`＝`Some(None)`（任何版本都損毀）、
    /// 有值＝`Some(Some(_))`（v3 必須如此）。寫出一律 `Some(Some(_))`。
    #[serde(
        default,
        deserialize_with = "deserialize_present_repo_projects",
        skip_serializing_if = "Option::is_none"
    )]
    pub(crate) repo_projects: Option<Option<BTreeMap<String, StateRepoProject>>>,
}

/// 單一 Repo Project 在狀態檔中的內容（design D5）：`name`／`repo`／`stages`／`tasks` 必填；不保存 `overrides` 與
/// `active`（`deny_unknown_fields` 讓它們出現即損毀）。
///
/// `phases`（v4；openspec-stage-sync task 4.1，design D4）：v3 必須缺席、v4 必須有，由 [`check_version_shape`] 依
/// `version` 檢查。出現但值為 `null`（任何版本）一律損毀，所以用 [`deserialize_non_null`]，不讓 `null` 被
/// `Option` 收成缺席。寫出一律有值。
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct StateRepoProject {
    pub(crate) name: String,
    pub(crate) repo: String,
    pub(crate) stages: Vec<String>,
    #[serde(
        default,
        deserialize_with = "deserialize_non_null",
        skip_serializing_if = "Option::is_none"
    )]
    pub(crate) phases: Option<Vec<Option<OpenSpecPhase>>>,
    pub(crate) tasks: BTreeMap<String, StateRepoTask>,
}

/// 欄位出現就必須是非 `null` 的 `T`；缺席由 `#[serde(default)]` 給 `None`。
fn deserialize_non_null<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    T::deserialize(deserializer).map(Some)
}

/// Repo Project 底下單一 task 在狀態檔中的內容：手寫 project 的 [`StateTask`] 加選填的 `sync`
/// （v4；`sync` 在 v1～v3 出現即損毀，見 [`check_version_shape`]；`null` 同樣損毀）。
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct StateRepoTask {
    pub(crate) stage: String,
    pub(crate) mark: Mark,
    #[serde(
        default,
        deserialize_with = "deserialize_non_null",
        skip_serializing_if = "Option::is_none"
    )]
    pub(crate) sync: Option<StateSync>,
}

/// task 的同步狀態（openspec-stage-sync task 4.1，design D4）：`mode` 與 `applied` 都必填，`applied` 可為 `null`。
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct StateSync {
    pub(crate) mode: SyncMode,
    /// 刻意沒有 `default`：欄位缺席與 `null` 要分得開，缺席視為損毀（spec 形狀 `applied: {...} | null` 必填）。
    #[serde(deserialize_with = "deserialize_required_option")]
    pub(crate) applied: Option<StateObservation>,
}

/// 欄位必須出現，值可為 `null`；缺席時因為沒有 `#[serde(default)]` 而報 `missing field`。
fn deserialize_required_option<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer)
}

/// [`Observation`] 在狀態檔中的形狀；另立型別是為了 `deny_unknown_fields`。
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct StateObservation {
    pub(crate) change: String,
    pub(crate) phase: OpenSpecPhase,
    pub(crate) checked: u32,
    pub(crate) total: u32,
}

impl From<&Observation> for StateObservation {
    fn from(obs: &Observation) -> Self {
        Self {
            change: obs.change.clone(),
            phase: obs.phase,
            checked: obs.checked,
            total: obs.total,
        }
    }
}

impl From<StateObservation> for Observation {
    fn from(obs: StateObservation) -> Self {
        Self {
            change: obs.change,
            phase: obs.phase,
            checked: obs.checked,
            total: obs.total,
        }
    }
}

impl From<&TaskSync> for StateSync {
    fn from(sync: &TaskSync) -> Self {
        Self {
            mode: sync.mode,
            applied: sync.applied.as_ref().map(StateObservation::from),
        }
    }
}

impl From<StateSync> for TaskSync {
    fn from(sync: StateSync) -> Self {
        Self {
            mode: sync.mode,
            applied: sync.applied.map(Observation::from),
        }
    }
}

/// 同 [`deserialize_present_active`]：欄位出現（含 `null`）就包成 `Some`。
fn deserialize_present_repo_projects<'de, D>(
    deserializer: D,
) -> Result<Option<Option<BTreeMap<String, StateRepoProject>>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Option::<BTreeMap<String, StateRepoProject>>::deserialize(deserializer).map(Some)
}

/// 單一 project 在狀態檔中的內容；`tasks`／`overrides` 均為必填（理由見 [`StateFile`]）。
///
/// `active`（workstream id → task id）是雙層 `Option`：v1 檔沒有這個欄位、v2 檔必須有（可為空物件），
/// 兩種版本共用同一個結構，由載入時依 `version` 檢查有無（design D5；不用兩個 struct 加
/// `untagged`，免得錯誤訊息退化成「無法匹配任何變體」）。寫出一律 `Some(Some(_))`。
///
/// 雙層是為了分出欄位缺席與 `null`（ui-fixes task 3.4，design D9）：缺席＝`None`、`null`＝`Some(None)`、
/// 有值＝`Some(Some(_))`。單層 `Option` 會把 `null` 與缺席都收成 `None`，v1 的 `"active": null`
/// 就被誤放行。`default` 不可省：只加 `deserialize_with` 時缺席會報 `missing field`，合法的 v1 舊檔
/// 反被誤判為損毀。
#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct StateProject {
    pub(crate) tasks: BTreeMap<String, StateTask>,
    pub(crate) overrides: BTreeMap<String, StateOverride>,
    #[serde(default, deserialize_with = "deserialize_present_active")]
    pub(crate) active: Option<Option<BTreeMap<String, String>>>,
}

/// 欄位出現（含 `null`）就包成 `Some`；缺席時由 `#[serde(default)]` 給 `None`。
fn deserialize_present_active<'de, D>(
    deserializer: D,
) -> Result<Option<Option<BTreeMap<String, String>>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Option::<BTreeMap<String, String>>::deserialize(deserializer).map(Some)
}

/// 單一 task 在狀態檔中的進度。
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct StateTask {
    pub(crate) stage: String,
    pub(crate) mark: Mark,
}

/// 單一 workstream 在狀態檔中的畫面覆蓋。
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct StateOverride {
    pub(crate) runtime: String,
    pub(crate) pane_id: String,
}

/// 載入狀態檔，套用到設定檔解析出的 `projects` 上，回傳完整的 `DomainState`（spec
/// `pipeline-progress`「狀態檔載入與容錯」；design D5，容錯規則見本檔模組文件）。
///
/// `known_runtime_ids` 是設定檔目前登記的 runtime id 集合（`cockpit::config::Config::runtimes`
/// 的 `id`），用來判斷覆蓋的 `runtime` 是否已知；這裡不檢查該 runtime 是否連線、pane 是否存在
/// ——那是使用中設定新覆蓋（HTTP 端點）才做的 `validate_override`，載入既有狀態檔時原樣沿用
/// 舊覆蓋，留給投影／解析階段判斷是否失效。
///
/// # Errors
///
/// 檔案存在但讀取失敗、無法解析為狀態檔形狀、或 `version` 不是 [`STATE_FILE_VERSION`] 時回傳
/// [`ProgressError`]。
pub fn load_progress(
    path: &Path,
    projects: Vec<ProjectDef>,
    known_runtime_ids: &HashSet<&str>,
) -> Result<DomainState, ProgressError> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => {
            return Ok(DomainState::from_projects(projects));
        }
        Err(source) => {
            return Err(ProgressError::Read {
                path: path.to_path_buf(),
                source,
            });
        }
    };

    let mut state_file: StateFile =
        serde_json::from_str(&text).map_err(|error| ProgressError::Parse {
            path: path.to_path_buf(),
            message: error.to_string(),
        })?;

    check_version_shape(path, &state_file)?;
    let repo_projects = state_file
        .repo_projects
        .take()
        .flatten()
        .unwrap_or_default();
    let repo = resolve_repo_projects(path, repo_projects, known_runtime_ids)?;

    let mut domain = apply_state_file(path, projects, state_file, known_runtime_ids);
    // 被同 id 手寫 project 隱藏的 Repo Project 沒有投影；它的 stage 警告不掛到手寫 project 上（兩邊互不影響），
    // 只留操作記錄。
    for (project, list) in repo.warnings {
        if domain.projects.iter().any(|p| p.id == project) {
            tracing::warn!(
                path = %path.display(),
                project = %project,
                warnings = ?list,
                "被同 id 手寫 project 隱藏的 Repo Project 載入時有 stage 警告",
            );
        } else {
            domain.warnings.entry(project).or_default().extend(list);
        }
    }
    domain.repo_projects = repo.defs;
    domain.repo_progress = repo.progress;
    domain.repo_sync = repo.sync;
    domain.refresh_projects();
    Ok(domain)
}

/// [`resolve_repo_projects`] 的結果。
struct ResolvedRepoProjects {
    defs: Vec<RepoProjectDef>,
    progress: HashMap<ProjectId, HashMap<TaskId, TaskProgress>>,
    sync: HashMap<ProjectId, HashMap<TaskId, TaskSync>>,
    warnings: HashMap<ProjectId, Vec<String>>,
}

/// 解出 `repo_projects` 區段（design D4、D5）：定義不合 D6 規則或 `repo` 重複 → 損毀；task 全部讀入（不經
/// [`resolve_tasks`]、不以目前展開的 task 過濾），stage 不在 stages → 第一個 stage＋warning，runtime（task id
/// 最後一個 `~` 之前）不是設定中的 runtime → 忽略並 warn。
fn resolve_repo_projects(
    path: &Path,
    repo_projects: BTreeMap<String, StateRepoProject>,
    known_runtime_ids: &HashSet<&str>,
) -> Result<ResolvedRepoProjects, ProgressError> {
    let corrupt = |message: String| ProgressError::Parse {
        path: path.to_path_buf(),
        message,
    };
    let mut resolved = ResolvedRepoProjects {
        defs: Vec::with_capacity(repo_projects.len()),
        progress: HashMap::new(),
        sync: HashMap::new(),
        warnings: HashMap::new(),
    };
    let mut seen_repos: HashMap<String, String> = HashMap::new();

    for (id, entry) in repo_projects {
        if !is_valid_repo_project_id(&id) {
            return Err(corrupt(format!(
                "repo_projects 的 id 不合法（{id:?}；只能由英數字、_、- 組成）"
            )));
        }
        let name = normalize_repo_project_name(&entry.name).ok_or_else(|| {
            corrupt(format!(
                "repo_projects.{id} 的 name 不合法（去除前後空白後須為 1～64 個字元、不含控制字元或不可見的格式字元）"
            ))
        })?;
        let stages = normalize_repo_project_stages(&entry.stages).ok_or_else(|| {
            corrupt(format!(
                "repo_projects.{id} 的 stages 不合法（須為 1～12 個互不相同、去除前後空白後 1～32 個字元且不含控制字元或不可見格式字元的名稱）"
            ))
        })?;
        if let Some(other) = seen_repos.insert(entry.repo.clone(), id.clone()) {
            return Err(corrupt(format!(
                "repo_projects.{other} 與 repo_projects.{id} 的 repo 相同（{}）",
                entry.repo
            )));
        }

        // v4 照檔案載入（`null` 一律尊重）；沒有 `phases` 的 v1～v3 依站名一次性補預設對應
        // （openspec-stage-sync task 4.1，design D4）。`check_version_shape` 已保證 v4 有 `phases`、v3 沒有。
        let phases = match entry.phases {
            Some(phases) => {
                if !repo_project_phases_valid(&stages, &phases) {
                    return Err(corrupt(format!(
                        "repo_projects.{id} 的 phases 不合法（長度須等於 stages，且非 null 的值不可重複）"
                    )));
                }
                phases
            }
            None => default_phases(&stages),
        };

        let project_id = ProjectId::new(id.clone());
        let mut tasks = HashMap::with_capacity(entry.tasks.len());
        let mut task_sync = HashMap::new();
        for (task_id, state_task) in entry.tasks {
            let runtime_known = split_pane_item_id(&task_id)
                .is_some_and(|(runtime, _)| known_runtime_ids.contains(runtime.as_str()));
            if !runtime_known {
                tracing::warn!(
                    path = %path.display(),
                    project = %id,
                    task = %task_id,
                    "狀態檔中 Repo Project 的 task 指向設定檔沒有的 runtime，忽略",
                );
                continue;
            }
            let stage_reset = !stages.contains(&state_task.stage);
            let stage = if stage_reset {
                resolved
                    .warnings
                    .entry(project_id.clone())
                    .or_default()
                    .push(
                        Message::TaskStageReset {
                            task: task_id.clone(),
                            stage: state_task.stage,
                            start: stages[0].clone(),
                        }
                        .text(),
                    );
                stages[0].clone()
            } else {
                state_task.stage
            };
            let task_id = TaskId::new(task_id);
            if let Some(sync) = state_task.sync {
                let mut sync = TaskSync::from(sync);
                // 載入時 stage 已不存在而退回第一站：auto 的 `applied` 是「上次套用到哪一站」的紀錄，已經與卡片實際所在
                // 的站對不起來；留著的話偵測結果不變時（`applied` 與偵測結果相等）不再套用，卡片永久停在第一站。
                // 清成 `None` 讓下一輪偵測重新套用。manual 保留不動（手動優先，design D4／D10-3；
                // openspec-stage-sync task 7.2）。
                if stage_reset && sync.mode == SyncMode::Auto {
                    sync.applied = None;
                }
                task_sync.insert(task_id.clone(), sync);
            }
            tasks.insert(
                task_id,
                TaskProgress {
                    stage,
                    mark: state_task.mark,
                },
            );
        }
        if !tasks.is_empty() {
            resolved.progress.insert(project_id.clone(), tasks);
        }
        if !task_sync.is_empty() {
            resolved.sync.insert(project_id.clone(), task_sync);
        }
        resolved.defs.push(RepoProjectDef {
            id: project_id,
            name,
            repo: RepoKey::new(entry.repo),
            phases,
            stages,
        });
    }
    Ok(resolved)
}

/// 讀 v1～v3 檔時依站名補預設階段對應（openspec-stage-sync task 4.1，design D4）：站名完全等於 規劃／Plan → plan、
/// 實作／Implement → implement、審查／Review → review、完成／Complete → complete，其餘 `None`；同一個階段被多個
/// stage 取得時只保留 stage 順序中最先出現的一個，後出現的改為 `None`。
fn default_phases(stages: &[String]) -> Vec<Option<OpenSpecPhase>> {
    let mut taken: HashSet<OpenSpecPhase> = HashSet::new();
    stages
        .iter()
        .map(|stage| {
            let phase = match stage.as_str() {
                "規劃" | "Plan" => OpenSpecPhase::Plan,
                "實作" | "Implement" => OpenSpecPhase::Implement,
                "審查" | "Review" => OpenSpecPhase::Review,
                "完成" | "Complete" => OpenSpecPhase::Complete,
                _ => return None,
            };
            taken.insert(phase).then_some(phase)
        })
        .collect()
}

/// 依 `version` 檢查各版形狀（design D5、D10-3）。其他版本不支援。
///
/// - `active`：v1 不得有（含 `null`）；v2～v4 每個 project 都必須有非 `null` 的 `active`。
/// - `repo_projects`：v1、v2 不得有（含 `null`）；v3、v4 必須有非 `null` 的 `repo_projects`。
/// - `phases`（Repo Project）：v3 以前不得有；v4 每個 Repo Project 都必須有。
/// - `sync`（Repo Project 的 task）：v4 才可有；v3 以前出現即損毀。手寫 `projects` 底下的 task 帶 `sync` 由
///   [`StateTask`] 的 `deny_unknown_fields` 擋下（任何版本）。
fn check_version_shape(path: &Path, state_file: &StateFile) -> Result<(), ProgressError> {
    let parse_error = |message: String| ProgressError::Parse {
        path: path.to_path_buf(),
        message,
    };
    let version = state_file.version;
    match version {
        LEGACY_STATE_FILE_VERSION => {
            if let Some((id, _)) = state_file.projects.iter().find(|(_, p)| p.active.is_some()) {
                return Err(parse_error(format!(
                    "version 1 的狀態檔不應有 active 欄位（project {id}）"
                )));
            }
        }
        V2_STATE_FILE_VERSION | V3_STATE_FILE_VERSION | STATE_FILE_VERSION => {
            if let Some((id, _)) = state_file
                .projects
                .iter()
                .find(|(_, p)| !matches!(p.active, Some(Some(_))))
            {
                return Err(parse_error(format!(
                    "version {version} 的狀態檔每個 project 的 active 都必須是物件（project {id} 缺少該欄位或為 null）"
                )));
            }
        }
        version => {
            return Err(ProgressError::UnsupportedVersion {
                path: path.to_path_buf(),
                version,
            });
        }
    }

    let repo_projects = match (&state_file.repo_projects, version >= V3_STATE_FILE_VERSION) {
        (Some(Some(repo_projects)), true) => Some(repo_projects),
        (None, false) => None,
        (_, true) => {
            return Err(parse_error(format!(
                "version {version} 的狀態檔必須有 repo_projects 物件（缺少該欄位或為 null）"
            )));
        }
        (Some(_), false) => {
            return Err(parse_error(format!(
                "version {version} 的狀態檔不應有 repo_projects 欄位"
            )));
        }
    };

    let is_v4 = version >= STATE_FILE_VERSION;
    for (id, repo_project) in repo_projects.into_iter().flatten() {
        match (&repo_project.phases, is_v4) {
            (Some(_), true) | (None, false) => {}
            (None, true) => {
                return Err(parse_error(format!(
                    "version {version} 的狀態檔每個 Repo Project 都必須有 phases（repo_projects.{id} 缺少該欄位）"
                )));
            }
            (Some(_), false) => {
                return Err(parse_error(format!(
                    "version {version} 的狀態檔不應有 phases 欄位（repo_projects.{id}）"
                )));
            }
        }
        if !is_v4
            && let Some((task_id, _)) = repo_project.tasks.iter().find(|(_, t)| t.sync.is_some())
        {
            return Err(parse_error(format!(
                "version {version} 的狀態檔不應有 sync 欄位（repo_projects.{id} 的 task {task_id}）"
            )));
        }
    }
    Ok(())
}

/// 把解析成功的狀態檔套到 `projects` 上；純函數（不再碰檔案系統），容錯規則見模組文件。
fn apply_state_file(
    path: &Path,
    projects: Vec<ProjectDef>,
    mut state_file: StateFile,
    known_runtime_ids: &HashSet<&str>,
) -> DomainState {
    let known_project_ids: HashSet<&str> = projects.iter().map(|p| p.id.as_str()).collect();
    for unknown_project_id in state_file
        .projects
        .keys()
        .filter(|id| !known_project_ids.contains(id.as_str()))
    {
        tracing::warn!(
            path = %path.display(),
            project = %unknown_project_id,
            "狀態檔含設定檔沒有的 project，忽略",
        );
    }

    let mut progress: HashMap<ProjectId, HashMap<TaskId, TaskProgress>> =
        HashMap::with_capacity(projects.len());
    let mut overrides: HashMap<ProjectId, HashMap<WorkstreamId, Override>> = HashMap::new();
    let mut active: HashMap<ProjectId, HashMap<WorkstreamId, TaskId>> = HashMap::new();
    let mut warnings: HashMap<ProjectId, Vec<String>> = HashMap::new();

    for project in &projects {
        let state_project = state_file
            .projects
            .remove(project.id.as_str())
            .unwrap_or_default();

        let (task_progress, project_warnings) = resolve_tasks(project, &state_project.tasks);
        if !project_warnings.is_empty() {
            warnings.insert(project.id.clone(), project_warnings);
        }
        progress.insert(project.id.clone(), task_progress);

        let project_overrides =
            resolve_overrides(path, project, &state_project.overrides, known_runtime_ids);

        // 覆蓋在本次載入被忽略的 workstream：綁定已退回自動，目前 task 不再可信（review M3）。
        let dropped_overrides: HashSet<&str> = state_project
            .overrides
            .keys()
            .filter(|id| !project_overrides.contains_key(&WorkstreamId::new(id.as_str())))
            .map(String::as_str)
            .collect();
        let project_active = resolve_active(
            path,
            project,
            state_project.active.as_ref().and_then(Option::as_ref),
            &progress[&project.id],
            &dropped_overrides,
        );
        if !project_overrides.is_empty() {
            overrides.insert(project.id.clone(), project_overrides);
        }
        if !project_active.is_empty() {
            active.insert(project.id.clone(), project_active);
        }
    }

    DomainState {
        projects,
        progress,
        overrides,
        active,
        warnings,
        // Repo Project 由 `load_progress` 另外解出後補上（repo-projects task 4.1）。
        ..DomainState::default()
    }
}

/// 解出一個 project 底下每個 task 的進度：狀態檔中有紀錄且 `stage` 合法就照用；`stage` 不在
/// `project.stages` 就退回起始 stage、保留標記並產生一則使用者可見的 warning；狀態檔中沒有
/// 這個 task 就用初始進度。狀態檔中多出、設定檔沒有的 task id 一併記錄（操作記錄，不進
/// `DomainState.warnings`）。
fn resolve_tasks(
    project: &ProjectDef,
    state_tasks: &BTreeMap<String, StateTask>,
) -> (HashMap<TaskId, TaskProgress>, Vec<String>) {
    let mut progress = HashMap::with_capacity(project.tasks.len());
    let mut warnings = Vec::new();

    for task in &project.tasks {
        let resolved = match state_tasks.get(task.id.as_str()) {
            None => TaskProgress::initial(task),
            Some(state_task) if project.stages.iter().any(|s| s == &state_task.stage) => {
                TaskProgress {
                    stage: state_task.stage.clone(),
                    mark: state_task.mark,
                }
            }
            Some(state_task) => {
                warnings.push(
                    Message::TaskStageReset {
                        task: task.id.to_string(),
                        stage: state_task.stage.clone(),
                        start: task.stage.clone(),
                    }
                    .text(),
                );
                TaskProgress {
                    stage: task.stage.clone(),
                    mark: state_task.mark,
                }
            }
        };
        progress.insert(task.id.clone(), resolved);
    }

    let known_task_ids: HashSet<&str> = project.tasks.iter().map(|t| t.id.as_str()).collect();
    for unknown_task_id in state_tasks
        .keys()
        .filter(|id| !known_task_ids.contains(id.as_str()))
    {
        tracing::warn!(
            project = %project.id,
            task = %unknown_task_id,
            "狀態檔含設定檔沒有的 task，忽略",
        );
    }

    (progress, warnings)
}

/// 解出一個 project 底下的畫面覆蓋：`workstream` 不存在於設定檔、或 `runtime` 不在
/// `known_runtime_ids` 都忽略並記錄操作記錄；其餘照用。
fn resolve_overrides(
    path: &Path,
    project: &ProjectDef,
    state_overrides: &BTreeMap<String, StateOverride>,
    known_runtime_ids: &HashSet<&str>,
) -> HashMap<WorkstreamId, Override> {
    let known_workstream_ids: HashSet<&str> =
        project.workstreams.iter().map(|w| w.id.as_str()).collect();

    let mut overrides = HashMap::new();
    for (workstream_id, state_override) in state_overrides {
        if !known_workstream_ids.contains(workstream_id.as_str()) {
            tracing::warn!(
                path = %path.display(),
                project = %project.id,
                workstream = %workstream_id,
                "狀態檔的覆蓋指向設定檔沒有的 workstream，忽略",
            );
            continue;
        }
        if !known_runtime_ids.contains(state_override.runtime.as_str()) {
            tracing::warn!(
                path = %path.display(),
                project = %project.id,
                workstream = %workstream_id,
                runtime = %state_override.runtime,
                "狀態檔的覆蓋指向設定檔沒有的 runtime，忽略",
            );
            continue;
        }

        overrides.insert(
            WorkstreamId::new(workstream_id.clone()),
            Override {
                runtime: RuntimeId::new(state_override.runtime.clone()),
                pane_id: PaneId::new(state_override.pane_id.clone()),
            },
        );
    }

    overrides
}

/// 解出一個 project 底下的目前 task：workstream 或 task 不存在、task 不屬於該 workstream、或 task
/// 載入後標記不是 `none`、或該 workstream 的覆蓋在本次載入被忽略（`dropped_overrides`）都忽略並
/// 記錄操作記錄；其餘照用。`state_active` 為 `None`（v1 檔）時
/// 沒有任何目前 task。
fn resolve_active(
    path: &Path,
    project: &ProjectDef,
    state_active: Option<&BTreeMap<String, String>>,
    progress: &HashMap<TaskId, TaskProgress>,
    dropped_overrides: &HashSet<&str>,
) -> HashMap<WorkstreamId, TaskId> {
    let mut active = HashMap::new();
    for (workstream_id, task_id) in state_active.into_iter().flatten() {
        let workstream_known = project
            .workstreams
            .iter()
            .any(|w| w.id.as_str() == workstream_id);
        let task = project.tasks.iter().find(|t| t.id.as_str() == task_id);
        let reason = match task {
            _ if !workstream_known => "workstream 不在設定檔中",
            None => "task 不在設定檔中",
            Some(task) if task.workstream.as_str() != workstream_id => "task 不屬於該 workstream",
            Some(task) if progress.get(&task.id).is_some_and(|p| p.mark != Mark::None) => {
                "task 已有標記"
            }
            Some(_) if dropped_overrides.contains(workstream_id.as_str()) => {
                "workstream 的覆蓋已被忽略，綁定退回自動"
            }
            Some(_) => {
                active.insert(
                    WorkstreamId::new(workstream_id.clone()),
                    TaskId::new(task_id.clone()),
                );
                continue;
            }
        };
        tracing::warn!(
            path = %path.display(),
            project = %project.id,
            workstream = %workstream_id,
            task = %task_id,
            reason,
            "狀態檔的目前 task 無效，忽略",
        );
    }
    active
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Codex fix round 1：`tasks`／`overrides`／`projects` 改成必填後，確認序列化空集合仍會
    /// 明確輸出 `{}`（不是被省略），且能讀回自己剛寫出的格式——task 3.3 的寫入服務要靠這個
    /// 保證往返一致，不然「必填」反而會讓自己寫出的檔案讀不回來。
    #[test]
    fn empty_collections_serialize_as_explicit_empty_objects_and_round_trip() {
        let state = StateFile {
            version: STATE_FILE_VERSION,
            projects: BTreeMap::from([(
                "p".to_string(),
                StateProject {
                    tasks: BTreeMap::new(),
                    overrides: BTreeMap::new(),
                    active: Some(Some(BTreeMap::new())),
                },
            )]),
            repo_projects: Some(Some(BTreeMap::new())),
        };

        let json = serde_json::to_string(&state).expect("序列化應成功");

        assert!(
            json.contains("\"tasks\":{}"),
            "空 tasks 應明確輸出為 {{}}，不能被省略：{json}"
        );
        assert!(
            json.contains("\"overrides\":{}"),
            "空 overrides 應明確輸出為 {{}}，不能被省略：{json}"
        );
        assert!(
            json.contains("\"repo_projects\":{}"),
            "空 repo_projects 應明確輸出為 {{}}：{json}"
        );
        assert!(
            json.contains("\"active\":{}"),
            "空 active 應明確輸出為 {{}}，不能被省略：{json}"
        );

        let round_tripped: StateFile =
            serde_json::from_str(&json).expect("必填欄位變更後仍應能讀回自己寫出的格式");
        assert_eq!(round_tripped.version, STATE_FILE_VERSION);
        assert!(round_tripped.projects.contains_key("p"));
    }
}
