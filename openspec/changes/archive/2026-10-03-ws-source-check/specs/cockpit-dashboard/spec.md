# cockpit-dashboard（delta）

## ADDED Requirements

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
