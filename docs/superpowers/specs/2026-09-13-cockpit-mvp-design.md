# AI Agent Cockpit — MVP 架構設計

> Status: Draft for review（v2，已依找碴審閱修訂）
> Date: 2026-09-13
> 上游文件：`docs/cockpit-spec.md`（概念與方向，2026-09-10）
> 詞彙：`CONTEXT.md`；決策理由：`docs/adr/`；查證證據：`docs/research/2026-09-13/`
>
> 本文件把 spec 落成可實作的架構，並把 MVP 第一片（`herdr-client` 與
> `attach-herdr-runtimes` 兩個 change）寫到可以直接產 tasks 的程度。change 2、3 只寫
> 從第一天就必須遵守的約束。

## 1. 範圍與切片

MVP 依 spec §20 的 13 個必做項切成三個功能切片；第一片因工作量拆成兩個 OpenSpec change，
每個 change 都能獨立驗收：

| change | 名稱 | 涵蓋 spec §20 項目 | 驗收 |
|---|---|---|---|
| 1a | `herdr-client` | 1、2、3 的協定層；§11 全部 spike | contract test、假 HERDR、真機 `#[ignore]` 測試 |
| 1b | `attach-herdr-runtimes` | 4、5、8、10、13，以及 12 的 agent 層視覺（Working／Blocked／Idle／Done／Unknown） | Scenario A Attach、B Live state、F Reconnect |
| 2 | `pipeline-projection` | 6、7、9，以及 12 的 domain 層視覺（Failed／Completed） | Scenario C、D |
| 3 | `live-output` | 11 | Scenario E |

spec §20 item 12 的「Failed」在 change 1 沒有對應概念，因為 HERDR 的 agent 狀態沒有失敗值；
它屬於 change 2 的 `StageStatus`。MVP 完成後再評估的包裝項目：Tauri 桌面殼（ADR-0005）。

## 2. 已查證的 HERDR 事實（查證日期 2026-09-13）

來源優先序：本機 `herdr api schema --json` 匯出的 schema（一手）＞ HERDR 原始碼
（GitHub `herdrdev/herdr`，clone commit `bafbc0949dd996cf7fd0848c8965e254348cc11e`，
Cargo.toml version 0.9.0；此 commit 可能比本機安裝的 preview 新數日）＞
官網 <https://herdr.dev/docs/socket-api/>。原始碼行號以該 commit 為準。

證據檔案（`docs/research/2026-09-13/`）：

| 檔案 | 內容 |
|---|---|
| `herdr-schema-windows-0.9.0-preview-protocol22.json` | Windows 端 `herdr api schema --json` 原始輸出 |
| `herdr-schema-wsl-0.8.2-protocol20.json` | WSL 端同上 |
| `herdr-schema-findings.txt` | schema 萃取報告（methods、events、AgentStatus、pane.read、metadata、snapshot 形狀） |
| `herdr-source-findings.txt` | 原始碼引用報告（transport、handshake、每連線一 method、路徑規則、事件雙軌） |
| `herdr-schema-compare.sh`、`herdr-schema-compare-output.txt` | 兩版 schema 的 jq 比對腳本與輸出 |
| `herdr-status-server-windows.txt` | `herdr status server` 原始輸出 |

### 2.1 本機環境

| 側 | HERDR 版本 | protocol | socket | Rust |
|---|---|---|---|---|
| Windows 11 | 0.9.0-preview.2026-09-08-62431dbd033b | 22 | `C:\Users\<user>\AppData\Roaming\herdr\herdr.sock` | stable 1.97.1 msvc |
| WSL Ubuntu-24.04 | 0.8.2 | 20 | `/home/<user>/.config/herdr/herdr.sock` | stable 1.98.0 |

版本字串來自本機 `herdr --version`、`cargo --version` 實測（原始輸出見
`docs/research/2026-09-13/local-checks.txt`）。兩個 server 互不相通，WSL 端 server 平時不一定在跑。

### 2.2 Transport 與連線模型

- **框架**：newline-delimited JSON，一行一則。request／success_response／error_response
  三種訊息都以 `id`（string）配對。server 只對每條連線的第一行設 1 MiB 上限
  （`src/api/server.rs:32`）。
- **沒有 handshake、沒有認證**：accept 後直接讀第一行當 request 解析
  （`src/api/server.rs:156-176`）。log 裡的 `endpoint handshake` 屬於另一條
  `herdr-client.sock`（二進位 length-prefixed），與 API socket 無關。
- **每條連線只服務一個 method**（`src/api/server.rs:93-122`）：一般 request 回一行後
  server 關閉連線；`events.subscribe` 回 `subscription_started` 後持續推送事件，
  且不再讀取 client 輸入。client 的形狀因此是「長連線收事件 ＋ 每次請求開一條新短連線」。
- **Windows**：Win32 named pipe，pipe 名稱就是 socket 檔完整路徑，即
  `\\.\pipe\C:\Users\<user>\AppData\Roaming\herdr\herdr.sock`（`src/ipc.rs:70-71` 經
  `interprocess` 2.4.2 的 `GenericNamespaced` 轉換；本機以
  `[System.IO.Directory]::GetFiles("\\.\pipe\")` 實測看到該名稱）。`herdr.sock` 檔案內容
  `<pid>:<納秒時間戳>` 只是 server 自用的過期標記，client 端從不解析
  （`src/api/client.rs:96-98`）。
- **Linux／macOS**：AF_UNIX socket，路徑同 §2.1。
- **路徑規則**（`src/session.rs:173-181`，各平台共用）：CLI `--session` 明示 ＞
  環境變數 `HERDR_SOCKET_PATH`（`src/api/mod.rs:20`）＞ `HERDR_SESSION` 對應的
  `sessions/<name>/herdr.sock` ＞ 預設 `<config_dir>/herdr.sock`。沒有 config key 可改。
- **「server 沒在跑」的判定**：連線錯誤的 `io::ErrorKind` 為 `NotFound` 或
  `ConnectionRefused`（`src/cli.rs:821-829`）。
- **沒有序號、cursor、replay**：`EventEnvelope` 只有 `{event, data}`；
  `EventsSubscribeParams` 只有 `subscriptions`。斷線後只能重拿 snapshot。

### 2.3 事件與訂閱

