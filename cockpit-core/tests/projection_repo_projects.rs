//! repo-projects task 3.2（design D3、D4、D8）：投影輸出 Repo Project 需要的欄位——project 的 `kind`／`repo`、
//! workstream 的 `worktree`、固定 pane 工作線的名稱與排序、最上層 `detected_repos`、Repo Project 的目前 task。
//!
//! 一律比對序列化後的 JSON，確認欄位名稱與「沒有就省略」的形狀。

mod common;

use std::time::SystemTime;

use cockpit_core::{
    AgentStatus, ConnectionState, DomainState, Mark, PaneRepo, PaneRepos, ProjectDef, ProjectId,
    ProjectedState, RepoKey, RepoProjectDef, RuntimeStore, TaskDef, TaskId, TaskProgress,
    WorkstreamDef, WorkstreamId, project,
};
use serde_json::{Value, json};

use common::{empty_focused, pane, pane_id, runtime_id, snapshot, tab, workspace};

fn s(v: &str) -> String {
    v.to_string()
}

const APP: &str = r"d:\work\app\.git";
const LIB: &str = r"d:\work\lib\.git";
const LIB2: &str = r"d:\other\lib\.git";

fn connected() -> ConnectionState {
    ConnectionState::Connected {
        since: SystemTime::UNIX_EPOCH,
        server_version: s("0.9.0"),
        protocol: 1,
        last_snapshot_at: SystemTime::UNIX_EPOCH,
        settled: true,
        protocol_warning: None,
    }
}

fn repo_project(id: &str, name: &str, repo: &str) -> RepoProjectDef {
    RepoProjectDef {
        id: ProjectId::new(id),
        name: s(name),
        repo: RepoKey::new(repo),
        stages: vec![s("Plan"), s("Build")],
    }
}

fn pane_repo(repo: &str, name: &str, worktree: Option<&str>) -> PaneRepo {
    PaneRepo {
        repo: RepoKey::new(repo),
        default_name: s(name),
        worktree: worktree.map(s),
    }
}

fn pane_repos(entries: &[(&str, &str, PaneRepo)]) -> PaneRepos {
    entries
        .iter()
        .map(|(runtime, pane, repo)| ((runtime_id(runtime), pane_id(pane)), repo.clone()))
        .collect()
}

fn config_project(id: &str) -> ProjectDef {
    ProjectDef {
        id: ProjectId::new(id),
        name: s(id),
        stages: vec![s("A"), s("B")],
        workstreams: vec![WorkstreamDef {
            id: WorkstreamId::new("w"),
            name: s("w"),
            binding: None,
            pinned_pane: None,
        }],
        tasks: vec![TaskDef {
            id: TaskId::new("t1"),
            title: s("t1"),
            workstream: WorkstreamId::new("w"),
            stage: s("A"),
            depends_on: vec![],
        }],
        repo: None,
    }
}

/// 一個 pane 的描述：id、label、agent、agent 狀態。
struct PaneSpec {
    id: &'static str,
    label: Option<&'static str>,
    agent: Option<&'static str>,
    status: AgentStatus,
}

fn pane_spec(id: &'static str) -> PaneSpec {
    PaneSpec {
        id,
        label: None,
        agent: None,
        status: AgentStatus::Idle,
    }
}

/// 登記 runtime `id`（已 connected），workspace `wJ` 的 tab `wJ:t1` 依序放入 `panes`（`pane_seq` 即此順序）。
fn register(store: &mut RuntimeStore, id: &str, panes: &[PaneSpec]) {
    let runtime = runtime_id(id);
    store.register(runtime.clone(), s("herdr"), format!("tcp://{id}"));
    let built = panes
        .iter()
        .map(|spec| {
            let mut p = pane(spec.id, "wJ", "wJ:t1");
            p.label = spec.label.map(s);
            p.agent = spec.agent.map(s);
            p.agent_status = spec.status;
            p
        })
        .collect();
    store
        .replace(
            &runtime,
            snapshot(
                vec![workspace("wJ", 1)],
                vec![tab("wJ:t1", "wJ", 1)],
                built,
                vec![],
                empty_focused(),
            ),
        )
        .expect("replace 應成功");
    store
        .set_connection(&runtime, connected())
        .expect("剛登記過");
}

