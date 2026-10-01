# 交接：下一段任務

> **建立日期**：2026-10-01　|　**上一段做完的事**：change 7 `ui-fixes` 全流程完成——brainstorming → propose → SDD 實作 → 真機驗收 →
> Opus 5.5 審查（使用者決定取代 Codex）＋兩波修正＋兩次複審 → squash 併回 `main` → archive。
> **性質**：接手用文件，會過期，每段重寫。
> 為什麼做看 `docs/cockpit-spec.md`（北極星）與各 change 的 proposal；怎麼做看 `~/.claude/CLAUDE.md`（本 repo 精簡版在
> `AGENTS.md`）；規格看 `openspec/specs/`。

## 0. 三十秒版本

1. **沒有 active change**，`main` 乾淨、無 feature 分支。下一段＝**change 8「Live Output 上色」brainstorming**（第 2 節）。
2. change 7 以 Opus 取代 Codex 是使用者對 change 7 的決定，**不延伸到其他 change**；change 8 起照 CLAUDE.md 一律 Codex（週限額
   2026-10-04 10:55 恢復）。

## 1. 現在的狀態

- **`main`**：change 1a／1b／2／3／4／5a／5b／6／7 與小 change `html-charset`。本段 commit：`0632fa8`（change 7 squash）、`e1a3b73`（archive）、
  本檔。沒有 remote。
- change 7 內容與紀錄：archive `openspec/changes/archive/2026-10-01-ui-fixes/`（proposal、design、`sdd-ledger.md` 含全部 Ruling、三份 Opus
  審查摘要與延後項）；真機驗收 `docs/research/2026-10-01/ui-fixes-live.md`。
- **可用指令**（repo 根目錄）：
  - 全 gate：`cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test --workspace && cargo test -p cockpit --example ui_preview && markdownlint-cli2 "**/*.md" && openspec validate --all`
  - 預覽：`cargo run -p cockpit --example ui_preview`（`127.0.0.1:7770`；需要本機安裝 git）
  - 驗收腳本（不可並行、一律前景跑）：11 支——`docs/research/2026-09-15/reconnect-check.js`、`whatever-check.js`、
    `docs/research/2026-09-16/actions-check.js`、`channel-backoff-check.js`、`factory-floor-check.js`、`docs/research/2026-09-19/live-output-check.js`、
    `docs/research/2026-09-23/visual-check.js`、`docs/research/2026-09-27/files-check.js`、`docs/research/2026-09-28/git-check.js`、
    `docs/research/2026-10-01/progress-check.js`、`ui-fixes-check.js`。`factory-floor-check.js` 跑完
    `git checkout -- docs/research/2026-09-16/task-5.2-scenario-d.png`。
  - 截圖（已內建使用者名稱遮罩）：`node docs/research/2026-10-01/ui-fixes-check.js --screenshots`、`node docs/research/2026-10-01/progress-screenshots.js`
  - WSL 版 git 整合測試（opt-in；WSL 是 git 2.43，與 Windows 2.50 行為有差）：PowerShell
    `$env:COCKPIT_GIT_TEST_WSL_DISTRO="Ubuntu-24.04"; cargo test -p cockpit-git -- --ignored`
- **測試數字**（2026-10-01、`0632fa8`，當場跑為準）：workspace 1100 passed／0 failed／13 ignored；ui_preview 40；11 支腳本全 PASS；
  markdownlint 133 files 0 issues；`openspec validate --all` 19 passed。

## 2. 立刻要做：change 8「Live Output 上色」brainstorming

- 現況：`read_output` 固定 `format=text`，`OutputFormat` 只有 `Text`，前端 `textContent` 寫入；spec `live-output` 明文「純文字、不含控制序列」。
  `herdr-client` 已支援 `ReadFormat::Ansi`。
- **先做探測再設計**：設計文件 §2.6 與 `docs/research/2026-09-19/pane-read-probe.md` 第 4 節只在 WSL 0.8.2 看過 SGR `0`、`1`、`38;5;N`；Windows 0.9.x
  沒讀過 ansi 內容，真彩色、背景色、反白、全螢幕 TUI 都未驗。探測對 HERDR 唯讀（只送 `pane.read`）。
