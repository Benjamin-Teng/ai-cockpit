//! RED→GREEN 驗收測試（progress-model task 2.2）：`DomainState.active`（目前 task）的設定、
//! 拒絕與清除規則（spec `pipeline-domain`「目前 task」全部情境，design D1），外加
//! 「推進、退回、清除標記不影響目前 task」。

use cockpit_core::domain::{
    DomainState, Mark, Override, ProgressOp, ProjectDef, ProjectId, Rejection, TaskDef, TaskId,
    WorkstreamDef, WorkstreamId,
};
use cockpit_core::types::ids::{PaneId, RuntimeId};

fn pid() -> ProjectId {
    ProjectId::new("p")
}
fn ws(id: &str) -> WorkstreamId {
    WorkstreamId::new(id)
}
fn tid(id: &str) -> TaskId {
    TaskId::new(id)
}

fn task(id: &str, workstream: &str, stage: &str) -> TaskDef {
    TaskDef {
        id: tid(id),
        title: id.to_string(),
        workstream: ws(workstream),
        stage: stage.to_string(),
        depends_on: vec![],
    }
}

/// workstream `be`（t1、t2）與 `fe`（f1）；t1 在 Build、t2 與 f1 在 Plan。
fn state() -> DomainState {
    let workstream = |id: &str| WorkstreamDef {
        id: ws(id),
        name: id.to_string(),
        binding: None,
        pinned_pane: None,
    };
    DomainState::from_projects(vec![ProjectDef {
        id: pid(),
        name: "p".to_string(),
        stages: vec!["Spec".to_string(), "Plan".to_string(), "Build".to_string()],
        workstreams: vec![workstream("be"), workstream("fe")],
        tasks: vec![
            task("t1", "be", "Build"),
            task("t2", "be", "Plan"),
            task("f1", "fe", "Plan"),
        ],
        repo: None,
    }])
}

fn some_override() -> Override {
    Override {
        runtime: RuntimeId::new("win"),
        pane_id: PaneId::new("w1:p1"),
    }
}

fn active_of(state: &DomainState, workstream: &str) -> Option<TaskId> {
    state.active_task(&pid(), &ws(workstream)).cloned()
}

#[test]
fn new_state_has_no_active_task() {
    let state = state();
    assert!(state.active.is_empty());
    assert_eq!(active_of(&state, "be"), None);
}

#[test]
fn set_active_replaces_previous_active_task() {
    // spec 「宣告取代原本的目前 task」。
    let mut state = state();
    state.set_active(&pid(), &ws("be"), &tid("t1")).unwrap();
    assert_eq!(active_of(&state, "be"), Some(tid("t1")));

    state.set_active(&pid(), &ws("be"), &tid("t2")).unwrap();
    assert_eq!(active_of(&state, "be"), Some(tid("t2")));
}

#[test]
fn set_active_rejects_task_of_other_workstream() {
    let mut state = state();
    state.set_active(&pid(), &ws("be"), &tid("t1")).unwrap();
    let before = state.clone();

    let result = state.set_active(&pid(), &ws("be"), &tid("f1"));

    assert_eq!(result, Err(Rejection::TaskNotInWorkstream));
    assert_eq!(
        Rejection::TaskNotInWorkstream.to_string(),
        "task 不屬於該 workstream"
    );
    assert_eq!(state, before, "被拒絕時狀態不變");
}

#[test]
fn set_active_rejects_unknown_task() {
    let mut state = state();
    let before = state.clone();

    let result = state.set_active(&pid(), &ws("be"), &tid("ghost"));

    assert_eq!(result, Err(Rejection::TaskNotInWorkstream));
    assert_eq!(state, before);
}

#[test]
fn set_active_rejects_marked_task() {
    for op in [ProgressOp::Complete, ProgressOp::Fail] {
        let mut state = state();
        state.apply_progress(&pid(), &tid("t2"), op).unwrap();
        let before = state.clone();

        let result = state.set_active(&pid(), &ws("be"), &tid("t2"));

        assert_eq!(
            result,
            Err(Rejection::AlreadyMarked),
            "{op:?} 後不能設為目前 task"
        );
        assert_eq!(state, before);
    }
}

#[test]
fn complete_or_fail_clears_active_of_that_task() {
    // spec 「標完成後清除」；Fail 同理；清除標記後仍沒有目前 task。
    for op in [ProgressOp::Complete, ProgressOp::Fail] {
        let mut state = state();
        state.set_active(&pid(), &ws("be"), &tid("t1")).unwrap();

        state.apply_progress(&pid(), &tid("t1"), op).unwrap();
        assert_eq!(active_of(&state, "be"), None, "{op:?} 應清除目前 task");
        assert!(
            state.active.is_empty(),
            "沒有目前 task 的 project 不留空 map"
        );

        state
            .apply_progress(&pid(), &tid("t1"), ProgressOp::Clear)
            .unwrap();
        assert_eq!(
            active_of(&state, "be"),
            None,
            "清除標記不會讓目前 task 回來"
        );
    }
}

#[test]
fn marking_other_task_keeps_active() {
    // 目前 task 是 t1；同 workstream 的 t2 被標 Completed 不影響。
    let mut state = state();
    state.set_active(&pid(), &ws("be"), &tid("t1")).unwrap();

    state
        .apply_progress(&pid(), &tid("t2"), ProgressOp::Complete)
        .unwrap();

    assert_eq!(active_of(&state, "be"), Some(tid("t1")));
}

