# design：attach-herdr-runtimes

## Context

動機見 `proposal.md` Why。本文件只寫 change 1b 特有的取捨；整體架構、資料流、型別草案、事件對照、
路由與設定格式都在設計文件（`docs/superpowers/specs/2026-09-13-cockpit-mvp-design.md`）§4、§6、
§7、§8，需求在本 change 的七份 spec，這裡不重述。

現況與約束：

- `herdr-client` 已在 main（公開 API 見 `herdr-client/README.md`）：`Connector`／`Client`／
  `EventStream`／`IncomingEvent`／observer 型別／`test-support` 的 `FakeHerdr`。1b 只透過這些介面
  使用它，不改它的公開 API。
- 1a 留下要沿用的事實（一手來源 `docs/research/2026-09-13/change-1a-spikes.md` spike 3、5 與
  1a design D12；`docs/handover.md` §3 是索引）：訂閱探測失敗的 `error` 回應 `id` 帶
  `:sub:<n>:probe` 後綴，client 已處理成 `Remote`，1b 不再處理；WSL 0.8.2 的 L 訂閱剛建立時會補推
  舊事件（待在 Windows 0.9.0 查證）；`report_agent state=idle` 可能呈現 `Idle` 或 `Done`；
  `wsl.exe --list` 輸出 UTF-16LE 且 exit code 恆 0；子程序橋接「stdout 已關但 500 ms 內未結束」回
  `ConnectionAborted`，訊息 `bridge closed stdout but did not exit within 500ms`
  （`herdr-client/README.md`）。
- 依賴方向 ADR-0003：`cockpit-core` 不得依賴 `herdr-client`。
- 對 HERDR 完全唯讀（ADR-0001）；真機不得 `herdr server stop` Windows 端（設計文件 §10.1）。

## Goals / Non-Goals

**Goals:**

- 把設計文件 §4.2 的七步資料流拆成「與 runtime 無關的驅動器」與「HERDR 特有的連線順序」兩層，
  讓 change 2、3 與未來其他 runtime 只實作 `AgentRuntime`。
- 每條 spec 的每個 Scenario 都能用假 runtime 或假 HERDR 在 `cargo test` 內重現，真機只驗 Scenario
  A／B／F 與兩項 1a 遺留查證。
- 不引入 `futures` crate；新依賴只有 axum、toml、tracing-subscriber、chrono、anyhow 與三個 dev
  依賴；`cockpit` 不直接依賴 `herdr-client`（D16）。

**Non-Goals:**

- 不做任何 Domain 層（change 2）、`pane.read`（change 3）、寫入 HERDR、Tauri、service worker
  （proposal 非目標表）。
- 不重新設計設計文件已定的 JSON 形狀、路由、設定格式；只在下列 D3、D12 補漏。

## Decisions

### D1. 連線驅動器放在 `cockpit-core`，HERDR 特有順序封裝在 `HerdrRuntime`

設計文件 §4.2 把整個七步迴圈寫成「每個 runtime 一個 tokio task（cockpit-herdr）」，§6.2 又要求
`cockpit-core` 只看得到 `AgentRuntime` 的 `snapshot()`／`subscribe()`。兩者取其後者：

- `cockpit-core::driver::run(runtime, store, policy, cancel)` 負責與 runtime 種類無關的部分：
  `Connecting` → `subscribe()` → `snapshot()` 整份替換（期間丟棄事件）→ `Connected` → 套用事件；
  Drift 立即重拿（進行中不重複）；定期重拿；任何失敗 → `Disconnected` → 退避或固定間隔 → 重來。
  即 spec `runtime-driver`。
- `HerdrRuntime::subscribe()` 內部完成 Probe → seed snapshot → L → S，並回傳合併流；
  `HerdrRuntime::snapshot()` 取 authoritative snapshot、翻譯、順便把新的 pane 集合交給內部的 S
  管理器決定要不要重開。即 spec `herdr-runtime-session`。

理由：Drift、定期重拿、退避這些邏輯與 HERDR 無關，放 core 才能用不含 HERDR 的假 runtime 測；
cockpit-herdr 的測試專注在連線順序與 ReopenStatus。代價：`AgentRuntime` 的 `subscribe()` 語意變重
（它做四件事），文件要寫清楚。替代方案「全部放 cockpit-herdr、`AgentRuntime` 只剩 `id()`」會讓
§6.2 的抽象名存實亡，否決。

