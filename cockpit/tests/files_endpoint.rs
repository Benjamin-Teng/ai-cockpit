//! file-review task 3.1 驗收測試：pane → 檔案根目錄、允許清單、根目錄查詢端點（spec `file-review`
//! 「檔案根目錄與允許清單」「檔案端點的共同規則」「根目錄查詢端點」；design D1、D2、D10）。
//!
//! 同 `cockpit/tests/output_endpoint.rs` 用 `tower::ServiceExt::oneshot` 打 `router`，不開 port；
//! `AppState.port` 固定 0，請求帶 `Host: 127.0.0.1:0` 才能通過來源檢查。投影不經狀態庫，直接用
//! `watch::channel` 灌一份手組的 `ProjectedState`，之後 `send` 新的一份模擬「pane 關掉」——允許清單
//! 每次請求都讀 `watch::Receiver` 的最新值，這正是要驗的行為。檔案系統用自製 `TempDir`（同
//! `cockpit/tests/config.rs`，不加 `tempfile`）。這個檔不對 HERDR 送任何請求：`build_components`
//! 那個測試的 runtime 端點是一個不存在的指令（同 `cockpit/tests/app.rs::unreachable_config`）。

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::AtomicU16;
use std::time::{SystemTime, UNIX_EPOCH};

use axum::body::Body;
use axum::http::{HeaderMap, Request, StatusCode};
use cockpit::app;
use cockpit::config::{Config, ConfigSource, PollingConfig, RuntimeConfig, ServerConfig};
use cockpit::files::{
    FileApiError, FileSettings, PathMapping, RAW_SIZE_LIMIT, authorize_root, decode_root_id,
    encode_root_id, files_error_response, vscode_uri, wsl_host_path,
};
use cockpit::http::{self, AppState};
use cockpit_core::{ProjectedState, RuntimeId};
use cockpit_files::Root;
use cockpit_herdr::HerdrEndpoint;
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tokio::sync::watch;
use tower::ServiceExt;

// ---------------------------------------------------------------------------
// 鷹架
// ---------------------------------------------------------------------------

/// 測試用暫存目錄（同 `cockpit/tests/config.rs` 的自製版本；drop 時整個刪掉）。
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
            "cockpit-files-test-{tag}-{}-{nanos}",
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

/// 在暫存目錄裡做一個 `repo/.git`（資料夾）＋`repo/src` 的假 repo（spec「查到根目錄」的
/// `D:\repo\src` 形狀），回傳 `(repo 根, src)`。
fn make_repo(tmp: &TempDir) -> (PathBuf, PathBuf) {
    let root = tmp.path().join("repo");
    fs::create_dir_all(root.join(".git")).expect("建立 .git 資料夾");
    let src = root.join("src");
    fs::create_dir_all(&src).expect("建立 src");
    (root, src)
}

fn path_str(path: &Path) -> &str {
    path.to_str().expect("暫存路徑應是合法 UTF-8")
}

/// 一個 pane 的描述：`(pane id, cwd, exited)`。
type PaneSpec<'a> = (&'a str, Option<&'a str>, bool);

