//! repo-projects task 4.4 驗收測試：Repo Project 管理端點（`POST /api/repo-projects`、`PATCH`／`DELETE
//! /api/repo-projects/{pid}`）、免帶 id 推進（`POST /api/agent/advance`）、覆蓋端點對固定 pane 的 409
//! `not_overridable`（design D6、D7；spec `repo-projects`「加入 Repo Project」「修改 Repo Project 名稱與 stages」
//! 「移除 Repo Project」「Repo Project 的輸入驗證」「Repo Project 管理端點的來源檢查與錯誤本體」、`agent-reporting`
//! 「免帶 id 推進」、`pipeline-progress`「綁定覆蓋端點」「寫入端點只接受本機同源請求」）。
//!
//! 同 `cockpit/tests/agent_api.rs`：`tower::ServiceExt::oneshot` 打真 `router`，後面接真寫入服務與真投影，
//! `AppState.port` 停在 0、請求固定帶 `Host: 127.0.0.1:0`。pane 的 repo 歸類（resolver 的產出）直接寫進
//! `DomainState::pane_repos`；「經由 WSL 的 runtime」以 `path_mappings` 登記 `PathMapping::Wsl` 模擬。

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::AtomicU16;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use axum::body::Body;
use axum::http::{Request, StatusCode};
use cockpit::files::PathMapping;
use cockpit::http::{self, AppState};
use cockpit::progress_service::ProgressService;
use cockpit_core::{
    AgentStatus, BindingSpec, ConnectionState, DomainState, Focused, Pane, PaneId, PaneRepo,
    PaneRepos, ProjectDef, ProjectId, RepoKey, RuntimeId, RuntimeSnapshot, RuntimeStore,
    StoreHandle, Tab, TabId, TaskDef, TaskId, Workspace, WorkspaceId, WorkstreamDef, WorkstreamId,
    spawn_projector,
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

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
            "cockpit-repo-project-api-{tag}-{}-{nanos}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("建立測試暫存目錄");
        Self { path }
    }

    fn state_path(&self) -> PathBuf {
        self.path.join("state.json")
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

const WAIT_TIMEOUT: Duration = Duration::from_secs(5);
const APP_REPO: &str = r"d:\work\app\.git";
const LIB_REPO: &str = r"d:\work\lib\.git";
const WSL_REPO: &str = "/home/dev/wslrepo/.git";

fn s(v: &str) -> String {
    v.to_string()
}

fn pane_entry(workspace: &str, pane_id: &str) -> (Workspace, Tab, Pane) {
    (
        Workspace {
            id: WorkspaceId::new(workspace),
            label: Some(workspace.to_string()),
            number: 1,
            agent_status: AgentStatus::Idle,
            focused: false,
        },
        Tab {
            id: TabId::new(format!("{workspace}:t1")),
            workspace_id: WorkspaceId::new(workspace),
            number: 1,
            agent_status: AgentStatus::Idle,
            focused: false,
        },
        Pane {
            id: PaneId::new(pane_id),
            workspace_id: WorkspaceId::new(workspace),
            tab_id: TabId::new(format!("{workspace}:t1")),
            agent: None,
            agent_status: AgentStatus::Idle,
            title: None,
            cwd: None,
            label: None,
            focused: false,
            exited: false,
            updated_at: SystemTime::UNIX_EPOCH,
        },
    )
}

/// 在 `store` 登記並連線 runtime `id`，帶入 `entries`（`(workspace, pane_id)`）。
fn add_runtime(store: &mut RuntimeStore, id: &str, entries: &[(&str, &str)]) {
    let id = RuntimeId::new(id);
    store.register(id.clone(), "herdr".to_string(), "test".to_string());
    let (mut workspaces, mut tabs, mut panes) = (Vec::new(), Vec::new(), Vec::new());
    for (workspace, pane_id) in entries {
        let (w, t, p) = pane_entry(workspace, pane_id);
        workspaces.push(w);
        tabs.push(t);
        panes.push(p);
    }
    store
        .replace(
            &id,
            RuntimeSnapshot {
                server_version: "test".to_string(),
                protocol: 1,
                workspaces,
                tabs,
                panes,
                agents: Vec::new(),
                focused: Focused {
                    workspace_id: None,
                    tab_id: None,
                    pane_id: None,
                },
                protocol_warning: None,
            },
        )
        .expect("runtime 剛登記，replace 不應該失敗");
    store
        .set_connection(
            &id,
            ConnectionState::Connected {
                since: SystemTime::UNIX_EPOCH,
                server_version: "test".to_string(),
                protocol: 1,
                last_snapshot_at: SystemTime::UNIX_EPOCH,
                settled: true,
                protocol_warning: None,
            },
        )
        .expect("runtime 剛登記，set_connection 不應該失敗");
}

/// runtime `local`：`wJ:p1`、`wK:p2`、`wL:p3`（都在 `app` repo）與 `wM:p4`（`lib` repo）；runtime `wsl`：`w1:p1`
/// （`wslrepo`，經由 WSL 連線）。
fn store() -> RuntimeStore {
    let mut store = RuntimeStore::new();
    add_runtime(
        &mut store,
        "local",
        &[
            ("wJ", "wJ:p1"),
            ("wK", "wK:p2"),
            ("wL", "wL:p3"),
            ("wM", "wM:p4"),
        ],
    );
    add_runtime(&mut store, "wsl", &[("w1", "w1:p1")]);
    store
}

fn pane_repos() -> PaneRepos {
    let entry = |runtime: &str, pane: &str, repo: &str, name: &str| {
        (
            (RuntimeId::new(runtime), PaneId::new(pane)),
            PaneRepo {
                repo: RepoKey::new(repo),
                default_name: s(name),
                worktree: None,
            },
        )
    };
    [
        entry("local", "wJ:p1", APP_REPO, "app"),
        entry("local", "wK:p2", APP_REPO, "app"),
        entry("local", "wL:p3", APP_REPO, "app"),
        entry("local", "wM:p4", LIB_REPO, "lib"),
        entry("wsl", "w1:p1", WSL_REPO, "wslrepo"),
    ]
    .into_iter()
    .collect()
}

fn workstream(id: &str, workspace: &str) -> WorkstreamDef {
    WorkstreamDef {
        id: WorkstreamId::new(id),
        name: id.to_string(),
        binding: Some(BindingSpec {
            runtime: RuntimeId::new("local"),
            workspace: workspace.to_string(),
            pane_label: None,
            cwd: None,
            agent: None,
        }),
        pinned_pane: None,
    }
}

fn task(id: &str, workstream: &str, stage: &str) -> TaskDef {
    TaskDef {
        id: TaskId::new(id),
        title: format!("title-{id}"),
        workstream: WorkstreamId::new(workstream),
        stage: stage.to_string(),
        depends_on: Vec::new(),
    }
}

/// 手寫 project `p`（stages `Plan`→`Build`）：`be` 綁 workspace `wJ`（pane `wJ:p1`），task `t1`、`t2` 都在 `Plan`。
fn hand_project() -> ProjectDef {
    ProjectDef {
        id: ProjectId::new("p"),
        name: s("p"),
        stages: vec![s("Plan"), s("Build")],
        workstreams: vec![workstream("be", "wJ")],
        tasks: vec![task("t1", "be", "Plan"), task("t2", "be", "Plan")],
        repo: None,
    }
}

/// 手寫 project `solo`：`so` 綁 workspace `wM`（pane `wM:p4`），只有一張 task `s1`，起始 stage 由呼叫端決定。
fn solo_project(start: &str) -> ProjectDef {
    ProjectDef {
        id: ProjectId::new("solo"),
        name: s("solo"),
        stages: vec![s("Plan"), s("Build")],
        workstreams: vec![workstream("so", "wM")],
        tasks: vec![task("s1", "so", start)],
        repo: None,
    }
}

struct Env {
    handle: StoreHandle,
    router: axum::Router,
}

fn build(projects: Vec<ProjectDef>, state_path: PathBuf) -> Env {
    let store = store();
    let runtime_ids = store.runtime_ids();
    let mut domain = DomainState::from_projects(projects);
    domain.pane_repos = pane_repos();
    domain.refresh_projects();
    let handle = StoreHandle::new_with_domain(store, domain);
    let service = ProgressService::new(handle.clone(), state_path);
    let _projector = spawn_projector(handle.clone());
    let path_mappings: HashMap<RuntimeId, PathMapping> = runtime_ids
        .into_iter()
        .map(|id| {
            let mapping = if id.as_str() == "wsl" {
                PathMapping::Wsl {
                    distro: s("Ubuntu"),
                }
            } else {
                PathMapping::Native
            };
            (id, mapping)
        })
        .collect();
    let state = AppState {
        state: handle.subscribe(),
        progress: Some(service),
        port: Arc::new(AtomicU16::new(0)),
        runtimes: Arc::new(HashMap::new()),
        path_mappings: Arc::new(path_mappings),
        files: Arc::new(cockpit::files::FileSettings::embedded()),
        git_runner: Arc::new(cockpit_git::GitRunner::new()),
        activity: cockpit::http::ClientActivity::new(),
    };
    Env {
        handle,
        router: http::router(state),
    }
}

/// 沒有任何手寫 project 的環境。
fn build_empty(dir: &TempDir) -> Env {
    build(Vec::new(), dir.state_path())
}

async fn send_with(
    router: &axum::Router,
    method: &str,
    uri: &str,
    headers: &[(&str, &str)],
    body: Option<&str>,
) -> (StatusCode, Value) {
    let mut builder = Request::builder().method(method).uri(uri);
    if !headers.iter().any(|(name, _)| *name == "host") {
        builder = builder.header("host", "127.0.0.1:0");
    }
    for (name, value) in headers {
        builder = builder.header(*name, *value);
    }
    let body = body.map_or_else(Body::empty, |text| Body::from(text.to_string()));
    let response = router
        .clone()
        .oneshot(builder.body(body).expect("request 建構不應該失敗"))
        .await
        .expect("oneshot 呼叫不應該失敗");
    let status = response.status();
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("body 收集不應該失敗")
        .to_bytes();
    let value = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or(Value::Null)
    };
    (status, value)
}

