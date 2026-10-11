//! repo-projects task 4.2（design D3、D4、D6）：core 這一側的 Repo Project 操作規則——
//!
//! - 進度讀寫依生效 def 的種類分流（`set_active`／`apply_progress` 走 `progress_for`），Repo Project 的
//!   目前 task 不保存（標記 none 即目前 task，宣告是空操作）；
//! - 由名稱產生 id（spec `repo-projects`「Repo Project 的 id 產生」）；
//! - `PATCH` 的 stage 對應（「修改 Repo Project 名稱與 stages」）；
//! - 清除消失 pane 的進度（「Repo Project 進度的保存與清除」），依據 `RuntimeStore` 當下的連線狀態與 pane 樹。

mod common;

use std::collections::{HashMap, HashSet};
use std::time::{Duration, SystemTime};

use cockpit_core::{
    ConnectionState, DomainState, Mark, OpenSpecPhase, PaneRepo, PaneRepos, ProgressOp, ProjectDef,
    ProjectId, Rejection, RepoKey, RepoProjectDef, RuntimeStore, StageEdit, TaskDef, TaskId,
    TaskProgress, WorkstreamDef, WorkstreamId, apply_stage_edits, derive_repo_project_id,
    repo_project_phases_valid,
};

use common::{empty_focused, pane, pane_id, runtime_id, snapshot};

const APP_REPO: &str = r"d:\work\app\.git";

fn s(v: &str) -> String {
    v.to_string()
}

fn connected() -> ConnectionState {
    ConnectionState::Connected {
        since: SystemTime::UNIX_EPOCH,
        server_version: s("0.9.0"),
        protocol: 1,
        last_snapshot_at: SystemTime::UNIX_EPOCH,
        settled: true,
        protocol_warning: None,
    }
}

fn disconnected() -> ConnectionState {
    ConnectionState::Disconnected {
        reason: s("事件流結束"),
        retry_in: Duration::from_secs(5),
    }
}

fn config_project(id: &str) -> ProjectDef {
    ProjectDef {
        id: ProjectId::new(id),
        name: s(id),
        stages: vec![s("A"), s("B")],
        workstreams: vec![WorkstreamDef {
            id: WorkstreamId::new("w"),
            name: s("w"),
            binding: None,
            pinned_pane: None,
        }],
        tasks: vec![TaskDef {
            id: TaskId::new("t1"),
            title: s("t1"),
            workstream: WorkstreamId::new("w"),
            stage: s("A"),
            depends_on: Vec::new(),
        }],
        repo: None,
    }
}

fn progress(stage: &str, mark: Mark) -> TaskProgress {
    TaskProgress {
        stage: s(stage),
        mark,
    }
}

/// Repo Project `app`（stages `Plan`→`Build`→`Review`），`local` 的 `wJ:p1` 歸入它；`config` 為手寫 project。
fn domain_with_app(config: Vec<ProjectDef>) -> DomainState {
    let mut domain = DomainState::from_projects(config);
    domain.repo_projects = vec![RepoProjectDef {
        id: ProjectId::new("app"),
        name: s("app"),
        repo: RepoKey::new(APP_REPO),
        stages: vec![s("Plan"), s("Build"), s("Review")],
        phases: vec![None; 3],
    }];
    let mut pane_repos = PaneRepos::new();
    pane_repos.insert(
        (runtime_id("local"), pane_id("wJ:p1")),
        PaneRepo {
            repo: RepoKey::new(APP_REPO),
            default_name: s("app"),
            worktree: None,
            root: None,
        },
    );
    domain.pane_repos = pane_repos;
    domain.refresh_projects();
    domain
}

fn app() -> ProjectId {
    ProjectId::new("app")
}

fn p1() -> TaskId {
    TaskId::new("local~wJ:p1")
}

// ---------------------------------------------------------------------------
// 進度分流（控制端裁決：單一判斷點 `progress_for`）
// ---------------------------------------------------------------------------

