# repo-projects（delta）

## MODIFIED Requirements

### Requirement: 納入 repo 判定的 pane 與更新時機

系統必須只對下列 pane 判定 repo：所屬 runtime 有路徑對應（可把 pane 的 cwd 轉成主機路徑）、runtime 目前為 `connected`、
pane 未 exited、cwd 非空。其餘 pane（沒有 cwd、runtime 沒有路徑對應、已 exited）不歸入任何 repo。未連線 runtime 的 pane
不重新查 git，沿用上一次的歸類結果（它們的工作線仍在，綁定顯示 `runtime_disconnected`）；系統不得為了判定 repo 而啟動 WSL
發行版。pane 的 cwd 改變沒有事件通知，因此在下一次重新
抓取 snapshot（預設每 30 秒）後才反映；反映後 pane 改歸入新 cwd 所屬的 repo。判定結果與上次不同時，投影立即更新（`version`
遞增）；結果相同時不得造成投影變動。判定結果除所屬 repo 外，還包含該 pane 所在 worktree 的根目錄（`root`，主機路徑字串；cwd 在 worktree
的子目錄內時為該 worktree 的根目錄，不是 cwd），供 `openspec-stage-sync` 的 OpenSpec 進度偵測以 worktree 為單位查詢；根目錄與
repo 歸類一併更新，未連線 runtime 的 pane 沿用上一次的根目錄。Windows 路徑的根目錄取 git 回報的工作樹根目錄，正斜線換成反斜線、
大小寫保留（不套用 repo key 的小寫化）；WSL 路徑的根目錄為 `\\wsl.localhost\<distro>\...` 形式，無法轉成這種形式時該 pane 沒有根目錄，
不被 OpenSpec 進度偵測查詢（仍照常歸類 repo）。

#### Scenario: 範圍外的 pane

- **GIVEN** 三個 pane：一個已 exited、一個 cwd 為空、一個屬於沒有路徑對應的 runtime，其 cwd 都在某個 git repo 內
- **WHEN** 判定
- **THEN** 三者都不歸入任何 repo

#### Scenario: 未連線的 runtime 沿用上次歸類

- **GIVEN** WSL runtime `wsl` 的 pane `w1:p1` 已歸入 repo `app`；之後 `wsl` 斷線（發行版已關閉）
- **WHEN** 斷線期間產生投影
- **THEN** 系統沒有對 `wsl` 的 pane 執行任何 git 查詢、也沒有啟動 WSL 發行版；`w1:p1` 仍歸入 `app`，其工作線仍在，`binding.state`
  為 `runtime_disconnected`；重新連上後才重新查詢

#### Scenario: cwd 變動在下一次 snapshot 後反映

- **GIVEN** pane 的 cwd 在 repo X 內，已歸入 X；使用者在該 pane 內 `cd` 到 repo Y
- **WHEN** 下一次重新抓取 snapshot 完成
- **THEN** 該 pane 歸入 Y，不再歸入 X；在此之前仍歸入 X

#### Scenario: 判定結果帶有 worktree 根目錄

- **GIVEN** pane A 的 cwd 為 `D:\work\app\src`（主 worktree 的子目錄）；pane B 的 cwd 為 `D:\work\app-wt\feat\lib`，是 linked worktree
  `D:\work\app-wt\feat` 內的子目錄
- **WHEN** 判定兩個 pane 的 repo
- **THEN** A 的 worktree 根目錄為 `D:\work\app`，B 的為 `D:\work\app-wt\feat`；兩者的 repo key 相同

#### Scenario: 未連線 runtime 沿用上次的 worktree 根目錄

- **GIVEN** WSL runtime `wsl` 的 pane `w1:p1` 已歸入 repo `app`，worktree 根目錄為 `\\wsl.localhost\Ubuntu\home\u\app`；之後 `wsl` 斷線
- **WHEN** 斷線期間產生投影
- **THEN** 系統沒有對該 pane 執行任何 git 查詢，其 worktree 根目錄仍是 `\\wsl.localhost\Ubuntu\home\u\app`

#### Scenario: Windows 的 worktree 根目錄保留大小寫

