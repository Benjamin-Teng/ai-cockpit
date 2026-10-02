# ui-fixes 真機驗收（task 5.1）

> 日期：2026-10-01　|　對象：Windows 端 HERDR 真實 pane、真的 `target/debug/cockpit.exe`。
> 性質：一次性真機驗收紀錄；環境事實與觀察值不是規格。對 HERDR 全程唯讀，未執行 `herdr server stop`；
> 「通道斷線」是停掉 cockpit 自己的行程，HERDR 沒有被動過。

## 結論

三項畫面修補在真機上都成立，沒有發現與 spec 不符之處；有三處因真機條件而**未能驗證**，見下節。

| 項目 | 結果 |
|---|---|
| ① 滑鼠點 pane 列、task 按鈕後 `:focus-visible` 為 false，Tab 後為 true | 通過（點擊當下與其後 1 秒背景重畫後都為 false；Tab 後為 true，600 ms 後仍為 true） |
| ② 停掉 cockpit 行程：右欄 runtime 卡標「最後已知」、pane 列與 task 節點計算顏色等於 `--text-dim`；重啟（同 port）後自動重連並還原 | 通過（頁面沒有重新載入，`data-channel-state` 自行回到 `connected`） |
| ③ Git 頁在本 repo 搜尋 `fix`、Enter 跳第一筆、捲到底後 `scrollTop` 不變 | 通過，但**未能觸發第二批載入**（見下） |

### 未能驗證

1. **「無法觸發第二批」**：Git Graph 每批 200 列，本 repo 目前只有 74 個 commit（`git rev-list --all --count`
   為 74），第一批就載完，捲到底後列數維持 74、「載入更多」按鈕不顯示。③ 量到的 `scrollTop` 不變（1629 到 1629）
   只證明「捲到底後搜尋狀態沒有把清單拉回」，**不是**「第二批載入後 `scrollTop` 不變」。後者由
   `docs/research/2026-09-28/git-check.js` 的 `git-review/搜尋命中後背景載入不拉動捲動` 段在 264 個 commit 的
   fixture 上覆蓋，沒有在真機驗證。
2. **pane 列與 agent 狀態的「轉暗」在真機上沒有對比**：四個 pane 的 `agent_status` 整段驗收都是 `idle`，
   而 `idle` 的狀態色本來就是 `--text-dim`，所以連線中與斷線中的計算顏色相同（都是 `rgb(163, 183, 201)`）。
   斷線中等於 `--text-dim` 的斷言為真，但區分不出「有轉暗」與「本來就暗」。有對比的證據只有 task 節點
   （連線中 `ready` 色條與狀態文字 3 與 9 個元素不等於 `--text-dim`，斷線中全等於）、runtime 卡連線狀態與符號
   （連線中為成功色，斷線中等於 `--text-dim`）、左欄 project 計數。running／blocked／failed 的 pane 與 task
   外框、running 柔光、列首「未宣告 task」提示在真機畫面上都沒有出現，這些由 `ui-fixes-check.js` D1／D2
   在 `ui_preview` 上覆蓋。
3. **只有一個 runtime**：臨時設定只設一個 `win`，所以「多張 runtime 卡」沒有在真機上出現（驗收腳本原本斷言至少兩張，
   在這份暫存副本改成至少一張）。

## 環境

- HERDR：`herdr 0.9.2-preview.2026-09-29-8e78f929d8f0`（client）；runtime 卡讀到 server
  `0.9.0-preview.2026-09-08-62431dbd033b`、protocol 22。
- Windows 端四個 workspace／pane：`ai-cockpit`（`wW:p1`，本 shell 所在）、`repo-a`（`wX:p1`）、
  `repo-b`（`wY:p1`）、`repo-c`（`wZ:p1`），`agent` 皆為 `claude`、狀態皆 `idle`。
- cockpit：分支 `feat/ui-fixes`、commit `aed51fe`，`cargo build -p cockpit` 後直接執行
  `target\debug\cockpit.exe --config <臨時設定檔>`，監聽 `127.0.0.1:7792`（7770 無人使用，7778 有別的程序占用，
  故改用 7792；驗收前確認無 LISTEN）。
