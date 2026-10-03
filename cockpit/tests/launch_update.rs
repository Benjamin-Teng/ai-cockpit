//! auto-update tasks 3.1–3.3 驗收測試：`cockpit-launch` 的更新檢查、詢問、下載驗證與交棒
//! （spec `auto-update` 全部、`desktop-launch`「啟動器」第 4 步；design D1、D2、D5–D8）。
//!
//! 每個測試自建一個沙箱（`CARGO_TARGET_TMPDIR` 下）：
//!
//! - `app\`：複製進來的 `cockpit-launch.exe` 與 `cockpit.exe`；安裝版情境再放一個空的 `unins000.exe`。
//! - `data\`：工作目錄，放指定測試專用埠的 `cockpit.toml`（不碰使用者的 7770）；`cockpit.log` 與
//!   更新檢查紀錄 `cockpit.update.json` 都在這裡。
//! - `tmp\`：啟動器的 `TMP`／`TEMP`，所以 `%TEMP%\ai-cockpit-update` 是沙箱自己的，不會清到使用者的。
//! - `record\`：假瀏覽器與假安裝檔（同一支 example `fake_update_installer`）寫下收到的引數。
//!
//! 一律設假的 `COCKPIT_BROWSER`（不會在使用者桌面開 Chrome）與 `COCKPIT_LAUNCH_DIALOG_FILE`
//! （訊息框改寫檔，不會跳出阻塞的視窗）。本機 axum 假伺服器提供 `releases/latest` 與下載路徑，
//! 記錄收到的每個請求路徑，用來斷言「沒有發出請求」。
//!
//! 清理：沙箱 drop 時只結束「執行檔路徑恰為本沙箱 `app\cockpit.exe`」的程序（測試自己起的後端），
//! 不碰任何其他程序。
//!
//! 假安裝檔要先建好 examples：`cargo test` 預設會建；只跑本檔時用
//! `cargo build -p cockpit --example fake_update_installer` 再 `cargo test -p cockpit --test launch_update`。

#![cfg(windows)]

use std::collections::HashMap;
use std::net::{SocketAddr, TcpListener};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};

use axum::Router;
use axum::extract::State;
use axum::http::{StatusCode, Uri};
use cockpit::launch::{self, LaunchLang, ProbeOutcome};
use cockpit::update;
use sha2::Digest;

/// 假 release 的版本：一定比執行中的 crate 版本新。
const NEW_VERSION: &str = "9.9.9";
const LATEST_PATH: &str = "/repos/Benjamin-Teng/ai-cockpit/releases/latest";
const SUMS_PATH: &str = "/download/v9.9.9/SHA256SUMS.txt";
const INSTALLER_NAME: &str = "ai-cockpit-9.9.9-x64-setup.exe";
const INSTALLER_PATH: &str = "/download/v9.9.9/ai-cockpit-9.9.9-x64-setup.exe";
/// 假安裝檔的主檔名（紀錄檔名用）。
const INSTALLER_STEM: &str = "ai-cockpit-9.9.9-x64-setup";
const BROWSER_STEM: &str = "fake-browser";

// ---------------------------------------------------------------------------
// 假伺服器
// ---------------------------------------------------------------------------

type Routes = HashMap<&'static str, (u16, Vec<u8>)>;

#[derive(Clone)]
struct ServerState {
    routes: Arc<Routes>,
    requests: Arc<Mutex<Vec<String>>>,
}

/// 本機 axum 假伺服器：依路徑回固定狀態碼與本體，沒列出的路徑回 404；記錄每個請求的路徑。
struct FakeServer {
    base: String,
    requests: Arc<Mutex<Vec<String>>>,
    _runtime: tokio::runtime::Runtime,
}

impl FakeServer {
    fn start(routes: Routes) -> FakeServer {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .expect("建立 tokio runtime");
        let requests = Arc::new(Mutex::new(Vec::new()));
        let state = ServerState {
            routes: Arc::new(routes),
            requests: Arc::clone(&requests),
        };
        let listener = runtime
            .block_on(tokio::net::TcpListener::bind("127.0.0.1:0"))
            .expect("假伺服器綁定埠");
        let addr = listener.local_addr().expect("假伺服器位址");
        let app = Router::new().fallback(serve).with_state(state);
        runtime.spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        FakeServer {
            base: format!("http://{addr}"),
            requests,
            _runtime: runtime,
        }
    }

    fn api_url(&self) -> String {
        format!("{}{LATEST_PATH}", self.base)
    }

    fn download_base(&self) -> String {
        format!("{}/download", self.base)
    }

