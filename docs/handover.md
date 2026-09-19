# 交接：下一段任務

> **建立日期**：2026-09-19　|　**上一段做完的事**：change 2 `pipeline-projection` 全部 27 個 task 完成，
> 已 squash 併回 `main`（`e33de5b`）、8 份 delta spec 同步進主規格、change 目錄 archive 到
> `openspec/changes/archive/2026-09-19-pipeline-projection/`。Cockpit 現在能顯示並操作自己管理的 Pipeline 進度。
> **性質**：接手用文件，會過期，每段重寫。
> 為什麼做看 `docs/cockpit-spec.md` 與設計文件 `docs/superpowers/specs/2026-09-13-cockpit-mvp-design.md`、
> 怎麼做看 `~/.claude/CLAUDE.md`（本 repo 精簡版在 `AGENTS.md`）、規格看 `openspec/specs/`（15 份主規格）。

## 0. 三十秒版本

1. **沒有時效性任務，也沒有進行中的 change**（`openspec list` 為空）。下一段＝change 3 `live-output`
   （把 pane 畫面內容拉進 Cockpit），流程從探索（brainstorming）→ `/opsx:propose live-output` 開始。
2. change 2 的 Codex 補審**已決定不做**（使用者 2026-09-19 明示接受 fresh opus 的替代審查）。之後若想補，
   focus 要自己重寫（本機 `.superpowers/sdd/` 已刪，ledger 存在 archive 目錄的 `sdd-ledger.md`）。
3. 環境：WSL 測試 server 已停；本機 `cockpit.toml`（gitignored）末尾留有 change 2 驗收用的 `[state]` 與
   `[[project]]` 區段，不需要時整段刪除即可（刪掉後 `cockpit.state.json` 也可刪）。
4. `main` 乾淨、沒有 remote、沒有 feature 分支。

## 1. 現在的狀態

- 已上線（`main`）：
  - `herdr-client/`：HERDR JSON-RPC 客戶端（named pipe／unix socket、WSL 橋接、假 HERDR 測試鷹架）。
  - `cockpit-core/`：與 runtime 種類無關的核心。`types/`、`store.rs`（狀態庫、Drift）、`handle.rs`
    （`StoreHandle` 同鎖持 runtime＋domain、投影任務、失效覆蓋去重送出）、`projection.rs`（`ProjectedState`，
    含 `projects[]`）、`driver.rs`（連線生命週期：訂閱→snapshot→套用、Drift 重拿＋追加、**連線後沉降重拿**、
    退避）、`domain/`（Pipeline 型別、`apply_op`、`resolve_binding`／`validate_override`、`derive_status`）。
  - `cockpit-herdr/`：HERDR 接合（snapshot／事件翻譯、WSL 探測、per-pane 狀態訂閱重開、`factory.rs`）。
  - `cockpit/`：程式本體。`config.rs`（`[[runtime]]`／`[[project]]`／`[state]`）、`progress.rs`（狀態檔載入）、
    `progress_service.rs`（寫入交易：計算→`.tmp`→rename→生效）、`http.rs`＋`source_check.rs`（GET 路由、
    寫入端點、Host／Origin 檢查）、`app.rs`（組裝與停止）、`assets/app/`（`render.js` Factory Floor、
    `actions.js` 互動、`channel.js` 通道）、`examples/ui_preview.rs`。
