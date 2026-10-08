//! `RepoResolver`：pane cwd → repo 的背景歸類工作（repo-projects task 4.3；design D1、D2、D4；spec `repo-projects`
//! 「repo 身分判定」「納入 repo 判定的 pane 與更新時機」「Repo Project 進度的保存與清除」）。
//!
//! 每一輪（投影改變，或距上一輪結束 [`ROUND_TIMER`] 都沒有變動）：
//!
//! 1. 從投影收集**已連線**、有路徑對應的 runtime 中未 exited、cwd 非空的 pane。未連線 runtime 的 pane 不查 git
//!    （對斷線的 WSL runtime 執行 `wsl.exe -d` 可能把發行版開機），沿用上一輪的歸類結果。查詢目標是 WSL 時
//!    （含 Windows runtime 的 pane 停在 WSL 的 UNC 路徑），另以不會開機的 [`RunningDistros`] 探測確認目標發行版
//!    正在執行（結果跨輪沿用 [`RETRY_TTL`]，逾時 [`DISTRO_PROBE_TIMEOUT`] 與失敗都視為沒在跑）；沒在跑就不查，
//!    該 pane 沿用上一輪的歸類。
//! 2. 以 `(runtime, cwd)` 查快取；沒有或過期才經 [`RepoLookup`] 查 git，一次一支（依序）。成功的結果
//!    [`SUCCESS_TTL`] 後過期，「不是 repo」與暫時錯誤 [`RETRY_TTL`] 後過期；過期時間從查詢**開始**起算，
//!    所以 [`ROUND_TIMER`] 觸發的那一輪一定看得到上一輪查的「不是 repo」已過期。
//! 3. 算出的 `pane_repos` 與上次送出的不同時才交給 [`PaneReposSink::submit`]（寫入服務的
//!    `set_pane_repos`）；送出會讓投影改變、再喚醒一輪，但那一輪算出的內容相同、不再送出，不會無限循環。
//! 4. 每一輪結束都 [`PaneReposSink::request_clear`] 一次（寫入服務的 `clear_vanished_progress`），不論
//!    `pane_repos` 有沒有變；清除判定在寫入服務的鎖內依 `RuntimeStore` 進行，這裡不判斷。
//!
//! repo key 正規化（design D2）：Windows 路徑正斜線轉反斜線、整串轉小寫；WSL 輸出的 POSIX 路徑經
//! [`wsl_host_path`] 轉成 `\\wsl.localhost\<distro>\...`、保留大小寫。預設名稱與 worktree 標註取自 git 原始輸出。

use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use cockpit_core::{PaneRepo, PaneRepos, ProjectedConnection, ProjectedState, RepoKey, RuntimeId};
use cockpit_git::{GitRunner, GitTarget, RepoIdentity, RepoIdentityOutput, select_target};
use tokio::sync::watch;
use tokio::task::JoinHandle;
use tokio::time::Instant;

use crate::files::{PathMapping, wsl_host_path};
use crate::progress_service::{ProgressService, WriteError};

/// 成功歸類（是 repo）的快取沿用時間（design D2）。
pub const SUCCESS_TTL: Duration = Duration::from_secs(10 * 60);
/// 「不是 repo」與暫時錯誤的快取沿用時間（design D2）。
pub const RETRY_TTL: Duration = Duration::from_secs(60);
/// WSL 探測（[`RunningDistros`]）的逾時；逾時視為沒有發行版在執行（`wsl.exe` 卡住時不讓整個 resolver 停住）。
pub const DISTRO_PROBE_TIMEOUT: Duration = Duration::from_secs(5);
/// 投影沒有變動時，距上一輪結束多久再跑一輪（讓過期項目也會重查；design D2）。
pub const ROUND_TIMER: Duration = Duration::from_secs(60);

