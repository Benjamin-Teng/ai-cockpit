# SDD ledger — plan: openspec/changes/live-output/tasks.md

Spec（binding）：`openspec/changes/live-output/specs/*/spec.md`；design：`openspec/changes/live-output/design.md`。
分支 `change-3-live-output`，MERGE_BASE＝`58f0e16`（main）。artifacts commit `f204721`。
Reviewer：Codex adversarial-review（`node ~/.claude/plugins/cache/openai-codex/codex/1.0.6/scripts/codex-companion.mjs adversarial-review --wait --base <BASE> "<focus>"`；focus 不放反引號；
看 log 有無 `usage limit`／`Turn failed`）。repo 的 stop review gate 未啟用（2026-09-19 setup --json）。
Implementer：sonnet（卡住升 opus）。`task-brief` 腳本不認 OpenSpec checkbox 格式 → brief 由控制端手抽到 `task-<N>-brief.md`。

## Preflight 衝突掃描

| 對象 | 產出 vs 消費 | 發現 |
|---|---|---|
| 1.2 → 1.3 | fixture `pane-read-p<protocol>.json` → 合約測試讀它 | 一致（1.3 指回 1.2 的檔名） |
| 2.1 → 2.2 | `PaneOutput`／`OutputFormat`／`PaneNotFound` → trait 方法簽名 | 一致 |
| 2.2 ↔ 3.1 | 2.2 實作 HerdrRuntime 成功路徑；3.1 才寫它的測試 | **衝突**：2.2 的 HERDR 實作沒有自己的 red→green。見 Ruling R1 |
| 2.2 → 3.2 | 成功路徑 → 錯誤對應 | 一致（2.2 階段錯誤一律 Failed，3.2 加 pane_not_found） |
| 3.1／3.2 ↔ 3.3 | 共用 `cockpit-herdr/tests/read_output.rs`、3.3 另動 `herdr-client/src/testing/` | 循序，無衝突 |
| 3.4 | 動 `cockpit-herdr/tests/session.rs` | 與 3.1–3.3 不同檔 |
| 4.1 → 4.2 | `AppState.runtimes` → handler 查表 | 一致 |
| 4.2 ↔ 4.3 | 4.2 做端點、4.3 才掛來源檢查 | **衝突**：中間 commit 的端點沒有來源檢查；4.3 還得回頭改 4.2 測試的 Host。見 R2 |
| 4.1／4.4 | 都是 cockpit 的小型接線（AppState 欄位、`/app/output.js` 路由） | 同形小改，可併。見 R3 |
| 4.2 ↔ cockpit-core FakeRuntime | cockpit 的測試拿不到 `cockpit-core/tests/common` | cockpit 要自備假 runtime（`cockpit/tests/` 內）；非衝突，dispatch 時交代 |
| 4.2 → 5.1 | 「與正式服務相同的輸出 handler」→ ui_preview 要拿得到 | handler／路由要能被 example 重用；dispatch 5.1 時交代，必要時 4.2 把它做成 pub |
| 4.4 → 5.2 | 佔位 `output.js` → 真正內容 | 一致 |
| 5.2 → 5.3 → 5.4 → 5.5 | 共用 `output.js` 與 `live-output-check.js`（分段追加） | 循序；介面＝design D8 的 `window.liveOutput` |
| 5.3 ↔ 既有 CDP 腳本 | 動 `render.js`／`actions.js` | 5.3 已要求重跑 `actions-check.js`、`factory-floor-check.js` |
| 6.1 ↔ 6.2 | 同一份驗收文件 | 6.2 需要使用者本人 → 見 R5 |
| 7.3 ↔ 8.x | handover 描述最終狀態，但排在 8 之前 | 見 R4 |
| 8.2 ↔ SDD final review | 同一件事 | 8.2 即 SDD 的 whole-branch review |

每個 task 自身一致性：1.1 無程式碼（控制端自己跑）；1.2 純真機操作；其餘 task 的測試名與所述行為相符，
未見「斷言空無一物的測試」或強制逐字複製邏輯。3.3 明示不得用 `start_paused`，與 design D9 一致。

## Rulings

- Ruling R1：2.2 與 3.1 併成一個 dispatch 與一次 review（3.1 的兩個測試就是 2.2 HERDR 實作的 red→green）— tasks 把實作與其測試拆到兩個 task，TDD 下不成立 — 錯了的代價：一次 review 的 diff 較大。
- Ruling R2：4.2 與 4.3 併成一個 dispatch 與一次 review — pane 內容端點不該有「沒有來源檢查」的中間 commit，且 4.3 必然回改 4.2 的測試 — 代價：安全關鍵的 diff 集中在一次 review，focus 要寫清楚兩個 spec requirement。
- Ruling R3：4.1 與 4.4 併成一個 dispatch（同為 cockpit 接線小改）— skill「batch small same-shape work」— 代價：無。
- Ruling R4：7.3（重寫 handover）移到 8.2 之後最後做 — handover 要描述最終狀態（含 review 結果與坑）— 代價：8.2 的 Codex review 不含 handover 的 diff（文件，非 diff 審查對象，另由 Claude subagent read-back）。
- Ruling R5：6.2（使用者手動驗收）移到最後，與完成報告一起交給使用者 — 它需要使用者本人，不該卡住其餘 task — 代價：若使用者驗收發現問題，要在 final review 之後再開修正。
- Ruling R6：1.1、1.2 由控制端自己做，不派 implementer — 1.1 是跑一個指令記數字；1.2 是與本 session 稍早探測相同的真機操作（WSL 寫入已由使用者於 2026-09-19 同意、tasks.md 明列 opt-in），無程式碼、無 review 面 — 代價：控制端 context 多幾百行輸出。1.2 的 fixture 併入 1.3 的 review。
- Ruling R7：7.1＋7.2（純文件）不送 Codex，改派 fresh Claude subagent 做內容 read-back — CLAUDE.md：Codex 管 diff 審查，非 diff 的文件內容正確性歸 Claude subagent — 代價：無。

## 執行單元（依序）

U1＝1.1（控制端）｜U2＝1.2（控制端）＋1.3｜U3＝2.1｜U4＝2.2＋3.1｜U5＝3.2｜U6＝3.3｜U7＝3.4｜U8＝4.1＋4.4｜
U9＝4.2＋4.3｜U10＝5.1｜U11＝5.2｜U12＝5.3｜U13＝5.4｜U14＝5.5｜U15＝6.1｜U16＝7.1＋7.2｜U17＝8.1｜U18＝8.2（final）｜
U19＝7.3｜U20＝6.2（使用者）

## Progress

Task 1.1: complete (baseline at f204721: passed=494 failed=0 ignored=10; controller-run, no diff)
Task 1.2: complete (controller-run; fixture herdr-client/tests/fixtures/pane-read-p20.json uncommitted→goes with 1.3; WSL 0.8.2 pane_not_found verified, research note §5 updated; tab closed, server stopped)
Task 1.3: implemented (commits 46328b5..938cd23; implementer DONE; report task-1.3-report.md) — REVIEW PENDING
BLOCKER 2026-09-19：Codex usage limit（log：`You've hit your usage limit … try again at Sep 20th, 2026 9:58 AM`／`Turn failed`）。task 1.3 的 review 未執行。已停下來問使用者怎麼走。
USER DECISION 2026-09-19（Codex 額度用盡）：繼續實作；每單元由 fresh Claude subagent 做「規格對照檢查」（非 diff review：只驗有沒有照 spec／brief 做、測試是否真的驗到行為）；tasks.md 一律不打勾、ledger 標 REVIEW PENDING；2026-09-20 09:58 後依 crate 分組跑 Codex adversarial review（預計分組：G1 herdr-client＋cockpit-core＝1.3、2.1、2.2；G2 cockpit-herdr＝3.1–3.4；G3 cockpit 後端＝4.1–4.4；G4 前端＋ui_preview＝5.1–5.5；G5 whole-branch＝8.2），findings 修完才打勾、才宣稱完成。

