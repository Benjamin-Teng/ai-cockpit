//! RED→GREEN 驗收測試（Task 2.5，change `pipeline-projection`）：投影加上 `projects`（spec
//! `state-projection` 「投影形狀」「Project 投影」）、`StoreHandle` 持有 Domain 狀態並觸發投影、
//! 投影任務把非空 `stale_overrides` 送到可注入的 `mpsc::Sender`（design D2、D3）。
//!
//! 投影任務的時間控制沿用 `projector.rs` 檔頭說明的策略：`start_paused` ＋ `rx.changed()` 搭配
//! `tokio::time::timeout`，逾時本身就是「沒有廣播」的證據。

mod common;

use std::time::{Duration, SystemTime};

use cockpit_core::{
    AgentStatus, BindingSpec, ConnectionState, DomainState, Mark, Override, ProjectDef, ProjectId,
    RuntimeStore, StoreHandle, TaskDef, TaskId, TaskProgress, WorkstreamDef, WorkstreamId, project,
    spawn_projector, spawn_projector_with_stale_sink,
};
use serde_json::json;
use tokio::sync::mpsc;

use common::{empty_focused, pane, pane_id, runtime_id, snapshot, tab, workspace};

const EXPECT_BROADCAST_TIMEOUT: Duration = Duration::from_secs(5);

fn epoch_secs(secs: u64) -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(secs)
}

fn connected_state() -> ConnectionState {
    ConnectionState::Connected {
        since: SystemTime::UNIX_EPOCH,
        server_version: "0.9.0".to_string(),
        protocol: 1,
        last_snapshot_at: SystemTime::UNIX_EPOCH,
        settled: true,
        protocol_warning: None,
    }
}

/// workspace `wJ`（label `ai-cockpit`）底下 tab `wJ:t1`，pane `wJ:p1`（agent `claude`，狀態
/// `status`）。
fn scenario_c_snapshot(status: AgentStatus) -> cockpit_core::RuntimeSnapshot {
    let mut ws = workspace("wJ", 1);
    ws.label = Some("ai-cockpit".to_string());
    let mut bound = pane("wJ:p1", "wJ", "wJ:t1");
    bound.agent = Some("claude".to_string());
    bound.agent_status = status;
    snapshot(
        vec![ws],
        vec![tab("wJ:t1", "wJ", 1)],
        vec![bound],
        vec![],
        empty_focused(),
    )
}

/// runtime `win` 已 `connected`，內容為 [`scenario_c_snapshot`]。
fn scenario_c_store(status: AgentStatus) -> RuntimeStore {
    let mut store = RuntimeStore::new();
    let win = runtime_id("win");
    store.register(win.clone(), "herdr".to_string(), "tcp://win".to_string());
    store
        .set_connection(&win, connected_state())
        .expect("剛登記過");
    store
        .replace(&win, scenario_c_snapshot(status))
        .expect("replace 應成功");
    store
}

/// Project `p`：stages `Design`／`Implement`／`Review`；workstream `be` 綁 `win`／`ai-cockpit`／
/// agent `claude`，workstream `fe` 沒有 binding；task `A`（`be`，起始 `Implement`）、task `B`
/// （`fe`，依賴 `A`）。
fn scenario_c_project() -> ProjectDef {
    ProjectDef {
        id: ProjectId::new("p"),
        name: "Project P".to_string(),
        stages: vec![
            "Design".to_string(),
            "Implement".to_string(),
            "Review".to_string(),
        ],
        workstreams: vec![
            WorkstreamDef {
                id: WorkstreamId::new("be"),
                name: "Backend".to_string(),
                binding: Some(BindingSpec {
                    runtime: runtime_id("win"),
                    workspace: "ai-cockpit".to_string(),
                    pane_label: None,
                    cwd: None,
                    agent: Some("claude".to_string()),
                }),
                pinned_pane: None,
            },
            WorkstreamDef {
                id: WorkstreamId::new("fe"),
                name: "fe".to_string(),
                binding: None,
                pinned_pane: None,
            },
        ],
        tasks: vec![
            TaskDef {
                id: TaskId::new("A"),
                title: "Task A".to_string(),
                workstream: WorkstreamId::new("be"),
                stage: "Implement".to_string(),
                depends_on: vec![],
            },
            TaskDef {
                id: TaskId::new("B"),
                title: "B".to_string(),
                workstream: WorkstreamId::new("fe"),
                stage: "Design".to_string(),
                depends_on: vec![TaskId::new("A")],
            },
        ],
        repo: None,
    }
}