Drift 或定期重拿**進行中**到達的事件照常套用，替換完成後以 snapshot 為準，之後的失配再走 Drift；
不另做「重拿期間丟棄」模式——初次連線的丟棄是因為狀態庫還是空的，重拿時狀態庫已有內容，多一套
緩衝邏輯只會增加狀態機分支。代價：重拿與事件交錯時可能短暫回退一筆，下一筆事件或 snapshot 修正。

回寫設計文件 §4.1、§4.2（迴圈所在 crate）與 §7.1（task 4.5）。

### D2. 事件流以 `tokio::sync::mpsc` 承載，項目是 `Result<RuntimeEvent, RuntimeError>`，不引 `futures`

設計文件 §6.2 寫 `BoxStream<'static, RuntimeEvent>`。改為 `cockpit-core` 定義
`RuntimeEvents { rx: mpsc::Receiver<Result<RuntimeEvent, RuntimeError>>, _guard }`：

- `Result` 讓「L 或 S 斷了」能帶原因傳給驅動器（純 `RuntimeEvent` 沒有錯誤通道，只能用流結束
  表示，原因會遺失）。
- `_guard` 持有內部 reader task 的 `JoinHandle`，drop 時 `abort()`，保證「釋放事件流即關閉連線與
  子程序」（spec `runtime-driver`「可停止」、`herdr-runtime-session`「釋放事件流」）。只靠 receiver
  drop 讓 sender 失敗來終止不夠：卡在 `recv_line().await` 的 task 要等到下一個事件才會發現。
- 不引 `futures`／`tokio-stream`：1a 已刻意避免（`EventStream::next` 是 inherent 方法），本 change
  同樣只提供 `async fn next()`。

通道容量 1024；HERDR 事件量在本機每秒個位數（spike 3 的 136 筆是數分鐘累積），不會撐滿。
回寫設計文件 §6.2 的簽章（task 4.5）。

### D3. `RuntimeEvent` 補三個變體，`FocusChanged` 是部分更新

設計文件 §7.2 把 `workspace_renamed`／`tab_renamed` 寫成「`WorkspaceUpserted`（以現有物件改
label）」，但翻譯層是無狀態的純函數，手上只有 id 與 label，組不出完整物件。補
`WorkspaceRelabeled { id, label }`、`TabRelabeled { id, label }`，由狀態庫改標籤、不存在 → Drift。
同理 `workspace_focused`／`tab_focused`／`pane_focused` 各只帶一個 id，`FocusChanged` 改為三個
`Option` 欄位、`None` 表示該層不變。`TabsReplaced` 帶 `workspace_id`（`tab_moved` payload 只有該
workspace 的 tabs）。這三處回寫設計文件 §6.1、§7.2（task 4.5）。

### D4. 探測類錯誤用 `RuntimeError::Unavailable { reason, retry_after }` 表達固定間隔

設計文件 §7.1 規定 WSL 探測失敗「以 `wsl_probe_secs` 間隔再探，不進退避序列」，其他失敗走退避。
驅動器不認得 WSL，所以由錯誤本身帶提示：`RuntimeError` 分 `Unavailable { reason, retry_after }`
與 `Failed(String)`，驅動器 `retry_in = err.retry_after().unwrap_or_else(|| backoff.next())`，且
`Unavailable` 不推進退避。替代方案「驅動器依 runtime kind 查表」把 HERDR 知識帶進 core，否決。

### D5. 移除事件連鎖刪除子物件

`herdr-client/tests/fixtures/events-lifecycle-p20.ndjson`（spike 3 真機擷取，136 行）有多次
`tab_closed`，卻沒有任何一行 `pane_closed`：HERDR 關 tab 時不替其下 pane 另發事件。若不連鎖，孤兒
pane 會留到下一次定期 snapshot（最長 30 秒）才消失。因此 `TabRemoved` 連帶移除其 panes 與 agent
紀錄、`WorkspaceRemoved` 連帶移除其 tabs／panes／agents。代價：若某版 HERDR 改成會另發
`pane_closed`，那筆會撞 Drift 觸發一次重拿——自癒，只是多一次 snapshot。真機驗收（task 4.2）順手
記錄 Windows 0.9.0 關 tab 時的實際事件序列。

