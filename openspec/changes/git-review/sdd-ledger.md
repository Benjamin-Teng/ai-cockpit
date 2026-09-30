# SDD ledger — plan: openspec/changes/git-review/tasks.md

- spec：`openspec/changes/git-review/specs/`（binding）；design：`openspec/changes/git-review/design.md`
- 分支：`feat/git-review`，起點 `45f1174`（main）；propose commit `126ef47`
- 授權：使用者 2026-09-28 離席前授權「這次的 change 全部給 Claude 採納，second opinion 給 Codex 審核」。
  task review 一律 Codex（`codex-companion.mjs adversarial-review`，只採信最後「# Codex Adversarial Review」段）。
- 暫存產物（brief、report、review package）：`.superpowers/sdd/tasks/`（git 忽略）

## Codex 可用性

- 2026-09-28 20:09：propose 審查（R-propose-1）撞 usage limit，恢復時間 2026-09-29 00:22。依 handover 對策：審查排隊、實作不停，
  未經 Codex 通過的 task 不勾選、不宣稱完成。恢復後順序：propose 審查 → 依 task 順序補審（可合併 `--base` 較早的範圍）。
- **2026-09-30 使用者決定：本 change 不等 Codex（週限額到 2026-10-04 才恢復），改由 Sonnet 5.5 subagent 驗收，並記載
  「未經 Codex 審查」。** 這是使用者當下指示覆寫 `~/.claude/CLAUDE.md`「diff 審查一律 Codex」的單次例外，不延伸到其他 change。
  審查依 handover 原批次切分（1、2、3、4、4b、5），結果見「Sonnet 驗收」節；本 change 的所有 task 勾選以此為準，**全部未經 Codex 審查**。

## Pre-flight 衝突掃描

| 對象 | 產出 → 使用 | 結果 |
|---|---|---|
| 1.2 ↔ 2.1 | 1.2 實測固定前綴與 `--end-of-options` 可用性 → 2.1 的 argv 斷言 | 一致；1.2 若推翻某旗標，先 `/opsx:update` 再做 2.1 |
| 1.2 ↔ 2.3 | 1.2 記錄的真實輸出 → 2.3 解析器 fixture；1.2⑤ 決定 `ChangedFiles` 一次或兩次呼叫 | 一致 |
| 2.1 ↔ 2.2 | 2.1 的 `GitQuery`／`GitTarget` → 2.2 執行器 | 一致 |
| 2.2 ↔ 3.2 | 3.2 要以「執行器測試計數器」斷言「沒有啟動任何子程序」，2.2 沒提到計數器 | 見 Ruling P1 |
| 2.4 ↔ 3.3 | 2.4 未追蹤檔案的純函式吃位元組 → 3.3 以 5a 路徑界限讀檔後傳入 | 一致 |
| 2.6 ↔ spec | spec「WSL repo 在 WSL 內執行且引數不經 shell」的層級是端點，2.6 只在 crate 層、且為 `#[ignore]` | 見 Ruling P2 |
| 3.1 ↔ 3.2／3.3 | 3.1 的 `ui_preview` fixture 給前端腳本用；3.2／3.3 測試自建暫存 repo | 不共用，一致 |
| 3.3 ↔ 4.1 | 3.3 以空殼 `/app/git.js` 完成路由測試 → 4.1 填入骨架 | 一致（3.3 只建空殼與路由） |
| 4.1 ↔ 4.2–4.5 | 4.1 的分頁 `kind` 介面 → 各 kind 模組 | 一致 |
| 4.2 ↔ 4.3–4.5 | 4.2 建立 `git-check.js` → 後續 task 追加段落 | 一致 |
| 4.5 ↔ file-review delta | 「還原 git 分頁與變更分頁」需 diff 與 Graph 分頁都已存在 → 放在 4.5 | 一致 |
| 每個 task 自身 | 驗收列出的 scenario 與 spec 名稱逐一核對 | 2.6 的標記檔腳本在 Windows 以 git 內附的 sh 執行（git for Windows 以 sh 執行設定中的指令），可接受 |

