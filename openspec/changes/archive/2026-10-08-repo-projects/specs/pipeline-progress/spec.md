# pipeline-progress（delta）

## MODIFIED Requirements

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

系統必須以 JSON 狀態檔保存進度、覆蓋、目前 task 與畫面加入的 Repo Project，形狀為 `{"version": 3, "projects":
{"<pid>": {"tasks": {"<tid>": {"stage": "<stage>", "mark": "none|completed|failed"}}, "overrides": {"<wid>":
{"runtime": "<id>", "pane_id": "<pane id>"}}, "active": {"<wid>": "<tid>"}}}, "repo_projects": {"<pid>": {"name":
"<名稱>", "repo": "<repo key>", "stages": ["<stage>", ...], "tasks": {"<tid>": {"stage": "<stage>", "mark":
"none|completed|failed"}}}}}`（沒有目前 task 的 workstream 不出現在 `active`；`projects` 是手寫 project 的區段，形狀與
版本 2 相同，只放手寫 project；`repo_projects` 的 `<tid>` 是 `<runtime id>~<pane id>`（runtime id 可含 `~`，要拆出 runtime 時從最後一個
`~` 切開），Repo Project 不保存 `overrides` 與 `active`，其 task 進度與手寫 project 的進度分開存放）。
`repo_projects` 的項目順序不代表 Project 的顯示順序（顯示順序見 `repo-projects`「Repo Project 與手寫 project 並列及 id 撞名」）；Repo Project 的 task 進度依已保存的對照表全部寫出，不以目前
展開出來的 task 過濾（見 `repo-projects`「Repo Project 進度的保存與清除」）。
每次操作被接受後，系統必須先把完整的新狀態寫入狀態檔（先寫同目錄暫存檔再以取代方式改名，寫入期間中斷不會留下
半份檔案），寫入成功後才讓記憶體中的狀態生效；多個寫入請求依序處理，不交錯。寫檔與否取決於序列化後的狀態檔內容：新內容與目前檔案內容完全相同時不寫。pane 的歸類結果與 Repo Project 展開出的
workstream、task 本身不寫入狀態檔，因此 pane 進出、cwd 改變、名稱改變不會改寫檔案。沒有狀態檔路徑時（見
`pipeline-config`「狀態檔位置」）只更新記憶體。系統寫出的狀態檔一律為 `version: 3`。

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
  `wJ:p1`（runtime `local`）的 task 推進到 `Build`
- **WHEN** 讀取寫出的狀態檔
- **THEN** `version` 為 3；`repo_projects.app` 為 `{"name":"App","repo":"d:\\work\\app\\.git","stages":["Plan","Build"],"tasks":{"local~wJ:p1":{"stage":"Build","mark":"none"}}}`，
  不含 `overrides` 與 `active`

#### Scenario: 只有 pane 進出與 cwd 改變時不改寫 v2 檔

- **GIVEN** 狀態檔為 `version: 2`（沒有 `repo_projects`），使用者沒有做任何會被接受的操作
- **WHEN** 期間有 pane 開啟、關閉，pane 的 cwd 改變，repo 歸類結果隨之改變
- **THEN** 狀態檔仍是 `version: 2`，內容與位元組都不變

#### Scenario: 寫出內容與現有檔案相同就不寫

- **GIVEN** 狀態檔為 `version: 3`，內容與目前記憶體序列化後的結果完全相同
- **WHEN** 發生只改變 pane 歸類或名稱的事件
- **THEN** 不改寫狀態檔（檔案修改時間不變）

#### Scenario: Repo Project 的顯示順序不依賴檔案順序

- **GIVEN** 先後加入 Repo Project `zeta`、`alpha`，狀態檔以任意 key 順序寫出
- **WHEN** 重啟
- **THEN** `projects` 中這兩個 Repo Project 的順序依名稱排序為 `alpha`、`zeta`

### Requirement: 狀態檔載入與容錯

