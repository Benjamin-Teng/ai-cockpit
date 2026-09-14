# design：herdr-client

## Context

動機見 `proposal.md`「Why」。API 形狀已由設計文件 §5 定案，本文件不重述；只寫本 change
特有的取捨與設計文件沒講到的細節。約束：

- 目前 repo 沒有任何程式碼，本 change 建立 workspace 與第一個 crate。
- §11 五個 spike 是 tasks 第一批；任一不成立就停下來改設計文件，不在本 change 內硬繞。
- 本 crate 不得依賴任何 `cockpit-*`（ADR-0003）；change 1 只呼叫 `session.snapshot` 與
  `events.subscribe`（ADR-0001）。
- 測試層見設計文件 §10.1：單元、假 HERDR（client 層）、合約、真機 `#[ignore]`。

## Goals / Non-Goals

**Goals:**

- change 1b 只透過 `Connector`／`Client`／型別使用本 crate，不需要知道 transport 差異。
- 除真機測試外，所有行為都能以假 HERDR 在本機無 HERDR 的情況下驗證。
- spike 不只回答問題，還要產出後續測試用的真機 fixture。

**Non-Goals（設計層）:**

- 不在 client 內做重試、退避、逾時：這些是連線迴圈（change 1b）的責任，client 保持
  「一次連線、一次 method」的簡單語意。
- 不做 protocol 協商或版本檢查：只把 snapshot 的 `version`／`protocol` 原樣暴露。

## Decisions

### D1. spike 產出 fixture，而不是手寫 fixture

spike 1 與 2 各存一份真機 snapshot（protocol 22、20），spike 3 存真機事件行
（生命週期與 `pane.agent_status_changed` 各一份 NDJSON）。去識別化（cwd、title、label）後
放 `herdr-client/tests/fixtures/`，合約測試驗它們仍通過 schema。
**替代**：依 schema 手寫 fixture。捨棄原因：會把我們對 payload 的假設當成事實，而設計文件
§7.2 明講「fixture 與表不符時以 fixture 為準」。

### D2. spike 4 以本 crate 的 example 代替 cockpit.exe

§11 第 4 項寫「從 HERDR pane 內啟動 cockpit.exe」，但 cockpit 到 change 1b 才存在。受測的
性質是子程序 spawn 旗標（`CREATE_NO_WINDOW`），它就在本 crate，所以用
`herdr-client/examples/` 的小程式在 HERDR pane 內執行即可；change 1b 真機驗收時再以
cockpit.exe 複驗一次。

### D3. 子程序型連線的 ServerNotRunning 在第一次讀取時判定

§5.1 把「子程序立即結束」歸為 `ConnectError::ServerNotRunning`，但沒說何時判定。
`connect()` 只 spawn 不等待；串流在「還沒讀到任何一行就 EOF、且子程序已結束」時，把 EOF
轉成 `ConnectionRefused` 類 I/O 錯誤並附 stderr 內容；`Client` 在第一次讀取遇到
`NotFound`／`ConnectionRefused` 時對應成 `ServerNotRunning`。三種 connector 對上層呈現同一種
錯誤，只是時間點不同。
**替代**：`connect()` spawn 後等一小段 grace window 看子程序有沒有退出。捨棄原因：
`wsl.exe` 啟動要 0.1–0.3 秒（ADR-0002），grace 要比它長才有意義，等於每次 WSL 連線都付
固定延遲，而且仍有競態。`request` 與 `subscribe` 都會在 connect 後立刻讀一行
（回應或 `subscription_started`），延後判定不會漏掉任何情境。

### D4. 事件分軌以「已知名稱集合」判定，未知名稱保留原字串

§5.2 寫「依 `event` 是否含 `.` 分軌」。實作改為：名稱在 26 種 `EventKind` 內 → 生命週期；
在 3 種 `SubscriptionEventKind` 內 → 每 pane；其餘 → `Unknown { event, data }`。效果與
點號分軌一致，但未知名稱不會被吞進某個 `Unknown` 變體而丟掉字串，§9 要求的 debug 日誌
才有東西可記。

### D5. 回應 id 不符歸 Protocol，不歸 Remote

§5.2 把「`id` 不符」與 `error` 物件都歸 `Remote { code, message }`，但 id 不符沒有
`code`／`message` 可填，硬填等於捏造。改歸 `Protocol`，與「JSON 解析失敗」、「`result` 形狀
不符」同類：都是對方沒照協定講話。

