//! 檔案端點共用的錯誤型別（file-review spec「檔案端點的共同規則」）。
//!
//! 每個變體對應 spec 的一個 `code`；HTTP 狀態碼的對應由 `cockpit` 負責，這個 crate 不談 HTTP。
//! 錯誤一律不夾帶請求路徑的片段原文。

/// 檔案端點的錯誤。
#[derive(Debug)]
pub enum FilesError {
    BadRequest,
    PathOutsideRoot,
    NotFound,
    WrongKind,
    TooLarge,
    NotMarkdown,
    Io(std::io::Error),
}

impl FilesError {
    /// spec「檔案端點的共同規則」的錯誤代碼。
    pub fn code(&self) -> &'static str {
        match self {
            FilesError::BadRequest => "bad_request",
            FilesError::PathOutsideRoot => "path_outside_root",
            FilesError::NotFound => "not_found",
            FilesError::WrongKind => "wrong_kind",
            FilesError::TooLarge => "too_large",
            FilesError::NotMarkdown => "not_markdown",
            FilesError::Io(_) => "io_error",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::FilesError;

    #[test]
    fn code_matches_spec_error_codes() {
        let cases = [
            (FilesError::BadRequest, "bad_request"),
            (FilesError::PathOutsideRoot, "path_outside_root"),
            (FilesError::NotFound, "not_found"),
            (FilesError::WrongKind, "wrong_kind"),
            (FilesError::TooLarge, "too_large"),
            (FilesError::NotMarkdown, "not_markdown"),
            (FilesError::Io(std::io::Error::other("x")), "io_error"),
        ];
        for (err, code) in cases {
            assert_eq!(err.code(), code, "{err:?}");
        }
    }
}
