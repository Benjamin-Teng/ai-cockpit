//! git-review task 3.2 驗收測試：git 端點第一批（`status`、`refs`、`log`、`commit`、`changes`、
//! `merge-base`）與共同規則（spec `git-review`「git 端點的共同規則」「狀態端點」「refs 端點」
//! 「commit 清單端點」「commit 詳情端點」「變更檔案清單端點」「共同祖先端點」；design D1–D6、D8）。
//!
//! 鷹架同 `cockpit/tests/files_endpoint.rs`：`tower::ServiceExt::oneshot` 打 `router`、投影用
//! `watch::channel` 手灌、暫存目錄用自製 `TempDir`。**與 files_endpoint.rs 不同的是**：這裡的暫存
//! 目錄是真正的 `git init` 出來的 repo（brief 要求），一律建在 `std::env::temp_dir()` 下（不在本
//! repo 子目錄建 repo），建立後以 `git -C <dir> rev-parse --show-toplevel` 驗證等於該目錄；所有
//! fixture 用的 git 呼叫都帶 `-c user.name=x -c user.email=x@x -c commit.gpgsign=false`，不改動
//! 全域設定。「擁有者不符不繞過」不在本機製造不同擁有者的 repo（brief 明文禁止），只在
//! `cockpit/src/git.rs` 的單元測試（`classify_runner_error_matches_design_d6_table`）與本檔的原始碼
//! 掃描測試（`no_code_adds_safe_directory_to_bypass_ownership_check`）涵蓋。

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::AtomicU16;
use std::time::{SystemTime, UNIX_EPOCH};

use axum::body::Body;
use axum::http::{Request, StatusCode};
use cockpit::files::{FileSettings, PathMapping};
use cockpit::http::{self, AppState};
use cockpit_core::{ProjectedState, RuntimeId};
use cockpit_git::GitRunner;
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tokio::sync::watch;
use tower::ServiceExt;

// ---------------------------------------------------------------------------
// 鷹架：暫存 git repo
// ---------------------------------------------------------------------------

/// 測試用暫存目錄（同 `cockpit/tests/files_endpoint.rs`；drop 時整個刪掉）。
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
            "cockpit-git-test-{tag}-{}-{nanos}",
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

/// 跑一次 fixture 用的 git 呼叫：固定帶 `-c user.name=x -c user.email=x@x
/// -c commit.gpgsign=false`（brief 要求，不改全域設定），失敗就 panic 附上 stderr。
fn git_ok(dir: &Path, args: &[&str]) -> String {
    let output = git_raw(dir, args);
    assert!(
        output.status.success(),
        "git {args:?} 失敗：{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

/// 同 [`git_ok`]，但不斷言成功——只給「合法會以非零結束」的呼叫用（例如製造合併衝突、
/// 驗證 merge-base 沒有共同祖先）。
fn git_raw(dir: &Path, args: &[&str]) -> std::process::Output {
    Command::new("git")
        .args([
            "-c",
            "user.name=x",
            "-c",
            "user.email=x@x",
            "-c",
            "commit.gpgsign=false",
        ])
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .unwrap_or_else(|e| panic!("執行 git -C {} {args:?} 失敗：{e}", dir.display()))
}

fn write_file(repo: &Path, rel: &str, content: &[u8]) {
    let path = repo.join(rel);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("建立上層資料夾");
    }
    fs::write(&path, content).unwrap_or_else(|e| panic!("寫入 {rel} 失敗：{e}"));
}

/// `git init` 一個新 repo（`main` 為預設分支），驗證 `rev-parse --show-toplevel` 等於這個目錄
/// （brief 驗收要求）。
fn init_repo(tmp: &TempDir, name: &str) -> PathBuf {
    let dir = tmp.path().join(name);
    fs::create_dir_all(&dir).expect("建立 repo 目錄");
    git_ok(&dir, &["init", "--quiet", "-b", "main"]);

    let reported = git_ok(&dir, &["rev-parse", "--show-toplevel"]);
    let reported_path = PathBuf::from(reported.replace('/', "\\"));
    let expected = fs::canonicalize(&dir).expect("canonicalize 建立的目錄");
    let actual = fs::canonicalize(&reported_path).unwrap_or(reported_path);
    assert_eq!(
        actual, expected,
        "git 回報的 toplevel 必須等於建立的目錄（brief 驗收要求）"
    );
    dir
}

fn commit_all(repo: &Path, message: &str) -> String {
    git_ok(repo, &["add", "-A"]);
    git_ok(repo, &["commit", "-m", message]);
    git_ok(repo, &["rev-parse", "HEAD"])
}

fn head_oid(repo: &Path) -> String {
    git_ok(repo, &["rev-parse", "HEAD"])
}

fn path_str(path: &Path) -> &str {
    path.to_str().expect("暫存路徑應是合法 UTF-8")
}

// ---------------------------------------------------------------------------
// 鷹架：AppState／HTTP
// ---------------------------------------------------------------------------

const WIN: &str = "win";

fn projected(cwd: &str) -> Arc<ProjectedState> {
    let state = json!({
        "version": 1,
        "generated_at": "2026-09-29T00:00:00Z",
        "runtimes": [{
            "id": WIN,
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
                    "panes": [{
                        "id": "w1:p1",
                        "agent": null,
                        "agent_status": "idle",
                        "title": null,
                        "cwd": cwd,
                        "label": null,
                        "focused": false,
                        "exited": false,
                        "updated_at": "2026-09-29T00:00:00Z"
                    }]
                }]
            }]
        }],
        "projects": [],
        "recent_events": []
    });
    Arc::new(serde_json::from_value(state).expect("手組投影應能反序列化"))
}

/// 一個以 `repo` 為根目錄（`win` runtime 的唯一 pane）的路由表；回傳 `(router, root_id,
/// git_runner)`——`git_runner` 讓測試斷言「沒有啟動任何子程序」（`spawned()`）。
struct Fixture {
    router: axum::Router,
    root_id: String,
    git_runner: Arc<GitRunner>,
    _tx: watch::Sender<Arc<ProjectedState>>,
}

impl Fixture {
    fn new(repo: &Path) -> Self {
        let (tx, rx) = watch::channel(projected(path_str(repo)));
        let mut path_mappings = HashMap::new();
        path_mappings.insert(RuntimeId::new(WIN), PathMapping::Native);
        let git_runner = Arc::new(GitRunner::new());
        let app = AppState {
            state: rx,
            progress: None,
            port: Arc::new(AtomicU16::new(0)),
            runtimes: Arc::new(HashMap::new()),
            path_mappings: Arc::new(path_mappings),
            files: Arc::new(FileSettings::embedded()),
            git_runner: Arc::clone(&git_runner),
        };
        let root_id = cockpit::files::encode_root_id(repo).expect("repo 路徑應可編碼");
        Self {
            router: http::router(app),
            root_id,
            git_runner,
            _tx: tx,
        }
    }

    fn uri(&self, suffix: &str) -> String {
        format!("/api/git/{WIN}/{}{suffix}", self.root_id)
    }

    async fn get(&self, suffix: &str) -> Reply {
        send(&self.router, "GET", &self.uri(suffix)).await
    }
}

struct Reply {
    status: StatusCode,
    raw: String,
    json: Value,
    /// git-review task 3.3：`blob`／`render` 需要斷言 `Content-Type`／`Content-Security-Policy`
    /// 標頭（既有測試都只看 `status`／`raw`／`json`，新增欄位不影響它們）。
    headers: axum::http::HeaderMap,
}

impl Reply {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers.get(name).and_then(|v| v.to_str().ok())
    }
}

