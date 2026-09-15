//! RED→GREEN 驗收測試（Task 1.3）：`RuntimeStore` 的整份替換與逐筆套用。

mod common;

use std::time::{Duration, SystemTime};

use cockpit_core::{
    AgentStatus, ConnectionState, FocusChange, RuntimeEvent, RuntimeState, RuntimeStore,
};

use common::{
    agent, empty_focused, pane, pane_id, runtime_id, snapshot, tab, tab_id, workspace, workspace_id,
};

#[test]
fn replace_drops_objects_missing_from_snapshot() {
    let mut store = RuntimeStore::new();
    let win = runtime_id("win");
    store.register(win.clone(), "herdr".to_string(), "tcp://win".to_string());

    let first = snapshot(
        vec![workspace("wJ", 1)],
        vec![tab("wJ:t1", "wJ", 1)],
        vec![pane("wJ:p1", "wJ", "wJ:t1")],
        vec![],
        empty_focused(),
    );
    store.replace(&win, first).expect("第一次 replace 應成功");

    let second = snapshot(
        vec![workspace("wJ", 1)],
        vec![tab("wJ:t1", "wJ", 1)],
        vec![pane("wJ:p2", "wJ", "wJ:t1")],
        vec![],
        empty_focused(),
    );
    store.replace(&win, second).expect("第二次 replace 應成功");

    let state = store.state(&win).expect("win 應已登記");
    assert!(
        !state.panes.contains_key(&pane_id("wJ:p1")),
        "被換掉的 pane 不該殘留"
    );
    assert!(state.panes.contains_key(&pane_id("wJ:p2")));
    assert!(state.workspaces.contains_key(&workspace_id("wJ")));
    assert!(state.tabs.contains_key(&tab_id("wJ:t1")));
}

#[test]
fn tab_removed_cascades_panes_and_agents() {
    let mut store = RuntimeStore::new();
    let win = runtime_id("win");
    store.register(win.clone(), "herdr".to_string(), "tcp://win".to_string());

    let snap = snapshot(
        vec![workspace("wD", 1)],
        vec![tab("wD:t3", "wD", 1), tab("wD:t9", "wD", 2)],
        vec![pane("wD:p3", "wD", "wD:t3"), pane("wD:p9", "wD", "wD:t9")],
        vec![agent(
            "wD:p3",
            "wD",
            "wD:t3",
            "claude",
            AgentStatus::Working,
        )],
        empty_focused(),
    );
    store.replace(&win, snap).expect("replace 應成功");

    store
        .apply(
            &win,
            RuntimeEvent::TabRemoved(tab_id("wD:t3")),
            SystemTime::now(),
        )
        .expect("TabRemoved 應成功");

    let state = store.state(&win).expect("win 應已登記");
    assert!(!state.tabs.contains_key(&tab_id("wD:t3")));
    assert!(!state.panes.contains_key(&pane_id("wD:p3")));
    assert!(!state.agents.contains_key(&pane_id("wD:p3")));
    assert!(
        state.tabs.contains_key(&tab_id("wD:t9")),
        "其他 tab 不受影響"
    );
    assert!(state.panes.contains_key(&pane_id("wD:p9")));
}

#[test]
fn workspace_removed_cascades_everything_below() {
    let mut store = RuntimeStore::new();
    let win = runtime_id("win");
    store.register(win.clone(), "herdr".to_string(), "tcp://win".to_string());

    let snap = snapshot(
        vec![workspace("wA", 1), workspace("wB", 2)],
        vec![tab("wA:t1", "wA", 1), tab("wB:t1", "wB", 1)],
        vec![pane("wA:p1", "wA", "wA:t1"), pane("wB:p1", "wB", "wB:t1")],
        vec![agent("wA:p1", "wA", "wA:t1", "claude", AgentStatus::Idle)],
        empty_focused(),
    );
    store.replace(&win, snap).expect("replace 應成功");

    store
        .apply(
            &win,
            RuntimeEvent::WorkspaceRemoved(workspace_id("wA")),
            SystemTime::now(),
        )
        .expect("WorkspaceRemoved 應成功");

    let state = store.state(&win).expect("win 應已登記");
    assert!(!state.workspaces.contains_key(&workspace_id("wA")));
    assert!(!state.tabs.contains_key(&tab_id("wA:t1")));
    assert!(!state.panes.contains_key(&pane_id("wA:p1")));
    assert!(!state.agents.contains_key(&pane_id("wA:p1")));

    assert!(state.workspaces.contains_key(&workspace_id("wB")));
    assert!(state.tabs.contains_key(&tab_id("wB:t1")));
    assert!(state.panes.contains_key(&pane_id("wB:p1")));
}

