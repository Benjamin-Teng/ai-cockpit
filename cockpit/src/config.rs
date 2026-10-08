//! Cockpit 設定檔載入與驗證（spec `cockpit-config` 全部；design D16）。
//!
//! 來源順序：`--config <path>` → 工作目錄的 `cockpit.toml` → 零設定模式（等同一筆
//! `id = "local"`、`kind = "herdr"`、端點 [`HerdrEndpoint::Default`] 的 runtime，其餘皆
//! 預設值）。工作目錄與環境變數以參數注入（`lookup_env`、`cwd`），呼叫端（`main`）包一層
//! 真值，測試才不會隨機器狀態飄（同 1a `default_socket_path(lookup_env, config_dir)` 的做法）。
//!
//! `cockpit` 不直接依賴 `herdr-client`：端點型別是 `cockpit-herdr` 公開的
//! [`HerdrEndpoint`]，「自動找本機預設路徑」在這裡只表示為 [`HerdrEndpoint::Default`]，
//! 實際路徑解析留給 2.7 的工廠（呼叫 `herdr-client` 的 `default_socket_path_from_env()`）。

use std::collections::{HashMap, HashSet};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::path::{Path, PathBuf};

use cockpit_core::{
    BindingSpec, ProjectDef, ProjectId, RuntimeId, TaskDef, TaskId, WorkstreamDef, WorkstreamId,
};
use cockpit_herdr::HerdrEndpoint;
use serde::Deserialize;

/// `listen` 未指定時的預設值。
const DEFAULT_LISTEN: &str = "127.0.0.1:7770";
/// `polling.resnapshot_secs` 未指定時的預設值。
const DEFAULT_RESNAPSHOT_SECS: u64 = 30;
/// `polling.wsl_probe_secs` 未指定時的預設值。
const DEFAULT_WSL_PROBE_SECS: u64 = 60;
/// 零設定模式、或設定檔存在但沒有任何 `[[runtime]]` 時，自動補上的那一筆 runtime 的 id。
const ZERO_CONFIG_RUNTIME_ID: &str = "local";
/// 狀態檔預設檔名（有設定檔時在設定檔目錄下，零設定模式在 [`ZERO_CONFIG_STATE_DIR`] 下）。
const STATE_FILE_NAME: &str = "cockpit.state.json";
/// 零設定模式的狀態檔所在資料夾的上層，取自這個環境變數（repo-projects task 4.1，design D5）。
const ZERO_CONFIG_STATE_DIR_ENV: &str = "LOCALAPPDATA";
/// 零設定模式的狀態檔資料夾名稱：`%LOCALAPPDATA%\ai-cockpit`。
const ZERO_CONFIG_STATE_DIR: &str = "ai-cockpit";

/// 解析並驗證過的完整設定。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
    /// HTTP／WS 伺服器設定。
    pub server: ServerConfig,
    /// 輪詢間隔設定。
    pub polling: PollingConfig,
    /// 要接上的 HERDR runtime 清單，順序與設定檔中出現的順序一致。
    pub runtimes: Vec<RuntimeConfig>,
    /// 解析並驗證過的 Project 清單，依設定檔中出現的順序（task 3.1；spec `pipeline-config`
    /// 「Project 區段結構」）。
    pub projects: Vec<ProjectDef>,
    /// 狀態檔的實際路徑；`None` 表示沒有狀態檔、狀態只存在記憶體（內嵌設定，或零設定模式找不到
    /// `LOCALAPPDATA`——spec `pipeline-config`「狀態檔位置」；repo-projects task 4.1）。有設定檔時一律有路徑，
    /// 不論有沒有 project。
    pub state_path: Option<PathBuf>,
    /// 這份設定實際來自哪裡；`main` 用它印出「用了哪個設定」（review round 1 finding 2：
    /// `load` 的回傳型別改回單純的 `Result<Config, ConfigError>`，來源資訊改放進
    /// `Config` 自己的欄位，不再另外回一個 tuple）。
    pub source: ConfigSource,
}

/// `[server]` 區塊。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServerConfig {
    /// 伺服器監聽位址：IP 只能是 `127.0.0.1` 或 `::1`、埠不得為 80（埠 0 允許，測試用）。理由：
    /// 來源檢查（`crate::source_check`）只認 `127.0.0.1`／`localhost`／`[::1]` 加明確埠的 `Host`，
    /// 其他 loopback 位址（例如 `127.0.0.2`）或埠 80（瀏覽器會省略預設埠）會讓整個儀表板與
    /// 啟動器 403。
    pub listen: SocketAddr,
}