- 可用指令（repo 根）：
  - 全 gate：`cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test --workspace && markdownlint-cli2 "**/*.md" && openspec validate --all`
  - 啟動：`cargo run -p cockpit -- --config cockpit.toml`（零設定：`cargo run -p cockpit`；日誌 `RUST_LOG=debug`）
  - 畫面預覽（不需 HERDR）：`cargo run -p cockpit --example ui_preview`
  - 寫入 API（要帶合法 Host）：`curl -X POST -H "Host: 127.0.0.1:7770" http://127.0.0.1:7770/api/projects/<pid>/tasks/<tid>/advance`
    （op＝advance／complete／fail／clear；覆蓋 `PUT`／`DELETE …/workstreams/<wid>/override`，本體
    `{"runtime": "...", "pane_id": "..."}`；成功 204）
  - 真機驗收腳本：`docs/research/2026-09-15/`（Scenario A／B／F）、`docs/research/2026-09-16/`
    （`pipeline-check.py` Scenario C／D、三支 headless Chrome 腳本）。WSL 腳本要
    `HERDR_CLIENT_TEST_ALLOW_WSL_WRITES=1`＋`PYTHONUTF8=1`，跑前重啟 WSL 測試 server。
  - WSL 測試 server：啟動 `wsl.exe -d Ubuntu-24.04 -e bash -lc "setsid -f ~/.local/bin/herdr server >/tmp/herdr-server.log 2>&1 </dev/null"`；
    停 `wsl.exe -d Ubuntu-24.04 -e bash -lc "~/.local/bin/herdr server stop"`（**只能停 WSL 端**）
  - Windows 端唯讀事件擷取：`cargo run -p herdr-client --example capture_events -- --seconds 5`
  - Codex review：`node ~/.claude/plugins/cache/openai-codex/codex/1.0.6/scripts/codex-companion.mjs adversarial-review --wait --base main "<focus>"`
- 測試與 gate：以當場輸出為準。2026-09-19 於 `b231456`：`cargo test --workspace` 494 passed／0 failed／
  10 ignored（ignored 需真機 HERDR）、fmt／clippy 0、markdownlint 63 files 0 issues、
  `openspec validate --all` 15 passed／0 failed。
- 規格：`openspec/specs/` 15 份。change 2 新增 `pipeline-config`、`pipeline-domain`、`pipeline-progress`、
  `runtime-binding`；修改 `cockpit-config`、`cockpit-dashboard`、`runtime-driver`、`state-projection`。
- 版控：`main` 在 `b231456`（change 2 的 `e33de5b`＋archive `4cfec16`＋Purpose `b231456`）；沒有 remote。

## 2. 立刻要做：change 3 `live-output` 的探索與 propose

為什麼：MVP 第三片——畫面目前只看得到「誰在動」，看不到 agent 在講什麼。要把 pane 的畫面內容拉進 Cockpit
（設計文件 §1 表、`docs/cockpit-spec.md`）。

propose 前要讀的（不要重新推導）：

1. `docs/adr/ADR-0002`（拉取頻率與方案 A／B 的取捨）、設計文件對 `pane.read` 的段落。
2. change 1a 已建好的 `herdr-client` 型別 `PaneReadParams`／`PaneReadResult`（含 `revision`）——不用新寫協定層。
3. **重播 bug 對新訂閱的影響**（memory `herdr-subscribe-replays-event-history`）：輸出輪詢若要另開連線，注意
   WSL 端每條新 `events.subscribe` 會重播歷史；`pane.read` 是請求／回應，不受影響，但任何新訂閱都要考慮。
4. WSL 端讀取成本：`wsl.exe`＋socket 每次數百 ms，頻率超過每秒一次要改 ADR-0002 方案 B。
5. 既有約束（設計文件 §12）：對 HERDR 唯讀、不寫 metadata、usage 面板不估算。

流程：探索走 `superpowers:brainstorming`；計畫一律 `/opsx:propose live-output`；apply 第一個動作載入
`superpowers:subagent-driven-development`（memory `apply-phase-must-run-through-sdd-skill`）。

## 3. 接著要做：Direction 01 視覺改版

已選定的美術方向在 `docs/direction-01-visual-design.md`（配色表、關鍵語彙）與
`docs/cockpit-dashboard-direction-01-concept.png`（完整概念圖，功能區塊以它為準），十種方向比較稿在
`docs/cockpit-art-directions.html`。change 2 刻意**沒有**套用（使用者 2026-09-16 決定），現在的
`style.css` 仍是 1b 的樣式語彙。改版時一併處理兩個已知的使用性問題（第 5 節 M2／M3）。

## 4. 這一段踩過的坑（不要再推導一次）

（**不會報錯的錯誤**加粗。change 2 新增的在前，較早仍有效的在後。）