`events.subscribe` 的 `Subscription` 有 27 種（`src/api/schema/events.rs:18-85`）：

- **24 種無參數的生命週期訂閱**：workspace 8 種（created、updated、metadata_updated、
  renamed、moved、reordered、closed、focused）、worktree 3 種（created、opened、removed）、
  tab 5 種（created、closed、focused、renamed、moved）、pane 7 種（created、closed、
  updated、focused、moved、exited、agent_detected）、layout 1 種（updated）。推送 frame 為
  `{"event": "<snake_case>", "data": {...}}`。
- **3 種每 pane 的訂閱，`pane_id` 必填**：`pane.output_matched`、
  `pane.agent_status_changed`（另有可選的 `agent_status` 過濾）、`pane.scroll_changed`。
  推送 frame 為 `{"event": "pane.xxx", "data": {...}}`，點號命名。訂閱時 server 會先
  `pane.get` 探測該 pane，探測失敗整個 subscribe 請求失敗（`src/api/subscriptions.rs:205-231`）。

**沒有全域的 agent 狀態訂閱。** `EventKind` 26 種裡的 `pane_agent_status_changed` 與
`pane_output_changed` 沒有無參數訂閱可收。agent 狀態改變時 server 只發生命週期版的
`pane_agent_status_changed`（`src/app/api.rs` 的 `emit_pane_state_update`）。`pane.updated`
由多個呼叫點發出（agent 名稱改變、標題改變、metadata token 更新、agent 和解等，皆為
呈現資訊），**狀態改變不會觸發它**。因此要即時知道任一 pane 的狀態變化，只能對每個 pane
各訂一筆 `pane.agent_status_changed`。這是本設計 §4.2 與 §7.1 的直接依據。
另外，**pane 的 cwd 改變沒有任何事件**，只能靠定期 snapshot 更新。

判別規則：`event` 字串含 `.` 即每 pane 訂閱的推送。`pane.agent_status_changed` payload 兩軌相同：
`pane_id*`、`workspace_id*`、`agent_status*`、`agent`、`display_agent`、`state_labels`、`title`。

**L 訂閱建立瞬間的補推**（change 1b task 4.4 查證，2026-09-15）：WSL 0.8.2 會在 authoritative
snapshot 前推送少量剛發生過的事件（實測 2 筆），已由 §4.2「5 之前從 L／S 收到的事件一律丟棄」
的規則吸收；Windows 0.9.0 **不補推**——使用者在 Windows 端剛開關兩三個 tab 之後幾秒內建立 L 訂閱並
擷取 20 秒，只收到既有 Sidebar pane 的 `pane_updated`，沒有任何指向已關 tab／pane 的事件（證據見
`docs/research/2026-09-15/change-1b-acceptance.md` task 4.4 節第 3 點）。丟棄規則對兩側都保留，
不因 Windows 端不補推而簡化。

**Payload 完整性**（`src/api/schema/events.rs` 的 `EventData`）：

| 事件 | payload |
|---|---|
| `workspace.created`、`workspace.updated`、`workspace.metadata_updated` | 完整 `WorkspaceInfo` |
| `workspace.moved`、`workspace.reordered` | `workspaces: Vec<WorkspaceInfo>`（整份順序） |
| `workspace.closed` | `workspace_id`，可選舊 `WorkspaceInfo` |
| `workspace.renamed` | `workspace_id`、`label` |
| `worktree.*` | `WorkspaceInfo` 加 `WorktreeInfo` |
| `tab.created` | 完整 `TabInfo` |
| `tab.closed`、`tab.renamed` | id（renamed 另帶 `label`） |
| `pane.created`、`pane.updated` | 完整 `PaneInfo` |
| `pane.moved` | `previous_pane_id`、`previous_workspace_id`、`previous_tab_id` 加新的完整 `PaneInfo`；**pane id 在移動後會變**。另有 4 個選填欄位，只在移動跨 workspace／tab 邊界時出現、缺席為 `None`：`created_workspace: Option<WorkspaceInfo>`（移動同時新建了 workspace）、`created_tab: Option<TabInfo>`（移動同時新建了 tab）、`closed_workspace_id: Option<String>`（移動後原 workspace 被關閉）、`closed_tab_id: Option<String>`（移動後原 tab 被關閉） |
| `tab.moved` | `tab_id`、`workspace_id`、`insert_index`、`tabs: Vec<TabInfo>`（整份順序，與 `workspace.moved` 同構） |
| `pane.focused`、`tab.focused`、`workspace.focused` | id |
| `pane.closed`、`pane.exited` | `pane_id`、`workspace_id` |
| `pane.agent_detected` | `pane_id`、`workspace_id`、`agent: Option<String>`、`released: bool`（agent 離開時 `released` 為 true）。另有選填欄位 `final_status: Option<AgentStatus>`，值域同 §2.4 五值（`idle`／`working`／`blocked`／`done`／`unknown`），缺席為 `None`；語意（推測為 agent 離開瞬間的最後狀態）schema 與程式碼註解皆未進一步說明，由欄位命名推論，未查證 |
| `layout.updated` | `layout: PaneLayoutSnapshot` |

### 2.4 Agent 狀態語意

`AgentStatus` 五值：`idle`、`working`、`blocked`、`done`、`unknown`，出現在
`PaneInfo`、`AgentInfo`、`TabInfo`、`WorkspaceInfo` 四層（tab 與 workspace 為彙總）。

- `idle` 與 `done` 都代表 agent 可接受輸入；`done` 是 server 依「已看過」狀態衍生，
  Cockpit 的讀取不會標記已看。**`done` 不是任務完成的證據。**
- `blocked`：HERDR 辨識到批准或提問 UI。
- `unknown`：有 agent 但無法分類，不證明完成。
- 整合商回報用的 `PaneAgentState` 只有四值（無 `done`），佐證 `done` 為衍生。

### 2.5 Snapshot 形狀

`SessionSnapshot` 是五個平行陣列加外鍵：`workspaces`、`tabs`、`panes`、`layouts`、
`agents`，另有 `version`、`protocol`、`focused_workspace_id`、`focused_tab_id`、
`focused_pane_id`。id 格式：workspace `wJ`、tab `wJ:t1`、pane `wJ:p1`。
本機 2026-09-13 一次 snapshot 為 3,799 bytes（兩個 workspace；`local-checks.txt`）。

