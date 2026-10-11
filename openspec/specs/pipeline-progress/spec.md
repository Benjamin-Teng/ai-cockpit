# pipeline-progress Specification

## Purpose

定義 Cockpit 自己的寫入面：進度操作與綁定覆蓋的 HTTP 端點、只接受本機同源請求的來源檢查，以及讓進度
與覆蓋在重啟後保留的狀態檔讀寫與容錯。這些寫入只改 Cockpit 自己的狀態，對 HERDR 完全唯讀（ADR-0001）。

## Requirements

### Requirement: 進度寫入端點

系統必須提供 `POST /api/projects/<project>/tasks/<task>/<操作>`，`<操作>` 為 `advance`（推進）、`retreat`（退回）、
`complete`（標 Completed）、`fail`（標 Failed）、`clear`（清除標記），不需要請求本體。操作被接受且
持久化成功時回 204，之後的投影反映新進度；project 或 task 不存在、或 `<操作>` 不是上述五者時回 404；
操作被拒絕時回 409，本體為 `{"error": "<原因>"}`；狀態檔寫入失敗時回 500，本體為
`{"error": "<原因>"}`，且記憶體中的進度維持操作前的值。

#### Scenario: 推進成功

- **GIVEN** task `t1` 在 `Plan`、非最後一站，投影 version 為 8
- **WHEN** `POST /api/projects/p/tasks/t1/advance`
- **THEN** 回 204；稍後 `/api/state` 的 version 大於 8，`t1` 的 `stage` 為下一站

#### Scenario: 退回成功

- **GIVEN** task `t1` 在 `Build`、非第一站、標記 `none`
- **WHEN** `POST /api/projects/p/tasks/t1/retreat`
- **THEN** 回 204；稍後投影中 `t1` 的 `stage` 為上一站

#### Scenario: 被拒絕

- **GIVEN** `t1` 在最後一站
- **WHEN** `POST /api/projects/p/tasks/t1/advance`
- **THEN** 回 409，本體 `error` 指出已是最後一個 Stage，投影中的 `t1` 不變

#### Scenario: 第一站退回被拒絕

- **GIVEN** `t1` 在第一站
- **WHEN** `POST /api/projects/p/tasks/t1/retreat`
- **THEN** 回 409，本體 `error` 指出已是第一個 Stage，投影中的 `t1` 不變

#### Scenario: 不存在的 task

- **WHEN** `POST /api/projects/p/tasks/nope/complete`
- **THEN** 回 404

#### Scenario: 人工端點沒有宣告操作

- **WHEN** `POST /api/projects/p/tasks/t1/start`
- **THEN** 回 404，狀態不變

#### Scenario: 寫檔失敗不改記憶體

- **GIVEN** 狀態檔所在目錄不可寫
- **WHEN** `POST /api/projects/p/tasks/t1/complete`
- **THEN** 回 500，投影中 `t1` 的 `mark` 仍為 `none`

### Requirement: 綁定覆蓋端點

系統必須提供 `PUT /api/projects/<project>/workstreams/<workstream>/override`（本體
`{"runtime": "<id>", "pane_id": "<pane id>"}`）設定覆蓋、`DELETE` 同一路徑取消覆蓋。設定或取消成功且
持久化成功時回 204；project 或 workstream 不存在回 404；本體不是含這兩個字串欄位的 JSON 物件回 400；
覆蓋被拒絕（runtime 未設定、未連線、pane 不存在或已 exited）回 409，本體為 `{"error": "<原因>"}`；
狀態檔寫入失敗回 500 且記憶體不變。取消不存在的覆蓋回 204。workstream 是 Repo Project 固定綁定到 pane 的工作線
（見 `repo-projects`「Repo Project 工作線的固定 pane 綁定」）時，`PUT` 與 `DELETE` 都回 409，本體為
`{"error": "<原因>", "code": "not_overridable"}`，狀態不變。

#### Scenario: 設定覆蓋

