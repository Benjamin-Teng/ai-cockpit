# design：file-review

## Context

動機見 `proposal.md`；行為契約見本 change 的 `specs/`。現況中與本設計有關的限制：

- 前端全部以 `include_str!`／`include_bytes!` 內嵌，`/app/{file}`、`/icons/{file}` 是白名單 `match`（`cockpit/src/http.rs`），
  單一執行檔可搬移（設計文件 §8.1、change 1b design D13）。目前沒有任何第三方前端程式碼。
- 版面是 `.shell` grid；`#app` 為 `display: contents`，`#output` 是 `#app` 的兄弟節點，所以 Live Output 不被整頁重畫碰到
  （archive `2026-09-26-direction-01-visual/design.md` D2、D3）。
- `source_check` 目前以 `route_layer` 掛在寫入端點與輸出端點上（`cockpit/src/source_check.rs`）。
- pane 的 `cwd` 已在投影中（`cockpit-core/src/projection.rs` 的 `ProjectedPane.cwd`），格式依 runtime 而異
  （`docs/research/2026-09-27/file-review-probe.md` §1）；`cwd` 改變沒有事件（設計文件 §2.3）。
- `cockpit.toml` 的 WSL runtime 以 `wsl = { distro, socket }` 設定（設計文件 §8.2），distro 名稱可直接取用。

## Goals / Non-Goals

**Goals：**

- 讀檔的安全邊界集中在一個可單元測試的地方（`cockpit-files`），HTTP 層只做參數轉換與錯誤對應。
- 前端新功能完全在整頁重畫範圍之外，沿用 Live Output 已驗證過的 DOM 分離做法。
- 5b（git 唯讀層、diff、Git Graph）之後只需新增一種分頁內容與新的端點，不必再改版面。

**Non-Goals（設計層面）：**

- 不為檔案端點做快取或背景索引；每個請求即時讀檔系統。
- 不支援 HTTP Range；pdf.js 以整檔讀取。
- 不做淺色 icon 變體（對照表的 `light` 區段不用）與 `rootFolderNames`、`languageIds` 對照。

## Decisions

### D1 新 crate `cockpit-files`，只放純邏輯

`cockpit-files` 對外提供：根目錄推算（給定主機路徑，往上找 `.git`）、相對路徑解析與界限檢查、列一層目錄、中繼資料與
`viewer` 分類、icon 對照（`IconTheme::from_json(&[u8])`）、Markdown 渲染。它不知道 HTTP、HERDR、投影與 runtime 設定；
依賴只有 `comrak`、`ignore`、`serde`／`serde_json`。`cockpit` 依賴它，負責：pane → `cwd` → 主機路徑（WSL 轉換）、
允許清單、路由與錯誤對應、內嵌 vendored 資源。

- **為什麼不放進 `cockpit`**：`cockpit` 已是最大的 crate；把安全邊界抽成獨立 crate，測試不需要起 HTTP 服務或 HERDR，
  而且 5b 的 git 讀取可以用同樣的方式另開 crate。
- **依賴方向**：`cockpit → cockpit-files`；`cockpit-core`、`cockpit-herdr`、`herdr-client` 都不依賴它。新增
  `docs/adr/0006-cockpit-files-crate.md` 記錄這條邊界（ADR-0003 保持原樣）。

### D2 根目錄的身分與允許清單

- `root_id` = 根目錄主機路徑（推算結果、未 canonicalize 的形式）的 UTF-8 位元組以小寫十六進位編碼。同一個根目錄
  永遠得到同一個值；前端把它當不透明字串。它不是秘密，也不是授權憑證——授權來自允許清單。選十六進位而非 base64url：
  只用標準函式庫即可編解碼，不必新增依賴；網址較長無妨。
- 驗證 `(runtime, root_id)` 時，在處理請求的當下取最新投影，逐一對該 runtime 的 pane 推算根目錄，**找到相符即停**；
  全部不符就回 `root_unavailable`。不做快取：根目錄推算是每層一次 `metadata` 呼叫，pane 數量在十幾個以內，成本可忽略
  （WSL UNC 路徑較慢，見風險）。
