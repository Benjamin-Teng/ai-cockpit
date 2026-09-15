# 交接：下一段任務

> **建立日期**：2026-09-15　|　**上一段做完的事**：change 1b `attach-herdr-runtimes` 以 SDD 完成 30 個
> task 中的 27 個（第 1–3 組 24 個程式 task、4.3、4.5、4.6），三個 crate `cockpit-core`／`cockpit-herdr`／
> `cockpit` 全部建好並在真機（Windows HERDR 0.9.0 ＋ WSL 0.8.2 測試 server）通過 Scenario A／B（WSL 端）／F
> 的自動化驗收，已 squash 併回 `main` 並 archive；4.1、4.2、4.4 只剩使用者親自做的部分。
> **性質**：接手用文件，會過期，每段重寫。
> 為什麼做看 `docs/cockpit-spec.md` 與設計文件 `docs/superpowers/specs/2026-09-13-cockpit-mvp-design.md`、
> 怎麼做看 `~/.claude/CLAUDE.md`（本 repo 精簡版在 `AGENTS.md`）、1b 的 task 清單（含未勾三項）看
> `openspec/changes/archive/2026-09-15-attach-herdr-runtimes/tasks.md`；change 2 的待辦要等 propose 產出。

## 0. 三十秒版本

1. **分支已收尾**（2026-09-15，使用者遠端授權）：squash 併回 `main`（`2a4893b`）、分支已刪、change 已
   archive 到 `openspec/changes/archive/2026-09-15-attach-herdr-runtimes/`（含去識別化的 `sdd-ledger.md`），
   7 份 delta spec 已同步成 `openspec/specs/` 的主規格；本機 `.superpowers/sdd/` 已刪。
2. **使用者親自做的四件事**（第 2 節有指令與判讀），做完把結果補進
   `docs/research/2026-09-15/change-1b-acceptance.md` 對應小節並勾 archive 內 tasks.md 的 4.1／4.2／4.4；
   使用者已決不放寬標準、不擋 change 2，但 4.4 的 Windows 端結論在做完前會一直是「無法判定」。
3. 下一步：change 2 `pipeline-projection`——先口頭討論範圍（第 3 節），再 `/opsx:propose`。沒有時效性任務。
4. 環境：WSL 端 headless 測試 server **已停**（下次真機測試用第 1 節指令啟動；多輪 tab 開關後會累積補推，
   跑驗收腳本前重啟一次）；本機 `cockpit.toml`（gitignored）已指向兩側；`target/release/cockpit.exe` 對應
   commit `d58ea18`（分支上最後的程式碼版本，與 main 的 `2a4893b` 內容相同；三個驗收腳本在這版重跑皆 PASS）。

## 1. 現在的狀態

- 已產出（分支上，尚未併回 main）：
  - `cockpit-core/`：與 runtime 種類無關的核心。`src/types/`（id newtype、`AgentStatus` 五值、
    `RuntimeSnapshot`／`Workspace`／`Tab`／`Pane`／`Agent`、`RuntimeEvent` 與 `FocusChange`、
    `ConnectionState`）、`src/runtime.rs`（`AgentRuntime` trait、`RuntimeEvents` mpsc 事件流、
    `RuntimeError::{Unavailable{retry_after},Failed}`）、`src/store.rs`（`RuntimeStore`：登記、整份替換、
    逐筆套用、Drift、連鎖刪除、50 筆最近事件）、`src/projection.rs`（`ProjectedState` JSON）、
    `src/handle.rs`（`StoreHandle`：Mutex＋Notify＋50 ms 合併＋`watch`）、`src/driver.rs`（驅動器：
    訂閱 → authoritative snapshot → 套用；Drift 立即重拿並合併進行中；定期重拿；退避 1,2,4,8,16,30；
    `Unavailable` 固定間隔）。
  - `cockpit-herdr/`：HERDR 接合。`translate.rs`（snapshot／事件翻譯，26 種事件對照）、`probe.rs`
    （WSL 發行版探測，NUL 判別解碼）、`runtime.rs`（`HerdrRuntime`：探測 → seed snapshot → L → S，
    per-pane 狀態訂閱 200 ms 去抖動重開、generation 排序、一次性 `Shutdown`）、`factory.rs`
    （`build(HerdrEndpoint, BuildOptions)`，`cockpit` 不直接依賴 `herdr-client`）。
  - `cockpit/`：程式本體。`config.rs`（`--config`／cwd `cockpit.toml`／零設定，TOML
    `deny_unknown_fields`）、`runtimes.rs`（設定 → runtime 清單）、`http.rs`（axum：`/`、`/app/*`、
    `/api/state`、`/ws` 全量推送、PWA manifest／icons，全部 `include_str!`／`include_bytes!`）、
    `app.rs`（可注入 shutdown 的 `run_with_shutdown`）、`assets/`（純 JS `render.js`／`channel.js`、
    `style.css`、manifest、stdlib 產生的 PNG icon）、`examples/ui_preview.rs`（fixture 驅動的畫面預覽）、
    `README.md`（啟動、設定、單一執行檔、PWA、真機測試、驗收腳本）。
  - `herdr-client/` 加法：`testing` 的 `with_method_responses`、`FakeHerdr::closed_connections()`、
    `Step::Hold` 語意改為「讀到對端關閉為止」。
  - 驗收紀錄與腳本：`docs/research/2026-09-15/`（`change-1b-acceptance.md`、`acceptance_common.py`＋
    `attach-check.py`／`live-state-check.py`／`reconnect-real-check.py`、headless Chrome 的
    `whatever-check.js`／`reconnect-check.js`、`ui-preview.png`）。