/// spec「畫面進度操作」：Repo Project task 的進度操作寫進 `repo_progress`，不寫 `progress`。
#[test]
fn apply_progress_on_repo_task_writes_repo_progress() {
    let mut domain = domain_with_app(Vec::new());
    domain
        .repo_progress
        .entry(app())
        .or_default()
        .insert(p1(), progress("Plan", Mark::None));

    domain
        .apply_progress(&app(), &p1(), ProgressOp::Advance)
        .expect("推進應被接受");

    assert_eq!(
        domain.repo_progress[&app()][&p1()],
        progress("Build", Mark::None)
    );
    assert!(
        !domain.progress.contains_key(&app()),
        "不得寫進手寫 project 的進度表"
    );
}

/// spec「撞名時兩邊進度互不影響」：撞名時生效的是手寫 `app`，操作只動 `progress`。
#[test]
fn apply_progress_on_conflicting_config_project_leaves_hidden_repo_progress() {
    let mut domain = domain_with_app(vec![config_project("app")]);
    domain
        .repo_progress
        .entry(app())
        .or_default()
        .insert(p1(), progress("Review", Mark::None));

    domain
        .apply_progress(&app(), &TaskId::new("t1"), ProgressOp::Complete)
        .expect("完成應被接受");

    assert_eq!(
        domain.progress[&app()][&TaskId::new("t1")].mark,
        Mark::Completed
    );
    assert_eq!(
        domain.repo_progress[&app()][&p1()],
        progress("Review", Mark::None)
    );
}

/// spec「宣告目前 task 是空操作」：標記 none 時通過，但不保存 `active`。
#[test]
fn set_active_on_unmarked_repo_task_is_a_no_op() {
    let mut domain = domain_with_app(Vec::new());
    domain
        .repo_progress
        .entry(app())
        .or_default()
        .insert(p1(), progress("Plan", Mark::None));
    let before = domain.clone();

    domain
        .set_active(&app(), &WorkstreamId::new("local~wJ:p1"), &p1())
        .expect("標記 none 時宣告應通過");

    assert_eq!(domain, before, "Repo Project 不保存目前 task");
}

/// spec「已標記的 task 不能宣告」：標記看的是 `repo_progress`（手寫進度表裡沒有這個 task）。
#[test]
fn set_active_on_marked_repo_task_is_rejected() {
    let mut domain = domain_with_app(Vec::new());
    domain
        .repo_progress
        .entry(app())
        .or_default()
        .insert(p1(), progress("Plan", Mark::Completed));
    let before = domain.clone();

    let error = domain
        .set_active(&app(), &WorkstreamId::new("local~wJ:p1"), &p1())
        .expect_err("已標記應被拒絕");

    assert_eq!(error, Rejection::AlreadyMarked);
    assert_eq!(domain, before);
}

// ---------------------------------------------------------------------------
// id 產生（design D6）
// ---------------------------------------------------------------------------

#[test]
fn id_replaces_illegal_characters_and_merges_dashes() {
    assert_eq!(
        derive_repo_project_id("My App (v2)", |_| false).as_str(),
        "My-App-v2"
    );
    assert_eq!(
        derive_repo_project_id("--a__b--c--", |_| false).as_str(),
        "a__b-c"
    );
}

#[test]
fn id_of_only_illegal_characters_is_repo() {
    assert_eq!(
        derive_repo_project_id("前端專案", |_| false).as_str(),
        "repo"
    );
}

#[test]
fn id_is_truncated_to_48_characters() {
    let name = "a".repeat(60);
    assert_eq!(
        derive_repo_project_id(&name, |_| false).as_str(),
        "a".repeat(48)
    );
}

#[test]
fn id_gets_numeric_suffix_when_taken() {
    let taken: HashSet<&str> = HashSet::from(["app", "app-2"]);
    assert_eq!(
        derive_repo_project_id("app", |id| taken.contains(id)).as_str(),
        "app-3"
    );
    let taken: HashSet<&str> = HashSet::from(["repo"]);
    assert_eq!(
        derive_repo_project_id("前端", |id| taken.contains(id)).as_str(),
        "repo-2"
    );
}

// ---------------------------------------------------------------------------
// stage 對應（design D6）
// ---------------------------------------------------------------------------

fn edit(name: &str, from: Option<&str>) -> StageEdit {
    edit_p(name, from, None)
}

fn edit_p(name: &str, from: Option<&str>, phase: Option<OpenSpecPhase>) -> StageEdit {
    StageEdit {
        name: s(name),
        from: from.map(s),
        phase,
    }
}

