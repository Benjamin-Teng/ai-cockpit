//! HTTP 路由與內嵌靜態資源（spec `cockpit-dashboard`「路由與內嵌資源」「WebSocket 推送
//! 整張圖」；design D13、D9）。
//!
//! 所有靜態內容用 `include_str!`／`include_bytes!` 從 `cockpit/assets/` 內嵌進執行檔——不用
//! `ServeDir`、不在執行期讀檔案系統，滿足「單一執行檔複製到別的機器也能跑」（design D13；
//! 3.8 驗）。`bind`／`serve`（真的開 port）留給 3.8 的 `main`；這裡只組 [`Router`]，
//! `/api/state`、`/app/*` 等純 HTTP 路由測試用 `tower::ServiceExt::oneshot` 打，不開 port；
//! `/ws` 的測試（3.4，`cockpit/tests/ws.rs`）需要真正的 TCP 連線，另外自己
//! `TcpListener::bind("127.0.0.1:0")` + `axum::serve`。
//!
//! `/ws`：升級成 WebSocket 後，先送一則目前整份投影，之後只要投影 `version` 遞增
//! （`watch::Receiver::changed()` 觸發）就再送一份完整 JSON——不送增量（design D9；spec
//! 「WebSocket 推送整張圖」）。客戶端送來的任何訊息（文字、二進位、close 以外都忽略；
//! ping／pong 由 axum 自動回應，不會走到這裡）一律不理會；客戶端斷線或送錯只結束這個
//! task，不影響其他連線——每個連線各自 `subscribe()` 一份獨立的 `watch::Receiver`。
//!
//! 寫入端點（task 4.1；spec `pipeline-progress`「進度寫入端點」「綁定覆蓋端點」；design
//! D6）：`POST /api/projects/{project}/tasks/{task}/{op}`、`PUT`／`DELETE
//! /api/projects/{project}/workstreams/{workstream}/override`。`{op}` 在路由層以字串比對
//! [`ProgressOp`] 的四個值，不是其中之一直接回 404，不會進到 [`crate::progress_service`]；
//! project／task／workstream 是否存在則交給 [`ProgressService`] 判斷（[`WriteError`] 映射見
//! [`write_error_response`]）。成功一律 204、不回投影本體——畫面等 `/ws` 推送。這兩個路由額外
//! 掛了 task 4.2 的來源檢查 middleware（[`crate::source_check::source_check`]，Host／Origin），
//! 這裡的處理常式本身完全不管請求從哪裡來——那是 middleware 的事，擋下的請求根本不會進到
//! 這幾個 handler。

use std::sync::Arc;
use std::sync::atomic::AtomicU16;

use axum::Router;
use axum::body::Bytes;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post, put};
use cockpit_core::{
    Override, PaneId, ProgressOp, ProjectId, ProjectedState, RuntimeId, TaskId, WorkstreamId,
};
use serde::{Deserialize, Serialize};
use tokio::sync::watch;

use crate::progress_service::{ProgressService, WriteError};
use crate::source_check::source_check;

/// 路由共用的狀態：訂閱 [`cockpit_core::StoreHandle`] 廣播的投影（design D9），外加寫入服務
/// 與服務實際監聽的埠（design D6；task 4.1）。
///
/// `Clone` 便宜——`watch::Receiver` 本身可以自由複製；`progress` 是 `Option<ProgressService>`
/// 本身內部也是 `Arc`；`port` 是 `Arc<AtomicU16>`。都不需要額外的鎖包一層。
#[derive(Clone)]
pub struct AppState {
    /// 目前投影的訂閱端；`/api/state` 用 `borrow()` 讀現況，`/ws` 每個連線各自
    /// `clone()` 一份自己追（1.6 的 `StoreHandle::subscribe`）。
    pub state: watch::Receiver<Arc<ProjectedState>>,
    /// 進度與覆蓋的寫入服務；沒有任何 project 時是 `None`（design Migration Plan）——這時
    /// 任何 project／task／workstream 引用本來就等於「不存在」，寫入端點統一回 404，不需要
    /// 特別區分「沒有寫入服務」與「project 不存在」。
    pub progress: Option<ProgressService>,
    /// 服務實際監聽的埠（`TcpListener::local_addr()`，design D6）。路由表在監聽埠確定之前就
    /// 已經組好（`cockpit::app::build_components` 早於 `bind`），所以用 `Arc<AtomicU16>`：
    /// `cockpit::app::run` 綁定成功後把真正的埠寫進同一個 `Arc`，所有已經拿到 `AppState`
    /// clone 的請求都會讀到更新後的值。來源檢查 middleware（task 4.2）比對 `Host` 時要用
    /// 這個，不是設定裡寫的埠——`listen = "127.0.0.1:0"` 綁定後真正拿到的埠由作業系統指派。
    pub port: Arc<AtomicU16>,
}

