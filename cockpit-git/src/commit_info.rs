//! `CommitInfo` 查詢的輸出型別與解析器（git-review design D4「CommitInfo」）。
//!
//! task 2.3 只解析 `show` 本身吐出的欄位；spec「commit 詳情端點」另外要求的
//! `compared_to`／`files`（依第一個 parent 或 `EMPTY` 比較、呼叫 [`crate::ChangedFiles`]）
//! 是呼叫端（之後的 `cockpit` 端點）的編排邏輯，不在這裡。

use crate::parse_error::GitParseError;
use crate::parse_support::single_call;
use crate::query::CommitInfo;
use crate::runner::{CallOutcome, RunnerError};

/// `author`／`committer` 共用的形狀。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitPerson {
    pub name: String,
    pub email: String,
    pub time: i64,
}

/// `CommitInfo` 查詢的輸出。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitInfoOutput {
    pub oid: String,
    pub parents: Vec<String>,
    pub author: CommitPerson,
    pub committer: CommitPerson,
    pub message: String,
}

/// `%H%x00%P%x00%an%x00%ae%x00%at%x00%cn%x00%ce%x00%ct%x00%B`：固定 9 欄（design D4）。
const FIELD_COUNT: usize = 9;

impl CommitInfo {
    /// 解析 [`CommitInfo::commands`] 的唯一一次呼叫（design D4）。
    pub fn parse(
        &self,
        calls: &[Result<CallOutcome, RunnerError>],
    ) -> Result<CommitInfoOutput, GitParseError> {
        let outcome = single_call(calls)?;
        let mut tokens: Vec<&[u8]> = outcome.stdout.split(|&b| b == 0).collect();
        // `CommitInfo` 不可截斷（`truncatable()` 預設 `false`），超過上限時執行器已經回
        // `TooLarge`，這裡看到的一定是完整輸出，只需要丟掉結尾 NUL 產生的空字串。
        if matches!(tokens.last(), Some(last) if last.is_empty()) {
            tokens.pop();
        }
        if tokens.len() != FIELD_COUNT {
            return Err(GitParseError::MalformedOutput(format!(
                "show 輸出的欄位數不是 {FIELD_COUNT}（實際 {}）",
                tokens.len()
            )));
        }

        let field = |i: usize| String::from_utf8_lossy(tokens[i]).into_owned();
        let parse_time = |i: usize| -> Result<i64, GitParseError> {
            std::str::from_utf8(tokens[i])
                .ok()
                .and_then(|s| s.parse::<i64>().ok())
                .ok_or_else(|| GitParseError::MalformedOutput("commit 時間不是整數".to_string()))
        };

        let oid = field(0);
        let parents = field(1).split_whitespace().map(str::to_string).collect();
        let author = CommitPerson {
            name: field(2),
            email: field(3),
            time: parse_time(4)?,
        };
        let committer = CommitPerson {
            name: field(5),
            email: field(6),
            time: parse_time(7)?,
        };
        let message = field(8);

        Ok(CommitInfoOutput {
            oid,
            parents,
            author,
            committer,
            message,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::CommitInfoOutput;
    use crate::query::CommitInfo;
    use crate::runner::{CallOutcome, RunnerError};
    use crate::{GitParseError, Oid};

    fn nul_join(fields: &[&str]) -> Vec<u8> {
        let mut out = Vec::new();
        for f in fields {
            out.extend_from_slice(f.as_bytes());
            out.push(0);
        }
        out
    }

    fn oid(byte: char) -> Oid {
        Oid::parse(&std::iter::repeat_n(byte, 40).collect::<String>()).expect("測試 oid 應合法")
    }

    fn parse(stdout: Vec<u8>) -> Result<CommitInfoOutput, GitParseError> {
        let query = CommitInfo { oid: oid('a') };
        query.parse(&[Ok(CallOutcome {
            stdout,
            truncated: false,
        })])
    }

    #[test]
    fn parses_merge_commit_with_two_parents_and_multiline_message() {
        let stdout = nul_join(&[
            &oid('e').to_string(),
            &format!("{} {}", oid('d'), oid('c')),
            "Alice",
            "alice@example.com",
            "1700000000",
            "Bob",
            "bob@example.com",
            "1700000100",
            "Merge branch 'feat'\n\n完整訊息第二行",
        ]);

        let output = parse(stdout).expect("應解析成功");
        assert_eq!(output.oid, oid('e').to_string());
        assert_eq!(
            output.parents,
            vec![oid('d').to_string(), oid('c').to_string()]
        );
        assert_eq!(output.author.name, "Alice");
        assert_eq!(output.author.email, "alice@example.com");
        assert_eq!(output.author.time, 1700000000);
        assert_eq!(output.committer.name, "Bob");
        assert_eq!(output.committer.time, 1700000100);
        assert_eq!(output.message, "Merge branch 'feat'\n\n完整訊息第二行");
    }

    /// spec commit 詳情端點 Scenario「根 commit」的前提：沒有 parent。
    #[test]
    fn root_commit_has_empty_parents() {
        let stdout = nul_join(&[
            &oid('a').to_string(),
            "",
            "Alice",
            "alice@example.com",
            "1600000000",
            "Alice",
            "alice@example.com",
            "1600000000",
            "Initial commit\n",
        ]);
        let output = parse(stdout).expect("應解析成功");
        assert!(output.parents.is_empty());
    }

    #[test]
    fn wrong_field_count_is_malformed() {
        let stdout = nul_join(&[&oid('a').to_string(), "only two fields"]);
        let err = parse(stdout).expect_err("欄位數不對應回明確錯誤");
        assert!(matches!(err, GitParseError::MalformedOutput(_)));
    }

    #[test]
    fn non_integer_time_is_malformed() {
        let stdout = nul_join(&[
            &oid('a').to_string(),
            "",
            "Alice",
            "alice@example.com",
            "not-a-number",
            "Alice",
            "alice@example.com",
            "1600000000",
            "Initial commit",
        ]);
        let err = parse(stdout).expect_err("author time 不是整數應回明確錯誤");
        assert!(matches!(err, GitParseError::MalformedOutput(_)));
    }

    #[test]
    fn wrong_call_count_is_unexpected_call_count() {
        let query = CommitInfo { oid: oid('a') };
        let err = query
            .parse(&[])
            .expect_err("呼叫次數不是 1 應回 UnexpectedCallCount");
        assert_eq!(err, GitParseError::UnexpectedCallCount);
    }

    #[test]
    fn call_failure_maps_to_unexpected_call_outcome() {
        let query = CommitInfo { oid: oid('a') };
        let calls: Vec<Result<CallOutcome, RunnerError>> = vec![Err(RunnerError::Failed {
            exit_code: Some(128),
            stderr_tail: Vec::new(),
        })];
        let err = query
            .parse(&calls)
            .expect_err("show 呼叫失敗應轉成解析錯誤");
        assert!(matches!(err, GitParseError::UnexpectedCallOutcome(_)));
    }
}
