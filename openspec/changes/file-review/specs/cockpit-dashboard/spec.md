# cockpit-dashboard（delta）

## MODIFIED Requirements

### Requirement: 路由與內嵌資源

系統必須提供 `GET /`（`index.html`）、`GET /app/<檔名>`（`channel.js`、`render.js`、`actions.js`、`output.js`、
`files.js`、`viewers.js`、`style.css`）、`GET /manifest.webmanifest`、`GET /icons/<檔名>`（192 與 512 px PNG）、
`GET /vendor/<路徑>`（隨附的第三方前端資源：`pdfjs/` 下的 PDF 函式庫、worker、`cmaps/`、`standard_fonts/` 與該版本
函式庫執行時需要的其他資源檔，`material-icons/` 下的檔案 icon 與主題對照表）、`GET /api/state`（目前整張圖 JSON）、`GET /ws`、`live-output` 定義的
輸出讀取端點（`GET /api/runtimes/<runtime>/panes/<pane>/output`）、`file-review` 定義的檔案端點（根目錄查詢、列目錄、
中繼資料、Markdown 渲染、原始內容），以及 `pipeline-progress` 定義的寫入端點（`POST /api/projects/<project>/tasks/<task>/<操作>`、
`PUT`／`DELETE /api/projects/<project>/workstreams/<workstream>/override`）；所有靜態內容（含 `/vendor/` 下的全部檔案）
內嵌在執行檔內；`/vendor/` 的回應依副檔名給 content-type（`.mjs`／`.js` 為 JavaScript、`.svg` 為 `image/svg+xml`、
`.json` 為 `application/json`、`.wasm` 為 `application/wasm`、其餘為 `application/octet-stream`）並帶 `X-Content-Type-Options: nosniff`；其他路徑回 404；
服務只綁設定的 loopback 位址。

#### Scenario: 路由與 content-type

- **WHEN** 逐一請求 `/`、`/app/render.js`、`/app/actions.js`、`/app/output.js`、`/app/files.js`、`/app/viewers.js`、
  `/app/style.css`、`/manifest.webmanifest`、`/icons/icon-192.png`、`/api/state`
- **THEN** 皆為 200，content-type 分別為 HTML、JavaScript、JavaScript、JavaScript、JavaScript、JavaScript、CSS、
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
「版面與窄視窗」：頂列（產品名稱、每個 runtime 一個連線燈號：runtime `id` 與連線狀態）；左欄 Project 分頁的內容（見
「Project 切換」）；中上 Factory Floor（見「Factory Floor」）；右欄每個 runtime 一張卡（`id`、`endpoint`、連線
狀態與原因或 protocol 警告、server 版本、最後 snapshot 時間），卡內依 workspace 分組（標籤、number、彙總狀態），
每個 pane 一列（id、agent 名稱或 `shell`、agent 狀態、標題、cwd），卡片之下是最近事件；底列（通道狀態、
`version`）。agent 狀態依「Direction 01 視覺語彙」以符號＋文字＋色彩呈現：`working` 用品牌強調色、`blocked`
用警示色、`idle` 用次要文字色、`done` 用主要文字色加品牌強調色的空心標記（**不得**使用成功色，避免被讀成
task 完成）、`unknown` 與任何未知字串用次要文字色並顯示原字串、`exited` 加刪除線。連線狀態：`connected` 用
成功色、`connecting` 用警示色、`disconnected` 用失敗色。可點的互動只有「畫面操作」所列的按鈕、「Project 切換」
的 Project 選取、`live-output`「選定一個 pane」所列的選取操作，與 `file-review` 所列的左欄分頁、檔案樹、檔案分頁與
檢視器內的操作；pane 的 `done` 顯示為 `done`，不出現「完成」字樣。整頁重畫不得清除進行中的畫面操作狀態（改綁模式、
最近一次操作的錯誤訊息）與 Project 選取，也不得清除 Live Output 的選取、面板內容與捲動位置——Live Output 面板、左欄的
分頁列與檔案樹、中欄下半部的分頁區都不屬於整頁重畫的範圍。整頁重畫也不得讓鍵盤焦點消失：
重畫前焦點若在 `#app` 內某個可互動的元素上（pane 列、「畫面操作」的按鈕、「看輸出」、Project 項目等），重畫後
焦點必須落在代表同一個對象、同一個操作的新元素上；該元素在新畫面中已不存在或已不可互動時，焦點才可以離開。整頁重畫不得重置各區塊內部的捲動位置。當頁面與 cockpit
服務的通道（見「通道重連」）不是 `connected`（即 `disconnected` 或 `connecting`）時，頂列每個 runtime 的連線燈號一律改用
`--text-dim`、不使用連線狀態原本對應的顏色，且狀態文字前加「最後已知」（例如「最後已知：connected」）；通道恢復 `connected`
後，燈號才依實際連線狀態還原對應顏色。

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

