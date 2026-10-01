# pipeline-progress（delta）

## MODIFIED Requirements

### Requirement: 狀態檔載入與容錯

系統必須在啟動時（Project 清單非空時）讀取狀態檔：檔案不存在 → 所有 task 用初始進度、沒有覆蓋、沒有目前 task，
不立即建立檔案；`version` 為 1 → 依舊形狀（沒有 `active`）讀取，所有 workstream 沒有目前 task，下次寫入時寫成
`version: 2`；`version` 為 1 的檔案中，任一 project 出現 `active` 欄位（不論值為何，含 `null` 與空物件）即視為損毀，啟動失敗，
訊息含狀態檔路徑與原因；檔案無法解析為對應版本的形狀、或 `version` 不是 1 或 2 → 啟動失敗，訊息含狀態檔路徑與原因；檔案
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

#### Scenario: v1 檔出現 active 視為損毀

- **GIVEN** 狀態檔為 `{"version":1,"projects":{"p":{"tasks":{"t1":{"stage":"Build","mark":"none"}},"overrides":{},"active":{}}}}`
- **WHEN** 啟動
- **THEN** 啟動失敗，訊息含狀態檔路徑與原因

#### Scenario: v1 檔的 active 為 null 也視為損毀

- **GIVEN** 狀態檔為 `{"version":1,"projects":{"p":{"tasks":{"t1":{"stage":"Build","mark":"none"}},"overrides":{},"active":null}}}`
- **WHEN** 啟動
- **THEN** 啟動失敗，訊息含狀態檔路徑與原因

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
