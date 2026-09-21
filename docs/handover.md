# 交接：下一段任務

> **建立日期**：2026-09-21　|　**上一段做完的事**：change 3 `live-output` 在分支 `change-3-live-output` 實作完成，
> 全 gate 綠、逐組經 Codex 審過、WSL 與 Windows 兩端真機驗收完成；**尚未併回 `main`**，剩兩個待使用者決定的項目
> （真機驗收腳本的 3 個 parked finding、鍵盤焦點在重畫時消失）。Cockpit 現在能顯示選定 pane 的即時輸出。
> **性質**：接手用文件，會過期，每段重寫。
> 為什麼做看 `docs/cockpit-spec.md` 與設計文件 `docs/superpowers/specs/2026-09-13-cockpit-mvp-design.md`、
> 怎麼做看 `~/.claude/CLAUDE.md`（本 repo 精簡版在 `AGENTS.md`）、規格看 `openspec/specs/`（15 份主規格）。

## 0. 三十秒版本

1. **沒有時效性任務。** active change = `live-output`（進度一律現場跑 `openspec status --change live-output`，不要信這裡的數字）。
   程式與文件都在分支 `change-3-live-output`，`main` 還停在 change 2。
2. 併回 `main` 之前還差兩個決定，都需要使用者（第 5 節前兩列）：
   - **task 8.2**：整支分支的 Codex review 還留著 3 個 parked finding，全在
     `docs/research/2026-09-19/live-output-real-check.js`（只供開發者手動跑的真機腳本，不是產品碼）。要修或接受。
     產品碼的 findings 已全部 ADDRESSED。
   - **鍵盤焦點在整頁重畫時消失**（Windows 端驗收時發現）：會讓 spec 新增的「pane 列可用鍵盤選定」在真實的重畫頻率下
     幾乎用不到。要在 change 3 內修（`render.js` 重畫後還原焦點，順便解掉 M3），或接受並留給視覺改版。
   task 6.2（Windows 端驗收）已於 2026-09-21 由 Claude 代為執行並通過，紀錄在 `docs/research/2026-09-19/live-output-acceptance.md` 文末。
3. 環境是乾淨的（2026-09-21 核對）：WSL 測試 server 已停、沒有殘留的 `cockpit.exe`／`ui_preview.exe`／headless Chrome、
   7770／779x 沒有 LISTEN、本機 `cockpit.toml`（gitignored）沒被動過、沒有狀態檔。
4. 沒有 remote。

## 1. 現在的狀態

- 已上線（`main`）：change 1a／1b／2——`herdr-client/`、`cockpit-core/`、`cockpit-herdr/`、`cockpit/`，各模組職責見
  `git show main:docs/handover.md` 第 1 節，或 `cockpit/README.md`、`herdr-client/README.md`（另兩個 crate 沒有 README）；
  這一段沒有改變它們的分工。
- 在分支 `change-3-live-output` 上新增的：
  - `cockpit-core/src/runtime.rs`：`AgentRuntime::read_output(&PaneId, max_lines)`、`PaneOutput { format, text, truncated }`
    （刻意不含 `revision`）、`RuntimeError::PaneNotFound`。
  - `cockpit-herdr/src/runtime.rs`：以 `pane.read`（`recent`／`text`／`lines`＝上限）實作；`pane_not_found`→`PaneNotFound`，其餘→`Failed`；
    同一個 runtime 的輸出讀取以一把 `tokio::sync::Mutex` 排隊（不與 snapshot／subscribe 互斥）。
  - `cockpit/src/http.rs`：`GET /api/runtimes/{runtime}/panes/{pane}/output`，掛 `source_check`；狀態碼與標頭見
    `openspec/changes/live-output/specs/live-output/spec.md`；`AppState.runtimes` 是 runtime id → `Arc<dyn AgentRuntime>` 的對照表。
  - `cockpit/assets/app/output.js`（面板＋輪詢）、`actions.js`（`ui.selected`、選取不參與 `latestOp`）、`render.js`（pane 列可選、
    「看輸出」、`liveOutput.setKnownPanes`）；面板 `#output` 是 `#app` 的兄弟節點，不在整頁重畫範圍內。
  - `cockpit/examples/ui_preview.rs`：腳本化假 runtime（`COCKPIT_PREVIEW_OUTPUT_MODES`、`COCKPIT_PREVIEW_VANISH_PANE`，語法見檔頭）。
  - `herdr-client/examples/probe_pane_read.rs`：`pane.read` 真機探測工具（HERDR 升版時重跑）。
