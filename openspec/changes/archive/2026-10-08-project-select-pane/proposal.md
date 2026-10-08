# project-select-pane

## Why

使用者 2026-10-08 回報：在左欄點選 project 之後，「檔案」「變更」分頁與右欄 runtime 卡片沒有跟著換到那個 project。現況是「選定
project」與「選定 pane」兩個選取互相獨立：選 project 只換 Factory Floor，檔案樹、git 狀態、Live Output 與右欄高亮都只跟著選定的 pane。
Repo Project（change `repo-projects`）上線後，一個 project 就是一個 repo，使用者自然期待選了 project 旁邊就看這個 repo。

brainstorming 定案（使用者選 A）：選定 project 時自動選定它的一個 pane，效果等同按該工作線的「看輸出」。

## What Changes

- **選定 project 時自動選定 pane**：使用者在左欄點選或以鍵盤選定一個 project，或「加入」後畫面自動選定新 project 時，依序挑選：
  1. `binding.state` 為 `bound` 且綁定 agent 狀態為 working 的 workstream，依畫面順序取第一條；
  2. 否則第一條 `binding.state` 為 `bound` 的 workstream；
  3. 都沒有就不改變目前選定的 pane。
  挑到時等同按該 workstream 的「看輸出」：選定該 pane（檔案、變更、Live Output 跟著換，分頁區切到 Live Output）。
- **右欄捲動到選定的 pane**：因選定 project 而選定 pane 時，右欄 runtime 卡片中該 pane 列捲動到可見位置。
- **頁面載入時的預設 project 不自動選 pane**（避免一打開就被切到 Live Output）。

## 非目標

- 不新增「只換檔案與變更根目錄、不選 pane」的機制（brainstorming 方案 B，使用者未選）。
- 不改「選定 pane」本身的行為（`live-output`「選定一個 pane」：選定時分頁區切到 Live Output）。
- 不讓選定 pane 反過來切換 project。
- 不改後端、投影與任何端點；不新增 HERDR method（設計文件 §2、ADR-0001）。

## Capabilities

### New Capabilities

（無）

### Modified Capabilities

- `cockpit-dashboard`：「Project 切換」加入選定 project 時自動選定 pane 的規則與右欄捲動。
- `file-review`：「檔案分頁」中「切換 Project 不改變已打開的分頁與並排組合」改為：已打開的分頁與並排組合不變，但切到有可選 pane 的
  project 時目前分頁會因選定 pane 而切到 Live Output。

## Impact

- 前端：`cockpit/assets/app/actions.js`（`select-project` 分支、加入後自動選定的路徑）、`cockpit/assets/app/render.js`（右欄捲動）。
  不新增 i18n 字串。
- 驗收：`docs/research/2026-10-08/repo-projects-check.js` 補段落；既有 16 支腳本全綠（`files-check.js`／`split-check.js` 中「切換 Project
  不影響分頁／並排」的斷言要依新規格調整，改用沒有可選 pane 的 project 驗「不變」，另驗有可選 pane 時切到 Live Output 且分頁與並排保留）。
- 文件：`cockpit/README.md` 一句說明；`CHANGELOG.md` 的 `## [Unreleased]`。
