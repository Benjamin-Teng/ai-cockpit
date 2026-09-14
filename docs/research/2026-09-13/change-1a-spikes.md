# change 1a spike 紀錄

執行日期 2026-09-13，HERDR 版本 `0.9.0-preview.2026-09-08-62431dbd033b`（`private_protocol`
22），分支 `feat/herdr-client`。

## Spike 1：named pipe

### 問題

tokio 內建 `tokio::net::windows::named_pipe::ClientOptions`（不引第三方 crate）能否開啟名稱含
冒號與反斜線的 named pipe（`\\.\pipe\` 接 HERDR socket 路徑全文，例如
`\\.\pipe\C:\Users\<user>\AppData\Roaming\herdr\herdr.sock`），送一行 `session.snapshot`
request 並讀到完整一行回應；並確認 3 條並發連線是否會遇到 `ERROR_PIPE_BUSY`（231）。

### 做法

建立根 `Cargo.toml`（virtual workspace，`members = ["herdr-client"]`，`resolver = "3"`，
`edition = "2024"`）與 `herdr-client` crate 骨架，dev-dependency `tokio 1.53.1`（features
`net`、`io-util`、`rt-multi-thread`、`macros`、`process`、`time`）與 `serde_json 1.0.151`。
在 `herdr-client/tests/real_herdr.rs`（`#![cfg(windows)]`）寫兩個 `#[ignore]` 的
`#[tokio::test]`：

- `spike1_named_pipe_session_snapshot`：組出 pipe 名稱、`ClientOptions::new().open(...)`
  （遇 231 依 tokio 文件建議等 50ms 重試，上限 1 秒），寫入一行 request，`BufReader::read_line`
  讀一行，斷言 `id == "1"`、`result.type == "session_snapshot"`、
  `result.snapshot.protocol` 為整數，並把原始回應整行寫到 `target/spike1-raw-snapshot.json`。
- `spike1_three_concurrent_connections`：同時 spawn 3 個 task 各自開一條連線送
  `session.snapshot`，用 `AtomicUsize` 統計 open 時遇到 231 的總次數，斷言 3 條都拿到
  `session_snapshot`。

執行 `cargo test -p herdr-client --test real_herdr -- --ignored spike1 --nocapture`，完整輸出
存於 `target/spike1-output.txt`。

> **更新（task 5.1）**：`spike1_named_pipe_session_snapshot` 已改寫為
> `tests/real_herdr.rs` 的 `real_win_snapshot`（改用 `Client::request`）；
> `spike1_three_concurrent_connections` 驗的並發重試行為已由
> `tests/transport.rs` 的 `named_pipe_busy_instance_retries_then_succeeds`／
> `named_pipe_busy_timeout_is_io`（對假 HERDR）覆蓋，未沿用。以下內容維持
> spike 當時的原始紀錄，不回溯修改。

### 證據

```text
running 2 tests
pipe name: \\.\pipe\C:\Users\<user>\AppData\Roaming\herdr\herdr.sock
open 遇到 ERROR_PIPE_BUSY(231) 次數: 0
回應行長度: 5753 bytes
version: Some(String("0.9.0-preview.2026-09-08-62431dbd033b"))
protocol: 22
workspaces 陣列長度: 3
agents 陣列長度: 2
test spike1_named_pipe_session_snapshot ... ok
3 條並發連線總計遇到 ERROR_PIPE_BUSY(231) 次數: 2
test spike1_three_concurrent_connections ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s
```

### 結論

- tokio `ClientOptions::open` **接受**含冒號與反斜線的 pipe 名稱：以完整字串
  `\\.\pipe\C:\Users\<user>\AppData\Roaming\herdr\herdr.sock` 呼叫，單條連線 open 一次即成功
  （0 次 231），不需任何跳脫或改名處理。
- 3 條並發連線**會**遇到 `ERROR_PIPE_BUSY`（本次量測 2 次），但 50ms 重試、1 秒上限的邏輯
  足以讓全部 3 條連線最終都拿到 `session_snapshot`，無一失敗。
- 回應**一行即完整**：`read_line` 單次呼叫即讀到合法 JSON（5753 bytes），
  `result.type == "session_snapshot"`、`result.snapshot.protocol == 22`（整數），
  對應原始 fixture `herdr-client/tests/fixtures/snapshot-p22.json`（去識別化後）。

### 對設計的影響

三項全部成立，設計文件 §5.1 不需修改；D9 的重試策略（231 時等 50ms、上限 1 秒）在本次真機
量測下足夠涵蓋 3 條並發連線的忙碌窗口，`Risks / Trade-offs` 一節「tokio `ClientOptions::open`
不接受含冒號的 pipe 名稱」風險不成立，不需改用 `interprocess` crate。

## Spike 2：WSL nc 橋接

### 問題

Windows 端能否以 `wsl.exe -e nc -U <socket>`（`tokio::process::Command`，不引第三方 crate）
橋接 WSL 端 HERDR headless server：`session.snapshot` 一問一答能否用 `nc` 正常收發、server
關閉連線後 `nc` 是否自行結束（`-N` 有無差異）、socket 不存在時多快失敗、以及
`events.subscribe` 訂閱到生命週期事件的延遲是否小於 1 秒。

### 做法