- **GIVEN** runtime `win` 為 `connected` 且有未 exited 的 pane `wJ:p2`
- **WHEN** `PUT /api/projects/p/workstreams/be/override`，本體 `{"runtime":"win","pane_id":"wJ:p2"}`
- **THEN** 回 204；投影中 `be` 的 `binding` 為 `bound`、`pane_id` 為 `wJ:p2`、`source` 為 `override`

#### Scenario: workstream 不存在

- **WHEN** `PUT /api/projects/p/workstreams/nope/override`，本體 `{"runtime":"win","pane_id":"wJ:p2"}`
- **THEN** 回 404

#### Scenario: 覆蓋被拒絕

- **GIVEN** runtime `wsl` 為 `disconnected`
- **WHEN** `PUT /api/projects/p/workstreams/be/override`，本體 `{"runtime":"wsl","pane_id":"w1:p1"}`
- **THEN** 回 409，本體 `error` 指出 runtime 未連線，覆蓋不變

#### Scenario: 覆蓋寫檔失敗

- **GIVEN** 狀態檔所在目錄不可寫，runtime `win` 為 `connected` 且有未 exited 的 pane `wJ:p2`
- **WHEN** `PUT /api/projects/p/workstreams/be/override`，本體 `{"runtime":"win","pane_id":"wJ:p2"}`
- **THEN** 回 500，投影中 `be` 的 `binding.source` 不是 `override`

#### Scenario: 本體缺欄位

- **WHEN** `PUT` 同路徑，本體 `{"runtime":"win"}`
- **THEN** 回 400，覆蓋不變

#### Scenario: 取消覆蓋

- **GIVEN** `be` 有覆蓋
- **WHEN** `DELETE /api/projects/p/workstreams/be/override`
- **THEN** 回 204；投影中 `be` 的 `binding.source` 不再是 `override`

#### Scenario: 固定 pane 的工作線不能改綁

- **GIVEN** Repo Project `app` 的 workstream `local~wJ:p1` 固定綁定到 pane `wJ:p1`，runtime `local` 為 `connected` 且有未
  exited 的 pane `wJ:p2`
- **WHEN** `PUT /api/projects/app/workstreams/local~wJ:p1/override`，本體 `{"runtime":"local","pane_id":"wJ:p2"}`
- **THEN** 回 409，本體 `code` 為 `not_overridable`；該 workstream 的 `binding` 仍為 `bound`、`pane_id` 為 `wJ:p1`、`source`
  為 `pane`

#### Scenario: 固定 pane 的工作線不能取消改綁

- **WHEN** `DELETE /api/projects/app/workstreams/local~wJ:p1/override`
- **THEN** 回 409，本體 `code` 為 `not_overridable`，狀態不變

### Requirement: 寫入端點只接受本機同源請求

系統必須對所有寫入端點（`POST`、`PUT`、`PATCH`、`DELETE`）檢查請求：`Host` 標頭必須是 `127.0.0.1:<port>`、
`localhost:<port>` 或 `[::1]:<port>`（`<port>` 為服務實際監聽的埠）；若帶 `Origin` 標頭，其值必須是
`http://` 加上同一個 `Host` 值。不符合時回 403 且不改任何狀態。沒有 `Origin` 標頭但 `Host` 合格的請求
（例如命令列工具）接受。

#### Scenario: 跨站表單被拒

- **WHEN** 請求帶 `Host: 127.0.0.1:7770`、`Origin: https://evil.example`
- **THEN** 回 403，進度不變

#### Scenario: DNS rebinding 被拒

- **WHEN** 請求帶 `Host: evil.example:7770`
- **THEN** 回 403

#### Scenario: 自家頁面與命令列可用

- **WHEN** 分別以 `Host: 127.0.0.1:7770`＋`Origin: http://127.0.0.1:7770`，與只有 `Host: localhost:7770`
  送出合法操作
- **THEN** 兩者都被處理（204 或依規則的 409）

#### Scenario: PATCH 也檢查來源

- **WHEN** `PATCH /api/repo-projects/app` 帶 `Host: 127.0.0.1:7770`、`Origin: https://evil.example`，本體 `{"name":"x"}`
- **THEN** 回 403，Repo Project 的名稱不變

