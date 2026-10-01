# herdr-runtime-session（delta）

## MODIFIED Requirements

### Requirement: 讀取 pane 輸出

系統必須以 `pane.read` 實作 `AgentRuntime` 的讀取輸出：參數固定為 `source` = `recent`、`format` = `ansi`、
`lines` = 呼叫端給的行數上限，不送 `strip_ansi`；回應的 `text` 依 `live-output`「輸出樣式轉換」轉成純文字與帶樣式的片段後交回，
`truncated` 原樣交回，不使用回應的 `revision`（見 `herdr-observer-types`），也不檢查回應的 `format`（回應不含控制序列時即為
一段無樣式的片段）。HERDR 回錯誤碼 `pane_not_found` 時回報「pane 不存在」；其他任何錯誤
（連不上、其他錯誤碼、回應無法解析）回報帶原因的失敗，不得誤報為「pane 不存在」；讀取輸出不做 WSL 探測、
不自行重試。同一個 runtime 同一時間至多一筆
進行中的 `pane.read`，其餘排隊等候；輸出讀取不得影響進行中的事件流與 snapshot。

#### Scenario: 送出的參數

- **WHEN** 以行數上限 200 讀取 pane `w1:p1` 的輸出
- **THEN** 假 HERDR 收到 `pane.read`，params 為 `pane_id` = `w1:p1`、`source` = `recent`、`format` = `ansi`、
  `lines` = 200，沒有 `strip_ansi`

#### Scenario: 回應原樣交回

- **GIVEN** 假 HERDR 回應 `text` 為 `a\nb`、`truncated` 為 `true`、`revision` 為 0
- **WHEN** 讀取輸出
- **THEN** 得到文字 `a\nb`、單一段無樣式的片段、`truncated` 為 `true`、格式為純文字

#### Scenario: 帶樣式的回應

- **GIVEN** 假 HERDR 回應 `text` 為 `ESC[0mESC[38;5;1merrESC[0m ok\r\n`、`truncated` 為 `false`
- **WHEN** 讀取輸出
- **THEN** 得到文字 `err ok\n`，片段為 `err`（`fg: red`）與其後的空格加 `ok\n`（無樣式）

#### Scenario: pane 不存在

- **GIVEN** 假 HERDR 對 `pane.read` 回錯誤碼 `pane_not_found`
- **WHEN** 讀取輸出
- **THEN** 得到「pane 不存在」錯誤

#### Scenario: 連不上

- **GIVEN** HERDR 端點不存在
- **WHEN** 讀取輸出
- **THEN** 得到帶原因的失敗，且不是「pane 不存在」

#### Scenario: 其他錯誤碼

- **GIVEN** 假 HERDR 對 `pane.read` 回錯誤碼 `internal_error`
- **WHEN** 讀取輸出
- **THEN** 得到帶原因的失敗，原因含該錯誤碼，且不是「pane 不存在」

#### Scenario: 同 runtime 不並發

- **GIVEN** 假 HERDR 對每筆 `pane.read` 延遲 200 ms 才回應
- **WHEN** 同時發起 4 筆輸出讀取
- **THEN** 4 筆都成功，且假 HERDR 觀察到同時進行中的 `pane.read` 連線數從未超過 1

#### Scenario: 不干擾事件流

- **GIVEN** 事件流已建立並持續收到事件
- **WHEN** 期間讀取輸出 10 次
- **THEN** 事件流沒有中斷、沒有重開訂閱
