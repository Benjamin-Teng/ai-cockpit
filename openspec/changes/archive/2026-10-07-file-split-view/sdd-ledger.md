# SDD ledger：file-split-view

- plan：`openspec/changes/file-split-view/tasks.md`；spec：同目錄 `specs/`、`design.md`
- 分支：`feat/file-split-view`。數字量於 `d17ded7`（plan commit，當時位於 origin/main `8afd835` 之上）；之後分支只 rebase 到 `7f6ce36`（plan commit 因此改為 `7c2ddd7`），
  差異僅 `CHANGELOG.md` 一行，沒有重量。
- 工作區（git-ignored）：`.superpowers/sdd/`（briefs、reports、進度紀錄），收尾時隨 worktree 刪除；需要留存的審查結論與
  裁決收在 §4

## 1. 基線（task 1.1）

環境：Rust 1.97.1（`rust-toolchain.toml`）、Node v22.19.0、Windows。worktree 的 `target/` 是新建的，
腳本跑的是本 worktree 建出的 `target/debug/examples/ui_preview.exe`。

| 指令 | 結果 |
|---|---|
| `cargo test --workspace` | exit 0，68 個結果列合計 passed 1304、failed 0、ignored 13（含 doc-tests） |
| `cargo test -p cockpit --example ui_preview` | exit 0，62 passed、0 failed、0 ignored |

### 既有腳本清單（共 14 支，每個改前端的 task 結束時全部必須全綠）

所有腳本都以 `path.resolve(__dirname, '..', '..', '..')` 推算 repo 根，沒有寫死絕對路徑。

| # | 腳本 | 結果 | 備註 |
|---|---|---|---|
| 1 | `docs/research/2026-09-15/reconnect-check.js` | exit 0，ok 14、FAIL 0 | 約 13 秒 |
| 2 | `docs/research/2026-09-15/whatever-check.js` | exit 0，ok 9、FAIL 0 | 約 3 秒 |
| 3 | `docs/research/2026-09-16/actions-check.js` | exit 0，ok 86、FAIL 0 | 約 19 秒 |
| 4 | `docs/research/2026-09-16/channel-backoff-check.js` | exit 0，ok 19、FAIL 0 | 約 49 秒；讀內嵌的 `channel.js`，自帶假 WS server |
| 5 | `docs/research/2026-09-16/factory-floor-check.js` | exit 0，ok 131、FAIL 0 | 覆寫 `task-5.2-scenario-d.png`，跑完 `git checkout --` 還原 |
| 6 | `docs/research/2026-09-19/live-output-check.js` | exit 0，ok 345、FAIL 0 | 約 214 秒；R 段沒有 FAIL |
| 7 | `docs/research/2026-09-23/visual-check.js` | exit 0，ok 1757、FAIL 0 | 約 241 秒；腳本結尾摘要為既有段落 FAIL 0、FT1–FT3 FAIL 0、收尾 FAIL 0（FT 段說明見下表後） |
| 8 | `docs/research/2026-09-27/files-check.js` | exit 0，ok 578、FAIL 0 | 約 191 秒 |
| 9 | `docs/research/2026-09-28/git-check.js` | exit 0，ok 770、FAIL 0 | 約 219 秒 |
| 10 | `docs/research/2026-10-01/progress-check.js` | exit 0，ok 61、FAIL 0 | 約 8 秒 |
| 11 | `docs/research/2026-10-01/ui-fixes-check.js` | exit 0，ok 349、FAIL 0 | 約 27 秒 |
| 12 | `docs/research/2026-10-02/notify-check.js` | exit 0，PASS 253（新行為 191＋前置 62）、FAIL 0 | 約 78 秒 |
| 13 | `docs/research/2026-10-02/output-color-check.js` | exit 0，PASS 89、FAIL 0 | 約 45 秒 |
| 14 | `docs/research/2026-10-03/i18n-check.js` | exit 0，PASS 1460（新行為 948＋前置 512）、FAIL 0 | 約 139 秒；預設含第 5 段，會重拍 `i18n-en-*.png`，跑完 `git checkout -- docs/research/2026-10-03` 還原 |

visual-check 的 FT1–FT3 是 file-review task 3.5 加入的段落（檔案分頁相關）。腳本輸出與註解仍寫「4.x 前端落地前預期 RED」，這句已過期：現在 file-review 已落地，三段實際為綠，不是預期紅。

跑完後以 `git status` 檢查：被覆寫的只有 `task-5.2-scenario-d.png` 與 `i18n-en-*.png`，已還原；7770、7830 起的埠段與各 CDP port 都沒有 LISTEN，沒有殘留 `ui_preview`。

### 不列入的腳本與理由

| 腳本 | 理由 |
|---|---|
| `docs/research/2026-09-19/live-output-real-check.js` | 對真的 WSL 測試 server 與 `cockpit.exe`，真機腳本 |
| `docs/research/2026-10-02/idle-exit-check.js` | 測 `cockpit.exe --exit-when-idle` 的啟動器行為，不是 `ui_preview` |
| `docs/research/2026-10-02/deid-check.js` | 去識別化掃描，不是前端斷言 |
| `docs/research/2026-10-03/download-section-check.js` | 對象是 `site/` 宣傳頁，不是 `ui_preview` |
| `docs/research/2026-10-01/progress-screenshots.js` | 只產生截圖，沒有斷言 |
| `docs/research/2026-10-04/fake-update-server.js` | 自動更新端到端真機用的假伺服器 |
| `packaging/icon/gen-icon.js` | 產生圖示檔，不是驗收 |
| `packaging/smoke-test.ps1`、`scripts/install-desktop.ps1`、`docs/assets/gen-readme-banner.js` | 安裝與打包用，與 `ui_preview` 無關 |

### flaky 與重跑紀錄

