# design：ws-source-check

## Context

動機見 `proposal.md`；行為契約見本 change 的 `specs/` 下三份 delta（`cockpit-dashboard`、`cockpit-config`、`desktop-launch`）。現況：`cockpit/src/source_check.rs` 的
`source_check` middleware 以 `MethodRouter::route_layer` 掛在寫入、輸出、檔案、git、agent 路由上；`/api/state` 與 `/ws`
直接 `get(handler)`，沒有掛。`--exit-when-idle` 的活動紀錄在 handler 內（`api_state` 的 `record_request()`、`ws_handler`
升級後的 `connect()`）。

## Goals / Non-Goals

**Goals：**

- 兩條路由掛上同一個 `source_check`，規則、錯誤本體與其他端點一致。
- 讓「設定驗證放行、但來源檢查會擋」的 `listen` 值不再存在（D3）。

**Non-Goals：**

- 不改 `source_check` 的判定規則；不檢查靜態資源路由（`GET /` 只加防嵌入標頭，D4）。

## Decisions

### D1 重用 `source_check`，以 `route_layer` 掛在兩條路由上

`.route("/api/state", get(api_state).route_layer(…))`、`.route("/ws", get(ws_handler).route_layer(…))`。middleware 在
handler 之前執行：被拒的 `/api/state` 不會呼叫 `record_request()`；被拒的 `/ws` 不會進到 `WebSocketUpgrade`
extractor，因此不升級、不呼叫 `connect()`。閒置計時不必另外處理。

- **替代方案：在 handler 內自行檢查**。會重複一份判定邏輯，且 `/ws` 要在 extractor 之前判斷才不會升級；不採用。
- **替代方案：整個 `Router` 套一層**。會連靜態資源一起檢查，與其他端點的掛法不一致；不採用。

### D2 不帶 `Origin` 的請求照常放行

瀏覽器對 WebSocket 一律帶 `Origin`，所以外站網頁一定會被擋；不帶 `Origin` 的是命令列工具、桌面啟動器的偵測
（`cockpit/src/launch.rs` 以 `Host: <監聽位址>` 發 `GET /api/state`），以及驗收腳本用的 Node 內建 `WebSocket`
（2026-10-03 實測不帶 `Origin`、`Host` 為 `127.0.0.1:<port>`）。三者都維持可用。

### D3 設定驗證收緊：`listen` 只能是 `127.0.0.1` 或 `::1`、埠不得為 80

審查（I2）發現：設定驗證原本只要求 `IpAddr::is_loopback()`（整個 `127.0.0.0/8`、任何埠），但來源檢查只認 `127.0.0.1`／
`localhost`／`[::1]` 加明確埠。兩者之間的縫隙在改動前只壞寫入按鈕，改動後會讓整個儀表板與桌面啟動器壞掉：

- `listen = "127.0.0.2:7770"`：啟動器偵測送 `Host: 127.0.0.2:7770` → 403，被誤判為「已被其他程式占用」或後端「沒有就緒」
  而殺掉剛啟動的後端；手動執行時頁面載得到但 `/ws` 永遠 403。
- `listen = "127.0.0.1:80"`：瀏覽器省略預設埠送 `Host: 127.0.0.1`，對不上 `127.0.0.1:80`，`/ws` 403。

決定：驗證時就擋（`cockpit/src/config.rs`），訊息指出欄位、原值與原因；埠 0 維持允許（Rust 測試依賴作業系統指派
的埠）。同步修改 `cockpit-config` 主 spec（本 change 的 delta）、`cockpit/README.md` 與 `cockpit.example.toml`。

- **替代方案：讓 `source_check` 額外接受 `listen` 的 IP 字面值與「埠 80 省略埠」**。會改動既有寫入端點的規則（`pipeline-progress`、
  `live-output` 等多份 spec），攻擊面也跟著放寬；不採用。

### D4 `GET /` 加 `X-Frame-Options: DENY` 與 `Content-Security-Policy: frame-ancestors 'none'`

審查（M1）：外站網頁可以把 `http://127.0.0.1:<port>/` 以 iframe 嵌入，iframe 內的 `channel.js` 以 Cockpit 自己的來源連 `/ws`，
`Origin` 與 `Host` 同源、通過來源檢查，使 `--exit-when-idle` 的後端永遠不閒置結束，也讓寫入按鈕可被點擊劫持（後者是既有問題）。
`GET /` 的回應加兩個防嵌入標頭即可同時擋掉兩者；`GET /` 仍不做來源檢查（內容公開，且桌面啟動器與瀏覽器直接開啟都不帶
`Origin`）。

## Risks / Trade-offs

- **已接受的殘留**：外站網頁仍可用 `fetch("http://127.0.0.1:<port>/", {mode: "no-cors"})`（或 `<img>`）定期請求 `GET /`。
  `GET /` 不經來源檢查且會延長閒置期限，所以這種頁面開著時，`--exit-when-idle` 的後端在最後一個連線關閉後仍可能不結束。
  讀不到任何資料（`/api/state`、`/ws` 都被擋），後果僅是後端多活一陣子；要根治得在 `GET /` 依 `Sec-Fetch-Site` 區分，
  或停止讓 `GET /` 延長期限（會動到 `desktop-launch` 的啟動器偵測語意），不值得，因此接受。proposal 的「另一個後果」因此只算
  「經由 `/ws` 與 `/api/state` 的保活」已解決。
- 使用者若以自訂 hosts 別名等非 loopback 主機名稱開啟 Cockpit，頁面本身載得到，但 `/ws` 會被拒、畫面停在連線中；與寫入端點的
  既有行為一致（按鈕會 403），`cockpit/README.md` 已說明只能用 `127.0.0.1`、`localhost` 或 `[::1]`。