### D6. 串流以 `Option<Result<IncomingEvent, StreamError>>` 呈現結束原因

正常 EOF → `None`；I/O 錯誤 → 先 `Some(Err(原因))` 再 `None`。change 1b 需要原因字串來
填 `ConnectionState::Disconnected { reason }`（§9「不吞掉」）。壞行不進 `Err`，只記 warn
後跳過。

### D7. 假 HERDR 放在 `test-support` feature 的公開模組，監聽真實 transport

- 位置：`herdr_client::testing`（feature `test-support`）。本 crate 以 self
  dev-dependency 開這個 feature 測自己；change 1b 在 `cockpit-herdr` 的 dev-dependency 開
  同一 feature 重用，符合 §10.1「假 HERDR 在 client 層與迴圈層兩處使用」。
- transport：Windows 上用 tokio `ServerOptions` 開含冒號路徑名稱的 named pipe，unix 上用
  `UnixListener`，讓 `NamedPipeConnector`／`UnixSocketConnector` 在無 HERDR 的機器上也被
  真的走到；名稱用暫存目錄路徑加隨機後綴，避免撞到真的 HERDR。
- 可控行為：以 fixture 回 `session.snapshot`；`events.subscribe` 回
  `subscription_started` 後推送腳本事件；可指定某 `pane_id` 探測失敗回 `error`；可回
  錯 id、回非 JSON、收到 request 後直接關閉、推壞行、正常關閉、非正常中斷。
- **替代**：`tests/common/mod.rs`（跨 crate 不能共用）、第五個 crate `herdr-fake`（違反
  ADR-0003 的四 crate 配置）。

### D8. 合約測試的 schema 根選擇方式

兩份 schema 檔頂層是 `{ "$schema", "protocol", "schema_version", "schemas": { request,
success_response, error_response, event, subscription_event }, "title" }`，內部 `$ref`
一律是文件絕對指標（例如 `#/schemas/success_response/$defs/SessionSnapshot`）。因此不能把
`schemas.request` 單獨抽出來編譯，要以整份文件為根、在頂層加 `"$ref": "#/schemas/<root>"`
來選根，用 `jsonschema` 的 draft 2020-12 模式驗證（§15 查證 0.56.0 有 `draft202012` 模組）。
schema 檔從 `docs/research/2026-09-13/` 複製到 `tests/fixtures/`，兩處各留一份，research
目錄的 README 註明複本位置；不加跨目錄一致性檢查，保持 crate 可獨立重用。

### D9. Windows 常數硬寫，PIPE_BUSY 有限重試

`CREATE_NO_WINDOW = 0x0800_0000` 直接以常數寫在 cfg(windows) 區塊，不為一個常數引入
`windows-sys`。`ClientOptions::open` 遇 `ERROR_PIPE_BUSY`（231）時依 tokio 文件建議等 50 ms
重試，上限 1 秒後回 `Io`。spike 1 會同時開 3 條連線觀察是否真的遇到。

### D10. request id、逾時、`pane.read`

- `id` 為程序內遞增計數器的十進位字串；唯一性只需在程序內成立（HERDR 每連線一 method，
  不會跨連線比對）。
- 不設逾時；呼叫端用 `tokio::time::timeout` 包。
- `pane.read` 的 `Request` 實作與型別一起提供，但 change 1 沒有任何呼叫者；合約測試只驗
  序列化。

### D11. 真機測試以環境變數指定目標

`herdr-client/tests/real_herdr.rs` 全部 `#[ignore]`；目標由 `HERDR_CLIENT_TEST_WIN_SOCKET`
（省略則用本機預設路徑解析）、`HERDR_CLIENT_TEST_WSL_DISTRO`、
`HERDR_CLIENT_TEST_WSL_SOCKET` 指定。spike 測試分三檔（`real_herdr.rs`、`real_herdr_wsl.rs`、
`real_herdr_subscriptions.rs`），spike 過後改寫成用 `Client`。

### D12. 訂閱探測失敗的 error 回應 `id` 帶後綴（spike 3 發現）

HERDR 對 `events.subscribe` 逐一探測 `pane_id`，探測失敗時回的 `error` 回應 `id` 是
`<request id>:sub:<序號>:probe`（Windows 0.9.0 與 WSL 0.8.2 一致，`code` 為 `pane_not_found`）。
若照 D5「id 不符 → Protocol」處理，會把明確的遠端錯誤誤判成協定錯誤。因此 `error` 回應的 `id`
只要等於 request id、或以 `<request id>:` 開頭，就視為相符並回 `Remote`；成功回應仍要求完全相等。
序號可供日後指出是清單中第幾筆訂閱失敗，本 change 只保留在錯誤訊息裡。

