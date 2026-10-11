//! OpenSpec 進度同步狀態與「自動移動／手動優先」的純函式狀態轉移（openspec-stage-sync task 3.2，design D2、D3）。
//!
//! 型別：[`Observation`]（偵測結果，即「進度指紋」）、[`SyncMode`]、[`TaskSync`]。`DomainState` 以兩份資料保存它們：
//! `repo_sync`（持久，每張 Repo Project task 的 [`TaskSync`]）與 `openspec_obs`（不持久，最新一輪偵測結果）。
//!
//! 這裡的方法全是 `DomainState` 上的純函式：不做 IO、不讀時間，回傳「狀態是否有改變」，讓寫入服務（task 4.3）在
//! 同一把寫入鎖內呼叫、有改變才落檔。同步狀態分開存放而不擴充 `TaskProgress`，所以 `apply_op` 與
//! `drop_untouched_initial` 的進度規則一行不改（design D2）。

use std::collections::{BTreeMap, BTreeSet, HashMap};

use serde::{Deserialize, Serialize};

use crate::domain::ids::{ProjectId, TaskId};
use crate::domain::progress::{Mark, TaskProgress};
use crate::domain::repo::{OpenSpecPhase, RepoProjectDef, split_pane_item_id};
use crate::domain::state::DomainState;
use crate::types::ids::{PaneId, RuntimeId};

/// 一輪偵測得到的 OpenSpec 進度（design D2「進度指紋」）：對應到的 change 名稱、判定出的階段與 `tasks.md` 的勾選數。
/// 比較時四個欄位全部參與，任一欄位不同就視為不同的偵測結果（design D3 第 3 條）。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Observation {
    /// 對應到的 change 名稱。
    pub change: String,
    /// 判定出的階段。
    pub phase: OpenSpecPhase,
    /// 已勾選的 checkbox 數。
    pub checked: u32,
    /// checkbox 總數。
    pub total: u32,
}

/// 卡片目前由誰決定位置（design D3）。序列化為小寫字串 `"auto" | "manual"`。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SyncMode {
    /// 偵測結果有變化時自動移動。
    Auto,
    /// 使用者手動推進／退回過；同一個偵測結果不再把卡片拉走，結果變化時恢復為 [`SyncMode::Auto`]。
    Manual,
}

/// 一張 Repo Project task 的同步狀態（design D2）。只有真的套用過偵測結果、或被手動入口標記過的 task 才有；有同步狀態的
/// task 一定有 `repo_progress` 項目，且不會被 [`DomainState::drop_untouched_initial`] 撤回。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskSync {
    /// 目前模式。
    pub mode: SyncMode,
    /// 上一次套用的偵測結果；從未套用過、或被清掉等待重新套用時為 `None`。
    pub applied: Option<Observation>,
}

/// 最新一輪偵測結果：`(runtime id, pane id)` → 偵測結果。不在這裡的 pane 沒有結果。型別慣例同 [`PaneRepos`]。
///
/// [`PaneRepos`]: crate::domain::repo::PaneRepos
pub type OpenSpecObservations = BTreeMap<(RuntimeId, PaneId), Observation>;

/// 每個 Repo Project 的每張 task 的同步狀態（持久）：Repo Project id → task id → [`TaskSync`]。
pub type RepoSync = HashMap<ProjectId, HashMap<TaskId, TaskSync>>;

/// 一張可以有同步狀態的 task 所需的定義資訊（呼叫當下從生效的 project 與 Repo Project 定義取出的副本）。
struct SyncTarget {
    /// 生效 stage 清單的第一個；沒有進度項目時的「目前 stage」與初始進度都是它。
    first_stage: String,
    /// 生效 stage 清單，用來確認自動移動寫入的 stage 一定在 `stages` 內。
    stages: Vec<String>,
    /// 該 task 所屬的 Repo Project 定義（取階段對應）。
    repo_def: RepoProjectDef,
}

impl SyncTarget {
    /// 擁有 `phase` 的 stage；沒有 stage 對應該階段、或對應到的名稱不在生效 `stages` 內（定義與生效清單暫時不一致）時
    /// 為 `None`。不在 `stages` 內的名稱絕不能寫進進度，否則之後的 `apply_op` 會 panic。
    fn stage_for_phase(&self, phase: OpenSpecPhase) -> Option<&str> {
        let index = self
            .repo_def
            .phases
            .iter()
            .position(|p| *p == Some(phase))?;
        let stage = self.repo_def.stages.get(index)?;
        self.stages.contains(stage).then_some(stage.as_str())
    }
}

impl DomainState {
    /// 取 `task` 的同步所需定義：`project` 必須是**生效的 Repo Project**（`repo` 有值；手寫 project、被同 id 手寫
    /// project 撞名隱藏的 Repo Project、未知 project 都不是），`task` 必須是它目前展開出來的 task，且 stages 非空。
    /// 否則 `None`，呼叫端視為 no-op。
    fn sync_target(&self, project: &ProjectId, task: &TaskId) -> Option<SyncTarget> {
        let def = self.projects.iter().find(|p| &p.id == project)?;
        let repo = def.repo.as_ref()?;
        if !def.tasks.iter().any(|t| &t.id == task) {
            return None;
        }
        let repo_def = self
            .repo_projects
            .iter()
            .find(|r| &r.id == project && &r.repo == repo)?;
        Some(SyncTarget {
            first_stage: def.stages.first()?.clone(),
            stages: def.stages.clone(),
            repo_def: repo_def.clone(),
        })
    }