- Ruling P1: 執行器提供只在測試可見的啟動計數（`#[cfg(test)]` 無法跨 crate，改用 `cockpit-git` 的 `test-support` feature 或建構子注入
  計數器，由 3.2 實作時決定並加在 2.x 已完成的 crate 上）— spec「沒有啟動任何子程序」需要可觀測的證據，這是最小侵入的做法 —
  錯了的代價：feature 名稱或注入方式要改，影響只在測試碼。
- Ruling P2: 端點層的 WSL 注入情境不另建端點測試；crate 層 `#[ignore]` 測試＋5.2 的 WSL 真機驗收合起來涵蓋，因為端點層只負責選目標
  （2.1 的純函式已單元測試），實際執行路徑完全在 crate 內 — 錯了的代價：端點層若另有把路徑交給 shell 的程式碼會漏測；Codex review
  需特別看 `cockpit/src/git.rs` 是否只經 `cockpit-git` 執行。

## 基線

- 2026-09-28，`feat/git-review` @ `44eb00e` 之前（僅文件變更）：`cargo test --workspace` 718 passed／0 failed／11 ignored；
  `cargo test -p cockpit --example ui_preview` 31 passed；既有 8 支腳本全 PASS（visual-check 27 段、files-check 自我測試 2＋25 段 0 FAIL）。
  報告：`.superpowers/sdd/tasks/task-1.1-report.md`。
- Task 1.1: complete（只跑不改，無 diff，免 Codex review）

## 進度

- Task 1.2: DONE（`eeec154`，probe 文件 `docs/research/2026-09-28/git-review-probe.md`）；唯一不一致 ⑤ → design D4 改為
  `ChangedFiles` 固定分兩次呼叫並以 `--name-status` 為準合併。Codex review：排隊中（usage limit）。
- Ruling R1: `--name-status`／`--numstat` 分兩次呼叫、以 name-status 為準、numstat 缺漏時行數 null — 實測兩版本 git 都無法一次輸出；
  以 name-status 為準是因為狀態與路徑是 spec 的必要欄位、行數可為 null — 錯了的代價：兩次呼叫間檔案變動時行數短暫不準（下次輪詢修正）。
- Task 2.1: DONE（`7a3f3d9..4f4b9e4`：`5d6f959` crate 骨架、`4f4b9e4` ADR-0007；56 tests＋1 compile_fail doctest）。Codex review：排隊中。
  實作者疑慮：①`ChangedFiles`／`FileDiff` 兩側組合的子命令映射（`Oid→INDEX` 帶明確 oid、`diff-tree` 加 `-p` 出 patch）是依 git
  語意推得，未經 probe 實測 → 交 2.4／2.6 以真實 repo 驗證；②`RefName` 驗證為 check-ref-format 的保守子集 → 可接受，因為 D5 規定
  ref 必須逐字等於當下 `for-each-ref` 輸出才放行，驗證只是縱深防禦。
- Task 2.2: 實作者 DONE（`b1a1bcb..0a9c41f`）。控制端檢查發現 `QueryPlan` 欄位與 `GitRunner::execute` 皆 `pub`，crate 外可執行任意
  argv，違反 design D1 Goal → fix round 1/5 已交原實作者（封閉入口只經 `GitQuery`，任意 argv 入口限 `test-support` feature）。
  其他記錄：多次呼叫查詢中單次非零結束不中止整個查詢（`RunOutput.calls` 為 `Vec<Result<…>>`，2.3 須依此解析）；`Blob` 先查
  `BlobSize` 屬呼叫端編排，留給 3.3；stderr 保留前 8 KiB。
- Task 2.2: fix round 1/5（1 addressed by implementer, 待 Codex 確認；`0a9c41f..b4d9625`）——`QueryPlan` 欄位私有、正式入口只剩
  `GitRunner::run<Q: GitQuery>`，任意 argv 入口限 `test-support`；以未開 feature 的暫時 consumer crate `cargo check` 證明四條路徑
  編譯失敗（doctest 會因 dev-dependency 自我引用洩漏 feature 而無效，已記於報告）。lib 測試數 59（原報告的 64 為誤記，未刪測試）。
  Codex review：排隊中（範圍 `b1a1bcb..b4d9625`）。
