//! 後端訊息代碼目錄（ui-language design D4）。
//!
//! 投影裡有幾種文字（連線斷線原因、HERDR protocol 警告、project 警告、最近事件 `drift` 的原因）原本就是給人看的繁中字串，
//! 前端要依介面語言翻譯，需要穩定的代碼與參數。每一則訊息只在這裡定義一次：[`Message`] 的
//! [`text`](Message::text)（繁中原文）與 [`msg`](Message::msg)（`{code, params}`）出自同一個變體，
//! 產生端一律用 `Message::…text()` 組字串，所以兩者不會各寫各的。
//!
//! 字串欄位（`RuntimeError`、`ConnectionState`、`DomainState.warnings`）穿過很多層，沿路只帶原文；
//! 投影時以 [`Message::classify`] 從原文還原成 [`Message`]。`classify` 是 `text` 的反向，目錄內每則
//! 訊息的來回一致由 `tests/message.rs` 守住；還原不了的原文（HERDR 或作業系統回傳的英文錯誤等）
//! 歸 [`Message::Raw`]，代碼 `raw`、參數 `text`。

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// 序列化為 `{"code": "...", "params": {...}}` 的訊息代碼；`params` 的值一律是字串。
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MessageCode {
    /// snake_case 代碼；欄位缺漏的舊 JSON 反序列化為空字串（前端視為字典沒有的代碼，退回原文）。
    pub code: String,
    /// 代碼的具名參數；沒有參數時為空物件。
    #[serde(default)]
    pub params: BTreeMap<String, String>,
}

/// 目錄內的一則訊息。欄位值是原樣字串（pane id、distro、HERDR 原文都不翻譯）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Message {
    /// `wsl_distro_not_running`（`distro`）：WSL 發行版沒在跑。
    WslDistroNotRunning { distro: String },
    /// `wsl_probe_failed`（`detail`）：`wsl.exe` 探測指令失敗。
    WslProbeFailed { detail: String },
    /// `snapshot_failed`（`detail`）：取 `session.snapshot` 失敗。
    SnapshotFailed { detail: String },
    /// `seed_snapshot_failed`（`detail`）：開事件流前的 seed snapshot 失敗。
    SeedSnapshotFailed { detail: String },
    /// `lifecycle_subscribe_failed`（`detail`）：L（生命週期）訂閱建立失敗。
    LifecycleSubscribeFailed { detail: String },
    /// `status_subscribe_failed`（`detail`）：S（每 pane 狀態）訂閱建立失敗。
    StatusSubscribeFailed { detail: String },
    /// `status_resubscribe_failed`（`detail`）：S 訂閱重開失敗。
    StatusResubscribeFailed { detail: String },
    /// `protocol_untested`（`protocol`、`tested`）：HERDR protocol 版本不在已測範圍。
    ProtocolUntested { protocol: String, tested: String },
    /// `event_stream_ended`：事件流自然結束。
    EventStreamEnded,
    /// `event_connection_error`（`label`、`detail`）：事件連線（`label` 為 `L`／`S`）I/O 錯誤。
    EventConnectionError { label: String, detail: String },
    /// `event_connection_ended`（`label`）：事件連線（`label` 為 `L`／`S`）結束。
    EventConnectionEnded { label: String },
    /// `task_stage_reset`（`task`、`stage`、`start`）：狀態檔的 stage 已不在 pipeline，退回起始 stage。
    TaskStageReset {
        task: String,
        stage: String,
        start: String,
    },
    /// `drift_workspace_not_found`（`id`）：事件指到的 workspace 不在狀態庫（drift，會觸發重拿快照）。
    DriftWorkspaceNotFound { id: String },
    /// `drift_tab_not_found`（`id`）：事件指到的 tab 不在狀態庫。
    DriftTabNotFound { id: String },
    /// `drift_pane_not_found`（`id`）：事件指到的 pane 不在狀態庫。
    DriftPaneNotFound { id: String },
    /// `drift_runtime_not_registered`（`id`）：事件或快照指到的 runtime 沒登記。
    DriftRuntimeNotRegistered { id: String },
    /// `event_payload_unparsable`（`event`、`detail`）：HERDR 事件的 payload 無法解析（`detail` 是底層 serde 英文錯誤）。
    EventPayloadUnparsable { event: String, detail: String },
    /// `raw`（`text`）：無法歸類的原文。
    Raw { text: String },
}

