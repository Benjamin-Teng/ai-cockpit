# pipeline-domain（delta）

## MODIFIED Requirements

### Requirement: 進度操作

系統必須提供五種進度操作並依下列規則接受或拒絕：「推進」在標記為 `none` 且目前 Stage 不是最後一個時
接受，把目前 Stage 改為 `stages` 中的下一個；「退回」在標記為 `none` 且目前 Stage 不是第一個時接受，把目前
Stage 改為 `stages` 中的上一個、不改標記；「標 Completed」與「標 Failed」在標記為 `none` 時接受，
分別把標記設為 `completed`、`failed`，不改目前 Stage；「清除標記」一律接受，把標記設為 `none`、不改
目前 Stage。被拒絕的操作不得改變任何進度，並回報拒絕原因（已是最後一個 Stage、已是第一個 Stage、已有標記）。

#### Scenario: 推進到下一站

- **GIVEN** `stages = ["Spec", "Plan", "Build"]`，`t1` 在 `Plan`、標記 `none`
- **WHEN** 推進
- **THEN** `t1` 在 `Build`、標記 `none`

#### Scenario: 最後一站不能推進

- **GIVEN** `t1` 在 `Build`（最後一個）
- **WHEN** 推進
- **THEN** 操作被拒絕，原因指出已是最後一個 Stage，`t1` 仍在 `Build`

#### Scenario: 退回到上一站

- **GIVEN** `stages = ["Spec", "Plan", "Build"]`，`t1` 在 `Build`、標記 `none`
- **WHEN** 退回
- **THEN** `t1` 在 `Plan`、標記 `none`

#### Scenario: 第一站不能退回

- **GIVEN** `t1` 在 `Spec`（第一個）、標記 `none`
- **WHEN** 退回
- **THEN** 操作被拒絕，原因指出已是第一個 Stage，`t1` 仍在 `Spec`

#### Scenario: 有標記時不能推進或重標

- **GIVEN** `t1` 標記為 `failed`
- **WHEN** 推進，或標 Completed
- **THEN** 兩者都被拒絕，原因指出已有標記，進度不變

#### Scenario: 有標記時不能退回

- **GIVEN** `t1` 在 `Plan`（非第一個）、標記為 `failed`
- **WHEN** 退回
- **THEN** 操作被拒絕，原因指出已有標記，`t1` 仍在 `Plan`

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
(3) 所屬 workstream 的綁定解析結果為「已綁定」、該 runtime 目前為 `connected`，且這個 Task 是該 workstream
的目前 task：綁定 pane 的 `agent_status` 為 `working` → `running`；為 `blocked` → `blocked`；其他（`idle`、
`done`、`unknown`、pane 沒有 agent）→ `ready`。(4) 其餘情況（workstream 沒有 binding、未綁定、歧義、runtime
未連線、這個 Task 不是目前 task、workstream 沒有目前 task）→ `ready`。系統不得從任何 `AgentStatus` 推得
`completed` 或 `failed`。

#### Scenario: Scenario C — 綁定 agent working 顯示 Running

- **GIVEN** Project 的 stages 含 `Implement`，task `A` 在 `Implement`、標記 `none`、無依賴，所屬
  workstream 已綁定到 runtime `win`（`connected`）的 pane `wJ:p1`，且 `A` 是該 workstream 的目前 task
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

- **GIVEN** task `B` 的 `depends_on = ["A"]`，`A` 標記 `none`；`B` 是所屬 workstream 的目前 task，該 workstream
  綁定的 pane 為 `working`
- **WHEN** 推導
- **THEN** `B` 為 `pending`；把 `A` 標 Completed 後 `B` 變成 `running`

#### Scenario: 標記優先於依賴與 agent

- **GIVEN** `B` 依賴未完成的 `A`，`B` 綁定的 pane 為 `working`
- **WHEN** `B` 被標 Failed
- **THEN** `B` 為 `failed`

#### Scenario: runtime 斷線不沿用舊狀態

- **GIVEN** `A` 是目前 task，綁定的 pane 最後已知為 `working`
- **WHEN** 該 runtime 變成 `disconnected`
- **THEN** `A` 為 `ready`

#### Scenario: Scenario D — 平行 workstream 各自呈現

- **GIVEN** workstream `backend`、`frontend`、`tests` 分別綁定三個不同 pane，task `be1` 在 `Implement`、
  `fe1` 在 `Plan`、`qa1` 在 `Test`，皆無標記與依賴，且分別是各自 workstream 的目前 task
- **WHEN** 三個 pane 同時為 `working`
- **THEN** 三個 task 皆為 `running`，且各自的目前 Stage 分別為 `Implement`、`Plan`、`Test`

#### Scenario: 同一 workstream 多個 Task

- **GIVEN** workstream `backend` 有 task `x`、`z`（標記皆 `none`、無依賴）與 `y`（標記 `completed`），目前 task 為 `x`，
  綁定 pane 為 `working`
- **WHEN** 推導
- **THEN** `x` 為 `running`、`z` 為 `ready`、`y` 為 `completed`

#### Scenario: 沒有目前 task 時不猜

- **GIVEN** workstream `backend` 有 task `x`、`z`（標記皆 `none`、無依賴），沒有目前 task，綁定 pane 為 `working`
- **WHEN** 推導
- **THEN** `x`、`z` 皆為 `ready`

## ADDED Requirements

### Requirement: 目前 task

系統必須為每條 workstream 維護至多一個「目前 task」，表示綁定到它的 agent 正在做哪個 task。目前 task 只能是同一
workstream、標記為 `none` 的 task。設定（由 agent 宣告或 agent 推進）時：task 不屬於該 workstream 或標記不是 `none`
則拒絕，原因指出不屬於或已有標記，狀態不變；否則取代原本的目前 task。下列任一情況發生時，系統必須清除該 workstream
的目前 task：目前 task 被標 Completed 或標 Failed；該 workstream 的覆蓋被設定為與原本不同的值、被取消（原本有覆蓋）
或因失效被移除（含載入狀態檔時因覆蓋無效而被忽略）。推進、退回、清除標記、重設相同的覆蓋、取消不存在的覆蓋都不影響
目前 task。

#### Scenario: 宣告取代原本的目前 task

- **GIVEN** workstream `be` 的目前 task 為 `t1`，`t2` 同屬 `be`、標記 `none`
- **WHEN** 把目前 task 設為 `t2`
- **THEN** `be` 的目前 task 為 `t2`

#### Scenario: 標完成後清除

- **GIVEN** `be` 的目前 task 為 `t1`
- **WHEN** `t1` 被標 Completed
- **THEN** `be` 沒有目前 task；之後清除 `t1` 的標記，`be` 仍沒有目前 task

#### Scenario: 改綁後清除

- **GIVEN** `be` 的目前 task 為 `t1`
- **WHEN** 為 `be` 設定覆蓋
- **THEN** `be` 沒有目前 task

#### Scenario: 重設相同覆蓋不清除

- **GIVEN** `be` 的覆蓋為 `win`／`wJ:p2`，目前 task 為 `t1`
- **WHEN** 再為 `be` 設定同一個覆蓋 `win`／`wJ:p2`
- **THEN** `be` 的目前 task 仍為 `t1`，狀態檔不重寫

#### Scenario: 退回不清除

- **GIVEN** `be` 的目前 task 為 `t1`，`t1` 在 `Build`
- **WHEN** 退回 `t1`
- **THEN** `t1` 在上一站，`be` 的目前 task 仍為 `t1`