- **GIVEN** pane 的 cwd 為 `D:\Work\App\src`，git 回報工作樹根目錄為 `D:/Work/App`，共同 `.git` 目錄為 `D:/Work/App/.git`
- **WHEN** 判定
- **THEN** worktree 根目錄為 `D:\Work\App`（反斜線、保留大小寫），repo key 仍為 `d:\work\app\.git`

#### Scenario: WSL 的 worktree 根目錄

- **GIVEN** WSL 發行版 `Ubuntu` 的 pane，cwd 為 `/home/u/app/src`，git 回報工作樹根目錄為 `/home/u/app`
- **WHEN** 判定
- **THEN** worktree 根目錄為 `\\wsl.localhost\Ubuntu\home\u\app`

#### Scenario: 轉不出主機路徑的根目錄

- **GIVEN** 某 pane 的 worktree 根目錄無法轉成主機路徑
- **WHEN** 判定
- **THEN** 該 pane 仍歸入所屬 repo，但沒有 worktree 根目錄，不被 OpenSpec 進度偵測查詢

### Requirement: 加入 Repo Project

系統必須提供 `POST /api/repo-projects`，本體為 JSON `{"repo": "<repo key>", "stages": ["<stage>", ...], "phases": [<階段或 null>, ...], "name": "<名稱>"}`
（`name` 選填，未給時用 repo 的預設名稱；`phases` 選填，與 `stages` 逐項對齊，每項為 `plan`、`implement`、`review`、`complete` 之一
或 `null`，省略或整個 `phases` 為 `null` 時等同全部為 `null`，意義與限制見「Stage 對應 OpenSpec 階段」）。`repo` 必須是目前 `detected_repos` 所列或已歸類 pane 所屬的 repo key（只能加入
偵測到的 repo，不接受任意路徑），否則回 404，`code` 為 `repo_not_detected`；該 repo 已被某個 Repo Project 加入回 409，
`code` 為 `repo_already_added`。接受且持久化成功時回 201，本體為 `{"id": "<新 Project 的 id>"}`，id 依「Repo Project 的 id
產生」。加入後新的 Repo Project 立即出現在投影的 `projects`，其 workstream 與 task 依「由 pane 推導 workstream 與 task」展開，
所有 task 的起始進度為第一個 stage、標記 `none`。名稱、stages 與 phases 的驗證見「Repo Project 的輸入驗證」。寫入失敗時回 500，`code` 為 `persist_failed`，且
記憶體不變。

#### Scenario: 加入成功

- **GIVEN** `detected_repos` 含 repo `d:\work\app\.git`（名稱 `app`），有兩個 pane 在其中；沒有任何 project
- **WHEN** `POST /api/repo-projects`，本體 `{"repo":"d:\\work\\app\\.git","stages":["Plan","Build","Review","Done"]}`
- **THEN** 回 201，本體 `{"id":"app"}`；稍後 `projects` 有一個 `kind` 為 `repo` 的 Project：`id` 為 `app`、`name` 為 `app`、
  `stages` 為所給四個、`stage_phases` 為四個 `null`（本體省略 `phases`），兩條 workstream 各有一張在 `Plan` 的 task；`detected_repos`
  不再含該 repo

#### Scenario: 指定名稱

- **WHEN** 加入時本體另帶 `"name":"  My App  "`
- **THEN** Project 的 `name` 為 `My App`（去除前後空白），`id` 依名稱產生

#### Scenario: 不是偵測到的 repo

- **WHEN** `POST /api/repo-projects`，`repo` 為不在偵測清單的 `C:\anywhere\.git`
- **THEN** 回 404，`code` 為 `repo_not_detected`，狀態不變

#### Scenario: 重複加入

- **GIVEN** repo `d:\work\app\.git` 已被某個 Repo Project 加入
- **WHEN** 再對同一個 `repo` 送 `POST /api/repo-projects`
- **THEN** 回 409，`code` 為 `repo_already_added`，狀態不變

#### Scenario: 加入時寫檔失敗

- **GIVEN** 狀態檔所在目錄不可寫
- **WHEN** `POST /api/repo-projects` 帶合法本體
- **THEN** 回 500，`projects` 沒有新增的 Project，`detected_repos` 仍含該 repo

#### Scenario: 加入時帶階段對應

