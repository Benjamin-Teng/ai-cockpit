# SDD ledger — plan: openspec/changes/openspec-stage-sync/tasks.md

Ruling: ledger 放在 change 目錄（`sdd-ledger.md`，隨分支提交），brief 與 report 放 SDD workspace
`.superpowers/sdd/tasks-openspec-stage-sync/` — 沿用 change `repo-projects` 的先例，讓 review 與 archive 看得到紀錄 —
若錯只是紀錄位置不同，無功能影響。

Ruling: 每個 task 的 task review 由 Opus 5.5 subagent 執行（專案 memory `opus-review-counts-as-codex-review`）；
4.7 與 7.2 另以 Codex `adversarial-review` 為主、Opus 為退路 — 全域 CLAUDE.md 要求 diff 審查走 Codex，
專案 memory 允許 Opus 視同 — 若錯，逐 task 審查少了一層 Codex 視角，由 4.7／7.2 兩次 Codex 補上。

Spec：`openspec/changes/openspec-stage-sync/specs/**`（binding），design：`design.md`（D1～D10）。

## Pre-flight 衝突掃描

| 任務對 | 產出 → 消費 | 結果 |
|---|---|---|
| 3.1 ↔ 3.2 | `RepoProjectDef.phases` → 自動移動找對應 stage、修改對應時重套 | 一致；3.2 依賴 3.1 |
| 3.1 ↔ 4.1 | `phases` → 狀態檔 v4 序列化與 v1–3 補對應 | 一致 |
| 3.2 ↔ 4.1 | `TaskSync` → 狀態檔 task 的 `sync` | 一致 |
| 3.2 ↔ 4.3 | `apply_openspec`、手動標記 helper → 寫入服務入口 | 一致；`openspec_obs` 整表替換（D10-6）在 4.3 的 `sync_openspec` 做 |
| 3.3 ↔ 4.6 ↔ 5.1／5.2 | `stage_phases`、`task.sync` → fixture、卡片標示、對話框 | 一致 |
| 2.1 ↔ 4.4 | `CurrentBranch` → 背景工作 | 一致 |
| 4.2 ↔ 4.4 | 純偵測函式 → 背景工作 | 一致 |
| 4.3 ↔ 4.5 | service 接受 `phases`／`phase` → HTTP 本體 | 一致；錯誤碼依 D10-1 |
| 4.5 ↔ 5.2 | 請求本體形狀 → 前端送出 | 一致 |
| 4.4 ↔ 既有 `RepoResolver` | `PaneRepo` 加 worktree 根目錄 | 4.4 會改 `repo_resolver.rs`，需保持既有測試綠 |

各 task 自身一致性：1.1～7.4 逐一檢查，測試清單與指定實作相符，沒有自相矛盾。掃描無衝突。

## Artifacts 交叉驗證的裁決（開工前）

fresh subagent 交叉驗證 artifacts，找出 14 項問題，裁決如下並回寫 design／specs／tasks：

- Ruling: 同步狀態一律伴隨進度項目，缺就補初始進度；有 TaskSync 的 task 不算 untouched — 存檔與清除只看
  `repo_progress` 一份鍵，最單純 — 若錯，只會多寫一筆初始進度。
- Ruling: 推進與標記手動放在同一個 write 閉包 — 避免鎖外標記選錯 task，也避免同步插在中間 — 無明顯代價。
- Ruling: 不新增錯誤代碼，沿用 `invalid_stages`／`invalid_body` — 與 D10-1 一致 — 錯誤訊息較籠統。
- Ruling: 「對應是否改變」以 phase 為主體，經 `from` 對應比較；改名或重排不觸發 — 符合直覺 — 若錯，改名後少一次重套。
- Ruling: `PaneRepo.root` 保留大小寫的主機路徑，WSL 經 `wsl_host_path` — 讀檔需要真實路徑 — 無。
- Ruling: archive 同 slug 多個日期時取最大者 — 最近一次 archive 最可能是現行的 — 罕見情境。
- Ruling: `CurrentBranch` 用 `symbolic-ref -q HEAD` 並自行去掉 `refs/heads/` — 避開 `--short` 遇同名 tag 的歧義 — 無。
- Ruling: v3 檔帶 `phases`／`sync`、手寫 project 的 task 帶 `sync`，一律視為損毀 — 不猜意圖 — 手改檔案的人會啟動失敗。
- Ruling: 7.3 驗 WSL 不被開機，只在使用者的 WSL 本來就沒在跑時驗；不得為了驗證關掉使用者的 WSL — 不干擾使用者 —
  可能留下未驗項目。