- Ruling R8：規格對照檢查與下一單元的實作者並行派出（檢查對象是固定 commit 範圍，不受後續 commit 影響）；檢查出的問題等當前實作者回報後再續派原實作者修，不同時跑兩個實作者 — 省牆鐘時間 — 代價：問題晚一個單元才修，若下一單元建在有問題的介面上要一起改。

Task 1.3: spec-check ✅ (sonnet, no findings; commits 46328b5..938cd23) — CODEX REVIEW PENDING (G1)
Task 2.1: implemented (commits 938cd23..64f80b2; DONE; concern: PaneNotFound Display 用中文，比照 Rejection 慣例) — spec-check dispatched
Task 2.1: spec-check ✅ (sonnet, no findings; 中文 Display 與 cockpit-core 慣例一致) — CODEX REVIEW PENDING (G1)
Task 2.2+3.1: implemented (commits 64f80b2..add23dc = d5263ba, add23dc; DONE; workspace 501/0/10 controller-verified) — spec-check dispatched
Task 2.2+3.1: spec-check ✅ (sonnet, no findings; strip_ansi 驗的是假 HERDR 實收 request 行；未用 revision；無 Cargo.toml 異動) — CODEX REVIEW PENDING (2.2→G1, 3.1→G2)
Task 3.2: implemented (commits add23dc..9768519; DONE; 4 tests incl. unparseable_response_is_failed) — spec-check dispatched
Task 3.2: spec-check ✅ (sonnet; 0 Critical/Important) — CODEX REVIEW PENDING (G2)
Task 3.2: minor (routed→3.4): spec「讀取輸出不做 WSL 探測」沒有以 wsl 型 runtime（Some(WslProbe)）的測試佐證，只靠讀碼。

- Ruling R9：上述 minor 不留到 final triage，併入 task 3.4 的 dispatch（同 crate、純測試、成本低）— spec 本文的句子應有測試 — 代價：3.4 範圍略大於 tasks.md 原文。
Task 3.3: implemented (commits 9768519..bdad407; DONE; RED=max concurrency 4 before lock, GREEN after; herdr-client 鷹架 +4 tests) — spec-check dispatched
Task 3.3: spec-check ✅ (sonnet, no findings; guard 具名持有整段、snapshot/subscribe 不取鎖、tokio Mutex、無 spawn、並發在假 HERDR 端原子計數) — CODEX REVIEW PENDING (G2)
Task 3.4: implemented (commits bdad407..d7a4047; DONE_WITH_CONCERNS; 只動 tests/，產品碼 diff 為空——控制端已核) — spec-check dispatched
- Ruling R10：tasks.md 3.4 把既有 method 集合測試的位置寫成 session.rs，實際在 reopen.rs（控制端寫 tasks 時記錯）；實作者就地在 reopen.rs 改名並加新測試，「不干擾事件流」照 brief 放 session.rs — 接受，tasks.md 原文已由控制端更正 — 代價：無。
Task 3.4: spec-check ✅ (sonnet, no findings; 既有 method 集合斷言未削弱；cockpit-herdr 測試連跑兩次一致、無 flaky) — CODEX REVIEW PENDING (G2)。G2（cockpit-herdr：3.1–3.4）實作全數完成。
Task 4.1+4.4: implemented (commits d7a4047..767bbb2 = 1a200af, 767bbb2; DONE; 收尾路徑查證：多一份 Arc 不影響 shutdown) — spec-check dispatched
Task 4.1+4.4: spec-check ✅ (sonnet, no findings; 進表的 Arc 與交給 driver 的是同一實例；測試走真正的 build_components；shutdown 不看 strong_count；output.js 只有註解、404 未放寬) — CODEX REVIEW PENDING (G3)
INTERRUPTED 2026-09-19 夜：task 4.2+4.3 實作者因 Claude session limit（HTTP 429）中途終止；工作樹留有未 commit 的半成品（cockpit/src/http.rs +117、source_check.rs 註解、cockpit/tests/output_endpoint.rs 未追蹤；測試尚未跑、無 report）。BASE 仍為 767bbb2（其後只有控制端的 tasks.md 簿記 commit）。
RESUMED 2026-09-20 16:32：使用者指示從中斷處繼續。Codex 額度應已恢復（原訂 09:58）。
CODEX G1 2026-09-20：range 46328b5..d5263ba → Verdict approve, no material findings（log 26 行、實際執行指令、無 usage limit）。
Task 1.3: complete (commits 46328b5..938cd23, spec-check ✅ + Codex G1 clean)
Task 2.1: complete (commits 938cd23..64f80b2, spec-check ✅ + Codex G1 clean)
Task 2.2: complete (commit d5263ba, spec-check ✅ + Codex G1 clean)
Task 4.2+4.3: implemented (commits 00dd646..e16f8c9 = e16f8c9 單一 commit；DONE_WITH_CONCERNS；cockpit 148/0/1、output_endpoint 13/13) — opus spec-check dispatched
- Ruling R11：`cockpit` 加 `async-trait = "0.1.92"` 為 **dev-dependency**，測試與 example 的假 runtime 改用 `#[async_trait::async_trait]` 巨集，移除手寫展開簽章 — 它已在 Cargo.lock（cockpit-core／cockpit-herdr／herdr-client 都在用同一版），不下載任何新套件、不改產品依賴圖；手寫展開綁死巨集內部形狀（`'life0`／`'async_trait`），且 task 5.1 還要在 example 再寫一個實作 — 代價：共用守則「不新增任何 crate 依賴」對此放寬一次；若使用者不同意，改回手寫展開即可（僅測試碼）。併入 4.2+4.3 的 fix round 與 spec-check findings 一起處理。
CODEX G2 2026-09-20：range d5263ba..d7a4047 → Verdict approve, no material findings（log 46 行、無 usage limit）。
Task 3.1: complete (commit add23dc, spec-check ✅ + Codex G2 clean)
Task 3.2: complete (commits add23dc..9768519, spec-check ✅ + Codex G2 clean; minor→3.4 已處理)
Task 3.3: complete (commits 9768519..bdad407, spec-check ✅ + Codex G2 clean)
Task 3.4: complete (commits bdad407..d7a4047, spec-check ✅ + Codex G2 clean)
Task 4.2+4.3: opus spec-check → SPEC ✅、0 Critical、1 Important、5 Minor（HEAD 經 source_check 已由 axum 原始碼確認；source_check.rs 只動註解；無繞過路徑）
- Ruling R12：spec `live-output`「輸出讀取端點」加嚴——no-store／nosniff 擴及該端點所有回應、405 本體明訂為 {"error":…}、新增情境「錯誤回應也不可快取」— 404／405 啟發式可快取、錯誤本體反射路徑片段；屬既有安全意圖內的加嚴 — 代價：spec 比使用者核可時多一條情境（已 commit，收尾時列出）。
- Ruling R13：5 個 Minor 不延到 final triage，全部併入 fix round 1（安全關鍵端點、每項成本低）；commit message「14 個測試」實為 13，不改寫歷史，僅記錄。
Task 4.2+4.3: fix round 1/5 (7 findings + R11 + R12 addressed per implementer; commits e16f8c9..5ff0800; controller-verified: Cargo.lock 無新 package、source_check.rs 僅註解、無手寫展開殘留；cockpit 151/0/1) — Codex G3 dispatched（取代 Claude scoped re-review：Codex 即權威 diff 審查，fix diff 在其範圍內）
CODEX G3 2026-09-20：range d7a4047..5ff0800 → Verdict needs-attention（log 32 行、無 usage limit）。1 finding：
  [medium] cockpit/src/http.rs:399-402 — 無效 UTF-8 路徑（例 /panes/%FF/output）由 axum Path extractor 在 handler 前回純文字 400：缺 no-store／nosniff、本體非 {"error":…}；「怪異 pane id」測試只驗 4xx 所以沒抓到。
  處置：尚未實測重現（CLAUDE.md：Codex findings 須重現才採信）→ 併入 fix round 2 的 RED：先寫 %FF 測試，紅了才修；不紅則回報 finding 不成立。排在 task 5.1 實作者回報之後（同 crate，不並行兩個實作者）。
