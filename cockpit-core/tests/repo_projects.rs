//! repo-projects task 3.1（design D3）：Repo Project 展開成一般 `ProjectDef` 的純函數、
//! `<runtime id>~<pane id>` 的 id 規則、worktree 標註、固定 pane 綁定（`source: pane` 的三種狀態）、
//! 手寫 project 與 Repo Project 並列及 id 撞名。
//!
//! 名稱與排序在投影時取（task 3.2），這裡只驗展開結果的 id、綁定與 worktree 標註。

mod common;

use std::time::{Duration, SystemTime};

use cockpit_core::domain::binding::resolve_binding;
use cockpit_core::{
    AgentStatus, BindingResolution, BindingSource, ConnectionState, DomainState, Mark, Message,
    Override, PaneRepo, PaneRepos, PinnedPane, ProjectDef, ProjectId, RepoKey, RepoProjectDef,
    RuntimeStore, TaskDef, TaskId, TaskProgress, WorkstreamDef, WorkstreamId, expand_repo_projects,
    is_valid_repo_project_id, normalize_repo_project_name, normalize_repo_project_stages,
    pane_item_id, project, split_pane_item_id,
};
use serde_json::json;

use common::{empty_focused, pane, pane_id, runtime_id, snapshot, tab, workspace};

fn s(v: &str) -> String {
    v.to_string()
}

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

fn disconnected() -> ConnectionState {
    ConnectionState::Disconnected {
        reason: s("事件流結束"),
        retry_in: Duration::from_secs(5),
    }
}

fn repo_project(id: &str, name: &str, repo: &str) -> RepoProjectDef {
    RepoProjectDef {
        id: ProjectId::new(id),
        name: s(name),
        repo: RepoKey::new(repo),
        stages: vec![s("Plan"), s("Implement"), s("Review")],
        phases: vec![None; 3],
    }
}

