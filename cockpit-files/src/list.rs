//! 列一層目錄（file-review spec「列目錄端點」、design D4；file-review task 2.3 fix round 1
//! 改用 `GitignoreBuilder` 逐層比對，取代原本的 `WalkBuilder`／`filter_entry`）。
//!
//! 不用 `WalkBuilder` 走祖先鏈的原因（Codex review 四條 finding、控制端裁決 R12）：那個做法只驗
//! 最終目標的實體路徑在根內，沒有逐層驗證中途每一段，讓兩段各自安全的連結接力（`root/link` 指到
//! 根外、根外的 `back` 又指回根內）可以繞過界限檢查；`filter_entry` 用字面路徑比對祖先鏈，
//! 請求路徑的大小寫或 8.3 別名跟磁碟上不同就會整層被剪枝、靜默回傳空清單；`WalkBuilder` 預設還會
//! 讀 `.ignore`（優先於 `.gitignore`），超出允許的過濾來源；讀取錯誤也被 `let Ok(entry) = .. else
//! { continue }` 吞掉，回傳一個看似成功、其實是空的清單。
//!
//! 新做法：
//! 1. 對 `rel` 的每個前綴（根、根/a、根/a/b……直到目標）各自呼叫 [`resolve`]，任一層的實體路徑
//!    不在根目錄內就整個請求失敗（`resolve` 的懸空連結一致拒絕規則同樣沿用）。
//! 2. 用 [`ignore::gitignore::GitignoreBuilder`] 為每一層建一個 matcher：matcher 的根是該層的
//!    **邏輯路徑**（`root.join(前 i 段)`，不 canonicalize，保留請求當下的大小寫／別名字面），
//!    加入的檔案是該層**實體目錄**（`resolve` 算出來的那個）下的 `.gitignore`（存在才加）；根是
//!    git repo 且 `root/.git` 是資料夾時，另把 `root/.git/info/exclude` 併入根那一層（`.git` 是
//!    檔案的 worktree 不處理 exclude）。只加這兩種檔案，不讀 `.ignore`、不讀使用者全域設定、
//!    不讀根以上任何檔。Windows 上 `case_insensitive(true)`（對齊 Git for Windows 預設
//!    `core.ignorecase=true`）。
//! 3. 列舉用 `fs::read_dir`（目標的實體路徑）；`read_dir` 本身或任一項目的錯誤一律轉成
//!    `FilesError::Io`，不吞。比對每個子項目時用**邏輯路徑**（目標的邏輯路徑 `.join(name)`），
//!    由深到淺取第一個非 `None` 的 `Match`。
//!
//! 已知限制（R12(d)，接受）：8.3 短名稱若出現在請求路徑裡，`.gitignore` 對「含目錄名稱那一段」
//! 的規則（例如 `long-directory-name-inside/*.txt`）可能因為字面比對用的是短別名而不生效；只匹配
//! 檔名（不含斜線）的規則不受影響，因為比對只看子項目自己的真實檔名。
//!
//! file-review task 2.3 fix round 2（Codex review 兩條新 finding、控制端裁決 R13）另外處理兩件事：
//! 1. **規則來源本身若是連結就整個略過**：`.gitignore`、`.git`、`.git/info`、`.git/info/exclude`
//!    任一是 symlink／junction／其他 reparse point，就不採用那個規則來源（不報錯），對齊 git 本身
//!    不跟隨 `.gitignore` symlink 的行為──否則根外的規則檔內容可以透過連結生效，變成另一種繞過
//!    根界限的管道。用 [`is_link_like`] 對每個規則來源檔案／目錄自己的 `symlink_metadata`
//!    判斷，跟它所在的（已驗證在根內的）目錄是不是連結無關。
//! 2. **祖先目錄一旦被排除，不能被深層規則重新納入**：git 語意是「父目錄整個被排除時，無法用更
//!    深層的 `!pattern` 復活裡面的檔案」；因此列出目標之前，要先由根往下確認目標本身與每個祖先
//!    目錄（不含根本身）有沒有被「它上方各層」（不含它自己那層）的 matcher 判成 `Ignore`——判斷時
//!    `is_dir` 固定為 `true`，一樣由深到淺取第一個非 `None` 的結果。只要有任一層被排除，整個列目錄
//!    直接回傳空 `entries`（`omitted`／`skipped` 皆為 0），不是錯誤。

