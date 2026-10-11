//! progress-model task 3.4 驗收測試：agent 端點 `GET /api/agent/tasks`、
//! `POST /api/agent/projects/{p}/tasks/{t}/{op}`（spec `agent-reporting` 全部；design D4、D6）。
//!
//! 同 `cockpit/tests/pipeline_api.rs`：用 `tower::ServiceExt::oneshot` 打真 `router`，後面接真
//! 寫入服務與真投影，`AppState.port` 停在 0、請求固定帶 `Host: 127.0.0.1:0`。runtime 狀態以
//! `RuntimeStore` 初始快照灌入；「經由 WSL 的 runtime」以 `AppState::path_mappings` 登記
//! `PathMapping::Wsl` 模擬（正式路徑由設定檔的 `wsl` endpoint 經
//! `PathMapping::from_endpoint` 產生，`HerdrEndpoint::Wsl` 即 `PathMapping::Wsl`），不需要
//! 真的起 WSL。

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
    AgentStatus, BindingSpec, ConnectionState, DomainState, Focused, Pane, PaneId, ProjectDef,
    ProjectId, RuntimeEvent, RuntimeId, RuntimeSnapshot, RuntimeStore, StoreHandle, Tab, TabId,
    TaskDef, TaskId, Workspace, WorkspaceId, WorkstreamDef, WorkstreamId, spawn_projector,
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

/// 每個測試專用的暫存目錄（同 `cockpit/tests/pipeline_api.rs`）。
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
            "cockpit-agent-api-test-{tag}-{}-{nanos}",
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

const WAIT_TIMEOUT: Duration = Duration::from_secs(5);
const PANE: &str = "wJ:p1";

fn binding(runtime: &str, workspace: &str) -> Option<BindingSpec> {
    Some(BindingSpec {
        runtime: RuntimeId::new(runtime),
        workspace: workspace.to_string(),
        pane_label: None,
        cwd: None,
        agent: None,
    })
}

