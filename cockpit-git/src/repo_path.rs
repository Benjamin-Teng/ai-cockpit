//! repo 內相對路徑的片段驗證（git-review task 2.1 brief：「`RepoPath` 自己實作與 5a
//! `relpath.rs` 相同的片段規則」）。
//!
//! **刻意重複 `cockpit-files/src/relpath.rs` 的 [`check_segment`] 規則**：`cockpit-git`
//! 依 design D1 不得依賴 `cockpit-files`，所以這裡是縱深防禦的第二道關卡——即使
//! `cockpit`（HTTP 層）用 5a 的 `RelPath` 解碼與檢查有漏洞，本 crate 在把路徑放進
//! git 的 argv 之前仍會再驗一次。輸入是**已經解碼**的相對路徑字串（percent-decoding
//! 是 HTTP 層的職責，不在這裡做）。

use std::fmt;

/// 已通過片段檢查的 repo 內相對路徑（用於 `FileDiff`、`BlobSize`、`Blob` 這些需要指定
/// 檔案的查詢）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoPath {
    segments: Vec<String>,
}

/// [`RepoPath::parse`] 失敗的原因。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepoPathError {
    /// 整體為空字串（git 查詢一定需要指定檔案，不像 5a 的 `RelPath` 可以代表根目錄本身）。
    Empty,
    /// 某一段不符合 [`check_segment`] 的規則。
    BadSegment,
}

impl fmt::Display for RepoPathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RepoPathError::Empty => write!(f, "路徑不可為空"),
            RepoPathError::BadSegment => write!(f, "路徑含不合法的片段"),
        }
    }
}

impl std::error::Error for RepoPathError {}

impl RepoPath {
    /// 解析已解碼的相對路徑字串（以 `/` 分段）。
    pub fn parse(raw: &str) -> Result<RepoPath, RepoPathError> {
        if raw.is_empty() {
            return Err(RepoPathError::Empty);
        }
        let segments = raw
            .split('/')
            .map(|seg| {
                if check_segment(seg) {
                    Ok(seg.to_string())
                } else {
                    Err(RepoPathError::BadSegment)
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(RepoPath { segments })
    }

    /// 還原成 git 用的相對路徑字串（一律 `/` 分隔——git 的 pathspec 與 `<rev>:<path>`
    /// 語法都不接受 `\`，即使在 Windows 上也一樣）。
    pub fn as_git_str(&self) -> String {
        self.segments.join("/")
    }
}

/// 與 `cockpit-files/src/relpath.rs` 的 `check_segment` 逐條相同的規則（見本模組文件
/// 開頭的「刻意重複」說明）。
fn check_segment(segment: &str) -> bool {
    let bad = segment.is_empty()
        || segment == "."
        || segment == ".."
        || segment.contains(['/', '\\', ':', '\0'])
        || segment.ends_with('.')
        || segment.ends_with(char::is_whitespace)
        || is_reserved_device_name(segment);
    !bad
}

/// 去掉副檔名後、不分大小寫是否為 Windows 保留裝置名（與 5a 完全相同的規則）。
fn is_reserved_device_name(segment: &str) -> bool {
    let stem = segment.split('.').next().unwrap_or_default().trim_end();
    let upper = stem.to_ascii_uppercase();
    match upper.as_str() {
        "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$" => true,
        _ => upper
            .strip_prefix("COM")
            .or_else(|| upper.strip_prefix("LPT"))
            .is_some_and(|digit| {
                matches!(
                    digit,
                    "0" | "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
                )
            }),
    }
}

#[cfg(test)]
mod tests {
    use super::{RepoPath, RepoPathError};

    #[test]
    fn parses_plain_relative_path_into_segments() {
        let p = RepoPath::parse("docs/plan.md").expect("應合法");
        assert_eq!(p.as_git_str(), "docs/plan.md");
    }

    #[test]
    fn accepts_non_ascii_segment() {
        let p = RepoPath::parse("中文檔名.txt").expect("應合法");
        assert_eq!(p.as_git_str(), "中文檔名.txt");
    }

    #[test]
    fn rejects_empty_string() {
        assert_eq!(RepoPath::parse(""), Err(RepoPathError::Empty));
    }

    #[test]
    fn rejects_empty_segments() {
        for raw in ["/", "/a", "a/", "a//b", "a/b/"] {
            assert_eq!(
                RepoPath::parse(raw),
                Err(RepoPathError::BadSegment),
                "{raw:?} 應為 BadSegment"
            );
        }
    }

    #[test]
    fn rejects_dot_and_dotdot_segments() {
        for raw in [".", "..", "a/./b", "a/../b"] {
            assert_eq!(
                RepoPath::parse(raw),
                Err(RepoPathError::BadSegment),
                "{raw:?} 應為 BadSegment"
            );
        }
    }

    #[test]
    fn rejects_backslash_colon_and_nul() {
        for raw in ["a\\b", "C:", "a.txt:stream", "a\0b"] {
            assert_eq!(
                RepoPath::parse(raw),
                Err(RepoPathError::BadSegment),
                "{raw:?} 應為 BadSegment"
            );
        }
    }

    #[test]
    fn rejects_segments_ending_with_dot_or_whitespace() {
        for raw in ["a.", "a ", "dir /x", "a.txt."] {
            assert_eq!(
                RepoPath::parse(raw),
                Err(RepoPathError::BadSegment),
                "{raw:?} 應為 BadSegment"
            );
        }
    }

    #[test]
    fn rejects_reserved_device_names_case_insensitively_and_with_extension() {
        for raw in ["CON", "con.txt", "NUL.tar.gz", "COM1", "docs/prn.md"] {
            assert_eq!(
                RepoPath::parse(raw),
                Err(RepoPathError::BadSegment),
                "{raw:?} 應為 BadSegment"
            );
        }
    }

    #[test]
    fn accepts_names_that_merely_contain_a_reserved_device_name() {
        RepoPath::parse("CONSOLE.md").expect("應合法");
        RepoPath::parse("xCON").expect("應合法");
    }
}
