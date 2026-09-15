//! Task 3.2 驗收測試：`cockpit::runtimes::build` 把 `Config` 轉成 `RuntimeEntry` 清單
//! （design D16；spec 相關段落見 task-3.2-brief.md）。
//!
//! 用 `cockpit::config::parse_toml` 組出 `Config`，確認 runtime 組裝：
//! - 保留設定檔順序與 id／kind／endpoint 描述（依 2.7 `Connector::describe()` 的格式）；
//! - 省略端點時走 `HerdrEndpoint::Default`；
//! - 工廠回錯誤時附上是哪一筆 runtime id；
//! - `[polling] wsl_probe_secs` 確實轉傳到工廠（review round 1 finding：原本只能靠型別
//!   保證是 `u64`，寫死成別的值也不會被任何測試抓到）。

use std::time::Duration;

use cockpit::config::{self, Config, ConfigSource, PollingConfig, RuntimeConfig, ServerConfig};
use cockpit::runtimes::{self, BuildError};
use cockpit_herdr::HerdrEndpoint;

/// 本平台 socket 端點 `Connector::describe()` 的前綴（同 2.7 `factory.rs` 測試）。
#[cfg(windows)]
const SOCKET_PREFIX: &str = "named-pipe ";
#[cfg(not(windows))]
const SOCKET_PREFIX: &str = "unix-socket ";

#[test]
fn entries_follow_config_order_and_ids() {
    let toml = r#"
[[runtime]]
id = "a"
kind = "herdr"
socket = "C:\\x\\a.sock"

[[runtime]]
id = "b"
kind = "herdr"
wsl = { distro = "Ubuntu-24.04", socket = "/home/b/.config/herdr/herdr.sock" }

[[runtime]]
id = "c"
kind = "herdr"
command = ["some-bridge", "--arg"]
"#;
    let config = config::parse_toml(toml).expect("設定檔應解析成功");

    let entries = runtimes::build(&config).expect("三筆合法設定都應該組得起來");

    assert_eq!(entries.len(), 3, "應該恰好三個 entry");

    assert_eq!(entries[0].id.as_str(), "a");
    assert_eq!(entries[0].kind, "herdr");
    assert!(
        entries[0].endpoint.starts_with(SOCKET_PREFIX),
        "entry a 的 endpoint 應以 {SOCKET_PREFIX:?} 開頭，實際: {:?}",
        entries[0].endpoint
    );

    assert_eq!(entries[1].id.as_str(), "b");
    assert_eq!(entries[1].kind, "herdr");
    assert!(
        entries[1].endpoint.contains("Ubuntu-24.04"),
        "entry b 的 endpoint 應含發行版名稱，實際: {:?}",
        entries[1].endpoint
    );

    assert_eq!(entries[2].id.as_str(), "c");
    assert_eq!(entries[2].kind, "herdr");
    assert!(
        entries[2].endpoint.contains("some-bridge"),
        "entry c 的 endpoint 應含指令名稱，實際: {:?}",
        entries[2].endpoint
    );

    // 順序＝設定檔順序：id 依序是 a、b、c。
    let ids: Vec<&str> = entries.iter().map(|entry| entry.id.as_str()).collect();
    assert_eq!(ids, vec!["a", "b", "c"], "entry 順序應與設定檔一致");
}

#[test]
fn omitted_endpoint_uses_default() {
    let toml = r#"
[[runtime]]
id = "local"
kind = "herdr"
"#;
    let config = config::parse_toml(toml).expect("設定檔應解析成功");

    let entries = runtimes::build(&config).expect("省略端點應該組得起來");

    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].id.as_str(), "local");
    assert!(
        entries[0].endpoint.starts_with(SOCKET_PREFIX),
        "省略端點應走 Default（同 Socket transport），實際: {:?}",
        entries[0].endpoint
    );
    assert!(
        entries[0].endpoint.contains("herdr.sock"),
        "Default 端點的描述應含預設檔名 herdr.sock，實際: {:?}",
        entries[0].endpoint
    );
}

#[test]
fn empty_command_propagates_build_error_with_id() {
    // `command = []` 在 3.1 已被 config 驗證擋掉（parse_toml 會回 ConfigError::Invalid），
    // 所以這裡繞過 parse_toml，直接組一個帶非法端點的 `Config` 值。
    let config = Config {
        server: ServerConfig {
            listen: "127.0.0.1:7770".parse().expect("合法的 loopback 位址"),
        },
        polling: PollingConfig {
            resnapshot_secs: 30,
            wsl_probe_secs: 60,
        },
        runtimes: vec![RuntimeConfig {
            id: "broken".to_string(),
            kind: "herdr".to_string(),
            endpoint: HerdrEndpoint::Command(Vec::new()),
        }],
        source: ConfigSource::Inline,
    };

    let error = runtimes::build(&config)
        .err()
        .expect("空的 command argv 應該回錯誤");

    let message = error.to_string();
    assert!(
        message.contains("broken"),
        "錯誤訊息應含出錯的 runtime id，實際: {message:?}"
    );
    assert!(matches!(error, BuildError::Factory { id, .. } if id.as_str() == "broken"));
}

#[test]
fn wsl_probe_secs_is_forwarded_to_wsl_runtime() {
    let toml = r#"
[polling]
wsl_probe_secs = 37

[[runtime]]
id = "wsl"
kind = "herdr"
wsl = { distro = "Ubuntu-24.04", socket = "/home/x/.config/herdr/herdr.sock" }

[[runtime]]
id = "win"
kind = "herdr"
socket = "C:\\x\\win.sock"
"#;
    let config = config::parse_toml(toml).expect("設定檔應解析成功");

    let entries = runtimes::build(&config).expect("兩筆合法設定都應該組得起來");

    assert_eq!(entries.len(), 2);

    let wsl_entry = entries
        .iter()
        .find(|entry| entry.id.as_str() == "wsl")
        .expect("應該有一筆 id 為 wsl 的 entry");
    assert_eq!(
        wsl_entry.wsl_retry_after,
        Some(Duration::from_secs(37)),
        "wsl 端點的 wsl_retry_after 應該等於設定檔的 polling.wsl_probe_secs"
    );

    let socket_entry = entries
        .iter()
        .find(|entry| entry.id.as_str() == "win")
        .expect("應該有一筆 id 為 win 的 entry");
    assert_eq!(
        socket_entry.wsl_retry_after, None,
        "socket 端點不是 wsl 型，wsl_retry_after 應該是 None"
    );
}