- 可用指令（可直接複製執行；repo 根）：
  - 全 workspace gate：`cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test --workspace && markdownlint-cli2 "**/*.md"`（markdownlint 確認 `Linting: N files` 不為 0）
  - 啟動：`cargo run -p cockpit -- --config cockpit.toml`（零設定：`cargo run -p cockpit`；日誌 `RUST_LOG=debug`）；release：`cargo build --release -p cockpit`
  - 畫面預覽（不需 HERDR）：`cargo run -p cockpit --example ui_preview`
  - 真機 ignored 測試（只連 Windows 端、唯讀）：`cargo test -p cockpit --test real_attach -- --ignored --test-threads=1`
  - 驗收腳本（都要先 release build；Python 走 `uv run --no-project`，Windows 主控台加 `PYTHONUTF8=1`）：
    - Scenario A（唯讀）：`uv run --no-project python docs/research/2026-09-15/attach-check.py`
    - Scenario B WSL 端（會對 WSL 測試 server 建／關 tab）：`HERDR_CLIENT_TEST_ALLOW_WSL_WRITES=1 uv run --no-project python docs/research/2026-09-15/live-state-check.py`
    - Scenario F（會停／重啟 WSL 測試 server）：`HERDR_CLIENT_TEST_ALLOW_WSL_WRITES=1 COCKPIT_ACCEPT_STOP_WSL_DISTRO=Ubuntu-24.04 uv run --no-project python docs/research/2026-09-15/reconnect-real-check.py`
  - WSL 端測試 server 啟動：`wsl.exe -d Ubuntu-24.04 -e bash -lc "setsid -f ~/.local/bin/herdr server >/tmp/herdr-server.log 2>&1 </dev/null"`；停：`wsl.exe -d Ubuntu-24.04 -e bash -lc "~/.local/bin/herdr server stop"`（**只能停 WSL 端**）
  - Windows 端事件擷取（唯讀）：`cargo run -p herdr-client --example capture_events -- --seconds 20`
  - OpenSpec：`openspec list`（目前沒有進行中的 change）、`openspec validate --all`（11 份主規格；SHALL/MUST
    警告不算 ERROR）；change 2 propose 後改用 `openspec status --change pipeline-projection`
  - Codex scoped review：`node ~/.claude/plugins/cache/openai-codex/codex/1.0.6/scripts/codex-companion.mjs adversarial-review "<focus>"`（沒有 `--help`；focus 字串不要放反引號，bash 會當命令替換）
