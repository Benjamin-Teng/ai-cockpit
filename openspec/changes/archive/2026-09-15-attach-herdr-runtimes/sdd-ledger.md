# SDD ledger — plan: openspec/changes/attach-herdr-runtimes/tasks.md

Spec authority: openspec/changes/attach-herdr-runtimes/specs/*/spec.md（7 份）；design：同目錄 design.md（D1–D16）；
設計文件：docs/superpowers/specs/2026-09-13-cockpit-mvp-design.md。
Branch: feat/attach-herdr-runtimes（從 main a575842 分出；propose commits 5000a44、545b311）。
Started: 2026-09-14。

## Setup rulings

- Ruling: 在既有分支 feat/attach-herdr-runtimes 的主 checkout 直接實作，不另開 worktree — 分支已非 main，1a 也是同一做法，
  另開 worktree 會讓 target/ 重建與路徑（.superpowers、fixture 相對路徑）全部改變 — 代價：若使用者同時在 main 上動手，工作樹會混；
  目前 repo 只有這條線在動。
- Ruling: task reviewer 一律用 Codex adversarial-review（CLAUDE.md「Code review 路由」），不派 Claude subagent；
  reviewer 模板的內容（brief、report、review package 路徑、global constraints）改寫進 Codex 的 focus 字串；
  Codex 沙箱唯讀、只能跑 `cargo fmt --check`，測試證據以實作者報告為準 — 代價：Codex 靜態推導可能誤判，findings 實測後才採信。
- Ruling: 實作者 model：規格完整、1–2 檔的 task 用 sonnet；驅動器狀態機（1.7–1.9）、HerdrRuntime 連線／重開（2.5–2.7）用 opus；
  fix round 4–5 升級一階。
- Ruling: 每個 task 用 TDD（superpowers:test-driven-development），報告要有 RED／GREEN 證據；review 通過後一個 commit
  （handover §5 授權）。

## Pre-flight scan（2026-09-14）

| 對 / 單一 task | 產出 vs 消費 | 結果 |
|---|---|---|
| 1.1 ↔ 1.2–1.9 | 1.1 建 crate 與依賴（含 chrono）；後續加模組 | 一致；1.5 的 RFC 3339 測試需要 chrono，1.1 已列 |
| 1.2 ↔ 1.3/1.4 | 1.2 定義 RuntimeEvent 全部變體（含 D3 三個）；1.3 apply 要處理每個變體 | 一致 |
| 1.2 ↔ 1.7 | 1.2 出 AgentRuntime／RuntimeEvents／RuntimeError；1.7 消費 | 一致；RuntimeEvents 的 drop→abort 由 1.2 測 |
| 1.5 ↔ 1.6 | 1.5 純函數投影；1.6 要「忽略 generated_at 的相等比較」 | 缺口：1.5 要提供不含 generated_at 的比較（例如 content_eq 或把 generated_at 排除在 PartialEq 外），1.6 才有得用 → 帶進 1.5 dispatch |
| 1.6 ↔ 1.7 | 1.6 StoreHandle 的 replace／apply／set_connection 都要標 dirty；1.7 驅動器只透過 StoreHandle 寫 | 一致；順序 1.6 先 |
| 1.7 ↔ 1.8 ↔ 1.9 | 同一個 driver.rs 與 FakeRuntime 逐段擴充 | 一致（循序）；1.7 的 FakeRuntime 要一次做齊「可延遲的 snapshot 佇列、可回 Unavailable」，1.8/1.9 才不用改鷹架 → 帶進 1.7 dispatch |
| 2.1 ↔ 2.2 | 同一個 translate.rs | 一致 |
| 2.4 ↔ 2.7 | 2.4 的 with_method_responses 供 2.7 的 Drift 重拿測試 | 一致 |
| 2.5 ↔ 2.6 | 2.5 的 subscribe() 已「重置 S 管理器」，代表 S 管理器結構在 2.5 建立、2.6 加 reopen | 一致；2.5 dispatch 要說明 S 管理器最小形狀 |
| 2.7 ↔ 3.2 | 2.7 出 cockpit_herdr::build(HerdrEndpoint, options)；3.2 消費 | 一致；options 至少含 wsl_probe_secs |
| 3.3 ↔ 3.5/3.7 | 3.3 放最小版 assets；3.5、3.7 換正式內容並加測試 | 一致 |
| 1.6 ↔ 3.4 | watch channel 供 WS 訂閱 | 一致 |
| 3.1 ↔ 3.8 | load(args, cwd, lookup_env)；main 包真值 | 一致 |
| 4.1／4.2 | 需要使用者目視與在 pane 下指令 | 非自動化：到時停下請使用者做，其餘部分先做完 |
| 各 task 自洽 | 每個 task 的測試名 vs 程式碼 | 一致；1.1 的 `grep -c` 在 0 筆時 exit 1，只當人工驗收指令用，不放進腳本 |
| 審查準則衝突 | 有沒有 task 強制寫「不斷言的測試」或重複邏輯 | 無 |

## Task log

- Ruling: `scripts/task-brief` 只認 `Task N` 標題、對 `- [ ] X.Y` 格式回 not found，brief 改由控制端手寫
  （標頭約束＋task 原文＋相關 spec／design 條文）到 `.superpowers/sdd/tasks/task-X.Y-brief.md` — 代價：手寫可能漏抄，
  reviewer 同時拿得到 tasks.md 路徑可對照。
- Ruling: 實作者在 review 前就 commit（review-package 需要 git 範圍；Codex 也能審已 commit 的分支 diff），fix round
  追加 commit，收尾時 squash 併回 main — handover §5 寫「review 通過後一個 commit」，語意上等價（最終併入 main 的
  是一個 squash）— 代價：分支歷史較碎，squash 時消失。
- Ruling: `cockpit` crate 同時有 `src/lib.rs`（config、runtimes、http、ws 等模組，供 `tests/` 整合測試引用）與薄的
  `src/main.rs`；tasks.md 寫「`src/lib.rs` 或 `src/main.rs`」沒說清楚 — 整合測試無法引用 bin crate 的模組 —
  代價：多一個 lib target，可忽略。
- Ruling: 依賴版本一律用 `cargo add` 讓 cargo 從 registry 取最新相容版並寫進 Cargo.lock，不查網頁抄版本號 —
  符合 CLAUDE.md 鐵則 3（不填記憶值）— 代價：無。

### Task 1.1

- dispatched implementer (sonnet) BASE=545b311; DONE, commit 90f5336; baseline workspace 181/0/7; gate 全綠。
- concern（不擋）：tokio-tungstenite dev 解到 0.30，axum 內部 tungstenite 0.29，lock 出現兩版；design 風險表已預期（只當測試客戶端）。
- review package: .superpowers/sdd/tasks/review-545b311..90f5336.diff；Codex adversarial-review 派出（背景 job）。
- Codex review：needs-attention，2 medium findings 皆為「cockpit-herdr／cockpit 的 Cargo.toml 含 brief 未列的共用依賴
  （tokio、serde、serde_json、thiserror、tracing、async-trait）」；Part 2 無實質品質問題；fmt --check 實跑 exit 0。
- Ruling: 保留這些共用依賴、不進 fix loop — 這些依賴是我在 dispatch 裁決 3 明確要求加的，task 1.1 的目的是一次定好三個
  crate 的依賴集（proposal Impact 已列為新 crate 的依賴集），1.2／2.x／3.x 立刻會用到；tasks.md 1.1 原文「加 X、Y」有歧義，
  已改寫為「共用依賴三個 crate 依需要都可列」並標 [x] — 代價：cockpit 的 tokio／serde 在 3.1 之前掛著沒用（無 warning）；
  若最終 review 仍有未用依賴再移除。
- Task 1.1: complete (commits 545b311..90f5336, 2 findings ruled — plan wording amended)

### Task 1.2

- plan amendment commit c7b04d9（tasks.md 1.1 措辭）；dispatched implementer (sonnet) BASE=c7b04d9。
- Ruling: `FocusChanged` 帶獨立的 `FocusChange` struct（三個 Option，None＝該層不變），不共用 `Focused`（None＝沒有焦點）—
  兩者語意不同 — 代價：多一個小 struct。
- Ruling: 四個 id 都用 newtype（serde transparent）— 型別安全，避免 pane id 與 tab id 混用 — 代價：翻譯層多幾個 `.into()`。
- 1.2 DONE, commit 0159029；TDD RED→GREEN；crate gate 全綠。review package: review-c7b04d9..0159029.diff；Codex 派出（背景）。
- Codex review 1.2：needs-attention。(1) medium：CAPACITY 1024 只是常數，`RuntimeEvents::new(rx, tasks)` 接受任意容量的 receiver；
  (2) low：id newtype 多了 new/as_str/Display/Hash/Ord/serde。
- Ruling: (2) 保留 — 這些是 dispatch 裁決 2 要求的，1.3 用 Hash/Eq 當 map key、1.5 用 serde 出 JSON、Display 進日誌與 reason；
  low 屬 Minor 不進 loop — 代價：若最終沒用到某個 trait，final review 再刪。
- Task 1.2: fix round 1/5 開始（1 open：容量由建構 API 強制）— 修法：`RuntimeEvents::channel() -> (mpsc::Sender<Item>, Unstarted)`、
  `Unstarted::start(tasks) -> RuntimeEvents`，拿掉公開的 `new(rx, tasks)`；測試改用 channel()，加一個斷言 sender.max_capacity()==1024。
- Task 1.2: fix round 1/5 實作完成（commit 9012a9c：channel()/UnstartedEvents::start()，新測試 runtime_events_channel_has_fixed_capacity；
  5 tests pass、gate 綠）；scoped re-review package review-0159029..9012a9c.diff，Codex 派出（背景）。
- Task 1.2: fix round 1/5 (1 addressed, 0 open — 容量由 channel() 強制; commits 0159029..9012a9c)；Codex re-review approve，無新破壞。
- Task 1.2: complete (commits c7b04d9..9012a9c, review clean after 1 fix round; 1 low finding ruled keep)

### Task 1.3

- dispatched implementer (sonnet) BASE=9012a9c。
- Ruling: `RuntimeStore` 要先 `register(id, kind, endpoint)`，對未登記 runtime 的 `apply`／`replace` 回 `Err(Drift)`（不 panic、不隱式登記）—
  kind／endpoint 是投影要的登記資料，隱式登記會留空字串 — 代價：測試多一行 register。
- Ruling: `apply` 帶 `now: SystemTime` 參數（AgentStatusChanged／PaneExited／AgentDetected 要更新 updated_at，1.5 的 recent_events 也要 at）—
  時鐘注入讓測試可決定 — 代價：驅動器多傳一個參數。
- Ruling: `WorkspacesReplaced`／`TabsReplaced` 對「新清單裡不再存在」的父層同樣連鎖刪除子物件（與 D5 一致）；
  `PaneMoved` 時 agent 紀錄跟著換 key（若舊 pane 有 agent 紀錄）— 代價：HERDR 若在 moved 後再發 agent_detected，只是覆蓋。
- Ruling: `FocusChanged` 的「同層只有一個」＝整個 runtime 同層唯一（對齊 `Focused` 只有單一 id）；主體不存在 → Drift。
- 1.3 DONE, commit a058fe0；8 tests RED→GREEN；gate 綠。concern：`Tab` 無 label 欄位，`TabRelabeled` 只驗存在性——
  與 §6.4 投影（tab 不顯示 label）一致，接受。review package review-9012a9c..a058fe0.diff；Codex 派出（背景）。
- Codex review 1.3：needs-attention，3 findings（high：PaneUpserted/PaneMoved/FocusChanged 未設 updated_at=now；high：AgentStatusChanged
  在紀錄缺失且事件 agent=None 時不補建；medium：連鎖刪除走父→子 map 會漏孤兒）。三項核實成立。
- Task 1.3: fix round 1/5 開始（3 open）；resume 原實作者，修法：updated_at 一律 now、紀錄名稱取事件 agent 否則 pane.agent、連鎖刪除依
  workspace_id/tab_id 欄位 retain；補 6 個測試。
- Task 1.3: fix round 1/5 實作完成（commit 2c2e197，14 tests pass，gate 綠）；scoped re-review package review-a058fe0..2c2e197.diff，Codex 派出（背景）。
- Task 1.3: fix round 1/5 (2 addressed, 1 open — finding 3 的 WorkspacesReplaced/TabsReplaced 仍以舊 map 算 removed，父層本缺的孤兒殘留; commits a058fe0..2c2e197)
- Task 1.3: fix round 2/5 開始（1 open）；修法：replacement 後以 keep set retain 子物件；補 2 測試。
- Task 1.3: fix round 2/5 實作完成（commit baefc1f，16 tests，gate 綠）；scoped re-review package review-2c2e197..baefc1f.diff，Codex 派出（背景）。
- Task 1.3: fix round 2/5 (1 addressed, 0 open; commits 2c2e197..baefc1f)；Codex approve，無新破壞。
- Task 1.3: complete (commits 9012a9c..baefc1f, review clean after 2 fix rounds)

### Task 1.4

- dispatched implementer (sonnet) BASE=baefc1f。
- Ruling: `set_connection(id, state) -> Result<(), Drift>`（未登記 → Drift，與 replace/apply 一致）、`connection(id) -> Option<&ConnectionState>` —
  代價：驅動器要處理一個幾乎不會發生的 Err。
- 1.4 DONE, commit b61a073；21 store tests；4 個測試為 1.3 行為的回歸保護、1 個 RED→GREEN；gate 綠。review package review-baefc1f..b61a073.diff；Codex 派出（背景）。
- Codex review 1.4：needs-attention，1 medium：連線測試沒證明 set_connection 只動 connection、沒測未登記 → Drift（實作靜態檢查正確）。
- Task 1.4: fix round 1/5 開始（1 open，只補測試）。
- Note: Codex focus 字串用 bash 雙引號傳，內含反引號會被當指令替換執行（本輪出現 "translated_drift_passes_through: command not found"，無害）；
  之後 focus 文字不用反引號。
- Task 1.4: fix round 1/5 實作完成（commit 283ccdd，22 store tests，gate 綠）；scoped re-review package review-b61a073..283ccdd.diff，Codex 派出（背景）。
- Task 1.4: fix round 1/5 (1 addressed, 0 open; commits b61a073..283ccdd)；Codex approve。
- Task 1.4: complete (commits baefc1f..283ccdd, review clean after 1 fix round)

### Task 1.5

- dispatched implementer (sonnet) BASE=283ccdd。
- Ruling: 純函數簽章 `project(&RuntimeStore, version: u64, now: SystemTime) -> ProjectedState`，version 由投影任務（1.6）決定；
  `ProjectedState::content_eq(&self, &other)` 忽略 version 與 generated_at — 代價：無。
- Ruling: recent_events 由 `RuntimeStore::apply` 記錄（成功、Noted、Drift 都記；kind 為 RuntimeEvent 變體的 snake_case，Noted 用原 HERDR 事件名，
  Drift 用 "drift"）；投影結構的時間欄位在投影時就轉成 RFC 3339 字串並 derive Deserialize（3.5 的 fixture 要能反序列化）— 代價：
  投影結構與模型結構分開維護。
- 1.5 DONE, commit d6245d6；7 projection tests＋既有 27 全過；gate 綠。concern：未跑形式 RED（測試與實作同輪）。
- Ruling: 1.5 不重寫，改要求突變證據（對每個測試各弄壞一處實作、確認該測試失敗、還原）補進報告，在 review 後的 fix round 一併做 —
  TDD 的目的是證明測試能抓錯，突變檢查提供等價證據 — 代價：若某測試突變也不失敗，該測試要重寫。
- review package review-283ccdd..d6245d6.diff；Codex 派出（背景）。
- Codex review 1.5：needs-attention，2 findings（high：AgentStatusChanged 的 kind 應為 pane.agent_status_changed（§6.4、4.2 驗收）；
  medium：pane／tab 事件多填父層 id，spec「主體 id」單數）。兩項核實成立。
- Ruling: 撤回「kind＝變體 snake_case」對 AgentStatusChanged 的適用，改 pane.agent_status_changed（spec／設計文件優先）；其餘變體維持
  snake_case — 代價：kind 命名混合兩種風格，集中定義在一處減少混淆。
- Ruling: recent_events 每筆只填一個主體 id（dispatch 與 brief 矛盾，依 spec 單數）— 代價：畫面上 pane 事件看不到所屬 tab，可從 id 前綴推。
- Task 1.5: fix round 1/5 開始（2 open ＋ 突變證據要求）。
- Task 1.5: fix round 1/5 實作完成（commit b028dae；36 tests；突變檢查 7/7 失敗；額外發現 content_eq 與 orphan 測試原本測不到，已補強）；
  scoped re-review package review-d6245d6..b028dae.diff，Codex 派出（背景）。
- Ruling（流程）: 自 1.6 起每個 task 的 dispatch 都要求「突變證據」（每個新測試至少一個突變會讓它失敗），與 RED 證據並列 —
  1.5 的突變檢查揪出兩個 RED 也不會揪出的洞（content_eq 沒被測、orphan 測試對照組缺失）— 代價：每個 task 多幾分鐘。
- Task 1.5: fix round 1/5 (2 addressed, 0 open; commits d6245d6..b028dae)；Codex approve。
- Task 1.5: complete (commits 283ccdd..b028dae, review clean after 1 fix round; 突變證據 7/7)
- tasks.md 勾選 commit a87036b。

### Task 1.6

- dispatched implementer (sonnet) BASE=a87036b。
- Ruling: `StoreHandle::new(store)` 建立時就送出第一份投影（version 1），之後只在 content_eq 不等時遞增；`subscribe()` 回 watch::Receiver；
  wrapper 方法（register/replace/apply/set_connection）鎖→呼叫→notify_one；`spawn_projector(handle)` 起任務 — 代價：無。
- 1.6 DONE, commit 1b6ca3c；4 projector tests RED→GREEN，突變 4/4；crate 40 tests；gate 綠。
- Ruling: 允許 cockpit-core 加 `[dev-dependencies] tokio = { features = ["test-util"] }`（虛擬時間測試必要；正式依賴不動）；
  1.7–1.9 同樣沿用 — 代價：無。
- review package review-a87036b..1b6ca3c.diff；Codex 派出（背景）。
- Codex review 1.6：needs-attention，1 high：watch::Sender::send 在零 receiver 時不更新保留值 → 新訂閱者拿舊投影；核實成立（tokio watch 語意）。
- Task 1.6: fix round 1/5 開始（1 open）；修法 send_replace＋零 receiver 測試。
- Task 1.6: fix round 1/5 實作完成（commit 4715e22，41 tests，gate 綠）；scoped re-review package review-1b6ca3c..4715e22.diff，Codex 派出（背景）。
- Task 1.6: fix round 1/5 (1 addressed, 0 open; commits 1b6ca3c..4715e22)；Codex approve。
- Task 1.6: complete (commits a87036b..4715e22, review clean after 1 fix round)

### Task 1.7

- dispatched implementer (opus) BASE=4715e22。
- Ruling: 1.7 只實作「連線→丟棄→套用→可停止」，錯誤與串流結束一律 Disconnected{retry_in: 1s} 固定等 1 秒後重來（占位，1.9 換退避）、
  不做 Drift 重拿與定期重拿（1.8 用 RED 補）— 讓 1.8／1.9 的 TDD 有真正的 RED — 代價：1.7 的迴圈骨架在 1.8／1.9 會被改兩次。
- Ruling: 停止用 `tokio::sync::oneshot::Receiver<()>`（sender drop 也算停止），不引 tokio-util — 代價：無。
- Ruling: `Policy { resnapshot: Duration, backoff: Vec<Duration> }` 一次定義齊（Default 30s／1,2,4,8,16,30），1.7 只用 resnapshot 以外的欄位當占位 — 代價：無。
- 1.7 DONE, commit e16a197；3 driver tests RED→GREEN，突變 4/4；crate 44 tests；gate 綠。
- concern（帶進 1.9）：PLACEHOLDER_RETRY 1s 與 backoff[0] 同值，退避測試要驗到第 2、3 次才有鑑別力。
- Ruling: 實作者把 commit trailer 寫成 Opus 型號，不 amend（squash 併 main 時統一 trailer）— 代價：分支歷史 trailer 不一致，squash 後消失。
- review package review-4715e22..e16a197.diff；Codex 派出（背景）。
- Codex review 1.7：approve，0 findings。
- Task 1.7: complete (commits 4715e22..e16a197, review clean)

### Task 1.8

- dispatched implementer (opus, fresh) BASE=e16a197。
- 1.8 DONE, commit bfb49af；5 driver tests（4 RED，1 否定斷言靠突變證明）；突變 6/6；crate 49 tests；gate 綠。
- concern（帶進 final review／1b 真機）：持續 Drift 會變緊密重拿迴圈（spec 未要求最小間隔；design 風險表已列，真機驗收看重拿頻率再決定）。
- review package review-e16a197..bfb49af.diff；Codex 派出（背景）。
- Codex review 1.8：approve，0 findings。
- Task 1.8: complete (commits e16a197..bfb49af, review clean)

### Task 1.9

- dispatched implementer (opus, fresh) BASE=bfb49af。帶入 1.7 concern（占位 1s＝backoff[0]，測試要驗到第 2、3 次）與 1.8 提醒（advance 分小步）。
- 1.9 DONE, commit 760b078；5 driver tests（3 RED，2 靠突變）；突變 7/7；crate 54 tests；gate 綠；既有測試零修改。
- minor (deferred): Unavailable 只從 subscribe 來源測過（snapshot／串流來源靠共用 Outcome::from_error）；retry_after 0 無下限。
- review package review-bfb49af..760b078.diff；Codex 派出（背景）。
- Codex review 1.9：approve，0 findings。
- Task 1.9: complete (commits bfb49af..760b078, review clean)
- 第 1 組完成：cockpit-core 54 tests；tasks.md 勾選 commit（見 git log）。

### Task 2.1

- dispatched implementer (sonnet) BASE=tasks.md 勾選 commit 之後的 HEAD（見上一行 git log）。
- Ruling: HERDR `AgentInfo.agent` 為 None 的紀錄不翻（core `Agent.agent` 是 String），記 tracing::debug — 代價：畫面上該 pane 仍有狀態，只是 agents 清單少一筆（投影不用 agents 清單）。
- Ruling: `TabInfo.label` 丟棄（core `Tab` 無 label，投影不顯示）— 代價：日後要顯示 tab 標籤得回頭加欄位。
- （補記）第 1 組勾選 commit 7e6e592；Task 2.1 BASE=7e6e592。
- 2.1 DONE, commit 7a122e5；3 tests RED→GREEN；突變 4/4；gate 綠。minor (deferred)：pane label 空字串→None 沒有 fixture 覆蓋（共用 label() helper）。
- review package review-7e6e592..7a122e5.diff；Codex 派出（背景）。
- Codex review 2.1：approve，0 findings。
- Task 2.1: complete (commits 7e6e592..7a122e5, review clean)

### Task 2.2

- dispatched implementer (sonnet) BASE=7a122e5。
- Ruling: `translate::event(&IncomingEvent, now) -> Option<RuntimeEvent>`（加 now 參數；payload 解析失敗回 Some(Drift)；忽略類回 None 並 tracing::debug）；
  測試端自行由事件名判斷 Lifecycle／PerPane（含 '.' 者為 PerPane，用 serde 反序列化 kind）— 代價：測試多一個小 helper。
- 2.2 DONE, commit ac9972b；8 tests RED→GREEN；突變 8/8；gate 綠。note：解析用 macro parse_or_drift!（serde 非直接依賴，不改 Cargo.toml）。
- review package review-7a122e5..ac9972b.diff；Codex 派出（背景）。
- Codex review 2.2：approve，0 findings。
- Task 2.2: complete (commits 7a122e5..ac9972b, review clean)

### Task 2.3

- dispatched implementer (sonnet) BASE=ac9972b。
- Ruling: 探測邏輯拆成純函數 `evaluate(distro, outcome)`（可測）＋ 真實 `WslProber`（spawn）；`FakeProber` 放 tests/common（只有 cockpit-herdr 的測試用）—
  代價：無。
- 2.3 DONE, commit 63e8f68；4 tests＋1 ignored；突變 4/4；真機 probe 回「未啟動」（WSL 未跑，指令正常）；gate 綠。
- review package review-ac9972b..63e8f68.diff；Codex 派出（背景）。
- Codex review 2.3：approve，0 findings。
- Task 2.3: complete (commits ac9972b..63e8f68, review clean)

### Task 2.4

- dispatched implementer (sonnet) BASE=63e8f68（只動 herdr-client/src/testing、tests/fake_herdr.rs、README）。
- Ruling: 同一 method 同時用過 with_method_response 與 with_method_responses 時「最後呼叫的 builder 勝出」— 代價：無。
- 2.4 DONE, commit 73d904a；herdr-client 183 passed／7 ignored；突變 3/3；gate＋markdownlint 綠。
- review package review-63e8f68..73d904a.diff；Codex 派出（背景）。
- Codex review 2.4：approve，0 findings。
- Task 2.4: complete (commits 63e8f68..73d904a, review clean)

### Task 2.5

- dispatched implementer (opus) BASE=73d904a。
- Ruling: `HerdrRuntime::new(id, connector, prober: Option<Box<dyn DistroProber>>)`，wsl_probe_secs 由 prober 自帶（WslProber.retry_after），
  簽章不重複傳 — 代價：無。
- Ruling: 任一 reader task 結束時送 Err 後 abort 另一條（共用 abort handle 集合），讓 next() 之後回 None（spec「之後流結束」）—
  代價：多一個共用結構。
- Ruling: 2.5 的 `snapshot()` 先做純翻譯版（protocol_warning None、不通知 S 管理器），2.6 再擴 — 讓 2.6 有 RED — 代價：無。
- Ruling: 「釋放事件流 → 假 HERDR 觀察到連線關閉」若 FakeHerdr 沒有觀察 API，可加一個加法的 test-support API（例如 open 連線計數），
  否則退而斷言 reader task 被 abort 的 drop 旗標並在報告註明 — 代價：可能再動 herdr-client 一次。
- 2.5 DONE, commit 0521bd5；7 session tests；突變 9/9；cockpit-herdr 22 tests、herdr-client 24 fake_herdr tests；workspace 全綠。
- Ruling: 接受 herdr-client test-support 的 `Step::Hold` 語意由「pending 到 FakeHerdr drop」改為「讀到對端關閉為止」＋新增
  `FakeHerdr::closed_connections()`——這是唯一能觀察對端關閉的做法，1a 既有測試全過、只在 test-support 內 —
  代價：Hold 不再是「絕對不讀」的掛起；若某測試依賴這點，final review 會抓到。
- minor (deferred): reader task 收尾會 abort 自己的 AbortHandle（無害）；StatusSubscription.pane_ids 只寫不讀（2.6 用）。
- review package review-73d904a..0521bd5.diff；Codex 派出（背景）。
- Codex review 2.5：needs-attention，2 findings（high：L/S 同時終止可能送兩個 Err；medium：建構子無 wsl_probe_secs，固定間隔靠 prober 自帶）。
- Ruling: 撤回「wsl_probe_secs 由 prober 自帶」— `WslProbe` 加 `retry_after`，runtime 把任何探測錯誤正規化成 Unavailable{retry_after}；
  建構子維持 `new(id, connector, Option<WslProbe>)`（struct 比散裝參數清楚），tasks.md 2.5 簽章文字在完成時改寫 — 代價：與 plan 字面不同。
- Task 2.5: fix round 1/5 開始（2 open）。
- Task 2.5: fix round 1/5 實作完成（commit 4f5a595；session 9 tests；突變 2/2；競態測試需 multi_thread flavor 才有偵測力，已註解）；
  scoped re-review package review-0521bd5..4f5a595.diff，Codex 派出（背景）。
- Task 2.5: fix round 1/5 (2 addressed, 0 open; commits 0521bd5..4f5a595)；Codex approve。
- Task 2.5: complete (commits 73d904a..4f5a595, review clean after 1 fix round)

### Task 2.6

- dispatched implementer (opus) BASE=tasks.md 勾選 commit（見下一行 git log）。
- Ruling: S 管理器的去抖動用「記錄最新目標集合＋單一排程 task（sleep 200 ms 後讀最新集合重開一次）」；重開 task 與新 S reader 都登記進
  shutdown registry（RuntimeEvents drop 一併 abort）；重開失敗走 2.5 的一次性關閉旗標送 Err — 代價：無。
- （補記）tasks.md 2.1–2.5 勾選＋2.5 簽章改寫 commit c9e8b38；Task 2.6 BASE=c9e8b38。
- Task 2.6 實作者（opus）在 RED 階段被 API 用量上限中斷（429，重置 3:30am Asia/Taipei）；工作樹只留下未追蹤的 tests/reopen.rs；
  使用者指示接續，已 resume 同一實作者從 RED 繼續。
- 2.6 DONE（resume 後完成）, commit 81b600a；reopen 10 tests（8 指定＋2 補強）；突變 9 個全命中；cockpit-herdr 34 tests；gate 連跑 3 次綠。
- minor (deferred)：snapshot() 直接覆寫 desired，可能蓋掉去抖動中尚未套用的 L 事件目標（snapshot 比事件舊時），下一個 pane 事件或定期 snapshot 會修正；
  Shutdown.aborts 每次重開 +2 只增不減；重疊測試依賴真實時間（350 ms 觀察點）。
- review package review-c9e8b38..81b600a.diff；Codex 派出（背景）。
- Codex review 2.6：needs-attention，3 findings（high：舊 snapshot 覆寫 L 更新的 desired；high：兩次重開反序完成舊蓋新；medium：abort_all 不設 closing）。
  三項核實成立（finding 1 定期 snapshot 會修但 ≤30 s 漏訂閱，仍要根治）。
- Task 2.6: fix round 1/5 開始（3 open）；修法：generation 序號、提交前驗 gen 與 session、每輪新的 Shutdown 並先設 closing 再掃描；補 3 測試。
- Task 2.6: fix round 1/5 實作完成（commit 237de3d；reopen 13 tests＋Shutdown 單元測試；突變 4/5 命中，F4「提交前驗 session」為無覆蓋的防禦性檢查——
  reset 的 abort_all 讓舊 task 到不了提交，測試構造不出）；scoped re-review package review-81b600a..237de3d.diff，Codex 派出（背景）。
- minor (deferred)：reopen 測試依賴真實時間（1.67 s，餘裕 ≥100 ms），慢 CI 可能 flaky。
- Task 2.6: fix round 1/5 (3 addressed, 0 open; commits 81b600a..237de3d)；Codex approve（session guard 無直接測試，已揭露，不擋）。
- Task 2.6: complete (commits c9e8b38..237de3d, review clean after 1 fix round)

### Task 2.7

- dispatched implementer (opus) BASE=237de3d。
- Ruling: 工廠回 `BuiltRuntime { endpoint: String, runtime: Arc<HerdrRuntime> }`（`HerdrRuntime` 加 `wsl_distro() -> Option<&str>` 供測試觀察有無探測器），
  `cockpit` 端再轉 `Arc<dyn AgentRuntime>` — 比 tuple 清楚 — 代價：無。
- 2.7 DONE, commit 30f27bb；factory 3＋loop_integration 4；突變 7/7；cockpit-herdr 45 tests；gate 連跑 3 次綠。
- minor (deferred)：loop_integration 的 RED 訊號弱（組合既有程式，效力靠突變表）；s_probe_failure 依賴 2.5 s wall-clock；
  BuildError::UnsupportedSocket 在 windows／unix 為死碼。
- review package review-237de3d..30f27bb.diff；Codex 派出（背景）。
- Codex review 2.7：needs-attention，2 medium（完整週期測試只比 id、S 清單只驗 any；退避測試靠 2.5 s 瞬間狀態會 flaky）。核實成立，實作不用動。
- Task 2.7: fix round 1/5 開始（2 open，只改測試）。
- Task 2.7: fix round 1/5 實作完成（commit 6b7d148，只改測試；突變 3/3；連跑 3 次綠）。
- Ruling: 接受實作者偏離「用 translate::snapshot 算期望值」的指示，改手寫 expected_state()——同源期望值讓突變抓不到（實測）—
  代價：fixture 與期望值兩份手寫資料要同步維護。
- scoped re-review package review-30f27bb..6b7d148.diff，Codex 派出（背景）。
- Task 2.7: fix round 1/5 (2 addressed, 0 open; commits 30f27bb..6b7d148)；Codex approve。
- Task 2.7: complete (commits 237de3d..6b7d148, review clean after 1 fix round)
- 第 2 組完成：cockpit-herdr 45 tests；tasks.md 勾選 commit 3dd923a。

### Task 3.1

- dispatched implementer (sonnet) BASE=3dd923a。
- Ruling: 命令列只認 `--config <path>`，手寫解析不引 clap；Config 內的端點型別直接用 cockpit_herdr::HerdrEndpoint；
  listen 允許 127.0.0.0/8 與 ::1；TOML 全部 deny_unknown_fields — 代價：無。
- 3.1 DONE, commit b2ff2aa；12 config tests（10 指定＋2 補 Explicit/Cwd 分支）；突變 12/12；gate 綠；cockpit 無 herdr-client 依賴。
- note：--config 相對路徑不 join cwd（依 brief 字面）；秒數／listen 錯誤不含 id（全域設定）。
- review package review-3dd923a..b2ff2aa.diff；Codex 派出（背景）。
- Codex review 3.1：needs-attention，4 findings（high：相對 --config 未 join 注入 cwd；medium：load 回 tuple 偏離計畫；medium：全域欄位錯誤無 id；
  medium：Path::exists fail-open）。
- Ruling: 全域欄位（server.listen、polling.*）的錯誤以「區段.欄位名」識別，不硬套 runtime id（spec 措辭不精確，已改 spec 與 brief 並 commit）—
  代價：無。
- Ruling: 撤回 load 回 tuple 的裁決，改 `Config.source: ConfigSource` 欄位、load 回 `Result<Config>`（對齊計畫文字）— 代價：無。
- Task 3.1: fix round 1/5 開始（3 open：cwd join、fail-open、load 簽章）。
- Task 3.1: fix round 1/5 實作完成（commit 9bfa949；14 config tests；突變：cwd join KILLED；fail-open 用「吞非 NotFound」突變 KILLED，
  「改回 exists()」對目錄模擬案例 SURVIVED——權限導致 exists() 回 false 的真實情境沒有可攜測試，記為 deferred）；
  scoped re-review package review-b2ff2aa..9bfa949.diff，Codex 派出（背景）。
- Task 3.1: fix round 1/5 (3 addressed, 1 resolved by ruling, 0 open; commits b2ff2aa..9bfa949)；Codex approve。
- Task 3.1: complete (commits 3dd923a..9bfa949, review clean after 1 fix round)
- minor (deferred)：Path::exists() 因權限回 false 的真實情境無可攜測試（實作已改 read_to_string 只認 NotFound）。

### Task 3.2

- dispatched implementer (sonnet) BASE=9bfa949。
- 3.2 DONE, commit 5a0a515；3 tests；突變 3/4（wsl_probe_secs 寫死無法從外部觀察，已註明）；gate 綠；cargo tree 無 herdr-client。
- review package review-9bfa949..5a0a515.diff；Codex 派出（背景）。
- Codex review 3.2：needs-attention，1 high（wsl_probe_secs 轉傳不可觀察，突變寫死 0 仍綠）。核實成立。
- Ruling: `HerdrRuntime::wsl_retry_after()` accessor（cockpit-herdr 加法）＋ `RuntimeEntry.wsl_retry_after` 欄位，測試用 37 s 驗 — 代價：無。
- Task 3.2: fix round 1/5 開始（1 open）。
- Task 3.2: fix round 1/5 實作完成（commit 6e1aedf；4 tests；突變 1/1）；scoped re-review package review-5a0a515..6e1aedf.diff，Codex 派出（背景）。
- Task 3.2: fix round 1/5 (1 addressed, 0 open; commits 5a0a515..6e1aedf)；Codex approve。
- Task 3.2: complete (commits 9bfa949..6e1aedf, review clean after 1 fix round)

### Task 3.3

- dispatched implementer (sonnet) BASE=6e1aedf。
- Ruling: /ws 在 3.3 先掛 501 占位（3.4 實作）；AppState 只含 `watch::Receiver<Arc<ProjectedState>>`；靜態檔用 include_str!／include_bytes!，
  最小版 PNG 為合法 1×1（3.7 換正式圖示）— 代價：無。
- 3.3 DONE, commit 35034bb；4 http tests；突變 4/4；cockpit 22 tests；gate 綠。note：/ws 已掛 501 路由（3.4 要改 route 不是新增）；
  /api/state 用手動 serde_json::to_vec（serde 無 rc feature）。
- review package review-6e1aedf..35034bb.diff；Codex 派出（背景）。
- Codex review 3.3：approve，0 findings。
- Task 3.3: complete (commits 6e1aedf..35034bb, review clean)

### Task 3.4

- dispatched implementer (sonnet) BASE=35034bb。
- Ruling: 允許 cockpit 加 dev-dependency `futures-util`（tokio-tungstenite 的 Stream/Sink trait 在測試端需要；正式依賴不動）— 代價：無。
- Ruling: /ws handler 用 rx.borrow_and_update() 先送現況再等 changed()；客戶端訊息（文字／二進位）一律忽略、Close 結束該連線 — 代價：無。
- 3.4 DONE, commit 7a8e6bc；4 ws tests；突變 4/4；cockpit 25 tests；gate 連跑 3 次綠；dev 加 futures-util（已裁決）。
- review package review-35034bb..7a8e6bc.diff；Codex 派出（背景）。
- Codex review 3.4：approve，0 findings。
- Task 3.4: complete (commits 35034bb..7a8e6bc, review clean)
- tasks.md 3.1–3.4 勾選 commit 6dbae11。

### Task 3.5

- dispatched implementer (sonnet) BASE=6dbae11。
- Ruling: render.js 以純函數 renderState(state) -> DOM 樹，onState 只做「換掉 #app 內容」；channel.js 另暴露 onChannel(status) 給頂列顯示通道狀態；
  ui_preview example 用 watch::channel 直接推 ProjectedState（不經 StoreHandle），每 2 s send_replace 一份改過狀態的複本 — 代價：無。
- 3.5 DONE, commit cd27c92；http 5＋fixture 1；突變 2/2（第一輪發現斷言太鬆已修）；ui_preview 實跑 version 7→12；gate 綠。
- Ruling: spec「未知狀態不破壞畫面」的 whatever 無法經型別化後端送出（core AgentStatus 封閉五值、無 serde(other)），fixture 用 unknown；
  3.6 改在瀏覽器 console 直接 window.onState(改成 whatever 的 state) 驗 render.js 容錯 — 代價：該情境只有手動／瀏覽器工具驗證。
- review package review-6dbae11..cd27c92.diff；Codex 派出（背景）。
- Codex review 3.5：needs-attention，1 medium（ui_preview interval 首 tick 立即觸發，啟動即改 fixture）。核實成立。
- Task 3.5: fix round 1/5 開始（1 open，只改 example）。
- Task 3.5: fix round 1/5 實作完成（commit b5315ad；實測 T+0.8s version 7／T+2.5s version 8）；scoped re-review package review-cd27c92..b5315ad.diff，Codex 派出（背景）。
- Task 3.5: fix round 1/5 (1 addressed, 0 open; commits cd27c92..b5315ad)；Codex approve。
- Task 3.5: complete (commits 6dbae11..b5315ad, review clean after 1 fix round)

### Task 3.6（控制端自行驗證；Chrome 擴充功能未連線，改用 headless Chrome）

- Ruling: 3.6 是驗證不是實作，由控制端用 headless Chrome（--screenshot／--dump-dom／DevTools protocol）執行；發現畫面問題才派 fix — 代價：無。
- headless 結果：截圖 scratchpad/ui/ui-preview.png；DOM：runtime-card 2（connected 1／disconnected 1）、pane-row 3、status-working/done/unknown 色塊、
  exited 1、WSL 原因文字有、無「完成」、頂列 version 與 channel-status=connected、最近事件 3；console 無錯誤；
  harness 餵 whatever → status-unknown、文字 whatever、無 status-whatever class。
- 3.6 通道重連：reconnect-check.js（DevTools protocol）PASS：connected v7 → 停 preview disconnected → 重啟 connected v8。
- 3.6 產出 commit 4cb4740：docs/research/2026-09-15/{ui-preview.png, change-1b-acceptance.md, reconnect-check.js}；tasks.md 3.5、3.6 勾選同 commit；
  markdownlint 0 issue；去識別化 grep 空。review package review-b5315ad..4cb4740.diff。
- Task 3.6 Codex review 派出（背景，審文件與證據）。
- Ruling: 3.7 與 3.6 的 review 並行——3.6 無實作者、fix 頂多改 docs，與 3.7（assets／tests/pwa.rs）不重疊 — 代價：無。

### Task 3.7

- dispatched implementer (sonnet) BASE=4cb4740。
- Codex review 3.6：needs-attention，4 medium（harness 證據不在 repo；無 gate 證據；reconnect 腳本 PASS 條件太弱；taskkill /IM 全域誤殺）。
  四項核實成立。
- Ruling: 3.6 的修正由控制端自己做（動的是控制端寫的驗證腳本與文件，不是產品程式碼；Codex 仍做 scoped re-review）— 代價：
  控制端 context 多一些。腳本改寫完成（reconnect-check.js 強化斷言＋PID 範圍清理；新增 whatever-check.js），
  **執行要等 3.7 完成**（3.7 改 assets 會重編 ui_preview.exe，執行中的 exe 會擋 cargo 寫入）。
- 3.7 DONE, commit 3d1993b；3 pwa tests；突變 4/4；cockpit 31 tests；gate 綠。review package review-4cb4740..3d1993b.diff；Codex 派出（背景）。
- Task 3.6: fix round 1/5 實作完成（控制端；commit f9d542e：whatever-check.js PASS、reconnect-check.js 逐條斷言 PASS、cockpit gate 31 tests＋markdownlint 0）；
  scoped re-review package review-3d1993b..f9d542e.diff。
- Codex review 3.7：needs-attention，1 low（manifest 多 scope 欄位）。
- Ruling: 保留 manifest 的 scope:"/"（dispatch 裁決 1 明文允許、標準 PWA 欄位、與 start_url 一致）；low 屬 Minor 不進 loop — 代價：無。
- Task 3.7: complete (commits 4cb4740..3d1993b, 1 low ruled keep)
- tasks.md 3.5–3.7 勾選 commit db17698。

### Task 3.8

- dispatched implementer (opus) BASE=db17698（與 3.6 複審並行；檔案不重疊）。
- Ruling: main 的優雅結束＝ctrl_c 後 drop 所有驅動器 stop sender（驅動器自行釋放事件流）再結束 serve；port 占用 → anyhow context 含位址與原因，非零結束 — 代價：無。
- Task 3.6: fix round 1/5 (3 addressed, 1 open — 生命週期未完整在 try/finally; commits 4cb4740..f9d542e)
- Task 3.6: fix round 2/5 開始（控制端改兩個腳本：全部 spawn／mkdtemp 移入 try、error handler、finally 依存在性清理並驗證 taskkill）；
  執行與 acceptance.md 更新等 3.8 完成後（避免搶 ui_preview.exe）。
- 3.8 DONE, commit 0fc9714；app 3 tests；cockpit 34 passed／1 ignored；workspace 317 passed／10 ignored；真機 ignored 測試實跑通過（pane 4＝4）；
  release 單一執行檔／port 占用／Ctrl-C 皆實測；`herdr api snapshot` 無旗標、輸出即 JSON（result.snapshot.panes）。
- review package review-db17698..0fc9714.diff；Codex 派出（背景）。
- Task 3.6: fix round 2/5 實作完成（commit 0320241；兩腳本重跑 PASS）；scoped re-review package review-f9d542e..0320241.diff，Codex 派出（背景）。
- 第 4 組前置：Windows HERDR running（0.9.0-preview, protocol 22）；WSL Ubuntu-24.04 測試 server 以 setsid -f 啟動成功（0.8.2, protocol 20）。
- Task 3.6: fix round 2/5 (1 addressed; new finding：文件 commit 歸屬錯; commits f9d542e..0320241)
- Task 3.6: fix round 3/5 實作完成（commit 308a4ec，只改歸屬字句）；docs-only package .superpowers/sdd/tasks/review-0320241..308a4ec-docs.diff，Codex 派出（背景）。
- Codex review 3.8：needs-attention，1 medium（ctrl_c().await.ok() 吞掉註冊錯誤）。核實成立。Task 3.8: fix round 1/5 開始（1 open）。
- Task 3.6: fix round 3/5 (1 addressed, 0 open; commits 0320241..308a4ec)；Codex approve。
- Task 3.6: complete (commits 3d1993b..308a4ec, review clean after 3 fix rounds; 控制端執行)
- 3.8 實作者（opus）在 fix round 1 開頭被用量上限中斷（429，重置 12pm）；工作樹乾淨；13:39 已過重置，使用者指示接續，resume 同一實作者。
- Task 3.8: fix round 1/5 實作完成（resume 後；commit 5b0d163；app 5 tests；突變 2/2；cockpit 36 passed）；scoped re-review package .superpowers/sdd/tasks/review-0fc9714..5b0d163-cockpit.diff，Codex 派出（背景）。
- minor (deferred)：run_with_shutdown「收不到 shutdown 結果」的 bail 分支為防禦碼無測試；stop_senders 測試依賴 Policy 預設退避第一項 1 s。
- Task 3.8: fix round 1/5 (1 addressed, 0 open; commits 0fc9714..5b0d163)；Codex approve。
- Task 3.8: complete (commits db17698..5b0d163, review clean after 1 fix round)
- 第 3 組完成：cockpit 36 tests／1 ignored；workspace 317+ tests；tasks.md 勾選 commit f315de2（24/30）。
- release build target/release/cockpit.exe 完成（含 5b0d163）。

### Task 4.1（控制端執行自動化部分；目視與 PWA 安裝需使用者）

- 前置：Windows HERDR running、WSL 測試 server running、本機 cockpit.toml（win 預設端點＋wsl Ubuntu-24.04）。
- 腳本 docs/research/2026-09-15/attach-check.py（stdlib；uv run --no-project）；ruff 2 errors 待修（subprocess.run 缺 check=）。
- 4.1 自動化部分 PASS（attach-check.py：win／wsl 皆 connected，workspace／pane／agent 集合與 herdr api snapshot 一致；real_attach ignored 測試 ok）；
  已 commit（見 git log）。待使用者：HERDR pane 內啟動目視無新視窗、Chrome 安裝 PWA。
- Ruling: 4.1–4.3 的證據都寫進同一份 acceptance.md，Codex review 在 4.3 做完後對三者一次審（都是驗證紀錄、無產品程式碼）— 代價：
  4.1 的 review 延後到 4.3。

### Task 4.2／4.3／4.4（控制端執行）

- 4.2 WSL 端 live-state-check.py PASS：新 pane 0.11 s 出現；working／blocked／idle 各 ≤0.12 s 顯示且最近事件有 pane.agent_status_changed；
  idle 呈現 idle；釋放後 unknown；關 tab 只有 tab_removed（D5 證實）；無 workspace_updated（D6 證實）；Windows 端全程 connected。
  第一次執行在 200 ms 去抖動窗內回報 → S 尚未重開、狀態經 pane_updated 到達；腳本改等 0.5 s。Windows 端下指令待使用者。
- 4.3 reconnect-real-check.py PASS：首次原因「server is shutting down」（HERDR 優雅關閉時對進行中 snapshot 的回應），之後
  ServerNotRunning 描述；退避 1,2,4,8；Windows 端全程 connected；重啟後 5.3 s 回到 connected 且 pane 集合一致。
- Ruling: 4.3 的「首次原因指出 L 或 S」放寬為「反映對端關閉（L／S 結束或 server 關閉中）」— 真機 HERDR 停止時會先回
  server_unavailable，原因字串更精確且符合 spec「錯誤的描述字串」— 代價：tasks.md 4.3 文字要跟著改（4.5）。
- 4.4：Windows L 串流 20 s 無舊事件但無觸發條件 → 無法判定；cockpit debug：win discarded=0、wsl discarded=2（0.8.2 有補推，
  已被丟棄規則吸收）。結論寫進 acceptance.md，§2.3 加註留 4.5。
- Ruling: 4.4 對 Windows 端定案需要使用者先開關幾個 tab 再跑 capture_events；本次以「無法判定」記錄，不擋 4.5／4.6 — 代價：
  handover 要把它列為下一段的待查證。
- 4.2–4.4 證據 commit 92b5044；tasks.md 勾 4.3、4.4（4.1、4.2 待使用者部分）；docs-only package .superpowers/sdd/tasks/review-f315de2..92b5044-docs.diff，Codex 派出（背景，審 4.1–4.4）。
- Task 4.5 dispatched implementer (sonnet)，與 4.1–4.4 的 docs review 並行（檔案不重疊）。
- Codex review 4.1–4.4（docs-only，package review-f315de2..92b5044-docs.diff）：needs-attention，5 findings：
  (1) high：Scenario A／B 缺真人驗收 → 4.1、4.2 本來就未勾，acceptance.md 已揭露，不動；
  (2) high：4.4 未滿足「Windows 端近期有 tab 開關」前提就勾選、§2.3 回寫延到 4.5；
  (3) high：reconnect-real-check.py 停 server 前沒驗證是測試 server、沒有寫入 opt-in；
  (4) medium：Scenario F 只比 pane id 集合，不足以證明「內容與新 snapshot 一致」；
  (5) medium：live-state-check.py 的 tab.create 回應遺失時會留下測試 tab。
- Ruling: (2) 取消 4.4 勾選，維持「無法判定」的證據但列為待使用者（先開關幾個 Windows tab 再跑 capture_events）；
  §2.3 回寫由 4.5 完成（commit 5bfcb11）— 前提未滿足就宣稱完成會誤導 — 代價：4.4 在分支收尾時仍開著，handover 要帶。
- Ruling: (3)(4)(5) 由控制端自己修（驗證腳本非產品程式碼，沿 3.6 的裁決）：抽共用模組 acceptance_common.py
  （起停 cockpit、snapshot、JSON-RPC、compare_projection 逐欄位比對：workspace／tab／pane／focused，路徑類欄位不印值）；
  reconnect 加三層 guard（HERDR_CLIENT_TEST_ALLOW_WSL_WRITES=1、COCKPIT_ACCEPT_STOP_WSL_DISTRO=`<distro>` 須等於設定、
  herdr status server --json 須 running 且 socket 與設定相同）並檢查 stop／restart 結果；live-state 的 finally 改為
  重新 snapshot、關掉所有帶本腳本 label 且不在 baseline 的 tab、殘留即 FAIL — 代價：三個腳本重跑、acceptance.md 三節重寫。
- 4.1–4.4 fix round 1/5：attach-check PASS（逐欄位一致）；reconnect PASS（guard 負案例：無 opt-in exit 0、distro 不符 exit 2；
  退避 1,2,4,8；重啟後 8.0 s 回 connected；逐欄位一致）；live-state 第一次 FAIL「pane 集合回到 baseline」——
  HERDR 0.8.2 關 tab 後會把 Sidebar pane 換 id（p23→p26，snapshot 證實），且會補推上一輪舊 tab／pane 的事件
  → 多筆 Drift → 重拿；收斂實測 1.9 s（scratchpad converge-check.py），不是等 30 s 定期重拿。
- Ruling: live-state 的收尾斷言改為「35 s 內投影與重新取得的 snapshot 逐欄位一致」並記錄收斂秒數與 Drift 原因；
  不把 Sidebar 換 id 當 cockpit 問題（HERDR 行為，投影正確跟上）— 代價：無。
- minor (deferred)：Drift 觸發的重拿 snapshot 可能比重拿期間已套用的事件舊（WSL 走 wsl.exe+nc，比 L 串流慢），
  會短暫蓋掉較新的焦點／label；目前靠後續事件、Drift 或定期重拿收斂（實測 ≤2 s）。嚴格解法：重拿期間有事件套用就再重拿一次（設上限）。
- 環境筆記：發現一個 01:57 起就在跑的殘留 cockpit.exe（非腳本產生；腳本的 stop_cockpit 都有驗 API 消失），佔住 7770 導致
  腳本「已有服務在跑」；Git Bash 下 taskkill /PID 會被 MSYS 路徑轉換吃掉旗標，要加 MSYS_NO_PATHCONV=1。
- Task 4.5: complete (commit 5bfcb11；設計文件 §2.3／§4.1／§4.2／§6.1／§6.2／§6.4／§7.1／§7.2／§9、CONTEXT.md、AGENTS.md、
  cockpit/README.md、tasks.md 4.3 文字；handover §3 已是 NUL 版未動；markdownlint 37 files 0 issues)。docs review 併入最終全分支 review。
- 4.1–4.4 fix round 1 commits 3088707..7ce7229；scoped re-review package .superpowers/sdd/tasks/review-5bfcb11..7ce7229-docs-fix1.diff，Codex 派出（背景）。
- Task 4.6 gate（2026-09-15）：fmt 0、clippy -D warnings 0、cargo test --workspace 319 passed／0 failed／10 ignored、
  markdownlint 37 files 0 issues、openspec validate 只有 SHALL/MUST 警告（無 ERROR）、去識別化 grep 為空。handover v7 草稿已寫（待填數字）。
- Codex scoped re-review 4.1–4.4 round 1：needs-attention。原 (2)(3) addressed；(1) 維持（使用者項目）；(4)(5) 補：
  新 finding：compare_projection 漏比 exited（medium）；live-state cleanup 的 snapshot 只試一次、回應遺失時沒有 tab_id 可 fallback（medium）；
  reconnect 的 finally 內 ensure_running 拋例外會跳過 stop_cockpit（medium）；acceptance.md 4.4 節仍寫「pane 集合回到 baseline」（low）。
- Ruling: 四條全部成立、全部修（控制端自修）：pane expected 加 exited=False 並做突變證據；marked_tabs 重試 3 次＋已知 tab_id 先直接關＋
  取不到就印人工清理指令並 FAIL；reconnect 巢狀 try/finally＋ensure_running 吞 subprocess 例外回 False；4.4 措辭改逐欄位一致 — 代價：無。
- 4.1–4.4 fix round 2 commits 2ac0e90..753a7a0（三腳本重跑 PASS；exited 突變 KILLED）；package .superpowers/sdd/tasks/review-7ce7229..753a7a0-docs-fix2.diff，Codex 派出（背景）。
- Task 4.6: complete (commit 34dd6bc；gate 全綠、validate 無 ERROR、去識別化空、handover v7)。剩 4.1／4.2／4.4 使用者部分。
- 最終全分支 Codex review 派出（背景；a575842..34dd6bc，59 commits；與 4.1–4.4 round 2 re-review 並行）。
- Codex scoped re-review 4.1–4.4 round 2：approve（a／b／c／d 全部 addressed，無 material findings）。Task 4.1–4.4 自動化部分 complete；4.1／4.2／4.4 的使用者部分留待使用者。
- 最終全分支 Codex review：needs-attention。Part 1：herdr-runtime-session 一項 Misunderstood（初始 S handle 未納入 generation 序列化）；
  其餘 capability 無 Missing／Extra；唯讀與 ADR-0003 皆符合。Part 2：無 Critical；Important×2（F1 初始 S handle 覆蓋較新一代；
  F2 run_with_shutdown 不等 driver／projector）；Important×1（4.1／4.2／4.4 未勾）。Minor：channel.js onopen 即歸零退避、JSON.parse 未隔離。
- Ruling: F1、F2 控制端讀碼核實成立，進最終修正波（一次 dispatch，opus）；4.1／4.2／4.4 是使用者項目，是否擋 merge 交使用者決定；
  兩條 Minor 記 deferred（channel.js 的退避歸零在真實 server 合約下不會反覆斷線；JSON.parse 只吃自家 server 的合法 JSON）— 代價：無。
- 最終修正波 BASE 34dd6bc；brief .superpowers/sdd/tasks/final-fix-wave-brief.md；dispatched implementer (opus)。
- 最終修正波 DONE：069ac89（F1）、198d397（F2）；workspace 0 failed；突變 4 組；package .superpowers/sdd/tasks/review-34dd6bc..198d397-final-fix.diff，Codex scoped re-review 派出（背景）。
- Ruling: 接受 F2 測試 (a) 偏離 brief（用停止後慢 300 ms 的合成 driver task 取代真 driver::run）— 實作者實測真驅動器停得太快、
  突變殺不掉；測試目標是「cleanup 有 await drivers」，真驅動器會停由既有兩條測試覆蓋 — 代價：(a) 不驗真驅動器與 cleanup 的整合，
  但 (b) 與既有測試補上。10 s×N 逐一 timeout 記 deferred（目前最多兩筆 runtime）。
- 修正波後全 gate（198d397）：fmt 0、clippy 0、cargo test --workspace 326 passed／0 failed／10 ignored、markdownlint 37 files 0；
  release 重建並重跑 4.1／4.2／4.3 腳本皆 PASS（4.2 收斂 3.0 s、4.3 回 connected 7.3 s）。
- Codex scoped re-review 最終修正波 round 1：needs-attention。F1 addressed；F2 部分 addressed——逾時分支只 abort 不 await JoinHandle，
  「回傳前真的收乾淨」在逾時路徑不成立（high）。其餘（cleanup 三種 return 路徑、10 s 揭露、合成 driver 驗到逐一 await、(b) 不 flaky）通過。
- Ruling: 成立，進 fix round 2（resume 同一實作者）：timeout(&mut driver) 逾時後 driver.abort() 再 await；shutdown_components 的逾時改為參數
  （run_with_shutdown 傳常數），測試用 100 ms 逾時＋不理停止訊號、帶 Drop guard 的 driver task，斷言回傳時 guard 已觸發 — 代價：無。
- 最終修正波 fix round 2 DONE：commit d58ea18（timeout(&mut driver) → abort → await；shutdown_components 逾時改參數並公開；新測試
  shutdown_components_waits_for_aborted_driver_to_actually_drop，RED＋突變各一次）；cockpit 40 passed；workspace 0 failed。
- Ruling: 接受 shutdown_components／DRIVER_SHUTDOWN_TIMEOUT 公開（測試與嵌入者可自訂逾時；doc 已寫明副作用）；
  「逾時後 await 無上限」記 deferred（驅動器目前沒有阻塞 Drop）— 代價：無。package .superpowers/sdd/tasks/review-198d397..d58ea18-final-fix2.diff，Codex 派出（背景）。
- fix round 2 後全 gate（d58ea18）：fmt 0、clippy 0、cargo test --workspace 327 passed／0 failed／10 ignored、markdownlint 37 files 0、validate 0 ERROR；release 重建、4.1／4.2／4.3 重跑 PASS。
- Codex scoped re-review 最終修正波 round 2：approve（逾時路徑 abort 後 await；所有 return 路徑在 driver future drop 後才返回；測試對
  current-thread runtime 的依賴只會退化成假性通過、不誤報；公開 API 風險已在 doc 揭露）。最終全分支 review 收斂：clean。
- 收尾狀態：分支 feat/attach-herdr-runtimes HEAD d58ea18（＋文件 commit）；併回 main 與 archive 待使用者決定；4.1／4.2／4.4 使用者部分待做。
- 使用者決定（2026-09-15，遠端）：現在 squash 併回 main 並 archive（三項未勾照實帶警告）；4.1／4.2／4.4 維持未勾不放寬；
  deferred minor 全部帶進 change 2 待辦；change 2 先口頭討論範圍、併回後再 propose。
