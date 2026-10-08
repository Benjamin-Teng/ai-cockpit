//! `RepoIdentity` 查詢的輸出解析（repo-projects task 2.1，design D1）。
//!
//! `rev-parse --path-format=absolute --git-common-dir --git-dir --show-toplevel` 成功時輸出三行，
//! 依引數順序：共同 `.git` 目錄、該工作樹自己的 git 目錄、工作樹根目錄。這裡**只回傳 git 的
//! 原始路徑字串**，不做 repo key 正規化（小寫、反斜線、WSL 轉 UNC；那是 task 4.3 的事）。
//!
//! # 「不是 repo」與「暫時性錯誤」的分工
//!
//! git 對「不是 repo」「裸 repo」「位於 `.git` 目錄內」「cwd 不存在」都以 exit 128 結束
//! （2026-10-08 於 Windows git 2.50.1 實測；裸 repo 與 `.git` 內時 stdout 還會先印出前兩行，
//! 之後才以 `fatal: this operation must be run in a work tree` 失敗，所以**非零結束時絕不能
//! 解析 stdout**）。[`RepoIdentity::parse`] 因此把結果分三類：
//!
//! - `Ok(Some(..))`：成功，三個原始路徑。
//! - `Ok(None)`：這個 cwd 不屬於任何 repo——唯一的呼叫是 [`RunnerError::Failed`] 且
//!   `exit_code == Some(128)`。
//! - `Err(..)`：暫時性或非預期的錯誤，呼叫端應「暫時不歸類、稍後重試」，不得當作「不是 repo」：
//!   輸出格式不符（[`GitParseError::MalformedOutput`]）、呼叫次數不對、`Failed` 但結束碼不是
//!   128（含 `None`，即執行期 I/O 錯誤）、`TooLarge`。
//!
//! [`GitRunner::run`](crate::GitRunner::run) 本身回 `Err(RunnerError::{Unavailable, Untrusted,
//! Timeout})`（git 不存在、dubious ownership、逾時）時根本走不到 `parse`，呼叫端同樣視為暫時性
//! 錯誤。

use crate::parse_error::GitParseError;
use crate::query::RepoIdentity;
use crate::runner::{CallOutcome, RunnerError};

/// git 對「不是 repo」類情況的結束碼（`fatal:` 一律 128）。
const GIT_FATAL_EXIT_CODE: i32 = 128;

/// [`RepoIdentity`] 成功時的輸出：git 回報的三個**原始**路徑字串（未正規化）。
/// Windows git 輸出正斜線（`D:/work/app/.git`），WSL 端輸出 POSIX 路徑。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoIdentityOutput {
    /// `--git-common-dir`：共同 `.git` 目錄（主 worktree 與所有 linked worktree 共用）。
    pub common_dir: String,
    /// `--git-dir`：這個工作樹自己的 git 目錄（linked worktree 時是 `<共同>/worktrees/<名稱>`）。
    pub git_dir: String,
    /// `--show-toplevel`：工作樹根目錄。
    pub toplevel: String,
}