### 2.6 讀取 pane 輸出（change 3 用）

`pane.read` params：`pane_id*`、`source*`（`visible`／`recent`／`recent_unwrapped`／
`detection`）、`format`（`text`／`ansi`）、`lines`、`strip_ansi`。回傳單一 `text` 字串、
`revision`（u64）、`truncated`。**沒有 offset、cursor、since**；增量只能靠比對 `revision`。

### 2.7 Metadata（change 2 用）

`pane.report_metadata`：`tokens`（≤16 個 key，key 符合 `^[A-Za-z0-9_-]{1,32}$`）、
`state_labels`（自由 key-value）、`title`、`display_agent`、`ttl_ms`（≤ 24 小時）、
`seq`（樂觀鎖）。snapshot 內 `tokens` 上限 32。本機 snapshot 的 `tokens` 內容是
sidebar plugin 寫入的數值字串（推測為時間戳），**不是 LLM token 用量**。

### 2.8 Windows 與 WSL 版本差異

以 `herdr-schema-compare.sh` 比對兩份 bundled schema（輸出見
`herdr-schema-compare-output.txt`）：Windows 多 11 個 request method（`client_shell.surface.set`、
`command.invoke`、`integration.list`、`pane.copy_motion`、`pane.copy_search`、
`pane.edit_scrollback`、`pane.link.activate`、`pane.scroll`、`pane.selection.read`、
`product_announcement.dismiss`、`release_notes.dismiss`），全是 UI 類。其餘 `EventKind`、
`Subscription`、`AgentStatus`、`ReadSource`、`SessionSnapshot`、`WorkspaceInfo`、`PaneInfo`、
`AgentInfo`、`PaneReadResult`、`PaneReadParams`、`EventsSubscribeParams` 完全相同。
observer 子集在 protocol 20 與 22 是同一份合約。

### 2.9 WSL 端工具與探測

- WSL Ubuntu-24.04 內建 OpenBSD netcat（`/usr/bin/nc`，本機 `nc -h` 顯示 Debian patchlevel
  1.226-1ubuntu2），支援 `-U` 連 unix socket。
- `wsl.exe --list --running --quiet` 在虛擬機停止時執行，前後皆無 `vmmem`／`vmmemWSL`
  程序、兩次輸出皆空，**不會喚醒虛擬機**（本機 2026-09-13 實測，輸出見 `local-checks.txt`）。

## 3. 決策摘要

| # | 決策 | ADR |
|---|---|---|
| 1 | Observer first：change 1 只讀，不寫入 HERDR | ADR-0001 |
| 2 | WSL 端經 `wsl.exe -e nc -U` 子程序當 stdio 傳話人 | ADR-0002 |
| 3 | cargo workspace 四個 crate，依賴方向由 Cargo 強制 | ADR-0003 |
| 4 | 後端到畫面：每次變動推整張圖，附遞增 version | ADR-0004 |
| 5 | 畫面形式：本機網頁 ＋ PWA manifest；Tauri 等 MVP 後 | ADR-0005 |

其他直接定案：設定檔 TOML；HERDR 型別手寫 observer 子集，不從 schema 生成；
前端純 HTML／JS 不用框架；schema JSON 進 repo 當 contract test fixture；
版本落差不擋連線只記警告。

## 4. 架構

### 4.1 Crate 與依賴方向

```text
cockpit            程式本體：讀設定、啟動 runtime、axum HTTP + WebSocket、靜態網頁（內嵌）
  ├─ cockpit-herdr 接合層：HerdrRuntime 實作 AgentRuntime；HERDR 型別 → core 型別；
  │    │              HERDR 特有連線順序（Probe→seed→L→S，封裝在 subscribe() 內）
  │    ├─ herdr-client   只懂 HERDR：協定型別（observer 子集）、Connector、request／subscribe
  │    └─ cockpit-core   只懂 Cockpit：RuntimeId、runtime 模型、AgentRuntime trait、RuntimeStore、
  │                       ProjectedState、driver::run（與 runtime 種類無關的連線驅動器）
  └─ cockpit-core
```

規則：`cockpit-core` 不得依賴 `herdr-client` 或任何 `cockpit-*`（它是最底層，天然不會有這種
依賴，這裡重申是為了讓 change 2、3 的 domain 型別也守住）；`herdr-client` 不得依賴任何
`cockpit-*`；`cockpit` 不直接依賴 `herdr-client`，只透過 `cockpit-herdr` 的工廠函式取得
`Arc<dyn AgentRuntime>`（ADR-0003、設計文件 D16）。change 2 的 domain（Project、Pipeline、
Task）與投影加在 `cockpit-core`。「連線驅動器與 HERDR 特有連線順序分屬 `cockpit-core` 與
`cockpit-herdr`」見 §4.2、§7.1（設計文件 D1）。

Rust edition 2024。依賴（版本實作時取當時穩定版，不在此鎖定）：`tokio`、`axum`（開 `ws`
feature）、`serde`、`serde_json`、`toml`、`tracing`、`tracing-subscriber`、`thiserror`；
bin 另加 `anyhow`。Windows named pipe 用 tokio 內建
`tokio::net::windows::named_pipe::ClientOptions`，不引第三方 crate。

### 4.2 資料流（change 1b）

```text
TOML 設定：每個 runtime 一筆
        ↓
每個 runtime 一個 tokio task（cockpit-herdr）
  1. Probe：wsl 型 runtime 先探發行版是否在跑
  2. 短連線 session.snapshot（seed）→ 只取 pane id 清單
  3. 長連線 L：events.subscribe 24 種生命週期訂閱
  4. 長連線 S：events.subscribe 每個 seed pane 一筆 pane.agent_status_changed
  5. 短連線 session.snapshot（authoritative）→ 翻成 RuntimeSnapshot → RuntimeStore::replace
     5 之前從 L／S 收到的事件一律丟棄
  6. Streaming：L／S 事件翻成 RuntimeEvent → RuntimeStore::apply
     pane 集合改變 → 重開 S；每 resnapshot_secs → replace；Drift → 立刻 replace
  7. L 或 S 斷 → 退避 → 回到 1
        ↓
RuntimeStore 每次變動 → 重算 ProjectedState → 內容有變才 version+1 並廣播
        ↓
每個 WebSocket 客戶端收到整張圖，整頁重畫
```