async fn send(router: &axum::Router, method: &str, uri: &str) -> Reply {
    let request = Request::builder()
        .method(method)
        .uri(uri)
        .header("host", "127.0.0.1:0")
        .body(Body::empty())
        .expect("request 建構不應該失敗");
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
        raw,
        json,
        headers,
    }
}

fn assert_coded_error(reply: &Reply, status: StatusCode, code: &str) {
    assert_eq!(reply.status, status, "本體：{}", reply.raw);
    assert_eq!(reply.json["code"], code, "本體：{}", reply.raw);
    assert!(
        reply.json["error"].as_str().is_some_and(|s| !s.is_empty()),
        "error 應為非空字串：{}",
        reply.raw
    );
}

fn find_entry<'a>(entries: &'a [Value], path: &str) -> &'a Value {
    entries
        .iter()
        .find(|e| e["path"] == path)
        .unwrap_or_else(|| panic!("找不到 {path}：{entries:?}"))
}

fn find_entry_in_group<'a>(entries: &'a [Value], path: &str, group: &str) -> &'a Value {
    entries
        .iter()
        .find(|e| e["path"] == path && e["group"] == group)
        .unwrap_or_else(|| panic!("找不到 {path}（{group}）：{entries:?}"))
}

// ===========================================================================
// git 端點的共同規則
// ===========================================================================

/// Scenario: 不是 git repo
#[tokio::test]
async fn common_scenario_not_a_git_repo() {
    let tmp = TempDir::new("not-git");
    let dir = tmp.path().join("plain");
    fs::create_dir_all(&dir).expect("建立非 git 資料夾");
    let fx = Fixture::new(&dir);

    let reply = fx.get("/status").await;

    assert_coded_error(&reply, StatusCode::CONFLICT, "not_git");
    assert_eq!(fx.git_runner.spawned(), 0, "not_git 不應啟動任何子程序");
}

/// Scenario: 版本語法被拒
#[tokio::test]
async fn common_scenario_bad_version_syntax_is_rejected() {
    let tmp = TempDir::new("bad-version");
    let repo = init_repo(&tmp, "repo");
    write_file(&repo, "a.txt", b"1");
    commit_all(&repo, "base");
    let fx = Fixture::new(&repo);

    for from in ["HEAD~1", "main", "-p", ":/fix", "ABCDEF"] {
        let uri = format!("/changes?from={from}&to=INDEX");
        let reply = fx.get(&uri).await;
        assert_coded_error(&reply, StatusCode::BAD_REQUEST, "bad_request");
    }
    assert_eq!(fx.git_runner.spawned(), 0, "版本語法被拒不應啟動任何子程序");
}

/// Scenario: 不存在的 commit
#[tokio::test]
async fn common_scenario_nonexistent_commit_is_rev_unknown() {
    let tmp = TempDir::new("rev-unknown");
    let repo = init_repo(&tmp, "repo");
    write_file(&repo, "a.txt", b"1");
    commit_all(&repo, "base");
    let fx = Fixture::new(&repo);

    let zeros = "0".repeat(40);
    let reply = fx.get(&format!("/commit/{zeros}")).await;
    assert_coded_error(&reply, StatusCode::NOT_FOUND, "rev_unknown");
}

/// Scenario: 同源檢查
#[tokio::test]
async fn common_scenario_dns_rebinding_is_forbidden_source() {
    let tmp = TempDir::new("rebinding");
    let repo = init_repo(&tmp, "repo");
    write_file(&repo, "a.txt", b"1");
    commit_all(&repo, "base");
    let fx = Fixture::new(&repo);

    let request = Request::builder()
        .method("GET")
        .uri(fx.uri("/status"))
        .header("host", "evil.example:7770")
        .body(Body::empty())
        .expect("request 建構不應該失敗");
    let response = fx
        .router
        .clone()
        .oneshot(request)
        .await
        .expect("oneshot 呼叫不應該失敗");
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        fx.git_runner.spawned(),
        0,
        "來源檢查擋下時不應啟動任何子程序"
    );
}

/// 「擁有者不符不繞過」：不在本機製造不同擁有者的 repo（brief 明文禁止），改為掃描原始碼確認
/// 沒有任何程式碼加 `safe.directory` 來繞過 git 的 ownership 檢查（`RunnerError::Untrusted` →
/// `git_untrusted` 的對應由 `cockpit/src/git.rs` 的單元測試
/// `classify_runner_error_matches_design_d6_table` 涵蓋）。
#[test]
fn no_code_adds_safe_directory_to_bypass_ownership_check() {
    for crate_dir in ["cockpit/src", "cockpit-git/src"] {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join(crate_dir);
        assert!(
            !scan_dir_for(&root, "safe.directory"),
            "{crate_dir} 不得出現 safe.directory（會繞過 dubious ownership 檢查）"
        );
    }
}

/// Ruling P2：`cockpit` crate 內不得出現任何 `Command::new`——執行 git 只能經
/// `cockpit_git::GitRunner::run`。
#[test]
fn cockpit_crate_never_spawns_a_command_directly() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    assert!(
        !scan_dir_for(&root, "Command::new"),
        "cockpit/src 不得出現 Command::new（P2：執行 git 只能經 GitRunner::run）"
    );
}

fn scan_dir_for(dir: &Path, needle: &str) -> bool {
    let Ok(entries) = fs::read_dir(dir) else {
        return false;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if scan_dir_for(&path, needle) {
                return true;
            }
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            let content = fs::read_to_string(&path).unwrap_or_default();
            if content.contains(needle) {
                return true;
            }
        }
    }
    false
}

// ===========================================================================
// 狀態端點
// ===========================================================================

/// Scenario: 各組變更
#[tokio::test]
async fn status_scenario_each_group() {
    let tmp = TempDir::new("status-groups");
    let repo = init_repo(&tmp, "repo");
    write_file(&repo, "a.rs", b"1");
    write_file(&repo, "b.rs", b"1");
    write_file(&repo, "c.rs", b"1");
    write_file(&repo, "old.txt", b"1");
    commit_all(&repo, "base");

    write_file(&repo, "a.rs", b"2");
    git_ok(&repo, &["add", "a.rs"]);

    write_file(&repo, "b.rs", b"2");

    write_file(&repo, "c.rs", b"2");
    git_ok(&repo, &["add", "c.rs"]);
    write_file(&repo, "c.rs", b"3");

    write_file(&repo, "new.md", b"x");

    git_ok(&repo, &["mv", "old.txt", "renamed.txt"]);

    let fx = Fixture::new(&repo);
    let reply = fx.get("/status").await;

    assert_eq!(reply.status, StatusCode::OK, "本體：{}", reply.raw);
    assert_eq!(reply.json["branch"]["head"], "main");
    let entries = reply.json["entries"].as_array().expect("entries 應為陣列");

    let a = find_entry_in_group(entries, "a.rs", "staged");
    assert_eq!(a["status"], "M");
    assert!(a["icon"].as_str().is_some_and(|s| !s.is_empty()));

    assert_eq!(
        find_entry_in_group(entries, "b.rs", "unstaged")["status"],
        "M"
    );
    assert_eq!(
        find_entry_in_group(entries, "c.rs", "staged")["status"],
        "M"
    );
    assert_eq!(
        find_entry_in_group(entries, "c.rs", "unstaged")["status"],
        "M"
    );
    assert_eq!(
        find_entry_in_group(entries, "new.md", "untracked")["status"],
        "?"
    );
    let renamed = find_entry_in_group(entries, "renamed.txt", "staged");
    assert_eq!(renamed["status"], "R");
    assert_eq!(renamed["old_path"], "old.txt");
}

