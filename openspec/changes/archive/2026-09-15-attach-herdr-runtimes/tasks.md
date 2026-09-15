# tasks：attach-herdr-runtimes

> **crate gate**（每個 task 結尾都要跑該 task 動到的 crate，0 error 才算完成）：
> `cargo fmt --check && cargo clippy -p <crate> --all-targets -- -D warnings && cargo test -p <crate>`；
> 動到 `.md` 另在 repo 根跑 `markdownlint-cli2 "**/*.md"`。第 4 組的 4.1–4.4 不動程式碼，由 4.6 的
> 全 workspace gate 收：
> `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test --workspace && markdownlint-cli2 "**/*.md"`。
>
> **真機先決條件**（只有第 4 組需要）：Windows 端 HERDR 在跑（`herdr status server`）；WSL 端測試
> server 用 `wsl.exe -d Ubuntu-24.04 -e bash -lc "setsid -f ~/.local/bin/herdr server >/tmp/herdr-server.log 2>&1 </dev/null"`
> 啟動。全程只送 `session.snapshot` 與 `events.subscribe`；**不得對 Windows 端 `herdr server stop`**
> （設計文件 §10.1 的禁令只針對 Windows 端；WSL 端 headless 測試 server 可以停）；對 WSL 端測試
> server 的寫入（建 tab、`pane.report_agent`）要 `HERDR_CLIENT_TEST_ALLOW_WSL_WRITES=1` 明確 opt-in。
> 從 Git Bash 呼叫 `wsl.exe` 傳 POSIX 路徑要前置 `MSYS_NO_PATHCONV=1`。
>
> **跨 crate fixture**：`cockpit-herdr` 讀 1a 的 fixture 用
> `concat!(env!("CARGO_MANIFEST_DIR"), "/../herdr-client/tests/fixtures/<檔名>")`。
>
> **流程**：apply 走 SDD（第一個動作載入 `superpowers:subagent-driven-development`），每個 task 一個
> fresh 實作者、一次 Codex review、review 通過後一個 commit；不平行派工。spec 與 design 的對應：
> `runtime-model`／`runtime-driver`／`state-projection` → 第 1 組；`herdr-runtime-translation`／
> `herdr-runtime-session` → 第 2 組；`cockpit-config`／`cockpit-dashboard` → 第 3 組。

## 1. Workspace 骨架與 `cockpit-core`（spec `runtime-model`、`runtime-driver`、`state-projection`）

- [x] 1.1 建立 `cockpit-core/`、`cockpit-herdr/`、`cockpit/` 三個 crate 骨架（各自 `Cargo.toml`、
  `src/lib.rs`；`cockpit` 另有薄的 `src/main.rs`；`tests/.gitkeep`），根 `Cargo.toml` members 加入
  三者；一次把三個 crate 的依賴集定好（後續 task 不再動 `Cargo.toml`，除非該 task 明說），依
  design Context、D14、D16：共用依賴 `tokio`、`serde`、`serde_json`、`thiserror`、`tracing`、
  `async-trait`（1a 已在 `Cargo.lock` 的版本）三個 crate 依需要都可列；`cockpit-core` 另加
  `chrono`，且不得含 `herdr-client` 或任何 `cockpit-*`；`cockpit-herdr` 另加 `cockpit-core`、
  `herdr-client`，dev 加 `herdr-client`（`test-support`）；`cockpit` 另加 `cockpit-core`、
  `cockpit-herdr`（**不加** `herdr-client`）、`axum`（`ws`）、`toml`、`tracing-subscriber`、
  `anyhow`，dev 加 `tower`、`http-body-util`、`tokio-tungstenite`（版本以 `cargo add` 解析並記進
  報告）。先記基線 `cargo test --workspace` 結果。驗收：`cargo tree -p cockpit-core -e normal | tail -n +2 | grep -c "herdr-client\|cockpit-"`
  為 0；`cargo tree -p cockpit -e normal --depth 1 | grep -c herdr-client` 為 0；
  `cargo build --workspace` 成功。結尾跑全 workspace gate。（執行時裁決：原文「加 X、Y」被 Codex
  解讀為封閉清單，改寫成現在這樣；共用依賴保留，見 SDD ledger Task 1.1。）
