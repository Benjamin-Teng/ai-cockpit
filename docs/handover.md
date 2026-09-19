# 交接：下一段任務

> **建立日期**：2026-09-19　|　**上一段做完的事**：change 2 `pipeline-projection` 以 SDD 完成全部 27 個 task
> （domain 純函數、pipeline 設定、狀態檔與寫入服務、HTTP 寫入端點與來源檢查、Factory Floor 畫面與互動、1b deferred
> minor、真機 WSL 驗收、全 workspace gate、使用者手動驗收）；apply 期間發現並修好 HERDR 0.8.2 訂閱重播 bug（新增 task 6.6）。
> **性質**：接手用文件，會過期，每段重寫。
> 為什麼做看 `docs/cockpit-spec.md` 與設計文件 `docs/superpowers/specs/2026-09-13-cockpit-mvp-design.md`、
> 怎麼做看 `~/.claude/CLAUDE.md`（本 repo 精簡版在 `AGENTS.md`）、完整待辦看
> `openspec/changes/pipeline-projection/tasks.md`，進度現場跑 `openspec status --change pipeline-projection`。

## 0. 三十秒版本

1. **Codex 額度要到 2026-09-20 09:58 才重置**。本 change 最後幾輪審查（4.2 fix round 1、4.3、6.5、6.6 與全分支
   最終審查）是 fresh opus 替代審查，**Codex 正式審查尚未補跑**。錯過的代價：依 CLAUDE.md，未經 Codex 審過不能宣稱
   可合併——除非使用者明示接受替代審查。9/20 之後照第 2 節指令補跑。
2. **27 個 task 全勾**（7.3 使用者手動驗收已於 2026-09-19 通過，結果在驗收文件 task 7.3 節）。剩下的只有
   Codex 補審（第 2.2 節），通過後 squash 併回 `main` 並 archive（第 3 節）。
3. 分支 `feat/pipeline-projection`，本檔 commit 後工作樹乾淨；SDD ledger 在本機 `.superpowers/sdd/tasks/progress.md`（**不進版控**，
   archive 時要去識別化複製成 change 目錄的 `sdd-ledger.md`，否則全部 Ruling 會遺失）。
4. 環境：WSL 測試 server **已停**；7.3 驗收用的 cockpit 已停（跑之前先 `netstat -ano | grep 7770` 確認沒有殘留）。
   本機 `cockpit.toml`（gitignored）末尾留有 7.3 的驗收 project 區段，不需要時整段刪除即可。

## 1. 現在的狀態

- 已產出（分支上，尚未併回 main；1b 產出見 `openspec/specs/` 主規格與各 crate README）：
  - `cockpit-core/src/domain/`：`ids.rs`（ProjectId／WorkstreamId／TaskId）、`config.rs`（ProjectDef／WorkstreamDef／
    BindingSpec／TaskDef）、`progress.rs`（Mark、TaskProgress、ProgressOp、`apply_op`）、`binding.rs`（Override、
    BindingResolution 五種、`resolve_binding`→`(結果, 覆蓋失效)`、`validate_override`）、`status.rs`（StageStatus 六值、
    `derive_status`）、`state.rs`（DomainState）、`rejection.rs`。
  - `cockpit-core/src/projection.rs`＋`handle.rs`：投影加 `projects[]`；`StoreHandle` 同鎖持 runtime＋domain
    （`new_with_domain`／`set_domain`／`with_domain`），`spawn_projector_with_stale_sink` 把失效覆蓋去重後送
    `UnboundedSender`。
  - `cockpit-core/src/driver.rs`：Drift 追加重拿（上限 2）、固定重試間隔下限 1 s、**連線後沉降重拿**（進入 Connected
    後事件流靜默 1 s 再取 snapshot，最晚第 5 s，每條連線一次）。
  - `cockpit/src/`：`config.rs`（`[[project]]`／`[state] path`，錯誤路徑 `project.<pid>.…`）、`progress.rs`（狀態檔載入
    與容錯）、`progress_service.rs`（寫入交易在服務自己 spawn 的 task 內：計算→寫 `.tmp`→rename→set_domain）、
    `http.rs`（寫入路由）、`source_check.rs`（Host／Origin 檢查）、`app.rs`（組裝、shutdown 共用期限）。
  - `cockpit/assets/app/`：`render.js`（`renderState(state, ui)`，Factory Floor）、`actions.js`（pointerdown 委派、
    改綁模式、錯誤序號）、`channel.js`（退避在收到第一則訊息時歸零、壞 JSON 略過）。
  - `cockpit-herdr`：`Shutdown.aborts` 修剪、F4 session guard 測試。
  - 驗收：`docs/research/2026-09-16/`（`pipeline-projection-acceptance.md`、`channel-backoff-check.js`、
    `factory-floor-check.js`、`actions-check.js`、`pipeline-check.py`、截圖）。
  - 文件：設計文件 §2.3（重播事實）、§8.1–8.3、§10.2；`cockpit/README.md`（pipeline 設定、寫入 API、單實例、回滾、
    listen／狀態檔目錄限制）；`CONTEXT.md` Domain 詞彙；`.gitignore` 加 `cockpit.state.json`。
