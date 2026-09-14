//! Transport 層驗收測試（tasks 2.1、2.2；spec `herdr-transport`）。全部走假對端，
//! 不需要真機 HERDR。

use std::sync::atomic::{AtomicU64, Ordering};

#[cfg(windows)]
use herdr_client::connector::NamedPipeConnector;
#[cfg(unix)]
use herdr_client::connector::UnixSocketConnector;
use herdr_client::connector::{ChildStdioConnector, ConnectError, Connector, NdjsonStream};

#[cfg(windows)]
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
#[cfg(windows)]
use tokio::net::windows::named_pipe::{ClientOptions, ServerOptions};

#[cfg(unix)]
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

/// 測試用假對端子程序的完整路徑，由 `[[bin]] name = "herdr-client-test-child"` 建置
/// （task 2.2 Open Questions：採 `[[bin]]` + `required-features` 做法，實測見
/// `herdr-client/README.md`）。
fn test_child_bin() -> &'static str {
    env!("CARGO_BIN_EXE_herdr-client-test-child")
}

/// `Box<dyn NdjsonStream>` 沒有實作 `Debug`，`Result::expect_err`／`unwrap_err` 需要
/// `T: Debug` 才能組 panic 訊息，所以 `connect()` 的結果不能直接用 `expect_err`；
/// 改用這個小 helper 手動配對。
fn expect_connect_err(
    result: Result<Box<dyn NdjsonStream>, ConnectError>,
    msg: &str,
) -> ConnectError {
    match result {
        Ok(_) => panic!("{msg}"),
        Err(e) => e,
    }
}

static UNIQUE_COUNTER: AtomicU64 = AtomicU64::new(0);

/// 產生本次測試 process 內唯一的字串後綴，避免不同測試撞名（含隨機成分的暫存路徑）。
fn unique_suffix() -> String {
    let n = UNIQUE_COUNTER.fetch_add(1, Ordering::SeqCst);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock 應晚於 UNIX_EPOCH")
        .as_nanos();
    format!("{}-{n}-{nanos}", std::process::id())
}

// ---------------------------------------------------------------------------
// 2.1 named pipe / unix socket
// ---------------------------------------------------------------------------

#[cfg(windows)]
fn unique_pipe_target(tag: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(format!(
        r"C:\herdr-client-test\{tag}-{}.sock",
        unique_suffix()
    ))
}

#[cfg(windows)]
fn to_pipe_name(path: &std::path::Path) -> String {
    format!(r"\\.\pipe\{}", path.display())
}

#[cfg(unix)]
fn unique_unix_socket_path(tag: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("herdr-client-test-{tag}-{}.sock", unique_suffix()))
}

