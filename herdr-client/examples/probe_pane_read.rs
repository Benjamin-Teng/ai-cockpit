//! Spike 工具（change 3 `live-output` 設計前探測）：對真機 HERDR 反覆呼叫 `pane.read`，量測
//! 延遲、回應大小、`revision` 行為與內容雜湊變化。
//!
//! repo 至今從未對真機呼叫過 `pane.read`（design D10：change 1 刻意不送這個 method，只在合約
//! 測試驗證序列化）。這支工具走真正的
//! `herdr_client::client::Client::request(PaneReadRequest(..))` 路徑，同時驗證
//! `PaneReadResult` 型別能否反序列化真機回應。**只提供量測，不做任何設計決策**——實驗由
//! 指揮官親自跑、親自判讀輸出。
//!
//! 用法:
//!
//! ```sh
//! # 先看目前有哪些 pane（唯讀，只呼叫 session.snapshot；不印 title／cwd）
//! cargo run -p herdr-client --example probe_pane_read -- --list
//!
//! # Windows 端：對某個 pane 連續讀 20 次、每次「開始」間隔 500ms，metadata-only
//! # （絕不印出／寫出 text 內容——這是隱私要求，Windows 端會拿這個模式讀使用者真實的 pane）
//! cargo run -p herdr-client --example probe_pane_read -- \
//!     --pane wJ:p1 --count 20 --interval-ms 500 --metadata-only
//!
//! # WSL 測試 server：對某個 pane 讀 5 次，把每次內容有變化的 text 存成檔案供事後比對
//! cargo run -p herdr-client --example probe_pane_read -- \
//!     --wsl Ubuntu-24.04 /home/<user>/.config/herdr/herdr.sock \
//!     --pane wJ:p1 --count 5 --dump-dir target/probe-dump
//! ```
//!
//! 環境變數：`HERDR_CLIENT_TEST_WIN_SOCKET`（Windows 端 HERDR API socket 路徑，省略則用
//! `default_socket_path_from_env()`；只在沒給 `--wsl` 時生效，跟 `capture_events.rs` 同名同
//! 語意）。
//!
//! 硬性限制：全程只送 `session.snapshot`／`pane.read`，兩者都是唯讀 method（design D10 的
//! observer 子集），不執行任何會改變 HERDR 狀態的指令。
//!
//! **`--metadata-only` 的隱私保證**：程式碼中只有三處讀取 `PaneReadResult.text`——算
//! `text_bytes`（`.len()`）、算 `text_lines`（`.lines().count()`）、算 `text_hash`
//! （`DefaultHasher`，非密碼學強度，只為偵測內容是否變化）——三者都是派生的中繼資料數字，不是
//! 原始內容本身，且一律印到 stdout（TSV 欄位）。唯一會把原始 `.text` 寫出去的路徑是
//! `--dump-dir` 那個 `if let Some(dir) = &args.dump_dir` 分支，跟 `--metadata-only` 在
//! `parse_args` 裡互斥（同時給两者直接報錯、非 0 結束，不會執行到任何探測邏輯）。

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use herdr_client::client::{Client, PaneReadRequest, SessionSnapshotRequest};
use herdr_client::connector::{ChildStdioConnector, Connector, default_socket_path_from_env};
use herdr_client::types::{AgentStatus, PaneReadParams, ReadFormat, ReadSource};

/// 連線端點：預設走 Windows 端 named pipe；`--wsl <distro> <socket>` 改用
/// `ChildStdioConnector` 透過 `wsl.exe -d <distro> -e nc -U <socket>` 橋接（同
/// `cockpit-herdr/src/factory.rs` 的組法，ADR-0002／1a spike 2 查證過的橋接指令，逐字如此）。
enum Endpoint {
    Default,
    Wsl { distro: String, socket: String },
}

