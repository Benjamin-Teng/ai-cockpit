//! `GitRunner` 的執行器驗收測試（git-review task 2.2，design D3）。
//!
//! 全程只用假程式（`cockpit-git-test-fake`，`src/bin/git_fake.rs`）驅動，**不啟動真正的
//! git**（那是 task 2.6 的事）：假程式的 argv 透過 [`cockpit_git::QueryPlan::for_test`]
//! 直接塞進 `QueryPlan`，繞過 [`cockpit_git::GitQuery`]（sealed trait，本檔案在 crate 外，
//! 本來就無法實作它）。`QueryPlan::for_test` 與 `GitRunner::execute` 都只在
//! `test-support` feature 下存在（fix round 1，design D1 Goal「HTTP 層拿不到任意 git 引數
//! 的入口」——正式建置下沒有這兩個入口，見 `src/runner.rs` 模組文件「crate 外部入口的
//! 封閉性」）；本檔案能用它們是因為 `Cargo.toml` 的 `[dev-dependencies]` 對自己開了這個
//! feature，只在 `cargo test` 時生效。
//! 逾時／並行測試用短逾時（`GitRunner::with_limits`）而非 design 正式的 10 秒／4 支，
//! 避免測試變慢；brief 明確授權這個測試專用建構子。

use std::time::{Duration, Instant};

use cockpit_git::{GitRunner, QueryPlan, RunnerError};

fn fake_bin() -> &'static str {
    env!("CARGO_BIN_EXE_cockpit-git-test-fake")
}

fn plan(calls: Vec<Vec<String>>, stdout_cap: usize, truncatable: bool) -> QueryPlan {
    QueryPlan::for_test(calls, stdout_cap, truncatable)
}

fn call(args: &[&str]) -> Vec<String> {
    let mut argv = vec![fake_bin().to_string()];
    argv.extend(args.iter().map(|s| s.to_string()));
    argv
}

/// 查作業系統確認 pid 是否仍存在：Windows 用 `tasklist /FI "PID eq <pid>"`，unix 用
/// `kill -0 <pid>`（同 `herdr-client/tests/transport.rs` 的 `process_exists` 寫法——直接呼叫
/// 這兩支系統工具本身，不經 `cmd /c`／shell，符合 AGENTS.md 對驗證輔助工具的一般要求；本檔案
/// 的假程式本體仍完全不依賴 shell）。
fn process_exists(pid: u32) -> bool {
    #[cfg(windows)]
    {
        let output = std::process::Command::new("tasklist")
            .args(["/FI", &format!("PID eq {pid}"), "/NH"])
            .output()
            .expect("執行 tasklist 失敗");
        let stdout = String::from_utf8_lossy(&output.stdout);
        stdout.contains(&pid.to_string())
    }
    #[cfg(unix)]
    {
        let status = std::process::Command::new("kill")
            .args(["-0", &pid.to_string()])
            .status()
            .expect("執行 kill -0 失敗");
        status.success()
    }
}

// ---------------------------------------------------------------------------
// 找不到程式 → Unavailable（brief 驗收）
// ---------------------------------------------------------------------------

#[tokio::test]
async fn missing_program_is_unavailable_and_does_not_count_as_spawned() {
    let runner = GitRunner::with_limits(4, Duration::from_secs(5));
    let missing = vec!["cockpit-git-definitely-does-not-exist-xyz".to_string()];

    let err = runner
        .execute(plan(vec![missing], 1024, false))
        .await
        .expect_err("找不到程式應回錯誤");

    assert!(
        matches!(err, RunnerError::Unavailable(_)),
        "應分類為 Unavailable，實際：{err:?}"
    );
    assert_eq!(
        runner.spawned(),
        0,
        "spawn 失敗不應計入已啟動子程序次數（control-panel ruling P1）"
    );
}