### D13. schema 必填欄位缺席時解析失敗，不以預設值靜默通過（review 發現）

§5.2 寫「全部 `#[serde(default)]`」。Codex review 指出：容器層 default 會把拼錯或漏映射的欄位靜默成
空字串／0／Unknown，測試與真機都看不出來。改為：**已建模**且 schema 標 required 的欄位不加 default，缺席時
反序列化回 Err（上層視為 Protocol／Drift）；Cockpit 用不到的 required 欄位（例如 `terminal_id`）維持不建模，
其缺席不偵測（§5.2 observer 子集原則優先）；schema 非必填的欄位一律 `Option` 並欄位級 default；
仍不用 `deny_unknown_fields`（未知欄位照舊忽略）。代價：HERDR 若在未來版本移除某個必填欄位，
解析會失敗而不是降級；以兩份 protocol 版本的合約測試與真機測試守住。

## Risks / Trade-offs

- [tokio `ClientOptions::open` 不接受含冒號的 pipe 名稱] → spike 1 先驗。不成立則要改用
  `interprocess` crate，違反 §4.1「不引第三方 crate」，回頭改設計文件。
- [nc 緩衝或半關閉行為讓事件延遲或連線不結束] → spike 2 量測；不成立則 ADR-0002 方案 B
  提前，只換 Connector。
- [重開 S 訂閱的重疊期間丟事件] → spike 3 驗；不成立則 change 1b 在重開 S 後必須立刻
  resnapshot，回頭改 §4.2。
- [`pane_moved` 帶 `created_workspace`／`created_tab` 時，change 1b 若只 upsert pane 會
  因 tab 不存在而 Drift] → 本 change 把四個選填欄位（含 `closed_tab_id`）納入型別；§7.2 對照表需在 change 1b
  補處理。功能上不會錯（Drift 會重拿 snapshot），只是多一次 snapshot。
- [設計文件 §2.3 的 payload 表漏列 `pane_moved` 的 `closed_tab_id` 與 `pane_agent_detected` 的
  `final_status`（兩份 schema 都有）] → 本 change 的型別依 schema 納入；設計文件表格待補，
  回報使用者，不在本 change 內改設計文件。
- [WSL 0.8.2 的 L 訂閱剛建立時推送了訂閱前的舊事件，並出現「建立 tab N 才補推 tab N-2 的 closed」
  的延遲模式（spike 3 附帶觀察，未深究）] → §4.2 已規定 authoritative snapshot 前的事件一律丟棄，
  之後的過期事件走 Drift 重拿，功能可容忍；列為 change 1b 真機驗收的待查證項，本 change 不處理。
- [本機 preview build 與研究時的原始碼 HEAD 有約五天落差] → 凡原始碼推導與 spike 結果
  衝突，以 spike 與 fixture 為準。
- [去識別化後 fixture 不再通過 schema] → 合約測試同時驗 fixture 本身。
- [self dev-dependency 開 feature 的 cargo 行為] → edition 2024 預設 resolver 已隔離
  dev-dependency 的 feature，不會讓 `test-support` 進入正常建置；若實作時發現外洩，改為
  `cfg(test)` 加獨立 `tests/common`，只影響 change 1b 的重用方式。
- [spike 5 重驗需要 WSL 虛擬機停止] → `wsl.exe --shutdown` 會關掉 WSL 端 HERDR 與所有
  pane，是否執行由使用者當場決定；不執行則沿用 §2.9 的一次實測並只補記輸出編碼處理。

## Migration Plan

全新 crate，無部署與資料遷移。回退方式：刪除 `herdr-client/` 與 workspace `Cargo.toml`。

## Open Questions

- `ChildStdioConnector` 的本機測試用哪種子程序當假對端：候選為 `[[bin]]` 加
  `required-features = ["test-support"]` 的假 stdio server（整合測試以
  `CARGO_BIN_EXE_<name>` 取路徑），或平台 shell 單行（`cmd /c`、`sh -c`）。不影響 spec
  與任務切分，實作 2.2 時決定並記在 README。
- fixture 去識別化用腳本還是手改：只影響 fixture 的產生方式，spike 1 決定。