#[test]
fn pane_moved_replaces_old_id() {
    let mut store = RuntimeStore::new();
    let win = runtime_id("win");
    store.register(win.clone(), "herdr".to_string(), "tcp://win".to_string());

    let snap = snapshot(
        vec![workspace("wJ", 1), workspace("wK", 2)],
        vec![tab("wJ:t1", "wJ", 1), tab("wK:t1", "wK", 1)],
        vec![pane("wJ:p1", "wJ", "wJ:t1")],
        vec![],
        empty_focused(),
    );
    store.replace(&win, snap).expect("replace 應成功");

    let moved_pane = pane("wK:p7", "wK", "wK:t1");
    store
        .apply(
            &win,
            RuntimeEvent::PaneMoved {
                previous: pane_id("wJ:p1"),
                pane: moved_pane,
            },
            SystemTime::now(),
        )
        .expect("PaneMoved 應成功");

    let state = store.state(&win).expect("win 應已登記");
    assert!(!state.panes.contains_key(&pane_id("wJ:p1")));
    let new_pane = state.panes.get(&pane_id("wK:p7")).expect("wK:p7 應查得到");
    assert_eq!(new_pane.tab_id, tab_id("wK:t1"));
    assert_eq!(new_pane.workspace_id, workspace_id("wK"));
}

#[test]
fn agent_detected_none_clears_pane_agent_and_record() {
    let mut store = RuntimeStore::new();
    let win = runtime_id("win");
    store.register(win.clone(), "herdr".to_string(), "tcp://win".to_string());

    let mut p1 = pane("wJ:p1", "wJ", "wJ:t1");
    p1.agent = Some("claude".to_string());
    let snap = snapshot(
        vec![workspace("wJ", 1)],
        vec![tab("wJ:t1", "wJ", 1)],
        vec![p1],
        vec![agent("wJ:p1", "wJ", "wJ:t1", "claude", AgentStatus::Idle)],
        empty_focused(),
    );
    store.replace(&win, snap).expect("replace 應成功");

    store
        .apply(
            &win,
            RuntimeEvent::AgentDetected {
                pane_id: pane_id("wJ:p1"),
                agent: None,
            },
            SystemTime::now(),
        )
        .expect("AgentDetected 應成功");

    let state = store.state(&win).expect("win 應已登記");
    let pane = state.panes.get(&pane_id("wJ:p1")).expect("pane 仍應存在");
    assert_eq!(pane.agent, None);
    assert!(!state.agents.contains_key(&pane_id("wJ:p1")));
}