struct Args {
    endpoint: Endpoint,
    list: bool,
    pane: Option<String>,
    source: ReadSource,
    format: Option<ReadFormat>,
    lines: Option<u32>,
    strip_ansi: Option<bool>,
    count: u32,
    interval_ms: u64,
    dump_dir: Option<PathBuf>,
}

fn print_usage() {
    eprintln!(
        "用法: probe_pane_read --list [端點旗標]\n       probe_pane_read --pane <id> [選項] [端點旗標]"
    );
    eprintln!();
    eprintln!("端點旗標（擇一，預設 Windows 端 named pipe）:");
    eprintln!("  --wsl <distro> <socket>   改用 wsl.exe -d <distro> -e nc -U <socket> 橋接");
    eprintln!();
    eprintln!("模式:");
    eprintln!("  --list                    呼叫 session.snapshot，列出每個 pane 後結束");
    eprintln!("                            （不印 title／cwd）");
    eprintln!();
    eprintln!("pane.read 參數（非 --list 時，--pane 必填）:");
    eprintln!("  --pane <id>               目標 pane id");
    eprintln!(
        "  --source <值>             visible|recent|recent_unwrapped|detection（預設 visible）"
    );
    eprintln!("  --format <值>             text|ansi（不給就不送這個欄位）");
    eprintln!("  --lines <N>               只讀最後 N 行（不給就不送這個欄位）");
    eprintln!("  --strip-ansi <true|false> 是否去除 ansi 序列（不給就不送這個欄位）");
    eprintln!();
    eprintln!("迭代控制:");
    eprintln!("  --count <N>               迭代次數（預設 10）");
    eprintln!("  --interval-ms <M>         每次請求「開始」之間的間隔（預設 1000）");
    eprintln!();
    eprintln!("輸出控制（互斥）:");
    eprintln!("  --metadata-only           絕不印出／寫出 text 內容，只留中繼資料");
    eprintln!("  --dump-dir <dir>          text 雜湊與上次不同時（含第一次），原樣寫進");
    eprintln!("                            <dir>/iter-<NNN>-rev-<revision>.txt");
    eprintln!();
    eprintln!("  -h, --help                印出本說明並離開，不連線");
    eprintln!();
    eprintln!("環境變數:");
    eprintln!(
        "  HERDR_CLIENT_TEST_WIN_SOCKET（Windows 端 HERDR API socket 路徑，只在沒給 --wsl 時\
         生效，預設 default_socket_path_from_env()）"
    );
}

/// 取下一個參數值；缺席就印錯誤＋用法、以非 0 結束（不悄悄落入預設值）。
fn take_value(args: &mut impl Iterator<Item = String>, flag: &str) -> String {
    args.next().unwrap_or_else(|| {
        eprintln!("{flag} 需要一個參數");
        print_usage();
        std::process::exit(2);
    })
}

fn parse_source(value: &str) -> ReadSource {
    match value {
        "visible" => ReadSource::Visible,
        "recent" => ReadSource::Recent,
        "recent_unwrapped" => ReadSource::RecentUnwrapped,
        "detection" => ReadSource::Detection,
        other => {
            eprintln!(
                "--source 的值不合法: {other}（合法值: visible|recent|recent_unwrapped|detection）"
            );
            print_usage();
            std::process::exit(2);
        }
    }
}

fn parse_format(value: &str) -> ReadFormat {
    match value {
        "text" => ReadFormat::Text,
        "ansi" => ReadFormat::Ansi,
        other => {
            eprintln!("--format 的值不合法: {other}（合法值: text|ansi）");
            print_usage();
            std::process::exit(2);
        }
    }
}

fn parse_strip_ansi(value: &str) -> bool {
    match value {
        "true" => true,
        "false" => false,
        other => {
            eprintln!("--strip-ansi 的值不合法: {other}（合法值: true|false）");
            print_usage();
            std::process::exit(2);
        }
    }
}

fn parse_u32(flag: &str, value: &str) -> u32 {
    value.parse().unwrap_or_else(|e| {
        eprintln!("{flag} 參數不是合法整數: {e}");
        print_usage();
        std::process::exit(2);
    })
}

