//! openspec-stage-sync task 4.4 驗收測試：背景偵測工作 `OpenSpecSync`（design D5、D10-4、D10-6；spec
//! `openspec-stage-sync`「偵測的時機與去重」「偵測不得喚醒 WSL 或查詢未連線的 runtime」「偵測全程唯讀」）。
//!
//! 除最後的真實 git 唯讀驗證外，全部用 tokio 暫停時間與假實作（pane 清單、分支查詢、讀檔、WSL 探測、輸出端）。
//! 「一輪」的觀測點：工作啟動當下跑第一輪，之後每 [`SYNC_INTERVAL`] 一輪；假實作都不耗時，暫停的時鐘只有在工作
//! 閒置時才前進，所以測試睡到「第 n 輪的時間點再多 1 ms」時，第 n 輪一定已跑完。

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use cockpit::openspec_sync_job::{
    BranchLookup, DETECT_TIMEOUT, FsOpenSpecReader, GitBranchLookup, OpenSpecReader, OpenSpecSink,
    SYNC_INTERVAL, SyncPane, SyncPanes, spawn_openspec_sync,
};
use cockpit::progress_service::{OpenSpecEntry, ProgressService, WriteError};
use cockpit::repo_resolver::{DISTRO_PROBE_TIMEOUT, RunningDistros};
use cockpit_core::{
    ConnectionState, DomainState, Observation, OpenSpecPhase, PaneId, PaneRepo, PinnedPane,
    ProjectDef, ProjectId, RepoKey, RepoProjectDef, RuntimeId, RuntimeStore, StoreHandle, TaskDef,
    TaskId, WorkstreamDef, WorkstreamId,
};
use cockpit_git::GitTarget;
use tokio::sync::Notify;
use tokio::task::JoinHandle;

// ---------------------------------------------------------------------------
// 假實作
// ---------------------------------------------------------------------------

/// 可在測試中途改寫的 pane 清單，兼當假的 domain：`current`（domain 中的偵測結果）由 [`RecordingSink`] 送出成功時
/// 更新。
#[derive(Clone, Default)]
struct FakePanes(Arc<Mutex<Vec<SyncPane>>>);

impl FakePanes {
    /// 換上新的 pane 清單（`current` 欄位忽略）。比照 `set_pane_repos`：歸類沒變的 pane 保留目前的偵測結果，
    /// 歸類變了或新出現的 pane 沒有結果。
    fn set(&self, panes: Vec<SyncPane>) {
        let mut guard = self.0.lock().unwrap();
        let next = panes
            .into_iter()
            .map(|mut pane| {
                pane.current = guard
                    .iter()
                    .find(|old| old.pane == pane.pane && old.location.same_location(&pane.location))
                    .and_then(|old| old.current.clone());
                pane
            })
            .collect();
        *guard = next;
    }

    /// 比照 `sync_openspec`：整張表取代 `openspec_obs`，歸類不符的那筆當無結果。
    fn apply(&self, entries: &[OpenSpecEntry]) {
        for pane in self.0.lock().unwrap().iter_mut() {
            pane.current = entries
                .iter()
                .find(|e| e.pane == pane.pane && e.location.same_location(&pane.location))
                .and_then(|e| e.observation.clone());
        }
    }

    fn current(&self, id: &str) -> Option<Observation> {
        self.0
            .lock()
            .unwrap()
            .iter()
            .find(|pane| pane.pane == key(id))
            .and_then(|pane| pane.current.clone())
    }
}

impl SyncPanes for FakePanes {
    fn panes(&self) -> Vec<SyncPane> {
        self.0.lock().unwrap().clone()
    }
}

/// 分支查詢結果：`Ok(Some)` 分支、`Ok(None)` detached、`Err` 查詢出錯。
type BranchOutcome = Result<Option<String>, String>;

/// 依 `GitTarget` 回預先設定的分支查詢結果；沒設定的回 `Ok(None)`（detached）。記下每一次查詢。
#[derive(Clone, Default)]
struct FakeBranches(Arc<FakeBranchesInner>);

#[derive(Default)]
struct FakeBranchesInner {
    outcomes: Mutex<Vec<(GitTarget, BranchOutcome)>>,
    calls: Mutex<Vec<GitTarget>>,
}

impl FakeBranches {
    fn set(&self, target: GitTarget, outcome: BranchOutcome) {
        let mut outcomes = self.0.outcomes.lock().unwrap();
        outcomes.retain(|(t, _)| *t != target);
        outcomes.push((target, outcome));
    }

    fn calls(&self) -> usize {
        self.0.calls.lock().unwrap().len()
    }

    fn targets(&self) -> Vec<GitTarget> {
        self.0.calls.lock().unwrap().clone()
    }
}

impl BranchLookup for FakeBranches {
    async fn current_branch(&self, target: &GitTarget) -> Result<Option<String>, String> {
        self.0.calls.lock().unwrap().push(target.clone());
        self.0
            .outcomes
            .lock()
            .unwrap()
            .iter()
            .find(|(t, _)| t == target)
            .map_or(Ok(None), |(_, outcome)| outcome.clone())
    }
}

type DetectHook = Box<dyn FnOnce() -> JoinHandle<()> + Send>;

/// 依根目錄回預先設定的偵測結果；沒設定的回 `None`。記下每一次 `(根目錄, 分支)`。
#[derive(Clone, Default)]
struct FakeReader(Arc<FakeReaderInner>);

#[derive(Default)]
struct FakeReaderInner {
    results: Mutex<HashMap<String, Option<Observation>>>,
    calls: Mutex<Vec<(String, Option<String>)>>,
    /// 下一次偵測時執行一次、並等它完成的副作用（模擬偵測期間 pane 改歸類）。
    on_next_detect: Mutex<Option<DetectHook>>,
    /// 讀檔卡住的根目錄：偵測時等到 [`FakeReader::release`] 才返回（模擬 `\\wsl.localhost` 卡在 9P）。
    stalled: Mutex<HashMap<String, Arc<Notify>>>,
}

impl FakeReader {
    fn set(&self, root: &str, obs: Option<Observation>) {
        self.0.results.lock().unwrap().insert(s(root), obs);
    }

    fn calls(&self) -> Vec<(String, Option<String>)> {
        self.0.calls.lock().unwrap().clone()
    }

    fn on_next_detect(&self, effect: impl FnOnce() -> JoinHandle<()> + Send + 'static) {
        *self.0.on_next_detect.lock().unwrap() = Some(Box::new(effect));
    }

    /// 之後對 `root` 的偵測都卡住，直到 [`FakeReader::release`]。
    fn stall(&self, root: &str) {
        self.0
            .stalled
            .lock()
            .unwrap()
            .insert(s(root), Arc::new(Notify::new()));
    }

    /// 讓卡住的偵測返回；之後對 `root` 的偵測不再卡住。
    fn release(&self, root: &str) {
        if let Some(gate) = self.0.stalled.lock().unwrap().remove(root) {
            gate.notify_one();
        }
    }

    fn calls_for(&self, root: &str) -> usize {
        self.calls().iter().filter(|(r, _)| r == root).count()
    }
}

impl OpenSpecReader for FakeReader {
    async fn detect(&self, root: &str, branch: Option<String>) -> Option<Observation> {
        self.0.calls.lock().unwrap().push((s(root), branch));
        let effect = self.0.on_next_detect.lock().unwrap().take();
        if let Some(effect) = effect {
            effect().await.expect("副作用不 panic");
        }
        let gate = self.0.stalled.lock().unwrap().get(root).cloned();
        if let Some(gate) = gate {
            gate.notified().await;
        }
        self.0.results.lock().unwrap().get(root).cloned().flatten()
    }
}

