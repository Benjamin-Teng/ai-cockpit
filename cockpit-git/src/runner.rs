//! git 子程序執行器（git-review design D3）：並行上限、含排隊的逾時、stdout 邊讀邊數的上限
//! 與可截斷查詢的前段結果、stderr 只進日誌、無法啟動／dubious ownership／逾時／其他非零的
//! 錯誤分類。
//!
//! # crate 外部入口的封閉性（fix round 1，design D1 Goal「HTTP 層拿不到任意 git 引數的入口」）
//!
//! 正式建置（未開 `test-support` feature）下，crate 外**唯一**能讓 [`GitRunner`] 啟動子程序
//! 的路徑是 [`GitRunner::run`]：它只吃 `Q: GitQuery`（sealed trait，crate 外無法命名或實作，
//! 見 `query.rs`「`GitQuery`」文件的 `compile_fail` doctest），argv 完全由 crate 內 10 個查詢
//! 型別各自的 `commands()` 決定，呼叫端沒有「自訂 argv」的入口。
//!
//! 這個保證原本會被 [`QueryPlan`] 破壞——它的三個欄位（`calls`／`stdout_cap`／
//! `truncatable`）一開始是 `pub`，`GitRunner` 也曾經公開一個直接吃 `QueryPlan` 的
//! `execute` 方法，等於任何依賴 `cockpit-git` 的 crate 都能自己組一個帶任意程式與任意引數
//! 的 `QueryPlan` 丟給執行器，完全繞過 sealed `GitQuery`。fix round 1 修正：`QueryPlan`
//! 的欄位改為私有，唯一能以任意 argv 建構它的建構子（[`QueryPlan::for_test`]）與唯一能
//! 直接執行它的方法（`GitRunner::execute`）都加了 `#[cfg(feature = "test-support")]`，只在
//! 開這個 feature 時才存在——正式建置（`cargo build`／一般依賴 `cockpit-git` 的 crate）看不到
//! 它們，只有 `cockpit-git/tests/runner.rs` 用得到（sealed trait 無法在獨立編譯的整合測試
//! crate 裡實作，所以那裡本來就得改用假程式的 argv 直接組 `QueryPlan` 測試，這是唯一合法的
//! 例外用途）。
//!
//! **這裡沒有放 `compile_fail` doctest 證明上面這段話**：實測發現 `cargo test -p
//! cockpit-git` 的 doctest 編譯**看得到** `test-support` feature（`Cargo.toml` 的
//! `[dev-dependencies]` 對自己開了這個 feature 供 `tests/runner.rs` 用，而 doctest 與
//! 一般測試共用同一次 `cargo test` 的 feature 解析結果，不像 `cargo build`／一般依賴
//! `cockpit-git` 的 crate 那樣看不到 dev-dependency 的 feature）——寫一個
//! `QueryPlan::for_test(..)` 的 `compile_fail` doctest 在這個環境下會編譯成功，doctest 本身
//! 反而失敗（「Test compiled successfully, but it's marked `compile_fail`」）。改用一個
//! **真正在 crate 外、未開 `test-support` feature** 的暫時 consumer crate 驗證（`cargo
//! check`，指令與輸出見 `task-2.2-report.md`「Fix round 1」）：`QueryPlan { .. }` struct
//! literal → `E0451`（私有欄位）、`QueryPlan::for_test(..)` → `E0599`（方法不存在）、
//! `runner.execute(..)` → `E0599`（方法不存在）、自訂型別實作 `GitQuery` → `E0277`
//! （sealed trait bound 不滿足）——四條路徑都證實在正式建置下無法使用。
//!
//! # 架構決定：`run`／`execute` 對多次呼叫查詢（`Refs`、`ChangedFiles`）的處理
//!
//! design D4 的 `Refs` 用 `rev-parse --verify -q HEAD` 取得 HEAD 的 hash——在還沒有任何
//! commit 的 repo（`HEAD` 是懸空的 symbolic ref）這個呼叫**合法地**以非零結束（`-q` 抑制
//! 錯誤訊息，這正是它存在的理由）。這代表「這次呼叫非零結束」不等於「這個查詢失敗」——
//! 是不是失敗要看是哪個查詢、哪個子命令，那是 task 2.3 解析器的知識，不是本模組（task 2.2）
//! 該有的知識。
//!
//! 所以執行器對一個查詢的多次呼叫（`QueryPlan` 內部的 `calls`）逐一執行，**只有**
//! [`RunnerError::Unavailable`]、[`RunnerError::Untrusted`]、[`RunnerError::Timeout`]
//! 這三種「環境本身有問題、換哪個子命令都一樣會發生」的情況會讓整個查詢立即中止並回傳單一
//! 錯誤；[`RunnerError::Failed`]（單純非零結束）與 [`RunnerError::TooLarge`] 則記錄在對應那
//! 次呼叫的位置（[`RunOutput::calls`] 是 `Vec<Result<..>>`），迴圈繼續執行剩下的呼叫。
//! 之後 task 2.3 的解析器可以逐一檢視每次呼叫的結果，把「這個查詢已知合法的非零結束」
//! （`Refs` 的 `rev-parse`／`symbolic-ref`、`MergeBase` 無共同祖先、`cat-file` 物件不存在、
//! `VerifyCommit` 不存在）解讀成正常輸出，其餘才視為真正的錯誤。這個決定影響 task 2.3 怎麼
//! consume `RunOutput`，回報中會特別標注供控制端／Codex 覆核。

