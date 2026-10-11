//! openspec-stage-sync task 3.2（design D2、D3、D10）：`DomainState` 的同步狀態與「自動移動／手動優先」純函式
//! 狀態轉移。測試名稱對應 spec `openspec-stage-sync` 與 `repo-projects` 的 scenario。
//!
//! 全部在 `DomainState` 上直接呼叫（不經 `ProgressService`、不碰 IO、不讀時間）；服務層接線屬於 task 4.3。

mod common;

use std::time::SystemTime;

use cockpit_core::{
    ConnectionState, DomainState, Mark, Observation, OpenSpecPhase, PaneRepo, PaneRepos,
    ProgressOp, ProjectDef, ProjectId, RepoKey, RepoProjectDef, RuntimeStore, SyncMode, TaskDef,
    TaskId, TaskProgress, TaskSync, WorkstreamDef, WorkstreamId,
};

use common::{empty_focused, pane, pane_id, runtime_id, snapshot};

const APP_REPO: &str = r"d:\work\app\.git";

fn s(v: &str) -> String {
    v.to_string()
}

fn app() -> ProjectId {
    ProjectId::new("app")
}

fn p1() -> TaskId {
    TaskId::new("local~wJ:p1")
}

fn p2() -> TaskId {
    TaskId::new("local~wJ:p2")
}

fn four_stages() -> Vec<String> {
    vec![s("規劃"), s("實作"), s("審查"), s("完成")]
}

fn four_phases() -> Vec<Option<OpenSpecPhase>> {
    OpenSpecPhase::ALL.into_iter().map(Some).collect()
}

fn obs(change: &str, phase: OpenSpecPhase, checked: u32, total: u32) -> Observation {
    Observation {
        change: s(change),
        phase,
        checked,
        total,
    }
}

fn progress(stage: &str, mark: Mark) -> TaskProgress {
    TaskProgress {
        stage: s(stage),
        mark,
    }
}

fn sync_of(mode: SyncMode, applied: Option<Observation>) -> TaskSync {
    TaskSync { mode, applied }
}

/// Repo Project `app`（指定 stages 與 phases），`local` 的 `wJ:p1`、`wJ:p2` 歸入它；沒有任何進度與同步狀態。
fn domain_with(stages: Vec<String>, phases: Vec<Option<OpenSpecPhase>>) -> DomainState {
    let repo_projects = vec![RepoProjectDef {
        id: app(),
        name: s("app"),
        repo: RepoKey::new(APP_REPO),
        stages,
        phases,
    }];
    let mut pane_repos = PaneRepos::new();
    for pane in ["wJ:p1", "wJ:p2"] {
        pane_repos.insert(
            (runtime_id("local"), pane_id(pane)),
            PaneRepo {
                repo: RepoKey::new(APP_REPO),
                default_name: s("app"),
                worktree: None,
                root: None,
            },
        );
    }
    let mut domain = DomainState {
        repo_projects,
        pane_repos,
        ..DomainState::default()
    };
    domain.refresh_projects();
    domain
}

fn domain_default() -> DomainState {
    domain_with(four_stages(), four_phases())
}

fn set_progress(domain: &mut DomainState, task: &TaskId, stage: &str, mark: Mark) {
    domain
        .repo_progress
        .entry(app())
        .or_default()
        .insert(task.clone(), progress(stage, mark));
}

fn set_sync(domain: &mut DomainState, task: &TaskId, sync: TaskSync) {
    domain
        .repo_sync
        .entry(app())
        .or_default()
        .insert(task.clone(), sync);
}

fn progress_of(domain: &DomainState, task: &TaskId) -> Option<TaskProgress> {
    domain
        .repo_progress
        .get(&app())
        .and_then(|t| t.get(task))
        .cloned()
}

fn sync_in(domain: &DomainState, task: &TaskId) -> Option<TaskSync> {
    domain
        .repo_sync
        .get(&app())
        .and_then(|t| t.get(task))
        .cloned()
}

// ---------------------------------------------------------------------------
// D3 第 4 條：自動移動
// ---------------------------------------------------------------------------

/// spec「新卡片第一次偵測就移到對應站」。
#[test]
fn new_card_first_observation_moves_to_mapped_stage() {
    let mut domain = domain_default();
    let seen = obs("foo", OpenSpecPhase::Implement, 3, 8);

    let changed = domain.apply_openspec(&app(), &p1(), Some(&seen));

    assert!(changed);
    assert_eq!(
        progress_of(&domain, &p1()),
        Some(progress("實作", Mark::None))
    );
    assert_eq!(
        sync_in(&domain, &p1()),
        Some(sync_of(SyncMode::Auto, Some(seen)))
    );
}

