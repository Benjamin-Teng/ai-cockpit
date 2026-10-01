# tasks：progress-model

> 執行路徑：SDD｜理由：跨 `cockpit-core`、`cockpit` 後端、前端三塊，約 13 個可獨立驗收的 task；改動狀態檔格式（v1→v2 相容）與
> 寫入交易（單鎖、先落檔再生效），並新增對外 HTTP 入口與身分判定，屬中高風險；前端改動會碰到既有驗收腳本依賴的「running」前提。
>
> **審查紀錄**：Codex 週限額用盡，使用者 2026-10-01 決定改以 Opus 5.5 subagent 做整支分支審查＋複審取代 3.5、4.4、5.3 的 Codex 審查（見 `sdd-ledger.md`）。原安排：Codex adversarial-review 依階段批次跑，不逐 task——第 3 節完成後審 `cockpit-core`＋後端，第 4 節完成後審前端，
> 5.3 審整支分支。findings 與處理記在 `sdd-ledger.md`。

通則（每個 task 都適用）：

- 結尾跑 `cargo fmt --check`、`cargo clippy -p <本 task 改到的 crate> --all-targets -- -D warnings`、`cargo test -p <同上>`；
  有改 `cockpit/examples/ui_preview.rs` 時加跑 `cargo test -p cockpit --example ui_preview`；有改 `.md` 時在 repo 根跑
  `markdownlint-cli2 "**/*.md"`（核對 `Linting: N files` 不為 0）；有改 `openspec/` 時跑 `openspec validate --all`。輸出貼進回報。
- Rust 的 task 以 `superpowers:test-driven-development` 進行，回報附 red→green 證據。
- 前端資源以內嵌方式編進執行檔：改完 `cockpit/assets/` 一定先 `cargo build -p cockpit --example ui_preview` 再跑任何腳本。
- **「既有腳本」**＝`docs/research/2026-09-15/reconnect-check.js`、`whatever-check.js`、`docs/research/2026-09-16/actions-check.js`、
  `channel-backoff-check.js`、`factory-floor-check.js`、`docs/research/2026-09-19/live-output-check.js`、
  `docs/research/2026-09-23/visual-check.js`、`docs/research/2026-09-27/files-check.js`、`docs/research/2026-09-28/git-check.js`。
  **每個改前端或 `ui_preview` 的 task 結束時全部必須全綠**：只改被本 task 打壞、且 spec 已改變的斷言；腳本修改與產品修改分開
  commit，commit 訊息逐條列出被改的斷言與對應的 spec scenario；不得放寬 spec 沒有改變的斷言。驗收腳本不可並行跑（共用暫存目錄）。
- `factory-floor-check.js` 會覆寫 `docs/research/2026-09-16/task-5.2-scenario-d.png`，跑完 `git checkout --` 還原。
- 跑腳本前確認 7770 沒有人在用；**不是自己開的程序只能回報、不准砍**；跑完依 PID 收尾，確認 7770 與 CDP port 沒有 LISTEN。
- 對 HERDR 只送唯讀請求；Windows 端不得 `herdr server stop`（`AGENTS.md`）。
- 程式碼註解若要引用 task，寫成 `progress-model task N.M`。`tasks.md` 由控制端統一勾。
- 多個 subagent 不同時改同一個工作樹；commit 只 `git add <具體路徑>`。

## 1. 基線

- [x] 1.1 記錄基線：在分支 `feat/progress-model` 跑 `cargo test --workspace`、`cargo test -p cockpit --example ui_preview`、既有腳本，
  把 passed／failed／ignored 數字與各腳本結果寫進 `openspec/changes/progress-model/sdd-ledger.md`；驗收＝ledger 有這些數字，failed 為 0、
  腳本全綠（既有 flaky `abort_await_is_bounded`、`visual-check.js` DF1 收尾若失敗，重跑一次並記錄）

## 2. `cockpit-core`

- [x] 2.1 退回操作（spec `pipeline-domain`「進度操作」）：`ProgressOp::Retreat`、`Rejection` 新增「已是第一個 Stage」；`apply_op` 依 spec
  處理；驗收＝單元測試涵蓋「退回到上一站」「第一站不能退回」「有標記時不能退回」與既有推進測試仍綠
- [x] 2.2 目前 task（spec `pipeline-domain`「目前 task」、design D1）：`DomainState` 新增 `active`；純函數設定目前 task（不屬於該
  workstream、已有標記 → 新的拒絕原因）；`Complete`／`Fail` 清除；覆蓋設定、取消、失效移除時清除（找出現有覆蓋變更在 core 的路徑並在
  同處清除）；驗收＝單元測試涵蓋該 requirement 全部 scenario，外加「推進、退回、清除標記不影響目前 task」
- [x] 2.3 推導與投影（spec `pipeline-domain`「StageStatus 推導」、`state-projection`「Project 投影」、design D2）：`derive_status` 加
  `is_active`；投影 workstream 加 `active_task`、`activity_undeclared`；驗收＝單元測試涵蓋「StageStatus 推導」全部 scenario（含「沒有
  目前 task 時不猜」「同一 workstream 多個 Task」）與「Project 投影」的 Scenario C JSON、「工作中但未宣告」；`content_eq` 對新欄位敏感
  （兩份只差 `active_task` 的投影不相等）

## 3. `cockpit` 後端

