# design：repo-projects

## Context

動機見 `proposal.md`「Why」。決定設計的現況：

- Project 定義只來自 `cockpit.toml`，只在啟動時讀一次（`cockpit/src/config.rs` `load`）。`cockpit/src/app.rs`
  `build_components` 只在有 project 時才建立 `ProgressService` 與失效覆蓋的清除工作，零設定模式沒有狀態檔
  （`config.rs` `resolve_state_path`）。
- core 的 `StoreHandle::set_domain` 可以整份換掉 `DomainState` 並觸發重新投影；投影 `project_with_stale`
  是純函數，只吃 `RuntimeStore` 與 `DomainState`（`cockpit-core/src/projection.rs`）。core 不做 IO（ADR-0003）。
- 寫入一律經 `ProgressService::write` 的單一鎖：驗證 → 原子寫檔 → `set_domain`（`progress_service.rs`）。
  另開工作直接呼叫 `set_domain` 會與它互相覆蓋。
- pane 的 cwd 只在 snapshot 與 `pane_created`／`pane_updated` 的完整 PaneInfo 裡；cwd 改變沒有事件，靠定期重新抓
  snapshot（預設 `resnapshot_secs = 30`，`docs/research/2026-09-27/file-review-probe.md`）。
- pane id 形如 `wJ:p1`，HERDR 重啟後不變、關掉的 id 不重用（`docs/research/2026-09-19/live-output-acceptance.md`、
  `docs/research/2026-09-16/pipeline-projection-acceptance.md`）；`pane_moved` 會改變 id。Windows 與 WSL 的 HERDR
  各自編號，可能撞號（`agent-reporting`「pane 身分判定」）。
- 現有的根目錄判斷 `cockpit-files` `find_root` 把 linked worktree 當成自己的根，無法把同一個 repo 的 worktree
  歸在一起。`cockpit-git` 的 `GitRunner` 已能對 Windows 路徑與 WSL 路徑（經 `wsl.exe --exec`）執行允許清單內的查詢，
  最多同時 4 支、單次 10 秒逾時。

## Goals / Non-Goals

**Goals:**

- 手寫 project 的程式路徑（設定解析、綁定解析、進度操作、agent 端點）盡量不分岔：Repo Project 在進入投影與寫入
  服務之前就展開成一般的 `ProjectDef`。
- 所有變更（加入、移除、改名、改 stage、pane 進出、進度）都經同一把鎖，不會互相覆蓋。
- 啟動時、runtime 斷線時不誤刪進度。

**Non-Goals:**（範圍見 `proposal.md`「非目標」）

- 不做 repo 偵測結果的持久化：每次啟動重新偵測。
- 不把偵測擴到「沒有 cwd 的 pane」或「runtime 沒有路徑對應」的情況（例如只給 `command` 的 runtime）：這些 pane
  不歸入任何 repo。

## Decisions

### D1：repo 身分用 git 的共同 `.git` 目錄

對 pane cwd（經既有 `PathMapping` 轉成主機路徑）執行
`git -C <cwd> rev-parse --path-format=absolute --git-common-dir --git-dir --show-toplevel`，在 `cockpit-git` 新增一個
sealed 查詢。輸出三行，依引數順序：共同 `.git` 目錄、這個工作樹自己的 git 目錄、工作樹根目錄。結束碼 128（不是 repo、
裸 repo、在 `.git` 內、cwd 不存在）→ 這個 cwd 不屬於任何 repo；此時不看 stdout（裸 repo 與 `.git` 內會先印兩行才失敗）。
其他非 0 結束碼（例如 WSL 端找不到 git 時 `env` 回 127）歸為暫時錯誤。
其他執行錯誤（逾時、`git` 不存在、不受信任）→ 暫時無法判定，該 pane 不歸類，稍後重試。

- **repo key**：共同 `.git` 目錄的主機路徑。Windows 輸出的正斜線轉成反斜線、整串轉小寫（見 D2）；WSL 輸出的 POSIX
  路徑經既有的 `wsl_host_path` 轉成 `\\wsl.localhost\<distro>\...`。key 只是比對用的字串，不是使用者輸入。
- **預設名稱**：共同目錄名稱是 `.git` 時取它上一層資料夾名稱（主 worktree 的資料夾名）；否則取共同目錄名稱並去掉
  結尾的 `.git`。
- **worktree 標註**：自己的 git 目錄不等於共同目錄（正規化後比較）時，該 pane 在 linked worktree 裡，畫面標註工作樹
  根目錄的資料夾名稱。不用「工作樹根目錄是否等於共同目錄上一層」判斷，因為 submodule 的共同目錄是
  `.git/modules/<名稱>`，那樣會誤判成 linked worktree。