/// 手組一份投影：每個 runtime 一個 workspace、一個 tab，底下放給定的 pane。
fn projected(runtimes: &[(&str, &[PaneSpec<'_>])]) -> Arc<ProjectedState> {
    let runtimes: Vec<Value> = runtimes
        .iter()
        .map(|(id, panes)| {
            let panes: Vec<Value> = panes
                .iter()
                .map(|(pane, cwd, exited)| {
                    json!({
                        "id": pane,
                        "agent": null,
                        "agent_status": "idle",
                        "title": null,
                        "cwd": cwd,
                        "label": null,
                        "focused": false,
                        "exited": exited,
                        "updated_at": "2026-09-27T00:00:00Z"
                    })
                })
                .collect();
            json!({
                "id": id,
                "kind": "herdr",
                "endpoint": "test",
                "connection": { "state": "connecting" },
                "focused": { "workspace_id": null, "tab_id": null, "pane_id": null },
                "workspaces": [{
                    "id": "w1",
                    "label": null,
                    "number": 1,
                    "agent_status": "idle",
                    "focused": false,
                    "tabs": [{
                        "id": "w1:t1",
                        "number": 1,
                        "agent_status": "idle",
                        "focused": false,
                        "panes": panes
                    }]
                }]
            })
        })
        .collect();
    let state = json!({
        "version": 1,
        "generated_at": "2026-09-27T00:00:00Z",
        "runtimes": runtimes,
        "projects": [],
        "recent_events": []
    });
    Arc::new(serde_json::from_value(state).expect("手組投影應能反序列化"))
}

/// 組 `AppState`：投影由呼叫端給（回傳 `watch::Sender` 讓測試之後換掉），runtime 路徑對應表由
/// `mappings` 給；`runtimes`（輸出端點用的 `AgentRuntime` 表）留空——檔案端點不碰它。
fn build(
    initial: Arc<ProjectedState>,
    mappings: &[(&str, PathMapping)],
) -> (watch::Sender<Arc<ProjectedState>>, AppState) {
    let (tx, rx) = watch::channel(initial);
    let path_mappings: HashMap<RuntimeId, PathMapping> = mappings
        .iter()
        .map(|(id, mapping)| (RuntimeId::new(*id), mapping.clone()))
        .collect();
    let app = AppState {
        state: rx,
        progress: None,
        port: Arc::new(AtomicU16::new(0)),
        runtimes: Arc::new(HashMap::new()),
        path_mappings: Arc::new(path_mappings),
        files: Arc::new(FileSettings::embedded()),
    };
    (tx, app)
}

fn root_uri(runtime: &str, pane: &str) -> String {
    format!("/api/runtimes/{runtime}/panes/{pane}/root")
}

struct Reply {
    status: StatusCode,
    headers: HeaderMap,
    raw: String,
    json: Value,
}

async fn send(router: &axum::Router, method: &str, uri: &str) -> Reply {
    send_with_headers(router, method, uri, &[("host", "127.0.0.1:0")]).await
}

async fn send_with_headers(
    router: &axum::Router,
    method: &str,
    uri: &str,
    headers: &[(&str, &str)],
) -> Reply {
    let mut builder = Request::builder().method(method).uri(uri);
    for (name, value) in headers {
        builder = builder.header(*name, *value);
    }
    let request = builder.body(Body::empty()).expect("request 建構不應該失敗");
    let response = router
        .clone()
        .oneshot(request)
        .await
        .expect("oneshot 呼叫不應該失敗");
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("body 收集不應該失敗")
        .to_bytes();
    let raw = String::from_utf8_lossy(&bytes).into_owned();
    let json = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    Reply {
        status,
        headers,
        raw,
        json,
    }
}

/// 所有回應（含錯誤）都要帶的兩個標頭（spec「檔案端點的共同規則」）。
fn assert_security_headers(reply: &Reply) {
    assert_eq!(
        reply
            .headers
            .get("cache-control")
            .and_then(|v| v.to_str().ok()),
        Some("no-store"),
        "缺 Cache-Control: no-store（status {}）",
        reply.status
    );
    assert_eq!(
        reply
            .headers
            .get("x-content-type-options")
            .and_then(|v| v.to_str().ok()),
        Some("nosniff"),
        "缺 X-Content-Type-Options: nosniff（status {}）",
        reply.status
    );
}

/// 非 200 回應：狀態碼、`code`、非空中文 `error`、兩個標頭，且本體不含 `forbidden` 列出的請求
/// 路徑片段原文。
fn assert_coded_error(reply: &Reply, status: StatusCode, code: &str, forbidden: &[&str]) {
    assert_eq!(reply.status, status, "本體：{}", reply.raw);
    assert_eq!(reply.json["code"], code, "本體：{}", reply.raw);
    assert!(
        reply.json["error"].as_str().is_some_and(|s| !s.is_empty()),
        "error 應為非空字串：{}",
        reply.raw
    );
    for fragment in forbidden {
        assert!(
            !reply.raw.contains(fragment),
            "錯誤本體不得含請求路徑片段原文 {fragment:?}：{}",
            reply.raw
        );
    }
    assert_security_headers(reply);
}

const WIN: &str = "win";

// ---------------------------------------------------------------------------
// wsl_host_path（純函式；spec「WSL 路徑轉換」）
// ---------------------------------------------------------------------------

/// Scenario: WSL 路徑轉換
#[test]
fn wsl_host_path_scenario_converts_posix_to_wsl_unc() {
    assert_eq!(
        wsl_host_path("Ubuntu-24.04", "/home/u/repo"),
        Some(PathBuf::from(r"\\wsl.localhost\Ubuntu-24.04\home\u\repo"))
    );
}

#[test]
fn wsl_host_path_rejects_paths_not_starting_with_slash() {
    for posix in ["", "home/u", "~/repo", r"C:\repo", r"\home\u", "./x"] {
        assert_eq!(wsl_host_path("Ubuntu", posix), None, "{posix:?}");
    }
}

#[test]
fn wsl_host_path_root_slash_is_distro_share_root() {
    assert_eq!(
        wsl_host_path("Ubuntu", "/"),
        Some(PathBuf::from(r"\\wsl.localhost\Ubuntu\"))
    );
}

#[test]
fn wsl_host_path_ignores_empty_segments_from_repeated_or_trailing_slashes() {
    let expected = Some(PathBuf::from(r"\\wsl.localhost\Ubuntu\home\u\repo"));
    for posix in ["/home/u/repo/", "//home//u///repo", "/home/u/repo//"] {
        assert_eq!(wsl_host_path("Ubuntu", posix), expected, "{posix:?}");
    }
}

#[test]
fn wsl_host_path_rejects_dot_and_dotdot_segments() {
    for posix in ["/home/u/..", "/home/../etc", "/home/./u", "/.", "/.."] {
        assert_eq!(wsl_host_path("Ubuntu", posix), None, "{posix:?}");
    }
}

#[test]
fn wsl_host_path_rejects_characters_windows_cannot_represent_in_a_segment() {
    for posix in [
        r"/home/u\repo",
        "/home/u:repo",
        "/home/u\0repo",
        "/home/u*",
        "/home/u?",
        "/home/\"u\"",
        "/home/<u>",
        "/home/u|v",
        "/home/u\tv",
        "/home/u\x1fv",
    ] {
        assert_eq!(wsl_host_path("Ubuntu", posix), None, "{posix:?}");
    }
}

#[test]
fn wsl_host_path_rejects_segments_ending_with_dot_or_space() {
    for posix in ["/home/u/repo.", "/home/u/repo ", "/home/u. /x"] {
        assert_eq!(wsl_host_path("Ubuntu", posix), None, "{posix:?}");
    }
}

#[test]
fn wsl_host_path_keeps_non_ascii_and_ordinary_punctuation() {
    assert_eq!(
        wsl_host_path("Ubuntu", "/home/u/專案 a.b-c_d@e"),
        Some(PathBuf::from(
            r"\\wsl.localhost\Ubuntu\home\u\專案 a.b-c_d@e"
        ))
    );
}

#[test]
fn wsl_host_path_rejects_unusable_distro_names() {
    for distro in ["", r"a\b", "a/b", ".", "..", "a:b", "a?"] {
        assert_eq!(wsl_host_path(distro, "/home/u"), None, "{distro:?}");
    }
}

// ---------------------------------------------------------------------------
// root_id 編解碼（design D2）
// ---------------------------------------------------------------------------

#[test]
fn root_id_is_lowercase_hex_of_utf8_bytes_and_round_trips() {
    let path = Path::new(r"D:\repo\專");
    let id = encode_root_id(path).expect("UTF-8 路徑應可編碼");
    let expected: String = r"D:\repo\專".bytes().map(|b| format!("{b:02x}")).collect();
    assert_eq!(id, expected);
    assert!(
        id.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    );
    assert_eq!(decode_root_id(&id), Ok(PathBuf::from(r"D:\repo\專")));
}

#[test]
fn decode_root_id_rejects_malformed_input() {
    for bad in ["", "0", "abc", "zz", "4G", "4A", "c328", "ff"] {
        assert_eq!(
            decode_root_id(bad),
            Err(FileApiError::BadRequest),
            "{bad:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// 根目錄查詢端點（spec「根目錄查詢端點」）
// ---------------------------------------------------------------------------

/// Scenario: 查到根目錄
#[tokio::test]
async fn root_endpoint_scenario_finds_git_root_above_cwd() {
    let tmp = TempDir::new("found");
    let (root, src) = make_repo(&tmp);
    let (_tx, app) = build(
        projected(&[(WIN, &[("w1:p1", Some(path_str(&src)), false)])]),
        &[(WIN, PathMapping::Native)],
    );
    let router = http::router(app);

    let reply = send(&router, "GET", &root_uri(WIN, "w1:p1")).await;

    assert_eq!(reply.status, StatusCode::OK, "本體：{}", reply.raw);
    assert_security_headers(&reply);
    let body = &reply.json;
    assert_eq!(body["runtime"], WIN);
    assert_eq!(body["pane_id"], "w1:p1");
    assert_eq!(body["root_path"], path_str(&root));
    assert_eq!(body["cwd_path"], path_str(&src));
    assert_eq!(body["name"], "repo");
    assert_eq!(body["is_git"], true);
    assert_eq!(
        body["root_id"],
        encode_root_id(&root).expect("可編碼"),
        "root_id 應為 root_path 的十六進位編碼"
    );

    // 同一個根目錄在任何時候得到相同的值。
    let again = send(&router, "GET", &root_uri(WIN, "w1:p1")).await;
    assert_eq!(again.json["root_id"], body["root_id"]);
}

#[tokio::test]
async fn root_endpoint_non_git_cwd_is_its_own_root() {
    let tmp = TempDir::new("nongit");
    let notes = tmp.path().join("notes");
    fs::create_dir_all(&notes).expect("建立 notes");
    let (_tx, app) = build(
        projected(&[(WIN, &[("w1:p1", Some(path_str(&notes)), false)])]),
        &[(WIN, PathMapping::Native)],
    );
    let reply = send(&http::router(app), "GET", &root_uri(WIN, "w1:p1")).await;

    assert_eq!(reply.status, StatusCode::OK, "本體：{}", reply.raw);
    // 暫存目錄上層理論上沒有 `.git`；若這台機器真的有，這個斷言會先壞並說明原因。
    assert_eq!(reply.json["root_path"], path_str(&notes));
    assert_eq!(reply.json["name"], "notes");
    assert_eq!(reply.json["is_git"], false);
}

/// Scenario: pane 沒有 cwd
#[tokio::test]
async fn root_endpoint_scenario_pane_without_cwd_is_no_root() {
    let (_tx, app) = build(
        projected(&[(WIN, &[("w1:p2", None, false)])]),
        &[(WIN, PathMapping::Native)],
    );
    let reply = send(&http::router(app), "GET", &root_uri(WIN, "w1:p2")).await;
    assert_coded_error(&reply, StatusCode::NOT_FOUND, "no_root", &["w1:p2"]);
}

#[tokio::test]
async fn root_endpoint_missing_cwd_directory_is_no_root() {
    let tmp = TempDir::new("gone");
    let gone = tmp.path().join("does-not-exist");
    let (_tx, app) = build(
        projected(&[(WIN, &[("w1:p1", Some(path_str(&gone)), false)])]),
        &[(WIN, PathMapping::Native)],
    );
    let reply = send(&http::router(app), "GET", &root_uri(WIN, "w1:p1")).await;
    assert_coded_error(
        &reply,
        StatusCode::NOT_FOUND,
        "no_root",
        &["does-not-exist"],
    );
}

#[tokio::test]
async fn root_endpoint_relative_native_cwd_is_no_root() {
    // 相對路徑會被解讀成相對於 cockpit 自己的工作目錄——不是 pane 的目錄，一律視為沒有根目錄。
    let (_tx, app) = build(
        projected(&[(WIN, &[("w1:p1", Some("src"), false)])]),
        &[(WIN, PathMapping::Native)],
    );
    let reply = send(&http::router(app), "GET", &root_uri(WIN, "w1:p1")).await;
    assert_coded_error(&reply, StatusCode::NOT_FOUND, "no_root", &[]);
}

#[tokio::test]
async fn root_endpoint_native_cwd_with_parent_dir_component_is_no_root() {
    let tmp = TempDir::new("dotdot");
    let (_root, src) = make_repo(&tmp);
    let weird = format!(r"{}\..\src", path_str(&src));
    let (_tx, app) = build(
        projected(&[(WIN, &[("w1:p1", Some(&weird), false)])]),
        &[(WIN, PathMapping::Native)],
    );
    let reply = send(&http::router(app), "GET", &root_uri(WIN, "w1:p1")).await;
    assert_coded_error(&reply, StatusCode::NOT_FOUND, "no_root", &[]);
}

#[tokio::test]
async fn root_endpoint_wsl_runtime_with_non_posix_cwd_is_no_root() {
    // WSL runtime 的 cwd 一定要經 `wsl_host_path`；不是 `/` 開頭（例如誤報一個 Windows 路徑）
    // 就沒有根目錄，不會退回「原樣當主機路徑」。
    let tmp = TempDir::new("wsl-nonposix");
    let (_root, src) = make_repo(&tmp);
    let (_tx, app) = build(
        projected(&[("wsl", &[("w1:p1", Some(path_str(&src)), false)])]),
        &[(
            "wsl",
            PathMapping::Wsl {
                distro: "Ubuntu".to_string(),
            },
        )],
    );
    let reply = send(&http::router(app), "GET", &root_uri("wsl", "w1:p1")).await;
    assert_coded_error(&reply, StatusCode::NOT_FOUND, "no_root", &[]);
}

#[tokio::test]
async fn root_endpoint_unknown_runtime_is_runtime_unknown() {
    let (_tx, app) = build(
        projected(&[(WIN, &[("w1:p1", None, false)])]),
        &[(WIN, PathMapping::Native)],
    );
    let reply = send(&http::router(app), "GET", &root_uri("nosuchrt", "w1:p1")).await;
    assert_coded_error(
        &reply,
        StatusCode::NOT_FOUND,
        "runtime_unknown",
        &["nosuchrt", "w1:p1"],
    );
}

#[tokio::test]
async fn root_endpoint_unknown_pane_is_pane_unknown() {
    let (_tx, app) = build(
        projected(&[(WIN, &[("w1:p1", None, false)])]),
        &[(WIN, PathMapping::Native)],
    );
    let reply = send(&http::router(app), "GET", &root_uri(WIN, "nosuchpane")).await;
    assert_coded_error(
        &reply,
        StatusCode::NOT_FOUND,
        "pane_unknown",
        &["nosuchpane"],
    );
}

#[tokio::test]
async fn root_endpoint_pane_of_another_runtime_is_pane_unknown() {
    let (_tx, app) = build(
        projected(&[(WIN, &[]), ("other", &[("w1:p9", None, false)])]),
        &[(WIN, PathMapping::Native), ("other", PathMapping::Native)],
    );
    let reply = send(&http::router(app), "GET", &root_uri(WIN, "w1:p9")).await;
    assert_coded_error(&reply, StatusCode::NOT_FOUND, "pane_unknown", &[]);
}

#[tokio::test]
async fn root_endpoint_non_utf8_path_segment_is_bad_request() {
    let (_tx, app) = build(projected(&[(WIN, &[])]), &[(WIN, PathMapping::Native)]);
    let reply = send(
        &http::router(app),
        "GET",
        "/api/runtimes/win/panes/%FF/root",
    )
    .await;
    assert_coded_error(&reply, StatusCode::BAD_REQUEST, "bad_request", &["%FF"]);
}

#[tokio::test]
async fn root_endpoint_rejects_other_methods_with_405() {
    let (_tx, app) = build(
        projected(&[(WIN, &[("w1:p1", None, false)])]),
        &[(WIN, PathMapping::Native)],
    );
    let router = http::router(app);

    for method in ["POST", "PUT", "DELETE", "PATCH"] {
        let reply = send(&router, method, &root_uri(WIN, "w1:p1")).await;
        assert_coded_error(
            &reply,
            StatusCode::METHOD_NOT_ALLOWED,
            "method_not_allowed",
            &[],
        );
    }
}

#[tokio::test]
async fn root_endpoint_head_is_405_without_body_regardless_of_source() {
    let (_tx, app) = build(
        projected(&[(WIN, &[("w1:p1", None, false)])]),
        &[(WIN, PathMapping::Native)],
    );
    let router = http::router(app);

    for headers in [
        vec![("host", "127.0.0.1:0")],
        vec![("host", "evil.example:0")],
        vec![("host", "127.0.0.1:0"), ("origin", "http://evil.example")],
    ] {
        let reply = send_with_headers(&router, "HEAD", &root_uri(WIN, "w1:p1"), &headers).await;
        assert_eq!(reply.status, StatusCode::METHOD_NOT_ALLOWED, "{headers:?}");
        assert!(reply.raw.is_empty(), "HEAD 不應有本體：{:?}", reply.raw);
        assert_security_headers(&reply);
    }
}

#[tokio::test]
async fn root_endpoint_rejects_foreign_host_and_origin_with_403() {
    let tmp = TempDir::new("source");
    let (_root, src) = make_repo(&tmp);
    let (_tx, app) = build(
        projected(&[(WIN, &[("w1:p1", Some(path_str(&src)), false)])]),
        &[(WIN, PathMapping::Native)],
    );
    let router = http::router(app);

    for headers in [
        vec![("host", "evil.example:0")],
        vec![("host", "127.0.0.1:0"), ("origin", "http://evil.example")],
        vec![],
    ] {
        let reply = send_with_headers(&router, "GET", &root_uri(WIN, "w1:p1"), &headers).await;
        assert_eq!(
            reply.status,
            StatusCode::FORBIDDEN,
            "{headers:?}：{}",
            reply.raw
        );
        assert!(
            reply.json.get("root_path").is_none(),
            "403 不應帶出根目錄：{}",
            reply.raw
        );
        assert_security_headers(&reply);
    }
}

/// 組裝層接線：`build_components` 必須把設定裡每一筆 runtime 放進路徑對應表，否則真實服務對
/// 每個 runtime 都會回 `runtime_unknown`。端點是不存在的指令，不會碰到 HERDR。
#[tokio::test]
async fn build_components_registers_every_configured_runtime_for_files() {
    let config = Config {
        server: ServerConfig {
            listen: "127.0.0.1:0".parse().expect("測試位址應可解析"),
        },
        polling: PollingConfig {
            resnapshot_secs: 30,
            wsl_probe_secs: 60,
        },
        runtimes: vec![RuntimeConfig {
            id: "local".to_string(),
            kind: "herdr".to_string(),
            endpoint: HerdrEndpoint::Command(vec!["cockpit-test-no-such-command".to_string()]),
        }],
        projects: Vec::new(),
        state_path: None,
        source: ConfigSource::Inline,
    };
    let components = app::build_components(&config).expect("組裝應該成功");

    let reply = send(&components.router, "GET", &root_uri("local", "nosuchpane")).await;
    assert_coded_error(&reply, StatusCode::NOT_FOUND, "pane_unknown", &[]);

    let reply = send(&components.router, "GET", &root_uri("nosuchrt", "x")).await;
    assert_coded_error(&reply, StatusCode::NOT_FOUND, "runtime_unknown", &[]);

    app::shutdown_components(
        components.stops,
        components.drivers,
        components.projector,
        std::time::Duration::from_secs(5),
    )
    .await;
}

#[test]
fn path_mapping_follows_runtime_endpoint() {
    assert_eq!(
        PathMapping::from_endpoint(&HerdrEndpoint::Wsl {
            distro: "Ubuntu-24.04".to_string(),
            socket: "/tmp/herdr.sock".to_string(),
        }),
        PathMapping::Wsl {
            distro: "Ubuntu-24.04".to_string()
        }
    );
    for endpoint in [
        HerdrEndpoint::Default,
        HerdrEndpoint::Socket(PathBuf::from("pipe")),
        HerdrEndpoint::Command(vec!["x".to_string()]),
    ] {
        assert_eq!(
            PathMapping::from_endpoint(&endpoint),
            PathMapping::Native,
            "{endpoint:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// 允許清單（design D2；給 file-review task 3.2 的所有檔案端點重用）
// ---------------------------------------------------------------------------

/// Scenario: pane 關掉後根目錄不可用
#[tokio::test]
async fn allowlist_scenario_root_unavailable_after_pane_closes() {
    let tmp = TempDir::new("closed");
    let (root, src) = make_repo(&tmp);
    let (tx, app) = build(
        projected(&[(WIN, &[("w1:p1", Some(path_str(&src)), false)])]),
        &[(WIN, PathMapping::Native)],
    );
    let router = http::router(app.clone());
    let reply = send(&router, "GET", &root_uri(WIN, "w1:p1")).await;
    let root_id = reply.json["root_id"]
        .as_str()
        .expect("應拿到 root_id")
        .to_string();

    assert_eq!(
        authorize_root(&app, WIN, &root_id).await,
        Ok(Root {
            path: root.clone(),
            is_git: true
        }),
        "pane 還在時根目錄應可用"
    );

    tx.send(projected(&[(WIN, &[])])).expect("receiver 還在");

    assert_eq!(
        authorize_root(&app, WIN, &root_id).await,
        Err(FileApiError::RootUnavailable),
        "最新投影沒有 w1:p1 後，同一個 root_id 應不可用"
    );
}

#[tokio::test]
async fn allowlist_includes_exited_panes() {
    let tmp = TempDir::new("exited");
    let (root, src) = make_repo(&tmp);
    let (_tx, app) = build(
        projected(&[(WIN, &[("w1:p1", Some(path_str(&src)), true)])]),
        &[(WIN, PathMapping::Native)],
    );
    let id = encode_root_id(&root).expect("可編碼");
    assert_eq!(
        authorize_root(&app, WIN, &id).await.map(|r| r.path),
        Ok(root)
    );
}

#[tokio::test]
async fn allowlist_rejects_self_encoded_root_not_in_list() {
    let tmp = TempDir::new("outside");
    let (_root, src) = make_repo(&tmp);
    let outside = TempDir::new("outside-other");
    let (_tx, app) = build(
        projected(&[(WIN, &[("w1:p1", Some(path_str(&src)), false)])]),
        &[(WIN, PathMapping::Native)],
    );

    for path in [
        Path::new(r"C:\Windows"),
        outside.path(),
        // 允許清單內根目錄的上層與子目錄：字面前綴相符也不行，必須正好是推算出的根目錄。
        tmp.path(),
        src.as_path(),
    ] {
        let id = encode_root_id(path).expect("可編碼");
        assert_eq!(
            authorize_root(&app, WIN, &id).await,
            Err(FileApiError::RootUnavailable),
            "{}",
            path.display()
        );
    }
}

#[tokio::test]
async fn allowlist_is_scoped_per_runtime() {
    let tmp = TempDir::new("scoped");
    let (root, src) = make_repo(&tmp);
    let (_tx, app) = build(
        projected(&[
            (WIN, &[("w1:p1", Some(path_str(&src)), false)]),
            ("other", &[]),
        ]),
        &[(WIN, PathMapping::Native), ("other", PathMapping::Native)],
    );
    let id = encode_root_id(&root).expect("可編碼");
    assert!(authorize_root(&app, WIN, &id).await.is_ok());
    assert_eq!(
        authorize_root(&app, "other", &id).await,
        Err(FileApiError::RootUnavailable)
    );
}

#[tokio::test]
async fn allowlist_bad_root_id_and_unknown_runtime() {
    let (_tx, app) = build(projected(&[(WIN, &[])]), &[(WIN, PathMapping::Native)]);
    assert_eq!(
        authorize_root(&app, WIN, "not-hex").await,
        Err(FileApiError::BadRequest)
    );
    let id = encode_root_id(Path::new(r"D:\repo")).expect("可編碼");
    assert_eq!(
        authorize_root(&app, "nosuchrt", &id).await,
        Err(FileApiError::RuntimeUnknown)
    );
}

#[test]
fn file_api_error_codes_and_statuses_match_spec() {
    let cases = [
        (FileApiError::BadRequest, 400, "bad_request"),
        (FileApiError::RuntimeUnknown, 404, "runtime_unknown"),
        (FileApiError::PaneUnknown, 404, "pane_unknown"),
        (FileApiError::NoRoot, 404, "no_root"),
        (FileApiError::RootUnavailable, 404, "root_unavailable"),
        (FileApiError::MethodNotAllowed, 405, "method_not_allowed"),
        (FileApiError::Internal, 500, "io_error"),
    ];
    for (err, status, code) in cases {
        assert_eq!(err.status().as_u16(), status, "{err:?}");
        assert_eq!(err.code(), code, "{err:?}");
    }
}

// ===========================================================================
// file-review task 3.2：列目錄、中繼資料、Markdown 渲染、原始內容四組端點
// （spec「檔案端點的共同規則」「列目錄端點」「中繼資料端點」「Markdown 渲染端點」「原始內容端點」
// 「在 VS Code 開啟」「檔案 icon」；design D2、D3、D5、D9、D10）
// ===========================================================================

/// 一個已在允許清單內的根目錄：`tmp/repo`（含 `.git` 資料夾），runtime `win` 的 pane `w1:p1`
/// 的 `cwd` 就是它。`tmp` 本身（根目錄的上層）拿來放「根目錄外」的檔案。
struct Fixture {
    tmp: TempDir,
    root: PathBuf,
    root_id: String,
    router: axum::Router,
    _tx: watch::Sender<Arc<ProjectedState>>,
}

impl Fixture {
    fn new(tag: &str) -> Self {
        Self::with_settings(tag, FileSettings::embedded())
    }

    fn with_raw_limit(tag: &str, limit: u64) -> Self {
        let mut settings = FileSettings::embedded();
        settings.raw_limit = limit;
        Self::with_settings(tag, settings)
    }

    fn with_settings(tag: &str, settings: FileSettings) -> Self {
        let tmp = TempDir::new(tag);
        let (root, _src) = make_repo(&tmp);
        let (tx, mut app) = build(
            projected(&[(WIN, &[("w1:p1", Some(path_str(&root)), false)])]),
            &[(WIN, PathMapping::Native)],
        );
        app.files = Arc::new(settings);
        let root_id = encode_root_id(&root).expect("可編碼");
        Self {
            tmp,
            root,
            root_id,
            router: http::router(app),
            _tx: tx,
        }
    }

    /// `rel` 原樣（仍為 percent-encoded）接在 `/api/files/win/<root_id>/<op>` 之後；空字串＝根目錄本身。
    fn uri(&self, op: &str, rel: &str) -> String {
        files_uri(WIN, &self.root_id, op, rel)
    }

    fn write(&self, rel: &str, content: &[u8]) {
        let path = self.root.join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("建立上層資料夾");
        }
        fs::write(&path, content).unwrap_or_else(|e| panic!("寫入 {rel} 失敗：{e}"));
    }

    async fn get(&self, op: &str, rel: &str) -> Reply {
        send(&self.router, "GET", &self.uri(op, rel)).await
    }
}

fn files_uri(runtime: &str, root_id: &str, op: &str, rel: &str) -> String {
    if rel.is_empty() {
        format!("/api/files/{runtime}/{root_id}/{op}")
    } else {
        format!("/api/files/{runtime}/{root_id}/{op}/{rel}")
    }
}

fn header<'a>(reply: &'a Reply, name: &str) -> Option<&'a str> {
    reply.headers.get(name).and_then(|v| v.to_str().ok())
}

const OPS: [&str; 4] = ["list", "meta", "render", "raw"];

fn embedded_icons() -> cockpit_files::IconTheme {
    FileSettings::embedded().icons
}

// ---------------------------------------------------------------------------
// 共同規則
// ---------------------------------------------------------------------------

/// Scenario: 用 .. 跳出根目錄——五個輸入經 HTTP 原始 URI 送出，四個端點都回 400 `bad_request`。
/// 根目錄外真的放一個 `secret.txt`（`docs/../../secret.txt` 若被字面解析就會讀到它）。
#[tokio::test]
async fn common_scenario_dotdot_and_malformed_segments_are_bad_request() {
    let fx = Fixture::new("escape");
    fs::write(fx.tmp.path().join("secret.txt"), "TOP-SECRET").expect("寫根外檔案");
    fx.write("docs/a.md", b"# a");

    for rel in [
        "docs/../../secret.txt",
        "..%2F..%2Fsecret.txt",
        "C:%5Cx",
        "CON.txt",
        "a.",
    ] {
        for op in OPS {
            let reply = fx.get(op, rel).await;
            assert_coded_error(
                &reply,
                StatusCode::BAD_REQUEST,
                "bad_request",
                &[rel, "secret", "TOP-SECRET", "CON", "C:"],
            );
        }
    }
}

/// 片段規則在碰檔案系統之前就生效：root_id 不在允許清單時，壞的相對路徑仍是 400，不是 404
/// `root_unavailable`——代表判斷順序是先驗路徑、再做任何檔案系統存取（允許清單推算會碰檔案系統）。
#[tokio::test]
async fn common_bad_relative_path_is_rejected_before_any_filesystem_access() {
    let (_tx, app) = build(projected(&[(WIN, &[])]), &[(WIN, PathMapping::Native)]);
    let router = http::router(app);
    let id = encode_root_id(Path::new(r"C:\Windows")).expect("可編碼");
    let reply = send(&router, "GET", &files_uri(WIN, &id, "raw", "..%2Fx")).await;
    assert_coded_error(&reply, StatusCode::BAD_REQUEST, "bad_request", &["..%2Fx"]);
}

/// 其他 bad_request 形狀：非 UTF-8、空片段、`list/` 結尾空片段、`%` 後不是十六進位、NUL、ADS 等。
#[tokio::test]
async fn common_other_malformed_relative_paths_are_bad_request() {
    let fx = Fixture::new("malformed");
    fx.write("docs/a.md", b"# a");
    for (op, rel) in [
        ("raw", "%FF"),
        ("raw", "docs//a.md"),
        ("list", "docs/"),
        ("raw", "docs/%zz"),
        ("raw", "a%00b"),
        ("raw", "docs/a.md:stream"),
        ("meta", "docs/./a.md"),
        ("raw", "AUX"),
        ("raw", "nul.md"),
        ("raw", "a%2Fb"),
        ("raw", "a%5Cb"),
        ("raw", "a%20"),
    ] {
        let reply = fx.get(op, rel).await;
        assert_coded_error(&reply, StatusCode::BAD_REQUEST, "bad_request", &[rel]);
    }
}

#[tokio::test]
async fn common_runtime_and_root_id_segments() {
    let fx = Fixture::new("runtime-seg");
    let reply = send(
        &fx.router,
        "GET",
        &files_uri("%FF", &fx.root_id, "list", ""),
    )
    .await;
    assert_coded_error(&reply, StatusCode::BAD_REQUEST, "bad_request", &["%FF"]);

    let reply = send(
        &fx.router,
        "GET",
        &files_uri("nosuchrt", &fx.root_id, "list", ""),
    )
    .await;
    assert_coded_error(
        &reply,
        StatusCode::NOT_FOUND,
        "runtime_unknown",
        &["nosuchrt"],
    );

    // percent-encoded 的 runtime id 解碼後比對（`w%69n` ＝ `win`）。
    let reply = send(
        &fx.router,
        "GET",
        &files_uri("w%69n", &fx.root_id, "list", ""),
    )
    .await;
    assert_eq!(reply.status, StatusCode::OK, "本體：{}", reply.raw);

    // root_id 不是合法十六進位 → 400。
    let reply = send(&fx.router, "GET", &files_uri(WIN, "zz", "list", "")).await;
    assert_coded_error(&reply, StatusCode::BAD_REQUEST, "bad_request", &["zz"]);
}

/// Scenario: 符號連結指向根目錄外（junction 版：不需要建立符號連結的權限）。
#[cfg(windows)]
#[tokio::test]
async fn common_scenario_link_pointing_outside_root_is_403() {
    let fx = Fixture::new("link-out");
    let outside = fx.tmp.path().join("outside");
    fs::create_dir_all(&outside).expect("建立根外資料夾");
    fs::write(outside.join("secret.txt"), "TOP-SECRET").expect("寫根外檔案");
    let link = fx.root.join("linkdir");
    let out = std::process::Command::new("cmd")
        .args(["/c", "mklink", "/J"])
        .arg(&link)
        .arg(&outside)
        .output()
        .expect("執行 cmd mklink /J");
    assert!(
        out.status.success(),
        "mklink /J 失敗：{}",
        String::from_utf8_lossy(&out.stderr)
    );

    for (op, rel) in [
        ("raw", "linkdir/secret.txt"),
        ("meta", "linkdir/secret.txt"),
        ("render", "linkdir/secret.txt"),
        ("list", "linkdir"),
    ] {
        let reply = fx.get(op, rel).await;
        assert_coded_error(
            &reply,
            StatusCode::FORBIDDEN,
            "path_outside_root",
            &["TOP-SECRET", "secret", "linkdir"],
        );
    }

    // spec 逐字的形狀是檔案符號連結 `link.txt`；建立它需要權限（開發人員模式或系統管理員），
    // 建不出來就只驗上面的 junction 版。
    let file_link = fx.root.join("link.txt");
    match std::os::windows::fs::symlink_file(outside.join("secret.txt"), &file_link) {
        Ok(()) => {
            let reply = fx.get("raw", "link.txt").await;
            assert_coded_error(
                &reply,
                StatusCode::FORBIDDEN,
                "path_outside_root",
                &["TOP-SECRET"],
            );
        }
        Err(e) => eprintln!("略過檔案符號連結版（無建立權限）：{e}"),
    }
}

/// Scenario: 根目錄不在允許清單
#[tokio::test]
async fn common_scenario_root_not_in_allowlist_is_root_unavailable() {
    let fx = Fixture::new("not-listed");
    let id = encode_root_id(Path::new(r"C:\Windows")).expect("可編碼");
    let reply = send(&fx.router, "GET", &files_uri(WIN, &id, "list", "")).await;
    assert_coded_error(&reply, StatusCode::NOT_FOUND, "root_unavailable", &[]);
    let reply = send(&fx.router, "GET", &files_uri(WIN, &id, "raw", "win.ini")).await;
    assert_coded_error(
        &reply,
        StatusCode::NOT_FOUND,
        "root_unavailable",
        &["win.ini"],
    );
}

/// Scenario: DNS rebinding 被拒——每個檔案端點；403 本體 `code` 為 `forbidden_source`，不含
/// `Host`／`Origin` 原文（design D10）。
#[tokio::test]
async fn common_scenario_dns_rebinding_is_forbidden_source_without_echo() {
    let fx = Fixture::new("rebinding");
    fx.write("docs/a.md", b"# a");
    let uris = [
        fx.uri("list", ""),
        fx.uri("list", "docs"),
        fx.uri("meta", "docs/a.md"),
        fx.uri("render", "docs/a.md"),
        fx.uri("raw", "docs/a.md"),
    ];
    for uri in &uris {
        for headers in [
            vec![("host", "evil.example:7770")],
            vec![("host", "evil.example:0")],
            vec![("host", "127.0.0.1:0"), ("origin", "http://evil.example")],
            vec![("host", "127.0.0.1:0"), ("host", "evil.example:0")],
            vec![],
        ] {
            let reply = send_with_headers(&fx.router, "GET", uri, &headers).await;
            assert_coded_error(
                &reply,
                StatusCode::FORBIDDEN,
                "forbidden_source",
                &["evil.example", "7770", "# a"],
            );
        }
    }
}

/// design D10：`source_check` 的 403 本體改成帶 `code`、不含標頭原文，套用它的既有端點（輸出、
/// 寫入）也一樣；這些端點的 405 本體同樣帶 `method_not_allowed`。
#[tokio::test]
async fn source_check_403_and_405_are_coded_on_existing_endpoints_too() {
    let (_tx, app) = build(projected(&[(WIN, &[])]), &[(WIN, PathMapping::Native)]);
    let router = http::router(app);

    for (method, uri) in [
        ("GET", "/api/runtimes/win/panes/w1:p1/output"),
        ("POST", "/api/projects/p/tasks/t1/advance"),
        ("PUT", "/api/projects/p/workstreams/be/override"),
        ("DELETE", "/api/projects/p/workstreams/be/override"),
    ] {
        for headers in [
            vec![("host", "evil.example:0")],
            vec![("host", "127.0.0.1:0"), ("origin", "https://evil.example")],
        ] {
            let reply = send_with_headers(&router, method, uri, &headers).await;
            assert_coded_error(
                &reply,
                StatusCode::FORBIDDEN,
                "forbidden_source",
                &["evil.example"],
            );
        }
    }

    for (method, uri) in [
        ("POST", "/api/runtimes/win/panes/w1:p1/output"),
        ("GET", "/api/projects/p/tasks/t1/advance"),
        ("GET", "/api/projects/p/workstreams/be/override"),
    ] {
        let reply = send(&router, method, uri).await;
        assert_coded_error(
            &reply,
            StatusCode::METHOD_NOT_ALLOWED,
            "method_not_allowed",
            &[],
        );
    }
}

/// Scenario: 不接受其他 method——每個檔案端點；`HEAD` 不論 `Host`／`Origin` 都是 405、沒有本體。
#[tokio::test]
async fn common_scenario_other_methods_are_405() {
    let fx = Fixture::new("methods");
    fx.write("docs/a.md", b"# a");
    let uris = [
        fx.uri("list", ""),
        fx.uri("list", "docs"),
        fx.uri("meta", "docs/a.md"),
        fx.uri("render", "docs/a.md"),
        fx.uri("raw", "docs/a.md"),
    ];
    for uri in &uris {
        for method in ["POST", "PUT", "DELETE", "PATCH"] {
            let reply = send(&fx.router, method, uri).await;
            assert_coded_error(
                &reply,
                StatusCode::METHOD_NOT_ALLOWED,
                "method_not_allowed",
                &["# a"],
            );
        }
        for headers in [
            vec![("host", "127.0.0.1:0")],
            vec![("host", "evil.example:0")],
            vec![("host", "127.0.0.1:0"), ("origin", "http://evil.example")],
        ] {
            let reply = send_with_headers(&fx.router, "HEAD", uri, &headers).await;
            assert_eq!(
                reply.status,
                StatusCode::METHOD_NOT_ALLOWED,
                "{uri} {headers:?}"
            );
            assert!(reply.raw.is_empty(), "HEAD 不應有本體：{:?}", reply.raw);
            assert_security_headers(&reply);
        }
    }
}

/// Scenario: 被隱藏的檔案仍可直接讀取
#[tokio::test]
async fn common_scenario_hidden_file_is_still_directly_readable() {
    let fx = Fixture::new("hidden");
    fx.write(".gitignore", b".env\n");
    fx.write(".env", b"TOKEN=1");

    let list = fx.get("list", "").await;
    assert_eq!(list.status, StatusCode::OK, "本體：{}", list.raw);
    assert_security_headers(&list);
    let names: Vec<&str> = list.json["entries"]
        .as_array()
        .expect("entries 應為陣列")
        .iter()
        .map(|e| e["name"].as_str().expect("name 應為字串"))
        .collect();
    assert!(!names.contains(&".env"), "列表不應有 .env：{names:?}");
    assert!(!names.contains(&".git"), "列表不應有 .git：{names:?}");
    assert!(names.contains(&".gitignore"), "{names:?}");

    let meta = fx.get("meta", ".env").await;
    assert_eq!(meta.status, StatusCode::OK, "本體：{}", meta.raw);
    assert_security_headers(&meta);
    let raw = fx.get("raw", ".env").await;
    assert_eq!(raw.status, StatusCode::OK);
    assert_eq!(raw.raw, "TOKEN=1");
}

#[tokio::test]
async fn common_not_found_and_wrong_kind_per_endpoint() {
    let fx = Fixture::new("kinds");
    fx.write("docs/a.md", b"# a");
    fx.write("note.txt", b"hi");

    for op in OPS {
        let reply = fx.get(op, "missing-file.md").await;
        assert_coded_error(
            &reply,
            StatusCode::NOT_FOUND,
            "not_found",
            &["missing-file"],
        );
    }

    // 對檔案要列目錄、對資料夾要中繼資料／渲染／內容 → 400 wrong_kind。
    for (op, rel) in [
        ("list", "note.txt"),
        ("meta", "docs"),
        ("render", "docs"),
        ("raw", "docs"),
    ] {
        let reply = fx.get(op, rel).await;
        assert_coded_error(&reply, StatusCode::BAD_REQUEST, "wrong_kind", &[rel]);
    }
}

#[test]
fn files_error_mapping_matches_spec_and_hides_io_message() {
    use cockpit_files::FilesError;
    let cases = [
        (FilesError::BadRequest, 400, "bad_request"),
        (FilesError::PathOutsideRoot, 403, "path_outside_root"),
        (FilesError::NotFound, 404, "not_found"),
        (FilesError::WrongKind, 400, "wrong_kind"),
        (FilesError::TooLarge, 413, "too_large"),
        (FilesError::NotMarkdown, 415, "not_markdown"),
        (
            FilesError::Io(std::io::Error::other(r"C:\secret\path.txt 拒絕存取")),
            500,
            "io_error",
        ),
    ];
    let rt = tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("建立 runtime");
    for (err, status, code) in cases {
        let response = files_error_response(err);
        let headers = response.headers().clone();
        let actual_status = response.status();
        let bytes = rt
            .block_on(response.into_body().collect())
            .expect("收本體")
            .to_bytes();
        let reply = Reply {
            status: actual_status,
            headers,
            raw: String::from_utf8_lossy(&bytes).into_owned(),
            json: serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        };
        assert_coded_error(
            &reply,
            StatusCode::from_u16(status).expect("合法狀態碼"),
            code,
            &["secret", "path.txt", "拒絕存取"],
        );
    }
}

// ---------------------------------------------------------------------------
// 列目錄端點＋檔案 icon
// ---------------------------------------------------------------------------

/// spec「列目錄端點」的回應形狀、spec「檔案 icon」的「常見檔案」「資料夾展開」。
#[tokio::test]
async fn list_scenario_entries_carry_icons() {
    let fx = Fixture::new("list-icons");
    fs::create_dir_all(fx.root.join("src")).expect("建立 src");
    for name in [
        "README.md",
        "Cargo.toml",
        "report.pdf",
        "index.html",
        "unknown.zzz",
        "x.md",
    ] {
        fx.write(name, b"x");
    }
    let icons = embedded_icons();

    let reply = fx.get("list", "").await;
    assert_eq!(reply.status, StatusCode::OK, "本體：{}", reply.raw);
    assert_security_headers(&reply);
    assert_eq!(header(&reply, "content-type"), Some("application/json"));
    assert_eq!(reply.json["omitted"], 0);
    assert_eq!(reply.json["skipped"], 0);
    let entries = reply.json["entries"].as_array().expect("entries 應為陣列");
    let find = |name: &str| {
        entries
            .iter()
            .find(|e| e["name"] == name)
            .unwrap_or_else(|| panic!("列表應含 {name}：{entries:?}"))
    };

    let src = find("src");
    assert_eq!(src["kind"], "dir");
    assert_eq!(src["icon"], "folder-src.svg");
    assert_eq!(src["icon_open"], "folder-src-open.svg");
    assert_ne!(src["icon"], src["icon_open"]);

    let readme = find("README.md");
    assert_eq!(readme["kind"], "file");
    assert_eq!(readme["icon"], "readme.svg", "完整檔名優先於 .md");
    assert_eq!(readme["icon_open"], Value::Null, "檔案的 icon_open 為 null");
    assert_eq!(find("x.md")["icon"], "markdown.svg");
    assert_eq!(find("report.pdf")["icon"], icons.file_icon("report.pdf"));
    assert_eq!(find("index.html")["icon"], icons.file_icon("index.html"));
    assert_eq!(find("Cargo.toml")["icon"], icons.file_icon("Cargo.toml"));
    assert_eq!(find("unknown.zzz")["icon"], "file.svg", "預設檔案 icon");

    // 每筆恰好四個欄位；`icon_open` 一定出現（檔案為 null，不是省略）。
    for entry in entries {
        let object = entry.as_object().expect("entry 應為物件");
        assert_eq!(object.len(), 4, "{entry}");
        assert!(object.contains_key("icon_open"), "{entry}");
    }

    // `.git` 不列出；資料夾在前。
    assert_eq!(entries[0]["name"], "src", "{entries:?}");
    assert!(entries.iter().all(|e| e["name"] != ".git"));

    // 子目錄的列目錄。
    fx.write("src/main.rs", b"fn main() {}");
    let reply = fx.get("list", "src").await;
    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(reply.json["entries"][0]["name"], "main.rs");
    assert_eq!(reply.json["entries"][0]["icon"], icons.file_icon("main.rs"));
}

/// 啟動時交給 `IconTheme::from_json` 的內嵌對照表必須能解析（design D9；解析失敗＝程式錯誤）。
#[test]
fn embedded_icon_theme_parses_and_raw_limit_is_50_mib() {
    let icons = embedded_icons();
    assert_eq!(icons.file_icon("README.md"), "readme.svg");
    assert_eq!(
        icons.folder_icon("src"),
        ("folder-src.svg", "folder-src-open.svg")
    );
    assert_eq!(FileSettings::embedded().raw_limit, RAW_SIZE_LIMIT);
    assert_eq!(RAW_SIZE_LIMIT, 50 * 1024 * 1024);
}

// ---------------------------------------------------------------------------
// 中繼資料端點＋在 VS Code 開啟
// ---------------------------------------------------------------------------

/// Scenario: Windows 檔案（經 HTTP：根目錄是暫存目錄，`vscode_uri` 依其主機路徑產生）。
#[tokio::test]
async fn meta_scenario_windows_file_has_vscode_uri_and_icon() {
    let fx = Fixture::new("meta-win");
    fx.write("docs/a b.md", b"# hello");

    let reply = fx.get("meta", "docs/a%20b.md").await;
    assert_eq!(reply.status, StatusCode::OK, "本體：{}", reply.raw);
    assert_security_headers(&reply);
    assert_eq!(header(&reply, "content-type"), Some("application/json"));
    let body = &reply.json;
    assert_eq!(body["size"], 7);
    assert!(body["modified_ms"].as_i64().is_some_and(|ms| ms > 0));
    assert_eq!(body["viewer"], "markdown");
    assert_eq!(body["icon"], "markdown.svg");
    let root = path_str(&fx.root).replace('\\', "/");
    let (drive, rest) = root.split_once('/').expect("磁碟代號之後有 /");
    let rest: Vec<String> = rest.split('/').map(percent_encode_for_test).collect();
    let expected = format!("vscode://file/{drive}/{}/docs/a%20b.md", rest.join("/"));
    assert_eq!(body["vscode_uri"], expected);
    assert_eq!(body.as_object().expect("物件").len(), 5, "{body}");
}

fn percent_encode_for_test(seg: &str) -> String {
    seg.bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
                char::from(b).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}

/// spec「中繼資料端點」「分類」經 HTTP。
#[tokio::test]
async fn meta_viewer_classification_over_http() {
    let fx = Fixture::new("meta-classify");
    fx.write("a.MD", b"# a");
    fx.write("b.pdf", b"%PDF-1.4");
    fx.write("c.htm", b"<p>c</p>");
    fx.write("d.toml", b"a = 1");
    fx.write("e.png", b"\x89PNG\0\0");
    for (rel, viewer) in [
        ("a.MD", "markdown"),
        ("b.pdf", "pdf"),
        ("c.htm", "html"),
        ("d.toml", "text"),
        ("e.png", "unsupported"),
    ] {
        let reply = fx.get("meta", rel).await;
        assert_eq!(reply.status, StatusCode::OK, "{rel}：{}", reply.raw);
        assert_eq!(reply.json["viewer"], viewer, "{rel}");
    }
}

/// Scenario: Windows 檔案（純函式，spec 逐字）。
#[test]
fn vscode_uri_scenario_windows_file() {
    assert_eq!(
        vscode_uri(
            &PathMapping::Native,
            Path::new(r"D:\repo"),
            &["docs".to_string(), "a b.md".to_string()]
        ),
        Some("vscode://file/D:/repo/docs/a%20b.md".to_string())
    );
}

/// Scenario: WSL 檔案（純函式，spec 逐字；結尾 `:1` 不可省略）。
#[test]
fn vscode_uri_scenario_wsl_file() {
    let wsl = PathMapping::Wsl {
        distro: "Ubuntu-24.04".to_string(),
    };
    assert_eq!(
        vscode_uri(
            &wsl,
            Path::new(r"\\wsl.localhost\Ubuntu-24.04\home\u\repo"),
            &["a.md".to_string()]
        ),
        Some("vscode://vscode-remote/wsl+Ubuntu-24.04/home/u/repo/a.md:1".to_string())
    );
}

#[test]
fn vscode_uri_encodes_every_byte_outside_unreserved() {
    // 非 ASCII（UTF-8 位元組逐一編碼）與各種保留字元都要編碼，只有 `A-Za-z0-9-._~` 原樣。
    assert_eq!(
        vscode_uri(
            &PathMapping::Native,
            Path::new(r"D:\專案 x"),
            &["a#b%c?d+e&f'g;h=i@j!k~l_m.n-o.md".to_string()]
        ),
        Some(
            "vscode://file/D:/%E5%B0%88%E6%A1%88%20x/a%23b%25c%3Fd%2Be%26f%27g%3Bh%3Di%40j%21k~l_m.n-o.md"
                .to_string()
        )
    );
    let wsl = PathMapping::Wsl {
        distro: "My Distro".to_string(),
    };
    assert_eq!(
        vscode_uri(
            &wsl,
            Path::new(r"\\wsl.localhost\My Distro\home\u\專"),
            &["a b.md".to_string()]
        ),
        Some("vscode://vscode-remote/wsl+My%20Distro/home/u/%E5%B0%88/a%20b.md:1".to_string())
    );
}

#[test]
fn vscode_uri_edge_shapes() {
    let file = ["a.md".to_string()];
    // 磁碟根。
    assert_eq!(
        vscode_uri(&PathMapping::Native, Path::new(r"D:\"), &file),
        Some("vscode://file/D:/a.md".to_string())
    );
    // `/` 分隔的 Windows 路徑同樣接受。
    assert_eq!(
        vscode_uri(&PathMapping::Native, Path::new("D:/repo"), &file),
        Some("vscode://file/D:/repo/a.md".to_string())
    );
    // distro 根。
    let wsl = PathMapping::Wsl {
        distro: "Ubuntu".to_string(),
    };
    assert_eq!(
        vscode_uri(&wsl, Path::new(r"\\wsl.localhost\Ubuntu\"), &file),
        Some("vscode://vscode-remote/wsl+Ubuntu/a.md:1".to_string())
    );
    // 無法產生 → None：非 WSL runtime 的 UNC 根目錄、不是磁碟代號開頭、WSL 根目錄不是該 distro
    // 的 `\\wsl.localhost` 路徑、沒有相對路徑（根目錄本身不是檔案）。
    let none_cases: [(PathMapping, &str, &[String]); 9] = [
        (PathMapping::Native, r"\\server\share\repo", &file),
        (PathMapping::Native, r"\\wsl.localhost\Ubuntu\home", &file),
        (PathMapping::Native, r"\\?\D:\repo", &file),
        (PathMapping::Native, "repo", &file),
        (PathMapping::Native, "/home/u", &file),
        (wsl.clone(), r"\\wsl.localhost\Other\home", &file),
        (wsl.clone(), r"D:\repo", &file),
        (wsl.clone(), r"\\wsl$\Ubuntu\home", &file),
        (PathMapping::Native, r"D:\repo", &[]),
    ];
    for (mapping, root, rel) in none_cases {
        assert_eq!(
            vscode_uri(&mapping, Path::new(root), rel),
            None,
            "{mapping:?} {root}"
        );
    }
}

// ---------------------------------------------------------------------------
// Markdown 渲染端點
// ---------------------------------------------------------------------------

#[tokio::test]
async fn render_returns_sandboxed_html_fragment() {
    let fx = Fixture::new("render");
    fx.write(
        "doc.md",
        "# 標題\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\n- [x] 完成項\n\n~~刪除~~\n\n<script>window.pwned=1</script>\n"
            .as_bytes(),
    );
    let reply = fx.get("render", "doc.md").await;
    assert_eq!(reply.status, StatusCode::OK, "本體：{}", reply.raw);
    assert_security_headers(&reply);
    assert_eq!(
        header(&reply, "content-type"),
        Some("text/html; charset=utf-8")
    );
    assert_eq!(header(&reply, "content-security-policy"), Some("sandbox"));
    assert!(reply.raw.contains("<table"), "{}", reply.raw);
    assert!(reply.raw.contains("<del>"), "{}", reply.raw);
    assert!(reply.raw.contains("id=\"md-"), "{}", reply.raw);
    assert!(!reply.raw.contains("<script"), "{}", reply.raw);
    assert!(!reply.raw.contains("<html"), "{}", reply.raw);
}

#[tokio::test]
async fn render_non_markdown_is_415_not_markdown() {
    let fx = Fixture::new("render-415");
    fx.write("note.txt", b"# not md");
    fx.write("page.html", b"<p>x</p>");
    for rel in ["note.txt", "page.html"] {
        let reply = fx.get("render", rel).await;
        assert_coded_error(
            &reply,
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "not_markdown",
            &[rel, "not md"],
        );
    }
}

#[tokio::test]
async fn render_over_2_mib_is_413() {
    let fx = Fixture::new("render-413");
    fx.write("big.md", &vec![b'a'; 2 * 1024 * 1024 + 1]);
    let reply = fx.get("render", "big.md").await;
    assert_coded_error(
        &reply,
        StatusCode::PAYLOAD_TOO_LARGE,
        "too_large",
        &["big.md"],
    );
}

// ---------------------------------------------------------------------------
// 原始內容端點
// ---------------------------------------------------------------------------

/// Scenario: HTML 帶 sandbox
#[tokio::test]
async fn raw_scenario_html_has_sandbox() {
    let fx = Fixture::new("raw-html");
    let html = "<script>window.x=1</script><p>頁</p>";
    fx.write("page.html", html.as_bytes());
    let reply = fx.get("raw", "page.html").await;
    assert_eq!(reply.status, StatusCode::OK, "本體：{}", reply.raw);
    assert_eq!(header(&reply, "content-type"), Some("text/html"));
    assert_eq!(header(&reply, "content-security-policy"), Some("sandbox"));
    assert_eq!(header(&reply, "x-content-type-options"), Some("nosniff"));
    assert_security_headers(&reply);
    assert_eq!(reply.raw, html);
}

/// spec「原始內容端點」content-type 對照表（副檔名不分大小寫；其他 viewer 為 text 的檔案為
/// `text/plain; charset=utf-8`，其餘 `application/octet-stream`）；每個回應都帶 CSP sandbox。
#[tokio::test]
async fn raw_content_type_table() {
    let fx = Fixture::new("raw-types");
    let cases: &[(&str, &[u8], &str)] = &[
        ("a.html", b"<p>", "text/html"),
        ("a.HTM", b"<p>", "text/html"),
        ("a.pdf", b"%PDF", "application/pdf"),
        ("a.svg", b"<svg/>", "image/svg+xml"),
        ("a.PNG", b"\x89PNG\0", "image/png"),
        ("a.jpg", b"\xff\xd8\0", "image/jpeg"),
        ("a.JPEG", b"\xff\xd8\0", "image/jpeg"),
        ("a.gif", b"GIF89a\0", "image/gif"),
        ("a.webp", b"RIFF\0", "image/webp"),
        ("a.css", b"p{}", "text/css"),
        ("a.md", b"# a", "text/plain; charset=utf-8"),
        ("a.Markdown", b"# a", "text/plain; charset=utf-8"),
        ("a.txt", b"\0binary but .txt", "text/plain; charset=utf-8"),
        ("a.toml", b"a = 1", "text/plain; charset=utf-8"),
        ("Makefile", b"all:", "text/plain; charset=utf-8"),
        ("a.rs", "// 中文".as_bytes(), "text/plain; charset=utf-8"),
        ("a.bin", b"\0\x01\x02", "application/octet-stream"),
        ("a.dat", b"\xff\xfe\xfd", "application/octet-stream"),
        ("a.js", b"alert(1)", "text/plain; charset=utf-8"),
    ];
    for (name, content, content_type) in cases {
        fx.write(name, content);
        let reply = fx.get("raw", name).await;
        assert_eq!(reply.status, StatusCode::OK, "{name}：{}", reply.raw);
        assert_eq!(
            header(&reply, "content-type"),
            Some(*content_type),
            "{name}"
        );
        assert_eq!(
            header(&reply, "content-security-policy"),
            Some("sandbox"),
            "{name}"
        );
        assert_security_headers(&reply);
        assert_eq!(reply.raw, String::from_utf8_lossy(content), "{name}");
    }
}

/// Scenario: 太大——上限由測試注入（正式值 50 MiB，見
/// `embedded_icon_theme_parses_and_raw_limit_is_50_mib`），不在 repo 放大檔。
#[tokio::test]
async fn raw_scenario_too_large_with_injected_limit() {
    let fx = Fixture::with_raw_limit("raw-413", 16);
    fx.write("big.pdf", &[b'a'; 17]);
    fx.write("exact.pdf", &[b'a'; 16]);

    let reply = fx.get("raw", "big.pdf").await;
    assert_coded_error(
        &reply,
        StatusCode::PAYLOAD_TOO_LARGE,
        "too_large",
        &["big.pdf", "aaaa"],
    );

    let reply = fx.get("raw", "exact.pdf").await;
    assert_eq!(reply.status, StatusCode::OK, "恰好等於上限應可讀");
    assert_eq!(reply.raw.len(), 16);
}

/// Scenario: 太大（spec 逐字 60 MiB，正式上限）：`File::set_len` 造出長度 60 MiB 的檔案（內容
/// 未寫入，建立很快），只在暫存目錄、不進版控。
#[tokio::test]
async fn raw_scenario_too_large_60_mib_with_production_limit() {
    let fx = Fixture::new("raw-60mib");
    let file = fs::File::create(fx.root.join("big.pdf")).expect("建立 big.pdf");
    file.set_len(60 * 1024 * 1024).expect("set_len");
    drop(file);
    let reply = fx.get("raw", "big.pdf").await;
    assert_coded_error(&reply, StatusCode::PAYLOAD_TOO_LARGE, "too_large", &[]);
}

/// `.../list/`（空的 `{*path}`）與 `.../raw`（少了相對路徑）不命中任何檔案端點路由（matchit 的
/// catch-all 不收空值），落到 axum 預設 404——重點是 `.../list/` 不會被當成根目錄列出。
#[tokio::test]
async fn trailing_slash_or_missing_path_does_not_match_an_endpoint() {
    let fx = Fixture::new("trailing");
    fx.write("a.md", b"# a");
    for op in OPS {
        let uri = format!("{}/", fx.uri(op, ""));
        let reply = send(&fx.router, "GET", &uri).await;
        assert_eq!(reply.status, StatusCode::NOT_FOUND, "{uri}：{}", reply.raw);
        assert!(!reply.raw.contains("a.md"), "{uri}：{}", reply.raw);
    }
    for op in ["meta", "render", "raw"] {
        let reply = fx.get(op, "").await;
        assert_eq!(reply.status, StatusCode::NOT_FOUND, "{op}：{}", reply.raw);
    }
}