/// Scenario: 合併衝突
#[tokio::test]
async fn status_scenario_merge_conflict() {
    let tmp = TempDir::new("status-conflict");
    let repo = init_repo(&tmp, "repo");
    write_file(&repo, "conflict.txt", b"base\n");
    commit_all(&repo, "base");

    git_ok(&repo, &["checkout", "-b", "feature"]);
    write_file(&repo, "conflict.txt", b"feature\n");
    commit_all(&repo, "feature change");

    git_ok(&repo, &["checkout", "main"]);
    write_file(&repo, "conflict.txt", b"main\n");
    commit_all(&repo, "main change");

    // 合併會因為兩側都改了 conflict.txt 而衝突，非零結束是預期中的，不用 git_ok。
    let merge = git_raw(&repo, &["merge", "feature", "--no-edit"]);
    assert!(!merge.status.success(), "這個合併應該要衝突");

    let fx = Fixture::new(&repo);
    let reply = fx.get("/status").await;

    assert_eq!(reply.status, StatusCode::OK, "本體：{}", reply.raw);
    let entries = reply.json["entries"].as_array().expect("entries 應為陣列");
    let conflict = find_entry_in_group(entries, "conflict.txt", "conflict");
    assert_eq!(conflict["status"], "U");
}

/// Scenario: 還沒有 commit 的 repo
#[tokio::test]
async fn status_scenario_repo_without_any_commit_yet() {
    let tmp = TempDir::new("status-no-commit");
    let repo = init_repo(&tmp, "repo");
    write_file(&repo, "a.txt", b"1");
    git_ok(&repo, &["add", "a.txt"]);

    let fx = Fixture::new(&repo);
    let reply = fx.get("/status").await;

    assert_eq!(reply.status, StatusCode::OK, "本體：{}", reply.raw);
    assert_eq!(reply.json["branch"]["oid"], Value::Null);
    let entries = reply.json["entries"].as_array().expect("entries 應為陣列");
    assert_eq!(find_entry(entries, "a.txt")["group"], "staged");
}

// ===========================================================================
// refs 端點
// ===========================================================================

/// Scenario: 分支、遠端與 tag
#[tokio::test]
async fn refs_scenario_branches_remote_and_tag() {
    let tmp = TempDir::new("refs");
    let repo = init_repo(&tmp, "repo");
    write_file(&repo, "a.txt", b"1");
    let main_oid = commit_all(&repo, "base");

    git_ok(&repo, &["checkout", "-b", "feat/x"]);
    write_file(&repo, "b.txt", b"1");
    commit_all(&repo, "feat commit");

    git_ok(&repo, &["checkout", "main"]);
    git_ok(&repo, &["tag", "-a", "v0.1", "-m", "v0.1", &main_oid]);
    git_ok(
        &repo,
        &["update-ref", "refs/remotes/origin/main", &main_oid],
    );

    // 一筆 stash：不應出現在 refs 清單裡。
    write_file(&repo, "a.txt", b"2");
    git_ok(&repo, &["stash", "push", "-m", "wip"]);

    let fx = Fixture::new(&repo);
    let reply = fx.get("/refs").await;

    assert_eq!(reply.status, StatusCode::OK, "本體：{}", reply.raw);
    assert_eq!(reply.json["head"]["ref"], "refs/heads/main");
    let refs = reply.json["refs"].as_array().expect("refs 應為陣列");
    assert_eq!(refs.len(), 4, "{refs:?}");

    let find = |name: &str| {
        refs.iter()
            .find(|r| r["name"] == name)
            .unwrap_or_else(|| panic!("找不到 {name}：{refs:?}"))
    };
    assert_eq!(find("refs/heads/main")["kind"], "branch");
    assert_eq!(find("refs/heads/feat/x")["kind"], "branch");
    assert_eq!(find("refs/remotes/origin/main")["kind"], "remote");
    let tag = find("refs/tags/v0.1");
    assert_eq!(tag["kind"], "tag");
    assert_eq!(tag["oid"], main_oid);
    assert!(
        refs.iter().all(|r| r["commit"] == true),
        "此 scenario 的所有 ref 都指向 commit：{refs:?}"
    );
    assert!(
        !refs
            .iter()
            .any(|r| r["name"].as_str().unwrap_or("").starts_with("refs/stash")),
        "不應有任何 refs/stash 項目：{refs:?}"
    );
}

/// ui-fixes task 3.1，Scenario: tag 指向非 commit 的物件（refs 端點）＋預設起點不含該 oid
/// （commit 清單端點）。真實 repo 建一個輕量 tag 指向 tree：refs 仍列出、`commit` 為 false，
/// 預設 log 不因壞起點整個失敗、`tips` 不含該 tree 的 oid。
#[tokio::test]
async fn refs_and_default_log_handle_tag_pointing_at_tree() {
    let tmp = TempDir::new("refs-tree-tag");
    let repo = init_repo(&tmp, "repo");
    write_file(&repo, "a.txt", b"1");
    let main_oid = commit_all(&repo, "base");
    let tree_oid = git_ok(&repo, &["rev-parse", "HEAD^{tree}"]);
    git_ok(&repo, &["tag", "tree-tag", &tree_oid]);

    let fx = Fixture::new(&repo);
    let reply = fx.get("/refs").await;
    assert_eq!(reply.status, StatusCode::OK, "本體：{}", reply.raw);
    let refs = reply.json["refs"].as_array().expect("refs 應為陣列");
    let find = |name: &str| {
        refs.iter()
            .find(|r| r["name"] == name)
            .unwrap_or_else(|| panic!("找不到 {name}：{refs:?}"))
    };
    let tree_tag = find("refs/tags/tree-tag");
    assert_eq!(tree_tag["kind"], "tag");
    assert_eq!(tree_tag["oid"], tree_oid);
    assert_eq!(tree_tag["commit"], false);
    assert_eq!(find("refs/heads/main")["commit"], true);
    assert_eq!(find("refs/heads/main")["oid"], main_oid);

    let log = fx.get("/log").await;
    assert_eq!(log.status, StatusCode::OK, "本體：{}", log.raw);
    let tips: Vec<&str> = log.json["tips"]
        .as_array()
        .expect("tips 應為陣列")
        .iter()
        .map(|v| v.as_str().expect("tip 應為字串"))
        .collect();
    assert_eq!(tips, [main_oid.as_str()], "tips 不得含 tree 的 oid");
    assert_eq!(log.json["rows"].as_array().expect("rows").len(), 1);
}

// ===========================================================================
// commit 清單端點
// ===========================================================================

fn commit_empty(repo: &Path, message: &str) -> String {
    git_ok(repo, &["commit", "--allow-empty", "-m", message]);
    head_oid(repo)
}