`--path-format` 是 git 2.31 加入（git RelNotes 2.31.0）；不帶時子目錄會回相對路徑。本機 Windows git 2.50.1、WSL git
2.43.0（2026-10-08 實測）。實作前依鐵則 3 再對 git 官方文件確認旗標與輸出順序。

**不選的做法**：

- 讀 `.git` 檔案的 `gitdir:` 與 `commondir`：不用啟動 git，但要自己處理相對路徑、`GIT_DIR` 類設定與 WSL 經
  `\\wsl.localhost` 的符號連結（專案 memory：Windows 看 WSL 符號連結會失敗）。
- 沿用 `find_root`：worktree 會被拆成不同 repo，違反 brainstorming 的決定。

### D2：repo 偵測是 `cockpit` 的背景工作，結果經寫入服務進 Domain

新增 `RepoResolver`（`cockpit` crate）：

1. 監看投影的 watch channel。投影改變時，收集每個**已連線**、有路徑對應的 runtime 裡、未 exited、cwd 非空的 pane。
   未連線的 runtime 不查 git（對斷線的 WSL runtime 執行 `wsl.exe -d` 可能把發行版開機，違反 `herdr-runtime-session`
   「WSL 探測」不啟動發行版的設計），它的 pane 沿用上次的歸類結果。
2. 以 `(runtime, cwd)` 為鍵查快取；沒有或過期才查 git。成功的結果 10 分鐘後過期，「不是 repo」與暫時錯誤 60 秒後過期。
   另有 60 秒的定時器，讓過期項目在投影沒變時也會重查。查詢依序執行（一次一支），避免佔滿 `GitRunner` 的 4 個名額，
   拖慢左欄「變更」分頁。
3. 算出 `pane_repos`：`(runtime, pane_id) → { repo key, 預設名稱, worktree 資料夾名稱（主 worktree 為空）}`，
   **和上次送出的內容不同時**才交給寫入服務。投影會因此改變並再次喚醒 resolver，但第二次算出的內容相同，不會無限循環。
   這些欄位只取決於 cwd，不含會變動的 pane label 或 agent 名稱（那些在投影時取，見 D3）。
4. 每一輪（投影改變或定時器）結束時請寫入服務做一次「清除檢查」（D4）。

**repo key 正規化**：Windows 路徑把正斜線轉成反斜線、**整串轉小寫**（NTFS 不分大小寫，同一個資料夾以不同大小寫
`cd` 進去時 git 回的路徑大小寫可能不同）；WSL 輸出的 POSIX 路徑經 `wsl_host_path` 轉成 `\\wsl.localhost\<distro>\...`，
保留大小寫（Linux 區分大小寫）。預設名稱取自 git 原始輸出，保留大小寫。

查詢介面以 trait 注入（正式實作用 `GitRunner`），讓 resolver 的測試不必啟動 git。

**不選的做法**：在前端依 cwd 分組。agent 回報要在後端找得到 workstream，前端分組做不到。

### D3：Repo Project 在寫入服務裡展開成一般 `ProjectDef`

`DomainState` 新增：

- `repo_projects`：Repo Project 定義 `{ id, name, repo key, stages }`。
- `repo_progress`：Repo Project 的 task 進度，以 Repo Project id、task id 為鍵，**與手寫 project 的 `progress` 分開存放**。
  id 撞名時兩邊互不影響；移除 Repo Project 只清自己的 `repo_progress`。
- `pane_repos`：D2 的結果，不持久化。

core 新增一個純函數，輸入「手寫 project 清單、Repo Project 定義、`pane_repos`」，輸出「實際生效的 project 清單」
與警告：

- 每個 Repo Project 展開成一個 `ProjectDef`：`pane_repos` 中 repo key 相符的每個 pane 各一條 workstream 與一張 task。
- project 清單的順序：手寫的在前（設定檔順序），Repo Project 在後，依名稱（不分大小寫）排序、同名再依 id。不保存加入順序，
  因為狀態檔以物件保存、輸出依 key 排序。
- workstream 與 task 的 id 都是 `<runtime id>~<pane id>`（例如 `local~wJ:p1`）。runtime id 只檢查非空、不重複，**可能含
  `~`**；HERDR pane id 形如 `w<ws>:p<n>`、不含 `~`。所以需要從 id 拆回 runtime 時一律從**最後一個** `~` 切開。
  前端組 URL 時照現有做法逐段編碼。