- 本次基線所有腳本一次跑完全綠，沒有需要重跑的項目。
- 在這之前，同一批腳本在 rebase 之前的起點（`1719371`）跑過第一輪（後來因讓出 7770 而暫停）：
  - `visual-check.js` 當時 FAIL 4 行，彙總為 P1、FT2：
    - `port 7830（preview-FT2）應該不再有 LISTENING 的行程`：當時另一個 session 正在同時跑 `visual-check.js`，其 `ui_preview` 佔著 7830（與 FT2 同埠），推定為撞埠。
    - `chrome-P1 PID 36024 已終止（tasklist 查無此 PID）`：這是 Chrome 的收尾檢查，與 7830 無關，原因**未查明**。
    - 其餘行為斷言全過（該輪 ok 1754）。舊的失敗輸出只留在 session 暫存、未進 repo。
  - 跑腳本期間曾有人啟動已安裝版 `cockpit.exe` 佔用 7770，不是本 session 的程序，沒有處理，等其退出後才續跑。
- 本次基線（`d17ded7`）的 visual-check 沒有再出現這 4 行。但兩輪的程式碼不同，而且只多跑了一次，這連「同一份程式碼上
  未重現」都算不上，不能證明 P1 是撞埠造成：列為疑似偶發，之後 `chrome-P1` 再出現就要深查，不可當 flaky 略過。
  後續各 task 的追蹤見 §4.5。
- `live-output-check.js` R 段已知偶發競態（兩次分開的 `eval` 間節點被換掉）：本次基線沒有觸發。

### 其他基準（供後續比對）

- 本 task 沒有改任何 Rust 或產品程式碼，所以沒有跑 `cargo fmt --check` 與 `cargo clippy`；上面的 `cargo test` 數字即 Rust 側基線。
- `markdownlint-cli2 "**/*.md"`：`Linting: 202 files`、0 issues（量於加入本檔之後）。
- `openspec validate --all`：`Totals: 26 passed, 0 failed (26 items)`。

## 2. 舊判斷盤點（task 2.1）

依 design Risks「漏掉某個依賴目前分頁的舊判斷」：grep `cockpit/assets/app/files.js` 的 `currentTab()`、`currentReviewTabId`、
`panel.hidden`（含其他以 `.hidden` 判斷或設定可見的寫法），逐處判斷原意是「焦點」（目前分頁）還是「可見」（畫面上看得到）。
行號量於 `7b7af65`，實作時以函式名為準。

### 分頁框架（`currentTab()`／`currentReviewTabId`／tabpanel 的 `hidden`）

