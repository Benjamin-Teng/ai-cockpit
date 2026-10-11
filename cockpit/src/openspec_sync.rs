//! OpenSpec 偵測的純邏輯（openspec-stage-sync task 4.2；design D6、D10-4、D10-5、D10-9）。
//!
//! 給定 worktree 根目錄與目前分支，直接讀 `openspec/changes/` 判定該 worktree 對應的 change 與階段，
//! 不呼叫 `openspec` CLI、不跑 git、不碰 tokio。全程唯讀。

use std::io::ErrorKind;
use std::path::Path;

use cockpit_core::{Observation, OpenSpecPhase};
use cockpit_files::{FilesError, read_capped};

/// `tasks.md` 的讀取上限：1 MiB。大於此值才算超限，剛好等於可讀（design D10-9）。
pub const TASKS_MD_LIMIT: u64 = 1_048_576;

/// 偵測 `root`（worktree 根目錄）目前對應的 OpenSpec 進度。`branch` 為目前分支名稱（detached 為 `None`）。
/// `None` 表示無結果，包含所有「無法判斷」的情況（目錄或 `tasks.md` 讀取錯誤、超限、非 UTF-8）。
///
/// 對應順序（design D6）：分支最後一段等於進行中 change → 否則 archive 有 `YYYY-MM-DD-<該段>`（取日期最大者，
/// 階段 complete）→ 否則進行中恰好一個 → 否則無結果。`archive/` 只在前一步沒命中且有分支時才讀。
pub fn detect(root: &Path, branch: Option<&str>) -> Option<Observation> {
    let changes_dir = root.join("openspec").join("changes");
    let in_progress = list_in_progress(&changes_dir).ok()??;
    let segment = branch
        .and_then(|b| b.rsplit('/').next())
        .filter(|s| !s.is_empty());

    if let Some(seg) = segment
        && in_progress.iter().any(|name| name == seg)
    {
        return observe(&changes_dir.join(seg), false);
    }
    if let Some(seg) = segment {
        let archive_dir = changes_dir.join("archive");
        if let Some(dir_name) = latest_archive(&archive_dir, seg).ok()? {
            return observe_named(seg, &archive_dir.join(dir_name), true);
        }
    }
    match in_progress.as_slice() {
        [only] => observe(&changes_dir.join(only), false),
        _ => None,
    }
}

/// 進行中的 change 名稱（`changes/` 的直接子目錄，`archive` 除外）。`Ok(None)` ＝ 目錄不存在；`Err` ＝ 讀取錯誤。
fn list_in_progress(changes_dir: &Path) -> Result<Option<Vec<String>>, ()> {
    let Some(entries) = read_dir_opt(changes_dir)? else {
        return Ok(None);
    };
    let mut names = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|_| ())?;
        if !is_dir(&entry.path())? {
            continue;
        }
        if let Some(name) = entry.file_name().to_str()
            && name != "archive"
        {
            names.push(name.to_string());
        }
    }
    Ok(Some(names))
}

/// `archive/` 下目錄名為 `YYYY-MM-DD-<slug>` 者，取日期字串最大的那個目錄名。`archive/` 不存在 → `Ok(None)`。
fn latest_archive(archive_dir: &Path, slug: &str) -> Result<Option<String>, ()> {
    let Some(entries) = read_dir_opt(archive_dir)? else {
        return Ok(None);
    };
    let mut best: Option<String> = None;
    for entry in entries {
        let entry = entry.map_err(|_| ())?;
        let Some(name) = entry.file_name().to_str().map(str::to_string) else {
            continue;
        };
        if !is_archive_name_for(&name, slug) || !is_dir(&entry.path())? {
            continue;
        }
        // 日期固定 10 個 ASCII 字元，整串字典序即時間序；同 slug 下比整個目錄名等同比日期。
        if best.as_ref().is_none_or(|b| name > *b) {
            best = Some(name);
        }
    }
    Ok(best)
}

/// 目錄名是否為嚴格的 `YYYY-MM-DD-<slug>`（四位數字-兩位-兩位-後接完整 slug）。
fn is_archive_name_for(name: &str, slug: &str) -> bool {
    let bytes = name.as_bytes();
    if bytes.len() != 11 + slug.len() || !name.is_char_boundary(11) {
        return false;
    }
    let (prefix, rest) = name.split_at(11);
    let p = prefix.as_bytes();
    let digits_ok = [0, 1, 2, 3, 5, 6, 8, 9]
        .iter()
        .all(|&i| p[i].is_ascii_digit());
    digits_ok && p[4] == b'-' && p[7] == b'-' && p[10] == b'-' && rest == slug
}

