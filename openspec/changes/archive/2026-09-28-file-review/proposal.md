# file-review（change 5a）

## Why

使用者用 HERDR 跑 agent 時，最欠缺的是「就地看 agent 產出的檔案」：規格、計畫、研究報告（md）、PDF 報表、HTML 預覽頁，
現在都得切到別的程式開。Cockpit 已經知道每個 pane 在哪個資料夾（pane 的 `cwd`，設計文件 §2.5、
`docs/research/2026-09-27/file-review-probe.md` §1），差一個能安全讀檔、渲染並自動跟上改動的 Review 區。
使用者 2026-09-26 把原路線的「change 5 .md 瀏覽」擴成「檔案瀏覽與 Review」，2026-09-27 brainstorming 決定拆成
5a（本 change：檔案瀏覽＋Review＋自動更新）與 5b（git 唯讀層＋diff＋Git Graph）。

## What Changes

- **檔案樹**：左欄頂端改為「Project／檔案」兩個分頁。「檔案」分頁顯示目前選定 pane 所屬 repo 的檔案樹；
  樹根＝從該 pane 的 `cwd` 往上找到的 git repo 根目錄（`.git` 為資料夾或檔案），找不到就用 `cwd` 本身。
  依 `.gitignore` 過濾，`.git/` 一律隱藏，資料夾展開時才讀取那一層。
- **下半部分頁**：中欄下半部改為分頁區，第一個分頁固定是 Live Output，其後每個打開的檔案一個分頁（可關閉）。
  **Live Output 不再永遠可見**：只在它的分頁為目前分頁時顯示，且只在那時輪詢；選定 pane 時自動切回 Live Output 分頁。
- **四種檢視器**：md（伺服器端以 `comrak` 渲染、清掉原始 HTML 與危險連結，repo 內相對連結在 Review 區開啟）、
  pdf（`pdf.js`，連續捲動＋頁碼／翻頁／縮放／符合寬度，不做文字層）、html（sandbox iframe，不允許腳本）、
  其他文字檔（等寬純文字＋行號）；二進位或其他格式顯示「不支援預覽」。
- **自動更新**：前端每 2 秒詢問目前顯示中的檔案的大小與修改時間，有變才重讀，盡量保住捲動位置或頁碼。
- **分頁還原**：打開的檔案分頁、目前分頁、左欄選的分頁存在瀏覽器本機，重新整理後還原。
- **「在 VS Code 開啟」**：以 `vscode://file/...` 連結交給 VS Code；WSL 檔案的連結格式先實測，不通就對 WSL 檔案不顯示。
- **唯讀檔案端點**：新增根目錄查詢、列目錄、檔案中繼資料、Markdown 渲染、原始內容共五組 `GET` 端點。安全邊界：
  根目錄必須在「由目前所有 pane 的 `cwd` 推算出的允許清單」內；解析後的實體路徑不得跳出根目錄；套用既有本機同源檢查；
  只讀、有大小上限；HTML 回應帶 `Content-Security-Policy: sandbox`。WSL pane 的 POSIX `cwd` 轉成
  `\\wsl.localhost\<distro>\...` 讀取（`docs/research/2026-09-27/file-review-probe.md` §2）。
- **檔案 icon**：Material Icon Theme 5.38.1（MIT）VSIX 中全部 1251 個 SVG 與其對照表，保留原色；由伺服器依對照表
  為每筆目錄項目指定 icon。
- **視覺規則例外**：icon SVG 的顏色、PDF 頁面、HTML iframe 內容屬於「檔案內容」，不受十色 token 與四階字級規則約束；
  Cockpit 自己的 UI 仍全部遵守。
- **新 crate `cockpit-files`**：純邏輯（根目錄、路徑界限、列目錄、分類、icon 對照、Markdown 渲染），`cockpit` 依賴它。

## 非目標

- **不做 git 相關功能**：diff、Git Graph、commit 歷史都屬 5b。Windows 端 git 對 WSL repo 會被 dubious ownership 擋下
  （`docs/research/2026-09-27/file-review-probe.md` §3），git 讀取層要另外設計。
- **不寫入任何檔案、不執行任何程式**：沒有編輯、存檔、刪除、重新命名；「在 VS Code 開啟」只是連結，伺服器不啟動程序。
- **不動 HERDR 唯讀約束**：只用既有 snapshot 裡的 pane `cwd`，不新增任何 HERDR method（設計文件 §2、ADR-0001；
  pane `cwd` 改變沒有事件、只靠 snapshot 更新，見設計文件 §2.3）。
- **不做 VS Code extension host**，也不做 Ctrl+P 快速開檔、整個 repo 的內容搜尋、語法上色、PDF 文字層（選取與搜尋）。
- **不在 `cockpit.toml` 新增 Project 根目錄設定**：樹根一律由 pane 推算（設計文件 §8.2 的設定結構不變）。
- **不改投影 JSON 與 WebSocket 推送**（設計文件 §6.4、ADR-0004）：根目錄由獨立端點查詢，不放進 `ProjectedState`。
- 不做檔案系統監看（file watcher）：WSL 路徑的監看通知不可靠，改用定時詢問。

## Capabilities

### New Capabilities

- `file-review`：檔案根目錄與允許清單、唯讀檔案端點與其安全邊界、檔案樹、下半部檔案分頁、四種檢視器、自動更新、
  分頁還原、「在 VS Code 開啟」、檔案 icon。

### Modified Capabilities

- `cockpit-dashboard`：「路由與內嵌資源」加入檔案端點與 vendored 資源（pdf.js、icon）；「畫面整頁重畫」把左欄分頁列、檔案樹與
  下半部分頁區排除在重畫範圍外；「版面與窄視窗」改為左欄
  Project／檔案分頁、中欄下半部為分頁區；「Direction 01 視覺語彙」加入檔案內容的色彩例外。
- `live-output`：「選定一個 pane」的面板由常駐改為下半部第一個分頁，選定 pane 時切回該分頁；「輪詢與顯示」改為只在
  Live Output 分頁顯示期間輪詢，切回時立即請求一次。

## Impact

- 新 crate：`cockpit-files`（加入 workspace；新增 ADR-0006 記錄依賴方向，ADR-0003 不改）。
- `cockpit`：`src/http.rs`（新路由、vendored 資源服務）、新增檔案端點模組、pane `cwd` → 主機路徑轉換、`examples/ui_preview.rs`
  （假 repo fixture 與帶 `cwd` 的假 pane）。
- 前端：`cockpit/assets/index.html`、`app/render.js`（左欄分頁）、`app/output.js`（分頁化）、新增檔案樹與檢視器模組、
  `app/style.css`；新增 `cockpit/assets/vendor/`（pdf.js、Material Icon Theme 與各自的 LICENSE）。
- 新依賴：`comrak`、`ignore`、`include_dir`（Rust）；pdfjs-dist 6.3.289、Material Icon Theme 5.38.1（vendored 檔案）。
  執行檔約增加 8 MB。
- 驗收腳本：新增 `files-check.js`；`visual-check.js` 加段落；既有六支腳本（含 `live-output-check.js`）中「Live Output 常駐可見」
  相關斷言要隨分頁化改寫。
- `source_check`：403／405 回應本體加 `code`、不再帶出請求標頭原文（影響既有輸出與寫入端點的錯誤本體，只增欄位）。
- 文件：`CONTEXT.md`（檔案分頁、檔案根目錄詞條）、`cockpit/README.md`、`docs/handover.md`、`docs/research/2026-09-27/file-review-probe.md`（已建立）。
