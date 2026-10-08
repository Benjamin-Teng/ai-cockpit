//! Repo Project（repo-projects task 3.1，design D3）：由畫面加入、身分是一個 git repo 的 Project。
//! 定義 [`RepoProjectDef`]、pane 歸類結果 [`PaneRepos`]，以及把 Repo Project 展開成一般
//! [`ProjectDef`] 的純函數 [`expand_repo_projects`]。展開後投影、進度操作、agent 端點沿用手寫 project
//! 的程式路徑，不必分辨兩種 project。
//!
//! 展開只放 id、固定 pane 與 worktree 標註；workstream 名稱、task 標題與 workstream 排序由投影從
//! 當下的 pane 資料取（task 3.2），這裡的 `name`／`title` 先放 pane id 當佔位。展開不碰任何進度。

use std::collections::{BTreeMap, HashMap};
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::domain::config::{PinnedPane, ProjectDef, TaskDef, WorkstreamDef};
use crate::domain::ids::{ProjectId, TaskId, WorkstreamId};
use crate::message::Message;
use crate::types::ids::{PaneId, RuntimeId};

/// 一個 git repo 的身分：共同 `.git` 目錄的主機路徑（正規化規則見 design D1、D2）。只用於比對與在
/// 端點之間傳遞，不是使用者輸入。
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RepoKey(String);

impl RepoKey {
    /// 用任何可轉成 `String` 的值建立一個 `RepoKey`（呼叫端負責正規化）。
    pub fn new(key: impl Into<String>) -> Self {
        Self(key.into())
    }

    /// 借出底層字串內容。
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RepoKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// 一個 Repo Project 的定義（design D3、D5）：id 產生後不隨改名改變；`stages` 已通過驗證（D6）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RepoProjectDef {
    /// project id（與手寫 project 共用命名空間；撞名時手寫優先）。
    pub id: ProjectId,
    /// 顯示名稱。
    pub name: String,
    /// 這個 Repo Project 對應的 repo。
    pub repo: RepoKey,
    /// Stage 的線性順序。
    pub stages: Vec<String>,
}

/// 一個 pane 的 repo 歸類結果（design D2 第 3 點）：只取決於 pane 的 cwd。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PaneRepo {
    /// pane 所屬的 repo。
    pub repo: RepoKey,
    /// 這個 repo 的預設名稱（design D1，保留 git 原始輸出的大小寫）。
    pub default_name: String,
    /// pane 位於 linked worktree 時為 worktree 資料夾名稱；主 worktree 為 `None`。
    pub worktree: Option<String>,
}

/// 所有已歸類的 pane：`(runtime id, pane id)` → 歸類結果。不在這裡的 pane 不屬於任何 repo（或尚未判定）。
/// 用 `BTreeMap` 讓展開結果的順序固定（實際顯示順序由投影決定）。
pub type PaneRepos = BTreeMap<(RuntimeId, PaneId), PaneRepo>;

/// [`expand_repo_projects`] 的結果。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RepoExpansion {
    /// 實際生效的 project 清單：手寫的依原順序在前，展開的 Repo Project 在後（依名稱不分大小寫、同名依 id）。
    pub projects: Vec<ProjectDef>,
    /// 撞名警告：手寫 project id → 訊息原文（[`Message::RepoProjectIdConflict`] 的 `text()`）。
    /// 只含撞名警告；載入狀態檔產生的其他警告不在這裡。
    pub warnings: HashMap<ProjectId, Vec<String>>,
}

/// Repo Project 名稱去除前後空白後的字元數上限（design D6）。
pub const REPO_PROJECT_NAME_MAX_CHARS: usize = 64;
/// Repo Project 的 stage 數量上限（design D6）。
pub const REPO_PROJECT_STAGES_MAX: usize = 12;
/// 單一 stage 名稱去除前後空白後的字元數上限（design D6）。
pub const REPO_PROJECT_STAGE_MAX_CHARS: usize = 32;