fn workstream(id: &str, runtime: &str, workspace: &str) -> WorkstreamDef {
    WorkstreamDef {
        id: WorkstreamId::new(id),
        name: id.to_string(),
        binding: binding(runtime, workspace),
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

/// project `p`（stages `Plan`→`Build`）：`be` 綁 `win` 的 workspace `wJ`（pane `wJ:p1`）、
/// `fe` 綁 `win` 的 workspace `wK`（pane `wK:p2`）；`t1` 在 `Plan`、`t2` 在 `Build`（都屬 `be`），
/// `f1` 屬 `fe`。
fn sample_project() -> ProjectDef {
    ProjectDef {
        id: ProjectId::new("p"),
        name: "p".to_string(),
        stages: vec!["Plan".to_string(), "Build".to_string()],
        workstreams: vec![workstream("be", "win", "wJ"), workstream("fe", "win", "wK")],
        tasks: vec![
            task("t1", "be", "Plan"),
            task("t2", "be", "Build"),
            task("f1", "fe", "Plan"),
        ],
        repo: None,
    }
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

fn win_store() -> RuntimeStore {
    let mut store = RuntimeStore::new();
    add_runtime(&mut store, "win", &[("wJ", PANE), ("wK", "wK:p2")]);
    store
}

fn set_agent_status(handle: &StoreHandle, runtime: &str, pane_id: &str, status: AgentStatus) {
    handle
        .apply(
            &RuntimeId::new(runtime),
            RuntimeEvent::AgentStatusChanged {
                pane_id: PaneId::new(pane_id),
                status,
                title: None,
                agent: Some("claude".to_string()),
            },
            SystemTime::now(),
        )
        .expect("pane 已存在，apply 不應該失敗");
}

/// 組出真寫入服務＋真投影的 `AppState`；`wsl_runtimes` 內的 runtime 在 `path_mappings` 登記成
/// `PathMapping::Wsl`，其餘登記成 `Native`。
fn build_with(
    projects: Vec<ProjectDef>,
    store: RuntimeStore,
    wsl_runtimes: &[&str],
    state_path: PathBuf,
) -> (StoreHandle, AppState) {
    let runtime_ids: Vec<RuntimeId> = store.runtime_ids();
    let handle = StoreHandle::new_with_domain(store, DomainState::from_projects(projects));
    let service = ProgressService::new(handle.clone(), state_path);
    let _projector = spawn_projector(handle.clone());
    let path_mappings: HashMap<RuntimeId, PathMapping> = runtime_ids
        .into_iter()
        .map(|id| {
            let mapping = if wsl_runtimes.contains(&id.as_str()) {
                PathMapping::Wsl {
                    distro: "Ubuntu".to_string(),
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
    (handle, state)
}

fn build(path: PathBuf) -> (StoreHandle, AppState) {
    build_with(vec![sample_project()], win_store(), &[], path)
}

async fn send(
    router: &axum::Router,
    method: &str,
    uri: &str,
    extra_headers: &[(&str, &str)],
) -> (StatusCode, Value) {
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .header("host", "127.0.0.1:0");
    for (name, value) in extra_headers {
        builder = builder.header(*name, *value);
    }
    let response = router
        .clone()
        .oneshot(builder.body(Body::empty()).expect("request 建構不應該失敗"))
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

/// 帶 `X-Herdr-Pane-Id` 的 agent 請求。
async fn agent(router: &axum::Router, method: &str, uri: &str, pane: &str) -> (StatusCode, Value) {
    send(router, method, uri, &[("x-herdr-pane-id", pane)]).await
}

/// `PUT` 帶 JSON 本體（改綁用）。
async fn put_json(router: &axum::Router, uri: &str, body: &str) -> (StatusCode, Value) {
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(uri)
                .header("host", "127.0.0.1:0")
                .body(Body::from(body.to_string()))
                .expect("request 建構不應該失敗"),
        )
        .await
        .expect("oneshot 呼叫不應該失敗");
    (response.status(), Value::Null)
}

async fn state_json(router: &axum::Router) -> Value {
    send(router, "GET", "/api/state", &[]).await.1
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

fn find_workstream<'a>(state: &'a Value, project: &str, ws: &str) -> &'a Value {
    state["projects"]
        .as_array()
        .and_then(|ps| ps.iter().find(|p| p["id"] == project))
        .and_then(|p| p["workstreams"].as_array())
        .and_then(|wss| wss.iter().find(|w| w["id"] == ws))
        .expect("投影應該有這條 workstream")
}

fn find_task<'a>(state: &'a Value, project: &str, id: &str) -> &'a Value {
    state["projects"]
        .as_array()
        .and_then(|ps| ps.iter().find(|p| p["id"] == project))
        .and_then(|p| p["tasks"].as_array())
        .and_then(|ts| ts.iter().find(|t| t["id"] == id))
        .expect("投影應該有這個 task")
}

fn active_of(state: &Value, ws: &str) -> Value {
    find_workstream(state, "p", ws)["active_task"].clone()
}

/// 讓投影先反映初始綁定（`bound`），避免測試在投影還沒第一次算出來時就送請求。
async fn wait_bound(router: &axum::Router, ws: &str) {
    wait_state(router, "workstream 綁定", |s| {
        find_workstream(s, "p", ws)["binding"]["state"] == "bound"
    })
    .await;
}

// ---------------------------------------------------------------------------
// pane 身分判定
// ---------------------------------------------------------------------------

#[tokio::test]
async fn missing_header_is_400_missing_pane_id() {
    let dir = TempDir::new("missing-header");
    let (_h, state) = build(dir.path().join("state.json"));
    let router = http::router(state);

    let (status, body) = send(&router, "GET", "/api/agent/tasks", &[]).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["code"], "missing_pane_id");
    assert!(body["error"].as_str().is_some_and(|s| !s.is_empty()));

    // 只有空白也視為缺少；POST 同樣（且標頭檢查先於 project／task 存在檢查）。
    let (status, body) = agent(&router, "GET", "/api/agent/tasks", "   ").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["code"], "missing_pane_id");
    let (status, body) = send(
        &router,
        "POST",
        "/api/agent/projects/nope/tasks/nope/start",
        &[],
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["code"], "missing_pane_id");
}

/// spec「WSL runtime 的 pane 不算」：綁在 WSL runtime 的 pane，GET 回 200 但清單為空，
/// POST 回 403 `pane_not_bound`。
#[tokio::test]
async fn wsl_runtime_pane_does_not_count() {
    let dir = TempDir::new("wsl");
    let project = ProjectDef {
        id: ProjectId::new("p"),
        name: "p".to_string(),
        stages: vec!["Plan".to_string(), "Build".to_string()],
        workstreams: vec![workstream("be", "wsl", "w1")],
        tasks: vec![task("t1", "be", "Plan")],
        repo: None,
    };
    let mut store = RuntimeStore::new();
    add_runtime(&mut store, "wsl", &[("w1", "w1:p1")]);
    let (handle, state) = build_with(vec![project], store, &["wsl"], dir.path().join("s.json"));
    let router = http::router(state);
    wait_bound(&router, "be").await;

    let (status, body) = agent(&router, "GET", "/api/agent/tasks", "w1:p1").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, json!({"pane_id": "w1:p1", "workstreams": []}));

    let (status, body) = agent(
        &router,
        "POST",
        "/api/agent/projects/p/tasks/t1/start",
        "w1:p1",
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["code"], "pane_not_bound");
    assert!(handle.with_domain(|d| d.active.is_empty()));
}

/// spec「兩個 Windows runtime 撞號」：`win`、`win2` 各有 workstream 綁到同一個 pane id。
#[tokio::test]
async fn pane_id_collision_across_windows_runtimes_binds_nothing() {
    let dir = TempDir::new("collision");
    let project = ProjectDef {
        id: ProjectId::new("p"),
        name: "p".to_string(),
        stages: vec!["Plan".to_string(), "Build".to_string()],
        workstreams: vec![workstream("a", "win", "w1"), workstream("b", "win2", "w1")],
        tasks: vec![task("ta", "a", "Plan"), task("tb", "b", "Plan")],
        repo: None,
    };
    let mut store = RuntimeStore::new();
    add_runtime(&mut store, "win", &[("w1", "w1:p1")]);
    add_runtime(&mut store, "win2", &[("w1", "w1:p1")]);
    let (handle, state) = build_with(vec![project], store, &[], dir.path().join("s.json"));
    let router = http::router(state);
    wait_bound(&router, "a").await;
    wait_bound(&router, "b").await;

    let (status, body) = agent(&router, "GET", "/api/agent/tasks", "w1:p1").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["workstreams"], json!([]));

    for task in ["ta", "tb"] {
        let (status, body) = agent(
            &router,
            "POST",
            &format!("/api/agent/projects/p/tasks/{task}/start"),
            "w1:p1",
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["code"], "pane_not_bound");
    }
    assert!(handle.with_domain(|d| d.active.is_empty()));
}

