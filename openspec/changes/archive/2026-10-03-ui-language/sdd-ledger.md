# SDD ledger：ui-language

- plan：`openspec/changes/ui-language/tasks.md`；spec：同目錄 `specs/ui-language/spec.md`、`design.md`
- 分支：`ui-language`，起點 `cbf08f4`（propose commit）
- 工作區（git-ignored）：`.superpowers/sdd/tasks-ui-language/`（briefs、reports、review packages、`mkbrief.sh`）

## 使用者決定（2026-10-03）

- 預設語言跟宣傳頁同一套規則；頂列獨立切換按鈕；後端訊息改代碼由前端翻譯；啟動器跟隨 Windows 顯示語言。
- 使用者以 `/opsx:apply` 表示同意對話中提出的設計並開工（提案未另外逐份審閱）。

## 前置裁決（SDD 流程）

- Ruling: ledger 放在 change 目錄、brief 用工作區 `mkbrief.sh`；per-task 不另派 reviewer，控制端核對 red→green 與驗收證據；
  diff 審查改為三段 Opus 審查（本專案視同 Codex）：第 1、2 節完成後（前端）、第 3、4 節完成後（後端與啟動器）、5.4 整支 —
  沿用 change 7、8、11 的作法，字串搬移的 diff 很大、逐 task 審查成本高 — 若錯：單一 task 的問題要到階段審查才發現。
- Ruling: 實作者用 Sonnet；階段審查與整支審查用 Opus — 字串搬移是機械性工作、審查需要跨模組判斷 — 若錯：實作回合數增加。

## 衝突掃描

| 項目 | 產出 → 消費 | 發現與裁決 |
|---|---|---|
| 1.1 → 2.1–2.4、3.3 | 1.1 建 `i18n.js`、`t()`、鍵命名規則；後續 task 往字典加鍵 | 無衝突；循序執行，避免同時改字典 |
| 1.2 ↔ 2.1 | 兩者都改 `render.js`、`actions.js` | 無衝突；1.2 先加切換按鈕（字串直接進字典），2.1 再搬其餘字串 |
| 2.1 ↔ 2.4（design D5） | `http.rs` 的「`render.js` 不得含『完成』」斷言在 2.1 搬走字串後會變成恆真 | **Ruling: D5 的字典守門在 2.1 先建立（涵蓋 render 的 `done` 相關鍵），2.4 擴充到 notify 鍵並移除舊斷言 — 避免守門在 2.1–2.4 之間失效 — 若錯：多一個暫時重複的斷言** |
| 2.x 驗收 ② ↔ 3.3 | 2.x 要求英文畫面沒有繁中字典字串，但連線原因、project 警告、錯誤 banner 原因來自後端，3.3 前仍是繁中 | **Ruling: 2.x 的 ② 段排除後端原文欄位（runtime 卡 reason／protocol warning、project 警告、錯誤 banner 的原因部分），由 3.3 補上這幾段 — 後端代碼在 3.x 才有 — 若錯：無** |
| ② 的比對方式 | 字典值與範例資料可能撞字（短詞如「檔案」） | **Ruling: ② 以「整段文字或屬性值等於某個繁中字典值（去掉佔位符後的固定片段）」為準，並對長度 ≥ 4 個中文字的固定片段做包含比對；範例資料的節點不列入（實作者在 1.1 定案並寫進腳本註解） — 避免誤判使用者資料 — 若錯：漏抓極短的殘留字串，由截圖審核補** |
| 3.1、3.2 → 3.3 | 3.3 依賴 3.1 的錯誤代碼與 3.2 的投影欄位 | 無衝突；循序 |
| 3.2 ↔ `ui_preview` | 投影新增欄位；`cockpit/tests/fixtures/projected-state.json` 與 `ui_preview` 範例資料可能缺欄 | **Ruling: 新欄位以 `#[serde(default)]` 相容舊 fixture；`ui_preview` 範例資料補上代碼，讓英文驗收看得到翻譯結果 — 若錯：無** |
| 4.1 | 只動啟動器 | 與其他 task 無共用檔案 |
| 5.3 ↔ 3.x | 文件列後端代碼清單 | 5.3 以 3.1、3.2 定案的清單為準 |
| 各 task 自洽 | 2.2–2.4 寫「同上」 | 指 2.1 的要求（全部介面字串改用 `t()`、拼接改具名佔位符範本、英文字典補齊），dispatch 時明寫 |

