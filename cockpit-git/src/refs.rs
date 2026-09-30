//! `Refs` 查詢的輸出型別與解析器（git-review design D4「Refs」、spec「refs 端點」）。
//!
//! 三次呼叫（design D4）：`for-each-ref`（列出 `refs/heads`／`refs/remotes`／`refs/tags`）、
//! `rev-parse --verify -q HEAD`、`symbolic-ref -q HEAD`。後兩者用 `-q` 讓「合法地沒有值」
//! （還沒有任何 commit／分離 HEAD）以非零結束、無 stdout——這是 task 2.2 報告「架構決定 2」
//! 指名要 task 2.3 處理的部分：這裡把這兩次呼叫的 `RunnerError::Failed`（且不是其他錯誤
//! 變體）解讀成「沒有值」而不是真正的失敗。

use crate::parse_error::GitParseError;
use crate::query::Refs;
use crate::runner::{CallOutcome, RunnerError};

/// spec「refs 端點」的 `head` 欄位。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RefsHead {
    pub oid: Option<String>,
    pub git_ref: Option<String>,
}

/// spec「refs 端點」`refs` 的 `kind`。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefKind {
    Branch,
    Remote,
    Tag,
}

/// spec「refs 端點」`refs` 的一筆。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefEntry {
    pub name: String,
    pub short: String,
    pub kind: RefKind,
    pub oid: String,
}

/// `Refs` 查詢的完整輸出。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RefsOutput {
    pub head: RefsHead,
    pub refs: Vec<RefEntry>,
}

impl Refs {
    /// 解析 [`Refs::commands`] 的 3 次呼叫（design D4）。
    pub fn parse(
        &self,
        calls: &[Result<CallOutcome, RunnerError>],
    ) -> Result<RefsOutput, GitParseError> {
        if calls.len() != 3 {
            return Err(GitParseError::UnexpectedCallCount);
        }
        let for_each_ref = calls[0]
            .as_ref()
            .map_err(|err| GitParseError::UnexpectedCallOutcome(err.to_string()))?;
        let oid = optional_single_line(&calls[1])?;
        let git_ref = optional_single_line(&calls[2])?;

        let refs = parse_for_each_ref(&for_each_ref.stdout)?;

        Ok(RefsOutput {
            head: RefsHead { oid, git_ref },
            refs,
        })
    }
}

/// `rev-parse --verify -q HEAD` 與 `symbolic-ref -q HEAD` 共用的解析：`-q` 讓「合法地沒有
/// 值」以非零結束、無 stdout；成功時單行輸出（含結尾換行，`trim` 去掉）。
fn optional_single_line(
    call: &Result<CallOutcome, RunnerError>,
) -> Result<Option<String>, GitParseError> {
    match call {
        Ok(outcome) => {
            let text = String::from_utf8_lossy(&outcome.stdout);
            let trimmed = text.trim();
            if trimmed.is_empty() {
                Ok(None)
            } else {
                Ok(Some(trimmed.to_string()))
            }
        }
        // `-q` 讓「沒有值」以非零結束、stderr 為空——這正是這裡預期會看到的「已知合法失敗」
        // （task 2.2 報告「架構決定 2」）。其餘錯誤變體（不該發生：這兩個呼叫沒有大輸出，
        // `Unavailable`／`Untrusted`／`Timeout` 也只會在第一次呼叫就讓整個 `RunOutput`
        // 提早回 `Err`，走不到這裡）一律視為非預期，回明確錯誤而非假裝成「沒有值」。
        Err(RunnerError::Failed { .. }) => Ok(None),
        Err(other) => Err(GitParseError::UnexpectedCallOutcome(other.to_string())),
    }
}

