# 交接：下一段任務

> **建立日期**：2026-10-02　|　**上一段做完的事**：change 11 `desktop-launch-notify`——一鍵開啟的桌面啟動器（`cockpit-launch`＋桌面捷徑）、
> 關窗即結束（`cockpit --exit-when-idle`）、四類可開關的桌面通知。使用者授權細節由 Claude 決定、事後彙整；審查由 Opus 5.5 擔任（本專案視同 Codex）。
> 同日稍早：change 9 去識別化並改寫 git 歷史、repo 公開、change 10 Live Output 色彩微調。
> 同日最後：宣傳頁（GitHub Pages）與 README 中英雙語（非 OpenSpec change，文件與靜態頁）。
> **性質**：接手用文件，會過期，每段重寫。
> 為什麼做看 `docs/cockpit-spec.md`（北極星）與各 change 的 proposal；怎麼做看 `~/.claude/CLAUDE.md`（本 repo 精簡版在
> `AGENTS.md`）；規格看 `openspec/specs/`。

## 0. 三十秒版本

1. **等使用者實際使用後回饋**（使用者 2026-10-02：「使用一陣子再來看」）。回來時**先問兩件事**：
   - **通知真機確認**（archive 中未勾的 task 4.2）：設定面板按「允許通知」→ 最小化視窗、等 pane 變 `blocked` → 確認收到 Windows 通知、點下去視窗回來並選定
     該 pane → 最小化 10 分鐘以上仍收得到。結果補記 `docs/research/2026-10-02/desktop-launch-live.md`。
   - **關窗後 agent 回報會遺失可不可以接受**（第 4 節第一列）。
2. repo **已公開** `https://github.com/Benjamin-Teng/ai-cockpit`；**每次推送前**跑 `node docs/research/2026-10-02/deid-check.js --history`（0 命中才推）；
   **歷史不再改寫**。
3. 審查：本專案 Opus 5.5 subagent 審查**視同 Codex**（memory 有）。
4. **建議下一個 change：`/ws`、`/api/state` 加來源檢查**（延後項 m1）。README 已公開揭露「瀏覽器裡開著的網頁讀得到儀表板狀態」，修好後要同步改 `README.md`、`README.zh-TW.md` 的「Read-only by design」段與 `site/` 的對應文字。

## 1. 現在的狀態

- **`main`**：change 1a 至 11 與小 change `html-charset`。本段 commit：`36d0c3f`（change 11 squash）、archive 與本檔。remote：`origin`（公開，`main` 追蹤 `origin/main`）。
  改寫歷史前的舊編號查 `docs/research/2026-10-02/commit-map.txt`（只收可達 commit；檔案內容裡的舊編號沒改）。
