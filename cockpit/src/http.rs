//! HTTP 路由與內嵌靜態資源（spec `cockpit-dashboard`「路由與內嵌資源」「WebSocket 推送
//! 整張圖」；design D13、D9）。
//!
//! 所有靜態內容用 `include_str!`／`include_bytes!` 從 `cockpit/assets/` 內嵌進執行檔——不用
//! `ServeDir`、不在執行期讀檔案系統，滿足「單一執行檔複製到別的機器也能跑」（design D13；
//! 3.8 驗）。`bind`／`serve`（真的開 port）留給 3.8 的 `main`；這裡只組 [`Router`]，
//! `/api/state`、`/app/*` 等純 HTTP 路由測試用 `tower::ServiceExt::oneshot` 打，不開 port；
//! `/ws` 的測試（3.4，`cockpit/tests/ws.rs`）需要真正的 TCP 連線，另外自己
//! `TcpListener::bind("127.0.0.1:0")` + `axum::serve`。
//!
//! `/ws`：升級成 WebSocket 後，先送一則目前整份投影，之後只要投影 `version` 遞增
//! （`watch::Receiver::changed()` 觸發）就再送一份完整 JSON——不送增量（design D9；spec
//! 「WebSocket 推送整張圖」）。客戶端送來的任何訊息（文字、二進位、close 以外都忽略；
//! ping／pong 由 axum 自動回應，不會走到這裡）一律不理會；客戶端斷線或送錯只結束這個
//! task，不影響其他連線——每個連線各自 `subscribe()` 一份獨立的 `watch::Receiver`。
//!
//! 寫入端點（task 4.1；spec `pipeline-progress`「進度寫入端點」「綁定覆蓋端點」；design
//! D6）：`POST /api/projects/{project}/tasks/{task}/{op}`、`PUT`／`DELETE
//! /api/projects/{project}/workstreams/{workstream}/override`。`{op}` 在路由層以字串比對
//! [`ProgressOp`] 的四個值，不是其中之一直接回 404，不會進到 [`crate::progress_service`]；
//! project／task／workstream 是否存在則交給 [`ProgressService`] 判斷（[`WriteError`] 映射見
//! [`write_error_response`]）。成功一律 204、不回投影本體——畫面等 `/ws` 推送。這兩個路由額外
//! 掛了 task 4.2 的來源檢查 middleware（[`crate::source_check::source_check`]，Host／Origin），
//! 這裡的處理常式本身完全不管請求從哪裡來——那是 middleware 的事，擋下的請求根本不會進到
//! 這幾個 handler。
//!
//! Repo Project 管理端點（repo-projects task 4.4；spec `repo-projects` 管理端點；design D6）：`POST
//! /api/repo-projects`（201 `{"id"}`）、`PATCH`／`DELETE /api/repo-projects/{pid}`（204），本體一律以 `Bytes` 讀入再解析
//! （不檢查 Content-Type，同覆蓋端點）；`invalid_body` 由這裡判定，其餘驗證與型別化錯誤（[`RepoProjectError`]）
//! 在 [`ProgressService`]。覆蓋端點對固定 pane 的工作線回 409 `not_overridable`（[`WriteError::NotOverridable`]，
//! 在寫入鎖內判定）。`POST /api/agent/advance` 的處理常式在 [`crate::agent`]。
//!
//! 輸出讀取端點（live-output task 4.2／4.3；spec `live-output`「輸出讀取端點」「輸出端點只接受
//! 本機同源請求」；design D2、D4、D6、D7）：`GET
//! /api/runtimes/{runtime}/panes/{pane}/output`。以常數 200 行呼叫
//! [`cockpit_core::AgentRuntime::read_output`]，外包 5 秒 `tokio::time::timeout`——逾時即
//! drop 讀取 future，回 504，不讓一個卡住的 runtime 拖住其他請求（design D6）。`{runtime}`
//! 不在 [`AppState::runtimes`] 內時直接 404、不呼叫任何 runtime；
//! [`cockpit_core::RuntimeError::PaneNotFound`] 也回 404；[`cockpit_core::RuntimeError::Unavailable`]
//! 與 `Failed` 一律 503。**這個端點的所有回應（含 403／404／405／503／504）都帶**
//! `Cache-Control: no-store`（畫面內容不該進任何快取）與 `X-Content-Type-Options: nosniff`
//! （JSON 不被當成腳本或 HTML 嗅探）——live-output fix round 1（R12）把範圍從「200 回應」
//! 擴大成「這個端點的所有回應」，做法是讓 [`error_response`] 統一補上這兩個標頭（見它的文件；
//! 順帶讓寫入端點的錯誤回應也多這兩個標頭，無害）。這條路由同樣掛了來源檢查 middleware
//! （design D7：重用寫入端點的 [`crate::source_check::source_check`]，不額外要求
//! `Sec-Fetch-Site`）——pane 畫面內容（可能含 token／密碼）只給本機同源頁面與命令列讀到。
//! 非 `GET` 一律 405、不得對 runtime 發出讀取（spec 情境「不接受其他 method」逐字含 `HEAD`；
//! 「一律」代表不論 `Host`／`Origin` 是否合法）。大多數 method（`POST` 等）走這條路由專屬的
//! `MethodRouter::fallback`（不是 axum 內建的 405，本體要是 `{"error": ...}`）——`route_layer`
//! 不包 fallback，所以這條路徑的 405 既不經 `source_check`、也不會呼叫任何 runtime（R12；同時
//! 是 fix round 1 finding 2）。`HEAD` 額外明確掛 `.head(output_method_not_allowed)`（先前
//! whole-branch review F2 加的；final fix round 2 finding B 改了掛的位置）：axum 對只掛了
//! `get()` 的路由本來會把 `HEAD` 自動導去同一個 `get` slot（見 axum 0.8.9
//! `routing::method_routing::MethodRouter::call_with_state` 的 `call!(req, HEAD, head)` 先於
//! `call!(req, HEAD, get)`），這裡明確註冊 `head` 插槽覆蓋掉那條預設路徑；但 `route_layer` 只包
//! 它被呼叫**當下**已註冊的方法插槽（axum 0.8.9 `MethodRouter::route_layer` 實作：對每個當時
//! 已存在的 endpoint 各自包一層，回傳新的 `MethodRouter`，之後才呼叫的 `.head(...)` 等註冊方法
//! 是加在沒被包過的新插槽上）——`router()` 因此把 `.head(...)` 移到 `.route_layer(...)`
//! **之後**才呼叫，讓 `HEAD` 不經 `source_check`，跟 `.fallback(...)` 接住的 `POST` 等其餘
//! method 同一套優先序：不論 `Host`／`Origin` 合不合法，`HEAD` 一律 405，不會呼叫
//! `read_output`（已用測試證明，見 `cockpit/tests/output_endpoint.rs` 的
//! `read_output_head_always_405_regardless_of_host_or_origin`，涵蓋合法 Host、不合法 Host、
//! 合法 Host 但跨站 Origin 三種情境）。`HEAD` 回應依 HTTP 沒有本體：不論命中哪個 handler，axum
//! 都會在 top-level Route 依請求方法自動清空本體（見 `routing::route::RouteFuture::poll` 對
//! `Method::HEAD` 的處理），跟這裡刻意回 405 而不是 200 是兩件事——之前（fix round 1
//! finding 6）`HEAD` 落在 `get` slot 時同樣沒有本體，但錯在會真的呼叫 `read_output`；現在連
//! 呼叫都不會發生。`{runtime}`／`{pane}` percent-decode 後不是合法 UTF-8 時，
//! [`read_pane_output`] 自己截下 axum `Path` extractor 的 rejection 並改走
//! [`error_response`]（400；live-output fix round 2，Codex adversarial review finding）——
//! 這條路由上目前每一種會產生回應的來源（`source_check` 的 403、handler 的
//! 200／404／503／504、405 fallback、`Path` rejection）都經過 [`error_response`] 或
//! [`output_response`]，統一帶兩個安全標頭與 JSON `error` 本體；沒有第三種來源（`State`
//! extractor 不會失敗）。
//!
//! 根目錄查詢端點（file-review task 3.1；spec `file-review`「根目錄查詢端點」「檔案端點的共同
//! 規則」）：`GET /api/runtimes/{runtime}/panes/{pane}/root`，處理常式與允許清單在
//! [`crate::files`]。路由掛法與輸出端點完全相同（`get` → `.fallback(405)` →
//! `.route_layer(source_check)` → `.head(405)`，理由同上）；錯誤本體多一個 `code` 欄位
//! （[`coded_error_response`]），同樣經 [`with_no_store_headers`] 帶兩個安全標頭。
//!
//! 檔案端點（file-review task 3.2；spec `file-review`「檔案端點的共同規則」與四個端點各自的
//! requirement）：`GET /api/files/{runtime}/{root_id}/list`（根目錄本身）、`.../list/{*path}`、
//! `.../meta/{*path}`、`.../render/{*path}`、`.../raw/{*path}`，處理常式在 [`crate::files`]；掛法同上。
//! design D10 同時把 `source_check` 的 403 與套用它的端點（輸出、寫入）的 405 本體改成帶 `code`
//! （`forbidden_source`、`method_not_allowed`），403 本體不再帶出請求的 `Host`／`Origin`。
//!
//! git 端點（git-review task 3.2；spec `git-review`「git 端點的共同規則」與各端點的
//! requirement；design D6）：`GET /api/git/{runtime}/{root_id}/status`／`refs`／`log`／
//! `commit/{hash}`／`changes`／`merge-base`，處理常式在 [`crate::git`]；掛法同檔案端點
//! （`get` → `.fallback(405)` → `.route_layer(source_check)` → `.head(405)`）。405 直接重用
//! [`crate::files::method_not_allowed`]——本體與理由都通用，不必為 git 端點另開一份。執行 git
//! 只經 [`crate::http::AppState::git_runner`]（全程序共用一個 [`cockpit_git::GitRunner`]）。
//!
//! vendored 資源（file-review task 3.3；spec `cockpit-dashboard`「路由與內嵌資源」；design D9）：
//! `GET /vendor/{*path}`，處理常式與內嵌目錄在 [`crate::vendor`]。不套用 `source_check`（跟
//! `/app/`、`/icons/` 一樣是公開靜態資源），也不覆寫 `HEAD`／其他 method 的處理——未註冊的 method
//! 落回 axum 內建的 405、`HEAD` 落回 axum 內建的「轉發到 `GET` 再清空本體」，理由見
//! [`crate::vendor`] 模組文件。

