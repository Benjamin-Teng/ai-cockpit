# design：pipeline-projection

## Context

動機見 `proposal.md`；行為合約見本 change 的 `specs/`。本文件只寫 change 2 特有的取捨，架構總覽不重述，
請對照設計文件 `docs/superpowers/specs/2026-09-13-cockpit-mvp-design.md`：§4.1（domain 加在
`cockpit-core`）、§6.3／§6.4（`RuntimeStore`、`ProjectedState` 與 JSON 基底）、§8（HTTP、設定、畫面）、
§12（binding 不綁 pane id、不寫 HERDR metadata）。

現況中與本 change 直接相關的事實（1b 產出）：

- `project(store, version, now)` 是唯一組裝投影的地方，只讀 `RuntimeStore`；`spawn_projector` 以
  `Notify`＋50 ms 合併、`content_eq` 比較、`send_replace` 廣播。`StoreHandle` 是 `Mutex<RuntimeStore>`
  外加寫入方法，每個寫入方法都 `notify_one()`。
- runtime 斷線時狀態庫**保留**最後一份 pane 資料（`set_connection` 只改連線狀態），所以 domain 推導不能
  只看 pane 存在與否，必須同時看連線狀態。
- `Config` 的 `ConfigSource::{Explicit, Cwd}` 帶設定檔完整路徑，`ZeroConfig`／`Inline` 沒有路徑。
- `http.rs` 只有 `GET` 路由，`AppState` 只有 `watch::Receiver`；`cockpit` crate 沒有接假 HERDR，既有測試
  直接操作 `StoreHandle` 與 `router`。
- 前端每次推送整頁重畫（`render.js` 純函數），`channel.js` 在 `onopen` 歸零退避。

## Goals / Non-Goals

**Goals:**

- Domain 型別、進度轉移、綁定解析、StageStatus 推導全部是 `cockpit-core` 內的純函數，可不經 HTTP 與
  HERDR 單元測試。
- 投影仍是單一 version 序列：Runtime 層與 Domain 層任何變動都走同一個投影任務。
- 寫入路徑單純且可證明不遺失：序列化、先落檔再生效。

**Non-Goals:**

- 不做多 Cockpit 實例共用同一狀態檔的協調（檔案鎖、合併）。
- 不處理 `/ws` 的跨來源讀取（1b 既有行為，見 Risks）。
- 不改 `herdr-client`；`cockpit-herdr` 只動 `Shutdown.aborts` 修剪。

## Decisions

### D1 Domain 放 `cockpit-core::domain`，設定解析與檔案 IO 放 `cockpit`

`cockpit-core` 新增 `domain` 模組：定義型別（`ProjectDef`／`StageName`／`WorkstreamDef`／`BindingSpec`／
`TaskDef`、`TaskProgress`／`Mark`、`Override`、`BindingResolution`、`StageStatus`）、進度操作
`apply_op(&ProjectDef, &Progress, op) -> Result<Progress, Rejection>`、設定覆蓋時的驗證
`validate_override(&Override, &RuntimeStore) -> Result<(), Rejection>`（runtime 未登記／未連線／pane 不存在或
已 exited）、`resolve_binding(&WorkstreamDef, Option<&Override>, &RuntimeStore)`、`derive_status(...)`。`cockpit` 的 `config.rs` 負責 TOML → `ProjectDef`
與驗證（沿用手寫「區段.欄位」錯誤字串的既有做法），新模組 `progress.rs` 負責狀態檔讀寫與寫入序列化。

- 為什麼：設計文件 §4.1 明定 domain 在 `cockpit-core`；純函數讓 Scenario C／D 的核心判定可在 core 測完。
- 否決：全部放 `cockpit`（domain 會和 axum／檔案 IO 糾纏，core 的投影也拿不到）；把 TOML 解析放 core
  （core 不該知道設定檔格式，1b 也是 `cockpit` 解析後把結果交給 core）。

### D2 Domain 狀態掛進 `StoreHandle`，投影簽章改為 `project(store, domain, version, now)`

`StoreHandle` 的 `Mutex` 內容由 `RuntimeStore` 改為 `{ runtime: RuntimeStore, domain: DomainState }`
（`DomainState`＝`Vec<ProjectDef>`＋各 task 進度＋覆蓋＋載入時的 warnings）；新增 `set_domain` 類寫入方法，
同樣 `notify_one()`。投影任務在同一把鎖內讀兩者，綁定解析與 StageStatus 推導在投影時即時計算、不存快取。

- 為什麼：兩層共用一個 version 序列與同一個 50 ms 合併，天然滿足「Domain 變動也遵守 version 與合併廣播」；
  綁定依賴 runtime 狀態，同一把鎖下讀才不會看到兩層不一致的瞬間。
