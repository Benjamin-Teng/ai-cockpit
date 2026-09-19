//! Task 3.2 驗收測試：`cockpit::progress` 的狀態檔載入與容錯
//! （spec `pipeline-progress`「狀態檔載入與容錯」；design D5）。

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use cockpit::config::{self, Args, Config};
use cockpit::progress::{self, ProgressError};
use cockpit_core::{Mark, Override, PaneId, ProjectId, RuntimeId, TaskId};

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
    fs::write(&path, r#"{"version": 2, "projects": {}}"#).expect("寫入版本不符狀態檔");
    let runtimes = runtime_ids(&config);

    let error = progress::load_progress(&path, config.projects.clone(), &runtimes)
        .expect_err("version != 1 應載入失敗");

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