- **名稱與排序在投影時取**：展開時只放 id 與 worktree 標註；投影對「固定 pane」的 workstream 從當下的 pane 資料取名稱
  （pane 的 label，空的話用 agent 名稱，再空用 pane id；task 標題相同），並依 runtime 在設定中的順序、再依 pane 在
  snapshot 的順序排列。pane label 或 agent 改變時名稱跟著變，不必經過寫入服務。
- workstream 的綁定是新的「固定 pane」綁定（`runtime-binding` 的既有解析不參與）：pane 未 exited 時為 `bound`，
  來源 `pane`；不在 pane 樹或已 exited 時為 `unbound`（`pane_repos` 更新前的短暫空窗）；runtime 斷線為
  `runtime_disconnected`（三種情況的 `source` 都是 `pane`）。固定 pane 的 workstream 不接受改綁：覆蓋端點的 `PUT` 與
  `DELETE` 都回 409，`code` 為 `not_overridable`；畫面不顯示它的「改綁」鈕。
- 手寫 project 與 Repo Project id 相同時，Repo Project 不展開，並在手寫 project 的 `warnings` 加一則訊息
  （`code` 為 `repo_project_id_conflict`，參數為該 id）。被隱藏的 Repo Project 仍可經 `/api/repo-projects/{pid}`
  改名或移除，因為那組端點只查 Repo Project 定義。

`DomainState.projects` 仍是「實際生效的清單」，所以投影、進度操作、agent 端點的既有程式不必分辨兩種 project。
寫入服務在每次 `repo_projects` 或 `pane_repos` 變動時重算這份清單，再 `set_domain`。進度的讀寫依 project 種類分流到
`progress` 或 `repo_progress`，這是唯一要分辨種類的地方。Repo Project 不經過 `set_active`／`active`（D4）。

**何時寫檔**：寫入服務**比較寫出的檔案內容**（序列化後的狀態檔）而不是比較 `DomainState`，內容不變就不寫。`pane_repos`
不持久化，Repo Project 的展開結果也不寫進檔案，所以 pane 進出、cwd 改變只會改記憶體，不會落檔。否則使用者只是開啟
新版，舊的 v2 檔就會在沒有任何操作下被升成 v3，零設定模式也會在啟動時建檔。

**不選的做法**：

- 投影時才即時推導：投影、進度操作、agent 端點都要各自加一套「這個 task 存在嗎」的判斷，分岔點多。
- 把偵測到的 pane 寫成設定檔式的 workstream（brainstorming 的做法二）：同資料夾兩個 pane 會歧義、pane id 不該當
  設定鍵，已被使用者否決。

### D4：進度的保存與清除

- Repo Project task 的進度存在 `repo_progress`（D3），載入與寫出都**不經過**手寫 project 用的 `resolve_tasks`
  （它會丟掉設定中沒有的 task），而是依對照表全部讀入、全部寫出，不以「目前展開出來的 task」過濾。這樣啟動初期
  `pane_repos` 還是空的時候，任何一次寫入都不會抹掉其他 pane 的進度。
- **只在下列情況清除** Repo Project 某張 task 的進度：該 runtime 目前已連線，而且在它目前的 pane 樹中**沒有**這個 pane id。
  已 exited 但仍在樹中的 pane 不清除（保守；不確定 HERDR 是否會在同一個 id 下重啟 pane）。runtime 未連線、尚未連上
  （啟動初期）時一律保留。cwd 離開 repo 不算清除條件，pane 再 `cd` 回來就接回原進度。
- **清除檢查由寫入服務在鎖內執行**，依據是 core 的 `RuntimeStore` 當下的連線狀態與 pane 樹，不是投影（專案 memory
  `check-against-lagging-projection-misses-fresh-writes`）。resolver 每一輪都會請求一次（D2），所以 Cockpit 關閉期間被關掉的
  pane，在 runtime 重新連上、**沉降重拿完成後**的第一輪就會被清掉，不依賴 `pane_repos` 有沒有變。有清掉東西才寫檔。
  剛連上時 HERDR 的首份 snapshot 可能還沒含全部恢復的 pane，所以 driver 在 `Connected` 帶 `settled` 旗標（初次 snapshot
  為 false，沉降重拿完成後為 true，重連歸 false），清除只在 `settled` 為 true 時進行；否則一次重啟就可能不可逆地刪掉進度。
- 指向設定中不存在的 runtime（從 task id 最後一個 `~` 前取出）的項目，載入時忽略並記 warn，下次寫入時不寫出
  （與手寫 project 的覆蓋規則一致）。