/// 假的「正在執行的 WSL 發行版」清單。記下探測次數。
#[derive(Clone)]
struct FakeDistros(Arc<FakeDistrosInner>);

struct FakeDistrosInner {
    running: Mutex<Vec<String>>,
    probes: Mutex<usize>,
    /// 優先於 `running`：依序回給接下來的探測，用完才回 `running`。
    script: Mutex<VecDeque<Vec<String>>>,
    /// 探測本身壞掉時的行為（`None`＝正常回清單）。
    broken: Mutex<Option<ProbeFault>>,
    /// 第 N 次探測（從 1 起算）執行失敗，其餘照常；`None`＝不用。
    fail_at: Mutex<Option<usize>>,
}

/// 探測本身壞掉的兩種樣子。
#[derive(Clone, Copy)]
enum ProbeFault {
    /// `wsl.exe --list` 卡住，永遠不返回。
    Hang,
    /// 探測執行失敗（`running()` 回 `None`）。
    Fail,
}

impl FakeDistros {
    fn new(running: &[&str]) -> Self {
        Self(Arc::new(FakeDistrosInner {
            running: Mutex::new(running.iter().map(|n| s(n)).collect()),
            probes: Mutex::new(0),
            script: Mutex::new(VecDeque::new()),
            broken: Mutex::new(None),
            fail_at: Mutex::new(None),
        }))
    }

    /// 第 `nth` 次探測（從 1 起算，含已發生的）執行失敗，其餘照常。
    fn fail_probe_number(&self, nth: usize) {
        *self.0.fail_at.lock().unwrap() = Some(nth);
    }

    /// 之後的探測都以 `fault` 的方式壞掉；`None` 恢復正常。
    fn break_with(&self, fault: Option<ProbeFault>) {
        *self.0.broken.lock().unwrap() = fault;
    }

    /// 接下來的探測依序回 `answers`，用完才回 [`FakeDistros::set`] 設的清單。
    fn script(&self, answers: &[&[&str]]) {
        *self.0.script.lock().unwrap() = answers
            .iter()
            .map(|names| names.iter().map(|n| s(n)).collect())
            .collect();
    }

    fn set(&self, running: &[&str]) {
        *self.0.running.lock().unwrap() = running.iter().map(|n| s(n)).collect();
    }

    fn probes(&self) -> usize {
        *self.0.probes.lock().unwrap()
    }
}

impl RunningDistros for FakeDistros {
    async fn running(&self) -> Option<Vec<String>> {
        let number = {
            let mut probes = self.0.probes.lock().unwrap();
            *probes += 1;
            *probes
        };
        if *self.0.fail_at.lock().unwrap() == Some(number) {
            return None;
        }
        let fault = *self.0.broken.lock().unwrap();
        match fault {
            Some(ProbeFault::Hang) => std::future::pending::<()>().await,
            Some(ProbeFault::Fail) => return None,
            None => {}
        }
        let scripted = self.0.script.lock().unwrap().pop_front();
        Some(scripted.unwrap_or_else(|| self.0.running.lock().unwrap().clone()))
    }
}

/// 記下每一次送出的表；`fail_next` 次數內回落檔失敗（假 domain 不變），成功時套用到假 domain。
#[derive(Clone)]
struct RecordingSink(Arc<RecordingSinkInner>);

struct RecordingSinkInner {
    domain: FakePanes,
    submits: Mutex<Vec<Vec<OpenSpecEntry>>>,
    fail_next: Mutex<usize>,
}

impl RecordingSink {
    fn new(domain: FakePanes) -> Self {
        Self(Arc::new(RecordingSinkInner {
            domain,
            submits: Mutex::new(Vec::new()),
            fail_next: Mutex::new(0),
        }))
    }

    fn submits(&self) -> Vec<Vec<OpenSpecEntry>> {
        self.0.submits.lock().unwrap().clone()
    }

    fn fail_next(&self, times: usize) {
        *self.0.fail_next.lock().unwrap() = times;
    }
}

