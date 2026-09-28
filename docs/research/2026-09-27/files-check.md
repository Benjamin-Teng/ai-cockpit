# files-check.js 使用說明

> 日期：2026-09-27（file-review task 3.5）。對象：`docs/research/2026-09-27/files-check.js`。
> 性質：腳本用法、段落代號與前端契約的參考文件；內容與腳本檔頭註解一致，改契約時兩邊一起改。

## 用途

對 `cockpit --example ui_preview`（固定 fixture 起的 dashboard，不需要 HERDR）逐條驗證
`openspec/changes/file-review/specs/` 中屬於前端的 scenario：

- `file-review`：「左欄檔案樹」「檔案分頁」「檔案檢視器」「自動更新」「分頁還原」「在 VS Code 開啟」的全部 scenario。
  後端純 API 的 scenario（根目錄推算、端點共同規則、列目錄、中繼資料、渲染、原始內容、檔案 icon）由 Rust 測試守，不在本檔。
- `live-output` delta 新增的三個 scenario：「選定 pane 時切回 Live Output 分頁」「檔案分頁期間不請求輸出」「切回時保持貼底」。

`cockpit-dashboard` delta 的「分頁很多不撐破頁面」「頻繁重畫不影響檔案分頁」「Markdown 檢視遵守色彩與對比」三段放在
`docs/research/2026-09-23/visual-check.js`（代號 `FT1`／`FT2`／`FT3`），用的是同一份前端契約。

這是「先寫測試」：file-review 4.x 的前端落地前，`self/` 開頭的自我測試段必須通過，其餘每個 scenario 段都依預期失敗
（RED），失敗訊息指出缺的是哪一條前端契約。某段在前端還沒做時竟然通過，代表斷言太弱。

headless Chrome＋CDP，啟動、收尾、行程所有權模型與段落代號寫法沿用 `visual-check.js`；輸出請求計數與貼底判斷沿用
`docs/research/2026-09-19/live-output-check.js`。

## 事前準備

- **不需要 HERDR**：全程只對 `ui_preview` 操作。
- **先 build**：`cargo build -p cockpit --example ui_preview`。前端資源內嵌在執行檔裡，改了 `cockpit/assets/` 一定要重新
  build，腳本不會自己 build，也不會判斷執行檔是不是最新的。
- **需要 Chrome**：預設 `C:\Program Files\Google\Chrome\Application\chrome.exe`，可用 `COCKPIT_CHROME` 指定。
- Node 22，不需要額外套件。
- **不要與其他驗收腳本同時跑**：並行時各腳本的計時斷言與收尾的行程、埠清查會互相干擾。`ui_preview` 啟動時先綁定埠、
  綁定成功後才清理 `%TEMP%` 下前一次沒收掉的 `cockpit-ui-preview-<PID>-*`，而且只刪 PID 已不存在的那些（PID 解不出來或
  查不到存活狀態就保留；見 `cockpit/examples/ui_preview.rs` 的 `should_delete_stale_dir`），不會刪到仍在執行的實例的
  暫存副本。本腳本開跑前若偵測到有 `ui_preview.exe` 在跑、或 `127.0.0.1:7770` 有人 LISTEN，就印
  `RESULT: FAIL (環境)` 並以 exit 2 結束，不動別人的行程。

## 執行方式

```bash
cargo build -p cockpit --example ui_preview

node docs/research/2026-09-27/files-check.js                                    # 全部段落
node docs/research/2026-09-27/files-check.js "self/鷹架,file-review/中文 PDF"    # 只跑指定段落（逗號分隔）
node docs/research/2026-09-27/files-check.js live-output/                       # 以「capability/」選該前綴的全部段落
node docs/research/2026-09-27/files-check.js --scratch=D:\tmp\shots              # 截圖目錄
```

