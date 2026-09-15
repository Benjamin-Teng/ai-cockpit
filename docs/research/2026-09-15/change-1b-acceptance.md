# change 1b `attach-herdr-runtimes` 驗收紀錄

執行日期 2026-09-15，分支 `feat/attach-herdr-runtimes`。本檔依 task 進度逐節追加：畫面（task 3.6）、
Scenario A／B／F 與 L 訂閱查證（task 4.1–4.4）。所有路徑與使用者名稱皆去識別化。

## 畫面（task 3.6，spec `cockpit-dashboard`「畫面整頁重畫」）

環境：Chrome 擴充功能未連線，改用本機 Chrome headless 模式（`--headless=new`）對
`cargo run -p cockpit --example ui_preview`（fixture `cockpit/tests/fixtures/projected-state.json`）
做截圖、DOM dump 與 DevTools protocol 觀察；程式碼版本 commit `b5315ad`。

### Scenario「兩個 runtime 的畫面」

截圖：`ui-preview.png`（`--window-size=1280,1400 --virtual-time-budget=4000 --screenshot`）。

`--dump-dom` 檢查結果：

| 檢查項 | 結果 |
|---|---|
| `.runtime-card` 數 | 2（`win` 為 `conn-connected`、`wsl` 為 `conn-disconnected`） |
| `wsl` 卡顯示原因與重試 | 「reason: WSL 發行版 Ubuntu-24.04 未啟動」「retry in 60s」 |
| pane 列數 | 3（`wJ:p1` working、`wJ:p2` done 且 `exited` 刪除線、`wJ:p3` unknown） |
| 狀態色塊 class | `status-working`、`status-done`、`status-unknown`（workspace／tab 彙總另各一個 `status-working`） |
| 頂列 | 產品名稱、`#channel-status` 為 `connected`、`#version` 為 `v46`（headless 下 WebSocket 亦連上） |
| 最近事件 | 3 筆（`pane.agent_status_changed`、`pane.exited`、`drift`） |
| 「完成」字樣 | 無 |
| console | `--enable-logging=stderr` 無任何 `CONSOLE` 訊息（無錯誤） |

### Scenario「未知狀態不破壞畫面」

後端型別 `cockpit_core::AgentStatus` 是封閉五值，`whatever` 無法經 `/api/state` 送出（fixture 只能用
`unknown`）；改以 harness 頁面驗 `render.js` 的容錯。可重跑的腳本 `whatever-check.js`（Node 22 內建
API，不引 npm 套件）：自己啟動 `ui_preview`、取 `/api/state`、把 `panes[0].agent_status` 改成
`"whatever"`、產生載入同一份 `/app/style.css` 與 `/app/render.js` 的 harness 頁並呼叫
`window.onState(state)`、headless Chrome `--dump-dom` 後斷言；整個生命週期在同一個 try/finally 內，
只依 PID 清理自己啟動的程序。執行結果（腳本版本 commit `0320241`；受測產品程式碼同 `0fc9714`）：

```text
node docs/research/2026-09-15/whatever-check.js
ok   headless Chrome dump-dom 成功（5208 bytes）
ok   pane 列數為 3（3）
rows: [{"cls":"unknown","text":"whatever","title":true},{"cls":"done","text":"done","title":false},{"cls":"unknown","text":"unknown","title":false}]
ok   whatever 列的色塊 class 為 status-unknown（unknown）
ok   whatever 列的色塊文字保留原字串（whatever）
ok   whatever 列的色塊 title 屬性為原字串
ok   沒有出現未經白名單的 status-whatever class
ok   其他列的色塊 class 仍在五值白名單內
ok   taskkill PID 80784 成功
ok   preview 已停止（API 不再回應）
RESULT: PASS
```

暗灰色由 `style.css` 的 `.status-unknown` 規則決定（同 fixture 內 `unknown` 那列）。

### Scenario「通道重連」