- Task 2.3: DONE（`90ff6c3..d692a44`，101 lib＋11 整合＋1 doctest）。`parse` 為 inherent 方法、吃 `RunOutput::calls`；新增
  `GitParseError`。待 2.6 以真實 repo 驗證：`--numstat -z` 改名紀錄格式（推導未實測，錯時僅行數落回 null）。衝突紀錄一律 `U`
  （符合 spec 字母集合）。Output 未加 Serialize，JSON 形狀留給 3.2。Codex review：排隊中。
- Task 2.4: DONE（`7ca2db4..bad43d7`，145 lib＋11 整合＋1 doctest）。2.1 的兩側組合 argv 以真實 repo 實測全部正確；`diff-tree -p` 根 commit
  仍多印一行 commit hash（probe ⑥ 未涵蓋），解析器已處理。Codex review：排隊中。
- Ruling R2: 帶 `old_path` 但 git 未判定為改名（相似度低於門檻）時輸出兩個 `diff --git` 區塊 → 依序合併 rows、不報錯 — 使用者仍看得到
  完整差異，且從「變更」清單點進來的改名一定是 git 已判定的改名 — 錯了的代價：極少數情況畫面呈現為「整檔刪除＋整檔新增」。
- Ruling R3: `is_oid_like` 只用於略過 `diff-tree` 輸出第一行的 commit hash，接受 — 路徑行一定以 `diff --git` 開頭，不會被誤判。
- Task 2.4: minor (deferred): `FileDiff::parse` 未去除行尾 `\r`，`untracked_file_diff` 有去除，兩者不一致（顯示用）。
- Task 2.5: DONE（`3c25edf..11ec5b9`，graph 9 tests 含隨機 DAG 連續性與前綴穩定性質測試）。API：`GraphCommit<'a> { oid, parents }`，
  不依賴 `LogRow`。Codex review：排隊中。
- Task 2.2: fix round 2/5 已交原實作者——`timeout_terminates_the_subprocess` 在負載下偶發失敗（單獨跑 0/20）；根因：逾時 150 ms 短於
  忙碌時假程式啟動到寫 PID 的時間，測試前提不成立（測試競態，非執行器 bug）。
- Task 2.2: fix round 2/5（`11ec5b9..3983d3b`，只動 `tests/runner.rs`）：三個計時測試加大安全餘裕，`--test runner` 連跑 10 次全綠。
  Codex review：排隊中（範圍 `b1a1bcb..3983d3b` 中屬 2.2 的 commit）。
- Task 2.6: DONE（`6ffa5a4`，`cockpit-git/tests/real_git.rs` 29 passed＋1 ignored；WSL 版手動 `--ignored` 連跑 3 次 ok）。以真實 git 驗證
  2.1–2.5 全部正確（含 2.3 的 numstat 改名格式，以 hex dump 確認）；安全性與不寫 index 測試皆附「不帶唯讀前綴會觸發」的對照測試。
  Codex review：排隊中。
- 事故：2.6 實作者在 repo 內子目錄做實測，`git init` 失敗後把兩個 scratch commit（`384f5c5`、`3133623`）提交進本分支；它嘗試
  `reset --hard` 被 auto-mode 擋下並要求控制端 rebase。Ruling R4: 不執行被擋下的改寫歷史操作，改以 `git revert`（`f7d147c`、
  `f99a31f`）撤銷 — 不改寫歷史、分支最終會 squash，結果等價；`git diff 3c7c1bf HEAD` 只剩 `real_git.rs` — 錯了的代價：分支歷史多 4 筆
  雜訊 commit，squash 後消失。已寫 memory `git-in-subdir-falls-through-to-enclosing-repo`；之後派工的 brief 一律要求暫存 repo 建在
  repo 外並先驗 `rev-parse --show-toplevel`。
