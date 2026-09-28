//! Material Icon Theme 對照表解析與 icon 查詢（file-review spec「檔案 icon」、design D9）。
//!
//! 只解析 `fileNames`、`fileExtensions`、`folderNames`、`folderNamesExpanded` 與預設
//! `file`／`folder`／`folderExpanded` 三個鍵；`light`、`rootFolderNames`、`languageIds` 等其餘
//! 欄位依 design.md Non-Goals 不使用，serde 對未知欄位預設忽略，不必宣告。

use std::collections::HashMap;
use std::fmt;

use serde::Deserialize;

/// [`IconTheme::from_json`] 的錯誤。
#[derive(Debug)]
pub enum IconThemeError {
    /// JSON 格式或型別不符合預期形狀。
    Json(serde_json::Error),
    /// 缺少必要的預設鍵（`"file"`、`"folder"`、`"folderExpanded"` 其中之一），或其值在
    /// `iconDefinitions` 找不到對應條目——兩種情況都讓查詢無法保證有預設值可回，一律視為解析
    /// 失敗，不是「查無則忽略」。攜帶的字串是 spec 用詞的原始鍵名，方便除錯。
    MissingDefault(&'static str),
}

impl fmt::Display for IconThemeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IconThemeError::Json(e) => write!(f, "material-icons.json 解析失敗：{e}"),
            IconThemeError::MissingDefault(key) => {
                write!(
                    f,
                    "material-icons.json 缺少必要的預設鍵或其對照表條目：{key}"
                )
            }
        }
    }
}

impl std::error::Error for IconThemeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            IconThemeError::Json(e) => Some(e),
            IconThemeError::MissingDefault(_) => None,
        }
    }
}

impl From<serde_json::Error> for IconThemeError {
    fn from(e: serde_json::Error) -> Self {
        IconThemeError::Json(e)
    }
}

#[derive(Deserialize)]
struct RawIconDefinition {
    #[serde(rename = "iconPath")]
    icon_path: String,
}

/// 只宣告用得到的欄位；其餘欄位（`light`、`rootFolderNames`、`languageIds` 等，design.md
/// Non-Goals）交給 serde 預設忽略未知欄位。
#[derive(Deserialize)]
struct RawTheme {
    #[serde(rename = "iconDefinitions", default)]
    icon_definitions: HashMap<String, RawIconDefinition>,
    #[serde(rename = "fileNames", default)]
    file_names: HashMap<String, String>,
    #[serde(rename = "fileExtensions", default)]
    file_extensions: HashMap<String, String>,
    #[serde(rename = "folderNames", default)]
    folder_names: HashMap<String, String>,
    #[serde(rename = "folderNamesExpanded", default)]
    folder_names_expanded: HashMap<String, String>,
    file: Option<String>,
    folder: Option<String>,
    #[serde(rename = "folderExpanded")]
    folder_expanded: Option<String>,
}

/// Material Icon Theme 對照表（file-review spec「檔案 icon」、design D9）。
///
/// 對外只提供查詢，回傳值一律是 SVG **檔名**（例如 `readme.svg`，不含目錄）；`cockpit` 再拼成
/// `/vendor/material-icons/icons/<檔名>` 這樣的服務網址（見 `cockpit/assets/vendor/README.md`
/// 對 `iconPath` 的說明）。
#[derive(Debug, Clone)]
pub struct IconTheme {
    file_names: HashMap<String, String>,
    file_extensions: HashMap<String, String>,
    folder_names: HashMap<String, String>,
    folder_names_expanded: HashMap<String, String>,
    default_file: String,
    default_folder: String,
    default_folder_expanded: String,
}

