# tasks：herdr-client

> **crate gate**（每個 task 結尾都要跑，0 error 才算完成）：
> `cargo fmt --check && cargo clippy -p herdr-client --all-targets -- -D warnings && cargo test -p herdr-client`
>
> **spike 先決條件**：Windows 端 HERDR 在跑（`herdr status server`）；spike 2、4 另需 WSL 端
> HERDR 在跑（`wsl.exe -e bash -lc "~/.local/bin/herdr status server"`）。全程只送
> `session.snapshot` 與 `events.subscribe`；不得用 `herdr server stop` 製造斷線。
> spike 紀錄檔：`docs/research/<執行日期>/change-1a-spikes.md`，每個 spike 一節，含
> 「問題／做法／證據（指令與輸出）／結論／對設計的影響」。從 Git Bash 呼叫 `wsl.exe` 傳
> POSIX 路徑要前置 `MSYS_NO_PATHCONV=1`（機制見 `docs/handover.md` §4，原案例為 `/mnt/c/...`；
> `/home/...` 是否同樣被轉換，spike 2 順手驗證並記錄）。
> 真機測試目標由環境變數 `HERDR_CLIENT_TEST_WIN_SOCKET`、`HERDR_CLIENT_TEST_WSL_DISTRO`、
> `HERDR_CLIENT_TEST_WSL_SOCKET` 指定（design D11）。

## 1. Spike（設計文件 §11；任一不成立就停在 1.6）

- [x] 1.1 Spike 1 named pipe：建立根 `Cargo.toml`（workspace，members 只有 `herdr-client`）與
  `herdr-client` crate 骨架（edition 2024、空 `lib.rs`、dev-dependency `tokio` 開 `net`、
  `io-util`、`rt-multi-thread`、`macros`、`process`、`time`）；寫 `herdr-client/tests/real_herdr.rs`
  的 `#[ignore]` 測試 `spike1_named_pipe_session_snapshot`（以 `ClientOptions::open` 開
  `\\.\pipe\` 加 `herdr status server` 回報的 socket 路徑，送一行
  `{"id":"1","method":"session.snapshot","params":{}}`，讀一行並印出）與
  `spike1_three_concurrent_connections`（同時開 3 條連線各送一次，記錄是否出現
  `ERROR_PIPE_BUSY` 231）。驗收：`cargo test -p herdr-client --test real_herdr -- --ignored spike1`
  通過；原始回應去識別化後存 `herdr-client/tests/fixtures/snapshot-p22.json`；兩項結果寫入
  spike 紀錄。結尾跑 crate gate。
- [x] 1.2 Spike 2 WSL nc 橋接：`#[ignore]` 測試 `spike2_wsl_nc_snapshot`（`tokio::process::Command`
  啟動 `wsl.exe -d <distro> -e nc -U <socket>`，stdin 送 `session.snapshot` 一行、stdout 讀一行；
  記錄 server 關閉連線後 nc 是否自行結束、加與不加 `-N` 的差異）與
  `spike2_wsl_nc_subscribe_latency`（同法送含 `pane.created` 等生命週期訂閱的
  `events.subscribe`，讀到 `subscription_started` 後在 WSL 端 HERDR 手動新開一個 pane，量事件
  到達延遲）。驗收：兩個測試通過；延遲小於 1 秒；去識別化 snapshot 存
  `tests/fixtures/snapshot-p20.json`；nc 參數決定與三項量測寫入 spike 紀錄。結尾跑 crate gate。