use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::AtomicU16;
use std::sync::{Arc, LazyLock};
use std::time::Duration;

use axum::Router;
use axum::body::Bytes;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, State};
use axum::http::{HeaderName, HeaderValue, Method, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, patch, post, put};
use cockpit_core::{
    AgentRuntime, OutputFormat, Override, PaneId, ProgressOp, ProjectId, ProjectedState,
    READ_OUTPUT_FAILED_PREFIX, RepoKey, RuntimeError, RuntimeId, TaskId, WorkstreamId,
};
use serde::{Deserialize, Serialize};
use tokio::sync::watch;

use crate::files;
use crate::progress_service::{
    NewRepoProject, ProgressService, RepoProjectError, RepoProjectPatch, StageEditInput, WriteError,
};
use crate::source_check::source_check;

/// 路由共用的狀態：訂閱 [`cockpit_core::StoreHandle`] 廣播的投影（design D9），外加寫入服務、
/// 服務實際監聽的埠（design D6；task 4.1），以及依 id 查找 runtime 的對照表（design D2 第三點；
/// live-output task 4.1，之後的輸出讀取端點會用它）。
///
/// `Clone` 便宜——`watch::Receiver` 本身可以自由複製；`progress` 是 `Option<ProgressService>`
/// 本身內部也是 `Arc`；`port` 是 `Arc<AtomicU16>`；`runtimes` 是 `Arc<HashMap<..>>`。都不需要
/// 額外的鎖包一層。
#[derive(Clone)]
pub struct AppState {
    /// 目前投影的訂閱端；`/api/state` 用 `borrow()` 讀現況，`/ws` 每個連線各自
    /// `clone()` 一份自己追（1.6 的 `StoreHandle::subscribe`）。
    pub state: watch::Receiver<Arc<ProjectedState>>,
    /// 進度與覆蓋的寫入服務。`cockpit::app::build_components` 一律建立（repo-projects task 4.1）；
    /// `None` 只出現在不帶寫入服務的測試與 `ui_preview`——這時任何 project／task／workstream 引用都等於
    /// 「不存在」，寫入端點統一回 404，不需要特別區分「沒有寫入服務」與「project 不存在」。
    pub progress: Option<ProgressService>,
    /// 服務實際監聽的埠（`TcpListener::local_addr()`，design D6）。路由表在監聽埠確定之前就
    /// 已經組好（`cockpit::app::build_components` 早於 `bind`），所以用 `Arc<AtomicU16>`：
    /// `cockpit::app::run` 綁定成功後把真正的埠寫進同一個 `Arc`，所有已經拿到 `AppState`
    /// clone 的請求都會讀到更新後的值。來源檢查 middleware（task 4.2）比對 `Host` 時要用
    /// 這個，不是設定裡寫的埠——`listen = "127.0.0.1:0"` 綁定後真正拿到的埠由作業系統指派。
    pub port: Arc<AtomicU16>,
    /// 依設定裡的 `[[runtime]].id` 查找可用 runtime（design D2 第三點）：
    /// `cockpit::app::build_components` 建好每個 runtime 後 clone 一份 `Arc` 進這個表，原本
    /// 那份照舊交給 `driver::run`——同一個 runtime 因此有兩個 `Arc` 擁有者，但 driver 的生命週期
    /// 不受影響（table 不持有任何會阻塞停止流程的資源，見 `cockpit::app` 的 `shutdown_all`
    /// 文件）。表在啟動時建立後不再變動，不需要鎖。
    pub runtimes: Arc<HashMap<RuntimeId, Arc<dyn AgentRuntime>>>,
    /// 檔案端點用：每個設定中的 runtime 怎麼把 pane 回報的 `cwd` 轉成服務所在主機的路徑
    /// （file-review task 3.1；spec「檔案根目錄與允許清單」；design D2）。`cockpit::app::build_components`
    /// 依 `config.runtimes` 逐筆建立（見 [`crate::files::PathMapping::from_endpoint`]），啟動後不再變動。
    /// 檔案端點判斷「runtime 是不是設定中的 id」只看這張表：不在表內一律 `runtime_unknown`
    /// ——沒有登記對應方式的 runtime 不會被猜成「原樣當主機路徑」（fail-closed）。
    pub path_mappings: Arc<HashMap<RuntimeId, crate::files::PathMapping>>,
    /// 檔案端點啟動時就決定的設定（file-review task 3.2；design D9）：解析一次的 icon 對照表與原始
    /// 內容大小上限（正式值 [`crate::files::RAW_SIZE_LIMIT`]，測試可注入較小值）。
    pub files: Arc<crate::files::FileSettings>,
    /// git 端點共用的執行器（git-review task 3.2；design D3）：全程序只有一個，讓「同時最多 4 支
    /// git 子程序」的並行上限對所有請求生效，而不是每個請求各自一個。`GitRunner` 本身不是
    /// `Clone`，用 `Arc` 讓 `AppState`（`derive(Clone)`）可以便宜複製。
    pub git_runner: Arc<cockpit_git::GitRunner>,
    /// 客戶端活動（desktop-launch-notify task 2.1；design D6）：目前開著的 `/ws` 連線數與最近一次
    /// `GET /`／`GET /api/state` 的時間，供 `--exit-when-idle` 的閒置監看
    /// （`cockpit::app::shutdown_signal`）使用。不帶旗標時照樣記錄，只是沒有人監看。
    pub activity: ClientActivity,
}