- 段落代號含空白時整個參數要加引號。
- 代號拼錯、參數是空字串或只有逗號時，在啟動任何行程之前印 `RESULT: FAIL (段落代號)`，exit 2。
- 截圖目錄（「中文 PDF」段存 viewport 截圖供目視）：`--scratch=<目錄>` 優先，其次環境變數 `FILES_CHECK_SCRATCH`，
  預設 `%TEMP%`。截圖路徑印在輸出中。

## 段落代號對照表

| 代號 | spec（capability／Requirement） | 做法摘要 |
| --- | --- | --- |
| `self/鷹架` | 腳本自我測試 | 啟動 ui_preview 並讀到暫存副本路徑；CDP 連上並等到首份投影；網路請求依 URL 分類；Fetch 攔截改寫中繼資料；message 監聽器；改寫暫存副本再還原；以 vendored pdf.js 校準 PDF 像素偵測器（正負對照）；以合成 DOM 驗前端契約的定位規則；以合成 DOM 驗檔案樹「節點沒被換掉」偵測器（重設某列 innerHTML、換掉捲動容器都必須轉紅）；`analyzeOutputAcrossSwitch` 六個純函式案例，以及在目前前端改真的 Live Output tab 的 `aria-selected` 做 (a)–(d) 正負對照（見「請求分類方式」）；收尾衛生（行程、埠、暫存目錄、repo 內 fixture 雜湊不變） |
| `self/段落代號` | 腳本自我測試 | `parseSegmentArg` 對合法、拼錯、大小寫不符、未知前綴、空字串、只有逗號各驗一次；實際以錯的代號與空字串執行本檔，必須 exit 2、不啟動行程 |
| `file-review/切到檔案分頁` | file-review／左欄檔案樹 | 選定 `wJ:p4` 後點「檔案」：頂端顯示 `review-repo`、`win` 與「重新整理」；第一層列＝列目錄端點回傳的子項目（集合） |
| `file-review/沒有選定 pane` | file-review／左欄檔案樹 | 不選 pane 就切到「檔案」：顯示空狀態文案；從導覽起沒有任何檔案端點（含根目錄查詢）的請求 |
| `file-review/展開狀態跨根目錄保留` | file-review／左欄檔案樹 | 展開 `src` → 改選 `wJ:p5`（other-repo）→ 改選回 `wJ:p4`：`src` 仍展開 |
| `file-review/重畫不影響檔案樹` | file-review／左欄檔案樹 | 推送 100 ms；暫存副本加 `many/`（80 個檔）讓樹可捲；捲到中段、焦點在檔案列；3 秒後 tree、每一列與列內所有子孫節點都相同；捲動容器仍是原本那一個且在頁面上，捲動位置讀目前的捲動容器；展開狀態與焦點不變 |
| `file-review/同一 pane 改 cwd 後檔案樹跟著換根` | file-review／左欄檔案樹（最終修正波 F1：根目錄改變時讀取） | `COCKPIT_PREVIEW_PUSH_MS=300`、`COCKPIT_PREVIEW_CWD_TO_OTHER_REPO=wJ:p4=7000`（7 秒後服務端把 `wJ:p4` 的 cwd 改成 other-repo，pane id 不變）；選 `wJ:p4`、左欄「檔案」顯示 review-repo 後等服務端改 cwd：5 秒內頂端換成 other-repo、第一層＝other-repo 的子項目，不需重新整理或切換分頁；觀察期間（至少 10 份投影）根目錄查詢恰 1 次 |
| `file-review/開檔新增分頁` | file-review／檔案分頁 | 只有 Live Output 時點 `README.md`：分頁數 1 → 2、README 為目前分頁、Live Output 仍第一；title 含路徑與根目錄名稱 |
| `file-review/重複開啟不新增` | file-review／檔案分頁 | 開 `README.md`、`docs/a.md` 後再點 `README.md`：分頁數不變、目前為 README |
| `file-review/關閉目前分頁` | file-review／檔案分頁 | 暫存副本加 `a.md`／`b.md`／`c.md`；選 b 後按關閉：目前為 c |
| `file-review/切換 Project 不影響分頁` | file-review／檔案分頁 | 開 README 後切到左欄 Project、點另一個 Project：README 分頁仍在且為目前分頁 |
| `file-review/md 相對連結在分頁區開啟` | file-review／檔案檢視器 | 點 README 內「設計」：`docs/design.md` 分頁為目前分頁，「決策」標題在內容捲動容器可視範圍內且 `scrollTop > 0`，整頁沒有離開 |
| `file-review/外部圖片不載入` | file-review／檔案檢視器 | CDP Network：沒有任何對 `example.com` 的請求；內容顯示替代文字 `logo`；`raw/docs/pic.png` 請求 200 且圖片已載入 |
| `file-review/HTML 內的腳本不執行` | file-review／檔案檢視器 | 頁面 `message` 監聽器沒收到 `ran`；iframe 帶 `sandbox` 且不含 `allow-scripts`／`allow-same-origin`；`raw/style.css` 請求 200（含 OOPIF 子 session 的網路事件） |
| `file-review/中文 PDF` | file-review／檔案檢視器 | 工具列可見文字「1 / 3」；3 個 canvas 逐一捲入可視範圍後非背景像素比例 > 0.2%；第 1 頁標題帶（頁高 4%–11.5%）> 1%；存 viewport 截圖，存不下來判 FAIL，存下來印一行「需目視確認（非方框）：<路徑>」。**本段 PASS 不含「非方框」判定**，要搭配控制端目視截圖 |
| `file-review/純文字不被解讀` | file-review／檔案檢視器 | `note.txt`：可見文字含 `<b>x</b>` 原字樣，tabpanel 內沒有 `b` 元素 |
| `file-review/改檔後更新並保住捲動` | file-review／自動更新 | `long.md` 捲到中段，在暫存副本末端加一段：3 秒內出現，`scrollTop` 不變（±1 px） |
| `file-review/Live Output 分頁時不查詢` | file-review／自動更新 | 先確認檔案分頁期間有中繼資料查詢；切到 Live Output 後 10 秒：頁面端記錄（fetch／XHR，發出當下同步記下 Live Output 是否為目前分頁）中，Live Output 為目前分頁時發出的中繼資料請求為 0；CDP 總數健全檢查（見「請求分類方式」） |
| `file-review/檔案被刪掉後又出現` | file-review／自動更新 | 暫存副本加 `plan.md`；刪除後可見文字含「過期」「檔案已不存在」且保留舊內容；5 秒後重建，3 秒內顯示新內容、兩者消失 |
| `file-review/重新整理後還原` | file-review／分頁還原 | 開 README、`docs/a.md`（目前）、左欄「檔案」後 `Page.reload`：順序、目前分頁、內容、左欄分頁都還原 |
| `file-review/儲存內容損毀` | file-review／分頁還原 | 先讓前端寫入，把當下所有 localStorage 值改成 `{not json` 後重載：只有 Live Output 分頁；pane 列、Factory Floor、左欄分頁、Live Output 面板正常；console 有警告、沒有未捕捉例外 |
| `file-review/Windows 檔案` | file-review／在 VS Code 開啟 | 暫存副本加 `docs/a b.md`：中繼資料 `vscode_uri`＝由暫存副本路徑算出的 `vscode://file/<磁碟>:/…/docs/a%20b.md`，連結 `href` 等於它 |
| `file-review/WSL 檔案` | file-review／在 VS Code 開啟 | 暫存副本加 `a.md`；以 CDP Fetch 把其中繼資料回應的 `vscode_uri` 改成 spec 的 WSL 形狀；連結 `href` 照原值顯示 |
| `live-output/選定 pane 時切回 Live Output 分頁` | live-output／選定一個 pane | 已選 `wJ:p4`、目前為 README 分頁時點 `wJ:p1` 列：Live Output 成為目前分頁、面板可見並顯示 `wJ:p1`、README 分頁仍在 |
| `live-output/檔案分頁期間不請求輸出` | live-output／輪詢與顯示 | 延遲回應情境 `wJ:p4=delay:1500`，確認切走當下有進行中的輸出請求才切走；以頁面端記錄（fetch／XHR，發出與 settle 當下同步記下 Live Output 是否為目前分頁）逐筆分類：切走前發出、切走後完成的至多 1 筆；檔案分頁期間發出的 0 筆；點 Live Output tab 後 250 ms 內發出新請求；CDP 總數健全檢查（見「請求分類方式」） |
| `live-output/切回時保持貼底` | live-output／輪詢與顯示 | `wJ:p4=long`（內容超過一屏）停在最底端；切到檔案分頁 5 秒後切回：立即在最底端，3 秒內最後一行 ≥ 切走時 +4 且仍貼底 |