/// `[polling]` 區塊。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PollingConfig {
    /// 重新拿整份 snapshot 的間隔秒數；不得為 0。
    pub resnapshot_secs: u64,
    /// WSL 探測失敗後的固定重試間隔秒數；不得為 0。
    pub wsl_probe_secs: u64,
}

/// 一筆 `[[runtime]]`。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimeConfig {
    /// 這個 runtime 在狀態庫裡的 id；不得為空白、不得與其他筆重複。
    pub id: String,
    /// runtime 種類；目前只接受 `"herdr"`。
    pub kind: String,
    /// 連線端點；`socket`／`wsl`／`command` 三選一，三者都省略時是 [`HerdrEndpoint::Default`]。
    pub endpoint: HerdrEndpoint,
}

/// 命令列參數：`--config` 與 `--exit-when-idle`。
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Args {
    /// `--config <path>` 指定的路徑；沒給就是 `None`。
    pub config: Option<PathBuf>,
    /// 有沒有給 `--exit-when-idle`（desktop-launch-notify task 2.1；spec `desktop-launch`
    /// 「閒置自動結束」；design D6）：給了就在沒有任何畫面連線時自行結束，見
    /// `cockpit::app::shutdown_signal`。
    pub exit_when_idle: bool,
}

/// [`Config::source`] 實際採用了哪個設定來源。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConfigSource {
    /// 來自 `--config <path>` 指定的檔案；已經依注入的 `cwd` 解析成絕對路徑（review round 1
    /// finding 1）。
    Explicit(PathBuf),
    /// 來自工作目錄的 `cockpit.toml`。
    Cwd(PathBuf),
    /// 沒有 `--config`、工作目錄也沒有 `cockpit.toml`：全部用預設值。
    ZeroConfig,
    /// 直接呼叫 [`parse_toml`]：沒有檔案，文字是呼叫端傳進來的。
    Inline,
}

/// 設定載入與驗證失敗的原因。
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    /// `--config <path>` 指定的檔案不存在。
    #[error("設定檔不存在：{}", path.display())]
    NotFound {
        /// 找不到的路徑。
        path: PathBuf,
    },
    /// 設定檔存在但讀取失敗（權限、I/O 等）。
    #[error("讀取設定檔失敗（{}）：{source}", path.display())]
    Read {
        /// 讀取失敗的路徑。
        path: PathBuf,
        /// 底層 I/O 錯誤。
        #[source]
        source: std::io::Error,
    },
    /// TOML 語法錯誤，讀不成 [`RawConfig`]。
    #[error("解析設定檔失敗（{path}）：{message}")]
    Parse {
        /// 出錯的檔案路徑；直接呼叫 [`parse_toml`]（沒有檔案）時是 `"<inline>"`。
        path: String,
        /// 底層解析器的錯誤訊息。
        message: String,
    },
    /// TOML 語法正確，但驗證規則不過（未知 kind、端點三選一、id 重複／空白、`listen` 不合規則、
    /// 秒數為 0……）；訊息裡含 runtime id（或序號）與原因。
    #[error("{0}")]
    Invalid(String),
}

/// 解析命令列參數：`--config <path>`、`--config=<path>` 與布林旗標 `--exit-when-idle`
/// （desktop-launch-notify task 2.1；design D6），順序不拘。`--config` 重複時以最後一個為準
/// （既有行為）；`--exit-when-idle` 重複視為錯誤。
///
/// # Errors
///
/// 出現任何其他參數（包含 `--config` 後面沒接路徑、`--exit-when-idle=<值>`）或
/// `--exit-when-idle` 重複時都回傳 [`ConfigError::Invalid`]。
pub fn parse_args(argv: &[String]) -> Result<Args, ConfigError> {
    let mut config = None;
    let mut exit_when_idle = false;
    let mut i = 0;
    while i < argv.len() {
        let arg = argv[i].as_str();
        if arg == "--exit-when-idle" {
            if exit_when_idle {
                return Err(ConfigError::Invalid(
                    "--exit-when-idle 重複指定".to_string(),
                ));
            }
            exit_when_idle = true;
            i += 1;
        } else if let Some(value) = arg.strip_prefix("--config=") {
            config = Some(PathBuf::from(value));
            i += 1;
        } else if arg == "--config" {
            let value = argv
                .get(i + 1)
                .ok_or_else(|| ConfigError::Invalid("--config 需要接一個路徑參數".to_string()))?;
            config = Some(PathBuf::from(value));
            i += 2;
        } else {
            return Err(ConfigError::Invalid(format!("不明參數：{arg}")));
        }
    }
    Ok(Args {
        config,
        exit_when_idle,
    })
}

