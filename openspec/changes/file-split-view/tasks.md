# tasks：file-split-view

> 執行路徑：SDD｜理由：前端的分頁框架要從「單一目前分頁」改成「可見集合」，牽動生命週期、輪詢、持久化、鍵盤與版面五塊，
> 各自可以獨立驗收。改壞的話會影響既有的檔案分頁、git 分頁與 Live Output，返工成本高（design D2、Risks「漏掉某個依賴
> 目前分頁的舊判斷」）。
>
> **審查**：依專案 memory `opus-review-counts-as-codex-review`，由 Opus 5.5 subagent 審查 diff，視同 Codex review。分兩批：
> 第 3 節完成後審一次（3.7），收尾前審整支分支（5.2）。findings 與處理方式記在 `sdd-ledger.md`。

**工作樹**：一律在 worktree `D:\projects\ai-cockpit-split`（分支 `feat/file-split-view`）。不碰 `D:\projects\ai-cockpit`，
那裡另有 session 在做 App icon。

通則（每個 task 都適用）：

- **品質 gate**：
  - 本 change 不改 Rust，每個 task 結尾仍跑 `cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、
    `cargo test --workspace`，確認沒有連帶壞掉。
  - 有改 `cockpit/examples/` 時加跑 `cargo test -p cockpit --example ui_preview`。
  - 有改 `.md` 時在 repo 根跑 `markdownlint-cli2 "**/*.md"`，並核對 `Linting: N files` 的 N 不為 0。
  - 有改 `openspec/` 時跑 `openspec validate --all`。
  - gate 要單獨跑、看結束碼，不接管線（專案 memory `gate-exit-code-swallowed-by-pipe`）。輸出貼進回報。
- **跑腳本前先重建**：前端資源內嵌在執行檔裡，改完 `cockpit/assets/` 一定先 `cargo build -p cockpit --example ui_preview`
  再跑任何腳本（專案 memory `acceptance-scripts-run-stale-embedded-assets`）。
- **「既有腳本」**＝1.1 寫進 `sdd-ledger.md` 的清單。
  - 每個改前端的 task 結束時，既有腳本必須全綠。
  - 只能改被本 task 打壞、而且 spec 已改變的斷言，不得放寬 spec 沒有改變的斷言。
  - 腳本修改與產品修改分開 commit，commit 訊息逐條列出被改的斷言與對應的 spec scenario。
  - 腳本不可並行跑，一律前景跑。
- **新斷言集中在新腳本** `docs/research/2026-10-04/split-check.js`，附用法說明 `split-check.md`。寫法比照
  `docs/research/2026-09-27/files-check.js`：headless Chrome＋raw CDP，用 `Input.dispatchMouseEvent`／`Input.dispatchKeyEvent`
  產生真實輸入（含 Ctrl 修飾鍵）。由 2.1 建立，後續 task 擴充。新斷言先在未修的程式上跑出紅，再修。
- **port 7770**：跑腳本前確認 7770 沒有人在用。另一個 session 也可能在跑 `ui_preview`，撞 port 時先回報，不搶。
  **不是自己開的程序只能回報、不准砍**。跑完依 PID 收尾，確認 7770 與 CDP port 都沒有 LISTEN。
- **介面文字**：新增的文字一律放進 `cockpit/assets/app/i18n.js` 的中英兩份字典，不寫死在 JS 裡（`ui-language`）。
- **註解與勾選**：程式碼註解若要引用 task，寫成 `file-split-view task N.M`。`tasks.md` 由控制端統一勾。
- **提交**：多個 subagent 不同時改同一個工作樹；commit 只 `git add <具體路徑>`。

## 1. 基線

- [x] 1.1 記錄基線與既有腳本清單。
  - 腳本清單：以 `openspec/changes/archive/2026-10-02-live-output-color/tasks.md` 的「既有腳本」為底，加上之後的 change
    新增、以 `ui_preview` 為對象的腳本。候選有 `docs/research/2026-10-02/` 與 `2026-10-03/` 的 `*-check.js`，逐一讀檔頭
    確認對象。啟動器或真機用的腳本不列入，並註明理由。
  - 在 `feat/file-split-view` 跑 `cargo test --workspace`、`cargo test -p cockpit --example ui_preview` 與清單中的全部腳本。
  - 把清單、passed／failed／ignored 數字與各腳本結果寫進 `openspec/changes/file-split-view/sdd-ledger.md`。
  - 驗收＝ledger 有清單與數字，failed 為 0、腳本全綠。已知 flaky 的項目重跑一次並記錄。

## 2. 分頁框架：可見集合（`files.js`，行為不變的重構）

- [x] 2.1 盤點舊判斷，並建立 `split-check.js` 骨架。
  - 盤點：grep `files.js` 裡所有 `currentTab()`、`currentReviewTabId`、`panel.hidden` 的用法，逐一判斷原意是「焦點」還是
    「可見」，清單寫進 ledger（design Risks）。
  - 骨架：建立 `split-check.js` 與 `split-check.md`，含啟動、收尾、共用工具，以及一段「沒有並排時的基準」斷言：開兩個檔案
    分頁時只有目前分頁可見，只有它發出中繼資料查詢。
  - 驗收＝ledger 有盤點清單；新腳本在未改的程式上全綠（這段是基準，本來就該綠）；既有腳本全綠。
- [x] 2.2 引入 `splitTabs`、`splitFocus`、`visibleTabs()`、`applyVisibility()`（design D1、D2）。
  - `selectTab()` 改成「先改狀態、再 `applyVisibility()`」。
  - `kind.activate`／`deactivate` 改由可見集合的比對觸發。
  - 2.1 盤點出、原意是「可見」的判斷改用 `isVisible(ft)`。
  - 這個 task 還不開放任何加入並排的入口，`splitTabs` 恆為空。
  - 驗收＝既有腳本全綠（證明沒有並排時行為不變）；`split-check.js` 基準段仍綠。
- [x] 2.3 檔案輪詢改為每個分頁各一條（design D3；spec「自動更新」）。
  - 單例 `poll` 搬進 `ft.poll`，`pollMeta(ft)` 的繼續條件改成 `isVisible(ft)`，每個分頁各自用 `gen` 丟棄舊回應。
  - `split-check.js` 加斷言：快速在 A、B 兩個檔案分頁間切換時，A 的舊回應不會蓋到 B；關閉分頁後，它進行中的查詢會被中止。
  - 驗收＝新斷言綠；既有腳本全綠，`files-check.js` 的「Live Output 分頁時不查詢」等自動更新斷言不得放寬。

## 3. 並排功能

- [x] 3.1 並排的狀態轉換（design D1、D9；spec「檔案並排」的加入、替換、移出、非檔案分頁、3 欄已滿）。
  - 實作加入、替換焦點欄、移出、關閉並排中分頁時的焦點規則，以及選定非檔案分頁時保留並排組合、選回時整組恢復。
  - 入口驗 kind，只收檔案分頁。
  - 這個 task 先用最簡的版面：並排面板同時解除 `hidden`，排版可以暫時不正確。
  - `openFile()` 的 error 重試條件改成「已經可見」（design D2）。
  - `split-check.js` 加斷言，對應 spec scenario：
    - 「加入並排」「替換焦點欄不影響其他欄」「從檔案樹開檔替換焦點欄」「三欄已滿時替換焦點欄」。
    - 「移出焦點欄」「關閉後只剩一個時解除並排」「不在並排中時關閉並排組合的成員」「不在並排中時加入已滿的並排組合」。
    - 「切到 Live Output 後整組恢復」「非檔案分頁不能並排」「並排中的非焦點欄也更新」。
    - 「替換焦點欄不影響其他欄」與「切到 Live Output 後整組恢復」要實際量 `scrollTop`，不能只驗欄位排列。另外驗證其他欄
      的內容沒有被重新讀取：前後沒有對該檔案的內容請求。
    - 點在並排中、可見但不是焦點欄的 error 欄，會立即重試讀取。
  - 驗收＝新斷言先紅後綠，既有腳本全綠。
- [x] 3.2 版面（design D4；spec「三欄並排不撐破頁面」）。
  - `#review[data-split]` 的 grid、面板 `order`、焦點欄 `data-split-focus` 外框，只用既有 token。
  - 不得搬動任何面板的 DOM。
  - 如有需要，在 `ui_preview` 的 fixture 補一個含 300 個字元長行的文字檔，並同步更新會列出 fixture 檔案的既有斷言（依通則）。
  - `split-check.js` 加斷言：
    - 1280 寬下兩欄、三欄等寬（各欄寬差不超過 1px）。
    - 頁面沒有橫向捲軸、中欄寬度與沒有並排時相同。
    - html 檢視器的 iframe 在並排切換前後是同一個節點，沒有重新載入。
    - PDF 欄寬改變後仍為符合寬度。
    - spec「整頁重畫不影響並排」「切換 Project 不影響並排」：並排狀態、焦點欄、捲動位置不變，面板節點沒有被換掉。
  - 驗收＝新斷言綠，既有腳本全綠。
