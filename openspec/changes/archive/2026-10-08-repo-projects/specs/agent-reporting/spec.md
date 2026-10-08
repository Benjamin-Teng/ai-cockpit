# agent-reporting（delta）

## ADDED Requirements

### Requirement: 免帶 id 推進

系統必須提供 `POST /api/agent/advance`，不需要請求本體，讓 agent 不必知道 project 與 task 的 id 就能推進自己的 task。
身分判定同「pane 身分判定」（缺少 `X-Herdr-Pane-Id` 回 400，`code` 為 `missing_pane_id`；來源檢查同其他 agent 端點）。
候選 task 的取法：對每條綁定到這個 pane 的 workstream（手寫與 Repo Project 皆可，`source` 為 `auto`、`override` 或 `pane`），
若它有目前 task 就取該 task，否則若它恰有一張 task 就取那張，否則該 workstream 不提供候選。候選恰為一張時，照「agent 推進」
的規則推進（推進規則同 `pipeline-domain`「進度操作」的推進，接受時在同一次持久化中把 task 推進到下一站並設為其 workstream 的
目前 task），成功回 204，被拒絕回 409 且不改任何狀態，本體為 `{"error": "<原因>"}`；候選為零張回 404，`code` 為
`no_task_for_pane`；候選為兩張以上回 409，`code` 為 `ambiguous_task`，狀態不變；狀態檔寫入失敗回 500 且記憶體不變。寫入生效
時身分判定所依據的覆蓋事實必須仍成立，不成立時回 403，`code` 為 `pane_not_bound`。

#### Scenario: Repo Project 的 pane 免帶 id 推進

- **GIVEN** Repo Project `app` 的 workstream `local~wJ:p1` 固定綁定到 `wJ:p1`，其 task 在 `Plan`、標記 `none`
- **WHEN** `POST /api/agent/advance`，`X-Herdr-Pane-Id: wJ:p1`
- **THEN** 回 204；稍後投影中該 task 在 `Plan` 的下一站，重啟後保留

#### Scenario: 手寫 project 已宣告目前 task

- **GIVEN** 手寫 project `p` 的 workstream `be` 綁定到 `wJ:p1`，有 task `t1`、`t2`，目前 task 為 `t2`（在 `Plan`）
- **WHEN** `POST /api/agent/advance`，`X-Herdr-Pane-Id: wJ:p1`
- **THEN** 回 204；`t2` 推進到下一站，`t1` 不變

#### Scenario: 手寫 project 只有一張 task

- **GIVEN** `be` 綁定到 `wJ:p1`，沒有目前 task，只有一張 task `t1`（在 `Plan`、標記 `none`）
- **WHEN** `POST /api/agent/advance`，`X-Herdr-Pane-Id: wJ:p1`
- **THEN** 回 204；`t1` 推進到下一站，`be` 的目前 task 為 `t1`

#### Scenario: 沒有目前 task 且有多張 task

- **GIVEN** `be` 綁定到 `wJ:p1`，沒有目前 task，有 task `t1`、`t2`
- **WHEN** `POST /api/agent/advance`，`X-Herdr-Pane-Id: wJ:p1`
- **THEN** 回 404，`code` 為 `no_task_for_pane`，狀態不變

#### Scenario: 這個 pane 沒有綁定任何 workstream

- **WHEN** `POST /api/agent/advance`，`X-Herdr-Pane-Id: wX:p9`，沒有 workstream 綁定到它
- **THEN** 回 404，`code` 為 `no_task_for_pane`

#### Scenario: 兩條 workstream 綁定到同一個 pane

- **GIVEN** 手寫 project 的 workstream `be`（有目前 task `t1`）綁定到 `wJ:p1`；Repo Project `app` 的 workstream
  `local~wJ:p1` 也固定綁定到 `wJ:p1`
- **WHEN** `POST /api/agent/advance`，`X-Herdr-Pane-Id: wJ:p1`
- **THEN** 回 409，`code` 為 `ambiguous_task`，兩張 task 的進度都不變

#### Scenario: 最後一站推進被拒

- **GIVEN** 候選 task 在最後一個 stage
- **WHEN** `POST /api/agent/advance`，帶對應的 `X-Herdr-Pane-Id`
- **THEN** 回 409，本體 `error` 指出已是最後一個 Stage，task 不變

#### Scenario: 候選 task 已有標記

- **GIVEN** 綁定到 `wJ:p1` 的 workstream 恰有一張 task，標記為 `completed`，沒有目前 task
- **WHEN** `POST /api/agent/advance`，`X-Herdr-Pane-Id: wJ:p1`
- **THEN** 回 409，task 不變

#### Scenario: WSL runtime 的 Repo Project pane