#[test]
fn agent_status_changed_updates_pane_and_agent_record() {
    let mut store = RuntimeStore::new();
    let win = runtime_id("win");
    store.register(win.clone(), "herdr".to_string(), "tcp://win".to_string());

    let mut p1 = pane("wJ:p1", "wJ", "wJ:t1");
    p1.agent = Some("claude".to_string());
    p1.agent_status = AgentStatus::Idle;
    let snap = snapshot(
        vec![workspace("wJ", 1)],
        vec![tab("wJ:t1", "wJ", 1)],
        vec![p1],
        vec![agent("wJ:p1", "wJ", "wJ:t1", "claude", AgentStatus::Idle)],
        empty_focused(),
    );
    store.replace(&win, snap).expect("replace 應成功");

    store
        .apply(
            &win,
            RuntimeEvent::AgentStatusChanged {
                pane_id: pane_id("wJ:p1"),
                status: AgentStatus::Working,
                title: Some("x".to_string()),
                agent: None,
            },
            SystemTime::now(),
        )
        .expect("AgentStatusChanged 應成功");

    let state = store.state(&win).expect("win 應已登記");
    let pane = state.panes.get(&pane_id("wJ:p1")).expect("pane 仍應存在");
    assert_eq!(pane.agent_status, AgentStatus::Working);
    assert_eq!(pane.title.as_deref(), Some("x"));
    assert_eq!(pane.agent.as_deref(), Some("claude"));

    let record = state
        .agents
        .get(&pane_id("wJ:p1"))
        .expect("agent 紀錄仍應存在");
    assert_eq!(record.agent_status, AgentStatus::Working);
    assert_eq!(record.agent, "claude");
}

#[test]
fn focus_changed_partial_keeps_other_levels() {
    let mut store = RuntimeStore::new();
    let win = runtime_id("win");
    store.register(win.clone(), "herdr".to_string(), "tcp://win".to_string());

    let mut t1 = tab("wJ:t1", "wJ", 1);
    t1.focused = true;
    let t2 = tab("wJ:t2", "wJ", 2);
    let mut focused = empty_focused();
    focused.tab_id = Some(tab_id("wJ:t1"));

    let snap = snapshot(
        vec![workspace("wJ", 1)],
        vec![t1, t2],
        vec![],
        vec![],
        focused,
    );
    store.replace(&win, snap).expect("replace 應成功");

    store
        .apply(
            &win,
            RuntimeEvent::FocusChanged(FocusChange {
                workspace_id: None,
                tab_id: Some(tab_id("wJ:t2")),
                pane_id: None,
            }),
            SystemTime::now(),
        )
        .expect("FocusChanged 應成功");

    let state = store.state(&win).expect("win 應已登記");
    assert!(!state.tabs.get(&tab_id("wJ:t1")).unwrap().focused);
    assert!(state.tabs.get(&tab_id("wJ:t2")).unwrap().focused);
    assert_eq!(state.focused.tab_id, Some(tab_id("wJ:t2")));
    assert_eq!(state.focused.workspace_id, None, "workspace 層焦點不變");
    assert_eq!(state.focused.pane_id, None, "pane 層焦點不變");
}

#[test]
fn pane_exited_marks_but_keeps_pane() {
    let mut store = RuntimeStore::new();
    let win = runtime_id("win");
    store.register(win.clone(), "herdr".to_string(), "tcp://win".to_string());

    let snap = snapshot(
        vec![workspace("wJ", 1)],
        vec![tab("wJ:t1", "wJ", 1)],
        vec![pane("wJ:p1", "wJ", "wJ:t1")],
        vec![],
        empty_focused(),
    );
    store.replace(&win, snap).expect("replace 應成功");

    let now = SystemTime::now();
    store
        .apply(&win, RuntimeEvent::PaneExited(pane_id("wJ:p1")), now)
        .expect("PaneExited 應成功");

    let state = store.state(&win).expect("win 應已登記");
    let pane = state.panes.get(&pane_id("wJ:p1")).expect("pane 仍應存在");
    assert!(pane.exited);
    assert_eq!(pane.updated_at, now);
}

// ---- Fix round 1（review findings 1–3）----

#[test]
fn pane_upserted_sets_updated_at_to_now() {
    let mut store = RuntimeStore::new();
    let win = runtime_id("win");
    store.register(win.clone(), "herdr".to_string(), "tcp://win".to_string());

    let snap = snapshot(
        vec![workspace("wJ", 1)],
        vec![tab("wJ:t1", "wJ", 1)],
        vec![],
        vec![],
        empty_focused(),
    );
    store.replace(&win, snap).expect("replace 應成功");

    let now = SystemTime::now();
    // `pane()` builder 給的 updated_at 固定是 UNIX_EPOCH，跟 `now` 不同，確保斷言有意義。
    let new_pane = pane("wJ:p1", "wJ", "wJ:t1");
    store
        .apply(&win, RuntimeEvent::PaneUpserted(new_pane), now)
        .expect("PaneUpserted 應成功");

    let state = store.state(&win).expect("win 應已登記");
    let stored = state.panes.get(&pane_id("wJ:p1")).expect("pane 應存在");
    assert_eq!(
        stored.updated_at, now,
        "PaneUpserted 應把 updated_at 覆蓋成 apply 傳入的 now"
    );
}

