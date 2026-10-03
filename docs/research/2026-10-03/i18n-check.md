# i18n-check：介面語言驗收

驗收 OpenSpec change `ui-language` 的前端行為。腳本：`docs/research/2026-10-03/i18n-check.js`。寫法比照
`docs/research/2026-10-02/notify-check.js`：raw CDP over WebSocket、headless Chrome（自己的 user-data-dir）、
自己 spawn `ui_preview`、依 PID 收尾。

權威是 spec：`openspec/changes/ui-language/specs/ui-language/spec.md`；段落設計見同 change 的 `design.md` D7。

## 用法

在 repo 根執行，先建置（前端資源內嵌在執行檔，改過 `cockpit/assets/` 一定要重建）：

```bash
cargo build -p cockpit --example ui_preview
node docs/research/2026-10-03/i18n-check.js            # 預設跑全部的段（1 至 5；5 會重拍 `i18n-en-*.png`）
node docs/research/2026-10-03/i18n-check.js --only=1   # 只跑某幾段（逗號分隔）
```

- 只需要 Node 22 與 Chrome（預設 `C:\Program Files\Google\Chrome\Application\chrome.exe`，可用環境變數
  `COCKPIT_CHROME` 指定）。第 1 段不需要瀏覽器與 `ui_preview`。
- 埠從 7780 起往上找空埠（避開 7770），Chrome 的 CDP 埠從 18830 起。**驗收腳本不可並行跑。**
- 每條斷言印 `PASS`／`FAIL`。「前置：」開頭的是環境與既有行為的前提，不是本 change 的新行為；最後一行
  `RESULT: PASS` 或 `RESULT: FAIL (N)`，有失敗時結束碼為 2。
- 收尾只依 PID 終止自己 spawn 的 `ui_preview.exe` 與 `chrome.exe` 並刪暫存 user-data-dir。

## 分段

