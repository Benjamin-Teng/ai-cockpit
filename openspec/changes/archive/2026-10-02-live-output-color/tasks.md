# tasks：live-output-color

> 執行路徑：SDD｜理由：跨 `cockpit-core`（型別）、`cockpit-herdr`（新依賴與解析器）、`cockpit`（端點、前端、`ui_preview`）與驗收腳本，
> 共 18 個 task；端點 JSON 擴充屬對外格式，解析錯誤屬「不報錯、只畫錯」，前端改寫牽動既有驗收腳本，錯了返工貴。
>
> **審查**：使用者 2026-10-02 指示，本 change 的 Codex 審查一律由 Opus 5.5 取代（跑完視同取代 Codex）。依階段批次審，不逐 task：
> 第 4 節完成後審後端（4.3），第 5 節完成後審前端（5.6），7.3 審整支分支。findings 與處理記在 `sdd-ledger.md`，
> 處理走 superpowers:receiving-code-review（先實測重現再改）。

通則（每個 task 都適用）：

- 結尾跑 `cargo fmt --check`、`cargo clippy -p <本 task 改到的 crate> --all-targets -- -D warnings`、`cargo test -p <同上>`；
  有改 `cockpit/examples/ui_preview.rs` 時加跑 `cargo test -p cockpit --example ui_preview`；有改 `.md` 時在 repo 根跑
  `markdownlint-cli2 "**/*.md"`（核對 `Linting: N files` 不為 0）；有改 `openspec/` 時跑 `openspec validate --all`。輸出貼進回報。
- Rust 的 task 以 `superpowers:test-driven-development` 進行，回報附 red→green 證據；前端 task 的新斷言先在未修的程式上跑出紅，再修。
- 前端資源以內嵌方式編進執行檔：改完 `cockpit/assets/` 一定先 `cargo build -p cockpit --example ui_preview` 再跑任何腳本。
- **「既有腳本」**＝`docs/research/2026-09-15/reconnect-check.js`、`whatever-check.js`、`docs/research/2026-09-16/actions-check.js`、
  `channel-backoff-check.js`、`factory-floor-check.js`、`docs/research/2026-09-19/live-output-check.js`、
  `docs/research/2026-09-23/visual-check.js`、`docs/research/2026-09-27/files-check.js`、`docs/research/2026-09-28/git-check.js`、
  `docs/research/2026-10-01/progress-check.js`、`docs/research/2026-10-01/ui-fixes-check.js`。**每個改前端或 `ui_preview` 的 task
  結束時全部必須全綠**：只改被本 task 打壞、且 spec 已改變的斷言；腳本修改與產品修改分開 commit，commit 訊息逐條列出被改的斷言
  與對應的 spec scenario；不得放寬 spec 沒有改變的斷言。驗收腳本不可並行跑，一律前景跑。`live-output-check.js` R 段偶發 FAIL 是
  已知腳本競態（兩次分開的 `eval` 間節點被換掉），重跑一次並記錄。
- 本 change 的新前端斷言集中在新腳本 `docs/research/2026-10-02/output-color-check.js`（寫法比照 `docs/research/2026-10-01/ui-fixes-check.js`，
  raw CDP、headless Chrome，附 `output-color-check.md` 用法），由 5.3 建立。
- `factory-floor-check.js` 會覆寫 `docs/research/2026-09-16/task-5.2-scenario-d.png`，跑完 `git checkout --` 還原。
- 跑腳本前確認 7770 沒有人在用；**不是自己開的程序只能回報、不准砍**；跑完依 PID 收尾，確認 7770 與 CDP port 沒有 LISTEN。
- 對 HERDR 只送唯讀請求；Windows 端不得 `herdr server stop`（`AGENTS.md`）。pane 畫面原文只能存到 repo 外、用完即刪，不得進 repo。
- 截圖一律遮罩使用者名稱與主機名稱（作法見 `docs/research/2026-10-01/ui-fixes-check.js --screenshots`），commit 前逐張看圖。
- 程式碼註解若要引用 task，寫成 `live-output-color task N.M`。`tasks.md` 由控制端統一勾。
- 多個 subagent 不同時改同一個工作樹；commit 只 `git add <具體路徑>`。

## 1. 基線

- [x] 1.1 記錄基線：在分支 `feat/live-output-color` 跑 `cargo test --workspace`、`cargo test -p cockpit --example ui_preview`、既有腳本，把
  passed／failed／ignored 數字與各腳本結果寫進 `openspec/changes/live-output-color/sdd-ledger.md`；驗收＝ledger 有這些數字，failed 為 0、
  腳本全綠

