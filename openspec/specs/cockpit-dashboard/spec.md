# cockpit-dashboard Specification

## Purpose

定義 `cockpit` 的 HTTP／WebSocket 服務與畫面：路由、整張圖推送、只綁 loopback、內嵌靜態資源、
PWA、畫面呈現規則與通道重連、Factory Floor 網格與畫面操作、pane 選定與重畫時保留鍵盤焦點、
Direction 01 視覺語彙與三欄版面（含 Project 切換）。證據：設計文件 §8.1、§8.3、§9、§15、ADR-0004、ADR-0005。

## Requirements

### Requirement: 路由與內嵌資源

系統必須提供 `GET /`（`index.html`）、`GET /app/<檔名>`（`i18n.js`、`channel.js`、`render.js`、`actions.js`、`output.js`、
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

- **WHEN** 逐一請求 `/`、`/app/i18n.js`、`/app/render.js`、`/app/actions.js`、`/app/output.js`、`/app/files.js`、`/app/viewers.js`、
  `/app/git.js`、`/app/notify.js`、`/app/style.css`、`/manifest.webmanifest`、`/icons/icon-192.png`、`/api/state`
- **THEN** 皆為 200，content-type 分別為 HTML、JavaScript、JavaScript、JavaScript、JavaScript、JavaScript、JavaScript、JavaScript、JavaScript、CSS、
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
解析為 JSON 的訊息時略過該則並在 console 記警告，通道不中斷。`render.js` 每次收到整張圖就整頁重畫，版面依
「版面與窄視窗」：頂列（產品名稱、每個 runtime 一個連線燈號：runtime `id` 與連線狀態、`desktop-notifications`「通知設定」的鈴鐺按鈕）；左欄 Project 分頁的內容（見
「Project 切換」）；中上 Factory Floor（見「Factory Floor」）；右欄每個 runtime 一張卡（`id`、`endpoint`、連線
狀態與原因或 protocol 警告、server 版本、最後 snapshot 時間），卡內依 workspace 分組（標籤、number、彙總狀態），
每個 pane 一列（id、agent 名稱或 `shell`、agent 狀態、標題、cwd），卡片之下是最近事件；底列（通道狀態、
程式版本號，例如 `v0.1.2`，取自 `cockpit` crate 版本；投影的 `version` 不顯示）。agent 狀態依「Direction 01 視覺語彙」以符號＋文字＋色彩呈現：`working` 用品牌強調色、`blocked`
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
  每個 pane 一列且 agent 狀態的符號、文字與色彩對應狀態；底列顯示程式版本號（`v` 加 crate 版本）

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

系統必須在中上區域畫出目前選定 Project（見「Project 切換」）的 Factory Floor：標題為 Project `name`，有
`warnings` 時逐則顯示；區域內是一張網格，欄為 `stages`（設定順序）、列為 workstreams（設定順序）；每列列首顯示
workstream `name` 與綁定摘要（`bound` 顯示 runtime 與 pane id，`source` 為 `override` 時加「改綁」標示；`unbound`
顯示「未綁定」；`ambiguous` 顯示「歧義」與候選數；`runtime_disconnected` 顯示「runtime 未連線」，`source` 為 `override`
時同樣加「改綁」標示；`none` 顯示「無綁定」）；workstream 的 `activity_undeclared` 為 `true` 時，列首另顯示文字提示「工作中・未宣告 task」（以警示色
呈現、不得有動畫），為 `false` 時不顯示；每個 task 以節點出現在所屬 workstream 列與目前 `stage` 欄交會的格子，同格多個 task 依設定順序排列；
不畫依賴箭頭。節點以表面色為底、左緣一條狀態色條，並顯示 `title` 與「狀態符號＋`status` 文字」，狀態文字與色條
使用狀態色：`running` 品牌強調色、`blocked` 警示色、`ready` 主要文字色、`pending` 次要文字色、`failed` 失敗色、
`completed` 成功色；`running` 節點另有靜止的強調（品牌強調色的較粗外框與柔光），**不得**有動畫。`completed`
不得與 pane `done` 使用同一種顏色。網格寬度超出區域時，區域內部可橫向捲動，頁面本身不得出現橫向捲軸。

#### Scenario: Scenario D 的畫面

