# openspec-stage-sync 真機冒煙（task 7.3）

> 日期：2026-10-11（檔案放在 change 收尾批次的 `2026-10-10` 資料夾）。對象：OpenSpec change `openspec-stage-sync` 的 task 7.3。
> 性質：接真實 Windows 端 HERDR 的一次性唯讀冒煙紀錄；可重跑的斷言在同資料夾的 `stage-sync-check.js`（ui_preview 假資料）。
> 依去識別化規則，使用者其他 repo 不寫名稱，只寫「其他 repo」；路徑只寫本 repo 自己。

## 環境

- Windows 11 Home 10.0.26200；Node v22.19.0；rustc 1.97.1。
- Cockpit：分支 `feat/openspec-stage-sync`，HEAD `8248bb6` 的 debug 建置（`cargo build -p cockpit` 後直接執行
  `target\debug\cockpit.exe --config <臨時設定檔>`；畫面底列 `v0.1.5`）。
- HERDR：server `0.9.2-preview.2026-09-29-8e78f929d8f0`、protocol 22（來自投影 `runtimes[0].connection`）。
- 設定：repo 外的臨時資料夾（Claude 的 scratchpad，位於 `%TEMP%` 底下），內容只有 `[server] listen = "127.0.0.1:7967"`、
  一筆 Windows 端 runtime（`id = "win"`、`kind = "herdr"`、socket 省略）、`[state] path = "smoke.state.json"`（同資料夾）。
  沒有碰 `%LOCALAPPDATA%\ai-cockpit\` 的狀態檔。
- 啟動前用 `netstat -ano` 確認 7967 沒有 LISTEN；使用者自己的 Cockpit（7770）當時沒有在跑，系統內沒有任何 `cockpit.exe`。
- Chrome 154.0.8037.98，只用 `--headless=new --screenshot` 單次截圖（自己的 `--user-data-dir`，截完自行結束），截圖目視後隨臨時資料夾刪除。
- 對 HERDR 全程唯讀：沒有開或關 pane、沒有送輸入、沒有執行任何 `herdr` 指令，只靠 Cockpit 投影（`GET /api/state`）讀取。
- 觀測方式：一支 Node 腳本每 250 ms 讀一次 `GET /api/state`，只在 `ai-cockpit` 卡片的 `stage`、`mark`、`sync` 有變化時印出，
  時間從腳本啟動起算（腳本緊接在改檔或送請求之後啟動）。

## 與 tasks.md 文字的差異

`tasks.md` 的 7.3 寫「在 repo 外的暫存 repo 改 `tasks.md` 勾選」。本次依派工指示，改為**直接暫時修改本 repo 工作區的**
`openspec/changes/openspec-stage-sync/tasks.md`，因為唯一有真 pane 的 worktree 就是本 repo 主 worktree；在 repo 外的暫存 repo
改檔不會有任何 pane 指向它（要有就得在 HERDR 開 pane，違反唯讀）。結束時以 `git restore` 還原，`git diff --exit-code` 為 0。
修改期間使用者自己的 Cockpit 沒有在跑，不會被這段暫時的勾選帶動。

## 結果總表

| 項目 | 結果 | 摘要 |
| --- | --- | --- |
| 1. 啟動與連線 | PASS | 7967 LISTEN；runtime `win` 為 `connected`；4 個 pane、偵測到 4 個 repo |
| 2. 加入本 repo | PASS | `POST /api/repo-projects` 帶預設四站與 phases，201 `{"id":"ai-cockpit"}` |
| 3. 卡片對上 change、勾選數、階段 | PASS | `sync.change` 為 `openspec-stage-sync`、17／20、`implement`、在「實作」、`auto` |
| 4a. 全部勾選 → 審查 | PASS | 約 2.1 秒後移到「審查」、20／20、`review`、仍 `auto` |
| 4b. 手動退回後停住 | PASS | 204；立即為「實作」、`manual`；之後 17 秒沒被拉回 |
| 4c. 取消一項勾選 → 恢復自動 | PASS | 約 6.2 秒後 19／20、`implement`、`auto`，停在「實作」 |
| 4d. 還原 tasks.md | PASS | `git diff` 為空；約 1.6 秒後卡片回到 17／20、`auto`、「實作」 |
| 5. WSL 不被開機 | PASS（附限制） | 啟動前與啟動後約 160 秒，`wsl.exe --list --running` 都沒有執行中的發行版 |
| 6. 收尾 | PASS | 只結束自己的 `cockpit.exe`；7967 不再 LISTEN；無殘留程序 |

## 1. 啟動與連線

- 啟動後 stderr 日誌三行 INFO：載入設定（`listen=127.0.0.1:7967 runtimes=1`）、登記 runtime `win`（named-pipe）、dashboard 已啟動。
- 第一次 `GET /api/state`：`runtimes[0].connection.state` 為 `connected`；pane 樹有 4 個 workspace，各 1 個 tab、1 個 pane，agent 都是
  `claude`。
- 其中 `w3:p1` 的 cwd 恰為本 repo 根目錄（`D:\projects\ai-cockpit`），本 repo 目前分支為 `feat/openspec-stage-sync`。另外 3 個 pane 在使用者的其他 repo。
- `detected_repos` 4 筆，本 repo 那筆 `name` 為 `ai-cockpit`、`pane_count` 為 1。

## 2. 加入本 repo

- 本體：`repo` 取自投影 `detected_repos[].repo` 原值（小寫磁碟代號、結尾 `\.git`），`stages` 為繁中預設
  `規劃、實作、審查、完成`，`phases` 為 `plan、implement、review、complete`，與前端 `actions.js` 的「加入」相同。
- 回應 201 `{"id":"ai-cockpit"}`。投影中該 Project 的 `kind` 為 `repo`，`stage_phases` 與送出的 `phases` 一致，task id 為 `win~w3:p1`。
- 備註：一開始兩次 400 `invalid_body` 與一次 404 `repo_not_detected` 是本機 shell 送出本體時反斜線被吃掉（Git Bash 的引號處理），
  不是產品問題；改成用 Node 從投影取 `repo` 原值寫檔、再經標準輸入送出後即 201。

## 3. 卡片對上 change，勾選數與階段一致

- 人工數 `tasks.md`：`- [x]` 17 項、`- [ ]` 3 項（7.2、7.3、7.4），共 20 項。
- 加入後約 2 秒讀投影，卡片 `sync` 為 `{"change":"openspec-stage-sync","phase":"implement","checked":17,"total":20,"mode":"auto"}`，
  `stage` 為「實作」、`mark` 為 `none`。
- 分支 `feat/openspec-stage-sync` 最後一段等於 change 名稱，符合 README「分支名對上 change」的第一順位。

## 4. 移動測試

| 步驟 | 動作 | 預期 | 實際（時間自動作後起算） |
| --- | --- | --- | --- |
| a | 把剩下 3 個未勾項目改成已勾 | 約 10 秒內移到「審查」、`auto` | 0.02 秒仍為「實作」17／20；2.09 秒變「審查」、20／20、`review`、`auto`；觀察到 20 秒不再變 |
| b | `POST /api/projects/ai-cockpit/tasks/win~w3%3Ap1/retreat` | `manual`、停在「實作」，15 秒內不被拉回 | 204；第一次讀取即「實作」、`manual`（偵測結果仍為 `review` 20／20）；觀察到 17 秒不變 |
| c | 把其中一項（7.4）改回未勾 | 恢復 `auto`，依 `implement` 對應停在「實作」 | 6.21 秒變為 19／20、`implement`、`auto`，`stage` 仍為「實作」；觀察到 20 秒不再變 |
| d | `git restore` 還原 `tasks.md` | `git diff` 為空，卡片回 17／20 | `git diff --exit-code` 為 0、`git status --short` 為空；1.59 秒後 17／20、`implement`、`auto`、「實作」 |

- 4b 期間用 headless Chrome 截一張圖目視：Factory Floor 的「實作」欄有該卡片，標示一行為 `openspec-stage-sync`、`20/20`、`手動`，
  符合 README「手動時標示變淡並寫手動」的描述。
- 最後的狀態檔：`"version": 4`，`repo_projects["ai-cockpit"]` 有 `phases`（四站對應），task 有 `sync`：`mode` 為 `auto`、
  `applied` 為 `openspec-stage-sync`、`implement`、17／20，與投影一致。
- Cockpit 的 stderr 日誌全程沒有 WARN 或 ERROR。

## 5. WSL 不被開機

- 啟動前：`wsl.exe --list --running --quiet` 輸出為空（去掉 NUL 後 0 位元組）；以 PowerShell 看不加 `--quiet` 的訊息為
  「沒有任何正在執行中的發行版」。所以前提成立，本項可驗。
- Cockpit 啟動後約 160 秒（跨過一次 30 秒的重新抓 snapshot 與多輪約 10 秒的偵測）再查：`--list --running --quiet` 仍為空，
  `--list --verbose` 顯示兩個發行版都是 `Stopped`。
- **限制**：本次設定只有 Windows 端 runtime，4 個 pane 的 cwd 都在 Windows 磁碟上，沒有任何 cwd 是 `\\wsl.localhost\...` 的 pane。
  所以這一項只證明「一般 Windows 情境下 Cockpit 不會去碰 WSL」，**沒有**走到「pane 的 cwd 在 WSL 而發行版停止時跳過偵測」那條防護路徑。
  要真機驗那條路徑，需要使用者自己在 Windows 端 HERDR 開一個 cwd 在 `\\wsl.localhost\...` 的 pane 並把對應 repo 加成 Repo Project，
  本次不能代開；該路徑由 `cockpit` 的 Rust 測試負責。

## 6. 收尾

- 結束前以 `Get-CimInstance` 核對 PID 的名稱為 `cockpit.exe`、命令列含本次臨時資料夾、建立時間與啟動時記下的一致，才 `Stop-Process`。
- 結束後 7967 只剩 TIME_WAIT、沒有 LISTEN；系統內沒有 `cockpit.exe`；命令列含本次臨時資料夾的程序為 0（含 headless Chrome，
  其 `--user-data-dir` 也在該資料夾）。HERDR 的兩個程序（建立時間早於本次）沒有被碰。
- 臨時資料夾（設定檔、狀態檔、日誌、截圖、Chrome 使用者資料夾）在寫完本紀錄後刪除。
- repo 內只暫時改過 `tasks.md`，已還原；沒有修改任何程式碼。

## 未驗項目

- WSL 防護的真正路徑（cwd 在 WSL、發行版停止）：見第 5 項限制。
- 「分支名對不上時退回 archive 或唯一進行中 change」的順位、`complete` 階段（archive 後）：真機只有一個以分支名對上的 change，
  而 archive 會改動本 repo 的 change 結構，不在本次範圍；由 `stage-sync-check.js` 與 Rust 測試負責。
