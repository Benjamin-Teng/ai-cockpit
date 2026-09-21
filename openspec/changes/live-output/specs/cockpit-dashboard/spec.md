# cockpit-dashboard（delta）

## MODIFIED Requirements

### Requirement: 路由與內嵌資源

系統必須提供 `GET /`（`index.html`）、`GET /app/<檔名>`（`channel.js`、`render.js`、`actions.js`、`output.js`、
`style.css`）、`GET /manifest.webmanifest`、`GET /icons/<檔名>`（192 與 512 px PNG）、`GET /api/state`（目前整張圖
JSON）、`GET /ws`、`live-output` 定義的輸出讀取端點（`GET /api/runtimes/<runtime>/panes/<pane>/output`），
以及 `pipeline-progress` 定義的寫入端點（`POST /api/projects/<project>/tasks/<task>/<操作>`、
`PUT`／`DELETE /api/projects/<project>/workstreams/<workstream>/override`）；所有靜態內容內嵌在執行檔內；
其他路徑回 404；服務只綁設定的 loopback 位址。

#### Scenario: 路由與 content-type

- **WHEN** 逐一請求 `/`、`/app/render.js`、`/app/actions.js`、`/app/output.js`、`/app/style.css`、
  `/manifest.webmanifest`、`/icons/icon-192.png`、`/api/state`
- **THEN** 皆為 200，content-type 分別為 HTML、JavaScript、JavaScript、JavaScript、CSS、
  `application/manifest+json`、`image/png`、`application/json`

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

### Requirement: 畫面整頁重畫

系統必須讓 `channel.js` 只負責連 `/ws`、收到訊息就呼叫 `onState(state)`、斷線後以 1、2、4、8 秒
退避（上限 8 秒）重連，且只在重連後收到第一則訊息時才把退避歸零（連上後立刻被關閉不歸零）；收到無法
解析為 JSON 的訊息時略過該則並在 console 記警告，通道不中斷。`render.js` 每次收到整張圖就整頁重畫：
頂列（產品名稱、通道狀態、`version`）；每個 Project 一塊 Factory Floor（見「Factory Floor」）；每個
runtime 一張卡（`id`、`endpoint`、連線狀態與原因或 protocol 警告、server 版本、最後 snapshot 時間）；卡內
依 workspace 分組（標籤、number、彙總狀態），每個 pane 一列（id、agent 名稱或 `shell`、狀態色塊：
`working` 綠、`blocked` 琥珀、`done` 藍、`idle` 灰、`unknown` 與任何未知字串暗灰、`exited` 加刪除線、
標題、cwd）；頁尾最近事件；深色配色；可點的互動只有「畫面操作」所列的按鈕與 `live-output`「選定一個 pane」
所列的選取操作；pane 的 `done` 顯示為 `done`，不出現「完成」字樣。整頁重畫不得清除進行中的畫面操作狀態
（改綁模式、最近一次操作的錯誤訊息），也不得清除 Live Output 的選取、面板內容與捲動位置——Live Output 面板
不屬於整頁重畫的範圍。整頁重畫也不得讓鍵盤焦點消失：重畫前焦點若在 `#app` 內某個可互動的元素上（pane 列、「畫面操作」的按鈕、
「看輸出」等），重畫後焦點必須落在代表同一個對象、同一個操作的新元素上；該元素在新畫面中已不存在或已不可互動時，焦點才可以離開。

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

#### Scenario: 重畫不丟鍵盤焦點

- **GIVEN** 焦點在某個 task 節點的「推進」按鈕上，投影每 100 ms 推送一份新的 version
- **WHEN** 經過 2 秒後按 Enter
- **THEN** 這段期間焦點始終在該 task 的「推進」按鈕上（節點可以是新的），按 Enter 後服務收到該 task 的 `advance` 請求

#### Scenario: 頻繁重畫不影響 Live Output 面板

- **GIVEN** 已選定一個 pane、面板內容超過一屏且使用者已往上捲，投影每 100 ms 推送一份新的 version
- **WHEN** 經過 3 秒
- **THEN** 面板的 DOM 節點沒有被換掉，捲動位置不變，選定標示仍在
