//! `StageStatus`：Cockpit 投影出的狀態，序列化為小寫字串（spec `pipeline-domain` 「StageStatus
//! 推導」；`CONTEXT.md` StageStatus）。`derive_status`（task 2.4）依標記 → 依賴 Pending → 綁定
//! agent 狀態 → Ready 的優先序推導出這個狀態（design D8）。`Completed`／`Failed` 只能來自
//! `Mark`——`derive_status` 的實作（先判標記，之後的分支完全推不出這兩個值）與底下的測試
//! （`cockpit-core/tests/domain_status.rs` 的 `done_is_not_completed`）共同保證這條規則。
//!
//! `derive_status` 額外接觸 `crate::types::agent_status::AgentStatus`——這是 `cockpit-core`
//! 自己對 Runtime 層 agent 狀態的型別（不是 `herdr-client` 型別），沒有違反 ADR-0003「不依賴
//! herdr-client 或任何 cockpit-* crate」。它以獨立參數傳入，不是讓 `derive_status` 自己拿
//! `RuntimeStore` 去查：呼叫端（task 2.5 投影）從 `resolve_binding` 的結果與 `RuntimeStore`
//! 查出綁定 pane 目前的 `AgentStatus` 一次，同一 workstream 底下的每個 Task 各自呼叫
//! `derive_status` 時重複傳入同一份查詢結果，不必每個 Task 各自查一次狀態庫（spec 「同一
//! workstream 的多個 Task 共用同一個綁定結果、各自推導」）。

use serde::{Deserialize, Serialize};

use crate::domain::binding::BindingResolution;
use crate::domain::progress::Mark;
use crate::types::agent_status::AgentStatus;

/// 一個 Task 投影出的狀態，六選一（spec `pipeline-domain` 「StageStatus 推導」）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StageStatus {
    /// 依賴的 task 尚未全部標為 `completed`。
    Pending,
    /// 沒有被依賴擋住，但目前沒有對應的 `running`／`blocked` 訊號。
    Ready,
    /// 綁定的 pane 目前 `AgentStatus` 為 `working`。
    Running,
    /// 綁定的 pane 目前 `AgentStatus` 為 `blocked`。
    Blocked,
    /// 已標 Failed。
    Failed,
    /// 已標 Completed。
    Completed,
}

/// 依 spec `pipeline-domain` 「StageStatus 推導」的優先序推導一個 Task 的 `StageStatus`
/// （design D8）：
///
/// 1. `mark` 為 `Completed`／`Failed` → 對應的 `StageStatus`（優先於下面所有規則，包含依賴
///    未完成與綁定 agent 狀態）。
/// 2. `dependency_marks` 中任一不是 `Mark::Completed` → `Pending`。
/// 3. 這個 Task 是所屬 workstream 的目前 task（`is_active`），且 `binding` 解析為 `Bound`（`resolve_binding` 只在對應 runtime 已 `connected` 時才回傳
///    `Bound`，這裡不必再另外檢查一次連線狀態）：`agent_status` 為 `Some(AgentStatus::Working)`
///    → `Running`；`Some(AgentStatus::Blocked)` → `Blocked`；其餘（`Idle`／`Done`／`Unknown`／
///    `None`，即 pane 沒有 agent 或呼叫端沒查到）→ `Ready`。
/// 4. 其餘情況（這個 Task 不是目前 task、workstream 沒有目前 task、`binding` 是 `None`／`RuntimeDisconnected`／`Unbound`／`Ambiguous`）→ `Ready`。
///
/// `dependency_marks` 只需要依賴 task 的 `Mark`（規則 2 的判定依據只看標記，不看依賴 task
/// 所在的 Stage）；呼叫端自行從 `TaskDef::depends_on` 映射出對應 task 的 `Mark` 清單，空切片
/// 表示沒有依賴（一律視為通過）。
///
/// 純函數：不做任何 IO，也不查 `RuntimeStore`——所有輸入都已經是呼叫端算好的值。
pub fn derive_status(
    mark: Mark,
    dependency_marks: &[Mark],
    binding: &BindingResolution,
    agent_status: Option<AgentStatus>,
    is_active: bool,
) -> StageStatus {
    match mark {
        Mark::Completed => return StageStatus::Completed,
        Mark::Failed => return StageStatus::Failed,
        Mark::None => {}
    }

    if dependency_marks
        .iter()
        .any(|dependency_mark| *dependency_mark != Mark::Completed)
    {
        return StageStatus::Pending;
    }

    // 沒有目前 task 時不從 agent 狀態猜是哪個 task 在做（progress-model task 2.3）。
    if is_active && matches!(binding, BindingResolution::Bound { .. }) {
        return match agent_status {
            Some(AgentStatus::Working) => StageStatus::Running,
            Some(AgentStatus::Blocked) => StageStatus::Blocked,
            _ => StageStatus::Ready,
        };
    }

    StageStatus::Ready
}
