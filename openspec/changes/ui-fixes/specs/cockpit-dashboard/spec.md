# cockpit-dashboard（delta）

## MODIFIED Requirements

### Requirement: 畫面整頁重畫

系統必須讓 `channel.js` 只負責連 `/ws`、收到訊息就呼叫 `onState(state)`、斷線後以 1、2、4、8 秒
退避（上限 8 秒）重連，且只在重連後收到第一則訊息時才把退避歸零（連上後立刻被關閉不歸零）；收到無法
解析為 JSON 的訊息時略過該則並在 console 記警告，通道不中斷。`render.js` 每次收到整張圖就整頁重畫，版面依
「版面與窄視窗」：頂列（產品名稱、每個 runtime 一個連線燈號：runtime `id` 與連線狀態）；左欄 Project 分頁的內容（見
「Project 切換」）；中上 Factory Floor（見「Factory Floor」）；右欄每個 runtime 一張卡（`id`、`endpoint`、連線
狀態與原因或 protocol 警告、server 版本、最後 snapshot 時間），卡內依 workspace 分組（標籤、number、彙總狀態），
每個 pane 一列（id、agent 名稱或 `shell`、agent 狀態、標題、cwd），卡片之下是最近事件；底列（通道狀態、
`version`）。agent 狀態依「Direction 01 視覺語彙」以符號＋文字＋色彩呈現：`working` 用品牌強調色、`blocked`
用警示色、`idle` 用次要文字色、`done` 用主要文字色加品牌強調色的空心標記（**不得**使用成功色，避免被讀成
task 完成）、`unknown` 與任何未知字串用次要文字色並顯示原字串、`exited` 加刪除線。連線狀態：`connected` 用
成功色、`connecting` 用警示色、`disconnected` 用失敗色。可點的互動只有「畫面操作」所列的按鈕、「Project 切換」
的 Project 選取、`live-output`「選定一個 pane」所列的選取操作，與 `file-review` 所列的左欄分頁、檔案樹、檔案分頁與
檢視器內的操作；pane 的 `done` 顯示為 `done`，不出現「完成」字樣。整頁重畫不得清除進行中的畫面操作狀態（改綁模式、
最近一次操作的錯誤訊息）與 Project 選取，也不得清除 Live Output 的選取、面板內容與捲動位置——Live Output 面板、左欄的
分頁列與檔案樹、中欄下半部的分頁區都不屬於整頁重畫的範圍。整頁重畫也不得讓鍵盤焦點消失：
重畫前焦點若在 `#app` 內某個可互動的元素上（pane 列、「畫面操作」的按鈕、「看輸出」、Project 項目等），重畫後
焦點必須落在代表同一個對象、同一個操作的新元素上；該元素在新畫面中已不存在或已不可互動時，焦點才可以離開。焦點還原後是否
呈現焦點外框，依使用者最近一次的輸入方式決定：最近一次是滑鼠（pointer）操作（含該操作觸發的重畫與其後的背景重畫）時，
焦點仍必須還原到新元素上，但該元素不得呈現焦點外框（不匹配 `:focus-visible`）；最近一次是鍵盤操作時，焦點還原且焦點外框
照常呈現（見「Direction 01 視覺語彙」）。滑鼠操作之後使用者改用鍵盤（例如按 Tab）移動焦點時，焦點外框照常出現。
「最近一次輸入方式」另有兩條例外，比照瀏覽器原生的 `:focus-visible` 判定：(1) 帶 Alt、Ctrl 或 Meta 的按鍵（含單按
這些鍵）不改變「最近一次輸入方式」，Shift 與一般按鍵才算鍵盤操作；(2) 滑鼠操作沒有移動焦點（例如按住捲軸）時，原本呈現
焦點外框的元素在其後的重畫仍照常呈現焦點外框；在畫面元素上以滑鼠觸發的操作（會移動或重設焦點的點擊）一律不呈現。整頁重畫不得重置
各區塊內部的捲動位置。當頁面與 cockpit
服務的通道（見「通道重連」）不是 `connected`（即 `disconnected` 或 `connecting`）時，畫面上來自投影的即時狀態一律改為
「最後已知」的非即時呈現：頂列每個 runtime 的連線燈號一律改用 `--text-dim`、不使用連線狀態原本對應的顏色，且狀態文字前加
「最後已知」（例如「最後已知：connected」）；右欄每張 runtime 卡的連線狀態（符號與文字）同樣改用 `--text-dim`、不使用連線
狀態原本對應的顏色，且連線狀態文字前加「最後已知」；右欄 runtime 卡內 workspace、tab 與 pane 列的 agent 狀態（符號與文字），
中欄 Factory Floor 內以狀態色表達的元素（task 節點的狀態色條、狀態文字與 `running` 節點的強調外框、workstream 列首的
「未宣告 task」提示），以及左欄 project 項目的狀態計數，一律改用 `--text-dim`
（狀態文字與符號本身仍保留，只是不再用狀態色）；這些非即時呈現仍須符合「Direction 01 視覺語彙」的文字對比。通道恢復
`connected` 後，上述各處才依實際狀態還原對應顏色與文字。

