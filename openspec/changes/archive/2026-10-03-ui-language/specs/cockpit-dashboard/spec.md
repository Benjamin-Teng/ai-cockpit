# cockpit-dashboard（delta）

## MODIFIED Requirements

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