`reconnect-check.js`（Node 22，headless Chrome `--remote-debugging-port` ＋ `Runtime.evaluate`，
不引 npm 套件；整個生命週期在同一個 try/finally 內，只終止自己 spawn 的程序、依 PID）逐條斷言，
任一不成立就非零結束。執行結果（腳本版本 commit `0320241`；受測產品程式碼同 `0fc9714`）：

```text
node docs/research/2026-09-15/reconnect-check.js
T0: status=connected version=v7 cards=2
ok   T0 通道狀態為 connected
ok   T0 version 合法（v7）
ok   T0 兩張 runtime 卡（2）
ok   taskkill PID 108348 成功
ok   preview 已停止（API 不再回應）
after stop: status=disconnected version=v8
ok   停掉 preview 後通道狀態為 disconnected
after restart: status=connected version=v7 (before=v8) cards=2 pane0=working
ok   重啟後通道狀態回到 connected
ok   重啟後收到新的整份投影（version v8 → v7）
ok   重啟後仍有兩張 runtime 卡（2）
+2.5s: version=v8 pane0=idle
ok   重連後持續收到推送（version v7 → v8）
ok   pane 狀態色塊隨推送重畫（working → idle）
ok   taskkill PID 81708 成功
ok   taskkill PID 83040 成功
ok   preview 已停止（API 不再回應）
RESULT: PASS
```

停掉 `ui_preview` 後 1.5 秒內頂列變 `disconnected`；重啟後不到 1 秒（`channel.js` 退避第一段 1 s）
回到 `connected` 並收到新程序的整份投影（新程序從 fixture 的 v7 起算）；之後 2.5 秒內 version 再變、
pane 色塊由 `working` 輪替為 `idle`，證明是整份重畫而非只換連線狀態。

### Gate 證據（程式碼 commit `3d1993b`，含 task 3.7 的 assets）

```text
cargo fmt --check                                            → exit 0
cargo clippy -p cockpit --all-targets -- -D warnings         → Finished, 0 warning
cargo test -p cockpit                                        → config 14、fixture 1、http 5、pwa 3、runtimes 4、ws 4，共 31 passed / 0 failed
markdownlint-cli2 "**/*.md"                                  → Linting: 36 files, Summary: 0 issues
```

### 結論

三個 Scenario 皆通過，未發現需要回頭修改 task 3.5 檔案的問題。限制：Chrome 擴充功能未連線，所以
沒有人眼目視；headless 截圖即為人眼版面的依據，Chrome 選單安裝 PWA 的手動驗收留到 task 4.1。

## Scenario A Attach（task 4.1，設計文件 §10.2）

環境：Windows 端 HERDR `0.9.0-preview.2026-09-08-62431dbd033b`（protocol 22）日常在跑；WSL 端
`Ubuntu-24.04` 的 headless 測試 server `0.8.2`（protocol 20）以 handover §1 的 `setsid -f` 指令啟動；
本機 `cockpit.toml`（gitignored）兩筆 runtime：`win` 省略端點（自動找預設 named pipe）、`wsl`
指向該發行版的 unix socket。受測程式為 `cargo build --release -p cockpit` 的 `cockpit.exe`（commit
`5b0d163`）。全程對 HERDR 唯讀。

### 自動化部分：`attach-check.py`

