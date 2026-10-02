# 交接：下一段任務

> **建立日期**：2026-10-02　|　**上一段做完的事**：change 9 `deidentify`（推 remote 前去識別化）——A 段清理目前檔案＋防再犯檢查腳本，
> B 段以 git-filter-repo **改寫全部 git 歷史**（作者改為 GitHub noreply），兩段都經 Opus 5.5 審查／獨立驗證（使用者決定取代 Codex）。
> **性質**：接手用文件，會過期，每段重寫。
> 為什麼做看 `docs/cockpit-spec.md`（北極星）與各 change 的 proposal；怎麼做看 `~/.claude/CLAUDE.md`（本 repo 精簡版在
> `AGENTS.md`）；規格看 `openspec/specs/`。

## 0. 三十秒版本

1. **沒有 active change**，`main` 乾淨、無 feature 分支，**repo 已可推上 remote**。尚未建立 remote、尚未推送——建立 GitHub repo、公開與否、推送
   都等使用者指示。**每次推送前**先跑 `node docs/research/2026-10-02/deid-check.js --history`（0 命中才推）。
2. **git 歷史已於 2026-10-02 改寫**，所有 commit 編號都換了；文件裡的舊編號用 `docs/research/2026-10-02/commit-map.txt` 查（第 1 節）。
3. change 7、8、9 以 Opus 取代 Codex 都是使用者對各該 change 的個別決定，**不延伸到下一個 change**；之後照 CLAUDE.md 一律 Codex
   （週限額 2026-10-04 10:55 恢復，之前要審查就先問使用者）。

## 1. 現在的狀態

- **`main`**：change 1a／1b／2／3／4／5a／5b／6／7／8／9 與小 change `html-charset`。本段 commit（改寫後編號）：`ca4fad1`（change 9 A 段 squash）、
  `1dd8b68`（B 段完成與對照表）、archive 與本檔。change 8 squash 是 `f065468`、其 archive 是 `bd8bb6e`。沒有 remote。
- **歷史改寫**（細節：archive `openspec/changes/archive/2026-10-02-deidentify/` 的 design D5）：
  - 作者與 committer 全為 `Benjamin-Teng <68319994+Benjamin-Teng@users.noreply.github.com>`；本 repo 的 `git config user.name／user.email` 已設成同一身分。
  - **舊編號對照**：`docs/research/2026-10-02/commit-map.txt`（首行 `old new`，之後每行 40 碼舊→新）。用法：`grep '^<舊短編號>' docs/research/2026-10-02/commit-map.txt`。
    commit 訊息裡的舊編號已由 filter-repo 自動改寫；**檔案內容（handover、ledger、研究紀錄）裡的舊編號沒改**，要靠對照表查。已被 squash 掉的 feature
    分支 commit 不在對照表裡，查不到。
  - **改寫前的完整備份（含原始個人資料，不會被推送）**：`D:\projects\ai-cockpit-backup-2026-10-02\`（整份目錄連 `.git`，排除 `target/`）與
    `D:\projects\ai-cockpit-mirror-2026-10-02.git`（mirror）。**保留或刪除由使用者決定**；推送與否都不影響它們。
- **去識別化現況**：Windows／WSL 使用者名稱換成 `<user>`，三個私人 repo 名稱換成 `repo-a`／`repo-b`／`repo-c`；change 7 三張真機截圖已遮蓋；使用者決定
  **保留** `quant-dev`（WSL 日常工作 repo）與 `shioaji`（workstream id、概念圖範例標題）。
- **防再犯檢查** `docs/research/2026-10-02/deid-check.js`（用法 `deid-check.md`）：詞表執行時取得（使用者名稱、主機名稱含 NetBIOS、WSL 使用者名稱、
  global git email）＋本機詞表 `.deid-terms`（repo 根，**不進 git**；放私人 repo 名稱與改寫前的 email，換機器要自己重建）。檔案模式（預設，可 `--rev`）、
  `--history`（全部物件、作者欄位、ref 名稱）。只比對位元組，**截圖畫面要另外逐張看圖**。
- **可用指令**（repo 根目錄）：
  - 全 gate：`cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test --workspace && cargo test -p cockpit --example ui_preview && markdownlint-cli2 "**/*.md" && openspec validate --all`
  - 推送前：`node docs/research/2026-10-02/deid-check.js && node docs/research/2026-10-02/deid-check.js --history`
  - 預覽：`cargo run -p cockpit --example ui_preview`（`127.0.0.1:7770`；需要本機安裝 git）。上色樣本：PowerShell
    `$env:COCKPIT_PREVIEW_OUTPUT_MODES="wJ:p1=ansi;wJ:p4=ansi-flip"` 後再跑。
  - 驗收腳本（不可並行、一律前景跑、不要包短 timeout）：12 支——`docs/research/2026-09-15/reconnect-check.js`、`whatever-check.js`、
    `docs/research/2026-09-16/actions-check.js`、`channel-backoff-check.js`、`factory-floor-check.js`、`docs/research/2026-09-19/live-output-check.js`、
    `docs/research/2026-09-23/visual-check.js`、`docs/research/2026-09-27/files-check.js`、`docs/research/2026-09-28/git-check.js`、
    `docs/research/2026-10-01/progress-check.js`、`ui-fixes-check.js`、`docs/research/2026-10-02/output-color-check.js`。`factory-floor-check.js` 跑完
    `git checkout -- docs/research/2026-09-16/task-5.2-scenario-d.png`。
  - WSL 真機測試的 socket 預設值改為執行時由 WSL `$HOME` 推得（`herdr-client/tests/real_herdr.rs`、`examples/spike4_no_window.rs`），可用
    `HERDR_CLIENT_TEST_WSL_SOCKET` 覆寫。
- **測試數字**（2026-10-02、改寫後 `main`，當場跑為準）：workspace 1151 passed／0 failed／13 ignored；ui_preview 48；12 支腳本全 PASS；
  markdownlint 0 issues；`openspec validate --all` 19 passed；`deid-check.js --history` 0 命中。

## 2. 立刻要做：等使用者決定推送方式

- 使用者指示建立 remote 時：GitHub 帳號 `Benjamin-Teng`（`gh` 已登入）。建 repo、設定 remote、推送都是對外動作，**每一步先跟使用者確認**（公開／私人、repo 名稱）。
- 推送前必跑 `deid-check.js`（檔案模式＋`--history`），只推 `main`。
- 推送之後若要再改寫歷史，代價變高（要 force push、所有 clone 失效），所以有疑慮要在第一次推送前提出。

## 3. 這一段踩過的坑

（**不會報錯的錯誤**加粗。）

- **Git Bash 的 MSYS 路徑轉換會改寫以 `/` 開頭的引數，連 `git grep -F "/home/x/"` 的搜尋樣式也是 → 0 命中、exit 正常**——殘留檢查一律在 Node 內比對（memory 有）。
- **git-filter-repo 的細節**（實測）：
  - **`--mailmap` 的單欄寫法 `New <new>` 什麼都不改**；要 `New <new> <舊 email>`，會同時改 author 與 committer。
  - `--replace-text` 的 literal 分大小寫，要不分大小寫用 `regex:(?i)…`；`#` 開頭的行不是註解、會被當 literal；檔首 BOM 會讓第一條失效。
  - **指向 tree 的 ref（例如 `refs/codex/*`）只警告、不處理，舊 tree 仍可達**——要先 `git update-ref -d`。
  - 結束時自動 `reset --hard`、清 reflog、gc，**舊物件立刻消失**，驗證失敗只能從備份還原 → 先在 `git clone --no-local` 的副本演練。
  - 副本裡只要有一個未追蹤檔就判定「不是 fresh clone」而拒絕執行（演練時先別放 `.deid-terms`）。
