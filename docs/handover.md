# 交接：下一段任務

> **建立日期**：2026-10-01　|　**上一段做完的事**：change 6 `progress-model` 從 brainstorming 到 propose、SDD 實作、真機驗收全部完成，
> 停在**等 Codex 審查**（週限額用盡，2026-10-04 10:55 恢復）。**尚未併回 `main`**。
> **性質**：接手用文件，會過期，每段重寫。
> 為什麼做看 `docs/cockpit-spec.md`（北極星）與各 change 的 proposal；怎麼做看 `~/.claude/CLAUDE.md`（本 repo 精簡版在
> `AGENTS.md`）；規格看 `openspec/specs/` 與 `openspec/changes/progress-model/specs/`。

## 0. 三十秒版本

1. **2026-10-04 10:55 之後跑 Codex 審查**，範圍是整支 `feat/progress-model`（第 2 節）。審查前 change 6 不算完成，不得併回、不得 archive。
2. 審查通過 → squash 併回 `main` → archive（delta spec 同步進主規格）→ 重寫本檔。之後是 change 7（第 3 節）。

## 1. 現在的狀態

- **分支 `feat/progress-model`**（從 `main` 的 `e04644f` 分出，約 30 個 commit），**工作樹乾淨**。`main` 本段沒動。沒有 remote。
- change 6 內容（使用者 2026-10-01 逐項選定，裁決全在 `openspec/changes/progress-model/sdd-ledger.md`）：
  - agent 以 `X-Herdr-Pane-Id: $HERDR_PANE_ID` 呼叫 `GET /api/agent/tasks`、`POST /api/agent/projects/{p}/tasks/{t}/{start|advance}`；
    只能動綁到該 pane 的 workstream 底下的 task；只認非 WSL runtime（實作：`cockpit/src/agent.rs`）。
  - 每條 workstream 一個「目前 task」：只有它會因 agent 狀態呈 running／blocked；沒宣告時全部 ready，列首顯示「工作中・未宣告 task」。
  - 人工新增「退回」（`retreat`），必須先清除標記、第一站不能退。agent 不能標完成／失敗／退回。
  - 狀態檔升 `version: 2`（多 `active`），v1 照常讀；回退方法在 `cockpit/README.md`。
  - 使用說明（含可貼進其他專案 `AGENTS.md` 的短文）：`cockpit/README.md`「agent 回報進度」。
- **可用指令**（repo 根目錄）：
  - 全 gate：`cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test --workspace && cargo test -p cockpit --example ui_preview && markdownlint-cli2 "**/*.md" && openspec validate --all`
  - 預覽：`cargo run -p cockpit --example ui_preview`（`127.0.0.1:7770`）；新情境 Project「Scenario D Demo」的 `Undeclared` 列與 `Retreat target`。
  - 驗收腳本（不可並行、一律前景跑）：既有 9 支（清單見 `openspec/changes/progress-model/tasks.md` 通則）＋新的
    `node docs/research/2026-10-01/progress-check.js`（用法 `progress-check.md`）。`factory-floor-check.js` 跑完 `git checkout -- docs/research/2026-09-16/task-5.2-scenario-d.png`。
- **測試數字**（2026-10-01，當場跑為準）：workspace 1073 passed／0 failed／12 ignored；ui_preview 39；腳本 10 支全 PASS；
  markdownlint 120 files 0 issues；`openspec validate --all` 19 passed。
- 真機紀錄：`docs/research/2026-10-01/agent-report-live.md`。

## 2. 立刻要做：Codex 審查（2026-10-04 10:55 之後）

1. `git switch feat/progress-model`，先跑一次全 gate 確認還是綠的。
2. 前景跑（不要背景，`--wait` 在背景會提早返回）：

   ```bash
   node ~/.claude/plugins/cache/openai-codex/codex/<最新版>/scripts/codex-companion.mjs adversarial-review --wait --base main "<focus>"
   ```

   focus 建議：交易原子性（先落檔再生效、單鎖、取消安全）、目前 task 的清除規則、v1/v2 相容、agent 端點的錯誤碼順序與 pane 身分繞過、
   前端退回按鈕與未宣告提示在整頁重畫下的正確性。上次的 focus 原文在 `.superpowers/sdd/tasks-progress-model/codex-3.5.md` 旁的 ledger 紀錄。
3. 判讀：只有輸出最後的「# Codex Adversarial Review」結論段算數（memory `codex-review-verdict-only-valid-in-final-section`）。
   findings 一律實測重現後才採信（唯讀沙箱，它只能靜態推導）。