- 測試與 gate：以當場輸出為準。最終修正波後（`d58ea18`）`cargo test --workspace` 為 327 passed／0 failed／
  10 ignored（真機 ignored）；fmt／clippy `-D warnings`／markdownlint（`Linting: 37 files`）皆 0；
  `openspec validate --all` 11 passed／0 failed（只有 SHALL/MUST 警告）；
  去識別化 grep（tasks.md 4.6 那條：使用者名稱、email、家目錄；對 `docs/research/2026-09-15/`、
  `cockpit/tests/fixtures/`、`cockpit.example.toml`）為空。
- 版控：`main` 在 1a 收尾的 `a575842` 之後多了 `2a4893b`（squash 併入 1b 分支的 66 個 commit）與
  archive commit（見 `git log`）；feature 分支已刪；沒有 remote。SDD ledger（每輪 review、fix round、
  全部 `Ruling:`、deferred minor、使用者決定）永久存於
  `openspec/changes/archive/2026-09-15-attach-herdr-runtimes/sdd-ledger.md`。

## 2. 立刻要做：change 1b 的使用者驗收（分支收尾已完成）

分支收尾已於 2026-09-15 做完（squash 併回、archive、規格同步、分支刪除）；下面第 1–3 點留作紀錄，
供下一次 change 收尾照抄：

1. 最終全分支 Codex review（對 `34dd6bc`）已做：無 Critical；Important 三條——(F1) `HerdrRuntime::subscribe()`
   最後無條件寫回初始 S handle，可能蓋掉 200 ms 去抖動內已提交的新一代 S handle（兩條 S 同時送事件）；
   (F2) `run_with_shutdown` 只 drop 停止把手、不 await 驅動器也不停投影任務（嵌入或重複啟停會漏 task）；
   (F3) 4.1／4.2／4.4 未勾（使用者項目）。F1、F2 已修（`069ac89` 初始 handle 只在 `state.handle` 為
   `None` 時安裝、否則 abort 自己；`198d397`＋`d58ea18` 抽出公開的 `shutdown_components(stops, drivers,
   projector, driver_timeout)`：drop 停止把手 → 逐一 `timeout(&mut driver)`，逾時 `abort()` 後仍 `await`
   → `projector.abort()` 後 `await`，所有 return 路徑都走它），Codex scoped re-review 兩輪後 approve
   （結果與最後一輪的 verdict 見 ledger）。Minor／deferred：`channel.js` 在 `onopen` 就把退避歸零（server
   反覆接受後立刻斷線會以 1 秒重連）、`JSON.parse` 沒包 try/catch（只吃自家 server 的合法 JSON）、
   driver 逾時逐一計算最壞 10 s×N、逾時後的 `await` 沒有上限（被 abort 的 task 若卡在阻塞 `Drop` 會
   一直等，目前驅動器沒有這種程式碼）。
2. 已做：`git switch main && git merge --squash feat/attach-herdr-runtimes && git commit -F <msg>`
   （`2a4893b`），`git diff main <branch> --stat` 為空後 `git branch -D`。
3. 已做：archive——ledger 去識別化複製成 `sdd-ledger.md`；7 份 delta spec 都是純 ADDED，同步只需把標題
   `# <cap>（delta）` 改 `# <cap> Specification`、`## ADDED Requirements` 改 `## Requirements` 後放到
   `openspec/specs/<cap>/spec.md`；change 目錄 `git mv` 到 `openspec/changes/archive/2026-09-15-attach-herdr-runtimes/`；
   刪本機 `.superpowers/sdd/`。tasks.md 內 4.1／4.2／4.4 仍未勾（使用者已決照實保留）。

使用者親自做的四件事（判讀寫在括號內；結果補進 acceptance.md 對應小節）：

1. **spike 4 目視**：在 HERDR 的某個 pane 內執行 `target\release\cockpit.exe`（或 `cargo run -p cockpit`），
   看有沒有跳出新視窗（正常：沒有任何新視窗，只在該 pane 印 `listening on 127.0.0.1:7770`；若跳窗，
   `CREATE_NO_WINDOW` 旗標失效，走 `/opsx:update`）。
2. **PWA 安裝**：Chrome 開 `http://127.0.0.1:7770/`，網址列右側或選單應有「安裝 Cockpit」（正常：可安裝成
   獨立視窗；沒有安裝選項時先看 DevTools Application → Manifest 的錯誤）。