fn domain_with(
    config: Vec<ProjectDef>,
    repo_projects: Vec<RepoProjectDef>,
    pane_repos: PaneRepos,
) -> DomainState {
    let mut domain = DomainState::from_projects(config);
    domain.repo_projects = repo_projects;
    domain.pane_repos = pane_repos;
    domain.refresh_projects();
    domain
}

fn projected_json(store: &RuntimeStore, domain: &DomainState) -> Value {
    let projected = project(store, domain, 1, SystemTime::UNIX_EPOCH);
    serde_json::to_value(&projected).expect("可序列化")
}

fn find<'a>(items: &'a Value, key: &str, id: &str) -> &'a Value {
    items
        .as_array()
        .expect("應為陣列")
        .iter()
        .find(|item| item[key] == id)
        .unwrap_or_else(|| panic!("找不到 {key}={id}"))
}

fn ids(items: &Value) -> Vec<String> {
    items
        .as_array()
        .expect("應為陣列")
        .iter()
        .map(|item| item["id"].as_str().expect("id 為字串").to_string())
        .collect()
}

// ---------------------------------------------------------------------------
// kind／repo
// ---------------------------------------------------------------------------

/// spec「手寫 project 與 Repo Project 的 kind」：`h` 的 `kind` 為 `config` 且沒有 `repo` 欄位；
/// `app` 的 `kind` 為 `repo`、`repo` 為 repo key；順序是手寫在前。
#[test]
fn config_and_repo_projects_carry_kind_and_only_repo_projects_carry_repo() {
    let store = RuntimeStore::new();
    let domain = domain_with(
        vec![config_project("h")],
        vec![repo_project("app", "app", APP)],
        PaneRepos::new(),
    );
    let value = projected_json(&store, &domain);
    let projects = &value["projects"];

    assert_eq!(ids(projects), vec!["h", "app"]);
    let h = &projects[0];
    assert_eq!(h["kind"], "config");
    assert!(h.get("repo").is_none(), "手寫 project 沒有 repo 欄位：{h}");
    let app = &projects[1];
    assert_eq!(app["kind"], "repo");
    assert_eq!(app["repo"], APP);
}

/// 沒有 pane 的 Repo Project 仍在清單、仍帶 `kind` 與 `repo`，`workstreams`／`tasks` 為空陣列。
#[test]
fn repo_project_without_panes_is_still_projected_with_kind_and_repo() {
    let store = RuntimeStore::new();
    let domain = domain_with(
        vec![],
        vec![repo_project("app", "app", APP)],
        PaneRepos::new(),
    );
    let value = projected_json(&store, &domain);
    let app = &value["projects"][0];
    assert_eq!(app["kind"], "repo");
    assert_eq!(app["repo"], APP);
    assert_eq!(app["workstreams"], json!([]));
    assert_eq!(app["tasks"], json!([]));
}

/// id 撞名時生效的是手寫 project：`kind` 為 `config`、沒有 `repo`，`warnings` 帶撞名訊息。
#[test]
fn id_conflict_projects_the_config_project_as_kind_config() {
    let store = RuntimeStore::new();
    let domain = domain_with(
        vec![config_project("app")],
        vec![repo_project("app", "app", APP)],
        pane_repos(&[("local", "wJ:p1", pane_repo(APP, "app", None))]),
    );
    let value = projected_json(&store, &domain);
    assert_eq!(value["projects"].as_array().unwrap().len(), 1);
    let app = &value["projects"][0];
    assert_eq!(app["kind"], "config");
    assert!(app.get("repo").is_none());
    assert_eq!(
        app["warning_msgs"][0]["code"], "repo_project_id_conflict",
        "{app}"
    );
}

// ---------------------------------------------------------------------------
// 工作線與 task
// ---------------------------------------------------------------------------