#### Scenario: 兩個 runtime 的畫面

- **GIVEN** 服務提供一份含 `win`（connected）與 `wsl`（disconnected，附原因）的投影
- **WHEN** 以瀏覽器開啟 `/`
- **THEN** 頂列出現兩個連線燈號（`win` 為成功色、`wsl` 為失敗色，皆附文字）；右欄出現兩張卡，`wsl` 卡顯示原因；
  每個 pane 一列且 agent 狀態的符號、文字與色彩對應狀態；底列顯示 version

#### Scenario: done 不使用成功色

- **GIVEN** 某 pane 的 `agent_status` 為 `done`，某 task 的 `status` 為 `completed`
- **WHEN** 重畫
- **THEN** 該 pane 列顯示 `done` 文字，其狀態文字的計算顏色不等於成功色，也不等於 `completed` 節點狀態文字的
  計算顏色；頁面任何位置不出現「完成」字樣

#### Scenario: 未知狀態不破壞畫面

- **GIVEN** 某 pane 的 `agent_status` 為 `whatever`
- **WHEN** 重畫
- **THEN** 該列以次要文字色顯示原字串，其他列正常

#### Scenario: 通道重連

- **GIVEN** 頁面已連上
- **WHEN** 服務停止再啟動
- **THEN** 底列通道狀態先顯示斷線、恢復後顯示已連線並重畫最新一份

#### Scenario: 通道斷線時 runtime 燈號標示為最後已知

- **GIVEN** 頂列顯示 `win` 為 connected
- **WHEN** 頁面與 cockpit 服務的通道斷線
- **THEN** 頂列 `win` 的燈號文字含「最後已知」，且不使用成功色；通道恢復後還原

#### Scenario: 通道斷線時 runtime 卡顯示最後已知

- **GIVEN** 右欄 `win` 的 runtime 卡顯示連線狀態 `connected`（成功色）
- **WHEN** 頁面與 cockpit 服務的通道斷線
- **THEN** `win` 卡的連線狀態文字含「最後已知」且仍可讀到 `connected`，其計算顏色為 `--text-dim`、不是成功色；
  通道恢復 `connected` 後，該卡回到「最後已知」字樣消失、連線狀態為成功色

#### Scenario: 通道斷線時中欄與 pane 列的狀態色轉暗

- **GIVEN** 某 pane 的 `agent_status` 為 `working`，綁定它的 task 節點狀態為 `running`
- **WHEN** 頁面與 cockpit 服務的通道斷線
- **THEN** 該 pane 列的 agent 狀態符號與文字、該 task 節點的狀態色條與狀態文字，計算顏色皆為 `--text-dim`、不是品牌強調色；
  狀態文字仍為 `working`／`running`；左欄該 project 的狀態計數與任何「未宣告 task」提示的計算顏色也為 `--text-dim`；
  按鈕（含「取消改綁」）不轉暗、仍可操作；通道恢復 `connected` 後各處還原為原本的狀態色

