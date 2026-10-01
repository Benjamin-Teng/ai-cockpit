# 交接：下一段任務

> **建立日期**：2026-10-01　|　**上一段做完的事**：change 6 `progress-model` 全流程完成——brainstorming → propose → SDD 實作 → 真機驗收 →
> Opus 5.5 整支分支審查（使用者決定取代 Codex）＋修 5 項＋複審 → squash 併回 `main` → archive。
> **性質**：接手用文件，會過期，每段重寫。
> 為什麼做看 `docs/cockpit-spec.md`（北極星）與各 change 的 proposal；怎麼做看 `~/.claude/CLAUDE.md`（本 repo 精簡版在
> `AGENTS.md`）；規格看 `openspec/specs/`。

## 0. 三十秒版本

1. **沒有 active change**，`main` 乾淨、無 feature 分支。下一段＝**change 7「畫面與操作修補」brainstorming**（第 2 節）。
2. change 6 未經 Codex、改由 Opus 5.5 審查是使用者的單次決定，**不延伸到其他 change**。Codex 恢復後（2026-10-04 10:55 起）
   要不要補審 change 6（範圍 `e04644f..37005c1`）或 5b（`45f1174..427118b`），由使用者決定。

## 1. 現在的狀態

- **`main`**：change 1a／1b／2／3／4／5a／5b／6 與小 change `html-charset`。本段 commit：`37005c1`（change 6 squash）、`95247b7`（archive）、本檔。
  沒有 remote。
- change 6 內容：agent 以 `X-Herdr-Pane-Id: $HERDR_PANE_ID` 呼叫 `/api/agent/...`（`cockpit/src/agent.rs`）回報「目前 task」與推進；
  每條 workstream 只有目前 task 會隨 agent 狀態呈 running／blocked，未宣告時列首提示；人工「退回」；狀態檔 version 2。
  使用說明（含可貼進其他專案 `AGENTS.md` 的短文）在 `cockpit/README.md`「agent 回報進度」。規格 `openspec/specs/agent-reporting/` 等；
  紀錄 `openspec/changes/archive/2026-10-01-progress-model/sdd-ledger.md`（全部 Ruling、Opus 審查與延後項）。
- **可用指令**（repo 根目錄）：
  - 全 gate：`cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test --workspace && cargo test -p cockpit --example ui_preview && markdownlint-cli2 "**/*.md" && openspec validate --all`
  - 預覽：`cargo run -p cockpit --example ui_preview`（`127.0.0.1:7770`；需要本機安裝 git）
  - 驗收腳本（不可並行、一律前景跑）：既有 9 支（清單見 archive `2026-10-01-progress-model/tasks.md` 通則）＋
    `node docs/research/2026-10-01/progress-check.js`。`factory-floor-check.js` 跑完 `git checkout -- docs/research/2026-09-16/task-5.2-scenario-d.png`。
  - WSL 版 git 整合測試（opt-in）：PowerShell `$env:COCKPIT_GIT_TEST_WSL_DISTRO="Ubuntu-24.04"; cargo test -p cockpit-git -- --ignored`
- **測試數字**（2026-10-01，當場跑為準）：workspace 1083 passed／0 failed／12 ignored；ui_preview 39；10 支腳本全 PASS；
  markdownlint 121 files 0 issues；`openspec validate --all` 19 passed。

## 2. 立刻要做：change 7「畫面與操作修補」brainstorming

- 既有：Live Output 上色；M2（斷線期間無法取消改綁）；U1（滑鼠點擊後殘留焦點框，改 `:focus-visible`）；U2（服務斷線時右欄 runtime 卡仍綠色）。
- 5b 延後清單（archive `2026-10-01-git-review/sdd-ledger.md`「Sonnet 驗收」節），優先：Graph 起點全放 argv 超過命令列上限、tag 指向非 commit、
  逾時只殺轉手程式、未清繼承的 `GIT_DIR`、前端搜尋分批載入拉回命中列等。
- change 6 延後（archive `2026-10-01-progress-model/sdd-ledger.md`）：Opus 複審 N1（撞號只數投影 pane 樹，孤兒 pane 漏判，約一行）、N2（`stale_pane_start_right_after_rebind_is_403` 分不出鎖內路徑）、M5（v2 未知 project 缺 `active` 啟動失敗，裁決不修）；**WSL 內的 agent 回報**（WSL2 NAT 下連不到 Windows 的 127.0.0.1；選項：`.wslconfig` mirrored 網路，或 Cockpit 另聽 WSL
  虛擬網卡＋重設計來源檢查）；v1 狀態檔手寫 `"active": null` 會被放行（極端情況）；以自訂 `command` 端點橋接 WSL 的 runtime 被當成 Windows 端。
