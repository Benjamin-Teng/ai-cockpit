//! Task 3.2 驗收測試：`cockpit::progress` 的狀態檔載入與容錯
//! （spec `pipeline-progress`「狀態檔載入與容錯」；design D5）。

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use cockpit::config::{self, Args, Config};
use cockpit::progress::{self, ProgressError};
use cockpit::progress_service::ProgressService;
use cockpit_core::{
    Mark, Override, PaneId, ProgressOp, ProjectId, RuntimeId, RuntimeStore, StoreHandle, TaskId,
    WorkstreamId,
};

/// 每個測試專用的暫存目錄；沿用 `cockpit/tests/config.rs` 的自製 `TempDir`
/// （task 3.1 裁決：不加 `tempfile`）。
struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(tag: &str) -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock 應晚於 UNIX_EPOCH")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "cockpit-progress-test-{tag}-{}-{nanos}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("建立測試暫存目錄");
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn write(&self, name: &str, contents: &str) -> PathBuf {
        let file = self.path.join(name);
        fs::write(&file, contents).expect("寫入測試檔案");
        file
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn no_env(_key: &str) -> Option<String> {
    None
}

/// 一份設定：一個 project `p`（stages `Spec`→`Build`）、一條 workstream `be`（綁 runtime
/// `win`）、一個 task `t1`（起始 stage 預設＝`Spec`）。
const SAMPLE_CONFIG: &str = r#"
[[runtime]]
id = "win"
kind = "herdr"

[[project]]
id = "p"
stages = ["Spec", "Build"]

[[project.workstream]]
id = "be"

[[project.task]]
id = "t1"
workstream = "be"
"#;

/// 同 `SAMPLE_CONFIG`，但 `stages` 已把 `Build` 改名成 `Implement`（Scenario「stage 被改名」）。
const RENAMED_STAGE_CONFIG: &str = r#"
[[runtime]]
id = "win"
kind = "herdr"

[[project]]
id = "p"
stages = ["Spec", "Implement"]

[[project.workstream]]
id = "be"

[[project.task]]
id = "t1"
workstream = "be"
"#;

/// 設定檔已刪掉 task `old`（Scenario「設定檔刪掉的 task」）。
const TASK_DELETED_CONFIG: &str = r#"
[[runtime]]
id = "win"
kind = "herdr"

[[project]]
id = "p"
stages = ["Spec", "Build"]

[[project.workstream]]
id = "be"

[[project.task]]
id = "t1"
workstream = "be"
"#;

/// 把 `toml_text` 寫成 `dir` 底下的 `cockpit.toml` 並載入；回傳解析後的 `Config`。
fn load_config(dir: &TempDir, toml_text: &str) -> Config {
    dir.write("cockpit.toml", toml_text);
    let args = Args { config: None };
    config::load(&args, dir.path(), &no_env).expect("測試設定檔應可載入")
}

fn runtime_ids(config: &Config) -> HashSet<&str> {
    config.runtimes.iter().map(|r| r.id.as_str()).collect()
}

fn state_path(config: &Config) -> PathBuf {
    config
        .state_path
        .clone()
        .expect("有 project 的設定應解出 state_path")
}

#[test]
fn missing_state_file_yields_initial_progress_without_creating_file() {
    let dir = TempDir::new("missing");
    let config = load_config(&dir, SAMPLE_CONFIG);
    let path = state_path(&config);
    let runtimes = runtime_ids(&config);

    let domain = progress::load_progress(&path, config.projects.clone(), &runtimes)
        .expect("沒有狀態檔應該載入成功");

    assert!(!path.exists(), "載入不應建立狀態檔");
    let t1 = domain.progress[&ProjectId::new("p")]
        .get(&TaskId::new("t1"))
        .expect("t1 應有初始進度");
    assert_eq!(t1.stage, "Spec");
    assert_eq!(t1.mark, Mark::None);
    assert!(domain.overrides.is_empty());
    assert!(domain.warnings.is_empty());
}

#[test]
fn corrupt_state_file_fails_with_path() {
    let dir = TempDir::new("corrupt");
    let config = load_config(&dir, SAMPLE_CONFIG);
    let path = state_path(&config);
    fs::write(&path, "{not json").expect("寫入損毀狀態檔");
    let runtimes = runtime_ids(&config);

    let error = progress::load_progress(&path, config.projects.clone(), &runtimes)
        .expect_err("損毀狀態檔應載入失敗");

    assert!(matches!(error, ProgressError::Parse { .. }));
    assert!(
        error.to_string().contains(&path.display().to_string()),
        "錯誤訊息應含狀態檔路徑：{error}"
    );
}

#[test]
fn unsupported_version_fails_with_path() {
    let dir = TempDir::new("version");
    let config = load_config(&dir, SAMPLE_CONFIG);
    let path = state_path(&config);
    fs::write(&path, r#"{"version": 3, "projects": {}}"#).expect("寫入版本不符狀態檔");
    let runtimes = runtime_ids(&config);

    let error = progress::load_progress(&path, config.projects.clone(), &runtimes)
        .expect_err("version 不是 1 或 2 應載入失敗");

    assert!(matches!(error, ProgressError::UnsupportedVersion { .. }));
    assert!(
        error.to_string().contains(&path.display().to_string()),
        "錯誤訊息應含狀態檔路徑：{error}"
    );
}

#[test]
fn invalid_mark_value_is_treated_as_corrupt() {
    let dir = TempDir::new("bad-mark");
    let config = load_config(&dir, SAMPLE_CONFIG);
    let path = state_path(&config);
    fs::write(
        &path,
        r#"{"version": 1, "projects": {"p": {"tasks": {"t1": {"stage": "Spec", "mark": "bogus"}}, "overrides": {}}}}"#,
    )
    .expect("寫入 mark 不合法的狀態檔");
    let runtimes = runtime_ids(&config);

    let error = progress::load_progress(&path, config.projects.clone(), &runtimes)
        .expect_err("mark 不合法應視為損毀");

    assert!(matches!(error, ProgressError::Parse { .. }));
}

/// Codex fix round 1（high）：`{"version": 1}` 整個沒有 `projects` 欄位，過去因為
/// `#[serde(default)]` 會被靜默當成「沒有任何 project 的進度／覆蓋」而載入成功，等於把已存在
/// 的進度悄悄重設；現在必須視為無法解析成狀態檔形狀，啟動失敗。
#[test]
fn missing_projects_field_is_treated_as_corrupt() {
    let dir = TempDir::new("missing-projects-field");
    let config = load_config(&dir, SAMPLE_CONFIG);
    let path = state_path(&config);
    fs::write(&path, r#"{"version": 1}"#).expect("寫入缺 projects 欄位的狀態檔");
    let runtimes = runtime_ids(&config);

    let error = progress::load_progress(&path, config.projects.clone(), &runtimes)
        .expect_err("缺 projects 欄位應視為損毀，不能靜默當成空狀態");

    assert!(matches!(error, ProgressError::Parse { .. }));
    assert!(
        error.to_string().contains(&path.display().to_string()),
        "錯誤訊息應含狀態檔路徑：{error}"
    );
}

/// Codex fix round 1（high）：project 物件存在，但缺 `tasks` 欄位——同上，不能被
/// `#[serde(default)]` 靜默補成空集合。
#[test]
fn missing_tasks_field_in_project_is_treated_as_corrupt() {
    let dir = TempDir::new("missing-tasks-field");
    let config = load_config(&dir, SAMPLE_CONFIG);
    let path = state_path(&config);
    fs::write(
        &path,
        r#"{"version": 1, "projects": {"p": {"overrides": {}}}}"#,
    )
    .expect("寫入缺 tasks 欄位的 project");
    let runtimes = runtime_ids(&config);

    let error = progress::load_progress(&path, config.projects.clone(), &runtimes)
        .expect_err("project 缺 tasks 欄位應視為損毀");

    assert!(matches!(error, ProgressError::Parse { .. }));
}

/// Codex fix round 1（high）：project 物件存在，但缺 `overrides` 欄位。
#[test]
fn missing_overrides_field_in_project_is_treated_as_corrupt() {
    let dir = TempDir::new("missing-overrides-field");
    let config = load_config(&dir, SAMPLE_CONFIG);
    let path = state_path(&config);
    fs::write(&path, r#"{"version": 1, "projects": {"p": {"tasks": {}}}}"#)
        .expect("寫入缺 overrides 欄位的 project");
    let runtimes = runtime_ids(&config);

    let error = progress::load_progress(&path, config.projects.clone(), &runtimes)
        .expect_err("project 缺 overrides 欄位應視為損毀");

    assert!(matches!(error, ProgressError::Parse { .. }));
}

#[test]
fn unknown_project_in_state_file_is_ignored() {
    let dir = TempDir::new("unknown-project");
    let config = load_config(&dir, SAMPLE_CONFIG);
    let path = state_path(&config);
    fs::write(
        &path,
        r#"{"version": 1, "projects": {"ghost": {"tasks": {}, "overrides": {}}}}"#,
    )
    .expect("寫入含未知 project 的狀態檔");
    let runtimes = runtime_ids(&config);

    let domain = progress::load_progress(&path, config.projects.clone(), &runtimes)
        .expect("未知 project 應忽略，不影響啟動");

    assert_eq!(domain.projects.len(), 1);
    assert!(!domain.progress.contains_key(&ProjectId::new("ghost")));
}

#[test]
fn unknown_task_in_state_file_is_ignored() {
    let dir = TempDir::new("unknown-task");
    let config = load_config(&dir, TASK_DELETED_CONFIG);
    let path = state_path(&config);
    fs::write(
        &path,
        r#"{"version": 1, "projects": {"p": {"tasks": {
            "t1": {"stage": "Spec", "mark": "none"},
            "old": {"stage": "Build", "mark": "completed"}
        }, "overrides": {}}}}"#,
    )
    .expect("寫入含已刪除 task 的狀態檔");
    let runtimes = runtime_ids(&config);

    let domain = progress::load_progress(&path, config.projects.clone(), &runtimes)
        .expect("設定檔已刪掉的 task 應忽略，不影響啟動");

    let project_progress = &domain.progress[&ProjectId::new("p")];
    assert_eq!(project_progress.len(), 1, "只剩設定檔仍有的 t1");
    assert!(!project_progress.contains_key(&TaskId::new("old")));
}