async fn send(
    router: &axum::Router,
    method: &str,
    uri: &str,
    body: Option<&str>,
) -> (StatusCode, Value) {
    send_with(router, method, uri, &[], body).await
}

/// 帶 `X-Herdr-Pane-Id` 的 agent 請求。
async fn agent(router: &axum::Router, method: &str, uri: &str, pane: &str) -> (StatusCode, Value) {
    send_with(router, method, uri, &[("x-herdr-pane-id", pane)], None).await
}

async fn advance(router: &axum::Router, pane: &str) -> (StatusCode, Value) {
    agent(router, "POST", "/api/agent/advance", pane).await
}

async fn state_json(router: &axum::Router) -> Value {
    send(router, "GET", "/api/state", None).await.1
}

/// 輪詢 `/api/state` 直到 `check` 成立，回傳那份投影；逾時就讓斷言失敗。
async fn wait_state(router: &axum::Router, what: &str, check: impl Fn(&Value) -> bool) -> Value {
    let deadline = Instant::now() + WAIT_TIMEOUT;
    loop {
        let state = state_json(router).await;
        if check(&state) {
            return state;
        }
        assert!(Instant::now() < deadline, "等待投影逾時：{what}");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

fn project<'a>(state: &'a Value, id: &str) -> Option<&'a Value> {
    state["projects"]
        .as_array()
        .and_then(|ps| ps.iter().find(|p| p["id"] == id))
}

fn find_task<'a>(state: &'a Value, project_id: &str, id: &str) -> &'a Value {
    project(state, project_id)
        .and_then(|p| p["tasks"].as_array())
        .and_then(|ts| ts.iter().find(|t| t["id"] == id))
        .unwrap_or_else(|| panic!("投影應該有 {project_id} 的 task {id}"))
}

fn find_workstream<'a>(state: &'a Value, project_id: &str, id: &str) -> &'a Value {
    project(state, project_id)
        .and_then(|p| p["workstreams"].as_array())
        .and_then(|ws| ws.iter().find(|w| w["id"] == id))
        .unwrap_or_else(|| panic!("投影應該有 {project_id} 的 workstream {id}"))
}

fn stage_of(state: &Value, project_id: &str, task: &str) -> String {
    find_task(state, project_id, task)["stage"]
        .as_str()
        .expect("stage 是字串")
        .to_string()
}

fn read_state_file(path: &Path) -> Value {
    serde_json::from_str(&fs::read_to_string(path).expect("讀狀態檔")).expect("狀態檔是 JSON")
}

/// 錯誤本體：`error` 非空、`code` 為預期。
fn assert_error(status: StatusCode, body: &Value, want_status: StatusCode, want_code: &str) {
    assert_eq!(status, want_status, "本體：{body:?}");
    assert_eq!(body["code"], want_code, "本體：{body:?}");
    assert!(
        body["error"].as_str().is_some_and(|e| !e.is_empty()),
        "error 欄位應非空：{body:?}"
    );
}

/// 加入 `app`（stages `Plan`、`Build`）並等到投影出現；回傳 id。
async fn add_app(env: &Env) -> String {
    let (status, body) = send(
        &env.router,
        "POST",
        "/api/repo-projects",
        Some(&json!({"repo": APP_REPO, "stages": ["Plan", "Build"]}).to_string()),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "本體：{body:?}");
    let id = body["id"].as_str().expect("回傳 id").to_string();
    wait_state(&env.router, "app 出現在投影", |st| {
        project(st, &id).is_some()
    })
    .await;
    id
}

// ---------------------------------------------------------------------------
// POST /api/repo-projects
// ---------------------------------------------------------------------------

