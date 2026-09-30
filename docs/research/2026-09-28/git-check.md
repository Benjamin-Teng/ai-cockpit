# git-check.js 使用說明

> 日期：2026-09-28（git-review task 4.2 起；task 4.3 加 diff 分頁內容；task 4.4 加 Git Graph 分頁內容；
> task 4.5 加 commit 詳情、比較與某版本檔案分頁；2026-09-30 目視驗收缺陷 V1／V2 修正（Ruling R11／R12）
> 加兩段；2026-09-30 code review 缺陷 M1／M2／M3 加三段）。對象：`docs/research/2026-09-28/git-check.js`。性質：腳本用法、段落代號的參考文件；內容與
> 腳本檔頭註解一致，改契約時兩邊一起改。

## 用途

對 `cockpit --example ui_preview`（固定 fixture 起的 dashboard，不需要 HERDR）逐條驗證
`openspec/changes/git-review/specs/git-review/spec.md`「左欄變更分頁」的三個 scenario（task 4.2）：

- 顯示變更並開啟 diff
- 新變更自動出現
- 不是目前分頁時不讀取

與「diff 分頁」的三個 scenario（task 4.3）：

- 左右並排呈現
- 改檔後更新
- 窄視窗不橫向捲動

目視驗收發現的兩個缺陷（2026-09-30，Ruling R11／R12；不在 spec 原始逐字 scenario 內，是驗收後追加的
回歸測試）：

- 合併衝突開 diff
- 刪除檔的工具列停用

「左欄變更分頁」另有 fix round 2 補的第四個 scenario（不在 spec 逐字 scenario 內，是整頁重畫 regression 的回歸測試）：

- 重畫不影響變更分頁

與「Git Graph 分頁」的四個 scenario（task 4.4）：

- 開啟並分批載入
- 搜尋跳轉
- 分支變更提示
- 重畫不影響 Git Graph

與「commit 詳情與比較」「某版本檔案分頁」的五個 scenario（task 4.5），加上 file-review「分頁還原」的
「還原 git 分頁與變更分頁」情境（放在本檔而不是 `files-check.js`，因為需要 diff／Git Graph／rev 分頁與
review-repo 的 git fixture）：

- 看 commit 的變更並開 diff
- 比較兩個 commit
- 複製 hash
- 看舊版規格
- commit 版本不輪詢
- （file-review）還原 git 分頁與變更分頁

code review 找到的三個前端缺陷回歸測試（M1／M2／M3；不在 spec 逐字 scenario 內）：

- 隱藏的 Git Graph 不自動載入
- 比較詳情標頭不含 undefined
- 看此版本按鈕鍵盤操作

headless Chrome＋CDP，啟動、收尾、行程所有權模型與段落代號寫法沿用 `docs/research/2026-09-27/files-check.js`
（本身沿用 `docs/research/2026-09-23/visual-check.js`）。

## 為什麼「左欄三個分頁」不在這裡

`file-review`「左欄檔案樹」MODIFIED 的 scenario「左欄三個分頁」（WHEN 載入頁面 THEN 左欄頂端依序為
「Project」「檔案」「變更」，方向鍵與 Enter 能切到「變更」）只驗左欄 tablist 本身的通用機制——三個分頁按 DOM
順序排列、方向鍵移動焦點、Enter 選定——跟 git 後端或「變更」面板的內容完全無關。`files-check.js` 已經是
file-review「左欄檔案樹」全部前端 scenario 的家，也已經有現成的左欄分頁夾具（`switchLeftTab()`、
`window.__fc.leftSelected()`），不必為了這一段另外啟動一套 preview／Chrome。這段放在
`docs/research/2026-09-27/files-check.js`，代號 `file-review/左欄三個分頁`。

## 事前準備

- **不需要 HERDR**：全程只對 `ui_preview` 操作。
- **先 build**：`cargo build -p cockpit --example ui_preview`。前端資源內嵌在執行檔裡，改了 `cockpit/assets/`
  一定要重新 build，腳本不會自己 build。