/// spec「Repo Project 的工作線與 task」：pane label `backend`、agent `claude` working、位於 linked worktree。
#[test]
fn repo_project_workstream_and_task_come_from_the_pane() {
    let mut store = RuntimeStore::new();
    register(
        &mut store,
        "local",
        &[PaneSpec {
            id: "wJ:p1",
            label: Some("backend"),
            agent: Some("claude"),
            status: AgentStatus::Working,
        }],
    );
    let domain = domain_with(
        vec![],
        vec![repo_project("app", "app", APP)],
        pane_repos(&[("local", "wJ:p1", pane_repo(APP, "app", Some("app-wt")))]),
    );
    let value = projected_json(&store, &domain);
    let app = &value["projects"][0];

    assert_eq!(
        app["workstreams"][0],
        json!({
            "id": "local~wJ:p1",
            "name": "backend",
            "worktree": "app-wt",
            "binding": {
                "state": "bound", "runtime": "local", "pane_id": "wJ:p1", "source": "pane",
                "agent": "claude", "agent_status": "working"
            },
            "active_task": "local~wJ:p1",
            "activity_undeclared": false
        })
    );
    assert_eq!(
        app["tasks"][0],
        json!({
            "id": "local~wJ:p1",
            "title": "backend",
            "workstream": "local~wJ:p1",
            "stage": "Plan",
            "mark": "none",
            "status": "running",
            "depends_on": []
        })
    );
}

/// spec「位於主 worktree 的 pane 沒有 worktree 欄位」，與「手寫 project 的 workstream 沒有 worktree」。
#[test]
fn main_worktree_and_config_workstreams_have_no_worktree_key() {
    let mut store = RuntimeStore::new();
    register(
        &mut store,
        "local",
        &[pane_spec("wJ:p1"), pane_spec("wJ:p3")],
    );
    let domain = domain_with(
        vec![config_project("h")],
        vec![repo_project("app", "app", APP)],
        pane_repos(&[
            ("local", "wJ:p1", pane_repo(APP, "app", None)),
            ("local", "wJ:p3", pane_repo(APP, "app", Some("app-wt"))),
        ]),
    );
    let value = projected_json(&store, &domain);
    let workstreams = &value["projects"][1]["workstreams"];
    assert!(
        find(workstreams, "id", "local~wJ:p1")
            .get("worktree")
            .is_none()
    );
    assert_eq!(find(workstreams, "id", "local~wJ:p3")["worktree"], "app-wt");
    assert!(
        value["projects"][0]["workstreams"][0]
            .get("worktree")
            .is_none()
    );
}

/// spec「每個 pane 一條工作線」：名稱取 label，label 空取 agent 名稱，再空用 pane id；task 標題同名。
#[test]
fn workstream_name_falls_back_from_label_to_agent_to_pane_id() {
    let mut store = RuntimeStore::new();
    register(
        &mut store,
        "local",
        &[
            PaneSpec {
                id: "wJ:p1",
                label: Some("backend"),
                agent: Some("claude"),
                status: AgentStatus::Idle,
            },
            PaneSpec {
                id: "wJ:p2",
                label: None,
                agent: Some("claude"),
                status: AgentStatus::Idle,
            },
            PaneSpec {
                id: "wJ:p3",
                label: Some(""),
                agent: Some("codex"),
                status: AgentStatus::Idle,
            },
        ],
    );
    register(&mut store, "wsl", &[pane_spec("w1:p1")]);
    let domain = domain_with(
        vec![],
        vec![repo_project("app", "app", APP)],
        pane_repos(&[
            ("local", "wJ:p1", pane_repo(APP, "app", None)),
            ("local", "wJ:p2", pane_repo(APP, "app", None)),
            ("local", "wJ:p3", pane_repo(APP, "app", None)),
            ("wsl", "w1:p1", pane_repo(APP, "app", None)),
        ]),
    );
    let value = projected_json(&store, &domain);
    let app = &value["projects"][0];
    let expected = [
        ("local~wJ:p1", "backend"),
        ("local~wJ:p2", "claude"),
        ("local~wJ:p3", "codex"),
        ("wsl~w1:p1", "w1:p1"),
    ];
    for (id, name) in expected {
        assert_eq!(find(&app["workstreams"], "id", id)["name"], name, "{id}");
        assert_eq!(find(&app["tasks"], "id", id)["title"], name, "{id}");
    }
}

