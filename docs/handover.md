# 交接：下一段任務

> **建立日期**：2026-09-29　|　**上一段做完的事**：change 5b `git-review`（git 唯讀層、diff、Git Graph）brainstorming →
> propose → SDD 實作全部 task 完成、全 gate 綠、WSL 真機驗收通過；**全部尚未經 Codex 審查**（Codex 週限額用盡）。
> **性質**：接手用文件，會過期，每段重寫。
> 為什麼做看 `docs/cockpit-spec.md`（北極星）與各 change 的 proposal；怎麼做看 `~/.claude/CLAUDE.md`（本 repo 精簡版在
> `AGENTS.md`）；規格看 `openspec/specs/` 與 `openspec/changes/git-review/specs/`。

## 0. 三十秒版本

1. **active change＝`git-review`，分支 `feat/git-review`（尚未併回 `main`）**。程式碼、測試、文件都完成；**目視驗收已通過**（使用者
   2026-09-30 授權由 Claude 判定，過程抓到並修好 V1 衝突檔 diff 502、V2 刪除檔工具列未停用）。但 `tasks.md` 一格都沒勾：依規則，
   沒經 Codex 通過的 task 不勾選、不宣稱完成。
2. **Codex 用量上限要到 2026-10-04 10:55 才恢復**（週限額）。恢復後依第 2.2 節的順序補審，這是併回前唯一剩下的關卡。
   若不想等，改用其他審查方式要由使用者決定（CLAUDE.md 規定 diff 審查一律 Codex，Claude 不得自行改派 Claude subagent 代審）。
3. brainstorming 期間使用者授權「這次 change 全部給 Claude 採納，second opinion 給 Codex」，所以範圍以外的細節都由 Claude 裁決，
   裁決清單在 `openspec/changes/git-review/sdd-ledger.md`（搜尋 `Ruling`），使用者可逐條推翻。

## 1. 現在的狀態

- **`main`**：change 1a／1b／2／3／4／5a 與小 change `html-charset`（見上一版 `git show 45f1174:docs/handover.md`）。
- **`feat/git-review`**（起點 `45f1174`，62 個 commit）：
  - 新 crate `cockpit-git`（ADR `docs/adr/0007-cockpit-git-crate.md`）：sealed `GitQuery` 封閉查詢、`GitTarget`（Windows repo 走
    `git -C`；`\\wsl.localhost\<distro>\…` 的 repo 走 `wsl.exe -d <distro> --exec env LC_ALL=C git -C <posix>`）、`GitRunner`（並行 4、
    含排隊 10 秒逾時、各查詢輸出上限、錯誤分類）、解析器、單檔 diff 左右並排對齊、Graph 排版（前綴穩定）。
  - `cockpit` 後端 `cockpit/src/git.rs`：`/api/git/<runtime>/<root_id>/{status,refs,log,commit/<hash>,changes,diff,merge-base,
    meta/<rev>/…,blob/<rev>/…,render/<rev>/…}`，說明在 `cockpit/README.md`「git 唯讀讀取」。
  - 前端 `cockpit/assets/app/git.js`：左欄「變更」分頁、diff 分頁（左右並排）、Git Graph 分頁（篩選、搜尋、分批載入、commit 詳情、
    比較、複製）、某版本檔案分頁；`files.js` 分頁一般化為帶 `kind` 的框架，本機儲存 `cockpit.fileTabs` 升 `v:2`（相容 `v:1`）。
  - `ui_preview` 的 `review-repo`／`other-repo` 改為真正的 git repo（fast-import 264 commit；`other-repo` 是進行中的衝突 merge）。
