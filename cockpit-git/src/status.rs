//! `Status` 查詢的輸出型別與解析器（git-review design D4「Status」、spec「狀態端點」、
//! task 2.3 brief）。
//!
//! 輸入是 `status --porcelain=v2 -z --branch --untracked-files=all` 的原始 stdout
//! （NUL 分隔：`-z` 讓每個標頭欄位與每筆紀錄各自是一個 NUL 結尾的 token）。純函式：不啟動
//! git，測試全部以 `git-review-probe.md` ④ 的真實輸出或依 porcelain v2 格式自行構造的位元組
//! 字面值為 fixture。

use crate::parse_error::GitParseError;
use crate::parse_support::single_call;
use crate::query::Status;
use crate::runner::{CallOutcome, RunnerError};

/// spec「狀態端點」的 `branch` 欄位。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StatusBranch {
    pub head: Option<String>,
    pub oid: Option<String>,
    pub upstream: Option<String>,
    pub ahead: Option<u32>,
    pub behind: Option<u32>,
}

/// spec「狀態端點」`entries` 的 `group`。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusGroup {
    Conflict,
    Staged,
    Unstaged,
    Untracked,
}

/// spec「狀態端點」`entries` 的一筆。**不含 `icon`**——brief 的控制端裁決：`icon` 由
/// `cockpit` 用 5a 的 icon 對照補上，不在這個 crate 的職責內。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusEntry {
    pub path: String,
    pub old_path: Option<String>,
    pub group: StatusGroup,
    pub status: char,
}

/// `Status` 查詢的完整輸出。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StatusOutput {
    pub branch: StatusBranch,
    pub entries: Vec<StatusEntry>,
    pub truncated: bool,
    pub skipped: u32,
}

impl Status {
    /// 解析 [`Status::commands`] 的唯一一次呼叫（design D4）。
    pub fn parse(
        &self,
        calls: &[Result<CallOutcome, RunnerError>],
    ) -> Result<StatusOutput, GitParseError> {
        let outcome = single_call(calls)?;
        parse_status_bytes(&outcome.stdout, outcome.truncated)
    }
}

fn parse_status_bytes(stdout: &[u8], truncated: bool) -> Result<StatusOutput, GitParseError> {
    let mut tokens: Vec<&[u8]> = stdout.split(|&b| b == 0).collect();
    // `-z` 讓每個標頭欄位與每筆紀錄都以 NUL 結尾，所以完整輸出 split 後最後一段恆為空字串；
    // 截斷時最後一段可能落在紀錄或欄位中間（不保證是完整紀錄），一律捨棄（design：「回傳已
    // 完整讀取的前段」，寧可少一筆也不要回傳半筆）。
    if let Some(last) = tokens.last()
        && (truncated || last.is_empty())
    {
        tokens.pop();
    }

    let mut branch = StatusBranch::default();
    let mut entries = Vec::new();
    let mut skipped: u32 = 0;

    let mut i = 0;
    while i < tokens.len() {
        let token = tokens[i];
        if token.starts_with(b"# ") {
            parse_header_line(&token[2..], &mut branch);
            i += 1;
            continue;
        }
        match token.first() {
            Some(&b'1') => {
                match parse_ordinary_record(token)? {
                    Some(new_entries) => entries.extend(new_entries),
                    None => skipped += 1,
                }
                i += 1;
            }
            Some(&b'2') => {
                let Some(&orig) = tokens.get(i + 1) else {
                    if truncated {
                        break;
                    }
                    return Err(GitParseError::MalformedOutput(
                        "`2` 改名／複製紀錄缺少原路徑欄位".to_string(),
                    ));
                };
                match parse_rename_record(token, orig)? {
                    Some(new_entries) => entries.extend(new_entries),
                    None => skipped += 1,
                }
                i += 2;
            }
            Some(&b'u') => {
                match parse_conflict_record(token)? {
                    Some(entry) => entries.push(entry),
                    None => skipped += 1,
                }
                i += 1;
            }
            Some(&b'?') => {
                match parse_untracked_record(token)? {
                    Some(entry) => entries.push(entry),
                    None => skipped += 1,
                }
                i += 1;
            }
            _ => {
                // 未知的紀錄型別（含 `!`（忽略檔案，未請求時不該出現）與其他未來新增型別）：
                // 略過而非報錯（design 風險欄「git 的 porcelain 格式在不同版本的差異」）。
                i += 1;
            }
        }
    }

    Ok(StatusOutput {
        branch,
        entries,
        truncated,
        skipped,
    })
}

