//! HERDR observer 子集型別（spec `herdr-observer-types`、`herdr-event-subscription`）。
//!
//! 只涵蓋觀測所需的欄位子集，對未知欄位與未知值向前相容（不用 `deny_unknown_fields`；
//! `AgentStatus`、`EventKind`／`SubscriptionEventKind` 對未知值／名稱各有處理方式，見各自
//! 的文件註解）。分成五個檔案：
//!
//! - [`agent_status`]：`AgentStatus` 五值。
//! - [`snapshot`]：`session.snapshot` 回應的形狀。
//! - [`pane_read`]：`pane.read` 的參數與結果（change 1 不呼叫，供 change 3 用）。
//! - [`events`]：事件與訂閱型別。
//! - [`wire`]：request／response／event 的外層信封，供第 4 組 `Client` 使用。

mod agent_status;
mod events;
mod pane_read;
mod snapshot;
mod wire;

pub use agent_status::AgentStatus;
pub use events::{
    EventKind, EventsSubscribeParams, PaneAgentDetectedPayload, PaneAgentStatusChangedPayload,
    PaneClosedPayload, PaneFocusedPayload, PaneMovedPayload, PanePayload, Subscription,
    SubscriptionEventKind, TabClosedPayload, TabFocusedPayload, TabMovedPayload, TabPayload,
    TabRenamedPayload, UnknownEventName, WorkspaceClosedPayload, WorkspaceFocusedPayload,
    WorkspacePayload, WorkspaceRenamedPayload, WorkspacesReplacedPayload,
};
pub use pane_read::{PaneReadParams, PaneReadResult, ReadFormat, ReadSource};
pub use snapshot::{AgentInfo, PaneInfo, SessionSnapshot, TabInfo, WorkspaceInfo};
pub use wire::{
    ErrorBody, EventEnvelope, PaneReadResultEnvelope, RequestEnvelope, ResponseEnvelope,
    SessionSnapshotResult, SubscriptionStarted,
};
