# 交接：下一段任務

> **建立日期**：2026-09-28　|　**上一段做完的事**：change 5a `file-review` 完成——使用者目視驗收通過、squash 併回 `main`
> （`f56a463`）、3 份 delta spec 同步進主規格並 archive（`b5c936b`）。
> **性質**：接手用文件，會過期，每段重寫。
> 為什麼做看 `docs/cockpit-spec.md`（北極星）與各 change 的 proposal；怎麼做看 `~/.claude/CLAUDE.md`（本 repo 精簡版在
> `AGENTS.md`）；規格看 `openspec/specs/`。

## 0. 三十秒版本

1. **目前沒有 active change**，工作分支是 `main`（`feat/file-review` 已刪）。
2. **下一步：開 change 5b（git 唯讀層、diff、Git Graph）的 brainstorming**，第一題問「Git Graph 只看還是也能操作」，
   見第 3 節。開工前先開 feature 分支。
3. 待使用者決定、不擋 5b 的兩件事：沒有 `<meta charset>` 的 UTF-8 HTML 在檢視器會亂碼（要改 spec），以及 deferred
   lows 清單。兩者都在第 5 節。

## 1. 現在的狀態

- **已上線（`main`）**：change 1a／1b／2／3／4／5a。
  - change 5a 新增：
    - 新 crate `cockpit-files`（ADR `docs/adr/0006-cockpit-files-crate.md`；`cockpit-core` 不依賴它，方向見 ADR-0003）：
      根目錄推算、相對路徑界限、列目錄（`.gitignore` 過濾）、中繼資料與 viewer 分類、icon 對照、Markdown 渲染。
    - `cockpit` 後端：`/api/runtimes/<runtime>/panes/<pane>/root`、`/api/files/<runtime>/<root_id>/{list,meta,render,raw}`、
      `/vendor/<路徑>`（內嵌 pdf.js 與 Material Icon Theme）。端點與安全邊界詳細說明在 `cockpit/README.md`
      「檔案瀏覽與 Review」一節，不在這裡重複。
    - 前端：`/app/files.js`（檔案樹、檔案分頁）、`/app/viewers.js`（Markdown／純文字／HTML／PDF 檢視器），
      下半部分頁區（`#review`，Live Output 移入成為第一個分頁 + 檔案分頁）。
    - 驗收腳本 `docs/research/2026-09-27/files-check.js`（用法見同目錄 `files-check.md`），`visual-check.js` 補新段落。
  - change 4 的畫面規則（`.shell` 三欄外框、10 色 token、`--fs-*` 四階、Live Output 過期標示）仍然有效，見
    `git show 7aebc3b:docs/handover.md` 第 1 節。
- **archive**：`openspec/changes/archive/2026-09-28-file-review/`（change 5a），其中 `sdd-ledger.md` 是 SDD 全紀錄
  （裁決 R1–R30、每個 task 的審查輪次、deferred minors、待使用者決定事項）。上一個是
  `openspec/changes/archive/2026-09-26-direction-01-visual/`（change 4）。
- **可用指令**（repo 根目錄）：
  - 全 gate：`cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test --workspace && cargo test -p cockpit --example ui_preview && markdownlint-cli2 "**/*.md" && openspec validate --all`
  - 正式服務：`cargo run -p cockpit`（讀工作目錄的 `cockpit.toml`；沒有就是零設定模式，Factory Floor 為空）
  - 預覽（不需要 HERDR，資料寫死、按鈕只記錄不改狀態，含假 repo fixture）：
    `cargo run -p cockpit --example ui_preview`（`127.0.0.1:7770`）
  - 視覺驗收：`node docs/research/2026-09-23/visual-check.js [段代號,...]`，用法與段落表見同目錄 `visual-check.md`。
    打錯段代號會 exit 2。
  - 檔案瀏覽驗收：`node docs/research/2026-09-27/files-check.js [段代號,...]`，用法見同目錄 `files-check.md`。
  - 既有六支腳本：`docs/research/2026-09-15/reconnect-check.js`、`whatever-check.js`、`docs/research/2026-09-16/actions-check.js`、
    `channel-backoff-check.js`、`factory-floor-check.js`、`docs/research/2026-09-19/live-output-check.js`
  - WSL 真機：`docs/research/2026-09-19/live-output-real-check.js`（用法見 `live-output-acceptance.md`）；
    `--self-test-stale-signals` 可離線跑。
  - Codex 審查：`node ~/.claude/plugins/cache/openai-codex/codex/<最新版號>/scripts/codex-companion.mjs adversarial-review --wait --base <BASE> "<focus>"`
