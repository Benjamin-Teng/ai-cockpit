//! 檔案中繼資料與 `viewer` 分類（file-review spec「中繼資料端點」；`icon`／`vscode_uri`
//! 不在本 task，由 2.5／3.2 處理）。

use std::fs;
use std::io::Read;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;

use crate::{FilesError, RelPath, resolve};

/// 分類判斷只讀檔案前這麼多位元組（spec「中繼資料端點」：不受大小上限限制）。
const CLASSIFY_HEAD_LIMIT: u64 = 8192;

/// 檔案分頁該用哪種檢視器（file-review spec「中繼資料端點」）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Viewer {
    Markdown,
    Pdf,
    Html,
    Text,
    Unsupported,
}

/// 檔案中繼資料（`icon`／`vscode_uri` 由 2.5／3.2 在 `cockpit` 組回應時加上，不在這個 crate）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FileMeta {
    pub size: u64,
    pub modified_ms: i64,
    pub viewer: Viewer,
}

/// 查詢 `root` 內 `rel` 指定檔案的中繼資料。
///
/// 目標解析沿用 [`resolve`]（界限與錯誤沿用它）；目標是資料夾 → [`FilesError::WrongKind`]。
pub fn file_meta(root: &Path, rel: &RelPath) -> Result<FileMeta, FilesError> {
    let path = resolve(root, rel)?;
    let metadata = fs::metadata(&path).map_err(FilesError::Io)?;
    if metadata.is_dir() {
        return Err(FilesError::WrongKind);
    }

    let size = metadata.len();
    let modified_ms = metadata
        .modified()
        .map_err(FilesError::Io)
        .map(system_time_to_epoch_ms)?;
    let viewer = classify_viewer(&path)?;

    Ok(FileMeta {
        size,
        modified_ms,
        viewer,
    })
}

/// `SystemTime` 轉成 Unix epoch 毫秒；早於 epoch 時回傳負值，兩種方向都用
/// `duration_since` 的 `Ok`／`Err` 分支計算，不呼叫任何會 panic 的轉換
/// （`as_millis()` 回傳的 `u128` 用 `try_from` 轉 `i64`，溢位時飽和到邊界，同樣不 panic）。
fn system_time_to_epoch_ms(time: SystemTime) -> i64 {
    match time.duration_since(UNIX_EPOCH) {
        Ok(since_epoch) => i64::try_from(since_epoch.as_millis()).unwrap_or(i64::MAX),
        Err(before_epoch) => {
            let ms = before_epoch.duration().as_millis();
            i64::try_from(ms).map(|ms| -ms).unwrap_or(i64::MIN)
        }
    }
}

/// 依副檔名（不分大小寫，只看最後一段）與前 [`CLASSIFY_HEAD_LIMIT`] 位元組的內容分類 viewer
/// （file-review spec「中繼資料端點」）：讀取判斷所需的位元組後交給 [`classify_viewer_bytes`]
/// ——那裡才是唯一定義這套分類規則的地方，這裡只負責從磁碟拿到 `name`／`head`。
fn classify_viewer(path: &Path) -> Result<Viewer, FilesError> {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    let head = read_head(path, CLASSIFY_HEAD_LIMIT)?;
    Ok(classify_viewer_bytes(name, &head))
}

/// 依副檔名（不分大小寫，只看最後一段）與內容前 [`CLASSIFY_HEAD_LIMIT`] 位元組分類 viewer
/// （file-review spec「中繼資料端點」）：純函式，不碰檔案系統。`name` 只需要是檔名字面（含副
/// 檔名），不必是磁碟路徑；`head` 是判斷用的內容位元組，超過 [`CLASSIFY_HEAD_LIMIT`] 時只取
/// 前段（呼叫端傳更多位元組進來也安全，不會多讀出界）。
///
/// `pub`（git-review task 3.3 fix round 2；Ruling R10）：git-review 的某版本檔案內容端點
/// `meta` 重用這個函式分類 git blob 的內容（內容經 `cockpit-git` 的 `BlobHead` 查詢取得前 8192
/// 位元組，不是磁碟路徑）——這是唯一定義這套分類規則的地方，[`classify_viewer`]（磁碟路徑）與
/// 跨 crate 的呼叫端都呼叫它，不重寫第二份規則。
pub fn classify_viewer_bytes(name: &str, head: &[u8]) -> Viewer {
    if let Some(ext) = lowercase_extension_of_name(name) {
        match ext.as_str() {
            "md" | "markdown" => return Viewer::Markdown,
            "pdf" => return Viewer::Pdf,
            "html" | "htm" => return Viewer::Html,
            _ => {}
        }
    }

    let capped_len = head.len().min(CLASSIFY_HEAD_LIMIT as usize);
    classify_by_content(&head[..capped_len])
}