/// 是否為目錄（跟隨符號連結）。NotFound（例如懸空連結）→ `false`；其他 metadata 錯誤 → `Err`，不可吞成 `false`。
fn is_dir(path: &Path) -> Result<bool, ()> {
    match std::fs::metadata(path) {
        Ok(md) => Ok(md.is_dir()),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(false),
        Err(_) => Err(()),
    }
}

/// `read_dir`，目錄不存在 → `Ok(None)`，其他錯誤 → `Err`。
fn read_dir_opt(dir: &Path) -> Result<Option<std::fs::ReadDir>, ()> {
    match std::fs::read_dir(dir) {
        Ok(rd) => Ok(Some(rd)),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
        Err(_) => Err(()),
    }
}

fn observe(change_dir: &Path, archived: bool) -> Option<Observation> {
    let name = change_dir.file_name()?.to_str()?.to_string();
    observe_named(&name, change_dir, archived)
}

/// 讀 `change_dir/tasks.md` 組出偵測結果；`change` 為回報的 change 名稱（archive 目錄名帶日期，不能直接用）。
fn observe_named(change: &str, change_dir: &Path, archived: bool) -> Option<Observation> {
    let (checked, total) = match read_capped(&change_dir.join("tasks.md"), TASKS_MD_LIMIT) {
        Ok(bytes) => {
            let text = std::str::from_utf8(&bytes).ok()?;
            // 去掉開頭的 UTF-8 BOM，否則第一行的 checkbox 會被漏算。
            parse_checkboxes(text.strip_prefix('\u{feff}').unwrap_or(text))
        }
        // 檔案不存在不是錯誤：視為 0／0（design D10-5）。
        Err(FilesError::Io(e)) if e.kind() == ErrorKind::NotFound => (0, 0),
        Err(_) => return None,
    };
    let phase = if archived {
        OpenSpecPhase::Complete
    } else {
        phase_for(checked, total)
    };
    Some(Observation {
        change: change.to_string(),
        phase,
        checked,
        total,
    })
}

/// 解析 `tasks.md` 文字，回傳 `(checked, total)`。
///
/// 一行的行首空白（空白或 tab）之後是 `-`、`*` 或 `+`，接恰好一個空白，再接 `[ ]`／`[x]`／`[X]`；
/// `]` 之後接什麼都行（與 OpenSpec CLI 一致，`- [x]foo` 也算）。有序清單等其他寫法不計。
pub fn parse_checkboxes(text: &str) -> (u32, u32) {
    let (mut checked, mut total) = (0, 0);
    for line in text.lines() {
        // `lines()` 已去掉換行（含 CRLF）。
        let Some(rest) = line
            .trim_start_matches([' ', '\t'])
            .strip_prefix(['-', '*', '+'])
        else {
            continue;
        };
        let Some(rest) = rest.strip_prefix(' ') else {
            continue;
        };
        let is_checked = if rest.starts_with("[ ]") {
            false
        } else if rest.starts_with("[x]") || rest.starts_with("[X]") {
            true
        } else {
            continue;
        };
        total += 1;
        if is_checked {
            checked += 1;
        }
    }
    (checked, total)
}

