# cockpit-dashboard（delta）

## MODIFIED Requirements

### Requirement: 路由與內嵌資源

系統必須提供 `GET /`（`index.html`）、`GET /app/<檔名>`（`channel.js`、`render.js`、`actions.js`、`output.js`、
`files.js`、`viewers.js`、`git.js`、`style.css`）、`GET /manifest.webmanifest`、`GET /icons/<檔名>`（192 與 512 px PNG）、
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
  `/app/git.js`、`/app/style.css`、`/manifest.webmanifest`、`/icons/icon-192.png`、`/api/state`
- **THEN** 皆為 200，content-type 分別為 HTML、JavaScript、JavaScript、JavaScript、JavaScript、JavaScript、JavaScript、CSS、
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
