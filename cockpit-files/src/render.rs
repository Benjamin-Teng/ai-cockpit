//! Markdown 渲染（file-review spec「Markdown 渲染端點」；design D5）。
//!
//! comrak 0.55.0，選項出處見各欄位旁註解（docs.rs 與
//! `~/.cargo/registry/src/*/comrak-0.55.0/src/parser/options.rs` 對應行號，task 2.6 report 有完整記錄）。

use std::path::Path;

use comrak::{Options, markdown_to_html};

use crate::{FilesError, RelPath, Viewer, file_meta, read_capped, resolve};

/// 渲染端點的檔案大小上限（spec「Markdown 渲染端點」：超過 2 MiB 回 413 too_large）。
const RENDER_SIZE_LIMIT: u64 = 2 * 1024 * 1024;

/// 標題 `id` 前綴（spec：一律以 `md-` 開頭，避免與頁面既有元素的 `id` 撞名）。
const HEADER_ID_PREFIX: &str = "md-";

/// 查詢 `root` 內 `rel` 指定 Markdown 檔案的渲染結果（HTML 片段）。
///
/// 檢查順序：[`resolve`] 界限（`NotFound`／`PathOutsideRoot`／`BadRequest`）→ 資料夾 →
/// [`FilesError::WrongKind`] → viewer 不是 markdown → [`FilesError::NotMarkdown`] → 大小超過
/// [`RENDER_SIZE_LIMIT`]（用 [`file_meta`] 已經算出的 `size`，只是快速路徑的預檢，不代表最終
/// 保證） → [`FilesError::TooLarge`]；通過後以 [`read_capped`] 讀取內容——這裡才是真正把關的
/// 地方：`file_meta` 的 metadata 檢查與這次讀取之間，檔案可能被追加或替換成更大的內容
/// （TOCTOU；task 2.6 fix round 1 的 Codex review finding），`read_capped` 一律以「實際讀到的
/// 位元組數」而非事先算出的 `size` 判斷是否超過上限，即使預檢當下沒超過，讀取時仍可能因為
/// 檔案在此刻已變大而回 `TooLarge`。內容非法 UTF-8 以替代字元處理，不回錯誤。
pub fn render_markdown(root: &Path, rel: &RelPath) -> Result<String, FilesError> {
    let meta = file_meta(root, rel)?;
    if meta.viewer != Viewer::Markdown {
        return Err(FilesError::NotMarkdown);
    }
    if meta.size > RENDER_SIZE_LIMIT {
        return Err(FilesError::TooLarge);
    }

    let path = resolve(root, rel)?;
    let bytes = read_capped(&path, RENDER_SIZE_LIMIT)?;
    let src = String::from_utf8_lossy(&bytes);
    Ok(render_markdown_str(&src))
}

/// 純函式：把 Markdown 原始字串渲染為 HTML 片段（design D5 comrak 選項）。
///
/// 選項出處（comrak 0.55.0，`~/.cargo/registry/src/index.crates.io-*/comrak-0.55.0/src/parser/options.rs`；
/// 行號為 task 2.6 查證時所見，僅供對照）：
/// - `extension.table`（第 78 行）、`extension.tasklist`（第 110 行）、`extension.strikethrough`
///   （第 45 行）、`extension.autolink`（第 91 行）：GFM 擴充開關。
/// - `extension.header_id_prefix`（第 139 行）：design 寫的 `header_ids` 欄位在這個版本已改名為
///   `header_id_prefix`（`Option<String>`），設為 `Some("md-")` 讓每個標題的 `id` 帶 `md-` 前綴
///   （docs.rs：<https://docs.rs/comrak/0.55.0/comrak/struct.Extension.html#structfield.header_id_prefix>）。
/// - `render.r#unsafe`（第 1148 行）：**不設定，維持 `Options::default()` 的 `false`**——comrak 在
///   `false` 時把原始 HTML block／inline 換成 `<!-- raw HTML omitted -->` 註解（`src/html.rs` 第
///   676～681、724～729 行），且對 `<a href>`／`<img src>` 逐一檢查 `dangerous_url`（`src/html.rs`
///   第 757、834、1615 行），命中危險 scheme 時該屬性留空字串而非省略整個標籤。是否涵蓋 spec 列出的
///   `javascript:`／`vbscript:`／`data:`（圖片以外）等 scheme，由下面測試逐一斷言，不假設。
pub fn render_markdown_str(src: &str) -> String {
    let mut options = Options::default();
    options.extension.table = true;
    options.extension.tasklist = true;
    options.extension.strikethrough = true;
    options.extension.autolink = true;
    options.extension.header_id_prefix = Some(HEADER_ID_PREFIX.to_string());
    markdown_to_html(src, &options)
}