/// spec 「沒有 Project」：`projects` 為 `[]`，其餘欄位與沒有 Domain 層時相同。
#[test]
fn no_projects_projects_is_empty_array() {
    let store = scenario_c_store(AgentStatus::Idle);
    let projected = project(&store, &DomainState::default(), 3, epoch_secs(10));

    assert!(projected.projects.is_empty());
    let value = serde_json::to_value(&projected).expect("序列化應成功");
    assert_eq!(value["projects"], json!([]));
    // repo-projects task 3.2：沒有 Project 也沒有偵測到的 repo 時，`detected_repos` 為 `[]`。
    assert_eq!(value["detected_repos"], json!([]));
    let keys: Vec<&str> = value
        .as_object()
        .expect("頂層是物件")
        .keys()
        .map(String::as_str)
        .collect();
    for key in [
        "version",
        "generated_at",
        "runtimes",
        "projects",
        "detected_repos",
        "recent_events",
    ] {
        assert!(keys.contains(&key), "頂層應有 {key}");
    }
    assert_eq!(keys.len(), 6, "頂層只應有六個欄位：{keys:?}");
    assert_eq!(value["runtimes"][0]["id"], json!("win"));
}

/// spec 「Scenario C 的 JSON 欄位」：`A` 的 `stage`＝`Implement`、`status`＝`running`；`be` 的
/// `binding` 逐欄位等於 spec 給定的物件；另驗 project／workstream／task 其餘欄位形狀。
#[test]
fn scenario_c_binding_json_fields() {
    let store = scenario_c_store(AgentStatus::Working);
    let mut domain = DomainState::from_projects(vec![scenario_c_project()]);
    // spec Scenario C 的 GIVEN：`be` 的目前 task 為 `A`。
    domain
        .set_active(
            &ProjectId::new("p"),
            &WorkstreamId::new("be"),
            &TaskId::new("A"),
        )
        .expect("A 屬於 be 且無標記");
    let projected = project(&store, &domain, 1, epoch_secs(10));
    let value = serde_json::to_value(&projected).expect("序列化應成功");

    let p = &value["projects"][0];
    assert_eq!(p["id"], json!("p"));
    assert_eq!(p["name"], json!("Project P"));
    assert_eq!(p["stages"], json!(["Design", "Implement", "Review"]));
    assert_eq!(p["warnings"], json!([]));
    assert_eq!(p["warning_msgs"], json!([]));

    let be = p["workstreams"]
        .as_array()
        .expect("workstreams 是陣列")
        .iter()
        .find(|w| w["id"] == json!("be"))
        .expect("應有 be");
    assert_eq!(be["name"], json!("Backend"));
    assert_eq!(
        be["binding"],
        json!({
            "state": "bound",
            "runtime": "win",
            "pane_id": "wJ:p1",
            "source": "auto",
            "agent": "claude",
            "agent_status": "working"
        })
    );
    assert_eq!(be["active_task"], json!("A"));
    assert_eq!(be["activity_undeclared"], json!(false));
    assert_eq!(p["workstreams"][1]["binding"], json!({"state": "none"}));
    assert_eq!(p["workstreams"][1]["active_task"], json!(null));
    assert_eq!(p["workstreams"][1]["activity_undeclared"], json!(false));

    let a = p["tasks"]
        .as_array()
        .expect("tasks 是陣列")
        .iter()
        .find(|t| t["id"] == json!("A"))
        .expect("應有 A");
    assert_eq!(
        *a,
        json!({
            "id": "A",
            "title": "Task A",
            "workstream": "be",
            "stage": "Implement",
            "mark": "none",
            "status": "running",
            "depends_on": [],
            "sync": null
        })
    );
    assert_eq!(p["tasks"][1]["status"], json!("pending"));
    assert_eq!(p["tasks"][1]["depends_on"], json!(["A"]));
}

