//! Task 3.4 驗收測試：`cockpit::http::router` 的 `/ws` WebSocket 推送（spec
//! `cockpit-dashboard`「WebSocket 推送整張圖」；design D9、D14）。
//!
//! 這裡真的綁 `127.0.0.1:0` 開 port（`/ws` 需要真正的 TCP upgrade，`tower::oneshot`
//! 打不出握手），客戶端用 `tokio-tungstenite`（design D14：版本與 axum 內部
//! `tungstenite` 不同無妨，只當客戶端用）。每個 await 都包一層 `timeout`，避免測試在
//! CI 卡死。

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use cockpit::http::{self, AppState};
use cockpit_core::{
    Focused, ProjectedState, RuntimeId, RuntimeSnapshot, RuntimeStore, StoreHandle, Workspace,
    WorkspaceId, spawn_projector,
};
use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use tokio::net::TcpListener;
use tokio::task::JoinHandle;
use tokio::time::timeout;
use tokio_tungstenite::MaybeTlsStream;
use tokio_tungstenite::tungstenite::Message as WsMessage;

/// 每個 await 的逾時上限；卡住就當測試失敗，不要無限期掛著。
const TIMEOUT: Duration = Duration::from_secs(2);

type WsStream = tokio_tungstenite::WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;

/// 起一個真的監聽 `127.0.0.1:0` 的 server，回傳它的位址與背景任務的 handle。
async fn serve(handle: &StoreHandle) -> (SocketAddr, JoinHandle<()>) {
    let state = AppState::new(handle.subscribe());
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind 127.0.0.1:0 不應該失敗");
    let addr = listener.local_addr().expect("local_addr 不應該失敗");
    let router = http::router(state);
    let join = tokio::spawn(async move {
        axum::serve(listener, router)
            .await
            .expect("axum::serve 不應該失敗");
    });
    (addr, join)
}

/// 連上 `/ws`，逾時保護。
async fn connect(addr: SocketAddr) -> WsStream {
    let (ws, _response) = timeout(
        TIMEOUT,
        tokio_tungstenite::connect_async(format!("ws://{addr}/ws")),
    )
    .await
    .expect("WebSocket 連線逾時")
    .expect("WebSocket 連線不應該失敗");
    ws
}

/// 讀下一則訊息，斷言是 Text 且能解析成 JSON；逾時保護。
async fn next_json(ws: &mut WsStream) -> Value {
    let msg = timeout(TIMEOUT, ws.next())
        .await
        .expect("等待下一則推送逾時")
        .expect("串流不應該提前結束")
        .expect("讀取訊息不應該失敗（server 不應該中斷連線）");
    match msg {
        WsMessage::Text(text) => serde_json::from_str(&text).expect("推送內容應該是合法 JSON"),
        other => panic!("推送應該是 Text 訊息，實際收到: {other:?}"),
    }
}

/// 一份含一個 workspace 的快照，用來驅動「狀態變了」。
fn one_workspace_snapshot() -> RuntimeSnapshot {
    RuntimeSnapshot {
        server_version: "0.9.0".to_string(),
        protocol: 1,
        workspaces: vec![Workspace {
            id: WorkspaceId::new("wJ"),
            label: None,
            number: 1,
            agent_status: cockpit_core::AgentStatus::Idle,
            focused: false,
        }],
        tabs: vec![],
        panes: vec![],
        agents: vec![],
        focused: Focused {
            workspace_id: None,
            tab_id: None,
            pane_id: None,
        },
        protocol_warning: None,
    }
}

/// 等一個 `StoreHandle` 的訂閱端收到下一次投影廣播（逾時保護）。
async fn wait_for_projection(handle: &StoreHandle) {
    let mut probe = handle.subscribe();
    timeout(TIMEOUT, probe.changed())
        .await
        .expect("等待投影廣播逾時")
        .expect("watch channel 不應該關閉");
}

/// GIVEN 投影現況 WHEN 客戶端連上 `/ws` THEN 第一則訊息的 JSON 等於 `handle.current()`
/// （即等於 `/api/state`，因為兩者共用同一個序列化函數與同一份 `Arc`）。
#[tokio::test]
async fn first_message_equals_api_state() {
    let handle = StoreHandle::new(RuntimeStore::new());
    let (addr, _server) = serve(&handle).await;

    let mut ws = connect(addr).await;
    let actual = next_json(&mut ws).await;

    let expected: Arc<ProjectedState> = handle.current();
    let expected = serde_json::to_value(&*expected).expect("ProjectedState 應該可序列化");

    assert_eq!(
        actual, expected,
        "第一則訊息應該等於目前投影（＝/api/state）"
    );
}