#[cfg(test)]
mod tests {
    use super::{RENDER_SIZE_LIMIT, render_markdown, render_markdown_str};
    use crate::test_support::TempDir;
    use crate::{FilesError, RelPath};
    use std::fs;
    use std::path::Path;

    fn rel(raw: &str) -> RelPath {
        RelPath::parse(raw).expect("測試輸入應合法")
    }

    fn write(dir: &Path, name: &str, content: &[u8]) {
        fs::write(dir.join(name), content).unwrap_or_else(|e| panic!("寫入 {name} 失敗：{e}"));
    }

    /// Scenario: GFM 元素——`doc.md` 含一個表格、一個 `- [x] 完成項` 任務清單與 `~~刪除~~`；
    /// 輸出含 `table` 元素、一個已勾選的 checkbox（`checked` 屬性與 `type="checkbox"`）與 `del` 元素。
    #[test]
    fn scenario_gfm_elements() {
        let src = "| a | b |\n|---|---|\n| c | d |\n\n- [x] 完成項\n- [ ] 未完成項\n\n~~刪除~~\n";
        let html = render_markdown_str(src);
        eprintln!("scenario_gfm_elements html=\n{html}");

        assert!(html.contains("<table"), "html={html}");
        assert!(html.contains("<del>"), "html={html}");

        // 已勾選的 checkbox：同時帶 type="checkbox" 與 checked 屬性。
        let checked_checkbox = "<input type=\"checkbox\" checked=\"\" disabled=\"\" />";
        assert!(
            html.contains(checked_checkbox),
            "應含已勾選的 checkbox：html={html}"
        );
        // 未勾選的 checkbox 不帶 checked。
        let unchecked_checkbox = "<input type=\"checkbox\" disabled=\"\" />";
        assert!(
            html.contains(unchecked_checkbox),
            "應含未勾選的 checkbox：html={html}"
        );
    }

