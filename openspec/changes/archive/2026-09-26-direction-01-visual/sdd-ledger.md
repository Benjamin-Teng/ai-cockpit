# SDD ledger — plan: openspec/changes/direction-01-visual/tasks.md

Spec（binding）：`openspec/changes/direction-01-visual/specs/{cockpit-dashboard,live-output}/spec.md`；design：同目錄 `design.md`；
視覺唯一依據 `docs/direction-01-visual-design.md`。分支 `feat/direction-01-visual`，MERGE_BASE＝`f6e0892`（main），artifacts `d9d68b4`。
Reviewer：Codex adversarial-review（`node ~/.claude/plugins/cache/openai-codex/codex/1.0.6/scripts/codex-companion.mjs adversarial-review --wait --base <BASE> "<focus>"`；
focus 不放反引號；看 log 有無 `usage limit`／`Turn failed`；唯讀沙箱只能 `cargo fmt --check`，findings 重現才採信）＋ sonnet 規格對照檢查（非 diff）。
Implementer：sonnet（卡住升 opus）。`task-brief` 腳本不認 OpenSpec checkbox → brief 由控制端手抽到 `task-<N>-brief.md`。

## Preflight 衝突掃描

| 對象 | 產出 vs 消費 | 發現 |
|---|---|---|
| 1.2 ↔ 通則 | 1.2「兩份 delta spec 每個 scenario 各寫一段」vs 通則「行為未變的 scenario 由既有腳本守住，visual-check 不重做」 | **衝突** → R1 |
| 1.2 → 2.1–5.2 | visual-check 段落 → 各 task 驗收 | 各 task 驗收列的段名全在 spec 內，一致；段名加 `dashboard/`／`live-output/` 前綴 |
| 1.2 RED | 「各 scenario 段依預期失敗」vs 現行前端可能已滿足（不為字體發請求、未知 status 不破壞畫面等） | **衝突** → R3 |
| 2.1 ↔ 4.1 | 2.1 驗收「桌面寬度不整頁捲動」要求 Live Output 在視窗內可見；面板常駐到 4.1 才做 | **衝突** → R2 |
| 2.3 ↔ 3.3 | 「兩個 runtime 的畫面」同一段，2.3 驗頂／底列、3.3 驗右欄 | 段內子斷言分標 → R4 |
| 3.4 ↔ 4.2 ↔ 5.2 | 「文字對比」同一段，錯誤改綁／過期／全狀態分屬三 task | 同上 → R4 |
| 2.1 → 3.1／3.4／4.1 | 2.1 建 `data-region`（projects、banner、output）→ 後續填內容 | 一致；2.1 需先放 projects 區塊與 output 的 `data-region` |
| 2.1 ↔ live-output-check.js | 2.1 改寫兩處 `scrollTo` 手法；4.1／4.2 再改面板相關斷言 | 循序同檔，無衝突 |
| 3.2 ↔ factory-floor-check.js | 逐字 HTML 斷言 → 改 DOM 結構判斷 | 一致 |
| 4.2 ↔ live-output-real-check.js | 只改不跑，5.4 實跑 | 一致 |
| 5.1 ↔ visual-check.js | 5.1 要用 visual-check 走訪 DOM 列 class | 1.2 未要求此功能 → 5.1 自行加段；非衝突 |
| 5.2 → 5.5 | 截圖給使用者目視 → 點頭後才 5.5 | 5.5 需使用者 → R7 |
| 5.3 ↔ ledger | `.gitignore` 加 `.superpowers/` | ledger 本來就被 exclude；archive 時比照 change 3 複製為 `sdd-ledger.md` |
| 5.4 ↔ SDD final review | 同一件事 | 5.4 即 whole-branch review |

每個 task 自身一致性：1.1 無程式碼（控制端跑）；2.1 同時改產品與 `live-output-check.js`（通則要求分開 commit，brief 交代）；
2.3 需手動停／重啟 `ui_preview`（implementer 可做）；其餘 task 的驗收段名與所述行為相符，未見斷言空無一物或強制逐字複製。

## Rulings

- Ruling R1：visual-check.js 只為「本 change 改變了行為或外觀」的 scenario 寫段——即各 task 驗收列出的段（2.1：桌面寬度不整頁捲動、中等寬度、窄視窗單欄、網格過寬時不撐破頁面、長名稱不溢出；2.2：減少動態、不為字體發出網路請求、running 節點沒有動畫；2.3／3.3：兩個 runtime 的畫面；3.1：切換 Project、選取跨重畫保留（dashboard）、鍵盤切換與焦點保留、切換 Project 不清除錯誤也不離開改綁模式、各狀態數量、沒有 Project、兩個 Project；3.2：Scenario D 的畫面、未知 status 不破壞畫面、狀態不只靠顏色；3.3：done 不使用成功色、未知狀態不破壞畫面（dashboard）；3.4／4.2／5.2：文字對比；4.1：沒有選取時顯示空狀態、點 pane 列、取消選取、面板打開時仍可操作頁面下方的內容、鍵盤選定）；通則點名由既有腳本守的 scenario 不重做 — 通則明文且 tasks 驗收清單與之吻合，1.2 的「每個 scenario」是概括寫法 — 錯了的代價：某個既有腳本其實沒守到的 scenario 少一層驗證（final review 對照通則清單）。
- Ruling R2：2.1 的「桌面寬度不整頁捲動」段中「Live Output 在視窗內可見」子斷言延到 4.1 才要求；段內以明確 PENDING(4.1) 印出，不得印 PASS — 面板常駐是 4.1 的範圍，2.1 無法滿足 — 代價：2.1 到 4.1 之間這條子斷言不守。
- Ruling R3：1.2 的 RED 要求放寬為「每段要嘛 RED、要嘛在回報中列出為何現行前端已滿足」；已綠的段須有自我測試證明偵測器有辨識力（例如注入外部字體請求會被抓到、注入動畫會被抓到）— 現行前端可能本來就滿足部分 scenario，硬要 RED 會逼出假測試 — 代價：無。
- Ruling R4：同一 scenario 分屬多個 task 驗收時（兩個 runtime 的畫面、文字對比），段內子斷言以標籤區分（例如 `topbar`／`statusbar`／`runtimes`；`banner`／`stale`／`all`），每個 task 只要求自己那部分通過，5.2 要求全部 — 代價：無。
- Ruling R5：1.1 由控制端自己跑，不派 implementer — 只是跑指令記數字，無 diff — 代價：無。
- Ruling R6：每 task 的審查＝sonnet 規格對照檢查（非 diff：有沒有照 spec／brief、測試是否真有辨識力）＋ Codex adversarial-review（diff）；不派 Claude 當 diff reviewer（CLAUDE.md）；比照 change 3 — 代價：每 task 兩個審查座位。
- Ruling R7：5.5 等使用者對 5.2 截圖點頭後才做；5.4 的 Codex whole-branch review 即 SDD final review — tasks.md 明文 — 代價：無。

## Progress

Task 1.1: complete (baseline at e6163f6, controller-run R5: workspace passed=531 failed=0 ignored=10; ui_preview 13/0/0; six scripts PASS — reconnect 10s, whatever 1s, actions 16s, channel-backoff 50s, factory-floor 6s, live-output 170s; no leftover LISTEN on 7770/92xx)
Task 1.2: dispatched (BASE e6163f6)
USER DECISION 2026-09-24：外觀要由 frontend-design skill 審核。