- [x] 3.3 並排鈕、Ctrl＋點選、Ctrl＋Enter、欄位標記與無障礙（design D7；spec 的「Ctrl＋點選加入並排」「鍵盤加入並排」
  「沒有另一個檔案分頁時不並排」「並排鈕停用時 Ctrl＋點選等同一般選定」）。
  - `wireTablist()` 把 event 傳給 `onActivate(tab, event)`，左欄呼叫端不受影響。Ctrl＋Enter 的 keydown 要 `preventDefault()`。
  - 並排鈕的 `aria-pressed`、`aria-disabled`＋`title`，以及 tabindex 規則（只有目前分頁的並排鈕在 Tab 順序中）。
  - `split-check.js` 另外驗證三件事：Live Output 與 git 類分頁上沒有並排鈕；非目前分頁的並排鈕只在滑鼠移上去時顯示；
    Ctrl＋Enter 只觸發一次動作（沒有多跑一次一般選定）。
  - 欄位編號徽章，加上用 `aria-describedby` 連到分頁按鈕的「並排第 N 欄」。
  - 文字中英兩份都放進 `i18n.js`。
  - `split-check.js` 涵蓋上述 scenario，並驗證切換語言後並排鈕的 `title` 與欄位說明會跟著換。
  - 驗收＝新斷言先紅後綠，既有腳本全綠（含 i18n 相關腳本）。
