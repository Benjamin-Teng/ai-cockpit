# proposal：ws-source-check

## Why

寫入、輸出讀取、檔案與 git 端點都有來源檢查（`pipeline-progress`「寫入端點只接受本機同源請求」的 Host／Origin 規則），
但 `GET /api/state` 與 `GET /ws` 沒有。WebSocket 不受瀏覽器的同源政策（CORS）限制，所以 Cockpit 執行時，使用者瀏覽器裡
開著的任何網頁都能連 `ws://127.0.0.1:<port>/ws`，拿到整張投影：專案、task、workspace 名稱、pane 標籤與工作目錄。
`/api/state` 不檢查 `Host`，DNS rebinding 的網頁也能讀到同一份資料。

這是 change 11 審查時延後的 m1（另一個後果：任何網頁連著 `/ws` 就能讓 `--exit-when-idle` 的後端一直不結束）。
宣傳頁與 README 已公開揭露這個限制，使用者 2026-10-03 決定補上。

## What Changes

- `GET /api/state` 與 `GET /ws` 套用與寫入端點相同的來源檢查：`Host` 必須是本機位址加實際監聽埠，帶 `Origin` 時必須與
  `Host` 同源；不符合回 403（`code` 為 `forbidden_source`），`/ws` 不升級成 WebSocket。
- 被拒絕的 `/ws` 請求不算連線，不影響 `--exit-when-idle` 的計時；被拒絕的 `/api/state` 不延長閒置期限。
- README、`README.zh-TW.md` 移除「尚未檢查」的揭露，改寫成全部資料端點都有來源檢查（`site/` 宣傳頁本來就沒有這段揭露，不動）。
- 設定驗證收緊（審查 I2）：`[server] listen` 的位址只能是 `127.0.0.1` 或 `::1`、埠不得為 80；否則來源檢查會讓儀表板與桌面啟動器
  全部 403（design D3）。
- `GET /` 回應加 `X-Frame-Options: DENY` 與 `Content-Security-Policy: frame-ancestors 'none'`，擋外站 iframe 嵌入（design D4）。

## 非目標

- 不檢查靜態資源（`/`、`/app/…`、`/manifest.webmanifest`、`/icons/…`、`/vendor/…`）：內容公開、不含使用者資料。
- 不改來源規則本身（不加 token、不要求 `Origin`）：命令列工具與桌面啟動器不帶 `Origin`，維持可用。

## Capabilities

### New Capabilities

（無）

### Modified Capabilities

- `cockpit-dashboard`：新增需求「狀態端點只接受本機同源請求」「儀表板頁面不得被嵌入框架」。
- `cockpit-config`：修改需求「驗證錯誤指出是哪一筆」（`listen` 位址與埠規則）。
- `desktop-launch`：修改需求「閒置自動結束」（只有通過來源檢查的 `GET /api/state` 延長期限；被拒的請求不算）。

## Impact

- 程式：`cockpit/src/http.rs` 的路由表（`/api/state`、`/ws` 加 `route_layer(source_check)`；`GET /` 加防嵌入標頭）、
  `cockpit/src/source_check.rs` 模組文件、`cockpit/src/config.rs`（`listen` 驗證）。
- 測試：`cockpit/tests/http.rs`、`cockpit/tests/ws.rs`、`cockpit/tests/idle_exit.rs`、`cockpit/tests/config.rs`，另有
  `cockpit/tests/app.rs`、`cockpit/tests/real_attach.rs` 補來源檢查所需的 `Host`／回填埠。
- 文件：`cockpit/README.md`、`cockpit.example.toml`、`README.md`、`README.zh-TW.md`、`docs/handover.md`。