/// 一次 repo 身分查詢的結果。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LookupOutcome {
    /// 是 repo：git 回報的三個原始路徑。
    Repo(RepoIdentityOutput),
    /// 不在任何 repo 內（不是 repo、裸 repo、在 `.git` 內、cwd 不存在）。
    NotRepo,
    /// 暫時無法判定（git 不存在、逾時、輸出不符等）；附原因只給日誌用。
    Unavailable(String),
}

/// repo 身分查詢（design D2：以 trait 注入，正式實作 [`GitRepoLookup`] 包 `GitRunner`，測試用假查詢）。
pub trait RepoLookup: Send + Sync + 'static {
    /// 對 `target` 查 repo 身分。
    fn lookup(&self, target: &GitTarget) -> impl Future<Output = LookupOutcome> + Send;
}

/// 正式的查詢實作：經 `GitRunner` 執行 `RepoIdentity`。與 git 端點共用同一個 runner，「同時最多 4 支」的上限
/// 對兩者一起生效；resolver 依序查詢，最多只佔其中一個名額。
pub struct GitRepoLookup {
    runner: Arc<GitRunner>,
}

impl GitRepoLookup {
    /// 用共用的 runner 建立。
    pub fn new(runner: Arc<GitRunner>) -> Self {
        Self { runner }
    }
}

impl RepoLookup for GitRepoLookup {
    /// 結果分類照 `RepoIdentity::parse` 的約定（repo-projects task 2.1）：`run` 的 `Err`（git 不存在、不受信任、
    /// 逾時）與 `parse` 的 `Err`（結束碼不是 128、輸出不符）都是暫時錯誤；`parse` 回 `Ok(None)` 才是「不是 repo」。
    async fn lookup(&self, target: &GitTarget) -> LookupOutcome {
        let run = match self.runner.run(&RepoIdentity, target).await {
            Ok(run) => run,
            Err(error) => return LookupOutcome::Unavailable(error.to_string()),
        };
        match RepoIdentity::parse(&run.calls) {
            Ok(Some(output)) => LookupOutcome::Repo(output),
            Ok(None) => LookupOutcome::NotRepo,
            Err(error) => LookupOutcome::Unavailable(error.to_string()),
        }
    }
}

/// 列出正在執行的 WSL 發行版（spec `repo-projects`「納入 repo 判定的 pane 與更新時機」：系統不得為了判定 repo 而
/// 啟動 WSL 發行版）。查詢目標是 WSL 時，resolver 先以它確認目標發行版在跑；正式實作 [`WslRunningDistros`]，
/// 測試用假清單。
pub trait RunningDistros: Send + Sync + 'static {
    /// 正在執行的發行版名稱；探測失敗回 `None`（resolver 視為都沒在跑）。
    fn running(&self) -> impl Future<Output = Option<Vec<String>>> + Send;
}

/// 正式的探測實作：跑 `wsl.exe --list --running --quiet`（只列清單，不會把發行版開機；與 `cockpit-herdr` 的
/// WSL 探測共用同一支指令與 UTF-16LE／UTF-8 解碼）。
pub struct WslRunningDistros;

impl RunningDistros for WslRunningDistros {
    async fn running(&self) -> Option<Vec<String>> {
        // 逾時由 resolver 端的 `tokio::time::timeout` 負責；逾時時這個 future 被 drop，`kill_on_drop` 讓卡住的
        // `wsl.exe` 一併結束，不留下孤兒程序。
        match cockpit_herdr::probe::list_running(true).await {
            Ok(outcome) if outcome.status_ok => {
                Some(cockpit_herdr::probe::decode_list(&outcome.stdout))
            }
            Ok(outcome) => {
                let stderr = String::from_utf8_lossy(&outcome.stderr);
                tracing::debug!(stderr = %stderr.trim(), "WSL 探測非零結束，視為沒有發行版在執行");
                None
            }
            Err(error) => {
                tracing::debug!(%error, "WSL 探測無法執行，視為沒有發行版在執行");
                None
            }
        }
    }
}

