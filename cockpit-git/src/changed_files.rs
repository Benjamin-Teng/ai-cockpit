//! `ChangedFiles` 查詢的輸出型別與解析器（git-review design D4「ChangedFiles」、spec「變更
//! 檔案清單端點」、task 1.2 probe ⑤⑥）。
//!
//! 兩次呼叫（`--name-status`／`--numstat`，probe ⑤：兩者不能同一次輸出）以路徑合併：
//! `--name-status` 決定檔案集合、狀態字母與改名原路徑；`--numstat` 只補增刪行數，缺漏
//! （兩次呼叫之間工作區改變、或該路徑不在 numstat 輸出中）時為 `None`（design D4 備註）。
//! `diff-tree -r --root`（`Empty → Oid` 根 commit）在檔案列之前多印一行 commit hash
//! （probe ⑥），兩次呼叫都要跳過這一行。

use std::collections::HashMap;

use crate::parse_error::GitParseError;
use crate::query::ChangedFiles;
use crate::runner::{CallOutcome, RunnerError};

/// spec「變更檔案清單端點」`files` 的一筆。不含 `icon`（同 [`crate::StatusEntry`] 的控制端
/// 裁決：由 `cockpit` 補上）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangedFile {
    pub status: char,
    pub path: String,
    pub old_path: Option<String>,
    pub additions: Option<u64>,
    pub deletions: Option<u64>,
}

/// `ChangedFiles` 查詢的輸出。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ChangedFilesOutput {
    pub files: Vec<ChangedFile>,
    pub truncated: bool,
    pub skipped: u32,
}

impl ChangedFiles {
    /// 解析 [`ChangedFiles::commands`] 的兩次呼叫（design D4：`calls[0]` 是
    /// `--name-status`、`calls[1]` 是 `--numstat`——順序與 `commands()` 回傳的順序一致）。
    pub fn parse(
        &self,
        calls: &[Result<CallOutcome, RunnerError>],
    ) -> Result<ChangedFilesOutput, GitParseError> {
        if calls.len() != 2 {
            return Err(GitParseError::UnexpectedCallCount);
        }
        let name_status = calls[0]
            .as_ref()
            .map_err(|err| GitParseError::UnexpectedCallOutcome(err.to_string()))?;
        let numstat = calls[1]
            .as_ref()
            .map_err(|err| GitParseError::UnexpectedCallOutcome(err.to_string()))?;

        let is_diff_tree = self.family_subcommand() == "diff-tree";

        let (records, skipped) =
            parse_name_status(&name_status.stdout, name_status.truncated, is_diff_tree)?;
        let counts = parse_numstat(&numstat.stdout, numstat.truncated, is_diff_tree)?;

        // Ruling R13：`diff-files` 不刷新 stat cache（刻意的：刷新會重寫 `.git/index`），所以
        // 「只 touch、內容沒變」的檔案在 `--name-status` 仍被誤報成 `M`；但 `--numstat` 會真的
        // 比對內容，不列出它們（純權限變更則會列成 `0\t0\t<path>`，不會被誤刪）。因此狀態為
        // `M` 而不在 numstat 輸出的路徑就是 stat-dirty 假陽性，丟掉。numstat 被截斷時缺漏的
        // 路徑可能只是沒讀到，不能據此丟，所以截斷時不濾。
        let drop_stat_dirty = self.family_subcommand() == "diff-files" && !numstat.truncated;
        let files = records
            .into_iter()
            .filter(|r| !(drop_stat_dirty && r.status == 'M' && !counts.contains_key(&r.path)))
            .map(|r| {
                let (additions, deletions) = counts.get(&r.path).copied().unwrap_or((None, None));
                ChangedFile {
                    status: r.status,
                    path: r.path,
                    old_path: r.old_path,
                    additions,
                    deletions,
                }
            })
            .collect();

        Ok(ChangedFilesOutput {
            files,
            truncated: name_status.truncated || numstat.truncated,
            skipped,
        })
    }
}

struct NameStatusRecord {
    status: char,
    path: String,
    old_path: Option<String>,
}

fn split_nul_tokens(stdout: &[u8], truncated: bool) -> Vec<&[u8]> {
    let mut tokens: Vec<&[u8]> = stdout.split(|&b| b == 0).collect();
    if let Some(last) = tokens.last()
        && (truncated || last.is_empty())
    {
        tokens.pop();
    }
    tokens
}

