//! Task 3.3 驗收測試：`cockpit::progress_service` 的寫入服務
//! （spec `pipeline-progress`「狀態檔格式與持久化」「進度寫入端點」「綁定覆蓋端點」；design D3、D4）。

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use cockpit::progress;
use cockpit::progress_service::{ProgressService, WriteError};
use cockpit_core::{
    AgentStatus, ConnectionState, DomainState, Focused, Mark, Override, Pane, PaneId, ProgressOp,
    ProjectDef, ProjectId, Rejection, RuntimeId, RuntimeSnapshot, RuntimeStore, StaleOverride,
    StoreHandle, TabId, TaskDef, TaskId, WorkspaceId, WorkstreamDef, WorkstreamId,
    spawn_projector_with_stale_sink,
};
use tokio::sync::mpsc;

/// 每個測試專用的暫存目錄；沿用 `cockpit/tests/config.rs` 的自製 `TempDir`（不加 `tempfile`）。
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
            "cockpit-progress-service-test-{tag}-{}-{nanos}",
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

const RUNTIME: &str = "win";

/// M4 迴歸測試用：夠寬的 domain（多個 project，每個 project 多條 workstream／task），讓狀態檔
/// 序列化的 map key 順序若依賴 `HashMap` 雜湊而非排序，幾乎必然在兩次獨立寫出間出現差異。
const N_PROJECTS: usize = 5;
const TASKS_PER_PROJECT: usize = 6;

/// `N_PROJECTS` 個 project `p0..`，每個各有 `TASKS_PER_PROJECT` 條 workstream `ws0..` 與同編號
/// 的 task `t0..`（`t{n}` 屬於 `ws{n}`），stages 皆為 `Spec`→`Build`，起始 stage 皆為 `Spec`。
fn wide_projects() -> Vec<ProjectDef> {
    (0..N_PROJECTS)
        .map(|i| {
            let workstreams: Vec<WorkstreamDef> = (0..TASKS_PER_PROJECT)
                .map(|w| WorkstreamDef {
                    id: WorkstreamId::new(format!("ws{w}")),
                    name: format!("ws{w}"),
                    binding: None,
                })
                .collect();
            let tasks: Vec<TaskDef> = (0..TASKS_PER_PROJECT)
                .map(|w| TaskDef {
                    id: TaskId::new(format!("t{w}")),
                    title: format!("t{w}"),
                    workstream: WorkstreamId::new(format!("ws{w}")),
                    stage: "Spec".to_string(),
                    depends_on: Vec::new(),
                })
                .collect();
            ProjectDef {
                id: ProjectId::new(format!("p{i}")),
                name: format!("p{i}"),
                stages: vec!["Spec".to_string(), "Build".to_string()],
                workstreams,
                tasks,
            }
        })
        .collect()
}

/// 一個 project `p`（stages `Spec`→`Build`）、兩條 workstream `be`／`fe`、兩個 task `a`（`be`）
/// 與 `b`（`fe`），起始 stage 皆為 `Spec`。
fn sample_projects() -> Vec<ProjectDef> {
    let workstream = |id: &str| WorkstreamDef {
        id: WorkstreamId::new(id),
        name: id.to_string(),
        binding: None,
    };
    let task = |id: &str, ws: &str| TaskDef {
        id: TaskId::new(id),
        title: id.to_string(),
        workstream: WorkstreamId::new(ws),
        stage: "Spec".to_string(),
        depends_on: Vec::new(),
    };
    vec![ProjectDef {
        id: ProjectId::new("p"),
        name: "p".to_string(),
        stages: vec!["Spec".to_string(), "Build".to_string()],
        workstreams: vec![workstream("be"), workstream("fe")],
        tasks: vec![task("a", "be"), task("b", "fe")],
    }]
}

fn pane(id: &str, exited: bool) -> Pane {
    Pane {
        id: PaneId::new(id),
        workspace_id: WorkspaceId::new("w1"),
        tab_id: TabId::new("t1"),
        agent: None,
        agent_status: AgentStatus::Idle,
        title: None,
        cwd: None,
        label: None,
        focused: false,
        exited,
        updated_at: SystemTime::UNIX_EPOCH,
    }
}

