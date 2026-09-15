//! Runtime 層型別骨架：獨立於 HERDR，不依賴 `herdr-client` 或任何 `cockpit-*` crate（ADR-0003）。

pub mod driver;
pub mod handle;
pub mod projection;
pub mod runtime;
pub mod store;
pub mod types;

pub use driver::{Policy, run};
pub use handle::{StoreHandle, spawn_projector};
pub use projection::{
    ProjectedConnection, ProjectedEvent, ProjectedPane, ProjectedRuntime, ProjectedState,
    ProjectedTab, ProjectedWorkspace, project,
};
pub use runtime::{AgentRuntime, RuntimeError, RuntimeEvents, UnstartedEvents};
pub use store::{Drift, RECENT_EVENTS_CAPACITY, RecentEvent, RuntimeState, RuntimeStore};
pub use types::{
    Agent, AgentStatus, ConnectionState, FocusChange, Focused, Pane, PaneId, RuntimeEvent,
    RuntimeId, RuntimeSnapshot, Tab, TabId, Workspace, WorkspaceId,
};
