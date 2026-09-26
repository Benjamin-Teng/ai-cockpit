# 交接：下一段任務

> **建立日期**：2026-09-25　|　**上一段做完的事**：change 4 `direction-01-visual` 以 SDD 執行到 task 3.4。完成 8/16：1.1、1.2、2.1–2.3、3.1–3.3。
> 3.4 停在 fix round 1 之後，還剩一條測試 finding。使用者要求在這裡收尾、換 session。
> **性質**：接手用文件，會過期，每段重寫。
> 為什麼做看 `openspec/changes/direction-01-visual/proposal.md` 與 `docs/direction-01-visual-design.md`；
> 怎麼做看 `~/.claude/CLAUDE.md`（本 repo 精簡版在 `AGENTS.md`）；規格看 `openspec/specs/` 與本 change 的 delta。

## 0. 三十秒版本

1. **沒有時效性任務。** 分支 `feat/direction-01-visual`，領先 `main`（`f6e0892`）71 個 commit；沒有 remote。
2. **接手第一步**：先讀 SDD ledger `.superpowers/sdd/tasks/progress.md`（本機檔案、不在版控）。裡面有第 1–36 條裁決、
   每個 task 的審查紀錄、所有延後項目，以及最後一行的 `PAUSE` 說明。**ledger 與 `git log` 優先於你的記憶。**
3. **下一步**：使用者下 `/opsx:apply` 之後，用 Skill 工具載入 `superpowers:subagent-driven-development`（它會認得既有 ledger，
   從 3.4 繼續），接著做 **3.4 fix round 2**（見第 2 節），然後是 4.1 → 4.2 → 5.1 → 5.2（截圖給使用者目視）→ 5.3 → 5.4 → 5.5。
4. 使用者嫌實作太久，所以才換 session。延續這個節奏：**只剩 Minor 就不再開修正輪**（裁決 R25），改記進 ledger，交給 5.1 清理。

## 1. 現在的狀態

- **已上線（`main`）**：change 1a／1b／2／3。各模組職責見 `cockpit/README.md`、`herdr-client/README.md`；依賴方向見 ADR-0003。
- **本分支已完成的畫面**（前端在 `cockpit/assets/`，以 `include_str!` 編進執行檔）：
  - 2.1 外框：`.shell` grid。`#app` 設 `display: contents`，各區塊帶 `data-region`（topbar／projects／banner／floor／runtimes／events／statusbar／output）。
    每個區塊都是「不捲的框＋內層捲動容器」。三個斷點＋高度門檻（寬 ≥1200 且高 ≥720 才固定一屏）。
    `paint()` 會記下並寫回內層捲動位置（`render.js` 的 `SCROLL_KEEP_SELECTORS`）。
  - 2.2 token：只准 10 個色彩 token 與 `--fs-*` 四階字級（20／14／13／12）。`visual-check.js` 的 TK1 靜態段會擋寫死的色值與字級。
  - 2.3 頂列與底列：
    - 每個 runtime 有連線燈號，三態三種形狀（實心圓／空心圓／叉）。
    - 通道斷線時，燈號標「最後已知」並改用 `--text-dim`。
    - 頂列高度由字級推導（`@property` 註冊的 `--shell-topbar-h`）。
    - 可整頁捲動的版面裡，底列是 sticky。
  - 3.1 左欄 Project 切換：計數 chip、`select-project`（不遞增 `latestOp`、不清錯誤）。選取的 Project 消失時，正式改選第一個。
  - 3.2 Factory Floor：
    - 節點是表面色底加狀態色條，再配「符號＋文字」。
    - running 用 2px 冰青外框加柔光；failed、blocked 用 1px 狀態色框。
    - 刻度畫在 stage 欄首上緣；列首與欄首 sticky。
  - 3.3 右欄：
    - 整區一層框，runtime 之間用分隔線。
    - pane 列排成兩行；cwd 省略前段、保留尾段。
    - HERDR 目前聚焦的 pane 標「作用中」。
  - 3.4（未勾）：錯誤與改綁提示同屬一套「左緣條」外觀；錯誤提示加 ✕；按鈕不換行。