fn stages(list: &[&str]) -> Vec<String> {
    list.iter().map(|v| s(v)).collect()
}

/// spec「stage 改名、新增、刪除、排序」。
#[test]
fn stage_edits_rename_add_delete() {
    let remap = apply_stage_edits(
        &stages(&["Plan", "Implement", "Review", "Done"]),
        &[
            edit("Plan", Some("Plan")),
            edit("Design", None),
            edit("Build", Some("Implement")),
            edit("Done", Some("Done")),
        ],
    )
    .expect("合法的編輯");

    assert_eq!(remap.stages(), stages(&["Plan", "Design", "Build", "Done"]));
    assert_eq!(remap.stage_for("Plan"), "Plan");
    assert_eq!(remap.stage_for("Implement"), "Build");
    assert_eq!(remap.stage_for("Review"), "Plan", "被刪除 → 第一個新 stage");
    assert_eq!(remap.stage_for("Done"), "Done");
}

/// spec「重新排序保留 task 所在 stage」。
#[test]
fn stage_edits_reorder_keeps_stage() {
    let remap = apply_stage_edits(
        &stages(&["A", "B"]),
        &[edit("B", Some("B")), edit("A", Some("A"))],
    )
    .expect("合法的編輯");
    assert_eq!(remap.stages(), stages(&["B", "A"]));
    assert_eq!(remap.stage_for("B"), "B");
}

/// spec「前後空白被去除」對 stage 名稱同樣適用。
#[test]
fn stage_edits_trim_names() {
    let remap = apply_stage_edits(&stages(&["A"]), &[edit("  A2 ", Some("A"))]).expect("合法");
    assert_eq!(remap.stages(), stages(&["A2"]));
    assert_eq!(remap.stage_for("A"), "A2");
}

/// spec「同一個舊名稱被引用兩次」「from 不是現有 stage」與一般 stage 規則。
#[test]
fn stage_edits_reject_bad_from_and_bad_names() {
    let current = stages(&["Plan", "Build"]);
    assert!(
        apply_stage_edits(
            &current,
            &[edit("P1", Some("Plan")), edit("P2", Some("Plan"))]
        )
        .is_none(),
        "同一個舊名稱被引用兩次"
    );
    assert!(apply_stage_edits(&current, &[edit("X", Some("Nope"))]).is_none());
    assert!(apply_stage_edits(&current, &[]).is_none(), "空清單");
    assert!(
        apply_stage_edits(&current, &[edit("A", None), edit("A", None)]).is_none(),
        "名稱重複"
    );
    assert!(apply_stage_edits(&current, &[edit("Plan\u{7}", None)]).is_none());
}

// ---------------------------------------------------------------------------
// stage 對應 OpenSpec 階段（openspec-stage-sync task 3.1，design D1、D10-2）
// ---------------------------------------------------------------------------

use OpenSpecPhase::{Complete, Implement, Plan, Review};

fn phases(list: &[Option<OpenSpecPhase>]) -> Vec<Option<OpenSpecPhase>> {
    list.to_vec()
}

/// 階段在狀態檔與投影中的字串形式固定為小寫四值。
#[test]
fn phase_string_forms_are_stable() {
    for (phase, text) in [
        (Plan, "plan"),
        (Implement, "implement"),
        (Review, "review"),
        (Complete, "complete"),
    ] {
        assert_eq!(phase.as_str(), text);
        assert_eq!(OpenSpecPhase::parse(text), Some(phase));
        assert_eq!(
            serde_json::to_string(&phase).unwrap(),
            format!("\"{text}\"")
        );
        assert_eq!(
            serde_json::from_str::<OpenSpecPhase>(&format!("\"{text}\"")).unwrap(),
            phase
        );
    }
    assert_eq!(OpenSpecPhase::parse("done"), None);
    assert_eq!(OpenSpecPhase::parse("Plan"), None, "大小寫須完全相符");
    assert_eq!(OpenSpecPhase::parse(""), None);
}