impl IconTheme {
    /// 解析 Material Icon Theme 的 `material-icons.json`（design D9）。
    ///
    /// `fileNames`、`fileExtensions`、`folderNames`、`folderNamesExpanded` 的鍵一律轉小寫建表；
    /// 值先在 `iconDefinitions` 查出 `iconPath`，取其檔名部分（最後一段，`/`、`\` 皆視為分隔
    /// 字元），查不到對應條目就略過那一條，不視為整體解析失敗。預設檔案 icon（`file` 鍵）、預設
    /// 資料夾收合／展開（`folder`／`folderExpanded`）三者缺一（缺鍵本身，或其值在
    /// `iconDefinitions` 找不到）即回傳 [`IconThemeError::MissingDefault`]。
    pub fn from_json(bytes: &[u8]) -> Result<Self, IconThemeError> {
        let raw: RawTheme = serde_json::from_slice(bytes)?;

        let resolve = |key: &str| -> Option<String> {
            raw.icon_definitions
                .get(key)
                .map(|def| icon_basename(&def.icon_path))
        };

        let default_file = raw
            .file
            .as_deref()
            .and_then(resolve)
            .ok_or(IconThemeError::MissingDefault("file"))?;
        let default_folder = raw
            .folder
            .as_deref()
            .and_then(resolve)
            .ok_or(IconThemeError::MissingDefault("folder"))?;
        let default_folder_expanded = raw
            .folder_expanded
            .as_deref()
            .and_then(resolve)
            .ok_or(IconThemeError::MissingDefault("folderExpanded"))?;

        Ok(IconTheme {
            file_names: build_table(&raw.file_names, &resolve),
            file_extensions: build_table(&raw.file_extensions, &resolve),
            folder_names: build_table(&raw.folder_names, &resolve),
            folder_names_expanded: build_table(&raw.folder_names_expanded, &resolve),
            default_file,
            default_folder,
            default_folder_expanded,
        })
    }

    /// 檔案的 icon 檔名（spec「檔案 icon」檔案規則）。依序比對：
    /// 1. 完整檔名（不分大小寫）。
    /// 2. 副檔名，由最長的多段往短比對（例如 `a.d.ts` 先比 `d.ts` 再比 `ts`）；點開頭的檔名
    ///    （例如 `.gitignore`）完整檔名查無時，退回把開頭那個點之後的部分當副檔名比對
    ///    （即 `gitignore`）——這是同一條「切成 `.` 分段、從第 2 段起往後拼接」規則的自然結果，
    ///    不需要特判。
    /// 3. 預設檔案 icon。
    pub fn file_icon(&self, name: &str) -> &str {
        let lower = name.to_lowercase();
        if let Some(icon) = self.file_names.get(&lower) {
            return icon;
        }
        let parts: Vec<&str> = lower.split('.').collect();
        for start in 1..parts.len() {
            let candidate = parts[start..].join(".");
            if let Some(icon) = self.file_extensions.get(&candidate) {
                return icon;
            }
        }
        &self.default_file
    }

    /// 資料夾的（收合, 展開）icon 檔名（spec「檔案 icon」資料夾規則）：名稱（不分大小寫）分別查
    /// `folderNames`／`folderNamesExpanded`，沒有對應時各自用預設資料夾 icon。
    pub fn folder_icon(&self, name: &str) -> (&str, &str) {
        let lower = name.to_lowercase();
        let collapsed = self
            .folder_names
            .get(&lower)
            .map(String::as_str)
            .unwrap_or(&self.default_folder);
        let expanded = self
            .folder_names_expanded
            .get(&lower)
            .map(String::as_str)
            .unwrap_or(&self.default_folder_expanded);
        (collapsed, expanded)
    }
}

/// 把一個原始對照表（鍵→`iconDefinitions` 鍵）轉成鍵→SVG 檔名，鍵轉小寫；值查無對應定義的條目
/// 直接跳過（[`from_json`](IconTheme::from_json) 文件所述的略過規則）。
fn build_table(
    raw: &HashMap<String, String>,
    resolve: &impl Fn(&str) -> Option<String>,
) -> HashMap<String, String> {
    raw.iter()
        .filter_map(|(k, v)| resolve(v).map(|icon| (k.to_lowercase(), icon)))
        .collect()
}

