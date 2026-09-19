//! RED→GREEN 驗收測試（Task 2.2，change `pipeline-projection`）：`apply_op` 的四種進度操作
//! 接受／拒絕規則（spec `pipeline-domain`「進度操作」全部情境），以及 `agent_status_never_changes_progress`
//! ——對一個真的 `RuntimeStore` 逐筆套用 `AgentStatusChanged`，證明旁邊一份 `DomainState` 完全
//! 不受影響（spec「agent done 不改進度」情境的一般化；domain 掛進 `StoreHandle` 是 task 2.5 的
//! 事，這裡兩者刻意各自獨立，不做任何接線）。

mod common;

use std::time::SystemTime;

use cockpit_core::domain::{
    DomainState, Mark, ProgressOp, ProjectDef, Rejection, TaskProgress, apply_op,
};
use cockpit_core::{AgentStatus, RuntimeEvent, RuntimeStore};

use common::{empty_focused, pane, pane_id, runtime_id, snapshot, tab, workspace};

/// `stages = ["Spec", "Plan", "Build"]`，沒有 workstream／task（`apply_op` 只用得到 `stages`）。
fn sample_project() -> ProjectDef {
    use cockpit_core::domain::{ProjectId, TaskDef, TaskId, WorkstreamDef, WorkstreamId};

    ProjectDef {
        id: ProjectId::new("p"),
        name: "p".to_string(),
        stages: vec!["Spec".to_string(), "Plan".to_string(), "Build".to_string()],
        workstreams: vec![WorkstreamDef {
            id: WorkstreamId::new("be"),
            name: "be".to_string(),
            binding: None,
        }],
        tasks: vec![TaskDef {
            id: TaskId::new("t1"),
            title: "t1".to_string(),
            workstream: WorkstreamId::new("be"),
            stage: "Plan".to_string(),
            depends_on: vec![],
        }],
    }
}

fn progress_at(stage: &str, mark: Mark) -> TaskProgress {
    TaskProgress {
        stage: stage.to_string(),
        mark,
    }
}

#[test]
fn advance_moves_to_next_stage() {
    // spec 「推進到下一站」：stages = [Spec, Plan, Build]，t1 在 Plan、標記 none。
    let project = sample_project();
    let progress = progress_at("Plan", Mark::None);

    let result = apply_op(&project, &progress, ProgressOp::Advance).expect("Plan 不是最後一站");

    assert_eq!(result, progress_at("Build", Mark::None));
}

#[test]
fn advance_rejected_on_last_stage() {
    // spec 「最後一站不能推進」：t1 在 Build（最後一個），被拒絕且進度不變。
    let project = sample_project();
    let progress = progress_at("Build", Mark::None);

    let result = apply_op(&project, &progress, ProgressOp::Advance);

    assert_eq!(result, Err(Rejection::AlreadyLastStage));
}

#[test]
fn advance_rejected_when_already_marked() {
    // spec 「有標記時不能推進或重標」：t1 標記為 failed，推進被拒絕、進度不變。
    let project = sample_project();
    let progress = progress_at("Plan", Mark::Failed);

    let result = apply_op(&project, &progress, ProgressOp::Advance);

    assert_eq!(result, Err(Rejection::AlreadyMarked));
}

#[test]
fn complete_rejected_when_already_marked() {
    // 同一情境套用在「標 Completed」：t1 標記為 failed 時不能重標。
    let project = sample_project();
    let progress = progress_at("Plan", Mark::Failed);

    let result = apply_op(&project, &progress, ProgressOp::Complete);

    assert_eq!(result, Err(Rejection::AlreadyMarked));
}

#[test]
fn fail_rejected_when_already_marked() {
    // 「標 Failed」同樣在已有標記時被拒絕（spec 「進度操作」需求本文，涵蓋 complete 與 fail 兩者）。
    let project = sample_project();
    let progress = progress_at("Plan", Mark::Completed);

    let result = apply_op(&project, &progress, ProgressOp::Fail);

    assert_eq!(result, Err(Rejection::AlreadyMarked));
}

#[test]
fn clear_is_idempotent_and_always_accepted() {
    // spec 「清除標記可反悔」：t1 在 Plan、標記 completed，清除後標記 none；再清除一次同樣接受
    // 且不變（task 原文「clear 在 none 時回傳相同狀態」）。
    let project = sample_project();
    let marked = progress_at("Plan", Mark::Completed);

    let cleared = apply_op(&project, &marked, ProgressOp::Clear).expect("清除一律接受");
    assert_eq!(cleared, progress_at("Plan", Mark::None));

    let cleared_again =
        apply_op(&project, &cleared, ProgressOp::Clear).expect("在 none 時清除仍一律接受");
    assert_eq!(cleared_again, cleared);
}