/// resolver 的輸出端（正式實作是 [`ProgressService`]）。
pub trait PaneReposSink: Send + Sync + 'static {
    /// 換上新的歸類結果（寫入服務的 `set_pane_repos`）。
    fn submit(&self, pane_repos: PaneRepos) -> impl Future<Output = Result<(), WriteError>> + Send;
    /// 請求一次消失 pane 的進度清除檢查（寫入服務的 `clear_vanished_progress`）。
    fn request_clear(&self) -> impl Future<Output = Result<(), WriteError>> + Send;
}

impl PaneReposSink for ProgressService {
    async fn submit(&self, pane_repos: PaneRepos) -> Result<(), WriteError> {
        self.set_pane_repos(pane_repos).await
    }

    async fn request_clear(&self) -> Result<(), WriteError> {
        self.clear_vanished_progress().await
    }
}

/// 起 resolver 背景工作。它不會自己結束（投影的 watch 傳送端活得跟 `StoreHandle` 一樣久），關機時由
/// `app::shutdown_all` `abort()` 再 `await`。
pub fn spawn_repo_resolver<L: RepoLookup, D: RunningDistros, S: PaneReposSink>(
    state: watch::Receiver<Arc<ProjectedState>>,
    mappings: Arc<HashMap<RuntimeId, PathMapping>>,
    lookup: L,
    distros: D,
    sink: S,
) -> JoinHandle<()> {
    let resolver = Resolver {
        mappings,
        lookup,
        distros,
        sink,
        cache: HashMap::new(),
        last_computed: PaneRepos::new(),
        last_sent: PaneRepos::new(),
        submit_failures: FailureLog::default(),
        clear_failures: FailureLog::default(),
        distro_probe: None,
    };
    tokio::spawn(resolver.run(state))
}

/// 一個 `(runtime, cwd)` 的快取項目：歸類結果（`None`＝不是 repo、暫時錯誤或路徑轉不出來）與過期時間。
struct CacheEntry {
    repo: Option<PaneRepo>,
    expires_at: Instant,
}

struct Resolver<L, D, S> {
    mappings: Arc<HashMap<RuntimeId, PathMapping>>,
    lookup: L,
    distros: D,
    sink: S,
    cache: HashMap<(RuntimeId, String), CacheEntry>,
    /// 上一輪算出的結果；未連線 runtime 的 pane 沿用這裡的歸類。
    last_computed: PaneRepos,
    /// 上一次成功交給 `sink` 的內容；寫入服務的 `pane_repos` 啟動時為空，所以初值也是空。
    last_sent: PaneRepos,
    /// `submit` 連續失敗的 log 降噪。
    submit_failures: FailureLog,
    /// `request_clear` 連續失敗的 log 降噪。
    clear_failures: FailureLog,
    /// 最近一次 WSL 探測的結果（跨輪沿用到過期）；`None`＝還沒探測或已過期。
    distro_probe: Option<DistroProbe>,
}

/// 一次 WSL 探測的結果：正在執行的發行版（逾時或失敗為空）與過期時間。
struct DistroProbe {
    running: Vec<String>,
    expires_at: Instant,
}

impl<L: RepoLookup, D: RunningDistros, S: PaneReposSink> Resolver<L, D, S> {
    async fn run(mut self, mut state: watch::Receiver<Arc<ProjectedState>>) {
        loop {
            // 先把這一份投影標成已讀再開始：這一輪期間的變動會讓下面的 `changed()` 立刻完成，不會漏。
            let current = Arc::clone(&state.borrow_and_update());
            self.round(&current, &state).await;
            tokio::select! {
                changed = state.changed() => {
                    if changed.is_err() {
                        return;
                    }
                }
                () = tokio::time::sleep(ROUND_TIMER) => {}
            }
        }
    }

