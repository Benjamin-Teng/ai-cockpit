# state-projection（delta）

## MODIFIED Requirements

### Requirement: 投影形狀

系統必須由狀態庫與 Domain 狀態以純函數產生 `ProjectedState`，JSON 形狀依設計文件 §6.4：頂層 `version`、
`generated_at`（RFC 3339）、`runtimes`（依設定檔順序）、`projects`（見「Project 投影」；沒有 Project 時為
空陣列）、`recent_events`；每個 runtime 含 `id`、`kind`、`endpoint`、`connection`、`focused`、`workspaces`；
`connection.state` 為 `connecting`／`connected`／`disconnected`，`connected` 另帶 `since`、
`server_version`、`protocol`、`last_snapshot_at`、`protocol_warning`（無則 `null`），`disconnected` 另帶
`reason`、`retry_in_secs`；workspace 巢狀 tabs 巢狀 panes，workspace 與 tab 依 `number` 遞增、pane 依進入
狀態庫的先後；父層不在狀態庫的物件不出現在投影；狀態值序列化為小寫字串（`working` 等）。`projects` 以外
的既有欄位名稱、型別與排序規則不因 Domain 層而改變。

#### Scenario: 巢狀投影

- **GIVEN** 狀態庫有 runtime `win`：workspace `wJ` 含 tab `wJ:t1`，其下 pane `wJ:p1`、`wJ:p2`
- **WHEN** 產生投影
- **THEN** JSON 的 `runtimes[0].workspaces[0].tabs[0].panes` 有兩筆，欄位名與 §6.4 一致

#### Scenario: 斷線狀態帶原因

- **GIVEN** runtime `wsl` 為 `Disconnected`，原因 `r`、60 秒後重試
- **WHEN** 產生投影
- **THEN** 該 runtime 的 `connection` 為 `{"state":"disconnected","reason":"r","retry_in_secs":60}`

#### Scenario: 沒有 Project

- **GIVEN** 設定檔沒有 `[[project]]`
- **WHEN** 產生投影
- **THEN** `projects` 為 `[]`，其餘欄位與沒有 Domain 層時相同

## ADDED Requirements

### Requirement: Project 投影

系統必須在 `projects` 中依設定檔順序為每個 Project 輸出：`id`、`name`、`stages`（字串陣列，設定順序）、
`warnings`（字串陣列，無則空）、`workstreams`、`tasks`。每筆 workstream 含 `id`、`name`、`binding`；
`binding.state` 為 `none`、`runtime_disconnected`、`bound`、`unbound`、`ambiguous` 之一——
`runtime_disconnected`、`unbound` 另帶 `runtime`；`ambiguous` 另帶 `runtime` 與 `candidates`（pane id
陣列）；`bound` 另帶 `runtime`、`pane_id`、`source`（`auto`／`override`）、`agent`（無則 `null`）、
`agent_status`（小寫字串）。每筆 task 含 `id`、`title`、`workstream`、`stage`（目前 Stage）、`mark`
（`none`／`completed`／`failed`）、`status`（`pending`／`ready`／`running`／`blocked`／`failed`／
`completed`）、`depends_on`（task id 陣列）。Domain 狀態改變（進度操作、覆蓋設定或取消、覆蓋失效）與
Runtime 層改變一樣觸發投影，並遵守「version 只在內容改變時遞增」與「合併廣播」。

#### Scenario: Scenario C 的 JSON 欄位

- **GIVEN** task `A` 在 `Implement`、所屬 workstream `be` 解析為 `bound` 到 `win`／`wJ:p1`
- **WHEN** `wJ:p1` 的 agent 狀態變為 `working` 後產生投影
- **THEN** `projects[0].tasks` 中 `id` 為 `A` 的項目 `stage` 為 `Implement`、`status` 為 `running`；
  `projects[0].workstreams` 中 `be` 的 `binding` 為 `{"state":"bound","runtime":"win","pane_id":"wJ:p1","source":"auto","agent":"claude","agent_status":"working"}`

#### Scenario: 進度操作遞增 version

- **GIVEN** 投影 version 為 5，Runtime 層沒有變動
- **WHEN** 一次進度操作被接受
- **THEN** 觀察者收到 version 6，其中該 task 的進度已更新

#### Scenario: 無效操作不遞增

- **GIVEN** 投影 version 為 5
- **WHEN** 一次進度操作被拒絕
- **THEN** version 仍為 5，觀察者沒有收到新的一份