**步驟歸屬**（設計文件 D1）：步驟 1–4（Probe、seed snapshot、開 L、開 S）是 HERDR 特有的連線
順序，封裝在 `HerdrRuntime::subscribe()` 內部，對外只回傳合併後的單一事件流；步驟 5–7
（authoritative snapshot、Streaming 套用事件、Drift／定期重拿、失敗退避重來）與 runtime 種類
無關，由 `cockpit-core::driver::run(runtime, store, policy, cancel)` 執行，只認得
`AgentRuntime` 的 `snapshot()`／`subscribe()`。上圖寫的「每個 runtime 一個 tokio task
（cockpit-herdr）」因此更精確地說是「一個 tokio task 跑 `cockpit-core` 的驅動器，驅動器內部
呼叫的 `subscribe()` 才是 `cockpit-herdr` 的程式碼」；讓 change 2、3 與未來其他 runtime 只需
實作 `AgentRuntime`，不必重寫 Drift、定期重拿、退避這些邏輯。

seed snapshot 只為了在開 S 之前知道有哪些 pane，讓 S 在 authoritative snapshot 之前就開好，
agent 狀態變化的空窗只剩一次 snapshot 來回。change 1 只呼叫兩個 HERDR method：
`session.snapshot`、`events.subscribe`。

## 5. `herdr-client`

### 5.1 Connector

```rust
#[async_trait]
pub trait Connector: Send + Sync {
    /// 每呼叫一次開一條新的 NDJSON 連線；HERDR 每條連線只服務一個 method。
    async fn connect(&self) -> Result<Box<dyn NdjsonStream>, ConnectError>;
    fn describe(&self) -> String; // 給畫面顯示，例如 "named-pipe C:\...\herdr.sock"
}
```

三個實作：

| 實作 | 平台 | 做法 |
|---|---|---|
| `NamedPipeConnector { path }` | Windows | `ClientOptions::new().open(r"\\.\pipe\" + path)` |
| `UnixSocketConnector { path }` | unix | `tokio::net::UnixStream::connect(path)` |
| `ChildStdioConnector { command, args }` | 任何 | `tokio::process::Command` 開子程序，stdin／stdout 接成 stream，stderr 收進 log；Windows 上加 `creation_flags(CREATE_NO_WINDOW)`，不與 `DETACHED_PROCESS` 併用 |

`ConnectError` 區分 `ServerNotRunning`（NotFound／ConnectionRefused／子程序立即結束）、
`Io`、`Spawn`。

### 5.2 Client

```rust
pub struct Client { connector: Arc<dyn Connector> }

impl Client {
    pub async fn request<R: Request>(&self, req: R) -> Result<R::Response, RequestError>;
    pub async fn subscribe(&self, subs: &[Subscription]) -> Result<EventStream, RequestError>;
}
```

- `request`：開連線、送一行 `{"id","method","params"}`、讀一行、關閉。回應 `id` 不符或
  `error` 物件 → `RequestError::Remote { code, message }`；JSON 解析失敗 → `Protocol`。
- `subscribe`：開連線、送 `events.subscribe`、讀到 `subscription_started` 後回傳
  `EventStream`；收到 `error` 回應（例如某個 `pane_id` 已不存在）→ `Remote`。stream 逐行
  解析成 `IncomingEvent`（`Lifecycle(EventKind, Value)` 或 `PerPane(SubscriptionEventKind, Value)`），
  依 `event` 是否含 `.` 分軌。壞掉的行記 warn 後跳過，連線不斷；連線 EOF → stream 結束。
- `Subscription` 型別涵蓋 24 種無參數與 `PaneAgentStatusChanged { pane_id }`；
  `pane.output_matched`、`pane.scroll_changed` 不建模。
- 型別：只手寫 observer 子集（`SessionSnapshot`、`WorkspaceInfo`、`TabInfo`、`PaneInfo`、
  `AgentInfo`、`AgentStatus`、各事件 payload、`PaneReadParams`／`PaneReadResult`）。全部
  `#[serde(default)]`、不用 `deny_unknown_fields`、非必填欄位一律 `Option`；`AgentStatus`
  用 `#[serde(other)]` 收 `Unknown`。
- Fixture：`herdr-client/tests/fixtures/` 放兩份 schema（由 `docs/research/2026-09-13/`
  複製，或實作時重新匯出並更新兩處）；contract test 用 JSON Schema 驗證我們序列化的
  request 與解析用的 fixture（用 `jsonschema` crate，見 §15）。

## 6. `cockpit-core`

### 6.1 Runtime 模型

```rust
pub struct RuntimeId(String);            // 來自設定檔，例如 "win"、"wsl"

pub enum AgentStatus { Idle, Working, Blocked, Done, Unknown }

pub struct Workspace { id, label: Option<String>, number: u32, agent_status, focused: bool }
pub struct Tab       { id, workspace_id, number: u32, agent_status, focused: bool }
pub struct Pane      { id, workspace_id, tab_id, agent: Option<String>, agent_status,
                       title: Option<String>, cwd: Option<String>, label: Option<String>,
                       focused: bool, exited: bool, updated_at: SystemTime }
pub struct Agent     { agent: String, pane_id, workspace_id, tab_id, agent_status }
pub struct Focused   { workspace_id: Option<WorkspaceId>, tab_id: Option<TabId>, pane_id: Option<PaneId> }
// FocusChanged 事件用；每個 Option 表示「該層本次是否隨事件一起改變」，None 表示不變
// （與 Focused 語意不同：Focused 的 None 表示「目前沒有東西被聚焦」）。
pub struct FocusChange { workspace_id: Option<WorkspaceId>, tab_id: Option<TabId>, pane_id: Option<PaneId> }

pub struct RuntimeSnapshot { server_version: String, protocol: u32,
                             workspaces: Vec<Workspace>, tabs: Vec<Tab>, panes: Vec<Pane>, agents: Vec<Agent>,
                             focused: Focused }
// HERDR snapshot 的 layouts 陣列刻意不建模：change 1 不畫 pane 幾何配置。

pub enum RuntimeEvent {
    WorkspaceUpserted(Workspace), WorkspacesReplaced(Vec<Workspace>), WorkspaceRemoved(WorkspaceId),
    WorkspaceRelabeled { id: WorkspaceId, label: String },
    TabUpserted(Tab), TabsReplaced { workspace_id: WorkspaceId, tabs: Vec<Tab> }, TabRemoved(TabId),
    TabRelabeled { id: TabId, label: String },
    PaneUpserted(Pane), PaneMoved { previous: PaneId, pane: Pane }, PaneRemoved(PaneId), PaneExited(PaneId),
    AgentDetected { pane_id, agent: Option<String> },
    AgentStatusChanged { pane_id, status: AgentStatus, title: Option<String>, agent: Option<String> },
    FocusChanged(FocusChange),
    Drift { reason: String },     // 見 §6.3
    Noted { kind: String },       // 只進 recent_events，不改狀態
}

pub enum ConnectionState {
    Connecting,
    Connected { since, server_version, protocol, last_snapshot_at },
    Disconnected { reason: String, retry_in: Duration },
}
```