/// review M2：撞號判定看「另一個非 WSL runtime 上存在同 id 且未 exited 的 pane」，不論它有沒有綁定；
/// `win2` 的 `w1:p1` 沒有任何 workstream 綁它，請求仍可能來自那裡，必須視為未綁定。
#[tokio::test]
async fn unbound_same_id_pane_on_other_windows_runtime_is_collision() {
    let dir = TempDir::new("collision-unbound");
    let project = ProjectDef {
        id: ProjectId::new("p"),
        name: "p".to_string(),
        stages: vec!["Plan".to_string(), "Build".to_string()],
        workstreams: vec![workstream("a", "win", "w1")],
        tasks: vec![task("ta", "a", "Plan")],
        repo: None,
    };
    let mut store = RuntimeStore::new();
    add_runtime(&mut store, "win", &[("w1", "w1:p1")]);
    add_runtime(&mut store, "win2", &[("w9", "w1:p1")]);
    let (handle, state) = build_with(vec![project], store, &[], dir.path().join("s.json"));
    let router = http::router(state);
    wait_bound(&router, "a").await;

    let (status, body) = agent(&router, "GET", "/api/agent/tasks", "w1:p1").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["workstreams"], json!([]));
    let (status, body) = agent(
        &router,
        "POST",
        "/api/agent/projects/p/tasks/ta/start",
        "w1:p1",
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["code"], "pane_not_bound");
    assert!(handle.with_domain(|d| d.active.is_empty()));
}

