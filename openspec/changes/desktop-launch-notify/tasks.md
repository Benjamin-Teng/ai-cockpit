# tasks：desktop-launch-notify

> 執行路徑：SDD｜理由：跨後端（設定、`/ws` 計數、閒置關閉）、新執行檔（啟動器）、PowerShell 安裝腳本、前端新模組與設定面板、`ui_preview` fixture、
> 兩支新驗收腳本，共 17 個可獨立驗收的 task；啟動與關閉流程錯了會留下孤兒程序或無聲失敗，返工貴。
>
> **審查**：本專案 Opus 5.5 審查視同 Codex（使用者 2026-10-02 指示）。依階段批次審：第 2 節完成後審後端與啟動器（2.6），第 3 節完成後審前端（3.6），
> 5.3 審整支分支。findings 實測重現後才改，處理記在 `sdd-ledger.md`。
> **決策授權**：使用者 2026-10-02 指示細節由 Claude 決定、事後彙整；所有裁決以 `Ruling:` 記入 ledger。

通則（每個 task 都適用）：

- 結尾跑 `cargo fmt --check`、`cargo clippy -p <本 task 改到的 crate> --all-targets -- -D warnings`、`cargo test -p <同上>`；
  有改 `cockpit/examples/ui_preview.rs` 時加跑 `cargo test -p cockpit --example ui_preview`；有改 `.md` 時在 repo 根跑
  `markdownlint-cli2 "**/*.md"`（核對 `Linting: N files` 不為 0）；有改 `openspec/` 時跑 `openspec validate --all`。輸出貼進回報。
- Rust 的 task 以 `superpowers:test-driven-development` 進行，回報附 red→green 證據；前端 task 的新斷言先在未修的程式上跑出紅，再修。
- 前端資源以內嵌方式編進執行檔：改完 `cockpit/assets/` 一定先 `cargo build -p cockpit --example ui_preview` 再跑任何腳本。
- **「既有腳本」**＝`docs/handover.md` 第 1 節列的 12 支驗收腳本。**每個改前端、`ui_preview` 或 `/ws` 的 task 結束時全部必須全綠**：只改被本 task 打壞、
  且 spec 已改變的斷言；腳本修改與產品修改分開 commit。驗收腳本不可並行跑，一律前景跑。已知偶發（重跑一次並記錄）：`live-output-check.js`、
  `factory-floor-check.js`、`git-check.js`「收尾衛生」、`reconnect-check.js`（Chrome 未出現 page target）。
- 跑腳本前確認 7770 沒有人在用；**不是自己開的程序只能回報、不准砍**；跑完依 PID 收尾。
- 對 HERDR 只送唯讀請求；Windows 端不得 `herdr server stop`（`AGENTS.md`）。驗收用的 `cockpit` 一律以臨時設定（runtime 指向不存在的 socket）或 `ui_preview` 執行。
- 推送前跑 `node docs/research/2026-10-02/deid-check.js`（檔案模式）0 命中；新截圖逐張看圖。
- 程式碼註解若要引用 task，寫成 `desktop-launch-notify task N.M`。`tasks.md` 由控制端統一勾。
- 多個 subagent 不同時改同一個工作樹；commit 只 `git add <具體路徑>`。

## 1. 基線

- [x] 1.1 記錄基線：`cargo test --workspace`、`cargo test -p cockpit --example ui_preview`、12 支既有腳本，數字寫進 `openspec/changes/desktop-launch-notify/sdd-ledger.md`

## 2. 後端與啟動器

- [x] 2.1 `--exit-when-idle`（design D6）：`config.rs` 解析（與 `--config` 併用、順序不拘、重複給視為錯誤）；`http.rs` 以 drop guard 維護 `/ws` 連線數（`watch<usize>`
  放進 `AppState`）與 `GET /`／`GET /api/state` 的最近活動時間；`app.rs` 閒置監看（`IdlePolicy`，正式值 60 秒／10 秒）與 Ctrl-C 合併；`main.rs` 的 tracing
  改 `.with_ansi(stderr().is_terminal())`（design D3）。驗收：單元測試以 `tokio::time::pause` 涵蓋 spec「閒置自動結束」五個 scenario；整合測試確認連上 `/ws`
  時計數為 1、關閉後回 0、`/api/state` 不計入連線數但更新活動時間；不帶旗標時既有測試全綠；因 `AppState` 欄位變動會改到 `ui_preview.rs`，既有腳本全綠
- [x] 2.2 `launch.rs` 純函式（design D1–D4）：引數解析、設定→網址、Cockpit 回應判斷、瀏覽器候選（注入「檔案是否存在」與環境變數）、後端引數與 log 路徑。驗收：單元測試
  涵蓋 spec「啟動器」各規則，含瀏覽器選擇順序、`COCKPIT_BROWSER` 不存在時略過、非 Cockpit 回應判為占用、log 路徑兩種情況
