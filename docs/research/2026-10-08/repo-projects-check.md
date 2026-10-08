# repo-projects-check.js 使用說明

> 日期：2026-10-08（repo-projects task 5.1 建立）。對象：`docs/research/2026-10-08/repo-projects-check.js`。
> 性質：腳本用法與段落代號的參考文件；內容與腳本檔頭註解一致，加段落時兩邊一起改（task 5.2、5.3 會往上加）。

## 用途

對 `cockpit --example ui_preview`（固定 fixture 起的 dashboard，不需要 HERDR）驗證
`openspec/changes/repo-projects/specs/cockpit-dashboard/spec.md`「Project 切換」中屬於前端的 scenario：左欄「偵測到的 repo」區、
「加入」送出的請求與依介面語言的預設 stages、加入後自動選定新 Project、Factory Floor 空狀態文字（task 5.1），以及 Repo Project
的「⋯」選單、改名、編輯 stage、移除確認、對話框跨重畫保留與鍵盤操作、錯誤 code 的介面語言顯示（task 5.2），以及 Factory Floor 工作線列首的 worktree 標註與固定 pane（`source: pane`）不顯示改綁鈕（task 5.3）。

腳本目前共 28 段：1 段自我測試（`self/`）、27 段 spec scenario 與相關邊界（`cockpit-dashboard/`；task 5.1 12 段、其中 3 段是
fix round 1 加的；task 5.2 14 段、其中 2 段是 fix round 1、1 段是 fix round 2 加的；task 5.3 1 段）。新斷言依 tasks.md 通則先在未改的前端跑出紅，再改前端轉綠（紅綠紀錄見
`.superpowers/sdd/tasks-repo-projects/task-5.1-report.md`、`task-5.2-report.md`、`task-5.3-report.md`）。

headless Chrome＋raw CDP。啟動、收尾、行程所有權模型、段落代號與輸出格式沿用 `docs/research/2026-10-04/split-check.js`。
點擊與按鍵都用 `Input.dispatchMouseEvent`／`Input.dispatchKeyEvent` 送真的輸入；語言切換也是真的點頂列的切換鈕。

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
| `ui_preview` | 7950 | `pickPort(7950)`，被占用就往上找 |
| headless Chrome 的 CDP | 19610 | `pickPort(19610)`，同上 |

選這兩個起點的理由：2026-10-08 grep `docs/research/*/*.js` 的 `pickPort(...)` 與 `PORT` 常數，其他腳本的 preview 用過
7770、7780、7792、7793、7830、7870、7910、7970、7990，CDP 用過 18781–18991、19000–19200、19310、19410–19440、19510 與 9333，
7950 與 19610 一帶沒有人用。

- 開跑前 7950 或 19610 已有人 LISTEN，或已有 user-data-dir 含 `cockpit-chrome-repoprojcheck-` 的 Chrome，就印
  `RESULT: FAIL (環境)` 並以 exit 2 結束。本腳本不動別人的行程，殘留的 Chrome 也只回報、不砍。
- 本腳本不使用 7770（使用者的 `cockpit.exe`）；有人 LISTEN 時只印一行「注意」。7778 是與本專案無關的 ASUS 服務，不碰。

## 執行方式

```bash
cargo build -p cockpit --example ui_preview

node docs/research/2026-10-08/repo-projects-check.js                       # 全部段落
node docs/research/2026-10-08/repo-projects-check.js "cockpit-dashboard/"  # 只跑指定段落（逗號分隔）
```

- **參數**：只有一個位置參數，內容是以逗號分隔的段落代號清單。省略參數就跑全部段落；給第二個位置參數也算錯誤。
- 以 `<前綴>/` 結尾的代號選該前綴的全部段落；其餘代號必須與對照表逐字相同。
- 代號拼錯、參數是空字串或只有逗號時，在啟動任何行程之前印 `RESULT: FAIL (段落代號)`，exit 2。
- 全部段落約 3 分鐘（每段各起一組 `ui_preview`＋Chrome）。

## 輸出與結束碼