/// 依勾選數判定尚未 archive 的 change 的階段。
pub fn phase_for(checked: u32, total: u32) -> OpenSpecPhase {
    if checked == 0 {
        OpenSpecPhase::Plan
    } else if checked < total {
        OpenSpecPhase::Implement
    } else {
        OpenSpecPhase::Review
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU32, Ordering};

    /// 建在 `%TEMP%` 的暫存 worktree 根目錄，離開作用域時刪除。
    struct Tmp(PathBuf);

    impl Tmp {
        fn new() -> Self {
            static N: AtomicU32 = AtomicU32::new(0);
            let dir = std::env::temp_dir().join(format!(
                "cockpit-openspec-sync-{}-{}",
                std::process::id(),
                N.fetch_add(1, Ordering::Relaxed)
            ));
            let _ = fs::remove_dir_all(&dir);
            fs::create_dir_all(&dir).unwrap();
            Tmp(dir)
        }
        fn changes(&self) -> PathBuf {
            self.0.join("openspec").join("changes")
        }
        /// 建進行中的 change 目錄，`tasks` 有值才寫 `tasks.md`。
        fn change(&self, name: &str, tasks: Option<&str>) {
            let dir = self.changes().join(name);
            fs::create_dir_all(&dir).unwrap();
            if let Some(text) = tasks {
                fs::write(dir.join("tasks.md"), text).unwrap();
            }
        }
        fn archived(&self, dir_name: &str, tasks: Option<&str>) {
            self.change(&format!("archive/{dir_name}"), tasks);
        }
    }

    impl Drop for Tmp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn obs(change: &str, phase: OpenSpecPhase, checked: u32, total: u32) -> Option<Observation> {
        Some(Observation {
            change: change.to_string(),
            phase,
            checked,
            total,
        })
    }

    const TASKS_3_OF_8: &str =
        "- [x] a\n- [x] b\n- [x] c\n- [ ] d\n- [ ] e\n- [ ] f\n- [ ] g\n- [ ] h\n";
    const TASKS_8_OF_8: &str =
        "- [x] a\n- [x] b\n- [x] c\n- [x] d\n- [x] e\n- [x] f\n- [x] g\n- [x] h\n";

    // ---- change 對應 ----

    #[test]
    fn branch_last_segment_matches_in_progress_change() {
        let t = Tmp::new();
        t.change("foo", Some(TASKS_3_OF_8));
        t.change("bar", None);
        assert_eq!(
            detect(&t.0, Some("feat/foo")),
            obs("foo", OpenSpecPhase::Implement, 3, 8)
        );
    }

    #[test]
    fn branch_without_slash_matches() {
        let t = Tmp::new();
        t.change("foo", Some(TASKS_3_OF_8));
        t.change("bar", None);
        assert_eq!(
            detect(&t.0, Some("foo")),
            obs("foo", OpenSpecPhase::Implement, 3, 8)
        );
    }

    #[test]
    fn takes_last_segment_not_first() {
        let t = Tmp::new();
        t.change("foo", Some(TASKS_3_OF_8));
        t.change("feat", None);
        assert_eq!(
            detect(&t.0, Some("user/feat/foo")),
            obs("foo", OpenSpecPhase::Implement, 3, 8)
        );
    }

    #[test]
    fn archive_match_beats_single_in_progress_fallback() {
        let t = Tmp::new();
        t.change("bar", None);
        t.archived("2026-10-08-foo", Some(TASKS_8_OF_8));
        assert_eq!(
            detect(&t.0, Some("feat/foo")),
            obs("foo", OpenSpecPhase::Complete, 8, 8)
        );
    }

    #[test]
    fn in_progress_beats_archive() {
        let t = Tmp::new();
        t.change("foo", Some(TASKS_3_OF_8));
        t.archived("2026-10-08-foo", Some(TASKS_8_OF_8));
        assert_eq!(
            detect(&t.0, Some("feat/foo")),
            obs("foo", OpenSpecPhase::Implement, 3, 8)
        );
    }

    #[test]
    fn archive_dir_name_must_match_exactly() {
        let t = Tmp::new();
        t.change("bar", None);
        t.change("baz", None);
        for name in [
            "2026-10-08-foo-bar",
            "foo",
            "10-08-foo",
            "2026-10-08-fo",
            "2026-1-08-foo",
            "x2026-10-08-foo",
            "2026-10-08foo",
            "abcd-10-08-foo",
        ] {
            t.archived(name, Some(TASKS_8_OF_8));
        }
        assert_eq!(detect(&t.0, Some("feat/foo")), None);
    }

    #[test]
    fn archive_entry_that_is_a_file_is_not_a_match() {
        let t = Tmp::new();
        t.change("bar", None);
        t.change("baz", None);
        fs::create_dir_all(t.changes().join("archive")).unwrap();
        fs::write(t.changes().join("archive/2026-10-08-foo"), "x").unwrap();
        assert_eq!(detect(&t.0, Some("feat/foo")), None);
    }

    #[test]
    fn single_in_progress_fallback_on_unrelated_branch() {
        let t = Tmp::new();
        t.change("foo", Some(TASKS_8_OF_8));
        assert_eq!(
            detect(&t.0, Some("main")),
            obs("foo", OpenSpecPhase::Review, 8, 8)
        );
    }

    #[test]
    fn multiple_in_progress_and_unmatched_branch_is_none() {
        let t = Tmp::new();
        t.change("foo", None);
        t.change("bar", None);
        assert_eq!(detect(&t.0, Some("main")), None);
    }

    #[test]
    fn detached_head_with_exactly_one_in_progress() {
        let t = Tmp::new();
        t.change("foo", Some(TASKS_3_OF_8));
        assert_eq!(
            detect(&t.0, None),
            obs("foo", OpenSpecPhase::Implement, 3, 8)
        );
    }

    #[test]
    fn detached_head_with_two_in_progress_is_none() {
        let t = Tmp::new();
        t.change("foo", None);
        t.change("bar", None);
        assert_eq!(detect(&t.0, None), None);
    }

    #[test]
    fn detached_head_never_matches_archive() {
        let t = Tmp::new();
        t.change("bar", None);
        t.change("baz", None);
        t.archived("2026-10-08-foo", Some(TASKS_8_OF_8));
        assert_eq!(detect(&t.0, None), None);
    }

    #[test]
    fn empty_last_segment_matches_nothing() {
        let t = Tmp::new();
        t.change("bar", None);
        t.change("baz", None);
        t.archived("2026-10-08-", Some(TASKS_8_OF_8));
        assert_eq!(detect(&t.0, Some("feat/")), None);
    }

    #[test]
    fn no_openspec_dir_is_none() {
        let t = Tmp::new();
        assert_eq!(detect(&t.0, Some("feat/foo")), None);
        fs::create_dir_all(t.0.join("openspec")).unwrap();
        assert_eq!(detect(&t.0, Some("feat/foo")), None);
    }

    #[test]
    fn archive_dir_and_plain_files_are_not_changes() {
        let t = Tmp::new();
        fs::create_dir_all(t.changes().join("archive")).unwrap();
        fs::write(t.changes().join("notes.md"), "x").unwrap();
        assert_eq!(detect(&t.0, Some("feat/archive")), None);
    }

    #[test]
    fn archive_with_several_dates_takes_latest() {
        let t = Tmp::new();
        t.archived("2026-09-01-foo", Some(TASKS_3_OF_8));
        t.archived("2026-10-08-foo", Some(TASKS_8_OF_8));
        t.archived("2026-05-30-foo", Some("- [ ] x\n"));
        assert_eq!(
            detect(&t.0, Some("feat/foo")),
            obs("foo", OpenSpecPhase::Complete, 8, 8)
        );
    }

    #[test]
    fn archive_without_tasks_md_is_complete_zero_zero() {
        let t = Tmp::new();
        t.archived("2026-10-08-foo", None);
        assert_eq!(
            detect(&t.0, Some("feat/foo")),
            obs("foo", OpenSpecPhase::Complete, 0, 0)
        );
    }

    // ---- 階段判定 ----

    #[test]
    fn missing_tasks_md_is_plan() {
        let t = Tmp::new();
        t.change("foo", None);
        assert_eq!(
            detect(&t.0, Some("foo")),
            obs("foo", OpenSpecPhase::Plan, 0, 0)
        );
    }

    #[test]
    fn no_checks_is_plan() {
        let t = Tmp::new();
        t.change("foo", Some("- [ ] a\n- [ ] b\n- [ ] c\n- [ ] d\n- [ ] e\n"));
        assert_eq!(
            detect(&t.0, Some("foo")),
            obs("foo", OpenSpecPhase::Plan, 0, 5)
        );
    }

    #[test]
    fn no_checkbox_at_all_is_plan() {
        let t = Tmp::new();
        t.change("foo", Some("# tasks\n\nsome text\n"));
        assert_eq!(
            detect(&t.0, Some("foo")),
            obs("foo", OpenSpecPhase::Plan, 0, 0)
        );
    }

    #[test]
    fn all_checked_in_progress_is_review() {
        let t = Tmp::new();
        t.change("foo", Some(TASKS_8_OF_8));
        assert_eq!(
            detect(&t.0, Some("foo")),
            obs("foo", OpenSpecPhase::Review, 8, 8)
        );
    }

    #[test]
    fn phase_for_boundaries() {
        assert_eq!(phase_for(0, 0), OpenSpecPhase::Plan);
        assert_eq!(phase_for(0, 5), OpenSpecPhase::Plan);
        assert_eq!(phase_for(1, 5), OpenSpecPhase::Implement);
        assert_eq!(phase_for(4, 5), OpenSpecPhase::Implement);
        assert_eq!(phase_for(5, 5), OpenSpecPhase::Review);
        assert_eq!(phase_for(1, 1), OpenSpecPhase::Review);
    }

    #[test]
    fn checkbox_recognition_matches_spec_scenario() {
        let text = "- [x] a\n  * [ ] b\n+ [X] c\n1. [x] d\n- [] e\n-[x] f\n- [y] g\n[x] h\n";
        assert_eq!(parse_checkboxes(text), (2, 3));
        let t = Tmp::new();
        t.change("foo", Some(text));
        assert_eq!(
            detect(&t.0, Some("foo")),
            obs("foo", OpenSpecPhase::Implement, 2, 3)
        );
    }

    #[test]
    fn checkbox_allows_text_right_after_bracket() {
        // 與 OpenSpec CLI（`^\s*[-*]\s*\[([\sxX])\]`）一致：`]` 之後接什麼都行，`- [x]foo` 照算。
        assert_eq!(parse_checkboxes("- [x]foo\n- [ ]bar\n"), (1, 2));
        assert_eq!(parse_checkboxes("- [x]\n- [ ]\n"), (1, 2));
        assert_eq!(parse_checkboxes("- [x]\ttab\n"), (1, 1));
    }

    #[test]
    fn leading_bom_does_not_hide_first_line_checkbox() {
        let t = Tmp::new();
        t.change("foo", None);
        fs::write(
            t.changes().join("foo/tasks.md"),
            "\u{feff}- [x] a\n- [ ] b\n",
        )
        .unwrap();
        assert_eq!(
            detect(&t.0, Some("foo")),
            obs("foo", OpenSpecPhase::Implement, 1, 2)
        );
    }

    #[test]
    fn archive_is_read_lazily_only_when_needed() {
        // 釘住惰性讀取（控制端已裁決接受）：分支命中進行中的 change 時，不可讀的 archive 不影響結果。
        let t = Tmp::new();
        t.change("foo", Some(TASKS_3_OF_8));
        fs::write(t.changes().join("archive"), "not a dir").unwrap();
        assert_eq!(
            detect(&t.0, Some("feat/foo")),
            obs("foo", OpenSpecPhase::Implement, 3, 8)
        );
    }

    #[test]
    fn dangling_symlink_entry_is_skipped_not_an_error() {
        // 懸空連結：metadata 回 NotFound，略過（不算 change、也不使整個 worktree 無法判斷）。
        // Windows 建連結需要權限／開發人員模式；建不出來就略過此測試（metadata 的其他錯誤無法在測試中造出）。
        let t = Tmp::new();
        t.change("foo", Some(TASKS_3_OF_8));
        let link = t.changes().join("ghost");
        #[cfg(windows)]
        let made = std::os::windows::fs::symlink_dir(t.0.join("nowhere"), &link).is_ok();
        #[cfg(unix)]
        let made = std::os::unix::fs::symlink(t.0.join("nowhere"), &link).is_ok();
        if !made {
            eprintln!("無法建立符號連結，略過 dangling_symlink_entry_is_skipped_not_an_error");
            return;
        }
        assert_eq!(
            detect(&t.0, Some("main")),
            obs("foo", OpenSpecPhase::Implement, 3, 8)
        );
    }

    #[test]
    fn checkbox_handles_crlf_tabs_and_double_space() {
        assert_eq!(parse_checkboxes("- [x] a\r\n- [ ] b\r\n"), (1, 2));
        assert_eq!(parse_checkboxes("\t- [x] a\n"), (1, 1));
        assert_eq!(parse_checkboxes("- [x]\r\n"), (1, 1));
        // 項目符號後只接受一個空白（規格寫「一個空白」）。
        assert_eq!(parse_checkboxes("-  [x] a\n"), (0, 0));
    }

    // ---- 讀取上限與無法判斷 ----

    /// 一行 checkbox 加上補到指定長度的文字，內容為合法 UTF-8。
    fn padded(len: usize) -> String {
        let head = "- [x] a\n";
        let mut s = String::from(head);
        s.push_str(&"x".repeat(len - head.len()));
        s
    }

    #[test]
    fn tasks_md_exactly_1_mib_is_readable() {
        let t = Tmp::new();
        let text = padded(1_048_576);
        assert_eq!(text.len() as u64, TASKS_MD_LIMIT);
        t.change("foo", Some(&text));
        assert_eq!(
            detect(&t.0, Some("foo")),
            obs("foo", OpenSpecPhase::Review, 1, 1)
        );
    }

    #[test]
    fn tasks_md_over_1_mib_is_none() {
        let t = Tmp::new();
        t.change("foo", Some(&padded(1_048_577)));
        assert_eq!(detect(&t.0, Some("foo")), None);
        let t2 = Tmp::new();
        t2.change("foo", Some(&padded(2 * 1_048_576)));
        assert_eq!(detect(&t2.0, Some("foo")), None);
    }

    #[test]
    fn archived_tasks_md_over_limit_is_none() {
        let t = Tmp::new();
        t.archived("2026-10-08-foo", Some(&padded(1_048_577)));
        assert_eq!(detect(&t.0, Some("foo")), None);
    }

    #[test]
    fn tasks_md_not_utf8_is_none() {
        let t = Tmp::new();
        t.change("foo", None);
        fs::write(t.changes().join("foo/tasks.md"), b"- [x] a\n\xff\xfe\n").unwrap();
        assert_eq!(detect(&t.0, Some("foo")), None);
    }

    #[test]
    fn tasks_md_unreadable_is_none_not_plan() {
        // 把 tasks.md 做成目錄：開啟／讀取必然失敗，且不是 NotFound。
        let t = Tmp::new();
        t.change("foo", None);
        fs::create_dir_all(t.changes().join("foo/tasks.md")).unwrap();
        assert_eq!(detect(&t.0, Some("foo")), None);
    }

    #[test]
    fn changes_dir_read_error_is_none() {
        // 在 Windows 上難以造出權限不足；把 `changes` 做成檔案：存在但 read_dir 失敗（非 NotFound）。
        let t = Tmp::new();
        fs::create_dir_all(t.0.join("openspec")).unwrap();
        fs::write(t.changes(), "not a dir").unwrap();
        assert_eq!(detect(&t.0, Some("foo")), None);
    }

    #[test]
    fn archive_dir_read_error_is_none_not_absent() {
        // `archive` 做成檔案：存在但讀取失敗。若誤當成「不存在」，分支 main 會退路對上唯一進行中的 bar。
        let t = Tmp::new();
        t.change("bar", Some(TASKS_3_OF_8));
        fs::write(t.changes().join("archive"), "not a dir").unwrap();
        assert_eq!(detect(&t.0, Some("main")), None);
    }

    #[test]
    fn missing_archive_dir_is_just_no_archive_match() {
        let t = Tmp::new();
        t.change("bar", Some(TASKS_3_OF_8));
        assert_eq!(
            detect(&t.0, Some("main")),
            obs("bar", OpenSpecPhase::Implement, 3, 8)
        );
    }

    // ---- 唯讀 ----

    type Snap = Vec<(PathBuf, u64, std::time::SystemTime)>;

    fn snapshot(root: &Path) -> Snap {
        fn walk(dir: &Path, out: &mut Snap) {
            for e in fs::read_dir(dir).unwrap() {
                let e = e.unwrap();
                // 用 fs::metadata 查檔案本身：NTFS 目錄項目快取的時間戳可能過時。
                let md = fs::metadata(e.path()).unwrap();
                out.push((e.path(), md.len(), md.modified().unwrap()));
                if md.is_dir() {
                    walk(&e.path(), out);
                }
            }
        }
        let mut out = Vec::new();
        walk(root, &mut out);
        out.sort();
        out
    }

    #[test]
    fn detection_is_read_only() {
        let t = Tmp::new();
        t.change("foo", Some(TASKS_3_OF_8));
        t.change("bar", None);
        t.archived("2026-10-08-baz", Some(TASKS_8_OF_8));
        let before = snapshot(&t.0);
        for branch in [Some("feat/foo"), Some("baz"), Some("main"), None] {
            let _ = detect(&t.0, branch);
        }
        assert_eq!(snapshot(&t.0), before);
    }
}