3. **Scenario B Windows 端**：在 Windows 端某個 agent pane 對 agent 下一句指令，畫面上該 pane 應在一秒內
   變 `working`、頁尾最近事件出現 `pane.agent_status_changed`（正常：≤1 s；超過 1 s 或沒有事件 → 看
   `RUST_LOG=debug` 的 S 訂閱日誌，可能是 per-pane 訂閱沒開成）。Cockpit 不會送任何 prompt 給 agent。
4. **4.4 定案**：在 Windows 端開兩三個 tab 再關掉，**立刻**跑
   `cargo run -p herdr-client --example capture_events -- --seconds 20`，看 L 的前幾行有沒有指向剛關掉的
   tab／pane 的事件（有 → Windows 0.9.0 也會補推，開 cockpit 確認畫面沒有幽靈 pane，並把設計文件 §2.3
   的「無法判定」改成結論；沒有 → 結論寫「Windows 0.9.0 不補推」）。

## 3. 接著要做：change 2 `pipeline-projection` 的 propose

為什麼：MVP 第二片（設計文件 §1 表：spec §20 的 6、7、9 與 12 的 domain 層 Failed／Completed；驗收
Scenario C、D）。流程與 1b 相同：探索走 brainstorming，計畫一律 `/opsx:propose pipeline-projection`，
apply 第一個動作載入 `superpowers:subagent-driven-development`（memory `apply-phase-must-run-through-sdd-skill`）。

propose 前置：

- 讀設計文件 §12「change 2、3 從第一天就要遵守的約束」（binding 不綁 pane id；不寫 HERDR metadata；
  usage 面板不估算）與 §6.4（投影 JSON 是 change 2 domain 層要疊上去的基底）。
- 1b 已回寫設計文件（commit `5bfcb11`）：§4.1／§4.2／§7.1 驅動器歸屬與 NUL 判別、§6.1 `RuntimeEvent`
  變體、§6.2 `subscribe()` 簽章、§6.4 `protocol_warning` 與 `recent_events` 規則、§7.2 事件對照、§9
  「server is shutting down」。change 2 的 spec 直接引用這些，不要再從 1b 的 delta spec 抄。

change 2 要直接沿用、不用重新推導的 1b 產出：

- **驅動器介面**：`cockpit_core::runtime::AgentRuntime`（`snapshot()`／`subscribe() -> RuntimeEvents`）
  與 `driver::run(runtime, store, policy, stop)`；新 runtime 種類只要實作 trait，驅動器不改。
- **狀態與投影**：`StoreHandle::subscribe()` 給 `watch::Receiver<Arc<ProjectedState>>`；`ProjectedState`
  的 JSON 形狀（§6.4）是 `/api/state` 與 `/ws` 的合約，change 2 加 domain 層時**加欄位不改既有欄位**。
- **前端介面**：`channel.js` 暴露 `onState(state)`／`onChannel(status)`，`render.js` 是純函數
  `renderState(state) -> DOM`，整頁重畫；change 2 的 Factory Floor 視覺沿用這個模式。
- **設定**：TOML `deny_unknown_fields`，來源 `--config` → cwd → 零設定；change 2 的 pipeline 設定加在同一
  個檔，全域欄位錯誤以「區段.欄位名」識別。
- **真機事實（1b 驗收得到）**：HERDR 0.8.2 關 tab 後會把 Sidebar pane 換 id、並補推指向已關 tab 的事件
  （Drift → 重拿，2 秒內收斂）；Windows 0.9.0 在 20 秒擷取內沒看到補推（前提未滿足，見第 2 節第 4
  項）；`report_agent state=idle` 這次呈現 `idle`（1a 曾見 `done`，兩者都可能）；狀態變化不伴隨
  `workspace_updated`，tab／workspace 彙總狀態要等下一次 snapshot；HERDR 優雅關閉時對進行中的
  `session.snapshot` 回 `server_unavailable: server is shutting down`，斷線首個原因可能是它而不是 L／S
  EOF。
