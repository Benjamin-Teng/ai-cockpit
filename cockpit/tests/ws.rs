//! Task 3.4 驗收測試：`cockpit::http::router` 的 `/ws` WebSocket 推送（spec
//! `cockpit-dashboard`「WebSocket 推送整張圖」；design D9、D14）。
//!
//! 這裡真的綁 `127.0.0.1:0` 開 port（`/ws` 需要真正的 TCP upgrade，`tower::oneshot`
//! 打不出握手），客戶端用 `tokio-tungstenite`（design D14：版本與 axum 內部
//! `tungstenite` 不同無妨，只當客戶端用）。每個 await 都包一層 `timeout`，避免測試在
//! CI 卡死。

use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::Ordering;
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
use tokio_tungstenite::tungstenite::client::IntoClientRequest;

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
    // `/ws` 套來源檢查（ws-source-check）：回填實際監聽埠，同 `app::run_with_shutdown` 的做法。
    state.port.store(addr.port(), Ordering::Relaxed);
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

/// 以指定的 `Origin`（`None` 表示不帶）請求升級 `/ws`；`Host` 由 tokio-tungstenite 依 URL 自動帶上
/// （`127.0.0.1:<port>`）。
async fn try_connect_with_origin(
    addr: SocketAddr,
    origin: Option<&str>,
) -> Result<WsStream, tokio_tungstenite::tungstenite::Error> {
    let mut request = format!("ws://{addr}/ws")
        .into_client_request()
        .expect("升級請求應該組得起來");
    if let Some(origin) = origin {
        request
            .headers_mut()
            .insert("origin", origin.parse().expect("Origin 應該是合法標頭值"));
    }
    timeout(TIMEOUT, tokio_tungstenite::connect_async(request))
        .await
        .expect("WebSocket 連線逾時")
        .map(|(ws, _response)| ws)
}

/// ws-source-check Scenario「外站網頁連 /ws 被拒」：外站 `Origin` 升級 `/ws` 得到 403（`code` 為
/// `forbidden_source`），沒有升級成 WebSocket、沒有收到任何狀態。
#[tokio::test]
async fn foreign_origin_upgrade_is_rejected_with_403() {
    let handle = StoreHandle::new(RuntimeStore::new());
    let (addr, _server) = serve(&handle).await;

    for origin in ["https://evil.example", "http://evil.example"] {
        match try_connect_with_origin(addr, Some(origin)).await {
            Ok(_) => panic!("Origin {origin} 不該升級成功"),
            Err(tokio_tungstenite::tungstenite::Error::Http(response)) => {
                assert_eq!(
                    response.status(),
                    axum::http::StatusCode::FORBIDDEN,
                    "Origin {origin} 應該回 403"
                );
                let body = response.body().as_deref().unwrap_or_default();
                let parsed: Value = serde_json::from_slice(body).expect("403 本體應該是 JSON");
                assert_eq!(parsed["code"], "forbidden_source");
            }
            Err(other) => panic!("Origin {origin} 應該得到 HTTP 403 回應，實際：{other:?}"),
        }
    }
}

/// Scenario「同源與命令列照常可用」：`Origin` 與 `Host` 同源升級成功並收到現況；不帶 `Origin`
/// （命令列、驗收腳本）同樣可用。
#[tokio::test]
async fn same_origin_and_originless_upgrade_receive_current_state() {
    let handle = StoreHandle::new(RuntimeStore::new());
    let (addr, _server) = serve(&handle).await;
    let expected = serde_json::to_value(&*handle.current()).expect("ProjectedState 應該可序列化");

    let same_origin = format!("http://{addr}");
    for origin in [Some(same_origin.as_str()), None] {
        let mut ws = try_connect_with_origin(addr, origin)
            .await
            .unwrap_or_else(|e| panic!("Origin {origin:?} 應該升級成功：{e:?}"));
        assert_eq!(
            next_json(&mut ws).await,
            expected,
            "Origin {origin:?} 應該收到目前整張圖"
        );
    }
}

/// 送一個手寫的 `/ws` 升級請求（`host_headers` 為要送的全部 `Host` 行值，可重複、可為空），回傳回應的
/// 第一行（狀態列）。手寫是為了能控制 `Host`：tokio-tungstenite 一律依 URL 自己帶 `Host`。
async fn raw_upgrade_status_line(addr: SocketAddr, host_headers: &[&str]) -> String {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let mut request = String::from("GET /ws HTTP/1.1\r\n");
    for host in host_headers {
        request.push_str(&format!("Host: {host}\r\n"));
    }
    request.push_str(
        "Connection: Upgrade\r\nUpgrade: websocket\r\nSec-WebSocket-Version: 13\r\n\
         Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\r\n",
    );
    let mut stream = timeout(TIMEOUT, tokio::net::TcpStream::connect(addr))
        .await
        .expect("連線逾時")
        .expect("連得上自己剛開的 server");
    stream
        .write_all(request.as_bytes())
        .await
        .expect("送出請求不應該失敗");
    // 101 之後連線不會自己關（讀不到 EOF），所以只讀到第一批資料；403 回應很小，一次讀得完。
    let mut buf = vec![0u8; 4096];
    let n = timeout(TIMEOUT, stream.read(&mut buf))
        .await
        .expect("讀取回應逾時")
        .expect("讀取回應不應該失敗");
    let text = String::from_utf8_lossy(&buf[..n]).into_owned();
    text.lines().next().unwrap_or_default().to_string()
}

/// DNS rebinding 形狀：`Host` 不是本機位址（或埠不符）的 `/ws` 升級回 403、不升級成 WebSocket。
#[tokio::test]
async fn bad_host_upgrade_is_rejected_with_403() {
    let handle = StoreHandle::new(RuntimeStore::new());
    let (addr, _server) = serve(&handle).await;

    for host in [
        format!("evil.example:{}", addr.port()),
        format!("127.0.0.1:{}", addr.port().wrapping_add(1)),
    ] {
        let status = raw_upgrade_status_line(addr, &[&host]).await;
        assert!(
            status.starts_with("HTTP/1.1 403"),
            "Host {host} 的升級應該回 403，實際：{status}"
        );
    }
}

/// 重複 `Host`（即使其中一個合法）的 `/ws` 升級回 403、不升級。
#[tokio::test]
async fn duplicated_host_upgrade_is_rejected_with_403() {
    let handle = StoreHandle::new(RuntimeStore::new());
    let (addr, _server) = serve(&handle).await;
    let own = format!("127.0.0.1:{}", addr.port());

    for hosts in [
        vec![own.as_str(), own.as_str()],
        vec![own.as_str(), "evil.example:80"],
        vec!["evil.example:80", own.as_str()],
    ] {
        let status = raw_upgrade_status_line(addr, &hosts).await;
        assert!(
            status.starts_with("HTTP/1.1 403"),
            "重複 Host {hosts:?} 的升級應該回 403，實際：{status}"
        );
    }
}