- [x] 2.3 `cockpit-launch` bin 與 `default-run`（design D1、D3、D5）：串接瀏覽器選擇、偵測、背景啟動（`CREATE_NO_WINDOW`、stdin null、log 導向、提早結束後重新偵測）、
  就緒輪詢、開瀏覽器、訊息框（含 `COCKPIT_LAUNCH_DIALOG_FILE` 測試入口）。驗收：`cargo build -p cockpit --bins` 成功、`cargo run -p cockpit -- --help` 類的既有用法
  仍選到 `cockpit`；以臨時設定、`COCKPIT_LAUNCH_DIALOG_FILE` 與 `COCKPIT_BROWSER` 指向一支只記錄引數的假瀏覽器（repo 外）實測 spec「啟動器」全部 scenario：
  沒在跑→啟動後端（引數為絕對路徑＋`--exit-when-idle`、`cockpit.log` 無 ANSI 色碼）並以 `--app=<網址>` 呼叫假瀏覽器；已在跑→不啟動新後端；埠被非 Cockpit 服務占用→
  訊息；狀態檔損毀→訊息含 log 路徑與最後數行；同時兩次→只留一個後端、兩次都開瀏覽器、無訊息；找不到瀏覽器→訊息且未啟動後端
- [x] 2.4 `scripts/install-desktop.ps1`（spec「桌面捷徑安裝腳本」）。驗收：以 `-InstallDir`、`-ShortcutDir` 指到暫存目錄實跑，檢查兩個執行檔與捷徑的目標、引數、
  工作目錄（有設定檔與零設定兩種）；讓暫存目錄中的 `cockpit.exe` 執行中再跑一次，確認腳本拒絕且檔案未變
- [x] 2.5 `docs/research/2026-10-02/idle-exit-check.js`（附 `.md`，design D10）：連上後關閉→約 10 秒開始關閉且結束碼 0；關閉後 3 秒內重連→持續執行；關閉 8 秒時
  `GET /api/state` 再 5 秒內重連→持續執行；不帶旗標→不結束。不要求在舊版跑紅（舊版不認得旗標、立即失敗，紅證明不了東西；行為由 2.1 的單元測試 red→green 把關）
- [x] 2.6 後端與啟動器階段審查（Opus 5.5）：範圍為第 2 節的 diff

## 3. 前端通知

- [x] 3.1 `ui_preview` 的 `COCKPIT_PREVIEW_TRANSITIONS`（design D9）。驗收：`cargo test -p cockpit --example ui_preview` 新增解析與套用測試；既有腳本全綠
- [x] 3.2 `docs/research/2026-10-02/notify-check.js`（附 `.md`，design D10）：逐條驗 spec「通知事件」「通知呈現」「通知設定」全部 scenario，以及 cockpit-dashboard delta
  新增的兩個 scenario（鈴鐺焦點、錯誤訊息保留）、`/app/notify.js` 路由、設定面板開啟時的對比與焦點外框、`renotify` 參數。先在 3.3 之前的程式上跑出紅，記錄
- [x] 3.3 `notify.js`、`render.js`（呼叫 observe、頂列鈴鐺）、`actions.js`（`notify-settings` 提早處理、`cockpitActions.selectPane`）、`index.html` 載入順序、
  `http.rs` 內嵌、`style.css`（design D7、D8）。驗收：3.2 腳本全綠、既有腳本全綠、
  `visual-check.js` 色票規則無新違規
- [x] 3.4 截圖：設定面板開啟時 1536／700 寬（`notify-check.js --screenshots`，遮罩真名），存 `docs/research/2026-10-02/`；逐張看圖
- [x] 3.5 外觀審核：frontend-design 審核模式評 3.4 截圖；採納的修正回到 3.3 並重跑 3.2
- [x] 3.6 前端階段審查（Opus 5.5）：範圍為第 3 節的 diff

## 4. 真機

- [x] 4.1 真機（控制端執行）：以安裝腳本安裝到真實位置、從桌面捷徑開啟、確認獨立視窗出現與畫面正常、在設定面板允許通知、關閉視窗後約 10 秒內 `cockpit.exe` 結束；
  再點捷徑兩次確認不重複啟動。紀錄寫 `docs/research/2026-10-02/desktop-launch-live.md`（只記中繼資料，真機畫面截圖不進 repo）

- [ ] 4.2 **使用者真機確認通知**（需使用者操作；整支分支審查 I3）：在設定面板按「允許通知」並允許；把 Cockpit 視窗最小化，等某個 pane 變成 `blocked`，
  確認收到 Windows 通知、點通知後視窗回到前景並選定該 pane；再最小化 10 分鐘以上確認仍收得到。結果補記 `docs/research/2026-10-02/desktop-launch-live.md`

## 5. 文件與收尾

- [x] 5.1 文件：ADR-0005 加「2026-10-02 補充」段（使用者實際需求、選擇啟動器＋網頁通知而非 Tauri 的理由）；`cockpit/README.md`（命令列 `--exit-when-idle`、
  `default-run`、啟動器與安裝腳本）；`CONTEXT.md` 若有「啟動」「通知」相關詞彙則補；`docs/handover.md` 的可用指令加安裝與捷徑說明。驗收：markdownlint 0 issues、`openspec validate --all` 通過
- [x] 5.2 全 gate：Rust gate、`ui_preview` 測試、markdownlint、`openspec validate --all`、12 支既有腳本、`idle-exit-check.js`、`notify-check.js`、`deid-check.js`（檔案＋`--history`）
- [x] 5.3 整支分支審查（Opus 5.5，最強模型）：範圍 `main..HEAD`；findings 處理後重跑 5.2