三個腳本共用 `acceptance_common.py`（stdlib）：自己啟動 `cockpit.exe`、輪詢 `/api/state`、取兩側
`herdr api snapshot`、對 WSL 端測試 server 送 JSON-RPC、結束時只依 PID 收掉自己起的程序，以及
`compare_projection`——照 `cockpit-herdr/src/translate.rs` 與 `cockpit-core/src/projection.rs` 的規則把
投影對 snapshot **逐欄位**比對：workspace（id 集合、label、number、agent_status、focused）、每個
workspace 底下的 tab（id 集合、number、agent_status、focused）、每個 tab 底下的 pane（id 集合、agent、
agent_status、title、cwd、label、focused）、`focused.{workspace_id,tab_id,pane_id}`；不符時只印 id 與
欄位名，路徑類欄位不印值。腳本版本：commit `3088707`（第一版只比 id 集合與 agent 名稱，Codex review
指出不足以支持「內容與 snapshot 一致」，改為逐欄位）；round 2 commit `2ac0e90` 再補 pane 的 `exited`
必須為 `false`（snapshot 沒有這欄、翻譯固定 false；突變證據：把投影的 `exited` 改成 `true` 會得到
`pane w1:p1 exited：投影 True vs snapshot False`）、live-state 清理的 snapshot 重試 3 次與已知 tab id 先
直接關、reconnect 的巢狀 `finally`。下面三節的輸出是 `3088707` 版本的執行；`2ac0e90` 版本重跑三者皆
`RESULT: PASS`（4.2 收斂 3.0 s、4.3 回到 connected 7.4 s，其餘數字同量級）。

受測程式的最終版本：最終全分支 review 後的修正波（commit `069ac89` 初始 S handle 不覆蓋較新一代、
`198d397`＋`d58ea18` `run_with_shutdown` 等驅動器與投影任務收乾淨，逾時 abort 後仍 await）之後，分別以
`198d397` 與 `d58ea18` 重建 release 並重跑三個腳本，皆 `RESULT: PASS`（4.1 兩側逐欄位一致；4.2 狀態延遲
0.10–0.15 s、關 tab 後 3.0 s 收斂、無殘留 tab；4.3 退避 1,2,4,8、7.1–7.3 s 回到 connected、逐欄位一致）。

`uv run --no-project python docs/research/2026-09-15/attach-check.py`：

```text
ok   /api/state 有回應
version 5 runtimes [('win', 'connected'), ('wsl', 'connected')] (0.5s)
ok   runtime win 為 connected
  win: server 0.9.0-preview.2026-09-08-62431dbd033b protocol 22 warning None
ok   runtime wsl 為 connected
  wsl: server 0.8.2 protocol 20 warning None
ok   runtime 順序與設定一致（win）
  win: projection workspaces=2 tabs=2 panes=4 agents=['claude', 'claude']
  win: herdr      workspaces=2 tabs=2 panes=4
ok   win 投影與 herdr api snapshot 逐欄位一致（workspace／tab／pane／focused）
ok   runtime 順序與設定一致（wsl）
  wsl: projection workspaces=1 tabs=1 panes=4 agents=[]
  wsl: herdr      workspaces=1 tabs=1 panes=4
ok   wsl 投影與 herdr api snapshot 逐欄位一致（workspace／tab／pane／focused）
ok   cockpit 已停止（API 不再回應）
RESULT: PASS
```

從啟動到兩筆都 `connected` 0.5 秒（投影 version 5）。兩側 `protocol_warning` 皆 `None`（20 與 22
都在已測範圍）。

`cargo test -p cockpit --test real_attach -- --ignored`（零設定、只連 Windows 端）：

```text
test real_zero_config_connects_and_pane_count_matches_herdr_snapshot ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.22s
```

### 待使用者親自做的部分

- 在 HERDR pane 內執行 `cargo run -p cockpit`（或 release 的 `cockpit.exe`），目視確認沒有跳出新視窗
  （spike 4 目視複驗；自動化的 `Get-Process | Where MainWindowHandle -ne 0` 前後比對抓不到瞬間視窗）。
- 以 Chrome 開 `http://127.0.0.1:7770/`，從選單「安裝」成獨立視窗的應用程式（PWA 手動驗收）。

真機畫面的截圖含本機 cwd 路徑與使用者名稱，不進 repo；版面依據以 task 3.6 的 `ui-preview.png` 為準，
真機差異只在資料內容。

## Scenario B Live state（task 4.2）

### WSL 端自動化部分：`live-state-check.py`（需 `HERDR_CLIENT_TEST_ALLOW_WSL_WRITES=1`）