新增 `herdr-client/tests/real_herdr_wsl.rs`（`#![cfg(windows)]`，`#[ignore]`，環境變數
`HERDR_CLIENT_TEST_WSL_DISTRO`／`HERDR_CLIENT_TEST_WSL_SOCKET`，預設
`Ubuntu-24.04`／`/home/<user>/.config/herdr/herdr.sock`），四個 `#[tokio::test]`：

- `spike2_wsl_nc_snapshot`／`spike2_wsl_nc_snapshot_with_dash_n`：spawn
  `wsl.exe -d <distro> -e nc [-N] -U <socket>`（`kill_on_drop(true)`），寫一行
  `session.snapshot`、flush 後 `drop(stdin)`（模擬 smoke test 中 `printf` 管線結束的 EOF），
  `read_line` 讀回應，量 spawn→回應、回應→`child.wait()` 結束兩段耗時，斷言
  `result.type == "session_snapshot"`、`snapshot.protocol == 20`、nc 成功退出。
- `spike2_wsl_nc_missing_socket_exits_immediately`：socket 換成
  `/tmp/does-not-exist.sock`，斷言 2 秒內結束、exit 非 0、stderr 含 `No such file`。
- `spike2_wsl_nc_subscribe_latency`：一條長連線送 `events.subscribe`（`tab.created`／
  `tab.closed`／`pane.created`／`pane.closed`）等到 `subscription_started`；用
  `Command::output()` 呼叫 `herdr tab create --workspace wD --label spike2 --no-focus`，
  以其**回傳時刻**與訂閱串流中比對到自己新 tab id 的 `tab_created` 事件之間的時間差量延遲；
  `herdr tab close <id>` 後同法量 `tab_closed` 延遲；`TabGuard`（`Drop` 內用同步
  `std::process::Command` 補關）確保測試提前結束也會清理自己建立的 tab；結尾
  drop 訂閱子程序、等 500ms、跑 `pgrep -a nc` 斷言無殘留。

執行 `cargo test -p herdr-client --test real_herdr_wsl -- --ignored spike2 --nocapture
--test-threads=1`，完整輸出存 `target/spike2-output.txt`；原始回應存
`target/spike2-raw-snapshot-p20.json`，用新寫的 `docs/research/2026-09-13/deidentify-fixture.py`
去識別化後存 `herdr-client/tests/fixtures/snapshot-p20.json`。

> **更新（task 5.1）**：本節四個測試已改寫、併入 `tests/real_herdr.rs`（改用
> `Client::request`／`Client::subscribe` 搭配 `ChildStdioConnector`）後，
> `real_herdr_wsl.rs` 已刪除：`spike2_wsl_nc_snapshot` → `real_wsl_snapshot_via_child_stdio`；
> `spike2_wsl_nc_subscribe_latency` 的「建 tab 觸發事件」精神保留在
> `real_wsl_lifecycle_stream_via_child_stdio`（拿掉延遲量測，`Client::subscribe`
> 本身不做逾時保證）；`spike2_wsl_nc_snapshot_with_dash_n`／
> `spike2_wsl_nc_missing_socket_exits_immediately` 驗的是 `nc` 命令列本身的行為，
> 已由 `ChildStdioConnector` 的單元測試（`tests/child_bridge.rs`、
> `tests/transport.rs`，對假子程序）覆蓋，未沿用。以下內容維持 spike 當時的原始
> 紀錄，不回溯修改。

### 證據

```text
test spike2_wsl_nc_missing_socket_exits_immediately ... 耗時: 123 ms
exit status: ExitStatus(ExitStatus(1))
stderr: nc: /tmp/does-not-exist.sock: No such file or directory
ok
test spike2_wsl_nc_snapshot ... 回應 bytes: 3057
spawn → 讀到回應: 55 ms
讀到回應 → nc 結束: 61 ms
nc exit status: ExitStatus(ExitStatus(0))
ok
test spike2_wsl_nc_snapshot_with_dash_n ... 回應 bytes: 3057
spawn → 讀到回應: 55 ms
讀到回應 → nc 結束: 64 ms
nc exit status: ExitStatus(ExitStatus(0))
ok
test spike2_wsl_nc_subscribe_latency ... 建立的測試 tab id: wD:t7
tab_created 事件延遲: 377 ms
tab_closed 事件延遲: 74 ms
pgrep -a nc 輸出: ""
ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.75s
```

另外，`--test-threads` 預設值（多執行緒並行）跑同一批測試時，`spike2_wsl_nc_snapshot` 與
`spike2_wsl_nc_snapshot_with_dash_n` 的耗時分佈相近（spawn→回應 45–47 ms、回應→結束
91–93 ms），與 `--test-threads=1` 的結果量級一致，只是單執行緒版本可明確歸屬每個數字給
哪個測試，故取單執行緒版本入紀錄。

MSYS 路徑轉換的實測：`MSYS_NO_PATHCONV=1 bash -c 'printf ... | wsl.exe -d Ubuntu-24.04 -e nc
-U /home/<user>/.config/herdr/herdr.sock'` 能正常連上並解析出合法 JSON；同一指令若不設
`MSYS_NO_PATHCONV=1` 直接跑，`nc` 回報
`nc: C:/Program Files/Git/home/<user>/.config/herdr/herdr.sock: No such file or directory`（本檔
在 Git Bash 手動驗證時遇到，用於確認 Rust `tokio::process::Command` 不受影響）。本次
`real_herdr_wsl.rs` 全程用 `tokio::process::Command` 傳遞 `/home/<user>/...` 路徑，四個測試
全部連線成功、無需設任何 MSYS 相關環境變數，證實 Rust 端不經過 MSYS 路徑轉譯。