翻譯層是無狀態的純函數，手上只有 HERDR 給的 id 與新值，組不出完整物件，所以
`workspace_renamed`／`tab_renamed` 各自補一個只帶 id 與新 label 的變體
（`WorkspaceRelabeled`、`TabRelabeled`），由狀態庫改標籤、目標不存在則 `Drift`；
`workspace_focused`／`tab_focused`／`pane_focused` 各只帶一個 id，`FocusChanged` 因此用
`FocusChange`（三個 `Option`，`None` 表示該層不變）而非完整的 `Focused`；`TabsReplaced` 帶
`workspace_id`（`tab_moved` payload 只有該 workspace 的 tabs，組不出跨 workspace 的整份
tabs）；`pane_agent_detected` 的 `agent` 欄位本身是 `Option<String>`（agent 離開時可能為
`null`），`AgentDetected` 照樣帶 `Option`（設計文件 D3）。

### 6.2 AgentRuntime trait

```rust
#[async_trait]
pub trait AgentRuntime: Send + Sync {
    fn id(&self) -> &RuntimeId;
    async fn snapshot(&self) -> Result<RuntimeSnapshot, RuntimeError>;
    async fn subscribe(&self) -> Result<RuntimeEvents, RuntimeError>;
}

pub struct RuntimeEvents { /* mpsc::Receiver<Result<RuntimeEvent, RuntimeError>> 加一個
                              drop 時 abort() 內部 reader task 的 guard */ }

pub enum RuntimeError {
    Unavailable { reason: String, retry_after: Duration }, // 固定間隔重試，不進退避序列
    Failed(String),                                        // 進退避序列
}
```

`subscribe` 回傳 `RuntimeEvents`：`tokio::sync::mpsc::Receiver<Result<RuntimeEvent, RuntimeError>>`
加一個 abort-on-drop guard——drop 掉事件流即 `abort()` 內部 reader task，保證釋放連線與子程序；
不引入 `futures`／`tokio-stream`，只提供 `async fn next()`（設計文件 D2）。它是接合層已合併 L
與 S 兩條連線後的單一事件流；每 pane 訂閱的管理（§7.1）是接合層內部的事，`cockpit-core`
看不到。`RuntimeError::Unavailable { reason, retry_after }` 表示探測類失敗要用固定間隔重試
（例如 §7.1 的 WSL 探測），驅動器不進退避序列；其餘失敗用 `Failed(String)`，走一般的退避序列
（設計文件 D4）。change 3 再加 `read_output`；Phase 2 再加 `prompt` 等寫入方法，並以
capability 宣告。

### 6.3 RuntimeStore 與 ProjectedState

- `RuntimeStore`：`HashMap<RuntimeId, RuntimeState>`；`RuntimeState` 內是扁平的
  `workspaces`、`tabs`、`panes`、`agents` map 加 `connection` 與 `focused`。
  `replace(snapshot)` 整份換，`apply(event)` 逐筆改。
- **Drift 的定義**：事件的主體 id 不在狀態庫（例如 `PaneRemoved` 指向沒見過的 pane），
  或事件帶入的物件其父層 id 不在狀態庫（例如 `PaneUpserted` 的 `tab_id` 不存在）。
  `apply` 遇到就回傳 `Drift`，由接合層決定重拿 snapshot。啟動與重連時 authoritative
  snapshot 之前的事件已被丟棄，不會出現 Drift 風暴。
- `ProjectedState`：給畫面看的巢狀結構（runtime → workspace → tab → pane），加
  `version: u64`、`generated_at`、`recent_events`（每個 runtime 各保留最近 50 筆的合併，
  記憶體 ring buffer）。由 `RuntimeStore` 純函數產生；比對前一份相等就不遞增 version、不廣播。
- 廣播：`tokio::sync::watch<Arc<ProjectedState>>`；變動在 50 ms 內合併成一次。
- 已知且接受的風險：L 與 S 是兩條連線，順序不跨連線保證。`pane.updated` 帶的 `PaneInfo`
  可能夾帶比 S 上最新狀態舊的 `agent_status`，造成毫秒級回退；下一個狀態事件或定期
  snapshot 會修正。

### 6.4 對畫面的 JSON

```json
{
  "version": 42,
  "generated_at": "2026-09-13T02:10:00Z",
  "runtimes": [
    {
      "id": "win",
      "kind": "herdr",
      "endpoint": "named-pipe C:\\Users\\<user>\\AppData\\Roaming\\herdr\\herdr.sock",
      "connection": { "state": "connected", "since": "...", "server_version": "0.9.0-preview...", "protocol": 22, "last_snapshot_at": "...", "protocol_warning": null },
      "focused": { "workspace_id": "wJ", "tab_id": "wJ:t1", "pane_id": "wJ:p1" },
      "workspaces": [
        { "id": "wJ", "label": "ai-cockpit", "number": 2, "agent_status": "working", "focused": true,
          "tabs": [
            { "id": "wJ:t1", "number": 1, "agent_status": "working", "focused": true,
              "panes": [
                { "id": "wJ:p1", "agent": "claude", "agent_status": "working", "title": "...", "cwd": "D:\\projects\\ai-cockpit", "label": null, "focused": true, "exited": false, "updated_at": "..." }
              ] }
          ] }
      ]
    }
  ],
  "recent_events": [
    { "at": "...", "runtime": "win", "kind": "pane.agent_status_changed", "pane_id": "wJ:p1", "detail": "working" },
    { "at": "...", "runtime": "win", "kind": "tab_removed", "tab_id": "wJ:t2" },
    { "at": "...", "runtime": "win", "kind": "focus_changed", "pane_id": "wJ:p1" },
    { "at": "...", "runtime": "win", "kind": "drift" }
  ]
}
```

