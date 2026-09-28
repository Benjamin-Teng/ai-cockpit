# SDD ledger — plan: openspec/changes/file-review/tasks.md

> 快照：SDD 控制端本機 ledger（`.superpowers/sdd/tasks/progress.md`）在 change file-review 收斂時的存檔，供 archive 與交接參考；本機路徑中的使用者名稱已換成 `<USER>`。

Spec：openspec/changes/file-review/specs/{file-review,cockpit-dashboard,live-output}/spec.md；design.md。
分支 feat/file-review，起點 7aebc3b。

## Pre-flight 衝突掃描

| 對象 | 產出 ↔ 消費 | 發現 | 裁決 |
|---|---|---|---|
| SDD reviewer 步驟 ↔ ~/.claude/CLAUDE.md | — | skill 要 Claude task reviewer；CLAUDE.md 規定 diff 審查一律 Codex | R1 |
| 1.3 ↔ 2.5、3.3、4.5 | 1.3 產 vendor/；2.5 讀 material-icons.json、3.3 內嵌、4.5 載入 pdf.js | 依賴順序與 tasks 順序一致 | 無衝突 |
| 1.2 ↔ 3.2、4.3 | 1.2 決定 WSL vscode_uri 是否存在 | 3.2 產 vscode_uri 前需要 1.2 結果 | 1.2 先做（需使用者點連結） |
| 2.1 | 「版本依 design」但 design 沒寫版本 | probe §4 有版本：comrak 0.55.0、ignore 0.4.33、include_dir 0.7.4 | R2 |
| 2.2 ↔ 3.2 | 2.2 定相對路徑 API；3.2 由 HTTP 傳入 | axum `Path` 會先整段 percent-decode，`a%2Fb` 會變兩段而漏擋 | R3 |
| 3.2 | spec 要 HEAD → 405 | axum `get()` 自動接 HEAD | R4（寫進 3.2 dispatch） |
| 3.2 vscode_uri | 需要 percent-encode | 未核准新 crate | R5 |
| 3.4 ↔ 3.5 ↔ 4.x | 3.4 產 fixture 與假 pane；3.5 腳本依 fixture；4.x 讓段落轉綠 | 一致 | 無衝突 |
| 4.1 ↔ 既有腳本 | 分頁化改前提 | tasks 通則已規定只改 spec 已改的斷言、分開 commit | 無衝突 |
| 2.1 自洽 | ADR 0006＋crate＋根目錄推算 | 自洽 | — |
| 2.2 自洽 | junction 退路、#[ignore] 註明 | 自洽 | — |
| 2.3 自洽 | 從根目錄往下走、require_git(false) | 自洽 | — |
| 2.4 自洽 | 8192 bytes、截斷 UTF-8 | 自洽 | — |
| 2.5 自洽 | 實檔測試讀 cockpit/assets/vendor（跨 crate 相對路徑） | 可行，檔案不存在時失敗 | — |
| 2.6 自洽 | comrak header_ids 前綴 md- | API 名稱需查該版 docs.rs | 寫進 dispatch（鐵則 3） |
| 3.1–3.3 自洽 | — | 自洽 | — |
| 5.2 | 需 WSL 端測試 server | 不可用時 ledger 註明 | — |

- Ruling R1: 每個 task 的 reviewer 一律用 Codex `adversarial-review`（CLAUDE.md 路徑②），focus 帶 brief 路徑與 spec scenario；不派 Claude reviewer；findings 先重現才採信 — 使用者全域規則優先於 skill — 若錯：Codex 靜態推導漏掉的問題較多，最後整支分支 review 補網。
- Ruling R2: 依賴版本取 probe §4 查證值（comrak 0.55.0、ignore 0.4.33、include_dir 0.7.4），API 以該版 docs.rs 為準 — design 未寫、probe 是本 change 的查證紀錄 — 若錯：升版只影響 Cargo.toml 與少量 API。
- Ruling R3: `cockpit-files` 的相對路徑 API 吃「仍是 percent-encoded 的原始字串」，自己以 `/` 切段再逐段解碼與檢查；HTTP 層必須從原始 URI 取路徑，不得用 axum `Path` 已解碼的值 — spec 要求 `%2F` 解碼後的 `/` 回 400 — 若錯：界限檢查可能被繞過（安全），故取嚴格路線。
- Ruling R4: 3.2 必須讓 HEAD 回 405（axum `get` 預設接 HEAD，需顯式處理）— spec「不接受其他 method」— 若錯：一條 scenario 失敗。
- Ruling R5: percent-encode／decode 手寫（小函式＋單元測試），不新增未經使用者同意的 crate；若 workspace 既有依賴已直接帶 `percent-encoding` 可用之 — CLAUDE.md「裝新套件先問」 — 若錯：多幾十行程式碼。

