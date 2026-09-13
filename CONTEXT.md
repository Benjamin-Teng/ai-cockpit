# CONTEXT — AI Agent Cockpit 詞彙表

> 只定義名詞，不寫行為。行為看 `openspec/specs/`，架構看
> `docs/superpowers/specs/2026-09-13-cockpit-mvp-design.md`，理由看 `docs/adr/`。

## 兩層模型，不可混用

Cockpit 刻意把「HERDR 說的」與「Cockpit 自己認定的」分成兩層。Runtime 層的型別由
`cockpit-core` 定義、由接合層從 HERDR 翻譯而來；Domain 層的型別只存在於 `cockpit-core`，
**不得引用任何 HERDR 型別**。兩層唯一的接點是 `RuntimeBinding`。

## Runtime 層（HERDR 的事實，經翻譯）

| 名詞 | 定義 |
|---|---|
| Runtime | 一個被 Cockpit 連上的 agent 執行環境實例。目前只有 HERDR，且一台機器可有多個（Windows 端、WSL 端各一）。 |
| RuntimeId | 設定檔給每個 Runtime 的短名，例如 `win`、`wsl`。所有 Runtime 層 id 都要配 RuntimeId 才唯一。 |
| Workspace | HERDR 的 workspace，id 形如 `wJ`。HERDR 的 UI 與行銷文案有時稱 Space；**Cockpit 一律用 Workspace，不用 Space**。 |
| Tab | HERDR workspace 內的分頁，id 形如 `wJ:t1`。 |
| Pane | HERDR tab 內的終端機格子，id 形如 `wJ:p1`。一個 pane 可能有 agent，也可能只是 shell。 |
| Agent | HERDR 偵測到在某個 pane 裡跑的 coding agent（claude、codex 等）。 |
| AgentStatus | HERDR 給的五種狀態：Idle、Working、Blocked、Done、Unknown。Done 只表示「已 idle 且尚未被看過」，**不是任務完成**。 |
| Worktree | HERDR 的 git worktree 物件，是 HERDR 的一等公民。change 1 只記錄事件，不建模。 |
| Layout | HERDR 對 pane 幾何配置的描述（snapshot 的 `layouts` 陣列）。change 1 不建模、不顯示。 |
| RuntimeSnapshot | 某一刻某個 Runtime 的完整狀態：workspaces、tabs、panes、agents 加版本資訊。 |
| RuntimeEvent | 翻譯後的單筆變化，例如 AgentStatusChanged、PaneRemoved、Drift。 |
| ConnectionState | Cockpit 與某個 Runtime 的連線狀態：Connecting、Connected、Disconnected。 |
| Drift | 事件提到狀態庫裡不存在的物件、或 payload 不足以更新，代表狀態庫已與 HERDR 脫節，必須重拿 snapshot。 |

## Domain 層（Cockpit 自己的認定，change 2 起）

| 名詞 | 定義 |
|---|---|
| Project | 一個被 Cockpit 管理的工作整體，含一條 Pipeline 與若干 Workstream。 |
| Pipeline | 工作依賴圖（DAG），由 Stage 與 Dependency 組成。第一版視覺可線性，模型從一開始允許 DAG。 |
| Stage | Pipeline 上的一站，例如 Spec、Plan、Implement、Test、Review。 |
| Dependency | Stage 之間的先後關係。 |
| Workstream | Project 內平行推進的一條線，例如 Backend、Frontend、Docs。**是 Cockpit 概念，不是 HERDR 的 Workspace**；兩者的對應由設定決定，不強制一對一，是否允許多對多由 change 2 決定。 |
| Task | 一個 Workstream 在某個 Stage 的具體工作單位。 |
| Artifact | Task 產出的東西：檔案、報告、測試結果。 |
| RuntimeBinding | Task 與 Runtime 層物件的對應。至少含 RuntimeId；以穩定特徵（workspace 標籤、cwd、agent 種類）匹配，pane id 只是解析結果。 |
| StageStatus | Cockpit 投影出的狀態：Pending、Ready、Running、Blocked、Failed、Completed。Completed 只能來自 Cockpit 規則或人工，不可由 AgentStatus 推得。 |

## 投影層

| 名詞 | 定義 |
|---|---|
| ProjectedState | 給畫面看的整張圖，由 RuntimeStore（change 2 起加上 Domain）純函數產生，附遞增 version。 |
| Factory Floor | Pipeline × Workstream 的二維視覺，UI 名稱，不是型別。 |
| Live Output | 選定 pane 的輸出投影，來自 HERDR `pane.read`，不是 terminal。 |

## 禁用與改稱

- Space → Workspace。
- 「Pipeline status」不可直接等於 HERDR 的 AgentStatus。
- 「完成」在 Runtime 層沒有對應名詞；不要把 Done 翻成 Completed。