- 每條斷言一行 `ok` 或 `FAIL`；`need()` 前置條件不成立時 FAIL 並中止該段（後面的斷言建立在它之上）。
- 最後印段落彙總（每段 `PASS` 或 `FAIL(n)` 與第一條失敗）、收尾衛生，以及 `RESULT: PASS` 或 `RESULT: FAIL (n)`。
- 結束碼：全綠 0；有任何 FAIL 為 2；腳本本身例外為 1。
- 收尾衛生：本腳本 spawn 的行程都已結束、用過的埠都沒有 LISTEN、沒有殘留的 headless Chrome、`ui_preview` 的暫存副本目錄
  已刪除。只終止自己 spawn、還握著 ChildProcess 且沒觀察到 exit 的行程。

## 新投影怎麼來

`ui_preview` 的寫入端點只記錄請求（stdout 一行 `write-request <METHOD> <PATH> <BODY>`，腳本收進 `preview.writes`）、
不改投影（repo-projects design Risks「`ui_preview` 沒有真的後端狀態」）。`POST /api/repo-projects` 的本體以正式端點的型別與
core 的正規化規則驗證，合法時回 201 `{"id": ...}`。`PATCH`／`DELETE /api/repo-projects/<pid>` 不驗本體、回 204。

要模擬被拒絕時用 `COCKPIT_PREVIEW_WRITE_RULES`（只比對路徑、不分方法）。狀態碼後可再接 `:<code>`（task 5.2 擴充），錯誤本體就多帶
`"code"`，例如 `/api/repo-projects/demo-app=0:400:invalid_stages` 回 `{"error":"ui_preview 模擬回應 400","code":"invalid_stages"}`：
繁中介面顯示 `error` 原文，英文介面依 `code` 顯示字典的 `msg.invalid_stages`。

所以「含新 Project 的投影到達」由腳本在頁面裡呼叫 `window.onState`（`channel.js` 收到 `/ws` 訊息時呼叫的同一個入口）注入：
以 `window.cockpitLatestState()` 為底、`version` 加 1，再依段落需要修改（例如把 `billing-api` 從 `detected_repos` 拿掉、在
`projects` 最後加一個 `kind: repo` 的 Project；它的 workstreams 與 tasks 留空，因為真後端一個 pane 只屬於一個 repo）。注入前先把 `window.onState` 換成空函式，擋掉之後真的推送，做法同
`docs/research/2026-09-23/visual-check.js` 的 `injectState()`。重畫以 `#version` 的 `data-state-version` 等於注入的
`version` 判斷，不讀 `#version` 的文字。

## 前端契約

| 代號 | 定位規則 |
| --- | --- |
| C1 | 左欄 Project 分頁＝`[data-region="projects"]`；Project 項目＝`[data-action="select-project"][data-project]`，選定者帶 `.selected` |
| C2 | 偵測區＝`[data-region="projects"]` 內的 `.detected-repos`，標題 `.detected-repos-title`、無項目時的說明 `.detected-repos-empty`；每項＝`.detected-repo[data-repo]`，名稱 `.detected-repo-name`、pane 數 `.detected-repo-count`、加入鈕 `[data-action="add-repo"][data-repo]`（進行中或已成功、投影尚未反映時帶 `aria-disabled="true"`，不用 `disabled` 屬性） |
| C3 | Factory Floor 顯示的 Project＝`[data-region="floor"] .project[data-project]`；空狀態＝`.floor-empty-state`；左欄 Project 清單的空狀態＝`.projects-empty-state` |
| C4 | 「⋯」＝`[data-region="projects"] [data-action="project-menu"][data-project]`（`<button>`，`aria-expanded`、`aria-controls` 指向選單 id）；選單＝`.project-menu[data-project]`，項目 `[data-action="project-rename"\|"project-edit-stages"\|"project-remove"][data-project]` |
| C5 | 對話框＝`dialog.project-dialog`（body 底下、`#app` 之外，一直存在；開啟時有 `open` 屬性，`data-dialog` 為 `rename`／`stages`／`remove`）。標題 `.project-dialog-title`、說明 `.project-dialog-text`、改名輸入 `input.project-dialog-name`、stage 列 `.stage-row`（輸入 `input.stage-name`，列內 `[data-stage-op="up"\|"down"\|"delete"]`）、新增 `[data-stage-op="add"]`、底部 `[data-dialog-op="cancel"\|"submit"]`、錯誤 `.project-dialog-error`（沒有 `hidden` 時即顯示） |

