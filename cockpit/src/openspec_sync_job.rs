//! `OpenSpecSync`：OpenSpec 進度的背景偵測工作（openspec-stage-sync task 4.4；design D5、D10-4、D10-6；spec
//! `openspec-stage-sync`「偵測的時機與去重」「偵測不得喚醒 WSL 或查詢未連線的 runtime」「偵測全程唯讀」）。
//!
//! 啟動當下跑第一輪，之後每 [`SYNC_INTERVAL`] 一輪：
//!
//! 1. 從寫入服務的 domain 取出所有 Repo Project task 的 pane 與它們的歸類（[`SyncPanes`]；不用 `RepoResolver`
//!    自己可能較新的歸類，Task 3.2 carry），依 worktree 根目錄（`PaneRepo::root`）分組。沒有根目錄的 pane 不查詢，
//!    表中為無結果。
//! 2. 每個 worktree 先過防護（design D5 第 2 點）：組內所屬 runtime 已連線的 pane 才會得到新結果，一個都沒有就整組
//!    跳過；根目錄是 `\\wsl.localhost\`／`\\wsl$\` 而該發行版沒在執行（[`RunningDistros`]）也整組跳過。WSL 的
//!    探測在**該 worktree 的 git 查詢前一刻**做，每個 WSL worktree 各探測一次，不共用同一輪較早的探測結果
//!    （openspec-stage-sync task 4.7，Codex TOCTOU finding）。探測本身逾時（`DISTRO_PROBE_TIMEOUT`）或執行失敗時，
//!    本輪其餘 WSL worktree 不再探測、一律跳過（circuit breaker，task 4.7 Codex 複審），一輪最多只等一次探測上限；
//!    下一輪重新嘗試。被跳過的 pane 沿用上一輪的結果（歸類相同時），剛啟動沒有上一輪則為無結果（design D10-6）。
//!    跳過時不執行 git、不讀檔。
//! 3. 經 [`BranchLookup`]（`CurrentBranch`）取目前分支，成功才經 [`OpenSpecReader`]（task 4.2 的 `detect`）讀檔；
//!    分支查詢出錯時不讀檔，該 worktree 為無結果（Task 4.2 carry）。同一個 worktree 每輪只查一次、讀一次，各
//!    worktree 依序查詢（同 `RepoResolver`，最多佔 `GitRunner` 一個名額）。
//! 4. 讀檔有上限 [`DETECT_TIMEOUT`]（task 4.7，Codex UNC 讀檔 finding）：讀檔在另一個 task 上跑，逾時該 worktree
//!    本輪為無結果，其他 worktree 照常處理。逾時的讀檔不會被取消（blocking 執行緒收不掉），它返回之前同一個
//!    worktree 不再派新的讀檔、每輪為無結果，避免卡在 9P 的 blocking 執行緒越堆越多；它返回後下一輪恢復。
//! 5. 去重依據是 **domain 現況**，不是自己記的「上次送出」（Task 4.4 審查 fix round 1）：[`SyncPanes`] 一併帶出
//!    domain 中每個 pane 目前的偵測結果（`openspec_obs`），算出的整張表裡只要有任一筆結果與它不同，就把整張表
//!    （所有 Repo Project pane，每筆附偵測當下的歸類）交給 [`OpenSpecSink::submit`]（寫入服務的 `sync_openspec`）；
//!    全部相同才不送。送出失敗時寫入服務記憶體不變，下一輪自然再比到差異而重送（Task 4.3 carry）。歸類在兩輪之間
//!    A→B→A 時，`set_pane_repos` 會丟掉該 pane 的結果，下一輪也因與現況不同而補送。
//!
//! 時間窗（已知、接受）：
//!
//! - 連線狀態與 pane 清單只在每輪開頭讀一次；一輪進行中 runtime 斷線，這一輪剩下的查詢照樣進行。
//! - WSL 探測與 git 查詢、讀檔之間仍有毫秒級的時間窗：發行版剛好在這中間被停止時，`wsl.exe -d` 或經
//!   `\\wsl.localhost` 讀檔仍可能把它開機。完全消除需要不經 `wsl.exe -d` 與 UNC 的機制，超出本 change 的範圍
//!   （design「Risks / Trade-offs」；`RepoResolver` 的探測結果沿用 60 秒，時間窗更長）。
//!
//! 全程唯讀：只跑 `git symbolic-ref -q HEAD` 與讀 `openspec/changes/`，不新增任何 HERDR method。

