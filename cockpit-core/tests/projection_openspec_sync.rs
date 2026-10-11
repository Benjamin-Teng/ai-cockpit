//! openspec-stage-sync task 3.3（design D8；spec `state-projection`）：投影輸出 `stage_phases` 與 task 的
//! `sync`。測試名稱對應 spec scenario；一律比對序列化後的 JSON，確認欄位名稱與 `null` 的形狀。
//!
//! 「偵測結果改變時 version 遞增」的投影任務測試沿用 `projection_projects.rs` 的時間控制：`start_paused` ＋
//! `rx.changed()` 搭配 `tokio::time::timeout`，逾時本身就是「沒有廣播」的證據。

mod common;

use std::time::{Duration, SystemTime};

use cockpit_core::{
    DomainState, Mark, Observation, OpenSpecPhase, PaneRepo, PaneRepos, ProjectDef, ProjectId,
    ProjectedState, RepoKey, RepoProjectDef, RuntimeStore, StoreHandle, SyncMode, TaskDef, TaskId,
    TaskProgress, TaskSync, WorkstreamDef, WorkstreamId, project, spawn_projector,
};
use serde_json::{Value, json};

use common::{pane_id, runtime_id};

fn s(v: &str) -> String {
    v.to_string()
}

const APP: &str = r"d:\work\app\.git";
const EXPECT_BROADCAST_TIMEOUT: Duration = Duration::from_secs(5);

fn app() -> ProjectId {
    ProjectId::new("app")
}

fn p1() -> TaskId {
    TaskId::new("local~wJ:p1")
}

fn obs(change: &str, phase: OpenSpecPhase, checked: u32, total: u32) -> Observation {
    Observation {
        change: s(change),
        phase,
        checked,
        total,
    }
}

fn repo_project(stages: &[&str], phases: Vec<Option<OpenSpecPhase>>) -> RepoProjectDef {
    RepoProjectDef {
        id: app(),
        name: s("app"),
        repo: RepoKey::new(APP),
        stages: stages.iter().map(|x| s(x)).collect(),
        phases,
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
            depends_on: vec![],
        }],
        repo: None,
    }
}

/// Repo Project `app`（指定 stages 與 phases），`local` 的 `wJ:p1` 歸入它；沒有任何進度、同步狀態與偵測結果。
fn domain_with(
    config: Vec<ProjectDef>,
    stages: &[&str],
    phases: Vec<Option<OpenSpecPhase>>,
) -> DomainState {
    let mut domain = DomainState::from_projects(config);
    domain.repo_projects = vec![repo_project(stages, phases)];
    domain.pane_repos = PaneRepos::from([(
        (runtime_id("local"), pane_id("wJ:p1")),
        PaneRepo {
            repo: RepoKey::new(APP),
            default_name: s("app"),
            worktree: None,
            root: None,
        },
    )]);
    domain.refresh_projects();
    domain
}

fn four_stage_domain() -> DomainState {
    domain_with(
        vec![],
        &["規劃", "實作", "審查", "完成"],
        OpenSpecPhase::ALL.into_iter().map(Some).collect(),
    )
}

fn observe(domain: &mut DomainState, value: Observation) {
    domain
        .openspec_obs
        .insert((runtime_id("local"), pane_id("wJ:p1")), value);
}

fn save_sync(domain: &mut DomainState, mode: SyncMode, applied: Option<Observation>) {
    domain
        .repo_sync
        .entry(app())
        .or_default()
        .insert(p1(), TaskSync { mode, applied });
}

fn projected_json(domain: &DomainState) -> Value {
    let projected = project(&RuntimeStore::new(), domain, 1, SystemTime::UNIX_EPOCH);
    serde_json::to_value(&projected).expect("可序列化")
}

fn find<'a>(items: &'a Value, id: &str) -> &'a Value {
    items
        .as_array()
        .expect("應為陣列")
        .iter()
        .find(|item| item["id"] == id)
        .unwrap_or_else(|| panic!("找不到 id={id}"))
}

/// 該 task 在 `app` 底下投影出的 `sync`。
fn task_sync(domain: &DomainState) -> Value {
    let value = projected_json(domain);
    let task = find(&find(&value["projects"], "app")["tasks"], "local~wJ:p1");
    task.get("sync").cloned().expect("task 應帶 sync 欄位")
}

// ---------------------------------------------------------------------------
// stage_phases
// ---------------------------------------------------------------------------

/// spec「Repo Project 的階段對應」：Repo Project 的 `stage_phases` 逐項對齊 `stages`；手寫 project 為 `[]`。
#[test]
fn repo_project_stage_phases_follow_the_mapping_and_config_project_is_empty() {
    let domain = domain_with(
        vec![config_project("h")],
        &["規劃", "實作", "審查", "完成"],
        OpenSpecPhase::ALL.into_iter().map(Some).collect(),
    );
    let value = projected_json(&domain);
    assert_eq!(
        find(&value["projects"], "app")["stage_phases"],
        json!(["plan", "implement", "review", "complete"])
    );
    assert_eq!(find(&value["projects"], "h")["stage_phases"], json!([]));
}

