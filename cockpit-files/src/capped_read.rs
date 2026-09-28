//! 有上限的檔案讀取（file-review task 2.6 fix round 1：Codex review 指出 metadata 大小預檢
//! 與實際讀取之間存在 TOCTOU——檔案可能在兩者之間被追加或替換成更大的內容，先前算出的
//! `size` 就不再準確）。
//!
//! [`read_capped`] 一律用 [`Read::take`] 界限實際讀取的位元組數（最多 `limit + 1`），不論
//! metadata 說了什麼；只要讀到的位元組數真的超過 `limit` 就回 [`FilesError::TooLarge`]，
//! 不會把超過上限的內容交給呼叫端（例如 markdown 渲染或 3.2 的原始內容端點）。

use std::fs;
use std::io::Read;
use std::path::Path;

use crate::FilesError;

/// 開啟 `path` 並讀取內容，最多讀 `limit + 1` 個位元組；讀到的位元組數超過 `limit` →
/// [`FilesError::TooLarge`]（不管 metadata 回報的 `size` 是多少，一律以實際讀到的位元組數為準，
/// 避免「metadata 檢查之後、真正讀取之前，檔案被追加或替換」的 TOCTOU）。
pub fn read_capped(path: &Path, limit: u64) -> Result<Vec<u8>, FilesError> {
    let file = fs::File::open(path).map_err(FilesError::Io)?;
    let mut buf = Vec::new();
    file.take(limit + 1)
        .read_to_end(&mut buf)
        .map_err(FilesError::Io)?;
    if buf.len() as u64 > limit {
        return Err(FilesError::TooLarge);
    }
    Ok(buf)
}

#[cfg(test)]
mod tests {
    use super::read_capped;
    use crate::FilesError;
    use crate::test_support::TempDir;
    use std::fs;

    /// 實際位元組數超過 `limit` → `TooLarge`（不管 metadata 事先算出什麼）。
    #[test]
    fn content_larger_than_limit_is_too_large() {
        let tmp = TempDir::new("capped-read-too-large");
        let root = tmp.path();
        let path = root.join("big.bin");
        fs::write(&path, vec![b'a'; 17]).expect("寫入測試檔");

        match read_capped(&path, 16) {
            Err(FilesError::TooLarge) => {}
            other => panic!("應為 TooLarge，卻得到 {other:?}"),
        }
    }

    /// 恰好等於 `limit` → 成功，回傳完整內容。
    #[test]
    fn content_exactly_at_limit_is_ok() {
        let tmp = TempDir::new("capped-read-exact");
        let root = tmp.path();
        let path = root.join("exact.bin");
        let content = vec![b'a'; 16];
        fs::write(&path, &content).expect("寫入測試檔");

        let read = read_capped(&path, 16).expect("恰好等於上限不應視為 TooLarge");
        assert_eq!(read, content);
    }

    /// 小於 `limit` → 成功，回傳完整內容（不會被截斷成 `limit` 長度）。
    #[test]
    fn content_smaller_than_limit_returns_full_content() {
        let tmp = TempDir::new("capped-read-small");
        let root = tmp.path();
        let path = root.join("small.bin");
        let content = vec![b'a'; 3];
        fs::write(&path, &content).expect("寫入測試檔");

        let read = read_capped(&path, 16).expect("應可讀取");
        assert_eq!(read, content);
    }

    /// 不存在的檔案 → `Io`。
    #[test]
    fn missing_file_is_io_error() {
        let tmp = TempDir::new("capped-read-missing");
        let root = tmp.path();
        let path = root.join("nope.bin");

        match read_capped(&path, 16) {
            Err(FilesError::Io(_)) => {}
            other => panic!("應為 Io，卻得到 {other:?}"),
        }
    }
}
