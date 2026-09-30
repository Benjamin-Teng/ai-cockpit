//! `Log` 查詢的輸出型別與解析器（git-review design D4「Log」）。
//!
//! task 2.3 brief：只解析到「commit 清單」為止（`oid`、`parents`、`author`、`email`、
//! `time`、`subject`）；Graph 排版是 task 2.5，`offset`／`limit` 的切片留給呼叫端——這裡
//! 回傳 git 依 `-n <limit>`（`limit` 欄位語意是 `offset + limit`，見 [`crate::query::Log`]
//! 文件）已經吐出的全部列。

use crate::parse_error::GitParseError;
use crate::parse_support::single_call;
use crate::query::Log;
use crate::runner::{CallOutcome, RunnerError};

/// commit 清單的一列（不含 `graph`——task 2.5 的範圍）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogRow {
    pub oid: String,
    pub parents: Vec<String>,
    pub author: String,
    pub email: String,
    pub time: i64,
    pub subject: String,
}

/// `Log` 查詢的輸出。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LogOutput {
    pub rows: Vec<LogRow>,
}

/// `%H%x00%P%x00%an%x00%ae%x00%at%x00%s`：每個 commit 固定 6 欄（design D4）。
const FIELD_COUNT: usize = 6;

impl Log {
    /// 解析 [`Log::commands`] 的唯一一次呼叫（design D4）。
    pub fn parse(
        &self,
        calls: &[Result<CallOutcome, RunnerError>],
    ) -> Result<LogOutput, GitParseError> {
        let outcome = single_call(calls)?;
        let mut tokens: Vec<&[u8]> = outcome.stdout.split(|&b| b == 0).collect();
        // `-z` 讓每個 commit 紀錄（最後一欄 `%s` 之後）以 NUL 結尾取代預設的換行；`Log` 不是
        // 可截斷查詢（`truncatable()` 預設 `false`），超過上限時執行器已經回 `TooLarge`，
        // 這裡看到的一定是完整輸出，只需要丟掉最後那個由結尾 NUL 產生的空字串。
        if matches!(tokens.last(), Some(last) if last.is_empty()) {
            tokens.pop();
        }
        if !tokens.len().is_multiple_of(FIELD_COUNT) {
            return Err(GitParseError::MalformedOutput(format!(
                "log 輸出的欄位數（{}）不是 {FIELD_COUNT} 的倍數",
                tokens.len()
            )));
        }

        let mut rows = Vec::with_capacity(tokens.len() / FIELD_COUNT);
        for chunk in tokens.chunks_exact(FIELD_COUNT) {
            let oid = String::from_utf8_lossy(chunk[0]).into_owned();
            let parents = String::from_utf8_lossy(chunk[1])
                .split_whitespace()
                .map(str::to_string)
                .collect();
            let author = String::from_utf8_lossy(chunk[2]).into_owned();
            let email = String::from_utf8_lossy(chunk[3]).into_owned();
            let time = std::str::from_utf8(chunk[4])
                .ok()
                .and_then(|s| s.parse::<i64>().ok())
                .ok_or_else(|| GitParseError::MalformedOutput("log 的 %at 不是整數".to_string()))?;
            let subject = String::from_utf8_lossy(chunk[5]).into_owned();
            rows.push(LogRow {
                oid,
                parents,
                author,
                email,
                time,
                subject,
            });
        }

        Ok(LogOutput { rows })
    }
}

#[cfg(test)]
mod tests {
    use super::LogOutput;
    use crate::query::Log;
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

    fn parse(stdout: Vec<u8>) -> Result<LogOutput, GitParseError> {
        let query = Log {
            tips: vec![oid('a')],
            limit: 200,
        };
        query.parse(&[Ok(CallOutcome {
            stdout,
            truncated: false,
        })])
    }

