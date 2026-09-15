//! Task 3.7 驗收測試：PWA manifest 與圖示（spec `cockpit-dashboard`「PWA 可安裝」；
//! design D13）。
//!
//! 同 3.3（`cockpit/tests/http.rs`）用 `tower::ServiceExt::oneshot` 打 `router`，不開 port。

use axum::body::Body;
use axum::http::{Request, StatusCode};
use cockpit::http::{self, AppState};
use cockpit_core::{RuntimeStore, StoreHandle};
use http_body_util::BodyExt;
use serde_json::Value;
use tower::ServiceExt;

/// PNG 檔頭簽章（RFC 2083 §3.1）。
const PNG_SIGNATURE: [u8; 8] = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];

fn new_app_state() -> (StoreHandle, AppState) {
    let handle = StoreHandle::new(RuntimeStore::new());
    let state = AppState {
        state: handle.subscribe(),
    };
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

/// CRC32（IEEE 802.3，多項式 0xEDB88320，PNG 規格使用的同一種）：測試不能加依賴
/// （dev-dependencies 已固定），所以自己算一張 256 項查表，驗證 IHDR chunk 的 CRC
/// 沒被手滑改壞。
fn crc32(bytes: &[u8]) -> u32 {
    fn table_entry(n: u8) -> u32 {
        let mut c = n as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 {
                0xEDB88320 ^ (c >> 1)
            } else {
                c >> 1
            };
        }
        c
    }

    let mut crc: u32 = 0xFFFFFFFF;
    for &byte in bytes {
        let idx = ((crc ^ byte as u32) & 0xFF) as u8;
        crc = table_entry(idx) ^ (crc >> 8);
    }
    crc ^ 0xFFFFFFFF
}

/// spec「PWA 可安裝」節錄：manifest 含 `name`、`short_name`、`start_url` 為 `/`、
/// `display` 為 `standalone`、深色 `background_color`／`theme_color`、`icons` 含
/// 192 與 512 px PNG，兩個圖示 URL 都能取得且為 PNG。
#[tokio::test]
async fn manifest_has_required_fields_and_icon_urls() {
    let (_handle, state) = new_app_state();

    let response = get(http::router(state.clone()), "/manifest.webmanifest").await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(content_type(&response), "application/manifest+json");

    let body = body_bytes(response).await;
    let manifest: Value = serde_json::from_slice(&body).expect("manifest 應該是合法 JSON");

    assert!(
        manifest.get("name").and_then(Value::as_str).is_some(),
        "manifest 應該有 name"
    );
    assert!(
        manifest.get("short_name").and_then(Value::as_str).is_some(),
        "manifest 應該有 short_name"
    );
    assert_eq!(
        manifest.get("start_url").and_then(Value::as_str),
        Some("/"),
        "start_url 應該是 /"
    );
    assert_eq!(
        manifest.get("display").and_then(Value::as_str),
        Some("standalone"),
        "display 應該是 standalone"
    );

    for field in ["background_color", "theme_color"] {
        let color = manifest
            .get(field)
            .and_then(Value::as_str)
            .unwrap_or_else(|| panic!("manifest 應該有 {field}"));
        assert!(
            color.starts_with('#'),
            "{field} 應該是 # 開頭的色碼，實際: {color:?}"
        );
    }

    let icons = manifest
        .get("icons")
        .and_then(Value::as_array)
        .expect("manifest 應該有 icons 陣列");
    assert_eq!(icons.len(), 2, "icons 應該恰好兩筆");

    let mut sizes: Vec<&str> = icons
        .iter()
        .map(|icon| {
            assert_eq!(
                icon.get("type").and_then(Value::as_str),
                Some("image/png"),
                "每個 icon 的 type 應該是 image/png"
            );
            icon.get("sizes")
                .and_then(Value::as_str)
                .expect("每個 icon 應該有 sizes")
        })
        .collect();
    sizes.sort_unstable();
    assert_eq!(
        sizes,
        vec!["192x192", "512x512"],
        "icons 的 sizes 應該恰好是 192x192 與 512x512"
    );

    for icon in icons {
        let src = icon
            .get("src")
            .and_then(Value::as_str)
            .expect("每個 icon 應該有 src");
        let icon_response = get(http::router(state.clone()), src).await;
        assert_eq!(icon_response.status(), StatusCode::OK, "{src} 應該回 200");
        assert_eq!(
            content_type(&icon_response),
            "image/png",
            "{src} 的 content-type 應該是 image/png"
        );
        let icon_body = body_bytes(icon_response).await;
        assert!(
            icon_body.starts_with(&PNG_SIGNATURE),
            "{src} 的 body 前 8 bytes 應該是 PNG 簽章"
        );
    }
}

/// spec 節錄：圖示要是真的 192／512 px PNG，不是佔位圖；額外驗 IHDR chunk 的
/// CRC32 沒被改壞（PNG 規格要求每個 chunk 都要有正確 CRC，壞掉的圖示在部分瀏覽器
/// 會被直接拒收）。
#[tokio::test]
async fn icons_are_png_with_declared_dimensions() {
    let (_handle, state) = new_app_state();

    for (path, expected_size) in [
        ("/icons/icon-192.png", 192u32),
        ("/icons/icon-512.png", 512u32),
    ] {
        let response = get(http::router(state.clone()), path).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = body_bytes(response).await;

        assert!(
            body.starts_with(&PNG_SIGNATURE),
            "{path} 的 body 前 8 bytes 應該是 PNG 簽章"
        );
        assert!(body.len() >= 33, "{path} 的 body 太短，容不下 IHDR chunk");

        let width = u32::from_be_bytes(body[16..20].try_into().unwrap());
        let height = u32::from_be_bytes(body[20..24].try_into().unwrap());
        assert_eq!(width, expected_size, "{path} 的寬應該是 {expected_size}");
        assert_eq!(height, expected_size, "{path} 的高應該是 {expected_size}");

        // IHDR chunk：bytes 12..29 是 "IHDR" + 13 bytes 資料，bytes 29..33 是該
        // chunk 的 CRC32（over type + data）。
        let ihdr_crc_input = &body[12..29];
        let stored_crc = u32::from_be_bytes(body[29..33].try_into().unwrap());
        assert_eq!(
            crc32(ihdr_crc_input),
            stored_crc,
            "{path} 的 IHDR CRC32 應該正確"
        );
    }
}

/// spec 節錄：`index.html` 以 `<link rel="manifest">` 引用 manifest。
#[tokio::test]
async fn index_links_manifest() {
    let (_handle, state) = new_app_state();

    let response = get(http::router(state), "/").await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = body_bytes(response).await;
    let text = String::from_utf8(body).expect("body 應該是合法 UTF-8");

    assert!(
        text.contains(r#"rel="manifest""#),
        "index.html 應該有 rel=\"manifest\""
    );
    assert!(
        text.contains("/manifest.webmanifest"),
        "index.html 應該引用 /manifest.webmanifest"
    );
}
