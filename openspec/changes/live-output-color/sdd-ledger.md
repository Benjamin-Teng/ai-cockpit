# SDD ledger：live-output-color

- plan：`openspec/changes/live-output-color/tasks.md`；spec：同目錄 `specs/`、`design.md`
- 分支：`feat/live-output-color`，起點 `a13e76f`（propose commit）
- 工作區（git-ignored）：`.superpowers/sdd/tasks-live-output-color/`（briefs、reports、review packages、`mkbrief.sh`）

## 前置裁決

- Ruling: ledger 放在 `openspec/changes/live-output-color/sdd-ledger.md`（隨 change archive），不放 SDD 工作區的 `progress.md` — tasks.md 1.1
  指定此路徑，前幾個 change 都如此 — 若錯：只是位置不同，無功能影響。
- Ruling: per-task 不另派 reviewer，改為控制端核對實作者的 red→green 與 gate 輸出；diff 審查依 tasks.md 4.3／5.6／7.3 由 Opus 5.5 分階段跑
  （使用者 2026-10-02 指示 Opus 取代 Codex）— 全域 CLAUDE.md「diff 審查不派 Claude subagent」與 tasks.md 的階段審查安排優先於 SDD skill 的
  per-task reviewer — 若錯：單一 task 的問題要到階段審查才被發現，返工成本較高。
- Ruling: SDD skill 的 `scripts/task-brief` 只認 `### Task N`，改用工作區 `mkbrief.sh` 從 checkbox 格式抽 task 原文＋通則 — 若錯：brief 漏字，
  實作者回頭讀 tasks.md 即可補。
- Ruling: 5.3（建驗收腳本）先於 5.2（前端實作）派工：腳本直接在當下 HEAD（5.1 完成、前端未改）跑出紅，免去 tasks.md 寫的「repo 外 worktree
  取 5.1 commit 重建」— 兩者證明的是同一件事（新斷言在舊前端上為紅），少一次完整建置 — 若錯：腳本自身的錯要到 5.2 才暴露，屆時腳本修正須
  與產品修正分開 commit。
- Ruling: 4.2（後端整合確認）與 5.5（frontend-design 外觀審核）由控制端自己執行，不派實作者 — 前者是驗證、後者是載入 skill 的審核模式 — 若錯：
  無實作內容，影響僅在紀錄。

## 前置衝突掃描

| 對象 | 產出 → 使用 | 結果 |
|---|---|---|
| 2.1 ↔ 3.1 | `AnsiColor`／`SegmentStyle`／`OutputSegment`／`PaneOutput::from_segments` → 解析器產出片段 | 介面定為 `cockpit_herdr::ansi::parse(&str) -> Vec<OutputSegment>`，呼叫端以 `from_segments` 包成 `PaneOutput`（合併與丟空片段在 core 做，解析器不必重複） |
| 2.1 ↔ 3.2 | 2.1 把 `read_output` 改用 `plain`、`read_output.rs` 測試改讀取函式 → 3.2 改送 ansi 並改同一批測試 | 同檔依序修改，順序正確 |
| 2.1 ↔ 4.1 | 2.1 讓 `OutputBody` 改用讀取函式 → 4.1 加 `segments` | 依序，無衝突 |
| 2.1 ↔ 5.1 | 2.1 讓 `ui_preview` 既有模式用 `plain` → 5.1 加 `ansi`／`ansi-flip` | 依序，無衝突 |
| 3.1 ↔ 5.1 | `pub mod ansi` → `ui_preview` 以真解析器產生片段 | 5.1 依賴 3.1 的 `pub` 可見性；3.1 brief 註明 |
| 5.1 ↔ 5.3 ↔ 5.2 | `ui_preview` 樣本 → 腳本斷言 → 前端實作 | 改為 5.3 先於 5.2（見裁決） |
| 5.3 ↔ 5.4 | 腳本 `--screenshots` → 截圖 | 5.4 擴充 5.3 的腳本；依序 |
| 5.4 ↔ 5.5 | 截圖 → 外觀審核 | 依序 |
| 6.1 | `live-output-real-check.js` 可能逐字比對 `text` | design D3 已預告；task 文字一致 |
| 2.1 自身 | 檔案清單 8 處、驗收含 runtime-model 新 scenario | 一致 |
| 3.1 自身 | 測試清單與 spec「輸出樣式轉換」逐條對應 | 一致（含 DEL／C1、32 參數、`4:3`） |
| 3.2 自身 | 測試對應 herdr-runtime-session delta 三個 scenario 的變動 | 一致 |
| 4.1 自身 | 端點兩個 scenario | 一致 |
| 5.1 自身 | 樣本涵蓋對比全組合 | 一致（design D8） |
| 5.3 自身 | 紅的程序 | 改由裁決取代 |
| 7.1 自身 | 改主 spec Purpose | OpenSpec instructions 規定 Purpose 直接改主 spec，一致 |

