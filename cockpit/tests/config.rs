//! Task 3.1 驗收測試：`cockpit::config` 的來源順序、零設定、預設值與驗證規則
//! （spec `cockpit-config` 全部；design D16）。

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use cockpit::config::{self, Args, ConfigError, ConfigSource};
use cockpit_core::BindingSpec;
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

// -- Task 3.1：`[[project]]`（workstream／binding／task）與 `[state] path`
// （spec `pipeline-config` 全部、`cockpit-config` delta「範例設定檔含 project 可解析」
// 「未知區段」；design D1） ---------------------------------------------------------

/// repo 根的設定範例，內嵌進測試執行檔（同 `cockpit/tests/app.rs` 的 `EXAMPLE_CONFIG`）。
const EXAMPLE_CONFIG_WITH_PROJECT: &str = include_str!("../../cockpit.example.toml");

#[test]
fn example_config_project_list_is_not_empty() {
    let config = config::parse_toml(EXAMPLE_CONFIG_WITH_PROJECT)
        .expect("cockpit.example.toml 應該解析並驗證通過");

    assert!(
        !config.projects.is_empty(),
        "範例設定檔應該示範至少一個 [[project]]"
    );
}

#[test]
fn unknown_top_level_section_pipeline_fails() {
    let toml = r#"
[pipeline]
foo = 1
"#;
    let err = config::parse_toml(toml).expect_err("[pipeline] 未知區段應該失敗");
    assert!(
        err.to_string().contains("pipeline"),
        "訊息應含 pipeline：{err}"
    );
}

// -- Project 區段結構 -----------------------------------------------------------

#[test]
fn minimal_project_parses() {
    let toml = r#"
[[project]]
id = "p"
stages = ["Spec", "Build"]

[[project.workstream]]
id = "be"

[[project.task]]
id = "t1"
workstream = "be"
"#;
    let config = config::parse_toml(toml).expect("最小 Project 應該解析成功");

    assert_eq!(config.projects.len(), 1);
    let project = &config.projects[0];
    assert_eq!(project.id.as_str(), "p");
    assert_eq!(project.name, "p", "未給 name 應該預設等於 id");
    assert_eq!(
        project.stages,
        vec!["Spec".to_string(), "Build".to_string()]
    );

    assert_eq!(project.workstreams.len(), 1);
    let workstream = &project.workstreams[0];
    assert_eq!(workstream.id.as_str(), "be");
    assert_eq!(workstream.name, "be");
    assert!(workstream.binding.is_none(), "沒給 binding 應該是 None");

    assert_eq!(project.tasks.len(), 1);
    let task = &project.tasks[0];
    assert_eq!(task.id.as_str(), "t1");
    assert_eq!(task.title, "t1", "未給 title 應該預設等於 id");
    assert_eq!(task.workstream.as_str(), "be");
    assert_eq!(task.stage, "Spec", "未給 stage 應該預設為 stages 第一個");
    assert!(task.depends_on.is_empty());
}

#[test]
fn full_binding_parses() {
    let toml = r#"
[[runtime]]
id = "wsl"
kind = "herdr"

[[project]]
id = "p"
stages = ["Spec", "Build"]

[[project.workstream]]
id = "be"
binding = { runtime = "wsl", workspace = "ai-cockpit", pane_label = "backend", cwd = "worktrees/backend", agent = "claude" }
"#;
    let config = config::parse_toml(toml).expect("完整 binding 應該解析成功");

    let binding = config.projects[0].workstreams[0]
        .binding
        .clone()
        .expect("應該有 binding");
    assert_eq!(
        binding,
        BindingSpec {
            runtime: cockpit_core::RuntimeId::new("wsl"),
            workspace: "ai-cockpit".to_string(),
            pane_label: Some("backend".to_string()),
            cwd: Some("worktrees/backend".to_string()),
            agent: Some("claude".to_string()),
        }
    );
}

#[test]
fn no_project_section_yields_empty_projects() {
    let toml = r#"
[server]
listen = "127.0.0.1:7770"

[[runtime]]
id = "win"
kind = "herdr"
"#;
    let config = config::parse_toml(toml).expect("沒有 project 區段應該解析成功");
    assert!(config.projects.is_empty());
}

