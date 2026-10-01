# 交接：下一段任務

> **建立日期**：2026-10-01　|　**上一段做完的事**：change 7 `ui-fixes` 全流程到「實作＋真機驗收＋全 gate」——brainstorming → propose →
> SDD 實作（task 1.1–5.2 全綠）；只剩 Codex 審查（等週限額）與收尾。
> **性質**：接手用文件，會過期，每段重寫。
> 為什麼做看 `docs/cockpit-spec.md`（北極星）與各 change 的 proposal；怎麼做看 `~/.claude/CLAUDE.md`（本 repo 精簡版在
> `AGENTS.md`）；規格看 `openspec/specs/`。

## 0. 三十秒版本

1. **active change＝`ui-fixes`**，分支 `feat/ui-fixes`（未併回 `main`）。進度一律現場跑 `openspec status --change ui-fixes`；
   紀錄在 `openspec/changes/ui-fixes/sdd-ledger.md`。
2. **2026-10-04 10:55 Codex 週限額恢復後**，依序跑 3.7 → 4.9 → 5.3 三次審查（第 2 節），處理 findings 後才能 5.4 收尾（squash 併回、archive）。
   在那之前不要 squash、不要併回；不要以其他審查取代（CLAUDE.md 一律 Codex，change 6 改用 Opus 是單次決定）。

## 1. 現在的狀態

- **`main`**：change 1a 到 6 與 `html-charset`，`504a687`。沒有 remote。
- **`feat/ui-fixes`**：起點 `0ec278e`（propose），實作範圍 `0ec278e..d883413`。內容（細節看 change 的 proposal／design）：
  - 斷線（瀏覽器與 cockpit 服務的通道）時右欄、中欄、左欄計數轉 `--text-dim` 並標「最後已知」；覆蓋造成的 `runtime_disconnected` 帶
    `source`，斷線期間可「取消改綁」。
  - 滑鼠觸發的焦點還原不畫焦點外框（`window.cockpitFocus.focus`，在 `cockpit/assets/app/output.js`），鍵盤照常。
  - Git 頁：refs 每筆多 `commit` 布林、非 commit tag 不當預設起點也不誤報「分支已變更」；git 子行程清 15 個 repo 區域環境變數並
    `LC_ALL=C`；搜尋後分批載入不拉回；檔案清單截斷提示；詳情重建焦點保留。
  - agent 回報：撞號涵蓋 `Bound` 孤兒 pane；v1 狀態檔 `"active": null` 視為損毀。
- **可用指令**（repo 根目錄）：
  - 全 gate：`cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test --workspace && cargo test -p cockpit --example ui_preview && markdownlint-cli2 "**/*.md" && openspec validate --all`
  - 預覽：`cargo run -p cockpit --example ui_preview`（`127.0.0.1:7770`；需要本機安裝 git）
  - 驗收腳本（不可並行、一律前景跑）：既有 10 支（清單見 `openspec/changes/ui-fixes/tasks.md` 通則）＋
    `node docs/research/2026-10-01/ui-fixes-check.js`。`factory-floor-check.js` 跑完 `git checkout -- docs/research/2026-09-16/task-5.2-scenario-d.png`。
  - WSL 版 git 整合測試（opt-in）：PowerShell `$env:COCKPIT_GIT_TEST_WSL_DISTRO="Ubuntu-24.04"; cargo test -p cockpit-git -- --ignored`
- **測試數字**（2026-10-01、`d883413`，當場跑為準）：workspace 1098 passed／0 failed／12 ignored；ui_preview 40；11 支腳本全 PASS；
  markdownlint 133 files 0 issues；`openspec validate --all` 20 passed。
- 真機驗收紀錄：`docs/research/2026-10-01/ui-fixes-live.md`（含未能真機驗證的項目與原因）。

## 2. 立刻要做（2026-10-04 10:55 起）：Codex 審查 3.7 → 4.9 → 5.3

指令（Bash，前景；focus 不放反引號）：

```bash
node ~/.claude/plugins/cache/openai-codex/codex/1.0.6/scripts/codex-companion.mjs adversarial-review --wait --base main "<focus>"
```

- 3.7 focus：`cockpit-core`、`cockpit-git`、`cockpit` 後端（tasks 第 2、3 節；commits `0ec278e..6bb5d71`）。
- 4.9 focus：`cockpit/assets/`、`cockpit/examples/ui_preview.rs`、驗收腳本（第 4 節；`6bb5d71..f22f029`）。
- 5.3：整支分支，並請它檢視 ledger 的 `minor (deferred)` 與 `Ruling:` 行。
- 判讀：只有輸出最後的「# Codex Adversarial Review」結論段才算數；`grep "^# Codex"` 為 0 就重跑（開頭的 verdict 可能是占位）。
  沙箱唯讀，cargo 跑不了，findings 是靜態推導——每條先實測重現再採信，處理依 superpowers:receiving-code-review，結果寫進 ledger。