/// review M2：另一個 Windows runtime 上同 id 的 pane 已 exited 就不算歧義，綁定照常成立；
/// 另一個 runtime 是 WSL 時同理（WSL 的 pane 本來就不能拿來認人）。
#[tokio::test]
async fn exited_or_wsl_same_id_pane_is_not_collision() {
    let dir = TempDir::new("collision-exited");
    let project = ProjectDef {
        id: ProjectId::new("p"),
        name: "p".to_string(),
        stages: vec!["Plan".to_string(), "Build".to_string()],
        workstreams: vec![workstream("a", "win", "w1")],
        tasks: vec![task("ta", "a", "Plan")],
        repo: None,
    };
    let mut store = RuntimeStore::new();
    add_runtime(&mut store, "win", &[("w1", "w1:p1")]);
    add_runtime(&mut store, "win2", &[("w9", "w1:p1")]);
    add_runtime(&mut store, "wsl", &[("w8", "w1:p1")]);
    let (handle, state) = build_with(vec![project], store, &["wsl"], dir.path().join("s.json"));
    handle
        .apply(
            &RuntimeId::new("win2"),
            RuntimeEvent::PaneExited(PaneId::new("w1:p1")),
            SystemTime::now(),
        )
        .expect("pane 已存在");
    let router = http::router(state);
    wait_bound(&router, "a").await;
    wait_state(&router, "win2 的 pane 已 exited", |s| {
        s["runtimes"]
            .as_array()
            .and_then(|rs| rs.iter().find(|r| r["id"] == "win2"))
            .and_then(|r| r["workspaces"][0]["tabs"][0]["panes"][0]["exited"].as_bool())
            == Some(true)
    })
    .await;

    let (status, _) = agent(
        &router,
        "POST",
        "/api/agent/projects/p/tasks/ta/start",
        "w1:p1",
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
}

/// spec「斷線 runtime 最後已知的 pane 仍參與撞號」（ui-fixes task 3.3）：`win2` 斷線後它最後已知的
/// pane 樹照常保留在投影中，其中未 exited 的 `w1:p1` 仍算擁有者，與 `win` 撞號。這是現行行為的
/// 回歸測試，加入時就是綠的（撞號判定本來就只看 pane 樹，不看連線狀態）。
#[tokio::test]
async fn disconnected_runtime_last_known_pane_still_collides() {
    let dir = TempDir::new("collision-disconnected");
    let project = ProjectDef {
        id: ProjectId::new("p"),
        name: "p".to_string(),
        stages: vec!["Plan".to_string(), "Build".to_string()],
        workstreams: vec![workstream("a", "win", "w1")],
        tasks: vec![task("ta", "a", "Plan")],
        repo: None,
    };
    let mut store = RuntimeStore::new();
    add_runtime(&mut store, "win", &[("w1", "w1:p1")]);
    add_runtime(&mut store, "win2", &[("w9", "w1:p1")]);
    let (handle, state) = build_with(vec![project], store, &[], dir.path().join("s.json"));
    handle
        .set_connection(
            &RuntimeId::new("win2"),
            ConnectionState::Disconnected {
                reason: "test".to_string(),
                retry_in: Duration::from_secs(1),
            },
        )
        .expect("runtime 已登記");
    let router = http::router(state);
    wait_bound(&router, "a").await;
    wait_state(
        &router,
        "win2 已斷線但保留最後已知的 pane",
        |s| {
            s["runtimes"]
                .as_array()
                .and_then(|rs| rs.iter().find(|r| r["id"] == "win2"))
                .is_some_and(|r| {
                    r["connection"]["state"] == "disconnected"
                        && r["workspaces"][0]["tabs"][0]["panes"][0]["id"] == "w1:p1"
                })
        },
    )
    .await;

    let (status, body) = agent(&router, "GET", "/api/agent/tasks", "w1:p1").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["workstreams"], json!([]));
    let (status, body) = agent(
        &router,
        "POST",
        "/api/agent/projects/p/tasks/ta/start",
        "w1:p1",
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["code"], "pane_not_bound");
    assert!(handle.with_domain(|d| d.active.is_empty()));
}

/// review M1：改綁的 `PUT` 已 204 之後，舊 pane 立刻 `start` 必須被拒，不能留下由已不屬於這條
/// workstream 的 agent 宣告的目前 task。
///
/// 這個測試驗的範圍是端到端結果：`403 pane_not_bound`、且 `be` 沒有目前 task。它**分不出**請求
/// 是被哪一條路擋下的：投影還沒重算時由 service 的鎖內重驗擋下，或投影已追上時由 handler 的
/// 綁定判定擋下，兩條路結果相同，且哪條發生取決於時序（ui-fixes task 3.5、design D10）。要確定性
/// 地證明鎖內重驗有效，看 service 層的 basis 測試（不經投影、直接造出「判定依據已不成立」）：
/// `cockpit/tests/progress_service.rs` 的
/// `declare_active_rejects_when_binding_basis_no_longer_holds`（`start` 路徑）與
/// `agent_advance_rejects_when_binding_basis_no_longer_holds`（`advance` 路徑）。
#[tokio::test]
async fn start_from_old_pane_after_rebind_leaves_no_active_task() {
    let dir = TempDir::new("rebind-race");
    let (handle, state) = build(dir.path().join("state.json"));
    let router = http::router(state);
    wait_bound(&router, "be").await;

    let (status, _) = put_json(
        &router,
        "/api/projects/p/workstreams/be/override",
        r#"{"runtime":"win","pane_id":"wK:p2"}"#,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, body) = agent(
        &router,
        "POST",
        "/api/agent/projects/p/tasks/t1/start",
        PANE,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["code"], "pane_not_bound");

    // 等投影追上之後也沒有目前 task。
    wait_state(&router, "be 改綁到 wK:p2", |s| {
        find_workstream(s, "p", "be")["binding"]["pane_id"] == "wK:p2"
    })
    .await;
    assert!(handle.with_domain(|d| d.active.is_empty()));
}