#### Scenario: 頻繁重畫不影響檔案分頁

- **GIVEN** 已打開一個 Markdown 檔案分頁且往下捲動，投影每 100 ms 推送一份新的 version
- **WHEN** 經過 3 秒
- **THEN** 分頁區與檔案內容的 DOM 節點沒有被換掉，捲動位置不變，目前分頁不變

#### Scenario: 重畫不重置區塊內部捲動位置

- **GIVEN** 視窗寬 ≥1200 且高 ≥720（固定一屏），使用者已把 Factory Floor 與右欄的內部捲動容器都捲到非 0 的位置
- **WHEN** 收到新投影並整頁重畫
- **THEN** 兩個容器的捲動位置都不變，頁面本身也沒有捲動

### Requirement: Direction 01 視覺語彙

系統必須以 `docs/direction-01-visual-design.md` 的色彩表作為畫面唯一的色彩來源：主底色 `#101A2A`、更深背景
`#091320`、表面層 `#142338`、主要文字 `#E5EDF3`、次要文字 `#A3B7C9`、品牌強調 `#63D5E8`、靜態框線 `#294258`、
成功 `#39D5AC`、警示 `#E9BC73`、失敗 `#F47279`；只提供這一套深色配色。凡是以顏色表達的狀態（task status、agent
狀態、連線狀態、Live Output 與檔案分頁的過期標示）都必須同時以文字呈現，task status 另有符號。畫面上所有文字與其實際背景的
對比必須至少 4.5:1；表達狀態所必需的圖形（狀態色條、未知狀態的虛線外框、鍵盤焦點外框）與相鄰背景的對比必須至少
3:1——靜態框線色只能用於裝飾性分隔。介面文字使用系統內建的無襯線字體，時間、id、數值與 Live Output 使用系統內建的
等寬字體且數字等寬；頁面不得為字體發出任何網路請求。整頁重畫不得造成任何可見的動畫或過渡重播；使用者的系統設定為
減少動態（`prefers-reduced-motion: reduce`）時，畫面不得有任何動畫或過渡。可聚焦元素以鍵盤聚焦時必須顯示品牌強調色
的焦點外框。

**檔案內容的例外**：以下三者屬於被檢視的檔案內容而非 Cockpit 介面，不受上述色彩來源、字體與對比規則約束：檔案 icon 圖檔
本身的顏色（`file-review`「檔案 icon」）、PDF 頁面畫出的內容、HTML 檢視器 iframe 內的內容。「不得為字體發出網路請求」同樣
不適用於 PDF 檢視器為畫出 PDF 內容而向本服務 `/vendor/pdfjs/` 讀取的字型資料（仍不得對外部網域發出請求），也不約束 HTML 檢視器 iframe 內的檔案自己發出的請求（該檔案的內容由使用者的 repo
決定）。Markdown 渲染結果、純文字
檢視、分頁列、檔案樹、工具列等 Cockpit 自己畫的部分不在例外內，必須遵守本需求全部規則。

#### Scenario: 文字對比

- **GIVEN** 投影含各種 task status、agent 狀態、連線狀態、warnings 與錯誤訊息
- **WHEN** 對畫面上每個含文字的元素，以其計算後的文字色與實際背景色計算對比
- **THEN** 每一組對比皆不低於 4.5:1

#### Scenario: 狀態不只靠顏色

- **GIVEN** 投影含 `running`、`blocked`、`failed`、`completed` 各一個 task
- **WHEN** 以灰階檢視畫面（或只讀節點的文字內容）
- **THEN** 每個節點都能由符號與 `status` 文字分辨狀態