fn parse_name_status(
    stdout: &[u8],
    truncated: bool,
    is_diff_tree: bool,
) -> Result<(Vec<NameStatusRecord>, u32), GitParseError> {
    let tokens = split_nul_tokens(stdout, truncated);

    let mut i = if is_diff_tree {
        // `diff-tree -r --root` 在檔案列之前多印一行 commit hash（probe ⑥），跳過；
        // 若截斷到連這行都不完整（tokens 為空），視為沒有任何檔案列可讀。
        if tokens.is_empty() {
            return Ok((Vec::new(), 0));
        }
        1
    } else {
        0
    };

    let mut records = Vec::new();
    let mut skipped = 0u32;
    while i < tokens.len() {
        let token = tokens[i];
        if token.is_empty() {
            i += 1;
            continue;
        }
        let Ok(status_str) = std::str::from_utf8(token) else {
            return Err(GitParseError::MalformedOutput(
                "name-status 的狀態欄位不是合法文字".to_string(),
            ));
        };
        let Some(status_char) = status_str.chars().next() else {
            return Err(GitParseError::MalformedOutput(
                "name-status 出現空狀態欄位".to_string(),
            ));
        };

        if status_char == 'R' || status_char == 'C' {
            let (Some(&old_path_tok), Some(&path_tok)) = (tokens.get(i + 1), tokens.get(i + 2))
            else {
                if truncated {
                    break; // 截斷在改名／複製紀錄的路徑欄位之前，捨棄這筆不完整紀錄
                }
                return Err(GitParseError::MalformedOutput(
                    "name-status 的改名／複製紀錄缺少路徑欄位".to_string(),
                ));
            };
            match (
                std::str::from_utf8(old_path_tok),
                std::str::from_utf8(path_tok),
            ) {
                (Ok(old_path), Ok(path)) => records.push(NameStatusRecord {
                    status: status_char,
                    path: path.to_string(),
                    old_path: Some(old_path.to_string()),
                }),
                _ => skipped += 1,
            }
            i += 3;
        } else {
            let Some(&path_tok) = tokens.get(i + 1) else {
                if truncated {
                    break;
                }
                return Err(GitParseError::MalformedOutput(
                    "name-status 紀錄缺少路徑欄位".to_string(),
                ));
            };
            match std::str::from_utf8(path_tok) {
                Ok(path) => records.push(NameStatusRecord {
                    status: status_char,
                    path: path.to_string(),
                    old_path: None,
                }),
                Err(_) => skipped += 1,
            }
            i += 2;
        }
    }

    Ok((records, skipped))
}

/// `--numstat -z` 的 rename／copy 紀錄格式未在 task 1.2 probe 中直接實測（probe ⑤ 只測過
/// 一般修改與二進位新增檔），這裡依 git 已知行為推導：非改名紀錄是單一 token
/// `"<added>\t<deleted>\t<path>"`；改名／複製紀錄的第三欄留空（token 為
/// `"<added>\t<deleted>\t"`），緊接著兩個獨立的 NUL 分隔 token 分別是原路徑與新路徑。
/// **這個假設沒有實測驗證，已在報告「疑慮」中標注，建議 task 2.6 以真實 repo 確認**；即使
/// 假設有誤，最壞情況只是該筆的 `additions`／`deletions` 落回 `None`（design D4 本來就允許
/// 這個備援：兩次呼叫之間工作區可能改變，numstat 缺漏的路徑本來就是 `null`）。
type NumstatCounts = HashMap<String, (Option<u64>, Option<u64>)>;

fn parse_numstat(
    stdout: &[u8],
    truncated: bool,
    is_diff_tree: bool,
) -> Result<NumstatCounts, GitParseError> {
    let tokens = split_nul_tokens(stdout, truncated);

    let mut i = if is_diff_tree {
        if tokens.is_empty() {
            return Ok(HashMap::new());
        }
        1
    } else {
        0
    };

    let mut map = HashMap::new();
    while i < tokens.len() {
        let token = tokens[i];
        if token.is_empty() {
            i += 1;
            continue;
        }
        let Ok(text) = std::str::from_utf8(token) else {
            return Err(GitParseError::MalformedOutput(
                "numstat 紀錄不是合法文字".to_string(),
            ));
        };
        let mut fields = text.splitn(3, '\t');
        let (Some(added), Some(deleted), Some(third)) =
            (fields.next(), fields.next(), fields.next())
        else {
            return Err(GitParseError::MalformedOutput(
                "numstat 紀錄欄位數不足".to_string(),
            ));
        };

        if third.is_empty() {
            let (Some(&_old_path_tok), Some(&new_path_tok)) =
                (tokens.get(i + 1), tokens.get(i + 2))
            else {
                if truncated {
                    break;
                }
                return Err(GitParseError::MalformedOutput(
                    "numstat 的改名／複製紀錄缺少路徑欄位".to_string(),
                ));
            };
            if let Ok(new_path) = std::str::from_utf8(new_path_tok) {
                map.insert(new_path.to_string(), parse_counts(added, deleted));
            }
            i += 3;
        } else {
            map.insert(third.to_string(), parse_counts(added, deleted));
            i += 1;
        }
    }

    Ok(map)
}