- [x] 1.2 `cockpit-core` Runtime 層型別（design D2、D3、D4；設計文件 §6.1）：`RuntimeId`、
  `AgentStatus`（serde 小寫）、`Workspace`／`Tab`／`Pane`／`Agent`／`Focused`、`RuntimeSnapshot`
  （含 `protocol_warning: Option<String>`）、`RuntimeEvent`（§6.1 全部變體加 `WorkspaceRelabeled`、
  `TabRelabeled`、`TabsReplaced { workspace_id, tabs }`、三個 `Option` 的 `FocusChanged`）、
  `ConnectionState`、`RuntimeError { Unavailable { reason, retry_after }, Failed(String) }` 與
  `retry_after()`、`AgentRuntime` trait、`RuntimeEvents`（mpsc receiver ＋ abort-on-drop guard，
  `async fn next()`）。驗收測試（`cockpit-core/tests/types.rs`）：`agent_status_serializes_lowercase`、
  `agent_status_has_exactly_five_values_and_no_completed`、`runtime_error_retry_after_only_for_unavailable`、
  `runtime_events_drop_aborts_reader_task`。結尾跑 `cockpit-core` gate。
- [x] 1.3 `RuntimeStore` 的整份替換與逐筆套用（spec `runtime-model`「整份替換」「逐筆套用」）：
  `replace(runtime_id, snapshot)`、`apply(runtime_id, event) -> Result<(), Drift>`，含連鎖刪除
  （design D5）、`PaneMoved`、`PaneExited`、`AgentDetected` 設定／清除、`AgentStatusChanged`、
  `FocusChanged` 同層唯一、`updated_at` 更新、pane 進入序號（design D7）。驗收測試
  （`cockpit-core/tests/store.rs`）：`replace_drops_objects_missing_from_snapshot`、
  `tab_removed_cascades_panes_and_agents`、`workspace_removed_cascades_everything_below`、
  `pane_moved_replaces_old_id`、`agent_detected_none_clears_pane_agent_and_record`、
  `agent_status_changed_updates_pane_and_agent_record`、`focus_changed_partial_keeps_other_levels`、
  `pane_exited_marks_but_keeps_pane`。結尾跑 `cockpit-core` gate。
- [x] 1.4 Drift 判定與連線狀態（spec `runtime-model`「Drift 判定」「連線狀態紀錄」「型別獨立」）：
  主體不存在、父層不存在、翻譯層 `Drift` 原樣傳出，套用失敗時狀態庫不變；`set_connection`／
  `connection(runtime_id)`，新登記為 `Connecting`；跨 runtime 隔離。驗收測試（`store.rs` 續）：
  `remove_unknown_pane_is_drift_and_store_unchanged`、`upsert_with_unknown_parent_is_drift`、
  `translated_drift_passes_through`、`same_pane_id_in_two_runtimes_is_isolated`、
  `connection_state_defaults_to_connecting_and_keeps_reason`。結尾跑 `cockpit-core` gate。
- [x] 1.5 `ProjectedState` 純函數與最近事件（spec `state-projection`「投影形狀」「最近事件」；設計
  文件 §6.4、design D7、D9、D12）：巢狀 JSON、`connection` 三態序列化（含 `protocol_warning`、
  `retry_in_secs`）、時間欄位以 `chrono` 轉 RFC 3339、workspace／tab 依 `number`、pane 依序號、
  孤兒不投影、每 runtime 50 筆 ring buffer、合併最新在前最多 50、`Noted` 只進紀錄。驗收測試
  （`cockpit-core/tests/projection.rs`）：`nested_projection_matches_design_json_shape`（以
  `serde_json::Value` 比對整份預期 JSON）、`disconnected_connection_carries_reason_and_retry`、
  `timestamps_are_rfc3339`、`ordering_by_number_and_pane_sequence`、`orphan_pane_not_projected`、
  `recent_events_keeps_latest_fifty`、`noted_only_enters_recent_events`。結尾跑 `cockpit-core` gate。