- **GIVEN** `detected_repos` 含 repo `d:\work\app\.git`
- **WHEN** `POST /api/repo-projects`，本體 `{"repo":"d:\\work\\app\\.git","stages":["規劃","實作","審查","完成"],"phases":["plan","implement","review","complete"]}`
- **THEN** 回 201；稍後該 Project 的 `stage_phases` 為 `["plan","implement","review","complete"]`，狀態檔中 `phases` 相同

#### Scenario: 階段對應可以只對應部分 stage

- **WHEN** `POST /api/repo-projects`，本體 `stages` 為 `["Plan","Build","Done"]`、`phases` 為 `["plan",null,null]`
- **THEN** 回 201；`stage_phases` 為 `["plan",null,null]`

#### Scenario: phases 長度與 stages 不同

- **WHEN** `POST /api/repo-projects`，本體 `stages` 有 3 項、`phases` 有 2 項
- **THEN** 回 400，`code` 為 `invalid_stages`，沒有新增 Project

### Requirement: 修改 Repo Project 名稱與 stages

系統必須提供 `PATCH /api/repo-projects/<pid>`，本體為 JSON `{"name": "<名稱>", "stages": [{"name": "<stage>", "from":
"<舊 stage 名稱>", "phase": "<階段>"}, ...]}`，`name` 與 `stages` 皆選填，但至少要給一個；`from` 與 `phase` 也是選填。接受且持久化成功時回 204；`pid` 只對 Repo Project 的定義查找
（包含因 id 撞名而被手寫 project 隱藏、未展開的 Repo Project，見「Repo Project 與手寫 project 並列及 id 撞名」）：`pid` 不是任何
Repo Project 的 id 但是手寫 project 的 id 時回 409，`code` 為 `not_repo_project`；兩者都不是回 404，`code` 為
`unknown_project`。寫檔失敗回 500，`code` 為 `persist_failed`，記憶體不變。同時給 `name` 與 `stages` 時兩者一起
生效或一起被拒絕。Project 的 `id` 不隨改名改變。

`stages` 是修改後完整的有序 stage 清單，順序即新的線性順序；其中 `from` 為目前某個 stage 的名稱時，表示這個新 stage 由該舊
stage 改名（名稱相同即只調整順序）而來，`from` 為 `null`（或省略）表示新增的 stage；沒有被任何新 stage 以 `from` 引用的舊
stage 視為刪除。`from` 必須是修改前的 stage 名稱，且每個舊名稱最多被一個新 stage 引用。每張 task 的標記保留；task 的新
stage：它原本所在的 stage 被某個新 stage 以 `from` 引用時，為該新 stage；否則（所在 stage 被刪除）為新清單的第一個 stage。

`stages` 每一列的 `phase` 是該 stage 修改後對應的 OpenSpec 階段（`plan`、`implement`、`review`、`complete` 之一或 `null`，意義與限制見
「Stage 對應 OpenSpec 階段」）；省略等同 `null`（不是沿用舊值）。因此給了 `stages` 時，修改後的階段對應完全由這份清單的各列決定，
不另依舊對應做搬移；只給 `name` 時階段對應不變。階段對應是否「改變」的判準：對每個 phase，比較修改前擁有它的 stage 經本次 `from` 對應後的新名稱，與修改後擁有它的
stage 名稱；任一 phase 不同即算改變，`from` 為 `null` 的新列視為新身分（不等於任何舊 stage）。因此改名但 phase 跟著走、或只重排，
都不算改變；某個 phase 換了擁有者（含改由新列擁有、原本有擁有者而改為沒有）才算改變。修改前擁有某 phase 的 stage 被刪除、
修改後也沒有任何 stage 擁有它時，視為不變。階段對應改變時，依 `openspec-stage-sync`「階段對應被修改時重新套用」處理自動模式的 task。

#### Scenario: 只改名稱

- **GIVEN** Repo Project `app` 的名稱為 `app`
- **WHEN** `PATCH /api/repo-projects/app`，本體 `{"name":"App 前端"}`
- **THEN** 回 204；稍後投影中 `app` 的 `name` 為 `App 前端`，`id` 仍為 `app`，stages、階段對應與進度不變

#### Scenario: stage 改名、新增、刪除、排序

- **GIVEN** `app` 的 stages 為 `Plan`、`Implement`、`Review`、`Done`；task `a` 在 `Plan`、`b` 在 `Implement`（標記
  `failed`）、`c` 在 `Review`、`d` 在 `Done`
