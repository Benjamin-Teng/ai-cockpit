# state-projection（delta）

## Purpose

定義給畫面看的整張圖 `ProjectedState`：JSON 形狀、`version` 語意、合併廣播、最近事件。
證據：設計文件 §6.3、§6.4、ADR-0004。

## ADDED Requirements

### Requirement: 投影形狀

系統必須由狀態庫以純函數產生 `ProjectedState`，JSON 形狀依設計文件 §6.4：頂層 `version`、
`generated_at`（RFC 3339）、`runtimes`（依設定檔順序）、`recent_events`；每個 runtime 含 `id`、
`kind`、`endpoint`、`connection`、`focused`、`workspaces`；`connection.state` 為 `connecting`／
`connected`／`disconnected`，`connected` 另帶 `since`、`server_version`、`protocol`、
`last_snapshot_at`、`protocol_warning`（無則 `null`），`disconnected` 另帶 `reason`、`retry_in_secs`；
workspace 巢狀 tabs 巢狀 panes，workspace 與 tab 依 `number` 遞增、pane 依進入狀態庫的先後；父層不在
狀態庫的物件不出現在投影；狀態值序列化為小寫字串（`working` 等）。

#### Scenario: 巢狀投影

- **GIVEN** 狀態庫有 runtime `win`：workspace `wJ` 含 tab `wJ:t1`，其下 pane `wJ:p1`、`wJ:p2`
- **WHEN** 產生投影
- **THEN** JSON 的 `runtimes[0].workspaces[0].tabs[0].panes` 有兩筆，欄位名與 §6.4 一致

#### Scenario: 斷線狀態帶原因

- **GIVEN** runtime `wsl` 為 `Disconnected`，原因 `r`、60 秒後重試
- **WHEN** 產生投影
- **THEN** 該 runtime 的 `connection` 為 `{"state":"disconnected","reason":"r","retry_in_secs":60}`

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
