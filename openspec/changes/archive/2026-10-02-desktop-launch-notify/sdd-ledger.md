# SDD ledger：desktop-launch-notify

- plan：`openspec/changes/desktop-launch-notify/tasks.md`；spec：同目錄 `specs/`、`design.md`
- 分支：`feat/desktop-launch-notify`，起點 `eeaae29`（propose commit）
- 工作區（git-ignored）：`.superpowers/sdd/tasks-desktop-launch-notify/`（briefs、reports、review packages、`mkbrief.sh`）

## 使用者決定（2026-10-02）

- 需求：一個捷徑就開好、桌面通知；關視窗即整個結束；方案 A（啟動器＋網頁通知，不包 Tauri）。
- 通知四類都做、可開關，預設只開 agent blocked 與 task failed。
- 其餘細節由 Claude 決定、事後彙整（下列 Ruling）。

## 規劃階段裁決（使用者授權 Claude 決定）

- Ruling: 視窗在前景（頁面可見且有焦點）時不發通知 — 畫面上本來就看得到，避免重複打擾 — 若錯：使用者在看 Cockpit 時漏掉提示，可改為一律通知。
- Ruling: 一次更新超過 3 件事件合併成一則「Cockpit：N 件事需要注意」 — 避免洗版 — 若錯：多事件時細節要點進畫面看。
- Ruling: 頁面載入的第一份狀態只當基準、重連後與斷線前最後一份比對 — 開窗不跳舊事件、斷線期間的變化仍補報 — 若錯：重連後可能一次跳數則。
- Ruling: task 類通知標題與面板標籤用 `task failed`／`task completed` — `cockpit-dashboard` 既有規定「頁面不出現『完成』」 — 若錯：只是用字。
- Ruling: 保留 task failed／completed 通知，即使目前只由人工標記產生（驗證者 M1） — 使用者明確要求四類都做；spec 寫明現況 — 若錯：兩個開關目前幾乎不會觸發。
- Ruling: 閒置 10 秒、從未連線 60 秒；閒置計時中 `GET /`／`GET /api/state` 延長期限 — 涵蓋重新整理與「啟動器偵測到在跑、正要開窗」的競態 — 若錯：極慢的冷啟動仍可能撞上，可再延長。
- Ruling: 瀏覽器只找標準安裝位置＋`COCKPIT_BROWSER` 覆寫，不查登錄檔 — KISS — 若錯：非標準安裝要設環境變數。
- Ruling: 訊息框直接宣告 `user32` 的 `MessageBoxW`，不加 `windows-sys`；`COCKPIT_LAUNCH_DIALOG_FILE` 作為測試入口 — 只用一個函式；訊息框會阻塞自動驗收 — 若錯：多一個測試用環境變數。
- Ruling: log 導向檔案時以 `is_terminal` 關閉 ANSI 色碼（改 `main.rs`），不在啟動器設 `NO_COLOR` — 對所有導向情境都正確 — 若錯：無。
- Ruling: 安裝到 `%LOCALAPPDATA%\ai-cockpit\bin\`、零設定時捷徑工作目錄為 `%LOCALAPPDATA%\ai-cockpit\`、捷徑用預設圖示 — 避免鎖住 `target/`、避免工作目錄落在系統資料夾 — 若錯：位置可用參數改。
- Ruling: 通知點擊經新的 `cockpitActions.selectPane` 走點 pane 列的同一路徑；改綁模式或 pane 已消失時只帶到前景 — 與 pane 列的既有規則一致 — 若錯：無。

## 前置裁決（SDD 流程）

- Ruling: ledger 放在 change 目錄、per-task 不另派 reviewer（控制端核對 red→green），diff 審查依 tasks.md 2.6／3.6／5.3 由 Opus 5.5 分階段跑（本專案視同 Codex）；
  brief 用工作區 `mkbrief.sh` — 沿用 change 7、8 的作法 — 若錯：單一 task 的問題要到階段審查才發現。
- Ruling: 2.5 不要求在舊版跑紅（舊版不認得旗標、立即失敗，紅無證明力），行為由 2.1 的 Rust 單元測試 red→green 把關 — 若錯：腳本自身錯誤要到真跑才暴露。
- Ruling: 4.1 真機驗收由控制端執行（會在使用者桌面開出瀏覽器視窗，屬產品本身的用途） — 若錯：使用者當時正在用電腦會看到視窗閃現。

## 前置衝突掃描

| 對象 | 產出 → 使用 | 結果 |
|---|---|---|
| 2.1 ↔ 2.3 | `--exit-when-idle` 旗標 → 啟動器傳給後端 | 依序；2.3 用 2.1 的旗標名 |
| 2.1 ↔ 2.5 | 閒置規則 → 腳本驗證 | 依序 |
| 2.1 ↔ 3.1 | `AppState` 新欄位改到 `ui_preview.rs` → 3.1 再改 `ui_preview.rs` | 同檔依序，無衝突 |
| 2.2 ↔ 2.3 | `launch.rs` 純函式 → bin 串接 | 依序 |
| 2.3 ↔ 2.4 | 兩個執行檔名 → 安裝腳本複製 | 依序 |
| 3.1 ↔ 3.2 ↔ 3.3 | `COCKPIT_PREVIEW_TRANSITIONS` → 腳本 → 前端 | 腳本先於前端（跑出紅），依 tasks 順序 |
| 3.3 ↔ 3.4 | 面板 → 截圖 | 依序 |
| 2.x、3.x ↔ 4.1 | 安裝腳本、前端 → 真機 | 4.1 在全部實作後 |

## 進度

- Ruling: 1.1 基線沿用 change 10 收尾 gate 的實測數字（feat 分支相對 main 只多規劃文件，程式碼相同）— 省一輪約 25 分鐘的全套腳本 — 若錯：基線與現況不符，2.x 的全綠比對會在第一次跑時暴露。
- Task 1.1: 基線（程式碼同 main 1767e39）— workspace 1152 passed／0 failed／13 ignored；ui_preview 48；12 支既有腳本全 PASS（reconnect-check 首跑 Chrome 未出現 page target、重跑 PASS）；deid-check 0 命中。
- Task 1.1: complete
- Task 2.1: complete（1964d23；red＝編譯失敗＋骨架 10 個 idle_exit／config／app 測試失敗→green；workspace 1171／0／13、ui_preview 48、12 支既有腳本一次全 PASS；實機 60 秒結束碼 0、log 無 ANSI）
  - Ruling: `AppState` 只加一個 `activity: ClientActivity` 欄位包住連線數與最近活動兩個 watch（design D6 寫成兩個欄位）— 少改既有 struct literal、語意相同 — 若錯：只是結構差異。
  - Ruling: `HEAD /`、`HEAD /api/state` 不更新活動時間 — spec「其他 HTTP 請求不影響計時」只列 GET — 若錯：HEAD 探測不能延長期限（啟動器用 GET，無影響）。
  - Ruling: 「降為 0」的時刻取監看端收到 `changed()` 的時刻 — 只會讓期限略晚、不會提早 — 若錯：閒置結束晚幾毫秒。
  - Task 2.1: minor (deferred): `tests/app.rs` 的 `components_with_slow_driver` 自組 `Components` 時 `activity` 與 router 內不共用頻道；現有測試不受影響。
- Ruling: 2.2（launch.rs 純函式）與 2.3（cockpit-launch bin）合併成一次派工 — 純函式與串接緊密相依，分開派只會重建脈絡 — 若錯：一次 diff 較大，階段審查照常涵蓋。
- Task 2.2: complete（ecfafc9；launch.rs 27 個單元測試先紅後綠）
- Task 2.3: complete（ca84b9c；workspace 1198／0／13；真機實測 spec「啟動器」S1–S5 全符合；S6 找不到瀏覽器因 Windows 依 ProgramW6432 重算 ProgramFiles、藏不住已安裝的 Chrome，由單元測試涵蓋；`cargo run -p cockpit` 仍選到 cockpit）
  - Ruling: 架構守門測試 `cockpit_crate_never_spawns_a_command_directly`（`cockpit/src` 不得出現 `Command::new`）只豁免 `src/bin/cockpit-launch.rs` 一個檔案並斷言其存在 — 啟動器的職責就是啟動子程序（design D1 規定放在 src/bin），git 等子程序仍須經 cockpit-git — 若錯：豁免面擴大一個檔案。
  - Ruling: 15 秒未就緒時啟動器 kill 並 wait 自己啟動的後端 — 避免卡在監聽前、閒置計時未啟動的孤兒程序 — 若錯：極慢啟動被提早中止，訊息框會說明。
  - Ruling: 連得上但讀取逾時、連線被重設或回應不完整一律視為埠被占用 — 保守、不誤啟動第二個後端 — 若錯：剛好在後端啟動中的瞬間偵測會報占用。
  - Ruling: `COCKPIT_LAUNCH_DIALOG_FILE` 寫檔失敗時略過、不改跳阻塞的訊息框 — 測試入口不應卡住自動驗收 — 若錯：測試時看不到訊息。
  - Task 2.3: 實作者嘗試 S6 時意外開了一個 Chrome `--app` 視窗（以暫存空目錄為設定檔的獨立執行個體，與使用者的 Chrome 無關），已以 CloseMainWindow 正常關閉、確認無殘留。
- Ruling: 2.4（安裝腳本）與 2.5（idle-exit-check.js）交同一個 subagent 依序做、分開 commit — 兩者小且不共用檔案 — 若錯：無。
- Task 2.4: complete（5406878；pwsh 7 與 Windows PowerShell 5.1 皆以暫存目錄實測：有設定檔、零設定兩種捷徑內容符合 spec；執行中拒絕且檔案 SHA256 不變；腳本存 UTF-8 BOM 供 5.1 解析中文）
- Task 2.5: complete（cb7ab64；idle-exit-check RESULT: PASS，關閉後 10.0 秒開始關閉、結束碼 0；b／c／d 持續執行）
  - Ruling: idle-exit-check 以 cockpit 日誌「開始正常關閉」行首的時間戳判定開始關閉的時刻 — 關閉只需數毫秒，輪詢程序狀態抓不到 — 若錯：日誌文字或時間格式改動時腳本需同步（已寫入腳本與 .md）。
- Task 2.6: 後端與啟動器階段審查（Opus 5.5，eeaae29..bad7143）— approve，Critical 0／Important 0／Minor 9（報告：工作區 review-2.6-report.md）；變異測試確認閒置規則被鎖住，啟動器 S1–S5 與「同時點兩次」6 輪實測正確。
  - Ruling: 以下 Minor 併入第 3 節後的修正波一起修，不另開修正輪 — 都是小改、不影響第 3 節 — 若錯：晚一點修。
    - 2.6 M2：安裝腳本的執行中檢查在建置之前、32 位元 PS 5.1 讀不到 64 位元程序 Path 時整個略過 → 改在複製前檢查、取不到 Path 時以檔案能否獨占開啟判斷。
    - 2.6 M8：launch.rs 三個測試寫死 Windows 路徑，非 Windows 會紅 → 以 cfg 限定或改用平台無關路徑。
    - 2.6：啟動器啟動後端時後端繼承呼叫端 stdout handle（擷取輸出的呼叫端會等到後端結束）→ 確認 stdout／stderr 只指向 log、不繼承其他 handle。
    - 2.6：安裝腳本寫死 `target\release`、忽略 `CARGO_TARGET_DIR`；PS 5.1 以 `2>&1` 執行時 cargo 的 stderr 被當錯誤中止 → 修正。
  - Task 2.6: minor (deferred): WScript.Shell 建捷徑時路徑含系統字碼頁以外字元會失敗；剛好在閒置期限那幾毫秒點捷徑的偵測誤判；`cockpit.log` 截斷留下 NUL 區段（design 已列）；缺「升級成功才加一」與 `run()` 接上旗標的 Rust 測試（JS 腳本有涵蓋）。
- Task 2.6: complete
- Task 3.1: complete（fed0310；ui_preview 60 測試、12 支既有腳本全 PASS；手動確認轉換生效且輪替停用；額外於啟動時驗證轉換目標存在、打錯即點名環境變數失敗——接受）
- Task 3.2: 腳本 0460d4d（156 條新行為斷言在未實作前端的 1c9ced5 上全紅、40 條前置斷言全綠；紅證據 工作區 task-3.2-red.txt；以 repo 外替身前端驗過腳本本身）。待 3.3 後轉綠才算完成。
  - Ruling: 採用 notify-check 定的 DOM 約定（面板 `id="notify-panel"` 在 body 下、開關帶 `data-notify-kind` 依 blocked／done／failed／completed 排列、按鈕文字「允許通知」、權限說明含「已允許」「網站設定」「不支援」、`toNotifications` 項目含 title／body／tag；完整見 notify-check.md「DOM 約定」）— spec／design 未指定，3.3 需要固定的對齊點 — 若錯：改 DOM 時腳本同步。
  - Ruling: 合併通知只驗「前 3 件標題出現且以『…』結尾」，不規定排列與分隔 — spec 未定義「前 3 件」順序 — 若錯：順序不穩定時使用者看到的 3 件不固定。
  - Ruling: 腳本兩條斷言來自 design（notify.js 先於 render.js 載入；頁面 hidden 時即使有焦點也發通知）— design D7 明定 — 若錯：與 spec 精神一致，無衝突。
  - Ruling: spec 範例的 `wJ:p2`（fixture 中為 exited）改用 `wJ:p1`（綁定 Backend×2 與 Undeclared，順便驗去重）與 `wJ:p3` — 依 fixture 實況 — 若錯：無。
- Task 3.3: complete（9e3a9e5；notify-check 新行為 159/159、前置 42/42；12 支既有腳本全 PASS——visual-check 首跑 [G1/glow] 抓到面板 box-shadow 違反「只有 running 節點發光」，已拿掉；cockpit 395／0、ui_preview 60；Rust 路由測試先紅後綠）
  - Ruling: `render.js` 新增唯讀 `window.cockpitLatestState()` 供 `cockpitActions.selectPane` 判斷 pane 是否仍在最新投影 — design 未列，但比讓 actions.js 自存一份狀態單純 — 若錯：多一個全域唯讀函式。
  - Ruling: 點面板外關閉時於 pointerup 判斷焦點：落在 body 才交回鈴鐺，點到可聚焦元素（如 pane 列）由它接手 — 符合 spec「若焦點原本在面板內才回鈴鐺」的精神 — 若錯：無。
  - Task 3.3: minor (deferred): 鈴鐺沒有 `aria-expanded`（交 3.6 前端審查判定是否必修）。
  - Task 3.3: 觀察到既有偶發 `tests/app.rs::abort_await_is_bounded`（高負載下超過 3 秒上限），重跑通過。
- Task 3.2: complete（0460d4d；3.3 後全綠）
- Task 3.4: complete（b7bb2d7；notify-panel-1536／700 兩張，控制端逐張看圖：無真名、面板清楚；notify-check 仍 159／42 全 PASS；deid-check 0 命中）
- Task 3.5: complete（控制端以 frontend-design 審核模式評 3.4 截圖）
  - Ruling: 採納——勾選框加 `accent-color: var(--accent)`（目前是瀏覽器預設藍，不屬於色票）；鈴鐺加 `aria-expanded` 並在面板開啟時呈現按下狀態（同時解掉 3.3 延後的 aria-expanded）— 視覺一致與可及性 — 若錯：僅外觀差異。
  - Ruling: 不採納——700 寬時面板蓋住頁籤列（浮層正常行為）；英文標籤加中文說明的混排維持（標籤需與 spec 一致、說明補足語意）。
  - 上述採納項與 2.6 延後的 5 個小項，併入 3.6 審查後的同一修正波。
- Task 3.6: 前端階段審查（Opus 5.5，a050f5e..4ee620e）— approve，Critical 0／Important 0／Minor 10（報告：工作區 review-3.6-report.md）；15 個變異中 14 個被 notify-check 抓到，唯一存活的是「鈴鐺遞增 latestOp」（M1，測試缺口）。
  - Ruling: 修正波涵蓋 3.5 採納兩項、2.6 延後五項、3.6 的 M1–M5 與 M8–M10，一次派工 — 都是小改、集中處理少重建脈絡 — 若錯：一次 diff 較雜，由整支分支審查把關。
  - Task 3.6: minor (deferred): M6 點 HTML 預覽 iframe 內部不會關閉面板；M7 多個 Cockpit 頁面同時開時設定不同步、通知會重複發。
- Task 3.6: complete（修正波另記）
- 修正波（003e34d..7113ad5）：3.5 兩項、2.6 五項、3.6 M1–M5／M8–M10 全修；notify-check 新行為 180/180、前置 66/66（修正前 17 條 FAIL）；idle-exit-check PASS；12 支既有腳本全 PASS（visual-check CL1 擴充「面板開啟」走訪狀態 7113ad5）；workspace 1198／0／13、ui_preview 61；安裝腳本在 pwsh 7、PS 5.1 64／32 位元實測拒絕與安裝；啟動器 handle 繼承修正後擷取輸出的呼叫端 2 秒返回（原 61 秒）。實作者依不貳過規則新增兩則 memory（Windows 子程序繼承呼叫端 std handle、PowerShell 5.1 原生 stderr 與跨位元 Path），控制端檢查為機制層、無敏感值。
  - Ruling: 修正波不另做縮小範圍複審，由 5.3 整支分支審查一併涵蓋 — 全為 Minor、每項有紅綠證據與變異驗證 — 若錯：修正本身的新破壞晚一步發現。
  - Ruling: M4 選「面板跟著重新定位」、M9 選「程式管理焦點、不搬 DOM」（已補進 design D8）— 避免讀到一半被關、避免影響 #app 結構 — 若錯：互動細節差異。
- Task 4.1: complete（控制端真機：安裝到真實位置、捷徑啟動 1.5 秒就緒、單一後端、無主控台、log 無色碼；連點兩次不重複啟動；只關標題為「AI Agent Cockpit」的 3 個視窗後約 11 秒後端結束、使用者其他 Chrome 不受影響；紀錄 docs/research/2026-10-02/desktop-launch-live.md）
  - Task 4.1: 未真機驗證（需使用者操作）：允許通知的授權詢問、真實 Windows 通知呈現與點擊帶回、最小化後是否被瀏覽器凍結。安裝目錄與桌面捷徑保留供使用者使用。
- Ruling: 5.1（文件）與 5.2（全 gate）交同一個 subagent 依序做 — gate 要在文件改完後跑 — 若錯：無。
- Task 5.2: 全 gate（HEAD 963712c）— fmt／clippy 0 error；workspace 1198 passed／0 failed／13 ignored（abort_await_is_bounded 未偶發）；ui_preview 61 passed；markdownlint 161 files 0 issues；openspec validate --all 20 passed; 腳本：reconnect／whatever／actions／channel-backoff／factory-floor／live-output／visual／files／git／progress／ui-fixes／output-color／idle-exit／notify 全 PASS（乾淨序列重跑一輪，無偶發）；deid-check 檔案／歷史 0 命中
- Task 5.1: complete（963712c；ADR-0005 補充段、cockpit/README.md；CONTEXT.md 為名詞詞彙表、啟動器與通知屬行為，不動）
  - Task 5.2: 第一輪批次腳本因背景執行逾時後重疊執行造成假 FAIL，實作者等程序結束後乾淨序列重跑全 PASS；只清了自己的孤兒程序。
- Task 5.3: 整支分支審查（Opus 5.5，1767e39..4591300）— needs-fix，Critical 0／Important 3／Minor 7（報告：工作區 review-5.3-report.md）；修正波未引入新破壞。
  - Ruling: I1（啟動器模式下視窗關閉後 agent 回報與寫入請求靜默遺失）維持「關視窗即結束」，於 spec「閒置自動結束」、design Risks、README 寫明，列為交使用者重新評估的第一項 — 使用者授權 Claude 決定；改常駐需系統匣（使用者已選不做），改 agent 回報屬另一 capability — 若錯：使用者關窗期間的進度回報會遺失，需改設計。
  - Ruling: I2（多視窗時通知設定不同步、互相覆寫）修——notify.js 監聽 `storage` 事件重讀設定並同步開關，notify-check 補兩分頁斷言 — 啟動器的設計會開出多個同源視窗，3.6 M7 因此升級 — 若錯：無。
  - Ruling: I3（通知未在真機驗證）不假裝完成：新增未勾選的 task 4.2「使用者真機確認通知」，archive 時保留為未完成並寫進交接手冊第一項 — 需要使用者操作、控制端無法代按允許通知 — 若錯：通知在真機若有問題要等使用者回報才發現。
  - Ruling: 同批修 m2（README 指向 change 目錄的 spec 路徑，archive 後失效）、m3（README「約 10 秒內結束」改為「10 秒後開始關閉」）、m7（desktop-launch Purpose 不限 Windows，已改）；m4（handover）於收尾重寫；m1（任何網頁可連 /ws 使後端不閒置）、m5（log 覆寫）、m6（啟動器與安裝腳本缺 repo 內回歸驗收）延後。
- 最終修正（151abe5..d8876af）：I2 notify.js 監聽 storage 事件跨視窗同步（T 段 5 條先紅後綠）；README m2／m3 與 I1 後果說明；notify-check 190/190、前置 62/62；12 支既有腳本全 PASS；cockpit 測試無失敗；markdownlint 0；deid 0。
- Task 5.3: complete（needs-fix 項已處理：I1 依裁決文件化、I2 修、I3 轉為使用者 task 4.2；m2／m3／m7 修、m4 收尾重寫）
