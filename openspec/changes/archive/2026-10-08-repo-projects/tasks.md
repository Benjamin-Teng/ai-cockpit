# tasks：repo-projects

> 執行路徑：SDD｜理由：改動橫跨 `cockpit-git`、`cockpit-core`、`cockpit` 後端與前端四塊，各自可以獨立驗收；
> 涉及狀態檔格式升版、寫入服務的單一鎖與背景工作的並發（design D2～D5），後面的 task 建在前面之上，錯了返工貴。
>
> **審查**：依專案 memory `opus-review-counts-as-codex-review`，由 Opus 5.5 subagent 審查 diff，視同 Codex review。分兩批：
> 第 4 節（後端）完成後審一次（4.6），收尾前審整支分支（7.2）。findings 與處理方式記在 `sdd-ledger.md`。
>
> **授權**：使用者 2026-10-08 授權其餘設計與實作由 Claude 全權決定，最後回報決定清單。**不併回 `main`、不發版**，等使用者確認。

**工作樹**：`D:\projects\ai-cockpit`，分支 `feat/repo-projects`。

通則（每個 task 都適用）：

- **品質 gate**：每個 task 結尾跑 `cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test --workspace`；
  有改 `cockpit/examples/` 時加跑 `cargo test -p cockpit --example ui_preview`；有改 `.md` 時在 repo 根跑
  `markdownlint-cli2 "**/*.md"` 並核對 `Linting: N files` 的 N 不為 0；有改 `openspec/` 時跑 `openspec validate --all`。
  gate 單獨跑、看結束碼，不接管線（專案 memory `gate-exit-code-swallowed-by-pipe`）。輸出貼進回報。
- **TDD**：實作者載入 `superpowers:test-driven-development`，新行為先寫會失敗的測試，回報附 red → green 證據。
  測試名稱盡量對應 spec scenario。
- **跑腳本前先重建**：改完 `cockpit/assets/` 一定先 `cargo build -p cockpit --example ui_preview` 再跑腳本
  （專案 memory `acceptance-scripts-run-stale-embedded-assets`）。
- **既有腳本**＝1.1 寫進 `sdd-ledger.md` 的清單。每個改前端或投影的 task 結束時既有腳本必須全綠；只能改被本 task 打壞、
  且 spec 已改變的斷言，不得放寬 spec 沒改變的斷言；腳本修改與產品修改分開 commit，commit 訊息列出被改的斷言與對應 scenario。
  腳本不可並行，一律前景跑。
- **port**：跑腳本前用 `netstat -ano` 確認 7770、7830 一帶與 CDP port 沒人在用。使用者自己的 Cockpit 也用 7770
  （`docs/handover.md` 第 1 節）。**不是自己開的程序只能回報、不准砍**（專案 memory
  `harness-pid-checks-misreport-on-windows`）。
- **暫存 git repo** 一律建在 repo 外（`%TEMP%` 下），建立後先驗 `git rev-parse --show-toplevel` 是暫存目錄本身
  （專案 memory `git-in-subdir-falls-through-to-enclosing-repo`）。
- **HERDR 唯讀**：不新增任何 HERDR method（AGENTS.md）。
- **介面文字**一律放進 `cockpit/assets/app/i18n.js` 中英兩份字典；新的後端錯誤 code 與 `Message` 變體都要有 `msg.<code>`
  （對帳測試 `cockpit/tests/http.rs` `every_backend_message_code_has_a_msg_key_in_both_dictionaries`）。
- **註解與勾選**：程式碼註解引用 task 寫成 `repo-projects task N.M`；`tasks.md` 由控制端統一勾。
- **提交**：一次只有一個 subagent 改工作樹；commit 只 `git add <具體路徑>`；不 push。

## 1. 基線

- [x] 1.1 記錄基線與既有腳本清單。
  - 腳本清單：`docs/handover.md` 第 1 節「可用指令」列出的 15 支（file-split-view 收尾全綠的集合）；`idle-exit-check.js`
    測啟動器、不列入。
  - 跑 `cargo test --workspace`、`cargo test -p cockpit --example ui_preview` 與清單中全部腳本。
  - 把清單、passed／failed／ignored 數字與各腳本結果寫進 `openspec/changes/repo-projects/sdd-ledger.md`。
  - 驗收＝ledger 有清單與數字，failed 為 0、腳本全綠；已知偶發項目重跑一次並記錄。

## 2. repo 身分查詢（`cockpit-git`）