#[test]
fn complete_accepted_when_not_last_stage() {
    // spec 「不在最後一站也能標 Completed」：t1 在 Spec（非最後一個）、標記 none。
    let project = sample_project();
    let progress = progress_at("Spec", Mark::None);

    let result = apply_op(&project, &progress, ProgressOp::Complete).expect("標記為 none 時可標記");

    assert_eq!(result, progress_at("Spec", Mark::Completed));
}

#[test]
fn fail_accepted_when_not_last_stage() {
    // 與上一個測試對稱：標 Failed 同樣不受「是否最後一站」影響，只看標記是否為 none。
    let project = sample_project();
    let progress = progress_at("Spec", Mark::None);

    let result = apply_op(&project, &progress, ProgressOp::Fail).expect("標記為 none 時可標記");

    assert_eq!(result, progress_at("Spec", Mark::Failed));
}

#[test]
fn rejected_ops_do_not_change_progress() {
    // 「被拒絕的操作不得改變任何進度」：直接比對 Err 回傳前後，呼叫端拿到的仍是原本的 progress
    // （`apply_op` 回傳 `Result`，被拒絕時完全沒有機會覆寫呼叫端持有的舊值——這裡用原值與
    // 呼叫後仍持有的原值相等來具體證明)。
    let project = sample_project();
    let original = progress_at("Build", Mark::None);
    let before = original.clone();

    let result = apply_op(&project, &original, ProgressOp::Advance);

    assert_eq!(result, Err(Rejection::AlreadyLastStage));
    assert_eq!(
        original, before,
        "apply_op 必須是唯讀的，不得動到傳入的 progress"
    );
}

#[test]
fn agent_status_never_changes_progress() {
    // spec 「agent done 不改進度」的一般化：對一個真的 `RuntimeStore` 逐一套用 pane 的
    // `AgentStatusChanged`（至少 working → done，這裡涵蓋全部五個值），每一步都先確認事件
    // 真的套用到 store（不是空操作），再斷言旁邊一份 `DomainState` 與套用前完全相等
    // （bit-for-bit equal）。`DomainState` 與 `RuntimeStore` 這裡刻意各自獨立、沒有任何接線
    // ——`apply_op` 的簽章沒有 `AgentStatus` 參數，domain 層目前也沒有任何函數會去讀
    // `RuntimeStore`，所以套用 runtime 事件在架構上就不可能碰到 `DomainState`。

    // 1. Domain 端：先用 apply_op 把進度推到一個非初值的狀態，證明後面的「不變」不是因為
    //    進度本來就沒東西好動。
    let project = sample_project();
    let initial = progress_at("Plan", Mark::None);
    let progress = apply_op(&project, &initial, ProgressOp::Advance).expect("Plan 不是最後一站");
    assert_ne!(
        progress, initial,
        "先製造一個非初值的進度，才能測「狀態變化不影響它」"
    );

    let mut domain = DomainState::from_projects(vec![project.clone()]);
    let project_id = project.id.clone();
    let task_id = project.tasks[0].id.clone();
    domain
        .progress
        .get_mut(&project_id)
        .expect("project 應該有進度 map")
        .insert(task_id, progress.clone());
    let domain_before = domain.clone();

    // 2. Runtime 端：獨立的 RuntimeStore，一個 pane，逐一套用 agent 狀態事件。
    let mut store = RuntimeStore::new();
    let win = runtime_id("win");
    store.register(win.clone(), "herdr".to_string(), "tcp://win".to_string());
    let snap = snapshot(
        vec![workspace("wJ", 1)],
        vec![tab("wJ:t1", "wJ", 1)],
        vec![pane("wJ:p1", "wJ", "wJ:t1")],
        vec![],
        empty_focused(),
    );
    store.replace(&win, snap).expect("replace 應成功");

    for status in [
        AgentStatus::Working,
        AgentStatus::Blocked,
        AgentStatus::Done,
        AgentStatus::Idle,
        AgentStatus::Unknown,
    ] {
        store
            .apply(
                &win,
                RuntimeEvent::AgentStatusChanged {
                    pane_id: pane_id("wJ:p1"),
                    status,
                    title: None,
                    agent: None,
                },
                SystemTime::now(),
            )
            .expect("AgentStatusChanged 應成功");

        // 先證明事件真的套用了（不是空操作）：pane 的 agent_status 已依事件更新。
        let applied_status = store
            .state(&win)
            .expect("win 應已登記")
            .panes
            .get(&pane_id("wJ:p1"))
            .expect("pane 仍應存在")
            .agent_status;
        assert_eq!(
            applied_status, status,
            "agent_status 應該真的被套用到 RuntimeStore，不是空操作"
        );

        // 核心斷言：RuntimeStore 已經變了，但 DomainState 必須與套用任何事件之前完全相等。
        assert_eq!(
            domain, domain_before,
            "AgentStatus::{status:?} 不應影響 DomainState"
        );
    }
}