#[cfg(windows)]
#[tokio::test]
async fn connect_missing_target_is_server_not_running() {
    let path = unique_pipe_target("missing");
    let connector = NamedPipeConnector::new(&path);
    let err = expect_connect_err(connector.connect().await, "不存在的 pipe 路徑應該連線失敗");
    assert!(
        matches!(err, ConnectError::ServerNotRunning { .. }),
        "應歸類為 ServerNotRunning，實際: {err:?}"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn connect_missing_target_is_server_not_running() {
    let path = unique_unix_socket_path("missing");
    let connector = UnixSocketConnector::new(&path);
    let err = expect_connect_err(
        connector.connect().await,
        "不存在的 socket 路徑應該連線失敗",
    );
    assert!(
        matches!(err, ConnectError::ServerNotRunning { .. }),
        "應歸類為 ServerNotRunning，實際: {err:?}"
    );
}

#[tokio::test]
async fn describe_mentions_kind_and_target() {
    let child = ChildStdioConnector::new("some-cmd", ["a", "b"]);
    let desc = child.describe();
    assert!(desc.contains("child"), "描述應含 transport 種類: {desc}");
    assert!(desc.contains("some-cmd"), "描述應含指令列: {desc}");
    assert!(desc.contains("a"), "描述應含引數: {desc}");
    assert!(desc.contains("b"), "描述應含引數: {desc}");

    #[cfg(windows)]
    {
        let pipe = NamedPipeConnector::new(r"C:\fake\herdr.sock");
        let desc = pipe.describe();
        assert!(
            desc.contains("named-pipe"),
            "描述應含 transport 種類: {desc}"
        );
        assert!(desc.contains(r"C:\fake\herdr.sock"), "描述應含路徑: {desc}");
    }

    #[cfg(unix)]
    {
        let sock = UnixSocketConnector::new("/fake/herdr.sock");
        let desc = sock.describe();
        assert!(
            desc.contains("unix-socket"),
            "描述應含 transport 種類: {desc}"
        );
        assert!(desc.contains("/fake/herdr.sock"), "描述應含路徑: {desc}");
    }
}

/// 開一個可依序接受 `connections` 條連線的假 named pipe echo server。第一個 instance
/// 在函式回傳前就同步建立好，呼叫端拿到 `JoinHandle` 後可以立刻 `connect()`；若把建立
/// 也放進 `tokio::spawn` 的 async block，client 有機會在 task 真的執行到 `create()` 前
/// 就搶著開連線，開出 `ServerNotRunning`（本檔一開始就踩到這個競態，用這個函式修正）。
#[cfg(windows)]
fn spawn_named_pipe_multi_echo_server(
    pipe_name: String,
    connections: usize,
) -> tokio::task::JoinHandle<()> {
    let mut pending = Some(
        ServerOptions::new()
            .create(&pipe_name)
            .expect("建立假 named pipe instance 失敗"),
    );
    tokio::spawn(async move {
        for _ in 0..connections {
            let server = match pending.take() {
                Some(server) => server,
                None => ServerOptions::new()
                    .create(&pipe_name)
                    .expect("建立下一個假 named pipe instance 失敗"),
            };
            server.connect().await.expect("等待 client 連線失敗");
            let (reader, mut writer) = tokio::io::split(server);
            let mut reader = BufReader::new(reader);
            let mut line = String::new();
            reader.read_line(&mut line).await.expect("讀取一行失敗");
            writer
                .write_all(line.as_bytes())
                .await
                .expect("回應一行失敗");
            writer.flush().await.expect("flush 失敗");
        }
    })
}

/// `UnixListener::bind` 是同步呼叫，在 `tokio::spawn` 之前就完成，所以呼叫端拿到
/// `JoinHandle` 時 socket 已經存在，沒有 windows 版那個「建立與接受都在背景 task 裡」
/// 的競態。
#[cfg(unix)]
fn spawn_unix_multi_echo_server(
    path: std::path::PathBuf,
    connections: usize,
) -> tokio::task::JoinHandle<()> {
    let listener = tokio::net::UnixListener::bind(&path).expect("bind unix socket 失敗");
    tokio::spawn(async move {
        for _ in 0..connections {
            let (stream, _) = listener.accept().await.expect("accept 失敗");
            let (reader, mut writer) = tokio::io::split(stream);
            let mut reader = BufReader::new(reader);
            let mut line = String::new();
            reader.read_line(&mut line).await.expect("讀取一行失敗");
            writer
                .write_all(line.as_bytes())
                .await
                .expect("回應一行失敗");
            writer.flush().await.expect("flush 失敗");
        }
        let _ = std::fs::remove_file(&path);
    })
}

#[cfg(windows)]
#[tokio::test]
async fn named_pipe_roundtrip_with_colon_path() {
    let path = unique_pipe_target("roundtrip");
    let pipe_name = to_pipe_name(&path);
    let server = ServerOptions::new()
        .first_pipe_instance(true)
        .create(&pipe_name)
        .expect("建立假 named pipe server 失敗");
    let server_task = tokio::spawn(async move {
        server.connect().await.expect("等待連線失敗");
        let (reader, mut writer) = tokio::io::split(server);
        let mut reader = BufReader::new(reader);
        let mut line = String::new();
        reader.read_line(&mut line).await.expect("讀取一行失敗");
        writer
            .write_all(line.as_bytes())
            .await
            .expect("回應一行失敗");
        writer.flush().await.expect("flush 失敗");
    });

    let connector = NamedPipeConnector::new(&path);
    let mut stream = connector.connect().await.expect("連線失敗");
    stream.send_line("ping").await.expect("送出失敗");
    let line = stream
        .recv_line()
        .await
        .expect("讀取失敗")
        .expect("不應為 EOF");
    assert_eq!(line, "ping");

    server_task.await.expect("假 server task panic");
}

#[cfg(unix)]
#[tokio::test]
async fn unix_socket_roundtrip() {
    let path = unique_unix_socket_path("roundtrip");
    let listener = tokio::net::UnixListener::bind(&path).expect("bind unix socket 失敗");
    let server_path = path.clone();
    let server_task = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.expect("accept 失敗");
        let (reader, mut writer) = tokio::io::split(stream);
        let mut reader = BufReader::new(reader);
        let mut line = String::new();
        reader.read_line(&mut line).await.expect("讀取一行失敗");
        writer
            .write_all(line.as_bytes())
            .await
            .expect("回應一行失敗");
        writer.flush().await.expect("flush 失敗");
        let _ = std::fs::remove_file(&server_path);
    });

    let connector = UnixSocketConnector::new(&path);
    let mut stream = connector.connect().await.expect("連線失敗");
    stream.send_line("ping").await.expect("送出失敗");
    let line = stream
        .recv_line()
        .await
        .expect("讀取失敗")
        .expect("不應為 EOF");
    assert_eq!(line, "ping");

    server_task.await.expect("假 server task panic");
}