Task 5.1: implemented (commits 5ff0800..84e225d; DONE; 實跑驗收六種模式＋VANISH_PANE；疑慮：預設 notfound 的 wJ:p2 為 exited 不可點，UI 情境要用 COCKPIT_PREVIEW_OUTPUT_MODES 疊到可點的 pane) — spec-check dispatched
Task 5.1: spec-check ✅ (sonnet; 16 個前端情境皆有可行觸發路徑；走真正的 router()＋source_check；port 回填無空窗；0 Critical/Important) — CODEX REVIEW PENDING (G4)
Task 5.1: minor (routed→5.2)：OUTPUT_MODES／VANISH_PANE 不驗 pane id 是否存在於 fixture，打錯 id 會靜默套用預設行為→驗收腳本可能假綠；parse 的 delay:<非數字>／fail:<非數字>／vanish 缺 `=` 無單元測試。
Task 5.1: minor (carried in briefs 5.2–5.5)：html／notfound 模式預設不在可點選的 pane 上，UI 點擊情境要用 COCKPIT_PREVIEW_OUTPUT_MODES 疊到 wJ:p1／wJ:p3；ticker 行號基準是 runtime 建立時間（非第一次讀取），腳本只能做相對斷言。
- Ruling R14：5.1 的 pane-id 驗證與解析測試缺口併入 task 5.2 的 dispatch（5.2 是第一個重度使用替身的人；靜默套用預設會讓它自己的驗收假綠）— 代價：5.2 多一個小 commit。
Task 4.2+4.3: fix round 2/5 (Codex finding 經 RED 重現「成立」：%FF → text/plain 400、無安全標頭；修為 PathRejection→error_response(400)；路由全部回應來源列表核對；空 pane 段為 matchit 合法片段，前輪假設更正；commits 5ff0800..efcae8e 中的 efcae8e；cockpit 152/0/1、output_endpoint 17/17) — Codex scoped re-review dispatched
CODEX G3 re-review 2026-09-20：commit efcae8e → Verdict approve；原 finding ADDRESSED（cockpit/src/http.rs:414-425）；no new breakage（log 16 行、無 usage limit）。
Task 4.2+4.3: minor (deferred)：空 pane 段（/panes//output）會命中路由並把空 pane id 送到 runtime.read_output（早於 efcae8e 即如此）；HERDR 回 pane_not_found → 404，無害；final review 分流時決定是否在 handler 先擋。
Task 4.1: complete (commit 1a200af, spec-check ✅ + Codex G3 clean)
Task 4.4: complete (commit 767bbb2, spec-check ✅ + Codex G3 clean)
Task 4.2: complete (commits e16f8c9, 5ff0800, efcae8e; opus spec-check ✅; fix rounds 2/5; Codex G3 finding addressed, re-review clean)
Task 4.3: complete (同 4.2，同一組 commit)
Task 5.2: implemented (commits efcae8e..1e4151e = 0be71f0 R14, 1e4151e; DONE; CDP 第一段 4 情境 RED→GREEN；既有三支 CDP 重跑 PASS；疑慮：include_str! 需重 build、cockpitActions.clearSelected 待 5.3 補) — spec-check dispatched
Task 5.2: spec-check ✅ (sonnet; 0 Critical/Important；四條競態路徑推演無雙鏈無漏排；全檔僅 textContent；貼底在寫入前量測；R14 的 pane 清單取自實際 fixture) — CODEX REVIEW PENDING (G4)
Task 5.2: minor (deferred)：ui_preview 的 TcpListener::bind 早於 validate_known_panes()，驗證失敗前會短暫佔埠；功能上仍是「啟動失敗＋清楚訊息」。
Task 5.2: note：檢查者未實跑 CDP（5.3 實作者正使用同一環境）；5.3–5.5 每次都會重跑整支 live-output-check.js，第一段的實跑佐證由後續 task 的輸出提供。
Task 5.3: implemented (commits 1e4151e..3001c88; DONE；CDP 第一＋二段 PASS、既有兩支 CDP PASS) — spec-check dispatched
Task 5.3: controller findings（併入 fix round 1）：(a) 面板打開時 CDP 點擊畫面下半部的 #app 元素，pointerdown 完全沒送達，根因未查，實作者以「挪到面板關閉時做」繞過——可能是真實的使用者可見缺陷（sticky 面板遮擋），須查根因；(b) 情境 D「面板捲動位置不變」量到 before 0→after 0，無法區分「沒變」與「被重設為 0」，須改用中段非零值。
Task 5.3: spec-check → SPEC ✅、0 Critical、2 Important（scrollTop 基準 0 無辨識力；鍵盤可及性無自動化驗證）、2 Minor（改綁模式下點列非按鈕區會觸發選取且 hover 樣式蓋掉 bind-target 底色；gate 用 -p cockpit 而非 --workspace）。檢查者未獨立調查點擊失效根因（同意實作者「腳本環境細節」之說）——控制端不採納，仍要求查根因。
- Ruling R15：spec「選定一個 pane」加嚴（commit 見 git log）：改綁模式期間 pane 列不可點選、既有選取保留；面板打開時頁面其餘操作必須仍可操作；pane 列須可鍵盤選定；新增三個情境 — 改綁是模態操作，列同時是選取目標會誤觸且樣式互蓋；「點不到」若為真即產品缺陷，須有情境釘住 — 代價：spec 比使用者核可時多三個情境；5.3 需一輪修正。
Task 5.3: fix round 1/5 (R15＋三個新情境＋scrollTop 中段值＋workspace gate＋額外修正「取消選取」計數時序；根因結論「腳本問題」：scrollIntoView 後緊接 Input.dispatchMouseEvent 的時序縫隙，elementsFromPoint 疊層無 #output、與 push 間隔／DOM 結構無關、不加延遲 1/20 失敗、加 100ms 後 32/32 成功——控制端抽查 report 後採信；commits 3001c88..2d8e528 中的 2d8e528；live-output-check.js A–E 三次重跑 PASS) — scoped re-check dispatched
Task 5.3: note→handover 第 4 節：CDP 腳本在 scrollIntoView 之後要 settle 再送合成滑鼠事件；factory-floor-check.js 的 --dump-dom 對殘留程序／CPU 敏感，會環境性 flake。
Task 5.3: scoped re-check → 5/5 findings ADDRESSED、無新破壞（「取消選取」時序修正判定為合理：clear 後 current=null，已在飛請求落地走世代不符分支、maybeStartImmediatePoll 不會再排）— CODEX REVIEW PENDING (G4)
Task 5.3: minor (routed→5.5)：情境「面板打開時仍可操作頁面下方的內容」的 GIVEN 以 belowFold||overlapsPanel 判定，實測走的是 belowFold——沒有真的驗到「被面板遮住」；spec 本文「『畫面操作』的所有按鈕必須仍可操作」只驗了 pane 列／bind-here／鍵盤，未驗 task 操作按鈕。
Task 5.3: minor (deferred)：.selected 與 .bind-target 同列疊加時的視覺結果無斷言（style.css 既有行為）；根因調查的統計力偏弱（0.95^32≈0.19），結論以機制解釋＋elementsFromPoint 疊層為主要依據。
- Ruling R16：上述 routed minor 併入 task 5.5 的 dispatch（同一支 CDP 腳本的最後一個前端 task）— spec 的 GIVEN 應被精確滿足 — 代價：5.5 多兩個斷言。
Task 5.4: implemented (commits 2d8e528..7f5d794；只動 live-output-check.js，新增 F/G/H/I；output.js 無缺陷；四情境各有 RED；「貼底」改用 ticker＋矮視窗因 long 截斷後 scrollHeight 不變無辨識力) — spec-check dispatched
Task 5.4: controller finding（routed→5.5）：D／E 段偶發 FAIL 的訊息是「找不到可點的元素」「逾時：選取」——不是滑鼠命中時序（實作者歸因有誤），是 CDP.click 開頭 querySelector 為 null 即判 FAIL、沒有等元素出現（頁面剛載入、首份投影未畫出）。產品端事件掛在 pointerdown，不受重畫換節點影響（控制端已核 actions.js:195）。
- Ruling R17：CDP.click 加「等元素出現」（輪詢至逾時）併入 task 5.5；5.5 驗收加「整支腳本連續 5 次全 PASS」作為無 flake 的證據 — 會 flake 的驗收腳本讓 gate 失去可信度 — 代價：5.5 的驗收時間變長（每輪約數分鐘）。
Task 5.4: spec-check ✅ (sonnet; 0 Critical/Important；F 的「任一 ≥3s 視窗內最大行號必前進」與 spec 3 秒門檻等價（ticker 確定性每秒一行）；G 以 MutationObserver 三層斷言且以 preview 記錄行佐證 A 在飛；H／I 的 RED 可信；A–E 段斷言未被觸碰) — CODEX REVIEW PENDING (G4)
Task 5.4: minor (deferred)：I 段的 gap<6 複製了 output.js 的 NEAR_BOTTOM_PX，產品改門檻時測試不會自動跟上（黑箱測試的取捨）。
Task 5.5: implemented (commits 7f5d794..e540025 = f32e706 R17, e540025 呈現＋R16；DONE_WITH_CONCERNS；CDP A–M 連續 5 次 PASS（118–120s/次）、actions 72/72、factory-floor 105/105、workspace gate 乾淨) — spec-check dispatched
Task 5.5: controller finding（待併入 fix round）：preview 的 `fail:<k>` 從第一次讀取就失敗，無法滿足 spec「runtime 斷線後恢復」的 GIVEN（已選定且面板有內容）；實作者以假 fetch（M 段）補窄邊界。應擴充替身（先成功 n 次再失敗 k 次）讓主情境走真端點。
Task 5.5: note：commit f32e706 主旨手誤「再確元座標」，不改寫歷史；NETWORK_ERROR_TEXT 與 `"HTTP <status>"` 為實作自訂字串（spec 未指定逐字）。
NOTE 2026-09-20：repo 根出現雜散檔 C:Users…scratchpadfix_diff.txt（5.3 複檢 agent 以未加引號的反斜線路徑重導向 git show 所致；bash 吃掉反斜線→落在 cwd）。未曾被 commit；已刪除；兩份共用守則已加「路徑寫法」一節。→handover 第 4 節。
Task 5.5: spec-check ✅ (sonnet; 0 Critical/Important；三方競態皆擋下、503 重試維持 1 秒節奏、clearFailure 獨立於相同內容 early-return、R16 GIVEN 已收緊、R17 重試逾額為 FAIL) — CODEX REVIEW PENDING (G4)
Task 5.5: minor (deferred)：report 的 markdownlint 段落是空承諾（report 檔在 git-ignored 工作區，不影響 repo）。
- Ruling R18：不為「runtime 斷線後恢復」的 GIVEN（面板已有內容）擴充 preview 替身；L 段（真端點、無先前內容）＋M 段（真瀏覽器、假 fetch、有先前內容且恢復時文字相同）已覆蓋 output.js 的行為；GIVEN 完整的端到端驗證改放 task 6.1 真機驗收（面板有內容時停 WSL 測試 server → 503 標過期＋原因 → 重啟 → 恢復並清除）— 真機驗證比擴充替身更接近 spec 的情境、且不增加替身複雜度 — 代價：若 6.1 因環境因素做不了這一步，這個 GIVEN 只剩 L＋M 的組合覆蓋。
Task 7.1+7.2: implemented (commits e540025..4b69313 = 1eee01f, 4b69313；DONE；markdownlint 72 files 0 issues；README 實際位於 cockpit/README.md) — Claude read-back dispatched（R7：純文件不送 Codex）
CODEX G4 2026-09-20：paths cockpit/assets, cockpit/examples, live-output-check.js over 5ff0800..e540025 → Verdict needs-attention（log 40 行、無 usage limit）。2 findings：
  [high] cockpit/assets/app/output.js:360-369 — select() 在有請求在飛時不發新請求、等舊請求結束；舊 pane 慢（≤5s 服務端逾時）時切到健康 pane 仍 >3s 無內容；fetch 永久 pending 則輪詢永遠停住。世代序號擋不了這個阻塞。
  [medium] cockpit/assets/app/actions.js:135-141 — perform() 對所有 action 遞增 latestOp 並清 ui.error；新增的 select-pane／select-bound-pane 會讓進行中的 POST／PUT 變 stale，其後失敗被 showError 忽略 → 寫入失敗被靜默吞掉。
  控制端讀碼：兩者機制皆在程式碼中確認存在；仍須以 RED 重現後才修。
- Ruling R19：選取（select-pane／select-bound-pane）不是「畫面操作」：不遞增 latestOp、不清 ui.error；既有 rebind／rebind-cancel 的行為（change 2）不動 — 主規格「畫面操作」的錯誤訊息語意針對寫入操作；觀看輸出不該讓寫入失敗消失 — 代價：使用者選取 pane 後，先前的錯誤訊息仍留在畫面上直到下一次寫入操作或手動關閉。
- Ruling R20：output.js 改用 per-generation AbortController：select／clear／markGone 時 abort 舊請求並立即對新選取發請求；另加前端逾時（6 秒，略大於服務端 5 秒）abort 以防 fetch 永久 pending；「同時至多一個進行中請求」仍成立（abort 後才發）— 對應 spec「3 秒內反映」與「舊回應不蓋掉新選取」— 代價：design D8 原寫「不設逾時也可以」，此處加嚴；design.md 同步更新。
G4 fix wave：單一 fix dispatch（fresh sonnet；原 5.2／5.3／5.5 實作者 context 已各用 26–55 萬 token，續派風險高）。
Task 7.1+7.2: Claude read-back → 10/12 通過；2 高：(1) cockpit/README.md:174-176 狀態碼清單漏 400；(2) cockpit/README.md:182-183 安全說明過度承諾（暗示伺服器擋掉跨站請求，與 design D7 矛盾）。另指出 spec／design 也沒有 400。
- Ruling R21：spec「輸出讀取端點」補 400（本文、標頭清單、新情境「路徑不合法」）、design D6 表補一列 — fix round 2 當時只改程式未回寫規格，屬控制端疏漏 — 代價：無（程式與測試已符合）。README 兩點續派文件實作者修。
Task 7.1+7.2: fix round 1/5 (2 addressed, 0 open — README 補 400、安全說明改為準確版；commit ab0406e，path-scoped)；控制端自行複檢 9 行 diff：兩項皆 ADDRESSED、無過度承諾殘留。
Task 7.1: complete (commit 1eee01f, Claude read-back ✅ — R7：純文件不送 Codex)
Task 7.2: complete (commits 4b69313, ab0406e; Claude read-back 2 findings → fix round 1 → controller re-check clean)
INTERRUPTED 2026-09-20 晚：G4 fix wave 實作者因 Claude session limit（HTTP 429，21:30 重置）終止；工作樹乾淨、無 commit、無 report（中斷在讀檔分析階段，無半成品）。
RESUMED 2026-09-20 22:24：使用者指示接續；續派同一 agent。
G4 fix wave: 兩個 Codex findings 皆經 RED 重現「成立」並修正（commits ab4f0da R19、34e2fb5 R20、518f9e4 design D8；live-output-check.js A–P 連續 3 次 PASS（各 135s）、actions／factory-floor 無回歸、workspace gate 乾淨、markdownlint 72/0）；疑慮：REQUEST_TIMEOUT_MS=6000 與逾時原因字串為自訂值；G 段在 abort 機制下主要測到 abort 路徑、世代序號路徑的覆蓋變窄 — Codex scoped re-review dispatched
CODEX G4 re-review 2026-09-20：commits ab4f0da, 34e2fb5 → 原 Finding 1 ADDRESSED（output.js:293）、Finding 2 ADDRESSED（actions.js:148）；Verdict needs-attention，fix diff 的新 findings 2 個（log 40 行、無 usage limit）：
  [medium] output.js:293-309 — abortInFlight() 在 abort 前就清空 inFlightController、select() 隨即發新 fetch，不等舊 Promise settle；若 fetch 忽略 AbortSignal，未結束請求會累積，違反「至多一個進行中」。
  [low] live-output-check.js:1810-1814 — N 情境的洩漏檢查只篩 title 不篩 text；G 段因舊請求被 abort 已失去對 generation guard 的辨識力。
- Ruling R22：[medium] 判為「有爭議、部分採納」：產品只跑在遵守 AbortSignal 的原生 fetch 上，abort 即在網路層取消，「至多一個進行中」在受支援的環境成立；O 段「忽略 abort 的假 fetch」是模擬請求卡住的測試手法，不是受支援的執行環境——不改產品碼的取消模型（等舊 Promise settle 會把 Finding 1 的阻塞帶回來）。採納 Codex 的替代建議：以 CDP Network 事件證明「舊請求已被取消（loadingFailed／canceled）之後／同時，新請求才發出，任一時刻未取消的輸出請求 ≤ 1」，並在腳本註解與 report 明寫 O 段的假 fetch 是模擬手法 — 代價：若日後在非瀏覽器環境重用 output.js（fetch polyfill 不支援 abort），此不變量不保證。
- [low] 採納：N 情境改為檢查整段 text 歷史；新增情境以「忽略 abort、延遲成功」的假 fetch 讓舊回應確實晚到，重建 generation guard 的辨識力（附 RED：暫時拿掉世代檢查應 FAIL）。
G4 fix round 2/5 dispatched（續派同一實作者）。
G4 fix round 2/5：Finding A（R22）補 N 段 Network 事件佐證「至多一個進行中」；Finding B 補 N 段 text 歷史檢查＋新增 Q 段（忽略 abort、延遲成功的假 fetch）重建 generation guard 辨識力（附 RED）；另自行發現並修 Q 段跨行程時間戳比較的 flake（commits 4769040, 7825f69, 7b28ab5；A–Q 連續 3 次 PASS 各約 141s；workspace gate 乾淨；控制端核：產品碼非註解變更為空）— Codex scoped re-review dispatched；同時派出 task 6.1。
CODEX G4 re-review round 2 2026-09-20：range 518f9e4..7b28ab5 → Finding A ADDRESSED（live-output-check.js:1832）；Finding B NOT ADDRESSED（live-output-check.js:2088-2130）：Q 段 A 的假回應靠 2500ms timer＋測試固定 sleep 3200ms，未確認 A 確實在改選之後 resolve → 移除 generation guard 時仍可能通過，RED 辨識力不可靠。output.js 此範圍只有註解變更。（log 14 行、無 usage limit）
  控制端判定：成立；採 Codex 建議的確定性寫法（測試端持有 A 的 resolver，B 顯示後才觸發，等 A-settled marker 再檢查）。
G4 fix round 3/5：排在 task 6.1 實作者回報之後（不並行兩個實作者；兩者都用 cargo／Chrome）。原 G4 實作者 context 已約 45 萬 token → 改派 fresh sonnet，brief 見 task-G4-fix3-brief.md。
Task 6.1: implemented (commit c0bb7b0；DONE_WITH_CONCERNS；真機腳本連續兩次整支 0 FAIL；R18 兩次皆觀察到 (a) 同一 pane 仍在→過期標示與原因消失、輪詢恢復，(b) 未觸發；新發現：WSL 測試 server stop／重啟會殺掉 pane 內執行中的 shell（pane id 保留、內容重置為新 prompt）→handover 第 4 節) — 控制端核環境：WSL server 已停、埠已釋放、無殘留程序、使用者 cockpit.toml 未動（mtime 09-19 18:57）、無狀態檔、工作樹乾淨 — spec-check dispatched
G4 fix round 3/5 dispatched（fresh sonnet；只動 Q 段）。
Task 6.1: spec-check ✅ (sonnet; 0 Critical/Important；操作邊界符合：唯一的 herdr server stop 經 wsl.exe、taskkill 只用 /PID、設定檔在 os.tmpdir() 且用完刪除、repo 根 cockpit.toml 零引用；R18 的腳本修正判定為合理——Cockpit 忠實反映重啟後被清空的 pane；驗收紀錄 6 處抽核逐字相符、未涵蓋項目誠實記錄) — 腳本屬測試碼，併入 8.2 whole-branch Codex review
Task 6.1: minor (deferred)：「pane 被關掉」未獨立斷言 is-stale/opacity（產品碼確有 setStale(true)）；R18 (a) 的「內容已讀到」只斷言 length>0（隨後的重送迴圈已佐證）。
G4 fix round 3/5：Q 段改為測試端持有 A 的 resolver、B 顯示後才 resolve、等 Response.text() 已讀＋50ms 再斷言；RED（移除世代檢查）連 3 次 FAIL、GREEN 連 3 次 PASS；整支 A–Q 1 次 PASS（139s）；workspace gate 乾淨；控制端核：cockpit/ 無變更（commit 86106e2）— Codex scoped re-review round 3 dispatched
Task 8.1: gate run 2026-09-20 於 86106e2（控制端自跑）：cargo fmt --check OK；clippy --all-targets -D warnings 乾淨；cargo test --workspace passed=531 failed=0 ignored=10（基線 494/0/10，+37，ignored 不變）；cargo test -p cockpit --example ui_preview 另計（workspace 測試不含 example 內的單元測試）；markdownlint 73 files 0 issues；openspec validate --all 16/16。若 Codex 複審後腳本再改，只需重跑 CDP 腳本（不在 cargo gate 內）；8.2 之後若動到 .rs 要重跑本 gate。
CODEX G4 re-review round 3 2026-09-20：commit 86106e2 → Verdict approve；Finding ADDRESSED（live-output-check.js:2097-2197；runPoll 先讀 response.text() 再呼叫 onPollSettled，body-read marker＋macrotask 足以涵蓋 stale SUCCESS 的處理）；no material findings（log 27 行、無 usage limit）。
Task 5.1: complete (commit 84e225d；spec-check ✅；Codex G4 clean)
Task 5.2: complete (commits 0be71f0, 1e4151e；spec-check ✅；Codex G4：Finding 1 [high] → 34e2fb5 ADDRESSED)
Task 5.3: complete (commits 3001c88, 2d8e528；spec-check ✅＋fix round 1＋複檢；Codex G4：Finding 2 [medium] → ab4f0da ADDRESSED)
Task 5.4: complete (commit 7f5d794；spec-check ✅；Codex G4 clean)
Task 5.5: complete (commits f32e706, e540025；spec-check ✅；Codex G4 clean)
G4 fix wave: fix rounds 3/5（ab4f0da, 34e2fb5, 518f9e4 → 4769040, 7825f69, 7b28ab5 → 86106e2）；Codex 三次複審後 approve。
CODEX FINAL (task 8.2) 2026-09-21：whole-branch 58f0e16..2fd3bb2（51 files, +8295/−101）→ Verdict needs-attention（log 47 行、無 usage limit）。確認無問題：archive-safety、Cargo 依賴方向、前端純文字呈現。deferred minors (a)–(f) 全部判定「可保留」。Findings：
  [high] live-output-real-check.js:583-587 — 啟動前不確認 WSL server 是否已在跑、無條件 wslServerStarted=true；finally 全域 herdr server stop 並關閉所有固定 label 的 tab → 可能破壞非本次建立的資源。【成立；依 label 掃殘留是控制端 brief 要求的，未限定 baseline 之外，屬控制端疏漏】
  [high] tasks.md:45-58 — 6.2／7.3／8.1 未完成、handover 仍是舊的。【流程順序所致，非程式缺陷：7.3 依 R4 排在 8.2 之後；8.1 gate 已於 86106e2 跑過全綠；6.2 需使用者。照計畫完成】
  [medium] cockpit/src/http.rs:151-155 — get() 自動服務 HEAD → HEAD 進 handler 並觸發 pane.read，與「只接受 GET」矛盾且佔用讀取鎖。【成立；fix round 1 時控制端要求測試斷言 HEAD 200，屬控制端誤判】
  [medium] live-output-real-check.js:53-60 等 — 真實使用者名稱／主機名／本機路徑。【成立；本分支新增的命中：acceptance.md 7 行、real-check.js 1、pane-read-probe.md 1（控制端自己貼的 cat -v 節錄）、probe_pane_read.rs 1。main 上既有檔案（change-1a-spikes.md、real_herdr.rs、herdr-client/README.md 等）不在本 change 範圍，完成報告另提】
  [low] CONTEXT.md:53 —「全站至多選定一個 pane」應為每個瀏覽器頁面各自一個。【成立】
- Ruling R23：輸出端點 HEAD 改回 405、不觸發讀取；spec 已明訂（commit f801b69）— spec 字面「只接受 GET」、HEAD 無用途且白佔 HERDR I/O — 代價：推翻 fix round 1 的 HEAD 200 測試；若日後有工具以 HEAD 探活會得到 405。
- Ruling R24：個資清理只處理本分支新增的行；main 上既有的列入完成報告交使用者決定 — 不擴大 change 範圍、不動 archive — 代價：repo 內仍有既有的真實主機名／路徑（無 remote，風險低）。
FINAL FIX WAVE：單一 fix dispatch（fresh sonnet），brief 見 task-final-fix-brief.md；之後一次 scoped re-review。
FINAL FIX WAVE: implemented (commits f801b69..a039a46 = 4aeaf02 F2 HEAD→405, c74edb6 F3 文件＋F4, a039a46 F1＋F3 腳本；implementer DONE；控制端核：WSL server 已停、埠與程序無殘留、cockpit.toml 未動、工作樹乾淨、本分支 + 行個資命中 0) — Codex scoped re-review dispatched；控制端同時重跑全 gate
Task 8.1: gate re-run 2026-09-21 於 a039a46（final fix 之後，控制端自跑）：fmt OK；clippy 乾淨；cargo test --workspace passed=531 failed=0 ignored=10；ui_preview example tests 13 passed；markdownlint 73 files 0 issues；openspec validate --all 16/16。
CODEX FINAL re-review 2026-09-21：range f801b69..a039a46 → F3 ADDRESSED、F4 ADDRESSED；F1 NOT ADDRESSED、F2 NOT ADDRESSED；另 1 個新 finding；Verdict needs-attention（log 27 行、無 usage limit）：
  [high] live-output-real-check.js:648-680, 807-809 — server 所有權仍有競態：檢查與啟動非原子；R18 重啟未重新檢查；startWslServer() 不驗證結果即標 owned、waitForWslServerUp() 回傳值被忽略；finally 仍以布林旗標授權全域 server stop。
  [medium] cockpit/src/http.rs:161-164 — HEAD handler 在 route_layer 內 → 不合法 Host 的 HEAD 回 403；spec／design D6／README 宣稱含 HEAD 一律 405（POST 不合法 Host 是 405，因 fallback 在 layer 外）。
  [medium] live-output-real-check.js:360-362 — COCKPIT_ACCEPT_WSL_SOCKET 未經驗證即插入 bash -lc 字串（shell interpolation）；寫 TOML 時亦未跳脫。
- Ruling R25：再開一輪修正（final fix round 2），不依 SDD「final 之後無第二輪、殘留交使用者」處理 — 指示優先序：使用者指示（2026-09-19：「findings 修完才打勾、才宣稱完成」）與全域 CLAUDE.md（Codex 補審通過才可宣稱完成）＞ skill；三項皆具體且成本低，停下來問只會卡住使用者 — 代價：多一輪 implementer＋一次 Codex 複審；若這輪之後仍有殘留，才逐條裁決並交使用者，不再開第三輪。
- 控制端疏漏記錄：HEAD 的 403／405 是控制端在 final-fix brief 寫「兩者都可以」造成的，與自己寫進 spec 的「一律 405」矛盾。
FINAL FIX ROUND 2 dispatched（fresh sonnet；brief 見 task-final-fix2-brief.md）。
FINAL FIX ROUND 2: implemented (commits a039a46..ef50620 = 89bb4c0 B：HEAD 一律 405、ef50620 A＋C：PID 所有權＋消除 shell 注入面；implementer DONE；真機 (i)(ii)(iii) PASS；workspace gate 乾淨；個資 grep 0；控制端核：無殘留程序／埠、WSL server 已停、工作樹乾淨) — Codex scoped re-review dispatched
NOTE：實作者發現並修正「wsl.exe 呼叫巢狀 setsid -f bash -c 的 detach 競態（約 1/4 機率行程憑空消失、exit 0 無錯誤）」，並自行寫入專案 memory wsl-nested-setsid-detach-races-with-caller-exit；控制端審過內容後保留，修掉兩處過度宣稱（只在一台機器實測；成因屬推測）並加上與 handover 既有坑的關聯。
Task 8.1: gate re-run 2026-09-21 於 ef50620（final fix round 2 之後，控制端自跑）：fmt OK；clippy 乾淨；cargo test --workspace passed=531 failed=0 ignored=10；ui_preview example tests 13 passed；markdownlint 73 files 0 issues；openspec validate --all 16/16；工作樹乾淨。
CODEX FINAL re-review round 2 2026-09-21：range a039a46..ef50620 → B ADDRESSED（cockpit/src/http.rs:167-170：GET 保留 source_check、HEAD 在 layer 外固定 405、405 經 error_response 帶兩個安全標頭）；C ADDRESSED（real-check.js:361-423, 465-470, 664-685；未發現 tab／pane id 被插入 shell）；A NOT ADDRESSED；Verdict needs-attention（log 23 行、無 usage limit）。殘留 3 個，全在 docs/research/2026-09-19/live-output-real-check.js（僅供開發者手動執行的真機驗收腳本，非產品碼）：
  [high] :540-550 — PID 比對與全域 herdr server stop 之間有 TOCTOU。
  [medium] :884-886, :1043-1048 — verifyOwnership 回 false 後主流程仍繼續（ownedPid 直接取自 pidfile）。
  [medium] :1278-1291 — --verify-ownership-mismatch 模式繞過 HERDR_CLIENT_TEST_ALLOW_WSL_WRITES opt-in 檢查。
依 R25 不再開第三輪；逐條裁決並 parked，交使用者：
- Final: parked — [high] TOCTOU — Ruling R26：真實但非 load-bearing：競態窗為兩次 wsl.exe 呼叫之間（約數十 ms），且須「自己的 server 恰在此時退出＋另一個 server 恰在此時啟動」；單人開發機、AGENTS.md 明定 WSL 測試 server 可停 — 沒有任何後續工作建在它上面 — 若錯的代價：極端交錯下誤停一個 WSL 端的 HERDR server（其 pane 內的 shell 會被收掉）。建議修法（待使用者決定）：改以 `kill -TERM <ownedPid>` 並等待該 PID 結束，需先真機確認 HERDR 對 SIGTERM 會乾淨收尾（刪 socket）。
- Final: parked — [medium] verifyOwnership 結果被忽略 — Ruling R27：真實的腳本邏輯缺口、修法 3 行（false 即 throw，兩處）；不是 load-bearing（停止前仍會再比對 PID，不會誤停；但期間可能對外來 server 建／關 tab）— 若錯的代價：啟動競態下對別人的 WSL server 建立一個測試 tab 並送 tick 迴圈，收尾會關掉自己建的 tab。
- Final: parked — [medium] 測試掛勾繞過 opt-in — Ruling R28：真實、修法是把 opt-in 檢查移到共用入口；不是 load-bearing — 若錯的代價：有人不帶環境變數直接跑該掛勾會啟動一個不會自動停的 WSL server。
- Ruling R29：task 6.1 仍判定完成——其驗收條件（腳本全數 PASS、結果寫入驗收文件）已達成且經規格對照檢查；上述 3 項是腳本「不誤傷外部資源」的健壯性，不影響已取得的驗收結果。task 8.2 **不打勾**：其驗收條件是「review 無未處理的成立 finding」，現有 3 個 parked finding 待使用者決定。
Task 6.1: complete (commits c0bb7b0, a039a46, ef50620；spec-check ✅；whole-branch Codex：3 parked with rulings R26–R28)
Task 8.1: complete (gate green at ef50620；見上)
Task 7.3: complete (handover v15 由控制端親自重寫——整個 change 的脈絡只在主對話；fresh sonnet read-back：9/10 通過，2 處修正〔cockpit-core／cockpit-herdr 無 README；Codex 路徑不寫死版號〕＋驗收文件 6.2 清單補 4 項；markdownlint 73/0；個資 grep 0)
STATUS 2026-09-21：23/25 勾。未勾：6.2（使用者手動驗收）、8.2（3 parked findings 待使用者決定，R26–R28）。工作區保留（final review 尚未 clean，不刪 .superpowers/sdd/tasks/）。
Task 6.2: complete (2026-09-21 使用者授權 Claude 以 Claude in Chrome 代為執行；tasks.md 6.2 的檢查項全數通過，量測值記於 live-output-acceptance.md；對 Windows 端 HERDR 全程唯讀、未讀出畫面文字；「關掉被選的 pane」未在 Windows 端執行〔寫入類操作，超出約束〕，已由 6.1 與 CDP J／K 段涵蓋；環境已收：cockpit PID 47812 已停、7793 釋放、暫存設定與狀態檔已刪、cockpit.toml mtime 不變；commit 見 git log)
FINDING 2026-09-21（Windows 端驗收）：整頁重畫使被聚焦的 pane 列節點被換掉、焦點掉回 body（實測約 5 秒一次）→ spec 新增的「鍵盤選定」在真實重畫頻率下幾乎用不到；機制即既有的 M3，非 change 3 引入；CDP 鍵盤情境因聚焦後數毫秒內按鍵而未抓到。
- Ruling R30：不自行修；列為待使用者決定（change 3 內修並過 Codex，或接受並留給視覺改版＋spec 加註）— 屬範圍決定（是否把 M3 拉進 change 3）— 代價：在使用者決定前，鍵盤選定在真實環境不可靠。
STATUS：24/25 勾。未勾：8.2。待使用者決定：R26–R28（真機腳本 3 個 parked finding）、R30（鍵盤焦點）。
USER DECISION 2026-09-21：鍵盤焦點採選項 A（在 change 3 內修，一併解掉 M3）；「由你完整收尾」——控制端據此一併修 3 個 parked finding（R26–R28，照先前建議）、過 Codex、勾 8.2、squash 併回 main、archive、重寫 handover、清理分支。spec 已加：live-output 情境「鍵盤焦點跨重畫保留」、cockpit-dashboard「畫面整頁重畫」本文與情境「重畫不丟鍵盤焦點」。
WRAP-UP FIX 1（焦點還原）dispatched。
WRAP-UP FIX 1（焦點還原）: implemented (commit af7aec4；render.js paint() 加 capture→replaceChildren→restore；CDP 新增 R／S 段；A–S 連續 2 次 PASS；actions／factory-floor PASS；workspace gate 乾淨) — Codex review dispatched；同時派出 WRAP-UP FIX 2（真機腳本 3 findings）
CODEX review 2026-09-21：commit af7aec4（焦點還原）→ Verdict needs-attention（log 15 行、無 usage limit）。3 findings，控制端讀後判斷機制皆成立：
  [high] render.js:628-635 — actions.js 在 pointerdown 內同步 perform()→同步 repaint；瀏覽器對被按元素的預設聚焦在 pointerdown 派發之後，paint() 捕捉到的仍是先前聚焦的 A，還原到新的 A；按了 B 焦點卻留在 A，再按 Enter 可能再次執行 A。【本次修正引入的回歸】
  [medium] render.js:594-603 — 「看輸出」按鈕身分只有 action／runtime／pane；兩個 workstream 綁同一 pane 時身分相同，焦點會從第二顆跳到第一顆。
  [medium] live-output-check.js:2515-2528 — 不捲動測試中目標始終在視窗內，移除 preventScroll 仍綠，無辨識力。
處置：fix round 1，排在 WRAP-UP FIX 2 實作者回報之後（不並行兩個實作者）；續派原焦點實作者。
WRAP-UP FIX 2（真機腳本）: implemented (commit 6433654；SIGTERM 實測 3/3：140–250ms 結束、無孤兒、socket 殘留可安全清、下次啟動正常；killAndWaitPid 單一 wsl.exe 呼叫對 ownedPid；verifyOwnership 失敗即 throw；opt-in 檢查移至共用入口；真機 (i)–(v) PASS；控制端核環境乾淨) — Codex re-review dispatched；同時續派焦點實作者做 fix round 1
INTERRUPTED 2026-09-21：焦點還原 fix round 1 的實作者因 Claude session limit（HTTP 429，12:00 重置）終止；中斷在讀檔階段，工作樹乾淨、無半成品。使用者 19:20 指示「兩個都修」。RESUMED：續派同一 agent。
CODEX re-review 2026-09-21：commit 6433654（真機腳本）→ Finding 2 ADDRESSED（:966-978, 1138-1153）、Finding 3 ADDRESSED（:1446-1456）；Finding 1 NOT ADDRESSED；Verdict needs-attention（log 13 行、無 usage limit）。未發現可執行的全域 server stop、shell 插值或 Windows 端操作。
  [high] :615-625 — 比對 PID（pgrep）與 killAndWaitPid 是兩次 wsl.exe invocation；送 signal 前不驗證 `/proc/<pid>` 的程序名或啟動身分；TERM 後以 kill -0 輪詢，PID 被重用時會對新程序送 SIGKILL。
  [medium] :516-527 — wslSocketExists／listHerdrServerPids／rm -f socket 是三次 invocation，外來 server 可在其間啟動，socket 被刪。
處置：script fix round 2，排在焦點還原 fix round 1 之後（不並行兩個實作者）。
焦點還原 fix round 1/5：3 findings 皆經 RED 重現「成立」並修正（commits 92f79d5 Finding 2〔看輸出加 data-project／data-source-workstream〕、632481e Finding 1〔actions.js 設 pending focus target＋pointerdown 委派 preventDefault 關掉相容滑鼠事件；根因：同步 repaint 後瀏覽器補送的相容 mousedown／click 對新 DOM 重新 hit-test 搶走焦點〕、661e9c6 Finding 3〔不捲動情境改為先聚焦→捲出視窗→記基準；RED 拿掉 preventScroll 正確 FAIL〕；live-output-check.js A–U 連續 PASS；actions／factory-floor PASS；workspace gate 乾淨) — Codex re-review dispatched；同時派出 script fix round 2
CODEX re-review 2026-09-21：commits 92f79d5, 632481e, 661e9c6（焦點還原 fix round 1）→ Finding 1 ADDRESSED（actions.js:245-252、render.js:641-710）、Finding 2 ADDRESSED（render.js:323-329, 649-662）、Finding 3 ADDRESSED（live-output-check.js:2510-2570）；Verdict needs-attention，新 1 個（log 21 行、無 usage limit）：
  [medium] actions.js:250-252 — 清除 pending focus target 在 perform(target) 之後、沒有 finally；perform 同步呼叫鏈（liveOutput.select、repaint、DOM rendering）任一處拋錯就跳過清除，之後無關的重畫會優先用殘留 target 還原焦點，Enter 可能再次觸發該操作。
