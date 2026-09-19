# SDD ledger — plan: openspec/changes/pipeline-projection/tasks.md

Spec authority: openspec/changes/pipeline-projection/specs/*/spec.md（8 份）；design：同目錄 design.md（D1–D11）；
設計文件：docs/superpowers/specs/2026-09-13-cockpit-mvp-design.md。
Branch: feat/pipeline-projection（artifacts commit 181ee8c）。Started: 2026-09-16。

## Setup rulings

- Ruling: 沿用 1b：在既有分支主 checkout 實作、不另開 worktree — 分支非 main、target/ 與相對路徑不變 — 代價：使用者同時動 main 會混。
- Ruling: task reviewer 一律 Codex adversarial-review（`--base <BASE>`，背景 job），不派 Claude subagent；findings 實測後才採信 — 代價：Codex 靜態推導可能誤判。
- Ruling: brief 由控制端腳本產生（task 原文＋spec/design 指標），`scripts/task-brief` 不認 `- [ ] X.Y` — 代價：指標可能漏，brief 附 tasks.md 路徑可對照。
- Ruling: 實作者先 commit 再審，fix round 追加 commit；收尾 squash — 同 1b。
- Ruling: 實作者 model：預設 sonnet；2.5（投影＋StoreHandle 併發）、3.3（寫入服務序列化）、5.3（前端互動）、6.1（驅動器）、6.4（shutdown）用 opus；fix round 4–5 升級。
- Ruling: 研究／驗收目錄一律 `docs/research/2026-09-16/`，驗收文件 `pipeline-projection-acceptance.md` — tasks 寫「<日期>」未定 — 代價：無。

## Pre-flight scan（2026-09-16）

| 對 / 單一 task | 產出 vs 消費 | 結果 |
|---|---|---|
| 1.1 ↔ 2.x | CONTEXT.md 詞彙先定，程式命名跟隨 | 一致 |
| 2.1 ↔ 2.2/2.3/2.4 | 2.1 型別；2.2 apply_op、2.3 resolve_binding/validate_override、2.4 derive_status 消費 | 一致；2.1 要一併定義 `Rejection`、op 列舉（apply_op 簽章需要）→ 帶進 2.1 dispatch |
| 2.3 ↔ 2.4 | derive_status 吃 BindingResolution | 一致 |
| 2.4 ↔ 2.5 | 投影呼叫 resolve_binding＋derive_status，收集 stale_overrides | 一致 |
| 2.5 ↔ 3.3 | 2.5 StoreHandle domain 寫入方法與 stale mpsc；3.3 寫入服務消費 | 一致；順序 2.5 先 |
| 2.5 ↔ 1b fixture | projected-state.json 加 projects | 2.5 自帶 |
| 3.1 ↔ 3.2 ↔ 3.4 | 3.1 產 ProjectDef 與狀態檔路徑；3.2 載入；3.4 組裝 | 一致 |
| 3.3 ↔ 4.1 | 4.1 AppState 持寫入服務 | 一致 |
| 4.1 ↔ 5.3 | 4.1 要 `GET /app/actions.js`（include_str!），檔案由 5.3 建 | **衝突**→ Ruling 見下 |
| 5.2 ↔ 5.3 | 5.2 `renderState(state, ui)`，ui 由 5.3 actions.js 提供 | 缺口→ Ruling 見下 |
| 3.4 ↔ 6.4 | 3.4 在 shutdown 收寫入服務；6.4 改 shutdown_components 期限 | 一致（循序，6.4 保留 3.4 的收尾） |
| 6.1/6.2 | 同一 driver.rs、driver 測試 | 一致（循序） |
| 6.5 ↔ 6.1–6.4 | 測試改 paused time 可能碰 6.x 動過的檔 | 一致（6.5 最後） |
| 7.2 | 會對 WSL 測試 server 建 pane／report_agent、重啟 server | 已授權範圍（handover、AGENTS.md 允許 WSL 端） |
| 7.3 | 使用者親自做 | 非自動化：做到時請使用者 |
| 各 task 自洽 | 測試名 vs 程式碼、建立 vs 後續觸碰的檔案 | 一致（除上列） |
| 審查準則衝突 | 強制無斷言測試或重複邏輯 | 無 |

- Ruling: 4.1 先建最小 `cockpit/assets/actions.js`（空模組＋註解）以便路由與 content-type 測試，5.3 填入正式內容 — tasks 順序與依賴相反 — 代價：無。
- Ruling: 5.2 的 `renderState(state, ui)` 把 `ui` 視為選填（缺省＝無改綁、無錯誤），5.3 再接上 actions.js — 代價：無。

## Task log

### Task 1.1

- dispatched implementer (sonnet) BASE=181ee8c
- DONE, commit 9d4b7e0；markdownlint 0（57 files）。Codex review 派出（--base 181ee8c，輸出 codex-1.1.txt）。
- Ruling: 前一 task 審查期間即派下一 task 的實作者（檔案不重疊時）— 省牆鐘時間 — 代價：fix round 與新 task 同時 commit 可能撞 index.lock，重試即可。

### Task 2.1

- dispatched implementer (sonnet) BASE=9d4b7e0
- (1.1) Codex review：needs-attention，1 high：Override 失效條件漏 connected 前提（斷線應保留）；對照 runtime-binding spec 屬實 → fix round 1 resume 實作者。
- Task 1.1: fix round 1/5 (1 addressed, 0 open — Override 斷線保留; commits 9d4b7e0..87b14f3)；Codex re-review approve。
- Task 1.1: complete (commits 181ee8c..87b14f3, review clean)
- (2.1) DONE, commit 10ba90f；domain_types 7/7、cockpit-core 61 passed；fmt/clippy 0。concern：BindingSource 不 derive Serialize（對外 JSON 留給 2.5）。Codex review 派出（--base 87b14f3）。
- Ruling: 型別型 task 審查期間不平行派同模組的下一 task — 2.2 直接建在 2.1 型別上，fix round 改型別會撞 — 代價：多等一輪 Codex。
- Task 2.1: complete (commits 87b14f3..10ba90f, review clean)
- Ruling: 2.2（apply_op）與 2.3（resolve_binding/validate_override）平行派——函數分屬不同檔、只共用 2.1 已審定型別；兩者不改 tasks.md 勾選（控制端統一勾）、commit 只 add 自己的檔 — 代價：mod.rs 同時改可能要重試合併。

### Task 2.2 / 2.3

- dispatched 2.2 implementer (sonnet)、2.3 implementer (sonnet) BASE=10ba90f
- (2.2) DONE, commit 259840e；domain_progress 10/10、crate 全過；fmt 順帶格式化 2.3 進行中的 binding.rs（未 add）。Codex review 派出。
- (2.2) Codex：needs-attention，1 medium：agent_status_never_changes_progress 沒真的套用狀態事件（無效測試）→ 視為 Important，fix round 1 resume（改用真 RuntimeStore 套事件）。
- (2.3) DONE, commit 61a390b；cockpit-core 89 passed（domain_binding 18）；resolve_binding 回 (BindingResolution, bool stale)。Codex review 派出。
- (2.3) Codex：needs-attention，2 medium（測試缺口）：斷線測試無殘留 pane；候選篩選未測 agent 與大小寫 → fix round 1 resume。
- Task 2.2: fix round 1/5 (1 addressed, 0 open — 真 RuntimeStore 套事件; commits 61a390b..2c005fc)；Codex approve。
- Task 2.2: complete (commits 10ba90f..2c005fc, review clean)

### Task 2.4

- dispatched implementer (sonnet) BASE=2c005fc（2.3 fix round 只改測試，平行）
- Task 2.3: fix round 1/5 (2 addressed, 0 open — 斷線殘留 pane、agent/大小寫篩選; commits 2c005fc..e7181e7)；Codex approve。
- Task 2.3: complete (commits 10ba90f..e7181e7 中的 61a390b、e7181e7, review clean)
- (2.4) DONE, commit 8bb9b04；domain_status 10/10。Codex review 派出。
- Task 2.4: complete (commits e7181e7..8bb9b04, review clean)；控制端實跑 cockpit-core 100 passed。

### Task 2.5

- dispatched implementer (opus) BASE=0e11331

### Task 3.1

- dispatched implementer (sonnet) BASE=0e11331，與 2.5 平行（config.rs 與 core 投影不重疊）
- (2.5) DONE_WITH_CONCERNS, commit 08404d4；core+cockpit 146 passed；新增 new_with_domain/set_domain/with_domain/spawn_projector_with_stale_sink；stale 用 try_send（滿則丟、下次投影重送）；覆蓋刪除前每次投影重送同一筆 → 3.3 需冪等（刪除前比對覆蓋仍相同）；BindingSource 加 serde。clippy config target 失敗來自 3.1 進行中檔案（非 2.5）。Codex review 派出。
- (2.5) Codex：needs-attention，1 high：try_send Full/Closed 被忽略，stale 覆蓋可能永久不刪 → fix round 1 resume。
- Ruling: stale 交付改 UnboundedSender＋投影端「已送出集合」去重（不做 ack 機制）— 寫入服務是唯一 receiver、與程式同生命週期，unbounded 不會丟；去重避免重複塞 channel — 代價：receiver 意外關閉時失效覆蓋不刪（記 error），重啟後投影重新判定。
- (3.1) DONE_WITH_CONCERNS, commit f50b6fe；config 42/42、cockpit 全過、clippy 0。
- Ruling: binding.runtime 對照「最終 runtime 清單（含設定檔無 [[runtime]] 時自動補的 local）」— spec 情境只要求未設定的 id 被拒；自動補的 local 確實會存在並被連線 — 代價：若使用者以為 local 未設定卻綁上，只是能綁到本機預設 socket，無害。
- (3.1) Codex review 派出。
- (2.5) fix round 1 實作完成 commit 8e51351；core+cockpit 177 passed、兩 crate clippy 0。Task 2.5: minor (deferred): 同一覆蓋在 50 ms 合併窗內被刪又設回且再失效時不重送。scoped re-review 派出。

### Task 3.2

- dispatched implementer (sonnet) BASE=8e51351
- (2.5) Codex re-review：原 finding（Full 遺失）已消除；新 high「send 成功即去重，接收端寫檔失敗則永不重送」。
- Task 2.5: parked — send 成功即去重、無 ack — Ruling: 不改。design D3 規定失效刪除「落檔失敗記 error、記憶體照刪」，寫入服務（3.3）收到後必定從 DomainState 移除該覆蓋，投影端隨即把它移出已送集合；receiver 在處理前終止＝程式結束，重啟後投影重新判定並再送（design Risks 已接受此窗口）。加 ack 協定超出 spec — 代價：若 3.3 未照 D3 在失敗時仍刪記憶體，覆蓋會殘留；3.3 dispatch 帶此約束並要求測試。
- Task 2.5: fix round 1/5 (1 addressed, 0 open; commits 0e11331..8e51351)
- Task 2.5: complete (commits 0e11331..8e51351, 1 parked)
- (3.1) Codex：needs-attention，high：binding.runtime 以含自動 local 的清單驗證；medium：缺漏 id 拿不到序號路徑。
- Ruling（撤銷上方 3.1 local 裁決）: 照 spec 第 55 行「設定檔中任何一筆 [[runtime]] 的 id」，自動補的 local 不算 — 先前裁決與 spec 原文衝突 — 代價：無 [[runtime]] 的設定檔無法綁 local，須明寫 [[runtime]]。兩條皆進 fix round 1。
- Task 3.1: fix round 1/5 (2 addressed, 0 open — 明確宣告 runtime、缺漏 id 序號路徑; commits f50b6fe..97cecac)；Codex approve。
- Task 3.1: complete (commits 08404d4..97cecac 中的 f50b6fe、97cecac, review clean)

### Task 6.1

- dispatched implementer (opus) BASE=97cecac，與 3.2 平行（檔案不重疊）
- (3.2) DONE, commit a877fe8；progress_file 11/11、cockpit gate 綠；未知項只 tracing warn、stage 不在 stages 才進投影 warnings（對照 spec 131-133 一致）。Codex review 派出。
- (3.2) Codex：needs-attention，1 high：projects/tasks/overrides 用 serde(default)，不完整狀態檔被當合法 → fix round 1 resume（改必填＋三測試）。
- (6.1) DONE_WITH_CONCERNS, commit d922a34；driver 14/14、core 全過；既有測試期望依 spec 情境 3 改為 snapshot 共 3 次。
- Ruling: 定期重拿進行中遇到 Drift → 併入、不追加重拿 — spec 只規範「由 Drift 觸發」的重拿，定期重拿本身就是收斂機制 — 代價：該窗口內的短暫倒退要等下一次 Drift 或定期重拿（預設 30 s）才收斂。
- (6.1) Codex review 派出。

### Task 6.3

- dispatched implementer (sonnet) BASE=d922a34，與 3.2 fix、6.1 審查平行（只動 cockpit-herdr）
- (6.1) Codex：needs-attention，1 medium（標 Important）：snapshot 完成與 Drift 事件同 tick 就緒時，biased select 先裝 snapshot，下一輪 Drift 被當新觸發多拿一次。
- Task 6.1: parked — 同 tick 就緒時 Drift 被當新觸發 — Ruling: 不改。該 Drift 是在替換「之後」才套用，store 已不在重拿中，spec「重拿進行中再遇到 Drift 不重複觸發」不適用；且新 snapshot 可能早於該 Drift 的成因，再拿一次語意上正確；biased 順序為 1b 既有設計（停止最優先、snapshot 先於事件避免事件洪流餓死重拿）。改成先排空事件反而要處理餓死 — 代價：競態時多一次 snapshot（WSL 端數百 ms），無正確性問題。
- Task 6.1: complete (commits a877fe8..d922a34, 1 parked)
- Task 3.2: fix round 1/5 (1 addressed, 0 open — 三欄位必填; commits d922a34..f9eb9e2)；Codex approve。
- Task 3.2: complete (commits 97cecac..f9eb9e2 中的 a877fe8、f9eb9e2, review clean)

### Task 3.3

- dispatched implementer (opus) BASE=a448e19
- (6.3) DONE, commit 9c4add4；cockpit-herdr 全過、reopen 新測試 RED 42→GREEN ≤8。Codex review 派出。

### Task 6.2

- dispatched implementer (sonnet) BASE=9c4add4，與 3.3 平行
- (3.3) DONE, commit af6a85e；progress_service 10 tests（含落檔失敗記憶體照刪、已改綁略過），連跑 5 次穩定；gate 綠。Codex review 派出。
- (6.3) Codex：唯一 finding 為 tasks.md 未勾（控制端刻意集中勾選，屬 plan-mandated 衝突）。
- Task 6.3: parked — tasks.md 勾選 — Ruling: 平行作業期間由控制端統一勾選（見 2.2/2.3 ruling），不要求實作 commit 帶勾 — 代價：無。runtime 邏輯 Codex 確認正確。
- Task 6.3: complete (commits a448e19..9c4add4, review clean on code)
- (6.2) DONE, commit 882fde4；driver 16/16。Codex review 派出。
- (3.3) Codex：needs-attention，1 high：寫入 future 被取消時 spawn_blocking detach，鎖釋放、set_domain 跳過、後續寫入與舊寫檔重疊 → fix round 1 resume。
- Ruling: 交易整段包進 tokio::spawn（service-owned task 持鎖到 set_domain 結束），呼叫端只 await JoinHandle；不做 actor 重構 — 最小修法即達取消安全 — 代價：每筆寫入多一次 task spawn，可忽略。
- (6.2) Codex：needs-attention，1 medium（Important）：snapshot Unavailable 測試沒有 60 秒前的負向斷言 → fix round 1 resume。
- Task 6.2: fix round 1/5 (1 addressed, 0 open; commits 882fde4..9c72091)；Codex approve。
- Task 6.2: complete (commits af6a85e..9c72091 中的 882fde4、9c72091, review clean)

### Task 5.1

- dispatched implementer (sonnet) BASE=9c72091，與 3.3 fix 平行
- Task 3.3: fix round 1/5 (1 addressed, 0 open — 交易改 service-owned spawn; commits 9c72091..f0d924b)；Codex re-review：原 finding ADDRESSED，另提 high「doc(hidden) with_write_hook 在 Finish panic 會使磁碟/記憶體分裂」。
- Task 3.3: parked — with_write_hook panic — Ruling: 不改。hook 只供測試、正式 `new` 不帶 hook、無任何正式呼叫端；panic 情境需呼叫端自己注入會 panic 的 callback — 代價：未來若有人在正式碼用 hook 可能踩到；final review 可再評估改為 cfg(test)/feature。
- Task 3.3: complete (commits 9c4add4..f0d924b 中的 af6a85e、f0d924b, 1 parked)

### Task 3.4

- dispatched implementer (sonnet) BASE=79f0dc4，與 5.1 平行
- (5.1) DONE, commit c130c51；瀏覽器腳本退避 1/2/4/8/8 PASS；cockpit 99 passed。Codex review 派出。
- (5.1) Codex：needs-attention，high：退避重設在 JSON.parse 之後（壞 JSON 首訊息不重設）；medium：http.rs 兩測試為原始碼字串比對。
- Ruling: 「收到第一則訊息」含壞 JSON（照 spec 字面）；前端行為證據以瀏覽器腳本為準，移除/降格 Rust 字串比對測試 — 代價：前端行為回歸不在 cargo test 內，需重跑研究腳本。fix round 1 resume。
- (3.4) DONE, commit bb391a4（自行勾了 tasks.md 3.4，審查未過會取消勾）；app 12/12、cockpit 全過。Codex review 派出。
- (3.4) Codex：needs-attention，high：bind 失敗洩漏 projector/stale-remover；medium：初始投影在 runtime 登記前發布、restart 測試沒驗投影 → fix round 1 resume。
- Task 5.1: fix round 1/5 (2 addressed, 0 open; commits bb391a4..eab33ac)；Codex approve。
- Task 5.1: complete (commits 79f0dc4..eab33ac 中的 c130c51、eab33ac, review clean)

### Task 5.2

- dispatched implementer (sonnet) BASE=eab33ac，與 3.4 fix 平行
- Task 3.4: fix round 1/5 (2 addressed, 0 open; commits eab33ac..e2ba407)；Codex approve。
- Task 3.4: complete (commits c130c51..e2ba407 中的 bb391a4、e2ba407, review clean)

### Task 4.1

- dispatched implementer (sonnet) BASE=e2ba407，與 5.2 平行
- (5.2) DONE_WITH_CONCERNS, commit 9caed2c；dump-dom 腳本 PASS；concern：fixture 只含 running/pending/completed，其他狀態與 unknown 未實測；未輸出按鈕（留 5.3）。Codex review 派出。
- (5.2) Codex：needs-attention，1 medium（Important）：驗收腳本／fixture 未涵蓋其餘狀態色、unknown、綁定摘要、override、warning → fix round 1 resume。
- (4.1) DONE, commit ac790d6；pipeline_api 16/16、http 5/5、gate 綠；最小改動 ui_preview.rs 一行（AppState::new）已通知 5.2；port 為 `Arc<AtomicU16>`，bind 後寫入，4.2 消費。Codex review 派出。
- (4.1) Codex：needs-attention，3 Important：run_with_shutdown 不回填實際埠；成功測試未驗狀態檔與投影；404 無 JSON 本體 → fix round 1 resume。
- Task 5.2: fix round 1/5 (0 addressed, 1 open — 顏色未以 computed style 斷言; commits ac790d6..10bb7a5)；fix round 2 resume。
- Task 5.2: fix round 2/5 (1 addressed, 0 open; commits 10bb7a5..2db1083)；Codex approve。
- Task 5.2: complete (commits e2ba407..2db1083 中的 9caed2c、10bb7a5、2db1083, review clean)

### Task 5.3

- dispatched implementer (opus) BASE=2db1083，與 4.1 fix 平行（不碰 http.rs/app.rs/pipeline_api.rs）
- Task 4.1: fix round 1/5 (3 addressed, 0 open; commits 2db1083..95de522)；Codex approve。
- Task 4.1: complete (commits 9caed2c..95de522 中的 ac790d6、95de522, review clean)

### Task 4.2

- dispatched implementer (sonnet) BASE=95de522，與 5.3 平行
- (5.3) DONE_WITH_CONCERNS, commit a606cce；actions-check.js PASS（pointerdown→click 突變掉到 0/10）；cockpit 121 passed。改動 5.2 腳本 regex 與 fixture（task 9 個）。Task 5.3: minor (deferred): 整頁重畫使鍵盤焦點消失（spec 未要求保留）。Codex review 派出。
- (5.3) Codex：needs-attention，1 high（Important）：較舊請求失敗會蓋上過期錯誤 → fix round 1 resume（遞增序號＋交錯回應情境）。
- (4.2) DONE, commit f67bf89；pipeline_api 20/20、workspace 全綠。Codex review 派出。
- Task 5.3: fix round 1/5 (1 addressed, 0 open; commits f67bf89..a7a721b)；Codex approve。
- Task 5.3: complete (commits 95de522..a7a721b 中的 a606cce、a7a721b, review clean)

### Task 6.4

- dispatched implementer (opus) BASE=9fc686f，與 4.2 審查平行
- (4.2) Codex：needs-attention，medium：重複 Host/Origin 只驗第一個 → Important，fix round 1 resume。Task 4.2: minor (deferred): `LOCALHOST:<port>` 大小寫被 403（瀏覽器與常見 CLI 送小寫，影響小）。
- (6.4) DONE_WITH_CONCERNS, commit 3f5ab38；app 18/18。
- Ruling: 接受 cockpit dev-dependencies 的 tokio 開 `test-util` feature — 非新套件、Cargo.lock 不變、cockpit-core 同做法，paused time 測試必需 — 代價：無。
- (6.4) Codex review 派出。
- Task 6.4: complete (commits 9fc686f..3f5ab38, review clean)

### Task 6.5

- dispatched implementer (sonnet) BASE=3f5ab38，與 4.2 fix 平行
- (4.2) fix round 1 commit 2972237（重複 Host/Origin 403＋測試）。Codex re-review 回 usage limit（重置 2026-09-17 02:13）。
- Ruling: Codex 額度用盡期間，依 1a 先例（git-branch-workflow 收尾清單）改派 fresh opus subagent 做對抗式審查，ledger 標「替代審查」；額度重置後對這些 commit 範圍補跑 Codex 同一 focus 作二次確認 — 代價：非 Codex 視角，二次確認可能翻出新 finding。
- 待 Codex 二次確認清單：4.2 r1（3f5ab38..2972237）
- Task 4.2: fix round 1/5 (1 addressed, 0 open; commits 3f5ab38..2972237)；替代審查（fresh opus）approve。Task 4.2: minor (deferred): duplicate_host 測試註解與測資不符、第二值本身不合法無法單獨證明「偵測重複」路徑（RED 實跑已佐證）。
- Task 4.2: complete (commits a606cce..2972237 中的 f67bf89、2972237, review clean；r1 待 Codex 二次確認)

### Task 4.3

- dispatched implementer (sonnet) BASE=2972237，與 6.5 平行
- (4.3) DONE, commit ee6dc2a；scenario_ 2/2。替代審查（fresh opus）派出。
- (4.3) 替代審查 approve。Task 4.3: minor (deferred): Scenario D 三 pane 同時 working，抓不到「task 讀錯 workstream binding」（spec 字面不要求）；D 未先驗 ready。
- Task 4.3: complete (commits 2972237..ee6dc2a, review clean；待 Codex 二次確認)
- 待 Codex 二次確認清單：4.3（2972237..ee6dc2a）

### Task 6.5（續）

- DONE, commit c0679d5；workspace 全過。
- F4 session guard：已補測（decide_reopen_commit_detects_round_changed、…_when_no_session、…_generation_changed、…_proceeds_when_unchanged；寫在 runtime.rs mod tests，因受測函數 crate-private）。
- Ruling: reopen.rs「重疊 350 ms」與 reopen 約 1.67 s 不改 paused time — FakeHerdr 走真實 named pipe/unix socket，實測 start_paused 使 5 s timeout 在真 connect 完成前觸發（Elapsed）— 代價：這些測試仍依真實時間，慢 CI 可能 flaky。
- Ruling: loop_integration.rs（約 2.5 s）不改 paused time — 需 multi_thread 真並行，tokio 巨集拒絕 start_paused＋multi_thread（編譯錯誤實測）— 代價：同上。
- Ruling: Path::exists() 權限情境不補測 — 生產碼無 Path::exists() 呼叫，存在判斷一律 read_to_string＋ErrorKind::NotFound — 代價：日後若新增 exists() 呼叫此 ruling 過期。
- Ruling: tokio-tungstenite 0.29（axum 內部）／0.30（cockpit dev 測試客戶端）並存不處理 — 依賴邊獨立、型別不跨界 — 代價：多編一份 crate。
- 替代審查（fresh opus）派出。
- (6.5) 替代審查（fresh opus）approve。Task 6.5: minor (deferred): decide_reopen_commit_detects_round_changed 的 generation 與參數相同，沒釘住「先驗 session 再驗 generation」順序（一行可補，final wave 處理）；Task 6.5: minor (deferred): reopen_after_debounce 呼叫處分派無測試（真實時序走不到，殘餘缺口）。
- Task 6.5: complete (commits 3f5ab38..c0679d5, review clean；待 Codex 二次確認)
- 待 Codex 二次確認清單：6.5（c0679d5）

### Task 7.2

- dispatched implementer (opus) BASE=c463ccd（真機 WSL；只寫 WSL 測試 server）

### Task 7.4

- Ruling: 7.4 拆兩段——設計文件回寫、README、.gitignore 現在派；docs/handover.md 重寫留到 final review 與 7.3 使用者驗收後由控制端做 — handover 要反映最終狀態 — 代價：7.4 勾選延後。
- dispatched implementer (sonnet) BASE=c463ccd，與 7.2 平行
- (7.4 前半) DONE, commit 2cb6632；markdownlint 58 files 0；check-ignore 有輸出（非錨定，涵蓋子目錄狀態檔）。文件內容正確性由 fresh sonnet 對照程式驗證（非 diff 審查，依 CLAUDE.md 歸 Claude subagent）。
- (7.4) 文件核對 has-errors：§10.2 標題仍寫 1b；C 列、D 列把 domain/畫面/真機驗證混掛在 pipeline_api 測試名下 → fix round 1 resume。其餘精確值（路由、狀態碼、Host/Origin、TOML 欄位、狀態檔、畫面、curl）核對正確。
- (7.4) fix round 1 commit 8136770；控制端讀 diff 確認 C/D 驗法歸屬已拆清楚。7.4 前半完成；handover 重寫待收尾（未勾）。

- (7.2) DONE_WITH_CONCERNS, commit 057aaed；pipeline-check.py PASS（以最多 30 s 收斂為前提）。**發現產品 bug**：WSL HERDR 0.8.2 新訂閱重播整段事件歷史，snapshot 後到的重播被套用，投影倒退至多 30 s（無 Drift）。pane id 不重用（wE:p5 → wE:p6）。已寫 memory herdr-subscribe-replays-event-history（鐵則 10）。
- 此為 plan 未預期的設計缺陷（spec runtime-driver 需改），修法有多種取捨 → 停下請使用者決定。
- 控制端唯讀擷取 Windows 0.9.0（capture_events 5 s）：只 2 筆當前值 pane_updated，無歷史重播 → bug 目前只影響 WSL 0.8.2。
- 使用者決定（2026-09-17）：重播 bug 本 change 內修；/opsx:update 寫入 runtime-driver ADDED「連線後沉降重拿」（靜默 1 s、上限 5 s）、design D12＋Risks、tasks 6.6、7.2 嚴格時限重跑、7.4 補 §2.3。commit 13d6aae。

### Task 6.6

- dispatched implementer (opus) BASE=13d6aae
- (6.6) DONE_WITH_CONCERNS, commit 0bb3e10；driver 20/20；5 個既有測試期望調整（snapshot +1、periodic 時間軸後推 1 s）；併入進行中重拿無專屬測試。替代審查（fresh opus）派出。
- (6.6) 替代審查 needs-attention：I1 併入進行中重拿無測試（Important）；M1 沉降失敗→Disconnected 無測試（一併補）。Task 6.6: minor (deferred): M2 併入第 2 次追加重拿且其間套用重播時，要等定期重拿（需 snapshot 連慢 3 次）。→ fix round 1 resume。
- (6.6) fix round 1 commit 7d4becd（2 測試＋突變驗證）；scoped re-review（同一 opus reviewer resume）派出。
- Task 6.6: fix round 1/5 (2 addressed, 0 open; commits 0bb3e10..7d4becd)；替代審查 approve。
- Task 6.6: complete (commits 0bb3e10..7d4becd, review clean；待 Codex 二次確認)
- 待 Codex 二次確認清單：6.6（0bb3e10 前一 commit..7d4becd）
- (7.2) fix round 1 commit 9162205：嚴格 3 s 時限下重啟情境 FAIL（4.1／4.45 s 成立），其餘 PASS；S 重開未見重播倒退；pane id 不重用。WSL 重播經 wsl.exe＋nc 送達慢（到 t+3.36 s 仍在套用），沉降在上限內（≤5 s）蓋回。
- Ruling: 7.2 時限由 3 s 改為 6 s（沉降上限 5 s＋1 s 量測餘裕）— 3 s 是控制端寫 tasks 時自訂、與已核准的 spec 上限 5 s 不一致（plan 缺陷）；實作符合 spec — 代價：WSL 0.8.2 啟動／重連後最多約 5 s 畫面不準（原 30 s），需向使用者揭露（先前口頭估 1–2 秒偏樂觀）；Windows 0.9.0 不受影響。tasks.md 7.2、design Risks 同步改。
- Task 7.2: fix round 2 commit 4067fb7：6 s 門檻 PASS（啟動 2.30 s、重啟 4.20 s；其餘 ≤0.2 s），WSL server 已停。控制端修正驗收文件 Windows 結論來源。
- Task 7.2: complete (commits c463ccd..4067fb7；驗收腳本實跑為證據)
- Task 7.4: 前半 complete（2cb6632、8136770、a0b51ae；文件核對 subagent 驗證＋控制端讀 diff）；handover 重寫待收尾。
- 7.1 預跑（HEAD f4bb1a8＋a0b51ae）：fmt 0、clippy 0、cargo test --workspace 493 passed／0 failed／10 ignored、markdownlint 58 files 0 issues、openspec validate --all 12 passed／0 failed。final review 修正波後重跑並貼入驗收文件。
- Task 7.5: complete — 去識別化 grep（使用者名稱、email、家目錄；research/2026-09-16、cockpit/tests/fixtures、cockpit.example.toml）輸出為空（grep exit 1）；git check-ignore cockpit.state.json 有輸出。
- Final whole-branch review：MERGE_BASE=536204eca278400c4a1ab9e1e9698a8d60c94c0d；Codex 額度 02:13 重置，排程 02:16 背景啟動（同 job 兼做 4.2 r1、4.3、6.5、6.6 的 Codex 二次確認）。
- Final review（Codex, 02:16）：輸出「approve / No material findings」為**無效結果**——log 顯示執行 10 個指令後 `usage limit … try again at Sep 20th, 2026 9:58 AM`、Turn failed。
- Ruling: final whole-branch review 改派 fresh opus 替代審查（1a 先例）；Codex 全分支審查＋4.2 r1／4.3／6.5／6.6 二次確認延到 2026-09-20 09:58 後補跑，handover 註明，未補跑前不宣稱 merge ready — 代價：合併需等 Codex 或使用者明示接受替代審查。
- Final review（fresh opus 替代）：無 Critical／Important；Minor M1–M6；deferred/parked 全數 ok-to-defer。Verdict（僅程式碼）ready-to-merge。
- Ruling: final 修正波只修 M4（狀態檔 HashMap→BTreeMap 固定順序）、M6（README 改狀態檔要先停服務）、M1／M5 以 README 註明（listen 須為 127.0.0.1／localhost／[::1] 且帶非預設埠；[state] path 父目錄須存在）、6.5 minor（round_changed 測試 generation 改不同值釘住順序）；M2（斷線期間無取消改綁按鈕）、M3（重畫丟焦點）留交接 — 修正小、spec 字面已滿足 — 代價：M2/M3 使用性問題留到下個 change。
- Task 7.1: complete — gate 於 14959a3 全綠（fmt 0、clippy 0、test 494/0/10、markdownlint 58 files 0、openspec 12/0），輸出寫入驗收文件。
- Final fix wave（fc5744d、6ab160f、14959a3）scoped re-review：M4、M6、M5、6.5 ADDRESSED；M1 的 80 埠 README 解法錯（實測網址明寫 :80 仍送 Host 無埠 → 403）；M5 出處指錯檔。
- Ruling: 殘餘兩處為 README 單句錯誤，skill 不開第二修正波，控制端直接改句（reviewer 已附實測重現，改為「不要用 80 埠」、出處改 write_atomically）— 代價：此兩句未經第二次審查；Codex 9/20 補審時涵蓋。
- 7.4 後半：handover v12 重寫，fresh sonnet 核對事實（修正 cockpit-dashboard delta 分類）後 commit。7.4 待 7.3 與 Codex 補審後勾選。
- Task 7.3: complete（2026-09-19）——使用者目視網格／推進／改綁「沒問題」；控制端代跑重啟保留（覆蓋＋completed 標記重啟後逐欄相同、狀態檔位元組相同）與清除標記，驗收後還原狀態。未做 Windows 端覆蓋失效目視（覆蓋指向使用中 agent pane），沿用 7.2 自動驗收。
- Task 7.4: complete（設計文件 §2.3／§8.x／§10.2、README、.gitignore、handover v12→v13）。
- tasks.md 27/27 全勾。剩：Codex 補審（9/20 09:58 後）→ 使用者授權 squash 併回 main → archive（ledger 去識別化複製成 sdd-ledger.md）。
