//! openspec-stage-sync task 4.1 驗收測試：狀態檔 v4（design D4、D10-3；spec `pipeline-progress`
//! 「狀態檔格式與持久化」「狀態檔載入與容錯」）。
//!
//! 載入規則打 `progress::load_progress`；寫出規則打真的 `ProgressService`（狀態檔放在每個測試自己的暫存目錄）。

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use cockpit::progress::{self, ProgressError};
use cockpit::progress_service::ProgressService;
use cockpit_core::{
    DomainState, Observation, OpenSpecPhase, ProgressOp, ProjectDef, ProjectId, RuntimeStore,
    StoreHandle, SyncMode, TaskDef, TaskId, TaskSync, WorkstreamDef, WorkstreamId,
};
use serde_json::{Value, json};

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
            "cockpit-state-v4-test-{tag}-{}-{nanos}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("建立測試暫存目錄");
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

/// 手寫 project `p`（stages `Spec`→`Build`、workstream `be`、task `t1`），供「做一次被接受的操作」觸發寫檔。
fn config_project(id: &str) -> ProjectDef {
    ProjectDef {
        id: ProjectId::new(id),
        name: id.to_string(),
        stages: vec!["Spec".to_string(), "Build".to_string()],
        workstreams: vec![WorkstreamDef {
            id: WorkstreamId::new("be"),
            name: "be".to_string(),
            binding: None,
            pinned_pane: None,
        }],
        tasks: vec![TaskDef {
            id: TaskId::new("t1"),
            title: "t1".to_string(),
            workstream: WorkstreamId::new("be"),
            stage: "Spec".to_string(),
            depends_on: Vec::new(),
        }],
        repo: None,
    }
}

fn runtimes() -> HashSet<&'static str> {
    HashSet::from(["local", "dev~1"])
}

fn load(dir: &TempDir, contents: &str) -> (PathBuf, Result<DomainState, ProgressError>) {
    let path = dir.path().join("cockpit.state.json");
    fs::write(&path, contents).expect("寫入狀態檔");
    let result = progress::load_progress(&path, vec![config_project("p")], &runtimes());
    (path, result)
}

fn assert_corrupt(tag: &str, contents: &str) {
    let dir = TempDir::new(tag);
    let (path, result) = load(&dir, contents);
    let error = match result {
        Ok(_) => panic!("{tag}：應視為損毀、啟動失敗"),
        Err(error) => error,
    };
    assert!(
        matches!(error, ProgressError::Parse { .. }),
        "{tag}：應為 Parse 錯誤：{error}"
    );
    let message = error.to_string();
    assert!(
        message.contains(&path.display().to_string()),
        "{tag}：訊息應含狀態檔路徑：{message}"
    );
}

fn phases_of(domain: &DomainState, pid: &str) -> Vec<Option<OpenSpecPhase>> {
    domain
        .repo_projects
        .iter()
        .find(|d| d.id.as_str() == pid)
        .unwrap_or_else(|| panic!("找不到 Repo Project {pid}"))
        .phases
        .clone()
}

fn v3_with(repo_projects: Value) -> String {
    json!({"version":3,"projects":{},"repo_projects":repo_projects}).to_string()
}

fn v4_with(repo_projects: Value) -> String {
    json!({"version":4,"projects":{},"repo_projects":repo_projects}).to_string()
}

/// 載入後對手寫 project 做一次被接受的操作，使檔案被改寫，回傳寫出的 JSON 與把它讀回的 `DomainState`。
async fn load_then_write(tag: &str, contents: &str) -> (Value, DomainState) {
    let dir = TempDir::new(tag);
    let (path, result) = load(&dir, contents);
    let domain = result.expect("應載入成功");
    let handle = StoreHandle::new_with_domain(RuntimeStore::new(), domain);
    let service = ProgressService::new(handle, path.clone());
    service
        .apply_progress(
            &ProjectId::new("p"),
            &TaskId::new("t1"),
            ProgressOp::Advance,
        )
        .await
        .expect("推進應成功");
    let written: Value =
        serde_json::from_str(&fs::read_to_string(&path).expect("讀狀態檔")).expect("JSON");
    let reloaded = progress::load_progress(&path, vec![config_project("p")], &runtimes())
        .expect("寫出的檔案應能讀回");
    (written, reloaded)
}

fn p(phase: OpenSpecPhase) -> Option<OpenSpecPhase> {
    Some(phase)
}

