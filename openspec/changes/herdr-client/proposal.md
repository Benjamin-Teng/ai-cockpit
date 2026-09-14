# herdr-client（change 1a）

## Why

Cockpit 的每個功能都建立在「能穩定、唯讀地讀到 HERDR 狀態」之上，但 HERDR 的幾個特性若不先在
最底層釘住，change 1b 的連線迴圈就會建在未驗證的假設上：Windows 端是 named pipe 而非 TCP
（設計文件 §2.2）、WSL 端只能經子程序 stdio 橋接（ADR-0002）、每條連線只服務一個 method
（§2.2）、沒有全域 agent 狀態訂閱、只能每 pane 各訂一筆（§2.3）。本 change 產出
`herdr-client` crate：只懂 HERDR 協定的 observer 子集，並先以設計文件 §11 的五個 spike
驗證這些假設，任一不成立就回頭改設計文件，而不是改到一半才發現。

## What Changes

- 新增 cargo workspace 與第一個 crate `herdr-client`（其餘三個 crate 由 change 1b 建立，
  依賴方向見 ADR-0003）。
- 執行設計文件 §11 的五個 spike，結果與擷取到的真機資料（snapshot、事件行）去識別化後
  進 repo，作為後續測試的 fixture。
- `Connector` trait 與三個實作：Windows named pipe、unix socket、子程序 stdio 橋接
  （設計文件 §5.1）；連線失敗統一分類為 ServerNotRunning／Spawn／Io。
- `Client`：單次 request（change 1 只用 `session.snapshot`）與長連線 subscribe
  （24 種生命週期訂閱＋每 pane 的 `pane.agent_status_changed`），事件串流分軌、壞行跳過、
  EOF 結束（§5.2）。
- HERDR observer 子集型別：`SessionSnapshot` 與四個 Info、`AgentStatus`（未知值收成
  Unknown）、change 1b 會消費的事件 payload、`pane.read` 的參數與結果型別（供 change 3）。
- 本機預設 API socket 路徑解析（§2.2 的優先序規則）：設定檔 §8.2 寫「省略則照 HERDR 規則
  找預設」，這是 HERDR 知識，放在本 crate。
- 合約測試：以 protocol 20 與 22 兩份 schema 為 fixture，驗我們序列化的 request 與解析用的
  fixture（§5.2、§10.1）。
- 假 HERDR（client 層）：程序內假 server，供本 crate 測試，並以 feature 供 change 1b 的
  連線迴圈測試重用（§10.1）。
- 真機 `#[ignore]` 測試與執行說明。
- 沒有 **BREAKING**：全新 crate，沒有既有使用者。

## Capabilities

### New Capabilities

- `herdr-transport`：建立一條 NDJSON 連線的三種方式、連線失敗分類、描述字串、本機預設
  socket 路徑解析。
- `herdr-request`：單次 request／response 往返的語意與錯誤對應；只提供 observer 子集的
  method。
- `herdr-event-subscription`：長連線訂閱的建立、事件分軌、壞行與結束語意、子程序橋接下的
  即時性。
- `herdr-observer-types`：snapshot 與事件 payload 的解析規則、`AgentStatus` 五值與未知值、
  兩個 protocol 版本的合約。

### Modified Capabilities

無。`openspec/specs/` 目前為空，本 change 是第一個。

## 非目標

| 非目標 | 對應設計文件章節 |
|---|---|
| 呼叫任何會改變 HERDR 狀態的 method；提供其型別 | ADR-0001、§3 決策 1、§4.2 末段 |
| 連線迴圈、退避、ReopenStatus、WSL 探測的實作 | §4.2、§7.1（屬 change 1b；spike 5 只驗證探測指令的行為並記錄） |
| HERDR 型別翻成 `RuntimeSnapshot`／`RuntimeEvent`、`RuntimeStore` | §6、§7.2（屬 change 1b） |
| 建模 `layouts`、`worktree`、`tokens` | §6.1 註、§12、`CONTEXT.md` |
| 訂閱 `pane.output_matched`、`pane.scroll_changed`；使用 `agent_status` 過濾 | §5.2 |
| 對真機呼叫 `pane.read`（只提供型別與序列化） | §2.6（屬 change 3） |
| 自寫 Linux relay（ADR-0002 方案 B） | ADR-0002 Alternatives |
| 從 schema 自動生成型別 | §3 直接定案 |
| HTTP、WebSocket、畫面、設定檔載入 | §8（屬 change 1b） |
| 版本落差的警告與顯示（本 crate 只把 `version`／`protocol` 原樣暴露） | §3 直接定案 |
| request 逾時與重試 | §5.2（request／subscribe 只定義單次連線語意）、§4.2 步驟 7（退避屬連線迴圈）；design.md Non-Goals |

## Impact

- **新檔案**：根 `Cargo.toml`（workspace）、`herdr-client/`（`src/`、`tests/`、
  `tests/fixtures/`、`examples/`、`README.md`）、`docs/research/<執行日期>/change-1a-spikes.md`。
- **新依賴**（版本實作時取當時穩定版）：`tokio`（`net`、`process`、`io-util`、`rt-multi-thread`、
  `macros`、`time`、`sync`）、`serde`、`serde_json`、`thiserror`、`tracing`、`async-trait`；
  dev：`jsonschema`（§15 查證為 0.56.0，draft 2020-12）。Windows named pipe 用 tokio 內建
  `tokio::net::windows::named_pipe`，不引第三方 crate（§4.1）。`CREATE_NO_WINDOW` 以常數硬寫，
  不引 `windows-sys`。
- **真機影響**：spike 與真機測試會連上 Windows 端與 WSL 端的 HERDR，全部唯讀（只送
  `session.snapshot`、`events.subscribe`）。不得用 `herdr server stop` 製造斷線（§10.1、
  `AGENTS.md`）。spike 5 若要重驗「不喚醒虛擬機」需 WSL 虛擬機處於停止狀態，是否執行
  `wsl.exe --shutdown` 由使用者決定，因為它會關掉 WSL 端 HERDR 與其所有 pane。
- **對後續 change 的影響**：change 1b 直接依賴本 crate 的 `Connector`／`Client`／型別與假
  HERDR；任一 spike 不成立時，設計文件 §5、§11 與本 change 的 artifacts 要先修訂
  （`/opsx:update`）才能繼續。
- **證據落差**：本機 HERDR 是 `0.9.0-preview.2026-09-08-62431dbd033b`，研究時讀的原始碼
  HEAD 與它可能有約五天落差（`docs/research/2026-09-13/herdr-source-findings.txt`）。
  凡原始碼推導與真機 spike 結果衝突，以 spike 為準。
