//! Workstream 與 Runtime 層 pane 的對應：畫面覆蓋 `Override` 與解析結果 `BindingResolution`
//! （spec `runtime-binding`）、解析規則 `resolve_binding` 與覆蓋驗證 `validate_override`
//! （task 2.3；design D1、D3、D7）。`BindingResolution` 刻意不 `derive(Serialize)`——與
//! `ConnectionState` 同一個理由：對外的 JSON 形狀（含 `agent`／`agent_status` 等投影時才補上
//! 的欄位）由 task 2.5 的 `ProjectedBinding` 負責，這裡只是 Domain 層的純粹判定結果。
//! `BindingSource` 例外地帶 serde（小寫 `auto`／`override`）：它只是一個列舉值，投影直接
//! 原樣輸出，另外包一層同形狀的列舉沒有好處。
//!
//! `resolve_binding`／`validate_override` 讀 `crate::store::RuntimeStore`——那是 `cockpit-core`
//! 自己的 Runtime 層狀態庫（不是 `herdr-client` 型別），domain 對它完全唯讀，不違反
//! `domain` 模組「不得 `use` HERDR 型別」的約束（ADR-0003）。

use serde::{Deserialize, Serialize};

use crate::domain::config::{BindingSpec, WorkstreamDef};
use crate::domain::rejection::Rejection;
use crate::store::{RuntimeState, RuntimeStore};
use crate::types::connection::ConnectionState;
use crate::types::ids::{PaneId, RuntimeId, WorkspaceId};

/// 畫面對一條 Workstream 的臨時改綁，取代自動解析（spec `runtime-binding` 「畫面覆蓋」；
/// `CONTEXT.md` Override）。存進狀態檔、重啟保留；綁定的 pane 消失即立即失效。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Override {
    /// 覆蓋指定的 runtime id。
    pub runtime: RuntimeId,
    /// 覆蓋指定的 pane id。
    pub pane_id: PaneId,
}

/// 一條 Workstream 綁定解析出的來源：自動解析，或畫面覆蓋；序列化為 `auto`／`override`
/// （spec `state-projection` 「Project 投影」）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BindingSource {
    /// 由 `resolve_binding` 依穩定特徵自動解析出。
    Auto,
    /// 來自畫面設定的 `Override`。
    Override,
}

/// 一條 Workstream 綁定的解析結果，恰好五種之一（spec `runtime-binding` 「解析結果種類」）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BindingResolution {
    /// 這條 workstream 沒有 binding，也沒有覆蓋。
    None,
    /// 要使用的 runtime 目前不是 `connected`。
    RuntimeDisconnected {
        /// 未連線的 runtime id。
        runtime: RuntimeId,
    },
    /// 恰好解析到一個 pane（自動解析或覆蓋）。
    Bound {
        /// 綁定所在的 runtime id。
        runtime: RuntimeId,
        /// 解析到的 pane id。
        pane_id: PaneId,
        /// 這個結果的來源。
        source: BindingSource,
    },
    /// 自動解析的候選為 0 個。
    Unbound {
        /// 嘗試解析的 runtime id。
        runtime: RuntimeId,
    },
    /// 自動解析的候選超過 1 個。
    Ambiguous {
        /// 嘗試解析的 runtime id。
        runtime: RuntimeId,
        /// 候選 pane id，依狀態庫順序排列。
        candidates: Vec<PaneId>,
    },
}

/// 某個 runtime 目前是否 `connected`；未登記的 runtime 視為未連線。
fn is_connected(store: &RuntimeStore, runtime: &RuntimeId) -> bool {
    matches!(
        store.connection(runtime),
        Some(ConnectionState::Connected { .. })
    )
}

/// 選填欄位的比對：`expected` 為 `None` 一律視為通過（不以此篩選），否則要求完全相等、
/// 區分大小寫（design D7）。
fn optional_field_matches(expected: &Option<String>, actual: &Option<String>) -> bool {
    match expected {
        None => true,
        Some(expected) => actual.as_deref() == Some(expected.as_str()),
    }
}

/// 把一個路徑正規化成片段序列：`\` 視為 `/`、忽略空片段（design D7；spec 「以穩定特徵
/// 自動解析」）。
fn cwd_segments(path: &str) -> Vec<&str> {
    // `split` 在 `\` 與 `/` 都當分隔字元，等同先把 `\` 換成 `/` 再切——不必配置新
    // `String` 就能借出原本字串的切片。
    path.split(['\\', '/']).filter(|s| !s.is_empty()).collect()
}

/// `binding.cwd` 是否以連續片段出現在 pane cwd 的片段序列中；`expected` 為 `None` 一律
/// 通過，pane 沒有 cwd 則一律不通過（除非 `expected` 本身也是 `None`）。
fn cwd_matches(expected: Option<&str>, actual: Option<&str>) -> bool {
    let Some(expected) = expected else {
        return true;
    };
    let expected_segments = cwd_segments(expected);
    if expected_segments.is_empty() {
        return true;
    }
    let Some(actual) = actual else {
        return false;
    };
    let actual_segments = cwd_segments(actual);
    if expected_segments.len() > actual_segments.len() {
        return false;
    }
    actual_segments
        .windows(expected_segments.len())
        .any(|window| window == expected_segments.as_slice())
}