use std::fmt;
use std::io;
use std::process::Stdio;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use tokio::io::AsyncReadExt;
use tokio::process::{Child, Command};
use tokio::sync::Semaphore;
use tokio::time::Instant;

use crate::GitTarget;
use crate::query::GitQuery;

/// Windows `CREATE_NO_WINDOW`：啟動子程序不彈出主控台視窗（design D2；同
/// `cockpit-herdr/src/probe.rs`、`herdr-client/src/connector/child_stdio.rs` 的做法）。
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// design D3：stderr 最多保留 8 KiB，只進日誌，不放進回應本體。
const STDERR_TAIL_CAP: usize = 8 * 1024;

/// design D3：同時最多 4 支 git 子程序。
const DEFAULT_CONCURRENCY: usize = 4;

/// design D3：單次查詢逾時 10 秒（含排隊）。
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);

/// design D6：dubious ownership 的 stderr 判別字串（`git-review-probe.md` ⑦ 逐字相符）。
const DUBIOUS_OWNERSHIP_MARKER: &[u8] = b"detected dubious ownership";

/// 一個查詢要執行的所有呼叫與這個查詢的上限設定：從 [`GitQuery`] 取出（`stdout_cap`／
/// `truncatable`），與 [`GitRunner`] 解耦——`GitRunner` 本身不需要知道 sealed trait 的存在，
/// 只需要一批 argv 加兩個設定值。
///
/// **三個欄位都是私有的**（fix round 1）：唯一的正式建構路徑是 [`QueryPlan::from_query`]
/// （只接受 `Q: GitQuery`，argv 由 sealed trait 內的型別決定，crate 外無法繞過）；以任意
/// argv 直接建構的 [`QueryPlan::for_test`] 只在 `test-support` feature 下存在，只給
/// `cockpit-git/tests/runner.rs` 用假程式測試（見本模組文件「crate 外部入口的封閉性」）。
#[derive(Debug, Clone)]
pub struct QueryPlan {
    /// 依序執行的呼叫（每個元素是一次 git 呼叫的完整 argv，含程式名）。
    calls: Vec<Vec<String>>,
    /// 套用到**每一次**呼叫的 stdout 上限（design D3：上限對查詢而非對次數，`Refs`／
    /// `ChangedFiles` 的每次呼叫各自套用同一個上限）。
    stdout_cap: usize,
    /// 超過 `stdout_cap` 時能不能回傳截斷後的前段（design D3：只有 `Status`／`ChangedFiles`）。
    truncatable: bool,
}

impl QueryPlan {
    /// 從一個 [`GitQuery`] 與執行目標建立 [`QueryPlan`]——正式建置下唯一的建構路徑，
    /// `pub(crate)`（不對外公開）：`cockpit`（未來呼叫端）改用 [`GitRunner::run`]，不需要、
    /// 也拿不到 `QueryPlan` 本身。
    pub(crate) fn from_query<Q: GitQuery>(query: &Q, target: &GitTarget) -> QueryPlan {
        QueryPlan {
            calls: query.commands(target),
            stdout_cap: query.stdout_cap(),
            truncatable: query.truncatable(),
        }
    }

