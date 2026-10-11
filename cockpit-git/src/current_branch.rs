//! `CurrentBranch` 查詢的輸出解析（openspec-stage-sync task 2.1，design D7）。

//!
//! `symbolic-ref -q HEAD` 的三種結果（spec `git-review`「目前分支」）：
//!
//! - exit 0、輸出以 `refs/heads/` 開頭 → `Ok(Some(分支名稱))`（含 `/` 的名稱完整保留）。
//! - exit 0、輸出不是 `refs/heads/` 開頭（例如 HEAD 指向 `refs/remotes/...`）→ `Ok(None)`。
//! - exit 1 且 stderr 為空 → detached HEAD（`-q` 讓它安靜地失敗，git 官方文件 `git-symbolic-ref`）
//!   → `Ok(None)`。stderr 非空的 exit 1 **不是** detached（例如 `wsl.exe` 自己失敗也以 1 結束），
//!   視為錯誤。
//! - 其他（不是 repo 的 128、其他結束碼、逾時、擁有者不符、輸出格式不符）→ `Err`，呼叫端不得
//!   據此認定「沒有分支」。

use crate::parse_error::GitParseError;
use crate::query::CurrentBranch;
use crate::runner::{CallOutcome, RunnerError};

/// `-q` 下 `symbolic-ref` 對「不是符號參照（detached HEAD）」的結束碼。
const NOT_A_SYMBOLIC_REF_EXIT_CODE: i32 = 1;
const HEADS_PREFIX: &str = "refs/heads/";

