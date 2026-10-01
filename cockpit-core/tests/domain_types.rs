//! RED→GREEN 驗收測試（Task 2.1，change `pipeline-projection`）：`cockpit-core/src/domain/`
//! 型別的序列化字串，以及 `DomainState` 的初始進度＝起始 stage＋`none`。
//! 不測任何函數本體（`apply_op`／`resolve_binding`／`validate_override`／`derive_status` 留給
//! 2.2–2.4），只測型別本身存在、序列化字串正確、`DomainState` 的初始建構規則。

use cockpit_core::domain::{
    BindingResolution, BindingSource, BindingSpec, DomainState, Mark, Override, ProgressOp,
    ProjectDef, ProjectId, Rejection, StageStatus, TaskDef, TaskId, TaskProgress, WorkstreamDef,
    WorkstreamId,
};
use cockpit_core::{PaneId, RuntimeId};

#[test]
fn mark_serializes_lowercase_and_defaults_to_none() {
    let cases: [(Mark, &str); 3] = [
        (Mark::None, "none"),
        (Mark::Completed, "completed"),
        (Mark::Failed, "failed"),
    ];

    for (mark, expected) in cases {
        let json = serde_json::to_string(&mark).expect("serialize Mark");
        assert_eq!(json, format!("\"{expected}\""));

        let round_tripped: Mark = serde_json::from_str(&json).expect("deserialize Mark");
        assert_eq!(round_tripped, mark);
    }

    assert_eq!(Mark::default(), Mark::None);
}

#[test]
fn stage_status_has_exactly_six_lowercase_values() {
    let cases: [(StageStatus, &str); 6] = [
        (StageStatus::Pending, "pending"),
        (StageStatus::Ready, "ready"),
        (StageStatus::Running, "running"),
        (StageStatus::Blocked, "blocked"),
        (StageStatus::Failed, "failed"),
        (StageStatus::Completed, "completed"),
    ];

    let mut names = std::collections::BTreeSet::new();
    for (status, expected) in cases {
        let json = serde_json::to_string(&status).expect("serialize StageStatus");
        assert_eq!(json, format!("\"{expected}\""));

        let round_tripped: StageStatus =
            serde_json::from_str(&json).expect("deserialize StageStatus");
        assert_eq!(round_tripped, status);

        names.insert(expected);
    }
    assert_eq!(names.len(), 6);
}

#[test]
fn progress_op_serializes_to_spec_path_strings() {
    let cases: [(ProgressOp, &str); 4] = [
        (ProgressOp::Advance, "advance"),
        (ProgressOp::Complete, "complete"),
        (ProgressOp::Fail, "fail"),
        (ProgressOp::Clear, "clear"),
    ];

    for (op, expected) in cases {
        let json = serde_json::to_string(&op).expect("serialize ProgressOp");
        assert_eq!(json, format!("\"{expected}\""));

        let round_tripped: ProgressOp =
            serde_json::from_str(&json).expect("deserialize ProgressOp");
        assert_eq!(round_tripped, op);
    }
}

#[test]
fn binding_resolution_has_exactly_five_variants() {
    let runtime = RuntimeId::new("win");
    let pane = PaneId::new("wJ:p1");

    // 沒有 `_` 分支：未來多加變體會讓這裡編譯失敗，逼著回來更新窮舉。
    let variants = [
        BindingResolution::None,
        BindingResolution::RuntimeDisconnected {
            runtime: runtime.clone(),
            source: BindingSource::Auto,
        },
        BindingResolution::Bound {
            runtime: runtime.clone(),
            pane_id: pane.clone(),
            source: BindingSource::Auto,
        },
        BindingResolution::Unbound {
            runtime: runtime.clone(),
        },
        BindingResolution::Ambiguous {
            runtime,
            candidates: vec![pane],
        },
    ];

    let mut names = std::collections::BTreeSet::new();
    for variant in &variants {
        let name = match variant {
            BindingResolution::None => "none",
            BindingResolution::RuntimeDisconnected { .. } => "runtime_disconnected",
            BindingResolution::Bound { .. } => "bound",
            BindingResolution::Unbound { .. } => "unbound",
            BindingResolution::Ambiguous { .. } => "ambiguous",
        };
        names.insert(name);
    }
    assert_eq!(names.len(), 5);
}

#[test]
fn rejection_covers_progress_and_override_reasons() {
    // 六種拒絕原因都存在，且 Display 產生非空字串（供 HTTP 409 body 直接使用）。
    let reasons = [
        Rejection::AlreadyLastStage,
        Rejection::AlreadyMarked,
        Rejection::RuntimeNotRegistered,
        Rejection::RuntimeNotConnected,
        Rejection::PaneNotFound,
        Rejection::PaneExited,
    ];

    for reason in reasons {
        assert!(!reason.to_string().is_empty());
    }
}

#[test]
fn override_holds_runtime_and_pane() {
    let over = Override {
        runtime: RuntimeId::new("win"),
        pane_id: PaneId::new("wJ:p2"),
    };
    assert_eq!(over.runtime, RuntimeId::new("win"));
    assert_eq!(over.pane_id, PaneId::new("wJ:p2"));
}

fn sample_project() -> ProjectDef {
    ProjectDef {
        id: ProjectId::new("p"),
        name: "p".to_string(),
        stages: vec!["Spec".to_string(), "Plan".to_string(), "Build".to_string()],
        workstreams: vec![WorkstreamDef {
            id: WorkstreamId::new("be"),
            name: "be".to_string(),
            binding: Some(BindingSpec {
                runtime: RuntimeId::new("win"),
                workspace: "ai-cockpit".to_string(),
                pane_label: None,
                cwd: None,
                agent: None,
            }),
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

#[test]
fn domain_state_initial_progress_is_starting_stage_and_none() {
    let project = sample_project();
    let state = DomainState::from_projects(vec![project]);

    let progress = state
        .progress
        .get(&ProjectId::new("p"))
        .and_then(|tasks| tasks.get(&TaskId::new("t1")))
        .expect("t1 應該有初始進度");

    assert_eq!(
        progress,
        &TaskProgress {
            stage: "Plan".to_string(),
            mark: Mark::None
        }
    );
    assert!(state.overrides.is_empty());
    assert!(
        state
            .warnings
            .get(&ProjectId::new("p"))
            .is_none_or(Vec::is_empty)
    );
}