    /// `state` 是這一輪開始時的投影；`latest` 是同一個 watch 頻道，查 git 前用它讀**最新**投影重新確認連線。
    async fn round(
        &mut self,
        state: &ProjectedState,
        latest: &watch::Receiver<Arc<ProjectedState>>,
    ) {
        let now = Instant::now();
        self.cache.retain(|_, entry| entry.expires_at > now);
        if self
            .distro_probe
            .as_ref()
            .is_some_and(|probe| probe.expires_at <= now)
        {
            self.distro_probe = None;
        }

        let mappings = Arc::clone(&self.mappings);
        let mut computed = PaneRepos::new();
        'runtimes: for runtime in &state.runtimes {
            let Some(mapping) = mappings.get(&runtime.id) else {
                continue;
            };
            if !is_connected(&runtime.connection) {
                self.carry_over(&runtime.id, &mut computed);
                continue;
            }
            let panes = runtime
                .workspaces
                .iter()
                .flat_map(|workspace| &workspace.tabs)
                .flat_map(|tab| &tab.panes);
            for pane in panes {
                if pane.exited {
                    continue;
                }
                let Some(cwd) = pane.cwd.as_deref().filter(|cwd| !cwd.is_empty()) else {
                    continue;
                };
                let key = (runtime.id.clone(), pane.id.clone());
                match self.resolve(&runtime.id, mapping, cwd, latest).await {
                    Resolution::Classified(Some(repo)) => {
                        computed.insert(key, repo);
                    }
                    Resolution::Classified(None) => {}
                    Resolution::DistroNotRunning => {
                        if let Some(repo) = self.last_computed.get(&key) {
                            computed.insert(key, repo.clone());
                        }
                    }
                    Resolution::RuntimeNotConnected => {
                        // 一輪進行中斷線：丟掉這個 runtime 這一輪的部分結果，整個改走沿用分支。
                        computed.retain(|(id, _), _| *id != runtime.id);
                        self.carry_over(&runtime.id, &mut computed);
                        continue 'runtimes;
                    }
                }
            }
        }
        self.last_computed = computed.clone();