- **GIVEN** 投影中 Project `p` 的 stages 為 `Plan`、`Implement`、`Test`，workstream `backend`、`frontend`、
  `tests` 各有一個 `running` 的 task，分別在 `Implement`、`Plan`、`Test`
- **WHEN** 以瀏覽器開啟 `/`
- **THEN** 網格有三列三欄，三個 `running` 節點分別位於（backend, Implement）、（frontend, Plan）、（tests, Test），
  狀態文字為品牌強調色並有強調外框

#### Scenario: 未宣告 task 的提示

- **GIVEN** 投影中 workstream `backend` 的 `activity_undeclared` 為 `true`、`frontend` 為 `false`
- **WHEN** 重畫
- **THEN** `backend` 列首含「工作中・未宣告 task」，`frontend` 列首不含；停在此狀態再收到一份新投影整頁重畫後結果相同

#### Scenario: 覆蓋造成的斷線仍顯示改綁標示

- **GIVEN** workstream `be` 的覆蓋指向 `wsl`／`w1:p1`，`wsl` 為 `disconnected`（`be` 的 `binding` 為
  `{"state":"runtime_disconnected","runtime":"wsl","source":"override"}`），workstream `fe` 為自動綁定且其 runtime 斷線
- **WHEN** 重畫
- **THEN** `be` 的綁定摘要顯示「runtime 未連線」與「改綁」標示（徽章）；`fe` 的綁定摘要顯示「runtime 未連線」、沒有「改綁」標示
  （此處的標示指綁定摘要內的徽章，與列首的「改綁」按鈕不同）

#### Scenario: 兩個 Project

- **GIVEN** 投影有 Project `p1`、`p2`，使用者尚未選定任何 Project
- **WHEN** 重畫
- **THEN** 中上區域只有一張 Factory Floor，顯示 `p1`；`p2` 的網格不在畫面上，改由左欄切換（見「Project 切換」）

#### Scenario: running 節點沒有動畫

- **GIVEN** 某 task 的 `status` 為 `running`
- **WHEN** 重畫後讀取該節點與其偽元素的計算樣式
- **THEN** `animation-name` 皆為 `none`

#### Scenario: 未知 status 不破壞畫面

- **GIVEN** 某 task 的 `status` 為 `whatever`
- **WHEN** 重畫
- **THEN** 該節點以次要文字色與虛線外框顯示原字串，其他節點正常

#### Scenario: 網格過寬時不撐破頁面

- **GIVEN** 選定 Project 有 10 個 stages，視窗寬 1536
- **WHEN** 重畫
- **THEN** Factory Floor 區域內可橫向捲動看到最後一欄，`document.documentElement.scrollWidth` 不大於視窗寬度

### Requirement: 畫面操作

系統必須在畫面上提供下列操作，並以 `pipeline-progress` 的寫入端點送出：task 節點上，`mark` 為 `none` 且
不在最後一個 stage 時顯示「推進」；`mark` 為 `none` 且不在第一個 stage 時顯示「退回」；`mark` 為 `none` 時顯示「Completed」與「Failed」；`mark` 不是 `none`
時只顯示「清除標記」。workstream 列首顯示「改綁」（`binding.source` 為 `pane` 的固定 pane 工作線不顯示「改綁」與「取消改綁」，因為覆蓋端點會回 409 `not_overridable`）；`binding.source` 為 `override` 時另顯示「取消改綁」，包含綁定狀態為 `runtime_disconnected`、
且 `source` 為 `override`（覆蓋造成的斷線）的情況；`source` 為 `auto` 時不顯示「取消改綁」。
按「改綁」進入改綁模式：頁面顯示指出目標 workstream 的提示與「取消」，所有 `connected` runtime 卡中未
exited 的 pane 列出現「綁定到這裡」，按下即送出覆蓋，成功或按「取消」即離開改綁模式。寫入端點回非 2xx
或請求失敗時，頁面顯示錯誤訊息（含回應本體的 `error`），直到下一次操作或使用者關閉；操作成功後畫面
不自行修改狀態，一律等 `/ws` 推送的新投影重畫。

#### Scenario: 推進按鈕

- **GIVEN** task `t1` 在 `Plan`（非最後一站）、`mark` 為 `none`
- **WHEN** 在畫面按 `t1` 的「推進」
- **THEN** 服務收到 `POST /api/projects/p/tasks/t1/advance`；新投影到達後節點出現在下一站的欄

#### Scenario: 退回按鈕