### Requirement: 狀態檔格式與持久化

系統必須以 JSON 狀態檔保存進度、覆蓋、目前 task 與畫面加入的 Repo Project，形狀為 `{"version": 4, "projects":
{"<pid>": {"tasks": {"<tid>": {"stage": "<stage>", "mark": "none|completed|failed"}}, "overrides": {"<wid>":
{"runtime": "<id>", "pane_id": "<pane id>"}}, "active": {"<wid>": "<tid>"}}}, "repo_projects": {"<pid>": {"name":
"<名稱>", "repo": "<repo key>", "stages": ["<stage>", ...], "phases": ["plan|implement|review|complete|null", ...],
"tasks": {"<tid>": {"stage": "<stage>", "mark": "none|completed|failed", "sync": {"mode": "auto|manual", "applied":
{"change": "<change 名稱>", "phase": "plan|implement|review|complete", "checked": <整數>, "total": <整數>} | null}}}}}}`
（沒有目前 task 的 workstream 不出現在 `active`；`projects` 是手寫 project 的區段，形狀與
版本 2 相同，只放手寫 project；`repo_projects` 的 `<tid>` 是 `<runtime id>~<pane id>`（runtime id 可含 `~`，要拆出 runtime 時從最後一個
`~` 切開），Repo Project 不保存 `overrides` 與 `active`，其 task 進度與手寫 project 的進度分開存放；`phases` 與 `stages` 逐項對齊，
`null` 表示該 stage 不對應任何 OpenSpec 階段，非 `null` 的值互不重複；task 的 `sync` 選填，只有 `repo_projects` 底下的 task 才有（手寫 `projects` 底下的 task 帶 `sync` 視為損毀，見「狀態檔載入與容錯」；
有 `sync` 的 task 一定同時有 `stage` 與 `mark`，即有進度項目），
只在該 task 套用過 OpenSpec 偵測結果或被人工／agent 推進過時才有紀錄，`applied` 是上一次套用的偵測結果，尚未套用過時為 `null`）。
`repo_projects` 的項目順序不代表 Project 的顯示順序（顯示順序見 `repo-projects`「Repo Project 與手寫 project 並列及 id 撞名」）；Repo Project 的 task 進度依已保存的對照表全部寫出，不以目前
展開出來的 task 過濾（見 `repo-projects`「Repo Project 進度的保存與清除」）。每一輪 OpenSpec 偵測的最新結果（task 當下對上的
change、階段、勾選數）只存在記憶體、不寫入狀態檔；偵測只有在改變某張 task 的所在 stage 或同步狀態時才會造成寫檔，偵測結果沒有造成這類
改變時不改寫狀態檔。
每次操作被接受後，系統必須先把完整的新狀態寫入狀態檔（先寫同目錄暫存檔再以取代方式改名，寫入期間中斷不會留下
半份檔案），寫入成功後才讓記憶體中的狀態生效；多個寫入請求依序處理，不交錯。寫檔與否取決於序列化後的狀態檔內容：新內容與目前檔案內容完全相同時不寫。pane 的歸類結果與 Repo Project 展開出的
workstream、task 本身不寫入狀態檔，因此 pane 進出、cwd 改變、名稱改變不會改寫檔案。沒有狀態檔路徑時（見
`pipeline-config`「狀態檔位置」）只更新記憶體。系統寫出的狀態檔一律為 `version: 4`。

#### Scenario: 重啟後保留

- **GIVEN** `t1` 已推進到 `Build` 並標 Completed，`be` 設了覆蓋，workstream `fe` 的目前 task 為 `f1`
- **WHEN** 停止並重新啟動系統（同一份設定與狀態檔）
- **THEN** `t1` 在 `Build`、標記 `completed`；`be` 的覆蓋仍在（依覆蓋規則解析）；`fe` 的目前 task 為 `f1`

#### Scenario: 並發寫入不遺失

- **GIVEN** task `a`、`b` 皆標記 `none`
- **WHEN** 幾乎同時送出 `a` 的 complete 與 `b` 的 fail
- **THEN** 兩者都回 204，狀態檔中 `a` 為 `completed`、`b` 為 `failed`

