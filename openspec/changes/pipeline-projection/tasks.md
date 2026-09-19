# tasks：pipeline-projection

## 1. 詞彙先行

- [x] 1.1 更新 `CONTEXT.md` Domain 層：Pipeline（MVP 為線性 stages）、Dependency 改為「Task 之間的依賴（`depends_on`）」、
  Task（隨進度操作在 Stage 間推進、帶人工標記）、RuntimeBinding（掛在 Workstream、task 共用；pane id 只是解析結果）、
  新增 Mark、Override 兩個名詞；Workstream 的「是否多對多由 change 2 決定」改為「MVP 一對一」。驗收：
  `markdownlint-cli2 "**/*.md"` 0 error 且 `Linting: N files` 不為 0；`grep -n "Dependency" CONTEXT.md` 顯示新定義（design D8）。

## 2. cockpit-core：Domain 純函數與投影

- [x] 2.1 新增 `cockpit-core/src/domain/` 型別：`ProjectDef`、`WorkstreamDef`、`BindingSpec`、`TaskDef`、`Mark`、
  `TaskProgress`、`Override`、`DomainState`（含 warnings）、`BindingResolution`（五種）、`StageStatus`（六值，小寫序列化）；
  不得 `use` 任何 HERDR 型別。驗收：`cargo test -p cockpit-core --test domain_types`（序列化字串、`DomainState`
  初始進度＝起始 stage＋`none`）；`cargo fmt --check && cargo clippy -p cockpit-core --all-targets -- -D warnings && cargo test -p cockpit-core`。
- [x] 2.2 實作進度操作 `apply_op`（advance／complete／fail／clear 與拒絕原因；clear 在 `none` 時回傳相同狀態）。
  驗收：`cargo test -p cockpit-core --test domain_progress` 涵蓋 `pipeline-domain`「進度操作」全部情境，另含
  `agent_status_never_changes_progress`（對同一份進度套用各種 runtime 狀態變化後進度不變）；同 2.1 的 crate gate。
