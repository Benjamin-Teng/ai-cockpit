# 交接：下一段任務

> **建立日期**：2026-10-01　|　**上一段做完的事**：change 5b `git-review` 經 Sonnet 5.5 分 6 批驗收（修 7 項）→ squash 併回 `main`
> → archive（delta spec 同步進主規格）。**全程未經 Codex 審查**（使用者 2026-09-30 決定不等週限額）。
> **性質**：接手用文件，會過期，每段重寫。
> 為什麼做看 `docs/cockpit-spec.md`（北極星）與各 change 的 proposal；怎麼做看 `~/.claude/CLAUDE.md`（本 repo 精簡版在
> `AGENTS.md`）；規格看 `openspec/specs/`。

## 0. 三十秒版本

1. **沒有 active change**，`main` 乾淨、無 feature 分支。下一段＝**change 6「進度模型」brainstorming**（第 3 節）。
2. `git-review` 未經 Codex 審查是使用者的單次決定，**不延伸到其他 change**：change 6 起 diff 審查照 CLAUDE.md 一律 Codex。
   若 Codex 恢復後（2026-10-04 10:55 起）想補審已併回的 5b，範圍＝`45f1174..427118b`，由使用者決定要不要做。

## 1. 現在的狀態

- **`main`**：change 1a／1b／2／3／4／5a／5b 與小 change `html-charset`。本段 commit：`427118b`（5b squash）、`e1d75bb`（archive）、本檔。
  沒有 remote。
- 5b 內容：新 crate `cockpit-git`（ADR `docs/adr/0007-cockpit-git-crate.md`）、`cockpit/src/git.rs` 的 `/api/git/...` 唯讀端點
  （說明在 `cockpit/README.md`「git 唯讀讀取」）、前端 `cockpit/assets/app/git.js`（變更分頁、diff、Git Graph、某版本檔案分頁）。
  規格 `openspec/specs/git-review/`；紀錄 `openspec/changes/archive/2026-10-01-git-review/sdd-ledger.md`（Ruling、Sonnet 驗收、延後清單）。
- **可用指令**（repo 根目錄）：
  - 全 gate：`cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test --workspace && cargo test -p cockpit --example ui_preview && markdownlint-cli2 "**/*.md" && openspec validate --all`
  - 預覽：`cargo run -p cockpit --example ui_preview`（`127.0.0.1:7770`；需要本機安裝 git）
  - 驗收腳本（不可並行；清單見 archive 內 `tasks.md` 通則）：`node docs/research/2026-09-28/git-check.js [段代號,...]`（用法 `git-check.md`）、
    `files-check.js`、`visual-check.js` 與既有六支。`factory-floor-check.js` 跑完 `git checkout -- docs/research/2026-09-16/task-5.2-scenario-d.png`。
  - WSL 版 git 整合測試（opt-in）：PowerShell `$env:COCKPIT_GIT_TEST_WSL_DISTRO="Ubuntu-24.04"; cargo test -p cockpit-git -- --ignored`
- **測試數字**（2026-10-01，`054e10e`，當場跑為準）：`cargo test --workspace` 1004 passed／0 failed／12 ignored；`ui_preview` 38；
  9 支腳本全 PASS（git-check 24 段）；`markdownlint-cli2` 108 files 0 issues；`openspec validate --all` 18 passed。

## 2. 立刻要做：change 6「進度模型」brainstorming

使用者 2026-09-26 定案的方向：

- 進度目前全靠人工按按鈕。改成 agent 呼叫 cockpit 的 API 回報進度，寫入 cockpit 自己的狀態檔，**不需要推翻「對 HERDR 唯讀」**；
  「HERDR 的 `done` 不等於 task 完成」保留。
- 活動狀態目前只到 workstream 層級（見 `CONTEXT.md`），要精確到 task；推進要有「退回」操作。

流程：`git switch -c feat/<slug>` → superpowers:brainstorming → `/opsx:propose`（`tasks.md` 開頭寫執行路徑分流）→ apply → Codex 審查 → 併回。

## 3. 接著要做：change 7「畫面與操作修補」

- 既有：Live Output 上色；M2（斷線期間無法取消改綁，`ProjectedBinding::RuntimeDisconnected` 不帶 `source`）；U1（滑鼠點擊後殘留冰青焦點框，
  改 `:focus-visible`，確認鍵盤焦點還原不退步）；U2（服務斷線時右欄 runtime 卡仍綠色 connected）。