#### Scenario: 斷線中整頁重畫後仍為最後已知

- **GIVEN** 頁面與 cockpit 服務的通道已斷線，上述各處已轉為 `--text-dim`
- **WHEN** 使用者點選某個 pane 列觸發整頁重畫
- **THEN** 重畫後右欄 runtime 卡仍標「最後已知」，pane 列與 task 節點的狀態色仍為 `--text-dim`

#### Scenario: 連上即斷不歸零退避

- **GIVEN** 服務接受 WebSocket 後立即關閉、不送任何訊息，重複發生
- **WHEN** 觀察重連間隔
- **THEN** 間隔依 1、2、4、8、8 秒遞增，不回到 1 秒

#### Scenario: 壞訊息不中斷

- **GIVEN** 頁面已連上
- **WHEN** 收到一則 `{not json`，接著收到一份合法投影
- **THEN** 第一則被略過、console 有警告，第二則正常重畫，通道狀態維持已連線

#### Scenario: 重畫不丟鍵盤焦點

- **GIVEN** 焦點在某個 task 節點的「推進」按鈕上，投影每 100 ms 推送一份新的 version
- **WHEN** 經過 2 秒後按 Enter
- **THEN** 這段期間焦點始終在該 task 的「推進」按鈕上（節點可以是新的），按 Enter 後服務收到該 task 的 `advance` 請求

#### Scenario: 滑鼠點擊後重畫不呈現焦點外框

- **GIVEN** 右欄某 pane 列，投影每 100 ms 推送一份新的 version
- **WHEN** 以滑鼠點該 pane 列，並經過 1 秒讓整頁重畫數次
- **THEN** 焦點仍在代表同一個 pane 的新 pane 列上，且該元素不匹配 `:focus-visible`、沒有焦點外框

#### Scenario: 鍵盤觸發的重畫保留焦點外框

- **GIVEN** 以鍵盤 Tab 把焦點移到某 task 節點的「推進」按鈕（外框可見）
- **WHEN** 按 Enter 觸發推進，服務回 204、新投影到達並重畫
- **THEN** 焦點在新畫面中代表同一個 task 的按鈕（或該 task 在新畫面中仍可互動的對應按鈕）上，且該元素匹配 `:focus-visible`、焦點外框可見

#### Scenario: 滑鼠點擊之後按 Tab 外框照常出現

- **GIVEN** 以滑鼠點了某 pane 列，焦點外框不可見
- **WHEN** 按 Tab 把焦點移到下一個可互動元素
- **THEN** 該元素匹配 `:focus-visible`，焦點外框照常出現

#### Scenario: 修飾鍵不改變最近一次輸入方式

- **GIVEN** 以滑鼠點了某 pane 列，焦點外框不可見，投影每 100 ms 推送一份新的 version
- **WHEN** 單按 Alt、Ctrl（不接其他鍵），並經過 1 秒讓整頁重畫數次
- **THEN** 焦點仍在代表同一個 pane 的新 pane 列上，且該元素不匹配 `:focus-visible`、焦點外框不出現

#### Scenario: 不移動焦點的滑鼠操作不洗掉焦點外框

- **GIVEN** 以鍵盤 Tab 把焦點移到某 task 節點的「推進」按鈕（外框可見），投影每 100 ms 推送一份新的 version
- **WHEN** 以滑鼠按住一個不會移動焦點的位置（例如可捲容器的捲軸），並經過 1 秒讓整頁重畫數次
- **THEN** 焦點仍在代表同一個 task 的「推進」按鈕上，且該元素匹配 `:focus-visible`、焦點外框仍在