/// spec「勾選數變化但階段不變」。
#[test]
fn checkbox_change_within_same_phase_updates_applied_only() {
    let mut domain = domain_default();
    set_progress(&mut domain, &p1(), "實作", Mark::None);
    set_sync(
        &mut domain,
        &p1(),
        sync_of(
            SyncMode::Auto,
            Some(obs("foo", OpenSpecPhase::Implement, 3, 8)),
        ),
    );
    let next = obs("foo", OpenSpecPhase::Implement, 4, 8);

    assert!(domain.apply_openspec(&app(), &p1(), Some(&next)));

    assert_eq!(
        progress_of(&domain, &p1()),
        Some(progress("實作", Mark::None))
    );
    assert_eq!(
        sync_in(&domain, &p1()),
        Some(sync_of(SyncMode::Auto, Some(next)))
    );
}

/// spec「階段改變時跨站移動」：規劃直接到審查，不經過實作。
#[test]
fn phase_jump_moves_across_stages() {
    let mut domain = domain_default();
    set_progress(&mut domain, &p1(), "規劃", Mark::None);
    set_sync(
        &mut domain,
        &p1(),
        sync_of(SyncMode::Auto, Some(obs("foo", OpenSpecPhase::Plan, 0, 5))),
    );

    assert!(domain.apply_openspec(
        &app(),
        &p1(),
        Some(&obs("foo", OpenSpecPhase::Review, 5, 5))
    ));

    assert_eq!(
        progress_of(&domain, &p1()),
        Some(progress("審查", Mark::None))
    );
}

/// spec「階段倒退時移回」。
#[test]
fn phase_regression_moves_back() {
    let mut domain = domain_default();
    set_progress(&mut domain, &p1(), "審查", Mark::None);
    set_sync(
        &mut domain,
        &p1(),
        sync_of(
            SyncMode::Auto,
            Some(obs("foo", OpenSpecPhase::Review, 5, 5)),
        ),
    );

    assert!(domain.apply_openspec(
        &app(),
        &p1(),
        Some(&obs("foo", OpenSpecPhase::Implement, 4, 5))
    ));

    assert_eq!(
        progress_of(&domain, &p1()),
        Some(progress("實作", Mark::None))
    );
}

/// spec「archive 移到完成站但不標 Completed」：跨過最後一站前的推進限制，標記維持 none。
#[test]
fn archive_moves_to_complete_stage_without_marking_completed() {
    let mut domain = domain_default();
    set_progress(&mut domain, &p1(), "審查", Mark::None);
    set_sync(
        &mut domain,
        &p1(),
        sync_of(
            SyncMode::Auto,
            Some(obs("foo", OpenSpecPhase::Review, 8, 8)),
        ),
    );

    assert!(domain.apply_openspec(
        &app(),
        &p1(),
        Some(&obs("foo", OpenSpecPhase::Complete, 8, 8))
    ));

    assert_eq!(
        progress_of(&domain, &p1()),
        Some(progress("完成", Mark::None))
    );
}

/// spec「該階段沒有對應的 stage」：卡片不動，但 `applied` 與 `mode` 更新。
#[test]
fn unmapped_phase_updates_applied_and_mode_but_does_not_move() {
    let mut domain = domain_with(
        vec![s("Plan"), s("Build"), s("Done")],
        vec![
            Some(OpenSpecPhase::Plan),
            None,
            Some(OpenSpecPhase::Complete),
        ],
    );
    set_progress(&mut domain, &p1(), "Build", Mark::None);
    set_sync(&mut domain, &p1(), sync_of(SyncMode::Manual, None));
    let seen = obs("foo", OpenSpecPhase::Implement, 3, 8);

    assert!(domain.apply_openspec(&app(), &p1(), Some(&seen)));

    assert_eq!(
        progress_of(&domain, &p1()),
        Some(progress("Build", Mark::None))
    );
    assert_eq!(
        sync_in(&domain, &p1()),
        Some(sync_of(SyncMode::Auto, Some(seen)))
    );
}

/// spec「目前所在 stage 就是對應的 stage」：手動轉為自動，`applied` 更新。
#[test]
fn already_on_target_stage_updates_applied_and_turns_auto() {
    let mut domain = domain_default();
    set_progress(&mut domain, &p1(), "實作", Mark::None);
    set_sync(
        &mut domain,
        &p1(),
        sync_of(
            SyncMode::Manual,
            Some(obs("foo", OpenSpecPhase::Implement, 3, 8)),
        ),
    );
    let next = obs("foo", OpenSpecPhase::Implement, 4, 8);

    assert!(domain.apply_openspec(&app(), &p1(), Some(&next)));

    assert_eq!(
        progress_of(&domain, &p1()),
        Some(progress("實作", Mark::None))
    );
    assert_eq!(
        sync_in(&domain, &p1()),
        Some(sync_of(SyncMode::Auto, Some(next)))
    );
}

