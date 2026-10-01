# agent-reporting Specification

## Purpose

定義 agent 從自己所在的 HERDR pane 主動呼叫 Cockpit 回報進度的 HTTP 入口：以 pane id 表明身分、查詢自己綁定的 task、
宣告目前 task、推進 task。寫入只改 Cockpit 自己的狀態，對 HERDR 完全唯讀。

## Requirements

### Requirement: pane 身分判定

系統必須要求所有 agent 端點（路徑以 `/api/agent/` 開頭）的請求帶 `X-Herdr-Pane-Id` 標頭，其值為 agent 所在 pane 的
pane id（即 HERDR 在 pane 內提供的 `HERDR_PANE_ID` 環境變數）；標頭缺少或去除前後空白後為空時回 400，本體
`{"error": "<原因>", "code": "missing_pane_id"}`。一條 workstream「綁定到這個 pane」的條件是：它在最新投影中的
`binding.state` 為 `bound`（`source` 為 `auto` 或 `override` 皆可）、`binding.pane_id` 等於標頭值，且 `binding.runtime`
不是經由 WSL 連線的 runtime。若兩個以上的非 WSL runtime 上都存在 pane id 等於標頭值、且未 exited 的 pane（不論是否
綁定），則視為沒有任何 workstream 綁定到這個 pane。寫入（宣告、推進）生效時，身分判定所依據的覆蓋事實必須仍成立：
判定時該 workstream 的綁定來源為 `override`，則寫入時它的覆蓋必須仍是同一個 runtime 與 pane id；來源為 `auto`，則寫入
時它必須仍沒有覆蓋；不成立時回 403，本體 `code` 為 `pane_not_bound`，狀態不變。agent 端點與寫入端點套用同一個本機
同源來源檢查（見 `pipeline-progress`「寫入端點只接受本機同源請求」）。

#### Scenario: 缺標頭

- **WHEN** `GET /api/agent/tasks`，沒有 `X-Herdr-Pane-Id`
- **THEN** 回 400，本體 `code` 為 `missing_pane_id`

#### Scenario: WSL runtime 的 pane 不算

- **GIVEN** workstream `be` 綁定到經由 WSL 連線的 runtime `wsl` 的 pane `w1:p1`
- **WHEN** `GET /api/agent/tasks`，`X-Herdr-Pane-Id: w1:p1`
- **THEN** 回 200，`workstreams` 為空陣列

#### Scenario: 兩個 Windows runtime 撞號

- **GIVEN** 非 WSL runtime `win` 有 workstream `be` 綁定到 pane `w1:p1`；非 WSL runtime `win2` 也有一個未 exited 的 pane
  `w1:p1`，沒有綁定任何 workstream
- **WHEN** `GET /api/agent/tasks`，`X-Herdr-Pane-Id: w1:p1`
- **THEN** 回 200，`workstreams` 為空陣列

#### Scenario: 改綁後舊 pane 的宣告被拒

- **GIVEN** workstream `be` 自動綁定到 `wJ:p1`；使用者剛把 `be` 的覆蓋設為 `wK:p2` 並收到 204，投影尚未反映改綁
- **WHEN** `POST /api/agent/projects/p/tasks/t1/start`，`X-Herdr-Pane-Id: wJ:p1`
- **THEN** 回 403，本體 `code` 為 `pane_not_bound`，`be` 沒有目前 task

#### Scenario: 跨站請求被拒

- **WHEN** `POST /api/agent/projects/p/tasks/t1/start`，帶 `Origin: https://evil.example`
- **THEN** 回 403，狀態不變

### Requirement: 查詢自己綁定的 task

系統必須提供 `GET /api/agent/tasks`，回 200，本體為 `{"pane_id": "<標頭值>", "workstreams": [...]}`。`workstreams` 依
Project 設定順序、再依 workstream 設定順序，列出所有綁定到這個 pane 的 workstream，每筆含 `project`（project id）、
`workstream`（workstream id）、`active_task`（目前 task 的 id，無則 `null`）、`tasks`。`tasks` 依設定順序列出該 workstream
的每個 task，每筆含 `id`、`title`、`stage`、`next_stage`（下一個 Stage，已是最後一站則 `null`）、`mark`、`status`、
`depends_on`，值與同一時刻的投影一致。沒有任何綁定時 `workstreams` 為空陣列。