/// Repo Project 名稱與 stage 名稱不接受的字元（design D6）：控制字元，加上會讓顯示錯亂的零寬與雙向控制格式字元
/// （U+200B、U+200E–U+200F、U+061C、U+202A–U+202E、U+2060–U+2064、U+2066–U+2069、U+FEFF；repo-projects
/// task 4.6）。U+200C（ZWNJ）與 U+200D（ZWJ）放行：波斯語等文字與 emoji 序列（如 👩‍💻）需要它們。
pub fn is_disallowed_label_char(c: char) -> bool {
    c.is_control()
        || matches!(
            c,
            '\u{200B}'
                | '\u{200E}'..='\u{200F}'
                | '\u{061C}'
                | '\u{202A}'..='\u{202E}'
                | '\u{2060}'..='\u{2064}'
                | '\u{2066}'..='\u{2069}'
                | '\u{FEFF}'
        )
}

/// 去除前後空白後檢查一段文字：字元數在 `1..=max` 且不含 [`is_disallowed_label_char`] 的字元時回傳去除空白後的值。
fn normalize_label(raw: &str, max_chars: usize) -> Option<String> {
    let trimmed = raw.trim();
    let count = trimmed.chars().count();
    if count == 0 || count > max_chars || trimmed.chars().any(is_disallowed_label_char) {
        return None;
    }
    Some(trimmed.to_string())
}

/// Repo Project 名稱的驗證與正規化（design D6；spec `repo-projects`「Repo Project 的輸入驗證」）：去除前後
/// 空白後 1～64 個字元、不含控制字元與不可見的格式字元（[`is_disallowed_label_char`]）時回傳去除空白後的名稱，
/// 否則 `None`。加入／修改端點與狀態檔載入共用（repo-projects task 4.1）。
pub fn normalize_repo_project_name(raw: &str) -> Option<String> {
    normalize_label(raw, REPO_PROJECT_NAME_MAX_CHARS)
}

/// Repo Project stages 的驗證與正規化（design D6）：1～12 個；每個去除前後空白後 1～32 個字元、不含
/// [`is_disallowed_label_char`] 的字元，
/// 且去除空白後互不相同。合法時回傳去除空白後的清單（順序不變），否則 `None`。`PATCH` 的 `from` 規則不在這裡。
pub fn normalize_repo_project_stages(raw: &[String]) -> Option<Vec<String>> {
    if raw.is_empty() || raw.len() > REPO_PROJECT_STAGES_MAX {
        return None;
    }
    let mut stages: Vec<String> = Vec::with_capacity(raw.len());
    for stage in raw {
        let stage = normalize_label(stage, REPO_PROJECT_STAGE_MAX_CHARS)?;
        if stages.contains(&stage) {
            return None;
        }
        stages.push(stage);
    }
    Some(stages)
}

/// Repo Project id 的長度上限，與設定檔 id 規則 `^[A-Za-z0-9_-]{1,64}$` 一致（repo-projects task 4.6）。
const REPO_PROJECT_ID_MAX_LEN: usize = 64;