## 進度

- Task 1.1: 基線（HEAD a13e76f）— workspace 1100 passed／0 failed／13 ignored（63 個測試套件）；ui_preview 40 passed；腳本：reconnect-check PASS、whatever-check PASS、actions-check PASS、channel-backoff-check PASS、factory-floor-check PASS（已還原 task-5.2-scenario-d.png）、live-output-check PASS（R 段未觸發偶發競態）、visual-check PASS、files-check PASS、git-check 首跑 FAIL（重跑 FAIL，第三次 PASS）、progress-check PASS、ui-fixes-check PASS。git-check 兩次 FAIL 都只有收尾衛生一條：「結束後沒有殘留的 headless Chrome（user-data-dir 含 cockpit-chrome-gitcheck-）」；其餘所有段落皆 PASS；實測看到一個 Chrome utility 子行程（chrome.mojom 系）在腳本收尾後約數秒才消失，屬環境時序競態，非產品斷言；第三次 PASS。
- Task 1.1: complete（控制端核對：數字與報告一致）。git-check「收尾衛生」偶發 FAIL 列為已知偶發（Chrome utility 子行程延遲退出），後續遇到重跑並記錄。
- Task 2.1: complete（commits 966c131..431a7ca；控制端核對 red=新測試編譯失敗、green=workspace 1107／0、ui_preview 40、fmt／clippy 乾淨；7 個新測試對應 brief）
- Task 3.1: 實作 d035c53（36 測試；red＝E0432 編譯失敗＋歸色替身 10 FAILED；workspace 1143／0／13）。實作者提出三個 spec 邊角：
  - Ruling: SGR 參數上限 32 計入冒號子參數（照 vte 行為改 spec 措辭，commit c224767）— spec 原文未定義子參數算不算、真機不會遇到 — 若錯：
    極長 SGR 的取捨不同，無實際影響。
  - Ruling: `58`（底線顏色）照 `38`／`48` 規則吃參數但不產生樣式（改 spec 並加 scenario，commit c224767；交回實作者修）— 照原字面
    `58;5;1` 的 `1` 會被誤判為粗體 — 若錯：`58` 實際出現時的呈現不同，目前真機未見。
  - Ruling: `38;5:1`（被吃掉的參數帶冒號）不生效、冒號寫法只接受 3／5／6 個元素、序列中夾的 C0 由 vte 執行（`\n`／`\t` 保留）維持實作現狀 —
    皆為 spec 未定義的畸形輸入，現狀與終端機行為一致 — 若錯：畸形輸入的呈現差一兩個字元。
- Task 3.1: fix round 1/1（58 與子參數上限，a691b09；58 兩測試先紅後綠，另兩條鎖住既有行為）
- Task 3.1: complete（commits e673661..a691b09，控制端核對；39 測試）
- Task 3.2: complete（commits 248e79b..e87b131，控制端核對；red 2 FAILED→green，read_output.rs 9 passed，workspace 全綠）
- Task 4.1: complete（commits a14cce2..4188914，控制端核對；red 2 failed（segments 為 Null）→green，output_endpoint 19 passed）
- Task 4.2: complete（控制端執行：fmt／clippy 乾淨、workspace 1149 passed／0 failed／13 ignored；ui_preview /output 實測 wJ:p1（3 行）、wJ:p3（200 行、truncated true）皆 segments 單段無樣式、串接等於 text、format text）
- Task 4.3: 後端階段審查（Opus 5.5 取代 Codex，範圍 a13e76f..283b1d3）— approve，Critical 0／Important 0／Minor 5（報告：工作區 review-4.3-report.md）。
  審查者以 16,777,216 個真彩色＋256 色與兩套獨立參考實作比對歸色，0 不符。
  - Ruling: F1–F3（8-bit C1 引導字元、DCS 內文含 0x9C、ESC 後接非 ASCII）接受不改，於 design D1 註明已知限制 — vte 只認 7-bit 序列，HERDR
    實測只輸出 7-bit SGR — 若錯：日後 HERDR 輸出 8-bit 序列時畫面多出幾個殘字，屆時再自行前處理。
  - Task 4.3: minor (deferred): F4 `cockpit-core/src/runtime.rs`／`cockpit-herdr/src/runtime.rs` doc 殘留（OutputFormat 重複行、PaneOutput doc 仍寫
    「用 text 去重」、design D 編號未標出處）— 7.3 前修。
  - Task 4.3: minor (deferred): F5 端點層缺「text 為空時 segments 為 []」與「頂層恰為六個鍵」測試 — 7.3 前補。