4. 修正派 subagent，修完再審一次。ledger 的 3.5、4.4、5.3 依結果勾選（三次審查可合併成一次整支分支審查，ledger 註明）。
5. 通過後：squash 併回 `main` → `/opsx:archive progress-model` → 刪 `feat/progress-model` 與 `.superpowers/sdd/tasks-progress-model/` → 重寫本檔。

## 3. 接著要做：change 7「畫面與操作修補」

- 既有：Live Output 上色；M2（斷線期間無法取消改綁）；U1（滑鼠點擊後殘留焦點框，改 `:focus-visible`）；U2（服務斷線時右欄 runtime 卡仍綠色）。
- 5b 延後清單（archive `2026-10-01-git-review/sdd-ledger.md`「Sonnet 驗收」節），優先：Graph 起點全放 argv 超過命令列上限、tag 指向非 commit、
  逾時只殺轉手程式、未清繼承的 `GIT_DIR`、前端搜尋分批載入拉回命中列等。
- change 6 延後：**WSL 內的 agent 回報**（WSL2 NAT 下連不到 Windows 的 127.0.0.1；選項：`.wslconfig` mirrored 網路，或 Cockpit 另聽 WSL
  虛擬網卡＋重設計來源檢查）；v1 狀態檔手寫 `"active": null` 會被放行（極端情況）；以自訂 `command` 端點橋接 WSL 的 runtime 被當成 Windows 端。
- 5b 是否補 Codex 審（範圍 `45f1174..427118b`）由使用者決定。

## 4. 這一段踩過的坑

（**不會報錯的錯誤**加粗。）

- **真機驗收時，跑驗收的 pane 本身 agent_status 恆為 idle**：在 HERDR pane 裡由 Claude 派出的 subagent 執行 curl 時，`herdr pane get` 連續取樣都是
  idle，所以真機沒觀察到 running 與 `activity_undeclared=true`（由整合測試與 `progress-check.js` 覆蓋）。要真機看 running，得在另一個有 agent
  正在輸出的 pane 裡跑 curl。原因未查證。
- SDD skill 的 `scripts/task-brief` 只認 `### Task N` 標題，對 OpenSpec 的 checkbox `tasks.md` 找不到 task；本段改用工作區內自寫的 `mkbrief.sh`。
- SDD skill 預設每個 task 派 Claude reviewer，與全域「diff 審查一律 Codex」衝突；本段裁決為控制端核對證據＋Codex 分階段審查（ledger 前置裁決）。
- 驗收腳本放背景跑會被時限殺掉、留下孤兒 node／ui_preview（本段發生一次，已收掉）——腳本一律前景跑。
- `markdownlint-cli2` 的 MD041：OpenSpec delta spec、proposal、design 第一行要有 H1（比照 archive 內的 `# <capability>（delta）`）。

5b 與更早仍有效的坑：`git show e04644f:docs/handover.md` 第 4 節（再往前的鏈結見該節末）。

## 5. 已定但未執行的決策

| 決策 | 狀態 |
|---|---|
| 路線：change 6 → change 7 畫面與操作修補 → 去識別化 → 評估 Tauri | **已定** |
| change 6 審查一律 Codex，不以 Claude subagent 取代；通過前不併回 | **已定**（5b 的豁免不延伸） |
| change 6 細節裁決（ledger 內全部 `Ruling:`） | **Claude 依授權裁決**，使用者可推翻 |
| 5b 以 Sonnet 驗收取代 Codex | 使用者決定（2026-09-30），僅限 5b |
| deferred lows（5a 九項、5b 延後清單、change 6 延後） | **延後**，change 7 挑選 |
| `main` 上既有檔案含真實主機名／使用者名稱 | **未處理**：推 remote 前另開小 change |
| Codex stop review gate（per-repo）本 repo 未啟用 | 靠流程內手動跑 `adversarial-review` |
| Claude 在 feature 分支 commit，收尾 squash 併回 main | **已授權** |

## 6. 之後的路

change 6 Codex 審查與併回 → change 7 畫面與操作修補 → 推 remote 前的去識別化小 change → 評估 Tauri 桌面殼（ADR-0005）。

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
| 28 | 2026-10-01 | change 6 `progress-model`：brainstorming → propose → SDD 實作 → 真機驗收；等 Codex（10-04 恢復），未併回 |