- **需要 Chrome**：預設 `C:\Program Files\Google\Chrome\Application\chrome.exe`，可用 `COCKPIT_CHROME` 指定。
- **需要安裝 git**：系統要能在 PATH 找到 `git` 可執行檔——`ui_preview` 背後的服務端點靠它讀 repo 狀態，
  腳本本身操作暫存副本 fixture（`git commit-tree`、`update-ref`、`commit --allow-empty` 等）也靠它；沒裝
  git 時多數段落會在等待服務回應或寫入 fixture 時逾時失敗，不是腳本或服務的 bug。
- Node 22，不需要額外套件。
- **不要與其他驗收腳本同時跑**（尤其 `files-check.js`／`visual-check.js`：都會啟動 `ui_preview` 並綁定
  `127.0.0.1:7770` 附近的埠，計時斷言與收尾的行程、埠清查會互相干擾）。開跑前若偵測到有 `ui_preview.exe`
  在跑、或 `127.0.0.1:7770` 有人 LISTEN，就印 `RESULT: FAIL (環境)` 並以 exit 2 結束，不動別人的行程。

## 執行方式

```bash
cargo build -p cockpit --example ui_preview

node docs/research/2026-09-28/git-check.js                                       # 全部段落
node docs/research/2026-09-28/git-check.js "self/鷹架,git-review/新變更自動出現"  # 只跑指定段落（逗號分隔）
node docs/research/2026-09-28/git-check.js git-review/                           # 以「capability/」選該前綴的全部段落
```

- 段落代號含空白時整個參數要加引號。
- 代號拼錯、參數是空字串或只有逗號時，在啟動任何行程之前印 `RESULT: FAIL (段落代號)`，exit 2。

## 段落代號對照表