// ---------------------------------------------------------------------------
// v1～v3 升級補預設對應
// ---------------------------------------------------------------------------

/// Scenario「讀取 v3 檔時補上預設對應（繁中站名）」。
#[tokio::test]
async fn v3_file_with_chinese_stage_names_gets_default_phases_and_is_written_as_v4() {
    let (written, reloaded) = load_then_write(
        "v3-zh",
        &v3_with(
            json!({"app":{"name":"App","repo":"r","stages":["規劃","實作","審查","完成"],"tasks":{}}}),
        ),
    )
    .await;

    assert_eq!(written["version"], 4);
    assert_eq!(
        written["repo_projects"]["app"]["phases"],
        json!(["plan", "implement", "review", "complete"])
    );
    assert_eq!(
        phases_of(&reloaded, "app"),
        vec![
            p(OpenSpecPhase::Plan),
            p(OpenSpecPhase::Implement),
            p(OpenSpecPhase::Review),
            p(OpenSpecPhase::Complete)
        ]
    );
}

/// Scenario「讀取 v3 檔時補上預設對應（英文站名與部分對得上）」。
#[test]
fn v3_file_with_english_and_partially_matching_stage_names_gets_default_phases() {
    let dir = TempDir::new("v3-en");
    let (_, result) = load(
        &dir,
        &v3_with(json!({
            "a":{"name":"A","repo":"ra","stages":["Plan","Implement","Review","Complete"],"tasks":{}},
            "b":{"name":"B","repo":"rb","stages":["Plan","Build","Review","Done"],"tasks":{}},
        })),
    );
    let domain = result.expect("載入");
    assert_eq!(
        phases_of(&domain, "a"),
        vec![
            p(OpenSpecPhase::Plan),
            p(OpenSpecPhase::Implement),
            p(OpenSpecPhase::Review),
            p(OpenSpecPhase::Complete)
        ]
    );
    assert_eq!(
        phases_of(&domain, "b"),
        vec![p(OpenSpecPhase::Plan), None, p(OpenSpecPhase::Review), None]
    );
}

/// Scenario「讀取 v3 檔時補對應遇到重複」：後出現的改為 null。
#[test]
fn v3_default_phases_keep_only_the_first_stage_on_duplicates() {
    let dir = TempDir::new("v3-dup");
    let (_, result) = load(
        &dir,
        &v3_with(
            json!({"app":{"name":"App","repo":"r","stages":["Plan","規劃","Review"],"tasks":{}}}),
        ),
    );
    assert_eq!(
        phases_of(&result.expect("載入"), "app"),
        vec![p(OpenSpecPhase::Plan), None, p(OpenSpecPhase::Review)]
    );
}

/// 沒有任何 Repo Project 的 v2 檔也照樣升版號（design Migration Plan）。
#[tokio::test]
async fn v2_file_is_upgraded_to_v4_on_first_write() {
    let (written, _) = load_then_write(
        "v2-upgrade",
        r#"{"version":2,"projects":{"p":{"tasks":{"t1":{"stage":"Spec","mark":"none"}},"overrides":{},"active":{}}}}"#,
    )
    .await;
    assert_eq!(written["version"], 4);
    assert_eq!(written["repo_projects"], json!({}));
}

// ---------------------------------------------------------------------------
// v4 讀寫
// ---------------------------------------------------------------------------

/// Scenario「v4 檔的 null 一律尊重」。
#[test]
fn v4_file_nulls_are_respected_even_for_default_stage_names() {
    let dir = TempDir::new("v4-null");
    let (_, result) = load(
        &dir,
        &v4_with(
            json!({"app":{"name":"App","repo":"r","stages":["Plan","Implement"],"phases":[null,null],"tasks":{}}}),
        ),
    );
    assert_eq!(phases_of(&result.expect("載入"), "app"), vec![None, None]);
}

/// v4 往返不變：載入後寫出，`repo_projects` 與原檔 JSON 相同（含 `sync`）。
#[tokio::test]
async fn v4_file_round_trips_unchanged() {
    let repo_projects = json!({
        "app": {
            "name": "App",
            "repo": "d:\\work\\app\\.git",
            "stages": ["Plan", "Build", "Ship"],
            "phases": ["plan", null, "complete"],
            "tasks": {
                "local~wJ:p1": {
                    "stage": "Build", "mark": "none",
                    "sync": {"mode": "auto", "applied": {"change": "foo", "phase": "implement", "checked": 3, "total": 8}}
                },
                "local~wJ:p2": {
                    "stage": "Plan", "mark": "completed",
                    "sync": {"mode": "manual", "applied": null}
                },
                "local~wJ:p3": {"stage": "Plan", "mark": "failed"}
            }
        }
    });
    let (written, _) = load_then_write("v4-roundtrip", &v4_with(repo_projects.clone())).await;
    assert_eq!(written["version"], 4);
    assert_eq!(written["repo_projects"], repo_projects);
}