對 WSL 端 headless 測試 server 的 workspace `wD` 用 1a 真機鷹架同一套 JSON-RPC（`wsl.exe -e nc -U`）
做 `tab.create` → `pane.report_agent` working／blocked／idle → `pane.clear_agent_authority` →
`tab.close`，每步觀察 cockpit 的 `/api/state`。清理：`finally` 重新取 WSL 端 snapshot，把 `wD` 裡帶本腳本
專用 label、且不在起始 baseline 的 tab 全部 `tab.close`（即使 `tab.create` 的回應遺失也收得掉），殘留即
FAIL。腳本版本 commit `3088707`；下面是 WSL 測試 server 重啟後（沒有補推積壓）的執行：

```text
ok   兩筆 runtime 皆 connected
baseline wsl panes: ['wD:p1', 'wD:p26', 'wD:p3', 'wD:p4']
created tab wD:t13 pane wD:p2B
ok   新 pane wD:p2B 在 0.12s 內出現在投影（L 事件 → PaneUpserted）
ok   pane.report_agent(working) 回 ok
ok   report working → 投影顯示 working，延遲 0.10s（<1s；驗 ReopenStatus 讓新 pane 的狀態即時到達）
ok   最近事件含 pane.agent_status_changed（wD:p2B）
ok   report blocked → 投影顯示 blocked，延遲 0.12s
ok   最近事件含 pane.agent_status_changed（wD:p2B）
ok   report idle → 投影顯示 idle，延遲 0.12s
ok   最近事件含 pane.agent_status_changed（wD:p2B）
workspace_updated seen during status changes: False
ok   pane.clear_agent_authority 回 ok
after release: agent_status=unknown (0.11s)
ok   tab.close 回 ok
ok   關 tab 後 pane wD:p2B 在 0.12s 內從投影消失
events after tab.close (newest first): [('layout_updated', None), ('focus_changed', 'wD:p1'), ('pane_removed', 'wD:p26'),
  ('pane_upserted', 'wD:p2D'), ('focus_changed', 'wD:t1'), ('tab_removed', 'wD:t13'), ('focus_changed', 'wD')]
ok   pane 已不在投影（連鎖刪除）
  t+0.1s version=17 diffs=4
    diff: pane wD:p1 focused：投影 True vs snapshot False
    diff: pane wD:p2D label：投影與 snapshot 不同（值不印）
    diff: pane wD:p2D focused：投影 False vs snapshot True
    diff: focused pane_id：投影 'wD:p1' vs snapshot 'wD:p2D'
  t+0.7s version=19 diffs=4
    diff: pane wD:p1 focused：投影 False vs snapshot True
    diff: pane wD:p2D label：投影與 snapshot 不同（值不印）
    diff: pane wD:p2D focused：投影 True vs snapshot False
    diff: focused pane_id：投影 'wD:p2D' vs snapshot 'wD:p1'
  t+1.9s version=27 diffs=0
ok   關 tab 後投影在 1.86s 內與 WSL 端重新取得的 snapshot 逐欄位一致（workspace／tab／pane／focused）
pane set delta vs baseline: removed=['wD:p26'] added=['wD:p2D']
wsl drift events in recent_events (at, reason): [('...T06:15:38Z', 'tab wD:t13 不存在'), ('...', 'pane wD:p2B 不存在'),
  ('...', 'pane wD:p2B 不存在'), ('...', 'tab wD:t13 不存在'), ('...', 'tab wD:t13 不存在')]
ok   Windows 端全程 connected
ok   cleanup：WSL 端沒有殘留的測試 tab（殘留：[]）
ok   cockpit 已停止（API 不再回應）
RESULT: PASS
```

關 tab 之後的觀察（第一版腳本斷言「pane 集合回到 baseline」在這裡會失敗，是 HERDR 行為不是投影錯）：