- **WHEN** `PATCH /api/repo-projects/app`，本體 `{"stages":[{"name":"Plan","from":"Plan"},{"name":"Design","from":null},
  {"name":"Build","from":"Implement"},{"name":"Done","from":"Done"}]}`（`Review` 被刪除）
- **THEN** 回 204；stages 依序為 `Plan`、`Design`、`Build`、`Done`；`a` 仍在 `Plan`、`b` 在 `Build` 且標記仍為 `failed`、
  `c` 在 `Plan`（所在 stage 被刪除，改用第一個 stage）、`d` 在 `Done`；各列都沒有 `phase`，因此 `stage_phases` 全為 `null`

#### Scenario: from 省略與 null 都表示新增

- **GIVEN** stages 為 `A`
- **WHEN** `PATCH`，本體 `{"stages":[{"name":"A","from":"A"},{"name":"B"},{"name":"C","from":null}]}`
- **THEN** 回 204；stages 依序為 `A`、`B`、`C`，`B` 與 `C` 都是新增的 stage

#### Scenario: 重新排序保留 task 所在 stage

- **GIVEN** stages 為 `A`、`B`；task `t` 在 `B`
- **WHEN** `PATCH`，本體 `{"stages":[{"name":"B","from":"B"},{"name":"A","from":"A"}]}`
- **THEN** stages 依序為 `B`、`A`，`t` 仍在 `B`

#### Scenario: 同一個舊名稱被引用兩次

- **WHEN** `PATCH` 的 `stages` 有兩個新 stage 的 `from` 都是 `Plan`
- **THEN** 回 400，`code` 為 `invalid_stages`，stages 與進度不變

#### Scenario: from 不是現有 stage

- **WHEN** `PATCH` 的 `stages` 某項 `from` 為不存在的 `Nope`
- **THEN** 回 400，`code` 為 `invalid_stages`，stages 與進度不變

#### Scenario: 兩者同時給但 stages 不合法

- **WHEN** `PATCH` 本體同時有合法的 `name` 與 `stages: []`
- **THEN** 回 400，`code` 為 `invalid_stages`，名稱也沒有被修改

#### Scenario: 空本體

- **WHEN** `PATCH /api/repo-projects/app`，本體 `{}`
- **THEN** 回 400，`code` 為 `invalid_body`，狀態不變

#### Scenario: pid 不存在或是手寫 project

- **GIVEN** 手寫 project `hand`；沒有 `ghost`
- **WHEN** 分別 `PATCH /api/repo-projects/ghost` 與 `PATCH /api/repo-projects/hand`，本體 `{"name":"x"}`
- **THEN** 前者回 404（`code` 為 `unknown_project`），後者回 409（`code` 為 `not_repo_project`），`hand` 不變

#### Scenario: 修改 stages 時同時設定階段對應

- **GIVEN** `app` 的 stages 為 `Plan`、`Build`、`Done`，`stage_phases` 為 `["plan",null,null]`
- **WHEN** `PATCH /api/repo-projects/app`，本體 `{"stages":[{"name":"Plan","from":"Plan","phase":"plan"},{"name":"Build","from":"Build","phase":"implement"},{"name":"Done","from":"Done","phase":"complete"}]}`
- **THEN** 回 204；稍後 `stage_phases` 為 `["plan","implement","complete"]`

#### Scenario: phase 省略視為不對應

- **GIVEN** `app` 的 `stage_phases` 為 `["plan","implement"]`，stages 為 `Plan`、`Build`
- **WHEN** `PATCH /api/repo-projects/app`，本體 `{"stages":[{"name":"Plan","from":"Plan"},{"name":"Build","from":"Build"}]}`（每列都沒有 `phase`）
- **THEN** 回 204；稍後 `stage_phases` 為 `[null,null]`

#### Scenario: 排序時階段對應跟著各列走

- **GIVEN** `app` 的 stages 為 `A`、`B`，`stage_phases` 為 `["plan","review"]`
- **WHEN** `PATCH /api/repo-projects/app`，本體 `{"stages":[{"name":"B","from":"B","phase":"review"},{"name":"A","from":"A","phase":"plan"}]}`
- **THEN** stages 依序為 `B`、`A`，`stage_phases` 為 `["review","plan"]`

