//! `cockpit-git` 對真實 git 的整合測試（git-review task 2.6）。
//!
//! 第一次端到端驗證「argv 組裝（task 2.1）＋執行器（task 2.2）＋解析器（task 2.3／2.4）」
//! 接起來是對的，並證明 design「git 讀取的安全邊界」與「讀取狀態不寫入 index」兩條唯讀保證。
//! 之前的 task 全部只用假程式或依 porcelain 格式手寫的位元組字面值測試，本檔案是第一次真的
//! 啟動 Windows git（`#[ignore]` 的 WSL 版見檔案末尾）。
//!
//! **找不到 git 時測試失敗而非略過**：`run_git`／`wsl_exec` 直接 `.expect()` spawn 的結果，
//! 找不到程式會直接 panic 讓測試失敗（design 風險欄「`cockpit-git` 的整合測試在找不到 git 時
//! 失敗而不是略過，避免 gate 假綠」）。
//!
//! **只在本測試自建的暫存 repo 內寫入**：每個測試用 [`TempRepo::new`] 在 `std::env::temp_dir()`
//! 下建立唯一名稱的目錄，`Drop` 時 `remove_dir_all` 清除；建 fixture（`init`／`commit`／
//! `mv`／`config` 等）一律用 `std::process::Command` 直接呼叫 git，**不經 `GitRunner`**
//! （`GitRunner` 只能執行 [`cockpit_git::GitQuery`] 這個封閉清單裡的查詢）。commit 用固定的
//! `GIT_AUTHOR_DATE`／`GIT_COMMITTER_DATE`（`2026-01-01T00:00:<seq>+00:00`）讓歷史可重現；
//! `core.autocrlf=false` 固定關閉，避免這台機器的全域設定讓工作區內容被轉換 CRLF，干擾
//! `FileDiff` 對行內容的斷言。
//!
//! 章節：Status／Refs／Log＋CommitInfo／ChangedFiles／FileDiff／MergeBase／VerifyCommit／
//! Blob（每個查詢至少跑一次真實 git，`?`／`multi-call` 查詢額外驗證合法非零結束）、
//! 安全性測試（外部程式不被執行＋對照組）、不寫 index 測試（＋對照組）、`#[ignore]` 的 WSL 版。

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use cockpit_git::{
    Blob, BlobHead, BlobId, BlobSize, ChangedFiles, CommitInfo, DiffRow, FileDiff, GitParseError,
    GitRunner, GitTarget, GraphCommit, Log, MergeBase, Oid, RefKind, Refs, RepoPath, RunnerError,
    Side, Status, StatusGroup, VerifyCommit, layout,
};

// ---------------------------------------------------------------------------
// 暫存 repo 與 fixture 小工具
// ---------------------------------------------------------------------------

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn unique_dir(label: &str) -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("系統時間應晚於 UNIX epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "cockpit-git-real-{label}-{}-{nanos}-{n}",
        std::process::id()
    ))
}

/// 每個查詢都帶的 fixture 用 `-c` 集合：身分（commit／tag／merge 需要）與關閉 gpg 簽章、
/// 固定關閉 `core.autocrlf`（避免這台機器的全域設定把工作區內容轉成 CRLF）。
const FIXTURE_CONFIG: &[&str] = &[
    "-c",
    "user.name=cockpit-git-test",
    "-c",
    "user.email=test@example.invalid",
    "-c",
    "commit.gpgsign=false",
    "-c",
    "core.autocrlf=false",
];

fn run_git(dir: &Path, args: &[&str]) -> std::process::Output {
    Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(FIXTURE_CONFIG)
        .args(args)
        .output()
        .expect("啟動 git 失敗——測試環境應已安裝 git；找不到就是環境問題，不是測試邏輯問題")
}

