# herdr-transport Specification

## Purpose

定義 Cockpit 與 HERDR API socket 之間建立一條 NDJSON 連線的方式：Windows named pipe、
unix domain socket、以及經子程序 stdio 的橋接（WSL 端用），並統一連線失敗的分類，
讓上層不必知道 transport 差異。證據：設計文件 §2.2、ADR-0002、
`docs/research/2026-09-13/herdr-source-findings.txt` §2 到 §5。

## Requirements

### Requirement: 每次呼叫建立一條全新的 NDJSON 連線

系統必須在每次要求連線時建立一條全新的連線，提供「送出一行」與「讀取一行」的介面，
且不得在同一條連線上多工多個 method。理由：HERDR 每條連線只服務一個 method，
一般 request 回一行後由 server 關閉，`events.subscribe` 則持續推送（設計文件 §2.2）。

#### Scenario: 連續兩次連線互相獨立

- **GIVEN** 假 HERDR 在本機 transport 上監聽
- **WHEN** 連續呼叫兩次連線
- **THEN** 得到兩條互不共享狀態的連線，各自可送一行、讀一行；關閉其中一條不影響另一條

### Requirement: Windows named pipe 連線

系統必須在 Windows 上以 named pipe 連上 HERDR，pipe 名稱為 `\.\pipe\` 接上 HERDR 回報的
socket 檔案完整路徑（含磁碟機冒號與反斜線，例如
`\.\pipe\C:\Users\<user>\AppData\Roaming\herdr\herdr.sock`），且不得解析 `herdr.sock`
檔案的內容（其內容 `<pid>:<納秒時間戳>` 是 server 自用標記；`herdr-source-findings.txt`
§2b、§2c）。

#### Scenario: 以含冒號與反斜線的路徑連線

- **GIVEN** Windows 上有 named pipe server 監聽在 `\.\pipe\C:\...\herdr.sock` 形式的名稱
- **WHEN** 以該 socket 檔案路徑建立連線
- **THEN** 連線成功，且可完成一次送一行、讀一行

#### Scenario: pipe instance 忙碌時重試

- **GIVEN** server 的 pipe instance 暫時全部忙碌（作業系統回 `ERROR_PIPE_BUSY`）
- **WHEN** 建立連線
- **THEN** 系統在有限時間內重試，instance 釋出後連線成功；超過上限則回 Io 類錯誤，
  不無限等待

### Requirement: unix domain socket 連線

系統必須在 unix 平台以 AF_UNIX socket 連上指定路徑（WSL 端預設
`/home/<user>/.config/herdr/herdr.sock`，設計文件 §2.1）。

#### Scenario: 連上 unix socket

- **GIVEN** unix 平台上有 server 監聽於某 socket 路徑
- **WHEN** 以該路徑建立連線
- **THEN** 連線成功，可送一行、讀一行

### Requirement: 子程序 stdio 橋接連線

系統必須能以任意指令列啟動子程序，把子程序的 stdin／stdout 當作一條 NDJSON 連線，
子程序的 stderr 進入日誌而不混入資料流；在 Windows 上啟動子程序不得出現新視窗
（ADR-0002：`CREATE_NO_WINDOW`，不與 `DETACHED_PROCESS` 併用）。

#### Scenario: 經子程序完成一次往返

- **GIVEN** 一個把 stdin 轉送到假 HERDR、再把回應寫到 stdout 的子程序指令
- **WHEN** 以該指令建立連線並送一行
- **THEN** 讀到對應的一行回應，且子程序寫到 stderr 的內容不出現在讀到的資料中

#### Scenario: Windows 上不出現額外視窗

- **GIVEN** 在 HERDR pane 內啟動的程式
- **WHEN** 它以子程序橋接方式連上 WSL 端 HERDR 並完成一次往返
- **THEN** 桌面上不出現任何新的主控台視窗

#### Scenario: 連線釋放時子程序終止

- **GIVEN** 一條經子程序建立的連線
- **WHEN** 該連線被丟棄
- **THEN** 子程序在短時間內結束，不留下孤兒程序

### Requirement: 連線失敗分類

系統必須在建立連線時、或最遲在該連線第一次讀取時，把失敗分成三類並附可讀的原因文字：
ServerNotRunning（目標不存在或拒絕連線；或橋接子程序在送出任何資料前就結束）、
Spawn（子程序無法啟動）、Io（其他 I/O 錯誤）。分類依據：HERDR 自己以
`io::ErrorKind::NotFound` 與 `ConnectionRefused` 判定 server 未啟動（設計文件 §2.2）。

#### Scenario: named pipe 或 socket 不存在

- **GIVEN** 指定的 pipe 或 socket 路徑沒有任何 server
- **WHEN** 建立連線
- **THEN** 得到 ServerNotRunning

#### Scenario: 橋接子程序立即結束

- **GIVEN** 子程序指令啟動後因目標不存在而立刻以非零狀態結束
- **WHEN** 建立連線並在該連線上送一行、讀一行
- **THEN** 得到 ServerNotRunning，原因文字含子程序 stderr 的內容

#### Scenario: 指令不存在

- **GIVEN** 子程序指令列指向不存在的執行檔
- **WHEN** 建立連線
- **THEN** 得到 Spawn

### Requirement: 連線描述字串

系統必須為每種連線提供一個給人看的描述字串，指出 transport 種類與目標（路徑或指令列），
供畫面與日誌顯示（設計文件 §5.1 `describe`）。

#### Scenario: 描述 named pipe 連線

- **WHEN** 詢問一個 named pipe 連線器的描述
- **THEN** 得到含 transport 種類與 socket 路徑的非空字串

#### Scenario: 描述子程序連線

- **WHEN** 詢問一個子程序連線器的描述
- **THEN** 得到含 transport 種類與完整指令列的非空字串

### Requirement: 本機預設 socket 路徑解析

系統必須能依 HERDR 的規則解析「執行 Cockpit 的這台機器」上的預設 API socket 路徑：
環境變數 `HERDR_SOCKET_PATH` 優先；其次環境變數 `HERDR_SESSION` 對應
`<設定目錄>/sessions/<name>/herdr.sock`；否則 `<設定目錄>/herdr.sock`。設定目錄依 HERDR 原始碼
`src/config/io.rs:30-68`（commit `bafbc0949`）的規則：環境變數 `XDG_CONFIG_HOME` 有設（空字串也算）
就用 `<XDG_CONFIG_HOME>/herdr`，**不分平台**；否則 Windows 依序取 `%APPDATA%\herdr`、
`%USERPROFILE%\AppData\Roaming\herdr`、`$HOME/.config/herdr`，unix 取 `$HOME/.config/herdr`，都沒有
時退到暫存目錄下的 `herdr`（設計文件 §2.2、`herdr-source-findings.txt` §5、task 2.3 查證）。WSL 端的
路徑不在此解析，由設定檔提供。

#### Scenario: 沒有環境變數

- **GIVEN** 未設定 `HERDR_SOCKET_PATH` 與 `HERDR_SESSION`
- **WHEN** 解析預設路徑
- **THEN** 得到 `<設定目錄>/herdr.sock`

#### Scenario: HERDR_SOCKET_PATH 優先

- **GIVEN** 同時設定 `HERDR_SOCKET_PATH` 與 `HERDR_SESSION`
- **WHEN** 解析預設路徑
- **THEN** 得到 `HERDR_SOCKET_PATH` 的值

#### Scenario: XDG_CONFIG_HOME 跨平台優先

- **GIVEN** 設定 `XDG_CONFIG_HOME` 為 `/x`，且 `APPDATA` 也有值
- **WHEN** 在任一平台解析預設路徑
- **THEN** 得到 `/x/herdr/herdr.sock`

#### Scenario: HERDR_SESSION 對應 sessions 子目錄

- **GIVEN** 只設定 `HERDR_SESSION` 為 `dev`
- **WHEN** 解析預設路徑
- **THEN** 得到 `<設定目錄>/sessions/dev/herdr.sock`