- Repo Project 的 workstream 只有一張 task：**該 task 標記為 `none` 時就是目前 task**，標了 `completed`／`failed` 時沒有
  目前 task（與 `pipeline-domain`「目前 task」的清除規則一致）；`active` 不為 Repo Project 保存。宣告目前 task 的端點
  對它照既有規則判定（已標記 → 409），通過時回 204、狀態不變。

### D5：狀態檔 v3 與位置

- 形狀：v2 的 `projects` 不變，加 `repo_projects`：
  `{"<pid>": {"name": "...", "repo": "<repo key>", "stages": [...], "tasks": {"<tid>": {"stage": "...", "mark": "..."}}}}`。
  寫出一律 `version: 3`；v1、v2 依舊規則讀取，沒有 `repo_projects` 視為空；v1、v2 檔出現 `repo_projects` 欄位視為損毀，
  啟動失敗（與 v1 檔出現 `active` 的規則同理）；**v3 檔必須有 `repo_projects`**（可以是空物件），缺少視為損毀，與既有
  「必填欄位缺漏就啟動失敗」的原則一致。`deny_unknown_fields` 照舊。手寫 project 的 `projects` 區段只放手寫 project。
- 載入：Repo Project 的 id、名稱、stage 不符合 D6 的規則，或兩個 Repo Project 的 `repo` 相同 → 啟動失敗（與損毀同等），
  訊息含狀態檔路徑；task 的 stage 不在該 project 的 stages → 改用第一個 stage、保留標記、加警告（與手寫規則一致）。
- 位置：有設定檔時照舊。**零設定模式**改為 `%LOCALAPPDATA%\ai-cockpit\cockpit.state.json`，第一次寫入時建立資料夾；
  `LOCALAPPDATA` 不存在時沒有狀態檔，只存在記憶體並記 warn。測試用的 inline 設定沒有狀態檔，同樣只存在記憶體。
- 寫入服務改為**永遠建立**；沒有狀態檔路徑時只更新記憶體。原本「Project 清單為空時不讀也不寫狀態檔」改為：
  有路徑就在啟動時讀，只有序列化內容改變時才寫（D3「何時寫檔」），所以沒加過 Repo Project、也沒有手寫 project 時仍然
  不會建立檔案。既有測試 `cockpit/tests/app.rs` `no_projects_creates_no_state_file` 的意圖保留，依新規則改寫。

**不選的做法**：另開 `cockpit.projects.json`。兩份檔案無法一起原子寫入，移除 project 時定義與進度可能對不上。

### D6：管理端點與驗證

新增端點都掛既有的本機同源來源檢查，本體為 JSON：

| 方法與路徑 | 本體 | 成功 |
|---|---|---|
| `POST /api/repo-projects` | `{"repo": "<key>", "stages": ["..."], "name": "..."}`（`name` 選填） | 201 `{"id": "..."}` |
| `PATCH /api/repo-projects/{pid}` | `{"name": "...", "stages": [{"name": "...", "from": "舊名"}]}`（`name`、`stages` 皆選填、至少一個；`from` 選填，省略或 `null` 表示新增的 stage） | 204 |
| `DELETE /api/repo-projects/{pid}` | 無 | 204 |

- `repo` 必須是目前 `pane_repos` 裡有的 key（只能加入偵測到的 repo，不接受任意路徑），否則 404 `repo_not_detected`；
  已加入過的 repo → 409 `repo_already_added`。
- 名稱：去除前後空白後 1～64 個字元，不含控制字元、零寬或雙向格式字元（U+200B、U+200E–U+200F、U+061C、U+202A–U+202E、
  U+2060–U+2064、U+2066–U+2069、U+FEFF）。U+200C（ZWNJ）、U+200D（ZWJ）放行：emoji 組合序列與波斯文、印度系文字需要它們，
  且不改變顯示方向。未給時用 D1 的預設名稱。
- stages：1～12 個；每個去除前後空白後 1～32 個字元、不含上述字元、互不相同。`from` 必須是目前的 stage 名稱，
  每個舊名最多被一個新 stage 引用。task 的新 stage：所在 stage 被某個新 stage 以 `from` 引用 → 該新 stage；否則 → 第一個
  新 stage。標記保留。
- id：由名稱產生，`[A-Za-z0-9_-]` 以外的字元換成 `-`、連續的 `-` 合併、去掉頭尾 `-`、截到 48 字元，空的話用 `repo`；
  與現有所有 project id 重複時加 `-2`、`-3`…。id 產生後不隨改名改變。
- `pid` 不是任何 Repo Project 的 id：是手寫 project 的 id → 409 `not_repo_project`；都不是 → 404 `unknown_project`。
- `name`、`stages` 為 `null` 視同省略。本體不是合法 JSON、欄位型別不對、有未知欄位、`PATCH` 兩個欄位都沒給 → 400 `invalid_body`；名稱不合規則 → 400
  `invalid_name`；stages 不合規則（含 `from` 不存在或重複引用）→ 400 `invalid_stages`。