use std::collections::{BTreeMap, HashMap, HashSet};
use std::future::Future;
use std::path::Path;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use cockpit_core::{
    ConnectionState, Observation, PaneId, PaneRepo, RuntimeId, StoreHandle, split_pane_item_id,
};
use cockpit_git::{CurrentBranch, GitRunner, GitTarget, select_target};
use tokio::task::JoinHandle;

use crate::openspec_sync::detect;
use crate::progress_service::{OpenSpecEntry, ProgressService, WriteError};
use crate::repo_resolver::{FailureLog, LogLevel, RunningDistros, try_probe_running_distros};

/// 兩輪偵測的間隔（design D5）。
pub const SYNC_INTERVAL: Duration = Duration::from_secs(10);

/// 每個 worktree 一次讀檔（[`OpenSpecReader::detect`]）的上限（openspec-stage-sync task 4.7）。
pub const DETECT_TIMEOUT: Duration = Duration::from_secs(5);

/// 一個要偵測的 pane：所屬 Repo Project task 的 pane、它目前的歸類、所屬 runtime 是否已連線。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SyncPane {
    /// `(runtime id, pane id)`。
    pub pane: (RuntimeId, PaneId),
    /// 寫入服務 domain 中這個 pane 目前的歸類（含 worktree 根目錄）。
    pub location: PaneRepo,
    /// 所屬 runtime 目前是否為已連線。
    pub connected: bool,
    /// domain 中這個 pane 目前的偵測結果（`openspec_obs`）；去重以它為準。
    pub current: Option<Observation>,
}

/// 偵測對象的來源（正式實作是 [`StoreHandle`]：寫入服務的 domain 與 `RuntimeStore` 的連線狀態）。
pub trait SyncPanes: Send + Sync + 'static {
    /// 所有展開中的 Repo Project task 的 pane，依 `(runtime, pane)` 排序、不重複。手寫 project 的 pane 不在其中。
    fn panes(&self) -> Vec<SyncPane>;
}

impl SyncPanes for StoreHandle {
    fn panes(&self) -> Vec<SyncPane> {
        let located: Located = self.with_domain(|domain| {
            domain
                .projects
                .iter()
                .filter(|def| def.repo.is_some())
                .flat_map(|def| &def.tasks)
                .filter_map(|task| {
                    let key = split_pane_item_id(task.id.as_str())?;
                    let location = domain.pane_repos.get(&key)?.clone();
                    let current = domain.openspec_obs.get(&key).cloned();
                    Some((key, (location, current)))
                })
                .collect()
        });
        self.with_store(|store| {
            located
                .into_iter()
                .map(|(pane, (location, current))| SyncPane {
                    connected: matches!(
                        store.connection(&pane.0),
                        Some(ConnectionState::Connected { .. })
                    ),
                    pane,
                    location,
                    current,
                })
                .collect()
        })
    }
}

/// pane → (目前歸類, domain 中目前的偵測結果)。
type Located = BTreeMap<(RuntimeId, PaneId), (PaneRepo, Option<Observation>)>;

/// 目前分支查詢（design D7）：`Ok(Some)` 分支名稱、`Ok(None)` detached、`Err` 查詢出錯（附原因只給日誌用）。
pub trait BranchLookup: Send + Sync + 'static {
    /// 對 `target` 查目前分支。
    fn current_branch(
        &self,
        target: &GitTarget,
    ) -> impl Future<Output = Result<Option<String>, String>> + Send;
}

