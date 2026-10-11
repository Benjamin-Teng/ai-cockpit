//! openspec-stage-sync task 4.3 驗收測試：寫入服務接上 OpenSpec 同步（design D2、D3、D8、D10；spec
//! `openspec-stage-sync`「自動移動規則」「已標記的卡片不自動移動」「手動操作暫時優先」「階段對應被修改時重新套用」
//! 「卡片同步狀態的持久化」、`repo-projects`「加入 Repo Project」「修改 Repo Project 名稱與 stages」「移除 Repo Project」、
//! `pipeline-progress`「狀態檔格式與持久化」）。
//!
//! 全部打真的 `ProgressService`，狀態檔放在每個測試自己的暫存目錄。偵測本身（task 4.4）不在這裡：測試直接呼叫
//! `sync_openspec` 送整張對照表。

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use cockpit::progress;
use cockpit::progress_service::{
    BindingBasis, NewRepoProject, OpenSpecEntry, ProgressService, RepoProjectError,
    RepoProjectPatch, StageEditInput, WriteError, WriteHook, WriteStage,
};
use cockpit_core::{
    DomainState, Mark, Observation, OpenSpecPhase, PaneId, PaneRepo, PaneRepos, ProgressOp,
    ProjectDef, ProjectId, ProjectedTaskSync, RepoKey, RepoProjectDef, RuntimeId, RuntimeStore,
    StoreHandle, SyncMode, TaskDef, TaskId, TaskProgress, TaskSync, WorkstreamDef, WorkstreamId,
    project,
};
use serde_json::{Value, json};

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(tag: &str) -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock 應晚於 UNIX_EPOCH")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "cockpit-openspec-sync-service-{tag}-{}-{nanos}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("建立測試暫存目錄");
        Self { path }
    }

    fn state_path(&self) -> PathBuf {
        self.path.join("cockpit.state.json")
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

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

fn key(pane: &str) -> (RuntimeId, PaneId) {
    (RuntimeId::new("local"), PaneId::new(pane))
}

fn obs(change: &str, phase: OpenSpecPhase, checked: u32, total: u32) -> Observation {
    Observation {
        change: s(change),
        phase,
        checked,
        total,
    }
}

fn implement(checked: u32) -> Observation {
    obs("foo", OpenSpecPhase::Implement, checked, 8)
}

fn four_stages() -> Vec<String> {
    vec![s("規劃"), s("實作"), s("審查"), s("完成")]
}

fn four_phases() -> Vec<Option<OpenSpecPhase>> {
    OpenSpecPhase::ALL.into_iter().map(Some).collect()
}

/// 手寫 project `h`（stages `Spec`→`Build`、workstream `be`、task `t1`）。
fn config_project() -> ProjectDef {
    ProjectDef {
        id: ProjectId::new("h"),
        name: s("h"),
        stages: vec![s("Spec"), s("Build")],
        workstreams: vec![WorkstreamDef {
            id: WorkstreamId::new("be"),
            name: s("be"),
            binding: None,
            pinned_pane: None,
        }],
        tasks: vec![TaskDef {
            id: TaskId::new("t1"),
            title: s("t1"),
            workstream: WorkstreamId::new("be"),
            stage: s("Spec"),
            depends_on: Vec::new(),
        }],
        repo: None,
    }
}

fn app_pane_repos(panes: &[&str]) -> PaneRepos {
    panes
        .iter()
        .map(|pane| {
            (
                key(pane),
                PaneRepo {
                    repo: RepoKey::new(APP_REPO),
                    default_name: s("app"),
                    worktree: None,
                    root: None,
                },
            )
        })
        .collect()
}

/// 手寫 project `h`＋Repo Project `app`（指定 stages 與 phases），`wJ:p1`、`wJ:p2` 歸入 `app`；沒有任何進度與同步狀態。
fn domain_with(stages: Vec<String>, phases: Vec<Option<OpenSpecPhase>>) -> DomainState {
    let mut domain = DomainState::from_projects(vec![config_project()]);
    domain.repo_projects.push(RepoProjectDef {
        id: app(),
        name: s("app"),
        repo: RepoKey::new(APP_REPO),
        stages,
        phases,
    });
    domain.pane_repos = app_pane_repos(&["wJ:p1", "wJ:p2"]);
    domain.refresh_projects();
    domain
}

fn domain_default() -> DomainState {
    domain_with(four_stages(), four_phases())
}

fn store_connecting() -> RuntimeStore {
    let mut store = RuntimeStore::new();
    store.register(RuntimeId::new("local"), s("herdr"), s("test"));
    store
}

/// 計算落檔次數的服務（hook 只數 `Start`）。
fn counting_service(handle: &StoreHandle, path: &Path) -> (ProgressService, Arc<AtomicUsize>) {
    let writes = Arc::new(AtomicUsize::new(0));
    let hook: WriteHook = {
        let writes = writes.clone();
        Arc::new(move |stage| {
            if stage == WriteStage::Start {
                writes.fetch_add(1, Ordering::SeqCst);
            }
        })
    };
    (
        ProgressService::with_write_hook(handle.clone(), path.to_path_buf(), hook),
        writes,
    )
}

fn writes(counter: &AtomicUsize) -> usize {
    counter.load(Ordering::SeqCst)
}

fn read_json(path: &Path) -> Value {
    serde_json::from_str(&fs::read_to_string(path).expect("讀狀態檔")).expect("狀態檔是 JSON")
}

fn domain_of(handle: &StoreHandle) -> DomainState {
    handle.with_domain(Clone::clone)
}

fn stage_of(handle: &StoreHandle, task: &TaskId) -> Option<String> {
    domain_of(handle)
        .repo_progress
        .get(&app())
        .and_then(|t| t.get(task))
        .map(|p| p.stage.clone())
}

fn sync_of(handle: &StoreHandle, task: &TaskId) -> Option<TaskSync> {
    domain_of(handle)
        .repo_sync
        .get(&app())
        .and_then(|t| t.get(task))
        .cloned()
}

fn task_sync(mode: SyncMode, applied: Option<Observation>) -> TaskSync {
    TaskSync { mode, applied }
}

/// 投影中 `app` 的 task 的 `sync`。
fn projected_sync(handle: &StoreHandle, task: &TaskId) -> Option<ProjectedTaskSync> {
    let domain = domain_of(handle);
    let state = handle.with_store(|store| project(store, &domain, 1, SystemTime::now()));
    state
        .projects
        .iter()
        .find(|p| p.id == app())
        .and_then(|p| p.tasks.iter().find(|t| &t.id == task))
        .expect("投影中有該 task")
        .sync
        .clone()
}

/// R1（Task 4.1 carry）：有同步狀態的 task 一定有進度項目。
fn assert_sync_backed_by_progress(handle: &StoreHandle) {
    let domain = domain_of(handle);
    for (project, table) in &domain.repo_sync {
        for task in table.keys() {
            assert!(
                domain
                    .repo_progress
                    .get(project)
                    .is_some_and(|p| p.contains_key(task)),
                "{project}/{task} 有 sync 卻沒有進度項目"
            );
        }
    }
}

/// 一筆偵測結果，附上偵測當下的歸類。
fn entry(pane: &str, location: PaneRepo, observation: Option<Observation>) -> OpenSpecEntry {
    OpenSpecEntry {
        pane: key(pane),
        location,
        observation,
    }
}

/// 以 `app` 主 worktree 的歸類（同 [`app_pane_repos`]）送整張表。
async fn sync(service: &ProgressService, entries: &[(&str, Option<Observation>)]) {
    service
        .sync_openspec(
            entries
                .iter()
                .map(|(pane, obs)| entry(pane, classified(APP_REPO, None), obs.clone()))
                .collect(),
        )
        .await
        .expect("同步應成功");
}

// ---------------------------------------------------------------------------
// 自動移動與持久化
// ---------------------------------------------------------------------------

/// spec「新卡片第一次偵測就移到對應站」「寫出 task 的同步狀態」：自動移動落檔；沒有結果的 pane 沒有 `sync`。
#[tokio::test]
async fn auto_move_is_persisted_with_sync_state() {
    let dir = TempDir::new("auto");
    let handle = StoreHandle::new_with_domain(store_connecting(), domain_default());
    let (service, counter) = counting_service(&handle, &dir.state_path());

    sync(&service, &[("wJ:p1", Some(implement(3))), ("wJ:p2", None)]).await;

    assert_eq!(writes(&counter), 1);
    assert_eq!(stage_of(&handle, &p1()).as_deref(), Some("實作"));
    assert_eq!(
        projected_sync(&handle, &p1()),
        Some(ProjectedTaskSync {
            change: s("foo"),
            phase: OpenSpecPhase::Implement,
            checked: 3,
            total: 8,
            mode: SyncMode::Auto,
        })
    );
    let file = read_json(&dir.state_path());
    assert_eq!(
        file["repo_projects"]["app"]["tasks"]["local~wJ:p1"],
        json!({"stage": "實作", "mark": "none", "sync": {"mode": "auto", "applied":
            {"change": "foo", "phase": "implement", "checked": 3, "total": 8}}})
    );
    assert!(file["repo_projects"]["app"]["tasks"]["local~wJ:p2"].is_null());
    assert_eq!(projected_sync(&handle, &p2()), None);
    assert_sync_backed_by_progress(&handle);
}

/// spec「結果沒變不更新投影」「自動模式重啟後不重複移動」：同一張表送兩次，第二次 Domain 完全不變、不落檔。
#[tokio::test]
async fn same_observations_twice_change_nothing_the_second_time() {
    let dir = TempDir::new("same");
    let handle = StoreHandle::new_with_domain(store_connecting(), domain_default());
    let (service, counter) = counting_service(&handle, &dir.state_path());
    sync(&service, &[("wJ:p1", Some(implement(3)))]).await;
    let before = domain_of(&handle);
    let bytes = fs::read(dir.state_path()).expect("已寫檔");

    sync(&service, &[("wJ:p1", Some(implement(3)))]).await;

    assert_eq!(domain_of(&handle), before);
    assert_eq!(writes(&counter), 1);
    assert_eq!(fs::read(dir.state_path()).expect("讀檔"), bytes);
}

/// spec「偵測結果沒有造成改變時不改寫狀態檔」：標記中的卡片收到新結果，只更新記憶體中的最新結果（投影可見），
/// 不寫檔。
#[tokio::test]
async fn observation_without_effect_updates_memory_but_not_state_file() {
    let dir = TempDir::new("no-effect");
    let mut domain = domain_default();
    domain.repo_progress.entry(app()).or_default().insert(
        p1(),
        TaskProgress {
            stage: s("實作"),
            mark: Mark::Completed,
        },
    );
    let handle = StoreHandle::new_with_domain(store_connecting(), domain);
    let (service, counter) = counting_service(&handle, &dir.state_path());

    sync(&service, &[("wJ:p1", Some(implement(5)))]).await;

    assert_eq!(writes(&counter), 0);
    assert!(!dir.state_path().exists());
    assert_eq!(stage_of(&handle, &p1()).as_deref(), Some("實作"));
    assert_eq!(sync_of(&handle, &p1()), None);
    assert_eq!(
        domain_of(&handle).openspec_obs.get(&key("wJ:p1")),
        Some(&implement(5))
    );
    assert_eq!(projected_sync(&handle, &p1()).map(|s| s.checked), Some(5));
}

/// spec「偵測的最新結果不寫入狀態檔」：之後因其他原因寫檔，標記中的 task 沒有 `sync`、檔案中沒有 change 名稱。
#[tokio::test]
async fn latest_observation_is_not_written_to_state_file() {
    let dir = TempDir::new("obs-not-written");
    let mut domain = domain_default();
    domain.repo_progress.entry(app()).or_default().insert(
        p1(),
        TaskProgress {
            stage: s("實作"),
            mark: Mark::Completed,
        },
    );
    let handle = StoreHandle::new_with_domain(store_connecting(), domain);
    let service = ProgressService::new(handle.clone(), dir.state_path());
    sync(
        &service,
        &[(
            "wJ:p1",
            Some(obs("unique-change-name", OpenSpecPhase::Review, 1, 2)),
        )],
    )
    .await;

    service
        .apply_progress(
            &ProjectId::new("h"),
            &TaskId::new("t1"),
            ProgressOp::Advance,
        )
        .await
        .expect("手寫 project 推進");

    let text = fs::read_to_string(dir.state_path()).expect("已寫檔");
    assert!(!text.contains("unique-change-name"), "{text}");
    assert_eq!(
        read_json(&dir.state_path())["repo_projects"]["app"]["tasks"]["local~wJ:p1"],
        json!({"stage": "實作", "mark": "completed"})
    );
}

/// Task 3.3 carry：送來的是整張對照表——值為 `None` 或不在表中的 pane 從 `openspec_obs` 移除（投影 `sync` 為 null）。
#[tokio::test]
async fn sync_replaces_whole_observation_table() {
    let handle = StoreHandle::new_with_domain(store_connecting(), domain_default());
    let service = ProgressService::in_memory(handle.clone());
    sync(
        &service,
        &[("wJ:p1", Some(implement(3))), ("wJ:p2", Some(implement(3)))],
    )
    .await;

    sync(&service, &[("wJ:p1", None)]).await;

    let domain = domain_of(&handle);
    assert!(domain.openspec_obs.is_empty(), "{:?}", domain.openspec_obs);
    assert_eq!(projected_sync(&handle, &p1()), None);
    assert_eq!(projected_sync(&handle, &p2()), None);
    // spec「無結果不改變任何東西」：位置與同步狀態都保留。
    assert_eq!(stage_of(&handle, &p1()).as_deref(), Some("實作"));
    assert_eq!(
        sync_of(&handle, &p1()),
        Some(task_sync(SyncMode::Auto, Some(implement(3))))
    );
}

/// 同步落檔失敗：回 `Persist`，記憶體（含最新偵測結果）不變，下一輪可重送。
#[tokio::test]
async fn sync_persist_failure_keeps_memory() {
    let dir = TempDir::new("sync-persist-fail");
    let handle = StoreHandle::new_with_domain(store_connecting(), domain_default());
    let service = ProgressService::new(handle.clone(), dir.path.join("missing").join("s.json"));
    let before = domain_of(&handle);

    let result = service
        .sync_openspec(vec![entry(
            "wJ:p1",
            classified(APP_REPO, None),
            Some(implement(3)),
        )])
        .await;

    assert!(
        matches!(result, Err(WriteError::Persist { .. })),
        "{result:?}"
    );
    assert_eq!(domain_of(&handle), before);
}

/// 原本沒有歸類的 pane 歸入 Repo Project：記憶體中它的偵測結果來源不明（不是對目前歸類的 worktree 查到的），丟棄、
/// 不套用；新卡片等下一輪偵測（task 4.3 review fix round 1 修正了原本「立即套用」的預期）。
#[tokio::test]
async fn newly_classified_pane_does_not_use_observation_of_unknown_origin() {
    let mut domain = domain_default();
    domain.pane_repos = app_pane_repos(&["wJ:p1"]);
    domain.refresh_projects();
    domain.openspec_obs.insert(key("wJ:p2"), implement(3));
    let handle = StoreHandle::new_with_domain(store_connecting(), domain);
    let service = ProgressService::in_memory(handle.clone());

    service
        .set_pane_repos(app_pane_repos(&["wJ:p1", "wJ:p2"]))
        .await
        .expect("更新歸類");

    assert_eq!(stage_of(&handle, &p2()), None);
    assert_eq!(sync_of(&handle, &p2()), None);
    assert!(domain_of(&handle).openspec_obs.is_empty());

    // 下一輪偵測送來 p2 的結果後才套用。
    sync(&service, &[("wJ:p2", Some(implement(3)))]).await;
    assert_eq!(stage_of(&handle, &p2()).as_deref(), Some("實作"));
}

/// 歸類沒變時重送同一份歸類結果：偵測結果保留，Domain 不變（全表重套冪等）。
#[tokio::test]
async fn unchanged_classification_keeps_observations_and_changes_nothing() {
    let handle = StoreHandle::new_with_domain(store_connecting(), domain_default());
    let service = ProgressService::in_memory(handle.clone());
    sync(&service, &[("wJ:p1", Some(implement(3)))]).await;
    let before = domain_of(&handle);

    service
        .set_pane_repos(app_pane_repos(&["wJ:p1", "wJ:p2"]))
        .await
        .expect("更新歸類");

    assert_eq!(domain_of(&handle), before);
}

// ---------------------------------------------------------------------------
// 手動入口
// ---------------------------------------------------------------------------

/// spec「人工推進後卡片留在手動位置」「推進只落檔一次」「重啟後手動優先仍有效」「重啟期間 OpenSpec 進度已變」。
#[tokio::test]
async fn manual_advance_holds_position_across_restart_until_observation_changes() {
    let dir = TempDir::new("manual");
    let handle = StoreHandle::new_with_domain(store_connecting(), domain_default());
    let (service, counter) = counting_service(&handle, &dir.state_path());
    sync(&service, &[("wJ:p1", Some(implement(3)))]).await;
    let before_advance = writes(&counter);

    service
        .apply_progress(&app(), &p1(), ProgressOp::Advance)
        .await
        .expect("推進應成功");

    assert_eq!(writes(&counter), before_advance + 1, "推進與標記只落檔一次");
    assert_eq!(
        read_json(&dir.state_path())["repo_projects"]["app"]["tasks"]["local~wJ:p1"],
        json!({"stage": "審查", "mark": "none", "sync": {"mode": "manual", "applied":
            {"change": "foo", "phase": "implement", "checked": 3, "total": 8}}})
    );
    sync(&service, &[("wJ:p1", Some(implement(3)))]).await;
    assert_eq!(stage_of(&handle, &p1()).as_deref(), Some("審查"));
    assert_eq!(
        projected_sync(&handle, &p1()).map(|s| s.mode),
        Some(SyncMode::Manual)
    );

    // 重啟：從狀態檔讀回，最新偵測結果不在檔裡。
    let runtimes: HashSet<&str> = HashSet::from(["local"]);
    let mut reloaded =
        progress::load_progress(&dir.state_path(), vec![config_project()], &runtimes)
            .expect("可讀回");
    assert!(reloaded.openspec_obs.is_empty());
    reloaded.pane_repos = app_pane_repos(&["wJ:p1", "wJ:p2"]);
    reloaded.refresh_projects();
    let handle = StoreHandle::new_with_domain(store_connecting(), reloaded);
    let service = ProgressService::new(handle.clone(), dir.state_path());

    sync(&service, &[("wJ:p1", Some(implement(3)))]).await;
    assert_eq!(stage_of(&handle, &p1()).as_deref(), Some("審查"));
    assert_eq!(
        sync_of(&handle, &p1()).map(|s| s.mode),
        Some(SyncMode::Manual)
    );

    sync(&service, &[("wJ:p1", Some(implement(4)))]).await;
    assert_eq!(stage_of(&handle, &p1()).as_deref(), Some("實作"));
    assert_eq!(
        sync_of(&handle, &p1()),
        Some(task_sync(SyncMode::Auto, Some(implement(4))))
    );
}

/// 寫一份 v4 狀態檔：`app` 的 `local~wJ:p1` 停在已不存在的站，帶指定的同步狀態；載入、補上歸類後交給服務。
fn reload_with_vanished_stage(dir: &TempDir, mode: &str) -> (StoreHandle, ProgressService) {
    fs::write(
        dir.state_path(),
        json!({"version": 4, "projects": {}, "repo_projects": {"app": {
            "name": "app", "repo": APP_REPO,
            "stages": ["規劃", "實作", "審查", "完成"],
            "phases": ["plan", "implement", "review", "complete"],
            "tasks": {"local~wJ:p1": {"stage": "已刪除的站", "mark": "none", "sync": {
                "mode": mode,
                "applied": {"change": "foo", "phase": "implement", "checked": 3, "total": 8}}}}
        }}})
        .to_string(),
    )
    .expect("寫入狀態檔");
    let runtimes: HashSet<&str> = HashSet::from(["local"]);
    let mut loaded = progress::load_progress(&dir.state_path(), vec![config_project()], &runtimes)
        .expect("可讀回");
    loaded.pane_repos = app_pane_repos(&["wJ:p1", "wJ:p2"]);
    loaded.refresh_projects();
    let handle = StoreHandle::new_with_domain(store_connecting(), loaded);
    let service = ProgressService::new(handle.clone(), dir.state_path());
    (handle, service)
}

/// task 7.2 Codex finding（design D4）：載入時 stage 不在 stages 內而退回第一站，auto 的 `applied` 一併清掉；
/// 之後偵測結果與清掉前相同，仍會把卡片移到對應站（否則 `applied` 與偵測結果相等而不再套用，卡片永久停在第一站）。
#[tokio::test]
async fn auto_card_reset_on_load_is_moved_again_by_unchanged_observation() {
    let dir = TempDir::new("reset-auto");
    let (handle, service) = reload_with_vanished_stage(&dir, "auto");

    assert_eq!(stage_of(&handle, &p1()).as_deref(), Some("規劃"));
    assert_eq!(
        sync_of(&handle, &p1()),
        Some(task_sync(SyncMode::Auto, None)),
        "退回第一站的 auto 卡片，applied 清為 None"
    );

    sync(&service, &[("wJ:p1", Some(implement(3)))]).await;

    assert_eq!(stage_of(&handle, &p1()).as_deref(), Some("實作"));
    assert_eq!(
        sync_of(&handle, &p1()),
        Some(task_sync(SyncMode::Auto, Some(implement(3))))
    );
}

/// 對照：manual 的卡片在同樣情況下保留 `applied`（手動優先，不被偵測結果拉走）。
#[tokio::test]
async fn manual_card_reset_on_load_keeps_applied() {
    let dir = TempDir::new("reset-manual");
    let (handle, service) = reload_with_vanished_stage(&dir, "manual");

    assert_eq!(stage_of(&handle, &p1()).as_deref(), Some("規劃"));
    assert_eq!(
        sync_of(&handle, &p1()),
        Some(task_sync(SyncMode::Manual, Some(implement(3))))
    );

    sync(&service, &[("wJ:p1", Some(implement(3)))]).await;

    assert_eq!(
        stage_of(&handle, &p1()).as_deref(),
        Some("規劃"),
        "偵測結果沒變，手動卡片不動"
    );
}

/// spec「人工退回同樣標記手動」：`applied` 不變。
#[tokio::test]
async fn manual_retreat_marks_manual_and_keeps_applied() {
    let handle = StoreHandle::new_with_domain(store_connecting(), domain_default());
    let service = ProgressService::in_memory(handle.clone());
    let review = obs("foo", OpenSpecPhase::Review, 5, 5);
    sync(&service, &[("wJ:p1", Some(review.clone()))]).await;
    assert_eq!(stage_of(&handle, &p1()).as_deref(), Some("審查"));

    service
        .apply_progress(&app(), &p1(), ProgressOp::Retreat)
        .await
        .expect("退回應成功");

    assert_eq!(stage_of(&handle, &p1()).as_deref(), Some("實作"));
    assert_eq!(
        sync_of(&handle, &p1()),
        Some(task_sync(SyncMode::Manual, Some(review)))
    );
}

/// spec「agent 的推進端點標記手動」：兩個 agent 推進入口各推一張自動的 task，都變手動，各只落檔一次。
#[tokio::test]
async fn agent_advance_endpoints_mark_manual() {
    let dir = TempDir::new("agent");
    let handle = StoreHandle::new_with_domain(store_connecting(), domain_default());
    let (service, counter) = counting_service(&handle, &dir.state_path());
    sync(
        &service,
        &[("wJ:p1", Some(implement(3))), ("wJ:p2", Some(implement(3)))],
    )
    .await;
    let base = writes(&counter);

    service
        .agent_advance(&app(), &p1(), &BindingBasis::Pinned)
        .await
        .expect("agent 推進");
    service
        .agent_advance_for_pane(vec![(
            app(),
            WorkstreamId::new("local~wJ:p2"),
            BindingBasis::Pinned,
        )])
        .await
        .expect("免帶 id 推進");

    assert_eq!(writes(&counter), base + 2);
    for task in [p1(), p2()] {
        assert_eq!(stage_of(&handle, &task).as_deref(), Some("審查"));
        assert_eq!(
            sync_of(&handle, &task),
            Some(task_sync(SyncMode::Manual, Some(implement(3)))),
            "{task}"
        );
    }
    let file = read_json(&dir.state_path());
    for task in ["local~wJ:p1", "local~wJ:p2"] {
        assert_eq!(
            file["repo_projects"]["app"]["tasks"][task]["sync"]["mode"],
            "manual"
        );
    }
}

/// spec「被拒絕的推進不改變模式」：最後一站推進回 409，`mode` 仍是自動、不落檔。
#[tokio::test]
async fn rejected_advance_keeps_auto_mode() {
    let dir = TempDir::new("rejected");
    let handle = StoreHandle::new_with_domain(store_connecting(), domain_default());
    let (service, counter) = counting_service(&handle, &dir.state_path());
    let done = obs("foo", OpenSpecPhase::Complete, 8, 8);
    sync(&service, &[("wJ:p1", Some(done.clone()))]).await;
    assert_eq!(stage_of(&handle, &p1()).as_deref(), Some("完成"));
    let before = domain_of(&handle);
    let base = writes(&counter);

    let manual = service
        .apply_progress(&app(), &p1(), ProgressOp::Advance)
        .await;
    let agent = service
        .agent_advance(&app(), &p1(), &BindingBasis::Pinned)
        .await;
    let agent_for_pane = service
        .agent_advance_for_pane(vec![(
            app(),
            WorkstreamId::new("local~wJ:p1"),
            BindingBasis::Pinned,
        )])
        .await;

    assert!(matches!(manual, Err(WriteError::Rejected(_))), "{manual:?}");
    assert!(matches!(agent, Err(WriteError::Rejected(_))), "{agent:?}");
    assert!(
        matches!(agent_for_pane, Err(WriteError::Rejected(_))),
        "{agent_for_pane:?}"
    );
    assert_eq!(domain_of(&handle), before);
    assert_eq!(writes(&counter), base);
    assert_eq!(
        sync_of(&handle, &p1()),
        Some(task_sync(SyncMode::Auto, Some(done)))
    );
}

/// spec「標記與清除不改變模式」：A 自動、B 手動；對 A 標 Completed 再清除、對 B 標 Failed 再清除，模式都不變。
#[tokio::test]
async fn complete_fail_and_clear_do_not_change_mode() {
    let handle = StoreHandle::new_with_domain(store_connecting(), domain_default());
    let service = ProgressService::in_memory(handle.clone());
    sync(
        &service,
        &[("wJ:p1", Some(implement(3))), ("wJ:p2", Some(implement(3)))],
    )
    .await;
    service
        .apply_progress(&app(), &p2(), ProgressOp::Advance)
        .await
        .expect("B 手動推進");

    for (task, op) in [(p1(), ProgressOp::Complete), (p2(), ProgressOp::Fail)] {
        service
            .apply_progress(&app(), &task, op)
            .await
            .expect("標記應成功");
        service
            .apply_progress(&app(), &task, ProgressOp::Clear)
            .await
            .expect("清除應成功");
    }

    assert_eq!(
        sync_of(&handle, &p1()).map(|s| s.mode),
        Some(SyncMode::Auto)
    );
    assert_eq!(
        sync_of(&handle, &p2()).map(|s| s.mode),
        Some(SyncMode::Manual)
    );
    assert_eq!(stage_of(&handle, &p2()).as_deref(), Some("審查"));
}

/// spec「還沒有同步狀態時手動推進」：當下已有偵測結果（第一輪套用之前），手動後 `applied` 為該結果，下一輪相同不拉走。
#[tokio::test]
async fn manual_advance_without_sync_captures_current_observation() {
    let mut domain = domain_default();
    domain.openspec_obs.insert(key("wJ:p1"), implement(3));
    let handle = StoreHandle::new_with_domain(store_connecting(), domain);
    let service = ProgressService::in_memory(handle.clone());

    service
        .apply_progress(&app(), &p1(), ProgressOp::Advance)
        .await
        .expect("推進應成功");
    assert_eq!(
        sync_of(&handle, &p1()),
        Some(task_sync(SyncMode::Manual, Some(implement(3))))
    );

    sync(&service, &[("wJ:p1", Some(implement(3)))]).await;
    assert_eq!(stage_of(&handle, &p1()).as_deref(), Some("實作"));
    assert_eq!(
        sync_of(&handle, &p1()).map(|s| s.mode),
        Some(SyncMode::Manual)
    );
}

/// spec「沒有偵測結果時手動推進」：`applied` 為無（狀態檔 `null`）；偵測結果首次出現時依自動移動規則套用。
#[tokio::test]
async fn manual_advance_without_observation_then_first_observation_applies() {
    let dir = TempDir::new("manual-no-obs");
    let handle = StoreHandle::new_with_domain(store_connecting(), domain_default());
    let service = ProgressService::new(handle.clone(), dir.state_path());

    service
        .apply_progress(&app(), &p1(), ProgressOp::Advance)
        .await
        .expect("推進應成功");
    assert_eq!(
        read_json(&dir.state_path())["repo_projects"]["app"]["tasks"]["local~wJ:p1"],
        json!({"stage": "實作", "mark": "none", "sync": {"mode": "manual", "applied": null}})
    );

    sync(
        &service,
        &[("wJ:p1", Some(obs("foo", OpenSpecPhase::Review, 5, 5)))],
    )
    .await;
    assert_eq!(stage_of(&handle, &p1()).as_deref(), Some("審查"));
    assert_eq!(
        sync_of(&handle, &p1()).map(|s| s.mode),
        Some(SyncMode::Auto)
    );
}

/// spec「手寫 project 的推進不產生同步狀態」。
#[tokio::test]
async fn hand_written_project_advance_has_no_sync() {
    let dir = TempDir::new("hand");
    let mut domain = domain_default();
    domain.openspec_obs.insert(key("wJ:p1"), implement(3));
    let handle = StoreHandle::new_with_domain(store_connecting(), domain);
    let service = ProgressService::new(handle.clone(), dir.state_path());

    service
        .apply_progress(
            &ProjectId::new("h"),
            &TaskId::new("t1"),
            ProgressOp::Advance,
        )
        .await
        .expect("推進應成功");

    assert!(domain_of(&handle).repo_sync.is_empty());
    assert_eq!(
        read_json(&dir.state_path())["projects"]["h"]["tasks"]["t1"],
        json!({"stage": "Build", "mark": "none"})
    );
}

/// Task 3.2 carry「偵測不變時清除標記卡片會移動」／spec「清除標記後恢復自動」：標記期間偵測結果已變，之後沒有新的
/// 偵測輪（背景工作去重不送），清除標記的同一次寫入就依最新結果移動。
#[tokio::test]
async fn clearing_mark_moves_card_without_new_observation_round() {
    let dir = TempDir::new("clear-moves");
    let handle = StoreHandle::new_with_domain(store_connecting(), domain_default());
    let (service, counter) = counting_service(&handle, &dir.state_path());
    sync(&service, &[("wJ:p1", Some(implement(3)))]).await;
    service
        .apply_progress(&app(), &p1(), ProgressOp::Complete)
        .await
        .expect("標 Completed");
    let review = obs("foo", OpenSpecPhase::Review, 8, 8);
    sync(&service, &[("wJ:p1", Some(review.clone()))]).await;
    assert_eq!(stage_of(&handle, &p1()).as_deref(), Some("實作"));
    assert_eq!(
        projected_sync(&handle, &p1()).map(|s| s.checked),
        Some(8),
        "spec「標記期間不移動」：投影顯示最新結果"
    );
    let base = writes(&counter);

    service
        .apply_progress(&app(), &p1(), ProgressOp::Clear)
        .await
        .expect("清除標記");

    assert_eq!(writes(&counter), base + 1, "清除與重套只落檔一次");
    assert_eq!(stage_of(&handle, &p1()).as_deref(), Some("審查"));
    assert_eq!(
        sync_of(&handle, &p1()),
        Some(task_sync(SyncMode::Auto, Some(review)))
    );
}

// ---------------------------------------------------------------------------
// 階段對應的修改
// ---------------------------------------------------------------------------

fn edit(name: &str, from: Option<&str>, phase: Option<OpenSpecPhase>) -> StageEditInput {
    StageEditInput {
        name: s(name),
        from: from.map(s),
        phase: phase.map(|phase| s(phase.as_str())),
    }
}

fn abc_domain() -> DomainState {
    domain_with(
        vec![s("A"), s("B"), s("C")],
        vec![
            Some(OpenSpecPhase::Plan),
            Some(OpenSpecPhase::Implement),
            Some(OpenSpecPhase::Review),
        ],
    )
}

fn stages_patch(edits: Vec<StageEditInput>) -> RepoProjectPatch {
    RepoProjectPatch {
        name: None,
        stages: Some(edits),
    }
}

/// Task 3.2 carry「偵測不變時 PATCH 改對應 Auto 卡片會移動」／spec「自動的卡片依新對應移動」「手動的卡片不動」；
/// Task 3.1 deferred：服務層以 `StageEditInput { phase: Some(..) }` 寫入 Domain 的定義。
#[tokio::test]
async fn patch_mapping_moves_auto_card_and_leaves_manual_card() {
    let dir = TempDir::new("patch-moves");
    let handle = StoreHandle::new_with_domain(store_connecting(), abc_domain());
    let (service, counter) = counting_service(&handle, &dir.state_path());
    sync(
        &service,
        &[("wJ:p1", Some(implement(3))), ("wJ:p2", Some(implement(3)))],
    )
    .await;
    service
        .apply_progress(&app(), &p2(), ProgressOp::Retreat)
        .await
        .expect("p2 手動退回到 A");
    let base = writes(&counter);

    service
        .update_repo_project(
            &app(),
            stages_patch(vec![
                edit("A", Some("A"), Some(OpenSpecPhase::Plan)),
                edit("B", Some("B"), Some(OpenSpecPhase::Review)),
                edit("C", Some("C"), Some(OpenSpecPhase::Implement)),
            ]),
        )
        .await
        .expect("PATCH 應成功");

    assert_eq!(writes(&counter), base + 1);
    let domain = domain_of(&handle);
    let def = domain
        .repo_projects
        .iter()
        .find(|d| d.id == app())
        .expect("有 app");
    assert_eq!(
        def.phases,
        vec![
            Some(OpenSpecPhase::Plan),
            Some(OpenSpecPhase::Review),
            Some(OpenSpecPhase::Implement),
        ]
    );
    assert_eq!(stage_of(&handle, &p1()).as_deref(), Some("C"));
    assert_eq!(
        sync_of(&handle, &p1()),
        Some(task_sync(SyncMode::Auto, Some(implement(3))))
    );
    assert_eq!(stage_of(&handle, &p2()).as_deref(), Some("A"));
    assert_eq!(
        sync_of(&handle, &p2()),
        Some(task_sync(SyncMode::Manual, Some(implement(3))))
    );
    assert_eq!(
        read_json(&dir.state_path())["repo_projects"]["app"]["phases"],
        json!(["plan", "review", "implement"])
    );
}

/// spec「phase 換了擁有者算改變」與「改名且 phase 不變不清 applied」「只重排不清 applied」「只改名稱不重新套用」：
/// 當下沒有偵測結果時，只有對應真的改變才把自動 task 的 `applied` 清為無。
#[tokio::test]
async fn only_real_mapping_changes_clear_auto_applied() {
    let handle = StoreHandle::new_with_domain(store_connecting(), abc_domain());
    let service = ProgressService::in_memory(handle.clone());
    sync(&service, &[("wJ:p1", Some(implement(3)))]).await;
    // 偵測結果消失（例如切到對不上 change 的分支）：同步狀態保留，重套沒有東西可套。
    sync(&service, &[]).await;
    let kept = Some(task_sync(SyncMode::Auto, Some(implement(3))));

    service
        .update_repo_project(
            &app(),
            RepoProjectPatch {
                name: Some(s("x")),
                stages: None,
            },
        )
        .await
        .expect("只改名稱");
    assert_eq!(sync_of(&handle, &p1()), kept, "只改名稱");

    service
        .update_repo_project(
            &app(),
            stages_patch(vec![
                edit("A", Some("A"), Some(OpenSpecPhase::Plan)),
                edit("Make", Some("B"), Some(OpenSpecPhase::Implement)),
                edit("C", Some("C"), Some(OpenSpecPhase::Review)),
            ]),
        )
        .await
        .expect("改名、phase 跟著走");
    assert_eq!(sync_of(&handle, &p1()), kept, "改名且 phase 不變");

    service
        .update_repo_project(
            &app(),
            stages_patch(vec![
                edit("C", Some("C"), Some(OpenSpecPhase::Review)),
                edit("A", Some("A"), Some(OpenSpecPhase::Plan)),
                edit("Make", Some("Make"), Some(OpenSpecPhase::Implement)),
            ]),
        )
        .await
        .expect("只重排");
    assert_eq!(sync_of(&handle, &p1()), kept, "只重排");

    service
        .update_repo_project(
            &app(),
            stages_patch(vec![
                edit("C", Some("C"), Some(OpenSpecPhase::Implement)),
                edit("A", Some("A"), Some(OpenSpecPhase::Plan)),
                edit("Make", Some("Make"), Some(OpenSpecPhase::Review)),
            ]),
        )
        .await
        .expect("phase 換擁有者");
    assert_eq!(
        sync_of(&handle, &p1()),
        Some(task_sync(SyncMode::Auto, None)),
        "phase 換了擁有者"
    );
    assert_sync_backed_by_progress(&handle);
}

// ---------------------------------------------------------------------------
// 加入與移除
// ---------------------------------------------------------------------------

fn add_request(phases: Option<Vec<Option<OpenSpecPhase>>>) -> NewRepoProject {
    NewRepoProject {
        repo: RepoKey::new(APP_REPO),
        stages: four_stages(),
        name: None,
        phases: phases.map(phase_strings),
    }
}

/// 階段 → 服務邊界收的字串（`NewRepoProject::phases`）。
fn phase_strings(phases: Vec<Option<OpenSpecPhase>>) -> Vec<Option<String>> {
    phases
        .into_iter()
        .map(|phase| phase.map(|phase| s(phase.as_str())))
        .collect()
}

/// 只有 `app` repo 的 pane、沒有任何 Repo Project 定義。
fn detected_only() -> DomainState {
    let mut domain = DomainState::from_projects(vec![config_project()]);
    domain.pane_repos = app_pane_repos(&["wJ:p1", "wJ:p2"]);
    domain.refresh_projects();
    domain
}

/// spec「加入時帶階段對應」：`phases` 寫進定義與狀態檔。
#[tokio::test]
async fn add_with_phases_persists_them() {
    let dir = TempDir::new("add-phases");
    let handle = StoreHandle::new_with_domain(store_connecting(), detected_only());
    let service = ProgressService::new(handle.clone(), dir.state_path());

    let id = service
        .add_repo_project(add_request(Some(four_phases())))
        .await
        .expect("加入應成功");

    assert_eq!(id, app());
    assert_eq!(
        read_json(&dir.state_path())["repo_projects"]["app"]["phases"],
        json!(["plan", "implement", "review", "complete"])
    );
}

/// spec「phases 長度與 stages 不同」「階段對應重複」：回 `InvalidStages`，沒有新增 Project。
#[tokio::test]
async fn add_with_invalid_phases_is_rejected() {
    let handle = StoreHandle::new_with_domain(store_connecting(), detected_only());
    let service = ProgressService::in_memory(handle.clone());
    let before = domain_of(&handle);

    for phases in [
        vec![Some(OpenSpecPhase::Plan), None],
        vec![
            Some(OpenSpecPhase::Plan),
            Some(OpenSpecPhase::Plan),
            None,
            None,
        ],
    ] {
        let result = service.add_repo_project(add_request(Some(phases))).await;
        assert!(
            matches!(result, Err(RepoProjectError::InvalidStages)),
            "{result:?}"
        );
    }
    assert_eq!(domain_of(&handle), before);
}

/// Task 3.2 carry：加入時清掉同 id 殘留的進度，同步狀態也一併清掉（不然違反「有 sync 必有進度」）。
#[tokio::test]
async fn add_clears_residual_progress_and_sync_with_same_id() {
    let mut domain = detected_only();
    domain.repo_progress.entry(app()).or_default().insert(
        p1(),
        TaskProgress {
            stage: s("審查"),
            mark: Mark::None,
        },
    );
    domain
        .repo_sync
        .entry(app())
        .or_default()
        .insert(p1(), task_sync(SyncMode::Manual, None));
    let handle = StoreHandle::new_with_domain(store_connecting(), domain);
    let service = ProgressService::in_memory(handle.clone());

    service
        .add_repo_project(add_request(None))
        .await
        .expect("加入應成功");

    let domain = domain_of(&handle);
    assert!(!domain.repo_progress.contains_key(&app()));
    assert!(!domain.repo_sync.contains_key(&app()));
}

/// spec「移除後同步狀態一併消失」：移除後再加入，該 task 沒有舊的同步狀態，在第一個 stage。
#[tokio::test]
async fn remove_clears_sync_and_readd_starts_fresh() {
    let dir = TempDir::new("remove");
    let handle = StoreHandle::new_with_domain(store_connecting(), domain_default());
    let service = ProgressService::new(handle.clone(), dir.state_path());
    service
        .apply_progress(&app(), &p1(), ProgressOp::Advance)
        .await
        .expect("手動推進");
    assert!(sync_of(&handle, &p1()).is_some());

    service.remove_repo_project(&app()).await.expect("移除");
    assert!(domain_of(&handle).repo_sync.is_empty());

    service
        .add_repo_project(add_request(Some(four_phases())))
        .await
        .expect("再加入");
    assert_eq!(sync_of(&handle, &p1()), None);
    assert_eq!(stage_of(&handle, &p1()), None, "沒有紀錄即第一個 stage");
    assert!(read_json(&dir.state_path())["repo_projects"]["app"]["tasks"]["local~wJ:p1"].is_null());
}

// ---------------------------------------------------------------------------
// 並發
// ---------------------------------------------------------------------------

/// 自動、`applied` 3/8、在 `實作` 的 p1；當下偵測結果也是 3/8。
fn p1_auto_on_implement() -> DomainState {
    let mut domain = domain_default();
    domain.repo_progress.entry(app()).or_default().insert(
        p1(),
        TaskProgress {
            stage: s("實作"),
            mark: Mark::None,
        },
    );
    domain
        .repo_sync
        .entry(app())
        .or_default()
        .insert(p1(), task_sync(SyncMode::Auto, Some(implement(3))));
    domain.openspec_obs.insert(key("wJ:p1"), implement(3));
    domain
}

/// spec「推進與同步並發不遺失、不被拉回」：最終只會是「先同步後推進」（審查、手動、4/8）或「先推進後同步」
/// （實作、自動、4/8），不會出現「實作且手動」；狀態檔與記憶體一致。
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_advance_and_sync_end_in_one_of_two_orders() {
    for round in 0..30 {
        let dir = TempDir::new(&format!("race-{round}"));
        let handle = StoreHandle::new_with_domain(store_connecting(), p1_auto_on_implement());
        let service = ProgressService::new(handle.clone(), dir.state_path());

        let advance = {
            let service = service.clone();
            tokio::spawn(async move {
                service
                    .apply_progress(&app(), &p1(), ProgressOp::Advance)
                    .await
            })
        };
        let detect = {
            let service = service.clone();
            tokio::spawn(async move {
                service
                    .sync_openspec(vec![entry(
                        "wJ:p1",
                        classified(APP_REPO, None),
                        Some(implement(4)),
                    )])
                    .await
            })
        };
        advance.await.expect("不 panic").expect("推進被接受");
        detect.await.expect("不 panic").expect("同步被接受");

        let stage = stage_of(&handle, &p1());
        let sync = sync_of(&handle, &p1());
        let sync_then_advance = (
            Some(s("審查")),
            Some(task_sync(SyncMode::Manual, Some(implement(4)))),
        );
        let advance_then_sync = (
            Some(s("實作")),
            Some(task_sync(SyncMode::Auto, Some(implement(4)))),
        );
        let outcome = (stage, sync);
        assert!(
            outcome == sync_then_advance || outcome == advance_then_sync,
            "round {round}: {outcome:?}"
        );
        let file = read_json(&dir.state_path());
        let task = &file["repo_projects"]["app"]["tasks"]["local~wJ:p1"];
        assert_eq!(task["stage"], json!(outcome.0), "round {round}");
        assert_eq!(
            task["sync"]["mode"],
            json!(outcome.1.as_ref().map(|s| s.mode)),
            "round {round}"
        );
    }
}

/// spec「推進與同步並發不遺失、不被拉回」的「先推進後同步」順序（確定性）：推進後手動、`applied` 仍 3/8；
/// 新結果 4/8 與 `applied` 不同，移回 `實作` 並轉回自動。
#[tokio::test]
async fn advance_then_sync_moves_back_to_mapped_stage_as_auto() {
    let handle = StoreHandle::new_with_domain(store_connecting(), p1_auto_on_implement());
    let service = ProgressService::in_memory(handle.clone());

    service
        .apply_progress(&app(), &p1(), ProgressOp::Advance)
        .await
        .expect("推進被接受");
    assert_eq!(stage_of(&handle, &p1()).as_deref(), Some("審查"));
    assert_eq!(
        sync_of(&handle, &p1()),
        Some(task_sync(SyncMode::Manual, Some(implement(3))))
    );
    sync(&service, &[("wJ:p1", Some(implement(4)))]).await;

    assert_eq!(stage_of(&handle, &p1()).as_deref(), Some("實作"));
    assert_eq!(
        sync_of(&handle, &p1()),
        Some(task_sync(SyncMode::Auto, Some(implement(4))))
    );
}

/// spec「推進與同步並發不遺失、不被拉回」的「先同步後推進」順序（確定性）：同步後仍在 `實作`、自動、4/8；推進後在
/// `審查`、手動、`applied` 4/8。
#[tokio::test]
async fn sync_then_advance_keeps_manual_position() {
    let handle = StoreHandle::new_with_domain(store_connecting(), p1_auto_on_implement());
    let service = ProgressService::in_memory(handle.clone());

    sync(&service, &[("wJ:p1", Some(implement(4)))]).await;
    assert_eq!(stage_of(&handle, &p1()).as_deref(), Some("實作"));
    service
        .apply_progress(&app(), &p1(), ProgressOp::Advance)
        .await
        .expect("推進被接受");

    assert_eq!(stage_of(&handle, &p1()).as_deref(), Some("審查"));
    assert_eq!(
        sync_of(&handle, &p1()),
        Some(task_sync(SyncMode::Manual, Some(implement(4))))
    );
}

/// spec「判定以寫入鎖內的當下狀態為準」與「並發寫入不遺失」：同步與另一張卡的人工標記、手寫 project 推進同時送，
/// 全部生效並寫進狀態檔。
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_sync_and_manual_writes_are_not_lost() {
    for round in 0..20 {
        let dir = TempDir::new(&format!("concurrent-{round}"));
        let handle = StoreHandle::new_with_domain(store_connecting(), domain_default());
        let service = ProgressService::new(handle.clone(), dir.state_path());

        let detect = {
            let service = service.clone();
            tokio::spawn(async move {
                service
                    .sync_openspec(vec![entry(
                        "wJ:p1",
                        classified(APP_REPO, None),
                        Some(implement(3)),
                    )])
                    .await
            })
        };
        let fail = {
            let service = service.clone();
            tokio::spawn(async move {
                service
                    .apply_progress(&app(), &p2(), ProgressOp::Fail)
                    .await
            })
        };
        let hand = {
            let service = service.clone();
            tokio::spawn(async move {
                service
                    .apply_progress(
                        &ProjectId::new("h"),
                        &TaskId::new("t1"),
                        ProgressOp::Advance,
                    )
                    .await
            })
        };
        detect.await.expect("不 panic").expect("同步被接受");
        fail.await.expect("不 panic").expect("標 Failed 被接受");
        hand.await.expect("不 panic").expect("手寫推進被接受");

        let file = read_json(&dir.state_path());
        let tasks = &file["repo_projects"]["app"]["tasks"];
        assert_eq!(tasks["local~wJ:p1"]["stage"], "實作", "round {round}");
        assert_eq!(
            tasks["local~wJ:p1"]["sync"]["mode"], "auto",
            "round {round}"
        );
        assert_eq!(tasks["local~wJ:p2"]["mark"], "failed", "round {round}");
        assert_eq!(
            file["projects"]["h"]["tasks"]["t1"]["stage"], "Build",
            "round {round}"
        );
    }
}

// ---------------------------------------------------------------------------
// 歸類改變時丟棄舊歸類的偵測結果（task 4.3 review fix round 1）
// ---------------------------------------------------------------------------

const LIB_REPO: &str = r"d:\work\lib\.git";

fn lib() -> ProjectId {
    ProjectId::new("lib")
}

fn classified(repo: &str, worktree: Option<&str>) -> PaneRepo {
    PaneRepo {
        repo: RepoKey::new(repo),
        default_name: s(if repo == APP_REPO { "app" } else { "lib" }),
        worktree: worktree.map(s),
        root: None,
    }
}

/// `app`（`wJ:p1`、`wJ:p2`）＋`lib` 兩個 Repo Project，對應都是預設四站。
fn two_projects() -> DomainState {
    let mut domain = domain_default();
    domain.repo_projects.push(RepoProjectDef {
        id: lib(),
        name: s("lib"),
        repo: RepoKey::new(LIB_REPO),
        stages: four_stages(),
        phases: four_phases(),
    });
    domain.refresh_projects();
    domain
}

/// pane 從 Repo Project `app` 改歸類到 `lib`：`app` 那邊的偵測結果不得套到 `lib` 的卡片上——`lib` 的 task 沒有
/// 同步狀態、仍在第一個 stage，`openspec_obs` 也沒有該 pane；狀態檔不寫出 `lib` 的 task。
#[tokio::test]
async fn reclassified_pane_drops_observation_of_previous_repo() {
    let dir = TempDir::new("reclassify");
    let handle = StoreHandle::new_with_domain(store_connecting(), two_projects());
    let service = ProgressService::new(handle.clone(), dir.state_path());
    sync(&service, &[("wJ:p1", Some(implement(3)))]).await;
    assert_eq!(stage_of(&handle, &p1()).as_deref(), Some("實作"));

    let mut moved = app_pane_repos(&["wJ:p2"]);
    moved.insert(key("wJ:p1"), classified(LIB_REPO, None));
    service.set_pane_repos(moved).await.expect("更新歸類");

    let domain = domain_of(&handle);
    assert!(!domain.openspec_obs.contains_key(&key("wJ:p1")));
    assert!(
        !domain.repo_sync.contains_key(&lib()),
        "{:?}",
        domain.repo_sync
    );
    assert!(!domain.repo_progress.contains_key(&lib()));
    assert!(
        read_json(&dir.state_path())["repo_projects"]["lib"]["tasks"]
            .as_object()
            .is_some_and(|tasks| tasks.is_empty())
    );
    // 手動入口在這段時間窗內也不會把舊結果記成 applied。
    service
        .apply_progress(&lib(), &p1(), ProgressOp::Advance)
        .await
        .expect("推進");
    assert_eq!(
        domain_of(&handle).repo_sync[&lib()][&p1()],
        task_sync(SyncMode::Manual, None)
    );
}

/// 同一個 repo 但換了 worktree：偵測結果屬於舊 worktree，一併丟棄（投影 `sync` 為 null）；歸類沒變的 pane 保留。
#[tokio::test]
async fn worktree_change_drops_observation_but_unchanged_pane_keeps_it() {
    let handle = StoreHandle::new_with_domain(store_connecting(), domain_default());
    let service = ProgressService::in_memory(handle.clone());
    sync(
        &service,
        &[("wJ:p1", Some(implement(3))), ("wJ:p2", Some(implement(3)))],
    )
    .await;

    let mut moved = app_pane_repos(&["wJ:p2"]);
    moved.insert(key("wJ:p1"), classified(APP_REPO, Some("wt")));
    service.set_pane_repos(moved).await.expect("更新歸類");

    let domain = domain_of(&handle);
    assert!(!domain.openspec_obs.contains_key(&key("wJ:p1")));
    assert_eq!(domain.openspec_obs.get(&key("wJ:p2")), Some(&implement(3)));
    assert_eq!(projected_sync(&handle, &p1()), None);
}

/// pane 離開所有 repo（不在新的歸類結果中）：偵測結果丟棄。
#[tokio::test]
async fn unclassified_pane_drops_observation() {
    let handle = StoreHandle::new_with_domain(store_connecting(), domain_default());
    let service = ProgressService::in_memory(handle.clone());
    sync(&service, &[("wJ:p1", Some(implement(3)))]).await;

    service
        .set_pane_repos(app_pane_repos(&["wJ:p2"]))
        .await
        .expect("更新歸類");

    assert!(domain_of(&handle).openspec_obs.is_empty());
}

/// `add_repo_project` 的重套路徑：pane 先從 `app` 改歸類到尚未加入的 `lib` repo，再加入 `lib`——加入時不得把 `app`
/// 那邊的偵測結果套到新卡片上。
#[tokio::test]
async fn adding_repo_project_after_reclassification_does_not_apply_stale_observation() {
    let handle = StoreHandle::new_with_domain(store_connecting(), domain_default());
    let service = ProgressService::in_memory(handle.clone());
    sync(&service, &[("wJ:p1", Some(implement(3)))]).await;
    let mut moved = app_pane_repos(&["wJ:p2"]);
    moved.insert(key("wJ:p1"), classified(LIB_REPO, None));
    service.set_pane_repos(moved).await.expect("更新歸類");

    let id = service
        .add_repo_project(NewRepoProject {
            repo: RepoKey::new(LIB_REPO),
            stages: four_stages(),
            name: None,
            phases: Some(phase_strings(four_phases())),
        })
        .await
        .expect("加入 lib");

    assert_eq!(id, lib());
    let domain = domain_of(&handle);
    assert!(!domain.repo_sync.contains_key(&lib()));
    assert!(!domain.repo_progress.contains_key(&lib()));
}

// ---------------------------------------------------------------------------
// worktree 根目錄與偵測當下的歸類（openspec-stage-sync task 4.4；Task 4.3 Ruling）
// ---------------------------------------------------------------------------

fn rooted(root: &str) -> PaneRepo {
    PaneRepo {
        root: Some(s(root)),
        ..classified(APP_REPO, None)
    }
}

/// 同 repo、同 worktree 標註，只有根目錄不同（例如 pane 從 `D:\work\app` 換到另一個也叫主 worktree 的 clone）：
/// 偵測結果屬於舊根目錄，一併丟棄；歸類沒變的 pane 保留（Task 4.3 carry：比較整個歸類去掉 `default_name`）。
#[tokio::test]
async fn root_change_drops_observation() {
    let mut domain = domain_default();
    domain.pane_repos = [
        (key("wJ:p1"), rooted(r"D:\work\app")),
        (key("wJ:p2"), rooted(r"D:\work\app")),
    ]
    .into_iter()
    .collect();
    domain.refresh_projects();
    let handle = StoreHandle::new_with_domain(store_connecting(), domain);
    let service = ProgressService::in_memory(handle.clone());
    service
        .sync_openspec(vec![
            entry("wJ:p1", rooted(r"D:\work\app"), Some(implement(3))),
            entry("wJ:p2", rooted(r"D:\work\app"), Some(implement(3))),
        ])
        .await
        .expect("同步");
    assert_eq!(domain_of(&handle).openspec_obs.len(), 2);

    let moved: PaneRepos = [
        (key("wJ:p1"), rooted(r"D:\work\app2")),
        (key("wJ:p2"), rooted(r"D:\work\app")),
    ]
    .into_iter()
    .collect();
    service.set_pane_repos(moved).await.expect("更新歸類");

    let domain = domain_of(&handle);
    assert!(!domain.openspec_obs.contains_key(&key("wJ:p1")));
    assert_eq!(domain.openspec_obs.get(&key("wJ:p2")), Some(&implement(3)));
}

/// 只有預設名稱不同（git 回報的大小寫不同）不算改歸類：偵測結果保留。
#[tokio::test]
async fn default_name_change_keeps_observation() {
    let handle = StoreHandle::new_with_domain(store_connecting(), domain_default());
    let service = ProgressService::in_memory(handle.clone());
    sync(&service, &[("wJ:p1", Some(implement(3)))]).await;

    let mut renamed = app_pane_repos(&["wJ:p1", "wJ:p2"]);
    renamed.get_mut(&key("wJ:p1")).expect("有 p1").default_name = s("App");
    service.set_pane_repos(renamed).await.expect("更新歸類");

    assert_eq!(
        domain_of(&handle).openspec_obs.get(&key("wJ:p1")),
        Some(&implement(3))
    );
}

/// Task 4.3 Ruling：偵測與送出之間 pane 改了歸類——送來的歸類與寫入鎖內的 `pane_repos` 不符，該筆當無結果：不進
/// `openspec_obs`、卡片不動、不建立同步狀態；歸類相符的 pane 照常套用。
#[tokio::test]
async fn entry_detected_under_previous_classification_is_not_applied() {
    let handle = StoreHandle::new_with_domain(store_connecting(), domain_default());
    let service = ProgressService::in_memory(handle.clone());

    service
        .sync_openspec(vec![
            entry("wJ:p1", rooted(r"D:\work\old"), Some(implement(3))),
            entry("wJ:p2", classified(APP_REPO, None), Some(implement(3))),
        ])
        .await
        .expect("同步");

    let domain = domain_of(&handle);
    assert!(!domain.openspec_obs.contains_key(&key("wJ:p1")));
    assert_eq!(sync_of(&handle, &p1()), None);
    assert_ne!(stage_of(&handle, &p1()).as_deref(), Some("實作"));
    assert_eq!(domain.openspec_obs.get(&key("wJ:p2")), Some(&implement(3)));
    assert_eq!(stage_of(&handle, &p2()).as_deref(), Some("實作"));
}

/// 送來的 pane 已不在任何 repo（`pane_repos` 沒有它）：當無結果。
#[tokio::test]
async fn entry_for_unclassified_pane_is_not_applied() {
    let handle = StoreHandle::new_with_domain(store_connecting(), domain_default());
    let service = ProgressService::in_memory(handle.clone());

    service
        .sync_openspec(vec![entry(
            "wJ:p9",
            classified(APP_REPO, None),
            Some(implement(3)),
        )])
        .await
        .expect("同步");

    assert!(domain_of(&handle).openspec_obs.is_empty());
}

// ---------------------------------------------------------------------------
// 不變式：`openspec_obs` 只含展開中 Repo Project task 的 pane（Task 4.4 review fix round 2）
// ---------------------------------------------------------------------------

/// 有偵測結果 X 的 Repo Project 被移除，再以同一個 repo 重新加入、這一輪偵測為無結果：過時的 X 不得把新卡片移走，
/// 新卡片沒有同步狀態、停在第一個 stage；移除當下 `openspec_obs` 就不含它的 pane。
#[tokio::test]
async fn removed_project_observation_is_not_applied_after_re_adding() {
    let handle = StoreHandle::new_with_domain(store_connecting(), domain_default());
    let service = ProgressService::in_memory(handle.clone());
    sync(&service, &[("wJ:p1", Some(implement(3)))]).await;
    assert_eq!(stage_of(&handle, &p1()).as_deref(), Some("實作"));

    service.remove_repo_project(&app()).await.expect("移除");
    assert!(
        domain_of(&handle).openspec_obs.is_empty(),
        "移除後不得殘留它的 pane 的偵測結果"
    );

    let id = service
        .add_repo_project(NewRepoProject {
            repo: RepoKey::new(APP_REPO),
            stages: four_stages(),
            name: None,
            phases: Some(phase_strings(four_phases())),
        })
        .await
        .expect("重新加入");
    assert_eq!(id, app());
    sync(&service, &[("wJ:p1", None), ("wJ:p2", None)]).await;

    assert_eq!(sync_of(&handle, &p1()), None);
    assert_ne!(stage_of(&handle, &p1()).as_deref(), Some("實作"));
    assert_eq!(projected_sync(&handle, &p1()), None);
}

/// 與手寫 project 同 id 而被隱藏的 Repo Project：它的 pane 沒有展開成 task，偵測結果不得留在 `openspec_obs`
/// （送來的表帶著相符的歸類也一樣），歸類更新後也一樣。
#[tokio::test]
async fn hidden_repo_project_panes_have_no_observation() {
    let mut domain = domain_default();
    domain.repo_projects[0].id = ProjectId::new("h");
    domain.refresh_projects();
    let handle = StoreHandle::new_with_domain(store_connecting(), domain);
    let service = ProgressService::in_memory(handle.clone());

    sync(&service, &[("wJ:p1", Some(implement(3)))]).await;
    assert!(domain_of(&handle).openspec_obs.is_empty());

    service
        .set_pane_repos(app_pane_repos(&["wJ:p1", "wJ:p2"]))
        .await
        .expect("更新歸類");
    assert!(domain_of(&handle).openspec_obs.is_empty());
}