/// runtime `win` 已登記、`connected`，pane 清單為 `panes`。
fn connected_store(panes: Vec<Pane>) -> RuntimeStore {
    let id = RuntimeId::new(RUNTIME);
    let mut store = RuntimeStore::new();
    store.register(id.clone(), "herdr".to_string(), "test".to_string());
    store
        .replace(
            &id,
            RuntimeSnapshot {
                server_version: "test".to_string(),
                protocol: 1,
                workspaces: Vec::new(),
                tabs: Vec::new(),
                panes,
                agents: Vec::new(),
                focused: Focused {
                    workspace_id: None,
                    tab_id: None,
                    pane_id: None,
                },
                protocol_warning: None,
            },
        )
        .expect("runtime 已登記");
    store
        .set_connection(
            &id,
            ConnectionState::Connected {
                since: SystemTime::UNIX_EPOCH,
                server_version: "test".to_string(),
                protocol: 1,
                last_snapshot_at: SystemTime::UNIX_EPOCH,
                protocol_warning: None,
            },
        )
        .expect("runtime 已登記");
    store
}

fn over(pane_id: &str) -> Override {
    Override {
        runtime: RuntimeId::new(RUNTIME),
        pane_id: PaneId::new(pane_id),
    }
}

fn pid() -> ProjectId {
    ProjectId::new("p")
}

fn mark_of(handle: &StoreHandle, task: &str) -> Mark {
    handle.with_domain(|d| d.progress[&pid()][&TaskId::new(task)].mark)
}

fn override_of(handle: &StoreHandle, workstream: &str) -> Option<Override> {
    handle.with_domain(|d| {
        d.overrides
            .get(&pid())
            .and_then(|m| m.get(&WorkstreamId::new(workstream)))
            .cloned()
    })
}

/// 從狀態檔重新載入（走 3.2 的載入路徑），用來確認寫出的檔案形狀正確、內容與記憶體一致。
fn reload(path: &Path) -> DomainState {
    let runtimes: HashSet<&str> = HashSet::from([RUNTIME]);
    progress::load_progress(path, sample_projects(), &runtimes).expect("寫出的狀態檔應可載入")
}

fn tmp_path_of(path: &Path) -> PathBuf {
    let mut name = path.file_name().expect("有檔名").to_os_string();
    name.push(".tmp");
    path.with_file_name(name)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_writes_not_lost() {
    let dir = TempDir::new("concurrent");
    let path = dir.path().join("state.json");
    let handle = StoreHandle::new_with_domain(
        RuntimeStore::new(),
        DomainState::from_projects(sample_projects()),
    );
    let service = ProgressService::new(handle.clone(), path.clone());

    // 各 50 次交錯操作：偶數次 clear、奇數次標記；第 49 次（最後一次）是標記。
    let spawn_ops = |task: &'static str, mark_op: ProgressOp| {
        let service = service.clone();
        tokio::spawn(async move {
            for i in 0..50 {
                let op = if i % 2 == 1 {
                    mark_op
                } else {
                    ProgressOp::Clear
                };
                service
                    .apply_progress(&pid(), &TaskId::new(task), op)
                    .await
                    .expect("交錯操作都應被接受");
                tokio::task::yield_now().await;
            }
        })
    };
    let a = spawn_ops("a", ProgressOp::Complete);
    let b = spawn_ops("b", ProgressOp::Fail);
    a.await.expect("task a 不應 panic");
    b.await.expect("task b 不應 panic");

    assert_eq!(mark_of(&handle, "a"), Mark::Completed);
    assert_eq!(mark_of(&handle, "b"), Mark::Failed);

    let on_disk = reload(&path);
    let in_memory = handle.with_domain(|d| d.clone());
    assert_eq!(on_disk.progress, in_memory.progress, "檔案與記憶體應一致");
    assert_eq!(on_disk.overrides, in_memory.overrides);
    assert!(!tmp_path_of(&path).exists(), "成功寫入後不應殘留 .tmp");
}