- Task 3.1: DONE（`db5c075..b806f77`，只改 `ui_preview.rs`）。`review-repo` 264 commit（fast-import）＋各情境集中在 `history/`；
  `other-repo` 改為進行中的衝突 merge（HEAD 在 `branch-b`；既有腳本只依賴其 `README.md` 與列目錄，未受影響）。兩次啟動 HEAD 相同。
  隔離：`GIT_CONFIG_NOSYSTEM=1`＋`GIT_CONFIG_GLOBAL` 指空檔、`init` 後驗 `show-toplevel`。啟動多約 1.2 秒。8 支腳本全綠、未改斷言。
  Codex review：排隊中。
- Task 3.2: 實作者 DONE（`4cb378b..5a476cb`，git_endpoint 23 passed＋git.rs 8 單元測試）。中途因 Claude API session 上限中斷一次，
  resume 後完成。fix round 1/5 已交原實作者：log 的不存在 `tip` 應回 `rev_unknown`。
- Ruling R5: 只在 `Log` 失敗時才逐一 `VerifyCommit` tips 以分出 `rev_unknown` — 正常路徑不多花 N 次 git 呼叫（WSL 每次約 70 ms）—
  錯了的代價：失敗路徑多幾次呼叫，無正確性影響。
- Ruling R6: `limit>200` 回 400 維持 — spec 明文「最大 200……違反皆回 400」。
- Codex propose 審查：00:25 起的兩次 `--wait` 都提早返回（job 其實仍在跑）；以 `status`／`result` 背景輪詢取結果。
- Task 3.2: fix round 1/5（`5a476cb..c1391a4`）：log 失敗時才逐一驗 tips → `rev_unknown`；補 3 個測試（含 `changes` 的不存在 hash）。
  git_endpoint 26 passed。Codex review：排隊中。
- 2026-09-29 01:10：Codex 再度 usage limit，**恢復時間 2026-10-04 10:55**（週限額）。00:25 那兩個 job 的 worker 在背景 shell 結束後
  失聯（停在第一個指令，`cancel`／`result` 皆 "No job found"），已放棄。Ruling R7: 實作照常進行、全部 task 標「待 Codex 審查」
  不勾選；不以 Claude subagent 代替 Codex 當 diff reviewer（CLAUDE.md「Code review 路由」與使用者指示）；控制端逐 task 做事實檢查
  作為補充而非替代。10/04 恢復後依序補審：propose（worktree `44eb00e`）→ 2.x → 3.x → 4.x，可合併 `--base` 較早的範圍 —
  錯了的代價：問題晚 5 天才被 Codex 發現，返工範圍較大；使用者回來可選擇改用其他審查方式。
- 推測（未證實）：`adversarial-review --wait` 放在背景 Bash 時提早返回、worker 失聯；也可能是當時已接近用量上限而卡住。
  之後一律在前景執行（timeout 600000），若再發生再判別。
- Task 3.3: 實作者 DONE（`2a68a82..e9eac1a`，git_endpoint＋http 54 passed）。fix round 1/5 已交原實作者：`meta` 的 `blob` 是 FNV
  內容雜湊（讀整份 blob），違反 spec「物件 hash」且暫存區分頁每 2 秒輪詢會整份讀取最多 50 MiB。
- Ruling R8: 封閉查詢新增 `BlobId`（`rev-parse --verify -q --end-of-options <rev>:<path>`），`meta` 以它取物件 hash、不讀內容；design D1／D4
  已更新 — 最便宜且符合 spec 字面 — 錯了的代價：多一種查詢型別，需補 argv 與整合測試。