/// spec「phases 長度與 stages 不同」「phases 內有重複的階段」。
#[test]
fn phases_must_align_and_be_unique() {
    let two = stages(&["A", "B"]);
    assert!(repo_project_phases_valid(&two, &[None, None]));
    assert!(repo_project_phases_valid(&two, &[Some(Plan), None]));
    assert!(repo_project_phases_valid(&two, &[Some(Plan), Some(Review)]));
    assert!(!repo_project_phases_valid(&two, &[None]), "太短");
    assert!(
        !repo_project_phases_valid(&two, &[None, None, None]),
        "太長"
    );
    assert!(
        !repo_project_phases_valid(&two, &[Some(Plan), Some(Plan)]),
        "非空值重複"
    );
}

/// spec「stage 改名、新增、刪除、排序」＋「phase 省略視為不對應」：每列帶自己的 phase，順序即結果順序。
#[test]
fn stage_edits_produce_phases_from_each_row() {
    let remap = apply_stage_edits(
        &stages(&["Plan", "Implement", "Review", "Done"]),
        &[
            edit_p("Plan", Some("Plan"), Some(Plan)),
            edit_p("Design", None, None),
            edit_p("Build", Some("Implement"), Some(Implement)),
            edit_p("Done", Some("Done"), Some(Complete)),
        ],
    )
    .expect("合法的編輯");
    assert_eq!(
        remap.phases(),
        phases(&[Some(Plan), None, Some(Implement), Some(Complete)])
    );
    assert_eq!(remap.phases().len(), remap.stages().len());

    // 省略 phase（None）不是沿用舊值。
    let omitted = apply_stage_edits(
        &stages(&["Plan", "Build"]),
        &[edit("Plan", Some("Plan")), edit("Build", Some("Build"))],
    )
    .expect("合法");
    assert_eq!(omitted.phases(), phases(&[None, None]));
}

/// spec「PATCH 的 phase 重複」→ `invalid_stages`（`apply_stage_edits` 回 `None`）。
#[test]
fn stage_edits_reject_duplicate_phases() {
    let current = stages(&["A", "B"]);
    assert!(
        apply_stage_edits(
            &current,
            &[
                edit_p("A", Some("A"), Some(Review)),
                edit_p("B", Some("B"), Some(Review)),
            ]
        )
        .is_none()
    );
    // 多列為 None 不算重複。
    assert!(apply_stage_edits(&current, &[edit("A", Some("A")), edit("B", Some("B"))]).is_some());
}

/// 取一份「修改前」的定義並回報這次編輯是否改變階段對應。
fn changed(old_stages: &[&str], old_phases: &[Option<OpenSpecPhase>], edits: &[StageEdit]) -> bool {
    let old = stages(old_stages);
    apply_stage_edits(&old, edits)
        .expect("合法的編輯")
        .phases_changed(&old, old_phases)
}

/// spec「改名但 phase 跟著走不算改變」。
#[test]
fn phases_unchanged_when_rename_follows_phase() {
    assert!(!changed(
        &["Plan", "Build"],
        &[Some(Plan), Some(Implement)],
        &[
            edit_p("Plan", Some("Plan"), Some(Plan)),
            edit_p("Make", Some("Build"), Some(Implement)),
        ],
    ));
}

/// spec「只重排不算改變」。
#[test]
fn phases_unchanged_when_only_reordered() {
    assert!(!changed(
        &["A", "B"],
        &[Some(Plan), Some(Review)],
        &[
            edit_p("B", Some("B"), Some(Review)),
            edit_p("A", Some("A"), Some(Plan)),
        ],
    ));
}

/// 沒有任何對應、也沒有被加上時不算改變（只改 stage 名稱／新增沒對應的站）。
#[test]
fn phases_unchanged_when_no_mapping_before_and_after() {
    assert!(!changed(
        &["A", "B"],
        &[None, None],
        &[edit("A", Some("A")), edit("C", None), edit("B2", Some("B"))],
    ));
}

/// spec「phase 換了擁有者算改變」。
#[test]
fn phases_changed_when_owner_swapped() {
    assert!(changed(
        &["A", "B"],
        &[Some(Plan), Some(Review)],
        &[
            edit_p("A", Some("A"), Some(Review)),
            edit_p("B", Some("B"), Some(Plan)),
        ],
    ));
}