| # | 位置 | 寫法 | 原意 | 理由與第 2 節的處理 |
|---|---|---|---|---|
| 1 | `files.js:116` | `var currentReviewTabId = LIVE_TAB_ID` | 焦點 | 宣告；design D1 保留原意，另加 `splitTabs`／`splitFocus` |
| 2 | `files.js:981` `currentTab()` | `tabById(currentReviewTabId)` | 焦點 | 定義本身是「目前分頁」；唯一的呼叫端是 #8，改掉 #8 後沒有呼叫端，可刪 |
| 3 | `files.js:1021` `openTab()` | `tab.els.tab.id !== currentReviewTabId` 才 `selectTab()` | 焦點 | 「切過去成為目前分頁」，已是目前分頁就不重選；git 類 kind 不進並排，語意不變 |
| 4 | `files.js:1103` `createFileTab()` | `panel.hidden = true` | 可見 | 新面板一開始不可見，等進入可見集合才由 `applyVisibility()` 清掉；保留 |
| 5 | `files.js:1139` 內容區 scroll listener | `if (!panel.hidden) captureFileScroll(ft)` | 可見 | 只有可見時 `scrollTop` 可信；並排的非焦點欄也要記捲動。只要 `applyVisibility()` 是 tabpanel `hidden` 的唯一寫入者、維持「`hidden` ⇔ 不在可見集合」，寫法可保留；也可改 `isVisible(ft)` |
| 6 | `files.js:1165`、`1172` `FILE_KIND.activate`／`deactivate` | `startPolling(tab)`／`stopPolling()`（全域單例） | 可見 | design D2 把 activate／deactivate 的意思改成「變成可見／不可見」；D3 改成 `ft.poll` 每分頁一條，`deactivate` 只停自己那條 |
| 7 | `files.js:1254` `startPolling()` | 先 `stopPolling()` 再查 | 可見 | 「同時只有一條鏈」的前提來自「只有一個可見檔案分頁」；D3 改為只重啟該分頁自己的鏈，不得停到別欄 |
| 8 | `files.js:1260` `pollMeta()` | `currentTab() !== ft` 就停 | 可見 | spec「自動更新」：每個可見的檔案分頁各自查詢。並排中的非焦點欄用這個條件會停掉，必須改成 `isVisible(ft)`（D3） |
| 9 | `files.js:1393` `applyPendingAnchor()` | `ft.els.panel.hidden` 就不捲 | 可見 | 捲到錨點需要版面；在非焦點欄點 md 連結（D6 先 pointerdown 改焦點欄再開檔）仍要可捲。理由同 #5，可保留或改 `isVisible(ft)` |
| 10 | `files.js:1434` `selectTab()` 開頭 | `tabEl.id === currentReviewTabId` 就 return | 焦點 | 重選目前分頁不做事。並排中點非焦點欄的分頁是焦點變化，照常往下走 |
| 11 | `files.js:1438`–`1445` `selectTab()` | `leaving = tabById(currentReviewTabId)`，對它呼叫 `deactivate` | 可見 | 原意是「離開畫面的分頁收掉資源」，只是過去離開焦點＝離開畫面。並排中切焦點欄時離開焦點的分頁仍可見，不可 deactivate；改由 `applyVisibility()` 比對新舊集合（D2） |
| 12 | `files.js:1451`–`1453` `selectTab()` 的 `apply` | `panel.hidden = tabs[i] !== tabEl` | 可見 | 只讓目前分頁的面板可見；改成依可見集合設定（D2 第 2、3 步），含 Live Output 的面板 |
| 13 | `files.js:1449`、`1458`–`1462` `selectTab()` 的 `apply` | `markSelected()`（`aria-selected`）、`.is-current`、關閉鈕 `tabIndex` | 焦點 | ARIA 選定、目前分頁外觀與 roving tabindex 都只對一個目前分頁；保留（D1） |
| 14 | `files.js:1464`–`1465`、`1467`–`1474` `selectTab()` | `fromLive`／`toLive`（比 `currentReviewTabId` 與 `LIVE_TAB_ID`）決定 `liveOutput.tabHidden(apply)`／`tabShown(apply)` | 可見 | 這是 Live Output 面板的進出畫面通知。Live Output 不進並排，所以它「可見」恰等於「是目前分頁」，判斷可改成「Live Output 是否進出可見集合」，但包裝與呼叫順序要照舊（D2「Live Output 的回呼包裝」） |
| 15 | `files.js:1466` `selectTab()` | `currentReviewTabId = tabEl.id` | 焦點 | 改目前分頁；D2 拆成「先依並排規則改狀態，再 `applyVisibility()`」 |
| 16 | `files.js:1475`–`1480` `selectTab()` | 對 `entering` 呼叫 `activate` | 可見 | 原意是「進入畫面的分頁開始輪詢」；從 Live Output 回到並排組合時會有 2～3 個分頁同時進入，改由 `applyVisibility()` 處理（D2） |
| 17 | `files.js:1498`–`1500` `openFile()` | `ft.els.tab.id === currentReviewTabId` 時不 `selectTab()`；其中 `ft.status === "error"` 就 `startPolling(ft)` 立即重試 | 焦點＋可見（要拆） | 「是否要 `selectTab()`」是焦點；「error 時立即重試」是可見（design D2）：條件要改成「呼叫前就已可見而且處於 error」，否則並排中點一個可見但不是焦點欄的 error 欄，只會切焦點、不會重試。呼叫前不可見的分頁不必重試，`selectTab()` 讓它進入可見集合時 `activate` 會立即查一次。重試只能重啟該分頁自己的輪詢（D3） |
| 18 | `files.js:1520` `closeTab()` | `tab.els.tab.id === currentReviewTabId` 時 `selectTab(neighbor)` | 焦點（隱含可見） | 關閉目前分頁時移交焦點，同時靠 `selectTab()` 裡的 `deactivate` 停掉它的輪詢。並排中關閉一個不是焦點欄的可見分頁時不會走這條，舊程式碼不會 deactivate；舊的 `dispose` 只設 `closed`、加 `gen`，不清輪詢計時器（過去不需要：非目前分頁不會在輪詢）。第 2 節要讓 `dispose`（或 `closeTab` 在 `applyVisibility()` 之後）停掉該分頁自己的 `ft.poll`，再依 D9 重算焦點與可見集合 |
| 19 | `files.js:1586` `persistTabs()` | `current: tabById(currentReviewTabId)` | 焦點 | 持久化的 `current` 是目前分頁；並排另存 `split`／`splitFocus`（D8） |
| 20 | `files.js:1761`–`1763` `restoreTabs()` | `selectTab(cur.els.tab)` | 焦點 | 還原目前分頁。D8 要在這之前還原 `splitTabs`／`splitFocus`，讓這次 `selectTab()` 一次算出完整的可見集合 |
| 21 | `files.js:1716`–`1718` `restoreTabs()` 上方註解 | 「其餘分頁在第一次成為目前分頁時才查」 | 可見 | 註解措辭；改成「第一次變成可見時才查」 |

### 與分頁可見無關的 `.hidden`（不需處理）

| 位置 | 寫法 | 理由 |
|---|---|---|
| `files.js:330`、`354`、`361`、`366`、`625`、`635`、`643`、`650`–`652`、`671` | `treeHead`／`treeStatus`／`treeEl`／`filesEmpty`／`filesPanel` 的 `hidden` | 左欄檔案樹與它所在的「檔案」左欄分頁，與分頁區無關 |
| `files.js:819`、`822`、`848` `setLeftTab()` | `filesPanel`／`changesPanel`／`projects` 的 `hidden` | 左欄三個分頁的切換 |
| `files.js:1117`、`1123`、`1131`、`1224` `setHidden()` | 檔案面板內 `staleLabel`／`vscode`／`status` 的 `hidden` | 面板內子元素的顯示與否，不代表分頁是否可見 |

### `files.js` 以外的相關前提（第 2 節不改，記錄供審查對照）

- `git.js:773`、`2556`、`3004` 的 scroll listener，`git.js:1874` `isGraphTabActive()`，`git.js:2724` 的錨點判斷，都以
  `panel.hidden` 判斷 git 類分頁是否可見。git 類分頁不進並排組合，對它們來說「可見」等於「目前分頁」，所以這些寫法
  在並排後仍成立（design Non-Goals）。前提是 `splitTabs` 只收檔案 kind（design Risks「git.js 的隱含前提」）。
- `files.js:1537` `wireTablist(reviewTablist, selectTab)`：`onActivate` 拿不到 event，D7 的 Ctrl＋點選要讓 `wireTablist()`
  把 event 傳成第二個參數（左欄的呼叫端忽略）。

## 3. 輪詢搬移的負向對照（task 2.3）

split-check 的段落至今只在正確的程式上跑過綠。task 2.3 把輪詢搬進 `ft.poll` 之後，在本機暫時改壞 `files.js`、重建
`ui_preview`、跑指定段落，確認斷言會變紅，再還原（與改壞前的檔案 `cmp` 相同，沒有 commit）。