impl CurrentBranch {
    /// 解析 [`CurrentBranch::commands`](crate::GitQuery::commands) 的唯一一次呼叫。回傳值的語意
    /// 見模組文件：`Ok(None)` 是「沒有分支」，不是錯誤。
    pub fn parse(
        calls: &[Result<CallOutcome, RunnerError>],
    ) -> Result<Option<String>, GitParseError> {
        if calls.len() != 1 {
            return Err(GitParseError::UnexpectedCallCount);
        }
        let outcome = match &calls[0] {
            Ok(outcome) => outcome,
            Err(RunnerError::Failed {
                exit_code: Some(NOT_A_SYMBOLIC_REF_EXIT_CODE),
                stderr_tail,
            }) if stderr_tail.is_empty() => return Ok(None),
            Err(other) => return Err(GitParseError::UnexpectedCallOutcome(other.to_string())),
        };

        let text = std::str::from_utf8(&outcome.stdout).map_err(|_| {
            GitParseError::MalformedOutput("symbolic-ref 輸出不是合法的 UTF-8".to_string())
        })?;
        // 只剝掉結尾的一個換行（可帶 CR）；refname 本身不可 trim。
        let body = text.strip_suffix('\n').unwrap_or(text);
        let refname = body.strip_suffix('\r').unwrap_or(body);
        if refname.is_empty() {
            return Err(GitParseError::MalformedOutput(
                "symbolic-ref 成功但沒有輸出".to_string(),
            ));
        }
        match refname.strip_prefix(HEADS_PREFIX) {
            Some("") => Err(GitParseError::MalformedOutput(
                "symbolic-ref 輸出只有 refs/heads/ 前綴，沒有分支名稱".to_string(),
            )),
            Some(branch) => Ok(Some(branch.to_string())),
            None => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::parse_error::GitParseError;
    use crate::query::CurrentBranch;
    use crate::runner::{CallOutcome, RunnerError};

    fn ok_call(stdout: &str) -> Vec<Result<CallOutcome, RunnerError>> {
        vec![Ok(CallOutcome {
            stdout: stdout.as_bytes().to_vec(),
            truncated: false,
        })]
    }

    fn failed_call(exit_code: Option<i32>, stderr: &str) -> Vec<Result<CallOutcome, RunnerError>> {
        vec![Err(RunnerError::Failed {
            exit_code,
            stderr_tail: stderr.as_bytes().to_vec(),
        })]
    }

    /// spec「目前分支查詢回傳分支名稱」：去掉 `refs/heads/` 與結尾換行。
    #[test]
    fn plain_branch_strips_refs_heads_prefix() {
        assert_eq!(
            CurrentBranch::parse(&ok_call("refs/heads/main\n")),
            Ok(Some("main".to_string()))
        );
    }

    #[test]
    fn parses_without_trailing_newline() {
        assert_eq!(
            CurrentBranch::parse(&ok_call("refs/heads/main")),
            Ok(Some("main".to_string()))
        );
    }

    /// spec「目前分支查詢回傳分支名稱」：含 `/` 的名稱完整保留。
    #[test]
    fn branch_name_with_slashes_is_kept_whole() {
        assert_eq!(
            CurrentBranch::parse(&ok_call("refs/heads/feat/openspec-stage-sync\n")),
            Ok(Some("feat/openspec-stage-sync".to_string()))
        );
        assert_eq!(
            CurrentBranch::parse(&ok_call("refs/heads/heads/x\n")),
            Ok(Some("heads/x".to_string())),
            "只去掉最前面的 refs/heads/，分支名稱本身叫 heads/x 時不可再被吃掉"
        );
    }

    /// CRLF 結尾：`\r` 不可留在分支名稱尾巴。
    #[test]
    fn crlf_line_ending_is_stripped() {
        assert_eq!(
            CurrentBranch::parse(&ok_call("refs/heads/main\r\n")),
            Ok(Some("main".to_string()))
        );
    }

    /// spec「HEAD 指向非 refs/heads 的 ref」：成功但不是分支 → `None`，不是錯誤。
    #[test]
    fn symbolic_ref_outside_refs_heads_is_none() {
        for text in [
            "refs/remotes/origin/main\n",
            "refs/tags/v1\n",
            "refs/headsx/main\n",
            "main\n",
        ] {
            assert_eq!(CurrentBranch::parse(&ok_call(text)), Ok(None), "{text:?}");
        }
    }

    /// spec「detached HEAD 沒有分支但不是錯誤」：`-q` 下 exit 1、stderr 空 → `None`。
    #[test]
    fn detached_head_exit_1_without_output_is_none() {
        assert_eq!(CurrentBranch::parse(&failed_call(Some(1), "")), Ok(None));
    }

    /// exit 1 但 stderr 有內容（例如 `wsl.exe` 自己失敗也是 exit 1）不是 detached：錯誤。
    #[test]
    fn exit_1_with_stderr_is_an_error_not_detached() {
        assert!(matches!(
            CurrentBranch::parse(&failed_call(Some(1), "WSL distro not found")),
            Err(GitParseError::UnexpectedCallOutcome(_))
        ));
    }

    /// spec「目前分支查詢失敗為錯誤」：不是 repo（128）、無結束碼、其他結束碼 → 錯誤。
    #[test]
    fn other_failures_are_errors() {
        for exit_code in [Some(128), Some(2), None] {
            assert!(
                matches!(
                    CurrentBranch::parse(&failed_call(exit_code, "fatal: not a git repository")),
                    Err(GitParseError::UnexpectedCallOutcome(_))
                ),
                "{exit_code:?}"
            );
        }
        for err in [
            RunnerError::Timeout,
            RunnerError::TooLarge,
            RunnerError::Untrusted {
                stderr_tail: b"detected dubious ownership".to_vec(),
            },
        ] {
            assert!(matches!(
                CurrentBranch::parse(&[Err(err)]),
                Err(GitParseError::UnexpectedCallOutcome(_))
            ));
        }
    }

    #[test]
    fn empty_or_prefix_only_output_is_malformed() {
        for text in ["", "\n", "refs/heads/\n"] {
            assert!(
                matches!(
                    CurrentBranch::parse(&ok_call(text)),
                    Err(GitParseError::MalformedOutput(_))
                ),
                "{text:?}"
            );
        }
    }

    #[test]
    fn non_utf8_output_is_malformed() {
        let calls = vec![Ok(CallOutcome {
            stdout: b"refs/heads/\xff\xfe\n".to_vec(),
            truncated: false,
        })];
        assert!(matches!(
            CurrentBranch::parse(&calls),
            Err(GitParseError::MalformedOutput(_))
        ));
    }

    #[test]
    fn wrong_call_count_is_an_error() {
        assert_eq!(
            CurrentBranch::parse(&[]),
            Err(GitParseError::UnexpectedCallCount)
        );
    }
}
