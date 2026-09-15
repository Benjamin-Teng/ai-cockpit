//! `FakeHerdr`：程序內的假 HERDR server，監聽真實 transport（Windows named pipe、unix
//! socket），供本 crate 與 change 1b 在 `test-support` feature 下重用（design D7）。

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use tokio::task::{JoinHandle, JoinSet};

#[cfg(windows)]
use tokio::io::BufReader;
#[cfg(windows)]
use tokio::net::windows::named_pipe::{NamedPipeServer, ServerOptions};

#[cfg(unix)]
use tokio::io::BufReader;
#[cfg(unix)]
use tokio::net::UnixListener;

use crate::connector::Connector;
#[cfg(windows)]
use crate::connector::NamedPipeConnector;
#[cfg(unix)]
use crate::connector::UnixSocketConnector;

use super::config::FakeHerdrConfig;
use super::connection::handle_connection;

/// 一條連線的觀察紀錄：收到的行，以及假 HERDR 這端是否已經關閉這條連線。
#[derive(Default, Clone)]
struct ConnectionRecord {
    lines: Vec<String>,
    closed: bool,
}

/// 每條連線的觀察紀錄，依連線建立順序排列（`FakeHerdr::received`／
/// `FakeHerdr::closed_connections` 用同一組索引）。
#[derive(Default)]
pub(super) struct SharedState {
    connections: Mutex<Vec<ConnectionRecord>>,
}

impl SharedState {
    fn new_connection(&self) -> usize {
        let mut guard = self.connections.lock().expect("connections mutex poisoned");
        guard.push(ConnectionRecord::default());
        guard.len() - 1
    }

    pub(super) fn record_line(&self, index: usize, line: String) {
        let mut guard = self.connections.lock().expect("connections mutex poisoned");
        guard[index].lines.push(line);
    }

    /// 標記第 `index` 條連線已經在假 HERDR 這端結束（handler 正常跑完、或被
    /// `FakeHerdr::drop` 的 `abort_all()` 取消）。由 `handle_connection` 的 drop guard 呼叫，
    /// 所以兩種結束方式都會走到這裡。
    pub(super) fn mark_closed(&self, index: usize) {
        let mut guard = self.connections.lock().expect("connections mutex poisoned");
        if let Some(record) = guard.get_mut(index) {
            record.closed = true;
        }
    }

    fn received(&self) -> Vec<Vec<String>> {
        self.connections
            .lock()
            .expect("connections mutex poisoned")
            .iter()
            .map(|record| record.lines.clone())
            .collect()
    }

    fn closed(&self) -> Vec<bool> {
        self.connections
            .lock()
            .expect("connections mutex poisoned")
            .iter()
            .map(|record| record.closed)
            .collect()
    }
}

/// 目前所有連線 handler task 的集合，與「是否已關閉」的旗標放在同一把鎖裡（fix round 2
/// finding 2）。
///
/// fix round 1 只用 `Arc<Mutex<JoinSet<()>>>`，`FakeHerdr::drop` 對它 `abort_all()`；但
/// `abort_all()` 只能取消**當下已經在集合裡**的 task。多執行緒 runtime 下，accept loop
/// 可能已經從 `accept().await`／`connect().await` 返回（也就是已經接受了一條連線），卻還沒
/// 執行到 `handlers.lock().spawn(task)` 那一行——如果 `FakeHerdr::drop` 剛好在這個「已接受、
/// 尚未註冊」的窗口裡跑完 `abort_all()`，接下來才姍姍來遲的 `spawn(task)` 會把這個新 handler
/// 加進一個「已經 abort 過」但沒有人會再 abort 第二次的 `JoinSet`，這個 handler 就永遠不會
/// 被取消（Codex scoped re-review round 1 finding 2、fix round 2）。
///
/// 修法：`closed` 旗標與 `JoinSet` 共用同一把鎖，且都只在持鎖期間讀寫。`FakeHerdr::drop`
/// 持鎖依序做「設 `closed = true`」與「`abort_all()`」，兩步都在同一次上鎖裡完成；accept
/// loop 每次要註冊新 handler 前也持同一把鎖檢查 `closed`——若已關閉就直接丟棄剛接受的連線
/// （連 `handle_connection` 都不會被 spawn，`io` 隨這次 lock scope 結束而 drop，底層連線
/// 隨之關閉），不然才 `spawn`。因為兩邊用的是同一把鎖，`spawn` 若真的發生，一定是在
/// `drop` 讀到 `closed` 之前完成（互斥鎖排他性），所以一定會被隨後的 `abort_all()` 捕捉到；
/// 反過來，若 `drop` 已經跑完（`closed` 已經是 `true`、鎖已釋放），accept loop 之後才搶到
/// 鎖的任何一次 `spawn` 嘗試都會看到 `closed == true` 而放棄註冊。沒有介於兩者之間、
/// 誰都沒鎖住的窗口。
struct HandlerRegistry {
    tasks: JoinSet<()>,
    closed: bool,
}