use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

use ignore::Match;
use ignore::gitignore::{Gitignore, GitignoreBuilder};
use serde::Serialize;

use crate::relpath::is_link_like;
use crate::{FilesError, RelPath, resolve};

/// 一個目錄超過此筆數時截斷，見 spec「列目錄端點」「大目錄截斷」。
const MAX_ENTRIES: usize = 5000;

/// 目錄項目的種類。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Dir,
    File,
}

/// 一個目錄項目。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Entry {
    pub name: String,
    pub kind: Kind,
}

/// 列一層目錄的結果（spec「列目錄端點」）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Listing {
    pub entries: Vec<Entry>,
    pub omitted: usize,
    pub skipped: usize,
}

/// 列出 `root` 內 `rel` 指定目錄的直接子項目（不遞迴，design D4）。
///
/// 逐層（根、根/第一段、……、目標本身）以 [`resolve`] 做界限檢查（任一層跳出根目錄、不存在 →
/// 沿用其錯誤，見模組說明第 1 點）；目標是檔案 → [`FilesError::WrongKind`]。
pub fn list_dir(root: &Path, rel: &RelPath) -> Result<Listing, FilesError> {
    let segments = rel.segments();
    let target_depth = segments.len();

    // 逐層界限檢查：real_ancestors[i] 是第 i 層（0＝根目錄本身）的實體路徑，logical_ancestors[i]
    // 是同一層的邏輯路徑（字面 join，不 canonicalize）。任一層 `resolve` 失敗就整個請求失敗
    // （file-review task 2.3 fix round 1：不能只驗最終目標，兩段各自安全的連結接力可能繞一圈
    // 又回到根內）。
    let mut real_ancestors: Vec<PathBuf> = Vec::with_capacity(target_depth + 1);
    let mut logical_ancestors: Vec<PathBuf> = Vec::with_capacity(target_depth + 1);
    let mut logical_cursor = root.to_path_buf();
    for i in 0..=target_depth {
        logical_ancestors.push(logical_cursor.clone());
        let prefix = RelPath::from_segments(segments[..i].to_vec());
        real_ancestors.push(resolve(root, &prefix)?);
        if i < target_depth {
            logical_cursor = logical_cursor.join(&segments[i]);
        }
    }
    let target_real = &real_ancestors[target_depth];
    let target_logical = &logical_ancestors[target_depth];

    let target_meta = fs::metadata(target_real).map_err(FilesError::Io)?;
    if !target_meta.is_dir() {
        return Err(FilesError::WrongKind);
    }

    // 每一層各自的 gitignore matcher：根＝該層的邏輯路徑，加入的檔案＝該層實體目錄下的
    // `.gitignore`；根層另外可能有 `.git/info/exclude`。exclude 先加、`.gitignore` 後加，讓
    // `.gitignore`（`ignore` crate 對同一個 matcher 內衝突規則是「後加優先」）在衝突時贏過
    // exclude，對齊 git 本身「.gitignore 優先於 info/exclude」的順位。規則來源本身是連結就整個
    // 略過（`build_layer_matcher` 內部處理，見模組說明 fix round 2 第 1 點）。
    let matchers: Vec<Gitignore> = (0..=target_depth)
        .map(|i| build_layer_matcher(&logical_ancestors[i], &real_ancestors[i], i == 0))
        .collect();

    // 祖先目錄（不含根本身、含目標本身）一旦被「它上方各層」的 matcher 排除，深層規則不能重新
    // 納入（file-review task 2.3 fix round 2，模組說明第 2 點）：直接回傳空清單，不是錯誤。
    for k in 1..=target_depth {
        let ancestor_ignored =
            matchers[..k]
                .iter()
                .rev()
                .find_map(|m| match m.matched(&logical_ancestors[k], true) {
                    Match::None => None,
                    Match::Ignore(_) => Some(true),
                    Match::Whitelist(_) => Some(false),
                });
        if ancestor_ignored == Some(true) {
            return Ok(Listing {
                entries: Vec::new(),
                omitted: 0,
                skipped: 0,
            });
        }
    }

    // 列舉：目標的實體路徑；`read_dir` 本身與任一項目的錯誤一律回 Io，不吞（file-review task 2.3
    // fix round 1：目標存在但列舉失敗時，不能回傳一個看似成功、其實是空的清單）。
    let mut dirs: Vec<(String, String)> = Vec::new();
    let mut files: Vec<(String, String)> = Vec::new();
    let mut skipped = 0usize;

    for entry in fs::read_dir(target_real).map_err(FilesError::Io)? {
        let entry = entry.map_err(FilesError::Io)?;
        let file_name = entry.file_name();
        if file_name == OsStr::new(".git") {
            continue;
        }
        let Some(name) = file_name.to_str() else {
            skipped += 1;
            continue;
        };
        // 種類以 fs::metadata（跟隨連結）判斷；指向根目錄外的連結仍會列出，開啟時由 resolve 擋下。
        // 連結失效（fs::metadata 出錯）時當成檔案列出（挑一個一致做法，不整條略過）。
        let is_dir = fs::metadata(entry.path())
            .map(|m| m.is_dir())
            .unwrap_or(false);

        let child_logical = target_logical.join(name);
        let ignored = matchers
            .iter()
            .rev()
            .find_map(|m| match m.matched(&child_logical, is_dir) {
                Match::None => None,
                Match::Ignore(_) => Some(true),
                Match::Whitelist(_) => Some(false),
            });
        if ignored == Some(true) {
            continue;
        }

        let key = (name.to_lowercase(), name.to_string());
        if is_dir {
            dirs.push(key);
        } else {
            files.push(key);
        }
    }

    let (entries, omitted) = build_entries(dirs, files);
    Ok(Listing {
        entries,
        omitted,
        skipped,
    })
}