/// 檔名字面的副檔名（不分大小寫，只看最後一段：`a.md.txt` 是 `txt`，`.gitignore` 沒有副檔名）
/// ——對字面操作，不需要是磁碟路徑，同 [`classify_viewer_bytes`] 的用法。
fn lowercase_extension_of_name(name: &str) -> Option<String> {
    Path::new(name)
        .extension()
        .and_then(|ext| ext.to_str())
        .map(str::to_ascii_lowercase)
}

/// 只讀判斷所需的前 `limit` 位元組，不讀整檔。
fn read_head(path: &Path, limit: u64) -> Result<Vec<u8>, FilesError> {
    let file = fs::File::open(path).map_err(FilesError::Io)?;
    let mut buf = Vec::new();
    file.take(limit)
        .read_to_end(&mut buf)
        .map_err(FilesError::Io)?;
    Ok(buf)
}

/// 空檔 → text；含 NUL → unsupported；合法 UTF-8 → text；結尾被截斷的多位元組字元
/// （`Utf8Error::error_len() == None`）容許 → text；其餘非法 UTF-8 → unsupported。
fn classify_by_content(head: &[u8]) -> Viewer {
    if head.contains(&0) {
        return Viewer::Unsupported;
    }
    match std::str::from_utf8(head) {
        Ok(_) => Viewer::Text,
        Err(err) if err.error_len().is_none() => Viewer::Text,
        Err(_) => Viewer::Unsupported,
    }
}

#[cfg(test)]
mod tests {
    use super::super::{FilesError, RelPath, Viewer, classify_viewer_bytes, file_meta};
    use crate::test_support::TempDir;
    use std::fs;
    use std::path::Path;

    /// git-review task 3.3 fix round 2（Ruling R10）：`classify_viewer_bytes` 是純函式，
    /// 不需要暫存目錄——與 `scenario_classifies_by_extension_and_content` 用同一組輸入，
    /// 證明兩者是同一套規則（`file_meta` 內部就是呼叫這個函式）。
    #[test]
    fn classify_viewer_bytes_matches_extension_and_content_rules() {
        assert_eq!(classify_viewer_bytes("a.MD", b"# hi"), Viewer::Markdown);
        assert_eq!(classify_viewer_bytes("b.pdf", b"%PDF-1.4"), Viewer::Pdf);
        assert_eq!(classify_viewer_bytes("c.htm", b"<p>hi</p>"), Viewer::Html);
        assert_eq!(classify_viewer_bytes("d.toml", b"key = 1"), Viewer::Text);
        assert_eq!(
            classify_viewer_bytes("e.png", b"\x89PNG\0garbage"),
            Viewer::Unsupported
        );
    }

    /// 沒有可辨識副檔名時退回內容判斷：含 NUL → unsupported，合法 UTF-8 → text。
    #[test]
    fn classify_viewer_bytes_without_known_extension_falls_back_to_content() {
        assert_eq!(
            classify_viewer_bytes("noext", b"plain text content"),
            Viewer::Text
        );
        assert_eq!(
            classify_viewer_bytes("noext", b"\x00binary garbage"),
            Viewer::Unsupported
        );
    }

