# cockpit-config（delta）

## MODIFIED Requirements

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
