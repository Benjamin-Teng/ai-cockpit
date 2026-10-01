//! `cockpit` example：用固定 fixture 起一個真的 dashboard server，之後每個推送間隔（預設 2 秒）輪替一個
//! pane 的 `agent_status`，供瀏覽器工具與人眼檢查（design D14；Task 3.5）。不需要 HERDR，
//! 也不需要真正的 runtime。
//!
//! ```text
//! cargo run -p cockpit --example ui_preview
//! ```
//!
//! 預設監聽 `127.0.0.1:7770`；可用環境變數 `COCKPIT_PREVIEW_LISTEN` 覆寫（例如
//! `COCKPIT_PREVIEW_LISTEN=127.0.0.1:8080`）。Ctrl-C 結束。
//!
//! 推送間隔預設 2 秒；`COCKPIT_PREVIEW_PUSH_MS=100` 切成每 100 ms 推送一份新 version 的模式
//! （spec `cockpit-dashboard`「頻繁重畫時按鈕仍有效」；task 5.3）。
//!
//! 寫入端點（`POST /api/projects/{project}/tasks/{task}/{op}`、`PUT`／`DELETE
//! /api/projects/{project}/workstreams/{workstream}/override`）在這裡**只記錄請求並回 204**，
//! 不改投影：每筆請求在 stdout 印一行 `write-request <METHOD> <PATH> <BODY>`，供瀏覽器驗收
//! 腳本（`docs/research/2026-09-16/actions-check.js`）比對畫面送出了什麼（task 5.3）。
//!
//! 情境補充（ui-fixes task 4.1）：project `cockpit` 末尾的 workstream `ovr` 以覆蓋綁到斷線的 `wsl`
//! （`runtime_disconnected`＋`source: override`），供驗收「斷線期間可取消改綁」。
//!
//! 要模擬慢回應或被拒絕時，設 `COCKPIT_PREVIEW_WRITE_RULES`：以 `;` 分隔的
//! `<PATH>=<延遲毫秒>:<狀態碼>`，例如
//! `COCKPIT_PREVIEW_WRITE_RULES=/api/projects/cockpit/tasks/be-1/fail=1500:409`。符合路徑的
//! 請求照樣先記錄，再等指定延遲、回指定狀態碼（非 2xx 附 `{"error": ...}` 本體）；其他路徑
//! 仍立即回 204（task 5.3 fix round 1）。
//!
//! ## 輸出讀取端點的假 `AgentRuntime`（live-output task 5.1）
//!
//! 掛的是與正式服務**相同**的路由與來源檢查（`cockpit::http::router` 的
//! `GET /api/runtimes/{runtime}/panes/{pane}/output`；design D9 第三點）——這裡不另外寫一份
//! handler。假 runtime 的 id 固定是 `win`（跟投影裡的 runtime 一致），能服務的 pane 來自
//! fixture（`wJ:p1`–`wJ:p5`）。每次請求進到假 runtime 的 `read_output` 時，stdout
//! 印一行 `output-request <runtime> <pane>`（被來源檢查擋下的 403 請求不會進到這裡，自然不
//! 印）。
//!
//! 每個 pane 的行為（模式）預設：
//!
//! - `wJ:p1`：`ticker`——內容每秒多一行（`line 1`、`line 2`、……），立即回應。
//! - `wJ:p3`：`long`——一開始就有 300+ 行、`truncated=true`，內容仍每秒多一行。
//! - `wJ:p2`：`notfound`——一律 404（這個 pane 在 fixture 裡 `exited=true`，本來就不能從
//!   pane 列點選；要測「pane 已不存在」的 404 情境，直接對它的路徑發請求，或用
//!   `COCKPIT_PREVIEW_OUTPUT_MODES` 把 `notfound` 疊到一個可點選的 pane 上）。
//! - `wJ:p4`、`wJ:p5`（file-review 的假 repo pane，見 [`add_review_fixture_panes`]）：`ticker`
//!   （file-review task 4.2：選定後輸出端點不能 404，否則 Live Output 判「pane 已不存在」、清掉選取）。
//!
//! 用 `COCKPIT_PREVIEW_OUTPUT_MODES` 覆寫或新增：`;` 分隔的 `<pane>=<模式>` 規則（同
//! `COCKPIT_PREVIEW_WRITE_RULES` 的分隔慣例），例如
//! `COCKPIT_PREVIEW_OUTPUT_MODES=wJ:p1=delay:3000;wJ:p3=fail:2`。可用的模式：
//!
//! - `ticker`：同預設，內容每秒多一行、立即回應。
//! - `long`：同預設，一開始 300+ 行、`truncated=true`，之後仍每秒多一行。
//! - `delay:<毫秒>`：內容同 `ticker`（每秒多一行），但等待 `<毫秒>` 之後才回應——供「舊回應
//!   不蓋掉新選取」「請求不堆積」。
//! - `fail:<次數>`：前 `<次數>` 次請求回 503（`RuntimeError::Unavailable`），之後恢復成
//!   `ticker`——供「runtime 斷線後恢復」。
//! - `notfound`：一律 404（`RuntimeError::PaneNotFound`）。
//! - `html`：固定回一段含 `<script>window.pwned=1</script>` 與 `<b>x</b>` 的文字，內容不隨
//!   時間變化——供「內容不被當成 HTML」。
//!
//! 「pane 被關掉」情境（spec「失敗與消失的呈現」）：設
//! `COCKPIT_PREVIEW_VANISH_PANE=<pane>=<毫秒>`，該 pane 會在推送迴圈經過那麼久之後，從之後
//! 每一份推送的投影中被拿掉（例如 `COCKPIT_PREVIEW_VANISH_PANE=wJ:p1=5000`：5 秒後
//! `wJ:p1` 消失）。只支援一個 pane。
//!
//! 「同一個 pane 的 cwd 改到另一個 repo」情境（file-review 最終修正波 F1）：設
//! `COCKPIT_PREVIEW_CWD_TO_OTHER_REPO=<pane>=<毫秒>`，該 pane 的 `cwd` 會在推送迴圈經過那麼久之後
//! 改成 `other-repo` 的路徑（格式同 `COCKPIT_PREVIEW_VANISH_PANE`；只支援一個 pane）。推送迴圈只在
//! 每個推送間隔檢查一次，搭配較短的 `COCKPIT_PREVIEW_PUSH_MS` 使用。

use std::collections::{HashMap, HashSet};
use std::env;
use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicU16, AtomicU32, Ordering};
use std::time::{Duration, SystemTime};

use anyhow::Context;
use async_trait::async_trait;
use axum::Router;
use axum::body::Bytes;
use axum::extract::State;
use axum::http::{Method, StatusCode, Uri, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{post, put};
use cockpit::http::{AppState, router};
use cockpit_core::{
    AgentRuntime, AgentStatus, BindingSource, Mark, OutputFormat, PaneId, PaneOutput,
    ProjectedBinding, ProjectedPane, ProjectedState, ProjectedTab, ProjectedTask,
    ProjectedWorkstream, RuntimeError, RuntimeEvents, RuntimeId, RuntimeSnapshot, StageStatus,
    TabId, TaskId, WorkstreamId,
};
use tokio::net::TcpListener;
use tokio::sync::watch;
use tokio::time::{Instant, interval_at};

/// Task 3.5 手工改過的 fixture（`cockpit/tests/fixtures/projected-state.json`）：兩個
/// runtime（`win` connected、`wsl` disconnected 附原因），`win` 底下一個 workspace、一個
/// tab、三個 pane。跟 `cockpit/tests/fixture.rs` 的 `fixture_projected_state_deserializes`
/// 共用同一份檔案。
const FIXTURE: &str = include_str!("../tests/fixtures/projected-state.json");

const DEFAULT_LISTEN: &str = "127.0.0.1:7770";

const DEFAULT_PUSH_MS: u64 = 2000;

/// 假 `AgentRuntime` 的 id；必須跟 fixture 裡的 runtime id 一致（task 5.1 brief）。
const OUTPUT_RUNTIME_ID: &str = "win";

/// `html` 模式的固定內容（task 5.1 brief 逐字：含 `<script>` 與 `<b>` 字樣）。
const HTML_PROBE_TEXT: &str = "before\n<script>window.pwned=1</script>\n<b>x</b>\nafter";

/// file-review task 3.4（design D12）：`review-repo` fixture 的來源目錄。用
/// `CARGO_MANIFEST_DIR` 錨定成編譯期絕對路徑，不受 `cargo run`／`cargo test` 實際執行時的
/// 工作目錄影響；內容在版控中（`.git/`／`.gitignore`／`target/`／`.env` 除外——那幾項只在
/// [`setup_review_fixture`] 複製到暫存目錄之後才建立，理由見 design D12）。
const REVIEW_REPO_SOURCE: &str =
    concat!(env!("CARGO_MANIFEST_DIR"), "/examples/fixtures/review-repo");

/// [`setup_review_fixture`] 建立的暫存目錄一律以這個字串開頭，供
/// [`cleanup_stale_review_fixtures`] 認得哪些殘留目錄是自己的、可以安全刪除。
const REVIEW_FIXTURE_PREFIX: &str = "cockpit-ui-preview-";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let mut initial: ProjectedState =
        serde_json::from_str(FIXTURE).expect("fixture 應該能反序列化成 ProjectedState");

    // file-review task 3.4 fix round 2（Codex 對 3.1–3.4 合併審查 finding）：先把埠綁好，才去碰
    // fixture 的暫存目錄。原本的順序是先 `setup_review_fixture`（含
    // `cleanup_stale_review_fixtures`）才 `bind`：如果誤啟動第二個同埠實例，它會先刪掉第一個
    // 實例的 `review-repo`／`other-repo`（見 [`cleanup_stale_review_fixtures`] 的存活檢查也可能
    // 判斷失誤），再因為埠已被占用而退出——這時第一個實例的檔案請求全部開始失敗，驗收期間對副本
    // 做的任何改寫也一起沒了。`bind` 失敗就直接 `?` 往外拋，這個行程完全不會建立任何暫存目錄，
    // 也不會跑到會刪東西的清理邏輯。
    let listen_addr =
        env::var("COCKPIT_PREVIEW_LISTEN").unwrap_or_else(|_| DEFAULT_LISTEN.to_string());
    let listener = TcpListener::bind(&listen_addr).await?;
    let actual_addr = listener.local_addr()?;

    // file-review task 3.4（design D12）：把假 repo 複製到暫存目錄，並在假投影中掛上兩個指向
    // 副本的 pane，供之後的瀏覽器驗收腳本測檔案瀏覽。複製失敗（例如暫存目錄不可寫）視為啟動
    // 失敗，不悄悄退化成「沒有檔案根目錄可用」。
    let review_fixture = setup_review_fixture()
        .context("file-review task 3.4：準備 review-repo／other-repo fixture 失敗")?;
    add_review_fixture_panes(&mut initial, &review_fixture)
        .context("file-review task 3.4：把 fixture pane 掛進假投影失敗")?;
    apply_progress_scenarios(&mut initial);
    apply_override_disconnected_scenario(&mut initial);

    // R14 要在 `initial` 被搬進 `Arc::new` 之前先蒐集 pane id 集合，晚一步就借不到了。
    let known_panes = known_pane_ids(&initial);

    let (tx, rx) = watch::channel(Arc::new(initial));

    let push_every = push_interval()?;
    let write_rules = Arc::new(write_rules()?);
    let output_modes =
        parse_output_modes(env::var("COCKPIT_PREVIEW_OUTPUT_MODES").ok().as_deref())?;
    let vanish = parse_vanish_pane(env::var("COCKPIT_PREVIEW_VANISH_PANE").ok().as_deref())?;
    let cwd_move = parse_pane_after(CWD_MOVE_VAR, env::var(CWD_MOVE_VAR).ok().as_deref())?;
    // R14（live-output task 5.1 規格對照檢查的控制端追加）：這兩個環境變數指到 fixture
    // 投影裡不存在的 pane id 時，原本會靜默成功、對那個打錯的 pane id 套用預設行為——驗收
    // 腳本如果打錯 pane id，會在假 runtime 的預設行為下意外變成假綠。啟動時就擋下來，給
    // 清楚的錯誤訊息。
    validate_known_panes(&output_modes, &vanish, &known_panes)?;
    let cwd_move = match cwd_move {
        Some((pane, after)) => {
            anyhow::ensure!(
                known_panes.contains(&pane),
                "{CWD_MOVE_VAR} 指到 fixture 投影裡不存在的 pane id：{pane}"
            );
            let to = review_fixture
                .other_repo
                .to_str()
                .with_context(|| {
                    format!(
                        "other-repo 的路徑不是合法 UTF-8：{:?}",
                        review_fixture.other_repo
                    )
                })?
                .to_string();
            Some(CwdMove { pane, after, to })
        }
        None => None,
    };

    let fake_runtime = Arc::new(FakeOutputRuntime::new(OUTPUT_RUNTIME_ID, output_modes));
    let mut runtimes: HashMap<RuntimeId, Arc<dyn AgentRuntime>> = HashMap::new();
    runtimes.insert(RuntimeId::new(OUTPUT_RUNTIME_ID), fake_runtime);

    // brief（task 5.1）：`AppState::new` 的 port 初值是 0，`source_check` 會把帶明確埠號的
    // `Host`（例如瀏覽器與 curl 送來的 `Host: 127.0.0.1:<port>`）一律擋成 403（見
    // `AppState::new` 文件「fail-closed」那段）。這裡的 `listener` 已經在組 `AppState` 之前
    // 綁定好，所以直接把系統實際指派的埠寫進去即可，不需要像
    // `cockpit::app::run_with_shutdown` 那樣先組路由表、綁定後才回填。
    // file-review task 3.4：檔案端點的路徑對應表——`win` 是非 WSL 的假 runtime，`cwd` 原樣當
    // 主機路徑（`cockpit::files::PathMapping::Native`）。`wsl` 這個假 runtime 沒有對應表項目，
    // 對它的根目錄查詢一律 `runtime_unknown`（本 task 的 fixture 兩個新 pane 都掛在 `win`
    // 底下，見 [`add_review_fixture_panes`]）。
    let mut path_mappings: HashMap<RuntimeId, cockpit::files::PathMapping> = HashMap::new();
    path_mappings.insert(
        RuntimeId::new(OUTPUT_RUNTIME_ID),
        cockpit::files::PathMapping::Native,
    );

    let app_state = AppState {
        state: rx,
        progress: None,
        port: Arc::new(AtomicU16::new(actual_addr.port())),
        runtimes: Arc::new(runtimes),
        path_mappings: Arc::new(path_mappings),
        files: Arc::new(cockpit::files::FileSettings::embedded()),
        git_runner: Arc::new(cockpit_git::GitRunner::new()),
    };

    // 寫入路由放外層、其餘交給真正的 dashboard router 當 fallback：`Router::merge` 遇到同一
    // 路徑已有 POST／PUT／DELETE（http.rs 的正式寫入端點）會 panic，fallback 則只在外層沒有
    // 符合的路徑時才轉交。外層路徑符合但方法不符（例如 GET）由外層回 405，跟正式路由一致。
    let app = Router::new()
        .route(
            "/api/projects/{project}/tasks/{task}/{op}",
            post(record_write_request),
        )
        .route(
            "/api/projects/{project}/workstreams/{workstream}/override",
            put(record_write_request).delete(record_write_request),
        )
        .fallback_service(router(app_state))
        .with_state(write_rules);

    println!("ui_preview 監聽 http://{actual_addr}（Ctrl-C 結束）");
    println!(
        "推送間隔 {} ms；寫入請求只記錄、回 204",
        push_every.as_millis()
    );
    println!("試試：curl http://{actual_addr}/api/state");
    println!(
        "輸出讀取端點範例：curl -H \"Host: 127.0.0.1:{}\" http://{actual_addr}/api/runtimes/{OUTPUT_RUNTIME_ID}/panes/wJ:p1/output",
        actual_addr.port()
    );
    // file-review task 3.4：印出兩個假根目錄的路徑，供 task 3.5 的驗收腳本讀取並改寫副本內的
    // 檔案（例如測試「改檔後更新並保住捲動」）。
    println!("review-repo: {}", review_fixture.review_repo.display());
    println!("other-repo: {}", review_fixture.other_repo.display());

    let cycle_task = tokio::spawn(push_loop(tx, push_every, vanish, cwd_move, Instant::now()));

    tokio::select! {
        result = axum::serve(listener, app) => {
            result?;
        }
        _ = tokio::signal::ctrl_c() => {
            println!("收到 Ctrl-C，結束 ui_preview");
        }
    }

    cycle_task.abort();
    // file-review task 3.4：結束時清掉整個暫存目錄（`review-repo`／`other-repo` 的共同上層）。
    // 正常結束與 Ctrl-C 都會走到這裡；服務被強制關閉（例如工作管理員砍行程）沒有機會執行到
    // 這裡時，下次啟動的 `setup_review_fixture` 會經 `cleanup_stale_review_fixtures` 補收。
    cleanup_review_fixture(&review_fixture);
    Ok(())
}