- [x] 3.4 焦點欄切換（design D6；spec「在欄內點選切換焦點欄」）。
  - 在 `#review` 以 capture 階段監聽 `pointerdown`，不呼叫 `focus()`、不 `preventDefault()`。
  - `split-check.js` 加斷言：
    - 在非焦點欄的內容上按下滑鼠，該欄成為焦點欄，兩欄都沒有重新讀取內容。
    - 在非焦點欄點 md 相對連結，目標檔案在那一欄開啟。
    - 在非焦點的 PDF 欄點「下一頁」，翻頁照常生效。
    - 滑鼠點完之後沒有出現 `:focus-visible` 外框。
  - 驗收＝新斷言先紅後綠，既有腳本全綠。
- [x] 3.5 窄視窗（design D5；spec「窄視窗只顯示焦點欄」「窄視窗下仍替換焦點欄」「只查詢可見的檔案分頁」）。
  - 用 `matchMedia("(min-width: 760px)")` 觸發 `applyVisibility()`。
  - `split-check.js` 以 CDP `Emulation.setDeviceMetricsOverride` 在 1280 與 700 之間切換，斷言兩件事：顯示的欄數；
    服務實際收到的中繼資料查詢只涵蓋可見的分頁。
  - 驗收＝新斷言先紅後綠，既有腳本全綠。
- [x] 3.6 持久化（design D8；spec「分頁還原」的「還原並排」「選定 Live Output 時重新整理」「並排資料不合法時忽略」
  「舊格式照常還原」）。
  - `persistTabs()` 寫入可選的 `split`／`splitFocus` 欄位，`restoreTabs()` 驗證並還原。索引一律指儲存的 `tabs` 陣列位置，
    還原時建「儲存位置 → 還原出的分頁」對照表（design D8）。
  - `split-check.js` 加斷言：
    - 上列 4 個 scenario。「並排資料不合法時忽略」要逐一涵蓋 spec 列出的五種情況，其中「指到一筆被略過的分頁」要確認
      並排沒有因位移而指到別的分頁。
    - 焦點欄不在並排組合中時改用第一欄。
    - 寫入帶 `split` 的資料後，用 `main` 版本的 `isStoredState()` 檢查（從 `git show main:cockpit/assets/app/files.js`
      取出該函式，在頁面中求值），確認它仍回傳 true，回滾時分頁不會遺失。
      （註：實作改為比對固定的 tag `v0.1.2`，因為 squash 併回 `main` 之後 `main` 已含並排，理由見 design Migration Plan
      與 `split-check.md`「回滾相容」。）
  - 驗收＝新斷言先紅後綠，既有腳本全綠。