## 進度

- Task 1.1: dispatched（Sonnet，BASE f14430a）
- Task 1.1: complete（commits f14430a..66f7b00；控制端核對 RED：路由與載入順序 2 測試 FAIL→GREEN 18 passed；i18n-check ①③ 73＋21 PASS、變異後 FAIL(10)；visual-check PASS）
  - Ruling: 主 spec `cockpit-dashboard`「路由與內嵌資源」需列入 `/app/i18n.js` — 補一份 MODIFIED delta（完整複製原需求與 scenario），併入 5.3 文件任務 — 不補則 archive 後主 spec 的白名單與程式不一致。
  - Ruling: `localStorage` 不可用時切換按鈕停用並以 title 說明（1.2 實作） — reload 後選擇會遺失，按了沒反應比停用更糟 — 若錯：極少數環境無法切換語言。
  - Ruling: 接受實作者額外加的 `apply`、`dictionaries`、`data-i18n-params` — 驗收腳本與狀態列範本需要 — 若錯：多兩個公開成員。
  - 記錄：改 `cockpit/assets/` 後須先重建 `ui_preview` 才跑腳本（專案 memory 已記），之後每個 dispatch 明寫。
- Task 1.2: dispatched（Sonnet，BASE 66f7b00）
- Task 1.2: complete（commits cada83e..d65245c；控制端核對 ④ 按鈕不存在時 FAIL、拿掉 storage 同步時 FAIL(2)→PASS；i18n-check ①③④ 106＋29、notify／ui-fixes／visual PASS）
  - Ruling: 接受實作者改 `visual-check.js`（CL1 死規則檢查需要 `.lang-toggle:disabled` 的取樣狀態） — 樣式規則要有覆蓋 — 若錯：多一個取樣步驟。
  - Ruling: 接受「載入時可存、之後寫入失敗則按下不動作」 — 極少見（配額滿），停用判斷只在載入時做 — 若錯：該情況下按鈕無回饋。
  - Ruling（給 2.x）: 英文複數用 `tn(key, n, params)` 取 `key.one`／`key.other`，繁中兩個鍵同文以保持鍵一致 — KISS、不引入 Intl.PluralRules 規則表 — 若錯：只支援英文單複數兩型（本 change 只有中英）。
- Task 2.1: dispatched（Sonnet，BASE d65245c）
- Task 2.1: complete（commits d6a4329..4d75ad1；控制端核對 RED：D5 測試插入違規字 FAIL、② 硬寫兩串 FAIL(28)→PASS；i18n-check ①–④ 220＋72、8 支既有腳本 PASS）
  - 記錄：task 1.2 造成 4 支既有腳本的自製測試頁（未載入 `i18n.js`）在 `renderLangToggle` 拋錯，1.2 當時未跑到這些腳本；2.1 已在 harness 補載入。之後每個前端 task 的驗收清單含 actions／factory-floor／progress／whatever-check。
  - Ruling: `PENDING_MODULE_SEL` 暫時排除 #files-panel、#changes-panel、#review、#notify-panel，2.2–2.4 各自拿掉對應選取器 — 避免未搬模組的硬寫字串撞到已搬鍵 — 若錯：最後仍有排除殘留，5.1 檢查清單必須為空。
  - Ruling: D5 守門目前 0 個受保護鍵；2.4 的通知鍵命名必須落在守門規則內（區段以 `done` 或 `agentDone` 開頭），並加斷言「受保護鍵數 > 0」 — 防止守門永遠恆真 — 若錯：無。
  - Task 2.1: minor (deferred): render.js 兩處、actions.js 一處區域變數遮蔽模組層級 `t`，目前未呼叫 `t()`，日後易誤用。