### D6. tab／workspace 的彙總狀態沿用 HERDR 給的值，不自行重算

snapshot 與 `workspace_*`／`tab_*` upsert 事件都帶 HERDR 算好的 `agent_status`；
`pane.agent_status_changed` 只更新 pane。Cockpit 不定義「多個 pane 怎麼彙總」的規則（那是 HERDR 的
runtime facts，`docs/cockpit-spec.md` §24 source of truth），所以 tab／workspace 的彙總值最長要等
到下一次 snapshot 才追上 pane。Scenario B 的通過條件是 pane 一秒內變色，不受影響。若真機驗收發現
HERDR 在狀態變化時也會發 `workspace_updated`／`tab_*`（fixture 裡 `pane_updated` 很頻繁，但沒
觀察到 tab 層），落差會更小，記進 acceptance 紀錄即可。

### D7. 投影的順序與孤兒

狀態庫用 map 存，投影要有穩定順序：workspace 與 tab 依 `number` 遞增；pane 依進入狀態庫的先後
（狀態庫給每個 pane 一個遞增序號，snapshot 替換時照 snapshot 陣列順序給號）。pane id 字串
（`wJ:p10` 排在 `wJ:p2` 前）不適合當排序鍵。父層不存在的物件不投影（D5 之後理論上不會有，留作
防禦）。

### D8. WSL 探測：以 NUL byte 判別編碼，探測只在建立事件流時做，探測器可注入

- spike 5 建議「一律 `from_utf16_lossy`」；但使用者環境若設了 `WSL_UTF8=1`，輸出會是 UTF-8，硬當
  UTF-16 解會變亂碼。改成：stdout 含 NUL byte → UTF-16LE，否則 UTF-8（spike 5 同時查證了
  `from_utf8().is_ok()` 對 UTF-16LE 也回 `Ok`，所以判別只能靠 NUL）。
- 只在 `subscribe()` 前探測，`snapshot()` 不探測：`Connected` 期間 L／S 連線活著就代表虛擬機在跑；
  定期 snapshot 若也探測，每 30 秒多一次 `wsl.exe` 啟動。虛擬機關掉時 L／S 會 EOF → 斷線 → 下一輪
  再探測。
- 探測器是 `cockpit-herdr` 內的 trait（真實現跑 `wsl.exe --list --running --quiet`，Windows 加
  `CREATE_NO_WINDOW`；測試用假探測器回固定清單或錯誤），解碼是純函數各自測。
- 回寫設計文件 §7.1「輸出為 UTF-16，解析時要轉碼」為 NUL 判別規則，並更正 `docs/handover.md`
  §3「一律 `from_utf16_lossy`」那條（task 4.5）。

### D9. 狀態庫共享與投影任務

`cockpit-core::StoreHandle`：`Arc<Mutex<RuntimeStore>>` 加一個 `tokio::sync::Notify`（dirty）。
驅動器每次 `replace`／`apply`／改連線狀態後 `notify_one()`。投影任務：等 dirty → 睡 50 ms（合併）
→ 鎖住算 `ProjectedState` → 與前一份比較（`PartialEq` 排除 `generated_at`）→ 有變才 `version + 1`
並 `watch::Sender::send`。WebSocket 端各自 `watch::Receiver`，連上先送 `borrow()` 的現況。
`std::sync::Mutex` 而非 tokio 的：臨界區只有純記憶體操作，不跨 `await`。時間欄位
（`generated_at`、`since`、`last_snapshot_at`、`updated_at`、`recent_events[].at`）內部存
`SystemTime`，投影時用 `chrono` 格式化為 RFC 3339（UTC）；std 沒有這個能力，手刻日期算法不值得。

### D10. ReopenStatus 的實作位置與失敗處理

S 管理器住在 `HerdrRuntime` 內（`Arc<Mutex<StatusSubscription>>`），L reader 看到
`pane_created`／`pane_closed`／`pane_moved`、或 `snapshot()` 算出集合不同時，把新集合丟給它；它以
200 ms 去抖動後開新 S、收到 `subscription_started` 才關舊 S（spike 3 驗證重疊不丟事件）。新 S 建立
失敗（例如 pane 已消失）不在原地重試，改為往合併流送 `Err`，讓驅動器整輪重來——重來本來就會重取
seed，比在 S 管理器內再做一套重試簡單。代價：一次多餘的斷線顯示。S 管理器跨重連共用同一個
`HerdrRuntime` 實例，所以每次 `subscribe()` 開頭先重置它（清掉上一輪的清單、連線與待處理的去抖動）。

