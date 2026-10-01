//! 狀態檔載入（design D1／D5；spec `pipeline-progress`「狀態檔載入與容錯」）。
//!
//! 只做讀取與容錯，把設定檔解出的 `Vec<ProjectDef>`（task 3.1 的 `config::load`）與（若有）既有
//! 狀態檔內容合併成一份 `cockpit_core::DomainState`。寫入（`.tmp` 再 `rename`、序列化）是
//! task 3.3 的職責；這裡另外定義「狀態檔 JSON ↔ 型別」的 serde 結構（[`StateFile`] 等），供 3.3
//! 直接重用序列化寫出，避免兩邊各自定義一份容易漂移的 JSON 形狀。
//!
//! 容錯規則（design D5）：
//! - 狀態檔不存在：所有 task 用初始進度、沒有覆蓋，**不建立檔案**。
//! - 檔案無法解析為狀態檔形狀、或 `version` 不是 1 或 2：視為損毀，回傳 [`ProgressError`]（呼叫端
//!   應視為啟動失敗；這是使用者手改檔案才會發生的情況，靜默丟棄會吞掉進度）。
//! - 狀態檔中的 project／task／workstream 在設定檔不存在，或覆蓋的 `runtime` 不是設定檔中的
//!   runtime：忽略該筆並以 `tracing::warn!` 記錄一則操作記錄，不進入回傳的 `DomainState`
//!   （下次寫入自然不再寫出）。
//! - task 的 `stage` 不在所屬 Project 的 `stages`：改用該 Project 設定檔的起始 stage、保留
//!   標記，並在回傳值的 `warnings[project_id]` 加入一則含 task id 與原 stage 值的訊息——這則要
//!   顯示給使用者（投影 `warnings`），跟前一條「忽略並 warn」的操作記錄不同層級。
//! - 設定檔中有、狀態檔中沒有的 task：用初始進度。
//! - v1 舊檔沒有 `active`（有就是損毀）、v2 每個 project 都必須有 `active`（缺就是損毀）；
//!   `active` 的無效項目（workstream／task 不存在、task 不屬於該 workstream、載入後標記不是
//!   none）忽略並 warn（progress-model task 3.1，design D5）。

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use cockpit_core::{
    DomainState, Mark, Override, PaneId, ProjectDef, ProjectId, RuntimeId, TaskId, TaskProgress,
    WorkstreamId,
};
use serde::{Deserialize, Serialize};

/// 狀態檔寫出時的 `version`（含目前 task 的 v2；progress-model task 3.1）。
pub const STATE_FILE_VERSION: u64 = 2;

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
    /// `version` 不是 1 或 [`STATE_FILE_VERSION`]。
    #[error("狀態檔版本不支援（{}）：期望 1 或 {STATE_FILE_VERSION}，收到 {version}", path.display())]
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
}

/// 單一 project 在狀態檔中的內容；`tasks`／`overrides` 均為必填（理由見 [`StateFile`]）。
///
/// `active`（workstream id → task id）是 `Option`：v1 檔沒有這個欄位、v2 檔必須有（可為空物件），
/// 兩種版本共用同一個結構，由載入時依 `version` 檢查有無（design D5；不用兩個 struct 加
/// `untagged`，免得錯誤訊息退化成「無法匹配任何變體」）。寫出一律 `Some`。
#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct StateProject {
    pub(crate) tasks: BTreeMap<String, StateTask>,
    pub(crate) overrides: BTreeMap<String, StateOverride>,
    pub(crate) active: Option<BTreeMap<String, String>>,
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

    let state_file: StateFile =
        serde_json::from_str(&text).map_err(|error| ProgressError::Parse {
            path: path.to_path_buf(),
            message: error.to_string(),
        })?;

    check_version_shape(path, &state_file)?;

    Ok(apply_state_file(
        path,
        projects,
        state_file,
        known_runtime_ids,
    ))
}

/// 依 `version` 檢查 `active` 欄位有無：v1 不得有、v2 每個 project 都必須有；其他版本不支援。
fn check_version_shape(path: &Path, state_file: &StateFile) -> Result<(), ProgressError> {
    let parse_error = |message: String| ProgressError::Parse {
        path: path.to_path_buf(),
        message,
    };
    match state_file.version {
        LEGACY_STATE_FILE_VERSION => {
            if let Some((id, _)) = state_file.projects.iter().find(|(_, p)| p.active.is_some()) {
                return Err(parse_error(format!(
                    "version 1 的狀態檔不應有 active 欄位（project {id}）"
                )));
            }
        }
        STATE_FILE_VERSION => {
            if let Some((id, _)) = state_file.projects.iter().find(|(_, p)| p.active.is_none()) {
                return Err(parse_error(format!(
                    "version 2 的狀態檔每個 project 都必須有 active 欄位（缺 project {id}）"
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
            state_project.active.as_ref(),
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
                warnings.push(format!(
                    "task {} 的 stage「{}」已不在 pipeline 的 stages 中，已退回起始 stage「{}」",
                    task.id, state_task.stage, task.stage
                ));
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
                    active: Some(BTreeMap::new()),
                },
            )]),
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
            json.contains("\"active\":{}"),
            "空 active 應明確輸出為 {{}}，不能被省略：{json}"
        );

        let round_tripped: StateFile =
            serde_json::from_str(&json).expect("必填欄位變更後仍應能讀回自己寫出的格式");
        assert_eq!(round_tripped.version, STATE_FILE_VERSION);
        assert!(round_tripped.projects.contains_key("p"));
    }
}