- Task 4.3: complete
- Task 5.1: complete（commits b2e57a8..39b4468，控制端核對；red＝編譯失敗→green，7 測試、83 個標籤（定位方式見 ui_preview.rs ansi_sample 文件註解）；11 支既有腳本全 PASS；visual-check 首跑是實作者自設 timeout 280 殺掉，重跑 PASS）
- Task 5.3: 腳本 7fb0920（58 條斷言；在未改前端的 HEAD 4632bbb 跑出 29 FAIL／29 PASS，紅皆為依賴新行為者；紅證據 工作區 task-5.3-red.txt）。待 5.2 後轉綠才算完成。
  - Ruling: 「過期時有色內容一併轉暗」的斷言加上「兩個片段為各自獨立節點」與「503 前有色、恢復後回到有色」— 舊前端整框轉暗會讓單純比文字色變成假綠；
    要分別上色本來就需要獨立節點 — 若錯：該條斷言比 spec 多要求一點 DOM 形狀，日後改成非 span 呈現時需改腳本。
- Task 5.2: complete（commits 39e2e9a..74ee09d：產品 fe8f2b4、ui_preview 補 [bg=cyan] 685f15a＋74ee09d、visual-check CL1 97dedba；控制端核對無 innerHTML／style 屬性、reverse 不加 ansi-bg*、片段字串去重、segments 非陣列防禦）。output-color-check 59 PASS／0 FAIL（對比最低 4.69:1）；11 支既有腳本全 PASS（live-output-check 首跑一項 wJ:p1 pane 列焦點斷言 FAIL、重跑 PASS——新的偶發位置，記錄觀察）；ui_preview 47、cockpit 全綠。
  - Ruling: visual-check.js CL1「沒有死規則」新增兩個走訪狀態（選 ansi pane、fetch 攔截 503 的過期＋上色）而非豁免 ansi-* 規則 — 斷言未放寬、覆蓋擴大 — 若錯：CL1 多兩段走訪，執行時間略增。
  - Ruling: 接受 ui_preview ansi 樣本補 `[bg=cyan]`（46），標籤 83→84 — 讓 `.ansi-bg-cyan` 有元素可對；動到 5.1 產物但只增不改 — 若錯：無。
- Task 5.3: complete（7fb0920；5.2 後 59 PASS／0 FAIL，紅→綠完成）
- Task 5.4: complete（fe60fdb；控制端逐張看過 1536／1100／700 三張圖：無真名，畫面上的 D:\projects\<user>\ai-cockpit 是 fixture cockpit/tests/fixtures/projected-state.json 原有佔位字；實作者 PNG 位元組 grep 真名 0）
- Task 5.5: complete（控制端以 frontend-design 審核模式評 output-color-{1536,1100,700}.png；spec 定死項不評）— 無必改項。
  - Task 5.5: minor (deferred): 連續兩行都有背景時行間有細縫（行內 span 背景只畫字身高，不是 1.4 行高）；補滿需實測字身高度，補過頭會讓相鄰行的 20% 淡底
    重疊加深；使用者核可的原型即為此樣貌 — 延後。
  - Task 5.5: 交使用者：12px Cascadia Mono 下粗體 600 與一般字差距小；改 700 屬 spec 變更，不擅改。
- Ruling: 7.1（三處文件字句：CONTEXT.md「Live Output」、設計文件決策表兩列、主 spec live-output Purpose 第一句）由控制端直接改，不派實作者 — 純文件、各一句，且在 5.6 審查進行中可並行不衝突 — 若錯：措辭不當，由 7.3 整支分支審查把關。
- Task 7.1: complete（markdownlint 142 files 0 issues、openspec validate --all 20 passed）
- Task 5.6: 前端階段審查（Opus 5.5 取代 Codex，範圍 b2e57a8..6c74276）— needs-fix，Critical 0／Important 1／Minor 5；產品碼無缺陷（審查者以 Fetch 注入
  16×16 前景背景＋dim／reverse 共 371 組合實測全符 spec，最低對比 4.69:1）。F1（Important）：16 色前景與亮色背景無驗收覆蓋。
  - Ruling: F2（200 本體為 null 時輪詢靜默停止，改版前既有）與 F3（防禦路徑 textContent 可能≠text）一併修，不延後 — 修法各一兩行；F2 是「不報錯、
    畫面凍結」型缺陷，F3 讓 spec「textContent 等於 text」在所有路徑成立 — 若錯：多一點範圍，但都有新斷言把關。
  - Ruling: 4.3 延後的 F4、F5 併入同一修正波 — 都是收尾前要補的小項，一次派工少重建脈絡 — 若錯：無。
  - Task 5.6: fix wave 派工（5.6 F1–F6、4.3 F4–F5），起點 51a56ae。