/// 建一層的 gitignore matcher（見模組說明第 2 點）。`is_root` 為真時另外檢查
/// `.git/info/exclude`（`root/.git` 是資料夾才有；是檔案的 worktree 不處理）。規則來源本身
/// （`.gitignore`、`.git`、`.git/info`、`.git/info/exclude`）只要有一個是連結／junction／其他
/// reparse point，就整個略過那個規則來源，不採用它的內容（file-review task 2.3 fix round 2 第 1
/// 點：`resolve` 只驗證了請求的目錄本身在根內，沒有驗證後續才載入的規則檔／目錄）。
fn build_layer_matcher(logical_dir: &Path, real_dir: &Path, is_root: bool) -> Gitignore {
    let mut builder = GitignoreBuilder::new(logical_dir);
    let _ = builder.case_insensitive(cfg!(windows));

    if is_root {
        let git_marker = real_dir.join(".git");
        if is_usable_regular_dir(&git_marker) {
            let info_dir = git_marker.join("info");
            if is_usable_regular_dir(&info_dir) {
                let exclude = info_dir.join("exclude");
                if is_usable_regular_file(&exclude) {
                    builder.add(&exclude);
                }
            }
        }
    }

    let gitignore_file = real_dir.join(".gitignore");
    if is_usable_regular_file(&gitignore_file) {
        builder.add(&gitignore_file);
    }

    builder.build().unwrap_or_else(|_| Gitignore::empty())
}

/// `path` 存在、是一般檔案，而且本身不是連結／junction／其他 reparse point。用
/// `symlink_metadata`（不跟隨連結）才能正確判斷「這個規則來源本身」，`is_file()`／`is_dir()`
/// 那種會跟隨連結的檢查看不出來。
fn is_usable_regular_file(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|m| m.is_file() && !is_link_like(&m))
}

/// 同 [`is_usable_regular_file`]，但要求是一般資料夾。
fn is_usable_regular_dir(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|m| m.is_dir() && !is_link_like(&m))
}

/// 排序（資料夾在前、檔案在後，各自不分大小寫；同名只差大小寫時用原字串當第二鍵，保證穩定）
/// 與截斷（5000 筆，之後做）。抽成純函式方便不靠檔案系統直接測排序規則。
fn build_entries(
    mut dirs: Vec<(String, String)>,
    mut files: Vec<(String, String)>,
) -> (Vec<Entry>, usize) {
    dirs.sort();
    files.sort();
    let mut entries: Vec<Entry> = dirs
        .into_iter()
        .map(|(_, name)| Entry {
            name,
            kind: Kind::Dir,
        })
        .chain(files.into_iter().map(|(_, name)| Entry {
            name,
            kind: Kind::File,
        }))
        .collect();
    let omitted = entries.len().saturating_sub(MAX_ENTRIES);
    entries.truncate(MAX_ENTRIES);
    (entries, omitted)
}