/// spec「名稱跟著 pane 的 label 變」：label 改了，id、進度不變，名稱與標題跟著變，Domain 沒有被改寫。
#[test]
fn name_follows_the_pane_label_without_touching_domain() {
    let mut store = RuntimeStore::new();
    let mut spec = pane_spec("wJ:p1");
    spec.label = Some("backend");
    register(&mut store, "local", &[spec]);
    let domain = domain_with(
        vec![],
        vec![repo_project("app", "app", APP)],
        pane_repos(&[("local", "wJ:p1", pane_repo(APP, "app", None))]),
    );
    let before = projected_json(&store, &domain);
    assert_eq!(before["projects"][0]["workstreams"][0]["name"], "backend");

    let mut spec = pane_spec("wJ:p1");
    spec.label = Some("api");
    register_again(&mut store, "local", &[spec]);
    let domain_before = domain.clone();
    let after = projected_json(&store, &domain);
    let app = &after["projects"][0];
    assert_eq!(app["workstreams"][0]["name"], "api");
    assert_eq!(app["workstreams"][0]["id"], "local~wJ:p1");
    assert_eq!(app["tasks"][0]["title"], "api");
    assert_eq!(app["tasks"][0]["id"], "local~wJ:p1");
    assert_eq!(domain, domain_before, "投影不改 Domain");
}

/// 同 runtime 重新送 snapshot（label 改變）。
fn register_again(store: &mut RuntimeStore, id: &str, panes: &[PaneSpec]) {
    let runtime = runtime_id(id);
    let built = panes
        .iter()
        .map(|spec| {
            let mut p = pane(spec.id, "wJ", "wJ:t1");
            p.label = spec.label.map(s);
            p.agent = spec.agent.map(s);
            p.agent_status = spec.status;
            p
        })
        .collect();
    store
        .replace(
            &runtime,
            snapshot(
                vec![workspace("wJ", 1)],
                vec![tab("wJ:t1", "wJ", 1)],
                built,
                vec![],
                empty_focused(),
            ),
        )
        .expect("replace 應成功");
}

/// pane 不在 pane 樹（歸類尚未更新的空窗）：名稱與標題用 pane id，binding 為 unbound。
#[test]
fn pane_missing_from_the_tree_uses_the_pane_id_as_name() {
    let mut store = RuntimeStore::new();
    register(&mut store, "local", &[pane_spec("wJ:p1")]);
    let domain = domain_with(
        vec![],
        vec![repo_project("app", "app", APP)],
        pane_repos(&[("local", "wJ:p9", pane_repo(APP, "app", None))]),
    );
    let value = projected_json(&store, &domain);
    let app = &value["projects"][0];
    assert_eq!(app["workstreams"][0]["name"], "wJ:p9");
    assert_eq!(
        app["workstreams"][0]["binding"],
        json!({"state": "unbound", "runtime": "local", "source": "pane"})
    );
    assert_eq!(app["tasks"][0]["title"], "wJ:p9");
}

/// spec「每個 pane 一條工作線」：先依 runtime 在設定中的順序（登記順序），再依 pane 在 snapshot 的順序。
/// 刻意讓兩者都和 id 字典序相反：runtime `zeta` 先登記，其中 `wJ:p9` 先進入狀態庫。
#[test]
fn workstreams_and_tasks_follow_runtime_registration_then_snapshot_order() {
    let mut store = RuntimeStore::new();
    register(
        &mut store,
        "zeta",
        &[pane_spec("wJ:p9"), pane_spec("wJ:p1")],
    );
    register(
        &mut store,
        "alpha",
        &[pane_spec("w1:p2"), pane_spec("w1:p1")],
    );
    let domain = domain_with(
        vec![],
        vec![repo_project("app", "app", APP)],
        pane_repos(&[
            ("alpha", "w1:p1", pane_repo(APP, "app", None)),
            ("alpha", "w1:p2", pane_repo(APP, "app", None)),
            ("zeta", "wJ:p1", pane_repo(APP, "app", None)),
            ("zeta", "wJ:p9", pane_repo(APP, "app", None)),
        ]),
    );
    let value = projected_json(&store, &domain);
    let app = &value["projects"][0];
    let expected = vec!["zeta~wJ:p9", "zeta~wJ:p1", "alpha~w1:p2", "alpha~w1:p1"];
    assert_eq!(ids(&app["workstreams"]), expected);
    assert_eq!(ids(&app["tasks"]), expected, "task 與 workstream 同序");
}