#[test]
fn pane_moved_sets_updated_at_to_now() {
    let mut store = RuntimeStore::new();
    let win = runtime_id("win");
    store.register(win.clone(), "herdr".to_string(), "tcp://win".to_string());

    let snap = snapshot(
        vec![workspace("wJ", 1), workspace("wK", 2)],
        vec![tab("wJ:t1", "wJ", 1), tab("wK:t1", "wK", 1)],
        vec![pane("wJ:p1", "wJ", "wJ:t1")],
        vec![],
        empty_focused(),
    );
    store.replace(&win, snap).expect("replace 應成功");

    let now = SystemTime::now();
    let moved_pane = pane("wK:p7", "wK", "wK:t1");
    store
        .apply(
            &win,
            RuntimeEvent::PaneMoved {
                previous: pane_id("wJ:p1"),
                pane: moved_pane,
            },
            now,
        )
        .expect("PaneMoved 應成功");

    let state = store.state(&win).expect("win 應已登記");
    let stored = state.panes.get(&pane_id("wK:p7")).expect("wK:p7 應查得到");
    assert_eq!(
        stored.updated_at, now,
        "PaneMoved 應把新物件的 updated_at 覆蓋成 apply 傳入的 now"
    );
}

#[test]
fn focus_changed_updates_updated_at_of_changed_panes_only() {
    let mut store = RuntimeStore::new();
    let win = runtime_id("win");
    store.register(win.clone(), "herdr".to_string(), "tcp://win".to_string());

    let earlier = SystemTime::UNIX_EPOCH;
    let mut p1 = pane("wJ:p1", "wJ", "wJ:t1");
    p1.focused = true; // 目前焦點所在
    p1.updated_at = earlier;
    let mut p2 = pane("wJ:p2", "wJ", "wJ:t1");
    p2.updated_at = earlier;
    let mut p3 = pane("wJ:p3", "wJ", "wJ:t1"); // 這次事件不影響它
    p3.updated_at = earlier;

    let mut focused = empty_focused();
    focused.pane_id = Some(pane_id("wJ:p1"));

    let snap = snapshot(
        vec![workspace("wJ", 1)],
        vec![tab("wJ:t1", "wJ", 1)],
        vec![p1, p2, p3],
        vec![],
        focused,
    );
    store.replace(&win, snap).expect("replace 應成功");

    let now = SystemTime::now();
    store
        .apply(
            &win,
            RuntimeEvent::FocusChanged(FocusChange {
                workspace_id: None,
                tab_id: None,
                pane_id: Some(pane_id("wJ:p2")),
            }),
            now,
        )
        .expect("FocusChanged 應成功");

    let state = store.state(&win).expect("win 應已登記");

    let old_focus = state.panes.get(&pane_id("wJ:p1")).unwrap();
    assert!(!old_focus.focused);
    assert_eq!(
        old_focus.updated_at, now,
        "舊焦點 pane 的 focused 從 true 變 false，應更新 updated_at"
    );

    let new_focus = state.panes.get(&pane_id("wJ:p2")).unwrap();
    assert!(new_focus.focused);
    assert_eq!(
        new_focus.updated_at, now,
        "新焦點 pane 的 focused 從 false 變 true，應更新 updated_at"
    );

    let untouched = state.panes.get(&pane_id("wJ:p3")).unwrap();
    assert!(!untouched.focused);
    assert_eq!(
        untouched.updated_at, earlier,
        "focused 值沒變的 pane 不該更新 updated_at"
    );
}

