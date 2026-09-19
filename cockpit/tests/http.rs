//! Task 3.3 驗收測試：`cockpit::http::router` 的路由表與內嵌靜態資源（spec
//! `cockpit-dashboard`「路由與內嵌資源」；design D13、D9）。
//!
//! 全部用 `tower::ServiceExt::oneshot` 打進 `Router`，不開 port；每個測試自己建一個
//! `StoreHandle`，互不共用狀態。

use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use cockpit::http::{self, AppState};
use cockpit_core::{ProjectedState, RuntimeId, RuntimeStore, StoreHandle, spawn_projector};
use http_body_util::BodyExt;
use tower::ServiceExt;

/// PNG 檔頭簽章（RFC 2083 §3.1）：所有合法 PNG 開頭都是這 8 bytes。
const PNG_SIGNATURE: [u8; 8] = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];

fn new_app_state() -> (StoreHandle, AppState) {
    let handle = StoreHandle::new(RuntimeStore::new());
    let state = AppState::new(handle.subscribe());
    (handle, state)
}

async fn body_bytes(response: axum::response::Response) -> Vec<u8> {
    response
        .into_body()
        .collect()
        .await
        .expect("body 收集不應該失敗")
        .to_bytes()
        .to_vec()
}

fn content_type(response: &axum::response::Response) -> String {
    response
        .headers()
        .get(axum::http::header::CONTENT_TYPE)
        .expect("回應應該帶 content-type")
        .to_str()
        .expect("content-type 應該是合法字串")
        .to_string()
}

#[tokio::test]
async fn routes_return_200_with_expected_content_types() {
    let (_handle, state) = new_app_state();

    let cases: Vec<(&str, &str)> = vec![
        ("/", "text/html"),
        ("/app/render.js", "text/javascript"),
        ("/app/style.css", "text/css"),
        ("/app/channel.js", "text/javascript"),
        ("/app/actions.js", "text/javascript"),
        ("/manifest.webmanifest", "application/manifest+json"),
        ("/icons/icon-192.png", "image/png"),
        ("/icons/icon-512.png", "image/png"),
        ("/api/state", "application/json"),
    ];

    for (path, expected_prefix) in cases {
        let router = http::router(state.clone());
        let request = Request::builder()
            .uri(path)
            .body(Body::empty())
            .expect("request 建構不應該失敗");
        let response = router
            .oneshot(request)
            .await
            .expect("oneshot 呼叫不應該失敗");

        assert_eq!(response.status(), StatusCode::OK, "{path} 應該回 200");
        let ct = content_type(&response);
        assert!(
            ct.starts_with(expected_prefix),
            "{path} 的 content-type 應以 {expected_prefix:?} 開頭，實際: {ct:?}"
        );

        if path == "/" {
            let body = body_bytes(response).await;
            assert_eq!(
                body,
                include_str!("../assets/index.html").as_bytes(),
                "/ 的 body 應該等於內嵌的 index.html"
            );
        } else if path == "/icons/icon-192.png" || path == "/icons/icon-512.png" {
            let body = body_bytes(response).await;
            assert!(
                body.starts_with(&PNG_SIGNATURE),
                "{path} 的 body 前 8 bytes 應該是 PNG 簽章"
            );
        }
    }
}

#[tokio::test]
async fn api_state_equals_current_projection_version() {
    let (handle, state) = new_app_state();
    let router = http::router(state);

    let id = RuntimeId::new("win");
    handle.register(
        id.clone(),
        "herdr".to_string(),
        "named-pipe \\\\.\\pipe\\x".to_string(),
    );
    let _projector = spawn_projector(handle.clone());

    // 等投影任務把 register 造成的髒資料算成新一版投影並廣播。
    let mut probe = handle.subscribe();
    tokio::time::timeout(Duration::from_secs(5), probe.changed())
        .await
        .expect("投影廣播不應該逾時")
        .expect("watch channel 不應該關閉");

    let expected: Arc<ProjectedState> = handle.current();

    let request = Request::builder()
        .uri("/api/state")
        .body(Body::empty())
        .expect("request 建構不應該失敗");
    let response = router
        .oneshot(request)
        .await
        .expect("oneshot 呼叫不應該失敗");
    assert_eq!(response.status(), StatusCode::OK);

    let body = body_bytes(response).await;
    let parsed: ProjectedState = serde_json::from_slice(&body).expect("body 應該是合法 JSON");

    assert_eq!(parsed.version, expected.version, "version 應該等於目前投影");
    assert_eq!(parsed.runtimes.len(), 1, "應該恰好登記了一個 runtime");
    assert_eq!(
        parsed.runtimes[0].id, id,
        "runtimes[0].id 應該等於登記的 id"
    );
}