/// 手寫 project 的 workstream 與 task 順序、名稱不受影響（設定順序、設定中的名稱）。
#[test]
fn config_project_order_and_names_are_untouched() {
    let mut store = RuntimeStore::new();
    register(&mut store, "local", &[pane_spec("wJ:p1")]);
    let domain = domain_with(vec![config_project("h")], vec![], PaneRepos::new());
    let value = projected_json(&store, &domain);
    assert_eq!(value["projects"][0]["workstreams"][0]["name"], "w");
    assert_eq!(value["projects"][0]["tasks"][0]["title"], "t1");
    assert_eq!(
        value["projects"][0]["workstreams"][0]["active_task"],
        Value::Null
    );
}

// ---------------------------------------------------------------------------
// 目前 task 與進度
// ---------------------------------------------------------------------------

fn one_pane_repo_domain(progress: Option<TaskProgress>) -> (RuntimeStore, DomainState) {
    let mut store = RuntimeStore::new();
    register(
        &mut store,
        "local",
        &[PaneSpec {
            id: "wJ:p1",
            label: Some("backend"),
            agent: Some("claude"),
            status: AgentStatus::Working,
        }],
    );
    let mut domain = domain_with(
        vec![],
        vec![repo_project("app", "app", APP)],
        pane_repos(&[("local", "wJ:p1", pane_repo(APP, "app", None))]),
    );
    if let Some(progress) = progress {
        domain.repo_progress.insert(
            ProjectId::new("app"),
            [(TaskId::new("local~wJ:p1"), progress)].into(),
        );
    }
    (store, domain)
}

/// spec「Repo Project 的 task 未標記時就是目前 task」「agent 工作中即為 running」：標記為 none 時
/// `active_task` 是該 task，task 讀的是 `repo_progress`（stage 取自那裡）。
#[test]
fn unmarked_repo_task_is_the_active_task_and_progress_comes_from_repo_progress() {
    let (store, domain) = one_pane_repo_domain(Some(TaskProgress {
        stage: s("Build"),
        mark: Mark::None,
    }));
    let value = projected_json(&store, &domain);
    let app = &value["projects"][0];
    assert_eq!(app["workstreams"][0]["active_task"], "local~wJ:p1");
    assert_eq!(app["workstreams"][0]["activity_undeclared"], false);
    assert_eq!(app["tasks"][0]["stage"], "Build");
    assert_eq!(app["tasks"][0]["status"], "running");
}

/// spec「標 Completed 後沒有目前 task」：completed／failed 時 `active_task` 為 null；清除標記後回來。
#[test]
fn marked_repo_task_has_no_active_task_until_the_mark_is_cleared() {
    for (mark, status) in [(Mark::Completed, "completed"), (Mark::Failed, "failed")] {
        let (store, domain) = one_pane_repo_domain(Some(TaskProgress {
            stage: s("Plan"),
            mark,
        }));
        let value = projected_json(&store, &domain);
        let app = &value["projects"][0];
        assert_eq!(
            app["workstreams"][0]["active_task"],
            Value::Null,
            "{mark:?}"
        );
        assert_eq!(app["tasks"][0]["mark"], status, "{mark:?}");
        assert_eq!(app["tasks"][0]["status"], status, "{mark:?}");
    }

    let (store, domain) = one_pane_repo_domain(Some(TaskProgress {
        stage: s("Plan"),
        mark: Mark::None,
    }));
    let value = projected_json(&store, &domain);
    assert_eq!(
        value["projects"][0]["workstreams"][0]["active_task"], "local~wJ:p1",
        "清除標記後又成為目前 task"
    );
}