    /// 測試專用建構子：直接指定任意 argv／上限／可截斷性，繞過 sealed [`GitQuery`]。
    /// **只在 `test-support` feature 下存在**——正式建置（`cargo build`，或任何依賴
    /// `cockpit-git` 卻未開這個 feature 的 crate）看不到這個方法，也就沒有辦法用它餵任意
    /// 程式與引數給 [`GitRunner`]（design D1 Goal；見本模組文件「crate 外部入口的封閉性」）。
    /// 只給 `cockpit-git/tests/runner.rs` 用：sealed trait 無法在獨立編譯的整合測試 crate
    /// 裡實作，所以那裡改用假程式的 argv 直接組 `QueryPlan` 測逾時／上限／並行等行為。
    #[cfg(feature = "test-support")]
    pub fn for_test(calls: Vec<Vec<String>>, stdout_cap: usize, truncatable: bool) -> QueryPlan {
        QueryPlan {
            calls,
            stdout_cap,
            truncatable,
        }
    }
}

/// 一次呼叫成功時的結果（`truncated: true` 時 `stdout` 只保留前 `stdout_cap` 個位元組）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallOutcome {
    pub stdout: Vec<u8>,
    pub truncated: bool,
}

/// [`GitRunner::run`] 成功時的結果：`QueryPlan` 內部每個呼叫各自的結果。
///
/// 見本模組文件「架構決定」：只有 [`RunnerError::Failed`]／[`RunnerError::TooLarge`] 會出現在
/// 這裡（其餘錯誤會讓整個查詢中止並回傳 `Err`，不會走到這裡）。
#[derive(Debug)]
pub struct RunOutput {
    pub calls: Vec<Result<CallOutcome, RunnerError>>,
}

/// [`GitRunner::run`]／內部單次呼叫的錯誤分類（design D3／D6，D6 的 HTTP 對應由後續
/// task 負責，這裡只提供分類後的 Rust 型別）。
#[derive(Debug)]
pub enum RunnerError {
    /// 子程序無法啟動（找不到程式；D6 對應 `git_unavailable`）。
    Unavailable(io::Error),
    /// stderr 含 `detected dubious ownership`（D6 對應 `git_untrusted`）。
    Untrusted { stderr_tail: Vec<u8> },
    /// 逾時（含排隊；D6 對應 `git_timeout`）。
    Timeout,
    /// 非零結束、且 stderr 不含 dubious ownership 標記（D6 對應 `git_failed`）。
    /// `exit_code` 在正常結束時一定是 `Some`；被我們強制終止（例如逾時）不會走到這個變體。
    Failed {
        exit_code: Option<i32>,
        stderr_tail: Vec<u8>,
    },
    /// 超過查詢的 stdout 上限且該查詢不可截斷（design D3）。
    TooLarge,
}

impl fmt::Display for RunnerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RunnerError::Unavailable(io) => write!(f, "無法啟動子程序：{io}"),
            RunnerError::Untrusted { .. } => write!(f, "git 回報 dubious ownership"),
            RunnerError::Timeout => write!(f, "逾時（含排隊）"),
            RunnerError::Failed { exit_code, .. } => {
                write!(f, "git 以非零結束（exit_code={exit_code:?}）")
            }
            RunnerError::TooLarge => write!(f, "輸出超過上限且不可截斷"),
        }
    }
}

impl std::error::Error for RunnerError {}

/// git 子程序執行器：並行上限（[`tokio::sync::Semaphore`]）＋含排隊的逾時
/// （[`tokio::time::timeout_at`]）＋stdout 邊讀邊數的上限＋錯誤分類（design D3）。
pub struct GitRunner {
    semaphore: Arc<Semaphore>,
    spawned: Arc<AtomicU64>,
    timeout: Duration,
}

impl Default for GitRunner {
    fn default() -> Self {
        GitRunner::new()
    }
}

impl GitRunner {
    /// design D3 預設值：同時最多 4 支、單次查詢逾時 10 秒（含排隊）。正式程式碼一律用這個
    /// 建構子。
    pub fn new() -> GitRunner {
        GitRunner::with_limits(DEFAULT_CONCURRENCY, DEFAULT_TIMEOUT)
    }

    /// 測試用：自訂並行上限與逾時，讓測試不必真的等 10 秒。正式程式碼不應該呼叫這個建構子
    /// （brief「crate 內測試用建構子指定程式路徑」：本 crate 用它搭配假程式的 argv 驗證逾時／
    /// 上限／並行行為，不需要等 design 的正式 10 秒／4 支）。
    pub fn with_limits(concurrency: usize, timeout: Duration) -> GitRunner {
        GitRunner {
            semaphore: Arc::new(Semaphore::new(concurrency)),
            spawned: Arc::new(AtomicU64::new(0)),
            timeout,
        }
    }