### 結論

- **server 關連線後 nc 會自行結束**：兩個 snapshot 測試都在讀到回應後不到 100 ms 內
  (`61 ms`／`64 ms`) 觀察到 `child.wait()` 完成且 `ExitStatus(0)`，不需額外訊號或逾時強制關閉。
- **`-N` 無實質差異，決定不使用**：加與不加 `-N`，spawn→回應（`55 ms`／`55 ms`）與
  回應→結束（`61 ms`／`64 ms`）幾乎相同；由於本檔測試在寫完 request 後就主動
  `drop(stdin)` 送出 EOF（`-N` 的作用範圍正是「stdin 出現 EOF 後把網路 socket 的寫入端也
  關閉」），兩種模式效果一致，選擇**不加 `-N`**（維持 nc 呼叫最簡單）。
- **socket 不存在時 nc 幾乎瞬間失敗**：耗時 `123 ms`（多數是 `wsl.exe` 啟動本身的固定成本，
  WSL 內部 `nc` 本身量測僅約 4 ms，見上方「做法」前的手動驗證），exit code 非 0，
  stderr 為 `nc: /tmp/does-not-exist.sock: No such file or directory`，含關鍵字
  `No such file` 供 design D3 的 EOF／stderr 判別邏輯使用。
- **訂閱事件延遲遠低於 1 秒**：`tab_created` 延遲 `377 ms`、`tab_closed` 延遲 `74 ms`
  （另一次多執行緒跑法量到 `81 ms`／`92 ms`，數字有波動但同樣遠低於門檻），兩者都在
  1000 ms 門檻內；`tab_created` 這次偏高，推測與同機並行跑 Spike 3（Windows 端另開的
  named pipe 連線）造成的系統負載有關，非本橋接方式的固有延遲。
- **`pgrep -a nc` 確認無殘留**：`drop` 訂閱用的 `tokio::process::Child`（`kill_on_drop(true)`）
  後等 500 ms，`wsl.exe -d Ubuntu-24.04 -e pgrep -a nc` 輸出為空字串，代表殺掉 Windows 端的
  `wsl.exe` 包裝行程確實會一併終止 WSL 端底下的 `nc`，沒有留下孤兒行程。

### 對設計的影響

五項全部成立，design D3（`ServerNotRunning` 於首次讀取時判定、以 EOF＋子程序狀態＋stderr
綜合判斷）與 §5.1 的 API 形狀**不需修改**；`Risks / Trade-offs` 一節「nc 緩衝或半關閉行為
讓事件延遲或連線不結束」的風險**不成立**，不需要提前導入 ADR-0002 方案 B（換 Connector）。
`-N` 決定不使用，`ChildStdioConnector`（design 待實作項）送出 request 後應比照本次做法主動
關閉 stdin 觸發 EOF，不依賴 `-N` 這個 nc 專屬旗標（保留可攜性、未來若換其他子程序橋接方式
不受影響）。訂閱延遲的兩次量測（`74–377 ms`／`81–92 ms`）雖有波動，但都遠低於 1000 ms
門檻，不需要調整 §9 或任何逾時常數；如果之後要在文件補一個代表性數字，建議取偏保守的
`377 ms`（tab_created，含並行負載干擾的那次）而非樂觀值。

## Spike 3：訂閱併存與重開

### 問題

驗證三件事：（1）`events.subscribe` 的生命週期訂閱 L（24 種無參數訂閱）與逐 pane 狀態訂閱
S（`pane.agent_status_changed`）能否併存，且 L 是否真的收不到任何狀態變化事件（設計文件
§2.3「沒有全域的 agent 狀態訂閱」）；（2）S 訂閱清單裡混入不存在的 `pane_id` 時，整個
`events.subscribe` request 是否回 `error`（而非部分成功），以及該連線之後是否被 server
關閉；（3）重開 S 訂閱（先開 S2、等 `subscription_started` 後再關 S1）的重疊期間是否會漏收
事件（design D9「Risks」一節列的風險、§4.2 ReopenStatus 的前提）。Windows 端另外只驗證
唯讀情境下（1）（2）是否成立。

### 做法

**指揮官調整**：tasks.md 1.3 原文要全部三項都在 Windows 端做，但「重開重疊期間不丟事件」
需要對某個 agent pane 下指令才會有狀態變化，而 Windows 端是使用者正在用的環境，不可寫入。
改為：狀態變化類測試（S 的事件序列、重開重疊）搬到 WSL 端做，在自己新建的 tab／pane 上
用整合商 API 人工觸發 `pane.agent_status_changed`，完全不碰使用者既有 pane；Windows 端只做
唯讀檢查（L＋S 併存、不存在 pane 回 error），全程只送 `session.snapshot` 與
`events.subscribe`，不觸發任何事件。