- **HERDR 0.8.2 關 tab 後會把 `wD` 的 Sidebar pane 換一個 id**（`wD:p26` → `wD:p2D`，`herdr api snapshot`
  證實），並在 0.1–0.7 秒內把焦點在 `p1` 與新 Sidebar 之間來回移動；cockpit 收到 `pane_removed` ＋
  `pane_upserted` 跟上。
- **tab 關閉後 HERDR 仍會送幾筆指向已關 tab／pane 的事件**（`tab wD:t13 不存在`、`pane wD:p2B 不存在`），
  cockpit 依設計記 Drift 並立即重拿 snapshot；1.9 秒後投影與 snapshot 逐欄位一致（含新 Sidebar 的 label
  與焦點）。中間 0.1 s／0.7 s 兩個時點的差異就是「重拿的 snapshot 比同時到達的事件舊」的短暫視窗，由
  後續事件與再一次 Drift 重拿收斂；定期重拿（30 s）是最後保險，這次沒有等到它。
- **同一個 0.8.2 server 連續做多輪 tab 開關後會累積補推**：重啟前的一次執行，關 tab 時收到上一輪 tab
  的 `tab_upserted`＋`tab_removed`（建立 tab N 時補推 tab N-1 的生命週期，與 handover §3 的「一代延遲」
  一致）以及更早幾輪的 `tab wD:t0 不存在`／`pane wD:p24 不存在`，連續多筆 Drift 各觸發一次重拿（WSL 端
  走 `wsl.exe`＋`nc`，每次數百 ms），那一輪新 pane 出現花 1.31 s、`working` 0.87 s、`idle` 1.21 s，且
  `working` 的 S 事件沒趕上（狀態經重拿到達）。Windows 0.9.0 沒有觀察到補推（見 4.4），日常環境不受
  影響；WSL 端的改善方向（Drift 重拿加短合併窗）記在 handover 待辦，不在 1b 範圍。

順手記錄（回應 design 的三個問題）：

- **D5**：關 tab 對該 tab 底下的 pane（`wD:p2B`）只收到 `tab_removed`（來自 `tab_closed`）與 focus 變化，
  **沒有** `pane_closed`——WSL 0.8.2 與 spike 3 的 fixture 一致，狀態庫的連鎖刪除是必要的（上面的
  `pane_removed wD:p26` 是 Sidebar 換 id，不是被關 tab 的 pane）。
- **D6**：三次狀態變化期間最近事件裡沒有 `workspace_updated`，tab／workspace 的彙總狀態確實要等下一次
  snapshot（≤ `resnapshot_secs`）才追上。
- **idle／Done**：這次 `report_agent state=idle` 呈現 `idle`（1a 曾觀察到跳成 `done`，兩者都可能）。
- **ReopenStatus 的 200 ms 去抖動窗**：第一次執行時在 pane 建立後不到 200 ms 就送 report，狀態經 L 的
  `pane_updated` 到達投影（0.2 s 內），但 S 訂閱尚未重開完成所以最近事件沒有 `pane.agent_status_changed`；
  等 0.5 s 再報告就正常。真實 agent 不會在 pane 建立後 200 ms 內變狀態，不視為問題。

### Windows 端：待使用者親自做

在 Windows 端某個 agent pane 對 agent 下一句指令，觀察該 pane 一秒內變 `working`、頁尾最近事件出現
`pane.agent_status_changed`；全程 Cockpit 不會送任何 prompt 給 agent（只有 `session.snapshot` 與
`events.subscribe`）。這需要真人在 HERDR 的 pane 裡操作，無法由腳本代替。

## Scenario F Reconnect（task 4.3，只在 WSL 端）