#[tokio::test]
async fn write_failure_keeps_memory() {
    let dir = TempDir::new("write-failure");
    let path = dir.path().join("state.json");
    // 目標路徑是既有目錄：`.tmp` 寫得出來，但 rename 取代目錄會失敗。
    fs::create_dir_all(&path).expect("在狀態檔路徑建立目錄");
    let handle = StoreHandle::new_with_domain(
        connected_store(vec![pane("wJ:p2", false)]),
        DomainState::from_projects(sample_projects()),
    );
    let service = ProgressService::new(handle.clone(), path.clone());

    let error = service
        .apply_progress(&pid(), &TaskId::new("a"), ProgressOp::Complete)
        .await
        .expect_err("rename 失敗應回錯");
    assert!(matches!(error, WriteError::Persist { .. }), "{error:?}");
    assert_eq!(mark_of(&handle, "a"), Mark::None, "落檔失敗記憶體不應生效");

    let error = service
        .set_override(&pid(), &WorkstreamId::new("be"), over("wJ:p2"))
        .await
        .expect_err("覆蓋落檔失敗應回錯");
    assert!(matches!(error, WriteError::Persist { .. }), "{error:?}");
    assert_eq!(
        override_of(&handle, "be"),
        None,
        "覆蓋落檔失敗記憶體不應生效"
    );
}

#[tokio::test]
async fn rename_replaces_existing_file() {
    let dir = TempDir::new("replace");
    let path = dir.path().join("state.json");
    fs::write(
        &path,
        r#"{"version": 1, "projects": {"p": {"tasks": {"a": {"stage": "Spec", "mark": "none"}}, "overrides": {}}}}"#,
    )
    .expect("寫入既有狀態檔");
    let handle = StoreHandle::new_with_domain(RuntimeStore::new(), reload(&path));
    let service = ProgressService::new(handle.clone(), path.clone());

    service
        .apply_progress(&pid(), &TaskId::new("a"), ProgressOp::Advance)
        .await
        .expect("推進應成功並覆蓋既有檔");

    let on_disk = reload(&path);
    assert_eq!(on_disk.progress[&pid()][&TaskId::new("a")].stage, "Build");
    assert!(!tmp_path_of(&path).exists(), "成功寫入後不應殘留 .tmp");
    assert_eq!(
        handle.with_domain(|d| d.progress[&pid()][&TaskId::new("a")].stage.clone()),
        "Build"
    );
}

/// 投影任務偵測到失效覆蓋（runtime connected、pane 不存在）→ 經 channel 交給寫入服務 → 記憶體
/// 刪除並落檔（design D3）。
#[tokio::test]
async fn stale_override_removed_and_persisted() {
    let dir = TempDir::new("stale");
    let path = dir.path().join("state.json");
    let mut domain = DomainState::from_projects(sample_projects());
    domain.overrides.insert(
        pid(),
        HashMap::from([
            (WorkstreamId::new("be"), over("wJ:gone")),
            (WorkstreamId::new("fe"), over("wJ:alive")),
        ]),
    );
    let handle =
        StoreHandle::new_with_domain(connected_store(vec![pane("wJ:alive", false)]), domain);
    let service = ProgressService::new(handle.clone(), path.clone());

    let (stale_tx, stale_rx) = mpsc::unbounded_channel();
    let remover = service.spawn_stale_remover(stale_rx);
    let projector = spawn_projector_with_stale_sink(handle.clone(), stale_tx);
    // 觸發一次投影。
    handle.set_domain(handle.with_domain(|d| d.clone()));

    wait_until(|| override_of(&handle, "be").is_none()).await;
    wait_until(|| path.exists()).await;

    assert_eq!(
        override_of(&handle, "fe"),
        Some(over("wJ:alive")),
        "未失效的覆蓋不動"
    );
    let on_disk = reload(&path);
    let overrides = on_disk.overrides.get(&pid()).expect("fe 覆蓋仍在檔案中");
    assert!(
        !overrides.contains_key(&WorkstreamId::new("be")),
        "失效覆蓋應從檔案刪除"
    );
    assert_eq!(
        overrides.get(&WorkstreamId::new("fe")),
        Some(&over("wJ:alive"))
    );

    projector.abort();
    remover.abort();
}

