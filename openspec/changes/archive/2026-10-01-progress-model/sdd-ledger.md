# SDD ledger：progress-model

- plan：`openspec/changes/progress-model/tasks.md`；spec：同目錄 `specs/`、`design.md`
- 分支：`feat/progress-model`，起點 `f616308`（propose commit 之後）
- 工作區（git-ignored）：`.superpowers/sdd/tasks-progress-model/`（briefs、reports）

## 前置裁決

- Ruling: per-task 不派 Claude subagent 審 diff，改為控制端核對實作者的 red→green 與 gate 輸出，diff 審查依 tasks.md 3.5／4.4／5.3
  由 Codex 分階段跑 — 全域 CLAUDE.md「diff 審查一律 Codex」優先於 SDD skill 的 task reviewer；分階段是為了節省 Codex 週限額 —
  若錯：單一 task 的問題要到階段審查才被發現，修正成本較高。
- Ruling: 「目前 task」在 Complete／Fail 時的清除放在 DomainState 層級的純函數，不塞進只看 `TaskProgress` 的 `apply_op` — 規則需要
  workstream 資訊 — 若錯：2.2 與 3.2 的介面要再調一次。

## 前置衝突掃描

| 對象 | 產出 → 使用 | 結果 |
|---|---|---|
| 2.1 ↔ 2.2 | `progress.rs` 的 `apply_op`／`Rejection` | 2.2 的清除改在 state 層（見裁決），不衝突 |
| 2.2 ↔ 3.1 | `DomainState.active` → 狀態檔讀寫 | 3.1 依 2.2 的型別 |
| 2.2 ↔ 3.2 | core 清除函數 → service 的覆蓋三路徑 | 3.2 必須呼叫 2.2 的函數，不另寫規則 |
| 2.3 ↔ 3.4 | 投影 `active_task` → agent GET | 3.4 讀投影 |
| 2.3 ↔ 4.1 | 推導改變 → `ui_preview` 既有 running 消失 | 4.1 補目前 task；4.1 之前不跑前端腳本以外的斷言調整 |
| 4.2 ↔ 4.3 | 共用 `progress-check.js` | 4.3 擴充，不重寫 |
| 每個 task 自身 | 驗收條件與要動的檔案一致 | 一致；3.4 新增 `agent_api.rs` 為新檔 |

## 進度

## 基線（task 1.1）

- 日期：2026-10-01；HEAD：`53fc9f7`（分支 `feat/progress-model`，尚無產品程式碼改動）
- `cargo test --workspace`：1004 passed／0 failed／12 ignored（各 test binary 的 `test result` 加總）
- `cargo test -p cockpit --example ui_preview`：38 passed／0 failed／0 ignored
- 既有腳本（依序、不並行；跑前 7770 無 LISTEN，每支跑完 7770 與 CDP port 無殘留 LISTEN）：

| 腳本 | 結果 |
|---|---|
| `2026-09-15/reconnect-check.js` | PASS |
| `2026-09-15/whatever-check.js` | PASS |
| `2026-09-16/actions-check.js` | PASS |
| `2026-09-16/channel-backoff-check.js` | PASS |
| `2026-09-16/factory-floor-check.js` | PASS（跑完已 `git checkout --` 還原截圖） |
| `2026-09-19/live-output-check.js` | PASS |
| `2026-09-23/visual-check.js` | PASS（DF1 收尾未失敗） |
| `2026-09-27/files-check.js` | 第一次 FAIL（`file-review/中文 PDF` 觀察不到子行程 exit 事件，其後各段 preview 起不來，連鎖 FAIL）；重跑一次 PASS |
| `2026-09-28/git-check.js` | PASS |

