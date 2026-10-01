//! `DomainState`：`StoreHandle` 內與 `RuntimeStore` 並列的 Domain 層狀態（design D2）。
//! `Vec<ProjectDef>` 加每個 Task 的進度、每條 Workstream 的覆蓋、載入時的 warnings。
//! 狀態檔讀寫（含容錯與 warning 產生）屬於 `cockpit` 的 `progress.rs`（design D1）；這裡只提供
//! 「沒有狀態檔」時的初始建構——每個 Task 的初始進度＝起始 Stage＋`Mark::None`
//! （spec `pipeline-domain` 「初始進度」）。

use std::collections::HashMap;

use crate::domain::binding::Override;
use crate::domain::config::ProjectDef;
use crate::domain::ids::{ProjectId, TaskId, WorkstreamId};
use crate::domain::progress::{Mark, ProgressOp, TaskProgress, apply_op};
use crate::domain::rejection::Rejection;

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
    /// 每個 Project 內每條 Workstream 的目前 task（綁定的 agent 正在做哪個 task）；沒有目前 task
    /// 的 workstream 不在這裡，空 map 也不保留（progress-model task 2.2，design D1）。
    pub active: HashMap<ProjectId, HashMap<WorkstreamId, TaskId>>,
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
            active: HashMap::new(),
            warnings: HashMap::new(),
        }
    }
}

/// 目前 task 與覆蓋的狀態轉移（progress-model task 2.2，design D1）。這些方法只改 `self`、
/// 不做 IO；回傳 `Err` 時 `self` 完全不變，呼叫端可在 clone 上呼叫、成功才落檔並換上新狀態。
impl DomainState {
    /// 取一條 workstream 目前的 task。
    pub fn active_task(&self, project: &ProjectId, workstream: &WorkstreamId) -> Option<&TaskId> {
        self.active.get(project)?.get(workstream)
    }

    /// 把 `task` 設為 `workstream` 的目前 task，取代原本的（spec `pipeline-domain`「目前 task」）。
    ///
    /// # Errors
    ///
    /// task 不存在或不屬於該 workstream → [`Rejection::TaskNotInWorkstream`]；task 標記不是
    /// `none` → [`Rejection::AlreadyMarked`]。檢查順序：先歸屬、再標記。
    pub fn set_active(
        &mut self,
        project: &ProjectId,
        workstream: &WorkstreamId,
        task: &TaskId,
    ) -> Result<(), Rejection> {
        let belongs = self
            .projects
            .iter()
            .find(|p| &p.id == project)
            .and_then(|p| p.tasks.iter().find(|t| &t.id == task))
            .is_some_and(|t| &t.workstream == workstream);
        if !belongs {
            return Err(Rejection::TaskNotInWorkstream);
        }
        let marked = self
            .progress
            .get(project)
            .and_then(|m| m.get(task))
            .is_some_and(|p| p.mark != Mark::None);
        if marked {
            return Err(Rejection::AlreadyMarked);
        }
        self.active
            .entry(project.clone())
            .or_default()
            .insert(workstream.clone(), task.clone());
        Ok(())
    }

    /// 清除一條 workstream 的目前 task；本來就沒有時什麼都不做。
    pub fn clear_active(&mut self, project: &ProjectId, workstream: &WorkstreamId) {
        if let Some(map) = self.active.get_mut(project) {
            map.remove(workstream);
            if map.is_empty() {
                self.active.remove(project);
            }
        }
    }

    /// 對一個 task 套用進度操作並寫回 `progress`；`Complete`／`Fail` 成功時，若該 task 是所屬
    /// workstream 的目前 task 就一併清除。推進、退回、清除標記不動目前 task。被拒絕時不改任何東西。
    ///
    /// 前置條件：`project`、`task` 存在於 `projects`，且 `progress` 有該 task 的項目
    /// （呼叫端已在同一把鎖內確認；與 `apply_op` 對 stage 的前置條件同類）。
    ///
    /// # Errors
    ///
    /// 與 [`apply_op`] 相同。
    pub fn apply_progress(
        &mut self,
        project: &ProjectId,
        task: &TaskId,
        op: ProgressOp,
    ) -> Result<(), Rejection> {
        let def = self
            .projects
            .iter()
            .find(|p| &p.id == project)
            .expect("project 必須存在（呼叫端前置條件）");
        let workstream = def
            .tasks
            .iter()
            .find(|t| &t.id == task)
            .expect("task 必須存在（呼叫端前置條件）")
            .workstream
            .clone();
        let current = self
            .progress
            .get(project)
            .and_then(|m| m.get(task))
            .expect("task 必須有進度項目（呼叫端前置條件）");
        let next = apply_op(def, current, op)?;
        self.progress
            .entry(project.clone())
            .or_default()
            .insert(task.clone(), next);

        if matches!(op, ProgressOp::Complete | ProgressOp::Fail)
            && self.active_task(project, &workstream) == Some(task)
        {
            self.clear_active(project, &workstream);
        }
        Ok(())
    }

    /// 設定一條 workstream 的畫面覆蓋，並清除該 workstream 的目前 task（改綁後 agent 已不同）。
    /// 覆蓋本身的合法性（`validate_override`）由呼叫端先檢查。新值與現有覆蓋相等時綁定沒變，
    /// 直接返回、不清目前 task（與 `remove_override` 的 no-op 規則對稱）。
    pub fn set_override(
        &mut self,
        project: &ProjectId,
        workstream: &WorkstreamId,
        override_: Override,
    ) {
        if self.overrides.get(project).and_then(|m| m.get(workstream)) == Some(&override_) {
            return;
        }
        self.overrides
            .entry(project.clone())
            .or_default()
            .insert(workstream.clone(), override_);
        self.clear_active(project, workstream);
    }

    /// 移除一條 workstream 的覆蓋（取消或失效移除共用）；**真的移除了一筆覆蓋**時才連帶清除目前
    /// task。覆蓋本來就不存在時是 no-op、不算取消，狀態完全不變。project 已沒有任何覆蓋時連同外層
    /// 項目移除，讓「沒有覆蓋」只有一種表示法。
    pub fn remove_override(&mut self, project: &ProjectId, workstream: &WorkstreamId) {
        let Some(map) = self.overrides.get_mut(project) else {
            return;
        };
        if map.remove(workstream).is_none() {
            return;
        }
        if map.is_empty() {
            self.overrides.remove(project);
        }
        self.clear_active(project, workstream);
    }
}