- 可用指令（repo 根）：
  - 全 gate：`cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test --workspace && markdownlint-cli2 "**/*.md" && openspec validate --all`
  - 啟動：`cargo run -p cockpit -- --config cockpit.toml`；畫面預覽（不需 HERDR）：`cargo run -p cockpit --example ui_preview`
  - 寫入 API（要帶合法 Host）：`curl -X POST -H "Host: 127.0.0.1:7770" http://127.0.0.1:7770/api/projects/<pid>/tasks/<tid>/advance`
    （op＝advance／complete／fail／clear；覆蓋 `PUT`／`DELETE /api/projects/<pid>/workstreams/<wid>/override`，PUT 本體
    `{"runtime": "...", "pane_id": "..."}`；成功 204）
  - 前端驗收腳本（先 `cargo build -p cockpit --example ui_preview`）：`node docs/research/2026-09-16/actions-check.js`、
    `factory-floor-check.js`、`channel-backoff-check.js`
  - 真機 WSL 驗收（先重啟 WSL 測試 server、`cargo build --release -p cockpit`）：
    `HERDR_CLIENT_TEST_ALLOW_WSL_WRITES=1 PYTHONUTF8=1 uv run --no-project python docs/research/2026-09-16/pipeline-check.py`
  - WSL 測試 server：啟動 `wsl.exe -d Ubuntu-24.04 -e bash -lc "setsid -f ~/.local/bin/herdr server >/tmp/herdr-server.log 2>&1 </dev/null"`；
    停 `wsl.exe -d Ubuntu-24.04 -e bash -lc "~/.local/bin/herdr server stop"`（**只能停 WSL 端**）
  - Windows 端唯讀事件擷取：`cargo run -p herdr-client --example capture_events -- --seconds 5`
  - Codex review：`node ~/.claude/plugins/cache/openai-codex/codex/1.0.6/scripts/codex-companion.mjs adversarial-review --wait --base main "<focus>"`
- 測試與 gate：以當場輸出為準。2026-09-17 於 `14959a3`：fmt 0、clippy 0、`cargo test --workspace` 494 passed／0 failed／
  10 ignored、markdownlint 58 files 0 issues、`openspec validate --all` 12 passed／0 failed（輸出在驗收文件 task 7.1 節）；
  之後只改了 README 兩句與本檔。
- 版控：`main` 在 `536204e`；分支自 `181ee8c`（artifacts）起約 60 個 commit；沒有 remote。

## 2. 立刻要做

### 2.1 使用者手動驗收（task 7.3）—— 已於 2026-09-19 完成