impl AppState {
    /// 只有讀路由（沒有寫入服務）時的建構子：`progress` 固定 `None`、`port` 初值 0——沒有
    /// 寫入端點時 `port` 不會被任何 middleware 讀到。
    pub fn new(state: watch::Receiver<Arc<ProjectedState>>) -> Self {
        Self {
            state,
            progress: None,
            port: Arc::new(AtomicU16::new(0)),
        }
    }
}

/// 組出完整的路由表：`/`、`/app/{file}`、`/manifest.webmanifest`、`/icons/{file}`、
/// `/api/state`、`/ws`、寫入端點；其他路徑落回 axum 預設的 404。
///
/// 兩個寫入路由額外用 `route_layer` 掛 [`source_check`]（task 4.2；design D6）——只套在
/// 這兩條，`/api/state`、`/ws` 等讀路由完全不受影響（見 [`crate::source_check`] 模組文件對
/// `route_layer` 範圍的說明）。
pub fn router(app: AppState) -> Router {
    let source_check_layer = axum::middleware::from_fn_with_state(app.clone(), source_check);
    Router::new()
        .route("/", get(index))
        .route("/app/{file}", get(app_asset))
        .route("/manifest.webmanifest", get(manifest))
        .route("/icons/{file}", get(icon))
        .route("/api/state", get(api_state))
        .route("/ws", get(ws_handler))
        .route(
            "/api/projects/{project}/tasks/{task}/{op}",
            post(progress_op).route_layer(source_check_layer.clone()),
        )
        .route(
            "/api/projects/{project}/workstreams/{workstream}/override",
            put(set_override)
                .delete(clear_override)
                .route_layer(source_check_layer),
        )
        .with_state(app)
}

async fn index() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        include_str!("../assets/index.html"),
    )
}

/// `/app/<file>`：只認得這三個檔名，查表命中就回對應內嵌內容，其餘 404——不是「任意檔名
/// 都能讀」的通用靜態伺服（design D13 明確排除 `ServeDir`）。
async fn app_asset(Path(file): Path<String>) -> Response {
    match file.as_str() {
        "channel.js" => (
            [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
            include_str!("../assets/app/channel.js"),
        )
            .into_response(),
        "render.js" => (
            [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
            include_str!("../assets/app/render.js"),
        )
            .into_response(),
        "style.css" => (
            [(header::CONTENT_TYPE, "text/css; charset=utf-8")],
            include_str!("../assets/app/style.css"),
        )
            .into_response(),
        "actions.js" => (
            [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
            include_str!("../assets/app/actions.js"),
        )
            .into_response(),
        _ => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn manifest() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "application/manifest+json")],
        include_str!("../assets/manifest.webmanifest"),
    )
}

/// `/icons/<file>`：同 [`app_asset`]，只認得這兩個檔名。
async fn icon(Path(file): Path<String>) -> Response {
    match file.as_str() {
        "icon-192.png" => (
            [(header::CONTENT_TYPE, "image/png")],
            include_bytes!("../assets/icons/icon-192.png").as_slice(),
        )
            .into_response(),
        "icon-512.png" => (
            [(header::CONTENT_TYPE, "image/png")],
            include_bytes!("../assets/icons/icon-512.png").as_slice(),
        )
            .into_response(),
        _ => StatusCode::NOT_FOUND.into_response(),
    }
}

/// 目前整份投影（design D9：讀 `watch::Receiver` 的 `borrow()` 現況，不觸發重新計算）。
///
/// 手動序列化而不是 `axum::Json`：`Arc<ProjectedState>: Serialize` 需要 serde 的 `rc`
/// feature（`cockpit/Cargo.toml` 沒開，且不在本 task 授權改動範圍），所以用 `&**` 兩層
/// deref（`Ref<Arc<ProjectedState>>` → `Arc<ProjectedState>` → `ProjectedState`）拿到
/// `&ProjectedState` 再交給 [`state_json`]——與 `/ws` 共用同一個序列化函數（design D9
/// 節錄）。
async fn api_state(State(app): State<AppState>) -> impl IntoResponse {
    let body = state_json(&app.state.borrow());
    ([(header::CONTENT_TYPE, "application/json")], body)
}

/// 序列化一份投影成 JSON 文字；`/api/state` 與 `/ws` 共用，避免兩份序列化邏輯（design D9
/// 節錄：「序列化：每則訊息＝`serde_json::to_string(&*state)`；可與 `/api/state` 共用同一個
/// 序列化函數」）。
fn state_json(state: &ProjectedState) -> String {
    serde_json::to_string(state).expect("ProjectedState 序列化不會失敗")
}

/// `/ws`：升級成 WebSocket，把訂閱端交給 [`handle_socket`]（design D9）。
async fn ws_handler(ws: WebSocketUpgrade, State(app): State<AppState>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, app.state.clone()))
}