type HandlerTasks = Arc<Mutex<HandlerRegistry>>;

/// 持鎖檢查 `closed`；未關閉則把 `task` 加進集合並回傳 `true`，已關閉則什麼都不做並回傳
/// `false`（呼叫端據此丟棄剛接受、來不及註冊的連線）。鎖中毒（另一執行緒 panic 於持鎖期間）
/// 視同已關閉，同樣回傳 `false`——保守起見，寧可少接受一條連線，也不要在無法確定狀態時
/// 繼續服務。
fn try_register_handler(
    handlers: &HandlerTasks,
    task: impl std::future::Future<Output = ()> + Send + 'static,
) -> bool {
    match handlers.lock() {
        Ok(mut registry) => {
            if registry.closed {
                false
            } else {
                registry.tasks.spawn(task);
                true
            }
        }
        Err(_) => false,
    }
}

/// 程序內的假 HERDR server：監聽一個真實的 named pipe（Windows）或 unix socket（unix），
/// 依 `FakeHerdrConfig` 決定每條連線的行為（design D7）。
pub struct FakeHerdr {
    endpoint_path: PathBuf,
    listener_task: JoinHandle<()>,
    shared: Arc<SharedState>,
    handlers: HandlerTasks,
}

impl FakeHerdr {
    /// 啟動假 HERDR：建立監聽端點並開始接受連線。Windows 上第一個 named pipe instance
    /// 在回傳前就同步建立好，避免呼叫端立刻 `connect()` 時撞上「instance 還沒建立」的競態
    /// （對照 `tests/transport.rs` 的 `spawn_named_pipe_multi_echo_server` 註解）。
    pub async fn start(config: FakeHerdrConfig) -> std::io::Result<Self> {
        let config = Arc::new(config);
        let shared = Arc::new(SharedState::default());
        let handlers: HandlerTasks = Arc::new(Mutex::new(HandlerRegistry {
            tasks: JoinSet::new(),
            closed: false,
        }));
        let (endpoint_path, listener_task) =
            spawn_listener(config, shared.clone(), handlers.clone()).await?;
        Ok(Self {
            endpoint_path,
            listener_task,
            shared,
            handlers,
        })
    }

    /// 指向這個假 HERDR 端點的 connector（Windows 為 `NamedPipeConnector`、unix 為
    /// `UnixSocketConnector`）。
    #[must_use]
    pub fn connector(&self) -> Box<dyn Connector> {
        #[cfg(windows)]
        {
            Box::new(NamedPipeConnector::new(self.endpoint_path.clone()))
        }
        #[cfg(unix)]
        {
            Box::new(UnixSocketConnector::new(self.endpoint_path.clone()))
        }
    }

    /// 端點路徑（Windows 不含 `\\.\pipe\` 前綴；fix round 1 finding 3：兩個平台都放在系統
    /// 暫存目錄下，Windows 因此是含磁碟機代號與反斜線的絕對路徑，對照 design D7「名稱用
    /// 暫存路徑加隨機後綴」）。
    #[must_use]
    pub fn endpoint_path(&self) -> &Path {
        &self.endpoint_path
    }

    /// 每條連線收到的行，依連線建立順序排列；每個內層 `Vec` 目前只會有一行（假 HERDR 只把
    /// 第一行——決定 method 的那一行——記錄下來，之後讀到的內容不記錄）。
    #[must_use]
    pub fn received(&self) -> Vec<Vec<String>> {
        self.shared.received()
    }

    /// 每條連線在假 HERDR 這端是否已經關閉，索引與 [`FakeHerdr::received`] 對齊。
    ///
    /// `true` 代表這條連線的 handler 已經結束（回完 request 就收工、腳本跑到
    /// `Step::Close`／`Step::Abort`、對端把連線關掉讓 `Step::Hold` 讀到 EOF，或整個
    /// `FakeHerdr` 被 drop 時被 `abort_all()` 取消），底層連線隨之釋放。供呼叫端驗證
    /// 「失敗時已開的連線要關閉」「釋放事件流時連線要關閉」這類行為；對端關閉是非同步觀察到
    /// 的，呼叫端通常要輪詢加逾時，不能假設 drop 之後立刻為 `true`。
    #[must_use]
    pub fn closed_connections(&self) -> Vec<bool> {
        self.shared.closed()
    }
}