/// Repo Project id 是否合法（design D5、D6）：1～64 個字元且只由 `[A-Za-z0-9_-]` 組成。
pub fn is_valid_repo_project_id(id: &str) -> bool {
    (1..=REPO_PROJECT_ID_MAX_LEN).contains(&id.len())
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// 固定 pane 工作線與 task 的 id：`<runtime id>~<pane id>`（design D3）。
pub fn pane_item_id(runtime: &RuntimeId, pane: &PaneId) -> String {
    format!("{}~{}", runtime.as_str(), pane.as_str())
}

/// 把 [`pane_item_id`] 拆回 `(runtime, pane)`：runtime id 可能含 `~`、pane id 不含，所以從**最後一個**
/// `~` 切開。兩邊有一邊是空的（或沒有 `~`）回 `None`。
pub fn split_pane_item_id(id: &str) -> Option<(RuntimeId, PaneId)> {
    let (runtime, pane) = id.rsplit_once('~')?;
    if runtime.is_empty() || pane.is_empty() {
        return None;
    }
    Some((RuntimeId::new(runtime), PaneId::new(pane)))
}

/// 把 Repo Project 展開成一般 `ProjectDef`，與手寫 project 合成實際生效的清單（design D3；spec
/// `repo-projects`「由 pane 推導 workstream 與 task」「Repo Project 與手寫 project 並列及 id 撞名」）。
///
/// - `config_projects` 必須只含手寫 project（`repo` 為 `None`）；傳入時已帶 `repo` 的項目會被略過，
///   避免把上一輪展開的結果當成手寫 project 而自己跟自己撞名。
/// - 每個 Repo Project（展開結果的 `repo` 為它的 repo key）：`pane_repos` 中 repo 相符的每個 pane 各一條 workstream 與一張 task，id 都是
///   [`pane_item_id`]；workstream 帶 [`PinnedPane`]（含 worktree 標註）、沒有 `binding`；task 在第一個
///   stage、沒有依賴。沒有歸入的 pane 時 workstreams／tasks 為空，project 照樣存在。
/// - 與某個手寫 project id 相同的 Repo Project 不展開，該手寫 project 得到一則撞名警告。
/// - 純函數：不讀寫進度、不看 Runtime 層。
pub fn expand_repo_projects(
    config_projects: &[ProjectDef],
    repo_projects: &[RepoProjectDef],
    pane_repos: &PaneRepos,
) -> RepoExpansion {
    let config_projects: Vec<&ProjectDef> = config_projects
        .iter()
        .filter(|p| p.repo.is_none())
        .collect();
    let mut warnings: HashMap<ProjectId, Vec<String>> = HashMap::new();
    let mut visible: Vec<&RepoProjectDef> = Vec::with_capacity(repo_projects.len());
    for def in repo_projects {
        if config_projects.iter().any(|p| p.id == def.id) {
            warnings.entry(def.id.clone()).or_default().push(
                Message::RepoProjectIdConflict {
                    id: def.id.as_str().to_string(),
                }
                .text(),
            );
        } else {
            visible.push(def);
        }
    }
    visible.sort_by(|a, b| {
        a.name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
            .then_with(|| a.id.cmp(&b.id))
    });

    let mut projects: Vec<ProjectDef> = config_projects.into_iter().cloned().collect();
    projects.extend(visible.into_iter().map(|def| expand_one(def, pane_repos)));
    RepoExpansion { projects, warnings }
}

fn expand_one(def: &RepoProjectDef, pane_repos: &PaneRepos) -> ProjectDef {
    let first_stage = def.stages.first().cloned().unwrap_or_default();
    let mut workstreams = Vec::new();
    let mut tasks = Vec::new();
    for ((runtime, pane), repo) in pane_repos {
        if repo.repo != def.repo {
            continue;
        }
        let id = pane_item_id(runtime, pane);
        let workstream_id = WorkstreamId::new(id.clone());
        // 名稱與標題由投影從 pane 資料覆寫（repo-projects task 3.2）；這裡先放 pane id。
        workstreams.push(WorkstreamDef {
            id: workstream_id.clone(),
            name: pane.as_str().to_string(),
            binding: None,
            pinned_pane: Some(PinnedPane {
                runtime: runtime.clone(),
                pane_id: pane.clone(),
                worktree: repo.worktree.clone(),
            }),
        });
        tasks.push(TaskDef {
            id: TaskId::new(id),
            title: pane.as_str().to_string(),
            workstream: workstream_id,
            stage: first_stage.clone(),
            depends_on: Vec::new(),
        });
    }
    ProjectDef {
        id: def.id.clone(),
        name: def.name.clone(),
        stages: def.stages.clone(),
        workstreams,
        tasks,
        repo: Some(def.repo.clone()),
    }
}

/// Repo Project id 由名稱產生時截到的字元數（design D6）。
pub const REPO_PROJECT_ID_MAX_CHARS: usize = 48;

/// 由名稱產生 Repo Project 的 id（design D6；spec `repo-projects`「Repo Project 的 id 產生」，repo-projects
/// task 4.2）：`[A-Za-z0-9_-]` 以外的每個字元換成 `-`、連續的 `-` 合併、去掉頭尾 `-`、截到 48 個字元，空的話用
/// `repo`；`is_taken` 認為已存在時依序加 `-2`、`-3`……。呼叫端負責把「所有現有 project id」（手寫、Repo，含被
/// 撞名隱藏的）交給 `is_taken`。
pub fn derive_repo_project_id(name: &str, is_taken: impl Fn(&str) -> bool) -> ProjectId {
    let mut base = String::with_capacity(name.len());
    for c in name.chars() {
        let c = if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
            c
        } else {
            '-'
        };
        if c == '-' && base.ends_with('-') {
            continue;
        }
        base.push(c);
    }
    // 替換後只剩 ASCII，位元組數即字元數。
    let mut base: String = base
        .trim_matches('-')
        .chars()
        .take(REPO_PROJECT_ID_MAX_CHARS)
        .collect();
    if base.is_empty() {
        base = "repo".to_string();
    }
    if !is_taken(&base) {
        return ProjectId::new(base);
    }
    let mut n: u64 = 2;
    loop {
        let candidate = format!("{base}-{n}");
        if !is_taken(&candidate) {
            return ProjectId::new(candidate);
        }
        n += 1;
    }
}