/// 撞號只算非 WSL：同一個 pane id 在 WSL runtime 與一個 Windows runtime 都有綁定時，
/// WSL 那條先被排除，Windows 那條仍然成立。
#[tokio::test]
async fn wsl_binding_is_excluded_before_collision_check() {
    let dir = TempDir::new("wsl-and-win");
    let project = ProjectDef {
        id: ProjectId::new("p"),
        name: "p".to_string(),
        stages: vec!["Plan".to_string(), "Build".to_string()],
        workstreams: vec![workstream("a", "win", "w1"), workstream("b", "wsl", "w1")],
        tasks: vec![task("ta", "a", "Plan"), task("tb", "b", "Plan")],
        repo: None,
    };
    let mut store = RuntimeStore::new();
    add_runtime(&mut store, "win", &[("w1", "w1:p1")]);
    add_runtime(&mut store, "wsl", &[("w1", "w1:p1")]);
    let (_h, state) = build_with(vec![project], store, &["wsl"], dir.path().join("s.json"));
    let router = http::router(state);
    wait_bound(&router, "a").await;
    wait_bound(&router, "b").await;

    let (_, body) = agent(&router, "GET", "/api/agent/tasks", "w1:p1").await;
    let listed = body["workstreams"].as_array().expect("應為陣列");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0]["workstream"], "a");
}

/// spec「跨站請求被拒」：來源檢查最先，狀態不變；GET 也套用。
#[tokio::test]
async fn cross_site_origin_rejected_before_anything_else() {
    let dir = TempDir::new("cross-site");
    let (handle, state) = build(dir.path().join("state.json"));
    let router = http::router(state);
    wait_bound(&router, "be").await;

    // 沒帶 pane id 標頭也一樣是 403（來源檢查先於標頭檢查）。
    let (status, body) = send(
        &router,
        "POST",
        "/api/agent/projects/p/tasks/t1/start",
        &[("origin", "https://evil.example")],
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["code"], "forbidden_source");
    assert!(handle.with_domain(|d| d.active.is_empty()), "狀態不變");

    let (status, body) = send(
        &router,
        "GET",
        "/api/agent/tasks",
        &[
            ("origin", "https://evil.example"),
            ("x-herdr-pane-id", PANE),
        ],
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["code"], "forbidden_source");
}

#[tokio::test]
async fn wrong_method_is_405() {
    let dir = TempDir::new("method");
    let (_h, state) = build(dir.path().join("state.json"));
    let router = http::router(state);

    let (status, body) = agent(&router, "POST", "/api/agent/tasks", PANE).await;
    assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(body["code"], "method_not_allowed");
    let (status, body) = agent(&router, "GET", "/api/agent/projects/p/tasks/t1/start", PANE).await;
    assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(body["code"], "method_not_allowed");
}

// ---------------------------------------------------------------------------
// 查詢自己綁定的 task
// ---------------------------------------------------------------------------

