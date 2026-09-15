//! Runtime 層的 id newtype：每個 id 都以 `RuntimeId` 限定才唯一（同一個 workspace/tab/pane
//! id 在不同 runtime 底下可能重複，呼叫端要一併帶上 `RuntimeId` 才能定位）。

use std::fmt;

use serde::{Deserialize, Serialize};

/// 一個 HERDR runtime 的識別碼，來自設定檔（例如 `"win"`、`"wsl"`）。
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RuntimeId(String);

impl RuntimeId {
    /// 用任何可轉成 `String` 的值建立一個 `RuntimeId`。
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// 借出底層字串內容。
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RuntimeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// 單一 runtime 內某個 workspace 的識別碼，須與 `RuntimeId` 搭配才全域唯一。
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct WorkspaceId(String);

impl WorkspaceId {
    /// 用任何可轉成 `String` 的值建立一個 `WorkspaceId`。
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// 借出底層字串內容。
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for WorkspaceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// 單一 runtime 內某個 tab 的識別碼，須與 `RuntimeId` 搭配才全域唯一。
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TabId(String);

impl TabId {
    /// 用任何可轉成 `String` 的值建立一個 `TabId`。
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// 借出底層字串內容。
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for TabId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// 單一 runtime 內某個 pane 的識別碼，須與 `RuntimeId` 搭配才全域唯一。
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PaneId(String);

impl PaneId {
    /// 用任何可轉成 `String` 的值建立一個 `PaneId`。
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// 借出底層字串內容。
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for PaneId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