fn parse_u64(flag: &str, value: &str) -> u64 {
    value.parse().unwrap_or_else(|e| {
        eprintln!("{flag} 參數不是合法整數: {e}");
        print_usage();
        std::process::exit(2);
    })
}

/// 在讀任何環境變數或連線之前完整解析 argv（同 `capture_events.rs`／`spike4_no_window.rs` 的
/// 教訓：未知參數一律視為錯誤，不悄悄落入預設值）。
fn parse_args() -> Args {
    let mut endpoint = Endpoint::Default;
    let mut list = false;
    let mut pane: Option<String> = None;
    let mut source = ReadSource::Visible;
    let mut format: Option<ReadFormat> = None;
    let mut lines: Option<u32> = None;
    let mut strip_ansi: Option<bool> = None;
    let mut count = 10u32;
    let mut interval_ms = 1000u64;
    let mut metadata_only = false;
    let mut dump_dir: Option<PathBuf> = None;

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => {
                print_usage();
                std::process::exit(0);
            }
            "--list" => list = true,
            "--wsl" => {
                let distro = take_value(&mut args, "--wsl");
                let socket = take_value(&mut args, "--wsl");
                endpoint = Endpoint::Wsl { distro, socket };
            }
            "--pane" => pane = Some(take_value(&mut args, "--pane")),
            "--source" => source = parse_source(&take_value(&mut args, "--source")),
            "--format" => format = Some(parse_format(&take_value(&mut args, "--format"))),
            "--lines" => lines = Some(parse_u32("--lines", &take_value(&mut args, "--lines"))),
            "--strip-ansi" => {
                strip_ansi = Some(parse_strip_ansi(&take_value(&mut args, "--strip-ansi")));
            }
            "--count" => count = parse_u32("--count", &take_value(&mut args, "--count")),
            "--interval-ms" => {
                interval_ms = parse_u64("--interval-ms", &take_value(&mut args, "--interval-ms"));
            }
            "--metadata-only" => metadata_only = true,
            "--dump-dir" => dump_dir = Some(PathBuf::from(take_value(&mut args, "--dump-dir"))),
            other => {
                eprintln!("unknown argument: {other}");
                print_usage();
                std::process::exit(2);
            }
        }
    }

    if !list && pane.is_none() {
        eprintln!("非 --list 模式必須指定 --pane <id>");
        print_usage();
        std::process::exit(2);
    }
    if metadata_only && dump_dir.is_some() {
        eprintln!("--metadata-only 與 --dump-dir 不能同時使用");
        print_usage();
        std::process::exit(2);
    }

    Args {
        endpoint,
        list,
        pane,
        source,
        format,
        lines,
        strip_ansi,
        count,
        interval_ms,
        dump_dir,
    }
}

fn read_source_str(source: ReadSource) -> &'static str {
    match source {
        ReadSource::Visible => "visible",
        ReadSource::Recent => "recent",
        ReadSource::RecentUnwrapped => "recent_unwrapped",
        ReadSource::Detection => "detection",
    }
}

fn read_format_str(format: ReadFormat) -> &'static str {
    match format {
        ReadFormat::Text => "text",
        ReadFormat::Ansi => "ansi",
    }
}

fn agent_status_str(status: AgentStatus) -> &'static str {
    match status {
        AgentStatus::Idle => "idle",
        AgentStatus::Working => "working",
        AgentStatus::Blocked => "blocked",
        AgentStatus::Done => "done",
        AgentStatus::Unknown => "unknown",
    }
}

