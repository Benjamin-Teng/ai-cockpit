# stage-sync-check.js 使用說明

> 日期：2026-10-10（openspec-stage-sync task 5.1 建立，task 5.2 加入對話框與加入的段落，task 5.3 設計審核修正加入說明文字、下拉外觀與超長名稱的段落）。對象：`docs/research/2026-10-10/stage-sync-check.js`。
> 性質：腳本用法與段落代號的參考文件；內容與腳本檔頭註解一致，加段落時兩邊一起改。

## 用途

對 `cockpit --example ui_preview`（固定 fixture 起的 dashboard，不需要 HERDR）驗證
`openspec/changes/openspec-stage-sync/specs/cockpit-dashboard/spec.md`「卡片的 OpenSpec 同步標示」與
`specs/ui-language/spec.md`「OpenSpec 同步相關介面文字」中屬於卡片標示的 scenario：自動與手動各一張卡片的標示文字
（繁中與英文各一次）、手動標示較淡但計算後文字對比不低於 4.5:1、`sync` 為 `null` 的卡片沒有標示、change 名稱不以 HTML
解讀、整頁重畫後標示仍在並隨新投影更新。

task 5.2 另驗 `cockpit-dashboard` 的「編輯 stage」對話框階段下拉（初始值、選項、唯一性自動切換、送出本體每列帶 `phase`、
過期檢查涵蓋 `stage_phases`、跨整頁重畫保留）與「加入」送出的 `phases`。送出的請求本體取自 `ui_preview` stdout 的
`write-request <METHOD> <PATH> <BODY>` 記錄行（本體逐字），腳本逐字比對。

fixture：`demo-app` 的兩張卡片——`add-login`（implement 3/8、自動，在 Build）與 `fix-cache`（review 5/5、手動，在 Plan）；
其餘 Project 的 task 沒有 `sync`。

headless Chrome＋raw CDP，啟動與段落代號格式沿用 `docs/research/2026-10-08/repo-projects-check.js`。點擊用
`Input.dispatchMouseEvent` 送真的輸入，語言切換也是真的點頂列切換鈕。

## 事前準備

- **不需要 HERDR**：全程只對 `ui_preview` 操作。
- **先 build**：`cargo build -p cockpit --example ui_preview`。前端資產內嵌在執行檔裡，改了 `cockpit/assets/` 一定要重新
  build，腳本不會自己 build，也不會判斷執行檔是不是最新的。
- **需要 Chrome**：預設 `C:\Program Files\Google\Chrome\Application\chrome.exe`，可用 `COCKPIT_CHROME` 指定。
- Node 22，不需要額外套件。
- **不要與其他驗收腳本同時跑**：埠不會撞，但並行時的負載會影響計時斷言。
- **只在 Windows 上跑**：收尾用 `netstat`、`taskkill` 與 PowerShell 的 `Get-CimInstance`，執行檔固定找
  `target/debug/examples/ui_preview.exe`。

## 埠

| 用途 | 起點 | 說明 |
| --- | --- | --- |
| `ui_preview` | 7930 | `pickPort(7930)`，被占用就往上找 |
| headless Chrome 的 CDP | 19710 | `pickPort(19710)`，同上 |

選這兩個起點的理由：2026-10-10 grep `docs/research/*/*.js` 的 `pickPort(...)` 與 `PORT` 常數，其他腳本的 preview 用過
7770、7780、7790、7792、7793、7800、7830、7870、7910、7950、7970、7990，CDP 用過 9333、18781–18991、19000–19200、19310、
19410–19440、19510、19610，7930 與 19710 一帶沒有人用；跑前也以 `netstat -ano` 確認沒有人 LISTEN。

- 開跑前 7930 或 19710 已有人 LISTEN，或已有 user-data-dir 含 `cockpit-chrome-stagesynccheck-` 的 Chrome，就印
  `RESULT: FAIL (環境)` 並以 exit 2 結束。本腳本不動別人的行程，殘留的 Chrome 也只回報、不砍。
- 本腳本不使用 7770（使用者的 `cockpit.exe`）；有人 LISTEN 時只印一行「注意」。7778 是與本專案無關的 ASUS 服務，不碰。

## 執行方式

```bash
cargo build -p cockpit --example ui_preview

node docs/research/2026-10-10/stage-sync-check.js            # 全部段落
node docs/research/2026-10-10/stage-sync-check.js "card/"    # 只跑指定段落（逗號分隔）
```