| 代號 | 暫時的改法 | 跑的段落 | 結果 |
|---|---|---|---|
| NC-1 | `FILE_KIND.deactivate` 不呼叫 `stopPolling(tab)` | `baseline/`、`file-review/` | 基準段**仍綠**；快速切換段 FAIL 4；關閉段綠 |
| NC-1b | NC-1，再把 `pollMeta()` 開頭的繼續條件拿掉 `!isVisible(ft)` | `baseline/` | 基準段 FAIL 4 |
| NC-2 | `deactivate` 與 `dispose` 都不呼叫 `stopPolling(tab)` | `file-review/關閉分頁中止進行中的查詢` | FAIL 2 |

- **NC-1 的基準段為什麼仍綠**：`pollMeta()` 每一輪開頭都檢查 `isVisible(ft)`（task 2.2 起）。deactivate 不停輪詢時，
  被切走的分頁排好的下一輪一觸發就因為不可見而結束，最多只有切換當下已在半路的那一筆會回來。基準段先等 0.5 秒再開
  6 秒觀察窗，那一筆不會落在窗內，所以看不到。換句話說，基準段擋的是「不可見還繼續查」，擋不到「deactivate 沒有
  中止進行中的請求」。這兩層防線要同時失效（NC-1b）基準段才會紅：輸出為兩個方向各有
  `不可見的 README.md／docs/a.md 沒有被查詢（實際 3 次）` 與 `窗內只有 … 的中繼資料查詢` 共 4 條 FAIL。
- **NC-1 由快速切換段擋下**：最後一下切走 A 時 A 的查詢沒有被中止，放行的 404 被套用到 A，FAIL 為
  `docs/a.md（不可見）沒有過期標示`、`docs/a.md（不可見）的狀態列隱藏（實際顯示 "檔案已不存在"）`、
  `攔下的第 2 筆 … 在放行前就被頁面中止（實際 failed:false, status:404）`、`… 放行時 Chrome 回報已不存在（實際 delivered）`。
- **NC-1 下關閉段仍綠**是預期的：task 2.3 讓 `dispose` 自己停 `ft.poll`，不依賴 deactivate。NC-2 再拿掉 dispose 的
  停止才紅：`關閉後 1 秒內，卡住的 docs/a.md 查詢被頁面中止（實際 null）`、`… 放行時 Chrome 回報已不存在（實際 delivered）`。
- **兩個新段落在搬移前的程式上就是綠的**（連跑 5 次全綠）：舊的單例輪詢在 deactivate 時以全域 `poll.gen` 丟棄舊回應並
  abort，`startPolling()` 也會先停掉全域唯一的鏈，沒有並排時兩條防線都成立。單例會出錯的情境（兩個分頁同時可見時
  互相覆寫 controller／timer、重試停掉別欄）要到第 3 節有並排入口才觀察得到，本 task 只能靠程式碼檢查，見 task 2.3 報告。

## 4. 各 task 審查結論與裁決

SDD 過程中的進度紀錄、各 task 的實作與審查報告都在 git-ignored 的 `.superpowers/sdd/`，收尾時會隨 worktree 一起刪除。
本節把之後還需要的結論、裁決與前例收在這裡，archive 後仍會保留。

### 4.1 各 task 的審查結論

依 Ruling「衝突 D」，每個 task 都審一次。diff 審查一律由 Opus 5.5 執行（專案 memory `opus-review-counts-as-codex-review`，
視同 Codex review）；沒有程式碼 diff 的 1.1 與 4.2 用 sonnet 做內容審查。下表的 Minor 是審查當下的數量，處置見 §4.6。

| Task | 實作 | 審查 | 結論 | 修正輪 | Critical／Important |
|---|---|---|---|---|---|
| 1.1 基線 | sonnet | sonnet（內容） | 修正後通過 | 1 | 0／1：flaky 結論下得太強，改為「疑似偶發」 |
| 2.1 盤點與骨架 | opus | opus | Approved | 0 | 0／0（Minor 9） |
| 2.2 可見集合 | opus | opus | Approved | 0 | 0／0（Minor 4） |
| 2.3 每分頁輪詢 | opus | opus | Approved | 0 | 0／0（Minor 4）；負向對照見 §3 |
| 3.1 狀態轉換 | opus | opus | Approved | 0 | 0／0（Minor 5）；spec 規則逐條對照全吻合 |
| 3.2 版面 | opus | opus | Approved | 0 | 0／0（Minor 6）；visual-check CL1 的處置獲認可 |
| 3.3 並排鈕與無障礙 | opus | opus | Needs fixes → Approved | 1 | 0／1：`aria-describedby` 取代了 `title`，同名檔分不出路徑 |
| 3.4 焦點欄切換 | opus | opus | spec 不符 → Approved | 2 | 0／1：兩個 html 欄的 iframe 之間移動焦點時焦點欄不切換（探針實測）。第 1 輪是控制端在審查前就派的 iframe 修正 |
| 3.5 窄視窗 | opus | opus | Approved | 0 | 0／0（Minor 6） |
| 3.6 持久化 | opus | opus | Approved | 0 | 0／0（Minor 4） |
| 3.7 階段審查 | — | opus | Approved | 0 | 0／0（M1～M3，見 §4.3） |
| 4.1 設計審核 | sonnet（修正） | opus（設計審核、修正審查） | 需修 → Approved | 1 | 設計審核 0／1（F1）＋Minor 4；修正審查 0／0（Minor 3） |
| 4.2 文件 | sonnet | sonnet（內容） | 需修 → 通過 | 1 | 0／3：README 停用條件不精確、缺「不在並排中」與窄視窗替換、archive 後會失效的 change 路徑 |
| 5.2 整支分支 | — | opus | 修正後可合併 | 1 | 0／2：見 §4.4 |

### 4.2 裁決（Ruling）

每條依「決定 — 理由 — 若錯的代價」記錄。5.2 最終審查逐條判定，全部站得住。

