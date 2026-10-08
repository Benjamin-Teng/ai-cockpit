# repo-projects 真機冒煙（task 7.3）

> 日期：2026-10-08。對象：OpenSpec change `repo-projects` 的 task 7.3。
> 性質：接真實 Windows 端 HERDR 的一次性唯讀冒煙紀錄；可重跑的斷言在 `repo-projects-check.js`（ui_preview 假資料）。
> 路徑與 repo 名稱依去識別化規則以代稱表示：`<repo-B>`～`<repo-E>` 是使用者的私人 repo，`<dir-F>` 是一個不在 git repo 內的資料夾；
> `ai-cockpit` 是本 repo 自己。

## 環境

- Windows 11 Home 10.0.26200；Chrome 為 headless 模式（`--headless=new`、CDP、`--lang=zh-TW`，視窗 1536×1024）。
- Cockpit：分支 `feat/repo-projects`，HEAD `a5f098d` 的 debug 建置（`cargo build -p cockpit` 後執行；畫面底列 `v0.1.3`）。
- HERDR：server `0.9.2-preview.2026-09-29-8e78f929d8f0`、protocol 22（來自投影 `runtimes[0].connection`）。
- 設定：repo 外的臨時設定檔（`%TEMP%` 下的專用資料夾），`[server] listen = "127.0.0.1:7965"`，只有一筆 Windows 端 runtime
  （`id = "win"`、`kind = "herdr"`、socket 省略），`[state] path` 指到同一個臨時資料夾。啟動前 7965、7966、19655 都沒有人 LISTEN；
  使用者自己的 Cockpit（7770）當時沒有在跑。第 6 項另開第二個臨時資料夾與 `127.0.0.1:7966`。
- PowerShell：7.6.6（第 3 項主要環境）與 Windows PowerShell 5.1.26100（第 3 項交叉驗證）。
- 對 HERDR 全程唯讀：沒有開 pane、沒有送輸入、沒有執行任何 `herdr` 控制指令，只靠 Cockpit 投影讀取。
- 截圖：存在 `%TEMP%` 的臨時資料夾，目視後隨資料夾刪除，不進 repo（畫面含使用者的 pane 清單與路徑）。

## 結果總表

| 項目 | 結果 | 摘要 |
| --- | --- | --- |
| 1. 偵測到的 repo 與 pane 一致 | PASS（附限制） | 6 個 pane、5 個 repo；非 git 的 pane 正確排除；啟動後 852 ms 出現第一份非空 `detected_repos` |
| 2. 按「加入」 | PASS（附限制） | 自動選定、1 pane 1 列、四個預設 stage、無改綁鈕；真機沒有 linked worktree 的 pane，worktree 標註只驗了「不該出現時不出現」 |
| 3. README 管理端點範例 | PASS（附 3 項文件發現） | 範例可執行、回應符合文件；PowerShell 區塊缺 PATCH，且 5.1 在 UTF-8 主控台會被 BOM 擊敗 |
| 4. 改名、改 stage 後卡片位置 | PASS | 有紀錄與無紀錄的 task 都留在原 stage |
| 5. 移除後回到偵測區 | PASS | 偵測區恢復、pane 數正確、狀態檔同步清空 |
| 6. 狀態檔 | PASS | `version` 為 3、有 `repo_projects`；只開啟不操作不建檔 |
| 7. `POST /api/agent/advance` 假 pane id | PASS | 404 `no_task_for_pane`，未帶標頭回 400，沒有任何 task 被推進 |

## 1. 偵測到的 repo 與實際 pane 一致

- 取得方式：`GET /api/state` 的 `runtimes[0].workspaces[].tabs[].panes[]`（pane 樹與 cwd）對照 `detected_repos`；再用 `git -C <cwd>
  rev-parse --show-toplevel --git-common-dir` 與 `git worktree list` 人工判斷每個 cwd 是否在 git repo 內、是否為 linked worktree。