- 可用指令（repo 根）：
  - 全 gate：`cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test --workspace && cargo test -p cockpit --example ui_preview && markdownlint-cli2 "**/*.md" && openspec validate --all`
    （`cargo test --workspace` **不含** example 內的單元測試，所以 `ui_preview` 要另外跑）
  - 啟動：`cargo run -p cockpit -- --config cockpit.toml`；畫面預覽（不需 HERDR）：`cargo run -p cockpit --example ui_preview`
  - 輸出端點：`curl -H "Host: 127.0.0.1:7770" "http://127.0.0.1:7770/api/runtimes/<runtime>/panes/<pane>/output"`（要帶合法 `Host`；說明見 `cockpit/README.md`）
  - 前端驗收（headless Chrome，對 `ui_preview`）：`node docs/research/2026-09-19/live-output-check.js`（整支約 2.5 分鐘；只跑某幾段就加段落代號，例 `… Q`）；
    既有的 `node docs/research/2026-09-16/actions-check.js`、`factory-floor-check.js`
  - 真機驗收（WSL）：`docs/research/2026-09-19/live-output-real-check.js`，執行方式與必要的環境變數見檔頭與
    `docs/research/2026-09-19/live-output-acceptance.md`；**跑之前先看第 5 節第一列**
  - `pane.read` 探測：`cargo run -p herdr-client --example probe_pane_read -- --list`／`-- --pane <id> --count 20 --interval-ms 500 --metadata-only`
  - WSL 測試 server：啟動 `wsl.exe -d Ubuntu-24.04 -e bash -lc "setsid -f ~/.local/bin/herdr server >/tmp/herdr-server.log 2>&1 </dev/null"`；
    停 `wsl.exe -d Ubuntu-24.04 -e bash -lc "~/.local/bin/herdr server stop"`（**只能停 WSL 端**）
  - Codex review：`node ~/.claude/plugins/cache/openai-codex/codex/*/scripts/codex-companion.mjs adversarial-review --wait --base <ref> "<focus>"`
    （`*` 取最新版號；裝了多個版本時先 `ls ~/.claude/plugins/cache/openai-codex/codex/` 挑最新的那個寫死）
- 測試與 gate：以當場輸出為準。2026-09-21 於 `ef50620`：`cargo test --workspace` 531 passed／0 failed／10 ignored（ignored 需真機 HERDR）、
  `ui_preview` example 13 passed、fmt／clippy 0、markdownlint 73 files 0 issues、`openspec validate --all` 16 passed。
- 規格：主規格仍是 15 份（change 3 的 delta 還沒 sync）。delta 在 `openspec/changes/live-output/specs/`：新增 `live-output`；
  修改 `runtime-model`、`herdr-runtime-session`（含一條 RENAMED）、`herdr-observer-types`、`cockpit-dashboard`。
- 版控：`main` 在 `58f0e16`；分支 `change-3-live-output` 逐 task 一個 commit，收尾時 squash（數量現場看 `git log --oneline main..`）。

## 2. 立刻要做：收掉 change 3

### 2.1 task 6.2——Windows 端驗收：已完成

2026-09-21 由 Claude 以真實 Chrome 代為執行（使用者授權），`tasks.md` 6.2 的檢查項全數通過；逐項的量測值在
`docs/research/2026-09-19/live-output-acceptance.md` 的「Windows 端手動驗收（task 6.2）」。兩點要知道：

- 「在 HERDR 關掉被選的 pane」**沒有在 Windows 端做**（那是對使用者真實工作階段的寫入類操作，超出 Windows 端唯讀的約束）；
  同一情境已在 WSL 真機（task 6.1）與替身（`live-output-check.js` J／K 段）驗過。要親眼確認就手動關一個不要的 pane 觀察：
  面板應顯示「pane 已不存在」、內容變淡、不再發 `/output` 請求。
