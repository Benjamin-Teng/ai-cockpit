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
use crate::domain::repo::{PaneRepos, RepoProjectDef, expand_repo_projects, split_pane_item_id};
use crate::message::Message;
use crate::store::RuntimeStore;
use crate::types::connection::ConnectionState;

/// `cockpit-core` 的 Domain 層完整狀態：Project 結構、各 Task 的進度、各 Workstream 的覆蓋、
/// 載入時的 warnings（依 project 分組）。`Default` 是「沒有任何 Project」的狀態（spec
/// `state-projection` 「沒有 Project」）。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DomainState {
    /// 實際生效的 Project 清單：手寫的依設定檔順序在前，展開後的 Repo Project 在後（repo-projects
    /// design D3）。有 Repo Project 時由 [`DomainState::refresh_projects`] 重算，投影、進度操作與 agent
    /// 端點不必分辨兩種 project。
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
    ///
    /// **每一則 warning 的字串必須由 [`crate::Message`] 的 `text()` 產生**（ui-language design D4）：
    /// 投影以 `Message::classify` 反推 `warning_msgs`，英文介面才能依代碼翻譯；自己 `format!` 的繁中原文會被
    /// 歸成 `raw`、英文介面退回顯示繁中，而且不會有任何測試失敗。新增 warning 種類時，先在 `message.rs` 新增
    /// `Message` 變體並補前端字典的 `msg.<code>`。
    pub warnings: HashMap<ProjectId, Vec<String>>,
    /// Repo Project 定義（repo-projects task 3.1，design D3）；順序不具意義，展開時依名稱排序。
    /// 包含因 id 撞名而未展開的 Repo Project。
    pub repo_projects: Vec<RepoProjectDef>,
    /// Repo Project 的 task 進度（design D3、D4）：Repo Project id → task id（`<runtime>~<pane>`）→ 進度。
    /// 與手寫 project 的 `progress` 分開存放，id 撞名時兩邊互不影響；不以目前展開出來的 task 過濾。
    pub repo_progress: HashMap<ProjectId, HashMap<TaskId, TaskProgress>>,
    /// pane 的 repo 歸類結果（design D2），不持久化。
    pub pane_repos: PaneRepos,
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
            repo_projects: Vec::new(),
            repo_progress: HashMap::new(),
            pane_repos: PaneRepos::new(),
        }
    }

    /// 以 `projects` 中的手寫 project（`repo` 為 `None`，依原順序）、`repo_projects` 與 `pane_repos`
    /// 重算 `projects`（[`expand_repo_projects`]），並把 `warnings` 中的撞名警告換成這次的結果；載入時
    /// 產生的其他警告保留、順序不變，撞名警告接在後面。不碰任何進度、覆蓋與目前 task（repo-projects
    /// task 3.1，design D3）。手寫 project 清單取自 `projects` 本身，呼叫端不必另外保存；重複呼叫結果相同。
    pub fn refresh_projects(&mut self) {
        let expansion = expand_repo_projects(&self.projects, &self.repo_projects, &self.pane_repos);
        self.projects = expansion.projects;
        self.warnings.retain(|_, list| {
            list.retain(|text| {
                !matches!(
                    Message::classify(text),
                    Message::RepoProjectIdConflict { .. }
                )
            });
            !list.is_empty()
        });
        for (project, list) in expansion.warnings {
            self.warnings.entry(project).or_default().extend(list);
        }
    }

    /// 取一個 project 的 task 進度表（repo-projects task 4.1，design D3）：**依 `def.repo` 分流**——生效 def
    /// 是 Repo Project（`repo` 有值）讀 `repo_progress`，手寫 project 讀 `progress`。這是進度讀寫唯一分辨
    /// project 種類的地方；不得改以「id 是否在 `repo_projects` 裡」判斷（id 撞名時會讀到另一邊的進度）。
    /// 該 project 還沒有任何進度時回 `None`。
    pub fn progress_for(&self, def: &ProjectDef) -> Option<&HashMap<TaskId, TaskProgress>> {
        if def.repo.is_some() {
            self.repo_progress.get(&def.id)
        } else {
            self.progress.get(&def.id)
        }
    }

    /// 同 [`DomainState::progress_for`] 的分流規則，取可寫的進度表；沒有時建立空表。`def` 通常是
    /// `projects` 中項目的 clone（借用規則不允許同時借 `projects` 與進度表）。
    pub fn progress_for_mut(&mut self, def: &ProjectDef) -> &mut HashMap<TaskId, TaskProgress> {
        self.progress_tables_mut(def)
            .entry(def.id.clone())
            .or_default()
    }

    /// [`DomainState::progress_for_mut`] 與 [`DomainState::drop_untouched_initial`] 共用的分流：依 `def.repo` 取
    /// `repo_progress` 或 `progress` 整張對照表。
    fn progress_tables_mut(
        &mut self,
        def: &ProjectDef,
    ) -> &mut HashMap<ProjectId, HashMap<TaskId, TaskProgress>> {
        if def.repo.is_some() {
            &mut self.repo_progress
        } else {
            &mut self.progress
        }
    }

    /// 撤回一筆「為了滿足前置條件才補上」的初始進度（repo-projects task 4.2）：`def` 的進度表（分流規則同
    /// [`DomainState::progress_for`]）中 `task` 的項目仍等於 `initial` 時移除；`remove_empty_table` 為 `true`（表也是
    /// 剛才補的）且表因此變空時連同表移除。只在呼叫端剛補上這筆、操作又沒有改變它時呼叫——沒有紀錄與初始值同義，
    /// 留著會讓空操作（例如對 Repo Project task 宣告、對未標記的 task 清除標記）多出一筆紀錄而落檔。
    pub fn drop_untouched_initial(
        &mut self,
        def: &ProjectDef,
        task: &TaskId,
        initial: &TaskProgress,
        remove_empty_table: bool,
    ) {
        let table = self.progress_tables_mut(def);
        let Some(entries) = table.get_mut(&def.id) else {
            return;
        };
        if entries.get(task) == Some(initial) {
            entries.remove(task);
            if remove_empty_table && entries.is_empty() {
                table.remove(&def.id);
            }
        }
    }

    /// 清除消失 pane 的 Repo Project 進度（design D4；spec `repo-projects`「Repo Project 進度的保存與清除」，
    /// repo-projects task 4.2）：`repo_progress` 的每一筆（含被撞名隱藏的 Repo Project），只在它的 runtime（task id
    /// 最後一個 `~` 之前）在 `store` 中為 `connected`、這輪連線的沉降重拿已換上（`settled`，repo-projects
    /// task 4.6：剛連上的首份 snapshot 可能還不完整，清除不可逆）、且該 runtime 目前的 pane 樹**沒有**這個
    /// pane id 時清除。已 exited 但仍在樹中、runtime 未連線／尚未連上／尚未沉降／未登記、id 拆不開時一律保留。依據是呼叫當下的
    /// `RuntimeStore`，不是投影（專案 memory：用落後的投影做檢查會放過剛寫入的狀態）。被這次清空的進度表一併
    /// 移除。有清掉任何一筆時回 `true`。
    pub fn clear_vanished_repo_progress(&mut self, store: &RuntimeStore) -> bool {
        let vanished = |task: &TaskId| -> bool {
            let Some((runtime, pane)) = split_pane_item_id(task.as_str()) else {
                return false;
            };
            store.state(&runtime).is_some_and(|state| {
                matches!(
                    state.connection,
                    ConnectionState::Connected { settled: true, .. }
                ) && !state.panes.contains_key(&pane)
            })
        };
        let mut cleared = false;
        self.repo_progress.retain(|_, table| {
            let before = table.len();
            table.retain(|task, _| !vanished(task));
            if table.len() == before {
                return true;
            }
            cleared = true;
            !table.is_empty()
        });
        cleared
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
    /// 標記依生效 def 的種類讀（[`DomainState::progress_for`]）。Repo Project（`def.repo` 有值）的目前 task 就是
    /// 標記為 none 的那張、不保存（design D4，repo-projects task 4.2）：照同樣規則判定，通過時什麼都不改。
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
        let Some(def) = self.projects.iter().find(|p| &p.id == project) else {
            return Err(Rejection::TaskNotInWorkstream);
        };
        let belongs = def
            .tasks
            .iter()
            .find(|t| &t.id == task)
            .is_some_and(|t| &t.workstream == workstream);
        if !belongs {
            return Err(Rejection::TaskNotInWorkstream);
        }
        let marked = self
            .progress_for(def)
            .and_then(|m| m.get(task))
            .is_some_and(|p| p.mark != Mark::None);
        if marked {
            return Err(Rejection::AlreadyMarked);
        }
        if def.repo.is_some() {
            return Ok(());
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

    /// 對一個 task 套用進度操作並寫回生效 def 種類對應的進度表（[`DomainState::progress_for`]，
    /// repo-projects task 4.2）；`Complete`／`Fail` 成功時，若該 task 是所屬 workstream 的目前 task
    /// 就一併清除。推進、退回、清除標記不動目前 task。被拒絕時不改任何東西。
    ///
    /// 前置條件：`project`、`task` 存在於 `projects`，且該進度表有這個 task 的項目
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
            .expect("project 必須存在（呼叫端前置條件）")
            .clone();
        let workstream = def
            .tasks
            .iter()
            .find(|t| &t.id == task)
            .expect("task 必須存在（呼叫端前置條件）")
            .workstream
            .clone();
        let current = self
            .progress_for(&def)
            .and_then(|m| m.get(task))
            .expect("task 必須有進度項目（呼叫端前置條件）");
        let next = apply_op(&def, current, op)?;
        self.progress_for_mut(&def).insert(task.clone(), next);

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