- **設計上留下的 deferred minor**（不擋 1b，change 2 若碰到同區域再處理）：Drift 觸發的重拿 snapshot 可能
  比重拿期間已套用的事件舊（WSL 走 `wsl.exe`＋`nc`，比 L 串流慢），會短暫蓋掉較新的焦點／label，靠後續
  事件或定期重拿收斂——嚴格解法是「重拿期間有事件套用就再拿一次（設上限）」；`Shutdown.aborts` 每次重開
  只增不減；幾個依真實時間的測試（reopen 1.67 s、loop_integration 2.5 s、重疊 350 ms）慢 CI 可能 flaky；
  F4 session guard 無直接測試；`Path::exists()` 因權限回 false 的情境無可攜測試；`Unavailable` 只從
  subscribe 來源測過、`retry_after` 0 無下限；`run_with_shutdown` 收不到 shutdown 結果的 bail 分支無測試；
  tokio-tungstenite dev 依賴與 axum 內部 tungstenite 版本不同（lock 兩版，只當測試客戶端）。

## 4. 這一段踩過的坑（不要再推導一次）

（本段新增；**不會報錯的錯誤**加粗）

- **Drift 重拿的 snapshot 可能比進行中套用的事件舊，投影會短暫倒退**（焦點、label），沒有任何錯誤日誌；
  實測 2 秒內由後續事件或再一次 Drift 收斂。驗收腳本因此不能在關 tab 後 0.5 s 就比對，要輪詢到一致並
  記錄秒數。
- **同一個 WSL 0.8.2 測試 server 連續多輪 tab 開關後會累積補推**：關 tab N 時補推 tab N-1 的
  `tab_upserted`＋`tab_removed`，更早幾輪的零星事件也會冒出來，每筆都 Drift → 重拿（WSL 端每次數百
  ms），狀態延遲從 0.1 s 劣化到 1.3 s。正解：跑驗收腳本前重啟 WSL 測試 server；不要拿有積壓的執行當
  延遲證據。
- **HERDR 0.8.2 關 tab 後會重建 Sidebar pane（換 id）**：「pane 集合回到 baseline」這種斷言必失敗；正解
  是與重新取得的 snapshot 逐欄位比對。
- **HERDR 優雅關閉時，進行中的 `session.snapshot` 會收到 `server_unavailable: server is shutting down`**，
  所以斷線的首個原因不一定是 L／S EOF；驅動器兩者都當斷線，測試與文件不要寫死「首次原因是 L 或 S」。
- **殘留的 `cockpit.exe` 會佔住 7770**（本段發現一個從凌晨跑到下午的）：三個腳本啟動前都會檢查
  `/api/state` 是否已有回應並拒絕啟動，看到「已有服務在跑」先 `netstat -ano | grep 7770` 找 PID。
  **Git Bash 下 `taskkill /PID` 會被 MSYS 路徑轉換吃掉旗標**（變成 `C:/Program Files/Git/PID`），
  要加 `MSYS_NO_PATHCONV=1`；Python 的 `subprocess` 不經過這層，腳本內不用。
- **ReopenStatus 的 200 ms 去抖動窗**：pane 建立後 200 ms 內的狀態變化不會有 `pane.agent_status_changed`
  （S 尚未重開），狀態經 L 的 `pane_updated` 到達；真實 agent 不會這麼快，腳本等 0.5 s 再報告。
- **`tokio::sync::watch::Sender::send` 在沒有 receiver 時直接丟掉**：投影任務要用 `send_replace`，否則
  第一個 WS 客戶端連上前的狀態全部遺失（1.6 Codex 抓到）。
- **`tokio::time::interval` 第一個 tick 立即觸發**：預覽輪播要用 `interval_at(start + period, period)`
  （3.5）。
- **`ctrl_c().await.ok()` 會把「註冊 Ctrl-C 監聽失敗」吞成正常結束**：3.8 改成註冊失敗非零結束、shutdown
  future 可注入。
- **`Vec<AbortHandle>` 只增不減**（`Shutdown.aborts`）：每次重開 +2，長跑會慢慢長；deferred。
- **Chrome extension 沒連上時**用 headless Chrome：`chrome.exe --headless=new --dump-dom` 或
  `--remote-debugging-port` ＋ Node 22 內建 `WebSocket` 走 DevTools protocol（`reconnect-check.js`），
  截圖用 `--screenshot`；收尾要依 PID 殺，不要 `taskkill /IM chrome.exe`。
