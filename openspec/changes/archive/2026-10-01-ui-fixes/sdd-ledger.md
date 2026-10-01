# SDD ledger：ui-fixes

- plan：`openspec/changes/ui-fixes/tasks.md`；spec：同目錄 `specs/`、`design.md`
- 分支：`feat/ui-fixes`，起點 `0ec278e`（propose commit）
- 工作區（git-ignored）：`.superpowers/sdd/tasks-ui-fixes/`（briefs、reports、`mkbrief.sh`）

## 前置裁決

- Ruling: ledger 放在 `openspec/changes/ui-fixes/sdd-ledger.md`（隨 change archive），不放 SDD 工作區的 `progress.md` — tasks.md 1.1／3.6
  指定此路徑，且前幾個 change 都如此 — 若錯：只是位置不同，無功能影響。
- Ruling: per-task 不派 Claude subagent 審 diff，改為控制端核對實作者的 red→green 與 gate 輸出，diff 審查依 tasks.md 3.7／4.9／5.3
  由 Codex 分階段跑（2026-10-04 10:55 後）— 全域 CLAUDE.md「diff 審查一律 Codex」優先於 SDD skill 的 task reviewer — 若錯：單一 task
  的問題要到階段審查才被發現，返工成本較高。
- Ruling: SDD skill 的 `scripts/task-brief` 只認 `### Task N`，改用工作區 `mkbrief.sh` 從 checkbox 格式抽 task 原文＋通則 — 若錯：brief 漏字，
  實作者回頭讀 tasks.md 即可補。
- Ruling: 3.3 與 3.5 都只改 `cockpit/src/agent.rs`／`cockpit/tests/agent_api.rs`，合併成一次派工 — 同檔小改、分兩次只會重建脈絡 — 若錯：
  一次 diff 稍大，對分階段 Codex 審查無影響。
- Ruling: 等 Codex 期間（3.7、4.9）先跳過、續做後續 task，Codex 恢復後依序補審（tasks.md 開頭已載明）— 若錯：補審 findings 可能推翻後續
  task，需返工。

## 前置衝突掃描

| 對象 | 產出 → 使用 | 結果 |
|---|---|---|
| 2.1 ↔ 4.1 | `projected-state.json` 補 `source`（2.1）→ `ui_preview` 讀同一份 fixture、4.1 新增覆蓋斷線情境 | 2.1 必須先讓 fixture 可反序列化，4.1 再加情境；順序正確 |
| 2.1 ↔ 4.4 | 投影 `runtime_disconnected.source` → `render.js` 徽章與「取消改綁」 | 4.4 讀 2.1 的欄位名 `source`，值 `auto`／`override` |
| 3.1 ↔ 4.6 | refs 回應 `commit` 布林 → `git.js` `computeExpectedTips` | 4.6 依賴 3.1；順序正確 |
| 3.1 ↔ 3.2 | 同在 `cockpit-git`：3.1 動 `query.rs`／`refs.rs`，3.2 動 `runner.rs` | 檔案不重疊；`query.rs` 的 argv 斷言只在 3.1 改 |
| 3.3 ↔ 3.5 | 同改 `agent_api.rs` | 合併派工（見裁決） |
| 3.4 | `progress.rs` 型別改動 → `progress_service.rs` 寫出處 | task 文字已列兩檔，一致 |
| 4.2 ↔ 4.3／4.4 | 4.2 建 `ui-fixes-check.js` → 4.3／4.4 擴充 | 擴充不重寫 |
| 4.2 ↔ 4.8 | 4.2 的焦點 helper（`output.js` 全域）→ Git 詳情焦點還原 | 4.8 呼叫 4.2 的 helper，不另寫 |
| 4.5／4.6／4.7／4.8 | 同改 `git.js`、同擴充 `git-check.js` | 依序做，不並行 |
| 1.1 | 基線數字 → 各 task 比對 | 一致 |
| 2.1 自身 | 驗收列出受影響測試檔 ↔ 要改的型別 | 一致 |
| 3.1 自身 | 測試（tree tag）↔ 改 `query.rs`、`refs.rs`、`git.rs` | 一致 |
| 3.2 自身 | 測試③要改呼叫端行程環境 ↔ 獨立整合測試檔 | 一致（D5 已說明） |
| 3.3 自身 | spec 寫入被拒 scenario ↔ 整合層造不出孤兒 | task 允許以單元測試證明判定為空，一致 |
| 4.7 自身 | 截斷提示斷言 ↔ fixture 能否產生截斷 | 未定：上限可能很大，交實作者查上限後選最便宜的造法（小上限測試 repo 或後端可設定上限僅限測試）；若需改產品上限的可設定性，回報後由控制端裁決 |
| 4.3 自身 | 「三寬截圖＋frontend-design 審核」由控制端做 ↔ 實作者交付腳本與截圖 | 一致 |

