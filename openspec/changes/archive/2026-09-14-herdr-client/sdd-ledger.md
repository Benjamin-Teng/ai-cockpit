# SDD ledger — plan: openspec/changes/herdr-client/tasks.md

Spec: openspec/changes/herdr-client/specs/{herdr-transport,herdr-request,herdr-event-subscription,herdr-observer-types}/spec.md
Design: openspec/changes/herdr-client/design.md（D1–D12）
Branch: feat/herdr-client（base main a39354b；使用者規則：commit 要授權才做，目前全部未 commit）
SDD 採用時點：2026-09-13，task 1.1–1.6 完成、2.x／3.x／Codex 修正三個 agent 在跑之後才切換。

## Preflight rulings（切換前已做的裁決，補記）

- Ruling: spike 3 的狀態變化類測試改在 WSL 端以底層 `pane.report_agent` 製造事件、Windows 端只做唯讀檢查 — 因 Windows 端是使用者環境不可寫，WSL 端 headless server 可自動化 — 代價：p22 事件 fixture 延到 task 5.1 才擷取。
- Ruling: design D12，`events.subscribe` 探測失敗的 error 回應 `id` 為 `<request id>:sub:<n>:probe` 視為相符並回 Remote — spike 3 真機實測 — 代價：若 HERDR 改格式，會誤判成 Protocol；有 contract／真機測試守。
- Ruling: 第 2 組與第 3 組平行派工（違反 SDD「不可平行」） — 檔案完全分離（src/connector vs src/types），切換前已派出 — 代價：Codex 的 working-tree review 會看到混合 diff；以每組各跑一次 Codex review 補救，第 4 組起不再平行。
- Ruling: Codex 對 spike 程式碼的兩個 findings（真機測試寫入 WSL、spike4 `--help`）以「寫入需 `HERDR_CLIENT_TEST_ALLOW_WSL_WRITES=1` opt-in ＋ guard 先於建立」處理，不是移除寫入 — 保留 spike 的自動事件製造能力 — 代價：opt-in 後仍會動 WSL 端測試 server 的 tab。
- Ruling: codex-result-handling skill 要求 review 後先問使用者才可改；依 CLAUDE.md 鐵則 9 以使用者規則為準（findings 實測重現後即依 receiving-code-review 處理） — 代價：使用者可能想先看 findings 再決定。
- Ruling: 用 feature 分支不用 worktree — 使用者 git-branch-workflow 指南以分支為標準 — 代價：無工作區隔離。
- Ruling: `openspec validate --strict` 的 SHALL/MUST 警告不視為失敗（中文「系統必須」） — task 5.2 改用非 strict — 代價：無。
- Ruling: 為 spike 2–4 以 `setsid -f ~/.local/bin/herdr server` 在 WSL 端啟動 HERDR 0.8.2 headless server（目前仍在跑，帶起使用者存檔的 workspace wD） — 測試需要 — 代價：使用者的 WSL 環境多一個常駐程序；收尾時要問使用者是否 `herdr server stop`（WSL 端）。
- Ruling: spike 5(a)「虛擬機停止時探測不喚醒」本次不重驗，沿用 local-checks.txt — 需 `wsl.exe --shutdown`，屬破壞性，交使用者決定 — 代價：無新證據。
- Ruling: 誤觸的 Codex adversarial review（`--help` 被當 focus）當作第 1 組的 task review 使用 — 內容正是 spike 程式碼 — 代價：無。
- 待使用者決定：是否授權每個 task 完成、review 通過後各做一個 commit（SDD 的 review package 靠 BASE..HEAD）。未授權前 review 一律用 working-tree diff。

## Preflight conflict scan（剩餘 task 的介面對照）

