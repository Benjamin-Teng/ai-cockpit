//! Runtime 工廠（design D16）：把一個「端點設定」變成一個接好 connector 與（必要時）WSL
//! 探測器的 [`HerdrRuntime`]，外加一段給人看的端點描述。
//!
//! 這個工廠放在 `cockpit-herdr` 而不是 `cockpit`，是為了讓 `cockpit` 完全不依賴
//! `herdr-client`：呼叫端只碰 [`HerdrEndpoint`]／[`BuildOptions`]／[`BuiltRuntime`] 這三個
//! 本 crate 的型別，`NamedPipeConnector`／`UnixSocketConnector`／`ChildStdioConnector` 都不
//! 出現在它的簽章裡。
//!
//! 四種端點（design D16）：
//!
//! | 端點 | connector | 探測器 |
//! |---|---|---|
//! | [`HerdrEndpoint::Socket`] | `NamedPipeConnector`（Windows）／`UnixSocketConnector`（unix） | 無 |
//! | [`HerdrEndpoint::Wsl`] | `ChildStdioConnector` 跑 `wsl.exe -d <distro> -e nc -U <socket>` | [`WslProber`] |
//! | [`HerdrEndpoint::Command`] | `ChildStdioConnector` 跑 `argv` | 無 |
//! | [`HerdrEndpoint::Default`] | 同 `Socket`，路徑取 `default_socket_path_from_env()` | 無 |

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use cockpit_core::RuntimeId;
#[cfg(windows)]
use herdr_client::connector::NamedPipeConnector;
#[cfg(all(unix, not(windows)))]
use herdr_client::connector::UnixSocketConnector;
use herdr_client::connector::{ChildStdioConnector, Connector, default_socket_path_from_env};

use crate::probe::WslProber;
use crate::runtime::{HerdrRuntime, WslProbe};

/// 一個 HERDR runtime 的連線端點設定（design D16）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HerdrEndpoint {
    /// 直接連一個 socket 路徑：Windows 是 named pipe 名稱，unix 是 unix socket 路徑。
    Socket(PathBuf),
    /// 透過 `wsl.exe` 橋接到 WSL 發行版裡的 unix socket（ADR-0002）；會裝 WSL 探測器。
    Wsl {
        /// WSL 發行版名稱，例如 `"Ubuntu-24.04"`。
        distro: String,
        /// 發行版裡的 socket 路徑（WSL 端的路徑，不是 Windows 路徑）。
        socket: String,
    },
    /// 自訂橋接指令：`argv[0]` 是指令，其餘是引數；不探測。
    Command(Vec<String>),
    /// 用 `herdr-client` 的預設 socket 路徑（依環境變數解析），其餘同 [`HerdrEndpoint::Socket`]。
    Default,
}

/// [`build`] 的其他參數（design D16：`options` 至少含 `id` 與 `wsl_probe_secs`）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BuildOptions {
    /// 這個 runtime 在狀態庫裡的 id。
    pub id: RuntimeId,
    /// `wsl` 型端點探測失敗時的固定重試間隔（秒，design D4）；其他端點不使用。
    pub wsl_probe_secs: u64,
}

/// [`build`] 的產出：端點描述（取 `Connector::describe()`，供畫面與日誌顯示）與 runtime 本體。
///
/// `runtime` 是 `Arc<HerdrRuntime>` 而不是 `Arc<dyn AgentRuntime>`：具體型別保留給呼叫端
/// （例如測試要看 [`HerdrRuntime::wsl_distro`]），要當 trait object 用時直接 coerce 即可。
pub struct BuiltRuntime {
    /// 端點描述，例如 `named-pipe C:\x\herdr.sock`、`child wsl.exe -d Ubuntu-24.04 -e nc -U …`。
    pub endpoint: String,
    /// 建好的 runtime。
    pub runtime: Arc<HerdrRuntime>,
}

/// [`build`] 失敗的原因。設定本身不合法（空的 `argv`）或這個平台沒有對應的 transport 時才
/// 會發生；其餘端點一律建得起來（工廠不連線，連不連得上是驅動器的事）。
#[derive(Debug, thiserror::Error)]
pub enum BuildError {
    /// `Command` 端點的 `argv` 是空的，沒有指令可跑。
    #[error("Command 端點的 argv 不能是空的")]
    EmptyCommand,
    /// 這個平台沒有 socket 端點對應的 transport（既不是 Windows 也不是 unix）。
    #[error(
        "這個平台不支援 socket 端點（{path}）：既不是 Windows 的 named pipe 也不是 unix socket"
    )]
    UnsupportedSocket {
        /// 原本要連的路徑，供錯誤訊息辨認是哪一筆設定。
        path: String,
    },
}

/// 依端點設定建立一個 [`HerdrRuntime`]（design D16）。
///
/// # Errors
///
/// [`HerdrEndpoint::Command`] 的 `argv` 為空，或這個平台不支援 socket 端點時回傳
/// [`BuildError`]。
pub fn build(endpoint: HerdrEndpoint, options: BuildOptions) -> Result<BuiltRuntime, BuildError> {
    let (connector, wsl): (Arc<dyn Connector>, Option<WslProbe>) = match endpoint {
        HerdrEndpoint::Socket(path) => (socket_connector(path)?, None),
        HerdrEndpoint::Default => (socket_connector(default_socket_path_from_env())?, None),
        HerdrEndpoint::Wsl { distro, socket } => {
            // ADR-0002／1a spike 2 查證過的橋接指令，逐字如此（沒有 `-N`）。
            let connector = ChildStdioConnector::new(
                "wsl.exe",
                ["-d", distro.as_str(), "-e", "nc", "-U", socket.as_str()],
            );
            let retry_after = Duration::from_secs(options.wsl_probe_secs);
            let probe = WslProbe {
                distro,
                prober: Box::new(WslProber { retry_after }),
                retry_after,
            };
            (Arc::new(connector), Some(probe))
        }
        HerdrEndpoint::Command(argv) => {
            let mut argv = argv.into_iter();
            let command = argv.next().ok_or(BuildError::EmptyCommand)?;
            (Arc::new(ChildStdioConnector::new(command, argv)), None)
        }
    };

    let description = connector.describe();
    Ok(BuiltRuntime {
        endpoint: description,
        runtime: Arc::new(HerdrRuntime::new(options.id, connector, wsl)),
    })
}

/// socket 路徑對應的 connector：Windows 用 named pipe、unix 用 unix socket。
#[cfg(windows)]
fn socket_connector(path: PathBuf) -> Result<Arc<dyn Connector>, BuildError> {
    Ok(Arc::new(NamedPipeConnector::new(path)))
}

/// socket 路徑對應的 connector：Windows 用 named pipe、unix 用 unix socket。
#[cfg(all(unix, not(windows)))]
fn socket_connector(path: PathBuf) -> Result<Arc<dyn Connector>, BuildError> {
    Ok(Arc::new(UnixSocketConnector::new(path)))
}

/// 既不是 Windows 也不是 unix：`herdr-client` 沒有對應的 transport，只能回錯誤。
#[cfg(not(any(windows, unix)))]
fn socket_connector(path: PathBuf) -> Result<Arc<dyn Connector>, BuildError> {
    Err(BuildError::UnsupportedSocket {
        path: path.display().to_string(),
    })
}