#[cfg(test)]
mod tests {
    use super::{Entry, Kind, build_entries, list_dir};
    use crate::FilesError;
    use crate::relpath::RelPath;
    use crate::test_support::TempDir;
    use std::fs;
    use std::path::Path;

    fn rel(raw: &str) -> RelPath {
        RelPath::parse(raw).expect("測試輸入應合法")
    }

    fn names(entries: &[Entry]) -> Vec<&str> {
        entries.iter().map(|e| e.name.as_str()).collect()
    }

    fn write(dir: &Path, name: &str, content: &str) {
        fs::write(dir.join(name), content).unwrap_or_else(|e| panic!("寫入 {name} 失敗：{e}"));
    }

    fn mkdir(dir: &Path, name: &str) {
        fs::create_dir_all(dir.join(name)).unwrap_or_else(|e| panic!("建立 {name} 失敗：{e}"));
    }

    /// Scenario: 依 .gitignore 過濾（同時涵蓋「點開頭項目照常列出」：`.github`、`.env.example`、
    /// `.gitignore` 都照常列出，只有 `.git` 被隱藏）。
    #[test]
    fn scenario_filters_by_gitignore() {
        let tmp = TempDir::new("list-gitignore-filter");
        let root = tmp.path();
        write(root, ".gitignore", "target/\n");
        mkdir(root, "target");
        mkdir(root, "src");
        mkdir(root, ".github");
        mkdir(root, ".git");
        write(root, "README.md", "# hi");
        write(root, ".env.example", "KEY=");

        let listing = list_dir(root, &rel("")).expect("列出根目錄應成功");

        assert_eq!(
            names(&listing.entries),
            vec![".github", "src", ".env.example", ".gitignore", "README.md"]
        );
        assert_eq!(listing.omitted, 0);
        assert_eq!(listing.skipped, 0);
    }

    /// Scenario: 大目錄截斷
    #[test]
    fn scenario_truncates_large_directory() {
        let tmp = TempDir::new("list-truncate");
        let root = tmp.path();
        for i in 0..5003 {
            write(root, &format!("f{i:05}.txt"), "");
        }

        let listing = list_dir(root, &rel("")).expect("列出大目錄應成功");

        assert_eq!(listing.entries.len(), 5000);
        assert_eq!(listing.omitted, 3);
        assert_eq!(listing.skipped, 0);
    }

    /// Scenario: 根目錄以上的 .gitignore 不生效
    #[test]
    fn scenario_gitignore_above_root_does_not_apply() {
        let tmp = TempDir::new("list-outer-gitignore");
        let outer = tmp.path();
        write(outer, ".gitignore", "*.md\n");
        let root = outer.join("repo");
        fs::create_dir_all(&root).expect("建立根目錄");
        write(&root, "README.md", "# hi");

        let listing = list_dir(&root, &rel("")).expect("列出根目錄應成功");

        assert_eq!(names(&listing.entries), vec!["README.md"]);
    }

    /// 非 git 目錄也套用 .gitignore（鎖住 `require_git(false)`：`ignore` 的預設是只在真的 git repo
    /// 內才套用 gitignore 規則，這裡的根目錄完全沒有 `.git`）。
    #[test]
    fn applies_gitignore_even_without_a_git_repo() {
        let tmp = TempDir::new("list-no-git-repo");
        let root = tmp.path();
        write(root, ".gitignore", "ignored.txt\n");
        write(root, "ignored.txt", "x");
        write(root, "kept.txt", "x");

        let listing = list_dir(root, &rel("")).expect("列出根目錄應成功");

        assert_eq!(names(&listing.entries), vec![".gitignore", "kept.txt"]);
    }

    /// 子資料夾自己的 .gitignore：根目錄沒有 `.gitignore`，只有子資料夾有，列出子資料夾時仍套用。
    #[test]
    fn applies_a_subdirectorys_own_gitignore() {
        let tmp = TempDir::new("list-sub-gitignore");
        let root = tmp.path();
        mkdir(root, "sub");
        let sub = root.join("sub");
        write(&sub, ".gitignore", "secret.txt\n");
        write(&sub, "secret.txt", "x");
        write(&sub, "keep.txt", "x");

        let listing = list_dir(root, &rel("sub")).expect("列出子資料夾應成功");

        assert_eq!(names(&listing.entries), vec![".gitignore", "keep.txt"]);
    }