#[test]
fn marking_task_keeps_active_of_other_workstream() {
    let mut state = state();
    state.set_active(&pid(), &ws("fe"), &tid("f1")).unwrap();
    state.set_active(&pid(), &ws("be"), &tid("t1")).unwrap();

    state
        .apply_progress(&pid(), &tid("t1"), ProgressOp::Complete)
        .unwrap();

    assert_eq!(active_of(&state, "be"), None);
    assert_eq!(active_of(&state, "fe"), Some(tid("f1")));
}

#[test]
fn rejected_op_keeps_progress_and_active() {
    // t2 已 Failed，再對它標 Completed 被拒絕；同 workstream 的目前 task t1 不受影響。
    let mut state = state();
    state
        .apply_progress(&pid(), &tid("t2"), ProgressOp::Fail)
        .unwrap();
    state.set_active(&pid(), &ws("be"), &tid("t1")).unwrap();
    let before = state.clone();

    let result = state.apply_progress(&pid(), &tid("t2"), ProgressOp::Complete);

    assert_eq!(result, Err(Rejection::AlreadyMarked));
    assert_eq!(state, before);
}

#[test]
fn apply_progress_writes_new_progress() {
    let mut state = state();

    state
        .apply_progress(&pid(), &tid("t2"), ProgressOp::Advance)
        .unwrap();

    let progress = &state.progress[&pid()][&tid("t2")];
    assert_eq!(
        (progress.stage.as_str(), progress.mark),
        ("Build", Mark::None)
    );
}

#[test]
fn advance_retreat_clear_do_not_affect_active() {
    // spec 「退回不清除」＋「推進、退回、清除標記不影響目前 task」。
    let mut state = state();
    state.set_active(&pid(), &ws("be"), &tid("t2")).unwrap();

    state
        .apply_progress(&pid(), &tid("t2"), ProgressOp::Advance)
        .unwrap();
    assert_eq!(active_of(&state, "be"), Some(tid("t2")));

    state
        .apply_progress(&pid(), &tid("t2"), ProgressOp::Retreat)
        .unwrap();
    assert_eq!(active_of(&state, "be"), Some(tid("t2")));
    assert_eq!(state.progress[&pid()][&tid("t2")].stage, "Plan");

    state
        .apply_progress(&pid(), &tid("t2"), ProgressOp::Clear)
        .unwrap();
    assert_eq!(active_of(&state, "be"), Some(tid("t2")));
}

#[test]
fn set_override_clears_active() {
    // spec 「改綁後清除」。
    let mut state = state();
    state.set_active(&pid(), &ws("be"), &tid("t1")).unwrap();
    state.set_active(&pid(), &ws("fe"), &tid("f1")).unwrap();

    state.set_override(&pid(), &ws("be"), some_override());

    assert_eq!(active_of(&state, "be"), None);
    assert_eq!(
        active_of(&state, "fe"),
        Some(tid("f1")),
        "別條 workstream 不受影響"
    );
    assert_eq!(state.overrides[&pid()][&ws("be")], some_override());
}

#[test]
fn remove_override_clears_active_and_drops_empty_maps() {
    // 取消與失效移除共用同一條路徑。
    let mut state = state();
    state.set_override(&pid(), &ws("be"), some_override());
    state.set_active(&pid(), &ws("be"), &tid("t1")).unwrap();

    state.remove_override(&pid(), &ws("be"));

    assert_eq!(active_of(&state, "be"), None);
    assert!(state.overrides.is_empty(), "最後一筆覆蓋移除後不留空 map");
    assert!(state.active.is_empty());
}

#[test]
fn remove_override_without_override_keeps_active() {
    // 覆蓋本來就不存在時是 no-op（DELETE 回 204），不算「被取消」，不得清除目前 task。
    let mut state = state();
    state.set_active(&pid(), &ws("be"), &tid("t1")).unwrap();
    let before = state.clone();

    state.remove_override(&pid(), &ws("be"));

    assert_eq!(active_of(&state, "be"), Some(tid("t1")));
    assert_eq!(state, before);
}

#[test]
fn clear_active_is_noop_when_nothing_active() {
    let mut state = state();
    let before = state.clone();

    state.clear_active(&pid(), &ws("be"));

    assert_eq!(state, before);
}

#[test]
fn set_same_override_keeps_active_and_state_unchanged() {
    // spec 「重設相同覆蓋不清」：綁定沒變，agent 也沒變，無副作用。
    let mut state = state();
    state.set_override(&pid(), &ws("be"), some_override());
    state.set_active(&pid(), &ws("be"), &tid("t1")).unwrap();
    let before = state.clone();

    state.set_override(&pid(), &ws("be"), some_override());

    assert_eq!(active_of(&state, "be"), Some(tid("t1")));
    assert_eq!(state, before);
}

#[test]
fn set_different_override_still_clears_active() {
    let mut state = state();
    state.set_override(&pid(), &ws("be"), some_override());
    state.set_active(&pid(), &ws("be"), &tid("t1")).unwrap();

    state.set_override(
        &pid(),
        &ws("be"),
        Override {
            runtime: RuntimeId::new("win"),
            pane_id: PaneId::new("w2:p2"),
        },
    );

    assert_eq!(active_of(&state, "be"), None);
}
