# herdr-observer-types

## Purpose

定義 Cockpit 對 HERDR 資料形狀的解讀規則：只涵蓋觀測所需的子集，對未知欄位與未知值向前
相容，並以 protocol 20 與 22 兩份 schema 作為合約。證據：設計文件 §2.3 到 §2.8、§9；
`docs/research/2026-09-13/herdr-schema-findings.txt`、`herdr-schema-compare-output.txt`。

## ADDED Requirements

### Requirement: snapshot 解析

系統必須把 `session.snapshot` 的回應（`result` 的 `type` 為 `session_snapshot`、內容在
`snapshot`）解析為含下列欄位的結構，欄位名與 HERDR 一致：頂層 `version`、`protocol`、
`workspaces`、`tabs`、`panes`、`agents`、`focused_workspace_id`、`focused_tab_id`、
`focused_pane_id`；workspace 至少含 `workspace_id`、`label`、`number`、`active_tab_id`、
`agent_status`、`focused`；tab 至少含 `tab_id`、`workspace_id`、`number`、`label`、
`agent_status`、`focused`；pane 至少含 `pane_id`、`workspace_id`、`tab_id`、`agent`、
`agent_status`、`title`、`terminal_title`、`cwd`、`label`、`focused`、`revision`；agent 至少含
`agent`、`pane_id`、`workspace_id`、`tab_id`、`agent_status`。`layouts` 不解析為結構。
以上五個陣列是平行陣列、靠 id 字串互相參照，不是巢狀（`herdr-schema-findings.txt` §8）。

#### Scenario: 真機 snapshot fixture 解析

- **GIVEN** 從真機匯出並去識別化的 snapshot fixture（protocol 22 與 20 各一）
- **WHEN** 解析
- **THEN** 成功；workspace、tab、pane、agent 的數量與 fixture 陣列長度一致；`version` 與
  `protocol` 與 fixture 相同

#### Scenario: 未知欄位與缺少的選填欄位

- **GIVEN** 一個 pane 物件多了未知欄位 `future_field`，且缺少 `cwd`
- **WHEN** 解析
- **THEN** 成功；`cwd` 為空值；未知欄位被忽略

### Requirement: AgentStatus 五值與未知值

系統必須把 `idle`、`working`、`blocked`、`done`、`unknown` 解析為對應的五個值，任何其他
字串解析為 Unknown 而不是失敗。`done` 只代表 HERDR 的「已 idle 且尚未被看過」（設計文件
§2.4），系統不得提供任何把 AgentStatus 轉成「完成」的判斷或欄位。

#### Scenario: 已知值

- **WHEN** 解析 `done`
- **THEN** 得到 Done；型別本身沒有「是否完成」之類的判斷

#### Scenario: 未知值

- **WHEN** 解析 `meditating`
- **THEN** 得到 Unknown，解析不失敗

### Requirement: 事件 payload 型別

系統必須為 change 1b 會消費的事件提供 payload 型別，欄位名以設計文件 §2.3 的 payload 表
與真機 fixture 為準，缺少的非必填欄位為空值：

- 完整物件：`workspace_created`、`workspace_updated`、`workspace_metadata_updated` 帶
  workspace；`tab_created` 帶 tab；`pane_created`、`pane_updated` 帶 pane。
- 整份順序：`workspace_moved`、`workspace_reordered` 帶 `workspaces` 陣列；`tab_moved` 帶
  `tab_id`、`workspace_id`、`insert_index`、`tabs` 陣列。
- id 類：`workspace_closed`（`workspace_id`）、`workspace_renamed`（`workspace_id`、`label`）、
  `tab_closed`（至少 `tab_id`）、`tab_renamed`（`tab_id`、`label`）、`pane_closed`、
  `pane_exited`（`pane_id`、`workspace_id`）、`workspace_focused`、`tab_focused`、
  `pane_focused`（至少對應的 id）。
- `pane_moved`：`previous_pane_id`、`previous_workspace_id`、`previous_tab_id`、`pane`
  必有；`created_workspace`、`created_tab`、`closed_workspace_id`、`closed_tab_id` 選填
  （schema 有、§2.3 表未列）。
- `pane_agent_detected`：`pane_id`、`workspace_id`、`agent`、`released`；`final_status` 選填，
  值域同 AgentStatus（schema 有、§2.3 表未列）。
- `pane.agent_status_changed`：`pane_id`、`workspace_id`、`agent_status`、`agent`、
  `display_agent`、`title`、`state_labels`。
- `worktree_*`、`layout_updated`、`pane_output_changed`：不提供型別，上層只記事件名。

#### Scenario: 真機事件 fixture 解析

- **GIVEN** 真機擷取並去識別化的事件 fixture（含生命週期事件與 `pane.agent_status_changed`）
- **WHEN** 依每行的 `event` 名選對應型別解析 `data`
- **THEN** 全部成功，必填欄位皆有值

#### Scenario: pane_moved 保留新舊 id

- **WHEN** 解析 `previous_pane_id` 為 `wJ:p1`、新 pane 的 `pane_id` 為 `wK:p3` 的 `pane_moved`
- **THEN** 兩個 id 都保留且可以不同

#### Scenario: pane_agent_detected 的釋放

- **WHEN** 解析 `agent` 為 null、`released` 為 true 的 `pane_agent_detected`
- **THEN** `agent` 為空值、`released` 為 true，解析不失敗

### Requirement: 兩個 protocol 版本的合約

系統必須以 protocol 20（WSL 0.8.2）與 22（Windows 0.9.0-preview）兩份 schema 為合約：
所有序列化的 request 通過兩份 schema 的 `request` 根 schema；所有解析用 fixture 通過對應的
根 schema（`success_response`、`event`、`subscription_event`）。依據：observer 子集在兩個
版本是同一份合約（設計文件 §2.8、`herdr-schema-compare-output.txt`）。

#### Scenario: 合約測試

- **GIVEN** 兩份 schema fixture
- **WHEN** 執行合約測試
- **THEN** `session.snapshot`、`pane.read`、含 24 種生命週期與 N 筆每 pane 訂閱的
  `events.subscribe` 三種 request，以及所有 snapshot 與事件 fixture，在兩個版本下都通過驗證

### Requirement: pane.read 型別（供 change 3）

系統必須提供 `pane.read` 的參數（`pane_id`、`source`、`format`、`lines`、`strip_ansi`）與
結果（`pane_id`、`workspace_id`、`tab_id`、`source`、`format`、`text`、`revision`、
`truncated`）型別，`source` 值域為 `visible`、`recent`、`recent_unwrapped`、`detection`
（設計文件 §2.6），`format` 值域為 `text`、`ansi`（schema `ReadFormat`），兩者都不得序列化出值域外的
字串。change 1 不對真機呼叫。

#### Scenario: 序列化 pane.read request

- **WHEN** 序列化 `source` 為 `recent`、`lines` 為 200 的 `pane.read` request
- **THEN** 通過兩份 schema 的 `request` 根 schema