#### Scenario: 兩列對應同一個階段

- **WHEN** `PATCH` 的 `stages` 有兩列的 `phase` 都是 `review`
- **THEN** 回 400，`code` 為 `invalid_stages`，stages、階段對應與進度都不變

#### Scenario: 只改名稱不影響階段對應

- **GIVEN** `app` 的 `stage_phases` 為 `["plan","implement"]`
- **WHEN** `PATCH /api/repo-projects/app`，本體 `{"name":"App 前端"}`
- **THEN** 回 204；`stage_phases` 仍為 `["plan","implement"]`

#### Scenario: 改名但 phase 跟著走不算改變

- **GIVEN** `app` 的 stages 為 `Plan`、`Build`，`stage_phases` 為 `["plan","implement"]`；自動 task 的同步狀態 `applied` 為 `foo`／`implement`／3／8
- **WHEN** `PATCH /api/repo-projects/app`，本體 `{"stages":[{"name":"Plan","from":"Plan","phase":"plan"},{"name":"Make","from":"Build","phase":"implement"}]}`
- **THEN** 回 204；`stage_phases` 為 `["plan","implement"]`，該 task 的 `applied` 不被清除

#### Scenario: 只重排不算改變

- **GIVEN** `app` 的 stages 為 `A`、`B`，`stage_phases` 為 `["plan","review"]`；自動 task 的 `applied` 為 `foo`／`review`／5／5
- **WHEN** `PATCH`，本體 `{"stages":[{"name":"B","from":"B","phase":"review"},{"name":"A","from":"A","phase":"plan"}]}`
- **THEN** 回 204；該 task 的 `applied` 不被清除

#### Scenario: phase 換了擁有者算改變

- **GIVEN** `app` 的 stages 為 `A`、`B`，`stage_phases` 為 `["plan","review"]`；自動 task 的 `applied` 為 `foo`／`review`／5／5
- **WHEN** `PATCH`，本體 `{"stages":[{"name":"A","from":"A","phase":"review"},{"name":"B","from":"B","phase":"plan"}]}`
- **THEN** 回 204；該 task 的 `applied` 被清為無

### Requirement: 移除 Repo Project

系統必須提供 `DELETE /api/repo-projects/<pid>`，不需要請求本體。接受且持久化成功時回 204：該 Repo Project 的定義與它所有
task 的進度與同步狀態一併從狀態與狀態檔移除；`pid` 的查找規則同「修改 Repo Project 名稱與 stages」（被撞名隱藏的 Repo Project 仍可移除）：
是手寫 project 的 id 而不是 Repo Project 的 id 回 409，`code` 為 `not_repo_project`；兩者都不是回 404，`code` 為 `unknown_project`；
寫檔失敗回 500，`code` 為 `persist_failed`，記憶體不變。移除不影響其他 Project、不影響任何 pane，也不改寫 `cockpit.toml`。

#### Scenario: 移除成功

- **GIVEN** Repo Project `app` 有兩張 task、各有進度，兩個 pane 仍開著
- **WHEN** `DELETE /api/repo-projects/app`
- **THEN** 回 204；稍後 `projects` 不含 `app`，`detected_repos` 重新含該 repo；狀態檔中沒有 `app` 的定義與進度；
  其他 Project 不變

#### Scenario: 移除後重新加入，進度重新開始

- **GIVEN** 移除 `app` 之前其 task 在 `Review`
- **WHEN** 移除 `app` 後再加入同一個 repo
- **THEN** 新的 Repo Project 中該 pane 的 task 在第一個 stage、標記 `none`

#### Scenario: 移除不存在或手寫的 project

- **WHEN** `DELETE /api/repo-projects/ghost` 與 `DELETE /api/repo-projects/hand`（手寫）
- **THEN** 分別回 404（`unknown_project`）與 409（`not_repo_project`），狀態不變

#### Scenario: 移除後同步狀態一併消失

- **GIVEN** Repo Project `app` 的 task `local~wJ:p1` 有同步狀態（`manual`）
- **WHEN** `DELETE /api/repo-projects/app` 後再加入同一個 repo
- **THEN** 新 Repo Project 中該 task 沒有舊的同步狀態：在第一個 stage、標記 `none`，之後的偵測結果依新 task 的規則套用