#[test]
fn unknown_field_in_task_fails_naming_field() {
    let toml = r#"
[[project]]
id = "p"
stages = ["Spec"]

[[project.workstream]]
id = "be"

[[project.task]]
id = "t1"
workstream = "be"
owner = "x"
"#;
    let err = config::parse_toml(toml).expect_err("task 多餘欄位應該失敗");
    assert!(err.to_string().contains("owner"), "訊息應含 owner：{err}");
}

// -- Project 區段驗證 -----------------------------------------------------------

#[test]
fn task_workstream_not_found_fails() {
    let toml = r#"
[[project]]
id = "p"
stages = ["Spec"]

[[project.task]]
id = "t1"
workstream = "fe"
"#;
    let err = config::parse_toml(toml).expect_err("task 指向不存在的 workstream 應該失敗");
    let message = err.to_string();
    assert!(
        message.contains("project.p.task.t1.workstream"),
        "訊息應含 project.p.task.t1.workstream：{message}"
    );
    assert!(message.contains("fe"), "訊息應含 fe：{message}");
}

#[test]
fn dependency_cycle_fails() {
    let toml = r#"
[[project]]
id = "p"
stages = ["Spec"]

[[project.workstream]]
id = "be"

[[project.task]]
id = "a"
workstream = "be"
depends_on = ["b"]

[[project.task]]
id = "b"
workstream = "be"
depends_on = ["a"]
"#;
    let err = config::parse_toml(toml).expect_err("依賴成環應該失敗");
    let message = err.to_string();
    assert!(
        message.contains("depends_on"),
        "訊息應含 depends_on：{message}"
    );
    assert!(message.contains('a'), "訊息應含 a：{message}");
    assert!(message.contains('b'), "訊息應含 b：{message}");
}

#[test]
fn binding_unconfigured_runtime_fails() {
    let toml = r#"
[[runtime]]
id = "win"
kind = "herdr"

[[project]]
id = "p"
stages = ["Spec"]

[[project.workstream]]
id = "be"
binding = { runtime = "wsl", workspace = "ai-cockpit" }
"#;
    let err = config::parse_toml(toml).expect_err("binding 指向未設定的 runtime 應該失敗");
    let message = err.to_string();
    assert!(
        message.contains("project.p.workstream.be.binding.runtime"),
        "訊息應含 project.p.workstream.be.binding.runtime：{message}"
    );
    assert!(message.contains("wsl"), "訊息應含 wsl：{message}");
}

// -- Fix round 1 finding 1：binding.runtime 只能是設定檔明確宣告的 `[[runtime]]`，
// 零設定自動補的 `local` 不算數 ------------------------------------------------

#[test]
fn binding_runtime_local_without_declared_runtime_fails() {
    let toml = r#"
[[project]]
id = "p"
stages = ["Spec"]

[[project.workstream]]
id = "be"
binding = { runtime = "local", workspace = "ai-cockpit" }
"#;
    let err = config::parse_toml(toml)
        .expect_err("沒有 [[runtime]] 時 binding.runtime = \"local\" 應該失敗");
    let message = err.to_string();
    assert!(
        message.contains("project.p.workstream.be.binding.runtime"),
        "訊息應含 project.p.workstream.be.binding.runtime：{message}"
    );
    assert!(message.contains("local"), "訊息應含 local：{message}");
}

#[test]
fn task_id_with_illegal_characters_fails() {
    let toml = r#"
[[project]]
id = "p"
stages = ["Spec"]

[[project.workstream]]
id = "be"

[[project.task]]
id = "a/b"
workstream = "be"
"#;
    let err = config::parse_toml(toml).expect_err("id 含非法字元應該失敗");
    assert!(err.to_string().contains("a/b"), "訊息應含 a/b：{err}");
}

#[test]
fn duplicate_stage_fails() {
    let toml = r#"
[[project]]
id = "p"
stages = ["Spec", "Spec"]
"#;
    let err = config::parse_toml(toml).expect_err("stage 重複應該失敗");
    let message = err.to_string();
    assert!(
        message.contains("project.p.stages"),
        "訊息應含 project.p.stages：{message}"
    );
    assert!(message.contains("Spec"), "訊息應含 Spec：{message}");
}

#[test]
fn empty_stages_fails() {
    let toml = r#"
[[project]]
id = "p"
stages = []
"#;
    let err = config::parse_toml(toml).expect_err("stages 為空應該失敗");
    assert!(err.to_string().contains("project.p.stages"));
}

