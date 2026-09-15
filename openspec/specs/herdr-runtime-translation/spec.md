# herdr-runtime-translation Specification

## Purpose

定義 `cockpit-herdr` 把 `herdr-client` 的 observer 型別（snapshot 與 26 種事件）翻成 Runtime 層
型別的規則。證據：設計文件 §2.3、§2.4、§7.2、§9；fixture `herdr-client/tests/fixtures/`。

## Requirements

### Requirement: snapshot 翻譯

系統必須把 HERDR `SessionSnapshot` 翻成 `RuntimeSnapshot`：`version` → `server_version`、
`protocol` 原樣；workspaces、tabs、panes、agents 逐筆對應（id 欄位原樣、`label` 空字串視為無標籤、
`agent_status` 逐值對應且 `Unknown` 維持 `Unknown`）；pane 的 `exited` 為 false（snapshot 沒有此
欄位）、`updated_at` 為翻譯時間；`focused` 取三個 `focused_*` 欄位；`layouts` 不翻譯。

#### Scenario: 兩個 protocol 版本的 fixture

- **GIVEN** `snapshot-p22.json` 與 `snapshot-p20.json`
- **WHEN** 各自翻譯
- **THEN** workspaces／tabs／panes／agents 筆數與 id 集合和 fixture 相同，`protocol` 分別為 22 與 20

#### Scenario: 未知狀態

- **GIVEN** snapshot 中某 pane 的 `agent_status` 是 HERDR 未來新增的字串
- **WHEN** 翻譯
- **THEN** 該 pane 狀態為 `Unknown`，其餘欄位正常

### Requirement: 事件翻譯對照

系統必須依設計文件 §7.2 把每個 HERDR 事件翻成零或一個 `RuntimeEvent`：`workspace_created`／
`workspace_updated`／`workspace_metadata_updated` → `WorkspaceUpserted`；`workspace_moved`／
`workspace_reordered` → `WorkspacesReplaced`；`workspace_renamed` → `WorkspaceRelabeled`；
`workspace_closed` → `WorkspaceRemoved`；`tab_created` → `TabUpserted`；`tab_renamed` →
`TabRelabeled`；`tab_closed` → `TabRemoved`；`tab_moved` → `TabsReplaced`（帶 `workspace_id`）；
`pane_created`／`pane_updated` → `PaneUpserted`；`pane_moved` → `PaneMoved`；`pane_closed` →
`PaneRemoved`；`pane_exited` → `PaneExited`；`pane_agent_detected` → `AgentDetected`（`released` 為
true 或 `agent` 為空 → 無 agent；`final_status` 不使用）；`pane.agent_status_changed` →
`AgentStatusChanged`；`workspace_focused`／`tab_focused`／`pane_focused` → 只帶該層級的
`FocusChanged`；`layout_updated`、`worktree_*` → `Noted`；生命週期版 `pane_agent_status_changed`
與 `pane_output_changed`（沒有對應的無參數訂閱、正常收不到，設計文件 §2.3）、
`pane.output_matched`／`pane.scroll_changed`（不訂閱）與未知事件名稱 → 不產生事件並記 debug；
payload 無法解析為對應型別 → `Drift`（原因含事件名）。

#### Scenario: 真機 fixture 全部可翻譯

- **GIVEN** `events-lifecycle-p22.ndjson`、`events-lifecycle-p20.ndjson`、`events-status-p22.ndjson`、
  `events-status-p20.ndjson`
- **WHEN** 逐行翻譯
- **THEN** 沒有任何一行產生 `Drift`，每行的結果種類與對照表一致

#### Scenario: pane_moved 保留新舊 id

- **GIVEN** `pane_moved` 事件 `previous_pane_id` 為 `wJ:p1`、`pane.pane_id` 為 `wK:p7`
- **WHEN** 翻譯
- **THEN** 得到 `PaneMoved`，舊 id `wJ:p1`、新 pane 的 id `wK:p7`

#### Scenario: agent 釋放

- **GIVEN** `pane_agent_detected` 的 `released` 為 true
- **WHEN** 翻譯
- **THEN** 得到 `AgentDetected` 且 agent 為空

#### Scenario: 狀態事件

- **GIVEN** `pane.agent_status_changed` 的 `agent_status` 為 `done`、`title` 為 `x`
- **WHEN** 翻譯
- **THEN** 得到 `AgentStatusChanged`，狀態 `Done`、標題 `x`，且沒有任何欄位表示「完成」

#### Scenario: payload 不足

- **GIVEN** `pane_created` 事件的 `data` 缺 `pane`
- **WHEN** 翻譯
- **THEN** 得到 `Drift`，原因含 `pane_created`

#### Scenario: 未知事件名稱

- **GIVEN** 事件名稱為 `pane_teleported`
- **WHEN** 翻譯
- **THEN** 不產生事件