        if computed != self.last_sent {
            let result = self.sink.submit(computed.clone()).await;
            if result.is_ok() {
                self.last_sent = computed;
            }
            // 記憶體沒變；`last_sent` 不更新，下一輪會再送。
            self.submit_failures
                .report(&result, "更新 pane 的 repo 歸類失敗，下一輪重試");
        }
        let result = self.sink.request_clear().await;
        self.clear_failures
            .report(&result, "清除消失 pane 的進度失敗，下一輪重試");
    }

    /// 不查 git（對斷線的 WSL runtime 執行 `wsl.exe -d` 可能把發行版開機）：沿用上一輪對這個 runtime 的歸類。
    fn carry_over(&self, runtime: &RuntimeId, computed: &mut PaneRepos) {
        computed.extend(
            self.last_computed
                .iter()
                .filter(|((id, _), _)| id == runtime)
                .map(|(key, repo)| (key.clone(), repo.clone())),
        );
    }

    /// 查快取，沒有（或已在這一輪開頭被剔除的過期項目）才查 git。cwd 轉不出主機路徑或執行目標時不查、不快取。
    ///
    /// 查 git 之前從 `latest` 讀最新投影，確認 runtime **仍**已連線（一輪可能跑好幾秒，期間 WSL 可能被
    /// `wsl --shutdown`）；不是就回 [`Resolution::RuntimeNotConnected`]、不查。投影本身落後 `RuntimeStore` 一個合併窗
    /// （50 ms），這個窗內的斷線擋不住。
    ///
    /// 查詢目標是 WSL 時（WSL runtime 的 pane，或 Windows runtime 的 pane 停在 `\\wsl.localhost\`／`\\wsl$\` 路徑），
    /// 另外確認目標發行版正在執行：pane 所屬的 runtime 已連線不代表目標發行版在跑，對沒在跑的發行版執行
    /// `wsl.exe -d` 會把它開機。探測結果見 [`Self::running_distros`]（跨輪沿用、逾時與失敗視為沒在跑）；
    /// 沒在跑就回 [`Resolution::DistroNotRunning`]、不查、不快取。
    async fn resolve(
        &mut self,
        runtime: &RuntimeId,
        mapping: &PathMapping,
        cwd: &str,
        latest: &watch::Receiver<Arc<ProjectedState>>,
    ) -> Resolution {
        let key = (runtime.clone(), cwd.to_string());
        if let Some(entry) = self.cache.get(&key) {
            return Resolution::Classified(entry.repo.clone());
        }
        let Some(target) = mapping
            .host_path(cwd)
            .and_then(|host| select_target(host.to_str()?).ok())
        else {
            return Resolution::Classified(None);
        };
        let still_connected = latest
            .borrow()
            .runtimes
            .iter()
            .find(|candidate| candidate.id == *runtime)
            .is_some_and(|candidate| is_connected(&candidate.connection));
        if !still_connected {
            return Resolution::RuntimeNotConnected;
        }
        if let GitTarget::Wsl { distro, .. } = &target {
            let running = self.running_distros().await;
            if !running.iter().any(|name| name.eq_ignore_ascii_case(distro)) {
                tracing::debug!(%runtime, %distro, "WSL 發行版沒在執行，不查 git（不為判定 repo 開機）");
                return Resolution::DistroNotRunning;
            }
        }

        // 過期時間從查詢開始算：定時器在一輪結束後才開始計時，所以 60 秒後那一輪一定看得到它已過期。
        let started = Instant::now();
        let (repo, ttl) = match self.lookup.lookup(&target).await {
            LookupOutcome::Repo(output) => match classify(&target, &output) {
                Some(repo) => (Some(repo), SUCCESS_TTL),
                None => {
                    tracing::debug!(%runtime, "git 回報的路徑無法轉成 repo key，暫不歸類");
                    (None, RETRY_TTL)
                }
            },
            LookupOutcome::NotRepo => (None, RETRY_TTL),
            LookupOutcome::Unavailable(reason) => {
                tracing::debug!(%runtime, %reason, "暫時無法判定 pane 所屬 repo，稍後重試");
                (None, RETRY_TTL)
            }
        };
        self.cache.insert(
            key,
            CacheEntry {
                repo: repo.clone(),
                expires_at: started + ttl,
            },
        );
        Resolution::Classified(repo)
    }
}

impl<L: RepoLookup, D: RunningDistros, S: PaneReposSink> Resolver<L, D, S> {
    /// 正在執行的 WSL 發行版。沿用未過期的上次結果；否則探測一次，結果（在跑／沒在跑都一樣）從探測**開始**起算
    /// [`RETRY_TTL`] 後過期——pane 停在已停止發行版的 UNC 路徑時，不會每次投影改變都起一支 `wsl.exe`。探測超過
    /// [`DISTRO_PROBE_TIMEOUT`] 就放棄（`wsl.exe` 卡住時不讓整個 resolver 停住，原生 pane 照常歸類）；逾時與失敗都視為
    /// 沒有發行版在執行。
    async fn running_distros(&mut self) -> &[String] {
        if self.distro_probe.is_none() {
            let started = Instant::now();
            let running =
                match tokio::time::timeout(DISTRO_PROBE_TIMEOUT, self.distros.running()).await {
                    Ok(running) => running.unwrap_or_default(),
                    Err(_) => {
                        tracing::debug!("WSL 探測逾時，視為沒有發行版在執行");
                        Vec::new()
                    }
                };
            self.distro_probe = Some(DistroProbe {
                running,
                expires_at: started + RETRY_TTL,
            });
        }
        self.distro_probe
            .as_ref()
            .map_or(&[], |probe| probe.running.as_slice())
    }
}

