# 交接：下一段任務

> **建立日期**：2026-09-26　|　**上一段做完的事**：change 4 `direction-01-visual`（Direction 01 視覺改版）16/16 完成，
> squash 併回 `main` 並 archive；使用者同時重排了之後的路線（change 5／6／7）。
> **性質**：接手用文件，會過期，每段重寫。
> 為什麼做看 `docs/cockpit-spec.md`（北極星）與各 change 的 proposal；怎麼做看 `~/.claude/CLAUDE.md`（本 repo 精簡版在
> `AGENTS.md`）；規格看 `openspec/specs/`。

## 0. 三十秒版本

1. **沒有時效性任務。** 在 `main` 上工作，沒有 remote、沒有進行中的分支。
2. **下一步**：開 change 5「檔案瀏覽與 Review」。先用 Skill 載入 `superpowers:brainstorming`，把第 2 節列的待定題逐一問使用者，
   定案後 `/opsx:propose`。brainstorming 時選項要先白話解釋名詞與後果（見 memory `user-prefers-plain-language-before-tradeoffs`）。
3. change 5 會裝新套件（Markdown 渲染、PDF 檢視、可能還有 git graph 繪圖），**每一個都要先問使用者**。

## 1. 現在的狀態

- **已上線（`main`）**：change 1a／1b／2／3／4。各模組職責見 `cockpit/README.md`、`herdr-client/README.md`；依賴方向見 ADR-0003。
  - change 4 的畫面：
    - `.shell` 三欄外框，寬 ≥1200 且高 ≥720 固定一屏，其餘整頁捲動。
    - 顏色只准用 10 個色彩 token，字級只准 `--fs-*` 四階（TK1 靜態檢查會擋）。
    - Live Output 常駐，沒選 pane 時顯示空狀態；過期標示是 `--text-dim` 文字＋`--warn` 左緣條＋「過期」字樣。
    - 設計依據是 `docs/direction-01-visual-design.md` 與 archive 裡的 design D1–D11。
- **archive**：`openspec/changes/archive/2026-09-26-direction-01-visual/`。SDD 全紀錄在其中的 `sdd-ledger.md`：
  裁決 R1–R43、每個 task 的審查輪次、所有使用者裁決。
- **可用指令**（repo 根目錄）：
  - 全 gate：`cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test --workspace && cargo test -p cockpit --example ui_preview && markdownlint-cli2 "**/*.md" && openspec validate --all`
  - 正式服務：`cargo run -p cockpit`（讀工作目錄的 `cockpit.toml`；沒有就是零設定模式，Factory Floor 為空）
  - 預覽（不需要 HERDR，資料寫死、按鈕只記錄不改狀態）：`cargo run -p cockpit --example ui_preview`（`127.0.0.1:7770`）
  - 視覺驗收：`node docs/research/2026-09-23/visual-check.js [段代號,...]`，用法與段落表見同目錄 `visual-check.md`。
    打錯段代號會 exit 2。
  - 既有六支腳本：`docs/research/2026-09-15/reconnect-check.js`、`whatever-check.js`、`docs/research/2026-09-16/actions-check.js`、
    `channel-backoff-check.js`、`factory-floor-check.js`、`docs/research/2026-09-19/live-output-check.js`
  - WSL 真機：`docs/research/2026-09-19/live-output-real-check.js`（用法見 `live-output-acceptance.md`）；
    `--self-test-stale-signals` 可離線跑。
  - Codex 審查：`node ~/.claude/plugins/cache/openai-codex/codex/1.0.6/scripts/codex-companion.mjs adversarial-review --wait --base <BASE> "<focus>"`
- **測試數字**：以當場輸出為準。最後紀錄（2026-09-26，併回前）：workspace 531 passed／0 failed／10 ignored；
  ui_preview example 13；visual-check 1464 ok／0 FAIL；六支腳本全 PASS；WSL 真機 53 ok；markdownlint 0 issues；
  `openspec validate --all` 全過。