fn parse_header_line(rest: &[u8], branch: &mut StatusBranch) {
    let Ok(line) = std::str::from_utf8(rest) else {
        return; // 標頭理論上恆為 ASCII；不是就當成不認得的標頭略過
    };
    if let Some(value) = line.strip_prefix("branch.oid ") {
        branch.oid = if value == "(initial)" {
            None
        } else {
            Some(value.to_string())
        };
    } else if let Some(value) = line.strip_prefix("branch.head ") {
        branch.head = if value == "(detached)" {
            None
        } else {
            Some(value.to_string())
        };
    } else if let Some(value) = line.strip_prefix("branch.upstream ") {
        branch.upstream = Some(value.to_string());
    } else if let Some(value) = line.strip_prefix("branch.ab ")
        && let Some((ahead, behind)) = parse_ahead_behind(value)
    {
        branch.ahead = Some(ahead);
        branch.behind = Some(behind);
    }
    // 其他未知的標頭鍵（目前 porcelain v2 只定義這 4 種）略過。
}

fn parse_ahead_behind(value: &str) -> Option<(u32, u32)> {
    // "+<ahead> -<behind>"
    let mut parts = value.split(' ');
    let ahead = parts.next()?.strip_prefix('+')?.parse().ok()?;
    let behind = parts.next()?.strip_prefix('-')?.parse().ok()?;
    Some((ahead, behind))
}

/// 在每個 hunk 內把 XY 兩個字元各自轉成 0～2 筆 [`StatusEntry`]（design：「同一個檔案同時有
/// 已暫存與未暫存的修改時，兩組各出現一筆」）。
fn split_xy_to_entries(x: u8, y: u8, path: String, old_path: Option<String>) -> Vec<StatusEntry> {
    let mut out = Vec::new();
    if x != b'.' {
        out.push(StatusEntry {
            path: path.clone(),
            old_path: old_path.clone(),
            group: StatusGroup::Staged,
            status: x as char,
        });
    }
    if y != b'.' {
        out.push(StatusEntry {
            path,
            old_path,
            group: StatusGroup::Unstaged,
            status: y as char,
        });
    }
    out
}

/// 把一個 token 從頭切成 `n` 個以空白分隔的欄位，前 `n-1` 個欄位在第一個空白處截斷，
/// 最後一個欄位是剩下的全部位元組（可能仍含空白——這是路徑欄位允許含空白的關鍵）。
/// 空白不足 `n-1` 個時回傳 `None`（欄位數不符）。
fn split_fixed(input: &[u8], n: usize) -> Option<Vec<&[u8]>> {
    let mut parts = Vec::with_capacity(n);
    let mut rest = input;
    for _ in 0..n - 1 {
        let pos = rest.iter().position(|&b| b == b' ')?;
        parts.push(&rest[..pos]);
        rest = &rest[pos + 1..];
    }
    parts.push(rest);
    Some(parts)
}

fn utf8_or_none(bytes: &[u8]) -> Option<String> {
    std::str::from_utf8(bytes).ok().map(|s| s.to_string())
}

fn parse_ordinary_record(token: &[u8]) -> Result<Option<Vec<StatusEntry>>, GitParseError> {
    // "1 XY sub mH mI mW hH hI path" = 9 欄
    let parts = split_fixed(token, 9)
        .ok_or_else(|| GitParseError::MalformedOutput("`1` 紀錄欄位數不足".to_string()))?;
    let xy = parts[1];
    if xy.len() != 2 {
        return Err(GitParseError::MalformedOutput(
            "`1` 紀錄的 XY 欄位長度不是 2".to_string(),
        ));
    }
    let Some(path) = utf8_or_none(parts[8]) else {
        return Ok(None);
    };
    Ok(Some(split_xy_to_entries(xy[0], xy[1], path, None)))
}

fn parse_rename_record(
    token: &[u8],
    orig_token: &[u8],
) -> Result<Option<Vec<StatusEntry>>, GitParseError> {
    // "2 XY sub mH mI mW hH hI Xscore path" = 10 欄
    let parts = split_fixed(token, 10)
        .ok_or_else(|| GitParseError::MalformedOutput("`2` 紀錄欄位數不足".to_string()))?;
    let xy = parts[1];
    if xy.len() != 2 {
        return Err(GitParseError::MalformedOutput(
            "`2` 紀錄的 XY 欄位長度不是 2".to_string(),
        ));
    }
    let (Some(path), Some(old_path)) = (utf8_or_none(parts[9]), utf8_or_none(orig_token)) else {
        return Ok(None);
    };
    Ok(Some(split_xy_to_entries(
        xy[0],
        xy[1],
        path,
        Some(old_path),
    )))
}

