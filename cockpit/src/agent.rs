//! agent 端點（progress-model task 3.4；spec `agent-reporting`；design D4、D6）：
//! `GET /api/agent/tasks`、`POST /api/agent/projects/{project}/tasks/{task}/{op}`（`op` 只有
//! `start`、`advance`）。
//!
//! pane 身分（design D4）：handler 讀「最新一份投影」，找 `binding.state == bound` 且
//! `pane_id` 等於 `X-Herdr-Pane-Id` 標頭的 workstream，再排除經由 WSL 連線的 runtime（WSL 內
//! 的 pane id 與 Windows 端互不相通，不能拿來認人）；另外只要兩個以上非 WSL runtime 上都存在
//! id 等於標頭、未 exited 的 pane（不論有沒有綁定；review M2），或有 workstream 以 `bound` 綁到
//! 該 runtime 的這個 pane id 卻不在它的 pane 樹中（孤兒 pane；ui-fixes task 3.3），就無法分辨請求
//! 來源（撞號），一律視為沒有任何綁定；斷線 runtime 最後已知的 pane 樹照常參與。判定依據的覆蓋事實（來源 `override` 的 runtime＋pane id，或 `auto`）
//! 連同請求交給寫入服務，在寫入鎖內重驗（review M1，投影最多落後約 50 ms）。「經由 WSL」取自 [`AppState::path_mappings`]：它由設定檔每個 runtime
//! 的 endpoint 建立，`HerdrEndpoint::Wsl` 恰好對應 [`PathMapping::Wsl`]，不另開一份清單；
//! 表裡沒有的 runtime 視為非 WSL（正式啟動時設定中的每個 runtime 都在表內）。
//!
//! 免帶 id 推進 `POST /api/agent/advance`（repo-projects task 4.4；design D7）：不帶路徑參數，身分判定同上；
//! 候選 task＝每條綁定到這個 pane 的 workstream 的目前 task（沒有目前 task 時，恰有一張才取），在寫入鎖內依
//! Domain 選（不用可能落後的投影），恰一張就照 `advance` 推進，零張 404 `no_task_for_pane`、兩張以上 409
//! `ambiguous_task`。
//!
//! 判定順序（design D6）：來源檢查（middleware）→ `op` 不是 `start`／`advance`（404）→ 標頭缺
//! 或空白（400 `missing_pane_id`）→ project／task 不存在（404）→ task 所屬 workstream 未綁到此
//! pane（403 `pane_not_bound`）→ 規則（409）→ 落檔（500）。先查存在再查綁定，打錯 id 才會得到
//! 404 而不是誤導的 403。實際寫入交給 [`crate::progress_service::ProgressService`]。

use std::collections::{HashMap, HashSet};

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use cockpit_core::{
    BindingSource, Override, ProjectId, ProjectedBinding, ProjectedProject, ProjectedState,
    RuntimeId, TaskId, WorkstreamId,
};
use serde_json::{Value, json};

use crate::files::PathMapping;
use crate::http::{
    AppState, coded_error_response, coded_error_response_with_params, invalid_op_response,
    with_no_store_headers, write_error_response,
};
use crate::progress_service::{BindingBasis, WriteError};

/// pane id 標頭名稱（值為 HERDR 在 pane 內提供的 `HERDR_PANE_ID` 環境變數）。
const PANE_ID_HEADER: &str = "x-herdr-pane-id";