| # | 時點 | 決定 | 理由 | 若錯的代價 |
|---|---|---|---|---|
| 1 | 開工前（衝突 A） | 3.1 的 split-check 只驗狀態（可見面板、欄位順序、焦點欄、aria、沒有重讀、捲動不變），等寬與幾何斷言歸 3.2 | 3.1 明文「排版可以暫時不正確」，spec 不因此改變 | 3.1 驗收較鬆，3.2 補上即可。實際上 3.2 起 `expectSplit()` 每次都驗版面 |
| 2 | 開工前（衝突 B） | 基本並排鈕（`<button>`、click、`aria-pressed`、i18n 文字）移到 3.1；3.3 負責其餘入口與無障礙 | 3.1 要有入口才能以真實輸入驗收 | 3.3 範圍略小，沒有返工 |
| 3 | 開工前（衝突 C） | `#review[data-split]`＝實際可見的並排欄數；窄視窗只剩焦點欄時移除 | spec「窄視窗只顯示焦點欄」 | 改一處 CSS |
| 4 | 開工前（衝突 D） | 每個 task 都審一次（Opus），3.7 與 5.2 保留為階段審查與整支分支審查 | 不違反使用者指示，只是多花審查成本 | 多付審查 token |
| 5 | 1.1 | 1.1 只有文件、沒有程式碼 diff，用 sonnet 做內容審查 | 全域分工：非 diff 的內容正確性歸 Claude subagent | 基線數字有誤時，後續回歸判斷失準 |
| 6 | 1.1 | 發版順序：app-icon 先發 v0.1.2，本 change 發 v0.1.3；對方併回後本分支 rebase 並重跑基線 | 「自動更新管理」session 提議，當時對方已完成、本 change 剛開工 | 只影響版號順序 |
| 7 | 1.1 | 5.4 不重寫 `docs/handover.md`，改由「自動更新管理」session 在兩個 change 都發版後統一重寫；本 change 收尾以訊息回報 | 同儕轉述的使用者指示，低風險、可逆 | handover 晚更新，使用者可要求補寫 |
| 8 | 1.1 | 1.1 暫停，讓出 7770 給對方跑驗收，之後 rebase 再重跑 | 避免基線做兩次 | 只延後開工時間 |
| 9 | 2.2 | `aria-selected`、`.is-current`、關閉鈕 tabindex 在 `applyVisibility()` 的 apply 裡與 `hidden` 一起更新，排在 `activate` 之前（偏離 design D2 的字面「最後更新」，D2 已同步更正） | files-check 契約 C3 要求先更新 `aria-selected` 再開始輪詢 | 只是順序描述，沒有行為代價 |
| 10 | 3.2 | 接受 visual-check CL1 的走訪新增兩欄、三欄取樣 | 只擴充死規則偵測的走訪狀態，沒有修改或放寬任何斷言；通則禁止的是放寬 | 審查會指出（3.2 與 5.2 審查都確認沒有放寬） |
| 11 | 3.3 | design D7 改用 `hidden` span，`aria-describedby` 同時連「並排第 N 欄」與路徑 | accname 1.2：被直接參照的 hidden 節點仍計入說明；visually-hidden 會在 tablist 留下靜態文字 | 只是描述措辭 |
| 12 | 3.4 | html 欄的 iframe 用 window blur 加「`activeElement` 是非焦點欄的 iframe」切換焦點欄；Tab 進 iframe 也會切換，接受。spec 不改 | spec「在該欄內按下滑鼠」有約束力；不需要 iframe 內的腳本，也不碰 sandbox | alt-tab 等情境可能誤切焦點欄；可退回「spec 為 html 欄列例外」 |
| 13 | 3.4 | iframe 之間的焦點移動用條件式輪詢 `activeElement`（只在並排中、而且 `activeElement` 是並排欄的 iframe 時，約 200 ms，條件不成立就停）。spec 不改 | 瀏覽器不通知父文件，只能輪詢；成本是一個條件式計時器 | 多一點複雜度；同樣可退回 spec 例外 |
| 14 | 3.6 | 儲存資料缺 `splitFocus` 時，視同「不在並排組合中」，改用第一欄 | spec「儲存的焦點欄不在並排組合中時改用第一欄」的自然延伸 | 只影響手改過的 localStorage |
| 15 | 3.7 | M1：Tab 順序依開啟順序而不是欄位順序，列為 design Risks 的刻意取捨，不搬 DOM | 搬 DOM 會讓 iframe 重新載入；各欄獨立，不影響操作 | 鍵盤與螢幕閱讀器使用者感到跳動。後來 4.1 F4 以 `reading-flow` 修好 Chromium 137 以上的情況，本條只剩「不搬 DOM」仍成立 |
| 16 | 4.1 | design.md Risks 改寫（`reading-flow`）不交使用者 | memory 規則指的是專案設計文件與 spec；本 change 的 design.md 是實作設計，外觀定案沒有變 | 使用者可要求退回 |
| 17 | 5.2 | 整支分支審查（5.2）先於全 gate（5.1） | 修正輪會改程式，5.1 要量最終版本 | 只是順序 |

另有一條範圍外的延後項：視窗寬度小於 760 時，`scrollIntoView({ block: "nearest" })` 會讓元素停在固定狀態列底下
（可能違反 WCAG 2.4.11）。這是既有問題，3.5 審查確認本 change 沒有加劇（本分支 diff 沒有 `scrollIntoView`），
收尾時回報使用者，建議另開 change（例如 `scroll-padding-bottom`）。

**使用者裁決（2026-10-06，來自 4.1 設計審核的 U1、U2）**，已寫入 proposal 非目標：

- U1：三欄維持等寬，不設最小欄寬，也不在窄時自動降為兩欄。
- U2：停用的並排鈕維持顯示，不改成平時隱藏。

### 4.3 階段審查（3.7）與設計審核（4.1）

**3.7 階段審查**（Opus，範圍 `e1bbaa2..4c916a9`，產品 diff 加上腳本另讀）：Approved，0 Critical／Important。