/// Scenario: 分批載入一致
#[tokio::test]
async fn log_scenario_batched_loading_is_consistent() {
    let tmp = TempDir::new("log-batches");
    let repo = init_repo(&tmp, "repo");
    for i in 0..260 {
        commit_empty(&repo, &format!("commit {i}"));
    }
    let fx = Fixture::new(&repo);

    let first = fx.get("/log?limit=200").await;
    assert_eq!(first.status, StatusCode::OK, "本體：{}", first.raw);
    assert_eq!(first.json["has_more"], true);
    let tips: Vec<String> = first.json["tips"]
        .as_array()
        .expect("tips 應為陣列")
        .iter()
        .map(|v| v.as_str().expect("tip 應為字串").to_string())
        .collect();
    let tip_query: String = tips
        .iter()
        .map(|t| format!("tip={t}"))
        .collect::<Vec<_>>()
        .join("&");

    let second = fx
        .get(&format!("/log?{tip_query}&offset=200&limit=200"))
        .await;
    assert_eq!(second.status, StatusCode::OK, "本體：{}", second.raw);
    let second_rows = second.json["rows"].as_array().expect("rows 應為陣列");
    assert_eq!(second_rows.len(), 60);
    assert_eq!(second.json["has_more"], false);

    let whole_first = fx
        .get(&format!("/log?{tip_query}&offset=0&limit=200"))
        .await;
    let whole_second = fx
        .get(&format!("/log?{tip_query}&offset=150&limit=100"))
        .await;
    assert_eq!(
        whole_second.status,
        StatusCode::OK,
        "本體：{}",
        whole_second.raw
    );

    let mut by_oid: HashMap<String, Value> = HashMap::new();
    for row in whole_first.json["rows"].as_array().unwrap() {
        by_oid.insert(
            row["oid"].as_str().unwrap().to_string(),
            row["graph"].clone(),
        );
    }
    for row in whole_second.json["rows"].as_array().unwrap() {
        let oid = row["oid"].as_str().unwrap().to_string();
        if let Some(expected_graph) = by_oid.get(&oid) {
            assert_eq!(
                &row["graph"], expected_graph,
                "同一個 commit 在不同分法下的 graph 必須完全相同：{oid}"
            );
        }
    }
}

/// Scenario: 分支與合併的排版
#[tokio::test]
async fn log_scenario_branch_and_merge_layout() {
    let tmp = TempDir::new("log-merge");
    let repo = init_repo(&tmp, "repo");
    commit_empty(&repo, "A");
    let b = commit_empty(&repo, "B");
    git_ok(&repo, &["checkout", "-b", "feat"]);
    let c = commit_empty(&repo, "C");
    git_ok(&repo, &["checkout", "main"]);
    let d = commit_empty(&repo, "D");
    git_ok(&repo, &["merge", "feat", "--no-ff", "--no-edit", "-m", "E"]);
    let e = head_oid(&repo);
    assert_ne!(b, c);
    let _ = d;

    let fx = Fixture::new(&repo);
    let reply = fx.get("/log").await;
    assert_eq!(reply.status, StatusCode::OK, "本體：{}", reply.raw);
    let rows = reply.json["rows"].as_array().expect("rows 應為陣列");

    let find = |oid: &str| {
        rows.iter()
            .find(|r| r["oid"] == oid)
            .unwrap_or_else(|| panic!("找不到 {oid}：{rows:?}"))
    };
    let e_row = find(&e);
    let parents: Vec<&str> = e_row["parents"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p.as_str().unwrap())
        .collect();
    assert_eq!(parents.len(), 2, "E 應有兩個 parent");

    let d_row = find(parents[0]);
    let c_row = find(parents[1]);
    let b_row = find(&b);
    let a_col = find(rows.last().unwrap()["oid"].as_str().unwrap())["graph"]["col"].clone();

    assert_eq!(
        e_row["graph"]["col"], d_row["graph"]["col"],
        "E 與 D 應同欄"
    );
    assert_eq!(
        d_row["graph"]["col"], b_row["graph"]["col"],
        "D 與 B 應同欄"
    );
    assert_eq!(b_row["graph"]["col"], a_col, "B 與 A 應同欄");
    assert_ne!(
        c_row["graph"]["col"], e_row["graph"]["col"],
        "C 的欄應與 E 不同"
    );
}

/// Scenario: 依分支篩選
#[tokio::test]
async fn log_scenario_filter_by_branch() {
    let tmp = TempDir::new("log-filter");
    let repo = init_repo(&tmp, "repo");
    commit_empty(&repo, "A");
    git_ok(&repo, &["checkout", "-b", "feat/x"]);
    let extra1 = commit_empty(&repo, "extra1");
    let extra2 = commit_empty(&repo, "extra2");
    git_ok(&repo, &["checkout", "main"]);

    let fx = Fixture::new(&repo);
    let reply = fx.get("/log?ref=refs/heads/main").await;
    assert_eq!(reply.status, StatusCode::OK, "本體：{}", reply.raw);
    let rows = reply.json["rows"].as_array().expect("rows 應為陣列");
    assert!(!rows.iter().any(|r| r["oid"] == extra1));
    assert!(!rows.iter().any(|r| r["oid"] == extra2));
}

/// Scenario: 不在清單中的 ref
#[tokio::test]
async fn log_scenario_ref_not_in_list() {
    let tmp = TempDir::new("log-ref-unknown");
    let repo = init_repo(&tmp, "repo");
    write_file(&repo, "a.txt", b"1");
    commit_all(&repo, "base");
    let fx = Fixture::new(&repo);

    let reply = fx.get("/log?ref=refs/heads/does-not-exist").await;
    assert_coded_error(&reply, StatusCode::NOT_FOUND, "ref_unknown");

    let reply = fx.get("/log?ref=HEAD").await;
    assert_coded_error(&reply, StatusCode::NOT_FOUND, "ref_unknown");
}

#[tokio::test]
async fn log_ref_and_tip_together_is_bad_request() {
    let tmp = TempDir::new("log-ref-tip");
    let repo = init_repo(&tmp, "repo");
    write_file(&repo, "a.txt", b"1");
    let oid = commit_all(&repo, "base");
    let fx = Fixture::new(&repo);

    let reply = fx.get(&format!("/log?ref=refs/heads/main&tip={oid}")).await;
    assert_coded_error(&reply, StatusCode::BAD_REQUEST, "bad_request");
    assert_eq!(fx.git_runner.spawned(), 0);
}

#[tokio::test]
async fn log_offset_plus_limit_over_5000_is_bad_request() {
    let tmp = TempDir::new("log-over-cap");
    let repo = init_repo(&tmp, "repo");
    write_file(&repo, "a.txt", b"1");
    commit_all(&repo, "base");
    let fx = Fixture::new(&repo);

    let reply = fx.get("/log?offset=4900&limit=200").await;
    assert_coded_error(&reply, StatusCode::BAD_REQUEST, "bad_request");
    assert_eq!(fx.git_runner.spawned(), 0);
}

/// fix round 1（控制端裁決）：`tip` 指向不存在的 commit 時，`Log` 查詢本身會非零結束；
/// 這裡要細分成 404 `rev_unknown`，不是籠統的 `git_failed`。
#[tokio::test]
async fn log_scenario_tip_pointing_to_nonexistent_commit_is_rev_unknown() {
    let tmp = TempDir::new("log-tip-unknown");
    let repo = init_repo(&tmp, "repo");
    write_file(&repo, "a.txt", b"1");
    commit_all(&repo, "base");
    let fx = Fixture::new(&repo);

    let zeros = "0".repeat(40);
    let reply = fx.get(&format!("/log?tip={zeros}")).await;
    assert_coded_error(&reply, StatusCode::NOT_FOUND, "rev_unknown");
}

/// 同上，但混合一個存在與一個不存在的 tip：只要有任何一個不是這個 repo 的 commit 就
/// 回 `rev_unknown`。
#[tokio::test]
async fn log_scenario_one_valid_and_one_invalid_tip_is_rev_unknown() {
    let tmp = TempDir::new("log-tip-mixed");
    let repo = init_repo(&tmp, "repo");
    write_file(&repo, "a.txt", b"1");
    let oid = commit_all(&repo, "base");
    let fx = Fixture::new(&repo);

    let zeros = "0".repeat(40);
    let reply = fx.get(&format!("/log?tip={oid}&tip={zeros}")).await;
    assert_coded_error(&reply, StatusCode::NOT_FOUND, "rev_unknown");
}

// ===========================================================================
// commit 詳情端點
// ===========================================================================