/// spec「加入成功」。
#[tokio::test]
async fn add_returns_201_and_project_appears_with_tasks_per_pane() {
    let dir = TempDir::new("add");
    let env = build_empty(&dir);

    let (status, body) = send(
        &env.router,
        "POST",
        "/api/repo-projects",
        Some(&json!({"repo": APP_REPO, "stages": ["Plan", "Build", "Review", "Done"]}).to_string()),
    )
    .await;

    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(body, json!({"id": "app"}));
    let state = wait_state(&env.router, "app 出現", |st| project(st, "app").is_some()).await;
    let app = project(&state, "app").expect("app");
    assert_eq!(app["kind"], "repo");
    assert_eq!(app["name"], "app");
    assert_eq!(app["stages"], json!(["Plan", "Build", "Review", "Done"]));
    assert_eq!(app["workstreams"].as_array().expect("workstreams").len(), 3);
    for pane in ["wJ:p1", "wK:p2", "wL:p3"] {
        assert_eq!(stage_of(&state, "app", &format!("local~{pane}")), "Plan");
    }
    let detected: Vec<&Value> = state["detected_repos"]
        .as_array()
        .expect("detected_repos")
        .iter()
        .map(|r| &r["repo"])
        .collect();
    assert!(
        !detected.contains(&&json!(APP_REPO)),
        "已加入的 repo 不在偵測區：{detected:?}"
    );
    assert!(detected.contains(&&json!(LIB_REPO)), "lib 仍在偵測區");
    assert_eq!(
        read_state_file(&dir.state_path())["repo_projects"]["app"]["repo"],
        APP_REPO
    );
}

/// spec「指定名稱」「前後空白被去除」：名稱與 stage 名稱去除前後空白，id 依名稱產生。
#[tokio::test]
async fn add_trims_name_and_stages_and_derives_id_from_name() {
    let dir = TempDir::new("add-trim");
    let env = build_empty(&dir);

    let (status, body) = send(
        &env.router,
        "POST",
        "/api/repo-projects",
        Some(
            &json!({"repo": APP_REPO, "stages": [" Plan ", "Build"], "name": "  My App (v2)  "})
                .to_string(),
        ),
    )
    .await;

    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(body, json!({"id": "My-App-v2"}));
    let state = wait_state(&env.router, "出現", |st| {
        project(st, "My-App-v2").is_some()
    })
    .await;
    let added = project(&state, "My-App-v2").expect("project");
    assert_eq!(added["name"], "My App (v2)");
    assert_eq!(added["stages"], json!(["Plan", "Build"]));
}

/// spec「不是偵測到的 repo」。
#[tokio::test]
async fn add_unknown_repo_is_404_repo_not_detected() {
    let dir = TempDir::new("add-unknown");
    let env = build_empty(&dir);

    let (status, body) = send(
        &env.router,
        "POST",
        "/api/repo-projects",
        Some(&json!({"repo": r"C:\anywhere\.git", "stages": ["Plan"]}).to_string()),
    )
    .await;

    assert_error(status, &body, StatusCode::NOT_FOUND, "repo_not_detected");
    assert!(
        !body.to_string().contains("anywhere"),
        "錯誤本體不得回顯 repo key（主機路徑）：{body:?}"
    );
    assert!(env.handle.with_domain(|d| d.repo_projects.is_empty()));
}

/// spec「重複加入」。
#[tokio::test]
async fn add_duplicate_repo_is_409_repo_already_added() {
    let dir = TempDir::new("add-dup");
    let env = build_empty(&dir);
    add_app(&env).await;

    let (status, body) = send(
        &env.router,
        "POST",
        "/api/repo-projects",
        Some(&json!({"repo": APP_REPO, "stages": ["Plan"]}).to_string()),
    )
    .await;

    assert_error(status, &body, StatusCode::CONFLICT, "repo_already_added");
    assert_eq!(env.handle.with_domain(|d| d.repo_projects.len()), 1);
}

/// spec「加入時寫檔失敗」：狀態檔路徑是既有目錄，rename 取代目錄會失敗。
#[tokio::test]
async fn add_persist_failure_is_500_and_state_unchanged() {
    let dir = TempDir::new("add-persist");
    let path = dir.state_path();
    fs::create_dir_all(&path).expect("在狀態檔路徑建立目錄");
    let env = build(Vec::new(), path);

    let (status, body) = send(
        &env.router,
        "POST",
        "/api/repo-projects",
        Some(&json!({"repo": APP_REPO, "stages": ["Plan"]}).to_string()),
    )
    .await;

    assert_error(
        status,
        &body,
        StatusCode::INTERNAL_SERVER_ERROR,
        "persist_failed",
    );
    assert!(env.handle.with_domain(|d| d.repo_projects.is_empty()));
    let state = state_json(&env.router).await;
    assert!(project(&state, "app").is_none());
    assert!(
        state["detected_repos"]
            .as_array()
            .expect("detected_repos")
            .iter()
            .any(|r| r["repo"] == APP_REPO),
        "detected_repos 仍含該 repo"
    );
}

/// spec「名稱太長」「stage 數量與重複」「stage 名稱含控制字元」：名稱與 stages 的驗證，狀態不變。
#[tokio::test]
async fn add_validation_errors_are_400_with_specific_codes() {
    let dir = TempDir::new("add-validation");
    let env = build_empty(&dir);
    let too_many: Vec<String> = (0..13).map(|i| format!("S{i}")).collect();
    let cases = [
        (
            "名稱 65 字元",
            json!({"repo": APP_REPO, "stages": ["Plan"], "name": "x".repeat(65)}),
            "invalid_name",
        ),
        (
            "名稱只有空白",
            json!({"repo": APP_REPO, "stages": ["Plan"], "name": "   "}),
            "invalid_name",
        ),
        (
            "名稱含控制字元",
            json!({"repo": APP_REPO, "stages": ["Plan"], "name": "a\u{7}b"}),
            "invalid_name",
        ),
        (
            "名稱含雙向覆寫字元",
            json!({"repo": APP_REPO, "stages": ["Plan"], "name": "a\u{202E}b"}),
            "invalid_name",
        ),
        (
            "名稱含零寬字元",
            json!({"repo": APP_REPO, "stages": ["Plan"], "name": "a\u{200B}b"}),
            "invalid_name",
        ),
        (
            "stages 為空",
            json!({"repo": APP_REPO, "stages": []}),
            "invalid_stages",
        ),
        (
            "stages 13 個",
            json!({"repo": APP_REPO, "stages": too_many}),
            "invalid_stages",
        ),
        (
            "stages 重複",
            json!({"repo": APP_REPO, "stages": ["Plan", "Plan"]}),
            "invalid_stages",
        ),
        (
            "stage 含控制字元",
            json!({"repo": APP_REPO, "stages": ["Plan\u{7}"]}),
            "invalid_stages",
        ),
        (
            "stage 含 BOM",
            json!({"repo": APP_REPO, "stages": ["\u{FEFF}Plan"]}),
            "invalid_stages",
        ),
        (
            "stage 33 字元",
            json!({"repo": APP_REPO, "stages": ["y".repeat(33)]}),
            "invalid_stages",
        ),
    ];

    for (what, body, code) in cases {
        let (status, response) = send(
            &env.router,
            "POST",
            "/api/repo-projects",
            Some(&body.to_string()),
        )
        .await;
        assert_error(status, &response, StatusCode::BAD_REQUEST, code);
        assert!(
            env.handle.with_domain(|d| d.repo_projects.is_empty()),
            "{what}：不得新增 Project"
        );
    }
}