**與委派指示的另一處差異（實作時發現）**：原指示要用 CLI
`herdr pane report-agent --source .. --agent .. --state ..` 觸發狀態變化，但 WSL 端 HERDR
0.8.2 build 的 `pane report-agent`／`release-agent` 子命令，只要給超過一個具名 flag 就一律
回 `unknown option`——用空格分隔、`=` 形式、不同引數順序、單一 flag 逐一隔離，反覆驗證都
重現同一結果（`--help` 顯示的多 flag 用法與實際可解析的引數不符）。改用 `herdr api schema`
查到的底層 JSON-RPC 方法 `pane.report_agent`／`pane.clear_agent_authority` 直接對 socket
送（`PaneReportAgentParams`／`PaneClearAgentAuthorityParams` 兩者的必填欄位都通過 schema
驗證），效果相同，一樣算使用「整合商 API」，只是不透過那層壞掉的 CLI 包裝。

新增 `herdr-client/tests/real_herdr_subscriptions.rs`（`#![cfg(windows)]`，4 個 `#[ignore]`
測試），沿用 spike 1 的 named pipe 開法與 spike 2 的 `wsl.exe -e nc -U` 橋接開法，抽出共用
的 `Connection` helper 讓兩種連線共用同一組讀寫介面。因為 crate 的 dev-dependency 沒開
tokio `sync` feature（依指示不得改 `Cargo.toml`），背景事件收集改用 `AtomicBool` 停止旗標
加每輪 100ms 的 `tokio::time::timeout` 輪詢（`read_line` 官方文件保證 cancel-safe，輪詢逾時
不會遺失半行），不用 `oneshot`／`tokio::sync::Mutex`。WSL 端測試自建的 tab 用 `TabGuard`
（Drop 時另開一條 std thread＋獨立迷你 tokio runtime 執行清理），確保斷言 panic 時仍會
`release-agent`／關 tab；實測驗證過一次 panic（見下方除錯過程）後 tab 確實被清理乾淨。

執行 `cargo test -p herdr-client --test real_herdr_subscriptions -- --ignored spike3
--nocapture --test-threads=1`，完整輸出存 `target/spike3-output.txt`。

> **更新（task 5.1）**：本節四個測試已改寫、併入 `tests/real_herdr.rs`（改用
> `Client::subscribe`）後，`real_herdr_subscriptions.rs` 已刪除：
> `spike3_windows_readonly_checks` 拆成 `real_win_lifecycle_and_status_streams`
> （(a) L＋S 併存唯讀）與 `real_win_missing_pane_is_remote`（(b) 不存在 pane →
> `RequestError::Remote`）；`spike3_wsl_missing_pane_fails_whole_request` →
> `real_wsl_missing_pane_fails_whole_request`（改成用 `session.snapshot` 動態查一個
> 既有 pane，不再寫死 `"wD:p1"`，因此改成全程唯讀、不再需要
> `HERDR_CLIENT_TEST_ALLOW_WSL_WRITES`）；`spike3_wsl_reopen_overlap_loses_nothing` →
> `real_wsl_reopen_overlap_loses_nothing`（仍需 opt-in）；
> `spike3_wsl_lifecycle_and_status_streams_coexist` 的「L／S 併存＋by-pane 過濾」
> 精神已由 task 4.3 的 `tests/subscribe.rs`（對假 HERDR）覆蓋，真機那份體力活沒有
> 沿用。task 5.1 實測另外發現：對 `pane.report_agent` 送 `state: "idle"`，WSL 端
> HERDR 回報的 `agent_status` 會在 `idle`／`done` 之間跳動（對應
> `AgentStatus::Done` 文件註解「已 idle 且尚未被看過」——headless 測試 server 沒有
> 真正 UI「看過」這個 pane），`real_wsl_reopen_overlap_loses_nothing` 已放寬對應斷言
> 接受兩者皆可。以下內容維持 spike 當時的原始紀錄，不回溯修改。

### 證據

```text
test spike3_windows_readonly_checks ... Windows 端目前 pane 數: 6
(a) 5 秒內 S 收到 0 筆
(b) 回應: {"error":{"code":"pane_not_found","message":"pane wZ:p999 not found"},
          "id":"B:sub:1:probe"}
ok
test spike3_wsl_lifecycle_and_status_streams_coexist ... 建立 tab: wD:tB, pane P = wD:pS
S 訂閱涵蓋 pane 數（含 P）: 6
S 收到事件數: 4
L 收到事件數: 136
S 完整狀態序列: ["working", "blocked", "idle", "unknown"]
ok
test spike3_wsl_missing_pane_fails_whole_request ...
回應: {"error":{"code":"pane_not_found","message":"pane wZ:p999 not found"},
      "id":"M:sub:1:probe"}
ok
test spike3_wsl_reopen_overlap_loses_nothing ... 建立 tab: wD:tC, pane P2 = wD:pV
[121.9288ms] S1 started
[181.8885ms] S2 started
[651.3248ms] S1 已關閉，重疊期收到 1 筆（working）
[277–1487ms] S2 依序收到: working, blocked, idle, working
ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 9.75s
```

### 結論

1. **L 與 S 併存是否互不影響**：成立。WSL 與 Windows 兩端皆同時開 L（24 種）＋S 並保持，
   兩者都各自收到 `subscription_started`，互不干擾；Windows 端 5 秒視窗內 S 收到 0 筆屬
   預期（唯讀檢查沒有人為觸發任何狀態變化）。
