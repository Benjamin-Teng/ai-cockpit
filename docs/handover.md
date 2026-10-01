# 交接：下一段任務

> **建立日期**：2026-10-02　|　**上一段做完的事**：change 8 `live-output-color` 全流程完成——ansi 真機探測 → brainstorming（原型比較，使用者選「照字面」）
> → propose → SDD 實作 → 真機驗收（Windows、WSL）→ Opus 5.5 審查（使用者決定取代 Codex）後端、前端、整支分支 → squash 併回 `main` → archive。
> **性質**：接手用文件，會過期，每段重寫。
> 為什麼做看 `docs/cockpit-spec.md`（北極星）與各 change 的 proposal；怎麼做看 `~/.claude/CLAUDE.md`（本 repo 精簡版在
> `AGENTS.md`）；規格看 `openspec/specs/`。

## 0. 三十秒版本

1. **沒有 active change**，`main` 乾淨、無 feature 分支。下一段＝**推 remote 前的去識別化小 change**（第 2 節），範圍與歷史處理方式已由使用者決定（第 4 節前兩列）。
2. change 7、8 以 Opus 取代 Codex 都是使用者對該 change 的個別決定，**不延伸到下一個 change**；之後照 CLAUDE.md 一律 Codex（週限額
   2026-10-04 10:55 恢復，之前要審查就先問使用者）。

## 1. 現在的狀態

- **`main`**：change 1a／1b／2／3／4／5a／5b／6／7／8 與小 change `html-charset`。本段 commit：`c7ce810`（change 8 squash）、`d0489db`（archive）、本檔。
  沒有 remote。
- change 8 內容與紀錄：archive `openspec/changes/archive/2026-10-02-live-output-color/`（proposal、design D1–D9、`sdd-ledger.md` 含全部 Ruling、
  三次 Opus 審查摘要與延後項）；探測 `docs/research/2026-10-02/ansi-probe.md`；真機驗收 `docs/research/2026-10-02/output-color-live.md`（只記中繼資料）。
- Live Output 現況：後端以 `vte` 0.15 解析 HERDR `format=ansi`（`cockpit-herdr/src/ansi.rs`），`/output` 回 `text`＋`segments`；前端依片段以
  `textContent` 建 span、class 來自固定對照表（`cockpit/assets/app/output.js`、`style.css` 的 `ansi-*`）。16 色對應既有色票，不新增色碼。
- **可用指令**（repo 根目錄）：
  - 全 gate：`cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test --workspace && cargo test -p cockpit --example ui_preview && markdownlint-cli2 "**/*.md" && openspec validate --all`
  - 預覽：`cargo run -p cockpit --example ui_preview`（`127.0.0.1:7770`；需要本機安裝 git）。上色樣本：
    PowerShell `$env:COCKPIT_PREVIEW_OUTPUT_MODES="wJ:p1=ansi;wJ:p4=ansi-flip"` 後再跑（標籤清單見 `ui_preview.rs` 的 `ansi_sample` 文件註解）。
  - 驗收腳本（不可並行、一律前景跑、不要包短 timeout，visual-check 要 5 分鐘以上）：12 支——`docs/research/2026-09-15/reconnect-check.js`、`whatever-check.js`、
    `docs/research/2026-09-16/actions-check.js`、`channel-backoff-check.js`、`factory-floor-check.js`、`docs/research/2026-09-19/live-output-check.js`、
    `docs/research/2026-09-23/visual-check.js`、`docs/research/2026-09-27/files-check.js`、`docs/research/2026-09-28/git-check.js`、
    `docs/research/2026-10-01/progress-check.js`、`ui-fixes-check.js`、`docs/research/2026-10-02/output-color-check.js`。`factory-floor-check.js` 跑完
    `git checkout -- docs/research/2026-09-16/task-5.2-scenario-d.png`。
  - 截圖（已內建使用者名稱遮罩）：`node docs/research/2026-10-02/output-color-check.js --screenshots`、`node docs/research/2026-10-01/ui-fixes-check.js --screenshots`、
    `node docs/research/2026-10-01/progress-screenshots.js`
  - WSL 真機腳本：`HERDR_CLIENT_TEST_ALLOW_WSL_WRITES=1 COCKPIT_ACCEPT_WSL_DISTRO=Ubuntu-24.04 node docs/research/2026-09-19/live-output-real-check.js`
    （先 `cargo build -p cockpit`；自己起停專屬 socket 的測試 server）
  - `pane.read` 唯讀探測：`cargo run -p herdr-client --example probe_pane_read -- --list`（用法與隱私限制見 `ansi-probe.md`「重跑」）
- **測試數字**（2026-10-02、`c7ce810`，當場跑為準）：workspace 1151 passed／0 failed／13 ignored；ui_preview 48；12 支腳本全 PASS；
  markdownlint 143 files 0 issues；`openspec validate --all` 19 passed。

## 2. 立刻要做：推 remote 前的去識別化小 change