- 5b 延後清單（逐條在 archive ledger「Sonnet 驗收」節）。優先度較高的：
  - **Graph 起點全放 argv／URL**：約 780 個以上不同 ref tip 的 repo 超過 Windows 命令列上限 → 503 `git_unavailable`（訊息誤導）。
    根治要改 design（`git log --stdin` 或 `--branches --remotes --tags HEAD`，分批一致性另想）。
  - tag 指向非 commit 物件時 Graph 失敗；`Refs` 1 MiB 不可截斷。
  - 逾時只殺 `cmd\git.exe` 轉手程式（未證實）；未清除繼承的 `GIT_DIR` 等環境變數；Native 未設 `LC_ALL=C`。
  - 前端：搜尋後分批載入把畫面拉回命中列、詳情重建丟焦點、一次性讀取失敗不重試、commit 詳情截斷無提示。
  - 其餘：Git Graph 詳情展開處車道線中斷、diff 行尾 `\r` 不一致、手打 URL 的資料夾路徑回 500 等。

## 4. 這一段踩過的坑

（**不會報錯的錯誤**加粗。）

git 與 WSL：

- **`wsl.exe -d <distro> -- <cmd>` 會經 Linux shell 重新解析引數**（`'$HOME'` 被展開，等於 shell 注入）；**`--cd <不存在路徑>` 靜默退回 `/`
  且 exit 0**。正解：一律 `--exec`，目錄交給 `git -C`（memory `wsl-exe-argument-passing-pitfalls`）。
- **在 repo 子目錄建暫存 repo，`git init` 失敗後 add／commit 會靜默落到外層真實 repo**：本段發生一次（2.6，已 revert）。正解：暫存 repo
  建在 repo 外並先驗 `rev-parse --show-toplevel`（memory `git-in-subdir-falls-through-to-enclosing-repo`）。被擋下的 `reset`／`rebase`
  不轉手給別人執行，改 `git revert`。
- `git diff` 的 `--name-status` 與 `--numstat` 不能同時輸出（兩版本都只印前者）→ 分兩次呼叫以路徑合併。
- `git diff-tree -p --root` 即使有 `-p` 仍先印一行 commit hash。
- `-M` 相似度低於門檻時，帶 `old_path` 的 diff 會拆成兩個 `diff --git` 區塊（Ruling R2：依序合併）。
- 未加 `--no-optional-locks` 的 `git status` 會改寫 index（實測對照測試在 `cockpit-git/tests/real_git.rs`），會跟 agent 的 commit 搶 `index.lock`。
- **porcelain `git diff`（工作區側）加了 `--no-optional-locks` 仍會刷新並重寫 `.git/index`**（只有 `status` 守這個旗標），exit 0、輸出無差異。
  正解：工作區側改 `diff-files`／`diff-index`（要 `-p`；name-status 的 stat-dirty 假陽性以 numstat 過濾），Ruling R13
  （memory `git-porcelain-refreshes-index-despite-no-optional-locks`）。單元測試與腳本抓不到，是 review 以 touch＋mtime 實測發現。
- **repo 設 `diff.suppressBlankEmpty=true` 時空白 context 行輸出 `""`**，解析器漏列、後續行號靜默錯位 → 固定前綴 `-c diff.suppressBlankEmpty=false`（R14）。
- **stderr 讀到上限就關管道，git 之後再寫 stderr 會 SIGPIPE**（WSL 大量 CRLF 警告 → exit 13 → 502）→ 超過上限後持續排空（R15）。
- 只有一邊有 stage 的衝突（UD、DU）對暫存區→工作區只印 `* Unmerged path <f>`，不是 `diff --cc` → 同樣回 409。
- **對衝突中（未合併）的檔案跑 `git diff`（暫存區→工作區）會輸出 `diff --cc` 三方格式、exit 0**，unified 解析器當格式錯誤 → 曾回 502。
  正解：衝突組改用 HEAD→工作區；遇 `diff --cc` 回 409 `unmerged_path`（memory `git-diff-unmerged-path-emits-combined-format`）。
  單元測試與腳本都沒涵蓋，是目視驗收逐一點過每種檔案狀態才抓到。
- Windows git 對 `\\wsl.localhost` repo 報 dubious ownership；本 change 不繞過（`-c safe.directory` 會讓 WSL repo 設定以 Windows 身分執行程式）。

實作與流程：

- **封閉性很容易被「為了測試方便」的公開入口破壞**：2.2 一度讓 `QueryPlan` 欄位與 `execute` 都 `pub`，crate 外可執行任意 argv。正解：正式入口只有
  `GitRunner::run<Q: GitQuery>`，任意 argv 入口限 `test-support` feature；證明方式是未開 feature 的暫時 consumer crate `cargo check`（doctest 會因
  dev-dependency 自我引用洩漏 feature 而失效）。
