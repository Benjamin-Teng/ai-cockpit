# herdr-event-subscription

## Purpose

定義長連線事件訂閱的行為：一條連線送一次 `events.subscribe`、收到 `subscription_started`
後持續收事件直到連線結束；涵蓋 24 種生命週期訂閱與每 pane 的 `pane.agent_status_changed`
訂閱。證據：設計文件 §2.3、§5.2；`docs/research/2026-09-13/herdr-source-findings.txt` §6、§7。

## ADDED Requirements

### Requirement: 訂閱建立

系統必須在一條新連線上送出含訂閱清單的 `events.subscribe`，讀到 `id` 相符且 `result` 為
`subscription_started` 型別的回應後，才把事件串流交給呼叫端；收到 `error` 回應時
整個訂閱失敗並回 Remote 錯誤，不建立串流。理由：HERDR 訂閱時會逐一探測 `pane_id`，
任一探測失敗整個 request 失敗（設計文件 §2.3）。探測失敗的 `error` 回應，其 `id` 為 request id 加上
`:sub:<序號>:probe` 後綴（真機實測，見 `docs/research/2026-09-13/change-1a-spikes.md` Spike 3），
系統必須把這種帶後綴的 `id` 視為相符，仍對應 Remote 錯誤。

#### Scenario: 24 種生命週期訂閱

- **GIVEN** 假 HERDR
- **WHEN** 以 24 種無參數訂閱建立訂閱
- **THEN** 假 HERDR 收到的 `subscriptions` 陣列含 24 個元素，各元素只有 `type` 欄位且值為
  `workspace.created` 等點號命名；request 通過兩份 schema fixture 的 `request` 根 schema；
  呼叫端拿到串流

#### Scenario: 每 pane 的 agent 狀態訂閱

- **WHEN** 以 N 個 pane id 建立 `pane.agent_status_changed` 訂閱
- **THEN** 假 HERDR 收到 N 個元素，各含 `type` 為 `pane.agent_status_changed` 與對應的
  `pane_id`，不含 `agent_status` 過濾欄位

#### Scenario: pane 不存在時整個訂閱失敗

- **GIVEN** 訂閱清單中有一個 pane id 是假 HERDR 設定為探測失敗的
- **WHEN** 建立訂閱
- **THEN** 得到 Remote 錯誤（`code` 為 `pane_not_found`；回應 `id` 為 `<request id>:sub:1:probe`
  仍視為相符），沒有串流被建立

#### Scenario: 兩條訂閱連線同時存在

- **GIVEN** 一條生命週期訂閱串流已建立
- **WHEN** 另建一條每 pane 訂閱串流
- **THEN** 兩條串流各自收到自己的事件，互不影響；關閉其中一條，另一條繼續收事件

### Requirement: 事件分軌

系統必須把每一行事件（含 `event` 與 `data` 兩個欄位）解析為三者之一：生命週期事件
（`event` 為 26 種底線命名之一，例如 `pane_created`）、每 pane 事件（`event` 為
`pane.agent_status_changed`、`pane.output_matched`、`pane.scroll_changed` 之一）、未知事件
（其餘名稱，保留原始名稱）。三者都原樣保留 `data` 供上層解析；未知事件不得中斷串流
（設計文件 §9）。

#### Scenario: 生命週期事件

- **WHEN** 假 HERDR 推送 `event` 為 `pane_created` 的一行
- **THEN** 串流產生一個生命週期事件，種類為 `pane_created`，`data` 與推送內容相同

#### Scenario: 每 pane 事件

- **WHEN** 假 HERDR 推送 `event` 為 `pane.agent_status_changed` 的一行
- **THEN** 串流產生一個每 pane 事件，種類為 `pane.agent_status_changed`

#### Scenario: 未知事件名稱

- **WHEN** 假 HERDR 推送 `event` 為 `pane_teleported` 的一行，接著一筆已知事件
- **THEN** 串流先產生一個未知事件並保留名稱 `pane_teleported`，再正常產生已知事件

### Requirement: 壞行與結束

系統必須對無法解析為 JSON、或缺少 `event`／`data` 欄位的行記錄警告並跳過，不中斷串流；
連線正常 EOF 時串流結束且不回報錯誤；連線因 I/O 錯誤中斷時串流回報含原因的錯誤後結束
（設計文件 §5.2、§9「任何連線錯誤都顯示原因，不吞掉」）。

#### Scenario: 壞行跳過

- **WHEN** 假 HERDR 推送一行非 JSON 文字、一行缺少 `event` 欄位的 JSON、再一行合法事件
- **THEN** 串流只產生合法事件那一筆

#### Scenario: 正常 EOF

- **WHEN** 假 HERDR 關閉連線
- **THEN** 串流結束，且沒有錯誤項

#### Scenario: I/O 錯誤

- **WHEN** 假 HERDR 以非正常方式中斷連線（例如強制斷開 pipe instance）
- **THEN** 串流回報一個含原因文字的錯誤項，之後結束

#### Scenario: 子程序橋接下的結束

- **GIVEN** 經子程序橋接建立的訂閱串流
- **WHEN** 遠端關閉連線
- **THEN** 串流結束，且橋接子程序終止

### Requirement: 事件即時送達

系統必須在事件到達時立即交給呼叫端，不得因 transport 或子程序的緩衝而延遲；設計文件 §10.2
的驗收以「一秒內」為準。理由：ADR-0002 把 nc 的緩衝行為列為 spike 項目。

#### Scenario: 子程序橋接下的延遲

- **GIVEN** 經子程序橋接建立的訂閱串流
- **WHEN** 遠端推送一筆事件
- **THEN** 呼叫端在一秒內收到該事件