spec scenario 裡的 pane 對應：`w1:p1`→`wJ:p4`（cwd＝review-repo/src）、`w2:p1`→`wJ:p5`（cwd＝other-repo）、`w1:p2`→`wJ:p1`
（ticker 輸出）。需要額外檔案或改寫檔案的段落一律只寫 ui_preview 的暫存副本（路徑取自 stdout 的 `review-repo: <路徑>`），
不碰 `cockpit/examples/fixtures/review-repo/`。

## 前端契約

給 file-review 4.x 前端照做。標「spec」的是 spec／design 明定；標「腳本約定」的是本腳本約定、可與前端協調（改了要同步改
`files-check.js` 的 `pageHelpers()` 與 `visual-check.js` 的 `filesContractHelpers()`）。

| 代號 | 內容 | 來源 |
| --- | --- | --- |
| C1 左欄分頁 | `#files` 內第一個 `role="tablist"`，兩個 `role="tab"`，可見文字恰為「Project」「檔案」；目前分頁 `aria-selected="true"` | `#files` 與兩個分頁名稱：spec／design D6；role 與 `aria-selected`：腳本約定 |
| C2 檔案樹 | `#files` 內 `role="tree"`；每一列一個 `role="treeitem"`，**扁平排列**（以 `aria-level` 表示深度，子列不巢狀放在 treeitem 內，點列的中心一定點到該列）；列的 `title` 恰為完整相對路徑（`/` 分隔）；資料夾列帶 `aria-expanded`；列可聚焦 | `title`、`aria-expanded`、鍵盤操作：spec；`role`、扁平排列：腳本約定 |
| C2 樹頂端 | tree 與左欄 tablist 之外的可見文字含根目錄 `name` 與 runtime `id`；有可見文字「重新整理」的 `<button>`；沒有選定 pane 時 `#files` 可見文字含「先在 Factory Floor 或 runtime 清單選一個 pane」 | spec |
| C2 捲動容器 | 檔案樹的捲動容器是 tree 本身，或它在 `#files` 內的祖先 | 腳本約定 |
| C3 分頁列 | `#review` 內 `role="tablist"`，每個分頁一個 `role="tab"`；第一個是 Live Output，可見文字含「Live Output」；檔案分頁可見文字含檔名、`title` 含完整相對路徑與根目錄名稱、帶 `data-path`＝相對路徑；目前分頁 `aria-selected="true"` | `#review`、`role="tablist"`、`title`：spec／design D6；其餘：腳本約定 |
| C3 分頁內容 | 每個 tab 以 `aria-controls` 指向自己的 `role="tabpanel"`；非目前分頁的 tabpanel 設 `hidden`；Live Output 的 tabpanel 內含 `#output`；切換分頁時先更新 `aria-selected`，再開始／停止輪詢 | `hidden` 切換與 `#output` 移入 `#review`：design D6；`aria-controls`／`role="tabpanel"`：腳本約定；更新順序：腳本約定（請求分類以發出當下的 `aria-selected` 為準） |
| C4 關閉按鈕 | `<button>`，`aria-label`（沒有時用可見文字）以「關閉」開頭；放在該 tab 內，或 tab 的包裝元素內（包裝元素不是 tablist 本身） | 腳本約定 |
| C5 工具列 | 目前 tabpanel 內一個 `<a>`，可見文字恰為「在 VS Code 開啟」，`href`＝中繼資料 `vscode_uri` 原值；`vscode_uri` 為 null 時不顯示 | spec |
| C6 檢視器 | 目前 tabpanel 內一個 `data-viewer="markdown｜text｜html｜pdf｜unsupported"` 的元素包住內容；內容捲動容器是它本身、它在 tabpanel 內的祖先，或 tabpanel 內第一個可捲動的子孫 | 腳本約定 |
| C6 PDF | 每頁一個 `<canvas>`（2d context，畫完後 `getImageData` 讀得到內容）；「目前頁 / 總頁數」以可見文字出現在 tabpanel 內（例如「1 / 3」） | canvas：design D7；「1 / 3」：spec；2d context：腳本約定 |
| C6 HTML | tabpanel 內的 `<iframe>`，`sandbox` 屬性不含 `allow-scripts` 與 `allow-same-origin` | spec |
| C7 過期標示 | 讀取失敗時保留內容，目前 tabpanel 的可見文字含「過期」，原因以可見文字呈現（例如「檔案已不存在」）；恢復後兩者都不在可見文字中 | 原因文案：spec；「過期」字樣：腳本約定（比照 Live Output 的「過期」） |
| C8 分頁還原 | 存在 `localStorage`，鍵名不限；讀到損毀值時 `console.warn` | 「console 有警告」：spec；鍵名不限：腳本約定（「儲存內容損毀」段把當下所有 localStorage 值改成非法 JSON） |