/// spec 「工作中但未宣告」：`be` 為 bound、`agent_status` 為 working／blocked 且沒有目前 task →
/// `active_task` 為 `null`、`activity_undeclared` 為 `true`，task 為 `ready`（不猜）；idle 則為
/// `false`；有目前 task 時為 `false`。
#[test]
fn activity_undeclared_only_when_bound_busy_and_no_active_task() {
    let pid = ProjectId::new("p");
    let be = WorkstreamId::new("be");
    let undeclared = |status: AgentStatus, active: bool| {
        let store = scenario_c_store(status);
        let mut domain = DomainState::from_projects(vec![scenario_c_project()]);
        if active {
            domain
                .set_active(&pid, &be, &TaskId::new("A"))
                .expect("A 屬於 be");
        }
        let value =
            serde_json::to_value(project(&store, &domain, 1, epoch_secs(0))).expect("序列化");
        let ws = value["projects"][0]["workstreams"][0].clone();
        let task_status = value["projects"][0]["tasks"][0]["status"].clone();
        (ws, task_status)
    };

    for status in [AgentStatus::Working, AgentStatus::Blocked] {
        let (ws, task_status) = undeclared(status, false);
        assert_eq!(ws["active_task"], json!(null));
        assert_eq!(ws["activity_undeclared"], json!(true), "{status:?}");
        assert_eq!(task_status, json!("ready"), "沒有目前 task 不猜");
    }
    for status in [AgentStatus::Idle, AgentStatus::Done, AgentStatus::Unknown] {
        let (ws, _) = undeclared(status, false);
        assert_eq!(ws["activity_undeclared"], json!(false), "{status:?}");
    }
    let (ws, task_status) = undeclared(AgentStatus::Working, true);
    assert_eq!(ws["activity_undeclared"], json!(false));
    assert_eq!(task_status, json!("running"));

    // 非 bound（fe 沒有 binding）即使沒有目前 task 也是 false。
    let store = scenario_c_store(AgentStatus::Working);
    let domain = DomainState::from_projects(vec![scenario_c_project()]);
    let value = serde_json::to_value(project(&store, &domain, 1, epoch_secs(0))).expect("序列化");
    assert_eq!(
        value["projects"][0]["workstreams"][1]["activity_undeclared"],
        json!(false)
    );
}

/// 兩份只差 `active_task` 的投影，`content_eq` 必須為 false（design D9：目前 task 改變要遞增
/// version）。
#[test]
fn content_eq_is_sensitive_to_active_task() {
    let store = scenario_c_store(AgentStatus::Idle);
    let without = DomainState::from_projects(vec![scenario_c_project()]);
    let mut with = without.clone();
    with.set_active(
        &ProjectId::new("p"),
        &WorkstreamId::new("be"),
        &TaskId::new("A"),
    )
    .expect("A 屬於 be");

    let a = project(&store, &without, 1, epoch_secs(0));
    let b = project(&store, &with, 1, epoch_secs(0));
    assert!(a.content_eq(&a.clone()));
    assert!(!a.content_eq(&b), "只差 active_task 的投影不應相等");
}