| 代號 | spec（capability／Requirement） | 做法摘要 |
| --- | --- | --- |
| `self/鷹架` | 腳本自我測試 | 啟動 ui_preview 並讀到 review-repo 暫存路徑；`wJ:p4` 根目錄為 review-repo、`is_git` 為 true；git 狀態端點回 200 並符合 task 3.1 fixture 的既定工作區狀態（`staged-change.txt` 已暫存 M、`unstaged-change.txt` 未暫存 M、`deleted-in-worktree.txt` 未暫存 D、`untracked-file.md` 未追蹤 ?）；網路記錄把 git 狀態端點分類為 `git_status`；收尾衛生（行程、埠、暫存目錄、repo 內 fixture 雜湊不變） |
| `self/段落代號` | 腳本自我測試 | `parseSegmentArg` 對合法、拼錯、未知前綴、空字串、只有逗號各驗一次；實際以錯的代號與空字串執行本檔，必須 exit 2、不啟動行程 |
| `git-review/顯示變更並開啟 diff` | git-review／左欄變更分頁 | 選定 `wJ:p4` 後切到「變更」：面板頂端顯示 `review-repo` 與分支 `main`；`history/unstaged-change.txt` 在「變更」組、`history/staged-change.txt` 在「已暫存」組、`history/untracked-file.md` 在「未追蹤」組；點 `history/unstaged-change.txt` 列開啟並選定它的 diff 分頁（`from=INDEX`、`to=WORKTREE`），標籤含檔名與「工作區」，title 含完整路徑與根目錄名稱；額外驗證按「Git Graph」開啟並選定該 repo 的 Git Graph 分頁（不是 spec 逐字要求，順手驗這條線同樣接得起來） |
| `git-review/新變更自動出現` | git-review／左欄變更分頁 | 先寫入 80 個填充檔案讓清單可以捲動並捲到非 0 位置；在暫存副本新增 `later.md`：3 秒內出現在「未追蹤」組，清單捲動位置不變（±1px） |
| `git-review/不是目前分頁時不讀取` | git-review／左欄變更分頁 | 選定 `wJ:p4` 並切到「檔案」分頁，觀察 10 秒：服務沒有收到任何 `/api/git/…/status` 請求；前置另外驗證切到「變更」分頁後確實會發出這個請求（證明計數器量得到，不是分類邏輯本身失效而假陽性通過） |
| `git-review/重畫不影響變更分頁` | git-review／左欄變更分頁（fix round 2 回歸測試，非 spec 逐字 scenario） | 寫入 30 個填充檔案讓「變更」清單可捲動，捲到非 0 位置並讓 `history/staged-change.txt` 列取得鍵盤焦點；以 100ms 推送間隔觸發至少 3 次整頁重畫：Project 清單仍隱藏（根因是 fix round 2 發現 `render.js` 曾誤用 `leftTab === "files"` 判斷是否該畫 Project，導致切到「變更」分頁後重畫會讓 Project 冒出來）、「變更」清單與每一列的子孫節點都沒被換掉、捲動容器仍是原本那個且捲動位置不變（±1px）、焦點仍在原本的列上 |
| `git-review/左右並排呈現` | git-review／diff 分頁（task 4.3） | `history/unstaged-change.txt` 的 diff（`INDEX→WORKTREE`）：唯一一列為 `change`、左右行號相同、左 `.diff-row-del`／右 `.diff-row-add`；工具列路徑與版本文字；「開啟檔案」在中繼資料查到工作區檔案後變可按、「在 VS Code 開啟」顯示且 `href` 以 `vscode://` 開頭；「看左側版本」開 `rev` 分頁（`INDEX`／原路徑）、「看右側版本」（`to=WORKTREE`）開檔案分頁；「開啟檔案」切到既有分頁、不新增。`history/deleted-in-worktree.txt` 的 diff：整份是 `delete` 列、右側 `.diff-row-blank`；「開啟檔案」停用、「在 VS Code 開啟」不顯示、「看右側版本」仍顯示但停用（目視驗收缺陷 V2、Ruling R12；停用樣式的可辨識性另有 `git-review/刪除檔的工具列停用` 段驗證）。暫存副本裡在 `docs/design.md` 兩處分散插入：兩個變更區塊之間有一列 `gap`，兩處插入各是一列 `add`（左側 `.diff-row-blank`）；格線本身是 4 欄 CSS grid |
| `git-review/合併衝突開 diff` | git-review／左欄變更分頁、單檔 diff 端點（目視驗收缺陷 V1、Ruling R11） | 選 `wJ:p5`（`other-repo`，design D10 fixture：進行中且有衝突的 merge，`conflict.txt`）、切到「變更」：`conflict.txt` 在「合併衝突」組；點列開出的 diff 分頁兩側是 HEAD（40 碼 hash）→ `WORKTREE`（不是 `INDEX→WORKTREE`——那個組合對未合併檔案會得到 `diff --cc` 三方格式，回 409 `unmerged_path`）；內容正常載入、沒有錯誤訊息、含衝突標記（`<<<<<<<`／`=======`／`>>>>>>>`）的新增列 |
| `git-review/刪除檔的工具列停用` | git-review／diff 分頁（目視驗收缺陷 V2、Ruling R12） | `history/deleted-in-worktree.txt` 的 diff（`INDEX→WORKTREE`，工作區已刪除）：「開啟檔案」與「看右側版本」（右側為 `WORKTREE`）都帶 `disabled` 屬性；兩者的計算樣式（`color`）跟同一顆按鈕在可用狀態（`history/staged-change.txt`，工作區仍有這個檔案）下不同——單靠 `disabled` 屬性不夠，`.action-button` 靜止時本來就是 `--text-dim`，要靠 `style.css` 新增的 `.action-button:disabled`（`color-mix` 從 `--text-dim` 推導的更淡版本）才分得出來 |
| `git-review/改檔後更新` | git-review／diff 分頁（task 4.3） | 暫存副本裡在 `long.md` 分散多處各改一行，讓 `INDEX→WORKTREE` 的 diff 內容夠高、確實需要捲動；開啟 diff 分頁並往下捲動；在檔案末端再新增一行：3 秒內以新增列出現、捲動位置不變（±2px） |
| `git-review/窄視窗不橫向捲動` | git-review／diff 分頁（task 4.3） | 暫存副本裡新增一個含 300 字元長行的未追蹤檔案；以預設寬度（1536）完成選取 pane、切「變更」分頁、點列開 diff 分頁（避免 700 寬單欄＋整頁捲動版面下 pane 列被 sticky 底列擋住點不到），只在量測當下切到 700×900：長行在 `.diff-text` 儲存格內折行（渲染高度 > 1.5 倍行高）、`documentElement` 沒有橫向捲軸 |
| `git-review/開啟並分批載入` | git-review／Git Graph 分頁（task 4.4） | 開 Git Graph：先顯示 200 列，捲到底部後共顯示 264 列（review-repo 的既定歷史）、沒有重複的 commit、末端沒有「載入更多」；264 未達 5000 上限不顯示「已達上限」；順手驗分支篩選 popover 列出三組並可用 Esc 關閉 |
| `git-review/搜尋跳轉` | git-review／Git Graph 分頁（task 4.4） | 在暫存副本疊 3 個含 `git-check-search-hit` 字樣的 commit；輸入（不分大小寫）後先只顯示「共 3 筆」，第一次 Enter 落第 1 筆、第二次落第 2 筆，跳到的列在可見範圍內且有標示，清單列數不變 |
| `git-review/分支變更提示` | git-review／Git Graph 分頁（task 4.4） | 在暫存副本 `commit --allow-empty` 新增一個 commit；3 秒內出現「分支已變更」與「重新載入」按鈕，期間清單內容與捲動位置不變；按「重新載入」後新 commit 出現在第一列 |
| `git-review/重畫不影響 Git Graph` | git-review／Git Graph 分頁（task 4.4） | 捲到非 0 位置、選取並聚焦某一列，觸發至少 3 次整頁重畫：捲動位置、選取、焦點、清單 DOM 節點都不變（task 4.5 起點選會展開 commit 詳情，本段等詳情讀取完成才拍快照，避免那次非同步讀取被誤判成「清單被改動」） |
| `git-review/看 commit 的變更並開 diff` | git-review／commit 詳情與比較（task 4.5） | 點 HEAD（review-repo 既定歷史裡的合併 commit，2 個 parents）：詳情顯示完整 hash、完整訊息、「與第一個父 commit 比較」註記，變更檔案清單含 `history/feature-formatting.txt`；點該檔案列開啟 diff 分頁（左側為第一個 parent、右側為 HEAD） |
| `git-review/比較兩個 commit` | git-review／commit 詳情與比較（task 4.5） | 點 commit E（HEAD）、按「選為比較基準」，再點 commit F（`feature/logging` tip）：詳情顯示「比較 E ↔ F」與兩者之間的變更檔案（不只一個）；切到「自分岔點起」後，檔案清單改為兩者共同祖先 B 與 F 之間的變更（只有 `history/feature-logging.txt`，跟「直接比較」明顯不同）；點檔案列開對應的 diff 分頁 |
| `git-review/複製 hash` | git-review／commit 詳情與比較（task 4.5） | CDP `Browser.grantPermissions` 授予剪貼簿權限；按完整 hash 旁的「複製」：畫面顯示「已複製」、`navigator.clipboard.readText()` 讀回的內容等於該 commit 的 40 碼 hash、提示在 2 秒後消失 |
| `git-review/看舊版規格` | git-review／某版本檔案分頁（task 4.5） | 搜尋並點「填充 commit #210」（`history/counter.txt` 在該版本內容為「210」，工作區已被之後的 commit 改寫成「post-branch」）的詳情，按 `history/counter.txt` 的「看此版本」：出現「counter.txt @ <短 hash>」分頁，以文字檢視器顯示「210」、不含「post-branch」；「開啟目前版本」開啟工作區版本的檔案分頁 |
| `git-review/commit 版本不輪詢` | git-review／某版本檔案分頁（task 4.5） | 同上開出「填充 commit #210」的 `history/counter.txt` 某版本分頁，內容載入完成後觀察 10 秒：服務沒有再收到該分頁（該 rev／路徑）的任何請求 |
| `file-review/還原 git 分頁與變更分頁` | file-review／分頁還原（task 4.5） | 開 README.md 檔案分頁、`history/unstaged-change.txt` 的 diff 分頁、Git Graph 分頁（目前分頁為 Git Graph、左欄為「變更」），重新整理：三個分頁依原順序還原、目前分頁為 Git Graph 且內容重新載入、左欄為「變更」；額外驗證（超出 spec 逐字的三分頁 scenario）：再開一個 `rev` 分頁（diff 的「看左側版本」）後第二次重新整理，確認 `rev` 分頁也能還原且仍是目前分頁 |
| `git-review/隱藏的 Git Graph 不自動載入` | git-review／Git Graph 分頁（code review 缺陷 M1） | 開 README.md 分頁與 Git Graph 分頁後切回 README.md，重新整理（還原後 Git Graph 分頁存在但不是目前分頁）：第一批 200 列載入後等 3 秒，`/log` 請求恰 1 次、列數仍 200（修正前背景一路載到 264）；切回 Git Graph 分頁並捲到底，仍能載入到 264 列 |
| `git-review/比較詳情標頭不含 undefined` | git-review／commit 詳情與比較（code review 缺陷 M2） | 選 A 並等詳情載入後 Ctrl+點 B，以 MutationObserver 記錄詳情面板每個瞬間的文字：標頭都不含 `undefined`、最終含 A／B 短 hash；再於 C 詳情載入中立刻 Ctrl+點 D：仍顯示「比較」畫面、沒有 pageerror |
| `git-review/看此版本按鈕鍵盤操作` | git-review／commit 詳情與比較（code review 缺陷 M3） | 點「填充 commit #210」，鍵盤聚焦 `history/counter.txt` 的「看此版本」並以 CDP 送真實 Enter：開出並選定某版本分頁，沒有 `history/counter.txt` 的 diff 分頁 |