    /// 根目錄是 git repo 時 `.git/info/exclude` 生效。
    #[test]
    fn git_info_exclude_applies_when_root_is_a_git_repo() {
        let tmp = TempDir::new("list-git-exclude");
        let root = tmp.path();
        fs::create_dir_all(root.join(".git").join("info")).expect("建立 .git/info");
        write(&root.join(".git").join("info"), "exclude", "secret.txt\n");
        write(root, "secret.txt", "x");
        write(root, "README.md", "# hi");

        let listing = list_dir(root, &rel("")).expect("列出根目錄應成功");

        assert_eq!(names(&listing.entries), vec!["README.md"]);
    }

    /// 目標是檔案 → WrongKind。
    #[test]
    fn target_that_is_a_file_is_wrong_kind() {
        let tmp = TempDir::new("list-wrong-kind");
        let root = tmp.path();
        write(root, "a.txt", "x");

        match list_dir(root, &rel("a.txt")) {
            Err(FilesError::WrongKind) => {}
            other => panic!("應為 WrongKind，卻得到 {other:?}"),
        }
    }

    /// 非 UTF-8 名稱的項目不列出，計入 `skipped`。以孤立 surrogate（wide `0xD800`）建立檔名，
    /// 這種名稱在 Rust 的 UTF-8 檢查下必然不合法；建不出來的環境就偵測後略過，不假造。
    #[cfg(windows)]
    #[test]
    fn non_utf8_names_are_skipped() {
        use std::ffi::OsString;
        use std::os::windows::ffi::OsStringExt;

        let tmp = TempDir::new("list-skip-non-utf8");
        let root = tmp.path();
        write(root, "normal.txt", "x");

        let wide: Vec<u16> = vec![
            'b' as u16, 'a' as u16, 'd' as u16, 0xD800, // 孤立 high surrogate
        ];
        let bad_name = OsString::from_wide(&wide);
        let bad_path = root.join(&bad_name);
        if let Err(e) = fs::write(&bad_path, "x") {
            eprintln!("這個檔案系統／環境無法建立含孤立 surrogate 的檔名，略過本測試：{e}");
            return;
        }

        let listing = list_dir(root, &rel("")).expect("列出根目錄應成功");

        assert_eq!(names(&listing.entries), vec!["normal.txt"]);
        assert_eq!(listing.skipped, 1);
    }

    /// 排序：資料夾在前、檔案在後，各自名稱不分大小寫；同名只差大小寫時用原字串當第二鍵，
    /// 保證穩定（Windows 檔案系統不分大小寫，無法用真實檔案重現同名只差大小寫，改測純函式）。
    #[test]
    fn sorts_case_insensitively_with_original_string_as_tiebreak() {
        let dirs = vec![
            ("banana".to_string(), "Banana".to_string()),
            ("apple".to_string(), "apple".to_string()),
        ];
        let files = vec![
            ("abc".to_string(), "abc".to_string()),
            ("abc".to_string(), "ABC".to_string()),
        ];

        let (entries, omitted) = build_entries(dirs, files);

        assert_eq!(omitted, 0);
        assert_eq!(
            entries,
            vec![
                Entry {
                    name: "apple".into(),
                    kind: Kind::Dir
                },
                Entry {
                    name: "Banana".into(),
                    kind: Kind::Dir
                },
                Entry {
                    name: "ABC".into(),
                    kind: Kind::File
                },
                Entry {
                    name: "abc".into(),
                    kind: Kind::File
                },
            ]
        );
    }

    /// 祖先鏈上有 junction 時（`resolve` 已確認實體路徑落在根目錄內），仍要能列出目標目錄，
    /// 不能因為 `follow_links(false)` 讓 walker 在連結那層就不往下走而回傳空清單。
    #[cfg(windows)]
    #[test]
    fn ancestor_link_does_not_produce_empty_listing() {
        use std::process::Command;

        let tmp = TempDir::new("list-ancestor-junction");
        let root = tmp.path();
        let real = root.join("real-target");
        fs::create_dir_all(real.join("content")).expect("建立實體目錄");
        write(&real.join("content"), "x.txt", "x");

        let link = root.join("linked");
        let out = Command::new("cmd")
            .args(["/c", "mklink", "/J"])
            .arg(&link)
            .arg(&real)
            .output()
            .expect("執行 cmd mklink /J");
        assert!(
            out.status.success(),
            "mklink /J 失敗：{}",
            String::from_utf8_lossy(&out.stderr)
        );

        let listing =
            list_dir(root, &rel("linked/content")).expect("經祖先鏈上的 junction 列目錄應成功");

        assert_eq!(names(&listing.entries), vec!["x.txt"]);
    }

