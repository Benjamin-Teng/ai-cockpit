# split-check.js 使用說明

> 日期：2026-10-04（file-split-view task 2.1 建立）。對象：`docs/research/2026-10-04/split-check.js`。
> 性質：腳本用法與段落代號的參考文件；內容與腳本檔頭註解一致，加段落時兩邊一起改。

## 用途

對 `cockpit --example ui_preview`（固定 fixture 起的 dashboard，不需要 HERDR）驗證
`openspec/specs/file-review/spec.md`「檔案並排」（與修改過的「檔案分頁」「自動更新」「分頁還原」）中屬於前端的 scenario。

腳本共 47 段：2 段自我測試（`self/`）、1 段沒有並排時的基準（`baseline/`，重構與並排功能都不得讓它變紅）、44 段 spec
scenario 與設計審核的斷言（`file-review/`）。完整清單與每段的做法見下方「段落代號對照表」；加段落時表與腳本檔頭註解一起改。
新斷言依 tasks.md 通則先在未修的程式上跑出紅，再修。

headless Chrome＋raw CDP。啟動、收尾、行程所有權模型、段落代號與輸出格式沿用
`docs/research/2026-09-27/files-check.js`，前端 DOM 的定位規則沿用它的前端契約 C1–C3（見 `files-check.md`「前端契約」）。
點擊與按鍵都用 `Input.dispatchMouseEvent`／`Input.dispatchKeyEvent` 送真的輸入；`clickEl()`、`pressKey()` 可帶
modifiers（`MOD.CTRL` 等），給 Ctrl＋點選與 Ctrl＋Enter 用。

## 事前準備

- **不需要 HERDR**：全程只對 `ui_preview` 操作。
- **先 build**：`cargo build -p cockpit --example ui_preview`。前端資源內嵌在執行檔裡，改了 `cockpit/assets/` 一定要重新
  build，腳本不會自己 build，也不會判斷執行檔是不是最新的。
- **需要 Chrome**：預設 `C:\Program Files\Google\Chrome\Application\chrome.exe`，可用 `COCKPIT_CHROME` 指定。
- Node 22，不需要額外套件。
- **不要與其他驗收腳本同時跑**：埠不會撞，但並行時的負載會影響計時斷言。
- **只在 Windows 上跑**：收尾用 `tasklist`、`netstat`、`taskkill`，執行檔固定找 `target/debug/examples/ui_preview.exe`。

## 埠

| 用途 | 起點 | 說明 |
| --- | --- | --- |
| `ui_preview` | 7910 | `pickPort(7910)`，被占用就往上找 |
| headless Chrome 的 CDP | 19510 | `pickPort(19510)`，同上 |

選這兩個起點的理由：2026-10-04 grep `docs/research/*/*.js` 的 `pickPort(...)` 與 `PORT` 常數，其他腳本的 preview 用過
7770、7780、7792、7793、7830、7870、7970、7990，CDP 用過 18781–18991、19000–19200、19310、19410–19440 與 9333，
7910 與 19510 一帶沒有人用。

- 開跑前 7910 或 19510 已有人 LISTEN，或已有 user-data-dir 含 `cockpit-chrome-splitcheck-` 的 Chrome，就印
  `RESULT: FAIL (環境)` 並以 exit 2 結束。本腳本不動別人的行程，殘留的 Chrome 也只回報、不砍。
- 本腳本不使用也不檢查 7770 是否可用（那是使用者的 `cockpit.exe` 或別的腳本）。7770 有人 LISTEN、或有其他
  `ui_preview.exe` 在跑時只印一行「注意」，不處理。

## 執行方式

```bash
cargo build -p cockpit --example ui_preview

node docs/research/2026-10-04/split-check.js                              # 全部段落
node docs/research/2026-10-04/split-check.js "self/鷹架,baseline/"        # 只跑指定段落（逗號分隔）
```

- **參數**：只有一個位置參數，內容是以逗號分隔的段落代號清單，沒有其他旗標。省略參數就跑全部段落。給第二個位置參數
  也算錯誤（`RESULT: FAIL (段落代號)`，exit 2）。
- 段落代號含空白時整個參數要加引號；以 `<前綴>/` 結尾的代號選該前綴的全部段落（`file-review/` 選 44 段、
  `self/` 選 2 段、`baseline/` 選 1 段）。其餘代號必須與對照表逐字相同，含全形的「＋」。
- 代號拼錯、參數是空字串或只有逗號時，在啟動任何行程之前印 `RESULT: FAIL (段落代號)`，exit 2。
- **只跑某一段**：例如 `node docs/research/2026-10-04/split-check.js "file-review/窄視窗只顯示焦點欄"`。只跑少數段時
  收尾衛生檢查照樣執行，用來確認沒有留下行程。
- **結束碼**：0 ＝全部通過；2 ＝有斷言失敗、環境衝突（`RESULT: FAIL (環境)`）或段落代號錯誤；1 ＝腳本本身丟出
  未捕捉的例外（例如找不到 Chrome，可用 `COCKPIT_CHROME` 指定）。

### 跑一次要多久

全部 47 段在前景跑一次要好幾分鐘，不是幾十秒：實測 372～427 秒（task 3.5 約 372 秒、task 3.6 的 46 段為 427 秒），
段落仍在增加，預期 7 分鐘以上，實際依機器負載而定。時間大多花在自動更新相關段落的真實等待（輪詢是每 2 秒一次，「只查詢可見的檔案分頁」依
spec 有三個 10 秒觀察窗，輪詢類段落另各有 6 秒窗）。跑的人與呼叫它的 session 要預留逾時：背景或排程執行時，逾時至少給
15 分鐘，不要用預設的 2 分鐘。

## 段落代號對照表