/// 一個 pane 所屬 workspace 的 `label` 是否等於 `expected`（完全相等、區分大小寫）。
fn workspace_label_matches(
    state: &RuntimeState,
    workspace_id: &WorkspaceId,
    expected: &str,
) -> bool {
    state
        .workspaces
        .get(workspace_id)
        .and_then(|workspace| workspace.label.as_deref())
        == Some(expected)
}

/// 以穩定特徵自動解析一條 `BindingSpec`（spec `runtime-binding` 「以穩定特徵自動解析」／
/// 「解析結果種類」；design D7）。runtime 未連線時不評估候選、不沿用上次結果。
fn resolve_auto(spec: &BindingSpec, store: &RuntimeStore) -> BindingResolution {
    let runtime = spec.runtime.clone();
    if !is_connected(store, &runtime) {
        return BindingResolution::RuntimeDisconnected { runtime };
    }
    // `is_connected` 已確認 `runtime` 已登記（`connection()` 只在登記過才回 `Some`），
    // 這裡的 `state()` 理論上一定命中；仍防禦性地退回未連線，不 panic。
    let Some(state) = store.state(&runtime) else {
        return BindingResolution::RuntimeDisconnected { runtime };
    };

    let mut candidates: Vec<(u64, PaneId)> = state
        .panes
        .values()
        .filter(|pane| !pane.exited)
        .filter(|pane| workspace_label_matches(state, &pane.workspace_id, &spec.workspace))
        .filter(|pane| optional_field_matches(&spec.pane_label, &pane.label))
        .filter(|pane| optional_field_matches(&spec.agent, &pane.agent))
        .filter(|pane| cwd_matches(spec.cwd.as_deref(), pane.cwd.as_deref()))
        .map(|pane| {
            let seq = state.pane_seq.get(&pane.id).copied().unwrap_or(u64::MAX);
            (seq, pane.id.clone())
        })
        .collect();
    candidates.sort_by_key(|(seq, _)| *seq);

    match candidates.len() {
        0 => BindingResolution::Unbound { runtime },
        1 => {
            let (_, pane_id) = candidates.into_iter().next().expect("恰好一筆");
            BindingResolution::Bound {
                runtime,
                pane_id,
                source: BindingSource::Auto,
            }
        }
        _ => BindingResolution::Ambiguous {
            runtime,
            candidates: candidates.into_iter().map(|(_, pane_id)| pane_id).collect(),
        },
    }
}

/// 解析一條 Workstream 的綁定：有覆蓋時覆蓋優先，否則以 `workstream.binding` 自動解析
/// （spec `runtime-binding` 「畫面覆蓋」；design D3、D7）。
///
/// 回傳 `(解析結果, 覆蓋是否已失效)`：後者為 `true` 只發生在「覆蓋的 runtime 已
/// `connected`，但 pane 不存在或已 `exited`」——此時回傳值已經是自動解析的結果（視同覆蓋
/// 不存在），呼叫端（task 2.5 投影）據此把該 workstream 列入 `stale_overrides`、交給寫入
/// 服務非同步刪除覆蓋（design D3）。runtime 未連線時覆蓋照樣保留、不視為失效。
pub fn resolve_binding(
    workstream: &WorkstreamDef,
    override_: Option<&Override>,
    store: &RuntimeStore,
) -> (BindingResolution, bool) {
    fn auto_or_none(workstream: &WorkstreamDef, store: &RuntimeStore) -> BindingResolution {
        match &workstream.binding {
            Some(spec) => resolve_auto(spec, store),
            None => BindingResolution::None,
        }
    }

    let Some(over) = override_ else {
        return (auto_or_none(workstream, store), false);
    };

    if !is_connected(store, &over.runtime) {
        // runtime 未連線：保留覆蓋，不評估 pane 是否存在（spec「斷線期間保留覆蓋」）。
        return (
            BindingResolution::RuntimeDisconnected {
                runtime: over.runtime.clone(),
            },
            false,
        );
    }

    let pane_alive = store
        .state(&over.runtime)
        .and_then(|state| state.panes.get(&over.pane_id))
        .is_some_and(|pane| !pane.exited);

    if pane_alive {
        return (
            BindingResolution::Bound {
                runtime: over.runtime.clone(),
                pane_id: over.pane_id.clone(),
                source: BindingSource::Override,
            },
            false,
        );
    }

    // runtime connected 但 pane 不存在或已 exited：覆蓋失效，回傳自動解析結果並標記失效。
    (auto_or_none(workstream, store), true)
}

/// 設定覆蓋前的驗證：runtime 必須已登記且 `connected`，狀態庫中必須有該 pane 且未
/// `exited`（spec `runtime-binding` 「畫面覆蓋」；design D1）。
pub fn validate_override(override_: &Override, store: &RuntimeStore) -> Result<(), Rejection> {
    let Some(state) = store.state(&override_.runtime) else {
        return Err(Rejection::RuntimeNotRegistered);
    };
    if !matches!(state.connection, ConnectionState::Connected { .. }) {
        return Err(Rejection::RuntimeNotConnected);
    }
    let Some(pane) = state.panes.get(&override_.pane_id) else {
        return Err(Rejection::PaneNotFound);
    };
    if pane.exited {
        return Err(Rejection::PaneExited);
    }
    Ok(())
}