- 設計要點（待 brainstorming 定）：SGR 白名單解析器以 `textContent` 逐段寫 `<span class>`、不用 `innerHTML`；色票對應既有 token；
  去重（現在靠「文字相同不重寫」）在 ansi 下是否穩定；`live-output-check.js`（約 3000 行，以 textContent 比對）要怎麼改。

流程：`git switch -c feat/<slug>` → superpowers:brainstorming → `/opsx:propose`（`tasks.md` 開頭寫執行路徑分流）→ apply → Codex 審查 → 併回。

## 3. 這一段踩過的坑

（**不會報錯的錯誤**加粗。）

- **Chrome 把「沒讓任何元素被滑鼠聚焦的滑鼠操作」之後的程式 `focus()` 判成 `:focus-visible`** → 依最後輸入方式傳 `focusVisible:false`；
  修飾鍵（Alt／Ctrl／Meta）不算鍵盤；不移動焦點的滑鼠操作沿用舊外框（memory 有）。
- **`%(*objecttype)` 剝 tag 的深度隨 git 版本而異**（WSL 2.43 一層、Windows 2.50 到底）→ 依 git 輸出細節的判定要兩端各實測（memory 有）。
- **前端複算後端規則（`computeExpectedTips`）**：後端規則一改前端就靜默誤報 → 改後端規則先 grep 前端複本（memory 有）。
- **去識別化只 grep 文字會漏掉截圖**：`ui_preview` fixture 的 cwd 用真實 `%TEMP%`，右欄可見時截圖帶出使用者名稱 → 逐張看圖、拍前遮罩（memory 有）。
- `Option<Option<T>>` 只加 `deserialize_with` 時缺席會報 `missing field`，要 `#[serde(default, deserialize_with = …)]`。
- `live-output-check.js` R 段偶發 FAIL（兩次分開的 `eval` 間被推送重畫換掉節點），腳本競態、重跑即綠；`factory-floor-check.js` 也偶發過一次。
- 驗收腳本一律前景跑；markdownlint 設定排除 `.superpowers/**`（工作區報告不會被檢查）。

change 6 與更早仍有效的坑：`git show 504a687:docs/handover.md` 第 3 節（再往前的鏈結見該節末）。

## 4. 已定但未執行的決策

| 決策 | 狀態 |
|---|---|
| 路線：change 8 Live Output 上色 → 去識別化 → 評估 Tauri | **已定** |
| change 7 以 Opus 5.5 取代 Codex 審查 | **使用者決定**（2026-10-01），僅限 change 7 |
| change 7 細節裁決（archive ledger 內全部 `Ruling:`） | **Claude 依授權裁決**，使用者可推翻 |
| change 5b（Sonnet 驗收）、change 6（Opus 審查）未經 Codex | 要不要補審（5b `45f1174..427118b`、6 `e04644f..37005c1`）由使用者決定 |
| 真機紀錄 `ui-fixes-live.md` 與真機截圖帶出使用者其他私人 repo 名稱（repo-a 等） | **待使用者決定**是否列入去識別化範圍 |
| 去識別化：`main` 上既有檔案含真實主機名／使用者名稱；**git 歷史中舊截圖**（例如 change 6 的 `progress-700.png` 舊版）重拍清不掉 | **未處理**：推 remote 前另開小 change，含 PNG 要逐張看圖、歷史要另行處理 |
| 延後：Graph 起點過多超過命令列上限、refs 超過 1 MiB、git 逾時只殺轉手程式、WSL 內 agent 回報、自訂 `command` 橋接 WSL、change 7 ledger 的 deferred minors、5a／5b 其餘 lows | **延後** |
| v2 狀態檔未知 project 缺 `active` 啟動失敗 | **決定不修**（change 6 裁決） |
| Codex stop review gate（per-repo）本 repo 未啟用 | 靠流程內手動跑 `adversarial-review` |
| Claude 在 feature 分支 commit，收尾 squash 併回 main | **已授權** |

## 5. 之後的路

change 8 Live Output 上色 → 推 remote 前的去識別化小 change → 評估 Tauri 桌面殼（ADR-0005）。

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
| 30 | 2026-10-01 | change 7 `ui-fixes` 實作與全 gate 完成，待審查 |
| 31 | 2026-10-01 | change 7 經 Opus 5.5 審查（取代 Codex）＋兩波修正 → squash 併回 `main` 並 archive，下一段為 change 8 |