### Requirement: Repo Project 的輸入驗證

系統必須對加入與修改端點的輸入做下列驗證，不符合時回 400，本體含 `error` 與 `code`，且不改變任何狀態：本體不是合法
JSON 物件、欄位型別不對（`repo` 非字串、`stages` 非陣列、`phases` 非陣列（`null` 例外，等同省略）或其中有不是字串也不是 `null` 的項目、`PATCH` 的 `phase` 不是字串也不是 `null` 等）、有未知欄位、`PATCH` 的 `name` 與 `stages` 都沒給 → `code` 為
`invalid_body`；`name` 去除前後空白後不是 1～64 個字元，或含控制字元、零寬或雙向格式字元 → `invalid_name`；`stages` 不是 1～12 個、任一 stage
名稱去除前後空白後不是 1～32 個字元、含控制字元或零寬、雙向格式字元、與同一份清單中另一個 stage 名稱相同，或 `PATCH` 的 `from` 不符合「修改
Repo Project 名稱與 stages」的規則（不存在或被重複引用），或階段對應不合法 → `invalid_stages`。階段對應不合法指下列任一：`POST` 的
`phases` 長度與 `stages` 不同；任一值是字串但不是 `plan`、`implement`、`review`、`complete` 之一；非 `null` 的值在同一份清單（`POST` 的
`phases`，或 `PATCH` 各列的 `phase`）中重複。名稱與 stage 名稱在驗證與保存前一律去除前後空白。

檢查順序固定，同時有多處不合法時回報最先命中的：本體解析（`invalid_body`，含 `PATCH` 的 `name` 與 `stages` 都沒給）→ `PATCH` 的 `pid`（`unknown_project`、`not_repo_project`）→ `name`（`invalid_name`）→
`stages`（`invalid_stages`）→ `phases`／`phase`（`invalid_stages`）。`POST` 則為本體解析 → `name` → `stages` → `phases` → `repo`（`repo_already_added`、`repo_not_detected`）。

#### Scenario: 名稱太長

- **WHEN** `POST /api/repo-projects`，`name` 為 65 個字元
- **THEN** 回 400，`code` 為 `invalid_name`，沒有新增 Project

#### Scenario: 空白名稱

- **WHEN** `PATCH /api/repo-projects/app`，本體 `{"name":"   "}`
- **THEN** 回 400，`code` 為 `invalid_name`，名稱不變

#### Scenario: stage 數量與重複

- **WHEN** 分別送 `stages` 為 `[]`、13 個不同名稱、`["Plan","Plan"]` 的加入請求
- **THEN** 三者都回 400，`code` 為 `invalid_stages`，沒有新增 Project

#### Scenario: stage 名稱含控制字元

- **WHEN** `stages` 含 `"Plan\u0007"`
- **THEN** 回 400，`code` 為 `invalid_stages`

#### Scenario: 本體不合法

- **WHEN** `POST /api/repo-projects`，本體分別為 `{not json`、`{"repo":"x","stages":["A"],"extra":1}`、`{"repo":1,"stages":["A"]}`
- **THEN** 三者都回 400，`code` 為 `invalid_body`，沒有新增 Project

#### Scenario: 前後空白被去除

- **WHEN** `stages` 為 `[" Plan ","Build"]`
- **THEN** 接受，Project 的 `stages` 為 `Plan`、`Build`

#### Scenario: 階段對應重複

- **WHEN** `POST /api/repo-projects`，`stages` 為 `["A","B"]`、`phases` 為 `["plan","plan"]`
- **THEN** 回 400，`code` 為 `invalid_stages`，沒有新增 Project

#### Scenario: 階段名稱不合法

- **WHEN** `POST /api/repo-projects`，`stages` 為 `["A"]`、`phases` 為 `["done"]`
- **THEN** 回 400，`code` 為 `invalid_stages`，沒有新增 Project

#### Scenario: phases 為 null 等同省略

- **WHEN** `POST /api/repo-projects`，本體 `{"repo":"d:\\work\\app\\.git","stages":["A","B"],"phases":null}`
- **THEN** 回 201；`stage_phases` 為 `[null,null]`

#### Scenario: 多處不合法時依檢查順序回報