fn parse_conflict_record(token: &[u8]) -> Result<Option<StatusEntry>, GitParseError> {
    // "u XY sub m1 m2 m3 mW h1 h2 h3 path" = 11 欄
    let parts = split_fixed(token, 11)
        .ok_or_else(|| GitParseError::MalformedOutput("`u` 紀錄欄位數不足".to_string()))?;
    let Some(path) = utf8_or_none(parts[10]) else {
        return Ok(None);
    };
    // 衝突細分類型（UU／AA／DD／AU…）不影響 spec 定義的字母集合（只有一個 `U`），一律用 'U'
    // （brief 決策：spec 的 status 字母集合裡衝突只有這一個字母）。
    Ok(Some(StatusEntry {
        path,
        old_path: None,
        group: StatusGroup::Conflict,
        status: 'U',
    }))
}

fn parse_untracked_record(token: &[u8]) -> Result<Option<StatusEntry>, GitParseError> {
    // "? path" = 2 欄
    let parts = split_fixed(token, 2)
        .ok_or_else(|| GitParseError::MalformedOutput("`?` 紀錄欄位數不足".to_string()))?;
    let Some(path) = utf8_or_none(parts[1]) else {
        return Ok(None);
    };
    Ok(Some(StatusEntry {
        path,
        old_path: None,
        group: StatusGroup::Untracked,
        status: '?',
    }))
}

#[cfg(test)]
mod tests {
    use super::{StatusGroup, StatusOutput};
    use crate::query::Status;
    use crate::runner::{CallOutcome, RunnerError};
    use crate::{GitParseError, GitQuery, GitTarget};

    /// 依 porcelain v2 -z 的格式（每個標頭欄位／紀錄各自一個 NUL 結尾 token）組出 fixture；
    /// 逐條 push 再統一補 NUL，模擬真實輸出恆以 NUL 結尾。
    fn nul_join(fields: &[&str]) -> Vec<u8> {
        let mut out = Vec::new();
        for f in fields {
            out.extend_from_slice(f.as_bytes());
            out.push(0);
        }
        out
    }

    fn ok_call(stdout: Vec<u8>, truncated: bool) -> Vec<Result<CallOutcome, RunnerError>> {
        vec![Ok(CallOutcome { stdout, truncated })]
    }

    fn parse(stdout: Vec<u8>, truncated: bool) -> Result<StatusOutput, GitParseError> {
        Status.parse(&ok_call(stdout, truncated))
    }

    const OID_A: &str = "1111111111111111111111111111111111111111";

    /// spec 狀態端點 Scenario「各組變更」。
    #[test]
    fn parses_each_group_scenario_from_spec() {
        let stdout = nul_join(&[
            &format!("# branch.oid {OID_A}"),
            "# branch.head main",
            "1 M. N... 100644 100644 100644 aaaa aaaa a.rs",
            "1 .M N... 100644 100644 100644 bbbb bbbb b.rs",
            "1 MM N... 100644 100644 100644 cccc cccc c.rs",
            "2 R. N... 100644 100644 100644 dddd eeee R100 renamed.txt",
            "old.txt",
            "? new.md",
        ]);

        let output = parse(stdout, false).expect("應解析成功");

        assert_eq!(output.branch.head.as_deref(), Some("main"));
        assert_eq!(output.branch.oid.as_deref(), Some(OID_A));
        assert!(!output.truncated);
        assert_eq!(output.skipped, 0);

        let find = |path: &str, group: StatusGroup| {
            output
                .entries
                .iter()
                .find(|e| e.path == path && e.group == group)
                .unwrap_or_else(|| panic!("找不到 {path} 的 {group:?} 項目"))
        };

        assert_eq!(find("a.rs", StatusGroup::Staged).status, 'M');
        assert_eq!(find("b.rs", StatusGroup::Unstaged).status, 'M');
        assert_eq!(find("c.rs", StatusGroup::Staged).status, 'M');
        assert_eq!(find("c.rs", StatusGroup::Unstaged).status, 'M');
        let new_md = find("new.md", StatusGroup::Untracked);
        assert_eq!(new_md.status, '?');
        assert_eq!(new_md.old_path, None);
        let renamed = find("renamed.txt", StatusGroup::Staged);
        assert_eq!(renamed.status, 'R');
        assert_eq!(renamed.old_path.as_deref(), Some("old.txt"));

        assert_eq!(output.entries.len(), 6);
    }

