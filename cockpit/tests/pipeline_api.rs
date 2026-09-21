//! Task 4.1／4.2 驗收測試：`cockpit::http::router` 的寫入端點與來源檢查 middleware（spec
//! `pipeline-progress`「進度寫入端點」「綁定覆蓋端點」「寫入端點只接受本機同源請求」；
//! `cockpit-dashboard`「寫入端點不接受 GET」；design D6）。
//!
//! 同 `cockpit/tests/http.rs` 用 `tower::ServiceExt::oneshot` 打 `router`，不開真正的 port——
//! `AppState.port` 停在 `build()` 給的值（預設 0），來源檢查用這個值比對 `Host`，不是真正監聽
//! 到的埠（design D6「測試用 port 0 也正確」）。task 4.1 那批測試本身不驗來源檢查，一律靠
//! [`send`] 帶固定的合法 `Host`；task 4.2 那批（檔尾「來源檢查 middleware」一節）用
//! [`send_with_headers`] 自己指定 `Host`／`Origin`。暫存狀態檔目錄沿用
//! `cockpit/tests/progress_service.rs` 自製的 `TempDir`（task 4.1 brief 約束：不加
//! `tempfile`）。
//!
//! Codex fix round 1 finding 2：成功案例不能只讀 `StoreHandle::with_domain`（那只證明寫入
//! 服務自己的記憶體狀態對，證不到投影與狀態檔）。`build()` 因此固定起一個真的
//! `spawn_projector`；成功的進度操作與覆蓋設定／取消都額外做兩件事：(a) 操作前記錄
//! `/api/state` 的 version，操作後輪詢到 version 真的遞增，斷言投影裡的 `stage`／`mark`／
//! `binding` 反映出變化；(b) 讀回磁碟上真正的狀態檔，斷言其內容。

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU16, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use axum::body::Body;
use axum::http::{Request, StatusCode};
use cockpit::http::{self, AppState};
use cockpit::progress_service::ProgressService;
use cockpit_core::{
    AgentStatus, BindingSpec, ConnectionState, DomainState, Focused, Mark, Override, Pane, PaneId,
    ProjectDef, ProjectId, RuntimeEvent, RuntimeId, RuntimeSnapshot, RuntimeStore, StoreHandle,
    Tab, TabId, TaskDef, TaskId, Workspace, WorkspaceId, WorkstreamDef, WorkstreamId,
    spawn_projector,
};
use http_body_util::BodyExt;
use serde_json::Value;
use tower::ServiceExt;

/// 每個測試專用的暫存目錄（同 `cockpit/tests/progress_service.rs`、
/// `cockpit/tests/app.rs`）。
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
            "cockpit-pipeline-api-test-{tag}-{}-{nanos}",
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

const RUNTIME: &str = "win";
/// 等投影／`/api/state` version 遞增的逾時上限；卡住就當測試失敗，不要無限期掛著。
const WAIT_TIMEOUT: Duration = Duration::from_secs(5);

/// 一個 project `p`（stages `Spec`→`Build`，恰好兩站——夠測「推進成功」與「已是最後一站」
/// 兩種情境）、一條 workstream `be`（沒有設定檔 `binding`，覆蓋獨立於它）、一個 task `t1`。
fn sample_project() -> ProjectDef {
    ProjectDef {
        id: ProjectId::new("p"),
        name: "p".to_string(),
        stages: vec!["Spec".to_string(), "Build".to_string()],
        workstreams: vec![WorkstreamDef {
            id: WorkstreamId::new("be"),
            name: "be".to_string(),
            binding: None,
        }],
        tasks: vec![TaskDef {
            id: TaskId::new("t1"),
            title: "t1".to_string(),
            workstream: WorkstreamId::new("be"),
            stage: "Spec".to_string(),
            depends_on: Vec::new(),
        }],
    }
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

/// runtime `win` 已登記、`connected`，帶一個未 exited 的 pane `p1`——讓 `validate_override`
/// 通過（覆蓋端點的成功／落檔失敗情境）。
fn connected_store() -> RuntimeStore {
    let id = RuntimeId::new(RUNTIME);
    let mut store = RuntimeStore::new();
    store.register(id.clone(), "herdr".to_string(), "test".to_string());
    store
        .replace(
            &id,
            RuntimeSnapshot {
                server_version: "test".to_string(),
                protocol: 1,
                workspaces: Vec::new(),
                tabs: Vec::new(),
                panes: vec![pane("p1", false)],
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
                protocol_warning: None,
            },
        )
        .expect("runtime 剛登記，set_connection 不應該失敗");
    store
}

/// 一個綁定用的 workspace／tab／pane 三件組：`label` 與 workspace id 相同（`resolve_auto`
/// 用 workspace 的 `label` 比對 `BindingSpec.workspace`），pane 的 agent 狀態由呼叫端指定。
fn bound_pane(
    workspace: &str,
    tab: &str,
    pane_id: &str,
    status: AgentStatus,
) -> (Workspace, Tab, Pane) {
    (
        Workspace {
            id: WorkspaceId::new(workspace),
            label: Some(workspace.to_string()),
            number: 1,
            agent_status: AgentStatus::Idle,
            focused: false,
        },
        Tab {
            id: TabId::new(tab),
            workspace_id: WorkspaceId::new(workspace),
            number: 1,
            agent_status: AgentStatus::Idle,
            focused: false,
        },
        Pane {
            id: PaneId::new(pane_id),
            workspace_id: WorkspaceId::new(workspace),
            tab_id: TabId::new(tab),
            agent: None,
            agent_status: status,
            title: None,
            cwd: None,
            label: None,
            focused: false,
            exited: false,
            updated_at: SystemTime::UNIX_EPOCH,
        },
    )
}

/// runtime `win` 已登記、`connected`，帶入 `entries`（`(workspace, tab, pane_id)`）描述的每組
/// workspace／tab／pane（Scenario C／D：`resolve_auto` 需要真的 workspace `label` 才能自動解析
/// binding，不能只灌 pane，同 `connected_store` 的用途但支援多筆、可控 workspace 標籤）。pane
/// 一律以 `AgentStatus::Idle` 起始，測試再用 [`set_agent_status`] 逐筆送 `AgentStatusChanged`
/// 模擬「pane 狀態變化」。
fn connected_bound_store(entries: &[(&str, &str, &str)]) -> RuntimeStore {
    let id = RuntimeId::new(RUNTIME);
    let mut store = RuntimeStore::new();
    store.register(id.clone(), "herdr".to_string(), "test".to_string());

    let mut workspaces = Vec::new();
    let mut tabs = Vec::new();
    let mut panes = Vec::new();
    for (workspace, tab, pane_id) in entries {
        let (w, t, p) = bound_pane(workspace, tab, pane_id, AgentStatus::Idle);
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
                protocol_warning: None,
            },
        )
        .expect("runtime 剛登記，set_connection 不應該失敗");
    store
}