- 以 4 個隨機種子跑了共 850 步的隨機操作（fuzz），沒有違反任何不變式。
- 觀察到 34 次輪詢，全部與可見集合吻合。
- Minor 三條：M1 Tab 順序（裁決 15）；M2 `splitAvailable()` 與 `toggleSplit()` 重複規則、M3 鍵盤按並排鈕移出目前分頁後
  焦點停在 `tabindex=-1` 的按鈕（延後到 5.2，處置見 §4.6）。

**4.1 設計審核**（Opus，依 frontend-design 原則，以 `docs/direction-01-visual-design.md` 與 spec 為定案）：需修。

| Finding | 內容 | 處理 |
|---|---|---|
| F1（Important） | 三欄時工具列的路徑被壓到約 18px，認不出是哪個檔案 | 已修：路徑 `min-width: 7em` |
| F2 | 窄欄時 PDF 工具列按鈕的文字逐字直排 | 已修 |
| F3 | 焦點欄只換 1px 框線的顏色，三欄時不夠醒目 | 已修：加 1px outline |
| F4 | Tab 進各欄的順序可以跟畫面一致，不必搬 DOM | 已修：`reading-flow: grid-order`（Chromium 137 以上） |
| F5 | 分頁列的冰青太多，非焦點欄的徽章與焦點欄互搶 | 已修：非焦點欄徽章改用 `--text-dim`（對比重算至少 7.3:1） |
| U1、U2 | 最小欄寬、停用鈕是否隱藏 | 交使用者，見上方使用者裁決 |

- 維持現狀、不列 finding 的項目：「◫」字形（由 Windows 內建的 Cambria Math 補字）、停用鈕平時外觀、非目前分頁的 18px
  空位（避免 hover 時分頁位移）、徽章數字偏上（次像素等級）。與 skill 通用原則衝突、依專案定案只記錄的項目有 4 條（字體、
  等寬數字、色票、編號標記）。
- **截圖**：三種寬度（1536、1100、700）乘以單欄、兩欄、三欄，加上特寫，共 37 張。存在 repo 外（審核 session 的暫存目錄），
  用途是審核當下的對照與修法預覽；不進 repo，worktree 刪除後不保證留存。拍攝前以 MutationObserver 把使用者名稱換成
  `<user>`，每張都斷言畫面文字不含真名，拍完逐張看過（專案 memory `deidentification-must-inspect-images-not-just-grep`）。

### 4.4 整支分支審查（5.2）

Opus，範圍 `e1bbaa2..e14fb38`（49 個 commit）。結論「修正後可合併」：0 Critical，產品邏輯沒有找到功能性錯誤。

- **I-1**：回滾相容斷言用 `git merge-base HEAD origin/main` 取舊版。squash 併回 `main` 後 merge-base 就是 HEAD 自己，
  斷言會恆真（假綠），沒有 `origin/main` 的 clone 也會失敗。處理：改成固定比對 tag `v0.1.2`（本 change 之前的最後一個
  發行版），並要求取出的版本不含 `splitTabs`（`split-check.md`「回滾相容」）。
- **I-2**：審查紀錄與裁決只存在 git-ignored 的進度紀錄，design D6 也沒寫 iframe 的做法。處理：寫進本節；design D6 補上
  html 欄（iframe）的做法、後果與已知缺口。
- 新提的 Minor 三條（m-1 split-check 檔頭敘述過期、m-2 tasks 5.4 的 handover 改派沒有註記、m-3 CONTEXT「焦點欄」的外框
  寫成無條件）與延後 Minor 的處置見 §4.6。
- 修正輪的驗收：
  - I-1 的鑑別力：照常比對 `v0.1.2` 時回傳 true，`v: 3` 對照為 false。暫時把回滾目標改成 `HEAD`，前置條件以
    「已含並排」失敗；再暫時拿掉這道檢查，讀到的是 HEAD 的 sha、同樣回傳 true。兩次讀到不同的 commit，證明平常讀的
    確實是 tag 的版本（兩版的 `isStoredState()` 原文相同，所以只看回傳值分不出來，要靠 sha 與 `splitTabs` 檢查）。
  - 重建 `ui_preview` 後，split-check 47／47 PASS，14 支既有腳本全綠（live-output 第 1 次的收尾偶發見 §4.5）。
  - `cargo test --workspace` passed 1304、failed 0、ignored 13，與基線相同。

### 4.5 偶發失敗紀錄

| 腳本 | 何時 | 現象 | 判斷與處置 |
|---|---|---|---|
| `live-output-check.js` R 段 | 2.3 | 第 1 次 FAIL 1：`wJ:p1 的 pane 列可以取得焦點`（chrome-R，鍵盤焦點跨重畫保留） | 已知競態：`focus()` 與檢查 `activeElement` 是兩次分開的 `eval`，中間節點被重畫換掉。重跑綠。與本 change 無關 |
| `actions-check.js` | 3.3 修正輪 | 第 1 次 FAIL 1：`按 Enter 送出一次 POST advance（實際 [advance, advance]）` | Factory Floor 推進鈕，與分頁列無關。重跑 3 次都綠。推測（未查證）：腳本等錯誤訊息消失後立刻清空請求紀錄，前一步的 POST 在清空之後才被記到。再出現就另開項目查證 |
| `split-check.js` 收尾 | 3.4 | `chrome-split-tree PID 91096` 收尾時已觀察到 exit，結尾複查又找到同一個 PID | 以 `Get-CimInstance Win32_Process` 查：是使用者桌面 Chrome 的 renderer，建立時間遠晚於該段結束，判定為 PID 重用。沒有動那個行程，重跑綠。判別方法見 `split-check.md`「已知的偶發失敗」 |
| `visual-check.js` `chrome-P1` | 1.1 第一輪 | `chrome-P1 PID 36024 已終止（tasklist 查無此 PID）` | 原因未查明（見 §1）。之後 2.1～3.4 與 5.2 修正輪的 `chrome-P1` 三項收尾檢查都通過，沒有再出現。再出現就要深查，不可當偶發略過 |
| `live-output-check.js` S 段收尾 | 5.2 修正輪 | 第 1 次 FAIL 1：`chrome-S PID 82628 已終止（tasklist 查無此 PID）`，緊接的 `preview-S` 與 port 檢查都過 | 腳本在 `taskkill /T /F` 之後立刻用 `tasklist` 查，沒有等行程結束。約 40 秒後以 `Get-CimInstance Win32_Process` 查已無此 PID，不是殘留，也不是被長期重用。推定為行程還在結束中就被查到；重跑綠。與本 change 無關 |
| `cargo test` `loop_integration` | 1.1 前後 | `l_close_disconnects_then_reconnects_with_fresh_seed` 偶發 | 上游時序空檔，`main` 的 `44732ed` 已修，rebase 後沒有再出現 |