| 產出 task → 消費 task | 產出什麼 | 消費什麼 | 發現 |
|---|---|---|---|
| 2.1 → 4.2／4.3 | `NdjsonStream`（send_line／recv_line）、`Connector`、`ConnectError::from_io` | Client 以 `Arc<dyn Connector>` 開連線、讀第一行 | 一致；4.2 需把第一次讀取的 NotFound／ConnectionRefused 對應成 `Connect(ServerNotRunning)`（D3） |
| 2.2 → 4.4 | `ChildStdioConnector`、`[[bin]] herdr-client-test-child`（echo／exit／sleep 模式） | 4.4 需要一個把 stdio 接到假 HERDR 原生 socket 的子程序 | **待定**：4.4 的 brief 要指定在 test child 加 `relay <socket-or-pipe>` 模式，或由假 HERDR 直接提供 stdio 服務；4.1 定案時決定 |
| 3.1 wire.rs → 4.2／4.3 | `RequestEnvelope`、`ResponseEnvelope`、`ErrorBody`、`SessionSnapshotResult`、`SubscriptionStarted`、`EventEnvelope` | Client 序列化 request、解析回應與事件 | 一致；4.2 的 `result` 形狀不符 → Protocol 依 `#[serde(tag="type")]` 解析失敗實現 |
| 3.2 → 4.3 | `EventKind`／`SubscriptionEventKind`（無 `#[serde(other)]`，`FromStr` 對未知回 Err） | `IncomingEvent::Unknown { event, data }` 承接未知名稱（D4） | 一致 |
| 3.3 → 4.x | contract test 的 validator helper（tests/contract.rs 內） | 4.x 若要驗假 HERDR 回應也符合 schema，需共用 helper | 建議 4.1 把 helper 搬到 `tests/common/`（或複製），不改 3.3 產物 |
| 4.1 → 4.2／4.3／4.4 | `herdr_client::testing::FakeHerdr`（feature test-support、self dev-dependency） | 所有 client 測試 | 4.1 要先驗 self dev-dependency 開 feature 在 `cargo test` 可行（D7 風險） |
| 1.x spike 測試 → 5.1 | `tests/real_herdr*.rs`（raw 連線寫法） | 5.1 改寫成用 `Client`，並擷取 p22 事件 fixture | Codex 修正 agent 正在改這三檔的 opt-in 與 guard；5.1 要在其之後 |
| tasks 5.2 | 全 workspace gate、Codex review、handover | — | 與 SDD 的 final whole-branch review 合併為一次 Codex review |

自洽檢查：每個 task 的測試名稱都在 task 文字內指定，與 spec 情境一一對應，未發現自相矛盾。

## Task 進度

