# herdr-runtime-session（delta）

## RENAMED Requirements

- FROM: `### Requirement: 只用兩個 method`
- TO: `### Requirement: 只用唯讀 method`

## MODIFIED Requirements

### Requirement: 只用唯讀 method

系統必須只對 HERDR 送出 `session.snapshot`、`events.subscribe` 與 `pane.read` 三個唯讀 method，每次 request
或訂閱各開一條連線（經 `herdr-client`）；`pane.read` 只在有人要求讀取 pane 輸出時才送出，建立事件流、取
snapshot、重開狀態訂閱的過程不得送出它。

#### Scenario: 假 HERDR 收到的 method 集合

- **WHEN** 完整跑過建立事件流、取 snapshot、重開狀態訂閱
- **THEN** 假 HERDR 收到的 method 只有 `session.snapshot` 與 `events.subscribe`

#### Scenario: 讀取輸出只多一種 method

- **WHEN** 在上述過程之外另外讀取一次 pane 輸出
- **THEN** 假 HERDR 多收到一筆 `pane.read`，沒有其他 method

## ADDED Requirements

### Requirement: 讀取 pane 輸出

系統必須以 `pane.read` 實作 `AgentRuntime` 的讀取輸出：參數固定為 `source` = `recent`、`format` = `text`、
`lines` = 呼叫端給的行數上限，不送 `strip_ansi`；把回應的 `text` 與 `truncated` 原樣交回，不使用回應的
`revision`（見 `herdr-observer-types`）。HERDR 回錯誤碼 `pane_not_found` 時回報「pane 不存在」；其他任何錯誤
（連不上、其他錯誤碼、回應無法解析）回報帶原因的失敗，不得誤報為「pane 不存在」；讀取輸出不做 WSL 探測、
不自行重試。同一個 runtime 同一時間至多一筆
進行中的 `pane.read`，其餘排隊等候；輸出讀取不得影響進行中的事件流與 snapshot。

#### Scenario: 送出的參數

- **WHEN** 以行數上限 200 讀取 pane `w1:p1` 的輸出
- **THEN** 假 HERDR 收到 `pane.read`，params 為 `pane_id` = `w1:p1`、`source` = `recent`、`format` = `text`、
  `lines` = 200，沒有 `strip_ansi`

#### Scenario: 回應原樣交回

- **GIVEN** 假 HERDR 回應 `text` 為 `a\nb`、`truncated` 為 `true`、`revision` 為 0
- **WHEN** 讀取輸出
- **THEN** 得到文字 `a\nb`、`truncated` 為 `true`、格式為純文字

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