- **HERDR 0.8.2 對每條新 `events.subscribe` 重播 server 啟動以來的整段事件歷史（舊值），部分晚於 snapshot 到達、不觸發
  Drift，投影靜默倒退**（label 變空、agent unknown、binding unbound）→ 已由「連線後沉降重拿」修正，WSL 端啟動／重啟後
  仍有最多約 5 s 不準（實測 2.3／4.2 s）。驗收時限要以沉降上限為準，別訂更嚴的門檻。Windows 0.9.0 唯讀擷取未見重播。
  詳見 memory `herdr-subscribe-replays-event-history`。
- **Codex 撞額度時仍會印出 `Verdict: approve`／`No material findings`**：看 log 的 `usage limit`／`Turn failed`。
- **`tokio::test` 的 `start_paused` 與 `multi_thread` 互斥（巨集編譯錯誤）；走真實 named pipe／socket 的測試用 paused
  time 會讓 timeout 在真 connect 完成前觸發**：`cockpit-herdr/tests/reopen.rs`、`loop_integration.rs` 維持真實時間。
- **寫入交易若由呼叫端 future 持鎖等 `spawn_blocking`，客戶端斷線（axum drop handler）會讓鎖提前釋放、寫檔 detach
  繼續跑，與下一筆寫入交錯**：交易整段放進服務自己 `tokio::spawn` 的 task。
- **`try_send` 到 bounded channel 失敗被忽略 → 失效覆蓋永遠不刪、重啟復活**：改 unbounded＋投影端去重。
- **狀態檔 `#[serde(default)]` 讓不完整檔案被當合法 → 靜默重設進度**：projects／tasks／overrides 必填。
- **`HashMap` 序列化 key 順序每次不同**（狀態檔 diff 雜訊）→ BTreeMap。
- **`HeaderMap::get` 只取第一個值**：來源檢查用 `get_all` 要求恰好一個 Host／Origin。
- **瀏覽器與 curl 會把 80 埠從 Host 省略（網址寫 `:80` 也一樣）**：寫入端點比對「Host＋實際埠」，listen 不可用 80、
  也不可用 127.0.0.1／localhost／[::1] 以外的 loopback。
- **原始碼字串比對的「行為測試」擋不住錯誤實作**：前端行為一律用 headless Chrome（CDP）腳本驗，顏色要比
  `getComputedStyle`，不是只比 class。
- **前端並發請求：較慢的舊請求失敗會蓋掉較新操作的畫面狀態**：每次操作配遞增序號，只有最新的能寫錯誤。
- **`new_with_domain` 若在 runtime 登記前建立，第一份投影（version 1）缺 runtimes、恢復的覆蓋解析成不可用**：先登記再建。
- **bind 失敗時已 spawn 的 projector／stale-remover 會 detach 洩漏**：所有 return 路徑走 `shutdown_all`。
- **平行派多個實作者時**：各自只 `git add` 自己的檔、tasks.md 由控制端統一勾；`cargo fmt -p` 會順手改到別人進行中的檔。
- **驗收門檻別自訂得比 spec 嚴**（7.2 曾寫 3 s，與 spec 上限 5 s 矛盾而假 FAIL）。

（1b 以前仍有效的坑，保留摘要）

- Drift 重拿的 snapshot 可能比進行中套用的事件舊，投影短暫倒退，驗收要輪詢到一致。
- HERDR 0.8.2 關 tab 後會重建 Sidebar pane（換 id）；pane id 不重用（關 `p5` 後新 pane 是 `p6`）。
- 優雅關閉時進行中的 `session.snapshot` 回 `server_unavailable: server is shutting down`。
- 殘留 `cockpit.exe` 佔 7770：`netstat -ano | grep 7770` 找 PID；Git Bash 下 `taskkill /PID` 要 `MSYS_NO_PATHCONV=1`。
- `watch::Sender::send` 無 receiver 時丟值 → `send_replace`；`tokio::time::interval` 第一個 tick 立即觸發。
- Windows 主控台 cp950：Python `encoding="utf-8"`＋`PYTHONUTF8=1`；headless Chrome 依 PID 收尾。
- `wsl.exe -e bash -lc '… &'` 起的程序會被收掉，要 `setsid -f`；Git Bash 對 `/home/...` 做 MSYS 轉換。
- named pipe 3 條並發會 `ERROR_PIPE_BUSY`；每條 API 連線只服務一個 method；HERDR 沒有全域 agent 狀態訂閱。
- `done` 是「idle 且未被看過」，不是 Completed；**不得對 Windows 端 `herdr server stop`**。
- markdownlint-cli2 在 repo 根跑並核對 `Linting: N files` 不為 0；Codex focus 字串不要放反引號。