/// spec「本體不合法」。
#[tokio::test]
async fn add_invalid_body_is_400_invalid_body() {
    let dir = TempDir::new("add-body");
    let env = build_empty(&dir);
    let bodies = [
        "{not json".to_string(),
        r#"{"repo":"x","stages":["A"],"extra":1}"#.to_string(),
        r#"{"repo":1,"stages":["A"]}"#.to_string(),
        r#"{"repo":"x"}"#.to_string(),
        r#"{"stages":["A"]}"#.to_string(),
        r#"{"repo":"x","stages":"A"}"#.to_string(),
        r#"[]"#.to_string(),
        String::new(),
    ];

    for body in bodies {
        let (status, response) = send(&env.router, "POST", "/api/repo-projects", Some(&body)).await;
        assert_error(status, &response, StatusCode::BAD_REQUEST, "invalid_body");
    }
    assert!(env.handle.with_domain(|d| d.repo_projects.is_empty()));
}

// ---------------------------------------------------------------------------
// PATCH /api/repo-projects/{pid}
// ---------------------------------------------------------------------------

async fn patch(env: &Env, pid: &str, body: &str) -> (StatusCode, Value) {
    send(
        &env.router,
        "PATCH",
        &format!("/api/repo-projects/{pid}"),
        Some(body),
    )
    .await
}

/// 真機冒煙發現 1（repo-projects task 7.3）：Windows PowerShell 5.1 在 UTF-8 主控台把 here-string 經標準輸入交給
/// `curl.exe` 時，本體開頭多一個 UTF-8 BOM（`EF BB BF`）。POST 與 PATCH 都容忍開頭的一個 BOM；只剝一個，
/// 兩個 BOM 仍是 400 `invalid_body`。
#[tokio::test]
async fn management_endpoints_accept_one_leading_utf8_bom() {
    let dir = TempDir::new("bom");
    let env = build_empty(&dir);
    let with_bom = |value: Value| format!("\u{FEFF}{value}");
    let post = with_bom(json!({"repo": APP_REPO, "stages": ["Plan", "Build"]}));
    assert_eq!(
        &post.as_bytes()[..3],
        [0xEF, 0xBB, 0xBF],
        "前提：本體以 BOM 開頭"
    );

    let (status, body) = send(&env.router, "POST", "/api/repo-projects", Some(&post)).await;
    assert_eq!(status, StatusCode::CREATED, "本體：{body:?}");
    assert_eq!(body, json!({"id": "app"}));
    wait_state(&env.router, "app 出現", |st| project(st, "app").is_some()).await;

    let (status, body) = patch(&env, "app", &with_bom(json!({"name": "App 前端"}))).await;
    assert_eq!(status, StatusCode::NO_CONTENT, "本體：{body:?}");
    wait_state(&env.router, "改名", |st| {
        project(st, "app").is_some_and(|p| p["name"] == "App 前端")
    })
    .await;

    let double = |value: Value| format!("\u{FEFF}\u{FEFF}{value}");
    let (status, body) = patch(&env, "app", &double(json!({"name": "X"}))).await;
    assert_error(status, &body, StatusCode::BAD_REQUEST, "invalid_body");
    let (status, body) = send(
        &env.router,
        "POST",
        "/api/repo-projects",
        Some(&double(json!({"repo": LIB_REPO, "stages": ["Plan"]}))),
    )
    .await;
    assert_error(status, &body, StatusCode::BAD_REQUEST, "invalid_body");
    let names: Vec<String> = env
        .handle
        .with_domain(|d| d.repo_projects.iter().map(|p| p.name.clone()).collect());
    assert_eq!(names, vec![s("App 前端")], "兩個 BOM 的請求不改狀態");
}