/// Task 3.5 驗收：內嵌資源已經是正式內容，不是 3.3 的占位檔（spec `cockpit-dashboard`
/// 「畫面整頁重畫」）。只查文字標記，不驗證實際渲染行為——那要真的執行 JS，這裡沒有引擎。
#[tokio::test]
async fn embedded_assets_are_the_final_files() {
    let (_handle, state) = new_app_state();

    async fn body_text(router: axum::Router, path: &str) -> String {
        let request = Request::builder()
            .uri(path)
            .body(Body::empty())
            .expect("request 建構不應該失敗");
        let response = router
            .oneshot(request)
            .await
            .expect("oneshot 呼叫不應該失敗");
        assert_eq!(response.status(), StatusCode::OK, "{path} 應該回 200");
        String::from_utf8(body_bytes(response).await).expect("body 應該是合法 UTF-8")
    }

    // 刻意比對「定義」而不是裸字 "onState"／"onChannel"：光靠裸字比對，一句提到這些
    // 名字的註解就能矇混過關（Task 3.5 突變證據：把 `window.onState = function` 改名
    // 後，檔案裡仍留著提到 "onState" 的註解，裸字版本的斷言不會抓到）。
    let render_js = body_text(http::router(state.clone()), "/app/render.js").await;
    assert!(
        render_js.contains("window.onState = function"),
        "render.js 應該把 onState 掛在 window 上"
    );
    assert!(
        render_js.contains("function renderState"),
        "render.js 應該定義 renderState"
    );

    let channel_js = body_text(http::router(state.clone()), "/app/channel.js").await;
    assert!(channel_js.contains("/ws"), "channel.js 應該連到 /ws");
    assert!(
        channel_js.contains("window.onChannel"),
        "channel.js 應該呼叫 window.onChannel"
    );

    let style_css = body_text(http::router(state.clone()), "/app/style.css").await;
    assert!(
        style_css.contains(".status-working"),
        "style.css 應該定義 .status-working"
    );

    let index_html = body_text(http::router(state.clone()), "/").await;
    assert!(
        index_html.contains(r#"id="app""#),
        "index.html 應該有 id=\"app\" 的容器"
    );
}

/// Task 5.3 驗收（design D9）：`index.html` 依序載入 render.js → actions.js → channel.js，
/// 且 actions.js 已是正式內容（根節點委派 `pointerdown`、鍵盤另收 `detail === 0` 的
/// `click`），不是 task 4.1 的佔位檔。只查文字標記；實際互動行為由
/// `docs/research/2026-09-16/actions-check.js`（headless Chrome）驗。
#[tokio::test]
async fn index_loads_actions_js_between_render_and_channel() {
    let (_handle, state) = new_app_state();

    async fn body_text(router: axum::Router, path: &str) -> String {
        let request = Request::builder()
            .uri(path)
            .body(Body::empty())
            .expect("request 建構不應該失敗");
        let response = router
            .oneshot(request)
            .await
            .expect("oneshot 呼叫不應該失敗");
        assert_eq!(response.status(), StatusCode::OK, "{path} 應該回 200");
        String::from_utf8(body_bytes(response).await).expect("body 應該是合法 UTF-8")
    }

    let index_html = body_text(http::router(state.clone()), "/").await;
    let position = |needle: &str| {
        index_html
            .find(needle)
            .unwrap_or_else(|| panic!("index.html 應該引用 {needle}"))
    };
    let render = position(r#"<script src="/app/render.js"></script>"#);
    let actions = position(r#"<script src="/app/actions.js"></script>"#);
    let channel = position(r#"<script src="/app/channel.js"></script>"#);
    assert!(
        render < actions && actions < channel,
        "載入順序應該是 render.js → actions.js → channel.js"
    );

    let actions_js = body_text(http::router(state.clone()), "/app/actions.js").await;
    assert!(
        actions_js.contains(r#"addEventListener("pointerdown""#),
        "actions.js 應該以 pointerdown 委派"
    );
    assert!(
        actions_js.contains("event.detail === 0"),
        "actions.js 應該另收 detail === 0 的鍵盤 click"
    );
}

/// Task 3.5 驗收（spec 節錄：「`done` 顯示為 `done`，不出現『完成』字樣」）：直接 grep
/// 內嵌的 render.js 原始碼，防止有人手滑把狀態文字翻成中文「完成」。
#[tokio::test]
async fn render_js_never_prints_completion_word() {
    let (_handle, state) = new_app_state();
    let router = http::router(state);

    let request = Request::builder()
        .uri("/app/render.js")
        .body(Body::empty())
        .expect("request 建構不應該失敗");
    let response = router
        .oneshot(request)
        .await
        .expect("oneshot 呼叫不應該失敗");
    assert_eq!(response.status(), StatusCode::OK);

    let text = String::from_utf8(body_bytes(response).await).expect("body 應該是合法 UTF-8");
    assert!(
        !text.contains("完成"),
        "render.js 不該出現「完成」字樣（done 顯示為 done）"
    );
}

#[tokio::test]
async fn unknown_path_is_404() {
    let (_handle, state) = new_app_state();

    for path in ["/nope", "/app/evil.js", "/icons/x.png"] {
        let router = http::router(state.clone());
        let request = Request::builder()
            .uri(path)
            .body(Body::empty())
            .expect("request 建構不應該失敗");
        let response = router
            .oneshot(request)
            .await
            .expect("oneshot 呼叫不應該失敗");
        assert_eq!(
            response.status(),
            StatusCode::NOT_FOUND,
            "{path} 應該回 404"
        );
    }
}