/// `binding` 另外四種狀態的欄位（spec 「Project 投影」）：`unbound` 帶 `runtime`，
/// `runtime_disconnected` 帶 `runtime` 與 `source`（自動為 `auto`、覆蓋為 `override`），`ambiguous` 帶 `runtime` 與 `candidates`，覆蓋的 `source` 為 `override`，warnings
/// 與 mark 原樣輸出。
#[test]
fn binding_variants_and_domain_fields_serialize() {
    let mut project_def = scenario_c_project();
    // fe 改綁不存在的 workspace → unbound；再加一條綁 wsl（未登記）→ runtime_disconnected。
    project_def.workstreams[1].binding = Some(BindingSpec {
        runtime: runtime_id("win"),
        workspace: "nope".to_string(),
        pane_label: None,
        cwd: None,
        agent: None,
    });
    project_def.workstreams.push(WorkstreamDef {
        id: WorkstreamId::new("ops"),
        name: "ops".to_string(),
        binding: Some(BindingSpec {
            runtime: runtime_id("wsl"),
            workspace: "x".to_string(),
            pane_label: None,
            cwd: None,
            agent: None,
        }),
        pinned_pane: None,
    });
    project_def.workstreams.push(WorkstreamDef {
        id: WorkstreamId::new("many"),
        name: "many".to_string(),
        binding: Some(BindingSpec {
            runtime: runtime_id("win"),
            workspace: "ai-cockpit".to_string(),
            pane_label: None,
            cwd: None,
            agent: None,
        }),
        pinned_pane: None,
    });
    // 覆蓋指向未登記的 wsl → 同樣是 runtime_disconnected，但來源為 override。
    project_def.workstreams.push(WorkstreamDef {
        id: WorkstreamId::new("ovr"),
        name: "ovr".to_string(),
        binding: None,
        pinned_pane: None,
    });

    let mut store = scenario_c_store(AgentStatus::Idle);
    let win = runtime_id("win");
    let mut ws = workspace("wJ", 1);
    ws.label = Some("ai-cockpit".to_string());
    let mut p1 = pane("wJ:p1", "wJ", "wJ:t1");
    p1.agent = Some("claude".to_string());
    store
        .replace(
            &win,
            snapshot(
                vec![ws],
                vec![tab("wJ:t1", "wJ", 1)],
                vec![p1, pane("wJ:p2", "wJ", "wJ:t1")],
                vec![],
                empty_focused(),
            ),
        )
        .expect("replace 應成功");

    let mut domain = DomainState::from_projects(vec![project_def]);
    let pid = ProjectId::new("p");
    domain.overrides.entry(pid.clone()).or_default().insert(
        WorkstreamId::new("be"),
        Override {
            runtime: win.clone(),
            pane_id: pane_id("wJ:p2"),
        },
    );
    domain.overrides.entry(pid.clone()).or_default().insert(
        WorkstreamId::new("ovr"),
        Override {
            runtime: runtime_id("wsl"),
            pane_id: pane_id("w1:p1"),
        },
    );
    domain.warnings.insert(
        pid.clone(),
        vec![
            "task B 的 stage 已不存在".to_string(),
            "task t1 的 stage「Old」已不在 pipeline 的 stages 中，已退回起始 stage「Spec」"
                .to_string(),
        ],
    );
    domain.progress.get_mut(&pid).expect("有 p").insert(
        TaskId::new("A"),
        TaskProgress {
            stage: "Review".to_string(),
            mark: Mark::Completed,
        },
    );

    let value = serde_json::to_value(project(&store, &domain, 1, epoch_secs(0))).expect("序列化");
    let p = &value["projects"][0];
    assert_eq!(
        p["warnings"],
        json!([
            "task B 的 stage 已不存在",
            "task t1 的 stage「Old」已不在 pipeline 的 stages 中，已退回起始 stage「Spec」"
        ])
    );
    // ui-language task 3.2：`warning_msgs` 與 `warnings` 等長、同順序；無法歸類者為 raw。
    assert_eq!(
        p["warning_msgs"],
        json!([
            {"code": "raw", "params": {"text": "task B 的 stage 已不存在"}},
            {"code": "task_stage_reset", "params": {"task": "t1", "stage": "Old", "start": "Spec"}}
        ])
    );
    assert_eq!(
        p["workstreams"][0]["binding"],
        json!({
            "state": "bound",
            "runtime": "win",
            "pane_id": "wJ:p2",
            "source": "override",
            "agent": null,
            "agent_status": "idle"
        })
    );
    assert_eq!(
        p["workstreams"][1]["binding"],
        json!({"state": "unbound", "runtime": "win"})
    );
    assert_eq!(
        p["workstreams"][2]["binding"],
        json!({"state": "runtime_disconnected", "runtime": "wsl", "source": "auto"})
    );
    assert_eq!(
        p["workstreams"][4]["binding"],
        json!({"state": "runtime_disconnected", "runtime": "wsl", "source": "override"})
    );
    assert_eq!(
        p["workstreams"][3]["binding"],
        json!({"state": "ambiguous", "runtime": "win", "candidates": ["wJ:p1", "wJ:p2"]})
    );
    assert_eq!(p["tasks"][0]["stage"], json!("Review"));
    assert_eq!(p["tasks"][0]["mark"], json!("completed"));
    assert_eq!(p["tasks"][0]["status"], json!("completed"));
    // A 已 completed，B 的依賴通過；fe unbound → ready。
    assert_eq!(p["tasks"][1]["status"], json!("ready"));
}

