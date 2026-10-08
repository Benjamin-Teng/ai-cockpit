//! repo-projects task 4.1 驗收測試：狀態檔 v3（design D3「何時寫檔」、D4、D5；spec `pipeline-progress`
//! 「狀態檔格式與持久化」「狀態檔載入與容錯」、`repo-projects`「Repo Project 進度的保存與清除」）。
//!
//! 載入規則打 `progress::load_progress`；寫出規則打真的 `ProgressService`（狀態檔放在每個測試自己的暫存目錄）。
//! Repo Project 的加入／修改／移除端點屬 task 4.2，這裡的 Repo Project 一律來自預先寫好的狀態檔。

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use cockpit::progress::{self, ProgressError};
use cockpit::progress_service::{ProgressService, StateFileTarget, WriteHook, WriteStage};
use cockpit_core::{
    DomainState, Mark, PaneId, PaneRepo, PaneRepos, ProgressOp, ProjectDef, ProjectId, RepoKey,
    RuntimeId, RuntimeStore, StoreHandle, TaskDef, TaskId, TaskProgress, WorkstreamDef,
    WorkstreamId,
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
            "cockpit-state-v3-test-{tag}-{}-{nanos}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("建立測試暫存目錄");
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

const APP_REPO: &str = r"d:\work\app\.git";

/// 手寫 project `p`（stages `Spec`→`Build`、workstream `be`、task `t1`）。
fn config_project(id: &str) -> ProjectDef {
    ProjectDef {
        id: ProjectId::new(id),
        name: id.to_string(),
        stages: vec!["Spec".to_string(), "Build".to_string()],
        workstreams: vec![WorkstreamDef {
            id: WorkstreamId::new("be"),
            name: "be".to_string(),
            binding: None,
            pinned_pane: None,
        }],
        tasks: vec![TaskDef {
            id: TaskId::new("t1"),
            title: "t1".to_string(),
            workstream: WorkstreamId::new("be"),
            stage: "Spec".to_string(),
            depends_on: Vec::new(),
        }],
        repo: None,
    }
}

fn runtimes() -> HashSet<&'static str> {
    HashSet::from(["local", "dev~1"])
}

/// 在 `dir` 寫入狀態檔 `contents`，以手寫 project `projects` 與 runtime `local`、`dev~1` 載入。
fn load(
    dir: &TempDir,
    contents: &str,
    projects: Vec<ProjectDef>,
) -> (PathBuf, Result<DomainState, ProgressError>) {
    let path = dir.path().join("cockpit.state.json");
    fs::write(&path, contents).expect("寫入狀態檔");
    let result = progress::load_progress(&path, projects, &runtimes());
    (path, result)
}

fn assert_corrupt_with_path(tag: &str, contents: &str, reason: &str) {
    let dir = TempDir::new(tag);
    let (path, result) = load(&dir, contents, vec![config_project("p")]);
    let error = match result {
        Ok(_) => panic!("{tag}：應視為損毀、啟動失敗"),
        Err(error) => error,
    };
    let message = error.to_string();
    assert!(
        message.contains(&path.display().to_string()),
        "{tag}：訊息應含狀態檔路徑：{message}"
    );
    assert!(
        message.contains(reason),
        "{tag}：訊息應指出原因 {reason}：{message}"
    );
}

fn v3(repo_projects: &str) -> String {
    format!(r#"{{"version":3,"projects":{{}},"repo_projects":{repo_projects}}}"#)
}

fn app_entry(tasks: &str) -> String {
    format!(
        r#"{{"name":"App","repo":"d:\\work\\app\\.git","stages":["Plan","Build"],"tasks":{tasks}}}"#
    )
}

fn repo_progress_of(domain: &DomainState, pid: &str, tid: &str) -> Option<TaskProgress> {
    domain
        .repo_progress
        .get(&ProjectId::new(pid))
        .and_then(|m| m.get(&TaskId::new(tid)))
        .cloned()
}

fn pane_repos_with(panes: &[(&str, &str)]) -> PaneRepos {
    panes
        .iter()
        .map(|(runtime, pane)| {
            (
                (RuntimeId::new(*runtime), PaneId::new(*pane)),
                PaneRepo {
                    repo: RepoKey::new(APP_REPO),
                    default_name: "app".to_string(),
                    worktree: None,
                },
            )
        })
        .collect()
}

fn read_json(path: &Path) -> Value {
    serde_json::from_str(&fs::read_to_string(path).expect("讀狀態檔")).expect("狀態檔應為 JSON")
}

/// 計算落檔次數的鉤子。
fn counting_hook() -> (WriteHook, Arc<AtomicUsize>) {
    let count = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&count);
    let hook: WriteHook = Arc::new(move |stage| {
        if stage == WriteStage::Start {
            counter.fetch_add(1, Ordering::SeqCst);
        }
    });
    (hook, count)
}