- **Windows 主控台 cp950**：Python 腳本的 `subprocess.run` 一律 `encoding="utf-8", errors="replace"`，
  `sys.stdout.reconfigure` 要先 `isinstance(sys.stdout, io.TextIOWrapper)`，外層 `PYTHONUTF8=1`。
- **SDD 的 `scripts/task-brief` 不認 `- [ ] X.Y` 格式**（只認 `Task N` 標題）：brief 由控制端手寫到
  `.superpowers/sdd/tasks/task-X.Y-brief.md`。
- **Codex focus 字串放反引號會被 bash 當命令替換**（無害但會跑奇怪指令）；`adversarial-review` 沒有
  `--help`，任何字串都會開一個 job。
- **Opus 實作者遇到 rate limit 會中途停**：先 `git status` 看工作樹是乾淨還是半成品，再用 `SendMessage`
  resume 同一個 agent（2.6、3.8 各一次），不要重派。
- **Codex 唯讀沙箱跑不了 cargo test**：測試證據一律來自實作者報告；Codex 只做靜態推導（本段它的
  findings 全部實測後才改，其中 1.1「共用依賴」、3.7「manifest scope」兩條裁決不改）。
- **`herdr status server --json`** 可用（`status`／`version`／`protocol`／`socket`），停 server 前用它驗證
  目標；`herdr api snapshot` 沒有旗標、一行 JSON。

（舊版仍有效的坑，保留）

- **`wsl.exe -e bash -lc '… &'` 起的程序會被 WSL 一起收掉**：要 `setsid -f`。
- **Git Bash 對 `wsl.exe` 的 `/home/...` 路徑做 MSYS 轉換**：手動測試加 `MSYS_NO_PATHCONV=1`；
  `tokio::process::Command` 不經過這層。
- **named pipe／unix socket「server 不回就關連線」對 client 只是乾淨 EOF**：client 自己判斷「EOF 但沒讀到
  完整回應」。
- **AF_UNIX 沒有 RST**：`Step::Abort` 在 unix 等同 `Close`，測試分平台斷言。
- **3 條並發 named pipe 連線會遇到 `ERROR_PIPE_BUSY`**：50 ms 重試、1 秒上限是必要的。
- **WSL 0.8.2 的 `herdr pane report-agent` CLI 多旗標會回 `unknown option`**：直接對 socket 送 JSON-RPC
  `pane.report_agent`／`pane.clear_agent_authority`（驗收腳本就是這樣做）。
- **rustdoc `compile_fail,E0xxx` 的錯誤碼不校驗**。
- **HERDR `config_dir()` 在 Windows 也看 `XDG_CONFIG_HOME`，空字串也算有設**（見 `herdr-client/README.md`）。
- **`String::from_utf8(bytes).is_ok()` 對 UTF-16LE 也會 `Ok`**：判斷要看有沒有 NUL byte（D8 就是這樣做）。
- **HERDR 沒有全域 agent 狀態訂閱**：每 pane 各訂一筆，pane 集合變了要重開（ReopenStatus 的由來）。
- **pane id 在 `pane.moved` 後會變**；**HERDR Windows 端是 named pipe 不是 TCP**；**每條 API 連線只服務
  一個 method**；**`done` 是「idle 且未被看過」**；**不得對 Windows 端 `herdr server stop`**；
  **markdownlint-cli2 在 repo 根跑並核對 `Linting: N files` 不是 0**。

## 5. 已定但未執行的決策