/// 原本有擁有者而改為沒有、或原本沒有而改由某列擁有，都算改變。
#[test]
fn phases_changed_when_gained_or_lost() {
    assert!(changed(
        &["A", "B"],
        &[Some(Plan), None],
        &[edit("A", Some("A")), edit("B", Some("B"))],
    ));
    assert!(changed(
        &["A", "B"],
        &[Some(Plan), None],
        &[
            edit_p("A", Some("A"), Some(Plan)),
            edit_p("B", Some("B"), Some(Review)),
        ],
    ));
}

/// `from: null` 的新列是新身分：即使名稱與舊 stage 相同，也不等於舊擁有者。
#[test]
fn phases_changed_when_new_row_takes_over_with_same_name() {
    assert!(changed(
        &["A", "B"],
        &[Some(Plan), None],
        &[edit_p("A", None, Some(Plan)), edit("B", Some("B"))],
    ));
    // 舊擁有者被刪除、由新列接手 → 改變。
    assert!(changed(
        &["A", "B"],
        &[Some(Plan), None],
        &[edit("B", Some("B")), edit_p("C", None, Some(Plan))],
    ));
}

/// 修改前擁有 phase 的 stage 被刪除、修改後也沒有任何 stage 擁有它時，兩邊都沒有擁有者，視為不變（spec）。
/// 其他 phase 不受影響時整體不變。
#[test]
fn phases_unchanged_when_owner_deleted_and_nobody_owns_it() {
    assert!(!changed(
        &["A", "B"],
        &[Some(Plan), Some(Review)],
        &[edit_p("B", Some("B"), Some(Review))],
    ));
}

/// 同時有一個 phase 沒變、另一個變了，整體算改變（任一不同即可）。
#[test]
fn phases_changed_when_any_single_phase_differs() {
    assert!(changed(
        &["A", "B", "C"],
        &[Some(Plan), Some(Implement), Some(Review)],
        &[
            edit_p("A", Some("A"), Some(Plan)),
            edit_p("B", Some("B"), Some(Review)),
            edit_p("C", Some("C"), Some(Implement)),
        ],
    ));
}

// ---------------------------------------------------------------------------
// 清除消失 pane 的進度（design D4）
// ---------------------------------------------------------------------------

/// `local` 已連線、pane 樹為 `panes`（`(id, exited)`）；`wsl` 依 `wsl` 參數決定連線狀態、pane 樹空。
fn store(panes: &[(&str, bool)], wsl: Option<ConnectionState>) -> RuntimeStore {
    let mut store = RuntimeStore::new();
    let local = runtime_id("local");
    store.register(local.clone(), s("herdr"), s("test"));
    let panes = panes
        .iter()
        .map(|(id, exited)| {
            let mut p = pane(id, "w1", "t1");
            p.exited = *exited;
            p
        })
        .collect();
    store
        .replace(
            &local,
            snapshot(Vec::new(), Vec::new(), panes, Vec::new(), empty_focused()),
        )
        .expect("已登記");
    store.set_connection(&local, connected()).expect("已登記");
    if let Some(state) = wsl {
        let wsl_id = runtime_id("wsl");
        store.register(wsl_id.clone(), s("herdr"), s("test"));
        store.set_connection(&wsl_id, state).expect("已登記");
    }
    store
}

fn repo_progress_of(entries: &[(&str, &str)]) -> HashMap<TaskId, TaskProgress> {
    entries
        .iter()
        .map(|(id, stage)| (TaskId::new(*id), progress(stage, Mark::None)))
        .collect()
}

/// spec「pane 關閉，進度一併移除」「已 exited 但仍在 pane 樹中不清除」。
#[test]
fn clears_only_panes_missing_from_a_connected_runtime() {
    let mut domain = domain_with_app(Vec::new());
    domain.repo_progress.insert(
        app(),
        repo_progress_of(&[
            ("local~wJ:p1", "Review"),
            ("local~wJ:p2", "Build"),
            ("local~wJ:p3", "Plan"),
        ]),
    );

    let cleared =
        domain.clear_vanished_repo_progress(&store(&[("wJ:p1", false), ("wJ:p3", true)], None));

    assert!(cleared);
    let remaining: HashSet<&str> = domain.repo_progress[&app()]
        .keys()
        .map(TaskId::as_str)
        .collect();
    assert_eq!(remaining, HashSet::from(["local~wJ:p1", "local~wJ:p3"]));
}