/// spec「列出綁定的 task」：只列綁定到這個 pane 的 workstream（`fe` 不在內），`next_stage` 由
/// project 的 stages 算，目前 task 為 `t1`。
#[tokio::test]
async fn list_bound_tasks() {
    let dir = TempDir::new("list");
    let (_h, state) = build(dir.path().join("state.json"));
    let router = http::router(state);
    wait_bound(&router, "be").await;

    let (status, _) = agent(
        &router,
        "POST",
        "/api/agent/projects/p/tasks/t1/start",
        PANE,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    wait_state(&router, "t1 成為目前 task", |s| {
        active_of(s, "be") == "t1"
    })
    .await;

    let (status, body) = agent(&router, "GET", "/api/agent/tasks", PANE).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["pane_id"], PANE);
    let listed = body["workstreams"].as_array().expect("應為陣列");
    assert_eq!(listed.len(), 1, "fe 綁在另一個 pane，不該出現");
    assert_eq!(listed[0]["project"], "p");
    assert_eq!(listed[0]["workstream"], "be");
    assert_eq!(listed[0]["active_task"], "t1");
    let tasks = listed[0]["tasks"].as_array().expect("tasks 應為陣列");
    assert_eq!(tasks.len(), 2);
    assert_eq!(
        tasks[0],
        json!({
            "id": "t1", "title": "title-t1", "stage": "Plan", "next_stage": "Build",
            "mark": "none", "status": "ready", "depends_on": [],
        })
    );
    assert_eq!(tasks[1]["id"], "t2");
    assert_eq!(tasks[1]["stage"], "Build");
    assert_eq!(
        tasks[1]["next_stage"],
        Value::Null,
        "最後一站的 next_stage 為 null"
    );
}

/// 沒有目前 task 時 `active_task` 為 `null`（不省略）。
#[tokio::test]
async fn list_without_active_task_is_null() {
    let dir = TempDir::new("list-null");
    let (_h, state) = build(dir.path().join("state.json"));
    let router = http::router(state);
    wait_bound(&router, "be").await;

    let (status, body) = agent(&router, "GET", "/api/agent/tasks", PANE).await;
    assert_eq!(status, StatusCode::OK);
    let listed = body["workstreams"].as_array().expect("應為陣列");
    assert_eq!(listed.len(), 1);
    assert!(
        listed[0]
            .as_object()
            .is_some_and(|o| o.contains_key("active_task")),
        "active_task 不可省略"
    );
    assert_eq!(listed[0]["active_task"], Value::Null);
}

/// 同一個 pane 綁兩條 workstream（兩條都自動解析到同一個 workspace 的唯一 pane）：兩條都列出，
/// 依設定順序。
#[tokio::test]
async fn same_pane_bound_by_two_workstreams_lists_both() {
    let dir = TempDir::new("two-ws");
    let project = ProjectDef {
        id: ProjectId::new("p"),
        name: "p".to_string(),
        stages: vec!["Plan".to_string(), "Build".to_string()],
        workstreams: vec![
            workstream("be", "win", "wJ"),
            workstream("api", "win", "wJ"),
        ],
        tasks: vec![task("t1", "be", "Plan"), task("a1", "api", "Plan")],
        repo: None,
    };
    let (_h, state) = build_with(vec![project], win_store(), &[], dir.path().join("s.json"));
    let router = http::router(state);
    wait_bound(&router, "be").await;
    wait_bound(&router, "api").await;

    let (status, body) = agent(&router, "GET", "/api/agent/tasks", PANE).await;
    assert_eq!(status, StatusCode::OK);
    let ids: Vec<&str> = body["workstreams"]
        .as_array()
        .expect("應為陣列")
        .iter()
        .map(|w| w["workstream"].as_str().expect("字串"))
        .collect();
    assert_eq!(ids, ["be", "api"]);

    // 兩條 workstream 的 task 都可以宣告。
    for task in ["t1", "a1"] {
        let (status, _) = agent(
            &router,
            "POST",
            &format!("/api/agent/projects/p/tasks/{task}/start"),
            PANE,
        )
        .await;
        assert_eq!(status, StatusCode::NO_CONTENT, "{task}");
    }
}

#[tokio::test]
async fn unknown_pane_lists_empty() {
    let dir = TempDir::new("unknown-pane");
    let (_h, state) = build(dir.path().join("state.json"));
    let router = http::router(state);
    wait_bound(&router, "be").await;

    let (status, body) = agent(&router, "GET", "/api/agent/tasks", "zzz:p9").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, json!({"pane_id": "zzz:p9", "workstreams": []}));
}

// ---------------------------------------------------------------------------
// 宣告目前 task
// ---------------------------------------------------------------------------

