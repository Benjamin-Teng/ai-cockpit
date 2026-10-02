# cockpit-config（delta）

## MODIFIED Requirements

### Requirement: 驗證錯誤指出是哪一筆

系統必須在下列情況啟動失敗並在訊息中指出識別與原因——逐筆 runtime 的問題以 runtime `id`（或序號）
識別、全域欄位的問題以區段與欄位名（`server.listen`、`polling.resnapshot_secs` 等）識別：同一筆給了兩個
以上端點、`id` 重複或空白、`listen` 的位址不是 `127.0.0.1` 或 `::1`、`listen` 的埠為 80（埠 0 允許，由作業系統
指派）、`resnapshot_secs` 或 `wsl_probe_secs` 為 0。`listen` 限縮的理由：來源檢查（`cockpit-dashboard`「狀態端點只接受
本機同源請求」、`pipeline-progress`「寫入端點只接受本機同源請求」）只認 `127.0.0.1:<port>`、`localhost:<port>`、
`[::1]:<port>` 的 `Host`，其他 loopback 位址或省略預設埠（80）的 `Host` 會讓儀表板與桌面啟動器全部被 403。

#### Scenario: 端點給了兩個

- **GIVEN** 某筆 `id = "x"` 同時有 `socket` 與 `wsl`
- **WHEN** 載入
- **THEN** 啟動失敗，訊息含 `x` 與「三選一」

#### Scenario: id 重複

- **GIVEN** 兩筆 runtime 都叫 `win`
- **WHEN** 載入
- **THEN** 啟動失敗，訊息含 `win`

#### Scenario: 非 loopback

- **GIVEN** `listen = "0.0.0.0:7770"`
- **WHEN** 載入
- **THEN** 啟動失敗，訊息含 `0.0.0.0`

#### Scenario: 其他 loopback 位址被拒

- **GIVEN** `listen = "127.0.0.2:7770"`
- **WHEN** 載入
- **THEN** 啟動失敗，訊息含 `server.listen` 與 `127.0.0.2:7770`

#### Scenario: 埠 80 被拒

- **GIVEN** `listen = "127.0.0.1:80"`
- **WHEN** 載入
- **THEN** 啟動失敗，訊息含 `server.listen` 與 `127.0.0.1:80`

#### Scenario: IPv6 loopback 與埠 0 通過

- **GIVEN** `listen = "[::1]:7770"`，或 `listen = "127.0.0.1:0"`
- **WHEN** 載入
- **THEN** 載入成功