    /// 已成功啟動（`spawn()` 沒有回傳 `Err`）的子程序累計次數；供之後 `cockpit` 的端點測試
    /// 斷言「參數錯誤時沒有啟動任何子程序」（control-panel ruling P1）。
    pub fn spawned(&self) -> u64 {
        self.spawned.load(Ordering::Relaxed)
    }

    /// 執行一個查詢——**正式建置下唯一能讓執行器啟動子程序的公開入口**（design D1 Goal
    /// 「HTTP 層拿不到任意 git 引數的入口」；見本模組文件「crate 外部入口的封閉性」）。
    /// `Q: GitQuery` 是 sealed trait，crate 外無法命名或實作它，argv 完全由 crate 內 10 個
    /// 查詢型別各自的 `commands()` 決定。
    pub async fn run<Q: GitQuery>(
        &self,
        query: &Q,
        target: &GitTarget,
    ) -> Result<RunOutput, RunnerError> {
        self.execute_plan(QueryPlan::from_query(query, target))
            .await
    }

    /// 測試專用：直接執行一個手動組出的 [`QueryPlan`]（繞過 [`GitQuery`]）。**只在
    /// `test-support` feature 下存在**，只給 `cockpit-git/tests/runner.rs` 用——sealed
    /// trait 無法在獨立編譯的整合測試 crate 裡實作，所以那裡改用假程式的 argv 直接組
    /// `QueryPlan`（經 [`QueryPlan::for_test`]）驗證逾時／上限／並行等行為。正式建置與任何
    /// 依賴 `cockpit-git` 卻未開這個 feature 的 crate 都看不到這個方法。
    #[cfg(feature = "test-support")]
    pub async fn execute(&self, plan: QueryPlan) -> Result<RunOutput, RunnerError> {
        self.execute_plan(plan).await
    }

    /// 執行一個 [`QueryPlan`] 的所有呼叫（實際邏輯；[`GitRunner::run`]／測試專用的
    /// `execute` 都是它的薄包裝）。見本模組文件「架構決定」：只有
    /// [`RunnerError::Unavailable`]／[`RunnerError::Untrusted`]／[`RunnerError::Timeout`]
    /// 會讓整個查詢中止並回傳 `Err`；其餘（`Failed`／`TooLarge`）記錄在對應呼叫的位置，
    /// 迴圈繼續執行剩下的呼叫。
    async fn execute_plan(&self, plan: QueryPlan) -> Result<RunOutput, RunnerError> {
        let deadline = Instant::now() + self.timeout;
        let mut calls = Vec::with_capacity(plan.calls.len());
        for argv in plan.calls {
            match self
                .run_one(argv, plan.stdout_cap, plan.truncatable, deadline)
                .await
            {
                Ok(outcome) => calls.push(Ok(outcome)),
                Err(err @ (RunnerError::Failed { .. } | RunnerError::TooLarge)) => {
                    calls.push(Err(err));
                }
                Err(systemic) => return Err(systemic),
            }
        }
        Ok(RunOutput { calls })
    }

    /// 執行單一呼叫：排隊（含逾時）→ spawn → 邊讀邊數 stdout／stderr → 分類結果。
    async fn run_one(
        &self,
        argv: Vec<String>,
        stdout_cap: usize,
        truncatable: bool,
        deadline: Instant,
    ) -> Result<CallOutcome, RunnerError> {
        let permit = match tokio::time::timeout_at(deadline, self.semaphore.acquire()).await {
            Ok(Ok(permit)) => permit,
            Ok(Err(_closed)) => {
                unreachable!("GitRunner 不會關閉自己的 semaphore")
            }
            Err(_elapsed) => return Err(RunnerError::Timeout),
        };

        let (program, rest) = argv
            .split_first()
            .expect("argv 不應為空（GitQuery::commands 的實作保證每個呼叫至少有程式名）");

        let mut cmd = Command::new(program);
        cmd.args(rest);
        cmd.stdin(Stdio::null());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());
        cmd.kill_on_drop(true);
        #[cfg(windows)]
        cmd.creation_flags(CREATE_NO_WINDOW);

        let mut child = match cmd.spawn() {
            Ok(child) => child,
            Err(io) => return Err(RunnerError::Unavailable(io)),
        };
        self.spawned.fetch_add(1, Ordering::Relaxed);
        // 持有 permit 至本函式結束（含下面的逾時／讀取／等待）才釋放；用底線前綴變數名避免
        // 「未使用」warning，同時清楚表達「只是要它活著」的用途。
        let _permit = permit;

