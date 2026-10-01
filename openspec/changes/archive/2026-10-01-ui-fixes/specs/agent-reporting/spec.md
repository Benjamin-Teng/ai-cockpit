# agent-reporting（delta）

## MODIFIED Requirements

### Requirement: pane 身分判定

系統必須要求所有 agent 端點（路徑以 `/api/agent/` 開頭）的請求帶 `X-Herdr-Pane-Id` 標頭，其值為 agent 所在 pane 的
pane id（即 HERDR 在 pane 內提供的 `HERDR_PANE_ID` 環境變數）；標頭缺少或去除前後空白後為空時回 400，本體
`{"error": "<原因>", "code": "missing_pane_id"}`。一條 workstream「綁定到這個 pane」的條件是：它在最新投影中的
`binding.state` 為 `bound`（`source` 為 `auto` 或 `override` 皆可）、`binding.pane_id` 等於標頭值，且 `binding.runtime`
不是經由 WSL 連線的 runtime。非 WSL runtime「擁有」標頭值這個 pane id 的條件是下列任一：它目前的 pane 樹中存在 pane id
等於標頭值、且未 exited 的 pane（不論是否綁定）；或最新投影中有任一 workstream 以 `bound` 綁定到該 runtime 的這個
pane id，即使該 pane 不在它的 pane 樹中（孤兒 pane）。runtime 斷線時，它最後已知的 pane 樹照常參與判定。
若兩個以上的非 WSL runtime 都擁有這個 pane id，
則視為沒有任何 workstream 綁定到這個 pane。寫入（宣告、推進）生效時，身分判定所依據的覆蓋事實必須仍成立：
判定時該 workstream 的綁定來源為 `override`，則寫入時它的覆蓋必須仍是同一個 runtime 與 pane id；來源為 `auto`，則寫入
時它必須仍沒有覆蓋；不成立時回 403，本體 `code` 為 `pane_not_bound`，狀態不變。agent 端點與寫入端點套用同一個本機
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

#### Scenario: 跨站請求被拒

- **WHEN** `POST /api/agent/projects/p/tasks/t1/start`，帶 `Origin: https://evil.example`
- **THEN** 回 403，狀態不變
