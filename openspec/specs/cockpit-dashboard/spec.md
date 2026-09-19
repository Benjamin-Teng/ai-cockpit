# cockpit-dashboard Specification

## Purpose

定義 `cockpit` 的 HTTP／WebSocket 服務與畫面：路由、整張圖推送、只綁 loopback、內嵌靜態資源、
PWA、畫面呈現規則與通道重連、Factory Floor 網格與畫面操作。證據：設計文件 §8.1、§8.3、§9、§15、ADR-0004、ADR-0005。

## Requirements

### Requirement: 路由與內嵌資源

系統必須提供 `GET /`（`index.html`）、`GET /app/<檔名>`（`channel.js`、`render.js`、`actions.js`、`style.css`）、
`GET /manifest.webmanifest`、`GET /icons/<檔名>`（192 與 512 px PNG）、`GET /api/state`（目前整張圖
JSON）、`GET /ws`，以及 `pipeline-progress` 定義的寫入端點（`POST /api/projects/<project>/tasks/<task>/<操作>`、
`PUT`／`DELETE /api/projects/<project>/workstreams/<workstream>/override`）；所有靜態內容內嵌在執行檔內；
其他路徑回 404；服務只綁設定的 loopback 位址。

#### Scenario: 路由與 content-type

- **WHEN** 逐一請求 `/`、`/app/render.js`、`/app/actions.js`、`/app/style.css`、`/manifest.webmanifest`、
  `/icons/icon-192.png`、`/api/state`
- **THEN** 皆為 200，content-type 分別為 HTML、JavaScript、JavaScript、CSS、`application/manifest+json`、
  `image/png`、`application/json`

#### Scenario: /api/state 與投影一致

- **GIVEN** 目前投影 version 為 3
- **WHEN** 請求 `/api/state`
- **THEN** 回傳的 JSON `version` 為 3、內容等於投影

#### Scenario: 單一執行檔

- **WHEN** 把執行檔複製到沒有原始碼的目錄執行
- **THEN** 上述路由仍正常

#### Scenario: 寫入端點不接受 GET

- **WHEN** `GET /api/projects/p/tasks/t1/advance`
- **THEN** 回 405，進度不變

### Requirement: WebSocket 推送整張圖

系統必須在客戶端連上 `/ws` 後立即送出目前整張圖（一則文字訊息、完整 JSON），之後每次 `version`
遞增再送一份完整 JSON；不送增量；多個客戶端各自收到；客戶端送來的訊息一律忽略。

#### Scenario: 連上即收到現況

- **GIVEN** 投影 version 3
- **WHEN** 客戶端連上 `/ws`
- **THEN** 第一則訊息的 `version` 為 3，且等於 `/api/state`

#### Scenario: 變動後收到新的一份

- **GIVEN** 客戶端已連上
- **WHEN** 狀態庫變動使 version 變 4
- **THEN** 客戶端收到 `version` 4 的完整 JSON

#### Scenario: 兩個客戶端

- **WHEN** 兩個客戶端同時連上後發生一次變動
- **THEN** 兩者都收到同一份

### Requirement: 畫面整頁重畫

系統必須讓 `channel.js` 只負責連 `/ws`、收到訊息就呼叫 `onState(state)`、斷線後以 1、2、4、8 秒
退避（上限 8 秒）重連，且只在重連後收到第一則訊息時才把退避歸零（連上後立刻被關閉不歸零）；收到無法
解析為 JSON 的訊息時略過該則並在 console 記警告，通道不中斷。`render.js` 每次收到整張圖就整頁重畫：
頂列（產品名稱、通道狀態、`version`）；每個 Project 一塊 Factory Floor（見「Factory Floor」）；每個
runtime 一張卡（`id`、`endpoint`、連線狀態與原因或 protocol 警告、server 版本、最後 snapshot 時間）；卡內
依 workspace 分組（標籤、number、彙總狀態），每個 pane 一列（id、agent 名稱或 `shell`、狀態色塊：
`working` 綠、`blocked` 琥珀、`done` 藍、`idle` 灰、`unknown` 與任何未知字串暗灰、`exited` 加刪除線、
標題、cwd）；頁尾最近事件；深色配色；可點的互動只有「畫面操作」所列的按鈕；pane 的 `done` 顯示為
`done`，不出現「完成」字樣。整頁重畫不得清除進行中的畫面操作狀態（改綁模式、最近一次操作的錯誤訊息）。

#### Scenario: 兩個 runtime 的畫面

- **GIVEN** 服務提供一份含 `win`（connected）與 `wsl`（disconnected，附原因）的投影
- **WHEN** 以瀏覽器開啟 `/`
- **THEN** 出現兩張卡，`wsl` 卡顯示原因；每個 pane 一列且色塊對應狀態；頂列顯示 version

#### Scenario: 未知狀態不破壞畫面

- **GIVEN** 某 pane 的 `agent_status` 為 `whatever`
- **WHEN** 重畫
- **THEN** 該列以暗灰色塊顯示原字串，其他列正常

#### Scenario: 通道重連

- **GIVEN** 頁面已連上
- **WHEN** 服務停止再啟動
- **THEN** 頂列通道狀態先顯示斷線、恢復後顯示已連線並重畫最新一份

#### Scenario: 連上即斷不歸零退避

- **GIVEN** 服務接受 WebSocket 後立即關閉、不送任何訊息，重複發生
- **WHEN** 觀察重連間隔
- **THEN** 間隔依 1、2、4、8、8 秒遞增，不回到 1 秒

