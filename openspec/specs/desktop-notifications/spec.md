# desktop-notifications Specification

## Purpose
定義 Cockpit 畫面開著時的桌面通知：哪些狀態變化會通知、如何避免重複與洗版、何時不打擾，以及使用者如何開關各類通知與授權。
通知只在 Cockpit 頁面開著時發生（含視窗最小化），不使用 service worker（ADR-0005）。

## Requirements

### Requirement: 通知事件

系統必須在頁面收到新的推送狀態時，與「上一份收到的狀態」比對，對下列轉換產生通知事件（僅限該類通知已開啟時）：

| 類別 | 條件 | 預設 |
|---|---|---|
| `blocked` | 某 pane（以 runtime id ＋ pane id 識別）的 `agent_status` 由其他值變成 `blocked` | 開 |
| `done` | 某 pane 的 `agent_status` 由其他值變成 `done` | 關 |
| `failed` | 某 task（以 project id ＋ task id 識別）的 `status` 由其他值變成 `failed` | 開 |
| `completed` | 某 task 的 `status` 由其他值變成 `completed` | 關 |

- 頁面載入後收到的第一份狀態只作為比對基準，不產生事件。
- 上一份狀態中不存在的 pane 或 task（新出現的）不產生事件；`exited` 為真的 pane 不產生事件。
- 通道斷線後重新連上時，與斷線前最後一份狀態比對（不重設基準）。
- 這裡的 `done` 是 HERDR 的「已停下且尚未被看過」，不代表 task 結束；task 是否結束只看 task 的 `status`。
- task 的 `failed`／`completed` 依 `pipeline-domain` 只由標記產生，而標記目前只能由使用者在畫面上操作（`agent-reporting` 不允許 agent 標記）；
  因此這兩類通知目前只在「從另一個視窗或另一台裝置標記」時才會出現。保留這兩類是使用者 2026-10-02 的決定，日後若開放 agent 回報結果即自然生效。
- 比對以推送快照為準：推送合併期間發生又消失的短暫狀態（例如 `blocked` 很快又回到 `working`）不會被看到，因此不通知。

#### Scenario: agent 卡住

- **GIVEN** 頁面已收到一份 pane `win`／`wJ:p1` 為 `working` 的狀態
- **WHEN** 下一份狀態中該 pane 變成 `blocked`，且 `blocked` 通知開啟
- **THEN** 產生一則 `blocked` 通知

#### Scenario: 第一份狀態不通知

- **GIVEN** 頁面剛載入
- **WHEN** 收到的第一份狀態中有 pane 為 `blocked`、task 為 `failed`
- **THEN** 不產生任何通知

#### Scenario: 預設關閉的類別

- **GIVEN** 通知設定為預設值
- **WHEN** 某 pane 由 `working` 變成 `done`，且某 task 由 `running` 變成 `completed`
- **THEN** 不產生通知

#### Scenario: 開啟後的類別

- **GIVEN** 使用者已開啟 `completed` 通知
- **WHEN** 某 task 由 `running` 變成 `completed`
- **THEN** 產生一則 `completed` 通知

#### Scenario: 重連後補報

- **GIVEN** 斷線前最後一份狀態中 task `t1` 為 `running`
- **WHEN** 重新連上後收到的狀態中 `t1` 為 `failed`
- **THEN** 產生一則 `failed` 通知

### Requirement: 通知呈現

系統必須以瀏覽器內建的通知功能呈現事件，規則如下：

- 頁面可見且視窗有焦點（使用者正在看 Cockpit）時，不發出通知。
- 同一份狀態產生 1 至 3 個事件時逐一發出；超過 3 個時只發一則合併通知，標題為「Cockpit：N 件事需要注意」（N 為事件數），內文列出前 3 件、其後以「…」表示。
- 單一事件的標題依類別為「agent 卡住」「agent 停下等你看」「task failed」「task completed」；內文：pane 事件為「`<runtime> / <pane id>`」，若有
  workstream 綁定到該 pane（`binding.state` 為 `bound` 且 runtime 與 pane id 相符）再加上「`（<workstream 名稱>）`」——多個時名稱去重後以「、」串接；
  task 事件為「`<project 名稱>：<task 標題>`」。
