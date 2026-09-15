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

use std::sync::Arc;

use axum::Router;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use cockpit_core::ProjectedState;
use tokio::sync::watch;

/// 路由共用的狀態：訂閱 [`cockpit_core::StoreHandle`] 廣播的投影（design D9）。
///
/// `Clone` 便宜——`watch::Receiver` 本身可以自由複製，各請求各自 `borrow()` 目前這一份，
/// 不需要額外的鎖或 `Arc` 包一層。
#[derive(Clone)]
pub struct AppState {
    /// 目前投影的訂閱端；`/api/state` 用 `borrow()` 讀現況，`/ws` 每個連線各自
    /// `clone()` 一份自己追（1.6 的 `StoreHandle::subscribe`）。
    pub state: watch::Receiver<Arc<ProjectedState>>,
}

/// 組出完整的路由表：`/`、`/app/{file}`、`/manifest.webmanifest`、`/icons/{file}`、
/// `/api/state`、`/ws`；其他路徑落回 axum 預設的 404。
pub fn router(app: AppState) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/app/{file}", get(app_asset))
        .route("/manifest.webmanifest", get(manifest))
        .route("/icons/{file}", get(icon))
        .route("/api/state", get(api_state))
        .route("/ws", get(ws_handler))
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
