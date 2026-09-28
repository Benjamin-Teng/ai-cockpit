# proposal：html-charset

## Why

原始內容端點對 `.html`／`.htm` 回不帶 charset 的 `text/html`（change 5a 主規格「原始內容端點」的規定）。檔案本身沒有
`<meta charset>` 時，瀏覽器改用系統預設的舊編碼解碼——在繁體中文 Windows 上的 Chrome 是 Big5——於是 UTF-8 寫成的
HTML 在檔案分頁的 iframe 裡顯示成亂碼。repo 裡的 HTML 絕大多數是 UTF-8、且常常沒有宣告 charset（例如工具產生的報告），
這是 file-review 使用上最直接看得到的缺陷。change 5a 實作時已發現並列為待使用者決定（archive
`2026-09-28-file-review/sdd-ledger.md`），使用者 2026-09-28 決定修，並選定「依內容決定」的做法。

## What Changes

- 原始內容端點對 `.html`／`.htm`：內容（實際回傳的位元組）是合法 UTF-8 時回 `text/html; charset=utf-8`；不是合法 UTF-8
  時維持不帶 charset 的 `text/html`，交給檔案內的 `<meta charset>` 或 BOM 決定。
- 修改 scenario「HTML 帶 sandbox」的 `Content-Type` 期望值（fixture `page.html` 是 UTF-8），新增「沒有宣告編碼的 UTF-8
  HTML」與「非 UTF-8 的 HTML」兩個 scenario。
- 其他副檔名的 content-type 規則、CSP、大小上限、錯誤處理都不變。

## 非目標

- 不改 `.css`：CSS 沒有 `@charset` 與 BOM 時沿用引用它的 HTML 的編碼，HTML 修正後同目錄樣式表跟著正確。
- 不偵測 Big5 等其他編碼、不做轉碼：非 UTF-8 的 HTML 維持原本行為（design D1 取捨）。
- 不改 HTML 檢視器的載入方式（仍以 iframe `src` 載入原始內容端點；change 5a 裁決 R30，見 archive `2026-09-28-file-review`
  的 design D8 與 sdd-ledger）。
- 不改 Markdown 渲染端點（本來就是 `text/html; charset=utf-8`）與純文字（本來就是 `text/plain; charset=utf-8`）。

## Capabilities

### New Capabilities

（無）

### Modified Capabilities

- `file-review`：「原始內容端點」的 `.html`／`.htm` content-type 規則改為依內容是否為合法 UTF-8 決定是否帶
  `charset=utf-8`；修改一個 scenario、新增兩個 scenario。

## Impact

- 程式：`cockpit/src/files.rs` 的原始內容回應（`raw_response`／`raw_content_type`）。
- 測試：`cockpit/tests/files_endpoint.rs` 中斷言 `.html` 為 `text/html` 的既有測試依 spec 更新，另補新 scenario 的測試；
  `docs/research/2026-09-27/files-check.js` 新增一段端對端檢查（iframe 中的中文顯示正確）。
- 文件：`cockpit/README.md`「檔案瀏覽與 Review」的 content-type 說明。
- 無新依賴、無資料遷移；前端不需修改。