- **桌面啟動**（archive `openspec/changes/archive/2026-10-02-desktop-launch-notify/`；規格 `openspec/specs/desktop-launch/`、`desktop-notifications/`）：
  - 使用者電腦已安裝：`%LOCALAPPDATA%\ai-cockpit\bin\`（`cockpit.exe`、`cockpit-launch.exe`）＋桌面捷徑「AI Agent Cockpit」（`--config` 指向 repo 根的
    `cockpit.toml`）。**更新**：改完程式後重跑 `pwsh -File scripts\install-desktop.ps1`（Cockpit 開著時腳本會拒絕，先關視窗等 10 秒）。
  - 啟動器：已在執行就只開視窗；否則背景啟動後端（log 在設定檔旁的 `cockpit.log`，已 gitignore）並以 Chrome（沒有則 Edge）`--app` 開窗；錯誤以訊息框說明。
  - 後端 `--exit-when-idle`：最後一個 `/ws` 關閉滿 10 秒（`GET /`、`/api/state` 會延長）或 60 秒內從未連線即正常結束。手動跑 `cockpit` 不帶旗標時不受影響。
  - 通知：頂列鈴鐺開設定面板；agent blocked／done、task failed／completed 四類開關（預設 blocked、failed）；前景不打擾、>3 件合併、點通知帶回並選定 pane、
    多視窗設定同步。**task failed／completed 目前只由人工標記產生**（agent 不能標記），所以這兩類幾乎只在另一個視窗標記時才會出現。
- **宣傳頁與 README**：`site/`（靜態頁，`.github/workflows/pages.yml` 在 `site/**` 有變動推上 `main` 時發布到 `https://benjamin-teng.github.io/ai-cockpit/`；Pages 來源已設為 GitHub Actions）。主視覺是 Signal Grid 點陣動畫（使用者 2026-10-02 從三個原型選定）。語言：`index.html` head 腳本決定——手動選過照 `localStorage` 的 `cockpit.site.lang`；否則第一順位瀏覽器語言符合 `zh`／`zh-Hant`／`zh-Hans`（可帶 TW／HK／MO／CN）或時區在台港澳中就用繁中；繁中字串在 `site/i18n.js`，鍵與 `index.html` 的 `data-i18n` 一對一。根目錄 `README.md`（英文）與 `README.zh-TW.md` 頂端互相連結；橫幅 `docs/assets/readme-banner.svg` 由 `node docs/assets/gen-readme-banner.js` 產生（改橫幅改產生器再重跑）。`site/img/` 的兩張截圖是 `docs/research/` 既有去識別化截圖的複本。
- **去識別化**：`docs/research/2026-10-02/deid-check.js`（用法 `deid-check.md`）＋本機詞表 `.deid-terms`（repo 根、不進 git，換機器要重建）。保留 `quant-dev`、`shioaji`。
- **可用指令**（repo 根目錄）：
  - 全 gate：`cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test --workspace && cargo test -p cockpit --example ui_preview && markdownlint-cli2 "**/*.md" && openspec validate --all`
  - 推送前：`node docs/research/2026-10-02/deid-check.js && node docs/research/2026-10-02/deid-check.js --history`
  - 預覽：`cargo run -p cockpit --example ui_preview`（`127.0.0.1:7770`）。排程狀態轉換：`COCKPIT_PREVIEW_TRANSITIONS`（`ui_preview.rs` 檔頭）；上色樣本
    `COCKPIT_PREVIEW_OUTPUT_MODES="wJ:p1=ansi;wJ:p4=ansi-flip"`。
  - 驗收腳本（不可並行、一律前景跑、不要包短 timeout、**不要用背景批次**）：12 支既有——`docs/research/2026-09-15/reconnect-check.js`、`whatever-check.js`、
    `docs/research/2026-09-16/actions-check.js`、`channel-backoff-check.js`、`factory-floor-check.js`、`docs/research/2026-09-19/live-output-check.js`、
    `docs/research/2026-09-23/visual-check.js`、`docs/research/2026-09-27/files-check.js`、`docs/research/2026-09-28/git-check.js`、
    `docs/research/2026-10-01/progress-check.js`、`ui-fixes-check.js`、`docs/research/2026-10-02/output-color-check.js`；本段新增
    `docs/research/2026-10-02/notify-check.js`、`idle-exit-check.js`。`factory-floor-check.js` 跑完 `git checkout -- docs/research/2026-09-16/task-5.2-scenario-d.png`。
  - 測啟動器時一律設 `COCKPIT_LAUNCH_DIALOG_FILE`（訊息框改寫檔）與指向假瀏覽器的 `COCKPIT_BROWSER`，避免在使用者桌面開真視窗。
- **測試數字**（2026-10-02，當場跑為準）：workspace 1198 passed／0 failed／13 ignored；ui_preview 61；12 支既有腳本＋notify-check（190／62）＋idle-exit-check 全 PASS；
  markdownlint 0 issues；`openspec validate --all` 21 passed；deid-check 0 命中。

## 2. 立刻要做：等使用者回饋

- 先問第 0 節兩件事。依回饋開 change：feature 分支 → brainstorming（範圍明確可跳）→ `/opsx:propose`（`tasks.md` 開頭寫執行路徑）→ apply → Opus 審查 → squash 併回 → 推送。
- 建 PR 要使用者明確要求；目前慣例是直接 squash 併回 `main` 後推送（已授權）。

## 3. 這一段踩過的坑

（**不會報錯的錯誤**加粗。）

