# design：live-output

## Context

動機見 `proposal.md`。與本設計有關的現況（2026-09-19 查證）：

- `herdr-client` 已有 `PaneReadParams`／`PaneReadResult`／`PaneReadRequest`（`src/types/pane_read.rs`、
  `src/client/request.rs`），只有序列化測試，沒有呼叫端。真機探測確認兩個版本的回應都能反序列化。
- `AgentRuntime`（`cockpit-core/src/runtime.rs`）只有 `id`／`snapshot`／`subscribe`；`RuntimeError` 只有
  `Unavailable { reason, retry_after }` 與 `Failed(String)`。
- `cockpit/src/app.rs` 把每個 runtime 直接 move 進 `driver::run`，HTTP 層的 `AppState` 拿不到 runtime。
- `/ws` 單向推整份 `ProjectedState`、沒有訊息信封、忽略客戶端訊息（`cockpit/src/http.rs` `handle_socket`）。
- `render.js` 每次投影更新就 `document.getElementById("app").replaceChildren(...)` 整頁重畫；`actions.js` 的
  UI 狀態只有 `rebind` 與 `error`，沒有「選取 pane」。
- `source_check`（`cockpit/src/source_check.rs`）目前只掛在寫入路由；它對沒有 `Origin` 的請求放行。

真機事實一律引用 `docs/research/2026-09-19/pane-read-probe.md`（下稱「探測筆記」），不重新推導：`revision`
恆為 0（第 2 節）；單次讀取 WSL 約 42 ms、Windows 約 1.2 ms、與大小無關（第 1 節）；`recent` 的 `lines`
從畫面格底端往上數且空白列也算、上限 1000（第 3 節）；全螢幕 agent 畫面沒有 scrollback（第 3 節）；
不存在的 pane 回錯誤碼 `pane_not_found`（第 5 節；只驗了 Windows 0.9.0，WSL 0.8.2 由 task 6.1 補驗——
若 WSL 端錯誤碼不同，錯誤對應要跟著改，否則面板會顯示「持續重試」而不是「pane 已不存在」）。

設計文件相關章節：§2.6（`pane.read`）、§6.2（`AgentRuntime` 預定加 `read_output`）、§8.3（畫面）、
§12（change 2、3 的約束）、§13 #4 #5。本 change 修正 §12 的 `revision` 前提（見 D3），§13 #4 同步更新。

## Goals / Non-Goals

**Goals:**

- 後端沒有任何與「誰在看哪個 pane」有關的狀態，也沒有新的背景工作要收尾。
- pane 畫面內容只給本機同源頁面與命令列讀到。
- Live Output 面板的捲動與內容不受每次投影重畫影響。
- 之後加上色只需改讀取參數與前端呈現，不動端點形狀與分層。

**Non-Goals:**

- 見 `proposal.md`「非目標」。設計層面另外排除：行級 diff、回應壓縮、ETag／304（本機 4–5 KB／秒不值得）、
  跨分頁共用讀取結果。

## Decisions

### D1：瀏覽器輪詢一個無狀態 GET 端點，不走 `/ws`

頁面有選取時每秒 `fetch` 一次 `GET /api/runtimes/<runtime>/panes/<pane>/output`；後端收到就即時讀一次回傳。

否決的替代方案（使用者 2026-09-19 在三案中選定本案）：

- **`/ws` 雙向化、每連線一個輪詢迴圈**：要把「整包就是投影」的訊息改成帶 `type` 的信封（破壞性變更，
  `cockpit-dashboard`「WebSocket 推送整張圖」與 `channel.js` 都要改）；每條連線多一個要管生命週期的迴圈；
  而且 `/ws` 沒有來源檢查（handover 第 5 節「已知不修」），輸出走它等於任何網站都讀得到終端機內容，
  得先翻案那一條。
- **後端集中式輸出服務（每個被看的 pane 一個迴圈、廣播給訂閱者）**：讀取次數最省、可對每個 runtime 排隊，
  但要做引用計數與回收。change 2 在「背景工作 detach 洩漏」「持鎖等 `spawn_blocking`」上踩過數個坑
  （handover 第 4 節）；實測讀取成本低到省下的次數不值這些零件。