- **環境**：port 7680（svchost）與 7778（ArmouryCrate）是系統服務，不要碰。
  使用者 2026-09-26 已移除 Windows 端 herdr 的 `herdr-sidebar` plugin；備份在 `%APPDATA%\herdr\*.bak-20260926-pre-sidebar-uninstall`，
  舊狀態資料改名為 `%LOCALAPPDATA%\herdr\plugins\herdr-sidebar.removed-20260926`。`layout.ps1`、`new-space.ps1` 還留著提到 sidebar 的過期註解。

## 2. 立刻要做：change 5「檔案瀏覽與 Review」brainstorming

使用者 2026-09-26 定的範圍（ledger 的 USER NOTE／USER DECISION 有原話）：

- **介面下半部可以 Review 檔案**，支援 md、pdf、html。使用者說這是 HERDR 最欠缺的功能。
- **檔案瀏覽 sidebar**：專注在某個 space（HERDR workspace）時可以打開，列出那個 space 的檔案，點了就在下半部 Review。
- **檔案 icon**：來源是 `https://github.com/material-extensions/vscode-material-icon-theme`（MIT，`icons/` 下約 900 個 SVG，
  有 `markdown.svg`、`pdf.svg`、`html.svg`）。**保留原色**，列為十色 token 規則的明文例外（使用者裁決）。
  design 與 spec 要寫明例外範圍，TK1 類檢查要排除 icon SVG；授權要保留 MIT 聲明。
- **Git Graph 類的 commit 圖**：參考 `https://github.com/mhutchie/vscode-git-graph/tree/d7f43f429a9e024e896bac9fc65fdc530935c812`。
  **不得取用它的程式碼**：它的 LICENSE 禁止散布衍生作品，推 remote 就可能構成散布。只拿它當功能與外觀參考，自己實作；
  畫圖若要用函式庫，另找寬鬆授權的。

brainstorming 要問使用者的題目：

1. 下半部 Review 區跟 Live Output 怎麼共處：分頁切換、並排，還是暫時取代？
2. 「space 的檔案」根目錄怎麼取：HERDR 給得出每個 pane 的 cwd，同一個 workspace 的 pane cwd 不同時選哪個？
3. sidebar 放哪：跟左欄 Project 切換疊放、分頁，還是從側邊滑出？這會牽動三欄版面（design D3）。
4. 讀檔端點的安全邊界：只允許讀哪些目錄？沿用既有的 `source_check`（只收本機同源）。
5. HTML 預覽一律放 sandbox iframe，不在 cockpit 頁面直接渲染。
6. Markdown 渲染套件、PDF 檢視套件、git graph 畫法：各選哪個？要先問使用者。
7. Git Graph 只看，還是也能操作？能操作就是寫入，範圍與風險大很多。
8. folder icon 的實際檔名（不是 `folder.svg`）要查證。

注意：

- 詞彙表 `CONTEXT.md` 規定 **Cockpit 不用 Space 一詞**。使用者口中的 space 就是 HERDR 的 workspace，寫 artifact 時用 workspace。
- 使用者問過能不能讓 cockpit 裝 VS Code extension。已建議不做 extension host（等於重寫 VS Code 核心），改成自己做少數功能，
  必要時加一顆「在 VS Code 開啟」按鈕。使用者要列出最想要的 VS Code 功能，brainstorming 時可以再問一次。

## 3. 接著要做：change 6「進度模型」與 change 7「畫面與操作修補」

- **change 6「進度模型」**（使用者 2026-09-26 定案）：
  - 進度目前全靠人工按按鈕，看板會跟真實進度脫節。方向是讓 agent 自己呼叫 cockpit 的 API 回報進度，
    寫入的是 cockpit 自己的狀態檔，**不需要推翻「對 HERDR 唯讀」**。「HERDR 的 `done` 不等於 task 完成」這條原則保留。
  - 活動狀態目前只到 workstream 層級（同一條 workstream 的 task 共用 pane 綁定，見 `CONTEXT.md`），要精確到 task。
  - 推進要有「退回」操作。現在誤按只能停服務、手改 `cockpit.state.json`（見 `cockpit/README.md` 的推進限制）。