既有 DOM（不是本 change 新增，腳本直接沿用）：pane 列 `.pane-row[data-runtime][data-pane]`（選定時加 `.selected`）、Project
項目 `button[data-action="select-project"][data-project]`（目前的帶 `aria-current="true"`）、Live Output 內容
`#output .output-text`、標題 `#output .output-title`。

## 輸出格式

- `ok  <說明>`／`FAIL <說明>`：每個斷言一行。前置條件不成立時記一條 `FAIL` 並中止該段，不再產生連帶失敗。
- `[<timestamp>] <說明>`：過程訊息，例如段落開頭 `=== <代號> ===`、PDF 偵測器校準值、截圖路徑。
- 結尾固定印「段落彙總」：每段一行 `PASS` 或 `FAIL(<n>) <代號> — <第一個失敗>`，自我測試段標 `[自我測試]`，另有一行
  `[收尾衛生]`；再印一行「自我測試段 x/y PASS；scenario 段 x/y FAIL；收尾衛生 PASS／FAIL」，最後 `RESULT: PASS` 或
  `RESULT: FAIL (<n>)`（exit 2）。自我測試段與收尾衛生必須 PASS；scenario 段依前端進度，尚未實作的應 FAIL（file-review 4.1 之後「沒有選定 pane」已可 PASS）。