/// 依 D16 的來源順序載入設定：`args.config` 有給就讀那個檔案（不存在即失敗）；否則讀
/// `cwd.join("cockpit.toml")`（存在就讀）；否則回傳全預設值的零設定。
///
/// `args.config` 若是相對路徑，依注入的 `cwd` 解析（`cwd.join(path)`），不是行程真正的
/// 工作目錄；絕對路徑原樣使用（review round 1 finding 1：原本直接把相對路徑丟給
/// `read_to_string`，基準會變成行程 cwd，跟「工作目錄以參數注入」的設計前提不一致）。
/// 錯誤訊息裡的路徑一律是解析後的完整路徑。
///
/// `cwd.join("cockpit.toml")` 探測改用 `read_to_string` 直接讀（review round 1
/// finding 4）：讀不到（`ErrorKind::NotFound`）才落回零設定；其他 I/O 錯誤（例如把
/// `cockpit.toml` 建成目錄、權限不足）一律回 [`ConfigError::Read`]，不會被靜默吃掉、
/// fail-open 成零設定。
///
/// `lookup_env` 只用來在零設定模式取 `LOCALAPPDATA` 決定狀態檔位置（repo-projects task 4.1，design D5）；
/// 本機 HERDR 預設路徑解析在 `cockpit-herdr` 的工廠。測試注入固定值，不碰使用者真的環境變數。
///
/// # Errors
///
/// 指定的檔案不存在、讀取失敗、TOML 語法錯誤、或驗證規則不過時回傳 [`ConfigError`]。
pub fn load(
    args: &Args,
    cwd: &Path,
    lookup_env: &dyn Fn(&str) -> Option<String>,
) -> Result<Config, ConfigError> {
    if let Some(explicit) = &args.config {
        let resolved = if explicit.is_absolute() {
            explicit.clone()
        } else {
            cwd.join(explicit)
        };
        let text = std::fs::read_to_string(&resolved).map_err(|source| {
            if source.kind() == std::io::ErrorKind::NotFound {
                ConfigError::NotFound {
                    path: resolved.clone(),
                }
            } else {
                ConfigError::Read {
                    path: resolved.clone(),
                    source,
                }
            }
        })?;
        let core = parse_toml_labelled(&text, &resolved.display().to_string())?;
        return Ok(core.with_source(ConfigSource::Explicit(resolved), lookup_env));
    }

    let candidate = cwd.join("cockpit.toml");
    match std::fs::read_to_string(&candidate) {
        Ok(text) => {
            let core = parse_toml_labelled(&text, &candidate.display().to_string())?;
            Ok(core.with_source(ConfigSource::Cwd(candidate), lookup_env))
        }
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => {
            Ok(zero_config().with_source(ConfigSource::ZeroConfig, lookup_env))
        }
        Err(source) => Err(ConfigError::Read {
            path: candidate,
            source,
        }),
    }
}

/// 純函數：解析＋驗證一段 TOML 文字，不碰檔案系統。測試主要打這個。`Config::source` 固定是
/// [`ConfigSource::Inline`]（沒有實際檔案）。
///
/// # Errors
///
/// TOML 語法錯誤或驗證規則不過時回傳 [`ConfigError`]（[`ConfigError::Parse`] 的 `path` 固定是
/// `"<inline>"`）。
pub fn parse_toml(text: &str) -> Result<Config, ConfigError> {
    Ok(parse_toml_labelled(text, "<inline>")?.with_source(ConfigSource::Inline, &|_| None))
}

/// [`parse_toml`] 的內部實作：解析錯誤的 `path` 標籤可以換成真實檔案路徑，供 [`load`] 共用。
/// 回傳還沒套上 [`ConfigSource`] 的 [`ConfigCore`]，來源交給呼叫端決定。
fn parse_toml_labelled(text: &str, path_label: &str) -> Result<ConfigCore, ConfigError> {
    let raw: RawConfig = toml::from_str(text).map_err(|error| ConfigError::Parse {
        path: path_label.to_string(),
        message: error.to_string(),
    })?;
    validate(raw)
}

/// 全預設值 + 零設定 runtime 清單；空的 [`RawConfig`] 一定驗證得過，`expect` 沒有失敗風險。
fn zero_config() -> ConfigCore {
    let empty = RawConfig {
        server: None,
        polling: None,
        runtime: Vec::new(),
        project: Vec::new(),
        state: None,
    };
    validate(empty).expect("空的 RawConfig 一定能通過驗證")
}