/// 透過 `StoreHandle::apply` 灌入一筆 `AgentStatusChanged`（Scenario C／D「以 StoreHandle 灌入
/// runtime……pane 狀態變化」）；固定帶一個 agent 名稱，狀態推導只看 `agent_status`，名稱本身
/// 不影響本測試斷言。
fn set_agent_status(handle: &StoreHandle, pane_id: &str, status: AgentStatus) {
    handle
        .apply(
            &RuntimeId::new(RUNTIME),
            RuntimeEvent::AgentStatusChanged {
                pane_id: PaneId::new(pane_id),
                status,
                title: None,
                agent: Some("claude".to_string()),
            },
            SystemTime::now(),
        )
        .expect("pane 已存在於 replace 灌入的快照中，apply 不應該失敗");
}

fn pid() -> ProjectId {
    ProjectId::new("p")
}

fn tid() -> TaskId {
    TaskId::new("t1")
}

fn wid() -> WorkstreamId {
    WorkstreamId::new("be")
}

fn stage_of(handle: &StoreHandle) -> String {
    handle.with_domain(|d| d.progress[&pid()][&tid()].stage.clone())
}

fn mark_of(handle: &StoreHandle) -> Mark {
    handle.with_domain(|d| d.progress[&pid()][&tid()].mark)
}

fn override_of(handle: &StoreHandle) -> Option<Override> {
    handle.with_domain(|d| d.overrides.get(&pid()).and_then(|m| m.get(&wid())).cloned())
}

/// 組一份帶 project `p` 的 `StoreHandle`（domain 已含 `sample_project` 的初始進度）、接上
/// 真正寫入服務、且已經起了一個真的 `spawn_projector` 的 `AppState`（Codex fix round 1
/// finding 2：成功案例要驗真投影，不能只看 `with_domain`）；`runtime_store` 由呼叫端決定
/// （覆蓋測試需要已連線的 pane）。投影任務是 detached 背景 task，測試結束時隨
/// `#[tokio::test]` 的 runtime 一起收掉，不需要另外停。
fn build(runtime_store: RuntimeStore, path: PathBuf) -> (StoreHandle, AppState) {
    build_with_projects(vec![sample_project()], runtime_store, path)
}

/// 同 [`build`]，但 Domain 狀態帶入任意一組 project（task 4.3：Scenario C／D 需要 `sample_project`
/// 沒有的 binding 設定與多 workstream 結構，不能沿用固定的 `sample_project`）。
fn build_with_projects(
    projects: Vec<ProjectDef>,
    runtime_store: RuntimeStore,
    path: PathBuf,
) -> (StoreHandle, AppState) {
    let handle = StoreHandle::new_with_domain(runtime_store, DomainState::from_projects(projects));
    let service = ProgressService::new(handle.clone(), path);
    let _projector = spawn_projector(handle.clone());
    let state = AppState {
        state: handle.subscribe(),
        progress: Some(service),
        port: Arc::new(AtomicU16::new(0)),
        runtimes: Arc::new(HashMap::new()),
    };
    (handle, state)
}

/// 送一個請求進 `router`，回傳狀態碼、（若有本體）解析出的 JSON、與 `Content-Type` 標頭
/// （沒有本體回 `Value::Null`；沒有標頭回 `None`）。固定帶 `Host: 127.0.0.1:0`——`build()`
/// 組出的 `AppState.port` 初值是 0（task 4.2 的來源檢查 middleware 比對實際監聽埠；design
/// D6「測試用 port 0 也正確」），沒有這個標頭寫入端點會被來源檢查擋成 403，跟這裡多數測試
/// 想驗的行為無關。需要另外的 `Host`／`Origin` 才用 [`send_with_headers`]。
async fn send(
    router: &axum::Router,
    method: &str,
    uri: &str,
    body: Option<&str>,
) -> (StatusCode, Value, Option<String>) {
    send_with_headers(router, method, uri, body, &[("host", "127.0.0.1:0")]).await
}