## 1. 基線

task 1.1 記錄，給後續 task 當回歸基準。

- 日期：2026-10-10
- HEAD：`f53aa7f`（`feat/openspec-stage-sync`，跑前工作樹乾淨）
- 工具鏈：跑前 `cargo build -p cockpit --example ui_preview` 與 `cargo build -p cockpit --examples` 皆 exit 0
- `cargo test --workspace`：exit 0；75 個測試套件合計 **1518 passed／0 failed／17 ignored**
- `cargo test -p cockpit --example ui_preview`：exit 0；**82 passed／0 failed／0 ignored**
- 環境：跑腳本前 `netstat -ano` 確認 7770～7999、19000 一帶與 9222 沒有監聽；唯一相關的監聽是 `127.0.0.1:7778`
  （ASUS 服務，與本專案無關，未動）。跑完後再查一次，仍只有 7778。
- 偶發項目：本輪 16 支腳本與 cargo test 全部一次通過，沒有任何偶發失敗，沒有重跑。

### 既有腳本清單

出處：`openspec/changes/archive/2026-10-08-repo-projects/sdd-ledger.md` 第 1 節的 15 支，加
`docs/research/2026-10-08/repo-projects-check.js`，共 16 支（已與 `docs/handover.md` 第 1 節「可用指令」核對）。
`idle-exit-check.js` 測啟動器，不列入。一律前景、一次一支、repo 根目錄 `node <路徑>`；`reconnect-check.js` 需先
`cargo build -p cockpit --examples`。

| 腳本 | 結果 | 備註 |
| --- | --- | --- |
| `docs/research/2026-09-15/reconnect-check.js` | PASS（exit 0） | 14 ok |
| `docs/research/2026-09-15/whatever-check.js` | PASS（exit 0） | 9 ok |
| `docs/research/2026-09-16/actions-check.js` | PASS（exit 0） | 92 ok |
| `docs/research/2026-09-16/channel-backoff-check.js` | PASS（exit 0） | 19 ok |
| `docs/research/2026-09-16/factory-floor-check.js` | PASS（exit 0） | 131 ok；會覆寫 `task-5.2-scenario-d.png` |
| `docs/research/2026-09-19/live-output-check.js` | PASS（exit 0） | 345 ok |
| `docs/research/2026-09-23/visual-check.js` | PASS（exit 0） | 1784 ok；既有段落／FT1–FT3／收尾 FAIL 皆 0 |
| `docs/research/2026-09-27/files-check.js` | PASS（exit 0） | 607 ok；自我測試 2/2、scenario 0/28 FAIL、收尾衛生 PASS |
| `docs/research/2026-09-28/git-check.js` | PASS（exit 0） | 770 ok；收尾衛生 PASS |
| `docs/research/2026-10-01/progress-check.js` | PASS（exit 0） | 64 ok |
| `docs/research/2026-10-01/ui-fixes-check.js` | PASS（exit 0） | 349 ok |
| `docs/research/2026-10-02/output-color-check.js` | PASS（exit 0） | 89 PASS |
| `docs/research/2026-10-02/notify-check.js` | PASS（exit 0） | 新行為 191／前置 62 斷言，FAIL 0 |
| `docs/research/2026-10-03/i18n-check.js` | PASS（exit 0） | 新行為 948／前置 512 斷言，FAIL 0；重拍 `i18n-en-*.png` 15 張 |
| `docs/research/2026-10-04/split-check.js` | PASS（exit 0） | 段落 48/48 PASS、收尾衛生 PASS |
| `docs/research/2026-10-08/repo-projects-check.js` | PASS（exit 0） | 段落 37/37 PASS、收尾衛生 PASS |

腳本跑完後工作樹有 16 張被重拍的截圖（`docs/research/2026-09-16/task-5.2-scenario-d.png` 與
`docs/research/2026-10-03/i18n-en-*.png` 15 張），內容不屬於本 task，本次提交不含。還原指令：
`git checkout -- docs/research/2026-09-16 docs/research/2026-10-03`（本 task 執行者的 auto mode 擋下該指令，留給控制端處理）。

## 進度

- Ruling: task 1.1 只有紀錄、沒有程式 diff，不另派 task reviewer，由控制端核對 ledger 的數字與腳本表 — 沒有可審的程式碼 —
  若錯，基線數字有誤會在 7.1 對帳時現形。