| 代號 | spec（capability／Requirement） | 做法摘要 |
| --- | --- | --- |
| `self/鷹架` | 腳本自我測試 | `metaPathOf()` 對一般、巢狀、含空白與中文（百分比編碼）、非中繼資料端點的網址各驗一次；`filePathOf()` 對 meta／render／raw、含空白、list 與不完整的網址各驗一次；`classify()` 對各端點各驗一次。不啟動行程 |
| `self/段落代號` | 腳本自我測試 | `parseSegmentArg()` 對合法、前綴、重複、大小寫不符、未知前綴、空字串、只有逗號各驗一次；實際以拼錯的代號與空字串執行本檔，必須 exit 2、印 `RESULT: FAIL (段落代號)`、沒有進入任何段落 |
| `baseline/沒有並排時只有目前分頁可見` | file-review／自動更新、檔案分頁（基準） | 選定 `wJ:p4`，從檔案樹開 `README.md` 與 `docs/a.md`。目前分頁為 `docs/a.md` 時：恰一個分頁 `aria-selected="true"`；`#review` 沒有 `data-split`；實際可見的 tabpanel 只有 `docs/a.md` 的，`README.md` 與 Live Output 的都設 `hidden` 且不可見；觀察 6 秒（等 0.5 秒後開始），服務收到的中繼資料查詢只有 `docs/a.md`（至少 2 次），`README.md` 0 次。點 `README.md` 分頁後反過來再驗一次。最後確認頁面沒有未捕捉例外 |
| `file-review/快速切換時舊回應丟棄` | file-review／自動更新（變成不可見之後才回來的舊回應丟棄；design D3）；task 2.3 | A＝`docs/a.md`、B＝`README.md`，兩者內容都畫好，目前為 A。以 Fetch 攔住 A 的中繼資料查詢，等一筆卡住；不等選定連點 B → A → B，等 B 在最後一下之後發出的查詢回來，再多等 0.3 秒，把攔下的 A 查詢全以 404 `not_found` 放行，之後再等 2.5 秒。斷言：攔下的 A 查詢恰 2 筆（自我驗證：中間那下確實讓 A 可見）；B 仍為目前分頁、沒有過期標示、狀態列隱藏、分頁圖示與內容和切換前相同；A 沒有過期標示、狀態列隱藏；最後一下之後 A 沒有新的 meta／render／raw 請求；每筆攔下的 A 查詢都在放行前被頁面中止（`Network.loadingFailed` 且 `canceled`），放行時 Chrome 回報已不存在；沒有未捕捉例外 |
| `file-review/關閉分頁中止進行中的查詢` | file-review／自動更新（被關閉之後才回來的舊回應丟棄；design D3）；task 2.3 | 同上開兩個分頁，目前為 `docs/a.md`；攔住它的中繼資料查詢，等一筆卡住後點它的關閉鈕。斷言：分頁與 tabpanel 都移除、`README.md` 成為目前分頁；卡住的那筆在 1 秒內被頁面中止（`canceled`），放行時 Chrome 回報已不存在；關閉後觀察 2.5 秒，`docs/a.md` 沒有新的 meta／render／raw 請求；`README.md` 沒有過期標示、狀態列隱藏；沒有未捕捉例外 |
| `file-review/加入並排` | file-review／檔案並排「加入並排」；task 3.1 | 開 `README.md`、`docs/a.md`，選 `README.md`，按 `docs/a.md` 的並排鈕。斷言（`expectSplit`，下同）：並排組合依序 `README.md`、`docs/a.md`，`docs/a.md` 為目前分頁，兩者 `aria-pressed="true"`，兩個 tabpanel 可見、其餘 hidden；`README.md` 沒有重新讀取內容 |
| `file-review/沒有另一個檔案分頁時不並排` | 檔案並排「沒有另一個檔案分頁時不並排」「加入」第二點；task 3.1 | 只開 `README.md`，按它的並排鈕；再選 Live Output 按一次；再打開 Git Graph（目前分頁為 Git Graph）按一次（task 3.3 加）。斷言：三次都沒有並排組合、仍單欄、`aria-pressed="false"`。task 3.3 加停用呈現（`expectDisabled()`）：每種情況 `README.md` 的並排鈕 `aria-disabled="true"`、不帶 `disabled` 屬性、`title` 為字典 `files.tab.splitDisabled` |
| `file-review/替換焦點欄不影響其他欄` | 檔案並排「替換焦點欄不影響其他欄」；task 3.1 | `long.md`（第 1 欄，捲到 40%）與 `docs/a.md`（第 2 欄、焦點欄）並排，另開 `README.md`；點選 `README.md` 分頁。斷言：並排組合為 `long.md`、`README.md`，`README.md` 為目前分頁；`docs/a.md` 仍在分頁列且 `aria-pressed="false"`；`long.md` 沒有 render／raw 請求、內容節點沒換、`scrollTop` 不變 |
| `file-review/從檔案樹開檔替換焦點欄` | 檔案並排「從檔案樹開檔替換焦點欄」及「替換」的 md 相對連結入口；task 3.1 | `docs/a.md`、`README.md`（焦點欄）並排，在 `README.md` 欄點 `docs/design.md#決策` 連結 → 第 2 欄換成新開的 `docs/design.md`；再選 `docs/a.md`，從檔案樹開 `note.txt` → 第 1 欄換成新開的 `note.txt`。每一步斷言並排組合、目前分頁、被取代的分頁仍在，另一欄沒有重新讀取 |
| `file-review/三欄已滿時替換焦點欄` | 檔案並排「三欄已滿時替換焦點欄」；task 3.1 | `README.md`、`docs/a.md`、`long.md` 並排、焦點欄 `docs/a.md`，按 `note.txt` 的並排鈕。斷言：依序 `README.md`、`note.txt`、`long.md`，`note.txt` 為目前分頁，`docs/a.md` 仍在 |
| `file-review/移出焦點欄` | 檔案並排「移出焦點欄」；task 3.1 | 三欄、焦點欄 `docs/a.md`，按它的並排鈕。斷言：`README.md`、`long.md` 兩欄，`long.md`（右側欄）為目前分頁，`docs/a.md` 仍在、`aria-pressed="false"`、不可見。再按 `docs/a.md` 的並排鈕（加到最右欄、成為焦點欄），再按一次移出：沒有右側欄，焦點欄改為左側的 `long.md`（task 5.2 修正輪） |
| `file-review/關閉後只剩一個時解除並排` | 檔案並排「關閉後只剩一個時解除並排」、design D9；task 3.1 | 分頁列順序 `README.md`、`docs/a.md`、`long.md`，前兩者並排、焦點欄 `docs/a.md`，關閉它。斷言：沒有並排組合，`README.md` 單欄並為目前分頁（若誤用「改為顯示右側分頁」會變成 `long.md`）。接著 `README.md`、`long.md`、`note.txt` 三欄、焦點欄 `README.md`，用滑鼠關閉非焦點欄 `long.md`：目前分頁不變，鍵盤焦點移到右側相鄰的 `note.txt`（task 5.2 修正輪） |
| `file-review/不在並排中時關閉並排組合的成員` | 檔案並排「不在並排中時關閉並排組合的成員」、design D9；task 3.1 | 三欄、焦點欄 `docs/a.md`，選 Live Output 後關閉 `docs/a.md`，再點 `README.md`。斷言：關閉後仍為 Live Output 單欄、並排組合剩 `README.md`、`long.md`；點選後兩欄並排、`README.md` 為目前分頁 |
| `file-review/不在並排中時加入已滿的並排組合` | 檔案並排「不在並排中時加入已滿的並排組合」；task 3.1 | 三欄、焦點欄 `docs/a.md`，選 Live Output 後按 `note.txt` 的並排鈕。斷言：依序 `README.md`、`note.txt`、`long.md` 三欄可見，`note.txt` 為目前分頁 |
| `file-review/切到 Live Output 後整組恢復` | 檔案並排「切到 Live Output 後整組恢復」；task 3.1 | `long.md`、`docs/design.md` 並排並各捲到中段，點 Live Output 再點 `long.md`。斷言：期間只有 Live Output 可見、並排組合保留；回來後兩欄可見、`long.md` 為目前分頁，兩欄 `scrollTop` 不變、沒有 render／raw 請求、內容節點沒換 |
| `file-review/非檔案分頁不能並排` | 檔案並排「非檔案分頁不能並排」（3.1 部分）；task 3.1 | `README.md`、`docs/a.md` 並排，從左欄「變更」打開 Git Graph。斷言：Git Graph 為目前分頁並單獨顯示、並排組合不變；Git Graph 與 Live Output 分頁沒有並排鈕；點回 `README.md` 後兩欄恢復。task 3.3 加：「沒有並排鈕」改用 `splitButtonsIn()`（帶 `aria-pressed` 的 `<button>` 與 `.review-tab-split` 都算）；另開 diff 分頁（`history/unstaged-change.txt`），它也沒有並排鈕；並排中按住 Ctrl 點選 Git Graph → 單獨顯示、並排組合不變；移出 `docs/a.md` 使並排解除後（spec 的 GIVEN）按住 Ctrl 點選 Git Graph → 單獨顯示、沒有形成並排 |
| `file-review/並排中的非焦點欄也更新` | 自動更新「並排中的非焦點欄也更新」；task 3.1 | `docs/a.md`（第 1 欄）與 `README.md`（焦點欄）並排，在暫存副本的 `docs/a.md` 末端追加一段。斷言：3 秒內第 1 欄出現新段落，焦點欄仍為 `README.md` |
| `file-review/並排中各欄輪詢互不影響` | 自動更新（每個可見分頁各自查詢）、design D2 的 error 重試、D3；task 3.1（2.3 留下的實證缺口） | 三欄 `README.md`、`docs/a.md`、`long.md`（焦點欄），另開 `note.txt`。① 觀察 6 秒：三欄各至少 2 次查詢、`note.txt` 0 次。② 攔住 `docs/a.md`（非焦點欄）的查詢後關閉它：1 秒內被中止、放行時已不存在，之後 6 秒 `README.md`、`long.md` 照常查詢、`docs/a.md` 沒有新請求。③ 刪掉 `README.md` 讓它進入 error，等它一次查詢回來後立刻從檔案樹點它（可見、非焦點、error）：在下一次排定查詢之前（回應後 1.5 秒內）就有一筆重試，之後 6 秒 `long.md` 照常查詢 |
| `file-review/並排版面等寬` | 檔案並排「加入並排」（等寬兩欄、左右順序）、「三欄已滿時替換焦點欄」（替換後的順序）、「版面」；design D4；task 3.2 | 視窗 1280×900。開 `README.md`、`docs/a.md`、`long.md`、`note.txt`；只有 `README.md` 時量中欄寬度；並排成兩欄、三欄；焦點欄移到中間後按 `note.txt` 的並排鈕，依序 `README.md`、`note.txt`、`long.md`（`note.txt` 的面板在 DOM 裡排在 `long.md` 之後，只有 CSS `order` 能讓它在中間）；選 Live Output 再點回。每一步 `checkLayout`，並驗 `#review` 子節點順序沒變 |
| `file-review/三欄並排不撐破頁面` | 檔案並排 scenario「三欄並排不撐破頁面」、design Risks 第一條；task 3.2 | 視窗 1280×900、推送間隔 1 秒。暫存副本寫 `long.txt`（第 2 行 300 個字元）。單欄的 `report.pdf` 先驗符合寬度並記下頁寬；`README.md`、`long.txt`、`report.pdf` 三欄並排後等一次整頁重畫。斷言：`checkLayout`；`long.txt` 的長行在欄內溢出且捲得動、面板本身沒有溢出；`report.pdf` 仍為符合寬度、頁寬比單欄小、捲動區沒有橫向溢出 |
| `file-review/並排切換不重新載入 iframe` | design D4「不搬 DOM」的理由（搬動 iframe 會重新載入）；task 3.2 | 視窗 1280×900。`page.html`（html 檢視器）的 iframe 貼記號、掛 load 計數。並排 → Live Output → 回到並排 → 三欄 → 換焦點欄 → 移出 → 單欄顯示。斷言：iframe 仍是同一個節點、load 0 次、沒有 `page.html` 的 raw 請求、`#review` 子節點順序沒變 |
| `file-review/整頁重畫不影響並排` | 檔案分頁 scenario「整頁重畫不影響並排」；task 3.2 | 視窗 1280×900、推送間隔 100 ms。`long.md`、`docs/design.md`（焦點欄）並排並各捲到中段，面板與內容節點貼記號；觀察 3 秒（重畫至少 10 次）。斷言：仍兩欄並排、焦點欄不變（含 `checkLayout`）、兩欄捲動位置不變、沒有重新讀取、內容節點與面板節點都沒換、子節點順序沒變 |
| `file-review/切換 Project 不影響並排` | 檔案分頁 scenario「切換 Project 不影響並排」；task 3.2 | 前置與斷言同上（推送間隔維持 10 分鐘）；操作改為點左欄 Project 分頁中的另一個 Project |
| `file-review/Ctrl＋點選加入並排` | 檔案並排「Ctrl＋點選加入並排」「並排鈕」；task 3.3 | 開 `README.md`、`docs/a.md`，選 `README.md`，按住 Ctrl 點選 `docs/a.md`。斷言同「加入並排」（兩欄、`docs/a.md` 為焦點欄、`README.md` 沒有重讀）；頁面收到恰一次 click、落在 `docs/a.md`、`ctrlKey` 為 true（驗 `clickEl()` 的 modifiers）。再按住 Ctrl 點選 `docs/a.md` → 等同按它的並排鈕：移出、並排解除，`README.md` 單欄 |
| `file-review/鍵盤加入並排` | 檔案並排「鍵盤加入並排」、design D7 的 `preventDefault()`；task 3.3 | 點 `README.md`（焦點在它上面），按右方向鍵（只移焦點、不選定），按 Ctrl＋Enter。斷言：兩欄並排、`docs/a.md` 為焦點欄、鍵盤焦點仍在 `docs/a.md`；之後頁面沒有收到任何 click（只觸發一次動作）；頁面收到的 keydown 與送出的相同（驗 `pressKey()`）。再按 Ctrl＋Enter → 移出、解除，同樣沒有 click。最後選 Live Output、右方向鍵到 `README.md` 按 Ctrl＋Enter（並排鈕停用）→ 等同一般選定，`README.md` 單欄，沒有 click |
| `file-review/並排鈕停用時 Ctrl＋點選等同一般選定` | 檔案並排「並排鈕停用時 Ctrl＋點選等同一般選定」；task 3.3 | 開 `README.md`、`docs/a.md`，目前 `docs/a.md`。斷言：`docs/a.md` 的並排鈕停用、`README.md` 的可用（`expectEnabled()`：沒有 `aria-disabled="true"`、`title` 為「並排」）；按住 Ctrl 點選 `docs/a.md` → 沒有並排、仍單欄。選 Live Output 後按住 Ctrl 點選 `README.md`（停用）→ `README.md` 單欄並為目前分頁 |
| `file-review/並排鈕的顯示時機與 Tab 順序` | 檔案並排「並排鈕」（顯示時機、Tab 順序）、design D7；task 3.3 | 開 `README.md`、`docs/a.md`、`long.md`，目前 `README.md`。滑鼠移開：只有目前分頁的並排鈕顯示；滑鼠移到 `docs/a.md` 上它才顯示、移開又不顯示；目前分頁的並排鈕 `tabIndex` 0，其餘 -1。按 `docs/a.md` 的並排鈕後換成它顯示、`README.md`（並排中但不是目前分頁）只在滑鼠移上去時顯示。真的按 Tab：分頁 → 它的並排鈕 → 它的關閉鈕。選 Live Output：全部不顯示、`tabIndex` 都是 -1。Live Output 沒有並排鈕 |
| `file-review/欄位編號標記與說明` | 檔案並排「焦點欄的標示與切換」（欄位編號與「並排第 N 欄」說明）、design D7；task 3.3 | 三欄 `README.md`、`docs/a.md`、`long.md`，另開未並排的 `note.txt`。斷言：並排組合中的分頁 `data-split-col`＝N、分頁按鈕 `::after` 的 content 以 `"N"` 開頭、`aria-describedby` 依序指到字典 `files.tab.splitCol`（「並排第 N 欄」）與完整路徑＋根目錄（同分頁的 `title`，修正第 1 輪）；無障礙樹上名稱仍是檔名（徽章不混進名稱）、說明為「並排第 N 欄」接著路徑與根目錄。`note.txt` 都沒有。選 Live Output 期間照舊；移出 `docs/a.md` 後 `long.md` 改為第 2 欄 |
| `file-review/並排鈕文字隨介面語言` | ui-language、task 3.3 | 五個鍵（`files.tab.split`、`splitDisabled`、`splitNamed`、`splitCol`、`title`）中英兩份字典都有且兩種語言不同。繁中時核對兩顆並排鈕的 `title`（可用／停用）、`aria-disabled`、`aria-label` 與並排後的說明（「並排第 N 欄」＋該語言的路徑與根目錄）；以 `cockpitI18n.setLang('en')` 切換、頁面重新載入、分頁還原後，以英文再核對一次（並排組合必須已經還原，沒有還原就失敗，不重新組） |
| `file-review/關閉並排的焦點欄時接手分頁捲進視野` | 檔案並排「移出」、審查第 1 項（`closeTab()` 的並排分支補 `revealTab()`）；task 3.3 | 視窗 1280×900、開 10 個檔案讓分頁列要橫向捲動。目前 `report.pdf`（右段、不是最後一個）時按 `README.md` 的並排鈕，前置確認 `report.pdf` 不在可視範圍；關閉 `README.md` → 並排解除、`report.pdf` 為目前分頁，它的包裝元素整個在可視範圍內。**只當回歸防護**：未修的程式上也綠，因為 Chrome 的 `focus()` 會把接手分頁捲進來（見腳本註解） |
| `file-review/不在並排中移出成員時分頁列不捲動` | 檔案並排「移出」（移出前不是並排中，目前分頁不變）；task 3.3 修正第 1 輪（審查 Minor 1） | 視窗 1280×900、開 10 個檔案。`page.html`、`report.pdf`、`style.css` 三欄並排後選 Live Output。先把 `page.html` 的並排鈕捲進可視範圍、記下 `scrollLeft`（需大於 0），按它移出 → `scrollLeft` 不變（≤ 1 px）。同樣記下 `report.pdf` 關閉鈕的基準，關閉它 → `scrollLeft` 不變、目前分頁仍是 Live Output、鍵盤焦點移到右側相鄰的 `style.css` |
| `file-review/在欄內點選切換焦點欄` | 檔案並排「在欄內點選切換焦點欄」「焦點欄的標示與切換」、design D6；task 3.4 | 推送間隔 1 秒。`long.md`（第 1 欄）與 `docs/design.md`（第 2 欄、焦點欄）並排，兩欄捲到中段並貼記號。在 `long.md` 欄的內容上按一下（`pressInHost()`，不捲動、不按在連結上）→ `long.md` 為焦點欄與目前分頁；兩欄都沒有重新讀取、捲動位置不變；沒有 `:focus-visible`，等一次整頁重畫後仍沒有。分頁列上的 `docs/design.md` 分頁只按下不放開 → 目前分頁仍是 `long.md`；放開後照一般選定。選 Live Output 後在其面板上按一下 → 仍是 Live Output、並排組合不變。最後按 Tab 自我驗證 `:focus-visible` 量得到 |
| `file-review/在非焦點欄點 md 相對連結在那一欄開啟` | 檔案並排「替換」（md 相對連結）、design D6；task 3.4 | `README.md`（第 1 欄）與 `docs/a.md`（第 2 欄、焦點欄）並排，在 `README.md` 欄點 `docs/design.md#決策` 的連結 → 並排組合依序 `docs/design.md`、`docs/a.md`，`docs/design.md` 為焦點欄；`README.md` 分頁仍在；`docs/a.md` 沒有重新讀取；沒有 `:focus-visible` |
| `file-review/在非焦點的 PDF 欄點下一頁照常翻頁` | design Risks「pointerdown 改焦點欄可能與既有點擊行為衝突」；task 3.4 | `report.pdf`（第 1 欄）與 `README.md`（第 2 欄、焦點欄）並排，等 PDF 重排成符合寬度、頁碼「1 / 3」。點 `report.pdf` 欄的「下一頁」→ `report.pdf` 為焦點欄，頁碼變「2 / 3」；兩欄都沒有重新讀取；沒有 `:focus-visible` |
| `file-review/在 html 欄按下滑鼠切換焦點欄` | 檔案並排「焦點欄的標示與切換」、design D6；task 3.4、修正第 1 輪（控制端裁決：iframe 以 window blur 補上） | `page.html`（第 1 欄）與 `README.md`（第 2 欄、焦點欄）並排，iframe 貼記號、掛 load 計數。① 在 `page.html` 的 iframe 上按一下 → `page.html` 為焦點欄；`README.md` 沒有重讀、內容節點沒換；iframe 是同一個節點、沒有重新載入、沒有 `page.html` 的 render／raw 請求；沒有 `:focus-visible`。② 在 `README.md` 內容上按一下再按 iframe → 再次切換。③ 點 `page.html` 分頁後按它自己的 iframe → 不變。④ 點 `README.md` 分頁，對 window 送合成的 blur（activeElement 不是 iframe）→ 不變。⑤ 在 `page.html` 的工具列按一下 → `page.html` 為焦點欄。⑥ 移出並排、單欄顯示 `page.html` 後按 iframe → 不變。每次按 iframe 都確認父文件 window 收到 blur |
| `file-review/兩個 html 欄之間直接切換焦點欄` | 檔案並排「焦點欄的標示與切換」；task 3.4 修正第 2 輪（審查 Important；控制端裁決 (a) 輪詢） | 在暫存副本寫 `page2.html`。`page.html`（第 1 欄）與 `page2.html`（第 2 欄、焦點欄）並排，兩個 iframe 貼記號、掛 load 計數，頁面裡以 `installTimerLog()` 記待執行的計時器。⓪ 焦點在父文件時沒有 `followIframeFocus` 計時器。① 按 `page.html` 的 iframe → 它成為焦點欄，且恰有一個輪詢計時器（正向對照）。② 直接按 `page2.html` 的 iframe（父文件收不到任何焦點事件）→ 它成為焦點欄；兩欄都沒有 render／raw 請求，兩個 iframe 都是同一個節點、沒有重新載入。③ 在 `page2.html` 的工具列按一下，焦點離開 iframe → 0.6 秒後沒有輪詢計時器，焦點欄不變 |
| `file-review/窄視窗只顯示焦點欄` | 檔案並排「窄視窗」、scenario「窄視窗只顯示焦點欄」、design D5；task 3.5 | 寬度以 `setWidth()` 切換（見「量測方式」）。寬 1280 時 `README.md` 與 `docs/a.md`（焦點欄）並排；改為 700 → 只顯示 `docs/a.md`（顯示的欄數 1）、並排組合與欄位標記不變、`#review` 沒有 `data-split`、頁面沒有橫向捲軸；改回 1280 → 恢復兩欄並排（顯示的欄數 2）。來回兩次 |
| `file-review/只查詢可見的檔案分頁` | 自動更新 scenario「只查詢可見的檔案分頁」（從不可見變成可見時立即查詢一次、不可見的分頁不得查詢、舊回應丟棄）；task 3.5 | 寬 1280，`README.md` 與 `docs/a.md`（焦點欄）並排，另開 `note.txt`。① 觀察 10 秒：服務只收到 `README.md` 與 `docs/a.md` 的中繼資料查詢（各至少 3 次），`note.txt` 0 次。② 以 Fetch 攔住 `README.md` 的查詢、等一筆卡住，改為寬 700 → 1 秒內被頁面中止，放行時 Chrome 回報已不存在；關掉攔截。③ 寬 700 觀察 10 秒：只有 `docs/a.md`（至少 3 次），`README.md`、`note.txt` 0 次；`README.md` 沒有過期標示。④ 改回寬 1280 → `README.md` 在 1 秒內被查一次；再觀察 10 秒，兩欄各至少 3 次、`note.txt` 0 次 |
| `file-review/窄視窗下仍替換焦點欄` | 檔案並排 scenario「窄視窗下仍替換焦點欄」；task 3.5 | 寬 1280 時 `README.md` 與 `docs/a.md`（焦點欄）並排、另開 `note.txt`，改為寬 700。點選 `note.txt` 分頁 → 只顯示 `note.txt`，並排組合依序 `README.md`、`note.txt`；改為寬 1280 → 這兩欄依序並排，`note.txt` 為焦點欄 |
| `file-review/窄視窗下按下滑鼠不切換焦點欄` | 檔案並排「窄視窗」＋design D6；task 3.5（3.4 審查交辦） | `README.md` 與 `docs/a.md`（焦點欄）並排。改為寬 700：非焦點欄的面板 `hidden`、點不到；在焦點欄內容上按一下 → 不變。改回寬 1280，在非焦點欄內容上按一下 → 它成為焦點欄。兩欄角色對調再來一輪 |
| `file-review/還原並排` | 分頁還原 scenario「還原並排」、design D8；task 3.6 | spec 的 `docs/b.md` 改用 `long.md`。打開 `README.md`、`docs/a.md`、`long.md`，組成並排前紀錄裡沒有 `split`／`splitFocus`。`long.md` 與 `README.md`（焦點欄）依序並排後，紀錄為 `v: 2`、`split: [2, 0]`、`splitFocus: 0`。重新整理 → 三個分頁依序還原，`long.md`、`README.md` 兩欄並排、`README.md` 為焦點欄，兩欄都顯示內容，沒有並排相關的警告 |
| `file-review/選定 Live Output 時重新整理` | 分頁還原 scenario「選定 Live Output 時重新整理」；task 3.6 | `README.md` 與 `docs/a.md`（焦點欄）並排後選定 Live Output，紀錄為 `current: null`、`split: [0, 1]`、`splitFocus: 1`。重新整理 → 目前分頁為 Live Output，只顯示它；欄位標記保留，兩個並排鈕 `aria-pressed="true"` 且可用。點選 `README.md` → 兩欄恢復並排並顯示內容 |
| `file-review/並排資料不合法時忽略` | 分頁還原 scenario「並排資料不合法時忽略」、design D8；task 3.6 | 打開 `README.md`、`docs/a.md`、`long.md`、`note.txt` 與 Git Graph，目前為 `docs/a.md`。以前端寫出的紀錄為底，逐一寫入：只指到一個分頁、指到 4 個、同一個索引兩次、同一個檔案存兩筆且 split 指到這兩筆、指到 Git Graph、指到被略過的檔案分頁、索引越界。每次重新載入後，分頁依序還原、目前為 `docs/a.md`、沒有並排、console 有提到並排的警告。被略過的分頁：在 `README.md` 之後插一筆 `../bad.md`，split 為 `[1, 2]`，另斷言並排不是位移後的 `docs/a.md`、`long.md`。正向對照：同一份 tabs、split `[3, 2]` → `long.md`、`docs/a.md` 依序並排，沒有並排相關的警告 |
| `file-review/舊格式照常還原` | 分頁還原 scenario「舊格式照常還原」；task 3.6 | 打開 `README.md`、`docs/a.md`，以前端寫出的紀錄改成 v1（去掉 `kind`、`current` 去掉 `rootName`）寫回、重新載入 → 兩個分頁依序還原並顯示內容、沒有並排；再以沒有 `split` 的 v2 紀錄做一次。兩次都沒有並排相關的警告 |
| `file-review/焦點欄不在並排組合中時改用第一欄` | 分頁還原需求（焦點欄的兩條規則）、design D8；task 3.6 | 焦點欄以「三欄已滿時按第 4 個檔案的並排鈕會取代焦點欄」觀察。split 為 `README.md`、`docs/a.md`、`long.md`，目前為 Live Output：splitFocus 指到 `note.txt` 或沒有 splitFocus → 按 `note.txt` 的並排鈕取代第 1 欄；對照 splitFocus 為 `docs/a.md` → 取代第 2 欄。splitFocus 為 `README.md`、目前為 `docs/a.md` → 還原成三欄並排、焦點欄 `docs/a.md`，選定 Live Output 後按 `note.txt` 的並排鈕 → 取代第 2 欄 |
| `file-review/寫入格式與回滾相容` | design D8、Migration Plan；task 3.6 | 打開 `README.md`、Git Graph、`docs/a.md`、`long.md`，把 Git Graph kind 的 `serialize()` 換成回傳 `null`。`docs/a.md`、`long.md`、`README.md` 並排、焦點欄 `docs/a.md` → 紀錄的 tabs 不含 Git Graph，`split: [1, 2, 0]`、`splitFocus: 1`（索引在略過之後才算）。選定 Live Output 後，用上一個發行版（tag `v0.1.2`）的 `isStoredState()` 檢查這份紀錄回傳 true，改成 `v: 3` 時回傳 false。重新載入 → 並排組合保留；依序移出 `README.md` 與 `docs/a.md` 使並排解除 → 紀錄不再有 `split`／`splitFocus` |
| `file-review/設計審核修正的外觀` | 設計審核修正（`file-split-view` task 4.1；非 spec scenario，是外觀與鍵盤順序的實作約束） | 兩組 preview。①視窗 1280，開 `README.md`、`long.md`、`note.txt`，並排成 `README.md`、`note.txt`、`long.md`（`note.txt` 的面板在 DOM 裡排在 `long.md` 之後）；沒有並排時 `#review` 的 `reading-flow` 為 `normal`，並排時為 `grid-order`；點 `note.txt` 讓焦點欄在中間，連按 Tab（最多 40 次）記錄走過的欄，各欄第一次被走到的順序必須是 `README.md`、`note.txt`、`long.md`（F4）。②在 1280 寬組好 `README.md`、`note.txt`、`report.pdf` 三欄後改成 1100 寬，焦點欄依序在最右、最左、中間，每一種都驗：每欄 `.file-toolbar-path` 寬度 ≥ 7em（F1）；焦點欄有 1px 實線 `--accent` 外框（`outline`）、框線仍是 1px `--accent`、外框沒有被任何有 overflow 的祖先裁掉，其他欄沒有外框（F3）；欄位徽章只有焦點欄（目前分頁）是 `--accent`、其餘是 `--text-dim`（F5）。③改成 800 寬、焦點欄 `report.pdf`：每顆 `.pdf-tool` 的 `white-space` 為 `nowrap`、高度 ≤ 30 px、右緣不超出欄內緣，`.pdf-toolbar-group` 的 `flex-wrap` 為 `wrap`（F2） |