- 每則通知設定識別標籤，同一個 pane 或 task 的同類事件以新通知取代舊通知，且取代時仍重新提醒使用者（不靜默取代）。
- 點選通知時將 Cockpit 視窗帶到前景並關閉該通知；pane 事件另外選定該 pane，效果等同點選該 pane 列（右欄選定標示、Live Output 顯示該 pane）。
  改綁模式期間或該 pane 已不在最新狀態中時，只帶到前景、不改變選取。
- 瀏覽器不支援通知或權限不是「已允許」時，不發出通知、不報錯。

#### Scenario: 正在看畫面時不打擾

- **GIVEN** Cockpit 頁面可見且視窗有焦點
- **WHEN** 某 pane 變成 `blocked`
- **THEN** 不發出通知

#### Scenario: 合併通知

- **GIVEN** 視窗不在前景
- **WHEN** 同一份狀態中有 4 個 task 變成 `failed`
- **THEN** 只發出一則標題為「Cockpit：4 件事需要注意」的通知

#### Scenario: 通知內容

- **GIVEN** workstream `backend` 綁定到 `win`／`wJ:p2`，視窗不在前景
- **WHEN** `wJ:p2` 變成 `blocked`
- **THEN** 通知標題為「agent 卡住」，內文為「win / wJ:p2（backend）」

#### Scenario: 點通知帶出 pane

- **GIVEN** 已發出 `wJ:p2` 的 `blocked` 通知
- **WHEN** 使用者點選該通知
- **THEN** Cockpit 視窗回到前景，右欄 `wJ:p2` 列出現選定標示，Live Output 顯示 `win`／`wJ:p2`

#### Scenario: 改綁模式中點通知

- **GIVEN** 頁面在改綁模式，目前選定 `wJ:p1`，已發出 `wJ:p2` 的 `blocked` 通知
- **WHEN** 使用者點選該通知
- **THEN** Cockpit 視窗回到前景，選取仍是 `wJ:p1`

### Requirement: 通知設定

系統必須在頂列提供通知設定入口（鈴鐺按鈕，可用鍵盤聚焦與操作），開啟的設定面板包含：

- 四個類別各一個開關，預設 `blocked`、`failed` 開，`done`、`completed` 關；變更立即生效，存在瀏覽器本機儲存，重新載入與重開視窗後保留；
  本機儲存不可用或內容損毀時使用預設值、不報錯。
- 目前的通知權限狀態：尚未決定時顯示「允許通知」按鈕（按下才向瀏覽器請求權限）；已封鎖時顯示說明，指引使用者到瀏覽器的網站設定解除；
  瀏覽器不支援通知時顯示不支援的說明。
- 四個開關的標籤為 `agent blocked`、`agent done`、`task failed`、`task completed`（各附一句中文說明；不出現「完成」二字，見 `cockpit-dashboard`
  「畫面整頁重畫」）。
- 設定面板不在整頁重畫的範圍內，整頁重畫不得關閉已開啟的面板或改變其內容。開啟時焦點移到第一個開關；按 Esc、再按一次鈴鐺、或點面板與鈴鐺以外的位置
  關閉，關閉後焦點回到鈴鐺（若焦點原本在面板內）。面板的文字對比、焦點外框與減少動態設定依「Direction 01 視覺語彙」。

#### Scenario: 預設值

- **GIVEN** 本機儲存中沒有通知設定
- **WHEN** 開啟設定面板
- **THEN** `blocked`、`failed` 為開，`done`、`completed` 為關

#### Scenario: 設定保留

- **GIVEN** 使用者關閉 `blocked`、開啟 `done`
- **WHEN** 重新載入頁面並開啟設定面板
- **THEN** `blocked` 為關、`done` 為開

#### Scenario: 請求權限

- **GIVEN** 通知權限尚未決定
- **WHEN** 使用者在設定面板按「允許通知」
- **THEN** 瀏覽器跳出權限詢問；允許後面板顯示已允許

#### Scenario: 重畫不關閉面板

- **GIVEN** 設定面板已開啟
- **WHEN** 收到新的推送狀態並整頁重畫
- **THEN** 面板仍開啟，開關狀態不變
