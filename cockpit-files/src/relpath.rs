//! 相對路徑解析與根目錄界限檢查（file-review design D3、spec「檔案端點的共同規則」）。
//!
//! 兩道關卡：
//! 1. [`RelPath::parse`]：在碰檔案系統之前，逐段 percent-decode 並套用 spec 的片段規則。
//! 2. [`resolve`]：根目錄與目標各自 `canonicalize`（展開符號連結、junction、8.3 短名稱，統一大小寫
//!    與 `\\?\` 前綴），再以 `Path::starts_with` 逐 component 判斷目標在根目錄內。

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::FilesError;

/// 已通過片段檢查的根目錄內相對路徑（片段皆已解碼）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelPath {
    segments: Vec<String>,
}

impl RelPath {
    /// 解析仍為 percent-encoded 的相對路徑原始字串（HTTP 層從原始 URI 取出，不先解碼）。
    ///
    /// - 空字串＝根目錄本身（`segments()` 為空）。
    /// - 以 `/` 切段 → 逐段 percent-decode（`%` 後不是兩位十六進位、或解碼後不是合法 UTF-8 →
    ///   `BadRequest`）→ 逐段套用 [`check_segment`] 的規則。
    pub fn parse(raw: &str) -> Result<RelPath, FilesError> {
        if raw.is_empty() {
            return Ok(RelPath {
                segments: Vec::new(),
            });
        }
        let segments = raw
            .split('/')
            .map(|seg| {
                let decoded = percent_decode(seg)?;
                check_segment(&decoded)?;
                Ok(decoded)
            })
            .collect::<Result<Vec<_>, FilesError>>()?;
        Ok(RelPath { segments })
    }

    /// 解碼後的各片段；根目錄本身為空。
    pub fn segments(&self) -> &[String] {
        &self.segments
    }

    /// 由已經過檢查的片段直接建構（例如某個 `RelPath` 的片段前綴）；供同一 crate 內需要
    /// 「逐層做界限檢查」的場合重用 [`resolve`]（file-review task 2.3 fix round 1）。
    pub(crate) fn from_segments(segments: Vec<String>) -> RelPath {
        RelPath { segments }
    }
}

/// 把 `rel` 解析為根目錄內的實體路徑（canonicalize 後的形式，之後開檔用它）。
///
/// - 根目錄 canonicalize 失敗：不存在 → `NotFound`，其他 → `Io`。
/// - 目標的實體路徑不在根目錄的實體路徑之內 → `PathOutsideRoot`。
/// - 目標 canonicalize 失敗時由 [`classify_unresolved`] 分類，重點是結果不得隨根外檔案是否存在
///   而改變（file-review task 2.2 fix round 1／2）。
///
/// 錯誤不夾帶任何路徑。
pub fn resolve(root: &Path, rel: &RelPath) -> Result<PathBuf, FilesError> {
    let real_root = fs::canonicalize(root).map_err(from_io)?;
    let mut target = root.to_path_buf();
    target.extend(&rel.segments);
    let real_target = match fs::canonicalize(&target) {
        Ok(real) => real,
        Err(err) => return Err(classify_unresolved(&target, &real_root, err)),
    };
    if real_target.starts_with(&real_root) {
        Ok(real_target)
    } else {
        Err(FilesError::PathOutsideRoot)
    }
}