/// spec「runtime 斷線不清除」、啟動初期（尚未連上）與未登記的 runtime 都保留；沒清任何東西時回 `false`
/// 且狀態不變。
#[test]
fn keeps_progress_of_runtimes_that_are_not_connected() {
    for wsl in [
        Some(disconnected()),
        Some(ConnectionState::Connecting),
        None,
    ] {
        let mut domain = domain_with_app(Vec::new());
        domain.repo_progress.insert(
            app(),
            repo_progress_of(&[("wsl~w1:p1", "Review"), ("local~wJ:p1", "Plan")]),
        );
        let before = domain.clone();

        let cleared = domain.clear_vanished_repo_progress(&store(&[("wJ:p1", false)], wsl.clone()));

        assert!(!cleared, "{wsl:?}");
        assert_eq!(domain, before, "{wsl:?}");
    }
}

/// repo-projects task 4.6：剛連上、沉降重拿還沒換上（`settled: false`）時 HERDR 的 pane 樹可能還不完整
/// （例如 server 剛重啟、pane 尚未恢復完），清除不可逆，所以不清；沉降重拿換上後同一棵樹才清。
#[test]
fn keeps_progress_until_connection_is_settled() {
    let unsettled = ConnectionState::Connected {
        since: SystemTime::UNIX_EPOCH,
        server_version: s("0.9.0"),
        protocol: 1,
        last_snapshot_at: SystemTime::UNIX_EPOCH,
        settled: false,
        protocol_warning: None,
    };
    let mut domain = domain_with_app(Vec::new());
    domain.repo_progress.insert(
        app(),
        repo_progress_of(&[("wsl~w1:p1", "Review"), ("local~wJ:p1", "Plan")]),
    );
    let before = domain.clone();

    let cleared = domain.clear_vanished_repo_progress(&store(&[("wJ:p1", false)], Some(unsettled)));

    assert!(!cleared, "沉降前不清");
    assert_eq!(domain, before);

    let cleared =
        domain.clear_vanished_repo_progress(&store(&[("wJ:p1", false)], Some(connected())));

    assert!(cleared, "沉降後清掉 wsl 樹中沒有的 pane");
    let remaining: Vec<&str> = domain.repo_progress[&app()]
        .keys()
        .map(TaskId::as_str)
        .collect();
    assert_eq!(remaining, vec!["local~wJ:p1"]);
}

/// 被撞名隱藏的 Repo Project 的進度同樣適用清除規則；清空的進度表一併移除，手寫 project 的進度不受影響。
#[test]
fn clears_hidden_repo_project_progress_but_not_config_progress() {
    let mut domain = domain_with_app(vec![config_project("app")]);
    domain
        .repo_progress
        .insert(app(), repo_progress_of(&[("local~wJ:p9", "Review")]));
    let config_progress = domain.progress.clone();

    let cleared = domain.clear_vanished_repo_progress(&store(&[], None));

    assert!(cleared);
    assert!(!domain.repo_progress.contains_key(&app()), "清空的表移除");
    assert_eq!(domain.progress, config_progress);
}

/// runtime id 含 `~` 時從最後一個 `~` 切出 runtime（design D3）。
#[test]
fn runtime_id_with_tilde_is_split_at_the_last_tilde() {
    let mut store = store(&[], None);
    let dev = runtime_id("dev~1");
    store.register(dev.clone(), s("herdr"), s("test"));
    store
        .replace(
            &dev,
            snapshot(
                Vec::new(),
                Vec::new(),
                vec![pane("wJ:p1", "w1", "t1")],
                Vec::new(),
                empty_focused(),
            ),
        )
        .expect("已登記");
    store.set_connection(&dev, connected()).expect("已登記");
    let mut domain = domain_with_app(Vec::new());
    domain.repo_progress.insert(
        app(),
        repo_progress_of(&[("dev~1~wJ:p1", "Review"), ("dev~1~wJ:p2", "Plan")]),
    );

    assert!(domain.clear_vanished_repo_progress(&store));
    let remaining: Vec<&str> = domain.repo_progress[&app()]
        .keys()
        .map(TaskId::as_str)
        .collect();
    assert_eq!(remaining, vec!["dev~1~wJ:p1"]);
}