- **WHEN** `PATCH /api/repo-projects/app`，本體 `{"name":"   ","stages":[{"name":"Plan","from":"Plan","phase":"nope"}]}`；或 `POST` 本體 `name` 為空白且 `phases` 含 `"nope"`
- **THEN** 兩者都回 400，`code` 為 `invalid_name`（名稱先於 stages 與 phases）；`PATCH` 的 `pid` 不存在時則先回 404 `unknown_project`

#### Scenario: phases 型別不對

- **WHEN** `POST /api/repo-projects`，本體 `{"repo":"x","stages":["A"],"phases":"plan"}`，或 `PATCH` 某列的 `phase` 為 `1`
- **THEN** 兩者都回 400，`code` 為 `invalid_body`，狀態不變

### Requirement: Repo Project 進度的保存與清除

系統必須把 Repo Project task 的進度（所在 stage 與標記）與同步狀態（自動或手動、上一次套用的偵測結果，見 `openspec-stage-sync`
「卡片同步狀態的持久化」）以 Repo Project id 與 task id 為鍵保存，與手寫 project 的進度分開存放
（id 撞名時兩邊互不影響），並隨每次需要寫檔的被接受寫入存入狀態檔（見 `pipeline-progress`「狀態檔格式與持久化」）。寫檔時 Repo Project 的進度與同步狀態全部依已保存的對照表寫出，不以目前展開出來的
task 過濾，使啟動初期尚未完成 pane 歸類時的任何一次寫入都不會抹掉其他 pane 的進度或同步狀態。

系統只在下列情況清除某張 Repo Project task 的進度與同步狀態，並同步寫檔：該 task 所屬 runtime 目前為 `connected`、這輪連線的沉降重拿
（`runtime-driver`「連線後沉降重拿」）已經完成，且它目前的 pane 樹中沒有這個 pane id。剛連上、沉降重拿完成前的 pane 樹可能
還不完整，因此不據以清除。已 exited 但仍在 pane 樹中的 pane 不清除；runtime 未連線、尚未連上（啟動初期）時一律保留。清除檢查
依據 runtime 當下的連線狀態與 pane 樹（不是可能落後的投影），在每一輪 repo 判定後執行，因此 Cockpit 關閉期間被關掉的 pane，
在其 runtime 重新連上、沉降重拿完成後的第一輪就會被清掉，不依賴歸類結果是否改變。pane 的 cwd 離開 repo、暫時性 git 錯誤
使 pane 暫不歸類時也不清除，pane 再次歸入同一個 repo 時接回原進度。pane 關閉後不保留「已結束」紀錄，也不在其他 pane 重用
該進度與同步狀態；`pane_moved` 造成 pane id 改變時，舊 id 視為消失、新 id 視為新 pane。

#### Scenario: pane 關閉，進度一併移除

- **GIVEN** `local~wJ:p1` 的 task 在 `Review`；runtime `local` 為 `connected`
- **WHEN** pane `wJ:p1` 被關閉（不在最新 pane 樹中）
- **THEN** 該 workstream 與 task 從投影消失，狀態檔中該 task 的進度被移除

#### Scenario: 已 exited 但仍在 pane 樹中不清除

- **GIVEN** pane `wJ:p1` 的 task 在 `Review`，runtime `local` 為 `connected`
- **WHEN** HERDR 回報該 pane `exited` 為 true 但仍在 pane 樹中
- **THEN** 該 workstream 從投影消失（已 exited 的 pane 不歸類），但 `local~wJ:p1` 的進度仍保留在狀態檔

#### Scenario: Cockpit 關閉期間被關掉的 pane

- **GIVEN** 狀態檔有 `local~wJ:p1` 與 `local~wJ:p2` 的進度；Cockpit 關閉期間 `wJ:p2` 被關掉
- **WHEN** 重啟，runtime `local` 連上且沉降重拿完成，之後第一輪 repo 判定結束
- **THEN** `local~wJ:p2` 的進度被清掉並寫檔，`local~wJ:p1` 的進度保留；即使這一輪 pane 歸類結果與重啟前記憶體中的相同

#### Scenario: runtime 斷線不清除

- **GIVEN** runtime `wsl` 的 pane `w1:p1` 對應的 task 在 `Review`
- **WHEN** `wsl` 斷線，之後重新連上且 snapshot 中仍有 `w1:p1`
- **THEN** 斷線期間進度仍在狀態檔；重連後 task 仍在 `Review`