#### Scenario: 頻繁重畫不影響 Live Output 面板

- **GIVEN** 已選定一個 pane、面板內容超過一屏且使用者已往上捲，投影每 100 ms 推送一份新的 version
- **WHEN** 經過 3 秒
- **THEN** 面板的 DOM 節點沒有被換掉，捲動位置不變，選定標示仍在

#### Scenario: 頻繁重畫不影響檔案分頁

- **GIVEN** 已打開一個 Markdown 檔案分頁且往下捲動，投影每 100 ms 推送一份新的 version
- **WHEN** 經過 3 秒
- **THEN** 分頁區與檔案內容的 DOM 節點沒有被換掉，捲動位置不變，目前分頁不變

#### Scenario: 重畫不重置區塊內部捲動位置

- **GIVEN** 視窗寬 ≥1200 且高 ≥720（固定一屏），使用者已把 Factory Floor 與右欄的內部捲動容器都捲到非 0 的位置
- **WHEN** 收到新投影並整頁重畫
- **THEN** 兩個容器的捲動位置都不變，頁面本身也沒有捲動

### Requirement: Factory Floor

系統必須在中上區域畫出目前選定 Project（見「Project 切換」）的 Factory Floor：標題為 Project `name`，有
`warnings` 時逐則顯示；區域內是一張網格，欄為 `stages`（設定順序）、列為 workstreams（設定順序）；每列列首顯示
workstream `name` 與綁定摘要（`bound` 顯示 runtime 與 pane id，`source` 為 `override` 時加「改綁」標示；`unbound`
顯示「未綁定」；`ambiguous` 顯示「歧義」與候選數；`runtime_disconnected` 顯示「runtime 未連線」，`source` 為 `override`
時同樣加「改綁」標示；`none` 顯示「無綁定」）；workstream 的 `activity_undeclared` 為 `true` 時，列首另顯示文字提示「工作中・未宣告 task」（以警示色
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

#### Scenario: 覆蓋造成的斷線仍顯示改綁標示

- **GIVEN** workstream `be` 的覆蓋指向 `wsl`／`w1:p1`，`wsl` 為 `disconnected`（`be` 的 `binding` 為
  `{"state":"runtime_disconnected","runtime":"wsl","source":"override"}`），workstream `fe` 為自動綁定且其 runtime 斷線
- **WHEN** 重畫
- **THEN** `be` 的綁定摘要顯示「runtime 未連線」與「改綁」標示（徽章）；`fe` 的綁定摘要顯示「runtime 未連線」、沒有「改綁」標示
  （此處的標示指綁定摘要內的徽章，與列首的「改綁」按鈕不同）

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
時只顯示「清除標記」。workstream 列首顯示「改綁」；`binding.source` 為 `override` 時另顯示「取消改綁」，包含綁定狀態為 `runtime_disconnected`、
且 `source` 為 `override`（覆蓋造成的斷線）的情況；`source` 為 `auto` 時不顯示「取消改綁」。
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

#### Scenario: 覆蓋造成的斷線可取消改綁

- **GIVEN** workstream `be` 的覆蓋指向 `wsl`／`w1:p1`，`wsl` 為 `disconnected`，`be` 的 `binding` 為
  `{"state":"runtime_disconnected","runtime":"wsl","source":"override"}`
- **WHEN** 重畫後在畫面按 `be` 的「取消改綁」
- **THEN** `be` 列首同時有「改綁」按鈕與「取消改綁」按鈕；服務收到 `DELETE /api/projects/p/workstreams/be/override`

#### Scenario: 自動綁定的斷線沒有取消改綁

- **GIVEN** workstream `fe` 沒有覆蓋、其自動綁定的 runtime 斷線，`binding` 為
  `{"state":"runtime_disconnected","runtime":"wsl","source":"auto"}`
- **WHEN** 重畫
- **THEN** `fe` 列首有「改綁」按鈕、沒有「取消改綁」按鈕

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