/// [`validate`] 的產出：[`Config`] 扣掉 `source`／`state_path` 欄位。`validate` 本身不知道
/// 自己是被 [`load`] 的哪個分支（`--config`／`cockpit.toml`／零設定）還是 [`parse_toml`]
/// 呼叫，所以先回傳這個，來源與依來源解出的狀態檔路徑由呼叫端用 [`ConfigCore::with_source`]
/// 補上（spec `pipeline-config`「狀態檔位置」：相對路徑相對於設定檔目錄解析，`Inline`／
/// `ZeroConfig` 沒有目錄可依附）。
struct ConfigCore {
    server: ServerConfig,
    polling: PollingConfig,
    runtimes: Vec<RuntimeConfig>,
    projects: Vec<ProjectDef>,
    /// `[state] path` 驗證過（非空字串）但尚未解析成絕對路徑的原始值。
    state_path_raw: Option<String>,
}

impl ConfigCore {
    /// 補上來源，變成完整的 [`Config`]；同時依來源解出 `state_path`（零設定模式從 `lookup_env` 取
    /// `LOCALAPPDATA`）。
    fn with_source(
        self,
        source: ConfigSource,
        lookup_env: &dyn Fn(&str) -> Option<String>,
    ) -> Config {
        let state_path = resolve_state_path(self.state_path_raw.as_deref(), &source, lookup_env);
        Config {
            server: self.server,
            polling: self.polling,
            runtimes: self.runtimes,
            projects: self.projects,
            state_path,
            source,
        }
    }
}

/// 決定狀態檔的實際路徑（spec `pipeline-config`「狀態檔位置」；repo-projects task 4.1，design D5）：
/// - 有設定檔：未給 `state_path_raw` 時為設定檔目錄下的 `cockpit.state.json`，給了相對路徑時相對於設定檔目錄
///   解析，給絕對路徑時照用。不論有沒有 project（狀態檔可能含畫面加入的 Repo Project）。
/// - 零設定模式：`%LOCALAPPDATA%\ai-cockpit\cockpit.state.json`；`LOCALAPPDATA` 不存在或為空字串時沒有狀態檔
///   （啟動時由 `app` 記 warn）。這裡只決定路徑，不建立資料夾——資料夾在第一次寫入時才建立。
/// - 內嵌設定（測試用）：沒有狀態檔。
fn resolve_state_path(
    state_path_raw: Option<&str>,
    source: &ConfigSource,
    lookup_env: &dyn Fn(&str) -> Option<String>,
) -> Option<PathBuf> {
    let config_path = match source {
        ConfigSource::Explicit(path) | ConfigSource::Cwd(path) => path,
        ConfigSource::ZeroConfig => {
            return lookup_env(ZERO_CONFIG_STATE_DIR_ENV)
                .filter(|dir| !dir.is_empty())
                .map(|dir| {
                    PathBuf::from(dir)
                        .join(ZERO_CONFIG_STATE_DIR)
                        .join(STATE_FILE_NAME)
                });
        }
        ConfigSource::Inline => return None,
    };
    let config_dir = config_path.parent().unwrap_or_else(|| Path::new(""));
    Some(match state_path_raw {
        Some(raw) => {
            let candidate = PathBuf::from(raw);
            if candidate.is_absolute() {
                candidate
            } else {
                config_dir.join(candidate)
            }
        }
        None => config_dir.join(STATE_FILE_NAME),
    })
}