- [x] 1.3 Spike 3 訂閱併存與重開：`#[ignore]` 測試 `spike3_lifecycle_and_status_streams_coexist`
  （Windows 端同時開 L：24 種生命週期、S：snapshot 中每個 pane 一筆
  `pane.agent_status_changed`，各自收 10 秒事件）、`spike3_missing_pane_fails_whole_request`
  （S 清單加入不存在的 `wZ:p999`，預期整個 request 回 `error`，記錄 `code` 與 `message`）、
  `spike3_reopen_overlap_loses_nothing`（開 S2 與 S1 同清單，S2 收到 `subscription_started`
  後關 S1；期間在某個 agent pane 手動下一句指令，比對 S1 關閉前後 S2 的 `agent_status` 序列
  無缺口）。驗收：三個測試通過；擷取的原始事件行去識別化後存
  `tests/fixtures/events-lifecycle-p22.ndjson` 與 `tests/fixtures/events-status-p22.ndjson`；
  三項結論寫入 spike 紀錄。結尾跑 crate gate。（執行時調整：狀態變化類測試改在 WSL 端以底層
  `pane.report_agent` 製造事件、Windows 端只做唯讀檢查，fixture 為 p20；WSL 0.8.2 的
  `herdr pane report-agent` CLI 帶多個旗標會回 unknown option。理由與證據見 spike 紀錄 Spike 3。）
- [x] 1.4 Spike 4 無額外視窗：`herdr-client/examples/spike4_no_window.rs` 以
  `creation_flags(0x0800_0000)`（`CREATE_NO_WINDOW`）啟動 spike 2 的 `wsl.exe` 指令完成一次
  `session.snapshot`；參數 `--with-window` 跑不加旗標的對照組。驗收：在 HERDR pane 內執行
  `cargo run -p herdr-client --example spike4_no_window` 目視無新視窗；對照組行為一併寫入
  spike 紀錄。結尾跑 crate gate。
- [x] 1.5 Spike 5 探測不喚醒虛擬機：`herdr-client/examples/spike5_wsl_probe.rs` 執行
  `wsl.exe --list --running --quiet`，印出 stdout 前 16 bytes 的十六進位與依實際編碼（預期
  UTF-16LE）解碼後的發行版清單。驗收：（a）WSL 虛擬機停止時（`Get-Process -Name vmmemWSL,vmmem`
  無結果；若虛擬機在跑，是否 `wsl.exe --shutdown` 由使用者決定，它會關掉 WSL 端 HERDR 與所有
  pane；不執行則沿用設計文件 §2.9 的實測）連跑兩次，前後 `Get-Process` 仍無結果；（b）虛擬機
  在跑時輸出含 `Ubuntu-24.04`；編碼處理方式寫入 spike 紀錄供 change 1b 沿用。結尾跑 crate gate。
- [x] 1.6 Spike 彙整與 go/no-go：補齊 `change-1a-spikes.md` 每節的「對設計的影響」，
  `docs/research/<執行日期>/README.md` 列出本次產出。驗收：在 repo 根跑
  `markdownlint-cli2 "**/*.md"` 為 0 issue；五個 spike 全部成立才進入第 2 組；任一不成立就停止、
  回報使用者，經 `/opsx:update herdr-client` 與修改設計文件 §5、§11 後才繼續。

## 2. Transport（spec `herdr-transport`）

- [x] 2.1 把 `tokio` 移為正式 `[dependencies]`，加入 `async-trait`、`serde`、`serde_json`、`thiserror`、
  `tracing`；定義 `NdjsonStream`、`Connector`、`ConnectError`，以及 `NamedPipeConnector`（cfg windows，
  `ERROR_PIPE_BUSY` 每 50 ms 重試、上限 1 秒）與 `UnixSocketConnector`（cfg unix）；`describe()`
  格式為 `named-pipe <path>`、`unix-socket <path>`。驗收測試（`herdr-client/tests/transport.rs`）：
  `connect_missing_target_is_server_not_running`、`describe_mentions_kind_and_target`、
  `two_connections_are_independent`、Windows 專屬 `named_pipe_roundtrip_with_colon_path`（以 tokio
  `ServerOptions` 開含冒號與反斜線名稱的 pipe 當對端）、unix 專屬 `unix_socket_roundtrip`。
  結尾跑 crate gate。
