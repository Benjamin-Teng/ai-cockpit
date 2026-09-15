//! Task 2.7 驗收測試：runtime 工廠 `cockpit_herdr::build`（design D16；spec `runtime-driver`）。
//!
//! 四種端點（`Socket`／`Wsl`／`Command`／`Default`）各自對應哪個 connector、要不要裝 WSL
//! 探測器、以及對外的 endpoint 描述字串長什麼樣子。工廠本身不連線，所以這些都是同步測試。

use std::path::PathBuf;

use cockpit_core::{AgentRuntime, RuntimeId};
use cockpit_herdr::{BuildOptions, HerdrEndpoint, build};

/// 測試用的 `BuildOptions`；`wsl_probe_secs` 取一個好辨認的值。
fn options(id: &str) -> BuildOptions {
    BuildOptions {
        id: RuntimeId::new(id),
        wsl_probe_secs: 7,
    }
}

/// 本平台的 `Connector::describe()` 前綴（`herdr-client` 的
/// `NamedPipeConnector`／`UnixSocketConnector`）。
#[cfg(windows)]
const SOCKET_PREFIX: &str = "named-pipe ";
#[cfg(not(windows))]
const SOCKET_PREFIX: &str = "unix-socket ";

#[test]
fn socket_endpoint_describes_named_pipe_or_unix_socket() {
    #[cfg(windows)]
    let path = PathBuf::from(r"C:\x\herdr.sock");
    #[cfg(not(windows))]
    let path = PathBuf::from("/tmp/x.sock");

    let built = build(HerdrEndpoint::Socket(path.clone()), options("win"))
        .expect("Socket 端點在本平台應該建得起來");

    assert!(
        built.endpoint.starts_with(SOCKET_PREFIX),
        "Socket 端點的描述應以 {SOCKET_PREFIX:?} 開頭，實際: {:?}",
        built.endpoint
    );
    assert!(
        built.endpoint.contains(&path.display().to_string()),
        "Socket 端點的描述應含路徑 {:?}，實際: {:?}",
        path.display().to_string(),
        built.endpoint
    );
    assert_eq!(built.runtime.id(), &RuntimeId::new("win"));
    assert_eq!(
        built.runtime.wsl_distro(),
        None,
        "Socket 端點不該裝 WSL 探測器"
    );

    // `Default` 走的是同一條路（`default_socket_path_from_env()` → `Socket`）：至少要拿到
    // 同一種 transport 的描述，且路徑不是空的。真實環境變數會影響路徑內容，所以只斷言
    // 前綴與「前綴後面還有東西」。
    let fallback =
        build(HerdrEndpoint::Default, options("win")).expect("Default 端點在本平台應該建得起來");
    assert!(
        fallback.endpoint.starts_with(SOCKET_PREFIX),
        "Default 端點應與 Socket 同一種 transport，實際: {:?}",
        fallback.endpoint
    );
    assert!(
        fallback.endpoint.len() > SOCKET_PREFIX.len(),
        "Default 端點的描述應含預設 socket 路徑，實際: {:?}",
        fallback.endpoint
    );
    assert_eq!(fallback.runtime.wsl_distro(), None);
}

#[test]
fn wsl_endpoint_describes_child_stdio_and_has_prober() {
    let built = build(
        HerdrEndpoint::Wsl {
            distro: "Ubuntu-24.04".to_string(),
            socket: "/home/x/.config/herdr/herdr.sock".to_string(),
        },
        options("wsl"),
    )
    .expect("Wsl 端點應該建得起來");

    assert!(
        built.endpoint.contains("wsl.exe"),
        "Wsl 端點的描述應含 wsl.exe，實際: {:?}",
        built.endpoint
    );
    assert!(
        built.endpoint.contains("Ubuntu-24.04"),
        "Wsl 端點的描述應含發行版名稱，實際: {:?}",
        built.endpoint
    );
    // 逐字對齊 ADR-0002 與 1a spike 2 查證過的橋接指令（沒有 `-N`）。
    assert_eq!(
        built.endpoint, "child wsl.exe -d Ubuntu-24.04 -e nc -U /home/x/.config/herdr/herdr.sock",
        "Wsl 端點的子程序指令列必須逐字對齊 ADR-0002"
    );
    assert_eq!(
        built.runtime.wsl_distro(),
        Some("Ubuntu-24.04"),
        "Wsl 端點一定要裝探測器"
    );
}

#[test]
fn command_endpoint_has_no_prober() {
    let built = build(
        HerdrEndpoint::Command(vec!["some-bridge".to_string(), "--arg".to_string()]),
        options("cmd"),
    )
    .expect("Command 端點應該建得起來");

    assert!(
        built.endpoint.contains("some-bridge"),
        "Command 端點的描述應含指令名稱，實際: {:?}",
        built.endpoint
    );
    assert!(
        built.endpoint.contains("--arg"),
        "Command 端點的描述應含引數，實際: {:?}",
        built.endpoint
    );
    assert_eq!(
        built.runtime.wsl_distro(),
        None,
        "Command 端點不該裝 WSL 探測器"
    );

    let error = build(HerdrEndpoint::Command(Vec::new()), options("cmd"))
        .err()
        .expect("空的 Command argv 應該回錯誤，不該建出 runtime");
    let message = error.to_string();
    assert!(
        !message.is_empty(),
        "BuildError 應該有可讀的原因文字，實際: {message:?}"
    );
}