| 段 | 內容 | 狀態 |
|---|---|---|
| 1 | 字典一致性：以 vm 沙箱載入 `i18n.js`，斷言繁中與英文字典鍵集合相同、每個鍵的具名佔位符集合相同、沒有空值、鍵名符合命名慣例；檢查 `t()` 的代入與缺鍵行為（回傳鍵本身、只 `console.warn` 一次）、`<html lang>`，以及 `index.html` 的每個 `data-i18n` 鍵都在字典裡、節點內的後備文字與繁中字典值逐字相同。task 2.1 起另驗 `tn()` 複數、`.one`／`.other` 成對，並檢查 `render.js`／`actions.js`／`output.js`／`files.js`／`viewers.js`／`git.js` 用到的字典鍵都存在、字典鍵都有人用、程式碼（註解與 `console.warn` 第一個引數的開發者訊息除外；CJK 標點「」、。（）：也算）沒有硬編碼的繁中字串，函式內沒有區域變數或參數遮蔽模組層級的 `t`／`tn`，`t()` 沒有用到複數基底鍵（該用 `tn()`），並以變異樣本驗掃描器本身。階段審查後另驗「第一次繪製前決定語言」：沙箱以 `readyState=loading` 驗 `<html lang>` 立即設好、英文先加 `i18n-pending`、`DOMContentLoaded` 與 1.5 秒計時器都會移除、繁中不加；`index.html` 的 `i18n.js` 在 `<head>` 內且是第一個 `<script src>`，`style.css` 有對應的隱藏規則。task 3.3 起另驗後端訊息代碼：字典的 `msg.*` 鍵與 3.1／3.2 的代碼清單一致（35 個），每個代碼的繁中範本等於後端原文、英文範本等於預期譯文，`tMsg()` 的行為（參數原樣代入、`raw`、未知代碼／缺欄位／參數不齊退回原文、不 `console.warn`、原型鏈名稱不當代碼） | task 1.1 已實作，task 2.1、2.2、2.3、3.3 擴充 |
| 2 | 英文介面沒有繁中字典字串（比對規則寫在腳本檔頭）。以 `cockpit.lang=en` 走 `render.js`／`actions.js`／`output.js` 的畫面：頂列與燈號、左欄 Project 清單、Factory Floor（改綁模式 banner、空狀態、警告數、歧義綁定、未宣告 task）、runtime 卡片、最近事件與底列通道（含通道斷線）、錯誤 banner（409、請求沒有完成）、Live Output（空、截斷、請求失敗、非 JSON 的 502、pane 已不存在、前端逾時；失敗狀態以換掉頁面的 `fetch` 提供）。每個畫面收集所有文字節點與 `title`／`aria-label`／`placeholder`，比對繁中字典值，另檢查 `#app`／`#output`／檔案樹／檔案分頁沒有殘留的 CJK 文字，各處英文字串逐字斷言；再以繁中跑同一流程，逐字比對改動前的繁中字串。所有模組都已進字典，不再有「待處理模組」的排除清單（原 `PENDING_MODULE_SEL` 已在 task 2.4 刪除）；殘留 CJK 檢查範圍含通知設定面板 `#notify-panel`。task 2.2 加入檔案畫面（另起一個推送間隔拉長的 `ui_preview`，英文與繁中各走一遍）：左欄檔案樹（空狀態、正在讀取根目錄、資料夾讀取中／空資料夾／「還有 N 項未顯示」單複數／讀取失敗、重新整理、根目錄查詢的 16 個錯誤碼）、檔案分頁（title、關閉鈕 aria-label、尚未讀取、正在讀取、讀取於、在 VS Code 開啟、過期與 7 個檔案分頁錯誤碼）、各檢視器（Markdown 錨點／被擋的連結與圖片、HTML iframe title、PDF 工具列與頁碼、PDF 無法解析、純文字檔案太大（大小已知與未知）、尚未實作的檢視器、二進位與圖片的不支援預覽）；fixture 沒有的邊界狀態靠頁面內的假 `fetch` 提供。task 2.3 加入 git 畫面（共用同一個推送間隔拉長的 `ui_preview`，英文與繁中各走一遍；`--git-only` 是只跑這一段的開發用旗標，要搭配 `--only=2`）：左欄「變更」面板（四個分組標題與筆數、衝突組、分離 HEAD、清單被截斷、沒有變更、正在讀取變更與根目錄、不是 git repo、過期、根目錄查詢 11 個與 git 狀態 15 個錯誤碼）、diff 分頁（工作區／已暫存／空的版本標示、分頁 title 與關閉鈕、二進位／只有權限改變／子模組／兩側相同／「省略 N 行」單複數／讀取中、過期與錯誤）、Git Graph 分頁（工具列、分支篩選三組、搜尋筆數單複數、詳情標籤與按鈕、複製回饋、比較基準與比較詳情、「變更檔案」單複數、詳情讀取中／錯誤／截斷／父 commit 不在已載入範圍、分支已變更提示、清單讀取中／錯誤／空／已達上限）、某版本檔案分頁（暫存區標示、分頁 title、開啟目前版本、過期、讀取中、尚未實作的檢視器、錯誤）；衝突組、merge commit、三種 ref、二百筆以上 commit 用 fixture 的真資料，其餘狀態（含 5000 筆的假 log）由假 `fetch` 提供。task 2.4 加入通知畫面（共用同一個 `ui_preview`，英文與繁中各走一遍；`--notify-only` 是只跑這一段的開發用旗標，要搭配 `--only=2`）：鈴鐺 `aria-label`／`title`、設定面板（標題、前景說明、四個事件名稱與說明）、權限四種狀態（尚未決定、已允許、已封鎖、不支援，另按下「允許通知」後轉為已允許）、桌面通知的標題與內文（單一的 agent blocked、帶兩個綁定名稱的 agent done、task failed、task completed，以及超過 3 件合併後的標題與內文）；權限與桌面通知由頁面內換掉 `window.Notification` 的記錄器提供，狀態轉換直接呼叫 `cockpitNotify.observe`。task 3.3 起後端訊息依代碼翻譯，不再從掃描中排除連線原因、protocol 警告與 project 警告：英文介面 WSL 卡片的原因是英文且含 `Ubuntu-24.04`；頁面內假 `fetch` 回帶 `code` 的 409 驗錯誤 banner（`already_last_stage`、含中文的參數原樣代入、`persist_failed`、未知代碼與沒有 `code` 的本體顯示原文）；Live Output 的 503／504 以代碼驗（`output_read_failed` 的 `detail` 原樣代入、`read_timeout`、未知代碼）；同步餵改過的投影驗 `reason_msg`／`protocol_warning_msg`／`warning_msgs` 的未知代碼、欄位不存在、陣列比 `warnings` 短時退回原文且不顯示代碼本身；繁中介面同流程斷言顯示後端原文 | task 2.1 已實作（render／actions／output），task 2.2 擴充（files／viewers），task 2.3 擴充（git），task 2.4 擴充（notify），task 3.3 擴充（後端訊息代碼） |
| 3 | 語言決定：spec 四個 scenario 端到端（CDP `Emulation.setTimezoneOverride`＋`Emulation.setUserAgentOverride` 的 `acceptLanguage`）；頁面內直接呼叫純函式 `cockpitI18n.resolveLang()` 驗邊界；`localStorage` 丟例外；`setLang()` 寫入並重新載入；第一次繪製前決定語言（`Page.addScriptToEvaluateOnNewDocument` 在文件開頭掛 `DOMContentLoaded` 探針，早於 `i18n.js` 自己的監聽：英文此刻 `<html lang>` 已是 `en`、`i18n-pending` 還在、靜態節點 computed `visibility: hidden` 且仍是後備文字，載入完成後移除並顯示英文；繁中不藏；`i18n.js` 的 `<script>` 父節點是 `HEAD`） | task 1.1 已實作，階段審查 1 M1 擴充 |
| 4 | 語言切換按鈕與兩個視窗的同步：按鈕位置（鈴鐺旁、與鈴鐺等高）、文字／`lang`／可及名稱；滑鼠點擊與鍵盤 Enter 切換（`cockpit.lang`、重新載入、`<html lang>`）；鍵盤焦點外框；同一個 Chrome 兩個分頁（`/json/new`，同源）其中一個切換、另一個跟著重載，無關的 storage 鍵不重載；`Storage.prototype.setItem` 丟例外（`Page.addScriptToEvaluateOnNewDocument`）時按鈕停用、有 title、點擊與 Enter 都不動作。段 1 另以沙箱驗 `canPersist`、`setLang` 寫入失敗不重載、`storage` 事件的重載判斷 | task 1.2 已實作 |
| 5 | 英文介面截圖（`cockpit.lang=en`）：1536×1024、1100×900、700×900 三種寬度各拍五個畫面（選了 pane 且 Live Output 顯示中的預設儀表板、通知設定面板、左欄 Changes、Git Graph 分頁、Markdown 檔案分頁），存成同目錄 `i18n-en-<畫面>-<寬度>.png`，交 frontend-design 審核。窄寬度時 Live Output、Git Graph、檔案分頁在第一屏之外，這三個畫面把 `#review` 捲到視窗頂端再拍。去識別化：拍前裝 `MutationObserver` 把文字節點與 `title`／`aria-label`／`placeholder` 裡 `Users\` 之後的路徑段與真實使用者名稱、主機名稱換成 `<user>`（名稱執行時由 `os` 取得，不寫進 repo），並斷言頁面文字與屬性不含真名；斷言只擋文字，**PNG 仍要逐張看圖確認**（見 `docs/research/2026-10-02/deid-check.md`）。畫面上的「Read at」是拍攝當下的時間，每次重拍會不同 | task 5.1 已實作 |

### 為什麼 ③ 要同時有端到端與純函式

Chrome 的 `--lang` 會把 `zh-SG`、`zh-HK` 等值改寫成別的語言碼，端到端沒辦法穩定產生這些輸入；所以邊界案例
（`zh-SG`、`zh-Hant-SG`、`zh-hant`、`zh-CN`、第二順位語言、非法的 `cockpit.lang` 等）直接在頁面內呼叫 `resolveLang()`，
端到端則用 CDP 真的覆寫時區與 `navigator.languages`，驗四個 scenario 與「localStorage 丟例外」「`setLang`」。

端到端以「左欄『變更』分頁按鈕」與左欄 tablist 的 `aria-label` 判斷靜態節點是否已套用該語言，因為 `render.js` 與
`files.js` 不會改寫它們。