impl Drop for FakeHerdr {
    fn drop(&mut self) {
        // 中止接受新連線的迴圈，讓端點停止再接受新連線（新連線會因為找不到 server 而失敗，
        // 對映成 `ConnectError::ServerNotRunning`）。`abort()` 是延後生效的（tokio 在該 task
        // 下一次 yield 才真的取消它），所以光靠這一行不能保證 accept loop 不會再跑完一次
        // 迭代；真正防止漏網 handler 的是下面「同一把鎖內設 closed、再 abort_all」。
        self.listener_task.abort();
        // fix round 2 finding 2：`closed = true` 與 `abort_all()` 必須在同一次上鎖裡依序
        // 完成，才能堵住 fix round 1 版本的競態（見 `HandlerRegistry` 文件註解）——
        // accept loop 的 `try_register_handler` 用的是同一把鎖，`spawn` 與這裡的
        // 「設 closed、abort_all」互斥，不會有兩邊都沒鎖住的窗口。
        if let Ok(mut registry) = self.handlers.lock() {
            registry.closed = true;
            registry.tasks.abort_all();
        }
        #[cfg(unix)]
        {
            let _ = std::fs::remove_file(&self.endpoint_path);
        }
    }
}

/// fix round 1 finding 3：兩個平台都用系統暫存目錄（`std::env::temp_dir()`）+ 隨機後綴
/// （pid、遞增計數器、奈秒時間戳三者合併，比對照 `tests/transport.rs::unique_suffix` 更嚴格），
/// 避免撞到真的 HERDR、也避免同一支測試行程內的多個假 HERDR 互撞。Windows 上因此得到
/// 含磁碟機代號與反斜線的絕對路徑（例如 `C:\Users\<user>\AppData\Local\Temp\
/// herdr-client-fake\<pid>-<n>-<nanos>.sock`），對映出的 pipe 名稱與真機 HERDR 的形狀一致
/// （design D7）。
fn unique_endpoint_path() -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock 應晚於 UNIX_EPOCH")
        .as_nanos();
    let suffix = format!("{}-{n}-{nanos}", std::process::id());
    std::env::temp_dir()
        .join("herdr-client-fake")
        .join(format!("{suffix}.sock"))
}

/// 建立監聽端點並 spawn 接受迴圈；回傳端點路徑與監聽 task 的 handle。
#[cfg(windows)]
async fn spawn_listener(
    config: Arc<FakeHerdrConfig>,
    shared: Arc<SharedState>,
    handlers: HandlerTasks,
) -> std::io::Result<(PathBuf, JoinHandle<()>)> {
    let endpoint_path = unique_endpoint_path();
    let pipe_name = format!(r"\\.\pipe\{}", endpoint_path.display());
    let first = ServerOptions::new()
        .first_pipe_instance(true)
        .create(&pipe_name)?;
    let listener_task = tokio::spawn(run_named_pipe_listener(
        pipe_name, first, config, shared, handlers,
    ));
    Ok((endpoint_path, listener_task))
}

/// 建立監聽端點並 spawn 接受迴圈；回傳端點路徑與監聽 task 的 handle。
#[cfg(unix)]
async fn spawn_listener(
    config: Arc<FakeHerdrConfig>,
    shared: Arc<SharedState>,
    handlers: HandlerTasks,
) -> std::io::Result<(PathBuf, JoinHandle<()>)> {
    let endpoint_path = unique_endpoint_path();
    if let Some(parent) = endpoint_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let listener = UnixListener::bind(&endpoint_path)?;
    let listener_task = tokio::spawn(run_unix_listener(listener, config, shared, handlers));
    Ok((endpoint_path, listener_task))
}

#[cfg(windows)]
async fn run_named_pipe_listener(
    pipe_name: String,
    mut pending: NamedPipeServer,
    config: Arc<FakeHerdrConfig>,
    shared: Arc<SharedState>,
    handlers: HandlerTasks,
) {
    loop {
        if pending.connect().await.is_err() {
            return;
        }
        let index = shared.new_connection();
        let io = BufReader::new(pending);
        let task = handle_connection(io, config.clone(), shared.clone(), index);
        // fix round 2 finding 2：註冊失敗（已關閉）代表 `FakeHerdr` 正在或已經被 drop，
        // 直接結束整個 accept 迴圈，不必再嘗試建立下一個 pipe instance。
        if !try_register_handler(&handlers, task) {
            return;
        }
        pending = match ServerOptions::new().create(&pipe_name) {
            Ok(server) => server,
            Err(_) => return,
        };
    }
}

#[cfg(unix)]
async fn run_unix_listener(
    listener: UnixListener,
    config: Arc<FakeHerdrConfig>,
    shared: Arc<SharedState>,
    handlers: HandlerTasks,
) {
    loop {
        let Ok((stream, _addr)) = listener.accept().await else {
            return;
        };
        let index = shared.new_connection();
        let io = BufReader::new(stream);
        let task = handle_connection(io, config.clone(), shared.clone(), index);
        // fix round 2 finding 2：註冊失敗（已關閉）代表 `FakeHerdr` 正在或已經被 drop。
        if !try_register_handler(&handlers, task) {
            return;
        }
    }
}
