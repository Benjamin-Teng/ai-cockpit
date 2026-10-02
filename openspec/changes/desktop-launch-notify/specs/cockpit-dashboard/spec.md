# cockpit-dashboard（delta）

## MODIFIED Requirements

### Requirement: 路由與內嵌資源

系統必須提供 `GET /`（`index.html`）、`GET /app/<檔名>`（`channel.js`、`render.js`、`actions.js`、`output.js`、
`files.js`、`viewers.js`、`git.js`、`notify.js`、`style.css`）、`GET /manifest.webmanifest`、`GET /icons/<檔名>`（192 與 512 px PNG）、
`GET /vendor/<路徑>`（隨附的第三方前端資源：`pdfjs/` 下的 PDF 函式庫、worker、`cmaps/`、`standard_fonts/` 與該版本
函式庫執行時需要的其他資源檔，`material-icons/` 下的檔案 icon 與主題對照表）、`GET /api/state`（目前整張圖 JSON）、`GET /ws`、`live-output` 定義的
輸出讀取端點（`GET /api/runtimes/<runtime>/panes/<pane>/output`）、`file-review` 定義的檔案端點（根目錄查詢、列目錄、
中繼資料、Markdown 渲染、原始內容）、`git-review` 定義的 git 端點（`GET /api/git/<runtime>/<root_id>/…`），以及 `pipeline-progress` 定義的寫入端點（`POST /api/projects/<project>/tasks/<task>/<操作>`、
`PUT`／`DELETE /api/projects/<project>/workstreams/<workstream>/override`）；所有靜態內容（含 `/vendor/` 下的全部檔案）
內嵌在執行檔內；`/vendor/` 的回應依副檔名給 content-type（`.mjs`／`.js` 為 JavaScript、`.svg` 為 `image/svg+xml`、
`.json` 為 `application/json`、`.wasm` 為 `application/wasm`、其餘為 `application/octet-stream`）並帶 `X-Content-Type-Options: nosniff`；其他路徑回 404；
服務只綁設定的 loopback 位址。

#### Scenario: 路由與 content-type

- **WHEN** 逐一請求 `/`、`/app/render.js`、`/app/actions.js`、`/app/output.js`、`/app/files.js`、`/app/viewers.js`、
  `/app/git.js`、`/app/notify.js`、`/app/style.css`、`/manifest.webmanifest`、`/icons/icon-192.png`、`/api/state`
- **THEN** 皆為 200，content-type 分別為 HTML、JavaScript、JavaScript、JavaScript、JavaScript、JavaScript、JavaScript、JavaScript、CSS、
  `application/manifest+json`、`image/png`、`application/json`

#### Scenario: vendored 資源

- **WHEN** 請求 PDF 函式庫主檔、其 worker、任一 `cmaps/` 檔、`/vendor/material-icons/` 下的預設檔案 icon，以及
  `/vendor/material-icons/不存在.svg`
- **THEN** 前四者為 200，content-type 分別為 JavaScript、JavaScript、`application/octet-stream`、`image/svg+xml`，皆帶
  `X-Content-Type-Options: nosniff`；最後一個為 404

#### Scenario: /api/state 與投影一致

- **GIVEN** 目前投影 version 為 3
- **WHEN** 請求 `/api/state`
- **THEN** 回傳的 JSON `version` 為 3、內容等於投影

#### Scenario: 單一執行檔

- **WHEN** 把執行檔複製到沒有原始碼的目錄執行
- **THEN** 上述路由（含 `/vendor/` 下的資源）仍正常

#### Scenario: 寫入端點不接受 GET

- **WHEN** `GET /api/projects/p/tasks/t1/advance`
- **THEN** 回 405，進度不變

### Requirement: 畫面整頁重畫