fn parse_for_each_ref(stdout: &[u8]) -> Result<Vec<RefEntry>, GitParseError> {
    let mut refs = Vec::new();
    for line in stdout.split(|&b| b == b'\n') {
        if line.is_empty() {
            continue;
        }
        let Ok(line) = std::str::from_utf8(line) else {
            // refname 理論上不會出現非法 UTF-8；穩健起見略過而非讓整個 Refs 解析失敗
            // （refs 端點沒有 `skipped` 欄位可回報，靜默跳過）。
            continue;
        };
        let fields: Vec<&str> = line.split('\0').collect();
        if fields.len() != 4 {
            return Err(GitParseError::MalformedOutput(
                "for-each-ref 一行的欄位數不是 4".to_string(),
            ));
        }
        let (name, objectname, peeled, symref) = (fields[0], fields[1], fields[2], fields[3]);

        if !symref.is_empty() {
            // 符號 ref（`refs/remotes/<remote>/HEAD` 這類）：略過（design D4／brief）。
            continue;
        }

        let (short, kind) = if let Some(rest) = name.strip_prefix("refs/heads/") {
            (rest, RefKind::Branch)
        } else if let Some(rest) = name.strip_prefix("refs/remotes/") {
            (rest, RefKind::Remote)
        } else if let Some(rest) = name.strip_prefix("refs/tags/") {
            (rest, RefKind::Tag)
        } else {
            // 不在三個命名空間內：理論上 for-each-ref 的 pattern 已經限制過，這裡是防禦。
            continue;
        };

        // 附註 tag 取剝皮後的 commit（design D4／spec）；輕量 tag 與分支／遠端分支的
        // `*objectname` 恆為空，直接用 `objectname`。
        let oid = if peeled.is_empty() {
            objectname
        } else {
            peeled
        };

        refs.push(RefEntry {
            name: name.to_string(),
            short: short.to_string(),
            kind,
            oid: oid.to_string(),
        });
    }
    Ok(refs)
}

#[cfg(test)]
mod tests {
    use super::{RefKind, RefsOutput};
    use crate::GitParseError;
    use crate::query::Refs;
    use crate::runner::{CallOutcome, RunnerError};

    fn line(fields: &[&str]) -> String {
        fields.join("\0")
    }

    fn calls(
        for_each_ref: Vec<u8>,
        rev_parse: Result<Vec<u8>, ()>,
        symbolic_ref: Result<Vec<u8>, ()>,
    ) -> Vec<Result<CallOutcome, RunnerError>> {
        let to_call = |r: Result<Vec<u8>, ()>| match r {
            Ok(stdout) => Ok(CallOutcome {
                stdout,
                truncated: false,
            }),
            Err(()) => Err(RunnerError::Failed {
                exit_code: Some(1),
                stderr_tail: Vec::new(),
            }),
        };
        vec![
            Ok(CallOutcome {
                stdout: for_each_ref,
                truncated: false,
            }),
            to_call(rev_parse),
            to_call(symbolic_ref),
        ]
    }

    const MAIN_OID: &str = "1111111111111111111111111111111111111111";
    const FEAT_OID: &str = "2222222222222222222222222222222222222222";
    const TAG_OBJ_OID: &str = "3333333333333333333333333333333333333333";
    const TAG_COMMIT_OID: &str = "4444444444444444444444444444444444444444";

    fn parse(
        for_each_ref: Vec<u8>,
        rev_parse: Result<Vec<u8>, ()>,
        symbolic_ref: Result<Vec<u8>, ()>,
    ) -> Result<RefsOutput, GitParseError> {
        Refs.parse(&calls(for_each_ref, rev_parse, symbolic_ref))
    }