2. **L 是否確實收不到狀態事件**：成立。`spike3_wsl_lifecycle_and_status_streams_coexist`
   對 P 觸發 working／blocked／idle 三次狀態變化期間，L 收到的 136 筆生命週期事件中
   `pane_agent_status_changed`（生命週期版本，底線命名）出現 **0** 次；設計文件 §2.3
   「沒有全域的 agent 狀態訂閱」得證。
3. **不存在的 pane 是否讓整個 request 回 error 及其 code／message**：成立，WSL 與
   Windows 兩端一致：`code = "pane_not_found"`、`message = "pane wZ:p999 not found"`。
   附帶發現：回應的 `id` 不是原始請求 id，而是 server 內部探測用的
   `<原id>:sub:<n>:probe`（例如 `M:sub:1:probe`）——斷言時不可假設回應 id 與送出時相符。
   WSL 端另確認：回 error 後該連線隨即 EOF（`nc` 自行結束、Rust 端 `read_line` 回
   `None`），與 named pipe／nc 橋接的既有假設一致。
4. **重開重疊期間是否兩邊都收到、S1 關閉後 S2 是否無缺口**：成立。重疊期間（S1、S2 都還
   開著）觸發一次 `working`，S1、S2 都收到；關閉 S1 後再連續觸發 blocked／idle／working
   三次，S2 完整依序收到全部 4 筆（working, blocked, idle, working），時間戳顯示無缺口
   （277ms→1487ms 平均間隔約 300–400ms，與觸發間隔一致）。
5. **Windows 端唯讀檢查結果**：兩項皆成立，全程無寫入操作：（a）L＋S 併存 5 秒皆收到
   `subscription_started`，5 秒內 L 收到 6 筆既有 pane 的 `pane_updated`（sidebar 內容更新，
   與本次測試無關的背景活動）、S 收到 0 筆（6 個既有 pane 皆無 agent 活動，屬正常）；
   （b）S 清單 = [snapshot 第一個 pane id, `wZ:p999`] 回 `error`，`code`／`message` 與
   WSL 端完全一致。

補充：`release-agent`（`pane.clear_agent_authority`）本身也會再觸發一筆
`agent_status: "unknown"` 的 `pane.agent_status_changed`（無 `agent` 欄位），不屬於任務要
驗的三段／四段序列，兩個 WSL 測試都刻意把「讀取並斷言序列」放在呼叫 `release-agent` 之前，
避免這筆額外事件混進斷言。

### 對設計的影響

五項結論全數成立，設計文件 §4.2 的 ReopenStatus 設計（「先開新的 S 並等到
`subscription_started`，再關舊的 S；重疊期間重複收到的狀態事件是冪等的」）**不需修改**；
§7.1「pane 集合改變 → ReopenStatus；200ms 內合併成一次」的重疊不丟事件前提在真機下得到
驗證（本次沒有量測 200ms 合併窗口本身，那是另一個獨立機制，不在本次驗收範圍）。

結論 3 的附帶發現（`error` 回應的 `id` 帶 server 內部探測後綴、不等於送出時的 id）建議
記在 task 3.3／4.2 的合約測試與 `RequestError` 實作備忘：解析 `events.subscribe` 的
error 回應時不可用 `id` 比對來源請求。

**額外觀察（不在本次驗收範圍，留給指揮官決定是否深究）**：WSL 端 L 訂閱在
**剛建立連線的瞬間**，於本次真機環境下觀察到先收到一批看似是先前（同一 session 內、數分鐘
前）在同一 workspace 建立並關閉過的測試 tab 的 `tab_created`／`tab_closed`，且呈現「建立
tab N 時才補推 tab N-2 的 closed」的一代延遲模式；這些 tab 在斷言當下早已不存在，不影響
本次任何斷言（全部斷言都用自己建立的 tab_id／pane_id 過濾，如指揮官指示）。若屬實，代表 L
訂閱在建立當下可能不是純粹「只推送訂閱之後的新事件」，而會補送一段近期歷史；change 1b
的連線迴圈若假設 L 只送未來事件，可能需要重新查證（本次未進一步深入，因為不影響 spike 3
的驗收範圍）。

## Spike 4：無額外視窗

### 問題

`ChildStdioConnector`（ADR-0002）每次連線都要在 Windows 端 spawn 一個 `wsl.exe -e nc -U <socket>`
子程序。若不加任何旗標，子程序有沒有機會在畫面上彈出一個新的主控台視窗，干擾使用者？
design D9 決定一律加 `CREATE_NO_WINDOW`（`0x0800_0000`），本 spike 驗證這個決定在
HERDR pane 內是否真的達到「無新視窗」，並補測父程序本身沒有可見主控台（未來 cockpit.exe
的啟動情境）時是否仍然成立。

### 做法

1. 新增 `herdr-client/examples/spike4_no_window.rs`：以 `tokio::process::Command` 啟動 spike 2
   查證過的橋接指令（`wsl.exe -d Ubuntu-24.04 -e nc -U /home/<user>/.config/herdr/herdr.sock`，
   stdin/stdout/stderr 全部 `Stdio::piped()`、`kill_on_drop(true)`），非對照組加
   `#[cfg(windows)] .creation_flags(0x0800_0000)`；`--with-window` 旗標跳過這行作對照組。
   送一次 `session.snapshot`，印出本程式與子程序 pid、`protocol`／`version`、回應 bytes、
   子程序 exit status。