## 2. `cockpit-core`：片段型別

- [x] 2.1 依 design D2 新增 `AnsiColor`、`SegmentStyle`、`OutputSegment`，`PaneOutput` 欄位改私有、加 `segments`、提供 `plain`／
  `from_segments` 與讀取函式；所有建構點（`cockpit-herdr/src/runtime.rs`、`cockpit/src/http.rs`、`cockpit/examples/ui_preview.rs`、
  `cockpit/tests/output_endpoint.rs`、`cockpit-core/tests/common/mod.rs`、`cockpit-core/tests/types.rs`）與直接讀欄位處
  （`cockpit-core/tests/runtime.rs`、`cockpit-herdr/tests/read_output.rs`）改用建構函式與讀取函式，行為不變。驗收：`cockpit-core` 單元
  測試涵蓋 `from_segments` 丟空片段、合併相鄰同樣式、`text` 等於串接、空輸入得空陣列（`runtime-model` delta「片段不變式由型別
  維持」），`plain("")` 得空陣列，`runtime-model`「假 runtime 回應輸出」的單一段無樣式片段，序列化省略 `None`／`false` 且樣式攤平（例 `{"text":"x","fg":"bright_red","bold":true}`）；`cargo test --workspace` 全綠

## 3. `cockpit-herdr`：ansi 解析

- [x] 3.1 加 `vte = "0.15"` 到 `cockpit-herdr`（不開額外 feature），新增 `pub mod ansi`，提供把 HERDR ansi 字串轉成
  `PaneOutput` 片段的函式（design D1、D4）。驗收：單元測試逐條覆蓋 spec「輸出樣式轉換」的每個 scenario，另含：每個 SGR 樣式參數的
  設定與取消、`0` 與空參數重設、`30`–`37`／`90`–`97`／`40`–`47`／`100`–`107` 全部 16 色、`39`／`49`、`38;5;N` 與 `38:5:N`、
  `38;2;R;G;B`、`38:2:R:G:B` 與 `38:2::R:G:B`、參數不完整（`38;5`、`38;2;1;2`）與超出範圍（`38;5;256`）不生效且後續參數照常、
  色立方與灰階各界線（無彩門檻 63／64、平均 47／48 與 159／160、每個色相界線兩側）、OSC 的 BEL 與 ST 結尾、DCS、帶私有前綴的
  CSI（`ESC[?25h`、`ESC[>4;2m`）、參數超過 32 個的 SGR、非色彩參數帶冒號子參數（`4:3`）、DEL 與 C1 字元、`\r` 丟棄、`\t` 與中文保留、結尾半截序列、串接等於 `text`
- [x] 3.2 `read_output` 改送 `format: Some(ReadFormat::Ansi)` 並以 3.1 解析（design D5）。驗收：`cockpit-herdr/tests/read_output.rs` 依
  `herdr-runtime-session` delta「讀取 pane 輸出」逐條對應：「送出的參數」改為斷言 `format` 為 `ansi`、「回應原樣交回」加斷言單一段
  無樣式片段、新增「帶樣式的回應」；其餘 scenario 的既有測試不變；`cargo test -p cockpit-herdr` 全綠

## 4. `cockpit`：端點

- [x] 4.1 `OutputBody` 帶出 `segments`（design D6）。驗收：`cockpit/tests/output_endpoint.rs` 新增 spec「讀到輸出」（第二行紅色 `error`）與
  「沒有樣式的輸出」scenario 的測試，並斷言回應中片段串接等於 `text`；既有端點測試不變且全綠
- [x] 4.2 後端整合確認：`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test --workspace` 全綠；以
  `cargo run -p cockpit --example ui_preview` 對既有模式請求 `/output`，確認 `segments` 為單段、`text` 與改版前相同。結果記入 ledger
- [x] 4.3 後端階段審查（Opus 5.5 取代 Codex）：範圍為第 2–4 節的 diff；findings 實測重現後才改，處理記入 `sdd-ledger.md`

## 5. 前端與 `ui_preview`

- [x] 5.1 `ui_preview` 新增 `ansi` 與 `ansi-flip` 輸出模式（design D8），以 `COCKPIT_PREVIEW_OUTPUT_MODES` 指定；樣本涵蓋 spec「輸出依樣式上色」
  各 scenario 需要的片段（design D8：「無前景＋7 種前景色票」×「無背景＋7 種背景色票」全組合、每種前景色票的反白、`black` 背景、
  `fg: red`＋`bg: white`＋`reverse`、粗體／斜體／底線、只帶 `dim`、上了色的 `<script>`／`<b>` 字樣）。驗收：
  `cargo test -p cockpit --example ui_preview` 新增測試斷言兩種模式的片段內容與 `ansi-flip` 交替；既有腳本全綠