/// 同 [`send`]，但自己指定完整的標頭清單（4.2 的來源檢查測試需要送出跟預設不同、甚至不合法
/// 的 `Host`／`Origin`，所以不跟 [`send`] 共用預設值，呼叫端要自己把想要的 `Host` 一起帶）。
async fn send_with_headers(
    router: &axum::Router,
    method: &str,
    uri: &str,
    body: Option<&str>,
    headers: &[(&str, &str)],
) -> (StatusCode, Value, Option<String>) {
    let mut builder = Request::builder().method(method).uri(uri);
    for (name, value) in headers {
        builder = builder.header(*name, *value);
    }
    let request = match body {
        Some(body) => builder.body(Body::from(body.to_string())),
        None => builder.body(Body::empty()),
    }
    .expect("request 建構不應該失敗");
    let response = router
        .clone()
        .oneshot(request)
        .await
        .expect("oneshot 呼叫不應該失敗");
    let content_type = response
        .headers()
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
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
    (status, value, content_type)
}

/// 錯誤回應共通斷言（400／404／409／500，task 4.1 原文「錯誤本體 `{"error": ...}`」；
/// Codex fix round 1 finding 3：所有這幾種狀態碼都要有 `Content-Type: application/json`
/// 與非空的 `error` 欄位——包含先前漏掉的 404）。
fn assert_error_response(
    status: StatusCode,
    body: &Value,
    content_type: &Option<String>,
    expected: StatusCode,
) {
    assert_eq!(status, expected);
    assert_eq!(
        content_type.as_deref(),
        Some("application/json"),
        "錯誤回應應該帶 Content-Type: application/json，實際：{content_type:?}"
    );
    assert!(
        body["error"].as_str().is_some_and(|s| !s.is_empty()),
        "本體應該有非空的 error 欄位，實際：{body:?}"
    );
}

/// 讀 `/api/state` 的 version（不是完整輪詢，只取當下一次）。
async fn state_version(router: &axum::Router) -> u64 {
    let (status, body, _content_type) = send(router, "GET", "/api/state", None).await;
    assert_eq!(status, StatusCode::OK, "/api/state 應該回 200");
    body["version"].as_u64().expect("version 應該是數字")
}

/// 輪詢 `/api/state` 直到 version 大於 `from`，回傳那一份投影（Codex fix round 1
/// finding 2：成功案例要驗證真投影跟上了寫入，不是只看記憶體）。逾時直接讓斷言失敗並印出
/// 最後一次讀到的 version，方便除錯。
async fn wait_for_version_increase(router: &axum::Router, from: u64) -> Value {
    let deadline = Instant::now() + WAIT_TIMEOUT;
    loop {
        let (status, body, _content_type) = send(router, "GET", "/api/state", None).await;
        assert_eq!(status, StatusCode::OK, "/api/state 應該回 200");
        let version = body["version"].as_u64().expect("version 應該是數字");
        if version > from {
            return body;
        }
        assert!(
            Instant::now() < deadline,
            "等待投影 version 超過 {from} 逾時，最後一次讀到的 version 是 {version}"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

fn find_project<'a>(state: &'a Value, project_id: &str) -> Option<&'a Value> {
    state["projects"]
        .as_array()?
        .iter()
        .find(|p| p["id"] == project_id)
}

fn find_task<'a>(state: &'a Value, project_id: &str, task_id: &str) -> Option<&'a Value> {
    find_project(state, project_id)?["tasks"]
        .as_array()?
        .iter()
        .find(|t| t["id"] == task_id)
}

fn find_workstream<'a>(
    state: &'a Value,
    project_id: &str,
    workstream_id: &str,
) -> Option<&'a Value> {
    find_project(state, project_id)?["workstreams"]
        .as_array()?
        .iter()
        .find(|w| w["id"] == workstream_id)
}

/// 讀回磁碟上真正的狀態檔並解析成 JSON（不是記憶體快照——Codex fix round 1 finding 2）。
fn read_state_file(path: &Path) -> Value {
    let raw = fs::read_to_string(path).expect("狀態檔應該存在");
    serde_json::from_str(&raw).expect("狀態檔應該是合法 JSON")
}

// ---------------------------------------------------------------------------
// 進度寫入端點（spec `pipeline-progress`「進度寫入端點」）
// ---------------------------------------------------------------------------