// ---------------------------------------------------------------------------
// LC_ALL=C 下的 dubious ownership stderr → Untrusted（probe ⑦ 原文）
// ---------------------------------------------------------------------------

#[tokio::test]
async fn dubious_ownership_stderr_is_untrusted() {
    let runner = GitRunner::with_limits(4, Duration::from_secs(5));
    // `git-review-probe.md` ⑦ 記錄的原文（Windows git 對 WSL repo 的 dubious ownership）。
    let stderr_text = "fatal: detected dubious ownership in repository at '//wsl.localhost/Ubuntu-24.04/tmp/probe-dir'";
    let argv = call(&["exit-with", "128", stderr_text]);

    let err = runner
        .execute(plan(vec![argv], 1024, false))
        .await
        .expect_err("dubious ownership 應回錯誤");

    match err {
        RunnerError::Untrusted { stderr_tail } => {
            let text = String::from_utf8_lossy(&stderr_tail);
            assert!(
                text.contains("detected dubious ownership"),
                "stderr_tail 應包含判別字串，實際：{text:?}"
            );
        }
        other => panic!("應分類為 Untrusted，實際：{other:?}"),
    }
}

// ---------------------------------------------------------------------------
// 其他非零結束（非 dubious ownership）→ Failed，且不中止同一查詢剩下的呼叫
// ---------------------------------------------------------------------------

#[tokio::test]
async fn generic_nonzero_exit_is_failed_with_exit_code_and_stderr() {
    let runner = GitRunner::with_limits(4, Duration::from_secs(5));
    // `git-review-probe.md` ⑦ 記錄的原文（非 repo 目錄）。
    let stderr_text = "fatal: not a git repository (or any of the parent directories): .git";
    let argv = call(&["exit-with", "128", stderr_text]);

    let output = runner
        .execute(plan(vec![argv], 1024, false))
        .await
        .expect("單次呼叫的非零結束不應讓 execute() 整體回錯誤");

    assert_eq!(output.calls.len(), 1);
    match &output.calls[0] {
        Err(RunnerError::Failed {
            exit_code,
            stderr_tail,
        }) => {
            assert_eq!(*exit_code, Some(128));
            assert!(String::from_utf8_lossy(stderr_tail).contains("not a git repository"));
        }
        other => panic!("應分類為 Failed，實際：{other:?}"),
    }
}

/// 架構決定（見 `src/runner.rs` 模組文件）：`Failed`／`TooLarge` 不會中止同一查詢剩下的呼叫
/// （對應 design D4 `Refs` 的 `rev-parse --verify -q HEAD` 在還沒有 commit 時合法地非零結束，
/// 後面的 `symbolic-ref -q HEAD` 仍應該被執行）。
#[tokio::test]
async fn failed_call_does_not_abort_remaining_calls_in_the_same_query() {
    let runner = GitRunner::with_limits(4, Duration::from_secs(5));
    let calls = vec![
        call(&["stdout-bytes", "3"]),
        call(&["exit-with", "1", ""]),
        call(&["stdout-bytes", "4"]),
    ];

    let output = runner
        .execute(plan(calls, 1024, false))
        .await
        .expect("Failed 不應讓整個查詢中止");

    assert_eq!(output.calls.len(), 3);
    assert_eq!(output.calls[0].as_ref().unwrap().stdout.len(), 3);
    assert!(matches!(
        output.calls[1],
        Err(RunnerError::Failed {
            exit_code: Some(1),
            ..
        })
    ));
    assert_eq!(output.calls[2].as_ref().unwrap().stdout.len(), 4);
}