2. 寫 `target/spike4-check.ps1`（不進 git）：用 `Get-Process | Where MainWindowHandle -ne 0`
   取執行前基準視窗清單，另計 `conhost`／`OpenConsole`／`WindowsTerminal` 程序數基準；用
   `Start-Process -PassThru` 背景啟動待測程式，每 200 ms 取樣一次直到 `$proc.HasExited`，
   記錄期間曾出現過、執行前不存在的視窗（依 process Id 判斷），以及三種主控台宿主程序數的
   期間最大值。
3. 主實驗：`Start-Process -FilePath cargo -ArgumentList run,-p,herdr-client,--example,
   spike4_no_window[,--,--with-window] -NoNewWindow`（`-NoNewWindow` 模擬 cargo／example
   父程序附掛在 HERDR pane 既有 console 上），CREATE_NO_WINDOW 組與 `--with-window` 對照組
   各跑 2 次。
4. 補充實驗：`Start-Process -FilePath target\debug\examples\spike4_no_window.exe -WindowStyle
   Hidden[,-ArgumentList --with-window]`，模擬父程序本身沒有可見 console（未來 cockpit.exe
   的啟動情境）；有旗標／無旗標各跑 1 次。
5. 輸出存 `target/spike4-output.txt`。

### 證據

六次試驗（4 主實驗 + 2 補充實驗）新出現視窗數皆為 0；`conhost` 計數在 22–25 之間小幅
波動（本機背景本就有 22 個以上 conhost，來自其他 HERDR pane）：

```text
--- CREATE_NO_WINDOW #1（cargo run, -NoNewWindow） ---
耗時: 372 ms, 取樣次數: 1, exit code: 0
新出現視窗數: 0
conhost: baseline=22 max=23

--- CREATE_NO_WINDOW #2 --- 耗時: 395 ms, 新出現視窗數: 0, conhost: 24 -> 24
--- 對照組 --with-window #1 --- 耗時: 335 ms, 新出現視窗數: 0, conhost: 23 -> 23
--- 對照組 --with-window #2 --- 耗時: 337 ms, 新出現視窗數: 0, conhost: 23 -> 23

--- 補充：無 console 父程序 + CREATE_NO_WINDOW（-WindowStyle Hidden） ---
耗時: 429 ms, 新出現視窗數: 0, conhost: baseline=23 max=25

--- 補充：無 console 父程序 + 對照組 --with-window（-WindowStyle Hidden） ---
耗時: 365 ms, 新出現視窗數: 0, conhost: baseline=23 max=24
```

（OpenConsole 全程 7、WindowsTerminal 全程 1，兩者六次試驗皆無變化，略。完整輸出見
`target/spike4-output.txt`。）

### 結論

- 從 HERDR pane 內啟動（父程序附掛既有 console）時，加 `CREATE_NO_WINDOW` 兩次試驗皆無新視窗。
- 對照組（同樣從 pane 內啟動、不加旗標）兩次試驗也皆無新視窗——與加旗標組**無法區分**。
  合理解讀：父程序（cargo／example）本身已附掛在 HERDR pane 的 console 上，`wsl.exe`
  子程序預設會繼承這個既有 console 而非另開一個，所以在這個啟動情境下，是否加
  `CREATE_NO_WINDOW` 對「看不看得到新視窗」不構成可觀察差異；旗標要防的是父程序**沒有**
  既有 console 可繼承的情境。
- 補充實驗（父程序用 `-WindowStyle Hidden` 啟動、沒有可見 console）：加旗標與對照組同樣皆無
  新視窗（`conhost` 計數雖各有 +2／+1 的小幅波動，但基準值本身在 22–25 間跳動、且兩組都有
  類似幅度的波動，判斷屬本機背景噪音而非本測試觸發，不構成有意義差異）。這與預期有落差：
  理論上「無 console 父程序 + 不加旗標」應該較可能觸發新視窗，但六次試驗（含補充）都沒觀察到
  任何一次 `MainWindowHandle != 0` 的新視窗，包括對照組。可能原因是 stdin/stdout/stderr 全部
  `Stdio::piped()`，`nc` 不需要互動式終端，Windows／WSL 未必會為它建立可見主控台，這件事本身
  可能比 `CREATE_NO_WINDOW` 旗標更關鍵。
- 侷限：單次試驗實際執行時間僅約 330–430 ms（多數落在 WSL 往返延遲 0.1–0.3 秒的量級），
  200 ms 取樣間隔下大多數試驗只取到 1 個樣本，無法排除中間曾出現、存在時間遠短於取樣間隔的
  視窗；`Get-Process` 的 `MainWindowHandle` 判定法本身也抓不到「建立但立刻隱藏」或未取得
  主視窗控制代碼的主控台。以上是自動化腳本的量測結果，**不等於人眼目視複驗**；建議請使用者
  在 HERDR pane 內親自跑一次 `cargo run -p herdr-client --example spike4_no_window`（與
  `-- --with-window` 對照組）目視確認無閃現視窗，此結論才算完全成立。

### 對設計的影響