spec scenario 裡的 pane 對應同 `files-check.md`：`w1:p1`→`wJ:p4`（cwd＝review-repo/src）、`w2:p1`→`wJ:p5`
（cwd＝other-repo）。需要額外檔案或改寫檔案的段落一律只寫 ui_preview 的暫存副本（路徑取自 stdout 的
`review-repo: <路徑>`，在 `ctx.preview.reviewRepo`），不碰 `cockpit/examples/fixtures/review-repo/`。

## 量測方式

- **中繼資料查詢**：CDP `Network.requestWillBeSent`（只記主頁面 session），以 `metaPathOf()` 把
  `/api/files/<runtime>/<root_id>/meta/<相對路徑>` 解成相對路徑分組計數。`observeMeta(ctx, windowMs)` 先等 0.5 秒，
  讓切換前已發出、正被中止的請求不落在觀察窗內，再數窗內的請求。
- **基準段的次數門檻**：目前分頁在 6 秒窗內至少 2 次。task 2.1 在未改的程式上實跑，兩個方向都是 3 次。相鄰兩次查詢
  的開始至少相隔 2 秒加一次回應時間，6 秒窗依相位落在 2～3 次，要求 3 次會在邊界上偶發失敗，所以取 2。這條只防
  「目前分頁沒在輪詢」，主斷言是另一個分頁 0 次。