- [x] 1.6 `StoreHandle` 與投影任務（spec `state-projection`「version 只在內容改變時遞增」「合併
  廣播」；design D9）：`Arc<Mutex<RuntimeStore>>` ＋ dirty `Notify` ＋ 50 ms 合併 ＋
  `watch::Sender<Arc<ProjectedState>>`，`generated_at` 不參與比較。驗收測試
  （`cockpit-core/tests/projector.rs`，`tokio::time::pause()`）：`unchanged_store_keeps_version_and_does_not_broadcast`、
  `changed_store_bumps_version_once`、`ten_changes_within_10ms_yield_at_most_two_broadcasts`、
  `new_subscriber_gets_current_state_immediately`。結尾跑 `cockpit-core` gate。
- [x] 1.7 連線驅動器（一）連線順序、丟棄、可停止（spec `runtime-driver`「訂閱先開…」「可停止」；
  design D1）：`driver::run(runtime, store, policy, cancel)` 骨架與 `policy { resnapshot, backoff }`；
  `cockpit-core/tests/common/mod.rs` 的 `FakeRuntime`（呼叫紀錄、可延遲的 snapshot 回應佇列、可控
  事件流、可回 `Unavailable`）；snapshot 前丟棄的事件數以 `tracing::debug!` 輸出。驗收測試
  （`cockpit-core/tests/driver.rs`，`tokio::time::pause()`）：`subscribes_before_snapshot`、
  `events_before_authoritative_snapshot_are_dropped_and_not_recorded`、
  `cancel_releases_stream_and_stops_calls`。結尾跑 `cockpit-core` gate。
- [x] 1.8 連線驅動器（二）Drift 與定期重拿（spec「Drift 立即重拿」「定期重拿」）。驗收測試
  （`driver.rs` 續）：`drift_triggers_one_resnapshot`、`drifts_during_pending_resnapshot_coalesce`、
  `events_during_resnapshot_are_applied_then_overwritten_by_snapshot`、
  `periodic_resnapshot_fires_and_resets_after_drift`、`no_resnapshot_while_disconnected`。結尾跑
  `cockpit-core` gate。
- [x] 1.9 連線驅動器（三）斷線、退避與固定間隔（spec「斷線、退避與固定間隔重試」；design D4）。
  驗收測試（`driver.rs` 續）：`backoff_sequence_1_2_4_8_16_30`、`backoff_resets_after_connected`、
  `unavailable_uses_fixed_interval_without_advancing_backoff`、
  `stream_error_reason_appears_in_disconnected`、`snapshot_failure_is_disconnected`。結尾跑
  `cockpit-core` gate。

## 2. `cockpit-herdr`（spec `herdr-runtime-translation`、`herdr-runtime-session`）

- [x] 2.1 snapshot 翻譯（spec「snapshot 翻譯」；design D15）：`translate::snapshot(&SessionSnapshot, now) -> RuntimeSnapshot`，
  label 空字串 → 無標籤、`exited` false、`focused` 三欄、`layouts` 不翻；測試從 JSON 起跑
  （`serde_json::from_value::<SessionSnapshot>`），未知狀態字串才會經 1a 的 `#[serde(other)]` 變
  `Unknown`。驗收測試（`cockpit-herdr/tests/translate.rs`）：
  `snapshot_p22_fixture_translates_with_same_counts_and_ids`、`snapshot_p20_fixture_translates`、
  `unknown_agent_status_string_maps_to_unknown`。結尾跑 `cockpit-herdr` gate。
- [x] 2.2 事件翻譯（spec「事件翻譯對照」；設計文件 §7.2、design D3、D11）：
  `translate::event(&IncomingEvent) -> Option<RuntimeEvent>`，26 種 `EventKind`、3 種
  `SubscriptionEventKind` 與未知名稱的對照，payload 解析失敗 → `Drift`。驗收測試（`translate.rs`
  續）：`every_event_kind_maps_per_table`（table-driven：對 29 種各合成一筆最小合法 payload，斷言
  結果種類）、`every_line_of_four_event_fixtures_translates_without_drift`、
  `pane_moved_keeps_previous_and_new_ids`、`pane_agent_detected_released_clears_agent`、
  `agent_status_changed_done_maps_to_done_only`、`missing_payload_field_is_drift_with_event_name`、
  `unknown_event_name_yields_none`、`focused_events_are_partial`。結尾跑 `cockpit-herdr` gate。