- Ruling: 腳本會重拍既有截圖，跑完後由控制端以 `git restore` 還原被重拍的 PNG — 截圖內容不屬本 change —
  若錯，只是 repo 內舊截圖未更新。
- Task 1.1: complete (commits f53aa7f..d4d7917, ledger-only)
- Task 2.1 查證：git-scm.com/docs/git-symbolic-ref（2026-10-10 WebFetch）。`-q` 原文「Do not issue an error message if the `<name>` is not a symbolic ref but a detached HEAD; instead exit with non-zero status silently.」；NOTES 節「exit 0 if the contents of the symbolic ref were printed correctly, 1 if the requested name is not a symbolic ref, or 128 if another error occurs.」；官方文件沒有明寫 detached 的結束碼，是由「非符號參照 → 1」推得，已以 git 2.50.1 實測 detached 為 exit 1、stderr 空（real_git 測試 `current_branch_on_detached_head_is_none_not_an_error`）。`--short` 文件只說「try to shorten」，故不用。實作另規定：exit 1 但 stderr 非空視為錯誤（`wsl.exe` 自己失敗也以 1 結束）。WSL 版 real_git 測試（`#[ignore]`）未跑，WSL 未啟動。
- Task 2.1: minor (deferred): `CurrentBranch::stdout_cap` 用 1 MiB 預設值，D7 寫「小上限」
- Task 2.1: minor (deferred): WSL 的 `wsl.exe` 若每次都往 stderr 印警告，detached 會恆為錯誤；4.4 記錯誤日誌時要去重或降級
- Task 2.1: minor (deferred): stdout 多行時沒判 `MalformedOutput`
- Task 2.1: minor (deferred): WSL real_git 測試沒 touch 已追蹤檔、沒比 index mtime
- Task 2.1: minor (deferred): `current_branch.rs` 模組文件夾空行
- Task 2.1: WSL 版 real_git 未跑（WSL 未啟動），改由 7.3 補驗
- Task 2.1: complete (commits d4d7917..37ab072, review clean, 5 minor deferred)
- Task 3.1: carry → 3.2：D10-2 判準已實作為 `StageRemap::phases_changed`，3.2 只接線；呼叫時傳覆寫前的 `def.stages`／`def.phases`
  （`progress_service.rs` 目前先覆寫）；可加 `debug_assert` 長度對齊
- Task 3.1: carry → 4.1：載入狀態檔時呼叫 `repo_project_phases_valid`，不合法視為損毀，不可靜默截斷
- Task 3.1: carry → 4.5：移除 `http.rs` 寫死的 `phase: None`；`phase` 以字串接收後用 `OpenSpecPhase::parse`，未知值回 `invalid_stages`
- Task 3.1: minor (deferred → 4.3)：服務層測試補 `StageEdit { phase: Some(..) }` 寫入 domain 的斷言
- Task 3.1: complete (commits 5b5f3eb..ee7dbb7, review clean)
- Task 3.2: Ruling: 「下一輪」語意由服務層全表重套實現：4.3 在 core 補純函式 `reapply_openspec_all`（走訪所有展開的 Repo Project
  task，以 `openspec_obs` 呼叫 `apply_openspec`，規則 3 保證冪等），在 `sync_openspec`（整表）、`clear` 成功的同一閉包、
  `reset_auto_applied` 之後、`set_pane_repos`／project 展開改變之後呼叫；4.4 送出端維持去重 — 背景工作內容不變不送時，
  「清除標記後恢復自動」「改對應後 Auto 卡片移動」兩個 scenario 不會發生 — 若錯，多一次無效走訪，無寫檔。
- Task 3.2: carry → 4.3：`add_repo_project` 清同 id 殘留進度時一併 `clear_repo_project_sync`；`update_repo_project` 覆寫前保留舊
  stages／phases 給 `phases_changed`；端到端測試補「偵測不變時清除標記卡片會移動」「偵測不變時 PATCH 改對應 Auto 卡片會移動」；
  complete／fail／clear 不改 mode 要在服務層測。
- Task 3.2: carry → 4.4：送出的 pane 清單必須取自 domain 投影（不可用 `RepoResolver` 自己較新的 `pane_repos`）。
- Task 3.2: minor (deferred): `state.rs` 兩段 `retain` 清除邏輯重複，可抽泛型小函式
- Task 3.2: minor (deferred): commit trailer 實作者用 Sonnet 5.5，與守則的 Opus 5.5 不同；不改寫歷史
- Task 3.2: complete (commits ee7dbb7..499b07a, review clean)
- Task 3.3: carry → 4.3：`openspec_obs` 在 pane 對不上 change 時必須移除該 key（投影以「沒有這筆」判定 null）
- Task 3.3: minor (deferred): `projection.rs:193` fallback 只比長度，宜改比 `stages` 內容；fallback 分支無測試
- Task 3.3: complete (commits 3407ef9..fb1e17c, review clean)
- Task 4.1: carry → 4.3／4.5：`repo_project_service.rs` 的 `add_creates_project_with_tasks_for_each_pane_and_persists` 釘了
  `"phases": [null,null,null,null]`，加入帶入對應後要改（spec 已改變，非放寬）