/// Scenario: merge commit 與第一個 parent 比較
#[tokio::test]
async fn commit_scenario_merge_commit_compares_to_first_parent() {
    let tmp = TempDir::new("commit-merge");
    let repo = init_repo(&tmp, "repo");
    write_file(&repo, "common.txt", b"base");
    commit_all(&repo, "A");
    write_file(&repo, "common.txt", b"B");
    let _b = commit_all(&repo, "B");

    git_ok(&repo, &["checkout", "-b", "feat"]);
    write_file(&repo, "feat.txt", b"C content");
    commit_all(&repo, "C");

    git_ok(&repo, &["checkout", "main"]);
    write_file(&repo, "main.txt", b"D content");
    let d = commit_all(&repo, "D");

    git_ok(&repo, &["merge", "feat", "--no-ff", "--no-edit", "-m", "E"]);
    let e = head_oid(&repo);

    let fx = Fixture::new(&repo);
    let reply = fx.get(&format!("/commit/{e}")).await;

    assert_eq!(reply.status, StatusCode::OK, "本體：{}", reply.raw);
    assert_eq!(reply.json["compared_to"], d);
    let files = reply.json["files"].as_array().expect("files 應為陣列");
    let feat = find_entry(files, "feat.txt");
    assert_eq!(feat["status"], "A");
}

/// Scenario: 根 commit
#[tokio::test]
async fn commit_scenario_root_commit_has_no_compared_to() {
    let tmp = TempDir::new("commit-root");
    let repo = init_repo(&tmp, "repo");
    write_file(&repo, "a.txt", b"1");
    write_file(&repo, "b.txt", b"1");
    let root = commit_all(&repo, "root");

    let fx = Fixture::new(&repo);
    let reply = fx.get(&format!("/commit/{root}")).await;

    assert_eq!(reply.status, StatusCode::OK, "本體：{}", reply.raw);
    assert_eq!(reply.json["compared_to"], Value::Null);
    let files = reply.json["files"].as_array().expect("files 應為陣列");
    assert_eq!(files.len(), 2);
    for file in files {
        assert_eq!(file["status"], "A");
    }
}

// ===========================================================================
// 變更檔案清單端點
// ===========================================================================

/// Scenario: 兩個 commit 之間
#[tokio::test]
async fn changes_scenario_between_two_commits() {
    let tmp = TempDir::new("changes-between");
    let repo = init_repo(&tmp, "repo");
    write_file(&repo, "m.rs", b"a\nb\nc\nd\n");
    write_file(&repo, "d.txt", b"stuff");
    write_file(&repo, "o.md", b"content");
    write_file(&repo, "img.png", b"\x89PNG\x00binary\x00data");
    let x = commit_all(&repo, "X");

    write_file(&repo, "n.md", b"new file");
    write_file(&repo, "m.rs", b"a\nc\nd\ne\nf\ng\n");
    fs::remove_file(repo.join("d.txt")).expect("刪除 d.txt");
    git_ok(&repo, &["mv", "o.md", "p.md"]);
    write_file(
        &repo,
        "img.png",
        b"\x89PNG\x00changed\x00binary\x00data\x00more",
    );
    let y = commit_all(&repo, "Y");

    let fx = Fixture::new(&repo);
    let reply = fx.get(&format!("/changes?from={x}&to={y}")).await;

    assert_eq!(reply.status, StatusCode::OK, "本體：{}", reply.raw);
    let files = reply.json["files"].as_array().expect("files 應為陣列");

    assert_eq!(find_entry(files, "n.md")["status"], "A");
    let m = find_entry(files, "m.rs");
    assert_eq!(m["status"], "M");
    assert_eq!(m["additions"], 3);
    assert_eq!(m["deletions"], 1);
    assert_eq!(find_entry(files, "d.txt")["status"], "D");
    let p = find_entry(files, "p.md");
    assert_eq!(p["status"], "R");
    assert_eq!(p["old_path"], "o.md");
    let img = find_entry(files, "img.png");
    assert_eq!(img["status"], "M");
    assert_eq!(img["additions"], Value::Null);
    assert_eq!(img["deletions"], Value::Null);
}

/// Scenario: 不允許的組合
#[tokio::test]
async fn changes_scenario_disallowed_combination() {
    let tmp = TempDir::new("changes-bad-combo");
    let repo = init_repo(&tmp, "repo");
    write_file(&repo, "a.txt", b"1");
    commit_all(&repo, "base");
    let fx = Fixture::new(&repo);

    let reply = fx.get("/changes?from=WORKTREE&to=INDEX").await;
    assert_coded_error(&reply, StatusCode::BAD_REQUEST, "bad_request");
    assert_eq!(fx.git_runner.spawned(), 0);
}

/// fix round 1（控制端裁決）：`from`／`to` 的 hash 不是這個 repo 的 commit → 404
/// `rev_unknown`（spec 共同規則情境「不存在的 commit」也適用於這個端點的版本參數）。
#[tokio::test]
async fn changes_scenario_nonexistent_commit_is_rev_unknown() {
    let tmp = TempDir::new("changes-rev-unknown");
    let repo = init_repo(&tmp, "repo");
    write_file(&repo, "a.txt", b"1");
    commit_all(&repo, "base");
    let fx = Fixture::new(&repo);

    let zeros = "0".repeat(40);
    let reply = fx.get(&format!("/changes?from={zeros}&to=INDEX")).await;
    assert_coded_error(&reply, StatusCode::NOT_FOUND, "rev_unknown");
}

// ===========================================================================
// 共同祖先端點
// ===========================================================================

/// Scenario: 分支的分岔點
#[tokio::test]
async fn merge_base_scenario_branch_point() {
    let tmp = TempDir::new("merge-base");
    let repo = init_repo(&tmp, "repo");
    commit_empty(&repo, "A");
    let b = commit_empty(&repo, "B");
    git_ok(&repo, &["checkout", "-b", "feat"]);
    let c = commit_empty(&repo, "C");
    git_ok(&repo, &["checkout", "main"]);
    let d = commit_empty(&repo, "D");
    let _ = d;

    let fx = Fixture::new(&repo);
    let reply = fx
        .get(&format!("/merge-base?a={d}&b={c}", d = head_oid(&repo)))
        .await;

    assert_eq!(reply.status, StatusCode::OK, "本體：{}", reply.raw);
    assert_eq!(reply.json["oid"], b);
}

#[tokio::test]
async fn merge_base_scenario_no_common_ancestor() {
    let tmp = TempDir::new("merge-base-none");
    let repo = init_repo(&tmp, "repo");
    let main_tip = commit_empty(&repo, "main root");

    git_ok(&repo, &["checkout", "--orphan", "orphan"]);
    let orphan_tip = commit_empty(&repo, "orphan root");

    let fx = Fixture::new(&repo);
    let reply = fx
        .get(&format!("/merge-base?a={main_tip}&b={orphan_tip}"))
        .await;

    assert_coded_error(&reply, StatusCode::CONFLICT, "no_merge_base");
}

#[tokio::test]
async fn merge_base_with_unknown_hash_is_rev_unknown() {
    let tmp = TempDir::new("merge-base-unknown");
    let repo = init_repo(&tmp, "repo");
    write_file(&repo, "a.txt", b"1");
    let oid = commit_all(&repo, "base");
    let fx = Fixture::new(&repo);

    let zeros = "0".repeat(40);
    let reply = fx.get(&format!("/merge-base?a={oid}&b={zeros}")).await;
    assert_coded_error(&reply, StatusCode::NOT_FOUND, "rev_unknown");
}

// ===========================================================================
// 單檔 diff 端點（git-review task 3.3；spec「單檔 diff 端點」；design D7）
// ===========================================================================

fn row<'a>(rows: &'a [Value], kind: &str, pred: impl Fn(&Value) -> bool) -> &'a Value {
    rows.iter()
        .find(|r| r["kind"] == kind && pred(r))
        .unwrap_or_else(|| panic!("找不到 kind={kind} 的列：{rows:?}"))
}