- 驗收用的是暫存目錄裡的設定檔（沿用 `cockpit.toml` 的 runtime、埠 7793、加一個測試 project）；要重做就照驗收文件開頭的說明，
  不要為了驗收去改 `cockpit.toml`。以瀏覽器自動化工具點擊時，頁面的 CSS 座標要換算成工具的座標框
  （當時是 1538／`window.innerWidth`≈0.6），否則會點到別的地方、看起來像「點了沒反應」。

### 2.2 兩個待決定的項目（見第 5 節前兩列），然後收尾

1. 兩個項目都處理完（修掉並經 Codex 複審，或使用者明示接受）→ 在 `tasks.md` 勾 8.2，`openspec status --change live-output` 應為全數完成。
2. 併回 `main`：照 `~/.claude/guides/git-branch-workflow.md` 的收尾清單，squash 成一個 commit
   （分支裡有兩個 commit 主旨有手誤、且早期 commit 含後來才匿名化的主機名，**一定要 squash**，不要保留逐 task 歷史）。
3. archive 走獨立的 commit：`/opsx:archive live-output`（先 sync 5 份 delta 進主規格；`herdr-runtime-session` 有一條 RENAMED，
   sync 後確認主規格的 requirement 標題是「只用唯讀 method」）。SDD ledger 在本機 `.superpowers/sdd/tasks/progress.md`（git-ignored），
   archive 前複製進 archive 目錄的 `sdd-ledger.md`，比照前兩個 change。
4. 重寫本檔。

## 3. 接著要做：Direction 01 視覺改版

**依據的分工（使用者 2026-09-21 裁決）：視覺設計完全參照 `docs/direction-01-visual-design.md`**（配色表、字體、關鍵語彙、元件語彙都以這份文件為準）；
概念圖 `docs/cockpit-dashboard-direction-01-concept.png` **只用來標示功能區的位置與排列**，不是配色、字體、間距或元件外觀的依據——
兩者有出入時以文件為準，不要從點陣圖取色或臨摹像素。概念圖上有、但產品還沒有的功能區（Add Agent、篩選、Flow／Details、底部輸入區等）
只是排列示意，不代表要在改版時實作。十種方向比較稿在 `docs/cockpit-art-directions.html`（僅供回顧選型過程）。現在的 `style.css` 仍是 1b 的樣式語彙，Live Output 面板也是沿用它做的。改版時一併處理：

- **Live Output 上色**：change 3 刻意只做純文字（使用者 2026-09-19 決定），端點回應已帶 `format` 欄位預留。`pane.read` 的 `ansi` 格式
  實測只有 SGR 序列（`docs/research/2026-09-19/pane-read-probe.md` 第 4 節；真彩色、背景色、全螢幕 TUI 的 `ansi` 輸出**未驗**），
  前端寫一個 SGR→樣式的小轉換器即可，不需要完整終端機模擬。256 色對應到畫面配色要在這裡一次定案。
- 兩個已知的使用性問題（第 5 節 M2／M3）。M3（整頁重畫使鍵盤焦點消失）對 Live Output 面板已不成立（面板在 `#app` 之外），
  但 `#app` 內的 pane 列、按鈕仍會。
- 前端改動的驗收一律用 headless Chrome 腳本；既有腳本依賴目前的 DOM 結構與 `data-action` 名稱，改版時要一起改。

## 4. 這一段踩過的坑（不要再推導一次）

（**不會報錯的錯誤**加粗。change 3 新增的在前，較早仍有效的在後。）

HERDR 行為：

- **`pane.read` 回應的 `revision` 在 WSL 0.8.2 與 Windows 0.9.0 都恆為 0**，內容怎麼變都不動 → 不能拿來去重或判新舊；
  靠它的話畫面讀到第一次就永遠凍結、沒有錯誤。見 memory `herdr-schema-fields-may-be-inert`。HERDR 升版後重跑探測筆記第 2 節。
- **`pane.read` 的 `lines` 從畫面格底端往上數、空白列也算**：給太小（例如 5）在畫面下半空白時回 0 bytes。要大於實際畫面高度（現用 200）。
  `recent` 的 server 端上限是 1000 行；全螢幕 TUI 沒有 scrollback（`recent`≈`visible`）。