    /// 該 task 沒有 `repo_progress` 項目時補初始進度（第一個 stage、`mark = none`，design D2）；補了回傳 `true`。
    fn ensure_repo_progress(&mut self, project: &ProjectId, task: &TaskId, first: &str) -> bool {
        let table = self.repo_progress.entry(project.clone()).or_default();
        if table.contains_key(task) {
            return false;
        }
        table.insert(
            task.clone(),
            TaskProgress {
                stage: first.to_string(),
                mark: Mark::None,
            },
        );
        true
    }

    /// 對一張 Repo Project task 套用一次偵測結果（openspec-stage-sync task 3.2，design D3、D10-7、D10-8）。
    /// 純函式：只改 `self` 的 `repo_progress` 與 `repo_sync`，不做 IO、不讀時間，也不寫 `openspec_obs`（最新偵測結果
    /// 由寫入服務另外更新）。依序判斷：
    ///
    /// 1. `obs` 為 `None`（對不上 change 或偵測失敗）→ 不改任何東西。
    /// 2. task 有進度項目且標記不是 `none` → 不改任何東西，`applied` 也不更新（清除標記後下一輪會因結果不同而套用）。
    /// 3. `obs` 等於 `applied` → 不改任何東西（手動優先靠這一條維持）。
    /// 4. 其他：`applied = obs`、`mode = Auto`；沒有進度項目就補初始進度（第一個 stage、`none`，「目前 stage」即第一個
    ///    stage）；找到擁有 `obs.phase` 的 stage、它在生效 `stages` 內且與目前不同時，直接把 `stage` 設為它（可跨站，
    ///    不經 `apply_op`）；找不到或就是目前 stage 時卡片不動。標記不變（archive 的 `complete` 也不自動標 Completed）。
    ///
    /// 回傳 `true` 表示狀態有改變，呼叫端（task 4.3）據此決定是否落檔。以下情況一律是 no-op 並回傳 `false`：
    /// `project` 不是生效的 Repo Project（手寫 `[[project]]`、被撞名隱藏、未知 id）、`task` 不在它目前展開的 task 內。
    pub fn apply_openspec(
        &mut self,
        project: &ProjectId,
        task: &TaskId,
        obs: Option<&Observation>,
    ) -> bool {
        let Some(obs) = obs else {
            return false;
        };
        let Some(target) = self.sync_target(project, task) else {
            return false;
        };
        let current = self.repo_progress.get(project).and_then(|t| t.get(task));
        if current.is_some_and(|p| p.mark != Mark::None) {
            return false;
        }
        let applied = self
            .repo_sync
            .get(project)
            .and_then(|t| t.get(task))
            .and_then(|s| s.applied.as_ref());
        if applied == Some(obs) {
            return false;
        }

        // 第 4 條：此時 `obs` 與 `applied` 不同，所以 `applied` 一定會變，整體一定有改變。
        self.ensure_repo_progress(project, task, &target.first_stage);
        self.repo_sync.entry(project.clone()).or_default().insert(
            task.clone(),
            TaskSync {
                mode: SyncMode::Auto,
                applied: Some(obs.clone()),
            },
        );
        if let Some(stage) = target.stage_for_phase(obs.phase)
            && let Some(progress) = self
                .repo_progress
                .get_mut(project)
                .and_then(|t| t.get_mut(task))
            && progress.stage != stage
        {
            progress.stage = stage.to_string();
        }
        true
    }

    /// 維持不變式「`openspec_obs` 只含目前展開中的 Repo Project task 的 pane」（openspec-stage-sync task 4.4 review fix
    /// round 2）：其餘 key 移除。Repo Project 被移除、被同 id 手寫 project 隱藏、或 pane 離開它之後，該 pane 已不在偵測
    /// 工作送出的表內；不在這裡清掉的話，過時的結果會一直留著，之後同一個 repo 重新加入時被重套到新卡片上。
    /// [`DomainState::refresh_projects`] 結尾一律呼叫，寫入服務整表同步後也呼叫。回傳是否有移除。
    pub fn retain_expanded_openspec_obs(&mut self) -> bool {
        let expanded: BTreeSet<(RuntimeId, PaneId)> = self
            .projects
            .iter()
            .filter(|def| def.repo.is_some())
            .flat_map(|def| &def.tasks)
            .filter_map(|task| split_pane_item_id(task.id.as_str()))
            .collect();
        let before = self.openspec_obs.len();
        self.openspec_obs.retain(|key, _| expanded.contains(key));
        self.openspec_obs.len() != before
    }

