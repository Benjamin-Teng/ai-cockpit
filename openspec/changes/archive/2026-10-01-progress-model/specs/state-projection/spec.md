# state-projection（delta）

## MODIFIED Requirements

### Requirement: Project 投影

系統必須在 `projects` 中依設定檔順序為每個 Project 輸出：`id`、`name`、`stages`（字串陣列，設定順序）、
`warnings`（字串陣列，無則空）、`workstreams`、`tasks`。每筆 workstream 含 `id`、`name`、`binding`、`active_task`
（目前 task 的 id，無則 `null`）、`activity_undeclared`（布林）；`binding.state` 為 `none`、`runtime_disconnected`、
`bound`、`unbound`、`ambiguous` 之一——
`runtime_disconnected`、`unbound` 另帶 `runtime`；`ambiguous` 另帶 `runtime` 與 `candidates`（pane id
陣列）；`bound` 另帶 `runtime`、`pane_id`、`source`（`auto`／`override`）、`agent`（無則 `null`）、
`agent_status`（小寫字串）。`activity_undeclared` 只在 `binding.state` 為 `bound`、`agent_status` 為 `working` 或
`blocked`、且 `active_task` 為 `null` 時為 `true`，其餘為 `false`。每筆 task 含 `id`、`title`、`workstream`、`stage`
（目前 Stage）、`mark`
（`none`／`completed`／`failed`）、`status`（`pending`／`ready`／`running`／`blocked`／`failed`／
`completed`）、`depends_on`（task id 陣列）。Domain 狀態改變（進度操作、目前 task 改變、覆蓋設定或取消、覆蓋失效）與
Runtime 層改變一樣觸發投影，並遵守「version 只在內容改變時遞增」與「合併廣播」。

#### Scenario: Scenario C 的 JSON 欄位

- **GIVEN** task `A` 在 `Implement`、所屬 workstream `be` 解析為 `bound` 到 `win`／`wJ:p1`，`be` 的目前 task 為 `A`
- **WHEN** `wJ:p1` 的 agent 狀態變為 `working` 後產生投影
- **THEN** `projects[0].tasks` 中 `id` 為 `A` 的項目 `stage` 為 `Implement`、`status` 為 `running`；
  `projects[0].workstreams` 中 `be` 的 `binding` 為 `{"state":"bound","runtime":"win","pane_id":"wJ:p1","source":"auto","agent":"claude","agent_status":"working"}`，
  `active_task` 為 `"A"`、`activity_undeclared` 為 `false`

#### Scenario: 工作中但未宣告

- **GIVEN** `be` 解析為 `bound`、`agent_status` 為 `working`，`be` 沒有目前 task
- **WHEN** 產生投影
- **THEN** `be` 的 `active_task` 為 `null`、`activity_undeclared` 為 `true`

#### Scenario: 進度操作遞增 version

- **GIVEN** 投影 version 為 5，Runtime 層沒有變動
- **WHEN** 一次進度操作被接受
- **THEN** 觀察者收到 version 6，其中該 task 的進度已更新

#### Scenario: 無效操作不遞增

- **GIVEN** 投影 version 為 5
- **WHEN** 一次進度操作被拒絕
- **THEN** version 仍為 5，觀察者沒有收到新的一份