- [x] 2.2 `ChildStdioConnector { command, args }`：`tokio::process` spawn、stdin／stdout 接成
  `NdjsonStream`、stderr 逐行進 `tracing::debug`、`kill_on_drop`、Windows `CREATE_NO_WINDOW`；
  讀到任何一行前就 EOF 且子程序已結束時回 `ConnectionRefused` 類錯誤並附 stderr（design D3）；
  spawn 失敗回 `Spawn`；`describe()` 格式為 `child <command> <args>`。先決：決定 design 開放問題的
  假對端做法並寫進 `herdr-client/README.md`。驗收測試：`child_roundtrip`、
  `child_stderr_not_in_stream`、`child_exits_immediately_is_server_not_running`、
  `child_missing_command_is_spawn`、`child_killed_on_drop`（以 pid 查作業系統確認已結束）。
  結尾跑 crate gate。
- [x] 2.3 本機預設 socket 路徑解析 `default_socket_path()`：`HERDR_SOCKET_PATH` 優先，其次
  `HERDR_SESSION` 對應 `<config_dir>/sessions/<name>/herdr.sock`，否則 `<config_dir>/herdr.sock`；
  config_dir 依 HERDR 原始碼 `src/config/io.rs:30-68`（commit `bafbc0949`，task 執行時 clone 查證）：
  `XDG_CONFIG_HOME` 有設（含空字串）則跨平台優先，否則 Windows `%APPDATA%` → `%USERPROFILE%\AppData\Roaming`
  → `$HOME/.config`，unix `$HOME/.config`，皆無則暫存目錄；README 引用檔案:行號。環境變數與 config_dir 以參數注入，不在測試中改真實環境。驗收測試（單元）：
  `default_path_without_env`、`socket_path_env_wins`、`session_env_maps_to_sessions_dir`。
  結尾跑 crate gate。

## 3. Observer 子集型別（spec `herdr-observer-types`）

- [x] 3.1 `AgentStatus`（小寫五值加 `#[serde(other)]` Unknown，不提供任何「完成」判斷）、
  `WorkspaceInfo`、`TabInfo`、`PaneInfo`、`AgentInfo`、`SessionSnapshot`（`layouts` 以
  `serde_json::Value` 保留）、`SessionSnapshotResult`、`PaneReadParams`、`PaneReadResult`、
  `ReadSource`；全部 `#[serde(default)]`、非必填欄位 `Option`、不用 `deny_unknown_fields`。
  驗收測試（`tests/types.rs`）：`snapshot_fixture_p22_parses`、`snapshot_fixture_p20_parses`
  （數量與 `version`、`protocol` 對照 fixture）、`unknown_fields_ignored_and_missing_optional_is_none`、
  `agent_status_unknown_catch_all`、`agent_status_done_parses_as_done`。結尾跑 crate gate。
- [x] 3.2 事件型別：`EventKind`（26 種）、`SubscriptionEventKind`（3 種）、`Subscription`（24 個
  單元變體加 `PaneAgentStatusChanged { pane_id }`，序列化為含點號 `type` 的物件）、
  `EventsSubscribeParams`、spec 列出的 payload 型別（含 `pane_moved` 的四個選填欄位與 `pane_agent_detected` 的
  `final_status`）。驗收測試：
  `subscription_serializes_dotted_type_only`、`per_pane_subscription_has_pane_id_and_no_filter`、
  `event_kind_roundtrips_all_26`、`event_fixtures_parse_with_matching_payload_types`（逐行依
  `event` 選型別）、`pane_moved_keeps_both_ids`、`pane_moved_optional_fields_default_to_none`、
  `pane_agent_detected_released_with_null_agent`。
  結尾跑 crate gate。
- [x] 3.3 合約測試：把 `docs/research/2026-09-13/` 的兩份 schema 複製到 `tests/fixtures/`
  （`schema-p22.json`、`schema-p20.json`），research README 註明複本位置；dev-dependency
  `jsonschema`（draft 2020-12）；依 design D8 以整份文件為根、頂層 `$ref` 選根。驗收測試
  （`tests/contract.rs`）：`requests_validate_against_both_schemas`（`session.snapshot`、
  `pane.read`、含 24 種生命週期加 3 筆每 pane 的 `events.subscribe`）、
  `snapshot_fixtures_validate_as_success_response`、
  `event_fixtures_validate_as_event_or_subscription_event`。結尾跑 crate gate。

## 4. Client 與假 HERDR（spec `herdr-request`、`herdr-event-subscription`）

