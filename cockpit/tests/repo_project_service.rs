//! repo-projects task 4.2 驗收測試：寫入服務的 Repo Project 操作（design D3、D4、D6；spec `repo-projects`
//! 「加入 Repo Project」「修改 Repo Project 名稱與 stages」「移除 Repo Project」「Repo Project 的輸入驗證」
//! 「Repo Project 的 id 產生」「Repo Project 每條工作線一張 task」「Repo Project 進度的保存與清除」
//! 「Repo Project 與手寫 project 並列及 id 撞名」「Repo Project 的變更立即生效」）。
//!
//! 全部打真的 `ProgressService`，狀態檔放在每個測試自己的暫存目錄。HTTP 對應屬 task 4.4，這裡只驗服務的
//! 型別化錯誤與狀態。

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use cockpit::progress::{self};
use cockpit::progress_service::{
    BindingBasis, NewRepoProject, ProgressService, RepoProjectError, RepoProjectPatch,
    StageEditInput, WriteError, WriteHook, WriteStage,
};
use cockpit_core::{
    AgentStatus, ConnectionState, DomainState, Focused, Mark, Message, Pane, PaneId, PaneRepo,
    PaneRepos, ProgressOp, ProjectDef, ProjectId, ProjectedState, Rejection, RepoKey,
    RepoProjectDef, RuntimeId, RuntimeSnapshot, RuntimeStore, StoreHandle, TabId, TaskDef, TaskId,
    TaskProgress, WorkspaceId, WorkstreamDef, WorkstreamId, project,
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
            "cockpit-repo-project-service-{tag}-{}-{nanos}",
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
const LIB_REPO: &str = r"d:\work\lib\.git";

fn s(v: &str) -> String {
    v.to_string()
}

fn stages(list: &[&str]) -> Vec<String> {
    list.iter().map(|v| s(v)).collect()
}

fn pid(id: &str) -> ProjectId {
    ProjectId::new(id)
}

fn tid(id: &str) -> TaskId {
    TaskId::new(id)
}

/// 手寫 project（stages `Spec`→`Build`、workstream `be`、task `t1`）。
fn config_project(id: &str) -> ProjectDef {
    ProjectDef {
        id: ProjectId::new(id),
        name: s(id),
        stages: stages(&["Spec", "Build"]),
        workstreams: vec![WorkstreamDef {
            id: WorkstreamId::new("be"),
            name: s("be"),
            binding: None,
            pinned_pane: None,
        }],
        tasks: vec![TaskDef {
            id: tid("t1"),
            title: s("t1"),
            workstream: WorkstreamId::new("be"),
            stage: s("Spec"),
            depends_on: Vec::new(),
        }],
        repo: None,
    }
}

fn pane_repo(repo: &str, default_name: &str) -> PaneRepo {
    PaneRepo {
        repo: RepoKey::new(repo),
        default_name: s(default_name),
        worktree: None,
        root: None,
    }
}

/// `(runtime, pane, repo key, 預設名稱)` → 歸類結果。
fn pane_repos(entries: &[(&str, &str, &str, &str)]) -> PaneRepos {
    entries
        .iter()
        .map(|(runtime, pane, repo, name)| {
            (
                (RuntimeId::new(*runtime), PaneId::new(*pane)),
                pane_repo(repo, name),
            )
        })
        .collect()
}

fn pane(id: &str, exited: bool) -> Pane {
    Pane {
        id: PaneId::new(id),
        workspace_id: WorkspaceId::new("w1"),
        tab_id: TabId::new("t1"),
        agent: None,
        agent_status: AgentStatus::Idle,
        title: None,
        cwd: None,
        label: None,
        focused: false,
        exited,
        updated_at: SystemTime::UNIX_EPOCH,
    }
}

fn snapshot_of(panes: Vec<Pane>) -> RuntimeSnapshot {
    RuntimeSnapshot {
        server_version: s("test"),
        protocol: 1,
        workspaces: Vec::new(),
        tabs: Vec::new(),
        panes,
        agents: Vec::new(),
        focused: Focused {
            workspace_id: None,
            tab_id: None,
            pane_id: None,
        },
        protocol_warning: None,
    }
}

fn connected() -> ConnectionState {
    ConnectionState::Connected {
        since: SystemTime::UNIX_EPOCH,
        server_version: s("test"),
        protocol: 1,
        last_snapshot_at: SystemTime::UNIX_EPOCH,
        settled: true,
        protocol_warning: None,
    }
}

/// runtime `local` 已登記、尚未連上（啟動初期）。
fn store_connecting() -> RuntimeStore {
    let mut store = RuntimeStore::new();
    store.register(RuntimeId::new("local"), s("herdr"), s("test"));
    store
}

/// runtime `local` 已連線，pane 樹為 `panes`（`(id, exited)`）。
fn store_connected(panes: &[(&str, bool)]) -> RuntimeStore {
    let id = RuntimeId::new("local");
    let mut store = store_connecting();
    let panes = panes.iter().map(|(p, exited)| pane(p, *exited)).collect();
    store.replace(&id, snapshot_of(panes)).expect("已登記");
    store.set_connection(&id, connected()).expect("已登記");
    store
}

/// 手寫 project `config`；`local` 的 `wJ:p1`、`wJ:p2` 在 `app` repo、`wJ:p3` 在 `lib` repo。
fn detected_domain(config: Vec<ProjectDef>) -> DomainState {
    let mut domain = DomainState::from_projects(config);
    domain.pane_repos = pane_repos(&[
        ("local", "wJ:p1", APP_REPO, "app"),
        ("local", "wJ:p2", APP_REPO, "app"),
        ("local", "wJ:p3", LIB_REPO, "lib"),
    ]);
    domain.refresh_projects();
    domain
}

/// 在 `domain` 加一個 Repo Project 定義與它的進度（不經服務，模擬從狀態檔載入）。
fn with_repo_project(
    mut domain: DomainState,
    id: &str,
    repo: &str,
    project_stages: &[&str],
    tasks: &[(&str, &str, Mark)],
) -> DomainState {
    domain.repo_projects.push(RepoProjectDef {
        id: pid(id),
        name: s(id),
        repo: RepoKey::new(repo),
        stages: stages(project_stages),
        phases: vec![None; project_stages.len()],
    });
    let table = domain.repo_progress.entry(pid(id)).or_default();
    for (task, stage, mark) in tasks {
        table.insert(
            tid(task),
            TaskProgress {
                stage: s(stage),
                mark: *mark,
            },
        );
    }
    domain.refresh_projects();
    domain
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

fn projection(handle: &StoreHandle) -> ProjectedState {
    let domain = handle.with_domain(Clone::clone);
    handle.with_store(|store| project(store, &domain, 1, SystemTime::now()))
}

fn domain_of(handle: &StoreHandle) -> DomainState {
    handle.with_domain(Clone::clone)
}

fn new_project(repo: &str, project_stages: &[&str], name: Option<&str>) -> NewRepoProject {
    NewRepoProject {
        repo: RepoKey::new(repo),
        stages: stages(project_stages),
        name: name.map(s),
        phases: None,
    }
}

fn four_stages() -> [&'static str; 4] {
    ["Plan", "Build", "Review", "Done"]
}

// ---------------------------------------------------------------------------
// 加入
// ---------------------------------------------------------------------------

/// spec「加入成功」。
#[tokio::test]
async fn add_creates_project_with_tasks_for_each_pane_and_persists() {
    let dir = TempDir::new("add");
    let handle = StoreHandle::new_with_domain(store_connecting(), detected_domain(Vec::new()));
    let service = ProgressService::new(handle.clone(), dir.state_path());

    let id = service
        .add_repo_project(new_project(APP_REPO, &four_stages(), None))
        .await
        .expect("加入應成功");

    assert_eq!(id, pid("app"));
    let state = projection(&handle);
    let app = state
        .projects
        .iter()
        .find(|p| p.id == pid("app"))
        .expect("投影中有 app");
    assert_eq!(app.name, "app");
    assert_eq!(app.stages, stages(&four_stages()));
    assert_eq!(app.workstreams.len(), 2);
    assert!(
        app.tasks
            .iter()
            .all(|t| t.stage == "Plan" && t.mark == Mark::None)
    );
    assert!(
        state
            .detected_repos
            .iter()
            .all(|r| r.repo != RepoKey::new(APP_REPO)),
        "已加入的 repo 不在偵測區"
    );
    assert_eq!(
        read_json(&dir.state_path())["repo_projects"]["app"],
        json!({"name": "app", "repo": APP_REPO, "stages": four_stages(), "phases": [null, null, null, null], "tasks": {}})
    );
}

/// spec「指定名稱」「非法字元與合併」：名稱去除前後空白，id 由名稱產生。
#[tokio::test]
async fn add_with_name_trims_and_derives_id_from_it() {
    let handle = StoreHandle::new_with_domain(store_connecting(), detected_domain(Vec::new()));
    let service = ProgressService::in_memory(handle.clone());

    let id = service
        .add_repo_project(new_project(APP_REPO, &["Plan"], Some("  My App (v2)  ")))
        .await
        .expect("加入應成功");

    assert_eq!(id, pid("My-App-v2"));
    let def = &domain_of(&handle).repo_projects[0];
    assert_eq!(def.name, "My App (v2)");
}

/// spec「與既有 id 重複」：與手寫 project、Repo Project 的 id 都要去重。
#[tokio::test]
async fn add_deduplicates_id_against_config_and_repo_projects() {
    let domain = with_repo_project(
        detected_domain(vec![config_project("app")]),
        "app-2",
        LIB_REPO,
        &["Plan"],
        &[],
    );
    let handle = StoreHandle::new_with_domain(store_connecting(), domain);
    let service = ProgressService::in_memory(handle.clone());

    let id = service
        .add_repo_project(new_project(APP_REPO, &["Plan"], None))
        .await
        .expect("加入應成功");

    assert_eq!(id, pid("app-3"));
}

/// spec「不是偵測到的 repo」。
#[tokio::test]
async fn add_rejects_undetected_repo() {
    let dir = TempDir::new("add-undetected");
    let handle = StoreHandle::new_with_domain(store_connecting(), detected_domain(Vec::new()));
    let service = ProgressService::new(handle.clone(), dir.state_path());
    let before = domain_of(&handle);

    let error = service
        .add_repo_project(new_project(r"c:\anywhere\.git", &["Plan"], None))
        .await
        .expect_err("未偵測到的 repo 應被拒");

    assert!(
        matches!(error, RepoProjectError::RepoNotDetected(_)),
        "{error:?}"
    );
    assert_eq!(error.code(), "repo_not_detected");
    assert_eq!(domain_of(&handle), before);
    assert!(!dir.state_path().exists());
}

/// spec「重複加入」。
#[tokio::test]
async fn add_rejects_repo_already_added() {
    let domain = with_repo_project(detected_domain(Vec::new()), "x", APP_REPO, &["Plan"], &[]);
    let handle = StoreHandle::new_with_domain(store_connecting(), domain);
    let service = ProgressService::in_memory(handle.clone());
    let before = domain_of(&handle);

    let error = service
        .add_repo_project(new_project(APP_REPO, &["Plan"], Some("other")))
        .await
        .expect_err("重複加入應被拒");

    assert!(
        matches!(error, RepoProjectError::RepoAlreadyAdded(_)),
        "{error:?}"
    );
    assert_eq!(error.code(), "repo_already_added");
    assert_eq!(domain_of(&handle), before);
}

/// spec「名稱太長」「stage 數量與重複」「stage 名稱含控制字元」。
#[tokio::test]
async fn add_validates_name_and_stages() {
    let handle = StoreHandle::new_with_domain(store_connecting(), detected_domain(Vec::new()));
    let service = ProgressService::in_memory(handle.clone());
    let before = domain_of(&handle);

    let long_name = "x".repeat(65);
    let error = service
        .add_repo_project(new_project(APP_REPO, &["Plan"], Some(&long_name)))
        .await
        .expect_err("名稱太長");
    assert!(matches!(error, RepoProjectError::InvalidName), "{error:?}");
    assert_eq!(error.code(), "invalid_name");

    let thirteen: Vec<String> = (0..13).map(|i| format!("S{i}")).collect();
    let thirteen: Vec<&str> = thirteen.iter().map(String::as_str).collect();
    for bad in [&[][..], &thirteen[..], &["Plan", "Plan"], &["Plan\u{7}"]] {
        let error = service
            .add_repo_project(new_project(APP_REPO, bad, None))
            .await
            .expect_err("stages 不合法");
        assert!(
            matches!(error, RepoProjectError::InvalidStages),
            "{bad:?} {error:?}"
        );
        assert_eq!(error.code(), "invalid_stages");
    }
    assert_eq!(domain_of(&handle), before, "被拒絕時沒有新增 Project");
}

/// spec「前後空白被去除」。
#[tokio::test]
async fn add_trims_stage_names() {
    let handle = StoreHandle::new_with_domain(store_connecting(), detected_domain(Vec::new()));
    let service = ProgressService::in_memory(handle.clone());

    service
        .add_repo_project(new_project(APP_REPO, &[" Plan ", "Build"], None))
        .await
        .expect("加入應成功");

    assert_eq!(
        domain_of(&handle).repo_projects[0].stages,
        stages(&["Plan", "Build"])
    );
    // openspec-stage-sync task 3.1：加入時 phases 與 stages 對齊（本體尚不帶 phases，全為不對應）。
    assert_eq!(domain_of(&handle).repo_projects[0].phases, vec![None, None]);
}

/// spec「加入時寫檔失敗」：回 persist_failed、記憶體不變（偵測區仍含該 repo）。
#[tokio::test]
async fn add_persist_failure_keeps_memory() {
    let dir = TempDir::new("add-persist");
    fs::create_dir_all(dir.state_path()).expect("在狀態檔路徑建目錄讓 rename 失敗");
    let handle = StoreHandle::new_with_domain(store_connecting(), detected_domain(Vec::new()));
    let service = ProgressService::new(handle.clone(), dir.state_path());
    let before = domain_of(&handle);

    let error = service
        .add_repo_project(new_project(APP_REPO, &["Plan"], None))
        .await
        .expect_err("落檔失敗");

    assert!(
        matches!(error, RepoProjectError::Write(WriteError::Persist { .. })),
        "{error:?}"
    );
    assert_eq!(error.code(), "persist_failed");
    assert_eq!(domain_of(&handle), before);
    assert!(
        projection(&handle)
            .detected_repos
            .iter()
            .any(|r| r.repo == RepoKey::new(APP_REPO))
    );
}

/// 預設名稱不合名稱規則（資料夾名超過 64 字元）時仍可加入：截成合法名稱。
#[tokio::test]
async fn add_with_overlong_default_name_still_succeeds() {
    let mut domain = DomainState::default();
    let long = "a".repeat(70);
    domain.pane_repos = pane_repos(&[("local", "wJ:p1", APP_REPO, &long)]);
    let handle = StoreHandle::new_with_domain(store_connecting(), domain);
    let service = ProgressService::in_memory(handle.clone());

    let id = service
        .add_repo_project(new_project(APP_REPO, &["Plan"], None))
        .await
        .expect("預設名稱過長仍可加入");

    assert_eq!(id.as_str(), "a".repeat(48));
    assert_eq!(domain_of(&handle).repo_projects[0].name, "a".repeat(64));
}

/// repo-projects task 4.6：預設名稱含零寬或雙向覆寫字元時，去掉這些字元後照用（不是退回 `repo`）。
#[tokio::test]
async fn add_strips_format_characters_from_default_name() {
    let domain = DomainState {
        pane_repos: pane_repos(&[("local", "wJ:p1", APP_REPO, "my\u{200B}app\u{202E}")]),
        ..DomainState::default()
    };
    let handle = StoreHandle::new_with_domain(store_connecting(), domain);
    let service = ProgressService::in_memory(handle.clone());

    service
        .add_repo_project(new_project(APP_REPO, &["Plan"], None))
        .await
        .expect("預設名稱含格式字元仍可加入");

    assert_eq!(domain_of(&handle).repo_projects[0].name, "myapp");
}

/// 預設名稱的 ZWJ／ZWNJ 是正當字元：清除其他格式字元時不拆開 emoji 序列（含超長截斷路徑）。
#[tokio::test]
async fn add_keeps_zwj_in_default_name() {
    let overlong = format!("👩\u{200D}💻{}", "a".repeat(70));
    for (folder, expected) in [
        (
            "my\u{200B}app 👩\u{200D}💻",
            "myapp 👩\u{200D}💻".to_string(),
        ),
        ("👩\u{200D}💻 app", "👩\u{200D}💻 app".to_string()),
        (overlong.as_str(), format!("👩\u{200D}💻{}", "a".repeat(61))),
    ] {
        let domain = DomainState {
            pane_repos: pane_repos(&[("local", "wJ:p1", APP_REPO, folder)]),
            ..DomainState::default()
        };
        let handle = StoreHandle::new_with_domain(store_connecting(), domain);
        let service = ProgressService::in_memory(handle.clone());

        service
            .add_repo_project(new_project(APP_REPO, &["Plan"], None))
            .await
            .expect("預設名稱含 ZWJ 仍可加入");

        assert_eq!(
            domain_of(&handle).repo_projects[0].name,
            expected,
            "{folder:?}"
        );
    }
}

/// U+061C（ALM）是雙向格式字元：使用者輸入的名稱含它被拒；預設名稱含它則去掉後照用。
#[tokio::test]
async fn add_rejects_alm_in_given_name_but_strips_from_default_name() {
    let domain = DomainState {
        pane_repos: pane_repos(&[("local", "wJ:p1", APP_REPO, "my\u{061C}app")]),
        ..DomainState::default()
    };
    let handle = StoreHandle::new_with_domain(store_connecting(), domain);
    let service = ProgressService::in_memory(handle.clone());

    let error = service
        .add_repo_project(new_project(APP_REPO, &["Plan"], Some("a\u{061C}b")))
        .await
        .expect_err("名稱含 U+061C");
    assert_eq!(error.code(), "invalid_name");

    service
        .add_repo_project(new_project(APP_REPO, &["Plan"], None))
        .await
        .expect("預設名稱含 U+061C 仍可加入");
    assert_eq!(domain_of(&handle).repo_projects[0].name, "myapp");
}

// ---------------------------------------------------------------------------
// 修改
// ---------------------------------------------------------------------------

fn edit(name: &str, from: Option<&str>) -> StageEditInput {
    StageEditInput {
        name: s(name),
        from: from.map(s),
        phase: None,
    }
}

/// spec「只改名稱」「改名不改 id」。
#[tokio::test]
async fn rename_keeps_id_stages_and_progress() {
    let dir = TempDir::new("rename");
    let domain = with_repo_project(
        detected_domain(Vec::new()),
        "app",
        APP_REPO,
        &["Plan", "Build"],
        &[("local~wJ:p1", "Build", Mark::None)],
    );
    let handle = StoreHandle::new_with_domain(store_connecting(), domain);
    let service = ProgressService::new(handle.clone(), dir.state_path());

    service
        .update_repo_project(
            &pid("app"),
            RepoProjectPatch {
                name: Some(s("App 前端")),
                stages: None,
            },
        )
        .await
        .expect("改名應成功");

    let state = projection(&handle);
    let app = state
        .projects
        .iter()
        .find(|p| p.id == pid("app"))
        .expect("app");
    assert_eq!(app.name, "App 前端");
    assert_eq!(app.stages, stages(&["Plan", "Build"]));
    let file = read_json(&dir.state_path());
    assert_eq!(file["repo_projects"]["app"]["name"], "App 前端");
    assert_eq!(
        file["repo_projects"]["app"]["tasks"]["local~wJ:p1"],
        json!({"stage": "Build", "mark": "none"})
    );
}

/// spec「stage 改名、新增、刪除、排序」：task 跟著 from 對應、被刪除的改到第一個 stage，標記保留。
#[tokio::test]
async fn edit_stages_remaps_tasks_and_keeps_marks() {
    let dir = TempDir::new("edit-stages");
    let domain = with_repo_project(
        detected_domain(Vec::new()),
        "app",
        APP_REPO,
        &["Plan", "Implement", "Review", "Done"],
        &[
            ("local~wJ:p1", "Plan", Mark::None),
            ("local~wJ:p2", "Implement", Mark::Failed),
            ("local~wJ:p8", "Review", Mark::None),
            ("local~wJ:p9", "Done", Mark::Completed),
        ],
    );
    let handle = StoreHandle::new_with_domain(store_connecting(), domain);
    let service = ProgressService::new(handle.clone(), dir.state_path());

    service
        .update_repo_project(
            &pid("app"),
            RepoProjectPatch {
                name: None,
                stages: Some(vec![
                    edit("Plan", Some("Plan")),
                    edit("Design", None),
                    edit("Build", Some("Implement")),
                    edit("Done", Some("Done")),
                ]),
            },
        )
        .await
        .expect("編輯 stages 應成功");

    let file = read_json(&dir.state_path());
    let app = &file["repo_projects"]["app"];
    assert_eq!(app["stages"], json!(["Plan", "Design", "Build", "Done"]));
    // openspec-stage-sync task 3.1：stages 改過後 phases 長度仍與 stages 對齊。
    assert_eq!(
        domain_of(&handle).repo_projects[0].phases,
        vec![None, None, None, None]
    );
    assert_eq!(
        app["tasks"],
        json!({
            "local~wJ:p1": {"stage": "Plan", "mark": "none"},
            "local~wJ:p2": {"stage": "Build", "mark": "failed"},
            "local~wJ:p8": {"stage": "Plan", "mark": "none"},
            "local~wJ:p9": {"stage": "Done", "mark": "completed"},
        })
    );
    let state = projection(&handle);
    let projected = state
        .projects
        .iter()
        .find(|p| p.id == pid("app"))
        .expect("app");
    assert_eq!(
        projected.stages,
        stages(&["Plan", "Design", "Build", "Done"])
    );
    let p2 = projected
        .tasks
        .iter()
        .find(|t| t.id == tid("local~wJ:p2"))
        .expect("p2 的 task");
    assert_eq!((p2.stage.as_str(), p2.mark), ("Build", Mark::Failed));
}

/// spec「同一個舊名稱被引用兩次」「from 不是現有 stage」「兩者同時給但 stages 不合法」「空白名稱」。
#[tokio::test]
async fn invalid_patch_changes_nothing() {
    let domain = with_repo_project(
        detected_domain(Vec::new()),
        "app",
        APP_REPO,
        &["Plan", "Build"],
        &[("local~wJ:p1", "Build", Mark::None)],
    );
    let handle = StoreHandle::new_with_domain(store_connecting(), domain);
    let service = ProgressService::in_memory(handle.clone());
    let before = domain_of(&handle);

    let cases = [
        (
            RepoProjectPatch {
                name: None,
                stages: Some(vec![edit("A", Some("Plan")), edit("B", Some("Plan"))]),
            },
            "invalid_stages",
        ),
        (
            RepoProjectPatch {
                name: None,
                stages: Some(vec![edit("A", Some("Nope"))]),
            },
            "invalid_stages",
        ),
        (
            RepoProjectPatch {
                name: Some(s("新名稱")),
                stages: Some(Vec::new()),
            },
            "invalid_stages",
        ),
        (
            RepoProjectPatch {
                name: Some(s("   ")),
                stages: None,
            },
            "invalid_name",
        ),
    ];
    for (patch, code) in cases {
        let error = service
            .update_repo_project(&pid("app"), patch)
            .await
            .expect_err("應被拒");
        assert_eq!(error.code(), code, "{error:?}");
        assert_eq!(domain_of(&handle), before, "{code}");
    }
}

/// spec「pid 不存在或是手寫 project」「移除不存在或手寫的 project」。
#[tokio::test]
async fn unknown_or_config_pid_is_rejected() {
    let handle = StoreHandle::new_with_domain(
        store_connecting(),
        detected_domain(vec![config_project("hand")]),
    );
    let service = ProgressService::in_memory(handle.clone());
    let before = domain_of(&handle);
    let rename = || RepoProjectPatch {
        name: Some(s("x")),
        stages: None,
    };

    let error = service
        .update_repo_project(&pid("ghost"), rename())
        .await
        .expect_err("不存在");
    assert!(
        matches!(error, RepoProjectError::UnknownProject(_)),
        "{error:?}"
    );
    assert_eq!(error.code(), "unknown_project");
    let error = service
        .update_repo_project(&pid("hand"), rename())
        .await
        .expect_err("手寫");
    assert!(
        matches!(error, RepoProjectError::NotRepoProject(_)),
        "{error:?}"
    );
    assert_eq!(error.code(), "not_repo_project");
    let error = service
        .remove_repo_project(&pid("ghost"))
        .await
        .expect_err("不存在");
    assert_eq!(error.code(), "unknown_project");
    let error = service
        .remove_repo_project(&pid("hand"))
        .await
        .expect_err("手寫");
    assert_eq!(error.code(), "not_repo_project");
    assert_eq!(domain_of(&handle), before);
}

// ---------------------------------------------------------------------------
// 移除
// ---------------------------------------------------------------------------

/// spec「移除成功」「移除後重新加入，進度重新開始」。
#[tokio::test]
async fn remove_drops_definition_and_progress_and_readd_starts_fresh() {
    let dir = TempDir::new("remove");
    let domain = with_repo_project(
        detected_domain(vec![config_project("hand")]),
        "app",
        APP_REPO,
        &["Plan", "Review"],
        &[
            ("local~wJ:p1", "Review", Mark::None),
            ("local~wJ:p2", "Review", Mark::Completed),
        ],
    );
    let handle = StoreHandle::new_with_domain(store_connecting(), domain);
    let service = ProgressService::new(handle.clone(), dir.state_path());
    service
        .apply_progress(&pid("hand"), &tid("t1"), ProgressOp::Advance)
        .await
        .expect("手寫 project 的操作");

    service
        .remove_repo_project(&pid("app"))
        .await
        .expect("移除應成功");

    let state = projection(&handle);
    assert!(state.projects.iter().all(|p| p.id != pid("app")));
    assert!(
        state
            .detected_repos
            .iter()
            .any(|r| r.repo == RepoKey::new(APP_REPO))
    );
    let file = read_json(&dir.state_path());
    assert_eq!(file["repo_projects"], json!({}));
    assert_eq!(
        file["projects"]["hand"]["tasks"]["t1"],
        json!({"stage": "Build", "mark": "none"}),
        "其他 Project 不變"
    );

    let id = service
        .add_repo_project(new_project(APP_REPO, &["Plan", "Review"], None))
        .await
        .expect("重新加入");
    let state = projection(&handle);
    let app = state
        .projects
        .iter()
        .find(|p| p.id == id)
        .expect("重新加入的 app");
    assert!(
        app.tasks
            .iter()
            .all(|t| t.stage == "Plan" && t.mark == Mark::None),
        "進度重新開始"
    );
}

// ---------------------------------------------------------------------------
// id 撞名
// ---------------------------------------------------------------------------

fn conflicting_domain() -> DomainState {
    with_repo_project(
        detected_domain(vec![config_project("app")]),
        "app",
        APP_REPO,
        &["Plan", "Review"],
        &[("local~wJ:p1", "Review", Mark::None)],
    )
}

/// spec「被隱藏的 Repo Project 仍可管理」：改名、移除都成功，移除後撞名警告消失。
#[tokio::test]
async fn hidden_repo_project_can_be_renamed_and_removed() {
    let handle = StoreHandle::new_with_domain(store_connecting(), conflicting_domain());
    let service = ProgressService::in_memory(handle.clone());
    assert!(!domain_of(&handle).warnings.is_empty(), "前提：有撞名警告");

    service
        .update_repo_project(
            &pid("app"),
            RepoProjectPatch {
                name: Some(s("x")),
                stages: None,
            },
        )
        .await
        .expect("被隱藏的 Repo Project 可改名");
    assert_eq!(domain_of(&handle).repo_projects[0].name, "x");

    service
        .remove_repo_project(&pid("app"))
        .await
        .expect("被隱藏的 Repo Project 可移除");
    let domain = domain_of(&handle);
    assert!(domain.repo_projects.is_empty());
    assert!(
        domain.warnings.is_empty(),
        "撞名警告消失：{:?}",
        domain.warnings
    );
}

/// spec「撞名時兩邊進度互不影響」。
#[tokio::test]
async fn conflicting_ids_keep_progress_independent() {
    let dir = TempDir::new("conflict");
    let handle = StoreHandle::new_with_domain(store_connecting(), conflicting_domain());
    let service = ProgressService::new(handle.clone(), dir.state_path());

    service
        .apply_progress(&pid("app"), &tid("t1"), ProgressOp::Complete)
        .await
        .expect("手寫 app 的 t1 complete");
    let domain = domain_of(&handle);
    assert_eq!(
        domain.repo_progress[&pid("app")][&tid("local~wJ:p1")].stage,
        "Review",
        "被隱藏 Repo Project 的進度不變"
    );

    service
        .remove_repo_project(&pid("app"))
        .await
        .expect("移除被隱藏的 Repo Project");
    let file = read_json(&dir.state_path());
    assert_eq!(file["repo_projects"], json!({}));
    assert_eq!(
        file["projects"]["app"]["tasks"]["t1"],
        json!({"stage": "Spec", "mark": "completed"})
    );
    assert_eq!(
        domain_of(&handle).progress[&pid("app")][&tid("t1")].mark,
        Mark::Completed
    );
}

/// 移除未被隱藏的 Repo Project 時，記憶體中它殘留的覆蓋與目前 task 一併清掉；被隱藏時這兩張表屬於同 id 的手寫
/// project，不動（fix round 1 Minor 2）。
#[tokio::test]
async fn remove_clears_residual_overrides_only_when_not_hidden() {
    let residual = |domain: &mut DomainState| {
        domain.overrides.entry(pid("app")).or_default().insert(
            WorkstreamId::new("be"),
            cockpit_core::Override {
                runtime: RuntimeId::new("local"),
                pane_id: PaneId::new("wJ:p7"),
            },
        );
        domain
            .active
            .entry(pid("app"))
            .or_default()
            .insert(WorkstreamId::new("be"), tid("t1"));
    };

    let mut visible = app_domain(&[]);
    residual(&mut visible);
    let handle = StoreHandle::new_with_domain(store_connecting(), visible);
    let service = ProgressService::in_memory(handle.clone());
    service
        .remove_repo_project(&pid("app"))
        .await
        .expect("移除");
    let domain = domain_of(&handle);
    assert!(!domain.overrides.contains_key(&pid("app")));
    assert!(!domain.active.contains_key(&pid("app")));

    let mut hidden = conflicting_domain();
    residual(&mut hidden);
    let handle = StoreHandle::new_with_domain(store_connecting(), hidden);
    let service = ProgressService::in_memory(handle.clone());
    service
        .remove_repo_project(&pid("app"))
        .await
        .expect("移除");
    let domain = domain_of(&handle);
    assert!(
        domain.overrides.contains_key(&pid("app")),
        "手寫 app 的覆蓋不動"
    );
    assert!(
        domain.active.contains_key(&pid("app")),
        "手寫 app 的目前 task 不動"
    );
}

/// 一則載入狀態檔時產生的 stage 退回警告（`Message::TaskStageReset` 的原文）。
fn stage_reset_warning(task: &str) -> String {
    Message::TaskStageReset {
        task: s(task),
        stage: s("Gone"),
        start: s("Plan"),
    }
    .text()
}

/// repo-projects task 4.6 Minor 2：移除未被隱藏的 Repo Project 時，`warnings[pid]`（它載入時的 stage 退回警告）
/// 一併清掉，同 repo、同名再加入拿到同一個 id 也不會冒出舊警告；被隱藏時 `warnings[pid]` 屬於同 id 的手寫
/// project，不動（撞名警告照 `refresh_projects` 規則消失）。
#[tokio::test]
async fn remove_clears_load_warnings_only_when_not_hidden() {
    let mut visible = app_domain(&[("local~wJ:p1", "Plan", Mark::None)]);
    visible
        .warnings
        .insert(pid("app"), vec![stage_reset_warning("local~wJ:p1")]);
    let handle = StoreHandle::new_with_domain(store_connecting(), visible);
    let service = ProgressService::in_memory(handle.clone());
    service
        .remove_repo_project(&pid("app"))
        .await
        .expect("移除");
    assert!(
        !domain_of(&handle).warnings.contains_key(&pid("app")),
        "{:?}",
        domain_of(&handle).warnings
    );
    let id = service
        .add_repo_project(new_project(APP_REPO, &["Plan", "Review"], Some("app")))
        .await
        .expect("重新加入");
    assert_eq!(id, pid("app"), "前提：再加入拿到同一個 id");
    let state = projection(&handle);
    let app = state
        .projects
        .iter()
        .find(|p| p.id == id)
        .expect("重新加入的 app");
    assert!(app.warnings.is_empty(), "{:?}", app.warnings);

    let mut hidden = conflicting_domain();
    let config_warning = Message::TaskStageReset {
        task: s("t1"),
        stage: s("Gone"),
        start: s("Spec"),
    }
    .text();
    hidden
        .warnings
        .entry(pid("app"))
        .or_default()
        .insert(0, config_warning.clone());
    let handle = StoreHandle::new_with_domain(store_connecting(), hidden);
    let service = ProgressService::in_memory(handle.clone());
    service
        .remove_repo_project(&pid("app"))
        .await
        .expect("移除");
    assert_eq!(
        domain_of(&handle).warnings.get(&pid("app")),
        Some(&vec![config_warning]),
        "手寫 app 的載入警告不動，撞名警告消失"
    );
}

/// repo-projects task 4.6 Minor 2：改 stages 後，未被隱藏的 Repo Project 載入時的 stage 退回警告已過時，清掉；
/// 被隱藏時 `warnings[pid]` 屬於同 id 的手寫 project，不動。
#[tokio::test]
async fn stage_edit_clears_stage_reset_warnings_only_when_not_hidden() {
    let patch = || RepoProjectPatch {
        name: None,
        stages: Some(vec![edit("Plan", Some("Plan")), edit("Ship", None)]),
    };

    let mut visible = app_domain(&[("local~wJ:p1", "Plan", Mark::None)]);
    visible
        .warnings
        .insert(pid("app"), vec![stage_reset_warning("local~wJ:p1")]);
    let handle = StoreHandle::new_with_domain(store_connecting(), visible);
    let service = ProgressService::in_memory(handle.clone());
    service
        .update_repo_project(&pid("app"), patch())
        .await
        .expect("改 stages");
    assert!(
        !domain_of(&handle).warnings.contains_key(&pid("app")),
        "{:?}",
        domain_of(&handle).warnings
    );

    let mut hidden = conflicting_domain();
    let config_warning = Message::TaskStageReset {
        task: s("t1"),
        stage: s("Gone"),
        start: s("Spec"),
    }
    .text();
    hidden
        .warnings
        .entry(pid("app"))
        .or_default()
        .insert(0, config_warning.clone());
    let before = hidden.warnings.clone();
    let handle = StoreHandle::new_with_domain(store_connecting(), hidden);
    let service = ProgressService::in_memory(handle.clone());
    service
        .update_repo_project(&pid("app"), patch())
        .await
        .expect("改被隱藏 Repo Project 的 stages");
    assert_eq!(domain_of(&handle).warnings, before, "手寫 app 的警告不動");
    assert!(
        domain_of(&handle).warnings[&pid("app")].contains(&config_warning),
        "前提：手寫 app 的 stage 退回警告仍在"
    );
}

/// repo-projects task 4.6 Minor 3：PATCH 先判定 pid 再驗名稱，與 DELETE 一致——對不存在或手寫的 pid 送不合法名稱，
/// 回 `unknown_project`／`not_repo_project`，不是 `invalid_name`。
#[tokio::test]
async fn patch_checks_pid_before_name() {
    let handle = StoreHandle::new_with_domain(
        store_connecting(),
        detected_domain(vec![config_project("hand")]),
    );
    let service = ProgressService::in_memory(handle.clone());
    let blank = || RepoProjectPatch {
        name: Some(s("   ")),
        stages: None,
    };

    let error = service
        .update_repo_project(&pid("ghost"), blank())
        .await
        .expect_err("不存在");
    assert_eq!(error.code(), "unknown_project", "{error:?}");
    let error = service
        .update_repo_project(&pid("hand"), blank())
        .await
        .expect_err("手寫");
    assert_eq!(error.code(), "not_repo_project", "{error:?}");
}

/// openspec-stage-sync task 4.5：階段字串在服務內依 pid → name → stages → phases 的順序判定。未知字串對不存在或
/// 手寫的 pid 仍回 `unknown_project`／`not_repo_project`，名稱不合法時先回 `invalid_name`。
#[tokio::test]
async fn unknown_phase_string_is_checked_after_pid_and_name() {
    let handle = StoreHandle::new_with_domain(
        store_connecting(),
        detected_domain(vec![config_project("hand")]),
    );
    let service = ProgressService::in_memory(handle.clone());
    service
        .add_repo_project(new_project(APP_REPO, &["A", "B"], None))
        .await
        .expect("加入 app");
    let bad_phase = |name: Option<&str>| RepoProjectPatch {
        name: name.map(s),
        stages: Some(vec![StageEditInput {
            name: s("A"),
            from: Some(s("A")),
            phase: Some(s("nope")),
        }]),
    };

    let error = service
        .update_repo_project(&pid("ghost"), bad_phase(None))
        .await
        .expect_err("不存在");
    assert_eq!(error.code(), "unknown_project", "{error:?}");
    let error = service
        .update_repo_project(&pid("hand"), bad_phase(None))
        .await
        .expect_err("手寫");
    assert_eq!(error.code(), "not_repo_project", "{error:?}");
    let error = service
        .update_repo_project(&pid("app"), bad_phase(Some("   ")))
        .await
        .expect_err("名稱不合法");
    assert_eq!(error.code(), "invalid_name", "{error:?}");
    let error = service
        .update_repo_project(&pid("app"), bad_phase(None))
        .await
        .expect_err("未知階段");
    assert_eq!(error.code(), "invalid_stages", "{error:?}");
}

// ---------------------------------------------------------------------------
// stage 編輯對沒有進度紀錄的 task（fix round 1 Critical 1）：沒有紀錄的 task 在**舊**的第一個 stage，
// 編輯後須依 `from` 對應，不得因新清單的第一個 stage 不同而靜默移動。
// ---------------------------------------------------------------------------

fn task_stages(handle: &StoreHandle, project_id: &str) -> Vec<(String, String)> {
    let state = projection(handle);
    let mut list: Vec<(String, String)> = state
        .projects
        .iter()
        .find(|p| p.id == pid(project_id))
        .expect("投影中有該 project")
        .tasks
        .iter()
        .map(|t| (t.id.as_str().to_string(), t.stage.clone()))
        .collect();
    list.sort();
    list
}

/// spec「重新排序保留 task 所在 stage」：剛加入（沒有任何進度紀錄）的 task 在 `A`，重排成 `B`、`A` 後仍在 `A`。
#[tokio::test]
async fn reorder_keeps_unrecorded_tasks_in_their_stage() {
    let dir = TempDir::new("reorder-unrecorded");
    let handle = StoreHandle::new_with_domain(store_connecting(), detected_domain(Vec::new()));
    let service = ProgressService::new(handle.clone(), dir.state_path());
    let id = service
        .add_repo_project(new_project(APP_REPO, &["A", "B"], None))
        .await
        .expect("加入");

    service
        .update_repo_project(
            &id,
            RepoProjectPatch {
                name: None,
                stages: Some(vec![edit("B", Some("B")), edit("A", Some("A"))]),
            },
        )
        .await
        .expect("重新排序");

    assert_eq!(
        task_stages(&handle, id.as_str()),
        vec![(s("local~wJ:p1"), s("A")), (s("local~wJ:p2"), s("A"))]
    );
    let file = read_json(&dir.state_path());
    assert_eq!(
        file["repo_projects"][id.as_str()]["tasks"]["local~wJ:p1"],
        json!({"stage": "A", "mark": "none"}),
        "重啟後也要在 A"
    );
}

/// 在最前面插入新 stage：沒有紀錄的 task 仍在原本的第一個 stage `Plan`，有紀錄的照 `from` 對應。
#[tokio::test]
async fn inserting_a_first_stage_keeps_unrecorded_tasks_in_place() {
    let handle = StoreHandle::new_with_domain(
        store_connecting(),
        app_domain(&[("local~wJ:p2", "Build", Mark::Failed)]),
    );
    let service = ProgressService::in_memory(handle.clone());

    service
        .update_repo_project(
            &pid("app"),
            RepoProjectPatch {
                name: None,
                stages: Some(vec![
                    edit("Backlog", None),
                    edit("Plan", Some("Plan")),
                    edit("Build", Some("Build")),
                    edit("Review", Some("Review")),
                ]),
            },
        )
        .await
        .expect("插入新 stage");

    assert_eq!(
        task_stages(&handle, "app"),
        vec![
            (s("local~wJ:p1"), s("Plan")),
            (s("local~wJ:p2"), s("Build"))
        ]
    );
    assert_eq!(
        domain_of(&handle).repo_progress[&pid("app")][&tid("local~wJ:p2")].mark,
        Mark::Failed
    );
}

/// 第一個 stage 被刪除時，沒有紀錄的 task 照規則落到新的第一個 stage（不需要補紀錄也正確）；第一個 stage
/// 只改名時跟著新名稱。
#[tokio::test]
async fn deleting_or_renaming_the_first_stage_moves_unrecorded_tasks_by_rule() {
    let handle = StoreHandle::new_with_domain(store_connecting(), app_domain(&[]));
    let service = ProgressService::in_memory(handle.clone());

    service
        .update_repo_project(
            &pid("app"),
            RepoProjectPatch {
                name: None,
                stages: Some(vec![
                    edit("Planning", Some("Plan")),
                    edit("Build", Some("Build")),
                ]),
            },
        )
        .await
        .expect("首位改名");
    assert_eq!(
        task_stages(&handle, "app"),
        vec![
            (s("local~wJ:p1"), s("Planning")),
            (s("local~wJ:p2"), s("Planning"))
        ]
    );

    service
        .update_repo_project(
            &pid("app"),
            RepoProjectPatch {
                name: None,
                stages: Some(vec![edit("Build", Some("Build")), edit("Ship", None)]),
            },
        )
        .await
        .expect("刪除首位");
    assert_eq!(
        task_stages(&handle, "app"),
        vec![
            (s("local~wJ:p1"), s("Build")),
            (s("local~wJ:p2"), s("Build"))
        ]
    );
}

/// 被撞名隱藏的 Repo Project 同樣正確：沒有紀錄的 pane（依 `pane_repos`，不是依展開結果）重排後仍在原本的第一個 stage。
#[tokio::test]
async fn hidden_repo_project_reorder_keeps_unrecorded_tasks() {
    let handle = StoreHandle::new_with_domain(store_connecting(), conflicting_domain());
    let service = ProgressService::in_memory(handle.clone());

    service
        .update_repo_project(
            &pid("app"),
            RepoProjectPatch {
                name: None,
                stages: Some(vec![
                    edit("Review", Some("Review")),
                    edit("Plan", Some("Plan")),
                ]),
            },
        )
        .await
        .expect("重新排序");

    let table = &domain_of(&handle).repo_progress[&pid("app")];
    assert_eq!(
        table[&tid("local~wJ:p1")].stage,
        "Review",
        "有紀錄的照 from"
    );
    assert_eq!(
        table[&tid("local~wJ:p2")].stage,
        "Plan",
        "沒紀錄的留在原本的第一個 stage"
    );
    assert!(
        !table.contains_key(&tid("local~wJ:p3")),
        "其他 repo 的 pane 不補紀錄"
    );
}

// ---------------------------------------------------------------------------
// Repo Project task 的進度操作（控制端裁決：經 progress_for 分流）
// ---------------------------------------------------------------------------

fn app_domain(tasks: &[(&str, &str, Mark)]) -> DomainState {
    with_repo_project(
        detected_domain(Vec::new()),
        "app",
        APP_REPO,
        &["Plan", "Build", "Review"],
        tasks,
    )
}

/// spec「畫面進度操作」：推進寫進 `repo_progress`、投影可見、並寫入狀態檔（修 HEAD 上寫進 `progress` 的靜默失效）。
#[tokio::test]
async fn progress_op_on_repo_task_is_visible_and_persisted() {
    let dir = TempDir::new("repo-advance");
    let handle = StoreHandle::new_with_domain(store_connecting(), app_domain(&[]));
    let service = ProgressService::new(handle.clone(), dir.state_path());

    service
        .apply_progress(&pid("app"), &tid("local~wJ:p1"), ProgressOp::Advance)
        .await
        .expect("推進應成功");

    let state = projection(&handle);
    let task = state
        .projects
        .iter()
        .find(|p| p.id == pid("app"))
        .and_then(|p| p.tasks.iter().find(|t| t.id == tid("local~wJ:p1")))
        .expect("投影中有該 task");
    assert_eq!(task.stage, "Build");
    assert!(!domain_of(&handle).progress.contains_key(&pid("app")));
    // openspec-stage-sync task 4.3：spec「手動操作暫時優先」「沒有偵測結果時手動推進」改變了這裡——人工推進是手動入口，
    // task 沒有同步狀態時建立 `{manual, applied: null}`。
    assert_eq!(
        read_json(&dir.state_path())["repo_projects"]["app"]["tasks"]["local~wJ:p1"],
        json!({"stage": "Build", "mark": "none", "sync": {"mode": "manual", "applied": null}})
    );
}

/// spec「標 Completed 後沒有目前 task」：完成後 active_task 為 null，清除標記後回來。
#[tokio::test]
async fn completing_repo_task_clears_active_task_until_cleared() {
    let handle =
        StoreHandle::new_with_domain(store_connected(&[("wJ:p1", false)]), app_domain(&[]));
    let service = ProgressService::in_memory(handle.clone());
    let active = |handle: &StoreHandle| {
        projection(handle)
            .projects
            .iter()
            .find(|p| p.id == pid("app"))
            .and_then(|p| {
                p.workstreams
                    .iter()
                    .find(|w| w.id == WorkstreamId::new("local~wJ:p1"))
            })
            .expect("工作線")
            .active_task
            .clone()
    };
    assert_eq!(active(&handle), Some(tid("local~wJ:p1")));

    service
        .apply_progress(&pid("app"), &tid("local~wJ:p1"), ProgressOp::Complete)
        .await
        .expect("complete");
    assert_eq!(active(&handle), None);

    service
        .apply_progress(&pid("app"), &tid("local~wJ:p1"), ProgressOp::Clear)
        .await
        .expect("clear");
    assert_eq!(active(&handle), Some(tid("local~wJ:p1")));
}

/// spec「宣告目前 task 是空操作」：通過、狀態不變、不寫檔（沒有進度紀錄也不因補初始值而落檔）。
#[tokio::test]
async fn declaring_unmarked_repo_task_changes_nothing() {
    let dir = TempDir::new("declare-noop");
    let handle = StoreHandle::new_with_domain(store_connecting(), app_domain(&[]));
    let (service, counter) = counting_service(&handle, &dir.state_path());
    let before = domain_of(&handle);

    service
        .declare_active(&pid("app"), &tid("local~wJ:p1"), &BindingBasis::Pinned)
        .await
        .expect("宣告應通過");
    service
        .apply_progress(&pid("app"), &tid("local~wJ:p1"), ProgressOp::Clear)
        .await
        .expect("對沒有標記的 task clear 是空操作");

    assert_eq!(domain_of(&handle), before);
    assert_eq!(writes(&counter), 0);
    assert!(!dir.state_path().exists());
}

/// spec「已標記的 task 不能宣告」。
#[tokio::test]
async fn declaring_marked_repo_task_is_rejected() {
    let handle = StoreHandle::new_with_domain(
        store_connecting(),
        app_domain(&[("local~wJ:p1", "Plan", Mark::Completed)]),
    );
    let service = ProgressService::in_memory(handle.clone());
    let before = domain_of(&handle);

    let error = service
        .declare_active(&pid("app"), &tid("local~wJ:p1"), &BindingBasis::Pinned)
        .await
        .expect_err("已標記應被拒");

    assert!(
        matches!(error, WriteError::Rejected(Rejection::AlreadyMarked)),
        "{error:?}"
    );
    assert_eq!(domain_of(&handle), before);
}

/// agent 推進 Repo Project task：stage 前進，`active` 不為 Repo Project 保存。
#[tokio::test]
async fn agent_advance_on_repo_task_moves_stage_without_storing_active() {
    let handle = StoreHandle::new_with_domain(store_connecting(), app_domain(&[]));
    let service = ProgressService::in_memory(handle.clone());

    service
        .agent_advance(&pid("app"), &tid("local~wJ:p1"), &BindingBasis::Pinned)
        .await
        .expect("推進應成功");

    let domain = domain_of(&handle);
    assert_eq!(
        domain.repo_progress[&pid("app")][&tid("local~wJ:p1")].stage,
        "Build"
    );
    assert!(domain.active.is_empty());
}

// ---------------------------------------------------------------------------
// 清除消失 pane 的進度（design D4）
// ---------------------------------------------------------------------------

/// spec「pane 關閉，進度一併移除」「已 exited 但仍在 pane 樹中不清除」：有清掉才寫檔。
#[tokio::test]
async fn clear_check_removes_closed_panes_and_persists() {
    let dir = TempDir::new("clear");
    let domain = app_domain(&[
        ("local~wJ:p1", "Review", Mark::None),
        ("local~wJ:p2", "Review", Mark::None),
        ("local~wJ:p3", "Build", Mark::None),
    ]);
    let handle = StoreHandle::new_with_domain(
        store_connected(&[("wJ:p1", false), ("wJ:p3", true)]),
        domain,
    );
    let (service, counter) = counting_service(&handle, &dir.state_path());

    service.clear_vanished_progress().await.expect("清除檢查");

    assert_eq!(writes(&counter), 1);
    let tasks = &read_json(&dir.state_path())["repo_projects"]["app"]["tasks"];
    assert_eq!(
        tasks,
        &json!({
            "local~wJ:p1": {"stage": "Review", "mark": "none"},
            "local~wJ:p3": {"stage": "Build", "mark": "none"},
        })
    );

    service.clear_vanished_progress().await.expect("再檢查一次");
    assert_eq!(writes(&counter), 1, "沒清掉東西時不寫檔");
}

/// spec「runtime 斷線不清除」：斷線（pane 樹已空）期間檢查不清除、不寫檔；重連後 snapshot 仍有該 pane，進度仍在。
#[tokio::test]
async fn clear_check_keeps_progress_while_runtime_is_disconnected() {
    let dir = TempDir::new("clear-disconnected");
    let domain = app_domain(&[("local~wJ:p1", "Review", Mark::None)]);
    let handle = StoreHandle::new_with_domain(store_connected(&[("wJ:p1", false)]), domain);
    let (service, counter) = counting_service(&handle, &dir.state_path());
    let local = RuntimeId::new("local");

    handle
        .replace(&local, snapshot_of(Vec::new()))
        .expect("已登記");
    handle
        .set_connection(
            &local,
            ConnectionState::Disconnected {
                reason: s("事件流結束"),
                retry_in: Duration::from_secs(5),
            },
        )
        .expect("已登記");
    service.clear_vanished_progress().await.expect("清除檢查");
    assert_eq!(writes(&counter), 0, "斷線期間不清除、不寫檔");

    handle
        .replace(&local, snapshot_of(vec![pane("wJ:p1", false)]))
        .expect("已登記");
    handle.set_connection(&local, connected()).expect("已登記");
    service.clear_vanished_progress().await.expect("清除檢查");

    assert_eq!(writes(&counter), 0);
    assert_eq!(
        domain_of(&handle).repo_progress[&pid("app")][&tid("local~wJ:p1")].stage,
        "Review"
    );
}

/// spec「Cockpit 關閉期間被關掉的 pane」：歸類結果與上次相同，清除檢查照樣依 runtime 當下的 pane 樹清掉。
#[tokio::test]
async fn clear_check_does_not_depend_on_pane_repos_changing() {
    let domain = app_domain(&[
        ("local~wJ:p1", "Review", Mark::None),
        ("local~wJ:p2", "Build", Mark::None),
    ]);
    let pane_repos_now = domain.pane_repos.clone();
    let handle = StoreHandle::new_with_domain(store_connected(&[("wJ:p1", false)]), domain);
    let service = ProgressService::in_memory(handle.clone());

    service
        .set_pane_repos(pane_repos_now)
        .await
        .expect("歸類結果不變");
    service.clear_vanished_progress().await.expect("清除檢查");

    let table = &domain_of(&handle).repo_progress[&pid("app")];
    assert!(table.contains_key(&tid("local~wJ:p1")));
    assert!(!table.contains_key(&tid("local~wJ:p2")));
}

/// repo-projects task 4.6：runtime 剛連上、沉降重拿還沒換上時，首份 snapshot 少了 pane 也不清、不寫檔；
/// 沉降重拿換上（`settled: true`）後的下一次檢查才清。
#[tokio::test]
async fn clear_check_waits_for_settled_connection() {
    let dir = TempDir::new("clear-unsettled");
    let domain = app_domain(&[
        ("local~wJ:p1", "Review", Mark::None),
        ("local~wJ:p2", "Build", Mark::None),
    ]);
    let handle = StoreHandle::new_with_domain(store_connecting(), domain);
    let (service, counter) = counting_service(&handle, &dir.state_path());
    let local = RuntimeId::new("local");
    let connected_with = |settled: bool| ConnectionState::Connected {
        since: SystemTime::UNIX_EPOCH,
        server_version: s("test"),
        protocol: 1,
        last_snapshot_at: SystemTime::UNIX_EPOCH,
        settled,
        protocol_warning: None,
    };

    handle
        .replace(&local, snapshot_of(vec![pane("wJ:p1", false)]))
        .expect("已登記");
    handle
        .set_connection(&local, connected_with(false))
        .expect("已登記");
    service.clear_vanished_progress().await.expect("清除檢查");
    assert_eq!(writes(&counter), 0, "沉降前不清除、不寫檔");
    assert!(domain_of(&handle).repo_progress[&pid("app")].contains_key(&tid("local~wJ:p2")));

    handle
        .set_connection(&local, connected_with(true))
        .expect("已登記");
    service.clear_vanished_progress().await.expect("清除檢查");

    assert_eq!(writes(&counter), 1);
    let table = &domain_of(&handle).repo_progress[&pid("app")];
    assert!(table.contains_key(&tid("local~wJ:p1")));
    assert!(!table.contains_key(&tid("local~wJ:p2")));
}

/// 清除時寫檔失敗：回 persist_failed、記憶體不變（下一輪會重試）。
#[tokio::test]
async fn clear_check_persist_failure_keeps_memory() {
    let dir = TempDir::new("clear-persist");
    fs::create_dir_all(dir.state_path()).expect("在狀態檔路徑建目錄讓 rename 失敗");
    let domain = app_domain(&[("local~wJ:p2", "Build", Mark::None)]);
    let handle = StoreHandle::new_with_domain(store_connected(&[]), domain);
    let service = ProgressService::new(handle.clone(), dir.state_path());
    let before = domain_of(&handle);

    let error = service
        .clear_vanished_progress()
        .await
        .expect_err("落檔失敗");

    assert!(matches!(error, WriteError::Persist { .. }), "{error:?}");
    assert_eq!(domain_of(&handle), before);
}

// ---------------------------------------------------------------------------
// 啟動初期與並發
// ---------------------------------------------------------------------------

/// spec「啟動初期的寫入不抹掉進度」：runtime 尚未連上、`pane_repos` 為空時，手寫 project 的操作與
/// Repo Project 的改名都不抹掉 `app` 兩張 task 的進度。
#[tokio::test]
async fn early_writes_with_empty_pane_repos_keep_repo_progress() {
    let dir = TempDir::new("early");
    let mut domain = DomainState::from_projects(vec![config_project("hand")]);
    domain = with_repo_project(
        domain,
        "app",
        APP_REPO,
        &["Plan", "Review"],
        &[
            ("local~wJ:p1", "Review", Mark::None),
            ("local~wJ:p2", "Plan", Mark::Failed),
        ],
    );
    assert!(domain.pane_repos.is_empty());
    let handle = StoreHandle::new_with_domain(store_connecting(), domain);
    let service = ProgressService::new(handle.clone(), dir.state_path());
    let expected = json!({
        "local~wJ:p1": {"stage": "Review", "mark": "none"},
        "local~wJ:p2": {"stage": "Plan", "mark": "failed"},
    });

    service
        .apply_progress(&pid("hand"), &tid("t1"), ProgressOp::Complete)
        .await
        .expect("手寫 project 的操作");
    assert_eq!(
        read_json(&dir.state_path())["repo_projects"]["app"]["tasks"],
        expected
    );

    service
        .update_repo_project(
            &pid("app"),
            RepoProjectPatch {
                name: Some(s("App")),
                stages: None,
            },
        )
        .await
        .expect("改名");
    service
        .clear_vanished_progress()
        .await
        .expect("尚未連上時清除檢查");
    assert_eq!(
        read_json(&dir.state_path())["repo_projects"]["app"]["tasks"],
        expected
    );
}

/// spec「並發寫入不遺失」：幾乎同時送 `x` 的 complete、`app` 的改名與另一個 Repo Project 的加入。
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_repo_project_writes_are_not_lost() {
    for round in 0..20 {
        let dir = TempDir::new(&format!("concurrent-{round}"));
        let handle = StoreHandle::new_with_domain(
            store_connecting(),
            app_domain(&[
                ("local~wJ:p1", "Plan", Mark::None),
                ("local~wJ:p2", "Plan", Mark::None),
            ]),
        );
        let service = ProgressService::new(handle.clone(), dir.state_path());

        let complete = {
            let service = service.clone();
            tokio::spawn(async move {
                service
                    .apply_progress(&pid("app"), &tid("local~wJ:p1"), ProgressOp::Complete)
                    .await
            })
        };
        let rename = {
            let service = service.clone();
            tokio::spawn(async move {
                service
                    .update_repo_project(
                        &pid("app"),
                        RepoProjectPatch {
                            name: Some(s("新名稱")),
                            stages: None,
                        },
                    )
                    .await
            })
        };
        let add = {
            let service = service.clone();
            tokio::spawn(async move {
                service
                    .add_repo_project(new_project(LIB_REPO, &["Plan"], None))
                    .await
            })
        };
        complete.await.expect("不 panic").expect("complete 被接受");
        rename.await.expect("不 panic").expect("改名被接受");
        let lib = add.await.expect("不 panic").expect("加入被接受");

        let file = read_json(&dir.state_path());
        assert_eq!(
            file["repo_projects"]["app"]["tasks"]["local~wJ:p1"]["mark"], "completed",
            "round {round}"
        );
        assert_eq!(
            file["repo_projects"]["app"]["name"], "新名稱",
            "round {round}"
        );
        assert_eq!(
            file["repo_projects"][lib.as_str()]["repo"],
            LIB_REPO,
            "round {round}"
        );
    }
}

/// 寫出的狀態檔可經載入路徑讀回，且 Repo Project 的定義與進度一致（重啟後保留）。
#[tokio::test]
async fn written_repo_projects_reload_identically() {
    let dir = TempDir::new("reload");
    let handle = StoreHandle::new_with_domain(store_connecting(), detected_domain(Vec::new()));
    let service = ProgressService::new(handle.clone(), dir.state_path());
    let id = service
        .add_repo_project(new_project(APP_REPO, &["Plan", "Build"], None))
        .await
        .expect("加入");
    service
        .apply_progress(&id, &tid("local~wJ:p1"), ProgressOp::Advance)
        .await
        .expect("推進");

    let runtimes: HashSet<&str> = HashSet::from(["local"]);
    let reloaded =
        progress::load_progress(&dir.state_path(), Vec::new(), &runtimes).expect("可讀回");
    let memory = domain_of(&handle);
    assert_eq!(reloaded.repo_projects, memory.repo_projects);
    assert_eq!(reloaded.repo_progress, memory.repo_progress);
}