        match tokio::time::timeout_at(deadline, communicate(&mut child, stdout_cap)).await {
            Ok(Ok(raw)) => classify(raw, truncatable),
            Ok(Err(io)) => Err(RunnerError::Failed {
                exit_code: None,
                stderr_tail: format!("執行期間發生非預期的 I/O 錯誤：{io}").into_bytes(),
            }),
            Err(_elapsed) => {
                // brief 驗收：逾時後要確定子程序已被終止——`kill()`（非 `start_kill()`）
                // 會等到子程序真的結束（reap）才回傳，不是只送出終止要求就算數。
                let _ = child.kill().await;
                Err(RunnerError::Timeout)
            }
        }
    }
}

/// spawn 之後、分類之前的原始結果。
struct RawOutcome {
    stdout: Vec<u8>,
    truncated: bool,
    /// 正常結束（不論 exit code）時是 `Some`；因超過 stdout 上限被我們主動終止時是 `None`
    /// （這時 `truncated` 一定是 `true`，呼叫端不會去看 `exit_code`）。
    exit_code: Option<i32>,
    stderr_tail: Vec<u8>,
}

/// 併發讀 stdout（上限 `stdout_cap`）與 stderr（只保留前 [`STDERR_TAIL_CAP`]、其餘排空丟棄，僅供日誌／錯誤分類
/// 用），依 stdout 是否超過上限決定要不要終止子程序，否則等待自然結束。
///
/// stdout／stderr 個別用 `tokio::spawn` 讀（而不是用 `tokio::join!` 同時等兩者都讀完）：
/// 如果 stdout 超過上限，我們必須**不等 stderr 讀完**就能得知「該終止了」——若改用
/// `tokio::join!`，子程序可能因為我們不再讀 stdout（pipe 滿了）而卡住不結束，stderr 那端
/// 又還沒有東西可讀或還沒等到 EOF，會造成死結。用 `tokio::spawn` 讓 stdout 的讀取獨立完成，
/// 我們拿到「超過上限」的結論後立刻 `kill()`，stderr 那個背景 task 才會因為 pipe 被關閉而
/// 很快讀到 EOF。
async fn communicate(child: &mut Child, stdout_cap: usize) -> io::Result<RawOutcome> {
    let stdout = child.stdout.take().expect("stdout 應為 piped");
    let stderr = child.stderr.take().expect("stderr 應為 piped");

    let stdout_task = tokio::spawn(read_capped(stdout, stdout_cap));
    let stderr_task = tokio::spawn(read_head_and_drain(stderr, STDERR_TAIL_CAP));

    let (stdout_bytes, stdout_truncated) = stdout_task
        .await
        .expect("讀 stdout 的背景 task 不應 panic")?;

    if stdout_truncated {
        // 已經確定超過上限：終止子程序，不必等它自然結束；kill() 會關閉它的 pipe，
        // 讓 stderr 背景 task 很快讀到 EOF。
        let _ = child.kill().await;
        // stderr 現在讀到 EOF 才結束；保險起見設短逾時，避免孫行程仍握著 pipe 時卡住。
        let stderr_bytes =
            match tokio::time::timeout(std::time::Duration::from_secs(2), stderr_task).await {
                Ok(Ok(Ok((bytes, _)))) => bytes,
                _ => Vec::new(),
            };
        Ok(RawOutcome {
            stdout: stdout_bytes,
            truncated: true,
            exit_code: None,
            stderr_tail: stderr_bytes,
        })
    } else {
        let status = child.wait().await?;
        let stderr_bytes = match stderr_task.await {
            Ok(Ok((bytes, _))) => bytes,
            _ => Vec::new(),
        };
        Ok(RawOutcome {
            stdout: stdout_bytes,
            truncated: false,
            exit_code: status.code(),
            stderr_tail: stderr_bytes,
        })
    }
}

