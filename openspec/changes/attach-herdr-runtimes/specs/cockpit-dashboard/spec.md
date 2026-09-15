# cockpit-dashboard（delta）

## Purpose

定義 `cockpit` 的 HTTP／WebSocket 服務與畫面：路由、整張圖推送、只綁 loopback、內嵌靜態資源、
PWA、畫面呈現規則與通道重連。證據：設計文件 §8.1、§8.3、§9、§15、ADR-0004、ADR-0005。

## ADDED Requirements

### Requirement: 路由與內嵌資源

系統必須提供 `GET /`（`index.html`）、`GET /app/<檔名>`（`channel.js`、`render.js`、`style.css`）、
`GET /manifest.webmanifest`、`GET /icons/<檔名>`（192 與 512 px PNG）、`GET /api/state`（目前整張圖
JSON）、`GET /ws`；所有靜態內容內嵌在執行檔內；其他路徑回 404；服務只綁設定的 loopback 位址。

#### Scenario: 路由與 content-type

- **WHEN** 逐一請求 `/`、`/app/render.js`、`/app/style.css`、`/manifest.webmanifest`、
  `/icons/icon-192.png`、`/api/state`
- **THEN** 皆為 200，content-type 分別為 HTML、JavaScript、CSS、`application/manifest+json`、
  `image/png`、`application/json`

#### Scenario: /api/state 與投影一致

- **GIVEN** 目前投影 version 為 3
- **WHEN** 請求 `/api/state`
- **THEN** 回傳的 JSON `version` 為 3、內容等於投影

#### Scenario: 單一執行檔

- **WHEN** 把執行檔複製到沒有原始碼的目錄執行
- **THEN** 上述路由仍正常

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
退避（上限 8 秒）重連；`render.js` 每次收到整張圖就整頁重畫：頂列（產品名稱、通道狀態、
`version`）；每個 runtime 一張卡（`id`、`endpoint`、連線狀態與原因或 protocol 警告、server 版本、
最後 snapshot 時間）；卡內依 workspace 分組（標籤、number、彙總狀態），每個 pane 一列（id、agent
名稱或 `shell`、狀態色塊：`working` 綠、`blocked` 琥珀、`done` 藍、`idle` 灰、`unknown` 與任何未知
字串暗灰、`exited` 加刪除線、標題、cwd）；頁尾最近事件；深色配色；沒有任何可點的互動；`done`
顯示為 `done`，不出現「完成」字樣。

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
