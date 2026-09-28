# tasks：file-review

> 執行路徑：SDD｜理由：約 20 個 task，一個 session 做不完；跨新 crate `cockpit-files`、`cockpit` 後端、前端三塊，各有可獨立驗收的工作；
> 含安全邊界（路徑界限、允許清單、HTML／SVG 隔離）與第三方程式碼引入，屬高風險；前端分頁化會改寫既有驗收腳本的前提，後面的 task
> 疊在前面之上，需要逐 task 的 Codex review。

通則（每個 task 都適用）：

- 結尾跑 `cargo fmt --check`、`cargo clippy -p <本 task 改到的 crate> --all-targets -- -D warnings`、`cargo test -p <同上>`；
  有改 `cockpit/examples/ui_preview.rs` 時加跑 `cargo test -p cockpit --example ui_preview`；有改 `.md` 時在 repo 根跑
  `markdownlint-cli2 "**/*.md"`（核對 `Linting: N files` 不為 0）；有改 `openspec/` 時跑 `openspec validate --all`。輸出貼進回報。
- Rust 的 task 以 `superpowers:test-driven-development` 進行，回報附 red→green 證據。
- 前端資源與 `vendor/` 以內嵌方式編進執行檔：改完 `cockpit/assets/` 一定先 `cargo build -p cockpit --example ui_preview`
  再跑任何腳本，否則腳本跑的是舊前端。
- **「既有六支腳本」**＝`docs/research/2026-09-15/reconnect-check.js`、`whatever-check.js`、
  `docs/research/2026-09-16/actions-check.js`、`channel-backoff-check.js`、`factory-floor-check.js`、
  `docs/research/2026-09-19/live-output-check.js`；加上 `docs/research/2026-09-23/visual-check.js`。**每個改前端的 task 結束時全部必須全綠**：
  只改被本 task 打壞、且 spec 已改變的斷言；腳本修改與產品修改分開 commit，commit 訊息逐條列出被改的斷言與對應的 spec scenario；
  不得放寬 spec 沒有改變的斷言。
- `factory-floor-check.js` 會覆寫 `docs/research/2026-09-16/task-5.2-scenario-d.png`，跑完 `git checkout --` 還原。
- 跑腳本前確認 7770 沒有人在用；**不是自己開的程序只能回報、不准砍**；跑完依 PID 收尾，確認 7770 與 CDP port 沒有 LISTEN。
- 對 HERDR 只送唯讀請求；Windows 端不得 `herdr server stop`（`AGENTS.md`）。
- 程式碼註解若要引用 task，寫成 `file-review task N.M`。`tasks.md` 由控制端統一勾。
- 每個改外觀的 task（4.1–4.6）結束後，由控制端做設計審核（1536／1100／700 三寬 viewport 截圖＋CSS diff，對照 design.md 與
  `docs/direction-01-visual-design.md`），並依 memory 過 frontend-design 審核；有 finding 才回頭修正。

## 1. 基線、實測與 vendored 資源

- [x] 1.1 記錄基線：在分支 `feat/file-review` 跑 `cargo test --workspace`、`cargo test -p cockpit --example ui_preview`、既有六支腳本與
  `visual-check.js`，把 passed／failed／ignored 數字與各腳本結果寫進 SDD ledger；驗收＝ledger 有這些數字，failed 為 0、腳本全綠
- [x] 1.2 實測「在 VS Code 開啟」的 WSL 連結：產生一個指向 WSL 內既有檔案的 `vscode://vscode-remote/wsl+<distro>/<path>` 連結與一個
  `vscode://file/<Windows 路徑>` 連結的測試頁，請使用者在 Chrome 各點一次並回報 VS Code 是否開到該檔；結果寫進
  `docs/research/2026-09-27/file-review-probe.md` 新的一節。WSL 連結不通時，以 `/opsx:update` 把 spec「在 VS Code 開啟」的 WSL
  scenario 改為「WSL 檔案不顯示此連結」；驗收＝probe 文件有結果、必要時 spec 已更新且 `openspec validate --all` 通過
- [x] 1.3 vendored 資源（design D9）：下載 pdfjs-dist 6.3.289 與 Material Icon Theme 5.38.1 VSIX，取出 design D9 列出的檔案放到
  `cockpit/assets/vendor/pdfjs/`、`cockpit/assets/vendor/material-icons/`（含各自的 LICENSE）；查證 pdf.js 6.3.289 執行時會讀取的其他資源目錄
  （如 `wasm/`、`iccs/`）並一併取出；新增 `cockpit/assets/vendor/README.md` 記錄版本、下載網址、取出方式、原始下載檔與關鍵檔案（design D9）的
  SHA-256；驗收＝`material-icons/` 下 SVG 數量為 1251、關鍵檔案以 `sha256sum` 重算與 README 相符、`markdownlint-cli2` 0 issues