// 原文的固定片段，`text` 與 `classify` 共用，不各寫一份。
const WSL_DISTRO_PREFIX: &str = "WSL 發行版 ";
const WSL_DISTRO_SUFFIX: &str = " 未啟動";
const WSL_PROBE_FAILED_PREFIX: &str = "WSL 探測失敗：";
const SNAPSHOT_FAILED_PREFIX: &str = "snapshot 失敗：";
const SEED_SNAPSHOT_FAILED_PREFIX: &str = "seed snapshot 失敗：";
const LIFECYCLE_SUBSCRIBE_FAILED_PREFIX: &str = "L 訂閱建立失敗：";
const STATUS_SUBSCRIBE_FAILED_PREFIX: &str = "S 訂閱建立失敗：";
const STATUS_RESUBSCRIBE_FAILED_PREFIX: &str = "S 重開失敗：";
const PROTOCOL_UNTESTED_PREFIX: &str = "HERDR protocol ";
const PROTOCOL_UNTESTED_MIDDLE: &str = " 不在已測範圍 ";
const EVENT_STREAM_ENDED: &str = "事件流結束";
const EVENT_CONNECTION_ERROR_MIDDLE: &str = " 連線錯誤：";
const EVENT_CONNECTION_ENDED_SUFFIX: &str = " 連線結束";
const TASK_STAGE_RESET_PREFIX: &str = "task ";
const TASK_STAGE_RESET_STAGE: &str = " 的 stage「";
const TASK_STAGE_RESET_START: &str = "」已不在 pipeline 的 stages 中，已退回起始 stage「";
const TASK_STAGE_RESET_SUFFIX: &str = "」";
const DRIFT_NOT_FOUND_SUFFIX: &str = " 不存在";
const DRIFT_WORKSPACE_PREFIX: &str = "workspace ";
const DRIFT_TAB_PREFIX: &str = "tab ";
const DRIFT_PANE_PREFIX: &str = "pane ";
const DRIFT_RUNTIME_PREFIX: &str = "runtime ";
const DRIFT_RUNTIME_SUFFIX: &str = " 未登記";
const EVENT_PAYLOAD_MIDDLE: &str = " payload 無法解析：";

impl Message {
    /// 繁中原文（給舊欄位、記錄與前端的退回顯示用）。
    pub fn text(&self) -> String {
        match self {
            Message::WslDistroNotRunning { distro } => {
                format!("{WSL_DISTRO_PREFIX}{distro}{WSL_DISTRO_SUFFIX}")
            }
            Message::WslProbeFailed { detail } => format!("{WSL_PROBE_FAILED_PREFIX}{detail}"),
            Message::SnapshotFailed { detail } => format!("{SNAPSHOT_FAILED_PREFIX}{detail}"),
            Message::SeedSnapshotFailed { detail } => {
                format!("{SEED_SNAPSHOT_FAILED_PREFIX}{detail}")
            }
            Message::LifecycleSubscribeFailed { detail } => {
                format!("{LIFECYCLE_SUBSCRIBE_FAILED_PREFIX}{detail}")
            }
            Message::StatusSubscribeFailed { detail } => {
                format!("{STATUS_SUBSCRIBE_FAILED_PREFIX}{detail}")
            }
            Message::StatusResubscribeFailed { detail } => {
                format!("{STATUS_RESUBSCRIBE_FAILED_PREFIX}{detail}")
            }
            Message::ProtocolUntested { protocol, tested } => {
                format!("{PROTOCOL_UNTESTED_PREFIX}{protocol}{PROTOCOL_UNTESTED_MIDDLE}{tested}")
            }
            Message::EventStreamEnded => EVENT_STREAM_ENDED.to_string(),
            Message::EventConnectionError { label, detail } => {
                format!("{label}{EVENT_CONNECTION_ERROR_MIDDLE}{detail}")
            }
            Message::EventConnectionEnded { label } => {
                format!("{label}{EVENT_CONNECTION_ENDED_SUFFIX}")
            }
            Message::TaskStageReset { task, stage, start } => format!(
                "{TASK_STAGE_RESET_PREFIX}{task}{TASK_STAGE_RESET_STAGE}{stage}\
                 {TASK_STAGE_RESET_START}{start}{TASK_STAGE_RESET_SUFFIX}"
            ),
            Message::DriftWorkspaceNotFound { id } => {
                format!("{DRIFT_WORKSPACE_PREFIX}{id}{DRIFT_NOT_FOUND_SUFFIX}")
            }
            Message::DriftTabNotFound { id } => {
                format!("{DRIFT_TAB_PREFIX}{id}{DRIFT_NOT_FOUND_SUFFIX}")
            }
            Message::DriftPaneNotFound { id } => {
                format!("{DRIFT_PANE_PREFIX}{id}{DRIFT_NOT_FOUND_SUFFIX}")
            }
            Message::DriftRuntimeNotRegistered { id } => {
                format!("{DRIFT_RUNTIME_PREFIX}{id}{DRIFT_RUNTIME_SUFFIX}")
            }
            Message::EventPayloadUnparsable { event, detail } => {
                format!("{event}{EVENT_PAYLOAD_MIDDLE}{detail}")
            }
            Message::Raw { text } => text.clone(),
        }
    }