## 進度

## 基線（task 1.1）

- 日期：2026-10-01；HEAD：`97f7d61`（尚無產品程式碼改動）
- `cargo test --workspace`：1083 passed／0 failed／12 ignored（62 個 test binary 加總）
- `cargo test -p cockpit --example ui_preview`：39 passed／0 failed／0 ignored
- 既有腳本 10 支（依序、前景、不並行）：全部 PASS 一次通過（`visual-check.js` DF1 收尾未失敗；`factory-floor-check.js` 跑完已還原截圖）；
  收尾 7770 與 CDP port 無 LISTEN、`git status` 乾淨。
- Task 1.1: complete (no commit；report 在工作區 `task-1.1-report.md`)
- Task 2.1: complete (commits ff954e0..5c15531；RED＝編譯失敗 E0559，GREEN＝workspace 全綠、ui_preview 39)
- Task 3.1: 觀察：本機 git 2.50.1 下 tips 含 tree oid 時 `/log` 實測仍回 200，5b 延後清單「整個 Graph 失敗」在本機未重現（WSL 端 git 版本未測）。
  排除非 commit 起點仍保留：前端分支變更偵測（4.6）需要與後端一致，且對任何 git 版本都無害。端到端測試以 `tips` 不含該 oid 為判準。
- Task 3.1: `/log?ref=<tree tag>` 明確選取維持現行（design Non-Goals 已載明），未處理。
- Task 3.1: complete (commits 185aa98..3643525；RED＝E0609 與暫拿掉過濾的端到端失敗，GREEN＝cockpit-git／cockpit 全綠、git-check PASS)
- Task 3.2: complete (commits 5348a76..a11a049；RED＝②env 斷言、③查到另一 repo 的 head=other；①為防護測試、一開始即綠)
- Task 3.3＋3.5: complete (commits cdde86f..293b894；RED＝孤兒單元測試在舊判定回非空；斷線最後已知 pane 為回歸測試、一開始即綠；
  寫入被拒以單元測試證明判定為空（`agent_op` 共用同一判定）；測試改名為 `start_from_old_pane_after_rebind_leaves_no_active_task`)
- Task 3.4: complete (commits 372f894..6bb5d71；RED＝v1 null 載入成功 27/1，GREEN＝progress_file 28 passed；v1 `{}`、v2 null 為回歸)
- Task 3.6: 後端完成時 `cargo test --workspace`（HEAD `6bb5d71`）：1098 passed／0 failed／12 ignored（63 個 test binary）
- Task 3.6: complete
- Task 3.7: 延後至 Codex 恢復（2026-10-04 10:55 後），審查範圍 `0ec278e..6bb5d71`（第 2、3 節）
- Task 4.1: complete (commits 12163f2..a4d42cd；ui_preview 新增 workstream `ovr`（cockpit project 末尾、無 task），RED＝E0425，GREEN＝ui_preview 40、10 支腳本全 PASS)
- Task 4.2: complete (commits 7994a7b..62669d2；RED＝ui-fixes-check FAIL(7)，GREEN＝連 3 次 PASS、10 支既有腳本 PASS；helper `window.cockpitFocus.focus(el)` 在 output.js)
- Task 4.2: minor (deferred): `factory-floor-check.js` 首跑一次 FAIL（左欄 Project 項目為 0，6 項），立即重跑 PASS，疑既有載入時序 flake，未重現；後續 task 再觀察。
- Task 4.3: 既有腳本調整（spec 已改變的前置條件，斷言不變）：`factory-floor-check.js` harness 未載 channel.js，補 `onChannel('connected')`；
  `visual-check.js` CL1 死規則清查補「通道 disconnected」取樣。見 commit b7b0ec1。
- Ruling: 左欄 project 的「警告 N」（`.project-item-warnings`）斷線時維持 `--warn` — 它是設定／綁定的結構警告、不是即時狀態，spec 清單未列 —
  若錯：斷線時該色仍醒目，使用者可能誤以為警告是即時的；補一條 spec 與一個選擇器即可。
- Task 4.3: 設計審核（控制端，三寬截圖＋frontend-design）：通過、無 findings。斷線時唯一保留的強烈色是底列「disconnected」，成為焦點；
  running 節點靠 2px 外框與 ▶ 仍可辨識；按鈕對比不變、不像停用；`--text-dim` 對各底色對比 7.67–9.04:1。