#[test]
fn agent_status_changed_creates_record_from_pane_agent_when_missing() {
    let mut store = RuntimeStore::new();
    let win = runtime_id("win");
    store.register(win.clone(), "herdr".to_string(), "tcp://win".to_string());

    let mut p1 = pane("wJ:p1", "wJ", "wJ:t1");
    p1.agent = Some("claude".to_string());
    let snap = snapshot(
        vec![workspace("wJ", 1)],
        vec![tab("wJ:t1", "wJ", 1)],
        vec![p1],
        vec![], // 刻意不放 agent 紀錄，模擬紀錄遺失但 pane 上仍有 agent 名稱
        empty_focused(),
    );
    store.replace(&win, snap).expect("replace 應成功");

    store
        .apply(
            &win,
            RuntimeEvent::AgentStatusChanged {
                pane_id: pane_id("wJ:p1"),
                status: AgentStatus::Blocked,
                title: None,
                agent: None,
            },
            SystemTime::now(),
        )
        .expect("AgentStatusChanged 應成功");

    let state = store.state(&win).expect("win 應已登記");
    let record = state
        .agents
        .get(&pane_id("wJ:p1"))
        .expect("紀錄不存在時應該用 pane 現有的 agent 名稱補建");
    assert_eq!(record.agent, "claude");
    assert_eq!(record.agent_status, AgentStatus::Blocked);
    assert_eq!(record.workspace_id, workspace_id("wJ"));
    assert_eq!(record.tab_id, tab_id("wJ:t1"));
}

#[test]
fn tab_removed_clears_orphan_agent_without_pane() {
    let mut store = RuntimeStore::new();
    let win = runtime_id("win");
    store.register(win.clone(), "herdr".to_string(), "tcp://win".to_string());

    // agent 紀錄指向 wD:t3 底下的 wD:pX，但 panes 裡沒有 wD:pX（模擬資料不一致：
    // `replace` 不驗證關聯完整性）。
    let snap = snapshot(
        vec![workspace("wD", 1)],
        vec![tab("wD:t3", "wD", 1)],
        vec![],
        vec![agent("wD:pX", "wD", "wD:t3", "claude", AgentStatus::Idle)],
        empty_focused(),
    );
    store.replace(&win, snap).expect("replace 應成功");

    store
        .apply(
            &win,
            RuntimeEvent::TabRemoved(tab_id("wD:t3")),
            SystemTime::now(),
        )
        .expect("TabRemoved 應成功");

    let state = store.state(&win).expect("win 應已登記");
    assert!(
        !state.agents.contains_key(&pane_id("wD:pX")),
        "沒有對應 pane 的孤兒 agent 紀錄，刪 tab 時也該一併清掉"
    );
}

#[test]
fn workspace_removed_clears_orphan_pane_without_tab() {
    let mut store = RuntimeStore::new();
    let win = runtime_id("win");
    store.register(win.clone(), "herdr".to_string(), "tcp://win".to_string());

    // pane 屬於 wA，但它的 tab_id 指向一個不存在於 tabs map 的 tab（模擬資料不一致）。
    let snap = snapshot(
        vec![workspace("wA", 1)],
        vec![],
        vec![pane("wA:p1", "wA", "wA:t-missing")],
        vec![],
        empty_focused(),
    );
    store.replace(&win, snap).expect("replace 應成功");

    store
        .apply(
            &win,
            RuntimeEvent::WorkspaceRemoved(workspace_id("wA")),
            SystemTime::now(),
        )
        .expect("WorkspaceRemoved 應成功");

    let state = store.state(&win).expect("win 應已登記");
    assert!(
        !state.panes.contains_key(&pane_id("wA:p1")),
        "沒有對應 tab 的孤兒 pane，刪 workspace 時也該一併清掉"
    );
    assert!(!state.pane_seq.contains_key(&pane_id("wA:p1")));
}

// ---- Fix round 2（review finding 3 未修完的那一半：replacement 事件對「父層原本就
// 缺失」的 orphan 沒清乾淨）----

