//! 檔案根目錄推算（file-review spec「檔案根目錄與允許清單」、design D1／D2）。

use std::path::{Path, PathBuf};

/// 檔案根目錄推算結果。
///
/// `path` 是**未 canonicalize** 的形式：只對輸入的 `start` 做字面上的逐層 `parent()`，
/// 不展開符號連結或 `..`（design D2：`root_id` 由它推出，必須穩定）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Root {
    pub path: PathBuf,
    pub is_git: bool,
}

/// 由 `start`（含）逐層往上找名為 `.git` 的資料夾或檔案，推算檔案根目錄。
///
/// - `start` 不存在、或不是資料夾 → `None`。
/// - 找到 `.git`（資料夾或檔案）→ 回傳其所在目錄，`is_git = true`。
/// - 到檔案系統頂端都找不到 → 回傳 `start` 本身，`is_git = false`。
pub fn find_root(start: &Path) -> Option<Root> {
    if !start.is_dir() {
        return None;
    }

    let mut current = start;
    loop {
        let git_marker = current.join(".git");
        if git_marker.is_dir() || git_marker.is_file() {
            return Some(Root {
                path: current.to_path_buf(),
                is_git: true,
            });
        }

        match current.parent() {
            Some(parent) => current = parent,
            None => {
                return Some(Root {
                    path: start.to_path_buf(),
                    is_git: false,
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Root, find_root};
    use crate::test_support::TempDir;
    use std::fs;

    /// Scenario: 往上找到 git repo 根目錄
    #[test]
    fn finds_git_root_when_dot_git_is_a_directory_above_start() {
        let tmp = TempDir::new("dir-repo");
        let repo_root = tmp.path().to_path_buf();
        fs::create_dir_all(repo_root.join(".git")).expect("建立 .git 資料夾");
        let deep = repo_root.join("src").join("deep");
        fs::create_dir_all(&deep).expect("建立深層子目錄");

        let root = find_root(&deep).expect("應找到根目錄");

        assert_eq!(
            root,
            Root {
                path: repo_root,
                is_git: true
            }
        );
    }

    /// Scenario: worktree 的 .git 是檔案
    #[test]
    fn finds_git_root_when_dot_git_is_a_file() {
        let tmp = TempDir::new("worktree");
        let wt_root = tmp.path().to_path_buf();
        fs::write(wt_root.join(".git"), "gitdir: ../main/.git/worktrees/wt\n")
            .expect("建立 .git 檔案");

        let root = find_root(&wt_root).expect("應找到根目錄");

        assert_eq!(
            root,
            Root {
                path: wt_root,
                is_git: true
            }
        );
    }

    /// Scenario: 不在 git repo 內
    #[test]
    fn falls_back_to_start_when_no_dot_git_found_up_to_filesystem_top() {
        let tmp = TempDir::new("no-repo");
        let notes = tmp.path().to_path_buf();

        let root = find_root(&notes).expect("即使沒有 .git 也要回傳 start 本身");

        assert_eq!(
            root,
            Root {
                path: notes,
                is_git: false
            }
        );
    }

    /// `start` 路徑不存在 → `None`。
    #[test]
    fn returns_none_when_start_path_does_not_exist() {
        let tmp = TempDir::new("missing");
        let missing = tmp.path().join("does-not-exist");

        assert_eq!(find_root(&missing), None);
    }

    /// `start` 存在但不是資料夾 → `None`。
    #[test]
    fn returns_none_when_start_path_is_not_a_directory() {
        let tmp = TempDir::new("is-file");
        let file = tmp.path().join("notes.txt");
        fs::write(&file, "hello").expect("建立測試檔案");

        assert_eq!(find_root(&file), None);
    }
}