/// spec 「進度操作遞增 version」：Runtime 層沒有變動，Domain 狀態改變 → 觀察者收到 version+1，
/// 其中該 task 的進度已更新。
#[tokio::test(start_paused = true)]
async fn domain_change_increments_version() {
    let domain = DomainState::from_projects(vec![scenario_c_project()]);
    let handle = StoreHandle::new_with_domain(scenario_c_store(AgentStatus::Idle), domain.clone());
    let mut rx = handle.subscribe();
    assert_eq!(handle.current().version, 1);
    assert_eq!(handle.current().projects[0].tasks[0].stage, "Implement");
    let projector = spawn_projector(handle.clone());

    let mut advanced = domain.clone();
    advanced
        .progress
        .get_mut(&ProjectId::new("p"))
        .expect("有 p")
        .insert(
            TaskId::new("A"),
            TaskProgress {
                stage: "Review".to_string(),
                mark: Mark::None,
            },
        );
    handle.set_domain(advanced.clone());

    tokio::time::timeout(EXPECT_BROADCAST_TIMEOUT, rx.changed())
        .await
        .expect("應在合理時間內收到廣播")
        .expect("channel 應仍存活");
    let current = rx.borrow().clone();
    assert_eq!(current.version, 2, "domain 變動應讓 version +1");
    assert_eq!(current.projects[0].tasks[0].stage, "Review");
    assert_eq!(handle.with_domain(|d| d.clone()), advanced);

    projector.abort();
}

/// spec 「無效操作不遞增」的投影面：寫入與現況相同的 Domain 狀態 → version 不變、沒有廣播。
#[tokio::test(start_paused = true)]
async fn same_domain_rewrite_does_not_increment() {
    let domain = DomainState::from_projects(vec![scenario_c_project()]);
    let handle = StoreHandle::new_with_domain(scenario_c_store(AgentStatus::Idle), domain.clone());
    let mut rx = handle.subscribe();
    let projector = spawn_projector(handle.clone());

    handle.set_domain(domain.clone());

    let result = tokio::time::timeout(Duration::from_millis(500), rx.changed()).await;
    assert!(result.is_err(), "內容沒變不該廣播");
    assert_eq!(handle.current().version, 1, "version 應維持 1");

    projector.abort();
}

/// design D3：覆蓋的 runtime 已 `connected` 但 pane 不存在 → 投影回到自動解析，並把該覆蓋送到
/// 注入的 `mpsc::Sender`。
#[tokio::test(start_paused = true)]
async fn stale_override_sent_to_channel() {
    let mut domain = DomainState::from_projects(vec![scenario_c_project()]);
    let stale = Override {
        runtime: runtime_id("win"),
        pane_id: pane_id("wJ:gone"),
    };
    domain
        .overrides
        .entry(ProjectId::new("p"))
        .or_default()
        .insert(WorkstreamId::new("be"), stale.clone());

    let handle = StoreHandle::new_with_domain(RuntimeStore::new(), domain);
    let (tx, mut stale_rx) = mpsc::unbounded_channel();
    let projector = spawn_projector_with_stale_sink(handle.clone(), tx);

    // runtime 尚未登記（未連線）時覆蓋保留、不算失效；登記＋連線＋快照後才判定失效。
    let win = runtime_id("win");
    handle.register(win.clone(), "herdr".to_string(), "tcp://win".to_string());
    handle
        .set_connection(&win, connected_state())
        .expect("剛登記過");
    handle
        .replace(&win, scenario_c_snapshot(AgentStatus::Working))
        .expect("replace 應成功");

    let received = tokio::time::timeout(EXPECT_BROADCAST_TIMEOUT, stale_rx.recv())
        .await
        .expect("應在合理時間內收到 stale 清單")
        .expect("channel 應仍存活");
    assert_eq!(received.len(), 1);
    assert_eq!(received[0].project, ProjectId::new("p"));
    assert_eq!(received[0].workstream, WorkstreamId::new("be"));
    assert_eq!(received[0].override_, stale);

    let current = handle.current();
    assert_eq!(
        serde_json::to_value(&current.projects[0].workstreams[0].binding).expect("序列化"),
        json!({
            "state": "bound",
            "runtime": "win",
            "pane_id": "wJ:p1",
            "source": "auto",
            "agent": "claude",
            "agent_status": "working"
        }),
        "覆蓋失效後畫面應立即回到自動解析"
    );

    projector.abort();
}