- Task 4.1: carry → 4.3：寫 `repo_sync` 的服務路徑要守 R1（sync 必有 progress），加 debug_assert 或測試
- Task 4.1: minor (deferred): 載入不驗 `checked <= total`、`change` 非空
- Task 4.1: minor (deferred): 載入時 stage 不存在被重設為第一站的 task 仍保留舊 `applied`，下一輪可能不移動（罕見）
- Task 4.1: complete (commits fb1e17c..e81927d, review clean)
- Task 4.2: Ruling: archive 目錄只在對應第 2 步需要時讀；第 1 步命中或 detached 時 archive 讀取錯誤不影響結果 — 此時 archive 內容
  不影響結果，無法判斷的前提不存在 — 若錯，archive 損壞時少一次「無法判斷」。spec scenario 字面未限定，7.2 前補註。
- Task 4.2: carry → 4.4：分支查詢出錯時不可呼叫 `detect`（不可把錯誤折成 None 分支當 detached），直接送 None；
  WSL 路徑呼叫 `detect` 前必須先過 `RunningDistros`。
- Task 4.2: minor (deferred): 分支以 `/` 結尾的路徑在 git 不合法，碰不到，保留無害
- Task 4.2: fix round 1/5 (4 addressed, 0 open — checkbox 限制過嚴、archive 惰性讀取測試、is_dir 吞錯、BOM; commits 0dfe849..19c8e46)
- Task 4.2: minor (deferred): `dangling_symlink_entry_is_skipped_not_an_error` 在 Windows 建不出連結時提早 return，形同空測試
- Task 4.2: complete (commits b7d382e..19c8e46, review clean after 1 fix round)
- Task 4.3: Ruling: `add_creates_project_with_tasks_for_each_pane_and_persists` 不改 — 它省略 phases，全 null 正是 spec「加入成功」
  scenario 的值；帶入對應另有測試 — 若錯，只是少一個斷言。
- Task 4.3: carry → 6.1／7.2 前：pipeline-progress「寫出 Repo Project」scenario 字面「推進到 Build、沒有 sync」與手動入口會建立
  sync 衝突，需改 spec 措辭。
- Task 4.3: carry → 4.4：`sync_openspec` 寫檔失敗時記憶體不變，去重不可把該輪記成已送出；送出的表必須涵蓋所有 Repo Project pane；
  4.4 加入 `PaneRepo.root` 後，`set_pane_repos` 的歸類改變判斷要一併比較 `root`。
- Task 4.3: Ruling（→ 4.4）：`sync_openspec` 每筆附上偵測當下的歸類（repo、worktree、root），鎖內與 `domain.pane_repos` 比對，
  不符就當無結果 — 背景工作偵測與送出之間 pane 可能改歸類，否則會把舊 worktree 的結果套到新卡片 — 若錯，只多一次比對。
  4.4 負責改簽章與測試。
- Task 4.3: Ruling 修訂（補 Task 3.2 Ruling）：`set_pane_repos` 重套前先丟棄歸類改變的 pane 的偵測結果，`openspec_obs` 每筆恆屬於
  該 pane 目前的歸類 — 否則舊 repo 的結果會套到新 repo 的卡片並落檔 — 若錯，新歸類的 pane 要等下一輪偵測才套用。
- Task 4.3: carry → 4.4：`PaneRepo` 加 `root` 後，`set_pane_repos` 的過濾要比較整個歸類（去掉 `default_name`），不可只比 repo、worktree。
- Task 4.3: minor (deferred): pane 改歸類後，舊 project 下該 task 的 `repo_sync` 與進度保留到 pane 消失（既有行為，規則 3 保證不誤移）
- Task 4.3: fix round 1/5 (4 addressed, 0 open — 舊歸類偵測結果被套到新卡片、doc、agent_advance_for_pane 被拒測試、序列測試;
  commits 0677eb4..aa274ff)