### 4.6 延後項與不修項

5.2 最終審查把進度紀錄中所有延後的 Minor 逐條分流。編號沿用該審查的分流表。

**合併前必修（task 5.2 修正輪已處理）**：

- #10：live-output R 段偶發補進 §4.5。
- #17a：`closeTab()` else 分支的註解改照實際分流條件「目前分頁是否改變」。
- #25a：`restoring` 的註解原寫「還原完成後寫一次」，與程式不符，改照實描述（不改行為：補寫會提早丟掉新版才認得的 kind）。
- #25b（＝I-1）：回滾斷言改比對 tag `v0.1.2`。
- #26a（＝3.7 M2）：`toggleSplit()` 開頭改用 `splitAvailable()`，停用條件只寫一處，行為不變。
- #28：README「並排時恰有一欄是焦點欄」改為「並排中」。

**可修（task 5.2 修正輪已處理）**：

- #1：§1 flaky 段把「已安裝版佔 7770」移回第一輪，「未重現」措辭降級。
- #4：刪掉 split-check 沒有呼叫端的 `opts.beforeNavigate`。
- #12b：`移出焦點欄` 段加「三欄時移出最右側的焦點欄，焦點欄改為左側欄」。
- #17b：`關閉後只剩一個時解除並排` 段加「並排中關閉非焦點欄，鍵盤焦點移到相鄰分頁」。
- #19：design D6 補行為界定：右鍵、中鍵與觸控的按下也切換焦點欄；Tab 進 md 連結不切換；拖曳選字沒有專門斷言。
- #21：焦點所在的 iframe 被換掉後漏偵測，記為已知缺口（`files.js` 的 iframe 註解區塊與 design D6）。
- #22a：`files.js` 窄視窗監聽的註解折行。
- #24b：`segSplitI18n` 拿掉「並排沒有還原就重新按」的退路，改為前置條件。
- #27c：design 的 MDN 網址改用新路徑（舊網址 301 轉址到新路徑）。
- m-1、m-2、m-3：split-check 檔頭改寫；tasks 5.4 與 proposal Impact 註記 handover 改派；CONTEXT「焦點欄」的外框加上
  「各欄並排顯示時」的條件。

**可修但跳過**：

- #26b（＝3.7 M3）：用鍵盤按並排鈕移出目前分頁後，焦點停在 `tabindex=-1` 的並排鈕上。焦點沒有遺失、按鈕可見，只是 roving
  不一致。修法是比照 `closeTab()` 把焦點移到新的目前分頁，這會改變產品的焦點行為，超出「不改產品行為」的修正輪範圍，
  留待之後另行處理。

**不修**（理由摘要）：

- 已在後續 task 處理（9 條）：#2、#3、#6、#7、#8、#11、#14、#15、#20。
- #5：`isPortListening` 只比對 127.0.0.1（`ui_preview` 只綁 127.0.0.1）、`OUR_PORTS` 排序只影響輸出、約 450 行照抄
  files-check 是 repo 慣例，抽共用模組要跨全部腳本另案處理。
- #9：快速切換段的三個隱含時序前提（`split-check.js` 快速切換段），都往失敗方向表現，不會假綠；出現 flake 先懷疑這段。
- #12a：替換分支與 `checkColumnUntouched` 缺負向對照。負向對照是一次性實證，3.7 的 fuzz 850 步沒有違反。
- #13：腳本內部整潔（overflowers 診斷重複、`need(!!s)` 無作用、溢出只查右緣）；左緣溢出已被「各欄在 `#review` 邊界內」擋住。
- #16：Ctrl＋Space 沒有斷言，spec 只規定 Ctrl＋Enter。
- #18：收尾比對 PID 不核對建立時間，往失敗方向表現，判別方法已寫進 `split-check.md`。
- #22b：窄視窗段的幾條斷言鑑別力較弱；`order` 與 `data-split` 由同一分支寫入，後者已有斷言。
- #23（只記錄）：用 Tab 鍵進入非焦點欄的元素後把視窗變窄，焦點掉到 body。spec 沒有規定，要同時用鍵盤與滑鼠才會發生，
  元素被隱藏時焦點回到 body 是瀏覽器的標準行為。
- #24a：不合法的 `split` 留在 localStorage 到下一次寫入，代價只是每次載入多一行警告（見 #25a）。
- #25c：不合法輸入可再補（非陣列、非整數、負數等）；`restoreSplit()` 對這些都有明確分支，spec 列的五種已涵蓋。
- #27a：焦點欄外框不被裁切只在 1100 寬驗過。靜態推導不會被裁，5.3 真機時順手看一眼 1280 以上的固定一屏版面。
- #27b：沒有驗 Shift＋Tab，`reading-flow` 的正反向順序由瀏覽器一致保證。

## 5.1 收尾驗收（HEAD a1e0cd3）

