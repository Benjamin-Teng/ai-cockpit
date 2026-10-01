# runtime-binding（delta）

## MODIFIED Requirements

### Requirement: 解析結果種類

系統必須為每條 workstream 產生恰好一種解析結果：`none`（沒有 binding 也沒有覆蓋）；
`runtime_disconnected`（要使用的 runtime 目前不是 `connected`，附 `runtime` 與 `source`：該 workstream 有覆蓋時
為 `override`，否則為 `auto`）；`bound`（附 `runtime`、
`pane_id`、`source` 為 `auto` 或 `override`）；`unbound`（自動解析的候選為 0 個，附 `runtime`）；
`ambiguous`（候選超過 1 個，附 `runtime` 與依狀態庫順序排列的候選 `pane_id` 清單）。runtime 未連線時
不評估候選，也不沿用上次的解析結果。

#### Scenario: 恰好一個

- **GIVEN** runtime `win` 為 `connected`，符合特徵的 pane 只有 `wJ:p1`
- **WHEN** 解析
- **THEN** 結果為 `bound`，`runtime` 為 `win`、`pane_id` 為 `wJ:p1`、`source` 為 `auto`

#### Scenario: 對到多個

- **GIVEN** binding 只有 `runtime` 與 `workspace`，該 workspace 有兩個未 exited 的 pane `wJ:p1`、`wJ:p2`
- **WHEN** 解析
- **THEN** 結果為 `ambiguous`，候選為 `["wJ:p1", "wJ:p2"]`

#### Scenario: 對不到

- **GIVEN** 沒有任何 workspace 的 label 等於 binding `workspace`
- **WHEN** 解析
- **THEN** 結果為 `unbound`

#### Scenario: runtime 斷線

- **GIVEN** 上一次解析為 `bound`，之後 runtime `wsl` 變成 `disconnected`
- **WHEN** 解析
- **THEN** 結果為 `runtime_disconnected`，`runtime` 為 `wsl`，`source` 為 `auto`

#### Scenario: 覆蓋造成的斷線帶出覆蓋來源

- **GIVEN** `be` 的覆蓋指向 `wsl`／`w1:p1`，之後 `wsl` 變成 `disconnected`
- **WHEN** 解析
- **THEN** 結果為 `runtime_disconnected`，`runtime` 為 `wsl`，`source` 為 `override`；覆蓋仍保留（見「畫面覆蓋」）
