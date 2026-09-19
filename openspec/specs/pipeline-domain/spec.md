# pipeline-domain Specification

## Purpose

定義 Cockpit 自己的 Domain 層進度模型：每個 Task 目前所在的 Stage 與人工標記、四種進度操作的合法轉移，
以及由進度、依賴與綁定 agent 狀態推導 StageStatus 的規則。證據：`docs/cockpit-spec.md` §9、§14、
`CONTEXT.md` Domain 層、設計文件 §2.4、§9。

## Requirements

### Requirement: Task 進度

系統必須為每個 Task 維護進度：目前所在的 Stage（初值為設定檔的起始 stage）與人工標記（`none`、
`completed`、`failed` 三者之一，初值 `none`）。進度只能經由進度操作改變；Runtime 層的任何狀態（含
`AgentStatus` 的 `done`、`idle`、`exited`）都不得改變進度。

#### Scenario: 初始進度

- **GIVEN** Project `p` 的 `stages = ["Spec", "Plan", "Build"]`，task `t1` 設定 `stage = "Plan"`，沒有狀態檔
- **WHEN** 系統啟動
- **THEN** `t1` 的目前 Stage 為 `Plan`、標記為 `none`

#### Scenario: agent done 不改進度

- **GIVEN** `t1` 綁定的 pane 由 `working` 變成 `done`
- **WHEN** 狀態更新
- **THEN** `t1` 的目前 Stage 與標記都不變

### Requirement: 進度操作

系統必須提供四種進度操作並依下列規則接受或拒絕：「推進」在標記為 `none` 且目前 Stage 不是最後一個時
接受，把目前 Stage 改為 `stages` 中的下一個；「標 Completed」與「標 Failed」在標記為 `none` 時接受，
分別把標記設為 `completed`、`failed`，不改目前 Stage；「清除標記」一律接受，把標記設為 `none`、不改
目前 Stage。被拒絕的操作不得改變任何進度，並回報拒絕原因（已是最後一個 Stage、已有標記）。

#### Scenario: 推進到下一站

- **GIVEN** `stages = ["Spec", "Plan", "Build"]`，`t1` 在 `Plan`、標記 `none`
- **WHEN** 推進
- **THEN** `t1` 在 `Build`、標記 `none`

#### Scenario: 最後一站不能推進

- **GIVEN** `t1` 在 `Build`（最後一個）
- **WHEN** 推進
- **THEN** 操作被拒絕，原因指出已是最後一個 Stage，`t1` 仍在 `Build`

#### Scenario: 有標記時不能推進或重標

- **GIVEN** `t1` 標記為 `failed`
- **WHEN** 推進，或標 Completed
- **THEN** 兩者都被拒絕，原因指出已有標記，進度不變

#### Scenario: 清除標記可反悔

- **GIVEN** `t1` 在 `Plan`、標記為 `completed`
- **WHEN** 清除標記
- **THEN** `t1` 在 `Plan`、標記 `none`；再清除一次同樣接受且不變

#### Scenario: 不在最後一站也能標 Completed

- **GIVEN** `t1` 在 `Spec`（非最後一個）、標記 `none`
- **WHEN** 標 Completed
- **THEN** 接受，`t1` 仍在 `Spec`、標記 `completed`

### Requirement: StageStatus 推導

系統必須依下列優先序為每個 Task 推導 StageStatus（序列化為小寫字串）：(1) 標記為 `completed` →
`completed`；標記為 `failed` → `failed`。(2) `depends_on` 中任一 task 的標記不是 `completed` → `pending`。
(3) 所屬 workstream 的綁定解析結果為「已綁定」且該 runtime 目前為 `connected`：綁定 pane 的
`agent_status` 為 `working` → `running`；為 `blocked` → `blocked`；其他（`idle`、`done`、`unknown`、
pane 沒有 agent）→ `ready`。(4) 其餘情況（workstream 沒有 binding、未綁定、歧義、runtime 未連線）→
`ready`。同一 workstream 的多個 Task 共用同一個綁定結果、各自推導。系統不得從任何 `AgentStatus` 推得
`completed` 或 `failed`。

#### Scenario: Scenario C — 綁定 agent working 顯示 Running

- **GIVEN** Project 的 stages 含 `Implement`，task `A` 在 `Implement`、標記 `none`、無依賴，所屬
  workstream 已綁定到 runtime `win`（`connected`）的 pane `wJ:p1`
- **WHEN** `wJ:p1` 的 `agent_status` 變成 `working`
- **THEN** `A` 的 StageStatus 為 `running`，目前 Stage 仍為 `Implement`

#### Scenario: 綁定 agent blocked

- **GIVEN** 同上
- **WHEN** `wJ:p1` 的 `agent_status` 變成 `blocked`
- **THEN** `A` 的 StageStatus 為 `blocked`

#### Scenario: done 不是 Completed

- **GIVEN** 同上
- **WHEN** `wJ:p1` 的 `agent_status` 變成 `done`
- **THEN** `A` 的 StageStatus 為 `ready`，不是 `completed`

#### Scenario: 依賴未完成為 Pending

- **GIVEN** task `B` 的 `depends_on = ["A"]`，`A` 標記 `none`；`B` 所屬 workstream 綁定的 pane 為 `working`
- **WHEN** 推導
- **THEN** `B` 為 `pending`；把 `A` 標 Completed 後 `B` 變成 `running`

#### Scenario: 標記優先於依賴與 agent

- **GIVEN** `B` 依賴未完成的 `A`，`B` 綁定的 pane 為 `working`
- **WHEN** `B` 被標 Failed
- **THEN** `B` 為 `failed`

#### Scenario: runtime 斷線不沿用舊狀態

- **GIVEN** `A` 綁定的 pane 最後已知為 `working`
- **WHEN** 該 runtime 變成 `disconnected`
- **THEN** `A` 為 `ready`

#### Scenario: Scenario D — 平行 workstream 各自呈現

- **GIVEN** workstream `backend`、`frontend`、`tests` 分別綁定三個不同 pane，task `be1` 在 `Implement`、
  `fe1` 在 `Plan`、`qa1` 在 `Test`，皆無標記與依賴
- **WHEN** 三個 pane 同時為 `working`
- **THEN** 三個 task 皆為 `running`，且各自的目前 Stage 分別為 `Implement`、`Plan`、`Test`

#### Scenario: 同一 workstream 多個 Task

- **GIVEN** workstream `backend` 有 task `x`（標記 `none`）與 `y`（標記 `completed`），綁定 pane 為 `working`
- **WHEN** 推導
- **THEN** `x` 為 `running`、`y` 為 `completed`
