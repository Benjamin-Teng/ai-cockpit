# live-output（change 3）

## Why

Cockpit 目前只看得到「哪個 pane 在動」，看不到 agent 在講什麼；要知道內容仍得切回 HERDR 逐個 pane 看。
MVP 第三片（設計文件 §1 切片表 change 3、`docs/cockpit-spec.md` §12、§20 item 11、Scenario E）要求：
點擊 pane 或 task 後，畫面顯示該 HERDR pane 的輸出。協定層（`pane.read` 型別與 request）在 change 1a
已備好但從未有呼叫端；2026-09-19 的真機探測（`docs/research/2026-09-19/pane-read-probe.md`）確認它
可用、便宜，並推翻了原設計的一個前提，現在可以動手。

## What Changes

- `cockpit-core` 的 `AgentRuntime` 抽象新增「讀取某個 pane 的輸出」能力，回傳純文字、格式標記與
  「是否還有更早內容未回傳」；新增「pane 不存在」的錯誤類別。
- `cockpit-herdr` 以既有的 `pane.read` request 實作該能力：固定讀最近 200 行純文字；同一個 runtime
  同時只放行一個輸出讀取。對 HERDR 送出的 method 由兩個（`session.snapshot`、`events.subscribe`）
  變成三個，仍全部唯讀。
- `cockpit` 新增唯讀端點 `GET /api/runtimes/<runtime>/panes/<pane>/output`，後端無狀態、每次請求即時讀一次；
  套用與寫入端點相同的本機同源檢查（pane 畫面可能含敏感內容）。
- 畫面新增 Live Output 面板：點 pane 列、或點已綁定 workstream 的「看輸出」選定一個 pane，頁面每秒向上述
  端點取一次並顯示；面板不在整頁重畫範圍內，捲動位置不被投影更新洗掉。
- **不依賴 `revision`**：實測 `pane.read` 回應的 `revision` 在 HERDR 0.8.2（WSL）與 0.9.0（Windows）都恆為 0
  （研究筆記第 2 節），設計文件 §12「以 `revision` 比對後才推送」的前提不成立；本 change 改由前端比對文字。
- 文件修正：ADR-0002 Consequences 補實測延遲（WSL 約 42 ms、Windows 約 1.2 ms，研究筆記第 1 節），
  結掉「WSL 端每秒多次 `pane.read` 是否改方案 B」這條未決事項；設計文件 §12、§13 註明 `revision` 前提已被推翻。

沒有 **BREAKING** 變更：`/ws` 訊息格式、`/api/state`、既有寫入端點、設定檔、狀態檔皆不變。

## Capabilities

### New Capabilities

- `live-output`: pane 輸出的讀取端點（路徑、回應格式、錯誤對應、本機同源檢查、逾時）與畫面上 Live Output
  面板的行為（選取、輪詢、過期回應丟棄、捲動、各種失敗狀態的呈現、內容一律當純文字）。

### Modified Capabilities

- `runtime-model`: 「AgentRuntime 抽象」新增讀取 pane 輸出的能力與「pane 不存在」錯誤。
- `herdr-runtime-session`: 「只用兩個 method」改為三個唯讀 method；新增「讀取 pane 輸出」需求
  （固定參數、錯誤對應、同 runtime 不並發）。
- `herdr-observer-types`: 「pane.read 型別」補上「`revision` 在已測版本恆為 0、呼叫端不得依賴」，
  並新增以真機回應 fixture 驗證可解析的情境。
- `cockpit-dashboard`: 「路由與內嵌資源」加入輸出端點與 `output.js`；「畫面整頁重畫」允許 `live-output`
  定義的選取互動，並要求重畫不得清除選取與面板狀態。

## 非目標

| 不做 | 依據 |
|---|---|
| 上色（`format=ansi`）、ANSI 解析 | 使用者 2026-09-19 決定第一版純文字、回應帶 `format` 欄位預留；設計文件 §13 #5；留給 Direction 01 視覺改版（`docs/direction-01-visual-design.md`） |
| 篩選、搜尋、錯誤詳細、底部輸入區 | 概念圖中這些只表達視覺排列（`docs/direction-01-visual-design.md` 「概念與產品功能界線」） |
| 同時顯示多個 pane 的輸出 | `CONTEXT.md`：Live Output 是「選定 pane 的輸出投影」 |
| 超過 200 行的歷史、分頁往回讀 | `pane.read` 沒有 offset／cursor（設計文件 §2.6）；全螢幕 agent 畫面沒有 scrollback（研究筆記第 3 節） |
| 對 pane 輸入、任何寫入 HERDR 的 method | 設計文件 §12、ADR-0001；`AGENTS.md` 硬性約束 |
| 後端主動推送輸出、`/ws` 雙向化、集中式輸出服務 | 使用者 2026-09-19 選定「瀏覽器每秒來問」；理由見 `design.md` D1 |
| `/ws` 的來源檢查 | handover 第 5 節「已知不修」；輸出不走 `/ws`，風險未因本 change 變大 |
| 可設定的輪詢間隔與行數 | 先寫死 1 秒／200 行；有需求再開設定 |
| Usage 面板、LLM 摘要 | 設計文件 §12；`docs/cockpit-spec.md` Scenario E「不啟動第二個 LLM summarizer」 |
| Direction 01 視覺改版 | handover 第 3 節：另開 change |
| ADR-0002 方案 B（自寫 Linux relay） | 實測每秒一次遠低於門檻（研究筆記第 1 節） |

## Impact

- 程式碼：`cockpit-core/src/runtime.rs`（trait 與錯誤型別）與所有 `AgentRuntime` 實作者（`HerdrRuntime`、
  `cockpit-core/tests/common` 的 `FakeRuntime`）；`cockpit/examples/ui_preview.rs`（新增腳本化假 runtime 供
  前端驗收）；`cockpit-herdr/src/runtime.rs`；
  `cockpit/src/http.rs`、`app.rs`；`cockpit/assets/`（新增 `app/output.js`；修改 `index.html`、
  `app/render.js`、`app/actions.js`、`app/style.css`）。`herdr-client` 只新增一份真機 fixture 與合約測試，
  型別不變。
- API：新增一個唯讀 GET 端點；其餘不變。
- 依賴：不新增 crate。依賴方向不變（ADR-0003：`cockpit-core` 不依賴 `herdr-client`，`cockpit` 不直接
  依賴 `herdr-client`）。
- HERDR：多送一種唯讀 method（`pane.read`），每個開著 Live Output 的瀏覽器分頁每秒一次。
- 安全：pane 畫面內容首次經由 HTTP 提供，端點必須通過本機同源檢查。
- 文件：ADR-0002、設計文件 §12／§13、`CONTEXT.md`、`README.md`、`docs/handover.md`。