fn parse_counts(added: &str, deleted: &str) -> (Option<u64>, Option<u64>) {
    let a = if added == "-" {
        None
    } else {
        added.parse().ok()
    };
    let d = if deleted == "-" {
        None
    } else {
        deleted.parse().ok()
    };
    (a, d)
}

#[cfg(test)]
mod tests {
    use super::ChangedFilesOutput;
    use crate::query::ChangedFiles;
    use crate::runner::{CallOutcome, RunnerError};
    use crate::{GitParseError, Oid, Side};

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

    fn ok(stdout: Vec<u8>) -> Result<CallOutcome, RunnerError> {
        Ok(CallOutcome {
            stdout,
            truncated: false,
        })
    }

    fn find<'a>(output: &'a ChangedFilesOutput, path: &str) -> &'a super::ChangedFile {
        output
            .files
            .iter()
            .find(|f| f.path == path)
            .unwrap_or_else(|| panic!("找不到 {path}"))
    }

    /// spec 變更檔案清單端點 Scenario「兩個 commit 之間」。
    #[test]
    fn parses_two_commits_between_scenario_from_spec() {
        let query = ChangedFiles::new(Side::Oid(oid('a')), Side::Oid(oid('b'))).unwrap();

        let name_status = nul_join(&[
            "A", "n.md", "M", "m.rs", "D", "d.txt", "R100", "o.md", "p.md", "M", "img.png",
        ]);
        let numstat = nul_join(&["5\t0\tn.md", "3\t1\tm.rs", "0\t4\td.txt", "-\t-\timg.png"]);
        // 改名紀錄額外用三個 NUL 分隔 token 附加在後面（見 `parse_numstat` 的假設說明）。
        let mut numstat = numstat;
        numstat.extend_from_slice(b"2\t0\t\0o.md\0p.md\0");

        let output = query
            .parse(&[ok(name_status), ok(numstat)])
            .expect("應解析成功");

        assert!(!output.truncated);
        assert_eq!(output.skipped, 0);
        assert_eq!(output.files.len(), 5);

        let n = find(&output, "n.md");
        assert_eq!(n.status, 'A');
        assert_eq!(n.old_path, None);

        let m = find(&output, "m.rs");
        assert_eq!(m.status, 'M');
        assert_eq!(m.additions, Some(3));
        assert_eq!(m.deletions, Some(1));

        let d = find(&output, "d.txt");
        assert_eq!(d.status, 'D');

        let p = find(&output, "p.md");
        assert_eq!(p.status, 'R');
        assert_eq!(p.old_path.as_deref(), Some("o.md"));

        let img = find(&output, "img.png");
        assert_eq!(img.status, 'M');
        assert_eq!(img.additions, None, "二進位檔的增刪行數應為 null");
        assert_eq!(img.deletions, None);
    }

    #[test]
    fn numstat_missing_path_yields_null_counts() {
        // 用 Oid→Oid：`diff` 家族不濾 stat-dirty，缺漏的路徑保留、增刪為 null。
        let query = ChangedFiles::new(Side::Oid(oid('a')), Side::Oid(oid('b'))).unwrap();
        let name_status = nul_join(&["M", "race.rs"]);
        let numstat = Vec::new(); // 兩次呼叫之間工作區改變，這個路徑在 numstat 完全消失

        let output = query
            .parse(&[ok(name_status), ok(numstat)])
            .expect("應解析成功");
        assert_eq!(output.files.len(), 1);
        assert_eq!(output.files[0].additions, None);
        assert_eq!(output.files[0].deletions, None);
    }

    /// Ruling R13：`diff-files` 家族（Index→Worktree）狀態 `M` 且不在 numstat 的路徑是
    /// stat-dirty 假陽性，丟掉；在 numstat 的（含純權限變更 `0\t0`）保留；非 `M` 不動。
    #[test]
    fn diff_files_drops_modified_paths_absent_from_numstat_only() {
        let query = ChangedFiles::new(Side::Index, Side::Worktree).unwrap();
        let name_status = nul_join(&[
            "M", "stale.rs", "M", "real.rs", "M", "mode.sh", "D", "gone.rs",
        ]);
        let numstat = nul_join(&["1\t1\treal.rs", "0\t0\tmode.sh", "0\t3\tgone.rs"]);

        let output = query
            .parse(&[ok(name_status), ok(numstat)])
            .expect("應解析成功");
        let paths: Vec<&str> = output.files.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(paths, ["real.rs", "mode.sh", "gone.rs"]);
    }

    /// numstat 被截斷時缺漏的路徑可能只是沒讀到，不能當假陽性丟掉。
    #[test]
    fn diff_files_keeps_paths_when_numstat_is_truncated() {
        let query = ChangedFiles::new(Side::Index, Side::Worktree).unwrap();
        let name_status = nul_join(&["M", "late.rs"]);
        let output = query
            .parse(&[
                ok(name_status),
                Ok(CallOutcome {
                    stdout: Vec::new(),
                    truncated: true,
                }),
            ])
            .expect("應解析成功");
        assert_eq!(output.files.len(), 1);
    }

    #[test]
    fn empty_to_oid_root_commit_skips_leading_commit_hash_line() {
        let query = ChangedFiles::new(Side::Empty, Side::Oid(oid('a'))).unwrap();
        let commit_hash = oid('a').to_string();

        let mut name_status = nul_join(&[&commit_hash]);
        name_status.extend_from_slice(&nul_join(&["A", "a.txt"]));
        let mut numstat = nul_join(&[&commit_hash]);
        numstat.extend_from_slice(&nul_join(&["3\t0\ta.txt"]));

        let output = query
            .parse(&[ok(name_status), ok(numstat)])
            .expect("應解析成功");
        assert_eq!(output.files.len(), 1);
        assert_eq!(output.files[0].path, "a.txt");
        assert_eq!(output.files[0].status, 'A');
        assert_eq!(output.files[0].additions, Some(3));
    }

    #[test]
    fn non_utf8_path_in_name_status_is_skipped_and_counted() {
        let query = ChangedFiles::new(Side::Oid(oid('a')), Side::Oid(oid('b'))).unwrap();
        let mut name_status = nul_join(&["M"]);
        name_status.extend_from_slice(b"bad_\xFF_path.rs\0");
        name_status.extend_from_slice(&nul_join(&["M", "ok.rs"]));

        let output = query
            .parse(&[ok(name_status), ok(Vec::new())])
            .expect("非法路徑應被略過而非整體失敗");
        assert_eq!(output.skipped, 1);
        assert_eq!(output.files.len(), 1);
        assert_eq!(output.files[0].path, "ok.rs");
    }

    #[test]
    fn truncated_drops_incomplete_trailing_rename_record() {
        let query = ChangedFiles::new(Side::Oid(oid('a')), Side::Oid(oid('b'))).unwrap();
        let mut name_status = nul_join(&["M", "a.rs"]);
        // 截斷在改名紀錄的路徑欄位之前：R100 token 存在，但配對的 old/new path 都不見了。
        name_status.extend_from_slice(b"R100");

        let output = query
            .parse(&[
                Ok(CallOutcome {
                    stdout: name_status,
                    truncated: true,
                }),
                ok(Vec::new()),
            ])
            .expect("截斷不完整紀錄應被捨棄，不是錯誤");
        assert!(output.truncated);
        assert_eq!(output.files.len(), 1);
        assert_eq!(output.files[0].path, "a.rs");
    }

    #[test]
    fn malformed_missing_path_without_truncation_is_error() {
        let query = ChangedFiles::new(Side::Index, Side::Worktree).unwrap();
        let name_status = nul_join(&["M"]); // 缺少路徑欄位，且不是截斷

        let err = query
            .parse(&[ok(name_status), ok(Vec::new())])
            .expect_err("非截斷情況下缺少路徑欄位應是錯誤");
        assert!(matches!(err, GitParseError::MalformedOutput(_)));
    }

    #[test]
    fn wrong_call_count_is_unexpected_call_count() {
        let query = ChangedFiles::new(Side::Index, Side::Worktree).unwrap();
        let err = query
            .parse(&[])
            .expect_err("呼叫次數不是 2 應回 UnexpectedCallCount");
        assert_eq!(err, GitParseError::UnexpectedCallCount);
    }

    #[test]
    fn call_failure_maps_to_unexpected_call_outcome() {
        let query = ChangedFiles::new(Side::Index, Side::Worktree).unwrap();
        let calls: Vec<Result<CallOutcome, RunnerError>> = vec![
            Err(RunnerError::Failed {
                exit_code: Some(128),
                stderr_tail: Vec::new(),
            }),
            ok(Vec::new()),
        ];
        let err = query
            .parse(&calls)
            .expect_err("name-status 呼叫失敗應轉成解析錯誤");
        assert!(matches!(err, GitParseError::UnexpectedCallOutcome(_)));
    }
}