- **change 7「畫面與操作修補」**：
  - Live Output 上色。
  - M2：斷線期間無法取消改綁，因為 `ProjectedBinding::RuntimeDisconnected` 不帶 `source`。
  - U1：滑鼠點按鈕或列之後會留下冰青焦點框，重畫後還在；選定的 pane 列因此像 running 節點。改法是 `:focus-visible`，
    要確認鍵盤焦點還原沒有退步。
  - U2：cockpit 服務斷線時，只有頂列標「最後已知」，右欄 runtime 卡片仍用綠色顯示 connected。
- **多 runtime 通用支援不排**：目前沒有第二個 runtime 要接（YAGNI），核心模型與執行環境已經分層（ADR-0003）。

## 4. 這一段踩過的坑（不要再推導一次）

（**不會報錯的錯誤**加粗。）

流程與工具：

- **Codex log 只有最後的「# Codex Adversarial Review」結論段才算數**：開頭的 `verdict: approve` 可能只是占位，斷線時程序也照樣 exit 0。
  讀結果一律用 `sed -n '/^# Codex/,$p'`，取不到就重跑。見 memory `codex-review-verdict-only-valid-in-final-section`。
- Codex 在唯讀沙箱只能靜態推導，**findings 一律先重現才採信**。本段最終修正波的四條都先重現過才修。
- **subagent 會自己砍掉「占用 port 的陌生程序」**：5.4 驗證時，一個 agent 砍了使用者手動開的 `ui_preview`。
  派工要明寫：不是自己開的程序只能回報，不准砍；跑 visual-check 前先確認 7770 沒人用。
- **跑 visual-check 時不能同時開另一個 `ui_preview`／Chrome**：收尾的衛生檢查會把它當殘留，判 FAIL。
- **給使用者目視的截圖不能用整頁截圖（`captureBeyondViewport`）**：sticky 底列會被凍在頁面中段，看起來像壞掉。改用 viewport 截圖。
- subagent 開了背景監看就結束時，通知會寫「waiting on background work」。這不代表完成，要等它真正回報。
- 同類 finding 連續三輪還在冒，就換設計，不要再補洞（本段 4.1 的對齊斷言從字形框改成 content-box 起點，一次解決）。
- `tasklist` 在 Git Bash 裡會用 Big5 印出亂碼；「沒有符合的工作」那行看起來是亂碼，不是錯誤。
- `markdownlint-cli2` 的「Summary: 0 issues in 0 files」指的是「有問題的檔案 0 個」，實際 lint 的檔數在上一行「Linting: N files」。

前端：

- **「貼底跟著走」若靠「下次更新時距底 ≤ 門檻」推導，捲動容器自己變矮（多一行提示）就會永久脫離貼底**：
  見 memory `stick-to-bottom-lost-when-container-resizes`；`output.js` 的 `keepPinnedAcross()` 是對策。
- **整頁 `replaceChildren` 重畫會丟掉只存在 DOM 上的狀態**：見 memory `full-repaint-discards-state-held-only-in-dom`。
  捲動快照現在帶 Project id，切換 Project 會歸零。
- **Chrome 用 Tab 聚焦時，元素橫向只要露出約 32px 以上就不捲**（實測，沒有一手文件），右緣按鈕的焦點框會被裁掉一截：
  光靠 `scroll-padding` 修不到，所以 `#app` 加了 focusin 補捲；重畫還原焦點時不能觸發它。
- **visual-check 的對比工具算不到半透明底色，CT1 的數字會偏高**（5.2 M5；人工重算全頁最低 5.23，仍合格）。之後改色要自己重算。
- **CSS 多行註解中間斷開，整條規則會被瀏覽器吞掉，而且沒有任何錯誤**：改完 CSS 要到 CSSOM 確認規則還在。
- **`factory-floor-check.js` 每跑一次都會覆寫已進版控的 `docs/research/2026-09-16/task-5.2-scenario-d.png`**，跑完要 `git checkout --` 還原。
- `scrollbar-width` 不會繼承，每個內層捲動容器都要各自設定。
- 截圖**不可以隱藏捲軸**，否則設計審核會誤判。