/// Scenario「讀取含同步狀態的 v4 檔」：`sync` 讀進 `repo_sync`。
#[test]
fn v4_sync_is_loaded_into_repo_sync() {
    let dir = TempDir::new("v4-sync-load");
    let (_, result) = load(
        &dir,
        &v4_with(
            json!({"app":{"name":"App","repo":"r","stages":["Plan","Build"],"phases":["plan",null],"tasks":{
                "local~wJ:p1":{"stage":"Build","mark":"none","sync":{"mode":"manual","applied":{"change":"foo","phase":"implement","checked":3,"total":8}}},
                "local~wJ:p2":{"stage":"Plan","mark":"none","sync":{"mode":"auto","applied":null}}
            }}}),
        ),
    );
    let domain = result.expect("載入");
    let table = domain
        .repo_sync
        .get(&ProjectId::new("app"))
        .expect("app 有同步狀態");
    assert_eq!(
        table.get(&TaskId::new("local~wJ:p1")),
        Some(&TaskSync {
            mode: SyncMode::Manual,
            applied: Some(Observation {
                change: "foo".to_string(),
                phase: OpenSpecPhase::Implement,
                checked: 3,
                total: 8
            })
        })
    );
    assert_eq!(
        table.get(&TaskId::new("local~wJ:p2")),
        Some(&TaskSync {
            mode: SyncMode::Auto,
            applied: None
        })
    );
}

/// 沒有 `sync` 的 task 不寫 `sync` 欄位，也不會在 `repo_sync` 出現。
#[tokio::test]
async fn task_without_sync_has_no_sync_field_on_either_side() {
    let (written, reloaded) = load_then_write(
        "no-sync",
        &v4_with(
            json!({"app":{"name":"App","repo":"r","stages":["Plan"],"phases":[null],"tasks":{
                "local~wJ:p1":{"stage":"Plan","mark":"none"}
            }}}),
        ),
    )
    .await;
    assert!(
        written["repo_projects"]["app"]["tasks"]["local~wJ:p1"]
            .get("sync")
            .is_none()
    );
    assert!(reloaded.repo_sync.is_empty());
}

/// Scenario「指向未設定 runtime 的 task 連同同步狀態被忽略」。
#[tokio::test]
async fn task_with_unknown_runtime_is_dropped_together_with_its_sync() {
    let (written, reloaded) = load_then_write(
        "ghost-sync",
        &v4_with(
            json!({"app":{"name":"App","repo":"r","stages":["Plan"],"phases":[null],"tasks":{
                "ghost~wJ:p1":{"stage":"Plan","mark":"none","sync":{"mode":"auto","applied":null}},
                "local~wJ:p2":{"stage":"Plan","mark":"none","sync":{"mode":"auto","applied":null}}
            }}}),
        ),
    )
    .await;
    let tasks = written["repo_projects"]["app"]["tasks"]
        .as_object()
        .expect("tasks 物件");
    assert!(!tasks.contains_key("ghost~wJ:p1"));
    assert!(tasks.contains_key("local~wJ:p2"));
    assert!(
        !reloaded
            .repo_sync
            .get(&ProjectId::new("app"))
            .is_some_and(|t| t.contains_key(&TaskId::new("ghost~wJ:p1")))
    );
}

// ---------------------------------------------------------------------------
// 損毀與版本
// ---------------------------------------------------------------------------

/// Scenario「v4 檔的 phases 長度不符視為損毀」。
#[test]
fn v4_phases_length_mismatch_is_corrupt() {
    assert_corrupt(
        "v4-len",
        &v4_with(
            json!({"app":{"name":"App","repo":"r","stages":["A","B","C"],"phases":["plan",null],"tasks":{}}}),
        ),
    );
}

/// Scenario「v4 檔的 phases 重複視為損毀」。
#[test]
fn v4_phases_duplicate_is_corrupt() {
    assert_corrupt(
        "v4-dup",
        &v4_with(
            json!({"app":{"name":"App","repo":"r","stages":["A","B","C"],"phases":["plan","plan",null],"tasks":{}}}),
        ),
    );
}