/// `/ws` 連線數與最近一次 `GET /`／`GET /api/state` 的時間（desktop-launch-notify task 2.1；
/// spec `desktop-launch`「閒置自動結束」；design D6）。
///
/// 兩者都用 `watch` 發布：監看端每次收到 `changed()` 就重新評估，能精確地在「降為 0」那一刻
/// 開始計時，也能在計時中被新連線打斷（design D6 選 `watch<usize>` 而非 `AtomicUsize`＋輪詢的
/// 理由）。`Clone` 共用同一組頻道——路由表內的 `AppState` 與 `cockpit::app::Components` 各持
/// 一份 clone，看到的是同一個計數。
#[derive(Clone, Debug)]
pub struct ClientActivity {
    /// 目前開著的 `/ws` 連線數；升級成功時遞增，[`ConnectionGuard`] drop 時遞減。
    connections: watch::Sender<usize>,
    /// 最近一次 `GET /`／`GET /api/state` 的時間；從沒有過就是 `None`。用 tokio 的
    /// `Instant`，測試暫停時鐘時也一致。
    last_request: watch::Sender<Option<tokio::time::Instant>>,
}

impl ClientActivity {
    /// 連線數 0、沒有任何請求紀錄。
    pub fn new() -> Self {
        Self {
            connections: watch::Sender::new(0),
            last_request: watch::Sender::new(None),
        }
    }

    /// 記一條新的 `/ws` 連線：連線數加一，回傳的 guard drop 時減一。遞減放在 `Drop`，連線
    /// 不論從哪一條路徑結束（正常 close、送出失敗、客戶端直接斷線、task 被取消）都會執行。
    ///
    /// `send_modify` 每次都通知監看端（即使中間沒有人看到 1，0→1→0 也會留下變更通知）。
    pub fn connect(&self) -> ConnectionGuard {
        self.connections.send_modify(|count| *count += 1);
        ConnectionGuard {
            connections: self.connections.clone(),
        }
    }

    /// 記一次 `GET /` 或通過來源檢查的 `GET /api/state`（時間取當下；被來源檢查拒絕的請求不會
    /// 走到 handler，所以不會呼叫這裡）。
    pub fn record_request(&self) {
        self.last_request
            .send_replace(Some(tokio::time::Instant::now()));
    }

    /// 訂閱目前的 `/ws` 連線數。
    pub fn connections(&self) -> watch::Receiver<usize> {
        self.connections.subscribe()
    }

    /// 訂閱最近一次 `GET /`／（通過來源檢查的）`GET /api/state` 的時間。
    pub fn last_request(&self) -> watch::Receiver<Option<tokio::time::Instant>> {
        self.last_request.subscribe()
    }
}

impl Default for ClientActivity {
    fn default() -> Self {
        Self::new()
    }
}

/// [`ClientActivity::connect`] 回傳的 drop guard：drop 時把連線數減一。
#[derive(Debug)]
#[must_use = "guard 一 drop 連線數就會減一，要持有到連線結束"]
pub struct ConnectionGuard {
    connections: watch::Sender<usize>,
}

impl Drop for ConnectionGuard {
    fn drop(&mut self) {
        self.connections
            .send_modify(|count| *count = count.saturating_sub(1));
    }
}

impl AppState {
    /// 只有讀路由（沒有寫入服務、沒有任何 runtime）時的建構子：`progress` 固定 `None`、
    /// `port` 初值 0、`runtimes` 是空表。**`port` 不是「不會被任何 middleware 讀到」**
    /// （live-output fix round 1 finding 5 修正這句過時註解）——`router()` 一律掛一條輸出
    /// 讀取端點（`GET /api/runtimes/{runtime}/panes/{pane}/output`），永遠套著
    /// `source_check`，所以只要有任何請求打這條路由，`port` 就會被讀到。在 `port` 被真正
    /// bind 的埠回填之前，這條路由對所有帶明確埠號的 `Host`（例如 `127.0.0.1:7770`）一律回
    /// 403——fail-closed，不是「不受檢查」；只有 `Host` 埠號剛好等於初值 `0` 的請求（測試
    /// 情境，真實瀏覽器與命令列不會這樣送）才會通過來源檢查。
    pub fn new(state: watch::Receiver<Arc<ProjectedState>>) -> Self {
        Self {
            state,
            progress: None,
            port: Arc::new(AtomicU16::new(0)),
            runtimes: Arc::new(HashMap::new()),
            path_mappings: Arc::new(HashMap::new()),
            files: Arc::new(crate::files::FileSettings::embedded()),
            git_runner: Arc::new(cockpit_git::GitRunner::new()),
            activity: ClientActivity::new(),
        }
    }
}