#[test]
fn workspaces_replaced_clears_children_of_missing_parents() {
    let mut store = RuntimeStore::new();
    let win = runtime_id("win");
    store.register(win.clone(), "herdr".to_string(), "tcp://win".to_string());

    // wZ 從一開始就不在 workspaces map 裡（`replace` 不驗證關聯完整性），但 pane／agent
    // 直接指向它——這正是 finding 3 描述的「父層原本已缺失的 orphan」。
    let snap = snapshot(
        vec![workspace("wA", 1)],
        vec![tab("wA:t1", "wA", 1)],
        vec![pane("wA:p1", "wA", "wA:t1"), pane("wZ:p9", "wZ", "wZ:t1")],
        vec![
            agent("wA:p1", "wA", "wA:t1", "claude", AgentStatus::Idle),
            agent("wZ:p9", "wZ", "wZ:t1", "codex", AgentStatus::Idle),
        ],
        empty_focused(),
    );
    store.replace(&win, snap).expect("replace 應成功");

    store
        .apply(
            &win,
            RuntimeEvent::WorkspacesReplaced(vec![workspace("wA", 1)]),
            SystemTime::now(),
        )
        .expect("WorkspacesReplaced 應成功");

    let state = store.state(&win).expect("win 應已登記");
    assert!(
        !state.panes.contains_key(&pane_id("wZ:p9")),
        "workspace_id 指向的 wZ 從沒在 workspaces map 裡，pane 也該被濾掉"
    );
    assert!(!state.pane_seq.contains_key(&pane_id("wZ:p9")));
    assert!(!state.agents.contains_key(&pane_id("wZ:p9")));

    assert!(state.panes.contains_key(&pane_id("wA:p1")), "wA 底下的不動");
    assert!(state.agents.contains_key(&pane_id("wA:p1")));
}

#[test]
fn tabs_replaced_clears_children_of_missing_tabs() {
    let mut store = RuntimeStore::new();
    let win = runtime_id("win");
    store.register(win.clone(), "herdr".to_string(), "tcp://win".to_string());

    // tabs 一開始就是空的（`wA:t-missing` 從沒在 tabs map 裡），但 pane／agent 直接指向它。
    let snap = snapshot(
        vec![workspace("wA", 1)],
        vec![],
        vec![pane("wA:p1", "wA", "wA:t-missing")],
        vec![agent(
            "wA:p1",
            "wA",
            "wA:t-missing",
            "claude",
            AgentStatus::Idle,
        )],
        empty_focused(),
    );
    store.replace(&win, snap).expect("replace 應成功");

    store
        .apply(
            &win,
            RuntimeEvent::TabsReplaced {
                workspace_id: workspace_id("wA"),
                tabs: vec![],
            },
            SystemTime::now(),
        )
        .expect("TabsReplaced 應成功");

    let state = store.state(&win).expect("win 應已登記");
    assert!(
        !state.panes.contains_key(&pane_id("wA:p1")),
        "tab_id 指向的 wA:t-missing 從沒在 tabs map 裡，pane 也該被濾掉"
    );
    assert!(!state.pane_seq.contains_key(&pane_id("wA:p1")));
    assert!(!state.agents.contains_key(&pane_id("wA:p1")));
}

// ---- Task 1.4：Drift 判定驗收與連線狀態紀錄 ----

#[test]
fn remove_unknown_pane_is_drift_and_store_unchanged() {
    let mut store = RuntimeStore::new();
    let win = runtime_id("win");
    store.register(win.clone(), "herdr".to_string(), "tcp://win".to_string());

    let snap = snapshot(
        vec![workspace("wJ", 1)],
        vec![tab("wJ:t1", "wJ", 1)],
        vec![pane("wJ:p1", "wJ", "wJ:t1")],
        vec![],
        empty_focused(),
    );
    store.replace(&win, snap).expect("replace 應成功");

    let before = store.state(&win).cloned().expect("win 應已登記");
    let err = store
        .apply(
            &win,
            RuntimeEvent::PaneRemoved(pane_id("wZ:p9")),
            SystemTime::now(),
        )
        .expect_err("移除不存在的 pane 應回傳 Drift");
    assert_eq!(err.reason, "pane wZ:p9 不存在");
    assert_eq!(
        store.state(&win).expect("win 應已登記"),
        &before,
        "狀態庫應與套用前相等"
    );
}