/// GIVEN 客戶端已連上 WHEN 狀態庫變動（`replace` 一份含一個 workspace 的快照）THEN
/// 客戶端收到下一則訊息的 `version` 為前一則 + 1，且是整份 JSON（`workspaces` 非空），
/// 不是只送版本號的增量。
#[tokio::test]
async fn store_change_pushes_next_version_full_json() {
    let handle = StoreHandle::new(RuntimeStore::new());
    let id = RuntimeId::new("win");
    handle.register(
        id.clone(),
        "herdr".to_string(),
        "named-pipe \\\\.\\pipe\\x".to_string(),
    );
    let _projector = spawn_projector(handle.clone());

    // 等 register 造成的髒資料被投影任務算成新一版並廣播（version 2）之後才開始連線，
    // 避免第一則訊息剛好卡在「register 已發生、投影還沒送出」的中間態。
    wait_for_projection(&handle).await;

    let (addr, _server) = serve(&handle).await;
    let mut ws = connect(addr).await;

    let first = next_json(&mut ws).await;
    let first_version = first["version"]
        .as_u64()
        .expect("第一則訊息的 version 應該是數字");

    handle
        .replace(&id, one_workspace_snapshot())
        .expect("replace 不應該失敗（runtime 已登記過）");

    let second = next_json(&mut ws).await;
    assert_eq!(
        second["version"].as_u64(),
        Some(first_version + 1),
        "下一則訊息的 version 應該遞增 1"
    );
    let workspaces = second["runtimes"][0]["workspaces"]
        .as_array()
        .expect("runtimes[0].workspaces 應該是陣列");
    assert!(
        !workspaces.is_empty(),
        "workspaces 應該非空（整份 JSON，不是只送 version 的增量）"
    );
}

/// GIVEN 兩個客戶端都連上 WHEN 發生一次狀態變動 THEN 兩者各收到一則新推送，`version`
/// 相同且等於 `handle.current().version`。
#[tokio::test]
async fn two_clients_both_receive_same_version() {
    let handle = StoreHandle::new(RuntimeStore::new());
    let (addr, _server) = serve(&handle).await;

    let mut ws1 = connect(addr).await;
    let mut ws2 = connect(addr).await;

    let _first1 = next_json(&mut ws1).await;
    let _first2 = next_json(&mut ws2).await;

    let id = RuntimeId::new("win");
    handle.register(
        id,
        "herdr".to_string(),
        "named-pipe \\\\.\\pipe\\x".to_string(),
    );
    let _projector = spawn_projector(handle.clone());

    let second1 = next_json(&mut ws1).await;
    let second2 = next_json(&mut ws2).await;

    assert_eq!(
        second1["version"], second2["version"],
        "兩個客戶端應該收到相同 version 的推送"
    );
    assert_eq!(
        second1["version"].as_u64(),
        Some(handle.current().version),
        "推送的 version 應該等於目前投影的 version"
    );
}

/// GIVEN 客戶端已連上 WHEN 客戶端送出文字與二進位訊息 THEN server 不理會、不 panic、
/// 連線仍然開著——之後的狀態變動仍能推送過去。
#[tokio::test]
async fn client_messages_are_ignored() {
    let handle = StoreHandle::new(RuntimeStore::new());
    let (addr, _server) = serve(&handle).await;

    let mut ws = connect(addr).await;
    let _first = next_json(&mut ws).await;

    timeout(TIMEOUT, ws.send(WsMessage::text("hi")))
        .await
        .expect("送出文字訊息逾時")
        .expect("送出文字訊息不應該失敗");
    timeout(TIMEOUT, ws.send(WsMessage::binary(vec![1, 2, 3])))
        .await
        .expect("送出二進位訊息逾時")
        .expect("送出二進位訊息不應該失敗");

    let id = RuntimeId::new("win");
    handle.register(
        id,
        "herdr".to_string(),
        "named-pipe \\\\.\\pipe\\x".to_string(),
    );
    let _projector = spawn_projector(handle.clone());

    let second = next_json(&mut ws).await;
    assert!(
        second["version"].as_u64().is_some(),
        "連線應該仍然開著，能收到下一則推送（server 沒有因為客戶端訊息而斷線或 panic）"
    );
}
