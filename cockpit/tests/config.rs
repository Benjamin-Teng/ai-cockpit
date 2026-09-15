//! Task 3.1 驗收測試：`cockpit::config` 的來源順序、零設定、預設值與驗證規則
//! （spec `cockpit-config` 全部；design D16）。

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use cockpit::config::{self, Args, ConfigError, ConfigSource};
use cockpit_herdr::HerdrEndpoint;

/// 每個測試專用的暫存目錄；`tempfile` 依 task 3.1 裁決不可加，改自己建目錄，
/// `Drop` 時清掉（沿用 `herdr-client` 既有測試的 pid + 奈秒後綴慣例）。
struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(tag: &str) -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock 應晚於 UNIX_EPOCH")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "cockpit-config-test-{tag}-{}-{nanos}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("建立測試暫存目錄");
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn write(&self, name: &str, contents: &str) -> PathBuf {
        let file = self.path.join(name);
        fs::write(&file, contents).expect("寫入測試設定檔");
        file
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn no_env(_key: &str) -> Option<String> {
    None
}

/// 設計文件 §8.2 的範例（brief 原文逐字，含註解與 Windows literal string 路徑）。
const DESIGN_DOC_EXAMPLE: &str = r#"
[server]
listen = "127.0.0.1:7770"

[polling]
resnapshot_secs = 30
wsl_probe_secs = 60

[[runtime]]
id = "win"
kind = "herdr"
# socket = 'C:\Users\<user>\AppData\Roaming\herdr\herdr.sock'   # 省略則照 HERDR 規則找預設

[[runtime]]
id = "wsl"
kind = "herdr"
wsl = { distro = "Ubuntu-24.04", socket = "/home/<user>/.config/herdr/herdr.sock" }

# 通用逃生口：任何能把 stdio 接到 HERDR socket 的指令
# [[runtime]]
# id = "other"
# kind = "herdr"
# command = ["some-bridge", "--arg"]
"#;

#[test]
fn zero_config_yields_local_runtime_with_default_socket() {
    let dir = TempDir::new("zero-config");
    let args = Args { config: None };

    let config = config::load(&args, dir.path(), &no_env).expect("零設定應該載入成功");

    assert_eq!(config.source, ConfigSource::ZeroConfig);
    assert_eq!(config.runtimes.len(), 1);
    assert_eq!(config.runtimes[0].id, "local");
    assert_eq!(config.runtimes[0].kind, "herdr");
    assert_eq!(config.runtimes[0].endpoint, HerdrEndpoint::Default);
    assert_eq!(
        config.server.listen,
        "127.0.0.1:7770".parse().expect("字面值可以解析")
    );
    assert_eq!(config.polling.resnapshot_secs, 30);
    assert_eq!(config.polling.wsl_probe_secs, 60);
}

#[test]
fn missing_config_path_fails_with_path() {
    let dir = TempDir::new("missing-path");
    let args = Args {
        config: Some(PathBuf::from("missing.toml")),
    };

    let err = config::load(&args, dir.path(), &no_env).expect_err("不存在的路徑應該失敗");

    assert!(matches!(err, ConfigError::NotFound { .. }));
    assert!(err.to_string().contains("missing.toml"));
}

#[test]
fn design_doc_example_parses() {
    let config = config::parse_toml(DESIGN_DOC_EXAMPLE).expect("設計文件範例應該能解析");

    assert_eq!(config.runtimes.len(), 2);
    assert_eq!(config.runtimes[0].id, "win");
    assert_eq!(config.runtimes[0].kind, "herdr");
    assert_eq!(config.runtimes[0].endpoint, HerdrEndpoint::Default);
    assert_eq!(config.runtimes[1].id, "wsl");
    assert_eq!(config.runtimes[1].kind, "herdr");
    assert_eq!(
        config.runtimes[1].endpoint,
        HerdrEndpoint::Wsl {
            distro: "Ubuntu-24.04".to_string(),
            socket: "/home/<user>/.config/herdr/herdr.sock".to_string(),
        }
    );
    assert_eq!(config.polling.resnapshot_secs, 30);
    assert_eq!(config.polling.wsl_probe_secs, 60);
    assert_eq!(
        config.server.listen,
        "127.0.0.1:7770".parse().expect("字面值可以解析")
    );
}

#[test]
fn unknown_kind_fails_naming_id() {
    let toml = r#"
[[runtime]]
id = "x"
kind = "tmux"
"#;

    let err = config::parse_toml(toml).expect_err("未知 kind 應該失敗");
    let message = err.to_string();
    assert!(message.contains('x'), "訊息應含 id：{message}");
    assert!(message.contains("tmux"), "訊息應含 kind 值：{message}");
}

#[test]
fn unknown_field_fails() {
    let bad_server = r#"
[server]
listen = "127.0.0.1:7770"
port = 1
"#;
    assert!(
        config::parse_toml(bad_server).is_err(),
        "[server] 多餘欄位應該失敗"
    );

    let bad_runtime = r#"
[[runtime]]
id = "x"
kind = "herdr"
foo = 1
"#;
    assert!(
        config::parse_toml(bad_runtime).is_err(),
        "[[runtime]] 多餘欄位應該失敗"
    );
}

#[test]
fn two_endpoints_fail_naming_id() {
    let toml = r#"
[[runtime]]
id = "x"
kind = "herdr"
socket = "some/path"
wsl = { distro = "Ubuntu-24.04", socket = "/some/path" }
"#;

    let err = config::parse_toml(toml).expect_err("同一筆給兩個端點應該失敗");
    let message = err.to_string();
    assert!(message.contains('x'), "訊息應含 id：{message}");
    assert!(message.contains("三選一"), "訊息應含「三選一」：{message}");
}

#[test]
fn duplicate_id_fails() {
    let toml = r#"
[[runtime]]
id = "win"
kind = "herdr"

[[runtime]]
id = "win"
kind = "herdr"
"#;

    let err = config::parse_toml(toml).expect_err("重複 id 應該失敗");
    assert!(err.to_string().contains("win"));
}

#[test]
fn non_loopback_listen_fails() {
    let toml = r#"
[server]
listen = "0.0.0.0:7770"
"#;
    let err = config::parse_toml(toml).expect_err("非 loopback 應該失敗");
    assert!(err.to_string().contains("0.0.0.0"));

    let ipv6_ok = r#"
[server]
listen = "[::1]:7770"
"#;
    let config = config::parse_toml(ipv6_ok).expect("IPv6 loopback 應該可以通過");
    assert!(config.server.listen.ip().is_loopback());
}

#[test]
fn zero_interval_fails() {
    let resnapshot_zero = r#"
[polling]
resnapshot_secs = 0
"#;
    let err = config::parse_toml(resnapshot_zero).expect_err("resnapshot_secs 為 0 應該失敗");
    assert!(err.to_string().contains("resnapshot_secs"));

    let wsl_probe_zero = r#"
[polling]
wsl_probe_secs = 0
"#;
    let err = config::parse_toml(wsl_probe_zero).expect_err("wsl_probe_secs 為 0 應該失敗");
    assert!(err.to_string().contains("wsl_probe_secs"));
}

#[test]
fn parse_args_accepts_config_flag_forms() {
    let args = config::parse_args(&["--config".to_string(), "x.toml".to_string()])
        .expect("分開寫的 --config 應該可以解析");
    assert_eq!(args.config, Some(PathBuf::from("x.toml")));

    let args = config::parse_args(&["--config=x.toml".to_string()])
        .expect("合併寫的 --config=x.toml 應該可以解析");
    assert_eq!(args.config, Some(PathBuf::from("x.toml")));

    let args = config::parse_args(&[]).expect("沒有參數應該回傳 None");
    assert_eq!(args.config, None);

    let err = config::parse_args(&["--unknown".to_string()]).expect_err("未知參數應該失敗");
    assert!(matches!(err, ConfigError::Invalid(_)));
}

// -- 補充：`load()` 三種來源順序中，另外兩種（Explicit／Cwd）的讀檔行為 -----------------
// brief 指定的九個測試 + `parse_args_accepts_config_flag_forms` 都沒有實際打到 `load()`
// 讀真實檔案的 `Explicit`／`Cwd` 分支（`zero_config_...` 只測 `ZeroConfig`、
// `missing_config_path_fails_with_path` 只測讀檔失敗）；補兩個測試堵住這個洞。

#[test]
fn explicit_config_path_is_read_and_reported_as_source() {
    let dir = TempDir::new("explicit-path");
    let file = dir.write(
        "somewhere.toml",
        r#"
[[runtime]]
id = "win"
kind = "herdr"
"#,
    );
    let args = Args {
        config: Some(file.clone()),
    };

    let config = config::load(&args, dir.path(), &no_env).expect("指定檔案應該載入成功");

    assert_eq!(config.source, ConfigSource::Explicit(file));
    assert_eq!(config.runtimes.len(), 1);
    assert_eq!(config.runtimes[0].id, "win");
}

#[test]
fn cwd_config_file_is_used_when_no_explicit_path() {
    let dir = TempDir::new("cwd-config");
    dir.write(
        "cockpit.toml",
        r#"
[[runtime]]
id = "cwd-runtime"
kind = "herdr"
"#,
    );
    let args = Args { config: None };

    let config = config::load(&args, dir.path(), &no_env).expect("cwd 的設定檔應該載入成功");

    assert_eq!(
        config.source,
        ConfigSource::Cwd(dir.path().join("cockpit.toml"))
    );
    assert_eq!(config.runtimes.len(), 1);
    assert_eq!(config.runtimes[0].id, "cwd-runtime");
}

// -- Review round 1 fix：relative --config 依注入的 cwd 解析、cwd 探測讀檔錯誤不再
// fail-open 成零設定 --------------------------------------------------------------

#[test]
fn relative_config_path_resolves_against_injected_cwd() {
    let dir = TempDir::new("relative-config");
    let sub_dir = dir.path().join("sub");
    fs::create_dir_all(&sub_dir).expect("建立 sub 目錄");
    fs::write(
        sub_dir.join("x.toml"),
        r#"
[[runtime]]
id = "rel"
kind = "herdr"
"#,
    )
    .expect("寫入測試設定檔");

    // 證明用的是注入的 cwd，不是行程真正的工作目錄：行程 cwd 底下不該剛好也有這個相對路徑，
    // 不然這個測試就算 `load` 誤用行程 cwd 也會通過，測不出問題。
    assert!(
        !Path::new("sub/x.toml").exists(),
        "行程 cwd 下不該有 sub/x.toml，測試才有意義"
    );

    let args = Args {
        config: Some(PathBuf::from("sub/x.toml")),
    };

    let config = config::load(&args, dir.path(), &no_env).expect("相對路徑應該依注入的 cwd 解析");

    assert_eq!(
        config.source,
        ConfigSource::Explicit(dir.path().join("sub").join("x.toml"))
    );
    assert_eq!(config.runtimes.len(), 1);
    assert_eq!(config.runtimes[0].id, "rel");
}

#[test]
fn cwd_config_read_error_is_not_zero_config() {
    let dir = TempDir::new("cwd-read-error");
    // 把 cockpit.toml 建成目錄：read_to_string 會回傳非 NotFound 的 I/O 錯誤（Windows 上
    // 通常是「拒絕存取」之類的錯誤，重點是它不是 NotFound）。
    fs::create_dir_all(dir.path().join("cockpit.toml")).expect("把 cockpit.toml 建成目錄");
    let args = Args { config: None };

    let err = config::load(&args, dir.path(), &no_env)
        .expect_err("cockpit.toml 讀取失敗不應該被當成零設定");

    assert!(
        matches!(err, ConfigError::Read { .. }),
        "應該是 Read 錯誤，不是別的：{err:?}"
    );
}
