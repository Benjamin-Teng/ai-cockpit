//! Runtime 層型別：獨立於 HERDR，`cockpit-core` 不依賴 `herdr-client` 或任何其他
//! `cockpit-*` crate（ADR-0003）。這裡的型別是驅動器、狀態庫與 UI 共用的資料模型。

pub mod agent_status;
pub mod connection;
pub mod events;
pub mod ids;
pub mod model;

pub use agent_status::AgentStatus;
pub use connection::ConnectionState;
pub use events::{FocusChange, RuntimeEvent};
pub use ids::{PaneId, RuntimeId, TabId, WorkspaceId};
pub use model::{Agent, Focused, Pane, RuntimeSnapshot, Tab, Workspace};