- [x] 2.1 新增 sealed 查詢 `rev-parse --path-format=absolute --git-common-dir --git-dir --show-toplevel`（design D1）。
  - 動手前對 git 官方文件（git-scm.com `git-rev-parse`）確認三個旗標與輸出順序，查證結果與日期寫進 ledger（鐵則 3）。
  - 照 `cockpit-git` 既有查詢（例如 `BlobId`）的模式：query struct、parse 模組、`lib.rs` 匯出、argv 斷言測試。
  - parse 測試涵蓋：三行正常輸出、行數不對、空行、CRLF 結尾。
  - 以 `#[ignore]` 之外的整合測試，在 `%TEMP%` 建主 repo＋linked worktree＋子目錄＋非 repo 目錄＋submodule，實際跑
    Windows git 驗證三行輸出與結束碼。建 submodule 要 `git -c protocol.file.allow=always submodule add`（git 2.38.1 起本機
    路徑預設禁止）；commit 一律帶 `-c user.name=… -c user.email=…`，CI 上沒有全域身分。
  - 驗收＝新測試先紅後綠；`cargo test -p cockpit-git` 通過。

## 3. Domain 與投影（`cockpit-core`）

- [x] 3.1 Repo Project 定義、`pane_repos`、固定 pane 綁定與展開函數（design D3；spec `repo-projects`「由 pane 推導
  workstream 與 task」「Repo Project 工作線的固定 pane 綁定」「Repo Project 與手寫 project 並列及 id 撞名」）。
  - `DomainState` 加 Repo Project 定義與 `pane_repos`；純函數輸出實際生效的 project 清單與撞名警告。
  - workstream／task id `<runtime>~<pane id>`、名稱規則、排序、worktree 標註、`source: pane` 的三種綁定狀態。
  - 新的 `Message` 變體 `repo_project_id_conflict`，同一個 task 在 `i18n.js` 兩種語言加 `msg.repo_project_id_conflict`
    （否則對帳測試先紅）。
  - `DomainState` 另有 `repo_progress`（與 `progress` 分開，design D3、D4）。
  - 驗收＝單元測試涵蓋上列 scenario；`cargo test --workspace` 通過（含對帳測試）。
- [x] 3.2 投影新欄位（design D8；spec `state-projection`「投影形狀」「Project 投影」）。
  - project 的 `kind`、`repo`，workstream 的 `worktree`，最上層 `detected_repos`（排序與 `pane_count`）。
  - Repo Project 的 task 標記為 `none` 時就是目前 task（design D4）。
  - 驗收＝投影測試涵蓋上列 scenario；version 只在內容改變時遞增的既有測試照常綠。

## 4. 後端（`cockpit`）

- [x] 4.1 狀態檔 v3 與位置（design D5；spec `pipeline-progress`「狀態檔格式與持久化」「狀態檔載入與容錯」、
  `pipeline-config`「狀態檔位置」）。
  - v3 讀寫、v1／v2 升級、v1／v2 出現 `repo_projects` 視為損毀、v4 不支援、Repo Project 定義不合規則時啟動失敗、
    stage 不存在退回第一個 stage 加警告、未知 runtime 的進度忽略。
  - 零設定模式的狀態檔位置 `%LOCALAPPDATA%\ai-cockpit\cockpit.state.json`（第一次寫入時建立資料夾）；`LOCALAPPDATA`
    不存在與 inline 設定時只存記憶體。測試以注入的環境值驗證，不碰使用者真的 `%LOCALAPPDATA%`。
  - 寫入服務改為永遠建立（`app.rs`），沒有路徑時只更新記憶體；失效覆蓋清除工作照常接上。
  - 寫檔與否改成比較序列化後的檔案內容（design D3「何時寫檔」）。
  - 會被改寫的既有測試先列進 ledger 再動：`cockpit/tests/app.rs` 的 `no_projects_creates_no_state_file`，以及
    `cockpit/src/config.rs` 中斷言 `state_path` 為 None 的測試；改寫時保留原意（沒有被接受的操作就不建檔）。
  - 驗收＝上列 scenario 各有測試，含「只有 v2 檔、沒有任何操作、pane 進出 → 檔案仍是 v2」；既有 v1／v2 測試照常綠。