    // ---- file-review task 2.3 fix round 1（Codex review） ----

    /// [high] 最終目標的實體路徑在根內，不代表沿途每一段連結都在根內：`root/link` 指到根外
    /// `outside`，`outside/back` 又指回 `root/docs`，兩段各自看都「有道理」，但請求 `link/back`
    /// 應該在 `link` 這一步（實體路徑＝`outside`，不在根內）就被擋下，不能因為兜一圈繞回根內就
    /// 放行（回歸測試：`resolve(root, rel)` 只驗最終目標，沒有逐層驗證中途每一段）。
    #[cfg(windows)]
    #[test]
    fn double_junction_escape_is_path_outside_root() {
        use std::process::Command;

        let root_tmp = TempDir::new("list-double-junction-root");
        let outside_tmp = TempDir::new("list-double-junction-outside");
        let root = root_tmp.path();
        let outside = outside_tmp.path();
        mkdir(root, "docs");

        let link = root.join("link");
        let out = Command::new("cmd")
            .args(["/c", "mklink", "/J"])
            .arg(&link)
            .arg(outside)
            .output()
            .expect("執行 cmd mklink /J（root/link -> outside）");
        assert!(
            out.status.success(),
            "mklink /J 失敗：{}",
            String::from_utf8_lossy(&out.stderr)
        );

        let back = outside.join("back");
        let out = Command::new("cmd")
            .args(["/c", "mklink", "/J"])
            .arg(&back)
            .arg(root.join("docs"))
            .output()
            .expect("執行 cmd mklink /J（outside/back -> root/docs）");
        assert!(
            out.status.success(),
            "mklink /J 失敗：{}",
            String::from_utf8_lossy(&out.stderr)
        );

        match list_dir(root, &rel("link/back")) {
            Err(FilesError::PathOutsideRoot) => {}
            other => panic!(
                "雙重 junction 繞回根內應為 PathOutsideRoot（第一段 link 本身就在根外），卻得到 {other:?}"
            ),
        }
    }

    /// [high] 目標目錄本身存在（`fs::metadata` 讀得到），但列舉內容時失敗（例如沒有列目錄權限），
    /// 必須回 `FilesError::Io`，不能吞掉錯誤變成一個看似成功、其實只是空的清單。
    #[cfg(windows)]
    #[test]
    fn unlistable_directory_is_io_error() {
        use std::process::Command;

        let tmp = TempDir::new("list-unlistable");
        let root = tmp.path();
        mkdir(root, "locked");
        let locked = root.join("locked");
        write(&locked, "a.txt", "x");

        let whoami = Command::new("whoami").output().expect("執行 whoami");
        let user = String::from_utf8_lossy(&whoami.stdout).trim().to_string();

        let deny = Command::new("icacls")
            .arg(&locked)
            .args(["/deny", &format!("{user}:(RD)")])
            .output()
            .expect("執行 icacls /deny");
        if !deny.status.success() {
            eprintln!(
                "這個環境無法用 icacls 拒絕列舉權限，略過本測試：{}",
                String::from_utf8_lossy(&deny.stderr)
            );
            return;
        }

        let result = list_dir(root, &rel("locked"));

        let restore = Command::new("icacls")
            .arg(&locked)
            .args(["/remove:d", &user])
            .output();
        assert!(
            matches!(&restore, Ok(o) if o.status.success()),
            "還原 icacls 權限失敗，暫存目錄可能無法正常清除：{restore:?}"
        );

        match result {
            Err(FilesError::Io(_)) => {}
            other => panic!("目錄無法列舉應回 Io，卻得到 {other:?}"),
        }
    }