#### Scenario: 啟動初期的寫入不抹掉進度

- **GIVEN** 狀態檔有 Repo Project `app` 的兩張 task 進度；剛啟動，runtime 尚未連上、pane 歸類結果為空
- **WHEN** 對手寫 project 的 task 做一次被接受的操作
- **THEN** 新寫出的狀態檔仍含 `app` 的兩張 task 進度

#### Scenario: cwd 離開再回來

- **GIVEN** pane `wJ:p1` 的 task 在 `Review`；pane 內 `cd` 離開 `app` repo（pane 仍存在）
- **WHEN** 下一次 snapshot 後，其 workstream 從 `app` 消失；之後再 `cd` 回 `app`
- **THEN** 離開期間進度未被清除；回來後 workstream 重現，task 仍在 `Review`

#### Scenario: 指向未設定 runtime 的進度

- **GIVEN** 狀態檔有 task 進度 `ghost~wJ:p1`，設定中沒有 runtime `ghost`
- **WHEN** 啟動後做一次被接受的操作
- **THEN** 啟動成功並記 warn；新寫出的狀態檔不含該項目

#### Scenario: pane 關閉，同步狀態一併移除

- **GIVEN** `local~wJ:p1` 的 task 有同步狀態（`manual`，已套用過 change `foo`）；runtime `local` 為 `connected`
- **WHEN** pane `wJ:p1` 被關閉（不在最新 pane 樹中）
- **THEN** 狀態檔中該 task 的進度與 `sync` 一併被移除

#### Scenario: cwd 離開再回來，同步狀態保留

- **GIVEN** pane `wJ:p1` 的 task 有同步狀態（`manual`）；pane 內 `cd` 離開 `app` repo 後再回來
- **WHEN** 重新歸入 `app`
- **THEN** 離開期間同步狀態未被清除；回來後該 task 的 `sync.mode` 仍為 `manual`

#### Scenario: 啟動初期的寫入不抹掉同步狀態

- **GIVEN** 狀態檔有 Repo Project `app` 的 task 同步狀態；剛啟動，runtime 尚未連上、pane 歸類結果為空
- **WHEN** 對手寫 project 的 task 做一次被接受的操作
- **THEN** 新寫出的狀態檔仍含 `app` 的 task 同步狀態

## ADDED Requirements

### Requirement: Stage 對應 OpenSpec 階段

系統必須讓每個 Repo Project 的每個 stage 可對應一個 OpenSpec 階段：`plan`（規劃）、`implement`（實作）、`review`（審查）、`complete`（完成），
或不對應（`null`）。同一個階段最多對應一個 stage（非 `null` 的值互不重複）。對應存成與 `stages` 逐項對齊的陣列（長度恆等於 `stages`），
隨 Repo Project 的定義保存於狀態檔，並以 `stage_phases` 輸出在投影（見 `state-projection`「Project 投影」）。階段對應只決定 OpenSpec 進度偵測
結果要把卡片移到哪一站（見 `openspec-stage-sync`），不影響 stage 的名稱、順序、進度操作或 StageStatus。沒有任何 stage 對應階段的 Repo Project，
行為與沒有此功能時完全相同。手寫 project 沒有階段對應。

#### Scenario: 同一階段最多對應一站

- **GIVEN** Repo Project 的 stages 為 `A`、`B`
- **WHEN** 兩個 stage 都對應 `review`
- **THEN** 這份對應不合法，被拒絕（見「Repo Project 的輸入驗證」）

#### Scenario: 不對應的 stage

- **GIVEN** stages 為 `Plan`、`Build`、`Done`，`stage_phases` 為 `["plan",null,"complete"]`
- **WHEN** 偵測結果的階段為 `implement`
- **THEN** 沒有 stage 對應 `implement`，卡片不移動（見 `openspec-stage-sync`）；`Build` 的名稱、順序與進度操作不受影響

#### Scenario: 沒有任何對應時行為不變

- **GIVEN** Repo Project 的 `stage_phases` 全為 `null`
- **WHEN** 偵測結果出現、變動
- **THEN** 卡片不會因偵測而移動，進度操作與沒有此功能時相同