- 否決：另開一條 domain `watch` 再在 HTTP 層合併（兩個 version、合併時機競爭）；把解析結果存進狀態庫
  （每次 runtime 事件都得同步更新，重複投影該做的事）。

### D3 覆蓋失效：投影維持純函數，刪除由寫入服務非同步完成

`resolve_binding` 遇到「覆蓋的 runtime 已 connected 但 pane 不存在或 exited」時，直接回傳自動解析的結果
（視同覆蓋不存在），並把該覆蓋列入 `stale_overrides`。投影任務把非空的清單送到 `cockpit` 的寫入服務
（`mpsc`），寫入服務走與 HTTP `DELETE` 相同的序列化寫入路徑刪除並落檔。

- 為什麼：投影不做 IO、不改狀態；即使刪除還沒落檔，畫面結果已經正確（spec「回到自動解析」即時成立）。
- 失效刪除落檔失敗：記 error、記憶體中照樣刪除，下一次任何成功寫入會把正確狀態寫出（與 HTTP 寫入「失敗
  不生效」不同，因為此處保留失效覆蓋沒有意義）。

### D4 寫入序列化：單一 `tokio::sync::Mutex` 包「計算 → 落檔 → 生效」

寫入服務持有一把 async `Mutex`，每次寫入在鎖內：從 `StoreHandle` 讀目前 domain → core 純函數算出新
domain（被拒絕就回 409，不落檔）→ `spawn_blocking` 寫同目錄暫存檔 `<name>.tmp` 再 `std::fs::rename` 取代 →
成功後 `StoreHandle` 套用新 domain。清除標記在標記已是 `none` 時算出相同狀態，不落檔、不通知。

- 為什麼：一把鎖就保證並發請求不交錯、先落檔再生效；Windows 上 `std::fs::rename` 可覆蓋既有檔
  （`MoveFileExW` 帶 `MOVEFILE_REPLACE_EXISTING`，實作時以測試確認）。
- 否決：先改記憶體再背景落檔（寫檔失敗時畫面與檔案不一致，違反 spec）；每 task 一個檔（KISS）。

### D5 狀態檔形狀與相容

JSON、`version: 1`、存全部 task（不只存非初值），形狀見 `pipeline-progress` spec。載入時未知的
project／task／workstream 忽略並 warn，下次寫入自然不寫出；stage 不在 `stages` 時退回起始 stage，warning
放進 `DomainState.warnings`，**只在啟動時產生、保留到重啟**（寫入後不自動清除，避免 warning 在使用者沒注意
到時就消失）。`mark` 等欄位不合法視為損毀、啟動失敗——這是使用者手改檔案才會發生的情況，靜默丟棄會吞掉
進度。

### D6 寫入端點與來源檢查

路由：`POST /api/projects/{project}/tasks/{task}/{op}`、`PUT`／`DELETE
/api/projects/{project}/workstreams/{workstream}/override`。`{op}` 以字串比對四值，其他回 404。成功 204、
不回投影本體——畫面一律等 `/ws`（ADR-0004 整張圖推送不變）。

來源檢查做成只套在寫入路由上的 axum middleware：`Host` 必須是 `127.0.0.1`／`localhost`／`[::1]` 加上
**實際監聽埠**（由 `TcpListener::local_addr()` 取得後注入 `AppState`，測試用 port 0 也正確）；有 `Origin`
時必須等於 `http://<Host>`。

- 為什麼：沒有登入與 session，CSRF token 無處存放；瀏覽器對跨站 `POST` 一定帶 `Origin`，擋 `Origin` 即擋
  跨站表單與 `fetch`；檢查 `Host` 擋 DNS rebinding。
- 否決：要求 `Content-Type: application/json` 當防線（`POST` 端點沒有本體，且 `text/plain` 表單可繞過）；
  CORS 設定（CORS 只管讀回應，不擋請求送達）。

### D7 綁定匹配語意