    /// [medium] `WalkBuilder` 預設會讀 `.ignore` 檔且優先於 `.gitignore`；列目錄端點的過濾來源只
    /// 該是 `.gitignore` 與（根目錄的）`.git/info/exclude`，`.ignore` 的內容（不論排除或白名單）
    /// 都不該影響結果。
    #[test]
    fn dot_ignore_file_does_not_affect_listing() {
        let tmp = TempDir::new("list-dot-ignore-file");
        let root = tmp.path();
        write(root, ".ignore", "README.md\n!target/\n");
        write(root, "README.md", "# hi");
        mkdir(root, "target");

        let listing = list_dir(root, &rel("")).expect("列出根目錄應成功");

        assert_eq!(
            names(&listing.entries),
            vec!["target", ".ignore", "README.md"]
        );
    }

    /// [medium] 祖先鏈若用字面路徑跟實際目錄名稱比對，請求路徑大小寫跟磁碟上不同（Windows 檔案
    /// 系統不分大小寫，這完全合法）就會讓那一層被剪枝、靜默回傳空清單；子資料夾自己的
    /// `.gitignore` 也要照常套用。
    #[cfg(windows)]
    #[test]
    fn ancestor_segment_case_mismatch_still_lists_full_contents() {
        let tmp = TempDir::new("list-case-mismatch");
        let root = tmp.path();
        mkdir(root, "docs");
        let docs = root.join("docs");
        write(&docs, ".gitignore", "secret.txt\n");
        write(&docs, "secret.txt", "x");
        write(&docs, "guide.md", "# g");

        let listing = list_dir(root, &rel("DOCS")).expect("大小寫不同仍應列出");

        assert_eq!(names(&listing.entries), vec![".gitignore", "guide.md"]);
    }

    /// [medium]（延伸）8.3 短檔名出現在請求路徑時，同樣要能穿透別名差異列出完整子項目。
    #[cfg(windows)]
    #[test]
    fn ancestor_segment_8dot3_alias_still_lists_full_contents() {
        use std::os::windows::process::CommandExt;
        use std::path::PathBuf;
        use std::process::Command;

        let tmp = TempDir::new("list-8dot3-alias");
        let root = tmp.path();
        mkdir(root, "long-directory-name-inside");
        let dir = root.join("long-directory-name-inside");
        write(&dir, "note.txt", "n");

        let out = Command::new("cmd")
            .raw_arg(format!(
                "/c for %I in (\"{}\") do @echo %~sI",
                dir.display()
            ))
            .output()
            .expect("執行 cmd 取得 8.3 短名稱");
        let text = String::from_utf8_lossy(&out.stdout);
        let short_path = PathBuf::from(text.trim());
        let short_name = short_path
            .file_name()
            .and_then(|n| n.to_str())
            .map(str::to_string);
        let long_name = dir.file_name().and_then(|n| n.to_str()).map(str::to_string);

        let Some(short_name) =
            short_name.filter(|s| Some(s) != long_name.as_ref() && s.contains('~'))
        else {
            eprintln!("暫存目錄所在磁碟未產生 8.3 短名稱（已停用 8.3），略過此測試");
            return;
        };

        let listing = list_dir(root, &rel(&short_name)).expect("8.3 別名應可列出");

        assert_eq!(names(&listing.entries), vec!["note.txt"]);
    }

    // ---- file-review task 2.3 fix round 2（Codex review） ----

    /// [high] `root/.gitignore` 是指向根外規則檔的 symlink 時，git 本身不跟隨它；我們也不該
    /// 跟隨，否則根外的內容可以透過這個連結決定根內清單要不要顯示某個項目。本機一般帳號沒有建立
    /// 檔案 symlink 的權限（`ERROR_PRIVILEGE_NOT_HELD` = os error 1314）時偵測後略過本測試。
    #[cfg(windows)]
    #[test]
    fn gitignore_symlink_to_outside_file_is_ignored_as_a_rule_source() {
        let root_tmp = TempDir::new("list-gitignore-symlink-root");
        let outside_tmp = TempDir::new("list-gitignore-symlink-outside");
        let root = root_tmp.path();
        let outside = outside_tmp.path();
        write(outside, "outside-rules.txt", "README.md\n");
        write(root, "README.md", "# hi");

        let link = root.join(".gitignore");
        match std::os::windows::fs::symlink_file(outside.join("outside-rules.txt"), &link) {
            Ok(()) => {}
            Err(e) if e.raw_os_error() == Some(1314) => {
                eprintln!("沒有建立檔案 symlink 的權限（os error 1314），略過本測試");
                return;
            }
            Err(e) => panic!("建立 .gitignore symlink 失敗：{e}"),
        }

        let listing = list_dir(root, &rel("")).expect("列出根目錄應成功");

        assert_eq!(names(&listing.entries), vec![".gitignore", "README.md"]);
    }