`connection.state` 為 `disconnected` 時另帶 `reason` 與 `retry_in_secs`；`protocol` 不在已測
範圍 20..=22 時 `protocol_warning` 帶警告文字，否則為 `null`（設計文件 D12）。

`recent_events[].kind` 的填法（1b 執行時裁決）：`AgentStatusChanged` 一律記成
`pane.agent_status_changed`（對齊 S 連線的原始事件名，即上面範例第一筆）；其餘
`RuntimeEvent` 變體記成該變體名稱的 snake_case（`pane_upserted`、`tab_removed`、
`workspace_relabeled`、`tabs_replaced`、`focus_changed`……）；`Noted` 記成原始 HERDR 事件名
（例如 `layout_updated`）；`Drift` 記成 `drift`。每筆只填一個主體 id 欄位：pane 事件填
`pane_id`、tab 事件填 `tab_id`、workspace 事件填 `workspace_id`；`TabsReplaced` 填
`workspace_id`；`FocusChanged` 填 `FocusChange` 三個 `Option` 裡最深一層非 `None` 的那個
（pane 優先於 tab、tab 優先於 workspace）；`Noted`、`Drift`、`WorkspacesReplaced` 不填任何
主體 id。

## 7. `cockpit-herdr` 接合層

### 7.1 連線迴圈狀態機

```text
Probe → SeedSnapshot → SubscribeLifecycle(L) → SubscribeStatus(S) → Snapshot → Streaming
  │ 任一步失敗                                                                  │
  ▼                                                                             │ L／S 事件 → apply
Backoff（1s, 2s, 4s … 上限 30s；成功後歸零）→ Probe                              │ pane 集合改變 → ReopenStatus
                                                                                │ 每 resnapshot_secs → replace
                                                                                │ Drift → 立刻 replace
                                                                                │ L 或 S EOF／錯誤 → Backoff
```

**crate 歸屬**（設計文件 D1）：左側 Probe→SeedSnapshot→SubscribeLifecycle(L)→SubscribeStatus(S)
是 `HerdrRuntime::subscribe()` 內部完成的 HERDR 特有順序，對外只回傳合併後的單一事件流；右側
Snapshot（authoritative）→Streaming（apply、ReopenStatus、定期重拿、Drift 重拿）與失敗後的
Backoff→回到 Probe，屬於與 runtime 種類無關的驅動器 `cockpit-core::driver::run`，只透過
`AgentRuntime` 的 `snapshot()`／`subscribe()` 呼叫左側；ReopenStatus 仍是 `HerdrRuntime`
內部的事（S 管理器住在裡面，見 §6.2、§7.2）。

- **與 spec §24 的順序刻意不同**：spec 寫「snapshot → reconcile → subscribe」，本設計改為
  「訂閱先開好、再拿 authoritative snapshot」，目的是把事件空窗縮到一次 snapshot 來回。
  reconcile 的角色由「authoritative snapshot 整份替換」承擔。
- **Seed snapshot**：只取 pane id 清單，不套用到狀態庫。若 S 的 subscribe 因某個 seed pane
  已消失而失敗，回到 Probe 重來。
- **ReopenStatus**：`pane.created`、`pane.closed`、`pane.moved`、以及任何 snapshot 替換後
  pane 集合有變時觸發；200 ms 內合併成一次。做法是先開新的 S 並等到 `subscription_started`，
  再關舊的 S；重疊期間重複收到的狀態事件是冪等的。
- **定期 snapshot 只在 Streaming 狀態跑**；斷線期間不做。任何一次成功的 snapshot（定期或
  Drift 觸發）都重設計時器；Drift 觸發的 snapshot 進行中時，再來的 Drift 不重複觸發。
- **WSL 探測**：`wsl` 型 runtime 在 Probe 階段先跑 `wsl.exe --list --running --quiet`。
  發行版不在清單 → `Disconnected`，原因「WSL 發行版 <名稱> 未啟動」；探測指令本身失敗
  （wsl.exe 不存在、非零結束碼）→ `Disconnected`，原因「WSL 探測失敗：」加上 stderr 內容。兩者都以
  `wsl_probe_secs` 間隔再探，不進退避序列。**編碼判別**：stdout 含 NUL byte 才是 UTF-16LE、用
  `from_utf16_lossy` 解碼，否則當 UTF-8——使用者環境設了 `WSL_UTF8=1` 時輸出即為 UTF-8，且
  `from_utf8().is_ok()` 對 UTF-16LE 內容也會回 `Ok`，判別只能靠 NUL byte（設計文件 D8）。探測只
  在建立事件流（`subscribe()`）前做一次，`Connected` 期間與定期 snapshot 都不重探：L／S 連線
  活著就代表虛擬機在跑，虛擬機關掉時 L／S 會 EOF 觸發下一輪重連才再探測。
- **版本**：snapshot 回傳的 `protocol` 不在已測範圍 20..=22 時記 warn 並在畫面連線狀態旁標註，
  照常運作。

### 7.2 HERDR 事件 → RuntimeEvent 對照