## 預覽資料

`ui_preview` 啟動時把 `cockpit/examples/fixtures/review-repo/` 複製到
`%TEMP%\cockpit-ui-preview-<pid>-<ns>\review-repo`（stdout 印 `review-repo: <路徑>`），另建 `other-repo`；假
pane `win/wJ:p4`（cwd＝`review-repo/src`）往上找到 `review-repo` 根目錄。工作區既有四種未 commit 狀態（見
`.superpowers/sdd/tasks/task-3.1-report.md`）：

| 路徑 | 組 | 狀態 |
| --- | --- | --- |
| `history/staged-change.txt` | 已暫存 | `M` |
| `history/unstaged-change.txt` | 變更 | `M` |
| `history/deleted-in-worktree.txt` | 變更 | `D` |
| `history/untracked-file.md` | 未追蹤 | `?` |

需要額外檔案（「新變更自動出現」段的填充清單與 `later.md`）一律只寫暫存副本，不碰 repo 內的 fixture（`self/鷹架`
另外用雜湊確認 repo 內 fixture 沒被改）。

task 4.5 的五段 `git-review/` 段落與「還原 git 分頁與變更分頁」段依賴 review-repo 既定歷史裡的分支結構
（design D10；`cockpit/examples/ui_preview.rs` `build_review_history_stream()`）：main 的 HEAD 是合併
commit E（「合併 feature/formatting 回 main」），第一個 parent 是 main 上緊接分支點之後的 D，第二個
parent 是 `feature/formatting` 的 tip C；`feature/logging` 的 tip F 與 C 從同一個分支點 B（「新增工作區
情境要用到的追蹤檔案」commit）分岔，B 是 E 與 F 的共同祖先且與兩者都不同。`history/counter.txt` 被逐一
填充 commit（`#1`…`#250`）與最後一次「post-branch」改寫，內容隨版本改變，用來驗「看舊版規格」讀到的是
「當時」的內容。這些都是既有 fixture，不需要额外寫入。

## 為什麼「Git Graph」按鈕的驗證放在「顯示變更並開啟 diff」段

spec「左欄變更分頁」的三個 scenario 沒有單獨一個「按 Git Graph 開分頁」的 scenario（那顆按鈕的行為描述在
Requirement 正文，不是獨立 scenario），本檔選擇在已經打開「變更」面板、已知 root_id 的「顯示變更並開啟 diff」段
順手多驗一步，不另開一個段落——這是本檔的裁量（超出 brief 逐字要求的自我驗證），不是 spec 遺漏了什麼。
