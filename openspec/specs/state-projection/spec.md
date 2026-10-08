# state-projection Specification

## Purpose

定義給畫面看的整張圖 `ProjectedState`：JSON 形狀、`version` 語意、合併廣播、最近事件、Project（Pipeline）投影。
證據：設計文件 §6.3、§6.4、ADR-0004。

## Requirements

### Requirement: 投影形狀

系統必須由狀態庫與 Domain 狀態以純函數產生 `ProjectedState`，JSON 形狀依設計文件 §6.4：頂層 `version`、
`generated_at`（RFC 3339）、`runtimes`（依設定檔順序）、`projects`（見「Project 投影」；沒有 Project 時為
空陣列）、`detected_repos`（見 `repo-projects`「偵測到的 repo 清單」；沒有時為空陣列）、`recent_events`；每個 runtime 含
`id`、`kind`、`endpoint`、`connection`、`focused`、`workspaces`；
`connection.state` 為 `connecting`／`connected`／`disconnected`，`connected` 另帶 `since`、
`server_version`、`protocol`、`last_snapshot_at`、`protocol_warning`（無則 `null`），`disconnected` 另帶
`reason`、`retry_in_secs`；workspace 巢狀 tabs 巢狀 panes，workspace 與 tab 依 `number` 遞增、pane 依進入
狀態庫的先後；父層不在狀態庫的物件不出現在投影；狀態值序列化為小寫字串（`working` 等）。`projects` 與 `detected_repos` 以外
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

- **GIVEN** 設定檔沒有 `[[project]]`，也沒有 Repo Project，且沒有 pane 位於 git repo 內
- **WHEN** 產生投影
- **THEN** `projects` 與 `detected_repos` 皆為 `[]`，其餘欄位與沒有 Domain 層時相同

#### Scenario: 偵測到的 repo 在最上層

- **GIVEN** 有 pane 位於尚未加入的 git repo `app` 內
- **WHEN** 產生投影
- **THEN** 頂層 `detected_repos` 有一筆 `{"repo":"<repo key>","name":"app","pane_count":1}`，`projects` 不含 `app`

### Requirement: version 只在內容改變時遞增

系統必須比較新投影與前一份（忽略 `generated_at`），相等則沿用前一份的 `version` 且不廣播；不相等
則 `version` 加一並廣播整份。

#### Scenario: 沒變不遞增

- **GIVEN** 已產生 version 5 的投影
- **WHEN** 狀態庫沒有任何變動又觸發一次投影
- **THEN** version 仍為 5，觀察者沒有收到新的一份

#### Scenario: 有變才遞增

- **GIVEN** version 5
- **WHEN** 套用一筆改變 pane 狀態的事件
- **THEN** 觀察者收到 version 6 的完整投影

### Requirement: 合併廣播

系統必須把 50 ms 內的多次狀態變動合併成一次投影與廣播；新加入的觀察者立即取得目前的一份。

#### Scenario: 連續變動合併

- **GIVEN** 時間可控
- **WHEN** 在 10 ms 內套用 10 筆事件
- **THEN** 觀察者最多收到 2 份，最後一份反映全部 10 筆

#### Scenario: 新觀察者立即取得現況

- **GIVEN** 目前 version 7
- **WHEN** 新觀察者加入
- **THEN** 立即取得 version 7 的整份

### Requirement: 最近事件

系統必須為每個 runtime 保留最近 50 筆事件紀錄（`at`、`runtime`、`kind`、主體 id、`detail`），投影時
把所有 runtime 的紀錄合併、最新在前、最多 50 筆；`Noted` 事件只進紀錄不改狀態；snapshot 前被丟棄的
事件不進紀錄。

#### Scenario: 超過 50 筆丟最舊

- **WHEN** 對同一 runtime 套用 60 筆事件
- **THEN** 紀錄只有最新 50 筆

#### Scenario: Noted 只進紀錄

- **GIVEN** 狀態庫某內容
- **WHEN** 套用 `Noted { kind: "layout_updated" }`
- **THEN** 狀態庫內容不變、最近事件第一筆的 `kind` 為 `layout_updated`

### Requirement: Project 投影