    /// 代碼與參數。
    pub fn msg(&self) -> MessageCode {
        let (code, params): (&str, Vec<(&str, &String)>) = match self {
            Message::WslDistroNotRunning { distro } => {
                ("wsl_distro_not_running", vec![("distro", distro)])
            }
            Message::WslProbeFailed { detail } => ("wsl_probe_failed", vec![("detail", detail)]),
            Message::SnapshotFailed { detail } => ("snapshot_failed", vec![("detail", detail)]),
            Message::SeedSnapshotFailed { detail } => {
                ("seed_snapshot_failed", vec![("detail", detail)])
            }
            Message::LifecycleSubscribeFailed { detail } => {
                ("lifecycle_subscribe_failed", vec![("detail", detail)])
            }
            Message::StatusSubscribeFailed { detail } => {
                ("status_subscribe_failed", vec![("detail", detail)])
            }
            Message::StatusResubscribeFailed { detail } => {
                ("status_resubscribe_failed", vec![("detail", detail)])
            }
            Message::ProtocolUntested { protocol, tested } => (
                "protocol_untested",
                vec![("protocol", protocol), ("tested", tested)],
            ),
            Message::EventStreamEnded => ("event_stream_ended", vec![]),
            Message::EventConnectionError { label, detail } => (
                "event_connection_error",
                vec![("label", label), ("detail", detail)],
            ),
            Message::EventConnectionEnded { label } => {
                ("event_connection_ended", vec![("label", label)])
            }
            Message::TaskStageReset { task, stage, start } => (
                "task_stage_reset",
                vec![("task", task), ("stage", stage), ("start", start)],
            ),
            Message::DriftWorkspaceNotFound { id } => {
                ("drift_workspace_not_found", vec![("id", id)])
            }
            Message::DriftTabNotFound { id } => ("drift_tab_not_found", vec![("id", id)]),
            Message::DriftPaneNotFound { id } => ("drift_pane_not_found", vec![("id", id)]),
            Message::DriftRuntimeNotRegistered { id } => {
                ("drift_runtime_not_registered", vec![("id", id)])
            }
            Message::EventPayloadUnparsable { event, detail } => (
                "event_payload_unparsable",
                vec![("event", event), ("detail", detail)],
            ),
            Message::Raw { text } => ("raw", vec![("text", text)]),
        };
        MessageCode {
            code: code.to_string(),
            params: params
                .into_iter()
                .map(|(name, value)| (name.to_string(), value.clone()))
                .collect(),
        }
    }

    /// 從繁中原文還原；不符合任何目錄格式就是 [`Message::Raw`]。
    pub fn classify(text: &str) -> Message {
        classify_known(text).unwrap_or_else(|| Message::Raw {
            text: text.to_string(),
        })
    }
}