- [x] 4.1 假 HERDR `herdr_client::testing::FakeHerdr`（feature `test-support`，以 self
  dev-dependency 開啟）：監聽真實 transport（Windows named pipe、unix socket，名稱用暫存路徑加
  隨機後綴）；每連線讀第一行決定 method；可設定 snapshot fixture、`events.subscribe` 的腳本
  事件、探測失敗的 `pane_id` 集合、回錯 id、回非 JSON、回 `pong` 型 `result`、收到後直接關閉、
  推壞行、正常關閉、非正常中斷；記錄每條連線收到的行供斷言。驗收測試（`tests/fake_herdr.rs`）：
  `fake_serves_snapshot_over_native_transport`、`fake_records_received_lines`、
  `fake_closes_after_single_response`。結尾跑 crate gate。
- [x] 4.2 `Client::request`、`Request` trait（`SessionSnapshotRequest`、`PaneReadRequest`）、
  `RequestError`（Connect、Remote、Protocol、Io 四類）、程序內遞增 id。驗收測試
  （`tests/request.rs`）：`request_snapshot_ok_and_connection_closed_after_one_line`、
  `request_ids_are_unique`、`request_remote_error_keeps_code_and_message`、
  `request_id_mismatch_is_protocol`、`request_non_json_is_protocol`、
  `request_wrong_result_type_is_protocol`、`request_closed_before_reply_is_connect_error_with_reason`、
  `request_server_not_running`。結尾跑 crate gate。
- [x] 4.3 `Client::subscribe`、`EventStream`（`next()` 回 `Option<Result<IncomingEvent, StreamError>>`，
  design D6）、`IncomingEvent`（Lifecycle、PerPane、Unknown 三變體，design D4）。驗收測試
  （`tests/subscribe.rs`）：`subscribe_lifecycle_24_sends_expected_types`、
  `subscribe_per_pane_n_sends_pane_ids`、`subscribe_missing_pane_is_remote_and_no_stream`、
  `two_streams_are_independent`、`events_route_to_lifecycle_per_pane_unknown`、
  `malformed_lines_are_skipped_with_warn`、`eof_ends_stream_without_error`、
  `io_error_is_reported_then_stream_ends`。結尾跑 crate gate。
- [x] 4.4 子程序橋接接上 Client：以 2.2 決定的假對端讓 `ChildStdioConnector` 接到假 HERDR。驗收
  測試：`child_bridge_request_roundtrip`、`child_bridge_stream_ends_and_child_exits`、
  `child_bridge_event_arrives_within_1s`。結尾跑 crate gate。

## 5. 真機測試、文件與收尾

- [x] 5.1 把 `tests/real_herdr.rs` 的 spike 測試改寫成用 `Client` 的 `#[ignore]` 真機測試：
  `real_win_snapshot`、`real_win_lifecycle_and_status_streams`、`real_wsl_snapshot_via_child_stdio`、
  `real_wsl_lifecycle_stream_via_child_stdio`；並在 Windows 端擷取去識別化的
  `tests/fixtures/events-lifecycle-p22.ndjson` 與 `events-status-p22.ndjson`（狀態事件需使用者在某個
  agent pane 下一句指令）；`herdr-client/README.md` 寫用途、公開 API 一覽、
  三個環境變數、執行方式、假對端做法。驗收：兩端 HERDR 在跑時
  `cargo test -p herdr-client --test real_herdr -- --ignored` 全過並貼輸出；README 過
  markdownlint。結尾跑 crate gate。
- [x] 5.2 全 workspace gate 與交接：`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、
  `cargo test --workspace`、repo 根 `markdownlint-cli2 "**/*.md"` 全 0 error；
  `openspec validate herdr-client` 無 ERROR（strict 模式的 SHALL/MUST 警告是 validator 對英文的
  啟發式，中文「系統必須」不適用，不視為失敗）；Codex adversarial review（`AGENTS.md` 路徑）
  的 findings 逐條實測後處理；重寫 `docs/handover.md`（狀態改為 change 1a 完成、下一步 change 1b
  propose）。驗收：以上指令輸出貼在完成回報中。
