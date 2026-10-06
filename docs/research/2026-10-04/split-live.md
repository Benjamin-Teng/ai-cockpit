# file-split-view 真機驗收（task 5.3）

> 日期：2026-10-06。對象：OpenSpec change `file-split-view` 的 task 5.3。
> 性質：接真實 Windows 端 HERDR、讀真實檔案的一次性驗收紀錄；可重跑的斷言在 `split-check.js`（ui_preview 假資料）。

## 環境

- Cockpit：分支 `feat/file-split-view` 的 debug 建置（HEAD `96d1dc0`；最後一個動到 `cockpit/` 的 commit 是 `cf8c90b`，
  執行檔建置時間晚於它），畫面底列版本 `v0.1.2`。
- 設定：放在 repo 外的臨時設定檔，listen `127.0.0.1:7781`，只有一個 Windows 端 HERDR runtime（id `win`）。
- HERDR：已連上（`/api/state` 的 connection.state 為 `connected`）。
- 瀏覽器：headless Chrome＋raw CDP，`Emulation.setDeviceMetricsOverride` 設 2560×1440、deviceScaleFactor 1。
- 探針：從 `split-check.js` 複製工具函式到 repo 外改寫的一次性腳本，用完不保存。repo 內的腳本沒有修改。
- 讀取對象：cwd 為 `<repo>`（使用者主工作區）的 pane；檔案根目錄即 `<repo>`。
- 截圖：已目視確認、未保存（真機畫面含其他工作內容，依專案規矩看過即刪）。

## 結果總表

| 項目 | 結果 | 數字 |
| --- | --- | --- |
| 1. 讀取與並排 | PASS | 兩欄 992／992 px、三欄 658.66／658.67／658.67 px（差 0.02 px）；頁面無橫向捲軸 |
| 2. 非焦點欄自動更新 | PASS | 檔尾加一行後 1760 ms 出現在非焦點欄；git status 前後皆為空 |
| 3. 重新整理還原 | PASS（spec 範圍）；附一項觀察 | 並排組合與焦點欄還原；各欄捲動位置回到 0（見下） |
| 4. 切走再切回 | PASS | 三欄捲動位置前後完全相同（差 0 px），沒有重新讀取內容 |
| 5. 輪詢只查看得到的分頁 | PASS | 10 秒內三個可見分頁各 5 次，不可見分頁 0 次 |
| 6. 窄視窗 | PASS | 700 寬只顯示焦點欄、6 秒內只查它（3 次）；改回 2560 恢復三欄 |

探針共 340 條斷言，0 條失敗；頁面沒有未捕捉例外。

## 1. 讀取與並排

- 打開 `AGENTS.md`、`README.md`、`cockpit/README.md`、`docs/handover.md` 四個檔案分頁（`AGENTS.md` 只打開、不並排，供第 5 項用）。
- 選 `README.md` 後按 `cockpit/README.md` 的並排鈕 → 兩欄；再按 `docs/handover.md` 的並排鈕 → 三欄。
- 每一步都跑 `split-check.js` 的 `expectSplit()`／`checkLayout()` 同一組斷言：欄位順序、欄位編號、aria-pressed、
  只有焦點欄帶 `data-split-focus` 與 accent 外框、各欄同一列不重疊、各欄寬加間距等於中欄寬、欄內容不溢出欄寬。
- 量測（中欄 `#review` 寬 1992 px、欄間距 8 px）：

| 欄數 | 各欄寬（px） | 寬差 | 頁面 scrollWidth／clientWidth |
| --- | --- | --- | --- |
| 2 | 992、992 | 0 | 2560／2560 |
| 3 | 658.66、658.67、658.67 | 0.02 | 2560／2560 |

- 內容渲染：每欄的 `.md-body` 都有內容與標題（`README.md` 8 個、`cockpit/README.md` 28 個、`docs/handover.md` 8 個 h1–h3）。
- 閱讀版面：
  - `.md-body` 的計算後 max-width 為 603.75 px，等於同字型 80 個字元的寬度（603.8 px），兩欄與三欄都達到這個上限。
    三欄時檢視器內寬 623 px，仍大於 80ch，所以不會壓窄行寬。
  - 工具列高 20 px、可見；路徑欄兩欄時 737 px、三欄時 404 px，都沒有被截斷（scrollWidth 不大於 clientWidth）。