- Task 2.2: dispatched（Sonnet，BASE 4d75ad1）
- Task 2.2: complete（commits 3fc0440..3bff86f；控制端核對 RED：files／viewers 改回硬寫 FAIL(68)→PASS；i18n-check 349＋174、files／visual／actions／whatever／ui-fixes PASS；workspace 1212 passed）
  - Ruling: 「過期」在 files 另開 `files.toolbar.stale`、不共用 `output.stale` — 不同模組各自改字不連動 — 若錯：兩處英文可能不一致。
  - 記錄：fixture 沒有的邊界狀態以頁面內假 fetch 驗證前端文字，後端真實回應由既有 files-check 覆蓋。
- Task 2.3: dispatched（Sonnet，BASE 3bff86f）
- 記錄（2026-10-03，使用者交代）：本 change 完成並併回 main 後，傳訊息通知另一個 session「打包發佈與版本管理」（cross-session，名稱即位址），內容含 main 的 commit、改動摘要（啟動器語言、README 變更）。已先回覆它 Tauri 結論（不包 Tauri）與本分支會動到的檔案。
- Task 2.3: complete（commits 0dfd7a5..5844006；控制端核對 RED：git.graph.loadMore 改回硬寫 FAIL(49)→PASS；i18n-check 654＋356、git-check（收尾衛生一次過）／files／visual／actions／whatever PASS）
  - Ruling: 與 files.js 語意完全相同的字串重用 `files.*` 鍵（error、loadingRoot、loading、notImplemented），其餘 88 個 `git.*` — 同義同字、避免兩處英文不一致 — 若錯：日後要分開時改鍵。
  - Ruling: 接受 ② 的排除從整個 `.files-runtime` 縮小到兩個表頭欄位，並新增開發旗標 `--git-only` — 讓分支資訊（含「分離 HEAD」）受檢 — 若錯：無。
- Task 2.4: dispatched（Sonnet，BASE 5844006）
- Task 2.4: complete（commits 1428686..6917007；控制端核對 RED：守門鍵 0 個 FAIL、英文 done 說明含 finished FAIL、notify 硬寫 FAIL(5)→PASS；i18n-check 763＋393、notify／ui-fixes／visual／actions／whatever／git-check PASS）
  - Ruling: 接受 `git.copy.done` 改名 `git.copy.copied` 與「`notify.kind.done.desc`、`notify.title.done` 必須存在」斷言 — 原名被守門誤認、使「守門鍵 > 0」空轉 — 若錯：無。
  - Ruling: 既有 14 支驗收腳本靠本機時區 Asia/Taipei 才走繁中 — 5.2 一併讓每支腳本啟動 Chrome 時帶 `--lang=zh-TW`（第一順位語言即決定 zh），不依賴機器時區 — 換機器或 CI 會整批變英文而失敗 — 若錯：多改 14 支腳本的啟動參數。
- 階段審查 1（前端，tasks 1.1–2.4）：dispatched（Opus，範圍 cbf08f4..6917007）
- Task 3.1: dispatched（Sonnet，與階段審查 1 並行；只動後端 Rust）
- Task 3.1: complete（commits 1517cd7..6761bc9；控制端核對 RED：新測試編譯失敗與 agent_api 6 個斷言 FAIL→GREEN；三項 Rust gate 綠、ui_preview 61）
  - Ruling: `set_override` 非法 JSON 與 Live Output 路徑非 UTF-8 的兩個 400 不加代碼 — D4 未列、前端不會觸發 — 若錯：這兩個錯誤在英文介面顯示原文。
  - Ruling: `output_read_failed` 的 `detail` 可能是繁中或英文原文，3.3 範本直接代入 `{detail}` — 原因來自 HERDR／作業系統，不翻 — 若錯：英文句中夾原文。
  - Ruling: 新增 `READ_OUTPUT_FAILED_PREFIX` 共用常數，讓 `detail` 不帶繁中前綴 — 英文範本不夾中文 — 若錯：無。