/// 從標頭取出去除前後空白後的 pane id；缺少、不是合法字串、或空白回 `None`（呼叫端回
/// [`missing_pane_id`]）。
fn pane_id_header(headers: &HeaderMap) -> Option<String> {
    headers
        .get(PANE_ID_HEADER)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

/// 400 `missing_pane_id`。
fn missing_pane_id() -> Response {
    coded_error_response(
        StatusCode::BAD_REQUEST,
        "missing_pane_id",
        "缺少 X-Herdr-Pane-Id 標頭（值為 pane 內的 HERDR_PANE_ID）",
    )
}

/// 這個 runtime 是否經由 WSL 連線（見模組文件）。
fn is_wsl(app: &AppState, runtime: &RuntimeId) -> bool {
    matches!(
        app.path_mappings.get(runtime),
        Some(PathMapping::Wsl { .. })
    )
}

/// 綁定到 `pane_id` 的 workstream，以 `(project id, workstream id)` 為 key，值是判定依據的綁定
/// 來源（交給寫入服務在鎖內重驗，review M1）。判定規則見模組文件。
fn bound_workstreams(
    app: &AppState,
    projected: &ProjectedState,
    pane_id: &str,
) -> HashMap<(String, String), BindingBasis> {
    bound_workstreams_in(projected, pane_id, |runtime| is_wsl(app, runtime))
}

/// [`bound_workstreams`] 的判定本體，只依賴投影與「這個 runtime 是否經由 WSL」的判斷，
/// 方便以手工組的 [`ProjectedState`] 單元測試（ui-fixes task 3.3）。
fn bound_workstreams_in(
    projected: &ProjectedState,
    pane_id: &str,
    is_wsl: impl Fn(&RuntimeId) -> bool,
) -> HashMap<(String, String), BindingBasis> {
    // 撞號（review M2；ui-fixes task 3.3、design D8）：兩個以上非 WSL runtime「擁有」這個 pane id
    // 就無法分辨請求來自哪裡，一律視為沒有任何綁定。擁有＝pane 樹中有 id 相同、未 exited 的 pane
    // （不論有沒有綁定、連線狀態為何，斷線 runtime 最後已知的 pane 樹照常算），或投影中有
    // workstream 以 `bound` 綁到該 runtime 的這個 pane id（即使 pane 不在樹中，孤兒 pane）。
    // 以 runtime id 去重，同一個 runtime 兩個條件都成立只算一個。
    let mut owners: HashSet<&RuntimeId> = projected
        .runtimes
        .iter()
        .filter(|runtime| !is_wsl(&runtime.id))
        .filter(|runtime| {
            runtime
                .workspaces
                .iter()
                .flat_map(|workspace| &workspace.tabs)
                .flat_map(|tab| &tab.panes)
                .any(|pane| pane.id.as_str() == pane_id && !pane.exited)
        })
        .map(|runtime| &runtime.id)
        .collect();
    owners.extend(
        projected
            .projects
            .iter()
            .flat_map(|project| &project.workstreams)
            .filter_map(|workstream| match &workstream.binding {
                ProjectedBinding::Bound {
                    runtime,
                    pane_id: bound,
                    ..
                } if bound.as_str() == pane_id && !is_wsl(runtime) => Some(runtime),
                _ => None,
            }),
    );
    if owners.len() > 1 {
        return HashMap::new();
    }

    let mut found = HashMap::new();
    for project in &projected.projects {
        for workstream in &project.workstreams {
            if let ProjectedBinding::Bound {
                runtime,
                pane_id: bound,
                source,
                ..
            } = &workstream.binding
                && bound.as_str() == pane_id
                && !is_wsl(runtime)
            {
                let basis = match source {
                    BindingSource::Override => BindingBasis::Override(Override {
                        runtime: runtime.clone(),
                        pane_id: bound.clone(),
                    }),
                    BindingSource::Auto => BindingBasis::Auto,
                    // 固定 pane（Repo Project）：重驗時忽略覆蓋，只確認仍是固定 pane 的工作線
                    // （repo-projects task 3.1）。
                    BindingSource::Pane => BindingBasis::Pinned,
                };
                found.insert(
                    (
                        project.id.as_str().to_string(),
                        workstream.id.as_str().to_string(),
                    ),
                    basis,
                );
            }
        }
    }
    found
}

/// `GET /api/agent/tasks`（spec「查詢自己綁定的 task」）。
pub(crate) async fn list_tasks(State(app): State<AppState>, headers: HeaderMap) -> Response {
    let Some(pane_id) = pane_id_header(&headers) else {
        return missing_pane_id();
    };
    let projected = app.state.borrow().clone();
    let bound = bound_workstreams(&app, &projected, &pane_id);

    let mut workstreams = Vec::new();
    for project in &projected.projects {
        for workstream in &project.workstreams {
            let key = (
                project.id.as_str().to_string(),
                workstream.id.as_str().to_string(),
            );
            if !bound.contains_key(&key) {
                continue;
            }
            let tasks: Vec<Value> = project
                .tasks
                .iter()
                .filter(|task| task.workstream == workstream.id)
                .map(|task| {
                    json!({
                        "id": task.id,
                        "title": task.title,
                        "stage": task.stage,
                        "next_stage": next_stage(project, &task.stage),
                        "mark": task.mark,
                        "status": task.status,
                        "depends_on": task.depends_on,
                    })
                })
                .collect();
            workstreams.push(json!({
                "project": project.id,
                "workstream": workstream.id,
                "active_task": workstream.active_task,
                "tasks": tasks,
            }));
        }
    }

    let body = json!({ "pane_id": pane_id, "workstreams": workstreams }).to_string();
    with_no_store_headers(
        (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "application/json")],
            body,
        )
            .into_response(),
    )
}

