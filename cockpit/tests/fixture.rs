//! Task 3.5 驗收：`cockpit/tests/fixtures/projected-state.json` 的形狀（design D14）。
//!
//! 這份 fixture 是 `examples/ui_preview.rs` 的起始資料，也是之後人眼檢查用的固定樣本；
//! 這裡只驗證它能反序列化成 `cockpit_core::ProjectedState`（欄位形狀跟 1.5 的序列化輸出
//! 一致），不重新驗證投影規則本身——那是 `cockpit-core/tests/projection.rs` 的事。

use cockpit_core::ProjectedState;

const FIXTURE: &str = include_str!("fixtures/projected-state.json");

#[test]
fn fixture_projected_state_deserializes() {
    let state: ProjectedState =
        serde_json::from_str(FIXTURE).expect("fixture 應該能反序列化成 ProjectedState");

    assert_eq!(state.version, 7, "fixture 的 version 應該是 7");
    assert_eq!(state.runtimes.len(), 2, "fixture 應該恰好有兩個 runtime");

    let win = state
        .runtimes
        .iter()
        .find(|r| r.id.as_str() == "win")
        .expect("應該有 win runtime");
    assert_eq!(win.workspaces.len(), 1, "win 應該有一個 workspace");
    assert_eq!(
        win.workspaces[0].tabs.len(),
        1,
        "win 的 workspace 應該有一個 tab"
    );
    assert_eq!(
        win.workspaces[0].tabs[0].panes.len(),
        3,
        "win 的 tab 應該有三個 pane"
    );
    assert!(
        win.workspaces[0].tabs[0]
            .panes
            .iter()
            .any(|pane| pane.exited),
        "至少一個 pane 應該是 exited: true"
    );
    assert!(
        win.workspaces[0].tabs[0]
            .panes
            .iter()
            .any(|pane| pane.agent.is_none()),
        "至少一個 pane 應該是 agent: null"
    );

    let wsl = state
        .runtimes
        .iter()
        .find(|r| r.id.as_str() == "wsl")
        .expect("應該有 wsl runtime");
    assert!(wsl.workspaces.is_empty(), "wsl 不應該有任何 workspace");

    assert_eq!(state.recent_events.len(), 3, "fixture 應該有三筆最近事件");
}