- WSL 轉換寫成純函式 `wsl_host_path(distro, posix) -> PathBuf`（在 `cockpit`），只接受以 `/` 開頭的路徑；非絕對路徑視為
  沒有根目錄。
- **替代方案**：伺服器發不透明隨機 token 對應根目錄（重新整理頁面或服務重啟後就失效，破壞分頁還原）；以 pane 當身分
  （pane 關掉、agent `cd` 到別處分頁就壞）。都不採用。

### D3 相對路徑解析與界限

1. URL 路徑片段逐段解碼；spec「檔案端點的共同規則」列出的片段（空、`.`、`..`、含 `/`、`\`、`:`、NUL、以 `.` 或空白結尾、
   Windows 保留裝置名）一律 `bad_request`，在碰檔案系統之前擋掉。理由：`\` 與 `:` 會形成磁碟代號相對路徑與 NTFS 替代資料流
   （`a.txt:stream`）；`%2F` 解碼後的 `/` 會繞過逐段檢查；保留裝置名與結尾點／空白在 Win32 路徑正規化時會被改寫，開到裝置可能
   卡住。8.3 短檔名（`PROGRA~1`）不另外擋：它仍會經 canonicalize 解析成長檔名再做界限判斷。
2. `root.join(segments...)` 後，根目錄與目標**各自** `std::fs::canonicalize`，以 `Path::starts_with`（逐 component 比對）
   判斷目標是否在根目錄內。兩邊都經 canonicalize，因此 `\\?\`、`\\?\UNC\` 前綴與大小寫一致。
3. 開檔用 canonicalize 後的路徑。檢查與開檔之間被換成符號連結的競態（TOCTOU）不處理：這是單人本機工具，攻擊者要能
   改寫你 repo 內的檔案才做得到，此時他已經有同等權限。

### D4 列目錄：逐層驗證＋`ignore::gitignore::GitignoreBuilder`＋`read_dir`

（task 2.3 實作後改寫，控制端裁決 R12。原設計以 `ignore::WalkBuilder` 沿邏輯路徑從根目錄走到目標，Codex review 找出四個同源
問題：先出根再回根的連結鏈會讓根外 `.gitignore` 生效、walk 錯誤被吞成成功的空清單、預設 `.ignore` 被套用、大小寫與 8.3
別名比對不到祖先鏈而回空清單。）

1. 對相對路徑的每個前綴（根、`根/a`、`根/a/b`……到目標）各自 canonicalize；任一層的實體路徑不在根目錄的實體路徑內 →
   `path_outside_root`；懸空連結依 D3 一致拒絕。
2. 為每一層建一個 `GitignoreBuilder`：比對用的根是該層的**邏輯路徑**，讀入的檔案是該層**實體目錄**下的 `.gitignore`；根目錄是
   git repo 且 `.git` 是資料夾時，根那一層另加 `.git/info/exclude`。不讀 `.ignore`、使用者全域設定與根目錄以上的任何檔案
   （spec「根目錄以上的 .gitignore 不生效」）。Windows 上不分大小寫比對（同 Git for Windows 預設 `core.ignorecase=true`）。
   每個子項目由深到淺取第一個有結果的 matcher。
3. `fs::read_dir(目標的實體路徑)` 列舉；讀取錯誤一律 `io_error`，不吞掉。略過名為 `.git` 的項目。項目種類以 `fs::metadata`
   （跟隨連結）判斷；指向根目錄外的連結仍會列出，但開啟時由 D3 擋下（回 403）。
4. 請求路徑含 8.3 短名稱時，`.gitignore` 以字面比對可能不生效；檔案樹一律使用列表回傳的實際名稱，不受影響。

不用 git 指令，因為 Windows 端 git 對 WSL repo 會被 dubious ownership 擋下（`file-review-probe.md` §3）。

### D5 Markdown 由 comrak 在伺服器渲染

選項：`extension.table`、`tasklist`、`strikethrough`、`autolink`、`header_ids = Some("md-")`（前綴避免與 `#app`、`#output`
等頁面 id 撞名）；`render.r#unsafe = false`（預設）。comrak 在 `unsafe` 為 false 時會以註解取代原始 HTML，並清掉危險 URL；是否涵蓋 spec 列出的所有 scheme，由
`cockpit-files` 的單元測試逐一斷言（不假設）。前端先把回應放進 `<template>`（其內容是惰性的，圖片不會載入），改寫
`a[href]` 與 `img[src]`（D8）之後才移入內容容器。渲染端點回應帶 `Content-Security-Policy: sandbox`，直接開啟該網址時也不會在
Cockpit 的來源下執行任何東西。