處置：焦點還原 fix round 2/5（fresh sonnet；原實作者 context 已約 52 萬 token）；排在真機腳本 round 2 實作者回報之後。
WRAP-UP FIX 2 round 2: implemented (commit 736eca3；PID＋starttime＋comm 身分憑證、同一 invocation 內核對→TERM→輪詢核對→KILL 前核對；不再刪 socket；拒絕條件改為「有 herdr 程序或 socket 連得上」；真機 (i)–(v) PASS；疑慮：HERDR 0.8.2 對 SIGTERM 是否自清 socket 兩輪觀察不一致，腳本已不依賴任一假設；控制端核環境乾淨) — Codex re-review dispatched；同時派出焦點還原 fix round 2
CODEX re-review 2026-09-21：commit 736eca3（真機腳本 round 2）→ A ADDRESSED（:578-605；stat 解析與 starttime 索引正確、TERM／輪詢／KILL 前皆重驗）、B ADDRESSED（:629-664；已完全移除 socket unlink）；單次 bash 內核對到 kill 的極短 PID 重用窗口 Codex 自判「僅屬理論風險、單使用者開發機上不具實質可利用性」；Verdict needs-attention，新 1 個（log 13 行、無 usage limit）：
  [medium] :649-684 — wslServerAlreadyRunning() 先查 process 再做最長 3 秒的 socket probe；外來 server 恰在 process 檢查後啟動、且 probe 逾時前尚未能回應 → 被判為 stale → 腳本啟動第二個 server、可能接管同一 socket。Codex 建議：有界的穩定觀察期＋啟動前最後一次檢查；「更穩妥的做法是讓本次驗收使用專屬 socket 路徑」。