    /// spec refs 端點 Scenario「分支、遠端與 tag」。
    #[test]
    fn parses_branches_remote_and_annotated_tag_scenario_from_spec() {
        let for_each_ref = [
            line(&["refs/heads/main", MAIN_OID, "", ""]),
            line(&["refs/heads/feat/x", FEAT_OID, "", ""]),
            line(&["refs/remotes/origin/main", MAIN_OID, "", ""]),
            // 遠端 HEAD 的 symref：應被略過。
            line(&[
                "refs/remotes/origin/HEAD",
                MAIN_OID,
                "",
                "refs/remotes/origin/main",
            ]),
            line(&["refs/tags/v0.1", TAG_OBJ_OID, TAG_COMMIT_OID, ""]),
        ]
        .join("\n")
            + "\n";

        let output = parse(
            for_each_ref.into_bytes(),
            Ok(format!("{MAIN_OID}\n").into_bytes()),
            Ok(b"refs/heads/main\n".to_vec()),
        )
        .expect("應解析成功");

        assert_eq!(output.refs.len(), 4, "遠端 HEAD 的 symref 不應出現");
        assert_eq!(output.head.oid.as_deref(), Some(MAIN_OID));
        assert_eq!(output.head.git_ref.as_deref(), Some("refs/heads/main"));

        let find = |name: &str| {
            output
                .refs
                .iter()
                .find(|r| r.name == name)
                .unwrap_or_else(|| panic!("找不到 {name}"))
        };
        assert_eq!(find("refs/heads/main").kind, RefKind::Branch);
        assert_eq!(find("refs/heads/main").short, "main");
        assert_eq!(find("refs/heads/feat/x").kind, RefKind::Branch);
        assert_eq!(find("refs/heads/feat/x").short, "feat/x");
        assert_eq!(find("refs/remotes/origin/main").kind, RefKind::Remote);
        assert_eq!(find("refs/remotes/origin/main").short, "origin/main");
        let tag = find("refs/tags/v0.1");
        assert_eq!(tag.kind, RefKind::Tag);
        assert_eq!(tag.short, "v0.1");
        assert_eq!(tag.oid, TAG_COMMIT_OID, "附註 tag 應取剝皮後的 commit");

        assert!(
            !output.refs.iter().any(|r| r.name.ends_with("/HEAD")),
            "不應有任何 symref 項目"
        );
    }

    #[test]
    fn no_commits_yet_and_detached_head_yield_none() {
        let output = parse(Vec::new(), Err(()), Err(())).expect("應解析成功");
        assert_eq!(output.head.oid, None);
        assert_eq!(output.head.git_ref, None);
        assert!(output.refs.is_empty());
    }

    #[test]
    fn wrong_call_count_is_unexpected_call_count() {
        let err = Refs
            .parse(&[])
            .expect_err("呼叫次數不是 3 應回 UnexpectedCallCount");
        assert_eq!(err, GitParseError::UnexpectedCallCount);
    }

    #[test]
    fn for_each_ref_call_failure_maps_to_unexpected_call_outcome() {
        let calls: Vec<Result<CallOutcome, RunnerError>> = vec![
            Err(RunnerError::Failed {
                exit_code: Some(128),
                stderr_tail: Vec::new(),
            }),
            Ok(CallOutcome {
                stdout: Vec::new(),
                truncated: false,
            }),
            Ok(CallOutcome {
                stdout: Vec::new(),
                truncated: false,
            }),
        ];
        let err = Refs
            .parse(&calls)
            .expect_err("for-each-ref 失敗應轉成解析錯誤");
        assert!(matches!(err, GitParseError::UnexpectedCallOutcome(_)));
    }

    #[test]
    fn non_utf8_refname_line_is_skipped_silently() {
        let mut for_each_ref = Vec::new();
        for_each_ref.extend_from_slice(b"refs/heads/\xFFbad\0");
        for_each_ref.extend_from_slice(MAIN_OID.as_bytes());
        for_each_ref.extend_from_slice(b"\0\0\0\n");
        for_each_ref.extend_from_slice(line(&["refs/heads/ok", FEAT_OID, "", ""]).as_bytes());
        for_each_ref.push(b'\n');

        let output = parse(for_each_ref, Err(()), Err(())).expect("非法 UTF-8 應被略過而非報錯");
        assert_eq!(output.refs.len(), 1);
        assert_eq!(output.refs[0].name, "refs/heads/ok");
    }

    #[test]
    fn malformed_field_count_is_error() {
        let for_each_ref = b"refs/heads/main\0only-two-fields\n".to_vec();
        let err = parse(for_each_ref, Err(()), Err(())).expect_err("欄位數不符應回明確錯誤");
        assert!(matches!(err, GitParseError::MalformedOutput(_)));
    }
}