/// 組出完整的路由表：`/`、`/app/{file}`、`/manifest.webmanifest`、`/icons/{file}`、
/// `/api/state`、`/ws`、寫入端點、輸出讀取端點；其他路徑落回 axum 預設的 404。
///
/// 寫入、輸出讀取、檔案、git、agent 端點，以及 `/api/state`、`/ws`，額外用 `route_layer` 掛
/// [`source_check`]（task 4.2；live-output task 4.3；design D6、D7；ws-source-check design D1）；
/// 只有靜態資源（`/`、`/app/…`、`/manifest.webmanifest`、`/icons/…`、`/vendor/…`）不檢查。
/// 被拒絕的 `/api/state` 不會執行 handler（不記活動時間），被拒絕的 `/ws` 不會進到
/// `WebSocketUpgrade` extractor（不升級、不計連線），所以 `--exit-when-idle` 的計時不受影響
/// （見 [`crate::source_check`] 模組文件對 `route_layer` 範圍的說明）。
pub fn router(app: AppState) -> Router {
    let source_check_layer = axum::middleware::from_fn_with_state(app.clone(), source_check);
    // 檔案端點的共同掛法（`get` → `.fallback(405)` → `.route_layer(source_check)` → `.head(405)`，
    // 理由見模組文件）。
    macro_rules! file_route {
        ($handler:expr) => {
            get($handler)
                .fallback(files::method_not_allowed)
                .route_layer(source_check_layer.clone())
                .head(files::method_not_allowed)
        };
    }
    Router::new()
        .route("/", get(index))
        .route("/app/{file}", get(app_asset))
        .route("/manifest.webmanifest", get(manifest))
        .route("/icons/{file}", get(icon))
        // vendored 資源（file-review task 3.3；design D9）：公開靜態資源，不套用 source_check，
        // 掛法同 `/app/`、`/icons/`——只註冊 `get`，其餘 method 與 `HEAD` 交給 axum 內建行為。
        .route("/vendor/{*path}", get(crate::vendor::vendor_asset))
        // 狀態端點（ws-source-check；design D1）：與寫入端點同一條來源規則。`route_layer` 在 handler
        // 之前執行，被拒絕的請求不呼叫 `record_request()`／`connect()`。
        .route(
            "/api/state",
            get(api_state).route_layer(source_check_layer.clone()),
        )
        .route(
            "/ws",
            get(ws_handler).route_layer(source_check_layer.clone()),
        )
        .route(
            "/api/projects/{project}/tasks/{task}/{op}",
            post(progress_op)
                .fallback(write_method_not_allowed)
                .route_layer(source_check_layer.clone()),
        )
        // agent 端點（progress-model task 3.4；design D6）：兩條都套 `source_check`。GET 用共同的
        // `file_route!` 掛法；POST 同寫入端點，未註冊的 method 走 405 fallback（不經來源檢查）。
        .route("/api/agent/tasks", file_route!(crate::agent::list_tasks))
        .route(
            "/api/agent/projects/{project}/tasks/{task}/{op}",
            post(crate::agent::agent_op)
                .fallback(write_method_not_allowed)
                .route_layer(source_check_layer.clone()),
        )
        // 免帶 id 推進（repo-projects task 4.4；design D7）：身分判定與其他 agent 端點相同，沒有路徑參數。
        .route(
            "/api/agent/advance",
            post(crate::agent::advance)
                .fallback(write_method_not_allowed)
                .route_layer(source_check_layer.clone()),
        )
        // Repo Project 管理端點（repo-projects task 4.4；design D6）：同寫入端點的掛法，本體為 JSON、
        // 不檢查 Content-Type（同覆蓋端點，design D6 否決把它當防線）。
        .route(
            "/api/repo-projects",
            post(add_repo_project)
                .fallback(write_method_not_allowed)
                .route_layer(source_check_layer.clone()),
        )
        .route(
            "/api/repo-projects/{pid}",
            patch(update_repo_project)
                .delete(remove_repo_project)
                .fallback(write_method_not_allowed)
                .route_layer(source_check_layer.clone()),
        )
        .route(
            "/api/projects/{project}/workstreams/{workstream}/override",
            put(set_override)
                .delete(clear_override)
                .fallback(write_method_not_allowed)
                .route_layer(source_check_layer.clone()),
        )
        .route(
            "/api/runtimes/{runtime}/panes/{pane}/output",
            get(read_pane_output)
                .fallback(output_method_not_allowed)
                .route_layer(source_check_layer.clone())
                .head(output_method_not_allowed),
        )
        // 根目錄查詢端點（file-review task 3.1）：與輸出端點同一個掛法、同一個順序——`get` →
        // `.fallback(405)` → `.route_layer(source_check)`（只包到當下已註冊的 `get`）→
        // `.head(405)`（在 route_layer 之後註冊，不經 source_check）。結果：`GET` 先過來源檢查
        // （403），`POST` 等與 `HEAD` 不論 Host／Origin 一律 405。
        .route(
            "/api/runtimes/{runtime}/panes/{pane}/root",
            get(files::pane_root)
                .fallback(files::method_not_allowed)
                .route_layer(source_check_layer.clone())
                .head(files::method_not_allowed),
        )
        // 檔案端點（file-review task 3.2）：同一個掛法。`list` 有兩條（根目錄本身、子路徑）；
        // 處理常式一律從原始請求 URI 取相對路徑（見 `crate::files` 的 `parse_target`），`{*path}`
        // 只負責讓路由比對得到，其解碼值不被使用。
        .route(
            "/api/files/{runtime}/{root_id}/list",
            file_route!(files::list),
        )
        .route(
            "/api/files/{runtime}/{root_id}/list/{*path}",
            file_route!(files::list),
        )
        .route(
            "/api/files/{runtime}/{root_id}/meta/{*path}",
            file_route!(files::meta),
        )
        .route(
            "/api/files/{runtime}/{root_id}/render/{*path}",
            file_route!(files::render),
        )
        .route(
            "/api/files/{runtime}/{root_id}/raw/{*path}",
            file_route!(files::raw),
        )
        // git 端點（git-review task 3.2；design D6）：同一個掛法，405 fallback 重用檔案端點的
        // `files::method_not_allowed`（本體與理由都通用，不必另開一份）。
        .route(
            "/api/git/{runtime}/{root_id}/status",
            file_route!(crate::git::status),
        )
        .route(
            "/api/git/{runtime}/{root_id}/refs",
            file_route!(crate::git::refs),
        )
        .route(
            "/api/git/{runtime}/{root_id}/log",
            file_route!(crate::git::log),
        )
        .route(
            "/api/git/{runtime}/{root_id}/commit/{hash}",
            file_route!(crate::git::commit),
        )
        .route(
            "/api/git/{runtime}/{root_id}/changes",
            file_route!(crate::git::changes),
        )
        .route(
            "/api/git/{runtime}/{root_id}/merge-base",
            file_route!(crate::git::merge_base),
        )
        // git 端點第二批（git-review task 3.3；design D6、D7）：`diff` 的相對路徑放在
        // query string，不需要路由層的 `{*path}`；`meta`／`blob`／`render` 以
        // `/<rev>/<相對路徑>` 放在網址路徑（同檔案端點的 `{*path}` catch-all，處理常式一律
        // 從原始請求 URI 自行解析，見 `crate::git::parse_rev_target`）。
        .route(
            "/api/git/{runtime}/{root_id}/diff",
            file_route!(crate::git::diff),
        )
        .route(
            "/api/git/{runtime}/{root_id}/meta/{*path}",
            file_route!(crate::git::meta),
        )
        .route(
            "/api/git/{runtime}/{root_id}/blob/{*path}",
            file_route!(crate::git::blob),
        )
        .route(
            "/api/git/{runtime}/{root_id}/render/{*path}",
            file_route!(crate::git::render),
        )
        // `/api/files/` 底下其餘形狀（`.../list/` 這種空的 `{*path}`——matchit 的 catch-all 不收空值、
        // `.../raw` 少了路徑、不認得的端點名）不命中任何路由，落到 axum 預設的 404（空本體）。想用
        // `/api/files/{*rest}` 接住它們會與上面的 `{*path}` 路由衝突（matchit 插入時 panic）。
        .with_state(app)
}

/// `GET /`：內嵌的 `index.html`。`GET` 時記一次活動（desktop-launch-notify task 2.1；spec
/// 「閒置自動結束」：啟動器開窗前後的請求延長閒置期限）；axum 把 `HEAD` 也導到這裡，`HEAD`
/// 不算（spec：其他 HTTP 請求不影響計時）。
///
/// 回應帶 `X-Frame-Options: DENY` 與 `Content-Security-Policy: frame-ancestors 'none'`
/// （ws-source-check fix round 1）：外站網頁不能把儀表板以 iframe 嵌進去——否則 iframe 內的
/// `channel.js` 以 Cockpit 自己的 origin 連 `/ws`、來源檢查合格，會讓 `--exit-when-idle` 的後端
/// 永遠不閒置結束，也讓寫入按鈕可被點擊劫持。`GET /` 本身不做來源檢查（見 [`router`]）。
///
/// 送出的內容是 [`INDEX_HTML`]：內嵌檔中的程式版本占位字已換成本 crate 版本。
async fn index(method: Method, State(app): State<AppState>) -> impl IntoResponse {
    if method == Method::GET {
        app.activity.record_request();
    }
    (
        [
            (header::CONTENT_TYPE, "text/html; charset=utf-8"),
            (header::X_FRAME_OPTIONS, "DENY"),
            (header::CONTENT_SECURITY_POLICY, "frame-ancestors 'none'"),
        ],
        INDEX_HTML.as_str(),
    )
}

/// 內嵌的 `index.html`，`<meta name="cockpit-version">` 的占位字換成 `CARGO_PKG_VERSION`
/// （2026-10-05 使用者指示：底列顯示程式版本；`render.js` 讀這個 meta）。只在第一次請求時算一次。
static INDEX_HTML: LazyLock<String> = LazyLock::new(|| {
    include_str!("../assets/index.html").replace("__COCKPIT_VERSION__", env!("CARGO_PKG_VERSION"))
});

