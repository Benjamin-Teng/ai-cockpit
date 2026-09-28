# file-review（delta）

## MODIFIED Requirements

### Requirement: 原始內容端點

系統必須提供 `GET /api/files/<runtime>/<root_id>/raw/<相對路徑>`：回 200 與檔案原始位元組。`Content-Type` 依副檔名（不分
大小寫）：`.html`／`.htm` → 回應的位元組是合法 UTF-8 時為 `text/html; charset=utf-8`，不是合法 UTF-8 時為不帶 charset 的
`text/html`（由檔案內的 `<meta charset>` 或 BOM 決定編碼）、`.pdf` → `application/pdf`、`.svg` → `image/svg+xml`、
`.png` → `image/png`、`.jpg`／`.jpeg` → `image/jpeg`、`.gif` → `image/gif`、`.webp` → `image/webp`、`.css` → `text/css`、
`.md`／`.markdown`／`.txt` 與其他 `viewer` 為 `text` 的檔案 → `text/plain; charset=utf-8`，其餘 → `application/octet-stream`。
所有原始內容回應都必須帶 `Content-Security-Policy: sandbox`（使直接開啟該網址時，HTML 或 SVG 中的腳本不在 Cockpit 的來源下
執行）。檔案超過 50 MiB 回 413 `too_large`。

#### Scenario: HTML 帶 sandbox

- **GIVEN** `page.html` 的內容是 UTF-8
- **WHEN** 請求 `page.html` 的原始內容
- **THEN** 回 200，`Content-Type` 為 `text/html; charset=utf-8`，帶 `Content-Security-Policy: sandbox` 與
  `X-Content-Type-Options: nosniff`

#### Scenario: 沒有宣告編碼的 UTF-8 HTML

- **GIVEN** `nometa.html` 以 UTF-8 寫成、含中文「檔案瀏覽」，檔內沒有 `<meta charset>`，也沒有 BOM
- **WHEN** 請求其原始內容，並在檔案分頁中開啟它
- **THEN** 原始內容回應的 `Content-Type` 為 `text/html; charset=utf-8`；檔案分頁的 HTML 檢視器顯示「檔案瀏覽」字樣，不是亂碼

#### Scenario: 非 UTF-8 的 HTML

- **GIVEN** `old.html` 以 Big5 編碼寫成、含中文，並宣告 `<meta charset="big5">`
- **WHEN** 請求其原始內容
- **THEN** 回 200，`Content-Type` 為不帶 charset 的 `text/html`，回應本體與檔案位元組完全相同

#### Scenario: 太大

- **GIVEN** `big.pdf` 為 60 MiB
- **WHEN** 請求其原始內容
- **THEN** 回 413，`code` 為 `too_large`