// ---------------------------------------------------------------------------
// file-review task 3.4：假 repo fixture（design D12）
// ---------------------------------------------------------------------------

/// [`setup_review_fixture`] 的結果：兩個假根目錄複製到暫存目錄之後的路徑，供
/// [`add_review_fixture_panes`] 掛進假投影，也印到 stdout 供驗收腳本讀取。
struct ReviewFixture {
    /// 暫存父目錄（`review-repo`／`other-repo` 的共同上層），結束時整個刪除
    /// （[`cleanup_review_fixture`]）。
    temp_root: PathBuf,
    /// `review-repo` 副本的根目錄（含真正的 git 歷史——260 個以上 commit、分支與 merge、tag、
    /// 遠端追蹤分支、改名、刪除、二進位檔、中文檔名，以及工作區的已暫存／未暫存／未追蹤／
    /// 已刪除情境，見 [`build_review_repo_repo`]；另有 `.gitignore`、`target/`、`.env`；
    /// git-review task 3.1，design D10）。
    review_repo: PathBuf,
    /// `other-repo` 的根目錄（真正的 git 歷史，含一個進行中且有衝突的 merge，見
    /// [`build_other_repo_repo`]；另有 `README.md`、`lib/`；git-review task 3.1，design D10）。
    other_repo: PathBuf,
}