- 5b 是否補 Codex 審（範圍 `45f1174..427118b`）由使用者決定。

流程：`git switch -c feat/<slug>` → superpowers:brainstorming → `/opsx:propose`（`tasks.md` 開頭寫執行路徑分流）→ apply → 審查 → 併回。
審查：Codex 週限額 2026-10-04 10:55 恢復後照 CLAUDE.md 一律 Codex；改用 Opus 取代是使用者對 change 6 的單次決定。

## 3. 這一段踩過的坑

（**不會報錯的錯誤**加粗。）

- **真機驗收時，跑驗收的 pane 本身 agent_status 恆為 idle**：在 HERDR pane 裡由 Claude 派出的 subagent 執行 curl 時，`herdr pane get` 連續取樣都是
  idle，所以真機沒觀察到 running 與 `activity_undeclared=true`（由整合測試與 `progress-check.js` 覆蓋）。要真機看 running，得在另一個有 agent
  正在輸出的 pane 裡跑 curl。原因未查證。
- SDD skill 的 `scripts/task-brief` 只認 `### Task N` 標題，對 OpenSpec 的 checkbox `tasks.md` 找不到 task；本段改用工作區內自寫的 `mkbrief.sh`。
- SDD skill 預設每個 task 派 Claude reviewer，與全域「diff 審查一律 Codex」衝突；本段裁決為控制端核對證據＋Codex 分階段審查（ledger 前置裁決）。
- 驗收腳本放背景跑會被時限殺掉、留下孤兒 node／ui_preview（本段發生一次，已收掉）——腳本一律前景跑。
- `markdownlint-cli2` 的 MD041：OpenSpec delta spec、proposal、design 第一行要有 H1（比照 archive 內的 `# <capability>（delta）`）。

5b 與更早仍有效的坑：`git show e04644f:docs/handover.md` 第 4 節（再往前的鏈結見該節末）。

## 4. 已定但未執行的決策

| 決策 | 狀態 |
|---|---|
| 路線：change 6 → change 7 畫面與操作修補 → 去識別化 → 評估 Tauri | **已定** |
| change 6 以 Opus 5.5 取代 Codex 審查 | **使用者決定**（2026-10-01），僅限 change 6 |
| change 6 細節裁決（ledger 內全部 `Ruling:`） | **Claude 依授權裁決**，使用者可推翻 |
| 5b 以 Sonnet 驗收取代 Codex | 使用者決定（2026-09-30），僅限 5b |
| deferred lows（5a 九項、5b 延後清單、change 6 延後：WSL 回報、N1、N2、M5 等） | **延後**，change 7 挑選 |
| `main` 上既有檔案含真實主機名／使用者名稱 | **未處理**：推 remote 前另開小 change |
| Codex stop review gate（per-repo）本 repo 未啟用 | 靠流程內手動跑 `adversarial-review` |
| Claude 在 feature 分支 commit，收尾 squash 併回 main | **已授權** |

## 5. 之後的路

change 7 畫面與操作修補 → 推 remote 前的去識別化小 change → 評估 Tauri 桌面殼（ADR-0005）。

## 版本紀錄

| 版本 | 日期 | 變更 |
|---|---|---|
| 1–10 | 2026-09-13～15 | change 1a／1b |
| 11–14 | 2026-09-15～19 | change 2 |
| 15–17 | 2026-09-21～23 | change 3 `live-output` |
| 18–20 | 2026-09-23～26 | change 4 `direction-01-visual` |
| 21–23 | 2026-09-27～28 | change 5a `file-review` |
| 24–25 | 2026-09-28 | 小 change `html-charset` |
| 26–27 | 2026-09-29～10-01 | change 5b `git-review`（Sonnet 驗收、未經 Codex）併回並 archive |
| 28 | 2026-10-01 | change 6 `progress-model`：brainstorming → propose → SDD 實作 → 真機驗收 |
| 29 | 2026-10-01 | change 6 經 Opus 5.5 審查（取代 Codex）＋修正＋複審 → squash 併回 `main` 並 archive |