- [x] 2.3 WSL 探測（spec「WSL 探測」；design D8）：`probe::decode_list(bytes) -> Vec<String>`（NUL →
  UTF-16LE，否則 UTF-8，裁 `\r` 與空行）、`DistroProber` trait、真實現（`wsl.exe --list --running --quiet`，
  Windows `CREATE_NO_WINDOW`，失敗轉「WSL 探測失敗：」）、`FakeProber`。驗收測試
  （`cockpit-herdr/tests/probe.rs`）：`decode_utf16le_list`、`decode_utf8_list`、
  `distro_missing_is_unavailable_with_fixed_interval`、`command_failure_is_unavailable_with_stderr`，
  以及 `#[ignore]` 的 `real_wsl_probe_lists_running_distro`（本機有 WSL 時手動跑）。結尾跑
  `cockpit-herdr` gate。
- [x] 2.4 `FakeHerdr` 加法擴充（design D14；只動 `herdr-client/src/testing`，不改 1a spec）：
  `with_method_responses(method, Vec<MethodResponse>)` 依呼叫序回應、最後一筆重複。驗收測試
  （`herdr-client/tests/fake_herdr.rs` 續）：`method_responses_are_served_in_order_then_repeat_last`。
  結尾跑 `herdr-client` gate。
- [x] 2.5 `HerdrRuntime` 的建立事件流與合併流（spec「建立事件流的順序」「合併事件流與結束」；design
  D1、D2、D10）：`HerdrRuntime::new(id, connector, wsl: Option<WslProbe { distro, prober, retry_after }>)`（執行時裁決：探測參數包成 struct，固定間隔由 runtime 持有並正規化所有探測錯誤，見 SDD ledger Task 2.5）；
  `subscribe()` 先重置 S 管理器，再依序探測 → seed → L → S（seed 無 pane 不開 S）→
  `RuntimeEvents`；L／S 各一個 reader task 翻譯後送 mpsc，任一結束送 `Err` 後關另一條；失敗時關
  已開連線。驗收測試（`cockpit-herdr/tests/session.rs`，L／S 用 `SubscribeMatcher::LifecycleOnly`／
  `PerPane` 分流）：`connection_order_is_snapshot_then_l_then_s`、`seed_without_panes_opens_only_l`、
  `probe_failure_on_seed_pane_fails_subscribe_and_closes_l`、`events_from_l_and_s_arrive_in_one_stream`、
  `l_close_yields_error_naming_l_then_ends`、`dropping_stream_closes_both_connections`、
  `prober_unavailable_short_circuits_before_any_connection`。結尾跑 `cockpit-herdr` gate。
- [x] 2.6 ReopenStatus、`snapshot()` 與版本警告（spec「pane 集合改變時重開狀態訂閱」「取得
  snapshot 與版本警告」「只用兩個 method」；design D10、D12）：S 管理器 200 ms 去抖動、新 S 收到
  `subscription_started` 才關舊 S、新清單為空只關舊 S、失敗送 `Err`；`snapshot()` 翻譯並把 pane
  集合交給 S 管理器，`protocol` 不在 20..=22 填警告。驗收測試（`cockpit-herdr/tests/reopen.rs`）：
  `pane_created_reopens_s_with_new_list_and_closes_old_after_started`、
  `three_pane_events_within_100ms_reopen_once`、`snapshot_with_different_pane_set_reopens`、
  `empty_new_list_only_closes_old_s`、`reopen_failure_ends_stream_with_error`、
  `protocol_23_sets_warning_and_still_succeeds`、`protocol_22_has_no_warning`、
  `fake_only_receives_snapshot_and_subscribe_methods`。結尾跑 `cockpit-herdr` gate。