- Task 4.3: complete (commits ea38099..aa274ff, review clean after 1 fix round)
- Task 4.4: Ruling（修訂 Task 3.2 Ruling 的「送出端維持去重」）：背景工作的去重依據改為 domain 現有 `openspec_obs`，
  不再用自己的 `last_sent` — 歸類 A→B→A 會讓 domain 丟掉結果而 `last_sent` 不變，造成靜默且不自癒的遺失 — 若錯，多送幾次
  無變化的表，寫入服務會 no-op。
- Task 4.4: Ruling: 同一 worktree 混合已連線與未連線 runtime 的 pane 時，照常查詢，未連線的 pane 沿用上一輪 — WSL 防護看路徑，
  不會因此開機；照字面做會讓已連線的 pane 被不相干的斷線拖住 — 若錯，未連線 runtime 的 pane 多得到一次結果。
- Task 4.4: carry → 6.1（spec 措辭修正，與 4.3 的一起）：openspec-stage-sync spec「未連線 runtime 不查詢」補多 runtime 共用
  worktree 的情形；「不呼叫 `wsl.exe`」改為「不執行會啟動發行版的指令」（`wsl.exe --list --running` 探測是防護本身）。
- Task 4.4: fix round 1/5 (5 addressed, 2 open — toplevel 未 canonicalize（CI 8.3 短名）、去重改依 domain 後殘留表外結果被重新加入時套用;
  commits 90337a4..29862b3)
- Task 4.4: Ruling: 寫入服務維持不變式「`openspec_obs` 只含目前展開的 Repo Project task 的 pane」，在改變展開的寫入路徑重套前 retain —
  從源頭消除殘留，比背景工作另比表外 key 單純 — 若錯，被隱藏後再顯示的 pane 要等下一輪偵測才有結果。
- Task 4.4: fix round 2/5 (3 addressed, 0 open — toplevel canonicalize、openspec_obs 只含展開中 pane、RootFailures 不建空表項;
  commits 29862b3..7490a69)
- Task 4.4: minor (deferred): `hidden_repo_project_panes_have_no_observation` 後半段無鑑別力
- Task 4.4: minor (deferred): `real_detection_leaves_repo_untouched` 依賴 current_thread runtime，且等滿 10 秒拉長測試時間
- Task 4.4: complete (commits 73cb11d..7490a69, review clean after 2 fix rounds)
- Task 4.5: Ruling: `"phases": null` 等同省略 — 與 `name: null`、`from: null` 慣例一致 — spec 嚴格讀可能判 `invalid_body`，6.1 補一句。
- Task 4.5: Ruling: 階段字串的轉換移進服務端 transaction，順序 pid → name → stages → phases — 維持「先判定 pid 再驗內容」契約、
  正式端點與 ui_preview 一致 — 無。
- Task 4.5: fix round 1/5 (3 addressed, 0 open — 檢查順序、null phases、doc; commits dcf9529..0ecba66)
- Task 4.5: complete (commits f2f4a9e..0ecba66, review clean after 1 fix round)

## 4.7 後端 Codex 審查

`codex-companion adversarial-review --base main --scope branch`，結論段 `# Codex Adversarial Review`：Verdict needs-attention，2 項。

- Codex [high] UNC 讀檔無逾時，卡住時同步停擺、程式無法結束 — 查證成立：`cockpit/src/main.rs` 用 `#[tokio::main]`，runtime drop
  會無限期等 `spawn_blocking`；`app.rs` 的 shutdown 註解也承認。
  Ruling: 偵測工作每個 root 的讀檔包 `tokio::time::timeout`（逾時該 root 本輪 None），同一 root 前一次 blocking 讀檔未返回前不再派新的；
  `main.rs` 改手動建 runtime 並以 `shutdown_timeout` 收尾 — 讓卡住的 9P 只影響該 repo，且不拖住關機 — 若錯，卡住的執行緒在背景洩漏到返回為止。
- Codex [medium] WSL running 探測與 `wsl.exe -d` 查詢之間有 TOCTOU，可能啟動剛停止的發行版 — 查證成立（邏輯上）；RepoResolver 有同性質、
  時間窗更長的既有風險；經 `\wsl.localhost` 讀檔同樣會喚醒，換機制無法根除。
  Ruling: 每個 WSL root 在 git 查詢與讀檔前一刻重新探測一次，把時間窗縮到毫秒級；殘餘風險寫進 design Risks — 完全消除需要不經
  `wsl.exe -d` 與 UNC 的機制，超出本 change — 若錯，使用者剛停掉 WSL 的那一瞬間偵測可能把它開回來。
