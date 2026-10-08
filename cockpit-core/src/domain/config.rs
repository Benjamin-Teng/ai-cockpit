//! Project 結構的 Domain 型別：由 `cockpit` 的 `config.rs` 從 TOML 解析並驗證後建出
//! （spec `pipeline-config`；design D1）。這裡只描述形狀，不做驗證——欄位預設值（例如
//! task 未給 `stage` 時取 `stages` 第一個）由 `cockpit` 在建構前解出，`TaskDef.stage` 已是
//! 解出後的起始 stage，不是 `Option`。
//!
//! 不得 `use` 任何 HERDR 型別（ADR-0003）；`BindingSpec.runtime` 重用
//! `crate::types::ids::RuntimeId`——那是 Runtime 層本來就有、且是兩層唯一接點的型別，不是
//! HERDR 型別。

use crate::domain::ids::{ProjectId, TaskId, WorkstreamId};
use crate::domain::repo::RepoKey;
use crate::types::ids::{PaneId, RuntimeId};

/// 一條 Workstream 的 Runtime 綁定設定（spec `pipeline-config` 「Project 區段結構」）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BindingSpec {
    /// 要綁定的 runtime id（必須是設定檔中已登記的某個 `[[runtime]]`）。
    pub runtime: RuntimeId,
    /// 目標 workspace 的標籤，完全相等比對。
    pub workspace: String,
    /// 目標 pane 的標籤，完全相等比對；`None` 表示不以此篩選。
    pub pane_label: Option<String>,
    /// 目標 pane cwd 的路徑片段，正規化後以連續片段比對；`None` 表示不以此篩選。
    pub cwd: Option<String>,
    /// 目標 pane 的 agent 名稱，完全相等比對；`None` 表示不以此篩選。
    pub agent: Option<String>,
}

/// 一條 Workstream（spec `pipeline-config` 「Project 區段結構」）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkstreamDef {
    /// 同一 Project 內唯一的 workstream id。
    pub id: WorkstreamId,
    /// 顯示名稱（設定檔未給時預設等於 `id`，由 `cockpit` 解出）。
    pub name: String,
    /// Runtime 綁定設定；`None` 表示這條 workstream 沒有 binding。
    pub binding: Option<BindingSpec>,
    /// Repo Project 展開出的工作線固定綁定的 pane（repo-projects task 3.1，design D3）；手寫 project
    /// 一律為 `None`。有值時 `binding` 為 `None`，綁定解析只看這個 pane、不接受覆蓋。
    pub pinned_pane: Option<PinnedPane>,
}

/// Repo Project 工作線固定綁定的 pane（spec `repo-projects`「Repo Project 工作線的固定 pane 綁定」）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PinnedPane {
    /// pane 所在的 runtime id。
    pub runtime: RuntimeId,
    /// pane id。
    pub pane_id: PaneId,
    /// pane 位於 linked worktree 時為 worktree 資料夾名稱；主 worktree 為 `None`（design D1）。
    pub worktree: Option<String>,
}

/// 一個 Task（spec `pipeline-config` 「Project 區段結構」）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TaskDef {
    /// 同一 Project 內唯一的 task id。
    pub id: TaskId,
    /// 顯示標題（設定檔未給時預設等於 `id`，由 `cockpit` 解出）。
    pub title: String,
    /// 所屬 workstream 的 id。
    pub workstream: WorkstreamId,
    /// 起始 Stage（設定檔未給時預設為所屬 Project `stages` 第一個，由 `cockpit` 解出）；
    /// 用於建構 `TaskProgress` 的初始值，之後只能經由進度操作改變。
    pub stage: String,
    /// 依賴的 task id（同一 Project 內），預設空。
    pub depends_on: Vec<TaskId>,
}

/// 一個 Project：一條 Pipeline（`stages`）加若干 Workstream 與 Task
/// （spec `pipeline-config` 「Project 區段結構」）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectDef {
    /// 設定檔內唯一的 project id。
    pub id: ProjectId,
    /// 顯示名稱（設定檔未給時預設等於 `id`，由 `cockpit` 解出）。
    pub name: String,
    /// Stage 的線性順序；字串同時是 Stage 的識別與顯示名稱。
    pub stages: Vec<String>,
    /// 這個 Project 底下的 workstream，依設定檔順序。
    pub workstreams: Vec<WorkstreamDef>,
    /// 這個 Project 底下的 task，依設定檔順序。
    pub tasks: Vec<TaskDef>,
    /// 種類標記（repo-projects task 3.1，design D3、D8）：手寫於設定檔的 project 為 `None`；由
    /// [`crate::expand_repo_projects`] 展開的 Repo Project 為它的 repo key（也是投影要輸出的 `repo`）。
    /// 撞名時生效清單裡的是手寫 project（`None`），所以判斷種類一律看這個欄位，不看 `repo_projects`
    /// 有沒有同 id 的定義。
    pub repo: Option<RepoKey>,
}