#### Scenario: 減少動態

- **GIVEN** 瀏覽器模擬 `prefers-reduced-motion: reduce`
- **WHEN** 畫面重畫並滑過、聚焦任一按鈕
- **THEN** 所有元素的計算樣式 `animation-name` 為 `none`，`transition-duration` 為 `0s`

#### Scenario: 不為字體發出網路請求

- **WHEN** 載入 `/` 並等待首份投影畫完
- **THEN** 頁面發出的請求只有本服務自己的路徑，沒有任何字體檔或外部網域的請求

#### Scenario: Markdown 檢視遵守色彩與對比

- **GIVEN** 已打開一個含標題、段落、連結、表格、行內程式碼與程式碼區塊的 Markdown 檔案分頁
- **WHEN** 對內容區每個含文字的元素計算對比，並檢查其計算後的文字色與背景色
- **THEN** 每一組對比皆不低於 4.5:1，所有顏色都來自上述色彩表

### Requirement: 版面與窄視窗

系統必須以三欄版面呈現：頂列與底列橫跨全寬；左欄頂端為「Project」「檔案」兩個分頁，其下顯示目前分頁的內容（Project 清單
或檔案樹）；中欄上為 Factory Floor、下為分頁區（第一個分頁為 Live Output，其後為檔案分頁，見 `file-review`「檔案分頁」）；
右欄上為 runtime 卡、下為最近事件；錯誤訊息與改綁提示顯示在中欄上方。視窗寬度至少 1200 CSS px **且**高度至少 720 CSS px 時，
頁面高度等於視窗高度、不整頁捲動，內容超出的區域各自內部捲動。視窗寬度至少 1200 CSS px 但高度小於 720 CSS px 時，
仍維持三欄排列，但取消固定高度、允許整頁捲動。視窗寬度小於 1200 且至少 760 時，右欄移到中欄下方，取消固定高度、
允許整頁捲動。視窗寬度小於 760 時，所有區域排成單欄並允許整頁捲動。任何寬度或高度下頁面都不得出現橫向捲軸；過長的
workstream 名稱、task 標題、pane 標題、cwd、檔名與分頁名稱不得溢出其所屬區域（以換行或省略呈現，完整內容可由 `title`
屬性取得）。

#### Scenario: 桌面寬度不整頁捲動

- **GIVEN** 視窗 1536×1024，投影含兩個 runtime、各 5 個 pane、30 筆最近事件
- **WHEN** 重畫
- **THEN** `document.documentElement.scrollHeight` 不大於視窗高度；頂列、左欄、Factory Floor、下半部分頁區、runtime 卡、
  最近事件、底列都在視窗內可見；右欄內容超出時右欄內部可捲動

#### Scenario: 寬但矮的視窗

- **GIVEN** 視窗 1280×650
- **WHEN** 重畫
- **THEN** 畫面仍維持三欄排列，頁面可整頁捲動，沒有橫向捲軸；Factory Floor 高度不小於 240px、下半部分頁區高度不小於 320px

#### Scenario: 中等寬度

- **GIVEN** 視窗寬 1100
- **WHEN** 重畫
- **THEN** runtime 卡位於 Factory Floor 與下半部分頁區的下方，頁面可整頁捲動，沒有橫向捲軸

#### Scenario: 窄視窗單欄

- **GIVEN** 視窗寬 700
- **WHEN** 重畫
- **THEN** 各區域由上往下單欄排列，頁面可整頁捲動，沒有橫向捲軸

#### Scenario: 長名稱不溢出

- **GIVEN** 某 workstream 名稱與某 pane 的 cwd 各長 200 個字元
- **WHEN** 重畫
- **THEN** 兩者都沒有超出所屬區域的邊界，頁面沒有橫向捲軸

#### Scenario: 分頁很多不撐破頁面

- **GIVEN** 視窗寬 1280，已打開 20 個檔名各長 60 個字元的檔案分頁
- **WHEN** 重畫
- **THEN** 分頁列在內部橫向捲動，頁面沒有橫向捲軸，中欄寬度不變