    /// 以 `openspec_obs` 對每張展開的 Repo Project task 呼叫 [`DomainState::apply_openspec`]（openspec-stage-sync
    /// task 4.3；Task 3.2 Ruling：「下一輪」的語意由全表重套實現）。沒有偵測結果的 task 傳 `None`（no-op）。
    /// D3 第 3 條保證冪等：連續呼叫兩次，第二次一定回傳 `false`。
    ///
    /// 寫入服務在下列時機於同一個寫入閉包內呼叫：整表同步（`sync_openspec`）、人工清除標記成功後、
    /// [`DomainState::reset_auto_applied`] 之後、以及 pane 歸類或 Repo Project 定義改變展開結果之後
    /// （先 `refresh_projects` 再呼叫，走訪的是新的展開結果）。回傳是否有任何 task 改變。
    pub fn reapply_openspec_all(&mut self) -> bool {
        let targets: Vec<(ProjectId, TaskId, Option<Observation>)> = self
            .projects
            .iter()
            .filter(|def| def.repo.is_some())
            .flat_map(|def| {
                def.tasks.iter().map(|task| {
                    let obs = split_pane_item_id(task.id.as_str())
                        .and_then(|key| self.openspec_obs.get(&key).cloned());
                    (def.id.clone(), task.id.clone(), obs)
                })
            })
            .collect();
        let mut changed = false;
        for (project, task, obs) in targets {
            changed |= self.apply_openspec(&project, &task, obs.as_ref());
        }
        changed
    }

    /// 把一張 Repo Project task 標為手動（openspec-stage-sync task 3.2，design D3「手動標記」）。給寫入服務（task 4.3）
    /// 在**同一個寫入閉包內**、`apply_progress` 成功之後呼叫（推進與標記只落檔一次；被拒絕的操作不呼叫）。
    ///
    /// - 已有 [`TaskSync`]：只把 `mode` 改為 [`SyncMode::Manual`]，`applied` 不變。
    /// - 沒有：建立 `{ Manual, applied }`，`applied` 取 `openspec_obs` 中這張 task 的 pane 當下的偵測結果（task id 為
    ///   `<runtime>~<pane>`，以 [`split_pane_item_id`] 拆開），沒有結果時為 `None`（design D10-7）。
    /// - task 沒有 `repo_progress` 項目時補初始進度（design D2）。
    ///
    /// 回傳 `true` 表示狀態有改變。`project` 不是生效的 Repo Project、`task` 不在展開的 task 內時是 no-op 並回傳
    /// `false`（手寫 project 的 task 沒有同步狀態）。
    pub fn mark_manual(&mut self, project: &ProjectId, task: &TaskId) -> bool {
        let Some(target) = self.sync_target(project, task) else {
            return false;
        };
        let mut changed = self.ensure_repo_progress(project, task, &target.first_stage);
        let existing = self.repo_sync.get(project).and_then(|t| t.get(task));
        match existing {
            Some(sync) if sync.mode == SyncMode::Manual => {}
            Some(_) => {
                if let Some(sync) = self
                    .repo_sync
                    .get_mut(project)
                    .and_then(|t| t.get_mut(task))
                {
                    sync.mode = SyncMode::Manual;
                }
                changed = true;
            }
            None => {
                let applied = split_pane_item_id(task.as_str())
                    .and_then(|key| self.openspec_obs.get(&key).cloned());
                self.repo_sync.entry(project.clone()).or_default().insert(
                    task.clone(),
                    TaskSync {
                        mode: SyncMode::Manual,
                        applied,
                    },
                );
                changed = true;
            }
        }
        changed
    }

    /// 階段對應被修改時，把 `project` 所有 `mode = Auto` 的 task 的 `applied` 清成 `None`，使下一輪偵測依新對應重新套用
    /// （openspec-stage-sync task 3.2，design D3「階段對應被修改時」）；`Manual` 的 task 不動。回傳是否有任何 `applied`
    /// 被清掉。
    ///
    /// **是否該呼叫由呼叫端判斷**：只有 [`StageRemap::phases_changed`] 為 `true` 才呼叫（design D10-2，判準只有那一套）。
    /// `update_repo_project` 以覆寫**之前**的 `def.stages`／`def.phases` 比較，呼叫後再以
    /// [`DomainState::reapply_openspec_all`] 依新對應重套（openspec-stage-sync task 4.3）。
    ///
    /// [`StageRemap::phases_changed`]: crate::domain::repo::StageRemap::phases_changed
    pub fn reset_auto_applied(&mut self, project: &ProjectId) -> bool {
        let Some(table) = self.repo_sync.get_mut(project) else {
            return false;
        };
        let mut changed = false;
        for sync in table.values_mut() {
            if sync.mode == SyncMode::Auto && sync.applied.is_some() {
                sync.applied = None;
                changed = true;
            }
        }
        changed
    }

    /// 移除 Repo Project 時清掉它的全部同步狀態（openspec-stage-sync task 3.2）；有清掉任何東西時回傳 `true`。
    pub fn clear_repo_project_sync(&mut self, project: &ProjectId) -> bool {
        self.repo_sync.remove(project).is_some()
    }
}