- [x] 2.3 實作 `resolve_binding`：候選篩選（workspace label、pane_label、agent、cwd 片段含 `\` 正規化、排除 exited）、
  runtime 未連線 → `runtime_disconnected`、覆蓋優先與失效（回傳自動解析結果並列入 `stale_overrides`）。驗收：
  `cargo test -p cockpit-core --test domain_binding` 涵蓋 `runtime-binding` 全部情境（含 pane 換 id、`backend-old` 不匹配、
  斷線期間保留覆蓋）；另實作 `validate_override`（runtime 未登記、未連線、pane 不存在、pane 已 exited 四種拒絕），
  `domain_binding` 含對應四個測試；同 crate gate。
- [x] 2.4 實作 `derive_status`（標記 → 依賴 Pending → 綁定 agent 狀態 → Ready）。驗收：`cargo test -p cockpit-core --test domain_status`
  含 `scenario_c_running_in_implement`、`scenario_d_parallel_workstreams`、`done_is_not_completed`、`disconnected_is_ready`、
  `same_workstream_multiple_tasks`；同 crate gate。
- [x] 2.5 投影擴充：`ProjectedState` 加 `projects`（`ProjectedProject`／`ProjectedWorkstream`／`ProjectedTask`，形狀依
  `state-projection`「Project 投影」）、`project(store, domain, version, now)`；`StoreHandle` 內含 domain、新增 domain 寫入方法
  （`notify_one`）、投影任務把非空 `stale_overrides` 送到可注入的 `mpsc::Sender`（design D2、D3）。驗收：
  `cargo test -p cockpit-core`（新增：無 project 時 `projects == []`、Scenario C 的 binding JSON 逐欄位、domain 變動 version+1、
  相同 domain 再寫不遞增、stale override 送達 channel）；`cockpit/tests/fixture.rs` 與 `cockpit/tests/fixtures/projected-state.json`
  補 `projects` 後 `cargo test -p cockpit --test fixture` 通過；兩個 crate 的 fmt、clippy、test。

## 3. cockpit：設定、狀態檔、寫入服務

- [x] 3.1 `config.rs` 解析 `[[project]]`（含 workstream／binding／task）與 `[state] path`，全部 `deny_unknown_fields`，驗證錯誤
  以 `project.<pid>.…` 手寫字串識別（id 正規式、重複、stages、workstream／stage 參照、`depends_on` 不存在／自己／成環、
  binding runtime 未設定、空字串）；狀態檔路徑依設定檔目錄解析、零設定無路徑。`cockpit.example.toml` 加一個示範 project。
  驗收：`cargo test -p cockpit --test config` 涵蓋 `pipeline-config` 與 `cockpit-config` delta 全部情境（含範例檔可解析、
  `[pipeline]` 未知區段）；`cargo fmt --check && cargo clippy -p cockpit --all-targets -- -D warnings && cargo test -p cockpit`。
- [x] 3.2 新增 `cockpit/src/progress.rs` 狀態檔載入：不存在 → 初始進度且不建檔；損毀或 `version != 1` → 錯誤含路徑；
  未知 project／task／workstream／覆蓋 runtime → 忽略並 warn；stage 不在 stages → 退回起始 stage 並加 warning。驗收：
  `cargo test -p cockpit --test progress_file`（暫存目錄沿用自製 `TempDir`，不加 `tempfile`）涵蓋 `pipeline-progress`
  「狀態檔載入與容錯」全部情境；同 3.1 的 crate gate。
- [x] 3.3 寫入服務：單一 async `Mutex` 內「core 計算 → 拒絕回錯 → `spawn_blocking` 寫 `.tmp` 再 `rename` → 成功才套用到
  `StoreHandle`」；接收 stale override 清單並以同一路徑刪除（落檔失敗記 error、記憶體照刪）。驗收：`cargo test -p cockpit --test progress_service`
  含 `concurrent_writes_not_lost`（兩個 task 並發各 50 次交錯操作，最後檔案與記憶體一致）、`write_failure_keeps_memory`
  （目標路徑為既有目錄使 rename 失敗）、`rename_replaces_existing_file`（Windows 覆蓋既有檔）、`stale_override_removed_and_persisted`；同 crate gate。
- [x] 3.4 `app.rs` 組裝：啟動時有 project 才載入狀態檔（失敗即啟動失敗、訊息含路徑）、建立寫入服務、接 stale channel、
  停止時一併收掉寫入服務。驗收：`cargo test -p cockpit --test app` 新增 `corrupt_state_file_fails_startup`、
  `no_projects_creates_no_state_file`、`restart_keeps_progress_and_override`；同 crate gate。

## 4. cockpit：HTTP 寫入端點

- [x] 4.1 `http.rs` 加 `POST /api/projects/{project}/tasks/{task}/{op}`、`PUT`／`DELETE /api/projects/{project}/workstreams/{workstream}/override`
  與 `GET /app/actions.js`；狀態碼 204／400／404／409／500，錯誤本體 `{"error": ...}`；`AppState` 加寫入服務與實際監聽埠。
  驗收：`cargo test -p cockpit --test pipeline_api` 涵蓋 `pipeline-progress`「進度寫入端點」「綁定覆蓋端點」全部情境（含覆蓋端點的
  `override_unknown_workstream_404`、`override_rejected_409`、`override_write_failure_500`）與
  `cockpit-dashboard`「寫入端點不接受 GET」；`cargo test -p cockpit --test http` 的 content-type 情境加 `actions.js`；同 crate gate。
- [x] 4.2 寫入路由的來源檢查 middleware（Host 為 loopback 三種寫法＋實際埠；有 Origin 時須為 `http://<Host>`；否則 403）。
  驗收：`cargo test -p cockpit --test pipeline_api` 新增 `cross_site_origin_rejected`、`dns_rebinding_host_rejected`、
  `same_origin_and_cli_accepted`、`port_zero_uses_actual_port`；同 crate gate。
- [x] 4.3 Scenario C／D 的 HTTP 層整合測試：以 `StoreHandle` 灌入 runtime（connected、pane 狀態變化）、真 `router`、暫存狀態檔，
  只經 `/api/state` 與寫入端點觀察。驗收：`cargo test -p cockpit --test pipeline_api scenario_` 跑出
  `scenario_c_pipeline_projection`（task 在 `Implement`、pane working 後 `status` 為 `running`）與
  `scenario_d_parallel_collaboration`（三條 workstream 三個 stage 同時 `running`，再推進其中一個後 `stage` 改變、其餘不變）皆通過；同 crate gate。