- **GIVEN** stages 為 `Plan`、`Build`，task `t1` 在 `Build`、`mark` 為 `none`；task `t2` 在 `Plan`、`mark` 為 `none`；
  task `t3` 在 `Build`、`mark` 為 `failed`
- **WHEN** 重畫後在畫面按 `t1` 的「退回」
- **THEN** `t2` 與 `t3` 的節點沒有「退回」；服務收到 `POST /api/projects/p/tasks/t1/retreat`；新投影到達後 `t1` 的節點出現在
  `Plan` 欄

#### Scenario: 覆蓋造成的斷線可取消改綁

- **GIVEN** workstream `be` 的覆蓋指向 `wsl`／`w1:p1`，`wsl` 為 `disconnected`，`be` 的 `binding` 為
  `{"state":"runtime_disconnected","runtime":"wsl","source":"override"}`
- **WHEN** 重畫後在畫面按 `be` 的「取消改綁」
- **THEN** `be` 列首同時有「改綁」按鈕與「取消改綁」按鈕；服務收到 `DELETE /api/projects/p/workstreams/be/override`

#### Scenario: 自動綁定的斷線沒有取消改綁

- **GIVEN** workstream `fe` 沒有覆蓋、其自動綁定的 runtime 斷線，`binding` 為
  `{"state":"runtime_disconnected","runtime":"wsl","source":"auto"}`
- **WHEN** 重畫
- **THEN** `fe` 列首有「改綁」按鈕、沒有「取消改綁」按鈕

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

#### Scenario: 固定 pane 的工作線沒有改綁鈕

- **GIVEN** Repo Project 的 workstream `local~wJ:p1` 的 `binding.source` 為 `pane`，手寫 project 的 workstream `be` 的 `source` 為 `auto`
- **WHEN** 重畫
- **THEN** `local~wJ:p1` 列首沒有「改綁」與「取消改綁」按鈕；`be` 列首有「改綁」按鈕

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

系統必須以三欄版面呈現：頂列與底列橫跨全寬；左欄頂端為「Project」「檔案」「變更」三個分頁，其下顯示目前分頁的內容
（Project 清單、檔案樹或 git 變更清單）；中欄上為 Factory Floor、下為分頁區（第一個分頁為 Live Output，其後為檔案分頁與
`git-review` 定義的分頁，見 `file-review`「檔案分頁」）；
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

#### Scenario: diff 與 Git Graph 不撐破頁面

- **GIVEN** 視窗寬 700，已打開一個含 300 個字元長行的 diff 分頁與一個有 8 條以上並行車道的 Git Graph 分頁
- **WHEN** 分別切到這兩個分頁
- **THEN** 長行在所屬欄內折行，Graph 在分頁內容區內部捲動，頁面沒有橫向捲軸

### Requirement: Project 切換

系統必須在左欄依 `projects` 順序列出每個 Project：顯示 `name`、`warnings` 數量（有時）與各 task `status` 的數量
（以 `status` 字串標示，不使用「完成」字樣）。使用者點選或以鍵盤（Enter 或 Space）選定某個 Project 後，Factory Floor
改為顯示該 Project；目前選定的項目必須有可辨識的標示。未選定過時預設選定第一個 Project；選定的 Project 已不在最新投影中時
改為選定第一個。選取只存在於該瀏覽器頁面（不送到服務、不持久化，重新整理後回到預設），整頁重畫不得清除選取。選定 Project
不是「畫面操作」：不得清除最近一次操作的錯誤訊息、不得影響進行中的寫入請求的結果呈現、不得離開改綁模式。投影沒有任何
Project 時，左欄與 Factory Floor 區域顯示沒有 Project 的空狀態，頁面其他部分正常；Factory Floor 的空狀態文字指向左欄
「偵測到的 repo」區（說明在那裡加入 repo 即可看到 Factory Floor），不得出現需要重啟 Cockpit 的字樣。

左欄 Project 分頁在 Project 清單之外必須有「偵測到的 repo」區，依投影 `detected_repos` 的順序列出每個 repo：顯示 `name`、
`pane_count` 與「加入」鈕；沒有偵測到的 repo 時該區不列任何項目、沒有「加入」鈕。按「加入」送出 `POST /api/repo-projects`，
本體為 `{"repo": <該項的 repo>, "stages": <預設 stages>}`（不帶 `name`）；預設 stages 依介面語言：繁中為 `規劃`、`實作`、
`審查`、`完成`，英文為 `Plan`、`Implement`、`Review`、`Complete`。