/// spec「只改名稱」。
#[tokio::test]
async fn patch_name_only_is_204_and_id_stays() {
    let dir = TempDir::new("patch-name");
    let env = build_empty(&dir);
    add_app(&env).await;

    let (status, body) = patch(&env, "app", r#"{"name":"App 前端"}"#).await;

    assert_eq!(status, StatusCode::NO_CONTENT, "本體：{body:?}");
    let state = wait_state(&env.router, "改名", |st| {
        project(st, "app").is_some_and(|p| p["name"] == "App 前端")
    })
    .await;
    assert_eq!(
        project(&state, "app").expect("app")["stages"],
        json!(["Plan", "Build"])
    );
}

/// spec「stage 改名、新增、刪除、排序」（經 HTTP）：`b` 標記保留、被刪除 stage 的 task 落到第一個 stage。
#[tokio::test]
async fn patch_stage_edits_remap_tasks_and_keep_marks() {
    let dir = TempDir::new("patch-stages");
    let env = build_empty(&dir);
    let (status, _) = send(
        &env.router,
        "POST",
        "/api/repo-projects",
        Some(
            &json!({"repo": APP_REPO, "stages": ["Plan", "Implement", "Review", "Done"]})
                .to_string(),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    wait_state(&env.router, "出現", |st| project(st, "app").is_some()).await;
    // a（wJ:p1）留在 Plan；b（wK:p2）推進到 Implement 並標 failed；c（wL:p3）推進到 Review。
    let step = |pane: &str, op: &'static str| {
        let uri = format!("/api/projects/app/tasks/local~{pane}/{op}");
        let router = env.router.clone();
        async move {
            let (status, body) = send(&router, "POST", &uri, None).await;
            assert_eq!(status, StatusCode::NO_CONTENT, "{uri}：{body:?}");
        }
    };
    step("wK:p2", "advance").await;
    step("wK:p2", "fail").await;
    step("wL:p3", "advance").await;
    step("wL:p3", "advance").await;

    let (status, body) = patch(
        &env,
        "app",
        r#"{"stages":[{"name":"Plan","from":"Plan"},{"name":"Design","from":null},{"name":"Build","from":"Implement"},{"name":"Done","from":"Done"}]}"#,
    )
    .await;

    assert_eq!(status, StatusCode::NO_CONTENT, "本體：{body:?}");
    let state = wait_state(&env.router, "stages 更新", |st| {
        project(st, "app")
            .is_some_and(|p| p["stages"] == json!(["Plan", "Design", "Build", "Done"]))
    })
    .await;
    assert_eq!(stage_of(&state, "app", "local~wJ:p1"), "Plan");
    assert_eq!(stage_of(&state, "app", "local~wK:p2"), "Build");
    assert_eq!(find_task(&state, "app", "local~wK:p2")["mark"], "failed");
    assert_eq!(
        stage_of(&state, "app", "local~wL:p3"),
        "Plan",
        "Review 被刪除，改用第一個 stage"
    );
}

/// spec「from 省略與 null 都表示新增」。
#[tokio::test]
async fn patch_from_omitted_and_null_both_mean_new_stage() {
    let dir = TempDir::new("patch-from-null");
    let env = build_empty(&dir);
    add_app(&env).await;

    let (status, body) = patch(
        &env,
        "app",
        r#"{"stages":[{"name":"Plan","from":"Plan"},{"name":"B"},{"name":"C","from":null}]}"#,
    )
    .await;

    assert_eq!(status, StatusCode::NO_CONTENT, "本體：{body:?}");
    wait_state(&env.router, "stages 更新", |st| {
        project(st, "app").is_some_and(|p| p["stages"] == json!(["Plan", "B", "C"]))
    })
    .await;
}

/// spec「同一個舊名稱被引用兩次」「from 不是現有 stage」「兩者同時給但 stages 不合法」：400 `invalid_stages`，
/// 名稱與 stages 都不變。
#[tokio::test]
async fn patch_invalid_stages_is_400_and_changes_nothing() {
    let dir = TempDir::new("patch-bad-stages");
    let env = build_empty(&dir);
    add_app(&env).await;
    let bodies = [
        r#"{"stages":[{"name":"X","from":"Plan"},{"name":"Y","from":"Plan"}]}"#,
        r#"{"stages":[{"name":"X","from":"Nope"}]}"#,
        r#"{"name":"改了","stages":[]}"#,
        r#"{"name":"改了","stages":[{"name":"X","from":"Plan"},{"name":"X"}]}"#,
    ];

    for body in bodies {
        let (status, response) = patch(&env, "app", body).await;
        assert_error(status, &response, StatusCode::BAD_REQUEST, "invalid_stages");
    }

    let (name, stages) = env.handle.with_domain(|d| {
        let def = &d.repo_projects[0];
        (def.name.clone(), def.stages.clone())
    });
    assert_eq!(name, "app");
    assert_eq!(stages, vec![s("Plan"), s("Build")]);
}

/// spec「空白名稱」。
#[tokio::test]
async fn patch_blank_name_is_400_invalid_name() {
    let dir = TempDir::new("patch-blank");
    let env = build_empty(&dir);
    add_app(&env).await;

    let (status, body) = patch(&env, "app", r#"{"name":"   "}"#).await;

    assert_error(status, &body, StatusCode::BAD_REQUEST, "invalid_name");
    assert_eq!(
        env.handle.with_domain(|d| d.repo_projects[0].name.clone()),
        "app"
    );
}

/// spec「空本體」與本體形狀錯誤。
#[tokio::test]
async fn patch_invalid_body_is_400_invalid_body() {
    let dir = TempDir::new("patch-body");
    let env = build_empty(&dir);
    add_app(&env).await;
    let bodies = [
        "{}",
        r#"{"name":null}"#,
        r#"{"name":1}"#,
        r#"{"stages":"x"}"#,
        r#"{"stages":[{"name":"X","extra":1}]}"#,
        r#"{"stages":[{"from":"Plan"}]}"#,
        r#"{"name":"x","extra":1}"#,
        "{not json",
        "",
    ];

    for body in bodies {
        let (status, response) = patch(&env, "app", body).await;
        assert_error(status, &response, StatusCode::BAD_REQUEST, "invalid_body");
    }
    assert_eq!(
        env.handle.with_domain(|d| d.repo_projects[0].name.clone()),
        "app"
    );
}

/// spec「pid 不存在或是手寫 project」。
#[tokio::test]
async fn patch_ghost_is_404_and_hand_written_is_409() {
    let dir = TempDir::new("patch-pid");
    let env = build(vec![hand_project()], dir.state_path());

    let (status, body) = patch(&env, "ghost", r#"{"name":"x"}"#).await;
    assert_error(status, &body, StatusCode::NOT_FOUND, "unknown_project");
    assert_eq!(body["params"]["id"], "ghost");

    let (status, body) = patch(&env, "p", r#"{"name":"x"}"#).await;
    assert_error(status, &body, StatusCode::CONFLICT, "not_repo_project");
    assert_eq!(body["params"]["id"], "p");
    assert_eq!(
        env.handle.with_domain(|d| d
            .projects
            .iter()
            .find(|p| p.id.as_str() == "p")
            .map(|p| p.name.clone())),
        Some(s("p"))
    );
}

// ---------------------------------------------------------------------------
// DELETE /api/repo-projects/{pid}
// ---------------------------------------------------------------------------

/// spec「移除成功」。
#[tokio::test]
async fn delete_removes_project_and_repo_returns_to_detected() {
    let dir = TempDir::new("delete");
    let env = build(vec![hand_project()], dir.state_path());
    add_app(&env).await;

    let (status, body) = send(&env.router, "DELETE", "/api/repo-projects/app", None).await;

    assert_eq!(status, StatusCode::NO_CONTENT, "本體：{body:?}");
    let state = wait_state(&env.router, "app 消失", |st| project(st, "app").is_none()).await;
    assert!(project(&state, "p").is_some(), "其他 Project 不變");
    assert!(
        state["detected_repos"]
            .as_array()
            .expect("detected_repos")
            .iter()
            .any(|r| r["repo"] == APP_REPO),
        "detected_repos 重新含該 repo"
    );
    assert!(
        read_state_file(&dir.state_path())["repo_projects"]
            .get("app")
            .is_none()
    );
}

/// spec「移除不存在或手寫的 project」。
#[tokio::test]
async fn delete_ghost_is_404_and_hand_written_is_409() {
    let dir = TempDir::new("delete-pid");
    let env = build(vec![hand_project()], dir.state_path());

    let (status, body) = send(&env.router, "DELETE", "/api/repo-projects/ghost", None).await;
    assert_error(status, &body, StatusCode::NOT_FOUND, "unknown_project");

    let (status, body) = send(&env.router, "DELETE", "/api/repo-projects/p", None).await;
    assert_error(status, &body, StatusCode::CONFLICT, "not_repo_project");
    assert!(
        env.handle
            .with_domain(|d| d.projects.iter().any(|p| p.id.as_str() == "p")),
        "手寫 project 不得被移除"
    );
}

// ---------------------------------------------------------------------------
// 來源檢查與方法不允許（spec「Repo Project 管理端點的來源檢查與錯誤本體」「寫入端點只接受本機同源請求」）
// ---------------------------------------------------------------------------

/// spec「跨站請求被拒」「DNS rebinding 被拒」「PATCH 也檢查來源」：三個端點都擋，狀態不變。
#[tokio::test]
async fn management_endpoints_reject_foreign_origin_and_host() {
    let dir = TempDir::new("source-check");
    let env = build_empty(&dir);
    add_app(&env).await;
    let add_body = json!({"repo": LIB_REPO, "stages": ["Plan"]}).to_string();
    let cases: [(&str, &str, Option<&str>); 3] = [
        ("POST", "/api/repo-projects", Some(add_body.as_str())),
        ("PATCH", "/api/repo-projects/app", Some(r#"{"name":"x"}"#)),
        ("DELETE", "/api/repo-projects/app", None),
    ];

    for (method, uri, body) in cases {
        let foreign_origin = [("origin", "https://evil.example")];
        let foreign_host = [("host", "evil.example:0")];
        for headers in [&foreign_origin[..], &foreign_host[..]] {
            let (status, response) = send_with(&env.router, method, uri, headers, body).await;
            assert_error(status, &response, StatusCode::FORBIDDEN, "forbidden_source");
        }
    }

    let (count, name) = env
        .handle
        .with_domain(|d| (d.repo_projects.len(), d.repo_projects[0].name.clone()));
    assert_eq!(
        (count, name.as_str()),
        (1, "app"),
        "被擋下的請求不得改任何狀態"
    );

    // 對照：同源（Host 合格、Origin 與 Host 一致）照常處理。
    let (status, _) = send_with(
        &env.router,
        "PATCH",
        "/api/repo-projects/app",
        &[("origin", "http://127.0.0.1:0")],
        Some(r#"{"name":"ok"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
}

/// 方法不允許：405，本體帶 `code: method_not_allowed`，且不經來源檢查、不改狀態。
#[tokio::test]
async fn management_endpoints_reject_other_methods_with_405() {
    let dir = TempDir::new("methods");
    let env = build_empty(&dir);
    add_app(&env).await;
    let cases = [
        ("GET", "/api/repo-projects"),
        ("PUT", "/api/repo-projects"),
        ("PATCH", "/api/repo-projects"),
        ("DELETE", "/api/repo-projects"),
        ("GET", "/api/repo-projects/app"),
        ("POST", "/api/repo-projects/app"),
        ("PUT", "/api/repo-projects/app"),
    ];

    for (method, uri) in cases {
        let (status, body) = send(&env.router, method, uri, Some("{}")).await;
        assert_error(
            status,
            &body,
            StatusCode::METHOD_NOT_ALLOWED,
            "method_not_allowed",
        );
    }
    assert_eq!(env.handle.with_domain(|d| d.repo_projects.len()), 1);
}

// ---------------------------------------------------------------------------
// 覆蓋端點對固定 pane 的工作線（spec `pipeline-progress`「綁定覆蓋端點」）
// ---------------------------------------------------------------------------

/// spec「固定 pane 的工作線不能改綁」「不能取消改綁」：`PUT` 與 `DELETE` 都是 409 `not_overridable`，
/// 該工作線仍為固定 pane 的 `bound`。
#[tokio::test]
async fn pinned_workstream_override_put_and_delete_are_409_not_overridable() {
    let dir = TempDir::new("override-pinned");
    let env = build_empty(&dir);
    add_app(&env).await;
    let uri = "/api/projects/app/workstreams/local~wJ:p1/override";

    let (status, body) = send(
        &env.router,
        "PUT",
        uri,
        Some(r#"{"runtime":"local","pane_id":"wK:p2"}"#),
    )
    .await;
    assert_error(status, &body, StatusCode::CONFLICT, "not_overridable");

    let (status, body) = send(&env.router, "DELETE", uri, None).await;
    assert_error(status, &body, StatusCode::CONFLICT, "not_overridable");

    let state = state_json(&env.router).await;
    let binding = &find_workstream(&state, "app", "local~wJ:p1")["binding"];
    assert_eq!(binding["state"], "bound");
    assert_eq!(binding["pane_id"], "wJ:p1");
    assert_eq!(binding["source"], "pane");
    assert!(
        env.handle.with_domain(|d| d.overrides.is_empty()),
        "不得留下覆蓋"
    );
}

/// 固定 pane 的 409 在鎖內判定：不存在的 workstream 仍是 404 `unknown_workstream`，不存在的 project 仍是 404。
#[tokio::test]
async fn pinned_override_check_keeps_404_for_unknown_ids() {
    let dir = TempDir::new("override-404");
    let env = build_empty(&dir);
    add_app(&env).await;

    let (status, body) = send(
        &env.router,
        "PUT",
        "/api/projects/app/workstreams/nope/override",
        Some(r#"{"runtime":"local","pane_id":"wK:p2"}"#),
    )
    .await;
    assert_error(status, &body, StatusCode::NOT_FOUND, "unknown_workstream");

    let (status, body) = send(
        &env.router,
        "DELETE",
        "/api/projects/ghost/workstreams/x/override",
        None,
    )
    .await;
    assert_error(status, &body, StatusCode::NOT_FOUND, "unknown_project");
}

/// 對照：手寫 project 的 workstream 照舊可以改綁與取消。
#[tokio::test]
async fn hand_written_workstream_override_still_works() {
    let dir = TempDir::new("override-hand");
    let env = build(vec![hand_project()], dir.state_path());

    let (status, body) = send(
        &env.router,
        "PUT",
        "/api/projects/p/workstreams/be/override",
        Some(r#"{"runtime":"local","pane_id":"wK:p2"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "本體：{body:?}");
    let (status, _) = send(
        &env.router,
        "DELETE",
        "/api/projects/p/workstreams/be/override",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
}

// ---------------------------------------------------------------------------
// POST /api/agent/advance（spec `agent-reporting`「免帶 id 推進」）
// ---------------------------------------------------------------------------

/// spec「Repo Project 的 pane 免帶 id 推進」：回 204，task 到下一站，狀態檔保留。
#[tokio::test]
async fn advance_repo_project_pane_moves_to_next_stage_and_persists() {
    let dir = TempDir::new("adv-repo");
    let env = build_empty(&dir);
    add_app(&env).await;

    let (status, body) = advance(&env.router, "wJ:p1").await;

    assert_eq!(status, StatusCode::NO_CONTENT, "本體：{body:?}");
    let state = wait_state(&env.router, "推進", |st| {
        stage_of(st, "app", "local~wJ:p1") == "Build"
    })
    .await;
    assert_eq!(
        stage_of(&state, "app", "local~wK:p2"),
        "Plan",
        "其他 pane 不動"
    );
    assert_eq!(
        read_state_file(&dir.state_path())["repo_projects"]["app"]["tasks"]["local~wJ:p1"],
        json!({"stage": "Build", "mark": "none"})
    );
}

/// spec「手寫 project 已宣告目前 task」：目前 task 為 `t2`，推進的是 `t2`，`t1` 不變。
#[tokio::test]
async fn advance_picks_the_active_task_of_a_hand_written_workstream() {
    let dir = TempDir::new("adv-active");
    let env = build(vec![hand_project()], dir.state_path());
    wait_state(&env.router, "綁定", |st| {
        find_workstream(st, "p", "be")["binding"]["state"] == "bound"
    })
    .await;
    let (status, _) = agent(
        &env.router,
        "POST",
        "/api/agent/projects/p/tasks/t2/start",
        "wJ:p1",
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let (status, body) = advance(&env.router, "wJ:p1").await;

    assert_eq!(status, StatusCode::NO_CONTENT, "本體：{body:?}");
    let state = wait_state(&env.router, "推進", |st| {
        stage_of(st, "p", "t2") == "Build"
    })
    .await;
    assert_eq!(stage_of(&state, "p", "t1"), "Plan");
}

/// 投影落後時的免帶 id 推進（fix round 1，專案 memory `check-against-lagging-projection-misses-fresh-writes`）：
/// `be` 的目前 task 為 `t1`，agent 宣告 `t2` 得 204 後**不等投影**立刻免帶 id 推進，必須推進剛宣告的 `t2`
/// （候選在寫入鎖內依 Domain 選），`t1` 不動，目前 task 仍是 `t2`。
#[tokio::test]
async fn advance_right_after_start_uses_the_fresh_active_task_not_the_lagging_projection() {
    let dir = TempDir::new("adv-fresh-active");
    let env = build(vec![hand_project()], dir.state_path());
    wait_state(&env.router, "綁定", |st| {
        find_workstream(st, "p", "be")["binding"]["state"] == "bound"
    })
    .await;
    let (status, _) = agent(
        &env.router,
        "POST",
        "/api/agent/projects/p/tasks/t1/start",
        "wJ:p1",
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    wait_state(&env.router, "目前 task 為 t1", |st| {
        find_workstream(st, "p", "be")["active_task"] == "t1"
    })
    .await;

    // 宣告 t2 與推進之間不等投影：投影此刻仍可能顯示 t1。
    let (status, _) = agent(
        &env.router,
        "POST",
        "/api/agent/projects/p/tasks/t2/start",
        "wJ:p1",
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, body) = advance(&env.router, "wJ:p1").await;
    assert_eq!(status, StatusCode::NO_CONTENT, "本體：{body:?}");

    let state = wait_state(&env.router, "t2 推進", |st| {
        stage_of(st, "p", "t2") == "Build"
    })
    .await;
    assert_eq!(stage_of(&state, "p", "t1"), "Plan", "t1 不得被推進");
    let active = env.handle.with_domain(|d| {
        d.active_task(&ProjectId::new("p"), &WorkstreamId::new("be"))
            .cloned()
    });
    assert_eq!(active, Some(TaskId::new("t2")), "目前 task 仍是剛宣告的 t2");
}

/// spec「手寫 project 只有一張 task」：沒有目前 task、只有一張 task，推進它並成為目前 task。
#[tokio::test]
async fn advance_picks_the_only_task_and_makes_it_active() {
    let dir = TempDir::new("adv-solo");
    let env = build(vec![solo_project("Plan")], dir.state_path());
    wait_state(&env.router, "綁定", |st| {
        find_workstream(st, "solo", "so")["binding"]["state"] == "bound"
    })
    .await;

    let (status, body) = advance(&env.router, "wM:p4").await;

    assert_eq!(status, StatusCode::NO_CONTENT, "本體：{body:?}");
    let state = wait_state(&env.router, "推進", |st| {
        stage_of(st, "solo", "s1") == "Build"
    })
    .await;
    assert_eq!(find_workstream(&state, "solo", "so")["active_task"], "s1");
}

/// spec「沒有目前 task 且有多張 task」：404 `no_task_for_pane`，狀態不變。
#[tokio::test]
async fn advance_with_many_tasks_and_no_active_is_404_no_task_for_pane() {
    let dir = TempDir::new("adv-many");
    let env = build(vec![hand_project()], dir.state_path());
    wait_state(&env.router, "綁定", |st| {
        find_workstream(st, "p", "be")["binding"]["state"] == "bound"
    })
    .await;

    let (status, body) = advance(&env.router, "wJ:p1").await;

    assert_error(status, &body, StatusCode::NOT_FOUND, "no_task_for_pane");
    let state = state_json(&env.router).await;
    assert_eq!(stage_of(&state, "p", "t1"), "Plan");
    assert_eq!(stage_of(&state, "p", "t2"), "Plan");
}

/// spec「這個 pane 沒有綁定任何 workstream」。
#[tokio::test]
async fn advance_from_unbound_pane_is_404_no_task_for_pane() {
    let dir = TempDir::new("adv-unbound");
    let env = build(vec![hand_project()], dir.state_path());

    let (status, body) = advance(&env.router, "wX:p9").await;

    assert_error(status, &body, StatusCode::NOT_FOUND, "no_task_for_pane");
}

/// spec「兩條 workstream 綁定到同一個 pane」：手寫 `be`（目前 task `t1`）與 Repo Project 的 `local~wJ:p1` 都綁
/// `wJ:p1`，409 `ambiguous_task`，兩張 task 的進度都不變。
#[tokio::test]
async fn advance_with_two_candidates_is_409_ambiguous_task() {
    let dir = TempDir::new("adv-ambiguous");
    let env = build(vec![hand_project()], dir.state_path());
    add_app(&env).await;
    wait_state(&env.router, "綁定", |st| {
        find_workstream(st, "p", "be")["binding"]["state"] == "bound"
    })
    .await;
    let (status, _) = agent(
        &env.router,
        "POST",
        "/api/agent/projects/p/tasks/t1/start",
        "wJ:p1",
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let (status, body) = advance(&env.router, "wJ:p1").await;

    assert_error(status, &body, StatusCode::CONFLICT, "ambiguous_task");
    let state = state_json(&env.router).await;
    assert_eq!(stage_of(&state, "p", "t1"), "Plan");
    assert_eq!(stage_of(&state, "app", "local~wJ:p1"), "Plan");
}

/// spec「最後一站推進被拒」：409 `already_last_stage`，task 不變。
#[tokio::test]
async fn advance_at_last_stage_is_409() {
    let dir = TempDir::new("adv-last");
    let env = build(vec![solo_project("Build")], dir.state_path());
    wait_state(&env.router, "綁定", |st| {
        find_workstream(st, "solo", "so")["binding"]["state"] == "bound"
    })
    .await;

    let (status, body) = advance(&env.router, "wM:p4").await;

    assert_error(status, &body, StatusCode::CONFLICT, "already_last_stage");
    assert_eq!(
        stage_of(&state_json(&env.router).await, "solo", "s1"),
        "Build"
    );
}

/// spec「候選 task 已有標記」：標 completed 的唯一 task 仍是候選，被規則拒絕為 409。
#[tokio::test]
async fn advance_on_marked_only_task_is_409_already_marked() {
    let dir = TempDir::new("adv-marked");
    let env = build(vec![solo_project("Plan")], dir.state_path());
    wait_state(&env.router, "綁定", |st| {
        find_workstream(st, "solo", "so")["binding"]["state"] == "bound"
    })
    .await;
    let (status, _) = send(
        &env.router,
        "POST",
        "/api/projects/solo/tasks/s1/complete",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let (status, body) = advance(&env.router, "wM:p4").await;

    assert_error(status, &body, StatusCode::CONFLICT, "already_marked");
    let state = wait_state(&env.router, "s1 標 completed", |st| {
        find_task(st, "solo", "s1")["mark"] == "completed"
    })
    .await;
    assert_eq!(stage_of(&state, "solo", "s1"), "Plan");
}

/// spec「Repo Project 已標記的 task」：標 completed 的 Repo Project task 沒有目前 task，仍是唯一候選，409。
#[tokio::test]
async fn advance_on_marked_repo_task_is_409() {
    let dir = TempDir::new("adv-marked-repo");
    let env = build_empty(&dir);
    add_app(&env).await;
    let (status, _) = send(
        &env.router,
        "POST",
        "/api/projects/app/tasks/local~wJ:p1/complete",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let (status, body) = advance(&env.router, "wJ:p1").await;

    assert_error(status, &body, StatusCode::CONFLICT, "already_marked");
    assert_eq!(
        stage_of(&state_json(&env.router).await, "app", "local~wJ:p1"),
        "Plan"
    );
}

/// spec「WSL runtime 的 Repo Project pane」：WSL 的 pane 不算綁定，404 `no_task_for_pane`。
#[tokio::test]
async fn advance_from_wsl_repo_project_pane_is_404() {
    let dir = TempDir::new("adv-wsl");
    let env = build_empty(&dir);
    let (status, body) = send(
        &env.router,
        "POST",
        "/api/repo-projects",
        Some(&json!({"repo": WSL_REPO, "stages": ["Plan", "Build"]}).to_string()),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "本體：{body:?}");
    wait_state(&env.router, "出現", |st| project(st, "wslrepo").is_some()).await;

    let (status, body) = advance(&env.router, "w1:p1").await;

    assert_error(status, &body, StatusCode::NOT_FOUND, "no_task_for_pane");
    assert_eq!(
        stage_of(&state_json(&env.router).await, "wslrepo", "wsl~w1:p1"),
        "Plan"
    );
}

/// spec「缺標頭」。
#[tokio::test]
async fn advance_without_pane_header_is_400_missing_pane_id() {
    let dir = TempDir::new("adv-header");
    let env = build(vec![hand_project()], dir.state_path());

    let (status, body) = send(&env.router, "POST", "/api/agent/advance", None).await;
    assert_error(status, &body, StatusCode::BAD_REQUEST, "missing_pane_id");

    let (status, body) = advance(&env.router, "   ").await;
    assert_error(status, &body, StatusCode::BAD_REQUEST, "missing_pane_id");
}

/// spec「寫檔失敗」：500 `persist_failed`，task 的 stage 不變。
#[tokio::test]
async fn advance_persist_failure_is_500_and_stage_unchanged() {
    let dir = TempDir::new("adv-persist");
    let path = dir.state_path();
    fs::create_dir_all(&path).expect("在狀態檔路徑建立目錄");
    let env = build(vec![solo_project("Plan")], path);
    wait_state(&env.router, "綁定", |st| {
        find_workstream(st, "solo", "so")["binding"]["state"] == "bound"
    })
    .await;

    let (status, body) = advance(&env.router, "wM:p4").await;

    assert_error(
        status,
        &body,
        StatusCode::INTERNAL_SERVER_ERROR,
        "persist_failed",
    );
    assert_eq!(
        stage_of(&state_json(&env.router).await, "solo", "s1"),
        "Plan"
    );
}

/// 來源檢查與方法：跨站 403、非 POST 為 405，兩者都不改狀態。
#[tokio::test]
async fn advance_checks_source_and_method() {
    let dir = TempDir::new("adv-source");
    let env = build(vec![solo_project("Plan")], dir.state_path());
    wait_state(&env.router, "綁定", |st| {
        find_workstream(st, "solo", "so")["binding"]["state"] == "bound"
    })
    .await;

    for headers in [
        [
            ("x-herdr-pane-id", "wM:p4"),
            ("origin", "https://evil.example"),
        ],
        [("x-herdr-pane-id", "wM:p4"), ("host", "evil.example:0")],
    ] {
        let (status, body) =
            send_with(&env.router, "POST", "/api/agent/advance", &headers, None).await;
        assert_error(status, &body, StatusCode::FORBIDDEN, "forbidden_source");
    }
    for method in ["GET", "PUT", "PATCH", "DELETE"] {
        let (status, body) = agent(&env.router, method, "/api/agent/advance", "wM:p4").await;
        assert_error(
            status,
            &body,
            StatusCode::METHOD_NOT_ALLOWED,
            "method_not_allowed",
        );
    }
    assert_eq!(
        stage_of(&state_json(&env.router).await, "solo", "s1"),
        "Plan"
    );
}

// ---------------------------------------------------------------------------
// agent 端點對 Repo Project 固定綁定的 pane（最終審查 M1；spec `agent-reporting`「pane 身分判定」
// 「宣告目前 task」）。路徑用前端實際送出的逐段編碼（`encodeURIComponent`：`:` → `%3A`，`~` 不編碼），
// 同時證明路由解碼含 `:` 的 task id。
// ---------------------------------------------------------------------------

/// `local~wJ:p1` 逐段編碼後的樣子。
const WJ_TASK_ENCODED: &str = "local~wJ%3Ap1";

/// spec「Repo Project 固定綁定的 pane 算綁定」：`GET /api/agent/tasks` 回該 pane 的固定綁定工作線，目前 task 為它唯一的
/// task，`tasks` 只有該 task。
#[tokio::test]
async fn agent_tasks_lists_repo_project_pinned_pane_as_bound() {
    let dir = TempDir::new("agent-tasks-repo");
    let env = build_empty(&dir);
    add_app(&env).await;

    let (status, body) = agent(&env.router, "GET", "/api/agent/tasks", "wJ:p1").await;

    assert_eq!(status, StatusCode::OK, "本體：{body:?}");
    assert_eq!(body["pane_id"], "wJ:p1");
    let listed = body["workstreams"]
        .as_array()
        .expect("workstreams 應為陣列");
    assert_eq!(listed.len(), 1, "本體：{body:?}");
    assert_eq!(listed[0]["project"], "app");
    assert_eq!(listed[0]["workstream"], "local~wJ:p1");
    assert_eq!(listed[0]["active_task"], "local~wJ:p1");
    let tasks = listed[0]["tasks"].as_array().expect("tasks 應為陣列");
    assert_eq!(tasks.len(), 1, "只有該 pane 的 task：{tasks:?}");
    assert_eq!(tasks[0]["id"], "local~wJ:p1");
    assert_eq!(tasks[0]["stage"], "Plan");
    assert_eq!(tasks[0]["mark"], "none");
}

/// spec「Repo Project 的 task 宣告為空操作」：未標記的 task `start` 回 204，投影與狀態檔都不變（不重寫檔案）。
#[tokio::test]
async fn agent_start_on_unmarked_repo_task_is_204_and_writes_nothing() {
    let dir = TempDir::new("agent-start-repo");
    let env = build_empty(&dir);
    add_app(&env).await;
    let before_bytes = fs::read(dir.state_path()).expect("加入後狀態檔存在");
    let before_mtime = fs::metadata(dir.state_path())
        .and_then(|m| m.modified())
        .expect("狀態檔 mtime");
    let before_state = state_json(&env.router).await;

    let uri = format!("/api/agent/projects/app/tasks/{WJ_TASK_ENCODED}/start");
    let (status, body) = agent(&env.router, "POST", &uri, "wJ:p1").await;

    assert_eq!(status, StatusCode::NO_CONTENT, "本體：{body:?}");
    // 等過投影的合併窗，確認沒有任何變動被發佈。
    tokio::time::sleep(Duration::from_millis(200)).await;
    let after_state = state_json(&env.router).await;
    assert_eq!(project(&after_state, "app"), project(&before_state, "app"));
    assert_eq!(
        fs::read(dir.state_path()).expect("狀態檔仍在"),
        before_bytes,
        "狀態檔內容不變"
    );
    assert_eq!(
        fs::metadata(dir.state_path())
            .and_then(|m| m.modified())
            .expect("狀態檔 mtime"),
        before_mtime,
        "狀態檔沒有被重寫"
    );
}

/// spec「Repo Project 已標記的 task 不能宣告」：標 failed 的 task `start` 回 409，狀態不變。
#[tokio::test]
async fn agent_start_on_marked_repo_task_is_409() {
    let dir = TempDir::new("agent-start-repo-marked");
    let env = build_empty(&dir);
    add_app(&env).await;
    let (status, body) = send(
        &env.router,
        "POST",
        &format!("/api/projects/app/tasks/{WJ_TASK_ENCODED}/fail"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "本體：{body:?}");
    let marked = wait_state(&env.router, "local~wJ:p1 標 failed", |st| {
        find_task(st, "app", "local~wJ:p1")["mark"] == "failed"
    })
    .await;
    let before_bytes = fs::read(dir.state_path()).expect("狀態檔存在");

    let uri = format!("/api/agent/projects/app/tasks/{WJ_TASK_ENCODED}/start");
    let (status, body) = agent(&env.router, "POST", &uri, "wJ:p1").await;

    assert_error(status, &body, StatusCode::CONFLICT, "already_marked");
    tokio::time::sleep(Duration::from_millis(200)).await;
    let after = state_json(&env.router).await;
    assert_eq!(project(&after, "app"), project(&marked, "app"));
    assert_eq!(
        fs::read(dir.state_path()).expect("狀態檔仍在"),
        before_bytes,
        "狀態檔內容不變"
    );
}