代價：輸出由「推送」變成「每秒來問」，體感延遲最多約一秒；兩個分頁看同一個 pane 會各讀一次。
瀏覽器對背景分頁的計時器降頻在這裡是好事——沒人看就少讀。

### D2：分層——trait 加一個方法，HTTP 層只認 trait

- `cockpit-core`：`AgentRuntime` 加 `async fn read_output(&self, pane: &PaneId, max_lines: u32) ->
  Result<PaneOutput, RuntimeError>`。`PaneOutput { format: OutputFormat, text: String, truncated: bool }`，
  `OutputFormat` 目前只有 `Text`（序列化為 `"text"`）。`RuntimeError` 加 `PaneNotFound`（帶 pane id）。
  不給預設實作：預設回「不支援」會讓忘記實作的 runtime 在執行期才露餡，編譯期就該失敗。
- `cockpit-herdr`：`HerdrRuntime::read_output` 用既有 connector 開一條連線送 `PaneReadRequest`，與 `snapshot()`
  走同一條 `Client::request` 路徑。`RequestError::Remote { code: "pane_not_found", .. }` → `PaneNotFound`；
  其餘一律 → `Failed(原因)`，與 `snapshot()` 現行做法一致（它把所有請求錯誤都對應成 `Failed`；`Unavailable`
  只用在 `subscribe()` 的 WSL 探測）。`read_output` 不做 WSL 探測、不重試：重試節奏在前端（每秒一次），
  探測會讓每次讀取多啟動一個 `wsl.exe`。
- `cockpit`：`AppState` 加 `runtimes: Arc<HashMap<RuntimeId, Arc<dyn AgentRuntime>>>`。`app.rs` 建好 runtime 後
  clone 一份 `Arc` 進表、原本那份照舊交給 `driver::run`；driver 不改。表在啟動時建立後不再變動，不需要鎖。

依賴方向不變（ADR-0003）：`cockpit` 仍只透過 `cockpit-core` 的 trait 與 `cockpit-herdr` 的 factory 接觸 runtime。

否決：在 `cockpit` 的 handler 直接呼叫 `herdr-client`——違反 ADR-0003，也讓 `ui_preview` 與測試無法用假 runtime。

### D3：不用 `revision`；去重放在前端，用字串相等

`PaneOutput` 刻意不帶 `revision`：它實測恆為 0，放進核心型別只會誘導之後的人依賴它（這正是設計文件 §12 原本
的錯）。後端不做去重——無狀態端點沒有「上一次」可比。前端拿到 `text` 與目前面板內容相同就不動 DOM。

不算雜湊：比較兩個數 KB 的字串比算雜湊便宜，也少一份程式碼。工作中的 agent pane 幾乎每次都不同（探測筆記
結論 5），去重只在 pane 靜止時省 DOM 更新，不是流量控制手段；流量由 1 秒間隔控制。

### D4：讀取參數固定為 `recent`／200 行／`text`

- `recent` 而非 `visible`：對全螢幕 agent 畫面兩者等價；對一般 shell，`recent` 多拿得到往上捲的歷史。
- 200 行：必須大於任何實際的畫面高度。`lines` 從畫面格底端往上數、空白列也算，給太小會讀到一片空白
  （探測筆記第 3 節：39 列的 pane 下半空白時 `lines=5` 回 0 bytes）。200 行純文字約 10–20 KB，可接受。
- 不送 `strip_ansi`：`format=text` 不給它時回應已無控制序列；給 `false` 的行為未驗（探測筆記第 4 節）。
- `max_lines` 由 `cockpit` 以常數 200 傳入而不是寫死在 `cockpit-herdr`：行數是產品決定，不是 HERDR 接合細節。

### D5：同一個 runtime 的輸出讀取排隊

`HerdrRuntime` 持一把 `tokio::sync::Mutex<()>`，`read_output` 整段持有。理由：Windows named pipe 3 條並發會
`ERROR_PIPE_BUSY`（handover 第 4 節；`herdr-client` 以 50 ms 重試、上限 1 秒），driver 已經佔用訂閱與 snapshot
的連線，多個分頁同時讀輸出會自己撞出忙碌重試。單次讀取 1–42 ms，排隊的等待可忽略。