fn pane_repo(repo: &str, worktree: Option<&str>) -> PaneRepo {
    PaneRepo {
        repo: RepoKey::new(repo),
        default_name: s("app"),
        worktree: worktree.map(s),
        root: None,
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

const APP: &str = r"d:\work\app\.git";
const OTHER: &str = r"d:\work\other\.git";

fn ids(project: &ProjectDef) -> (Vec<&str>, Vec<&str>) {
    (
        project.workstreams.iter().map(|w| w.id.as_str()).collect(),
        project.tasks.iter().map(|t| t.id.as_str()).collect(),
    )
}

// ---------------------------------------------------------------------------
// 由 pane 推導 workstream 與 task
// ---------------------------------------------------------------------------

/// spec「每個 pane 一條工作線」：歸入該 repo 的每個 pane 各一條 workstream 與一張 id 相同的 task；
/// 其他 repo 的 pane 不歸入。
#[test]
fn each_pane_in_the_repo_becomes_one_workstream_and_one_task() {
    let repos = pane_repos(&[
        ("local", "wJ:p1", pane_repo(APP, None)),
        ("local", "wJ:p2", pane_repo(APP, None)),
        ("local", "wJ:p9", pane_repo(OTHER, None)),
        ("wsl", "w1:p1", pane_repo(APP, None)),
    ]);
    let expansion = expand_repo_projects(&[], &[repo_project("app", "app", APP)], &repos);

    assert_eq!(expansion.projects.len(), 1);
    let app = &expansion.projects[0];
    assert_eq!(app.id.as_str(), "app");
    assert_eq!(app.name, "app");
    assert_eq!(app.stages, vec![s("Plan"), s("Implement"), s("Review")]);
    let (workstreams, tasks) = ids(app);
    let mut sorted = workstreams.clone();
    sorted.sort_unstable();
    assert_eq!(sorted, vec!["local~wJ:p1", "local~wJ:p2", "wsl~w1:p1"]);
    assert_eq!(
        tasks, workstreams,
        "task 與 workstream 一對一、id 相同、同序"
    );

    for (ws, task) in app.workstreams.iter().zip(&app.tasks) {
        assert_eq!(task.workstream, ws.id);
        assert_eq!(task.stage, "Plan", "新 task 在第一個 stage");
        assert!(task.depends_on.is_empty());
        assert!(
            ws.binding.is_none(),
            "固定 pane 不經 runtime-binding 的自動解析"
        );
        let pinned = ws
            .pinned_pane
            .as_ref()
            .expect("Repo Project 的工作線固定綁定 pane");
        assert_eq!(
            pane_item_id(&pinned.runtime, &pinned.pane_id),
            ws.id.as_str()
        );
    }
    assert!(expansion.warnings.is_empty());
}

/// spec「runtime id 含 ~」：id 為 `dev~1~wJ:p1`，從最後一個 `~` 拆回 runtime `dev~1` 與 pane `wJ:p1`。
#[test]
fn runtime_id_containing_tilde_splits_at_the_last_tilde() {
    let repos = pane_repos(&[("dev~1", "wJ:p1", pane_repo(APP, None))]);
    let expansion = expand_repo_projects(&[], &[repo_project("app", "app", APP)], &repos);
    let ws = &expansion.projects[0].workstreams[0];
    assert_eq!(ws.id.as_str(), "dev~1~wJ:p1");
    assert_eq!(expansion.projects[0].tasks[0].id.as_str(), "dev~1~wJ:p1");

    assert_eq!(
        split_pane_item_id("dev~1~wJ:p1"),
        Some((runtime_id("dev~1"), pane_id("wJ:p1")))
    );
    assert_eq!(
        split_pane_item_id("local~wJ:p1"),
        Some((runtime_id("local"), pane_id("wJ:p1")))
    );
    // 拆不出非空的 runtime 與 pane 的不是固定 pane 的 id。
    for bad in ["wJ:p1", "~wJ:p1", "local~", ""] {
        assert_eq!(split_pane_item_id(bad), None, "{bad}");
    }
}

/// spec「linked worktree 標註」：worktree 標註帶在展開結果上，主 worktree 沒有標註。
#[test]
fn linked_worktree_annotation_is_carried_on_the_expanded_workstream() {
    let repos = pane_repos(&[
        ("local", "wJ:p1", pane_repo(APP, None)),
        ("local", "wJ:p3", pane_repo(APP, Some("app-wt"))),
    ]);
    let expansion = expand_repo_projects(&[], &[repo_project("app", "app", APP)], &repos);
    let worktree_of = |id: &str| {
        expansion.projects[0]
            .workstreams
            .iter()
            .find(|w| w.id.as_str() == id)
            .and_then(|w| w.pinned_pane.as_ref())
            .expect("工作線存在且固定綁定")
            .worktree
            .clone()
    };
    assert_eq!(worktree_of("local~wJ:p3"), Some(s("app-wt")));
    assert_eq!(worktree_of("local~wJ:p1"), None);
}

/// spec「沒有 pane 的 Repo Project」：仍在清單中，workstreams 與 tasks 為空。
#[test]
fn repo_project_without_panes_still_appears_with_empty_workstreams_and_tasks() {
    let repos = pane_repos(&[("local", "wJ:p9", pane_repo(OTHER, None))]);
    let expansion = expand_repo_projects(&[], &[repo_project("app", "app", APP)], &repos);
    assert_eq!(expansion.projects.len(), 1);
    assert!(expansion.projects[0].workstreams.is_empty());
    assert!(expansion.projects[0].tasks.is_empty());
}

// ---------------------------------------------------------------------------
// 並列與撞名
// ---------------------------------------------------------------------------

/// spec「並列順序」：手寫依設定檔順序在前；Repo Project 依名稱不分大小寫排序、同名再依 id。
#[test]
fn config_projects_come_first_then_repo_projects_by_case_insensitive_name_then_id() {
    let expansion = expand_repo_projects(
        &[config_project("h2"), config_project("h1")],
        &[
            repo_project("beta", "beta", r"d:\b\.git"),
            repo_project("alpha", "Alpha", r"d:\a\.git"),
            repo_project("same-2", "same", r"d:\s2\.git"),
            repo_project("same", "Same", r"d:\s1\.git"),
        ],
        &PaneRepos::new(),
    );
    let order: Vec<&str> = expansion.projects.iter().map(|p| p.id.as_str()).collect();
    assert_eq!(order, vec!["h2", "h1", "alpha", "beta", "same", "same-2"]);
    // 手寫 project 原樣通過。
    assert_eq!(expansion.projects[0], config_project("h2"));
}

/// spec「id 撞名」：手寫優先，Repo Project 不展開；手寫 project 得到一則
/// `repo_project_id_conflict`（參數 `id`）警告。
#[test]
fn id_conflict_hides_the_repo_project_and_warns_on_the_config_project() {
    let repos = pane_repos(&[("local", "wJ:p1", pane_repo(APP, None))]);
    let expansion = expand_repo_projects(
        &[config_project("app")],
        &[repo_project("app", "app", APP)],
        &repos,
    );
    assert_eq!(expansion.projects, vec![config_project("app")]);

    let warnings = expansion
        .warnings
        .get(&ProjectId::new("app"))
        .expect("手寫 app 要有撞名警告");
    assert_eq!(warnings.len(), 1);
    let message = Message::classify(&warnings[0]);
    assert_eq!(message, Message::RepoProjectIdConflict { id: s("app") });
    let code = message.msg();
    assert_eq!(code.code, "repo_project_id_conflict");
    assert_eq!(code.params.get("id").map(String::as_str), Some("app"));
    assert!(warnings[0].contains("app"), "訊息內容含該 id");
    assert_eq!(expansion.warnings.len(), 1);
}

/// `refresh_projects`：重算實際生效的清單；撞名警告每次重算，載入時的其他警告保留；
/// 不碰 `progress`／`repo_progress`。撞名解除後警告消失（spec「被隱藏的 Repo Project 仍可管理」）。
#[test]
fn refresh_projects_recomputes_conflict_warnings_and_keeps_other_state() {
    let load_warning = Message::TaskStageReset {
        task: s("t1"),
        stage: s("Old"),
        start: s("A"),
    }
    .text();
    let mut domain = DomainState::from_projects(vec![config_project("app")]);
    domain
        .warnings
        .insert(ProjectId::new("app"), vec![load_warning.clone()]);
    domain.repo_projects = vec![repo_project("app", "app", APP)];
    domain.pane_repos = pane_repos(&[("local", "wJ:p1", pane_repo(APP, None))]);
    let repo_task = TaskProgress {
        stage: s("Review"),
        mark: Mark::None,
    };
    domain.repo_progress.insert(
        ProjectId::new("app"),
        [(TaskId::new("local~wJ:p1"), repo_task.clone())].into(),
    );
    let progress_before = domain.progress.clone();

    domain.refresh_projects();
    domain.refresh_projects();
    let conflict = Message::RepoProjectIdConflict { id: s("app") }.text();
    assert_eq!(
        domain.warnings.get(&ProjectId::new("app")),
        Some(&vec![load_warning.clone(), conflict]),
        "重算不重複累積撞名警告"
    );
    assert_eq!(domain.projects, vec![config_project("app")]);
    assert_eq!(domain.progress, progress_before);
    assert_eq!(
        domain.repo_progress[&ProjectId::new("app")][&TaskId::new("local~wJ:p1")],
        repo_task
    );

    // 移除被隱藏的 Repo Project 後重算：撞名警告消失，載入警告保留。
    domain.repo_projects.clear();
    domain.refresh_projects();
    assert_eq!(
        domain.warnings.get(&ProjectId::new("app")),
        Some(&vec![load_warning])
    );

    // 只有撞名警告的 project：解除後連同外層項目移除。
    let mut domain = DomainState::from_projects(vec![config_project("x")]);
    domain.repo_projects = vec![repo_project("x", "x", APP)];
    domain.refresh_projects();
    assert!(domain.warnings.contains_key(&ProjectId::new("x")));
    domain.repo_projects.clear();
    domain.refresh_projects();
    assert!(!domain.warnings.contains_key(&ProjectId::new("x")));
}

/// `refresh_projects` 依 `repo_projects` 與 `pane_repos` 展開，Repo Project 排在手寫之後。
#[test]
fn refresh_projects_expands_repo_projects_after_config_projects() {
    let mut domain = DomainState::from_projects(vec![config_project("h")]);
    domain.repo_projects = vec![repo_project("app", "app", APP)];
    domain.pane_repos = pane_repos(&[("local", "wJ:p1", pane_repo(APP, None))]);
    domain.refresh_projects();
    let order: Vec<&str> = domain.projects.iter().map(|p| p.id.as_str()).collect();
    assert_eq!(order, vec!["h", "app"]);
    assert_eq!(ids(&domain.projects[1]).0, vec!["local~wJ:p1"]);
    assert!(domain.warnings.is_empty());
}

/// 種類標記（review fix round 1）：展開的 Repo Project 帶 `repo`，手寫 project 為 `None`；
/// 沒有 pane 的 Repo Project 也帶 `repo`（不能靠 `pinned_pane` 判斷種類）。
#[test]
fn expanded_repo_projects_carry_their_repo_key_and_config_projects_do_not() {
    let expansion = expand_repo_projects(
        &[config_project("h")],
        &[
            repo_project("app", "app", APP),
            repo_project("empty", "empty", OTHER),
        ],
        &pane_repos(&[("local", "wJ:p1", pane_repo(APP, None))]),
    );
    let repo_of = |id: &str| {
        expansion
            .projects
            .iter()
            .find(|p| p.id.as_str() == id)
            .unwrap_or_else(|| panic!("找不到 {id}"))
            .repo
            .clone()
    };
    assert_eq!(repo_of("h"), None);
    assert_eq!(repo_of("app"), Some(RepoKey::new(APP)));
    assert_eq!(repo_of("empty"), Some(RepoKey::new(OTHER)));
}

/// 撞名時生效清單中的 `app` 是手寫的（`repo` 為 `None`），即使 `repo_projects` 仍有 `app`。
#[test]
fn on_id_conflict_the_effective_project_is_the_config_one() {
    let mut domain = DomainState::from_projects(vec![config_project("app")]);
    domain.repo_projects = vec![repo_project("app", "app", APP)];
    domain.pane_repos = pane_repos(&[("local", "wJ:p1", pane_repo(APP, None))]);
    domain.refresh_projects();
    assert_eq!(domain.projects.len(), 1);
    assert_eq!(domain.projects[0].id.as_str(), "app");
    assert_eq!(domain.projects[0].repo, None);
    assert_eq!(domain.projects[0], config_project("app"));
}

/// 重複呼叫 `refresh_projects` 不會讓上一輪展開的 Repo Project 被當成手寫 project 而自己撞名；
/// 把生效清單直接傳給 `expand_repo_projects` 也一樣。
#[test]
fn repeated_refresh_does_not_conflict_with_its_own_expansion() {
    let mut domain = DomainState::from_projects(vec![config_project("h")]);
    domain.repo_projects = vec![repo_project("app", "app", APP)];
    domain.pane_repos = pane_repos(&[("local", "wJ:p1", pane_repo(APP, None))]);
    domain.refresh_projects();
    let first = domain.projects.clone();
    domain.refresh_projects();
    domain.refresh_projects();
    assert_eq!(domain.projects, first);
    let order: Vec<&str> = domain.projects.iter().map(|p| p.id.as_str()).collect();
    assert_eq!(order, vec!["h", "app"]);
    assert!(
        domain.warnings.is_empty(),
        "不得自撞：{:?}",
        domain.warnings
    );

    let again = expand_repo_projects(&domain.projects, &domain.repo_projects, &domain.pane_repos);
    assert_eq!(again.projects, first);
    assert!(again.warnings.is_empty());

    // pane 離開後重算：Repo Project 仍在、沒有工作線。
    domain.pane_repos.clear();
    domain.refresh_projects();
    assert_eq!(domain.projects.len(), 2);
    assert!(domain.projects[1].workstreams.is_empty());
}

// ---------------------------------------------------------------------------
// 固定 pane 綁定
// ---------------------------------------------------------------------------

fn pinned_workstream(runtime: &str, pane: &str) -> WorkstreamDef {
    WorkstreamDef {
        id: WorkstreamId::new(pane_item_id(&runtime_id(runtime), &pane_id(pane))),
        name: s(pane),
        binding: None,
        pinned_pane: Some(PinnedPane {
            runtime: runtime_id(runtime),
            pane_id: pane_id(pane),
            worktree: None,
        }),
    }
}

/// runtime `local`（狀態 `connection`）的 workspace `wJ` 有 `wJ:p1`（agent `claude` working，
/// `exited` 依參數）與 `wJ:p2`。
fn store_with(connection: ConnectionState, p1_exited: bool) -> RuntimeStore {
    let mut p1 = pane("wJ:p1", "wJ", "wJ:t1");
    p1.agent = Some(s("claude"));
    p1.agent_status = AgentStatus::Working;
    p1.exited = p1_exited;
    let p2 = pane("wJ:p2", "wJ", "wJ:t1");
    let mut store = RuntimeStore::new();
    let local = runtime_id("local");
    store.register(local.clone(), s("herdr"), s("tcp://local"));
    store
        .replace(
            &local,
            snapshot(
                vec![workspace("wJ", 1)],
                vec![tab("wJ:t1", "wJ", 1)],
                vec![p1, p2],
                vec![],
                empty_focused(),
            ),
        )
        .expect("replace 應成功");
    store.set_connection(&local, connection).expect("剛登記過");
    store
}

/// spec runtime-binding「固定 pane 的解析結果」：bound／unbound／runtime_disconnected，source 都是 pane。
#[test]
fn pinned_pane_resolves_to_bound_unbound_or_runtime_disconnected_with_source_pane() {
    let ws = pinned_workstream("local", "wJ:p1");

    let (bound, stale) = resolve_binding(&ws, None, &store_with(connected(), false));
    assert!(!stale);
    assert_eq!(
        bound,
        BindingResolution::Bound {
            runtime: runtime_id("local"),
            pane_id: pane_id("wJ:p1"),
            source: BindingSource::Pane,
        }
    );

    let unbound = BindingResolution::Unbound {
        runtime: runtime_id("local"),
        source: BindingSource::Pane,
    };
    // pane 已 exited。
    let (exited, _) = resolve_binding(&ws, None, &store_with(connected(), true));
    assert_eq!(exited, unbound);
    // pane 不在 pane 樹。
    let gone = pinned_workstream("local", "wJ:p7");
    let (missing, _) = resolve_binding(&gone, None, &store_with(connected(), false));
    assert_eq!(
        missing,
        BindingResolution::Unbound {
            runtime: runtime_id("local"),
            source: BindingSource::Pane,
        }
    );

    let (down, _) = resolve_binding(&ws, None, &store_with(disconnected(), false));
    assert_eq!(
        down,
        BindingResolution::RuntimeDisconnected {
            runtime: runtime_id("local"),
            source: BindingSource::Pane,
        }
    );
    // 未登記的 runtime 同樣視為未連線。
    let (unknown, _) = resolve_binding(
        &pinned_workstream("ghost", "wJ:p1"),
        None,
        &store_with(connected(), false),
    );
    assert_eq!(
        unknown,
        BindingResolution::RuntimeDisconnected {
            runtime: runtime_id("ghost"),
            source: BindingSource::Pane,
        }
    );
}

/// `BindingSource::Pane` 序列化為 `pane`（spec state-projection「Project 投影」）。
#[test]
fn binding_source_pane_serializes_as_pane() {
    assert_eq!(
        serde_json::to_value(BindingSource::Pane).unwrap(),
        json!("pane")
    );
    let back: BindingSource = serde_json::from_value(json!("pane")).unwrap();
    assert_eq!(back, BindingSource::Pane);
}

/// 固定 pane 不經覆蓋：即使 Domain 裡殘留一筆覆蓋，也照固定 pane 解析，且不回報失效覆蓋。
#[test]
fn pinned_pane_ignores_overrides_and_never_reports_them_stale() {
    let ws = pinned_workstream("local", "wJ:p1");
    let store = store_with(connected(), false);
    for target in ["wJ:p2", "wJ:p404"] {
        let over = Override {
            runtime: runtime_id("local"),
            pane_id: pane_id(target),
        };
        let (resolution, stale) = resolve_binding(&ws, Some(&over), &store);
        assert!(!stale, "{target}");
        assert_eq!(
            resolution,
            BindingResolution::Bound {
                runtime: runtime_id("local"),
                pane_id: pane_id("wJ:p1"),
                source: BindingSource::Pane,
            },
            "{target}"
        );
    }
}

/// spec「已綁定」「pane 已 exited 的空窗」「runtime 斷線」的 JSON；自動解析的 `unbound` 形狀不變（沒有 `source`）。
#[test]
fn pinned_bindings_serialize_with_source_pane_in_the_projection() {
    let mut domain = DomainState {
        repo_projects: vec![repo_project("app", "app", APP)],
        pane_repos: pane_repos(&[
            ("local", "wJ:p1", pane_repo(APP, None)),
            ("local", "wJ:p7", pane_repo(APP, None)),
            ("wsl", "w1:p1", pane_repo(APP, None)),
        ]),
        ..DomainState::default()
    };
    domain.refresh_projects();

    let mut store = store_with(connected(), false);
    let wsl = runtime_id("wsl");
    store.register(wsl.clone(), s("herdr"), s("tcp://wsl"));
    store
        .set_connection(&wsl, disconnected())
        .expect("剛登記過");

    let projected = project(&store, &domain, 1, SystemTime::UNIX_EPOCH);
    let value = serde_json::to_value(&projected.projects[0]).expect("可序列化");
    let binding_of = |id: &str| {
        value["workstreams"]
            .as_array()
            .unwrap()
            .iter()
            .find(|w| w["id"] == id)
            .unwrap_or_else(|| panic!("找不到 workstream {id}"))["binding"]
            .clone()
    };
    assert_eq!(
        binding_of("local~wJ:p1"),
        json!({"state":"bound","runtime":"local","pane_id":"wJ:p1","source":"pane","agent":"claude","agent_status":"working"})
    );
    assert_eq!(
        binding_of("local~wJ:p7"),
        json!({"state":"unbound","runtime":"local","source":"pane"})
    );
    assert_eq!(
        binding_of("wsl~w1:p1"),
        json!({"state":"runtime_disconnected","runtime":"wsl","source":"pane"})
    );

    // spec「新 pane 出現」：task 在第一個 stage、標記 none。
    for task in value["tasks"].as_array().unwrap() {
        assert_eq!(task["stage"], "Plan");
        assert_eq!(task["mark"], "none");
    }

    // 自動解析的 unbound 仍只有 state 與 runtime。
    let mut config = DomainState::from_projects(vec![ProjectDef {
        workstreams: vec![WorkstreamDef {
            id: WorkstreamId::new("be"),
            name: s("be"),
            binding: Some(cockpit_core::BindingSpec {
                runtime: runtime_id("local"),
                workspace: s("no-such-workspace"),
                pane_label: None,
                cwd: None,
                agent: None,
            }),
            pinned_pane: None,
        }],
        tasks: vec![],
        ..config_project("h")
    }]);
    config.warnings.clear();
    let projected = project(&store, &config, 1, SystemTime::UNIX_EPOCH);
    assert_eq!(
        serde_json::to_value(&projected.projects[0].workstreams[0].binding).unwrap(),
        json!({"state":"unbound","runtime":"local"})
    );
}

// ---------------------------------------------------------------------------
// repo-projects task 4.1：進度表分流與 Repo Project 定義的驗證規則
// ---------------------------------------------------------------------------

fn progress(stage: &str, mark: Mark) -> TaskProgress {
    TaskProgress {
        stage: s(stage),
        mark,
    }
}

/// 種類分流只看生效 def 的 `repo`（design D3）：id 撞名時手寫 `app` 讀寫 `progress`、Repo Project `app`
/// 讀寫 `repo_progress`，兩邊互不污染。
#[test]
fn progress_for_picks_the_table_by_the_effective_def_kind_even_on_id_conflict() {
    let mut domain = DomainState::from_projects(vec![config_project("app")]);
    domain.repo_projects = vec![repo_project("app", "app", APP)];
    domain.refresh_projects();
    let config_def = domain.projects[0].clone();
    let repo_def = ProjectDef {
        repo: Some(RepoKey::new(APP)),
        ..config_project("app")
    };
    domain
        .repo_progress
        .entry(ProjectId::new("app"))
        .or_default()
        .insert(TaskId::new("local~wJ:p1"), progress("Review", Mark::None));

    let config_table = domain.progress_for(&config_def).expect("手寫 app 有進度表");
    assert!(config_table.contains_key(&TaskId::new("t1")));
    assert!(!config_table.contains_key(&TaskId::new("local~wJ:p1")));
    let repo_table = domain
        .progress_for(&repo_def)
        .expect("Repo Project app 有進度表");
    assert_eq!(
        repo_table.get(&TaskId::new("local~wJ:p1")),
        Some(&progress("Review", Mark::None))
    );

    domain
        .progress_for_mut(&repo_def)
        .insert(TaskId::new("local~wJ:p2"), progress("Plan", Mark::Failed));
    domain
        .progress_for_mut(&config_def)
        .insert(TaskId::new("t1"), progress("B", Mark::Completed));
    assert_eq!(
        domain.repo_progress[&ProjectId::new("app")].get(&TaskId::new("local~wJ:p2")),
        Some(&progress("Plan", Mark::Failed))
    );
    assert!(!domain.progress[&ProjectId::new("app")].contains_key(&TaskId::new("local~wJ:p2")));
    assert_eq!(
        domain.progress[&ProjectId::new("app")].get(&TaskId::new("t1")),
        Some(&progress("B", Mark::Completed))
    );
    assert!(!domain.repo_progress[&ProjectId::new("app")].contains_key(&TaskId::new("t1")));
}

/// `progress_for` 在該 project 沒有任何進度時回 `None`；`progress_for_mut` 會建立空表。
#[test]
fn progress_for_mut_creates_an_empty_table_for_a_new_repo_project() {
    let mut domain = DomainState::default();
    let repo_def = ProjectDef {
        repo: Some(RepoKey::new(APP)),
        ..config_project("app")
    };
    assert!(domain.progress_for(&repo_def).is_none());
    assert!(domain.progress_for_mut(&repo_def).is_empty());
    assert!(domain.repo_progress.contains_key(&ProjectId::new("app")));
    assert!(domain.progress.is_empty());
}

/// spec `repo-projects`「Repo Project 的輸入驗證」：名稱去除前後空白後 1～64 個字元、不含控制字元。
#[test]
fn repo_project_name_rules() {
    assert_eq!(normalize_repo_project_name("  App "), Some(s("App")));
    assert_eq!(
        normalize_repo_project_name(&"字".repeat(64)),
        Some("字".repeat(64))
    );
    assert_eq!(normalize_repo_project_name(&"a".repeat(65)), None);
    assert_eq!(normalize_repo_project_name(""), None);
    assert_eq!(normalize_repo_project_name("   "), None);
    assert_eq!(normalize_repo_project_name("A\u{7}B"), None);
    // repo-projects task 4.6：零寬與雙向覆寫等格式字元（U+200B、U+200E–U+200F、U+061C、U+202A–U+202E、
    // U+2060–U+2064、U+2066–U+2069、U+FEFF）會讓名稱顯示錯亂，一併拒絕；範圍兩端以外的鄰近字元照常接受。
    // U+200C／U+200D（ZWNJ／ZWJ）有正當用途，放行（見下一個測試）。
    for bad in [
        '\u{200B}', '\u{200E}', '\u{200F}', '\u{061C}', '\u{202A}', '\u{202E}', '\u{2060}',
        '\u{2064}', '\u{2066}', '\u{2069}', '\u{FEFF}',
    ] {
        assert_eq!(
            normalize_repo_project_name(&format!("A{bad}B")),
            None,
            "{bad:?}"
        );
    }
    for ok in ['\u{2010}', '\u{2070}', '\u{FEFE}', '\u{061B}', '\u{061D}'] {
        let name = format!("A{ok}B");
        assert_eq!(
            normalize_repo_project_name(&name),
            Some(name.clone()),
            "{ok:?}"
        );
    }
}

/// ZWJ（U+200D）組成的 emoji 序列與 ZWNJ（U+200C）是正當字元，名稱與 stage 都接受（design D6）。
#[test]
fn repo_project_labels_accept_zwj_and_zwnj() {
    for name in ["👩\u{200D}💻 app", "می\u{200C}خواهم"] {
        assert_eq!(
            normalize_repo_project_name(name),
            Some(name.to_string()),
            "{name:?}"
        );
        assert_eq!(
            normalize_repo_project_stages(&[name.to_string()]),
            Some(vec![name.to_string()]),
            "{name:?}"
        );
    }
    assert_eq!(normalize_repo_project_stages(&[s("Plan\u{061C}")]), None);
}

/// stages：1～12 個；每個去除前後空白後 1～32 個字元、不含控制字元、互不相同。
#[test]
fn repo_project_stage_rules() {
    let v = |items: &[&str]| items.iter().map(|x| s(x)).collect::<Vec<_>>();
    assert_eq!(
        normalize_repo_project_stages(&v(&[" Plan ", "Build"])),
        Some(v(&["Plan", "Build"]))
    );
    assert_eq!(normalize_repo_project_stages(&[]), None);
    let twelve: Vec<String> = (0..12).map(|i| format!("S{i}")).collect();
    assert_eq!(normalize_repo_project_stages(&twelve), Some(twelve.clone()));
    let thirteen: Vec<String> = (0..13).map(|i| format!("S{i}")).collect();
    assert_eq!(normalize_repo_project_stages(&thirteen), None);
    assert_eq!(normalize_repo_project_stages(&v(&["Plan", "Plan"])), None);
    assert_eq!(normalize_repo_project_stages(&v(&["Plan", " Plan"])), None);
    assert_eq!(normalize_repo_project_stages(&v(&["Plan\u{7}"])), None);
    assert_eq!(normalize_repo_project_stages(&v(&["Plan\u{202E}"])), None);
    assert_eq!(normalize_repo_project_stages(&v(&["\u{200B}Plan"])), None);
    assert_eq!(normalize_repo_project_stages(&v(&["  "])), None);
    assert_eq!(
        normalize_repo_project_stages(&[s(&"x".repeat(32))]),
        Some(vec![s(&"x".repeat(32))])
    );
    assert_eq!(normalize_repo_project_stages(&[s(&"x".repeat(33))]), None);
}

/// id 須由 `[A-Za-z0-9_-]` 組成（且非空）。
#[test]
fn repo_project_id_rules() {
    let max = "a".repeat(64);
    for ok in ["app", "my-app_2", "A-b", "repo", max.as_str()] {
        assert!(is_valid_repo_project_id(ok), "{ok} 應合法");
    }
    // repo-projects task 4.6：長度上限 64，與設定檔 id 規則 `^[A-Za-z0-9_-]{1,64}$` 一致。
    let too_long = "a".repeat(65);
    for bad in ["", "a/b", "a b", "app~1", "應用", too_long.as_str()] {
        assert!(!is_valid_repo_project_id(bad), "{bad:?} 應不合法");
    }
}
