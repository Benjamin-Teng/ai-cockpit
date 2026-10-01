# design：progress-model

## Context

動機見 proposal「Why」，行為以本 change 的 delta specs 為準。現況（2026-10-01 `main`）：

- 進度的純規則在 `cockpit-core/src/domain/progress.rs`（`ProgressOp`、`apply_op`）；`DomainState`（`domain/state.rs`）存
  `progress`、`overrides`、`warnings`。StageStatus 由 `domain/status.rs` 的 `derive_status(mark, dependency_marks, binding,
  agent_status)` 推導，`projection.rs` 對同一 workstream 的每個 task 傳入同一份綁定與 agent 狀態——這就是「整條 workstream
  一起 running」的來源。
- 唯一寫入者是 `cockpit/src/progress_service.rs`：一把 `tokio::Mutex` 內「讀 → 純函數算 → 相同不落檔 → 暫存檔＋rename →
  成功才 `set_domain`」，交易跑在自己 spawn 的 task 防取消；失效覆蓋由 `remove_stale` 在同一路徑移除。
- 狀態檔讀寫在 `cockpit/src/progress.rs`（`deny_unknown_fields`、`version` 必須為 1）。
- HTTP 在 `cockpit/src/http.rs`；寫入與敏感讀取路由套 `source_check.rs` 的本機同源檢查，沒有身分驗證。
- runtime 是否經由 WSL：`cockpit-herdr/src/factory.rs` 的 `HerdrEndpoint::Wsl`，由 `cockpit/src/config.rs` 的
  `RuntimeConfig.endpoint` 帶入。
- HERDR 在每個 pane 的環境變數提供 `HERDR_PANE_ID`（2026-10-01 在 Windows 端 HERDR 0.9.2 preview 的 pane 內實測：
  `HERDR_PANE_ID=wW:p1`，格式與投影的 `pane_id` 相同；同時有 `HERDR_WORKSPACE_ID`、`HERDR_TAB_ID`、`HERDR_SOCKET_PATH`）。

## Goals / Non-Goals

**Goals:**

- 「目前 task」是 Domain 狀態的一部分，與進度、覆蓋走同一條持久化與投影路徑，不另開儲存或通道。
- agent 端點只是既有 `ProgressService` 交易的另一個入口；規則（含推進規則）只寫一次，在 `cockpit-core`。
- 人工端點與 agent 端點的權限差異只存在 HTTP 路由層。

**Non-Goals:**

- 不做 pane 身分的密碼學驗證；`X-Herdr-Pane-Id` 可被任何本機程式偽造，用途是防誤操作（打錯 id 動到別條 workstream）。
- 不追蹤目前 task 的歷史、宣告時間或宣告者（投影不得含隨時間變動的欄位，ADR-0004 的 `content_eq`）。

## Decisions

### D1 目前 task 存在 `DomainState`，清除規則寫在純函數裡

`DomainState` 新增 `active: HashMap<ProjectId, HashMap<WorkstreamId, TaskId>>`。`cockpit-core` 提供純函數處理：

- 設定目前 task：驗證 task 屬於該 workstream、標記為 `none`，否則回新的 `Rejection` 變體。
- `apply_op` 處理 `Complete`／`Fail` 時，若該 task 是目前 task 就一併清除。
- 覆蓋的設定、取消、失效移除，一併清除該 workstream 的目前 task。

替代方案：把目前 task 放在 `cockpit` crate 的 service 層、只在記憶體。否決理由是重啟會丟，而且清除規則會散落到 HTTP 和
service 兩處。

### D2 `derive_status` 多一個 `is_active: bool` 輸入

規則改成：第 (3) 步只在 `is_active` 時看 agent 狀態，其餘回 `Ready`。`activity_undeclared` 不由 `derive_status` 算，
由 `projection.rs` 在組 workstream 時算（綁定為 Bound、agent 為 Working／Blocked、沒有目前 task）。

替代方案：傳整個 active 對應表進 `derive_status`。否決理由是它只需要一個布林，傳表會讓純函數知道太多。

### D3 agent 推進是一筆交易

`ProgressService` 新增一個交易：在同一把鎖內先 `apply_op(Advance)`，成功再設定目前 task，最後只落檔一次。任一步被拒絕
就整筆不生效。宣告（`start`）是另一個只設定目前 task 的交易。兩者都沿用「相同不落檔」：重複宣告同一個 task 回 204、
不寫檔、version 不遞增。

### D4 pane 身分用最新投影判定

agent 端點的 handler 先讀最新一份投影，找出 `binding.state == bound`、`pane_id` 等於標頭值的 workstream，再以設定檔排除
`HerdrEndpoint::Wsl` 的 runtime；同一 pane id 出現在兩個以上非 WSL runtime 時一律視為未綁定。通過後才呼叫
`ProgressService`。