#### Scenario: 列出綁定的 task

- **GIVEN** Project `p` 的 stages 為 `Plan`、`Build`；workstream `be` 綁定到 `win` 的 `wJ:p1`，有 task `t1`（在 `Plan`）、
  `t2`（在 `Build`）；目前 task 為 `t1`
- **WHEN** `GET /api/agent/tasks`，`X-Herdr-Pane-Id: wJ:p1`
- **THEN** 回 200；`workstreams` 只有一筆，`project` 為 `p`、`workstream` 為 `be`、`active_task` 為 `t1`；`t1` 的 `next_stage`
  為 `Build`，`t2` 的 `next_stage` 為 `null`

### Requirement: 宣告目前 task

系統必須提供 `POST /api/agent/projects/<project>/tasks/<task>/start`，不需要請求本體，把該 task 設為其 workstream 的目前
task（見 `pipeline-domain`「目前 task」）。接受且持久化成功時回 204；project 或 task 不存在回 404；task 所屬 workstream
沒有綁定到這個 pane 回 403，本體 `code` 為 `pane_not_bound`；宣告被拒絕（task 已有標記）回 409，本體為
`{"error": "<原因>"}`；狀態檔寫入失敗回 500 且記憶體不變。

#### Scenario: 宣告成功

- **GIVEN** `be` 綁定到 `wJ:p1`、`wJ:p1` 為 `working`，`t1`、`t2` 標記皆為 `none`、無依賴
- **WHEN** `POST /api/agent/projects/p/tasks/t2/start`，`X-Herdr-Pane-Id: wJ:p1`
- **THEN** 回 204；稍後投影中 `be` 的 `active_task` 為 `t2`，`t2` 為 `running`、`t1` 為 `ready`

#### Scenario: 動別人的 task

- **GIVEN** workstream `fe` 綁定到 `wJ:p2`，task `f1` 屬於 `fe`
- **WHEN** `POST /api/agent/projects/p/tasks/f1/start`，`X-Herdr-Pane-Id: wJ:p1`
- **THEN** 回 403，本體 `code` 為 `pane_not_bound`，`fe` 的目前 task 不變

#### Scenario: 已標記的 task 不能宣告

- **GIVEN** `t1` 標記為 `completed`，`be` 綁定到 `wJ:p1`
- **WHEN** `POST /api/agent/projects/p/tasks/t1/start`，`X-Herdr-Pane-Id: wJ:p1`
- **THEN** 回 409，目前 task 不變

### Requirement: agent 推進

系統必須提供 `POST /api/agent/projects/<project>/tasks/<task>/advance`，不需要請求本體。身分判定、404、403、500 同「宣告
目前 task」；推進規則與 `pipeline-domain`「進度操作」的推進相同，被拒絕時回 409 且不改任何狀態。接受時在同一次持久化中
把 task 推進到下一站並設為其 workstream 的目前 task，回 204。agent 端點不提供標 Completed、標 Failed、清除標記、退回：
`/api/agent/projects/<project>/tasks/<task>/<操作>` 的 `<操作>` 不是 `start` 或 `advance` 時回 404。

#### Scenario: 推進並設為目前 task

- **GIVEN** `t1` 在 `Plan`、標記 `none`，`be` 綁定到 `wJ:p1`，`be` 沒有目前 task
- **WHEN** `POST /api/agent/projects/p/tasks/t1/advance`，`X-Herdr-Pane-Id: wJ:p1`
- **THEN** 回 204；稍後投影中 `t1` 在 `Build`，`be` 的 `active_task` 為 `t1`；重啟後兩者都保留

#### Scenario: 最後一站推進被拒

- **GIVEN** `t2` 在 `Build`（最後一站），`be` 綁定到 `wJ:p1`，`be` 的目前 task 為 `t1`
- **WHEN** `POST /api/agent/projects/p/tasks/t2/advance`，`X-Herdr-Pane-Id: wJ:p1`
- **THEN** 回 409；`t2` 仍在 `Build`，`be` 的目前 task 仍為 `t1`

#### Scenario: agent 不能標完成

- **WHEN** `POST /api/agent/projects/p/tasks/t1/complete`，`X-Herdr-Pane-Id: wJ:p1`
- **THEN** 回 404，`t1` 的標記不變