| HERDR event | RuntimeEvent |
|---|---|
| `workspace_created`、`workspace_updated`、`workspace_metadata_updated` | `WorkspaceUpserted` |
| `workspace_moved`、`workspace_reordered` | `WorkspacesReplaced`（payload 帶整份 workspaces） |
| `workspace_renamed` | `WorkspaceRelabeled { id, label }`（不存在 → `Drift`） |
| `workspace_closed` | `WorkspaceRemoved` |
| `tab_created` | `TabUpserted` |
| `tab_renamed` | `TabRelabeled { id, label }`（不存在 → `Drift`） |
| `tab_closed` | `TabRemoved` |
| `tab_moved` | `TabsReplaced { workspace_id, tabs }`（payload 帶該 workspace 整份 tabs） |
| `pane_created`、`pane_updated` | `PaneUpserted` |
| `pane_moved` | `PaneMoved { previous, pane }`：移除舊 id、以新 `PaneInfo` upsert |
| `pane_closed` | `PaneRemoved` |
| `pane_exited` | `PaneExited` |
| `pane_agent_detected` | `AgentDetected`；`released` 為 true 或 `agent` 為 null 時清掉該 pane 的 agent |
| `pane.agent_status_changed`（S 連線） | `AgentStatusChanged` |
| `pane_agent_status_changed`（生命週期版，見 §2.3「沒有全域的 agent 狀態訂閱」） | 理論上不會出現在 L；若因故收到則忽略並記 debug |
| `workspace_focused`、`tab_focused`、`pane_focused` | 部分 `FocusChanged`：只有對應那個 `Option` 欄位是 `Some`，其餘 `None` |
| `layout_updated`、`worktree_*` | `Noted` |
| `pane_output_changed` | 無對應訂閱，收不到；若出現則忽略並記 debug |
| `pane.output_matched`、`pane.scroll_changed` | 不訂閱 |

各事件 payload 欄位以 §2.3 的表與 fixture 為準；實作時若 fixture 與表不符，以 fixture 為準並
回頭改本文件。payload 不足以 upsert 的一律 `Drift`。

## 8. `cockpit` 程式本體

### 8.1 HTTP 路由

| 路由 | 用途 |
|---|---|
| `GET /` | `index.html`（內嵌） |
| `GET /app/*.js`、`/app/*.css` | 靜態資源（內嵌） |
| `GET /manifest.webmanifest`、`GET /icons/*` | PWA 安裝 |
| `GET /ws` | WebSocket；連上先送目前整張圖，之後每次變動送一份 |
| `GET /api/state` | 同一份 JSON，給除錯與測試用 |

所有靜態檔以 `include_bytes!` 內嵌，單一執行檔。只綁 `127.0.0.1`。

### 8.2 設定檔

```toml
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
```

- 每筆 runtime 的 `socket`、`wsl`、`command` 三選一。**同時給兩個以上，啟動時報錯並指出
  是哪一筆**；都不給則自動找本機預設路徑（依 §2.2 規則：`HERDR_SOCKET_PATH` →
  `HERDR_SESSION` → 預設）。`id` 重複同樣啟動時報錯。
- 設定檔位置：`--config <path>`，否則工作目錄的 `cockpit.toml`，都沒有就零設定模式
  （等於一筆自動找本機的 runtime，id 為 `local`）。
- Windows 路徑在 TOML 用單引號 literal string。

### 8.3 畫面（change 1b）

檔案：`index.html`、`app/channel.js`、`app/render.js`、`app/style.css`、
`manifest.webmanifest`、`icons/`。

- `channel.js` 只做一件事：連 `/ws`、收到訊息就呼叫 `onState(state)`、斷線後退避重連。
  換 Tauri 時只改這個檔。
- `render.js` 收到整張圖就整頁重畫。版面由上到下：頂列（名稱、通道狀態、version）；
  每個 runtime 一張卡（id、endpoint、連線狀態與原因、server 版本、最後 snapshot 時間）；
  卡內以 workspace 分組，每組顯示標籤與彙總狀態，底下每個 pane 一列：id、agent 或 shell、
  狀態色塊（working 綠、blocked 琥珀、done 藍、idle 灰、unknown 暗灰、exited 加刪除線）、
  標題、cwd；頁尾「最近事件」50 筆。
- 深色配色沿用 `docs/cockpit-dashboard-concept.png`，不做該圖的版面。
- 沒有任何可點的互動。
- PWA：manifest 提供 `name`、192px 與 512px 圖示、`start_url`、`display: standalone`；
  不做 service worker，使用者從 Chrome 選單安裝即可（條件見 §15）。

## 9. 錯誤處理與 Unknown 原則

- 不認得的 `AgentStatus` 字串 → `Unknown`；不認得的事件種類 → 忽略並記 debug；
  不認得的欄位 → 忽略；壞掉的 JSON 行 → 記 warn、跳過、連線不斷。
- 任何連線錯誤都變成該 runtime 的 `Disconnected { reason }` 顯示在畫面，不吞掉。
- HERDR 優雅關閉時，進行中的 `session.snapshot` 會回 `server_unavailable: server is shutting
  down`；驅動器把這種請求失敗與 L／S EOF 同樣視為該 runtime 斷線，所以斷線的第一個原因不一定
  反映連線本身結束，也可能是 snapshot 請求先收到這個回應——兩者都算對端關閉（task 4.3 實測）。
- Cockpit 從不依 HERDR 狀態推論「任務完成」；change 1 沒有任務概念，change 2 的
  `StageStatus::Completed` 必須來自 Cockpit 自己的規則或人工。
- change 1 對 HERDR 完全唯讀。

## 10. 測試策略與驗收

### 10.1 測試層

| 層 | 內容 | 位置 |
|---|---|---|
| 單元 | HERDR 型別 → core 型別（各事件、5 種狀態、未知值）；`RuntimeStore::apply` 各分支含 Drift 與 PaneMoved；`ProjectedState` 純函數與「沒變不遞增」 | 各 crate `src/` |
| 假 HERDR | 程序內假 server：NDJSON、每連線一 method、fixture 驅動的 snapshot 與事件、可主動斷線、可讓某個 pane 探測失敗。驗連線迴圈全部狀態轉移、seed→L→S→snapshot 順序、事件丟棄、ReopenStatus 重疊、退避、Drift 重拿 | `herdr-client/tests/`（client 層）、`cockpit-herdr/tests/`（迴圈層） |
| 合約 | 兩份 schema fixture 驗 request 序列化與 fixture 形狀 | `herdr-client/tests/` |
| 真機 | `#[ignore]` 標記；README 寫明啟動方式 | `cockpit/tests/` |
| 畫面 | 以瀏覽器工具開頁面截圖、讀 console，對照 Scenario | 手動 |

**真機測試禁令**：不得以 `herdr server stop` 製造 Windows 端斷線，會殺掉所有 pane。
Windows 端斷線只用假 server 驗；真機斷線重連只在 WSL 端做。

### 10.2 Scenario 對應（change 1b）