## 2. `cockpit-files` crate

- [x] 2.1 新 crate 與根目錄推算：workspace 加入 `cockpit-files`（依賴 `comrak`、`ignore`、`serde`、`serde_json`，版本依 design）；
  新增 `docs/adr/0006-cockpit-files-crate.md`（design D1 的依賴方向）；實作「給定主機路徑，往上找 `.git`（資料夾或檔案）」的根目錄推算；
  驗收＝單元測試涵蓋 spec「往上找到 git repo 根目錄」「worktree 的 .git 是檔案」「不在 git repo 內」與路徑不存在，`cargo test -p cockpit-files` 全綠，
  `cargo tree -p cockpit-core` 不含 `cockpit-files`
- [x] 2.2 相對路徑解析與界限（design D3）：片段檢查（spec「檔案端點的共同規則」列出的全部片段規則 → `BadRequest`）、join 後雙邊 canonicalize、逐 component
  判斷在根目錄內；驗收＝單元測試涵蓋 spec「用 .. 跳出根目錄」（含 `%2F` 解碼後的 `/`、`C:` 片段、NUL、結尾點與空白、保留裝置名 `CON`／`con.txt`／`COM1`／`CONIN$`）「符號連結指向根目錄外」（Windows 無權限建立符號連結時改用 junction：
  測試以 `cmd /c mklink /J` 建立；兩者都做不到時標 `#[ignore]` 並註明原因）、大小寫不同的根目錄路徑、8.3 短檔名解析後仍受界限檢查，全綠
- [x] 2.3 列一層目錄（design D4）：逐層驗證實體路徑並以 `GitignoreBuilder` 套用各層 `.gitignore`（R12 改寫，原為 `WalkBuilder`）、略過 `.git`、資料夾在前且不分大小寫排序、5000 筆截斷（`omitted`）、非 UTF-8 名稱略過
  （`skipped`）；驗收＝單元測試涵蓋 spec「依 .gitignore 過濾」「大目錄截斷」、非 git 目錄也套用 `.gitignore`（鎖住 `require_git(false)`）、
  「根目錄以上的 .gitignore 不生效」、子資料夾自己的 `.gitignore`、點開頭項目照常列出，全綠
- [x] 2.4 中繼資料與 `viewer` 分類：`size`、`modified_ms`、依 spec 的副檔名與前 8192 位元組規則分類（含結尾被截斷的多位元組字元）；
  驗收＝單元測試涵蓋 spec「分類」「修改後中繼資料改變」與截斷的 UTF-8 邊界，全綠
- [x] 2.5 icon 對照（design D9）：`IconTheme::from_json(&[u8])`，檔案依完整檔名→最長多段副檔名→預設，資料夾依名稱（收合／展開）→預設，
  結果為 SVG 檔名；驗收＝以小型 fixture JSON 的單元測試涵蓋每條比對規則，並以 `cockpit/assets/vendor/material-icons/material-icons.json`
  實檔測試 spec「常見檔案」「資料夾展開」（實檔測試以相對路徑讀檔，檔案不存在時失敗而非略過），全綠
- [x] 2.6 Markdown 渲染（design D5）：comrak 選項、標題 `id`、2 MiB 上限、非 UTF-8 以替代字元處理；驗收＝單元測試涵蓋 spec「GFM 元素」
  「夾帶的 HTML 與危險連結被清掉」，並逐一斷言 `javascript:`、`vbscript:`、`data:text/html` 連結與 `data:image/png` 圖片的輸出行為，全綠

## 3. `cockpit` 後端與預覽

- [x] 3.1 根目錄與允許清單（design D2）：`wsl_host_path` 純函式、pane → 根目錄、以最新投影即時驗證 `(runtime, root_id)`、
  `GET /api/runtimes/<runtime>/panes/<pane>/root`；驗收＝`cockpit/tests/files_endpoint.rs` 以暫存目錄與假 runtime 狀態涵蓋 spec
  「WSL 路徑轉換」（純函式）「pane 關掉後根目錄不可用」「查到根目錄」「pane 沒有 cwd」與 `runtime_unknown`、`pane_unknown`，全綠