    #[test]
    fn parses_two_commits_with_a_merge_and_a_root() {
        let stdout = nul_join(&[
            &oid('b').to_string(),
            &format!("{} {}", oid('a'), oid('c')), // 合併 commit，2 個 parent
            "Alice",
            "alice@example.com",
            "1700000000",
            "Merge branch",
            &oid('a').to_string(),
            "", // root commit 沒有 parent
            "Bob",
            "bob@example.com",
            "1600000000",
            "Initial commit",
        ]);

        let output = parse(stdout).expect("應解析成功");
        assert_eq!(output.rows.len(), 2);

        let merge = &output.rows[0];
        assert_eq!(merge.oid, oid('b').to_string());
        assert_eq!(
            merge.parents,
            vec![oid('a').to_string(), oid('c').to_string()]
        );
        assert_eq!(merge.author, "Alice");
        assert_eq!(merge.email, "alice@example.com");
        assert_eq!(merge.time, 1700000000);
        assert_eq!(merge.subject, "Merge branch");

        let root = &output.rows[1];
        assert_eq!(root.oid, oid('a').to_string());
        assert!(root.parents.is_empty(), "根 commit 不應有 parent");
        assert_eq!(root.subject, "Initial commit");
    }

    #[test]
    fn lossy_converts_invalid_utf8_in_display_fields_without_erroring() {
        let mut stdout = Vec::new();
        stdout.extend_from_slice(oid('a').to_string().as_bytes());
        stdout.push(0);
        stdout.push(0); // parents 空
        stdout.extend_from_slice(b"Bad\xFFName");
        stdout.push(0);
        stdout.extend_from_slice(b"a@example.com");
        stdout.push(0);
        stdout.extend_from_slice(b"1600000000");
        stdout.push(0);
        stdout.extend_from_slice(b"Subject\xFFLine");
        stdout.push(0);

        let output = parse(stdout).expect("非法 UTF-8 應以有損轉換處理，不應報錯");
        assert_eq!(output.rows.len(), 1);
        assert!(output.rows[0].author.contains("Bad"));
        assert!(output.rows[0].subject.contains("Subject"));
    }

    #[test]
    fn field_count_not_multiple_of_six_is_malformed() {
        let mut stdout = nul_join(&[&oid('a').to_string(), "", "Bob", "bob@example.com"]);
        // 只有 4 欄，不是 6 的倍數，且沒有正常結尾——不透過 nul_join 的最後補 NUL 邏輯改變，
        // 直接視為破損。
        stdout.pop(); // 拿掉最後一個 NUL，讓總 token 數看起來像 4（不含尾端空字串）
        let err = parse(stdout).expect_err("欄位數不是 6 的倍數應回明確錯誤");
        assert!(matches!(err, GitParseError::MalformedOutput(_)));
    }

    #[test]
    fn non_integer_time_is_malformed() {
        let stdout = nul_join(&[
            &oid('a').to_string(),
            "",
            "Bob",
            "bob@example.com",
            "not-a-number",
            "Subject",
        ]);
        let err = parse(stdout).expect_err("%at 不是整數應回明確錯誤");
        assert!(matches!(err, GitParseError::MalformedOutput(_)));
    }

    #[test]
    fn wrong_call_count_is_unexpected_call_count() {
        let query = Log {
            tips: vec![oid('a')],
            limit: 1,
        };
        let err = query
            .parse(&[])
            .expect_err("呼叫次數不是 1 應回 UnexpectedCallCount");
        assert_eq!(err, GitParseError::UnexpectedCallCount);
    }

    #[test]
    fn call_failure_maps_to_unexpected_call_outcome() {
        let query = Log {
            tips: vec![oid('a')],
            limit: 1,
        };
        let calls: Vec<Result<CallOutcome, RunnerError>> = vec![Err(RunnerError::Failed {
            exit_code: Some(128),
            stderr_tail: Vec::new(),
        })];
        let err = query.parse(&calls).expect_err("log 呼叫失敗應轉成解析錯誤");
        assert!(matches!(err, GitParseError::UnexpectedCallOutcome(_)));
    }
}