/// 偵測結果的比較含 change 名稱：換 change、階段與勾選都一樣也算不同（design D3 第 3 條）。
#[test]
fn different_change_name_counts_as_different_observation() {
    let mut domain = domain_default();
    set_progress(&mut domain, &p1(), "實作", Mark::None);
    set_sync(
        &mut domain,
        &p1(),
        sync_of(
            SyncMode::Auto,
            Some(obs("foo", OpenSpecPhase::Implement, 3, 8)),
        ),
    );
    let other = obs("bar", OpenSpecPhase::Implement, 3, 8);

    assert!(domain.apply_openspec(&app(), &p1(), Some(&other)));

    assert_eq!(sync_in(&domain, &p1()).and_then(|s| s.applied), Some(other));
}

// ---------------------------------------------------------------------------
// D3 第 1、2、3 條：不改變任何東西
// ---------------------------------------------------------------------------

/// spec「無結果不改變任何東西」。
#[test]
fn no_observation_changes_nothing() {
    let mut domain = domain_default();
    set_progress(&mut domain, &p1(), "實作", Mark::None);
    set_sync(
        &mut domain,
        &p1(),
        sync_of(
            SyncMode::Auto,
            Some(obs("foo", OpenSpecPhase::Implement, 3, 8)),
        ),
    );
    let before = domain.clone();

    assert!(!domain.apply_openspec(&app(), &p1(), None));

    assert_eq!(domain, before);
}

/// 無結果也不為新 task 建立進度項目或同步狀態（沒用到功能的 Repo Project 行為完全不變）。
#[test]
fn no_observation_does_not_create_progress_or_sync_for_new_task() {
    let mut domain = domain_default();
    let before = domain.clone();

    assert!(!domain.apply_openspec(&app(), &p1(), None));

    assert_eq!(domain, before);
    assert!(domain.repo_progress.is_empty());
    assert!(domain.repo_sync.is_empty());
}

/// spec「結果相同不改變」：使用者在偵測到實作後手動推進到審查，同一個結果不把卡片拉回，模式維持手動。
#[test]
fn same_observation_changes_nothing_so_manual_position_holds() {
    let mut domain = domain_default();
    set_progress(&mut domain, &p1(), "審查", Mark::None);
    let seen = obs("foo", OpenSpecPhase::Implement, 3, 8);
    set_sync(
        &mut domain,
        &p1(),
        sync_of(SyncMode::Manual, Some(seen.clone())),
    );
    let before = domain.clone();

    assert!(!domain.apply_openspec(&app(), &p1(), Some(&seen)));

    assert_eq!(domain, before);
}

/// spec「標記期間不移動」：不移動、`applied` 不更新、標記不變。
#[test]
fn marked_task_is_not_moved_and_applied_is_not_updated() {
    for mark in [Mark::Completed, Mark::Failed] {
        let mut domain = domain_default();
        set_progress(&mut domain, &p1(), "實作", mark);
        set_sync(
            &mut domain,
            &p1(),
            sync_of(
                SyncMode::Auto,
                Some(obs("foo", OpenSpecPhase::Implement, 3, 8)),
            ),
        );
        let before = domain.clone();

        assert!(!domain.apply_openspec(
            &app(),
            &p1(),
            Some(&obs("foo", OpenSpecPhase::Review, 8, 8))
        ));

        assert_eq!(domain, before, "標記 {mark:?} 期間什麼都不改");
    }
}

/// spec「清除標記後恢復自動」：標記期間 `applied` 沒更新，清除後下一輪因結果不同而套用。
#[test]
fn clearing_mark_lets_next_round_apply() {
    let mut domain = domain_default();
    set_progress(&mut domain, &p1(), "實作", Mark::Completed);
    set_sync(
        &mut domain,
        &p1(),
        sync_of(
            SyncMode::Auto,
            Some(obs("foo", OpenSpecPhase::Implement, 3, 8)),
        ),
    );
    let latest = obs("foo", OpenSpecPhase::Review, 8, 8);
    assert!(!domain.apply_openspec(&app(), &p1(), Some(&latest)));

    domain
        .apply_progress(&app(), &p1(), ProgressOp::Clear)
        .expect("清除標記應被接受");
    assert!(domain.apply_openspec(&app(), &p1(), Some(&latest)));

    assert_eq!(
        progress_of(&domain, &p1()),
        Some(progress("審查", Mark::None))
    );
    assert_eq!(
        sync_in(&domain, &p1()),
        Some(sync_of(SyncMode::Auto, Some(latest)))
    );
}

// ---------------------------------------------------------------------------
// D2：同步狀態需要進度項目
// ---------------------------------------------------------------------------