系統必須讓 `channel.js` 只負責連 `/ws`、收到訊息就呼叫 `onState(state)`、斷線後以 1、2、4、8 秒
退避（上限 8 秒）重連，且只在重連後收到第一則訊息時才把退避歸零（連上後立刻被關閉不歸零）；收到無法
解析為 JSON 的訊息時略過該則並在 console 記警告，通道不中斷。`render.js` 每次收到整張圖就整頁重畫，版面依
「版面與窄視窗」：頂列（產品名稱、每個 runtime 一個連線燈號：runtime `id` 與連線狀態、`desktop-notifications`「通知設定」的鈴鐺按鈕）；左欄 Project 分頁的內容（見
「Project 切換」）；中上 Factory Floor（見「Factory Floor」）；右欄每個 runtime 一張卡（`id`、`endpoint`、連線
狀態與原因或 protocol 警告、server 版本、最後 snapshot 時間），卡內依 workspace 分組（標籤、number、彙總狀態），
每個 pane 一列（id、agent 名稱或 `shell`、agent 狀態、標題、cwd），卡片之下是最近事件；底列（通道狀態、
`version`）。agent 狀態依「Direction 01 視覺語彙」以符號＋文字＋色彩呈現：`working` 用品牌強調色、`blocked`
用警示色、`idle` 用次要文字色、`done` 用主要文字色加品牌強調色的空心標記（**不得**使用成功色，避免被讀成
task 完成）、`unknown` 與任何未知字串用次要文字色並顯示原字串、`exited` 加刪除線。連線狀態：`connected` 用
成功色、`connecting` 用警示色、`disconnected` 用失敗色。可點的互動只有「畫面操作」所列的按鈕、「Project 切換」
的 Project 選取、`live-output`「選定一個 pane」所列的選取操作，與 `file-review` 所列的左欄分頁、檔案樹、檔案分頁與
檢視器內的操作，以及 `desktop-notifications`「通知設定」的鈴鐺按鈕與設定面板（鈴鐺不屬於「畫面操作」，按下不改變其錯誤訊息與進行中的操作狀態）；pane 的 `done` 顯示為 `done`，不出現「完成」字樣。整頁重畫不得清除進行中的畫面操作狀態（改綁模式、
最近一次操作的錯誤訊息）與 Project 選取，也不得清除 Live Output 的選取、面板內容與捲動位置——Live Output 面板、左欄的
分頁列與檔案樹、中欄下半部的分頁區、通知設定面板都不屬於整頁重畫的範圍。整頁重畫也不得讓鍵盤焦點消失：
重畫前焦點若在 `#app` 內某個可互動的元素上（pane 列、「畫面操作」的按鈕、「看輸出」、Project 項目、通知鈴鐺等），重畫後
焦點必須落在代表同一個對象、同一個操作的新元素上；該元素在新畫面中已不存在或已不可互動時，焦點才可以離開。焦點還原後是否
呈現焦點外框，依使用者最近一次的輸入方式決定：最近一次是滑鼠（pointer）操作（含該操作觸發的重畫與其後的背景重畫）時，
焦點仍必須還原到新元素上，但該元素不得呈現焦點外框（不匹配 `:focus-visible`）；最近一次是鍵盤操作時，焦點還原且焦點外框
照常呈現（見「Direction 01 視覺語彙」）。滑鼠操作之後使用者改用鍵盤（例如按 Tab）移動焦點時，焦點外框照常出現。
「最近一次輸入方式」另有兩條例外，比照瀏覽器原生的 `:focus-visible` 判定：(1) 帶 Alt、Ctrl 或 Meta 的按鍵（含單按
這些鍵）不改變「最近一次輸入方式」，Shift 與一般按鍵才算鍵盤操作；(2) 滑鼠操作沒有移動焦點（例如按住捲軸）時，原本呈現
焦點外框的元素在其後的重畫仍照常呈現焦點外框；在畫面元素上以滑鼠觸發的操作（會移動或重設焦點的點擊）一律不呈現。整頁重畫不得重置
各區塊內部的捲動位置。當頁面與 cockpit
服務的通道（見「通道重連」）不是 `connected`（即 `disconnected` 或 `connecting`）時，畫面上來自投影的即時狀態一律改為
「最後已知」的非即時呈現：頂列每個 runtime 的連線燈號一律改用 `--text-dim`、不使用連線狀態原本對應的顏色，且狀態文字前加
「最後已知」（例如「最後已知：connected」）；右欄每張 runtime 卡的連線狀態（符號與文字）同樣改用 `--text-dim`、不使用連線
狀態原本對應的顏色，且連線狀態文字前加「最後已知」；右欄 runtime 卡內 workspace、tab 與 pane 列的 agent 狀態（符號與文字），
中欄 Factory Floor 內以狀態色表達的元素（task 節點的狀態色條、狀態文字與 `running` 節點的強調外框、workstream 列首的
「未宣告 task」提示），以及左欄 project 項目的狀態計數，一律改用 `--text-dim`
（狀態文字與符號本身仍保留，只是不再用狀態色）；這些非即時呈現仍須符合「Direction 01 視覺語彙」的文字對比。通道恢復
`connected` 後，上述各處才依實際狀態還原對應顏色與文字。