#[test]
fn task_missing_from_state_file_uses_initial_progress() {
    let dir = TempDir::new("task-missing");
    let config = load_config(&dir, SAMPLE_CONFIG);
    let path = state_path(&config);
    fs::write(
        &path,
        r#"{"version": 1, "projects": {"p": {"tasks": {}, "overrides": {}}}}"#,
    )
    .expect("寫入沒有 t1 的狀態檔");
    let runtimes = runtime_ids(&config);

    let domain = progress::load_progress(&path, config.projects.clone(), &runtimes)
        .expect("設定檔有、狀態檔沒有的 task 應載入成功");

    let t1 = domain.progress[&ProjectId::new("p")]
        .get(&TaskId::new("t1"))
        .expect("t1 應有初始進度");
    assert_eq!(t1.stage, "Spec");
    assert_eq!(t1.mark, Mark::None);
}

#[test]
fn stage_renamed_falls_back_to_initial_stage_and_warns() {
    let dir = TempDir::new("stage-renamed");
    let config = load_config(&dir, RENAMED_STAGE_CONFIG);
    let path = state_path(&config);
    fs::write(
        &path,
        r#"{"version": 1, "projects": {"p": {"tasks": {
            "t1": {"stage": "Build", "mark": "none"}
        }, "overrides": {}}}}"#,
    )
    .expect("寫入 stage 已不存在的狀態檔");
    let runtimes = runtime_ids(&config);

    let domain = progress::load_progress(&path, config.projects.clone(), &runtimes)
        .expect("stage 被改名應退回起始 stage，不是啟動失敗");

    let t1 = domain.progress[&ProjectId::new("p")]
        .get(&TaskId::new("t1"))
        .expect("t1 應存在");
    assert_eq!(t1.stage, "Spec", "應退回起始 stage");
    assert_eq!(t1.mark, Mark::None, "標記應保留");

    let project_warnings = domain
        .warnings
        .get(&ProjectId::new("p"))
        .expect("應有一則 warning");
    assert_eq!(project_warnings.len(), 1);
    assert!(project_warnings[0].contains("t1"));
    assert!(project_warnings[0].contains("Build"));
}