/// `/app/<file>`：只認得這幾個檔名，查表命中就回對應內嵌內容，其餘 404——不是「任意檔名
/// 都能讀」的通用靜態伺服（design D13 明確排除 `ServeDir`）。
async fn app_asset(Path(file): Path<String>) -> Response {
    match file.as_str() {
        "channel.js" => (
            [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
            include_str!("../assets/app/channel.js"),
        )
            .into_response(),
        "render.js" => (
            [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
            include_str!("../assets/app/render.js"),
        )
            .into_response(),
        "style.css" => (
            [(header::CONTENT_TYPE, "text/css; charset=utf-8")],
            include_str!("../assets/app/style.css"),
        )
            .into_response(),
        "actions.js" => (
            [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
            include_str!("../assets/app/actions.js"),
        )
            .into_response(),
        // ui-language task 1.1（design D1）：介面字典與語言決定；index.html 第一個載入。
        "i18n.js" => (
            [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
            include_str!("../assets/app/i18n.js"),
        )
            .into_response(),
        "output.js" => (
            [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
            include_str!("../assets/app/output.js"),
        )
            .into_response(),
        // file-review task 4.1（design D6）：左欄分頁／檔案樹／分頁區（files.js）與檢視器（viewers.js）。
        "files.js" => (
            [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
            include_str!("../assets/app/files.js"),
        )
            .into_response(),
        "viewers.js" => (
            [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
            include_str!("../assets/app/viewers.js"),
        )
            .into_response(),
        // git-review task 3.3：空殼檔案，內容由 task 4.x 填入（design D9「變更」面板／diff／
        // Graph／某版本分頁）；先掛路由與內嵌資源，讓 cockpit-dashboard「路由與 content-type」
        // 情境現在就能全綠。
        "git.js" => (
            [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
            include_str!("../assets/app/git.js"),
        )
            .into_response(),
        // desktop-launch-notify task 3.3（design D7、D8）：桌面通知模組與通知設定面板；
        // index.html 在 render.js 之前載入。
        "notify.js" => (
            [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
            include_str!("../assets/app/notify.js"),
        )
            .into_response(),
        _ => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn manifest() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "application/manifest+json")],
        include_str!("../assets/manifest.webmanifest"),
    )
}

/// `/icons/<file>`：同 [`app_asset`]，只認得這兩個檔名。
async fn icon(Path(file): Path<String>) -> Response {
    match file.as_str() {
        "icon-192.png" => (
            [(header::CONTENT_TYPE, "image/png")],
            include_bytes!("../assets/icons/icon-192.png").as_slice(),
        )
            .into_response(),
        "icon-512.png" => (
            [(header::CONTENT_TYPE, "image/png")],
            include_bytes!("../assets/icons/icon-512.png").as_slice(),
        )
            .into_response(),
        _ => StatusCode::NOT_FOUND.into_response(),
    }
}

/// 目前整份投影（design D9：讀 `watch::Receiver` 的 `borrow()` 現況，不觸發重新計算）。
///
/// 手動序列化而不是 `axum::Json`：`Arc<ProjectedState>: Serialize` 需要 serde 的 `rc`
/// feature（`cockpit/Cargo.toml` 沒開，且不在本 task 授權改動範圍），所以用 `&**` 兩層
/// deref（`Ref<Arc<ProjectedState>>` → `Arc<ProjectedState>` → `ProjectedState`）拿到
/// `&ProjectedState` 再交給 [`state_json`]——與 `/ws` 共用同一個序列化函數（design D9
/// 節錄）。
///
/// `GET` 時記一次活動（desktop-launch-notify task 2.1；同 [`index`]，`HEAD` 不算）：啟動器
/// 偵測「Cockpit 是否在執行」打的就是這條，偵測到之後才開窗，期間後端不該剛好閒置結束。
async fn api_state(method: Method, State(app): State<AppState>) -> impl IntoResponse {
    if method == Method::GET {
        app.activity.record_request();
    }
    let body = state_json(&app.state.borrow());
    ([(header::CONTENT_TYPE, "application/json")], body)
}

/// 序列化一份投影成 JSON 文字；`/api/state` 與 `/ws` 共用，避免兩份序列化邏輯（design D9
/// 節錄：「序列化：每則訊息＝`serde_json::to_string(&*state)`；可與 `/api/state` 共用同一個
/// 序列化函數」）。
fn state_json(state: &ProjectedState) -> String {
    serde_json::to_string(state).expect("ProjectedState 序列化不會失敗")
}

/// `/ws`：升級成 WebSocket，把訂閱端交給 [`handle_socket`]（design D9）。
///
/// 升級成功（進到 `on_upgrade` 的 callback）時才把連線數加一，guard 跟著連線 task 活到
/// [`handle_socket`] 結束——任何結束路徑（含 task 被取消）都會在 drop 時減一
/// （desktop-launch-notify task 2.1；design D6）。
async fn ws_handler(ws: WebSocketUpgrade, State(app): State<AppState>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| async move {
        let _connection = app.activity.connect();
        handle_socket(socket, app.state.clone()).await;
    })
}

/// 連線期間的完整生命週期：先送現況一份，之後每次投影 `version` 遞增就再送整份；忽略
/// 客戶端訊息；斷線或送失敗就結束（純觀察者，不對狀態庫做任何寫入）。
async fn handle_socket(mut socket: WebSocket, mut state: watch::Receiver<Arc<ProjectedState>>) {
    tracing::debug!("ws 連線開始");

    let first = state_json(&state.borrow_and_update());
    if socket.send(Message::text(first)).await.is_err() {
        tracing::debug!("ws 連線結束（送出第一則訊息失敗）");
        return;
    }

    loop {
        tokio::select! {
            changed = state.changed() => {
                if changed.is_err() {
                    // 送出端（StoreHandle）掉了，不會再有新的投影，結束這個連線。
                    break;
                }
                let body = state_json(&state.borrow_and_update());
                if socket.send(Message::text(body)).await.is_err() {
                    break;
                }
            }
            msg = socket.recv() => {
                match msg {
                    None | Some(Err(_)) | Some(Ok(Message::Close(_))) => break,
                    // 文字、二進位、ping、pong：一律忽略（ping／pong 其實已經被 axum
                    // 在更底層自動回應，不會走到這裡；保留這個分支只是讓比對窮盡）。
                    Some(Ok(_)) => {}
                }
            }
        }
    }

    tracing::debug!("ws 連線結束");
}

/// `POST /api/projects/<project>/tasks/<task>/<op>`（spec `pipeline-progress`「進度寫入端點」；
/// design D6：`<op>` 以字串比對四值，其他回 404，不進到 [`ProgressService`]）。
async fn progress_op(
    State(app): State<AppState>,
    Path((project, task, op)): Path<(String, String, String)>,
) -> Response {
    let Some(parsed_op) = parse_progress_op(&op) else {
        return invalid_op_response(&format!("不是合法的操作：{op}"), &op);
    };
    let Some(progress) = &app.progress else {
        // 沒有寫入服務時（只在測試與 ui_preview；app 一律建立）任何 project 引用都等於「不存在」；
        // 借用 WriteError::UnknownProject 的 Display，跟寫入服務判定「project 不存在」時
        // 回的本體用同一套措辭，不要另開一種說法（Codex fix round 1 finding 3：404 也要有
        // `{"error": ...}` 本體）。
        return write_error_response(WriteError::UnknownProject(ProjectId::new(project)));
    };
    match progress
        .apply_progress(&ProjectId::new(project), &TaskId::new(task), parsed_op)
        .await
    {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(error) => write_error_response(error),
    }
}

/// 404 `invalid_op`（參數 `op`）：進度端點與 agent 端點遇到不認得的 `<op>` 共用，`reason` 是各自的
/// 繁中原文（ui-language design D4）。
pub(crate) fn invalid_op_response(reason: &str, op: &str) -> Response {
    coded_error_response_with_params(StatusCode::NOT_FOUND, "invalid_op", reason, [("op", op)])
}

/// 把路徑上的 `<op>` 字串比對成 [`ProgressOp`]；不是五值之一回 `None`（design D6；progress-model
/// task 3.3 加入 `retreat`；`start` 是 agent 專屬，人工端點不認得）。
fn parse_progress_op(op: &str) -> Option<ProgressOp> {
    match op {
        "advance" => Some(ProgressOp::Advance),
        "retreat" => Some(ProgressOp::Retreat),
        "complete" => Some(ProgressOp::Complete),
        "fail" => Some(ProgressOp::Fail),
        "clear" => Some(ProgressOp::Clear),
        _ => None,
    }
}

/// `PUT` 本體的形狀（spec `pipeline-progress`「綁定覆蓋端點」）：`runtime`／`pane_id` 都必須
/// 是字串欄位；`serde` 解析失敗（缺欄位、型別不對、根本不是 JSON）統一回 400。
#[derive(Deserialize)]
struct OverrideRequest {
    runtime: String,
    pane_id: String,
}

/// `PUT /api/projects/<project>/workstreams/<workstream>/override`：設定畫面覆蓋（spec
/// 「綁定覆蓋端點」）。
///
/// 不檢查 `Content-Type`——design D6 明確否決把它當防線（沒有本體的 `POST` 端點與
/// `text/plain` 表單都能繞過），這裡直接把整個 body 當 JSON 解析。
async fn set_override(
    State(app): State<AppState>,
    Path((project, workstream)): Path<(String, String)>,
    body: Bytes,
) -> Response {
    let Some(progress) = &app.progress else {
        return write_error_response(WriteError::UnknownProject(ProjectId::new(project)));
    };
    let request: OverrideRequest = match serde_json::from_slice(&body) {
        Ok(request) => request,
        Err(_) => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "本體必須是含 runtime、pane_id 兩個字串欄位的 JSON 物件",
            );
        }
    };
    let override_ = Override {
        runtime: RuntimeId::new(request.runtime),
        pane_id: PaneId::new(request.pane_id),
    };
    match progress
        .set_override(
            &ProjectId::new(project),
            &WorkstreamId::new(workstream),
            override_,
        )
        .await
    {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(error) => write_error_response(error),
    }
}

/// `DELETE` 同路徑：取消畫面覆蓋；覆蓋本來就不存在時也回 204（spec「取消不存在的覆蓋回
/// 204」）。
async fn clear_override(
    State(app): State<AppState>,
    Path((project, workstream)): Path<(String, String)>,
) -> Response {
    let Some(progress) = &app.progress else {
        return write_error_response(WriteError::UnknownProject(ProjectId::new(project)));
    };
    match progress
        .clear_override(&ProjectId::new(project), &WorkstreamId::new(workstream))
        .await
    {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(error) => write_error_response(error),
    }
}

// ---------------------------------------------------------------------------
// Repo Project 管理端點（repo-projects task 4.4；spec `repo-projects`「加入 Repo Project」「修改 Repo Project
// 名稱與 stages」「移除 Repo Project」「Repo Project 的輸入驗證」；design D6）
// ---------------------------------------------------------------------------

/// `POST /api/repo-projects` 的本體。未知欄位、缺欄位、型別不對一律解析失敗，回 400 `invalid_body`
/// （見 [`parse_add_repo_project_body`]）。
///
/// 公開（`doc(hidden)`）只因為它是 [`parse_add_repo_project_body`] 的回傳型別：`ui_preview` 的假端點用同一個函式解析
/// （repo-projects task 5.1 fix round 1），其餘程式碼只在本模組使用。
#[doc(hidden)]
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AddRepoProjectRequest {
    /// repo key（投影 `detected_repos[].repo` 的原值）。
    pub repo: String,
    /// stage 清單（尚未正規化）。
    pub stages: Vec<String>,
    /// 顯示名稱（選填，尚未正規化）。
    #[serde(default)]
    pub name: Option<String>,
    /// 每站的 OpenSpec 階段對應（選填，與 `stages` 逐項對齊；`null` 與省略相同）。只以字串接收，由服務在名稱與
    /// stages 之後轉成階段並驗證：未知字串不可落到反序列化錯誤的 `invalid_body`，也不在 HTTP 層提早回錯，才不會
    /// 蓋掉較前面的檢查（openspec-stage-sync task 4.5，design D10-1）。
    #[serde(default)]
    pub phases: Option<Vec<Option<String>>>,
}

/// `PATCH` 本體中的一個 stage：`from` 省略與 `null` 都表示新增的 stage；`phase` 省略與 `null` 都表示不對應
/// （不是沿用舊值；openspec-stage-sync task 4.5）。
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StageEditRequest {
    name: String,
    #[serde(default)]
    from: Option<String>,
    #[serde(default)]
    phase: Option<String>,
}

/// `PATCH /api/repo-projects/{pid}` 的本體：`name`、`stages` 皆選填，但至少要給一個。
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UpdateRepoProjectRequest {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    stages: Option<Vec<StageEditRequest>>,
}

/// 400 `invalid_body`：本體不是合法 JSON 物件、欄位缺漏或型別不對、有未知欄位、`PATCH` 兩個欄位都沒給。
/// 一個固定原文（不回顯本體內容），英文介面以字典範本顯示。
fn invalid_body_response() -> Response {
    coded_error_response(
        StatusCode::BAD_REQUEST,
        "invalid_body",
        "請求本體不合法：不是有效的 JSON 物件，或欄位不符合格式",
    )
}

/// [`RepoProjectError`] → HTTP 回應（design D6）：`RepoNotDetected`／`UnknownProject` 404、`RepoAlreadyAdded`／
/// `NotRepoProject` 409、`InvalidName`／`InvalidStages` 400，`Write` 沿用 [`write_error_response`]。
fn repo_project_error_response(error: RepoProjectError) -> Response {
    let status = match error {
        RepoProjectError::Write(write) => return write_error_response(write),
        RepoProjectError::RepoNotDetected(_) | RepoProjectError::UnknownProject(_) => {
            StatusCode::NOT_FOUND
        }
        RepoProjectError::RepoAlreadyAdded(_) | RepoProjectError::NotRepoProject(_) => {
            StatusCode::CONFLICT
        }
        RepoProjectError::InvalidName | RepoProjectError::InvalidStages => StatusCode::BAD_REQUEST,
    };
    coded_error_response_with_params(status, error.code(), &error.to_string(), error.params())
}

/// 解析 `POST /api/repo-projects` 的本體；不合法時回 400 `invalid_body` 的回應（Box 起來，避免 `Result` 的 Err 過大）。
/// 正式端點與 `ui_preview` 的假端點共用這一個函式（repo-projects task 5.1 fix round 1），前端送錯形狀時 preview 也會失敗。
#[doc(hidden)]
pub fn parse_add_repo_project_body(body: &[u8]) -> Result<AddRepoProjectRequest, Box<Response>> {
    serde_json::from_slice(strip_utf8_bom(body)).map_err(|_| Box::new(invalid_body_response()))
}

/// 去掉本體開頭的**一個** UTF-8 BOM（`EF BB BF`），其餘原樣交給 JSON 解析。Windows PowerShell 5.1 在 UTF-8 主控台把
/// here-string 經標準輸入交給 `curl.exe` 時會多送 BOM（repo-projects task 7.3 真機冒煙實測），`serde_json` 不接受。
/// Repo Project 管理端點的 POST 與 PATCH 共用。
fn strip_utf8_bom(body: &[u8]) -> &[u8] {
    body.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(body)
}

/// [`RepoProjectError`] 的 HTTP 回應（同 [`repo_project_error_response`]）：讓 `ui_preview` 的假端點產生與正式端點
/// 逐字相同的錯誤本體，而不必公開回應函式（repo-projects task 5.1 fix round 1）。
impl IntoResponse for RepoProjectError {
    fn into_response(self) -> Response {
        repo_project_error_response(self)
    }
}

/// `POST /api/repo-projects`：加入 Repo Project，成功回 201 `{"id": "<新 Project 的 id>"}`。沒有寫入服務時
/// （只在測試與 `ui_preview`）沒有任何偵測到的 repo，等同 `repo_not_detected`。
async fn add_repo_project(State(app): State<AppState>, body: Bytes) -> Response {
    let request = match parse_add_repo_project_body(&body) {
        Ok(request) => request,
        Err(response) => return *response,
    };
    let repo = RepoKey::new(request.repo);
    let Some(progress) = &app.progress else {
        return repo_project_error_response(RepoProjectError::RepoNotDetected(repo));
    };
    let new_project = NewRepoProject {
        repo,
        stages: request.stages,
        name: request.name,
        phases: request.phases,
    };
    match progress.add_repo_project(new_project).await {
        Ok(id) => with_no_store_headers(
            (
                StatusCode::CREATED,
                [(header::CONTENT_TYPE, "application/json")],
                serde_json::json!({ "id": id }).to_string(),
            )
                .into_response(),
        ),
        Err(error) => repo_project_error_response(error),
    }
}

/// `PATCH /api/repo-projects/{pid}`：改名稱與（或）stages，成功回 204。兩個欄位都沒給屬 `invalid_body`，
/// 由這裡判定（服務收到兩欄皆 `None` 只做 pid 查找）。沒有寫入服務時任何 pid 都等於不存在。
async fn update_repo_project(
    State(app): State<AppState>,
    Path(pid): Path<String>,
    body: Bytes,
) -> Response {
    let Ok(request) = serde_json::from_slice::<UpdateRepoProjectRequest>(strip_utf8_bom(&body))
    else {
        return invalid_body_response();
    };
    if request.name.is_none() && request.stages.is_none() {
        return invalid_body_response();
    }
    let project = ProjectId::new(pid);
    let Some(progress) = &app.progress else {
        return repo_project_error_response(RepoProjectError::UnknownProject(project));
    };
    let patch = RepoProjectPatch {
        name: request.name,
        stages: request.stages.map(|edits| {
            edits
                .into_iter()
                .map(|edit| StageEditInput {
                    name: edit.name,
                    from: edit.from,
                    phase: edit.phase,
                })
                .collect()
        }),
    };
    match progress.update_repo_project(&project, patch).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(error) => repo_project_error_response(error),
    }
}

/// `DELETE /api/repo-projects/{pid}`：移除 Repo Project（定義與所有 task 的進度），成功回 204。
async fn remove_repo_project(State(app): State<AppState>, Path(pid): Path<String>) -> Response {
    let project = ProjectId::new(pid);
    let Some(progress) = &app.progress else {
        return repo_project_error_response(RepoProjectError::UnknownProject(project));
    };
    match progress.remove_repo_project(&project).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(error) => repo_project_error_response(error),
    }
}

// ---------------------------------------------------------------------------
// 輸出讀取端點（live-output task 4.2；spec `live-output`「輸出讀取端點」；design D2、D4、D6）
// ---------------------------------------------------------------------------

/// 單次讀取逾時（design D6：5 秒，含排隊等鎖的時間；逾時即 drop 讀取 future，不讓一個卡住的
/// runtime 拖住這個請求，也不影響其他請求）。
const OUTPUT_READ_TIMEOUT: Duration = Duration::from_secs(5);

/// 每次讀取最多回傳的行數（design D4：200 行是產品決定，由 `cockpit` 以常數傳入，不是 HERDR
/// 接合細節）。
const OUTPUT_MAX_LINES: u32 = 200;

/// `GET /api/runtimes/<runtime>/panes/<pane>/output`（live-output task 4.2；spec
/// `live-output`「輸出讀取端點」；design D2、D4、D6）：即時向該 runtime 讀一次 `pane` 的輸出，
/// 不快取、不經過狀態庫，也不留下任何跟這次請求有關的狀態。
///
/// `{runtime}` 不在 [`AppState::runtimes`] 內時直接 404，**不會**呼叫任何 runtime 的
/// `read_output`（spec 情境「不認識的 runtime」）。這條路由額外掛了來源檢查 middleware
/// （live-output task 4.3；design D7），被擋下的請求同樣不會進到這裡（見 `router()`）。
///
/// `path` 收 `Result<Path<(String, String)>, PathRejection>` 而不是直接 `Path<(String,
/// String)>`（live-output fix round 2；Codex adversarial review finding，medium）：路徑片段
/// percent-decode 後不是合法 UTF-8 時（例如 `%FF`），axum 的 `Path` extractor 會在進入這個
/// handler **之前**產生一個 rejection 回應——若讓 axum 自己處理，那個回應是純文字、沒有
/// `Cache-Control`／`X-Content-Type-Options`，本體也不是 `{"error": ...}`，違反 spec「這個
/// 端點的所有回應都要帶兩個標頭」。這裡截下 rejection 自己組 [`error_response`]；錯誤訊息
/// 固定為中文說明，**不**把 rejection 內部訊息（可能含使用者送來的原始路徑片段）反射進
/// `error` 字串。
async fn read_pane_output(
    State(app): State<AppState>,
    path: Result<Path<(String, String)>, axum::extract::rejection::PathRejection>,
) -> Response {
    let Path((runtime, pane)) = match path {
        Ok(path) => path,
        Err(_rejection) => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "路徑格式不正確：runtime 或 pane id 不是合法的 UTF-8 字串",
            );
        }
    };

    let Some(agent_runtime) = app.runtimes.get(&RuntimeId::new(runtime.as_str())) else {
        return coded_error_response_with_params(
            StatusCode::NOT_FOUND,
            "runtime_not_found",
            &format!("runtime 不存在：{runtime}"),
            [("runtime", runtime.as_str())],
        );
    };

    let pane_id = PaneId::new(pane.as_str());
    let read = tokio::time::timeout(
        OUTPUT_READ_TIMEOUT,
        agent_runtime.read_output(&pane_id, OUTPUT_MAX_LINES),
    )
    .await;

    match read {
        Ok(Ok(output)) => output_response(&runtime, &pane, output),
        Ok(Err(err @ RuntimeError::PaneNotFound { .. })) => coded_error_response_with_params(
            StatusCode::NOT_FOUND,
            "pane_gone",
            &err.to_string(),
            [("pane", pane.as_str())],
        ),
        Ok(Err(err @ (RuntimeError::Unavailable { .. } | RuntimeError::Failed(_)))) => {
            // `detail`：底層原因。`Failed` 的原文帶 cockpit-herdr 加的繁中前綴，剝掉讓英文介面
            // 的範本不夾繁中；`error` 欄位照舊是完整原文。
            let text = err.to_string();
            let detail = text
                .strip_prefix(READ_OUTPUT_FAILED_PREFIX)
                .unwrap_or(&text);
            coded_error_response_with_params(
                StatusCode::SERVICE_UNAVAILABLE,
                "output_read_failed",
                &text,
                [("detail", detail)],
            )
        }
        Err(_elapsed) => {
            coded_error_response(StatusCode::GATEWAY_TIMEOUT, "read_timeout", "讀取逾時")
        }
    }
}

