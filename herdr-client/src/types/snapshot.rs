//! `session.snapshot` 回應的形狀（spec `herdr-observer-types`「snapshot 解析」）。
//!
//! `workspaces`、`tabs`、`panes`、`agents` 四個陣列彼此是平行陣列、靠 id 字串互相參照，
//! 不是巢狀結構（`docs/research/2026-09-13/herdr-schema-findings.txt` §8）。只挑觀測所需
//! 的欄位子集；schema 有但這裡沒有的欄位在解析時被忽略（不用 `deny_unknown_fields`）。
//!
//! fix round 1 / finding 2：`WorkspaceInfo`、`TabInfo`、`PaneInfo`、`AgentInfo` 四個型別
//! **不**在容器層加 `#[serde(default)]`——那會讓 schema 標成必填的欄位（例如
//! `workspace_id`、`revision`、`agent_status`）在 JSON 裡缺席時也解析成功、只是靜默補上
//! 空字串／`0`／`AgentStatus::Unknown`，讓 Rust 欄位名拼錯或漏映射的錯誤被吃掉、只在執行期
//! 用到該欄位時才會顯形。改成只在真正選填（schema 非必填、型別是 `Option<T>`）的欄位上加
//! 逐欄 `#[serde(default)]`，讓它們缺席時如期變成 `None`，其餘必填欄位缺席則讓
//! `serde_json::from_value` 直接回 `Err`（見 `tests/types.rs` 的
//! `*_required_fields_are_all_enforced`）。這不違反 spec「未知欄位與缺少的選填欄位 → 成功」
//! 的 scenario——那個 scenario 測的是選填欄位，這裡的行為完全符合；spec 沒有要求必填欄位
//! 缺席時也要成功。
//!
//! fix round 2 / finding 2（2c）：`SessionSnapshot` 原本也在容器層開了 `#[serde(default)]`，
//! 與上面同一套理由不一致——`version`、`protocol`、`workspaces`、`tabs`、`panes`、`layouts`、
//! `agents` 是 schema 必填，缺席時不該靜默成功。改成只在三個 `focused_*`（schema 非必填、
//! nullable）欄位上加逐欄 `#[serde(default)]`。

use serde::{Deserialize, Serialize};

use super::agent_status::AgentStatus;

/// HERDR workspace（id 形如 `wJ`）。目前欄位全部是 schema 必填，缺一個就解析失敗。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct WorkspaceInfo {
    pub workspace_id: String,
    pub label: String,
    pub number: u32,
    pub active_tab_id: String,
    pub agent_status: AgentStatus,
    pub focused: bool,
    pub pane_count: u32,
    pub tab_count: u32,
}

/// HERDR workspace 內的分頁（id 形如 `wJ:t1`）。目前欄位全部是 schema 必填，缺一個就解析
/// 失敗。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct TabInfo {
    pub tab_id: String,
    pub workspace_id: String,
    pub number: u32,
    pub label: String,
    pub agent_status: AgentStatus,
    pub focused: bool,
    pub pane_count: u32,
}

/// HERDR tab 內的終端機格子（id 形如 `wJ:p1`）。`agent`、`title`、`terminal_title`、`cwd`、
/// `label` 是 schema 非必填（nullable）欄位，缺席時為 `None`；其餘欄位缺席會解析失敗。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct PaneInfo {
    pub pane_id: String,
    pub workspace_id: String,
    pub tab_id: String,
    #[serde(default)]
    pub agent: Option<String>,
    pub agent_status: AgentStatus,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub terminal_title: Option<String>,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub label: Option<String>,
    pub focused: bool,
    pub revision: u64,
}

/// HERDR 偵測到、跑在某個 pane 裡的 agent。`agent` 是 schema 非必填（nullable）欄位，缺席
/// 時為 `None`；其餘欄位缺席會解析失敗。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct AgentInfo {
    #[serde(default)]
    pub agent: Option<String>,
    pub pane_id: String,
    pub workspace_id: String,
    pub tab_id: String,
    pub agent_status: AgentStatus,
}

/// `session.snapshot` 的 `result.snapshot`：五個平行陣列加版本資訊與目前焦點。
///
/// `layouts` 刻意保留為原始 JSON（`Vec<serde_json::Value>`），不建模（change 1 不畫 pane
/// 幾何配置；設計文件 §6.1）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct SessionSnapshot {
    pub version: String,
    pub protocol: u32,
    pub workspaces: Vec<WorkspaceInfo>,
    pub tabs: Vec<TabInfo>,
    pub panes: Vec<PaneInfo>,
    pub agents: Vec<AgentInfo>,
    pub layouts: Vec<serde_json::Value>,
    #[serde(default)]
    pub focused_workspace_id: Option<String>,
    #[serde(default)]
    pub focused_tab_id: Option<String>,
    #[serde(default)]
    pub focused_pane_id: Option<String>,
}