#### Scenario: 寫出 Repo Project

- **GIVEN** 已加入名稱 `App`、repo key `d:\work\app\.git`、stages `["Plan","Build"]` 的 Repo Project（id `app`），其 pane
  `wJ:p1`（runtime `local`）的 task 進度在 `Build`，沒有同步狀態（例如載入自沒有 `sync` 的狀態檔）
- **WHEN** 狀態檔因其他寫入（例如手寫 project 的 task 被推進）而寫出，讀取寫出的狀態檔
- **THEN** `version` 為 4；`repo_projects.app` 為 `{"name":"App","repo":"d:\\work\\app\\.git","stages":["Plan","Build"],"phases":["plan",null],"tasks":{"local~wJ:p1":{"stage":"Build","mark":"none"}}}`
  （`phases` 為加入時所帶的對應；該 task 沒有 `sync` 紀錄），不含 `overrides` 與 `active`。
  若該 task 是被人工推進到 `Build`，檔案中它會另有 `sync`（`mode` 為 `manual`，見 `openspec-stage-sync`「手動操作暫時優先」）

#### Scenario: 只有 pane 進出與 cwd 改變時不改寫 v2 檔

- **GIVEN** 狀態檔為 `version: 2`（沒有 `repo_projects`），使用者沒有做任何會被接受的操作
- **WHEN** 期間有 pane 開啟、關閉，pane 的 cwd 改變，repo 歸類結果隨之改變
- **THEN** 狀態檔仍是 `version: 2`，內容與位元組都不變

#### Scenario: 寫出內容與現有檔案相同就不寫

- **GIVEN** 狀態檔為 `version: 4`，內容與目前記憶體序列化後的結果完全相同
- **WHEN** 發生只改變 pane 歸類或名稱的事件
- **THEN** 不改寫狀態檔（檔案修改時間不變）

#### Scenario: Repo Project 的顯示順序不依賴檔案順序

- **GIVEN** 先後加入 Repo Project `zeta`、`alpha`，狀態檔以任意 key 順序寫出
- **WHEN** 重啟
- **THEN** `projects` 中這兩個 Repo Project 的順序依名稱排序為 `alpha`、`zeta`

#### Scenario: 寫出 task 的同步狀態

- **GIVEN** Repo Project `app` 的 task `local~wJ:p1` 已套用過偵測結果 change `foo`、階段 `implement`、勾選 3 / 8，之後被人工推進一站
- **WHEN** 讀取寫出的狀態檔
- **THEN** 該 task 為 `{"stage":"Review","mark":"none","sync":{"mode":"manual","applied":{"change":"foo","phase":"implement","checked":3,"total":8}}}`

#### Scenario: 偵測結果沒有造成改變時不改寫狀態檔

- **GIVEN** 狀態檔為 `version: 4`；某 pane 的偵測結果在多輪偵測之間改變，但沒有造成任何 task 的所在 stage 或同步狀態改變（例如卡片已標
  Completed，或偵測結果對不上任何 change）
- **WHEN** 期間沒有其他被接受的操作
- **THEN** 狀態檔的內容與修改時間都不變

#### Scenario: 偵測的最新結果不寫入狀態檔

- **GIVEN** 某 pane 的偵測結果對上 change `foo`，且該 task 沒有套用過（例如帶著 Completed 標記）
- **WHEN** 因其他原因寫出狀態檔
- **THEN** 該 task 沒有 `sync` 紀錄，狀態檔中沒有 `foo`

### Requirement: 狀態檔載入與容錯

