# runtime-model Specification

## Purpose

定義 Cockpit 自己的 Runtime 層模型（`cockpit-core`）：從任一 runtime 翻譯而來的 snapshot 與事件、
`AgentRuntime` 抽象、狀態庫的整份替換與逐筆套用、Drift 判定。不依賴任何 HERDR 型別
（ADR-0003、`CONTEXT.md` 兩層模型）。證據：設計文件 §6.1–§6.3。

## Requirements

### Requirement: Runtime 層型別獨立於 HERDR

系統必須在 `cockpit-core` 定義 Runtime 層型別：`RuntimeId`、`AgentStatus`（`Idle`／`Working`／
`Blocked`／`Done`／`Unknown` 五值，沒有任何「完成」值或判斷方法）、`Workspace`／`Tab`／`Pane`／
`Agent`／`Focused`、`RuntimeSnapshot`、`RuntimeEvent`、`ConnectionState`；`cockpit-core` 不得依賴
`herdr-client` 或任何其他 `cockpit-*` crate；所有 Runtime 層 id 都以 `RuntimeId` 限定才唯一。

#### Scenario: 依賴方向由 cargo 強制

- **WHEN** 檢視 `cockpit-core` 的依賴樹（`cargo tree -p cockpit-core`）
- **THEN** 不含 `herdr-client`，也不含任何 `cockpit-*` crate

#### Scenario: AgentStatus 沒有完成語意

- **WHEN** 對 `AgentStatus` 做窮舉比對
- **THEN** 只有五個值，且 `Done` 的文件註解寫明「已 idle 且尚未被看過，不是任務完成」

#### Scenario: 同名 id 在不同 runtime 互不干擾

- **GIVEN** runtime `win` 與 `wsl` 各有一個 pane `wJ:p1`
- **WHEN** 只對 `wsl` 套用 `PaneRemoved(wJ:p1)`
- **THEN** `win` 的 `wJ:p1` 仍在

### Requirement: AgentRuntime 抽象

系統必須提供 `AgentRuntime` 抽象：回報自己的 `RuntimeId`、取得一份 `RuntimeSnapshot`、建立一條
已合併的事件流（每個項目是 `RuntimeEvent` 或帶原因的錯誤）；驅動器與狀態庫只透過這個抽象與
runtime 互動，看不到 runtime 內部有幾條連線；錯誤可附「固定重試間隔」提示。

#### Scenario: 假 runtime 能替代真 runtime

- **GIVEN** 一個以腳本回應 snapshot 與事件、不含任何 HERDR 程式碼的假 runtime
- **WHEN** 交給驅動器（見 `runtime-driver`）
- **THEN** 狀態庫最終內容與腳本一致

### Requirement: 整份替換

系統必須提供以一份 `RuntimeSnapshot` 整份取代該 runtime 狀態（workspaces、tabs、panes、agents、
focused、server 版本與 protocol）的操作；替換後舊物件不殘留；替換不影響其他 runtime。

#### Scenario: 替換後舊物件消失

- **GIVEN** 狀態庫中 runtime `win` 有 pane `wJ:p1`
- **WHEN** 以不含 `wJ:p1` 的 snapshot 整份替換 `win`
- **THEN** 查不到 `wJ:p1`，snapshot 內每個物件都查得到

### Requirement: 逐筆套用

系統必須提供逐筆套用 `RuntimeEvent` 的操作，規則：`WorkspaceUpserted`／`TabUpserted`／
`PaneUpserted` 新增或整個覆蓋同 id 物件；`WorkspacesReplaced` 整份取代該 runtime 的 workspaces，
`TabsReplaced { workspace_id, tabs }` 整份取代該 workspace 的 tabs；`WorkspaceRelabeled`／
`TabRelabeled` 只改標籤；`WorkspaceRemoved` 連帶移除其下 tabs、panes、agents，`TabRemoved` 連帶
移除其下 panes、agents（HERDR 關 tab 時不另發 `pane_closed`，證據：
`herdr-client/tests/fixtures/events-lifecycle-p20.ndjson` 全部 136 行只有 `tab_closed`、沒有任何
`pane_closed`），`PaneRemoved` 連帶移除該 pane 的 agent 紀錄；`PaneMoved { previous, pane }` 移除舊 id
再以新物件新增；`PaneExited` 標記 `exited` 為 true 但不移除；`AgentDetected { pane_id, agent }` 有 agent
時設定 pane 的 agent 並新增或覆蓋 agent 紀錄、無 agent 時清除兩者；`AgentStatusChanged` 更新該 pane
與其 agent 紀錄的狀態，並在事件帶 `title`／`agent` 時一併更新；`FocusChanged` 只更新事件帶的層級，
並讓同層只有一個物件 `focused` 為 true；`Noted` 不改狀態。pane 被任何事件更動時更新 `updated_at`。