/// 建一個 runtime `win` 已 connected、pane `wJ:p1` 存在的 handle，Domain 為 Scenario C 的
/// project；回傳「帶一筆指向不存在 pane 的覆蓋」的 domain 供測試寫入。
fn stale_setup() -> (StoreHandle, DomainState, DomainState, Override) {
    let plain = DomainState::from_projects(vec![scenario_c_project()]);
    let stale = Override {
        runtime: runtime_id("win"),
        pane_id: pane_id("wJ:gone"),
    };
    let mut with_override = plain.clone();
    with_override
        .overrides
        .entry(ProjectId::new("p"))
        .or_default()
        .insert(WorkstreamId::new("be"), stale.clone());
    let handle = StoreHandle::new_with_domain(scenario_c_store(AgentStatus::Idle), plain.clone());
    (handle, plain, with_override, stale)
}

/// 讓投影任務把目前的 dirty 通知跑完（含合併視窗）；暫停時鐘下不耗真實時間。
async fn settle() {
    tokio::time::sleep(Duration::from_millis(500)).await;
}

/// fix round 1：同一筆失效覆蓋在被刪除前歷經多次投影，只送一次。
#[tokio::test(start_paused = true)]
async fn same_stale_override_sent_once_across_projections() {
    let (handle, _plain, with_override, stale) = stale_setup();
    let (tx, mut stale_rx) = mpsc::unbounded_channel();
    let projector = spawn_projector_with_stale_sink(handle.clone(), tx);

    handle.set_domain(with_override.clone());
    settle().await;
    let first = stale_rx.try_recv().expect("第一次偵測應送出");
    assert_eq!(first.len(), 1);
    assert_eq!(first[0].override_, stale);

    // 再觸發多次投影（相同 domain、runtime 無實質變動），覆蓋仍失效。
    for _ in 0..3 {
        handle.set_domain(with_override.clone());
        handle
            .set_connection(&runtime_id("win"), connected_state())
            .expect("已登記");
        settle().await;
    }
    assert!(stale_rx.try_recv().is_err(), "同一筆失效覆蓋不應重複送出");

    projector.abort();
}

/// fix round 1：覆蓋被刪（不再出現在失效清單）後再次設定並失效，會再送一次。
#[tokio::test(start_paused = true)]
async fn stale_override_resent_after_removed_and_set_again() {
    let (handle, plain, with_override, stale) = stale_setup();
    let (tx, mut stale_rx) = mpsc::unbounded_channel();
    let projector = spawn_projector_with_stale_sink(handle.clone(), tx);

    handle.set_domain(with_override.clone());
    settle().await;
    assert_eq!(
        stale_rx.try_recv().expect("第一次應送出")[0].override_,
        stale
    );

    handle.set_domain(plain);
    settle().await;
    assert!(stale_rx.try_recv().is_err(), "覆蓋已刪，不應再送");

    handle.set_domain(with_override);
    settle().await;
    let again = stale_rx.try_recv().expect("再次設定且失效應再送");
    assert_eq!(again.len(), 1);
    assert_eq!(again[0].override_, stale);

    projector.abort();
}

/// fix round 1：接收端已關閉時，投影任務不 panic、照常投影與廣播。
#[tokio::test(start_paused = true)]
async fn closed_stale_receiver_does_not_stop_projection() {
    let (handle, _plain, with_override, _stale) = stale_setup();
    let (tx, stale_rx) = mpsc::unbounded_channel();
    drop(stale_rx);
    let mut rx = handle.subscribe();
    let projector = spawn_projector_with_stale_sink(handle.clone(), tx);

    // 覆蓋失效但 binding 仍自動解析為 wJ:p1，投影內容不變；接著推進 task 讓內容改變。
    handle.set_domain(with_override.clone());
    settle().await;
    let mut advanced = with_override;
    advanced
        .progress
        .get_mut(&ProjectId::new("p"))
        .expect("有 p")
        .insert(
            TaskId::new("A"),
            TaskProgress {
                stage: "Review".to_string(),
                mark: Mark::None,
            },
        );
    handle.set_domain(advanced);

    tokio::time::timeout(EXPECT_BROADCAST_TIMEOUT, rx.changed())
        .await
        .expect("接收端關閉後仍應廣播")
        .expect("channel 應仍存活");
    assert_eq!(rx.borrow().projects[0].tasks[0].stage, "Review");
    assert!(!projector.is_finished(), "投影任務不應結束（panic）");

    projector.abort();
}