/// 正式的分支查詢：經共用的 `GitRunner` 執行 [`CurrentBranch`]。
pub struct GitBranchLookup {
    runner: Arc<GitRunner>,
}

impl GitBranchLookup {
    /// 用共用的 runner 建立（「同時最多 4 支 git」的上限與 git 端點、repo resolver 一起生效）。
    pub fn new(runner: Arc<GitRunner>) -> Self {
        Self { runner }
    }
}

impl BranchLookup for GitBranchLookup {
    async fn current_branch(&self, target: &GitTarget) -> Result<Option<String>, String> {
        let run = self
            .runner
            .run(&CurrentBranch, target)
            .await
            .map_err(|error| error.to_string())?;
        CurrentBranch::parse(&run.calls).map_err(|error| error.to_string())
    }
}

/// 讀 `openspec/changes/` 判定偵測結果（task 4.2 的 `detect`）。
pub trait OpenSpecReader: Send + Sync + 'static {
    /// 偵測 `root`（worktree 根目錄的主機路徑）在分支 `branch` 下的結果。
    fn detect(
        &self,
        root: &str,
        branch: Option<String>,
    ) -> impl Future<Output = Option<Observation>> + Send;
}

/// 正式的讀檔實作：在 blocking 執行緒上呼叫 [`detect`]（讀目錄與檔案是阻塞 IO；`\\wsl.localhost` 上可能慢）。
pub struct FsOpenSpecReader;

impl OpenSpecReader for FsOpenSpecReader {
    async fn detect(&self, root: &str, branch: Option<String>) -> Option<Observation> {
        let root = root.to_string();
        tokio::task::spawn_blocking(move || detect(Path::new(&root), branch.as_deref()))
            .await
            .unwrap_or_else(|error| {
                tracing::debug!(%error, "OpenSpec 讀檔工作異常結束，本輪視為無結果");
                None
            })
    }
}

/// 偵測結果的輸出端（正式實作是 [`ProgressService`]）。
pub trait OpenSpecSink: Send + Sync + 'static {
    /// 送出整張對照表（寫入服務的 `sync_openspec`）。
    fn submit(
        &self,
        entries: Vec<OpenSpecEntry>,
    ) -> impl Future<Output = Result<(), WriteError>> + Send;
}

impl OpenSpecSink for ProgressService {
    async fn submit(&self, entries: Vec<OpenSpecEntry>) -> Result<(), WriteError> {
        self.sync_openspec(entries).await
    }
}

/// 起偵測背景工作。它不會自己結束，關機時由 `app::shutdown_all` `abort()` 再 `await`。
pub fn spawn_openspec_sync<P, B, R, D, S>(
    panes: P,
    branches: B,
    reader: R,
    distros: D,
    sink: S,
) -> JoinHandle<()>
where
    P: SyncPanes,
    B: BranchLookup,
    R: OpenSpecReader,
    D: RunningDistros,
    S: OpenSpecSink,
{
    let job = Job {
        panes,
        branches,
        reader: Arc::new(reader),
        distros,
        sink,
        last_computed: BTreeMap::new(),
        submit_failures: FailureLog::default(),
        root_failures: RootFailures::default(),
        reads_in_flight: ReadsInFlight::default(),
        read_timeouts: RootFailures::default(),
    };
    tokio::spawn(job.run())
}

struct Job<P, B, R, D, S> {
    panes: P,
    branches: B,
    /// 讀檔在另一個 task 上跑（逾時後仍要跑到返回），所以共用。
    reader: Arc<R>,
    distros: D,
    sink: S,
    /// 上一輪算出的表；被防護跳過的 pane 沿用這裡的結果。
    last_computed: BTreeMap<(RuntimeId, PaneId), OpenSpecEntry>,
    /// `submit` 連續失敗的 log 降噪。
    submit_failures: FailureLog,
    /// 分支查詢失敗的 log 降噪，依根目錄分開（ledger Task 2.1：`wsl.exe` 往 stderr 印警告時 detached 會恆為錯誤，
    /// 不可每輪洗版）。
    root_failures: RootFailures,
    /// 還沒返回的讀檔（依根目錄）；在其中的根目錄不再派新的讀檔。
    reads_in_flight: ReadsInFlight,
    /// 讀檔逾時（含前一次讀檔未返回而沒派）的 log 降噪，依根目錄分開。
    read_timeouts: RootFailures,
}