這把鎖只序列化輸出讀取彼此，不與 `snapshot()`／`subscribe()` 互斥——不讓 Live Output 拖慢狀態更新。
持鎖的是呼叫端 future：客戶端斷線時 axum drop handler，鎖隨 future 一起釋放，連線也隨之 drop，沒有 detach
的工作（與 change 2 寫入交易的情況不同：那裡有 `spawn_blocking` 會在 future 被 drop 後繼續跑）。

### D6：端點的錯誤對應與逾時

| 來源 | HTTP |
|---|---|
| `source_check` 不通過 | 403（handler 不執行，不發讀取） |
| 路徑片段解碼後不是合法 UTF-8（axum `Path` rejection，由 handler 攔下走 `error_response`） | 400 |
| runtime id 不在表內 | 404 |
| `RuntimeError::PaneNotFound` | 404 |
| `RuntimeError::Unavailable`、`Failed` | 503 |
| `tokio::time::timeout` 5 秒到期 | 504 |
| `GET` 以外的 method（含 `HEAD`） | 405 |

逾時包在 handler 這一層（含排隊等鎖的時間）：逾時即 drop 讀取 future。5 秒遠大於正常值，只為了不讓一個
卡住的 runtime 把請求永遠掛著。回應帶 `Cache-Control: no-store`（畫面內容不該進任何快取）與
`X-Content-Type-Options: nosniff`（JSON 不被當成腳本或 HTML 嗅探）。

pane id 含冒號（`w1:p1`）：前端以 `encodeURIComponent` 組路徑，axum 的 `Path` 會解碼。

### D7：來源檢查重用 `source_check`，不加 `Sec-Fetch-Site`

以 `route_layer` 把既有 `source_check` 掛到輸出路由。同源 `fetch` 的 `GET` 不帶 `Origin`，middleware 對此放行，
相容。

否決「額外要求 `Sec-Fetch-Site: same-origin`」：跨站頁面即使發得出這個 `GET`（`no-cors`），同源政策也不讓它
讀回應；`Host` 檢查已擋 DNS rebinding；端點唯讀、無副作用，盲發沒有傷害。多一條規則就多一份規格與測試，
而沒有擋到新的攻擊。`source_check.rs` 檔頭註解目前寫「只套在兩個寫入路由」，要一併更新。

### D8：前端——面板在 `#app` 之外，選取狀態放 `actions.js`

- `index.html` 在 `#app` 旁加常駐的 `<section id="output">`；新檔 `output.js` 擁有它。`render.js` 的
  `replaceChildren` 只作用在 `#app`，面板節點不會被換掉，捲動位置自然保留（同時避開 handover 第 5 節 M3）。
- 分工：`output.js` 只管面板與輪詢，對外暴露 `window.liveOutput = { select(runtime, paneId), clear(),
  setKnownPanes(panes) }`，自己不決定「誰被選」。選取狀態 `selected: null | { runtime, paneId }` 放進
  `actions.js` 既有的 `ui` 物件，與 `rebind`、`error` 並列：`render.js` 已經靠 `uiSnapshot()` 在每次重畫時
  讀它，被選 pane 列的標示不需要新機制。`actions.js` 改 `ui.selected` 時同步呼叫 `liveOutput.select`／
  `clear`；面板的「關閉」與「pane 已不存在」則由 `output.js` 回呼 `actions.js` 清掉 `ui.selected`。
  這條線讓輪詢核心可以在選取 UI 接上之前，單獨以 `liveOutput.select(...)` 驗證（task 5.2）。
- 點選用事件委派：pane 列加 `data-action="select-pane"`；workstream 列首的「看輸出」加
  `data-action="select-bound-pane"`。改綁模式下「綁定到這裡」按鈕的點擊不得冒泡成選取。
- `output.js` 輪詢：`setTimeout` 串接（回來後才排下一次），不用 `setInterval`——端點變慢時不堆積。每次換選取
  遞增一個世代序號，回應回來時序號不符就丟棄（change 2 踩過「較慢的舊請求蓋掉較新狀態」）。每次發請求建立一個
  `AbortController`；換選取／關閉／pane 消失時 abort 進行中的請求，換選取並立即對新選取發下一次請求（不等舊
  請求落地，否則慢 pane 或永久 pending 的請求會擋住切換）。前端逾時 6 秒（略大於服務端 5 秒逾時），逾時判定由
  計時器自己觸發、不依賴底層請求是否真的因為 abort 而失敗。abort 造成的失敗不算一次失敗（不標過期、不顯示
  原因、不多排一條輪詢鏈）；世代序號檢查仍保留（G4 fix wave Finding 1／R20——原本「换选取時若已有請求在飛就
  等它結束才補發」會讓慢 pane 擋住改選，違反「3 秒內反映」）。