// ---------------------------------------------------------------------------
// 載入
// ---------------------------------------------------------------------------

/// Scenario「讀取含 Repo Project 的 v3 檔」。
#[test]
fn v3_file_with_repo_project_loads_definition_and_progress() {
    let dir = TempDir::new("v3-load");
    let (_, result) = load(
        &dir,
        &v3(&format!(
            r#"{{"app":{}}}"#,
            app_entry(r#"{"local~wJ:p1":{"stage":"Build","mark":"failed"}}"#)
        )),
        Vec::new(),
    );
    let mut domain = result.expect("合法 v3 應載入成功");

    assert_eq!(domain.repo_projects.len(), 1);
    let def = &domain.repo_projects[0];
    assert_eq!(def.id.as_str(), "app");
    assert_eq!(def.name, "App");
    assert_eq!(def.repo, RepoKey::new(APP_REPO));
    assert_eq!(def.stages, vec!["Plan".to_string(), "Build".to_string()]);
    let app = domain
        .projects
        .iter()
        .find(|p| p.id.as_str() == "app")
        .expect("projects 含 Repo Project app");
    assert_eq!(app.repo, Some(RepoKey::new(APP_REPO)));

    // pane 歸入該 repo 後，其 task 在 Build、標記 failed。
    domain.pane_repos = pane_repos_with(&[("local", "wJ:p1")]);
    domain.refresh_projects();
    let app = domain
        .projects
        .iter()
        .find(|p| p.id.as_str() == "app")
        .expect("app")
        .clone();
    assert!(app.tasks.iter().any(|t| t.id.as_str() == "local~wJ:p1"));
    let table = domain.progress_for(&app).expect("app 有進度表");
    assert_eq!(
        table.get(&TaskId::new("local~wJ:p1")),
        Some(&TaskProgress {
            stage: "Build".to_string(),
            mark: Mark::Failed
        })
    );
}

/// Scenario「v3 檔缺 repo_projects 視為損毀」＋空物件可載入。
#[test]
fn v3_file_requires_repo_projects() {
    assert_corrupt_with_path(
        "v3-missing",
        r#"{"version":3,"projects":{}}"#,
        "repo_projects",
    );
    assert_corrupt_with_path(
        "v3-null",
        r#"{"version":3,"projects":{},"repo_projects":null}"#,
        "repo_projects",
    );

    let dir = TempDir::new("v3-empty");
    let (_, result) = load(&dir, &v3("{}"), vec![config_project("p")]);
    let domain = result.expect("repo_projects 為空物件應載入成功");
    assert!(domain.repo_projects.is_empty());
    assert!(domain.repo_progress.is_empty());
}

/// Scenario「v1、v2 檔出現 repo_projects 視為損毀」（不論值為何，含 null）。
#[test]
fn v1_and_v2_files_with_repo_projects_are_corrupt() {
    assert_corrupt_with_path(
        "v1-repo",
        r#"{"version":1,"projects":{},"repo_projects":{}}"#,
        "repo_projects",
    );
    assert_corrupt_with_path(
        "v2-repo",
        r#"{"version":2,"projects":{},"repo_projects":{}}"#,
        "repo_projects",
    );
    assert_corrupt_with_path(
        "v2-repo-null",
        r#"{"version":2,"projects":{},"repo_projects":null}"#,
        "repo_projects",
    );
}

/// Scenario「不支援的版本」：version 4。
#[test]
fn version_4_is_unsupported() {
    let dir = TempDir::new("v4");
    let (path, result) = load(
        &dir,
        r#"{"version":4,"projects":{},"repo_projects":{}}"#,
        Vec::new(),
    );
    let error = result.expect_err("version 4 應啟動失敗");
    assert!(
        matches!(error, ProgressError::UnsupportedVersion { .. }),
        "{error:?}"
    );
    assert!(error.to_string().contains(&path.display().to_string()));
}

/// Scenario「兩個 Repo Project 的 repo 相同視為損毀」。
#[test]
fn two_repo_projects_with_the_same_repo_are_corrupt() {
    let entry = app_entry("{}");
    assert_corrupt_with_path(
        "dup-repo",
        &v3(&format!(r#"{{"a":{entry},"b":{entry}}}"#)),
        "repo",
    );
}

/// Scenario「Repo Project 定義不合法」：stages 為空、id 含 `/`、名稱為空字串，以及其他違反 D6 規則的定義。
#[test]
fn invalid_repo_project_definitions_are_corrupt() {
    let with = |id: &str, name: &str, stages: &str| {
        v3(&format!(
            r#"{{"{id}":{{"name":{name},"repo":"d:\\x\\.git","stages":{stages},"tasks":{{}}}}}}"#
        ))
    };
    let long_name = format!("\"{}\"", "a".repeat(65));
    let thirteen = format!(
        "[{}]",
        (0..13)
            .map(|i| format!("\"S{i}\""))
            .collect::<Vec<_>>()
            .join(",")
    );
    let cases = [
        ("empty-stages", with("app", "\"App\"", "[]")),
        ("slash-id", with("a/b", "\"App\"", "[\"Plan\"]")),
        ("empty-name", with("app", "\"\"", "[\"Plan\"]")),
        ("blank-name", with("app", "\"   \"", "[\"Plan\"]")),
        ("long-name", with("app", &long_name, "[\"Plan\"]")),
        ("dup-stage", with("app", "\"App\"", "[\"Plan\",\"Plan\"]")),
        ("ctrl-stage", with("app", "\"App\"", "[\"Plan\\u0007\"]")),
        ("too-many-stages", with("app", "\"App\"", &thirteen)),
        // repo-projects task 4.6：格式字元與 id 長度上限。
        ("bidi-name", with("app", "\"A\\u202EB\"", "[\"Plan\"]")),
        (
            "zero-width-stage",
            with("app", "\"App\"", "[\"Pl\\u200Ban\"]"),
        ),
        ("long-id", with(&"a".repeat(65), "\"App\"", "[\"Plan\"]")),
    ];
    for (tag, contents) in cases {
        assert_corrupt_with_path(tag, &contents, "repo_projects");
    }
}

/// Repo Project 不保存 `overrides` 與 `active`；出現即違反形狀（`deny_unknown_fields`）。
#[test]
fn repo_project_with_overrides_or_active_is_corrupt() {
    for (tag, extra) in [
        ("overrides", r#""overrides":{}"#),
        ("active", r#""active":{}"#),
    ] {
        let contents = v3(&format!(
            r#"{{"app":{{"name":"App","repo":"d:\\x\\.git","stages":["Plan"],"tasks":{{}},{extra}}}}}"#
        ));
        let dir = TempDir::new(tag);
        let (_, result) = load(&dir, &contents, Vec::new());
        assert!(
            matches!(result, Err(ProgressError::Parse { .. })),
            "{tag} 應視為損毀"
        );
    }
}

/// 名稱與 stages 依輸入驗證規則去除前後空白後保存。
#[test]
fn repo_project_name_and_stages_are_trimmed_on_load() {
    let dir = TempDir::new("trim");
    let (_, result) = load(
        &dir,
        &v3(
            r#"{"app":{"name":" App ","repo":"d:\\x\\.git","stages":[" Plan ","Build"],"tasks":{}}}"#,
        ),
        Vec::new(),
    );
    let domain = result.expect("前後空白不算違規");
    assert_eq!(domain.repo_projects[0].name, "App");
    assert_eq!(
        domain.repo_projects[0].stages,
        vec!["Plan".to_string(), "Build".to_string()]
    );
}

/// Scenario「Repo Project 的 task 的 stage 被改掉」：改用第一個 stage、保留標記、加警告。
#[test]
fn repo_task_with_unknown_stage_falls_back_to_first_stage_with_warning() {
    let dir = TempDir::new("stage-reset");
    let (_, result) = load(
        &dir,
        &v3(&format!(
            r#"{{"app":{}}}"#,
            app_entry(r#"{"local~wJ:p1":{"stage":"Gone","mark":"completed"}}"#)
        )),
        Vec::new(),
    );
    let domain = result.expect("stage 不存在不算損毀");

    assert_eq!(
        repo_progress_of(&domain, "app", "local~wJ:p1"),
        Some(TaskProgress {
            stage: "Plan".to_string(),
            mark: Mark::Completed
        })
    );
    let warnings = domain
        .warnings
        .get(&ProjectId::new("app"))
        .expect("app 應有 warning");
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains("local~wJ:p1"), "{warnings:?}");
    assert!(warnings[0].contains("Gone"), "{warnings:?}");
}

/// 被同 id 手寫 project 隱藏的 Repo Project，其 stage 警告不掛到手寫 project 上（兩邊互不影響）。
#[test]
fn hidden_repo_project_stage_warning_does_not_land_on_the_config_project() {
    let dir = TempDir::new("stage-reset-hidden");
    let (_, result) = load(
        &dir,
        &format!(
            r#"{{"version":3,"projects":{{}},"repo_projects":{{"p":{}}}}}"#,
            app_entry(r#"{"local~wJ:p1":{"stage":"Gone","mark":"none"}}"#)
        ),
        vec![config_project("p")],
    );
    let domain = result.expect("載入成功");
    let warnings = domain
        .warnings
        .get(&ProjectId::new("p"))
        .cloned()
        .unwrap_or_default();
    assert!(
        warnings.iter().all(|w| !w.contains("Gone")),
        "手寫 p 不應出現被隱藏 Repo Project 的 stage 警告：{warnings:?}"
    );
    assert_eq!(
        repo_progress_of(&domain, "p", "local~wJ:p1").map(|p| p.stage),
        Some("Plan".to_string()),
        "被隱藏的 Repo Project 進度照規則保留"
    );
}

/// Scenario「runtime id 含 ~ 時從最後一個 ~ 切」＋「指向未設定的 runtime」：前者保留，後者與沒有 `~` 的
/// task id 都忽略。
#[test]
fn repo_task_runtime_is_taken_before_the_last_tilde() {
    let dir = TempDir::new("tilde");
    let (_, result) = load(
        &dir,
        &v3(&format!(
            r#"{{"app":{}}}"#,
            app_entry(
                r#"{"dev~1~wJ:p1":{"stage":"Build","mark":"none"},"ghost~wJ:p2":{"stage":"Build","mark":"none"},"nowhere":{"stage":"Plan","mark":"none"}}"#
            )
        )),
        Vec::new(),
    );
    let domain = result.expect("未知 runtime 不算損毀");
    assert!(repo_progress_of(&domain, "app", "dev~1~wJ:p1").is_some());
    assert!(repo_progress_of(&domain, "app", "ghost~wJ:p2").is_none());
    assert!(repo_progress_of(&domain, "app", "nowhere").is_none());
}

/// Scenario「Repo Project 的顯示順序不依賴檔案順序」。
#[test]
fn repo_projects_are_listed_by_name_regardless_of_file_order() {
    let dir = TempDir::new("order");
    let (_, result) = load(
        &dir,
        &v3(
            r#"{"zeta":{"name":"zeta","repo":"d:\\z\\.git","stages":["A"],"tasks":{}},"alpha":{"name":"alpha","repo":"d:\\a\\.git","stages":["A"],"tasks":{}}}"#,
        ),
        vec![config_project("p")],
    );
    let domain = result.expect("載入成功");
    let ids: Vec<&str> = domain.projects.iter().map(|p| p.id.as_str()).collect();
    assert_eq!(ids, vec!["p", "alpha", "zeta"]);
}

// ---------------------------------------------------------------------------
// 寫出
// ---------------------------------------------------------------------------

/// Scenario「寫出 Repo Project」：`version` 3；`repo_projects.app` 逐字相符、不含 `overrides`／`active`；
/// 手寫的 `projects` 區段不含 Repo Project。
#[tokio::test]
async fn written_file_is_v3_with_repo_projects_section() {
    let dir = TempDir::new("write-v3");
    let (path, result) = load(
        &dir,
        r#"{"version":2,"projects":{}}"#,
        vec![config_project("p")],
    );
    let mut domain = result.expect("載入 v2");
    // 4.2 的加入端點尚未存在：直接放一個已加入的 Repo Project 與它的進度。
    let (_, seeded) = load(
        &TempDir::new("write-v3-seed"),
        &v3(&format!(
            r#"{{"app":{}}}"#,
            app_entry(r#"{"local~wJ:p1":{"stage":"Build","mark":"none"}}"#)
        )),
        Vec::new(),
    );
    let seeded = seeded.expect("seed");
    domain.repo_projects = seeded.repo_projects;
    domain.repo_progress = seeded.repo_progress;
    domain.pane_repos = pane_repos_with(&[("local", "wJ:p1")]);
    domain.refresh_projects();

    let handle = StoreHandle::new_with_domain(RuntimeStore::new(), domain);
    let service = ProgressService::new(handle, path.clone());
    service
        .apply_progress(
            &ProjectId::new("p"),
            &TaskId::new("t1"),
            ProgressOp::Advance,
        )
        .await
        .expect("推進應成功");

    let written = read_json(&path);
    assert_eq!(written["version"], 3);
    assert_eq!(
        written["repo_projects"]["app"],
        json!({"name":"App","repo":"d:\\work\\app\\.git","stages":["Plan","Build"],"tasks":{"local~wJ:p1":{"stage":"Build","mark":"none"}}})
    );
    let section_keys: Vec<&String> = written["projects"]
        .as_object()
        .expect("projects 物件")
        .keys()
        .collect();
    assert_eq!(section_keys, vec!["p"], "projects 區段只放手寫 project");
    assert_eq!(written["projects"]["p"]["tasks"]["t1"]["stage"], "Build");

    // 寫出的檔案能讀回同樣的 Repo Project。
    let reloaded =
        progress::load_progress(&path, vec![config_project("p")], &runtimes()).expect("讀回 v3");
    assert_eq!(reloaded.repo_projects.len(), 1);
    assert_eq!(
        repo_progress_of(&reloaded, "app", "local~wJ:p1").map(|p| p.stage),
        Some("Build".to_string())
    );
}

/// Scenario「啟動初期的寫入不抹掉進度」：`pane_repos` 為空（沒有任何展開出來的 task）時，對手寫 project 的
/// 操作寫出的檔案仍含 Repo Project 的全部進度；指向未設定 runtime 的項目不寫出（Scenario「指向未設定 runtime
/// 的進度」）。
#[tokio::test]
async fn write_keeps_all_repo_progress_and_drops_unknown_runtime_items() {
    let dir = TempDir::new("write-all");
    let (path, result) = load(
        &dir,
        &format!(
            r#"{{"version":3,"projects":{{}},"repo_projects":{{"app":{}}}}}"#,
            app_entry(
                r#"{"local~wJ:p1":{"stage":"Build","mark":"none"},"local~wJ:p2":{"stage":"Plan","mark":"failed"},"dev~1~wJ:p3":{"stage":"Plan","mark":"none"},"ghost~wJ:p4":{"stage":"Plan","mark":"none"}}"#
            )
        ),
        vec![config_project("p")],
    );
    let domain = result.expect("載入");
    assert!(domain.pane_repos.is_empty());
    let handle = StoreHandle::new_with_domain(RuntimeStore::new(), domain);
    let service = ProgressService::new(handle, path.clone());
    service
        .apply_progress(
            &ProjectId::new("p"),
            &TaskId::new("t1"),
            ProgressOp::Complete,
        )
        .await
        .expect("操作應成功");

    let written = read_json(&path);
    assert_eq!(
        written["repo_projects"]["app"]["tasks"],
        json!({
            "dev~1~wJ:p3": {"stage":"Plan","mark":"none"},
            "local~wJ:p1": {"stage":"Build","mark":"none"},
            "local~wJ:p2": {"stage":"Plan","mark":"failed"},
        })
    );
}

/// Scenario「只有 pane 進出與 cwd 改變時不改寫 v2 檔」：沒有任何被接受的操作，只有 pane 歸類改變 → 檔案
/// 位元組不變（仍是 v2），且完全沒有落檔；記憶體照樣更新。
#[tokio::test]
async fn pane_changes_alone_do_not_rewrite_a_v2_file() {
    let dir = TempDir::new("v2-untouched");
    let original = r#"{"version":2,"projects":{"p":{"tasks":{"t1":{"stage":"Build","mark":"none"}},"overrides":{},"active":{"be":"t1"}}}}"#;
    let (path, result) = load(&dir, original, vec![config_project("p")]);
    let domain = result.expect("載入 v2");
    let handle = StoreHandle::new_with_domain(RuntimeStore::new(), domain);
    let (hook, writes) = counting_hook();
    let service = ProgressService::with_write_hook(handle.clone(), path.clone(), hook);

    service
        .set_pane_repos(pane_repos_with(&[("local", "wJ:p1"), ("local", "wJ:p2")]))
        .await
        .expect("pane 進入");
    service
        .set_pane_repos(pane_repos_with(&[("local", "wJ:p2")]))
        .await
        .expect("pane 離開");
    service
        .set_pane_repos(PaneRepos::new())
        .await
        .expect("cwd 離開 repo");

    assert_eq!(writes.load(Ordering::SeqCst), 0, "不應落檔");
    assert_eq!(
        fs::read_to_string(&path).expect("讀檔"),
        original,
        "位元組不變"
    );
    assert!(
        handle.with_domain(|d| d.pane_repos.is_empty()),
        "記憶體中的歸類結果照樣更新"
    );
}

/// Scenario「寫出內容與現有檔案相同就不寫」：v3 檔與記憶體序列化結果相同時，只改歸類不寫檔（修改時間不變）。
#[tokio::test]
async fn identical_serialization_is_not_rewritten() {
    let dir = TempDir::new("v3-untouched");
    let (path, result) = load(
        &dir,
        &v3(&format!(
            r#"{{"app":{}}}"#,
            app_entry(r#"{"local~wJ:p1":{"stage":"Build","mark":"none"}}"#)
        )),
        Vec::new(),
    );
    let domain = result.expect("載入 v3");
    let before = fs::metadata(&path)
        .and_then(|m| m.modified())
        .expect("mtime");
    let handle = StoreHandle::new_with_domain(RuntimeStore::new(), domain);
    let (hook, writes) = counting_hook();
    let service = ProgressService::with_write_hook(handle.clone(), path.clone(), hook);

    service
        .set_pane_repos(pane_repos_with(&[("local", "wJ:p1")]))
        .await
        .expect("pane 歸入");

    assert_eq!(writes.load(Ordering::SeqCst), 0, "不應落檔");
    let after = fs::metadata(&path)
        .and_then(|m| m.modified())
        .expect("mtime");
    assert_eq!(before, after, "修改時間不變");
    let app_tasks = handle.with_domain(|d| {
        d.projects
            .iter()
            .find(|p| p.id.as_str() == "app")
            .map(|p| p.tasks.len())
    });
    assert_eq!(app_tasks, Some(1), "重算後的生效清單照樣生效");
}

/// 沒有狀態檔路徑時（inline 設定、沒有 `LOCALAPPDATA`）只更新記憶體。
#[tokio::test]
async fn in_memory_service_updates_memory_without_a_file() {
    let handle = StoreHandle::new_with_domain(
        RuntimeStore::new(),
        DomainState::from_projects(vec![config_project("p")]),
    );
    let service = ProgressService::in_memory(handle.clone());
    service
        .apply_progress(
            &ProjectId::new("p"),
            &TaskId::new("t1"),
            ProgressOp::Complete,
        )
        .await
        .expect("沒有路徑時操作照樣成功");
    let mark = handle.with_domain(|d| d.progress[&ProjectId::new("p")][&TaskId::new("t1")].mark);
    assert_eq!(mark, Mark::Completed);
}

/// 零設定模式：狀態檔所在資料夾在第一次寫入時建立；有設定檔時維持原樣（資料夾不存在就是寫檔失敗）。
#[tokio::test]
async fn zero_config_target_creates_its_folder_on_first_write() {
    let dir = TempDir::new("create-dir");
    let folder = dir.path().join("Local").join("ai-cockpit");
    let path = folder.join("cockpit.state.json");
    let handle = StoreHandle::new_with_domain(
        RuntimeStore::new(),
        DomainState::from_projects(vec![config_project("p")]),
    );
    let service = ProgressService::with_target(
        handle,
        Some(StateFileTarget {
            path: path.clone(),
            create_parent_dir: true,
        }),
    );
    assert!(!folder.exists(), "建立服務時不建資料夾");

    service
        .apply_progress(
            &ProjectId::new("p"),
            &TaskId::new("t1"),
            ProgressOp::Advance,
        )
        .await
        .expect("第一次寫入應建立資料夾並成功");

    assert_eq!(read_json(&path)["version"], 3);
}