- Ruling R9: 未追蹤檔案（`EMPTY→WORKTREE`）帶 `old_path` 回 400 — 未追蹤檔案不可能是改名，拒絕比默默忽略更早暴露前端錯誤 — 錯了的代價：無。
- Task 3.3: minor (deferred): `render` 對非 Markdown 副檔名先跑一次 `BlobSize` 才回 `not_markdown`，可把副檔名判斷提前。
- Task 3.3: fix round 1/5（`9002edf..38ca201`）：新增 `BlobId`，`meta` 不再讀內容。帶出新偏差：`viewer` 分類退化為只看副檔名，違反 spec
  「分類規則同 file-review 中繼資料端點」（副檔名＋前 8192 位元組）→ fix round 2/5 已交原實作者。
- Ruling R10: 新增封閉查詢 `BlobHead`（argv 同 `Blob`，stdout 上限 8192、可截斷）供 `meta` 分類，design D1／D4 已更新 — 符合 spec 且不整份讀取
  — 錯了的代價：多一種查詢型別。
- Task 3.3: fix round 2/5（`9f3d2cf..61d43de`）：`BlobHead` 上線，`meta` 以 `cockpit_files::classify_viewer_bytes`（自 5a 分類拆出的純函式）
  分類，與 5a 中繼資料端點逐字一致（有 RED→GREEN）。git_endpoint 44、cockpit-git lib 169／real_git 34、cockpit-files 119 全綠。
  Codex review：排隊中（範圍 `2a68a82..61d43de` 中屬 3.3 者）。
- Task 4.1: DONE（`0083632..e6d979c`：`3dbb833` 產品、`e6d979c` 腳本）。kind 介面寫在 `files.js` 開頭與 task-4.1-report；`left:"changes"`
  在 4.2 前視同 `files`。新段「舊格式照常還原」在舊前端本來就會過（v:1 即舊格式），無法示範 RED，接受。8 支腳本全綠。Codex review：排隊中。
- Task 4.2: 實作者 DONE（`3832a13..1c1d998`：產品 `b04dc7b`、腳本 `1b3c2d7`／`837beeb`／`1c1d998`；9 支腳本全綠，git-check.js 新建）。
  控制端設計審核（截圖＋frontend-design）→ fix round 1/5：①1536 寬左欄標頭溢出、根目錄 name 未顯示（違反 spec）；②窄欄檔名與資料夾
  同時截斷，改為資料夾先縮。
- Task 4.2: fix round 1/5（`1c1d998..1e9adce`：產品 `5638bf3`、腳本 `1e9adce`）：標頭兩行、資料夾先縮；新截圖控制端目視確認通過；
  git-check 新增 bounding-rect 不重疊斷言（1536／1100）。本輪未重跑其餘 6 支舊腳本（改動限 `#changes-panel` 範圍）→ 由 4.3 收尾全跑補上。
  Codex review：排隊中。
- Task 4.3: DONE（`e4fdf96..ac8a8a6`：產品 `66fce7c`、腳本 `ac8a8a6`；9 支腳本全綠，含 6 支舊腳本補跑）。diff 分頁 serialize／deserialize
  已實作，完整還原驗收在 4.5。控制端設計審核（1536 gap 截圖、700 截圖）：diff 呈現符合 spec 與視覺規則，無 finding。Codex review：排隊中。
- Task 4.2: fix round 2/5 已交原實作者——控制端在 4.3 截圖發現：左欄在「變更」時整頁重畫會把 Project 清單顯示回來（`render.js:1105`
  仍以 `leftTab === "files"` 二選一判斷）；4.2 的腳本未涵蓋重畫情境，補段 `git-review/重畫不影響變更分頁`。
- Task 4.2: fix round 2/5（`ac8a8a6..add5cc7`：產品 `cacfb27`、腳本 `add5cc7`）：`render.js` 改為只有 `projects` 才顯示 Project 清單；
  新段 `git-review/重畫不影響變更分頁` 有 RED→GREEN；9 支腳本全綠。已寫 memory `binary-check-misroutes-new-enum-value`。
- Task 4.4: DONE（`e62d546..47ffad2`：產品 `ae95762`、腳本 `47ffad2`；git-check 13 段、9 支腳本全綠）。控制端設計審核（1536／700／popover
  截圖）：車道、merge、HEAD、ref 標籤三種樣式、窄視窗皆符合，無 finding。4.5 掛點：`git.js` 的 `onCommitSelected(tab, oid)`。
  Codex review：排隊中。