/// 邊讀邊數位元組，最多讀到 `cap + 1` 個位元組就停止（不等 EOF）——同
/// `cockpit-files::capped_read::read_capped` 的理由：以實際讀到的位元組數為準，不依賴事先
/// 知道大小，避免先看大小再整份讀的競態。回傳 `(bytes, truncated)`：`truncated` 為 `true`
/// 時 `bytes.len() == cap`（多讀到的那 1 個位元組被丟棄，只保留前 `cap` 個）。
async fn read_capped<R>(mut reader: R, cap: usize) -> io::Result<(Vec<u8>, bool)>
where
    R: tokio::io::AsyncRead + Unpin,
{
    let mut buf = Vec::new();
    let mut chunk = [0u8; 64 * 1024];
    loop {
        let n = reader.read(&mut chunk).await?;
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..n]);
        if buf.len() > cap {
            break;
        }
    }
    let truncated = buf.len() > cap;
    if truncated {
        buf.truncate(cap);
    }
    Ok((buf, truncated))
}

/// 讀 stderr：保留前 `cap` 個位元組（僅供日誌／錯誤分類），但**一律讀到 EOF**，超過的部分丟棄。
///
/// 不能像 [`read_capped`] 那樣讀滿就結束：讀端一關，git 之後再寫 stderr 就會寫入失敗（unix 上
/// SIGPIPE、exit 13），本來成功的查詢會被誤判為失敗（例如 `core.autocrlf=true` 時每個檔案一行
/// CRLF 警告，400 個檔約 51 KB，遠超 8 KiB 上限）。stdout 超過上限時終止子程序的行為不變，
/// 那時 pipe 由 `kill()` 關閉、這裡自然讀到 EOF。
async fn read_head_and_drain<R>(mut reader: R, cap: usize) -> io::Result<(Vec<u8>, bool)>
where
    R: tokio::io::AsyncRead + Unpin,
{
    let mut buf = Vec::new();
    let mut truncated = false;
    let mut chunk = [0u8; 8 * 1024];
    loop {
        let n = reader.read(&mut chunk).await?;
        if n == 0 {
            break;
        }
        let room = cap.saturating_sub(buf.len());
        if room >= n {
            buf.extend_from_slice(&chunk[..n]);
        } else {
            buf.extend_from_slice(&chunk[..room]);
            truncated = true;
        }
    }
    Ok((buf, truncated))
}

/// 把 [`RawOutcome`] 分類成 [`CallOutcome`] 或 [`RunnerError`]（design D3／D6）。
fn classify(raw: RawOutcome, truncatable: bool) -> Result<CallOutcome, RunnerError> {
    if raw.truncated {
        return if truncatable {
            Ok(CallOutcome {
                stdout: raw.stdout,
                truncated: true,
            })
        } else {
            Err(RunnerError::TooLarge)
        };
    }

    match raw.exit_code {
        Some(0) => Ok(CallOutcome {
            stdout: raw.stdout,
            truncated: false,
        }),
        other => {
            if contains(&raw.stderr_tail, DUBIOUS_OWNERSHIP_MARKER) {
                Err(RunnerError::Untrusted {
                    stderr_tail: raw.stderr_tail,
                })
            } else {
                Err(RunnerError::Failed {
                    exit_code: other,
                    stderr_tail: raw.stderr_tail,
                })
            }
        }
    }
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    if needle.is_empty() || haystack.len() < needle.len() {
        return needle.is_empty();
    }
    haystack.windows(needle.len()).any(|w| w == needle)
}

#[cfg(test)]
mod tests {
    use super::{QueryPlan, RunnerError};
    use crate::query::{ChangedFiles, GitQuery, Status};
    use crate::{GitTarget, Side};

    fn native() -> GitTarget {
        GitTarget::Native {
            path: r"C:\repo".to_string(),
        }
    }

    /// `QueryPlan::from_query` 只是把 [`GitQuery`] 的三個方法原樣接到 `QueryPlan` 的三個
    /// 欄位——這裡用真正的 sealed 型別驗證接線正確，不需要子程序。
    #[test]
    fn from_query_copies_commands_cap_and_truncatable() {
        let target = native();

        let status_plan = QueryPlan::from_query(&Status, &target);
        assert_eq!(status_plan.calls, Status.commands(&target));
        assert_eq!(status_plan.stdout_cap, Status.stdout_cap());
        assert!(status_plan.truncatable);

        let changed_files = ChangedFiles::new(Side::Index, Side::Worktree).unwrap();
        let changed_files_plan = QueryPlan::from_query(&changed_files, &target);
        assert_eq!(changed_files_plan.calls, changed_files.commands(&target));
        assert_eq!(changed_files_plan.calls.len(), 2);
        assert!(changed_files_plan.truncatable);
    }

    #[test]
    fn runner_error_display_mentions_kind() {
        let err = RunnerError::Timeout;
        assert!(format!("{err}").contains('逾'));
    }
}