- Task 4.6: Ruling: manual 的 fixture 卡片留在 Plan（brief 未指定站）— 避免動到既有腳本的卡片位置假設 — 該卡片無法展示「手動與對應站不一致」，
  5.x 視需要另加一張。
- Task 4.6: carry → 5.1：`repo-projects-check.js` 收尾殘留 PID 檢查 4 次中 3 次 FAIL（段落全 PASS；基線 1 次 PASS）。審查判定本 diff 不會
  產生子程序；需在基線上多跑幾次對照，確認是否為 PID 重用誤報（memory `harness-pid-checks-misreport-on-windows`）。
- Task 4.6: minor (deferred): `repo_project_scenario_sync_strings_are_neutral` 名不符實；手動卡片註解不精確；測試小重複
- Task 4.6: complete (commits 462ac47..e0cf030, review clean)
- 4.7 修正第 1 輪（commits e0cf030..d74e325）：Codex 複審判定 F1、F2 已依裁決落實；新增 1 項 medium：逐 root 探測讓 `wsl.exe --list` 故障線性放大。
  Ruling: 探測結果區分成功與逾時／失敗，單輪內一旦失敗就斷路，其餘 WSL root 當跳過 — 避免 N×5 秒停擺 — 若錯，探測偶發失敗時本輪
  WSL root 少查一次。
- 4.7 修正第 2 輪（commits d74e325..5acc904）：Opus 複審 ADDRESSED，無新問題（僅 debug 日誌措辭 minor）。
- Task 4.7: complete（Codex 2 項 + 複審 1 項均已處理）
- 調查（Task 4.6 carry）：`repo-projects-check.js` 收尾殘留 PID FAIL 為 PID 重用誤報 — HEAD 2/6、main 1/5 FAIL；殘留 PID 14144 經
  Get-CimInstance 為晚 16 秒建立的 svchost.exe，原 preview 早已確認終止。37 段落每次全 PASS。
  Ruling: 5.1 的新腳本以「PID＋建立時間」判定殘留；同時修正 `repo-projects-check.js` 的 `pidStillRunning` 判定（比對建立時間，
  或確認終止時即自 OUR_PIDS 移除），腳本修改另開 commit — 這是讓判定正確而非放寬 — 若錯，真殘留可能漏報（建立時間相同才算殘留，
  不會漏）。
- Task 5.1: Ruling: 同一 PID 重用誤報修正也套到 `split-check.js`（實測 2 次誤報，殘留 PID 為使用者的 msedge.exe、建立時間晚於 spawn）—
  機制相同、讓既有腳本全綠 — 若錯，只影響收尾衛生判定。
- Task 5.1: carry → 5.2：`stage-sync-check.js` 的 `killTree` 剛 taskkill 就判定（第 2 種誤報）、建立時間查不到時空洞通過、`ariaLabel` 死欄位；
  `render.js`、`style.css` 結構註解過時（四段）
- Task 5.1: complete (commits 02fe58d..698a2dd, review clean)
- Task 5.2: minor (deferred): `phasesOf` 把未知階段字串轉 null，後端新增階段時前端會靜默改掉對應；`PHASES` 旁宜註明與後端列舉同步
- Task 5.2: minor (deferred): `repo-projects-check.js` 注入 4 stage 但 `stage_phases` 只有 2 個，走補 null 容錯路徑
- Task 5.2: minor (deferred): 版面段落沒有先紅；segDialogKeyboard 第二次 Esc 未包 check
- Task 5.2: minor (deferred): `split-check.js` 收尾「殘留 headless Chrome」偶發誤報（同類時序機制），候選改短輪詢
- Task 5.2: complete (commits 0bd6532..697747f, review clean)

## 5.3 設計審核

frontend-design 審核（Opus subagent 載入 skill），截圖在本 session 的 scratchpad 目錄
（`floor-{1536,1100,700}.png`、`dialog-{1536,1100,700}.png`、`*-zoom.png`、`long-floor-*.png`），未進 repo。

- Important：對話框看不出下拉是 OpenSpec 階段。Ruling: 補 `actions.dialog.stages.hint` 一句說明，不加欄位標題 — 最少版面變動 — 若錯，
  說明文字被忽略時仍可能誤解。