    /// spec 狀態端點 Scenario「合併衝突」。
    #[test]
    fn parses_merge_conflict_scenario_from_spec() {
        let stdout = nul_join(&[
            &format!("# branch.oid {OID_A}"),
            "# branch.head main",
            "u UU N... 100644 100644 100644 100644 base ours theirs conflict.txt",
        ]);

        let output = parse(stdout, false).expect("應解析成功");

        assert_eq!(output.entries.len(), 1);
        let entry = &output.entries[0];
        assert_eq!(entry.path, "conflict.txt");
        assert_eq!(entry.group, StatusGroup::Conflict);
        assert_eq!(entry.status, 'U');
        assert_eq!(entry.old_path, None);
    }

    /// spec 狀態端點 Scenario「還沒有 commit 的 repo」。
    #[test]
    fn parses_no_commits_yet_scenario_from_spec() {
        let stdout = nul_join(&[
            "# branch.oid (initial)",
            "# branch.head main",
            "1 A. N... 000000 100644 100644 0000000000000000000000000000000000000000 aaaa a.txt",
        ]);

        let output = parse(stdout, false).expect("應解析成功");

        assert_eq!(output.branch.oid, None);
        assert_eq!(output.entries.len(), 1);
        assert_eq!(output.entries[0].path, "a.txt");
        assert_eq!(output.entries[0].group, StatusGroup::Staged);
        assert_eq!(output.entries[0].status, 'A');
    }

    #[test]
    fn detached_head_maps_to_none() {
        let stdout = nul_join(&[&format!("# branch.oid {OID_A}"), "# branch.head (detached)"]);
        let output = parse(stdout, false).expect("應解析成功");
        assert_eq!(output.branch.head, None);
        assert_eq!(output.branch.oid.as_deref(), Some(OID_A));
    }

    #[test]
    fn upstream_and_ahead_behind_are_parsed() {
        let stdout = nul_join(&[
            &format!("# branch.oid {OID_A}"),
            "# branch.head main",
            "# branch.upstream origin/main",
            "# branch.ab +2 -1",
        ]);
        let output = parse(stdout, false).expect("應解析成功");
        assert_eq!(output.branch.upstream.as_deref(), Some("origin/main"));
        assert_eq!(output.branch.ahead, Some(2));
        assert_eq!(output.branch.behind, Some(1));
    }

    #[test]
    fn no_upstream_leaves_ahead_behind_none() {
        let stdout = nul_join(&[&format!("# branch.oid {OID_A}"), "# branch.head main"]);
        let output = parse(stdout, false).expect("應解析成功");
        assert_eq!(output.branch.upstream, None);
        assert_eq!(output.branch.ahead, None);
        assert_eq!(output.branch.behind, None);
    }

    #[test]
    fn unknown_record_prefix_is_skipped_not_error() {
        let mut stdout = nul_join(&[
            &format!("# branch.oid {OID_A}"),
            "# branch.head main",
            "1 M. N... 100644 100644 100644 aaaa aaaa a.rs",
        ]);
        // 手動附加一筆 `!`（忽略檔案）型別的紀錄：未請求 `--ignored` 時理論上不會出現，
        // 但解析器對任何未知前綴都該略過而非報錯（design 風險欄）。
        stdout.extend_from_slice(b"! ignored.txt\0");

        let output = parse(stdout, false).expect("未知型別不應讓整體解析失敗");
        assert_eq!(output.entries.len(), 1);
        assert_eq!(output.entries[0].path, "a.rs");
    }

    #[test]
    fn non_utf8_path_is_skipped_and_counted() {
        let mut stdout = nul_join(&[&format!("# branch.oid {OID_A}"), "# branch.head main"]);
        // 手動構造一筆路徑含非法 UTF-8 位元組（0xFF）的未追蹤紀錄。
        stdout.extend_from_slice(b"? bad_\xFF_path.txt\0");
        stdout.extend_from_slice(b"? ok.txt\0");

        let output = parse(stdout, false).expect("應解析成功（非法路徑略過而非整體失敗）");
        assert_eq!(output.skipped, 1);
        assert_eq!(output.entries.len(), 1);
        assert_eq!(output.entries[0].path, "ok.txt");
    }