/// `--list`：呼叫唯讀的 `session.snapshot`，每個 pane 印一行（pane id、workspace/tab id、
/// agent 名稱、agent 狀態），不印 title／cwd。印完結束。
async fn run_list(client: &Client) {
    match client.request(SessionSnapshotRequest {}).await {
        Ok(result) => {
            for pane in &result.snapshot.panes {
                let agent = pane.agent.as_deref().unwrap_or("-");
                println!(
                    "{}\t{}/{}\t{}\t{}",
                    pane.pane_id,
                    pane.workspace_id,
                    pane.tab_id,
                    agent,
                    agent_status_str(pane.agent_status)
                );
            }
        }
        Err(e) => {
            eprintln!("session.snapshot 失敗: {e}");
            std::process::exit(1);
        }
    }
}

fn median(sorted: &[f64]) -> f64 {
    let n = sorted.len();
    if n == 0 {
        return f64::NAN;
    }
    if n % 2 == 1 {
        sorted[n / 2]
    } else {
        (sorted[n / 2 - 1] + sorted[n / 2]) / 2.0
    }
}

fn percentile95(sorted: &[f64]) -> f64 {
    let n = sorted.len();
    if n == 0 {
        return f64::NAN;
    }
    let rank = (0.95 * (n - 1) as f64).round() as usize;
    sorted[rank.min(n - 1)]
}

