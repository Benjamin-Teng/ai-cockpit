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

use std::collections::HashSet;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};

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

/// 解析並驗證過的完整設定。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
    /// HTTP／WS 伺服器設定。
    pub server: ServerConfig,
    /// 輪詢間隔設定。
    pub polling: PollingConfig,
    /// 要接上的 HERDR runtime 清單，順序與設定檔中出現的順序一致。
    pub runtimes: Vec<RuntimeConfig>,
    /// 這份設定實際來自哪裡；`main` 用它印出「用了哪個設定」（review round 1 finding 2：
    /// `load` 的回傳型別改回單純的 `Result<Config, ConfigError>`，來源資訊改放進
    /// `Config` 自己的欄位，不再另外回一個 tuple）。
    pub source: ConfigSource,
}

/// `[server]` 區塊。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServerConfig {
    /// 伺服器監聽位址；必須是 loopback（`IpAddr::is_loopback()` 為真）。
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

/// 命令列參數（目前只有 `--config`）。
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Args {
    /// `--config <path>` 指定的路徑；沒給就是 `None`。
    pub config: Option<PathBuf>,
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
    /// TOML 語法正確，但驗證規則不過（未知 kind、端點三選一、id 重複／空白、非 loopback、
    /// 秒數為 0……）；訊息裡含 runtime id（或序號）與原因。
    #[error("{0}")]
    Invalid(String),
}

/// 解析命令列參數；目前只認得 `--config <path>` 與 `--config=<path>` 兩種寫法。
///
/// # Errors
///
/// 出現任何其他參數（包含 `--config` 後面沒接路徑）都回傳 [`ConfigError::Invalid`]。
pub fn parse_args(argv: &[String]) -> Result<Args, ConfigError> {
    let mut config = None;
    let mut i = 0;
    while i < argv.len() {
        let arg = argv[i].as_str();
        if let Some(value) = arg.strip_prefix("--config=") {
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
    Ok(Args { config })
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
/// `lookup_env` 目前的驗證規則用不到（本機預設路徑解析在 `cockpit-herdr` 的工廠），簽章保留
/// 給零設定之外的未來用途，並讓呼叫端維持「工作目錄與環境變數以參數注入」的慣例。
///
/// # Errors
///
/// 指定的檔案不存在、讀取失敗、TOML 語法錯誤、或驗證規則不過時回傳 [`ConfigError`]。
pub fn load(
    args: &Args,
    cwd: &Path,
    _lookup_env: &dyn Fn(&str) -> Option<String>,
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
        return Ok(core.with_source(ConfigSource::Explicit(resolved)));
    }

    let candidate = cwd.join("cockpit.toml");
    match std::fs::read_to_string(&candidate) {
        Ok(text) => {
            let core = parse_toml_labelled(&text, &candidate.display().to_string())?;
            Ok(core.with_source(ConfigSource::Cwd(candidate)))
        }
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => {
            Ok(zero_config().with_source(ConfigSource::ZeroConfig))
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
    Ok(parse_toml_labelled(text, "<inline>")?.with_source(ConfigSource::Inline))
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
    };
    validate(empty).expect("空的 RawConfig 一定能通過驗證")
}

/// [`validate`] 的產出：[`Config`] 扣掉 `source` 欄位。`validate` 本身不知道自己是被
/// [`load`] 的哪個分支（`--config`／`cockpit.toml`／零設定）還是 [`parse_toml`] 呼叫，
/// 所以先回傳這個，來源由呼叫端用 [`ConfigCore::with_source`] 補上。
struct ConfigCore {
    server: ServerConfig,
    polling: PollingConfig,
    runtimes: Vec<RuntimeConfig>,
}

impl ConfigCore {
    /// 補上來源，變成完整的 [`Config`]。
    fn with_source(self, source: ConfigSource) -> Config {
        Config {
            server: self.server,
            polling: self.polling,
            runtimes: self.runtimes,
            source,
        }
    }
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

/// 把 [`RawConfig`] 套上預設值、驗證規則，變成 [`ConfigCore`]；驗證依 spec
/// 「驗證錯誤指出是哪一筆」的順序逐一檢查：kind、端點三選一、`command` 非空、id 非空白、
/// id 不重複，再來是 `server.listen`（loopback）與兩個間隔秒數（非 0）。
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

    if runtimes.is_empty() {
        runtimes.push(RuntimeConfig {
            id: ZERO_CONFIG_RUNTIME_ID.to_string(),
            kind: "herdr".to_string(),
            endpoint: HerdrEndpoint::Default,
        });
    }

    let listen: SocketAddr = listen_str.parse().map_err(|_| {
        ConfigError::Invalid(format!("server.listen 必須是 loopback 位址：{listen_str}"))
    })?;
    if !listen.ip().is_loopback() {
        return Err(ConfigError::Invalid(format!(
            "server.listen 必須是 loopback 位址：{listen_str}"
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
    })
}