/// 對照：`Unavailable` 屬於環境層級的系統性錯誤，應該中止整個查詢，不繼續嘗試剩下的呼叫。
#[tokio::test]
async fn unavailable_call_aborts_remaining_calls_in_the_same_query() {
    let runner = GitRunner::with_limits(4, Duration::from_secs(5));
    let calls = vec![
        call(&["stdout-bytes", "1"]),
        vec!["cockpit-git-definitely-does-not-exist-xyz".to_string()],
        call(&["stdout-bytes", "1"]),
    ];

    let err = runner
        .execute(plan(calls, 1024, false))
        .await
        .expect_err("Unavailable 應讓 execute() 整體回錯誤");

    assert!(matches!(err, RunnerError::Unavailable(_)));
    assert_eq!(
        runner.spawned(),
        1,
        "第二次呼叫 spawn 失敗後不應再嘗試第三次呼叫"
    );
}

// ---------------------------------------------------------------------------
// stdout 上限：可截斷查詢回前段＋truncated；不可截斷查詢回 TooLarge
// ---------------------------------------------------------------------------

#[tokio::test]
async fn stdout_within_cap_is_returned_in_full_untruncated() {
    let runner = GitRunner::with_limits(4, Duration::from_secs(5));
    let argv = call(&["stdout-bytes", "5"]);

    let output = runner
        .execute(plan(vec![argv], 10, false))
        .await
        .expect("未超過上限不應是錯誤");

    let outcome = output.calls[0].as_ref().expect("應成功");
    assert_eq!(outcome.stdout, vec![b'a'; 5]);
    assert!(!outcome.truncated);
}

#[tokio::test]
async fn truncatable_query_over_cap_returns_truncated_prefix_not_an_error() {
    let runner = GitRunner::with_limits(4, Duration::from_secs(5));
    let argv = call(&["stdout-bytes", "50"]);

    let output = runner
        .execute(plan(vec![argv], 10, true))
        .await
        .expect("可截斷查詢超過上限不應讓 execute() 整體回錯誤");

    let outcome = output.calls[0]
        .as_ref()
        .expect("應是截斷後的成功結果，不是 Err");
    assert_eq!(outcome.stdout, vec![b'a'; 10], "應只保留前 10 個位元組");
    assert!(outcome.truncated);
}

#[tokio::test]
async fn non_truncatable_query_over_cap_is_too_large() {
    let runner = GitRunner::with_limits(4, Duration::from_secs(5));
    let argv = call(&["stdout-bytes", "50"]);

    let output = runner
        .execute(plan(vec![argv], 10, false))
        .await
        .expect("TooLarge 是單次呼叫的結果，不應讓 execute() 整體回錯誤");

    assert!(matches!(output.calls[0], Err(RunnerError::TooLarge)));
}

/// 超過上限時子程序也要被終止（design D3「超過上限時終止子程序」），不是放著讓它繼續寫。
/// 用比較大的輸出（遠大於上限，且大到光是等它自然寫完也要花看得到的時間）加一個寬鬆但有效
/// 的整體逾時，證明 `execute()` 沒有傻等子程序自然結束。
#[tokio::test]
async fn stdout_over_cap_terminates_promptly_without_waiting_for_natural_exit() {
    // 假程式一次寫完 50 個位元組就結束，不會自然卡住；這裡用一個遠比正常執行時間寬鬆的上限
    // （`GitRunner` 本身的逾時是 5 秒，這裡的斷言比它還寬）確認「超過 stdout 上限」這條路徑
    // 沒有因為某個邏輯錯誤去等一個不存在的事件（例如等 stderr 讀到 EOF——見 `runner.rs`
    // `communicate` 的死結說明）而卡到接近整個查詢的逾時；不是在賭系統負載下的精確時序，
    // 上限留了充分餘裕（fix round 2：一併檢查本檔案是否有其他「短逾時／窄時間窗格＋真實子
    // 程序時間」的競態，這個斷言方向相反——用寬鬆上限而非窄下限，風險本來就低，但一併放寬）。
    let runner = GitRunner::with_limits(4, Duration::from_secs(5));
    let argv = call(&["stdout-bytes", "50"]);

    let started = Instant::now();
    let output = runner
        .execute(plan(vec![argv], 10, true))
        .await
        .expect("不應是 execute() 層級的錯誤");
    let elapsed = started.elapsed();

    assert!(output.calls[0].as_ref().unwrap().truncated);
    assert!(
        elapsed < Duration::from_secs(4),
        "超過上限應迅速終止子程序，實際耗時：{elapsed:?}"
    );
}

