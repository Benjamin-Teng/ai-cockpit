# cockpit-dashboard（delta）

## MODIFIED Requirements

### Requirement: Factory Floor

系統必須在中上區域畫出目前選定 Project（見「Project 切換」）的 Factory Floor：標題為 Project `name`，有
`warnings` 時逐則顯示；區域內是一張網格，欄為 `stages`（設定順序）、列為 workstreams（設定順序）；每列列首顯示
workstream `name` 與綁定摘要（`bound` 顯示 runtime 與 pane id，`source` 為 `override` 時加「改綁」標示；`unbound`
顯示「未綁定」；`ambiguous` 顯示「歧義」與候選數；`runtime_disconnected` 顯示「runtime 未連線」；`none` 顯示
「無綁定」）；workstream 的 `activity_undeclared` 為 `true` 時，列首另顯示文字提示「工作中・未宣告 task」（以警示色
呈現、不得有動畫），為 `false` 時不顯示；每個 task 以節點出現在所屬 workstream 列與目前 `stage` 欄交會的格子，同格多個 task 依設定順序排列；
不畫依賴箭頭。節點以表面色為底、左緣一條狀態色條，並顯示 `title` 與「狀態符號＋`status` 文字」，狀態文字與色條
使用狀態色：`running` 品牌強調色、`blocked` 警示色、`ready` 主要文字色、`pending` 次要文字色、`failed` 失敗色、
`completed` 成功色；`running` 節點另有靜止的強調（品牌強調色的較粗外框與柔光），**不得**有動畫。`completed`
不得與 pane `done` 使用同一種顏色。網格寬度超出區域時，區域內部可橫向捲動，頁面本身不得出現橫向捲軸。

#### Scenario: Scenario D 的畫面

- **GIVEN** 投影中 Project `p` 的 stages 為 `Plan`、`Implement`、`Test`，workstream `backend`、`frontend`、
  `tests` 各有一個 `running` 的 task，分別在 `Implement`、`Plan`、`Test`
- **WHEN** 以瀏覽器開啟 `/`
- **THEN** 網格有三列三欄，三個 `running` 節點分別位於（backend, Implement）、（frontend, Plan）、（tests, Test），
  狀態文字為品牌強調色並有強調外框

#### Scenario: 未宣告 task 的提示

- **GIVEN** 投影中 workstream `backend` 的 `activity_undeclared` 為 `true`、`frontend` 為 `false`
- **WHEN** 重畫
- **THEN** `backend` 列首含「工作中・未宣告 task」，`frontend` 列首不含；停在此狀態再收到一份新投影整頁重畫後結果相同

#### Scenario: 兩個 Project

- **GIVEN** 投影有 Project `p1`、`p2`，使用者尚未選定任何 Project
- **WHEN** 重畫
- **THEN** 中上區域只有一張 Factory Floor，顯示 `p1`；`p2` 的網格不在畫面上，改由左欄切換（見「Project 切換」）

#### Scenario: running 節點沒有動畫

- **GIVEN** 某 task 的 `status` 為 `running`
- **WHEN** 重畫後讀取該節點與其偽元素的計算樣式
- **THEN** `animation-name` 皆為 `none`

#### Scenario: 未知 status 不破壞畫面

- **GIVEN** 某 task 的 `status` 為 `whatever`
- **WHEN** 重畫
- **THEN** 該節點以次要文字色與虛線外框顯示原字串，其他節點正常

#### Scenario: 網格過寬時不撐破頁面

- **GIVEN** 選定 Project 有 10 個 stages，視窗寬 1536
- **WHEN** 重畫
- **THEN** Factory Floor 區域內可橫向捲動看到最後一欄，`document.documentElement.scrollWidth` 不大於視窗寬度

### Requirement: 畫面操作

系統必須在畫面上提供下列操作，並以 `pipeline-progress` 的寫入端點送出：task 節點上，`mark` 為 `none` 且
不在最後一個 stage 時顯示「推進」；`mark` 為 `none` 且不在第一個 stage 時顯示「退回」；`mark` 為 `none` 時顯示「Completed」與「Failed」；`mark` 不是 `none`
時只顯示「清除標記」。workstream 列首顯示「改綁」；`binding.source` 為 `override` 時另顯示「取消改綁」。
按「改綁」進入改綁模式：頁面顯示指出目標 workstream 的提示與「取消」，所有 `connected` runtime 卡中未
exited 的 pane 列出現「綁定到這裡」，按下即送出覆蓋，成功或按「取消」即離開改綁模式。寫入端點回非 2xx
或請求失敗時，頁面顯示錯誤訊息（含回應本體的 `error`），直到下一次操作或使用者關閉；操作成功後畫面
不自行修改狀態，一律等 `/ws` 推送的新投影重畫。

#### Scenario: 推進按鈕

- **GIVEN** task `t1` 在 `Plan`（非最後一站）、`mark` 為 `none`
- **WHEN** 在畫面按 `t1` 的「推進」
- **THEN** 服務收到 `POST /api/projects/p/tasks/t1/advance`；新投影到達後節點出現在下一站的欄

#### Scenario: 退回按鈕

- **GIVEN** stages 為 `Plan`、`Build`，task `t1` 在 `Build`、`mark` 為 `none`；task `t2` 在 `Plan`、`mark` 為 `none`；
  task `t3` 在 `Build`、`mark` 為 `failed`
- **WHEN** 重畫後在畫面按 `t1` 的「退回」
- **THEN** `t2` 與 `t3` 的節點沒有「退回」；服務收到 `POST /api/projects/p/tasks/t1/retreat`；新投影到達後 `t1` 的節點出現在
  `Plan` 欄

#### Scenario: 改綁模式跨重畫保留

- **GIVEN** 已按 workstream `be` 的「改綁」進入改綁模式
- **WHEN** 期間收到兩份新投影並整頁重畫
- **THEN** 改綁提示與各 pane 列的「綁定到這裡」仍在；按其中一列後服務收到對應的 `PUT` 覆蓋請求

#### Scenario: 頻繁重畫時按鈕仍有效

- **GIVEN** 投影每 100 ms 推送一份新的 version
- **WHEN** 連續按 10 次不同 task 的「Completed」
- **THEN** 服務收到 10 個對應的 `POST` 請求，沒有因為重畫而遺失

#### Scenario: 被拒絕時顯示原因

- **GIVEN** 服務對某次操作回 409，本體 `{"error":"已有標記"}`
- **WHEN** 該操作完成
- **THEN** 頁面顯示含「已有標記」的錯誤訊息，之後的重畫不清除它