- Task 4.3: complete (commits b02e997..b1c6c7c；RED＝ui-fixes-check FAIL(107)，GREEN＝278 ok、10 支既有腳本 PASS)
- Task 4.4: complete (commits 4e5f263..70944a7；RED＝FAIL(3) 徽章斷言，GREEN＝331 ok、10 支既有腳本 PASS；「取消改綁」因投影帶 source 已自動出現，未改該處)
- Ruling: 4.5 與 4.6 合併派工 — 同改 `git.js`／`git-check.js`、各為小改，分開會多跑一輪約 150 秒的 git-check 與全部腳本 — 若錯：一次 diff 稍大。
- Task 4.5＋4.6: complete (commits d63fa44..4d50b45；RED＝scrollTop 4971→1040、2068 ms 出現分支已變更，GREEN＝git-check 23 段與全部腳本 PASS；篩選分支路徑的 tips 刻意不過濾 commit，與後端篩選路徑一致)
- Task 4.7: complete (commits 974ec5f..4576fdc；RED＝FAIL(4)，GREEN＝11 支腳本 PASS；截斷以 fast-import 造 2640 個長路徑檔超過 4 MiB 上限，每段約 19 秒，未改產品上限)
- Task 4.8: complete (commits f45dfef..f22f029；RED＝鍵盤 3／滑鼠 2 項焦點落 body，GREEN＝11 支腳本 PASS；`data-focus-key` 身分，找不到時聚焦 `.commit-detail` 容器)
- Task 4.8: minor (deferred): 「找不到對應元素→聚焦容器」後備路徑無專屬驗收段（spec 兩個 scenario 都走找得到的路徑），只讀碼確認。
- Task 4.9: 延後至 Codex 恢復，審查範圍 `6bb5d71..f22f029`（第 4 節）
- Task 5.1: complete (commit b65d50c；真機 56 項斷言全過。未能真機驗證：第二批載入（本 repo 僅 74 commit）、running／blocked 外框與未宣告提示（pane 全 idle），
  由 `git-check.js`／`ui-fixes-check.js` 在 fixture 上覆蓋；去識別化經控制端 grep 與看圖確認)
- Task 5.1: minor (deferred): 真機暫存腳本有一次點「Git Graph」按鈕後未開分頁（加重點機制後未再發生），原因未查；可能是真機偶發點擊線索。
- Task 5.2: 全 gate（HEAD `7922119`）：fmt／clippy 通過；workspace 1098 passed／0 failed／12 ignored；ui_preview 40；markdownlint 133 files 0 issues；
  `openspec validate --all` 20 passed；11 支腳本全 PASS（`live-output-check.js` 首跑 FAIL(1)、單獨重跑 PASS，見下）；收尾乾淨。
- Task 5.2: minor (deferred): `live-output-check.js` R 段「wJ:p1 的 pane 列可以取得焦點」偶發 FAIL——腳本以兩次分開的 `eval` 先 `focus()` 再比
  `activeElement === __oldNode`，中間若有一次推送重畫（100 ms）節點即被換掉；屬腳本既有競態，非產品問題、非本 change 引入。修法：同一次 eval 內 focus 並比對。
- Task 5.2: complete
- Task 5.3、5.4: 待 Codex 恢復（2026-10-04 10:55 後）依序跑 3.7 → 4.9 → 5.3，再 5.4 收尾

## 審查（Opus 5.5 取代 Codex）

- 使用者決定（2026-10-01）：本 change 的 Codex 審查（3.7、4.9、5.3）全部由 Opus 5.5 subagent 代替，跑完視同取代 Codex。僅限本 change。
- 做法：3.7（core＋後端）與 4.9（前端）兩個唯讀審查並行；findings 經控制端實測驗證後，一次派 fresh 實作者修正＋scoped 複審；最後 5.3 整支分支審查。
- 3.7 Opus 審查（`opus-review-3.7.md`）：needs-fix，Critical 0／Important 1／Minor 4。
- Ruling: I1（git 2.43 的 `%(*objecttype)` 只剝一層，巢狀附註 tag 被判非 commit，經由它才到得了的 commit 從預設 Graph 靜默消失；WSL 2.43 與 Windows 2.50.1
  皆實測）→ `commit` 判定改為「剝開後確定為 tree 或 blob 才為 false，其餘為 true」，同步改 spec 的 `commit` 欄位說明、design D4 錯誤前提
  （「實測剝到底」只在新版 git 成立）、`refs.rs` 註解、`real_git.rs` 巢狀 tag 斷言改為不依賴 git 版本 — 兩版 `git log` 對 tree 起點都直接忽略（審查實測），
  舊 git 下多收一個無害；前端讀同一欄位，前後端仍一致；不另加一次 git 呼叫 — 若錯：舊 git 上「巢狀 tag→tree」被當成起點，`git log` 若在某版本改為報錯會讓 Graph 失敗。
- Ruling: 3.7 M1（繼承環境變數測試未驗 index 修改時間與目標 repo）、M2（「覆蓋失效退回自動、自動 runtime 斷線」路徑的 `source` 無測試）納入修正波 —
  spec 要求的項目、改動小 — 若錯：多幾個測試。