`reconnect-real-check.py`：對 **WSL 端 headless 測試 server** 下 `herdr server stop`（設計文件 §10.1 的
禁令只針對 Windows 端），觀察 8 秒後以 `setsid -f` 重啟。停 server 是破壞性操作，腳本有三層 guard，缺一
就不會動任何 server：`HERDR_CLIENT_TEST_ALLOW_WSL_WRITES=1`（與 1a 真機測試相同的 WSL 端寫入 opt-in；
未設就印說明、exit 0）、`COCKPIT_ACCEPT_STOP_WSL_DISTRO` 必須等於 `cockpit.toml` 裡 wsl runtime 的 distro
（操作者明確認可要停哪個 distro；不符 exit 2）、停之前 `herdr status server --json` 必須 `running` 且回報
的 socket 與 `cockpit.toml` 設定相同（確保停的是 cockpit 正在觀察的那個 server；不符 exit 2）。負案例實
測：不設 opt-in → `未設定 HERDR_CLIENT_TEST_ALLOW_WSL_WRITES=1：不停任何 server，直接結束（唯讀）。`
exit 0；`COCKPIT_ACCEPT_STOP_WSL_DISTRO=Debian` → `必須等於 cockpit.toml 的 wsl distro（'Ubuntu-24.04'）
才會停該 server；目前 'Debian'。` exit 2。腳本版本 commit `3088707`：

```text
guard ok: distro=Ubuntu-24.04 server 0.8.2 protocol 20 socket 與設定一致
ok   起始：兩筆 runtime 皆 connected（0.5s）
wsl herdr server stop → [] rc 0
ok   herdr server stop 回 0
ok   停後 herdr status server 不再 running
wsl connection history after stop (t, state, reason, retry_in_secs):
   (0.0,  disconnected, 'snapshot 失敗：remote error server_unavailable: server is shutting down (response id 10)', 1)
   (0.9,  connecting)
   (1.0,  disconnected, 'seed snapshot 失敗：connect error: HERDR server not running: bridge exited (exit code: 1): nc: <socket>: No such file or directory', 2)
   (2.99, connecting)
   (3.11, disconnected, '...HERDR server not running...', 4)
   (7.15, connecting)
   (7.27, disconnected, '...HERDR server not running...', 8)
ok   首次 disconnected 原因反映對端關閉（L／S 結束或 server 關閉中）
ok   首次重試秒數為 1（實際 1）
ok   重連期間原因含 ServerNotRunning 類描述
ok   重試秒數遞增（退避）：[1, 2, 4, 8]
ok   Windows 端全程 connected（不受 WSL 端斷線影響）
ok   setsid -f 重啟後 herdr status server 為 running
ok   重啟後 wsl 卡片在 8.0s 內回到 connected
projection workspaces=1 tabs=1 panes=4 agents=[]; herdr workspaces=1 tabs=1 panes=4
ok   重連後投影與重新取得的 snapshot 逐欄位一致（workspace／tab／pane／focused）
ok   Windows 端仍 connected
ok   cleanup：WSL 測試 server 在跑（必要時已用 setsid -f 重啟）
ok   cockpit 已停止（API 不再回應）
RESULT: PASS
```

（原因字串裡的 socket 路徑以 `<socket>` 代替。）第一版腳本的同一流程回到 `connected` 花 5.3 s，這次 8.0 s
——差別是這次腳本先等 2 秒確認 server 狀態再開始計時，加上退避正好落在 8 秒那一格。

觀察：

- 首次斷線原因不是 tasks.md 預期的「L 或 S 連線結束」，而是 HERDR 優雅關閉時對進行中的 `session.snapshot`
  回的 `server_unavailable: server is shutting down`——驅動器正好在重拿 snapshot（HERDR 關閉前會發一批
  事件，其中有觸發 Drift 的），這個原因更精確，spec「原因為錯誤的描述字串」成立。
- 之後每輪重連的原因都是 `herdr-client` 的 `ServerNotRunning` 描述（含 nc 的 `No such file or directory`）。
- HERDR 0.8.2 重啟後 workspace `wD` 的 Sidebar pane 換了 id（第一版執行 `wD:p1X` → `wD:p10`），投影與新
  snapshot 逐欄位一致，證明重連走的是整份替換而不是沿用舊狀態。

## L 訂閱補推舊事件查證（task 4.4，`docs/handover.md` §3 待查證項）