## 5. 畫面

- [x] 5.1 `channel.js`：退避只在收到第一則訊息時歸零；`JSON.parse` 失敗略過並 `console.warn`。驗收：擴充
  `docs/research/2026-09-15/reconnect-check.js` 的做法寫新腳本（放本 change 的研究日期目錄），以一個「接受後立即關閉」的
  測試 WebSocket server 觀察重連間隔 1、2、4、8、8 秒，以及送壞訊息後下一份正常重畫；輸出與結論寫入驗收文件；
  `cargo test -p cockpit`（資源內嵌測試）通過。
- [x] 5.2 `render.js` 改為 `renderState(state, ui)`，runtime 卡之前畫每個 Project 的 Factory Floor（stages 欄、workstream 列、
  綁定摘要五種文字、task 節點與六種狀態色、`running` 動態強調、warnings、未知 status 暗灰）；`style.css` 對應樣式
  （`completed` 紫、不用 `done` 藍）；`examples/ui_preview.rs` 的 fixture 加兩個 project（其一為 Scenario D 配置）。
  驗收：`cargo run -p cockpit --example ui_preview` 下以 headless Chrome `--dump-dom` 腳本斷言 Scenario D 三個節點位於正確
  （列, 欄）、兩個 Project 上下順序在 runtime 卡之前；`--screenshot` 截圖存研究目錄；`cargo test -p cockpit` 通過。
- [x] 5.3 新增 `assets/actions.js`：UI 狀態（改綁目標、錯誤訊息）、`fetch` 寫入、根節點事件委派以 `pointerdown` 觸發
  （鍵盤另收 `detail === 0` 的 `click`）、節點按鈕顯示規則、改綁模式（提示＋取消＋connected runtime 未 exited pane 列的
  「綁定到這裡」）、錯誤顯示跨重畫保留；`index.html` 引用。`ui_preview` 加僅記錄請求並回 204 的寫入路由與每 100 ms
  推送一份的模式。驗收：headless Chrome（DevTools protocol）腳本驗 `cockpit-dashboard`「畫面操作」四個情境（推進送出正確
  路徑、改綁模式跨兩次重畫後送出 `PUT`、100 ms 推送下連點 10 次收到 10 個 `POST`、409 錯誤訊息跨重畫保留）；
  `cargo test -p cockpit` 通過。

## 6. change 1b deferred minor

- [x] 6.1 驅動器 Drift 追加重拿：重拿進行中有事件成功套用 → 替換後再拿，每次 Drift 觸發最多追加 2 次（design D11）。
  驗收：`cargo test -p cockpit-core --test driver` 涵蓋 `runtime-driver` delta「Drift 立即重拿」四個情境（含
  `followup_resnapshot_capped_at_two`）；既有 driver 測試全數仍通過；`cargo fmt --check && cargo clippy -p cockpit-core --all-targets -- -D warnings && cargo test -p cockpit-core`。
- [x] 6.2 固定重試間隔下限 1 秒，並補「取得 snapshot 回 `Unavailable`」來源的測試。驗收：`cargo test -p cockpit-core --test driver`
  新增 `fixed_retry_interval_floor_one_second`、`snapshot_unavailable_uses_fixed_interval`；同 6.1 的 crate gate。
- [x] 6.3 `cockpit-herdr` 的 `Shutdown.aborts` 在登記時修剪已結束的 handle。驗收：`cargo test -p cockpit-herdr --test reopen`
  新增 `aborts_registry_does_not_grow_unbounded`（重開 20 次後登記數 ≤ 常數上限）；`cargo fmt --check && cargo clippy -p cockpit-herdr --all-targets -- -D warnings && cargo test -p cockpit-herdr`。
