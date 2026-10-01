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
| Driver（驅動器） | 對單一 Runtime 維持連線生命週期（訂閱、authoritative snapshot、逐筆套用、Drift 與定期重拿、退避）的 `cockpit-core` 元件，與 runtime 種類無關。 |

## Domain 層（Cockpit 自己的認定，change 2 起）

| 名詞 | 定義 |
|---|---|
| Project | 一個被 Cockpit 管理的工作整體，含一條 Pipeline 與若干 Workstream。 |
| Pipeline | Project 的 Stage 線性序列（MVP）；Stage 之間沒有依賴型別，依賴改為 Task 之間的 Dependency。Stage 層級 DAG 留待之後 change，模型上 `stages` 先維持 `Vec`。 |
| Stage | Pipeline 上的一站，例如 Spec、Plan、Implement、Test、Review。 |
| Dependency | Task 之間的依賴（設定檔 `depends_on`，同一 Project 內的 task id 陣列）。依賴的 task 標記未達 `completed` 時，依賴它的 task 呈現 Pending，優先序高於綁定 agent 狀態。 |
| Workstream | Project 內平行推進的一條線，例如 Backend、Frontend、Docs。**是 Cockpit 概念，不是 HERDR 的 Workspace**；兩者的對應由設定決定，MVP 為一對一（一條 Workstream 至多一個 binding，解析為至多一個 pane）。 |
| Task | 一個 Workstream 在某個 Stage 的具體工作單位；隨進度操作（推進、退回、標 Completed、標 Failed、清除標記）在 Stage 之間移動，並帶人工標記 Mark。同一 Workstream 可有多個 Task，共用同一個 RuntimeBinding 解析結果；其中只有目前 task 會因綁定 agent 的狀態變成 Running／Blocked，其餘維持 Ready。 |
| Artifact | Task 產出的東西：檔案、報告、測試結果。 |
| Mark | Task 的人工標記，三選一：`none`、`completed`、`failed`，初值 `none`，只能經由進度操作改變。Runtime 層的任何狀態（含 AgentStatus 的 `Done`）都不得改變 Mark。 |
| RuntimeBinding | Workstream 與 Runtime 層 pane 的對應，掛在 Workstream 上，同一 Workstream 內的 Task 共用同一份解析結果。至少含 RuntimeId；以穩定特徵（workspace 標籤、cwd、agent 種類）匹配，pane id 只是解析結果，不是設定中的鍵。 |
| Override | 畫面對某條 Workstream 的 RuntimeBinding 臨時改綁，指定一個 runtime 與 pane，取代自動解析。存進狀態檔、重啟保留；runtime 不是 `connected` 時保留覆蓋、呈現 `runtime_disconnected`，並非失效。只有 runtime 為 `connected` 但 pane 不存在或已 exited 才視為失效，立即回到自動解析，刪除以非同步方式落檔（design D3）。 |
| StageStatus | Cockpit 投影出的狀態：Pending、Ready、Running、Blocked、Failed、Completed。Running／Blocked 只會出現在 Workstream 的目前 task 上（綁定 agent 為 working／blocked 時）；Completed 只能來自 Cockpit 規則或人工，不可由 AgentStatus 推得。 |
| 目前 task | 一條 Workstream 至多一個，表示綁定到它的 agent 正在做哪個 Task，由 agent 呼叫 start 或 advance 宣告；只能是同 Workstream、Mark 為 `none` 的 Task。目前 task 被標 Completed／Failed、或 Workstream 的 Override 改變時清除；推進、退回、清除標記不影響它。存進狀態檔（v2 的 `active`）、重啟保留。 |
| 退回 | 進度操作之一，把 Task 的目前 Stage 改回上一站；Mark 為 `none` 且不在第一個 Stage 才接受。只有人能操作，agent 端點不提供。 |

## 投影層

| 名詞 | 定義 |
|---|---|
| ProjectedState | 給畫面看的整張圖，由 RuntimeStore（change 2 起加上 Domain）純函數產生，附遞增 version。 |
| Factory Floor | Pipeline × Workstream 的二維視覺，UI 名稱，不是型別。 |
| Live Output | 選定 pane 的輸出投影，來自 HERDR `pane.read`，不是 terminal。由頁面輪詢（每秒一次）、依 HERDR 的樣式上色的純文字（不含控制序列）、最多最近 200 行；選取只存在於單一瀏覽器頁面，不送到服務、不跨分頁共享。中欄下半部分頁化（change 5a）後，是下半部分頁區固定的第一個分頁、不可關閉。 |
| 檔案根目錄 | change 5a `file-review` 起：由選定 pane 的 `cwd` 往上找到的第一個 git repo 根目錄（`.git` 為資料夾或檔案）；找不到就以 `cwd` 本身為根目錄。`root_id` 是根目錄主機路徑的不透明編碼，不是秘密也不是授權憑證——授權來自「目前所有 pane 的根目錄」這份允許清單，每個請求當下重新驗證。 |
| 檔案分頁 | change 5a 起，Cockpit 中欄下半部分頁區裡「已開啟檔案」對應的分頁，開啟後顯示該檔案的 Markdown／PDF／HTML／純文字檢視器內容。**與 HERDR 的 Tab 是兩回事，不要混用**：HERDR 的 Tab 是某個 workspace 內的終端機分頁（id 形如 `wJ:t1`），檔案分頁是 Cockpit 自己畫面上的概念，跟任何 HERDR 物件無對應關係，只存在於瀏覽器本機（可還原）。 |
| 變更分頁 | change 5b `git-review` 起，Cockpit 左欄三個分頁之一（與「Project」「檔案」並列），顯示目前選定 pane 之檔案根目錄的 git 狀態（已暫存、變更、未追蹤等分組）。與 HERDR 的 Tab 無關，寫法同「檔案分頁」。 |
| diff 分頁 | change 5b 起，中欄下半部分頁區裡顯示單一檔案兩個版本之間差異的分頁，左右並排呈現舊版與新版。與 HERDR 的 Tab 無關，寫法同「檔案分頁」。 |
| Git Graph 分頁 | change 5b 起，中欄下半部分頁區裡顯示某個 git repo 的 commit 圖與清單的分頁，每個 repo 一個；可在其中選取 commit 展開詳情與版本比較。與 HERDR 的 Tab 無關，寫法同「檔案分頁」。 |
| 某版本檔案分頁 | change 5b 起，中欄下半部分頁區裡顯示某個 commit（或暫存區）版本之檔案內容的分頁，以既有的檔案檢視器呈現。與 HERDR 的 Tab 無關，寫法同「檔案分頁」。 |

## 禁用與改稱

- Space → Workspace。
- 「Pipeline status」不可直接等於 HERDR 的 AgentStatus。
- 「完成」在 Runtime 層沒有對應名詞；不要把 Done 翻成 Completed。