- 目視：三欄等寬、焦點欄外框清楚；兩欄時 80ch 上限使每欄右側留白，屬設計行為。工具列的路徑、讀取時間、
  「在 VS Code 開啟」鈕在三欄時都完整顯示。

## 2. 非焦點欄自動更新

- 測試前 `git -C <repo> status --porcelain` 輸出為空。
- 在 `<repo>` 根目錄建立 `zz-split-live-probe-<8 碼>.md`（內容一行）。檔案樹在重新選取 pane、切到「檔案」時就列出它，
  不需要按「重新整理」。
- 依序移出 `docs/handover.md`、`cockpit/README.md` 解除並排；選探針檔後按 `README.md` 的並排鈕 → 探針檔第 1 欄（非焦點）、
  `README.md` 第 2 欄（焦點）。
- 在探針檔末端加一行帶唯一標記的文字 → 1760 ms 後第 1 欄出現該行，焦點欄仍為 `README.md`。
- 測完立即刪除探針檔；`git status --porcelain` 輸出仍為空，與測試前相同。只建立、修改、刪除了這一個新檔。

## 3. 重新整理還原

- 前置：三欄並排，焦點欄改為 `cockpit/README.md`；三欄分別捲到 1149、7795、4484 px。
- 寫出的紀錄：`v: 2`，tabs 依序為四個檔案分頁，`split` 為 `[1, 2, 3]`，`splitFocus` 為 `2`，current 為 `cockpit/README.md`。
- 重新整理後：四個分頁依原順序還原，三欄依原順序並排、焦點欄為 `cockpit/README.md`，三欄內容都畫好，沒有並排相關的警告，
  版面數字與重新整理前相同。
- 觀察（不判為 FAIL）：三欄的捲動位置都回到 0。spec「分頁還原」列舉的保存狀態是分頁、目前分頁、並排組合、焦點欄與左欄
  目前分頁，不含捲動位置；捲動位置的保留只規定在「離開並排再回來」與整頁重畫（第 4 項）。若要求重新整理也保留捲動位置，
  需要另開 change 擴充儲存格式。
- 另：重新整理後 Live Output 的 pane 選取不還原，檔案樹顯示「先在 Factory Floor 或 runtime 清單選一個 pane」，
  符合 spec「Live Output 的 pane 選取不還原」；已打開的檔案分頁不受影響，照常讀取。

## 4. 切走再切回

- 三欄捲到 1341、8574、4805 px，並在各欄內容節點貼記號。
- 選 Live Output → 只顯示 Live Output，並排組合不變；1 秒後選回 `docs/handover.md` → 三欄恢復，`docs/handover.md` 為焦點欄。
- 三欄捲動位置與離開前完全相同，內容節點的記號都在，期間沒有對三個檔案發出 render／raw 請求。

## 5. 輪詢只查看得到的分頁

- 三欄並排、`AGENTS.md` 已打開但不可見，以 CDP Network 觀察 10.012 秒：
  - 中繼資料查詢共 15 次：`README.md`、`cockpit/README.md`、`docs/handover.md` 各 5 次。
  - `AGENTS.md` 0 次，沒有其他路徑被查。

## 6. 窄視窗

- 改成 700 寬：並排組合保留，只顯示焦點欄 `docs/handover.md`（寬 666 px），`#review` 沒有 `data-split`，頁面沒有橫向捲軸。
  觀察 6 秒，中繼資料查詢只有 `docs/handover.md`（3 次）。
- 改回 2560 寬：三欄恢復並排、焦點欄不變，各欄 658.66／658.67／658.67 px，頁面沒有橫向捲軸。

## 收尾

- 依 PID 停止本次啟動的 headless Chrome 與 Cockpit 後端；兩個埠都已沒有 LISTEN。
- `<repo>` 的 git status 與測試前相同（空）；探針檔已不存在。