/// spec「找不到對應 stage 時仍建立進度項目」：新 pane 沒有進度項目，也補初始進度；pane 消失時一起清除。
#[test]
fn unmapped_phase_for_new_task_still_creates_progress_and_sync_then_vanish_clears_both() {
    let mut domain = domain_with(
        vec![s("Plan"), s("Build"), s("Done")],
        vec![
            Some(OpenSpecPhase::Plan),
            None,
            Some(OpenSpecPhase::Complete),
        ],
    );
    let seen = obs("foo", OpenSpecPhase::Implement, 3, 8);

    assert!(domain.apply_openspec(&app(), &p1(), Some(&seen)));

    assert_eq!(
        progress_of(&domain, &p1()),
        Some(progress("Plan", Mark::None))
    );
    assert_eq!(
        sync_in(&domain, &p1()),
        Some(sync_of(SyncMode::Auto, Some(seen)))
    );

    // pane 從已連線且沉降的 runtime 的 pane 樹消失。
    assert!(domain.clear_vanished_repo_progress(&store(&[])));
    assert_eq!(progress_of(&domain, &p1()), None);
    assert_eq!(sync_in(&domain, &p1()), None);
    assert!(domain.repo_sync.is_empty(), "空表一併移除");
}

/// spec「目標就是第一個 stage」：不移動，但補進度項目與同步狀態，`drop_untouched_initial` 撤不掉。
#[test]
fn target_is_first_stage_creates_progress_and_sync_and_survives_drop_untouched_initial() {
    let mut domain = domain_default();
    let seen = obs("foo", OpenSpecPhase::Plan, 0, 5);

    assert!(domain.apply_openspec(&app(), &p1(), Some(&seen)));

    assert_eq!(
        progress_of(&domain, &p1()),
        Some(progress("規劃", Mark::None))
    );
    assert_eq!(
        sync_in(&domain, &p1()),
        Some(sync_of(SyncMode::Auto, Some(seen)))
    );

    // 與沒有任何人工操作的新 task 看起來相同的初始進度；有同步狀態就不得撤回。
    let def = domain
        .projects
        .iter()
        .find(|p| p.id == app())
        .expect("app 已展開")
        .clone();
    domain.drop_untouched_initial(&def, &p1(), &progress("規劃", Mark::None), true);
    assert_eq!(
        progress_of(&domain, &p1()),
        Some(progress("規劃", Mark::None))
    );
    assert!(sync_in(&domain, &p1()).is_some());
}

/// 對照組：沒有同步狀態的未動過初始進度照舊被撤回（既有規則不變）。
#[test]
fn drop_untouched_initial_still_removes_task_without_sync() {
    let mut domain = domain_default();
    set_progress(&mut domain, &p1(), "規劃", Mark::None);
    let def = domain
        .projects
        .iter()
        .find(|p| p.id == app())
        .expect("app 已展開")
        .clone();

    domain.drop_untouched_initial(&def, &p1(), &progress("規劃", Mark::None), true);

    assert_eq!(progress_of(&domain, &p1()), None);
    assert!(domain.repo_progress.is_empty());
}

/// 沒有進度項目時「目前 stage」視為第一個 stage：第一個 stage 之外的對應照常移動。
#[test]
fn task_without_progress_item_is_treated_as_on_first_stage() {
    let mut domain = domain_default();

    assert!(domain.apply_openspec(
        &app(),
        &p1(),
        Some(&obs("foo", OpenSpecPhase::Review, 5, 5))
    ));

    assert_eq!(
        progress_of(&domain, &p1()),
        Some(progress("審查", Mark::None))
    );
    // 另一張 task 不受影響。
    assert_eq!(progress_of(&domain, &p2()), None);
    assert_eq!(sync_in(&domain, &p2()), None);
}

// ---------------------------------------------------------------------------
// 防護：不觸發 apply_op 的 panic、手寫 project 不適用
// ---------------------------------------------------------------------------

/// 自動移動寫入的 stage 必須在生效 `stages` 內：階段對應指向不在 `stages` 內的名稱時視為沒有對應，
/// 之後的人工推進（`apply_op` 假設 stage 合法）不會 panic。
#[test]
fn target_stage_outside_effective_stages_is_treated_as_unmapped() {
    let mut domain = domain_default();
    // 只改定義、不 `refresh_projects`，模擬生效 stages 與定義暫時不一致。
    domain.repo_projects[0].stages = vec![s("規劃"), s("不存在的站"), s("審查"), s("完成")];
    let seen = obs("foo", OpenSpecPhase::Implement, 3, 8);

    assert!(domain.apply_openspec(&app(), &p1(), Some(&seen)));

    assert_eq!(
        progress_of(&domain, &p1()),
        Some(progress("規劃", Mark::None))
    );
    assert_eq!(
        sync_in(&domain, &p1()),
        Some(sync_of(SyncMode::Auto, Some(seen)))
    );
    domain
        .apply_progress(&app(), &p1(), ProgressOp::Advance)
        .expect("stage 合法，推進不得 panic");
}