- [x] 3.1 狀態檔 v2（spec `pipeline-progress`「狀態檔格式與持久化」「狀態檔載入與容錯」、design D5）：`cockpit/src/progress.rs`；驗收＝
  `cockpit/tests/progress_file.rs` 涵蓋「讀取 v1 舊檔」「不支援的版本」「無效的目前 task」（含不屬於、不存在、已有標記三種）、v1 帶
  `active` 被拒、v2 缺 `active` 被拒、v2 寫出再讀回相同、「重啟後保留」含目前 task
- [x] 3.2 寫入交易（design D3）：`ProgressService` 新增「宣告」與「agent 推進」交易，沿用單鎖、先落檔再生效、相同不落檔、取消安全；
  覆蓋設定／取消／`remove_stale` 經 2.2 的純函數清除目前 task；驗收＝`cockpit/tests/progress_service.rs` 涵蓋：agent 推進同時改 stage 與
  目前 task 且只寫一次檔、推進被拒時目前 task 不變、重複宣告同一 task 不寫檔且 version 不遞增、寫檔失敗兩者皆不變、覆蓋三種變更清除目前 task
- [x] 3.3 人工退回端點（spec `pipeline-progress`「進度寫入端點」）：`parse_progress_op` 接受 `retreat`；驗收＝`cockpit/tests/pipeline_api.rs`
  涵蓋「退回成功」「第一站退回被拒絕」「人工端點沒有宣告操作」
- [x] 3.4 agent 端點（spec `agent-reporting` 全部、design D4、D6）：`GET /api/agent/tasks`、`POST /api/agent/projects/{p}/tasks/{t}/{op}`，
  套 `source_check`；以最新投影＋設定檔的 WSL runtime 清單判定 pane 身分；判定順序依 D6；驗收＝`cockpit/tests/` 新檔 `agent_api.rs`
  涵蓋 `agent-reporting` 每個 scenario，外加「打錯 project／task 回 404 而非 403」「同一 pane 綁兩條 workstream 時兩條都列出」
- [x] 3.5 Codex 審查（後端）：跑 `codex-companion.mjs adversarial-review --base main`（前景、`--wait`），focus 為 core＋後端；依
  `superpowers:receiving-code-review` 實測驗證後才改；findings 與處理寫進 ledger；確認輸出有最後的「# Codex Adversarial Review」結論段

## 4. 前端與預覽

- [x] 4.1 `ui_preview` 情境：為既有情境中原本呈 `running`／`blocked` 的 task 設定目前 task，使既有畫面不變；另加一條 `activity_undeclared`
  為 `true` 的 workstream，以及至少一個 `mark` 為 `none` 且不在第一站、可按「退回」的 task；驗收＝`cargo test -p cockpit --example ui_preview`
  全綠、既有腳本全綠（若有斷言因新增的 workstream／task 而失敗，只能依通則調整）
- [x] 4.2 退回按鈕（spec `cockpit-dashboard`「畫面操作」）：`actions.js` 的 `TASK_OPS`、`render.js` 的 `renderTaskActions`；新驗收腳本
  `docs/research/2026-10-01/progress-check.js`（寫法比照 `actions-check.js`，附 `progress-check.md` 用法）涵蓋「退回按鈕」scenario
  （顯示條件三種、送出的請求、新投影後的位置）與「停在新狀態再整頁重畫」；驗收＝新腳本與既有腳本全綠
- [x] 4.3 未宣告提示（spec `cockpit-dashboard`「Factory Floor」、design D7）：列首提示；`progress-check.js` 加「未宣告 task 的提示」
  scenario（含重畫後仍正確、無動畫）；控制端做設計審核（1536／1100／700 三寬 viewport 截圖，對照 `docs/direction-01-visual-design.md`）
  並過 frontend-design 審核；驗收＝新腳本與既有腳本全綠、審核 findings 已處理
- [x] 4.4 Codex 審查（前端）：同 3.5，focus 為 `cockpit/assets/`、`ui_preview.rs` 與腳本

## 5. 文件、真機驗收與收尾

- [x] 5.1 文件（design D8、Migration Plan）：`cockpit/README.md` 新增「agent 回報進度」一節（端點、標頭、錯誤碼、PowerShell `curl.exe`
  與 bash 範例、可貼進 `AGENTS.md` 的短文）與狀態檔 v2／回退說明；`CONTEXT.md` 新增「目前 task」「退回」詞條並更新 Task、StageStatus
  的描述；驗收＝`markdownlint-cli2` 0 issues，README 範例在 5.2 實際執行過
- [x] 5.2 真機驗收：以臨時設定檔（放在 repo 外）啟動 Cockpit，一條 workstream 綁到 Windows 端 HERDR 的一個真實 pane；在該 pane 內依 README
  範例用 curl 跑 `GET /api/agent/tasks`、`start`、`advance`，以及用錯 pane id 的 403；確認畫面上只有目前 task 呈 `running`、未宣告時有提示；
  結果記錄到 `docs/research/2026-10-01/agent-report-live.md`；對 HERDR 只讀
- [x] 5.3 全 gate 與整支分支 Codex 審查：`cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test --workspace &&
  cargo test -p cockpit --example ui_preview && markdownlint-cli2 "**/*.md" && openspec validate --all`、既有腳本＋`progress-check.js` 全綠；
  `adversarial-review --base main` 結論段無未處理 finding；數字寫進 ledger