- `workspace`、`pane_label`、`agent` 完全相等、區分大小寫；`cwd` 以 `\`→`/` 正規化後比對連續路徑片段，
  區分大小寫。片段比對讓 `worktrees/backend` 能匹配 agent `cd` 進子目錄後的 cwd，又不會把 `backend-old`
  當成 `backend`。
- 設計文件 §2.3：pane cwd 改變沒有事件，只靠定期 snapshot（`resnapshot_secs`，預設 30 秒）更新，所以
  以 cwd 為主要特徵的綁定在 agent 換目錄後最多延遲一個重拿週期才重新解析——這是 HERDR 事實，不另外補救。
- `pane_label`、`cwd`、`agent` 全部選填：交接原本寫「pane 標籤或 cwd 擇一」，改成全選填是嚴格的超集；
  特徵不足時結果是 `ambiguous`，畫面會顯示，使用者可補特徵或改綁。

### D8 StageStatus 推導與 Dependency 語意

依 `pipeline-domain` spec 的優先序：標記 → 依賴（Pending）→ 綁定 agent 狀態 → Ready。Pending 放在 agent
狀態之前，是因為同一 workstream 的多個 task 共用綁定；否則一個依賴未完成、還沒開始的 task 會跟著同線
的 agent 一起亮成 Running。

`CONTEXT.md` 原把 Dependency 定義為「Stage 之間的先後關係」；本 change 經使用者決定改為 **Task 之間的
依賴**（`depends_on`），Stage 在 MVP 只是線性順序。實作第一批 task 先改 `CONTEXT.md` 的 Pipeline、
Dependency、Task、RuntimeBinding 定義，避免之後的程式與詞彙表不一致。Stage 層級 DAG 留待之後，模型上
`stages` 保持 `Vec`，不預先建 Stage 依賴型別（KISS）。

### D9 前端：`actions.js` 管互動狀態，`render.js` 維持純函數，事件委派用 `pointerdown`

- 新增 `assets/actions.js`：持有 UI 狀態（改綁模式的目標 workstream、最近錯誤訊息）、送出 `fetch`、錯誤
  時更新 UI 狀態並觸發以最新投影重畫。`render.js` 改為 `renderState(state, ui)`，節點只帶
  `data-action`／`data-project`／`data-task`／`data-workstream`／`data-runtime`／`data-pane` 屬性，不綁
  listener。`channel.js` 仍只管通道。
- 事件在頁面根節點以委派處理，**以 `pointerdown` 觸發**（鍵盤操作另收 `click` 且 `event.detail === 0`）。
  原因：整頁重畫可能發生在 `mousedown` 與 `mouseup` 之間，兩者落在不同 DOM 元素時瀏覽器不會送出 `click`，
  agent 活躍時每秒多次推送會讓按鈕「按了沒反應」。spec 的「頻繁重畫時按鈕仍有效」情境就是驗這件事。
- 否決：暫停重畫直到操作結束（狀態會落後、邏輯複雜）；改成局部 DOM diff（違反 1b 已定的整頁重畫模式、
  需要框架或大量手寫）。

### D10 Scenario C／D 的驗法

| 層 | 驗什麼 | 位置 |
|---|---|---|
| core 單元 | 進度轉移、綁定解析（含 pane 換 id、cwd 片段、斷線、覆蓋失效）、StageStatus 推導（Scenario C、D 的判定） | `cockpit-core/tests/domain*.rs` |
| cockpit 整合 | 以 `StoreHandle` 直接灌 runtime 狀態＋真的 `router`（含寫入端點與狀態檔於暫存目錄），比對 `/api/state` 的 `projects[0].tasks[].status`／`stage` 與 `workstreams[].binding`；來源檢查；並發寫入；重啟保留 | `cockpit/tests/pipeline_api.rs` |
| 真機（WSL） | WSL 測試 server 上以 JSON-RPC `pane.report_agent` 讓兩三個 pane 變 working（沿用 `live-state-check.py` 的做法，設計文件 §2.3 事實），release 版 cockpit 讀 `/api/state` 輪詢到對應 task `running`，記錄秒數；再以 HTTP 推進／標記並驗 version 與重啟保留 | `docs/research/<日期>/pipeline-check.py` |
| 畫面 | `ui_preview` fixture 加 `projects`，headless Chrome 截圖與 DOM 檢查（Scenario D 網格位置、頻繁重畫下點擊）；使用者手動目視 | `docs/research/<日期>/` |

測試暫存目錄沿用 `cockpit/tests/config.rs` 自製 `TempDir` 的做法（1b 裁決不加 `tempfile`）。

### D11 1b deferred minor 的處理方式

- **Drift 追加重拿**（spec 化）：驅動器在 Drift 重拿進行中記錄「有事件成功套用」旗標，替換後旗標為真
  且追加次數 < 2 則再拿一次；次數在每次由 Drift 觸發時歸零。
- **固定重試間隔下限 1 秒**（spec 化）：`max(retry_after, 1s)`。`wsl_probe_secs = 0` 已被設定驗證擋掉，
  下限是為了其他 runtime 實作。
- **`channel.js` 退避歸零時機、`JSON.parse` 防護**（spec 化）。
- **`Shutdown.aborts` 修剪**：登記新 handle 時 `retain(|h| !h.is_finished())`，行為不變、不寫 spec。
- **停止時驅動器逾時**：`shutdown_components` 改為所有驅動器共用一個總期限（不再逐一 10 s×N），逾時
  `abort()` 後的 `await` 另設 1 秒上限，超過記 warn 並放手。不寫 spec（非對外行為），以測試驗。
- **測試缺口**（F4 session guard、`Unavailable` 由 snapshot 來源、`run_with_shutdown` bail 分支、
  `Path::exists()` 權限情境、依真實時間的測試、tokio-tungstenite 兩版）：逐條補測，補不了的（例如權限
  情境無可攜測法、tungstenite 版本受 axum 綁定）在 SDD ledger 寫 `Ruling:` 並說明理由。

### D12 連線後沉降重拿（apply 期間 task 7.2 發現）

- 事實：WSL 端 HERDR 0.8.2 對每條新的 `events.subscribe` 重播 server 啟動以來的整段事件歷史（舊值），部分在
  驅動器取得 snapshot 之後才到；重播指向仍存在的 id，不觸發 Drift，投影靜默倒退到下一次定期重拿（30 秒）。
  判別證據與重現見 `docs/research/2026-09-16/pipeline-projection-acceptance.md` task 7.2 節；Windows 0.9.0
  以唯讀擷取未見重播。
- 決定：驅動器在每次進入 `Connected` 後，等事件流靜默 1 秒再取得一次 snapshot，最晚第 5 秒；每次連線一次，
  與進行中的重拿合併（spec `runtime-driver`「連線後沉降重拿」）。
- 為什麼放驅動器：與 runtime 種類無關、只看事件流節奏；1b 的 Drift 重拿已在同一層。靜默 1 秒：實測重播在
  連上後約 0.8 秒內送完；上限 5 秒：事件頻繁的 runtime 不會永遠等不到。
- 否決：在 `cockpit-herdr` 依版本號特判（綁版本、升版會過期）；只縮短定期重拿間隔（每 runtime 常駐成本，
  且仍有數秒錯誤窗口）；丟棄連線後固定時間內的所有事件（會吃掉真實的新狀態變化）。

## Risks / Trade-offs

- [pane id 被重用：覆蓋指向的 pane 關掉後、HERDR 在下一次 snapshot 前以同 id 建立另一個 pane] → 覆蓋會
  綁到錯的 pane。HERDR id 規則是否重用未查證；影響只是畫面顯示錯的 agent 狀態，使用者可取消改綁。
  apply 時若有機會在 WSL 測試 server 觀察關 pane 再開 pane 的 id，記錄到驗收文件。
- [覆蓋失效的刪除是非同步落檔（D3）：畫面已回到自動解析、刪除尚未落檔時程式被關掉] → 重啟後該覆蓋會
  暫時復活；runtime 重連、authoritative snapshot 替換後投影會再次判定失效並刪除，自我收斂。窗口只有一次
  寫檔的時間，接受；spec 的情境因此寫成「寫入完成後重啟也不存在」。
- [同一 workstream 多個進行中 task 一起亮] → 使用者已接受；以 Pending 優先緩解，視覺上各 task 仍各自可標記。
- [Windows cwd 大小寫不一致] → 區分大小寫可能對不上；畫面會顯示 `unbound`，使用者可調整設定。若實測常
  發生，再開 change 改為 Windows runtime 不分大小寫。
- [多個 Cockpit 實例共用同一狀態檔] → 後寫者覆蓋先寫者。單機單實例是預期用法，README 註明。
- [舊版 cockpit 讀到含 `[[project]]` 的設定檔會啟動失敗]（`deny_unknown_fields`）→ 回滾時需一併移除該區段，README 註明。
- [`/ws` 可被任意網站以 WebSocket 讀取投影（含 pane 標題與 cwd）] → 1b 既有行為，本 change 不擴大也不修；
  寫入端點已有來源檢查。記入交接，建議之後以同一個 middleware 套到 `/ws`。
- [`pointerdown` 觸發比 `click` 容易誤觸] → 按鈕都是可反悔的操作（清除標記、取消改綁）；推進沒有反悔按鈕，
  誤推只能改狀態檔，README 註明。

- [per-pane 狀態訂閱（S）重開時是否也重播舊狀態未驗證] → 沉降重拿只在進入 `Connected` 時做，S 重開不觸發；
  task 7.2 以嚴格時限（啟動與重啟後 6 秒內 `running`）重跑，並觀察建立／關閉 pane 後的狀態是否倒退，若倒退
  記入驗收文件並另議。

## Migration Plan

- 新區段全部選填，既有 `cockpit.toml` 不改就行為不變（`projects` 為空陣列，不讀寫狀態檔）。
- `.gitignore` 加 `cockpit.state.json`；`cockpit.example.toml` 加一個示範 project。
- 回滾：移除設定檔的 `[[project]]`／`[state]` 後換回舊版執行檔；狀態檔可留著（舊版不讀）。
