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
        ("/app/output.js", "text/javascript"),
        // file-review task 4.1：左欄分頁／分頁區（files.js）與檢視器（viewers.js）的路由。
        ("/app/files.js", "text/javascript"),
        ("/app/viewers.js", "text/javascript"),
        // git-review task 3.3：git.js 先以空殼檔案內嵌（cockpit-dashboard delta「路由與
        // content-type」）。
        ("/app/git.js", "text/javascript"),
        // desktop-launch-notify task 3.3（design D7）：桌面通知模組與設定面板（notify.js）。
        ("/app/notify.js", "text/javascript"),
        // ui-language task 1.1（design D1）：介面字典與語言決定（i18n.js），第一個載入。
        ("/app/i18n.js", "text/javascript"),
        ("/manifest.webmanifest", "application/manifest+json"),
        ("/icons/icon-192.png", "image/png"),
        ("/icons/icon-512.png", "image/png"),
        ("/api/state", "application/json"),
    ];

    for (path, expected_prefix) in cases {
        let router = http::router(state.clone());
        // `/api/state` 套來源檢查（ws-source-check），`AppState::new` 的 port 是 0，所以帶
        // `Host: 127.0.0.1:0`；其他路由不檢查，多帶無妨。
        let request = Request::builder()
            .uri(path)
            .header("host", "127.0.0.1:0")
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
        .header("host", "127.0.0.1:0")
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

    // ui-language task 1.1：i18n.js 是正式內容（掛 window.cockpitI18n、讀 cockpit.lang），不是佔位檔。
    let i18n_js = body_text(http::router(state.clone()), "/app/i18n.js").await;
    assert!(
        i18n_js.contains("window.cockpitI18n = {"),
        "i18n.js 應該把 cockpitI18n 掛在 window 上"
    );
    assert!(
        i18n_js.contains("cockpit.lang"),
        "i18n.js 應該讀寫 localStorage 的 cockpit.lang"
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
    // desktop-launch-notify task 3.3（design D7）：render.js 的 onState 會呼叫
    // window.cockpitNotify.observe，notify.js 要在 render.js 之前載入。
    let notify = position(r#"<script src="/app/notify.js"></script>"#);
    let render = position(r#"<script src="/app/render.js"></script>"#);
    assert!(notify < render, "notify.js 應該在 render.js 之前載入");
    // ui-language task 1.1（design D1、D2）：i18n.js 是第一個 <script>，其他模組載入時讀到的
    // window.cockpitI18n.t() 才已是正確語言。
    let i18n = position(r#"<script src="/app/i18n.js"></script>"#);
    let output = position(r#"<script src="/app/output.js"></script>"#);
    assert!(
        i18n < output,
        "i18n.js 應該在 output.js 之前載入（第一個 script）"
    );
    assert_eq!(
        index_html.find("<script src="),
        Some(i18n),
        "i18n.js 應該是 index.html 的第一個 <script src>"
    );
    // ui-language（階段審查 1 M1；spec「任何介面文字繪製之前決定語言」）：i18n.js 在 <head> 內同步載入
    // （在 <body> 之前執行，<html lang> 與語言先決定），style.css 有英文介面載入前先藏靜態節點的規則。
    let head_end = position("</head>");
    assert!(
        i18n < head_end,
        "i18n.js 應該在 <head> 內（</head> 之前）載入，語言才會在第一次繪製前決定"
    );
    let style_css = body_text(http::router(state.clone()), "/app/style.css").await;
    assert!(
        style_css.contains("html.i18n-pending [data-i18n]"),
        "style.css 應該有 html.i18n-pending [data-i18n] 規則，英文介面套用翻譯前先藏起靜態節點"
    );
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

/// 從 `i18n.js` 原始碼抓一份字典（`var zh = {` 或 `var en = {` 到 `};`）的 `鍵 -> 值`：只認
/// `"鍵": "值",` 這種單行條目（註解行與其他行略過），值裡的 `\"` 還原成 `"`。
fn i18n_dictionary(source: &str, start_marker: &str) -> Vec<(String, String)> {
    let start = source
        .find(start_marker)
        .unwrap_or_else(|| panic!("i18n.js 找不到 {start_marker}"));
    let body = &source[start + start_marker.len()..];
    let end = body.find("\n  };").expect("字典應該以 `};` 結尾");
    let mut entries = Vec::new();
    for line in body[..end].lines() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix('"') else {
            continue;
        };
        let Some((key, value)) = rest.split_once("\": \"") else {
            continue;
        };
        let value = value
            .strip_suffix("\",")
            .or_else(|| value.strip_suffix('"'))
            .unwrap_or(value);
        entries.push((key.to_string(), value.replace("\\\"", "\"")));
    }
    entries
}

/// 守門對象（design D5）：任何一段（以 `.` 分段）以 `done` 或 `agentDone` 開頭的字典鍵——
/// 描述 HERDR `done` 的文字都要用這個命名（見 i18n.js 檔頭）。
fn is_done_key(key: &str) -> bool {
    key.split('.')
        .any(|segment| segment.starts_with("done") || segment.starts_with("agentDone"))
}

/// 檢查守門對象：繁中值不得含「完成」，英文值不得含 complete／finished（不分大小寫，
/// `completed` 已被 `complete` 涵蓋）。回傳違規清單（空＝通過）。
fn done_guard_violations(source: &str) -> Vec<String> {
    let mut violations = Vec::new();
    for (key, value) in i18n_dictionary(source, "var zh = {") {
        if is_done_key(&key) && value.contains("完成") {
            violations.push(format!("zh {key}：{value}"));
        }
    }
    for (key, value) in i18n_dictionary(source, "var en = {") {
        let lower = value.to_lowercase();
        if is_done_key(&key) && (lower.contains("complete") || lower.contains("finished")) {
            violations.push(format!("en {key}：{value}"));
        }
    }
    violations
}

/// design D5：「done 不是完成」的禁字守門檢查字典（取代過去只看 render.js 原始碼的斷言）：描述 `done` 的鍵
/// 繁中不得含「完成」、英文不得含 complete／completed／finished。守門對象的數量也要 > 0，守門才不會空轉。
#[tokio::test]
async fn i18n_done_descriptions_avoid_completion_words() {
    let (_handle, state) = new_app_state();
    let request = Request::builder()
        .uri("/app/i18n.js")
        .body(Body::empty())
        .expect("request 建構不應該失敗");
    let response = http::router(state)
        .oneshot(request)
        .await
        .expect("oneshot 呼叫不應該失敗");
    assert_eq!(response.status(), StatusCode::OK);
    let source = String::from_utf8(body_bytes(response).await).expect("body 應該是合法 UTF-8");

    // 解析不能靜默失敗：兩份字典都要抓得到條目、鍵集合相同，否則下面的守門等於沒檢查。
    let zh = i18n_dictionary(&source, "var zh = {");
    let en = i18n_dictionary(&source, "var en = {");
    assert!(zh.len() > 30, "繁中字典應該抓得到條目（實際 {}）", zh.len());
    let zh_keys: Vec<&str> = zh.iter().map(|(k, _)| k.as_str()).collect();
    let en_keys: Vec<&str> = en.iter().map(|(k, _)| k.as_str()).collect();
    assert_eq!(zh_keys, en_keys, "兩份字典的鍵（含順序）應該相同");

    // 守門不能空轉：真實字典裡至少要有描述 done 的鍵（notify.kind.done.desc 等），兩份字典各自都要有。
    let guarded = |dict: &[(String, String)]| dict.iter().filter(|(k, _)| is_done_key(k)).count();
    assert!(
        guarded(&zh) > 0 && guarded(&en) > 0,
        "字典裡沒有任何守門對象的鍵（zh {}、en {}）：描述 HERDR done 的文字要照 is_done_key 命名",
        guarded(&zh),
        guarded(&en)
    );

    for key in ["notify.kind.done.desc", "notify.title.done"] {
        assert!(
            zh.iter().any(|(k, _)| k == key) && en.iter().any(|(k, _)| k == key),
            "字典缺少描述 done 的鍵 {key}"
        );
    }

    let violations = done_guard_violations(&source);
    assert!(
        violations.is_empty(),
        "描述 HERDR done 的字典值不得暗示 task 已完成：{violations:#?}"
    );
}

/// 守門本身要有牙齒：違規的字典值會被抓到，守門對象以外的鍵（例如「請求沒有完成」）不誤報。
#[test]
fn done_guard_flags_forbidden_words_only_on_done_keys() {
    let source = |zh_done: &str, en_done: &str| {
        format!(
            "var zh = {{\n    \"notify.kind.done\": \"{zh_done}\",\n    \"actions.error.network\": \"操作失敗（請求沒有完成）\"\n  }};\n\
             var en = {{\n    \"notify.kind.done\": \"{en_done}\",\n    \"actions.error.network\": \"Request did not complete\"\n  }};\n"
        )
    };
    assert!(done_guard_violations(&source("pane 已停下", "Pane stopped")).is_empty());
    assert_eq!(
        done_guard_violations(&source("task 完成", "Pane stopped")).len(),
        1
    );
    assert_eq!(
        done_guard_violations(&source("pane 已停下", "Task Completed")).len(),
        1
    );
    assert_eq!(
        done_guard_violations(&source("pane 已停下", "Agent finished")).len(),
        1
    );
    assert_eq!(done_guard_violations(&source("完成", "COMPLETE")).len(), 2);
    assert!(is_done_key("render.agentDone.hint") && is_done_key("notify.kind.doneBody"));
    assert!(!is_done_key("actions.error.network") && !is_done_key("render.task.advance"));
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

// ---------------------------------------------------------------------------
// `/vendor/{*path}` 內嵌資源（file-review task 3.3；spec `cockpit-dashboard`「路由與內嵌資源」
// scenario「vendored 資源」「單一執行檔」；design D9）
// ---------------------------------------------------------------------------

fn nosniff(response: &axum::response::Response) -> Option<String> {
    response
        .headers()
        .get("x-content-type-options")
        .map(|v| v.to_str().expect("nosniff 標頭值應為合法字串").to_string())
}

async fn get(router: axum::Router, path: &str) -> axum::response::Response {
    let request = Request::builder()
        .uri(path)
        .body(Body::empty())
        .expect("request 建構不應該失敗");
    router
        .oneshot(request)
        .await
        .expect("oneshot 呼叫不應該失敗")
}

/// scenario「vendored 資源」：pdf.js 函式庫主檔（`.mjs`）與 worker 皆為 JavaScript。
#[tokio::test]
async fn vendor_pdfjs_main_files_are_javascript() {
    let (_handle, state) = new_app_state();

    for path in [
        "/vendor/pdfjs/pdf.min.mjs",
        "/vendor/pdfjs/pdf.worker.min.mjs",
    ] {
        let response = get(http::router(state.clone()), path).await;
        assert_eq!(response.status(), StatusCode::OK, "{path} 應該回 200");
        assert_eq!(
            nosniff(&response).as_deref(),
            Some("nosniff"),
            "{path} 應該帶 nosniff"
        );
        let ct = content_type(&response);
        assert!(
            ct.starts_with("text/javascript"),
            "{path} 的 content-type 應以 text/javascript 開頭，實際: {ct:?}"
        );
        let body = body_bytes(response).await;
        assert!(!body.is_empty(), "{path} 的 body 不應為空");
    }
}

/// scenario「vendored 資源」：任一 `cmaps/` 檔為 `application/octet-stream`。
#[tokio::test]
async fn vendor_cmaps_file_is_octet_stream() {
    let (_handle, state) = new_app_state();
    let response = get(http::router(state), "/vendor/pdfjs/cmaps/78-EUC-H.bcmap").await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(nosniff(&response).as_deref(), Some("nosniff"));
    assert_eq!(content_type(&response), "application/octet-stream");
}

/// `.wasm` 一律 `application/wasm`（瀏覽器才會串流編譯）。
#[tokio::test]
async fn vendor_wasm_file_is_application_wasm() {
    let (_handle, state) = new_app_state();
    let response = get(http::router(state), "/vendor/pdfjs/wasm/jbig2.wasm").await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(nosniff(&response).as_deref(), Some("nosniff"));
    assert_eq!(content_type(&response), "application/wasm");
}

/// scenario「vendored 資源」：`/vendor/material-icons/icons/` 下的預設檔案 icon 為
/// `image/svg+xml`——預設檔案 icon 檔名從 `material-icons.json` 的 `file` 定義取，不寫死。
#[tokio::test]
async fn vendor_material_icons_default_file_icon_is_svg() {
    let (_handle, state) = new_app_state();

    let raw: serde_json::Value = serde_json::from_str(include_str!(
        "../assets/vendor/material-icons/material-icons.json"
    ))
    .expect("material-icons.json 應為合法 JSON");
    let default_key = raw["file"].as_str().expect("file 應存在");
    let icon_path = raw["iconDefinitions"][default_key]["iconPath"]
        .as_str()
        .expect("iconDefinitions.<file>.iconPath 應存在");
    let default_icon_name = icon_path
        .rsplit(['/', '\\'])
        .next()
        .expect("iconPath 應有檔名");

    let path = format!("/vendor/material-icons/icons/{default_icon_name}");
    let response = get(http::router(state), &path).await;
    assert_eq!(response.status(), StatusCode::OK, "{path} 應該回 200");
    assert_eq!(nosniff(&response).as_deref(), Some("nosniff"));
    assert_eq!(content_type(&response), "image/svg+xml");
}

/// 查無的 vendor 檔案回 404。
#[tokio::test]
async fn vendor_unknown_material_icon_is_404() {
    let (_handle, state) = new_app_state();
    let response = get(http::router(state), "/vendor/material-icons/不存在.svg").await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

/// 控制端裁決：跳出嘗試一律 404，不得讀到 vendor 目錄外的檔案。用 raw request（不經瀏覽器／axum
/// 正規化）直接打帶 `..`、`%2e%2e`／`%2F`、反斜線的路徑。
#[tokio::test]
async fn vendor_path_traversal_attempts_are_404() {
    let (_handle, state) = new_app_state();

    for path in [
        "/vendor/../Cargo.toml",
        "/vendor/pdfjs/..%2F..%2Fsrc%2Fhttp.rs",
        "/vendor/pdfjs/..\\..\\src\\http.rs",
    ] {
        let response = get(http::router(state.clone()), path).await;
        assert_eq!(
            response.status(),
            StatusCode::NOT_FOUND,
            "{path} 應該回 404（不得跳出 vendor 目錄）"
        );
    }
}

/// 只收 GET；不套用 `source_check`（vendor 是靜態資源，同 `/app/`）。
#[tokio::test]
async fn vendor_rejects_non_get_methods() {
    let (_handle, state) = new_app_state();
    let request = Request::builder()
        .method("POST")
        .uri("/vendor/pdfjs/pdf.min.mjs")
        .body(Body::empty())
        .expect("request 建構不應該失敗");
    let response = http::router(state)
        .oneshot(request)
        .await
        .expect("oneshot 呼叫不應該失敗");
    assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
}

// ---------------------------------------------------------------------------
// ws-source-check：`GET /api/state` 只接受本機同源請求（spec `cockpit-dashboard`「狀態端點只接受
// 本機同源請求」）。`AppState::new` 的 `port` 是 0，所以合法的 `Host` 是 `127.0.0.1:0` 等。
// ---------------------------------------------------------------------------

/// 以指定標頭（可重複同名）打 `/api/state`，回傳狀態碼與本體位元組。
async fn state_with_headers(headers: &[(&str, &str)]) -> (StatusCode, Vec<u8>) {
    let (_handle, state) = new_app_state();
    let mut builder = Request::builder().uri("/api/state");
    for (name, value) in headers {
        builder = builder.header(*name, *value);
    }
    let request = builder.body(Body::empty()).expect("request 建構不應該失敗");
    let response = http::router(state)
        .oneshot(request)
        .await
        .expect("oneshot 呼叫不應該失敗");
    let status = response.status();
    (status, body_bytes(response).await)
}

/// 被拒絕的回應：403、`code` 為 `forbidden_source`，本體不含投影內容（`version`、`runtimes`）。
fn assert_forbidden_source(case: &str, status: StatusCode, body: &[u8]) {
    assert_eq!(status, StatusCode::FORBIDDEN, "{case} 應該回 403");
    let parsed: serde_json::Value = serde_json::from_slice(body)
        .unwrap_or_else(|e| panic!("{case} 的 403 本體應該是 JSON：{e}"));
    assert_eq!(
        parsed["code"], "forbidden_source",
        "{case} 的 code 應該是 forbidden_source"
    );
    assert!(
        parsed.get("version").is_none() && parsed.get("runtimes").is_none(),
        "{case} 的本體不該含投影內容：{parsed}"
    );
}

/// Scenario「DNS rebinding 讀不到狀態」：`Host` 不是本機位址、缺 `Host`、重複 `Host` 都 403。
#[tokio::test]
async fn api_state_rejects_bad_host() {
    let cases: Vec<(&str, Vec<(&str, &str)>)> = vec![
        ("外站 Host", vec![("host", "evil.example:0")]),
        ("本機位址但埠不符", vec![("host", "127.0.0.1:7770")]),
        ("缺少 Host", vec![]),
        (
            "重複 Host（皆為合法值）",
            vec![("host", "127.0.0.1:0"), ("host", "127.0.0.1:0")],
        ),
        (
            "重複 Host（第二個是外站）",
            vec![("host", "127.0.0.1:0"), ("host", "evil.example:0")],
        ),
    ];
    for (case, headers) in cases {
        let (status, body) = state_with_headers(&headers).await;
        assert_forbidden_source(case, status, &body);
    }
}

/// 外站 `Origin`（即使 `Host` 合法）與重複 `Origin` 都 403。
#[tokio::test]
async fn api_state_rejects_bad_origin() {
    let cases: Vec<(&str, Vec<(&str, &str)>)> = vec![
        (
            "外站 Origin",
            vec![("host", "127.0.0.1:0"), ("origin", "https://evil.example")],
        ),
        (
            "Origin 與 Host 不同源",
            vec![("host", "127.0.0.1:0"), ("origin", "http://localhost:0")],
        ),
        (
            "重複 Origin",
            vec![
                ("host", "127.0.0.1:0"),
                ("origin", "http://127.0.0.1:0"),
                ("origin", "http://127.0.0.1:0"),
            ],
        ),
    ];
    for (case, headers) in cases {
        let (status, body) = state_with_headers(&headers).await;
        assert_forbidden_source(case, status, &body);
    }
}

/// Scenario「同源與命令列照常可用」：只帶 `Host: localhost:<port>`（命令列、桌面啟動器）與
/// `Host`＋相符 `Origin`（自家頁面）都回 200 與合法的投影 JSON。
#[tokio::test]
async fn api_state_accepts_same_origin_and_cli() {
    let cases: Vec<(&str, Vec<(&str, &str)>)> = vec![
        ("只帶 localhost Host", vec![("host", "localhost:0")]),
        ("只帶 127.0.0.1 Host", vec![("host", "127.0.0.1:0")]),
        ("只帶 [::1] Host", vec![("host", "[::1]:0")]),
        (
            "Host＋相符 Origin",
            vec![("host", "127.0.0.1:0"), ("origin", "http://127.0.0.1:0")],
        ),
    ];
    for (case, headers) in cases {
        let (status, body) = state_with_headers(&headers).await;
        assert_eq!(status, StatusCode::OK, "{case} 應該回 200");
        let _: ProjectedState = serde_json::from_slice(&body)
            .unwrap_or_else(|e| panic!("{case} 的本體應該是投影 JSON：{e}"));
    }
}

/// 以 `HEAD` 打 `/api/state`（axum 的 HEAD 借用 `get` 插槽，所以也要過來源檢查；日後若有人另外註冊
/// `.head(...)` 繞過它，這個測試會失敗）。
async fn head_state_status(headers: &[(&str, &str)]) -> StatusCode {
    let (_handle, state) = new_app_state();
    let mut builder = Request::builder().method("HEAD").uri("/api/state");
    for (name, value) in headers {
        builder = builder.header(*name, *value);
    }
    http::router(state)
        .oneshot(builder.body(Body::empty()).expect("request 建構不應該失敗"))
        .await
        .expect("oneshot 呼叫不應該失敗")
        .status()
}

/// `HEAD /api/state`：外站 `Host`、外站 `Origin`、重複 `Host` 都 403；合法 `Host` 回 200。
#[tokio::test]
async fn head_api_state_is_source_checked() {
    assert_eq!(
        head_state_status(&[("host", "evil.example:0")]).await,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        head_state_status(&[("host", "127.0.0.1:0"), ("origin", "https://evil.example")]).await,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        head_state_status(&[("host", "127.0.0.1:0"), ("host", "127.0.0.1:0")]).await,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        head_state_status(&[("host", "127.0.0.1:0")]).await,
        StatusCode::OK
    );
}

/// `GET /` 不得被外站網頁以 iframe 嵌入（防點擊劫持，也防外站 iframe 讓 `--exit-when-idle` 的後端
/// 因 iframe 內的 `/ws` 連線永遠不結束）：回應帶 `X-Frame-Options: DENY` 與
/// `Content-Security-Policy: frame-ancestors 'none'`。
#[tokio::test]
async fn index_forbids_framing() {
    let (_handle, state) = new_app_state();
    let response = http::router(state)
        .oneshot(
            Request::builder()
                .uri("/")
                .body(Body::empty())
                .expect("request 建構不應該失敗"),
        )
        .await
        .expect("oneshot 呼叫不應該失敗");
    assert_eq!(response.status(), StatusCode::OK);
    let header = |name: &str| {
        response
            .headers()
            .get(name)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned)
    };
    assert_eq!(header("x-frame-options").as_deref(), Some("DENY"));
    assert_eq!(
        header("content-security-policy").as_deref(),
        Some("frame-ancestors 'none'")
    );
}

/// 在 Rust 原始碼裡找出傳給 `coded_error_response(`／`coded_error_response_with_params(` 的第二個引數
/// 字串字面值（代碼）；第二個引數不是字面值的呼叫（例如 `error.code()`、函式定義本身）略過。
fn coded_response_literals(source: &str) -> Vec<String> {
    let mut codes = Vec::new();
    for name in ["coded_error_response(", "coded_error_response_with_params("] {
        for (at, _) in source.match_indices(name) {
            // 註解行與函式定義不算呼叫。
            let line_start = source[..at].rfind('\n').map_or(0, |i| i + 1);
            let prefix = source[line_start..at].trim_start();
            if prefix.starts_with("//") || prefix.ends_with("fn ") {
                continue;
            }
            let args = &source[at + name.len()..];
            let Some((_status, rest)) = args.split_once(',') else {
                continue;
            };
            let Some(rest) = rest.trim_start().strip_prefix('"') else {
                continue;
            };
            let Some((code, _)) = rest.split_once('"') else {
                continue;
            };
            codes.push(code.to_string());
        }
    }
    codes.sort();
    codes.dedup();
    codes
}

/// ui-language 階段審查 2 M2：後端能送出的每一個訊息代碼（錯誤本體與投影 `*_msg` 的 `code`）都要在
/// `i18n.js` 兩份字典有 `msg.<code>`，反過來字典的 `msg.*` 也都對得到一個後端代碼。代碼來源：
/// - `Rejection::code`、`WriteError::code`、`Message::msg`：以沒有萬用分支的 `match` 逐一列出每個變體，
///   新增變體時編譯失敗，逼人把它補進這裡，再由下面的字典斷言逼人補 `msg.<code>`；
/// - http／agent／來源檢查的固定代碼：掃 `src/http.rs`、`agent.rs`、`source_check.rs` 傳給
///   `coded_error_response*` 的字面值（新增一個固定代碼卻沒補字典會失敗）。
///
/// 檔案端點與 git 端點的代碼（`FileApiError`／`GitApiError`）不在這裡：它們有自己的前端 `ERROR_TEXT`
/// 表（files.js／git.js，鍵 `files.error.*`），由 `i18n-check.js` 的 ① 與 ② 逐一驗。
#[tokio::test]
async fn every_backend_message_code_has_a_msg_key_in_both_dictionaries() {
    use cockpit::progress_service::WriteError;
    use cockpit_core::{Message, ProjectId, Rejection, TaskId, WorkstreamId};
    use std::collections::BTreeSet;

    let s = |v: &str| v.to_string();

    // Rejection：Copy，直接列。
    let rejections = [
        Rejection::AlreadyLastStage,
        Rejection::AlreadyFirstStage,
        Rejection::AlreadyMarked,
        Rejection::TaskNotInWorkstream,
        Rejection::RuntimeNotRegistered,
        Rejection::RuntimeNotConnected,
        Rejection::PaneNotFound,
        Rejection::PaneExited,
    ];
    for r in &rejections {
        match r {
            Rejection::AlreadyLastStage
            | Rejection::AlreadyFirstStage
            | Rejection::AlreadyMarked
            | Rejection::TaskNotInWorkstream
            | Rejection::RuntimeNotRegistered
            | Rejection::RuntimeNotConnected
            | Rejection::PaneNotFound
            | Rejection::PaneExited => {}
        }
    }
    assert_eq!(
        rejections.len(),
        8,
        "Rejection 的變體數與上面的 match 對不上"
    );

    // WriteError：每個變體各一個（Internal 需要真的 JoinError）。
    let join_error = tokio::spawn(async { panic!("測試用：造出一個 JoinError") })
        .await
        .expect_err("panic 的 task 應該回 JoinError");
    let write_errors = vec![
        WriteError::UnknownProject(ProjectId::new("p")),
        WriteError::UnknownTask(TaskId::new("t")),
        WriteError::UnknownWorkstream(WorkstreamId::new("w")),
        WriteError::Rejected(Rejection::AlreadyMarked),
        WriteError::Persist {
            path: std::path::PathBuf::from("x.json"),
            source: std::io::Error::other("disk full"),
        },
        WriteError::PaneNotBound,
        WriteError::Internal(join_error),
    ];
    for e in &write_errors {
        match e {
            WriteError::UnknownProject(_)
            | WriteError::UnknownTask(_)
            | WriteError::UnknownWorkstream(_)
            | WriteError::Rejected(_)
            | WriteError::Persist { .. }
            | WriteError::PaneNotBound
            | WriteError::Internal(_) => {}
        }
    }
    assert_eq!(
        write_errors.len(),
        7,
        "WriteError 的變體數與上面的 match 對不上"
    );

    // Message：每個變體各一個。
    let messages = vec![
        Message::WslDistroNotRunning { distro: s("d") },
        Message::WslProbeFailed { detail: s("x") },
        Message::SnapshotFailed { detail: s("x") },
        Message::SeedSnapshotFailed { detail: s("x") },
        Message::LifecycleSubscribeFailed { detail: s("x") },
        Message::StatusSubscribeFailed { detail: s("x") },
        Message::StatusResubscribeFailed { detail: s("x") },
        Message::ProtocolUntested {
            protocol: s("1"),
            tested: s("2"),
        },
        Message::EventStreamEnded,
        Message::EventConnectionError {
            label: s("L"),
            detail: s("x"),
        },
        Message::EventConnectionEnded { label: s("S") },
        Message::TaskStageReset {
            task: s("t"),
            stage: s("a"),
            start: s("b"),
        },
        Message::DriftWorkspaceNotFound { id: s("w") },
        Message::DriftTabNotFound { id: s("t") },
        Message::DriftPaneNotFound { id: s("p") },
        Message::DriftRuntimeNotRegistered { id: s("r") },
        Message::EventPayloadUnparsable {
            event: s("e"),
            detail: s("x"),
        },
        Message::Raw { text: s("x") },
    ];
    for m in &messages {
        match m {
            Message::WslDistroNotRunning { .. }
            | Message::WslProbeFailed { .. }
            | Message::SnapshotFailed { .. }
            | Message::SeedSnapshotFailed { .. }
            | Message::LifecycleSubscribeFailed { .. }
            | Message::StatusSubscribeFailed { .. }
            | Message::StatusResubscribeFailed { .. }
            | Message::ProtocolUntested { .. }
            | Message::EventStreamEnded
            | Message::EventConnectionError { .. }
            | Message::EventConnectionEnded { .. }
            | Message::TaskStageReset { .. }
            | Message::DriftWorkspaceNotFound { .. }
            | Message::DriftTabNotFound { .. }
            | Message::DriftPaneNotFound { .. }
            | Message::DriftRuntimeNotRegistered { .. }
            | Message::EventPayloadUnparsable { .. }
            | Message::Raw { .. } => {}
        }
    }
    assert_eq!(messages.len(), 18, "Message 的變體數與上面的 match 對不上");

    let mut codes: BTreeSet<String> = BTreeSet::new();
    codes.extend(rejections.iter().map(|r| r.code().to_string()));
    codes.extend(write_errors.iter().map(|e| e.code().to_string()));
    codes.extend(messages.iter().map(|m| m.msg().code));

    // http／agent／來源檢查的固定代碼（掃原始碼）。掃描器要有牙齒：已知的九個都得抓到。
    let mut literals = coded_response_literals(include_str!("../src/http.rs"));
    literals.extend(coded_response_literals(include_str!("../src/agent.rs")));
    literals.extend(coded_response_literals(include_str!(
        "../src/source_check.rs"
    )));
    for known in [
        "invalid_op",
        "runtime_not_found",
        "pane_gone",
        "output_read_failed",
        "read_timeout",
        "method_not_allowed",
        "missing_pane_id",
        "pane_not_bound",
        "forbidden_source",
    ] {
        assert!(
            literals.iter().any(|c| c == known),
            "掃描 coded_error_response* 應該抓到固定代碼 {known}（實際 {literals:?}）"
        );
    }
    codes.extend(literals);

    let (_handle, state) = new_app_state();
    let response = http::router(state)
        .oneshot(
            Request::builder()
                .uri("/app/i18n.js")
                .body(Body::empty())
                .expect("request 建構不應該失敗"),
        )
        .await
        .expect("oneshot 呼叫不應該失敗");
    assert_eq!(response.status(), StatusCode::OK);
    let source = String::from_utf8(body_bytes(response).await).expect("body 應該是合法 UTF-8");
    let msg_keys = |marker: &str| -> BTreeSet<String> {
        i18n_dictionary(&source, marker)
            .into_iter()
            .filter_map(|(key, _)| key.strip_prefix("msg.").map(str::to_string))
            .collect()
    };
    let zh = msg_keys("var zh = {");
    let en = msg_keys("var en = {");

    let missing_zh: Vec<_> = codes.difference(&zh).collect();
    let missing_en: Vec<_> = codes.difference(&en).collect();
    assert!(
        missing_zh.is_empty() && missing_en.is_empty(),
        "後端會送出的代碼缺少字典鍵 msg.<code>（繁中缺 {missing_zh:?}、英文缺 {missing_en:?}）"
    );
    // 反向：字典的 msg.* 都對得到後端代碼（沒人送的鍵是死字串）。
    let dead_zh: Vec<_> = zh.difference(&codes).collect();
    let dead_en: Vec<_> = en.difference(&codes).collect();
    assert!(
        dead_zh.is_empty() && dead_en.is_empty(),
        "字典有後端不會送出的 msg.* 鍵（繁中 {dead_zh:?}、英文 {dead_en:?}）"
    );
    assert!(
        codes.len() > 30,
        "代碼清單應該有 30 個以上（實際 {}）",
        codes.len()
    );
}