## 清理行為

- 開跑前不殺任何別人的行程；只清掉上一次本腳本殘留、`user-data-dir` 含 `cockpit-chrome-filescheck-` 的 headless Chrome。
- 每段自己開一個 `ui_preview`（埠從 7870 起挑空的）與一個 headless Chrome（CDP 埠從 19310 起），段落結束時依
  「還握著 `ChildProcess` 且沒觀察到 exit 才終止」收尾，確認 PID 消失、埠不再 LISTEN；`ui_preview` 被 `taskkill /F`
  結束時沒有機會自己刪暫存副本，由腳本刪（只刪名稱以 `cockpit-ui-preview-` 開頭的那一個目錄）。
- 結尾再做一次總清查：7770 沒有 LISTEN、沒有殘留的 `ui_preview.exe` 與本腳本的 Chrome、本腳本開過的暫存目錄都已刪除。

## 已知限制

- **像素比例分不出「字」與「方框」**：「中文 PDF」段的標題判準只能證明標題帶有墨跡，這段的 PASS 不代表中文沒有變成方框；
  「非方框」一律由控制端目視輸出中「需目視確認（非方框）：」那一行的截圖判定（控制端裁決 R22）。截圖存不下來時本段 FAIL。
- **前端尚未實作的部分，scenario 段只跑到第一個缺的前端契約就停**：例如 4.2 之前每段在找不到檔案樹列時中止，後面的流程要等前端做出來
  才第一次真正執行；那時若某段失敗，先分辨是產品還是腳本的問題（契約的定位規則已由 `self/鷹架` 的合成 DOM 驗過）。