系統必須在啟動時（有狀態檔路徑時，見 `pipeline-config`「狀態檔位置」）讀取狀態檔：檔案不存在 → 所有 task 用初始進度、沒有覆蓋、
沒有目前 task、沒有 Repo Project，不立即建立檔案；`version` 為 1 → 依舊形狀（沒有 `active`）讀取，所有 workstream 沒有目前
task，下次寫入時寫成 `version: 4`；`version` 為 1 的檔案中，任一 project 出現 `active` 欄位（不論值為何，含 `null` 與空物件）即視為損毀，啟動失敗，
訊息含狀態檔路徑與原因；`version` 為 2 → 依版本 2 的形狀（含 `active`、沒有 `repo_projects`）讀取，沒有 Repo Project，下次寫入時寫成
`version: 4`；`version` 為 1 或 2 的檔案中出現 `repo_projects` 欄位（不論值為何）即視為損毀，啟動失敗，訊息含狀態檔路徑與原因；`version` 為 3 → 讀取 `projects`（同版本 2）與 `repo_projects`，`repo_projects` 必須存在（可以是空物件），缺少視為損毀，啟動失敗，
訊息含狀態檔路徑與原因；成功讀取時依下述「預設對應補齊」補上每個 Repo Project 的 `phases`，下次寫入時寫成 `version: 4`；`version` 為 4 →
讀取 `projects`（同版本 2）與 `repo_projects`（`repo_projects` 同樣必須存在，每個 Repo Project 另必須有 `phases`，task 可有
`sync`），`phases` 的內容一律照檔案載入，`null` 一律尊重、不再依站名推導；檔案無法解析為對應版本的形狀、或 `version`
不是 1、2、3 或 4（含 5 以上）→ 啟動失敗，訊息含狀態檔路徑與原因；檔案
中的 project、task、workstream 在設定檔不存在，或覆蓋的 `runtime` 不是設定檔中的 runtime → 忽略並記
warn，下次寫入時不再寫出；`active` 中的項目指向設定檔不存在的 workstream 或 task、task 不屬於該 workstream、
或 task 載入後的標記不是 `none`、或該 workstream 的覆蓋在這次載入中因無效而被忽略 → 忽略該項目並記 warn，下次寫入時不再寫出；task 的 `stage`
不在該 Project 的 `stages` → 該 task 改用設定檔的起始 stage、保留標記，並在該 Project 的投影
`warnings` 中加入指出 task id 與原 stage 值的一則訊息；設定檔中有、狀態檔中沒有的 task → 用初始進度。

`repo_projects` 的載入規則：Repo Project 的 `id`、`name`、`stages` 不符合 `repo-projects` 的規則（id 須由 `[A-Za-z0-9_-]` 組成、
名稱與 stages 須符合「Repo Project 的輸入驗證」），或兩個 Repo Project 的 `repo` 相同，或（版本 4）`phases` 的長度與 `stages` 不同、
非 `null` 的值有重複，或手寫 `projects` 底下的 task 帶有 `sync` → 視同損毀，啟動失敗，訊息含狀態檔路徑與原因；task 的 `stage` 不在該 Repo Project 的
`stages` → 改用第一個 stage、保留標記，並在該 Repo Project 的投影 `warnings` 中加入指出 task id 與原 stage 值的一則訊息；
這次退回第一個 stage 的 task 若帶有 `sync` 且 `mode` 為 `auto`，同時把它的 `applied` 清為 `null`（卡片已不在上次套用到的站，
清掉才會在下一輪偵測以相同結果重新套用），`mode` 為 `manual` 的 `sync` 原樣保留；
task id 的 runtime 部分（最後一個 `~` 之前）不是設定檔中的 runtime → 忽略並記 warn，下次寫入時不再寫出（連同它的 `sync`）。

預設對應補齊：讀取版本 1 到 3 的檔案時（版本 1、2 沒有 Repo Project，實際只影響版本 3），每個 Repo Project 的 `phases` 依 stage 名稱
一次性補齊：名稱完全等於 `規劃` 或 `Plan` → `plan`、`實作` 或 `Implement` → `implement`、`審查` 或 `Review` → `review`、
`完成` 或 `Complete` → `complete`，其餘為 `null`；補齊後若同一個階段被多個 stage 取得，只保留 stage 順序中最先出現的一個，後出現的改為 `null`。補齊後的結果就是明確資料，
下次寫入時落成版本 4；版本 4 的檔案不做這項補齊。

#### Scenario: 狀態檔損毀

