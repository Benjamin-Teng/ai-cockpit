# runtime-binding（delta）

## Purpose

定義 Workstream 與 Runtime 層 pane 的對應：以設定檔的穩定特徵自動解析成當下的 pane（pane id 只是解析
結果）、解析結果的種類，以及畫面臨時覆蓋的生命週期。證據：設計文件 §12「Binding 不綁 pane id 為唯一鍵」、
§2.3（pane id 在移動後會變）、`CONTEXT.md` RuntimeBinding。

## ADDED Requirements

### Requirement: 以穩定特徵自動解析

系統必須在每次 Runtime 層狀態或 Domain 狀態改變時，為每條有 `binding` 的 workstream 重新解析：候選
pane 為 binding `runtime` 的狀態庫中、`exited` 為 false、所屬 workspace 的 `label` 等於 binding
`workspace` 的 pane；若給了 `pane_label`，再篩 pane `label` 相等；若給了 `agent`，再篩 pane `agent`
相等；若給了 `cwd`，再篩 pane `cwd` 以路徑片段包含該值——兩者都把 `\` 視為 `/`、忽略空片段後，設定值的
片段序列須以連續片段出現在 pane `cwd` 的片段序列中（區分大小寫）。字串比對皆為完全相等、區分大小寫。
解析不得以 pane id 作為設定中的鍵，也不得寫入任何 HERDR 狀態。

#### Scenario: pane 換 id 後重新對上

- **GIVEN** workstream `be` 的 binding 為 `{ runtime = "wsl", workspace = "ai-cockpit", pane_label = "backend" }`，
  目前解析到 pane `w1:p2`
- **WHEN** 該 pane 被移動，狀態庫中改以 id `w1:p5` 出現，`label` 仍為 `backend`
- **THEN** `be` 解析為 `w1:p5`

#### Scenario: cwd 以片段匹配

- **GIVEN** binding `cwd = "worktrees/backend"`
- **WHEN** 候選 pane 的 cwd 分別為 `/home/u/proj/worktrees/backend/src`、`D:\proj\worktrees\backend`、
  `/home/u/proj/worktrees/backend-old`
- **THEN** 前兩個通過 cwd 篩選，第三個不通過

#### Scenario: exited pane 不當候選

- **GIVEN** 唯一符合特徵的 pane `exited` 為 true
- **WHEN** 解析
- **THEN** 結果為未綁定

### Requirement: 解析結果種類

系統必須為每條 workstream 產生恰好一種解析結果：`none`（沒有 binding 也沒有覆蓋）；
`runtime_disconnected`（要使用的 runtime 目前不是 `connected`，附 `runtime`）；`bound`（附 `runtime`、
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
- **THEN** 結果為 `runtime_disconnected`，`runtime` 為 `wsl`

### Requirement: 畫面覆蓋

系統必須允許為任一 workstream（不論有無 binding）設定覆蓋，覆蓋指定一個 runtime `id` 與 pane `id`；
設定時該 runtime 必須為 `connected` 且狀態庫中有該 pane 且未 exited，否則拒絕。覆蓋存在時取代自動
解析：覆蓋的 runtime 為 `connected` 且 pane 存在且未 exited → `bound`（`source` 為 `override`）；覆蓋的
runtime 不是 `connected` → `runtime_disconnected`（保留覆蓋）；覆蓋的 runtime 為 `connected` 但 pane
不存在或已 exited → 系統必須刪除該覆蓋並持久化刪除，該 workstream 回到自動解析。覆蓋可被取消，取消後
回到自動解析。

#### Scenario: 覆蓋歧義

- **GIVEN** workstream `be` 解析為 `ambiguous`，候選 `wJ:p1`、`wJ:p2`
- **WHEN** 設定覆蓋為 `win`／`wJ:p2`
- **THEN** `be` 為 `bound`，`pane_id` 為 `wJ:p2`、`source` 為 `override`

#### Scenario: pane 消失即失效

- **GIVEN** `be` 的覆蓋指向 `win`／`wJ:p2`，`win` 為 `connected`
- **WHEN** 狀態庫移除 `wJ:p2`
- **THEN** `be` 立即回到自動解析的結果；覆蓋的刪除寫入狀態檔，寫入完成後重啟也不存在

#### Scenario: 斷線期間保留覆蓋

- **GIVEN** `be` 的覆蓋指向 `wsl`／`w1:p1`
- **WHEN** `wsl` 斷線後重新連上，snapshot 中仍有未 exited 的 `w1:p1`
- **THEN** 斷線期間 `be` 為 `runtime_disconnected`；重連後 `be` 為 `bound`，`source` 為 `override`

#### Scenario: 覆蓋指向不存在的 pane 被拒絕

- **GIVEN** runtime `win` 為 `connected`，狀態庫沒有 `wJ:p9`
- **WHEN** 設定 `be` 的覆蓋為 `win`／`wJ:p9`
- **THEN** 拒絕，`be` 的解析結果不變

#### Scenario: 取消覆蓋

- **GIVEN** `be` 有覆蓋、自動解析會得到 `unbound`
- **WHEN** 取消覆蓋
- **THEN** `be` 為 `unbound`