/// 手寫 project 不適用：呼叫是 no-op，不產生同步狀態與進度。
#[test]
fn hand_written_project_is_a_no_op() {
    let config = ProjectDef {
        id: ProjectId::new("hand"),
        name: s("hand"),
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
    };
    let mut domain = DomainState::from_projects(vec![config]);
    let before = domain.clone();
    let seen = obs("foo", OpenSpecPhase::Implement, 3, 8);

    assert!(!domain.apply_openspec(&ProjectId::new("hand"), &TaskId::new("t1"), Some(&seen)));
    assert!(!domain.mark_manual(&ProjectId::new("hand"), &TaskId::new("t1")));

    assert_eq!(domain, before);
    assert!(domain.repo_sync.is_empty());
}

/// 被同 id 手寫 project 撞名隱藏的 Repo Project：生效的是手寫 project，不產生同步狀態。
#[test]
fn repo_project_hidden_by_config_project_is_a_no_op() {
    let config = ProjectDef {
        id: app(),
        name: s("app"),
        stages: vec![s("A"), s("B")],
        workstreams: Vec::new(),
        tasks: Vec::new(),
        repo: None,
    };
    let mut domain = domain_default();
    domain.projects.insert(0, config);
    domain.refresh_projects();
    let before = domain.clone();

    assert!(!domain.apply_openspec(
        &app(),
        &p1(),
        Some(&obs("foo", OpenSpecPhase::Implement, 3, 8))
    ));

    assert_eq!(domain, before);
}

/// 未知 project、不在展開 task 內的 id：no-op。
#[test]
fn unknown_project_or_task_is_a_no_op() {
    let mut domain = domain_default();
    let before = domain.clone();
    let seen = obs("foo", OpenSpecPhase::Implement, 3, 8);

    assert!(!domain.apply_openspec(&ProjectId::new("nope"), &p1(), Some(&seen)));
    assert!(!domain.apply_openspec(&app(), &TaskId::new("local~wJ:p9"), Some(&seen)));
    assert!(!domain.mark_manual(&ProjectId::new("nope"), &p1()));
    assert!(!domain.mark_manual(&app(), &TaskId::new("local~wJ:p9")));

    assert_eq!(domain, before);
}

// ---------------------------------------------------------------------------
// 手動優先
// ---------------------------------------------------------------------------

/// spec「人工推進後卡片留在手動位置」＋「偵測結果下一次變化時恢復自動」。
#[test]
fn manual_advance_holds_position_until_observation_changes() {
    let mut domain = domain_default();
    let seen = obs("foo", OpenSpecPhase::Implement, 3, 8);
    assert!(domain.apply_openspec(&app(), &p1(), Some(&seen)));
    assert_eq!(
        progress_of(&domain, &p1()),
        Some(progress("實作", Mark::None))
    );

    // 4.3 在同一個寫入閉包內：先 apply_progress，成功才標手動。
    domain
        .apply_progress(&app(), &p1(), ProgressOp::Advance)
        .expect("推進應被接受");
    assert!(domain.mark_manual(&app(), &p1()));
    assert_eq!(
        sync_in(&domain, &p1()),
        Some(sync_of(SyncMode::Manual, Some(seen.clone())))
    );

    // 同一個結果：不拉回。
    assert!(!domain.apply_openspec(&app(), &p1(), Some(&seen)));
    assert_eq!(
        progress_of(&domain, &p1()),
        Some(progress("審查", Mark::None))
    );

    // 結果變化：移回實作並恢復自動。
    let next = obs("foo", OpenSpecPhase::Implement, 4, 8);
    assert!(domain.apply_openspec(&app(), &p1(), Some(&next)));
    assert_eq!(
        progress_of(&domain, &p1()),
        Some(progress("實作", Mark::None))
    );
    assert_eq!(
        sync_in(&domain, &p1()),
        Some(sync_of(SyncMode::Auto, Some(next)))
    );
}

/// spec「還沒有同步狀態時手動推進」：以 `openspec_obs` 中該 pane 當下的結果建立 `{Manual, applied}`，
/// 下一輪同一結果不把卡片拉走。
#[test]
fn manual_without_sync_state_captures_current_observation() {
    let mut domain = domain_default();
    let seen = obs("foo", OpenSpecPhase::Implement, 3, 8);
    domain
        .openspec_obs
        .insert((runtime_id("local"), pane_id("wJ:p1")), seen.clone());
    // 新 pane 的人工推進：先補初始進度再推進（服務層 `ensure_task` 的行為）。
    set_progress(&mut domain, &p1(), "規劃", Mark::None);
    domain
        .apply_progress(&app(), &p1(), ProgressOp::Advance)
        .expect("推進應被接受");

    assert!(domain.mark_manual(&app(), &p1()));

    assert_eq!(
        sync_in(&domain, &p1()),
        Some(sync_of(SyncMode::Manual, Some(seen.clone())))
    );
    assert!(!domain.apply_openspec(&app(), &p1(), Some(&seen)));
    assert_eq!(
        progress_of(&domain, &p1()),
        Some(progress("實作", Mark::None))
    );
}