/// 輸出端點的 405 handler（spec 情境「不接受其他 method」：405，`POST` 本體 `{"error": ...}`、
/// `HEAD` 依 HTTP 沒有本體，沒有對 runtime 發出讀取；「一律」代表不論 `Host`／`Origin` 是否
/// 合法）。掛在兩個地方（見 `router()`）：`.fallback(...)` 接住 `POST` 等其餘 method——
/// `route_layer` 不包 fallback（`crate::source_check` 模組文件已說明範圍），這個路徑既不經
/// `source_check`、本身也不碰 `AppState::runtimes`，天生就滿足「不對 runtime 發出讀取」
/// （live-output fix round 1 finding 2）；`.head(...)` 額外明確接住 `HEAD`，且刻意掛在
/// `.route_layer(...)` **之後**（final fix round 2 finding B）——`route_layer` 只包它被呼叫
/// 當下已註冊的方法插槽，之後才註冊的 `head` 插槽不會被那層包住，因此 `HEAD` 跟 `.fallback(...)`
/// 接住的 `POST` 等其餘 method 同一套優先序：不論 `Host`／`Origin` 是否合法，一律 405，不經
/// `source_check`、也不會呼叫 `read_output`。用它取代 axum 內建的 405 fallback（本體是空的，
/// 不符合 spec 逐字要求的 `{"error": ...}`）。
async fn output_method_not_allowed() -> Response {
    // file-review task 3.2（design D10）：套用 source_check 的端點，405 本體一律帶 `code`。
    coded_error_response(
        StatusCode::METHOD_NOT_ALLOWED,
        "method_not_allowed",
        "這個端點只接受 GET",
    )
}