| Scenario | 驗法 | 通過條件 |
|---|---|---|
| A Attach | WSL 端 HERDR 先啟動；啟動 cockpit.exe，開 `http://127.0.0.1:7770` | 兩張 runtime 卡片皆 `connected`；**兩張**卡片各自列出的 workspace／pane／agent 與該側 `herdr api snapshot` 一致 |
| B Live state | 在 Windows 端某 pane 對 agent 下一句指令；在 WSL 端重複一次 | 該 pane 一秒內變 `working`；最近事件出現 `pane.agent_status_changed`；全程無任何 prompt 送往 agent；新開一個 pane 後它的狀態變化同樣即時出現（驗 ReopenStatus） |
| F Reconnect | 關掉再開 WSL 端 HERDR | 卡片由 `disconnected`（附原因）回到 `connected`；內容與重新拿到的 snapshot 一致 |

### 10.3 品質 gate

`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test --workspace`
全過；`.md` 過 markdownlint（repo 根 `.markdownlint-cli2.jsonc`，在 repo 根目錄執行）。
Codex review 沙箱唯讀而 cargo 必寫 `target/`，`AGENTS.md` 只給 `cargo fmt --check`
作零寫入指令，其餘 findings 實測後才採信。

## 11. change 1a 的 spike（tasks 第一批，任一不成立就回頭改本文件）

1. tokio `ClientOptions::open` 開得了含冒號與反斜線的 pipe 名稱，送 `session.snapshot` 得到回應。
2. `wsl.exe -d Ubuntu-24.04 -e nc -U <sock>` 經 stdio 送一行 request 收到一行 response；
   server 關閉時 nc 結束；是否需要 `-N`；長連線訂閱下事件不被緩衝延遲。
3. S 連線：一條 `events.subscribe` 帶 N 筆 `pane.agent_status_changed` 可成功，且與 L 連線
   （24 種生命週期）同時開著互不影響；其中一筆 `pane_id` 已不存在時整個請求回 `error`
   （預期，需確認）；重開 S 的重疊期間不丟事件。
4. 從 HERDR pane 內啟動 cockpit.exe 並連 WSL 時，不出現任何額外視窗。
5. `wsl.exe --list --running --quiet` 不喚醒虛擬機（§2.9 已實測一次，實作時再確認並記錄
   輸出編碼處理）。

## 12. change 2、3 從第一天就要遵守的約束

- **Binding 不綁 pane id 為唯一鍵**：pane id 在 pane 移動或重建後會變（§2.3）；change 2 的
  `RuntimeBinding` 要允許以 runtime id ＋ workspace 標籤或 cwd ＋ agent 種類等穩定特徵匹配，
  pane id 只是解析結果。
- **change 2、3 不寫入 HERDR metadata**：binding 的真相在 Cockpit 設定與狀態，
  `pane.report_metadata` 只讀不寫。Phase 2 若要寫入（例如把 stage 名稱顯示在 HERDR
  側欄），用 `cockpit.` 前綴的 key 並經 capability 檢查，且不得把 binding 真相移過去。
- **Live Output 讀取模型**：`pane.read` 無增量，change 3 以 `revision` 比對後才推送；
  WSL 端每次讀都要開子程序，讀取頻率超過每秒一次時即為改用 ADR-0002 方案 B 的時機。
- **Usage 面板**：HERDR `tokens` 不是 LLM 用量，沒有可靠來源前不顯示，也不估算。

## 13. spec §25 open questions 對照

| # | 問題 | 結論 |
|---|---|---|
| 1 | Space 與 Workspace 術語 | schema 只有 `workspace`；Cockpit 統一用 Workspace，見 `CONTEXT.md` |
| 2 | 各平台 transport | §2.2；WSL 經 nc 傳話 |
| 3 | agent status 是否穩定 | 五值 enum 四層皆有；`done` 為衍生，不當完成訊號；即時變化只能每 pane 訂閱（§2.3） |
| 4 | `pane.read` 增量策略 | 無增量，靠 `revision`；change 3 |
| 5 | ANSI parsing crate | change 3 再查證套件名 |
| 6 | worktree 與 project／workstream 對應 | change 2 |
| 7 | 設定格式 | TOML |
| 8 | 第一版是否存 event history | 不存；記憶體 ring buffer 50 筆僅供除錯 |
| 9 | HERDR metadata 可否存 binding | 只讀；§12 |
| 10 | metadata 所有權衝突 | change 1–3 唯讀，不會發生；寫入時前綴 key ＋ capability |
| 11 | token 用量來源 | 無可靠來源；§12 |
| 12 | Space／Workstream 多對多 | change 2 決定；`CONTEXT.md` 只承諾「不強制一對一」 |

## 14. 文件與流程

- 計畫層走 OpenSpec：`openspec init` 後先 `/opsx:propose herdr-client`，完成並歸檔後再
  `/opsx:propose attach-herdr-runtimes`；change 名 ＝ 分支 slug。
- 詞彙表 `CONTEXT.md`；決策 `docs/adr/`；交接 `docs/handover.md`；查證證據
  `docs/research/2026-09-13/`。
- 本文件在實作中若被 spike 推翻，先改本文件與對應 ADR，再改 tasks。

## 15. 外部精確資訊（查證日期 2026-09-13）

| 項目 | 結論 | 來源 |
|---|---|---|
| Chrome PWA：`127.0.0.1`／`localhost` 是否算 secure context | 算，含任意 port | MDN Making PWAs installable；chromestatus.com「Treat `http://localhost` as a secure context」 |
| Chrome PWA：是否要求 service worker | 從瀏覽器選單安裝**不需要**（Chrome 112 桌面版起）；只有「自動跳出安裝提示」仍要求 fetch handler，本專案不需要 | developer.chrome.com blog「update-install-criteria」（2023-12-05） |
| Chrome PWA：manifest 最少欄位 | `name` 或 `short_name`；`icons` 含 192px 與 512px；`start_url`；`display` 為 standalone 等值之一 | web.dev install-criteria（2024-09-19 更新） |
| Rust JSON Schema 驗證 crate | `jsonschema` 0.56.0（2026-09-10 發布），有 `draft202012` 模組，直接驗 `serde_json::Value`。次選 `boon` 0.6.1（更新較慢） | crates.io API、docs.rs |
| ANSI parsing crate | change 3 再查 | — |