#[test]
fn empty_string_stage_fails() {
    let toml = r#"
[[project]]
id = "p"
stages = ["Spec", ""]
"#;
    let err = config::parse_toml(toml).expect_err("stages 含空字串應該失敗");
    assert!(err.to_string().contains("project.p.stages"));
}

#[test]
fn duplicate_project_id_fails() {
    let toml = r#"
[[project]]
id = "p"
stages = ["Spec"]

[[project]]
id = "p"
stages = ["Spec"]
"#;
    let err = config::parse_toml(toml).expect_err("project id 重複應該失敗");
    assert!(err.to_string().contains("project.p"));
}

#[test]
fn duplicate_workstream_id_within_project_fails() {
    let toml = r#"
[[project]]
id = "p"
stages = ["Spec"]

[[project.workstream]]
id = "be"

[[project.workstream]]
id = "be"
"#;
    let err = config::parse_toml(toml).expect_err("同一 Project 內 workstream id 重複應該失敗");
    assert!(err.to_string().contains("project.p.workstream.be"));
}

#[test]
fn duplicate_task_id_within_project_fails() {
    let toml = r#"
[[project]]
id = "p"
stages = ["Spec"]

[[project.workstream]]
id = "be"

[[project.task]]
id = "t1"
workstream = "be"

[[project.task]]
id = "t1"
workstream = "be"
"#;
    let err = config::parse_toml(toml).expect_err("同一 Project 內 task id 重複應該失敗");
    assert!(err.to_string().contains("project.p.task.t1"));
}

#[test]
fn task_stage_not_in_stages_fails() {
    let toml = r#"
[[project]]
id = "p"
stages = ["Spec"]

[[project.workstream]]
id = "be"

[[project.task]]
id = "t1"
workstream = "be"
stage = "Build"
"#;
    let err = config::parse_toml(toml).expect_err("task stage 不在 stages 應該失敗");
    let message = err.to_string();
    assert!(message.contains("project.p.task.t1.stage"), "{message}");
    assert!(message.contains("Build"), "{message}");
}

#[test]
fn depends_on_self_fails() {
    let toml = r#"
[[project]]
id = "p"
stages = ["Spec"]

[[project.workstream]]
id = "be"

[[project.task]]
id = "a"
workstream = "be"
depends_on = ["a"]
"#;
    let err = config::parse_toml(toml).expect_err("依賴自己應該失敗");
    let message = err.to_string();
    assert!(message.contains("depends_on"), "{message}");
    assert!(message.contains("project.p.task.a"), "{message}");
}

#[test]
fn depends_on_nonexistent_task_fails() {
    let toml = r#"
[[project]]
id = "p"
stages = ["Spec"]

[[project.workstream]]
id = "be"

[[project.task]]
id = "a"
workstream = "be"
depends_on = ["missing"]
"#;
    let err = config::parse_toml(toml).expect_err("依賴不存在的 task 應該失敗");
    let message = err.to_string();
    assert!(message.contains("depends_on"), "{message}");
    assert!(message.contains("missing"), "{message}");
}

#[test]
fn binding_empty_workspace_fails() {
    let toml = r#"
[[runtime]]
id = "wsl"
kind = "herdr"

[[project]]
id = "p"
stages = ["Spec"]

[[project.workstream]]
id = "be"
binding = { runtime = "wsl", workspace = "" }
"#;
    let err = config::parse_toml(toml).expect_err("binding workspace 空字串應該失敗");
    assert!(
        err.to_string()
            .contains("project.p.workstream.be.binding.workspace")
    );
}

#[test]
fn binding_empty_optional_fields_fail() {
    for field in ["pane_label", "cwd", "agent"] {
        let toml = format!(
            r#"
[[runtime]]
id = "wsl"
kind = "herdr"

[[project]]
id = "p"
stages = ["Spec"]

[[project.workstream]]
id = "be"
binding = {{ runtime = "wsl", workspace = "ai-cockpit", {field} = "" }}
"#
        );
        let err = config::parse_toml(&toml).expect_err(&format!("binding {field} 空字串應該失敗"));
        let message = err.to_string();
        assert!(
            message.contains(&format!("binding.{field}")),
            "訊息應含 binding.{field}：{message}"
        );
    }
}