- 選取不參與 `actions.js` 的操作序號（`latestOp`）與錯誤清除：`select-pane`／`select-bound-pane`（含鍵盤觸發）
  只是切換面板看哪個 pane，不是 spec「畫面操作」定義的寫入按鈕，不得遞增 `latestOp`、不得清 `ui.error`——否則
  按下寫入按鈕之後立刻去選一個 pane 看輸出，那筆寫入稍後才失敗時錯誤訊息會被選取動作悄悄清掉／吞掉（G4 fix
  wave Finding 2／R19）。
- 文字一律 `textContent` 寫入 `<pre>`；不用 `innerHTML`。
- 貼底判定：更新前量 `scrollHeight - scrollTop - clientHeight` 小於數像素才在更新後捲到底。
- `render.js` 每次重畫後以 `liveOutput.setKnownPanes(...)` 交出目前投影中的 pane 集合，用來判定
  「被選 pane 已消失」。

### D9：驗證手段

- 後端行為：`cockpit-herdr` 用假 HERDR（`FakeHerdrConfig::with_method_response("pane.read", …)`）；「同 runtime
  不並發」需要假 HERDR 能延遲回應並回報同時連線數——`herdr-client/src/testing/` 若還做不到，在該 task 內擴充
  測試鷹架。這些測試走真實 named pipe／socket，用真實時間，不用 `start_paused`（handover 第 4 節）。
- 端點：`cockpit` 以假 runtime 測 200／403／404／503／504／405 與標頭；504 用 `start_paused` 是安全的
  （假 runtime 不經真實 transport）。
- 前端：一律 headless Chrome（CDP）腳本（比照 `docs/research/2026-09-16/actions-check.js`，以 `node` 執行），
  不做原始碼字串比對（handover 第 4 節）。`ui_preview` 目前不含任何 runtime（投影是直接組出來的）；本 change
  在 example 內加一個腳本化的假 `AgentRuntime`，只有 `read_output` 有內容（隨時間變化，可用環境變數注入
  延遲、503、404、含 `<script>` 的文字、超過一屏的長文），掛上與正式服務相同的輸出 handler，腳本對它驗
  `live-output` 前端各情境。
- 真機：WSL 測試 pane 跑每秒印一行的迴圈，驗「3 秒內反映」（門檻與 spec 相同，不自訂更嚴）。

## Risks / Trade-offs

- [pane 畫面含 token／密碼，經 HTTP 提供後暴露面變大] → 端點強制 `source_check`；服務只綁 loopback
  （既有）；`no-store`。殘餘風險：本機其他程式本來就能直接連 HERDR socket，Cockpit 沒有讓它更糟。
- [`recent`＋200 行對超高畫面（>200 列）會讀到不完整的一屏] → 實際終端機高度遠小於 200；真有需要再調常數。
- [HERDR 升版後 `pane.read` 行為改變（`revision` 開始遞增、`lines` 語意變）] → 不依賴 `revision`；
  `herdr-client/examples/probe_pane_read.rs` 保留，升版時重跑探測筆記第 2、3 節。
- [每個開著面板的分頁每秒一次讀取，WSL 端每次啟動一個 `wsl.exe`] → 實測 42 ms；使用者關面板或關分頁即停。
  若日後要更高頻率或多 pane 同看，再評估 ADR-0002 方案 B。
- [輸出讀取與 driver 的連線搶 named pipe] → D5 限制輸出讀取同時只有一條；`herdr-client` 既有忙碌重試兜底。
- [加 trait 方法會讓所有 `AgentRuntime` 實作者編譯失敗] → 刻意的（D2）；tasks 把「加方法＋補齊所有實作者」
  放在同一個 task，保持每個 commit 可編譯。

## Migration Plan

純新增，沒有資料或設定遷移。回退＝還原本 change 的 commit；狀態檔、設定檔、`/ws` 格式都沒動過。