/// spec「沒有偵測結果時手動推進」：`applied` 為無；之後結果首次出現就依自動規則套用並轉回自動。
#[test]
fn manual_without_observation_has_no_applied_then_first_observation_applies() {
    let mut domain = domain_default();
    set_progress(&mut domain, &p1(), "規劃", Mark::None);
    domain
        .apply_progress(&app(), &p1(), ProgressOp::Advance)
        .expect("推進應被接受");

    assert!(domain.mark_manual(&app(), &p1()));
    assert_eq!(
        sync_in(&domain, &p1()),
        Some(sync_of(SyncMode::Manual, None))
    );

    let first = obs("foo", OpenSpecPhase::Review, 5, 5);
    assert!(domain.apply_openspec(&app(), &p1(), Some(&first)));
    assert_eq!(
        progress_of(&domain, &p1()),
        Some(progress("審查", Mark::None))
    );
    assert_eq!(
        sync_in(&domain, &p1()),
        Some(sync_of(SyncMode::Auto, Some(first)))
    );
}

/// 已有同步狀態時只改 `mode`，`applied` 不變；已是手動再標一次回報沒有改變（呼叫端不必多落檔）。
#[test]
fn mark_manual_on_existing_sync_keeps_applied_and_reports_change_once() {
    let mut domain = domain_default();
    set_progress(&mut domain, &p1(), "審查", Mark::None);
    let applied = obs("foo", OpenSpecPhase::Implement, 3, 8);
    set_sync(
        &mut domain,
        &p1(),
        sync_of(SyncMode::Auto, Some(applied.clone())),
    );
    // 目前偵測結果與 applied 不同，也不得被拿來取代既有同步狀態的 applied。
    domain.openspec_obs.insert(
        (runtime_id("local"), pane_id("wJ:p1")),
        obs("foo", OpenSpecPhase::Review, 8, 8),
    );

    assert!(domain.mark_manual(&app(), &p1()));
    assert_eq!(
        sync_in(&domain, &p1()),
        Some(sync_of(SyncMode::Manual, Some(applied)))
    );
    let before = domain.clone();

    assert!(!domain.mark_manual(&app(), &p1()));
    assert_eq!(domain, before);
}

/// 建立同步狀態時沒有進度項目就補初始進度（design D2）。
#[test]
fn mark_manual_creates_missing_progress_item() {
    let mut domain = domain_default();

    assert!(domain.mark_manual(&app(), &p1()));

    assert_eq!(
        progress_of(&domain, &p1()),
        Some(progress("規劃", Mark::None))
    );
    assert_eq!(
        sync_in(&domain, &p1()),
        Some(sync_of(SyncMode::Manual, None))
    );
}

/// spec「標記與清除不改變模式」：`complete`／`fail`／`clear` 走 `apply_progress`，不碰 `repo_sync`。
#[test]
fn mark_operations_do_not_touch_sync_mode() {
    let mut domain = domain_default();
    set_progress(&mut domain, &p1(), "實作", Mark::None);
    set_sync(&mut domain, &p1(), sync_of(SyncMode::Auto, None));
    set_progress(&mut domain, &p2(), "實作", Mark::None);
    set_sync(&mut domain, &p2(), sync_of(SyncMode::Manual, None));

    for op in [ProgressOp::Complete, ProgressOp::Clear] {
        domain.apply_progress(&app(), &p1(), op).expect("接受");
    }
    for op in [ProgressOp::Fail, ProgressOp::Clear] {
        domain.apply_progress(&app(), &p2(), op).expect("接受");
    }

    assert_eq!(
        sync_in(&domain, &p1()).map(|s| s.mode),
        Some(SyncMode::Auto)
    );
    assert_eq!(
        sync_in(&domain, &p2()).map(|s| s.mode),
        Some(SyncMode::Manual)
    );
}

// ---------------------------------------------------------------------------
// 階段對應被修改時重新套用
// ---------------------------------------------------------------------------