- **測試數字**：以當場輸出為準。最後紀錄（2026-09-28，併回前全 gate）：
  `cargo test --workspace` 715 passed／0 failed／11 ignored；`ui_preview` example 31；
  `files-check.js` 26 段 ok 532／FAIL 0；`visual-check.js` ok 1648／FAIL 0；既有六支腳本全 PASS；
  `markdownlint-cli2` 0 issues；`openspec validate --all` 17 passed（archive 後新增 file-review 主規格，當場跑為準）。
- **環境**：port 7680（svchost）與 7778（ArmouryCrate）是系統服務，不要碰。change 4 移除 Windows 端 herdr 的
  `herdr-sidebar` plugin 的備份仍在（`%APPDATA%\herdr\*.bak-20260926-pre-sidebar-uninstall`），
  `%LOCALAPPDATA%\herdr\plugins\herdr-sidebar.removed-20260926`；`layout.ps1`、`new-space.ps1` 還留著提到 sidebar 的過期註解。

## 2. 立刻要做：change 5b brainstorming

使用者下指令後走 superpowers brainstorming，再 `/opsx:propose`。前置事實與要問的第一題見第 3 節「change 5b」。
開工前 `git switch -c feat/<slug>`。

## 3. 接著要做（依序）

### change 5b：git 唯讀層、diff、Git Graph（下一個要開 brainstorming 的 change）

- 第一題要問使用者：**Git Graph 只看還是也能操作**（決定範圍與複雜度）。
- **Windows 端 git 對 WSL repo 會被 dubious ownership 擋下**（`docs/research/2026-09-27/file-review-probe.md` §3 已實測
  `fatal: detected dubious ownership in repository`，未改 `safe.directory`）；5b 的 git 讀取層要先解這題（例如改在 WSL
  端執行 git、或設定 `safe.directory` 並評估風險），不能直接沿用 change 5a 的「Windows 端讀 WSL 路徑」做法。
- Git Graph 參考 `mhutchie/vscode-git-graph` 只能看外觀與功能，**不得取用程式碼**（LICENSE 禁止散布衍生作品）。
- 使用者仍建議不做 VS Code extension host（5a 已納入「在 VS Code 開啟」與分頁還原，Ctrl+P 與全 repo 搜尋不做）。
- 詞彙：使用者口中的 space＝HERDR workspace，artifact 一律寫 workspace。

### change 6「進度模型」（使用者 2026-09-26 定案，排在 5b 之後）

- 進度目前全靠人工按按鈕，看板會跟真實進度脫節。方向是讓 agent 自己呼叫 cockpit 的 API 回報進度，
  寫入的是 cockpit 自己的狀態檔，**不需要推翻「對 HERDR 唯讀」**。「HERDR 的 `done` 不等於 task 完成」這條原則保留。
- 活動狀態目前只到 workstream 層級（同一條 workstream 的 task 共用 pane 綁定，見 `CONTEXT.md`），要精確到 task。
- 推進要有「退回」操作。現在誤按只能停服務、手改 `cockpit.state.json`（見 `cockpit/README.md` 的推進限制）。

### change 7「畫面與操作修補」（排在 change 6 之後）

- Live Output 上色。
- M2：斷線期間無法取消改綁，因為 `ProjectedBinding::RuntimeDisconnected` 不帶 `source`。
- U1：滑鼠點按鈕或列之後會留下冰青焦點框，重畫後還在；選定的 pane 列因此像 running 節點。改法是 `:focus-visible`，
  要確認鍵盤焦點還原沒有退步。
- U2：cockpit 服務斷線時，只有頂列標「最後已知」，右欄 runtime 卡片仍用綠色顯示 connected。

多 runtime 通用支援不排（目前沒有第二個 runtime 要接，YAGNI；核心模型與執行環境已經分層，ADR-0003）。

## 4. 這一段（change 5a）踩過的坑

（**不會報錯的錯誤**加粗。全紀錄與裁決理由見 `openspec/changes/archive/2026-09-28-file-review/sdd-ledger.md`。）

流程與工具：