/// 連線期間的完整生命週期：先送現況一份，之後每次投影 `version` 遞增就再送整份；忽略
/// 客戶端訊息；斷線或送失敗就結束（純觀察者，不對狀態庫做任何寫入）。
async fn handle_socket(mut socket: WebSocket, mut state: watch::Receiver<Arc<ProjectedState>>) {
    tracing::debug!("ws 連線開始");

    let first = state_json(&state.borrow_and_update());
    if socket.send(Message::text(first)).await.is_err() {
        tracing::debug!("ws 連線結束（送出第一則訊息失敗）");
        return;
    }

    loop {
        tokio::select! {
            changed = state.changed() => {
                if changed.is_err() {
                    // 送出端（StoreHandle）掉了，不會再有新的投影，結束這個連線。
                    break;
                }
                let body = state_json(&state.borrow_and_update());
                if socket.send(Message::text(body)).await.is_err() {
                    break;
                }
            }
            msg = socket.recv() => {
                match msg {
                    None | Some(Err(_)) | Some(Ok(Message::Close(_))) => break,
                    // 文字、二進位、ping、pong：一律忽略（ping／pong 其實已經被 axum
                    // 在更底層自動回應，不會走到這裡；保留這個分支只是讓比對窮盡）。
                    Some(Ok(_)) => {}
                }
            }
        }
    }

    tracing::debug!("ws 連線結束");
}

/// `POST /api/projects/<project>/tasks/<task>/<op>`（spec `pipeline-progress`「進度寫入端點」；
/// design D6：`<op>` 以字串比對四值，其他回 404，不進到 [`ProgressService`]）。
async fn progress_op(
    State(app): State<AppState>,
    Path((project, task, op)): Path<(String, String, String)>,
) -> Response {
    let Some(parsed_op) = parse_progress_op(&op) else {
        return error_response(StatusCode::NOT_FOUND, &format!("不是合法的操作：{op}"));
    };
    let Some(progress) = &app.progress else {
        // 沒有任何 project 時（design Migration Plan）任何 project 引用都等於「不存在」；
        // 借用 WriteError::UnknownProject 的 Display，跟寫入服務判定「project 不存在」時
        // 回的本體用同一套措辭，不要另開一種說法（Codex fix round 1 finding 3：404 也要有
        // `{"error": ...}` 本體）。
        return write_error_response(WriteError::UnknownProject(ProjectId::new(project)));
    };
    match progress
        .apply_progress(&ProjectId::new(project), &TaskId::new(task), parsed_op)
        .await
    {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(error) => write_error_response(error),
    }
}

