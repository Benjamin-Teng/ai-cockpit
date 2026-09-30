//! git refname 的驗證（git-review design D5：「Graph 的分支篩選則送完整 refname，伺服器
//! 以當下 `Refs` 的結果逐字比對」）。
//!
//! **本 task（2.1）尚未有任何 [`crate::GitQuery`] 把 `RefName` 放進 argv**——design D5
//! 的比對是「逐字比對」，比對本身在呼叫端（之後的 task）做；`RefName` 在這裡先提供
//! 型別與基本語法驗證，供 2.3 解析 `Refs` 輸出、以及之後 task 驗證前端送來的 ref
//! 候選字串時使用，減少往下游傳遞明顯不是 refname 的字串（例如含控制字元、空白）。
//! 驗證規則依 `git-check-ref-format(1)` 的核心規則取保守子集（拒絕比放行更安全）：
//! 不得為空、不得含控制字元或空白、不得含 `..`、不得以 `/` 開頭或結尾、不得有連續
//! `//`、任何片段不得以 `.` 開頭或以 `.lock` 結尾、不得含 `~^:?*[\`、不得為單一 `@`。
//! 這是本 crate 自己的保守檢查，**不是** git 本身 `check-ref-format` 的完整重現——因為
//! `RefName` 從不被當成 argv 傳給 git（design D5：實際交給 git 的一律是先解析出的
//! commit hash），這裡只是防止明顯不像 refname 的字串流到下游比對邏輯。

use std::fmt;

/// 已通過基本語法檢查的完整 refname（例如 `refs/heads/main`）。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RefName(String);

/// [`RefName::parse`] 失敗的原因。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefNameError {
    Empty,
    ControlOrWhitespace,
    ContainsDotDot,
    LeadingOrTrailingSlash,
    ConsecutiveSlash,
    SegmentStartsWithDot,
    SegmentEndsWithDotLock,
    ForbiddenChar,
    SingleAt,
}

impl fmt::Display for RefNameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let msg = match self {
            RefNameError::Empty => "refname 不可為空",
            RefNameError::ControlOrWhitespace => "refname 不可含控制字元或空白",
            RefNameError::ContainsDotDot => "refname 不可含 ..",
            RefNameError::LeadingOrTrailingSlash => "refname 不可以 / 開頭或結尾",
            RefNameError::ConsecutiveSlash => "refname 不可含連續的 /",
            RefNameError::SegmentStartsWithDot => "refname 的片段不可以 . 開頭",
            RefNameError::SegmentEndsWithDotLock => "refname 的片段不可以 .lock 結尾",
            RefNameError::ForbiddenChar => "refname 含不允許的字元",
            RefNameError::SingleAt => "refname 不可為單一 @",
        };
        f.write_str(msg)
    }
}

impl std::error::Error for RefNameError {}

const FORBIDDEN_CHARS: [char; 6] = ['~', '^', ':', '?', '*', '['];

impl RefName {
    /// 驗證並建立一個 `RefName`。
    pub fn parse(raw: &str) -> Result<RefName, RefNameError> {
        if raw.is_empty() {
            return Err(RefNameError::Empty);
        }
        if raw == "@" {
            return Err(RefNameError::SingleAt);
        }
        if raw.chars().any(|c| c.is_whitespace() || (c.is_control())) {
            return Err(RefNameError::ControlOrWhitespace);
        }
        if raw.contains("..") {
            return Err(RefNameError::ContainsDotDot);
        }
        if raw.starts_with('/') || raw.ends_with('/') {
            return Err(RefNameError::LeadingOrTrailingSlash);
        }
        if raw.contains("//") {
            return Err(RefNameError::ConsecutiveSlash);
        }
        if raw.contains(FORBIDDEN_CHARS) || raw.contains('\\') {
            return Err(RefNameError::ForbiddenChar);
        }
        for segment in raw.split('/') {
            if segment.starts_with('.') {
                return Err(RefNameError::SegmentStartsWithDot);
            }
            if segment.ends_with(".lock") {
                return Err(RefNameError::SegmentEndsWithDotLock);
            }
        }
        Ok(RefName(raw.to_string()))
    }

    /// 底層字串。
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RefName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::{RefName, RefNameError};

    #[test]
    fn accepts_typical_refnames() {
        for raw in [
            "refs/heads/main",
            "refs/heads/feat/x",
            "refs/remotes/origin/main",
            "refs/tags/v0.1",
            "HEAD",
        ] {
            RefName::parse(raw).unwrap_or_else(|e| panic!("{raw:?} 應合法，卻是 {e:?}"));
        }
    }

    #[test]
    fn rejects_empty() {
        assert_eq!(RefName::parse(""), Err(RefNameError::Empty));
    }

    #[test]
    fn rejects_whitespace_and_control_chars() {
        for raw in ["refs/heads/a b", "refs/heads/a\tb", "refs/heads/a\nb"] {
            assert_eq!(RefName::parse(raw), Err(RefNameError::ControlOrWhitespace));
        }
    }

    #[test]
    fn rejects_dotdot() {
        assert_eq!(
            RefName::parse("refs/heads/a..b"),
            Err(RefNameError::ContainsDotDot)
        );
    }

    #[test]
    fn rejects_leading_or_trailing_slash() {
        for raw in ["/refs/heads/main", "refs/heads/main/"] {
            assert_eq!(
                RefName::parse(raw),
                Err(RefNameError::LeadingOrTrailingSlash)
            );
        }
    }

    #[test]
    fn rejects_consecutive_slash() {
        assert_eq!(
            RefName::parse("refs//heads/main"),
            Err(RefNameError::ConsecutiveSlash)
        );
    }

    #[test]
    fn rejects_segment_starting_with_dot() {
        assert_eq!(
            RefName::parse("refs/heads/.hidden"),
            Err(RefNameError::SegmentStartsWithDot)
        );
    }

    #[test]
    fn rejects_segment_ending_with_dot_lock() {
        assert_eq!(
            RefName::parse("refs/heads/main.lock"),
            Err(RefNameError::SegmentEndsWithDotLock)
        );
    }

    #[test]
    fn rejects_forbidden_chars() {
        for raw in [
            "refs/heads/a~b",
            "refs/heads/a^b",
            "refs/heads/a:b",
            "refs/heads/a?b",
            "refs/heads/a*b",
            "refs/heads/a[b",
            "refs/heads/a\\b",
        ] {
            assert_eq!(
                RefName::parse(raw),
                Err(RefNameError::ForbiddenChar),
                "{raw:?} 應為 ForbiddenChar"
            );
        }
    }

    #[test]
    fn rejects_single_at() {
        assert_eq!(RefName::parse("@"), Err(RefNameError::SingleAt));
    }
}