- **GIVEN** 狀態檔內容為 `{not json`
- **WHEN** 啟動
- **THEN** 啟動失敗，訊息含狀態檔路徑

#### Scenario: 讀取 v1 舊檔

- **GIVEN** 狀態檔為 `{"version":1,"projects":{"p":{"tasks":{"t1":{"stage":"Build","mark":"none"}},"overrides":{}}}}`
- **WHEN** 啟動後對 `t1` 做一次被接受的操作
- **THEN** 啟動成功，`t1` 在 `Build`；新寫出的狀態檔 `version` 為 4，且 `p` 含 `active` 欄位

#### Scenario: 讀取 v2 舊檔

- **GIVEN** 狀態檔為 `{"version":2,"projects":{"p":{"tasks":{"t1":{"stage":"Build","mark":"none"}},"overrides":{},"active":{"be":"t1"}}}}`
  且 `t1` 屬於 workstream `be`
- **WHEN** 啟動後對 `t1` 做一次被接受的操作
- **THEN** 啟動成功，`t1` 在 `Build`、`be` 的目前 task 為 `t1`，沒有 Repo Project；新寫出的狀態檔 `version` 為 4

#### Scenario: 讀取含 Repo Project 的 v3 檔

- **GIVEN** 狀態檔為 `{"version":3,"projects":{},"repo_projects":{"app":{"name":"App","repo":"d:\\work\\app\\.git","stages":["Plan","Build"],"tasks":{"local~wJ:p1":{"stage":"Build","mark":"failed"}}}}}`，
  設定檔有 runtime `local`
- **WHEN** 啟動
- **THEN** 啟動成功，`projects` 含 Repo Project `app`（名稱 `App`、stages 為 `Plan`、`Build`）；pane `wJ:p1` 歸入該 repo 後，其 task 在
  `Build`、標記 `failed`

#### Scenario: v1、v2 檔出現 repo_projects 視為損毀

- **GIVEN** 狀態檔為 `{"version":1,"projects":{},"repo_projects":{}}`，或 `{"version":2,"projects":{},"repo_projects":{}}`
- **WHEN** 分別啟動
- **THEN** 兩者都啟動失敗，訊息含狀態檔路徑與原因

#### Scenario: v3 檔缺 repo_projects 視為損毀

- **GIVEN** 狀態檔為 `{"version":3,"projects":{}}`
- **WHEN** 啟動
- **THEN** 啟動失敗，訊息含狀態檔路徑與原因；`{"version":3,"projects":{},"repo_projects":{}}` 則啟動成功、沒有 Repo Project

#### Scenario: 兩個 Repo Project 的 repo 相同視為損毀

- **GIVEN** v3 狀態檔的 `repo_projects` 有 `a` 與 `b` 兩項，`repo` 都是 `d:\work\app\.git`
- **WHEN** 啟動
- **THEN** 啟動失敗，訊息含狀態檔路徑與原因

#### Scenario: v1 檔出現 active 視為損毀

- **GIVEN** 狀態檔為 `{"version":1,"projects":{"p":{"tasks":{"t1":{"stage":"Build","mark":"none"}},"overrides":{},"active":{}}}}`
- **WHEN** 啟動
- **THEN** 啟動失敗，訊息含狀態檔路徑與原因

#### Scenario: v1 檔的 active 為 null 也視為損毀

- **GIVEN** 狀態檔為 `{"version":1,"projects":{"p":{"tasks":{"t1":{"stage":"Build","mark":"none"}},"overrides":{},"active":null}}}`
- **WHEN** 啟動
- **THEN** 啟動失敗，訊息含狀態檔路徑與原因

#### Scenario: 不支援的版本

- **GIVEN** 狀態檔 `version` 為 5
- **WHEN** 啟動
- **THEN** 啟動失敗，訊息含狀態檔路徑

#### Scenario: Repo Project 定義不合法

- **GIVEN** v3 狀態檔的 `repo_projects` 有一項 `stages` 為 `[]`（或 `id` 為 `a/b`、`name` 為空字串）
- **WHEN** 啟動
- **THEN** 啟動失敗，訊息含狀態檔路徑與原因