- Task 3.2: dispatched（Sonnet，BASE 6761bc9，與階段審查 1 並行；不動前端資產）
- 階段審查 1（前端）：VERDICT approve（C0／I0／M5／N5；報告 `.superpowers/sdd/tasks-ui-language/stage1-review.md`）
  - Stage1: minor (deferred): M1 英文介面可能先閃一幀繁中靜態文字（i18n.js 在 body 結尾、`<html lang>` 寫死 zh-Hant）——5.1 截圖審核時一併處理。
  - Stage1: minor (deferred): M2 六處函式內 `var t` 遮蔽模組 `t()`（含 notify.js:85）。
  - Stage1: minor (deferred): M3 Factory Floor 未綁定等文字在英文被省略且沒有 title。
  - Stage1: minor (deferred): M4 D5 守門解析器略過非單行條目、只靠鍵名判斷。
  - Stage1: minor (deferred): M5 i18n-check 的 CJK 字元類漏 `「」、。`；硬寫掃描整行跳過含 `console.warn(` 的行。
  - Ruling: 3.3 把 ② 對 `.connection-details dd` 的排除縮小到 reason 那一列並納入檢查 — 審查者指出翻好的 reason 否則不受檢 — 若錯：無。
- Task 3.2: complete（commits 09a43a9..189756f；控制端核對 RED：缺型別編譯失敗→GREEN；workspace 1224 passed、ui_preview 62）
  - Ruling: 接受實作者在 D4 的 7 個代碼外另加 6 個（seed_snapshot_failed、lifecycle_subscribe_failed、status_subscribe_failed、status_resubscribe_failed、event_connection_error、event_connection_ended） — 避免常見原因全退成 raw — 若錯：字典多 6 個鍵。
  - Ruling（暫定，交階段審查 2 判斷）: 投影的 `*_msg` 以 `Message::classify(原文)` 反推，而非產生端直接帶出 — 實作者評估改 60 處簽名過重；原文與代碼同出 `Message` 目錄並有來回測試 — 若錯：原文含「」或目錄外同前綴字串時誤歸類（目前無來源），屆時改為產生端直接帶 `Message`。
  - 記錄：ui_preview 樣本即 `cockpit/tests/fixtures/projected-state.json`，已加 WSL 斷線卡與 project 警告的代碼；其 project 警告原文為手寫樣本、`warning_msgs` 手填。
- Task 3.3: dispatched（Sonnet，BASE 189756f）
- Task 4.1: dispatched（Sonnet，與 3.3 並行；只動啟動器檔案）
- Task 4.1: complete（commits 66e94c7..a7be0fd；控制端核對 RED：LaunchText 未定義編譯失敗→GREEN launch 36 passed；workspace 1233 passed）
  - Ruling: 接受測試專用覆寫 `COCKPIT_LAUNCH_LANG=en|zh`（bin 檔頭標明 test-only） — 讓端到端驗英文訊息框不必改系統語言 — 若錯：多一個環境變數入口。
  - Ruling: 設定模組回傳的錯誤內文維持繁中，只有「設定錯誤：」「命令列錯誤：」前綴跟語言 — spec 非目標不翻設定驗證錯誤 — 若錯：英文 Windows 使用者看到中英夾雜的設定錯誤。
  - 事故：subagent 第一版端到端腳本沒藏住已安裝的 Chrome，在使用者桌面開出連 `127.0.0.1:7893` 的視窗並起後端（PID 41284）。控制端確認後端執行檔在 subagent 的暫存建置目錄後關閉；Chrome（PID 77652，可能是使用者的主程序）不動，請使用者手動關視窗；暫存目錄 `D:\cockpit-target-task41` 刪除被權限擋下，請使用者手動刪。已寫專案 memory。