- **尚未做的畫面**：Live Output 面板常駐與空狀態（4.1）。目前沒選 pane 時，中下區是空的，這是刻意保留，見裁決 R16。過期標示也還沒改版（4.2）。
- **可用指令**（repo 根目錄）：
  - 全 gate：`cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test --workspace && cargo test -p cockpit --example ui_preview && markdownlint-cli2 "**/*.md" && openspec validate --all`
  - 預覽（不需要 HERDR）：`cargo run -p cockpit --example ui_preview`（預設 `127.0.0.1:7770`）
  - 新驗收腳本：`node docs/research/2026-09-23/visual-check.js <段代號,...>`。段代號與用法見檔頭，常用的有
    TK1、S1–S5、V1–V4、G1、G2、P1、R1、CH1、D1、U1、CT1、RM1、FN1、LO1。
  - 既有六支腳本：`docs/research/2026-09-15/reconnect-check.js`、`whatever-check.js`、`docs/research/2026-09-16/actions-check.js`、
    `channel-backoff-check.js`、`factory-floor-check.js`、`docs/research/2026-09-19/live-output-check.js`
  - Codex 審查：`node ~/.claude/plugins/cache/openai-codex/codex/1.0.6/scripts/codex-companion.mjs adversarial-review --wait --scope branch --base <BASE> "<focus>"`
- **測試數字**：以當場輸出為準。最後紀錄（2026-09-25，`883c1f6`）：`cargo test -p cockpit` 156 passed／0 failed／1 ignored；
  六支腳本全 PASS；visual-check 除 LO1 以外全綠，另有一條 PEND(4.1)；markdownlint 80 檔 0 issues；`openspec validate --all` 17 passed。
  task 1.1 的 workspace 基線是 531／0／10。
- **環境**：乾淨，沒有殘留的 `ui_preview` 或 headless Chrome。port 7680（svchost）與 7778（ArmouryCrate）是系統服務，不是殘留，不要碰。

## 2. 立刻要做：task 3.4 fix round 2

- **唯一開著的 finding**（Codex，原文在 `.superpowers/sdd/tasks/codex-3.4-r1.log`）：
  - `visual-check.js` 的 banner-wrap 子段沒有證明「文字真的折行而且完整可見」：它只量按鈕，也只測 error 的「關閉」，沒測 rebind 的「取消」。
  - 負對照同時改了按鈕的 `white-space` 和 `flex-shrink`，所以兩項保護各自有沒有辨識力，證明不出來。
- **做法**：派一個新的 sonnet 實作者（原實作者的 context 不會跨 session）。給它下列檔案：
  - brief：`.superpowers/sdd/tasks/task-3.4-brief.md`
  - 報告：`.superpowers/sdd/tasks/task-3.4-report.md`，已有 fix round 1 章節
  - 上面那份 Codex log

  要它做到：
  - 斷言長文字節點存在，並以行高或元素高度證明至少兩行。
  - 斷言文字沒被裁切、沒有水平溢位。
  - error 與 rebind 兩個按鈕都要量。
  - 補「文字 nowrap」的負對照，並把按鈕的 `white-space` 與 `flex-shrink` 拆成兩個獨立負對照。

  可以順手做 ledger 裡兩條 3.4 Minor：✕ 對齊第一行；✕ 與文字的間距／兩則提示文字起點對齊。
- **審查**：
  - Codex scoped re-review，`--base 883c1f6`。
  - 設計審核：派新的 opus，先用 Skill 載入 `frontend-design:frontend-design`。
  - 通過就勾 3.4；只剩 Minor 就記進 ledger，不再開輪。
- 通則照舊：每個產品 task 都要讓六支既有腳本全綠。跑過 `factory-floor-check.js` 之後一律執行
  `git checkout -- docs/research/2026-09-16/task-5.2-scenario-d.png`。

## 3. 接著要做：4.1 → 5.5

- **4.1 Live Output 常駐**：沒選取時顯示空狀態；按鈕已改名「取消選取」，見 live-output spec。
  - 空狀態文案見 design D7。
  - `live-output-check.js` 中「沒選取時面板不存在／收起」的斷言要改寫成空狀態。
  - visual-check 裡 R2 的 `PENDING(4.1)` 與 LO1 在這個 task 轉綠。
  - 帶給 4.1 的事項：Live Output 的輸出窗要撐滿區塊（2.1 設計 M6）。