- [x] 4.2 寫入服務的 Repo Project 操作（design D3、D4、D6；spec `repo-projects` 的加入、修改、移除、驗證、id 產生、
  進度保存與清除）。
  - 加入、改名、改 stages（含 `from` 對應）、移除、更新 `pane_repos`、清除消失 pane 的進度，全部在同一把鎖內完成，
    每次重算實際生效的 project 清單再 `set_domain`。
  - 寫檔時 Repo Project 的進度依對照表全部寫出，不以目前展開的 task 過濾，也不經過 `resolve_tasks`（design D4）。
  - 清除檢查在鎖內依 core `RuntimeStore` 當下的連線狀態與 pane 樹判定，不讀投影（design D4）。
  - 驗收＝單元測試涵蓋上列 scenario，含「啟動初期 `pane_repos` 為空時寫入不抹掉其他 pane 的進度」「runtime 斷線不清除」
    與並發寫入不遺失。
- [x] 4.3 `RepoResolver` 背景工作（design D2、D4）。
  - 監看投影、只查已連線的 runtime、`(runtime, cwd)` 快取（成功 10 分鐘、其他 60 秒過期）、60 秒定時器、依序查詢、
    內容不變不送出、每輪結束請求一次清除檢查。
  - 查詢介面以 trait 注入；測試用假查詢與 tokio 暫停時間，涵蓋：新 pane 歸類、cwd 改變重查、過期重查、暫時錯誤不歸類、
    不無限循環（送出次數有上限）、未連線 runtime 不查詢且沿用上次歸類、重新連上後第一輪清除關閉期間消失的 pane。
  - repo key 正規化（Windows：反斜線、整串小寫；WSL：保留大小寫）與 WSL POSIX 路徑轉 `\\wsl.localhost\...` 有單元測試。
  - 驗收＝上列測試通過；`app.rs` 接上正式實作。
- [x] 4.4 HTTP 端點（design D6、D7；spec `repo-projects` 管理端點、`agent-reporting`「免帶 id 推進」「pane 身分判定」
  「宣告目前 task」、`pipeline-progress`「綁定覆蓋端點」「寫入端點只接受本機同源請求」、`ui-language`「後端訊息代碼」）。
  - `POST /api/repo-projects`、`PATCH`／`DELETE /api/repo-projects/{pid}`、`POST /api/agent/advance`，全部掛既有來源檢查；
    覆蓋端點對固定 pane 的 workstream 回 409 `not_overridable`。
  - 新 code（`repo_not_detected`、`repo_already_added`、`not_repo_project`、`invalid_body`、`invalid_name`、`invalid_stages`、
    `not_overridable`、`no_task_for_pane`、`ambiguous_task`、`repo_project_id_conflict`）在 `i18n.js` 兩種語言都有 `msg.<code>`。
  - 驗收＝`cockpit/tests/http.rs` 涵蓋上列 scenario（含來源檢查 403、方法不允許）；對帳測試通過。
- [x] 4.5 `ui_preview` fixture（design Risks「`ui_preview` 沒有真的後端狀態」）。
  - `ui_preview` 推送手工組的投影，寫入端點只記錄請求。fixture 直接帶 `detected_repos`（兩個）與一個 Repo Project
    （含一條 linked worktree 的工作線、`source: pane` 的綁定），新路由（`/api/repo-projects`、`/api/agent/advance`）掛到
    `ui_preview` 自己的假路由，回應照規格的成功碼並記錄請求，供前端腳本斷言。
  - 不在 fixture 裡放真的 git repo；fixture 字串不得含使用者名稱。
  - 驗收＝`cargo test -p cockpit --example ui_preview` 通過；既有腳本全綠（新增 project 會改變左欄清單，依通則處理受影響的斷言）。
- [x] 4.6 Opus 審查（後端）：派 Opus 5.5 subagent 審 `main..feat/repo-projects` 的 diff，focus 為狀態檔、寫入服務的鎖、
  resolver 的並發與清除條件、端點驗證。findings 依 `superpowers:receiving-code-review` 處理：先實測重現才採信。
  驗收＝ledger 記錄結論與每個 finding 的處理。

## 5. 前端（`cockpit/assets/app/`）

- [x] 5.1 偵測到的 repo 區與加入（design D9；spec `cockpit-dashboard`「Project 切換」）。
  - 左欄 Project 分頁的「偵測到的 repo」區、`pane_count`、「加入」鈕（送出依介面語言的預設 stages），加入後自動選取。
  - 空狀態文字拿掉「需要重啟」。
  - 新增驗收腳本 `docs/research/2026-10-08/repo-projects-check.js` 與用法 `repo-projects-check.md`，寫法比照
    `docs/research/2026-10-04/split-check.js`（headless Chrome＋raw CDP、真實輸入、以 `#version` 的 `data-state-version`
    判斷重畫）；使用新的固定 port，先 `netstat` 確認不與既有腳本重複。
  - 驗收＝新斷言先紅後綠；既有腳本全綠。