### D11. `pane_agent_detected.final_status` 不使用

設計文件 §2.3 表（本 change 前置已補）標明此欄位語意未查證。翻譯只用 `agent`／`released`；agent
釋放後 pane 的狀態由 S 上隨之而來的 `pane.agent_status_changed`（spike 3 觀察 `release-agent` 會
再推一筆 `unknown`）或下一次 snapshot 更新。不依賴推測語意。

### D12. protocol 警告放在投影的 `connection.protocol_warning`

設計文件 §7.1 說「在畫面連線狀態旁標註」，§6.4 的 JSON 沒有欄位。加 `protocol_warning: string|null`
到 `connection`（`connected` 時），`RuntimeSnapshot` 帶 `protocol_warning: Option<String>` 由
`HerdrRuntime::snapshot()` 填。回寫設計文件 §6.4（task 4.5）。

### D13. 靜態資源用 `include_str!`／`include_bytes!`，圖示由 stdlib 腳本產生後進 repo

`cockpit/assets/` 放 `index.html`、`app/*.js`、`app/style.css`、`manifest.webmanifest`、
`icons/icon-192.png`、`icons/icon-512.png`；不用 `build.rs`、不用 `rust-embed`。PNG 由
`cockpit/assets/gen-icons.py`（只用 `zlib`／`struct`，`uv run --no-project python` 執行）產生純色
底＋簡單幾何的圖示，產物進 repo，測試只驗 PNG 檔頭與尺寸。深色配色取
`docs/cockpit-dashboard-concept.png` 的底色系。

### D14. 測試工具與 `FakeHerdr` 的加法擴充

- HTTP 路由：`tower::ServiceExt::oneshot` 對 `Router` 直接發請求（dev 依賴 `tower`、
  `http-body-util`），不開 port。
- WebSocket：真的綁 `127.0.0.1:0`，dev 依賴 `tokio-tungstenite` 當客戶端。
- 驅動器：`cockpit-core/tests/` 用腳本式假 runtime（呼叫紀錄、可延遲的 snapshot、可控事件流）＋
  `tokio::time::pause()`。
- `HerdrRuntime`：`herdr-client` 的 `FakeHerdr`。只缺一個能力，在 `herdr-client/src/testing` 做
  加法（1a spec 不變）：`with_method_responses(method, Vec<MethodResponse>)` 依呼叫序回應、最後一筆
  重複（Drift 重拿要回不同的 snapshot）。區分 L 與 S 不用加：`SubscribeMatcher::LifecycleOnly`／
  `PerPane`（`herdr-client/src/testing/config.rs`）已能做到，`tests/fake_herdr.rs` 已有對應測試。
  改動只在 `test-support` feature 內，仍跑 `herdr-client` 的 crate gate。
- 畫面：`cockpit/examples/ui_preview.rs` 用一份 fixture `ProjectedState` 起 dashboard server 並每
  兩秒改一個 pane 的狀態，供瀏覽器工具與人眼檢查 Scenario「兩個 runtime 的畫面」與「未知狀態」；
  不需要 HERDR。
- 真機：`cockpit/tests/real_attach.rs` 全部 `#[ignore]`：零設定啟動、等 `Connected`、比對
  `/api/state` 的 pane 數與 `herdr api snapshot`（Windows 端）；WSL 端由設定檔指定。

### D15. pane 的 `exited` 只來自事件

HERDR snapshot 的 `PaneInfo` 沒有 exited 欄位（`herdr-client/src/types/snapshot.rs`），所以
`pane_exited` 標記的刪除線在下一次 snapshot 替換後會消失（若 HERDR 仍列出該 pane）。接受：exited
pane 通常很快被關閉；不為此保留跨 snapshot 的本地旗標（會與「整份替換」語意衝突）。

### D16. 程序生命週期與日誌