- **4.2 過期標示**：拿掉 opacity，改用 `--text-dim` 文字＋`--warn` 左緣條＋「過期」字樣（design D7）。
  WSL 真機腳本 `live-output-real-check.js` 的兩處 opacity 斷言也一起改，只改不跑。
- **5.1 清理**：刪掉沒被用到的舊 CSS，確認沒有 `@keyframes`。同時處理 ledger 裡所有交給 5.1 的延後項目（grep `5.1`）：
  - **R28**：寬 ≥1200 但高 <720 時，頂列可能多行，而高度上限只算單行。最小修法：寬 ≥1200 時頂列一律固定高度＋橫向捲動。
  - 底列 32px 寫死。
  - 連線符號 8px 應改 em。
  - 內層捲動容器要明設 `scrollbar-width: thin`。
  - 3.3 的兩條：連線明細值欄中文尾字掉行（`text-wrap: pretty`）；cwd 沒有前段時基線偏移。
  - 3.4 的兩條 ✕ 對齊（如果 3.4 沒順手做）。
  - 頂列上下框線不對稱（2.3 設計 M2，裁決 R23 當時決定不修；5.1 再看要不要順手處理）。
- **5.2**：visual-check 全跑。在 1536×1024、1100、700 三種寬度各截一張圖**給使用者目視**，等使用者點頭。
- **5.3**：`.gitignore` 加 `.superpowers/`；更新 `docs/direction-01-visual-design.md` 的狀態；新增 `docs/research/2026-09-23/visual-check.md`。
- **5.4**：全 gate，加上 `adversarial-review --base main` 的整支分支審查（也就是 SDD 的 final review）。
  - final review 要逐條處理 ledger 裡所有 `minor (deferred)` 與 `parked`。
  - 錯誤文案目前以 HTTP 碼和 API 路徑開頭（3.4 設計 M1），要決定是否留給後續 change。
- **5.5**：使用者點頭後重寫本檔。archive 時比照 change 3，把 ledger 複製成 `sdd-ledger.md` 放進 archive 目錄。

## 4. 這一段踩過的坑（不要再推導一次）

（**不會報錯的錯誤**加粗。）

流程與工具：

- **Codex log 只有最後的「# Codex Adversarial Review」結論段才算數**：開頭的 `verdict: approve` 可能只是占位；
  斷線（`connection closed`）時程序仍然 exit 0。讀結果一律用 `sed -n '/^# Codex/,$p'`，取不到就重跑。見 memory `codex-review-verdict-only-valid-in-final-section`。
- Codex 在唯讀沙箱只能靜態推導，**findings 一律先重現才採信**。它也曾被證實判斷錯誤，例如 3.2「網格比面板寬」實測不成立。
- 規格對照（sonnet）與 Codex 意見相反時，**查 spec 原文**再裁決。例：3.1 的 actions-check「10 次改 7 次」，spec 原文寫 10 次。
- session 與 Codex 額度會中斷 subagent，本段中斷過 4 次。遇到時用 SendMessage 續派同一個 agent，告訴它目前未提交的狀態，讓它從中斷處接著做。
- 開發者腳本的同類 finding 連續三輪還在冒，就換設計，不要再補洞。例：visual-check 的 finalSweep 改以 ChildProcess 判斷所有權；
  長名稱檢查改成「捲動歸零、只比水平」；頂列溢出放棄 +N 徽章，改純 CSS。
- `task-brief` 腳本讀不懂 OpenSpec 的 checkbox 格式。brief 用 `.superpowers/sdd/tasks/mkbrief.sh <task> "<ruling ids>"` 產生，
  再手寫「前面 task 帶來的事項」一節。**不要用 grep 自動抽取**，抽出來的東西太雜。

前端（本段新發現）：

- **整頁 `replaceChildren` 重畫會丟掉只存在 DOM 上的狀態**：內層捲動會歸零；旁路更新（例如 `onChannel` 直接改的文字）會被蓋回預設值。
  見 memory `full-repaint-discards-state-held-only-in-dom`。
- **首頁靜態占位內容帶有等待條件用的 class，等待條件就會在首份投影前提早成立**：這是 visual-check 偶發失敗的根因。
  現在一律用 `waitForFirstProjection()`（頂列要有 `[data-runtime]`，而且 `#version` 等於 `/api/state`）。