impl RepoIdentity {
    /// 解析 [`RepoIdentity::commands`](crate::GitQuery::commands) 的唯一一次呼叫。回傳值的語意
    /// 見模組文件：`Ok(None)` 代表「不是 repo」。
    pub fn parse(
        calls: &[Result<CallOutcome, RunnerError>],
    ) -> Result<Option<RepoIdentityOutput>, GitParseError> {
        if calls.len() != 1 {
            return Err(GitParseError::UnexpectedCallCount);
        }
        let outcome = match &calls[0] {
            Ok(outcome) => outcome,
            Err(RunnerError::Failed {
                exit_code: Some(GIT_FATAL_EXIT_CODE),
                ..
            }) => return Ok(None),
            Err(other) => return Err(GitParseError::UnexpectedCallOutcome(other.to_string())),
        };

        let text = std::str::from_utf8(&outcome.stdout).map_err(|_| {
            GitParseError::MalformedOutput("rev-parse 輸出不是合法的 UTF-8".to_string())
        })?;
        // 只剝掉結尾的一個換行（git 的輸出以換行結尾）；路徑本身的空白不可 trim。
        let body = text.strip_suffix('\n').unwrap_or(text);
        let lines: Vec<&str> = body
            .split('\n')
            .map(|line| line.strip_suffix('\r').unwrap_or(line))
            .collect();
        let [common_dir, git_dir, toplevel] = lines.as_slice() else {
            return Err(GitParseError::MalformedOutput(format!(
                "rev-parse 應輸出 3 行，實際 {} 行",
                lines.len()
            )));
        };
        if [common_dir, git_dir, toplevel]
            .iter()
            .any(|line| line.is_empty())
        {
            return Err(GitParseError::MalformedOutput(
                "rev-parse 輸出含空行".to_string(),
            ));
        }
        Ok(Some(RepoIdentityOutput {
            common_dir: (*common_dir).to_string(),
            git_dir: (*git_dir).to_string(),
            toplevel: (*toplevel).to_string(),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::RepoIdentityOutput;
    use crate::parse_error::GitParseError;
    use crate::query::RepoIdentity;
    use crate::runner::{CallOutcome, RunnerError};

    fn ok_call(stdout: &str) -> Vec<Result<CallOutcome, RunnerError>> {
        vec![Ok(CallOutcome {
            stdout: stdout.as_bytes().to_vec(),
            truncated: false,
        })]
    }

    fn failed_call(exit_code: Option<i32>) -> Vec<Result<CallOutcome, RunnerError>> {
        vec![Err(RunnerError::Failed {
            exit_code,
            stderr_tail: b"fatal: not a git repository".to_vec(),
        })]
    }

    fn expected() -> RepoIdentityOutput {
        RepoIdentityOutput {
            common_dir: "D:/work/app/.git".to_string(),
            git_dir: "D:/work/app/.git/worktrees/wt".to_string(),
            toplevel: "D:/work/app-wt/feat".to_string(),
        }
    }

    const THREE_LINES: &str =
        "D:/work/app/.git\nD:/work/app/.git/worktrees/wt\nD:/work/app-wt/feat\n";

    #[test]
    fn parses_three_lines_in_argument_order() {
        assert_eq!(
            RepoIdentity::parse(&ok_call(THREE_LINES)),
            Ok(Some(expected()))
        );
    }

    #[test]
    fn parses_without_trailing_newline() {
        let text = THREE_LINES.trim_end_matches('\n');
        assert_eq!(RepoIdentity::parse(&ok_call(text)), Ok(Some(expected())));
    }

    /// CRLF 結尾：每行的 `\r` 要去掉，不能留在路徑尾巴（會讓 repo key 永遠對不上）。
    #[test]
    fn parses_crlf_line_endings() {
        let text = THREE_LINES.replace('\n', "\r\n");
        assert_eq!(RepoIdentity::parse(&ok_call(&text)), Ok(Some(expected())));
    }

    /// 路徑中的空白是路徑的一部分，不可 trim。
    #[test]
    fn keeps_spaces_inside_and_around_paths() {
        let text = "D:/my work/app/.git\nD:/my work/app/.git\n D:/my work/app \n";
        let out = RepoIdentity::parse(&ok_call(text))
            .expect("應可解析")
            .expect("應是 repo");
        assert_eq!(out.common_dir, "D:/my work/app/.git");
        assert_eq!(out.toplevel, " D:/my work/app ");
    }

    #[test]
    fn rejects_wrong_line_counts() {
        for text in ["", "\n", "a\n", "a\nb\n", "a\nb\nc\nd\n"] {
            match RepoIdentity::parse(&ok_call(text)) {
                Err(GitParseError::MalformedOutput(_)) => {}
                other => panic!("{text:?} 應為 MalformedOutput，實際：{other:?}"),
            }
        }
    }

    #[test]
    fn rejects_empty_lines() {
        for text in ["\nb\nc\n", "a\n\nc\n", "a\nb\n\n", "a\r\n\r\nc\r\n"] {
            match RepoIdentity::parse(&ok_call(text)) {
                Err(GitParseError::MalformedOutput(_)) => {}
                other => panic!("{text:?} 應為 MalformedOutput，實際：{other:?}"),
            }
        }
    }

    #[test]
    fn rejects_non_utf8_output() {
        let calls = vec![Ok(CallOutcome {
            stdout: vec![b'a', b'\n', 0xff, 0xfe, b'\n', b'c', b'\n'],
            truncated: false,
        })];
        match RepoIdentity::parse(&calls) {
            Err(GitParseError::MalformedOutput(_)) => {}
            other => panic!("應為 MalformedOutput，實際：{other:?}"),
        }
    }

    /// exit 128 ＝ 不是 repo（含裸 repo、位於 `.git` 內、cwd 不存在）。
    #[test]
    fn exit_128_means_not_a_repo() {
        assert_eq!(RepoIdentity::parse(&failed_call(Some(128))), Ok(None));
    }

    /// 其他非零結束碼、或執行期 I/O 錯誤（`exit_code == None`）不是「不是 repo」，是暫時性錯誤。
    #[test]
    fn other_failures_are_errors_not_not_a_repo() {
        for code in [Some(1), Some(129), Some(-1), None] {
            match RepoIdentity::parse(&failed_call(code)) {
                Err(GitParseError::UnexpectedCallOutcome(_)) => {}
                other => panic!("{code:?} 應為 UnexpectedCallOutcome，實際：{other:?}"),
            }
        }
    }

    #[test]
    fn too_large_is_an_error() {
        let calls = vec![Err(RunnerError::TooLarge)];
        match RepoIdentity::parse(&calls) {
            Err(GitParseError::UnexpectedCallOutcome(_)) => {}
            other => panic!("應為 UnexpectedCallOutcome，實際：{other:?}"),
        }
    }

    #[test]
    fn rejects_wrong_call_count() {
        assert_eq!(
            RepoIdentity::parse(&[]),
            Err(GitParseError::UnexpectedCallCount)
        );
    }
}