- files-check 第一次 FAIL 的疑似原因：該次是在前一批長時間迴圈尾端執行、與背景任務逾時被中止交疊；單獨重跑即綠。屬環境／flaky，非產品問題。
- Task 1.1: complete (commit 597c9b1；files-check 首跑 FAIL、單獨重跑 PASS，記為環境交疊)
- Ruling: `remove_override` 只在真的移除一筆存在的覆蓋時才清目前 task，no-op 取消不動狀態 — spec「覆蓋被取消時清除」不涵蓋不存在的覆蓋，no-op 不應落檔 — 若錯：使用者對沒有覆蓋的 workstream 按取消改綁時，目前 task 不會被清（該按鈕本來就只在有覆蓋時出現）。
- Task 2.1: complete (commits df78d6d..2183ae3)
- Task 2.2: fix round 1/5 (1 addressed, 0 open — no-op 取消不清 active; commits f2ef057..af45a62)
- Task 2.2: complete (commits 2183ae3..af45a62)。給 3.2 的呼叫點清單在工作區 `task-2.1-2.2-report.md`。
- Ruling: `ProjectedWorkstream` 的 `active_task`／`activity_undeclared` 加 `#[serde(default)]`，只影響反序列化（測試 fixture 的舊 JSON），序列化一律輸出 — 免得為了 fixture 大改既有 JSON — 若錯：fixture 缺欄位不會被抓到，但產品端只序列化、不受影響。
- Ruling: `cockpit/tests/pipeline_api.rs` 的 `scenario_c_pipeline_projection`、`scenario_d_parallel_collaboration` 在 2.3 後因「沒有目前 task → ready」失敗（spec 已改變），交 3.4 以 agent `start` 端點在 GIVEN 宣告目前 task 後修正 — 宣告入口在 3.4 才存在 — 若錯：3.1–3.3 期間 workspace 有 2 個已知失敗。
- Task 2.3: complete (commits b41e227..600a951；workspace 1025 passed／2 known failed（見上）／12 ignored)
- Task 3.1: minor (deferred): v1 檔手寫 `"active": null` 會被當成缺欄位而放行（`Option` 無法區分 null 與缺）— 手改檔極端情況。
- Task 3.1: complete (commits 3c7474d..4aa6b90；`tests/app.rs` 的 version 斷言 1→2，spec「系統寫出的狀態檔一律為 version 2」)
- Task 3.2: complete (commits 4aa6b90..26e8df3；service API `declare_active`、`agent_advance`)
- Ruling: agent 路徑的 `<op>` 不是 `start`／`advance` 時先回 404、早於標頭檢查 — spec 寫「一律 404」，D6 的順序指的是合法 op 的判定 — 若錯：沒帶標頭打 `complete` 拿到 404 而非 400，對使用者無實害。
- Ruling: WSL 判定沿用 `AppState::path_mappings`（`HerdrEndpoint::Wsl` → `PathMapping::Wsl`），查不到的 runtime 視為非 WSL — 不新增狀態、不碰 ADR-0003 — 若錯：以自訂 `command` 端點橋接進 WSL 的 runtime 會被當成 Windows 端，WSL 內 agent 本來就連不到 Cockpit，實際影響僅限理論上的撞號判定。
- Task 3.3: complete (commits 33e7820..83ec007)
- Task 3.4: complete (commits 83ec007..63b18bf；workspace 1073 passed／0 failed／12 ignored，scenario C／D 改以 agent start 宣告)
- Task 3.5: blocked — Codex 回 usage limit（恢復時間 2026-10-04 10:55，輸出存工作區 `codex-3.5.md`）。Ruling: 繼續做 4.x、5.1、5.2，三次 Codex 審查延到額度恢復後跑；審查通過前不併回 main、不宣稱完成，也不以 Claude subagent 取代 — 全域規則「diff 審查一律 Codex」，使用者 5b 的豁免不延伸 — 若錯：後端問題要到前端做完才被發現，修正可能牽動前端。
- Ruling: `actions-check.js` 的「p 的 Completed 按鈕數」3→4、「合計連按 task 數」10→11 接受 — 是 fixture 新增 mark=none 的 task 造成的計數同步，斷言強度不變（不是放寬）；通則字面只允許 spec 已改變的斷言，此處依精神裁量 — 若錯：腳本修改應改以「新增情境放在不影響計數的位置」避免，但 factory-floor 斷言 project 數恰為 2，無處可放。
- Task 4.1: complete (commits 751cded..f9eb1fc；ui_preview 39 passed，9 支腳本全綠；新情境 `p/undeclared`（activity_undeclared）與 `p/undeclared-1`（Implement、可退回）)
- Task 4.2／4.3: 腳本調整 — `actions-check.js` 按鈕集合加 retreat（spec「畫面操作」新增退回）；`visual-check.js` CL1 在防禦狀態把 `cockpit/docs` 設 activity_undeclared=true 讓新選擇器 `.ff-undeclared` 有對象（spec「Factory Floor」新增提示），斷言未放寬。
- Task 4.3: 控制端設計審核（1536／700 截圖，對照 direction-01 與 frontend-design）：無 finding。提示文字為 spec 原文、`--warn` token、無動畫；節點按鈕換行屬既有版面。
- Task 4.2: complete (commits 2c4fd72..8800410)
- Task 4.3: complete (commits 8800410..0e9817b；progress-check 與 9 支既有腳本全綠)
- Task 4.4: blocked — 同 3.5，Codex 額度恢復後跑。
- Task 5.1: complete (commit 5c56cd1)
- Task 5.2: complete (commit 92d6b0b)。缺口：真機期間 `wW:p1` 的 agent_status 恆為 idle，未在真機觀察到「只有目前 task 呈 running」與 `activity_undeclared=true`，這兩項由 `agent_api.rs`、`pipeline_api.rs` scenario C／D 與 `progress-check.js` 覆蓋。
- Task 5.3: gate 部分通過（2026-10-01，HEAD 92d6b0b 之後只改文件）：fmt、clippy 乾淨；workspace 1073 passed／0 failed／12 ignored；ui_preview 39；markdownlint 120 files 0 issues；openspec 19 passed；腳本（progress-check＋9 支）最後一次全綠在 0e9817b，其後產品程式碼無改動。Codex 部分 blocked（同 3.5）。