- [x] 6.4 `shutdown_components` 改為所有驅動器共用總期限、abort 後 `await` 上限 1 秒（逾時 warn 放手），並補
  `run_with_shutdown` 收不到 shutdown 結果的 bail 分支測試。驗收：`cargo test -p cockpit --test app` 新增
  `shutdown_deadline_is_shared_across_drivers`（3 個卡住的驅動器總耗時 ≈ 一個期限而非三倍，paused time）、
  `abort_await_is_bounded`、`run_with_shutdown_bail_branch`；同 3.1 的 crate gate。
- [x] 6.5 測試缺口逐條處理並記錄：F4 session guard 補直接測試；reopen 1.67 s、loop_integration 2.5 s、重疊 350 ms 三個依真實
  時間的測試改用 paused time，改不了的寫 `Ruling:`；`Path::exists()` 權限情境與 tokio-tungstenite 兩版在 SDD ledger 寫
  `Ruling:`（附理由）。驗收：`cargo test --workspace` 全過；ledger 中五項各有「已補測（測試名）」或 `Ruling:`。
- [x] 6.6 驅動器連線後沉降重拿：進入 `Connected` 後事件流靜默 1 秒再取得 snapshot、最晚第 5 秒、每次連線一次、與進行中
  重拿合併、失敗視同斷線（design D12，apply 期間由 7.2 發現後加入）。驗收：`cargo test -p cockpit-core --test driver`
  涵蓋 `runtime-driver`「連線後沉降重拿」四個情境（`settle_resnapshot_after_quiet_second`、`settle_resnapshot_overrides_late_replay`、
  `settle_resnapshot_capped_at_five_seconds`、`settle_resnapshot_again_after_reconnect`）；既有 driver 測試全數仍通過；同 6.1 的 crate gate。

## 7. 驗收與文件回寫

- [x] 7.1 全 workspace gate：`cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test --workspace && markdownlint-cli2 "**/*.md"`
  （`Linting: N files` 不為 0）與 `openspec validate --all`（0 failed）。驗收：輸出貼進驗收文件。
- [x] 7.2 真機 Scenario C／D（WSL 端）：先重啟 WSL 測試 server，寫 `pipeline-check.py`（沿用 `acceptance_common.py`），用含
  一個 project、三條 workstream（以 pane label 或 cwd 綁 WSL 端三個 pane）的暫存設定啟動 release 版 cockpit，以 JSON-RPC
  `pane.report_agent` 讓 pane 變 working，輪詢 `/api/state` 直到對應 task `running` 並記錄秒數；再以 HTTP 推進／標記、
  重啟 cockpit 後驗進度保留；關 pane 驗覆蓋失效。6.6 完成後以嚴格時限重跑：啟動與重啟後 6 秒內（沉降上限 5 秒＋1 秒量測餘裕）對應 task 為 `running` 且之後不倒退（不得
  依賴 30 秒定期重拿收斂），並觀察建立／關閉 pane 後狀態是否倒退。驗收：`HERDR_CLIENT_TEST_ALLOW_WSL_WRITES=1 uv run --no-project python docs/research/<日期>/pipeline-check.py` PASS，結果寫入驗收文件。
- [x] 7.3 使用者手動驗收（使用者親自做）：Windows 端 `cockpit.toml` 加一個綁定真實 agent pane 的 project，對 agent 下一句指令，
  Factory Floor 該 task 一秒內變 `running`；在畫面按推進、Completed、清除標記、改綁與取消改綁，目視結果符合 spec。驗收：
  使用者回報結果寫入驗收文件。
- [x] 7.4 文件回寫：設計文件 §8.1（寫入路由）、§8.2（`[[project]]`／`[state]` 範例）、§8.3（Factory Floor 與互動）、§10.2
  （加 Scenario C、D 列）、§2.3（WSL 0.8.2 訂閱重播整段事件歷史，取代「補推少量」描述）；`cockpit/README.md`（pipeline 設定、寫入 API、單實例與回滾注意）；`.gitignore` 加
  `cockpit.state.json`；`docs/handover.md` 依範本重寫（含 `/ws` 跨來源讀取風險）。驗收：markdownlint 0 error；
  `git check-ignore cockpit.state.json` 有輸出。
- [x] 7.5 去識別化：對本 change 的研究目錄、`cockpit/tests/fixtures/`、`cockpit.example.toml` grep 使用者名稱、email、家目錄。
  驗收：grep 輸出為空。