#[cfg(windows)]
#[tokio::test]
async fn two_connections_are_independent() {
    let path = unique_pipe_target("indep");
    let pipe_name = to_pipe_name(&path);
    let _server = spawn_named_pipe_multi_echo_server(pipe_name, 2);

    let connector = NamedPipeConnector::new(&path);

    let mut c1 = connector.connect().await.expect("連線 1 失敗");
    c1.send_line("one").await.expect("連線 1 送出失敗");
    assert_eq!(
        c1.recv_line().await.expect("連線 1 讀取失敗"),
        Some("one".to_string())
    );
    drop(c1);

    let mut c2 = connector.connect().await.expect("連線 2 失敗");
    c2.send_line("two").await.expect("連線 2 送出失敗");
    assert_eq!(
        c2.recv_line().await.expect("連線 2 讀取失敗"),
        Some("two".to_string())
    );
}

#[cfg(unix)]
#[tokio::test]
async fn two_connections_are_independent() {
    let path = unique_unix_socket_path("indep");
    let _server = spawn_unix_multi_echo_server(path.clone(), 2);

    let connector = UnixSocketConnector::new(&path);

    let mut c1 = connector.connect().await.expect("連線 1 失敗");
    c1.send_line("one").await.expect("連線 1 送出失敗");
    assert_eq!(
        c1.recv_line().await.expect("連線 1 讀取失敗"),
        Some("one".to_string())
    );
    drop(c1);

    let mut c2 = connector.connect().await.expect("連線 2 失敗");
    c2.send_line("two").await.expect("連線 2 送出失敗");
    assert_eq!(
        c2.recv_line().await.expect("連線 2 讀取失敗"),
        Some("two".to_string())
    );
}