/// spec「部分 stage 沒有對應」：沒有對應的 stage 為 `null`，長度與 `stages` 相同。
#[test]
fn stage_phases_has_null_for_unmapped_stages() {
    let domain = domain_with(
        vec![],
        &["Plan", "Build", "Done"],
        vec![Some(OpenSpecPhase::Plan), None, None],
    );
    let value = projected_json(&domain);
    let app = find(&value["projects"], "app");
    assert_eq!(app["stage_phases"], json!(["plan", null, null]));
    assert_eq!(
        app["stage_phases"].as_array().unwrap().len(),
        app["stages"].as_array().unwrap().len()
    );
}

// ---------------------------------------------------------------------------
// task.sync
// ---------------------------------------------------------------------------

/// spec「task 的同步資訊」：偵測結果取自 `openspec_obs`，`mode` 取自保存的同步狀態（`manual`）。
#[test]
fn task_sync_combines_observation_with_saved_mode() {
    let mut domain = four_stage_domain();
    observe(&mut domain, obs("foo", OpenSpecPhase::Implement, 3, 8));
    save_sync(
        &mut domain,
        SyncMode::Manual,
        Some(obs("foo", OpenSpecPhase::Implement, 3, 8)),
    );
    assert_eq!(
        task_sync(&domain),
        json!({"change": "foo", "phase": "implement", "checked": 3, "total": 8, "mode": "manual"})
    );
}

/// `change`／`phase`／`checked`／`total` 取自當下偵測結果、不是 `applied`：兩者不同時以偵測結果為準。
#[test]
fn task_sync_numbers_come_from_the_current_observation_not_applied() {
    let mut domain = four_stage_domain();
    observe(&mut domain, obs("bar", OpenSpecPhase::Review, 7, 8));
    save_sync(
        &mut domain,
        SyncMode::Auto,
        Some(obs("foo", OpenSpecPhase::Implement, 3, 8)),
    );
    assert_eq!(
        task_sync(&domain),
        json!({"change": "bar", "phase": "review", "checked": 7, "total": 8, "mode": "auto"})
    );
}

/// spec「對不上 change 時 sync 為 null」：保存的同步狀態曾套用過 `foo`，但目前沒有偵測結果 → `null`。
#[test]
fn task_sync_is_null_when_there_is_no_current_observation_even_with_saved_state() {
    let mut domain = four_stage_domain();
    save_sync(
        &mut domain,
        SyncMode::Auto,
        Some(obs("foo", OpenSpecPhase::Implement, 3, 8)),
    );
    assert_eq!(task_sync(&domain), Value::Null);
}

/// spec「有偵測結果但沒有保存的同步狀態」：task 標記為 `completed`、沒有 `TaskSync` → `mode` 為 `auto`。
#[test]
fn task_sync_defaults_to_auto_when_no_saved_state() {
    let mut domain = four_stage_domain();
    domain.repo_progress.entry(app()).or_default().insert(
        p1(),
        TaskProgress {
            stage: s("審查"),
            mark: Mark::Completed,
        },
    );
    observe(&mut domain, obs("foo", OpenSpecPhase::Review, 5, 5));
    assert_eq!(
        task_sync(&domain),
        json!({"change": "foo", "phase": "review", "checked": 5, "total": 5, "mode": "auto"})
    );
}

/// 只對上別的 pane 的偵測結果不算這張 task 的：key 為 runtime＋pane。
#[test]
fn task_sync_ignores_observations_of_other_panes() {
    let mut domain = four_stage_domain();
    domain.openspec_obs.insert(
        (runtime_id("local"), pane_id("wJ:p2")),
        obs("foo", OpenSpecPhase::Plan, 0, 4),
    );
    domain.openspec_obs.insert(
        (runtime_id("wsl"), pane_id("wJ:p1")),
        obs("foo", OpenSpecPhase::Plan, 0, 4),
    );
    assert_eq!(task_sync(&domain), Value::Null);
}

/// spec「手寫 project 的 task 沒有同步資訊」：即使 `openspec_obs` 恰好有同 key 的資料，手寫 project 的 `sync` 仍為 `null`。
#[test]
fn config_project_task_sync_is_always_null() {
    let mut domain = four_stage_domain();
    let mut config = config_project("h");
    config.tasks[0].id = p1();
    domain.projects.insert(0, config);
    observe(&mut domain, obs("foo", OpenSpecPhase::Plan, 0, 4));
    let value = projected_json(&domain);
    let task = find(&find(&value["projects"], "h")["tasks"], "local~wJ:p1");
    assert_eq!(task.get("sync"), Some(&Value::Null), "應明確輸出 null");
}