/// 沒有 `repo_progress` 紀錄的新 pane：task 在第一個 stage、標記 none，也是目前 task（spec「新 pane 出現」）。
#[test]
fn new_repo_task_without_progress_starts_at_first_stage_and_is_active() {
    let (store, domain) = one_pane_repo_domain(None);
    let value = projected_json(&store, &domain);
    let app = &value["projects"][0];
    assert_eq!(app["tasks"][0]["stage"], "Plan");
    assert_eq!(app["tasks"][0]["mark"], "none");
    assert_eq!(app["workstreams"][0]["active_task"], "local~wJ:p1");
}

/// 同 id 的手寫 project 與被隱藏的 Repo Project：手寫 project 讀 `progress`，不被 `repo_progress` 污染。
#[test]
fn hidden_repo_project_progress_does_not_leak_into_the_config_project() {
    let store = RuntimeStore::new();
    let mut domain = domain_with(
        vec![config_project("app")],
        vec![repo_project("app", "app", APP)],
        PaneRepos::new(),
    );
    domain.repo_progress.insert(
        ProjectId::new("app"),
        [(
            TaskId::new("t1"),
            TaskProgress {
                stage: s("B"),
                mark: Mark::Completed,
            },
        )]
        .into(),
    );
    let value = projected_json(&store, &domain);
    assert_eq!(value["projects"][0]["tasks"][0]["stage"], "A");
    assert_eq!(value["projects"][0]["tasks"][0]["mark"], "none");
}

// ---------------------------------------------------------------------------
// detected_repos
// ---------------------------------------------------------------------------

/// spec「列出尚未加入的 repo」＋「Project 與偵測到的 repo 的排序」：名稱不分大小寫排序（`app` 在 `Lib` 前），
/// 同名依 repo，`pane_count` 為歸入該 repo 的 pane 數（跨 runtime、含 linked worktree）。
#[test]
fn detected_repos_are_sorted_by_name_case_insensitively_then_repo_with_pane_counts() {
    let store = RuntimeStore::new();
    let domain = domain_with(
        vec![],
        vec![],
        pane_repos(&[
            ("local", "wJ:p1", pane_repo(APP, "app", None)),
            ("wsl", "w1:p1", pane_repo(APP, "app", Some("app-wt"))),
            ("local", "wJ:p2", pane_repo(LIB, "Lib", None)),
            ("local", "wJ:p3", pane_repo(LIB2, "Lib", None)),
        ]),
    );
    let value = projected_json(&store, &domain);
    assert_eq!(
        value["detected_repos"],
        json!([
            {"repo": APP, "name": "app", "pane_count": 2},
            {"repo": LIB2, "name": "Lib", "pane_count": 1},
            {"repo": LIB, "name": "Lib", "pane_count": 1},
        ])
    );
    assert_eq!(value["projects"], json!([]));
}

/// spec「沒有 Project」：沒有任何歸類 pane 時 `detected_repos` 為空陣列（不省略）。
#[test]
fn detected_repos_is_an_empty_array_when_nothing_is_detected() {
    let store = RuntimeStore::new();
    let domain = DomainState::default();
    let value = projected_json(&store, &domain);
    assert_eq!(value["detected_repos"], json!([]));
    assert_eq!(value["projects"], json!([]));
}

/// spec「加入後消失、移除後回來」：已被任何 Repo Project 加入的 repo 不在清單；移除後回來。
#[test]
fn added_repos_leave_detected_repos_and_return_after_removal() {
    let store = RuntimeStore::new();
    let repos = pane_repos(&[
        ("local", "wJ:p1", pane_repo(APP, "app", None)),
        ("local", "wJ:p2", pane_repo(LIB, "lib", None)),
    ]);
    let mut domain = domain_with(vec![], vec![repo_project("app", "app", APP)], repos);
    let names = |domain: &DomainState| -> Vec<String> {
        projected_json(&store, domain)["detected_repos"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["name"].as_str().unwrap().to_string())
            .collect()
    };
    assert_eq!(names(&domain), vec!["lib"]);

    domain.repo_projects.clear();
    domain.refresh_projects();
    assert_eq!(names(&domain), vec!["app", "lib"]);
}