    #[test]
    fn non_utf8_path_in_rename_record_is_skipped_and_counted() {
        let mut stdout = nul_join(&[&format!("# branch.oid {OID_A}"), "# branch.head main"]);
        stdout.extend_from_slice(b"2 R. N... 100644 100644 100644 a b R100 bad_\xFF_new.txt\0");
        stdout.extend_from_slice(b"old.txt\0");

        let output = parse(stdout, false).expect("應解析成功");
        assert_eq!(output.skipped, 1);
        assert_eq!(output.entries.len(), 0);
    }

    #[test]
    fn truncated_drops_incomplete_trailing_rename_pair() {
        let mut stdout = nul_join(&[
            &format!("# branch.oid {OID_A}"),
            "# branch.head main",
            "1 M. N... 100644 100644 100644 aaaa aaaa a.rs",
        ]);
        // 截斷發生在改名紀錄的原路徑欄位「之前」：把結尾的 NUL 拿掉，模擬 read_capped
        // 在任意位元組位置停止（不保證落在紀錄邊界）。
        stdout.extend_from_slice(b"2 R. N... 100644 100644 100644 a b R100 renamed.txt\0old");
        // 注意：這裡故意不補結尾 NUL。

        let output = parse(stdout, true).expect("截斷不完整紀錄應被捨棄，不是錯誤");
        assert!(output.truncated);
        assert_eq!(output.entries.len(), 1, "只有完整的 a.rs 紀錄應被保留");
        assert_eq!(output.entries[0].path, "a.rs");
    }

    #[test]
    fn truncated_with_missing_rename_pair_token_is_dropped_not_errored() {
        // 更極端：截斷正好落在「2」紀錄的 NUL 之後，但配對的原路徑 token 完全不存在
        // （被截斷掉，split 之後找不到下一個 token）。
        let mut stdout = nul_join(&[
            &format!("# branch.oid {OID_A}"),
            "# branch.head main",
            "1 M. N... 100644 100644 100644 aaaa aaaa a.rs",
        ]);
        stdout.extend_from_slice(b"2 R. N... 100644 100644 100644 a b R100 renamed.txt\0");

        let output = parse(stdout, true).expect("截斷不完整紀錄應被捨棄，不是錯誤");
        assert_eq!(output.entries.len(), 1);
        assert_eq!(output.entries[0].path, "a.rs");
    }

    #[test]
    fn malformed_ordinary_record_without_truncation_is_error() {
        let stdout = nul_join(&[
            &format!("# branch.oid {OID_A}"),
            "# branch.head main",
            "1 M. N... 100644 a.rs", // 欄位數不足（缺 mI/mW/hH/hI）
        ]);

        let err = parse(stdout, false).expect_err("欄位數不符應回明確錯誤而非 panic");
        assert!(matches!(err, GitParseError::MalformedOutput(_)));
    }

    #[test]
    fn missing_rename_pair_without_truncation_is_error() {
        let stdout = nul_join(&[
            &format!("# branch.oid {OID_A}"),
            "# branch.head main",
            "2 R. N... 100644 100644 100644 a b R100 renamed.txt",
            // 沒有下一個 token 可以當原路徑，且 truncated=false，應視為格式破損。
        ]);

        let err = parse(stdout, false).expect_err("非截斷情況下缺少原路徑欄位應是錯誤");
        assert!(matches!(err, GitParseError::MalformedOutput(_)));
    }

    #[test]
    fn wrong_call_count_is_unexpected_call_count() {
        let err = Status
            .parse(&[])
            .expect_err("呼叫次數不是 1 應回 UnexpectedCallCount");
        assert_eq!(err, GitParseError::UnexpectedCallCount);
    }

    #[test]
    fn call_failure_maps_to_unexpected_call_outcome() {
        let calls: Vec<Result<CallOutcome, RunnerError>> = vec![Err(RunnerError::Failed {
            exit_code: Some(128),
            stderr_tail: Vec::new(),
        })];
        let err = Status
            .parse(&calls)
            .expect_err("status 呼叫失敗應轉成解析錯誤");
        assert!(matches!(err, GitParseError::UnexpectedCallOutcome(_)));
    }

    /// `Status::commands` 產生的 argv 與這裡的解析器對得上（同一個型別、同一份 design D4）。
    #[test]
    fn status_commands_still_matches_the_subcommand_this_parser_expects() {
        let target = GitTarget::Native {
            path: r"C:\repo".to_string(),
        };
        let calls = Status.commands(&target);
        assert_eq!(calls.len(), 1);
        assert!(calls[0].iter().any(|a| a == "--porcelain=v2"));
    }
}
