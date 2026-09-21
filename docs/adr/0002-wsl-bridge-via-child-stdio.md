# ADR-0002 WSL 端 HERDR 經子程序 stdio 傳話，第一版用 nc

- Status: Accepted
- Date: 2026-09-13

## Context

使用者同時有 Windows HERDR 與 WSL HERDR，兩個 server 互不相通。WSL 端的 AF_UNIX
socket 位於 Linux 虛擬機內，Windows 程式開不到。HERDR 協定是 NDJSON，且每條連線只
服務一個 method（訂閱連線長開、請求連線一問一答）。WSL Ubuntu 內建 OpenBSD netcat，
`nc -U` 可把 unix socket 接到 stdio。

## Decision

`herdr-client` 提供 `ChildStdioConnector`：每次要開連線就啟動一個子程序，把它的
stdin／stdout 當 NDJSON 連線。WSL 端第一版用 `wsl.exe -d <distro> -e nc -U <socket>`。
Windows 上子程序一律加 `CREATE_NO_WINDOW`，不與 `DETACHED_PROCESS` 併用。連線前先以
`wsl.exe --list --running --quiet` 探測發行版是否在跑，未跑就不啟動子程序。

## Alternatives rejected

- 自寫 Linux relay（方案 B）：行為可控但多一個建置目標。保留為升級路徑：當 change 3
  的 `pane.read` 頻率讓「每次請求啟動一個 wsl.exe」成為瓶頸時改用，只換 Connector。
- 兩側各跑一個 Cockpit、前端合併（方案 C）：兩個程序、兩個網址、跨 runtime 的 Project
  難表達。

## Consequences

- 每條連線一個子程序：WSL 端每個 runtime 常駐兩個 nc（生命週期訂閱與每 pane 狀態訂閱），
  每次 snapshot 與每次重開狀態訂閱各再啟動一次 `wsl.exe`（約 0.1–0.3 秒，2026-09-13 當時的估計值；
  2026-09-19 實測數字見下）。change 1 的頻率是連線時三次、之後每 30 秒一次與 pane 集合改變時一次，
  可接受。
- 不探測就連會反覆喚醒已休眠的 WSL 虛擬機；探測步驟不可省略。
  `wsl.exe --list --running --quiet` 已於 2026-09-13 實測不喚醒虛擬機。
- nc 的緩衝與半關閉行為列為 change 1a 的 spike。
- **2026-09-19 實測（change 3 `live-output` 設計前 spike）**：單次 `pane.read` 經這條
  `ChildStdioConnector` 路徑，WSL 端約 42 ms、Windows 端約 1.2 ms，且與回應大小無關（37 B 到
  6 KB 同速）。上面「約 0.1–0.3 秒」的估計比實測高出一個數量級。結論：**change 3 Live Output
  每秒一次的讀取頻率不需要方案 B**（自寫 Linux relay）；方案 B 仍保留為升級路徑，評估時機不變
  ——更高頻率輪詢或多個 pane 同時被看時再評估。數字來源與量測方法見
  `docs/research/2026-09-19/pane-read-probe.md` 第 1 節。