- **可見**：`window.__sc.reviewState()` 對每個分頁回報 tabpanel 的 `hidden` 屬性與實際可見（在文件中、不是
  `display:none`／`visibility:hidden`、有版面框）；`checkVisiblePanels()` 要求實際可見的恰為預期的分頁，其餘都
  `hidden` 且不可見，`#review` 內沒有無主的可見 tabpanel。
- **整頁重畫**：讀 `#version` 的 `data-state-version` 屬性（`stateVersion()`、`waitForRepaint()`）。`#version` 的
  textContent 是固定的程式版本號，不會隨投影改變，不能拿來判斷重畫。
- **讓回應一定晚到**（task 2.3）：`installMetaHold(ctx, 相對路徑)` 以 CDP `Fetch.enable`（Request 階段，只比對該檔的
  `/meta/<路徑>`）攔下查詢，不送到服務，直到 `release(status, body)` 以 `Fetch.fulfillRequest` 放行。放行時 Chrome
  回報 `Invalid InterceptionId.`（task 2.3 實測的訊息）時，代表這筆已不存在（頁面已中止它），記為 `gone`；放行成功記為
  `delivered`。放行的回應是 404 `not_found`：若被套用，畫面會出現「檔案已不存在」與過期標示，一看就知道。
- **中止**：`recordNetwork()` 另記 `Network.loadingFailed` 的 `canceled` 與時間、`responseReceived` 的時間。頁面
  呼叫 `AbortController.abort()` 時 Chrome 回報 `canceled: true`、`net::ERR_ABORTED`。
