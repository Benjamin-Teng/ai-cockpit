# pipeline-domain（delta）

## MODIFIED Requirements

### Requirement: 目前 task

系統必須為每條 workstream 維護至多一個「目前 task」，表示綁定到它的 agent 正在做哪個 task。目前 task 只能是同一
workstream、標記為 `none` 的 task。設定（由 agent 宣告或 agent 推進）時：task 不屬於該 workstream 或標記不是 `none`
則拒絕，原因指出不屬於或已有標記，狀態不變；否則取代原本的目前 task。下列任一情況發生時，系統必須清除該 workstream
的目前 task：目前 task 被標 Completed 或標 Failed；該 workstream 的覆蓋被設定為與原本不同的值、被取消（原本有覆蓋）
或因失效被移除（含載入狀態檔時因覆蓋無效而被忽略）。推進、退回、清除標記、重設相同的覆蓋、取消不存在的覆蓋都不影響
目前 task。Repo Project 的 workstream 只有一張 task，且固定綁定到 pane（沒有覆蓋）：該 task 標記為 `none` 時就是目前 task（不需要宣告，也不保存），標記為 `completed` 或 `failed` 時沒有目前 task，清除標記後又成為目前 task；其餘（宣告被拒絕的條件、推進與退回不影響）與上述相同。手寫 project 的規則不變。

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

#### Scenario: Repo Project 的 task 未標記時就是目前 task

- **GIVEN** Repo Project 的 workstream `local~wJ:p1` 只有一張 task，標記 `none`，沒有任何宣告
- **WHEN** 查詢該 workstream 的目前 task
- **THEN** 為該 task；標 Completed 後沒有目前 task，清除標記後又是該 task；推進或退回不影響

#### Scenario: 手寫 project 不因此自動有目前 task

- **GIVEN** 手寫 project 的 workstream 只有一張 task，標記 `none`，沒有宣告
- **WHEN** 查詢目前 task
- **THEN** 沒有目前 task（仍須由 agent 宣告或推進）
