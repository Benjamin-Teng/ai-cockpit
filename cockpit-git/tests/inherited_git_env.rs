//! ui-fixes task 3.2 / design D5 ③：呼叫端行程帶著 `GIT_DIR` 指向另一個 repo 時，查詢結果仍是目標 repo。
//!
//! 要改的是**呼叫端行程**的環境（runner 建的子行程會被清掉，在那裡設等於沒測），所以這個測試獨立成
//! 只含它自己的整合測試檔：每個整合測試檔是獨立執行檔，這裡只有一個 `#[test]`，在建立 tokio
//! runtime 之前 `unsafe { std::env::set_var(..) }`（edition 2024 要求 unsafe；沒有並行執行緒才安全），
//! 再手動建 runtime 跑查詢。兩個 repo 都在 `std::env::temp_dir()` 下現建、結束時清除；找不到 git 時
//! 失敗而非略過（同 `tests/real_git.rs`）。

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use cockpit_git::{GitRunner, GitTarget, Refs, Status, StatusGroup};

/// 建 fixture 用的 git 呼叫（不經 `GitRunner`）；必須在 `set_var` 之前使用。
fn git(dir: &Path, args: &[&str]) {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args([
            "-c",
            "user.name=cockpit-git-test",
            "-c",
            "user.email=test@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "-c",
            "core.autocrlf=false",
        ])
        .args(args)
        .output()
        .expect("啟動 git 失敗——測試環境應已安裝 git");
    assert!(
        out.status.success(),
        "git {args:?} 應成功，stderr：{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

fn mtime(path: &Path) -> SystemTime {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .expect("讀 index 修改時間失敗")
}

struct TempDir(PathBuf);

impl TempDir {
    fn new(label: &str) -> TempDir {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("系統時間應晚於 UNIX epoch")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "cockpit-git-inherited-env-{label}-{}-{nanos}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).expect("建立暫存目錄失敗");
        TempDir(dir)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn inherited_git_dir_does_not_redirect_queries() {
    // 目標 repo：main 分支、已提交 a.rs／b.rs，b.rs 有未暫存修改。
    let target = TempDir::new("target");
    git(&target.0, &["init", "-q", "-b", "main"]);
    std::fs::write(target.0.join("a.rs"), "fn a() {}\n").unwrap();
    std::fs::write(target.0.join("b.rs"), "fn b() {}\n").unwrap();
    git(&target.0, &["add", "."]);
    git(&target.0, &["commit", "-q", "-m", "initial"]);
    std::fs::write(target.0.join("b.rs"), "fn b() { /* changed */ }\n").unwrap();

    // 另一個 repo：other 分支、有一個 commit、沒有任何變更。
    let other = TempDir::new("other");
    git(&other.0, &["init", "-q", "-b", "other"]);
    std::fs::write(other.0.join("z.txt"), "z\n").unwrap();
    git(&other.0, &["add", "."]);
    git(&other.0, &["commit", "-q", "-m", "other initial"]);

    let other_git_dir = other.0.join(".git");
    let other_index = other_git_dir.join("index");
    let other_index_before = std::fs::read(&other_index).expect("讀另一個 repo 的 index 失敗");
    let other_mtime_before = mtime(&other_index);
    // spec 要求兩個 repo 的 index「內容與修改時間」都不變，所以目標 repo 也記下 fixture 完成後的狀態。
    let target_index = target.0.join(".git").join("index");
    let target_index_before = std::fs::read(&target_index).expect("讀目標 repo 的 index 失敗");
    let target_mtime_before = mtime(&target_index);

    // 建 runtime 之前就把「錯誤的 repo 定位」放進呼叫端行程的環境（此時還只有這一條執行緒）。
    // SAFETY: 這個執行檔只有這一個 #[test]，且 tokio runtime 尚未建立，沒有其他執行緒會同時讀寫環境。
    unsafe {
        std::env::set_var("GIT_DIR", &other_git_dir);
        std::env::set_var("GIT_WORK_TREE", &other.0);
        std::env::set_var("GIT_INDEX_FILE", &other_index);
    }

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("建立 tokio runtime 失敗");
    runtime.block_on(async {
        let runner = GitRunner::new();
        let git_target = GitTarget::Native {
            path: target.0.to_string_lossy().into_owned(),
        };

        let status_out = runner
            .run(&Status, &git_target)
            .await
            .expect("Status 應成功");
        let status = Status.parse(&status_out.calls).expect("Status 應能解析");
        assert_eq!(
            status.branch.head.as_deref(),
            Some("main"),
            "應讀到目標 repo 而不是 GIT_DIR 指向的 other"
        );
        assert!(
            status
                .entries
                .iter()
                .any(|e| e.path == "b.rs" && e.group == StatusGroup::Unstaged),
            "目標 repo 的 b.rs 未暫存修改應出現，實際：{:?}",
            status.entries
        );

        let refs_out = runner.run(&Refs, &git_target).await.expect("Refs 應成功");
        let refs = Refs.parse(&refs_out.calls).expect("Refs 應能解析");
        let names: Vec<&str> = refs.refs.iter().map(|r| r.name.as_str()).collect();
        assert!(names.contains(&"refs/heads/main"), "refs：{names:?}");
        assert!(!names.contains(&"refs/heads/other"), "refs：{names:?}");
    });

    assert_eq!(
        std::fs::read(&other_index).expect("讀另一個 repo 的 index 失敗"),
        other_index_before,
        "另一個 repo 的 index 內容不應被動到"
    );
    assert_eq!(
        mtime(&other_index),
        other_mtime_before,
        "另一個 repo 的 index 修改時間不應被動到"
    );
    assert_eq!(
        std::fs::read(&target_index).expect("讀目標 repo 的 index 失敗"),
        target_index_before,
        "目標 repo 的 index 內容不應被查詢動到"
    );
    assert_eq!(
        mtime(&target_index),
        target_mtime_before,
        "目標 repo 的 index 修改時間不應被查詢動到"
    );
}