- **點擊回覆早於點擊處理**：`Input.dispatchMouseEvent` 回覆之後，頁面的點擊處理還要再過幾毫秒（task 2.3 實測：中止
  事件落在回覆後 2～16 ms）。所以「點完」不等於「切換已完成」；快速切換段改等 B 在最後一下之後發出的查詢回來，
  再多等 0.3 秒才放行。task 2.3 第一版直接拿「點完」當完成，在搬移前的程式上跑 9 次有 6 次在中止之前就放行，造成假紅。
- **並排狀態**（task 3.1）：只驗狀態；幾何由下一項的 `checkLayout()` 驗。`window.__sc.splitState()` 讀出：
  - **欄位順序**：檔案分頁的包裝元素（`.review-tab`，分頁的父元素）帶 `data-split-col="1|2|3"`（design D7 的欄位標記），
    依編號排出並排組合；沒有並排組合時沒有任何分頁帶這個屬性。
  - **並排鈕**：包裝元素裡帶 `aria-pressed` 的 `<button>`（`splitButtonFor()`）。Live Output 分頁直接放在分頁列裡，
    沒有包裝元素。「某分頁有沒有並排鈕」（`buttons`）task 3.3 起改用 `splitButtonsIn()`：帶 `aria-pressed` 的 `<button>`
    與 `.review-tab-split` 都算，git 類分頁誤長出沒有 `aria-pressed` 的並排鈕也抓得到。
  - **目前分頁**：`aria-selected="true"`。
  - **可見**：tabpanel 實際可見的分頁。
  - `expectSplit()` 先輪詢到狀態相符再逐項斷言，另驗欄位編號連續、恰一個 `aria-selected`、`aria-pressed` 恰好反映是否
    在並排組合中、其餘 tabpanel 都 `hidden`。