/// `iconPath`（例如 `./../icons/git.svg`）取最後一段當 SVG 檔名；`/`、`\` 都視為分隔字元，避免
/// 對照表萬一含反斜線路徑時取不到正確的檔名。
fn icon_basename(icon_path: &str) -> String {
    icon_path
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(icon_path)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::super::{IconTheme, IconThemeError};

    /// 小型 fixture：每條比對規則各安排一個能分辨結果的條目。
    ///
    /// - `readme.md` 同時可由完整檔名（`fileNames`）與副檔名 `md`（`fileExtensions`）命中，
    ///   兩者指到不同 icon，用來驗證「完整檔名優先」。
    /// - `d.ts` 與 `ts` 都在 `fileExtensions`，指到不同 icon，驗證「多段副檔名最長優先」。
    /// - `gitignore` 只在 `fileExtensions`（沒有對應的 `fileNames` 完整檔名條目），驗證點開頭
    ///   檔名「完整檔名先比、查無再把去掉開頭那個點的部分當副檔名比對」。
    /// - `fileExtensions.ghost` 的值 `missing-def` 不是 `iconDefinitions` 的鍵，驗證「值找不到就
    ///   略過該條，退回預設」。
    /// - `folderNames`／`folderNamesExpanded` 對 `src` 各指到不同 icon，驗證資料夾收合／展開分開
    ///   查表；`docs` 沒有對應條目，驗證資料夾預設。
    const FIXTURE: &str = r#"{
        "iconDefinitions": {
            "def-file": { "iconPath": "./../icons/file.svg" },
            "def-folder": { "iconPath": "./../icons/folder.svg" },
            "def-folder-open": { "iconPath": "./../icons/folder-open.svg" },
            "readme": { "iconPath": "./../icons/readme.svg" },
            "markdown": { "iconPath": "./../icons/markdown.svg" },
            "typescript": { "iconPath": "./../icons/typescript.svg" },
            "typescript-def": { "iconPath": "./../icons/typescript-def.svg" },
            "gitignore-icon": { "iconPath": "./../icons/gitignore-icon.svg" },
            "folder-src": { "iconPath": "./../icons/folder-src.svg" },
            "folder-src-open": { "iconPath": "./../icons/folder-src-open.svg" }
        },
        "fileNames": {
            "readme.md": "readme"
        },
        "fileExtensions": {
            "md": "markdown",
            "ts": "typescript",
            "d.ts": "typescript-def",
            "gitignore": "gitignore-icon",
            "ghost": "missing-def"
        },
        "folderNames": {
            "src": "folder-src"
        },
        "folderNamesExpanded": {
            "src": "folder-src-open"
        },
        "file": "def-file",
        "folder": "def-folder",
        "folderExpanded": "def-folder-open"
    }"#;

    fn theme() -> IconTheme {
        IconTheme::from_json(FIXTURE.as_bytes()).expect("fixture 應解析成功")
    }

    /// scenario「常見檔案」的規則之一：完整檔名優先於副檔名。
    #[test]
    fn full_file_name_wins_over_extension() {
        let t = theme();
        assert_eq!(t.file_icon("readme.md"), "readme.svg");
    }

    /// 多段副檔名由最長往短比對：`a.d.ts` 先比 `d.ts`，命中就不再退到 `ts`。
    #[test]
    fn longest_multi_part_extension_wins() {
        let t = theme();
        assert_eq!(t.file_icon("a.d.ts"), "typescript-def.svg");
    }

    /// 只有單段副檔名時，退到最短的那一段。
    #[test]
    fn single_extension_falls_back_correctly() {
        let t = theme();
        assert_eq!(t.file_icon("index.ts"), "typescript.svg");
    }

    /// 比對不分大小寫：完整檔名與副檔名皆然。
    #[test]
    fn matching_is_case_insensitive() {
        let t = theme();
        assert_eq!(t.file_icon("README.MD"), "readme.svg");
        assert_eq!(t.file_icon("A.D.TS"), "typescript-def.svg");
    }

    /// 點開頭的檔名：完整檔名（含開頭的點）查無時，退回把去掉那個點之後的部分當副檔名比對。
    #[test]
    fn dotfile_without_full_name_entry_falls_back_to_extension() {
        let t = theme();
        assert_eq!(t.file_icon(".gitignore"), "gitignore-icon.svg");
    }

    /// 完全沒有比對到的檔案用預設檔案 icon。
    #[test]
    fn unknown_file_uses_default_icon() {
        let t = theme();
        assert_eq!(t.file_icon("unknown.zzz"), "file.svg");
    }

    /// 對照表的值若在 `iconDefinitions` 找不到，該條在解析時就被略過，查詢時等同沒有這條規則。
    #[test]
    fn entry_whose_value_is_missing_from_definitions_is_skipped() {
        let t = theme();
        assert_eq!(t.file_icon("x.ghost"), "file.svg");
    }

    /// 資料夾名稱有對應時，收合與展開分別查表，且兩者不同。
    #[test]
    fn folder_with_entry_uses_collapsed_and_expanded_icons() {
        let t = theme();
        assert_eq!(
            t.folder_icon("src"),
            ("folder-src.svg", "folder-src-open.svg")
        );
    }

    /// 資料夾名稱比對不分大小寫。
    #[test]
    fn folder_matching_is_case_insensitive() {
        let t = theme();
        assert_eq!(
            t.folder_icon("SRC"),
            ("folder-src.svg", "folder-src-open.svg")
        );
    }

    /// 沒有對應的資料夾名稱用預設資料夾 icon（收合／展開的預設本身也不同）。
    #[test]
    fn folder_without_entry_uses_default_icons() {
        let t = theme();
        assert_eq!(t.folder_icon("docs"), ("folder.svg", "folder-open.svg"));
    }

    /// 缺少 `file` 鍵時解析失敗。
    #[test]
    fn missing_default_file_key_is_an_error() {
        let json = r#"{
            "iconDefinitions": {
                "def-folder": { "iconPath": "./../icons/folder.svg" },
                "def-folder-open": { "iconPath": "./../icons/folder-open.svg" }
            },
            "folder": "def-folder",
            "folderExpanded": "def-folder-open"
        }"#;
        match IconTheme::from_json(json.as_bytes()) {
            Err(IconThemeError::MissingDefault("file")) => {}
            other => panic!("應為 MissingDefault(\"file\")，卻得到 {other:?}"),
        }
    }

    /// 缺少 `folder` 鍵時解析失敗。
    #[test]
    fn missing_default_folder_key_is_an_error() {
        let json = r#"{
            "iconDefinitions": {
                "def-file": { "iconPath": "./../icons/file.svg" },
                "def-folder-open": { "iconPath": "./../icons/folder-open.svg" }
            },
            "file": "def-file",
            "folderExpanded": "def-folder-open"
        }"#;
        match IconTheme::from_json(json.as_bytes()) {
            Err(IconThemeError::MissingDefault("folder")) => {}
            other => panic!("應為 MissingDefault(\"folder\")，卻得到 {other:?}"),
        }
    }

    /// 缺少 `folderExpanded` 鍵時解析失敗。
    #[test]
    fn missing_default_folder_expanded_key_is_an_error() {
        let json = r#"{
            "iconDefinitions": {
                "def-file": { "iconPath": "./../icons/file.svg" },
                "def-folder": { "iconPath": "./../icons/folder.svg" }
            },
            "file": "def-file",
            "folder": "def-folder"
        }"#;
        match IconTheme::from_json(json.as_bytes()) {
            Err(IconThemeError::MissingDefault("folderExpanded")) => {}
            other => panic!("應為 MissingDefault(\"folderExpanded\")，卻得到 {other:?}"),
        }
    }

    /// 預設鍵存在，但其值在 `iconDefinitions` 找不到對應條目時，同樣視為缺預設、解析失敗
    /// （不是「查無則忽略」——預設值本身必須可靠）。
    #[test]
    fn default_key_pointing_to_missing_definition_is_an_error() {
        let json = r#"{
            "iconDefinitions": {
                "def-folder": { "iconPath": "./../icons/folder.svg" },
                "def-folder-open": { "iconPath": "./../icons/folder-open.svg" }
            },
            "file": "no-such-definition",
            "folder": "def-folder",
            "folderExpanded": "def-folder-open"
        }"#;
        match IconTheme::from_json(json.as_bytes()) {
            Err(IconThemeError::MissingDefault("file")) => {}
            other => panic!("應為 MissingDefault(\"file\")，卻得到 {other:?}"),
        }
    }

    /// 不是合法 JSON 時回傳 `Json` 錯誤，不是預設鍵缺失。
    #[test]
    fn invalid_json_is_a_json_error() {
        match IconTheme::from_json(b"not json") {
            Err(IconThemeError::Json(_)) => {}
            other => panic!("應為 Json(_)，卻得到 {other:?}"),
        }
    }
}