/// design D3 裁決：失效覆蓋刪除落檔失敗時記 error，記憶體照樣刪除。
#[tokio::test]
async fn stale_override_removed_from_memory_even_if_persist_fails() {
    let dir = TempDir::new("stale-persist-fail");
    let path = dir.path().join("state.json");
    fs::create_dir_all(&path).expect("在狀態檔路徑建立目錄");
    let mut domain = DomainState::from_projects(sample_projects());
    domain.overrides.insert(
        pid(),
        HashMap::from([(WorkstreamId::new("be"), over("wJ:gone"))]),
    );
    let handle = StoreHandle::new_with_domain(connected_store(Vec::new()), domain);
    let service = ProgressService::new(handle.clone(), path.clone());

    service
        .remove_stale(vec![StaleOverride {
            project: pid(),
            workstream: WorkstreamId::new("be"),
            override_: over("wJ:gone"),
        }])
        .await;

    assert_eq!(override_of(&handle, "be"), None, "落檔失敗記憶體仍應刪除");
    assert!(path.is_dir(), "目標目錄不應被動到");
}

/// design D3：收到失效通知時覆蓋已被使用者改綁（與通知內容不同）→ 略過，不刪新覆蓋。
#[tokio::test]
async fn stale_override_skipped_when_override_changed() {
    let dir = TempDir::new("stale-changed");
    let path = dir.path().join("state.json");
    let mut domain = DomainState::from_projects(sample_projects());
    domain.overrides.insert(
        pid(),
        HashMap::from([(WorkstreamId::new("be"), over("wJ:new"))]),
    );
    let handle = StoreHandle::new_with_domain(connected_store(Vec::new()), domain);
    let service = ProgressService::new(handle.clone(), path.clone());

    service
        .remove_stale(vec![StaleOverride {
            project: pid(),
            workstream: WorkstreamId::new("be"),
            override_: over("wJ:old"),
        }])
        .await;

    assert_eq!(override_of(&handle, "be"), Some(over("wJ:new")));
    assert!(!path.exists(), "沒有變動不應落檔");
}

#[tokio::test]
async fn unknown_targets_are_not_found() {
    let dir = TempDir::new("not-found");
    let path = dir.path().join("state.json");
    let handle = StoreHandle::new_with_domain(
        connected_store(vec![pane("wJ:p2", false)]),
        DomainState::from_projects(sample_projects()),
    );
    let service = ProgressService::new(handle.clone(), path.clone());

    let error = service
        .apply_progress(
            &ProjectId::new("nope"),
            &TaskId::new("a"),
            ProgressOp::Complete,
        )
        .await
        .expect_err("未知 project");
    assert!(matches!(error, WriteError::UnknownProject(_)), "{error:?}");

    let error = service
        .apply_progress(&pid(), &TaskId::new("nope"), ProgressOp::Complete)
        .await
        .expect_err("未知 task");
    assert!(matches!(error, WriteError::UnknownTask(_)), "{error:?}");

    let error = service
        .set_override(&pid(), &WorkstreamId::new("nope"), over("wJ:p2"))
        .await
        .expect_err("未知 workstream");
    assert!(
        matches!(error, WriteError::UnknownWorkstream(_)),
        "{error:?}"
    );

    let error = service
        .clear_override(&ProjectId::new("nope"), &WorkstreamId::new("be"))
        .await
        .expect_err("取消覆蓋的未知 project");
    assert!(matches!(error, WriteError::UnknownProject(_)), "{error:?}");

    assert!(!path.exists(), "錯誤不應落檔");
}

#[tokio::test]
async fn rejections_do_not_persist() {
    let dir = TempDir::new("rejected");
    let path = dir.path().join("state.json");
    let handle = StoreHandle::new_with_domain(
        connected_store(vec![pane("wJ:dead", true)]),
        DomainState::from_projects(sample_projects()),
    );
    let service = ProgressService::new(handle.clone(), path.clone());

    // 起始 Spec → 推進到 Build（最後一站）會落檔；先刪掉檔案，再確認被拒絕的推進不重建檔案。
    service
        .apply_progress(&pid(), &TaskId::new("a"), ProgressOp::Advance)
        .await
        .expect("第一次推進應成功");
    fs::remove_file(&path).expect("刪除狀態檔");

    let error = service
        .apply_progress(&pid(), &TaskId::new("a"), ProgressOp::Advance)
        .await
        .expect_err("最後一站推進應被拒絕");
    assert!(
        matches!(error, WriteError::Rejected(Rejection::AlreadyLastStage)),
        "{error:?}"
    );

    let error = service
        .set_override(&pid(), &WorkstreamId::new("be"), over("wJ:dead"))
        .await
        .expect_err("exited pane 應被拒絕");
    assert!(
        matches!(error, WriteError::Rejected(Rejection::PaneExited)),
        "{error:?}"
    );
    assert_eq!(override_of(&handle, "be"), None);

    assert!(!path.exists(), "被拒絕的操作不應落檔");
}