/// 一個 worktree 本輪的處理結果。
enum Outcome {
    /// 查過了：已連線的 pane 得到這個結果（`None`＝無結果）。
    Detected(Option<Observation>),
    /// 被防護跳過：沿用上一輪。
    Skipped,
}

impl<P, B, R, D, S> Job<P, B, R, D, S>
where
    P: SyncPanes,
    B: BranchLookup,
    R: OpenSpecReader,
    D: RunningDistros,
    S: OpenSpecSink,
{
    async fn run(mut self) {
        loop {
            self.round().await;
            tokio::time::sleep(SYNC_INTERVAL).await;
        }
    }

    async fn round(&mut self) {
        let panes = self.panes.panes();
        let mut groups: BTreeMap<String, Vec<SyncPane>> = BTreeMap::new();
        let mut table: BTreeMap<(RuntimeId, PaneId), OpenSpecEntry> = BTreeMap::new();
        let mut rootless = Vec::new();
        for pane in panes {
            match pane.location.root.clone() {
                Some(root) => groups.entry(root).or_default().push(pane),
                None => {
                    table.insert(pane.pane.clone(), entry(&pane, None));
                    rootless.push(pane);
                }
            }
        }

        // 探測本身逾時或失敗後，本輪其餘 WSL worktree 不再探測、一律跳過（task 4.7 Codex 複審：避免每個 worktree
        // 各等一次探測上限、反覆啟動壞掉的 `wsl.exe`）。下一輪重新嘗試。
        let mut probe_broken = false;
        for (root, members) in &groups {
            let outcome = if members.iter().any(|pane| pane.connected) {
                self.query(root, &mut probe_broken).await
            } else {
                Outcome::Skipped
            };
            for pane in members {
                let observation = match &outcome {
                    Outcome::Detected(observation) if pane.connected => observation.clone(),
                    // 未連線的 pane（或整組被跳過）沿用上一輪；歸類變了就不沿用（結果屬於舊歸類）。
                    _ => self
                        .last_computed
                        .get(&pane.pane)
                        .filter(|previous| previous.location.same_location(&pane.location))
                        .and_then(|previous| previous.observation.clone()),
                };
                table.insert(pane.pane.clone(), entry(pane, observation));
            }
        }
        self.root_failures.retain(|root| groups.contains_key(root));
        self.read_timeouts.retain(|root| groups.contains_key(root));

        // 與 domain 現況比：任一筆結果不同才送（表中的歸類就是 domain 目前的歸類）。
        let differs = groups.values().flatten().chain(&rootless).any(|pane| {
            table.get(&pane.pane).and_then(|e| e.observation.as_ref()) != pane.current.as_ref()
        });
        let entries: Vec<OpenSpecEntry> = table.values().cloned().collect();
        self.last_computed = table;
        if differs {
            let result = self.sink.submit(entries).await;
            // 失敗時寫入服務的記憶體不變，下一輪比對時仍不同，自然重送。
            self.submit_failures
                .report(&result, "套用 OpenSpec 偵測結果失敗，下一輪重試");
        }
    }

    /// WSL 防護：探測發行版是否正在執行；沒在執行、或探測逾時／失敗都回 `false`（呼叫端回 [`Outcome::Skipped`]）。
    /// 探測本身逾時或失敗時順便把 `probe_broken` 設起來，本輪其餘 WSL worktree 不再探測。
    async fn wsl_distro_up(&self, root: &str, distro: &str, probe_broken: &mut bool) -> bool {
        if *probe_broken {
            tracing::debug!(%root, %distro, "本輪 WSL 探測已失敗，不偵測");
            return false;
        }
        let Some(running) = try_probe_running_distros(&self.distros).await else {
            tracing::debug!(%root, %distro, "WSL 探測逾時或失敗，本輪其餘 WSL worktree 都不偵測");
            *probe_broken = true;
            return false;
        };
        let up = running.iter().any(|name| name.eq_ignore_ascii_case(distro));
        if !up {
            tracing::debug!(%root, %distro, "WSL 發行版沒在執行，不偵測（不為偵測開機）");
        }
        up
    }

    /// 對一個 worktree 過 WSL 防護後查分支、讀檔。
    async fn query(&mut self, root: &str, probe_broken: &mut bool) -> Outcome {
        let Ok(target) = select_target(root) else {
            tracing::debug!(%root, "worktree 根目錄無法轉成 git 執行目標，本輪無結果");
            return Outcome::Detected(None);
        };
        // 在 git 查詢前一刻探測（task 4.7）：不沿用同一輪較早的探測結果，把「探測→`wsl.exe -d`」的時間窗縮到最短。
        if let GitTarget::Wsl { distro, .. } = &target
            && !self.wsl_distro_up(root, distro, probe_broken).await
        {
            return Outcome::Skipped;
        }
        if self.reads_in_flight.contains(root) {
            if self.read_timeouts.observe(root, true) == Some(LogLevel::Warn) {
                tracing::warn!(%root, "前一次 OpenSpec 讀檔仍未返回，該 worktree 本輪沒有偵測結果");
            } else {
                tracing::debug!(%root, "前一次 OpenSpec 讀檔仍未返回，本輪不再派讀檔");
            }
            return Outcome::Detected(None);
        }

        match self.branches.current_branch(&target).await {
            Ok(branch) => {
                if self.root_failures.observe(root, false) == Some(LogLevel::Info) {
                    tracing::info!(%root, "目前分支查詢已恢復");
                }
                // 讀檔前再探測一次（openspec-stage-sync task 7.2）：git 查詢期間發行版可能被停止，而讀 UNC 路徑
                // 會把它開機。
                if let GitTarget::Wsl { distro, .. } = &target
                    && !self.wsl_distro_up(root, distro, probe_broken).await
                {
                    return Outcome::Skipped;
                }
                Outcome::Detected(self.read(root, branch).await)
            }
            Err(reason) => {
                match self.root_failures.observe(root, true) {
                    Some(LogLevel::Warn) => {
                        tracing::warn!(%root, %reason, "目前分支查詢失敗，該 worktree 本輪沒有偵測結果");
                    }
                    _ => tracing::debug!(%root, %reason, "目前分支查詢持續失敗"),
                }
                Outcome::Detected(None)
            }
        }
    }
}