- [x] 5.2 `output.js` 改為依片段建節點、片段字串去重，`style.css` 新增 `ansi-*` 規則與過期覆寫（design D7）。驗收：既有腳本全綠
  （`live-output-check.js` 以 `textContent` 比對的段落不需修改）；`visual-check.js` 的色票規則沒有新增違規
- [x] 5.3 建立 `docs/research/2026-10-02/output-color-check.js`（附 `output-color-check.md`），以 `ui_preview` 的 `ansi`／`ansi-flip` 模式逐條驗
  spec「輸出依樣式上色」全部 scenario（含以 Chrome 計算後顏色合成到 `--bg-deep` 量對比 ≥4.5:1）、「輪詢與顯示」的「相同內容不重寫」
  （MutationObserver 確認無變動）與「只有顏色改變也會重畫」、「失敗與消失的呈現」的「過期時有色內容一併轉暗」，並斷言面板
  `textContent` 等於回應 `text`。紅的證據：在 repo 外以 `git worktree add` 取 5.1 完成時的 commit、複製新腳本過去、
  `cargo build -p cockpit --example ui_preview` 後跑，依賴新行為的斷言（上色、淡底、對比、只換色也重畫、過期轉暗）必須為紅
  （`textContent` 相等、相同不重寫、HTML 不被解讀在舊程式本來就綠，不要求紅）；記錄輸出後移除該 worktree，再於現行 HEAD 全綠。
  注意 7770 與 CDP port 不得與其他程序相撞
- [x] 5.4 截圖：`output-color-check.js --screenshots` 產生 1536、1100、700 寬的 `ansi` 模式截圖（遮罩），存
  `docs/research/2026-10-02/`。驗收：逐張看圖確認沒有使用者名稱與主機名稱，PNG 位元組與中繼資料 grep 真名為 0
- [x] 5.5 外觀審核：以 frontend-design skill 的審核模式評 5.4 截圖（spec 已定死的色票對應與淡底比例不在審核範圍，只評剩餘自由度）；
  採納的建議回到 5.2 修正並重跑 5.3，改設計文件或 spec 的建議交使用者。結果記入 ledger
- [x] 5.6 前端階段審查（Opus 5.5 取代 Codex）：範圍為第 5 節的 diff；處理同 4.3

## 6. 真機驗收

- [x] 6.1 重跑 `docs/research/2026-09-19/live-output-real-check.js`（WSL 測試 server，依該腳本的啟動與收尾流程）。若因 `text` 改由 ansi
  解析（行尾空白、結尾換行，design D3）而有逐字比對失敗，改為比對語意並在 commit 訊息說明；驗收＝全綠
- [x] 6.2 Windows 端真機：以實際設定啟動 `cockpit`，選定一個 Claude Code pane，確認有色片段呈現、`textContent` 與端點 `text` 相同、
  過期轉暗；另以 `probe_pane_read` 對同一 pane 量測端點 `text` 與 HERDR `format=text` 的差異只在行尾空白與結尾換行。紀錄寫
  `docs/research/2026-10-02/output-color-live.md`（只記中繼資料與遮罩截圖，不記畫面原文、不記其他 repo 名稱）

## 7. 文件與收尾

- [x] 7.1 文件：`CONTEXT.md`「Live Output」改寫「純文字」為「依 HERDR 樣式上色的純文字」；設計文件
  `docs/superpowers/specs/2026-09-13-cockpit-mvp-design.md` 決策表「ANSI parsing crate」兩列填入 `vte`（change 8）；`openspec/specs/live-output/spec.md`
  的 Purpose 第一句改為「依 HERDR 的樣式上色顯示」（OpenSpec 規定 Purpose 直接改主 spec）。驗收：markdownlint 0 issues、
  `openspec validate --all` 通過
- [x] 7.2 全 gate：`cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test --workspace && cargo test -p cockpit --example ui_preview && markdownlint-cli2 "**/*.md" && openspec validate --all`
  與既有腳本、`output-color-check.js` 全綠；數字記入 ledger
- [x] 7.3 整支分支審查（Opus 5.5 取代 Codex，最強模型）：範圍 `main..HEAD`；findings 處理後重跑 7.2