#[test]
fn upsert_with_unknown_parent_is_drift() {
    let mut store = RuntimeStore::new();
    let win = runtime_id("win");
    store.register(win.clone(), "herdr".to_string(), "tcp://win".to_string());

    // workspace wZ 存在，但 tab wZ:t1 不存在：驗的是「父層 tab 不存在」這條路徑，
    // 不是 workspace 不存在那條（那條在 1.3 已被其他測試涵蓋）。
    let snap = snapshot(
        vec![workspace("wZ", 1)],
        vec![],
        vec![],
        vec![],
        empty_focused(),
    );
    store.replace(&win, snap).expect("replace 應成功");

    let before = store.state(&win).cloned().expect("win 應已登記");
    let err = store
        .apply(
            &win,
            RuntimeEvent::PaneUpserted(pane("wZ:p1", "wZ", "wZ:t1")),
            SystemTime::now(),
        )
        .expect_err("父層 tab 不存在應回傳 Drift");
    assert_eq!(err.reason, "tab wZ:t1 不存在");
    assert_eq!(
        store.state(&win).expect("win 應已登記"),
        &before,
        "狀態庫應與套用前相等"
    );
}

#[test]
fn translated_drift_passes_through() {
    let mut store = RuntimeStore::new();
    let win = runtime_id("win");
    store.register(win.clone(), "herdr".to_string(), "tcp://win".to_string());

    let snap = snapshot(
        vec![workspace("wJ", 1)],
        vec![tab("wJ:t1", "wJ", 1)],
        vec![pane("wJ:p1", "wJ", "wJ:t1")],
        vec![],
        empty_focused(),
    );
    store.replace(&win, snap).expect("replace 應成功");

    let before = store.state(&win).cloned().expect("win 應已登記");
    let reason = "pane_created payload 缺 pane".to_string();
    let err = store
        .apply(
            &win,
            RuntimeEvent::Drift {
                reason: reason.clone(),
            },
            SystemTime::now(),
        )
        .expect_err("翻譯層的 Drift 應原樣傳出");
    assert_eq!(err.reason, reason);
    assert_eq!(
        store.state(&win).expect("win 應已登記"),
        &before,
        "狀態庫應與套用前相等"
    );
}

#[test]
fn same_pane_id_in_two_runtimes_is_isolated() {
    let mut store = RuntimeStore::new();
    let win = runtime_id("win");
    let wsl = runtime_id("wsl");
    store.register(win.clone(), "herdr".to_string(), "tcp://win".to_string());
    store.register(wsl.clone(), "herdr".to_string(), "tcp://wsl".to_string());

    let make_snap = || {
        snapshot(
            vec![workspace("wJ", 1)],
            vec![tab("wJ:t1", "wJ", 1)],
            vec![pane("wJ:p1", "wJ", "wJ:t1")],
            vec![],
            empty_focused(),
        )
    };
    store
        .replace(&win, make_snap())
        .expect("win replace 應成功");
    store
        .replace(&wsl, make_snap())
        .expect("wsl replace 應成功");

    store
        .apply(
            &wsl,
            RuntimeEvent::PaneRemoved(pane_id("wJ:p1")),
            SystemTime::now(),
        )
        .expect("PaneRemoved 應成功");

    let win_state = store.state(&win).expect("win 應已登記");
    assert!(
        win_state.panes.contains_key(&pane_id("wJ:p1")),
        "win 的 wJ:p1 不受 wsl 套用事件影響"
    );
    let wsl_state = store.state(&wsl).expect("wsl 應已登記");
    assert!(!wsl_state.panes.contains_key(&pane_id("wJ:p1")));
}

