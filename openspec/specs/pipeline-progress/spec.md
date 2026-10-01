# pipeline-progress Specification

## Purpose

定義 Cockpit 自己的寫入面：進度操作與綁定覆蓋的 HTTP 端點、只接受本機同源請求的來源檢查，以及讓進度
與覆蓋在重啟後保留的狀態檔讀寫與容錯。這些寫入只改 Cockpit 自己的狀態，對 HERDR 完全唯讀（ADR-0001）。

## Requirements

### Requirement: 進度寫入端點

系統必須提供 `POST /api/projects/<project>/tasks/<task>/<操作>`，`<操作>` 為 `advance`（推進）、`retreat`（退回）、
`complete`（標 Completed）、`fail`（標 Failed）、`clear`（清除標記），不需要請求本體。操作被接受且
持久化成功時回 204，之後的投影反映新進度；project 或 task 不存在、或 `<操作>` 不是上述五者時回 404；
操作被拒絕時回 409，本體為 `{"error": "<原因>"}`；狀態檔寫入失敗時回 500，本體為
`{"error": "<原因>"}`，且記憶體中的進度維持操作前的值。

#### Scenario: 推進成功

- **GIVEN** task `t1` 在 `Plan`、非最後一站，投影 version 為 8
- **WHEN** `POST /api/projects/p/tasks/t1/advance`
- **THEN** 回 204；稍後 `/api/state` 的 version 大於 8，`t1` 的 `stage` 為下一站

#### Scenario: 退回成功

- **GIVEN** task `t1` 在 `Build`、非第一站、標記 `none`
- **WHEN** `POST /api/projects/p/tasks/t1/retreat`
- **THEN** 回 204；稍後投影中 `t1` 的 `stage` 為上一站

#### Scenario: 被拒絕

- **GIVEN** `t1` 在最後一站
- **WHEN** `POST /api/projects/p/tasks/t1/advance`
- **THEN** 回 409，本體 `error` 指出已是最後一個 Stage，投影中的 `t1` 不變

#### Scenario: 第一站退回被拒絕

- **GIVEN** `t1` 在第一站
- **WHEN** `POST /api/projects/p/tasks/t1/retreat`
- **THEN** 回 409，本體 `error` 指出已是第一個 Stage，投影中的 `t1` 不變

#### Scenario: 不存在的 task

- **WHEN** `POST /api/projects/p/tasks/nope/complete`
- **THEN** 回 404

#### Scenario: 人工端點沒有宣告操作

- **WHEN** `POST /api/projects/p/tasks/t1/start`
- **THEN** 回 404，狀態不變

#### Scenario: 寫檔失敗不改記憶體

- **GIVEN** 狀態檔所在目錄不可寫
- **WHEN** `POST /api/projects/p/tasks/t1/complete`
- **THEN** 回 500，投影中 `t1` 的 `mark` 仍為 `none`

### Requirement: 綁定覆蓋端點

系統必須提供 `PUT /api/projects/<project>/workstreams/<workstream>/override`（本體
`{"runtime": "<id>", "pane_id": "<pane id>"}`）設定覆蓋、`DELETE` 同一路徑取消覆蓋。設定或取消成功且
持久化成功時回 204；project 或 workstream 不存在回 404；本體不是含這兩個字串欄位的 JSON 物件回 400；
覆蓋被拒絕（runtime 未設定、未連線、pane 不存在或已 exited）回 409，本體為 `{"error": "<原因>"}`；
狀態檔寫入失敗回 500 且記憶體不變。取消不存在的覆蓋回 204。

#### Scenario: 設定覆蓋

- **GIVEN** runtime `win` 為 `connected` 且有未 exited 的 pane `wJ:p2`
- **WHEN** `PUT /api/projects/p/workstreams/be/override`，本體 `{"runtime":"win","pane_id":"wJ:p2"}`
- **THEN** 回 204；投影中 `be` 的 `binding` 為 `bound`、`pane_id` 為 `wJ:p2`、`source` 為 `override`

#### Scenario: workstream 不存在

- **WHEN** `PUT /api/projects/p/workstreams/nope/override`，本體 `{"runtime":"win","pane_id":"wJ:p2"}`
- **THEN** 回 404

#### Scenario: 覆蓋被拒絕

- **GIVEN** runtime `wsl` 為 `disconnected`
- **WHEN** `PUT /api/projects/p/workstreams/be/override`，本體 `{"runtime":"wsl","pane_id":"w1:p1"}`
- **THEN** 回 409，本體 `error` 指出 runtime 未連線，覆蓋不變

#### Scenario: 覆蓋寫檔失敗

- **GIVEN** 狀態檔所在目錄不可寫，runtime `win` 為 `connected` 且有未 exited 的 pane `wJ:p2`
- **WHEN** `PUT /api/projects/p/workstreams/be/override`，本體 `{"runtime":"win","pane_id":"wJ:p2"}`
- **THEN** 回 500，投影中 `be` 的 `binding.source` 不是 `override`

#### Scenario: 本體缺欄位

- **WHEN** `PUT` 同路徑，本體 `{"runtime":"win"}`
- **THEN** 回 400，覆蓋不變

#### Scenario: 取消覆蓋

- **GIVEN** `be` 有覆蓋
- **WHEN** `DELETE /api/projects/p/workstreams/be/override`
- **THEN** 回 204；投影中 `be` 的 `binding.source` 不再是 `override`

### Requirement: 寫入端點只接受本機同源請求