- **並排版面**（task 3.2；design D4、控制端裁決 C）：`expectSplit()` 每次最後都呼叫 `checkLayout()`，所以 3.1 的狀態段落也一併
  驗版面。幾何取自 `window.__sc.layoutState()`（`getBoundingClientRect`、`getComputedStyle`）。
  - **並排顯示中**（預期可見的恰為並排組合、至少 2 個）：`#review` 的 `data-split` 等於欄數；各欄上緣相同、由左到右依並排組合
    的順序、互不重疊、等寬（寬差 ≤ 1 px）、落在 `#review` 左右邊界內，各欄寬加間距等於 `#review` 的寬（±2 px）；只有焦點欄
    的面板帶 `data-split-focus`，它的上緣框線顏色等於 `--accent`，其他欄不等於。
    各欄內容不超出欄寬（spec「某欄內容超出欄寬時，在該欄內捲動或折行」）：面板本身沒有橫向溢出（`scrollWidth` 不大於
    `clientWidth`＋1），檢視器捲動容器 `.file-viewer-host` 以外的元素（工具列等）右緣不超出面板內緣；不符時列出超出的元素。
  - **沒有並排顯示時**：`#review` 沒有 `data-split`，也沒有任何元素帶 `data-split-focus`。
  - **兩種情況都驗**：頁面沒有橫向捲軸（`documentElement` 的 `scrollWidth` 不大於 `clientWidth`）；`#review` 的寬（中欄寬度）
    與本段第一次量到的值相同（≤ 0.5 px）。3.2 的段落第一次量在並排之前。
  - **1280 寬**：headless Chrome 的 `--window-size=1280,900` 實測 `innerWidth` 只有 1258，所以 3.2 的段落改用
    `Emulation.setDeviceMetricsOverride` 固定 1280×900，再量 `innerWidth` 確認（`need1280()`）。
  - **不搬 DOM**：`window.__sc.reviewChildOrder()` 讀 `#review` 直屬子節點的 id 順序，操作前後比對（`checkChildOrder()`）。
    面板節點沒被換掉用面板元素上的記號判斷（`markPanels()`）。