/// 下一個 Stage；已是最後一站（或 stage 不在清單內）回 `None`。
fn next_stage<'a>(project: &'a ProjectedProject, stage: &str) -> Option<&'a str> {
    let index = project.stages.iter().position(|s| s == stage)?;
    project.stages.get(index + 1).map(String::as_str)
}

/// `POST /api/agent/projects/{project}/tasks/{task}/{op}`（spec「宣告目前 task」「agent 推進」）。
pub(crate) async fn agent_op(
    State(app): State<AppState>,
    headers: HeaderMap,
    Path((project, task, op)): Path<(String, String, String)>,
) -> Response {
    if op != "start" && op != "advance" {
        return invalid_op_response(&format!("agent 端點只提供 start、advance：{op}"), &op);
    }
    let Some(pane_id) = pane_id_header(&headers) else {
        return missing_pane_id();
    };
    let project_id = ProjectId::new(project);
    let task_id = TaskId::new(task);
    let Some(progress) = &app.progress else {
        return write_error_response(WriteError::UnknownProject(project_id));
    };

    let projected = app.state.borrow().clone();
    let Some(projected_project) = projected.projects.iter().find(|p| p.id == project_id) else {
        return write_error_response(WriteError::UnknownProject(project_id));
    };
    let Some(projected_task) = projected_project.tasks.iter().find(|t| t.id == task_id) else {
        return write_error_response(WriteError::UnknownTask(task_id));
    };
    let bound = bound_workstreams(&app, &projected, &pane_id);
    let key = (
        project_id.as_str().to_string(),
        projected_task.workstream.as_str().to_string(),
    );
    let Some(basis) = bound.get(&key) else {
        return coded_error_response_with_params(
            StatusCode::FORBIDDEN,
            "pane_not_bound",
            &format!(
                "task {} 所屬的 workstream 沒有綁定到 pane {pane_id}",
                task_id.as_str()
            ),
            [("task", task_id.as_str()), ("pane", pane_id.as_str())],
        );
    };

    let result = if op == "start" {
        progress.declare_active(&project_id, &task_id, basis).await
    } else {
        progress.agent_advance(&project_id, &task_id, basis).await
    };
    match result {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(error) => write_error_response(error),
    }
}

/// `POST /api/agent/advance`（spec `agent-reporting`「免帶 id 推進」；design D7）：不需要路徑參數與請求本體。
/// HTTP 層只用投影做 pane 身分判定（[`bound_workstreams`]），候選 task 的選擇與推進都在寫入鎖內依 Domain 進行
/// （[`crate::progress_service::ProgressService::agent_advance_for_pane`]），避免投影落後時推進到舊的 task。
/// 候選零張回 404 `no_task_for_pane`、兩張以上回 409 `ambiguous_task`（狀態都不變）；候選恰一張時的 403／409／500
/// 與 `POST …/{task}/advance` 相同。
pub(crate) async fn advance(State(app): State<AppState>, headers: HeaderMap) -> Response {
    let Some(pane_id) = pane_id_header(&headers) else {
        return missing_pane_id();
    };
    // 沒有寫入服務時（只在測試與 ui_preview）沒有任何綁定可言，與沒有候選同一個結果。
    let Some(progress) = &app.progress else {
        return write_error_response(WriteError::NoTaskForPane);
    };
    let projected = app.state.borrow().clone();
    let mut bound: Vec<_> = bound_workstreams(&app, &projected, &pane_id)
        .into_iter()
        .map(|((project, workstream), basis)| {
            (
                ProjectId::new(project),
                WorkstreamId::new(workstream),
                basis,
            )
        })
        .collect();
    bound.sort_by(|a, b| (a.0.as_str(), a.1.as_str()).cmp(&(b.0.as_str(), b.1.as_str())));
    match progress.agent_advance_for_pane(bound).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(error) => write_error_response(error),
    }
}