#[test]
fn valid_override_is_loaded() {
    let dir = TempDir::new("override-valid");
    let config = load_config(&dir, SAMPLE_CONFIG);
    let path = state_path(&config);
    fs::write(
        &path,
        r#"{"version": 1, "projects": {"p": {"tasks": {}, "overrides": {
            "be": {"runtime": "win", "pane_id": "w1:p1"}
        }}}}"#,
    )
    .expect("寫入含合法覆蓋的狀態檔");
    let runtimes = runtime_ids(&config);

    let domain = progress::load_progress(&path, config.projects.clone(), &runtimes)
        .expect("合法覆蓋應載入成功");

    let project_overrides = domain
        .overrides
        .get(&ProjectId::new("p"))
        .expect("p 應有覆蓋");
    assert_eq!(
        project_overrides.get(&cockpit_core::WorkstreamId::new("be")),
        Some(&Override {
            runtime: RuntimeId::new("win"),
            pane_id: PaneId::new("w1:p1"),
        })
    );
}

#[test]
fn override_for_unknown_workstream_is_ignored() {
    let dir = TempDir::new("override-unknown-ws");
    let config = load_config(&dir, SAMPLE_CONFIG);
    let path = state_path(&config);
    fs::write(
        &path,
        r#"{"version": 1, "projects": {"p": {"tasks": {}, "overrides": {
            "ghost": {"runtime": "win", "pane_id": "w1:p1"}
        }}}}"#,
    )
    .expect("寫入指向未知 workstream 的覆蓋");
    let runtimes = runtime_ids(&config);

    let domain = progress::load_progress(&path, config.projects.clone(), &runtimes)
        .expect("未知 workstream 的覆蓋應忽略，不影響啟動");

    assert!(!domain.overrides.contains_key(&ProjectId::new("p")));
}