/// 目標無法 canonicalize 時，從目標往上逐層檢查到第一個可解析的祖先（控制端裁決 R11）：
///
/// - 某層 `symlink_metadata` 看得到、卻無法解析，且它是連結／junction／其他 reparse point（懸空）
///   → `PathOutsideRoot`，即使它原本指向根內也一樣（一致拒絕優先於精準分類）。看得到的一般項目卻
///   回 NotFound（無從解釋）也同樣拒絕。
/// - 第一個可解析的祖先在根外 → `PathOutsideRoot`（優先於下面的 `Io`，免得以 500／403 探測根外）。
/// - 其餘情況：沿途 canonicalize 或 `symlink_metadata` 出現過非 NotFound 的錯誤 → `Io`（不吞掉）；
///   否則未解析的每一段都真的不存在、且可解析的祖先在根內 → `NotFound`。
fn classify_unresolved(target: &Path, real_root: &Path, first_err: io::Error) -> FilesError {
    let mut io_err: Option<io::Error> = None;
    let mut err = first_err;
    let mut current = target;
    loop {
        let err_is_not_found = err.kind() == io::ErrorKind::NotFound;
        if !err_is_not_found {
            io_err.get_or_insert(err);
        }
        match fs::symlink_metadata(current) {
            Ok(meta) if is_link_like(&meta) || err_is_not_found => {
                return FilesError::PathOutsideRoot;
            }
            Ok(_) => {}
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => {
                io_err.get_or_insert(e);
            }
        }
        let Some(parent) = current.parent() else {
            break;
        };
        current = parent;
        match fs::canonicalize(current) {
            Ok(real) if !real.starts_with(real_root) => return FilesError::PathOutsideRoot,
            Ok(_) => break,
            Err(e) => err = e,
        }
    }
    io_err.map_or(FilesError::NotFound, FilesError::Io)
}

/// 連結、junction 或其他 reparse point。
pub(crate) fn is_link_like(meta: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
        meta.file_type().is_symlink() || meta.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    }
    #[cfg(not(windows))]
    {
        meta.file_type().is_symlink()
    }
}

fn from_io(err: io::Error) -> FilesError {
    if err.kind() == io::ErrorKind::NotFound {
        FilesError::NotFound
    } else {
        FilesError::Io(err)
    }
}

/// 手寫的 percent-decoding（ledger R5：不加 crate）。`+` 不是空白（這是路徑，不是 query）。
fn percent_decode(segment: &str) -> Result<String, FilesError> {
    let bytes = segment.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hi = bytes.get(i + 1).and_then(|b| hex_value(*b));
            let lo = bytes.get(i + 2).and_then(|b| hex_value(*b));
            let (Some(hi), Some(lo)) = (hi, lo) else {
                return Err(FilesError::BadRequest);
            };
            out.push(hi << 4 | lo);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).map_err(|_| FilesError::BadRequest)
}