- **參數**：只有一個位置參數，內容是以逗號分隔的段落代號清單。省略參數就跑全部段落；給第二個位置參數也算錯誤。
- 以 `<前綴>/` 結尾的代號選該前綴的全部段落；其餘代號必須與對照表逐字相同。
- 代號拼錯、參數是空字串或只有逗號時，在啟動任何行程之前印 `RESULT: FAIL (段落代號)`，exit 2。
- 全部段落約 1 分鐘（每段各起一組 `ui_preview`＋Chrome）。

## 輸出與結束碼

- 每條斷言一行 `ok` 或 `FAIL`；`need()` 前置條件不成立時 FAIL 並中止該段。
- 最後印段落彙總、收尾衛生，以及 `RESULT: PASS` 或 `RESULT: FAIL (n)`。
- 結束碼：全綠 0；有任何 FAIL 為 2；腳本本身例外為 1。
- 收尾衛生：本腳本 spawn 的行程都已結束、用過的埠都沒有 LISTEN、沒有殘留的 headless Chrome、`ui_preview` 的暫存副本目錄
  已刪除。只終止自己 spawn、還握著 ChildProcess 且沒觀察到 exit 的行程。

### 殘留行程以「PID＋建立時間」判定

Windows 會重用 PID。只比 PID 時，收尾階段若有別的程式剛好拿到本腳本用過的 PID（例如晚幾秒建立的 `svchost.exe`），會被誤判成
殘留。所以 spawn 當下以 `Get-CimInstance Win32_Process` 記下 PID 與建立時間，收尾只有「同 PID 且建立時間相同」才算殘留，
PID 相同但建立時間不同視為被別的程式重用，不算。spawn 當下查不到建立時間的行程會印一行注意（不掛一條永遠為真的 ok），改靠
port 與 exit 事件確認。

另外，`taskkill` 之後行程可能還要幾百毫秒才真的結束，所以「已終止」不在剛送出終止時判定，而是在 `settleChild` 裡輪詢
（至多 5 秒）到該身分的行程不存在為止。

## 新投影怎麼來

`ui_preview` 不改投影。「新投影到達」由腳本在頁面裡呼叫 `window.onState`（`channel.js` 收到 `/ws` 訊息時呼叫的同一個入口）
注入：以 `window.cockpitLatestState()` 為底、`version` 加 1，再依段落需要修改（例如把某張卡片的 `sync` 改成 `null`、把
`sync.change` 改成含 HTML 的字串、改 `checked` 與 `mode`）。注入前先把 `window.onState` 換成空函式，擋掉之後真的推送。重畫以
`#version` 的 `data-state-version` 等於注入的 `version` 判斷，不讀 `#version` 的文字。

## 前端契約

| 代號 | 定位規則 |
| --- | --- |
| S1 | Factory Floor 的 task 節點＝`[data-region="floor"] .task-node` |
| S2 | 同步標示＝節點的直接子元素 `.task-sync`（`sync` 為 `null` 時不存在）；內含 `.task-sync-change`（change 名稱）、`.task-sync-progress`（`checked/total`）、`.task-sync-mode`（「自動」「手動」或 `Auto`、`Manual`）；`.task-sync` 的 `data-sync-mode` 為 `auto` 或 `manual`，`title` 含名稱與進度 |
| S3 | 「編輯 stage」對話框＝body 底下（`#app` 之外）的 `<dialog class="project-dialog" data-dialog="stages">`；每個 `.stage-row` 有一個 `<select class="stage-phase">`，`option` 的 `value` 依序為 `""`（不對應）、`plan`、`implement`、`review`、`complete`，顯示文字走 i18n；各列下拉的 `aria-label` 不同 |

## 對比怎麼量

頁面端以 canvas 把元素的計算顏色（含 `color-mix` 的結果）轉成 sRGB 8 位元，與 `.task-node` 的計算底色（不透明的 `--surface`）
依 WCAG 2.x 相對亮度算對比。斷言：自動與手動標示的名稱、進度、模式三段文字都不低於 4.5:1；手動的對比低於自動（較淡）；
顏色都是不透明色、標示與祖先的 `opacity` 乘積為 1（不靠透明度變淡）。

## 段落代號對照表