- **Codex 用量上限會中途用盡**：本段發生兩次（2026-09-27 19:49 與 2026-09-28 約 00:30），各停約 3 小時。對策：
  審查排隊、實作不停，只是未經 Codex 通過的 task 不勾選、不宣稱完成；恢復後依序補審，必要時把多個 task 合成一次
  `--base` 較早的審查（減少重疊）。
- 在 scratchpad 建 detached worktree（`git worktree add <scratchpad>/review-wt <commit>`）審特定 commit 範圍，
  避免審到進行中、還沒到那個範圍的改動。
- **Codex 有時只回開場白，不是結構化的審查輸出**（非 usage limit 導致）：判斷方式同 memory
  `codex-review-verdict-only-valid-in-final-section`（找不到「# Codex Adversarial Review」結論段），重跑即可。
- subagent 會自己開背景監看，回報「waiting on background work」不代表完成，要等它真正回報結果再往下走。
- 驗收腳本（`ui_preview` 系）不可並行跑，會共用暫存目錄互相干擾。

安全邊界與 Rust：

- **Windows 經 `\\wsl.localhost` 看 Linux 符號連結是無法跟隨的 reparse point**：`ResolveLinkTarget` 解不出目標、
  直接開檔失敗，`canonicalize` 因此失敗。WSL repo 內的符號連結（含指向根目錄內部的）一律視為跳出根目錄、一律
  403，這是 fail-closed 的功能限制不是漏洞（已寫 memory `herdr-schema-fields-may-be-inert.md` 同系列的環境認知，
  細節見 `docs/research/2026-09-27/file-review-probe.md` §6）。
- **axum 的 `{*path}` 會先把整段路徑 percent-decode 一次**，`a%2Fb` 解碼後變成兩段 `a/b`，若拿這個已解碼值做
  逐段界限檢查就會漏擋多重編碼的跳出嘗試。對策：從原始 URI 字串取路徑自己逐段解碼與檢查，不用 axum `Path`
  已解碼的值。
- **`ignore::WalkBuilder` 沿邏輯路徑走祖先鏈找 `.gitignore` 有多個同源漏洞**（規則檔本身是根外連結仍生效、
  祖先目錄被忽略時子項目卻仍列出等）：改成對每個路徑前綴分別 `canonicalize` 驗證是否在根內、用
  `GitignoreBuilder` 逐層以「實體目錄」載入規則、`fs::read_dir` 讀取，不吞任何錯誤（design D4 已依此改寫）。
  同類 finding 連續幾輪還在冒，就是設計方向錯了要換，不要逐條補洞。
- **先看 metadata 的 size 再整檔讀取，會被並行改寫繞過大小上限**（檢查通過後、讀取前檔案被換成更大的檔案）：
  改成有上限的讀取函式（`read_capped`），一路讀一路數位元組，超過就中止；前端串流／逐段讀同理。
- comrak 0.55 的選項是 `extension.header_id_prefix`（不是原設計文件寫的 `header_ids`，那是舊版 API），
  且會替每個標題自動加一個 `<a href="#未加前綴的id" class="anchor">`——這個 anchor 的 `href` 不含前綴，
  跟標題本身的 `id`（有前綴）不一致，若要改寫成應用內錨點連結要注意這個落差。
- 本機建不了檔案 symlink（`os error 1314`，權限不足）→ 測試改用 junction（`mklink /J`）驗證同一套邊界邏輯。

其他：

- VS Code 的 `vscode://vscode-remote/wsl+<distro>/<path>` 連結**結尾要加 `:1`**（行號），否則 VS Code 把遠端路徑
  當成資料夾開啟，不是開檔（實測見 probe §5）。
- Git Bash 裡打 `python3` 會叫出 Windows Store 的 stub 卡住，不會真的執行 Python。

change 5a 以前仍然有效的坑（HERDR 行為、change 4 前端、axum、WSL、Git Bash 等），見上一版
`git show 7aebc3b:docs/handover.md` 第 4 節（再往前的鏈結見該節末的指標）。

## 5. 已定但未執行的決策

