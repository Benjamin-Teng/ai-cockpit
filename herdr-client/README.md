# herdr-client

`herdr-client` 是 AI Agent Cockpit 用來跟 [HERDR](https://github.com) 的 observer API 對話的
底層 crate：只懂 HERDR 協定的 observer 子集（`session.snapshot`、`events.subscribe`），
不依賴任何 `cockpit-*` crate（ADR-0003），change 1b 只透過本 crate 的 `Connector`／`Client`／
型別使用它，不需要知道 transport 差異。詳細背景見
`docs/superpowers/specs/2026-09-13-cockpit-mvp-design.md` 與
`openspec/changes/herdr-client/`。

## Transport（`herdr_client::connector`）

三種建立一條 NDJSON 連線的方式，統一成 `Connector` trait；每次 `connect()` 開一條全新連線，
不在同一條連線上多工多個 method（HERDR 每連線一 method）。

```rust
pub trait NdjsonStream: Send {
    async fn send_line(&mut self, line: &str) -> std::io::Result<()>;
    async fn recv_line(&mut self) -> std::io::Result<Option<String>>;
}

pub trait Connector: Send + Sync {
    async fn connect(&self) -> Result<Box<dyn NdjsonStream>, ConnectError>;
    fn describe(&self) -> String;
}
```

| 型別 | 平台 | 說明 |
|---|---|---|
| `NamedPipeConnector { .. }` | `cfg(windows)` | pipe 名稱＝`\\.\pipe\` 接 `path` 全文（含磁碟機冒號、反斜線）；遇 `ERROR_PIPE_BUSY`（231）每 50ms 重試，上限 1 秒（`tests/transport.rs` 的 `named_pipe_busy_instance_retries_then_succeeds`／`named_pipe_busy_timeout_is_io` 用 `max_instances(1)` 佔住唯一 instance，實測驗證重試成功與逾時回 `Io` 兩條路徑；Codex fix round 1 finding 3） |
| `UnixSocketConnector { .. }` | `cfg(unix)` | `tokio::net::UnixStream::connect` |
| `ChildStdioConnector { command, args }` | 任何 | 子程序 stdin／stdout 當連線，stderr 進 `tracing::debug` 且保留最後 20 行；Windows 上加 `CREATE_NO_WINDOW`（`0x0800_0000`，硬寫常數不引 `windows-sys`） |

`ConnectError` 三類：`ServerNotRunning`（目標不存在／拒絕連線／子程序在送出任何資料前就
結束）、`Spawn`（子程序無法啟動）、`Io`（其他 I/O 錯誤）。`ConnectError::from_io` 依
`io::ErrorKind::NotFound`／`ConnectionRefused` 判定 `ServerNotRunning`，其餘歸 `Io`。

`ChildStdioConnector` 的 `ServerNotRunning` 判定時機（design D3；Codex fix round 1
finding 1、finding 2 修正後）：`connect()` 只 spawn 不等待；`send_line`／`recv_line`
共用同一套判定——「還沒收到過任何一行、且子程序已用 `try_wait`／短暫 `wait` 確認已結束」
時，把錯誤轉成 `io::ErrorKind::ConnectionRefused`，訊息附上 stderr 尾端內容：

- `recv_line()` 走到乾淨 EOF 時判定。已經收過至少一行後的 EOF 不再無條件視為正常結束
  （Codex 最終 review 二次確認 finding，取代全分支最終 review finding 3 的原始版本）：
  最多等 `POST_STREAM_EXIT_WAIT`（500ms）讓子程序結束狀態追上來——exit 0 才是正常
  `Ok(None)`；非零結束或 500ms 內仍未結束，都回 `io::ErrorKind::ConnectionAborted`
  （後者訊息為「bridge closed stdout but did not exit within 500ms」），不會把「橋接
  程序先關 stdout、之後才因故障退出」誤判成對面正常關閉連線。`try_wait`／`wait`
  本身失敗（OS／process-handle 層錯誤，非逾時）另分一類，訊息為
  「failed to wait for bridge after stdout EOF: {原始錯誤}」，不會被併入逾時那個
  固定訊息（fix round 3 finding：舊版用 `if let Ok(Some(status))` 靜默略過
  `try_wait` 的 `Err`，wait 錯誤因而被誤報成逾時，底層失效原因遺失）。
- `send_line()` 走到 `BrokenPipe`／`ConnectionReset`（子程序在我們寫入前就已關閉 stdin
  讀取端）時也判定同一套邏輯，不會把「子程序已死」誤報成普通 `Io` 錯誤（fix round 1
  finding 1：spec 的 Scenario 是先送一行、再讀一行，實測子程序已確定死亡後
  `send_line` 100% 撞到 `BrokenPipe`，不分類會誤判）。
- 組錯誤訊息前，會先（至多等 200ms）`await` stderr 背景 task 的 `JoinHandle`，確保
  tail 已經讀完才組訊息，不會因為背景 task 還沒被排到而回一個空的 stderr 內容
  （fix round 1 finding 2：這個修正是防禦性的——在本機 Windows + tokio
  current-thread／multi-thread runtime 上實測 140+ 次都沒能讓舊版「空 tail」重現，
  因為子程序在程式順序上一定先寫 stderr、才呼叫 `process::exit`，reactor 幾乎必然
  先送 stderr 就緒事件；但正確性不該依賴這個未保證的排程順序，所以仍然套用修正）。

### 本機預設 socket 路徑（`default_socket_path`）

```text
HERDR_SOCKET_PATH 優先
  → 其次 HERDR_SESSION 對應 <config_dir>/sessions/<name>/herdr.sock
  → 否則 <config_dir>/herdr.sock
```

`default_socket_path(lookup_env, config_dir)` 是純函數，環境變數與設定目錄以參數注入，
方便測試不改真實環境；`default_socket_path_from_env()` 包一層讀真實環境變數與平台設定
目錄，設定目錄的規則對齊 HERDR 自己的 `config_dir()`／`platform_config_dir()`（Codex
fix round 1 finding 4 查證定案，取代先前「待查證」的猜測；來源：clone
<https://github.com/herdrdev/herdr>，commit `bafbc0949dd996cf7fd0848c8965e254348cc11e`，
`src/config/io.rs:22-68`）：

1. `XDG_CONFIG_HOME`——**不分平台**，只要這個環境變數有設定（含空字串，HERDR 原始碼沒有
   排除空字串）就優先採用，接上 `herdr`。這跟先前假設的「只有 unix 吃 XDG」不同：HERDR
   在 Windows 上一樣先看這個變數。
2. 否則依平台 fallback：
   - Windows：`APPDATA` → `USERPROFILE\AppData\Roaming` → `HOME`（接 `.config/herdr`，
     即使在 Windows 分支，HOME fallback 仍然是 unix 風格的 `.config` 子目錄）→
     `std::env::temp_dir()`。
   - unix：`HOME`（`.config/herdr`）→ `std::env::temp_dir()`。

跟 HERDR 原始碼一樣不會「找不到就放棄」：兩邊都查不到時退到系統暫存目錄，所以
`default_socket_path_from_env()` 回傳 `PathBuf` 而非 `Option<PathBuf>`。另外，HERDR 的
`app_dir_name()`（`io.rs:22-28`）在自己的 debug build 會用 `"herdr-dev"` 而非 `"herdr"`；
本 crate 假設目標 HERDR 是一般發行版，固定用 `"herdr"`，不處理對方是 debug build 的情況。

不重現的細節：真機 HERDR（`session.rs` `active_name()`）會把字面值 `HERDR_SESSION=default`
或未通過 `validate_name` 的 session 名稱當成「沒有 session」；`default_socket_path` 沒有做
這個過濾，屬於本 change 刻意簡化的範圍（design 沒把它列進來）。

## 假對端（`ChildStdioConnector` 的測試用子程序）

`design.md` Open Questions 列了兩個候選：`[[bin]]` + `required-features` 的假 stdio server，
或平台 shell 單行。實測結果：**採用 `[[bin]]` + `required-features` 可行**——

```toml
[[bin]]
name = "herdr-client-test-child"
path = "src/bin/test_child.rs"
required-features = ["test-support"]

[dev-dependencies]
herdr-client = { path = ".", features = ["test-support"] }
```

`cargo test -p herdr-client` 會自動建置這個 bin（靠自己對自己的 dev-dependency 開
`test-support` feature），整合測試用 `env!("CARGO_BIN_EXE_herdr-client-test-child")` 取得
執行檔路徑；`cargo build -p herdr-client`（不含 `--tests`）不會建它，因為 `test-support`
沒有在正常建置時被啟用——與 design 的風險列表「edition 2024 預設 resolver 已隔離
dev-dependency 的 feature」一致，不需要退回 `cfg(test)` + `tests/common` 的替代方案。

`herdr-client-test-child` 依 argv\[1\] 分三種模式：

- `echo`：先在 stderr 印一行 `noise`（驗證 stderr 不混入 stdout），之後每讀一行 stdin 就在
  stdout 回一行 `{"echo":"<原行>"}`，直到 stdin EOF。
- `exit-immediately <code> <stderr文字>`：把 `<stderr文字>` 印到 stderr 後以 `<code>` 結束，
  不讀 stdin（模擬橋接目標不存在、子程序立即結束）。
- `exit-after-sleep <code> <stderr文字> <sleep_ms>`：把 `<stderr文字>` 印到 stderr 後睡
  `<sleep_ms>` 毫秒才以 `<code>` 結束（Codex fix round 1 finding 2：驗證 stderr 寫入與
  process exit 之間有明顯間隔的排程情境）。
- `sleep <secs>`：睡滿 `<secs>` 秒才結束，用來測 `kill_on_drop`。

Windows／unix 的 named pipe／unix socket 假對端則直接在 `tests/transport.rs` 內用 tokio 的
`ServerOptions`／`UnixListener` 開一次性或可依序接受多條連線的假 echo server，不需要額外
crate 或跨檔共用模組。

`ChildStdioStream::wait_for_exit(timeout)` 是只在 `test-support` feature 下編譯的
`#[doc(hidden)]` 測試專用方法（Codex fix round 2 finding B）：用 `Child::wait` 實際
poll／回收子程序的結束狀態，取代測試裡「先確定子程序死了才 send→recv」那段原本用外部
`tasklist`／`kill -0` 輪詢 pid 的邏輯——Unix 上已退出但還沒被 `wait`／`try_wait` 回收的
zombie 仍會通過 `kill -0`，會讓那段等待邏輯誤判成「還活著」。`process_exists`（`tasklist`／
`kill -0`）仍保留給 `child_killed_on_drop` 用：那個情境下 `ChildStdioStream` 已經被丟棄、
沒有 `Child` 物件可以 `wait`，只能用外部 OS 層檢查。

## 型別

`herdr_client::types` 只塞觀測所需的欄位子集，對 schema 有但這裡不需要的欄位一律忽略
（不用 `deny_unknown_fields`）；下面只列型別做什麼，欄位細節見各檔案的 doc comment。

| 檔案 | 型別 | 說明 |
|---|---|---|
| `agent_status.rs` | `AgentStatus` | 五值：`Idle`／`Working`／`Blocked`／`Done`／`Unknown`（`#[serde(other)]`，未知字串落這裡，不讓解析失敗）。**`Done` 只代表「已 idle 且尚未被看過」，不是任務完成**——型別本身不提供「是否完成」的判斷方法，上層要衍生完成語意得自己在 Domain 層定規則 |
| `snapshot.rs` | `SessionSnapshot` | `session.snapshot` 的 `result.snapshot`：`version`／`protocol`，四個平行陣列 `workspaces`／`tabs`／`panes`／`agents`（`Vec<WorkspaceInfo>` 等；彼此靠 id 字串互相參照，不是巢狀結構），`layouts: Vec<serde_json::Value>`（刻意不建模，change 1 不畫 pane 幾何配置），三個 `focused_*` 選填欄位 |
| `snapshot.rs` | `WorkspaceInfo`／`TabInfo` | 欄位全部 schema 必填，缺一個就解析失敗 |
| `snapshot.rs` | `PaneInfo` | `agent`／`title`／`terminal_title`／`cwd`／`label` 是選填（nullable）欄位，其餘必填 |
| `snapshot.rs` | `AgentInfo` | `agent` 選填，其餘必填 |
| `pane_read.rs` | `ReadSource`／`ReadFormat` | `pane.read` 的 `source`（`Visible`／`Recent`／`RecentUnwrapped`／`Detection`）與 `format`（`Text`／`Ansi`，封閉 enum，讓「送出 schema 不接受的字串」在編譯期就不可能） |
| `pane_read.rs` | `PaneReadParams`／`PaneReadResult` | `pane.read` 的請求參數（`format`／`lines`／`strip_ansi` 缺席時完全不序列化該欄位，不送 `null`）與回應（8 個欄位皆必填）；change 1 不對真機呼叫，供 change 3 用 |
| `events.rs` | `Subscription` | `events.subscribe` 的一筆訂閱項目：24 個無參數的生命週期訂閱（`type` 用點號命名，如 `{"type":"tab.created"}`）＋一個帶 `pane_id` 的 `PaneAgentStatusChanged { pane_id }`；`Subscription::all_lifecycle()` 回傳全部 24 種 |
| `events.rs` | `EventKind`（26 種）／`SubscriptionEventKind`（3 種） | 底線命名（如 `pane_created`）／點號命名（如 `pane.agent_status_changed`）的已知事件名稱集合；`FromStr` 成功代表落在集合裡，失敗交給呼叫端（`client::IncomingEvent::Unknown`）保留原始字串——兩者都不加 `#[serde(other)]`，未知名稱不會被靜默吃掉 |
| `events.rs` | 16 個事件 payload 型別 | 依 `event` 名稱挑對應型別解析 `data`（`WorkspacePayload`、`TabPayload`、`PanePayload`、`WorkspacesReplacedPayload`、`TabMovedPayload`、`WorkspaceClosedPayload`、`WorkspaceRenamedPayload`、`TabClosedPayload`、`TabRenamedPayload`、`PaneClosedPayload`、`WorkspaceFocusedPayload`、`TabFocusedPayload`、`PaneFocusedPayload`、`PaneMovedPayload`、`PaneAgentDetectedPayload`、`PaneAgentStatusChangedPayload`），完整對照表見 `tests/events.rs` |
| `wire.rs` | `RequestEnvelope`／`ResponseEnvelope`／`ErrorBody`／`EventEnvelope` | 送出的 `{"id","method","params"}`、收到的 `{"id","result"?,"error"?}`（`ErrorBody` 的 `code`／`message` 皆必填，缺一即整個 `error` 解析失敗）、收到的一行事件 `{"event","data"}` |
| `wire.rs` | `SessionSnapshotResult`／`PaneReadResultEnvelope`／`SubscriptionStarted` | 各自檢查 `result.type` 標籤是否符合預期（不符即解析失敗）；這些是 `client` 模組的實作細節，一般呼叫端只會用到下一節的 `Client` 與 request／event 型別 |

## Client 與假 HERDR

### 公開 API 一覽（`herdr_client::client`）

```rust
pub struct Client { /* ... */ }
impl Client {
    pub fn new(connector: Arc<dyn Connector>) -> Self;
    pub async fn request<R: Request>(&self, req: R) -> Result<R::Response, RequestError>;
    pub async fn subscribe(&self, subs: &[Subscription]) -> Result<EventStream, RequestError>;
}

pub trait Request: private::Sealed + Serialize {
    const METHOD: &'static str;
    type Response: DeserializeOwned;
}
pub struct SessionSnapshotRequest {}            // METHOD = "session.snapshot"
pub struct PaneReadRequest(pub PaneReadParams); // METHOD = "pane.read"（change 1 不呼叫）

pub struct EventStream { /* ... */ }
impl EventStream {
    pub async fn next(&mut self) -> Option<Result<IncomingEvent, StreamError>>;
}
pub enum IncomingEvent {
    Lifecycle(EventKind, serde_json::Value),
    PerPane(SubscriptionEventKind, serde_json::Value),
    Unknown { event: String, data: serde_json::Value },
}

pub enum RequestError { Connect(ConnectError), Remote { code: String, message: String, response_id: String }, Protocol(String), Io(std::io::Error) }
pub enum StreamError { Io(std::io::Error) }
```

`Client::request` 對每次呼叫開一條全新連線（不重試、不設逾時——這是連線迴圈〔change 1b〕的
責任）：送出一行 request、讀一行回應、解析、連線在函式結束時 drop。`Client::subscribe` 流程
類似，但讀到 `subscription_started` 後把連線包成 `EventStream` 交給呼叫端持續讀取事件，不會
主動關閉連線。

### `Request` 是 sealed

`Request` trait 只有本 crate 內的型別能實作（`private::Sealed` 定義在 `pub(crate) mod
private`，crate 外部看不到、也就無法實作）。這是刻意的安全邊界：HERDR 的 socket API 沒有
認證（ADR-0001），如果任何外部型別能自己實作 `Request`，就能自訂任意 `METHOD`（例如
`"server.stop"`、`"agent.prompt"`）交給 `Client::request` 送出，直接繞過「只提供 observer
子集」這個限制。目前公開的 request 型別只有兩種：`SessionSnapshotRequest`、
`PaneReadRequest`；`events.subscribe` 走專門的 `Client::subscribe` 方法，不透過 `Request`——
它內部用的 `EventsSubscribeRequest` 型別雖然也實作了 `Request`（sealed 邊界要求任何實作都要
有具體型別），但是 `pub(crate)`、不對外公開也不 re-export（全分支最終 review finding 2）：
外部呼叫端連命名它都做不到，也就不可能自己呼叫 `client.request(EventsSubscribeRequest(..))`
繞過 `subscribe` 的「訂閱建立成功才把連線包成 `EventStream`」語意。

### `EventStream::next` 語意

- 正常 EOF → `None`。
- I/O 錯誤 → 先回一次 `Some(Err(..))`，之後一律 `None`（不重複回報同一個錯誤，也不會在
  錯誤之後再嘗試讀取底層連線）。
- 壞行（非 JSON、或缺 `event`／`data` 欄位、或 `data` 不是 JSON 物件）→ 記
  `tracing::warn!` 後跳過，繼續讀下一行，不算一次呼叫結果（不中斷串流）。
- 不設逾時、不重連——這些是連線迴圈（change 1b）的責任；刻意不實作 `futures::Stream`
  trait，只提供這一個 inherent async 方法，避免為此另外引入 `futures` 依賴。

### `FakeHerdr`（`test-support` feature）

`herdr_client::testing::FakeHerdr` 是程序內、監聽真實 transport（Windows named pipe／unix
socket）的假 server，只在 `test-support` feature 開啟時編譯；本 crate 以 self dev-dependency
開這個 feature 測自己（機制同上面「假對端」節）。

```rust
use std::sync::Arc;
use herdr_client::client::{Client, SessionSnapshotRequest};
use herdr_client::testing::{FakeHerdr, FakeHerdrConfig, Step};

let config = FakeHerdrConfig::new()
    .with_snapshot_result(serde_json::json!({"type": "session_snapshot", "snapshot": { /* .. */ }}))
    .with_subscribe_script(vec![Step::Event(r#"{"event":"tab_created","data":{}}"#.into()), Step::Hold]);
let fake = FakeHerdr::start(config).await?;

let client = Client::new(Arc::from(fake.connector()));
let result = client.request(SessionSnapshotRequest {}).await?;
```

`FakeHerdrConfig` 用 builder 風格組裝：`with_method_response`（單一回應，每次呼叫都回同一個）／
`with_method_responses`（依呼叫序回應：第 n 次收到該 method 的 request 回第 n 筆，用完最後一筆
後持續重複，跨連線累積計數；用於 Drift 重拿 `session.snapshot` 要回不同內容的測試——對同一個
method，這兩個 builder 後呼叫者勝出）／`with_snapshot_result`／`with_snapshot_fixture_line`
（直接吃 `tests/fixtures/snapshot-p22.json` 這類整行 fixture，取其 `result` 欄位）、
`with_subscribe_script`（不分訂閱內容一律重播同一組 `Step`）／`with_subscribe_rule`（依
`SubscribeMatcher` 讓不同連線依訂閱內容收到不同腳本）、`with_failing_probe_pane_ids`
（design D12：讓指定 `pane_id` 的訂閱探測失敗，回 `error`）。
`Step` 描述一條 `events.subscribe` 連線在 `subscription_started` 之後依序執行的步驟：
`Event`（推一行合法事件）／`Malformed`（推一行壞資料）／`Delay`／`Close`（乾淨關閉）／
`Abort`（非乾淨中斷，平台行為不同見 `src/testing/script.rs`）／`Hold`（掛著直到對端關閉
連線、或 `FakeHerdr` 被 drop）。`fake.received()` 回傳每條連線收到的行，
`fake.closed_connections()` 回傳每條連線在假 HERDR 這端是否已關閉（索引與 `received()`
對齊，供「失敗時已開的連線要關閉」「釋放事件流時連線要關閉」這類驗證用；對端關閉是非同步
觀察到的，要輪詢加逾時），`fake.endpoint_path()` 回傳端點路徑，`fake.connector()` 回傳指向
這個假端點、可直接交給 `Client::new` 的 `Connector`。

## 真機測試

`herdr-client/tests/real_herdr.rs`（`#![cfg(windows)]`）全部 `#[ignore]`，需要真機 HERDR 在
跑才執行；全部改寫成用 `Client`（不再手動組 named pipe／`wsl.exe -e nc` 的逐行讀寫，那是
第 1 組 spike 時代、`Client` 還不存在時的做法）。目標由以下環境變數指定（省略則用內建
預設值，design D11）：

| 環境變數 | 用途 | 省略時的預設 |
|---|---|---|
| `HERDR_CLIENT_TEST_WIN_SOCKET` | Windows 端 HERDR API socket 路徑 | `default_socket_path_from_env()` |
| `HERDR_CLIENT_TEST_WSL_DISTRO` | WSL 發行版名稱 | `Ubuntu-24.04` |
| `HERDR_CLIENT_TEST_WSL_SOCKET` | WSL 端 HERDR API unix socket 路徑 | 由 WSL 的 `$HOME` 推得：`<HOME>/.config/herdr/herdr.sock`（取不到時測試失敗並提示設定本變數） |
| `HERDR_CLIENT_TEST_ALLOW_WSL_WRITES` | 是否允許在 WSL 端 workspace `wD` 建立/操作/關閉測試 tab | 未設定＝不允許（唯讀，寫入測試印訊息後直接跳過並算通過） |

執行方式（需要對應端的 HERDR server 正在跑，全程只送 `session.snapshot`／
`events.subscribe`，不執行任何會終止 server 或改變既有 tab／pane 的操作）：

```sh
# 唯讀模式：7 個測試都跑，需要寫入的 2 個印訊息後跳過
cargo test -p herdr-client --test real_herdr -- --ignored --test-threads=1

# 加 opt-in：額外在 WSL 端 workspace wD 建立/關閉自己的測試 tab
HERDR_CLIENT_TEST_ALLOW_WSL_WRITES=1 cargo test -p herdr-client --test real_herdr -- --ignored --test-threads=1
```

WSL 端測試 server 沒在跑時（`wsl.exe -d Ubuntu-24.04 -e bash -lc "~/.local/bin/herdr status
server"` 顯示 not running），用下面指令啟動（這是測試專用的 headless server，不是使用者
日常用的那個）：

```sh
wsl.exe -d Ubuntu-24.04 -e bash -lc "setsid -f ~/.local/bin/herdr server >/tmp/herdr-server.log 2>&1 </dev/null"
```

### p22 事件擷取工具（`examples/capture_events.rs`）

對 Windows 端 HERDR 直接用 `Connector`／`NdjsonStream`（不透過 `Client`／`EventStream`）開
兩條連線——L（24 種生命週期訂閱）與 S（對 `session.snapshot` 全部 pane 的
`pane.agent_status_changed` 訂閱）——把 `recv_line()` 讀到的每一行**逐字**寫進
`*.raw.ndjson`，供之後用 `docs/research/2026-09-13/deidentify-fixture.py --ndjson` 去識別化
成 `tests/fixtures/events-lifecycle-p22.ndjson`／`events-status-p22.ndjson`：

```sh
cargo run -p herdr-client --example capture_events -- --seconds 30
# 預設輸出到 target/capture/events-{lifecycle,status}-p22.raw.ndjson；--out <dir> 可改路徑

python docs/research/2026-09-13/deidentify-fixture.py \
  target/capture/events-lifecycle-p22.raw.ndjson \
  herdr-client/tests/fixtures/events-lifecycle-p22.ndjson \
  --ndjson
```

**`*.raw.ndjson` 是 wire 原貌，`tests/fixtures/events-*-p22.ndjson` 是去識別化後的重新
序列化**（task 5.1 fix round 1 finding 1）：擷取工具刻意不透過 `Client::subscribe` /
`EventStream::next`——那條路徑會把每一行解析成 `IncomingEvent` 再重建 JSON，空白、key
順序、頂層欄位都可能因此改變，而且壞行會被 `EventStream` 直接跳過，讓 `*.raw.ndjson`
沒辦法當作「真機協定原貌」或「schema drift」的可靠證據。改成手動組
`RequestEnvelope { method: "events.subscribe", .. }` 送出、`recv_line()` 讀到什麼字串就
寫什麼字串（只另外解析一份副本印 stderr 摘要，解析失敗照樣把原始行寫進 raw 檔並印
`malformed`）。`deidentify-fixture.py` 產生 `tests/fixtures/` 那份時當然還是要
`json.load`／`json.dump`（否則沒辦法替換敏感欄位），這一步的重新序列化沒問題——要避免的
是「還沒去識別化就先被序列化改樣貌」。

**擷取截止不是直接 `timeout` 取消讀取**（task 5.1 fix round 2 finding 1）：`--seconds`
到期時如果直接用 `tokio::time::timeout` 包住 `recv_line()`，逾時會取消這個 future——
`recv_line()` 底層是 `AsyncBufReadExt::read_line`，取消可能讓已經從底層 buffer `consume()`
掉、但還沒組成完整行的位元組憑空消失，跟「raw 擷取不能漏掉截止邊界最後一行」的目標衝突。
改成每條連線一個獨立 reader task，只做「`recv_line().await` 讀到完整行就送進
`tokio::sync::mpsc` channel」，`recv_line()` 本身從不被取消；核心邏輯
`capture_with_deadline` 只決定「還要不要繼續消耗 channel」：`--seconds` 到期後還有 200ms
的 grace，這段時間內送進 channel 的行照樣收下，grace 結束才真正停止。

**停止的那一刻不是單純 `break`**（task 5.1 fix round 3 finding 1）：`tokio::select!` 在多個
分支同時 ready 時是隨機選一個，不是「先看哪個先到」——就算某一行早就送進 channel、
`rx.recv()` 已經 ready，只要那一輪 poll 時計時器也剛好到期，`select!` 仍有機會選中計時器
分支直接結束，讓已經排隊的行被漏收。改法：計時器分支被選中時，先用 `rx.try_recv()`
（不等待，只問「現在有沒有」）把當下已經排隊的行收乾淨，才真正停止；`capture_with_deadline`
也不再靠「呼叫端 drop channel 接收端、reader task 下一次送失敗就自然結束」這種間接、時間點
不確定的終止方式——回傳前明確 `abort()` reader task 的 `JoinHandle` 再 `.await` 它，確保
函式回傳的當下連線一定已經真正 drop（即使 reader task 當時正卡在 `recv_line().await`，例如
對端只回完 `subscription_started` 就掛著不再送任何東西）。**保證的是**：「換行在
deadline + grace 之前送進 channel 的行必定被收下，不管 `select!` 那一輪運氣好不好」；
deadline + grace 那一刻仍在傳輸中、還沒組成完整行的半行本來就沒機會進 channel，不算漏記。

**擷取失敗不會留下半套的 raw 檔**（task 5.1 fix round 4 finding 1）：reader task 若 panic
（任何 `recv_line()` 實作的 bug 都可能造成），這次擷取到的事件集就不完整了。
`capture_with_deadline` 因此回傳 `Result<Vec<String>, CaptureError>`——只有
`JoinError::is_cancelled()`（我們自己 `abort()` 的結果）算預期，其他 `JoinError` 一律轉成
`CaptureError::ReaderFailed`（帶著原本的 panic 訊息），已經收到的那幾行不會被當成成功結果
交出去。`main()` 兩條連線只要任一條回 `Err`，就印出失敗原因、**兩份 `*.raw.ndjson` 一個都
不寫**、以非 0 結束。擷取工具的產出是要拿去當 fixture 的，寫出一份看起來正常、其實少了事件
的檔案比直接失敗糟糕得多——反過來說，`*.raw.ndjson` 存在就代表那次擷取的兩條連線都跑完了。

`capture_with_deadline`（連同抽出的 `drain_until`）不碰檔案系統，`examples/capture_events.rs`
底下的 `#[cfg(test)] mod tests` 用 `herdr_client::testing::FakeHerdr` 控制時序，加上直接餵
`drain_until` 一個「一開始就已經是 Ready」的計時器與一次塞進 64 筆訊息（讓「`select!`
連續 64 輪都沒有輪到 poll 計時器」的機率趨近於零）逼近確定性地重現這個競態，總共 5 個測試
（另有一個用 drop-flag 的測試替身驗證回傳前連線確實已經 drop，一個用「第一次 `recv_line()`
正常回一行、第二次就 panic」的測試替身驗證擷取失敗時呼叫端拿不到成功結果）。
`herdr-client/Cargo.toml` 的 `[[example]] name = "capture_events" test = true` 讓
`cargo test -p herdr-client` 直接建置並執行這些測試，不需要額外呼叫
`cargo test --example capture_events`（已實測確認，也確認過
`cargo build -p herdr-client --examples` 這種一般建置不受影響）。

`events-status-p22.raw.ndjson` 需要真的有 pane 的 agent 狀態在擷取期間變化才會有內容
（例如某個 agent pane 從 working 變 idle）；擷取期間都沒有變化的話這個檔案會是空的——這不
是錯誤，只是代表這次沒抓到，不要用空檔案去產生 `tests/fixtures/events-status-p22.ndjson`。
`tests/fixtures/events-lifecycle-p22.ndjson`（Windows 端真機擷取，全部 `pane_updated`）與
`events-status-p22.ndjson`（2 行 `pane.agent_status_changed`：`wJ:p1` 從 `done` 變
`working`，橫跨對話回合交界擷取到）都已經交付進 repo，`tests/contract.rs`／
`tests/events.rs` 對兩者的驗證都是無條件的（缺檔就直接測試失敗，task 5.1 fix round 1
finding 2、fix round 2）。