- **可用指令**（repo 根目錄）：
  - 全 gate：`cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test --workspace && cargo test -p cockpit --example ui_preview && markdownlint-cli2 "**/*.md" && openspec validate --all`
  - 預覽：`cargo run -p cockpit --example ui_preview`（`127.0.0.1:7770`；需要本機安裝 git）
  - 驗收腳本（不可並行）：`node docs/research/2026-09-28/git-check.js [段代號,...]`（用法 `git-check.md`）、
    `docs/research/2026-09-27/files-check.js`、`docs/research/2026-09-23/visual-check.js`，以及既有六支（清單見 `openspec/changes/git-review/tasks.md` 通則）。
  - WSL 版 git 整合測試（opt-in）：PowerShell `$env:COCKPIT_GIT_TEST_WSL_DISTRO="Ubuntu-24.04"; cargo test -p cockpit-git -- --ignored`
- **測試數字**（2026-09-30，`232c293`，當場跑為準）：`cargo test --workspace` 997 passed／0 failed／12 ignored；`ui_preview` 38；
  `git-check.js` 21 段、`files-check.js` 29 段、`visual-check.js` 29 段（以各腳本的段落清單計）、其餘六支全 PASS；`markdownlint-cli2` 108 files 0 issues；
  `openspec validate --all` 18 passed。
- **SDD 紀錄**：`openspec/changes/git-review/sdd-ledger.md`（基線、每個 task 的 commit 範圍、fix round、裁決、deferred minors、事故）。
  暫存的 brief／report 在 `.superpowers/sdd/tasks/`（git 忽略，本機才有）。

## 2. 立刻要做

### 2.1 目視驗收（已完成，2026-09-30 由 Claude 依授權判定通過）

19 步 headless 截圖逐張判定，紀錄在 ledger「目視驗收」節。若使用者想自己再看一次：

`cargo run -p cockpit --example ui_preview` 後開 `http://127.0.0.1:7770/`，選 `wJ:p4`（review-repo）：

- 左欄「變更」：看得到 `review-repo`、分支 `main`、已暫存 1／變更 2／未追蹤 1。點檔案開 diff 分頁，左右並排、紅綠底色、行號。
- 「Git Graph」：264 個 commit 分批載入、`feature/formatting` 合併回 main（`feature/logging` 刻意不合併，供分支篩選情境）、`main`／`origin/main`／`v0.1.0` 標籤；點 merge commit 展開詳情；
  「選為比較基準」後點另一個 commit 看比較；「看此版本」開某版本分頁。
