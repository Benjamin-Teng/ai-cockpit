# tasks：project-select-pane

> 執行路徑：opsx:apply｜理由：只改前端兩個檔（`actions.js`、`render.js`）的選取連動，沿用既有「看輸出」機制，不碰後端與投影；
> task 線性且少，沒有並發、安全或格式升版的風險。審查依專案 memory `opus-review-counts-as-codex-review`，整個 change 做完後由
> Opus 5.5 subagent 審一次 diff。

通則：

- 品質 gate（單獨跑、看結束碼，不接管線）：`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test --workspace`、
  `cargo test -p cockpit --example ui_preview`、`markdownlint-cli2 "**/*.md"`（N 不為 0）、`openspec validate --all`。
- 改了 `cockpit/assets/` 先 `cargo build -p cockpit --example ui_preview` 再跑腳本；腳本一次一支、前景跑；跑前 `netstat -ano` 確認 port，
  避開 7778、7679、7680；不是自己開的程序不准砍。
- 既有腳本只能改被本 change 打壞、且 spec 已改變的斷言；腳本修改與產品修改分開 commit。

## 1. 實作

- [x] 1.1 選定 project 時自動選定 pane（spec `cockpit-dashboard`「Project 切換」、`file-review`「檔案分頁」）。
  - `actions.js` 的 `select-project` 分支與「加入」後自動選定新 project 的路徑：依「bound 且 agent working → 第一條 bound → 不動」挑
    workstream，挑到時走與「看輸出」（`select-bound-pane`）相同的選定流程；頁面載入時的預設 project 不觸發。
  - `render.js`：因選定 project 而選定 pane 時，右欄該 pane 列 `scrollIntoView`（不搶焦點、不出現 `:focus-visible` 外框）。
  - `docs/research/2026-10-08/repo-projects-check.js` 補段落（先紅後綠）：working 優先、第一條 bound、全部未綁定時選定不變、加入後自動
    選定也選 pane、頁面載入不選 pane、右欄捲動、檔案分頁仍在且並排保留。
  - 依新規格調整 `files-check.js`／`split-check.js` 中「切換 Project 不影響分頁／並排」的斷言（改用沒有可選 pane 的 project 驗不變），
    另開 commit。
  - 驗收＝新斷言先紅後綠；既有 16 支腳本全綠；gate 全過。

## 2. 文件與審查

- [x] 2.1 `cockpit/README.md` 補一句「點 project 會自動選定它的 pane」；`CHANGELOG.md` 新建 `## [Unreleased]` 記這項變更。
  驗收＝markdownlint 0 error。
- [x] 2.2 Opus 審查整個 change 的 diff（`main..feat/project-select-pane`），findings 先實測重現才採信；處理結果記在本檔下方。
  驗收＝審查無未處理的 Critical／Important。

## 審查與處理紀錄

- 偏離：「檔案分頁仍在且並排保留」的斷言放在 `files-check.js`／`split-check.js`（開檔與並排的輔助函式在那兩支），未放在
  `repo-projects-check.js`。
- Opus 審查（2.2）：無 Critical。I1 窄視窗或高度不足時 `scrollIntoView` 會捲整頁 → 只在 `.runtime-cards` 為有界捲動區時於區內捲動、
  絕不捲整頁（spec 同步限縮）；M1 選到同一個 pane 時不重置 Live Output（放在選定 Project 的路徑，不改「選定 pane」本身的行為）；
  M2～M4 腳本與文件；CHANGELOG 補「改綁模式中不選 pane」。複審全部 ADDRESSED。
- 驗證：16 支既有腳本＋`repo-projects-check.js`（37 段）全綠；gate 全過。