/// 寫入端點的 405（file-review task 3.2；design D10：套用 source_check 的端點，405 本體一律是
/// `{"error": ..., "code": "method_not_allowed"}`）。掛在 `.fallback(...)`，跟輸出端點一樣不被
/// `route_layer` 包住：未註冊的 method 不經來源檢查、也不進寫入服務。先前這裡是 axum 內建的 405
/// （空本體），只有狀態碼被測試斷言，改成帶本體不影響那些斷言。
async fn write_method_not_allowed() -> Response {
    coded_error_response(
        StatusCode::METHOD_NOT_ALLOWED,
        "method_not_allowed",
        "這個端點不接受這個 method",
    )
}

/// 200 回應本體（spec 逐字欄位名：`runtime`、`pane_id`、`format`、`text`、`segments`、
/// `truncated`；`runtime`／`pane_id` 就是路徑上解碼後的值，不是從 `output` 推的；`segments`
/// 是 `text` 的樣式切分，live-output-color task 4.1／design D6）。標頭見
/// [`with_no_store_headers`]。
fn output_response(runtime: &str, pane_id: &str, output: cockpit_core::PaneOutput) -> Response {
    #[derive(Serialize)]
    struct OutputBody<'a> {
        runtime: &'a str,
        pane_id: &'a str,
        format: OutputFormat,
        text: &'a str,
        segments: &'a [cockpit_core::OutputSegment],
        truncated: bool,
    }

    let body = serde_json::to_string(&OutputBody {
        runtime,
        pane_id,
        format: output.format(),
        text: output.text(),
        segments: output.segments(),
        truncated: output.truncated(),
    })
    .expect("輸出回應只含字串、布林與樣式片段，序列化不會失敗");

    with_no_store_headers(
        (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "application/json")],
            body,
        )
            .into_response(),
    )
}