六次試驗皆未觀察到新視窗，`目視無新視窗` 這項驗收條件在自動化量測層級成立（待使用者目視複驗，
見上）；沒有出現「加旗標才無視窗、不加就有視窗」這種能證明旗標必要性的對照差異，但也沒有出現
任何反例。ADR-0002「Windows 上子程序一律加 `CREATE_NO_WINDOW`」與設計文件 §5.1 對應的決定
**不需修改**：這是防禦性、低成本的旗標（在父程序無 console 可繼承的情境下才會真正發揮作用，
例如未來 cockpit.exe 以無主控台方式啟動），本次試驗沒有證據顯示它有害或多餘，維持現狀即可。

## Spike 5：WSL 探測與輸出編碼

### 問題

`wsl.exe --list --running --quiet` 是否會喚醒已停止的 WSL 虛擬機（ADR-0002 的探測前提）；
`wsl.exe` 系列指令的 stdout 是什麼位元組編碼，change 1b 的探測與 nc 橋接實作要用哪種解碼
策略才不會把發行版清單解析成亂碼或誤判編碼。

### 做法

新增 `herdr-client/examples/spike5_wsl_probe.rs`，用 `tokio::process::Command` 執行四種
`wsl.exe` 呼叫（`--list --running --quiet` 與 `--list --quiet`，各在預設環境與
`WSL_UTF8=1` 環境各跑一次），對每次 stdout 印出：位元組長度、前 16 bytes 十六進位、是否含
NUL byte、`String::from_utf8` 結果、以 `from_utf16_lossy` 解碼後裁 `\r` 與空行的清單、
exit code；並依實測（非記憶）印出建議解碼策略。執行前後各用 PowerShell
`Get-Process -Name vmmemWSL,vmmem` 取一次程序快照。本次執行時虛擬機正在跑（供 spike 2～4
使用），(a) 停止狀態不重驗，沿用 `docs/research/2026-09-13/local-checks.txt`
2026-09-13 的實測。完整輸出見 `target/spike5-output.txt`。

### 證據

```text
Get-Process 前: Id=34368 ProcessName=vmmemWSL
Get-Process 後: Id=34368 ProcessName=vmmemWSL（同一個 pid，無新程序）

--- --list --running --quiet（預設環境） ---
exit code: 0；bytes 長度 28
前16 bytes: 55 00 62 00 75 00 6e 00 74 00 75 00 2d 00 32 00
含 NUL: true；from_utf8: Ok（偽陽性，見結論）
UTF-16LE 解碼: ["Ubuntu-24.04"]

--- --list --running --quiet（WSL_UTF8=1） ---
exit code: 0；bytes 長度 14；含 NUL: false
前16 bytes: 55 62 75 6e 74 75 2d 32 34 2e 30 34 0d 0a
UTF-8 內容即 "Ubuntu-24.04\r\n"

--- --list --quiet（不加 --running，預設環境） ---
exit code: 0；bytes 長度 60；含 NUL: true
UTF-16LE 解碼: ["Ubuntu-24.04", "docker-desktop"]
```

（完整四組輸出與程序快照見 `target/spike5-output.txt`，本節僅摘錄關鍵行。）

### 結論

- (a) 虛擬機停止時是否喚醒：**本次未重驗**——執行當下虛擬機正在跑（`Ubuntu-24.04`，供
  spike 2～4 使用），是否 `wsl.exe --shutdown` 由使用者決定；沿用
  `docs/research/2026-09-13/local-checks.txt` 2026-09-13 的實測（前後 `Get-Process`
  皆無 `vmmem`／`vmmemWSL`，兩次 `--list --running --quiet` 皆空輸出、exit code 0）。
  待使用者決定是否執行 `wsl.exe --shutdown` 後，用同一支 example 補驗一次。
- (b) 虛擬機在跑時：兩種 `--list` 變體、預設環境下輸出皆為 **UTF-16LE**（每字元後接
  `00` byte），皆含 NUL byte，`from_utf16_lossy` 解碼後可正確切出發行版清單。
  `wsl.exe --list --running --quiet` 有發行版執行時 exit code 為 `0`（與無發行版時的
  exit code 0 相同，不能靠 exit code 判斷有沒有結果，只能看 stdout 是否為空）。
  `wsl.exe --list --quiet`（不加 `--running`）與 `--running` 版本編碼相同，同為
  UTF-16LE。
- **`WSL_UTF8=1` 實測有效**：設定後兩種 `--list` 變體的 stdout 都變成不含 NUL byte 的
  合法 UTF-8（`"Ubuntu-24.04\r\n"`），bytes 長度也從偶數的 UTF-16LE 長度變成對應的
  UTF-8 長度（例如 28 bytes → 14 bytes）。此為本機 2026-09-13 實測結果，非引用
  WSL 文件記憶。
- **重要陷阱**：`String::from_utf8(bytes).is_ok()` 在預設環境（UTF-16LE）輸出上也回傳
  `Ok`——因為 NUL byte（`U+0000`）本身就是合法的單位元組 UTF-8 字元，ASCII 範圍字元
  交錯 `00` 組成的位元組序列逐一看都是合法 UTF-8 code point，只是拼出來的字串是
  `"U\0b\0u\0n\0t\0u…"` 這種不可用內容。**不能用 `from_utf8().is_ok()` 判斷是否為
  UTF-8，必須先檢查 `has_nul`**，這是本次實測中發現並修正的邏輯錯誤（先寫成
  `if is_utf8 { ... } else if has_nul { ... }`，實跑後看到輸出把 UTF-16LE 誤判成
  UTF-8，才把判斷順序改成先看 `has_nul`）。