| 決策 | 狀態 |
|---|---|
| 路線：change 5a 檔案瀏覽與 Review（已完成、已併回並 archive）→ change 5b git 唯讀層／diff／Git Graph → change 6 進度模型 → change 7 畫面與操作修補 | **已定** |
| 沒有 `<meta charset>` 的 UTF-8 HTML 在 iframe 會亂碼——spec 規定 raw `.html` 回 `text/html` 不帶 charset | **待使用者決定**：修正需改 spec（例如改回 `text/html; charset=utf-8`） |
| HTML 檢視器改用「隱藏 iframe 先載入、Resource Timing 確認 200 才換掉舊 iframe」（R30，取代最初的 R28/R29 兩次請求做法） | **已定**（實測可行，5.4 修正波已套用），理由：同時維持 spec 字面（iframe 載入原始內容端點）又消除 R28/R29 的競態；不做 `srcdoc`（會改寫使用者 HTML，違反 design D8「不改寫使用者內容」） |
| Markdown 以 `/` 開頭的相對連結以 repo 根目錄為基準（同 GitHub）；前端另加元素／屬性白名單清洗，縱深防禦 comrak 未來新增輸出元素 | **已定**（R25） |
| 允許清單範圍是「目前有 pane 的整個 repo」，shell pane 停在家目錄／磁碟根會讓整個範圍進允許清單 | **已定不額外限制**（只對本機同源、只讀，等同使用者在該 pane 本來能讀到的範圍） |
| PDF 解析失敗算「成功讀取後的一種顯示內容」，不算讀取失敗、不上過期標示 | **已定**（R26） |
| Git Graph：只當參考、不取用程式碼（授權禁止散布衍生作品） | **已定**（控制端查證 LICENSE） |
| 不做 VS Code extension host | **建議**，使用者尚未最終表態 |
| 多 runtime 通用支援 | **不做**，等真的有第二個 runtime 再開 |
| deferred lows（Codex 分級可延後，不擋合併）：405 缺 `Allow` 標頭（3.2）；`ui_preview` 外層寫入路由 405 空本體；1100 寬 Factory Floor 頂緣低 12px；1200×720 固定一屏時 Live Output 面板 227.6px；單欄開檔時分頁區高度在中繼資料回來前跳動一次；tablist 內含關閉 button 的 ARIA 結構（`role=presentation` 包裝）；PDF 固定倍率下 devicePixelRatio 改變不立即重畫；過期標示與中繼資料成功重疊時偶發短暫閃爍；Markdown 內文缺最大行寬（已在最終修正波補約 80ch，若之後嫌不夠可再調） | **延後**，細節見 `openspec/changes/archive/2026-09-28-file-review/sdd-ledger.md` |
| `main` 上既有檔案含真實主機名／使用者名稱 | **未處理**：推上遠端之前另開一個小 change 清掉 |
| Codex stop review gate（per-repo）本 repo 未啟用 | 靠流程內手動跑 `adversarial-review` |
| Claude 在 feature 分支 commit，收尾時 squash 併回 main | **已授權** |
| 前端要不要上 TypeScript | 使用者 2026-09-24 問過，尚未定案。建議：之後另開小 change，用 JSDoc＋`@ts-check` |

## 6. 之後的路

change 5a 檔案瀏覽與 Review（已完成）→ change 5b git 唯讀層／diff／Git Graph → change 6 進度模型 →
change 7 畫面與操作修補 → 推 remote 前的去識別化小 change → 評估 Tauri 桌面殼（ADR-0005）。
北極星見 `docs/cockpit-spec.md`。

## 版本紀錄

| 版本 | 日期 | 變更 |
|---|---|---|
| 1–10 | 2026-09-13～15 | change 1a／1b（詳見 git log 與 archive 目錄） |
| 11–14 | 2026-09-15～19 | change 2 propose／apply／驗收／併回 `main` 並 archive |
| 15–17 | 2026-09-21～23 | change 3 `live-output`：探測 → propose → SDD apply → 驗收 → 併回 `main` 並 archive |
| 18–19 | 2026-09-23～25 | change 4 `direction-01-visual`：brainstorming → propose → SDD 執行至 3.4 |
| 20 | 2026-09-26 | change 4 完成 3.4–5.5、使用者目視驗收通過、squash 併回 `main` 並 archive；路線重排為 change 5 檔案瀏覽與 Review、change 6 進度模型、change 7 畫面與操作修補 |
| 21 | 2026-09-27 | change 5 brainstorming（拆 5a／5b）、5a `file-review` propose 完成，開分支 `feat/file-review` |
| 22 | 2026-09-28 | change 5a `file-review` SDD apply 完成（task 1.1–5.4，整支分支 Codex review 通過），待使用者目視驗收（5.5） |
| 23 | 2026-09-28 | change 5a 使用者目視驗收通過、squash 併回 `main`、3 份 delta spec 同步並 archive；下一段為 change 5b brainstorming |