- Task 5.6: fix round 1/5（427debd..3387cef：產品 3ced345、Rust d8ad13f、腳本＋重拍截圖 895dbbc、design 3387cef）— 5.6 F1–F6、4.3 F4–F5 全修；紅綠證據見工作區
  fix-5.6-report.md；workspace／ui_preview 48／output-color-check（A9 117／117，最低 4.69:1）／11 支既有腳本全綠。
  - Ruling: F2 修正把「本體為陣列」也視為不合法（報告只提 null／非物件）— spec 規定回應是 JSON 物件，`[]` 原本會清空面板 — 若錯：陣列本體多一次
    「失敗」呈現而非清空，對合法回應無影響。
- Task 5.6: 複審（Opus，427debd..3387cef）— 8 項全 ADDRESSED，無新 Critical／Important；複審者以 repo 外變異測試證明 F5 三項收緊與 M1／X1 會紅。
  - Task 5.6: minor (deferred): O1 200 但本體不合法（非 JSON／非物件）持續發生時面板不標過期——change 3 既有行為，spec 失敗清單只列非 2xx 與請求失敗。
  - Task 5.6: minor (deferred): O2 M1 未斷言「不合法本體不清掉既有過期標示」。
  - Task 5.6: minor (deferred): 4.3 F4 殘留——`cockpit-herdr/src/runtime.rs` 的 design D4、`cockpit-core/src/runtime.rs` 的 design D2 未標出自哪個 change。
- Task 5.6: complete（fix round 1，review clean）
- Task 6.1: complete（live-output-real-check.js 原樣 RESULT: PASS，53 ok／0 FAIL；腳本不逐字比對 text，D3 差異不影響；WSL 測試 server 與專屬 socket 已收；無 commit）
- Ruling: 6.2 真機截圖不進 repo（存 repo 外、控制端看過即刪），`output-color-live.md` 只記中繼資料 — 真機 pane 畫面是使用者其他工作的內容，遮罩只能處理路徑與
  名稱、擋不住畫面本身 — 若錯：repo 少一張真機外觀證據，可由使用者自行開 Cockpit 確認。
- Task 6.2: complete（faeeec4 output-color-live.md，只記中繼資料；Windows 真機 wW:p1：107 段／53 帶樣式全符色票、textContent 等於自身回應 text（1＋8 次）、503 過期全暗且恢復、
  端點 text 與 HERDR format=text 差異只有 2 行行尾空白＋結尾換行；控制端看過 repo 外截圖（綠 Reconnected、紅 failed 正確上色）後已刪；文件 grep 真名與其他 repo 名 0）
  - Task 6.2: 真機未見背景、粗體、斜體、底線、反白（閒置 Claude pane 只有前景色與 dim），這些只由 ui_preview 覆蓋；已寫入 output-color-live.md「未能驗證」。
- Task 7.2: 全 gate（HEAD 642413a）— fmt／clippy 乾淨；workspace 1151 passed／0 failed／13 ignored；ui_preview 48；markdownlint 143 files 0 issues；openspec validate 20 passed；腳本：reconnect PASS、whatever PASS、actions PASS、channel-backoff PASS、factory-floor FAIL(6，已知 flake；重跑 PASS)、live-output PASS、visual-check PASS、files PASS、git PASS、progress PASS、ui-fixes PASS；output-color-check PASS
- Task 7.3: 整支分支審查（Opus 5.5 取代 Codex，1d3c475..1157649）— ready to merge，Critical 0／Important 0／Minor 5／nit 2（報告：工作區 review-7.3-report.md）。
  審查者另以 repo 外 `openspec archive --yes` 模擬套用成功、Windows 4 pane 唯讀探針去控制碼後與 format=text 無內容差異、36 檔＋37 commit 訊息真名 0 命中。
  - Task 7.3: 交使用者：M1 色相歸色的無彩門檻（最大−最小 < 64）會把 Claude Code 深色主題 diff 底色在 256 色／真彩色下歸成灰；HERDR 目前只傳 16 色故無影響，
    HERDR 改傳 256／真彩色時會不報錯地畫錯 — 建議另開小 change 調門檻。
  - Task 7.3: minor (deferred): M3 404 轉暗、「dim＋背景」「dim＋反白」對比只有 5.6 一次性探針驗過，未寫成斷言。
  - Task 7.3: minor (deferred): M4 8-bit C1／DCS 已知限制只在 design D1，主 spec 字面比實作嚴格。
  - Task 7.3: minor (deferred): M5 註解標號殘留三處（即 4.3 F4 殘留，原註「7.3 前修」改判延後）＋一處舊註解。
  - Ruling: 7.3 的 Minor 全數延後、不再開修正波 — 審查判定無合併前必修，均為註解、驗收補強或未發生的情境 — 若錯：延後項目需在之後的 change 補，記於 handover。
- Task 7.3: complete