#[cfg(test)]
mod tests {
    use cockpit_core::{
        AgentStatus, Focused, PaneId, ProjectKind, ProjectedConnection, ProjectedPane,
        ProjectedRuntime, ProjectedTab, ProjectedWorkspace, ProjectedWorkstream, TabId,
        WorkspaceId, WorkstreamId,
    };

    use super::*;

    fn pane(id: &str) -> ProjectedPane {
        ProjectedPane {
            id: PaneId::new(id),
            agent: None,
            agent_status: AgentStatus::Idle,
            title: None,
            cwd: None,
            label: None,
            focused: false,
            exited: false,
            updated_at: "1970-01-01T00:00:00Z".to_string(),
        }
    }

    /// 帶一個 workspace／tab、pane 樹為 `panes` 的 runtime。
    fn runtime(
        id: &str,
        connection: ProjectedConnection,
        panes: Vec<ProjectedPane>,
    ) -> ProjectedRuntime {
        ProjectedRuntime {
            id: RuntimeId::new(id),
            kind: "herdr".to_string(),
            endpoint: "test".to_string(),
            connection,
            focused: Focused {
                workspace_id: None,
                tab_id: None,
                pane_id: None,
            },
            workspaces: vec![ProjectedWorkspace {
                id: WorkspaceId::new("w1"),
                label: None,
                number: 1,
                agent_status: AgentStatus::Idle,
                focused: false,
                tabs: vec![ProjectedTab {
                    id: TabId::new("w1:t1"),
                    number: 1,
                    agent_status: AgentStatus::Idle,
                    focused: false,
                    panes,
                }],
            }],
        }
    }

    fn connected() -> ProjectedConnection {
        ProjectedConnection::Connected {
            since: "1970-01-01T00:00:00Z".to_string(),
            server_version: "test".to_string(),
            protocol: 1,
            last_snapshot_at: "1970-01-01T00:00:00Z".to_string(),
            protocol_warning: None,
            protocol_warning_msg: None,
        }
    }

    fn bound(runtime: &str, pane_id: &str) -> ProjectedBinding {
        ProjectedBinding::Bound {
            runtime: RuntimeId::new(runtime),
            pane_id: PaneId::new(pane_id),
            source: BindingSource::Override,
            agent: None,
            agent_status: AgentStatus::Idle,
        }
    }

    fn workstream(id: &str, binding: ProjectedBinding) -> ProjectedWorkstream {
        ProjectedWorkstream {
            id: WorkstreamId::new(id),
            name: id.to_string(),
            worktree: None,
            binding,
            active_task: None,
            activity_undeclared: false,
        }
    }

    fn state(
        runtimes: Vec<ProjectedRuntime>,
        workstreams: Vec<ProjectedWorkstream>,
    ) -> ProjectedState {
        ProjectedState {
            version: 1,
            generated_at: "1970-01-01T00:00:00Z".to_string(),
            runtimes,
            projects: vec![ProjectedProject {
                id: ProjectId::new("p"),
                name: "p".to_string(),
                kind: ProjectKind::Config,
                repo: None,
                stages: vec!["Plan".to_string()],
                warnings: Vec::new(),
                warning_msgs: Vec::new(),
                workstreams,
                tasks: Vec::new(),
            }],
            detected_repos: Vec::new(),
            recent_events: Vec::new(),
        }
    }

    fn never_wsl(_: &RuntimeId) -> bool {
        false
    }

