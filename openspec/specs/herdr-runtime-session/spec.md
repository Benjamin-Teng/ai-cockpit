# herdr-runtime-session Specification

## Purpose

定義 `HerdrRuntime` 對單一 HERDR 端點的連線與訂閱管理：連線順序、兩條訂閱合併、pane 集合改變時
重開狀態訂閱、WSL 探測、protocol 版本警告。證據：設計文件 §2.2、§2.3、§2.8（已測 protocol 為
20 與 22）、§4.2、§7.1、ADR-0002、`docs/research/2026-09-13/change-1a-spikes.md` spike 2、3、5。

## Requirements

### Requirement: 只用兩個 method

系統必須只對 HERDR 送出 `session.snapshot` 與 `events.subscribe`，每次 request 或訂閱各開一條連線
（經 `herdr-client`）。

#### Scenario: 假 HERDR 收到的 method 集合

- **WHEN** 完整跑過建立事件流、取 snapshot、重開狀態訂閱
- **THEN** 假 HERDR 收到的 method 只有 `session.snapshot` 與 `events.subscribe`

### Requirement: 建立事件流的順序

系統必須在建立事件流時依序：（`wsl` 型 runtime）探測發行版 → 短連線 `session.snapshot` 取 pane id
清單（seed，不套用到狀態庫）→ 長連線 L 訂閱全部 24 種生命週期 → 長連線 S 對 seed 中每個 pane 各
訂一筆 `pane.agent_status_changed`（seed 沒有 pane 時不開 S）→ 回傳合併後的事件流；任一步失敗即
建立失敗並附原因，已開的連線關閉。

#### Scenario: 連線順序

- **GIVEN** 假 HERDR 的 snapshot 有 3 個 pane
- **WHEN** 建立事件流
- **THEN** 假 HERDR 依序收到 `session.snapshot`、含 24 種訂閱的 `events.subscribe`、含 3 筆
  `pane.agent_status_changed` 的 `events.subscribe`

#### Scenario: seed 沒有 pane

- **GIVEN** 假 HERDR 的 snapshot 沒有 pane
- **WHEN** 建立事件流
- **THEN** 只開 L，不開 S，事件流可用

#### Scenario: 狀態訂閱探測失敗

- **GIVEN** 假 HERDR 讓某個 seed pane 的探測失敗（`pane_not_found`）
- **WHEN** 建立事件流
- **THEN** 建立失敗，原因含 `pane_not_found`，L 連線已關閉

### Requirement: 合併事件流與結束

系統必須把 L 與 S 收到的事件經翻譯後合併成一條流；任一條連線結束或發生 I/O 錯誤時，流送出一個
附原因（含是 L 或 S）的錯誤項後結束；流被釋放時所有連線與子程序關閉；壞行由 `herdr-client` 跳過、
不中斷流。

#### Scenario: 兩條連線的事件都到達

- **GIVEN** 假 HERDR 在 L 推 `tab_created`、在 S 推 `pane.agent_status_changed`
- **WHEN** 讀取事件流
- **THEN** 依到達順序得到 `TabUpserted` 與 `AgentStatusChanged`

#### Scenario: L 關閉

- **GIVEN** 假 HERDR 的 L 腳本為推一筆事件後關閉
- **WHEN** 讀取事件流
- **THEN** 得到該事件、接著一個原因含 `L` 的錯誤項、之後流結束

#### Scenario: 釋放事件流

- **WHEN** 釋放事件流
- **THEN** 假 HERDR 觀察到 L 與 S 連線都被關閉

### Requirement: pane 集合改變時重開狀態訂閱

系統必須在事件流觀察到 `pane_created`、`pane_closed`、`pane_moved`，或取得 snapshot 後 pane id
集合與目前 S 的清單不同時重開 S：先以新清單開新 S 並等到 `subscription_started`，再關舊 S；200 ms
內的多次觸發合併成一次；新清單為空時只關舊 S；新 S 建立失敗時視同 S 連線錯誤（流送錯誤項後結束）。

#### Scenario: 新 pane 加入訂閱

- **GIVEN** 事件流已建立、S 訂閱 `wJ:p1`
- **WHEN** L 推送 `pane_created`（`wJ:p2`）
- **THEN** 假 HERDR 收到新的 `events.subscribe` 含 `wJ:p1`、`wJ:p2`，且舊 S 在新 S 收到
  `subscription_started` 之後才被關閉

#### Scenario: 多次觸發合併

- **WHEN** L 在 100 ms 內連推 3 筆 `pane_created`
- **THEN** 假 HERDR 只多收到一次 S 的 `events.subscribe`，清單含 3 個新 pane

#### Scenario: snapshot 發現集合不同

- **GIVEN** S 訂閱 `wJ:p1`
- **WHEN** 取得的 snapshot 只有 `wJ:p3`
- **THEN** 重開 S、清單為 `wJ:p3`

### Requirement: WSL 探測

系統必須對 `wsl` 型 runtime 在建立事件流之前執行 `wsl.exe --list --running --quiet`（Windows 上以
不開視窗的方式）；stdout 含 NUL byte 時以 UTF-16LE 解碼，否則以 UTF-8 解碼，裁掉 `\r` 與空行得到
清單；判斷依清單是否含該發行版，不看 exit code；發行版不在清單 → 錯誤「WSL 發行版 <名稱> 未啟動」；
指令無法執行或非零結束 → 錯誤「WSL 探測失敗：」加 stderr；兩者都附固定重試間隔 `wsl_probe_secs`。
取 snapshot 不另外探測（連線存活代表虛擬機在跑）。

#### Scenario: UTF-16LE 輸出

- **GIVEN** 探測指令 stdout 為 `Ubuntu-24.04\r\n` 的 UTF-16LE 位元組
- **WHEN** 解析
- **THEN** 清單為 `["Ubuntu-24.04"]`

#### Scenario: UTF-8 輸出

- **GIVEN** stdout 為 `Ubuntu-24.04\r\n` 的 UTF-8 位元組（環境設了 `WSL_UTF8=1`）
- **WHEN** 解析
- **THEN** 清單同上

#### Scenario: 發行版未啟動

- **GIVEN** 清單為空、exit code 為 0
- **WHEN** 探測 `Ubuntu-24.04`
- **THEN** 錯誤訊息為「WSL 發行版 Ubuntu-24.04 未啟動」且附固定重試間隔

#### Scenario: 探測指令失敗

- **GIVEN** 指令不存在或非零結束、stderr 為 `boom`
- **WHEN** 探測
- **THEN** 錯誤訊息以「WSL 探測失敗：」開頭並含 `boom`，附固定重試間隔

### Requirement: 取得 snapshot 與版本警告

系統必須以短連線 `session.snapshot` 取得 snapshot 並翻譯；`protocol` 不在 20..=22 時記 warn 並在
結果附警告字串（含實際 protocol），照常回傳。

#### Scenario: 版本落差

- **GIVEN** 假 HERDR 的 snapshot `protocol` 為 23
- **WHEN** 取得 snapshot
- **THEN** 成功，附含 `23` 的警告字串

#### Scenario: 已測版本無警告

- **GIVEN** `protocol` 為 22
- **WHEN** 取得 snapshot
- **THEN** 沒有警告