/// [`Resolver::resolve`] 的結果。
enum Resolution {
    /// 有結果（快取或剛查完）；`None`＝不歸類。
    Classified(Option<PaneRepo>),
    /// 查 git 前發現 runtime 已不是已連線：沒有查。
    RuntimeNotConnected,
    /// 查詢目標是 WSL，但目標發行版沒在執行（或探測失敗）：沒有查；這個 pane 沿用上一輪的歸類。
    DistroNotRunning,
}

fn is_connected(connection: &ProjectedConnection) -> bool {
    matches!(connection, ProjectedConnection::Connected { .. })
}

/// 連續失敗只在第一次記 warn（之後降為 debug），成功後重置並記一次 info。狀態檔持續寫不進去時，每一輪都會重試，
/// 不這樣做會每輪洗一次 warn（比照 `cockpit-core` 投影任務的 `closed_logged`）。
#[derive(Default)]
struct FailureLog {
    failing: bool,
}

impl FailureLog {
    fn report(&mut self, result: &Result<(), WriteError>, message: &'static str) {
        if let Some(level) = self.observe(result.is_err()) {
            match (level, result) {
                (LogLevel::Warn, Err(error)) => tracing::warn!(%error, "{message}"),
                (LogLevel::Debug, Err(error)) => tracing::debug!(%error, "{message}（持續失敗）"),
                (_, _) => tracing::info!("{message}：已恢復"),
            }
        }
    }