#[test]
fn override_with_unknown_runtime_is_ignored() {
    let dir = TempDir::new("override-unknown-runtime");
    let config = load_config(&dir, SAMPLE_CONFIG);
    let path = state_path(&config);
    fs::write(
        &path,
        r#"{"version": 1, "projects": {"p": {"tasks": {}, "overrides": {
            "be": {"runtime": "ghost-runtime", "pane_id": "w1:p1"}
        }}}}"#,
    )
    .expect("寫入指向未知 runtime 的覆蓋");
    let runtimes = runtime_ids(&config);

    let domain = progress::load_progress(&path, config.projects.clone(), &runtimes)
        .expect("覆蓋 runtime 未知應忽略，不影響啟動");

    assert!(!domain.overrides.contains_key(&ProjectId::new("p")));
}

/// 兩條 workstream `be`／`fe`、各兩個 task：`b1`、`b2` 屬 `be`，`f1` 屬 `fe`（progress-model task 3.1
/// 的「目前 task」測試用）。
const ACTIVE_CONFIG: &str = r#"
[[runtime]]
id = "win"
kind = "herdr"

[[project]]
id = "p"
stages = ["Plan", "Build"]

[[project.workstream]]
id = "be"

[[project.workstream]]
id = "fe"

[[project.task]]
id = "b1"
workstream = "be"

[[project.task]]
id = "b2"
workstream = "be"

[[project.task]]
id = "f1"
workstream = "fe"
"#;

fn active_of(domain: &cockpit_core::DomainState, workstream: &str) -> Option<TaskId> {
    domain
        .active_task(&ProjectId::new("p"), &WorkstreamId::new(workstream))
        .cloned()
}

fn load_active_config(tag: &str, json: &str) -> Result<cockpit_core::DomainState, ProgressError> {
    let dir = TempDir::new(tag);
    let config = load_config(&dir, ACTIVE_CONFIG);
    let path = state_path(&config);
    fs::write(&path, json).expect("寫入狀態檔");
    let runtimes = runtime_ids(&config);
    progress::load_progress(&path, config.projects.clone(), &runtimes)
}

/// `b2` 載入後標記為 `completed`，其餘 `none`。
const ACTIVE_TASKS: &str = r#""tasks": {"b1": {"stage": "Plan", "mark": "none"}, "b2": {"stage": "Plan", "mark": "completed"}, "f1": {"stage": "Plan", "mark": "none"}}"#;

fn v2_json(active: &str) -> String {
    format!(
        r#"{{"version": 2, "projects": {{"p": {{{ACTIVE_TASKS}, "overrides": {{}}, "active": {active}}}}}}}"#
    )
}

#[test]
fn v1_file_loads_with_no_active_task() {
    let domain = load_active_config(
        "v1-legacy",
        r#"{"version": 1, "projects": {"p": {"tasks": {"b1": {"stage": "Build", "mark": "none"}}, "overrides": {}}}}"#,
    )
    .expect("v1 舊檔應可讀取");

    assert_eq!(
        domain.progress[&ProjectId::new("p")][&TaskId::new("b1")].stage,
        "Build"
    );
    assert!(domain.active.is_empty(), "v1 沒有目前 task");
}