- 單次 `pane.read`：WSL 約 42 ms、Windows 約 1.2 ms，與大小無關——文件原本估的「數百 ms」高估，每秒一次不需要 ADR-0002 方案 B。
- WSL 測試 server `stop` 再重啟後，**pane id 保留但 pane 內原本在跑的 shell 被殺掉、內容重置成新 prompt**——驗收腳本不能假設迴圈還在跑。
- 不存在的 pane 兩個版本都回錯誤碼 `pane_not_found`。

axum／HTTP：

- **`get(handler)` 會同時服務 `HEAD`**，`HEAD` 會真的進 handler（觸發一次讀取）；要擋就明確註冊 `.head(...)`。
- **`route_layer` 只包它被呼叫當下已註冊的方法插槽**，不包 fallback，也不包之後才註冊的方法：想讓某個 method 不經 middleware，
  就在 `.route_layer(...)` 之後才註冊它。
- **`Path` extractor 的 rejection（例如路徑片段 `%FF` 不是合法 UTF-8）會在 handler 之前回純文字 400**，不經自己的錯誤出口、沒有自訂標頭
  → handler 收 `Result<Path<..>, PathRejection>` 自己處理。
- matchit 把空字串當合法路徑片段：`/panes//output` 會命中 `{pane}` 路由。
- `error_response` 現在一律帶 `Cache-Control: no-store` 與 `X-Content-Type-Options: nosniff`（寫入端點的錯誤回應也跟著有，無害）。

前端與驗收腳本：

- **前端資源是 `include_str!` 編進執行檔的**：改了 `cockpit/assets/` 不重新 `cargo build -p cockpit --example ui_preview`，腳本跑的是舊前端，
  RED／GREEN 都是假的。
- **用固定 sleep 決定兩件事先後的測試沒有辨識力**（拿掉被測的防線照樣綠）：要驗「舊回應晚到被丟棄」，讓測試端持有假 `fetch` 的 resolver，
  等新內容顯示後才 resolve，再等一個可觀察的「已處理」標記。時間先後用同一個行程內的 `performance.now()`，**不要跨行程比時間戳**。
- **等高內容替換時瀏覽器不會動 `scrollTop`**：`ui_preview` 的 `long` 模式截斷後 `scrollHeight` 不再變，拿它驗「貼底跟著走」永遠綠；用 `ticker`＋矮視窗。
  「捲動位置不變」的基準值要用非零中段值（用 0 分不出「沒變」與「被重設」）。
- CDP：`scrollIntoView` 之後要 settle（現用 100 ms）再送合成滑鼠事件，否則偶發沒命中；點擊前要等元素出現（頁面剛載入、首份投影還沒畫）。
  `factory-floor-check.js` 的 `--dump-dom` 對殘留程序／CPU 敏感，會環境性假 FAIL——跑前清掉殘留的 preview／Chrome。
- 操作掛在 `pointerdown`（不是 `click`），所以按下到放開之間節點被整頁重畫換掉也不會丟點擊。
- `output.js` 換選取時會 abort 舊請求；「至多一個進行中請求」以原生 `fetch` 遵守 AbortSignal 為前提，腳本裡「忽略 abort 的假 `fetch`」只是
  模擬請求卡住的手法。abort 不算失敗（不標過期、不顯示原因）；世代序號仍保留（abort 不保證回應不會到）。
- **選取不是「畫面操作」**：`select-pane`／`select-bound-pane` 不遞增 `latestOp`、不清 `ui.error`，否則會讓進行中的寫入變 stale、其後的失敗被靜默吞掉。

環境與流程：

- **Git Bash 會吃掉未加引號的反斜線**：`> C:\Users\x\out.txt` 會在目前目錄建立名為 `C:Usersxout.txt` 的檔（曾在 repo 根留下垃圾檔）。
  路徑用正斜線或加雙引號。
- **`wsl.exe … bash -lc "setsid -f bash -c '…; exec <程式>'"` 若內層沒有自己重導向三個標準串流，約 1/4 機率整個行程憑空消失、exit 0 無錯誤**
  → 內層那條指令收尾加 `</dev/null >/dev/null 2>&1`。見 memory `wsl-nested-setsid-detach-races-with-caller-exit`。
