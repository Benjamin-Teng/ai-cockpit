//! `/vendor/{*path}` 內嵌第三方前端資源（file-review task 3.3；spec `cockpit-dashboard`「路由與內嵌
//! 資源」scenario「vendored 資源」「單一執行檔」；design D9）。
//!
//! `cockpit/assets/vendor/` 整棵樹以 [`include_dir!`] 內嵌進執行檔（[`VENDOR_DIR`]）——跟
//! `/app/`、`/icons/` 一樣不在執行期讀檔案系統、不用 `ServeDir`（design D13），差別是 vendor 的檔案
//! 樹是任意深度目錄（pdf.js 的 `cmaps/`、`standard_fonts/`、`wasm/`、`iccs/`，Material Icon Theme
//! 的 `icons/`），不能像 [`crate::http::app_asset`] 那樣窮舉成一個扁平 `match`，因此改用查表：
//! [`sanitize_path`] 先擋掉會跳出 vendor 目錄的片段，[`VENDOR_DIR::get_file`] 再查表，查無即 404。
//!
//! **這條路由不套用 [`crate::source_check::source_check`]**：vendor 內容跟 `/app/`、`/icons/` 一樣是
//! 純靜態、不含使用者資料的公開資源，沒有理由限制來源（design D9 只提「內嵌」，沒有比照檔案端點的
//! 授權模型）。也不检查 `HEAD`——`router()` 只掛 `get(vendor_asset)`，未註冊的 method（含 `HEAD`）
//! 交給 axum 內建的 405／自動 `HEAD`→`GET` 轉發處理，跟檔案端點刻意覆寫 `HEAD` 的做法不同（那是
//! file-review 檔案端點的規則，`/vendor/` 不是檔案端點）。
use axum::extract::Path;
use axum::http::{HeaderName, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use include_dir::{Dir, include_dir};

/// 整棵 `cockpit/assets/vendor/` 目錄樹，建置期內嵌（`$CARGO_MANIFEST_DIR` 指向 `cockpit/`，見
/// `cockpit/assets/vendor/README.md` 記錄的版本與取出方式）。
///
/// 同時供 [`crate::files::FileSettings::embedded`] 取用 `material-icons.json`——避免同一份
/// ~450 KiB 的檔案被 `include_bytes!` 與這裡的 `include_dir!` 各自內嵌一次。
pub static VENDOR_DIR: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/assets/vendor");

/// 擋掉會跳出 `VENDOR_DIR` 的相對路徑（控制端裁決）：
///
/// - 反斜線、NUL：Windows 上 `\` 可能被當成路徑分隔字元，NUL 對檔案系統無意義，一律拒絕。
/// - 逐段檢查空字串／`.`／`..`：擋掉連續 `/`、開頭或結尾 `/`（會產生空字串片段）、以及任何層級的
///   `..`。axum 的 `Path<String>` 對 `{*path}` 擷取到的整段字串做**一次性**百分比解碼
///   （`axum::routing::url_params::insert_url_params` → `PercentDecodedStr::new`），因此
///   `%2e%2e`、`%2F` 這類多重編碼在進入這個函式之前就已經還原成字面的 `..`、`/`——這裡只需要對
///   解碼後的結果做逐段檢查，不必自己再解一次百分比編碼。
///
/// `VENDOR_DIR` 本身只含 vendor 內容，沒有任何 `..` 條目，理論上 [`Dir::get_file`] 查表就查不到
/// 跳出目錄的路徑；这裡的檢查是縱深防禦（不依賴 `include_dir` 內部查表演算法的假設），並讓 3.2
/// 已經驗證過的跳出嘗試（見 `cockpit/tests/http.rs`）在這條路由上也有一致的行為。
fn sanitize_path(raw: &str) -> Option<&str> {
    if raw.is_empty() || raw.contains('\\') || raw.contains('\0') {
        return None;
    }
    if raw
        .split('/')
        .any(|segment| segment.is_empty() || segment == "." || segment == "..")
    {
        return None;
    }
    Some(raw)
}

/// 依副檔名決定 content-type（控制端裁決）：`.mjs`／`.js` 與既有 `/app/*.js`
/// （[`crate::http::app_asset`]）用同一個值，其餘型別各自對應，查無副檔名或不認得的副檔名（含
/// `.bcmap`、`.pfb`、`.ttf`、`.icc`、`.txt`、`LICENSE`）一律 `application/octet-stream`——這些都是
/// pdf.js／Material Icon Theme 執行時會讀取但瀏覽器不需要特殊解讀的資料檔。
fn content_type_for(path: &str) -> &'static str {
    let extension = std::path::Path::new(path)
        .extension()
        .and_then(|ext| ext.to_str());
    match extension.map(str::to_ascii_lowercase).as_deref() {
        Some("mjs") | Some("js") => "text/javascript; charset=utf-8",
        Some("svg") => "image/svg+xml",
        Some("json") => "application/json",
        Some("wasm") => "application/wasm",
        _ => "application/octet-stream",
    }
}

/// `GET /vendor/<path>`：從 [`VENDOR_DIR`] 查表，查無即 404；命中則依副檔名帶 content-type，並
/// 一律加 `X-Content-Type-Options: nosniff`（控制端裁決；理由同 live-output 的
/// [`crate::http::with_no_store_headers`]——JSON／SVG／JS 都不該被瀏覽器嗅探成別的型別）。
pub(crate) async fn vendor_asset(Path(path): Path<String>) -> Response {
    let Some(safe) = sanitize_path(&path) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let Some(file) = VENDOR_DIR.get_file(safe) else {
        return StatusCode::NOT_FOUND.into_response();
    };

    (
        [
            (
                header::CONTENT_TYPE,
                HeaderValue::from_static(content_type_for(safe)),
            ),
            (
                HeaderName::from_static("x-content-type-options"),
                HeaderValue::from_static("nosniff"),
            ),
        ],
        file.contents(),
    )
        .into_response()
}