#### Scenario: 壞訊息不中斷

- **GIVEN** 頁面已連上
- **WHEN** 收到一則 `{not json`，接著收到一份合法投影
- **THEN** 第一則被略過、console 有警告，第二則正常重畫，通道狀態維持已連線

### Requirement: PWA 可安裝

系統必須提供 manifest（`name`、`short_name`、`start_url` 為 `/`、`display` 為 `standalone`、`icons`
含 192 與 512 px PNG、深色 `background_color`／`theme_color`），並在 `index.html` 以
`<link rel="manifest">` 引用；不提供 service worker。

#### Scenario: manifest 欄位

- **WHEN** 請求 `/manifest.webmanifest`
- **THEN** JSON 含上述欄位，兩個圖示 URL 都能取得且為 PNG（檔頭 `89 50 4E 47`）

#### Scenario: 從 Chrome 安裝

- **WHEN** 以 Chrome 開 `http://127.0.0.1:7770/` 並從選單選「安裝」
- **THEN** 可安裝為獨立視窗的應用程式（手動驗收）

### Requirement: Factory Floor

系統必須在畫面上、runtime 卡之前，依 `projects` 順序為每個 Project 畫一塊區域（上下排列、不做切換選單）：
標題為 Project `name`，有 `warnings` 時逐則顯示；區域內是一張網格，欄為 `stages`（設定順序）、列為
workstreams（設定順序）；每列列首顯示 workstream `name` 與綁定摘要（`bound` 顯示 runtime 與 pane id，
`source` 為 `override` 時加「改綁」標示；`unbound` 顯示「未綁定」；`ambiguous` 顯示「歧義」與候選數；
`runtime_disconnected` 顯示「runtime 未連線」；`none` 顯示「無綁定」）；每個 task 以節點出現在所屬
workstream 列與目前 `stage` 欄交會的格子，同格多個 task 依設定順序排列；節點顯示 `title` 與 `status`，
色彩：`running` 綠、`blocked` 琥珀、`ready` 灰、`pending` 暗灰、`failed` 紅、`completed` 紫，`running`
節點另有動態強調；不畫依賴箭頭。`completed` 不得使用 pane `done` 的藍色。

#### Scenario: Scenario D 的畫面

- **GIVEN** 投影中 Project `p` 的 stages 為 `Plan`、`Implement`、`Test`，workstream `backend`、`frontend`、
  `tests` 各有一個 `running` 的 task，分別在 `Implement`、`Plan`、`Test`
- **WHEN** 以瀏覽器開啟 `/`
- **THEN** 網格有三列三欄，三個綠色節點分別位於（backend, Implement）、（frontend, Plan）、（tests, Test）

#### Scenario: 兩個 Project

- **GIVEN** 投影有 Project `p1`、`p2`
- **WHEN** 重畫
- **THEN** 出現兩塊區域，`p1` 在上、`p2` 在下，都在 runtime 卡之前

#### Scenario: 未知 status 不破壞畫面

- **GIVEN** 某 task 的 `status` 為 `whatever`
- **WHEN** 重畫
- **THEN** 該節點以暗灰色顯示原字串，其他節點正常

### Requirement: 畫面操作

系統必須在畫面上提供下列操作，並以 `pipeline-progress` 的寫入端點送出：task 節點上，`mark` 為 `none` 且
不在最後一個 stage 時顯示「推進」；`mark` 為 `none` 時顯示「Completed」與「Failed」；`mark` 不是 `none`
時只顯示「清除標記」。workstream 列首顯示「改綁」；`binding.source` 為 `override` 時另顯示「取消改綁」。
按「改綁」進入改綁模式：頁面顯示指出目標 workstream 的提示與「取消」，所有 `connected` runtime 卡中未
exited 的 pane 列出現「綁定到這裡」，按下即送出覆蓋，成功或按「取消」即離開改綁模式。寫入端點回非 2xx
或請求失敗時，頁面顯示錯誤訊息（含回應本體的 `error`），直到下一次操作或使用者關閉；操作成功後畫面
不自行修改狀態，一律等 `/ws` 推送的新投影重畫。

#### Scenario: 推進按鈕

- **GIVEN** task `t1` 在 `Plan`（非最後一站）、`mark` 為 `none`
- **WHEN** 在畫面按 `t1` 的「推進」
- **THEN** 服務收到 `POST /api/projects/p/tasks/t1/advance`；新投影到達後節點出現在下一站的欄

#### Scenario: 改綁模式跨重畫保留

- **GIVEN** 已按 workstream `be` 的「改綁」進入改綁模式
- **WHEN** 期間收到兩份新投影並整頁重畫
- **THEN** 改綁提示與各 pane 列的「綁定到這裡」仍在；按其中一列後服務收到對應的 `PUT` 覆蓋請求

#### Scenario: 頻繁重畫時按鈕仍有效

- **GIVEN** 投影每 100 ms 推送一份新的 version
- **WHEN** 連續按 10 次不同 task 的「Completed」
- **THEN** 服務收到 10 個對應的 `POST` 請求，沒有因為重畫而遺失

#### Scenario: 被拒絕時顯示原因

- **GIVEN** 服務對某次操作回 409，本體 `{"error":"已有標記"}`
- **WHEN** 該操作完成
- **THEN** 頁面顯示含「已有標記」的錯誤訊息，之後的重畫不清除它