`cockpit` 的 `main`：載入設定（`config::load(args, cwd, lookup_env)`，工作目錄與環境變數以參數
注入，`main` 包一層真值，測試才不會隨機器狀態飄；同 1a `default_socket_path` 的做法）→ 建
`StoreHandle` 與投影任務 → 每筆 runtime 交給 `cockpit-herdr` 的工廠
`cockpit_herdr::build(HerdrEndpoint, options) -> (endpoint 描述, Arc<dyn AgentRuntime>)`
（`HerdrEndpoint::Socket(path)` → `NamedPipeConnector`／`UnixSocketConnector`；`Wsl { distro, socket }`
→ `ChildStdioConnector` 跑 `wsl.exe -d <distro> -e nc -U <socket>` 加探測器；`Command(argv)` →
`ChildStdioConnector` 不探測；`Default` → `default_socket_path_from_env()`；描述取
`Connector::describe()`）並 spawn 驅動器 → 起 axum → 等 Ctrl-C → 取消驅動器（D2 的 guard 收連線
與子程序）→ 結束。工廠放 `cockpit-herdr` 是為了讓 `cockpit` 完全不依賴 `herdr-client`，維持
ADR-0003 的依賴圖（`cockpit → cockpit-herdr → herdr-client`）。日誌用 `tracing-subscriber` 的
env filter（`RUST_LOG`，預設 `info`），只寫 stderr；驅動器把「snapshot 前丟棄的事件數」以
`tracing::debug!` 輸出，供 task 4.4 觀察。

## Risks / Trade-offs

- [WSL 0.8.2 的 L 訂閱剛建立時補推舊事件；Windows 0.9.0 未知] → 驅動器丟棄 authoritative snapshot
  前的事件；之後的過期事件走 Drift 重拿（進行中不重複，速率被一次 snapshot 來回限制）。task 4.4 在
  Windows 端實測並回寫設計文件 §2.3。
- [fixture 出現 `pane_agent_detected` 早於同一 pane 的 `pane_created`（de-identify 後的觀察）] →
  同上，Drift 自癒；若真機驗收看到重拿次數異常（每分鐘超過個位數），再加「Drift 重拿最小間隔」。
- [pane 集合頻繁變動時每次重開 S 都是一次 `wsl.exe` 啟動（0.1–0.3 秒）] → 200 ms 去抖動；
  ADR-0002 已接受此成本，升級路徑是方案 B。
- [tab／workspace 彙總狀態最長落後一個 resnapshot 間隔（D6）] → pane 列即時；可把
  `resnapshot_secs` 調小。
- [Windows 端斷線重連只能用假 HERDR 驗] → 設計文件 §10.1 禁令；真機 Scenario F 只在 WSL 端。
- [畫面沒有自動化測試] → `ui_preview` example ＋ 瀏覽器工具截圖／DOM 檢查列入 task 3.6 驗收；
  render 邏輯保持純函數（state → DOM）以便日後加測試。
- [L 與 S 是兩條連線，順序不跨連線保證（設計文件 §6.3 已知）] → `pane_updated` 帶的舊狀態會被下一
  筆狀態事件或 snapshot 修正。
- [`listen` port 被占用] → 啟動失敗並印出位址與原因，不自動換 port。
- [`tokio-tungstenite` 與 axum 內部 `tungstenite` 版本不同] → 只當測試客戶端，型別不互通也無妨；
  實作時取 crates.io 當時穩定版並記進 `Cargo.lock`。

## Migration Plan

全新 crate，沒有既有資料或介面要遷移。啟動方式 `cargo run -p cockpit`（或 release build 的
`cockpit.exe`），停用就是不啟動它；對 HERDR 沒有任何殘留狀態（唯讀）。回退：刪除三個 crate 與
workspace members 即可，`herdr-client` 不受影響。

## Open Questions

- Windows 0.9.0 的 L 訂閱是否也在建立瞬間補推舊事件？答案不改 spec（丟棄與 Drift 已涵蓋），只決定
  設計文件 §2.3 要不要加註；task 4.4 回答。
- HERDR 在 pane 狀態變化時是否也發 `workspace_updated`？（tab 層沒有 `tab_updated` 這種事件，
  `EventKind` 26 種裡沒有。）只影響 D6 的落差大小，task 4.2 順手記錄。
- `pane_agent_detected.final_status` 的實際語意？本 change 不使用（D11），日後要用時再對真機實測。
