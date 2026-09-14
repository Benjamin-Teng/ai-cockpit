# herdr-request

## Purpose

定義單次 request／response 往返的行為：開一條連線、送一行、讀一行、關閉，以及回應與錯誤
如何對應到呼叫端看到的結果。change 1 只使用 `session.snapshot`（ADR-0001）。
證據：設計文件 §2.2、§5.2；`docs/research/2026-09-13/herdr-schema-findings.txt` §1。

## ADDED Requirements

### Requirement: 單次往返使用獨立連線

系統必須為每個 request 開一條新連線，送出一行 `{"id","method","params"}` JSON（`id` 為
字串，同一程序內不重複），讀取一行回應後關閉連線。

#### Scenario: session.snapshot 成功

- **GIVEN** 假 HERDR 以 fixture 回應 `session.snapshot`
- **WHEN** 送出 `session.snapshot`
- **THEN** 得到解析後的 snapshot，且假 HERDR 觀察到該連線只收到一行、之後連線被關閉

#### Scenario: request 的 id 不重複

- **WHEN** 連續送出兩個 request
- **THEN** 假 HERDR 收到的兩個 `id` 不同

### Requirement: 回應對應

系統必須把 `{"id","result"}` 且 `id` 相符的回應解析為該 request 的回應型別；
`{"id","error":{"code","message"}}` 對應 Remote 錯誤並原樣保留 `code` 與 `message`；
`error` 回應的 `id` 等於 request id、或為 request id 加上以 `:` 開頭的後綴（HERDR 對訂閱探測失敗會這樣
改寫，見 `herdr-event-subscription`）時仍視為相符；成功回應的 `id` 不符、整行無法解析為 JSON、或
`result` 形狀不符對應 Protocol 錯誤；連線在讀到回應前
中斷對應連線類錯誤並附原因。

#### Scenario: 遠端錯誤

- **GIVEN** 假 HERDR 對某 request 回 `error` 物件，`code` 為 `x`、`message` 為 `y`
- **WHEN** 送出該 request
- **THEN** 得到 Remote 錯誤，`code` 為 `x`、`message` 為 `y`

#### Scenario: id 不符

- **GIVEN** 假 HERDR 回覆的 `id` 與 request 不同
- **WHEN** 送出 request
- **THEN** 得到 Protocol 錯誤

#### Scenario: 回應不是 JSON

- **GIVEN** 假 HERDR 回一行非 JSON 文字
- **WHEN** 送出 request
- **THEN** 得到 Protocol 錯誤

#### Scenario: result 形狀不符

- **GIVEN** 假 HERDR 對 `session.snapshot` 回 `result` 的 `type` 為 `pong` 的回應
- **WHEN** 送出 `session.snapshot`
- **THEN** 得到 Protocol 錯誤

#### Scenario: 回應前連線中斷

- **GIVEN** 假 HERDR 收到 request 後不回應就關閉連線
- **WHEN** 送出 request
- **THEN** 得到連線類錯誤，且原因文字非空

#### Scenario: server 未啟動

- **GIVEN** 連線目標沒有 server
- **WHEN** 送出 request
- **THEN** 得到 ServerNotRunning

### Requirement: 只提供 observer 子集的 method

系統必須只提供 `session.snapshot`（參數為空物件）與 `pane.read`（供 change 3，change 1 不對
真機呼叫）兩個 request 型別，以及 `events.subscribe`（見 `herdr-event-subscription`）；
不得提供任何會改變 HERDR 狀態的 method 型別（ADR-0001：HERDR socket API 沒有認證，
`agent.prompt`、`server.stop` 等一旦誤觸後果嚴重）。

#### Scenario: 序列化的 request 符合兩個 protocol 版本的 schema

- **GIVEN** protocol 20 與 22 兩份 schema fixture
- **WHEN** 序列化 `session.snapshot` 與 `pane.read` 的 request
- **THEN** 兩份 schema 的 `request` 根 schema 都驗證通過

#### Scenario: 沒有寫入型 method

- **WHEN** 檢視 crate 公開的 request 型別清單
- **THEN** 只有 `session.snapshot`、`pane.read`、`events.subscribe`；沒有 `agent.prompt`、
  `server.stop`、`pane.report_metadata` 等會改變狀態的 method
