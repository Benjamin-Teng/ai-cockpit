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

/// OpenSpec change 的四個進度階段（openspec-stage-sync task 3.1，design D1）。每個 Stage 最多對應一個階段，
/// 同一個階段最多對應一個 Stage。序列化為小寫字串 `"plan" | "implement" | "review" | "complete"`。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OpenSpecPhase {
    /// 規劃：proposal／design／specs／tasks 還在寫。
    Plan,
    /// 實作：`tasks.md` 已有勾選進度、尚未全部完成。
    Implement,
    /// 審查：`tasks.md` 全部勾完、尚未 archive。
    Review,
    /// 完成：change 已 archive。
    Complete,
}

impl OpenSpecPhase {
    /// 四個階段（宣告順序）。
    pub const ALL: [OpenSpecPhase; 4] = [Self::Plan, Self::Implement, Self::Review, Self::Complete];

    /// 狀態檔、投影與 HTTP 本體使用的字串形式。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Plan => "plan",
            Self::Implement => "implement",
            Self::Review => "review",
            Self::Complete => "complete",
        }
    }

    /// [`Self::as_str`] 的反向：只接受完全相符的小寫字串，其他回 `None`。
    pub fn parse(raw: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|phase| phase.as_str() == raw)
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
    /// 每個 Stage 對應的 OpenSpec 階段（openspec-stage-sync task 3.1，design D1）：與 `stages` 逐項對齊、長度
    /// 恆等於 `stages`，非 `None` 的值互不重複（[`repo_project_phases_valid`]）。
    pub phases: Vec<Option<OpenSpecPhase>>,
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
    /// pane 所在 worktree 根目錄的主機路徑（openspec-stage-sync task 4.4，design D5；spec `repo-projects`「納入 repo
    /// 判定的 pane 與更新時機」）：Windows 為反斜線、保留大小寫；WSL 為 `\\wsl.localhost\<distro>\...`。轉不出主機路徑時為
    /// `None`，該 pane 仍歸入 repo，但不被 OpenSpec 進度偵測查詢。
    pub root: Option<String>,
}

impl PaneRepo {
    /// 兩個歸類結果是否指向同一個位置：除 `default_name`（顯示用的預設名稱）以外的欄位全部相同
    /// （openspec-stage-sync task 4.3 Ruling、task 4.4）。OpenSpec 偵測結果屬於「某個 repo 的某個 worktree 根目錄」，
    /// 這些欄位任一個變了，舊的偵測結果就不得再套到這個 pane 上。以「去掉 `default_name` 後整體相等」實作，日後新增欄位
    /// 自動納入比較。
    pub fn same_location(&self, other: &PaneRepo) -> bool {
        let strip = |repo: &PaneRepo| PaneRepo {
            default_name: String::new(),
            ..repo.clone()
        };
        strip(self) == strip(other)
    }
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

/// Repo Project 的階段對應是否合法（openspec-stage-sync task 3.1，design D1）：長度等於 `stages` 的長度，且非
/// `None` 的值互不重複（同一個階段最多對應一個 Stage）。不合法對應 `invalid_stages`。
pub fn repo_project_phases_valid(stages: &[String], phases: &[Option<OpenSpecPhase>]) -> bool {
    if phases.len() != stages.len() {
        return false;
    }
    OpenSpecPhase::ALL
        .into_iter()
        .all(|phase| phases.iter().filter(|p| **p == Some(phase)).count() <= 1)
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
    /// 這個新 stage 修改後對應的 OpenSpec 階段；`None` 表示不對應（不是沿用舊值，openspec-stage-sync task 3.1）。
    pub phase: Option<OpenSpecPhase>,
}

/// [`apply_stage_edits`] 的結果：新的 stage 清單與舊 stage → 新 stage 的對應。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StageRemap {
    /// 去除空白後的新 stage 清單（順序即新的線性順序）；只由 [`apply_stage_edits`] 建立，保證非空。
    stages: Vec<String>,
    /// 被某個新 stage 以 `from` 引用的舊 stage → 新 stage 名稱。
    renamed: HashMap<String, String>,
    /// 與 `stages` 對齊的新階段對應，已通過 [`repo_project_phases_valid`]。
    phases: Vec<Option<OpenSpecPhase>>,
}

impl StageRemap {
    /// 新的 stage 清單（非空）。
    pub fn stages(&self) -> &[String] {
        &self.stages
    }

    /// 與 [`Self::stages`] 逐項對齊的新階段對應（來自各列的 `phase`；長度相同、非 `None` 值不重複）。
    pub fn phases(&self) -> &[Option<OpenSpecPhase>] {
        &self.phases
    }

    /// 這次編輯是否改變了階段對應（openspec-stage-sync task 3.1，design D10-2）：對每個階段，比較「修改前擁有它的
    /// stage 經本次 `from` 對應後的新名稱」與「修改後擁有它的 stage 名稱」，任一階段不同即為改變。`from` 為 `None`
    /// 的新列是新身分，不等於任何舊 stage；修改前的擁有者被刪除（沒有新名稱）而修改後也沒有擁有者時，兩邊都是
    /// `None`，視為不變。`old_stages` 與 `old_phases` 是修改前的定義，須逐項對齊。
    pub fn phases_changed(
        &self,
        old_stages: &[String],
        old_phases: &[Option<OpenSpecPhase>],
    ) -> bool {
        debug_assert_eq!(
            old_stages.len(),
            old_phases.len(),
            "old_stages 與 old_phases 必須逐項對齊（修改前的定義）"
        );
        OpenSpecPhase::ALL.into_iter().any(|phase| {
            let before: Option<&String> = old_phases
                .iter()
                .position(|p| *p == Some(phase))
                .and_then(|index| old_stages.get(index))
                .and_then(|old_name| self.renamed.get(old_name));
            let after: Option<&String> = self
                .phases
                .iter()
                .position(|p| *p == Some(phase))
                .map(|index| &self.stages[index]);
            before != after
        })
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
/// 每個舊名稱最多被引用一次；各列的 `phase` 非 `None` 的值不得重複（openspec-stage-sync task 3.1）。不合規則回
/// `None`（對應 `invalid_stages`）。
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
    let phases: Vec<Option<OpenSpecPhase>> = edits.iter().map(|edit| edit.phase).collect();
    if !repo_project_phases_valid(&stages, &phases) {
        return None;
    }
    Some(StageRemap {
        stages,
        renamed,
        phases,
    })
}