    /// `head` 超過 [`super::CLASSIFY_HEAD_LIMIT`] 時只取前段判斷：超過門檻之後才出現的 NUL
    /// 不影響結果（同磁碟版本「只讀前 8192 位元組」的既有規則）。
    #[test]
    fn classify_viewer_bytes_only_looks_at_head_limit_bytes() {
        let mut head = vec![b'a'; 8192];
        head.push(0);
        assert_eq!(classify_viewer_bytes("noext", &head), Viewer::Text);
    }

    fn rel(raw: &str) -> RelPath {
        RelPath::parse(raw).expect("測試輸入應合法")
    }

    fn write(dir: &Path, name: &str, content: &[u8]) {
        fs::write(dir.join(name), content).unwrap_or_else(|e| panic!("寫入 {name} 失敗：{e}"));
    }

    /// Scenario: 分類（五個檔案逐一斷言）。
    #[test]
    fn scenario_classifies_by_extension_and_content() {
        let tmp = TempDir::new("meta-classify");
        let root = tmp.path();
        write(root, "a.MD", b"# hi");
        write(root, "b.pdf", b"%PDF-1.4");
        write(root, "c.htm", b"<p>hi</p>");
        write(root, "d.toml", b"key = 1");
        write(root, "e.png", b"\x89PNG\0garbage");

        let cases = [
            ("a.MD", Viewer::Markdown),
            ("b.pdf", Viewer::Pdf),
            ("c.htm", Viewer::Html),
            ("d.toml", Viewer::Text),
            ("e.png", Viewer::Unsupported),
        ];
        for (name, expected) in cases {
            let meta = file_meta(root, &rel(name)).unwrap_or_else(|e| panic!("{name}: {e:?}"));
            assert_eq!(meta.viewer, expected, "{name}");
        }
    }

    /// `.markdown`／`.htm`／`.html` 也各自分類正確（spec 除了 `.md`／`.pdf`／`.htm` 之外還列了
    /// 這幾個副檔名）。
    #[test]
    fn classifies_markdown_and_html_alternate_extensions() {
        let tmp = TempDir::new("meta-classify-alt-ext");
        let root = tmp.path();
        write(root, "a.markdown", b"# hi");
        write(root, "b.html", b"<p>hi</p>");
        write(root, "c.MARKDOWN", b"# hi");
        write(root, "d.HTML", b"<p>hi</p>");

        assert_eq!(
            file_meta(root, &rel("a.markdown")).unwrap().viewer,
            Viewer::Markdown
        );
        assert_eq!(
            file_meta(root, &rel("b.html")).unwrap().viewer,
            Viewer::Html
        );
        assert_eq!(
            file_meta(root, &rel("c.MARKDOWN")).unwrap().viewer,
            Viewer::Markdown
        );
        assert_eq!(
            file_meta(root, &rel("d.HTML")).unwrap().viewer,
            Viewer::Html
        );
    }

    /// 副檔名判定只看最後一段：`a.md.txt` 不是 markdown，落到內容判斷（合法文字 → text）。
    #[test]
    fn extension_check_only_looks_at_the_last_segment() {
        let tmp = TempDir::new("meta-last-segment-ext");
        let root = tmp.path();
        write(root, "a.md.txt", b"not markdown by extension");

        let meta = file_meta(root, &rel("a.md.txt")).expect("應可查詢");
        assert_eq!(meta.viewer, Viewer::Text);
    }

    /// 空檔 → text。
    #[test]
    fn empty_file_is_text() {
        let tmp = TempDir::new("meta-empty-file");
        let root = tmp.path();
        write(root, "empty.bin", b"");

        let meta = file_meta(root, &rel("empty.bin")).expect("應可查詢");
        assert_eq!(meta.viewer, Viewer::Text);
    }

    /// Scenario: 修改後中繼資料改變（用不同長度取代依賴時間解析度）。
    #[test]
    fn scenario_metadata_changes_after_modification() {
        let tmp = TempDir::new("meta-changed-after-write");
        let root = tmp.path();
        write(root, "README.md", b"# short");

        let before = file_meta(root, &rel("README.md")).expect("初次查詢應成功");

        write(
            root,
            "README.md",
            b"# a much longer piece of content than before",
        );
        let after = file_meta(root, &rel("README.md")).expect("改寫後查詢應成功");

        assert!(
            before.size != after.size || before.modified_ms != after.modified_ms,
            "size 或 modified_ms 至少一項應不同：before={before:?} after={after:?}"
        );
    }