fn classify_known(text: &str) -> Option<Message> {
    if text == EVENT_STREAM_ENDED {
        return Some(Message::EventStreamEnded);
    }
    if let Some(distro) = text
        .strip_prefix(WSL_DISTRO_PREFIX)
        .and_then(|rest| rest.strip_suffix(WSL_DISTRO_SUFFIX))
    {
        return Some(Message::WslDistroNotRunning {
            distro: distro.to_string(),
        });
    }
    let detail_of = |prefix: &str| text.strip_prefix(prefix).map(str::to_string);
    if let Some(detail) = detail_of(WSL_PROBE_FAILED_PREFIX) {
        return Some(Message::WslProbeFailed { detail });
    }
    // `seed snapshot 失敗：` 不以 `snapshot 失敗：` 開頭，兩者互不遮蔽。
    if let Some(detail) = detail_of(SNAPSHOT_FAILED_PREFIX) {
        return Some(Message::SnapshotFailed { detail });
    }
    if let Some(detail) = detail_of(SEED_SNAPSHOT_FAILED_PREFIX) {
        return Some(Message::SeedSnapshotFailed { detail });
    }
    if let Some(detail) = detail_of(LIFECYCLE_SUBSCRIBE_FAILED_PREFIX) {
        return Some(Message::LifecycleSubscribeFailed { detail });
    }
    if let Some(detail) = detail_of(STATUS_SUBSCRIBE_FAILED_PREFIX) {
        return Some(Message::StatusSubscribeFailed { detail });
    }
    if let Some(detail) = detail_of(STATUS_RESUBSCRIBE_FAILED_PREFIX) {
        return Some(Message::StatusResubscribeFailed { detail });
    }
    if let Some((protocol, tested)) = text
        .strip_prefix(PROTOCOL_UNTESTED_PREFIX)
        .and_then(|rest| rest.split_once(PROTOCOL_UNTESTED_MIDDLE))
    {
        return Some(Message::ProtocolUntested {
            protocol: protocol.to_string(),
            tested: tested.to_string(),
        });
    }
    if let Some(label) = text.strip_suffix(EVENT_CONNECTION_ENDED_SUFFIX)
        && is_connection_label(label)
    {
        return Some(Message::EventConnectionEnded {
            label: label.to_string(),
        });
    }
    if let Some((label, detail)) = text.split_once(EVENT_CONNECTION_ERROR_MIDDLE)
        && is_connection_label(label)
    {
        return Some(Message::EventConnectionError {
            label: label.to_string(),
            detail: detail.to_string(),
        });
    }
    if let Some((task, rest)) = text
        .strip_prefix(TASK_STAGE_RESET_PREFIX)
        .and_then(|rest| rest.split_once(TASK_STAGE_RESET_STAGE))
        && let Some((stage, rest)) = rest.split_once(TASK_STAGE_RESET_START)
        && let Some(start) = rest.strip_suffix(TASK_STAGE_RESET_SUFFIX)
    {
        return Some(Message::TaskStageReset {
            task: task.to_string(),
            stage: stage.to_string(),
            start: start.to_string(),
        });
    }
    // 「<種類> <id> 不存在」：id 是 HERDR 的 id（不含空白）；含空白或空的不算，避免把任意句子誤歸類。
    let id_of = |prefix: &str, suffix: &str| {
        text.strip_prefix(prefix)
            .and_then(|rest| rest.strip_suffix(suffix))
            .filter(|id| is_plain_id(id))
            .map(str::to_string)
    };
    if let Some(id) = id_of(DRIFT_WORKSPACE_PREFIX, DRIFT_NOT_FOUND_SUFFIX) {
        return Some(Message::DriftWorkspaceNotFound { id });
    }
    if let Some(id) = id_of(DRIFT_TAB_PREFIX, DRIFT_NOT_FOUND_SUFFIX) {
        return Some(Message::DriftTabNotFound { id });
    }
    if let Some(id) = id_of(DRIFT_PANE_PREFIX, DRIFT_NOT_FOUND_SUFFIX) {
        return Some(Message::DriftPaneNotFound { id });
    }
    if let Some(id) = id_of(DRIFT_RUNTIME_PREFIX, DRIFT_RUNTIME_SUFFIX) {
        return Some(Message::DriftRuntimeNotRegistered { id });
    }
    if let Some((event, detail)) = text.split_once(EVENT_PAYLOAD_MIDDLE)
        && is_plain_id(event)
    {
        return Some(Message::EventPayloadUnparsable {
            event: event.to_string(),
            detail: detail.to_string(),
        });
    }
    None
}

/// HERDR 的 id 與事件名稱：非空、不含空白。
fn is_plain_id(id: &str) -> bool {
    !id.is_empty() && !id.chars().any(char::is_whitespace)
}

/// 事件連線的 label 只有 `L`、`S` 兩種（`cockpit-herdr` 的 reader task）；限定它，避免任意含
/// 「 連線結束」字樣的原文被誤歸類。
fn is_connection_label(label: &str) -> bool {
    matches!(label, "L" | "S")
}