impl OpenSpecSink for RecordingSink {
    async fn submit(&self, entries: Vec<OpenSpecEntry>) -> Result<(), WriteError> {
        self.0.submits.lock().unwrap().push(entries.clone());
        {
            let mut fail = self.0.fail_next.lock().unwrap();
            if *fail > 0 {
                *fail -= 1;
                return Err(WriteError::Persist {
                    path: PathBuf::from("state.json"),
                    source: std::io::Error::other("模擬落檔失敗"),
                });
            }
        }
        self.0.domain.apply(&entries);
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 組裝
// ---------------------------------------------------------------------------

fn s(v: &str) -> String {
    v.to_string()
}

const APP_REPO: &str = r"d:\work\app\.git";
const APP_ROOT: &str = r"D:\work\app";
const WSL_ROOT: &str = r"\\wsl.localhost\Ubuntu\home\u\app";

fn key(pane: &str) -> (RuntimeId, PaneId) {
    (RuntimeId::new("local"), PaneId::new(pane))
}

fn location(root: Option<&str>) -> PaneRepo {
    PaneRepo {
        repo: RepoKey::new(APP_REPO),
        default_name: s("app"),
        worktree: None,
        root: root.map(s),
    }
}

fn pane(id: &str, root: Option<&str>, connected: bool) -> SyncPane {
    SyncPane {
        pane: key(id),
        location: location(root),
        connected,
        current: None,
    }
}

fn native(path: &str) -> GitTarget {
    GitTarget::Native { path: s(path) }
}

fn wsl_target() -> GitTarget {
    GitTarget::Wsl {
        distro: s("Ubuntu"),
        posix: s("/home/u/app"),
    }
}

fn implement(checked: u32) -> Observation {
    Observation {
        change: s("foo"),
        phase: OpenSpecPhase::Implement,
        checked,
        total: 8,
    }
}

fn entry(id: &str, root: Option<&str>, obs: Option<Observation>) -> OpenSpecEntry {
    OpenSpecEntry {
        pane: key(id),
        location: location(root),
        observation: obs,
    }
}

struct Harness {
    panes: FakePanes,
    branches: FakeBranches,
    reader: FakeReader,
    distros: FakeDistros,
    sink: RecordingSink,
    job: JoinHandle<()>,
}

impl Harness {
    /// 起工作並等第一輪（啟動當下）跑完。
    async fn start(panes: Vec<SyncPane>, distros: FakeDistros) -> Self {
        let harness = Self::spawn(panes, distros, |_| {});
        tokio::time::sleep(Duration::from_millis(1)).await;
        harness
    }

    /// 起工作但不等；`setup` 可在第一輪之前設定假實作。
    fn spawn(panes: Vec<SyncPane>, distros: FakeDistros, setup: impl FnOnce(&Self)) -> Self {
        let fake_panes = FakePanes::default();
        fake_panes.set(panes);
        let branches = FakeBranches::default();
        let reader = FakeReader::default();
        let sink = RecordingSink::new(fake_panes.clone());
        let mut harness = Self {
            panes: fake_panes,
            branches,
            reader,
            distros,
            sink,
            job: tokio::spawn(async {}),
        };
        setup(&harness);
        harness.job = spawn_openspec_sync(
            harness.panes.clone(),
            harness.branches.clone(),
            harness.reader.clone(),
            harness.distros.clone(),
            harness.sink.clone(),
        );
        harness
    }

    /// 等下一輪跑完。
    async fn next_round(&self) {
        tokio::time::sleep(SYNC_INTERVAL).await;
    }
}

impl Drop for Harness {
    fn drop(&mut self) {
        self.job.abort();
    }
}

// ---------------------------------------------------------------------------
// 正常送出、去重
// ---------------------------------------------------------------------------

/// spec「Windows 路徑的 worktree 不受 WSL 防護影響」：所有發行版都停止，Windows worktree 照常查詢；送出該 pane 的
/// 偵測結果（附偵測當下的歸類），讀檔拿到的是查到的分支。
#[tokio::test(start_paused = true)]
async fn windows_worktree_is_detected_and_submitted() {
    let h = Harness::spawn(
        vec![pane("wJ:p1", Some(APP_ROOT), true)],
        FakeDistros::new(&[]),
        |h| {
            h.branches.set(native(APP_ROOT), Ok(Some(s("feat/foo"))));
            h.reader.set(APP_ROOT, Some(implement(3)));
        },
    );
    tokio::time::sleep(Duration::from_millis(1)).await;

    assert_eq!(
        h.sink.submits(),
        vec![vec![entry("wJ:p1", Some(APP_ROOT), Some(implement(3)))]]
    );
    assert_eq!(h.reader.calls(), vec![(s(APP_ROOT), Some(s("feat/foo")))]);
    assert_eq!(h.distros.probes(), 0, "Windows 路徑不需探測 WSL");
}

/// spec「結果沒變不更新投影」：連續數輪結果相同，只送第一次。
#[tokio::test(start_paused = true)]
async fn unchanged_result_is_not_resent() {
    let h = Harness::spawn(
        vec![pane("wJ:p1", Some(APP_ROOT), true)],
        FakeDistros::new(&[]),
        |h| h.reader.set(APP_ROOT, Some(implement(3))),
    );
    tokio::time::sleep(Duration::from_millis(1)).await;
    for _ in 0..3 {
        h.next_round().await;
    }

    assert_eq!(h.reader.calls().len(), 4, "每輪都偵測");
    assert_eq!(h.sink.submits().len(), 1, "結果相同只送一次");
}

/// spec「約 10 秒內反映勾選變化」：結果變了，下一輪送出新表。
#[tokio::test(start_paused = true)]
async fn changed_result_is_sent_next_round() {
    let h = Harness::spawn(
        vec![pane("wJ:p1", Some(APP_ROOT), true)],
        FakeDistros::new(&[]),
        |h| h.reader.set(APP_ROOT, Some(implement(3))),
    );
    tokio::time::sleep(Duration::from_millis(1)).await;

    h.reader.set(APP_ROOT, Some(implement(4)));
    h.next_round().await;

    assert_eq!(
        h.sink.submits().last(),
        Some(&vec![entry("wJ:p1", Some(APP_ROOT), Some(implement(4)))])
    );
    assert_eq!(h.sink.submits().len(), 2);
}

/// Task 4.3 carry：送出失敗不得記為已送出——下一輪結果相同也要重送，成功後才停。
#[tokio::test(start_paused = true)]
async fn failed_submit_is_retried_next_round() {
    let h = Harness::spawn(
        vec![pane("wJ:p1", Some(APP_ROOT), true)],
        FakeDistros::new(&[]),
        |h| {
            h.reader.set(APP_ROOT, Some(implement(3)));
            h.sink.fail_next(1);
        },
    );
    tokio::time::sleep(Duration::from_millis(1)).await;
    assert_eq!(h.sink.submits().len(), 1);

    h.next_round().await;
    assert_eq!(h.sink.submits().len(), 2, "失敗的那一輪下一輪重送");
    assert_eq!(h.sink.submits()[0], h.sink.submits()[1]);

    h.next_round().await;
    assert_eq!(h.sink.submits().len(), 2, "重送成功後不再送");
}

// ---------------------------------------------------------------------------
// 分組、沒有根目錄、查詢錯誤
// ---------------------------------------------------------------------------

/// spec「同一個 worktree 只查詢一次」：3 個 pane 同一個根目錄，分支查詢與讀檔各一次，3 個 pane 都得到結果。
#[tokio::test(start_paused = true)]
async fn shared_worktree_is_queried_once() {
    let h = Harness::spawn(
        vec![
            pane("wJ:p1", Some(APP_ROOT), true),
            pane("wJ:p2", Some(APP_ROOT), true),
            pane("wJ:p3", Some(APP_ROOT), true),
        ],
        FakeDistros::new(&[]),
        |h| h.reader.set(APP_ROOT, Some(implement(3))),
    );
    tokio::time::sleep(Duration::from_millis(1)).await;

    assert_eq!(h.branches.calls(), 1);
    assert_eq!(h.reader.calls().len(), 1);
    assert_eq!(
        h.sink.submits(),
        vec![vec![
            entry("wJ:p1", Some(APP_ROOT), Some(implement(3))),
            entry("wJ:p2", Some(APP_ROOT), Some(implement(3))),
            entry("wJ:p3", Some(APP_ROOT), Some(implement(3))),
        ]]
    );
}

/// spec「沒有 worktree 根目錄的 pane 不偵測」：不查詢，表中該 pane 為無結果。
#[tokio::test(start_paused = true)]
async fn pane_without_root_is_not_queried() {
    let h = Harness::start(vec![pane("wJ:p1", None, true)], FakeDistros::new(&[])).await;

    assert_eq!(h.branches.calls(), 0);
    assert!(h.reader.calls().is_empty());
    assert_eq!(h.distros.probes(), 0);
    assert!(
        h.sink.submits().is_empty(),
        "無結果且 domain 也沒有結果：沒有差異不送"
    );
    assert_eq!(h.panes.current("wJ:p1"), None);
}

/// design D10-4、Task 4.2 carry：分支查詢出錯時不讀檔（不可把錯誤當 detached），該 worktree 為無結果——原本有結果的
/// pane 送出無結果。
#[tokio::test(start_paused = true)]
async fn branch_error_submits_none_without_detecting() {
    let h = Harness::spawn(
        vec![pane("wJ:p1", Some(APP_ROOT), true)],
        FakeDistros::new(&[]),
        |h| {
            h.branches.set(native(APP_ROOT), Ok(Some(s("foo"))));
            h.reader.set(APP_ROOT, Some(implement(3)));
        },
    );
    tokio::time::sleep(Duration::from_millis(1)).await;
    assert_eq!(h.panes.current("wJ:p1"), Some(implement(3)));

    h.branches.set(native(APP_ROOT), Err(s("逾時")));
    h.next_round().await;

    assert_eq!(h.branches.calls(), 2);
    assert_eq!(h.reader.calls().len(), 1, "查詢出錯不得呼叫 detect");
    assert_eq!(
        h.sink.submits().last(),
        Some(&vec![entry("wJ:p1", Some(APP_ROOT), None)])
    );
    assert_eq!(h.panes.current("wJ:p1"), None);
}

// ---------------------------------------------------------------------------
// 防護：未連線 runtime、WSL 發行版未在執行
// ---------------------------------------------------------------------------

/// spec「runtime 未連線」：不查詢；先前的偵測結果沿用、不重送（卡片與同步標示不變）。
#[tokio::test(start_paused = true)]
async fn disconnected_runtime_is_not_queried_and_keeps_previous_result() {
    let h = Harness::spawn(
        vec![pane("wJ:p1", Some(APP_ROOT), true)],
        FakeDistros::new(&[]),
        |h| h.reader.set(APP_ROOT, Some(implement(3))),
    );
    tokio::time::sleep(Duration::from_millis(1)).await;
    assert_eq!(h.branches.calls(), 1);

    h.panes.set(vec![pane("wJ:p1", Some(APP_ROOT), false)]);
    h.reader.set(APP_ROOT, Some(implement(5)));
    for _ in 0..3 {
        h.next_round().await;
    }

    assert_eq!(h.branches.calls(), 1, "未連線不查分支");
    assert_eq!(h.reader.calls().len(), 1, "未連線不讀檔");
    assert_eq!(h.sink.submits().len(), 1, "沿用上一輪結果，表沒變不重送");
}

/// spec「剛啟動時被跳過的 worktree 沒有偵測結果」（runtime 未連線版）：沒有上一輪結果時送無結果。
#[tokio::test(start_paused = true)]
async fn disconnected_runtime_at_startup_has_no_result() {
    let h = Harness::start(
        vec![pane("wJ:p1", Some(APP_ROOT), false)],
        FakeDistros::new(&[]),
    )
    .await;

    assert_eq!(h.branches.calls(), 0);
    assert!(h.reader.calls().is_empty());
    assert!(h.sink.submits().is_empty(), "沒有結果，與 domain 相同");
    assert_eq!(h.panes.current("wJ:p1"), None);
}

/// spec「WSL 發行版沒在執行」「恢復後接續偵測」：發行版停止期間數輪都不查分支、不讀檔，沿用上一輪結果；
/// 恢復執行後下一輪偵測到最新結果並送出。
#[tokio::test(start_paused = true)]
async fn stopped_distro_is_not_queried_and_resumes_after_restart() {
    let h = Harness::spawn(
        vec![pane("w1:p1", Some(WSL_ROOT), true)],
        FakeDistros::new(&["Ubuntu"]),
        |h| {
            h.branches.set(wsl_target(), Ok(Some(s("foo"))));
            h.reader.set(WSL_ROOT, Some(implement(3)));
        },
    );
    tokio::time::sleep(Duration::from_millis(1)).await;
    assert_eq!(h.branches.calls(), 1);
    assert_eq!(h.sink.submits().len(), 1);

    h.distros.set(&[]);
    h.reader.set(WSL_ROOT, Some(implement(8)));
    for _ in 0..3 {
        h.next_round().await;
    }
    assert_eq!(h.branches.calls(), 1, "發行版停止時不得執行 git");
    assert_eq!(h.reader.calls().len(), 1, "發行版停止時不得讀檔");
    assert_eq!(h.sink.submits().len(), 1, "沿用上一輪結果，不重送");

    h.distros.set(&["Ubuntu"]);
    h.next_round().await;
    assert_eq!(h.branches.calls(), 2);
    assert_eq!(
        h.sink.submits().last(),
        Some(&vec![OpenSpecEntry {
            pane: (RuntimeId::new("local"), PaneId::new("w1:p1")),
            location: location(Some(WSL_ROOT)),
            observation: Some(implement(8)),
        }])
    );
}

/// spec「剛啟動時被跳過的 worktree 沒有偵測結果」：發行版從啟動起就沒在跑，數輪都沒有偵測結果、不查詢；`\\wsl$\`
/// 開頭的根目錄同樣受防護。
#[tokio::test(start_paused = true)]
async fn worktree_skipped_since_startup_has_no_result() {
    let dollar_root = r"\\wsl$\Ubuntu\home\u\lib";
    let h = Harness::start(
        vec![
            pane("w1:p1", Some(WSL_ROOT), true),
            pane("w1:p2", Some(dollar_root), true),
        ],
        FakeDistros::new(&["Debian"]),
    )
    .await;
    h.next_round().await;
    h.next_round().await;

    assert_eq!(h.branches.calls(), 0);
    assert!(h.reader.calls().is_empty());
    assert!(h.sink.submits().is_empty(), "沒有結果，與 domain 相同");
    assert_eq!(h.panes.current("w1:p1"), None);
    assert_eq!(h.panes.current("w1:p2"), None);
}

/// D10-6：只有被跳過的 worktree 沿用上一輪，其他 worktree 照常送出新結果。
#[tokio::test(start_paused = true)]
async fn only_skipped_worktree_carries_over() {
    let h = Harness::spawn(
        vec![
            pane("w1:p1", Some(WSL_ROOT), true),
            pane("wJ:p2", Some(APP_ROOT), true),
        ],
        FakeDistros::new(&["Ubuntu"]),
        |h| {
            h.reader.set(WSL_ROOT, Some(implement(1)));
            h.reader.set(APP_ROOT, Some(implement(2)));
        },
    );
    tokio::time::sleep(Duration::from_millis(1)).await;

    h.distros.set(&[]);
    h.reader.set(WSL_ROOT, Some(implement(7)));
    h.reader.set(APP_ROOT, Some(implement(6)));
    h.next_round().await;

    assert_eq!(
        h.sink.submits().last(),
        Some(&vec![
            entry("w1:p1", Some(WSL_ROOT), Some(implement(1))),
            entry("wJ:p2", Some(APP_ROOT), Some(implement(6))),
        ])
    );
}

/// 同一個根目錄底下一個 pane 已連線、一個未連線：照樣查一次，已連線的拿新結果，未連線的沿用上一輪（控制端裁決）。
#[tokio::test(start_paused = true)]
async fn mixed_connection_group_updates_only_connected_panes() {
    let h = Harness::spawn(
        vec![
            pane("wJ:p1", Some(APP_ROOT), true),
            pane("wJ:p2", Some(APP_ROOT), true),
        ],
        FakeDistros::new(&[]),
        |h| h.reader.set(APP_ROOT, Some(implement(3))),
    );
    tokio::time::sleep(Duration::from_millis(1)).await;

    h.panes.set(vec![
        pane("wJ:p1", Some(APP_ROOT), true),
        pane("wJ:p2", Some(APP_ROOT), false),
    ]);
    h.reader.set(APP_ROOT, Some(implement(4)));
    h.next_round().await;

    assert_eq!(h.branches.calls(), 2, "組內有已連線的 pane，照樣查一次");
    assert_eq!(
        h.sink.submits().last(),
        Some(&vec![
            entry("wJ:p1", Some(APP_ROOT), Some(implement(4))),
            entry("wJ:p2", Some(APP_ROOT), Some(implement(3))),
        ])
    );
}

/// Task 4.7 Codex finding（TOCTOU）：每個 WSL worktree 在自己的 git 查詢前一刻各探測一次（不共用整輪的探測結果）；
/// 下一輪同樣各探測一次。
#[tokio::test(start_paused = true)]
async fn each_wsl_worktree_probes_right_before_its_query() {
    let lib_root = r"\\wsl.localhost\Ubuntu\home\u\lib";
    let h = Harness::start(
        vec![
            pane("w1:p1", Some(WSL_ROOT), true),
            pane("w1:p2", Some(lib_root), true),
        ],
        FakeDistros::new(&["Ubuntu"]),
    )
    .await;
    assert_eq!(
        h.distros.probes(),
        4,
        "每個 worktree 在 git 前、讀檔前各探測一次"
    );
    assert_eq!(h.branches.calls(), 2);

    h.next_round().await;
    assert_eq!(h.distros.probes(), 8);
}

/// Task 4.7 Codex finding（TOCTOU）：前一個 WSL worktree 查詢時發行版還在執行，輪到下一個 worktree 前已被停止——
/// 查詢前的探測看到它停了，就不執行 git、不讀檔（不能沿用同一輪較早的探測結果，否則 `wsl.exe -d` 會把它開機）。
#[tokio::test(start_paused = true)]
async fn distro_stopped_before_query_is_not_queried() {
    let lib_root = r"\\wsl.localhost\Ubuntu\home\u\lib";
    let h = Harness::spawn(
        vec![
            pane("w1:p1", Some(WSL_ROOT), true),
            pane("w1:p2", Some(lib_root), true),
        ],
        FakeDistros::new(&[]),
        |h| {
            h.distros.script(&[&["Ubuntu"], &["Ubuntu"]]);
            h.reader.set(WSL_ROOT, Some(implement(3)));
            h.reader.set(lib_root, Some(implement(5)));
        },
    );
    tokio::time::sleep(Duration::from_millis(1)).await;

    assert_eq!(
        h.branches.targets(),
        vec![wsl_target()],
        "停止後的 worktree 不得執行 git"
    );
    assert_eq!(
        h.reader.calls_for(lib_root),
        0,
        "停止後的 worktree 不得讀檔"
    );
    assert_eq!(
        h.sink.submits(),
        vec![vec![
            entry("w1:p1", Some(WSL_ROOT), Some(implement(3))),
            entry("w1:p2", Some(lib_root), None),
        ]]
    );
}

/// WSL worktree 的 pane 全部未連線：整組跳過，連探測都不做。
#[tokio::test(start_paused = true)]
async fn disconnected_wsl_worktree_is_not_probed() {
    let h = Harness::start(
        vec![pane("w1:p1", Some(WSL_ROOT), false)],
        FakeDistros::new(&["Ubuntu"]),
    )
    .await;
    h.next_round().await;

    assert_eq!(h.distros.probes(), 0);
    assert_eq!(h.branches.calls(), 0);
}

/// 被跳過時，歸類已改變的 pane 不沿用舊結果（舊結果屬於舊歸類）：發行版停止後 pane 換到另一個 WSL worktree，
/// 那一組也被跳過——這個 pane 沒有結果，也不會把舊結果送出去。
#[tokio::test(start_paused = true)]
async fn skipped_pane_with_changed_location_does_not_carry_over() {
    let other_root = r"\\wsl.localhost\Ubuntu\home\u\other";
    let h = Harness::spawn(
        vec![pane("w1:p1", Some(WSL_ROOT), true)],
        FakeDistros::new(&["Ubuntu"]),
        |h| h.reader.set(WSL_ROOT, Some(implement(3))),
    );
    tokio::time::sleep(Duration::from_millis(1)).await;
    assert_eq!(h.sink.submits().len(), 1);

    h.distros.set(&[]);
    h.panes.set(vec![pane("w1:p1", Some(other_root), true)]);
    h.next_round().await;

    assert_eq!(h.branches.calls(), 1);
    assert_eq!(
        h.sink.submits().len(),
        1,
        "沒有沿用舊結果，與 domain 現況（無結果）相同，不送"
    );
    assert_eq!(h.panes.current("w1:p1"), None);
}

// ---------------------------------------------------------------------------
// 探測本身壞掉時的 circuit breaker（Task 4.7 Codex 複審 finding）
// ---------------------------------------------------------------------------

/// 排序在 `\\wsl.localhost\...` 之後的 Windows 根目錄（小寫磁碟代號 `d` 排在 `\` 之後），用來驗證 WSL 的 circuit
/// breaker 跳開後，同一輪較晚的 Windows worktree 照常處理。
const LATE_NATIVE_ROOT: &str = r"d:\work\zzz";

/// 兩個 WSL worktree 加一個排在後面的 Windows worktree。第一輪發行版在執行、全部有結果；之後探測以 `fault` 壞掉。
async fn breaker_harness(fault: ProbeFault) -> (Harness, &'static str) {
    let lib_root = r"\\wsl.localhost\Ubuntu\home\u\lib";
    let h = Harness::start(
        vec![
            pane("w1:p1", Some(WSL_ROOT), true),
            pane("w1:p2", Some(lib_root), true),
            pane("wJ:p3", Some(LATE_NATIVE_ROOT), true),
        ],
        FakeDistros::new(&["Ubuntu"]),
    )
    .await;
    h.reader.set(WSL_ROOT, Some(implement(1)));
    h.reader.set(lib_root, Some(implement(2)));
    h.reader.set(LATE_NATIVE_ROOT, Some(implement(3)));
    h.next_round().await;
    assert_eq!(
        h.distros.probes(),
        8,
        "前提：前兩輪每個 WSL worktree 各探測兩次（git 前、讀檔前）"
    );
    assert_eq!(h.branches.calls(), 6);

    h.distros.break_with(Some(fault));
    h.reader.set(WSL_ROOT, Some(implement(7)));
    h.reader.set(lib_root, Some(implement(7)));
    h.reader.set(LATE_NATIVE_ROOT, Some(implement(6)));
    (h, lib_root)
}

/// 斷言壞掉的那一輪：只探測一次，其餘 WSL worktree 不執行 git、不讀檔、沿用上一輪結果；Windows worktree 照常。
fn assert_breaker_round(h: &Harness, lib_root: &str) {
    assert_eq!(h.distros.probes(), 9, "探測失敗後本輪不再探測");
    assert_eq!(
        h.branches.calls(),
        7,
        "這一輪只有 Windows worktree 執行 git"
    );
    assert_eq!(h.reader.calls_for(WSL_ROOT), 2, "WSL worktree 不讀檔");
    assert_eq!(h.reader.calls_for(lib_root), 2, "WSL worktree 不讀檔");
    assert_eq!(h.reader.calls_for(LATE_NATIVE_ROOT), 3);
    assert_eq!(
        h.sink.submits().last(),
        Some(&vec![
            entry("w1:p1", Some(WSL_ROOT), Some(implement(1))),
            entry("w1:p2", Some(lib_root), Some(implement(2))),
            entry("wJ:p3", Some(LATE_NATIVE_ROOT), Some(implement(6))),
        ])
    );
}

/// `wsl.exe --list` 卡住：整輪只等一次探測上限（不隨 WSL worktree 數線性增加），之後的 WSL worktree 跳過、沿用
/// 上一輪；排在後面的 Windows worktree 照常送出新結果。
#[tokio::test(start_paused = true)]
async fn hung_probe_trips_breaker_for_rest_of_round() {
    let (h, lib_root) = breaker_harness(ProbeFault::Hang).await;

    // 第三輪在 2×SYNC_INTERVAL 開始；只等一次探測上限就跑完。
    tokio::time::sleep(SYNC_INTERVAL + DISTRO_PROBE_TIMEOUT).await;
    assert_breaker_round(&h, lib_root);
}

/// 探測執行失敗（非逾時）：同樣本輪不再探測，WSL worktree 跳過、沿用上一輪；Windows worktree 照常。
#[tokio::test(start_paused = true)]
async fn failed_probe_trips_breaker_for_rest_of_round() {
    let (h, lib_root) = breaker_harness(ProbeFault::Fail).await;

    h.next_round().await;
    assert_breaker_round(&h, lib_root);
}

/// 下一輪重新嘗試：探測恢復後，每個 WSL worktree 又在查詢前各探測一次並送出新結果。
#[tokio::test(start_paused = true)]
async fn breaker_resets_next_round() {
    let (h, lib_root) = breaker_harness(ProbeFault::Fail).await;
    h.next_round().await;

    h.distros.break_with(None);
    h.next_round().await;
    assert_eq!(h.distros.probes(), 13);
    assert_eq!(
        h.sink.submits().last(),
        Some(&vec![
            entry("w1:p1", Some(WSL_ROOT), Some(implement(7))),
            entry("w1:p2", Some(lib_root), Some(implement(7))),
            entry("wJ:p3", Some(LATE_NATIVE_ROOT), Some(implement(6))),
        ])
    );
}

/// task 7.2 Codex finding：git 查詢期間發行版被停止（讀檔前的探測看到它沒在執行）——不得讀檔（UNC 路徑會把發行版
/// 開機），該 worktree 沿用上一輪結果。
#[tokio::test(start_paused = true)]
async fn distro_stopped_during_git_query_is_not_read() {
    let h = Harness::spawn(
        vec![pane("w1:p1", Some(WSL_ROOT), true)],
        FakeDistros::new(&["Ubuntu"]),
        |h| h.reader.set(WSL_ROOT, Some(implement(3))),
    );
    tokio::time::sleep(Duration::from_millis(1)).await;
    assert_eq!(h.reader.calls().len(), 1);
    assert_eq!(h.sink.submits().len(), 1);

    // 下一輪：git 前的探測看到在執行，讀檔前的探測看到已停止。
    h.distros.script(&[&["Ubuntu"], &[]]);
    h.reader.set(WSL_ROOT, Some(implement(8)));
    h.next_round().await;

    assert_eq!(h.branches.calls(), 2, "git 前的探測通過，照常查分支");
    assert_eq!(h.reader.calls().len(), 1, "讀檔前發行版已停止，不得讀檔");
    assert_eq!(h.sink.submits().len(), 1, "沿用上一輪結果，不重送");
    assert_eq!(h.panes.current("w1:p1"), Some(implement(3)));
}

/// task 7.2 Codex finding：讀檔前的探測本身失敗——不得讀檔、沿用上一輪，並觸發本輪斷路（其餘 WSL worktree 不再探測、
/// 不執行 git）；Windows worktree 照常。
#[tokio::test(start_paused = true)]
async fn failed_probe_before_read_skips_read_and_trips_breaker() {
    let lib_root = r"\\wsl.localhost\Ubuntu\home\u\lib";
    let h = Harness::start(
        vec![
            pane("w1:p1", Some(WSL_ROOT), true),
            pane("w1:p2", Some(lib_root), true),
            pane("wJ:p3", Some(LATE_NATIVE_ROOT), true),
        ],
        FakeDistros::new(&["Ubuntu"]),
    )
    .await;
    h.reader.set(WSL_ROOT, Some(implement(1)));
    h.reader.set(lib_root, Some(implement(2)));
    h.reader.set(LATE_NATIVE_ROOT, Some(implement(3)));
    h.next_round().await;
    assert_eq!(h.distros.probes(), 8, "前提：前兩輪各探測兩次");

    // 第三輪：第 9 次探測（WSL_ROOT 的 git 前）通過，第 10 次（WSL_ROOT 的讀檔前）失敗。
    h.distros.fail_probe_number(10);
    h.reader.set(WSL_ROOT, Some(implement(7)));
    h.reader.set(lib_root, Some(implement(7)));
    h.reader.set(LATE_NATIVE_ROOT, Some(implement(6)));
    h.next_round().await;

    assert_eq!(h.distros.probes(), 10, "失敗後本輪不再探測");
    assert_eq!(
        h.branches.calls(),
        8,
        "前兩輪各 3 次；第三輪只有 WSL_ROOT 與 Windows worktree 執行 git"
    );
    assert_eq!(h.reader.calls_for(WSL_ROOT), 2, "探測失敗後不得讀檔");
    assert_eq!(
        h.reader.calls_for(lib_root),
        2,
        "斷路後其餘 WSL worktree 不讀檔"
    );
    assert_eq!(
        h.sink.submits().last(),
        Some(&vec![
            entry("w1:p1", Some(WSL_ROOT), Some(implement(1))),
            entry("w1:p2", Some(lib_root), Some(implement(2))),
            entry("wJ:p3", Some(LATE_NATIVE_ROOT), Some(implement(6))),
        ])
    );
}

// ---------------------------------------------------------------------------
// 讀檔逾時（Task 4.7 Codex finding：UNC 讀檔卡住）
// ---------------------------------------------------------------------------

/// 排序在 [`APP_ROOT`] 之前的根目錄（分組依根目錄排序，卡住的先查，才驗得到「其他 root 照常」）。
const STUCK_ROOT: &str = r"D:\work\aaa";

/// 某個 root 讀檔卡住：等滿 [`DETECT_TIMEOUT`] 後該 root 本輪無結果，其他 root 照常送出結果。
#[tokio::test(start_paused = true)]
async fn stalled_read_times_out_and_other_roots_still_submit() {
    let h = Harness::spawn(
        vec![
            pane("wJ:p1", Some(STUCK_ROOT), true),
            pane("wJ:p2", Some(APP_ROOT), true),
        ],
        FakeDistros::new(&[]),
        |h| {
            h.reader.stall(STUCK_ROOT);
            h.reader.set(STUCK_ROOT, Some(implement(1)));
            h.reader.set(APP_ROOT, Some(implement(3)));
        },
    );
    tokio::time::sleep(DETECT_TIMEOUT + Duration::from_millis(1)).await;

    assert_eq!(
        h.sink.submits(),
        vec![vec![
            entry("wJ:p1", Some(STUCK_ROOT), None),
            entry("wJ:p2", Some(APP_ROOT), Some(implement(3))),
        ]]
    );
}

/// 卡住的讀檔還沒返回時，之後幾輪都不再對該 root 派讀檔（不堆積 blocking 執行緒），該 root 無結果；其他 root 每輪
/// 照常讀。卡住的讀檔返回後，下一輪恢復讀檔並送出結果。
#[tokio::test(start_paused = true)]
async fn stalled_root_is_not_read_again_until_previous_read_returns() {
    let h = Harness::spawn(
        vec![
            pane("wJ:p1", Some(STUCK_ROOT), true),
            pane("wJ:p2", Some(APP_ROOT), true),
        ],
        FakeDistros::new(&[]),
        |h| {
            h.reader.stall(STUCK_ROOT);
            h.reader.set(STUCK_ROOT, Some(implement(1)));
            h.reader.set(APP_ROOT, Some(implement(3)));
        },
    );
    tokio::time::sleep(DETECT_TIMEOUT + Duration::from_millis(1)).await;
    for _ in 0..3 {
        h.next_round().await;
    }
    assert_eq!(
        h.reader.calls_for(STUCK_ROOT),
        1,
        "前一次讀檔未返回，不得再派"
    );
    assert_eq!(h.reader.calls_for(APP_ROOT), 4, "其他 root 每輪照常讀");
    assert_eq!(h.panes.current("wJ:p1"), None);
    assert_eq!(h.panes.current("wJ:p2"), Some(implement(3)));

    h.reader.release(STUCK_ROOT);
    h.next_round().await;
    assert_eq!(h.reader.calls_for(STUCK_ROOT), 2, "返回後下一輪恢復讀檔");
    assert_eq!(h.panes.current("wJ:p1"), Some(implement(1)));
}

// ---------------------------------------------------------------------------
// pane 清單取自寫入服務的 domain
// ---------------------------------------------------------------------------

fn connected() -> ConnectionState {
    ConnectionState::Connected {
        since: SystemTime::UNIX_EPOCH,
        server_version: s("test"),
        protocol: 1,
        last_snapshot_at: SystemTime::UNIX_EPOCH,
        settled: true,
        protocol_warning: None,
    }
}

fn store(connected_runtimes: &[&str], registered: &[&str]) -> RuntimeStore {
    let mut store = RuntimeStore::new();
    for id in registered {
        store.register(RuntimeId::new(*id), s("herdr"), s("test"));
    }
    for id in connected_runtimes {
        store
            .set_connection(&RuntimeId::new(*id), connected())
            .expect("已登記");
    }
    store
}

/// 手寫 project `h`：workstream 固定在 pane `local:wJ:p3`（該 pane 歸入沒有 Repo Project 的 repo `lib`）。
fn handwritten_project() -> ProjectDef {
    ProjectDef {
        id: ProjectId::new("h"),
        name: s("h"),
        stages: vec![s("Spec"), s("Build")],
        workstreams: vec![WorkstreamDef {
            id: WorkstreamId::new("be"),
            name: s("be"),
            binding: None,
            pinned_pane: Some(PinnedPane {
                runtime: RuntimeId::new("local"),
                pane_id: PaneId::new("wJ:p3"),
                worktree: None,
            }),
        }],
        tasks: vec![TaskDef {
            id: TaskId::new("t1"),
            title: s("t1"),
            workstream: WorkstreamId::new("be"),
            stage: s("Spec"),
            depends_on: Vec::new(),
        }],
        repo: None,
    }
}

fn domain_with_app(panes: &[((&str, &str), PaneRepo)]) -> DomainState {
    let mut domain = DomainState::from_projects(vec![handwritten_project()]);
    domain.repo_projects.push(RepoProjectDef {
        id: ProjectId::new("app"),
        name: s("app"),
        repo: RepoKey::new(APP_REPO),
        stages: vec![s("規劃"), s("實作"), s("審查"), s("完成")],
        phases: OpenSpecPhase::ALL.into_iter().map(Some).collect(),
    });
    domain.pane_repos = panes
        .iter()
        .map(|((runtime, pane), repo)| {
            ((RuntimeId::new(*runtime), PaneId::new(*pane)), repo.clone())
        })
        .collect();
    domain.refresh_projects();
    domain
}

/// spec「手寫 project 不偵測」：pane 清單只有 Repo Project 的 pane；手寫 project 綁定的 pane（歸入沒有 Repo Project
/// 的 repo）與只被偵測到、未加入的 repo 的 pane 都不在清單中。`connected` 取自 runtime 當下的連線狀態，`current` 取自
/// domain 的 `openspec_obs`。
#[test]
fn store_handle_lists_only_repo_project_panes() {
    let lib = PaneRepo {
        repo: RepoKey::new(r"d:\work\lib\.git"),
        default_name: s("lib"),
        worktree: None,
        root: Some(s(r"D:\work\lib")),
    };
    let domain = domain_with_app(&[
        (("local", "wJ:p1"), location(Some(APP_ROOT))),
        (("wsl", "w1:p2"), location(Some(WSL_ROOT))),
        (("local", "wJ:p3"), lib),
    ]);
    let mut domain = domain;
    domain.openspec_obs.insert(key("wJ:p1"), implement(3));
    let handle = StoreHandle::new_with_domain(store(&["local"], &["local", "wsl"]), domain);

    let panes = handle.panes();

    assert_eq!(
        panes,
        vec![
            SyncPane {
                pane: key("wJ:p1"),
                location: location(Some(APP_ROOT)),
                connected: true,
                current: Some(implement(3)),
            },
            SyncPane {
                pane: (RuntimeId::new("wsl"), PaneId::new("w1:p2")),
                location: location(Some(WSL_ROOT)),
                connected: false,
                current: None,
            },
        ]
    );
}

/// Task 4.3 Ruling 端對端：偵測期間 pane 改了歸類（換到別的 worktree），送出的結果帶著舊歸類，寫入服務不套用——
/// 卡片不動、沒有同步狀態、`openspec_obs` 沒有該 pane。pane 清單與輸出端都是真的寫入服務。
#[tokio::test(start_paused = true)]
async fn reclassified_during_detection_is_not_applied() {
    let domain = domain_with_app(&[(("local", "wJ:p1"), location(Some(APP_ROOT)))]);
    let handle = StoreHandle::new_with_domain(store(&["local"], &["local"]), domain);
    let service = ProgressService::in_memory(handle.clone());
    let reader = FakeReader::default();
    reader.set(APP_ROOT, Some(implement(3)));
    reader.on_next_detect({
        let service = service.clone();
        move || {
            tokio::spawn(async move {
                let moved = [(key("wJ:p1"), location(Some(r"D:\work\app-wt")))]
                    .into_iter()
                    .collect();
                service.set_pane_repos(moved).await.expect("改歸類");
            })
        }
    });

    let job = spawn_openspec_sync(
        handle.clone(),
        FakeBranches::default(),
        reader.clone(),
        FakeDistros::new(&[]),
        service.clone(),
    );
    tokio::time::sleep(Duration::from_millis(1)).await;
    job.abort();

    assert_eq!(reader.calls().len(), 1);
    let domain = handle.with_domain(Clone::clone);
    assert!(domain.openspec_obs.is_empty(), "{:?}", domain.openspec_obs);
    assert!(domain.repo_sync.is_empty(), "{:?}", domain.repo_sync);
    assert!(
        domain.repo_progress.is_empty(),
        "{:?}",
        domain.repo_progress
    );
}

/// 同上的對照組：沒有改歸類時，經真的寫入服務套用（卡片移到「實作」、`openspec_obs` 有該 pane）。
#[tokio::test(start_paused = true)]
async fn detection_is_applied_through_progress_service() {
    let domain = domain_with_app(&[(("local", "wJ:p1"), location(Some(APP_ROOT)))]);
    let handle = StoreHandle::new_with_domain(store(&["local"], &["local"]), domain);
    let service = ProgressService::in_memory(handle.clone());
    let reader = FakeReader::default();
    reader.set(APP_ROOT, Some(implement(3)));

    let job = spawn_openspec_sync(
        handle.clone(),
        FakeBranches::default(),
        reader,
        FakeDistros::new(&[]),
        service,
    );
    tokio::time::sleep(Duration::from_millis(1)).await;
    job.abort();

    let domain = handle.with_domain(Clone::clone);
    assert_eq!(domain.openspec_obs.get(&key("wJ:p1")), Some(&implement(3)));
    let stage = &domain.repo_progress[&ProjectId::new("app")][&TaskId::new("local~wJ:p1")].stage;
    assert_eq!(stage, "實作");
}

/// Task 4.4 審查 fix round 1：兩輪之間 pane 歸類 A→B→A（例如 `cd /tmp && cd -`），`set_pane_repos` 在 B 時丟掉了它的
/// 偵測結果；下一輪偵測結果與上一輪相同，但 domain 現況已沒有它——仍必須送出，`openspec_obs` 恢復。去重依據是
/// domain 現況，不是自己記的「上次送出」。
#[tokio::test(start_paused = true)]
async fn observation_restored_after_reclassification_round_trip() {
    let domain = domain_with_app(&[(("local", "wJ:p1"), location(Some(APP_ROOT)))]);
    let handle = StoreHandle::new_with_domain(store(&["local"], &["local"]), domain);
    let service = ProgressService::in_memory(handle.clone());
    let reader = FakeReader::default();
    reader.set(APP_ROOT, Some(implement(3)));
    let job = spawn_openspec_sync(
        handle.clone(),
        FakeBranches::default(),
        reader,
        FakeDistros::new(&[]),
        service.clone(),
    );
    tokio::time::sleep(Duration::from_millis(1)).await;
    let obs_of = || handle.with_domain(|domain| domain.openspec_obs.get(&key("wJ:p1")).cloned());
    assert_eq!(obs_of(), Some(implement(3)));

    let at = |root: &str| [(key("wJ:p1"), location(Some(root)))].into_iter().collect();
    service.set_pane_repos(at(r"D:\tmp")).await.expect("改到 B");
    service.set_pane_repos(at(APP_ROOT)).await.expect("改回 A");
    assert_eq!(obs_of(), None, "前提：B 時偵測結果已被丟掉");

    tokio::time::sleep(SYNC_INTERVAL).await;
    job.abort();
    assert_eq!(obs_of(), Some(implement(3)), "下一輪必須重新送出");
}

// ---------------------------------------------------------------------------
// 偵測全程唯讀（真實 git 與檔案系統）
// ---------------------------------------------------------------------------

struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("時鐘")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "cockpit-openspec-sync-job-{tag}-{}-{nanos}",
            std::process::id()
        ));
        std::fs::create_dir_all(&path).expect("建立暫存目錄");
        Self(path)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn git(dir: &Path, args: &[&str]) -> std::process::Output {
    std::process::Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .expect("執行 git")
}