/// 反覆呼叫 `pane.read`：每次迭代印一行 TSV、`--dump-dir` 給的話把有變化的 text 存檔、結束時
/// 在 stderr 印統計摘要。
async fn run_probe(client: &Client, args: &Args) {
    let pane_id = args
        .pane
        .clone()
        .expect("parse_args 已保證非 --list 模式一定有 --pane");

    if let Some(dir) = &args.dump_dir
        && let Err(e) = std::fs::create_dir_all(dir)
    {
        eprintln!("建立 dump 目錄 {} 失敗: {e}", dir.display());
        std::process::exit(1);
    }

    println!(
        "iter\telapsed_ms\tok\trevision\ttruncated\ttext_bytes\ttext_lines\ttext_hash\trev_changed\thash_changed\tresp_source\tresp_format"
    );

    let mut prev_revision: Option<u64> = None;
    let mut prev_hash: Option<u64> = None;
    let mut elapsed_list: Vec<f64> = Vec::new();
    let mut bytes_list: Vec<usize> = Vec::new();
    let mut ok_count = 0u32;
    let mut fail_count = 0u32;
    let mut rev_changed_hash_same = 0u32;
    let mut hash_changed_rev_same = 0u32;

    for iter in 1..=args.count {
        let call_start = Instant::now();
        let req = PaneReadRequest(PaneReadParams {
            pane_id: pane_id.clone(),
            source: args.source,
            format: args.format,
            lines: args.lines,
            strip_ansi: args.strip_ansi,
        });
        let response = client.request(req).await;
        let elapsed_ms = call_start.elapsed().as_secs_f64() * 1000.0;
        elapsed_list.push(elapsed_ms);

        match response {
            Ok(envelope) => {
                ok_count += 1;
                let read = envelope.read;

                // 隱私保證（見檔頭註解）：這三行是全檔唯二讀取 `.text` 的地方之一（另一處是
                // 下面 `--dump-dir` 那個 `std::fs::write`），三者都只產生中繼資料數字，不輸出
                // 原始內容。
                let mut hasher = DefaultHasher::new();
                read.text.hash(&mut hasher);
                let hash = hasher.finish();
                let text_bytes = read.text.len();
                let text_lines = read.text.lines().count();

                let (rev_changed, hash_changed) = match (prev_revision, prev_hash) {
                    (Some(prev_rev), Some(prev_hash_value)) => {
                        let rc = prev_rev != read.revision;
                        let hc = prev_hash_value != hash;
                        if rc && !hc {
                            rev_changed_hash_same += 1;
                        }
                        if hc && !rc {
                            hash_changed_rev_same += 1;
                        }
                        (rc.to_string(), hc.to_string())
                    }
                    _ => ("-".to_string(), "-".to_string()),
                };

                if let Some(dir) = &args.dump_dir {
                    // 含第一次：`prev_hash` 是 `None` 時 `None != Some(hash)` 為真。
                    if prev_hash != Some(hash) {
                        let path = dir.join(format!("iter-{iter:03}-rev-{}.txt", read.revision));
                        // 全檔唯二把原始 `.text` 寫出去的地方；只在這個分支發生，且與
                        // `--metadata-only` 在 `parse_args` 裡互斥。
                        if let Err(e) = std::fs::write(&path, &read.text) {
                            eprintln!("寫入 {} 失敗: {e}", path.display());
                        }
                    }
                }

                println!(
                    "{iter}\t{elapsed_ms:.1}\ttrue\t{}\t{}\t{text_bytes}\t{text_lines}\t{hash:016x}\t{rev_changed}\t{hash_changed}\t{}\t{}",
                    read.revision,
                    read.truncated,
                    read_source_str(read.source),
                    read_format_str(read.format),
                );

                bytes_list.push(text_bytes);
                prev_revision = Some(read.revision);
                prev_hash = Some(hash);
            }
            Err(e) => {
                fail_count += 1;
                eprintln!("iter {iter} 失敗: {e}");
                println!("{iter}\t{elapsed_ms:.1}\tfalse\t-\t-\t-\t-\t-\t-\t-\t-\t-");
            }
        }

        if iter < args.count {
            let interval = Duration::from_millis(args.interval_ms);
            let spent = call_start.elapsed();
            if spent < interval {
                tokio::time::sleep(interval - spent).await;
            }
        }
    }

    eprintln!();
    eprintln!(
        "成功 {ok_count} 次、失敗 {fail_count} 次（共 {} 次嘗試）",
        ok_count + fail_count
    );
    if elapsed_list.is_empty() {
        eprintln!("沒有任何嘗試，無法計算 elapsed_ms 統計");
    } else {
        elapsed_list.sort_by(|a, b| a.partial_cmp(b).expect("elapsed_ms 不會是 NaN"));
        eprintln!(
            "elapsed_ms: min={:.1} median={:.1} p95={:.1} max={:.1}",
            elapsed_list.first().copied().unwrap_or(f64::NAN),
            median(&elapsed_list),
            percentile95(&elapsed_list),
            elapsed_list.last().copied().unwrap_or(f64::NAN),
        );
    }
    if bytes_list.is_empty() {
        eprintln!("text_bytes: 沒有成功的回應，無法計算");
    } else {
        eprintln!(
            "text_bytes: min={} max={}",
            bytes_list.iter().min().expect("非空"),
            bytes_list.iter().max().expect("非空"),
        );
    }
    eprintln!("revision 變但 hash 沒變: {rev_changed_hash_same} 次");
    eprintln!("hash 變但 revision 沒變: {hash_changed_rev_same} 次");
}

#[cfg(not(windows))]
fn main() {
    eprintln!(
        "probe_pane_read 走 Windows 端 HERDR named pipe，或透過 wsl.exe 橋接（ADR-0002）—— \
         兩者都仰賴 wsl.exe／named pipe，本平台無法使用，略過。"
    );
}

#[cfg(windows)]
#[tokio::main]
async fn main() {
    use std::sync::Arc;

    use herdr_client::connector::NamedPipeConnector;

    fn win_socket_path() -> PathBuf {
        std::env::var("HERDR_CLIENT_TEST_WIN_SOCKET")
            .map(PathBuf::from)
            .unwrap_or_else(|_| default_socket_path_from_env())
    }

    let args = parse_args();

    let connector: Arc<dyn Connector> = match &args.endpoint {
        Endpoint::Default => Arc::new(NamedPipeConnector::new(win_socket_path())),
        Endpoint::Wsl { distro, socket } => Arc::new(ChildStdioConnector::new(
            "wsl.exe",
            ["-d", distro.as_str(), "-e", "nc", "-U", socket.as_str()],
        )),
    };
    let client = Client::new(connector);

    if args.list {
        run_list(&client).await;
        return;
    }

    run_probe(&client, &args).await;
}