- **替代方案**：前端 `marked` 加 `DOMPurify`（使用者於 brainstorming 否決：多引入兩個第三方 JS，清洗責任外移）。

### D6 前端 DOM：新區塊都在 `#app` 之外

- `index.html` 新增兩個 `#app` 的兄弟節點：`#files`（左欄分頁列與檔案樹，Project 清單仍由 `render.js` 畫在 `#app` 內，
  以 grid area 疊在同一欄）與 `#review`（下半部分頁列＋各分頁內容，`#output` 移入其中成為第一個分頁的內容）。左欄目前分頁
  是 `files.js` 持有的狀態；`render.js` 每次整頁重畫時讀取它決定 Project 清單是否 `hidden`（`#app` 內的節點每次都會被
  `replaceChildren` 換掉，由 `files.js` 從外面改 `hidden` 會在下一次重畫被洗掉）。切換左欄分頁時 `files.js` 更新狀態並呼叫
  既有的重畫入口。
- 新模組：`files.js`（左欄分頁、檔案樹、分頁區、分頁還原、中繼資料輪詢）與 `viewers.js`（四種檢視器）。`output.js` 只多
  兩個入口：「分頁變為可見／不可見」，讓它暫停與恢復輪詢。
- 模組之間用既有的全域掛勾慣例（`window.onState` 同類）溝通：`output.js` 在選定 pane 時通知 `files.js`（切到 Live Output
  分頁、檔案樹改根目錄）。
- **狀態只存在模組變數**：展開狀態（依 `runtime + root_id`）、各分頁的捲動位置、頁碼，都存在模組變數，DOM 只是呈現
  （memory `full-repaint-discards-state-held-only-in-dom`）。
- **切換分頁用 `hidden` 屬性**。被隱藏的捲動容器的 `scrollTop` 不保證保留，因此切走前記下捲動位置與「是否貼底」，切回後
  寫回；Live Output 沿用 `output.js` 的 `keepPinnedAcross()`（memory `stick-to-bottom-lost-when-container-resizes`）。

### D7 PDF：pdf.js 函式庫模式、延遲載入

- 第一次開 PDF 時才 `import('/vendor/pdfjs/pdf.min.mjs')`，設定 `GlobalWorkerOptions.workerSrc`、`cMapUrl`
  （`/vendor/pdfjs/cmaps/`，`cMapPacked: true`）、`standardFontDataUrl`（`/vendor/pdfjs/standard_fonts/`）、
  `disableRange: true`、`disableStream: true`。
- 每頁先放一個依頁面比例定尺寸的佔位框，以 `IntersectionObserver` 在接近可視範圍時才畫到 `canvas`（考慮
  `devicePixelRatio`），避免上百頁的 PDF 一次畫完。縮放：符合寬度（預設）與 50%～300%、每次 25%。
- 不做文字層：文字層需要 pdf.js 的 CSS，會與 TK1 的色彩／字級檢查衝突，且 spec 不要求選取與搜尋。
- 自動更新時重新載入文件、捲回原本的頁碼。

### D8 前端的連結與圖片改寫

