//! Domain 層：Cockpit 自己對 Project／Workstream／Task 進度的認定，獨立於 HERDR
//! （`CONTEXT.md` 「兩層模型，不可混用」；design D1）。**不得 `use` 任何 HERDR 型別**——與
//! Runtime 層唯一的接點是 `BindingSpec.runtime`／`Override.runtime`／`BindingResolution` 內的
//! `RuntimeId`，以及 `Override.pane_id`／`BindingResolution` 內的 `PaneId`，兩者都是
//! `crate::types::ids` 既有的 Runtime 層 id，不是 HERDR 型別。
//!
//! 本模組（task 2.1）只定義型別；純函數（`apply_op`、`validate_override`、`resolve_binding`、
//! `derive_status`）留給 task 2.2–2.4，投影整合留給 task 2.5。

pub mod binding;
pub mod config;
pub mod ids;
pub mod progress;
pub mod rejection;
pub mod repo;
pub mod state;
pub mod status;

pub use binding::{BindingResolution, BindingSource, Override};
pub use config::{BindingSpec, PinnedPane, ProjectDef, TaskDef, WorkstreamDef};
pub use ids::{ProjectId, TaskId, WorkstreamId};
pub use progress::{Mark, ProgressOp, TaskProgress, apply_op};
pub use rejection::Rejection;
pub use repo::{
    PaneRepo, PaneRepos, REPO_PROJECT_NAME_MAX_CHARS, REPO_PROJECT_STAGE_MAX_CHARS,
    REPO_PROJECT_STAGES_MAX, RepoExpansion, RepoKey, RepoProjectDef, StageEdit, StageRemap,
    apply_stage_edits, derive_repo_project_id, expand_repo_projects, is_disallowed_label_char,
    is_valid_repo_project_id, normalize_repo_project_name, normalize_repo_project_stages,
    pane_item_id, split_pane_item_id,
};
pub use state::DomainState;
pub use status::{StageStatus, derive_status};