/// Scenario: 左右配對
#[tokio::test]
async fn diff_scenario_left_right_pairing() {
    let tmp = TempDir::new("diff-pairing");
    let repo = init_repo(&tmp, "repo");

    let baseline: Vec<String> = (1..=40).map(|n| format!("line{n}")).collect();
    write_file(&repo, "a.rs", baseline.join("\n").as_bytes());
    git_ok(&repo, &["add", "a.rs"]);

    let mut modified = baseline.clone();
    modified[9] = "line10-changed".to_string(); // 第 10 行被改寫
    modified.insert(11, "new-a".to_string()); // 第 11 行後新增兩行
    modified.insert(12, "new-b".to_string());
    assert_eq!(
        modified[31], "line30",
        "測試前提：刪除前索引 31 應為原本的第 30 行"
    );
    modified.remove(31); // 第 30 行被刪除
    write_file(&repo, "a.rs", modified.join("\n").as_bytes());

    let fx = Fixture::new(&repo);
    let reply = fx.get("/diff?from=INDEX&to=WORKTREE&path=a.rs").await;

    assert_eq!(reply.status, StatusCode::OK, "本體：{}", reply.raw);
    assert_eq!(reply.json["binary"], false);
    let rows = reply.json["rows"].as_array().expect("rows 應為陣列");

    assert_eq!(rows[0]["kind"], "gap", "第一列應省略第 1–6 行：{rows:?}");
    assert_eq!(rows[0]["lines"], 6);

    let change = row(rows, "change", |r| r["left"]["line"] == 10);
    assert_eq!(change["left"]["text"], "line10");
    assert_eq!(change["right"]["line"], 10);
    assert_eq!(change["right"]["text"], "line10-changed");

    let add_a = row(rows, "add", |r| r["right"]["text"] == "new-a");
    assert_eq!(add_a["right"]["line"], 12);
    let add_b = row(rows, "add", |r| r["right"]["text"] == "new-b");
    assert_eq!(add_b["right"]["line"], 13);

    let delete = row(rows, "delete", |r| r["left"]["text"] == "line30");
    assert_eq!(delete["left"]["line"], 30);

    let add_b_pos = rows
        .iter()
        .position(|r| r["right"]["text"] == "new-b")
        .expect("應找得到 new-b");
    let delete_pos = rows
        .iter()
        .position(|r| r["kind"] == "delete" && r["left"]["text"] == "line30")
        .expect("應找得到刪除列");
    assert!(
        rows[add_b_pos..delete_pos]
            .iter()
            .any(|r| r["kind"] == "gap"),
        "兩個變更區塊之間應有一列 gap：{rows:?}"
    );
}

/// Scenario: 內容更新後 version 改變
#[tokio::test]
async fn diff_scenario_version_changes_after_content_update() {
    let tmp = TempDir::new("diff-version");
    let repo = init_repo(&tmp, "repo");
    write_file(&repo, "a.rs", b"line1\nline2\n");
    git_ok(&repo, &["add", "a.rs"]);
    write_file(&repo, "a.rs", b"line1\nCHANGED\n");
    let fx = Fixture::new(&repo);
    let uri = "/diff?from=INDEX&to=WORKTREE&path=a.rs";

    let first = fx.get(uri).await;
    assert_eq!(first.status, StatusCode::OK, "本體：{}", first.raw);
    let v1 = first.json["version"].as_str().expect("version 應為字串");

    let second = fx.get(uri).await;
    assert_eq!(
        second.json["version"].as_str(),
        Some(v1),
        "未改檔案 version 應相同"
    );

    write_file(&repo, "a.rs", b"line1\nCHANGED-AGAIN\n");
    let third = fx.get(uri).await;
    assert_ne!(
        third.json["version"].as_str(),
        Some(v1),
        "改檔案後 version 應不同"
    );
}

/// Scenario: 未追蹤檔案
#[tokio::test]
async fn diff_scenario_untracked_file() {
    let tmp = TempDir::new("diff-untracked");
    let repo = init_repo(&tmp, "repo");
    write_file(&repo, "a.txt", b"1");
    commit_all(&repo, "base");
    write_file(&repo, "new.md", b"a\nb\nc");
    let fx = Fixture::new(&repo);

    let reply = fx.get("/diff?from=EMPTY&to=WORKTREE&path=new.md").await;

    assert_eq!(reply.status, StatusCode::OK, "本體：{}", reply.raw);
    let rows = reply.json["rows"].as_array().expect("rows 應為陣列");
    assert_eq!(rows.len(), 3);
    for (i, text) in ["a", "b", "c"].into_iter().enumerate() {
        assert_eq!(rows[i]["kind"], "add");
        assert_eq!(rows[i]["right"]["line"], (i + 1) as i64);
        assert_eq!(rows[i]["right"]["text"], text);
    }
}

/// Scenario: 二進位檔
#[tokio::test]
async fn diff_scenario_binary_file() {
    let tmp = TempDir::new("diff-binary");
    let repo = init_repo(&tmp, "repo");
    write_file(&repo, "img.png", &[0x89, 0x50, 0x4E, 0x47, 0, 1, 2, 3]);
    git_ok(&repo, &["add", "img.png"]);
    write_file(&repo, "img.png", &[0x89, 0x50, 0x4E, 0x47, 0, 9, 9, 9]);
    let fx = Fixture::new(&repo);

    let reply = fx.get("/diff?from=INDEX&to=WORKTREE&path=img.png").await;

    assert_eq!(reply.status, StatusCode::OK, "本體：{}", reply.raw);
    assert_eq!(reply.json["binary"], true);
    assert_eq!(
        reply.json["rows"].as_array().expect("rows 應為陣列").len(),
        0
    );
}

/// Scenario（共同規則）：跳出根目錄的路徑（WHEN 以 `path=../secret.txt` 請求 diff）。
#[tokio::test]
async fn diff_scenario_path_outside_root_is_bad_request() {
    let tmp = TempDir::new("diff-outside-root");
    let repo = init_repo(&tmp, "repo");
    write_file(&repo, "a.txt", b"1");
    commit_all(&repo, "base");
    let fx = Fixture::new(&repo);

    let reply = fx
        .get("/diff?from=INDEX&to=WORKTREE&path=../secret.txt")
        .await;

    assert_coded_error(&reply, StatusCode::BAD_REQUEST, "bad_request");
    assert_eq!(
        fx.git_runner.spawned(),
        0,
        "跳出根目錄的路徑不應啟動任何子程序"
    );
}

/// 未追蹤檔案（`EMPTY→WORKTREE`）帶 `old_path` 沒有意義，視為請求格式錯誤。
#[tokio::test]
async fn diff_untracked_with_old_path_is_bad_request() {
    let tmp = TempDir::new("diff-untracked-old-path");
    let repo = init_repo(&tmp, "repo");
    write_file(&repo, "a.txt", b"1");
    commit_all(&repo, "base");
    write_file(&repo, "new.md", b"x");
    let fx = Fixture::new(&repo);

    let reply = fx
        .get("/diff?from=EMPTY&to=WORKTREE&path=new.md&old_path=old.md")
        .await;

    assert_coded_error(&reply, StatusCode::BAD_REQUEST, "bad_request");
}