/// design D4：clear 在標記已是 none、取消不存在的覆蓋，都算出相同狀態——成功、不落檔、不通知。
#[tokio::test]
async fn unchanged_state_is_not_persisted() {
    let dir = TempDir::new("unchanged");
    let path = dir.path().join("state.json");
    let handle = StoreHandle::new_with_domain(
        RuntimeStore::new(),
        DomainState::from_projects(sample_projects()),
    );
    let service = ProgressService::new(handle.clone(), path.clone());

    service
        .apply_progress(&pid(), &TaskId::new("a"), ProgressOp::Clear)
        .await
        .expect("clear none 應成功");
    service
        .clear_override(&pid(), &WorkstreamId::new("be"))
        .await
        .expect("取消不存在的覆蓋應成功");

    assert!(!path.exists(), "狀態沒變不應落檔");
}

#[tokio::test]
async fn set_and_clear_override_persist() {
    let dir = TempDir::new("override");
    let path = dir.path().join("state.json");
    let handle = StoreHandle::new_with_domain(
        connected_store(vec![pane("wJ:p2", false)]),
        DomainState::from_projects(sample_projects()),
    );
    let service = ProgressService::new(handle.clone(), path.clone());

    service
        .set_override(&pid(), &WorkstreamId::new("be"), over("wJ:p2"))
        .await
        .expect("設定覆蓋應成功");
    assert_eq!(override_of(&handle, "be"), Some(over("wJ:p2")));
    assert_eq!(
        reload(&path).overrides[&pid()].get(&WorkstreamId::new("be")),
        Some(&over("wJ:p2"))
    );

    service
        .clear_override(&pid(), &WorkstreamId::new("be"))
        .await
        .expect("取消覆蓋應成功");
    assert_eq!(override_of(&handle, "be"), None);
    assert!(reload(&path).overrides.is_empty(), "檔案中的覆蓋應被刪除");
}

/// M4：`to_state_file` 序列化前若把 map 收進 `HashMap`，key 順序依雜湊而非排序，同一份 domain
/// 內容兩次獨立寫出（各自從頭建置 handle／service）在位元組上可能不同——這會讓部署／備份拿
/// 「內容沒變」的雜湊比對誤判為變更。用 `wide_projects()` 的寬度讓差異幾乎必然出現。
#[tokio::test]
async fn same_domain_written_twice_produces_identical_bytes() {
    async fn write_wide_domain(tag: &str) -> (TempDir, PathBuf) {
        let dir = TempDir::new(tag);
        let path = dir.path().join("state.json");
        let panes = (0..N_PROJECTS)
            .map(|i| pane(&format!("wJ:p{i}"), false))
            .collect();
        let handle = StoreHandle::new_with_domain(
            connected_store(panes),
            DomainState::from_projects(wide_projects()),
        );
        let service = ProgressService::new(handle.clone(), path.clone());

        for i in 0..N_PROJECTS {
            let project_id = ProjectId::new(format!("p{i}"));
            for w in 0..TASKS_PER_PROJECT {
                let ws = WorkstreamId::new(format!("ws{w}"));
                service
                    .set_override(&project_id, &ws, over(&format!("wJ:p{i}")))
                    .await
                    .expect("設定覆蓋應成功");
                service
                    .apply_progress(
                        &project_id,
                        &TaskId::new(format!("t{w}")),
                        ProgressOp::Advance,
                    )
                    .await
                    .expect("推進應成功");
            }
        }
        (dir, path)
    }

    let (dir_a, path_a) = write_wide_domain("wide-a").await;
    let (dir_b, path_b) = write_wide_domain("wide-b").await;

    let bytes_a = fs::read(&path_a).expect("讀出第一份狀態檔");
    let bytes_b = fs::read(&path_b).expect("讀出第二份狀態檔");
    assert_eq!(
        bytes_a, bytes_b,
        "同一 domain 內容寫兩次，位元組應完全相同（key 順序不可依賴 HashMap 雜湊）"
    );

    drop(dir_a);
    drop(dir_b);
}