/// 需要固定 `GIT_AUTHOR_DATE`／`GIT_COMMITTER_DATE` 的呼叫（`commit`、`merge`）。
fn run_git_with_dates(dir: &Path, seq: u32, args: &[&str]) {
    assert!(seq < 60, "測試前提：seq 要能放進固定日期字面值的秒數欄位");
    let date = format!("2026-01-01T00:00:{seq:02}+00:00");
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(FIXTURE_CONFIG)
        .args(args)
        .env("GIT_AUTHOR_DATE", &date)
        .env("GIT_COMMITTER_DATE", &date)
        .output()
        .expect("啟動 git 失敗");
    assert!(
        out.status.success(),
        "git {args:?} 應成功，stderr：{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

struct TempRepo {
    dir: PathBuf,
}

impl TempRepo {
    fn new(label: &str) -> TempRepo {
        let dir = unique_dir(label);
        std::fs::create_dir_all(&dir).expect("建立暫存目錄失敗");
        TempRepo { dir }
    }

    fn path(&self) -> &Path {
        &self.dir
    }

    fn init(&self) {
        self.git_ok(&["init", "-q", "-b", "main"]);
    }

    fn git(&self, args: &[&str]) -> std::process::Output {
        run_git(&self.dir, args)
    }

    fn git_ok(&self, args: &[&str]) -> std::process::Output {
        let out = self.git(args);
        assert!(
            out.status.success(),
            "git {args:?} 應成功，stderr：{}",
            String::from_utf8_lossy(&out.stderr)
        );
        out
    }

    fn commit_at(&self, seq: u32, message: &str) {
        run_git_with_dates(&self.dir, seq, &["commit", "-q", "-m", message]);
    }

    fn merge_at(&self, seq: u32, branch: &str, message: &str) {
        run_git_with_dates(
            &self.dir,
            seq,
            &["merge", "--no-ff", "-q", "-m", message, branch],
        );
    }

    fn write_file(&self, rel: &str, content: &str) {
        self.write_file_bytes(rel, content.as_bytes());
    }

    fn write_file_bytes(&self, rel: &str, content: &[u8]) {
        let path = self.dir.join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("建立父目錄失敗");
        }
        std::fs::write(&path, content).expect("寫入 fixture 檔案失敗");
    }

    fn native_target(&self) -> GitTarget {
        GitTarget::Native {
            path: self.dir.to_string_lossy().into_owned(),
        }
    }

    fn head_oid(&self) -> Oid {
        self.rev_oid("HEAD")
    }

    fn rev_oid(&self, rev: &str) -> Oid {
        let out = self.git_ok(&["rev-parse", rev]);
        let text = String::from_utf8_lossy(&out.stdout);
        Oid::parse(text.trim()).expect("rev-parse 應回傳合法 oid")
    }
}

impl Drop for TempRepo {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn to_forward_slash(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// 寫一支「不管收到什麼引數都會 touch 標記檔」的 shell 腳本（`.sh`，`#!/bin/sh`）。
/// probe ①：Windows git 用內建 shell 執行設定值指定的外部程式，路徑必須用正斜線
/// （反斜線會被 shell 當跳脫序列吃掉），`.bat` 檔不行（`sh` 不認得）。
fn write_marker_script(script_path: &Path, marker_path: &Path) {
    let marker = to_forward_slash(marker_path);
    let body = format!("#!/bin/sh\ntouch \"{marker}\"\n");
    std::fs::write(script_path, body).expect("寫入標記腳本失敗");
}

// ---------------------------------------------------------------------------
// Status
// ---------------------------------------------------------------------------

#[tokio::test]
async fn status_reports_each_group_from_spec_scenario() {
    let repo = TempRepo::new("status-groups");
    repo.init();
    repo.write_file("a.rs", "fn a() {}\n");
    repo.write_file("b.rs", "fn b() {}\n");
    repo.write_file("c.rs", "fn c() {}\n");
    repo.write_file("old.txt", "old content\n");
    repo.git_ok(&["add", "."]);
    repo.commit_at(1, "initial");

    // a.rs：只有已暫存修改
    repo.write_file("a.rs", "fn a() { /* changed */ }\n");
    repo.git_ok(&["add", "a.rs"]);

    // b.rs：只有未暫存修改
    repo.write_file("b.rs", "fn b() { /* changed */ }\n");

    // c.rs：已暫存與未暫存修改各一筆
    repo.write_file("c.rs", "fn c() { /* staged */ }\n");
    repo.git_ok(&["add", "c.rs"]);
    repo.write_file("c.rs", "fn c() { /* staged */ /* and unstaged */ }\n");

    // new.md：未追蹤
    repo.write_file("new.md", "hello\n");

    // old.txt -> renamed.txt：已暫存改名
    repo.git_ok(&["mv", "old.txt", "renamed.txt"]);

    let runner = GitRunner::new();
    let target = repo.native_target();
    let run_output = runner.run(&Status, &target).await.expect("Status 應成功");
    let status = Status.parse(&run_output.calls).expect("Status 應能解析");

    assert_eq!(status.branch.head.as_deref(), Some("main"));
    assert!(status.branch.oid.is_some());

    let find = |path: &str, group: StatusGroup| {
        status
            .entries
            .iter()
            .find(|e| e.path == path && e.group == group)
    };

    assert_eq!(
        find("a.rs", StatusGroup::Staged)
            .expect("a.rs 應為 staged")
            .status,
        'M'
    );
    assert_eq!(
        find("b.rs", StatusGroup::Unstaged)
            .expect("b.rs 應為 unstaged")
            .status,
        'M'
    );
    assert_eq!(
        find("c.rs", StatusGroup::Staged)
            .expect("c.rs 應有 staged 一筆")
            .status,
        'M'
    );
    assert_eq!(
        find("c.rs", StatusGroup::Unstaged)
            .expect("c.rs 應有 unstaged 一筆")
            .status,
        'M'
    );
    assert_eq!(
        find("new.md", StatusGroup::Untracked)
            .expect("new.md 應為 untracked")
            .status,
        '?'
    );
    let renamed = find("renamed.txt", StatusGroup::Staged).expect("renamed.txt 應為 staged");
    assert_eq!(renamed.status, 'R');
    assert_eq!(renamed.old_path.as_deref(), Some("old.txt"));
}

#[tokio::test]
async fn status_reports_merge_conflict_scenario() {
    let repo = TempRepo::new("status-conflict");
    repo.init();
    repo.write_file("conflict.txt", "base\n");
    repo.git_ok(&["add", "."]);
    repo.commit_at(1, "base");

    repo.git_ok(&["checkout", "-b", "branch-a"]);
    repo.write_file("conflict.txt", "branch-a change\n");
    repo.git_ok(&["add", "."]);
    repo.commit_at(2, "branch-a change");

    repo.git_ok(&["checkout", "main"]);
    repo.git_ok(&["checkout", "-b", "branch-b"]);
    repo.write_file("conflict.txt", "branch-b change\n");
    repo.git_ok(&["add", "."]);
    repo.commit_at(3, "branch-b change");

    let merge_out = repo.git(&["merge", "branch-a"]);
    assert!(
        !merge_out.status.success(),
        "merge 應因內容衝突而失敗，這是本測試需要的前提"
    );

    let runner = GitRunner::new();
    let target = repo.native_target();
    let run_output = runner
        .run(&Status, &target)
        .await
        .expect("Status 應成功（即使 merge 進行中）");
    let status = Status.parse(&run_output.calls).expect("Status 應能解析");

    let conflict = status
        .entries
        .iter()
        .find(|e| e.path == "conflict.txt" && e.group == StatusGroup::Conflict)
        .expect("conflict.txt 應在 conflict 組");
    assert_eq!(conflict.status, 'U');
}

#[tokio::test]
async fn status_reports_no_commits_yet_scenario() {
    let repo = TempRepo::new("status-no-commit");
    repo.init();
    repo.write_file("a.txt", "hello\n");
    repo.git_ok(&["add", "a.txt"]);

    let runner = GitRunner::new();
    let run_output = runner
        .run(&Status, &repo.native_target())
        .await
        .expect("Status 應成功");
    let status = Status.parse(&run_output.calls).expect("Status 應能解析");

    assert_eq!(status.branch.oid, None);
    let a = status
        .entries
        .iter()
        .find(|e| e.path == "a.txt")
        .expect("a.txt 應存在");
    assert_eq!(a.group, StatusGroup::Staged);
    assert_eq!(a.status, 'A');
}

// ---------------------------------------------------------------------------
// Refs（含 task 2.2 遺留疑慮：多次呼叫查詢的合法非零結束）
// ---------------------------------------------------------------------------

#[tokio::test]
async fn refs_lists_branches_remote_and_annotated_tag_scenario() {
    let repo = TempRepo::new("refs-scenario");
    repo.init();
    repo.write_file("a.txt", "1\n");
    repo.git_ok(&["add", "."]);
    repo.commit_at(1, "c1");
    let c1 = repo.head_oid();

    repo.git_ok(&["branch", "feat/x"]);
    repo.git_ok(&["update-ref", "refs/remotes/origin/main", c1.as_str()]);
    repo.git_ok(&["tag", "-a", "v0.1", "-m", "release"]);
    // 一筆 stash：spec 要求 refs/stash 不應出現。
    repo.write_file("a.txt", "2\n");
    repo.git_ok(&["stash", "push", "-m", "wip"]);

    let runner = GitRunner::new();
    let run_output = runner
        .run(&Refs, &repo.native_target())
        .await
        .expect("Refs 應成功");
    let refs = Refs.parse(&run_output.calls).expect("Refs 應能解析");

    assert_eq!(refs.head.oid.as_deref(), Some(c1.as_str()));
    assert_eq!(refs.head.git_ref.as_deref(), Some("refs/heads/main"));

    let names: Vec<&str> = refs.refs.iter().map(|r| r.name.as_str()).collect();
    assert!(names.contains(&"refs/heads/main"));
    assert!(names.contains(&"refs/heads/feat/x"));
    assert!(names.contains(&"refs/remotes/origin/main"));
    assert!(names.contains(&"refs/tags/v0.1"));
    assert!(
        !names.iter().any(|n| n.starts_with("refs/stash")),
        "refs/stash 不應出現在清單裡，實際：{names:?}"
    );

    let main_branch = refs
        .refs
        .iter()
        .find(|r| r.name == "refs/heads/main")
        .unwrap();
    assert_eq!(main_branch.kind, RefKind::Branch);
    assert_eq!(main_branch.short, "main");

    let origin_main = refs
        .refs
        .iter()
        .find(|r| r.name == "refs/remotes/origin/main")
        .unwrap();
    assert_eq!(origin_main.kind, RefKind::Remote);
    assert_eq!(origin_main.oid, c1.as_str());

    let tag = refs
        .refs
        .iter()
        .find(|r| r.name == "refs/tags/v0.1")
        .unwrap();
    assert_eq!(tag.kind, RefKind::Tag);
    assert_eq!(tag.oid, c1.as_str(), "附註 tag 的 oid 應為剝皮後的 commit");
}

/// ui-fixes task 3.1（spec「refs 端點」Scenario「tag 指向非 commit 的物件」、design D4）：
/// 真實 git 對四種 tag 組合（輕量指 tree、輕量指 blob、附註指 tree、巢狀附註指 tree）與
/// 附註指 commit 的 `%(objecttype)`／`%(*objecttype)` 行為——tag 仍列出、`oid` 為剝開後的
/// 物件、`commit` 為 false；其餘 ref 為 true。
#[tokio::test]
async fn refs_marks_non_commit_tags_with_peeled_oid_and_commit_false() {
    let repo = TempRepo::new("refs-non-commit-tags");
    repo.init();
    repo.write_file(
        "a.txt", "1
",
    );
    repo.git_ok(&["add", "."]);
    repo.commit_at(1, "c1");
    let c1 = repo.head_oid();

    let rev = |spec: &str| {
        String::from_utf8_lossy(&repo.git_ok(&["rev-parse", spec]).stdout)
            .trim()
            .to_string()
    };
    let tree = rev("HEAD^{tree}");
    let blob = rev("HEAD:a.txt");
    repo.git_ok(&["tag", "tree-tag", tree.as_str()]);
    repo.git_ok(&["tag", "blob-tag", blob.as_str()]);
    // 附註 tag 指向 tree，再以另一個附註 tag 指向它（巢狀）。
    repo.git_ok(&["tag", "-a", "ann-tree-tag", "-m", "t", tree.as_str()]);
    repo.git_ok(&["tag", "-a", "nested-tag", "-m", "n", "ann-tree-tag"]);
    repo.git_ok(&["tag", "-a", "ann-commit-tag", "-m", "c", "HEAD"]);
    // 附註 tag 指向附註 tag 再指向 commit（巢狀 → commit）。
    repo.git_ok(&[
        "tag",
        "-a",
        "nested-commit-tag",
        "-m",
        "nc",
        "ann-commit-tag",
    ]);

    let runner = GitRunner::new();
    let run_output = runner
        .run(&Refs, &repo.native_target())
        .await
        .expect("Refs 應成功");
    let refs = Refs.parse(&run_output.calls).expect("Refs 應能解析");

    let find = |name: &str| {
        refs.refs
            .iter()
            .find(|r| r.name == name)
            .unwrap_or_else(|| panic!("找不到 {name}：{:?}", refs.refs))
    };
    let main = find("refs/heads/main");
    assert!(main.commit);
    assert_eq!(main.oid, c1.as_str());
    for (name, expected_oid) in [
        ("refs/tags/tree-tag", tree.as_str()),
        ("refs/tags/blob-tag", blob.as_str()),
        ("refs/tags/ann-tree-tag", tree.as_str()),
    ] {
        let entry = find(name);
        assert_eq!(entry.kind, RefKind::Tag, "{name}");
        assert!(!entry.commit, "{name} 剝開後不是 commit");
        assert_eq!(entry.oid, expected_oid, "{name} 的 oid 應為剝開後的物件");
    }
    let ann_commit = find("refs/tags/ann-commit-tag");
    assert!(ann_commit.commit);
    assert_eq!(ann_commit.oid, c1.as_str());
    // 巢狀附註 tag：`%(*objecttype)` 在 git 2.50 剝到底、在 2.43 只剝一層（修正波 1 B-I1），
    // 所以斷言只鎖定與 git 版本無關的部分——巢狀 → commit 必為 true；巢狀 → tree 在新版為
    // false、舊版因無法確定而為 true，這裡不斷言其 `commit`（`kind` 仍必為 Tag，tag 必須列出）。
    let nested_commit = find("refs/tags/nested-commit-tag");
    assert!(
        nested_commit.commit,
        "巢狀 tag → commit 在任何 git 版本都必為 true"
    );
    let nested_tree = find("refs/tags/nested-tag");
    assert_eq!(nested_tree.kind, RefKind::Tag);
}

/// task 2.2 遺留疑慮：`rev-parse --verify -q HEAD` 在懸空分支上合法地以非零結束——
/// `Refs::parse` 必須把它解讀成「沒有值」而不是讓整個查詢失敗。
#[tokio::test]
async fn refs_on_repo_without_commits_is_not_an_error() {
    let repo = TempRepo::new("refs-no-commit");
    repo.init();

    let runner = GitRunner::new();
    let run_output = runner
        .run(&Refs, &repo.native_target())
        .await
        .expect("Refs 應成功（即使沒有任何 commit）");
    let refs = Refs.parse(&run_output.calls).expect("Refs 應能解析");

    assert_eq!(
        refs.head.oid, None,
        "沒有 commit 時 rev-parse HEAD 應合法地沒有值"
    );
    assert_eq!(
        refs.head.git_ref.as_deref(),
        Some("refs/heads/main"),
        "懸空分支的 symbolic-ref 仍應成功回傳"
    );
    assert!(refs.refs.is_empty(), "還沒有任何 commit 時不應有任何分支");
}

/// task 2.2 遺留疑慮：分離 HEAD 時 `symbolic-ref -q HEAD` 合法地以非零結束。
#[tokio::test]
async fn refs_on_detached_head_reports_oid_without_symbolic_ref() {
    let repo = TempRepo::new("refs-detached");
    repo.init();
    repo.write_file("a.txt", "1\n");
    repo.git_ok(&["add", "."]);
    repo.commit_at(1, "c1");
    let c1 = repo.head_oid();
    repo.git_ok(&["checkout", c1.as_str()]);

    let runner = GitRunner::new();
    let run_output = runner
        .run(&Refs, &repo.native_target())
        .await
        .expect("Refs 應成功（即使處於分離 HEAD）");
    let refs = Refs.parse(&run_output.calls).expect("Refs 應能解析");

    assert_eq!(refs.head.oid.as_deref(), Some(c1.as_str()));
    assert_eq!(
        refs.head.git_ref, None,
        "分離 HEAD 時 symbolic-ref 應合法地沒有值"
    );
}

// ---------------------------------------------------------------------------
// Log／CommitInfo（含與 graph::layout 的端到端串接）
// ---------------------------------------------------------------------------

#[tokio::test]
async fn log_and_commit_info_report_real_commits() {
    let repo = TempRepo::new("log-commit-info");
    repo.init();
    repo.write_file("a.txt", "1\n");
    repo.git_ok(&["add", "."]);
    repo.commit_at(1, "first commit");
    let c1 = repo.head_oid();

    repo.write_file("a.txt", "2\n");
    repo.git_ok(&["add", "."]);
    repo.commit_at(2, "second commit\n\nwith a body line");
    let c2 = repo.head_oid();

    let runner = GitRunner::new();
    let target = repo.native_target();

    let log_query = Log {
        tips: vec![c2.clone()],
        limit: 200,
    };
    let run_output = runner.run(&log_query, &target).await.expect("Log 應成功");
    let log = log_query.parse(&run_output.calls).expect("Log 應能解析");

    assert_eq!(log.rows.len(), 2);
    assert_eq!(log.rows[0].oid, c2.as_str());
    assert_eq!(log.rows[0].parents, vec![c1.as_str().to_string()]);
    assert_eq!(log.rows[0].subject, "second commit");
    assert_eq!(log.rows[1].oid, c1.as_str());
    assert!(log.rows[1].parents.is_empty());

    let commit_info_query = CommitInfo { oid: c2.clone() };
    let run_output = runner
        .run(&commit_info_query, &target)
        .await
        .expect("CommitInfo 應成功");
    let info = commit_info_query
        .parse(&run_output.calls)
        .expect("CommitInfo 應能解析");
    assert_eq!(info.oid, c2.as_str());
    assert_eq!(info.parents, vec![c1.as_str().to_string()]);
    assert!(info.message.contains("second commit"));
    assert!(info.message.contains("with a body line"));
    assert_eq!(info.author.name, "cockpit-git-test");
    assert_eq!(info.author.email, "test@example.invalid");
}

#[tokio::test]
async fn log_output_feeds_directly_into_graph_layout() {
    let repo = TempRepo::new("log-graph");
    repo.init();
    repo.write_file("f.txt", "a\n");
    repo.git_ok(&["add", "."]);
    repo.commit_at(1, "A");

    repo.write_file("f.txt", "b\n");
    repo.git_ok(&["add", "."]);
    repo.commit_at(2, "B");

    repo.git_ok(&["checkout", "-b", "feat"]);
    repo.write_file("f.txt", "c\n");
    repo.git_ok(&["add", "."]);
    repo.commit_at(3, "C");

    repo.git_ok(&["checkout", "main"]);
    repo.write_file("g.txt", "d\n");
    repo.git_ok(&["add", "."]);
    repo.commit_at(4, "D");

    repo.merge_at(5, "feat", "E merge");
    let head = repo.head_oid();

    let runner = GitRunner::new();
    let target = repo.native_target();
    let log_query = Log {
        tips: vec![head],
        limit: 200,
    };
    let run_output = runner.run(&log_query, &target).await.expect("Log 應成功");
    let log = log_query.parse(&run_output.calls).expect("Log 應能解析");

    assert_eq!(log.rows.len(), 5, "應有 A B C D E 共 5 個 commit");
    assert_eq!(log.rows[0].parents.len(), 2, "E 應有兩個 parent（D 與 C）");

    let commits: Vec<GraphCommit<'_>> = log
        .rows
        .iter()
        .map(|r| GraphCommit {
            oid: &r.oid,
            parents: &r.parents,
        })
        .collect();
    let graph_rows = layout(&commits);
    assert_eq!(graph_rows.len(), 5, "graph 排版的列數應與 commit 數相同");
}

// ---------------------------------------------------------------------------
// ChangedFiles（含 task 2.3 遺留疑慮：改名紀錄的 numstat 增刪行數）
// ---------------------------------------------------------------------------

#[tokio::test]
async fn changed_files_reports_full_spec_scenario_between_two_commits() {
    let repo = TempRepo::new("changed-files-scenario");
    repo.init();
    repo.write_file("m.rs", "line1\nline2\nline3\n");
    repo.write_file("d.txt", "to be deleted\n");
    repo.write_file("o.md", "line a\nline b\nline c\nline d\nline e\n");
    repo.write_file_bytes("img.png", &[0u8, 1, 2, 3, 4, 5, 6, 7, 8, 9]);
    repo.git_ok(&["add", "."]);
    repo.commit_at(1, "base");
    let base = repo.head_oid();

    repo.write_file("n.md", "new file\n");
    repo.write_file("m.rs", "line1\nline2 changed\nline3\nline4\n");
    std::fs::remove_file(repo.path().join("d.txt")).expect("刪除 fixture 檔案失敗");
    repo.git_ok(&["mv", "o.md", "p.md"]);
    repo.write_file_bytes("img.png", &[9u8, 8, 7, 6, 5, 4, 3, 2, 1, 0]);
    repo.git_ok(&["add", "-A"]);
    repo.commit_at(2, "changes");
    let head = repo.head_oid();

    let runner = GitRunner::new();
    let target = repo.native_target();
    let query = ChangedFiles::new(Side::Oid(base), Side::Oid(head)).expect("應為合法組合");
    let run_output = runner
        .run(&query, &target)
        .await
        .expect("ChangedFiles 應成功");
    let result = query
        .parse(&run_output.calls)
        .expect("ChangedFiles 應能解析");

    let find = |path: &str| result.files.iter().find(|f| f.path == path);

    assert_eq!(find("n.md").expect("n.md 應存在").status, 'A');

    let m = find("m.rs").expect("m.rs 應存在");
    assert_eq!(m.status, 'M');
    assert_eq!(m.additions, Some(2));
    assert_eq!(m.deletions, Some(1));

    assert_eq!(find("d.txt").expect("d.txt 應存在（已刪除）").status, 'D');

    let p = find("p.md").expect("p.md（改名後）應存在");
    assert_eq!(p.status, 'R');
    assert_eq!(p.old_path.as_deref(), Some("o.md"));

    let img = find("img.png").expect("img.png 應存在");
    assert_eq!(img.status, 'M');
    assert_eq!(img.additions, None, "二進位檔的增刪行數應為 None");
    assert_eq!(img.deletions, None);
}

/// task 2.3 遺留疑慮（決定 4）：`--numstat -z` 對改名紀錄的格式是推導出來的，未經 task 1.2
/// probe 實測——用真實改名＋小修改驗證 additions／deletions 不是 null。
#[tokio::test]
async fn changed_files_rename_with_edit_has_non_null_addition_and_deletion_counts() {
    let repo = TempRepo::new("changed-files-rename-edit");
    repo.init();
    repo.write_file("old.txt", "line1\nline2\nline3\nline4\nline5\n");
    repo.git_ok(&["add", "."]);
    repo.commit_at(1, "base");
    let base = repo.head_oid();

    repo.git_ok(&["mv", "old.txt", "renamed.txt"]);
    repo.write_file(
        "renamed.txt",
        "line1\nline2 edited\nline3\nline4\nline5\nline6\n",
    );
    repo.git_ok(&["add", "-A"]);
    repo.commit_at(2, "rename and edit");
    let head = repo.head_oid();

    let runner = GitRunner::new();
    let target = repo.native_target();
    let query = ChangedFiles::new(Side::Oid(base), Side::Oid(head)).expect("應為合法組合");
    let run_output = runner
        .run(&query, &target)
        .await
        .expect("ChangedFiles 應成功");
    let result = query
        .parse(&run_output.calls)
        .expect("ChangedFiles 應能解析");

    assert_eq!(result.files.len(), 1, "應只有一筆改名紀錄");
    let f = &result.files[0];
    assert_eq!(f.status, 'R');
    assert_eq!(f.path, "renamed.txt");
    assert_eq!(f.old_path.as_deref(), Some("old.txt"));
    assert_eq!(
        f.additions,
        Some(2),
        "改名並修改內容後，additions 不應為 None"
    );
    assert_eq!(
        f.deletions,
        Some(1),
        "改名並修改內容後，deletions 不應為 None"
    );
}

// ---------------------------------------------------------------------------
// FileDiff（含 task 2.4 遺留疑慮：每種兩側組合在真實 repo 上的輸出）
// ---------------------------------------------------------------------------

#[tokio::test]
async fn file_diff_index_to_worktree_reports_unstaged_change() {
    let repo = TempRepo::new("file-diff-unstaged");
    repo.init();
    repo.write_file("a.rs", "line1\nline2\nline3\n");
    repo.git_ok(&["add", "."]);
    repo.commit_at(1, "base");
    repo.write_file("a.rs", "line1\nline2 changed\nline3\n");

    let runner = GitRunner::new();
    let target = repo.native_target();
    let query = FileDiff::new(
        Side::Index,
        Side::Worktree,
        RepoPath::parse("a.rs").unwrap(),
        None,
    )
    .expect("應為合法組合");
    let run_output = runner.run(&query, &target).await.expect("FileDiff 應成功");
    let diff = query.parse(&run_output.calls).expect("FileDiff 應能解析");

    assert!(!diff.binary);
    let (left, right) = diff
        .rows
        .iter()
        .find_map(|r| match r {
            DiffRow::Change { left, right } if left.line == 2 => Some((left, right)),
            _ => None,
        })
        .expect("第 2 行應為 change");
    assert_eq!(left.text, "line2");
    assert_eq!(right.text, "line2 changed");
}

#[tokio::test]
async fn file_diff_oid_to_index_reports_staged_change() {
    let repo = TempRepo::new("file-diff-staged");
    repo.init();
    repo.write_file("a.rs", "line1\n");
    repo.git_ok(&["add", "."]);
    repo.commit_at(1, "base");
    let base = repo.head_oid();
    repo.write_file("a.rs", "line1\nline2\n");
    repo.git_ok(&["add", "."]);

    let runner = GitRunner::new();
    let target = repo.native_target();
    let query = FileDiff::new(
        Side::Oid(base),
        Side::Index,
        RepoPath::parse("a.rs").unwrap(),
        None,
    )
    .expect("應為合法組合");
    let run_output = runner.run(&query, &target).await.expect("FileDiff 應成功");
    let diff = query.parse(&run_output.calls).expect("FileDiff 應能解析");

    let right = diff
        .rows
        .iter()
        .find_map(|r| match r {
            DiffRow::Add { right } if right.line == 2 => Some(right),
            _ => None,
        })
        .expect("應有新增的第 2 行");
    assert_eq!(right.text, "line2");
}

#[tokio::test]
async fn file_diff_oid_to_oid_reports_change_between_two_commits() {
    let repo = TempRepo::new("file-diff-oid-oid");
    repo.init();
    repo.write_file("a.rs", "one\ntwo\n");
    repo.git_ok(&["add", "."]);
    repo.commit_at(1, "c1");
    let c1 = repo.head_oid();
    repo.write_file("a.rs", "one\ntwo\nthree\n");
    repo.git_ok(&["add", "."]);
    repo.commit_at(2, "c2");
    let c2 = repo.head_oid();

    let runner = GitRunner::new();
    let target = repo.native_target();
    let query = FileDiff::new(
        Side::Oid(c1),
        Side::Oid(c2),
        RepoPath::parse("a.rs").unwrap(),
        None,
    )
    .expect("應為合法組合");
    let run_output = runner.run(&query, &target).await.expect("FileDiff 應成功");
    let diff = query.parse(&run_output.calls).expect("FileDiff 應能解析");

    let right = diff
        .rows
        .iter()
        .find_map(|r| match r {
            DiffRow::Add { right } if right.line == 3 => Some(right),
            _ => None,
        })
        .expect("第 3 行應為新增");
    assert_eq!(right.text, "three");
}

#[tokio::test]
async fn file_diff_empty_to_oid_reports_root_commit_as_all_additions() {
    let repo = TempRepo::new("file-diff-root-commit");
    repo.init();
    repo.write_file("a.rs", "line1\nline2\n");
    repo.git_ok(&["add", "."]);
    repo.commit_at(1, "root");
    let root = repo.head_oid();

    let runner = GitRunner::new();
    let target = repo.native_target();
    let query = FileDiff::new(
        Side::Empty,
        Side::Oid(root),
        RepoPath::parse("a.rs").unwrap(),
        None,
    )
    .expect("應為合法組合（根 commit）");
    let run_output = runner.run(&query, &target).await.expect("FileDiff 應成功");
    let diff = query.parse(&run_output.calls).expect("FileDiff 應能解析");

    assert_eq!(diff.rows.len(), 2);
    assert!(diff.rows.iter().all(|r| matches!(r, DiffRow::Add { .. })));
}

#[tokio::test]
async fn file_diff_empty_to_index_reports_staged_file_in_repo_without_commits() {
    let repo = TempRepo::new("file-diff-empty-index");
    repo.init();
    repo.write_file("a.rs", "line1\n");
    repo.git_ok(&["add", "."]);

    let runner = GitRunner::new();
    let target = repo.native_target();
    let query = FileDiff::new(
        Side::Empty,
        Side::Index,
        RepoPath::parse("a.rs").unwrap(),
        None,
    )
    .expect("應為合法組合（無 commit 的 repo）");
    let run_output = runner.run(&query, &target).await.expect("FileDiff 應成功");
    let diff = query.parse(&run_output.calls).expect("FileDiff 應能解析");

    assert_eq!(diff.rows.len(), 1);
    assert!(matches!(
        &diff.rows[0],
        DiffRow::Add { right } if right.line == 1 && right.text == "line1"
    ));
}

#[tokio::test]
async fn file_diff_rename_with_content_change_reports_change_row() {
    // 用足夠長的內容讓 git 的 -M 相似度偵測真的判定為改名（而不是落回「新增一個檔案＋
    // 刪除另一個檔案」的兩個獨立區塊——task 2.4 報告「新發現②」記錄的低相似度分岔行為）：
    // 用 2 行內容做過這個測試時，1/2 行改動的相似度不足以被判定為 rename，git 會印成兩個
    // 獨立的 `diff --git` 區塊（各自一整塊 Delete／Add，沒有跨區塊配對的 Change 列）——這是
    // 本 task 第一次跑真實 git 才發現的 RED：改用 5 行、只改其中 1 行，相似度足夠觸發真正的
    // rename 偵測（已用 `git diff` 手動驗證印出 `rename from/to` 而非兩個獨立區塊）。
    let repo = TempRepo::new("file-diff-rename-edit");
    repo.init();
    repo.write_file("old.rs", "line1\nline2\nline3\nline4\nline5\n");
    repo.git_ok(&["add", "."]);
    repo.commit_at(1, "base");
    let base = repo.head_oid();
    repo.git_ok(&["mv", "old.rs", "new.rs"]);
    repo.write_file("new.rs", "line1\nline2 changed\nline3\nline4\nline5\n");
    repo.git_ok(&["add", "-A"]);
    repo.commit_at(2, "rename and edit");
    let head = repo.head_oid();

    let runner = GitRunner::new();
    let target = repo.native_target();
    let query = FileDiff::new(
        Side::Oid(base),
        Side::Oid(head),
        RepoPath::parse("new.rs").unwrap(),
        Some(RepoPath::parse("old.rs").unwrap()),
    )
    .expect("應為合法組合");
    let run_output = runner.run(&query, &target).await.expect("FileDiff 應成功");
    let diff = query.parse(&run_output.calls).expect("FileDiff 應能解析");

    assert!(!diff.binary);
    assert!(!diff.submodule);
    let (left, right) = diff
        .rows
        .iter()
        .find_map(|r| match r {
            DiffRow::Change { left, right } => Some((left, right)),
            _ => None,
        })
        .expect("應有一列 change");
    assert_eq!(left.text, "line2");
    assert_eq!(right.text, "line2 changed");
}

/// 驗證 task 2.4 報告「決定 4」：相似度低於 `-M` 門檻時，git 印出兩個獨立 `diff --git`
/// 區塊（各自一整段 Delete／Add），不是一個 rename 區塊——本 task 才第一次用真實 git
/// 驗證這條路徑（`file_diff_rename_with_content_change_reports_change_row` 一開始用的
/// 2 行小檔案就是意外踩到這個分支，見上方測試的註解）。
#[tokio::test]
async fn file_diff_with_low_similarity_rename_concatenates_two_blocks_without_error() {
    let repo = TempRepo::new("file-diff-low-similarity-rename");
    repo.init();
    repo.write_file("old.rs", "line1\nline2\n");
    repo.git_ok(&["add", "."]);
    repo.commit_at(1, "base");
    let base = repo.head_oid();
    repo.git_ok(&["mv", "old.rs", "new.rs"]);
    repo.write_file("new.rs", "line1\nline2 changed\n");
    repo.git_ok(&["add", "-A"]);
    repo.commit_at(2, "low similarity rename");
    let head = repo.head_oid();

    let runner = GitRunner::new();
    let target = repo.native_target();
    let query = FileDiff::new(
        Side::Oid(base),
        Side::Oid(head),
        RepoPath::parse("new.rs").unwrap(),
        Some(RepoPath::parse("old.rs").unwrap()),
    )
    .expect("應為合法組合");
    let run_output = runner.run(&query, &target).await.expect("FileDiff 應成功");
    let diff = query
        .parse(&run_output.calls)
        .expect("FileDiff 應能解析（不應因兩個獨立區塊而報錯）");

    assert!(!diff.binary);
    let deletes: Vec<_> = diff
        .rows
        .iter()
        .filter(|r| matches!(r, DiffRow::Delete { .. }))
        .collect();
    let adds: Vec<_> = diff
        .rows
        .iter()
        .filter(|r| matches!(r, DiffRow::Add { .. }))
        .collect();
    assert_eq!(deletes.len(), 2, "old.rs 的兩行應各自成為一筆 Delete");
    assert_eq!(adds.len(), 2, "new.rs 的兩行應各自成為一筆 Add");
    assert!(
        !diff
            .rows
            .iter()
            .any(|r| matches!(r, DiffRow::Change { .. })),
        "兩個獨立區塊之間不應出現跨區塊配對的 Change 列"
    );
}

/// 目視驗收缺陷 V1、Ruling R11：合併衝突中的檔案，`from=<HEAD 的 hash>&to=WORKTREE` 應回
/// 一般 unified diff（`FileDiff::new` 的 `Oid→Worktree` 組合），衝突標記（`<<<<<<<`／
/// `=======`／`>>>>>>>`）以 `add` 列呈現——spec「單檔 diff 端點」Scenario「合併衝突中的檔案」
/// 的前半段。
#[tokio::test]
async fn file_diff_oid_to_worktree_reports_conflict_markers_as_add_rows() {
    let repo = TempRepo::new("file-diff-conflict-oid-worktree");
    repo.init();
    repo.write_file("c.txt", "base\n");
    repo.git_ok(&["add", "."]);
    repo.commit_at(1, "base");

    repo.git_ok(&["checkout", "-b", "branch-a"]);
    repo.write_file("c.txt", "branch-a change\n");
    repo.git_ok(&["add", "."]);
    repo.commit_at(2, "branch-a change");

    repo.git_ok(&["checkout", "main"]);
    repo.git_ok(&["checkout", "-b", "branch-b"]);
    repo.write_file("c.txt", "branch-b change\n");
    repo.git_ok(&["add", "."]);
    repo.commit_at(3, "branch-b change");
    let head = repo.head_oid();

    let merge_out = repo.git(&["merge", "branch-a"]);
    assert!(
        !merge_out.status.success(),
        "merge 應因內容衝突而失敗，這是本測試需要的前提"
    );

    let runner = GitRunner::new();
    let target = repo.native_target();
    let query = FileDiff::new(
        Side::Oid(head),
        Side::Worktree,
        RepoPath::parse("c.txt").unwrap(),
        None,
    )
    .expect("Oid → Worktree 應為合法組合（Ruling R11）");
    let run_output = runner.run(&query, &target).await.expect("FileDiff 應成功");
    let diff = query
        .parse(&run_output.calls)
        .expect("HEAD→WORKTREE 對衝突檔案應是一般 unified diff，能正常解析");

    assert!(!diff.binary);
    let add_texts: Vec<&str> = diff
        .rows
        .iter()
        .filter_map(|r| match r {
            DiffRow::Add { right } => Some(right.text.as_str()),
            _ => None,
        })
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

/// 同一個衝突情境，`from=INDEX&to=WORKTREE` 應回 `GitParseError::Unmerged`（真實 git 2.5x
/// 對未合併檔案輸出的 `diff --cc` 三方格式）——spec「單檔 diff 端點」Scenario「合併衝突中的
/// 檔案」的後半段。
#[tokio::test]
async fn file_diff_index_to_worktree_on_conflicted_file_is_unmerged() {
    let repo = TempRepo::new("file-diff-conflict-index-worktree");
    repo.init();
    repo.write_file("c.txt", "base\n");
    repo.git_ok(&["add", "."]);
    repo.commit_at(1, "base");

    repo.git_ok(&["checkout", "-b", "branch-a"]);
    repo.write_file("c.txt", "branch-a change\n");
    repo.git_ok(&["add", "."]);
    repo.commit_at(2, "branch-a change");

    repo.git_ok(&["checkout", "main"]);
    repo.git_ok(&["checkout", "-b", "branch-b"]);
    repo.write_file("c.txt", "branch-b change\n");
    repo.git_ok(&["add", "."]);
    repo.commit_at(3, "branch-b change");

    let merge_out = repo.git(&["merge", "branch-a"]);
    assert!(
        !merge_out.status.success(),
        "merge 應因內容衝突而失敗，這是本測試需要的前提"
    );

    let runner = GitRunner::new();
    let target = repo.native_target();
    let query = FileDiff::new(
        Side::Index,
        Side::Worktree,
        RepoPath::parse("c.txt").unwrap(),
        None,
    )
    .expect("應為合法組合");
    let run_output = runner
        .run(&query, &target)
        .await
        .expect("FileDiff 應成功結束（exit 0）");
    let err = query
        .parse(&run_output.calls)
        .expect_err("未合併檔案的 INDEX→WORKTREE 應回 Unmerged，不是成功解析");
    assert_eq!(err, GitParseError::Unmerged);
}

// ---------------------------------------------------------------------------
// MergeBase
// ---------------------------------------------------------------------------

#[tokio::test]
async fn merge_base_finds_common_ancestor() {
    let repo = TempRepo::new("merge-base-found");
    repo.init();
    repo.write_file("a.txt", "1\n");
    repo.git_ok(&["add", "."]);
    repo.commit_at(1, "base");
    let base = repo.head_oid();

    repo.git_ok(&["checkout", "-b", "feat"]);
    repo.write_file("a.txt", "2\n");
    repo.git_ok(&["add", "."]);
    repo.commit_at(2, "feat commit");
    let feat = repo.head_oid();

    repo.git_ok(&["checkout", "main"]);
    repo.write_file("b.txt", "1\n");
    repo.git_ok(&["add", "."]);
    repo.commit_at(3, "main commit");
    let main = repo.head_oid();

    let runner = GitRunner::new();
    let query = MergeBase { a: main, b: feat };
    let run_output = runner
        .run(&query, &repo.native_target())
        .await
        .expect("MergeBase 應成功");
    let stdout = run_output.calls[0]
        .as_ref()
        .expect("找得到共同祖先時應成功")
        .stdout
        .clone();
    let text = String::from_utf8(stdout).expect("stdout 應為合法 UTF-8");
    assert_eq!(text.trim(), base.as_str());
}

/// 用兩個各自獨立、彼此沒有共同祖先的歷史（透過 `fetch` 併入同一個 repo，不用任何
/// `reset --hard`／`rm -rf` 之類的破壞性指令建 fixture）驗證「無共同祖先」的合法非零結束。
#[tokio::test]
async fn merge_base_with_no_common_ancestor_is_legitimate_nonzero_exit() {
    let repo_a = TempRepo::new("merge-base-none-a");
    repo_a.init();
    repo_a.write_file("a.txt", "1\n");
    repo_a.git_ok(&["add", "."]);
    repo_a.commit_at(1, "root a");
    let a = repo_a.head_oid();

    let repo_b = TempRepo::new("merge-base-none-b");
    repo_b.init();
    repo_b.write_file("b.txt", "1\n");
    repo_b.git_ok(&["add", "."]);
    repo_b.commit_at(1, "root b");
    let b = repo_b.head_oid();

    let repo_b_path = repo_b.path().to_string_lossy().into_owned();
    repo_a.git_ok(&["fetch", &repo_b_path, "main:refs/heads/unrelated"]);

    let runner = GitRunner::new();
    let query = MergeBase { a, b };
    let run_output = runner
        .run(&query, &repo_a.native_target())
        .await
        .expect("execute 層級不應失敗（無共同祖先是查詢層級的合法非零結束）");
    match &run_output.calls[0] {
        Err(RunnerError::Failed { exit_code, .. }) => assert_eq!(*exit_code, Some(1)),
        other => panic!("應為 Failed(exit_code=1)，實際：{other:?}"),
    }
}

// ---------------------------------------------------------------------------
// VerifyCommit
// ---------------------------------------------------------------------------

#[tokio::test]
async fn verify_commit_accepts_a_real_commit() {
    let repo = TempRepo::new("verify-commit-ok");
    repo.init();
    repo.write_file("a.txt", "1\n");
    repo.git_ok(&["add", "."]);
    repo.commit_at(1, "c1");
    let c1 = repo.head_oid();

    let runner = GitRunner::new();
    let query = VerifyCommit { oid: c1.clone() };
    let run_output = runner
        .run(&query, &repo.native_target())
        .await
        .expect("VerifyCommit 應成功");
    let outcome = run_output.calls[0].as_ref().expect("應成功");
    assert_eq!(String::from_utf8_lossy(&outcome.stdout).trim(), c1.as_str());
}

#[tokio::test]
async fn verify_commit_rejects_unknown_oid_as_legitimate_nonzero_exit() {
    let repo = TempRepo::new("verify-commit-unknown");
    repo.init();
    repo.write_file("a.txt", "1\n");
    repo.git_ok(&["add", "."]);
    repo.commit_at(1, "c1");

    let fake = Oid::parse(&"0".repeat(40)).expect("40 個 0 應為合法 Oid 語法");
    let runner = GitRunner::new();
    let query = VerifyCommit { oid: fake };
    let run_output = runner
        .run(&query, &repo.native_target())
        .await
        .expect("execute 層級不應失敗");
    match &run_output.calls[0] {
        Err(RunnerError::Failed { .. }) => {}
        other => panic!("應為 Failed，實際：{other:?}"),
    }
}

// ---------------------------------------------------------------------------
// BlobSize／Blob
// ---------------------------------------------------------------------------

#[tokio::test]
async fn blob_reads_file_content_at_a_given_commit() {
    let repo = TempRepo::new("blob-read");
    repo.init();
    repo.write_file("a.txt", "hello world\n");
    repo.git_ok(&["add", "."]);
    repo.commit_at(1, "c1");
    let c1 = repo.head_oid();

    let runner = GitRunner::new();
    let target = repo.native_target();
    let path = RepoPath::parse("a.txt").unwrap();

    let size_query = BlobSize::new(Side::Oid(c1.clone()), path.clone()).unwrap();
    let size_output = runner
        .run(&size_query, &target)
        .await
        .expect("BlobSize 應成功");
    let size_text = String::from_utf8_lossy(&size_output.calls[0].as_ref().unwrap().stdout)
        .trim()
        .to_string();
    assert_eq!(size_text, "hello world\n".len().to_string());

    let blob_query = Blob::new(Side::Oid(c1), path).unwrap();
    let blob_output = runner.run(&blob_query, &target).await.expect("Blob 應成功");
    let content = blob_output.calls[0].as_ref().unwrap().stdout.clone();
    assert_eq!(content, b"hello world\n");
}

#[tokio::test]
async fn blob_size_of_missing_path_is_legitimate_nonzero_exit() {
    let repo = TempRepo::new("blob-missing-path");
    repo.init();
    repo.write_file("a.txt", "1\n");
    repo.git_ok(&["add", "."]);
    repo.commit_at(1, "c1");
    let c1 = repo.head_oid();

    let runner = GitRunner::new();
    let path = RepoPath::parse("does-not-exist.txt").unwrap();
    let query = BlobSize::new(Side::Oid(c1), path).unwrap();
    let run_output = runner
        .run(&query, &repo.native_target())
        .await
        .expect("execute 層級不應失敗");
    match &run_output.calls[0] {
        Err(RunnerError::Failed { exit_code, .. }) => assert_eq!(*exit_code, Some(128)),
        other => panic!("應為 Failed(exit_code=128)，實際：{other:?}"),
    }
}

// ---------------------------------------------------------------------------
// BlobId（git-review task 3.3 fix round 1；Ruling R8）
// ---------------------------------------------------------------------------

/// `BlobId` 的結果必須等於直接跑 `git rev-parse <oid>:<path>` 的結果——這是它存在的唯一理由
/// （拿到跟真正的 git 物件 hash 逐字相同的值，而不是自訂雜湊）。
#[tokio::test]
async fn blob_id_matches_plain_git_rev_parse_for_a_commit() {
    let repo = TempRepo::new("blob-id-commit");
    repo.init();
    repo.write_file("a.txt", "hello world\n");
    repo.git_ok(&["add", "."]);
    repo.commit_at(1, "c1");
    let c1 = repo.head_oid();

    let expected_out = repo.git_ok(&["rev-parse", &format!("{}:a.txt", c1.as_str())]);
    let expected = String::from_utf8_lossy(&expected_out.stdout)
        .trim()
        .to_string();
    assert_eq!(expected.len(), 40, "測試前提：這台機器的 repo 應為 SHA-1");

    let runner = GitRunner::new();
    let target = repo.native_target();
    let path = RepoPath::parse("a.txt").unwrap();
    let query = BlobId::new(Side::Oid(c1), path).unwrap();
    let run_output = runner.run(&query, &target).await.expect("BlobId 應成功");
    let hash = BlobId::parse(&run_output.calls).expect("BlobId 應能解析");

    assert_eq!(hash, expected);
}

/// 暫存區版本同理：`BlobId` 讀的是 index 記錄的物件 hash，跟 `git rev-parse :<path>` 一致；
/// 修改暫存區內容後兩者一起變。
#[tokio::test]
async fn blob_id_matches_plain_git_rev_parse_for_the_index() {
    let repo = TempRepo::new("blob-id-index");
    repo.init();
    repo.write_file("a.txt", "staged one\n");
    repo.git_ok(&["add", "."]);

    let expected_out = repo.git_ok(&["rev-parse", ":a.txt"]);
    let expected_before = String::from_utf8_lossy(&expected_out.stdout)
        .trim()
        .to_string();

    let runner = GitRunner::new();
    let target = repo.native_target();
    let path = RepoPath::parse("a.txt").unwrap();
    let query = BlobId::new(Side::Index, path.clone()).unwrap();
    let run_output = runner.run(&query, &target).await.expect("BlobId 應成功");
    let hash_before = BlobId::parse(&run_output.calls).expect("BlobId 應能解析");
    assert_eq!(hash_before, expected_before);

    // 改暫存區內容：兩者應一起變。
    repo.write_file("a.txt", "staged two, different content\n");
    repo.git_ok(&["add", "."]);

    let expected_out = repo.git_ok(&["rev-parse", ":a.txt"]);
    let expected_after = String::from_utf8_lossy(&expected_out.stdout)
        .trim()
        .to_string();
    assert_ne!(
        expected_before, expected_after,
        "測試前提：修改內容後物件 hash 應改變"
    );

    let run_output = runner.run(&query, &target).await.expect("BlobId 應成功");
    let hash_after = BlobId::parse(&run_output.calls).expect("BlobId 應能解析");
    assert_eq!(hash_after, expected_after);
}

/// 檔案在該版本不存在 → 合法非零結束（同 `BlobSize`／`VerifyCommit` 的既有模式）。
#[tokio::test]
async fn blob_id_of_missing_path_is_legitimate_nonzero_exit() {
    let repo = TempRepo::new("blob-id-missing-path");
    repo.init();
    repo.write_file("a.txt", "1\n");
    repo.git_ok(&["add", "."]);
    repo.commit_at(1, "c1");
    let c1 = repo.head_oid();

    let runner = GitRunner::new();
    let path = RepoPath::parse("does-not-exist.txt").unwrap();
    let query = BlobId::new(Side::Oid(c1), path).unwrap();
    let run_output = runner
        .run(&query, &repo.native_target())
        .await
        .expect("execute 層級不應失敗");
    match &run_output.calls[0] {
        Err(RunnerError::Failed { .. }) => {}
        other => panic!("應為 Failed，實際：{other:?}"),
    }
}

// ---------------------------------------------------------------------------
// BlobHead（git-review task 3.3 fix round 2；Ruling R10）
// ---------------------------------------------------------------------------

/// 檔案大於 8192 位元組時，`BlobHead` 恰好回傳前 8192 位元組、`truncated` 為真——這是它存在的
/// 唯一理由（`meta` 端點藉此分類 `viewer` 而不必整份讀取）。
#[tokio::test]
async fn blob_head_returns_exactly_8192_bytes_and_truncated_for_a_larger_file() {
    let repo = TempRepo::new("blob-head-large");
    repo.init();
    // 產生一個超過 8192 位元組、內容可辨識的檔案（每個位元組是其索引對 251 取餘數，避免整份
    // 都是同一個位元組、無法驗證「恰好是前 8192 位元組」）。
    let content: Vec<u8> = (0..20_000).map(|i: usize| (i % 251) as u8).collect();
    assert!(content.len() > 8192, "測試前提：檔案應大於 8192 位元組");
    repo.write_file_bytes("big.bin", &content);
    repo.git_ok(&["add", "."]);
    repo.commit_at(1, "c1");
    let c1 = repo.head_oid();

    let runner = GitRunner::new();
    let target = repo.native_target();
    let path = RepoPath::parse("big.bin").unwrap();
    let query = BlobHead::new(Side::Oid(c1), path).unwrap();
    let run_output = runner
        .run(&query, &target)
        .await
        .expect("BlobHead 應成功（截斷不是錯誤）");

    let outcome = run_output.calls[0]
        .as_ref()
        .expect("截斷應回傳前段，不是 Err");
    assert!(outcome.truncated, "應標記為 truncated");
    assert_eq!(outcome.stdout.len(), 8192, "應恰好回傳 8192 個位元組");
    assert_eq!(
        outcome.stdout,
        &content[..8192],
        "回傳內容應等於檔案的前 8192 個位元組"
    );
}

/// 檔案小於 8192 位元組時，`BlobHead` 回傳完整內容、不截斷（同 `Blob` 讀到的內容一致）。
#[tokio::test]
async fn blob_head_returns_full_content_untruncated_for_a_smaller_file() {
    let repo = TempRepo::new("blob-head-small");
    repo.init();
    repo.write_file("small.txt", "hello world\n");
    repo.git_ok(&["add", "."]);
    repo.commit_at(1, "c1");
    let c1 = repo.head_oid();

    let runner = GitRunner::new();
    let target = repo.native_target();
    let path = RepoPath::parse("small.txt").unwrap();
    let query = BlobHead::new(Side::Oid(c1), path).unwrap();
    let run_output = runner.run(&query, &target).await.expect("BlobHead 應成功");

    let outcome = run_output.calls[0].as_ref().expect("應成功");
    assert!(!outcome.truncated);
    assert_eq!(outcome.stdout, b"hello world\n");
}

// ---------------------------------------------------------------------------
// 安全性：repo 設定的外部程式不被執行（spec「repo 設定的外部程式不被執行」）
// ---------------------------------------------------------------------------

/// 對照測試：不帶 cockpit 唯讀前綴的 plain `git status` 會觸發設定的 `core.fsmonitor` 腳本——
/// 證明標記檔機制本身有效，之後主測試斷言「標記檔不存在」不是因為腳本本身沒被正確設定。
#[test]
fn contrast_plain_git_status_triggers_configured_fsmonitor() {
    let repo = TempRepo::new("contrast-fsmonitor");
    repo.init();
    repo.write_file("a.txt", "hello\n");
    repo.git_ok(&["add", "."]);
    repo.commit_at(1, "base");

    let marker = repo.path().join("marker.touched");
    let script = repo.path().join("fsmonitor.sh");
    write_marker_script(&script, &marker);
    repo.git_ok(&["config", "core.fsmonitor", &to_forward_slash(&script)]);

    assert!(!marker.exists());
    let out = repo.git(&["status"]);
    assert!(
        out.status.success(),
        "plain git status 應成功：{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        marker.exists(),
        "不帶 -c core.fsmonitor=false 的 plain git status 應觸發設定的 fsmonitor 腳本"
    );
}

/// 對照測試：不帶 `--no-ext-diff` 的 plain `git diff` 會觸發設定的 `diff.external` 腳本。
#[test]
fn contrast_plain_git_diff_triggers_configured_diff_external() {
    let repo = TempRepo::new("contrast-diff-external");
    repo.init();
    repo.write_file("a.txt", "line1\n");
    repo.git_ok(&["add", "."]);
    repo.commit_at(1, "base");
    repo.write_file("a.txt", "line1 changed\n");

    let marker = repo.path().join("marker.touched");
    let script = repo.path().join("diff_external.sh");
    write_marker_script(&script, &marker);
    repo.git_ok(&["config", "diff.external", &to_forward_slash(&script)]);

    assert!(!marker.exists());
    let out = repo.git(&["diff"]);
    assert!(
        out.status.success(),
        "plain git diff 應成功：{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        marker.exists(),
        "不帶 --no-ext-diff 的 plain git diff 應觸發設定的 diff.external 腳本"
    );
}

/// 對照測試：不帶 `--no-textconv` 的 plain `git diff` 會觸發 `.gitattributes` 指定的
/// textconv 驅動。
#[test]
fn contrast_plain_git_diff_triggers_configured_textconv() {
    let repo = TempRepo::new("contrast-textconv");
    repo.init();
    repo.write_file("a.txt", "line1\n");
    repo.write_file(".gitattributes", "*.txt diff=marker\n");
    repo.git_ok(&["add", "."]);
    repo.commit_at(1, "base");
    repo.write_file("a.txt", "line1 changed\n");

    let marker = repo.path().join("marker.touched");
    let script = repo.path().join("textconv.sh");
    write_marker_script(&script, &marker);
    repo.git_ok(&["config", "diff.marker.textconv", &to_forward_slash(&script)]);

    assert!(!marker.exists());
    let out = repo.git(&["diff"]);
    assert!(
        out.status.success(),
        "plain git diff 應成功：{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        marker.exists(),
        "不帶 --no-textconv 的 plain git diff 應觸發設定的 textconv 驅動"
    );
}

/// 主測試：`.git/config` 同時設定 `core.fsmonitor`、`diff.external`、textconv（配
/// `.gitattributes`）、`log.showSignature=true`＋`gpg.program`，四個都指向會 touch 標記檔的
/// 腳本；依序對這個 repo 執行 `Status`、`Refs`、`Log`、`CommitInfo`、`ChangedFiles`、
/// `FileDiff`、`Blob`，全部經 `GitRunner::run`。所有設定都在 fixture 建置**完成之後**才寫入
/// （用 plain `git config`，不會自己觸發任何一個腳本），避免建 fixture 過程中的 plain git
/// 呼叫誤觸發標記檔，污染斷言。
#[tokio::test]
async fn queries_through_git_runner_never_execute_repo_configured_external_programs() {
    let repo = TempRepo::new("security-no-external-programs");
    repo.init();
    repo.write_file("a.txt", "line1\nline2\n");
    repo.git_ok(&["add", "."]);
    repo.commit_at(1, "base");

    repo.write_file(".gitattributes", "*.txt diff=marker\n");
    repo.git_ok(&["add", "."]);
    repo.commit_at(2, "add gitattributes");
    let head = repo.head_oid();

    let markers_dir = repo.path().join("_markers");
    std::fs::create_dir_all(&markers_dir).expect("建立標記檔目錄失敗");
    let fsmonitor_marker = markers_dir.join("fsmonitor.touched");
    let diff_external_marker = markers_dir.join("diff_external.touched");
    let textconv_marker = markers_dir.join("textconv.touched");
    let gpg_marker = markers_dir.join("gpg.touched");

    let fsmonitor_script = markers_dir.join("fsmonitor.sh");
    let diff_external_script = markers_dir.join("diff_external.sh");
    let textconv_script = markers_dir.join("textconv.sh");
    let gpg_script = markers_dir.join("gpg.sh");
    write_marker_script(&fsmonitor_script, &fsmonitor_marker);
    write_marker_script(&diff_external_script, &diff_external_marker);
    write_marker_script(&textconv_script, &textconv_marker);
    write_marker_script(&gpg_script, &gpg_marker);

    // 全部設定寫在 fixture 建置之後：`git config` 本身只改 `.git/config`，不會觸發任何
    // 一個腳本。
    repo.git_ok(&[
        "config",
        "core.fsmonitor",
        &to_forward_slash(&fsmonitor_script),
    ]);
    repo.git_ok(&[
        "config",
        "diff.external",
        &to_forward_slash(&diff_external_script),
    ]);
    repo.git_ok(&[
        "config",
        "diff.marker.textconv",
        &to_forward_slash(&textconv_script),
    ]);
    repo.git_ok(&["config", "log.showSignature", "true"]);
    repo.git_ok(&["config", "gpg.program", &to_forward_slash(&gpg_script)]);

    // 讓 a.txt 有未 commit 的修改，供 Status／ChangedFiles／FileDiff 查詢。
    repo.write_file("a.txt", "line1\nline2 changed\n");

    let runner = GitRunner::new();
    let target = repo.native_target();
    let a_path = RepoPath::parse("a.txt").unwrap();

    let run_output = runner
        .run(&Status, &target)
        .await
        .expect("Status 應成功（回 200 等價）");
    Status.parse(&run_output.calls).expect("Status 應能解析");

    let run_output = runner.run(&Refs, &target).await.expect("Refs 應成功");
    Refs.parse(&run_output.calls).expect("Refs 應能解析");

    let log_query = Log {
        tips: vec![head.clone()],
        limit: 200,
    };
    let run_output = runner.run(&log_query, &target).await.expect("Log 應成功");
    log_query.parse(&run_output.calls).expect("Log 應能解析");

    let commit_info_query = CommitInfo { oid: head.clone() };
    let run_output = runner
        .run(&commit_info_query, &target)
        .await
        .expect("CommitInfo 應成功");
    commit_info_query
        .parse(&run_output.calls)
        .expect("CommitInfo 應能解析");

    let changed_files_query = ChangedFiles::new(Side::Index, Side::Worktree).unwrap();
    let run_output = runner
        .run(&changed_files_query, &target)
        .await
        .expect("ChangedFiles 應成功");
    changed_files_query
        .parse(&run_output.calls)
        .expect("ChangedFiles 應能解析");

    let file_diff_query = FileDiff::new(Side::Index, Side::Worktree, a_path.clone(), None).unwrap();
    let run_output = runner
        .run(&file_diff_query, &target)
        .await
        .expect("FileDiff 應成功");
    file_diff_query
        .parse(&run_output.calls)
        .expect("FileDiff 應能解析");

    let blob_query = Blob::new(Side::Oid(head), a_path).unwrap();
    let run_output = runner.run(&blob_query, &target).await.expect("Blob 應成功");
    assert!(run_output.calls[0].is_ok(), "Blob 應成功讀到內容");

    assert!(
        !fsmonitor_marker.exists(),
        "core.fsmonitor 指定的腳本不應被執行"
    );
    assert!(
        !diff_external_marker.exists(),
        "diff.external 指定的腳本不應被執行"
    );
    assert!(!textconv_marker.exists(), "textconv 指定的驅動不應被執行");
    assert!(
        !gpg_marker.exists(),
        "log.showSignature=true 指定的 gpg.program 不應被執行"
    );
}

// ---------------------------------------------------------------------------
// 讀取狀態不寫入 index（spec「讀取狀態不寫入 index」）
// ---------------------------------------------------------------------------

/// 只改 mtime、不改內容——用一個未來時間戳確保與目前 index 快取的 stat 資訊不同，不依賴
/// 檔案系統的 mtime 解析度。
fn touch_future_mtime(path: &Path) {
    let file = std::fs::OpenOptions::new()
        .write(true)
        .open(path)
        .expect("開檔設定 mtime 失敗");
    let future = SystemTime::now() + Duration::from_secs(10);
    file.set_modified(future).expect("設定修改時間失敗");
}

#[tokio::test]
async fn status_via_git_runner_does_not_write_index() {
    let repo = TempRepo::new("no-index-write");
    repo.init();
    repo.write_file("tracked.txt", "hello\n");
    repo.git_ok(&["add", "."]);
    repo.commit_at(1, "base");
    // probe ⑩：先跑一次 status 讓 index 的 stat cache 穩定下來，才有可比較的基準。
    repo.git_ok(&["status"]);

    let index_path = repo.path().join(".git").join("index");
    let before_bytes = std::fs::read(&index_path).expect("應能讀取 .git/index");
    let before_mtime = std::fs::metadata(&index_path)
        .expect("應能讀取 .git/index 的 metadata")
        .modified()
        .expect("應能讀取修改時間");

    touch_future_mtime(&repo.path().join("tracked.txt"));

    let runner = GitRunner::new();
    let run_output = runner
        .run(&Status, &repo.native_target())
        .await
        .expect("Status 應成功");
    Status.parse(&run_output.calls).expect("Status 應能解析");

    let after_bytes = std::fs::read(&index_path).expect("應能讀取 .git/index");
    let after_mtime = std::fs::metadata(&index_path)
        .expect("應能讀取 .git/index 的 metadata")
        .modified()
        .expect("應能讀取修改時間");

    assert_eq!(before_bytes, after_bytes, ".git/index 的內容不應改變");
    assert_eq!(before_mtime, after_mtime, ".git/index 的修改時間不應改變");
    assert!(
        !repo.path().join(".git").join("index.lock").exists(),
        "不應出現 index.lock"
    );
}

/// 對照測試：不帶 `--no-optional-locks` 的 plain `git status` 會改寫 index 的 stat cache
/// （修改時間應改變）——證明「不寫入」的斷言不是因為這個情境本來就不會觸發 index 寫入。
#[test]
fn contrast_plain_git_status_writes_index_stat_cache() {
    let repo = TempRepo::new("contrast-index-write");
    repo.init();
    repo.write_file("tracked.txt", "hello\n");
    repo.git_ok(&["add", "."]);
    repo.commit_at(1, "base");
    repo.git_ok(&["status"]);

    let index_path = repo.path().join(".git").join("index");
    let before_mtime = std::fs::metadata(&index_path)
        .expect("應能讀取 .git/index 的 metadata")
        .modified()
        .expect("應能讀取修改時間");

    touch_future_mtime(&repo.path().join("tracked.txt"));

    let out = repo.git(&["status"]);
    assert!(
        out.status.success(),
        "plain git status 應成功：{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let after_mtime = std::fs::metadata(&index_path)
        .expect("應能讀取 .git/index 的 metadata")
        .modified()
        .expect("應能讀取修改時間");

    assert_ne!(
        before_mtime, after_mtime,
        "不帶 --no-optional-locks 的 plain git status 應改寫 index 的 stat cache（修改時間應改變）"
    );
}

/// repo 設 `diff.suppressBlankEmpty=true` 時，git 對空白 context 行輸出 `""` 而非 `" "`；解析器
/// 把 `""` 當殘渣跳過，會漏列並讓後續行號靜默錯位。固定前綴帶
/// `-c diff.suppressBlankEmpty=false`（命令列覆寫 repo 設定）還原成 `" "`。
#[tokio::test]
async fn file_diff_is_not_misaligned_by_repo_diff_suppress_blank_empty() {
    let repo = TempRepo::new("suppress-blank-empty");
    repo.init();
    repo.git_ok(&["config", "diff.suppressBlankEmpty", "true"]);
    repo.write_file("a.txt", "a\n\nb\nc\n");
    repo.git_ok(&["add", "."]);
    repo.commit_at(1, "base");
    repo.write_file("a.txt", "a\n\nb\nX\n");

    let runner = GitRunner::new();
    let target = repo.native_target();
    let query = FileDiff::new(
        Side::Index,
        Side::Worktree,
        RepoPath::parse("a.txt").unwrap(),
        None,
    )
    .expect("應為合法組合");
    let out = runner.run(&query, &target).await.expect("FileDiff 應成功");
    let diff = query.parse(&out.calls).expect("FileDiff 應能解析");

    assert!(
        diff.rows.iter().any(|r| matches!(
            r,
            DiffRow::Context { left, right } if left.line == 2 && left.text.is_empty() && right.line == 2
        )),
        "空白 context 行（第 2 行）應列出：{:?}",
        diff.rows
    );
    assert!(
        diff.rows.iter().any(|r| matches!(
            r,
            DiffRow::Change { left, right }
                if left.line == 4 && left.text == "c" && right.line == 4 && right.text == "X"
        )),
        "c→X 應在第 4 行：{:?}",
        diff.rows
    );
}

/// Ruling R13：工作區側的 diff（`Index → Worktree`、`Oid → Worktree`）不得改寫 `.git/index`。
/// porcelain `git diff` 會刷新 stat cache 並重寫 index（即使帶 `--no-optional-locks`），所以
/// 這兩個組合改走 plumbing `diff-files`／`diff-index`（不刷新 stat）。fixture：`stale.txt` 只
/// touch（內容不變，stat 變舊）、`real.txt` 真的改了內容。
#[tokio::test]
async fn worktree_side_diffs_do_not_write_index_and_ignore_stat_dirty_files() {
    let repo = TempRepo::new("worktree-diff-no-index-write");
    repo.init();
    repo.write_file("stale.txt", "same\n");
    repo.write_file("real.txt", "one\ntwo\n");
    repo.git_ok(&["add", "."]);
    repo.commit_at(1, "base");
    let base = repo.head_oid();
    repo.git_ok(&["status"]); // 讓 index 的 stat cache 穩定，作為可比較的基準

    touch_future_mtime(&repo.path().join("stale.txt"));
    repo.write_file("real.txt", "one\ntwo changed\n");
    touch_future_mtime(&repo.path().join("real.txt"));

    let index_path = repo.path().join(".git").join("index");
    let snapshot = || {
        (
            std::fs::read(&index_path).expect("應能讀取 .git/index"),
            std::fs::metadata(&index_path)
                .expect("應能讀取 .git/index 的 metadata")
                .modified()
                .expect("應能讀取修改時間"),
        )
    };
    let before = snapshot();

    let runner = GitRunner::new();
    let target = repo.native_target();

    // ChangedFiles（INDEX→WORKTREE）：stat-dirty 的 stale.txt 不得出現，real.txt 要在。
    let changed = ChangedFiles::new(Side::Index, Side::Worktree).expect("應為合法組合");
    let run_output = runner
        .run(&changed, &target)
        .await
        .expect("ChangedFiles 應成功");
    let listing = changed
        .parse(&run_output.calls)
        .expect("ChangedFiles 應能解析");
    assert!(
        listing.files.iter().all(|f| f.path != "stale.txt"),
        "只 touch 的檔案不應出現在清單：{:?}",
        listing.files
    );
    let real = listing
        .files
        .iter()
        .find(|f| f.path == "real.txt")
        .expect("真正修改的 real.txt 應在清單中");
    assert_eq!(real.status, 'M');
    assert_eq!(real.additions, Some(1));
    assert_eq!(real.deletions, Some(1));

    // FileDiff：INDEX→WORKTREE 與 HEAD oid→WORKTREE 兩種組合。
    for from in [Side::Index, Side::Oid(base.clone())] {
        let stale = FileDiff::new(
            from.clone(),
            Side::Worktree,
            RepoPath::parse("stale.txt").unwrap(),
            None,
        )
        .expect("應為合法組合");
        let out = runner.run(&stale, &target).await.expect("FileDiff 應成功");
        let diff = stale.parse(&out.calls).expect("FileDiff 應能解析");
        assert!(diff.rows.is_empty(), "stat-dirty 檔案的 diff 應為空");

        let real = FileDiff::new(
            from.clone(),
            Side::Worktree,
            RepoPath::parse("real.txt").unwrap(),
            None,
        )
        .expect("應為合法組合");
        let out = runner.run(&real, &target).await.expect("FileDiff 應成功");
        let diff = real.parse(&out.calls).expect("FileDiff 應能解析");
        assert!(
            diff.rows.iter().any(|r| matches!(
                r,
                DiffRow::Change { right, .. } if right.text == "two changed"
            )),
            "真正修改的檔案應有 change row：{:?}",
            diff.rows
        );
    }

    let after = snapshot();
    assert_eq!(before.0, after.0, ".git/index 的內容不應改變");
    assert_eq!(before.1, after.1, ".git/index 的修改時間不應改變");
    assert!(
        !repo.path().join(".git").join("index.lock").exists(),
        "不應出現 index.lock"
    );
}

/// 對照測試：porcelain `git diff --no-optional-locks` 對 stat 變舊的檔案仍會重寫 index——
/// 證明上一個測試的「不變」不是因為情境本來就不會觸發寫入（Ruling R13 的機制）。
#[test]
fn contrast_porcelain_diff_rewrites_index_despite_no_optional_locks() {
    let repo = TempRepo::new("contrast-diff-index-write");
    repo.init();
    repo.write_file("tracked.txt", "hello\n");
    repo.git_ok(&["add", "."]);
    repo.commit_at(1, "base");
    repo.git_ok(&["status"]);

    let index_path = repo.path().join(".git").join("index");
    let before = std::fs::metadata(&index_path)
        .expect("應能讀取 .git/index 的 metadata")
        .modified()
        .expect("應能讀取修改時間");

    touch_future_mtime(&repo.path().join("tracked.txt"));

    let out = repo.git(&["--no-optional-locks", "diff", "--name-status"]);
    assert!(out.status.success(), "git diff 應成功");

    let after = std::fs::metadata(&index_path)
        .expect("應能讀取 .git/index 的 metadata")
        .modified()
        .expect("應能讀取修改時間");
    assert_ne!(
        before, after,
        "porcelain git diff 應重寫 index（修改時間應改變）"
    );
}

// ---------------------------------------------------------------------------
// WSL 版（`#[ignore]`：`COCKPIT_GIT_TEST_WSL_DISTRO` 指定 distro，控制端手動執行
// `cargo test -p cockpit-git -- --ignored`）
// ---------------------------------------------------------------------------

/// 對 `wsl.exe` 的呼叫一律 `--exec`（不經 shell、不用 `--`／`--cd`，design D2），跟
/// `GitTarget::Wsl` 的 argv 組裝方式一致。fixture 建置也刻意全程走 `--exec`：即使是我們自己
/// 建立那個刻意取了 shell 特殊字元檔名的檔案，也是靠「沒有 shell 可以解讀它」這個機制本身，
/// 不是靠額外的跳脫或引號技巧。
fn wsl_exec(distro: &str, args: &[&str]) -> std::process::Output {
    Command::new("wsl.exe")
        .args(["-d", distro, "--exec"])
        .args(args)
        .output()
        .expect("啟動 wsl.exe 失敗")
}

fn wsl_exec_ok(distro: &str, args: &[&str]) -> std::process::Output {
    let out = wsl_exec(distro, args);
    assert!(
        out.status.success(),
        "wsl.exe --exec {args:?} 應成功，stderr：{}",
        String::from_utf8_lossy(&out.stderr)
    );
    out
}

fn wsl_git_ok(distro: &str, dir: &str, args: &[&str]) -> std::process::Output {
    let mut full: Vec<&str> = vec![
        "git",
        "-C",
        dir,
        "-c",
        "user.name=cockpit-git-test",
        "-c",
        "user.email=test@example.invalid",
        "-c",
        "commit.gpgsign=false",
    ];
    full.extend_from_slice(args);
    wsl_exec_ok(distro, &full)
}

/// commit 需要固定日期；`wsl.exe` 不會把 Windows 端的環境變數轉送進 WSL guest，所以用
/// `env KEY=value ... git ...` 的方式（跟 `GitTarget::Wsl` argv 本身用 `env LC_ALL=C` 的
/// 手法一致）讓這些變數對 git 子程序本身生效。
fn wsl_git_commit(distro: &str, dir: &str, seq: u32, message: &str) {
    let date = format!("2026-01-01T00:00:{seq:02}+00:00");
    let author = format!("GIT_AUTHOR_DATE={date}");
    let committer = format!("GIT_COMMITTER_DATE={date}");
    let out = wsl_exec(
        distro,
        &[
            "env",
            &author,
            &committer,
            "git",
            "-C",
            dir,
            "-c",
            "user.name=cockpit-git-test",
            "-c",
            "user.email=test@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "-q",
            "-m",
            message,
        ],
    );
    assert!(
        out.status.success(),
        "wsl git commit 應成功：{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// 不經 shell 寫入檔案內容：`tee` 是真正的程式，從 stdin 讀內容寫進指定路徑，沒有任何一步
/// 需要 shell 解讀重導向語法。
fn wsl_write_file(distro: &str, posix_path: &str, content: &[u8]) {
    let mut child = Command::new("wsl.exe")
        .args(["-d", distro, "--exec", "tee", posix_path])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .spawn()
        .expect("啟動 wsl.exe --exec tee 失敗");
    child
        .stdin
        .take()
        .expect("stdin 應為 piped")
        .write_all(content)
        .expect("寫入 tee 的 stdin 失敗");
    let status = child.wait().expect("等待 tee 結束失敗");
    assert!(status.success(), "wsl tee 寫入檔案失敗");
}

struct WslCleanup {
    distro: String,
    posix_dir: String,
}

impl Drop for WslCleanup {
    fn drop(&mut self) {
        let _ = Command::new("wsl.exe")
            .args(["-d", &self.distro, "--exec", "rm", "-rf", &self.posix_dir])
            .output();
    }
}

#[tokio::test]
#[ignore = "需要 WSL 與環境變數 COCKPIT_GIT_TEST_WSL_DISTRO；控制端手動執行 --ignored"]
async fn wsl_status_and_file_diff_do_not_go_through_shell() {
    let distro = match std::env::var("COCKPIT_GIT_TEST_WSL_DISTRO") {
        Ok(v) if !v.is_empty() => v,
        _ => panic!(
            "需要設定環境變數 COCKPIT_GIT_TEST_WSL_DISTRO 指定要用的 WSL distro（例如 \
             Ubuntu-24.04）才能跑這個測試；這個 #[ignore] 測試平常不會自動執行，需控制端手動跑 \
             `cargo test -p cockpit-git -- --ignored`"
        ),
    };

    let mktemp_out = wsl_exec_ok(&distro, &["mktemp", "-d"]);
    let posix_dir = String::from_utf8_lossy(&mktemp_out.stdout)
        .trim()
        .to_string();
    assert!(
        posix_dir.starts_with("/tmp/"),
        "mktemp -d 應回傳 /tmp 底下的路徑，實際：{posix_dir:?}"
    );
    let _cleanup = WslCleanup {
        distro: distro.clone(),
        posix_dir: posix_dir.clone(),
    };

    wsl_git_ok(&distro, &posix_dir, &["init", "-q", "-b", "main"]);

    // 追蹤一個一般檔案，之後修改它用於 FileDiff——brief：未追蹤檔案的 diff（EMPTY→WORKTREE）
    // 走 cockpit 讀檔的純函式路徑，不經 GitRunner／FileDiff 這個查詢型別，所以這裡改用一個
    // 已追蹤檔案的 FileDiff（Index→Worktree）驗證 WSL 目標下的完整路徑。
    let tracked_posix = format!("{posix_dir}/tracked.md");
    wsl_write_file(&distro, &tracked_posix, b"line1\nline2\n");
    wsl_git_ok(&distro, &posix_dir, &["add", "tracked.md"]);
    wsl_git_commit(&distro, &posix_dir, 1, "base");
    wsl_write_file(&distro, &tracked_posix, b"line1\nline2 changed\n");

    // 刻意取一個內含 shell 命令替換語法的未追蹤檔名；用 `--exec touch <literal>`
    // 直接建立（沒有 shell 介入，`touch` 收到的是字面字串，不會被展開）。
    let odd_name = "$(touch pwned).md";
    let odd_posix = format!("{posix_dir}/{odd_name}");
    wsl_exec_ok(&distro, &["touch", &odd_posix]);

    let target = GitTarget::Wsl {
        distro: distro.clone(),
        posix: posix_dir.clone(),
    };
    let runner = GitRunner::new();

    let run_output = runner
        .run(&Status, &target)
        .await
        .expect("WSL Status 應成功");
    let status = Status
        .parse(&run_output.calls)
        .expect("WSL Status 應能解析");

    let odd_entry = status
        .entries
        .iter()
        .find(|e| e.path == odd_name)
        .unwrap_or_else(|| {
            panic!(
                "應找到名為 {odd_name:?} 的未追蹤項目，實際 entries：{:?}",
                status.entries
            )
        });
    assert_eq!(odd_entry.group, StatusGroup::Untracked);
    assert_eq!(odd_entry.status, '?');

    let file_diff_query = FileDiff::new(
        Side::Index,
        Side::Worktree,
        RepoPath::parse("tracked.md").unwrap(),
        None,
    )
    .expect("應為合法組合");
    let run_output = runner
        .run(&file_diff_query, &target)
        .await
        .expect("WSL FileDiff 應成功");
    let diff = file_diff_query
        .parse(&run_output.calls)
        .expect("WSL FileDiff 應能解析");
    assert!(
        diff.rows
            .iter()
            .any(|r| matches!(r, DiffRow::Change { .. })),
        "tracked.md 的修改應產生一列 change"
    );

    // 沒有出現名為 `pwned` 的檔案——`$(touch pwned).md` 這個檔名沒有被任何一層 shell 展開。
    let pwned_check = wsl_exec(&distro, &["test", "-e", &format!("{posix_dir}/pwned")]);
    assert!(
        !pwned_check.status.success(),
        "WSL 內不應該出現名為 pwned 的檔案（代表命令注入發生）"
    );
}

/// ui-fixes 修正波 1 B-I1：WSL 端 git（實測 Ubuntu-24.04 為 2.43）的 `%(*objecttype)` 只剝一層，
/// 巢狀附註 tag 的 `*objecttype` 仍是 `tag`。巢狀 tag → commit 在任何版本都必須 `commit == true`
/// （舊判定在 2.43 會是 false，使該 commit 從預設 Graph 消失）；巢狀 tag → tree 的 `commit`
/// 隨版本而異（新版 false、舊版 true），只斷言它仍被列出。
#[tokio::test]
#[ignore = "需要 WSL 與環境變數 COCKPIT_GIT_TEST_WSL_DISTRO；控制端手動執行 --ignored"]
async fn wsl_refs_nested_annotated_tag_to_commit_is_commit() {
    let distro = match std::env::var("COCKPIT_GIT_TEST_WSL_DISTRO") {
        Ok(v) if !v.is_empty() => v,
        _ => panic!(
            "需要設定環境變數 COCKPIT_GIT_TEST_WSL_DISTRO 指定要用的 WSL distro（例如 \
             Ubuntu-24.04）才能跑這個測試；這個 #[ignore] 測試平常不會自動執行，需控制端手動跑 \
             `cargo test -p cockpit-git -- --ignored`"
        ),
    };

    let mktemp_out = wsl_exec_ok(&distro, &["mktemp", "-d"]);
    let posix_dir = String::from_utf8_lossy(&mktemp_out.stdout)
        .trim()
        .to_string();
    assert!(
        posix_dir.starts_with("/tmp/"),
        "mktemp -d 應回傳 /tmp 底下的路徑，實際：{posix_dir:?}"
    );
    let _cleanup = WslCleanup {
        distro: distro.clone(),
        posix_dir: posix_dir.clone(),
    };

    wsl_git_ok(&distro, &posix_dir, &["init", "-q", "-b", "main"]);
    wsl_write_file(&distro, &format!("{posix_dir}/a.txt"), b"1\n");
    wsl_git_ok(&distro, &posix_dir, &["add", "a.txt"]);
    wsl_git_commit(&distro, &posix_dir, 1, "c1");

    let tree_out = wsl_git_ok(&distro, &posix_dir, &["rev-parse", "HEAD^{tree}"]);
    let tree = String::from_utf8_lossy(&tree_out.stdout).trim().to_string();
    // 附註 tag → commit，再以另一個附註 tag 指向它（巢狀 → commit）。
    wsl_git_ok(
        &distro,
        &posix_dir,
        &["tag", "-a", "ann-commit-tag", "-m", "c", "HEAD"],
    );
    wsl_git_ok(
        &distro,
        &posix_dir,
        &[
            "tag",
            "-a",
            "nested-commit-tag",
            "-m",
            "nc",
            "ann-commit-tag",
        ],
    );
    // 附註 tag → tree，再巢狀。
    wsl_git_ok(
        &distro,
        &posix_dir,
        &["tag", "-a", "ann-tree-tag", "-m", "t", tree.as_str()],
    );
    wsl_git_ok(
        &distro,
        &posix_dir,
        &["tag", "-a", "nested-tree-tag", "-m", "nt", "ann-tree-tag"],
    );

    let target = GitTarget::Wsl {
        distro: distro.clone(),
        posix: posix_dir.clone(),
    };
    let runner = GitRunner::new();
    let run_output = runner.run(&Refs, &target).await.expect("WSL Refs 應成功");
    let refs = Refs.parse(&run_output.calls).expect("WSL Refs 應能解析");

    let find = |name: &str| {
        refs.refs
            .iter()
            .find(|r| r.name == name)
            .unwrap_or_else(|| panic!("找不到 {name}：{:?}", refs.refs))
    };
    assert!(find("refs/tags/ann-commit-tag").commit);
    assert!(
        find("refs/tags/nested-commit-tag").commit,
        "巢狀 tag → commit 在任何 git 版本都必為 true"
    );
    assert!(
        !find("refs/tags/ann-tree-tag").commit,
        "單層附註 tag → tree 在任何版本都剝得到 tree，必為 false"
    );
    assert_eq!(find("refs/tags/nested-tree-tag").kind, RefKind::Tag);
}