#### Scenario: 兩個 runtime 的畫面

- **GIVEN** 服務提供一份含 `win`（connected）與 `wsl`（disconnected，附原因）的投影
- **WHEN** 以瀏覽器開啟 `/`
- **THEN** 頂列出現兩個連線燈號（`win` 為成功色、`wsl` 為失敗色，皆附文字）；右欄出現兩張卡，`wsl` 卡顯示原因；
  每個 pane 一列且 agent 狀態的符號、文字與色彩對應狀態；底列顯示 version

#### Scenario: done 不使用成功色

- **GIVEN** 某 pane 的 `agent_status` 為 `done`，某 task 的 `status` 為 `completed`
- **WHEN** 重畫
- **THEN** 該 pane 列顯示 `done` 文字，其狀態文字的計算顏色不等於成功色，也不等於 `completed` 節點狀態文字的
  計算顏色；頁面任何位置不出現「完成」字樣

#### Scenario: 未知狀態不破壞畫面

- **GIVEN** 某 pane 的 `agent_status` 為 `whatever`
- **WHEN** 重畫
- **THEN** 該列以次要文字色顯示原字串，其他列正常

#### Scenario: 通道重連

- **GIVEN** 頁面已連上
- **WHEN** 服務停止再啟動
- **THEN** 底列通道狀態先顯示斷線、恢復後顯示已連線並重畫最新一份

#### Scenario: 通道斷線時 runtime 燈號標示為最後已知

- **GIVEN** 頂列顯示 `win` 為 connected
- **WHEN** 頁面與 cockpit 服務的通道斷線
- **THEN** 頂列 `win` 的燈號文字含「最後已知」，且不使用成功色；通道恢復後還原

#### Scenario: 通道斷線時 runtime 卡顯示最後已知

- **GIVEN** 右欄 `win` 的 runtime 卡顯示連線狀態 `connected`（成功色）
- **WHEN** 頁面與 cockpit 服務的通道斷線
- **THEN** `win` 卡的連線狀態文字含「最後已知」且仍可讀到 `connected`，其計算顏色為 `--text-dim`、不是成功色；
  通道恢復 `connected` 後，該卡回到「最後已知」字樣消失、連線狀態為成功色

#### Scenario: 通道斷線時中欄與 pane 列的狀態色轉暗

- **GIVEN** 某 pane 的 `agent_status` 為 `working`，綁定它的 task 節點狀態為 `running`
- **WHEN** 頁面與 cockpit 服務的通道斷線
- **THEN** 該 pane 列的 agent 狀態符號與文字、該 task 節點的狀態色條與狀態文字，計算顏色皆為 `--text-dim`、不是品牌強調色；
  狀態文字仍為 `working`／`running`；左欄該 project 的狀態計數與任何「未宣告 task」提示的計算顏色也為 `--text-dim`；
  按鈕（含「取消改綁」）不轉暗、仍可操作；通道恢復 `connected` 後各處還原為原本的狀態色

#### Scenario: 斷線中整頁重畫後仍為最後已知

- **GIVEN** 頁面與 cockpit 服務的通道已斷線，上述各處已轉為 `--text-dim`
- **WHEN** 使用者點選某個 pane 列觸發整頁重畫
- **THEN** 重畫後右欄 runtime 卡仍標「最後已知」，pane 列與 task 節點的狀態色仍為 `--text-dim`

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