- 修正照 SDD：派 fresh sonnet 實作者（合約與 brief 工具在 git-ignored 的 `.superpowers/sdd/tasks-ui-fixes/`：`contract.md`、`mkbrief.sh`）。
- 全部通過後 5.4：squash 併回 `main`（已授權）→ `/opsx:archive` → 重寫本檔。

## 3. 這一段踩過的坑

（**不會報錯的錯誤**加粗。）

- **Chrome 把「滑鼠操作後的程式焦點」當成 `:focus-visible`**：只要那次滑鼠操作沒讓任何元素被滑鼠聚焦（我們在 `pointerdown` 呼叫了
  `preventDefault`），之後的程式 `focus()` 就會畫外框。正解：依最後一次輸入方式傳 `focusVisible: false`（Chrome 154 有效）；
  輸入方式監聽要掛 `document` capture，才會早於 `#app` 上會同步重畫的委派。
- **`Option<Option<T>>` 只加 `deserialize_with` 時，欄位缺席會報 `missing field`**——要 `#[serde(default, deserialize_with = …)]`。
- **前端複算後端規則（`computeExpectedTips`）時，後端規則一改，前端就靜默誤報**；改後端起點規則時，要一起改前端的複算。
- **`live-output-check.js` R 段偶發 FAIL**：腳本分兩次 `eval` 先 focus 再比對節點，中間的推送重畫會換掉節點。屬腳本競態，重跑即綠
  （ledger 5.2 有修法）。`factory-floor-check.js` 也偶發過一次左欄為空的 FAIL，重跑即綠。
- 5b 延後清單說「tag 指向 tree 會讓整個 Graph 失敗」，在本機 git 2.50.1 沒有重現（`/log` 回 200）；排除仍保留，因為前端偵測需要一致。
- SDD 的 `scripts/task-brief` 不認 OpenSpec 的 checkbox `tasks.md`，改用工作區的 `mkbrief.sh`；per-task 不派 Claude reviewer，改由控制端核對
  證據＋Codex 分階段審查（ledger 前置裁決）。

change 6 與更早仍有效的坑：`git show 504a687:docs/handover.md` 第 3 節（再往前的鏈結見該節末）。

## 4. 已定但未執行的決策

| 決策 | 狀態 |
|---|---|
| 路線：change 7 畫面與操作修補 → change 8 Live Output 上色 → 去識別化 → 評估 Tauri | **已定**（2026-10-01 使用者把上色移出 change 7） |
| change 7 範圍：斷線與焦點、Git 頁小修、進度回報小修 | **使用者決定**（2026-10-01） |
| 斷線轉暗涵蓋全頁（右欄＋中欄＋左欄計數），不只 runtime 卡 | **使用者決定**（2026-10-01） |
| change 7 細節裁決（ledger 內全部 `Ruling:`，例如左欄「警告 N」斷線時不轉暗） | **Claude 依授權裁決**，使用者可推翻 |
| change 6 未經 Codex（Opus 取代）、5b 未經 Codex（Sonnet 驗收） | 要不要補審（5b `45f1174..427118b`、6 `e04644f..37005c1`）由使用者決定 |
| 延後：Graph 起點過多超過命令列上限、refs 超過 1 MiB、git 逾時只殺轉手程式、WSL 內 agent 回報、自訂 `command` 橋接 WSL、5a／5b 其餘 lows | **延後** |
| v2 狀態檔未知 project 缺 `active` 啟動失敗 | **決定不修**（change 6 裁決） |
| `main` 上既有檔案含真實主機名／使用者名稱 | **未處理**：推 remote 前另開小 change |
| Codex stop review gate（per-repo）本 repo 未啟用 | 靠流程內手動跑 `adversarial-review` |
| Claude 在 feature 分支 commit，收尾 squash 併回 main | **已授權** |

## 5. 之後的路

change 7 審查與收尾 → change 8 Live Output 上色（先探測 Windows 0.9.x 的 `format=ansi` 實際輸出，設計文件 §2.6 只驗過 WSL 0.8.2 的三種 SGR）
→ 推 remote 前的去識別化小 change → 評估 Tauri 桌面殼（ADR-0005）。

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
| 28–29 | 2026-10-01 | change 6 `progress-model`（Opus 審查取代 Codex）併回並 archive |
| 30 | 2026-10-01 | change 7 `ui-fixes`：propose → SDD 實作 → 真機驗收 → 全 gate；待 Codex 審查（10-04 起）與收尾 |