| 代號 | 驗什麼 |
| --- | --- |
| `self/段落代號與對比計算` | `parseSegmentArg` 的合法與不合法輸入；對比計算本身（黑對白 21:1、`#777` 對白低於 4.5） |
| `card/繁中標示與無 sync 的卡片` | scenario「自動同步的卡片」「沒有同步資訊的卡片」：`add-login` 顯示 `3/8` 與「自動」、`fix-cache` 顯示 `5/5` 與「手動」，節點由上而下為 標題、狀態、同步標示、按鈕；`cockpit` Project 的 task 都沒有標示 |
| `card/手動標示較淡且對比不低於 4.5` | scenario「手動的卡片顯示較淡的手動標示」：兩種標示的三段文字對比都不低於 4.5:1，手動比自動淡，不靠 `opacity`，字級為 12px |
| `card/英文介面` | scenario「英文介面」與 ui-language「英文介面沒有繁中字串」：標示為 `Auto`、`Manual`，標示文字與 `title` 不含 CJK 字元 |
| `card/sync 為 null 與名稱不以 HTML 解讀` | scenario「沒有同步資訊的卡片」「change 名稱不以 HTML 解讀」：注入 `sync: null` 的卡片節點只剩標題、狀態、按鈕；名稱為 `<img src=x onerror=alert(1)>` 時原樣顯示、沒有建立 `<img>` |
| `card/重畫後標示仍在並隨投影更新` | `PUSH_MS=100` 頻繁整頁重畫 1.5 秒（version 至少前進 3 次）期間每次取樣兩張標示都在；注入新投影改 `checked` 與 `mode` 後標示跟著更新；切換 Project 再切回與同內容的新投影之後標示不變 |
| `dialog/階段下拉的初值與選項` | scenario「編輯 stage 對話框的階段下拉」「英文介面的階段下拉」：`stage_phases` 為 `[plan, null, complete]` 時三列顯示「規劃」「不對應」「完成」，選項依序為 不對應、規劃、實作、審查、完成；新增的列為「不對應」；`stage_phases` 比 `stages` 短時缺的視為不對應；英文介面選項為 `None`、`Plan`…，`value` 不變，對話框不含 CJK |
| `dialog/唯一性自動切換與送出本體每列帶 phase` | scenario「選到已被使用的階段時他列改回不對應」與「編輯 stage」：以真的方向鍵改選（焦點留在下拉上），他列自動改回不對應；`PATCH` 本體每列都有 `phase`（字串或 `null`），新增的列為 `null` |
| `dialog/過期檢查涵蓋 stage_phases` | scenario「階段對應在別處被改過時不送出」：只有 `stage_phases` 變（`stages` 沒變）時按儲存不送 `PATCH`、對話框不關並提示、已編輯內容保留；對照組：只改名稱的新投影不算過期，照常送出 |
| `dialog/跨整頁重畫保留下拉選擇` | `PUSH_MS=100` 頻繁整頁重畫 1.5 秒期間下拉選擇與焦點每次取樣都在；上移讓列重建後選擇跟著列走（狀態在 `dlg.rows`，不只在 DOM）；重建後再整頁重畫仍保留；送出的本體反映這些選擇 |
| `dialog/鍵盤操作與既有焦點行為` | 名稱輸入框之後的 Tab 停點是同列的下拉；Tab／Shift+Tab 循環仍包住對話框；Esc 取消不送請求、焦點回「⋯」；改了下拉後取消再開，下拉回到投影的對應 |
| `add/加入送出預設四站與 phases` | scenario「按加入送出預設 stages」：`POST /api/repo-projects` 本體恰為 `{"repo":…,"stages":[…],"phases":["plan","implement","review","complete"]}`（沒有 `name`）；繁中與英文的 `stages` 不同、`phases` 相同 |
| `dialog/說明文字與下拉外觀` | task 5.3 設計審核：對話框說明文字（繁中、英文）點出每列的下拉是 OpenSpec 階段對應；同列名稱輸入框與階段下拉等高（差 1px 內）；「不對應」的計算顏色與已選階段不同、對比不低於 4.5:1，改選階段後恢復一般顏色 |
| `card/超長 change 名稱最多兩行` | task 5.3 設計審核：超長 change 名稱的標示高度不超過兩行、內容確實被截斷、整列 `title` 與文字內容仍是完整名稱；短名稱仍單行未截斷 |