系統必須在 `projects` 中為每個 Project 輸出：`id`、`name`、`kind`（手寫於設定檔的為 `config`，畫面加入的 Repo Project 為
`repo`）、`stages`（字串陣列，設定順序）、`warnings`（字串陣列，無則空）、`workstreams`、`tasks`；Repo Project 另帶 `repo`
（repo key），手寫 project 沒有 `repo` 欄位。順序為：手寫 project 依設定檔順序，其後是 Repo Project 依名稱（不分大小寫）排序、同名再依 `id`。
每筆 workstream 含 `id`、`name`、`binding`、`active_task`
（目前 task 的 id，無則 `null`）、`activity_undeclared`（布林）；Repo Project 位於 linked worktree 的 pane 所產生的 workstream
另帶 `worktree`（worktree 資料夾名稱），其餘 workstream 沒有 `worktree` 欄位。`binding.state` 為 `none`、`runtime_disconnected`、
`bound`、`unbound`、`ambiguous` 之一——
`unbound` 另帶 `runtime`（Repo Project 的 `unbound` 另帶 `source`，值為 `pane`）；`runtime_disconnected` 另帶 `runtime` 與 `source`（`auto`／`override`／`pane`，
與 `bound` 的 `source` 同義：這個斷線的綁定來自自動解析、使用者覆蓋，或 Repo Project 的固定 pane）；`ambiguous` 另帶 `runtime` 與 `candidates`（pane id
陣列）；`bound` 另帶 `runtime`、`pane_id`、`source`（`auto`／`override`／`pane`，`pane` 只出現在 Repo Project 的工作線）、`agent`（無則 `null`）、
`agent_status`（小寫字串）。`activity_undeclared` 只在 `binding.state` 為 `bound`、`agent_status` 為 `working` 或
`blocked`、且 `active_task` 為 `null` 時為 `true`，其餘為 `false`（Repo Project 的 task 標記為 `none` 時就是目前 task，見
`repo-projects`）。每筆 task 含 `id`、`title`、`workstream`、`stage`
（目前 Stage）、`mark`
（`none`／`completed`／`failed`）、`status`（`pending`／`ready`／`running`／`blocked`／`failed`／
`completed`）、`depends_on`（task id 陣列）。Domain 狀態改變（進度操作、目前 task 改變、覆蓋設定或取消、覆蓋失效、
Repo Project 的加入／修改／移除、pane 歸類結果改變）與
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

#### Scenario: 覆蓋綁定的 runtime 斷線

- **GIVEN** workstream `be` 的覆蓋指向 `wsl`／`w1:p1`，`wsl` 為 `disconnected`
- **WHEN** 產生投影
- **THEN** `be` 的 `binding` 為 `{"state":"runtime_disconnected","runtime":"wsl","source":"override"}`

#### Scenario: 自動綁定的 runtime 斷線

- **GIVEN** workstream `be` 沒有覆蓋、設定檔 binding 的 `runtime` 為 `wsl`，`wsl` 為 `disconnected`
- **WHEN** 產生投影
- **THEN** `be` 的 `binding` 為 `{"state":"runtime_disconnected","runtime":"wsl","source":"auto"}`

#### Scenario: 手寫 project 與 Repo Project 的 kind

- **GIVEN** 設定檔有手寫 project `h`；已加入 Repo Project `app`（repo key `d:\work\app\.git`）
- **WHEN** 產生投影
- **THEN** `projects` 依序為 `h` 與 `app`；`h` 的 `kind` 為 `config` 且沒有 `repo` 欄位；`app` 的 `kind` 為 `repo`、`repo` 為
  `d:\work\app\.git`

#### Scenario: Repo Project 的工作線與 task

- **GIVEN** Repo Project `app` 的 stages 為 `["Plan","Build"]`；runtime `local` 的 pane `wJ:p1`（label `backend`，agent
  `claude`，`working`）位於 linked worktree `app-wt`，task 在 `Plan`
- **WHEN** 產生投影
- **THEN** `app.workstreams[0]` 為 `id` `local~wJ:p1`、`name` `backend`、`worktree` `app-wt`、`binding` 為
  `{"state":"bound","runtime":"local","pane_id":"wJ:p1","source":"pane","agent":"claude","agent_status":"working"}`、
  `active_task` `local~wJ:p1`、`activity_undeclared` `false`；`app.tasks[0]` 為 `id` `local~wJ:p1`、`title` `backend`、
  `workstream` `local~wJ:p1`、`stage` `Plan`、`mark` `none`、`status` `running`、`depends_on` `[]`

#### Scenario: Project 與偵測到的 repo 的排序

- **GIVEN** 設定檔有手寫 project `z`、`a`；Repo Project 名稱為 `beta`（id `beta`）、`Alpha`（id `alpha-1`）、`alpha`（id `alpha-2`）；
  `detected_repos` 有兩個同名 `lib` 的 repo（repo key 不同）
- **WHEN** 產生投影
- **THEN** `projects` 依序為 `z`、`a`、`alpha-1`、`alpha-2`、`beta`（名稱不分大小寫排序、同名依 id）；兩個 `lib` 依 `repo` 排序；`detected_repos` 的名稱比較不分大小寫（`app` 在 `Lib` 之前）

#### Scenario: 位於主 worktree 的 pane 沒有 worktree 欄位

- **GIVEN** Repo Project 的某 pane 位於主 worktree
- **WHEN** 產生投影
- **THEN** 該 workstream 的 JSON 沒有 `worktree` 欄位

#### Scenario: pane 歸類結果改變遞增 version

- **GIVEN** 投影 version 為 5，Repo Project `app` 有一條 workstream
- **WHEN** 在 `app` 內新開一個 pane 且歸類完成
- **THEN** 觀察者收到 version 6，`app` 多一條 workstream 與一張 task