task 5.1。在最終版本（`a1e0cd3`，已含 5.2 修正輪）上跑完整 gate 與全部腳本。開跑前 `git status` 乾淨，先
`cargo build -p cockpit --example ui_preview`（exit 0）。每個 gate 單獨跑、不接管線，結束碼取自指令本身。
腳本逐支在前景執行，每支開跑前確認 7770、7780～7999、19000 一帶與 7910 沒有 LISTEN（唯一的 LISTEN 是 7778，ASUS，與本專案無關，沒有動）。
本節沒有改任何產品程式碼或腳本。

### Gate

| 指令 | 結束碼 | 結果 | 與 §1 基線 |
|---|---|---|---|
| `cargo fmt --check` | 0 | 無輸出 | §1 沒跑（當時沒有改程式碼） |
| `cargo clippy --all-targets -- -D warnings` | 0 | `Finished`，0 warning | 同上 |
| `cargo test --workspace` | 0 | 68 個結果列合計 passed 1304、failed 0、ignored 13 | 相同 |
| `cargo test -p cockpit --example ui_preview` | 0 | 62 passed、0 failed、0 ignored | 相同 |
| `markdownlint-cli2 "**/*.md"`（repo 根） | 0 | `Linting: 203 files`、`Summary: 0 issues in 0 files` | 202 → 203 files（新增 `split-check.md`），仍 0 issues |
| `openspec validate --all` | 0 | `Totals: 25 passed, 0 failed (25 items)` | 26 → 25：app-icon change 已 archive（`e1bbaa2`），項目少一個 change；輸出另有 141 條 SHALL／MUST 的 WARNING（既有 spec 的英文規範建議，不影響結果） |

### 腳本

「ok」是輸出中以 ok 加一個空白開頭的行數，「PASS」是以 PASS 加一個空白開頭的行數（notify、output-color、i18n 用 PASS 格式），
「FAIL」是以 FAIL 加一個空白開頭的行數。
全部 15 支都是第 1 次就 exit 0、FAIL 0，**沒有任何重跑**。

| # | 腳本 | 結束碼 | ok／PASS | FAIL | 秒數 | 與基線 |
|---|---|---|---|---|---|---|
| 1 | `2026-09-15/reconnect-check.js` | 0 | ok 14 | 0 | 12 | 相同（14） |
| 2 | `2026-09-15/whatever-check.js` | 0 | ok 9 | 0 | 4 | 相同（9） |
| 3 | `2026-09-16/actions-check.js` | 0 | ok 86 | 0 | 19 | 相同（86）；§4.5 的重複 POST 沒有出現 |
| 4 | `2026-09-16/channel-backoff-check.js` | 0 | ok 19 | 0 | 51 | 相同（19） |
| 5 | `2026-09-16/factory-floor-check.js` | 0 | ok 131 | 0 | 10 | 相同（131）；覆寫 `task-5.2-scenario-d.png`，已 `git checkout --` 還原 |
| 6 | `2026-09-19/live-output-check.js` | 0 | ok 345 | 0 | 221 | 相同（345）；R、S 段都沒有觸發 §4.5 的偶發 |
| 7 | `2026-09-23/visual-check.js` | 0 | ok 1769 | 0 | 250 | 1757 → 1769（+12）。基線後 `ed8405a`（task 3.2）與 `7697005`（task 4.1）擴充 CL1 走訪的並排狀態（Ruling 10，共 28 行新增，沒有改動或放寬任何斷言）；FT4 基線就有。摘要「既有段落 FAIL 數：0；FT1–FT3 FAIL 數：0；收尾 FAIL 數：0」。`chrome-P1` 三項收尾檢查（PID 已終止、exit 事件、CDP port 無 LISTEN）全過 |
| 8 | `2026-09-27/files-check.js` | 0 | ok 578 | 0 | 195 | 相同（578） |
| 9 | `2026-09-28/git-check.js` | 0 | ok 770 | 0 | 220 | 相同（770） |
| 10 | `2026-10-01/progress-check.js` | 0 | ok 61 | 0 | 9 | 相同（61） |
| 11 | `2026-10-01/ui-fixes-check.js` | 0 | ok 349 | 0 | 27 | 相同（349） |
| 12 | `2026-10-02/notify-check.js` | 0 | PASS 253（新行為 191＋前置 62） | 0 | 78 | 相同（253） |
| 13 | `2026-10-02/output-color-check.js` | 0 | PASS 89 | 0 | 45 | 相同（89） |
| 14 | `2026-10-03/i18n-check.js` | 0 | PASS 1460（新行為 948＋前置 512） | 0 | 142 | 相同（1460）；重拍 `i18n-en-*.png` 15 張，已 `git checkout -- docs/research/2026-10-03` 還原 |
| 15 | `2026-10-04/split-check.js`（新腳本） | 0 | 47／47 段 PASS（另加收尾衛生 PASS，共 48 行 `PASS`）；ok 行 6554 | 0 | 429 | 基線沒有這支，新增；段落數與 `split-check.md` 的 47 段相符，§4.4 的修正輪記錄也是 47／47 |

### 收尾狀態

- 跑完 `git status` 乾淨：被覆寫的 png 已還原，其餘沒有改動。
- 沒有殘留 `ui_preview.exe`；7770～7999、19000 一帶沒有任何 LISTEN（7778 為 ASUS，不是本專案的行程）。
- 偶發失敗：本輪沒有出現 §4.5 登記的任何一項（live-output R、S 段、actions-check 重複 POST、split-check PID 重用、visual-check `chrome-P1`），
  也沒有未登記的失敗，因此沒有重跑、沒有查 `Get-CimInstance`。
- 與基線的差異彙整：Rust 與 ui_preview 單元測試數字不變；markdownlint 多 1 個檔；openspec 項目少 1；
  腳本只有 visual-check 多 12 條（CL1 走訪）與新增 split-check，其餘 13 支數字與 §1 完全相同。