- **「WSL 檔案」只驗前端**：`ui_preview` 沒有 WSL runtime，服務端 WSL `vscode_uri` 的產生由 Rust 測試守。

## 請求分類方式（fix round 2，控制端裁決 R23）

「檔案分頁期間不請求輸出」與「Live Output 分頁時不查詢」要判斷每個請求是在哪個分頁為目前分頁時發出的。fix round 1 用
MutationObserver 回呼時間當切換邊界，但回呼在屬性變更之後的微任務才執行：同一個同步區塊裡「設定 `aria-selected` 後立即
fetch」的請求會落在回呼之前而被算錯邊（切回時誤報、切走時漏報）。現行做法不再用任何邊界時間分類：

- `pageHelpers()` 在任何前端腳本之前（`Page.addScriptToEvaluateOnNewDocument`）包住 `window.fetch` 與
  `XMLHttpRequest.prototype.open`／`send`。每個請求在**發出當下同步**記下當時 Live Output tab 的 `aria-selected` 與目前
  分頁身分（`at`：`live` 為 `"true"`／`"false"`，`tab` 為 `LIVE` 或檔案分頁的 `data-path`），settle 當下再記一次（`endAt`）。
  只由腳本注入，不改產品。
- 前提（契約 C3）：切換分頁時先更新 `aria-selected`，再開始／停止輪詢。
- `analyzeOutputAcrossSwitch()`（純函式）：`at.live === "false"` 的輸出請求＝檔案分頁期間發出（必須 0）；`at.live === "true"`
  且 `endAt.live === "false"`＝切走前發出、切走後完成（至多 1）；「切回後立即」＝capture 階段記下的點 Live Output tab 時間
  （在前端 handler 之前同步記錄）到第一個 `at.live === "true"` 請求的毫秒數（≤250）。
- 中繼資料：`at.live === "true"` 時發出的中繼資料請求必須 0。
- CDP 只做總數健全檢查（`cdpPageCountSanity`）：在應該沒有該類輪詢的時段，讀頁面端數 P1 → 等 400 ms → CDP 數 C（以
  session＋requestId 去重）→ 頁面端數 P2，要求 P1＝P2＝C；不等代表有請求走了未包到的途徑（`<img>`、`sendBeacon`、worker），
  判 FAIL。
- `self/鷹架` 的正負對照（在目前前端上實際跑，因 4.3 之前開不了檔案分頁，以直接改真的 Live Output tab 的 `aria-selected`
  模擬切換）：(a) 在點 Live Output tab 的 click handler 裡同步設回 `"true"` 後立即 fetch → 不誤報且判為立即；(b) 同步設成
  `"false"` 後立即 fetch → 被抓到；(c) 切走期間以 XHR 發輸出請求 → 被抓到；(d) 以 `<img>` 發一個輸出請求 → CDP 總數比
  頁面端多 1，健全檢查判 FAIL。另有純函式案例（含 Codex 重現：點擊 11000、請求 11000.05 不得誤報）。