fn hex_value(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

/// spec「檔案端點的共同規則」的片段規則（對解碼後的片段）。
fn check_segment(segment: &str) -> Result<(), FilesError> {
    let bad = segment.is_empty()
        || segment == "."
        || segment == ".."
        || segment.contains(['/', '\\', ':', '\0'])
        || segment.ends_with('.')
        || segment.ends_with(char::is_whitespace)
        || is_reserved_device_name(segment);
    if bad {
        Err(FilesError::BadRequest)
    } else {
        Ok(())
    }
}

/// 去掉副檔名後、不分大小寫是否為 Windows 保留裝置名。
///
/// 「去掉副檔名」依 Win32 的實際行為取寬：取**第一個** `.` 之前的主檔名（`NUL.tar.gz` 視同 `NUL`），
/// 再去掉主檔名結尾的空白（`CON .txt` 視同 `CON`）。COM／LPT 的數字除 `1`–`9` 外也含 `0` 與
/// ISO 8859-1 上標數字 `¹²³`（Win32 把它們當數字，見 Microsoft Learn「Naming Files, Paths, and
/// Namespaces」）。
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
    use super::{RelPath, resolve};
    use crate::FilesError;
    use crate::test_support::TempDir;
    use std::fs;
    use std::path::{Path, PathBuf};

    fn segments(raw: &str) -> Vec<String> {
        RelPath::parse(raw)
            .unwrap_or_else(|e| panic!("{raw:?} 應合法，卻得到 {e:?}"))
            .segments()
            .to_vec()
    }

    fn assert_bad_request(raw: &str) {
        match RelPath::parse(raw) {
            Err(FilesError::BadRequest) => {}
            other => panic!("{raw:?} 應為 BadRequest，卻得到 {other:?}"),
        }
    }

    // ---- RelPath::parse：合法輸入與 percent-decoding ----

    /// 空字串＝根目錄本身（給列目錄用）。
    #[test]
    fn empty_raw_is_the_root_itself() {
        assert!(segments("").is_empty());
    }

    #[test]
    fn splits_on_slash_into_segments() {
        assert_eq!(
            segments("docs/guide/readme.md"),
            ["docs", "guide", "readme.md"]
        );
    }

    #[test]
    fn percent_decodes_each_segment_including_utf8_and_lowercase_hex() {
        assert_eq!(segments("a%20b/%E4%B8%AD%e6%96%87.md"), ["a b", "中文.md"]);
    }

    #[test]
    fn keeps_already_decoded_non_ascii_and_allowed_punctuation() {
        assert_eq!(
            segments("中文/.hidden/PROGRA~1/a .txt/%25.md"),
            ["中文", ".hidden", "PROGRA~1", "a .txt", "%.md"]
        );
    }

    /// 裝置名只看去掉副檔名後的主檔名是否「等於」保留字，前後綴不同的名稱合法。
    #[test]
    fn accepts_names_that_merely_contain_a_reserved_device_name() {
        assert_eq!(
            segments("CONSOLE.md/xCON/COM10/LPT/con_fig.txt/.con/nul-report"),
            [
                "CONSOLE.md",
                "xCON",
                "COM10",
                "LPT",
                "con_fig.txt",
                ".con",
                "nul-report"
            ]
        );
    }

    #[test]
    fn rejects_malformed_percent_escapes() {
        for raw in ["%", "a%", "a%4", "%zz", "%4g", "%%41", "a/%G0"] {
            assert_bad_request(raw);
        }
    }

    /// spec：路徑片段不是合法 UTF-8 → bad_request。
    #[test]
    fn rejects_segments_that_decode_to_invalid_utf8() {
        for raw in ["%FF", "%C3", "%E4%B8", "ok/%80.md"] {
            assert_bad_request(raw);
        }
    }

    // ---- RelPath::parse：spec 片段規則 ----

    /// Scenario: 用 .. 跳出根目錄（spec 列出的五個輸入）
    #[test]
    fn scenario_dotdot_escape_inputs_are_bad_request() {
        for raw in [
            "docs/../../secret.txt",
            "..%2F..%2Fsecret.txt",
            "C:%5Cx",
            "CON.txt",
            "a.",
        ] {
            assert_bad_request(raw);
        }
    }

    /// 空片段：開頭、結尾、中間的 `/`。
    #[test]
    fn rejects_empty_segments() {
        for raw in ["/", "/a", "a/", "a//b", "a/b/"] {
            assert_bad_request(raw);
        }
    }

    #[test]
    fn rejects_dot_and_dotdot_segments_including_encoded_forms() {
        for raw in [".", "..", "a/./b", "a/../b", "%2E", "%2e%2E", "a/%2E%2E/b"] {
            assert_bad_request(raw);
        }
    }

    /// `%2F` 解碼後的 `/` 不得繞過逐段檢查。
    #[test]
    fn rejects_slash_decoded_from_percent_2f() {
        for raw in ["a%2Fb", "a%2fb", "%2Fetc"] {
            assert_bad_request(raw);
        }
    }

    #[test]
    fn rejects_backslash_and_colon() {
        for raw in [
            "a%5Cb",
            "%5C%5Cserver",
            "C:",
            "a.txt:stream",
            "a.txt%3A%3A$DATA",
        ] {
            assert_bad_request(raw);
        }
    }

    #[test]
    fn rejects_nul_character() {
        for raw in ["%00", "a%00.txt", "ok/b%00"] {
            assert_bad_request(raw);
        }
    }

    #[test]
    fn rejects_segments_ending_with_dot_or_whitespace() {
        for raw in [
            "a.", "a..", "...", "a%20", "dir%20/x", "a%09", "a.txt.", "x/%20",
        ] {
            assert_bad_request(raw);
        }
    }

    /// spec：去掉副檔名後、不分大小寫是 Windows 保留裝置名。
    #[test]
    fn rejects_reserved_device_names_case_insensitively() {
        for raw in [
            "CON", "con", "Con", "PRN", "aux", "NUL", "COM1", "com9", "LPT1", "lpt9", "CONIN$",
            "conin$", "CONOUT$", "Conout$",
        ] {
            assert_bad_request(raw);
        }
    }

    /// 「去掉副檔名」＝取第一個 `.` 之前的主檔名（Win32 對 `NUL.tar.gz` 也視同 `NUL`），
    /// 再去掉主檔名結尾的空白（Win32 對 `CON .txt` 也視同 `CON`）。
    #[test]
    fn rejects_reserved_device_names_with_any_extension() {
        for raw in [
            "CON.txt",
            "con.txt",
            "CON.tar.gz",
            "nul.",
            "Aux.md",
            "com1.log",
            "LPT9.x.y",
            "CONIN$.txt",
            "conout$.md",
            "CON%20.txt",
            "NUL%20%20.tar.gz",
            "docs/prn.md",
        ] {
            assert_bad_request(raw);
        }
    }

    /// Win32 把 ISO 8859-1 上標數字 ¹²³ 視為 COM#／LPT# 的數字（Microsoft Learn「Naming Files,
    /// Paths, and Namespaces」）；COM0／LPT0 曾列在同一份文件的舊版清單，一併擋下。
    #[test]
    fn rejects_superscript_and_zero_com_lpt_variants() {
        for raw in [
            "COM%C2%B9",
            "com%C2%B2.txt",
            "LPT%C2%B3",
            "COM¹",
            "lpt².md",
            "COM0",
            "lpt0.txt",
        ] {
            assert_bad_request(raw);
        }
    }

    // ---- resolve：界限檢查 ----

    /// 以根目錄下的 `root` 子目錄當根，同層放 `root-outside`（字串上以根目錄路徑為前綴，
    /// 用來確認界限判斷是逐 component 而不是字串前綴）。
    struct Fixture {
        _tmp: TempDir,
        root: PathBuf,
        outside: PathBuf,
    }

    fn fixture(tag: &str) -> Fixture {
        let tmp = TempDir::new(tag);
        let root = tmp.path().join("root");
        let outside = tmp.path().join("root-outside");
        fs::create_dir_all(root.join("docs")).expect("建立根目錄");
        fs::create_dir_all(&outside).expect("建立根目錄外資料夾");
        fs::write(root.join("docs").join("readme.md"), "# hi").expect("寫入根內檔案");
        fs::write(outside.join("secret.txt"), "secret").expect("寫入根外檔案");
        Fixture {
            _tmp: tmp,
            root,
            outside,
        }
    }

    fn rel(raw: &str) -> RelPath {
        RelPath::parse(raw).expect("測試輸入應合法")
    }

    fn canonical(p: &Path) -> PathBuf {
        fs::canonicalize(p).expect("canonicalize 測試路徑")
    }

    #[test]
    fn resolves_empty_rel_to_canonical_root() {
        let f = fixture("resolve-root");
        let got = resolve(&f.root, &rel("")).expect("根目錄本身應可解析");
        assert_eq!(got, canonical(&f.root));
    }

    #[test]
    fn resolves_file_inside_root_to_canonical_path() {
        let f = fixture("resolve-inside");
        let got = resolve(&f.root, &rel("docs/readme.md")).expect("根內檔案應可解析");
        assert_eq!(got, canonical(&f.root.join("docs").join("readme.md")));
    }

    #[test]
    fn missing_target_is_not_found() {
        let f = fixture("resolve-missing");
        match resolve(&f.root, &rel("docs/nope.md")) {
            Err(FilesError::NotFound) => {}
            other => panic!("應為 NotFound，卻得到 {other:?}"),
        }
    }

    #[test]
    fn missing_root_is_not_found() {
        let f = fixture("resolve-missing-root");
        match resolve(&f.root.join("gone"), &rel("")) {
            Err(FilesError::NotFound) => {}
            other => panic!("應為 NotFound，卻得到 {other:?}"),
        }
    }

    /// 第二道防線：即使繞過 `parse`（直接建構含 `..` 的 RelPath），canonicalize 後的界限檢查
    /// 仍擋下根目錄外的目標。
    #[test]
    fn boundary_check_rejects_escape_even_without_segment_rules() {
        let f = fixture("resolve-dotdot");
        let sneaky = RelPath {
            segments: vec!["..".into(), "root-outside".into(), "secret.txt".into()],
        };
        match resolve(&f.root, &sneaky) {
            Err(FilesError::PathOutsideRoot) => {}
            other => panic!("應為 PathOutsideRoot，卻得到 {other:?}"),
        }
    }

    /// 根內多層缺失的路徑仍是 NotFound（封住根外存在性探測後，根內行為不變）。
    #[test]
    fn missing_multi_level_target_inside_root_is_not_found() {
        let f = fixture("resolve-missing-deep");
        match resolve(&f.root, &rel("docs/a/b/c.md")) {
            Err(FilesError::NotFound) => {}
            other => panic!("應為 NotFound，卻得到 {other:?}"),
        }
    }

    /// file-review task 2.2 fix round 1：實體位置在根外的**不存在**目標也是 PathOutsideRoot，
    /// 不得以 NotFound／PathOutsideRoot 的差異探測根外檔案是否存在。
    #[test]
    fn missing_target_whose_real_location_is_outside_root_is_path_outside_root() {
        let f = fixture("resolve-dotdot-missing");
        for segments in [
            vec!["..", "root-outside", "nope.txt"],
            vec!["..", "root-outside", "a", "b", "c.txt"],
            vec!["..", "no-such-dir", "x.txt"],
        ] {
            let sneaky = RelPath {
                segments: segments.iter().map(|s| s.to_string()).collect(),
            };
            match resolve(&f.root, &sneaky) {
                Err(FilesError::PathOutsideRoot) => {}
                other => panic!("{segments:?} 應為 PathOutsideRoot，卻得到 {other:?}"),
            }
        }
    }

    #[cfg(windows)]
    mod windows {
        use super::{Fixture, canonical, fixture, rel};
        use crate::FilesError;
        use crate::relpath::{RelPath, resolve};
        use std::os::windows::process::CommandExt;
        use std::path::{Path, PathBuf};
        use std::process::Command;

        /// 以 `cmd /c mklink /J` 建立 junction（不需要建立符號連結的權限）。
        fn junction(link: &Path, target: &Path) {
            let out = Command::new("cmd")
                .args(["/c", "mklink", "/J"])
                .arg(link)
                .arg(target)
                .output()
                .expect("執行 cmd mklink /J");
            assert!(
                out.status.success(),
                "mklink /J 失敗：{}",
                String::from_utf8_lossy(&out.stderr)
            );
        }

        /// 以 cmd 的 `%~sI` 取得路徑最後一段的 8.3 短名稱；磁碟停用 8.3（短名稱與長名稱相同）時回 `None`。
        fn short_name(path: &Path) -> Option<String> {
            let out = Command::new("cmd")
                .raw_arg(format!(
                    "/c for %I in (\"{}\") do @echo %~sI",
                    path.display()
                ))
                .output()
                .expect("執行 cmd 取得 8.3 短名稱");
            let text = String::from_utf8_lossy(&out.stdout);
            let short = PathBuf::from(text.trim());
            let short = short.file_name()?.to_string_lossy().into_owned();
            let long = path.file_name()?.to_string_lossy().into_owned();
            (short != long && short.contains('~')).then_some(short)
        }

        fn assert_outside(f: &Fixture, raw: &str) {
            match resolve(&f.root, &rel(raw)) {
                Err(FilesError::PathOutsideRoot) => {}
                other => panic!("{raw:?} 應為 PathOutsideRoot，卻得到 {other:?}"),
            }
        }

        /// Scenario: 符號連結指向根目錄外
        ///
        /// 先試 `symlink_file`；沒有建立符號連結的權限（ERROR_PRIVILEGE_NOT_HELD＝1314，未開開發人員
        /// 模式的一般帳號即是）時，改以 junction 驗同一條規則。
        #[test]
        fn scenario_symlink_pointing_outside_root_is_path_outside_root() {
            let f = fixture("symlink-outside");
            let link = f.root.join("link.txt");
            match std::os::windows::fs::symlink_file(f.outside.join("secret.txt"), &link) {
                Ok(()) => assert_outside(&f, "link.txt"),
                Err(e) if e.raw_os_error() == Some(1314) => {
                    eprintln!("沒有建立符號連結的權限（os error 1314），改以 junction 驗證");
                    junction(&f.root.join("junction"), &f.outside);
                    assert_outside(&f, "junction/secret.txt");
                }
                Err(e) => panic!("建立符號連結失敗：{e}"),
            }
        }

        /// Scenario: 符號連結指向根目錄外（junction 版，一律執行）。
        ///
        /// junction 目標 `root-outside` 的字串以根目錄路徑為前綴，確認判斷是逐 component。
        #[test]
        fn junction_pointing_outside_root_is_path_outside_root() {
            let f = fixture("junction-outside");
            junction(&f.root.join("junction"), &f.outside);
            assert_outside(&f, "junction/secret.txt");
            assert_outside(&f, "junction");
        }

        /// file-review task 2.2 fix round 1（Codex finding：根外存在性探測）：同一個指向根外的 junction
        /// 底下，既存檔、缺檔、多層缺失路徑、穿過檔案的路徑一律 PathOutsideRoot，無從分辨存在與否。
        #[test]
        fn junction_outside_root_does_not_reveal_whether_target_exists() {
            let f = fixture("junction-oracle");
            junction(&f.root.join("junction"), &f.outside);
            assert_outside(&f, "junction/secret.txt");
            assert_outside(&f, "junction/nope.txt");
            assert_outside(&f, "junction/a/b/c.txt");
            assert_outside(&f, "junction/secret.txt/x");
            // 根內缺檔照舊 NotFound。
            match resolve(&f.root, &rel("docs/nope.md")) {
                Err(FilesError::NotFound) => {}
                other => panic!("根內缺檔應為 NotFound，卻得到 {other:?}"),
            }
        }

        /// file-review task 2.2 fix round 2（Codex finding：懸空的根外 junction 回退成 NotFound）：
        /// junction 指向的根外資料夾被刪掉、再重建，同一請求的結果都是 PathOutsideRoot，
        /// 不隨根外資料夾是否存在而改變。
        #[test]
        fn dangling_outside_junction_result_does_not_depend_on_outside_existence() {
            let f = fixture("junction-dangling-outside");
            junction(&f.root.join("junction"), &f.outside);
            let requests = ["junction/x.txt", "junction/a/b.txt", "junction"];

            std::fs::remove_dir_all(&f.outside).expect("刪除根外資料夾");
            for raw in requests {
                assert_outside(&f, raw);
            }

            std::fs::create_dir_all(&f.outside).expect("重建根外資料夾");
            for raw in requests {
                assert_outside(&f, raw);
            }
        }

        /// 控制端裁決 R11：存在但無法解析的連結一律拒絕，即使它原本指向根內（一致拒絕優先於精準分類）。
        #[test]
        fn dangling_junction_pointing_inside_root_is_path_outside_root() {
            let f = fixture("junction-dangling-inside");
            let target = f.root.join("docs2");
            std::fs::create_dir(&target).expect("建立根內資料夾");
            junction(&f.root.join("alias"), &target);
            std::fs::remove_dir(&target).expect("刪除 junction 目標");
            for raw in ["alias/x.txt", "alias/a/b.txt", "alias"] {
                assert_outside(&f, raw);
            }
        }

        /// canonicalize 的非 NotFound 錯誤（Windows 檔名不得含 `?` → ERROR_INVALID_NAME）不被吞掉：
        /// 在根內 → Io；經根外 junction → PathOutsideRoot（根外優先，避免以 500／403 探測）。
        #[test]
        fn non_not_found_errors_are_io_inside_root_and_outside_when_under_outside_link() {
            let f = fixture("invalid-name");
            match resolve(&f.root, &rel("docs/a%3Fb.md")) {
                Err(FilesError::Io(_)) => {}
                other => panic!("根內非法檔名應為 Io，卻得到 {other:?}"),
            }
            junction(&f.root.join("junction"), &f.outside);
            assert_outside(&f, "junction/a%3Fb.md");
        }

        /// 根目錄本身是 junction（指向另一個資料夾）：根內檔案照常解析成實體路徑；`..` 在解析時就被擋；
        /// 經根內 junction 指向「junction 目標之外」的東西（既存與不存在）都是 PathOutsideRoot。
        #[test]
        fn root_that_is_itself_a_junction_is_bounded_by_its_real_target() {
            let f = fixture("root-junction");
            let root_link = f.root.parent().unwrap().join("root-link");
            junction(&root_link, &f.root);

            let got = resolve(&root_link, &rel("docs/readme.md")).expect("根內檔案應可解析");
            assert_eq!(got, canonical(&f.root.join("docs").join("readme.md")));
            assert_eq!(
                resolve(&root_link, &rel("")).expect("根目錄本身應可解析"),
                canonical(&f.root)
            );

            match RelPath::parse("../root-outside/secret.txt") {
                Err(FilesError::BadRequest) => {}
                other => panic!("`..` 應為 BadRequest，卻得到 {other:?}"),
            }

            junction(&f.root.join("escape"), &f.outside);
            for raw in ["escape/secret.txt", "escape/nope.txt", "escape"] {
                match resolve(&root_link, &rel(raw)) {
                    Err(FilesError::PathOutsideRoot) => {}
                    other => panic!("{raw:?} 應為 PathOutsideRoot，卻得到 {other:?}"),
                }
            }
        }

        /// 指向根目錄內的 junction 照常解析，得到實體路徑。
        #[test]
        fn junction_pointing_inside_root_resolves_to_real_path() {
            let f = fixture("junction-inside");
            junction(&f.root.join("alias"), &f.root.join("docs"));
            let got = resolve(&f.root, &rel("alias/readme.md")).expect("根內 junction 應可解析");
            assert_eq!(got, canonical(&f.root.join("docs").join("readme.md")));
        }

        /// 根目錄以大小寫不同的字面傳入（全大寫），仍判斷為在根目錄內。
        #[test]
        fn root_given_in_different_case_still_contains_its_files() {
            let f = fixture("case-root");
            let upper_root = PathBuf::from(f.root.to_string_lossy().to_uppercase());
            assert_ne!(upper_root, f.root, "測試前提：大小寫字面確實不同");

            let got = resolve(&upper_root, &rel("DOCS/README.MD")).expect("大小寫不同仍應可解析");
            assert_eq!(got, canonical(&f.root.join("docs").join("readme.md")));

            junction(&f.root.join("junction"), &f.outside);
            match resolve(&upper_root, &rel("junction/secret.txt")) {
                Err(FilesError::PathOutsideRoot) => {}
                other => panic!("應為 PathOutsideRoot，卻得到 {other:?}"),
            }
        }

        /// 8.3 短檔名：短名稱片段（含 `~`）不另外擋，而是經 canonicalize 解析成長檔名後照樣受界限檢查。
        /// 暫存目錄所在磁碟停用 8.3 時無從驗證，印出原因後略過。
        #[test]
        fn short_8dot3_names_are_resolved_then_bounded() {
            let f = fixture("short-names");
            let inside_dir = f.root.join("long-directory-name-inside");
            std::fs::create_dir(&inside_dir).expect("建立根內長名稱資料夾");
            std::fs::write(inside_dir.join("note.txt"), "n").expect("寫入根內檔案");
            let outside_link = f.root.join("junction-to-outside-folder");
            junction(&outside_link, &f.outside);

            let (Some(inside_short), Some(outside_short)) =
                (short_name(&inside_dir), short_name(&outside_link))
            else {
                eprintln!("暫存目錄所在磁碟未產生 8.3 短名稱（已停用 8.3），略過此測試");
                return;
            };

            let got = resolve(&f.root, &rel(&format!("{inside_short}/note.txt")))
                .expect("根內短名稱應可解析");
            assert_eq!(got, canonical(&inside_dir.join("note.txt")));
            assert!(got.ends_with("long-directory-name-inside/note.txt"));

            match resolve(&f.root, &rel(&format!("{outside_short}/secret.txt"))) {
                Err(FilesError::PathOutsideRoot) => {}
                other => panic!("短名稱指向根目錄外應為 PathOutsideRoot，卻得到 {other:?}"),
            }

            // 根目錄本身以 8.3 短名稱字面傳入，也能判斷根內檔案在根內。
            let long_root = f.root.parent().unwrap().join("long-root-directory-name");
            std::fs::create_dir(&long_root).expect("建立長名稱根目錄");
            std::fs::write(long_root.join("a.txt"), "a").expect("寫入檔案");
            let root_short = short_name(&long_root).expect("同一磁碟應同樣產生短名稱");
            let short_root = long_root.parent().unwrap().join(root_short);
            let got = resolve(&short_root, &rel("a.txt")).expect("短名稱根目錄應可解析");
            assert_eq!(got, canonical(&long_root.join("a.txt")));
        }
    }
}