- `git fsck --lost-found` 會在 `.git/lost-found/` 留下舊 blob 的明文副本，gc 不會清，改寫後要手動刪。
- Bash 工具的安全檢查會擋 `rm -rf` 磁碟根目錄下的資料夾（例如 `D:/xxx`）與多指令串接的刪除 → 暫存一律放 `%TEMP%`，刪除用單一指令與字面路徑。
- 驗收腳本偶發：`git-check.js`「收尾衛生」、`factory-floor-check.js`、`live-output-check.js`（R 段或 pane 列焦點）→ 重跑一次並記錄兩次結果。

change 8 與更早仍有效的坑：本檔改寫前的版本第 3 節（`git show 11561e5:docs/handover.md`，即舊編號 `08f82ec`；再往前的鏈結見該節末，其中的舊編號用對照表換算）。

## 4. 已定但未執行的決策

| 決策 | 狀態 |
|---|---|
| 建立 GitHub remote、公開或私人、推送 | **待使用者指示** |
| 改寫前的兩份備份（含原始個人資料）保留或刪除 | **待使用者決定** |
| 色相歸色無彩門檻（change 8 審查 M1）：HERDR 改傳 256／真彩色前要不要先調 | **待使用者決定**，建議另開小 change |
| Live Output 粗體 600 在 12px 等寬字與一般字差距小，是否改 700（spec 變更） | **待使用者決定** |
| change 5b（Sonnet 驗收）、change 6／7／8／9（Opus 審查）未經 Codex | 要不要補審由使用者決定（舊編號範圍用 `commit-map.txt` 換算） |
| change 7／8／9 細節裁決（各 archive 的 ledger、design 內 `Ruling:` 與使用者決定） | **Claude 依授權裁決**，使用者可推翻 |
| 去識別化保留 `quant-dev`、`shioaji` | **已定**（使用者 2026-10-02） |
| 延後：change 8 deferred minors（連續兩行淡底細縫、200 但本體不合法不標過期、404 轉暗與 dim 組合未寫成斷言、主 spec 未記 vte 限制、註解標號殘留）；change 9 殘留（spike4 範例把含 WSL 使用者名稱的 socket 路徑印到本機終端機、無 BOM 單行純 CJK 的 UTF-16 詞表會誤判）；更早：Graph 起點過多、refs 超過 1 MiB、git 逾時只殺轉手程式、WSL 內 agent 回報、自訂 `command` 橋接 WSL、change 7 deferred minors、5a／5b 其餘 lows | **延後** |
| v2 狀態檔未知 project 缺 `active` 啟動失敗 | **決定不修**（change 6 裁決） |
| Codex stop review gate（per-repo）本 repo 未啟用 | 靠流程內手動跑 `adversarial-review` |
| Claude 在 feature 分支 commit，收尾 squash 併回 main | **已授權** |

## 5. 之後的路

使用者指示推送 → 評估 Tauri 桌面殼（ADR-0005）。

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
| 30–31 | 2026-10-01 | change 7 `ui-fixes`（Opus 審查取代 Codex）併回並 archive |
| 32 | 2026-10-02 | change 8 `live-output-color`（Opus 審查取代 Codex）併回並 archive |
| 33 | 2026-10-02 | change 9 `deidentify`：A 段清理併回、B 段改寫全部 git 歷史（Opus 審查與獨立驗證），archive；repo 可推送 |