/// Codex fix round 1 finding 3、design D9：`max_instances(1)` 讓這個 pipe 名稱同時只能
/// 存在一個 instance；先用一個「佔住者」client 把唯一的 instance 佔滿（Connected 狀態），
/// 這時真正要測的 `NamedPipeConnector::connect()` 應該先拿到 `ERROR_PIPE_BUSY`（231），
/// 依 D9 每 50ms 重試；佔住者釋出、server 重新 `disconnect()`＋`connect()` 接受下一個
/// client 後，重試應該在 1 秒 budget 內成功連上。
#[cfg(windows)]
#[tokio::test]
async fn named_pipe_busy_instance_retries_then_succeeds() {
    let path = unique_pipe_target("busy-retry");
    let pipe_name = to_pipe_name(&path);

    let server = ServerOptions::new()
        .max_instances(1)
        .create(&pipe_name)
        .expect("建立假 named pipe server 失敗（max_instances(1)）");

    // 佔住者：對同一個 pipe 名稱開一條連線，讓唯一的 instance 進入 Connected 狀態。
    let occupier = ClientOptions::new()
        .open(&pipe_name)
        .expect("佔住者連線應該立即成功（instance 剛建立、還沒被佔用）");
    server.connect().await.expect("等待佔住者連線失敗");

    // 這時 instance 已經被佔住者佔滿；真正要測的 connector 的 open() 應該先拿 231，
    // 依 D9 進入重試迴圈。
    let connector = NamedPipeConnector::new(&path);
    let connect_fut = connector.connect();

    // 給重試邏輯足夠時間至少撞上一次 231（重試間隔 50ms），再讓佔住者放手、
    // server 重新開始接受下一個 client。
    let release_and_reaccept = async {
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
        drop(occupier);
        server.disconnect().expect("disconnect 失敗");
        server.connect().await.expect("重新接受連線失敗")
    };

    // fix round 2 finding A：`tokio::join!` 同時等兩邊——若 `RETRY_BUDGET` 回歸成提早
    // 放棄，`connect_fut` 會馬上回錯，而 `release_and_reaccept` 仍會卡在
    // `server.connect().await` 等一個永遠不會再來的 client，讓整個 join 永久掛住。
    // 用遠大於 happy path（重試 150ms 後成功）的逾時包住整條競態，讓這種回歸表現成
    // 明確的 assertion 失敗，而不是把測試行程卡死（驗收見 fix round 2 fix report）。
    let joined = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        tokio::join!(release_and_reaccept, connect_fut)
    })
    .await
    .expect("5 秒內沒有完成：可能是重試邏輯提早放棄，讓 server 卡在等下一個永遠不會來的 client");

    let (_, connect_result) = joined;
    assert!(
        connect_result.is_ok(),
        "instance 釋出後應該在重試 budget 內連線成功，實際: {:?}",
        connect_result.err()
    );
}

/// Codex fix round 1 finding 3、design D9：佔住者全程不放手（超過 1 秒重試上限），
/// `connect()` 應該在有限時間內放棄並回 `ConnectError::Io`，不是無限等待；同時驗證
/// 真的花了接近 1 秒（而不是一遇到 231 就馬上放棄）。
#[cfg(windows)]
#[tokio::test]
async fn named_pipe_busy_timeout_is_io() {
    let path = unique_pipe_target("busy-timeout");
    let pipe_name = to_pipe_name(&path);

    let server = ServerOptions::new()
        .max_instances(1)
        .create(&pipe_name)
        .expect("建立假 named pipe server 失敗（max_instances(1)）");
    let occupier = ClientOptions::new()
        .open(&pipe_name)
        .expect("佔住者連線應該立即成功");
    server.connect().await.expect("等待佔住者連線失敗");

    let connector = NamedPipeConnector::new(&path);
    let start = std::time::Instant::now();
    // fix round 2 finding A：同樣包一層逾時保險——若重試邏輯回歸成「一直重試不放棄」，
    // 這裡不該無限期卡住，5 秒遠大於正常的 1 秒 retry budget。
    let result = tokio::time::timeout(std::time::Duration::from_secs(5), connector.connect())
        .await
        .expect("5 秒內應該要放棄重試（正常應該在 1 秒 budget 內就回錯）");
    let elapsed = start.elapsed();

    // 佔住者撐到這裡才放手，確保整個重試視窗內 instance 一直忙碌。
    drop(occupier);
    drop(server);

    match result {
        Ok(_) => panic!("超過重試上限應該連線失敗，實際成功了"),
        Err(err) => assert!(
            matches!(err, ConnectError::Io(_)),
            "超過重試上限應歸類為 Io，實際: {err:?}"
        ),
    }
    assert!(
        elapsed >= std::time::Duration::from_millis(900),
        "應該重試到接近 1 秒上限才放棄，實際: {elapsed:?}"
    );
    assert!(
        elapsed < std::time::Duration::from_secs(3),
        "不應該遠超過 1 秒上限，實際: {elapsed:?}"
    );
}