/// 未知的階段字串、`phases: null` 都是損毀（不靜默當成 null 或缺席）。
#[test]
fn v4_unknown_phase_string_or_null_phases_is_corrupt() {
    assert_corrupt(
        "v4-bogus",
        &v4_with(
            json!({"app":{"name":"App","repo":"r","stages":["A"],"phases":["bogus"],"tasks":{}}}),
        ),
    );
    assert_corrupt(
        "v4-null-phases",
        &v4_with(json!({"app":{"name":"App","repo":"r","stages":["A"],"phases":null,"tasks":{}}})),
    );
}

/// Scenario「v4 檔缺 phases 或 repo_projects 視為損毀」。
#[test]
fn v4_missing_phases_or_repo_projects_is_corrupt() {
    assert_corrupt(
        "v4-no-phases",
        &v4_with(json!({"app":{"name":"App","repo":"r","stages":["A"],"tasks":{}}})),
    );
    assert_corrupt("v4-no-repo-projects", r#"{"version":4,"projects":{}}"#);
    assert_corrupt(
        "v4-null-repo-projects",
        r#"{"version":4,"projects":{},"repo_projects":null}"#,
    );
}

/// Scenario「v3 檔含 phases 或 sync 視為損毀」（含 `null` 值）。
#[test]
fn v3_file_with_phases_or_sync_is_corrupt() {
    assert_corrupt(
        "v3-phases",
        &v3_with(
            json!({"app":{"name":"App","repo":"r","stages":["A"],"phases":[null],"tasks":{}}}),
        ),
    );
    assert_corrupt(
        "v3-null-phases",
        &v3_with(json!({"app":{"name":"App","repo":"r","stages":["A"],"phases":null,"tasks":{}}})),
    );
    assert_corrupt(
        "v3-sync",
        &v3_with(
            json!({"app":{"name":"App","repo":"r","stages":["A"],"tasks":{
                "local~wJ:p1":{"stage":"A","mark":"none","sync":{"mode":"auto","applied":null}}
            }}}),
        ),
    );
    assert_corrupt(
        "v3-null-sync",
        &v3_with(
            json!({"app":{"name":"App","repo":"r","stages":["A"],"tasks":{
                "local~wJ:p1":{"stage":"A","mark":"none","sync":null}
            }}}),
        ),
    );
}

/// Scenario「手寫 project 的 task 帶 sync 視為損毀」（v3、v4 都一樣）。
#[test]
fn hand_written_project_task_with_sync_is_corrupt() {
    for version in [3, 4] {
        let contents = json!({"version":version,"projects":{"p":{
            "tasks":{"t1":{"stage":"Build","mark":"none","sync":{"mode":"manual","applied":null}}},
            "overrides":{},"active":{}
        }},"repo_projects":{}})
        .to_string();
        assert_corrupt(&format!("project-sync-v{version}"), &contents);
    }
}

/// `sync` 形狀不合（mode 非 auto／manual、缺 applied、applied 缺欄位、多餘欄位）都是損毀。
#[test]
fn malformed_sync_is_corrupt() {
    let bad_syncs = [
        json!({"mode":"bogus","applied":null}),
        json!({"mode":"auto"}),
        json!({"mode":"auto","applied":{"change":"foo","phase":"implement","checked":1}}),
        json!({"mode":"auto","applied":{"change":"foo","phase":"nope","checked":1,"total":2}}),
        json!({"mode":"auto","applied":null,"extra":1}),
    ];
    for (index, sync) in bad_syncs.into_iter().enumerate() {
        assert_corrupt(
            &format!("bad-sync-{index}"),
            &v4_with(
                json!({"app":{"name":"App","repo":"r","stages":["A"],"phases":[null],"tasks":{
                    "local~wJ:p1":{"stage":"A","mark":"none","sync":sync}
                }}}),
            ),
        );
    }
}

/// Scenario「不支援的版本」：version 5。
#[test]
fn version_5_is_unsupported() {
    let dir = TempDir::new("v5");
    let (path, result) = load(&dir, r#"{"version":5,"projects":{},"repo_projects":{}}"#);
    let error = result.expect_err("version 5 應啟動失敗");
    assert!(
        matches!(error, ProgressError::UnsupportedVersion { version: 5, .. }),
        "應為 UnsupportedVersion：{error}"
    );
    assert!(error.to_string().contains(&path.display().to_string()));
}