- pane 樹：6 個 workspace，各 1 個 tab、1 個 pane，agent 皆為 `claude`、狀態 `idle`。

| pane 的 cwd | git 判斷 | 是否在 `detected_repos` | `pane_count` |
| --- | --- | --- | --- |
| `ai-cockpit` 的主 worktree | repo、主 worktree | 是（`ai-cockpit`） | 1 |
| `<repo-B>` | repo、主 worktree | 是 | 1 |
| `<repo-C>` | repo、主 worktree | 是 | 1 |
| `<repo-D>` | repo、主 worktree | 是 | 1 |
| `<repo-E>` | repo、主 worktree | 是 | 1 |
| `<dir-F>` | 不是 git repo（`rev-parse` 報 not a git repository） | 否 | — |

- `detected_repos` 恰好 5 筆，`name` 為資料夾名稱，`repo` 為共同 `.git` 目錄的小寫磁碟代號路徑（`d:\...\.git`），與人工判斷完全一致：
  不在 repo 內的 pane 沒有被歸類，也沒有造成錯誤。
- **linked worktree**：`<repo-C>` 磁碟上有 4 個 linked worktree、`<repo-D>` 有 1 個，但使用者沒有任何 pane 的 cwd 落在 linked worktree 裡
  （全是主 worktree）。所以「同一 repo 的多個 worktree 只列一次」「多個 pane 歸入同一 repo、`pane_count` 大於 1」
  **無法在這台機器上用真實資料驗證**（要驗就得在使用者的 HERDR 開 pane，違反唯讀約束）。這兩點由 `repo-projects-check.js` 的 fixture 與
  `cockpit` 的 Rust 測試負責。反向事實有驗到：有 linked worktree 的 repo 仍只列一次、`pane_count` 為 1，加入後也沒有 worktree 標註（第 2、4 項）。
- **首次歸類延遲**：以 20 ms 間隔輪詢 `/api/state`，從 `Start-Process` 起算：HTTP 開始回應 594 ms、HERDR runtime 轉為 `connected` 607 ms、
  `detected_repos` 第一次非空 852 ms（5 筆），也就是 runtime 連上之後約 245 ms 歸類完成。較早的一次啟動以約 0.7 秒間隔粗略輪詢，
  第一次取樣（啟動後約 4.4 秒）就已是 5 筆，與此一致。

## 2. 在 headless 瀏覽器按「加入」

- 開啟 `http://127.0.0.1:7965/`：左欄 Project 分頁顯示「沒有 Project」，其下「偵測到的 repo」區有 5 列（`ai-cockpit`、`<repo-B>`、`<repo-C>`、
  `<repo-D>`、`<repo-E>`），每列「N 個 pane」與「加入」鈕。
- 用 CDP 真滑鼠事件按 `<repo-E>` 那列的「加入」。約 1.5 秒內：
  - 偵測區剩 4 列（`<repo-E>` 消失），左欄出現同名 Repo Project，`selected` 且是唯一被選定者，摘要 `♢ ready 1`。
  - Factory Floor 顯示該 Project：標題為 repo 名稱、四個 stage 欄 `規劃`、`實作`、`審查`、`完成`（繁中介面預設）。
  - 1 條 workstream 列（id `win~<pane>`，名稱為 agent `claude`），列首綁定摘要為 `win / <pane id>`；task 在第一個 stage `規劃`，
    `mark` 為 `none`、`status` 為 `ready`。投影中 `kind` 為 `repo`、`binding.source` 為 `pane`。
  - 列首操作鈕只有 `select-bound-pane`（「看輸出」）：**沒有「改綁」、沒有「取消改綁」**，符合固定 pane 的規格。
  - 沒有 `.ff-worktree` 節點：該 pane 在主 worktree，符合「主 worktree 不標註」。
- 限制：這個 repo 只有 1 個 pane，「每個 pane 一列」只驗了 N = 1；「linked worktree 的列有 worktree 標註」在真機沒有素材可驗（見第 1 項）。
  投影的 `worktree` 欄位對主 worktree 的 pane 為空，前端沒有畫標註，這一半有驗到。