## 整支分支審查（Opus 5.5，取代 Codex）

- 使用者 2026-10-01 決定：Codex 額度恢復前，暫以 Opus 5.5 subagent 取代 Codex 審查本 change（僅限 change 6）。報告存工作區 `opus-review.md`。
- 結果：0 Critical、0 Important、6 Minor（M1 改綁競態 CONFIRMED、M2 撞號只看綁定 PLAUSIBLE、M3 載入丟覆蓋不清 active、M4 重設同覆蓋清 active、M5 未知 project 缺 active 啟動失敗、M6 註解過期）。
- Ruling: M1–M4、M6 修，M1–M4 先改 spec／design（agent-reporting「pane 身分判定」、pipeline-domain「目前 task」、pipeline-progress「狀態檔載入與容錯」、design D4）— 都是小改動且方向更保守 — 若錯：多一輪修正成本。
- Ruling: M5 不修 — 手改 v2 檔缺必填欄位即屬損毀，啟動失敗並指出路徑是既有對損毀檔的處理 — 若錯：手改檔的人要補 `"active": {}`。
- 修正：commits d6b8dc2..1e20d7a（M4 9538cab、M3 4517d10、M1+M2 98c9bbf、M6 1e20d7a）；workspace 1083 passed／0 failed／12 ignored；progress-check、actions-check PASS。
- 複審（Opus 5.5，報告 `opus-rereview.md`）：M1、M2、M3、M4、M6 皆 ADDRESSED；無 Critical／Important。
- Ruling: N1（撞號只數投影 pane 樹，綁到孤兒 pane 時漏判，PLAUSIBLE）延後到 change 7 — 只修一輪的規則；store 註明孤兒 pane 理論上不發生 — 若錯：兩個 Windows runtime 同時出現孤兒 pane 撞號時，agent 可能被認成另一條 workstream。
- Ruling: N2（`stale_pane_start_right_after_rebind_is_403` 在投影追上後也會通過，分不出鎖內路徑）延後 — 鎖內重驗已由 service 層 basis 測試確定性覆蓋 — 若錯：只影響該測試的說明力。
- Task 3.5／4.4／5.3 的 Codex 審查：依使用者決定以上述 Opus 整支分支審查＋複審取代，視為完成。
- 合併前驗收：10 支腳本（progress-check、actions-check＋其餘 8 支）於修正後 HEAD 全 PASS；gate 1083 passed／0 failed／12 ignored。