## 段落代號對照表

| 代號 | 驗什麼 |
| --- | --- |
| `self/段落代號` | `parseSegmentArg` 的合法與不合法輸入；以拼錯的代號與空字串執行本檔時 exit 2、不啟動行程 |
| `cockpit-dashboard/列出偵測到的 repo` | scenario「列出偵測到的 repo」：偵測區在 Project 清單之後，依序列出 `billing-api`（2 個 pane）與 `Docs Site`（1 個 pane），各有「加入」鈕（無障礙名稱含 repo 名稱）；英文介面同樣列出（`2 panes`／`1 pane`、`Add`）；只是重畫不送請求 |
| `cockpit-dashboard/按加入送出預設 stages` | scenario「按加入送出預設 stages」：滑鼠按「加入」與鍵盤 Enter 各送一筆 `POST /api/repo-projects`，本體恰為 `{"repo":…,"stages":["規劃","實作","審查","完成"]}`（沒有 `name`）；滑鼠按下後焦點在該鈕上、不呈現焦點外框；點頂列切換成英文後再按，`stages` 為 `Plan`、`Implement`、`Review`、`Complete` |
| `cockpit-dashboard/加入成功後自動選定新 Project` | scenario「加入成功後自動選定新 Project」：先選定 `p`，按加入、收到 201 後選取不變，偵測區仍列著它時「加入」為 `aria-disabled`、再按不送出；不含新 id 的投影不觸發；含 `billing-api` 的投影到達後它從偵測區消失、出現在 Project 清單並被選定、Factory Floor 顯示它；點回 `p` 後下一份投影不會再切走 |
| `cockpit-dashboard/加入回應晚於新投影時仍自動選定` | 以 `COCKPIT_PREVIEW_WRITE_RULES=/api/repo-projects=1500:201` 讓回應延遲：含新 id 的投影先到時不選，201 到達後選定 |
| `cockpit-dashboard/加入被拒絕時不自動選定` | `COCKPIT_PREVIEW_WRITE_RULES=/api/repo-projects=0:409`：顯示含原因的錯誤訊息；之後含該 repo 的投影到達也不選定，錯誤不被重畫清除；repo 回到偵測區時「加入」可按、再按送出第二筆 |
| `cockpit-dashboard/頻繁重畫時加入鈕的鍵盤焦點` | `COCKPIT_PREVIEW_PUSH_MS=100`：焦點在「加入」上 2 秒（至少重畫 10 次）始終不掉，按 Enter 照常送出；新 Project 被選定、「加入」鈕消失時鍵盤焦點移到新選定的 Project 項目上 |
| `cockpit-dashboard/空狀態指向偵測到的 repo` | scenario「空狀態指向偵測到的 repo」：`projects` 為空、`detected_repos` 有 `app` 時，左欄沒有任何 Project 項目，Factory Floor 的空狀態指向「偵測到的 repo」（英文 `Detected repos`）且不含「重啟」（英文不含 Restart）；左欄 Project 清單的空狀態仍是「沒有 Project」，偵測區列出 `app` 與「加入」鈕；繁中與英文各一次 |
| `cockpit-dashboard/沒有偵測到的 repo` | `detected_repos` 為空時偵測區仍在、不列任何項目、沒有「加入」鈕，只有一行說明 |
| `cockpit-dashboard/偵測區名稱不以 HTML 解讀` | scenario「名稱不以 HTML 解讀」的偵測區部分：名稱含 `<img … onerror=…>` 時原樣顯示、沒有建立 `<img>`、沒有執行指令碼；按加入送出的 `repo` 是該項的 repo key |
| `cockpit-dashboard/連點兩下加入只送一筆` | 以 `Input.dispatchMouseEvent` 送 clickCount 1→2 的雙擊：只送一筆 POST、沒有錯誤橫幅 |
| `cockpit-dashboard/加入進行中不重送且跨重畫保留` | `WRITE_RULES` 延遲 1500 ms＋`PUSH_MS=100`：進行中「加入」跨重畫維持 `aria-disabled="true"`（沒有 `disabled` 屬性）、焦點留在它上面，按 Enter 不送出；新 Project 的投影讓 repo 離開偵測區後解除，repo 再出現時可按 |
| `cockpit-dashboard/投影到達前手動切換則不自動選定` | 201 之後、含新 id 的投影到達之前手動選定 `p`：投影到達後仍是 `p`（使用者明確選擇優先） |
| `cockpit-dashboard/只有 Repo Project 有選單` | scenario「只有 Repo Project 有選單」：只有 `kind: repo` 的 `demo-app` 有「⋯」（`<button>`、文字 ⋯、無障礙名稱「管理 Demo App」／`Manage Demo App`），手寫的與 `kind` 為未知新值的都沒有；開啟後 `aria-expanded="true"`、`aria-controls` 指向選單，依序為「改名」「編輯 stage」「移除」（英文 Rename、Edit stages、Remove）；Esc、再按「⋯」、點別的 Project 都會收起，Esc 後焦點回到「⋯」；開關選單不送請求、不改變選取 |
| `cockpit-dashboard/改名` | scenario「改名」：對話框是 `#app` 之外的 modal `<dialog>`，預填目前名稱、焦點在輸入框；輸入 `App 前端` 按儲存 → `PATCH /api/repo-projects/demo-app`，本體恰為 `{"name":"App 前端"}`；204 後對話框關閉、焦點回到「⋯」且不呈現外框；新投影到達前畫面不自行改名，到達後左欄與 Factory Floor 標題顯示新名稱；在輸入框按 Enter 也送出 |
| `cockpit-dashboard/編輯 stage` | scenario「編輯 stage」：stages 為 Plan、Implement、Review、Done 時把 Implement 改名為 Build、刪除 Review、在最後新增 Ship → 本體恰為 spec 的那份（新增的 `from` 為 `null`）；再開時不留上次的編輯；上移／下移只調整順序（`from` 為各自的名稱、焦點跟著列走、第一列「上移」與最後一列「下移」為 `aria-disabled`、按了不動作）；刪除後新增同名的 stage，新增那列的 `from` 仍為 `null` |
| `cockpit-dashboard/取消編輯與取消移除不送請求` | scenario「取消編輯與取消移除不送請求」：stage 對話框改了內容後按「取消」、移除確認按「取消」、改名按 Esc，服務沒有收到任何 `PATCH` 或 `DELETE`；焦點回到「⋯」；再開時 stages 回到投影的值 |
| `cockpit-dashboard/移除要先確認` | scenario「移除要先確認」：先顯示含「Demo App」的確認、尚未送出；確認後 `DELETE /api/repo-projects/demo-app`（沒有本體）、對話框關閉；移除後的投影到達時 `demo-app` 從清單消失、原本選定它則改選第一個；英文確認含 `"Demo App"`、按鈕為 Remove；fix round 1：「⋯」隨 Project 消失後焦點落在選定的 cockpit 項目 |
| `cockpit-dashboard/對話框跨重畫保留` | scenario「對話框跨重畫保留」：`PUSH_MS=100`，選單開著時跨重畫仍開、焦點留在第一個項目；stage 對話框改了一半的名稱在 2 秒（至少重畫 10 次）內內容與焦點都保留，接著輸入的字接在後面，儲存送出完整名稱；移除確認同樣保留 |
| `cockpit-dashboard/對話框鍵盤操作與焦點回歸` | 鍵盤：在「⋯」按 Enter 開選單、焦點到「改名」並呈現外框；Tab 到「編輯 stage」按 Enter 開對話框、焦點進入第一個輸入框；連按 Tab 一圈焦點都在對話框內並回到第一個，Shift+Tab 到「儲存」；Esc 關閉、焦點回到「⋯」並呈現外框；移除確認同樣；滑鼠開啟後 Esc 也回到「⋯」 |
| `cockpit-dashboard/對話框送出前的基本提示` | 空白的 stage 名稱、重複的 stage 名稱（去除前後空白後相同）、沒有任何 stage、空白的 Project 名稱 → 不送出、對話框內顯示原因、焦點移到該欄，不寫進頁面錯誤橫幅；修正後提示消失並照常送出；長度規則交給後端：65 字元的名稱照樣送出 |
| `cockpit-dashboard/管理操作被拒絕時顯示原因` | scenario「被拒絕時顯示原因」與「錯誤代碼可翻譯」：`WRITE_RULES` 讓 `PATCH`／`DELETE` 回 400 `invalid_stages`（另一組 `invalid_name`）：對話框不關、已編輯的內容保留、「儲存」恢復可按；繁中顯示回應的 `error`，英文依 `code` 顯示字典的英文訊息，對話框內與頁面錯誤橫幅都有；關閉對話框與之後的重畫都不清除橫幅；移除被拒絕時確認不關 |
| `cockpit-dashboard/送出進行中不重送` | `WRITE_RULES` 延遲 4000 ms：進行中「儲存」為 `aria-disabled`＋`aria-busy`（沒有 `disabled` 屬性），再按與在輸入框按 Enter 都不送第二筆；fix round 1：進行中「取消」為 `aria-disabled`，按「取消」與連按 Esc 都不關閉；204 到達後對話框關閉，再開時「取消」可按 |
| `cockpit-dashboard/對話框名稱不以 HTML 解讀` | scenario「名稱不以 HTML 解讀」的選單與對話框部分：Project 名稱與 stage 名稱含 HTML 時，「⋯」的無障礙名稱、stage 對話框標題與輸入框、改名輸入框、移除確認都是原樣文字；頁面沒有多建立 `<img>`、沒有執行指令碼 |
| `cockpit-dashboard/Project 消失時焦點落在選定的 Project` | fix round 1：選單開著、焦點在選單項目上時 `demo-app` 從投影消失 → 選單收起、焦點落在選定的 `p` 項目；對話框開著時 `demo-app` 消失、按「取消」→ 同樣落在 `p`；焦點在「⋯」上時所有 Project 消失 → 落在偵測區標題，不落在回到偵測區的 repo 的「加入」，按 Enter 不送出加入 |
| `cockpit-dashboard/stages 已在別處變更時不送出` | fix round 1：stage 對話框開著時投影的 stages 被改 → 儲存不送出、對話框內提示「已在別處變更」、已編輯內容保留，重新開啟列出最新 stages；改名對話框開著時 Project 被移除 → 儲存不送出、提示「已不存在」 |
| `cockpit-dashboard/偵測區標題的焦點跨重畫保留` | fix round 2：焦點在「⋯」上時所有 Project 消失、焦點落到偵測區標題；之後每 100 ms 以 `onState` 重送 version＋1 的投影，2 秒內（至少重畫 10 次）`document.activeElement` 始終是新的標題節點 |
| `cockpit-dashboard/Repo Project 工作線的 worktree 標註與固定 pane 無改綁鈕` | task 5.3：demo-app 的 `win~wJ:p7`（`worktree` 為 `demo-app-wt`）列首顯示 worktree 標註（`.ff-worktree`，單一文字節點，`title` 依語言為「worktree：…」／「Worktree: …」）、`win~wJ:p6` 沒有；兩列（`source: pane`）列首只剩「看輸出」，沒有「改綁」「取消改綁」與「改綁」徽章；手寫 project cockpit 的 auto／override 列照舊有「改綁」；注入 `source: pane` 的 bound／unbound／runtime_disconnected 都沒有改綁鈕，`override` 照舊「改綁」＋「取消改綁」＋徽章、`auto` 只有「改綁」；worktree 含 HTML 時以原樣文字顯示、空字串不顯示；改綁模式下整頁重畫與一般重畫後仍正確；中英各驗一次 |

行為面（卡片回到第一個 stage、移除後回到偵測區）由後端整合測試與
task 7.3 真機冒煙驗證，不在本腳本。