#[test]
fn project_id_with_illegal_characters_fails() {
    let toml = r#"
[[project]]
id = "a/b"
stages = ["Spec"]
"#;
    let err = config::parse_toml(toml).expect_err("project id 含非法字元應該失敗");
    assert!(err.to_string().contains("a/b"));
}

#[test]
fn workstream_id_with_illegal_characters_fails() {
    let toml = r#"
[[project]]
id = "p"
stages = ["Spec"]

[[project.workstream]]
id = "a/b"
"#;
    let err = config::parse_toml(toml).expect_err("workstream id 含非法字元應該失敗");
    assert!(err.to_string().contains("a/b"));
}

// -- Fix round 1 finding 2：id 缺漏（不是格式錯誤，是整個欄位沒給）要能被序號路徑
// 識別，不能被 `toml::from_str` 的一般 「missing field」擋下 ---------------------

#[test]
fn missing_project_id_fails_with_ordinal_path() {
    let toml = r#"
[[project]]
stages = ["Spec"]
"#;
    let err = config::parse_toml(toml).expect_err("project 缺漏 id 應該失敗");
    let message = err.to_string();
    assert!(
        message.contains("project[1]"),
        "訊息應含序號路徑 project[1]：{message}"
    );
}

#[test]
fn missing_workstream_id_fails_with_ordinal_path() {
    let toml = r#"
[[project]]
id = "p"
stages = ["Spec"]

[[project.workstream]]
name = "無 id"
"#;
    let err = config::parse_toml(toml).expect_err("workstream 缺漏 id 應該失敗");
    let message = err.to_string();
    assert!(
        message.contains("project.p.workstream[1]"),
        "訊息應含序號路徑 project.p.workstream[1]：{message}"
    );
}

#[test]
fn missing_task_id_fails_with_ordinal_path() {
    let toml = r#"
[[project]]
id = "p"
stages = ["Spec"]

[[project.workstream]]
id = "be"

[[project.task]]
workstream = "be"
"#;
    let err = config::parse_toml(toml).expect_err("task 缺漏 id 應該失敗");
    let message = err.to_string();
    assert!(
        message.contains("project.p.task[1]"),
        "訊息應含序號路徑 project.p.task[1]：{message}"
    );
}

// -- 狀態檔位置 -------------------------------------------------------------------

#[test]
fn state_path_defaults_to_config_dir() {
    let dir = TempDir::new("state-default");
    let file = dir.write(
        "cockpit.toml",
        r#"
[[project]]
id = "p"
stages = ["Spec"]
"#,
    );
    let args = Args { config: Some(file) };

    let config = config::load(&args, dir.path(), &no_env).expect("含 project 的設定應該載入成功");

    assert_eq!(
        config.state_path,
        Some(dir.path().join("cockpit.state.json"))
    );
}

#[test]
fn state_path_relative_resolves_against_config_dir() {
    let dir = TempDir::new("state-relative");
    let file = dir.write(
        "cockpit.toml",
        r#"
[state]
path = "state/progress.json"

[[project]]
id = "p"
stages = ["Spec"]
"#,
    );
    let args = Args { config: Some(file) };

    let config = config::load(&args, dir.path(), &no_env).expect("含 project 的設定應該載入成功");

    assert_eq!(
        config.state_path,
        Some(dir.path().join("state").join("progress.json"))
    );
}

#[test]
fn no_project_means_no_state_path() {
    let dir = TempDir::new("state-no-project");
    let file = dir.write(
        "cockpit.toml",
        r#"
[[runtime]]
id = "win"
kind = "herdr"
"#,
    );
    let args = Args { config: Some(file) };

    let config = config::load(&args, dir.path(), &no_env).expect("沒有 project 應該載入成功");

    assert_eq!(config.state_path, None, "沒有 project 就不該有狀態檔路徑");
}

#[test]
fn zero_config_has_no_state_path() {
    let dir = TempDir::new("state-zero-config");
    let args = Args { config: None };

    let config = config::load(&args, dir.path(), &no_env).expect("零設定應該載入成功");

    assert_eq!(config.state_path, None);
}

#[test]
fn empty_state_path_fails() {
    let toml = r#"
[state]
path = ""
"#;
    let err = config::parse_toml(toml).expect_err("state.path 空字串應該失敗");
    assert!(err.to_string().contains("state.path"));
}