/// 舊 JSON（沒有 `stage_phases`／`sync`）仍可反序列化，預設為空陣列與 `null`。
#[test]
fn old_json_without_stage_phases_and_sync_still_deserializes() {
    let old = json!({
        "version": 1,
        "generated_at": "1970-01-01T00:00:00Z",
        "runtimes": [],
        "projects": [{
            "id": "p", "name": "p", "stages": ["A"], "warnings": [],
            "workstreams": [],
            "tasks": [{
                "id": "t", "title": "t", "workstream": "w", "stage": "A",
                "mark": "none", "status": "ready", "depends_on": []
            }]
        }],
        "recent_events": []
    });
    let state: ProjectedState = serde_json::from_value(old).expect("舊形狀應可讀");
    let back = serde_json::to_value(&state).unwrap();
    assert_eq!(back["projects"][0]["stage_phases"], json!([]));
    assert_eq!(back["projects"][0]["tasks"][0]["sync"], Value::Null);
}

// ---------------------------------------------------------------------------
// version
// ---------------------------------------------------------------------------

/// 偵測結果（或階段對應）不同，`content_eq` 必須為 false，投影任務才會遞增 version；只差 version／時間則相等。
#[test]
fn sync_and_stage_phases_participate_in_content_equality() {
    let at = |domain: &DomainState, version: u64, secs: u64| {
        project(
            &RuntimeStore::new(),
            domain,
            version,
            SystemTime::UNIX_EPOCH + Duration::from_secs(secs),
        )
    };
    let none = four_stage_domain();
    let mut seen = four_stage_domain();
    observe(&mut seen, obs("foo", OpenSpecPhase::Plan, 0, 4));
    let mut seen_more = four_stage_domain();
    observe(&mut seen_more, obs("foo", OpenSpecPhase::Plan, 1, 4));

    assert!(at(&seen, 1, 0).content_eq(&at(&seen, 2, 99)));
    assert!(
        !at(&none, 1, 0).content_eq(&at(&seen, 1, 0)),
        "有無偵測結果"
    );
    assert!(
        !at(&seen, 1, 0).content_eq(&at(&seen_more, 1, 0)),
        "勾選數不同"
    );

    // 只有階段對應不同（stages 相同）。
    let remapped = domain_with(
        vec![],
        &["規劃", "實作", "審查", "完成"],
        vec![None, None, None, None],
    );
    assert!(
        !at(&none, 1, 0).content_eq(&at(&remapped, 1, 0)),
        "階段對應不同"
    );
}

/// spec「偵測結果改變遞增 version」：version 為 1、`sync` 為 `null`，其 pane 的偵測結果變成對上 `foo` → 收到 version 2，
/// `sync` 不再是 `null`；同樣的偵測結果再寫一次則不遞增。
#[tokio::test(start_paused = true)]
async fn observation_change_increments_version_and_same_observation_does_not() {
    let domain = four_stage_domain();
    let handle = StoreHandle::new_with_domain(RuntimeStore::new(), domain.clone());
    let mut rx = handle.subscribe();
    assert_eq!(handle.current().version, 1);
    let sync_of = |state: &ProjectedState| {
        serde_json::to_value(state).unwrap()["projects"][0]["tasks"][0]["sync"].clone()
    };
    assert_eq!(sync_of(&handle.current()), Value::Null);
    let projector = spawn_projector(handle.clone());

    let mut detected = domain.clone();
    observe(&mut detected, obs("foo", OpenSpecPhase::Plan, 0, 4));
    handle.set_domain(detected.clone());
    tokio::time::timeout(EXPECT_BROADCAST_TIMEOUT, rx.changed())
        .await
        .expect("應在合理時間內收到廣播")
        .expect("channel 應仍存活");
    let current = rx.borrow().clone();
    assert_eq!(current.version, 2);
    assert_eq!(
        sync_of(&current),
        json!({"change": "foo", "phase": "plan", "checked": 0, "total": 4, "mode": "auto"})
    );

    handle.set_domain(detected.clone());
    let result = tokio::time::timeout(Duration::from_millis(500), rx.changed()).await;
    assert!(result.is_err(), "偵測結果沒變不該廣播");
    assert_eq!(handle.current().version, 2);

    // 勾選數改變 → 再遞增。
    let mut progressed = detected;
    observe(&mut progressed, obs("foo", OpenSpecPhase::Plan, 1, 4));
    handle.set_domain(progressed);
    tokio::time::timeout(EXPECT_BROADCAST_TIMEOUT, rx.changed())
        .await
        .expect("應在合理時間內收到廣播")
        .expect("channel 應仍存活");
    assert_eq!(rx.borrow().version, 3);

    projector.abort();
}
