//! repo-projects task 4.3 驗收測試：`RepoResolver` 背景工作（design D2、D4；spec `repo-projects`「repo 身分判定」
//! 「納入 repo 判定的 pane 與更新時機」「Repo Project 進度的保存與清除」）。
//!
//! 全部用 tokio 暫停時間（`start_paused`）與假查詢，不啟動 git。投影、投影任務與寫入服務都是真的：resolver
//! 送出的歸類結果經寫入服務改變投影、再喚醒 resolver，這條回饋路徑也在測試裡。
//!
//! 「一輪」的觀測點：resolver 每一輪結束都請求一次清除檢查，所以清除請求次數＝已跑完的輪數。等待時暫停的時鐘
//! 只會跳到最早的計時器（投影任務的 50 ms 合併窗，或 resolver 的 60 秒定時器），所以「等下一輪」花掉的虛擬時間
//! 能分辨這一輪是投影改變觸發（遠小於 1 秒）還是定時器觸發（60 秒）。

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use cockpit::files::PathMapping;
use cockpit::progress_service::{ProgressService, WriteError};
use cockpit::repo_resolver::{
    DISTRO_PROBE_TIMEOUT, LookupOutcome, PaneReposSink, ROUND_TIMER, RepoLookup, RunningDistros,
    spawn_repo_resolver,
};
use cockpit_core::{
    AgentStatus, ConnectionState, DomainState, Focused, Mark, Pane, PaneId, PaneRepo, PaneRepos,
    ProjectId, RepoKey, RepoProjectDef, RuntimeId, RuntimeSnapshot, RuntimeStore, StoreHandle, Tab,
    TabId, TaskId, TaskProgress, Workspace, WorkspaceId, spawn_projector,
};
use cockpit_git::{GitTarget, RepoIdentityOutput};
use tokio::sync::watch;
use tokio::task::JoinHandle;
use tokio::time::Instant;

// ---------------------------------------------------------------------------
// 假查詢與計數輸出端
// ---------------------------------------------------------------------------

/// 依 `GitTarget` 回預先設定的結果；沒設定的回 `NotRepo`。記下每一次查詢。
#[derive(Clone, Default)]
struct FakeLookup(Arc<FakeLookupInner>);

#[derive(Default)]
struct FakeLookupInner {
    outcomes: Mutex<Vec<(GitTarget, LookupOutcome)>>,
    calls: Mutex<Vec<GitTarget>>,
    /// 下一次查詢時執行一次的副作用（例如讓 runtime 在一輪進行中斷線）。
    on_next_lookup: Mutex<Option<Box<dyn FnOnce() + Send>>>,
    /// 每次查詢回傳前等待的時間（模擬 git 執行耗時）。
    delay: Mutex<Duration>,
}

impl FakeLookup {
    fn on_next_lookup(&self, effect: impl FnOnce() + Send + 'static) {
        *self.0.on_next_lookup.lock().unwrap() = Some(Box::new(effect));
    }

    fn set_delay(&self, delay: Duration) {
        *self.0.delay.lock().unwrap() = delay;
    }

    fn set(&self, target: GitTarget, outcome: LookupOutcome) {
        let mut outcomes = self.0.outcomes.lock().unwrap();
        outcomes.retain(|(t, _)| *t != target);
        outcomes.push((target, outcome));
    }

    fn calls_for(&self, target: &GitTarget) -> usize {
        self.0
            .calls
            .lock()
            .unwrap()
            .iter()
            .filter(|t| *t == target)
            .count()
    }

    fn total_calls(&self) -> usize {
        self.0.calls.lock().unwrap().len()
    }
}

impl RepoLookup for FakeLookup {
    async fn lookup(&self, target: &GitTarget) -> LookupOutcome {
        self.0.calls.lock().unwrap().push(target.clone());
        let effect = self.0.on_next_lookup.lock().unwrap().take();
        if let Some(effect) = effect {
            effect();
        }
        let delay = *self.0.delay.lock().unwrap();
        if !delay.is_zero() {
            tokio::time::sleep(delay).await;
        }
        self.0
            .outcomes
            .lock()
            .unwrap()
            .iter()
            .find(|(t, _)| t == target)
            .map(|(_, outcome)| outcome.clone())
            .unwrap_or(LookupOutcome::NotRepo)
    }
}

/// 假的「正在執行的 WSL 發行版」清單：`None`＝探測失敗。記下探測次數。預設只有 `Ubuntu` 在跑。
#[derive(Clone)]
struct FakeDistros(Arc<FakeDistrosInner>);

struct FakeDistrosInner {
    running: Mutex<Option<Vec<String>>>,
    probes: Mutex<usize>,
    /// 為真時探測永不回應（模擬 `wsl.exe` 卡住）。
    hang: Mutex<bool>,
}