#### Scenario: Repo Project 的 task 的 stage 被改掉

- **GIVEN** v3 狀態檔中 Repo Project `app` 的 stages 為 `["Plan","Build"]`，task `local~wJ:p1` 為 `{"stage":"Gone","mark":"completed"}`
- **WHEN** 啟動且該 pane 歸入 `app`
- **THEN** 啟動成功，該 task 在 `Plan`、標記仍為 `completed`，`app` 的 `warnings` 有一則含 `local~wJ:p1` 與 `Gone` 的訊息

#### Scenario: 退回第一站的 auto 卡片清掉 applied

- **GIVEN** v4 狀態檔中 Repo Project `app` 的 stages 為 `["Plan","Build"]`，task `local~wJ:p1` 為
  `{"stage":"Gone","mark":"none","sync":{"mode":"auto","applied":{"change":"foo","phase":"implement","checked":3,"total":8}}}`
- **WHEN** 啟動且該 pane 歸入 `app`，之後偵測結果仍是 `foo` 的 `implement`（3／8）
- **THEN** 載入後該 task 在 `Plan`、`sync.applied` 為 `null`（`mode` 仍為 `auto`）；下一輪偵測把它移到 `implement` 對應的 stage

#### Scenario: 退回第一站的 manual 卡片保留 applied

- **GIVEN** 同上，但 `sync.mode` 為 `manual`
- **WHEN** 啟動且該 pane 歸入 `app`
- **THEN** 載入後該 task 在 `Plan`、`sync` 原樣保留（`mode` 為 `manual`、`applied` 不變），偵測結果不變時不移動

#### Scenario: runtime id 含 ~ 時從最後一個 ~ 切

- **GIVEN** 設定檔有 runtime `dev~1`；v3 狀態檔中 Repo Project `app` 的 task 有 `dev~1~wJ:p1` 與 `ghost~wJ:p2`
- **WHEN** 啟動後做一次被接受的操作
- **THEN** `dev~1~wJ:p1` 對應 runtime `dev~1`，保留並寫回；`ghost~wJ:p2` 對應 runtime `ghost`（未設定），被忽略且不再寫出

#### Scenario: Repo Project 的 task 指向未設定的 runtime

- **GIVEN** v3 狀態檔中 `repo_projects.app.tasks` 有 `ghost~wJ:p1`，設定檔沒有 runtime `ghost`
- **WHEN** 啟動後做一次被接受的操作
- **THEN** 啟動成功並記 warn；新寫出的狀態檔不含 `ghost~wJ:p1`

#### Scenario: 無效的目前 task

- **GIVEN** v2 狀態檔中 `active` 為 `{"be": "f1"}`，但 `f1` 屬於 workstream `fe`
- **WHEN** 啟動
- **THEN** 啟動成功，`be` 沒有目前 task

#### Scenario: stage 被改名

- **GIVEN** 設定檔 stages 由 `["Spec","Build"]` 改為 `["Spec","Implement"]`，狀態檔中 `t1` 為
  `{"stage":"Build","mark":"none"}`，`t1` 的起始 stage 為 `Spec`
- **WHEN** 啟動
- **THEN** 啟動成功，`t1` 在 `Spec`；Project 投影的 `warnings` 有一則含 `t1` 與 `Build` 的訊息

#### Scenario: 設定檔刪掉的 task

- **GIVEN** 狀態檔有 task `old`，設定檔已沒有 `old`
- **WHEN** 啟動後對其他 task 做一次被接受的操作
- **THEN** 啟動成功，新寫出的狀態檔不含 `old`

#### Scenario: 讀取 v3 檔時補上預設對應（繁中站名）

- **GIVEN** 狀態檔為 `version: 3`，Repo Project `app` 的 stages 為 `["規劃","實作","審查","完成"]`
- **WHEN** 啟動後對任一 task 做一次被接受的操作
- **THEN** 啟動成功，`app` 的 `stage_phases` 為 `["plan","implement","review","complete"]`；新寫出的狀態檔 `version` 為 4，
  `repo_projects.app.phases` 為 `["plan","implement","review","complete"]`