impl<P, B, R, D, S> Job<P, B, R, D, S>
where
    R: OpenSpecReader,
{
    /// 讀檔，最多等 [`DETECT_TIMEOUT`]（task 4.7）。讀檔在另一個 task 上跑：逾時時只是不再等它，它會跑到返回為止，
    /// 返回時才把根目錄移出 [`ReadsInFlight`]。
    async fn read(&mut self, root: &str, branch: Option<String>) -> Option<Observation> {
        let in_flight = self.reads_in_flight.enter(root);
        let reader = Arc::clone(&self.reader);
        let owned_root = root.to_string();
        let read = tokio::spawn(async move {
            let _in_flight = in_flight;
            reader.detect(&owned_root, branch).await
        });
        match tokio::time::timeout(DETECT_TIMEOUT, read).await {
            Ok(joined) => {
                if self.read_timeouts.observe(root, false) == Some(LogLevel::Info) {
                    tracing::info!(%root, "OpenSpec 讀檔已恢復");
                }
                joined.unwrap_or_else(|error| {
                    tracing::debug!(%root, %error, "OpenSpec 讀檔工作異常結束，本輪視為無結果");
                    None
                })
            }
            Err(_) => {
                match self.read_timeouts.observe(root, true) {
                    Some(LogLevel::Warn) => tracing::warn!(
                        %root,
                        timeout_secs = DETECT_TIMEOUT.as_secs(),
                        "OpenSpec 讀檔逾時，該 worktree 本輪沒有偵測結果"
                    ),
                    _ => tracing::debug!(%root, "OpenSpec 讀檔持續逾時"),
                }
                None
            }
        }
    }
}