- 多個 agent 同時工作時共用同一個 git index：控制端與文件實作者 commit 要用指定路徑（`git commit -m … -- <path>`），commit 後 `git show --stat` 核對。
- Claude 的 session 用量上限（HTTP 429）會讓 subagent 中途終止：要求實作者「每完成一項就 commit 並寫進 report」，回來用 SendMessage 續派同一個
  agent（context 還在）；context 已用到數十萬 token 的 agent 改派 fresh 的，brief 寫完整。
- **Codex 撞額度時仍會印出 `Verdict: approve`／`No material findings`**：看 log 有沒有 `usage limit`／`Turn failed`、以及 `[codex]` 行數
  （真的有跑會有數十行）。focus 字串不要放反引號。
- Codex 在唯讀沙箱只能跑 `cargo fmt --check`；findings 要先以 RED 測試重現才採信（這一段的產品碼 findings 全數重現成立）。
- `cargo test --workspace` 不跑 example 內的 `#[cfg(test)]`。
- 驗收門檻別自訂得比 spec 嚴。

（change 2 以前仍有效的坑，保留摘要）

- **HERDR 0.8.2 對每條新 `events.subscribe` 重播 server 啟動以來的整段事件歷史**，部分晚於 snapshot 到達、不觸發 Drift、投影靜默倒退
  → 已由「連線後沉降重拿」修正，WSL 端啟動／重啟後仍有最多約 5 秒不準。見 memory `herdr-subscribe-replays-event-history`。
- `tokio::test` 的 `start_paused` 與 `multi_thread` 互斥；走真實 named pipe／socket 的測試用真實時間。
- 寫入交易整段放進服務自己 `tokio::spawn` 的 task（呼叫端 future 被 drop 時鎖會提前釋放）；`try_send` 失敗不可忽略；狀態檔欄位必填、用 BTreeMap。
- `HeaderMap::get` 只取第一個值，來源檢查用 `get_all` 要求恰好一個；瀏覽器與 curl 會省略 80 埠；listen 不可用 80。
- 前端並發請求要配遞增序號；`new_with_domain` 要在 runtime 登記之後建；bind 失敗的所有 return 路徑走 `shutdown_all`。
- Drift 重拿的 snapshot 可能比進行中的事件舊；HERDR 關 tab 後會重建 Sidebar pane（換 id），pane id 不重用。
- 殘留 `cockpit.exe` 佔 7770：`netstat -ano | grep 7770`；Git Bash 下 `taskkill /PID` 要 `MSYS_NO_PATHCONV=1`。
- Windows 主控台 cp950：Python `encoding="utf-8"`＋`PYTHONUTF8=1`；headless Chrome 依 PID 收尾。
- named pipe 3 條並發會 `ERROR_PIPE_BUSY`；每條 API 連線只服務一個 method；HERDR 沒有全域 agent 狀態訂閱。
- `done` 是「idle 且未被看過」，不是 Completed；**不得對 Windows 端 `herdr server stop`**。
- markdownlint-cli2 在 repo 根跑並核對 `Linting: N files` 不為 0。

## 5. 已定但未執行的決策