- [x] 2.7 工廠與迴圈整合（design D16；spec `runtime-driver` × `herdr-runtime-session`）：
  `cockpit_herdr::build(HerdrEndpoint, options) -> (endpoint 描述, Arc<dyn AgentRuntime>)`
  （`Socket`／`Wsl`／`Command`／`Default` 四種端點對應的 connector 與探測器）；驅動器 ＋
  `HerdrRuntime` ＋ `FakeHerdr` 跑完整週期。驗收測試（`cockpit-herdr/tests/factory.rs`：
  `socket_endpoint_describes_named_pipe_or_unix_socket`、`wsl_endpoint_describes_child_stdio_and_has_prober`、
  `command_endpoint_has_no_prober`；`cockpit-herdr/tests/loop_integration.rs`：
  `full_cycle_reaches_connected_with_store_equal_to_snapshot`、
  `l_close_disconnects_then_reconnects_with_fresh_seed`、
  `drift_event_triggers_resnapshot_with_second_response`（用 2.4 的回應佇列）、
  `s_probe_failure_backs_off_and_retries`）。結尾跑 `cockpit-herdr` gate。

## 3. `cockpit`（spec `cockpit-config`、`cockpit-dashboard`）

- [x] 3.1 設定檔（spec `cockpit-config` 全部；design D16）：`config::load(args, cwd, lookup_env) -> Result<Config>`
  （工作目錄與環境變數注入，`main` 包真值），來源順序、零設定、預設值、`kind` 只接受 `herdr`、
  端點三選一、未知欄位錯誤、驗證（重複 id、空 id、非 loopback、秒數為 0），錯誤訊息含 id 與原因。
  驗收測試（`cockpit/tests/config.rs`）：`zero_config_yields_local_runtime_with_default_socket`、
  `missing_config_path_fails_with_path`、`design_doc_example_parses`、`unknown_kind_fails_naming_id`、
  `unknown_field_fails`、`two_endpoints_fail_naming_id`、`duplicate_id_fails`、
  `non_loopback_listen_fails`、`zero_interval_fails`。結尾跑 `cockpit` gate。
- [x] 3.2 runtime 組裝：`runtimes::build(&Config) -> Vec<RuntimeEntry { id, kind, endpoint, runtime }>`，
  把每筆設定轉成 `HerdrEndpoint` 交給 2.7 的工廠（`cockpit` 不直接引用 `herdr-client`），順序依
  設定檔。驗收測試（`cockpit/tests/runtimes.rs`）：`entries_follow_config_order_and_ids`、
  `omitted_endpoint_uses_default`；`cargo tree -p cockpit -e normal --depth 1 | grep -c herdr-client`
  仍為 0。結尾跑 `cockpit` gate。
- [x] 3.3 HTTP 路由與內嵌資源（spec `cockpit-dashboard`「路由與內嵌資源」；design D13）：axum
  `Router`：`/`、`/app/{channel.js,render.js,style.css}`、`/manifest.webmanifest`、
  `/icons/icon-{192,512}.png`、`/api/state`、404；`cockpit/assets/` 先放可用的最小版檔案
  （3.5、3.7 換成正式內容）；只綁設定的 loopback 位址。驗收測試（`cockpit/tests/http.rs`，
  `tower::ServiceExt::oneshot`）：`routes_return_200_with_expected_content_types`、
  `api_state_equals_current_projection_version`、`unknown_path_is_404`。結尾跑 `cockpit` gate。
- [x] 3.4 WebSocket 推送（spec「WebSocket 推送整張圖」）：`/ws` 連上先送 `watch` 現況，之後每次
  變更送整份文字訊息，忽略客戶端訊息，客戶端斷線不影響其他人。驗收測試（`cockpit/tests/ws.rs`，
  綁 `127.0.0.1:0`、`tokio-tungstenite` 客戶端）：`first_message_equals_api_state`、
  `store_change_pushes_next_version_full_json`、`two_clients_both_receive_same_version`、
  `client_messages_are_ignored`。結尾跑 `cockpit` gate。