- **GIVEN** Repo Project 的 workstream 固定綁定到經由 WSL 連線的 runtime `wsl` 的 pane `w1:p1`
- **WHEN** `POST /api/agent/advance`，`X-Herdr-Pane-Id: w1:p1`
- **THEN** 回 404，`code` 為 `no_task_for_pane`（WSL runtime 的 pane 不算綁定，見「pane 身分判定」）

#### Scenario: 缺標頭

- **WHEN** `POST /api/agent/advance`，沒有 `X-Herdr-Pane-Id`
- **THEN** 回 400，`code` 為 `missing_pane_id`

#### Scenario: 寫檔失敗

- **GIVEN** 狀態檔所在目錄不可寫，候選恰為一張可推進的 task
- **WHEN** `POST /api/agent/advance`
- **THEN** 回 500，task 的 stage 不變

## MODIFIED Requirements

### Requirement: pane 身分判定

系統必須要求所有 agent 端點（路徑以 `/api/agent/` 開頭）的請求帶 `X-Herdr-Pane-Id` 標頭，其值為 agent 所在 pane 的
pane id（即 HERDR 在 pane 內提供的 `HERDR_PANE_ID` 環境變數）；標頭缺少或去除前後空白後為空時回 400，本體
`{"error": "<原因>", "code": "missing_pane_id"}`。一條 workstream「綁定到這個 pane」的條件是：它在最新投影中的
`binding.state` 為 `bound`（`source` 為 `auto`、`override` 或 `pane` 皆可）、`binding.pane_id` 等於標頭值，且 `binding.runtime`
不是經由 WSL 連線的 runtime。非 WSL runtime「擁有」標頭值這個 pane id 的條件是下列任一：它目前的 pane 樹中存在 pane id
等於標頭值、且未 exited 的 pane（不論是否綁定）；或最新投影中有任一 workstream 以 `bound` 綁定到該 runtime 的這個
pane id，即使該 pane 不在它的 pane 樹中（孤兒 pane）。runtime 斷線時，它最後已知的 pane 樹照常參與判定。
若兩個以上的非 WSL runtime 都擁有這個 pane id，
則視為沒有任何 workstream 綁定到這個 pane。寫入（宣告、推進）生效時，身分判定所依據的覆蓋事實必須仍成立：
判定時該 workstream 的綁定來源為 `override`，則寫入時它的覆蓋必須仍是同一個 runtime 與 pane id；來源為 `auto`，則寫入
時它必須仍沒有覆蓋；來源為 `pane`（Repo Project 固定綁定到 pane 的工作線，沒有覆蓋）不涉及覆蓋事實，不需要此項檢查；不成立時回 403，本體 `code` 為 `pane_not_bound`，狀態不變。agent 端點與寫入端點套用同一個本機
同源來源檢查（見 `pipeline-progress`「寫入端點只接受本機同源請求」）。

#### Scenario: 缺標頭

- **WHEN** `GET /api/agent/tasks`，沒有 `X-Herdr-Pane-Id`
- **THEN** 回 400，本體 `code` 為 `missing_pane_id`

#### Scenario: WSL runtime 的 pane 不算

- **GIVEN** workstream `be` 綁定到經由 WSL 連線的 runtime `wsl` 的 pane `w1:p1`
- **WHEN** `GET /api/agent/tasks`，`X-Herdr-Pane-Id: w1:p1`
- **THEN** 回 200，`workstreams` 為空陣列

#### Scenario: 兩個 Windows runtime 撞號

- **GIVEN** 非 WSL runtime `win` 有 workstream `be` 綁定到 pane `w1:p1`；非 WSL runtime `win2` 也有一個未 exited 的 pane
  `w1:p1`，沒有綁定任何 workstream
- **WHEN** `GET /api/agent/tasks`，`X-Herdr-Pane-Id: w1:p1`
- **THEN** 回 200，`workstreams` 為空陣列

#### Scenario: 孤兒 pane 與另一個 runtime 撞號

- **GIVEN** 非 WSL runtime `win` 有 workstream `be` 綁定到 pane `w1:p1`；最新投影中 workstream `fe` 以 `bound` 綁定到
  非 WSL runtime `win2` 的 `w1:p1`，但 `win2` 的 pane 樹中沒有該 pane（孤兒 pane）
- **WHEN** `GET /api/agent/tasks`，`X-Herdr-Pane-Id: w1:p1`
- **THEN** 回 200，`workstreams` 為空陣列

#### Scenario: 斷線 runtime 最後已知的 pane 仍參與撞號

- **GIVEN** 非 WSL runtime `win` 有 workstream `be` 綁定到 pane `w1:p1`；非 WSL runtime `win2` 目前為 `disconnected`，
  它最後已知的 pane 樹中有未 exited 的 pane `w1:p1`