    /// 截斷 UTF-8 邊界：前 8192 位元組恰好在中文字（3 bytes）中間切斷 → 容許，分類為 text。
    #[test]
    fn truncated_multibyte_char_at_8192_boundary_is_text() {
        let tmp = TempDir::new("meta-truncated-utf8-boundary");
        let root = tmp.path();
        let mut content = vec![b'a'; 8191];
        content.extend_from_slice("中".as_bytes()); // 3 bytes：位置 8191..8194
        content.extend_from_slice(b"more content after the cut, not read");
        assert!(content.len() > 8192, "測試前提：檔案要大於 8192 位元組");
        write(root, "boundary.dat", &content);

        let meta = file_meta(root, &rel("boundary.dat")).expect("應可查詢");
        assert_eq!(meta.viewer, Viewer::Text);
    }

    /// 非法 UTF-8（例如 0xFF）在前段（非結尾截斷）→ unsupported。
    #[test]
    fn invalid_utf8_byte_in_head_is_unsupported() {
        let tmp = TempDir::new("meta-invalid-utf8");
        let root = tmp.path();
        let mut content = vec![0xFFu8];
        content.extend_from_slice(b"rest of the file content");
        write(root, "invalid.dat", &content);

        let meta = file_meta(root, &rel("invalid.dat")).expect("應可查詢");
        assert_eq!(meta.viewer, Viewer::Unsupported);
    }

    /// 超過 8192 之後才出現 NUL → text（證明只讀前 8192 位元組）。
    #[test]
    fn nul_after_8192_bytes_is_not_seen() {
        let tmp = TempDir::new("meta-nul-after-8192");
        let root = tmp.path();
        let mut content = vec![b'a'; 8192];
        content.push(0);
        content.extend_from_slice(b"more");
        write(root, "late-nul.dat", &content);

        let meta = file_meta(root, &rel("late-nul.dat")).expect("應可查詢");
        assert_eq!(meta.viewer, Viewer::Text);
    }

    /// NUL 出現在前 8192 位元組內 → unsupported。
    #[test]
    fn nul_within_first_8192_bytes_is_unsupported() {
        let tmp = TempDir::new("meta-nul-within-8192");
        let root = tmp.path();
        let mut content = vec![b'a'; 100];
        content.push(0);
        write(root, "early-nul.dat", &content);

        let meta = file_meta(root, &rel("early-nul.dat")).expect("應可查詢");
        assert_eq!(meta.viewer, Viewer::Unsupported);
    }

    /// 目標是資料夾 → WrongKind。
    #[test]
    fn directory_target_is_wrong_kind() {
        let tmp = TempDir::new("meta-wrong-kind");
        let root = tmp.path();
        fs::create_dir_all(root.join("sub")).expect("建立子目錄");

        match file_meta(root, &rel("sub")) {
            Err(FilesError::WrongKind) => {}
            other => panic!("應為 WrongKind，卻得到 {other:?}"),
        }
    }

    /// 不存在 → NotFound。
    #[test]
    fn missing_target_is_not_found() {
        let tmp = TempDir::new("meta-not-found");
        let root = tmp.path();

        match file_meta(root, &rel("nope.txt")) {
            Err(FilesError::NotFound) => {}
            other => panic!("應為 NotFound，卻得到 {other:?}"),
        }
    }

    /// `size` 對應實際位元組數。
    #[test]
    fn size_matches_actual_byte_length() {
        let tmp = TempDir::new("meta-size");
        let root = tmp.path();
        write(root, "sized.txt", b"12345");

        let meta = file_meta(root, &rel("sized.txt")).expect("應可查詢");
        assert_eq!(meta.size, 5);
    }
}