- 截圖參考（本機 scratchpad，session 結束可能被清掉）：`shots-5.1\`、`shots-5.2\`（WSL 真機）。

### 2.2 Codex 補審（2026-10-04 10:55 之後）

**一律前景執行**（背景 Bash 執行時 `--wait` 曾提早返回、job 失聯，原因未證實），只採信 log 最後的「# Codex Adversarial Review」段。
每一批先在 scratchpad 建 detached worktree 固定在該批終點，再以起點為 `--base`：

```bash
git worktree add --detach <scratchpad>/rv <終點>
cd <scratchpad>/rv && node ~/.claude/plugins/cache/openai-codex/codex/<最新版號>/scripts/codex-companion.mjs adversarial-review --wait --base <起點> "<focus>"
```

| 批次 | 起點 → 終點 | 內容 |
|---|---|---|
| 1 | `main` → `44eb00e` | propose artifacts＋probe（純文件；focus 見 ledger「Codex 可用性」段的原提示） |
| 2 | `44eb00e` → `db5c075` | task 2.1–2.6（`cockpit-git`，含 revert 掉的誤提交） |
| 3 | `db5c075` → `0083632` | task 3.1–3.3（ui_preview fixture、git 端點；注意 Ruling P2：`cockpit/src/git.rs` 不得有 `Command::new`） |
| 4 | `0083632` → `34ceba5` | task 4.1–4.5（前端） |
| 4b | `7003b5a` → `232c293` | 目視驗收修正 V1／V2（Ruling R11、R12；含 spec／design 修改） |
| 5 | `main` → `HEAD` | 整支分支（task 5.4），帶 ledger 的 deferred minors 與 Ruling 清單請它判斷哪些須在併回前修 |

findings 先實測重現才採信（CLAUDE.md），修正依 SDD fix round 流程；每批通過才在 `tasks.md` 勾對應 task、在 ledger 補一行。
全部通過後：squash 併回 `main`（已授權）→ `/opsx:archive`（3 份 delta spec 同步進主規格）→ 重寫本檔。

## 3. 接著要做（依序）

### change 6「進度模型」（使用者 2026-09-26 定案）

- 進度目前全靠人工按按鈕。方向：agent 呼叫 cockpit 的 API 回報進度，寫入 cockpit 自己的狀態檔，**不需要推翻「對 HERDR 唯讀」**；
  「HERDR 的 `done` 不等於 task 完成」保留。
- 活動狀態目前只到 workstream 層級（見 `CONTEXT.md`），要精確到 task；推進要有「退回」操作。

### change 7「畫面與操作修補」

- Live Output 上色；M2（斷線期間無法取消改綁，`ProjectedBinding::RuntimeDisconnected` 不帶 `source`）；U1（滑鼠點擊後殘留冰青焦點框，
  改 `:focus-visible`，確認鍵盤焦點還原不退步）；U2（服務斷線時右欄 runtime 卡仍綠色 connected）。
- 本段新增候選：Git Graph 詳情展開處車道線中斷；diff 行尾 `\r` 處理不一致（ledger deferred minors）。

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
- Codex：週限額可能在一晚內用盡；`--wait` 放背景 Bash 時提早返回（見 2.2）。Claude API 的 session 上限也撞過一次（subagent 中斷，resume 即可）。
- `codex-companion.mjs cancel` 在 Git Bash 下要 `MSYS_NO_PATHCONV=1`（`taskkill /PID` 被吃）；失聯 job 的 `cancel`／`result` 會回 "No job found"。

change 5a 與更早仍有效的坑：`git show 45f1174:docs/handover.md` 第 4 節（再往前的鏈結見該節末）。

## 5. 已定但未執行的決策

| 決策 | 狀態 |
|---|---|
| 路線：5b `git-review`（實作完、待審）→ change 6 進度模型 → change 7 畫面與操作修補 → 去識別化 → 評估 Tauri | **已定** |
| 5b 範圍：Git Graph 只看＋安全操作（複製、看某版本、搜尋、跳檔案）；四種 diff；只做左右並排；左欄「變更」分頁；全部分支＋篩選；不拆 change | **使用者定案**（2026-09-28） |
| git 執行方式：WSL repo 在 WSL 內跑 git（`--exec`），不用 `-c safe.directory`、不用 gitoxide | **使用者定案**（2026-09-28） |
| 本 change 其餘細節（P1、P2、R1–R12） | **Claude 依授權裁決**，清單在 ledger，使用者可推翻 |
| 不做任何寫入 repo 的操作、不讀 stash、不做 unified diff／行內差異／語法上色／blame | **已定**（proposal 非目標） |
| Codex 恢復前不以 Claude subagent 代審 diff | **已定**（CLAUDE.md；使用者若要改用其他方式需明示） |
| deferred lows（5a 的 405 缺 `Allow` 等九項，見 `openspec/changes/archive/2026-09-28-file-review/sdd-ledger.md`）＋本段 deferred minors（ledger） | **延後** |
| `main` 上既有檔案含真實主機名／使用者名稱 | **未處理**：推 remote 前另開小 change |
| Codex stop review gate（per-repo）本 repo 未啟用 | 靠流程內手動跑 `adversarial-review` |
| Claude 在 feature 分支 commit，收尾 squash 併回 main | **已授權** |
| 不做 VS Code extension host；多 runtime 通用支援不做；前端 TypeScript 未定（建議 JSDoc＋`@ts-check`） | 同上一版 |

## 6. 之後的路

5b 審查併回 → change 6 進度模型 → change 7 畫面與操作修補 → 推 remote 前的去識別化小 change → 評估 Tauri 桌面殼（ADR-0005）。
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
| 26 | 2026-09-29～30 | change 5b `git-review`：brainstorming → propose → SDD 全部 task 實作完成、全 gate 綠、WSL 真機驗收與目視驗收通過（修 V1／V2）；Codex 週限額用盡，全部待審 |