替代方案：在 `ProgressService` 的鎖內重新解析綁定。否決理由是綁定來自 Runtime 層，本來就不受這把鎖保護；鎖內重解析
不會消除競態，只會讓 service 依賴 Runtime 層。但「人工改綁」這一種變動只存在 Domain 狀態，可以在鎖內檢查：handler 把判定
依據的覆蓋事實（來源為 `override` 時的 runtime 與 pane id，或來源為 `auto`）傳給 service，service 在鎖內比對 Domain 的覆蓋，
不符就回 `pane_not_bound`。這樣「改綁 204 之後、投影重算之前」舊 pane 的宣告會被拒（Opus 審查 M1 重現過這個情境）。剩下的
競態只有自動綁定本身的變動（pane 出現或消失），它不清目前 task，影響限於那一次宣告。

### D5 狀態檔 v2：單一結構、`active` 可選，載入時依版本檢查

序列化結構新增 `active: Option<BTreeMap<String, String>>`。載入規則：`version == 1` 時 `active` 必須不存在；
`version == 2` 時 `active` 必須存在（可為空物件）；其他版本啟動失敗。寫出一律 `version: 2`、每個 project 都帶 `active`。
`active` 的無效項目在載入時忽略並記 warn（spec「狀態檔載入與容錯」）。

替代方案：v1、v2 兩個獨立 struct 加 `#[serde(untagged)]`。否決理由是錯誤訊息會變成「無法匹配任何變體」，失去原本
`deny_unknown_fields` 指出哪個欄位錯的能力。

### D6 路由與錯誤碼

- 人工：`POST /api/projects/{p}/tasks/{t}/{op}` 的 `parse_progress_op` 多接受 `retreat`。
- agent：`GET /api/agent/tasks`、`POST /api/agent/projects/{p}/tasks/{t}/{op}`（`op` 只接受 `start`、`advance`，其他回
  404）。兩條都套 `source_check`。
- 錯誤本體沿用 `{"error": "..."}`；新錯誤加 `code`（`missing_pane_id`、`pane_not_bound`），比照 `forbidden_source`。
- 判定順序：來源檢查 → 標頭（400）→ project／task 存在（404）→ 綁定（403）→ 規則（409）→ 落檔（500）。先查存在再查
  綁定，是為了讓「打錯 id」得到 404 而不是誤導的 403。

### D7 畫面

`actions.js` 的 `TASK_OPS` 多 `retreat`，`render.js` 的 `renderTaskActions` 依 spec 顯示「退回」。「工作中・未宣告
task」放在 workstream 列首、綁定摘要之後，用警示色文字（direction-01 的既有 token），沒有動畫。實作前先讀
`docs/direction-01-visual-design.md` 與 `render.js` 現有列首結構，並依 memory 過 frontend-design 審核。

### D8 給 agent 的使用說明

`cockpit/README.md` 新增一節「agent 回報進度」：三個端點、標頭、錯誤碼，以及 PowerShell（`curl.exe`，避開
`Invoke-WebRequest` 的 `curl` 別名）與 bash 的範例，附一段可直接貼進專案 `AGENTS.md` 的短文：開始做某 task 時呼叫
`start`、完成一站時呼叫 `advance`、不要嘗試標完成。

## Risks / Trade-offs

- [agent 不宣告 → 畫面全部 `ready`，看起來比以前「安靜」] → `activity_undeclared` 提示讓你看得到；README 範例讓新專案
  一開始就接上。這是使用者選定的取捨（不猜）。
- [WSL 內的 agent 打不到 API] → 本 change 不處理。之後的選項：WSL mirrored 網路模式（改 `.wslconfig`，Cockpit 不用改）
  或 Cockpit 另外監聽 WSL 虛擬網卡（要重新設計來源檢查）。記在交接手冊的延後清單。
- [`X-Herdr-Pane-Id` 可偽造] → 與現行信任模型一致（本機任何程式本來就能直接打人工端點）；不宣稱是安全邊界。
- [身分檢查與寫入之間的競態（D4）] → 人工改綁在鎖內比對 Domain 覆蓋擋下；只剩自動綁定變動時的單次宣告。
- [驗收腳本與 `ui_preview` 情境依賴「整條 workstream 一起 running」] → 只改 spec 已改變的斷言，腳本修改與產品修改分開
  commit 並列出被改的斷言。

## Migration Plan

- 升級：直接換新版，舊 v1 狀態檔照常讀取，第一次寫入時存成 v2。
- 回退：舊版 Cockpit 讀到 v2 會啟動失敗。手動把 `version` 改回 1、刪掉每個 project 的 `active` 欄位即可回退，進度與
  覆蓋不受影響。這段寫進 `cockpit/README.md` 狀態檔一節。