// ---------------------------------------------------------------------------
// 2.2 child stdio
// ---------------------------------------------------------------------------

#[tokio::test]
async fn child_roundtrip() {
    let connector = ChildStdioConnector::new(test_child_bin(), ["echo"]);
    let mut stream = connector.connect().await.expect("connect 應成功");
    stream.send_line("hello").await.expect("送出失敗");
    let line = stream
        .recv_line()
        .await
        .expect("讀取失敗")
        .expect("不應為 EOF");
    let parsed: serde_json::Value = serde_json::from_str(&line).expect("回應應為合法 JSON");
    assert_eq!(
        parsed.get("echo").and_then(|v| v.as_str()),
        Some("hello"),
        "回應應為 {{\"echo\":\"hello\"}}，實際: {line}"
    );
}

#[tokio::test]
async fn child_stderr_not_in_stream() {
    let connector = ChildStdioConnector::new(test_child_bin(), ["echo"]);
    let mut stream = connector.connect().await.expect("connect 應成功");
    stream.send_line("hello").await.expect("送出失敗");
    let line = stream
        .recv_line()
        .await
        .expect("讀取失敗")
        .expect("不應為 EOF");
    assert!(
        !line.contains("noise"),
        "子程序 stderr 的內容不應出現在讀到的資料流中: {line}"
    );
}

/// Codex fix round 1 finding 1：spec 的 Scenario「橋接子程序立即結束」是先送一行、
/// 再讀一行；子程序已經結束時，`send_line` 會先撞上 `BrokenPipe`（子程序在我們寫入前
/// 就已關閉 stdin 讀取端），必須跟 `recv_line` 的 EOF 分類成同一種 `ConnectionRefused`
/// 類錯誤，訊息帶 stderr。用 `wait_for_exit`（`test-support` feature 下的 `Child::wait`
/// 實際 poll／reap，見 fix round 2 finding B）確保子程序「確定」已經結束才送 `ping`，
/// 排除「送出時子程序其實還沒死」的僥倖情況，讓這個情境可重現、不是碰運氣——不像外部
/// `kill -0`／`tasklist` 輪詢，這裡不會把 Unix 上「已退出但還沒被回收」的 zombie 誤判成
/// 還活著。
#[tokio::test]
async fn child_exits_immediately_is_server_not_running() {
    let connector = ChildStdioConnector::new(
        test_child_bin(),
        ["exit-immediately", "1", "boom: target not found"],
    );
    let mut stream = connector
        .spawn_stream()
        .await
        .expect("connect 只 spawn，不應該失敗");

    stream
        .wait_for_exit(std::time::Duration::from_secs(3))
        .await
        .expect("子程序應該在 3 秒內結束（exit-immediately）");

    let err = match stream.send_line("ping").await {
        Ok(()) => stream
            .recv_line()
            .await
            .expect_err("子程序立即結束時應在 send 或 recv 其中一步回錯誤"),
        Err(e) => e,
    };
    assert_eq!(
        err.kind(),
        std::io::ErrorKind::ConnectionRefused,
        "應歸類為 ConnectionRefused，實際: {err:?}"
    );
    assert!(
        err.to_string().contains("boom: target not found"),
        "錯誤訊息應含子程序 stderr 內容，實際: {err}"
    );
}