- Minor：下拉與名稱輸入差 2px、「不對應」與已選一樣亮、change 名稱無行數上限 — 一併修。
- 交使用者決定：手動標示是否加非顏色記號（例如虛線框或符號），會改 design D9「一行文字」。
- Task 5.3: carry → 6.1：hint「右側下拉」「The dropdown on the right」在窄寬時不準（下拉會換行），改為不含方位的說法，連同 `HINT_TEXT` 斷言
- Task 5.3: minor (deferred): `segCardLongChange` 未斷言 progress 與 mode 仍在卡片內；CSS 註解寫死「約 9:1」
- Task 5.3: complete (commits 6e0700c..0a34f47, review clean)
- Task 6.1: 文件內容驗證（Claude subagent，非 diff 審查）通過，4 處措辭修正於 df59d24，控制端核對修正項與指示一致。
- Task 6.1: complete (commits 2faeb5f..df59d24, review clean)

## 7.1 收尾驗證

task 7.1 記錄。日期：2026-10-11；HEAD：`ce4d259`（`feat/openspec-stage-sync`，跑前工作樹乾淨）。

| 指令 | 結束碼 | 摘要 |
| --- | --- | --- |
| `cargo fmt --check` | 0 | 無輸出 |
| `cargo clippy --all-targets -- -D warnings` | 0 | 0 warning |
| `cargo test --workspace` | 0 | 80 個測試套件合計 **1736 passed／0 failed／18 ignored**（基線 1518／0／17） |
| `cargo test -p cockpit --example ui_preview` | 0 | **87 passed／0 failed／0 ignored**（基線 82） |
| `markdownlint-cli2 "**/*.md"` | 0 | Linting: 238 files、0 issues |
| `openspec validate --all` | 0 | 26 passed／0 failed（只有 RFC 2119 的 WARNING，非錯誤） |
| `cargo build -p cockpit --example ui_preview`、`cargo build -p cockpit --examples` | 0、0 | — |
| `node docs/research/2026-10-02/deid-check.js` | **1** | 掃 2074 個檔案；命中 U 類 1 處，在本檔第 218 行（scratchpad 路徑含 Windows 使用者名稱），見下方「去識別化」 |

環境：跑腳本前後 `netstat -ano` 查 7700～7999、19000、9222，唯一監聽是 `127.0.0.1:7778`（ASUS 服務，未動）。
腳本一律前景、一次一支。第一輪被 shell 的 10 分鐘 timeout 中斷在 `files-check` 前（`visual-check` 已 PASS），
`files-check` 起的 10 支用長 timeout 重跑，非腳本失敗；中斷時沒有殘留程序或 port。

### 腳本結果

| 腳本 | 結果 | 備註 |
| --- | --- | --- |
| `docs/research/2026-09-15/reconnect-check.js` | PASS（exit 0） | 14 ok |
| `docs/research/2026-09-15/whatever-check.js` | PASS（exit 0） | 9 ok |
| `docs/research/2026-09-16/actions-check.js` | PASS（exit 0） | 92 ok |
| `docs/research/2026-09-16/channel-backoff-check.js` | PASS（exit 0） | 19 ok |
| `docs/research/2026-09-16/factory-floor-check.js` | PASS（exit 0） | 131 ok |
| `docs/research/2026-09-19/live-output-check.js` | PASS（exit 0） | 345 ok |
| `docs/research/2026-09-23/visual-check.js` | PASS（exit 0） | 1786 ok；既有段落／FT1–FT3／收尾 FAIL 皆 0 |
| `docs/research/2026-09-27/files-check.js` | PASS（exit 0） | 607 ok；自我測試 2/2、scenario 0/28 FAIL、收尾衛生 PASS |
| `docs/research/2026-09-28/git-check.js` | PASS（exit 0） | 770 ok；收尾衛生 PASS |
| `docs/research/2026-10-01/progress-check.js` | PASS（exit 0） | 64 ok |
| `docs/research/2026-10-01/ui-fixes-check.js` | PASS（exit 0） | 349 ok |
| `docs/research/2026-10-02/output-color-check.js` | PASS（exit 0） | 89 PASS／0 FAIL |
| `docs/research/2026-10-02/notify-check.js` | PASS（exit 0） | 新行為 191／前置 62 斷言，FAIL 0 |
| `docs/research/2026-10-03/i18n-check.js` | PASS（exit 0） | 新行為 948／前置 512 斷言，FAIL 0 |
| `docs/research/2026-10-04/split-check.js` | PASS（exit 0） | 段落 48/48 PASS、收尾衛生 PASS |
| `docs/research/2026-10-08/repo-projects-check.js` | PASS（exit 0） | 段落 37/37 PASS、收尾衛生 PASS |
| `docs/research/2026-10-10/stage-sync-check.js` | PASS（exit 0） | 492 ok；段落 15/15 PASS、收尾衛生 PASS |