- Task 3.3: 實作完成（commits 4ede7f4..2472c25；控制端核對 RED：三檔還原後 ② FAIL 186/709→GREEN；i18n-check 844＋403、6 支既有腳本一次過；workspace 1233 passed）
  - Ruling: 繁中介面一律顯示後端原文、英文才套 `msg.*` 範本；spec「後端訊息代碼」改寫成同義 — 原文即繁中且可能帶範本沒有的細節（persist／internal／Unavailable），也保住 fixture 手寫警告當選取器 — 若錯：繁中範本只用於驗證，不影響畫面。
  - Task 3.3: fix round 1/5 dispatched — `forbidden_source`、`method_not_allowed`、`missing_pane_id`、無參數的 `pane_not_bound` 缺 `msg.*` 鍵，英文介面會顯示繁中原文。
- 記錄（跨 session）：打包分支 `release-packaging` 會在 repo 根加 `rust-toolchain.toml` 釘 Rust 1.97.1（使用者已同意）；起因是 runner 1.98.1 的新 lint `clippy::chunks_exact_to_as_chunks` 打在 `cockpit-herdr/src/probe.rs` 的 `.chunks_exact(2)`。本分支不必處理；兩分支先後併入 main 時注意 probe.rs 無衝突（對方不改 probe.rs）。
- Task 3.3: fix round 1/5（commit 1fba085；補 `msg.forbidden_source`／`method_not_allowed`／`missing_pane_id`、無參數 `pane_not_bound`；RED 舊 i18n.js FAIL 20/799→GREEN 879＋421；actions-check PASS）
  - Ruling: 多段原文共用一個代碼（forbidden_source 5、method_not_allowed 2、pane_not_bound 2）時，繁中範本為概括句、不逐字等於原文 — 繁中介面顯示原文，範本只供英文對應 — 若錯：無。
- Task 3.3: complete（commits 4ede7f4..1fba085；修正輪的覆審併入階段審查 2）
- 階段審查 2（後端＋3.3 前端代碼顯示＋啟動器，tasks 3.1–4.1）：dispatched（Opus，範圍 6917007..1fba085）
- 階段審查 2（後端＋代碼顯示＋啟動器）：VERDICT approve（C0／I0／M3／N5；報告 `.superpowers/sdd/tasks-ui-language/stage2-review.md`）。task 3.2 暫定裁決（classify 反推）判定可接受，轉為定案。
  - Stage2: minor: M1 `Message` 目錄測試非窮舉；M2 Rust 代碼與 `msg.*` 字典無自動對帳；M3 `RuntimeError`／`DomainState.warnings` 缺「必經 Message」提醒。
- Ruling: 5.1 一併修便宜且防漂移的 deferred minors——Stage1 M1（英文先閃繁中）、M2（`var t` 遮蔽）、M3（截斷文字補 title）、M5（CJK 字元類與 console.warn 行略過）；Stage2 M1、M2、M3。Stage1 M4（D5 守門解析器）交整支審查分級 — 都是低成本、可防日後靜默退化 — 若錯：多一輪小修。
- Task 5.1（前半：minors＋⑤ 截圖）: dispatched（Sonnet，BASE 見下一筆 commit）
- Task 5.1 前半: commits 281e8b3..c039563（Stage1 M1/M2/M3/M5、Stage2 M1/M2/M3 全修且各有 RED；英文截圖 15 張，拍前遮罩、逐張看圖；i18n-check 927＋512；workspace 1235 passed）
  - 記錄：一張截圖的 IDAT 壓縮資料偶然含 3 字元 WSL 使用者名稱子字串（deid-check W 類命中），無損重壓後消失並 amend 未推送 commit；推送前跑 `deid-check --history` 確認。
- 設計審核（控制端，frontend-design 標準，看 default／notify／changes／graph 的 1536、1100、700）：英文用字、長度、版面整體過關。
  - Ruling: 最近事件的 drift 說明在英文介面仍顯示繁中（WSL 未啟動是常見情境）→ 事件加 `detail_msg`（同 classify），前端 tMsg；spec「介面文字涵蓋範圍」「後端訊息代碼」改為事件 detail 若是 Cockpit 訊息須翻譯 — 與 runtime 卡同一件事一中一英不一致 — 若錯：多一個投影欄位。
  - Ruling: 變更清單檔案旁的 `history` 改為 `log` — 1536 寬左欄會擠掉檔名（`h...`） — 若錯：只是用字。
  - Ruling: 徽章 `Rebound` 維持（與 `Rebind`／`Undo rebind` 成套） — 若錯：只是用字。