/// `PATCH /api/repo-projects/{pid}` 的一個新 stage（design D6）：`from` 為修改前某個 stage 的名稱時表示由它改名
/// （或只調整順序）而來；`None` 表示新增的 stage。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StageEdit {
    /// 新的 stage 名稱（尚未去除前後空白）。
    pub name: String,
    /// 這個新 stage 由哪個舊 stage 而來；`None` 表示新增。
    pub from: Option<String>,
}

/// [`apply_stage_edits`] 的結果：新的 stage 清單與舊 stage → 新 stage 的對應。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StageRemap {
    /// 去除空白後的新 stage 清單（順序即新的線性順序）；只由 [`apply_stage_edits`] 建立，保證非空。
    stages: Vec<String>,
    /// 被某個新 stage 以 `from` 引用的舊 stage → 新 stage 名稱。
    renamed: HashMap<String, String>,
}

impl StageRemap {
    /// 新的 stage 清單（非空）。
    pub fn stages(&self) -> &[String] {
        &self.stages
    }

    /// 新清單的第一個 stage。
    pub fn first_stage(&self) -> &str {
        // 欄位私有、只由 `apply_stage_edits` 以通過 `normalize_repo_project_stages` 的清單建立，不會是空的。
        self.stages.first().map_or("", String::as_str)
    }

    /// 原本在 `old` 的 task 的新 stage：`old` 被某個新 stage 以 `from` 引用時為該新 stage，否則（被刪除，或本來就
    /// 不在舊清單中）為新清單的第一個 stage（design D6）。
    pub fn stage_for(&self, old: &str) -> &str {
        self.renamed
            .get(old)
            .map_or_else(|| self.first_stage(), String::as_str)
    }
}

/// 驗證並套用 stage 編輯（design D6；spec `repo-projects`「修改 Repo Project 名稱與 stages」，repo-projects
/// task 4.2）：新名稱須符合 [`normalize_repo_project_stages`]；每個 `from` 必須（逐字）是 `current` 中的名稱，且
/// 每個舊名稱最多被引用一次。不合規則回 `None`（對應 `invalid_stages`）。
pub fn apply_stage_edits(current: &[String], edits: &[StageEdit]) -> Option<StageRemap> {
    let names: Vec<String> = edits.iter().map(|edit| edit.name.clone()).collect();
    let stages = normalize_repo_project_stages(&names)?;
    let mut renamed = HashMap::with_capacity(edits.len());
    for (edit, new_name) in edits.iter().zip(&stages) {
        let Some(from) = &edit.from else {
            continue;
        };
        if !current.contains(from) || renamed.insert(from.clone(), new_name.clone()).is_some() {
            return None;
        }
    }
    Some(StageRemap { stages, renamed })
}