/// 目視驗收缺陷 V1、Ruling R11 前半段：合併衝突中的檔案，`from=<HEAD 的 hash>&to=WORKTREE`
/// 應回 200，衝突標記以 `add` 列呈現（spec「單檔 diff 端點」Scenario「合併衝突中的檔案」）。
#[tokio::test]
async fn diff_scenario_merge_conflict_hash_to_worktree_shows_conflict_markers() {
    let tmp = TempDir::new("diff-conflict-hash-worktree");
    let repo = init_repo(&tmp, "repo");
    write_file(&repo, "c.txt", b"base\n");
    commit_all(&repo, "base");

    git_ok(&repo, &["checkout", "-b", "branch-a"]);
    write_file(&repo, "c.txt", b"branch-a change\n");
    commit_all(&repo, "branch-a change");

    git_ok(&repo, &["checkout", "main"]);
    git_ok(&repo, &["checkout", "-b", "branch-b"]);
    write_file(&repo, "c.txt", b"branch-b change\n");
    let head = commit_all(&repo, "branch-b change");

    let merge = git_raw(&repo, &["merge", "branch-a", "--no-edit"]);
    assert!(!merge.status.success(), "這個合併應該要衝突");

    let fx = Fixture::new(&repo);
    let reply = fx
        .get(&format!("/diff?from={head}&to=WORKTREE&path=c.txt"))
        .await;

    assert_eq!(reply.status, StatusCode::OK, "本體：{}", reply.raw);
    let rows = reply.json["rows"].as_array().expect("rows 應為陣列");
    let add_texts: Vec<&str> = rows
        .iter()
        .filter(|r| r["kind"] == "add")
        .map(|r| r["right"]["text"].as_str().expect("text 應為字串"))
        .collect();
    assert!(
        add_texts.iter().any(|t| t.starts_with("<<<<<<<")),
        "應含衝突標記 <<<<<<<：{add_texts:?}"
    );
    assert!(
        add_texts.contains(&"======="),
        "應含衝突標記 =======：{add_texts:?}"
    );
    assert!(
        add_texts.iter().any(|t| t.starts_with(">>>>>>>")),
        "應含衝突標記 >>>>>>>：{add_texts:?}"
    );
}

/// 目視驗收缺陷 V1、Ruling R11 後半段：同一個衝突情境，`from=INDEX&to=WORKTREE` 應回 409
/// `unmerged_path`（spec「單檔 diff 端點」Scenario「合併衝突中的檔案」）。
#[tokio::test]
async fn diff_scenario_merge_conflict_index_to_worktree_is_unmerged_path() {
    let tmp = TempDir::new("diff-conflict-index-worktree");
    let repo = init_repo(&tmp, "repo");
    write_file(&repo, "c.txt", b"base\n");
    commit_all(&repo, "base");

    git_ok(&repo, &["checkout", "-b", "branch-a"]);
    write_file(&repo, "c.txt", b"branch-a change\n");
    commit_all(&repo, "branch-a change");

    git_ok(&repo, &["checkout", "main"]);
    git_ok(&repo, &["checkout", "-b", "branch-b"]);
    write_file(&repo, "c.txt", b"branch-b change\n");
    commit_all(&repo, "branch-b change");

    let merge = git_raw(&repo, &["merge", "branch-a", "--no-edit"]);
    assert!(!merge.status.success(), "這個合併應該要衝突");

    let fx = Fixture::new(&repo);
    let reply = fx.get("/diff?from=INDEX&to=WORKTREE&path=c.txt").await;

    assert_coded_error(&reply, StatusCode::CONFLICT, "unmerged_path");
}

// ===========================================================================
// 某版本的檔案內容端點（git-review task 3.3；spec「某版本的檔案內容端點」；design D6）
// ===========================================================================

/// Scenario: 讀取舊版本
#[tokio::test]
async fn rev_content_scenario_read_old_version() {
    let tmp = TempDir::new("rev-old-version");
    let repo = init_repo(&tmp, "repo");
    write_file(&repo, "docs/plan.md", b"old-version");
    let x = commit_all(&repo, "base");
    write_file(&repo, "docs/plan.md", b"new-version");
    let fx = Fixture::new(&repo);

    let reply = fx.get(&format!("/blob/{x}/docs/plan.md")).await;

    assert_eq!(reply.status, StatusCode::OK, "本體：{}", reply.raw);
    assert_eq!(reply.raw, "old-version");
}

/// Scenario: 該版本沒有這個檔案
#[tokio::test]
async fn rev_content_scenario_not_found_in_rev() {
    let tmp = TempDir::new("rev-not-found");
    let repo = init_repo(&tmp, "repo");
    write_file(&repo, "a.txt", b"1");
    let x = commit_all(&repo, "base");
    let fx = Fixture::new(&repo);

    let reply = fx.get(&format!("/meta/{x}/does-not-exist.md")).await;

    assert_coded_error(&reply, StatusCode::NOT_FOUND, "not_found_in_rev");
}

/// Scenario: HTML 仍隔離
#[tokio::test]
async fn rev_content_scenario_html_still_sandboxed() {
    let tmp = TempDir::new("rev-html-sandbox");
    let repo = init_repo(&tmp, "repo");
    write_file(&repo, "page.html", b"<p>hi</p>");
    let x = commit_all(&repo, "base");
    let fx = Fixture::new(&repo);

    let reply = fx.get(&format!("/blob/{x}/page.html")).await;

    assert_eq!(reply.status, StatusCode::OK, "本體：{}", reply.raw);
    assert_eq!(
        reply.header("content-security-policy"),
        Some("sandbox"),
        "應帶 Content-Security-Policy: sandbox"
    );
}

/// （額外）`meta` 回應含 `size`／`viewer`／`icon`／`blob`，且不同版本內容不同時 `blob` 值不同。
#[tokio::test]
async fn rev_content_scenario_meta_fields_and_blob_changes_with_content() {
    let tmp = TempDir::new("rev-meta-fields");
    let repo = init_repo(&tmp, "repo");
    write_file(&repo, "a.md", b"# one");
    let x1 = commit_all(&repo, "one");
    write_file(&repo, "a.md", b"# two, longer content");
    let x2 = commit_all(&repo, "two");
    let fx = Fixture::new(&repo);

    let r1 = fx.get(&format!("/meta/{x1}/a.md")).await;
    assert_eq!(r1.status, StatusCode::OK, "本體：{}", r1.raw);
    assert_eq!(r1.json["viewer"], "markdown");
    assert_eq!(r1.json["size"], 5);
    assert!(r1.json["icon"].as_str().is_some_and(|s| !s.is_empty()));
    let blob1 = r1.json["blob"].as_str().expect("blob 應為字串").to_string();
    // fix round 1（Ruling R8）：`blob` 必須是真正的 git 物件 hash，逐字等於直接跑
    // `git rev-parse <rev>:<path>` 的結果，不是自訂的內容雜湊。
    let expected1 = git_ok(&repo, &["rev-parse", &format!("{x1}:a.md")]);
    assert_eq!(blob1, expected1);

    let r2 = fx.get(&format!("/meta/{x2}/a.md")).await;
    assert_eq!(r2.status, StatusCode::OK, "本體：{}", r2.raw);
    let blob2 = r2.json["blob"].as_str().expect("blob 應為字串").to_string();
    assert_ne!(blob1, blob2, "不同版本的內容應有不同的 blob 值");
    let expected2 = git_ok(&repo, &["rev-parse", &format!("{x2}:a.md")]);
    assert_eq!(blob2, expected2);
}