    /// 英文標題的 `id` 以 `md-` 開頭。
    #[test]
    fn heading_id_english_has_md_prefix() {
        let html = render_markdown_str("# README\n");
        eprintln!("heading_id_english html=\n{html}");
        assert!(html.contains(r#"id="md-readme""#), "html={html}");
    }

    /// 中文標題的 `id` 以 `md-` 開頭；comrak 的 anchorize 演算法（`Anchorizer::anchorize`，
    /// `src/html/anchorizer.rs`）只做小寫化＋保留字母／記號／數字／連接標點＋空白轉 `-`，中文字元
    /// 屬於 Unicode `Letter, Other`（`is_letter()` 為真）不受影響，因此 `# 決策` 的 slug 就是
    /// `決策` 原樣（不會被轉成拼音或被移除），完整 id 為 `md-決策`——與前端 D8「`#決策` 轉找
    /// `md-` + `決策` 再用 `CSS.escape`」的假設一致，兩邊不需要額外轉寫規則。
    #[test]
    fn heading_id_chinese_has_md_prefix_and_keeps_characters() {
        let html = render_markdown_str("# 決策\n");
        eprintln!("heading_id_chinese html=\n{html}");
        assert!(html.contains(r#"id="md-決策""#), "html={html}");
    }

    /// 中文標題含空白：空白轉 `-`（GFM 演算法），驗證非純中文字串也遵守同一規則。
    #[test]
    fn heading_id_chinese_with_space_replaces_space_with_dash() {
        let html = render_markdown_str("# 設計 決策\n");
        eprintln!("heading_id_chinese_with_space html=\n{html}");
        assert!(html.contains(r#"id="md-設計-決策""#), "html={html}");
    }

    /// Scenario: 夾帶的 HTML 與危險連結被清掉——`evil.md` 含 `<script>`、`<img onerror>` 與
    /// `[x](javascript:alert(1))`；輸出不含 `<script`、`onerror`、`javascript:`。
    #[test]
    fn scenario_evil_html_and_dangerous_link_are_stripped() {
        let src = "<script>window.pwned=1</script>\n\n<img src=x onerror=alert(1)>\n\n[x](javascript:alert(1))\n";
        let html = render_markdown_str(src);
        eprintln!("scenario_evil_html_and_dangerous_link html=\n{html}");

        assert!(!html.contains("<script"), "html={html}");
        assert!(!html.contains("onerror"), "html={html}");
        assert!(!html.contains("javascript:"), "html={html}");
    }

    /// `vbscript:` 連結不得出現在輸出中。
    #[test]
    fn dangerous_link_vbscript_is_stripped() {
        let html = render_markdown_str("[x](vbscript:msgbox(1))\n");
        eprintln!("dangerous_link_vbscript html=\n{html}");
        assert!(!html.contains("vbscript:"), "html={html}");
    }

    /// `data:text/html` 連結：`href` 不含 `data:`（圖片以外的 data: 一律清掉）。
    #[test]
    fn dangerous_link_data_text_html_href_has_no_data_scheme() {
        let html = render_markdown_str("[x](data:text/html,<script>alert(1)</script>)\n");
        eprintln!("dangerous_link_data_text_html html=\n{html}");
        assert!(!html.contains("data:"), "html={html}");
    }

    /// `data:image/png` 圖片：comrak 明確允許 `data:image/(png|gif|jpeg|webp)` 作為圖片來源
    /// （`src/scanners.re` `dangerous_url` 規則），spec 也只禁「圖片以外」的 `data:`，因此這裡
    /// 記錄並斷言實際行為——`src` 保留原始 `data:image/png` URI。
    #[test]
    fn data_image_png_src_is_allowed_in_practice() {
        let html = render_markdown_str("![p](data:image/png;base64,iVBORw0KGgo=)\n");
        eprintln!("data_image_png html=\n{html}");
        assert!(
            html.contains("src=\"data:image/png;base64,iVBORw0KGgo=\""),
            "comrak 允許 data:image/png 作為 <img src>，html={html}"
        );
    }

    /// 大小寫混用 `JaVaScRiPt:`：comrak 的 scanner 由 `re2c:case-insensitive = 1` 產生
    /// （`src/scanners.re` 檔頭），對危險 scheme 的比對不分大小寫。
    #[test]
    fn dangerous_link_mixed_case_javascript_is_stripped() {
        let html = render_markdown_str("[x](JaVaScRiPt:alert(1))\n");
        eprintln!("dangerous_link_mixed_case html=\n{html}");
        assert!(
            !html.to_ascii_lowercase().contains("javascript:"),
            "html={html}"
        );
    }

    /// scheme 中間夾 tab：`java\tscript:alert(1)`。CommonMark 的裸連結目標（未用 `<>` 包住）
    /// 遇到空白即結束目的地解析，因此這個字串多半不會被解析成完整連結；記錄實際輸出。
    #[test]
    fn dangerous_link_tab_inside_scheme_is_not_a_javascript_link() {
        let html = render_markdown_str("[x](java\tscript:alert(1))\n");
        eprintln!("dangerous_link_tab_inside_scheme html=\n{html}");
        assert!(
            !html.to_ascii_lowercase().contains("javascript:"),
            "html={html}"
        );
    }

    /// HTML 數字字元參照 `&#106;avascript:`（`&#106;` = `j`）：解碼後應視同 `javascript:`。
    #[test]
    fn dangerous_link_numeric_entity_javascript_is_stripped() {
        let html = render_markdown_str("[x](&#106;avascript:alert(1))\n");
        eprintln!("dangerous_link_numeric_entity html=\n{html}");
        assert!(
            !html.to_ascii_lowercase().contains("javascript:"),
            "html={html}"
        );
    }

    /// 前置空白 ` javascript:alert(1)`：CommonMark 目的地解析會先吃掉前導空白，等同
    /// `javascript:alert(1)`。
    #[test]
    fn dangerous_link_leading_space_before_scheme_is_stripped() {
        let html = render_markdown_str("[x]( javascript:alert(1))\n");
        eprintln!("dangerous_link_leading_space html=\n{html}");
        assert!(!html.contains("javascript:"), "html={html}");
    }

    /// CommonMark 自動連結 `<javascript:alert(1)>`（非 GFM autolink 擴充，是核心語法的
    /// `<scheme:...>`）：一樣要走 `dangerous_url` 檢查，`href` 不含 `javascript:`。
    ///
    /// 注意：CommonMark 自動連結以網址本身當連結文字，所以純文字節點（不是屬性）仍會顯示
    /// `javascript:alert(1)` 字樣——這不是 XSS 向量（不是屬性值、不會被瀏覽器當程式碼執行），
    /// spec 管的是「不得出現在輸出的 `href` 或 `src` 中」，所以這裡只斷言 `href`。
    #[test]
    fn commonmark_autolink_javascript_is_stripped() {
        let html = render_markdown_str("<javascript:alert(1)>\n");
        eprintln!("commonmark_autolink_javascript html=\n{html}");
        assert!(html.contains("href=\"\""), "應被清成空 href：html={html}");
        assert!(
            !html.to_ascii_lowercase().contains("href=\"javascript"),
            "html={html}"
        );
    }

    /// GFM autolink 擴充產生的連結（裸 URL，例如 `www.example.com`）也經過相同的輸出管線，
    /// 正常的 `http`/`www` 連結不受影響（沒有被誤清掉）。
    #[test]
    fn gfm_autolink_extension_produces_ordinary_link() {
        let html = render_markdown_str("見 www.example.com 說明。\n");
        eprintln!("gfm_autolink_extension html=\n{html}");
        assert!(
            html.contains("<a href=\"http://www.example.com\">"),
            "html={html}"
        );
    }

    /// 端到端：`render_markdown` 讀真實檔案並回傳與 `render_markdown_str` 相同的渲染結果。
    #[test]
    fn render_markdown_reads_file_and_renders() {
        let tmp = TempDir::new("render-happy-path");
        let root = tmp.path();
        write(root, "README.md", "# 標題\n\n內容。\n".as_bytes());

        let html = render_markdown(root, &rel("README.md")).expect("應可渲染");
        assert!(html.contains(r#"id="md-標題""#), "html={html}");
        assert!(html.contains("<p>內容。</p>"), "html={html}");
    }

    /// viewer 不是 markdown（例如 `.txt`）→ `NotMarkdown`。
    #[test]
    fn non_markdown_viewer_is_not_markdown_error() {
        let tmp = TempDir::new("render-not-markdown");
        let root = tmp.path();
        write(root, "note.txt", b"plain text");

        match render_markdown(root, &rel("note.txt")) {
            Err(FilesError::NotMarkdown) => {}
            other => panic!("應為 NotMarkdown，卻得到 {other:?}"),
        }
    }

    /// 目標是資料夾 → `WrongKind`。
    #[test]
    fn directory_target_is_wrong_kind() {
        let tmp = TempDir::new("render-wrong-kind");
        let root = tmp.path();
        fs::create_dir_all(root.join("sub.md")).expect("建立子目錄");

        match render_markdown(root, &rel("sub.md")) {
            Err(FilesError::WrongKind) => {}
            other => panic!("應為 WrongKind，卻得到 {other:?}"),
        }
    }

    /// 目標不存在 → `NotFound`（沿用 `resolve`／`file_meta` 的界限檢查）。
    #[test]
    fn missing_target_is_not_found() {
        let tmp = TempDir::new("render-not-found");
        let root = tmp.path();

        match render_markdown(root, &rel("nope.md")) {
            Err(FilesError::NotFound) => {}
            other => panic!("應為 NotFound，卻得到 {other:?}"),
        }
    }

    /// 超過 2 MiB（`RENDER_SIZE_LIMIT`）的 `.md` 檔 → `TooLarge`；恰好等於上限則不算超過。
    #[test]
    fn oversized_markdown_file_is_too_large() {
        let tmp = TempDir::new("render-too-large");
        let root = tmp.path();
        let just_over = vec![b'a'; (RENDER_SIZE_LIMIT + 1) as usize];
        write(root, "big.md", &just_over);
        let exactly_at_limit = vec![b'a'; RENDER_SIZE_LIMIT as usize];
        write(root, "exact.md", &exactly_at_limit);

        match render_markdown(root, &rel("big.md")) {
            Err(FilesError::TooLarge) => {}
            other => panic!("應為 TooLarge，卻得到 {other:?}"),
        }

        render_markdown(root, &rel("exact.md")).expect("恰好等於上限不應視為 TooLarge");
    }

    /// 內容非合法 UTF-8 時以替代字元處理，不回錯誤（`.md` 副檔名已足以判定 viewer，
    /// 不受內容影響）。
    #[test]
    fn invalid_utf8_content_is_replaced_not_errored() {
        let tmp = TempDir::new("render-invalid-utf8");
        let root = tmp.path();
        let mut content = b"# ok\n\n".to_vec();
        content.push(0xFF); // 不是合法 UTF-8 起始位元組
        content.extend_from_slice("\n之後還有內容\n".as_bytes());
        write(root, "invalid.md", &content);

        let html = render_markdown(root, &rel("invalid.md")).expect("非法 UTF-8 不應回錯誤");
        eprintln!("invalid_utf8_content html=\n{html}");
        assert!(
            html.contains('\u{FFFD}'),
            "應含 UTF-8 替代字元：html={html}"
        );
        assert!(html.contains("之後還有內容"), "html={html}");
    }
}