#[tokio::test]
async fn progress_advance_succeeds_204() {
    let dir = TempDir::new("advance");
    let path = dir.path().join("state.json");
    let (handle, state) = build(RuntimeStore::new(), path.clone());
    let router = http::router(state);

    let before = state_version(&router).await;

    let (status, _, _) = send(&router, "POST", "/api/projects/p/tasks/t1/advance", None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    // 真投影：等 version 真的遞增，不是只看 with_domain 的記憶體快照。
    let projected = wait_for_version_increase(&router, before).await;
    let task = find_task(&projected, "p", "t1").expect("投影應該有 task t1");
    assert_eq!(task["stage"], "Build", "投影中 t1 應該在下一站");
    assert_eq!(task["mark"], "none");

    assert_eq!(stage_of(&handle), "Build", "記憶體中的進度也應該一致");

    // 讀回真實狀態檔，不只信記憶體。
    let on_disk = read_state_file(&path);
    assert_eq!(on_disk["projects"]["p"]["tasks"]["t1"]["stage"], "Build");
    assert_eq!(on_disk["projects"]["p"]["tasks"]["t1"]["mark"], "none");
}

#[tokio::test]
async fn progress_rejected_409() {
    let dir = TempDir::new("rejected");
    let (handle, state) = build(RuntimeStore::new(), dir.path().join("state.json"));
    let router = http::router(state);

    let (status, _, _) = send(&router, "POST", "/api/projects/p/tasks/t1/advance", None).await;
    assert_eq!(
        status,
        StatusCode::NO_CONTENT,
        "第一次推進應該成功，t1 到 Build"
    );

    // `t1` 已經在最後一站（`Build`），再推進一次應該被拒絕。
    let (status, body, content_type) =
        send(&router, "POST", "/api/projects/p/tasks/t1/advance", None).await;

    assert_error_response(status, &body, &content_type, StatusCode::CONFLICT);
    assert_eq!(body["error"], "已是最後一個 Stage");
    assert_eq!(stage_of(&handle), "Build", "被拒絕不應改變投影中的進度");
}

#[tokio::test]
async fn progress_unknown_task_404() {
    let dir = TempDir::new("unknown-task");
    let (_handle, state) = build(RuntimeStore::new(), dir.path().join("state.json"));
    let router = http::router(state);

    let (status, body, content_type) =
        send(&router, "POST", "/api/projects/p/tasks/nope/complete", None).await;

    assert_error_response(status, &body, &content_type, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn progress_unknown_project_404() {
    let dir = TempDir::new("unknown-project");
    let (_handle, state) = build(RuntimeStore::new(), dir.path().join("state.json"));
    let router = http::router(state);

    let (status, body, content_type) = send(
        &router,
        "POST",
        "/api/projects/nope/tasks/t1/complete",
        None,
    )
    .await;

    assert_error_response(status, &body, &content_type, StatusCode::NOT_FOUND);
}

/// `<op>` 不是 `advance`／`complete`／`fail`／`clear` 四者之一：路由層就回 404，不進到寫入
/// 服務（design D6）。
#[tokio::test]
async fn progress_unknown_op_404() {
    let dir = TempDir::new("unknown-op");
    let (handle, state) = build(RuntimeStore::new(), dir.path().join("state.json"));
    let router = http::router(state);

    let (status, body, content_type) =
        send(&router, "POST", "/api/projects/p/tasks/t1/bogus", None).await;

    assert_error_response(status, &body, &content_type, StatusCode::NOT_FOUND);
    assert_eq!(stage_of(&handle), "Spec", "不合法的操作不該動到進度");
}

#[tokio::test]
async fn progress_write_failure_500() {
    let dir = TempDir::new("write-failure");
    let path = dir.path().join("state.json");
    // 目標路徑是既有目錄：`.tmp` 寫得出來，但 rename 取代目錄會失敗（同
    // `cockpit/tests/progress_service.rs::write_failure_keeps_memory` 的做法）。
    fs::create_dir_all(&path).expect("在狀態檔路徑建立目錄");
    let (handle, state) = build(RuntimeStore::new(), path);
    let router = http::router(state);

    let (status, body, content_type) =
        send(&router, "POST", "/api/projects/p/tasks/t1/complete", None).await;

    assert_error_response(
        status,
        &body,
        &content_type,
        StatusCode::INTERNAL_SERVER_ERROR,
    );
    assert_eq!(mark_of(&handle), Mark::None, "落檔失敗記憶體不應生效");
}

// ---------------------------------------------------------------------------
// 綁定覆蓋端點（spec `pipeline-progress`「綁定覆蓋端點」）
// ---------------------------------------------------------------------------

#[tokio::test]
async fn override_set_and_clear_204() {
    let dir = TempDir::new("override-set");
    let path = dir.path().join("state.json");
    let (handle, state) = build(connected_store(), path.clone());
    let router = http::router(state);

    let before_set = state_version(&router).await;

    let (status, _, _) = send(
        &router,
        "PUT",
        "/api/projects/p/workstreams/be/override",
        Some(r#"{"runtime":"win","pane_id":"p1"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let projected = wait_for_version_increase(&router, before_set).await;
    let workstream = find_workstream(&projected, "p", "be").expect("投影應該有 workstream be");
    assert_eq!(workstream["binding"]["state"], "bound");
    assert_eq!(workstream["binding"]["source"], "override");
    assert_eq!(workstream["binding"]["runtime"], "win");
    assert_eq!(workstream["binding"]["pane_id"], "p1");

    assert_eq!(
        override_of(&handle),
        Some(Override {
            runtime: RuntimeId::new("win"),
            pane_id: PaneId::new("p1"),
        })
    );

    let on_disk = read_state_file(&path);
    assert_eq!(
        on_disk["projects"]["p"]["overrides"]["be"]["runtime"],
        "win"
    );
    assert_eq!(on_disk["projects"]["p"]["overrides"]["be"]["pane_id"], "p1");

    // 取消覆蓋：一樣走真投影 + 真狀態檔，不是只看記憶體。
    let before_clear = state_version(&router).await;

    let (status, _, _) = send(
        &router,
        "DELETE",
        "/api/projects/p/workstreams/be/override",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let projected = wait_for_version_increase(&router, before_clear).await;
    let workstream = find_workstream(&projected, "p", "be").expect("投影應該有 workstream be");
    // `be` 在 `sample_project` 沒有設定檔 `binding`，取消覆蓋後應該回到「沒有 binding、也
    // 沒有覆蓋」（`ProjectedBinding::None`），不是只斷言「不是 override」。
    assert_eq!(
        workstream["binding"]["state"], "none",
        "取消後應該回到沒有 binding 也沒有覆蓋的狀態，實際：{:?}",
        workstream["binding"]
    );

    assert_eq!(override_of(&handle), None, "取消後不應該還有覆蓋");

    let on_disk = read_state_file(&path);
    let overrides = on_disk["projects"]["p"]["overrides"]
        .as_object()
        .expect("overrides 應該是物件（即使是空的）");
    assert!(
        !overrides.contains_key("be"),
        "取消後狀態檔不應該還留著 be 的覆蓋，實際：{overrides:?}"
    );
}

#[tokio::test]
async fn override_unknown_workstream_404() {
    let dir = TempDir::new("override-unknown-ws");
    let (_handle, state) = build(connected_store(), dir.path().join("state.json"));
    let router = http::router(state);

    let (status, body, content_type) = send(
        &router,
        "PUT",
        "/api/projects/p/workstreams/nope/override",
        Some(r#"{"runtime":"win","pane_id":"p1"}"#),
    )
    .await;

    assert_error_response(status, &body, &content_type, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn override_rejected_409() {
    let dir = TempDir::new("override-rejected");
    // `win` 從未登記：`validate_override` 回 `RuntimeNotRegistered`。
    let (handle, state) = build(RuntimeStore::new(), dir.path().join("state.json"));
    let router = http::router(state);

    let (status, body, content_type) = send(
        &router,
        "PUT",
        "/api/projects/p/workstreams/be/override",
        Some(r#"{"runtime":"win","pane_id":"p1"}"#),
    )
    .await;

    assert_error_response(status, &body, &content_type, StatusCode::CONFLICT);
    assert_eq!(body["error"], "runtime 未登記");
    assert_eq!(override_of(&handle), None, "被拒絕不應該留下覆蓋");
}

#[tokio::test]
async fn override_body_missing_field_400() {
    let dir = TempDir::new("override-bad-body");
    let (handle, state) = build(connected_store(), dir.path().join("state.json"));
    let router = http::router(state);

    let (status, body, content_type) = send(
        &router,
        "PUT",
        "/api/projects/p/workstreams/be/override",
        Some(r#"{"runtime":"win"}"#),
    )
    .await;

    assert_error_response(status, &body, &content_type, StatusCode::BAD_REQUEST);
    assert_eq!(override_of(&handle), None);
}

#[tokio::test]
async fn override_body_not_json_400() {
    let dir = TempDir::new("override-not-json");
    let (handle, state) = build(connected_store(), dir.path().join("state.json"));
    let router = http::router(state);

    let (status, body, content_type) = send(
        &router,
        "PUT",
        "/api/projects/p/workstreams/be/override",
        Some("not json"),
    )
    .await;

    assert_error_response(status, &body, &content_type, StatusCode::BAD_REQUEST);
    assert_eq!(override_of(&handle), None);
}

#[tokio::test]
async fn override_write_failure_500() {
    let dir = TempDir::new("override-write-failure");
    let path = dir.path().join("state.json");
    fs::create_dir_all(&path).expect("在狀態檔路徑建立目錄");
    let (handle, state) = build(connected_store(), path);
    let router = http::router(state);

    let (status, body, content_type) = send(
        &router,
        "PUT",
        "/api/projects/p/workstreams/be/override",
        Some(r#"{"runtime":"win","pane_id":"p1"}"#),
    )
    .await;

    assert_error_response(
        status,
        &body,
        &content_type,
        StatusCode::INTERNAL_SERVER_ERROR,
    );
    assert_eq!(override_of(&handle), None, "落檔失敗記憶體不應生效");
}

#[tokio::test]
async fn clear_nonexistent_override_204() {
    let dir = TempDir::new("clear-nonexistent");
    let (handle, state) = build(RuntimeStore::new(), dir.path().join("state.json"));
    let router = http::router(state);

    let (status, _, _) = send(
        &router,
        "DELETE",
        "/api/projects/p/workstreams/be/override",
        None,
    )
    .await;

    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(override_of(&handle), None);
}

// ---------------------------------------------------------------------------
// `cockpit-dashboard`「寫入端點不接受 GET」
// ---------------------------------------------------------------------------

#[tokio::test]
async fn get_progress_endpoint_is_405() {
    let dir = TempDir::new("get-progress-405");
    let (handle, state) = build(RuntimeStore::new(), dir.path().join("state.json"));
    let router = http::router(state);

    let (status, _, _) = send(&router, "GET", "/api/projects/p/tasks/t1/advance", None).await;

    assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(stage_of(&handle), "Spec", "GET 不該改變進度");
}

#[tokio::test]
async fn get_override_endpoint_is_405() {
    let dir = TempDir::new("get-override-405");
    let (handle, state) = build(connected_store(), dir.path().join("state.json"));
    let router = http::router(state);

    let (status, _, _) = send(
        &router,
        "GET",
        "/api/projects/p/workstreams/be/override",
        None,
    )
    .await;

    assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(override_of(&handle), None, "GET 不該改變覆蓋");
}

// ---------------------------------------------------------------------------
// 沒有任何 project 時（`AppState::progress` 為 `None`）：寫入端點一律 404，本體仍是
// `{"error": ...}`（Codex fix round 1 finding 3）。
// ---------------------------------------------------------------------------

#[tokio::test]
async fn no_progress_service_returns_404_for_writes() {
    let handle = StoreHandle::new(RuntimeStore::new());
    let state = AppState::new(handle.subscribe());
    let router = http::router(state);

    let (status, body, content_type) =
        send(&router, "POST", "/api/projects/p/tasks/t1/advance", None).await;
    assert_error_response(status, &body, &content_type, StatusCode::NOT_FOUND);

    let (status, body, content_type) = send(
        &router,
        "PUT",
        "/api/projects/p/workstreams/be/override",
        Some(r#"{"runtime":"win","pane_id":"p1"}"#),
    )
    .await;
    assert_error_response(status, &body, &content_type, StatusCode::NOT_FOUND);

    let (status, body, content_type) = send(
        &router,
        "DELETE",
        "/api/projects/p/workstreams/be/override",
        None,
    )
    .await;
    assert_error_response(status, &body, &content_type, StatusCode::NOT_FOUND);
}

// ---------------------------------------------------------------------------
// 來源檢查 middleware（spec `pipeline-progress`「寫入端點只接受本機同源請求」；design D6：
// 只套在寫入路由，`GET` 落回 axum 的 405、`/ws` 不套——見 `cockpit::source_check` 模組文件）
// ---------------------------------------------------------------------------

/// spec 情境「跨站表單被拒」：`Host` 合法但 `Origin` 是別的網站。
#[tokio::test]
async fn cross_site_origin_rejected() {
    let dir = TempDir::new("cross-site-origin");
    let (handle, state) = build(RuntimeStore::new(), dir.path().join("state.json"));
    let router = http::router(state);

    let (status, body, content_type) = send_with_headers(
        &router,
        "POST",
        "/api/projects/p/tasks/t1/advance",
        None,
        &[("host", "127.0.0.1:0"), ("origin", "https://evil.example")],
    )
    .await;

    assert_error_response(status, &body, &content_type, StatusCode::FORBIDDEN);
    assert_eq!(stage_of(&handle), "Spec", "跨站請求被拒不應改變進度");
}

/// spec 情境「DNS rebinding 被拒」：`Host` 本身就不是本機位址（沒有帶 `Origin`）。
#[tokio::test]
async fn dns_rebinding_host_rejected() {
    let dir = TempDir::new("dns-rebinding");
    let (handle, state) = build(RuntimeStore::new(), dir.path().join("state.json"));
    let router = http::router(state);

    let (status, body, content_type) = send_with_headers(
        &router,
        "POST",
        "/api/projects/p/tasks/t1/advance",
        None,
        &[("host", "evil.example:0")],
    )
    .await;

    assert_error_response(status, &body, &content_type, StatusCode::FORBIDDEN);
    assert_eq!(
        stage_of(&handle),
        "Spec",
        "DNS rebinding 請求被拒不應改變進度"
    );
}

/// spec 情境「自家頁面與命令列可用」：`Host`＋相符的 `Origin`（自家頁面），以及只有 `Host`
/// 沒有 `Origin`（命令列工具）都要被處理——用兩種不同的寫入操作各驗一次，避免第二次送出被
/// 第一次的結果（`t1` 已推進）干擾判讀。
#[tokio::test]
async fn same_origin_and_cli_accepted() {
    let dir = TempDir::new("same-origin-and-cli");
    let (handle, state) = build(connected_store(), dir.path().join("state.json"));
    let router = http::router(state);

    let (status, _, _) = send_with_headers(
        &router,
        "POST",
        "/api/projects/p/tasks/t1/advance",
        None,
        &[("host", "127.0.0.1:0"), ("origin", "http://127.0.0.1:0")],
    )
    .await;
    assert_eq!(
        status,
        StatusCode::NO_CONTENT,
        "自家頁面（Host 與相符的 Origin）應該被接受"
    );
    assert_eq!(stage_of(&handle), "Build");

    let (status, _, _) = send_with_headers(
        &router,
        "PUT",
        "/api/projects/p/workstreams/be/override",
        Some(r#"{"runtime":"win","pane_id":"p1"}"#),
        &[("host", "localhost:0")],
    )
    .await;
    assert_eq!(
        status,
        StatusCode::NO_CONTENT,
        "命令列工具（只有 Host，沒有 Origin）應該被接受"
    );
    assert_eq!(
        override_of(&handle),
        Some(Override {
            runtime: RuntimeId::new("win"),
            pane_id: PaneId::new("p1"),
        })
    );
}

/// design D6：`Host` 比對的是 `AppState.port` 目前的值，不是建構時的常數——`port = 0`
/// （`build()` 的初值，對應真正 `listen = "127.0.0.1:0"` 綁定前的狀態）本身要能通過；埠透過
/// 同一個 `Arc<AtomicU16>` 回填成真正監聽埠後（`cockpit::app::run_with_shutdown` 的做法），
/// 同一個已經組好的 router 要立刻認新埠，舊埠反而變成不合法。
#[tokio::test]
async fn port_zero_uses_actual_port() {
    let dir = TempDir::new("port-zero-actual-port");
    let (handle, state) = build(RuntimeStore::new(), dir.path().join("state.json"));
    let port = Arc::clone(&state.port);
    let router = http::router(state);

    let (status, _, _) = send_with_headers(
        &router,
        "POST",
        "/api/projects/p/tasks/t1/advance",
        None,
        &[("host", "127.0.0.1:0")],
    )
    .await;
    assert_eq!(
        status,
        StatusCode::NO_CONTENT,
        "port 0 時 Host 帶 0 應該被接受"
    );
    assert_eq!(stage_of(&handle), "Build");

    // 模擬 bind 完成後的回填：同一個 Arc，router 不用重建就要認新值。
    port.store(54321, Ordering::Relaxed);

    let (status, body, content_type) = send_with_headers(
        &router,
        "POST",
        "/api/projects/p/tasks/t1/complete",
        None,
        &[("host", "127.0.0.1:0")],
    )
    .await;
    assert_error_response(status, &body, &content_type, StatusCode::FORBIDDEN);
    assert_eq!(mark_of(&handle), Mark::None, "回填後舊埠應該被拒，進度不變");

    let (status, _, _) = send_with_headers(
        &router,
        "POST",
        "/api/projects/p/tasks/t1/complete",
        None,
        &[("host", "127.0.0.1:54321")],
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "回填後新埠應該被接受");
    assert_eq!(mark_of(&handle), Mark::Completed);
}

/// Fix round 1（Codex finding）：重複的 `Host` 標頭（HTTP 允許同名標頭出現多次）不能只看
/// `HeaderMap::get` 的第一個值——那樣第二個值完全不受檢查，來源判定產生歧義。這裡兩個值都是
/// 合法的 loopback 寫法（只是埠不同），驗的是「重複本身」就該被拒，不是「其中一個值不合法」。
/// 用 `send_with_headers` 對同一個鍵送兩次，`Request::builder().header()`（`http` crate）是
/// `try_append`，不是覆蓋，實測確認 `axum::Router::oneshot` 收到的請求裡兩個 `Host` 值都還在
/// （`source_check::exactly_one` 的單元測試 `exactly_one_duplicate_is_err` 已經直接對著
/// `HeaderMap` 驗證這件事；這裡再從 HTTP 層整合驗一次，確認沒有在更外層被合併掉）。
#[tokio::test]
async fn duplicate_host_rejected() {
    let dir = TempDir::new("duplicate-host");
    let (handle, state) = build(RuntimeStore::new(), dir.path().join("state.json"));
    let router = http::router(state);

    let (status, body, content_type) = send_with_headers(
        &router,
        "POST",
        "/api/projects/p/tasks/t1/advance",
        None,
        &[("host", "127.0.0.1:0"), ("host", "evil.example:0")],
    )
    .await;

    assert_error_response(status, &body, &content_type, StatusCode::FORBIDDEN);
    assert_eq!(stage_of(&handle), "Spec", "重複 Host 被拒不應改變進度");
}

/// 同上，但重複的是 `Origin`；`Host` 本身合法、單一。
#[tokio::test]
async fn duplicate_origin_rejected() {
    let dir = TempDir::new("duplicate-origin");
    let (handle, state) = build(RuntimeStore::new(), dir.path().join("state.json"));
    let router = http::router(state);

    let (status, body, content_type) = send_with_headers(
        &router,
        "POST",
        "/api/projects/p/tasks/t1/advance",
        None,
        &[
            ("host", "127.0.0.1:0"),
            ("origin", "http://127.0.0.1:0"),
            ("origin", "https://evil.example"),
        ],
    )
    .await;

    assert_error_response(status, &body, &content_type, StatusCode::FORBIDDEN);
    assert_eq!(stage_of(&handle), "Spec", "重複 Origin 被拒不應改變進度");
}

// ---------------------------------------------------------------------------
// Scenario C／D（spec `pipeline-domain`「StageStatus 推導」、`state-projection`「Project
// 投影」；task 4.3）：真 `router`＋真投影＋暫存狀態檔，runtime 狀態一律以 `StoreHandle` 灌入
// （`connected_bound_store` 建初始快照、[`set_agent_status`] 灌 pane 狀態變化），觀察只經
// `/api/state` 與寫入端點——不讀 `with_domain`。
// ---------------------------------------------------------------------------

/// Scenario C 用的 project：一條 workstream `be` 以設定檔 binding（不是覆蓋）自動解析到
/// workspace `wJ` 底下唯一的 pane，task `A` 一開始就在 `Implement`、無標記、無依賴。
fn scenario_c_project() -> ProjectDef {
    ProjectDef {
        id: ProjectId::new("p"),
        name: "p".to_string(),
        stages: vec!["Spec".to_string(), "Implement".to_string()],
        workstreams: vec![WorkstreamDef {
            id: WorkstreamId::new("be"),
            name: "be".to_string(),
            binding: Some(BindingSpec {
                runtime: RuntimeId::new(RUNTIME),
                workspace: "wJ".to_string(),
                pane_label: None,
                cwd: None,
                agent: None,
            }),
        }],
        tasks: vec![TaskDef {
            id: TaskId::new("A"),
            title: "A".to_string(),
            workstream: WorkstreamId::new("be"),
            stage: "Implement".to_string(),
            depends_on: Vec::new(),
        }],
    }
}

/// spec `pipeline-domain` Scenario C：task 在 `Implement`、綁定 pane 的 agent 狀態變成
/// `working` 後，投影中的 `status` 應為 `running`、`stage` 仍是 `Implement`（spec
/// `state-projection` Scenario C 的 JSON 欄位）。
#[tokio::test]
async fn scenario_c_pipeline_projection() {
    let dir = TempDir::new("scenario-c");
    let runtime_store = connected_bound_store(&[("wJ", "wJ:t1", "wJ:p1")]);
    let (handle, state) = build_with_projects(
        vec![scenario_c_project()],
        runtime_store,
        dir.path().join("state.json"),
    );
    let router = http::router(state);

    // 起始狀態：binding 已 bound，但 pane 還是 idle，task 應為 ready（不是 running）。
    let initial = state_version(&router).await;
    let projected = send(&router, "GET", "/api/state", None).await.1;
    let task = find_task(&projected, "p", "A").expect("投影應該有 task A");
    assert_eq!(task["status"], "ready", "pane 還是 idle 時不應該是 running");

    set_agent_status(&handle, "wJ:p1", AgentStatus::Working);

    let projected = wait_for_version_increase(&router, initial).await;
    let task = find_task(&projected, "p", "A").expect("投影應該有 task A");
    assert_eq!(
        task["stage"], "Implement",
        "pane working 改變的是 status，不是 stage"
    );
    assert_eq!(task["status"], "running");

    let workstream = find_workstream(&projected, "p", "be").expect("投影應該有 workstream be");
    assert_eq!(
        workstream["binding"],
        serde_json::json!({
            "state": "bound",
            "runtime": RUNTIME,
            "pane_id": "wJ:p1",
            "source": "auto",
            "agent": "claude",
            "agent_status": "working",
        }),
        "應符合 spec state-projection Scenario C 的 binding JSON 形狀"
    );
}

/// Scenario D 用的 project：三條各自綁定不同 pane 的 workstream，各帶一個 task 在不同 stage；
/// `stages` 讓每個 task 目前所在的 stage 都不是最後一個，之後才能挑其中一個推進。
fn scenario_d_project() -> ProjectDef {
    fn workstream(id: &str, workspace: &str) -> WorkstreamDef {
        WorkstreamDef {
            id: WorkstreamId::new(id),
            name: id.to_string(),
            binding: Some(BindingSpec {
                runtime: RuntimeId::new(RUNTIME),
                workspace: workspace.to_string(),
                pane_label: None,
                cwd: None,
                agent: None,
            }),
        }
    }

    fn task(id: &str, workstream: &str, stage: &str) -> TaskDef {
        TaskDef {
            id: TaskId::new(id),
            title: id.to_string(),
            workstream: WorkstreamId::new(workstream),
            stage: stage.to_string(),
            depends_on: Vec::new(),
        }
    }

    ProjectDef {
        id: ProjectId::new("p"),
        name: "p".to_string(),
        stages: vec![
            "Plan".to_string(),
            "Implement".to_string(),
            "Test".to_string(),
            "Done".to_string(),
        ],
        workstreams: vec![
            workstream("backend", "wBackend"),
            workstream("frontend", "wFrontend"),
            workstream("tests", "wTests"),
        ],
        tasks: vec![
            task("be1", "backend", "Implement"),
            task("fe1", "frontend", "Plan"),
            task("qa1", "tests", "Test"),
        ],
    }
}

/// spec `pipeline-domain` Scenario D：三條 workstream 各自綁定的 pane 同時變成 `working`，
/// 三個 task 應同時為 `running`、各自的 `stage` 維持設定的三個不同值；推進其中一個之後只有
/// 該 task 的 `stage` 改變，其餘兩個不受影響。
#[tokio::test]
async fn scenario_d_parallel_collaboration() {
    let dir = TempDir::new("scenario-d");
    let runtime_store = connected_bound_store(&[
        ("wBackend", "wBackend:t1", "wBackend:p1"),
        ("wFrontend", "wFrontend:t1", "wFrontend:p1"),
        ("wTests", "wTests:t1", "wTests:p1"),
    ]);
    let (handle, state) = build_with_projects(
        vec![scenario_d_project()],
        runtime_store,
        dir.path().join("state.json"),
    );
    let router = http::router(state);

    let before_working = state_version(&router).await;
    set_agent_status(&handle, "wBackend:p1", AgentStatus::Working);
    set_agent_status(&handle, "wFrontend:p1", AgentStatus::Working);
    set_agent_status(&handle, "wTests:p1", AgentStatus::Working);

    let projected = wait_for_version_increase(&router, before_working).await;
    let be1 = find_task(&projected, "p", "be1").expect("投影應該有 task be1");
    let fe1 = find_task(&projected, "p", "fe1").expect("投影應該有 task fe1");
    let qa1 = find_task(&projected, "p", "qa1").expect("投影應該有 task qa1");
    assert_eq!(be1["status"], "running");
    assert_eq!(fe1["status"], "running");
    assert_eq!(qa1["status"], "running");
    assert_eq!(be1["stage"], "Implement");
    assert_eq!(fe1["stage"], "Plan");
    assert_eq!(qa1["stage"], "Test");

    // 推進 fe1（在 Plan，非最後一站），其餘兩個 task 的 stage／status 不應受影響。
    let before_advance = state_version(&router).await;
    let (status, _, _) = send(&router, "POST", "/api/projects/p/tasks/fe1/advance", None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let projected = wait_for_version_increase(&router, before_advance).await;
    let be1 = find_task(&projected, "p", "be1").expect("投影應該有 task be1");
    let fe1 = find_task(&projected, "p", "fe1").expect("投影應該有 task fe1");
    let qa1 = find_task(&projected, "p", "qa1").expect("投影應該有 task qa1");
    assert_eq!(fe1["stage"], "Implement", "fe1 應該推進到下一站");
    assert_eq!(fe1["status"], "running", "fe1 的 pane 仍是 working");
    assert_eq!(
        be1["stage"], "Implement",
        "be1 的 stage 不應被 fe1 的推進影響"
    );
    assert_eq!(
        be1["status"], "running",
        "be1 的 status 不應被 fe1 的推進影響"
    );
    assert_eq!(qa1["stage"], "Test", "qa1 的 stage 不應被 fe1 的推進影響");
    assert_eq!(
        qa1["status"], "running",
        "qa1 的 status 不應被 fe1 的推進影響"
    );
}