/// Codex fix round 1 finding 2：子程序先寫 stderr、睡一小段再結束——stdout EOF／process
/// exit 跟「stderr 背景 task 讀到那一行」之間有明顯時間差，驗證這種排程順序下訊息一樣完整。
#[tokio::test]
async fn child_exit_after_sleep_stderr_still_captured() {
    let connector = ChildStdioConnector::new(
        test_child_bin(),
        ["exit-after-sleep", "1", "boom: delayed exit", "50"],
    );
    let mut stream = connector
        .connect()
        .await
        .expect("connect 只 spawn，不應該失敗");
    let err = match stream.send_line("ping").await {
        Ok(()) => stream
            .recv_line()
            .await
            .expect_err("子程序結束時應在 send 或 recv 其中一步回錯誤"),
        Err(e) => e,
    };
    assert_eq!(err.kind(), std::io::ErrorKind::ConnectionRefused);
    assert!(
        err.to_string().contains("boom: delayed exit"),
        "錯誤訊息應含子程序 stderr 內容，實際: {err}"
    );
}

/// Codex fix round 1 finding 2：`exit-immediately` 讓 stdout EOF／process exit／stderr
/// 資料幾乎同時出現，是背景 stderr task 還沒被排到就被主線讀 tail 的最壞情境；連跑 20 次
/// 統計驗證訊息不為空，而不是靠單次通過矇混過去。
#[tokio::test]
async fn child_exit_immediately_stderr_captured_repeatedly() {
    for i in 0..20 {
        let connector = ChildStdioConnector::new(
            test_child_bin(),
            ["exit-immediately", "1", "boom: repeat check"],
        );
        let mut stream = connector
            .connect()
            .await
            .unwrap_or_else(|e| panic!("第 {i} 次 connect 失敗: {e:?}"));
        let err = match stream.send_line("ping").await {
            Ok(()) => match stream.recv_line().await {
                Ok(Some(line)) => panic!("第 {i} 次不應該讀到任何一行: {line}"),
                Ok(None) => panic!("第 {i} 次不應該是乾淨 EOF"),
                Err(e) => e,
            },
            Err(e) => e,
        };
        assert_eq!(
            err.kind(),
            std::io::ErrorKind::ConnectionRefused,
            "第 {i} 次應歸類為 ConnectionRefused，實際: {err:?}"
        );
        let message = err.to_string();
        assert!(
            message.contains("boom: repeat check"),
            "第 {i} 次錯誤訊息應含 stderr 內容，實際: {message}"
        );
    }
}

#[tokio::test]
async fn child_missing_command_is_spawn() {
    let connector = ChildStdioConnector::new(
        "herdr-client-definitely-not-a-real-binary",
        Vec::<String>::new(),
    );
    let err = expect_connect_err(connector.connect().await, "指令不存在時應該 spawn 失敗");
    assert!(
        matches!(err, ConnectError::Spawn { .. }),
        "應歸類為 Spawn，實際: {err:?}"
    );
}

#[tokio::test]
async fn child_killed_on_drop() {
    let connector = ChildStdioConnector::new(test_child_bin(), ["sleep", "30"]);
    let stream = connector.spawn_stream().await.expect("connect 應成功");
    let pid = stream.pid().expect("剛 spawn 的子程序應有 pid");

    drop(stream);

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    loop {
        if !process_exists(pid) {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "child pid {pid} 在 drop 後 3 秒內應已結束（kill_on_drop）"
        );
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
}

/// 查作業系統確認 pid 是否仍存在：Windows 用 `tasklist /FI "PID eq <pid>"`，
/// unix 用 `kill -0 <pid>`（task 2.2 驗收測試 `child_killed_on_drop` 指定做法）。
fn process_exists(pid: u32) -> bool {
    #[cfg(windows)]
    {
        let output = std::process::Command::new("tasklist")
            .args(["/FI", &format!("PID eq {pid}"), "/NH"])
            .output()
            .expect("執行 tasklist 失敗");
        let stdout = String::from_utf8_lossy(&output.stdout);
        stdout.contains(&pid.to_string())
    }
    #[cfg(unix)]
    {
        let status = std::process::Command::new("kill")
            .args(["-0", &pid.to_string()])
            .status()
            .expect("執行 kill -0 失敗");
        status.success()
    }
}