/// spec「自動的卡片依新對應移動」＋「手動的卡片不動」。
#[test]
fn reset_auto_applied_clears_only_auto_tasks_then_reapplies_under_new_mapping() {
    let mut domain = domain_with(
        vec![s("A"), s("B"), s("C")],
        vec![
            Some(OpenSpecPhase::Plan),
            Some(OpenSpecPhase::Implement),
            Some(OpenSpecPhase::Review),
        ],
    );
    let seen = obs("foo", OpenSpecPhase::Implement, 3, 8);
    set_progress(&mut domain, &p1(), "B", Mark::None);
    set_sync(
        &mut domain,
        &p1(),
        sync_of(SyncMode::Auto, Some(seen.clone())),
    );
    set_progress(&mut domain, &p2(), "A", Mark::None);
    set_sync(
        &mut domain,
        &p2(),
        sync_of(SyncMode::Manual, Some(seen.clone())),
    );

    // PATCH 把對應改成 plan / review / implement。
    domain.repo_projects[0].phases = vec![
        Some(OpenSpecPhase::Plan),
        Some(OpenSpecPhase::Review),
        Some(OpenSpecPhase::Implement),
    ];
    assert!(domain.reset_auto_applied(&app()));

    assert_eq!(
        sync_in(&domain, &p1()),
        Some(sync_of(SyncMode::Auto, None)),
        "自動的 applied 清為無"
    );
    assert_eq!(
        sync_in(&domain, &p2()),
        Some(sync_of(SyncMode::Manual, Some(seen.clone()))),
        "手動的不動"
    );

    // 下一輪偵測：自動的移到新對應的 C；手動的同一結果不動。
    assert!(domain.apply_openspec(&app(), &p1(), Some(&seen)));
    assert!(!domain.apply_openspec(&app(), &p2(), Some(&seen)));
    assert_eq!(progress_of(&domain, &p1()), Some(progress("C", Mark::None)));
    assert_eq!(
        sync_in(&domain, &p1()),
        Some(sync_of(SyncMode::Auto, Some(seen)))
    );
    assert_eq!(progress_of(&domain, &p2()), Some(progress("A", Mark::None)));
}

/// 沒有任何可清的項目（無同步狀態、手動、或 applied 本來就是無）回報沒有改變。
#[test]
fn reset_auto_applied_reports_no_change_when_nothing_to_clear() {
    let mut domain = domain_default();
    assert!(!domain.reset_auto_applied(&app()));
    set_sync(&mut domain, &p1(), sync_of(SyncMode::Auto, None));
    set_sync(
        &mut domain,
        &p2(),
        sync_of(
            SyncMode::Manual,
            Some(obs("foo", OpenSpecPhase::Plan, 0, 1)),
        ),
    );
    let before = domain.clone();

    assert!(!domain.reset_auto_applied(&app()));

    assert_eq!(domain, before);
}

/// 只動指定 project 的同步狀態。
#[test]
fn reset_auto_applied_only_touches_the_given_project() {
    let mut domain = domain_default();
    let seen = obs("foo", OpenSpecPhase::Plan, 0, 1);
    domain
        .repo_sync
        .entry(ProjectId::new("other"))
        .or_default()
        .insert(p1(), sync_of(SyncMode::Auto, Some(seen.clone())));
    set_sync(
        &mut domain,
        &p1(),
        sync_of(SyncMode::Auto, Some(seen.clone())),
    );

    assert!(domain.reset_auto_applied(&app()));

    assert_eq!(
        domain.repo_sync[&ProjectId::new("other")][&p1()].applied,
        Some(seen)
    );
}

/// 移除 Repo Project 時同步狀態一併清掉。
#[test]
fn clear_repo_project_sync_removes_the_project_table() {
    let mut domain = domain_default();
    set_sync(&mut domain, &p1(), sync_of(SyncMode::Auto, None));
    domain
        .repo_sync
        .entry(ProjectId::new("other"))
        .or_default()
        .insert(p1(), sync_of(SyncMode::Auto, None));

    assert!(domain.clear_repo_project_sync(&app()));
    assert!(!domain.clear_repo_project_sync(&app()));

    assert!(!domain.repo_sync.contains_key(&app()));
    assert!(domain.repo_sync.contains_key(&ProjectId::new("other")));
}

// ---------------------------------------------------------------------------
// pane 消失一起清除
// ---------------------------------------------------------------------------

fn connected_settled() -> ConnectionState {
    ConnectionState::Connected {
        since: SystemTime::UNIX_EPOCH,
        server_version: s("0.9.0"),
        protocol: 1,
        last_snapshot_at: SystemTime::UNIX_EPOCH,
        settled: true,
        protocol_warning: None,
    }
}

/// `local` 已連線、沉降，pane 樹為 `panes`。
fn store(panes: &[&str]) -> RuntimeStore {
    let mut store = RuntimeStore::new();
    let local = runtime_id("local");
    store.register(local.clone(), s("herdr"), s("test"));
    let panes = panes.iter().map(|id| pane(id, "w1", "t1")).collect();
    store
        .replace(
            &local,
            snapshot(Vec::new(), Vec::new(), panes, Vec::new(), empty_focused()),
        )
        .expect("已登記");
    store
        .set_connection(&local, connected_settled())
        .expect("已登記");
    store
}

/// spec「pane 消失一起清除」：只清消失的 pane，留著的 pane 進度與同步狀態都在。
#[test]
fn vanished_pane_clears_progress_and_sync_together_but_keeps_present_pane() {
    let mut domain = domain_default();
    for task in [p1(), p2()] {
        set_progress(&mut domain, &task, "實作", Mark::None);
        set_sync(&mut domain, &task, sync_of(SyncMode::Auto, None));
    }

    assert!(domain.clear_vanished_repo_progress(&store(&["wJ:p1"])));

    assert!(progress_of(&domain, &p1()).is_some());
    assert!(sync_in(&domain, &p1()).is_some());
    assert_eq!(progress_of(&domain, &p2()), None);
    assert_eq!(sync_in(&domain, &p2()), None);
}

