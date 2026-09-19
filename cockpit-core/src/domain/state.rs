//! `DomainState`：`StoreHandle` 內與 `RuntimeStore` 並列的 Domain 層狀態（design D2）。
//! `Vec<ProjectDef>` 加每個 Task 的進度、每條 Workstream 的覆蓋、載入時的 warnings。
//! 狀態檔讀寫（含容錯與 warning 產生）屬於 `cockpit` 的 `progress.rs`（design D1）；這裡只提供
//! 「沒有狀態檔」時的初始建構——每個 Task 的初始進度＝起始 Stage＋`Mark::None`
//! （spec `pipeline-domain` 「初始進度」）。

use std::collections::HashMap;

use crate::domain::binding::Override;
use crate::domain::config::ProjectDef;
use crate::domain::ids::{ProjectId, TaskId, WorkstreamId};
use crate::domain::progress::TaskProgress;

/// `cockpit-core` 的 Domain 層完整狀態：Project 結構、各 Task 的進度、各 Workstream 的覆蓋、
/// 載入時的 warnings（依 project 分組）。`Default` 是「沒有任何 Project」的狀態（spec
/// `state-projection` 「沒有 Project」）。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DomainState {
    /// 所有 Project，依設定檔順序。
    pub projects: Vec<ProjectDef>,
    /// 每個 Project 內每個 Task 目前的進度。
    pub progress: HashMap<ProjectId, HashMap<TaskId, TaskProgress>>,
    /// 每個 Project 內每條 Workstream 目前的畫面覆蓋；沒有覆蓋的 workstream 不在這裡。
    pub overrides: HashMap<ProjectId, HashMap<WorkstreamId, Override>>,
    /// 每個 Project 載入狀態檔時產生的 warning（例如 task 的 `stage` 已不存在）；沒有 warning
    /// 的 project 不在這裡，等同空清單。
    pub warnings: HashMap<ProjectId, Vec<String>>,
}

impl DomainState {
    /// 從設定檔解出的 Project 清單建構初始 Domain 狀態：沒有狀態檔（或尚未讀取狀態檔）時，
    /// 每個 Task 的進度為起始 Stage＋`Mark::None`，沒有覆蓋、沒有 warning
    /// （spec `pipeline-domain` 「初始進度」情境）。
    pub fn from_projects(projects: Vec<ProjectDef>) -> Self {
        let mut progress = HashMap::with_capacity(projects.len());
        for project in &projects {
            let mut task_progress = HashMap::with_capacity(project.tasks.len());
            for task in &project.tasks {
                task_progress.insert(task.id.clone(), TaskProgress::initial(task));
            }
            progress.insert(project.id.clone(), task_progress);
        }

        Self {
            projects,
            progress,
            overrides: HashMap::new(),
            warnings: HashMap::new(),
        }
    }
}