- [x] 3.2 檔案端點：列目錄、中繼資料、Markdown 渲染、原始內容四組路由（含根目錄本身的 `list`）、spec「檔案端點的共同規則」的錯誤碼
  對應與本體、`Cache-Control`／`nosniff`、原始內容的 content-type 與 `Content-Security-Policy: sandbox`、大小上限、只收 `GET`、
  套用 `source_check` 並把它的 403／405 本體改為帶 `code`、不含請求標頭原文（design D10；既有輸出與寫入端點的測試若逐字比對本體一併改）；
  中繼資料回應含 `icon` 與 `vscode_uri`（依 runtime 設定產生）；驗收＝`files_endpoint.rs` 涵蓋 spec「檔案端點的共同規則」全部 scenario（含「被隱藏的檔案仍可直接讀取」）、「根目錄不在允許清單」、「HTML 帶 sandbox」
  「太大」（以稀疏檔或測試用較小上限注入，不在 repo 放大檔）、「Windows 檔案」「WSL 檔案」的 `vscode_uri`，
  `cargo test -p cockpit` 全綠（含既有 `output_endpoint.rs`、`pipeline_api.rs`）
- [x] 3.3 `/vendor/` 路由（design D9）：`include_dir` 內嵌 `cockpit/assets/vendor/`、依副檔名給 content-type、`nosniff`、查無 404；
  驗收＝`cockpit/tests/http.rs` 涵蓋 cockpit-dashboard spec「vendored 資源」，既有路由測試全綠；另把建好的執行檔
  複製到 repo 外的暫存目錄啟動，`curl` 一個 `/vendor/pdfjs/` 與一個 `/vendor/material-icons/` 檔案皆 200（spec「單一執行檔」），輸出貼進回報
- [x] 3.4 `ui_preview` 假 repo（design D12）：新增 `cockpit/examples/fixtures/review-repo/`（中文 PDF 的產生方式在此決定並把產生腳本
  與結果一併進版控），啟動時複製到暫存目錄並建立 `.git/` 與 `.gitignore`，假投影中至少兩個 pane 的 `cwd` 指向副本內（一個在子資料夾、
  一個在另一個根目錄），結束時清掉暫存目錄；驗收＝`cargo test -p cockpit --example ui_preview` 全綠；啟動 `ui_preview` 後以 `curl`
  取根目錄、列根目錄、讀 `README.md` 渲染結果，輸出貼進回報
- [x] 3.5 新增 `docs/research/2026-09-27/files-check.js`（headless Chrome＋CDP，沿用 `visual-check.js` 的啟動、收尾、狀態注入與段落代號
  寫法，可只跑指定段落，段落名稱加 capability 前綴如 `file-review/`、`live-output/`）：為 file-review spec 中屬於前端的每個 scenario 與
  live-output delta 新增的三個 scenario（「選定 pane 時切回 Live Output 分頁」「檔案分頁期間不請求輸出」「切回時保持貼底」）各寫一段；
  同時在 `visual-check.js` 新增 cockpit-dashboard delta 的「分頁很多不撐破頁面」「頻繁重畫不影響檔案分頁」「Markdown 檢視遵守色彩與對比」段；改寫檔案的段落只改 `ui_preview` 的暫存副本；驗收＝對目前前端執行時自我測試段通過、
  各 scenario 段依預期失敗（RED），腳本結束後沒有殘留程序

## 4. 前端

- [x] 4.1 分頁骨架（design D6）：`index.html` 新增 `#files`、`#review`，`#output` 移入 `#review` 成為第一個分頁；左欄「Project／檔案」分頁列；
  下半部分頁列（`role="tablist"`，只有 Live Output 時也顯示）；`output.js` 在分頁不可見時停止發新請求、切回時立即請求並保留貼底
  （`keepPinnedAcross()`）；選定 pane 時切到 Live Output 分頁；新增 `/app/files.js`、`/app/viewers.js` 路由與內嵌；驗收＝`files-check.js`
  的 `live-output/檔案分頁期間不請求輸出`、`live-output/切回時保持貼底`、`live-output/選定 pane 時切回 Live Output 分頁` 段通過，
  `http.rs` 的 cockpit-dashboard「路由與 content-type」通過，`visual-check.js` 版面段依 spec 改寫「Live Output」為「下半部分頁區」後全綠，
  既有六支腳本全綠
- [x] 4.2 檔案樹：根目錄查詢、延遲展開、依根目錄保留展開狀態、「重新整理」、空狀態與錯誤文案（design D10）、icon、`omitted`／`skipped`
  提示、鍵盤操作與 `aria-expanded`、整頁重畫不動檔案樹；驗收＝`files-check.js` 的「切到檔案分頁」「沒有選定 pane」「展開狀態跨根目錄保留」
  「重畫不影響檔案樹」段通過，既有腳本全綠