#[test]
fn v1_file_with_active_field_is_rejected() {
    let error = load_active_config(
        "v1-with-active",
        r#"{"version": 1, "projects": {"p": {"tasks": {}, "overrides": {}, "active": {}}}}"#,
    )
    .expect_err("v1 不得有 active");

    assert!(matches!(error, ProgressError::Parse { .. }), "{error:?}");
    assert!(
        error.to_string().contains("active"),
        "訊息應指出 active：{error}"
    );
}

/// 斷言載入失敗為 `Parse`，且訊息同時含狀態檔路徑與 `active`（spec「狀態檔載入與容錯」：訊息含路徑與原因）。
fn assert_active_rejected_with_path(tag: &str, json: &str) {
    let dir = TempDir::new(tag);
    let config = load_config(&dir, ACTIVE_CONFIG);
    let path = state_path(&config);
    fs::write(&path, json).expect("寫入狀態檔");
    let runtimes = runtime_ids(&config);

    let error = progress::load_progress(&path, config.projects.clone(), &runtimes)
        .expect_err("應視為損毀、啟動失敗");

    assert!(matches!(error, ProgressError::Parse { .. }), "{error:?}");
    let message = error.to_string();
    assert!(
        message.contains(&path.display().to_string()),
        "訊息應含狀態檔路徑：{message}"
    );
    assert!(message.contains("active"), "訊息應指出 active：{message}");
}

/// ui-fixes task 3.4：v1 的 `"active": null` 不得被當成欄位缺席而放行（先紅後綠）。
#[test]
fn v1_file_with_null_active_is_rejected() {
    assert_active_rejected_with_path(
        "v1-null-active",
        r#"{"version": 1, "projects": {"p": {"tasks": {}, "overrides": {}, "active": null}}}"#,
    );
}

/// ui-fixes task 3.4 回歸：v1 的 `"active": {}` 一直是損毀。
#[test]
fn v1_file_with_empty_object_active_is_rejected_with_path() {
    assert_active_rejected_with_path(
        "v1-empty-active",
        r#"{"version": 1, "projects": {"p": {"tasks": {}, "overrides": {}, "active": {}}}}"#,
    );
}

/// ui-fixes task 3.4 回歸：v2 的 `"active": null` 維持損毀（型別改成雙層 Option 後不能放行）。
#[test]
fn v2_file_with_null_active_is_rejected() {
    assert_active_rejected_with_path(
        "v2-null-active",
        r#"{"version": 2, "projects": {"p": {"tasks": {}, "overrides": {}, "active": null}}}"#,
    );
}

#[test]
fn v2_file_without_active_field_is_rejected() {
    let error = load_active_config(
        "v2-no-active",
        r#"{"version": 2, "projects": {"p": {"tasks": {}, "overrides": {}}}}"#,
    )
    .expect_err("v2 必須有 active");

    assert!(matches!(error, ProgressError::Parse { .. }), "{error:?}");
    assert!(
        error.to_string().contains("active"),
        "訊息應指出 active：{error}"
    );
}