## 進度

- Task 1.3: implementer DONE（commit 9bc1d0d，BASE 7aebc3b）；report task-1.3-report.md。SVG 1251、SHA-256 三處一致、markdownlint 0 issues。
- Ruling R6: 4.5 的 pdf.js getDocument 一併設定 `wasmUrl`（/vendor/pdfjs/wasm/）與 `iccUrl`（/vendor/pdfjs/iccs/），不只 D7 列的 cMapUrl／standardFontDataUrl — 1.3 查證 6.3.289 有四個資源目錄選項、目錄都已 vendor；不設時 JBIG2／JPEG2000 影像與 ICC 色彩可能畫不出 — 若錯：多兩行設定，無害。
- 1.2：使用者請控制端自行實測（2026-09-27）。
- Task 1.1: complete（純驗證，無 commit）— 基線：cargo test --workspace 531 passed／0 failed／10 ignored；ui_preview example 13／0／0；reconnect、whatever、actions、channel-backoff、factory-floor 皆 PASS；live-output-check 345 ok／0 FAIL；visual-check 1464 ok／0 FAIL／0 PEND。細節 task-1.1-report.md。
- Task 1.2: complete（commit ab1fb08，控制端執行；使用者授權代測）— WSL 連結需結尾 `:1`，否則 VS Code 當資料夾開；spec「在 VS Code 開啟」與 scenario「WSL 檔案」已改；openspec validate 17 passed；markdownlint 0 issues。
- Ruling R7: 1.2 以 Windows protocol handler＋UI Automation 讀 VS Code 確認框實測，而非在 Chrome 點擊；Chrome 端點擊留待 5.1 使用者目視 — 使用者請控制端代測、Chrome 外部協定確認框無法自動化 — 若錯：Chrome 若對 `:1` 或 `%20` 另行改寫，5.1 會發現，修正只動 vscode_uri 產生函式。
- Ruling R8: 1.2 為純文件／spec 修改，不單獨跑 Codex review，由 5.4 整支分支 review 涵蓋 — 無程式 diff — 若錯：spec 用詞問題延後到 5.4 才被發現。
- Task 1.3: Codex review needs-attention（1 medium：README 升版指令 cp -r 巢狀複製，控制端已重現）→ fix round 1 進行中（resume implementer）。
- Task 1.3: fix round 1/5 (1 addressed, 1 new open — README 升版指令下載失敗仍會 rm -rf; commits 9bc1d0d..02ecbaa)
- Task 1.3: fix round 2/5 (1 addressed, 0 open; commits 02ecbaa..caf9120)
- Task 1.3: complete (commits 7aebc3b..caf9120, review clean)
- Task 2.1: dispatched（BASE 8b6146a，sonnet）；共用約定 cockpit-files-conventions.md
- Task 2.1: complete (commits 8b6146a..d7f3964, review clean) — workspace 536 passed／0 failed／10 ignored
- Task 2.2: dispatched（BASE d7f3964，opus——安全邊界）
- Task 2.2: implementer DONE（9c20a8f，33 tests）。Codex needs-attention：1 medium（根外 junction 下不存在的目標回 NotFound → 可探測根外存在性）。
- Ruling R9: 封住存在性探測——canonicalize 失敗時以最長可解析祖先判斷，在根外就回 PathOutsideRoot — spec「解析為實體路徑後必須在根內，否則 403」— 若錯：根內「不存在」與「根外」的區分變粗，對使用者無可見影響。
- Ruling R10: `<>"|?*` 與控制字元片段不另擋，讓它落到 io_error；tab 結尾與 COM0／LPT0 多擋 — spec 未列，前者不構成跳出途徑、後者寧嚴 — 若錯：這類請求回 500 而非 400。
- Task 2.2: fix round 1/5 (1 addressed, 1 new open — 懸空 junction 仍回退 NotFound 可探測; commits 9c20a8f..d01adbd)
- Ruling R11: 往上走時「存在但無法解析」的項目（懸空連結／reparse point）一律 PathOutsideRoot，含指向根內者；非 NotFound 錯誤回 Io — 一致拒絕優先於精準分類 — 若錯：根內懸空連結顯示「跳出根目錄」而非「不存在」，文案略失準。
- Task 2.2: fix round 2/5 (1 addressed, 0 open; commits d01adbd..64c075c)
- Task 2.2: complete (commits d7f3964..64c075c, review clean) — cockpit-files 40 tests
- Task 2.3: dispatched（BASE 64c075c，sonnet）
- Task 2.3: implementer DONE（c433059，50 tests）；偏離 D4：祖先鏈有連結時 follow_links(true)。Codex needs-attention：2 high（先出根再回根的連結鏈使根外 .gitignore 生效；walk 錯誤被吞成成功空清單）、2 medium（預設 .ignore 生效；祖先鏈字面比較使 DOCS／8.3 別名回空清單）。
- Ruling R12: 2.3 改設計，不用 WalkBuilder 走祖先鏈：(a) 對 rel 的每個前綴（含根與目標）canonicalize，任一層實體不在根內 → PathOutsideRoot（沿用 2.2 的懸空連結一致拒絕）；(b) 以 `ignore::gitignore::GitignoreBuilder` 為每一層「邏輯路徑」載入該層實體目錄下的 `.gitignore`（根是 git repo 且 `.git` 為資料夾時加 `.git/info/exclude`），不讀 `.ignore`、不讀全域設定、不讀根以上；比對用邏輯路徑、由深到淺取第一個非 None 的結果；Windows 上 case_insensitive(true)（同 Git for Windows 預設 core.ignorecase）；(c) `fs::read_dir(目標實體路徑)`，任何 read_dir／項目錯誤 → FilesError::Io（不吞）；(d) 8.3 短名稱出現在請求路徑時 gitignore 以字面比對、可能不生效，接受並記入 report — 四條 finding 同源於「沿邏輯路徑 walk」，換做法一次解決、比逐條補洞可靠（handover：同類 finding 反覆冒就換設計） — 若錯：gitignore 語意細節（例如父目錄被忽略時子項目）與 git 不同，影響只在列表顯示，不影響安全邊界；design D4 待 2.3 完成後同步改寫。
- Task 2.3: fix round 1/5 (4 addressed, 2 new open — 規則檔本身是根外連結仍生效［high］；已忽略祖先內的子項目仍列出［medium］; commits c433059..45e77e2)；design D4 已依 R12 改寫（未 commit）。
- Ruling R13: 規則來源（.gitignore、.git、.git/info、exclude）任一是連結／reparse point 就略過該來源（同 git 不跟隨 .gitignore symlink）；祖先目錄被忽略時列表回空 entries（非錯誤），其內規則不得重新納入 — spec「過濾只取根目錄以內」＋git 語意 — 若錯：使用者故意以連結共用 .gitignore 時規則不生效（git 本身也不生效）。
- Task 2.3: fix round 2/5 (2 addressed, 0 open; commits 45e77e2..5c6835b)
- Task 2.3: complete (commits 64c075c..5c6835b, review clean) — cockpit-files 60 tests；檔案 symlink 路徑因本機無權限（1314）無實機證據，與 junction 共用判斷邏輯。
- Task 2.4: dispatched（BASE 4c3722c，sonnet）
- Task 2.4: complete (commits 4c3722c..16034dd, review clean) — 72 tests；modified_ms 為 i64
- Task 2.5: dispatched（BASE 16034dd，sonnet）
- Task 2.5: complete (commits 16034dd..f6ac0a3, review clean) — 92 tests
- Task 2.6: dispatched（BASE f6ac0a3，sonnet）
- Task 2.6: implementer DONE（aedf886，112 tests）；comrak 0.55 選項為 extension.header_id_prefix；中文標題 id＝md-原字、空白轉 -；comrak 會為每個標題加 `<a href="#未加前綴id" class="anchor">`（4.4 的 # 連結改寫會加 md- 前綴，一致）
- Task 2.6: Codex needs-attention 1 medium（大小預檢後 fs::read 無上限，並行變大可繞過 2 MiB）→ fix round 1：抽出有上限讀取函式，3.2 的 50 MiB 原始內容也要用它（帶入 3.2 dispatch）
- Task 2.6: fix round 1/5 (1 addressed, 0 open; commits aedf886..b93e204)；新增 pub read_capped(path, limit)
- Task 2.6: complete (commits f6ac0a3..b93e204, review clean)
- Ruling R14: 3.2 以 `include_bytes!` 內嵌 `material-icons.json`、啟動時 `IconTheme::from_json` 解析一次放進狀態；list／meta 回應的 icon 欄位是 SVG 檔名（spec「icon 檔名」），前端自行拼 `/vendor/material-icons/icons/<檔名>`；3.3 的 include_dir 不必重做這件事 — 3.2 在 3.3 之前、spec 要檔名 — 若錯：前端拼接路徑要改一行。
- Ruling R15: 檔案端點的檔案系統操作在 `tokio::task::spawn_blocking` 內做（WSL UNC 慢，不可卡住 async runtime） — design 風險段 — 若錯：多一層包裝，無害。
- Task 3.1: dispatched（BASE c5fe16b，opus）
- Task 3.1: implementer DONE（8660c63；files_endpoint 33 tests；cockpit 全綠）。給 3.2：`cockpit::files::authorize_root(&app, runtime, root_id) -> Result<Root, FileApiError>`，FileApiError 可 into_response；路由順序 get → fallback(405) → route_layer(source_check) → head(405)。
- Ruling R16: 非 WSL runtime 的 cwd 不是絕對路徑或含 `..` 時視為沒有根目錄；`command` 型端點一律當非 WSL（POSIX cwd 得 no_root）；root_id 先解碼（400）再查 runtime（404） — fail-closed，且 HERDR 正常不回報這些形狀 — 若錯：極少數 pane 顯示「沒有根目錄」。
- Codex 額度用盡（2026-09-27 19:49，恢復時間 22:37）：3.1 review 兩次失敗（第一次只回開場白、第二次 usage limit）。
- Ruling R17: Codex 恢復前繼續派後續 task，review 排隊；未經 Codex 通過的 task 不勾選、不宣稱完成；恢復後依序補審（3.1 起），findings 照常進 fix loop，必要時連帶修改後續 task — CLAUDE.md 規定 diff 審查只能 Codex、不得以 Claude reviewer 取代；停等 3 小時浪費使用者時間 — 若錯：後續 task 疊在有問題的程式上，補審後需要連帶返工。
- REVIEW BACKLOG: 3.1（BASE c5fe16b..8660c63）
- Task 3.2: dispatched（BASE 8660c63，opus）
- Task 3.2: implementer DONE（25af003；cockpit 217 passed、files_endpoint 61；ui_preview 13）。未改既有斷言（既有測試沒有逐字比對 403／405 本體）。
- Ruling R18: 未命中路由（`list/` 空尾、缺相對路徑、未知端點名）落到 axum 預設空本體 404、無 no-store／nosniff——接受 — 那些不是 spec 定義的端點，屬 cockpit-dashboard「其他路徑回 404」；改成單一 catch-all 路由會偏離路由形狀 — 若錯：少數錯誤網址少兩個標頭。
- Ruling R19: CSP sandbox 只加在渲染與原始內容處理常式產生的回應（含其錯誤），source_check 的 403／405 不加；meta 的 icon 依請求字面檔名、raw content-type 依實體路徑副檔名 — 前者不是內容、後者與 viewer 分類一致 — 若錯：8.3／連結情境 icon 與 content-type 不一致，只影響顯示。
- Task 3.2: minor (deferred): 寫入端點改用 fallback 後 405 少了 `Allow` 標頭；ui_preview 外層寫入路由仍回空本體 405（註解「跟正式路由一致」只剩狀態碼）。
- REVIEW BACKLOG: 3.2（8660c63..25af003）
- Task 3.3: dispatched（BASE 25af003，sonnet）
- Task 3.3: implementer DONE（cb254af；http.rs 13 passed；單一執行檔驗收 200；debug 執行檔 +6.14 MiB）。material-icons.json 改由 VENDOR_DIR 執行期取（不再重複內嵌）。
- REVIEW BACKLOG: 3.3（25af003..cb254af）
- Task 3.4: dispatched（BASE cb254af，sonnet）
- Task 3.4: 控制端 session 中斷（21:07 恢復）；agent 已 commit 494d0fb、fbe5546（whatever-check pane 列數 3→5，屬 fixture 資料計數）未寫 report；新假 pane 為 exited（選不到）→ resume 要求說明或改非 exited
- Task 3.4: implementer DONE（494d0fb、fbe5546、1a1f696、e009997）：fixture review-repo／other-repo、make-report-pdf.js；假 pane wJ:p4（review-repo/src）、wJ:p5（other-repo）非 exited、idle；既有腳本斷言修改兩處皆為 fixture 計數類：whatever-check pane 列數 3→5、actions-check 改綁候選 2→4 筆；ui_preview 19 tests；六支＋visual-check 全綠。Ctrl+C 清理路徑無實機證據（強殺＋重啟清理已驗）。
- REVIEW BACKLOG: 3.4（cb254af..e009997）
- Task 3.5: dispatched（BASE e009997，opus）
- Ruling R20: Codex 恢復後把 backlog 3.1–3.4 合成一次審查（--base c5fe16b，HEAD 至 3.4 末），findings 交單一修正 agent — adversarial-review 審的是 base 到工作樹的整段 diff，逐 task 分審會重疊 — 若錯：一次審查範圍較大、可能漏看細節，5.4 整支分支 review 補網。
- Task 3.5: implementer DONE（f2b5d6d）：files-check.js 23 scenario 段全 RED、自我測試 2/2 PASS；visual-check 新增 FT1–FT3（RED），既有 25 段全 PASS；前端契約 C1–C8 見 docs/research/2026-09-27/files-check.md「前端契約」。
- Task 3.4: minor (deferred): ui_preview 啟動時刪掉所有 cockpit-ui-preview-* 暫存目錄（含另一個執行中 ui_preview 的），驗收腳本不能並行；建議只刪 PID 已不存在者。
- Ruling R21: 4.1 驗收中的三個 live-output 段（檔案分頁期間不請求輸出、切回時保持貼底、選定 pane 時切回 Live Output 分頁）改到 4.3 結束時驗收；4.1 仍實作 output.js 的可見／不可見入口與選定 pane 切回分頁 — 這三段都要先由檔案樹開出檔案分頁（4.2、4.3 的功能），4.1 單獨無法驗 — 若錯：4.1 的 output.js 改動要到 4.3 才被腳本驗到，錯誤發現晚兩個 task。
- REVIEW BACKLOG: 3.5（e009997..f2b5d6d，腳本）
- Task 4.1: dispatched（BASE f2b5d6d，opus）；補審 worktree 建於 scratchpad/review-wt（detached e009997）
- Codex 補審 3.1–3.4（worktree @e009997，base c5fe16b）：needs-attention，唯一 finding [medium] ui_preview 啟動清理憑前綴刪除、且在 bind 前（即先前 deferred minor 升級）。3.1–3.3 無 finding。
- Task 3.4: fix round 2/5 dispatched（resume implementer）：先 bind 再準備 fixture、只刪 PID 已不存在的目錄。
- Task 4.1: implementer DONE（009e1a1 產品、3d85217 腳本）；六支＋visual-check 25 段 PASS（FT1–3 預期 FAIL）；files-check self 2 段＋「沒有選定 pane」PASS、其餘停在 C2；R21 三段以臨時 CDP probe 驗過入口。疑慮：1200×720 固定一屏時 Live Output 面板 227.6px（分頁區 268px）；files-check「重畫不影響檔案樹」偶發 1 次失敗（疑腳本 clickEl 在高頻重畫下點錯列）。
- Task 4.1 設計審核（控制端，三寬 6 張截圖＋frontend-design 準則）：通過。左欄與下半部分頁列同一語彙（表面色＋品牌色底線），無新顏色、無橫向捲軸，空狀態文案依 spec。minor (deferred): 1100 寬時 Factory Floor 外框頂緣比左欄分頁列低約 12px（可能為 change 4 既有）；1200×720 固定一屏時 Live Output 面板 227.6px。
- Task 4.1: Codex needs-attention 1 medium（切回時仍在進行的舊請求擋住立即請求、舊回應仍寫入面板）→ 待 3.4 fix 釋出 7770 後進 fix round 1
- Task 3.4: fix round 2/5 done（098c51f：先 bind 再建 fixture、只刪 tasklist 確認已死的 PID 目錄；ui_preview 29 tests；live-output-check、files-check self、reconnect-check PASS）
- Task 4.1: fix round 1 dispatched（resume implementer）
- Task 3.4: fix round 2 re-review approve。
- Task 3.1: complete (commits c5fe16b..8660c63, 合併審查 clean)
- Task 3.2: complete (commits 8660c63..25af003, 合併審查 clean)
- Task 3.3: complete (commits 25af003..cb254af, 合併審查 clean)
- Task 3.4: complete (commits cb254af..e009997＋098c51f, 1 fix round)
- Task 3.5: Codex needs-attention 3 medium（PDF 方框可假綠；FT2 未比對內容子節點 identity；切走後新發出的輸出請求被當成允許的舊請求）。
- Ruling R22: PDF「非方框」維持「像素比對＋控制端目視截圖」分工（tasks 4.5 驗收原文如此），但截圖失敗改判 FAIL、腳本輸出明確標「需目視確認：<路徑>」；另兩條照 Codex 建議修並加負對照 — plan 明定目視、字形辨識自動化成本高 — 若錯：方框問題靠控制端目視把關，漏看就漏。
- Task 4.1: fix round 1 done（e1586b8：切回淘汰舊請求；延遲 3000→17 ms、卡住 5988→11 ms）
- Task 3.5: fix round 1 dispatched（resume implementer；R22）
- Task 4.1: complete (commits f2b5d6d..e1586b8, 1 fix round；三個 live-output 段依 R21 於 4.3 驗收)
- Task 3.5: fix round 1 done（b8aae60）；files-check：self 2/2、沒有選定 pane PASS，其餘 22 段停在 C2
- Task 4.2: dispatched（BASE 40fa764，opus）
- Task 3.5: fix round 1/5 (2 addressed［PDF、FT2］, 1 partially + new — [high] MutationObserver 回呼時間當切換邊界會把切回後立即請求誤判為切走期間（假紅）；[medium] 只包 fetch 漏 XHR、CDP 交叉檢查窗口從 openFile 返回才開始; commits e1586b8..b8aae60)
- Ruling R23: 3.5 fix round 2 改為「請求發出當下同步記錄 Live Output 分頁狀態」（fetch 與 XHR 都包），分類以此為準，CDP 只做總數健全檢查；排在 4.2 之後、4.3 之前（避免 7770 衝突，且這些段落 4.3 才用得到） — 同步記錄消除時間對齊問題、比校準 CDP timestamp 簡單可靠 — 若錯：以 WebSocket／sendBeacon 等其他途徑發的輸出請求漏抓（產品只用 fetch）。
- Task 4.2: implementer DONE（3376a8b ui_preview、e88a587 產品、73abb20 visual-check CL1 走訪加入檔案樹狀態，斷言未改）；files-check 4.2 四段＋self PASS、其餘停 C3；六支全綠；visual-check ok 1527（FT1–3 預期 FAIL）。
- Ruling R24: 接受 4.2 改 ui_preview（wJ:p4／p5 加 ticker 輸出模式，否則輸出 404 會清掉選取、檔案樹無法驗）；取消選取時顯示空狀態、展開狀態保留；在 Project 分頁改選 pane 不查根目錄、切到檔案分頁才查 — 符合 spec「切到此分頁或根目錄改變時讀取」 — 若錯：切到檔案分頁時多一個短暫的讀取中狀態。
- Task 3.5: fix round 2 dispatched（R23）
- Task 4.2 設計審核：通過（三寬截圖；樹縮排／展開箭頭／icon 對齊清楚，過濾正確，窄視窗正常；frontend-design 無 finding）
- Codex 額度再次用盡（2026-09-28 約 00:30，恢復 03:39）。REVIEW BACKLOG: 4.2（40fa764..73abb20）；3.5 fix round 2（待 commit）
- Task 3.5: fix round 2 done（3a230d9，R23）；REVIEW BACKLOG 加 3.5 r2（b8aae60..3a230d9）
- Task 4.3: dispatched（BASE 3a230d9，opus）
- Task 4.3: implementer DONE（8dd788c 產品、08ccb6a visual-check CL1 走訪，斷言未改）；files-check ok 483：4.3 八段中七段 PASS＋R21 三個 live-output 段 PASS；「重新整理後還原」只剩「顯示 docs/a.md 內容」一條、FT2 只剩「long.md 顯示 Markdown 內容」→ 依賴 4.4，留 4.4 驗收；visual-check ok 1616（FT1 PASS）；六支全綠。
- Task 4.3: minor (deferred): 寬 <760 單欄時開檔到中繼資料回來前多一行「正在讀取…」使分頁區高度跳動一次；tablist 內有 role=presentation 包裝的關閉 button；4.4 做完後 `.file-placeholder` 規則要刪（CL1 死規則）。
- REVIEW BACKLOG: 4.3（3a230d9..08ccb6a）
- Task 4.3 設計審核：通過（分頁列語彙一致、工具列、20 長檔名分頁內部橫捲）
- Task 4.4: dispatched（BASE 08ccb6a，opus）
- Task 4.4: implementer DONE（46c33ef 產品、9d27b6a visual-check CL1 走訪，斷言未改）；files-check ok 500（剩中文 PDF、改檔後更新、刪掉又出現三段屬 4.5／4.6）；visual-check FAIL 0（FT1–3 PASS）；六支全綠；XSS probe 78/0（含模擬 comrak 清洗失效）。
- Ruling R25: Markdown 以 `/` 開頭的相對連結以 repo 根目錄為基準（同 GitHub）；前端另加元素／屬性白名單清洗（縱深防禦，comrak 新增輸出元素時要同步 ALLOWED）；相對路徑自行逐段解析（`new URL` 會把越界 `..` 截在根，無法偵測跳出） — D8 未明寫、此做法較嚴 — 若錯：`/` 連結開到 repo 根而非磁碟根。
- Task 4.4: 待使用者決定（spec 缺口）：沒有 `<meta charset>` 的 UTF-8 HTML 在 iframe 會亂碼——spec 規定 raw `.html` 回 `text/html` 不帶 charset；修正需改 spec（例如 `text/html; charset=utf-8`）。
- Task 4.4 設計審核：通過；minor (deferred): Markdown 內文無最大行寬，寬螢幕一行遠超 80 字（frontend-design 行長準則），建議內文 max-width 約 80ch。
- Task 4.4: 腳本疑慮：「改檔後更新並保住捲動」前置步驟找內容捲動容器得 null，疑腳本時序（比對到工具列檔名就開始量）→ 交 4.6 確認。
- REVIEW BACKLOG: 4.4（08ccb6a..9d27b6a）
- Task 4.5: dispatched（BASE 9d27b6a，opus）
- Task 4.5: implementer DONE（aaacea0 產品、0932a11 visual-check CL1 走訪，斷言未改）；files-check「中文 PDF」PASS；visual-check ok 1642；六支全綠；開 PDF 只有本服務請求；worker 釋放 1→1→0。
- Task 4.5 目視（R22）：中文標題與內文正確顯示、非方框（files-check-chinese-pdf-…17-42-47-985Z.png、200% 截圖）。設計審核通過。
- Ruling R26: 「PDF 無法解析」視為成功讀取後的顯示內容（不算讀取失敗、不上過期標示）；PDF 由 viewers.js 自行 fetch 成 ArrayBuffer 以 data 交給 pdf.js（為取得錯誤 code），disableRange／disableStream 因此無作用但保留 — spec 把「無法解析」與讀取失敗分開描述；整檔讀取本來就是 D7 的意圖 — 若錯：壞 PDF 不顯示過期標示。
- Task 4.5: minor (deferred): 固定縮放倍率下 devicePixelRatio 改變不會立即重畫（離開再進入可視範圍才重畫）。
- REVIEW BACKLOG: 4.5（9d27b6a..0932a11）
- Task 4.6: dispatched（BASE 0932a11，opus）
- Task 4.6: implementer DONE（337c321 files-check 前置等待修正（腳本時序 bug，斷言未改）、96b5b93 產品、b6a8c37 visual-check CL1 走訪）；files-check 全段 ok 511／FAIL 0；visual-check、六支全綠；probe46 50/0（PDF 頁碼保留、不堆積 2006 ms 間隔、舊回應丟棄、根目錄恢復）。
- Ruling R27: 讀取中切走就作廢（大 PDF 切回重新下載）；過期改色只套 Markdown 內文／標題與純文字，連結、PDF、HTML 不改色 — 世代分開的必然結果；spec 只要求「以相同的過期標示呈現」（左緣條＋字樣＋原因） — 若錯：大檔切換多一次下載；過期時連結仍是強調色。
- Task 4.6: minor (deferred): 中繼資料失敗與同版本讀取成功重疊時過期標示可能短暫閃一下。
- REVIEW BACKLOG: 4.6（0932a11..b6a8c37）
- Task 4.6 設計審核：通過（過期呈現與 Live Output 一致：--warn 左緣條＋「過期」＋原因，內文 --text-dim）
- Codex 補審開始（03:40）：前端 4.2–4.6 合併（worktree @b6a8c37, base 40fa764）
- 前端 4.2–4.6 合併審查：needs-attention 1 medium（htmlViewer 設 src 即回 ok，raw 失敗被記為成功、同版本不重試）
- Ruling R28: htmlViewer 先 fetch 原始內容端點確認 200 再設 iframe src（接受多一次下載；不用 srcdoc 以保留相對資源載入）；失敗回 code、保留舊內容、同版本重試 — spec 讀取失敗處理＋相對樣式表須載入 — 若錯：HTML 檔下載兩次。
- 前端 fix round 1 dispatched（resume 4.6 implementer）
- 腳本審查（b8aae60..b6a8c37）：approve。
- Task 3.5: complete (commits e009997..3a230d9＋4.x 腳本修改, 2 fix rounds)
- 前端 fix round 1/5（72b63cc）re-review：原 finding 首次失敗路徑已解決；殘餘 [medium] 預先 fetch 成功後 iframe 自身請求失敗仍會記為成功。
- Parked — Ruling R29: 接受 R28 兩次請求的殘餘競態（fetch 成功後毫秒內 iframe 請求失敗才會發生；下次檔案改變或重開分頁即恢復）；徹底解法需 srcdoc＋注入 `<base>`，等於改寫使用者 HTML，違反 D8 — 若錯：極少數情況下 HTML 分頁顯示錯誤頁直到檔案改變。
- Task 4.2: complete (commits 40fa764..73abb20, 合併審查)
- Task 4.3: complete (commits 3a230d9..08ccb6a, 合併審查)
- Task 4.4: complete (commits 08ccb6a..9d27b6a＋72b63cc, 合併審查 1 fix round、1 parked)
- Task 4.5: complete (commits 9d27b6a..0932a11, 合併審查)
- Task 4.6: complete (commits 0932a11..b6a8c37, 合併審查)
- Task 5.1: dispatched（sonnet）；Task 5.3: dispatched（sonnet，並行，只改文件）BASE 91be3e7
- Task 5.3: implementer DONE（03cc094）；markdownlint 92/0、openspec validate 17 passed；fresh subagent 事實核對派出
- Task 5.3: complete（03cc094；fresh subagent 事實核對約 30 條全部相符；diff 審查併入 5.4 整支分支 Codex review）
- Task 5.1: complete（驗證，無 commit）— files-check、visual-check（含完整文字對比）、六支全 PASS 0 FAIL；6 張 viewport 截圖在 scratchpad/final-5.1/，scrollWidth==clientWidth；待使用者目視（5.5 前）。
- Task 5.2: dispatched（opus）
- Task 5.2: complete（9bbf27f）— headless 測試 server（/tmp 專屬 socket、HOME 隔離）；(a)–(d) 通過；(e) passwd-link raw／meta／render 皆 403 path_outside_root、root:x:0:0 出現 0 次。發現：Windows 經 \wsl.localhost 看 Linux 符號連結是無法跟隨的 reparse point，根內連結也被 R11 拒絕、列表顯示為 file（功能限制，fail-closed）→ 已寫 memory；README 補說明列入最終修正波。
- Task 5.4 gate：fmt OK、clippy 0、workspace 715 passed／0 failed／11 ignored（新增 1 個是 files.rs doctest 範例標 ignore）、ui_preview 29、markdownlint 92/0、openspec 17 passed（log: gate-5.4.log）
- 整支分支審查（base e92ab05）：needs-attention 3 medium — (F1) 同 pane cwd 改變時檔案樹停在舊根；(F2) 純文字 2 MiB 上限只看 meta.size、raw 可更大（Codex 已以記憶體測試重現）；(F3) R29 升級為合併前處理。lows 分級：3.2 Allow、4.1 對齊與高度、4.3 高度跳動與 ARIA、4.4 行寬、4.5 DPR、4.6 閃爍 → 可延後；3.4 可結案（files-check.md 舊清理說明需同步）。
- Ruling R30: F3 不改 srcdoc（spec「以 iframe 載入原始內容端點」），改為新 iframe 先隱藏以 src 載入、以 Resource Timing（initiatorType iframe、responseStatus）確認 200 後才替換舊 iframe 並回 ok；確認失敗保留舊內容並回失敗 code；src 加唯一 query 參數以對應 timing entry。須實測 Chrome 行為，不可行則停下回報改回 R29＋交使用者決定 — 維持 spec 字面同時消除競態 — 若錯：多一個 query 參數與一段 timing 邏輯。
- 最終修正波 dispatched（單一 agent，opus）：F1、F2、F3（R30）＋低成本 lows：README 補 WSL 符號連結限制（5.2 發現）、files-check.md 清理說明同步（3.4）、Markdown 內文最大行寬約 80ch（4.4 設計 minor）。
- 最終修正波（3e6bc1a、a063a66、1d356c7、2304041）：F1–F3 RED→GREEN；R30 實測可行（responseStatus 反映 200／404／400／500／503；合成回應與連線失敗為 0；採 entry 或 load 先到者判斷）；files-check 26 段 ok 532、visual-check ok 1648、六支全綠；全 gate 綠（ui_preview 31）。scoped re-review：approve。
- Task 5.4: complete — 整支分支 Codex review：1 輪 3 medium → 修正波 → approve；lows 依 Codex 分級延後（見上）。