結果見 `docs/research/2026-09-16/pipeline-projection-acceptance.md` task 7.3 節：網格、推進、改綁由使用者目視確認；
重啟保留與清除標記由控制端代跑（覆蓋與標記重啟後逐欄相同）；覆蓋失效沿用 7.2 的 WSL 自動驗收。下列步驟保留供再次驗收照抄。

1. Windows 端 `cockpit.toml` 加一個 `[[project]]`（照 `cockpit.example.toml` 示範），一條 workstream 的 binding 寫
   Windows runtime id、workspace 標籤，再加 `pane_label` 或 `cwd` 對到真實 agent pane。
2. `cargo run -p cockpit -- --config cockpit.toml`，Chrome 開 `http://127.0.0.1:7770/`。
3. 看 Factory Floor：該 workstream 綁定摘要應為已綁定（顯示 pane）；若顯示未綁定／歧義，補特徵或用「改綁」。
4. 對 agent 下一句指令：該 task 應在一秒內變 `running`（動態強調）。超過 1 秒 → `RUST_LOG=debug` 看 S 訂閱。
5. 依序按「推進」「Completed」「清除標記」「改綁」→ 點某 pane 列的「綁定到這裡」→「取消改綁」：每次畫面在下一次推送後
   反映；停掉再啟動 cockpit，進度與覆蓋仍在（狀態檔 `cockpit.state.json` 在設定檔旁）。
6. 結果寫進 `docs/research/2026-09-16/pipeline-projection-acceptance.md` 新增 task 7.3 小節，tasks.md 勾 7.3。

### 2.2 Codex 補審（2026-09-20 09:58 之後）

- focus 已存在本機 `.superpowers/sdd/tasks/final-focus.txt`；指令：
  `node ~/.claude/plugins/cache/openai-codex/codex/1.0.6/scripts/codex-companion.mjs adversarial-review --wait --base main "$(cat .superpowers/sdd/tasks/final-focus.txt)"`
- 判讀：輸出開頭的 verdict 不可信——**先 grep log 有沒有 `usage limit` 或 `Turn failed`**（2026-09-17 那次在 10 個指令後
  撞額度，仍印出 `approve / No material findings`）。findings 一律實測重現後才採信（superpowers:receiving-code-review）。
- 審完且修正後進入第 3 節（tasks.md 已 27/27 全勾）。

## 3. 接著要做：分支收尾與 archive

照 1b 的做法（`openspec/changes/archive/2026-09-15-attach-herdr-runtimes/` 為範例）：

1. 取得使用者授權後 `git switch main && git merge --squash feat/pipeline-projection && git commit`；`git diff main feat/pipeline-projection --stat` 為空才 `git branch -D`。
2. ledger 去識別化複製成 `openspec/changes/pipeline-projection/sdd-ledger.md`（grep 使用者名稱、email、家目錄）。
3. delta spec 同步：`cockpit-config`（純 MODIFIED）、`runtime-driver`、`state-projection`、`cockpit-dashboard`（MODIFIED＋ADDED）
   要逐條合併進 `openspec/specs/<cap>/spec.md`（不能只改標題）；`pipeline-config`、`pipeline-domain`、`pipeline-progress`、
   `runtime-binding` 為純 ADDED，改標題後放入。可用 `/opsx:sync` 或 `/opsx:archive`。
4. 刪本機 `.superpowers/sdd/tasks/`，重寫本檔。

## 4. 這一段踩過的坑（不要再推導一次）