`kind` 為 `repo` 的 Project 項目必須有「⋯」選單（手寫的 Project 沒有），選單項目為「改名」「編輯 stage」「移除」：

- 「改名」讓使用者輸入新名稱，送出 `PATCH /api/repo-projects/<pid>`，本體 `{"name": <新名稱>}`。
- 「編輯 stage」開啟對話框，依序列出該 Project 目前的 stages，使用者可改名、新增、刪除、調整順序；按儲存送出
  `PATCH /api/repo-projects/<pid>`，本體 `{"stages": [{"name": <名稱>, "from": <原名稱 或 null>}, ...]}`，順序即對話框中的
  順序，原有項目的 `from` 為它原本的名稱，新增項目的 `from` 為 `null`；按取消不送出請求。
- 「移除」先顯示含 Project 名稱的確認，確認後送出 `DELETE /api/repo-projects/<pid>`；取消不送出請求。

加入、改名、編輯 stage、移除都是「畫面操作」：失敗（回非 2xx 或請求失敗）時依「畫面操作」顯示錯誤訊息（含回應本體的 `error`，
介面為英文時依 `code` 顯示英文），成功後畫面不自行修改狀態，一律等 `/ws` 推送的新投影重畫。加入成功（回 201 與新 Project 的 `id`）後，畫面在含該 `id` 的投影到達時自動選定這個新 Project（只選取一次，之後
使用者可自由切換；投影到達前使用者已手動選定其他 Project 時不自動切換）。同一個 repo 的加入請求進行中時，該 repo 的「加入」
鈕呈現忙碌且不再送出請求，直到請求失敗或該 repo 離開偵測區（避免連點造成第二筆請求以「已加入」失敗）。對話框與確認在開啟期間收到新投影並
整頁重畫時必須保留（含已輸入的內容與焦點）。所有名稱（Project、repo、stage、workstream）一律以文字節點呈現，不以 HTML 插入。

#### Scenario: 切換 Project

- **GIVEN** 投影有 Project `p1`、`p2`
- **WHEN** 開啟 `/`，之後點左欄的 `p2`
- **THEN** 開啟時左欄 `p1` 在上、`p2` 在下，`p1` 有選定標示、Factory Floor 顯示 `p1`；點選後 `p2` 有選定標示、
  Factory Floor 顯示 `p2`

#### Scenario: 選取跨重畫保留

- **GIVEN** 已選定 `p2`
- **WHEN** 期間收到兩份新投影並整頁重畫
- **THEN** 仍選定 `p2`、Factory Floor 仍顯示 `p2`

#### Scenario: 鍵盤切換與焦點保留

- **GIVEN** 焦點在左欄 `p2` 項目上，投影每 100 ms 推送一份新的 version
- **WHEN** 經過 2 秒後按 Enter
- **THEN** 這段期間焦點始終在 `p2` 項目上，按 Enter 後 Factory Floor 顯示 `p2`

#### Scenario: 切換 Project 不清除錯誤也不離開改綁模式

- **GIVEN** 頁面正顯示一則操作錯誤訊息，且已按 `p1` 某 workstream 的「改綁」進入改綁模式
- **WHEN** 點左欄的 `p2`
- **THEN** 錯誤訊息仍在，改綁提示與各 pane 列的「綁定到這裡」仍在

#### Scenario: 各狀態數量

- **GIVEN** `p1` 有 2 個 `running`、1 個 `completed`、1 個 `failed` 的 task，並有 1 則 warning
- **WHEN** 重畫
- **THEN** 左欄 `p1` 項目顯示 `running` 2、`completed` 1、`failed` 1 與 1 則 warning，不出現「完成」字樣

#### Scenario: 沒有 Project

- **GIVEN** 投影的 `projects` 為空
- **WHEN** 重畫
- **THEN** 左欄與 Factory Floor 區域顯示沒有 Project 的空狀態，runtime 卡與 Live Output 正常

#### Scenario: 空狀態指向偵測到的 repo

- **GIVEN** 投影的 `projects` 為空，`detected_repos` 有 repo `app`
- **WHEN** 重畫（繁中與英文各一次）
- **THEN** Factory Floor 的空狀態文字指向「偵測到的 repo」區，不含「需要重啟」（英文為 `Restart`）字樣；左欄「偵測到的 repo」區列出 `app`
  與「加入」鈕