    fn requests(&self) -> Vec<String> {
        self.requests.lock().unwrap().clone()
    }
}

async fn serve(State(state): State<ServerState>, uri: Uri) -> (StatusCode, Vec<u8>) {
    let path = uri.path().to_string();
    state.requests.lock().unwrap().push(path.clone());
    match state.routes.get(path.as_str()) {
        Some((status, body)) => (StatusCode::from_u16(*status).unwrap(), body.clone()),
        None => (StatusCode::NOT_FOUND, b"not found".to_vec()),
    }
}

fn release_json() -> Vec<u8> {
    format!(
        r#"{{"tag_name":"v{NEW_VERSION}","draft":false,"prerelease":false,"assets":[
            {{"name":"{INSTALLER_NAME}","browser_download_url":"https://example.invalid/x"}},
            {{"name":"SHA256SUMS.txt","browser_download_url":"https://example.invalid/y"}}]}}"#
    )
    .into_bytes()
}

fn sha256_hex(bytes: &[u8]) -> String {
    update::hex_lower(&sha2::Sha256::digest(bytes))
}

/// release.yml 格式的雜湊檔：依檔名排序、兩個空白、LF、結尾換行。
fn sums_for(installer_hash: &str) -> Vec<u8> {
    let zip_hash = "0".repeat(64);
    format!("{installer_hash}  {INSTALLER_NAME}\n{zip_hash}  ai-cockpit-{NEW_VERSION}-x64.zip\n")
        .into_bytes()
}

/// 有新版、雜湊檔與安裝檔都正確的路由；`installer` 是假伺服器提供的安裝檔內容。
fn new_release_routes(installer: Vec<u8>) -> Routes {
    let hash = sha256_hex(&installer);
    HashMap::from([
        (LATEST_PATH, (200, release_json())),
        (SUMS_PATH, (200, sums_for(&hash))),
        (INSTALLER_PATH, (200, installer)),
    ])
}

// ---------------------------------------------------------------------------
// 沙箱
// ---------------------------------------------------------------------------

/// 啟動器、假瀏覽器、假安裝檔在 `target/<profile>/` 下的位置。
fn launcher_exe() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_cockpit-launch"))
}

fn backend_exe() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_cockpit"))
}

fn fake_exe() -> PathBuf {
    let path = launcher_exe()
        .parent()
        .expect("啟動器所在目錄")
        .join("examples")
        .join("fake_update_installer.exe");
    assert!(
        path.is_file(),
        "找不到測試用假安裝檔 {}；請先建置 examples（`cargo test` 預設會建；只跑本檔時用 \
         `cargo build -p cockpit --example fake_update_installer` 再 `cargo test -p cockpit --test launch_update`）",
        path.display()
    );
    path
}