#### Scenario: 滑鼠點擊後重畫不呈現焦點外框

- **GIVEN** 右欄某 pane 列，投影每 100 ms 推送一份新的 version
- **WHEN** 以滑鼠點該 pane 列，並經過 1 秒讓整頁重畫數次
- **THEN** 焦點仍在代表同一個 pane 的新 pane 列上，且該元素不匹配 `:focus-visible`、沒有焦點外框

#### Scenario: 鍵盤觸發的重畫保留焦點外框

- **GIVEN** 以鍵盤 Tab 把焦點移到某 task 節點的「推進」按鈕（外框可見）
- **WHEN** 按 Enter 觸發推進，服務回 204、新投影到達並重畫
- **THEN** 焦點在新畫面中代表同一個 task 的按鈕（或該 task 在新畫面中仍可互動的對應按鈕）上，且該元素匹配 `:focus-visible`、焦點外框可見

#### Scenario: 滑鼠點擊之後按 Tab 外框照常出現

- **GIVEN** 以滑鼠點了某 pane 列，焦點外框不可見
- **WHEN** 按 Tab 把焦點移到下一個可互動元素
- **THEN** 該元素匹配 `:focus-visible`，焦點外框照常出現

#### Scenario: 修飾鍵不改變最近一次輸入方式

- **GIVEN** 以滑鼠點了某 pane 列，焦點外框不可見，投影每 100 ms 推送一份新的 version
- **WHEN** 單按 Alt、Ctrl（不接其他鍵），並經過 1 秒讓整頁重畫數次
- **THEN** 焦點仍在代表同一個 pane 的新 pane 列上，且該元素不匹配 `:focus-visible`、焦點外框不出現

#### Scenario: 不移動焦點的滑鼠操作不洗掉焦點外框

- **GIVEN** 以鍵盤 Tab 把焦點移到某 task 節點的「推進」按鈕（外框可見），投影每 100 ms 推送一份新的 version
- **WHEN** 以滑鼠按住一個不會移動焦點的位置（例如可捲容器的捲軸），並經過 1 秒讓整頁重畫數次
- **THEN** 焦點仍在代表同一個 task 的「推進」按鈕上，且該元素匹配 `:focus-visible`、焦點外框仍在

#### Scenario: 頻繁重畫不影響 Live Output 面板

- **GIVEN** 已選定一個 pane、面板內容超過一屏且使用者已往上捲，投影每 100 ms 推送一份新的 version
- **WHEN** 經過 3 秒
- **THEN** 面板的 DOM 節點沒有被換掉，捲動位置不變，選定標示仍在

#### Scenario: 頻繁重畫不影響檔案分頁

- **GIVEN** 已打開一個 Markdown 檔案分頁且往下捲動，投影每 100 ms 推送一份新的 version
- **WHEN** 經過 3 秒
- **THEN** 分頁區與檔案內容的 DOM 節點沒有被換掉，捲動位置不變，目前分頁不變

#### Scenario: 重畫不重置區塊內部捲動位置

- **GIVEN** 視窗寬 ≥1200 且高 ≥720（固定一屏），使用者已把 Factory Floor 與右欄的內部捲動容器都捲到非 0 的位置
- **WHEN** 收到新投影並整頁重畫
- **THEN** 兩個容器的捲動位置都不變，頁面本身也沒有捲動

#### Scenario: 鈴鐺按鈕跨重畫保留焦點

- **GIVEN** 以鍵盤 Tab 把焦點移到頂列的通知鈴鐺，投影每 100 ms 推送一份新的 version
- **WHEN** 經過 1 秒
- **THEN** 焦點仍在（新的）鈴鐺按鈕上且焦點外框可見；按 Enter 開啟通知設定面板

#### Scenario: 鈴鐺不影響畫面操作的錯誤訊息

- **GIVEN** 最近一次畫面操作失敗、錯誤訊息顯示中
- **WHEN** 按通知鈴鐺開啟再關閉設定面板
- **THEN** 錯誤訊息仍在