系統必須對所有寫入端點（`POST`、`PUT`、`DELETE`）檢查請求：`Host` 標頭必須是 `127.0.0.1:<port>`、
`localhost:<port>` 或 `[::1]:<port>`（`<port>` 為服務實際監聽的埠）；若帶 `Origin` 標頭，其值必須是
`http://` 加上同一個 `Host` 值。不符合時回 403 且不改任何狀態。沒有 `Origin` 標頭但 `Host` 合格的請求
（例如命令列工具）接受。

#### Scenario: 跨站表單被拒

- **WHEN** 請求帶 `Host: 127.0.0.1:7770`、`Origin: https://evil.example`
- **THEN** 回 403，進度不變

#### Scenario: DNS rebinding 被拒

- **WHEN** 請求帶 `Host: evil.example:7770`
- **THEN** 回 403

#### Scenario: 自家頁面與命令列可用

- **WHEN** 分別以 `Host: 127.0.0.1:7770`＋`Origin: http://127.0.0.1:7770`，與只有 `Host: localhost:7770`
  送出合法操作
- **THEN** 兩者都被處理（204 或依規則的 409）

### Requirement: 狀態檔格式與持久化

系統必須以 JSON 狀態檔保存進度、覆蓋與目前 task，形狀為 `{"version": 2, "projects": {"<pid>": {"tasks":
{"<tid>": {"stage": "<stage>", "mark": "none|completed|failed"}}, "overrides": {"<wid>": {"runtime":
"<id>", "pane_id": "<pane id>"}}, "active": {"<wid>": "<tid>"}}}}`（沒有目前 task 的 workstream 不出現在 `active`）。
每次操作被接受後，系統必須先把完整的新狀態寫入狀態檔（先寫同目錄暫存檔再以取代方式改名，寫入期間中斷不會留下
半份檔案），寫入成功後才讓記憶體中的狀態生效；多個寫入請求依序處理，不交錯。系統寫出的狀態檔一律為 `version: 2`。

#### Scenario: 重啟後保留

- **GIVEN** `t1` 已推進到 `Build` 並標 Completed，`be` 設了覆蓋，workstream `fe` 的目前 task 為 `f1`
- **WHEN** 停止並重新啟動系統（同一份設定與狀態檔）
- **THEN** `t1` 在 `Build`、標記 `completed`；`be` 的覆蓋仍在（依覆蓋規則解析）；`fe` 的目前 task 為 `f1`

#### Scenario: 並發寫入不遺失

- **GIVEN** task `a`、`b` 皆標記 `none`
- **WHEN** 幾乎同時送出 `a` 的 complete 與 `b` 的 fail
- **THEN** 兩者都回 204，狀態檔中 `a` 為 `completed`、`b` 為 `failed`

### Requirement: 狀態檔載入與容錯

系統必須在啟動時（Project 清單非空時）讀取狀態檔：檔案不存在 → 所有 task 用初始進度、沒有覆蓋、沒有目前 task，
不立即建立檔案；`version` 為 1 → 依舊形狀（沒有 `active`）讀取，所有 workstream 沒有目前 task，下次寫入時寫成
`version: 2`；檔案無法解析為對應版本的形狀、或 `version` 不是 1 或 2 → 啟動失敗，訊息含狀態檔路徑與原因；檔案
中的 project、task、workstream 在設定檔不存在，或覆蓋的 `runtime` 不是設定檔中的 runtime → 忽略並記
warn，下次寫入時不再寫出；`active` 中的項目指向設定檔不存在的 workstream 或 task、task 不屬於該 workstream、
或 task 載入後的標記不是 `none`、或該 workstream 的覆蓋在這次載入中因無效而被忽略 → 忽略該項目並記 warn，下次寫入時不再寫出；task 的 `stage`
不在該 Project 的 `stages` → 該 task 改用設定檔的起始 stage、保留標記，並在該 Project 的投影
`warnings` 中加入指出 task id 與原 stage 值的一則訊息；設定檔中有、狀態檔中沒有的 task → 用初始進度。

#### Scenario: 狀態檔損毀

- **GIVEN** 狀態檔內容為 `{not json`
- **WHEN** 啟動
- **THEN** 啟動失敗，訊息含狀態檔路徑

#### Scenario: 讀取 v1 舊檔

- **GIVEN** 狀態檔為 `{"version":1,"projects":{"p":{"tasks":{"t1":{"stage":"Build","mark":"none"}},"overrides":{}}}}`
- **WHEN** 啟動後對 `t1` 做一次被接受的操作
- **THEN** 啟動成功，`t1` 在 `Build`；新寫出的狀態檔 `version` 為 2，且 `p` 含 `active` 欄位

#### Scenario: 不支援的版本

- **GIVEN** 狀態檔 `version` 為 3
- **WHEN** 啟動
- **THEN** 啟動失敗，訊息含狀態檔路徑

#### Scenario: 無效的目前 task

- **GIVEN** v2 狀態檔中 `active` 為 `{"be": "f1"}`，但 `f1` 屬於 workstream `fe`
- **WHEN** 啟動
- **THEN** 啟動成功，`be` 沒有目前 task

#### Scenario: stage 被改名

- **GIVEN** 設定檔 stages 由 `["Spec","Build"]` 改為 `["Spec","Implement"]`，狀態檔中 `t1` 為
  `{"stage":"Build","mark":"none"}`，`t1` 的起始 stage 為 `Spec`
- **WHEN** 啟動
- **THEN** 啟動成功，`t1` 在 `Spec`；Project 投影的 `warnings` 有一則含 `t1` 與 `Build` 的訊息

#### Scenario: 設定檔刪掉的 task

- **GIVEN** 狀態檔有 task `old`，設定檔已沒有 `old`
- **WHEN** 啟動後對其他 task 做一次被接受的操作
- **THEN** 啟動成功，新寫出的狀態檔不含 `old`