系統必須在啟動時（有狀態檔路徑時，見 `pipeline-config`「狀態檔位置」）讀取狀態檔：檔案不存在 → 所有 task 用初始進度、沒有覆蓋、
沒有目前 task、沒有 Repo Project，不立即建立檔案；`version` 為 1 → 依舊形狀（沒有 `active`）讀取，所有 workstream 沒有目前
task，下次寫入時寫成 `version: 3`；`version` 為 1 的檔案中，任一 project 出現 `active` 欄位（不論值為何，含 `null` 與空物件）即視為損毀，啟動失敗，
訊息含狀態檔路徑與原因；`version` 為 2 → 依版本 2 的形狀（含 `active`、沒有 `repo_projects`）讀取，沒有 Repo Project，下次寫入時寫成
`version: 3`；`version` 為 1 或 2 的檔案中出現 `repo_projects` 欄位（不論值為何）即視為損毀，啟動失敗，訊息含狀態檔路徑與原因；`version` 為 3 → 讀取 `projects`（同版本 2）與 `repo_projects`，`repo_projects` 必須存在（可以是空物件），缺少視為損毀，啟動失敗，
訊息含狀態檔路徑與原因；檔案無法解析為對應版本的形狀、或 `version`
不是 1、2 或 3（含 4 以上）→ 啟動失敗，訊息含狀態檔路徑與原因；檔案
中的 project、task、workstream 在設定檔不存在，或覆蓋的 `runtime` 不是設定檔中的 runtime → 忽略並記
warn，下次寫入時不再寫出；`active` 中的項目指向設定檔不存在的 workstream 或 task、task 不屬於該 workstream、
或 task 載入後的標記不是 `none`、或該 workstream 的覆蓋在這次載入中因無效而被忽略 → 忽略該項目並記 warn，下次寫入時不再寫出；task 的 `stage`
不在該 Project 的 `stages` → 該 task 改用設定檔的起始 stage、保留標記，並在該 Project 的投影
`warnings` 中加入指出 task id 與原 stage 值的一則訊息；設定檔中有、狀態檔中沒有的 task → 用初始進度。

`repo_projects` 的載入規則：Repo Project 的 `id`、`name`、`stages` 不符合 `repo-projects` 的規則（id 須由 `[A-Za-z0-9_-]` 組成、
名稱與 stages 須符合「Repo Project 的輸入驗證」），或兩個 Repo Project 的 `repo` 相同 → 視同損毀，啟動失敗，訊息含狀態檔路徑與原因；task 的 `stage` 不在該 Repo Project 的
`stages` → 改用第一個 stage、保留標記，並在該 Repo Project 的投影 `warnings` 中加入指出 task id 與原 stage 值的一則訊息；
task id 的 runtime 部分（最後一個 `~` 之前）不是設定檔中的 runtime → 忽略並記 warn，下次寫入時不再寫出。

#### Scenario: 狀態檔損毀

- **GIVEN** 狀態檔內容為 `{not json`
- **WHEN** 啟動
- **THEN** 啟動失敗，訊息含狀態檔路徑

#### Scenario: 讀取 v1 舊檔

- **GIVEN** 狀態檔為 `{"version":1,"projects":{"p":{"tasks":{"t1":{"stage":"Build","mark":"none"}},"overrides":{}}}}`
- **WHEN** 啟動後對 `t1` 做一次被接受的操作
- **THEN** 啟動成功，`t1` 在 `Build`；新寫出的狀態檔 `version` 為 3，且 `p` 含 `active` 欄位

#### Scenario: 讀取 v2 舊檔

- **GIVEN** 狀態檔為 `{"version":2,"projects":{"p":{"tasks":{"t1":{"stage":"Build","mark":"none"}},"overrides":{},"active":{"be":"t1"}}}}`
  且 `t1` 屬於 workstream `be`
- **WHEN** 啟動後對 `t1` 做一次被接受的操作
- **THEN** 啟動成功，`t1` 在 `Build`、`be` 的目前 task 為 `t1`，沒有 Repo Project；新寫出的狀態檔 `version` 為 3

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

- **GIVEN** 狀態檔 `version` 為 4
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