/// 把路徑上的 `<op>` 字串比對成 [`ProgressOp`]；不是四值之一回 `None`（design D6）。
fn parse_progress_op(op: &str) -> Option<ProgressOp> {
    match op {
        "advance" => Some(ProgressOp::Advance),
        "complete" => Some(ProgressOp::Complete),
        "fail" => Some(ProgressOp::Fail),
        "clear" => Some(ProgressOp::Clear),
        _ => None,
    }
}

/// `PUT` 本體的形狀（spec `pipeline-progress`「綁定覆蓋端點」）：`runtime`／`pane_id` 都必須
/// 是字串欄位；`serde` 解析失敗（缺欄位、型別不對、根本不是 JSON）統一回 400。
#[derive(Deserialize)]
struct OverrideRequest {
    runtime: String,
    pane_id: String,
}

/// `PUT /api/projects/<project>/workstreams/<workstream>/override`：設定畫面覆蓋（spec
/// 「綁定覆蓋端點」）。
///
/// 不檢查 `Content-Type`——design D6 明確否決把它當防線（沒有本體的 `POST` 端點與
/// `text/plain` 表單都能繞過），這裡直接把整個 body 當 JSON 解析。
async fn set_override(
    State(app): State<AppState>,
    Path((project, workstream)): Path<(String, String)>,
    body: Bytes,
) -> Response {
    let Some(progress) = &app.progress else {
        return write_error_response(WriteError::UnknownProject(ProjectId::new(project)));
    };
    let request: OverrideRequest = match serde_json::from_slice(&body) {
        Ok(request) => request,
        Err(_) => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "本體必須是含 runtime、pane_id 兩個字串欄位的 JSON 物件",
            );
        }
    };
    let override_ = Override {
        runtime: RuntimeId::new(request.runtime),
        pane_id: PaneId::new(request.pane_id),
    };
    match progress
        .set_override(
            &ProjectId::new(project),
            &WorkstreamId::new(workstream),
            override_,
        )
        .await
    {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(error) => write_error_response(error),
    }
}

/// `DELETE` 同路徑：取消畫面覆蓋；覆蓋本來就不存在時也回 204（spec「取消不存在的覆蓋回
/// 204」）。
async fn clear_override(
    State(app): State<AppState>,
    Path((project, workstream)): Path<(String, String)>,
) -> Response {
    let Some(progress) = &app.progress else {
        return write_error_response(WriteError::UnknownProject(ProjectId::new(project)));
    };
    match progress
        .clear_override(&ProjectId::new(project), &WorkstreamId::new(workstream))
        .await
    {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(error) => write_error_response(error),
    }
}

/// [`WriteError`] → HTTP 回應：`Unknown*` 404、`Rejected` 409、`Persist`／`Internal` 500，
/// 本體一律是 `{"error": "<原因>"}`（task 4.1 原文「錯誤本體 `{"error": ...}`」——不是只有
/// 409／500，Codex fix round 1 finding 3：先前 404 回空本體，跟 tasks.md 4.1 明定的形狀
/// 不符；axum 自己判定路徑完全不匹配的 404（例如未知路徑）不在此限，那種情況根本不會進到
/// 這個函式）。
fn write_error_response(error: WriteError) -> Response {
    match &error {
        WriteError::UnknownProject(_)
        | WriteError::UnknownTask(_)
        | WriteError::UnknownWorkstream(_) => {
            error_response(StatusCode::NOT_FOUND, &error.to_string())
        }
        WriteError::Rejected(_) => error_response(StatusCode::CONFLICT, &error.to_string()),
        WriteError::Persist { .. } | WriteError::Internal(_) => {
            tracing::error!(%error, "寫入端點：狀態檔寫入失敗");
            error_response(StatusCode::INTERNAL_SERVER_ERROR, &error.to_string())
        }
    }
}

/// 錯誤回應本體 `{"error": "<reason>"}`（spec 多處要求的形狀）。
#[derive(Serialize)]
struct ErrorBody<'a> {
    error: &'a str,
}

pub(crate) fn error_response(status: StatusCode, reason: &str) -> Response {
    let body = serde_json::to_string(&ErrorBody { error: reason })
        .expect("ErrorBody 只含字串，序列化不會失敗");
    (status, [(header::CONTENT_TYPE, "application/json")], body).into_response()
}