三個觀察點（第 3 點是定案擷取）：

1. `cargo run -p herdr-client --example capture_events -- --seconds 20`（Windows 端，2026-09-15 13:57）：
   L 收到 16 筆，**全部**是既有 pane（`wJ:p2`、`wM:p2`，sidebar 的 token 更新）的 `pane_updated`；S 收到
   0 筆。當下 snapshot 有 2 個 tab、4 個 pane，沒有任何一筆事件指向 snapshot 中不存在的 tab／pane。
   但這 20 秒前後 Windows 端**沒有** tab 建立／關閉活動（spike 3 觀察到的補推是針對剛建立又關閉的 tab），
   所以 Windows 0.9.0 是否會補推舊 tab 事件**無法判定**；要在使用者剛開關過幾個 tab 之後再跑一次
   `capture_events` 才能定案（已於第 3 點定案）。
2. `RUST_LOG=debug cockpit.exe`（同時連兩側）啟動 8 秒的驅動器日誌：

   ```text
   DEBUG cockpit_core::driver: snapshot 前丟棄的事件數 runtime=win discarded=0
   DEBUG cockpit_core::driver: snapshot 前丟棄的事件數 runtime=wsl discarded=2
   ```

   WSL 0.8.2 在 authoritative snapshot 之前推了 2 筆事件（這次執行前 WSL 端剛有 task 4.2 的 tab 建立／
   關閉），與 spike 3 的觀察一致；Windows 0.9.0 為 0 筆。這兩筆依設計 §4.2 被丟棄，畫面沒有幽靈 pane
   （task 4.2 已驗關 tab 後投影在 1.9 秒內與重新取得的 snapshot 逐欄位一致、4.3 驗重連後與 snapshot 逐欄位
   一致）。

3. **定案擷取**（Windows 端，2026-09-15 22:21，同一指令；使用者親自在 Windows 端開兩三個 tab、關掉後
   幾秒內執行，滿足任務要求的「近期 tab 建立／關閉活動後」前提）：L 收到 14 筆，**全部**是既有兩個
   Sidebar pane（`wJ:p2`、`wM:p2`，各在 `wJ:t1`、`wM:t1` 底下）的 `pane_updated`，內容只是 sidebar token
   每 5 秒的例行刷新；S 收到 0 筆（期間沒有 agent 狀態變化，正常）。擷取當下 `herdr api snapshot` 有
   2 個 workspace、2 個 tab、5 個 pane；L 的前幾行與全程 20 秒都沒有任何 tab／pane 的建立或移除事件，
   也沒有任何一筆指向 snapshot 中不存在的 tab／pane。原始檔在 `target/capture/`（gitignored，含本機路徑，
   不進 repo）。Windows 端 HERDR 版本 0.9.0-preview（protocol 22）。

結論：**WSL 0.8.2 有補推、已被丟棄規則吸收；Windows 0.9.0 不補推**（第 3 點在指定前提下擷取，訂閱建立
瞬間與其後 20 秒都沒有指向已關 tab／pane 的事件）。設計文件 §2.3 的加註已由「無法判定」改成這個結論
（本次回寫）；因為沒有補推，不需要再開 cockpit 確認幽靈 pane。丟棄規則（設計 §4.2）對兩側都保留，
不因 Windows 端不補推而簡化。無需 `/opsx:update`。

**task 4.4 已勾選**（2026-09-15）：第 1 點的擷取因缺前提被 Codex review 指為不能定案；第 3 點由使用者
親自補齊前提後定案，結論與證據如上。

Scenario B 的多輪執行補充了 0.8.2 補推的精確模式（見上節）：關閉 tab N 時補推 tab N-1 的
`tab_upserted`＋`tab_removed`，以及更早幾輪已關 tab／pane 的零星事件；這些都指向 snapshot 中不存在的
主體，cockpit 以 Drift → 重拿處理，畫面在 2 秒內回到與 snapshot 一致，沒有幽靈 pane 殘留。