- [x] 5.2 管理選單：改名、編輯 stages 對話框、移除確認（design D9；spec `cockpit-dashboard` 對應 scenario）。
  - 對話框跨整頁重畫保留內容（專案 memory `full-repaint-discards-state-held-only-in-dom`）；鍵盤可操作、Esc 關閉、焦點回到
    觸發鈕。
  - 名稱一律以 `textContent` 呈現。
  - 驗收＝`repo-projects-check.js` 加斷言先紅後綠：送出的請求本體正確（改名、stages 含 `from`、移除）、對話框驗證錯誤、
    後端錯誤 code 以介面語言顯示、跨重畫保留；既有腳本全綠。行為面（卡片回到第一個 stage、移除後回到偵測區）由 4.2／4.4 的
    整合測試與 7.3 真機冒煙驗證。
- [x] 5.3 Factory Floor 的 Repo Project 呈現（spec `cockpit-dashboard` 的 worktree 標註、改綁鈕隱藏）。
  - worktree 標註、`source: pane` 不顯示「改綁」鈕。
  - 驗收＝`repo-projects-check.js` 加斷言先紅後綠；既有腳本全綠。

## 6. 設計審核與文件

- [x] 6.1 設計審核（專案 memory `frontend-appearance-reviewed-by-frontend-design-skill`）。
  - 用 `ui_preview` 拍 1536、1100、700 三種寬度下的偵測區、管理選單、stage 對話框截圖，對照
    `docs/direction-01-visual-design.md`，過 frontend-design 審核。截圖放 repo 外；要進 repo 的逐張看圖去識別化（專案 memory
    `deidentification-must-inspect-images-not-just-grep`，fixture 路徑含 `%TEMP%` 使用者名稱）。
  - 會動到設計文件的建議交給使用者決定。
  - 驗收＝findings 已處理或記錄待使用者決定，ledger 有截圖路徑。
- [x] 6.2 文件。
  - `CONTEXT.md`：Repo Project、偵測到的 repo、固定 pane 綁定；更新 Project、Workstream、RuntimeBinding 詞條。
  - 新 ADR `docs/adr/0008-repo-projects-derived-from-panes.md`（工作線由 pane 推導、定義存狀態檔，design D3、D5）。
  - `cockpit/README.md`、`README.md`：加入 repo 的用法、`POST /api/agent/advance`、cwd 反映最多約 30 秒、Windows／WSL 同一
    repo 會列成兩個的限制。
  - `CHANGELOG.md` 最上方建 `## [Unreleased]`，註明狀態檔 v3 與降版限制。
  - 驗收＝`markdownlint-cli2 "**/*.md"` 0 error（N 不為 0）。

## 7. 收尾

- [x] 7.1 全 gate 與全部腳本，各指令單獨跑、逐一看結束碼：`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、
  `cargo test --workspace`、`cargo test -p cockpit --example ui_preview`、`markdownlint-cli2 "**/*.md"`、
  `openspec validate --all`、既有腳本加 `repo-projects-check.js`、去識別化檢查
  `node docs/research/2026-10-02/deid-check.js`。驗收＝輸出貼進 ledger，全綠。
- [x] 7.2 Opus 審查（整支分支）：`main..feat/repo-projects` 全部 diff，處理方式同 4.6。驗收＝ledger 記錄結論段與每個 finding
  的處理。
- [x] 7.3 真機冒煙（Claude 執行，唯讀）：用 repo 外的臨時設定檔、未被佔用的 port 啟動 Cockpit 接 Windows 端 HERDR，
  headless Chrome 確認偵測到的 repo 與實際 pane 一致、加入後工作線正確、worktree 標註正確。臨時狀態檔放 repo 外，結束後只關
  自己開的程序。結果寫進 `docs/research/2026-10-08/repo-projects-live.md`，截圖不進 repo。驗收＝該檔存在、markdownlint 0 error。
- [x] 7.4 交接：重寫 `docs/handover.md`（依 `~/.claude/guides/handover-template.md`），寫明分支未併回、等使用者確認；
  刪除專案 memory `pending-update-signing-and-project-config-ux` 中「新增 project 免手寫 toml」一項。
  驗收＝markdownlint 0 error；`git status` 乾淨。