/// 還沒返回的讀檔所屬的根目錄（task 4.7）。[`ReadsInFlight::enter`] 回傳的守衛 drop 時移除。
#[derive(Clone, Default)]
struct ReadsInFlight(Arc<Mutex<HashSet<String>>>);

impl ReadsInFlight {
    fn contains(&self, root: &str) -> bool {
        self.lock().contains(root)
    }

    fn enter(&self, root: &str) -> InFlightRead {
        self.lock().insert(root.to_string());
        InFlightRead {
            set: self.clone(),
            root: root.to_string(),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashSet<String>> {
        // 鎖內只做集合增刪，不會 panic；就算中毒，集合本身仍一致。
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// 一次進行中的讀檔；drop（讀檔返回，或 runtime 關閉時 task 被丟棄）時把根目錄移出集合。
struct InFlightRead {
    set: ReadsInFlight,
    root: String,
}

impl Drop for InFlightRead {
    fn drop(&mut self) {
        self.set.lock().remove(&self.root);
    }
}

fn entry(pane: &SyncPane, observation: Option<Observation>) -> OpenSpecEntry {
    OpenSpecEntry {
        pane: pane.pane.clone(),
        location: pane.location.clone(),
        observation,
    }
}

/// 分支查詢失敗的 log 降噪，依根目錄各自一份 [`FailureLog`]：同一個根目錄連續失敗只在第一次回 `Warn`，之後 `Debug`，
/// 恢復時 `Info`；一直成功不記。
#[derive(Default)]
struct RootFailures(HashMap<String, FailureLog>);

impl RootFailures {
    fn observe(&mut self, root: &str, failed: bool) -> Option<LogLevel> {
        match self.0.get_mut(root) {
            Some(log) => log.observe(failed),
            // 沒有紀錄＝一直成功；成功時不必建表項。
            None if !failed => None,
            None => self.0.entry(root.to_string()).or_default().observe(true),
        }
    }

    /// 只保留還在的根目錄（不讓消失的 worktree 一直留在表裡）。
    fn retain(&mut self, keep: impl Fn(&str) -> bool) {
        self.0.retain(|root, _| keep(root));
    }
}

#[cfg(test)]
mod tests {
    use super::RootFailures;
    use crate::repo_resolver::LogLevel;

    /// ledger Task 2.1：同一個根目錄每輪都失敗時只 warn 一次；不同根目錄各自計算；恢復記 info，再失敗重新 warn。
    #[test]
    fn branch_failure_logs_are_deduplicated_per_root() {
        let mut failures = RootFailures::default();
        assert_eq!(failures.observe("a", true), Some(LogLevel::Warn));
        for _ in 0..10 {
            assert_eq!(failures.observe("a", true), Some(LogLevel::Debug));
        }
        assert_eq!(
            failures.observe("b", true),
            Some(LogLevel::Warn),
            "另一個根目錄各自計算"
        );
        assert_eq!(failures.observe("a", false), Some(LogLevel::Info));
        assert_eq!(failures.observe("a", false), None);
        assert_eq!(failures.observe("a", true), Some(LogLevel::Warn));

        failures.retain(|root| root == "a");
        assert_eq!(
            failures.observe("b", true),
            Some(LogLevel::Warn),
            "移除後重新出現的根目錄重新 warn"
        );
    }
}