- Task 5.1 後半＋5.2＋5.3: dispatched（Sonnet，單一 agent 依序）
- Task 5.1 後半＋5.2＋5.3: complete（commits 94a84d0..a8a855b；事件 `detail_msg`（只在 kind=drift 且 classify 非 raw 時填）、15 張截圖重拍並逐張看圖、deid 0 命中；15 個腳本檔加 `--lang=zh-TW`，強制 America/New_York 時仍繁中、對照組 en-US FAIL 44；14 支腳本一次全 PASS；README／cockpit/README／cockpit-dashboard MODIFIED delta（5 個 scenario 全在）；workspace 1237 passed）
  - Ruling（撤回）: 「`history` 改 `log`」撤回 — 那是 fixture 路徑的資料夾名稱（使用者資料），不是介面標籤；1536 寬時擠掉檔名屬既有版面問題，與本 change 無關 — 若錯：無。
  - Ruling: store.rs 約 25 條與 translate.rs 的 drift 原因補進 `Message` 目錄並補字典鍵（5.4 前完成） — spec 已規定 Cockpit 訊息的事件 detail 要翻譯，現況英文介面會夾繁中 — 若錯：多約 26 個目錄變體與字典鍵。
  - 記錄：宣傳頁 `s.note` 已由打包 session 在 `release-packaging`（3f9de0e）改寫；本分支不動 site/**。
  - 記錄：推送前必跑 `deid-check --history`（reflog 有被 amend 掉的舊 PNG，但未推送、不可達）。
- 補充修正（drift 原因目錄化）: dispatched（Sonnet，BASE a8a855b）
- 補充修正（drift 原因目錄化）: complete（commit d80494d；Message 13→18：drift_workspace_not_found／drift_tab_not_found／drift_pane_not_found／drift_runtime_not_registered（id）、event_payload_unparsable（event、detail）；RED：編譯期缺變體、對帳測試列 5 缺鍵、刪英文鍵時對帳與 ② 皆 FAIL；workspace 1238 passed）
  - Ruling: 接受 `drift_` 前綴 — 避免與 HTTP 既有 `pane_not_found`／`runtime_not_registered` 共用 `msg.*` 鍵而範本互相覆蓋 — 若錯：只是代碼名稱。
  - Ruling: `detail_msg` 維持只在 kind=drift 時填 — 實作者逐一查過 `describe_event`，其他種類 detail 為使用者名稱、HERDR 英文值或空字串 — 若錯：日後新種類帶 Cockpit 訊息時要放寬。
- Task 5.4 整支審查: dispatched（Opus，範圍 c2faee1..HEAD）
- Task 5.4 整支審查：VERDICT approve（C0／I0／M5／N6；報告 `.superpowers/sdd/tasks-ui-language/final-review.md`）；審查者實跑 fmt／clippy／workspace 1238 passed／i18n-check ①–④ 931＋423／openspec validate／markdownlint／deid-check（HEAD）0 命中，並逐張看 4 張截圖。合併前必修：無。
  - 控制端依建議修文件：M1 cockpit/README 註明啟動器設定與命令列錯誤細節維持繁中；M2 design D2（i18n.js 在 head＋i18n-pending）、D7（腳本帶 `--lang=zh-TW`）改寫；M3 spec 補兩個 400 不帶代碼的例外；M5 handover v39。
  - Final: minor (parked): M4（Stage1 M4）D5 守門解析器只認單行條目、只靠鍵名 — Ruling: 不修 — 目前字典全為單行、守門鍵合規，審查判定非阻擋 — 若錯：日後改字典格式或命名時守門可能靜默失效。
- Task 5.4: complete；change 全部 task 完成。