Markdown 內容在 `<template>` 中（放入頁面之前）逐一處理：`href` 以 `#` 開頭 → 同頁捲動（錨點名稱加上 `md-` 前綴後以
`CSS.escape` 找 `id`；跨檔連結的錨點同樣加前綴）；`http:`／`https:` → 設
`target="_blank"`、`rel="noopener noreferrer"`；其他不含 scheme 的相對路徑 → 以目前檔案所在資料夾為基準解析（`URL`
物件解析後再去掉前綴，解析結果跳出根目錄就不動作），攔截點擊改為開啟分頁；其餘 scheme → 攔截點擊不動作。`img[src]` 的
相對路徑改為對應的原始內容端點網址；絕對網址、`//` 開頭、`\\` 開頭或解析後跳出根目錄的圖片，把 `img` 換成顯示其 `alt` 的
文字節點。HTML 檢視器不做改寫：iframe 的 `src` 本身就是原始內容端點網址，相對引用自然落在同一個
端點下，並同樣經過 D3 檢查。

### D9 vendored 資源與嵌入

- `cockpit/assets/vendor/pdfjs/`：pdfjs-dist 6.3.289 的 `pdf.min.mjs`、`pdf.worker.min.mjs`、`cmaps/`、`standard_fonts/`、
  `LICENSE`，以及該版本執行時會讀取的其他資源目錄（例如 `wasm/`、`iccs/`；確切清單在 task 1.3 以該版本的 `src/display/api.js`
  選項與實際套件內容查證）。`.wasm` 以 `application/wasm` 提供，瀏覽器才會串流編譯。
- `cockpit/assets/vendor/material-icons/`：VSIX 5.38.1 的 `icons/*.svg`（1251 個）、`material-icons.json`、`LICENSE.txt`。
- `cockpit/assets/vendor/README.md` 記每個來源的版本、下載網址、取出方式、原始下載檔的 SHA-256，以及取出後關鍵檔案
  （`pdf.min.mjs`、`pdf.worker.min.mjs`、`material-icons.json`）的 SHA-256；之後升版照做。
- 以 `include_dir!` 內嵌整個 `vendor/`，`/vendor/{*path}` 從嵌入目錄查找；查無即 404。既有 `/app/`、`/icons/` 白名單不變。
- `material-icons.json` 由 `cockpit` 在啟動時交給 `IconTheme::from_json` 解析一次；對照表的值是 `iconDefinitions` 的鍵，
  再經 `iconPath` 取出 SVG 檔名。

### D10 錯誤本體與前端文案

沿用既有 `{"error": ...}` 形狀並加 `code`（spec「檔案端點的共同規則」），讓既有前端的錯誤處理習慣不變。`source_check`
的 403 與 405 目前沒有 `code`，且 403 本體會帶出請求的 `Host`／`Origin`：改成所有套用它的端點一律帶 `code`
（`forbidden_source`、`method_not_allowed`）、本體不含請求標頭原文。既有輸出端點與寫入端點的 spec 只規定 `error` 欄位，多一個
`code` 欄位不違反；其測試若逐字比對本體要跟著改。前端依 `code` 查表
顯示中文文案，不以 HTTP 狀態碼或 API 路徑開頭（change 4 延後殘項中的文案問題，新功能不再犯）。

### D11 視覺規則例外的落實

- icon 一律以 `<img src="/vendor/material-icons/…">` 顯示，不內嵌 SVG 原始碼到 DOM，因此 icon 的顏色不會出現在
  `style.css`，TK1（只掃 `style.css`）本來就不會掃到；spec 已明寫這是刻意的例外，不是漏洞。
- PDF `canvas` 與 HTML iframe 的內容不在 Cockpit 的 DOM 文字中，`visual-check.js` 的對比段落排除這兩種容器；Markdown
  內容區則納入對比檢查（spec「Markdown 檢視遵守色彩與對比」）。
- 新 UI 的樣式全部取用既有 10 個色彩 token 與 `--fs-*` 四階字級。外觀完成後過 frontend-design 設計審核
  （memory `frontend-appearance-reviewed-by-frontend-design-skill`）。