## 3. README 管理端點範例實跑

照抄 `cockpit/README.md`「把 repo 加成 Project」一節的 PowerShell 範例，port 改成 7965，`Host` 標頭同步改成 `127.0.0.1:7965`。

| 步驟 | 指令（摘要） | 回應 | 與文件 |
| --- | --- | --- | --- |
| a. 照抄 POST（本體的 repo 是 `d:\work\app\.git`） | here-string 經標準輸入 `-d '@-'` | 404 `repo_not_detected` | 符合文件：那只是佔位值，文件寫明 repo 要用 `detected_repos[].repo` 原樣送回 |
| b. POST，repo 換成真實偵測到的值、`name` 為 `app` | 同上 | 201 `{"id":"app"}` | 符合 |
| c. PATCH 只改 `name`（單引號 JSON 直接放 `-d`，即 README bash 區塊的寫法） | `curl.exe -X PATCH ... -d '{"name":...}'` | PowerShell 7.6：204；Windows PowerShell 5.1：400 `invalid_body` | 5.1 的失敗與 README「雙引號會被吃掉」的說明一致；7.3 以後的 PowerShell 沒有這個問題 |
| d. PATCH 只改 `name`（here-string 經標準輸入） | 同 a | 204（PowerShell 7.6 與 5.1 的 cp950 主控台）；5.1 的 UTF-8 主控台：400（見發現 1） | 見發現 1 |
| e. PATCH 改 stage（重新排序、改名、新增） | 同 d | 204 | 符合；`from` 保留 task，省略表示新增 |
| f. PATCH 本體含中文（PowerShell 7.6 直接 `-d '<json>'`） | 見 c | 204，名稱與 stage 中文正確寫入投影與狀態檔 | 符合 |
| g. DELETE（README 的第二條 PowerShell 指令，id 為 `app`） | `curl.exe -i -X DELETE .../app` | 204 | 符合 |
| h. 再 DELETE 同一個 id | 同 g | 404 `unknown_project`，本體含 `params.id` | 符合文件的代碼表 |

- 投影在 204 之後有一個合併窗：204 當下立刻讀 `/api/state` 仍是舊值，約 1 秒後才是新值。文件寫「畫面等 `/ws` 推送」，一致，但用 curl 驗證時要等一下。
- README PowerShell 區塊的 `curl.exe -i` 沒加 `-s`，輸出會夾一段 curl 的進度表；不影響結果，只是雜訊。
- 狀態碼、回應本體（`{"id":"app"}`、`{"error","code","params"}`）、投影欄位都與文件相符；沒有任何一條範例因為端點行為與文件不同而失敗。
  失敗的兩種情況（5.1 的引號、UTF-8 主控台的 BOM）都是 PowerShell 端的行為，列在「發現」。

## 4. 改名、改 stage 後卡片仍在原 stage

三個 Repo Project 同時存在（`<repo-C>` 的 `app`、`<repo-D>` 的 `proj-d`、`<repo-E>` 的預設名稱），分別涵蓋兩種 task：

| Project | 修改前 | PATCH | 修改後 |
| --- | --- | --- | --- |
| `app` | stages `Plan, Build, Review, Done`；task **有紀錄**（先用 `POST .../advance` 推到 `Build`） | 重新排序成 `Review, Plan, Implement, Done`，`Build` 改名 `Implement`（`from: "Build"`），新增 `Ship`（改名在另一次 PATCH） | 仍在 `Implement`（改名後的同一個 stage） |
| `proj-d` | stages `Plan, Build, Review, Done`；task **無紀錄**，在第一個 stage `Plan` | 在最前面插入新 stage `Intake`，其餘 `from` 對回原名 | 仍在 `Plan`，沒有被新的第一個 stage `Intake` 吃走 |
| `<repo-E>` | 繁中 stages `規劃, 實作, 審查, 完成`；task **無紀錄**，在 `規劃` | 把 `審查` 排到最前、使 `規劃` 不再是第一個，並新增 `上線`；另改名（本體含中文） | 仍在 `規劃` |