- 寫檔失敗 → 500 `persist_failed`（沿用既有寫入端點的 code），記憶體中的狀態不變。
- 錯誤本體沿用 `{"error", "code", "params"}`（`ui-language`「後端訊息代碼」），每個新 code 都要在 `i18n.js` 兩種語言加
  `msg.<code>`（對帳測試會擋）。

### D7：agent 免帶 id 推進

`POST /api/agent/advance`：身分判定沿用「pane 身分判定」（讀投影）；**候選 task 在寫入鎖內依 Domain 計算**，不讀投影（否則 agent 剛
`start` 新 task 後立刻推進，會在投影合併窗內推進舊的目前 task 並覆寫剛宣告的那張）。候選 task：每條綁定到這個 pane 的 workstream，若有目前 task
就取它，否則若恰有一張 task 就取它，否則不提供候選。候選恰為一張 → 照既有「agent 推進」規則推進；零張 → 404
`no_task_for_pane`；兩張以上 → 409 `ambiguous_task`。手寫 project 也適用，所以已經用 `start` 宣告過目前 task 的
agent 也能用。

### D8：投影

- 每個 project 加 `kind`（`config` 或 `repo`）；Repo Project 另有 `repo`（key）。workstream 加選填的 `worktree`
  （linked worktree 的資料夾名稱）。
- 最上層加 `detected_repos`：`pane_repos` 中尚未被任何 Repo Project 加入的 repo，每項 `{repo, name, pane_count}`，
  依 `name`（不分大小寫）排序、同名再依 `repo`。
- 新的 `Message` 變體：id 撞名警告。被撞名隱藏的 Repo Project 在畫面上看不到、也不在偵測區（因為已加入），只能經 API
  改名或移除；警告訊息寫明這一點。這是罕見情況（要先在畫面加入，再手寫一個同 id 的 project），不另做畫面。

### D9：前端

依 brainstorming 第 1 段（使用者 2026-10-08 同意）：左欄 Project 分頁加「偵測到的 repo」區與「加入」鈕；Repo Project
有「⋯」選單（改名、編輯 stage、移除）；編輯 stage 用對話框；移除前確認。預設 stage 由前端依介面語言送出：
中文「規劃、實作、審查、完成」，英文「Plan、Implement、Review、Complete」。空狀態文字拿掉「需要重啟」。外觀依專案
memory 過 frontend-design 的設計審核。所有名稱以 `textContent` 呈現，不以 HTML 插入。

## Risks / Trade-offs

- [降版]：v0.1.3 以前讀到 v3 狀態檔會拒絕啟動 → `CHANGELOG.md` 註明；自動更新只會升版。
- [cwd 反映延遲最多約 30 秒] → 沿用既有 `resnapshot_secs`，不新增輪詢；文件寫明。
- [git 查詢成本] 每個新 cwd 約 60～320 ms，WSL 約 120～180 ms（2026-10-08 實測）→ 快取加依序查詢；30 個 pane 首次約數秒。
- [同一個 repo 從 Windows 與 WSL（`/mnt/d/...`）兩邊開]：key 不同，會列成兩個 repo → 已知限制，文件寫明。
- [暫時錯誤時 pane 從 workstream 消失]：git 逾時等錯誤讓該 pane 暫時不歸類。D4 的清除條件不看歸類，所以進度不會丟。
- [寫入服務成為更多操作的瓶頸]：pane_repos 更新只在內容改變時發生，頻率低。
- [`ui_preview` 沒有真的後端狀態]：它推送手工組的投影，寫入端點只記錄請求並回 204。→ 前端驗收腳本只驗畫面呈現與
  送出的請求（fixture 直接帶 `detected_repos` 與一個 Repo Project）；加入、改 stage、清除等行為由 `cockpit/tests/` 的
  整合測試（真的寫入服務）與 task 7.3 的真機冒煙驗證。新路由在 `ui_preview` 要掛到它自己的假路由上。
- [標完成後 agent 仍在工作]：Repo Project 的 task 標了 completed 後沒有目前 task，agent 若仍在 working，畫面會顯示
  「工作中・未宣告 task」，與手寫 project 的行為一致；使用者清除標記即恢復。

## Migration Plan

- 升級：v2 狀態檔在第一次被接受的操作時寫成 v3，無需手動處理。
- 回滾：裝回舊版前刪除或改名 `cockpit.state.json`（會失去進度與 Repo Project）。