| 決策 | 狀態 |
|---|---|
| 真機腳本 `live-output-real-check.js` 的 3 個 Codex finding：①比對 PID 與全域 `herdr server stop` 之間有 TOCTOU（high）②`verifyOwnership` 回 false 後主流程仍繼續（medium）③`--verify-ownership-mismatch` 掛勾繞過 `HERDR_CLIENT_TEST_ALLOW_WSL_WRITES` 檢查（medium） | **待使用者決定**（task 8.2 因此未勾）。②③各是幾行的修正；①建議改成 `kill -TERM <ownedPid>` 並等待，需先真機確認 HERDR 對 SIGTERM 會刪 socket。修了要再過一次 Codex。風險評估：單人開發機、WSL 測試 server 依 `AGENTS.md` 本來就可停 |
| **鍵盤焦點在整頁重畫時消失，影響 change 3 新增的「pane 列可用鍵盤選定」**：pane 列可聚焦、Enter／Space 的處理正確，但投影每次更新都整頁重畫 `#app`，被聚焦的節點被換掉、焦點掉回 `<body>`（Windows 端實測約 5 秒一次）。`live-output-check.js` 的鍵盤情境是聚焦後數毫秒內按鍵，抓不到 | **待使用者決定**：在 change 3 內修（`render.js` 在 `replaceChildren` 前記下 `document.activeElement` 的 `data-*` 識別、重畫後找回新節點 `focus()`，一併解掉 M3；要補一個「聚焦→等一次重畫→按 Enter」的 CDP 情境並過 Codex），或接受現狀、連同 M3 留給視覺改版（那樣 spec「鍵盤選定」要加註已知限制） |
| `main` 上既有檔案含真實主機名／使用者名稱／路徑（`docs/research/2026-09-13/change-1a-spikes.md`、`herdr-client/tests/real_herdr.rs`、`herdr-client/README.md`、`cockpit/tests/config.rs` 等） | **未處理**：不在 change 3 範圍、archive 不該動。沒有 remote 前風險低；要推上遠端前另開一個小 change 清 |
| Live Output 上色（`format=ansi`） | **已決留給 Direction 01**（第 3 節） |
| 輪詢間隔 1 秒、行數 200、服務端逾時 5 秒、前端逾時 6 秒 | **寫死**；有需求再開設定 |
| WSL 端每秒多次 `pane.read` 是否改 ADR-0002 方案 B | **已結案：不需要**（實測 42 ms）。更高頻率或多 pane 同看時再評估 |
| `/ws` 可被任意網站以 WebSocket 讀取投影 | **已知不修**；輸出不走 `/ws`，風險沒有因 change 3 變大 |
| 輸出端點不檢查 `Sec-Fetch-Site`：無 `Origin` 的跨站 GET 會進到 handler 觸發一次讀取，但讀不到回應 | **已決接受**（design D7） |
| 空 pane 路徑段（`/panes//output`）會把空 pane id 送到 runtime，最後回 404 | **不修**（Codex 判定可保留；日後可收緊成 400） |
| `ui_preview` 先 bind 再驗證環境變數的 pane id；CDP 腳本複製了產品的貼底門檻 6px；`.selected` 與 `.bind-target` 疊加的視覺無斷言 | **不修**（Codex 判定可保留） |
| 斷線期間畫面沒有「取消改綁」按鈕（M2）；每次推送整頁重畫使 `#app` 內的鍵盤焦點消失（M3） | **留待視覺改版** |
| change 2 的 Codex 補審 | **已決不做**（2026-09-19 使用者接受 opus 替代審查） |
| Claude 在 feature 分支 commit、收尾 squash 併回 main | **已授權** |
| `[state] path` 父目錄不存在→每次寫入 500；大寫 `LOCALHOST` 的 Host 被 403；`with_write_hook` 可注入會 panic 的 callback；同 tick 就緒時 Drift 多拿一次 snapshot；真實時間測試不改 paused time；tokio-tungstenite 兩版並存 | **不修／不處理**（理由見 `git show main:docs/handover.md` 第 5 節） |
| per-pane S 訂閱重開是否也重播歷史；Windows 0.9.0 是否會重播歷史 | **未驗**；升版時重跑 `capture_events` |
| Tauri 桌面殼 | MVP 完成後再評估（ADR-0005） |

## 6. 之後的路

change 3 收掉（6.2、8.2 → squash 併回 → archive）→ Direction 01 視覺改版（含 Live Output 上色）→ MVP 完成後評估 Tauri 桌面殼
（ADR-0005）。北極星與整體範圍見 `docs/cockpit-spec.md`；每個 change 的歷史決策見 `openspec/changes/archive/*/`
（含各自的 `sdd-ledger.md`，裡面有所有 Ruling 與 parked findings）。change 3 的 ledger 目前在本機
`.superpowers/sdd/tasks/progress.md`（archive 時要複製進去）。

## 版本紀錄

| 版本 | 日期 | 變更 |
|---|---|---|
| 1–10 | 2026-09-13～15 | change 1a／1b（詳見 git log 與 archive 目錄） |
| 11–14 | 2026-09-15～19 | change 2 propose／apply／驗收／併回 `main` 並 archive |
| 15 | 2026-09-21 | change 3 `live-output`：探測推翻 `revision` 前提 → propose → SDD apply（Codex 逐組審、產品碼 findings 全數重現並修正） |
| 16 | 2026-09-21 | task 6.2 Windows 端驗收由 Claude 代為執行並通過；發現鍵盤焦點在重畫時消失；剩 8.2 與該發現待使用者決定 |