- **新增第三個列舉值時，舊的 `=== "某值"` 二選一判斷把新值歸錯邊**：左欄加「變更」後 `render.js` 仍 `leftTab === "files"`，重畫時 Project 清單蓋回來；
  切換當下被同步 DOM 操作遮住（memory `binary-check-misroutes-new-enum-value`）。
- `meta` 的 `blob` 一度用內容雜湊（整份讀取、違反 spec「物件 hash」）→ 新增封閉查詢 `BlobId`；只看副檔名分類 viewer 又違反 5a 規則 → `BlobHead`
  （上限 8192、可截斷）交給 `cockpit_files::classify_viewer_bytes`。
- 計時測試的逾時不能短於「系統忙碌時子程序啟動＋寫檔」的時間（`tests/runner.rs` 由 150 ms 改 2 s）。
- 既有 flaky（非本分支引入，交使用者決定是否修）：`cockpit/tests/app.rs` 的 `abort_await_is_bounded`、`visual-check.js` `DF1` 收尾的 chrome PID 確認。
- Codex：週限額可能在一晚內用盡；`--wait` 放背景 Bash 時提早返回（一律前景跑）。Claude API 的 session 上限也撞過一次（subagent 中斷，resume 即可）。
- 多個 subagent 同時改同一個工作樹：各自只 `git add <具體路徑>`，未提交的他人修改可能被夾帶進 commit（本段 ledger 那三行就是這樣進去的，內容無誤）。
- `codex-companion.mjs cancel` 在 Git Bash 下要 `MSYS_NO_PATHCONV=1`（`taskkill /PID` 被吃）；失聯 job 的 `cancel`／`result` 會回 "No job found"。

change 5a 與更早仍有效的坑：`git show 45f1174:docs/handover.md` 第 4 節（再往前的鏈結見該節末）。

## 5. 已定但未執行的決策

| 決策 | 狀態 |
|---|---|
| 路線：change 6 進度模型 → change 7 畫面與操作修補 → 去識別化 → 評估 Tauri | **已定** |
| 5b 以 Sonnet 5.5 驗收取代 Codex，記載「未經 Codex 審查」 | **使用者決定**（2026-09-30），僅限 5b |
| 5b 細節裁決（P1、P2、R1–R15） | **Claude 依授權裁決**，清單在 archive ledger，使用者可推翻 |
| 不做任何寫入 repo 的操作、不讀 stash、不做 unified diff／行內差異／語法上色／blame | **已定**（5b proposal 非目標） |
| deferred lows（5a 九項，見 `openspec/changes/archive/2026-09-28-file-review/sdd-ledger.md`）＋5b 延後清單 | **延後**，change 7 挑選 |
| `main` 上既有檔案含真實主機名／使用者名稱 | **未處理**：推 remote 前另開小 change |
| Codex stop review gate（per-repo）本 repo 未啟用 | 靠流程內手動跑 `adversarial-review` |
| Claude 在 feature 分支 commit，收尾 squash 併回 main | **已授權** |
| 不做 VS Code extension host；多 runtime 通用支援不做；前端 TypeScript 未定（建議 JSDoc＋`@ts-check`） | 同上一版 |

## 6. 之後的路

change 6 進度模型 → change 7 畫面與操作修補 → 推 remote 前的去識別化小 change → 評估 Tauri 桌面殼（ADR-0005）。
北極星見 `docs/cockpit-spec.md`。

## 版本紀錄

| 版本 | 日期 | 變更 |
|---|---|---|
| 1–10 | 2026-09-13～15 | change 1a／1b（詳見 git log 與 archive 目錄） |
| 11–14 | 2026-09-15～19 | change 2 propose／apply／驗收／併回 `main` 並 archive |
| 15–17 | 2026-09-21～23 | change 3 `live-output`：探測 → propose → SDD apply → 驗收 → 併回 `main` 並 archive |
| 18–20 | 2026-09-23～26 | change 4 `direction-01-visual`：brainstorming → propose → SDD → 驗收 → 併回並 archive |
| 21–23 | 2026-09-27～28 | change 5a `file-review`：brainstorming（拆 5a／5b）→ propose → SDD → 驗收 → 併回並 archive |
| 24–25 | 2026-09-28 | 小 change `html-charset` propose／apply／併回並 archive |
| 26 | 2026-09-29～30 | change 5b `git-review`：brainstorming → propose → SDD 實作、WSL 真機與目視驗收通過；Codex 週限額用盡 |
| 27 | 2026-10-01 | 5b 改由 Sonnet 5.5 分 6 批驗收（修 7 項，R13–R15）→ squash 併回 `main` 並 archive；未經 Codex 審查 |