async fn wait_until(mut condition: impl FnMut() -> bool) {
    tokio::time::timeout(Duration::from_secs(5), async {
        while !condition() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("等待條件逾時");
}

/// Codex fix round 1：呼叫端在落檔進行中被取消（axum 在客戶端斷線時 drop handler future），
/// 交易仍須完整跑完並持鎖到結束——下一筆寫入不得與仍在跑的 IO 重疊，最終磁碟與記憶體一致且
/// 含兩筆結果。
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn cancelled_caller_does_not_break_serialization() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    let dir = TempDir::new("cancel");
    let path = dir.path().join("state.json");
    let handle = StoreHandle::new_with_domain(
        RuntimeStore::new(),
        DomainState::from_projects(sample_projects()),
    );

    let (started_tx, started_rx) = std::sync::mpsc::channel::<()>();
    let (release_tx, release_rx) = std::sync::mpsc::channel::<()>();
    let started_tx = Mutex::new(started_tx);
    let release_rx = Mutex::new(release_rx);
    let starts = Arc::new(AtomicUsize::new(0));
    let in_flight = Arc::new(AtomicUsize::new(0));
    let max_in_flight = Arc::new(AtomicUsize::new(0));
    let hook: cockpit::progress_service::WriteHook = {
        let (starts, in_flight, max_in_flight) =
            (starts.clone(), in_flight.clone(), max_in_flight.clone());
        Arc::new(move |stage| match stage {
            cockpit::progress_service::WriteStage::Start => {
                let now = in_flight.fetch_add(1, Ordering::SeqCst) + 1;
                max_in_flight.fetch_max(now, Ordering::SeqCst);
                if starts.fetch_add(1, Ordering::SeqCst) == 0 {
                    // 第一次落檔：通知測試並卡住，直到測試放行。
                    started_tx.lock().unwrap().send(()).unwrap();
                    release_rx.lock().unwrap().recv().unwrap();
                }
            }
            cockpit::progress_service::WriteStage::Finish => {
                in_flight.fetch_sub(1, Ordering::SeqCst);
            }
        })
    };
    let service = ProgressService::with_write_hook(handle.clone(), path.clone(), hook);

    let first = tokio::spawn({
        let service = service.clone();
        async move {
            service
                .apply_progress(&pid(), &TaskId::new("a"), ProgressOp::Complete)
                .await
        }
    });
    tokio::task::spawn_blocking(move || started_rx.recv())
        .await
        .expect("等待執行緒不應 panic")
        .expect("第一筆應開始落檔");

    // 落檔進行中取消呼叫端，立刻送第二筆。
    first.abort();
    let second = tokio::spawn({
        let service = service.clone();
        async move {
            service
                .apply_progress(&pid(), &TaskId::new("b"), ProgressOp::Fail)
                .await
        }
    });
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(
        starts.load(Ordering::SeqCst),
        1,
        "第一筆落檔未結束前，第二筆不應開始 IO"
    );

    release_tx.send(()).expect("放行第一筆落檔");
    second
        .await
        .expect("第二筆不應 panic")
        .expect("第二筆應成功");

    assert_eq!(max_in_flight.load(Ordering::SeqCst), 1, "兩次 IO 不應重疊");
    assert_eq!(starts.load(Ordering::SeqCst), 2);
    assert_eq!(
        mark_of(&handle, "a"),
        Mark::Completed,
        "被取消的呼叫端交易仍應生效"
    );
    assert_eq!(mark_of(&handle, "b"), Mark::Failed);
    let on_disk = reload(&path);
    let in_memory = handle.with_domain(|d| d.clone());
    assert_eq!(on_disk.progress, in_memory.progress, "檔案與記憶體應一致");
}