/// 沒有消失的 pane：回報沒有改變。
#[test]
fn no_vanished_pane_reports_no_change() {
    let mut domain = domain_default();
    set_progress(&mut domain, &p1(), "實作", Mark::None);
    set_sync(&mut domain, &p1(), sync_of(SyncMode::Auto, None));
    let before = domain.clone();

    assert!(!domain.clear_vanished_repo_progress(&store(&["wJ:p1"])));

    assert_eq!(domain, before);
}

// ---------------------------------------------------------------------------
// 型別與預設值
// ---------------------------------------------------------------------------

/// `openspec_obs` 不持久、預設為空；`apply_openspec` 不寫它（投影用的最新結果由 task 4.3 的 `sync_openspec` 寫）。
#[test]
fn openspec_obs_defaults_empty_and_apply_does_not_write_it() {
    let mut domain = domain_default();
    assert!(domain.openspec_obs.is_empty());

    domain.apply_openspec(
        &app(),
        &p1(),
        Some(&obs("foo", OpenSpecPhase::Implement, 3, 8)),
    );

    assert!(domain.openspec_obs.is_empty());
}

/// `SyncMode` 序列化為小寫字串（狀態檔 `sync.mode` 的值）。
#[test]
fn sync_mode_serializes_lowercase() {
    assert_eq!(serde_json::to_string(&SyncMode::Auto).unwrap(), "\"auto\"");
    assert_eq!(
        serde_json::to_string(&SyncMode::Manual).unwrap(),
        "\"manual\""
    );
    assert_eq!(
        serde_json::to_value(obs("foo", OpenSpecPhase::Implement, 3, 8)).unwrap(),
        serde_json::json!({"change": "foo", "phase": "implement", "checked": 3, "total": 8})
    );
}

/// design D10-2 的 debug 防線：舊 stages 與舊 phases 長度必須一致，否則呼叫端傳錯定義會靜默誤判。
#[cfg(debug_assertions)]
#[test]
#[should_panic(expected = "old_stages")]
fn phases_changed_rejects_misaligned_old_definition_in_debug_builds() {
    let remap = cockpit_core::apply_stage_edits(
        &four_stages(),
        &[cockpit_core::StageEdit {
            name: s("規劃"),
            from: Some(s("規劃")),
            phase: Some(OpenSpecPhase::Plan),
        }],
    )
    .expect("合法編輯");

    let _ = remap.phases_changed(&four_stages(), &[Some(OpenSpecPhase::Plan)]);
}

// ---------------------------------------------------------------------------
// 全表重套（openspec-stage-sync task 4.3；Task 3.2 Ruling「下一輪」語意）
// ---------------------------------------------------------------------------

/// `reapply_openspec_all` 以 `openspec_obs` 對每張展開的 Repo Project task 套用；沒有偵測結果的 task 不動；
/// 第二次呼叫什麼都不改（D3 第 3 條保證冪等）。
#[test]
fn reapply_all_applies_latest_observations_and_is_idempotent() {
    let mut domain = domain_default();
    let seen = obs("foo", OpenSpecPhase::Review, 5, 5);
    domain
        .openspec_obs
        .insert((runtime_id("local"), pane_id("wJ:p1")), seen.clone());

    assert!(domain.reapply_openspec_all());

    assert_eq!(
        progress_of(&domain, &p1()),
        Some(progress("審查", Mark::None))
    );
    assert_eq!(
        sync_in(&domain, &p1()),
        Some(sync_of(SyncMode::Auto, Some(seen)))
    );
    assert_eq!(
        progress_of(&domain, &p2()),
        None,
        "沒有偵測結果的 task 不動"
    );
    assert_eq!(sync_in(&domain, &p2()), None);

    let before = domain.clone();
    assert!(!domain.reapply_openspec_all(), "第二次沒有任何改變");
    assert_eq!(domain, before);
}

/// 標記中的 task、沒有展開的 pane（不在 `pane_repos`）都不被全表重套改變。
#[test]
fn reapply_all_skips_marked_and_unexpanded_tasks() {
    let mut domain = domain_default();
    set_progress(&mut domain, &p1(), "實作", Mark::Failed);
    domain.openspec_obs.insert(
        (runtime_id("local"), pane_id("wJ:p1")),
        obs("foo", OpenSpecPhase::Review, 5, 5),
    );
    domain.openspec_obs.insert(
        (runtime_id("local"), pane_id("wJ:p9")),
        obs("foo", OpenSpecPhase::Review, 5, 5),
    );
    let before = domain.clone();

    assert!(!domain.reapply_openspec_all());

    assert_eq!(domain, before);
}