- Ruling R8：加一道「設計審核」座位（fresh opus subagent，先用 Skill 工具載入 `frontend-design:frontend-design`），與 spec-check、Codex 並列：(a) 前置：現在就審 `docs/direction-01-visual-design.md`＋design D3/D4/D8（唯讀、不跑 ui_preview），結論當建議交使用者，不擋 1.2；(b) 每個改外觀的 task（2.1、2.2、2.3、3.1、3.2、3.3、3.4、4.1、4.2）完成後，以 1536／1100／700 三寬截圖＋CSS diff 審；(c) 5.2 全面再審一次。優先序：設計文件與 spec＞skill 原則——skill 與設計文件或 spec 衝突處（例：spec 規定 id／時間／數值用等寬字、禁止動畫）只記錄、不列 finding；落在設計文件未規定的自由度內的問題才列 finding，進 fix loop，須附截圖或計算樣式證據。要改設計文件或 spec 色值的建議一律交使用者裁決 — 使用者要求；skill 自己也寫「brief 的指示優先」— 代價：每個視覺 task 多一個 opus 座位與一輪截圖時間。
Design preflight 2026-09-24：opus 審 → `design-review-preflight.md`（12 條建議＋各 task 截圖檢查清單）。待查證已由控制端查清：`Microsoft JhengHei UI`、`Cascadia Mono` 皆在本機 C:\Windows\Fonts；設定檔只在啟動時讀（cockpit/src/config.rs:177 `load`，無 watch）→ 加 Project 要重啟 cockpit。
USER DECISION 2026-09-24（建議 3／4／5／10 全採推薦）：①主標題＝選定 Project 名，產品名降為面板標題級，design 加 D10 字級階層；②固定一屏條件改為「寬 ≥1200 且高 ≥720」，矮時三欄＋整頁捲動（改 spec）；③切角只套 Factory Floor（切角＋刻度，刻度每 stage 一格、對齊欄位、有 running 的 stage 刻度較長且用 --text）與 Live Output（只切角、--bg-deep 底），左右欄只用 1px --line 框線（改 D8）；④Live Output「關閉」改名「取消選取」（改 live-output spec）。
- Ruling R9：前置審核中不需使用者裁決的建議 1、2、4(1)(3)、6、7、8、9、11、12 全數採納，寫進 design.md（D3、D4 補「冰青形狀分工」與「連線配色也適用底列通道狀態」、新增 D10 字級、D11 字體堆疊與文案），之後各 task 的設計審核以此為準 — 皆落在設計文件未規定的自由度內、不改 spec 行為 — 代價：實作者要讀的 design 變長；若某條截圖後不好看，由該 task 的設計審核提出修正。
- Ruling R10：高度門檻定為 720px（spec 需要可測的數字；審核建議「約 720，截圖定」）— 代價：若 5.2 截圖顯示門檻不妥，要回改 spec 一個數字。
Task 1.2: implemented (commits e6163f6..1cb3866; DONE; 188 ok/42 FAIL/1 PEND(4.1) RED as expected; S1–S3 self-tests PASS; V3/D1/FN1 already green w/ negative controls (R3); gates green cockpit 156/0/1) — concern→2.2/3.3: D1 用代理 token --conn-connected，--ok 在 2.2 才定義；P1/CT1 依賴「任何操作清 ui.error」順序
- Ruling R11：factory-floor-check.js 每跑一次就覆寫已進版控的 change 2 驗收截圖 docs/research/2026-09-16/task-5.2-scenario-d.png；每次跑完一律 git checkout -- 還原、不得 commit 它（基線那次已還原）— 那是 change 2 的歷史證據，不是本 change 的產出 — 代價：無；每個 dispatch 交代。
Task 1.2: review dispatched — Codex (--scope branch --base e6163f6, log codex-1.2.log) ＋ sonnet spec-check（task-1.2-speccheck.md）並行
Artifacts updated: b3ad4cf（spec×2、design D3/D4/D7/D8/D10/D11、tasks、proposal 兩處按鈕名；validate 17 passed、markdownlint 80/0）— Claude read-back dispatched（artifact-update-readback.md）
CODEX 1.2 2026-09-24：needs-attention，8 findings（5 high／3 medium；log 49 行、無 usage limit）：F1 對比忽略 opacity／alpha；F2 S1 注入自測與 P1 以固定等待、未證明期間真有推送；F3 startChrome／startPreview 失敗路徑殘留行程；F4 取消選取段缺「不請求／停止請求」斷言（其「文字應為關閉」部分依 b3ad4cf 已不成立）；F5 D1 成功色用舊代理值且 color OR background 過寬；F6 V2／V3 版面關係斷言太弱；F7 R1 statusbar 內容與 topbar 文字／色未斷言；F8 未逐 scenario 一段。
- Ruling R12：F8 與 R1 衝突 → R1 維持（不為行為未變的 scenario 重做段），但採納其後半：檔頭映射表要逐條列出 R1 排除的 scenario 由哪支既有腳本的哪一段守 — 通則明文由既有腳本守 — 代價：無。F4 只採「請求計數」部分，文字斷言維持「取消選取」（b3ad4cf 已提交）。其餘 F1–F3、F5–F7 進 fix round 1，實作者須先以負對照／刻意破壞證明 finding 成立（RED）再修，不成立則回報理由（Codex findings 須重現才採信）。
Artifact read-back ❌ 5 findings（proposal 兩處未同步高度門檻與 D8；D3 矮視窗分支缺 Floor 最小高度；D10 16px 無用途；D7 文案屬合理延伸）→ 續派原更新者修。
- Ruling R13：D10 改為四階 20／14／13／12px，刪掉未指派的 16px — KISS，沒有用途的階層只會讓實作者自己發明用途 — 代價：若日後需要中間字級要回補 token。
Artifact fix: 6b9a8fe（proposal F1/F2、D3 矮視窗最小高度＋spec THEN 引用 240/320px、D10 四階）；validate 17/0、markdownlint 80/0；控制端 grep 核對無「五階」殘留
SPEC-CHECK 1.2：❌ 1 Critical（V2 在現行前端實為 PASS，報告誤記為 RED；總數 43≠42）、1 Important（G1 混 2.2／3.2 無子標籤）、2 Minor → 併 Codex F1–F7＋F8 後半，fix round 1 續派原實作者（FIX_BASE 1cb3866）
Task 1.2: fix round 1 implemented (commit 599234f; 232 ok/48 FAIL/1 PEND; S1–S3 PASS；新增 FAIL 為真違規：D1 done/completed 文字色相同、V2 Live Output 與 Floor 重疊、R1 細節；34 spawned PIDs 全清；concern：F3 失敗清理路徑未以真故障誘發）— scoped re-review dispatched（Codex base 6b9a8fe＋sonnet speccheck 複審）
SPEC-CHECK 1.2 r1：4/4 ADDRESSED，新破壞 0（V2 實測 floor.bottom=911 > output.top=705 真違規；D1 done/completed 皆 rgb(15,17,21) 真違規）
CODEX 1.2 r1（base 6b9a8fe）：needs-attention；F1/F2/F3/F5 ADDRESSED；F4/F6/F7 NOT ADDRESSED；新 high：finalSweep 只憑 PID 殺程序樹、PID 重用可誤殺無關程序（log 無 usage limit）。Task 1.2: fix round 1/5 (spec-check 4/4 addressed; Codex 4 addressed, 3 open＋1 new; commit 599234f) → fix round 2 續派原實作者（FIX_BASE 599234f）
Task 1.2: fix round 2 implemented (commit f05733e; 225 ok/82 FAIL/1 PEND；V3 0→34 FAIL＝改用真 data-region 後如實 RED；finalSweep 改 untrack＋身分核對、新增 S4；carry→2.3：R1 期望 [data-conn-state]／[data-channel-state] 屬性約定) — Codex scoped re-review dispatched (base 599234f)
CODEX 1.2 r2（base 599234f）：needs-attention；empty baseline／exact state text／V3 ADDRESSED；finalSweep PID 重用仍 NOT ADDRESSED（核對與 taskkill 間競態；同路徑 ui_preview 身分相同）；新 medium：收尾失敗仍無條件 untrack（log 無 usage limit）。
Task 1.2: fix round 2/5 (3 addressed, 1 open + 1 new — finalSweep 所有權; commit f05733e)
- Ruling R14：finalSweep 換設計，不再以「重新查詢 PID＋身分比對」判定所有權：改以 Node 的 ChildProcess 物件為唯一依據——只對「尚未觀察到 exit 事件」的 child 送終止（Node／libuv 在 exit 被收割前一直持有該程序的 handle，Windows 在有 handle 開著時不會重用該 PID，所以此時的 PID 必屬本腳本）；只在觀察到 exit 事件後才 untrack；收尾失敗保留追蹤；移除 PowerShell 身分查詢。不上 Job Object — handover §4「開發者腳本同類 finding 第三輪還在冒就換設計」＋KISS — 代價：Chrome 根程序已退出但孫程序殘留的情況不再由 finalSweep 處理（只印警告與 PID 清單，由人處理）。
INTERRUPT 2026-09-24：fix round 3 實作者撞 session 額度（resets 04:20）中斷，腳本留 +128/−77 未提交；07:33 使用者指示繼續 → 續派同一實作者從中斷處接手
Task 1.2: fix round 3 implemented (commit e8d12e9; R14 落實、一手來源 Node docs／libuv src/win/process.c／Raymond Chen；S1–S4 PASS；269 ok/82 FAIL/1 PEND 與 r2 同；finalSweep 補殺 0) — Codex scoped re-review dispatched (base f05733e)
CODEX 1.2 r3（base f05733e）：ownership ADDRESSED；1 medium open：exited 但 port 仍 LISTEN 時仍 untrack，finalSweep 警告分支永不執行（四個 cleanup 路徑）；無其他新破壞（log 無 usage limit）。
Task 1.2: fix round 3/5 (1 addressed, 1 open — port-listening untrack; commit e8d12e9) → round 4 fresh opus implementer
Task 1.2: fix round 4 implemented (fresh opus; commit 7242dc2; RED 重現 2 FAIL→修後 S1–S4 PASS；全跑 277 ok/82 FAIL/1 PEND 同前；settleTrackedChild 只在 exited&&!portListening 才 untrack) — Codex scoped re-review dispatched (base e8d12e9)
CODEX 1.2 r4（base e8d12e9）：approve，No material findings（log 無 usage limit）。
Task 1.2: fix round 4/5 (1 addressed, 0 open; commit 7242dc2)
Task 1.2: complete (commits e6163f6..7242dc2 = 1cb3866, 599234f, f05733e, e8d12e9, 7242dc2; spec-check ✅ r1 + Codex approve r4)
Task 2.1: dispatched (BASE 40fd024; sonnet)
Task 2.1: implemented (commits 40fd024..2f8382e = 544471a 產品、2622bbc live-output-check、2f8382e visual-check V4＋V3；V1–V4 PASS；http.rs 6/6；六支全綠；cockpit 145/0/1；全段 372 ok/39 FAIL/1 PEND 皆屬後續 task）concerns：右欄 300px 擠；V3 banner 在 projects 之前 vs 設計清單；LO1 命中測試 FAIL 疑為工具未排除祖先裁切（carry→4.1）— review dispatched：Codex＋spec-check＋design review（R8）
REVIEW 2.1：Codex needs-attention 2 medium（C1 banner 全寬、非中欄且壓縮 Live Output；C2 live-output-check E 段改驗事件列、未逐一命中 pane／按鈕／Project 可見中心，且 cdp.click 會先 scrollIntoView）；spec-check ✅ 3 Minor（S1＝C1；S2 窄視窗順序；S3 LO1 FAIL 為 hitCenter 未交集祖先 overflow 裁切的工具限制）；design review 需修 12 條（I1＝C1；M1 #output 舊 margin 未對齊格線；M2 無 banner 時多 12px；M3 頂／底列 overflow:auto 裁掉焦點外框；M4 右欄比例綁中欄 55／45→拆三列軌道右欄 70／30；M7 單欄順序；I2 縱向捲動在 region 而非內層、框線與切角跟內容捲走、sticky 欄首不生效；I3 V1 長名稱量法禁止單行省略、與 D11 衝突；M5 捲軸色→2.2；M6 輸出窗未撐滿→4.1；M8 左欄 sticky→3.1；M9 交使用者）。
- Ruling R15：2.1 fix round 1 範圍＝C1/I1、C2、S3、M1、M2、M3、M4、M7、I2、I3。I2 雖被審核歸到 3.2，但它是外框的捲動層結構（region＝不捲的框、內層＝捲動容器），2.2 的 D8 切角與 3.2 的 sticky 都建在它上面，所以現在修；I3 是 2.1 自己的驗收段量法，改為「文字元素的可見框不超出所屬 region」，允許省略與換行（spec 原文即允許）；S3 的工具修正與 C2 共用「可見範圍＝與所有祖先 overflow 裁切框及視窗的交集」，一起修。M7 順序：頂列→Project→banner→Floor→runtime→Output→事件→底列（spec 未規定，採設計審核理由）。M5→2.2、M6→4.1、M8→3.1 帶到該 task 的 dispatch。M9 交使用者。— 代價：2.1 本輪改動面較大，需同時重跑六支與 visual-check。
USER DECISION 2026-09-24（M9）：寬 ≥1200 且高 <720 時 Factory Floor 加高度上限（約 100dvh − 頂列 − 提示 − 200px，下限 240px），第一屏要看得到 Live Output 開頭；併入 2.1 fix round 1，design.md D3 補一句（docs commit），V4 加 Live Output top < innerHeight 斷言。
INTERRUPT 2026-09-24：2.1 fix round 1 實作者撞 session 額度（resets 12:00），bb98299 docs 已提交、產品＋腳本未提交；12:01 使用者指示再試 → 續派同一實作者
Task 2.1: fix round 1 implemented (commits bb98299 docs, 1729028 產品, c761e83 live-output-check, e9b4a79 visual-check；V1–V4 PASS；六支全綠；http 6/6；gates 綠；validate 17/0；I1 邊界 1280×720 兩則提示 Floor＝240px；I3 量法依呈現方式分流、未採審核原建議)
Task 2.1: minor (deferred)：actions-check.js 偶發 flaky，實作者以未改動基準版重現同一失敗模式、判為既有時序問題（基線 1.1 時 1/1 PASS）；final review 分流，必要時開 systematic debugging。
Task 2.1: scoped re-review dispatched — Codex base 2f8382e ＋ 原設計審核員複審
CODEX 2.1 r1：usage limit（try again 12:34 PM），未審 → 12:35 自動重跑
DESIGN RE-REVIEW 2.1 r1：I1、M1、M2、M3、M4、M7、M9 ADDRESSED；I2 部分（外框不捲、內層捲已成立，但 Floor 的縱向捲動不在 .factory-floor 同一容器→sticky 基準未成立，違 R15 要求）；I3 替代量法漏判「所屬區域」（.pane-cwd x=1577–1720 落在 runtime 區 1224–1524 外仍判通過）；新 N1（1200–1280×720–761、兩則提示時 banner 84→61px 被切）；N2 右欄橫向捲軸浮在內容下 30px→3.3。carry→2.2：外框目前透明無框線，D8 切角時框線與底色移到外框。
CODEX 2.1 r1（rerun 12:35，base 2f8382e）：C1、C2 ADDRESSED；focus restoration 無新回歸；1 medium「未選取時 Live Output 仍隱藏，違常駐空狀態」（log 無 usage limit）。
- Ruling R16：Codex「Live Output 未選取時仍隱藏」不在 2.1 修 — 面板常駐與空狀態是 task 4.1 的範圍（tasks.md 4.1、Ruling R2），2.1 刻意保留舊的 display:none 以免 4.1 之前的腳本斷言提前改變 — 代價：2.1～3.4 期間中下區在未選取時是空的（已知，4.1 解）。
Task 2.1: fix round 1/5 (C1、C2、M1–M4、M7、M9、S3 addressed; open: I2 sticky 基準、I3 所屬區域量法、N1 banner 被壓扁; commits bb98299..e9b4a79) → fix round 2 續派原實作者
Task 2.1: fix round 2 implemented (commits 791d58a 產品、66bc53a visual-check；I2 捲動祖先＝.projects、I3 區域包含（cwd PENDING(3.3)）、N1 banner min-content＋Live Output r2 minmax(40px,…) 退讓約 23px；V1–V4、六支、http、gates 綠；concern：邊界下 Floor 仍可能 <240) — scoped re-review dispatched (Codex base e9b4a79＋設計審核員)
CODEX 2.1 r2（base e9b4a79）：needs-attention 2 medium：(a) banner 情境 Live Output 上半列可縮至 40px、份額隨 banner 改變，違 D3；新測試未守 Live Output 高度；(b) cwd PENDING 分支吞掉缺元素／隱藏／零尺寸等非預期回歸、未與 3.3 綁定（log 無 usage limit）。等設計審核量測表後合併 round 3。
DESIGN RE-REVIEW 2.1 r2：I2 ADDRESSED（.projects 雙向捲動）；I3 NOT（包含檢查用裁切後可見框，region 本身 overflow hidden → 恆真）；N1 NOT（banner 寫死 max-height 84px，1200 寬與 700 單欄折行後 103–104px 被裁）；高度表：Live Output 最低 245（1280／1200×720 兩則提示），一般尺寸不退讓；新 N3：.factory-floor 改 visible 後 10 stage 網格超出 .project 面板。carry→2.3：頂列高度若改要同步寫死的 token（否則 Live Output 可低至約 222）。
Task 2.1: fix round 2/5 (I2 addressed; open: 高度分配（Codex a＋N1）、I3 恆真、cwd PENDING 過寬（Codex b）、N3; commits 791d58a..66bc53a)
- Ruling R17：固定一屏的高度優先序＝banner 完整顯示（不設 max-height、不得裁切）＞ Live Output ≥240px 且高度不隨 banner 出現與否改變 ＞ Factory Floor ≥240px；三者無法同時滿足時（只可能在高約 720–761 且兩則提示折行時）由 Factory Floor 讓出、降到 240 以下並在內部捲動，報告寫出實測最小值 — D3 與 spec 都寫「提示從 Factory Floor 扣」、Live Output 是即時觀察窗不該被提示擠壓 — 代價：極端尺寸下 Factory Floor 只剩一兩列可見。
Task 2.1: fix round 3 implemented (commits ed58360 產品、218cfc7 visual-check；R17 矩陣 9 格×0/1/2 提示：Live Output 高度恆定、Floor 最低 240.00；N3 改 render.js 算術 min-width（max-content 會因 flex-wrap 視同 nowrap 撐寬）；I3 未裁切包含＋position:fixed 否定對照；六支、V1–V4、http、gates 綠）
carry→3.3：新 I3 量法下 cwd 因落在右欄捲動容器內而判 contained，STRICT_3_3 旗標因此失去牙齒；3.3 驗收須另斷言 runtimes 區內層無橫向捲動（scrollWidth ≤ clientWidth）且長 cwd 以省略或換行呈現、title 帶全文（spec「長名稱不溢出」＋D11）。
Task 2.1: scoped re-review dispatched (Codex base 66bc53a＋設計審核員)
CODEX 2.1 r3（base 66bc53a）：高度分配 ADDRESSED；cwd PENDING NOT（viaScroll 例外被溢出元素自己撐大的 scrollWidth 作證 → 仍恆真，strict 失效）；新 medium：render.js 算術 min-width 把 minmax(160px, auto) 首欄當 160，長 binding ID 撐寬後網格再超出面板（log 無 usage limit）。
- Ruling R18（換設計）：長名稱包含檢查連續三輪同類恆真 → 改為最簡判準：先把所有捲動容器 scrollLeft／scrollTop 歸零，只比水平方向，文字元素未裁切 rect 的 [left,right] 必須落在所屬 data-region 未裁切 rect 的 [left,right] 內，不設任何捲動例外；Factory Floor 的 stage 欄（spec 允許區內橫向捲動）不列入長名稱目標，只驗 workstream 名稱（首欄）與 pane 標題／cwd。cwd 目前會 FAIL → 預設印 PENDING(3.3)，STRICT_3_3 下硬失敗 — handover「開發者腳本同類 finding 第三輪還在冒就換設計」— 代價：縱向溢出不在此檢查內（spec 此 scenario 只談寬度與橫向捲軸）。
DESIGN RE-REVIEW 2.1 r3：I3 ADDRESSED（依可捲到＝不溢出定義；但 withinRegion 對流內元素近恆真 → 由 R18 取代）；N1 NOT（≥1200 段 floor min-height:240 未覆寫，Floor 未依 R17 讓出）；N3 ADDRESSED（10／3 stage × 1536／1200／700 面板左右恆 17px）；抽測一致；新 N4 Important：固定一屏下緣兩則提示時頂列 47→26px、.shell 溢出 2–40px、底列被裁出視窗、1200×720 Live Output 下緣超出 6px。
Task 2.1: fix round 3/5 (高度 banner／Live Output、N3 addressed; open: N1 Floor 未讓出、N4 頂底列被壓、Codex 算術 min-width 忽略 auto 軌道、containment 恆真（R18 換設計）; commits ed58360..218cfc7) → round 4 fresh opus
INTERRUPT 2026-09-24：2.1 fix round 4（opus）撞 session 額度（resets 17:00），4 檔未提交；17:11 使用者指示繼續 → 續派同一實作者
Task 2.1: fix round 4 implemented (fresh opus; commits df75f8e 產品、b581a99 visual-check、f790b4c live-output-check；V1–V4 PASS 392 ok、PENDING 三條（4.1 Live Output 可見、3.3 pane 標題、3.3 pane cwd）；STRICT_3_3 下 V1 FAIL(2) 如預期；六支 PASS；cockpit 156/0/1；Floor 讓出後最小 156.97px；Codex「網格比面板寬」實測不成立，真因為長 ID 撐首欄至 496px → 首欄上限 240＋省略）
ROOT CAUSE：actions-check 間歇失敗＝2.1 自身產品迴歸（整頁 replaceChildren 把內層捲動容器歸零，HEAD 上 4 次失敗 3 次）；paint() 改為記下並寫回（SCROLL_KEEP_SELECTORS）後 5/5；live-output-check R 段舊斷言「重畫後 scrollTop===0」是把迴歸寫成預期值 → 改「捲動位置不變」＋拿掉 preventScroll 的對照會 FAIL。memory 已寫（full-repaint-resets-inner-scroll-containers，控制端核過：記機制）。取代先前「actions-check flaky 為既有時序問題」的 deferred minor（該判斷錯誤）。
- Ruling R19：把「整頁重畫不重置區塊內部捲動位置」補成 dashboard delta spec 的 scenario（畫面整頁重畫 requirement 下）— 版面改為區塊內捲動後這是新的必要行為，舊版靠整頁捲動自然成立；不寫進 spec，後續 task 改 paint() 或新增捲動容器時沒有規格守 — 代價：spec 多一個 scenario，visual-check V1 已有對應斷言。
carry→3.3：pane 標題與 cwd 皆 PENDING(3.3)，3.3 驗收以 VISUAL_CHECK_STRICT_3_3=1 跑、兩者須轉綠；另加 runtimes 內層無橫向捲動斷言。
Task 2.1: scoped re-review dispatched (Codex base 218cfc7＋設計審核員)；R19 spec 補寫 dispatched
R19 spec: 4e34124（scenario「重畫不重置區塊內部捲動位置」＋tasks 2.1 驗收；validate 17/0、markdownlint 80/0；控制端 read-back 核過）
DESIGN RE-REVIEW 2.1 r4：N1、N4 ADDRESSED（1200×720 兩則長提示：頂列 47、banner 144、Floor 156.97、Live Output 268.03、底列下緣 708、.shell 720/720；與報告一致）；首欄 240＋省略合理；157px 可接受（R17 代價、暫態）；新退步 0；內層捲動保留實測有效。carry→3.2（Minor）：runtime 名稱長時省略號吃掉 pane id，拆兩個 span、只縮 runtime。
CODEX 2.1 r4（base 218cfc7）：approve，No material findings（scroll restore 與 preventScroll 焦點還原同步、不互相覆寫；R 段斷言仍具辨識力；log 無 usage limit）。
Task 2.1: fix round 4/5 (all addressed; commits df75f8e, b581a99, f790b4c)
Task 2.1: complete (commits 40fd024..f790b4c；spec-check ✅、Codex approve r4、design review r4 通過；4 輪修正)
Task 2.2: dispatched (BASE 190bafb; sonnet)
Task 2.2: implemented (commit d913584 style.css＋manifest；G2/RM1/FN1 RED→GREEN；V1–V4 0 FAIL；六支 PASS；gates 綠；concerns：舊狀態色留給 2.3/3.x、10/11/15/16px 字級未收斂進 token、刻度靜態骨架→3.2、pane-row focus outline-offset 改外推→3.3 覆核) — review dispatched：Codex＋spec-check＋design review（fresh opus）
CODEX 2.2（base 190bafb）：needs-attention；high：舊 GitHub 色盤 token（#3fb950/#58a6ff/#a371f7 等）仍被 task／連線／pane／選取樣式實際引用，違十色契約與「既有規則改為取用 token」；medium：.app-name 16、.project-name 15、.ff-binding-badge 11、.task-status-label 10px 未收斂 D10 且中文 <12px（log 無 usage limit）
SPEC-CHECK 2.2：✅ 1 Important（報告未揭露殘留：.ff-binding-badge rgba(230,230,230,.14)、#0f1115 文字、.pane-row.selected rgba(88,166,255,.16)；判歸 3.2／3.3）、2 Minor（--font-mono 只掛 .output-text，時間/id/數值歸 3.3；RM1 check 文字寫死「預期 FAIL」）；認為殘留皆可追溯到後續 task — 與 Codex 判斷相反，待裁決
DESIGN REVIEW 2.2：需修 8 條；Tab 巡 45／43 個元素焦點外框未被 clip-path 裁。F1 Important 切角補線偽元素以 padding box 定位、clip-path 以 border box → 浮在框內 2.5px、兩端缺口；F2 拿掉全大寫後 0.04em 字距殘留；F3 events 框 6px 圓角／12px 內距與其他不一致；F4 面板標題用 --fs-dense 應為 --fs-panel 14/600；F5 捲軸軌道 --bg-base 多一條色帶→transparent；F6 中性按鈕疊在未換皮彩色節點上對比 1.2–1.6:1→3.2 驗收列按鈕對比；F7 region 框內 .project／.runtime-card 框中框→3.2／3.3；F8 交使用者：D8 標題列右側偽元素刻度對不齊會橫向捲動的 stage 欄，且偽元素已用於補線 → 建議刻度改為 stage 欄首上緣 DOM（改 D8）。
- Ruling R20：採 Codex（2.2 現在收斂），不採 spec-check「殘留歸後續 task」— 任務原文「把既有規則改為取用 token」、spec「唯一色彩來源」；設計審核 F6 顯示新舊色盤混用已造成 1.2–1.6:1 按鈕對比。範圍：style.css 所有顏色改用 10 個 token 或由其推導（color-mix／token 透明度），舊狀態／連線／stage／tint token 移除或改指向核心 token，對應依 D4 對照表；所有 font-size 改用 --fs-* 四階（產品名 14、Project 名 20、badge／status 12 或 13，中文不低於 12）；後續 task 只改結構（符號、色條、外框），不得再引入色值或字級。新增 visual-check 靜態段：style.css 除 :root 的 10 個色彩宣告外無 hex／rgb 字面值、font-size 只能 var(--fs-*) — 代價：2.2 到 3.x 之間部分區塊的狀態呈現是「舊結構＋新色」，看起來半套。
Task 2.2: fix round 1 → 續派原實作者（Codex high＋medium、設計 F1–F5、spec-check Important 報告揭露補記、Minor RM1 文字）
USER DECISION 2026-09-24（F8）：Factory Floor 刻度移到每個 stage 欄首上緣、以 DOM 實作、隨欄橫向捲動；running 的 stage 刻度較長、--text；標題列右側不再有刻度；3.2 實作。design D8／tasks 更新 dispatched（sonnet）。
D8 update: 6780ceb（刻度改 stage 欄首上緣 DOM；tasks 2.2 只留切角、3.2 實作刻度；validate 17/0、markdownlint 80/0；控制端 read-back 核過）
Task 2.2: fix round 1 implemented (commits e815fc8 style.css、75d347f visual-check TK1＋factory-floor-check EXPECTED_COLORS；舊調色盤全指向 10 token、字級收斂 --fs-*；TK1 兩負對照命中、舊 style.css 抓到 4 筆；F1 補線改 border-box 漸層；F2–F5 修；G2/RM1/FN1/TK1 428 ok、V1–V4 392 ok、六支綠、http/pwa 綠)
carry→3.2：標題列右側的靜態刻度骨架（2.2 留下）依新 D8 要移除，改在 stage 欄首上緣以 DOM 實作；F6 按鈕對比列入 3.2 驗收；F7 框中框（.project）。carry→3.3：F7（.runtime-card）；--font-mono 用到時間／id／數值；pane-row focus outline-offset 外推覆核。
Task 2.2: scoped re-review dispatched (Codex base d913584＋設計審核員)
CODEX 2.2 r1（base d913584）：舊色盤、字級 ADDRESSED；新 high：.pane-row.selected 底色 color-mix accent 16% 違 D4（應 --surface＋左緣 accent）；medium：改綁提示映射成 --warn 違 D4（應 --accent）；medium：TK1 可被 hsl／oklch／named color／重複核心宣告繞過（log 無 usage limit）。TK1 修法不得新增 CSS parser 套件（鐵則：新套件先問）→ 宣告層級 regex＋允許清單。
DESIGN RE-REVIEW 2.2 r1：F1–F5 ADDRESSED（8 倍放大四角接上、Tab 45 個裁 0）；收斂後文字與膠囊對比合格；新 N1 Important：節點上動作按鈕 1.0–1.74:1、pending 節點三顆按鈕 1.00:1 看不見 → 2.2 補暫時規則（按鈕文字與外框 --bg-deep，6.69–15.76:1），3.2 換皮時刪除。carry→3.2／3.3（5 項觀察）：ready 近白比 running 亮、冰青用量過多、done 膠囊輪廓消失、idle 與 unknown 同色、狀態字與標題層次變平。
Task 2.2: fix round 1/5 (Codex 舊色盤、字級 addressed；設計 F1–F5 addressed; open: selected 底色、改綁色、TK1 可繞過、N1 按鈕對比; commits e815fc8..75d347f) → fix round 2 續派原實作者
Task 2.2: fix round 2 implemented (commits b259936 style.css、e639152 visual-check；selected→surface＋accent 左緣；rebind→accent；TK1 宣告層級逐一計數＋非 hex/rgb 色彩語法與約 50 具名顏色、六負對照；節點按鈕暫時 --bg-deep、G2 29 顆 ≥4.5:1（6.69–15.76）；全綠) — scoped re-review dispatched (Codex base 75d347f＋設計審核員)
CODEX 2.2 r2（base 75d347f）：selected、rebind ADDRESSED；按鈕規則與對比斷言到位；TK1 NOT：大寫 HSL()/OKLCH()（indexOf 區分大小寫、RGB_RE 無 i）、rebeccapurple（具名清單不全）、第二個 :root 的 --ok: var(--bad)（只解析第一個 :root）皆回 ok:true（log 無 usage limit）。
- Ruling R21：TK1 定型，不再逐條補洞：(a) 待檢內容先全部轉小寫；(b) 具名顏色改用 CSS Color 4 的完整具名色清單（固定集合，寫死在程式碼、註明來源），不是「常見色」；(c) 解析全檔所有 :root 區塊，核心 10 token 全域恰好各一次；任何選擇器裡重新宣告核心 token 名稱（--bg-deep 等）一律 FAIL；(d) 色彩函式黑名單不分大小寫。四個注入案例（HSL、OKLCH、rebeccapurple、第二個 :root 覆寫）＋先前六個負對照全部必須 FAIL — 同類 finding 第二輪，handover 規則是第三輪換設計；這次一次補齊到「規格上的有限集合」為止，避免第三輪 — 代價：若 Codex 第三輪再挖出同類繞過，改採使用者裁決（整段 TK1 降為 Minor 靜態輔助、不擋驗收）。
DESIGN RE-REVIEW 2.2 r2：N1 ADDRESSED（pending 9.04、running 10.85、failed 6.69，12 節點 ≥4.5）；選定列符合 D4；新退步：改綁提示四邊冰青框與焦點外框、running 撞形 → 改 --line 框＋2px 冰青左緣＋--surface 底；既有問題：running 節點按鈕聚焦時冰青外框落在冰青底上 1:1 → 暫時規則補 outline-color --bg-deep，3.2 一起刪。
Task 2.2: fix round 2/5 (selected、rebind 色、N1 addressed; open: TK1（R21）、改綁提示形狀、running 節點按鈕焦點外框; commits b259936..e639152) → fix round 3 續派原實作者
Task 2.2: fix round 3 implemented (commits 200138b style.css、bc77018 visual-check；TK1 定型 R21：小寫、CSS Color 4 完整 148 色（W3C 查證）、全檔 :root 全域計數＋非 :root 重宣告 FAIL、十個負對照全 FAIL；改綁提示 --line 框＋2px accent 左緣＋surface 底；running 按鈕 focus outline --bg-deep 10.85:1；全綠) — scoped re-review dispatched (Codex base e639152＋設計審核員)
CODEX 2.2 r3（base e639152）：原四 TK1 漏洞 ADDRESSED；新 medium：extractDeclarations 只收以 ; 結尾的宣告，區塊最後一筆省略分號（合法 CSS）時核心 token 覆寫與非法具名色皆 ok:true（log 無 usage limit）。同類第三輪 → 依 R21 代價條款交使用者裁決。
DESIGN RE-REVIEW 2.2 r3：改綁提示形狀、running 按鈕焦點外框皆 ADDRESSED；新退步 0。carry→3.4：6px 圓角讓冰青左條彎成括號形；錯誤提示仍四邊框、應收斂成左緣條家族。
USER DECISION 2026-09-24（TK1 第三輪）：再修「區塊最後一筆無分號」這一個就收；之後 Codex 若再找到同類 TK1 繞過，直接列 Minor、不擋驗收。
Task 2.2: fix round 3/5 (改綁提示形狀、running 按鈕焦點外框、TK1 原四漏洞 addressed; open: TK1 無尾分號; commits 200138b..bc77018) → round 4 fresh opus
Task 2.2: fix round 4 implemented (fresh opus; commit 97d2235 visual-check；宣告解析接受區塊結尾；另修 font-size 同樣要求 ;、含數字 custom property 名稱漏掃（真實 --shell-banner-1 一直沒被掃到）、nesting 父層宣告被跳過、字串／括號內 ; } 切錯；原 10＋新 7 負對照全 FAIL；G2/RM1/FN1/TK1 88 ok、V1–V4 392 ok、六支綠、gates 綠)
Task 2.2: minor (deferred)（依使用者 TK1 裁決）：stripCssComments 不認字串（字串內 /* 會吞內容）；font: 簡寫帶字級可繞過字級檢查。
Task 2.2: scoped re-review dispatched (Codex base bc77018)
CODEX 2.2 r4（base bc77018）：approve，No material findings（log 無 usage limit）。
Task 2.2: fix round 4/5 (all addressed; commit 97d2235)
Task 2.2: complete (commits 190bafb..97d2235 = d913584, e815fc8, 75d347f, b259936, e639152, 200138b, bc77018, 97d2235；spec-check ✅、Codex approve r4、design review r3 通過)
Task 2.3: dispatched (BASE 3202b68; sonnet)
Task 2.3: implemented (commit c47005f render.js/style.css/index.html；R1 topbar/statusbar RED→GREEN；--shell-topbar-h 47→45；onChannel 只改 #channel-status 文字子節點；TK1/R1/V1–V4/RM1/FN1/G2 綠；六支綠；reconnect 序列 connected→disconnected→connected) concern：組合跑時 topbar 燈號偶發 FAIL 一次，實作者判環境雜訊 — 控制端不採信（前例：actions-check「環境 flaky」實為產品迴歸），交 spec-check 重複跑重現 — review dispatched：Codex＋spec-check＋design review（fresh opus）
CODEX 2.3（base 3202b68）：needs-attention；high：renderChannelIndicator 每次重畫硬編 connected、onChannel 未保存狀態 → 斷線後任何 repaint 偽裝成 connected；medium：visual-check R1 以 .runtime-cards 判定首份投影畫出，但 index.html 靜態占位已含該 class → 可解釋 topbar 偶發 FAIL；medium：--shell-topbar-h 45px 寫死，多 runtime／長 id 會讓頂列折行、高度計算失準（log 無 usage limit）
SPEC-CHECK 2.3：✅ 1 Minor（手動重連以 reconnect-check.js 自動觀察替代）；偶發失敗 R1×10＋組合×5 共 15 次未重現、讀碼未找到 onChannel／repaint 競態；--shell-topbar-h 27 筆矩陣皆 45。
DESIGN REVIEW 2.3：需修；I1 Important：停掉 preview 後頂列燈號停在最後投影（綠 connected），1280×650 底列 top=724、700×900 top=1820 → 第一屏無斷線訊號 → 整頁捲動版面底列 sticky bottom:0；I2 交使用者（斷線時頂列燈號調暗標最後已知）；M1 480／390 寬產品名斷行、燈號列距 16px → nowrap＋row-gap 4px；M2 頂列 45 vs 目標 40、有框 vs 底列無框；M3 交使用者（灰階 ok／warn 1.05:1 → 三種形狀）；M4 燈號 title 只重複畫面字、底列無 title。
USER DECISION 2026-09-24：I2 採「通道斷線期間頂列燈號改 --text-dim 並在狀態字前加『最後已知』，恢復連線即還原」（spec 補 scenario）；M3 採「connected 實心圓、connecting 空心圓、disconnected 叉（CSS 畫），顏色照舊，頂列與底列共用」（改 D4）。
- Ruling R22：偶發 topbar FAIL 採 Codex 機制（index.html 靜態占位已含 .runtime-cards，R1 等待條件在首份投影前即成立），不採 spec-check「無已知因果」— 15 次未重現與「依 WebSocket 快慢的時序」一致，機制具體可驗 — 修法：visual-check 全檔的「首份投影已畫出」等待改為動態條件（topbar 出現 [data-runtime] 節點且 #version 等於 /api/state 的 version），並以「刻意延遲首份推送」的負對照證明舊條件會提早通過 — 代價：visual-check 多處等待點要一起改。
- Ruling R23：M2 不修，記 deferred minor — 純外觀微調且會牽動 --shell-topbar-h 與高度矩陣 — 代價：頂列比目標高 5px、上下列框線不對稱。
- Ruling R24：接受以 reconnect-check.js 取代「手動停止／重啟」— 該腳本正是停止再重啟 preview 並觀察通道狀態序列 — 代價：無。
Task 2.3: fix round 1 → 續派原實作者（Codex C1–C3、設計 I1、M1、M4、使用者 I2／M3）；artifact 更新（I2 spec scenario、D4 形狀）平行派出
2.3 artifacts: 63fcb93（spec 補最後已知規則＋scenario「通道斷線時 runtime 燈號標示為最後已知」、D4 三形狀、tasks 2.3；validate 17/0、markdownlint 80/0；控制端 read-back 核過）
Task 2.3: fix round 1 implemented (commits adea947 產品、672bf58 visual-check：21 處等待改 waitForFirstProjection＋S5 負對照、CH1(a–d)、V3/V4 因底列 sticky 調整重疊／順序斷言＋第一屏可見性斷言；CH1(a) 還原舊寫法確認 RED；全綠) — scoped re-review dispatched (Codex base c47005f＋設計審核員)
CODEX 2.3 r1（base c47005f）：C1 通道持久化、C2 首份投影等待、C3 頂列高度 ADDRESSED；新 medium：.runtime-lamp-stale 設 --warn 違 spec「一律 --text-dim」，CH1 只讀父層色所以漏；新 medium：V3/V4 移除 statusbar 順序與重疊檢查後只驗初始在視窗內，fixed／absolute 皆可過、沒讀 computed position、沒捲動後再量（log 無 usage limit）
DESIGN RE-REVIEW 2.3 r1：I1、M1、M4 ADDRESSED；三形狀 8px 與灰階可分；最後已知一眼可懂；sticky 底列捲到底不蓋內容；新 Minor：N1 固定一屏燈號過多時最左一顆連 id 被無聲裁掉（1200×720、5 runtime、斷線）；N2 id 固定截在 12ch、同前綴 id 變一樣；N3＝Codex medium（最後已知用 --warn）；N4 窄寬斷線時「最後已知」斷兩行；N5 斷線期間燈號 title 仍寫原狀態。
Task 2.3: fix round 1/5 (C1–C3、I1、M1、M4 addressed; open: 最後已知色（Codex＋N3）、V3/V4 sticky 斷言過寬（Codex）＋Minor N1、N2、N4、N5 順帶修; commits adea947..672bf58) → fix round 2 續派原實作者
- Ruling R25：本輪既然要修兩條 medium，把設計複審的 4 條 Minor（N1、N2、N4、N5）一起修 — 都是小改、使用者看得到 — 若之後只剩 Minor，不再開輪，記 deferred — 代價：本輪 diff 稍大。
Task 2.3: fix round 2 implemented (commits 053e81d 產品、f555b6e visual-check；最後已知 text-dim（還原 --warn 確認 CH1 RED）；+N 徽章；拿掉 12ch；nowrap；title 跟通道更新；checkStickyStatusbar 共用＋fixed/static 負對照；除錯：flex-end 往起始方向溢出 scrollWidth 量不到 → 改比 rect.left；全綠) — scoped re-review dispatched (Codex base 672bf58＋設計審核員)
CODEX 2.3 r2（base 672bf58）：最後已知色、sticky 斷言 ADDRESSED；新 3 medium 皆來自 +N 徽章：resize 與 onChannel 不重算、空徽章量測後填 +N 會再裁掉保留燈號（測試比 viewport 非 container）、收進徽章的 runtime 鍵盤無法取得（log 無 usage limit）
DESIGN RE-REVIEW 2.3 r2：N2–N5 ADDRESSED；N1 NOT（斷線不重畫、+N 只在重畫時算 → 加「最後已知」變寬後再被無聲裁掉；縮放亦同）；新 R2-1 Important：斷線的 runtime 被收進灰色 +1，頂列只剩綠燈；R2-2 Minor：390 寬、長 id、斷線時燈號撞產品名。
Task 2.3: fix round 2/5 (最後已知色、sticky 斷言、N2–N5 addressed; open: +N 徽章三 medium＋N1＋R2-1、R2-2; commits 053e81d..f555b6e)
- Ruling R26（換設計）：拿掉 +N 徽章與所有 JS 溢出量測，改純 CSS：固定一屏時燈號一律全部顯示、不隱藏任何一顆；空間不足時先縮 id（flex、min-width:0、ellipsis，完整在 title），符號與狀態文字（含「最後已知」）不縮；仍放不下則燈號列在頂列內橫向捲動（可見、不無聲裁切，頂列高度不變）；窄版面（<760）頂列允許換行，產品名與燈號不得重疊 — +N 機制衍生 resize／onChannel 重算、二次量測、鍵盤可達三個問題且會藏住斷線的 runtime（R2-1），KISS 下純 CSS 無這些問題 — 代價：runtime 極多時要橫向捲頂列才看得到全部（實際使用只有 2 個）。
Task 2.3: fix round 3 → 續派原實作者（最後一輪）
Task 2.3: fix round 3 implemented (commits 5360140 產品、31594ce visual-check；+N 徽章與 JS 量測移除、純 CSS 三層；--shell-topbar-h 45→57px（疑為 overflow-x: scroll 常駐捲軸）；style.css 內寫入除錯記錄註解；全綠) — scoped re-review dispatched (Codex base f555b6e＋設計審核員)
CODEX 2.3 r3（base f555b6e）：+N 三項結構性消失；medium：overflow-x: scroll 常駐捲軸，正常兩 runtime 也 45→57px（Floor 約 −6.6px、Live Output 約 −5.4px）；next steps：style.css 1575–1670 除錯歷程約 12.4KB 應移出，只留策略與 invariant（log 無 usage limit）
DESIGN RE-REVIEW 2.3 r3：N1、R2-1、R2-2 ADDRESSED；57px 量測：無溢出時常駐 15px 空捲軸軌道、Floor −6~7、Live Output −5~6（1280×720 剩 264）；建議（已注入實測）：overflow-x:auto＋頂列寫死 45px、上下 padding 0、align-items:stretch、產品名 align-self:center、token 回 45；新 R3-1 Important：id 被拉滿 200px、win 與 connected 間空 104px 易誤讀 → .runtime-lamp-id { flex: 0 1 auto }；R3-2 Minor＝常駐捲軸；R3-3 Minor：斷線時 id 被壓成「runtim…」看不到編號。
Task 2.3: fix round 3/5 (+N 三項、N1、R2-1、R2-2 addressed; open: 常駐捲軸 57px（Codex＋R3-2）、R3-1、R3-3、style.css 除錯註解 12.4KB; commits 5360140..31594ce) → round 4 fresh opus
Task 2.3: fix round 4 implemented (fresh opus; commits ba5ebe6 產品、9185685 visual-check；頂列 45px 無捲軸；1536×1024 Floor 481.3／Live Output 405.8，1280×720 314.1／268.9；id→狀態間距 109.5→6px；斷線 id 75.5px（燈號 min-width 220px 推算 10ch）；12KB 除錯歷程移報告附錄；新斷言對舊 CSS RED 6；全綠) — scoped re-review dispatched (Codex base 31594ce＋設計審核員)
DESIGN RE-REVIEW 2.3 r4：R3-1（id 21px、間距 6px）、R3-2（45px 無捲軸；Floor/LO 481/406、314/269）、R3-3（斷線 id 76px 讀得到編號）ADDRESSED；45px 下無裁切；燈號間 140px 空白接受（附條件）；新 R4-1 Minor：最後一顆燈號尾端空 108px、整組沒貼齊右緣 → 固定一屏斷點 .runtime-lamp { justify-content: flex-end }（注入實測右緣 x=1507）。
Task 2.3: minor (deferred)（R25：只剩 Minor 不再開輪）：R4-1 燈號組沒貼齊頂列右緣 → 帶到 5.1 清理一併處理；M2（R23）頂列框線不對稱。
CODEX 2.3 r4（base 31594ce）：常駐捲軸、除錯註解 ADDRESSED；新 2 medium：220px 燈號下限寫死、fallback 字型／文字縮放時 id 仍被壓到 <10ch；45px 固定高度（捲軸時 client 約 28px）在文字放大時裁掉燈號，app-name 可能溢出，測試未涵蓋（log 無 usage limit）。
Task 2.3: fix round 4/5 (常駐捲軸、除錯註解、R3-1、R3-2、R3-3 addressed; open: 220px 與 45px 對字型／文字縮放的依賴; commits ba5ebe6..9185685)
- Ruling R27：第 5 輪（上限）只做一件事：把燈號下限與頂列高度改成隨字級縮放的單位（rem／ch／em，由 line-height、字級、捲軸 gutter 推導），--shell-topbar-h 用同一個算式，不再寫死 px；測試加「根字級 200%」情境（模擬文字縮放）與較寬 fallback 字型各一，斷言燈號與產品名完整落在頂列內、id ≥10ch 或改走橫向捲動。續派 round-4 的 opus（R≥4 已是升級後的實作者、熟悉此段）— spec 未明文要求文字縮放，但修法小、可一次補齊 — 代價：若第 5 輪後仍有 finding，依斷路器逐條裁決。
Task 2.3: fix round 5 implemented (commits 03dd764 產品、339b1a5 visual-check；頂列高度＝line-height 1.4×字級＋24px padding＋2px 框線，@property 註冊，預設 45.6px、字級加倍 65.2px；220px 拿掉、id 下限 min(10ch, 文字寬)（calc-size() 僅 Chromium）＋line-clamp；CH1 (f) 對 round-4 CSS RED 3；兩負對照 FAIL；(e) flex 負對照改 round-3 完整寫法；R4-1 順帶貼齊右緣；concern：底列 32px 仍寫死；全綠) — final scoped re-review dispatched (Codex base 9185685＋設計審核員)
DESIGN RE-REVIEW 2.3 r5：R4-1 ADDRESSED（燈號各佔內容寬、右緣 x=1507）；預設字級無退步（逐列讀像素框線單列純 --line）；字級加倍頂列 65.2px 完整；新退步 0。carry→5.1（範圍外觀察）：連線符號固定 8px，字級加倍時偏小 → 改 em。
CODEX 2.3 r5（base 9185685）：220px 下限、45px 固定頂列 ADDRESSED；複雜度相稱；id-gap 負對照未放寬；新 medium：寬 ≥1200 且高 <720 時頂列不套固定高度／nowrap、可多行，但 M9 的 Factory Floor max-height 仍用單行 token 扣 → 文字放大或多長 id 時 Live Output 可能被推出第一屏；測試未涵蓋 1200×719（log 無 usage limit）。
Task 2.3: fix round 5/5 (220px、45px addressed; open: 寬矮版面 token 只代表單行高度; commits 03dd764..339b1a5)
Task 2.3: parked — 寬矮版面（≥1200、<720）頂列可多行但高度上限用單行 token — Ruling R28：斷路器裁決「真實但不擋後續」：只在寬矮＋文字放大或多個長 id 同時成立時發生，後續 task 不依賴頂列在寬矮版面的行為；帶到 5.1 以最小修法處理（≥1200 不論高度，頂列一律套固定 token 高度＋nowrap＋燈號列橫向捲動），並補 1200×719、200% 字級、五長 id、disconnected 的測試（實際頂列高＝扣除高、Live Output 開頭在第一屏）— 代價：2.3～5.1 之間此邊界情境 Live Output 可能落出第一屏。
Task 2.3: minor (deferred)：底列高度仍寫死 32px（文字放大可能被裁）→ 5.1；連線符號固定 8px（字級加倍偏小）→ 5.1 改 em。
Task 2.3: complete (commits 3202b68..339b1a5；5 輪修正、1 parked（R28）；spec-check ✅、design review r5 通過、Codex r5 僅餘 parked 項)
Task 3.1: dispatched (BASE 884e658; sonnet)
Task 3.1: implemented (commits 397bb84 產品、05bc9ee 腳本：factory-floor／actions／live-output／visual-check 因 Floor 收斂成單一 Project 改寫；P1 RED→GREEN；全綠；concerns：sticky 左欄蓋住 scrollIntoView 目標→scroll-margin-top；200 字 Project 名無自動化測試) — review dispatched：Codex＋spec-check＋design review（fresh opus）
CODEX 3.1（base 884e658）：needs-attention 3 medium：(1) resolveSelectedProject 只在 render 時 fallback，actions.js 保留舊 selectedProject → p2 消失再出現會自動跳回 p2，違「改為選定第一個」；(2) actions-check.js 10 次頻繁重畫驗收被改成 7 次（cockpit 只有 7 task）→ 失去涵蓋；(3) P1 在 409 已顯示後才切 Project，無法證明 select-project 不遞增 latestOp（需延遲失敗、在回應前切換）。其餘三支腳本改寫皆先切 Project、未失涵蓋；sticky／scroll-margin 未見衝突（log 無 usage limit）
DESIGN REVIEW 3.1：需修；定案皆達成（surface＋左緣、D11 排序、0 不顯示、無中點、tabular-nums、空狀態文案逐字）。I1 Important：<760 單欄左欄 sticky top:0，Project 多時佔 360px＋sticky 底列＝900 高的 44% → 單欄不 sticky、拿掉 scroll-margin-top:40vh；M1 兩欄 sticky top:0 貼視窗上緣、max-height:40vh 過短 → top:12px、max-height 扣間距與底列；M2 200 字名稱換行約 17 行 → 限 2 行＋title＋斷言；M3 chip「‖ blocked」的 ‖ 像分隔線 → chip 間距 ≥16px；M4 空狀態左欄文案與 Floor 幾乎重複 → 左欄只寫「沒有 Project」、P1 精確字串同步；M5（交 3.2）◇ 約其他符號一半大，3.2 截圖確認含左欄 chip；3.2 完成後再拍 running 節點按鈕焦點。注意：task-5.2-scenario-d.png 顯示已修改（非此座位），待 spec-check 收工後還原。
SPEC-CHECK 3.1：✅ 2 Minor（[data-action] scroll-margin-top:40vh 為全域選擇器；D6 改綁模式只驗 DOM）；判既有腳本失涵蓋 0 — 但把 actions-check 10→7 判為 (b)，與 Codex 相反。控制端查主 spec cockpit-dashboard「連續按 10 次不同 task 的 Completed → 收到 10 個對應 POST」→ 採 Codex：失去涵蓋。png 已由 spec-check 還原，git status 乾淨。
- Ruling R29：3.1 fix round 1 範圍＝Codex 3 條（fallback 要寫回 selectedProject；actions-check 恢復 10 次跨 Project；P1 補 in-flight 失敗後切 Project 的 latestOp 回歸）＋設計 I1、M1–M4（拿掉 40vh 全域 scroll-margin-top 同時解 spec-check Minor 1）；M5 交 3.2 — 代價：無。
Task 3.1: fix round 1 → 續派原實作者
Task 3.1: fix round 1 implemented (commits 8a10547 產品、86f8d60 actions-check＋visual-check；setSelectedProject 由 paint() 正式回寫第一個；單欄不 sticky、拿掉全域 40vh；兩欄 top:12px＋align-self:start（根因：grid stretch 撐高左欄）；名稱 2 行 clamp；chip gap 16；左欄空狀態「沒有 Project」；actions-check 恢復 10 次跨 Project；P1 in-flight 1500ms 延遲 409 後切換仍顯示錯誤；全綠) — scoped re-review dispatched (Codex base 05bc9ee＋設計審核員)
CODEX 3.1 r1（base 05bc9ee）：10-click、in-flight latestOp ADDRESSED；paint 回寫不循環、非 D6 畫面操作；fallback NOT：空 Project 投影時 resolvedForPersist 為 null 不回寫 → 恢復 [cockpit,p] 會跳回 p；缺 p→[]→[cockpit,p] 測試（log 無 usage limit）
DESIGN RE-REVIEW 3.1 r1：I1、M1–M4 ADDRESSED（700 寬遮擋 0、捲動後 top=12、名稱 2 行、‖ 前後 21/8px、左欄「沒有 Project」）；拿掉 40vh 後六版面無遮擋；新 N1 Important：頁面在頂端時左欄從 y=70 起、max-height 用貼頂算式 → 底緣 914 且 z-index:1 蓋住底列通道狀態（≥9 個 Project；760、1100、1199、1280×650、1536×700 重現；1280×650 超出視窗 14px）。
Task 3.1: fix round 1/5 (Codex 10-click、in-flight、設計 I1、M1–M4 addressed; open: 空投影 fallback、N1 左欄蓋住底列; commits 8a10547..86f8d60) → fix round 2 續派原實作者
- Ruling R30：N1 兩個都做：底列 z-index 高於左欄（sticky 底列是「永遠看得到通道狀態」的承諾，任何區塊都不得蓋住它）＋左欄 max-height 改成在頁面頂端時也不超出視窗（扣頂列、底列與間距）— 只做 z-index 會讓左欄底部被底列蓋住、只做高度在其他組合仍可能疊到 — 代價：無。
Task 3.1: fix round 2 implemented (commits 9d56d49 產品、461a785 visual-check；空投影正規化 null＋p→[]→[cockpit,p] 回歸；statusbar z:2 > projects z:1；左欄 max-height＝100dvh−topbar−36px−statusbar；V1 N1 子區塊 5 尺寸×捲動前後；stash 對 r1 RED→GREEN；全綠) — scoped re-review dispatched (Codex base 86f8d60＋設計審核員)
CODEX 3.1 r2（base 86f8d60）：approve，No material findings（空投影 ADDRESSED；max-height 只在可捲動版面、不影響固定一屏；左欄底緣受限於底列上方，焦點不被遮；log 無 usage limit）
DESIGN RE-REVIEW 3.1 r2：N1 ADDRESSED（五尺寸左欄底緣距底列 12px、捲動取樣 85 次被蓋 0）；最後一項焦點外框完整；固定一屏左欄 898px 未變短；新退步 0；觀察：左欄貼頂時下方留白 58–70px（R30 代價）。
Task 3.1: fix round 2/5 (all addressed; commits 9d56d49, 461a785)
Task 3.1: complete (commits 884e658..461a785；spec-check ✅、Codex approve r2、design review r2 通過)
Task 3.2: dispatched (BASE cb90635; sonnet)
Task 3.2: implemented (opus; commits e739df5 render.js/style.css/design.md（D4 ready 符號 ◇→♢）、f6d9693 factory-floor-check DOM 判斷＋visual-check G1；G1 RED 7→GREEN；890 ok／1 FAIL（R1 runtimes 屬 3.3）；factory-floor-check 改寫負對照 FAIL 14；.projects scroll-padding 防 sticky 遮擋（不加 V1/P1 命中 FAIL 72）；concerns：♢ 需設計確認、‖ 細長未換、Floor 被 Live Output 列壓縮屬 4.1) — review dispatched：Codex＋spec-check＋design review（fresh opus）
CODEX 3.2（base cb90635）：needs-attention 2 medium（實作大致符合 D3/D4/D8/D10）：(1) 未知 status 虛線外框只驗 style 與顏色未驗 border-width，四邊寬 0 仍通過（factory-floor-check 與 visual-check 同缺口）；(2) sticky 遮擋只測程式化 scrollIntoView，未測真實 Tab 焦點停在 sticky 列首／欄首底下（log 無 usage limit）
SPEC-CHECK 3.2：✅ 2 Minor（D4 ◇→♢ 與 ▶ 兩句黏同段；‖ 墨跡 5×12 較窄未換→設計裁決）；9 項帶過來事項全完成；factory-floor-check 14 條改寫 0 條放寬；◇ 只剩歷史文件；G1/G2 890 ok、V1、P1 無回歸
DESIGN REVIEW 3.2：需修 4 條；六問：running 最醒目、ready 不搶戲、冰青三形分得開、♢ 可接受、無彩色 emoji、▶ 接 U+FE0E、刻度可懂不吵、按鈕中性不誤認、sticky 成立不透明。I1 Important：failed／blocked 比 ready、completed 不醒目（僅 4px 色條，failed 紅條亮度 0.326 低於 pending）→ failed、blocked 加 1px 狀態色四邊細框＋狀態字 600（替代案「節點底色加狀態色」違 spec 表面色為底，交使用者）；M1 1100–1280 寬列首欄固定 240px、節點按鈕疊三列 → 列首上限約 200px；M2 交使用者：<1200 整頁捲動時 stage 欄首會捲出（D3 只要求橫捲時固定）；M3 內層捲軸未吃 scrollbar-width: thin（不繼承）→ 5.2；提醒：shots-3.2 截圖隱藏了捲軸，之後截圖不得隱藏。
- Ruling R31：I1 採第一方案（failed、blocked 1px 狀態色細框＋600 字重），不採底色方案 — 在自由度內、不動 spec；與 running 2px＋柔光形狀分得開 — 代價：節點多一種外框語彙。
- Ruling R32：M2 維持現行 D3（整頁捲動版面不做縱向 sticky 欄首），不打斷使用者 — 非違規、屬版面取捨，KISS — 代價：窄／矮視窗往下捲時要回頂端看 stage 名稱；使用者可推翻。M3 → 5.1 清理（scrollbar-width 對所有內層捲動容器明設）。
Task 3.2: fix round 1 → 續派原實作者（Codex 2 medium＋設計 I1、M1＋spec-check M1）
Task 3.2: fix round 1 implemented (commits f5f91df 產品、aaa2290 腳本；failed／blocked 1px 狀態色框＋600；首欄 200px＋格子／節點左右內距 8→6（1280 寬兩列，依字寬）；D4 分段；未知外框 border-width 斷言＋負對照；Tab／Shift+Tab／focus() 六情境遵守 scroll-padding、未加 focusin；911 ok／R1 runtimes FAIL（3.3）) — scoped re-review dispatched (Codex base f6d9693＋設計審核員)
CODEX 3.2 r1（base f6d9693）：approve，No material findings（未知外框寬度、真實 Tab 穿越 sticky 皆 ADDRESSED；1px 狀態框 ≥3:1、無 glow；log 無 usage limit）。
DESIGN RE-REVIEW 3.2 r1：I1、M1 ADDRESSED（瞇眼圖 failed／blocked 比 ready／completed 醒目、running 仍最醒目；1280×650 stage 欄 153.7px、按鈕 2 列）；密度可接受；新退步 0。
Task 3.2: fix round 1/5 (all addressed; commits f5f91df, aaa2290)
Task 3.2: minor (deferred)：1280 寬按鈕兩列依字寬（內距 6px），換字型或改按鈕文字可能回三列。
Task 3.2: complete (commits cb90635..aaa2290；spec-check ✅、Codex approve r1、design review r1 通過)
Task 3.3: dispatched (BASE 8bdee58; opus)
Task 3.3: implemented (opus; commits 26ac425 產品、d6b11c3 visual-check R1 runtimes／D1／U1／V1；六支未改全 PASS；14 段 991 ok；STRICT 預設化（VISUAL_CHECK_STRICT_3_3 失效）；新斷言先 FAIL 33；concerns：拿掉 region 框保留卡框（D8 右欄 1px 框 vs D11 只有卡框 → 可能衝突，brief 第 5 條措辭含糊）、workspace／tab HERDR 焦點底色拿掉（超範圍）、整頁捲動版面最近事件加高度上限（超範圍）、右欄 70／30 維持) — review dispatched：Codex＋spec-check＋design review（fresh opus）
CODEX 3.3（base 8bdee58）：needs-attention 2 medium：(1) 拿掉 runtimes region 1px 框違 D8（D11 的「只有卡一層框」限制的是卡內層次，未撤銷區域框），且 visual-check 4684-4691、4973 把區域框當失敗；(2) 最近事件 max-height 套用到所有整頁捲動版面，違 D3「取消固定高度、整頁捲動」。HERDR 焦點：主規格與 delta spec 皆未要求 workspace／tab 顯示焦點，資料與 focused class 保留 → 非規格退步。pane 選取、改綁、exited、焦點還原、Live Output 入口保留（log 無 usage limit）
SPEC-CHECK 3.3：✅ 1 Important（runtimes 拿掉 region 框、events 與 projects 仍有 → 左右與右欄內部不對稱，與 D8 字面衝突，建議交使用者）、1 Minor（workspace／tab HERDR 焦點底色：四份 spec 皆未要求，非規格迴歸，無自動化覆蓋）；事件高度上限判合規；U1 與 whatever-check 分工清楚；strict 預設化合理；R1/D1/U1/V1 404 ok、V3/V4 103 ok。
- Ruling R33：恢復 runtimes region 的 1px --line 框（D8 字面；D11「只有卡一層框」限制卡內層次、未撤銷區域框，採 Codex 讀法）；不交使用者 — 屬解讀、不改文件；左右欄與右欄兩區一致 — 代價：無。visual-check 把區域框當失敗的斷言要反轉。
- Ruling R34：拿掉整頁捲動版面的最近事件 max-height（採 Codex，不採 spec-check）— D3 對這些版面寫「取消固定高度、整頁捲動」，只有 Live Output 與寬矮版面 Floor 有指定上限；巢狀捲軸多一層操作 — 代價：事件多時頁面變長。
DESIGN REVIEW 3.3：需修 6 條；六問（掃讀、pane 兩行、四種列狀態、事件時間、70／30）合格；HERDR 焦點不必加回 workspace／tab。I1 Important：cwd 尾端省略留下相同前綴、看不出哪個是 \sub → 前段／尾段兩 span、省略前段；M1 事件 kind 欄差 18px、兩卡明細值欄差 21px → subgrid／固定標籤欄；M2 tab 邊界不清 → .tab + .tab 上間距 8px；M3 pane HERDR 焦點底色對 hover 1.03:1、與改綁目標同色、無說明 → 拿底色、加「作用中」文字；M4 連線明細值用 --text 等寬、蓋過 pane 狀態 → --text-dim（D10 輔助資訊）；M5 交使用者：框線。
USER DECISION 2026-09-25（右欄框線）：runtimes region 一層 1px --line 框（同左欄與 events），區內 runtime 之間改用上方分隔線，不再畫四邊卡框；design D11 改一句。→ 取代 Ruling R33 的「卡框保留」部分。
- Ruling R35：3.3 fix round 1＝使用者框線決定＋R34（拿掉整頁捲動版面事件 max-height）＋設計 I1、M1–M4（M3 採拿底色加「作用中」文字標記，只在 pane 列）；D11 文字更新由實作者另開 docs commit — 代價：無。
Task 3.3: fix round 1 → 續派原實作者
Task 3.3: fix round 1 implemented (commits 8acfb39 產品、b5a8d52 visual-check、7c0a0ca D11；新斷言修前 FAIL 12；1005 ok；六支、gates、markdownlint、validate 綠；concerns：tasks.md 3.3 原文仍寫卡框（控制端勾選時改）、cwd 省略號與尾段間偶有空隙、明細標籤欄 7em 寬字型可能折行) — scoped re-review dispatched (Codex base d6b11c3＋設計審核員)
CODEX 3.3 r1（base d6b11c3）：approve，No material findings（區塊框恢復＋分隔線、事件上限移除 ADDRESSED；cwd head/tail、作用中標記、subgrid／7em 對齊無破壞；反轉框線斷言與 cwd 斷言有非空前置條件；log 無 usage limit）
DESIGN RE-REVIEW 3.3 r1：I1、M1–M5 ADDRESSED；cwd 省略號空隙可接受；7em 標籤不折行；新 Minor N1（值欄變窄中文長句尾字掉行 → text-wrap: pretty）、N2（cwd 無前段時第二行基線偏 3px、列高 47 對 43 → 不產生空前段 span）。
Task 3.3: fix round 1/5 (all addressed; commits 8acfb39, b5a8d52, 7c0a0ca)
Task 3.3: minor (deferred)（R25）：N1、N2 → 5.1 清理。
Task 3.3: complete (commits 8bdee58..7c0a0ca；spec-check ✅、Codex approve r1、design review r1 通過；tasks.md 3.3 描述同步使用者框線裁決於 474dafa)
Task 3.4: dispatched (BASE 474dafa; opus)
Task 3.4: implemented (commits 85baede style.css、4b2b476 visual-check CT1；拿掉 .action-banner 6px 圓角、error-banner 改三邊 --line＋2px --bad 左緣＋surface 底；CT1 RED 3→GREEN；對比 error 5.67、rebind 9.20；1005 ok；六支綠) — review dispatched：Codex＋spec-check＋design review（fresh opus）
CODEX 3.4 第一次：app-server connection closed（log 2 行、無審查結果）→ 重跑
CODEX 3.4 第二次：log 18 行、無結論段，開頭 approve 是審查開始時的占位訊息 → 不採信、重跑
CODEX 3.4 第三次（base 474dafa，完整 39 行、無 usage limit）：needs-attention 1 medium：CT1 只比兩 banner 相等，未斷言底色＝surface、左框恰 2px solid、其餘 1px --line；按鈕只驗文字與上框色；負對照只注入 padding／圓角／文字色 → 補精確值斷言與對應負對照。CSS 本身符合 D4；actions-check 未變。
DESIGN REVIEW 3.4：需修；左緣條家族、直條、中性按鈕、焦點外框 OK。I1 Important：錯誤文字折行時「關閉」按鈕被擠成直排（42×20→30×36），1200×720 預設 409 文字即發生 → 按鈕 flex:0 0 auto＋nowrap、文字 min-width:0＋overflow-wrap:anywhere＋按鈕高度斷言＋負對照；M1 錯誤文案以 HTTP 碼與 API 路徑開頭、非使用者動作 → 需改 actions.js，超 3.4 範圍；M2 錯誤提示補 ✕（aria-hidden）以利灰階區分；記錄（非 finding）：兩則並存時改綁整句冰青比錯誤紅字亮，屬 D4 定案，要調得改 D4。
Task 3.4: minor (deferred)：錯誤文案（M1）→ final review 分流（可能屬後續 change）。
SPEC-CHECK 3.4：✅ 2 Minor（CT1 底色只驗相等未對照 SPEC_COLORS.surface、無獨立負對照；失敗訊息誤印文字色欄位）＝與 Codex 同源；CT1、V1–V4、actions-check PASS；錯誤文字對比 5.67 符合 spec。
- Ruling R36：3.4 fix round 1＝Codex CT1 精確值斷言（底色＝surface、左框 2px solid、其餘 1px --line、按鈕完整 D4 probe）＋對應負對照＋spec-check 失敗訊息修正＋設計 I1（按鈕不被擠直排）＋M2（錯誤提示補 ✕）；M1 文案延後 — 代價：無。
Task 3.4: fix round 1 → 續派原實作者
Task 3.4: fix round 1 implemented (commits 3ec6cdd 產品、883c1f6 visual-check；按鈕 flex 0 0 auto＋nowrap、文字 min-width 0＋overflow-wrap；錯誤提示補 ✕；CT1 精確值＋三負對照；banner-wrap 子段（負對照需同時還原 flex-shrink）；CT1 RED 6→GREEN；1005 ok；六支綠) — scoped re-review dispatched（最後一輪，使用者要求收尾）
DESIGN RE-REVIEW 3.4 r1：I1（5 寬度×2 文字 10 格按鈕皆 42×20、未裁）、M2（✕ 有）ADDRESSED；新 Minor ①折行時 ✕ 置中於整段、未對齊第一行（最多偏 47.5px）②flex 12px 間距讓 ✕ 離文字太遠、兩則提示文字起點錯開 29.5px。
Task 3.4: minor (deferred)（R25）：✕ 對齊第一行、✕ 與文字間距／兩則文字起點對齊 → 5.1。
CODEX 3.4 r1（base 4b2b476，31 行、有結論段、無 usage limit）：CT1 精確值 ADDRESSED；產品端 no-wrap 與 ✕ 無功能破壞；新 medium：banner-wrap 測試未證明文字真的折行且完整可見（丟棄 return !!t、只量按鈕、只測 error-dismiss 未測 rebind-cancel；負對照同時改 white-space 與 flex-shrink，無獨立辨識力）。
Task 3.4: fix round 1/5 (Codex CT1、設計 I1、M2 addressed; open: banner-wrap 測試辨識力（Codex medium）; commits 3ec6cdd, 883c1f6)
PAUSE 2026-09-25：使用者要求本 session 收尾、換 session。3.4 未勾選，停在 fix round 1 之後；下個 session 從 3.4 fix round 2 開始（只修 banner-wrap 測試＋可順手做的兩條 ✕ 對齊 Minor），實作者用新的 sonnet（原實作者 context 不跨 session），review 走 Codex scoped re-review（base 883c1f6）＋設計審核。
HANDOVER v19 committed 5e3daa0（read-back ✅ 補 R23 遺漏）
RESUME 2026-09-25（新 session）：使用者下 /opsx:apply → 3.4 fix round 2 派新 sonnet（原實作者 context 不跨 session；FIX_BASE 883c1f6）：Codex banner-wrap medium＋順手兩條 ✕ Minor
Task 3.4: fix round 2 dispatched (fresh sonnet；FIX_BASE 883c1f6，HEAD 9fd8ef7 僅 docs)
Task 3.4: fix round 2 implemented (commits c3b0d64 style.css ✕ 對齊第一行＋間距 12→4、bf5cd40 visual-check CT1 banner-wrap：Range 行數≥2、無裁切／無水平溢出、error-dismiss＋rebind-cancel、三負對照；誠實發現：單獨還原按鈕 white-space 或 flex-shrink 皆不會壓扁（文字 min-width:0 吸收收縮），只有兩者一起還原才重現；RED 用 fix round 1 前真實產品碼，rebind-cancel 舊版也被壓成 30×36；1119 ok／0 FAIL；六支綠；cockpit 156/0/1）— scoped re-review dispatched (Codex base 9fd8ef7＋設計審核 fresh opus)
CODEX 3.4 r2（base 9fd8ef7，23 行、有結論段、無 usage limit）：approve，No material findings（banner-wrap ADDRESSED；單獨還原 white-space／flex-shrink 視為冗餘保護、如實處理；兩者同時還原可重現；✕ 首行對齊與 4px 間距有辨識力負對照）。
DESIGN RE-REVIEW 3.4 r2：通過；N1（折行 2–4 行 ✕ 與第一行差 0px）、N2（符號框到文字 4px；兩則文字起點差 21.5＝符號寬＋4，屬有無符號的正常差）ADDRESSED；9 格按鈕皆 42×20、焦點外框完整、無新退步；觀察（非 finding）：折行時 ✕ 貼首行、按鈕在整則垂直置中，設計文件未規定。
Task 3.4: fix round 2/5 (all addressed; commits c3b0d64, bf5cd40)
Task 3.4: complete (commits 474dafa..bf5cd40；spec-check ✅、Codex approve r2、design review r2 通過；先前 deferred 的兩條 ✕ Minor 已於 r2 修掉，5.1 不必再做)
Task 3.4 checked off in tasks.md at a5940ee
Task 4.1: dispatched (BASE a5940ee; opus — 常駐面板牽動高度矩陣 R17、三支腳本改寫，2.1／2.3 版面高度曾多輪修正，直接用 opus 省輪數)
Task 4.1: implemented (opus; commits f74d0ec 產品、6519998 live-output-check、bf8f2b8 visual-check；21 段 1257 ok／0 FAIL／0 PEND（RED 31）；六支綠（live-output 312 ok）；gates 綠；高度：Live Output 空狀態＝有選取、不隨 banner 變；1536×1024 405.5、1280／1200×720 268.7；Floor 最低 161.8 同 BASE）concerns：<760 單欄空狀態面板僅約 71px（D3 未給值）；取消選取後焦點落 body（BASE 同）；V2–V4 舊註解殘留 — review dispatched：Codex＋spec-check＋design review
CODEX 4.1（base a5940ee，46 行、有結論段、無 usage limit）：needs-attention 2 medium：(1) <760 單欄空狀態塌到約 71px、無 min-height，選取後版面跳動；R17 矩陣 700×900 的高度斷言只在固定一屏分支跑 → 未擋；(2) 取消選取後焦點落 body（按鈕被 display:none），測試只 element.click() 未驗 activeElement → 建議焦點回原 pane 列，列不存在則回可聚焦的空狀態容器。
SPEC-CHECK 4.1：✅ 0 Critical／0 Important／2 Minor（V2–V4 舊註解過時；concern 1／2 判 spec 未規範、非違規，建議交設計審核）；live-output-check 改寫涵蓋變寬非恆真；R2 PENDING 轉真斷言；LO1 closeNeg／fillNeg 負對照；無 4.2 越界。
DESIGN REVIEW 4.1：需修；I1 Important：.output-text 撐滿後 --surface 底＋框線＋4px 圓角佔面板 87%，D8 的 --bg-deep 觀察窗不見、空／選取像兩個面板（2.2 carry）→ 透明底、拿框線圓角；I2 Important：.output-title 的 runtime／pane id 用無襯線，違 spec 與 D11 等寬 → --font-mono、14/600 不變；M1 輸出行高 1.5 應 D10 1.4；M2 .output-empty padding 12px 4px 使切換時首行跳 12／4px → 0；M3＝Codex 焦點；concern 1（窄版 71px）裁定可接受：面板往點擊處下方長高、不推走點擊位置，加 min-height 更差。交使用者：(1) D7 空狀態文案提到「runtime 清單」「Factory Floor」但畫面無這兩個區塊標題；(2) 760–1199 寬從 runtime 清單選 pane 時面板只露下半段，是否自動捲動（新增行為）。
- Ruling R37：Codex「窄版空狀態塌縮」不修，採設計審核判斷 — D3 對 <760 單欄寫「取消固定高度、整頁捲動」，M6 的「撐滿」是針對有指定高度的區塊；單欄順序 runtime→Output→事件，選取後面板往下長高、不推走點擊位置，只把事件往下推；加 min-height 會讓空狀態在單欄佔一大塊空白 — 代價：窄版選取時下方事件區跳動一次；若使用者覺得跳動不好，再補 min-height。
- Ruling R38：4.1 fix round 1 範圍＝Codex 焦點（取消選取後焦點回原 pane 列，列不存在則回 #output（tabindex=-1）＋以鍵盤觸發並斷言 activeElement 的測試與負對照）＋設計 I1、I2、M1、M2＋spec-check Minor（V2–V4 過時註解）；兩條交使用者的不擋 4.1 — 代價：無。
Task 4.1: fix round 1 → 續派原實作者（R38 範圍）；兩條交使用者題目已提出
USER DECISION 2026-09-25（4.1 設計審核交使用者兩題）：D7 空狀態文案維持現行；760–1199 選取後不自動捲動。
Task 4.1: fix round 1 implemented (commits a9e63a9 產品、8f3c5b5 visual-check；焦點交接（Enter 回 wJ:p1 列、列消失 Space 退回 #output）＋負對照；.output-text 透明無框；標題等寬；行高 1.4；.output-empty padding 0；V2–V4 註解；RED 6→GREEN；21 段 1272 ok；六支綠；cockpit 156/0/1、ui_preview 13) concerns：首行對齊實測差 1.0px 恰在 ≤1px 門檻；#output tabindex=-1 後滑鼠點面板內焦點停在 #output — scoped re-review dispatched (Codex base bf8f2b8＋原設計審核員)
CODEX 4.1 r1（base bf8f2b8，38 行、有結論段、無 usage limit）：焦點 ADDRESSED；窄版（R37）ADDRESSED；新 medium：首行對齊斷言用 glyph rect（無襯線 vs 等寬）、≤1px 門檻實測恰 1.0px、零餘裕、跨字型／DPR 易誤紅 → 改比 .output-empty 與 .output-header 的 content-box 起點＋注入 padding／位移負對照。等設計複審後合併 round 2。
DESIGN RE-REVIEW 4.1 r1：通過；I1、I2、M1、M2（左緣同為 257、上緣差 1.0px 肉眼不可見）、M3 ADDRESSED；tabindex=-1 無可見副作用（滑鼠點不畫框、拖曳選字正常）；新 Minor N1：.output-text 殘留 8px padding，輸出內容比標題縮進 8px（265 vs 257）→ 左右 padding 0；N2：退到 #output 的內縮焦點框在兩切角被 clip-path 切斷（1 倍仍清楚）。
Task 4.1: fix round 1/5 (焦點、I1、I2、M1、M2、spec-check 註解 addressed; open: 首行對齊斷言零餘裕（Codex medium）; commits a9e63a9, 8f3c5b5)
- Ruling R39：round 2＝Codex 對齊斷言改 layout invariant（content-box 起點）＋負對照；N1（.output-text 左右 padding 0）同輪做，因為它就是同一條對齊關係（R25）；N2 切角焦點框 → deferred 5.1 — 代價：無。
Task 4.1: minor (deferred)：N2 #output 焦點框在切角被 clip-path 切斷 → 5.1。
Task 4.1: fix round 2 implemented (commits 1766e6c 產品 .output-text 左右 padding 0、e5ebf70 visual-check [LO1/align] 改 content-box 起點 <0.5px＋三負對照；RED 8px；DPR 1.25 重跑 PASS；21 段 1273 ok；六支綠；gates 綠）— scoped re-review dispatched（Codex base 8f3c5b5；產品改動僅為設計審核員自己提出的 padding 0、已有對齊斷言守，不另開設計複審）
CODEX 4.1 r2（base 8f3c5b5，26 行、有結論段、無 usage limit）：glyph rect 問題 ADDRESSED；新 medium：lo1Align 只比 empty↔header、empty↔text，漏 header↔text（±0.49 可合計 0.98px 過關）→ 改 max−min 跨度 <0.5＋兩側各偏 0.49px 的負對照。
Task 4.1: fix round 2/5 (glyph rect、N1 padding addressed; open: 三者跨度判準; commits 1766e6c, e5ebf70) → fix round 3 續派原實作者
Task 4.1: fix round 3 implemented (commit 49d57ca visual-check；left 改三者極差 <0.5；±0.49 負對照 RED(舊 ok:true)→GREEN(span 0.96875 判不對齊)；21 段 1274 ok）— scoped re-review dispatched (Codex base e5ebf70)
CODEX 4.1 r3（base e5ebf70，13 行、有結論段、無 usage limit）：approve，No material findings。
Task 4.1: fix round 3/5 (all addressed; commit 49d57ca)
Task 4.1: complete (commits a5940ee..49d57ca；spec-check ✅、Codex approve r3、design review r1 通過；3 輪修正；1 deferred minor N2→5.1；R37 窄版不加 min-height)
Task 4.1 checked off in tasks.md at 682fe6e
Task 4.2: dispatched (BASE 682fe6e; sonnet — 範圍明確：CSS 換皮＋三支腳本斷言改寫)
Task 4.2: implemented (commits 005075f 產品、2044487 live-output-check、727ef0f live-output-real-check（只改不跑）；J/K/L/M RED 16→GREEN；live-output 330 ok；visual-check 21 段 1274 ok（CT1 stale 對比 9.04／10.58）；六支、gates 綠；visual-check 未改）concerns：「過期」標籤色選 --warn（D7 未指定）；「pane 已不存在」／截斷提示未另加樣式、換皮靠共用 is-stale — review dispatched：Codex＋spec-check＋design review
CODEX 4.2（base 682fe6e，49 行、有結論段、無 usage limit）：needs-attention 2 medium：(1)「pane 已不存在」與截斷提示 selector 與 BASE 相同、未換皮、無 computed-style 驗收；(2) checkNotStale 只驗 textColor≠--text-dim（null 也過）、未驗恢復成 --text 與 isStale=false；checkMarkedStale 未驗原因訊息 --warn；real-check 同缺口。
SPEC-CHECK 4.2：✅ 0 Critical／1 Important（tasks「pane 已不存在／截斷提示換皮」無具體規格，兩節點無樣式改動，請控制端裁決範圍）／1 Minor（「過期」標籤 --warn 屬設計決定、不違規）；8 處 opacity 斷言升為絕對值比對、非恆真；CT1 stale 實際量 computed 對比。
DESIGN REVIEW 4.2：需修；D7 四項落地、灰階可辨、切角色條俐落、恢復清乾淨、左緣對齊不動。I1 Important：截斷提示 --warn 且實機幾乎常駐（read_output 用 ReadSource::Recent，pane-read-probe §3 WSL 0.8.2 實測 recent 恆 truncated=true）→ 正常態常掛警示色、過期時與原因同色同級疊放、原因擠到第三行 → 截斷提示改 --text-dim、原因排在截斷提示之前；M1 標題長時「取消選取」折兩行 → flex:none＋nowrap；M2 截斷時被捲掉半截的行貼提示下方約 6px → gap 8px；concern（1）「過期」--warn 同意保留（2）gone 共用過期標示同意。交使用者：pane 已不存在時左緣色條改 --bad（要改 D7）。
- Ruling R40：tasks 4.2「pane 已不存在與截斷提示換皮」的範圍＝兩則提示依 Direction 01 語彙定色並補 computed-style 驗收：截斷提示是常駐的資訊性提示、不是警示 → --text-dim（採設計 I1），「pane 已不存在」維持 --bad（D4 失敗語意，2.2 已換成 token）；原因訊息排在截斷提示之前、截斷提示仍在內容區頂端（spec「頂端出現」）。Codex (1) 以此落實；spec-check Important 同此裁決 — D7 未規定這兩則提示外觀，屬自由度；截斷在實機近乎常駐，用警示色會讓 --warn 失去意義 — 代價：若使用者希望截斷更醒目，改回 --warn 只是一個 token。
- Ruling R41：4.2 fix round 1＝R40＋Codex (2)（恢復態精確 textColor===--text 且 isStale===false；過期態 isStale===true 且可見原因訊息 color===--warn；兩支腳本同步）＋設計 M1、M2（R25：既然開輪，小 Minor 一起修）；交使用者的 gone 色條 --bad 另問、不擋本輪 — 代價：無。
USER DECISION 2026-09-25（4.2 交使用者）：pane 已不存在時色條維持 --warn（D7 不改）。
Task 4.2: fix round 1 implemented (commits 2906e52 產品、7bcb433 live-output-check、24f8ef7 real-check（只改不跑）；截斷 --text-dim、gone --bad、原因移到截斷之前；assessNotStale／assessMarkedStale 精確判斷＋W 段 10/10 自測；.output-close nowrap；gap 8px；A/X RED 3+3→GREEN；live-output 345 ok；visual-check 1274 ok；六支、gates 綠）— scoped re-review dispatched (Codex base 727ef0f＋原設計審核員)
CODEX 4.2 r1（base 727ef0f，40 行、有結論段、無 usage limit）：(2) 恢復／原因色 ADDRESSED；(1) NOT：合成驗收已涵蓋，但 live-output-real-check.js 兩條真機 gone 路徑（R18 outcome b、關 tab 後 gone）未呼叫 readStaleSignals()/checkMarkedStale() → 補上＋負對照（只改不跑）。
DESIGN RE-REVIEW 4.2 r1：通過；I1、M1、M2 ADDRESSED（截斷 --text-dim、原因第二行；9 狀態按鈕單行 20px；gap 8px）；無回歸（左緣一致、gone 色條 --warn、提示 --bad、恢復清乾淨）；觀察：≥760 時原因行讓 `<pre>` 變矮、過期期間最後一行被切半截（加原因行後本有，本輪多約 6px）。
Task 4.2: minor (deferred)：過期期間原因行使 `<pre>` 變矮、最後一行被切半截 → 5.1 看是否順手處理。
Task 4.2: fix round 1/5 (Codex (2)、設計 I1、M1、M2 addressed; open: real-check 兩條真機 gone 路徑未呼叫過期樣式驗證; commits 2906e52..24f8ef7) → fix round 2 續派原實作者
Task 4.2: fix round 2 implemented (commit 339801a real-check：兩條真機 gone 路徑呼叫 readStaleSignals/checkMarkedStale；--self-test-stale-signals 離線自測 7/7、6 負對照；node --check 綠；預設呼叫仍要求 WSL opt-in）— scoped re-review dispatched (Codex base 24f8ef7)
CODEX 4.2 r2（base 24f8ef7，28 行、有結論段、無 usage limit）：兩條 gone 路徑已補完整檢查、旗標不繞過 opt-in 也不改預設入口；新 medium：模組載入即 fs.mkdtempSync 建 CURL_SCRATCH，自測旗標在其後才分流 → 唯讀環境 EPERM、一般環境 process.exit 略過 finally 留空目錄。
Task 4.2: fix round 2/5 (真機 gone 路徑 addressed; open: 自測前 eager mkdtemp; commit 339801a) → fix round 3 續派原實作者
Task 4.2: fix round 3 implemented (commit 8d98983 real-check：CURL_SCRATCH 惰性建立、finally 只在已建立時清；順帶修 --verify-ownership-mismatch 同缺陷；RED stash 0→1、GREEN 0→0；實作者清掉 %TEMP% 中 21 個既有 0 byte 空殘留目錄）— scoped re-review dispatched (Codex base 339801a)
CODEX 4.2 r3（base 339801a，19 行、有結論段、無 usage limit）：approve，No material findings（離線自測唯讀環境實跑通過）。
Task 4.2: fix round 3/5 (all addressed; commit 8d98983)
Task 4.2: complete (commits 682fe6e..8d98983；spec-check ✅、Codex approve r3、design review r1 通過；3 輪修正；1 deferred minor→5.1；R40 截斷提示 --text-dim)
Task 4.2 checked off in tasks.md at 0faa5df
Task 5.1: dispatched (BASE 0faa5df; opus — R28 頂列／底列高度推導牽動高度矩陣，2.3 曾 5 輪；加上刪 CSS 需全狀態判斷)
Task 5.1: implemented (opus; commits ac6a49e 死規則＋延後項目 1–5、7、54ed5ab cwd 空前段（item 6）、bd5ab6e 提示行出現時貼底維持（item 8；順修既有：截斷提示首次出現後永久不再貼底）、8740581 visual-check CL1／DF1；@keyframes 0；visual-check 1381 ok／0 FAIL（RED CL1 1、DF1 23）；六支、gates 綠；底列 32→31.8、Floor 讓出最低 161.86）concerns：item 9（R23 頂列框線不對稱）未做，交 5.2／final review — review dispatched：Codex＋spec-check＋design review
CODEX 5.1（base 0faa5df，55 行、有結論段、無 usage limit）：needs-attention 2 medium（實作看似正確，測試盲點）：(1) CL1 walk() 不管 @media／@supports 條件是否成立就遞迴、任一狀態有元素即判 matched → 規則在不成立的 @media 內仍判活；負對照 @media(min-width:1px) 恆成立 → 攜帶外層條件、matchMedia／CSS.supports 判斷＋負對照；(2) DF1/topbar 只驗燈號上下邊界，未驗 overflow-x、scrollWidth>clientWidth、首尾可捲入 → overflow-x:hidden 也過；補單獨覆寫 hidden 的負對照。
SPEC-CHECK 5.1：✅ 0 Critical／0 Important／1 Minor（[DF1/pin] 未單獨驗 markGone 路徑貼底）；項目 1–7 與 DF1 斷言＋負對照對上；項目 8 屬 live-output「停在底部會跟著走」範圍；項目 9 依 R23 可接受；刪除清單逐條 grep（含 connStateClass 動態組字串）皆真死；CL1 涵蓋各狀態。
DESIGN REVIEW 5.1：通過；1–8 ADDRESSED（1280×719 長 id 頂列 126→45.6、Live Output 回第一屏；提示行出現距底 24／48→0）；BASE vs HEAD 64 組狀態×寬度逐元素比對，差異皆對應 1–8、Tab 路徑每站截圖逐 byte 相同 → 無回歸；item 9 建議維持 R23 不修；Minor M1：.event-detail（及專案警告、Live Output 原因行）尾字單獨掉行 → text-wrap: pretty；M2：頂列 6px 圓角是全畫面唯一圓角 → 直角（不影響高度與 token）。交使用者：D4「切角補線不用冰青」vs item 7 聚焦時補線改冰青 → 建議只改 D4 措辭；item 9 修不修。
- Ruling R42：5.1 fix round 1＝Codex 2 medium（CL1 攜帶 @media／@supports 條件、DF1/topbar 驗橫向可捲＋單獨 overflow-x:hidden 負對照）＋設計 M1、M2＋spec-check Minor（[DF1/pin] 補 markGone 路徑）（R25：開輪就一起修小項）— 代價：無。
USER DECISION 2026-09-25（5.1 交使用者）：D4 補「聚焦時切角補線屬焦點框、改用冰青」措辭（只改文件）；item 9 頂底列框線不對稱維持不修（R23）。→ D4 措辭併入 5.1 fix round 1 的 docs commit
Task 5.1: fix round 1 implemented (commits 2889151 產品 pretty 補齊＋頂列直角、bea2718 visual-check CL1 @media 條件＋DF1 橫向捲動／pretty／圓角／markGone、92b876c D4 措辭（控制端 read-back 核過）；1392 ok；六支、gates、markdownlint 80/0、validate 17/0 綠；高度矩陣與上輪逐格相同）— scoped re-review dispatched（Codex base 8740581；產品改動僅設計審核員自己提出的兩項、有斷言守，不另開設計複審）
CODEX 5.1 r1（base 8740581，31 行、有結論段、無 usage limit）：approve，No material findings（CL1 條件逐層求值、DF1 橫向可捲皆 ADDRESSED）。
Task 5.1: fix round 1/5 (all addressed; commits 2889151, bea2718, 92b876c)
Task 5.1: complete (commits 0faa5df..92b876c；spec-check ✅、Codex approve r1、design review 通過；延後項目 1–8 完成、9 依使用者維持 R23)
Task 5.1 checked off in tasks.md at adad4f6
Task 5.2: dispatched (BASE adad4f6; sonnet — 全跑 visual-check＋三寬截圖到 scratch；之後 R8(c) 全面設計審核，最後交使用者目視)
Task 5.2: run (sonnet；visual-check 24 段 RESULT PASS 1392 ok／0 FAIL／0 PEND；CT1 banner／stale／all 全 ok；四張截圖於 scratch shots-5.2）concerns：全頁截圖 captureBeyondViewport 使 sticky 底列凍在中段（截圖假象）；CT1 未取樣 connecting 連線狀態文字對比（fixture 無）— R8(c) 全面設計審核 dispatched（opus；另產 viewport 截圖給使用者、量 connecting 對比）
DESIGN REVIEW 5.2（R8(c) 全面）：通過；0 Important；connecting 狀態文字最低 7.35:1（底列 connecting 標籤）、全部 ≥4.5（實際重連時底列不顯示 connecting、只在首次載入）；使用者截圖 6 張 viewport 於 scratch shots-5.2-user。
Task 5.2: minor (deferred)（R25，交 5.4 final review 分流）：M1 Tab 進 Factory Floor 捲動區邊緣按鈕焦點框被切一邊 → scroll-padding；M2 1100／700 寬 pane 列「作用中」與前後文字相距 130–483px；M3 最近事件區內距 8px vs 同欄 12px、無 Project 時兩段空狀態文字未對齊；M4 僅文字放大 200% 時 700 寬「Completed」按鈕超出節點 10.2px、1536 右欄明細值欄剩 84px（頁面縮放無此問題）；M5 visual-check 對比工具算不到半透明底色、CT1 數字偏高（重算全頁最低 5.23 仍合格）。
交使用者（5.2）：U1 滑鼠點按鈕／列留冰青焦點框、重畫後仍在，選定 pane 列像 running 節點；U2 cockpit 服務斷線時右欄仍綠色 connected（只有頂列標最後已知）；U3 設計文件寫檔名用等寬、D11／spec 未列，路徑與 cockpit.toml 目前一般字體。
USER DECISION 2026-09-26（5.2）：目視驗收通過；U1 滑鼠焦點框、U2 右欄斷線舊狀態 → 留給後續 change（記 handover）；U3 路徑維持一般字體，以 D11／spec 為準，視覺設計文件「檔名用等寬」一句於 5.3 改成一致。
Task 5.2: complete (visual-check 1392 ok／0 FAIL／0 PEND；R8(c) 設計審核通過；使用者目視通過；5 Minor 交 5.4 分流)
Task 5.2 checked off in tasks.md at a785eb1
Task 5.3: dispatched (BASE a785eb1; sonnet)
Task 5.3: complete (commit 80b3cd1：.gitignore、design-doc 狀態＋檔名等寬句依 U3、README 取消選取／常駐面板、新增 visual-check.md；markdownlint 81 檔 0 issues、validate 17/0；控制端核對 .gitignore 與 design-doc 兩處)
Task 5.3 checked off in tasks.md at 4efd749
Task 5.4: dispatched (BASE 4efd749) — gate＋腳本實跑（sonnet）與 Codex whole-branch review --base main 並行；final review 分流 ledger 所有 minor (deferred)／parked
READ-BACK 5.3（sonnet）：✅ visual-check.md 全部宣稱對得上 visual-check.js；README 四句對得上 D7／live-output spec／output.js。
CODEX 5.4 FINAL（base main，89 行、有結論段、無 usage limit）：needs-attention 3 medium＋1 low：F1 切換 Project 時 SCROLL_KEEP 以通用 selector 識別容器、未含 selectedProject → p1 的 Floor 捲動位置套到 p2（P1 未覆蓋）；F2 Factory Floor 右／下邊界按鈕焦點框被 overflow 裁（＝5.2 M1）；F3 700 寬＋僅文字 200% 時 Completed 按鈕溢出節點 10.2px（＝5.2 M4 前半）；F4(low) visual-check 未知段落代號全跳過仍印 RESULT PASS。分流：必修＝F1–F4、5.2 M1、M4 前半；已解決＝2.1 flaky（被 ROOT CAUSE 取代）、2.3 各項（5.1）、3.3 N1/N2、3.4 ✕、4.1 N2、4.2 貼底（5.1）；可延後＝2.2 TK1 註解字串／font 簡寫、3.2 1280 按鈕列、3.4 錯誤文案（M1）、R23、5.2 M2、M3、M4 右欄值欄、M5 半透明對比工具。
USER NOTE 2026-09-26：change 5 範圍擴大 —— 介面下半部要能 Review 檔案，支援 md、pdf、html（使用者：這是 HERDR 最欠缺的功能）。原定「change 5 .md 瀏覽」改為此範圍；待 change 4 收尾後 brainstorming（下半部與 Live Output 的關係、檔案來源與讀檔端點目錄限制、HTML sandbox、md／pdf 套件選擇需先問、Review 是否含標註／回饋）。5.5 handover 必須寫入。
USER NOTE 2026-09-26（續）：change 5 另需「檔案瀏覽 sidebar」區塊——專注特定 space（HERDR workspace）時可開啟、列出該 space 的檔案，點檔在下半部 Review；每個檔案依類型有 icon。brainstorming 待定：根目錄取自該 space 哪個 pane cwd、sidebar 與左欄 Project／三欄版面 D3 的關係、icon 來源（新套件需先問）與 token 色限制。
USER DECISION 2026-09-26（change 5 icon 來源）：用 <https://github.com/material-extensions/vscode-material-icon-theme>（控制端查證：MIT、未封存、icons/ 904 個 SVG、含 markdown.svg／pdf.svg／html.svg；資料夾 icon 非 folder.svg 待查）。待裁決：icon 內嵌固定色（例 markdown.svg fill #42a5f5）與 change 4 十色 token 規則衝突 → 保留原色列例外 or 改 currentColor 單色。授權須保留 MIT 聲明。
Task 5.4 verify（sonnet）：gate 全綠（workspace 531/0/10、ui_preview 13、markdownlint 81/0、validate 17/0）；六支 PASS；visual-check 1391 ok、唯一 FAIL＝收尾 7770 被外部 ui_preview（疑使用者手動開的，PID 101156）占用 → agent 自行 taskkill 該程序（越權：之後派工明令非自己開的程序只回報不砍；已告知使用者）；live-output-real-check.js WSL 0.8.2 隔離 server 實跑 PASS 53 ok、--self-test-stale-signals 7 ok。
- Ruling R43：5.4 最終修正波（一次派工）＝Codex final F1（切換 Project 捲動位置洩漏）、F2（Factory Floor 邊界焦點框被裁＝5.2 M1）、F3（700 寬僅文字 200% Completed 溢出＝5.2 M4 前半）、F4 low（visual-check 未知段落代號假綠）；每條先重現（RED）才修，不成立則回報；其餘可延後項依 Codex 分流記入 handover — 代價：無。
Final fix wave: dispatched (FIX_BASE 4efd749; opus)
USER DECISION 2026-09-26（change 5 icon 顏色）：Material Icon Theme 的 icon 保留原色，列為十色 token 規則的明文例外（change 5 的 design／spec 要寫明例外範圍，TK1 類檢查需排除 icon SVG）。folder icon 實際檔名仍待 brainstorming 時查證。
USER NOTE 2026-09-26（change 5 範圍追加）：納入 Git Graph 功能（參考 <https://github.com/mhutchie/vscode-git-graph/tree/d7f43f429a9e024e896bac9fc65fdc530935c812>）。控制端查證：該 commit 為 2021-09-19「#557 Improved context menu click event handling」；repo 最後 push 2023-07；LICENSE 為自訂（GitHub 標 Other）：允許 use／copy／modify，但「Permission is NOT GRANTED to publish, distribute, sublicense, and/or sell derivative works」→ 不得直接取用其程式碼（推 remote 即可能構成散布）；做法＝當功能／外觀參考、自行實作，畫圖函式庫另選寬鬆授權者（新套件先問）。brainstorming 待定：只讀 vs 可操作、顯示範圍（graph／分支／檔案變更）。另：使用者提過喜歡 VS Code 功能，已建議不做 extension host，改「自做少數功能＋可選『在 VS Code 開啟』按鈕」，待使用者列出最想要的功能。
USER DECISION 2026-09-26（路線）：change 5＝檔案瀏覽與 Review（md／pdf／html、space 檔案 sidebar、Material Icon Theme 原色 icon、Git Graph 類 commit 圖自行實作）；change 6＝進度模型（①進度靠人工維護→agent 經 cockpit API 回報（寫 cockpit 自己的狀態檔、不改 HERDR 唯讀；done≠完成原則保留）②活動狀態精確到 task ④推進加退回）；change 7＝Live Output 上色＋M2＋U1 滑鼠焦點框＋U2 右欄斷線舊狀態；③多 runtime 通用支援暫不排（YAGNI，ADR-0003 已分層）。取代原「change 5 .md、change 6 上色＋M2」。
Final fix wave: implemented (opus; commits f55585c 產品、738f2c2 visual-check＋visual-check.md；F1 重現→捲動快照帶 Project id、切換歸零、同 Project 重畫保留；F2 重現（1536 8 站、1280×650 11 站被裁／被 sticky 蓋）→ .projects scroll-padding 各 +6px＋#app focusin 補捲（實測 Chrome Tab 聚焦時橫向露出 ≥32px 即不捲）；F3 重現→節點按鈕 max-width:100%＋overflow-wrap:anywhere；F4 重現→未知／空代號 exit 2＋S6 自測；visual-check 1464 ok（新增 S6、FR1）；六支、gates 綠；7770 事先確認空閒）concerns：F3 極端字級 Completed 字內斷行；F2 focusin 補捲為新產品行為、依實測無一手文件；F1 切回原 Project 不還原位置 — scoped re-review dispatched（Codex base 4efd749）
CODEX 5.4 FINAL r1（base 4efd749，39 行、有結論段、無 usage limit）：approve，F1–F4 ADDRESSED，No material findings（F1–F3 靜態判定；未知段落四種參數實測 exit 2）。
Final fix wave: complete (commits f55585c, 738f2c2；修正波後重跑 fmt／clippy／cargo test -p cockpit／ui_preview／六支／visual-check 1464 ok／markdownlint 綠；workspace 測試 531/0/10 與 openspec validate 於修正波前跑過、修正波只動前端與腳本)
Task 5.4: complete (gate 全綠、六支與 visual-check 全綠、live-output-real-check WSL 實跑 53 ok、Codex final approve r1)
Task 5.4 checked off in tasks.md at d514073
Task 5.5: complete (handover v20 寫於 main；markdownlint 0；使用者 2026-09-26 目視驗收通過)
ARCHIVE 2026-09-26：delta spec 同步進主規格（cockpit-dashboard ADDED 3／MODIFIED 2、live-output MODIFIED 1；控制端逐條核對 requirement 各出現一次）；change 移至 openspec/changes/archive/2026-09-26-direction-01-visual；本 ledger 複製為 sdd-ledger.md