/// 把 [`REVIEW_REPO_SOURCE`] 複製到系統暫存目錄下一個唯一名稱的資料夾，並在副本內建立
/// `.gitignore`、`target/`、`.env`（design D12：這幾項會被 git 拒絕追蹤、或影響到本 repo 自己的
/// 忽略規則／建置產物，只能在複製之後動態建立，不能靜態存在於進版控的 fixture 目錄裡）；另建
/// 第二個根目錄 `other-repo`（`README.md`、`lib/`）。兩者都以本機 `git` 建立真正的歷史
/// （[`build_review_repo_repo`]／[`build_other_repo_repo`]；git-review task 3.1，design D10）
/// ——取代先前的空 `.git/` 資料夾；找不到 `git`、或任何一步 git 操作失敗，都視為啟動失敗（不靜默
/// 退化），因為 fixture 的內容是之後所有前端驗收腳本與人工測試的前提。
///
/// 啟動時先呼叫 [`cleanup_stale_review_fixtures`]，清掉前一次執行沒能正常結束（例如被強制關閉）
/// 而留下的舊暫存目錄。
///
/// # Errors
///
/// 複製或建立檔案系統項目失敗、或建立 git 歷史的任何一步失敗（含找不到 `git`）時回傳錯誤。
fn setup_review_fixture() -> anyhow::Result<ReviewFixture> {
    cleanup_stale_review_fixtures();

    let unique = format!(
        "{REVIEW_FIXTURE_PREFIX}{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    );
    let temp_root = env::temp_dir().join(unique);
    fs::create_dir_all(&temp_root)
        .with_context(|| format!("建立暫存目錄失敗：{}", temp_root.display()))?;

    let review_repo = temp_root.join("review-repo");
    copy_dir_recursive(Path::new(REVIEW_REPO_SOURCE), &review_repo).with_context(|| {
        format!(
            "複製 review-repo fixture 失敗：{} -> {}",
            REVIEW_REPO_SOURCE,
            review_repo.display()
        )
    })?;

    fs::write(review_repo.join(".gitignore"), "target/\n.env\n")
        .context("寫入 review-repo/.gitignore 失敗")?;
    fs::create_dir_all(review_repo.join("target")).context("建立 review-repo/target 失敗")?;
    fs::write(
        review_repo.join("target").join("build-marker.txt"),
        "假的建置產物（file-review task 3.4 fixture，不是真的由 cargo build 產生）。\n",
    )
    .context("寫入 review-repo/target/build-marker.txt 失敗")?;
    fs::write(review_repo.join(".env"), "SECRET=fixture-only-not-real\n")
        .context("寫入 review-repo/.env 失敗")?;

    let other_repo = temp_root.join("other-repo");
    fs::create_dir_all(other_repo.join("lib")).context("建立 other-repo/lib 失敗")?;
    fs::write(
        other_repo.join("README.md"),
        "# other-repo\n\n另一個根目錄（file-review task 3.4 fixture），用於驗證允許清單以
runtime 底下所有 pane 的 cwd 為界，而不是只有一個根目錄。\n",
    )
    .context("寫入 other-repo/README.md 失敗")?;

    // git-review task 3.1（design D10）：以本機 git 建立真正的歷史，取代先前的空 `.git/`
    // 資料夾。兩個 repo 共用同一份隔離設定（[`GitIsolation`]），確保 HEAD hash 不受使用者的
    // 全域／系統 git 設定影響、可重現。
    let isolation = GitIsolation::new(&temp_root).context("建立 git 隔離用的空全域設定檔失敗")?;
    build_review_repo_repo(&review_repo, &isolation)
        .context("git-review task 3.1：建立 review-repo 的真實 git 歷史失敗")?;
    build_other_repo_repo(&other_repo, &isolation).context(
        "git-review task 3.1：建立 other-repo 的真實 git 歷史（含一個進行中的合併衝突）失敗",
    )?;

    Ok(ReviewFixture {
        temp_root,
        review_repo,
        other_repo,
    })
}

// ---------------------------------------------------------------------------
// git-review task 3.1（design D10）：review-repo／other-repo 的真實 git 歷史
// ---------------------------------------------------------------------------

/// 固定作者身分，跟 [`GIT_FIXTURE_CONFIG`] 的 `user.name`／`user.email` 一致（fast-import 的
/// `author`／`committer`／`tagger` 行也用它）。
const GIT_FIXTURE_AUTHOR: &str = "cockpit-ui-preview <ui-preview@cockpit.invalid>";

/// 固定基準時間：2026-01-01T00:00:00Z 的 Unix 秒數。每個 fixture commit 依序遞增 60 秒
/// （[`git_fixture_date`]），讓歷史時間順序合理、且與時區無關。
const GIT_FIXTURE_BASE_EPOCH: i64 = 1_767_225_600;

/// 每次呼叫 git 都帶的固定前綴設定（`-c`）：身分、關掉 commit／tag 簽章、固定
/// `init.defaultBranch=main`，以及控制端裁決另外要求的 `core.autocrlf=false`／
/// `core.safecrlf=false`（避免這台機器的全域設定影響 fixture 內容與 HEAD hash 的可重現性）。
const GIT_FIXTURE_CONFIG: &[&str] = &[
    "-c",
    "user.name=cockpit-ui-preview",
    "-c",
    "user.email=ui-preview@cockpit.invalid",
    "-c",
    "commit.gpgsign=false",
    "-c",
    "tag.gpgsign=false",
    "-c",
    "core.autocrlf=false",
    "-c",
    "core.safecrlf=false",
    "-c",
    "init.defaultBranch=main",
];

/// 讓 fixture 建置不受使用者的全域／系統 git 設定影響（否則 HEAD hash 在不同機器、不同使用者
/// 設定下不可重現：例如全域設定了 `core.autocrlf=true` 或自訂的 `user.name`）。做法（控制端
/// 裁決，report 附實測）：`GIT_CONFIG_GLOBAL` 指向一個空檔案，完全蓋過 `~/.gitconfig`；
/// `GIT_CONFIG_NOSYSTEM=1` 另外關掉系統層設定（Windows 的機器層設定、`/etc/gitconfig`）。兩者
/// 合併後，[`GIT_FIXTURE_CONFIG`] 這組 `-c` 旗標就是 git 唯一讀得到的設定來源。
struct GitIsolation {
    empty_global_config: PathBuf,
}

impl GitIsolation {
    /// 在 `temp_root`（fixture 的暫存父目錄，結束時整個刪除）底下建立空的全域設定檔。
    fn new(temp_root: &Path) -> anyhow::Result<Self> {
        let empty_global_config = temp_root.join("git-empty-global-config");
        fs::write(&empty_global_config, "").context("建立空的全域 git 設定檔失敗")?;
        Ok(Self {
            empty_global_config,
        })
    }

    fn apply(&self, cmd: &mut Command) {
        cmd.env("GIT_CONFIG_NOSYSTEM", "1");
        cmd.env("GIT_CONFIG_GLOBAL", &self.empty_global_config);
    }
}

/// 組一個 `git -C <dir> <GIT_FIXTURE_CONFIG> <args>` 的 [`Command`]，套用 `isolation`。
fn git_cmd(dir: &Path, isolation: &GitIsolation, args: &[&str]) -> Command {
    let mut cmd = Command::new("git");
    cmd.arg("-C").arg(dir);
    cmd.args(GIT_FIXTURE_CONFIG);
    cmd.args(args);
    isolation.apply(&mut cmd);
    cmd
}

/// 執行一次 git，回傳 `Output`（不檢查結束狀態）。啟動失敗（找不到 `git`）時回傳清楚的錯誤，
/// 供呼叫端的 `.context(...)` 往外疊出「fixture 啟動失敗」的原因（brief：找不到 git 時 `ui_preview`
/// 啟動失敗並印出原因，不靜默降級）。
fn run_git(
    dir: &Path,
    isolation: &GitIsolation,
    args: &[&str],
) -> anyhow::Result<std::process::Output> {
    git_cmd(dir, isolation, args).output().with_context(|| {
        format!(
            "啟動 git 失敗（args={args:?}）：找不到 git，或無法執行——請安裝 git 並確認在 PATH 中"
        )
    })
}

/// [`run_git`] 再檢查結束狀態；失敗時把 stderr 一併附在錯誤訊息裡（這裡只用於建 fixture，不是
/// 對外的查詢路徑，附 stderr 有助於除錯，不受 spec「錯誤本體不得包含 git 的 stderr 原文」約束
/// ——那條規則管的是 git-review 端點的 HTTP 回應，不是這裡的啟動期診斷輸出）。
fn run_git_ok(
    dir: &Path,
    isolation: &GitIsolation,
    args: &[&str],
) -> anyhow::Result<std::process::Output> {
    let out = run_git(dir, isolation, args)?;
    anyhow::ensure!(
        out.status.success(),
        "git {args:?} 失敗（exit={:?}）：{}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );
    Ok(out)
}

/// [`run_git_ok`] 再帶固定的 `GIT_AUTHOR_DATE`／`GIT_COMMITTER_DATE`（[`git_fixture_date`]）
/// ——只有 `commit`／`merge` 這類會建立 commit 的呼叫需要。
fn run_git_ok_with_dates(
    dir: &Path,
    isolation: &GitIsolation,
    date_index: u32,
    args: &[&str],
) -> anyhow::Result<std::process::Output> {
    let date = git_fixture_date(date_index);
    let mut cmd = git_cmd(dir, isolation, args);
    cmd.env("GIT_AUTHOR_DATE", &date);
    cmd.env("GIT_COMMITTER_DATE", &date);
    let out = cmd.output().with_context(|| {
        format!(
            "啟動 git 失敗（args={args:?}）：找不到 git，或無法執行——請安裝 git 並確認在 PATH 中"
        )
    })?;
    anyhow::ensure!(
        out.status.success(),
        "git {args:?} 失敗（exit={:?}）：{}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );
    Ok(out)
}

/// 控制端裁決：`git init` 之後先驗證 `rev-parse --show-toplevel` 真的等於 `dir`，不符就中止——
/// 防止 `git init` 沒有如預期在 `dir` 內生效（例如 `dir` 其實在另一個既有 repo 底下）時，後續的
/// `commit`／`fast-import`／`reset --hard` 誤操作到別的 repo。兩側都先 `canonicalize`：Windows
/// git 印出的路徑可能用正斜線與短路徑，直接比對字串不可靠。
fn verify_repo_toplevel(dir: &Path, isolation: &GitIsolation) -> anyhow::Result<()> {
    let out = run_git_ok(dir, isolation, &["rev-parse", "--show-toplevel"])?;
    let printed = String::from_utf8(out.stdout)
        .context("git rev-parse --show-toplevel 輸出不是合法 UTF-8")?;
    let printed_path = PathBuf::from(printed.trim());
    let expected = fs::canonicalize(dir)
        .with_context(|| format!("無法正規化 fixture 目錄路徑：{}", dir.display()))?;
    let actual = fs::canonicalize(&printed_path).with_context(|| {
        format!("git rev-parse --show-toplevel 印出的路徑無法正規化：{printed_path:?}")
    })?;
    anyhow::ensure!(
        expected == actual,
        "git init 之後 rev-parse --show-toplevel 回傳 {actual:?}，與 fixture 目錄 {expected:?} \
         不符，中止啟動以避免對其他 repo 誤操作"
    );
    Ok(())
}

/// 把 `stream` 餵給 `git fast-import --quiet`（design D10：260 個以上 commit 用一次 fast-import
/// 建立，不逐筆 `git commit`）。
fn run_fast_import(dir: &Path, isolation: &GitIsolation, stream: &[u8]) -> anyhow::Result<()> {
    let mut cmd = git_cmd(dir, isolation, &["fast-import", "--quiet"]);
    cmd.stdin(Stdio::piped());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    let mut child = cmd.spawn().with_context(
        || "啟動 git fast-import 失敗：找不到 git，或無法執行——請安裝 git 並確認在 PATH 中",
    )?;
    child
        .stdin
        .take()
        .expect("已設定 Stdio::piped，spawn 成功後 stdin 一定存在")
        .write_all(stream)
        .context("寫入 git fast-import 的 stdin 失敗")?;
    let output = child
        .wait_with_output()
        .context("等待 git fast-import 結束失敗")?;
    anyhow::ensure!(
        output.status.success(),
        "git fast-import 失敗（exit={:?}）：{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(())
}

/// `index` 對應的固定時間：`GIT_FIXTURE_BASE_EPOCH + index * 60` 秒，UTC（`+0000`）。
fn git_fixture_date(index: u32) -> String {
    format!("{} +0000", GIT_FIXTURE_BASE_EPOCH + i64::from(index) * 60)
}

/// fast-import 的 `author`／`committer`／`tagger` 行內容（不含開頭的關鍵字）。
fn author_line(date_index: u32) -> String {
    format!("{GIT_FIXTURE_AUTHOR} {}", git_fixture_date(date_index))
}

/// 一段可重現的「二進位內容」：512 個位元組，涵蓋全部位元組值（含 `0x00`），`seed` 讓兩次呼叫的
/// 內容不同（供「新增二進位檔」與「修改二進位檔」各一次 fast-import commit）。
fn binary_pattern(seed: u8) -> Vec<u8> {
    (0u16..512).map(|i| (i as u8).wrapping_add(seed)).collect()
}

/// 寫入 fast-import 的 `commit` 指令區塊（`commit`／`mark`／`author`／`committer`／`data`），
/// 不含 `from`／`merge`（呼叫端視情況緊接著自己 `buf.extend_from_slice` 加上）、也不含檔案異動
/// 指令（[`fi_modify_bytes`] 等）。`message` 需含結尾換行。
fn fi_commit(buf: &mut Vec<u8>, ref_name: &str, mark: u32, date_index: u32, message: &[u8]) {
    buf.extend_from_slice(format!("commit {ref_name}\n").as_bytes());
    buf.extend_from_slice(format!("mark :{mark}\n").as_bytes());
    let author = author_line(date_index);
    buf.extend_from_slice(format!("author {author}\n").as_bytes());
    buf.extend_from_slice(format!("committer {author}\n").as_bytes());
    buf.extend_from_slice(format!("data {}\n", message.len()).as_bytes());
    buf.extend_from_slice(message);
    buf.push(b'\n');
}

/// 寫入一筆 `M 100644 inline <path>` 指令＋內容（二進位安全：位元組計數框住 `content`，內容本身
/// 可以含任意位元組，含 `0x00`）。
fn fi_modify_bytes(buf: &mut Vec<u8>, path: &str, content: &[u8]) {
    buf.extend_from_slice(format!("M 100644 inline {path}\n").as_bytes());
    buf.extend_from_slice(format!("data {}\n", content.len()).as_bytes());
    buf.extend_from_slice(content);
    buf.push(b'\n');
}

/// [`fi_modify_bytes`] 的文字版本。
fn fi_modify_text(buf: &mut Vec<u8>, path: &str, content: &str) {
    fi_modify_bytes(buf, path, content.as_bytes());
}

/// 寫入一筆改名指令（`R <from> <to>`）：內容沿用同一個 blob，不需要重新提供資料。
fn fi_rename(buf: &mut Vec<u8>, from: &str, to: &str) {
    buf.extend_from_slice(format!("R {from} {to}\n").as_bytes());
}

/// 寫入一筆刪除指令（`D <path>`）。
fn fi_delete(buf: &mut Vec<u8>, path: &str) {
    buf.extend_from_slice(format!("D {path}\n").as_bytes());
}

/// git-review task 3.1（design D10）：`review-repo` 的合成歷史——260 個以上 commit、兩條分支
/// （`feature/logging` 不合併回 main、供「依分支篩選」情境；`feature/formatting` 合併回 main）
/// 與一次 merge、附註 tag（`v0.1.0`）、一個假的遠端追蹤分支（`refs/remotes/origin/main`）、改名、
/// 刪除、二進位檔（新增與修改各一次）、中文檔名。全部包在新增的 `history/` 目錄底下，不觸碰
/// `root_sha`（既有 11 個靜態 fixture 檔案）的任何一個路徑。用一次 `git fast-import` 建立
/// （design D10：避免 260 次個別 `git commit` 的啟動開銷；耗時見報告）。
fn build_review_history_stream(root_sha: &str) -> Vec<u8> {
    /// 填充 commit 的數量（歷史主體，內容為遞增的計數器）。
    const BULK_FILLER: u32 = 250;
    /// 第幾筆填充 commit 上打附註 tag。
    const TAG_AT_FILLER: u32 = 100;
    /// 第幾筆填充 commit 是 `refs/remotes/origin/main` 指向的位置（刻意落後主線末端，模擬
    /// 「本地領先遠端」）。
    const REMOTE_AT_FILLER: u32 = 245;

    let mut buf = Vec::new();
    let mut mark: u32 = 0;
    let mut date_index: u32 = 1; // 0 給根 commit（在 fast-import 之外，plain `git commit` 建立）
    let mut tag_mark = 0u32;
    let mut remote_mark = 0u32;

    for i in 1..=BULK_FILLER {
        mark += 1;
        date_index += 1;
        let message = format!("填充 commit #{i}（git-review task 3.1 fixture）\n");
        fi_commit(
            &mut buf,
            "refs/heads/main",
            mark,
            date_index,
            message.as_bytes(),
        );
        if mark == 1 {
            buf.extend_from_slice(format!("from {root_sha}\n").as_bytes());
        }
        fi_modify_text(&mut buf, "history/counter.txt", &format!("{i}\n"));
        if i == TAG_AT_FILLER {
            tag_mark = mark;
        }
        if i == REMOTE_AT_FILLER {
            remote_mark = mark;
        }
    }

    mark += 1;
    date_index += 1;
    fi_commit(
        &mut buf,
        "refs/heads/main",
        mark,
        date_index,
        "新增待改名的檔案（git-review task 3.1 fixture）\n".as_bytes(),
    );
    fi_modify_text(
        &mut buf,
        "history/rename-source.txt",
        "這個檔案接下來會被改名（git-review task 3.1 fixture）。\n",
    );

    mark += 1;
    date_index += 1;
    fi_commit(
        &mut buf,
        "refs/heads/main",
        mark,
        date_index,
        "改名：history/rename-source.txt -> history/renamed-target.txt（git-review task 3.1 fixture）\n"
            .as_bytes(),
    );
    fi_rename(
        &mut buf,
        "history/rename-source.txt",
        "history/renamed-target.txt",
    );

    mark += 1;
    date_index += 1;
    fi_commit(
        &mut buf,
        "refs/heads/main",
        mark,
        date_index,
        "新增二進位檔（git-review task 3.1 fixture）\n".as_bytes(),
    );
    fi_modify_bytes(&mut buf, "history/binary-asset.bin", &binary_pattern(0));

    mark += 1;
    date_index += 1;
    fi_commit(
        &mut buf,
        "refs/heads/main",
        mark,
        date_index,
        "修改二進位檔（git-review task 3.1 fixture）\n".as_bytes(),
    );
    fi_modify_bytes(&mut buf, "history/binary-asset.bin", &binary_pattern(1));

    mark += 1;
    date_index += 1;
    fi_commit(
        &mut buf,
        "refs/heads/main",
        mark,
        date_index,
        "新增中文檔名（git-review task 3.1 fixture）\n".as_bytes(),
    );
    fi_modify_text(
        &mut buf,
        "history/中文檔名.md",
        "中文檔名測試（git-review task 3.1 fixture）。\n",
    );

    mark += 1;
    date_index += 1;
    fi_commit(
        &mut buf,
        "refs/heads/main",
        mark,
        date_index,
        "新增稍後會被刪除的檔案（git-review task 3.1 fixture）\n".as_bytes(),
    );
    fi_modify_text(
        &mut buf,
        "history/will-be-deleted-in-history.txt",
        "這個檔案接下來會從歷史中被刪除（git-review task 3.1 fixture）。\n",
    );

    mark += 1;
    date_index += 1;
    fi_commit(
        &mut buf,
        "refs/heads/main",
        mark,
        date_index,
        "刪除 history/will-be-deleted-in-history.txt（git-review task 3.1 fixture）\n".as_bytes(),
    );
    fi_delete(&mut buf, "history/will-be-deleted-in-history.txt");

    mark += 1;
    date_index += 1;
    fi_commit(
        &mut buf,
        "refs/heads/main",
        mark,
        date_index,
        "新增工作區情境要用到的追蹤檔案（git-review task 3.1 fixture）\n".as_bytes(),
    );
    fi_modify_text(
        &mut buf,
        "history/staged-change.txt",
        "尚未修改（git-review task 3.1 fixture）。\n",
    );
    fi_modify_text(
        &mut buf,
        "history/unstaged-change.txt",
        "尚未修改（git-review task 3.1 fixture）。\n",
    );
    fi_modify_text(
        &mut buf,
        "history/deleted-in-worktree.txt",
        "稍後會從工作區刪除，但保留在歷史中（git-review task 3.1 fixture）。\n",
    );

    let branch_point_mark = mark;

    mark += 1;
    date_index += 1;
    fi_commit(
        &mut buf,
        "refs/heads/feature/logging",
        mark,
        date_index,
        "feature/logging：第一個 commit（git-review task 3.1 fixture）\n".as_bytes(),
    );
    buf.extend_from_slice(format!("from :{branch_point_mark}\n").as_bytes());
    fi_modify_text(
        &mut buf,
        "history/feature-logging.txt",
        "feature/logging 分支的內容，第一版（git-review task 3.1 fixture）。\n",
    );

    mark += 1;
    date_index += 1;
    fi_commit(
        &mut buf,
        "refs/heads/feature/logging",
        mark,
        date_index,
        "feature/logging：第二個 commit（git-review task 3.1 fixture）\n".as_bytes(),
    );
    fi_modify_text(
        &mut buf,
        "history/feature-logging.txt",
        "feature/logging 分支的內容，第二版（git-review task 3.1 fixture）。\n",
    );

    mark += 1;
    date_index += 1;
    let feature_formatting_mark = mark;
    fi_commit(
        &mut buf,
        "refs/heads/feature/formatting",
        mark,
        date_index,
        "feature/formatting：修改格式化相關檔案（git-review task 3.1 fixture）\n".as_bytes(),
    );
    buf.extend_from_slice(format!("from :{branch_point_mark}\n").as_bytes());
    fi_modify_text(
        &mut buf,
        "history/feature-formatting.txt",
        "feature/formatting 分支的內容（git-review task 3.1 fixture）。\n",
    );

    mark += 1;
    date_index += 1;
    let main_before_merge_mark = mark;
    fi_commit(
        &mut buf,
        "refs/heads/main",
        mark,
        date_index,
        "分支點之後 main 再前進一個 commit（git-review task 3.1 fixture）\n".as_bytes(),
    );
    fi_modify_text(&mut buf, "history/counter.txt", "post-branch\n");

    mark += 1;
    date_index += 1;
    fi_commit(
        &mut buf,
        "refs/heads/main",
        mark,
        date_index,
        "合併 feature/formatting 回 main（git-review task 3.1 fixture）\n".as_bytes(),
    );
    buf.extend_from_slice(format!("from :{main_before_merge_mark}\n").as_bytes());
    buf.extend_from_slice(format!("merge :{feature_formatting_mark}\n").as_bytes());
    fi_modify_text(
        &mut buf,
        "history/feature-formatting.txt",
        "feature/formatting 分支的內容（git-review task 3.1 fixture）。\n",
    );

    buf.extend_from_slice(b"tag v0.1.0\n");
    buf.extend_from_slice(format!("from :{tag_mark}\n").as_bytes());
    buf.extend_from_slice(format!("tagger {}\n", author_line(date_index)).as_bytes());
    let tag_message = "release marker（git-review task 3.1 fixture）。\n";
    buf.extend_from_slice(format!("data {}\n", tag_message.len()).as_bytes());
    buf.extend_from_slice(tag_message.as_bytes());
    buf.push(b'\n');

    buf.extend_from_slice(b"reset refs/remotes/origin/main\n");
    buf.extend_from_slice(format!("from :{remote_mark}\n").as_bytes());

    buf
}

/// `review-repo`：`git init`、根 commit（既有 11 個靜態 fixture 檔案，逐位元組不變）、合成歷史
/// （[`build_review_history_stream`]），最後把工作區／index 同步到新的 `main` tip，並疊上
/// design D10 要求的四種工作區情境（已暫存、未暫存、未追蹤、已刪除；全部落在新增的 `history/`
/// 檔案上，不動任何一個既有靜態檔案）。
///
/// # Errors
///
/// 任一步 git 操作失敗（含找不到 `git`）時回傳錯誤。
fn build_review_repo_repo(review_repo: &Path, isolation: &GitIsolation) -> anyhow::Result<()> {
    run_git_ok(review_repo, isolation, &["init", "-q", "-b", "main"])?;
    verify_repo_toplevel(review_repo, isolation)?;

    // 根 commit：`.gitignore` 已在呼叫前寫入，`git add -A` 自然不會追蹤它排除的 `target/`／
    // `.env`；既有 11 個靜態檔案的內容在這裡逐位元組不變地進版——file-review 既有六支驗收腳本
    // 對它們的內容有逐字比對。
    run_git_ok(review_repo, isolation, &["add", "-A"])?;
    run_git_ok_with_dates(
        review_repo,
        isolation,
        0,
        &[
            "commit",
            "-q",
            "-m",
            "根 commit：既有 review-repo fixture 靜態內容（git-review task 3.1）",
        ],
    )?;
    let root_sha =
        String::from_utf8(run_git_ok(review_repo, isolation, &["rev-parse", "HEAD"])?.stdout)
            .context("git rev-parse HEAD 輸出不是合法 UTF-8")?
            .trim()
            .to_string();

    let stream = build_review_history_stream(&root_sha);
    run_fast_import(review_repo, isolation, &stream)?;

    // fast-import 只建物件與 refs，不動工作區與 index；同步兩者到 main 的新 tip，讓既有 11 個
    // 靜態檔案（內容不變）與新增的 `history/` 內容一起出現在磁碟上。
    run_git_ok(review_repo, isolation, &["reset", "-q", "--hard", "main"])?;

    // design D10：工作區的四種未 commit 情境，全部落在 `reset --hard` 剛materialize出來的
    // `history/` 檔案上，不動任何一個既有靜態檔案。
    fs::write(
        review_repo.join("history").join("staged-change.txt"),
        "已暫存修改（git-review task 3.1 fixture）。\n",
    )
    .context("寫入 history/staged-change.txt（已暫存修改）失敗")?;
    run_git_ok(
        review_repo,
        isolation,
        &["add", "history/staged-change.txt"],
    )?;

    fs::write(
        review_repo.join("history").join("unstaged-change.txt"),
        "未暫存修改（git-review task 3.1 fixture）。\n",
    )
    .context("寫入 history/unstaged-change.txt（未暫存修改）失敗")?;

    fs::write(
        review_repo.join("history").join("untracked-file.md"),
        "未追蹤檔案（git-review task 3.1 fixture）。\n",
    )
    .context("寫入 history/untracked-file.md（未追蹤檔案）失敗")?;

    fs::remove_file(review_repo.join("history").join("deleted-in-worktree.txt"))
        .context("刪除 history/deleted-in-worktree.txt（已刪除檔案）失敗")?;

    Ok(())
}

/// `other-repo`：`git init`、加入 `README.md`，然後建立一個進行中且有衝突的 merge（design D10：
/// 合併衝突放在 `other-repo`，前提是不改變既有驗收腳本依賴的行為——既有腳本只檢查
/// `README.md` 存在、`lib/` 是一個項目，兩者在這裡都不受影響）。`branch-a`／`branch-b` 各自從
/// 同一個基底修改 `conflict.txt`，在 `branch-b` 上合併 `branch-a` 預期以非零結束並留下衝突標記
/// 與 `.git/MERGE_HEAD`——這是本函式需要的前提，不是失敗。
///
/// # Errors
///
/// 任一步「預期成功」的 git 操作失敗（含找不到 `git`）時回傳錯誤；merge 本身「預期失敗」，若它
/// 意外成功則視為前提不成立，同樣回傳錯誤。
fn build_other_repo_repo(other_repo: &Path, isolation: &GitIsolation) -> anyhow::Result<()> {
    run_git_ok(other_repo, isolation, &["init", "-q", "-b", "main"])?;
    verify_repo_toplevel(other_repo, isolation)?;

    run_git_ok(other_repo, isolation, &["add", "-A"])?;
    run_git_ok_with_dates(
        other_repo,
        isolation,
        0,
        &[
            "commit",
            "-q",
            "-m",
            "other-repo fixture：加入 README.md（git-review task 3.1）",
        ],
    )?;

    fs::write(other_repo.join("conflict.txt"), "base\n")
        .context("寫入 other-repo/conflict.txt 失敗")?;
    run_git_ok(other_repo, isolation, &["add", "conflict.txt"])?;
    run_git_ok_with_dates(
        other_repo,
        isolation,
        1,
        &[
            "commit",
            "-q",
            "-m",
            "加入 conflict.txt 作為合併衝突的共同基底（git-review task 3.1）",
        ],
    )?;

    run_git_ok(other_repo, isolation, &["checkout", "-q", "-b", "branch-a"])?;
    fs::write(other_repo.join("conflict.txt"), "branch-a change\n")
        .context("寫入 other-repo/conflict.txt（branch-a）失敗")?;
    run_git_ok(other_repo, isolation, &["add", "conflict.txt"])?;
    run_git_ok_with_dates(
        other_repo,
        isolation,
        2,
        &["commit", "-q", "-m", "branch-a：修改 conflict.txt"],
    )?;

    run_git_ok(other_repo, isolation, &["checkout", "-q", "main"])?;
    run_git_ok(other_repo, isolation, &["checkout", "-q", "-b", "branch-b"])?;
    fs::write(other_repo.join("conflict.txt"), "branch-b change\n")
        .context("寫入 other-repo/conflict.txt（branch-b）失敗")?;
    run_git_ok(other_repo, isolation, &["add", "conflict.txt"])?;
    run_git_ok_with_dates(
        other_repo,
        isolation,
        3,
        &["commit", "-q", "-m", "branch-b：修改 conflict.txt"],
    )?;

    let merge_out = git_cmd(other_repo, isolation, &["merge", "branch-a"])
        .output()
        .with_context(
            || "啟動 git merge 失敗：找不到 git，或無法執行——請安裝 git 並確認在 PATH 中",
        )?;
    anyhow::ensure!(
        !merge_out.status.success(),
        "other-repo 的合併應該因內容衝突而失敗（這是本 fixture 需要的前提），實際卻成功了"
    );

    Ok(())
}

/// 遞迴複製 `src` 底下的所有檔案與資料夾到 `dst`（`dst` 不存在時建立）。只處理一般檔案與資料夾
/// ——fixture 來源目錄不含符號連結，不需要特別處理。
fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let target = dst.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir_recursive(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

/// 從暫存目錄名稱（`{REVIEW_FIXTURE_PREFIX}<pid>-<奈秒時間戳>`，見 [`setup_review_fixture`]）
/// 解出 `<pid>` 那一段；名稱不是這個形狀（前綴不對、缺 `-`、PID 那段不是合法 `u32`）時回
/// `None`——呼叫端把「解不出 PID」當成「不知道是誰在用，保留不刪」，不是「這個名字看起來像
/// 我們的東西，刪了再說」。
fn parse_fixture_dir_pid(name: &str) -> Option<u32> {
    let rest = name.strip_prefix(REVIEW_FIXTURE_PREFIX)?;
    let pid_str = rest.split('-').next()?;
    pid_str.parse::<u32>().ok()
}

/// 一個暫存目錄名稱在啟動清理時能不能刪：只有解析得出 PID（[`parse_fixture_dir_pid`]）、且
/// `pid_alive` 明確回報「這個 PID 現在不存在」（`Some(false)`）時才刪；解不出 PID，或
/// `pid_alive` 回報「還活著」（`Some(true)`）或「不知道」（`None`，例如查詢本身失敗）一律保留。
///
/// file-review task 3.4 fix round 2（Codex 對 3.1–3.4 合併審查 finding）：啟動清理原本只憑目錄
/// 名稱前綴就整批刪除，完全沒確認擁有者是否還在執行；`setup_review_fixture` 又在
/// `TcpListener::bind` 之前呼叫，於是「誤啟動第二個同埠實例」這種情境會先刪掉第一個實例的
/// `review-repo`／`other-repo`，讓它的檔案請求開始失敗、驗收期間對副本做的任何改寫也一起消失，
/// 隨後第二個實例才因為埠已被占用而退出——刪錯的代價已經造成，退出也於事無補。這裡改成「保留優先
/// 於刪除」：任何不確定的情況都不刪，讓真正的殘留目錄多留幾次啟動也沒關係（下次啟動還會再試一
/// 次），但絕不能刪到還在用的目錄。
///
/// 已知限制（控制端裁決）：PID 會被作業系統重複利用，這裡只用「這個 PID 現在是否存在」判斷存活，
/// 沒有比對目錄名稱裡奈秒時間戳與該 PID 的程序啟動時間（Windows 沒有從 `tasklist` 這類文字輸出
/// 取得程序啟動時間的簡單管道）。後果：如果一個殘留目錄的 PID 剛好被另一個仍在執行的無關行程
/// 重用，這個真正的殘留目錄會被保留而不是刪除——保守但安全，不會誤刪任何人正在用的東西。
fn should_delete_stale_dir(name: &str, pid_alive: impl Fn(u32) -> Option<bool>) -> bool {
    let Some(pid) = parse_fixture_dir_pid(name) else {
        return false;
    };
    pid_alive(pid) == Some(false)
}

/// 用 `tasklist /FI "PID eq <pid>" /NH` 判斷某個 PID 現在是否存在（Windows）；`tasklist` 本身
/// 無論找不找得到都以成功狀態結束，差別在輸出內容——找到時會有一行含該 PID 數字的資料列，找不到
/// 時只有一則（依系統語系而定的）提示訊息，不含任何數字。用「輸出裡有沒有一個 token 恰好等於這個
/// PID 的十進位字串」判斷，不依賴訊息文字本身（避開語系問題）。執行 `tasklist` 本身失敗（例如
/// 找不到這個執行檔）或它以非成功狀態結束時回 `None`（無法判斷）。
///
/// 這裡直接呼叫 `tasklist.exe`（`std::process::Command`，不經過任何 shell），引數各自成一個
/// 陣列元素，不會有 Git Bash／MSYS 那種把 `/FI` 當路徑轉換的問題（那是 shell 層級的行為，
/// `Command` 不會呼叫 shell 去解讀這些引數）。
fn pid_is_alive(pid: u32) -> Option<bool> {
    let output = std::process::Command::new("tasklist")
        .args(["/FI", &format!("PID eq {pid}"), "/NH"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let pid_str = pid.to_string();
    Some(stdout.split_whitespace().any(|token| token == pid_str))
}

/// [`cleanup_stale_fixtures_under`] 的核心邏輯，`pid_alive` 抽成參數讓測試不必依賴真正跑在這台
/// 機器上的行程（傳一個假的存活判斷進來，結果就是確定的）。清掉 `under` 底下所有以
/// [`REVIEW_FIXTURE_PREFIX`] 開頭、且經 [`should_delete_stale_dir`] 判斷可以刪的項目。刪除失敗
/// （例如檔案被其他行程鎖住）忽略不報錯——這只是盡力清理，不是正確性所需：就算清不掉，下一次呼叫
/// 還會再試一次，且不影響這一次啟動本身。
fn cleanup_stale_fixtures_with(under: &Path, pid_alive: impl Fn(u32) -> Option<bool>) {
    let Ok(entries) = fs::read_dir(under) else {
        return;
    };
    for entry in entries.flatten() {
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        if name.starts_with(REVIEW_FIXTURE_PREFIX) && should_delete_stale_dir(&name, &pid_alive) {
            let _ = fs::remove_dir_all(entry.path());
        }
    }
}

/// [`cleanup_stale_fixtures_with`] 搭配真正的 [`pid_is_alive`]（`under` 一律是
/// [`env::temp_dir`]；抽出 `under` 參數只是為了讓 [`cleanup_stale_fixtures_with`] 的測試不必碰
/// 真正的系統暫存目錄）。
fn cleanup_stale_fixtures_under(under: &Path) {
    cleanup_stale_fixtures_with(under, pid_is_alive);
}

/// [`cleanup_stale_fixtures_under`] 套用在真正的系統暫存目錄。
fn cleanup_stale_review_fixtures() {
    cleanup_stale_fixtures_under(&env::temp_dir());
}

/// 結束時清掉整個暫存目錄（呼叫點見 `main`：正常結束與 Ctrl-C 都會執行到）。刪除失敗只印錯誤、
/// 不讓整個程式因此以非零狀態結束——這是收尾步驟，服務本身已經在關閉了。
fn cleanup_review_fixture(fixture: &ReviewFixture) {
    if let Err(err) = fs::remove_dir_all(&fixture.temp_root) {
        eprintln!("清理暫存目錄失敗：{}：{err}", fixture.temp_root.display());
    }
}

/// file-review task 3.4（design D12）：把兩個指向假 repo 副本的 pane 掛進假投影——`wJ:p4` 的
/// `cwd` 在 `review-repo` 的子資料夾 `src`（驗「往上找到根目錄」），`wJ:p5` 的 `cwd` 直接是
/// `other-repo`（另一個根目錄）。兩者都掛在既有的 `win` runtime、既有 workspace `wJ`，但放在
/// **新增的** tab `wJ:t2`，`exited: false`：
///
/// - 不能動 `wJ:t1` 既有三個 pane 的任何欄位——既有六支驗收腳本對它們有逐字比對（例如
///   `actions-check.js` 的改綁候選清單、`reconnect-check.js` 挑第一列的狀態）。
/// - **必須是 `exited: false`**：`live-output` spec「選定一個 pane」寫明「runtime 卡中每個
///   *未 exited* 的 pane 列可點選」，file-review 的檔案樹又是以「目前選定 pane」的根目錄為準
///   （spec「左欄檔案樹」「切到檔案分頁」等 scenario 的 GIVEN 都是先選定一個 pane）——若這兩個
///   新 pane 是 `exited: true`，之後 task 3.5／4.x 的瀏覽器驗收腳本會沒有任何辦法把它們選成
///   目前的 pane，整個 fixture 對「透過畫面操作瀏覽檔案」這件事就形同虛設，只剩下 curl 直接打
///   pane id 這一種用法。原本選 `exited: true`是為了不驚動
///   `render.js`（`renderPane`）在改綁模式下把每個非 exited、connected runtime 的 pane 都列進
///   「綁定到這裡」候選清單，而 `actions-check.js` 對這份候選清單有逐字比對；但那份比對只是
///   「隨 fixture 資料變動的清單」（隨新 pane 數量增減），不是 spec 定義本身不能變的行為，因此
///   改成 `exited: false`、同時更新 `actions-check.js` 那一條斷言（獨立 commit），比犧牲兩個
///   pane 的可選取性更合理。`exited: false` 也仍會被
///   [`authorize_root`](cockpit::files::authorize_root) 算進允許清單（spec「檔案根目錄與允許
///   清單」對 exited／非 exited 一視同仁），不影響 curl 驗收。
///
/// 兩個新 pane 的 `cwd` 一定是合法 UTF-8（[`ReviewFixture`] 的路徑都是 [`env::temp_dir`] 疊上
/// 純 ASCII 段組成），這裡的 `to_str()` 失敗視為環境異常（例如系統暫存目錄本身不是合法
/// UTF-8），回傳錯誤而不是靜默造出一個沒有 `cwd` 的 pane。
///
/// # Errors
///
/// `fixture` 內任一路徑不是合法 UTF-8 時回傳錯誤。
fn add_review_fixture_panes(
    state: &mut ProjectedState,
    fixture: &ReviewFixture,
) -> anyhow::Result<()> {
    let review_src = fixture.review_repo.join("src");
    let review_src_cwd = review_src
        .to_str()
        .with_context(|| format!("review-repo/src 的路徑不是合法 UTF-8：{review_src:?}"))?
        .to_string();
    let other_repo_cwd = fixture
        .other_repo
        .to_str()
        .with_context(|| format!("other-repo 的路徑不是合法 UTF-8：{:?}", fixture.other_repo))?
        .to_string();

    let updated_at = "2026-09-27T00:00:00Z".to_string();
    let panes = vec![
        ProjectedPane {
            id: PaneId::new("wJ:p4"),
            agent: None,
            agent_status: AgentStatus::Idle,
            title: Some("review-repo（子資料夾）".to_string()),
            cwd: Some(review_src_cwd),
            label: None,
            focused: false,
            exited: false,
            updated_at: updated_at.clone(),
        },
        ProjectedPane {
            id: PaneId::new("wJ:p5"),
            agent: None,
            agent_status: AgentStatus::Idle,
            title: Some("other-repo".to_string()),
            cwd: Some(other_repo_cwd),
            label: None,
            focused: false,
            exited: false,
            updated_at,
        },
    ];
    let tab = ProjectedTab {
        id: TabId::new("wJ:t2"),
        number: 2,
        agent_status: AgentStatus::Idle,
        focused: false,
        panes,
    };

    let workspace = state
        .runtimes
        .iter_mut()
        .find(|rt| rt.id.as_str() == OUTPUT_RUNTIME_ID)
        .and_then(|rt| rt.workspaces.first_mut())
        .with_context(|| {
            format!("fixture 投影裡找不到 runtime {OUTPUT_RUNTIME_ID:?} 的第一個 workspace")
        })?;
    workspace.tabs.push(tab);
    Ok(())
}

/// 每條 workstream 的目前 task：既有情境裡原本呈 `running`／`blocked` 的 task 設為其
/// workstream 的目前 task（progress-model task 4.1；StageStatus 只有目前 task 才會跟 agent 狀態
/// 變 running／blocked），讓既有畫面與驗收腳本的斷言不變。`(project, workstream, task)`。
const PREVIEW_ACTIVE_TASKS: [(&str, &str, &str); 5] = [
    ("cockpit", "be", "be-1"),
    ("cockpit", "qa", "qa-1"),
    ("p", "backend", "backend-1"),
    ("p", "frontend", "frontend-1"),
    ("p", "tests", "tests-1"),
];

/// 預覽用的進度情境（progress-model task 4.1）：
///
/// 1. 為 [`PREVIEW_ACTIVE_TASKS`] 的 workstream 填上 `active_task`；
/// 2. 在 project `p`（Scenario D Demo）的 workstream／task 末尾附加：workstream `undeclared`
///    綁定 `wJ:p1`（working）、沒有目前 task，投影 `activity_undeclared=true`；其下 task
///    `undeclared-1`（stage `Implement`、`mark=none`、不在第一站）供「退回」按鈕驗收。不另開新
///    project：`factory-floor-check.js` 斷言左欄恰好兩個 Project（spec 未改變）。
///    新 task 為 `ready`、不是 running，不影響 p 的 running 節點數（3）。
fn apply_progress_scenarios(state: &mut ProjectedState) {
    for (project, workstream, task) in PREVIEW_ACTIVE_TASKS {
        let ws = state
            .projects
            .iter_mut()
            .find(|p| p.id.as_str() == project)
            .and_then(|p| {
                p.workstreams
                    .iter_mut()
                    .find(|w| w.id.as_str() == workstream)
            })
            .unwrap_or_else(|| panic!("fixture 缺少 workstream {project}/{workstream}"));
        ws.active_task = Some(TaskId::new(task));
    }

    let p = state
        .projects
        .iter_mut()
        .find(|p| p.id.as_str() == "p")
        .expect("fixture 缺少 project p");
    p.workstreams.push(ProjectedWorkstream {
        id: WorkstreamId::new("undeclared"),
        name: "Undeclared".to_string(),
        binding: ProjectedBinding::Bound {
            runtime: RuntimeId::new(OUTPUT_RUNTIME_ID),
            pane_id: PaneId::new("wJ:p1"),
            source: BindingSource::Auto,
            agent: Some("claude".to_string()),
            agent_status: AgentStatus::Working,
        },
        active_task: None,
        activity_undeclared: true,
    });
    p.tasks.push(ProjectedTask {
        id: TaskId::new("undeclared-1"),
        title: "Retreat target".to_string(),
        workstream: WorkstreamId::new("undeclared"),
        stage: "Implement".to_string(),
        mark: Mark::None,
        status: StageStatus::Ready,
        depends_on: Vec::new(),
    });
}

/// 斷線 runtime 上的覆蓋綁定情境（ui-fixes task 4.1）：在 project `cockpit` 的 workstream 末尾附加
/// `ovr`，以覆蓋綁到斷線的 `wsl` runtime，投影為 `runtime_disconnected`＋`source: override`
/// （fixture 內 `docs` 是同一狀態但 `source: auto`，沒有覆蓋可取消）。供前端驗收「斷線期間可取消改綁」
/// （`DELETE …/workstreams/ovr/override`，`ui_preview` 的寫入替身照舊只記錄並回 204）。
///
/// 放在 project `cockpit` 的最後、不掛任何 task：既有腳本以 `data-workstream` 選 workstream，
/// 也不計 workstream 或 task 數；不新增 project（`factory-floor-check.js` 斷言左欄恰好兩個）。
fn apply_override_disconnected_scenario(state: &mut ProjectedState) {
    let project = state
        .projects
        .iter_mut()
        .find(|p| p.id.as_str() == "cockpit")
        .expect("fixture 缺少 project cockpit");
    project.workstreams.push(ProjectedWorkstream {
        id: WorkstreamId::new("ovr"),
        name: "Override".to_string(),
        binding: ProjectedBinding::RuntimeDisconnected {
            runtime: RuntimeId::new("wsl"),
            source: BindingSource::Override,
        },
        active_task: None,
        activity_undeclared: false,
    });
}

/// `COCKPIT_PREVIEW_PUSH_MS`（正整數毫秒）→ 推送間隔；未設定為 2 秒。
fn push_interval() -> anyhow::Result<Duration> {
    match env::var("COCKPIT_PREVIEW_PUSH_MS") {
        Err(_) => Ok(Duration::from_millis(DEFAULT_PUSH_MS)),
        Ok(raw) => {
            let ms: u64 = raw.parse().with_context(|| {
                format!("COCKPIT_PREVIEW_PUSH_MS 必須是正整數毫秒，實際 {raw:?}")
            })?;
            anyhow::ensure!(ms > 0, "COCKPIT_PREVIEW_PUSH_MS 必須大於 0");
            Ok(Duration::from_millis(ms))
        }
    }
}

/// `COCKPIT_PREVIEW_WRITE_RULES` 的一條規則：路徑完全相符時延遲 `delay` 後回 `status`。
struct WriteRule {
    path: String,
    delay: Duration,
    status: StatusCode,
}

/// 解析 `COCKPIT_PREVIEW_WRITE_RULES`（格式見檔頭）；未設定為空。
fn write_rules() -> anyhow::Result<Vec<WriteRule>> {
    let Ok(raw) = env::var("COCKPIT_PREVIEW_WRITE_RULES") else {
        return Ok(Vec::new());
    };
    raw.split(';')
        .filter(|entry| !entry.trim().is_empty())
        .map(|entry| {
            let (path, spec) = entry
                .trim()
                .rsplit_once('=')
                .with_context(|| format!("規則缺 `=`：{entry:?}"))?;
            let (delay_ms, status) = spec
                .split_once(':')
                .with_context(|| format!("規則缺 `:`：{entry:?}"))?;
            Ok(WriteRule {
                path: path.to_string(),
                delay: Duration::from_millis(
                    delay_ms
                        .parse()
                        .with_context(|| format!("延遲不是整數毫秒：{entry:?}"))?,
                ),
                status: StatusCode::from_u16(
                    status
                        .parse()
                        .with_context(|| format!("狀態碼不是整數：{entry:?}"))?,
                )
                .with_context(|| format!("狀態碼不合法：{entry:?}"))?,
            })
        })
        .collect()
}

/// 寫入端點的替身：記錄請求（stdout 一行 `write-request <METHOD> <PATH> <BODY>`），不改投影
/// ——畫面收到的新投影仍只來自推送迴圈（task 5.3）。預設立即回 204；路徑符合
/// `COCKPIT_PREVIEW_WRITE_RULES` 時延遲後回指定狀態碼（fix round 1）。
async fn record_write_request(
    State(rules): State<Arc<Vec<WriteRule>>>,
    method: Method,
    uri: Uri,
    body: Bytes,
) -> Response {
    println!(
        "write-request {method} {} {}",
        uri.path(),
        String::from_utf8_lossy(&body)
    );
    let Some(rule) = rules.iter().find(|rule| rule.path == uri.path()) else {
        return StatusCode::NO_CONTENT.into_response();
    };
    tokio::time::sleep(rule.delay).await;
    if rule.status.is_success() {
        return rule.status.into_response();
    }
    let body =
        serde_json::json!({ "error": format!("ui_preview 模擬回應 {}", rule.status.as_u16()) })
            .to_string();
    (
        rule.status,
        [(header::CONTENT_TYPE, "application/json")],
        body,
    )
        .into_response()
}

// ---------------------------------------------------------------------------
// live-output task 5.1：輸出讀取端點的假 AgentRuntime
// ---------------------------------------------------------------------------

/// 單一 pane 的腳本化輸出行為（brief 逐字列出的六種）。
#[derive(Debug)]
enum OutputMode {
    /// 內容隨時間增加：起始行數 `start_lines`（`ticker` 為 0、`long` 為 300），之後每秒多一
    /// 行；回應前先等待 `delay`（`ticker`／`long` 為 0，`delay:<ms>` 用指定值）。行內容固定
    /// 為 `line <N>`，讓驗收腳本能斷言「N 秒內出現新行」。
    Growing { start_lines: u64, delay: Duration },
    /// 前 `remaining` 次呼叫回 `RuntimeError::Unavailable`（→ 503），之後恢復成起始行數 0、
    /// 無延遲的 `Growing`。用 `AtomicU32` 是因為 `AgentRuntime::read_output` 只拿 `&self`。
    FailThenRecover { remaining: AtomicU32 },
    /// 一律 `RuntimeError::PaneNotFound`（→ 404）。
    NotFound,
    /// 固定回 [`HTML_PROBE_TEXT`]，內容不隨時間變化。
    Html,
}

/// 假 `AgentRuntime`：`snapshot`／`subscribe` 不會被呼叫到（ui_preview 不經
/// `cockpit_core::driver::run`），`read_output` 依 pane id 查 `panes` 決定行為。
struct FakeOutputRuntime {
    id: RuntimeId,
    /// 供 [`OutputMode::Growing`] 算「已經過幾秒」的起點；整個 runtime 只有一個，讓不同 pane
    /// 的內容成長速度可比較（例如 `ticker` 與 `long` 在同一秒數下的行數差固定是
    /// `start_lines`）。
    started: Instant,
    panes: HashMap<PaneId, OutputMode>,
}

impl FakeOutputRuntime {
    fn new(id: &str, panes: HashMap<PaneId, OutputMode>) -> Self {
        Self {
            id: RuntimeId::new(id),
            started: Instant::now(),
            panes,
        }
    }
}

#[async_trait]
impl AgentRuntime for FakeOutputRuntime {
    fn id(&self) -> &RuntimeId {
        &self.id
    }

    async fn snapshot(&self) -> Result<RuntimeSnapshot, RuntimeError> {
        unimplemented!("ui_preview 不經 driver，這個假 runtime 只服務輸出讀取端點")
    }

    async fn subscribe(&self) -> Result<RuntimeEvents, RuntimeError> {
        unimplemented!("ui_preview 不經 driver，這個假 runtime 只服務輸出讀取端點")
    }

    async fn read_output(&self, pane: &PaneId, max_lines: u32) -> Result<PaneOutput, RuntimeError> {
        // brief：每次請求在 stdout 印一行；被 source_check 擋下的 403 請求不會走到這裡，自然
        // 不印。沒設定過模式的 pane（包含真的不存在的 pane id）視同 PaneNotFound。
        println!("output-request {} {}", self.id, pane);
        let Some(mode) = self.panes.get(pane) else {
            return Err(RuntimeError::PaneNotFound {
                pane_id: pane.clone(),
            });
        };
        match mode {
            OutputMode::NotFound => Err(RuntimeError::PaneNotFound {
                pane_id: pane.clone(),
            }),
            OutputMode::Html => Ok(PaneOutput {
                format: OutputFormat::Text,
                text: HTML_PROBE_TEXT.to_string(),
                truncated: false,
            }),
            OutputMode::Growing { start_lines, delay } => {
                if !delay.is_zero() {
                    tokio::time::sleep(*delay).await;
                }
                Ok(growing_output(*start_lines, self.started, max_lines))
            }
            OutputMode::FailThenRecover { remaining } => {
                let previous = remaining
                    .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |r| {
                        Some(r.saturating_sub(1))
                    })
                    .expect("closure 一律回傳 Some，fetch_update 不會失敗");
                if previous > 0 {
                    return Err(RuntimeError::Unavailable {
                        reason: format!("ui_preview 模擬斷線（還剩 {previous} 次恢復前）"),
                        retry_after: Duration::from_millis(500),
                    });
                }
                Ok(growing_output(0, self.started, max_lines))
            }
        }
    }
}

/// [`OutputMode::Growing`] 的內容產生：`started` 起算的秒數決定目前總行數
/// （`start_lines + elapsed_secs + 1`，讓 `elapsed == 0` 時就已經有第一行），回應只取最後
/// `max_lines` 行，`truncated` 為總行數是否超過 `max_lines`。
fn growing_output(start_lines: u64, started: Instant, max_lines: u32) -> PaneOutput {
    let elapsed_secs = started.elapsed().as_secs();
    let total_lines = start_lines + elapsed_secs + 1;
    let max_lines = u64::from(max_lines);
    let first_line = total_lines.saturating_sub(max_lines) + 1;
    let text = (first_line..=total_lines)
        .map(|n| format!("line {n}"))
        .collect::<Vec<_>>()
        .join("\n");
    PaneOutput {
        format: OutputFormat::Text,
        text,
        truncated: total_lines > max_lines,
    }
}

/// 沒有 `COCKPIT_PREVIEW_OUTPUT_MODES` 覆寫時的預設分配（檔頭文件節錄）：`wJ:p1` 為
/// `ticker`、`wJ:p3` 為 `long`、`wJ:p2`（`exited`）為 `notfound`；file-review 的兩個 fixture pane
/// `wJ:p4`、`wJ:p5` 為 `ticker`（file-review task 4.2：它們可被選定，沒有模式就一律 404，Live Output
/// 會立即判「pane 已不存在」並清掉選取，檔案樹跟著回到空狀態）。
fn default_output_modes() -> HashMap<PaneId, OutputMode> {
    let mut modes = HashMap::new();
    for pane in ["wJ:p1", "wJ:p4", "wJ:p5"] {
        modes.insert(
            PaneId::new(pane),
            OutputMode::Growing {
                start_lines: 0,
                delay: Duration::ZERO,
            },
        );
    }
    modes.insert(
        PaneId::new("wJ:p3"),
        OutputMode::Growing {
            start_lines: 300,
            delay: Duration::ZERO,
        },
    );
    modes.insert(PaneId::new("wJ:p2"), OutputMode::NotFound);
    modes
}

/// 解析 `COCKPIT_PREVIEW_OUTPUT_MODES`（格式見檔頭）：以 [`default_output_modes`] 為底，逐條
/// 規則覆寫或新增。未設定時原樣回傳預設值。
fn parse_output_modes(raw: Option<&str>) -> anyhow::Result<HashMap<PaneId, OutputMode>> {
    let mut modes = default_output_modes();
    let Some(raw) = raw else {
        return Ok(modes);
    };
    for entry in raw
        .split(';')
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
    {
        let (pane, mode_spec) = entry
            .split_once('=')
            .with_context(|| format!("COCKPIT_PREVIEW_OUTPUT_MODES 規則缺 `=`：{entry:?}"))?;
        let mode = parse_output_mode(mode_spec)
            .with_context(|| format!("COCKPIT_PREVIEW_OUTPUT_MODES 規則不合法：{entry:?}"))?;
        modes.insert(PaneId::new(pane), mode);
    }
    Ok(modes)
}

/// 解析單一模式字串（`<模式>` 或 `<模式>:<參數>`），[`parse_output_modes`] 拆出 `<pane>=` 之後
/// 剩下的部分。
fn parse_output_mode(spec: &str) -> anyhow::Result<OutputMode> {
    match spec.split_once(':') {
        Some(("delay", ms)) => Ok(OutputMode::Growing {
            start_lines: 0,
            delay: Duration::from_millis(
                ms.parse()
                    .with_context(|| format!("delay 的毫秒數不是整數：{ms:?}"))?,
            ),
        }),
        Some(("fail", count)) => Ok(OutputMode::FailThenRecover {
            remaining: AtomicU32::new(
                count
                    .parse()
                    .with_context(|| format!("fail 的次數不是整數：{count:?}"))?,
            ),
        }),
        Some((other, _)) => anyhow::bail!("不認識的模式：{other:?}"),
        None => match spec {
            "ticker" => Ok(OutputMode::Growing {
                start_lines: 0,
                delay: Duration::ZERO,
            }),
            "long" => Ok(OutputMode::Growing {
                start_lines: 300,
                delay: Duration::ZERO,
            }),
            "notfound" => Ok(OutputMode::NotFound),
            "html" => Ok(OutputMode::Html),
            other => anyhow::bail!("不認識的模式：{other:?}"),
        },
    }
}

// ---------------------------------------------------------------------------
// live-output task 5.1：pane 消失（spec「失敗與消失的呈現」Scenario「pane 被關掉」）
// ---------------------------------------------------------------------------

/// `COCKPIT_PREVIEW_VANISH_PANE` 的設定：`pane` 在推送迴圈經過 `after` 之後，從之後每一份
/// 推送的投影中被拿掉。
#[derive(Debug)]
struct VanishConfig {
    pane: PaneId,
    after: Duration,
}

/// 解析 `COCKPIT_PREVIEW_VANISH_PANE=<pane>=<毫秒>`（例如
/// `COCKPIT_PREVIEW_VANISH_PANE=wJ:p1=5000`）；未設定為 `None`。`pane` id 本身可能含冒號
/// （fixture 的 pane id 都是 `wJ:p1` 這種形式），所以用第一個 `=` 切，不能用 `:` 切。
fn parse_vanish_pane(raw: Option<&str>) -> anyhow::Result<Option<VanishConfig>> {
    Ok(parse_pane_after("COCKPIT_PREVIEW_VANISH_PANE", raw)?
        .map(|(pane, after)| VanishConfig { pane, after }))
}

/// `<pane>=<毫秒>` 形式的環境變數（`COCKPIT_PREVIEW_VANISH_PANE`、[`CWD_MOVE_VAR`]）共用的解析；
/// `var` 只用在錯誤訊息。未設定或空字串為 `None`。
fn parse_pane_after(var: &str, raw: Option<&str>) -> anyhow::Result<Option<(PaneId, Duration)>> {
    let Some(raw) = raw else {
        return Ok(None);
    };
    let raw = raw.trim();
    if raw.is_empty() {
        return Ok(None);
    }
    let (pane, after_ms) = raw
        .split_once('=')
        .with_context(|| format!("{var} 缺 `=`：{raw:?}"))?;
    let after = Duration::from_millis(
        after_ms
            .parse()
            .with_context(|| format!("{var} 的毫秒數不是整數：{raw:?}"))?,
    );
    Ok(Some((PaneId::new(pane), after)))
}

// ---------------------------------------------------------------------------
// file-review 最終修正波 F1：同一個 pane 的 cwd 改到另一個 repo
// ---------------------------------------------------------------------------

/// 設定這個環境變數時，指定的 pane 經過指定時間後 `cwd` 改成 `other-repo`（見檔頭）。
const CWD_MOVE_VAR: &str = "COCKPIT_PREVIEW_CWD_TO_OTHER_REPO";

/// [`CWD_MOVE_VAR`] 的設定：`pane` 在推送迴圈經過 `after` 之後，`cwd` 改成 `to`。
#[derive(Debug)]
struct CwdMove {
    pane: PaneId,
    after: Duration,
    to: String,
}

/// 把 `state` 裡 id 為 `target` 的 pane 的 `cwd` 改成 `cwd`（[`CwdMove`] 用）。
fn set_pane_cwd(state: &mut ProjectedState, target: &PaneId, cwd: &str) {
    for runtime in &mut state.runtimes {
        for workspace in &mut runtime.workspaces {
            for tab in &mut workspace.tabs {
                for pane in &mut tab.panes {
                    if &pane.id == target {
                        pane.cwd = Some(cwd.to_string());
                    }
                }
            }
        }
    }
}

/// 蒐集 `state` 裡所有 runtime／workspace／tab 底下出現過的 pane id（R14：驗證
/// `COCKPIT_PREVIEW_OUTPUT_MODES`／`COCKPIT_PREVIEW_VANISH_PANE` 有沒有指到打錯的 pane id）。
fn known_pane_ids(state: &ProjectedState) -> HashSet<PaneId> {
    state
        .runtimes
        .iter()
        .flat_map(|runtime| &runtime.workspaces)
        .flat_map(|workspace| &workspace.tabs)
        .flat_map(|tab| &tab.panes)
        .map(|pane| pane.id.clone())
        .collect()
}

/// R14：`output_modes`（`COCKPIT_PREVIEW_OUTPUT_MODES` 解析結果，含預設值）與 `vanish`
/// （`COCKPIT_PREVIEW_VANISH_PANE` 解析結果）提到的每個 pane id 都必須在 `known` 之中，否則
/// 回清楚的錯誤（列出打錯的 pane id）。預設值本身一定在 fixture 裡，只有使用者自己疊加的
/// 規則才可能打錯。
fn validate_known_panes(
    output_modes: &HashMap<PaneId, OutputMode>,
    vanish: &Option<VanishConfig>,
    known: &HashSet<PaneId>,
) -> anyhow::Result<()> {
    let mut unknown: Vec<&PaneId> = output_modes
        .keys()
        .filter(|pane| !known.contains(*pane))
        .collect();
    unknown.sort();
    anyhow::ensure!(
        unknown.is_empty(),
        "COCKPIT_PREVIEW_OUTPUT_MODES 指到 fixture 投影裡不存在的 pane id：{}",
        unknown
            .iter()
            .map(|pane| pane.to_string())
            .collect::<Vec<_>>()
            .join(", ")
    );
    if let Some(vanish) = vanish {
        anyhow::ensure!(
            known.contains(&vanish.pane),
            "COCKPIT_PREVIEW_VANISH_PANE 指到 fixture 投影裡不存在的 pane id：{}",
            vanish.pane
        );
    }
    Ok(())
}

/// 把 `target` 從 `state` 所有 runtime／workspace／tab 的 pane 清單中移除（[`VanishConfig`]
/// 用：模擬 pane 被關掉後從投影消失）。
fn remove_pane(state: &mut ProjectedState, target: &PaneId) {
    for runtime in &mut state.runtimes {
        for workspace in &mut runtime.workspaces {
            for tab in &mut workspace.tabs {
                tab.panes.retain(|pane| &pane.id != target);
            }
        }
    }
}

/// 每個推送間隔把第一個 runtime、第一個 workspace、第一個 tab、第一個 pane 的 `agent_status`
/// 在 working／idle／blocked 之間輪替，`version` 遞增、`generated_at` 更新為現在時間，讓
/// 連著的瀏覽器（`/ws`）與下一次 `/api/state` 都看得到變化（design D14）。`vanish` 有設定時，
/// 經過指定時間後把該 pane 從投影拿掉一次（task 5.1）。
async fn push_loop(
    tx: watch::Sender<Arc<ProjectedState>>,
    every: Duration,
    vanish: Option<VanishConfig>,
    cwd_move: Option<CwdMove>,
    started: Instant,
) {
    const CYCLE: [AgentStatus; 3] = [
        AgentStatus::Working,
        AgentStatus::Idle,
        AgentStatus::Blocked,
    ];
    let mut index = 0usize;
    let mut vanished = false;
    let mut moved = false;
    // `tokio::time::interval` 的第一次 `tick()` 會立即完成（design 沒特別要求，但
    // fix round 1 finding：這會讓背景 task 一啟動就把 fixture 的 version/狀態改掉，
    // 使剛啟動的 `/api/state` 看不到 fixture 原始值）。改用 `interval_at` 把第一個
    // tick 排在「現在 + 一個間隔」，讓啟動當下到第一次真的輪替之間有完整的一個間隔空窗，
    // `/api/state` 才能如預期在這段時間內看到 fixture 原封不動的內容。
    let mut ticker = interval_at(Instant::now() + every, every);

    loop {
        ticker.tick().await;
        let mut next = (**tx.borrow()).clone();

        if let Some(vanish) = &vanish
            && !vanished
            && started.elapsed() >= vanish.after
        {
            remove_pane(&mut next, &vanish.pane);
            vanished = true;
        }

        if let Some(cwd_move) = &cwd_move
            && !moved
            && started.elapsed() >= cwd_move.after
        {
            set_pane_cwd(&mut next, &cwd_move.pane, &cwd_move.to);
            moved = true;
        }

        index = (index + 1) % CYCLE.len();

        let updated = next
            .runtimes
            .first_mut()
            .and_then(|runtime| runtime.workspaces.first_mut())
            .and_then(|workspace| workspace.tabs.first_mut())
            .and_then(|tab| tab.panes.first_mut());

        let Some(pane) = updated else {
            // fixture 形狀跑掉（例如被改成沒有任何 pane）：沒東西可輪替，結束這個任務，
            // 但不影響 server 本身繼續服務目前這一份投影。
            break;
        };

        pane.agent_status = CYCLE[index];
        next.version += 1;
        next.generated_at = to_rfc3339(SystemTime::now());

        if tx.send(Arc::new(next)).is_err() {
            // 沒有任何 receiver 了（server 已經停了），沒必要再繼續輪替。
            break;
        }
    }
}

/// `SystemTime` → RFC 3339（UTC，秒精度，`YYYY-MM-DDTHH:MM:SSZ`），格式對齊
/// `cockpit-core::projection` 內部的同名私有函數。這裡自帶一份最小實作而不引入 `chrono`
/// ——`cockpit/Cargo.toml` 沒有這個依賴，本 task 不改 `Cargo.toml`。
fn to_rfc3339(time: SystemTime) -> String {
    let secs = time
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let days = secs.div_euclid(86_400);
    let secs_of_day = secs.rem_euclid(86_400);
    let hour = secs_of_day / 3600;
    let minute = (secs_of_day % 3600) / 60;
    let second = secs_of_day % 60;
    let (year, month, day) = civil_from_days(days);
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

/// Howard Hinnant 的 `civil_from_days` 演算法（公開演算法，非本專案原創）：把「自
/// 1970-01-01 起的天數」換算成公曆年月日。
fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if month <= 2 { y + 1 } else { y };
    (year, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `parse_output_mode`／`parse_output_modes` 覆蓋 brief 的六種模式與「規則不合法」路徑。

    #[test]
    fn parse_output_modes_none_returns_defaults() {
        let modes = parse_output_modes(None).expect("未設定應該成功回傳預設值");
        assert_eq!(modes.len(), 5);
        // file-review task 4.2：檔案樹的兩個 fixture pane 可被選定，輸出端點不能回 404（否則
        // Live Output 立即判「pane 已不存在」、清掉選取，檔案樹也跟著回到空狀態）。
        for pane in ["wJ:p4", "wJ:p5"] {
            assert!(
                matches!(
                    modes.get(&PaneId::new(pane)),
                    Some(OutputMode::Growing {
                        start_lines: 0,
                        delay
                    }) if delay.is_zero()
                ),
                "{pane} 預設應為 ticker"
            );
        }
        assert!(matches!(
            modes.get(&PaneId::new("wJ:p1")),
            Some(OutputMode::Growing {
                start_lines: 0,
                delay
            }) if delay.is_zero()
        ));
        assert!(matches!(
            modes.get(&PaneId::new("wJ:p3")),
            Some(OutputMode::Growing {
                start_lines: 300,
                delay
            }) if delay.is_zero()
        ));
        assert!(matches!(
            modes.get(&PaneId::new("wJ:p2")),
            Some(OutputMode::NotFound)
        ));
    }

    #[test]
    fn parse_output_modes_overrides_and_adds() {
        let modes = parse_output_modes(Some("wJ:p1=delay:3000;wJ:p3=fail:2;wJ:p2=html"))
            .expect("合法規則應該解析成功");
        assert!(matches!(
            modes.get(&PaneId::new("wJ:p1")),
            Some(OutputMode::Growing { start_lines: 0, delay })
            if *delay == Duration::from_millis(3000)
        ));
        match modes.get(&PaneId::new("wJ:p3")) {
            Some(OutputMode::FailThenRecover { remaining }) => {
                assert_eq!(remaining.load(Ordering::SeqCst), 2);
            }
            other => panic!("預期 FailThenRecover，實際：{other:?}"),
        }
        assert!(matches!(
            modes.get(&PaneId::new("wJ:p2")),
            Some(OutputMode::Html)
        ));
    }

    #[test]
    fn parse_output_modes_rejects_unknown_mode() {
        let error = parse_output_modes(Some("wJ:p1=bogus")).expect_err("不認識的模式應該回錯");
        assert!(format!("{error:#}").contains("不認識的模式"));
    }

    #[test]
    fn parse_output_modes_rejects_missing_equals() {
        let error = parse_output_modes(Some("wJ:p1")).expect_err("缺 `=` 應該回錯");
        assert!(format!("{error:#}").contains("缺"));
    }

    #[test]
    fn parse_vanish_pane_none_when_unset() {
        assert!(parse_vanish_pane(None).expect("None 應該成功").is_none());
    }

    #[test]
    fn parse_vanish_pane_parses_colon_pane_id() {
        let config = parse_vanish_pane(Some("wJ:p1=5000"))
            .expect("合法設定應該解析成功")
            .expect("應該回傳 Some");
        assert_eq!(config.pane, PaneId::new("wJ:p1"));
        assert_eq!(config.after, Duration::from_millis(5000));
    }

    #[test]
    fn parse_vanish_pane_rejects_non_integer_ms() {
        let error = parse_vanish_pane(Some("wJ:p1=soon")).expect_err("非整數毫秒應該回錯");
        assert!(format!("{error:#}").contains("不是整數"));
    }

    /// R14 補的測試（brief 逐字要求的四種）：`delay:<非數字>`、`fail:<非數字>`、
    /// `VANISH_PANE` 缺 `=`，以及「兩個環境變數指到 fixture 裡不存在的 pane id」。

    #[test]
    fn parse_output_mode_rejects_non_integer_delay() {
        let error = parse_output_modes(Some("wJ:p1=delay:abc")).expect_err("delay 非數字應該回錯");
        assert!(format!("{error:#}").contains("不是整數"));
    }

    #[test]
    fn parse_output_mode_rejects_non_integer_fail_count() {
        let error = parse_output_modes(Some("wJ:p1=fail:abc")).expect_err("fail 非數字應該回錯");
        assert!(format!("{error:#}").contains("不是整數"));
    }

    #[test]
    fn parse_vanish_pane_rejects_missing_equals() {
        let error = parse_vanish_pane(Some("wJ:p1")).expect_err("缺 `=` 應該回錯");
        assert!(format!("{error:#}").contains("缺"));
    }

    #[test]
    fn parse_pane_after_names_the_variable_in_errors() {
        let error = parse_pane_after(CWD_MOVE_VAR, Some("wJ:p4")).expect_err("缺 `=` 應該回錯");
        assert!(format!("{error:#}").contains(CWD_MOVE_VAR));
        let (pane, after) = parse_pane_after(CWD_MOVE_VAR, Some("wJ:p4=6000"))
            .expect("合法設定應該解析成功")
            .expect("應該回傳 Some");
        assert_eq!(pane, PaneId::new("wJ:p4"));
        assert_eq!(after, Duration::from_millis(6000));
    }

    #[test]
    fn set_pane_cwd_changes_only_the_target_pane() {
        let mut state: ProjectedState =
            serde_json::from_str(FIXTURE).expect("fixture 應該能反序列化");
        let cwds = |state: &ProjectedState| -> Vec<(String, Option<String>)> {
            state
                .runtimes
                .iter()
                .flat_map(|rt| &rt.workspaces)
                .flat_map(|ws| &ws.tabs)
                .flat_map(|tab| &tab.panes)
                .map(|pane| (pane.id.to_string(), pane.cwd.clone()))
                .collect()
        };
        let before = cwds(&state);
        set_pane_cwd(&mut state, &PaneId::new("wJ:p1"), "D:/moved");
        let after = cwds(&state);
        assert_eq!(after.len(), before.len());
        assert!(after.iter().any(|(id, _)| id == "wJ:p1"));
        for ((id, cwd), (_, old)) in after.iter().zip(before.iter()) {
            if id == "wJ:p1" {
                assert_eq!(cwd.as_deref(), Some("D:/moved"));
            } else {
                assert_eq!(cwd, old);
            }
        }
    }

    fn known_ids(ids: &[&str]) -> HashSet<PaneId> {
        ids.iter().map(|id| PaneId::new(*id)).collect()
    }

    #[test]
    fn validate_known_panes_rejects_unknown_output_mode_pane() {
        let known = known_ids(&["wJ:p1", "wJ:p2", "wJ:p3", "wJ:p4", "wJ:p5"]);
        let modes = parse_output_modes(Some("wJ:p9=ticker")).expect("合法規則應該解析成功");
        let error = validate_known_panes(&modes, &None, &known)
            .expect_err("COCKPIT_PREVIEW_OUTPUT_MODES 指到不存在的 pane id 應該回錯");
        let message = format!("{error:#}");
        assert!(
            message.contains("wJ:p9"),
            "訊息應該點名打錯的 pane id：{message}"
        );
        assert!(
            message.contains("COCKPIT_PREVIEW_OUTPUT_MODES"),
            "訊息應該點名是哪個環境變數：{message}"
        );
    }

    #[test]
    fn validate_known_panes_rejects_unknown_vanish_pane() {
        let known = known_ids(&["wJ:p1", "wJ:p2", "wJ:p3", "wJ:p4", "wJ:p5"]);
        let modes = default_output_modes();
        let vanish = parse_vanish_pane(Some("wJ:p9=5000")).expect("合法設定應該解析成功");
        let error = validate_known_panes(&modes, &vanish, &known)
            .expect_err("COCKPIT_PREVIEW_VANISH_PANE 指到不存在的 pane id 應該回錯");
        let message = format!("{error:#}");
        assert!(
            message.contains("wJ:p9"),
            "訊息應該點名打錯的 pane id：{message}"
        );
        assert!(
            message.contains("COCKPIT_PREVIEW_VANISH_PANE"),
            "訊息應該點名是哪個環境變數：{message}"
        );
    }

    #[test]
    fn validate_known_panes_accepts_defaults() {
        let known = known_ids(&["wJ:p1", "wJ:p2", "wJ:p3", "wJ:p4", "wJ:p5"]);
        let modes = default_output_modes();
        assert!(validate_known_panes(&modes, &None, &known).is_ok());
    }

    // -----------------------------------------------------------------------
    // file-review task 3.4：假 repo fixture
    // -----------------------------------------------------------------------

    /// 測試專用的暫存目錄，析構時自己清掉——沒有引入 `tempfile` 這個 crate（brief：本 task
    /// 不改 `Cargo.toml`），用行程 id＋單調遞增的計數器組唯一名稱，避免同一支測試行程內多個
    /// 測試互相碰撞。
    struct TestTempDir(PathBuf);

    impl TestTempDir {
        fn new(label: &str) -> Self {
            static COUNTER: AtomicU32 = AtomicU32::new(0);
            let n = COUNTER.fetch_add(1, Ordering::SeqCst);
            let dir = env::temp_dir().join(format!(
                "cockpit-ui-preview-test-{label}-{}-{n}",
                std::process::id()
            ));
            fs::create_dir_all(&dir).expect("建立測試暫存目錄失敗");
            Self(dir)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TestTempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn copy_dir_recursive_copies_nested_files_and_binary_content() {
        let root = TestTempDir::new("copy-src-dst");
        let src = root.path().join("src");
        let dst = root.path().join("dst");
        fs::create_dir_all(src.join("nested")).expect("建立來源巢狀資料夾");
        fs::write(src.join("top.txt"), "top level\n").expect("寫入來源頂層檔案");
        fs::write(src.join("nested").join("deep.txt"), "nested level\n").expect("寫入來源巢狀檔案");
        // 含 NUL 位元組的二進位內容（同 `bin.dat` fixture 的性質）：複製必須逐位元組保真，
        // 不能被當成文字處理而在 NUL 處截斷或轉換。
        fs::write(src.join("bin.dat"), b"AB\x00CD").expect("寫入來源二進位檔案");

        copy_dir_recursive(&src, &dst).expect("複製應該成功");

        assert_eq!(
            fs::read_to_string(dst.join("top.txt")).expect("讀取複製後的頂層檔案"),
            "top level\n"
        );
        assert_eq!(
            fs::read_to_string(dst.join("nested").join("deep.txt")).expect("讀取複製後的巢狀檔案"),
            "nested level\n"
        );
        assert_eq!(
            fs::read(dst.join("bin.dat")).expect("讀取複製後的二進位檔案"),
            b"AB\x00CD"
        );
    }

    #[test]
    fn copy_dir_recursive_creates_destination_when_missing() {
        let root = TestTempDir::new("copy-missing-dst");
        let src = root.path().join("src");
        fs::create_dir_all(&src).expect("建立來源資料夾");
        fs::write(src.join("a.txt"), "a").expect("寫入來源檔案");
        let dst = root
            .path()
            .join("does")
            .join("not")
            .join("exist")
            .join("yet");

        copy_dir_recursive(&src, &dst).expect("複製應該建立多層不存在的目的地");

        assert_eq!(
            fs::read_to_string(dst.join("a.txt")).expect("讀取檔案"),
            "a"
        );
    }

    // file-review task 3.4 fix round 2：`parse_fixture_dir_pid`／`should_delete_stale_dir` 是
    // 純函式，直接測；`cleanup_stale_fixtures_with` 用注入的假存活判斷測（不依賴這台機器上真正
    // 在跑的行程，結果才是確定的）。`cleanup_stale_fixtures_under`（接到真正的 `pid_is_alive`）
    // 不在這裡單元測——那是「呼叫真正的 tasklist」這件事本身，留給控制端裁決的實機驗收。

    #[test]
    fn parse_fixture_dir_pid_extracts_pid_from_expected_shape() {
        assert_eq!(
            parse_fixture_dir_pid("cockpit-ui-preview-1234-999999999"),
            Some(1234)
        );
    }

    #[test]
    fn parse_fixture_dir_pid_rejects_wrong_prefix() {
        assert_eq!(parse_fixture_dir_pid("unrelated-dir-1234-999"), None);
    }

    #[test]
    fn parse_fixture_dir_pid_rejects_missing_timestamp_segment() {
        // 沒有 `-<timestamp>` 那一段：`split('-').next()` 仍然拿得到東西，但那其實是整個剩餘
        // 字串——只要那段本身剛好是合法數字就會被解析出來，這裡刻意用不是數字的殘留字串測「解不
        // 出來」那條路徑。
        assert_eq!(parse_fixture_dir_pid("cockpit-ui-preview-notanumber"), None);
    }

    #[test]
    fn parse_fixture_dir_pid_rejects_non_numeric_pid_segment() {
        assert_eq!(parse_fixture_dir_pid("cockpit-ui-preview-abc-999"), None);
    }

    #[test]
    fn should_delete_stale_dir_true_when_pid_confirmed_dead() {
        assert!(should_delete_stale_dir(
            "cockpit-ui-preview-1234-999",
            |_| Some(false)
        ));
    }

    #[test]
    fn should_delete_stale_dir_false_when_pid_alive() {
        assert!(!should_delete_stale_dir(
            "cockpit-ui-preview-1234-999",
            |_| Some(true)
        ));
    }

    #[test]
    fn should_delete_stale_dir_false_when_liveness_unknown() {
        // 查不出來（例如 tasklist 執行失敗）：保留優先於刪除，不是「反正大概率沒人用就刪了」。
        assert!(!should_delete_stale_dir(
            "cockpit-ui-preview-1234-999",
            |_| None
        ));
    }

    #[test]
    fn should_delete_stale_dir_false_when_name_unparseable() {
        // 解不出 PID：同樣保留，不能因為「名字看起來像我們的東西」就刪。
        assert!(!should_delete_stale_dir("cockpit-ui-preview-oops", |_| {
            Some(false)
        }));
    }

    #[test]
    fn cleanup_stale_fixtures_with_removes_dead_pid_dir_keeps_others() {
        let scratch = TestTempDir::new("cleanup-scratch-dead");
        let dead = scratch
            .path()
            .join(format!("{REVIEW_FIXTURE_PREFIX}1234-456"));
        fs::create_dir_all(&dead).expect("建立模擬的殘留暫存目錄（PID 已死）");
        fs::write(dead.join("review-repo-marker.txt"), "stale").expect("寫入殘留目錄內的檔案");
        let unrelated = scratch.path().join("unrelated-dir");
        fs::create_dir_all(&unrelated).expect("建立不相干的目錄");

        cleanup_stale_fixtures_with(scratch.path(), |_| Some(false));

        assert!(!dead.exists(), "PID 已確認不存在的殘留目錄應該被刪除");
        assert!(unrelated.exists(), "不帶前綴的目錄不應該被動到");
    }

    #[test]
    fn cleanup_stale_fixtures_with_keeps_dir_when_pid_still_alive() {
        let scratch = TestTempDir::new("cleanup-scratch-alive");
        let in_use = scratch
            .path()
            .join(format!("{REVIEW_FIXTURE_PREFIX}1234-456"));
        fs::create_dir_all(&in_use).expect("建立模擬的、仍在使用中的暫存目錄");
        fs::write(in_use.join("review-repo-marker.txt"), "still in use")
            .expect("寫入使用中目錄內的檔案");

        // file-review task 3.4 fix round 2 finding 的核心場景：目錄名稱前綴相符，但擁有它的
        // 行程還活著——不能只憑前綴就刪。
        cleanup_stale_fixtures_with(scratch.path(), |_| Some(true));

        assert!(
            in_use.exists(),
            "PID 仍存活的目錄不應該被刪除（Codex fix round 2 finding）"
        );
        assert!(
            in_use.join("review-repo-marker.txt").exists(),
            "目錄內的檔案也應該完好保留"
        );
    }

    #[test]
    fn cleanup_stale_fixtures_with_keeps_dir_when_liveness_unknown() {
        let scratch = TestTempDir::new("cleanup-scratch-unknown");
        let unknown = scratch
            .path()
            .join(format!("{REVIEW_FIXTURE_PREFIX}1234-456"));
        fs::create_dir_all(&unknown).expect("建立模擬的暫存目錄");

        cleanup_stale_fixtures_with(scratch.path(), |_| None);

        assert!(unknown.exists(), "查不出存活與否時應該保留不刪");
    }

    #[test]
    fn cleanup_stale_fixtures_with_ignores_missing_directory() {
        let root = TestTempDir::new("cleanup-missing-parent");
        let missing = root.path().join("does-not-exist");
        // 不應該 panic；`fs::read_dir` 失敗時直接回傳。
        cleanup_stale_fixtures_with(&missing, |_| Some(false));
    }

    #[test]
    fn apply_progress_scenarios_sets_active_tasks_and_adds_undeclared_workstream() {
        let mut state: ProjectedState =
            serde_json::from_str(FIXTURE).expect("fixture 應該能反序列化");
        let before = state.projects.len();
        apply_progress_scenarios(&mut state);

        // 既有 running／blocked 的 task 都是其 workstream 的目前 task。
        for project in &state.projects[..before] {
            for task in project
                .tasks
                .iter()
                .filter(|t| matches!(t.status, StageStatus::Running | StageStatus::Blocked))
            {
                let ws = project
                    .workstreams
                    .iter()
                    .find(|w| w.id == task.workstream)
                    .expect("task 的 workstream 存在");
                assert_eq!(ws.active_task.as_ref(), Some(&task.id), "{}", task.id);
            }
        }

        let p = state
            .projects
            .iter()
            .find(|p| p.id.as_str() == "p")
            .unwrap();
        let ws = p.workstreams.last().unwrap();
        assert_eq!(ws.id.as_str(), "undeclared");
        assert!(ws.activity_undeclared && ws.active_task.is_none());
        let task = p.tasks.last().unwrap();
        assert_eq!(task.mark, Mark::None);
        assert_ne!(task.stage, p.stages[0], "不在第一站");
        assert_eq!(state.projects.len(), before, "不新增 project");
    }

    #[test]
    fn apply_override_disconnected_scenario_adds_override_binding_to_disconnected_runtime() {
        let mut state: ProjectedState =
            serde_json::from_str(FIXTURE).expect("fixture 應該能反序列化");
        let projects_before = state.projects.len();
        let cockpit_before = state.projects[0].workstreams.len();
        let ids_before: Vec<String> = state.projects[0]
            .workstreams
            .iter()
            .map(|w| w.id.as_str().to_string())
            .collect();
        apply_override_disconnected_scenario(&mut state);

        assert_eq!(state.projects.len(), projects_before, "不新增 project");
        let project = &state.projects[0];
        assert_eq!(project.id.as_str(), "cockpit");
        assert_eq!(project.workstreams.len(), cockpit_before + 1);
        // 既有 workstream 的順序不變，新的接在最後（不打亂既有腳本的選擇器）。
        let ids_after: Vec<&str> = project.workstreams.iter().map(|w| w.id.as_str()).collect();
        assert_eq!(&ids_after[..cockpit_before], ids_before.as_slice());
        let ovr = project.workstreams.last().unwrap();
        assert_eq!(ovr.id.as_str(), "ovr");
        assert_eq!(
            ovr.binding,
            ProjectedBinding::RuntimeDisconnected {
                runtime: RuntimeId::new("wsl"),
                source: BindingSource::Override,
            }
        );
        // 目標 runtime 確實是 disconnected，情境才自洽。
        let wsl = state
            .runtimes
            .iter()
            .find(|r| r.id.as_str() == "wsl")
            .expect("fixture 有 wsl runtime");
        assert!(matches!(
            wsl.connection,
            cockpit_core::ProjectedConnection::Disconnected { .. }
        ));
    }

    #[test]
    fn add_review_fixture_panes_adds_new_tab_without_touching_existing_panes() {
        let mut state: ProjectedState =
            serde_json::from_str(FIXTURE).expect("fixture 應該能反序列化");
        let original_workspace = state.runtimes[0].workspaces[0].clone();
        assert_eq!(
            original_workspace.tabs.len(),
            1,
            "前置條件：fixture 一開始只有一個 tab"
        );

        let fixture = ReviewFixture {
            temp_root: PathBuf::from(r"C:\fake\cockpit-ui-preview-test"),
            review_repo: PathBuf::from(r"C:\fake\cockpit-ui-preview-test\review-repo"),
            other_repo: PathBuf::from(r"C:\fake\cockpit-ui-preview-test\other-repo"),
        };
        add_review_fixture_panes(&mut state, &fixture).expect("掛上 fixture pane 應該成功");

        let workspace = &state.runtimes[0].workspaces[0];
        assert_eq!(workspace.tabs.len(), 2, "應該多出一個 tab，既有的 tab 不變");
        assert_eq!(
            workspace.tabs[0], original_workspace.tabs[0],
            "既有 tab（wJ:t1）與其三個 pane 必須逐位元組不變——既有六支驗收腳本對它們有逐字比對"
        );

        let new_tab = &workspace.tabs[1];
        assert_eq!(new_tab.id, TabId::new("wJ:t2"));
        assert_eq!(new_tab.panes.len(), 2);

        let p4 = &new_tab.panes[0];
        assert_eq!(p4.id, PaneId::new("wJ:p4"));
        assert_eq!(
            p4.cwd.as_deref(),
            Some(r"C:\fake\cockpit-ui-preview-test\review-repo\src")
        );
        assert!(
            !p4.exited,
            "新 pane 必須 exited=false，否則之後的瀏覽器驗收腳本無法在畫面上選定它（live-output \
             spec「選定一個 pane」：只有未 exited 的 pane 列可點選）"
        );

        let p5 = &new_tab.panes[1];
        assert_eq!(p5.id, PaneId::new("wJ:p5"));
        assert_eq!(
            p5.cwd.as_deref(),
            Some(r"C:\fake\cockpit-ui-preview-test\other-repo")
        );
        assert!(!p5.exited);
    }

    #[test]
    fn add_review_fixture_panes_errs_when_runtime_missing() {
        let mut state: ProjectedState =
            serde_json::from_str(FIXTURE).expect("fixture 應該能反序列化");
        state.runtimes.clear();
        let fixture = ReviewFixture {
            temp_root: PathBuf::from(r"C:\fake"),
            review_repo: PathBuf::from(r"C:\fake\review-repo"),
            other_repo: PathBuf::from(r"C:\fake\other-repo"),
        };
        let error = add_review_fixture_panes(&mut state, &fixture)
            .expect_err("runtime 或 workspace 不存在時應該回錯，而不是靜默什麼都不做");
        assert!(format!("{error:#}").contains(OUTPUT_RUNTIME_ID));
    }

    // -----------------------------------------------------------------------
    // git-review task 3.1：fast-import stream 產生（純函式，不啟動真正的 git）
    // -----------------------------------------------------------------------

    fn count_occurrences(haystack: &[u8], needle: &[u8]) -> usize {
        if needle.is_empty() || haystack.len() < needle.len() {
            return 0;
        }
        haystack
            .windows(needle.len())
            .filter(|w| *w == needle)
            .count()
    }

    #[test]
    fn git_fixture_date_is_deterministic_and_increasing() {
        assert_eq!(git_fixture_date(0), "1767225600 +0000");
        assert_eq!(git_fixture_date(1), "1767225660 +0000");
        assert_eq!(git_fixture_date(2), "1767225720 +0000");
    }

    #[test]
    fn author_line_combines_fixed_author_and_date() {
        assert_eq!(
            author_line(0),
            "cockpit-ui-preview <ui-preview@cockpit.invalid> 1767225600 +0000"
        );
    }

    #[test]
    fn binary_pattern_is_512_bytes_covers_nul_and_differs_by_seed() {
        let a = binary_pattern(0);
        let b = binary_pattern(1);
        assert_eq!(a.len(), 512);
        assert_eq!(b.len(), 512);
        assert!(a.contains(&0u8), "應涵蓋 0x00 這個位元組值");
        assert_ne!(
            a, b,
            "不同 seed 應產生不同內容，才能區分「新增」與「修改」兩次 commit"
        );
    }

    #[test]
    fn fi_modify_bytes_frames_content_by_exact_byte_count() {
        let mut buf = Vec::new();
        fi_modify_bytes(&mut buf, "a/b.bin", &[0u8, 1, 2, 255]);
        assert_eq!(
            buf,
            b"M 100644 inline a/b.bin\ndata 4\n\x00\x01\x02\xff\n".to_vec()
        );
    }

    #[test]
    fn fi_rename_and_fi_delete_produce_expected_lines() {
        let mut buf = Vec::new();
        fi_rename(&mut buf, "old.txt", "new.txt");
        fi_delete(&mut buf, "gone.txt");
        assert_eq!(buf, b"R old.txt new.txt\nD gone.txt\n".to_vec());
    }

    #[test]
    fn fi_commit_writes_header_before_any_from_or_file_command() {
        let mut buf = Vec::new();
        fi_commit(&mut buf, "refs/heads/main", 7, 3, b"hello\n");
        let text = String::from_utf8(buf).expect("內容全為 ASCII，應為合法 UTF-8");
        assert_eq!(
            text,
            "commit refs/heads/main\nmark :7\n\
             author cockpit-ui-preview <ui-preview@cockpit.invalid> 1767225780 +0000\n\
             committer cockpit-ui-preview <ui-preview@cockpit.invalid> 1767225780 +0000\n\
             data 6\nhello\n\n"
        );
    }

    /// [`build_review_history_stream`] 涵蓋 design D10 的整份清單：260 個以上 commit、兩條分支與
    /// 一次 merge、附註 tag、遠端追蹤分支、改名、刪除、二進位檔、中文檔名。逐項對生成的位元組流
    /// 斷言，不啟動真正的 git（那部分由控制端的實跑驗收覆蓋）。
    #[test]
    fn build_review_history_stream_covers_design_d10_checklist() {
        let root_sha = "0123456789abcdef0123456789abcdef01234567";
        let stream = build_review_history_stream(root_sha);

        let commit_count = count_occurrences(&stream, b"\ncommit refs/heads/")
            + usize::from(stream.starts_with(b"commit refs/heads/"));
        assert_eq!(
            commit_count + 1, // +1：根 commit 在 fast-import 之外、由 build_review_repo_repo 建立
            264,
            "根 commit＋fast-import 的 commit 數應為 264（design D10：至少 260 個）"
        );

        assert_eq!(
            count_occurrences(&stream, format!("from {root_sha}\n").as_bytes()),
            1,
            "只有第一筆 fast-import commit 需要明確 from 根 commit"
        );
        assert_eq!(
            count_occurrences(&stream, b"commit refs/heads/feature/logging\n"),
            2,
            "feature/logging 應有 2 個 commit 且不合併回 main（供「依分支篩選」情境）"
        );
        assert_eq!(
            count_occurrences(&stream, b"commit refs/heads/feature/formatting\n"),
            1
        );
        assert_eq!(
            count_occurrences(&stream, b"\nmerge :"),
            1,
            "應恰好一次 merge（design D10：兩條分支與一次 merge）"
        );
        assert_eq!(
            count_occurrences(
                &stream,
                b"R history/rename-source.txt history/renamed-target.txt\n"
            ),
            1,
            "應有一筆改名"
        );
        assert_eq!(
            count_occurrences(&stream, b"D history/will-be-deleted-in-history.txt\n"),
            1,
            "應有一筆刪除"
        );
        assert_eq!(
            count_occurrences(&stream, "history/中文檔名.md".as_bytes()),
            1,
            "應涵蓋中文檔名"
        );
        assert_eq!(
            count_occurrences(&stream, b"M 100644 inline history/binary-asset.bin\n"),
            2,
            "二進位檔應新增一次、修改一次"
        );
        assert_eq!(
            count_occurrences(&stream, b"\ntag v0.1.0\n"),
            1,
            "應有一個附註 tag"
        );
        assert_eq!(
            count_occurrences(&stream, b"\nreset refs/remotes/origin/main\n"),
            1,
            "應有一個假的遠端追蹤分支"
        );
        assert!(
            stream.ends_with(b"reset refs/remotes/origin/main\nfrom :245\n"),
            "遠端追蹤分支應指到既有的 mark（第 245 筆填充 commit）"
        );

        // design D10：工作區的四種未 commit 情境要用到的三個追蹤檔案（第四種「已暫存修改」
        // 直接沿用 staged-change.txt，不需要額外一個檔案），必須先進歷史才能在磁碟上被改寫。
        for path in [
            "history/staged-change.txt",
            "history/unstaged-change.txt",
            "history/deleted-in-worktree.txt",
        ] {
            assert_eq!(
                count_occurrences(&stream, format!("M 100644 inline {path}\n").as_bytes()),
                1,
                "{path} 應在歷史中出現一次，供之後的工作區情境使用"
            );
        }
    }
}