- 瀏覽器：Chrome 154.0.8037.58，`--headless=new`，viewport 1536 x 1000；以 raw CDP 的
  `Input.dispatchMouseEvent`／`Input.dispatchKeyEvent` 產生真實滑鼠與鍵盤輸入（寫法比照
  `docs/research/2026-10-01/ui-fixes-check.js`，不用 `el.click()`／`el.focus()`），Node v22.19.0。
- 搜尋框的文字用 `input.value` 加 `input` 事件填入（同 `git-check.js`），Enter 與所有點擊都是真實輸入。

## 臨時設定檔（放在 `%TEMP%\cockpit-ui-fixes-live\`，驗收後刪除）

```toml
[server]
listen = "127.0.0.1:7792"

[[runtime]]
id = "win"
kind = "herdr"

[[project]]
id = "live"
name = "Live"
stages = ["Plan", "Build", "Review"]

[[project.workstream]]
id = "mine"
binding = { runtime = "win", workspace = "ai-cockpit", agent = "claude" }

[[project.workstream]]
id = "other"
binding = { runtime = "win", workspace = "repo-a", agent = "claude" }

[[project.task]]
id = "t1"
title = "第一個"
workstream = "mine"
stage = "Plan"

[[project.task]]
id = "t2"
title = "第二個"
workstream = "mine"
stage = "Plan"

[[project.task]]
id = "o1"
title = "別人的"
workstream = "other"
stage = "Plan"

[state]
path = "state.json"
```

`mine` 綁到 `wW:p1`、`other` 綁到 `wX:p1`；Git 頁用的根目錄由 `wW:p1` 的 cwd（`D:\projects\ai-cockpit`，本 repo）
決定，`/api/runtimes/win/panes/wW:p1/root` 回 `is_git: true`。runtime 卡的 endpoint 欄位會顯示
`C:\Users\<user>\AppData\Roaming\herdr\herdr.sock`。

## 去識別化

runtime 卡的 endpoint 欄位含真實 Windows 使用者名稱（遮罩前頁面可見文字確實含使用者名稱，不含主機名稱）。
截圖前在頁面裝一個 `MutationObserver`，把文字節點中的使用者名稱與主機名稱換成 `<user>`／`<host>`（整頁重畫後仍生效），
再截圖；截圖時頁面可見文字已確認不含兩者，截圖中 endpoint 顯示為 `C:\Users\<user>\AppData\Roaming\herdr\herdr.sock`。
本紀錄的路徑同樣寫成 `C:\Users\<user>\...`。遮罩只改文字內容，顏色量測不受影響。
三張截圖另已遮蓋私人 repo 名稱（2026-10-02，change deidentify）：runtime 卡的 workspace 標題（含緊接其後的 `#n` 標號，避免由標題長度推測名稱）與 cwd 列以面板底色填滿。

## ① 焦點外框

每段開頭都重新載入頁面（「第一次點擊」要在沒有任何輸入記錄的新頁面上量）。

| 動作 | 點擊當下 | 1 秒後（背景重畫之後） |
|---|---|---|
| 滑鼠點 pane 列 `wW:p1` | 焦點在新 pane 列（`DIV.pane-row`，身分相同），`:focus-visible` = false | 同上，仍為 false |
| 滑鼠點 task「推進」按鈕（`t1`） | 焦點在新按鈕（`BUTTON.action-button`，身分相同），`:focus-visible` = false | 同上，仍為 false |

接著按 Tab：

- 點 pane 列後 Tab：焦點移到下一個 pane 列（`wX:p1`），`:focus-visible` = true；600 ms 的背景重畫後仍為 true。
- 點 task 按鈕後 Tab：焦點移到同一 task 的「Completed」按鈕，`:focus-visible` = true。

說明：點「推進」按鈕會讓 cockpit 自己的 `t1` 從 Plan 進到 Build（只改 cockpit 的狀態檔，不碰 HERDR；臨時目錄隨後刪除）。
所以截圖裡 `第一個` 在 Build 欄。

## ② 通道斷線與重連

1. 連線中基準：`data-channel-state` 為 `connected`，runtime 卡連線狀態為成功色 `rgb(57, 213, 172)`，沒有「最後已知」；
   task 節點色條與狀態文字與 `--text-dim`（`rgb(163, 183, 201)`）不同。截圖：
   `docs/research/2026-10-01/ui-fixes-live-connected.png`。