- 建議 change 1b 的探測實作：**不依賴呼叫端是否設定 `WSL_UTF8` 環境變數**，一律以
  `from_utf16_lossy` 解碼 `wsl.exe` 系列指令的 stdout（奇數長度時記警告，只丟最後一個
  落單 byte），再裁 `\r` 與空行取得發行版清單；判斷「有沒有執行中的發行版」看解碼後清單
  是否為空，不要看 exit code。

### 對設計的影響

- ADR-0002「連線前先以 `wsl.exe --list --running --quiet` 探測發行版是否在跑」的探測步驟
  維持不變；(a) 半段（停止時不喚醒）本次未重驗，沿用既有實測結論，待使用者決定
  `wsl.exe --shutdown` 後應補驗一次並回填本節。
- 設計文件 §2.9 建議補一句編碼處理說明：目前只寫「輸出皆無 `vmmem`」，未提編碼；應加註
  「`wsl.exe --list` 系列指令預設輸出 UTF-16LE（含交錯 NUL byte，`from_utf8().is_ok()`
  會誤判為合法 UTF-8，不可用此法判斷編碼）；探測與 nc 橋接實作一律以
  `String::from_utf16_lossy` 解碼，不依賴 `WSL_UTF8` 環境變數；判斷有無執行中發行版看
  解碼後清單是否為空，不看 exit code（恆為 0）」。

## 總結與 go/no-go

| Spike | 結論 | 對設計的影響 |
|---|---|---|
| 1 named pipe | 成立：含冒號與反斜線的名稱可直接開；3 條並發遇到 2 次 `ERROR_PIPE_BUSY`，重試邏輯必要 | §5.1 不改；design D9 的重試維持 |
| 2 WSL nc 橋接 | 成立：server 關連線後 nc 自行結束；`-N` 無差異不使用；socket 不存在時約 100 ms 內以 exit 1 結束且 stderr 含 `No such file`；訂閱事件延遲 305 ms／83 ms | ADR-0002 方案 A 維持；design D3 前提成立 |
| 3 訂閱併存與重開 | 成立：L／S 互不影響；L 收不到狀態事件；不存在 pane 整個 request 回 `pane_not_found`；重開重疊期兩邊都收到、S1 關後 S2 無缺口 | §4.2 ReopenStatus 不改；**新發現**：探測失敗的 error 回應 `id` 為 `<request id>:sub:<n>:probe`，client 要視為相符（design D12） |
| 4 無額外視窗 | 成立（六次試驗皆無新視窗），但加不加旗標無法區分；待使用者目視複驗 | ADR-0002 的 `CREATE_NO_WINDOW` 維持為防禦性設定 |
| 5 探測與編碼 | (b) 成立：預設輸出 UTF-16LE、exit code 恆 0、`WSL_UTF8=1` 有效但不依賴；(a) 本次未重驗，沿用 `local-checks.txt` | 1b 探測一律以 UTF-16LE 解碼、以清單是否為空判斷；(a) 待使用者決定 `wsl.exe --shutdown` 後補驗 |

**go/no-go：GO。** 五個 spike 都沒有推翻設計文件 §5、§11 與 ADR-0001／0002；需要回寫的只有
design D12（error id 後綴）與兩個待使用者的動作（spike 4 目視、spike 5(a) 補驗）。

**附帶觀察（未深究，列為 change 1b 真機驗收的待查證項）**：WSL 0.8.2 的 L 訂閱剛建立時推送了
數分鐘前已建立並關閉的舊 tab 的 `tab_created`／`tab_closed`，且出現「建立 tab N 才補推 tab N-2
的 closed」的延遲模式。§4.2 規定 authoritative snapshot 之前的事件一律丟棄，snapshot 之後的
過期事件會走 Drift 重拿，功能上可容忍；但 1b 要在真機驗收時確認 Windows 0.9.0 是否也有此行為。

**產物**：`herdr-client/tests/real_herdr.rs`、`real_herdr_wsl.rs`、`real_herdr_subscriptions.rs`
（全部 `#[ignore]`）、`herdr-client/examples/spike4_no_window.rs`、`spike5_wsl_probe.rs`、
fixture `herdr-client/tests/fixtures/snapshot-p22.json`、`snapshot-p20.json`、
`events-lifecycle-p20.ndjson`、`events-status-p20.ndjson`、去識別化腳本
`docs/research/2026-09-13/deidentify-fixture.py`。

> **更新（task 5.1）**：`real_herdr_wsl.rs`、`real_herdr_subscriptions.rs` 已改寫並併入
> `real_herdr.rs`（改用 `Client`）後刪除，見上面三節各自的「更新（task 5.1）」註記；
> `real_herdr.rs` 本身也已整份改寫（原本的 spike 1 raw 連線測試 → `real_win_snapshot`）。
> `examples/spike4_no_window.rs`、`spike5_wsl_probe.rs` 與既有 fixture 不受影響，原樣保留。