    /// spec「孤兒 pane 與另一個 runtime 撞號」：`win2` 的 pane 樹裡沒有 `w1:p1`，但 `fe` 以 `bound`
    /// 綁到它（孤兒 pane）；`win` 有 `be` 綁到真實存在的 `w1:p1`。兩個 runtime 都擁有這個 pane id，
    /// 無法分辨請求來源，結果必須是空集合（ui-fixes task 3.3，design D8）。`GET` 與寫入端點共用
    /// 這個判定，所以寫入同樣被拒。
    #[test]
    fn orphan_bound_pane_collides_with_other_runtime() {
        let projected = state(
            vec![
                runtime("win", connected(), vec![pane("w1:p1")]),
                runtime("win2", connected(), Vec::new()),
            ],
            vec![
                workstream("be", bound("win", "w1:p1")),
                workstream("fe", bound("win2", "w1:p1")),
            ],
        );
        assert!(bound_workstreams_in(&projected, "w1:p1", never_wsl).is_empty());
    }

    /// 對照組：沒有撞號時孤兒 pane 的綁定照常成立（避免上面的測試只是因為判定壞掉而空）。
    #[test]
    fn orphan_bound_pane_alone_still_binds() {
        let projected = state(
            vec![runtime("win2", connected(), Vec::new())],
            vec![workstream("fe", bound("win2", "w1:p1"))],
        );
        let found = bound_workstreams_in(&projected, "w1:p1", never_wsl);
        assert_eq!(found.len(), 1);
        assert!(found.contains_key(&("p".to_string(), "fe".to_string())));
    }

    /// 固定 pane（`source: pane`）的綁定以 `BindingBasis::Pinned` 為判定依據，不是 `Auto`
    /// （repo-projects task 3.1 review fix round 1）。
    #[test]
    fn pinned_pane_binding_uses_pinned_basis() {
        let projected = state(
            vec![runtime("local", connected(), vec![pane("wJ:p1")])],
            vec![workstream(
                "local~wJ:p1",
                ProjectedBinding::Bound {
                    runtime: RuntimeId::new("local"),
                    pane_id: PaneId::new("wJ:p1"),
                    source: BindingSource::Pane,
                    agent: None,
                    agent_status: AgentStatus::Idle,
                },
            )],
        );
        let found = bound_workstreams_in(&projected, "wJ:p1", never_wsl);
        assert_eq!(
            found.get(&("p".to_string(), "local~wJ:p1".to_string())),
            Some(&BindingBasis::Pinned)
        );
    }

    /// 同一個 runtime 同時「pane 樹有未 exited 的 pane」又「有 workstream bound 到它」只算一個擁有者
    /// （以 runtime id 去重），不能因為兩個條件都成立就當成撞號；兩條 workstream 都照常綁定。
    #[test]
    fn same_runtime_owning_by_both_conditions_counts_once() {
        let projected = state(
            vec![runtime("win", connected(), vec![pane("w1:p1")])],
            vec![
                workstream("be", bound("win", "w1:p1")),
                workstream("fe", bound("win", "w1:p1")),
            ],
        );
        assert_eq!(
            bound_workstreams_in(&projected, "w1:p1", never_wsl).len(),
            2
        );
    }

    /// 孤兒 pane 若屬於 WSL runtime，不算擁有者（WSL 的 pane id 與 Windows 端互不相通）。
    #[test]
    fn orphan_bound_pane_on_wsl_runtime_is_not_a_collision() {
        let projected = state(
            vec![
                runtime("win", connected(), vec![pane("w1:p1")]),
                runtime("wsl", connected(), Vec::new()),
            ],
            vec![
                workstream("be", bound("win", "w1:p1")),
                workstream("fe", bound("wsl", "w1:p1")),
            ],
        );
        let found = bound_workstreams_in(&projected, "w1:p1", |r| r.as_str() == "wsl");
        assert_eq!(found.len(), 1);
        assert!(found.contains_key(&("p".to_string(), "be".to_string())));
    }

    /// 已 exited 的 pane 不算擁有。
    #[test]
    fn exited_pane_does_not_own() {
        let mut exited = pane("w1:p1");
        exited.exited = true;
        let projected = state(
            vec![
                runtime("win", connected(), vec![pane("w1:p1")]),
                runtime("win2", connected(), vec![exited]),
            ],
            vec![workstream("be", bound("win", "w1:p1"))],
        );
        assert_eq!(
            bound_workstreams_in(&projected, "w1:p1", never_wsl).len(),
            1
        );
    }
}
