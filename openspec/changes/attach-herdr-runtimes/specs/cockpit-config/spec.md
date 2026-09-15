# cockpit-config（delta）

## Purpose

定義 `cockpit` 執行檔的設定來源、格式、預設值與驗證。證據：設計文件 §8.2、§2.2 路徑規則、
`herdr-client/README.md`「本機預設 socket 路徑」。

## ADDED Requirements

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
`HERDR_SOCKET_PATH` → `HERDR_SESSION` → 平台預設解析本機路徑；未知欄位視為錯誤。

#### Scenario: 設計文件範例可解析

- **GIVEN** 設計文件 §8.2 的範例（兩筆 runtime，`win` 省略端點、`wsl` 用 `wsl` 表）
- **WHEN** 載入
- **THEN** `win` 為本機預設路徑、`wsl` 的發行版為 `Ubuntu-24.04`、socket 為所給路徑，
  `resnapshot_secs` 30、`wsl_probe_secs` 60

#### Scenario: 未知 kind

- **GIVEN** 某筆 `kind = "tmux"`
- **WHEN** 載入
- **THEN** 啟動失敗，訊息含該筆 `id` 與 `tmux`

### Requirement: 驗證錯誤指出是哪一筆

系統必須在下列情況啟動失敗並在訊息中指出識別與原因——逐筆 runtime 的問題以 runtime `id`（或序號）
識別、全域欄位的問題以區段與欄位名（`server.listen`、`polling.resnapshot_secs` 等）識別：同一筆給了兩個
以上端點、`id` 重複或空白、`listen` 不是 loopback 位址、`resnapshot_secs` 或 `wsl_probe_secs` 為 0。

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