- Ruling R31：不再對「共用 socket 路徑」上的競態逐一補丁（已四輪，每補一個露出下一個）；改從根上換路——驗收腳本每次執行使用專屬的 socket 路徑（若 HERDR 0.8.2 的 `herdr server` 支援以環境變數指定；handover／example 設定提到 HERDR_SOCKET_PATH，需真機確認）。如此腳本與任何手動啟動的 server 不再競爭同一路徑，拒絕檢查、殘留 socket、誤判 stale 三類問題一併消失；PID＋starttime＋comm 的所有權憑證保留 — 依 judgment-rubrics「撞牆換路」— 代價：多一輪 implementer＋Codex；若 HERDR 不支援指定路徑，則本 finding parked 並附風險評估（需開發者恰在腳本啟動的約 3 秒窗內手動啟動 WSL server），不再開新一輪。
script fix round 3 排在焦點還原 fix round 2 實作者回報之後。
INTERRUPTED 2026-09-21：Claude Code session 重啟，焦點還原 fix round 2 的實作者未完成；工作樹留有未 commit 的半成品（cockpit/assets/app/actions.js +17/−3、docs/research/2026-09-19/live-output-check.js +117）、無 report、無殘留程序。RESUMED 20:51：續派同一 agent。
焦點還原 fix round 2/5：actions.js 的 perform(target) 改 try/finally、CDP 新增 V 段（RED：焦點被還原到注入例外的 pane 列；GREEN 後連續 2 次整支 A–V PASS）；actions PASS、factory-floor 環境性 flake 重跑 PASS；workspace gate 乾淨（commit c164b77）；疑慮：select-pane 拋錯時 ui.selected 已先設定、不回滾（非本次範圍）— Codex re-review dispatched；同時派出 script fix round 3（R31 專屬 socket 路徑）
CODEX re-review round 2 2026-09-21：commit c164b77（焦點還原 fix round 2）→ Verdict approve；finding ADDRESSED（actions.js:258-263）；鍵盤路徑不設 pending target、無 re-entrant 情況、V 段有辨識力；「就 c164b77 的 render.js／actions.js 而言，未發現違反『畫面整頁重畫』焦點要求的剩餘 material defect，此區域可合併」（log 23 行、無 usage limit）。
WRAP-UP FIX 1（焦點還原，使用者選項 A）: complete (commits af7aec4 → 92f79d5, 632481e, 661e9c6 → c164b77；fix rounds 2/5；Codex 4 個 findings 皆經 RED 重現並修正；Codex approve)
minor (deferred)：select-pane 的 perform() 中途拋錯時 ui.selected 已先設定、不回滾（畫面不受影響）；pointerdown preventDefault 對 CSS :active 視覺回饋的影響未驗。
USER DIRECTIVE 2026-09-21（下一段 Direction 01 視覺改版）：設計完全參照 docs/direction-01-visual-design.md；概念圖僅標示功能區的位置與排列。已寫入 handover 第 3 節與 direction-01 文件（加註、未改原句）。
script fix round 3（R31 專屬 socket 路徑）: implemented (commit 225077e；真機確認 HERDR 0.8.2 的 herdr server 採用 HERDR_SOCKET_PATH、與預設路徑的 server 可並存且互不可見；真機 (i)–(vi) 全過 52 ok／0 FAIL；控制端核環境乾淨) — Codex re-review dispatched；控制端同時重跑全 gate
CODEX re-review round 3 2026-09-21：commit 225077e（真機腳本 round 3）→ 上輪 medium ADDRESSED；未發現 Windows 端操作、machine-wide process check、越界刪除；/tmp symlink／刻意搶占判為理論風險；Verdict needs-attention，新 1 個 medium：listTabs() 仍用 herdr api snapshot CLI、沒帶 HERDR_SOCKET_PATH → 查到預設 socket（log 17 行、無 usage limit）。
- Ruling R32：這筆不是又一個理論競態，而是 R31 換路漏掉的具體呼叫點（開發者開著預設 server 時實際會發生），不適用「只剩理論風險就裁決結案」；屬幾行的修正，控制端直接改、真機實測、交 Codex 複審（diff 審查仍歸 Codex）。
script fix round 4: complete (commit 96e574f；listTabs() 改經 wslRpc 對專屬 socket 送 session.snapshot、格式不對就拋錯；log 印 baseline。真機兩情境 RESULT: PASS——(1) 預設 server 未啟動：baseline 拿到專屬 server 2 個 tab（修正前為空）；(2) 預設 server 並存且多一個標記 tab wD:t28（共 3 個）：baseline 仍只有專屬的 2 個，跑完後預設 server PID 1692 與 3 個 tab 原封不動。第一次並存嘗試因 Git Bash MSYS 路徑轉換吃掉 socket 路徑、標記 tab 沒建成，該次無辨識力、已作廢重跑。收尾：關掉標記 tab 與 round 3 遺留的 manual_dev_server_marker tab、停掉自己啟動的 WSL 預設 server；WSL 無 herdr 行程、無 /tmp/cockpit-accept-* 殘留、cockpit.toml mtime 未變)
CODEX re-review round 4 2026-09-21：commit 96e574f → Verdict approve；ADDRESSED；全檔無仍連到預設 socket 的呼叫；No material findings（log 含 19 行 [codex]、無 usage limit／Turn failed）。
FINAL GATE @96e574f：fmt OK／clippy 乾淨／cargo test --workspace 531 passed 0 failed 10 ignored／ui_preview example 13 passed／markdownlint 0 issues／openspec validate --all 16 passed。
TASK 8.2: complete——所有 Codex findings 皆經實測重現後修正或裁決（見各 CODEX 條目與 Rulings），無未處理的成立 finding。