#### Scenario: 列出偵測到的 repo

- **GIVEN** 投影的 `detected_repos` 為 `app`（`pane_count` 2）與 `lib`（`pane_count` 1），`projects` 有手寫的 `h`
- **WHEN** 重畫
- **THEN** 左欄 Project 分頁在 `h` 之外有「偵測到的 repo」區，依序列出 `app`（含數量 2）與 `lib`，各有「加入」鈕

#### Scenario: 按加入送出預設 stages

- **GIVEN** 介面為繁中，`detected_repos` 有 repo `app`（repo key `d:\work\app\.git`）
- **WHEN** 按 `app` 的「加入」
- **THEN** 服務收到 `POST /api/repo-projects`，本體 `{"repo":"d:\\work\\app\\.git","stages":["規劃","實作","審查","完成"]}`
  （沒有 `name`）；新投影到達後 `app` 從該區消失並出現在 Project 清單。介面為英文時 `stages` 為 `["Plan","Implement","Review","Complete"]`

#### Scenario: 加入成功後自動選定新 Project

- **GIVEN** 已選定 Project `h`，`detected_repos` 有 repo `app`
- **WHEN** 按 `app` 的「加入」，服務回 201 `{"id":"app"}`，稍後含 `app` 的新投影到達
- **THEN** 左欄選定 `app`，Factory Floor 顯示 `app`；之後點回 `h` 不會再被自動切走

#### Scenario: 投影到達前手動切換則不自動選定

- **GIVEN** 已選定 Project `h`，`detected_repos` 有 repo `app`
- **WHEN** 按 `app` 的「加入」，服務回 201 `{"id":"app"}`；含 `app` 的投影到達前，使用者點選 Project `h2`
- **THEN** 投影到達後仍選定 `h2`

#### Scenario: 連點加入只送出一次

- **GIVEN** `detected_repos` 有 repo `app`
- **WHEN** 快速連按兩次 `app` 的「加入」
- **THEN** 只送出一筆 `POST /api/repo-projects`，不顯示錯誤；請求進行中該鈕呈現忙碌

#### Scenario: 加入被拒絕時顯示原因

- **GIVEN** 服務對加入請求回 409，`code` 為 `repo_already_added`
- **WHEN** 該操作完成
- **THEN** 頁面顯示含原因的錯誤訊息（英文介面以字典的英文訊息），之後的重畫不清除它

#### Scenario: 只有 Repo Project 有選單

- **GIVEN** 投影有手寫的 `h`（`kind` 為 `config`）與 Repo Project `app`（`kind` 為 `repo`）
- **WHEN** 重畫
- **THEN** `app` 項目有「⋯」選單，選單含「改名」「編輯 stage」「移除」；`h` 項目沒有

#### Scenario: 改名

- **WHEN** 從 `app` 的「⋯」選單選「改名」，輸入 `App 前端` 並送出
- **THEN** 服務收到 `PATCH /api/repo-projects/app`，本體 `{"name":"App 前端"}`；新投影到達後左欄與 Factory Floor 標題顯示 `App 前端`

#### Scenario: 編輯 stage

- **GIVEN** `app` 的 stages 為 `Plan`、`Implement`、`Review`、`Done`
- **WHEN** 開啟「編輯 stage」對話框，把 `Implement` 改名為 `Build`、刪除 `Review`、在最後新增 `Ship`，按儲存
- **THEN** 服務收到 `PATCH /api/repo-projects/app`，本體 `{"stages":[{"name":"Plan","from":"Plan"},{"name":"Build","from":"Implement"},{"name":"Done","from":"Done"},{"name":"Ship","from":null}]}`

#### Scenario: 取消編輯與取消移除不送請求

- **WHEN** 開啟「編輯 stage」對話框後按取消；再從選單選「移除」，在確認中按取消
- **THEN** 服務沒有收到任何 `PATCH` 或 `DELETE` 請求

#### Scenario: 移除要先確認

- **WHEN** 從 `app` 的「⋯」選單選「移除」
- **THEN** 先顯示含 `app` 名稱的確認，尚未送出請求；確認後服務收到 `DELETE /api/repo-projects/app`，新投影到達後 `app` 從
  Project 清單消失；若它原是選定的 Project，改為選定第一個