- 3.7 minor (deferred): M3 `StateProject` derive `Default` 時 `active` 可能寫出 null（現行唯一寫出處正確）；M4 測試字串字面值含真換行（只影響可讀性）。
- 4.9 Opus 審查（`opus-review-4.9.md`）：needs-fix，Critical 0／Important 1／Minor 5。
- Ruling: 4.9 I1（keydown 不分修飾鍵，滑鼠點擊後按 Alt／Ctrl 下次重畫外框又出現；headless 實測）→ 比照 Chrome 原生判定，帶 Alt／Ctrl／Meta 的 keydown 不改
  輸入方式，Shift 照算 — 若錯：極少數以 Ctrl 組合鍵移動焦點的情境外框不出現。
- Ruling: 4.9 M1（鍵盤外框在不移動焦點的滑鼠操作後於重畫時消失）、M3（git-check.md 註明回歸守門）、M4（刪除無呼叫者的 `lastInput()`）納入修正波 — 小改 — 若錯：多幾行。
- 4.9 minor (deferred): M2 `data-focus-key` 不分詳情種類、分支與 tag 同短名時 copy 按鈕身分可能相撞（讀碼推導未實測）；M5 詳情容器 `tabIndex=-1` 使滑鼠點空白處焦點落在容器。
- 修正波 1：findings 與裁決清單在工作區 `fix-wave-1-findings.md`，修正前 HEAD `f943dfd`。
- 修正波 1：commits dc40b15..5de97d4（B-I1、B-M1、B-M2、F-I1、F-M1、F-M3、F-M4）；workspace 1100 passed／0 failed、11 支腳本 PASS；report `fix-wave-1-report.md`
- 修正波 1 複審（`opus-rereview-1.md`）：7 項全 ADDRESSED。新問題：Important——F-M1 行為與 spec `cockpit-dashboard` 焦點條文、design D1 字面矛盾（缺「滑鼠操作未移動焦點時原本有外框的照常呈現」例外）；
  Minor——Git 詳情鍵盤聚焦後再滑鼠點同一顆，重建後保留外框（與原生一致），與 `git-review` 滑鼠 scenario 字面有出入。範圍外：`cockpit/src/git.rs:631-632` 註解仍寫「git log 整個失敗」；
  舊 git 巢狀 tag 的 `oid` 為內層 tag 物件（既有行為），`git.js` 以 oid 比對處認不出。待 5.3 結果後併入修正波 2。
- 5.3 Opus 整支分支審查（`opus-review-5.3.md`）：needs-fix，Critical 0／Important 2／Minor 4；程式無新缺陷；ledger 全部 Ruling／deferred 維持延後。
  I1 修正波 1 的兩條焦點規則（修飾鍵、不移動焦點的滑鼠操作）未寫回 spec／design；I2 `ui-fixes-disconnected-700.png`、`-1536.png` 帶出真實使用者名稱
  （ui_preview fixture cwd 用真實 %TEMP%；`main` 上 change 6 的 `progress-700.png` 同樣問題）——控制端先前只 grep 文字與看真機截圖，漏看 ui_preview 截圖。
- Ruling: 修正波 2 處理 5.3 I1、I2、M1（`git.rs` 註解與 proposal Why 的「tree 起點讓 git log 失敗」不實）、M2（proposal Impact 檔案清單），連同複審的 Important（同 I1）與 Minor
  （Git 詳情鍵盤聚焦後滑鼠點同顆保留外框，寫成 spec 例外）；`progress-700.png` 一併重拍遮罩 — 同一機制、成本低 — 若錯：多改一張圖。
- Ruling: 5.3 M3（Git 詳情 F-M1 路徑無驗收段）延後；M4（真機紀錄帶出其他私人 repo 名稱）交使用者決定。
- Ruling: tasks 3.7、4.9 由 Opus 審查取代並完成（使用者決定），勾選。
- 修正波 2：commits 79eb13e..457f2c4（W2-I1、W2-I2〔六張圖重拍遮罩，控制端看過 `ui-fixes-disconnected-700.png` 乾淨〕、W2-M1、W2-M2）；report `fix-wave-2-report.md`
- 修正波 2 複審（`opus-rereview-2.md`）：4 項全 ADDRESSED、無新 Critical／Important，可併回。Minor：git-review 例外句放在 scenario 之後（archive 後會成為 scenario 內文）、
  design D4 替代方案仍預設「git log 會失敗」——控制端直接改兩處文字（純文件、幾行，不另派工）。其餘 Nit（scenario 措辭「按住／1 秒」比腳本寬鬆、腳本註解 `<host>`、
  proposal Impact 漏列幾個測試檔）延後。
- Task 5.3: complete（Opus 整支分支審查＋兩波修正＋兩次複審，取代 Codex）