- 範圍（現況已知）：`main` 上既有檔案含真實主機名／使用者名稱（`git grep -il` 以 `node -e "process.stdout.write(require('os').userInfo().username)"`、
  `os.hostname()` 取得的值搜，2026-10-02 命中 `docs/research/2026-09-13/` 四個檔與 archive `2026-09-15-attach-herdr-runtimes/tasks.md`）；PNG 要逐張看圖並
  grep 位元組；**git 歷史**中的舊截圖（change 6 `progress-700.png` 舊版）重拍清不掉，要另定歷史處理方式。
- 已定：其他私人 repo 名稱換成代號；推 remote 前改寫歷史（先備份整個 repo）。
- 流程：`git switch -c feat/<slug>` → 判斷是否需要 brainstorming（範圍明確時可直接 `/opsx:propose`，`tasks.md` 開頭寫執行路徑分流）→ apply → Codex 審查 → 併回。

## 3. 這一段踩過的坑

（**不會報錯的錯誤**加粗。）

- **HERDR `format=text` 會剪行尾空白並在結尾補換行，`format=ansi` 不會** → Live Output 的 `text` 改由 ansi 解析後，與舊值只差行尾空白與結尾換行
  （兩端實測，design D3 決定不剪）；任何拿 `format=text` 逐字比對的工具都會看到差異。
- **`vte` 只認 7-bit 序列**：8-bit C1 引導字元、DCS 內文含 0x9C、ESC 後接非 ASCII 會漏出或吃掉幾個字元（design D1 已知限制；HERDR 實測只輸出 7-bit SGR）。
- **色相歸色的無彩門檻（最大−最小 < 64）會把 Claude Code 深色主題的 diff 底色歸成灰** → 目前 HERDR 只傳 16 色所以沒事；HERDR 改傳 256 色／真彩色時會不報錯地畫錯（第 4 節待決）。
- **真機 pane 的截圖遮罩擋不住畫面內容本身**（遮罩只處理路徑與名稱）→ 真機截圖不進 repo，看完即刪；真機紀錄只寫中繼資料（memory 有）。
- SDD skill 的 `scripts/task-brief` 只認 `### Task N`，OpenSpec 的勾選格式抽不到 → 在 SDD 工作區自寫 brief 產生器（`awk` 用 `index()` 比對，見 change 7、8 的 ledger 裁決）。
- 驗收腳本偶發：`git-check.js`「收尾衛生」（Chrome utility 子行程晚幾秒才退）、`factory-floor-check.js`（Factory Floor 沒畫出來）、`live-output-check.js`（R 段或 pane 列焦點）
  → 重跑一次並記錄兩次結果。
- Bash 工具的安全檢查會擋 `rm -rf "$(…)"` → 先印出路徑，再刪字面路徑。
- 這台 Windows 的 `python3` 會開 VS Code 原生 REPL（Store 別名），不要拿來當腳本直譯器。

change 7 與更早仍有效的坑：`git show 1d3c475:docs/handover.md` 第 3 節（再往前的鏈結見該節末）。

## 4. 已定但未執行的決策

| 決策 | 狀態 |
|---|---|
| 真機紀錄 `ui-fixes-live.md` 與真機截圖帶出使用者其他私人 repo 名稱 | **已定**（使用者 2026-10-02）：列入去識別化，換成代號 |
| 去識別化如何處理 **git 歷史**中的舊圖與檔案 | **已定**（使用者 2026-10-02）：推 remote 前改寫歷史，改寫前先備份整個 repo |
| 色相歸色無彩門檻（change 8 審查 M1）：HERDR 改傳 256／真彩色前要不要先調 | **待使用者決定**，建議另開小 change |
| Live Output 粗體 600 在 12px 等寬字與一般字差距小，是否改 700（spec 變更） | **待使用者決定** |
| change 5b（Sonnet 驗收）、change 6／7／8（Opus 審查）未經 Codex | 要不要補審由使用者決定（5b `45f1174..427118b`、6 `e04644f..37005c1`、7、8 見各 archive） |
| change 7／8 細節裁決（各 archive ledger 內全部 `Ruling:`） | **Claude 依授權裁決**，使用者可推翻 |
| 延後：change 8 ledger 的 deferred minors（連續兩行淡底細縫、200 但本體不合法不標過期、404 轉暗與 dim 組合未寫成斷言、主 spec 未記 vte 限制、註解標號殘留）；更早：Graph 起點過多、refs 超過 1 MiB、git 逾時只殺轉手程式、WSL 內 agent 回報、自訂 `command` 橋接 WSL、change 7 deferred minors、5a／5b 其餘 lows | **延後** |
| v2 狀態檔未知 project 缺 `active` 啟動失敗 | **決定不修**（change 6 裁決） |
| Codex stop review gate（per-repo）本 repo 未啟用 | 靠流程內手動跑 `adversarial-review` |
| Claude 在 feature 分支 commit，收尾 squash 併回 main | **已授權** |

## 5. 之後的路

推 remote 前的去識別化小 change → 評估 Tauri 桌面殼（ADR-0005）。

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
| 32 | 2026-10-02 | change 8 `live-output-color`（Opus 5.5 審查取代 Codex）squash 併回 `main` 並 archive，下一段為去識別化 |
