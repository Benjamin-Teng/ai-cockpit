//! Domain 層的 id newtype：與 `crate::types::ids` 的 Runtime 層 id 同樣的理由——同一個字串
//! 在不同層級的意義不同（例如 workstream id 與 runtime id 撞名），呼叫端要以型別區分。
//! Domain 層與 Runtime 層唯一的接點是 `RuntimeBinding`（見 `BindingSpec`／`BindingResolution`
//! 直接重用 `crate::types::ids::{RuntimeId, PaneId}`）。

use std::fmt;

use serde::{Deserialize, Serialize};

/// 一個 Project 的識別碼，來自設定檔 `[[project]] id`。
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProjectId(String);

impl ProjectId {
    /// 用任何可轉成 `String` 的值建立一個 `ProjectId`。
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// 借出底層字串內容。
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ProjectId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// 一個 Workstream 的識別碼，須與 `ProjectId` 搭配才全域唯一（同一 Project 內唯一）。
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct WorkstreamId(String);

impl WorkstreamId {
    /// 用任何可轉成 `String` 的值建立一個 `WorkstreamId`。
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// 借出底層字串內容。
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for WorkstreamId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// 一個 Task 的識別碼，須與 `ProjectId` 搭配才全域唯一（同一 Project 內唯一）。
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TaskId(String);

impl TaskId {
    /// 用任何可轉成 `String` 的值建立一個 `TaskId`。
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// 借出底層字串內容。
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for TaskId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