- 觀察：既有測試 `cockpit/tests/app.rs` `abort_await_is_bounded`（change 2 加入，main 上就有）在 4.4 的整批測試中偶發失敗一次、重跑轉綠；
  非本分支引入。5.4 全 gate 若再出現就查根因。
- Task 4.5: DONE（`bb394e2..2199bf3`：產品 `0b885e7`、腳本 `2199bf3`；git-check 20 段、9 支腳本全綠）。控制端設計審核（詳情、比較、
  某版本 Markdown 截圖）：符合 spec。Codex review：排隊中。
- Task 4.5: minor (deferred): 詳情展開區塊處 Graph 車道線中斷（下方續接），不影響閱讀。
- Task 4.5: minor (deferred): 「parent 不在已載入範圍時顯示純文字」以行內樣式實作以避開 CL1 死規則檢查，且無自動化斷言（實作者以臨時段
  手動驗證）；Ctrl／⌘＋點選比較無專屬自動化段（與按鈕流程共用分流）。
- Task 5.3: DONE（`2a1fc54`：git-check.md、visual-check.md、CONTEXT.md 四個詞條；markdownlint 108 files 0 issues）。Codex review：排隊中。
- Task 5.1: DONE_WITH_CONCERNS→控制端判定通過：git-check 19 段（2 self＋16 git-review＋1 file-review；4.5 報的 20 為誤數，spec 前端情境
  逐一有對應段）、files-check 30、visual-check 28 段中 DF1 收尾的 chrome PID 確認偶發 FAIL 一次，控制端單獨重跑 `DF1` 3/3 PASS →
  判定為既有收尾時序 flaky，非本分支回歸；其餘 7 支 PASS。三寬 9 張截圖在 scratchpad `shots-5.1\`。
- 觀察：visual-check `DF1` 收尾偶發 flaky（taskkill 後 tasklist 查詢的時序），與 `abort_await_is_bounded` 同列為既有 flaky，交使用者決定是否另修。
- Task 5.2: DONE（`3952115`）：WSL HERDR 0.8.2＋真 cockpit.exe＋WSL 既有 repo（唯讀），root／status／refs／log／commit／diff 皆 200、
  headless 畫面正常；`.git/index` 前後 `1790586195:29344` 不變、無 `index.lock`。控制端把 probe 文件第 49 行的真實使用者名稱改為 `<user>`。
- Task 5.4: 全 gate（控制端 2026-09-29 當場跑）：fmt、clippy 通過；`cargo test --workspace` 988／0／12；ui_preview 38；markdownlint 108 files
  0 issues；`openspec validate --all` 18 passed；腳本見 5.1。**整支分支 Codex review 未跑**（週限額至 10/04 10:55），5.4 未完成。
- Task 5.5: `docs/handover.md` v26 重寫；fresh subagent 事實核對抓到 3 處錯（分支合併描述、審查批次空隙、腳本段數）＋README 把本 change
  誤標 change 6，皆已修。使用者目視驗收與 Codex 審查完成前不標記完成。

## 目視驗收（使用者 2026-09-30 交由 Claude 處理）

- headless Chrome 19 步截圖（scratchpad `visual-accept\`），控制端逐張判定。通過：三種 diff、刪除檔、Graph 頂端／merge 詳情／比較／自分岔點起、
  分支篩選、搜尋、分批載到底（264 列）、某版本 Markdown、衝突組列出、重新整理後分頁還原、700 寬三畫面。
- 不是缺陷：重新整理後沒有選定的 pane → spec「Live Output 的 pane 選取不還原」（5a 起的既有行為）。
- **缺陷 V1**：「合併衝突」組點檔案 → `diff` 502。根因（控制端在 repo 外暫存 repo 重現）：未合併檔案的 `git diff`（index→worktree）輸出
  `diff --cc` 三方格式、exit 0，解析器視為格式錯誤。已寫 memory `git-diff-unmerged-path-emits-combined-format`。
- Ruling R11: 「合併衝突」組改為 HEAD → 工作區；`FileDiff` 新增 hash→`WORKTREE` 組合；解析器遇 `diff --cc` 回 409 `unmerged_path`；spec
  「左欄變更分頁」「git 端點的共同規則」「單檔 diff 端點」與 design D5 已更新 — HEAD→工作區的 unified diff 能直接呈現衝突標記，且不必實作
  三方格式 — 錯了的代價：看不到「theirs」那一側的原始內容（衝突標記內已含）。
- **缺陷 V2**：刪除檔（工作區不存在）的 diff 分頁，「開啟檔案」看不出停用樣式；「看右側版本」（工作區側）仍顯示但開不出內容 →
  Ruling R12: 工作區不存在時「開啟檔案」與指向工作區的「看…版本」都停用，且有可辨識的停用樣式 — spec 已規定「開啟檔案」停用，
  工作區側的「看版本」同理 — 錯了的代價：無。
- V1／V2 修正：`6d87db4..232c293`（`b59b71a` Rust、`25a2334` 前端、`232c293` 腳本）。控制端目視確認修正截圖：衝突檔顯示「hash → 工作區」、
  衝突標記為新增列；刪除檔兩個按鈕呈停用樣式。控制端當場 gate：fmt、clippy 通過；`cargo test --workspace` 997／0／12；ui_preview 38；
  markdownlint 108 files 0 issues；`openspec validate --all` 18 passed；實作者回報 9 支腳本全 PASS（git-check 21 段）。
- **目視驗收：通過**（使用者授權由 Claude 判定）。Codex review：排隊中（範圍 `7003b5a..232c293`）。

## Sonnet 驗收（2026-09-30，取代 Codex）

- Ruling R13: 工作區側 diff 改用 plumbing（`diff-files`／`diff-index`）— porcelain `git diff` 遇 stat 變舊的檔案會刷新並重寫 `.git/index`（`--no-optional-locks` 擋不住），diff 分頁每 2 秒輪詢會與 agent 的 `git add`／`commit` 搶 `index.lock`，違反 spec「不取得 optional lock、不寫入 index」；來源：Sonnet 驗收批次 1 B1；`ChangedFiles` 對 `diff-files` 濾掉「狀態 M 且不在 numstat」的 stat-dirty 假陽性 — 錯了的代價：若 numstat 判斷有漏（例如某種變更 numstat 不列），該檔會從清單消失；已實測純權限變更仍列 `0\t0`，並有測試涵蓋，且截斷時不濾。
- Ruling R14: 固定前綴加 `-c diff.suppressBlankEmpty=false` — repo 設 `diff.suppressBlankEmpty=true` 時 git 對空白 context 行輸出 `""` 而非 `" "`，`file_diff.rs` 把它當殘渣跳過，漏列且後續行號靜默錯位；來源：Sonnet 驗收批次 2 — 錯了的代價：無（命令列 `-c` 覆寫 repo 設定，只影響 diff 輸出格式）。
- Ruling R15: `GitRunner` 的 stderr 保留前 8 KiB 但讀到 EOF、其餘丟棄 — 原本讀滿 8 KiB 即關閉管道，git 之後再寫 stderr 會 SIGPIPE 非零結束（WSL 實測：`core.autocrlf=true`、400 個檔的 CRLF 警告約 51 KB，`ChangedFiles` 兩次呼叫 exit 13 → 502）；來源：Sonnet 驗收批次 2 — 錯了的代價：無（stdout 超上限終止子程序的行為不變；kill 後等 stderr 設 2 秒逾時保險）。
- 其他修正（無新裁決）：批次 4b F1 只有一邊有 stage 的衝突（UD、DU 等）對 `INDEX→WORKTREE` 只印 `* Unmerged path <f>` → 也回 409
  `unmerged_path`（`9534d1f`）；批次 4 M1 隱藏的 Git Graph 分頁在背景載到 5000 筆、M2 進入比較時標頭 `undefined`／TypeError、M3 鍵盤按
  「看此版本」被整列攔走開成 diff（`8d3e170`，回歸段 `054e10e`，`self/段落代號` 計數 18→21 為段落清單計數、非行為斷言）。
- 批次與結論（reviewer＝Sonnet 5.5 subagent，唯讀；findings 由控制端實測或讀碼確認後才修）：

| 批次 | 範圍 | 結論 |
|---|---|---|
| 1 | `main..44eb00e` | B1（R13）已修；其餘延後 |
| 2 | `44eb00e..db5c075` | F1（R14）、F2（R15）已修；其餘延後 |
| 3 | `db5c075..0083632` | Ruling P2 通過（`cockpit/src` 無 `Command::new`）；findings 延後 |
| 4 | `0083632..34ceba5` | M1–M3 已修；minor 延後 |
| 4b | `7003b5a..232c293` | F1 已修；minor 延後 |
| 5 | `main..HEAD` | 架構約束全過、無必修；deferred minors 與 Ruling 皆判「合理／可延後」 |

- 延後（已知限制，候選 change 7）：
  - Graph 預設起點把全部 ref tip 放進 argv 與 URL `tip` 參數，約 780 個以上不同 tip 超過 Windows 命令列上限 → 503 `git_unavailable`
    （訊息誤導）；根治需改 design（`git log --stdin` 或 `--branches --remotes --tags HEAD`）。批次 1 M1、批次 3 F1。
  - tag 指向非 commit 物件時 Graph 失敗（批次 1 M2）；`Refs` 1 MiB 不可截斷，極多 refs 時整頁無 refs（批次 1 m9、批次 2 minor 3）。
  - 逾時只殺 `cmd\git.exe` 轉手程式，內層 git 可能成孤兒（未證實，批次 1 M3）；未清除繼承的 `GIT_DIR`／`GIT_WORK_TREE`／
    `GIT_INDEX_FILE`；Native 未設 `LC_ALL=C`，dubious ownership 判斷依賴英文 stderr（批次 1 m5、批次 2 minor 4）。
  - 空或損壞的 `.git` 資料夾會讓 git 落到外層 repo（批次 1 m4）；WSL distro 沒裝 git 時回 502 而非 503（批次 1 m6）；
    `//wsl.localhost/`、`\?\UNC\` 形式未判為 WSL（未證實，批次 2 minor 2）。
  - 手打 URL：未追蹤 diff 路徑是資料夾（含未追蹤的巢狀 repo `dir/`）回 500；含 `:`、`\`、結尾 `.`／空白、Windows 保留名的合法 git 路徑
    在清單可見但點開 400（批次 1 m7／m8、批次 2 minor 1、批次 3 F2／F4）；`diff` 未追蹤讀檔在 async 內阻塞 I/O（批次 3 F3）。
  - 「對方刪除」（UD）衝突檔 HEAD→工作區 diff 為空、無說明（批次 4b F2）。commit 詳情與比較清單被截斷時前端無提示（批次 5）。
  - 前端：搜尋後分批載入把畫面拉回命中列；詳情重建後焦點掉到 body；一次性讀取失敗後切回不重試；`restoreTabs` 的 `create` 無 try/catch
    （未證實）；listbox 內夾非 option 節點（批次 4 m1–m4）。
  - 測試缺口：WSL `$(touch pwned).md` 與 dubious ownership 的端點層驗證、Markdown-at-rev 前端自動化段（批次 5）。
- 最終 gate（控制端 2026-10-01 當場跑，`054e10e`）：fmt、clippy 通過；`cargo test --workspace` 1004／0／12；ui_preview 38；markdownlint
  108 files 0 issues；`openspec validate --all` 18 passed；9 支腳本全 PASS（git-check 24 段）。WSL git 2.43.0 另以 touch＋mtime 實測
  `diff-files`／`diff-index` 不改寫 index。**Sonnet 驗收：通過**，task 全勾；本 change 全程未經 Codex 審查。