- **Windows 上 spawn 的長命子程序會繼承呼叫端交給父程序的 stdout pipe**：擷取輸出的呼叫端卡到孫程序結束 → 啟動器在 spawn 前清掉自身標準 handle 的繼承旗標（memory 有）。
- **PowerShell 5.1：呼叫端導向錯誤串流時，原生程式的 stderr 變成終止錯誤；32 位元 PS 讀 64 位元程序的 Path 為空**（memory 有）。
- **啟動器模式下關窗約 10 秒後後端結束，之後 agent 的進度回報 `curl -s` 無聲失敗**（已寫進 spec 與 README，待使用者裁決）。
- 宣傳頁：`clip-path` 切角會連外推的 `outline` 一起切掉，鍵盤焦點框消失（`:focus-visible` 仍成立、無錯誤）→ 外框改內縮＋切角漸層補線（同介面做法）。headless 截圖用 `--virtual-time-budget` 時 CSS 延遲進場動畫不會前進，標題像是不見了 → 改用 CDP 實際時間等待。Chrome 的 `--lang=zh-HK`／`zh-SG` 會被換成 zh-TW／zh-CN，測語言判斷的邊界要直接驗判斷式，不能只靠 `--lang`。
- headless 以外的瀏覽器：使用者的 Chrome 已在執行時，`--app` 視窗會併入同一個程序；要關測試開出的視窗，只對「標題恰為 AI Agent Cockpit」的視窗送 `WM_CLOSE`，
  不可對 Chrome 程序用 `CloseMainWindow`（會關到使用者的主視窗）。PowerShell 每次呼叫是新工作階段，`Add-Type` 定義的型別不會保留。
- Windows 子程序會依 `ProgramW6432` 重算 `ProgramFiles`，改環境變數藏不住已安裝的 Chrome。
- 驗收腳本以背景批次＋短 timeout 執行時，殼被殺但迴圈續跑，兩輪重疊搶 7770 造成假 FAIL → 一律前景逐支跑。
- 既有偶發：`git-check.js`「收尾衛生」、`factory-floor-check.js`、`live-output-check.js`、`reconnect-check.js`（Chrome 20 秒內沒出現 page target）、
  `tests/app.rs::abort_await_is_bounded`（高負載）→ 重跑一次並記錄。

change 10 與更早仍有效的坑：本檔前一版第 3 節（`git log -2 --format=%h -- docs/handover.md` 取前一版編號後 `git show <編號>:docs/handover.md`）。

## 4. 已定但未執行的決策

| 決策 | 狀態 |
|---|---|
| 啟動器模式下關窗後 agent 回報遺失（change 11 整支審查 I1） | **Claude 依授權維持「關窗即結束」並文件化**；**待使用者裁決**（替代：後端常駐＋系統匣，或 agent 端回報失敗重試） |
| 通知的真機確認（change 11 task 4.2） | **待使用者操作** |
| change 11 其他細節（archive ledger 全部 `Ruling:`） | **Claude 依授權裁決**，使用者可推翻 |
| 是否加 LICENSE | 未決；使用者沒提，維持現狀。README 與宣傳頁刻意不提授權 |
| 延後：change 11 的 m1（任何網頁可連 `/ws` 使後端不閒置結束——建議另開 change 為 `/ws` 加來源檢查）、m5（`cockpit.log` 每次覆寫）、m6（啟動器與安裝腳本缺 repo 內回歸驗收）、3.6 M6（HTML 預覽 iframe 內點擊不關面板）、2.6 其餘小項；change 8 與更早的延後項（見前一版本檔第 4 節） | **延後** |
| v2 狀態檔未知 project 缺 `active` 啟動失敗 | **決定不修**（change 6 裁決） |
| Claude 在 feature 分支 commit，收尾 squash 併回 main 並推送 | **已授權** |

## 5. 之後的路

使用者回饋 → 依回饋修正。之後可考慮：`/ws` 來源檢查（同時解掉 m1）、若常駐需求出現再評估 Tauri（系統匣、開機自啟；ADR-0005 補充段）。

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
| 32 | 2026-10-02 | change 8 `live-output-color` 併回並 archive |
| 33–34 | 2026-10-02 | change 9 `deidentify`（改寫 git 歷史）；建立 GitHub repo 並推送 |
| 35 | 2026-10-02 | repo 公開；change 10 `output-color-tuning` |
| 36 | 2026-10-02 | change 11 `desktop-launch-notify`（啟動器、關窗即結束、桌面通知）併回並 archive；兩份備份已刪除 |
| 37 | 2026-10-02 | 宣傳頁（GitHub Pages）、README 中英雙語與生成藝術橫幅；Opus 審查兩輪 |