impl FakeDistros {
    fn new(running: Option<&[&str]>) -> Self {
        Self(Arc::new(FakeDistrosInner {
            running: Mutex::new(running.map(|names| names.iter().map(|n| s(n)).collect())),
            probes: Mutex::new(0),
            hang: Mutex::new(false),
        }))
    }

    fn hanging() -> Self {
        let distros = Self::new(Some(&["Ubuntu"]));
        *distros.0.hang.lock().unwrap() = true;
        distros
    }

    fn set(&self, running: Option<&[&str]>) {
        *self.0.running.lock().unwrap() = running.map(|names| names.iter().map(|n| s(n)).collect());
    }

    fn probes(&self) -> usize {
        *self.0.probes.lock().unwrap()
    }
}

impl Default for FakeDistros {
    fn default() -> Self {
        Self::new(Some(&["Ubuntu"]))
    }
}

impl RunningDistros for FakeDistros {
    async fn running(&self) -> Option<Vec<String>> {
        *self.0.probes.lock().unwrap() += 1;
        let hang = *self.0.hang.lock().unwrap();
        if hang {
            std::future::pending::<()>().await;
        }
        self.0.running.lock().unwrap().clone()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Counts {
    submits: usize,
    clears: usize,
}

/// 包真的寫入服務，數送出與清除請求的次數。
#[derive(Clone)]
struct CountingSink {
    service: ProgressService,
    counts: Arc<watch::Sender<Counts>>,
}

impl PaneReposSink for CountingSink {
    async fn submit(&self, pane_repos: PaneRepos) -> Result<(), WriteError> {
        let result = self.service.set_pane_repos(pane_repos).await;
        self.counts.send_modify(|c| c.submits += 1);
        result
    }

    async fn request_clear(&self) -> Result<(), WriteError> {
        let result = self.service.clear_vanished_progress().await;
        self.counts.send_modify(|c| c.clears += 1);
        result
    }
}

// ---------------------------------------------------------------------------
// 組裝
// ---------------------------------------------------------------------------

fn s(v: &str) -> String {
    v.to_string()
}

fn rid(id: &str) -> RuntimeId {
    RuntimeId::new(id)
}

fn native(path: &str) -> GitTarget {
    GitTarget::Native { path: s(path) }
}

fn wsl_target(posix: &str) -> GitTarget {
    GitTarget::Wsl {
        distro: s("Ubuntu"),
        posix: s(posix),
    }
}

fn repo_at(common_dir: &str, toplevel: &str) -> LookupOutcome {
    LookupOutcome::Repo(RepoIdentityOutput {
        common_dir: s(common_dir),
        git_dir: s(common_dir),
        toplevel: s(toplevel),
    })
}

fn pane_repo(key: &str, name: &str) -> PaneRepo {
    PaneRepo {
        repo: RepoKey::new(key),
        default_name: s(name),
        worktree: None,
    }
}

fn expected(entries: &[(&str, &str, &str, &str)]) -> PaneRepos {
    entries
        .iter()
        .map(|(runtime, pane, key, name)| {
            ((rid(runtime), PaneId::new(*pane)), pane_repo(key, name))
        })
        .collect()
}

/// `(pane id, cwd, exited)`。
type PaneSpec<'a> = (&'a str, Option<&'a str>, bool);

fn snapshot(panes: &[PaneSpec]) -> RuntimeSnapshot {
    RuntimeSnapshot {
        server_version: s("test"),
        protocol: 1,
        workspaces: vec![Workspace {
            id: WorkspaceId::new("w1"),
            label: None,
            number: 1,
            agent_status: AgentStatus::Idle,
            focused: false,
        }],
        tabs: vec![Tab {
            id: TabId::new("t1"),
            workspace_id: WorkspaceId::new("w1"),
            number: 1,
            agent_status: AgentStatus::Idle,
            focused: false,
        }],
        panes: panes
            .iter()
            .map(|(id, cwd, exited)| Pane {
                id: PaneId::new(*id),
                workspace_id: WorkspaceId::new("w1"),
                tab_id: TabId::new("t1"),
                agent: None,
                agent_status: AgentStatus::Idle,
                title: None,
                cwd: cwd.map(s),
                label: None,
                focused: false,
                exited: *exited,
                updated_at: SystemTime::UNIX_EPOCH,
            })
            .collect(),
        agents: Vec::new(),
        focused: Focused {
            workspace_id: None,
            tab_id: None,
            pane_id: None,
        },
        protocol_warning: None,
    }
}

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

fn disconnected() -> ConnectionState {
    ConnectionState::Disconnected {
        reason: s("test"),
        retry_in: Duration::from_secs(30),
    }
}

/// 登記 `runtimes`（全部尚未連上）。
fn store_with(runtimes: &[&str]) -> RuntimeStore {
    let mut store = RuntimeStore::new();
    for id in runtimes {
        store.register(rid(id), s("herdr"), s("test"));
    }
    store
}

/// `local`（Windows 路徑）與 `wsl`（distro `Ubuntu`）有路徑對應；`nomap` 沒有。
fn mappings() -> Arc<HashMap<RuntimeId, PathMapping>> {
    Arc::new(HashMap::from([
        (rid("local"), PathMapping::Native),
        (
            rid("wsl"),
            PathMapping::Wsl {
                distro: s("Ubuntu"),
            },
        ),
    ]))
}

struct Harness {
    handle: StoreHandle,
    lookup: FakeLookup,
    distros: FakeDistros,
    counts: Arc<watch::Sender<Counts>>,
    _projector: JoinHandle<()>,
    resolver: JoinHandle<()>,
}

impl Harness {
    fn start(store: RuntimeStore, domain: DomainState, lookup: FakeLookup) -> Self {
        Self::start_with_distros(store, domain, lookup, FakeDistros::default())
    }

    fn start_with_distros(
        store: RuntimeStore,
        domain: DomainState,
        lookup: FakeLookup,
        distros: FakeDistros,
    ) -> Self {
        let handle = StoreHandle::new_with_domain(store, domain);
        let projector = spawn_projector(handle.clone());
        let service = ProgressService::in_memory(handle.clone());
        let counts = Arc::new(watch::Sender::new(Counts::default()));
        let sink = CountingSink {
            service,
            counts: Arc::clone(&counts),
        };
        let resolver = spawn_repo_resolver(
            handle.subscribe(),
            mappings(),
            lookup.clone(),
            distros.clone(),
            sink,
        );
        Self {
            handle,
            lookup,
            distros,
            counts,
            _projector: projector,
            resolver,
        }
    }

    fn counts(&self) -> Counts {
        *self.counts.borrow()
    }

    /// 等到第 `n` 輪結束，回傳等待花掉的虛擬時間。
    async fn wait_round(&self, n: usize) -> Duration {
        let start = Instant::now();
        let mut rx = self.counts.subscribe();
        tokio::time::timeout(
            Duration::from_secs(3 * 3600),
            rx.wait_for(|c| c.clears >= n),
        )
        .await
        .unwrap_or_else(|_| panic!("等不到第 {n} 輪"))
        .expect("計數頻道不會關閉");
        start.elapsed()
    }

    /// 等下一輪，並斷言它是投影改變觸發的（不是 60 秒定時器）。
    async fn next_event_round(&self) {
        let n = self.counts().clears + 1;
        let waited = self.wait_round(n).await;
        assert!(
            waited < Duration::from_secs(1),
            "這一輪應由投影改變觸發，實際等了 {waited:?}"
        );
    }

    /// 等投影與 resolver 都靜下來（遠短於定時器）。
    async fn settle(&self) {
        tokio::time::sleep(Duration::from_secs(1)).await;
    }

    fn pane_repos(&self) -> PaneRepos {
        self.handle.with_domain(|d| d.pane_repos.clone())
    }

    fn replace(&self, runtime: &str, panes: &[PaneSpec]) {
        self.handle
            .replace(&rid(runtime), snapshot(panes))
            .expect("已登記");
    }

    fn set_connection(&self, runtime: &str, state: ConnectionState) {
        self.handle
            .set_connection(&rid(runtime), state)
            .expect("已登記");
    }
}

impl Drop for Harness {
    fn drop(&mut self) {
        self.resolver.abort();
    }
}

/// `local` 已連線、pane 樹為 `panes`。
fn connected_local(panes: &[PaneSpec]) -> RuntimeStore {
    let mut store = store_with(&["local"]);
    store
        .replace(&rid("local"), snapshot(panes))
        .expect("已登記");
    store
        .set_connection(&rid("local"), connected())
        .expect("已登記");
    store
}

const APP_SRC: &str = r"D:\work\app\src";
const APP_KEY: &str = r"d:\work\app\.git";
const LIB_DIR: &str = r"D:\work\lib";
const LIB_KEY: &str = r"d:\work\lib\.git";

fn app_lookup() -> FakeLookup {
    let lookup = FakeLookup::default();
    lookup.set(native(APP_SRC), repo_at("D:/work/app/.git", "D:/work/app"));
    lookup.set(native(LIB_DIR), repo_at("D:/work/lib/.git", "D:/work/lib"));
    lookup
}

// ---------------------------------------------------------------------------
// 測試
// ---------------------------------------------------------------------------

/// spec「新 pane 出現」：已連線 runtime 的新 pane 在下一輪歸類；同一個 cwd 只查一次（快取）。
#[tokio::test(start_paused = true)]
async fn new_pane_is_classified_and_same_cwd_is_cached() {
    let h = Harness::start(
        connected_local(&[("wJ:p1", Some(APP_SRC), false)]),
        DomainState::default(),
        app_lookup(),
    );
    h.wait_round(1).await;
    assert_eq!(
        h.pane_repos(),
        expected(&[("local", "wJ:p1", APP_KEY, "app")])
    );

    h.replace(
        "local",
        &[
            ("wJ:p1", Some(APP_SRC), false),
            ("wJ:p2", Some(APP_SRC), false),
        ],
    );
    h.settle().await;
    assert_eq!(
        h.pane_repos(),
        expected(&[
            ("local", "wJ:p1", APP_KEY, "app"),
            ("local", "wJ:p2", APP_KEY, "app"),
        ])
    );
    assert_eq!(
        h.lookup.calls_for(&native(APP_SRC)),
        1,
        "同一個 cwd 在快取期間只查一次"
    );
}

/// spec「cwd 變動在下一次 snapshot 後反映」：cwd 改變（新的快取鍵）就重查，pane 改歸新 repo。
#[tokio::test(start_paused = true)]
async fn cwd_change_is_requeried_and_reclassified() {
    let h = Harness::start(
        connected_local(&[("wJ:p1", Some(APP_SRC), false)]),
        DomainState::default(),
        app_lookup(),
    );
    h.wait_round(1).await;
    assert_eq!(
        h.pane_repos(),
        expected(&[("local", "wJ:p1", APP_KEY, "app")])
    );

    h.replace("local", &[("wJ:p1", Some(LIB_DIR), false)]);
    h.next_event_round().await;
    assert_eq!(
        h.pane_repos(),
        expected(&[("local", "wJ:p1", LIB_KEY, "lib")])
    );
    assert_eq!(h.lookup.calls_for(&native(LIB_DIR)), 1);
    assert_eq!(h.lookup.calls_for(&native(APP_SRC)), 1);
}

/// 「不是 repo」60 秒過期：投影沒變，定時器那一輪也會重查；成功的結果 10 分鐘內不重查、10 分鐘後重查。
#[tokio::test(start_paused = true)]
async fn expired_entries_are_requeried_by_the_timer() {
    let lookup = app_lookup();
    let tmp = native(r"D:\tmp");
    let h = Harness::start(
        connected_local(&[
            ("wJ:p1", Some(APP_SRC), false),
            ("wJ:p2", Some(r"D:\tmp"), false),
        ]),
        DomainState::default(),
        lookup,
    );
    h.wait_round(1).await;
    h.settle().await;
    let rounds = h.counts().clears;
    assert_eq!(h.lookup.calls_for(&tmp), 1);

    // 投影不再改變：下一輪由定時器觸發。
    let waited = h.wait_round(rounds + 1).await;
    assert!(
        waited >= ROUND_TIMER - Duration::from_secs(2) && waited <= ROUND_TIMER,
        "下一輪應由 60 秒定時器觸發，實際等了 {waited:?}"
    );
    assert_eq!(h.lookup.calls_for(&tmp), 2, "「不是 repo」60 秒後重查");
    assert_eq!(h.lookup.calls_for(&native(APP_SRC)), 1, "成功結果尚未過期");

    // 走到 10 分鐘之前：成功結果仍沿用。
    tokio::time::sleep(Duration::from_secs(8 * 60)).await;
    assert_eq!(
        h.lookup.calls_for(&native(APP_SRC)),
        1,
        "成功結果 10 分鐘內不重查"
    );
    // 再兩分鐘多：已過 10 分鐘，定時器那一輪重查。
    tokio::time::sleep(Duration::from_secs(2 * 60 + 5)).await;
    assert_eq!(
        h.lookup.calls_for(&native(APP_SRC)),
        2,
        "成功結果 10 分鐘後重查"
    );
    assert_eq!(
        h.pane_repos(),
        expected(&[("local", "wJ:p1", APP_KEY, "app")])
    );
}

/// spec「暫時性錯誤不當作非 repo」：暫時錯誤時不歸類；git 恢復後最多 60 秒的下一次重查把它歸入 repo。
#[tokio::test(start_paused = true)]
async fn transient_error_is_not_classified_and_retried_within_a_minute() {
    let lookup = FakeLookup::default();
    lookup.set(native(APP_SRC), LookupOutcome::Unavailable(s("逾時")));
    let h = Harness::start(
        connected_local(&[("wJ:p1", Some(APP_SRC), false)]),
        DomainState::default(),
        lookup.clone(),
    );
    h.wait_round(1).await;
    assert!(h.pane_repos().is_empty(), "暫時錯誤時不歸類");
    assert_eq!(h.counts().submits, 0, "歸類結果沒變，不送出");

    lookup.set(native(APP_SRC), repo_at("D:/work/app/.git", "D:/work/app"));
    tokio::time::sleep(ROUND_TIMER + Duration::from_secs(1)).await;
    assert_eq!(h.lookup.calls_for(&native(APP_SRC)), 2);
    assert_eq!(
        h.pane_repos(),
        expected(&[("local", "wJ:p1", APP_KEY, "app")])
    );
}

/// design D2 第 3 點：送出讓投影改變、喚醒下一輪，但內容相同不再送出——不會無限循環。
#[tokio::test(start_paused = true)]
async fn submitting_does_not_loop() {
    let h = Harness::start(
        connected_local(&[("wJ:p1", Some(APP_SRC), false)]),
        DomainState::default(),
        app_lookup(),
    );
    h.wait_round(1).await;
    // 第一輪送出 → 投影改變 → 第二輪（內容相同、不送出）→ 靜止。
    h.next_event_round().await;
    h.settle().await;
    assert_eq!(
        h.counts(),
        Counts {
            submits: 1,
            clears: 2
        }
    );

    // 跑 30 分鐘：只有定時器的輪次，重查結果相同，送出次數不增加。
    tokio::time::sleep(Duration::from_secs(30 * 60)).await;
    let counts = h.counts();
    assert_eq!(counts.submits, 1, "內容不變不得再送出");
    assert!(
        counts.clears <= 2 + 30,
        "輪數應受定時器限制（每 60 秒最多一輪），實際 {}",
        counts.clears
    );
}

/// spec「範圍外的 pane」：已 exited、cwd 為空或沒有、runtime 沒有路徑對應的 pane 都不查也不歸類。
#[tokio::test(start_paused = true)]
async fn out_of_scope_panes_are_neither_queried_nor_classified() {
    let mut store = store_with(&["local", "nomap"]);
    store
        .replace(
            &rid("local"),
            snapshot(&[
                ("wJ:p1", Some(APP_SRC), true),
                ("wJ:p2", Some(""), false),
                ("wJ:p3", None, false),
            ]),
        )
        .expect("已登記");
    store
        .set_connection(&rid("local"), connected())
        .expect("已登記");
    store
        .replace(&rid("nomap"), snapshot(&[("wK:p1", Some(APP_SRC), false)]))
        .expect("已登記");
    store
        .set_connection(&rid("nomap"), connected())
        .expect("已登記");

    let h = Harness::start(store, DomainState::default(), app_lookup());
    h.wait_round(1).await;
    h.settle().await;
    assert_eq!(h.lookup.total_calls(), 0);
    assert!(h.pane_repos().is_empty());
}

/// spec「未連線的 runtime 沿用上次歸類」：斷線期間不查 git（即使快取已過期）、沿用上次歸類；重新連上後才重查。
#[tokio::test(start_paused = true)]
async fn disconnected_runtime_is_not_queried_and_keeps_its_classification() {
    let lookup = FakeLookup::default();
    let app = wsl_target("/home/u/app");
    lookup.set(app.clone(), repo_at("/home/u/app/.git", "/home/u/app"));
    let mut store = store_with(&["wsl"]);
    store
        .replace(
            &rid("wsl"),
            snapshot(&[("w1:p1", Some("/home/u/app"), false)]),
        )
        .expect("已登記");
    store
        .set_connection(&rid("wsl"), connected())
        .expect("已登記");
    let h = Harness::start(store, DomainState::default(), lookup);
    h.wait_round(1).await;
    let classified = expected(&[(
        "wsl",
        "w1:p1",
        r"\\wsl.localhost\Ubuntu\home\u\app\.git",
        "app",
    )]);
    assert_eq!(h.pane_repos(), classified);
    assert_eq!(h.lookup.calls_for(&app), 1);

    h.set_connection("wsl", disconnected());
    h.next_event_round().await;
    // 超過成功結果的 10 分鐘：斷線中仍不查。多睡 30 秒避開定時器的相位，讓下面「重新連上」那一輪不會剛好與
    // 定時器那一輪重疊（重疊時 `next_event_round` 會等到定時器那一輪，而它看到的投影仍是斷線）。
    tokio::time::sleep(Duration::from_secs(11 * 60 + 30)).await;
    assert_eq!(h.lookup.calls_for(&app), 1, "斷線的 runtime 不得查 git");
    assert_eq!(h.pane_repos(), classified, "斷線期間沿用上次歸類");

    h.set_connection("wsl", connected());
    h.next_event_round().await;
    assert_eq!(h.lookup.calls_for(&app), 2, "重新連上後重查（快取已過期）");
    assert_eq!(h.pane_repos(), classified);
}

/// spec「Cockpit 關閉期間被關掉的 pane」：runtime 連上後的第一輪就請求清除，關閉期間消失的 pane 的進度被清掉；
/// 這一輪歸類結果沒有改變（沒有送出），清除照樣發生（端對端經真的寫入服務）。
#[tokio::test(start_paused = true)]
async fn first_round_after_connect_clears_progress_of_vanished_panes() {
    let mut domain = DomainState::default();
    domain.repo_projects.push(RepoProjectDef {
        id: ProjectId::new("app"),
        name: s("app"),
        repo: RepoKey::new(APP_KEY),
        stages: vec![s("Plan"), s("Review")],
    });
    let table = domain
        .repo_progress
        .entry(ProjectId::new("app"))
        .or_default();
    for task in ["local~wJ:p1", "local~wJ:p2"] {
        table.insert(
            TaskId::new(task),
            TaskProgress {
                stage: s("Review"),
                mark: Mark::None,
            },
        );
    }
    domain.refresh_projects();

    // 剛啟動：`local` 尚未連上（`Connecting`），但 store 已有 pane 樹、cwd 都在 repo 內——仍不得查 git、不歸類。
    let mut store = store_with(&["local"]);
    store
        .replace(
            &rid("local"),
            snapshot(&[
                ("wJ:p1", Some(APP_SRC), false),
                ("wJ:p2", Some(APP_SRC), false),
            ]),
        )
        .expect("已登記");
    let h = Harness::start(store, domain, app_lookup());
    h.wait_round(1).await;
    h.settle().await;
    assert_eq!(h.lookup.total_calls(), 0, "尚未連上的 runtime 不得查 git");
    assert!(h.pane_repos().is_empty(), "尚未連上時不歸類");
    let progress_keys = |h: &Harness| -> Vec<String> {
        let mut keys: Vec<String> = h.handle.with_domain(|d| {
            d.repo_progress
                .get(&ProjectId::new("app"))
                .map(|t| t.keys().map(|k| k.to_string()).collect())
                .unwrap_or_default()
        });
        keys.sort();
        keys
    };
    assert_eq!(
        progress_keys(&h),
        vec![s("local~wJ:p1"), s("local~wJ:p2")],
        "尚未連上時保留"
    );

    // 連上：pane 樹只剩 p1（cwd 不是 repo，歸類結果仍為空）。
    h.replace("local", &[("wJ:p1", Some(r"D:\tmp"), false)]);
    h.set_connection("local", connected());
    h.next_event_round().await;
    assert_eq!(
        progress_keys(&h),
        vec![s("local~wJ:p1")],
        "第一輪就清掉消失的 p2"
    );
    assert_eq!(h.counts().submits, 0, "歸類結果沒變，沒有送出");
}

/// spec「系統不得為了判定 repo 而啟動 WSL 發行版」：一輪進行中 runtime 斷線（例如 `wsl --shutdown`），剩下還沒查的
/// pane 不得再查 git；這個 runtime 改走沿用分支（沿用上一輪的歸類，這裡是空的）。
#[tokio::test(start_paused = true)]
async fn runtime_disconnecting_mid_round_stops_further_queries() {
    let lookup = FakeLookup::default();
    lookup.set(
        wsl_target("/home/u/app"),
        repo_at("/home/u/app/.git", "/home/u/app"),
    );
    lookup.set(
        wsl_target("/home/u/lib"),
        repo_at("/home/u/lib/.git", "/home/u/lib"),
    );
    let mut store = store_with(&["wsl"]);
    store
        .replace(
            &rid("wsl"),
            snapshot(&[
                ("w1:p1", Some("/home/u/app"), false),
                ("w1:p2", Some("/home/u/lib"), false),
            ]),
        )
        .expect("已登記");
    store
        .set_connection(&rid("wsl"), connected())
        .expect("已登記");
    let h = Harness::start(store, DomainState::default(), lookup.clone());
    // 第一次查詢一開始 runtime 就斷線；查詢本身耗時 1 秒，期間投影已反映斷線（合併窗 50 ms）。
    // resolver 的第一輪要等測試第一次 await 才會跑，所以這裡設定的副作用一定趕得上。
    let handle = h.handle.clone();
    lookup.on_next_lookup(move || {
        handle
            .set_connection(&rid("wsl"), disconnected())
            .expect("已登記");
    });
    lookup.set_delay(Duration::from_secs(1));

    h.wait_round(1).await;
    h.settle().await;
    assert_eq!(h.lookup.total_calls(), 1, "runtime 斷線後不得再查 git");
    assert!(
        h.pane_repos().is_empty(),
        "斷線的 runtime 沿用上一輪（空的）歸類，不採用這一輪的部分結果"
    );
}

/// Windows runtime 的 pane 停在 WSL 的 UNC 路徑（`\\wsl.localhost\`、`\\wsl$\`）時，查詢目標是 WSL；pane 所屬的
/// HERDR runtime 已連線不代表目標發行版在跑。
const UNC_APP: &str = r"\\wsl.localhost\Ubuntu\home\u\app";
const UNC_LIB: &str = r"\\wsl$\Ubuntu\home\u\lib";
const UNC_APP_KEY: &str = r"\\wsl.localhost\Ubuntu\home\u\app\.git";

/// spec「系統不得為了判定 repo 而啟動 WSL 發行版」（最終審查 I1）：Windows runtime 已連線、pane cwd 在 WSL 的 UNC
/// 路徑，但目標發行版沒在跑（含探測失敗）→ 一次 git 查詢都不做、不歸類；同一輪多個 WSL 目標只探測一次。
#[tokio::test(start_paused = true)]
async fn windows_pane_on_wsl_path_is_not_queried_when_distro_is_not_running() {
    let lookup = FakeLookup::default();
    lookup.set(
        wsl_target("/home/u/app"),
        repo_at("/home/u/app/.git", "/home/u/app"),
    );
    let distros = FakeDistros::new(Some(&["Debian"]));
    let h = Harness::start_with_distros(
        connected_local(&[
            ("wJ:p1", Some(UNC_APP), false),
            ("wJ:p2", Some(UNC_LIB), false),
        ]),
        DomainState::default(),
        lookup,
        distros,
    );
    h.wait_round(1).await;
    h.settle().await;
    assert_eq!(h.lookup.total_calls(), 0, "目標發行版沒在跑，不得查 git");
    assert!(h.pane_repos().is_empty());
    assert_eq!(h.distros.probes(), 1, "同一輪只探測一次");

    // 探測失敗視為沒在跑。
    h.distros.set(None);
    let probes = h.distros.probes();
    h.wait_round(h.counts().clears + 1).await;
    assert_eq!(h.lookup.total_calls(), 0, "探測失敗視為沒在跑");
    assert_eq!(h.distros.probes(), probes + 1, "下一輪重新探測");
}

/// 同上，目標發行版在跑（名稱大小寫不同也算）→ 照常查詢、歸類；之後發行版停了，快取過期也不再查、沿用上次歸類。
#[tokio::test(start_paused = true)]
async fn windows_pane_on_wsl_path_is_queried_only_while_distro_is_running() {
    let lookup = FakeLookup::default();
    let app = wsl_target("/home/u/app");
    lookup.set(app.clone(), repo_at("/home/u/app/.git", "/home/u/app"));
    let distros = FakeDistros::new(Some(&["ubuntu"]));
    let h = Harness::start_with_distros(
        connected_local(&[("wJ:p1", Some(UNC_APP), false)]),
        DomainState::default(),
        lookup,
        distros,
    );
    h.wait_round(1).await;
    h.settle().await;
    let classified = expected(&[("local", "wJ:p1", UNC_APP_KEY, "app")]);
    assert_eq!(h.lookup.calls_for(&app), 1, "發行版在跑：照常查詢");
    assert_eq!(h.pane_repos(), classified);

    h.distros.set(Some(&[]));
    // 超過成功結果的 10 分鐘：快取過期，但發行版沒在跑，不查、沿用上次歸類。
    tokio::time::sleep(Duration::from_secs(11 * 60 + 30)).await;
    assert_eq!(h.lookup.calls_for(&app), 1, "發行版停了，不得查 git");
    assert_eq!(h.pane_repos(), classified, "沿用上次歸類");
}

/// 最終修正波複審 Important：`wsl.exe` 卡住（WSL 服務異常）時探測永不回應。resolver 對探測設逾時
/// （[`DISTRO_PROBE_TIMEOUT`]），逾時視為沒在跑：這一輪照樣結束，原生 pane 照常歸類、清除照常請求，WSL 目標不查。
#[tokio::test(start_paused = true)]
async fn hanging_distro_probe_times_out_and_round_still_finishes() {
    let lookup = app_lookup();
    lookup.set(
        wsl_target("/home/u/app"),
        repo_at("/home/u/app/.git", "/home/u/app"),
    );
    let h = Harness::start_with_distros(
        connected_local(&[
            ("wJ:p0", Some(UNC_APP), false),
            ("wJ:p1", Some(APP_SRC), false),
        ]),
        DomainState::default(),
        lookup,
        FakeDistros::hanging(),
    );
    let waited = h.wait_round(1).await;
    assert!(
        waited >= DISTRO_PROBE_TIMEOUT && waited < DISTRO_PROBE_TIMEOUT + Duration::from_secs(1),
        "這一輪應在探測逾時後結束，實際等了 {waited:?}"
    );
    h.settle().await;
    assert_eq!(
        h.pane_repos(),
        expected(&[("local", "wJ:p1", APP_KEY, "app")]),
        "原生 pane 照常歸類"
    );
    assert_eq!(
        h.lookup.calls_for(&wsl_target("/home/u/app")),
        0,
        "逾時視為沒在跑，不查 WSL 目標"
    );
    assert_eq!(h.distros.probes(), 1);
}

/// 最終修正波複審 Minor：探測結果跨輪沿用 `RETRY_TTL`（在跑／沒在跑都沿用）。pane 停在已停止發行版的 UNC 路徑時，
/// 投影改變觸發的多輪只探測一次；過期後下一輪再探測。
#[tokio::test(start_paused = true)]
async fn distro_probe_result_is_reused_across_rounds_until_it_expires() {
    let distros = FakeDistros::new(Some(&[]));
    let h = Harness::start_with_distros(
        connected_local(&[("wJ:p0", Some(UNC_APP), false)]),
        DomainState::default(),
        app_lookup(),
        distros,
    );
    h.wait_round(1).await;
    assert_eq!(h.distros.probes(), 1);

    for extra in 1..=3 {
        let panes: Vec<(String, &str)> =
            (1..=extra).map(|n| (format!("wJ:p{n}"), APP_SRC)).collect();
        let mut specs: Vec<PaneSpec> = vec![("wJ:p0", Some(UNC_APP), false)];
        specs.extend(
            panes
                .iter()
                .map(|(id, cwd)| (id.as_str(), Some(*cwd), false)),
        );
        h.replace("local", &specs);
        h.next_event_round().await;
    }
    assert_eq!(h.distros.probes(), 1, "60 秒內多輪只探測一次");

    // 投影不再變動後，下一輪由定時器在 60 秒後觸發；那時探測結果（從第一輪探測開始算 60 秒）已過期。
    h.settle().await;
    assert_eq!(h.distros.probes(), 1, "靜下來後仍只探測一次");
    tokio::time::sleep(ROUND_TIMER + Duration::from_secs(5)).await;
    assert_eq!(h.distros.probes(), 2, "過期後下一輪再探測");
    assert_eq!(h.lookup.calls_for(&wsl_target("/home/u/app")), 0);
}

// ---------------------------------------------------------------------------
// 正式查詢實作（真的 git；暫存目錄建在 repo 外的 %TEMP%）
// ---------------------------------------------------------------------------

struct TempDir(std::path::PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        let nanos = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("system clock 應晚於 UNIX_EPOCH")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "cockpit-repo-resolver-{tag}-{}-{nanos}",
            std::process::id()
        ));
        std::fs::create_dir_all(&path).expect("建立測試暫存目錄");
        Self(path)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn git(dir: &std::path::Path, args: &[&str]) -> std::process::Output {
    std::process::Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .expect("執行 git")
}

/// `GitRepoLookup` 對真的 git：一般目錄是「不是 repo」，`git init` 過的目錄是 repo，歸類後 key 為小寫反斜線。
#[tokio::test]
async fn git_repo_lookup_distinguishes_repo_and_plain_directory() {
    use cockpit::repo_resolver::{GitRepoLookup, classify};

    let tmp = TempDir::new("lookup");
    let plain = tmp.0.join("plain");
    let repo = tmp.0.join("Repo");
    std::fs::create_dir_all(&plain).expect("建立一般目錄");
    std::fs::create_dir_all(&repo).expect("建立 repo 目錄");
    // 前提：暫存目錄不在任何 repo 內（否則一般目錄會被歸入外層 repo）。
    assert_eq!(
        git(&plain, &["rev-parse", "--git-dir"]).status.code(),
        Some(128),
        "暫存目錄不應在任何 git repo 內"
    );
    assert!(git(&repo, &["init", "-q"]).status.success(), "git init");
    let toplevel = git(&repo, &["rev-parse", "--show-toplevel"]);
    let toplevel = String::from_utf8(toplevel.stdout).expect("UTF-8");
    assert!(
        toplevel.trim().to_lowercase().ends_with("/repo"),
        "暫存 repo 的 toplevel 應是它自己：{toplevel}"
    );

    let lookup = GitRepoLookup::new(Arc::new(cockpit_git::GitRunner::new()));
    let plain_target = native(plain.to_str().expect("UTF-8"));
    assert_eq!(lookup.lookup(&plain_target).await, LookupOutcome::NotRepo);

    let repo_target = native(repo.to_str().expect("UTF-8"));
    let LookupOutcome::Repo(output) = lookup.lookup(&repo_target).await else {
        panic!("git init 過的目錄應是 repo");
    };
    let classified = classify(&repo_target, &output).expect("應歸類");
    assert_eq!(classified.default_name, "Repo", "預設名稱保留原始大小寫");
    assert_eq!(classified.worktree, None);
    let key = classified.repo.as_str();
    assert!(key.ends_with(r"\repo\.git"), "key 應為反斜線、小寫：{key}");
    assert_eq!(key, key.to_lowercase());
}