#[test]
fn v2_valid_active_is_loaded() {
    let domain =
        load_active_config("v2-valid", &v2_json(r#"{"be": "b1", "fe": "f1"}"#)).expect("合法 v2");

    assert_eq!(active_of(&domain, "be"), Some(TaskId::new("b1")));
    assert_eq!(active_of(&domain, "fe"), Some(TaskId::new("f1")));
}

#[test]
fn v2_active_pointing_to_task_of_other_workstream_is_ignored() {
    let domain = load_active_config("v2-wrong-ws", &v2_json(r#"{"be": "f1"}"#))
        .expect("無效 active 項目應忽略，不影響啟動");

    assert_eq!(active_of(&domain, "be"), None);
    assert!(domain.active.is_empty(), "空 map 不保留");
}

#[test]
fn v2_active_with_unknown_workstream_or_task_is_ignored() {
    let domain = load_active_config("v2-unknown", &v2_json(r#"{"ghost": "b1", "be": "ghost"}"#))
        .expect("無效 active 項目應忽略");

    assert!(domain.active.is_empty());
}

#[test]
fn v2_active_with_marked_task_is_ignored() {
    let domain =
        load_active_config("v2-marked", &v2_json(r#"{"be": "b2"}"#)).expect("已標記者應忽略");

    assert_eq!(active_of(&domain, "be"), None);
}

#[test]
fn v2_unknown_project_with_active_is_ignored() {
    let domain = load_active_config(
        "v2-ghost-project",
        r#"{"version": 2, "projects": {"ghost": {"tasks": {}, "overrides": {}, "active": {"be": "b1"}}}}"#,
    )
    .expect("未知 project 整筆忽略");

    assert!(domain.active.is_empty());
}

/// 載入帶目前 task 的 v2 檔，經服務寫出（任一被接受的操作），再讀回：目前 task 保留，且寫出的
/// 檔案是 v2、含 `active`（Scenario「重啟後保留」）。
#[tokio::test]
async fn written_state_is_v2_and_round_trips_with_active() {
    let dir = TempDir::new("round-trip");
    let config = load_config(&dir, ACTIVE_CONFIG);
    let path = state_path(&config);
    fs::write(&path, v2_json(r#"{"fe": "f1"}"#)).expect("寫入 v2 狀態檔");
    let runtimes = runtime_ids(&config);
    let domain = progress::load_progress(&path, config.projects.clone(), &runtimes).expect("載入");
    let handle = StoreHandle::new_with_domain(RuntimeStore::new(), domain);
    let service = ProgressService::new(handle.clone(), path.clone());

    service
        .apply_progress(
            &ProjectId::new("p"),
            &TaskId::new("b1"),
            ProgressOp::Advance,
        )
        .await
        .expect("推進應成功並落檔");

    let json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&path).expect("讀檔")).expect("JSON");
    assert_eq!(json["version"], 2);
    assert_eq!(
        json["projects"]["p"]["active"],
        serde_json::json!({"fe": "f1"})
    );

    let reloaded =
        progress::load_progress(&path, config.projects.clone(), &runtimes).expect("讀回");
    assert_eq!(
        reloaded,
        handle.with_domain(Clone::clone),
        "寫出再讀回應相同"
    );
    assert_eq!(active_of(&reloaded, "fe"), Some(TaskId::new("f1")));
}

/// v1 舊檔經一次被接受的操作後寫成 v2，且每個 project 都帶 `active`（空物件）
/// （Scenario「讀取 v1 舊檔」）。
#[tokio::test]
async fn v1_file_is_rewritten_as_v2_with_empty_active() {
    let dir = TempDir::new("v1-rewrite");
    let config = load_config(&dir, ACTIVE_CONFIG);
    let path = state_path(&config);
    fs::write(
        &path,
        r#"{"version": 1, "projects": {"p": {"tasks": {"b1": {"stage": "Plan", "mark": "none"}}, "overrides": {}}}}"#,
    )
    .expect("寫入 v1 狀態檔");
    let runtimes = runtime_ids(&config);
    let domain = progress::load_progress(&path, config.projects.clone(), &runtimes).expect("載入");
    let handle = StoreHandle::new_with_domain(RuntimeStore::new(), domain);
    let service = ProgressService::new(handle, path.clone());

    service
        .apply_progress(
            &ProjectId::new("p"),
            &TaskId::new("b1"),
            ProgressOp::Advance,
        )
        .await
        .expect("推進應成功並落檔");

    let json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&path).expect("讀檔")).expect("JSON");
    assert_eq!(json["version"], 2);
    assert_eq!(json["projects"]["p"]["active"], serde_json::json!({}));
}

#[test]
fn active_is_ignored_when_workstream_override_is_dropped_on_load() {
    // review M3：覆蓋因 runtime 不在設定檔被忽略 → 綁定退回自動（agent 可能已不同），
    // 該 workstream 的目前 task 一併忽略；沒有被丟棄覆蓋的 workstream 不受影響。
    let json = format!(
        r#"{{"version": 2, "projects": {{"p": {{{ACTIVE_TASKS}, "overrides": {{"be": {{"runtime": "ghost-runtime", "pane_id": "w1:p1"}}}}, "active": {{"be": "b1", "fe": "f1"}}}}}}}}"#
    );

    let domain = load_active_config("override-dropped", &json).expect("無效覆蓋應忽略，不影響啟動");

    assert!(
        domain
            .overrides
            .get(&ProjectId::new("p"))
            .is_none_or(|m| m.is_empty()),
        "覆蓋已被忽略"
    );
    assert_eq!(
        active_of(&domain, "be"),
        None,
        "被丟棄覆蓋的 workstream 目前 task 一併忽略"
    );
    assert_eq!(active_of(&domain, "fe"), Some(TaskId::new("f1")));
}