#[cfg(test)]
mod real_file_tests {
    use super::super::IconTheme;
    use std::path::Path;

    const MATERIAL_ICONS_JSON: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../cockpit/assets/vendor/material-icons/material-icons.json"
    );
    const ICONS_DIR: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../cockpit/assets/vendor/material-icons/icons"
    );

    fn load_theme() -> (IconTheme, serde_json::Value) {
        let bytes = std::fs::read(MATERIAL_ICONS_JSON).unwrap_or_else(|e| {
            panic!("實檔測試需要 {MATERIAL_ICONS_JSON} 存在（見 D9 vendor 佈局）：{e}")
        });
        let raw: serde_json::Value =
            serde_json::from_slice(&bytes).expect("material-icons.json 應為合法 JSON");
        let theme = IconTheme::from_json(&bytes).expect("實檔應解析成功");
        (theme, raw)
    }

    /// 從原始 JSON（不經 `IconTheme` 的內部邏輯）算出某個 `iconDefinitions` 鍵對應的 SVG
    /// 檔名，做為獨立於實作的期望值來源。
    fn expected_icon(raw: &serde_json::Value, definition_key: &str) -> String {
        let icon_path = raw["iconDefinitions"][definition_key]["iconPath"]
            .as_str()
            .unwrap_or_else(|| panic!("iconDefinitions.{definition_key}.iconPath 應為字串"));
        icon_path
            .rsplit(['/', '\\'])
            .next()
            .unwrap_or(icon_path)
            .to_string()
    }

    fn assert_svg_exists(name: &str) {
        let path = Path::new(ICONS_DIR).join(name);
        assert!(path.is_file(), "SVG 檔案應存在：{}", path.display());
    }

    /// Scenario「常見檔案」：`README.md` 用對照表 `readme.md` 對應的 icon（完整檔名優先於
    /// `.md`）。
    #[test]
    fn scenario_common_files_readme() {
        let (theme, raw) = load_theme();
        let definition_key = raw["fileNames"]["readme.md"]
            .as_str()
            .expect("fileNames.\"readme.md\" 應存在");
        let expected = expected_icon(&raw, definition_key);

        let actual = theme.file_icon("README.md");
        assert_eq!(actual, expected);
        assert_svg_exists(actual);
    }

    /// Scenario「常見檔案」：`report.pdf` 使用對照表 `pdf` 副檔名對應的 icon。
    #[test]
    fn scenario_common_files_pdf() {
        let (theme, raw) = load_theme();
        let definition_key = raw["fileExtensions"]["pdf"]
            .as_str()
            .expect("fileExtensions.pdf 應存在");
        let expected = expected_icon(&raw, definition_key);

        let actual = theme.file_icon("report.pdf");
        assert_eq!(actual, expected);
        assert_svg_exists(actual);
    }

    /// Scenario「常見檔案」：`index.html` 使用對照表 `html` 副檔名對應的 icon。
    #[test]
    fn scenario_common_files_html() {
        let (theme, raw) = load_theme();
        let definition_key = raw["fileExtensions"]["html"]
            .as_str()
            .expect("fileExtensions.html 應存在");
        let expected = expected_icon(&raw, definition_key);

        let actual = theme.file_icon("index.html");
        assert_eq!(actual, expected);
        assert_svg_exists(actual);
    }

    /// Scenario「常見檔案」：`unknown.zzz` 使用預設檔案 icon。
    #[test]
    fn scenario_common_files_unknown_uses_default() {
        let (theme, raw) = load_theme();
        let default_key = raw["file"].as_str().expect("file 應存在");
        let expected = expected_icon(&raw, default_key);

        let actual = theme.file_icon("unknown.zzz");
        assert_eq!(actual, expected);
        assert_svg_exists(actual);
    }

    /// Scenario「資料夾展開」：`src` 的收合與展開 icon 皆存在且兩者不同。
    #[test]
    fn scenario_folder_expansion_src() {
        let (theme, raw) = load_theme();
        let collapsed_key = raw["folderNames"]["src"]
            .as_str()
            .expect("folderNames.src 應存在");
        let expanded_key = raw["folderNamesExpanded"]["src"]
            .as_str()
            .expect("folderNamesExpanded.src 應存在");
        let expected_collapsed = expected_icon(&raw, collapsed_key);
        let expected_expanded = expected_icon(&raw, expanded_key);
        assert_ne!(
            expected_collapsed, expected_expanded,
            "fixture 前提：對照表本身的收合／展開 icon 應不同"
        );

        let (collapsed, expanded) = theme.folder_icon("src");
        assert_eq!(collapsed, expected_collapsed);
        assert_eq!(expanded, expected_expanded);
        assert_ne!(collapsed, expanded);
        assert_svg_exists(collapsed);
        assert_svg_exists(expanded);
    }
}