（本段新增；**不會報錯的錯誤**加粗）

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
| 重播 bug 在本 change 內修 | **已決並完成**（使用者 2026-09-17）：spec `runtime-driver` 加「連線後沉降重拿」、design D12、task 6.6 |
| 沉降參數 | **已決**：靜默 1 s、上限 5 s（使用者同意）；7.2 門檻依此改 6 s |
| Direction 01 視覺設計 | **已決不進 change 2**（使用者 2026-09-16）；另開 change，功能區塊以 `docs/cockpit-dashboard-direction-01-concept.png` 為準 |
| Codex 審查 | **待補**：9/20 09:58 後補跑全分支＋替代審查過的 commit；gate 已開 |
| 合併與 archive | **待使用者授權**（7.3 與 Codex 補審之後） |
| `/ws` 可被任意網站讀取投影 | **已知不修**（1b 既有）；建議之後把 `source_check` 套到 `/ws` |
| 斷線期間畫面無「取消改綁」按鈕（M2） | **留待之後**：spec 字面滿足，可用 curl DELETE |
| 每次推送整頁重畫使鍵盤焦點消失（M3） | **留待之後**：spec 未要求；改版畫面時處理 |
| `[state] path` 父目錄不存在每次寫入 500（M5） | **只寫 README**，不自動建目錄 |
| `LOCALHOST:<port>` 大寫 Host 被 403 | **不修**（fail-closed，瀏覽器送小寫） |
| `with_write_hook`（doc(hidden) 測試鉤子）panic 情境 | **不修**；之後可改 feature 限定 |
| 同 tick 就緒時 Drift 被當新觸發（多拿一次 snapshot） | **不修**（語意正確） |
| 真實時間測試（reopen 1.67 s、loop_integration 2.5 s、重疊 350 ms） | **不改 paused time**（第 4 節坑） |
| tokio-tungstenite 兩版並存、`Path::exists()` 權限情境 | **不處理**（依賴邊獨立；生產碼無 `exists()`） |
| per-pane S 訂閱重開是否重播 | **7.2 觀察未見倒退**；未做 nc 直接擷取 |
| Tauri | MVP 後再評估 |

## 6. 之後的路

change 2 收尾（7.3、Codex 補審、squash、archive）→ change 3 `live-output`（`pane.read` 以 `revision` 輪詢推送，
change 1a 已建 `PaneReadParams`／`PaneReadResult`；WSL 端讀取頻率超過每秒一次時改 ADR-0002 方案 B；注意重播 bug 對
新訂閱的影響）→ 視覺 change（Direction 01）→ MVP 完成後評估 Tauri 桌面殼（ADR-0005）。北極星與整體範圍見
`docs/cockpit-spec.md`。

## 版本紀錄

| 版本 | 日期 | 變更 |
|---|---|---|
| 1 | 2026-09-13 | 初版：brainstorming 完成、設計文件待審 |
| 2 | 2026-09-13 | 找碴審閱後：每 pane 狀態訂閱、change 1 拆 1a／1b、研究證據進 repo |
| 3 | 2026-09-14 | change 1a 18 task 完成 17；SDD＋Codex 逐 task review；spike 1–5 真機結論 |
| 4 | 2026-09-14 | change 1a 全部完成：最終 review 修正波、Codex 額度用盡改替代審查 |
| 5 | 2026-09-14 | change 1a 併回 main 並 archive；下一步 1b propose |
| 6 | 2026-09-14 | change 1b propose 完成、分支建立；下一步 1b apply |
| 7 | 2026-09-15 | change 1b apply：30 task 完成 27，真機 Scenario A／B／F，最終 review 兩條 Important 已修 |
| 8 | 2026-09-15 | 1b squash 併回 main、archive、規格同步；下一步 change 2 範圍討論 |
| 9 | 2026-09-15 | 4.4 定案 Windows 0.9.0 不補推 |
| 10 | 2026-09-15 | 1b 手動驗收通過，完全結案 |
| 11 | 2026-09-15 | change 2 propose 完成；三項使用者決定與兩條假設翻案；下一步 apply |
| 13 | 2026-09-19 | 7.3 使用者手動驗收通過（含控制端代跑的重啟保留比對）、7.4 勾選，27 task 全勾；剩 Codex 補審與收尾 |
| 12 | 2026-09-17 | change 2 apply：25／27 task 完成；發現並修 HERDR 0.8.2 訂閱重播 bug（D12、6.6）；Codex 額度用盡，後段替代審查，9/20 補審；剩 7.3 使用者驗收與收尾 |