// ---------------------------------------------------------------------------
// 逾時：子程序確定被終止
// ---------------------------------------------------------------------------

/// fix round 2：原本逾時只給 150 ms，系統忙碌時假程式從啟動到寫出 PID 檔可能超過
/// 150 ms，導致執行器在假程式來得及寫 PID 之前就終止它，PID 檔永遠不出現——這是測試的
/// 計時競態，不是執行器的 bug（控制端判讀，見 `task-2.2-report.md`「Fix round 2」）。改成
/// 逾時 2 秒、假程式睡 60 秒：2 秒遠大於任何合理的「行程啟動＋寫一個小檔案」耗時（即使系統
/// 忙碌），讓「逾時觸發前 PID 檔已經寫出」這個前提幾乎必然成立，而不是縮小某個判斷窗格去賭
/// 時序。
#[tokio::test]
async fn timeout_terminates_the_subprocess() {
    let dir = std::env::temp_dir().join(format!(
        "cockpit-git-timeout-test-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).expect("建立暫存目錄");
    let pid_file = dir.join("pid.txt");

    let runner = GitRunner::with_limits(4, Duration::from_secs(2));
    let argv = call(&[
        "sleep-record-pid",
        pid_file.to_str().expect("暫存路徑應為合法 UTF-8"),
        "60000",
    ]);

    let err = runner
        .execute(plan(vec![argv], 1024, false))
        .await
        .expect_err("應逾時");
    assert!(matches!(err, RunnerError::Timeout), "實際：{err:?}");

    // 此時執行器已經等滿 2 秒才判定逾時，假程式寫 PID 檔（啟動後幾乎立刻做的第一件事）早該
    // 完成；這裡仍留一個寬鬆的等待迴圈而不是直接讀檔案一次就斷言，避免檔案系統寫入可見性的
    // 極端延遲造成誤判，但正常情況下第一次迴圈就會成功。
    let pid_deadline = Instant::now() + Duration::from_secs(5);
    let pid: u32 = loop {
        if let Ok(content) = std::fs::read_to_string(&pid_file)
            && let Ok(pid) = content.trim().parse()
        {
            break pid;
        }
        assert!(Instant::now() < pid_deadline, "假程式應已寫入 PID 檔案");
        tokio::time::sleep(Duration::from_millis(20)).await;
    };

    let exit_deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if !process_exists(pid) {
            break;
        }
        assert!(
            Instant::now() < exit_deadline,
            "子程序（pid {pid}）在逾時後 5 秒內應已結束"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------------------
// 並行上限：第 5 支查詢等前面結束才啟動
// ---------------------------------------------------------------------------

/// fix round 2：原本 `HOLD_MS = 500`、在 `spawned() == 4` 之後只等 `HOLD_MS / 2`（250 ms）
/// 就斷言第 5 支還沒啟動——這個窗格假設 4 支 holder 的真實行程啟動時間彼此相差極小。系統忙碌
/// 時 4 次 `spawn()` 彼此之間可能有明顯不同的排程延遲（同一種「系統忙碌時真實子程序啟動時間
/// 拉長」的競態，控制端在 fix round 2 也要求一併檢查），若最早啟動的 holder 比最晚啟動的
/// holder 早了超過 250 ms，它可能在我們檢查的當下已經睡滿 500 ms 自然結束、釋放 semaphore，
/// 讓第 5 支提前啟動，使斷言失能。改成 `HOLD_MS = 3000`、檢查窗格固定給 800 ms（不隨
/// `HOLD_MS` 等比例縮放）：800 ms 遠大於任何合理的「4 次 spawn 彼此之間的排程延遲」，而
/// 3000 ms 的 hold 時間讓這個 800 ms 窗格相對於 hold 時間仍然很寬裕，兩個數字都不是靠精算
/// 卡在臨界值上。
#[tokio::test]
async fn fifth_call_waits_for_a_free_slot() {
    let runner = std::sync::Arc::new(GitRunner::with_limits(4, Duration::from_secs(30)));
    const HOLD_MS: u64 = 3000;
    const CHECK_AFTER_MS: u64 = 800;

    let mut holders = Vec::new();
    for _ in 0..4 {
        let runner = runner.clone();
        let argv = call(&["sleep", &HOLD_MS.to_string()]);
        holders.push(tokio::spawn(async move {
            runner.execute(plan(vec![argv], 1024, false)).await
        }));
    }

    // 等 4 支都真的啟動（spawn 成功）；deadline 也放寬，避免系統忙碌時連「4 支都啟動了沒」
    // 這個檢查本身就先假性逾時。
    let spawn_deadline = Instant::now() + Duration::from_secs(10);
    while runner.spawned() < 4 {
        assert!(
            Instant::now() < spawn_deadline,
            "4 支子程序應能在時限內全部啟動"
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    // 此時啟動第 5 支：它必須排隊等 semaphore 釋放，不應立刻 spawn。
    let runner_5th = runner.clone();
    let fifth = tokio::spawn(async move {
        runner_5th
            .execute(plan(vec![call(&["sleep", "10"])], 1024, false))
            .await
    });

    // 在前 4 支還在跑的期間（CHECK_AFTER_MS 遠小於 HOLD_MS，即使 4 支的實際啟動時間彼此有
    // 明顯偏移也還在安全範圍內），第 5 支不應該已經啟動。
    tokio::time::sleep(Duration::from_millis(CHECK_AFTER_MS)).await;
    assert_eq!(
        runner.spawned(),
        4,
        "第 5 支查詢應等前面 4 支結束才啟動（同時上限 4 支）"
    );

    for h in holders {
        h.await
            .expect("holder task 不應 panic")
            .expect("sleep 不應逾時或出錯");
    }
    fifth
        .await
        .expect("fifth task 不應 panic")
        .expect("sleep 不應逾時或出錯");

    assert_eq!(runner.spawned(), 5, "前 4 支結束後，第 5 支應已啟動並完成");
}

// ---------------------------------------------------------------------------
// stderr 超過保留上限後仍要持續排空（不能讓 git 收到 SIGPIPE）
// ---------------------------------------------------------------------------

/// 機制：stderr 只保留前 8 KiB，但若讀滿就結束並關閉管道，git 之後再寫 stderr（例如
/// `core.autocrlf=true` 的 400 個檔各一行 CRLF 警告，約 51 KB）會寫入失敗／收 SIGPIPE 而
/// 非零結束，讓本來成功的查詢變 `Failed`。假程式先寫 200 KB stderr、寫失敗就以 13 結束；
/// 執行器必須繼續排空 stderr，stdout 才會完整且回 Success。
#[tokio::test]
async fn stderr_far_beyond_cap_is_drained_so_the_child_still_succeeds() {
    let runner = GitRunner::with_limits(4, Duration::from_secs(10));
    let argv = call(&["stderr-then-stdout", "200000", "1234"]);

    let output = runner
        .execute(plan(vec![argv], 1024 * 1024, false))
        .await
        .expect("execute 不應整體失敗");

    assert_eq!(output.calls.len(), 1);
    let outcome = output.calls[0]
        .as_ref()
        .unwrap_or_else(|e| panic!("stderr 量大不應讓呼叫失敗，實際：{e:?}"));
    assert_eq!(outcome.stdout.len(), 1234, "stdout 應完整");
    assert!(!outcome.truncated);
}