- [x] 3.5 畫面檔案與預覽工具（spec「畫面整頁重畫」；設計文件 §8.3、design D13、D14）：
  `index.html`、`app/channel.js`（連 `/ws`、`onState`、1/2/4/8 秒退避重連、頂列通道狀態）、
  `app/render.js`（純函數 state → DOM，狀態色塊、未知字串暗灰、exited 刪除線、最近事件）、
  `app/style.css`（深色）；新建 fixture `cockpit/tests/fixtures/projected-state.json`（兩個
  runtime：`win` connected、`wsl` disconnected 附原因，含一個 `whatever` 狀態的 pane）；
  `cockpit/examples/ui_preview.rs` 以該 fixture 起服務並每 2 秒改一個 pane 狀態。驗收：
  `cargo run -p cockpit --example ui_preview` 可啟動且 `/api/state` 回 fixture 內容；
  `cockpit/tests/http.rs` 加 `embedded_assets_are_the_final_files`（`/app/render.js` 內容含
  `onState`）。結尾跑 `cockpit` gate。
- [x] 3.6 畫面驗收（spec「畫面整頁重畫」三個 Scenario）：跑 3.5 的 `ui_preview`，以瀏覽器工具開
  `http://127.0.0.1:7770/`，截圖存 `docs/research/<執行日期>/ui-preview.png`，DOM 檢查兩張卡、
  `wsl` 卡有原因、色塊 class 對應狀態、`whatever` 狀態列為暗灰、頂列有 version、console 無錯誤；
  停掉 example 再啟動，頂列通道狀態由斷線回到已連線且畫面重畫。驗收：截圖與 DOM 檢查結果寫進
  `docs/research/<執行日期>/change-1b-acceptance.md`「畫面」一節；有問題就改 3.5 的檔案。結尾跑
  `cockpit` gate 與 markdownlint。
- [x] 3.7 PWA（spec「PWA 可安裝」；design D13）：`manifest.webmanifest`（`name`、`short_name`、
  `start_url`、`display`、`icons`、深色 `background_color`／`theme_color`）、
  `cockpit/assets/gen-icons.py`（stdlib，`uv run --no-project python cockpit/assets/gen-icons.py`）
  產生 `icon-192.png`、`icon-512.png` 進 repo、`index.html` 加 `<link rel="manifest">`。驗收測試
  （`cockpit/tests/pwa.rs`）：`manifest_has_required_fields_and_icon_urls`、
  `icons_are_png_with_declared_dimensions`（檢查 `89 50 4E 47` 與 IHDR 寬高）。Chrome 選單安裝
  留到 4.1 一併做。結尾跑 `cockpit` gate。
- [x] 3.8 `main` 與真機測試骨架（design D16）：`--config` 解析、`tracing-subscriber` env filter、
  建 `StoreHandle` 與投影任務、每筆 runtime spawn 驅動器、起 axum、Ctrl-C 取消驅動器後結束；
  port 被占用時印位址與原因後非零結束；`cockpit/tests/real_attach.rs` 全部 `#[ignore]`
  （`real_zero_config_connects_and_pane_count_matches_herdr_snapshot`：零設定啟動、等 `Connected`、
  比對 `/api/state` 的 pane 數與 `herdr api snapshot` 的輸出——旗標與格式執行時先用 `--help`
  確認，不假設 `--json`）；`cockpit/README.md` 寫啟動、設定範例（`cockpit.example.toml` 進 repo、
  路徑用 `<user>` 佔位，`cockpit.toml` 進 `.gitignore`）、真機測試方式。驗收：
  `cargo run -p cockpit -- --config missing.toml` 非零結束且訊息含路徑；`cargo build --release -p cockpit`
  後把 `target/release/cockpit.exe` 複製到暫存目錄執行，`/` 與 `/icons/icon-192.png` 皆 200
  （spec「單一執行檔」）。結尾跑 `cockpit` gate 與 markdownlint。

## 4. 真機驗收與收尾（設計文件 §10.2；`docs/handover.md` §3）

- [x] 4.1 Scenario A Attach：啟動 WSL 端測試 server；寫本機 `cockpit.toml`（`win` 省略端點、`wsl`
  指 `Ubuntu-24.04` 與 WSL 端 socket 路徑）；**在 HERDR pane 內**執行 `cargo run -p cockpit`
  （spike 4 目視複驗：不出現新視窗，由使用者目視；自動化補充
  `Get-Process | Where MainWindowHandle -ne 0` 前後比對）；以瀏覽器工具開
  `http://127.0.0.1:7770/`。驗收：兩張卡皆 `connected`；兩側 pane／workspace 清單與各自
  `herdr api snapshot` 一致（`real_attach` 測試通過）；Chrome 選單可安裝；截圖與比對輸出（去識別
  化）存 `docs/research/<執行日期>/change-1b-acceptance.md`。