/// fix round 1（Ruling R8）：`INDEX` 版本的 `meta` 也要能算出物件 hash，且與直接跑
/// `git rev-parse :<path>` 一致；修改暫存區內容後值跟著改變。
#[tokio::test]
async fn rev_content_scenario_meta_blob_for_index_matches_git_rev_parse_and_changes() {
    let tmp = TempDir::new("rev-meta-index-blob");
    let repo = init_repo(&tmp, "repo");
    write_file(&repo, "a.txt", b"staged one");
    git_ok(&repo, &["add", "a.txt"]);
    let fx = Fixture::new(&repo);

    let r1 = fx.get("/meta/INDEX/a.txt").await;
    assert_eq!(r1.status, StatusCode::OK, "本體：{}", r1.raw);
    let blob1 = r1.json["blob"].as_str().expect("blob 應為字串").to_string();
    let expected1 = git_ok(&repo, &["rev-parse", ":a.txt"]);
    assert_eq!(blob1, expected1);

    write_file(&repo, "a.txt", b"staged two, different length");
    git_ok(&repo, &["add", "a.txt"]);
    let r2 = fx.get("/meta/INDEX/a.txt").await;
    assert_eq!(r2.status, StatusCode::OK, "本體：{}", r2.raw);
    let blob2 = r2.json["blob"].as_str().expect("blob 應為字串").to_string();
    let expected2 = git_ok(&repo, &["rev-parse", ":a.txt"]);
    assert_eq!(blob2, expected2);
    assert_ne!(blob1, blob2, "修改暫存區內容後 blob 應改變");
}

/// fix round 2（Ruling R10）：沒有可辨識副檔名的二進位檔（前段含 NUL），`meta` 的 `viewer`
/// 必須與同樣內容的工作區檔案經 5a 中繼資料端點得到的結果一致——兩者都呼叫
/// `cockpit_files::classify_viewer_bytes`，不是各自一套規則。
#[tokio::test]
async fn rev_content_scenario_meta_viewer_matches_file_review_for_binary_without_extension() {
    let tmp = TempDir::new("rev-meta-viewer-binary");
    let repo = init_repo(&tmp, "repo");
    let mut content = vec![b'a'; 100];
    content.push(0); // NUL 落在前 8192 位元組內。
    content.extend_from_slice(b"more binary content after the nul byte");
    write_file(&repo, "noext-binary", &content);
    let x = commit_all(&repo, "add binary file without extension");
    let fx = Fixture::new(&repo);

    // 5a：同樣內容的工作區檔案，經檔案端點取得 viewer 當作標準答案。
    let files_reply = send(
        &fx.router,
        "GET",
        &format!("/api/files/{WIN}/{}/meta/noext-binary", fx.root_id),
    )
    .await;
    assert_eq!(
        files_reply.status,
        StatusCode::OK,
        "本體：{}",
        files_reply.raw
    );
    let expected_viewer = files_reply.json["viewer"].clone();
    assert_eq!(expected_viewer, "unsupported", "測試前提：應為 unsupported");

    // git-review：同一個 commit 版本的 meta。
    let git_reply = fx.get(&format!("/meta/{x}/noext-binary")).await;
    assert_eq!(git_reply.status, StatusCode::OK, "本體：{}", git_reply.raw);
    assert_eq!(git_reply.json["viewer"], expected_viewer);
}

/// fix round 2（Ruling R10）：沒有可辨識副檔名的純文字檔，`meta` 的 `viewer` 同樣要與 5a
/// 中繼資料端點一致。
#[tokio::test]
async fn rev_content_scenario_meta_viewer_matches_file_review_for_text_without_extension() {
    let tmp = TempDir::new("rev-meta-viewer-text");
    let repo = init_repo(&tmp, "repo");
    write_file(
        &repo,
        "noext-text",
        b"plain text content without a recognizable extension",
    );
    let x = commit_all(&repo, "add text file without extension");
    let fx = Fixture::new(&repo);

    let files_reply = send(
        &fx.router,
        "GET",
        &format!("/api/files/{WIN}/{}/meta/noext-text", fx.root_id),
    )
    .await;
    assert_eq!(
        files_reply.status,
        StatusCode::OK,
        "本體：{}",
        files_reply.raw
    );
    let expected_viewer = files_reply.json["viewer"].clone();
    assert_eq!(expected_viewer, "text", "測試前提：應為 text");

    let git_reply = fx.get(&format!("/meta/{x}/noext-text")).await;
    assert_eq!(git_reply.status, StatusCode::OK, "本體：{}", git_reply.raw);
    assert_eq!(git_reply.json["viewer"], expected_viewer);
}

/// （額外）`render` 端點在某個版本渲染 Markdown。
#[tokio::test]
async fn rev_content_scenario_render_markdown_at_rev() {
    let tmp = TempDir::new("rev-render-md");
    let repo = init_repo(&tmp, "repo");
    write_file(&repo, "doc.md", b"# Title\n\nhello");
    let x = commit_all(&repo, "base");
    let fx = Fixture::new(&repo);

    let reply = fx.get(&format!("/render/{x}/doc.md")).await;

    assert_eq!(reply.status, StatusCode::OK, "本體：{}", reply.raw);
    assert!(reply.raw.contains("<h1"), "應渲染出標題：{}", reply.raw);
}

/// （額外）非 Markdown 檔案請求 `render` → 415 `not_markdown`。
#[tokio::test]
async fn rev_content_scenario_render_non_markdown_is_not_markdown() {
    let tmp = TempDir::new("rev-render-not-md");
    let repo = init_repo(&tmp, "repo");
    write_file(&repo, "a.txt", b"hello");
    let x = commit_all(&repo, "base");
    let fx = Fixture::new(&repo);

    let reply = fx.get(&format!("/render/{x}/a.txt")).await;

    assert_coded_error(&reply, StatusCode::UNSUPPORTED_MEDIA_TYPE, "not_markdown");
}

/// （額外）`<rev>` 只接受 hash 或 `INDEX`；其他版本語法一律 400，且不啟動任何子程序。
#[tokio::test]
async fn rev_content_scenario_bad_rev_syntax_is_rejected() {
    let tmp = TempDir::new("rev-bad-syntax");
    let repo = init_repo(&tmp, "repo");
    write_file(&repo, "a.txt", b"1");
    commit_all(&repo, "base");
    let fx = Fixture::new(&repo);

    for rev in ["HEAD", "WORKTREE", "EMPTY", "main", &"a".repeat(39)] {
        let reply = fx.get(&format!("/meta/{rev}/a.txt")).await;
        assert_coded_error(&reply, StatusCode::BAD_REQUEST, "bad_request");
    }
    assert_eq!(fx.git_runner.spawned(), 0, "版本語法被拒不應啟動任何子程序");
}

/// （額外）不存在的 hash → 404 `rev_unknown`（同共同規則情境「不存在的 commit」）。
#[tokio::test]
async fn rev_content_scenario_unknown_hash_is_rev_unknown() {
    let tmp = TempDir::new("rev-unknown-hash");
    let repo = init_repo(&tmp, "repo");
    write_file(&repo, "a.txt", b"1");
    commit_all(&repo, "base");
    let fx = Fixture::new(&repo);

    let zeros = "0".repeat(40);
    let reply = fx.get(&format!("/blob/{zeros}/a.txt")).await;
    assert_coded_error(&reply, StatusCode::NOT_FOUND, "rev_unknown");
}

/// （額外）`INDEX` 讀的是暫存區內容，不是工作區（spec：「這些端點不讀取工作區」）。
#[tokio::test]
async fn rev_content_scenario_index_reads_staged_not_worktree() {
    let tmp = TempDir::new("rev-index-not-worktree");
    let repo = init_repo(&tmp, "repo");
    write_file(&repo, "a.txt", b"staged-content");
    git_ok(&repo, &["add", "a.txt"]);
    write_file(&repo, "a.txt", b"worktree-content-longer");
    let fx = Fixture::new(&repo);

    let reply = fx.get("/blob/INDEX/a.txt").await;

    assert_eq!(reply.status, StatusCode::OK, "本體：{}", reply.raw);
    assert_eq!(reply.raw, "staged-content");
}
