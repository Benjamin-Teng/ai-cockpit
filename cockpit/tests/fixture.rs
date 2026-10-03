//! Task 3.5 驗收：`cockpit/tests/fixtures/projected-state.json` 的形狀（design D14）。
//!
//! 這份 fixture 是 `examples/ui_preview.rs` 的起始資料，也是之後人眼檢查用的固定樣本；
//! 這裡只驗證它能反序列化成 `cockpit_core::ProjectedState`（欄位形狀跟 1.5 的序列化輸出
//! 一致，含 change `pipeline-projection` 加的 `projects`），不重新驗證投影規則本身——那是 `cockpit-core/tests/projection.rs` 的事。

use cockpit_core::{
    AgentStatus, BindingSource, Mark, ProjectedBinding, ProjectedState, StageStatus,
};

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

    // change `pipeline-projection` task 2.5：`projects`（spec `state-projection` 「Project 投影」）。
    // task 5.2 加第二個 project（Scenario D 配置，見下方），驗收腳本要斷言「兩個 Project
    // 上下順序在 runtime 卡之前」，fixture 因此固定為兩個 project。
    assert_eq!(state.projects.len(), 2, "fixture 應該恰好有兩個 project");
    let project = &state.projects[0];
    assert_eq!(project.stages, ["Spec", "Implement", "Review"]);
    assert_eq!(project.warnings.len(), 1, "fixture 應該有一筆 warning");
    assert_eq!(
        project.warnings[0], "task docs-1 的 stage \"Draft\" 已不在 stages，退回起始 stage",
        "warning 內容應該跟畫面驗收腳本斷言的字串一致"
    );
    // ui-language task 3.2：`warning_msgs` 與 `warnings` 等長，且帶代碼與參數。
    assert_eq!(project.warning_msgs.len(), project.warnings.len());
    assert_eq!(project.warning_msgs[0].code, "task_stage_reset");
    assert_eq!(project.warning_msgs[0].params["task"], "docs-1");
    // task 5.2 fix round 1（Codex review：驗收證據不足）：原本只有 be／docs／ops 三條
    // workstream，覆蓋不到 bound(override)／unbound 兩種綁定結果；補 qa（bound + override）、
    // release（unbound），讓 fixture 覆蓋 state-projection spec 「Project 投影」列的五種
    // binding.state 之中的四種（另一種 ambiguous 由第二個 project 的 frontend workstream
    // 覆蓋，見下方）。
    assert_eq!(
        project.workstreams.len(),
        5,
        "fixture 應該有五條 workstream（be／docs／ops／qa／release）"
    );
    assert!(
        matches!(
            project.workstreams[0].binding,
            ProjectedBinding::Bound {
                source: BindingSource::Auto,
                agent_status: AgentStatus::Working,
                ..
            }
        ),
        "be 應該 bound 到 working 的 pane"
    );
    assert!(matches!(
        project.workstreams[1].binding,
        ProjectedBinding::RuntimeDisconnected { .. }
    ));
    assert!(matches!(
        project.workstreams[2].binding,
        ProjectedBinding::None
    ));
    let qa = project
        .workstreams
        .iter()
        .find(|w| w.id.as_str() == "qa")
        .expect("應該有 qa workstream");
    assert!(
        matches!(
            qa.binding,
            ProjectedBinding::Bound {
                source: BindingSource::Override,
                ..
            }
        ),
        "qa 應該是 override 綁定（測「改綁」標示）"
    );
    let release = project
        .workstreams
        .iter()
        .find(|w| w.id.as_str() == "release")
        .expect("應該有 release workstream");
    assert!(
        matches!(release.binding, ProjectedBinding::Unbound { .. }),
        "release 應該是 unbound"
    );

    // task 5.2 fix round 1：補 qa-1（blocked）、release-1（ready）、ops-2（failed），讓 fixture
    // 涵蓋 `StageStatus` 六個已知值全部（原本只有 running／pending／completed 三個）。
    // 「未知 status」無法用這份 fixture 覆蓋——`status` 欄位的型別是 `StageStatus`，一個沒有
    // `#[serde(other)]` catch-all 的封閉 enum，寫一個字面值以外的字串會讓
    // `serde_json::from_str::<ProjectedState>` 直接失敗（這裡、以及 `ui_preview.rs` 啟動時
    // 都會 panic）；那個情境只能在 JS 層直接餵一個沒經過 Rust 型別系統的物件給
    // `window.onState` 測，見 `docs/research/2026-09-16/factory-floor-check.js` 的
    // `scenarioUnknownStatus`。
    // task 5.3：補 be-2（Spec）、docs-2（Review，最後一站）、qa-2（Review，最後一站），三個都
    // `mark: none`——spec `cockpit-dashboard`「頻繁重畫時按鈕仍有效」要連按 10 個「不同 task」
    // 的「Completed」，而「Completed」只在 `mark` 為 `none` 的節點出現；兩個最後一站的 task
    // 同時是「最後一個 stage 不顯示推進」的真實樣本。
    assert_eq!(
        project.tasks.len(),
        9,
        "fixture 應該有九個 task（be-1／docs-1／ops-1／qa-1／release-1／ops-2／be-2／docs-2／qa-2）"
    );
    let statuses: Vec<StageStatus> = project.tasks[0..3].iter().map(|task| task.status).collect();
    assert_eq!(
        statuses,
        [
            StageStatus::Running,
            StageStatus::Pending,
            StageStatus::Completed
        ]
    );
    assert_eq!(project.tasks[2].mark, Mark::Completed);
    let status_of = |id: &str| {
        project
            .tasks
            .iter()
            .find(|t| t.id.as_str() == id)
            .unwrap_or_else(|| panic!("應該有 task {id}"))
            .status
    };
    assert_eq!(status_of("qa-1"), StageStatus::Blocked);
    assert_eq!(status_of("release-1"), StageStatus::Ready);
    assert_eq!(status_of("ops-2"), StageStatus::Failed);

    // task 5.2：第二個 project 是 spec `cockpit-dashboard`「Factory Floor」「Scenario D 的
    // 畫面」情境本身——stages `Plan`／`Implement`／`Test`，workstream `backend`／`frontend`／
    // `tests` 各有一個 running 的 task，分別在 `Implement`／`Plan`／`Test`。
    let scenario_d = &state.projects[1];
    assert_eq!(scenario_d.id.as_str(), "p");
    assert_eq!(scenario_d.stages, ["Plan", "Implement", "Test"]);
    assert_eq!(
        scenario_d.workstreams.len(),
        3,
        "Scenario D 應該有三條 workstream"
    );
    assert_eq!(scenario_d.tasks.len(), 3, "Scenario D 應該有三個 task");
    let placement: Vec<(&str, &str)> = scenario_d
        .tasks
        .iter()
        .map(|task| (task.workstream.as_str(), task.stage.as_str()))
        .collect();
    assert_eq!(
        placement,
        [
            ("backend", "Implement"),
            ("frontend", "Plan"),
            ("tests", "Test"),
        ],
        "Scenario D 的三個 task 應該分別位於 (backend, Implement)、(frontend, Plan)、(tests, Test)"
    );
    assert!(
        scenario_d
            .tasks
            .iter()
            .all(|task| task.status == StageStatus::Running),
        "Scenario D 的三個 task 都應該是 running"
    );

    // task 5.3：畫面上「Completed」按鈕的數量＝`mark` 為 `none` 的 task 數，至少要 10 個，
    // 瀏覽器驗收腳本才能連按 10 個不同 task（spec「頻繁重畫時按鈕仍有效」）。
    let unmarked = state
        .projects
        .iter()
        .flat_map(|p| p.tasks.iter())
        .filter(|task| task.mark == Mark::None)
        .count();
    assert!(
        unmarked >= 10,
        "fixture 至少要有 10 個 mark 為 none 的 task（實際 {unmarked}）"
    );
}

/// ui-language task 3.2：沒有任何 `*_msg` 欄位的舊形狀 fixture 仍能反序列化（`#[serde(default)]`）。
#[test]
fn fixture_without_message_code_fields_still_deserializes() {
    fn strip(value: &mut serde_json::Value) {
        match value {
            serde_json::Value::Object(map) => {
                for key in [
                    "reason_msg",
                    "protocol_warning_msg",
                    "warning_msgs",
                    "detail_msg",
                ] {
                    map.remove(key);
                }
                map.values_mut().for_each(strip);
            }
            serde_json::Value::Array(items) => items.iter_mut().for_each(strip),
            _ => {}
        }
    }
    let mut value: serde_json::Value = serde_json::from_str(FIXTURE).expect("fixture 是合法 JSON");
    strip(&mut value);
    assert!(!value.to_string().contains("_msg"), "前置：新欄位都已移除");

    let state: ProjectedState = serde_json::from_value(value).expect("舊形狀應該能反序列化");
    let project = &state.projects[0];
    assert_eq!(project.warnings.len(), 1);
    assert!(project.warning_msgs.is_empty(), "缺欄位取預設空陣列");
}