/// repo 內所有檔案（含 `.git`）的內容與修改時間。
fn snapshot_files(root: &Path) -> BTreeMap<PathBuf, (Vec<u8>, SystemTime)> {
    let mut files = BTreeMap::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).expect("讀目錄") {
            let path = entry.expect("目錄項目").path();
            let meta = std::fs::metadata(&path).expect("metadata");
            if meta.is_dir() {
                stack.push(path);
            } else {
                let content = std::fs::read(&path).expect("讀檔");
                files.insert(path, (content, meta.modified().expect("mtime")));
            }
        }
    }
    files
}

/// 包真的讀檔實作，數完成了幾次偵測（用來等「跑完兩輪」）。
#[derive(Clone)]
struct CountingReader(Arc<Mutex<usize>>);

impl OpenSpecReader for CountingReader {
    async fn detect(&self, root: &str, branch: Option<String>) -> Option<Observation> {
        let result = FsOpenSpecReader.detect(root, branch).await;
        *self.0.lock().unwrap() += 1;
        result
    }
}

/// spec「repo 不被改動」「機器上沒有 openspec 指令」：以真的 git（`symbolic-ref`）與真的讀檔跑一輪偵測，判定出
/// change 與勾選數；之後 repo 內所有檔案（含 `.git`）的內容與修改時間不變、沒有 `index.lock`、沒有新檔案。
/// 事先 touch 一個已追蹤檔，讓 index 的 stat 資訊過期——任何會重寫 index 的 git 指令都會被抓到。
#[tokio::test]
async fn real_detection_leaves_repo_untouched() {
    let tmp = TempDir::new("readonly");
    let repo = tmp.0.join("Repo");
    std::fs::create_dir_all(repo.join("openspec").join("changes").join("foo"))
        .expect("建立 change 目錄");
    assert!(git(&repo, &["init", "-q"]).status.success(), "git init");
    let toplevel =
        String::from_utf8(git(&repo, &["rev-parse", "--show-toplevel"]).stdout).expect("UTF-8");
    // 兩邊都 canonicalize 再比：CI（windows-latest）的 `%TEMP%` 是 8.3 短名 `RUNNER~1`，git 回報的是 canonical
    // 長路徑（同 `cockpit/tests/git_endpoint.rs` 的 `init_repo`）。
    let expected = std::fs::canonicalize(&repo).expect("canonicalize 暫存 repo");
    let actual = std::fs::canonicalize(toplevel.trim()).expect("canonicalize git 回報的 toplevel");
    assert_eq!(
        actual, expected,
        "暫存 repo 的 toplevel 應是它自己（不可落到外層 repo）"
    );
    std::fs::write(
        repo.join("openspec/changes/foo/tasks.md"),
        "- [x] 1.1 a\n- [ ] 1.2 b\n- [ ] 1.3 c\n",
    )
    .expect("寫 tasks.md");
    std::fs::write(repo.join("README.md"), "hi\n").expect("寫 README");
    let commit = |args: &[&str]| {
        let mut full = vec!["-c", "user.name=t", "-c", "user.email=t@example.com"];
        full.extend_from_slice(args);
        assert!(git(&repo, &full).status.success(), "git {args:?}");
    };
    commit(&["add", "-A"]);
    commit(&["commit", "-q", "-m", "init"]);
    commit(&["checkout", "-q", "-b", "feat/foo"]);
    // 讓 index 的 stat 資訊過期：mtime 改到未來。
    let readme = std::fs::File::options()
        .write(true)
        .open(repo.join("README.md"))
        .expect("開 README");
    readme
        .set_modified(SystemTime::now() + Duration::from_secs(120))
        .expect("改 mtime");
    drop(readme);
    let before = snapshot_files(&repo);

    let root = repo.to_str().expect("UTF-8").to_string();
    let panes = FakePanes::default();
    panes.set(vec![SyncPane {
        pane: key("wJ:p1"),
        location: location(Some(&root)),
        connected: true,
        current: None,
    }]);
    let sink = RecordingSink::new(panes.clone());
    let detects = Arc::new(Mutex::new(0));
    let job = spawn_openspec_sync(
        panes,
        GitBranchLookup::new(Arc::new(cockpit_git::GitRunner::new())),
        CountingReader(Arc::clone(&detects)),
        FakeDistros::new(&[]),
        sink.clone(),
    );
    // 跑兩輪（第二輪在 SYNC_INTERVAL 之後），真實時間。
    let deadline = tokio::time::Instant::now() + SYNC_INTERVAL + Duration::from_secs(30);
    while *detects.lock().unwrap() < 2 {
        assert!(tokio::time::Instant::now() < deadline, "時限內應跑完兩輪");
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    job.abort();
    let _ = job.await;

    let submits = sink.submits();
    assert_eq!(submits.len(), 1, "第二輪結果相同、domain 已有，不再送");
    assert_eq!(
        submits[0],
        vec![OpenSpecEntry {
            pane: key("wJ:p1"),
            location: location(Some(&root)),
            observation: Some(Observation {
                change: s("foo"),
                phase: OpenSpecPhase::Implement,
                checked: 1,
                total: 3,
            }),
        }]
    );
    let after = snapshot_files(&repo);
    assert!(!repo.join(".git").join("index.lock").exists());
    assert_eq!(
        before.keys().collect::<Vec<_>>(),
        after.keys().collect::<Vec<_>>(),
        "不得新增或刪除檔案"
    );
    for (path, (content, mtime)) in &before {
        let (after_content, after_mtime) = &after[path];
        assert_eq!(content, after_content, "{} 內容被改", path.display());
        assert_eq!(mtime, after_mtime, "{} 修改時間被改", path.display());
    }
}