change 3 以前仍然有效的坑（HERDR 行為、axum、WSL、Git Bash），見 `git show e6163f6:docs/handover.md` 第 4 節。

## 5. 已定但未執行的決策

| 決策 | 狀態 |
|---|---|
| 路線：change 5 檔案瀏覽與 Review → change 6 進度模型 → change 7 畫面與操作修補 | **已定**（使用者 2026-09-26，取代原「change 5 .md 瀏覽、change 6 上色＋M2」） |
| 外觀改動要多一道 frontend-design 設計審核（設計文件與 spec 優先，skill 只管剩下的自由度） | **已定**（使用者 2026-09-24），見 memory |
| change 5 icon：Material Icon Theme、保留原色、列為 token 規則例外 | **已定**（使用者 2026-09-26） |
| Git Graph：只當參考、不取用程式碼（授權禁止散布衍生作品） | **已定**（控制端查證 LICENSE） |
| 不做 VS Code extension host | **建議**，使用者尚未最終表態 |
| 多 runtime 通用支援 | **不做**，等真的有第二個 runtime 再開 |
| 可延後的 change 4 殘項（Codex final review 分流）：TK1 對註解內字串與 `font:` 簡寫的解析缺口；1280 寬節點按鈕排兩列依賴字型與文案；錯誤文案以 HTTP 碼與 API 路徑開頭；頂列與底列框線不對稱（R23，使用者接受）；pane 列「作用中」離前後文字太遠；最近事件區內距 8px 與同欄 12px 不一致；文字放大 200% 時右欄明細值欄偏窄；對比工具不支援半透明底 | **延後**，細節見 archive 的 `sdd-ledger.md` |
| 切回原 Project 不還原先前的捲動位置 | **已定**（最終修正波選擇「切換即歸零」） |
| 窄視窗往下捲時 stage 欄首會捲走（R32） | **使用者可推翻** |
| 兩則提示並存時，改綁提示的冰青比錯誤的紅字亮（D4 定案造成） | **只記錄**；要調整就得改 D4 |
| 前端要不要上 TypeScript | 使用者 2026-09-24 問過。建議：之後另開小 change，用 JSDoc＋`@ts-check` 檢查 Rust 投影與前端之間的型別一致 |
| `main` 上既有檔案含真實主機名／使用者名稱 | **未處理**：推上遠端之前另開一個小 change 清掉 |
| Codex stop review gate（per-repo）本 repo 未啟用 | 靠流程內手動跑 `adversarial-review` |
| Claude 在 feature 分支 commit，收尾時 squash 併回 main | **已授權** |

## 6. 之後的路

change 5 檔案瀏覽與 Review → change 6 進度模型 → change 7 畫面與操作修補 → 推 remote 前的去識別化小 change →
評估 Tauri 桌面殼（ADR-0005）。北極星見 `docs/cockpit-spec.md`。

## 版本紀錄

| 版本 | 日期 | 變更 |
|---|---|---|
| 1–10 | 2026-09-13～15 | change 1a／1b（詳見 git log 與 archive 目錄） |
| 11–14 | 2026-09-15～19 | change 2 propose／apply／驗收／併回 `main` 並 archive |
| 15–17 | 2026-09-21～23 | change 3 `live-output`：探測 → propose → SDD apply → 驗收 → 併回 `main` 並 archive |
| 18–19 | 2026-09-23～25 | change 4 `direction-01-visual`：brainstorming → propose → SDD 執行至 3.4 |
| 20 | 2026-09-26 | change 4 完成 3.4–5.5、使用者目視驗收通過、squash 併回 `main` 並 archive；路線重排為 change 5 檔案瀏覽與 Review、change 6 進度模型、change 7 畫面與操作修補 |