    /// [high] `root/.git/info` 是指向根外目錄的 junction 時，裡面的 `exclude` 檔案不該生效──
    /// 同一條規則：規則來源本身（含它的上層目錄）是連結就整個略過。junction 不需要特殊權限，
    /// 一律執行。
    #[cfg(windows)]
    #[test]
    fn git_info_junction_to_outside_dir_is_ignored_as_a_rule_source() {
        use std::process::Command;

        let root_tmp = TempDir::new("list-git-info-junction-root");
        let outside_tmp = TempDir::new("list-git-info-junction-outside");
        let root = root_tmp.path();
        let outside = outside_tmp.path();
        mkdir(root, ".git");
        write(outside, "exclude", "README.md\n");
        write(root, "README.md", "# hi");

        let info_link = root.join(".git").join("info");
        let out = Command::new("cmd")
            .args(["/c", "mklink", "/J"])
            .arg(&info_link)
            .arg(outside)
            .output()
            .expect("執行 cmd mklink /J（.git/info -> outside）");
        assert!(
            out.status.success(),
            "mklink /J 失敗：{}",
            String::from_utf8_lossy(&out.stderr)
        );

        let listing = list_dir(root, &rel("")).expect("列出根目錄應成功");

        assert_eq!(names(&listing.entries), vec!["README.md"]);
    }

    /// [medium] 根 `.gitignore` 用 `target/`（排除目錄本身）時，即使 `target` 自己的 `.gitignore`
    /// 想用 `!a.txt` 重新納入，git 語意上父目錄整個被排除就無法復活裡面的檔案：列 `target` 應為空
    /// 清單（成功、不是錯誤）。
    #[test]
    fn ancestor_directory_excluded_by_slash_pattern_cannot_be_reincluded() {
        let tmp = TempDir::new("list-ancestor-excluded-dir");
        let root = tmp.path();
        write(root, ".gitignore", "target/\n");
        mkdir(root, "target");
        let target = root.join("target");
        write(&target, ".gitignore", "!a.txt\n");
        write(&target, "a.txt", "x");

        let listing = list_dir(root, &rel("target")).expect("列出 target 應成功（不是錯誤）");

        assert_eq!(listing.entries, Vec::new());
        assert_eq!(listing.omitted, 0);
        assert_eq!(listing.skipped, 0);
    }

    /// [medium] 根 `.gitignore` 用 `target/*`（排除目錄底下的每個項目，不是目錄本身）搭配
    /// `!target/a.txt` 時，因為 `target` 本身沒被排除，`a.txt` 可以照 git 語意被重新納入：列
    /// `target` 只有 `a.txt`，其餘子項目仍被排除。
    #[test]
    fn ancestor_directory_not_excluded_allows_child_reinclusion() {
        let tmp = TempDir::new("list-ancestor-not-excluded");
        let root = tmp.path();
        write(root, ".gitignore", "target/*\n!target/a.txt\n");
        mkdir(root, "target");
        let target = root.join("target");
        write(&target, "a.txt", "x");
        write(&target, "b.txt", "x");

        let listing = list_dir(root, &rel("target")).expect("列出 target 應成功");

        assert_eq!(names(&listing.entries), vec!["a.txt"]);
    }

    /// [medium] `.git/info/exclude` 排除祖先目錄時，效果跟 `.gitignore` 排除祖先一樣：列出該
    /// 目錄回空清單。
    #[test]
    fn ancestor_directory_excluded_by_git_info_exclude() {
        let tmp = TempDir::new("list-ancestor-excluded-by-exclude");
        let root = tmp.path();
        fs::create_dir_all(root.join(".git").join("info")).expect("建立 .git/info");
        write(&root.join(".git").join("info"), "exclude", "target/\n");
        mkdir(root, "target");
        write(&root.join("target"), "a.txt", "x");

        let listing = list_dir(root, &rel("target")).expect("列出 target 應成功（不是錯誤）");

        assert_eq!(listing.entries, Vec::new());
        assert_eq!(listing.omitted, 0);
        assert_eq!(listing.skipped, 0);
    }
}