- [x] 3.7 Opus 審查（前端）：派 Opus 5.5 subagent 審 `main..feat/file-split-view` 的 diff，focus 為 `cockpit/assets/`、
  `split-check.js` 與 fixture。findings 依 `superpowers:receiving-code-review` 處理：先實測重現，才採信並修改。
  驗收＝ledger 記錄結論與每個 finding 的處理。

## 4. 設計審核與文件

- [x] 4.1 設計審核（專案 memory `frontend-appearance-reviewed-by-frontend-design-skill`）。
  - 用 `ui_preview` 拍 1536、1100、700 三種寬度下單欄、兩欄、三欄的截圖，對照 `docs/direction-01-visual-design.md`，
    並過 frontend-design 審核。
  - 拍截圖前確認畫面內沒有使用者名稱（專案 memory `deidentification-must-inspect-images-not-just-grep`）。截圖放在 repo 外，
    要進 repo 的才逐張檢查。
  - 會動到設計文件的建議交給使用者決定。
  - 驗收＝審核 findings 已處理或已記錄由使用者決定，ledger 有截圖路徑。
- [x] 4.2 文件。
  - `CONTEXT.md` 新增「檔案並排」「焦點欄」詞條，並註明與 HERDR 的 Tab 無關，寫法同「檔案分頁」。
  - `cockpit/README.md` 補並排的使用說明。
  - `split-check.md` 補齊用法。
  - 驗收＝`markdownlint-cli2 "**/*.md"` 0 error（`Linting: N files` 的 N 不為 0）。

## 5. 收尾

- [x] 5.1 全 gate 與全部腳本，以下各指令單獨跑、逐一看結束碼：
  - `cargo fmt --check`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo test --workspace`
  - `cargo test -p cockpit --example ui_preview`
  - `markdownlint-cli2 "**/*.md"`
  - `openspec validate --all`
  - 既有腳本加上 `split-check.js`

  驗收＝輸出貼進 ledger，全綠。
- [x] 5.2 Opus 審查（整支分支）：`main..feat/file-split-view` 全部 diff，處理方式同 3.7。驗收＝ledger 記錄結論段與每個 finding
  的處理。
- [x] 5.3 真機驗收（由使用者在實機 Chrome 確認）。
  - 用臨時設定檔（放在 repo 外）啟動 Cockpit，接 Windows 端 HERDR，在高解析大螢幕上確認：
    - 兩欄、三欄並排閱讀 md。
    - agent 改檔時非焦點欄會更新。
    - 重開後並排組合還原。
  - 結果寫進 `docs/research/2026-10-04/split-live.md`，截圖不進 repo。
  - 驗收＝該檔存在、markdownlint 0 error。
- [ ] 5.4 跨 session 協調與收尾。
  - 確認 `main` 目前的位置：「自動更新管理」session 的 App icon 若已先併回，先把本分支 rebase 到最新 `main` 並重跑 5.1。
  - squash 併回 `main`、archive change、重寫 `docs/handover.md`（依 `~/.claude/guides/handover-template.md`）。
    （註：依使用者指示，`docs/handover.md` 由「自動更新管理」session 在兩個 change 都發版後統一重寫；本 change 收尾改為
    以 SendMessage 回報。）
  - 用 send_message 通知「自動更新管理」session：本 change 改了哪些檔案（尤其 `style.css`、`index.html`、`i18n.js`），
    以及它的分支併回前要先 rebase。
  - 移除 worktree，刪除本地分支。
  - 驗收＝`openspec validate --all` 通過；`git status` 乾淨；`git worktree list` 不再有 `ai-cockpit-split`；
    已送出通知訊息。