- **WHEN** `GET /api/agent/tasks`，`X-Herdr-Pane-Id: w1:p1`
- **THEN** 回 200，`workstreams` 為空陣列

#### Scenario: 孤兒 pane 撞號時寫入被拒

- **GIVEN** 同「孤兒 pane 與另一個 runtime 撞號」
- **WHEN** `POST /api/agent/projects/p/tasks/t1/start`（`t1` 屬於 `be`），`X-Herdr-Pane-Id: w1:p1`
- **THEN** 回 403，本體 `code` 為 `pane_not_bound`，`be` 沒有目前 task

#### Scenario: 改綁後舊 pane 的宣告被拒

- **GIVEN** workstream `be` 自動綁定到 `wJ:p1`；使用者剛把 `be` 的覆蓋設為 `wK:p2` 並收到 204，投影尚未反映改綁
- **WHEN** `POST /api/agent/projects/p/tasks/t1/start`，`X-Herdr-Pane-Id: wJ:p1`
- **THEN** 回 403，本體 `code` 為 `pane_not_bound`，`be` 沒有目前 task

#### Scenario: Repo Project 固定綁定的 pane 算綁定

- **GIVEN** Repo Project `app` 的 workstream `local~wJ:p1` 固定綁定到非 WSL runtime `local` 的 pane `wJ:p1`
- **WHEN** `GET /api/agent/tasks`，`X-Herdr-Pane-Id: wJ:p1`
- **THEN** 回 200；`workstreams` 有一筆，`project` 為 `app`、`workstream` 為 `local~wJ:p1`、`active_task` 為 `local~wJ:p1`，
  `tasks` 只有該 task

#### Scenario: 跨站請求被拒

- **WHEN** `POST /api/agent/projects/p/tasks/t1/start`，帶 `Origin: https://evil.example`
- **THEN** 回 403，狀態不變

### Requirement: 宣告目前 task

系統必須提供 `POST /api/agent/projects/<project>/tasks/<task>/start`，不需要請求本體，把該 task 設為其 workstream 的目前
task（見 `pipeline-domain`「目前 task」）。接受且持久化成功時回 204；project 或 task 不存在回 404；task 所屬 workstream
沒有綁定到這個 pane 回 403，本體 `code` 為 `pane_not_bound`；宣告被拒絕（task 已有標記）回 409，本體為
`{"error": "<原因>"}`；狀態檔寫入失敗回 500 且記憶體不變。task 屬於 Repo Project 時（標記為 `none` 的 task 本來就是其 workstream 的目前
task，見 `repo-projects`「Repo Project 每條工作線一張 task」），同樣照上述規則判定（已有標記 → 409），通過時回 204，
狀態不變、不寫檔。

#### Scenario: 宣告成功

- **GIVEN** `be` 綁定到 `wJ:p1`、`wJ:p1` 為 `working`，`t1`、`t2` 標記皆為 `none`、無依賴
- **WHEN** `POST /api/agent/projects/p/tasks/t2/start`，`X-Herdr-Pane-Id: wJ:p1`
- **THEN** 回 204；稍後投影中 `be` 的 `active_task` 為 `t2`，`t2` 為 `running`、`t1` 為 `ready`

#### Scenario: 動別人的 task

- **GIVEN** workstream `fe` 綁定到 `wJ:p2`，task `f1` 屬於 `fe`
- **WHEN** `POST /api/agent/projects/p/tasks/f1/start`，`X-Herdr-Pane-Id: wJ:p1`
- **THEN** 回 403，本體 `code` 為 `pane_not_bound`，`fe` 的目前 task 不變

#### Scenario: 已標記的 task 不能宣告

- **GIVEN** `t1` 標記為 `completed`，`be` 綁定到 `wJ:p1`
- **WHEN** `POST /api/agent/projects/p/tasks/t1/start`，`X-Herdr-Pane-Id: wJ:p1`
- **THEN** 回 409，目前 task 不變

#### Scenario: Repo Project 的 task 宣告為空操作

- **GIVEN** Repo Project `app` 的 workstream `local~wJ:p1` 固定綁定到 `wJ:p1`，其 task 標記為 `none`
- **WHEN** `POST /api/agent/projects/app/tasks/local~wJ:p1/start`（id 逐段編碼），`X-Herdr-Pane-Id: wJ:p1`
- **THEN** 回 204；投影與狀態檔都不變

#### Scenario: Repo Project 已標記的 task 不能宣告

- **GIVEN** 同上，但 task 標記為 `failed`
- **WHEN** 同上
- **THEN** 回 409，狀態不變