#### Scenario: 關 tab 連帶移除 pane

- **GIVEN** tab `wD:t3` 下有 pane `wD:p3` 與其 agent 紀錄
- **WHEN** 套用 `TabRemoved(wD:t3)`
- **THEN** `wD:t3`、`wD:p3` 與該 agent 紀錄都查不到，其他 tab 不受影響

#### Scenario: pane 移動後舊 id 消失

- **GIVEN** pane `wJ:p1` 在 tab `wJ:t1`
- **WHEN** 套用 `PaneMoved { previous: wJ:p1, pane: wK:p7（屬 tab wK:t1）}`
- **THEN** 查不到 `wJ:p1`，查得到 `wK:p7` 且屬於 `wK:t1`

#### Scenario: agent 釋放

- **GIVEN** pane `wJ:p1` 的 agent 為 `claude`
- **WHEN** 套用 `AgentDetected { pane_id: wJ:p1, agent: None }`
- **THEN** 該 pane 的 agent 為空，agent 紀錄中沒有 `wJ:p1`

#### Scenario: 狀態變化更新 pane 與 agent 紀錄

- **GIVEN** pane `wJ:p1` 狀態 `Idle`、agent 為 `claude`
- **WHEN** 套用 `AgentStatusChanged { pane_id: wJ:p1, status: Working, title: Some("x"), agent: None }`
- **THEN** pane 與其 agent 紀錄的狀態都是 `Working`、標題為 `x`、agent 名稱仍為 `claude`

#### Scenario: 焦點只有一個

- **GIVEN** workspace `wJ` 的 tab `wJ:t1` 為 focused
- **WHEN** 套用只帶 `tab_id: wJ:t2` 的 `FocusChanged`
- **THEN** `wJ:t2` focused、`wJ:t1` 不 focused，`focused.tab_id` 為 `wJ:t2`，workspace 與 pane 的焦點不變

### Requirement: Drift 判定

系統必須在下列情況讓套用回傳 Drift（附原因）且不改變狀態庫：事件主體 id 不在狀態庫（Removed、
Relabeled、Moved 的舊 id、Exited、AgentDetected、AgentStatusChanged）、事件帶入物件的父層 id 不在
狀態庫（`TabUpserted` 的 workspace、`PaneUpserted`／`PaneMoved` 新物件的 workspace 或 tab、
`TabsReplaced` 的 workspace）、或事件本身即為翻譯層產生的 `Drift`。Drift 不是錯誤，由驅動器決定
重拿 snapshot。

#### Scenario: 移除不存在的 pane

- **GIVEN** 狀態庫沒有 `wZ:p9`
- **WHEN** 套用 `PaneRemoved(wZ:p9)`
- **THEN** 回傳 Drift，狀態庫與套用前相等

#### Scenario: 父層不存在

- **GIVEN** 狀態庫沒有 tab `wZ:t1`
- **WHEN** 套用屬於 `wZ:t1` 的 `PaneUpserted`
- **THEN** 回傳 Drift，狀態庫與套用前相等

#### Scenario: 翻譯層的 Drift 原樣傳出

- **WHEN** 套用 `Drift { reason: "pane_created payload 缺 pane" }`
- **THEN** 回傳同一個原因的 Drift

### Requirement: 連線狀態紀錄

系統必須為每個 runtime 保存 `ConnectionState`（`Connecting`；`Connected` 含開始時間、server 版本、
protocol、最後 snapshot 時間、可選的 protocol 警告；`Disconnected` 含原因與下次重試秒數），由驅動器
寫入、投影讀出；runtime 剛登記時為 `Connecting`。

#### Scenario: 斷線原因保留

- **WHEN** 驅動器把 runtime `wsl` 設為 `Disconnected`，原因「WSL 發行版 Ubuntu-24.04 未啟動」、60 秒後重試
- **THEN** 讀出的狀態帶同一個原因與秒數，其他 runtime 的狀態不變
