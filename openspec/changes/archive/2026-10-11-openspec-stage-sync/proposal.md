# openspec-stage-sync

## Why

使用者 2026-10-10 提出：希望 Factory Floor 的 pipeline 自動反映 AI 目前做到哪個階段，不必每階段手動按「推進」。
現況下，Repo Project 卡片只會因人工按鈕或 agent 主動呼叫 `/api/agent/advance` 而移動，`StageStatus` 只反映
agent 是 working 或 blocked，不反映進度（`pipeline-domain`「StageStatus 推導」）。使用者的 AI 工作大多在有
`openspec/` 的 repo 裡跑，change 目錄與 `tasks.md` 勾選本身就是可讀、不需 token、不需 HERDR 寫入的進度訊號。

brainstorming 定案（使用者 2026-10-10 依序選定）：

- 分兩期：先做本 change（唯讀「看得到」），「按鈕直接對 AI 下指令」留到之後另議（見非目標）。
- 進度來源以 OpenSpec 為主，agent 回報保留為沒有 OpenSpec 時的備案。
- pane 對應 change 依分支名稱，對不上時退回「該 worktree 只有一個進行中的 change」。
- 手動操作暫時優先，OpenSpec 進度下一次變化時恢復自動。
- 每個 Stage 可設定對應的 OpenSpec 階段。

其餘細節使用者授權依設計建議定案，記於 `design.md`。

## What Changes

- **OpenSpec 進度偵測**：Cockpit 定期讀取 Repo Project 每個 pane 所在 worktree 的目前分支與
  `openspec/changes/`，判定該 pane 正在做的 change 與其 OpenSpec 階段：
  - 規劃：change 存在，`tasks.md` 不存在或沒有勾選任何項目
  - 實作：部分勾選
  - 審查：全部勾選、尚未 archive
  - 完成：已移入 `openspec/changes/archive/<日期>-<slug>`
- **卡片自動移動**：偵測結果（change、階段、勾選數）與上次套用的結果不同時，把卡片移到「對應該階段」的 Stage。
  已有 Completed／Failed 標記的卡片不自動移動。
- **手動暫時優先**：人工「推進」「退回」或 agent 回報推進後，卡片停在手動位置；偵測結果下一次改變時才恢復自動。
  每張卡片記住目前是「自動」還是「手動」，重啟後仍然有效。
- **Stage 對應 OpenSpec 階段**：Repo Project 的每個 Stage 可設定對應規劃、實作、審查、完成之一，或不對應。
  同一個階段最多對應一個 Stage。在「編輯 stage」對話框設定；新加入的 Repo Project 預設四站自動帶好對應。
  既有 Repo Project 在升級後第一次載入時，站名剛好是預設四站名（繁中或英文）的，一次性補上明確對應。
- **畫面標示**：對上 change 的卡片顯示 change 名稱、勾選數，以及目前是自動或手動。
- **狀態檔升為 `version: 4`**：新增每站階段對應，以及每張卡片的自動或手動狀態和上次套用的偵測結果。
  **BREAKING（降版）**：v0.1.5 以前的版本讀到 v4 狀態檔會拒絕啟動。
- **新增一個唯讀 git 查詢**：取 worktree 目前分支名稱，加入 `cockpit-git` 的封閉清單。

## 非目標

- **不對 AI 下任何指令**：按鈕仍只改 Cockpit 自己的狀態。「推進＝送訊息讓 AI 接受建議」「Failed＝停止 agent 重來」
  需要 HERDR 寫入 method（`agent.prompt`、`pane.send_text` 等），違反 ADR-0001 與 `herdr-request` 規格。
  對應設計文件 `docs/cockpit-spec.md` §22「Phase 2 Interaction」與 §24「Commands」，留待之後另開 change 與 ADR。
- **不自動貼 Completed／Failed 標記**，archive 也不貼。標記一律由人來貼，維持 change `progress-model`
  （2026-10-01）的使用者決定。
- **不解析 pane 輸出、不用 LLM 判斷階段**（`docs/cockpit-spec.md` §3、§17、§21）。
- **不涵蓋手寫 `[[project]]`**：手寫 project 沒有 repo 與 worktree 的對應。
- **不偵測 brainstorming 等還沒建立 change 的階段**：對不上 change 的卡片維持手動，不顯示標示。
- **不新增任何 HERDR method**（設計文件 §2、ADR-0001、ADR-0008）。

## Capabilities

### New Capabilities

- `openspec-stage-sync`：OpenSpec 進度偵測（分支對應、階段判定、輪詢與 WSL 防護）、自動移動與手動優先規則、
  卡片的自動／手動狀態。

### Modified Capabilities

- `repo-projects`：Repo Project 定義增加每站 OpenSpec 階段對應；加入與修改 stages 的本體、驗證、改名與刪站連動；
  pane 消失時一併清除同步狀態。
- `pipeline-progress`：狀態檔升為 `version: 4`，格式加入階段對應與同步狀態，載入舊版本時一次性補上預設對應。
- `state-projection`：投影的 task 增加同步資訊；Repo Project 投影增加每站階段對應。
- `cockpit-dashboard`：卡片顯示同步標示；「編輯 stage」對話框可設定每站階段；加入時預設四站帶對應。
- `git-review`：封閉的唯讀 git 查詢清單新增「目前分支」。
- `ui-language`：新增字典鍵。

## Impact

- `cockpit-core`：`domain/repo.rs`（每站階段）、`domain/state.rs`（同步狀態與自動移動轉移）、`projection.rs`、
  `PaneRepo`（新增 worktree 根目錄欄位 `root`）。
- `cockpit-git`：新增 `CurrentBranch` 查詢。
- `cockpit`：
  - `progress.rs`：狀態檔 v4。
  - `progress_service.rs`：自動同步入口，人工與 agent 入口標記手動。
  - `repo_resolver.rs`：保留 worktree 根目錄。
  - 新模組 `openspec_sync.rs`：偵測工作。
  - `app.rs`、`http.rs`：組裝與端點本體。
- 前端：`cockpit/assets/app/actions.js`、`render.js`、`i18n.js`、樣式。
- 測試與驗收：`cockpit-core/tests/`、`cockpit/tests/`、`cockpit/examples/ui_preview.rs`，`docs/research/2026-10-10/` 新驗收腳本。
- 文件：`cockpit/README.md`、`CHANGELOG.md`、`CONTEXT.md`（新詞彙）、新 ADR-0009（狀態檔 v4 與自動移動的優先權）。