/// spec「宣告成功」：204；稍後 `be` 的 `active_task` 為 `t2`，`t2` 為 `running`、`t1` 為 `ready`；
/// 狀態檔落地為 v2 並含 `active`。
#[tokio::test]
async fn start_declares_active_task() {
    let dir = TempDir::new("start");
    let path = dir.path().join("state.json");
    let (handle, state) = build(path.clone());
    let router = http::router(state);
    wait_bound(&router, "be").await;
    set_agent_status(&handle, "win", PANE, AgentStatus::Working);

    let (status, body) = agent(
        &router,
        "POST",
        "/api/agent/projects/p/tasks/t2/start",
        PANE,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(body, Value::Null);

    let projected = wait_state(&router, "t2 running", |s| {
        active_of(s, "be") == "t2" && find_task(s, "p", "t2")["status"] == "running"
    })
    .await;
    assert_eq!(find_task(&projected, "p", "t1")["status"], "ready");

    let on_disk: Value =
        serde_json::from_str(&fs::read_to_string(&path).expect("狀態檔應該存在")).expect("JSON");
    assert_eq!(on_disk["version"], 4);
    assert_eq!(on_disk["projects"]["p"]["active"], json!({"be": "t2"}));
}

/// spec「動別人的 task」：`f1` 屬 `fe`（綁 `wK:p2`），用 `wJ:p1` 宣告回 403，`fe` 不變。
#[tokio::test]
async fn start_on_someone_elses_task_is_403() {
    let dir = TempDir::new("someone-else");
    let (handle, state) = build(dir.path().join("state.json"));
    let router = http::router(state);
    wait_bound(&router, "fe").await;

    let (status, body) = agent(
        &router,
        "POST",
        "/api/agent/projects/p/tasks/f1/start",
        PANE,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["code"], "pane_not_bound");
    assert_eq!(body["params"], json!({"task": "f1", "pane": PANE}));
    assert!(
        body["error"]
            .as_str()
            .is_some_and(|e| e.contains("沒有綁定到 pane")),
        "error 原文照舊：{body:?}"
    );
    assert!(
        handle.with_domain(|d| d.active.is_empty()),
        "fe 的目前 task 不變"
    );
}

/// spec「已標記的 task 不能宣告」。
#[tokio::test]
async fn start_on_marked_task_is_409() {
    let dir = TempDir::new("marked");
    let (handle, state) = build(dir.path().join("state.json"));
    let router = http::router(state);
    wait_bound(&router, "be").await;

    let (status, _) = send(&router, "POST", "/api/projects/p/tasks/t1/complete", &[]).await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let (status, body) = agent(
        &router,
        "POST",
        "/api/agent/projects/p/tasks/t1/start",
        PANE,
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert!(body["error"].as_str().is_some_and(|s| !s.is_empty()));
    assert_eq!(body["code"], "already_marked");
    assert!(body.get("params").is_none());
    assert!(
        handle.with_domain(|d| d.active.is_empty()),
        "目前 task 不變"
    );
}

/// design D6：先查存在再查綁定——打錯 project／task 回 404，不是誤導的 403（pane 沒綁任何東西
/// 也一樣）。
#[tokio::test]
async fn unknown_project_or_task_is_404_not_403() {
    let dir = TempDir::new("404-not-403");
    let (_h, state) = build(dir.path().join("state.json"));
    let router = http::router(state);
    wait_bound(&router, "be").await;

    for (uri, pane, code, id) in [
        (
            "/api/agent/projects/nope/tasks/t1/start",
            PANE,
            "unknown_project",
            "nope",
        ),
        (
            "/api/agent/projects/p/tasks/nope/start",
            PANE,
            "unknown_task",
            "nope",
        ),
        (
            "/api/agent/projects/p/tasks/nope/advance",
            "zzz:p9",
            "unknown_task",
            "nope",
        ),
        (
            "/api/agent/projects/nope/tasks/nope/advance",
            "zzz:p9",
            "unknown_project",
            "nope",
        ),
    ] {
        let (status, body) = agent(&router, "POST", uri, pane).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{uri}");
        assert!(
            body["error"].as_str().is_some_and(|s| !s.is_empty()),
            "{uri}"
        );
        assert_ne!(body["code"], "pane_not_bound", "{uri}");
        assert_eq!(body["code"], code, "{uri}");
        assert_eq!(body["params"], json!({"id": id}), "{uri}");
    }
}

/// 沒有任何 project（`progress` 為 `None`）時，POST 一律 404。
#[tokio::test]
async fn no_progress_service_is_404() {
    let dir = TempDir::new("no-service");
    let (_h, mut state) = build(dir.path().join("state.json"));
    state.progress = None;
    let router = http::router(state);

    let (status, body) = agent(
        &router,
        "POST",
        "/api/agent/projects/p/tasks/t1/start",
        PANE,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(body["error"].as_str().is_some_and(|s| !s.is_empty()));
    assert_eq!(body["code"], "unknown_project");
    assert_eq!(body["params"], json!({"id": "p"}));
}

// ---------------------------------------------------------------------------
// agent 推進
// ---------------------------------------------------------------------------

/// spec「推進並設為目前 task」：204；`t1` 在 `Build`、`be` 的 `active_task` 為 `t1`；狀態檔都有。
#[tokio::test]
async fn advance_moves_stage_and_sets_active() {
    let dir = TempDir::new("advance");
    let path = dir.path().join("state.json");
    let (_h, state) = build(path.clone());
    let router = http::router(state);
    wait_bound(&router, "be").await;

    let (status, _) = agent(
        &router,
        "POST",
        "/api/agent/projects/p/tasks/t1/advance",
        PANE,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    wait_state(&router, "t1 在 Build 且為目前 task", |s| {
        find_task(s, "p", "t1")["stage"] == "Build" && active_of(s, "be") == "t1"
    })
    .await;

    let on_disk: Value =
        serde_json::from_str(&fs::read_to_string(&path).expect("狀態檔應該存在")).expect("JSON");
    assert_eq!(on_disk["projects"]["p"]["tasks"]["t1"]["stage"], "Build");
    assert_eq!(on_disk["projects"]["p"]["active"], json!({"be": "t1"}));
}

/// spec「最後一站推進被拒」：409，`t2` 仍在 `Build`，目前 task 仍為 `t1`。
#[tokio::test]
async fn advance_at_last_stage_is_409() {
    let dir = TempDir::new("last-stage");
    let (_h, state) = build(dir.path().join("state.json"));
    let router = http::router(state);
    wait_bound(&router, "be").await;

    let (status, _) = agent(
        &router,
        "POST",
        "/api/agent/projects/p/tasks/t1/start",
        PANE,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let (status, body) = agent(
        &router,
        "POST",
        "/api/agent/projects/p/tasks/t2/advance",
        PANE,
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body["error"], "已是最後一個 Stage");
    assert_eq!(body["code"], "already_last_stage");
    assert!(body.get("params").is_none());

    let projected = wait_state(&router, "t1 仍為目前 task", |s| {
        active_of(s, "be") == "t1"
    })
    .await;
    assert_eq!(find_task(&projected, "p", "t2")["stage"], "Build");
}

/// advance 同樣先判綁定：動別人的 task 回 403，stage 不變。
#[tokio::test]
async fn advance_on_someone_elses_task_is_403() {
    let dir = TempDir::new("advance-403");
    let (handle, state) = build(dir.path().join("state.json"));
    let router = http::router(state);
    wait_bound(&router, "fe").await;

    let (status, body) = agent(
        &router,
        "POST",
        "/api/agent/projects/p/tasks/f1/advance",
        PANE,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["code"], "pane_not_bound");
    assert_eq!(
        handle.with_domain(|d| d.progress[&ProjectId::new("p")][&TaskId::new("f1")]
            .stage
            .clone()),
        "Plan"
    );
}

/// spec「agent 不能標完成」：`complete`、`fail`、`clear`、`retreat` 與任何其他操作都是 404，
/// 標記不變；操作檢查不需要綁定。
#[tokio::test]
async fn agent_cannot_use_other_ops() {
    let dir = TempDir::new("other-ops");
    let (handle, state) = build(dir.path().join("state.json"));
    let router = http::router(state);
    wait_bound(&router, "be").await;

    for op in ["complete", "fail", "clear", "retreat", "bogus"] {
        let (status, body) = agent(
            &router,
            "POST",
            &format!("/api/agent/projects/p/tasks/t1/{op}"),
            PANE,
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{op}");
        assert!(
            body["error"].as_str().is_some_and(|s| !s.is_empty()),
            "{op}"
        );
        assert_eq!(body["code"], "invalid_op", "{op}");
        assert_eq!(body["params"], json!({"op": op}), "{op}");
    }
    handle.with_domain(|d| {
        let t1 = &d.progress[&ProjectId::new("p")][&TaskId::new("t1")];
        assert_eq!(t1.mark, cockpit_core::Mark::None);
        assert_eq!(t1.stage, "Plan");
        assert!(d.active.is_empty());
    });
}
