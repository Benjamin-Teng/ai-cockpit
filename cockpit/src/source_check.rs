//! 寫入端點的來源檢查 middleware（spec `pipeline-progress`「寫入端點只接受本機同源請求」；
//! design D6；task 4.2）。
//!
//! 只套在 [`crate::http::router`] 內的兩個寫入路由（`POST .../{op}`、`PUT`／`DELETE
//! .../override`），用 `MethodRouter::route_layer` 而不是套在整個 `Router` 上——
//! `route_layer` 只包住已註冊的方法插槽（`post`／`put`／`delete`），沒註冊的方法（例如對這兩
//! 個路徑送 `GET`）落到 axum 內建的 fallback，直接照舊回 405，不會先經過這裡（axum
//! `method_routing.rs::route_layer` 的實作只 map `get`／`head`／`delete`／…／`connect` 這幾個
//! 插槽，不動 `fallback`）。`/api/state`、`/ws` 等讀路由完全沒有掛這層，同樣不受影響。
//!
//! 規則（spec 原文；design D6）：`Host` 標頭必須是 `127.0.0.1:<port>`／`localhost:<port>`／
//! `[::1]:<port>` 三者之一，`<port>` 是 [`crate::http::AppState::port`] 當下的值（每個請求都
//! 重新讀 `Arc<AtomicU16>`，不是 middleware 建構時的快照——服務真正 bind 之後靠同一個 `Arc`
//! 回填實際監聽埠，這裡要立刻認新值，`port = 0` 的測試環境也要正確比對）；有 `Origin` 標頭
//! 時，其值必須逐字等於 `http://` 加上前面驗證通過的那個 `Host` 值。任何一項不符合就回
//! 403、完全不進到 handler；本體沿用 [`crate::http::error_response`] 的 `{"error": ...}`
//! 慣例，不另開一套錯誤格式。
//!
//! 否決：不額外正規化 `Host`（大小寫、省略埠號等）——spec 要求的是「必須是」三種寫法之一
//! 加實際埠，字面比對已經足夠嚴謹，放寬比對只會擴大攻擊面而不會修到任何已知案例。
//!
//! Fix round 1（Codex finding）：`Host`／`Origin` 都用 [`exactly_one`] 取值，不是
//! `HeaderMap::get`——`get` 只回第一個值，HTTP 允許同名標頭重複出現，重複時哪一個是
//! 「真正」的 `Host`／`Origin` 沒有標準答案，只看第一個等於放任第二個完全不受檢查；一律當
//! 成不可信任、直接 403，不嘗試猜測該信哪一個。

use std::sync::atomic::Ordering;

use axum::extract::{Request, State};
use axum::http::{HeaderMap, HeaderName, StatusCode, header};
use axum::middleware::Next;
use axum::response::Response;

use crate::http::{AppState, error_response};

/// 檢查這次請求的 `Host`／`Origin`；不符合直接回 403，符合就放行給下一層（真正的 handler）。
pub async fn source_check(State(app): State<AppState>, request: Request, next: Next) -> Response {
    let port = app.port.load(Ordering::Relaxed);

    let host = match exactly_one(request.headers(), header::HOST) {
        Ok(Some(host)) => host,
        Ok(None) => return error_response(StatusCode::FORBIDDEN, "缺少合法的 Host 標頭"),
        Err(()) => {
            return error_response(StatusCode::FORBIDDEN, "Host 標頭重複或不是合法字串");
        }
    };

    if !host_is_loopback(&host, port) {
        return error_response(
            StatusCode::FORBIDDEN,
            &format!("Host 不是本機位址（含實際監聽埠 {port}）：{host}"),
        );
    }

    match exactly_one(request.headers(), header::ORIGIN) {
        Ok(None) => {}
        Ok(Some(origin)) => {
            let expected = format!("http://{host}");
            if origin != expected {
                return error_response(
                    StatusCode::FORBIDDEN,
                    &format!("Origin（{origin}）與 Host 不符，預期 {expected}"),
                );
            }
        }
        Err(()) => {
            return error_response(StatusCode::FORBIDDEN, "Origin 標頭重複或不是合法字串");
        }
    }

    next.run(request).await
}

/// 從標頭裡挑出「剛好一個」`name` 的值：完全沒有回 `Ok(None)`；剛好一個且是合法字串回
/// `Ok(Some(value))`；出現兩次以上（不管值是否相同）或不是合法字串一律 `Err(())`——呼叫端
/// 統一當成「不可信任」回 403（fix round 1：不能只看 `HeaderMap::get` 的第一個值）。
fn exactly_one(headers: &HeaderMap, name: HeaderName) -> Result<Option<String>, ()> {
    let mut values = headers.get_all(name).iter();
    let Some(first) = values.next() else {
        return Ok(None);
    };
    if values.next().is_some() {
        return Err(());
    }
    first.to_str().map(str::to_owned).map(Some).map_err(|_| ())
}

/// `Host` 是否為 loopback 三種寫法之一，且埠與 `port` 逐字相符（spec 原文；design D6）。
fn host_is_loopback(host: &str, port: u16) -> bool {
    host == format!("127.0.0.1:{port}")
        || host == format!("localhost:{port}")
        || host == format!("[::1]:{port}")
}

#[cfg(test)]
mod tests {
    use axum::http::{HeaderMap, HeaderValue, header};

    use super::{exactly_one, host_is_loopback};

    #[test]
    fn accepts_three_loopback_forms_with_matching_port() {
        assert!(host_is_loopback("127.0.0.1:7770", 7770));
        assert!(host_is_loopback("localhost:7770", 7770));
        assert!(host_is_loopback("[::1]:7770", 7770));
    }

    #[test]
    fn rejects_wrong_port_or_non_loopback_host() {
        assert!(!host_is_loopback("127.0.0.1:7771", 7770));
        assert!(!host_is_loopback("evil.example:7770", 7770));
        assert!(!host_is_loopback("127.0.0.1", 7770));
        assert!(!host_is_loopback("localhost", 0));
    }

    #[test]
    fn exactly_one_absent_is_none() {
        let headers = HeaderMap::new();
        assert_eq!(exactly_one(&headers, header::HOST), Ok(None));
    }

    #[test]
    fn exactly_one_single_value_is_some() {
        let mut headers = HeaderMap::new();
        headers.insert(header::HOST, HeaderValue::from_static("127.0.0.1:7770"));
        assert_eq!(
            exactly_one(&headers, header::HOST),
            Ok(Some("127.0.0.1:7770".to_string()))
        );
    }

    #[test]
    fn exactly_one_duplicate_is_err() {
        let mut headers = HeaderMap::new();
        headers.insert(header::HOST, HeaderValue::from_static("127.0.0.1:7770"));
        headers.append(header::HOST, HeaderValue::from_static("evil.example:7770"));
        assert_eq!(exactly_one(&headers, header::HOST), Err(()));
    }
}