### D12 驗收用的假 repo

`ui_preview` 新增一個 fixture 目錄 `cockpit/examples/fixtures/review-repo/`：含 `README.md`（含相對連結、錨點、圖片、表格、任務清單）、`docs/`、`long.md`、`page.html`（含 `<script>` 與同目錄
`style.css`）、`report.pdf`（3 頁、第 1 頁含中文，檔案由腳本產生並進版控）、`note.txt`、`bin.dat`（含 NUL）。
`.git/` 與 `.gitignore` **不放進 fixture**（git 拒絕追蹤名為 `.git` 的路徑；巢狀 `.gitignore` 會影響本 repo 自己的
忽略規則），由 `ui_preview` 啟動時在暫存副本中建立。`ui_preview` 啟動時把 fixture 複製到暫存目錄，假 pane 的 `cwd` 指向
暫存目錄內的子資料夾，使驗收腳本可以改寫檔案測試自動更新，
又不會弄髒 repo。

## Risks / Trade-offs

- [WSL UNC 路徑存取慢，且會喚醒 WSL VM] → 只在請求當下、找到相符即停；自動更新只查目前分頁的一個檔。實測若延遲過高，
  再考慮以 `cwd` 為鍵的短時快取（不改 spec）。
- [允許清單以「目前有 pane 的 repo」為界，範圍比「某一個 pane」大] → 這是刻意的：同一台機器上，使用者已經開 agent 在跑的
  repo 本來就是他要看的；清單外的一律拒絕。
- [cwd 不在 git repo 內時根目錄就是 cwd 本身；某個 shell pane 停在家目錄或 `C:\` 時，整個家目錄或整顆磁碟都會進入允許清單]
  → 仍只接受本機同源請求、只讀；這等同使用者在該 pane 裡本來就能讀的範圍。記在 `cockpit/README.md` 的安全說明，不另設限制。
- [WSL 經 `\\wsl.localhost`（9P）讀檔時，Linux 符號連結可能由 WSL 端直接解開，Windows 的 canonicalize 看不到它指向根目錄外]
  → task 5.2 實測「WSL repo 內指向 `/etc/passwd` 的符號連結」必須回 403；若實測洩漏，停下來回到設計（不在實作中自行補洞）。
- [執行檔增加約 8 MB、編譯時間變長] → 使用者已接受；`include_dir` 只在 `cockpit` crate 生效，`cockpit-files` 的測試不受影響。
- [pdf.js 是大型第三方程式碼] → 固定版本、記 SHA-256；只載入函式庫、不載入 viewer；PDF 只來自允許清單內的 repo。
- [comrak 的清洗行為與我們假設不同] → D5 的單元測試逐一斷言 spec 列出的危險輸入，升版時同一組測試把關。
- [Chrome 的 `vscode://vscode-remote/...` 連結行為沒有官方文件] → task 1.2 已實測（`file-review-probe.md` §5）：可用，但結尾
  必須加 `:1`，否則 VS Code 當成資料夾開啟；spec 已改。Chrome 端的實際點擊留待 task 5.1 使用者目視驗收確認。
- [非 git 目錄是否套用 `.gitignore`] → D4 改用 `GitignoreBuilder` 後不再依賴 `WalkBuilder` 的 `require_git` 預設值；仍以「非 git 目錄也套用 `.gitignore`」的
  單元測試鎖住。
- [既有驗收腳本假設 Live Output 永遠可見] → 分頁化的 task 逐條核對 `live-output-check.js`、`visual-check.js` 相關斷言；
  只改 spec 已改變的部分。

## Migration Plan

無資料遷移。新增的 `localStorage` 鍵只由新前端讀寫；舊前端不受影響。回退＝回到前一版執行檔。

## Open Questions

- `ui_preview` 用的中文 PDF 以什麼工具產生（repo 沒有 Python 環境的 PDF 函式庫；可用 Chrome `Page.printToPDF` 從一份
  HTML 產生），在 fixture task 內決定，不影響 spec 與 task 拆分。