/// 設定檔的原始（未驗證）結構；每個都 `deny_unknown_fields`，未知欄位一律視為錯誤。
/// `Option<T>` 欄位缺席時 serde 會自動當 `None`，不需要額外的 `#[serde(default)]`；
/// `runtime` 是 `Vec`，缺席時要靠 `#[serde(default)]` 才會是空陣列而不是必填錯誤。
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawConfig {
    server: Option<RawServer>,
    polling: Option<RawPolling>,
    #[serde(default)]
    runtime: Vec<RawRuntime>,
    #[serde(default)]
    project: Vec<RawProject>,
    state: Option<RawState>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawServer {
    listen: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPolling {
    resnapshot_secs: Option<u64>,
    wsl_probe_secs: Option<u64>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRuntime {
    id: String,
    kind: String,
    socket: Option<String>,
    wsl: Option<RawWsl>,
    command: Option<Vec<String>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawWsl {
    distro: String,
    socket: String,
}

/// 一筆 `[[project]]`（spec `pipeline-config`「Project 區段結構」）。
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawProject {
    // fix round 1 finding 2：`Option`，不是必填 `String`——缺漏時要能辨識出「這筆缺
    // id」並產生 spec 規定的序號路徑（`project[<ordinal>]`），不能讓 `toml::from_str`
    // 直接擋成一般的「missing field id」。
    id: Option<String>,
    name: Option<String>,
    stages: Vec<String>,
    #[serde(default)]
    workstream: Vec<RawWorkstream>,
    #[serde(default)]
    task: Vec<RawTask>,
}

/// 一筆 `[[project.workstream]]`。
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawWorkstream {
    // fix round 1 finding 2：同 `RawProject::id`。
    id: Option<String>,
    name: Option<String>,
    binding: Option<RawBinding>,
}

/// `[[project.workstream]]` 的 `binding` 表。
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawBinding {
    runtime: String,
    workspace: String,
    pane_label: Option<String>,
    cwd: Option<String>,
    agent: Option<String>,
}

/// 一筆 `[[project.task]]`。
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTask {
    // fix round 1 finding 2：同 `RawProject::id`。
    id: Option<String>,
    title: Option<String>,
    workstream: String,
    stage: Option<String>,
    #[serde(default)]
    depends_on: Vec<String>,
}

/// `[state]` 區段。
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawState {
    path: Option<String>,
}

/// 把 [`RawConfig`] 套上預設值、驗證規則，變成 [`ConfigCore`]；驗證依 spec
/// 「驗證錯誤指出是哪一筆」的順序逐一檢查：kind、端點三選一、`command` 非空、id 非空白、
/// id 不重複，再來是 `server.listen`（IP 只能是 `127.0.0.1`／`::1`、埠不得為 80）與兩個間隔秒數
/// （非 0）。
fn validate(raw: RawConfig) -> Result<ConfigCore, ConfigError> {
    let listen_str = raw
        .server
        .and_then(|server| server.listen)
        .unwrap_or_else(|| DEFAULT_LISTEN.to_string());
    let resnapshot_secs = raw
        .polling
        .as_ref()
        .and_then(|polling| polling.resnapshot_secs)
        .unwrap_or(DEFAULT_RESNAPSHOT_SECS);
    let wsl_probe_secs = raw
        .polling
        .as_ref()
        .and_then(|polling| polling.wsl_probe_secs)
        .unwrap_or(DEFAULT_WSL_PROBE_SECS);

    let mut seen_ids: HashSet<String> = HashSet::new();
    let mut runtimes = Vec::with_capacity(raw.runtime.len());
    for (index, raw_runtime) in raw.runtime.into_iter().enumerate() {
        let ordinal = index + 1;
        let RawRuntime {
            id,
            kind,
            socket,
            wsl,
            command,
        } = raw_runtime;

        if kind != "herdr" {
            return Err(ConfigError::Invalid(format!(
                "runtime {id}：kind 只接受 herdr（收到 {kind}）"
            )));
        }

        let mut given = Vec::with_capacity(3);
        if socket.is_some() {
            given.push("socket");
        }
        if wsl.is_some() {
            given.push("wsl");
        }
        if command.is_some() {
            given.push("command");
        }
        if given.len() >= 2 {
            return Err(ConfigError::Invalid(format!(
                "runtime {id}：socket、wsl、command 三選一，收到 {}",
                given.join("、")
            )));
        }

        if let Some(command) = &command
            && command.is_empty()
        {
            return Err(ConfigError::Invalid(format!(
                "runtime {id}：command 不得為空陣列"
            )));
        }

        if id.trim().is_empty() {
            return Err(ConfigError::Invalid(format!(
                "第 {ordinal} 筆 runtime：id 不得為空"
            )));
        }

        if !seen_ids.insert(id.clone()) {
            return Err(ConfigError::Invalid(format!("runtime id 重複：{id}")));
        }

        let endpoint = if let Some(socket) = socket {
            HerdrEndpoint::Socket(PathBuf::from(socket))
        } else if let Some(wsl) = wsl {
            HerdrEndpoint::Wsl {
                distro: wsl.distro,
                socket: wsl.socket,
            }
        } else if let Some(command) = command {
            HerdrEndpoint::Command(command)
        } else {
            HerdrEndpoint::Default
        };

        runtimes.push(RuntimeConfig { id, kind, endpoint });
    }

    // fix round 1 finding 1：binding.runtime 只能是設定檔明確宣告的 `[[runtime]]` id
    // （spec `pipeline-config`「Project 區段驗證」原文：「binding 的 runtime 不是
    // 設定檔中任何一筆 [[runtime]] 的 id」）——這個集合要在零設定注入 `local` **之前**
    // 拍照，不然沒寫任何 `[[runtime]]` 卻寫了 `binding.runtime = "local"` 會被誤判通過。
    let declared_runtime_ids: HashSet<&str> =
        runtimes.iter().map(|runtime| runtime.id.as_str()).collect();
    let projects = validate_projects(raw.project, &declared_runtime_ids)?;
    let state_path_raw = validate_state(raw.state)?;

    if runtimes.is_empty() {
        runtimes.push(RuntimeConfig {
            id: ZERO_CONFIG_RUNTIME_ID.to_string(),
            kind: "herdr".to_string(),
            endpoint: HerdrEndpoint::Default,
        });
    }

    let listen: SocketAddr = listen_str.parse().map_err(|_| {
        ConfigError::Invalid(format!(
            "server.listen 必須是 127.0.0.1 或 ::1 加埠：{listen_str}"
        ))
    })?;
    if listen.ip() != IpAddr::V4(Ipv4Addr::LOCALHOST)
        && listen.ip() != IpAddr::V6(Ipv6Addr::LOCALHOST)
    {
        return Err(ConfigError::Invalid(format!(
            "server.listen 的位址只能是 127.0.0.1 或 ::1（來源檢查只認這兩種 loopback 寫法）：{listen_str}"
        )));
    }
    if listen.port() == 80 {
        return Err(ConfigError::Invalid(format!(
            "server.listen 的埠不得為 80（瀏覽器會省略預設埠，來源檢查會把 Host 擋成 403）：{listen_str}"
        )));
    }

    if resnapshot_secs == 0 {
        return Err(ConfigError::Invalid(
            "polling.resnapshot_secs 不得為 0".to_string(),
        ));
    }
    if wsl_probe_secs == 0 {
        return Err(ConfigError::Invalid(
            "polling.wsl_probe_secs 不得為 0".to_string(),
        ));
    }

    Ok(ConfigCore {
        server: ServerConfig { listen },
        polling: PollingConfig {
            resnapshot_secs,
            wsl_probe_secs,
        },
        runtimes,
        projects,
        state_path_raw,
    })
}

/// project／workstream／task 的 id 規則（spec `pipeline-config`「Project 區段驗證」）：
/// `^[A-Za-z0-9_-]{1,64}$`。手寫字元檢查而非 regex crate（task 3.1 約束：不新增依賴）。
fn is_valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// project／workstream／task 共用：驗證一筆的 raw id。id 欄位在 `Raw*` 結構故意宣告成
/// `Option<String>`（不是必填 `String`）——如果宣告成必填，缺漏 `id = "..."` 這一行時
/// `toml::from_str` 會直接以一般的「missing field `id`」擋下，我們就拿不到 spec 規定
/// 「該筆 id 本身缺漏或非法時改以序號識別」的 `<prefix>[<ordinal>]` 路徑（fix round 1
/// finding 2）。`prefix` 已經是「這一層」的完整路徑前綴（例如 `"project"`、
/// `"project.p.workstream"`、`"project.p.task"`），加上 `[<ordinal>]` 就是 spec 例子
/// 裡的 `project.<pid>.task[2]` 格式。缺漏與格式不符（含空字串、非法字元、超長）共用
/// 同一種「序號路徑」，訊息文字分開方便除錯，但都可以用 `contains` 斷言命中規則。
fn validate_entry_id(
    prefix: &str,
    ordinal: usize,
    id: Option<String>,
) -> Result<String, ConfigError> {
    match id {
        None => Err(ConfigError::Invalid(format!(
            "{prefix}[{ordinal}]：id 缺漏"
        ))),
        Some(id) if !is_valid_id(&id) => Err(ConfigError::Invalid(format!(
            "{prefix}[{ordinal}]：id 格式錯誤（{id}）"
        ))),
        Some(id) => Ok(id),
    }
}

/// 驗證並轉換全部 `[[project]]`；`runtime_ids` 是設定檔明確宣告的 `[[runtime]]` id
/// 集合（不含零設定自動補的那一筆），binding 的 `runtime` 必須在這裡面。
fn validate_projects(
    raw_projects: Vec<RawProject>,
    runtime_ids: &HashSet<&str>,
) -> Result<Vec<ProjectDef>, ConfigError> {
    let mut seen_project_ids: HashSet<String> = HashSet::new();
    let mut projects = Vec::with_capacity(raw_projects.len());
    for (index, raw_project) in raw_projects.into_iter().enumerate() {
        projects.push(validate_project(
            index,
            raw_project,
            &mut seen_project_ids,
            runtime_ids,
        )?);
    }
    Ok(projects)
}

fn validate_project(
    index: usize,
    raw: RawProject,
    seen_project_ids: &mut HashSet<String>,
    runtime_ids: &HashSet<&str>,
) -> Result<ProjectDef, ConfigError> {
    let ordinal = index + 1;
    let RawProject {
        id,
        name,
        stages,
        workstream,
        task,
    } = raw;

    let id = validate_entry_id("project", ordinal, id)?;
    let project_label = format!("project.{id}");
    if !seen_project_ids.insert(id.clone()) {
        return Err(ConfigError::Invalid(format!(
            "{project_label}：project id 重複"
        )));
    }

    validate_stages(&project_label, &stages)?;
    let display_name = name.unwrap_or_else(|| id.clone());

    let mut seen_workstream_ids: HashSet<String> = HashSet::new();
    let mut workstreams = Vec::with_capacity(workstream.len());
    for (ws_index, raw_ws) in workstream.into_iter().enumerate() {
        workstreams.push(validate_workstream(
            &project_label,
            ws_index,
            raw_ws,
            &mut seen_workstream_ids,
            runtime_ids,
        )?);
    }
    let workstream_ids: HashSet<&str> = workstreams.iter().map(|w| w.id.as_str()).collect();

    let mut seen_task_ids: HashSet<String> = HashSet::new();
    let mut tasks = Vec::with_capacity(task.len());
    for (task_index, raw_task) in task.into_iter().enumerate() {
        tasks.push(validate_task(
            &project_label,
            task_index,
            raw_task,
            &mut seen_task_ids,
            &workstream_ids,
            &stages,
        )?);
    }

    validate_depends_on(&project_label, &tasks)?;

    Ok(ProjectDef {
        id: ProjectId::new(id),
        name: display_name,
        stages,
        workstreams,
        tasks,
        repo: None,
    })
}

/// `stages`：非空、不含空字串、不重複（spec `pipeline-config`「Project 區段驗證」）。
fn validate_stages(project_label: &str, stages: &[String]) -> Result<(), ConfigError> {
    if stages.is_empty() {
        return Err(ConfigError::Invalid(format!(
            "{project_label}.stages：不得為空"
        )));
    }
    let mut seen: HashSet<&str> = HashSet::new();
    for stage in stages {
        if stage.is_empty() {
            return Err(ConfigError::Invalid(format!(
                "{project_label}.stages：不得含空字串"
            )));
        }
        if !seen.insert(stage.as_str()) {
            return Err(ConfigError::Invalid(format!(
                "{project_label}.stages：重複（{stage}）"
            )));
        }
    }
    Ok(())
}

fn validate_workstream(
    project_label: &str,
    index: usize,
    raw: RawWorkstream,
    seen_ids: &mut HashSet<String>,
    runtime_ids: &HashSet<&str>,
) -> Result<WorkstreamDef, ConfigError> {
    let ordinal = index + 1;
    let RawWorkstream { id, name, binding } = raw;

    let id = validate_entry_id(&format!("{project_label}.workstream"), ordinal, id)?;
    let ws_label = format!("{project_label}.workstream.{id}");
    if !seen_ids.insert(id.clone()) {
        return Err(ConfigError::Invalid(format!(
            "{ws_label}：workstream id 重複"
        )));
    }

    let display_name = name.unwrap_or_else(|| id.clone());
    let binding = binding
        .map(|raw_binding| validate_binding(&ws_label, raw_binding, runtime_ids))
        .transpose()?;

    Ok(WorkstreamDef {
        id: WorkstreamId::new(id),
        name: display_name,
        binding,
        pinned_pane: None,
    })
}

/// `binding`：`runtime` 須是設定中已存在的 runtime id、`workspace` 非空字串、給了的
/// `pane_label`／`cwd`／`agent` 也不得是空字串（spec `pipeline-config`「Project 區段驗證」）。
fn validate_binding(
    ws_label: &str,
    raw: RawBinding,
    runtime_ids: &HashSet<&str>,
) -> Result<BindingSpec, ConfigError> {
    let RawBinding {
        runtime,
        workspace,
        pane_label,
        cwd,
        agent,
    } = raw;

    if !runtime_ids.contains(runtime.as_str()) {
        return Err(ConfigError::Invalid(format!(
            "{ws_label}.binding.runtime：未設定的 runtime（{runtime}）"
        )));
    }
    if workspace.is_empty() {
        return Err(ConfigError::Invalid(format!(
            "{ws_label}.binding.workspace：不得為空字串"
        )));
    }
    if let Some(pane_label) = &pane_label
        && pane_label.is_empty()
    {
        return Err(ConfigError::Invalid(format!(
            "{ws_label}.binding.pane_label：不得為空字串"
        )));
    }
    if let Some(cwd) = &cwd
        && cwd.is_empty()
    {
        return Err(ConfigError::Invalid(format!(
            "{ws_label}.binding.cwd：不得為空字串"
        )));
    }
    if let Some(agent) = &agent
        && agent.is_empty()
    {
        return Err(ConfigError::Invalid(format!(
            "{ws_label}.binding.agent：不得為空字串"
        )));
    }

    Ok(BindingSpec {
        runtime: RuntimeId::new(runtime),
        workspace,
        pane_label,
        cwd,
        agent,
    })
}

fn validate_task(
    project_label: &str,
    index: usize,
    raw: RawTask,
    seen_ids: &mut HashSet<String>,
    workstream_ids: &HashSet<&str>,
    stages: &[String],
) -> Result<TaskDef, ConfigError> {
    let ordinal = index + 1;
    let RawTask {
        id,
        title,
        workstream,
        stage,
        depends_on,
    } = raw;

    let id = validate_entry_id(&format!("{project_label}.task"), ordinal, id)?;
    let task_label = format!("{project_label}.task.{id}");
    if !seen_ids.insert(id.clone()) {
        return Err(ConfigError::Invalid(format!("{task_label}：task id 重複")));
    }

    if !workstream_ids.contains(workstream.as_str()) {
        return Err(ConfigError::Invalid(format!(
            "{task_label}.workstream：不存在（{workstream}）"
        )));
    }

    let resolved_stage = match stage {
        Some(stage) => {
            if !stages.iter().any(|s| s == &stage) {
                return Err(ConfigError::Invalid(format!(
                    "{task_label}.stage：不在 stages（{stage}）"
                )));
            }
            stage
        }
        None => stages[0].clone(),
    };

    let display_title = title.unwrap_or_else(|| id.clone());

    Ok(TaskDef {
        id: TaskId::new(id),
        title: display_title,
        workstream: WorkstreamId::new(workstream),
        stage: resolved_stage,
        depends_on: depends_on.into_iter().map(TaskId::new).collect(),
    })
}

/// `depends_on`：只能指向同一 Project 內存在的 task、不可指向自己、不可成環
/// （spec `pipeline-config`「Project 區段驗證」）。
fn validate_depends_on(project_label: &str, tasks: &[TaskDef]) -> Result<(), ConfigError> {
    let task_ids: HashSet<&str> = tasks.iter().map(|task| task.id.as_str()).collect();

    for task in tasks {
        for dep in &task.depends_on {
            if dep.as_str() == task.id.as_str() {
                return Err(ConfigError::Invalid(format!(
                    "{project_label}.task.{}.depends_on：不可依賴自己（{}）",
                    task.id, dep
                )));
            }
            if !task_ids.contains(dep.as_str()) {
                return Err(ConfigError::Invalid(format!(
                    "{project_label}.task.{}.depends_on：依賴不存在的 task（{}）",
                    task.id, dep
                )));
            }
        }
    }

    detect_dependency_cycle(project_label, tasks)
}

/// 標準三色 DFS 找環；`marks` 只用 `Visiting`／`Done` 兩色，未出現在 map 裡視為白色。
enum VisitMark {
    Visiting,
    Done,
}

fn detect_dependency_cycle(project_label: &str, tasks: &[TaskDef]) -> Result<(), ConfigError> {
    let deps: HashMap<&str, &[TaskId]> = tasks
        .iter()
        .map(|task| (task.id.as_str(), task.depends_on.as_slice()))
        .collect();

    let mut marks: HashMap<&str, VisitMark> = HashMap::new();
    for task in tasks {
        let mut path: Vec<&str> = Vec::new();
        visit_for_cycle(
            task.id.as_str(),
            &deps,
            &mut marks,
            &mut path,
            project_label,
        )?;
    }
    Ok(())
}

fn visit_for_cycle<'a>(
    node: &'a str,
    deps: &HashMap<&'a str, &'a [TaskId]>,
    marks: &mut HashMap<&'a str, VisitMark>,
    path: &mut Vec<&'a str>,
    project_label: &str,
) -> Result<(), ConfigError> {
    match marks.get(node) {
        Some(VisitMark::Done) => return Ok(()),
        Some(VisitMark::Visiting) => {
            let start = path.iter().position(|n| *n == node).unwrap_or(0);
            let mut cycle: Vec<&str> = path[start..].to_vec();
            cycle.push(node);
            return Err(ConfigError::Invalid(format!(
                "{project_label}.task.{node}.depends_on：形成環（{}）",
                cycle.join(" -> ")
            )));
        }
        None => {}
    }

    marks.insert(node, VisitMark::Visiting);
    path.push(node);
    if let Some(children) = deps.get(node) {
        for child in children.iter() {
            visit_for_cycle(child.as_str(), deps, marks, path, project_label)?;
        }
    }
    path.pop();
    marks.insert(node, VisitMark::Done);
    Ok(())
}

/// `[state]`：只有 `path`，給了就不得是空字串（spec `pipeline-config`「狀態檔位置」）；
/// 相對／絕對路徑解析留給 [`resolve_state_path`]（需要設定來源才知道基準目錄）。
fn validate_state(raw: Option<RawState>) -> Result<Option<String>, ConfigError> {
    let Some(RawState { path }) = raw else {
        return Ok(None);
    };
    match path {
        None => Ok(None),
        Some(path) if path.is_empty() => {
            Err(ConfigError::Invalid("state.path：不得為空字串".to_string()))
        }
        Some(path) => Ok(Some(path)),
    }
}