    /// 依這次是否失敗決定要記哪一級：第一次失敗 `Warn`、持續失敗 `Debug`、失敗後第一次成功 `Info`、一直成功不記。
    fn observe(&mut self, failed: bool) -> Option<LogLevel> {
        let level = match (self.failing, failed) {
            (false, true) => Some(LogLevel::Warn),
            (true, true) => Some(LogLevel::Debug),
            (true, false) => Some(LogLevel::Info),
            (false, false) => None,
        };
        self.failing = failed;
        level
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LogLevel {
    Warn,
    Debug,
    Info,
}

/// git 回報的原始路徑 → repo key（design D2）。`target` 是 WSL 時 `raw` 是該 distro 內的 POSIX 路徑；轉不出來
/// （含 Windows 路徑無法表示的字元）回 `None`。
pub fn repo_key(target: &GitTarget, raw: &str) -> Option<String> {
    match target {
        // NTFS 不分大小寫：同一個資料夾以不同大小寫進入時 git 回的路徑大小寫可能不同，整串轉小寫才比對得起來。
        GitTarget::Native { .. } => Some(raw.replace('/', "\\").to_lowercase()),
        // Linux 區分大小寫：保留原樣。
        GitTarget::Wsl { distro, .. } => wsl_host_path(distro, raw)?.to_str().map(str::to_string),
    }
}

/// 路徑的最後一段資料夾名稱（`/` 與 `\` 都當分隔符，略過空段）。
fn last_segment(raw: &str) -> Option<&str> {
    raw.split(['/', '\\']).rfind(|segment| !segment.is_empty())
}

/// 共同 `.git` 目錄的原始路徑 → 預設名稱（design D1）：名稱是 `.git` 時取上一層資料夾名稱，否則取它的名稱並去掉
/// 結尾的 `.git`。保留 git 原始輸出的大小寫。取不出來（例如 `/.git`）時退回最後一段原樣。
pub fn default_repo_name(common_dir: &str) -> String {
    let mut segments = common_dir
        .split(['/', '\\'])
        .filter(|segment| !segment.is_empty())
        .rev();
    let Some(last) = segments.next() else {
        return common_dir.to_string();
    };
    let name = if last == ".git" {
        segments.next().unwrap_or(last)
    } else {
        last.strip_suffix(".git")
            .filter(|stem| !stem.is_empty())
            .unwrap_or(last)
    };
    name.to_string()
}

/// 一次成功查詢的原始輸出 → 歸類結果（repo key、預設名稱、worktree 標註；design D1、D2）。路徑轉不出來時回
/// `None`（該 pane 不歸類）。
///
/// worktree 標註：自己的 git 目錄（正規化後）不等於共同目錄（正規化後）時，取工作樹根目錄的資料夾名稱。submodule
/// 的兩者相同（`.git/modules/<名稱>`），不會被誤標。
pub fn classify(target: &GitTarget, output: &RepoIdentityOutput) -> Option<PaneRepo> {
    let key = repo_key(target, &output.common_dir)?;
    let git_dir = repo_key(target, &output.git_dir)?;
    let worktree = if git_dir == key {
        None
    } else {
        Some(last_segment(&output.toplevel)?.to_string())
    };
    Some(PaneRepo {
        repo: RepoKey::new(key),
        default_name: default_repo_name(&output.common_dir),
        worktree,
    })
}

#[cfg(test)]
mod tests {
    use super::{FailureLog, LogLevel, classify, default_repo_name, repo_key};

    use cockpit_core::{PaneRepo, RepoKey};
    use cockpit_git::{GitTarget, RepoIdentityOutput};

    /// 狀態檔持續寫不進去時每一輪都會重試：連續失敗只有第一次是 warn，之後降為 debug；成功後重置，再失敗又是 warn。
    #[test]
    fn consecutive_failures_warn_only_once() {
        let mut log = FailureLog::default();
        assert_eq!(log.observe(false), None, "一直成功不記");
        assert_eq!(log.observe(true), Some(LogLevel::Warn), "第一次失敗");
        for _ in 0..20 {
            assert_eq!(
                log.observe(true),
                Some(LogLevel::Debug),
                "持續失敗不再 warn"
            );
        }
        assert_eq!(log.observe(false), Some(LogLevel::Info), "恢復時記一次");
        assert_eq!(log.observe(false), None);
        assert_eq!(
            log.observe(true),
            Some(LogLevel::Warn),
            "恢復後再失敗重新 warn"
        );
    }

    fn native() -> GitTarget {
        GitTarget::Native {
            path: r"D:\work\app".to_string(),
        }
    }

    fn wsl(distro: &str) -> GitTarget {
        GitTarget::Wsl {
            distro: distro.to_string(),
            posix: "/home/u/app".to_string(),
        }
    }

    fn output(common_dir: &str, git_dir: &str, toplevel: &str) -> RepoIdentityOutput {
        RepoIdentityOutput {
            common_dir: common_dir.to_string(),
            git_dir: git_dir.to_string(),
            toplevel: toplevel.to_string(),
        }
    }

    fn repo(key: &str, name: &str, worktree: Option<&str>) -> PaneRepo {
        PaneRepo {
            repo: RepoKey::new(key),
            default_name: name.to_string(),
            worktree: worktree.map(str::to_string),
        }
    }

    #[test]
    fn windows_key_uses_backslashes_and_lowercase() {
        assert_eq!(
            repo_key(&native(), "D:/Work/App/.git").as_deref(),
            Some(r"d:\work\app\.git")
        );
    }

    /// spec「大小寫不同的路徑歸同一個 repo」：key 相同，預設名稱保留 git 原始輸出的大小寫。
    #[test]
    fn same_windows_folder_in_different_case_is_one_repo() {
        let upper = classify(
            &native(),
            &output("D:/Work/App/.git", "D:/Work/App/.git", "D:/Work/App"),
        )
        .expect("應歸類");
        let lower = classify(
            &native(),
            &output("d:/work/app/.git", "d:/work/app/.git", "d:/work/app"),
        )
        .expect("應歸類");
        assert_eq!(upper.repo, lower.repo);
        assert_eq!(upper.repo, RepoKey::new(r"d:\work\app\.git"));
        assert_eq!(upper.default_name, "App");
        assert_eq!(lower.default_name, "app");
    }

    /// spec「WSL 的 repo key」「WSL 的 repo key 保留大小寫」。
    #[test]
    fn wsl_key_is_unc_path_and_keeps_case() {
        let upper = classify(
            &wsl("Ubuntu"),
            &output("/home/u/App/.git", "/home/u/App/.git", "/home/u/App"),
        )
        .expect("應歸類");
        let lower = classify(
            &wsl("Ubuntu"),
            &output("/home/u/app/.git", "/home/u/app/.git", "/home/u/app"),
        )
        .expect("應歸類");
        assert_eq!(
            upper,
            repo(r"\\wsl.localhost\Ubuntu\home\u\App\.git", "App", None)
        );
        assert_eq!(
            lower,
            repo(r"\\wsl.localhost\Ubuntu\home\u\app\.git", "app", None)
        );
        assert_ne!(upper.repo, lower.repo);
    }

    #[test]
    fn wsl_path_that_windows_cannot_represent_is_not_classified() {
        assert_eq!(repo_key(&wsl("Ubuntu"), "/home/u/a:b/.git"), None);
        assert_eq!(repo_key(&wsl("Ubuntu"), "relative/.git"), None);
        assert_eq!(
            classify(
                &wsl("Ubuntu"),
                &output("/home/u/a:b/.git", "/home/u/a:b/.git", "/home/u/a:b"),
            ),
            None
        );
    }

    /// spec「同一個 repo 的 worktree 歸在一起」。
    #[test]
    fn linked_worktree_shares_key_and_is_labelled_with_its_root_folder() {
        let main = classify(
            &native(),
            &output("D:/work/app/.git", "D:/work/app/.git", "D:/work/app"),
        )
        .expect("應歸類");
        let linked = classify(
            &native(),
            &output(
                "D:/work/app/.git",
                "D:/work/app/.git/worktrees/app-wt",
                "D:/work/app-wt",
            ),
        )
        .expect("應歸類");
        assert_eq!(main, repo(r"d:\work\app\.git", "app", None));
        assert_eq!(linked, repo(r"d:\work\app\.git", "app", Some("app-wt")));
    }

    /// 自己的 git 目錄與共同目錄只差大小寫時是同一個目錄（正規化後比較），不是 linked worktree。
    #[test]
    fn git_dir_differing_only_in_case_is_not_a_worktree() {
        let pane = classify(
            &native(),
            &output("D:/Work/App/.git", "d:/work/app/.git", "D:/Work/App"),
        )
        .expect("應歸類");
        assert_eq!(pane.worktree, None);
    }

    /// spec「submodule 不是 linked worktree」「預設名稱」。
    #[test]
    fn submodule_is_not_a_worktree_and_is_named_after_its_module() {
        let pane = classify(
            &native(),
            &output(
                "D:/work/super/.git/modules/lib",
                "D:/work/super/.git/modules/lib",
                "D:/work/super/lib",
            ),
        )
        .expect("應歸類");
        assert_eq!(pane, repo(r"d:\work\super\.git\modules\lib", "lib", None));
    }

    #[test]
    fn wsl_linked_worktree_is_labelled() {
        let pane = classify(
            &wsl("Ubuntu"),
            &output(
                "/home/u/app/.git",
                "/home/u/app/.git/worktrees/feat",
                "/home/u/Feat",
            ),
        )
        .expect("應歸類");
        assert_eq!(
            pane,
            repo(
                r"\\wsl.localhost\Ubuntu\home\u\app\.git",
                "app",
                Some("Feat")
            )
        );
    }

    #[test]
    fn default_name_rules() {
        assert_eq!(default_repo_name("D:/work/app/.git"), "app");
        assert_eq!(default_repo_name("D:/work/super/.git/modules/lib"), "lib");
        assert_eq!(default_repo_name("D:/repos/Tool.git"), "Tool");
        assert_eq!(default_repo_name("/home/u/App/.git"), "App");
    }
}