- **`factory-floor-check.js` 每跑一次都會覆寫已進版控的 `docs/research/2026-09-16/task-5.2-scenario-d.png`**，跑完要 `git checkout --` 還原。
- **flex `justify-content: flex-end` 往起始方向（向左）溢出時，`scrollWidth` 量不到**：要比 `getBoundingClientRect().left`。
- **CSS 多段註解中間斷開，會讓整條規則被瀏覽器吞掉，而且沒有任何錯誤**：改完 CSS 要到 CSSOM 確認規則還在。
- **`width: max-content` 計算 flex-wrap 容器時會當成 nowrap**，面板會被撐得太寬。
- **sticky 元素會蓋住被 `scrollIntoView` 捲到邊緣的目標**：用 `scroll-padding` 修（3.2）。
- 截圖**不可以隱藏捲軸**，否則設計審核會誤判。
- `pointerdown` 加 `preventDefault` 之後，按下態不要依賴 `:active`。

change 3 以前仍然有效的坑（HERDR 行為、axum、WSL、Git Bash），見 `git show e6163f6:docs/handover.md` 第 4 節。

## 5. 已定但未執行的決策

| 決策 | 狀態 |
|---|---|
| 路線：change 4 視覺改版 → change 5 .md 瀏覽 → change 6 Live Output 上色＋M2 | **已定**（使用者 2026-09-23） |
| 外觀改動要多一道 frontend-design 設計審核（設計文件與 spec 優先，skill 只管剩下的自由度） | **已定**（使用者 2026-09-24），見 memory |
| 本 change 的使用者裁決：　主標題＝Project 名；　高度門檻 720；　切角只在兩個面板，刻度在 stage 欄首上緣；　面板按鈕「取消選取」；　Factory Floor 在寬矮版面有高度上限；　通道斷線時燈號標「最後已知」；　三種連線形狀；　右欄整區一層框、卡片之間用分隔線；　TK1 同類繞過只記 Minor | **已定且已寫入** artifacts，見 ledger USER DECISION |
| 窄視窗往下捲時 stage 欄首會捲走（裁決 R32，維持 D3） | **使用者可推翻** |
| 兩則提示並存時，改綁提示的冰青比錯誤的紅字亮（D4 定案造成） | **只記錄**；要調整就得改 D4 |
| 前端要不要上 TypeScript | 使用者 2026-09-24 問過。建議：change 4 不做；之後可以另開小 change，用 JSDoc＋`@ts-check` 檢查 Rust 投影與前端之間的型別一致 |
| .md 瀏覽要用哪個 Markdown 渲染套件 | **未選**：change 5 brainstorming 時先問使用者 |
| M2 斷線期間無法取消改綁（`ProjectedBinding::RuntimeDisconnected` 不帶 `source`） | **排 change 6** |
| `main` 上既有檔案含真實主機名／使用者名稱 | **未處理**：推上遠端之前另開一個小 change 清掉 |
| Codex stop review gate（per-repo）本 repo 未啟用 | 靠流程內手動跑 `adversarial-review` |
| Claude 在 feature 分支 commit，收尾時 squash 併回 main | **已授權** |

## 6. 之後的路

change 4 收尾（併回 main 並 archive）→ change 5 .md 瀏覽 → change 6 上色＋M2 → 推 remote 前的去識別化小 change → 評估 Tauri 桌面殼（ADR-0005）。
北極星見 `docs/cockpit-spec.md`。

## 版本紀錄

| 版本 | 日期 | 變更 |
|---|---|---|
| 1–10 | 2026-09-13～15 | change 1a／1b（詳見 git log 與 archive 目錄） |
| 11–14 | 2026-09-15～19 | change 2 propose／apply／驗收／併回 `main` 並 archive |
| 15–17 | 2026-09-21～23 | change 3 `live-output`：探測 → propose → SDD apply → 驗收 → 併回 `main` 並 archive |
| 18 | 2026-09-23 | change 4 `direction-01-visual`：brainstorming → propose（SDD、16 task） |
| 19 | 2026-09-25 | change 4 SDD 執行至 3.4（8/16 完成）：前置設計審核與多項使用者裁決、36 條控制端裁決；使用者要求暫停換 session |