fn fake_installer_bytes() -> Vec<u8> {
    std::fs::read(fake_exe()).expect("讀取假安裝檔")
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Layout {
    /// 免安裝 zip 解開的目錄：沒有 `unins000.exe`。
    Zip,
    /// 安裝檔安裝的目錄：有 `unins000.exe`。
    Installed,
}

struct Sandbox {
    root: PathBuf,
    app: PathBuf,
    data: PathBuf,
    tmp: PathBuf,
    record: PathBuf,
    listen: SocketAddr,
}

impl Sandbox {
    fn new(name: &str, layout: Layout) -> Sandbox {
        let root = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("launch-update-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let app = root.join("app");
        let data = root.join("data");
        let tmp = root.join("tmp");
        let record = root.join("record");
        for dir in [&app, &data, &tmp, &record] {
            std::fs::create_dir_all(dir).expect("建立沙箱目錄");
        }
        std::fs::copy(launcher_exe(), app.join("cockpit-launch.exe")).expect("複製啟動器");
        std::fs::copy(backend_exe(), app.join("cockpit.exe")).expect("複製後端");
        if layout == Layout::Installed {
            std::fs::write(app.join(update::UNINSTALLER_FILE_NAME), b"").expect("放假的 unins000");
        }
        std::fs::copy(fake_exe(), root.join(format!("{BROWSER_STEM}.exe"))).expect("複製假瀏覽器");

        let listen = free_port();
        // runtime 指向不存在的 socket：後端照常提供 API，只是連不上 HERDR（不碰使用者的 HERDR）。
        let no_socket = root.join("no-such-herdr.sock");
        std::fs::write(
            data.join("cockpit.toml"),
            format!(
                "[server]\nlisten = \"{listen}\"\n\n[[runtime]]\nid = \"test\"\nkind = \"herdr\"\n\
                 socket = '{}'\n",
                no_socket.display()
            ),
        )
        .expect("寫設定檔");
        Sandbox {
            root,
            app,
            data,
            tmp,
            record,
            listen,
        }
    }

    fn state_file(&self) -> PathBuf {
        self.data.join(update::STATE_FILE_NAME)
    }

    fn dialog_file(&self) -> PathBuf {
        self.root.join("dialog.txt")
    }

    fn update_dir(&self) -> PathBuf {
        self.tmp.join(update::TEMP_DIR_NAME)
    }

    /// 程序 `pid` 的暫存子資料夾 `%TEMP%i-cockpit-update<pid>`（design D6）。
    fn work_dir(&self, pid: u32) -> PathBuf {
        update::work_dir(&self.tmp, pid)
    }

    fn dialog_text(&self) -> String {
        std::fs::read_to_string(self.dialog_file()).unwrap_or_default()
    }

    /// 啟動器命令：工作目錄 `data\`、`--config cockpit.toml`、假瀏覽器、對話框寫檔、英文訊息、
    /// 沙箱自己的 `TMP`／`TEMP`；清掉會改變行為的使用者環境變數（代理、更新開關、答案）。
    fn launcher(&self, server: &FakeServer) -> Command {
        let mut command = Command::new(self.app.join("cockpit-launch.exe"));
        command
            .args(["--config", "cockpit.toml"])
            .current_dir(&self.data)
            .env(
                "COCKPIT_BROWSER",
                self.root.join(format!("{BROWSER_STEM}.exe")),
            )
            .env("COCKPIT_LAUNCH_DIALOG_FILE", self.dialog_file())
            .env("COCKPIT_LAUNCH_LANG", "en")
            .env("TMP", &self.tmp)
            .env("TEMP", &self.tmp)
            .env(update::ENV_API_URL, server.api_url())
            .env(update::ENV_DOWNLOAD_BASE, server.download_base())
            .env("COCKPIT_TEST_FAKE_RECORD_DIR", &self.record)
            .env_remove("COCKPIT_TEST_FAKE_SLEEP_MS")
            .env_remove(update::ENV_NO_UPDATE_CHECK)
            .env_remove(update::ENV_UPDATE_ANSWER);
        for proxy in [
            "ALL_PROXY",
            "all_proxy",
            "HTTPS_PROXY",
            "https_proxy",
            "HTTP_PROXY",
            "http_proxy",
            "NO_PROXY",
            "no_proxy",
        ] {
            command.env_remove(proxy);
        }
        command
    }

    fn record_file(&self, stem: &str, ext: &str) -> PathBuf {
        self.record.join(format!("{stem}.{ext}"))
    }

    /// 假瀏覽器收到的引數；`timeout` 內沒有出現則為 `None`（沒被呼叫）。
    fn wait_browser_args(&self, timeout: Duration) -> Option<Vec<String>> {
        let path = self.record_file(BROWSER_STEM, "args");
        wait_for(timeout, || path.is_file()).then(|| read_lines(&path))
    }

    fn probe(&self) -> ProbeOutcome {
        launch::probe(self.listen, LaunchLang::En)
    }

    /// 斷言「照常啟動」：啟動器結束碼 0、後端在測試專用埠就緒、假瀏覽器以 `--app=<網址>` 被呼叫。
    fn assert_started_normally(&self, output: &Output) {
        assert!(
            output.status.success(),
            "啟動器應以 0 結束：{:?}；對話框：{}",
            output.status,
            self.dialog_text()
        );
        assert_eq!(self.probe(), ProbeOutcome::Cockpit, "測試專用埠上應有後端");
        let args = self
            .wait_browser_args(Duration::from_secs(10))
            .expect("假瀏覽器應被呼叫");
        assert_eq!(args, [format!("--app=http://{}/", self.listen)]);
    }

    /// 結束本沙箱 `app\cockpit.exe` 起的後端：只比對完整執行檔路徑，不碰其他程序。
    fn stop_own_backend(&self) {
        if matches!(self.probe(), ProbeOutcome::Unreachable(_)) {
            return;
        }
        let exe = self.app.join("cockpit.exe");
        let script = format!(
            "Get-Process -Name cockpit -ErrorAction SilentlyContinue | \
             Where-Object {{ $_.Path -eq '{}' }} | Stop-Process -Force",
            exe.display()
        );
        let _ = Command::new("powershell.exe")
            .args(["-NoProfile", "-NonInteractive", "-Command", &script])
            .output();
        let stopped = wait_for(Duration::from_secs(10), || {
            matches!(self.probe(), ProbeOutcome::Unreachable(_))
        });
        if !stopped {
            let message = format!("測試自己起的後端（{}）沒有結束", exe.display());
            // 從 Drop 呼叫：測試已在 panic 時再 panic 會讓整個測試程序中止、蓋掉原本的失敗原因，
            // 所以這時只印出來。
            if std::thread::panicking() {
                eprintln!("{message}");
            } else {
                panic!("{message}");
            }
        }
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        self.stop_own_backend();
        // 後端剛被結束時，埠已關但執行檔映像可能還沒釋放，刪除會暫時失敗，所以重試幾秒；
        // 仍刪不掉就留給下次同名沙箱開頭清掉。
        wait_for(Duration::from_secs(10), || {
            std::fs::remove_dir_all(&self.root).is_ok() || !self.root.exists()
        });
    }
}

/// 向系統要一個空閒埠：綁 0 埠、記下、放掉（測試專用，不撞使用者的 7770）。
fn free_port() -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").expect("取得空閒埠");
    listener.local_addr().expect("空閒埠位址")
}

fn wait_for(timeout: Duration, mut done: impl FnMut() -> bool) -> bool {
    let deadline = Instant::now() + timeout;
    loop {
        if done() {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn read_lines(path: &Path) -> Vec<String> {
    std::fs::read_to_string(path)
        .expect("讀取紀錄檔")
        .split('\n')
        .map(str::to_string)
        .collect()
}

/// 執行啟動器並擷取輸出（同 `Command::output`：stdin 空、stdout／stderr 擷取），一併回傳其 pid，
/// 用來找它的暫存子資料夾。
fn run_launcher(mut command: Command) -> (Output, u32) {
    let child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("執行啟動器");
    let pid = child.id();
    (child.wait_with_output().expect("等待啟動器"), pid)
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

fn read_record(sandbox: &Sandbox) -> update::CheckRecord {
    let text = std::fs::read_to_string(sandbox.state_file()).expect("應有更新檢查紀錄");
    update::parse_record(&text).expect("更新檢查紀錄應可解析")
}

/// 查詢花了多久：紀錄檔建立（發出查詢前寫 `pending`，沙箱開始時沒有紀錄檔）到最後一次修改
/// （結果寫回）。不從啟動器開始計時，因為那會把第 3 步偵測監聽埠的時間也算進去（Windows 連到
/// 沒在聽的埠要約 2 秒才回報拒絕）。
fn query_duration(sandbox: &Sandbox) -> Duration {
    let meta = std::fs::metadata(sandbox.state_file()).expect("紀錄檔中繼資料");
    let created = meta.created().expect("紀錄檔建立時間");
    let modified = meta.modified().expect("紀錄檔修改時間");
    modified.duration_since(created).unwrap_or_default()
}

// ---------------------------------------------------------------------------
// 3.1 何時檢查、判定有無新版
// ---------------------------------------------------------------------------

/// Scenario「免安裝 zip 不檢查」：沒有 unins000.exe → 不發請求、不建紀錄，照常啟動。
#[test]
fn zip_layout_makes_no_request_and_writes_no_record() {
    let server = FakeServer::start(new_release_routes(b"unused".to_vec()));
    let sandbox = Sandbox::new("zip", Layout::Zip);
    let output = sandbox.launcher(&server).output().expect("執行啟動器");
    sandbox.assert_started_normally(&output);
    assert_eq!(
        server.requests(),
        Vec::<String>::new(),
        "zip 目錄不應發出請求"
    );
    assert!(
        !sandbox.state_file().exists(),
        "zip 目錄不應建立更新檢查紀錄"
    );
    assert_eq!(sandbox.dialog_text(), "", "不應有任何訊息");
}

/// Scenario「使用者關閉檢查」：`COCKPIT_NO_UPDATE_CHECK=1` → 不發請求。
#[test]
fn disabled_by_env_makes_no_request() {
    let server = FakeServer::start(new_release_routes(b"unused".to_vec()));
    let sandbox = Sandbox::new("disabled", Layout::Installed);
    let output = sandbox
        .launcher(&server)
        .env(update::ENV_NO_UPDATE_CHECK, "1")
        .output()
        .expect("執行啟動器");
    sandbox.assert_started_normally(&output);
    assert_eq!(
        server.requests(),
        Vec::<String>::new(),
        "關閉檢查時不應發出請求"
    );
    assert!(!sandbox.state_file().exists());
    assert_eq!(sandbox.dialog_text(), "");
}

/// Scenario「24 小時內不重複檢查」：紀錄是 3 小時前 → 不發請求、紀錄不變。
#[test]
fn checked_within_24_hours_makes_no_request() {
    let server = FakeServer::start(new_release_routes(b"unused".to_vec()));
    let sandbox = Sandbox::new("throttled", Layout::Installed);
    let before = format!(
        r#"{{"checked_at":{},"result":"HTTP 404 (no stable release yet)"}}"#,
        unix_now() - 3 * 60 * 60
    );
    std::fs::write(sandbox.state_file(), &before).unwrap();
    let output = sandbox.launcher(&server).output().expect("執行啟動器");
    sandbox.assert_started_normally(&output);
    assert_eq!(
        server.requests(),
        Vec::<String>::new(),
        "24 小時內不應發出請求"
    );
    assert_eq!(
        std::fs::read_to_string(sandbox.state_file()).unwrap(),
        before
    );
}

/// Scenario「尚無正式版」：API 回 404 → 不顯示訊息、照常啟動；紀錄寫下時間與原因。
/// 接著 Scenario「後端已在執行」：刪掉紀錄再執行一次，後端已在 → 不發請求、只開視窗。
#[test]
fn http_404_starts_normally_and_records_reason_then_running_backend_skips_check() {
    let server = FakeServer::start(HashMap::from([(LATEST_PATH, (404, b"{}".to_vec()))]));
    let sandbox = Sandbox::new("http404", Layout::Installed);
    let start = unix_now();
    let output = sandbox.launcher(&server).output().expect("執行啟動器");
    sandbox.assert_started_normally(&output);
    assert_eq!(server.requests(), [LATEST_PATH]);
    let record = read_record(&sandbox);
    assert_eq!(record.result, "HTTP 404 (no stable release yet)");
    assert!(
        (start..=unix_now()).contains(&record.checked_at),
        "checked_at 應為這次執行的時間：{}",
        record.checked_at
    );
    assert_eq!(sandbox.dialog_text(), "", "404 不應顯示任何訊息");

    // 後端已在執行：即使紀錄不存在（已到期）也不檢查。
    std::fs::remove_file(sandbox.state_file()).unwrap();
    std::fs::remove_file(sandbox.record_file(BROWSER_STEM, "args")).unwrap();
    let output = sandbox.launcher(&server).output().expect("再次執行啟動器");
    sandbox.assert_started_normally(&output);
    assert_eq!(
        server.requests(),
        [LATEST_PATH],
        "後端已在執行時不應再發請求"
    );
    assert!(!sandbox.state_file().exists());
}

/// Scenario「離線」（連到未監聽的埠）：連線失敗 → 約 3 秒內放棄、不顯示訊息、照常啟動。
///
/// Windows 對沒在聽的埠約 2 秒才回報拒絕（重送 SYN），機器忙時可能先碰到 3 秒的整體逾時，所以結果接受
/// `connection failed` 或 `timed out` 兩種，時間門檻放寬到 3.5 秒（實測約 2.0 秒）。
#[test]
fn offline_unlistened_port_gives_up_and_starts_normally() {
    let server = FakeServer::start(HashMap::new());
    let sandbox = Sandbox::new("offline", Layout::Installed);
    let dead = free_port();
    let output = sandbox
        .launcher(&server)
        .env(update::ENV_API_URL, format!("http://{dead}{LATEST_PATH}"))
        .output()
        .expect("執行啟動器");
    sandbox.assert_started_normally(&output);
    let record = read_record(&sandbox);
    assert!(
        record.result.starts_with("connection failed") || record.result == "timed out",
        "{}",
        record.result
    );
    let took = query_duration(&sandbox);
    assert!(
        took < Duration::from_millis(3500),
        "檢查應在約 3 秒內結束：{took:?}"
    );
    assert_eq!(sandbox.dialog_text(), "");
}

/// Scenario「離線」的逾時面：伺服器接受連線但永不回應 → 約 3 秒逾時、照常啟動。
#[test]
fn unresponsive_server_times_out_after_3_seconds_and_starts_normally() {
    let server = FakeServer::start(HashMap::new());
    let sandbox = Sandbox::new("blackhole", Layout::Installed);
    let blackhole = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = blackhole.local_addr().unwrap();
    // 接受連線後握著不回應，直到測試結束。
    std::thread::spawn(move || {
        let mut held = Vec::new();
        for stream in blackhole.incoming() {
            held.push(stream);
        }
    });
    let output = sandbox
        .launcher(&server)
        .env(update::ENV_API_URL, format!("http://{addr}{LATEST_PATH}"))
        .output()
        .expect("執行啟動器");
    sandbox.assert_started_normally(&output);
    let record = read_record(&sandbox);
    assert_eq!(record.result, "timed out");
    let took = query_duration(&sandbox);
    // 下限確認沒有提早放棄（紀錄檔在發出查詢前建立，實測約 3.01 秒，機器忙只會更長）；上限只確認
    // 沒有拖太久，留 2 秒餘裕給忙碌的 CI。
    assert!(
        took >= Duration::from_millis(2900) && took < Duration::from_secs(5),
        "應在約 3 秒時逾時：{took:?}"
    );
    assert_eq!(sandbox.dialog_text(), "");
}

// ---------------------------------------------------------------------------
// 3.2 詢問與下載驗證
// ---------------------------------------------------------------------------

/// Scenario「選擇稍後」：答案預設「否」→ 詢問內容寫入對話框檔、沒有下載、照常啟動。
#[test]
fn answer_no_downloads_nothing_and_starts_normally() {
    let server = FakeServer::start(new_release_routes(fake_installer_bytes()));
    let sandbox = Sandbox::new("answer-no", Layout::Installed);
    let output = sandbox.launcher(&server).output().expect("執行啟動器");
    sandbox.assert_started_normally(&output);
    assert_eq!(server.requests(), [LATEST_PATH], "選「否」不應下載");
    let dialog = sandbox.dialog_text();
    assert!(
        dialog.contains(&format!(
            "A new version of AI Agent Cockpit is available: {NEW_VERSION} (current version: {}",
            env!("CARGO_PKG_VERSION")
        )),
        "{dialog}"
    );
    assert!(
        !sandbox.update_dir().exists(),
        "選「否」不應建立暫存子資料夾"
    );
    assert_eq!(
        read_record(&sandbox).result,
        format!("update available: {NEW_VERSION}")
    );
}

/// Scenario「雜湊不符」：安裝檔被刪、沒有執行、錯誤訊息寫入對話框檔、照常啟動。
#[test]
fn hash_mismatch_deletes_installer_and_starts_normally() {
    let mut routes = new_release_routes(fake_installer_bytes());
    routes.insert(SUMS_PATH, (200, sums_for(&"ab".repeat(32))));
    let server = FakeServer::start(routes);
    let sandbox = Sandbox::new("hash-mismatch", Layout::Installed);
    let mut command = sandbox.launcher(&server);
    command.env(update::ENV_UPDATE_ANSWER, "yes");
    let (output, pid) = run_launcher(command);
    sandbox.assert_started_normally(&output);
    assert_eq!(server.requests(), [LATEST_PATH, SUMS_PATH, INSTALLER_PATH]);
    // 下載在這個程序自己的子資料夾（design D6）；資料夾在、安裝檔不在，才證明是被刪掉而不是寫到別處。
    assert!(
        sandbox.work_dir(pid).is_dir(),
        "應在 {} 下載",
        sandbox.work_dir(pid).display()
    );
    assert!(
        !sandbox.work_dir(pid).join(INSTALLER_NAME).exists(),
        "雜湊不符的安裝檔應被刪除"
    );
    assert!(
        !sandbox.record_file(INSTALLER_STEM, "args").exists(),
        "雜湊不符的安裝檔不應被執行"
    );
    let dialog = sandbox.dialog_text();
    assert!(
        dialog.contains("does not match the checksum in SHA256SUMS.txt"),
        "{dialog}"
    );
    assert!(
        dialog.contains("keep using the current version"),
        "{dialog}"
    );
}

/// Scenario「下載中斷」的一種：安裝檔回 404 → 錯誤訊息、照常啟動。
#[test]
fn installer_download_404_shows_error_and_starts_normally() {
    let mut routes = new_release_routes(fake_installer_bytes());
    routes.remove(INSTALLER_PATH);
    let server = FakeServer::start(routes);
    let sandbox = Sandbox::new("download-404", Layout::Installed);
    let mut command = sandbox.launcher(&server);
    command.env(update::ENV_UPDATE_ANSWER, "yes");
    let (output, pid) = run_launcher(command);
    sandbox.assert_started_normally(&output);
    assert_eq!(server.requests(), [LATEST_PATH, SUMS_PATH, INSTALLER_PATH]);
    let dialog = sandbox.dialog_text();
    assert!(
        dialog.contains("the download failed (HTTP 404)"),
        "{dialog}"
    );
    assert!(sandbox.work_dir(pid).is_dir());
    assert!(!sandbox.work_dir(pid).join(INSTALLER_NAME).exists());
}

/// design D6：每個程序用自己的 `%TEMP%\ai-cockpit-update\<pid>\`，開始時清掉其他舊子資料夾，
/// 刪不掉的（檔案被占用：另一個啟動器正在下載、或安裝檔正在執行）略過。即使 24 小時內不檢查
/// （更新後由安裝檔重新啟動的那次就是），也照樣清理，所以上次更新留下的安裝檔下次啟動就清掉。
#[test]
fn stale_work_dirs_are_removed_and_in_use_ones_kept_even_when_throttled() {
    use std::os::windows::fs::OpenOptionsExt;
    /// Win32 `FILE_SHARE_READ`（learn.microsoft.com CreateFileW 的 dwShareMode）：不含
    /// `FILE_SHARE_DELETE`，別的程序刪不掉這個檔——模擬下載中的安裝檔。
    const FILE_SHARE_READ: u32 = 0x0000_0001;

    let server = FakeServer::start(new_release_routes(b"unused".to_vec()));
    let sandbox = Sandbox::new("stale-dirs", Layout::Installed);
    std::fs::write(
        sandbox.state_file(),
        format!(r#"{{"checked_at":{},"result":"x"}}"#, unix_now() - 60),
    )
    .unwrap();
    let stale = sandbox.update_dir().join("111");
    std::fs::create_dir_all(stale.join("nested")).unwrap();
    std::fs::write(stale.join(INSTALLER_NAME), b"old installer").unwrap();
    std::fs::write(stale.join("nested").join("setup.log"), b"old log").unwrap();
    let busy = sandbox.update_dir().join("222");
    std::fs::create_dir_all(&busy).unwrap();
    let held_path = busy.join(INSTALLER_NAME);
    let held = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .share_mode(FILE_SHARE_READ)
        .open(&held_path)
        .unwrap();
    let loose_file = sandbox.update_dir().join("note.txt");
    std::fs::write(&loose_file, b"not a work dir").unwrap();

    let output = sandbox.launcher(&server).output().expect("執行啟動器");
    sandbox.assert_started_normally(&output);
    assert_eq!(server.requests(), Vec::<String>::new(), "24 小時內不應查詢");
    assert!(!stale.exists(), "舊子資料夾應被清掉");
    assert!(held_path.is_file(), "被占用的檔案不應被刪除");
    assert!(loose_file.is_file(), "只清子資料夾，不動其他檔案");
    assert_eq!(sandbox.dialog_text(), "", "清理失敗不應顯示任何訊息");
    drop(held);
}

/// 雜湊檔沒有安裝檔那一行 → 雜湊檔錯誤訊息、不下載安裝檔、照常啟動。
#[test]
fn sums_without_installer_entry_shows_error_and_skips_installer_download() {
    let mut routes = new_release_routes(fake_installer_bytes());
    routes.insert(
        SUMS_PATH,
        (200, format!("{}  other.exe\n", "0".repeat(64)).into_bytes()),
    );
    let server = FakeServer::start(routes);
    let sandbox = Sandbox::new("sums-missing", Layout::Installed);
    let output = sandbox
        .launcher(&server)
        .env(update::ENV_UPDATE_ANSWER, "yes")
        .output()
        .expect("執行啟動器");
    sandbox.assert_started_normally(&output);
    assert_eq!(server.requests(), [LATEST_PATH, SUMS_PATH]);
    let dialog = sandbox.dialog_text();
    assert!(
        dialog.contains(&format!("SHA256SUMS.txt has no entry for {INSTALLER_NAME}")),
        "{dialog}"
    );
}

// ---------------------------------------------------------------------------
// 3.3 交棒
// ---------------------------------------------------------------------------

/// Scenario「交給安裝檔後不啟動後端」＋「完整更新」的啟動器端：驗證通過 → 以 design D8 參數啟動
/// 安裝檔、結束碼 0、沒有後端、沒有開瀏覽器；呼叫端擷取輸出時不會被還在執行的安裝檔卡住；
/// 下載檔沒有 Zone.Identifier 資料流。
#[test]
fn verified_installer_is_handed_off_and_launcher_exits_zero() {
    // 假安裝檔執行這麼久；啟動器（含下載）實測 1.3–2.5 秒就返回，10 秒留足 CI 的餘裕。
    const INSTALLER_SLEEP: Duration = Duration::from_secs(10);
    let server = FakeServer::start(new_release_routes(fake_installer_bytes()));
    let sandbox = Sandbox::new("handoff", Layout::Installed);

    let started = Instant::now();
    let mut command = sandbox.launcher(&server);
    command.env(update::ENV_UPDATE_ANSWER, "yes").env(
        "COCKPIT_TEST_FAKE_SLEEP_MS",
        INSTALLER_SLEEP.as_millis().to_string(),
    );
    let (output, pid) = run_launcher(command);
    let took = started.elapsed();
    let installer_done = sandbox.record_file(INSTALLER_STEM, "done");
    assert!(
        !installer_done.exists() && took < INSTALLER_SLEEP,
        "擷取輸出的呼叫端應在假安裝檔結束前返回（花了 {took:?}）"
    );
    assert_eq!(output.status.code(), Some(0), "交棒後啟動器應以 0 結束");
    assert_eq!(server.requests(), [LATEST_PATH, SUMS_PATH, INSTALLER_PATH]);

    // 安裝檔收到的引數與 design D8 完全相符。
    let args_file = sandbox.record_file(INSTALLER_STEM, "args");
    assert!(
        wait_for(Duration::from_secs(10), || args_file.is_file()),
        "假安裝檔應被執行"
    );
    // 安裝檔與 setup.log 都在啟動器自己的子資料夾 `<pid>`（design D6、D8）。
    let expected_log = format!("/LOG={}", sandbox.work_dir(pid).join("setup.log").display());
    assert_eq!(
        read_lines(&args_file),
        [
            "/SILENT",
            "/SUPPRESSMSGBOXES",
            "/NORESTART",
            "/NOCANCEL",
            "/COCKPITUPDATE=1",
            expected_log.as_str(),
        ]
    );
    let cmdline = std::fs::read_to_string(sandbox.record_file(INSTALLER_STEM, "cmdline")).unwrap();
    assert!(
        cmdline.ends_with(&format!(
            " /SILENT /SUPPRESSMSGBOXES /NORESTART /NOCANCEL /COCKPITUPDATE=1 {expected_log}"
        )),
        "原始命令列：{cmdline}"
    );

    // 沒有啟動後端、沒有開瀏覽器、沒有錯誤訊息。
    assert!(
        matches!(sandbox.probe(), ProbeOutcome::Unreachable(_)),
        "交棒後監聽埠上不應有後端"
    );
    assert_eq!(
        sandbox.wait_browser_args(Duration::from_secs(1)),
        None,
        "交棒後不應開瀏覽器"
    );
    assert!(
        !sandbox.data.join(launch::LOG_FILE_NAME).exists(),
        "不應啟動後端"
    );
    let dialog = sandbox.dialog_text();
    assert!(dialog.contains("A new version"), "{dialog}");
    assert!(!dialog.contains("Cannot update"), "{dialog}");

    // design D8：下載檔沒有 Zone.Identifier（先確認這個磁碟區支援替代資料流，否定才有意義）。
    let probe_file = sandbox.root.join("ads-probe.txt");
    std::fs::write(&probe_file, b"x").unwrap();
    let ads = PathBuf::from(format!("{}:probe", probe_file.display()));
    std::fs::write(&ads, b"y").expect("此磁碟區應支援替代資料流");
    assert_eq!(std::fs::read(&ads).unwrap(), b"y");
    let installer = sandbox.work_dir(pid).join(INSTALLER_NAME);
    assert!(installer.is_file(), "交棒後下載檔留在暫存子資料夾");
    let zone = PathBuf::from(format!("{}:Zone.Identifier", installer.display()));
    let error = std::fs::File::open(&zone).expect_err("下載檔不應有 Zone.Identifier");
    assert_eq!(error.kind(), std::io::ErrorKind::NotFound, "{error}");

    // 等假安裝檔結束再清理沙箱。
    assert!(wait_for(Duration::from_secs(20), || installer_done.exists()));
}

/// 安裝檔無法啟動（驗證通過但不是可執行檔）→ 錯誤訊息、照常啟動。
#[test]
fn installer_that_cannot_start_shows_error_and_starts_normally() {
    let server = FakeServer::start(new_release_routes(
        b"this is not a Windows executable".to_vec(),
    ));
    let sandbox = Sandbox::new("spawn-fail", Layout::Installed);
    let output = sandbox
        .launcher(&server)
        .env(update::ENV_UPDATE_ANSWER, "yes")
        .output()
        .expect("執行啟動器");
    sandbox.assert_started_normally(&output);
    let dialog = sandbox.dialog_text();
    assert!(dialog.contains("failed to start the installer"), "{dialog}");
    assert!(
        dialog.contains("keep using the current version"),
        "{dialog}"
    );
}
