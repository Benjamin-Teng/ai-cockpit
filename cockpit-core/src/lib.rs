//! Runtime 層型別骨架：獨立於 HERDR，不依賴 `herdr-client` 或任何 `cockpit-*` crate（ADR-0003）。

pub mod domain;
pub mod driver;
pub mod handle;
pub mod message;
pub mod projection;
pub mod runtime;
pub mod store;
pub mod types;

pub use domain::{
    BindingResolution, BindingSource, BindingSpec, DomainState, Mark, Override, ProgressOp,
    ProjectDef, ProjectId, Rejection, StageStatus, TaskDef, TaskId, TaskProgress, WorkstreamDef,
    WorkstreamId,
};
pub use driver::{Policy, run};
pub use handle::{StoreHandle, spawn_projector, spawn_projector_with_stale_sink};
pub use message::{Message, MessageCode};
pub use projection::{
    ProjectedBinding, ProjectedConnection, ProjectedEvent, ProjectedPane, ProjectedProject,
    ProjectedRuntime, ProjectedState, ProjectedTab, ProjectedTask, ProjectedWorkspace,
    ProjectedWorkstream, StaleOverride, project, project_with_stale,
};
pub use runtime::{
    AgentRuntime, AnsiColor, OutputFormat, OutputSegment, PaneOutput, READ_OUTPUT_FAILED_PREFIX,
    RuntimeError, RuntimeEvents, SegmentStyle, UnstartedEvents,
};
pub use store::{Drift, RECENT_EVENTS_CAPACITY, RecentEvent, RuntimeState, RuntimeStore};
pub use types::{
    Agent, AgentStatus, ConnectionState, FocusChange, Focused, Pane, PaneId, RuntimeEvent,
    RuntimeId, RuntimeSnapshot, Tab, TabId, Workspace, WorkspaceId,
};