- [x] 4.2 Scenario B Live state：Windows 端在某 agent pane 由使用者下一句指令，觀察該 pane 一秒內
  變 `working`、最近事件出現 `pane.agent_status_changed`；WSL 端以 `HERDR_CLIENT_TEST_ALLOW_WSL_WRITES=1`
  用 1a 真機測試的寫入鷹架（`pane.report_agent`）製造 working→blocked→idle；新建一個 pane 後其
  狀態變化同樣即時出現（驗 ReopenStatus）。順手記錄：關 tab 時 Windows 0.9.0 的事件序列（design
  D5）、狀態變化時是否伴隨 `workspace_updated`（design D6）、`report_agent state=idle` 呈現
  `idle` 還是 `done`。驗收：以上觀察與截圖寫進 acceptance 紀錄；全程無任何 prompt 送往 agent。
- [x] 4.3 Scenario F Reconnect（只在 WSL 端；`AGENTS.md` 的 `herdr server stop` 禁令只針對
  Windows 端，本 task 順手把那條補上「Windows 端」限定）：對 WSL 端測試 server `herdr server stop`
  再以 `setsid -f` 重啟。驗收：`wsl` 卡片先 `disconnected`，首次原因反映對端關閉（L／S 結束或
HERDR 回 `server is shutting down`），重連
  期間的原因含 `herdr-client` 的 `ServerNotRunning` 描述並顯示重試秒數；重啟後回到 `connected`，
  內容與重新取得的 snapshot 一致；Windows 端全程不受影響；實際原因字串與退避序列數字記入
  acceptance 紀錄。
- [x] 4.4 L 訂閱補推舊事件查證（`docs/handover.md` §3 待查證項）：在 Windows 端有近期 tab
  建立／關閉活動後（使用者日常操作即可）跑 `cargo run -p herdr-client --example capture_events -- --seconds 20`，
  檢查 L 的前幾行是否含 snapshot 中不存在的 tab／pane 事件；另看 cockpit 以 `RUST_LOG=debug`
  啟動時的「snapshot 前丟棄事件數」（1.7 的 debug 日誌）。驗收：結論（有／無／無法判定）與證據
  寫進 acceptance 紀錄並回寫設計文件 §2.3 加註；若有，確認畫面沒有殘留幽靈 pane（否則走
  `/opsx:update`）。
- [x] 4.5 文件回寫：設計文件 §4.1、§4.2、§7.1（design D1 驅動器在 `cockpit-core`；D8 NUL 判別
  編碼）、§6.1、§7.2（D3 三個變體；§7.2 補「生命週期版 `pane_agent_status_changed` → 忽略」一
  列）、§6.2（D2 事件流簽章）、§6.4（D12 `protocol_warning`）、§2.3（4.4 結論）；
  `docs/handover.md` §3「一律 `from_utf16_lossy`」改為 NUL 判別；`CONTEXT.md` Runtime 層加
  「Driver（驅動器）」一詞；`AGENTS.md` 硬性約束改為「`cockpit-core` 不得依賴 `herdr-client` 或
  任何 `cockpit-*`；`cockpit` 不直接依賴 `herdr-client`」；`cockpit/README.md` 補驗收結果連結。
  驗收：`markdownlint-cli2 "**/*.md"` 0 issue 且 `Linting` 檔數不為 0。
- [x] 4.6 收尾 gate：全 workspace gate 全綠、`openspec validate attach-herdr-runtimes` 無 ERROR、
  `tests/fixtures/`、`cockpit.example.toml` 與 acceptance 紀錄無未去識別化的本機路徑或使用者名稱
  （`grep -rn "<user>\|<user>@\|/home/<user>\b" docs/research/<執行日期>/ cockpit/tests/fixtures/ cockpit.example.toml`
  為空）；重寫 `docs/handover.md`（下一步 change 2 propose）。驗收：貼上各 gate 的原始輸出。