- **沒有重新讀取、捲動不變**（task 3.1）：`checkColumnUntouched()` 驗三件事：
  - 操作之後沒有該檔的 render／raw 請求。中繼資料查詢照常，不算重讀。
  - `.file-viewer-host` 第一個子節點上事先貼的記號仍在，表示內容節點沒有被換掉。
  - `scrollTop` 與操作前相差不超過 1 px。捲動用 `scrollHostTo()` 直接設 `scrollTop`，再等 0.3 秒讓 files.js 的捲動
    監聽記下位置。
- **並排鈕的呈現與欄位標記**（task 3.3）：`window.__sc.splitUi()` 以分頁名稱為鍵回報並排鈕的個數、`aria-pressed`、
  `aria-disabled`、是否帶 `disabled` 屬性、`title`、`aria-label`、`tabIndex`、是否顯示，以及包裝元素的 `data-split-col`、
  分頁 `aria-describedby` 指到的文字、分頁按鈕 `::after` 的 content。
  - **顯示**：`display` 不是 none、`visibility` 為 visible、`opacity` ≥ 0.99、有版面框。滑鼠用真的 `mouseMoved` 移到分頁中心
    （`hoverTab()`）或頁面左上角 (2, 2)（`mouseAway()`，先確認那裡不在分頁列裡）。
  - **無障礙樹**：`axOf(ctx, 選擇器)` 以 CDP `DOM.querySelector`＋`Accessibility.getPartialAXTree` 讀 role、名稱與說明。
    第一次用時先查 `README.md` 的關閉鈕，名稱必須是「關閉 README.md」。
  - **輸入是否到達頁面**：`installInputLog()` 在 document 捕捉階段記下 keydown（key、ctrlKey）與 click（落在哪個分頁、
    ctrlKey），`takeInput()` 取出並清空。用來驗 `clickEl()`／`pressKey()` 的修飾鍵，以及 Ctrl＋Enter 之後沒有多一次 click。
  - **介面文字**：期望值取自頁面的 `window.cockpitI18n.dictionaries`（`dictTexts()`），`{name}`、`{n}` 以 `fill()` 代入。
- **檔名對應**：spec scenario 的 `a.md`～`d.md` 依序對應 fixture 的 `README.md`、`docs/a.md`、`long.md`、`note.txt`。
  需要捲到中段的欄改用夠長的 `long.md` 與 `docs/design.md`，各段註解寫明。
- **立即重試的判準**（task 3.1）：等 error 欄的一次查詢回來後立刻點它，下一次排定的查詢在回應後 2 秒。所以「回應後
  1.5 秒內出現的查詢」一定是點擊觸發的重試，不會被排定的查詢混過去。
- **檔案分頁的狀態**：`fileSnap(ctx, 相對路徑)` 讀分頁與 tabpanel（面板 `hidden` 時也讀得到）：`aria-selected`、
  `.is-stale`、`.file-status` 是否顯示與文字、分頁圖示 `src`、`.file-viewer-host` 的文字內容。`waitFileReady()`
  等到狀態列隱藏、沒有過期標示、內容非空。
- **在欄內按下滑鼠**（task 3.4）：`clickEl()` 會先 `scrollIntoView`，會干擾「捲動位置不變」，所以按欄內容改用
  `freePoint()`＋`pressAt()`：在目標元素框內依序試幾個比例位置，取第一個命中測試落在該元素內、而且不在連結、按鈕、
  輸入欄位或分頁上的點，直接送 `mouseMoved`／`mousePressed`／`mouseReleased`，不捲動。
- **`:focus-visible` 外框**（task 3.4）：`checkNoFocusVisible()` 要求 `document.querySelector(':focus-visible')` 為 null，
  失敗時附 `activeElement` 與 `document.hasFocus()`。`在欄內點選切換焦點欄` 段最後按 Tab，確認量法量得到外框，不是恆為空。
- **html 檢視器的 iframe**（task 3.4、修正第 1 輪）：sandbox iframe 內的 `pointerdown` 不會傳到父文件；按進 iframe 時父文件
  的 `window` 收到 `blur`、`activeElement` 變成那個 `<iframe>`，產品靠這個切換焦點欄。`在 html 欄按下滑鼠切換焦點欄` 段在
  父文件記 `window` 的 blur 次數（只計 target 為 window 的），每次按 iframe 都先確認 blur 真的發生，「不變」的斷言才有意義。
  「切到別的應用程式」以 `window.dispatchEvent(new FocusEvent('blur'))` 模擬（headless 沒有真的視窗可切），這時
  `activeElement` 是分頁按鈕，不是 iframe。
- **輪詢計時器**（task 3.4 修正第 2 輪）：焦點從一個 iframe 直接移到另一個 iframe 時，父文件收不到任何焦點事件，產品改以
  短週期輪詢 `activeElement`。`installTimerLog()` 在頁面載入後包住 `setTimeout`／`clearTimeout`／`setInterval`／
  `clearInterval`，以 callback 的函式名稱記下還沒執行、也沒被清掉的計時器，不改產品程式。files.js 每次呼叫時才讀全域的
  `setTimeout`，所以載入後再包也攔得到。斷言以名稱 `followIframeFocus` 計數，並先做正向對照：焦點在 iframe 裡時恰有一個，
  確認量法量得到。
- **改變視窗寬度**（task 3.5）：`setWidth(ctx, w)` 以 `Emulation.setDeviceMetricsOverride`（`w`×900）改寬度，同一個頁面不重新
  載入，產品要靠 `matchMedia` 的 `change` 事件反應。改完等 0.3 秒，再確認 `innerWidth` 與 `(min-width: 760px)` 都跟著變。
  中欄寬度隨視窗改變，所以 `checkLayout()` 的「中欄寬度不變」基準每種寬度各存一份，切換時跟著換。700 寬是單欄版面、整頁捲動，
  底部有固定的狀態列：`clickEl()` 的 `scrollIntoView({ block: 'nearest' })` 會把下方的分頁捲到視窗最底、被狀態列蓋住，
  命中測試失敗。所以窄視窗下點分頁或按欄內容之前，先以 `revealTab()`／`revealHost()` 把目標捲到視窗中間（捲頁面，不捲
  檢視器容器本身）。