## 5. 已定但未執行的決策

| 決策 | 狀態 |
|---|---|
| change 2 的 Codex 補審 | **已決不做**（2026-09-19 使用者接受 opus 替代審查）；之後有需要再針對特定 diff 跑 |
| Claude 在 feature 分支 commit、收尾 squash 併回 main | **已授權**（逐 task 一個 commit；change 1a／1b／2 都這樣做） |
| Direction 01 視覺 | **已決另開 change**，不在 change 2 |
| `/ws` 可被任意網站以 WebSocket 讀取投影 | **已知不修**；建議之後把 `source_check` 也套到 `/ws` |
| 斷線期間畫面沒有「取消改綁」按鈕（M2） | **留待視覺改版**：spec 字面滿足，可用 `curl -X DELETE` |
| 每次推送整頁重畫使鍵盤焦點消失（M3） | **留待視覺改版**：spec 未要求保留焦點 |
| `[state] path` 父目錄不存在→每次寫入 500 | **只寫 README**，不自動建目錄 |
| 大寫 `LOCALHOST:<port>` 的 Host 被 403 | **不修**（fail-closed；瀏覽器與 curl 都送小寫） |
| `with_write_hook`（`doc(hidden)` 測試鉤子）注入會 panic 的 callback | **不修**；之後可改 feature 限定 |
| 同 tick 就緒時 Drift 被當新觸發（多拿一次 snapshot） | **不修**（語意正確，成本一次 snapshot） |
| 真實時間測試（reopen 1.67 s、loop_integration 2.5 s、重疊 350 ms） | **不改 paused time**（第 4 節：tokio 組合互斥、真實 transport） |
| tokio-tungstenite 兩版並存、`Path::exists()` 權限情境 | **不處理**（依賴邊獨立；生產碼無 `exists()` 呼叫） |
| per-pane S 訂閱重開是否也重播歷史 | **未驗**：7.2 觀察沒有倒退，但沒用 `nc` 直接擷取確認 |
| Windows 0.9.0 是否會重播歷史 | **唯讀擷取未見**（5 秒、長時間使用中的 server）；升版時重跑 `capture_events` 確認 |
| WSL 端每秒多次 `pane.read` | **未決**：超過每秒一次要改 ADR-0002 方案 B（change 3 要面對） |
| Tauri 桌面殼 | MVP 完成後再評估（ADR-0005） |

## 6. 之後的路

change 3 `live-output`（pane 輸出以 `revision` 輪詢推送）→ Direction 01 視覺改版 → MVP 完成後評估 Tauri
桌面殼（ADR-0005）。北極星與整體範圍見 `docs/cockpit-spec.md`；每個 change 的歷史決策見
`openspec/changes/archive/*/`（含各自的 `sdd-ledger.md`，裡面有所有 Ruling 與 parked findings）。

## 版本紀錄

| 版本 | 日期 | 變更 |
|---|---|---|
| 1–10 | 2026-09-13～15 | change 1a／1b：brainstorming、設計文件、`herdr-client`、三個 crate、真機 Scenario A／B／F、兩次併回 main 與 archive（詳見 git log 與 archive 目錄） |
| 11 | 2026-09-15 | change 2 propose 完成；三項使用者決定與兩條假設翻案 |
| 12 | 2026-09-17 | change 2 apply 25／27；發現並修 HERDR 訂閱重播 bug（D12、task 6.6）；Codex 額度用盡改替代審查 |
| 13 | 2026-09-19 | 7.3 使用者手動驗收通過、27 task 全勾 |
| 14 | 2026-09-19 | change 2 squash 併回 `main`、8 份 delta spec 同步、archive 完成、ledger 保存；下一段改為 change 3 `live-output` |