- 以上都用 `GET /api/state` 與 headless 瀏覽器 DOM 雙重確認：瀏覽器的 `.ff-cell[data-stage]` 計數，卡片只在預期的 stage 欄（例如
  `Intake:0 Plan:1 Build:0 Review:0 Done:0`），左欄名稱也即時換成新名字。
- 狀態檔在 PATCH 後的 `repo_projects.<id>.tasks` 都有該 task 的紀錄（stage 與 `mark`），與投影一致。
- 沒有出現「卡片跳回第一個 stage」或「卡片消失」。

## 5. 移除後回到偵測區

- 照 README 的 DELETE 範例移除 `app`（`<repo-C>`）：204；約 1 秒內左欄少一個項目，偵測區回來 `<repo-C>`（1 個 pane），其餘 Project 不受影響。
- 再移除剩下兩個：偵測區回到 5 列，左欄顯示「沒有 Project」。狀態檔 `repo_projects` 變成空物件 `{}`，不留任何已移除 Project 的 stage 或 task 紀錄。

## 6. 臨時狀態檔

- 第一個臨時資料夾第一次加入之後的狀態檔：`"version": 3`、`"projects": {}`、`"repo_projects"` 內有該 Project 的 `name`、`repo`、`stages`、`tasks`
  （剛加入時 `tasks` 為空物件，無紀錄；推進後才出現 `{"stage","mark"}`）。
- 該資料夾在加入之前（啟動後超過 20 秒、只被 `GET /api/state` 輪詢；兩次啟動各看過一次）沒有狀態檔。
- 全新的第二個臨時資料夾（`listen = 127.0.0.1:7966`）：啟動後用 headless Chrome 開著 dashboard 約 38 秒（跨過一次 30 秒的重新抓 snapshot），
  不做任何操作，資料夾內只有設定檔與兩個日誌檔，**沒有建立狀態檔**。零設定模式的 `%LOCALAPPDATA%\ai-cockpit\` 本次沒有碰（因為都用了 `--config`）。
- 兩個 Cockpit 的 stderr 日誌沒有任何 WARN 或 ERROR。

## 7. `POST /api/agent/advance` 假 pane id

- `curl.exe -X POST http://127.0.0.1:7965/api/agent/advance -H "X-Herdr-Pane-Id: wX:p999"`（該 pane 不存在、不屬於任何 Repo Project）：
  404，本體 `{"error":"這個 pane 沒有可推進的 task","code":"no_task_for_pane"}`，與文件相符。
- 不帶標頭：400。
- 兩次請求後，所有 Project 的 task stage 與 `mark` 與請求前相同；沒有用任何真實 pane id 推進。

## 發現

### 1. README 的 PowerShell here-string 範例在 UTF-8 主控台的 Windows PowerShell 5.1 會失敗（嚴重度：低到中，文件）

- 現象：同一份腳本，Windows PowerShell 5.1 在 cp950 主控台下 POST、PATCH 都成功；在 UTF-8（cp65001）主控台下全部 400 `invalid_body`。
  那兩個情境的 `$OutputEncoding` 都顯示 US-ASCII。
- 機制：5.1 把管線內容交給原生程式時，若主控台是 UTF-8 會在字串前面多寫 3 個位元組 `EF BB BF`（BOM）；用 `curl.exe --trace` 看
  實際送出的本體，確認是 26 位元組（BOM＋23 位元組 JSON），後端的 JSON 解析不接受 BOM，回 `invalid_body`。
- 重現：在 UTF-8 主控台的 Windows PowerShell 5.1（例如從 PowerShell 7 或 Claude Code 的 PowerShell 工具內開 `powershell.exe -File`），
  執行 README 的 here-string POST；對照從 Git Bash 開同一支腳本（cp950）則成功。