#### Scenario: 對話框跨重畫保留

- **GIVEN** 「編輯 stage」對話框開著，已改了一個 stage 名稱但尚未儲存
- **WHEN** 期間收到兩份新投影並整頁重畫
- **THEN** 對話框仍在，已輸入的內容與焦點保留

#### Scenario: 名稱不以 HTML 解讀

- **GIVEN** 某 Repo Project 的名稱為 `<img src=x onerror=alert(1)>`，其 stage 名稱與 pane label 也含 HTML 字元
- **WHEN** 重畫
- **THEN** 這些名稱在左欄、Factory Floor 與對話框中以原樣文字顯示，沒有建立任何 `<img>` 元素，也沒有執行指令碼

### Requirement: 狀態端點只接受本機同源請求

系統必須對 `GET /api/state` 與 `GET /ws` 套用 `pipeline-progress`「寫入端點只接受本機同源請求」的同一條來源規則：`Host`
標頭必須是 `127.0.0.1:<port>`、`localhost:<port>` 或 `[::1]:<port>`（`<port>` 為服務實際監聽的埠）；若帶 `Origin`
標頭，其值必須是 `http://` 加上同一個 `Host` 值；`Host` 或 `Origin` 重複出現視為不符合。不符合時回 403、本體的
`code` 為 `forbidden_source`，`/ws` 不升級成 WebSocket、不送出任何狀態。沒有 `Origin` 但 `Host` 合格的請求放行。
被拒絕的請求不算 `/ws` 連線，也不延長 `--exit-when-idle` 的閒置期限。

#### Scenario: 外站網頁連 /ws 被拒

- **WHEN** 以 `Host: 127.0.0.1:7770`、`Origin: https://evil.example` 請求升級 `/ws`
- **THEN** 回 403，`code` 為 `forbidden_source`，連線沒有升級、沒有收到任何狀態

#### Scenario: DNS rebinding 讀不到狀態

- **WHEN** 以 `Host: evil.example:7770` 請求 `/api/state`
- **THEN** 回 403，本體不含投影內容

#### Scenario: 同源與命令列照常可用

- **WHEN** 分別以 `Host: 127.0.0.1:7770`＋`Origin: http://127.0.0.1:7770` 升級 `/ws`，以及只帶 `Host: localhost:7770` 請求
  `/api/state`
- **THEN** `/ws` 升級成功並收到目前整張圖；`/api/state` 回 200

### Requirement: 儀表板頁面不得被嵌入框架

系統必須在 `GET /` 的回應帶 `X-Frame-Options: DENY` 與 `Content-Security-Policy: frame-ancestors 'none'`，使外站網頁無法把
儀表板以 iframe 嵌入（嵌入後 iframe 內的畫面會以 Cockpit 自己的來源連 `/ws`，通過來源檢查，使 `--exit-when-idle` 的後端
無法結束，也讓寫入按鈕可被點擊劫持）。

#### Scenario: 首頁禁止被嵌入

- **WHEN** 請求 `GET /`
- **THEN** 回應標頭含 `X-Frame-Options: DENY` 與 `Content-Security-Policy: frame-ancestors 'none'`

### Requirement: Repo Project 工作線的 worktree 標註

系統必須在 Factory Floor 的 workstream 列首，於 workstream 名稱下方以文字顯示該 workstream 的 `worktree`（linked worktree
的資料夾名稱，見 `state-projection`「Project 投影」）；沒有 `worktree` 時不顯示。顯示內容一律以文字節點呈現，不以 HTML 插入。
`binding.source` 為 `pane` 的綁定摘要與 `auto` 同樣不加「改綁」標示，列首也不顯示「改綁」鈕（見「畫面操作」）。

#### Scenario: linked worktree 的工作線

- **GIVEN** Repo Project 的 workstream `local~wJ:p3`（`name` 為 `backend`）的 `worktree` 為 `app-wt`
- **WHEN** 重畫
- **THEN** 該列列首顯示 `backend` 與 `app-wt`；`worktree` 為空的工作線列首沒有 worktree 標註

#### Scenario: 固定 pane 的綁定摘要沒有改綁標示

- **GIVEN** workstream 的 `binding` 為 `bound`、`source` 為 `pane`
- **WHEN** 重畫
- **THEN** 綁定摘要顯示 runtime 與 pane id，沒有「改綁」標示