| 決策 | 狀態 |
|---|---|
| Claude 在 feature 分支 commit | **已授權**（2026-09-14，逐 task 一個 commit）；1b 的 squash 併回 main 與 archive **已於 2026-09-15 授權並執行**（見第 1 節版控） |
| 4.1／4.2／4.4 的使用者部分 | **待使用者**（第 2 節四件事）；使用者已決：維持未勾、不放寬標準、archive 照實帶警告；不擋 change 2 |
| deferred minor（第 3 節清單） | **已決全部帶進 change 2 待辦**，不單獨開 change |
| change 2 的啟動 | **已決**：先口頭討論範圍（2026-09-15 遠端），併回後再 `/opsx:propose` |
| spike 4 目視複驗 | **待使用者**（第 2 節第 1 項）；`CREATE_NO_WINDOW` 維持 |
| spike 5(a) 重驗（需 `wsl.exe --shutdown`） | **已決不做**，沿用 9/13 實測 |
| Codex review gate | **已開**；本段全程用路徑②（Bash 直接跑 `adversarial-review`）逐 task 審，額度沒有用盡 |
| WSL 端測試 server | **在跑**（2026-09-15）；跑驗收腳本前重啟一次清補推積壓；不用時可停 |
| Drift 重拿與進行中事件的競爭 | **不在 1b 修**（deferred，第 3 節）；change 2 若動驅動器一併處理 |
| 4.4 的 Windows 端結論 | **無法判定**，tasks.md 未勾；等使用者開關 tab 後重擷取 |
| 1.1 三個 crate 一次列齊共用依賴（Codex 建議按需加） | **已決保留**：dispatch 裁決明文要求，後續 task 不再動 `Cargo.toml` |
| 3.7 manifest `scope: "/"`（Codex low） | **已決保留**：標準 PWA 欄位、與 `start_url` 一致 |
| 4.3 首次斷線原因寫法 | **已改**：tasks.md 與設計文件 §9 改為「反映對端關閉（L／S 結束或 server 關閉中）」 |
| f2a「為偵測缺席而建模 Cockpit 用不到的 required 欄位」（1a） | **已否決**（D13 限縮為已建模的必填欄位） |
| Tauri | MVP 後再評估，不做 |

## 6. 之後的路

change 2 `pipeline-projection`（domain 層：pipeline TOML 設定、`RuntimeBinding` 以穩定特徵匹配、
`StageStatus` Failed／Completed、Factory Floor 視覺；Scenario C、D）→ change 3 `live-output`
（`pane.read` 以 `revision` 輪詢推送，change 1a 已建 `PaneReadParams`／`PaneReadResult`；WSL 端讀取
頻率超過每秒一次時改 ADR-0002 方案 B）→ MVP 完成後評估 Tauri 桌面殼（ADR-0005）。北極星與整體範圍見
`docs/cockpit-spec.md`。

## 版本紀錄

| 版本 | 日期 | 變更 |
|---|---|---|
| 1 | 2026-09-13 | 初版：brainstorming 完成、設計文件待審 |
| 2 | 2026-09-13 | 找碴審閱後：每 pane 狀態訂閱、change 1 拆 1a／1b、研究證據進 repo |
| 3 | 2026-09-14 | change 1a `herdr-client` 18 task 完成 17；SDD＋Codex 逐 task review 全程記錄於 `.superpowers/sdd/tasks/progress.md`；5.2 全 workspace gate 已全綠一次，Codex 全分支 review 回 3 Important＋1 low，修正進行中；記錄 spike 1–5 真機結論、D12／D13 兩個 review 觸發的設計修正、假 HERDR／子程序橋接測試機制、待使用者的 5 項決定 |
| 4 | 2026-09-14 | change 1a 全部完成：最終 review 修正波、Codex 額度用盡改替代審查、deferred minor 與待決事項 |
| 5 | 2026-09-14 | change 1a 併回 main 並 archive；下一步改為 1b propose；使用者五項決定的結果（commit 授權、只關 WSL server、spike 4 留 1b、開 review gate、全套收尾） |
| 6 | 2026-09-14 | change 1b propose 完成並經 fresh reviewer 修正、設計文件 §2.3 補欄位、分支 `feat/attach-herdr-runtimes` 建立；下一步改為 1b apply（SDD） |
| 7 | 2026-09-15 | change 1b apply：30 task 完成 27（三個 crate、真機 Scenario A／B／F 自動化證據、設計文件回寫）；Codex 逐 task review 全部通過；最終全分支 review 的兩條 Important（初始 S handle 覆蓋、shutdown 不等 task）已修並經兩輪 scoped re-review approve；4.1／4.2／4.4 剩使用者部分；新增本段 15 條坑與 deferred minor 清單 |
| 8 | 2026-09-15 | 使用者遠端四項決定後：1b squash 併回 main（`2a4893b`）、archive 並同步 7 份主規格、分支刪除、WSL 測試 server 停；下一步改為 change 2 範圍討論與 propose |