/// spec「被隱藏的 Repo Project 仍可管理」：因 id 撞名而沒展開的 Repo Project，其 repo 也算已加入。
#[test]
fn repo_hidden_by_id_conflict_is_not_detected() {
    let store = RuntimeStore::new();
    let domain = domain_with(
        vec![config_project("app")],
        vec![repo_project("app", "app", APP)],
        pane_repos(&[("local", "wJ:p1", pane_repo(APP, "app", None))]),
    );
    let value = projected_json(&store, &domain);
    assert_eq!(value["detected_repos"], json!([]));
}

/// spec「手寫 project 不影響清單」：沒有 Repo Project 時，手寫 project 存在也不減少 `detected_repos`。
#[test]
fn config_projects_do_not_affect_detected_repos() {
    let store = RuntimeStore::new();
    let domain = domain_with(
        vec![config_project("h")],
        vec![],
        pane_repos(&[("local", "wJ:p1", pane_repo(APP, "app", None))]),
    );
    let value = projected_json(&store, &domain);
    assert_eq!(
        value["detected_repos"],
        json!([{"repo": APP, "name": "app", "pane_count": 1}])
    );
}

/// 同一 repo 的 pane 預設名稱不同時（規格沒寫）：取 `(runtime id, pane id)` 排序最前的 pane 的名稱，結果固定。
#[test]
fn detected_repo_name_comes_from_the_first_pane_when_names_differ() {
    let store = RuntimeStore::new();
    let domain = domain_with(
        vec![],
        vec![],
        pane_repos(&[
            ("wsl", "w1:p1", pane_repo(APP, "zzz", None)),
            ("local", "wJ:p2", pane_repo(APP, "second", None)),
            ("local", "wJ:p1", pane_repo(APP, "first", None)),
        ]),
    );
    let value = projected_json(&store, &domain);
    assert_eq!(
        value["detected_repos"],
        json!([{"repo": APP, "name": "first", "pane_count": 3}])
    );
}

// ---------------------------------------------------------------------------
// content_eq 與 JSON 往返
// ---------------------------------------------------------------------------

/// `detected_repos` 改變要算內容改變（1.6 據此遞增 version 與廣播）；相同的輸入重複投影內容相等。
#[test]
fn detected_repos_participate_in_content_equality() {
    let store = RuntimeStore::new();
    let one = domain_with(
        vec![],
        vec![],
        pane_repos(&[("local", "wJ:p1", pane_repo(APP, "app", None))]),
    );
    let two = domain_with(
        vec![],
        vec![],
        pane_repos(&[
            ("local", "wJ:p1", pane_repo(APP, "app", None)),
            ("local", "wJ:p2", pane_repo(APP, "app", None)),
        ]),
    );
    let at = |domain: &DomainState, version: u64, secs: u64| {
        project(
            &store,
            domain,
            version,
            SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(secs),
        )
    };
    assert!(
        at(&one, 1, 0).content_eq(&at(&one, 2, 99)),
        "只差 version 與時間"
    );
    assert!(
        !at(&one, 1, 0).content_eq(&at(&two, 1, 0)),
        "pane_count 不同"
    );
}

/// 舊 JSON（沒有 `kind`／`repo`／`worktree`／`detected_repos`）仍可反序列化，預設為手寫 project、無 worktree、空清單。
#[test]
fn old_json_without_the_new_fields_still_deserializes() {
    let old = json!({
        "version": 1,
        "generated_at": "1970-01-01T00:00:00Z",
        "runtimes": [],
        "projects": [{
            "id": "p", "name": "p", "stages": ["A"], "warnings": [],
            "workstreams": [{
                "id": "w", "name": "w", "binding": {"state": "none"},
                "active_task": null, "activity_undeclared": false
            }],
            "tasks": []
        }],
        "recent_events": []
    });
    let state: ProjectedState = serde_json::from_value(old).expect("舊形狀應可讀");
    assert!(state.detected_repos.is_empty());
    let back = serde_json::to_value(&state).unwrap();
    assert_eq!(back["projects"][0]["kind"], "config");
    assert!(back["projects"][0].get("repo").is_none());
    assert!(
        back["projects"][0]["workstreams"][0]
            .get("worktree")
            .is_none()
    );
}