- **只查詢可見的分頁**（task 3.5）：量的是頁面實際送出的中繼資料查詢（CDP `Network.requestWillBeSent`），不是 CSS。
  觀察窗依 spec 為 10 秒，可見分頁至少 3 次（理論 4～5 次），主斷言是不可見分頁 0 次。攔截用的 Fetch 在改寬度、放行之後以
  `Fetch.disable` 關掉，否則之後 `README.md` 的查詢會一直卡住，輪詢不會繼續。
- **持久化**（task 3.6）：
  - **讀寫紀錄**：直接讀寫 `localStorage` 的 `cockpit.fileTabs`。`expectStored()` 先輪詢到相符再逐項斷言：`v` 為 2、
    tabs 的順序、`split`／`splitFocus` 換算成名稱後相符；沒有並排組合時用 `hasOwnProperty` 確認兩個欄位都不存在。
  - **種資料**：先經畫面打開分頁，讓前端寫出真的 v2 紀錄，再讀回來改寫後寫回（`seedAndReload()`），每一筆分頁紀錄都是
    前端自己寫出的形狀，不必另外查根目錄。
  - **重新載入**（`reloadPage()`）：先在舊頁面貼 `window.__scOldDoc` 記號，`Page.reload` 之後等記號消失，確認是新的
    document，再等第一份真投影與分頁列。不先確認的話，舊頁面可能讓等待立刻成立。
  - **警告**：只算這次載入之後、文字提到「並排」的 `console.warn`（`splitWarnings()`）。被略過的分頁另有自己的警告，
    文字不提並排，不會混淆。
  - **回滾相容**：`rollbackIsStoredStateSource()` 以 `git show` 取出回滾目標 `ROLLBACK_REF`（tag `v0.1.2`，`7f6ce36`，
    本 change 之前的最後一個發行版）的 `cockpit/assets/app/files.js`，取出 `isStoredState()` 原文與它用到的常數
    `STORAGE_VERSION`、`LEFT_*`，直接嵌進 `Runtime.evaluate` 的運算式求值，不經 `eval`／`new Function`。另以 `v: 3`
    做對照，確認取出的函式會拒絕不認得的版號。
    - **為什麼用固定 tag**：task 3.6 原本以 `git merge-base HEAD origin/main` 取合併基準。本分支 squash 併回 `main` 之後，
      在 `main` 上跑時 merge-base 就是 HEAD 自己，取出的是改版後的 `isStoredState()`，斷言恆真，變成假綠；沒有
      `origin/main` 的 clone 也會直接失敗。固定 tag 在合併前後都指向同一個舊版本（task 5.2 最終審查 I-1）。
    - **防誤指**：取出的 `files.js` 若已含 `splitTabs`（也就是含並排的版本），前置條件直接失敗。所以回滾目標不會靜默
      變成自己比自己。clone 沒有帶 tag 時也是前置失敗，先 `git fetch --tags`。
    - 之後要拿新的發行版當回滾目標時，改 `ROLLBACK_REF`。

## 加段落的方式

1. 寫一個 `async function segXxx()`，用 `withCockpit(label, opts, body)` 開一組 preview＋chrome。`opts.env` 覆寫
   `ui_preview` 的環境變數，`opts.windowSize` 改視窗大小（例如窄視窗 `'700,900'`）。
2. 操作用 `openTree()`、`openFile()`、`clickTab()`（後兩者可帶 `{ modifiers: MOD.CTRL }`），斷言用 `check()`，
   前置條件用 `need()`。`clickTab()` 帶 `{ expectSelected: false }` 時點完就返回，不等選定（task 2.3 起用於連點；
   點擊處理比回覆晚幾毫秒，之後的斷言要自己等可觀察的結果，見「量測方式」）。
3. 在檔尾 `SEGMENTS` 加一列 `{ code: 'file-review/<scenario 名稱>', fn: segXxx }`，並更新本檔的段落代號對照表。

## 輸出格式

- `ok  <說明>`／`FAIL <說明>`：每個斷言一行。前置條件不成立時記一條 `FAIL` 並中止該段，不再產生連帶失敗。
- `[<timestamp>] <說明>`：過程訊息，例如段落開頭 `=== <代號> ===`、觀察窗內的查詢分組。
- 結尾 `=== 段落彙總 ===`：每段一行 `PASS` 或 `FAIL(n) — <第一條失敗>`，另有一行 `[收尾衛生]`，最後
  `RESULT: PASS` 或 `RESULT: FAIL (n)`（exit 2）。

## 已知的偶發失敗

- **收尾的 PID 誤報（PID 重用）**：收尾的「本腳本 spawn 過的行程都已不存在」只用 `tasklist /FI "PID eq <PID>"` 判斷，沒有核對
  行程建立時間。Windows 可能在段落結束後把同一個 PID 配給別的行程（task 3.4 實測過：重新分配到使用者桌面 Chrome 的
  renderer），就會多一條 `FAIL`，訊息形如 `chrome-<段名> PID N 已終止（tasklist 查無此 PID）`或「殘留 PID」。
  判別方式：腳本在段落收尾時已觀察到該行程的 exit 事件；首次出現時要用 `Get-CimInstance Win32_Process` 查那個 PID 的建立
  時間來確認，若建立時間遠晚於該段結束、或是使用者自己的行程，才是誤報。**不要砍那個行程**（不是本腳本開的），確認後
  重跑一次即可；若重跑仍殘留自己開的 Chrome（user-data-dir 含 `cockpit-chrome-splitcheck-`）才是真的殘留，要深查。
  **無法確認是 PID 重用時，不可當偶發處理**：`sdd-ledger.md` 另有一筆 `chrome-P1` 收尾失敗，原因未查明，同樣不可略過。
- **計時斷言在高負載下偶發**：自動更新的次數門檻已刻意放寬（見「量測方式」的基準段說明），但與其他驗收腳本或 `cargo`
  同時跑時仍可能邊界失敗（尚無實測紀錄，是依腳本註解與其他腳本的撞埠紀錄推測）。前景單獨重跑一次；連續失敗就不是偶發。
- 其他驗收腳本的已知偶發（例如 `live-output-check.js` 的 R 段、`actions-check.js` 的重複 POST）與本腳本無關，記在
  本 change 封存目錄（`openspec/changes/archive/` 下的 `file-split-view`）的 `sdd-ledger.md`「各 task 審查結論與裁決」
  一節的偶發失敗紀錄。

## 收尾判準

- 每段結束時依 PID 終止本段開的 Chrome 與 `ui_preview`（`taskkill /T /F`），確認觀察到 exit、埠不再 LISTEN、
  暫存目錄已刪。
- 全部結束後：本次用過的每個埠都沒有 LISTEN；本次 spawn 過的 PID 都已不存在；沒有 user-data-dir 含
  `cockpit-chrome-splitcheck-` 的 Chrome；本次開過的 `ui_preview` 暫存目錄都已刪除。
- 只終止本腳本自己 spawn、還握著 ChildProcess 且沒觀察到 exit 的行程，不碰其他 `ui_preview.exe` 或 Chrome。