17 支全綠，與基線 16 支逐支對得上，沒有偶發失敗。

### 去識別化

`deid-check.js` exit 1：唯一命中是本檔既有的第 218 行（task 1.1 之後寫入 ledger 的 scratchpad 路徑，含 Windows
使用者名稱）。本 task 未改動該行，也沒有新增命中。需控制端決定是否把該行改成不含使用者名稱的寫法。

### 工作樹

腳本重拍了 16 張既有截圖（`docs/research/2026-09-16/task-5.2-scenario-d.png`、
`docs/research/2026-10-03/i18n-en-*.png` 15 張），已 `git restore` 還原，`git status` 乾淨。

## 7.2 整支分支 Codex 審查

`adversarial-review --base main --scope branch`，Verdict needs-attention，2 項，控制端查證皆成立：

- [medium] WSL 讀檔前沒有重新探測（只在 git 前探測），與 4.7 裁決「git 查詢與讀檔前一刻」不符。
  Ruling: `current_branch` 成功後、讀檔前再探測一次，未執行或失敗回 Skipped；補「git 查詢期間停止發行版不得讀檔」測試 — 落實既有裁決 — 無。
- [medium] 載入時 stage 不存在被重設為第一站，仍保留 auto 的 `applied`，偵測不變時永久停錯站（即 Task 4.1 deferred minor）。
  Ruling: 載入重設 stage 且 mode 為 auto 時，`applied` 清為 None；manual 保留；補「重啟後偵測不變仍修正 auto 卡片」測試 — 消除不自癒狀態 — 無。

## 7.3 真機冒煙

結果見 `docs/research/2026-10-10/stage-sync-live.md`（commit b48d49e）：對上 change、勾選數一致、全勾移到審查、手動退回不被拉回、
取消一項恢復自動、還原後回到原狀，全部 PASS；Cockpit 執行 160 秒期間 WSL 發行版維持 Stopped。
未驗：WSL 路徑 pane 的防護（沒有 cwd 在 `\wsl.localhost` 的 pane）、分支對不上的後備、archive 後 complete（由腳本與 Rust 測試涵蓋）。
副作用：之後跑的 `deid-check.js` 以 `wsl.exe -d` 取使用者名稱，把 Ubuntu-24.04 開機（非 Cockpit 造成）；已寫專案 memory。

- Task 7.3: complete (commit b48d49e)

## 7.2 修正後 gate（HEAD b48d49e）

- `cargo fmt --check`：exit 0
- `cargo clippy --all-targets -- -D warnings`：exit 0，0 warning
- `cargo test --workspace`：exit 0，passed 1740／failed 0／ignored 18
- `cargo test -p cockpit --example ui_preview`：exit 0，87 passed／0 failed
- `markdownlint-cli2 "**/*.md"`：exit 0，Linting: 239 files，0 error（先修了本檔 7.3 節 MD032 缺空行）
- `openspec validate --all`：exit 0，26 passed／0 failed（僅 SHALL／MUST 英文規格慣例的 WARNING）
- `node docs/research/2026-10-10/stage-sync-check.js`：exit 0，15/15 PASS，收尾衛生 PASS
- `node docs/research/2026-10-08/repo-projects-check.js`：exit 0，37/37 PASS，收尾衛生 PASS

## 7.2 最終整支審查（Opus，code-reviewer 範本）

結論：產品程式無必修缺陷；兩項 Important 屬流程，已處理：7.3 交付物（b48d49e）、7.2 修正後 gate（fa1f650，全綠）。
修正 a74f554、ed8d978 經最終審查逐行讀過，邏輯與測試正確。

- Task 4.1 deferred minor「載入重設 stage 保留舊 applied」：已於 ed8d978 解決。
- Ruling: 下列 minor 留待之後，寫入 handover：暫時偵測失敗送 None，使手動入口記下 `applied=None` 而在下一輪被拉回；`set_pane_repos` 在狀態檔
  持續寫不進時整個不生效；WSL worktree 每 10 秒 3 支 `wsl.exe`；首次偵測即把狀態檔升為 v4（降版只能刪檔，可考慮先備份 v3）；tasks.md
  寫一半被讀到 — 都不在常見路徑、修法會牽動 spec — 若錯，使用者會在少見情境看到卡片多移動一次。
- Task 7.2: complete（Codex 2 項已修；最終審查 Ready with fixes，fixes 已完成）

- Task 7.4: complete (commit bbe070b)
