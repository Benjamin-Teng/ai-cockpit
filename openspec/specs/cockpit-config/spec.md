# cockpit-config Specification

## Purpose

定義 `cockpit` 執行檔的設定來源、格式、預設值與驗證（Project／Pipeline 區段本身的規則見 `pipeline-config`）。證據：設計文件 §8.2、§2.2 路徑規則、
`herdr-client/README.md`「本機預設 socket 路徑」。

## Requirements

### Requirement: 設定檔位置與零設定模式

系統必須依序決定設定：命令列 `--config <path>` → 工作目錄的 `cockpit.toml` → 零設定模式（等同
一筆 `id = "local"`、`kind = "herdr"`、自動找本機預設 socket 的 runtime，其餘皆預設值）；`--config`
指定的檔案不存在或無法解析時啟動失敗並印出路徑與原因。

#### Scenario: 零設定

- **GIVEN** 沒有 `--config`、工作目錄沒有 `cockpit.toml`
- **WHEN** 載入設定
- **THEN** 得到一筆 id 為 `local` 的 runtime，端點為 `herdr-client` 的預設路徑解析結果，`listen` 為
  `127.0.0.1:7770`

#### Scenario: 指定檔案不存在

- **WHEN** `--config missing.toml`
- **THEN** 啟動失敗，訊息含 `missing.toml`

### Requirement: 設定內容與預設值

系統必須解析 `[server] listen`（預設 `127.0.0.1:7770`）、`[polling] resnapshot_secs`（預設 30）與
`wsl_probe_secs`（預設 60）、`[[runtime]]` 每筆的 `id`、`kind`（只接受 `herdr`）與端點三選一：
`socket`（本機 socket 路徑）、`wsl = { distro, socket }`、`command = [程式, 參數…]`；三者都省略時依
`HERDR_SOCKET_PATH` → `HERDR_SESSION` → 平台預設解析本機路徑；另解析 `[[project]]` 與 `[state]` 兩個
區段（內容、預設值與驗證見 `pipeline-config`）；以上之外的未知區段或欄位視為錯誤。

#### Scenario: 設計文件範例可解析

- **GIVEN** 設計文件 §8.2 的範例（兩筆 runtime，`win` 省略端點、`wsl` 用 `wsl` 表）
- **WHEN** 載入
- **THEN** `win` 為本機預設路徑、`wsl` 的發行版為 `Ubuntu-24.04`、socket 為所給路徑，
  `resnapshot_secs` 30、`wsl_probe_secs` 60

#### Scenario: 未知 kind

- **GIVEN** 某筆 `kind = "tmux"`
- **WHEN** 載入
- **THEN** 啟動失敗，訊息含該筆 `id` 與 `tmux`

#### Scenario: 範例設定檔含 project 可解析

- **GIVEN** repo 根目錄的 `cockpit.example.toml`（含兩筆 runtime 與至少一筆 `[[project]]`）
- **WHEN** 載入
- **THEN** 載入成功，Project 清單非空

#### Scenario: 未知區段

- **GIVEN** 設定檔含 `[pipeline]` 區段
- **WHEN** 載入
- **THEN** 啟動失敗，訊息含 `pipeline`

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