#[test]
fn connection_state_defaults_to_connecting_and_keeps_reason() {
    let mut store = RuntimeStore::new();
    let win = runtime_id("win");
    let wsl = runtime_id("wsl");
    store.register(win.clone(), "herdr".to_string(), "tcp://win".to_string());
    store.register(wsl.clone(), "herdr".to_string(), "tcp://wsl".to_string());

    assert_eq!(store.connection(&win), Some(&ConnectionState::Connecting));
    assert_eq!(store.connection(&wsl), Some(&ConnectionState::Connecting));

    // 非空 snapshot：確保「只改 connection」的斷言不是靠兩邊都是空狀態矇混過去的。
    let make_snap = || {
        snapshot(
            vec![workspace("wJ", 1)],
            vec![tab("wJ:t1", "wJ", 1)],
            vec![pane("wJ:p1", "wJ", "wJ:t1")],
            vec![agent("wJ:p1", "wJ", "wJ:t1", "claude", AgentStatus::Idle)],
            empty_focused(),
        )
    };
    store
        .replace(&win, make_snap())
        .expect("win replace 應成功");
    store
        .replace(&wsl, make_snap())
        .expect("wsl replace 應成功");

    let before_win = store.state(&win).cloned().expect("win 應已登記");
    let before_wsl = store.state(&wsl).cloned().expect("wsl 應已登記");

    let disconnected = ConnectionState::Disconnected {
        reason: "WSL 發行版 Ubuntu-24.04 未啟動".to_string(),
        retry_in: Duration::from_secs(60),
    };
    store
        .set_connection(&wsl, disconnected.clone())
        .expect("set_connection 應成功");

    // wsl：整份 RuntimeState 除了 connection 之外都該跟套用前一樣——用「clone 後只換
    // connection」的預期值整份比對，而不是只挑欄位檢查。
    let expected_wsl = RuntimeState {
        connection: disconnected.clone(),
        ..before_wsl
    };
    assert_eq!(
        store.state(&wsl).expect("wsl 應已登記"),
        &expected_wsl,
        "set_connection 應只改 connection 欄位，其他欄位維持套用前的值"
    );
    assert_eq!(store.connection(&wsl), Some(&disconnected));

    // win：完全沒被動到，整份 RuntimeState 都該跟套用前相等。
    assert_eq!(
        store.state(&win).expect("win 應已登記"),
        &before_win,
        "未呼叫 set_connection 的 runtime 完整狀態不變"
    );
    assert_eq!(
        store.connection(&win),
        Some(&ConnectionState::Connecting),
        "其他 runtime 的連線狀態不變"
    );
}

#[test]
fn set_connection_on_unregistered_runtime_is_drift() {
    let mut store = RuntimeStore::new();
    let win = runtime_id("win");
    store.register(win.clone(), "herdr".to_string(), "tcp://win".to_string());

    let snap = snapshot(
        vec![workspace("wJ", 1)],
        vec![tab("wJ:t1", "wJ", 1)],
        vec![pane("wJ:p1", "wJ", "wJ:t1")],
        vec![],
        empty_focused(),
    );
    store.replace(&win, snap).expect("replace 應成功");

    let before_ids = store.runtime_ids();
    let before_win = store.state(&win).cloned().expect("win 應已登記");

    let ghost = runtime_id("ghost");
    let err = store
        .set_connection(
            &ghost,
            ConnectionState::Disconnected {
                reason: "不存在".to_string(),
                retry_in: Duration::from_secs(1),
            },
        )
        .expect_err("對未登記的 runtime 呼叫 set_connection 應回傳 Drift");
    assert_eq!(err.reason, "runtime ghost 未登記");

    assert_eq!(
        store.connection(&ghost),
        None,
        "未登記的 runtime 不該被隱式登記"
    );
    assert_eq!(
        store.runtime_ids(),
        before_ids,
        "失敗的 set_connection 不該隱式登記新 runtime"
    );
    assert_eq!(
        store.state(&win).expect("win 應已登記"),
        &before_win,
        "既有 runtime 的完整狀態不受影響"
    );
}