2. 依 Windows PID 終止 cockpit 行程，`:7792` 立即無 LISTEN；頁面 `data-channel-state` 變為 `disconnected`
   （頁面沒有重新載入，底列「cockpit 服務」燈號轉紅）。
3. 斷線中以 `getComputedStyle` 量到：
   - runtime 卡連線狀態（符號＋文字）、連線狀態文字節點、連線符號各 1 個，計算顏色全為 `--text-dim`；
     連線狀態可見文字為「最後已知 connected」，「最後已知」是狀態節點的手足、狀態節點 `textContent` 仍是 `connected`。
   - 卡內 agent 狀態文字 12 個、符號 12 個，全為 `--text-dim`（見「未能驗證」第 2 點，這組在連線中也是 `--text-dim`）。
   - pane 列（4 列）狀態文字與符號底色全為 `--text-dim`（同上說明）。
   - task 節點色條（左框）3 個、狀態文字與符號 9 個，全為 `--text-dim`（連線中這兩組不等於 `--text-dim`）。
   - 左欄 project 狀態計數 2 個，全為 `--text-dim`。
   - 沒有 running／failed／blocked 節點、沒有 running 柔光、沒有「未宣告 task」提示：真機上沒有這些元素，未能驗證。

   截圖：`docs/research/2026-10-01/ui-fixes-live-disconnected.png`（右欄 `win` 卡標「最後已知 connected」，
   task 節點色條為暗色，底列 `disconnected` 為紅）。
4. 以同一份設定、同一個 port 重啟 cockpit：新行程開始回應後，**未手動重新整理**，頁面的 `data-channel-state` 自行回到
   `connected`（以頁面內旗標確認是同一份 document），「最後已知」消失，runtime 卡連線狀態回到成功色，
   6 個有元素的狀態色群組（runtime 卡連線狀態兩組、連線符號、task 色條、task 狀態文字、project 計數）又不等於 `--text-dim`。
   重連後截圖：`docs/research/2026-10-01/ui-fixes-live-reconnected.png`。

## ③ Git 頁搜尋

1. 點 pane 列 `wW:p1`（cwd 是本 repo）、左欄「變更」分頁、「Git Graph」按鈕（都是真實滑鼠點擊），Git Graph 分頁開啟，
   載入 74 列，「載入更多」不顯示（`scrollHeight` 1924、`clientHeight` 295）。
2. 在搜尋框輸入 `fix`：顯示「共 37 筆」；按 Enter：顯示「第 1／共 37 筆」，標示的命中列是 HEAD 的
   「chore(ui-fixes): task 4.8 完成，4.9 待 Codex」（本來就在清單頂端，`scrollTop` 0）。
3. 捲到底：`scrollTop` 1629，等 1.5 秒後仍為 1629，列數仍為 74，計數仍為「第 1／共 37 筆」，沒有被拉回。
4. 因 74 個 commit 小於一批 200 列，**無法觸發第二批載入**（見「未能驗證」第 1 點）。

## 過程備註

- 驗收腳本是一次性暫存腳本，放在 `%TEMP%\cockpit-ui-fixes-live\`（量測段落沿用 `ui-fixes-check.js` D1 的
  `darkSnapshot`、`runtimeCardStale` 等頁面內函式），驗收後刪除，不進 repo。
- 前幾次跑時 ③ 在「Git Graph」按鈕點擊後一次沒有開出分頁（同一輪前面段落的斷言都已通過）。原因沒有深究：暫存腳本改成
  點擊後等 5 秒、沒開就重新量座標重點（最多 3 次），之後的完整一輪第 1 次點擊就開啟並全數通過。
  若日後在真機上手動點「Git Graph」偶爾沒反應，這是一個線索。
- 最終一輪共 56 項斷言通過、0 項失敗。

## 收尾

依 Windows PID 終止自己啟動的 cockpit（共兩個行程：停掉的與重啟的）與 headless Chrome，`:7792` 與 CDP 埠
`18810` 均無 LISTEN；臨時目錄（含設定檔、狀態檔、日誌、暫存腳本）已刪除；Chrome 的暫存 profile 由腳本自行清理。