- 影響：只影響照抄文件的使用者與 agent，不影響 Cockpit 本身。建議在 README 註明這個陷阱，並補一個不經標準輸入的寫法（例如先用不帶 BOM 的
  編碼把本體寫成檔案，再用 `--data-binary '@檔案'`；此替代寫法本次**未驗證**，採用前須實測）。

### 2. README 的 PowerShell 區塊沒有 PATCH 範例，且引號說明只適用部分版本（嚴重度：低，文件）

- 現象：PowerShell 區塊只有 POST 與 DELETE；PATCH 只出現在 bash 區塊。把 bash 區塊的 PATCH 原樣貼進 Windows PowerShell 5.1 得到 400
  `invalid_body`（引號被吃掉），貼進 PowerShell 7.6 則成功（204），因為 7.3 起原生程式引數預設用標準的引號跳脫
  （本次環境 `$PSNativeCommandArgumentPassing` 為 `Windows`）。
- 重現：PowerShell 5.1 執行 `curl.exe -X PATCH <url>/api/repo-projects/<id> -H 'Host: ...' -H 'Content-Type: application/json' -d '{"name":"x"}'`。
- 建議：PowerShell 區塊補一條 PATCH（here-string 寫法），並把「雙引號會被吃掉」改成「Windows PowerShell 5.1 與 7.2 以前」。

### 3. 管理端點 POST 的 `invalid_body` 錯誤文字提到 PATCH（嚴重度：低，使用者可見文字）

- 現象：對 `POST /api/repo-projects` 送壞本體，錯誤本體 `error` 為「請求本體不合法：必須是格式正確的 JSON 物件，欄位型別正確、沒有未知欄位
  （PATCH 至少要給 name 或 stages）」；POST 沒有「至少要給 name 或 stages」這條規則（`repo` 與 `stages` 才是必填），括號內的提示對 POST 是誤導。
- 重現：以 UTF-8 主控台的 5.1 做發現 1 的 POST，或任何無法解析的本體 POST 到 `/api/repo-projects`。
- 影響：錯誤代碼與狀態碼正確，只有繁中原文過度共用；英文字典 `msg.invalid_body`（`cockpit/assets/app/i18n.js`）的範本同樣含 `PATCH needs at least name or stages`，同一個代碼在 POST 與 PATCH 共用同一段文字。

### 4. 真機沒有多 pane 與 linked worktree 的 pane，兩項規格只能靠假資料驗（嚴重度：驗證範圍限制）

- 本次使用者的 HERDR 每個 repo 只有 1 個 pane，且都在主 worktree。「同一 repo 多個 worktree 只列一次」「多 pane 一列一個」
  「linked worktree 的列有 worktree 標註」沒有真機證據，只有 `repo-projects-check.js` 與 Rust 測試。若要補真機證據，需要使用者自己在 linked
  worktree 內開一個 pane（本次不能代開），再重跑第 1、2 項：預期該 repo 的 `pane_count` 變 2、仍只列一次，加入後多一列且列首有 `worktree：<資料夾名>`。

## 收尾

- 只結束本次自己開的程序：2 個 Cockpit（7965、7966）與 1 組 headless Chrome（主程序與 7 個子程序）。結束前逐一以 `Get-CimInstance` 核對
  命令列含本次臨時資料夾、建立時間與啟動時記下的一致；結束後 7965、7966、19655 都沒有 LISTEN，也沒有命令列含該資料夾的殘留程序。
- 兩個臨時資料夾（設定檔、狀態檔、日誌、截圖、Chrome 使用者資料夾）與 `%TEMP%` 下的一次性 CDP 工具腳本，確認只含本次建立的內容後刪除。
- 沒有修改 repo 內任何程式碼；使用者自己的 Cockpit、HERDR 與 `%LOCALAPPDATA%\ai-cockpit\` 都沒有被碰。