- [x] 4.3 檔案分頁與還原：開檔、重複開啟、關閉規則、鍵盤操作、分頁列內部橫向捲動、工具列（相對路徑、最後讀取時間、「在 VS Code 開啟」）、
  `localStorage` 還原（讀寫皆 try/catch）；驗收＝`files-check.js` 的「開檔新增分頁」「重複開啟不新增」「關閉目前分頁」「切換 Project
  不影響分頁」「重新整理後還原」「儲存內容損毀」「Windows 檔案」「WSL 檔案」（連結 `href` 斷言；WSL 依 1.2 結果）段、`visual-check.js`
  的「分頁很多不撐破頁面」「頻繁重畫不影響檔案分頁」段通過，既有腳本全綠
- [x] 4.4 Markdown、純文字、HTML 與不支援的檢視器（design D8、D11）：連結與圖片改寫、錨點捲動、行號、sandbox iframe、錯誤文案；
  驗收＝`files-check.js` 的「md 相對連結在分頁區開啟」「外部圖片不載入」「HTML 內的腳本不執行」（含 `style.css` 載入成功）「純文字不被解讀」段、
  `visual-check.js` 的「Markdown 檢視遵守色彩與對比」段通過，既有腳本全綠
- [x] 4.5 PDF 檢視器（design D7）：延遲載入 pdf.js、佔位框＋`IntersectionObserver`、頁碼／翻頁／縮放／符合寬度、解析失敗文案；
  驗收＝`files-check.js` 的「中文 PDF」段通過（第 1 頁中文標題以 canvas 像素與空白頁比對證明有字，並附一張 viewport 截圖給控制端目視），
  既有腳本全綠
- [x] 4.6 自動更新：只查目前檔案分頁、2 秒節奏、不堆積、切換時立即查、捲動位置與頁碼保留、過期標示、丟棄舊回應；驗收＝`files-check.js`
  的「改檔後更新並保住捲動」「Live Output 分頁時不查詢」「檔案被刪掉後又出現」段通過，既有腳本全綠

## 5. 收斂與驗收

- [x] 5.1 全面驗收：`files-check.js`、`visual-check.js`（含完整「文字對比」段，Markdown 內容納入、PDF canvas 與 iframe 排除）與既有六支
  腳本全跑，並在 1536×1024、1100、700 三種寬度各存一張 viewport 截圖（不用整頁截圖、不隱藏捲軸）到 scratch；驗收＝全部段落通過，
  截圖路徑寫進回報，交由使用者目視驗收
- [x] 5.2 WSL 真機驗收：WSL 端測試 server 可用時，以正式服務（`cargo run -p cockpit`）連 WSL runtime，選一個 cwd 在 WSL repo 內的 pane，
  開啟其 `README.md` 並改寫一次確認自動更新；在該 repo 內建立指向 `/etc/passwd` 的符號連結，確認原始內容端點回 403
  （design 風險；若洩漏則停下回報，不自行修補）；需要在 WSL 端建立 tab 時才設 `HERDR_CLIENT_TEST_ALLOW_WSL_WRITES=1`（`AGENTS.md`）；結果寫進 `docs/research/2026-09-27/file-review-probe.md`。不可用時在 ledger 註明未跑與原因；
  驗收＝probe 文件或 ledger 有紀錄
- [x] 5.3 文件：`cockpit/README.md` 補檔案瀏覽與 Review 的說明（端點、安全邊界、vendored 資源升版方式）；新增
  `docs/research/2026-09-27/files-check.md` 說明腳本用法與段落代號；`docs/research/2026-09-23/visual-check.md` 補新段落；`CONTEXT.md` 補「檔案分頁」「檔案根目錄」詞條並與 HERDR 的 Tab 區分；
  主規格 `openspec/specs/live-output/spec.md` 的 Purpose 把「常駐於版面中的顯示」改為「作為下半部第一個分頁的顯示」（delta 改不到 Purpose）；
  驗收＝`markdownlint-cli2 "**/*.md"` 0 issues
- [x] 5.4 全 gate 與整支分支 review：repo 根跑 `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test --workspace &&
  cargo test -p cockpit --example ui_preview && markdownlint-cli2 "**/*.md" && openspec validate --all`，所有腳本全跑；以
  `codex-companion.mjs adversarial-review --wait --base main` 做整支分支審查，只採信 log 最後「# Codex Adversarial Review」段的結論，
  findings 先重現再處理；驗收＝gate 輸出全綠、腳本全綠、Codex 結論與處理紀錄寫進 SDD ledger
- [x] 5.5 交接：依 `~/.claude/guides/handover-template.md` 整份重寫 `docs/handover.md`（active change、下一段為 5b、本段踩過的坑）；
  驗收＝`markdownlint-cli2 "**/*.md"` 0 issues，使用者確認目視驗收結果後才標記完成
