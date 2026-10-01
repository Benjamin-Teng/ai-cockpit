//! Task 進度：目前所在的 Stage 與人工標記 `Mark`（spec `pipeline-domain` 「Task 進度」）。
//! `ProgressOp` 是五種進度操作，序列化字串與 HTTP 路徑字串一致（spec `pipeline-progress`
//! 「進度寫入端點」的 `<操作>`）。轉移規則見本檔 `apply_op`（design D1）。

use serde::{Deserialize, Serialize};

use crate::domain::config::{ProjectDef, TaskDef};
use crate::domain::rejection::Rejection;

/// Task 的人工標記，三選一，初值 `None`，只能經由進度操作改變
/// （spec `pipeline-domain`；`CONTEXT.md` Mark）。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mark {
    /// 沒有標記。
    #[default]
    None,
    /// 已標 Completed。
    Completed,
    /// 已標 Failed。
    Failed,
}

/// 一個 Task 目前的進度：所在 Stage 與標記（spec `pipeline-domain` 「Task 進度」）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TaskProgress {
    /// 目前所在的 Stage。
    pub stage: String,
    /// 目前的人工標記。
    pub mark: Mark,
}

impl TaskProgress {
    /// 一個 Task 剛載入、還沒有任何狀態檔紀錄時的初始進度：起始 Stage＋`Mark::None`
    /// （spec `pipeline-domain` 「初始進度」情境）。
    pub fn initial(task: &TaskDef) -> Self {
        Self {
            stage: task.stage.clone(),
            mark: Mark::None,
        }
    }
}

/// 五種進度操作，序列化值與 `POST /api/projects/<project>/tasks/<task>/<操作>` 的路徑字串
/// 一致（spec `pipeline-progress`）。轉移規則見 `apply_op`。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProgressOp {
    /// 推進到下一個 Stage。
    Advance,
    /// 退回到上一個 Stage（progress-model task 2.1）。
    Retreat,
    /// 標 Completed。
    Complete,
    /// 標 Failed。
    Fail,
    /// 清除標記。
    Clear,
}

/// 對一個 Task 的目前進度套用一次進度操作，回傳操作後的新進度，或被拒絕時回傳原因
/// （spec `pipeline-domain` 「進度操作」；design D1 `apply_op`）。純函數，不做任何 IO，也不接觸
/// Runtime 層任何型別（含 `AgentStatus`）——唯一輸入是這個 Task 所屬 Project 的 `stages`
/// （決定「下一個 Stage」與「是否已是最後一個」）與目前進度本身，被拒絕時不回傳任何新值，
/// 呼叫端手上原本持有的 `progress` 完全沒被動過。
///
/// 前置條件：`progress.stage` 必須是 `project.stages` 其中之一——狀態檔載入時的容錯（不合法
/// stage 退回起始 stage）在 `cockpit` 的載入階段就完成（design D5），到這裡時已保證合法。
pub fn apply_op(
    project: &ProjectDef,
    progress: &TaskProgress,
    op: ProgressOp,
) -> Result<TaskProgress, Rejection> {
    match op {
        ProgressOp::Advance => {
            if progress.mark != Mark::None {
                return Err(Rejection::AlreadyMarked);
            }
            let current_index = project
                .stages
                .iter()
                .position(|stage| stage == &progress.stage)
                .expect(
                    "progress.stage 必須是 project.stages 其中之一（design D5，載入時已正規化）",
                );
            let next_index = current_index + 1;
            if next_index >= project.stages.len() {
                return Err(Rejection::AlreadyLastStage);
            }
            Ok(TaskProgress {
                stage: project.stages[next_index].clone(),
                mark: Mark::None,
            })
        }
        ProgressOp::Retreat => {
            if progress.mark != Mark::None {
                return Err(Rejection::AlreadyMarked);
            }
            let current_index = project
                .stages
                .iter()
                .position(|stage| stage == &progress.stage)
                .expect(
                    "progress.stage 必須是 project.stages 其中之一（design D5，載入時已正規化）",
                );
            let Some(previous_index) = current_index.checked_sub(1) else {
                return Err(Rejection::AlreadyFirstStage);
            };
            Ok(TaskProgress {
                stage: project.stages[previous_index].clone(),
                mark: Mark::None,
            })
        }
        ProgressOp::Complete => {
            if progress.mark != Mark::None {
                return Err(Rejection::AlreadyMarked);
            }
            Ok(TaskProgress {
                stage: progress.stage.clone(),
                mark: Mark::Completed,
            })
        }
        ProgressOp::Fail => {
            if progress.mark != Mark::None {
                return Err(Rejection::AlreadyMarked);
            }
            Ok(TaskProgress {
                stage: progress.stage.clone(),
                mark: Mark::Failed,
            })
        }
        ProgressOp::Clear => Ok(TaskProgress {
            stage: progress.stage.clone(),
            mark: Mark::None,
        }),
    }
}