/// 補上「不快取、不嗅探」的兩個標頭：`Cache-Control: no-store`（畫面內容不該進任何快取）與
/// `X-Content-Type-Options: nosniff`（JSON 不被當成腳本或 HTML 嗅探；design D6）。
///
/// live-output fix round 1（R12）把範圍從「輸出端點的 200 回應」擴大成「輸出端點的所有
/// 回應」，含 403（[`crate::source_check::source_check`] 擋下時呼叫的正是
/// [`error_response`]）、404、405（[`output_method_not_allowed`]）、503、504。最小且不會
/// 漏的做法是讓 [`error_response`] 統一補上這兩個標頭，[`output_response`] 共用同一份邏輯——
/// 副作用是寫入端點（`progress_op`／`set_override`／`clear_override`）的錯誤回應也會多這兩個
/// 標頭，這是刻意接受的：多兩個標頭對它們無害，換成只套輸出端點的獨立 wrapper 反而要多維護
/// 一條分支，且更容易在新增錯誤分支時漏掛。
pub(crate) fn with_no_store_headers(mut response: Response) -> Response {
    let headers = response.headers_mut();
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    headers.insert(
        HeaderName::from_static("x-content-type-options"),
        HeaderValue::from_static("nosniff"),
    );
    response
}

/// [`WriteError`] → HTTP 回應：`Unknown*`／`NoTaskForPane` 404、`Rejected`／`NotOverridable`／`AmbiguousTask` 409、`Persist`／`Internal` 500，
/// 本體一律是 `{"error": "<原因>"}`（task 4.1 原文「錯誤本體 `{"error": ...}`」——不是只有
/// 409／500，Codex fix round 1 finding 3：先前 404 回空本體，跟 tasks.md 4.1 明定的形狀
/// 不符；axum 自己判定路徑完全不匹配的 404（例如未知路徑）不在此限，那種情況根本不會進到
/// 這個函式）。
pub(crate) fn write_error_response(error: WriteError) -> Response {
    let status = match &error {
        WriteError::UnknownProject(_)
        | WriteError::UnknownTask(_)
        | WriteError::UnknownWorkstream(_)
        | WriteError::NoTaskForPane => StatusCode::NOT_FOUND,
        WriteError::PaneNotBound => StatusCode::FORBIDDEN,
        WriteError::Rejected(_) | WriteError::NotOverridable | WriteError::AmbiguousTask => {
            StatusCode::CONFLICT
        }
        WriteError::Persist { .. } | WriteError::Internal(_) => {
            tracing::error!(%error, "寫入端點：狀態檔寫入失敗");
            StatusCode::INTERNAL_SERVER_ERROR
        }
    };
    // `code`／`params` 由 `WriteError` 自己提供（ui-language design D4），`error` 仍是繁中原文。
    coded_error_response_with_params(status, error.code(), &error.to_string(), error.params())
}

/// 錯誤回應本體 `{"error": "<reason>"}`（spec 多處要求的形狀）；檔案端點（file-review design
/// D10）多一個 `code` 欄位，其餘端點沒有 `code` 時整個欄位省略，本體與先前逐字相同。
#[derive(Serialize)]
struct ErrorBody<'a> {
    error: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    code: Option<&'a str>,
    /// 代碼的參數（名稱 → 字串值；ui-language design D4）；沒有參數時整個欄位省略。
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    params: BTreeMap<String, String>,
}

/// 全 crate 共用的錯誤回應：本體 `{"error": "<reason>"}`，並帶上
/// [`with_no_store_headers`] 的兩個標頭（live-output fix round 1 R12：輸出端點的所有回應都
/// 要帶這兩個標頭，這裡是唯一、不會漏掉任何分支的掛點——見 [`with_no_store_headers`] 的文件
/// 說明為什麼連寫入端點的錯誤回應也一起帶）。
pub(crate) fn error_response(status: StatusCode, reason: &str) -> Response {
    error_body_response(status, reason, None, BTreeMap::new())
}

/// 同 [`error_response`]，本體多帶 `code`：`{"error": "<reason>", "code": "<code>"}`（file-review
/// spec「檔案端點的共同規則」；design D10）。檔案端點的錯誤一律走這裡（經
/// [`crate::files::FileApiError`] 的 `IntoResponse`）。
pub(crate) fn coded_error_response(status: StatusCode, code: &str, reason: &str) -> Response {
    error_body_response(status, reason, Some(code), BTreeMap::new())
}

/// 同 [`coded_error_response`]，本體再帶 `params`：`{"error", "code", "params": {…}}`（ui-language
/// design D4；前端以代碼加參數翻譯，`error` 仍是繁中原文）。`params` 為空時省略該欄位。
pub(crate) fn coded_error_response_with_params<K, V>(
    status: StatusCode,
    code: &str,
    reason: &str,
    params: impl IntoIterator<Item = (K, V)>,
) -> Response
where
    K: AsRef<str>,
    V: AsRef<str>,
{
    let params = params
        .into_iter()
        .map(|(name, value)| (name.as_ref().to_string(), value.as_ref().to_string()))
        .collect();
    error_body_response(status, reason, Some(code), params)
}

fn error_body_response(
    status: StatusCode,
    reason: &str,
    code: Option<&str>,
    params: BTreeMap<String, String>,
) -> Response {
    let body = serde_json::to_string(&ErrorBody {
        error: reason,
        code,
        params,
    })
    .expect("ErrorBody 只含字串，序列化不會失敗");
    with_no_store_headers(
        (status, [(header::CONTENT_TYPE, "application/json")], body).into_response(),
    )
}
