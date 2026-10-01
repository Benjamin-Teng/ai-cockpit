//! agent 端點（progress-model task 3.4；spec `agent-reporting`；design D4、D6）：
//! `GET /api/agent/tasks`、`POST /api/agent/projects/{project}/tasks/{task}/{op}`（`op` 只有
//! `start`、`advance`）。
//!
//! pane 身分（design D4）：handler 讀「最新一份投影」，找 `binding.state == bound` 且
//! `pane_id` 等於 `X-Herdr-Pane-Id` 標頭的 workstream，再排除經由 WSL 連線的 runtime（WSL 內
//! 的 pane id 與 Windows 端互不相通，不能拿來認人）；另外只要兩個以上非 WSL runtime 上都存在
//! id 等於標頭、未 exited 的 pane（不論有沒有綁定；review M2），就無法分辨請求來源（撞號），
//! 一律視為沒有任何綁定。判定依據的覆蓋事實（來源 `override` 的 runtime＋pane id，或 `auto`）
//! 連同請求交給寫入服務，在寫入鎖內重驗（review M1，投影最多落後約 50 ms）。「經由 WSL」取自 [`AppState::path_mappings`]：它由設定檔每個 runtime
//! 的 endpoint 建立，`HerdrEndpoint::Wsl` 恰好對應 [`PathMapping::Wsl`]，不另開一份清單；
//! 表裡沒有的 runtime 視為非 WSL（正式啟動時設定中的每個 runtime 都在表內）。
//!
//! 判定順序（design D6）：來源檢查（middleware）→ `op` 不是 `start`／`advance`（404）→ 標頭缺
//! 或空白（400 `missing_pane_id`）→ project／task 不存在（404）→ task 所屬 workstream 未綁到此
//! pane（403 `pane_not_bound`）→ 規則（409）→ 落檔（500）。先查存在再查綁定，打錯 id 才會得到
//! 404 而不是誤導的 403。實際寫入交給 [`crate::progress_service::ProgressService`]。

use std::collections::HashMap;

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use cockpit_core::{
    BindingSource, Override, ProjectId, ProjectedBinding, ProjectedProject, ProjectedState,
    RuntimeId, TaskId,
};
use serde_json::{Value, json};

use crate::files::PathMapping;
use crate::http::{
    AppState, coded_error_response, error_response, with_no_store_headers, write_error_response,
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
    // 撞號（review M2）：兩個以上非 WSL runtime 上都有 id 相同、未 exited 的 pane（不論有沒有
    // 綁定）就無法分辨請求來自哪裡，一律視為沒有任何綁定。
    let claiming_runtimes = projected
        .runtimes
        .iter()
        .filter(|runtime| !is_wsl(app, &runtime.id))
        .filter(|runtime| {
            runtime
                .workspaces
                .iter()
                .flat_map(|workspace| &workspace.tabs)
                .flat_map(|tab| &tab.panes)
                .any(|pane| pane.id.as_str() == pane_id && !pane.exited)
        })
        .count();
    if claiming_runtimes > 1 {
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
                && !is_wsl(app, runtime)
            {
                let basis = match source {
                    BindingSource::Override => BindingBasis::Override(Override {
                        runtime: runtime.clone(),
                        pane_id: bound.clone(),
                    }),
                    BindingSource::Auto => BindingBasis::Auto,
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
        return error_response(
            StatusCode::NOT_FOUND,
            &format!("agent 端點只提供 start、advance：{op}"),
        );
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
        return coded_error_response(
            StatusCode::FORBIDDEN,
            "pane_not_bound",
            &format!(
                "task {} 所屬的 workstream 沒有綁定到 pane {pane_id}",
                task_id.as_str()
            ),
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