- Task 1.1: complete (uncommitted; spike 1 兩測試通過，fixture snapshot-p22.json; review: 誤觸 Codex review 無針對此檔 finding)
- Task 1.2: complete (uncommitted; spike 2 四測試通過; Codex finding [high] 寫入 WSL ＋ guard 太晚 → 修正中，見 Ruling)
- Task 1.3: complete (uncommitted; spike 3 四測試通過; 同上 finding 修正中)
- Task 1.4: complete (uncommitted; Codex finding [medium] `--help` → 修正中)
- Task 1.5: complete (uncommitted)
- Task 1.6: complete (uncommitted; go/no-go GO; 紀錄 docs/research/2026-09-13/change-1a-spikes.md)
- Task 2.1–2.3: in progress（單一 sonnet agent，切換前派出，平行於 3.x）
- Task 3.1–3.3: in progress（單一 sonnet agent，切換前派出，平行於 2.x）
- Task 1.x Codex 修正: in progress（sonnet agent）
- Task 2.1–2.3: implemented (uncommitted; sonnet 單一 agent 批次；transport.rs 9 tests + default_path 3 unit tests 通過，指揮官重跑確認；假對端採 `[[bin]] herdr-client-test-child` + `required-features=["test-support"]` + self dev-dependency，實測 `CARGO_BIN_EXE_` 可解析；`$XDG_CONFIG_HOME` 標待查證) — task review（Codex）待第 3 組完成後一併跑
- Task 1.x Codex 修正: complete (uncommitted; finding 1 high／finding 2 medium 皆確認並修正；寫入需 `HERDR_CLIENT_TEST_ALLOW_WSL_WRITES=1`、TabGuard 先快照後建立、examples 的 argv 解析；AGENTS.md 補 opt-in 一句；指揮官重跑：無 opt-in 時 4 個寫入測試跳過、WSL tab 集合不變、`--help` exit 0、`--bogus` exit 2) — 待補：README 的真機測試一節要寫 opt-in 變數（併入 task 5.1）
- Task 3.1–3.3: implemented (uncommitted; sonnet 單一 agent 批次；types 6、events 8、contract 5 tests 通過，指揮官重跑全 crate gate 確認；payload 為包欄位形狀；D8 根選擇可行，`jsonschema::draft202012::new`；wire result 型別以手寫 Deserialize 檢查 `type`) — task review（Codex）啟動中
- Task 2.1–2.3: task review round 1（Codex, working-tree focus）→ 4 Important open: (a) child 立即退出時 send 路徑 BrokenPipe 未分類、測試未先 send（transport.rs:351-363, child_stdio.rs:153-155）；(b) stderr JoinHandle 丟棄、錯誤訊息可能空（child_stdio.rs:164-180）；(c) PIPE_BUSY 重試／逾時無測試（transport.rs:204-227）；(d) task 2.3 的 HERDR config_dir 一手查證未做（default_path.rs:29-33）。→ fix round 1/5：resume 原實作者
- Task 3.1–3.3: task review round 1（Codex）→ 3 open: (a) high `PaneReadParams.format: Option<String>` 可序列化出 schema 外的值（pane_read.rs:25-26；指揮官派工時寫錯型別，spec 已補 `format` 值域）；(b) medium snapshot 測試未逐欄核對（types.rs:40-67）；(c) medium 事件測試未斷言 payload 值、fixture 未涵蓋 moved 類分支（events.rs:130-194）。→ fix round 1/5：resume 原實作者（與第 2 組修正平行，延續 preflight 的平行裁決，檔案不重疊）
- Task 3.1–3.3: fix round 1/5 (3 addressed, 0 open — ReadFormat enum；Info 必填欄位不加 default；事件逐欄斷言＋10 個 schema-valid 最小案例；tests/common/mod.rs 共用 validator；指揮官重跑 types/events/contract 全過) → scoped re-review（Codex）啟動中
- Ruling: D13 schema 必填欄位缺席時解析失敗（偏離 §5.2「全部 default」） — Codex finding 成立，靜默預設值會掩蓋映射錯誤 — 代價：HERDR 未來移除必填欄位時會解析失敗而非降級。
- Task 2.1–2.3: fix round 1/5 (4 addressed, 0 open — send 路徑 BrokenPipe 分類共用 helper；stderr JoinHandle 保存並 200ms 內 join（競態未能重現，仍套用）；PIPE_BUSY 以 max_instances(1) 製造 231 兩個測試；config_dir() 依 HERDR 原始碼 src/config/io.rs:30-68 改寫：XDG_CONFIG_HOME 跨平台優先、空字串亦算有設；指揮官重跑 transport 20 tests 全過) → scoped re-review（Codex）啟動中
- Task 3.1–3.3: scoped re-review round 1（Codex）→ f1 ADDRESSED；f2 NOT ADDRESSED（未建模的 required 欄位缺席偵測不到；缺席測試只刪一個欄位）；f3 NOT ADDRESSED（逐欄斷言仍只比部分欄位、Option 無 Some 案例、agent 只比 is_some）
- Ruling: f2a「為偵測缺席而建模 Cockpit 用不到的 required 欄位（terminal_id 等）」不採納 — 違反 §5.2 observer 子集原則，D13 已限縮為「已建模的必填欄位」 — 代價：HERDR 若少送 Cockpit 不用的欄位，不會被察覺（本來就不影響功能）。
- Task 3.1–3.3: fix round 2/5 開始（f2b 表格化必填缺席測試＋SessionSnapshot 容器 default 移除；f3 通用逐欄比對 helper＋Some 案例）→ resume 原實作者
- Task 2.1–2.3: scoped re-review round 1（Codex）→ f1、f2、f4 ADDRESSED；f3 NOT ADDRESSED（成功重試測試在回歸時 tokio::join! 永久掛住，transport.rs:341-348）；新增 medium：Unix 以 kill -0 輪詢會把 zombie 當存活（transport.rs:454-462）。→ fix round 2/5：resume 原實作者
- Task 2.1–2.3: fix round 2/5 (2 addressed, 0 open — busy 測試外層 timeout(5s)，RETRY_BUDGET=0 時 ≤5s 明確失敗；`ChildStdioStream::wait_for_exit` test-support 限定 helper 取代 kill -0；cross-compile check --lib 通過；指揮官重跑 transport 13/13) → scoped re-review round 2（Codex）啟動中
- Task 3.1–3.3: fix round 2/5 (2 addressed, 0 open — 表格化必填缺席測試 `assert_required_fields_detected`；SessionSnapshot 容器 default 移除；通用 `assert_modeled_fields_match` 逐欄深比對；Option 欄位 Some 案例；指揮官重跑全 crate 通過) → scoped re-review round 2（Codex）啟動中
- Ruling: `.markdownlint-cli2.jsonc` 加 `.superpowers/**` 到 ignores — SDD scratch 不進 lint — 代價：無（該目錄本就 git exclude）。
- Task 2.1: complete (uncommitted, review clean after 2 fix rounds)
- Task 2.2: complete (uncommitted, review clean after 2 fix rounds)
- Task 2.3: complete (uncommitted, review clean after 2 fix rounds; config_dir 規則已依 HERDR src/config/io.rs 回寫 spec 與 tasks)
- Task 2.x: minor (deferred): CREATE_NO_WINDOW 的「桌面無視窗」只能從旗標看出，待使用者目視複驗（spike 4 已記錄）
- Task 3.1–3.3: scoped re-review round 2（Codex）→ f2 NOT ADDRESSED（required 清單只讀 p22，common/mod.rs:163-167）；f3 NOT ADDRESSED（helper 對 raw 缺 key 跳過，rename 拼錯會漏網，common/mod.rs:130-136）。→ fix round 3/5：resume 原實作者（最後一輪 resume；第 4 輪起換 fresh 更強模型）
- Task 3.1–3.3: fix round 3/5 (2 addressed, 0 open — helper 缺席預設失敗＋allow-list，負向測試 assert_modeled_fields_match_catches_wrong_serde_rename；required 清單 p22/p20 各跑＋drift 守衛；指揮官重跑 types/events/contract 通過) → scoped re-review round 3（Codex）啟動中
- Task 3.1: complete (uncommitted, review clean after 3 fix rounds)
- Task 3.2: complete (uncommitted, review clean after 3 fix rounds)
- Task 3.3: complete (uncommitted, review clean after 3 fix rounds; tests/common/mod.rs 提供 schema validator、assert_modeled_fields_match、assert_required_fields_detected 供後續 task 重用)
- Ruling: 4.4 的子程序橋接假對端 = 在 `herdr-client-test-child` 加 `relay <endpoint>` 模式（連到假 HERDR 的原生 socket／pipe，stdin↔socket 轉送） — 重用 2.2 的 bin 與 connector 程式碼，不另做 stdio 版假 HERDR — 代價：test_child 多一個模式。
- Task 4.1: dispatched（fresh sonnet；brief task-4.1-brief.md；report task-4.1-report.md；BASE = 工作樹（未 commit））
- Task 4.1: implemented (uncommitted; DONE; fake_herdr.rs 16 tests、crate 83 通過，指揮官重跑確認；concerns：stream 模組放寬為 pub(crate)；未設定 method 的回應預設等同 CloseBeforeReply；unix 路徑未實跑；發現 named pipe 不回就關只給乾淨 EOF、Abort 用 disconnect() 才有 Err(233)) → task review（Codex）啟動中
- Task 4.1: task review（Codex）→ 4 open: (1) high unix Abort 空實作＋測試跨平台斷言 Err（connection.rs:31-36）；(2) high drop 後 handler 存活（listener.rs:113-120）；(3) medium Windows 端點未用 temp_dir（listener.rs:125-139）；(4) medium 訂閱腳本無法依訂閱內容分派（config.rs:29-32）。⚠️ 子程序橋接端到端 → 由 4.4 驗證（controller 已確認屬設計）。→ fix round 1/5：resume 原實作者
- Ruling: Step::Abort 在 unix 明文降級為 EOF（AF_UNIX 無 RST 語意），測試依平台分開斷言 — 代價：unix 上 Client 的「I/O 錯誤」Scenario 只能靠 Windows 測試覆蓋。
- Task 4.1: fix round 1/5 (4 addressed, 0 open — unix Abort 文件化降級＋cfg 分測；JoinSet 追蹤 handler、Drop abort_all；temp_dir 端點；SubscribeMatcher/SubscribeRule 分派；fake_herdr 19 tests，指揮官重跑確認) → scoped re-review（Codex）啟動中
- Task 4.1: scoped re-review round 1（Codex）→ f1、f3、f4 ADDRESSED；f2 NOT ADDRESSED（Drop abort_all 快照與 accept loop spawn 的競態，listener.rs:120-130）；Minor: SubscribeMatcher precedence 未直接測試。→ fix round 2/5：resume 原實作者
- Task 4.1: fix round 2/5 (1 addressed, 0 open — HandlerRegistry{closed, JoinSet} 同鎖，Drop 持鎖 closed=true 再 abort_all，accept loop 持鎖檢查；multi_thread 壓力測試；precedence 測試；fake_herdr 21 tests，指揮官重跑確認) → scoped re-review round 2（Codex）啟動中
- Task 4.1: complete (uncommitted, review clean after 2 fix rounds; FakeHerdr API 見 task-4.1-report.md)
- Ruling: Client::request 在讀到回應前遇乾淨 EOF → `RequestError::Io(UnexpectedEof, "connection closed before response")`；第一次讀取遇 NotFound／ConnectionRefused → `RequestError::Connect(ServerNotRunning)`（D3） — 4.1 實測 named pipe 不回就關只給 EOF，transport 不會自己報錯 — 代價：1b 的 Disconnected reason 來自 Io 的訊息字串。
- Task 4.2: dispatched（fresh sonnet；brief task-4.2-brief.md；report task-4.2-report.md；BASE = 工作樹）
- Task 4.2: implementer 於 gate 前被 API 額度限制中斷（sonnet session limit，21:10 重置）；22:51 指揮官重跑：request.rs 10/10、fmt/clippy 乾淨、全 crate 綠；report 檔缺，已 resume 實作者只補寫報告（程式碼不動）
- Task 4.2: implemented (uncommitted; DONE; request.rs 10 tests，crate 103 通過；concerns：D12 後綴測試用檔內 test double；server_not_running 改用不存在端點避免 drop-then-connect 競態) → task review（Codex）啟動中
- Task 4.2: task review（Codex）→ 4 open: (1) critical Request trait 未 sealed（request.rs:12-16）；(2) critical 初次 send 的 ConnectionRefused 歸 Io（mod.rs:55-57）；(3) critical result+error 並存／ErrorBody 缺欄位仍當 Remote（mod.rs:103-131）；(4) high 成功案例未證明釋放連線（request.rs:51-57）。→ fix round 1/5：resume 原實作者
- Ruling: ErrorBody 的 code／message 依 D13 移除容器 default（允許 4.2 動 wire.rs 這一處） — schema required — 代價：無。
- Ruling: sealed trait 的 compile-fail 用 doc-test `compile_fail`，不加 trybuild 依賴 — KISS。
- Task 4.2: fix round 1/5 (4 addressed, 0 open — sealed Request＋compile_fail doc-test；classify_first_io_error 共用於 send/recv＋子程序整合測試；ErrorBody 移除容器 default、parse_response 四路 match＋矩陣測試；Drop-flag 釋放連線測試；指揮官重跑 request/doc/全 crate 通過) → scoped re-review（Codex）啟動中
- Task 4.2: scoped re-review round 1（Codex）→ f2、f3、f4 ADDRESSED；f1 NOT ADDRESSED（private sealing 模組對 crate 內 sibling 不可見，request.rs:48-51）。→ fix round 2/5：resume 原實作者
- Task 4.2: fix round 2/5 (1 addressed, 0 open — pub(crate) mod private；seal_sibling_test 正向；doctest 錯誤碼 E0277 實測；指揮官重跑 lib/doc/全 crate 通過) → scoped re-review round 2（Codex）啟動中
- Task 4.2: minor (deferred): rustdoc `compile_fail,<code>` 附加錯誤碼不校驗（實作者實測），註解已說明；無防呆。
- Task 4.2: complete (uncommitted, review clean after 2 fix rounds; Client::request／Request（sealed）／RequestError；parse_response 嚴格互斥；classify_first_io_error)
- Task 4.3: dispatched（fresh sonnet；brief task-4.3-brief.md；report task-4.3-report.md；BASE = 工作樹）
- Task 4.3: implemented (uncommitted; DONE; subscribe.rs 12 tests，crate 130 通過，指揮官重跑確認；concerns：closed-before-started 用檔內 test double；EventStream 手寫 Debug) → task review（Codex）啟動中
- Task 4.3: task review（Codex）→ 1 open: critical data 非物件仍當合法事件（subscribe.rs:137-141）；⚠️ 子程序橋接結束與一秒內送達 → task 4.4。→ fix round 1/5：resume 原實作者
- Task 4.3: fix round 1/5 (1 addressed, 0 open — data 必為物件、event 必為字串，6 單元＋7 整合負向測試；指揮官重跑 subscribe 與全 crate 通過) → scoped re-review（Codex）啟動中
- Task 4.3: complete (uncommitted, review clean after 1 fix round; Client::subscribe／EventStream／IncomingEvent／StreamError)
- Task 4.4: dispatched（fresh sonnet；brief task-4.4-brief.md；report task-4.4-report.md；BASE = 工作樹；relay 模式加在 test_child）
- Task 4.4: implemented (uncommitted; DONE; child_bridge.rs 5 tests，crate 148 通過，指揮官重跑確認；relay 模式加在 test_child) → task review（Codex）啟動中
- Task 4.4: task review（Codex）→ 2 open: (1) high 延遲測試無 timeout 且計時起點含子程序啟動（child_bridge.rs:184-203）；(2) high 存活探測把探測失敗當已結束、/proc 用 cfg(unix)、Windows 未查 tasklist exit status（child_bridge.rs:56-81）。→ fix round 1/5：resume 原實作者
- Task 4.4: fix round 1/5 (2 addressed, 0 open — 延遲測試 timeout(1s)＋起點改 subscribe 後；探測三態 Alive/Exited/ProbeError、/proc 限 linux、tasklist 檢查 exit status；指揮官重跑 child_bridge ×2 與全 crate 通過) → scoped re-review（Codex）啟動中
- Task 4.4: scoped re-review（Codex）→ f2 ADDRESSED；f1 標 NOT ADDRESSED 因 timeout 1.5s／下限 180ms 與指揮官派工文字（1s／200ms）不符
- Ruling: 保留 timeout 1.5s 與下限 180ms — timeout 只防無限卡住，規格「一秒內」仍由 `elapsed < 1s` 斷言把關；180ms 是假 HERDR Delay 起點早於 client 計時起點的容差（實測 220–240ms）；finding 衝突的是派工文字不是 spec — 代價：緩衝回歸要 1.5 秒才失敗而非 1 秒。
- Task 4.4: complete (uncommitted, review clean with 1 parked ruling; relay 模式、child_bridge 5 tests、延遲 19.7ms／231ms)
- Note: Codex 無法核實「test_child.rs 本輪未變」因整個 herdr-client/ 未 commit、無 git 基準 — 這是未授權 commit 的直接代價，待使用者決定。
- Task 5.1: dispatched（fresh sonnet；brief task-5.1-brief.md；report task-5.1-report.md）
- Task 5.1: implemented (uncommitted; DONE; real_herdr.rs 7 個 #[ignore] 測試兩種模式通過、舊 spike 檔刪除、capture_events example、events-lifecycle-p22.ndjson 24 行、README 三節補齊；status fixture 待指揮官在回合交界擷取；發現 report_agent idle 可能呈現 Idle 或 Done（符合 Done 語意）) → task review（Codex）啟動中；狀態擷取 240s 背景進行中
- Task 5.1: task review（Codex）→ 2 open: (1) high capture_events 重建行而非原始 wire 行（capture_events.rs:158-165）；(2) medium p22 lifecycle fixture 以 Path::exists 守衛靜默通過（contract.rs:164-190、events.rs）。→ fix round 1/5：resume 原實作者
- Ruling: 擷取工具改直接用 NdjsonStream 讀原始行，不加 library 的 raw API — YAGNI — 代價：example 內重複約 15 行 subscribe handshake。
- Task 5.1: fix round 1/5 (2 addressed, 0 open — capture_events 改寫原始行；lifecycle p22 重擷取 22 行；contract/events 無條件讀取；status p22 以 #[ignore] 佔位；指揮官重跑 gate 通過) → scoped re-review（Codex）啟動中；raw 狀態擷取 240s 背景進行中（target/capture-status2）
- Task 5.1: scoped re-review round 1（Codex）→ f1、f2 ADDRESSED；新增 high：截止時 timeout 取消非 cancellation-safe 的 recv_line 可能漏最後一行（capture_events.rs:232-261）。→ fix round 2/5：resume 原實作者（reader task＋grace drain，[[example]] test=true）
- Task 5.1: 指揮官以 raw capture_events 跨回合交界擷取 240s（target/capture-status2）：lifecycle 132 行、status 2 行（wJ:p1 done→working）；去識別化產出 tests/fixtures/events-status-p22.ndjson（2 行，0 洩漏）；已通知實作者在 fix round 2 一併把 status p22 佔位測試改無條件
- Task 5.1: fix round 2/5 (1 addressed, 0 open — reader task＋mpsc＋grace 200ms 的 capture_with_deadline；[[example]] test=true 讓 cargo test 跑 example 測試（實測）；status p22 測試改無條件；指揮官重跑全 crate 通過) → scoped re-review round 2（Codex）啟動中
- Task 5.1: scoped re-review round 2（Codex）→ 主修正 ADDRESSED；殘餘 high：grace timer 到期時 select! 可能略過已排隊事件、reader 未明確終止（capture_events.rs:209-223）。→ fix round 3/5：resume 原實作者（最後一輪 resume）
- Task 5.1: fix round 3/5 (1 addressed, 0 open — drain_until 在 timer 分支先 try_recv 清空；reader JoinHandle abort+await；競態測試以 64 筆預排訊息重現（bug 重引入 10/10 失敗）；drop-flag 測試；指揮官重跑 example 4 tests、全 crate、build --examples 通過) → scoped re-review round 3（Codex）啟動中
- Task 5.1: scoped re-review round 3（Codex）→ 主修正 ADDRESSED；殘餘 high：reader JoinError 非 cancelled（panic）被吞掉仍回傳 lines（capture_events.rs:271-275）。→ fix round 4/5：fresh implementer、模型升級 opus（SDD 規則）
- Task 5.1: fix round 4/5 (1 addressed, 0 open — fresh opus；capture_with_deadline 回 Result<_, CaptureError>，非 cancelled JoinError → Err；main 任一 Err 不寫檔 exit 1；PanickingStream 測試＋mutation check；指揮官重跑 example 5 tests、全 crate 通過) → scoped re-review round 4（Codex）啟動中
- Task 5.1: complete (uncommitted, review clean after 4 fix rounds; real_herdr.rs 7 tests、capture_events（raw、reader task、Result）、p22 lifecycle 22 行＋status 2 行 fixture、README 三節)
- Task 5.2: started — 全 workspace gate、openspec validate、Codex whole-branch final review（SDD final review）
- Final whole-branch review（Codex）→ Important×3：(1) 事件 payload 容器層 serde(default) 違反 D13（events.rs:274-407）；(2) EventsSubscribeRequest 公開且可交給 request()（client/mod.rs:14-16）；(3) 橋接握手後非零退出被當正常 EOF（child_stdio.rs:164-173）；medium：5.2 未完成（進行中）；low：D12 probe 序號未保留在 Remote 訊息（client/mod.rs:179-181）。→ 一次 fix wave（sonnet）涵蓋 1、2、3、5；handover 重寫平行派工
- Final fix wave: 4 addressed（payload 容器 default 移除＋16 表格化測試；EventsSubscribeRequest pub(crate)＋compile_fail；child_stdio EOF 時 500ms 內回收 exit status、非零 → Err；Remote.response_id）；指揮官重跑 workspace gate 與真機測試 → scoped re-review（Codex）啟動中
- Final fix wave scoped re-review：Codex 回「usage limit，03:55 重置」→ Ruling: 依 git-branch-workflow 收尾清單「額度不足時以 fresh subagent 對抗式審查代替，handover 註明」，改派 fresh opus 做 scoped re-review — 代價：非 Codex 視角；03:55 後可補跑 Codex 同一 focus 作二次確認。
- Final fix wave scoped re-review（fresh opus，替代 Codex）→ approve：4/4 ADDRESSED；新 Minor：WorkspaceFocusedPayload 缺表格化必填測試（events.rs:354）；nit：relay_then_fail 的 select! 取消非 cancel-safe 讀取（沿用既有 relay 寫法，風險極低）
- Ruling: 依 SDD 最終 review 不開第二波修正；WorkspaceFocusedPayload 測試記為 deferred minor 交使用者／下一段補 — 代價：少一個守門測試，行為本身正確。
- Task 5.2: complete (uncommitted; workspace gate 174/0/7、真機 7/7、openspec valid、handover 版本 4；Codex 二次確認待 03:55 後補跑)
- 全部 18 個 task complete。SDD workspace 保留至併回 main 後再刪。
- 使用者授權 commit：feat/herdr-client 上 5 個 commit（openspec 計畫與 spike 紀錄、crate、handover v4、chore .agents skills、WorkspaceFocusedPayload 守門測試）
- 使用者決定 #1：補守門測試＋等 Codex 二次確認（已補；Codex 二次確認執行中）；#2：只關 WSL 端測試 server、不 wsl --shutdown、spike 5(a) 不重驗
- 使用者決定 #3：spike 4 目視複驗留到 change 1b 真機驗收（Scenario A/B/F 從 HERDR pane 啟動 cockpit.exe 時一併看）；CREATE_NO_WINDOW 維持為防禦性設定
- WSL 端測試 server 已 `herdr server stop`（status: not running）；1b 真機測試時用 handover 第 1 節指令重新以 setsid -f 啟動
- 使用者決定 #4：開 Codex review gate（companion setup --enable-review-gate 成功，reviewGateEnabled=true）
- Codex 二次確認（額度重置後）→ f1、f2、f4 ADDRESSED；f3 NOT ADDRESSED：stdout EOF 後子程序 >500ms 才非零退出會被當正常 EOF（child_stdio.rs:238-245）
- Ruling: 雖 SDD 最終 review 規則不開第二波，但使用者決定 #1 明定「Codex 通過才併回」，且此為真實正確性缺口 → 做一次單一 finding 的小修正＋Codex scoped re-review — 代價：多一輪。採 Codex 建議：stdout 已關但 500ms 後子程序仍存活 → 回 ConnectionAborted 類錯誤（不回乾淨 EOF）。
- 使用者決定 #5：Codex approve 後全套由指揮官做——commit 修正 → squash 併回 main、刪 feat 分支 → /opsx:archive herdr-client 單獨 commit → 刪 SDD workspace → handover 改為「下一步 1b propose」
- Final fix round 2（Codex 二次確認 f3）: addressed — PostStreamExit 三分支；relay-then-fail 加 close_then_sleep_ms；2 個新測試；指揮官重跑 workspace gate 通過 → Codex scoped re-review 啟動中。備註：此分支（stdout 關但子程序 >500ms 存活）未在真機 WSL 驗（server 已依決定 #2 停止），nc 路徑不受影響。
- Codex scoped re-review（延遲退出修正）→ 核心 ADDRESSED；殘餘 high：try_wait/wait 的 OS 錯誤被併入 StillRunning（child_stdio.rs:257-264）
- Ruling: 採納錯誤傳播（exited_status_after_stream 回 `io::Result<PostStreamExit>`，wait 錯誤保留原因）；不做可注入 child-wait 的抽象，改以純函式映射＋單元測試覆蓋 — 代價：OS wait 錯誤路徑無整合測試。
- Final fix round 3（wait 錯誤傳播）: addressed — exited_status_after_stream 回 `io::Result<PostStreamExit>`；classify_post_stream 純函式＋4 單元測試；指揮官重跑 workspace gate 通過 → Codex scoped re-review 啟動中