#### Scenario: 讀取 v3 檔時補上預設對應（英文站名與部分對得上）

- **GIVEN** 狀態檔為 `version: 3`，Repo Project `a` 的 stages 為 `["Plan","Implement","Review","Complete"]`，Repo Project `b` 的 stages 為
  `["Plan","Build","Review","Done"]`
- **WHEN** 啟動
- **THEN** `a` 的 `stage_phases` 為 `["plan","implement","review","complete"]`；`b` 的 `stage_phases` 為 `["plan",null,"review",null]`

#### Scenario: 讀取 v3 檔時補對應遇到重複

- **GIVEN** 狀態檔為 `version: 3`，Repo Project 的 stages 為 `["Plan","規劃","Review"]`
- **WHEN** 啟動
- **THEN** `stage_phases` 為 `["plan",null,"review"]`（後出現的 `規劃` 改為 `null`）

#### Scenario: v4 檔的 null 一律尊重

- **GIVEN** 狀態檔為 `version: 4`，Repo Project 的 stages 為 `["Plan","Implement"]`、`phases` 為 `[null,null]`
- **WHEN** 啟動
- **THEN** 啟動成功，`stage_phases` 為 `[null,null]`，不因站名是預設名稱而補上對應

#### Scenario: 讀取含同步狀態的 v4 檔

- **GIVEN** 狀態檔為 `version: 4`，Repo Project `app` 的 task `local~wJ:p1` 為 `{"stage":"Review","mark":"none","sync":{"mode":"manual","applied":{"change":"foo","phase":"implement","checked":3,"total":8}}}`，
  設定檔有 runtime `local`
- **WHEN** 啟動，且 pane `wJ:p1` 歸入 `app`、偵測結果仍是 change `foo`、`implement`、3 / 8
- **THEN** 啟動成功，該 task 保持在 `Review`，`sync.mode` 為 `manual`

#### Scenario: v4 檔的 phases 長度不符視為損毀

- **GIVEN** 狀態檔為 `version: 4`，某 Repo Project 的 `stages` 有 3 項、`phases` 只有 2 項
- **WHEN** 啟動
- **THEN** 啟動失敗，訊息含狀態檔路徑與原因

#### Scenario: v4 檔的 phases 重複視為損毀

- **GIVEN** 狀態檔為 `version: 4`，某 Repo Project 的 `phases` 為 `["plan","plan",null]`
- **WHEN** 啟動
- **THEN** 啟動失敗，訊息含狀態檔路徑與原因

#### Scenario: v4 檔缺 phases 或 repo_projects 視為損毀

- **GIVEN** 狀態檔為 `version: 4`，Repo Project 沒有 `phases` 欄位；或 `{"version":4,"projects":{}}`
- **WHEN** 分別啟動
- **THEN** 兩者都啟動失敗，訊息含狀態檔路徑與原因

#### Scenario: 指向未設定 runtime 的 task 連同同步狀態被忽略

- **GIVEN** v4 狀態檔中 Repo Project `app` 的 task `ghost~wJ:p1` 帶有 `sync`，設定檔沒有 runtime `ghost`
- **WHEN** 啟動後做一次被接受的操作
- **THEN** 啟動成功並記 warn；新寫出的狀態檔不含 `ghost~wJ:p1`，也不含它的 `sync`

#### Scenario: v3 檔含 phases 或 sync 視為損毀

- **GIVEN** 狀態檔為 `version: 3`，某 Repo Project 帶有 `phases` 欄位；或某 Repo Project 的 task 帶有 `sync` 欄位
- **WHEN** 分別啟動
- **THEN** 兩者都啟動失敗（無法解析為版本 3 的形狀），訊息含狀態檔路徑與原因

#### Scenario: 手寫 project 的 task 帶 sync 視為損毀

- **GIVEN** 狀態檔為 `version: 4`，`projects.p.tasks.t1` 為 `{"stage":"Build","mark":"none","sync":{"mode":"manual","applied":null}}`
- **WHEN** 啟動
- **THEN** 啟動失敗，訊息含狀態檔路徑與原因
