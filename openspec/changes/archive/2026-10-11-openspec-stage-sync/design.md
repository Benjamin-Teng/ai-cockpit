# openspec-stage-sync 設計

## Context

動機見 `proposal.md`「Why」。現況與限制：

- Repo Project 的工作線由 pane 推導，每個 pane 一張 task，id 為 `<runtime>~<pane>`（ADR-0008）。
  - 定義是 `RepoProjectDef { id, name, repo, stages: Vec<String> }`。
  - 進度是 `TaskProgress { stage, mark }`，存在 `DomainState.repo_progress`。
- 進度寫入只有一個入口 `ProgressService::transact`：持寫入鎖、在 clone 上計算，序列化內容有變才落檔。
  人工與 agent 推進最後都走 `DomainState::apply_progress`，Domain 層分不出是誰推進的。
- `apply_op` 假設 `progress.stage` 一定在 `stages` 內，否則 panic。
- 狀態檔各層 struct 都有 `deny_unknown_fields`。版本檢查只認 1、2、3。
- 背景偵測已有先例 `RepoResolver`：
  - 每 60 秒或投影變動時跑一輪，以 trait sink 把結果送進 `ProgressService`。
  - runtime 未連線或 WSL 發行版沒在跑時，沿用上一輪結果，不查（避免把 WSL 開機）。
  - `PaneRepo` 目前沒保留 worktree 根目錄。
- `cockpit-git` 以 sealed `GitQuery` 封閉查詢清單。目前取分支只有偏重的 `Status`、`Refs`。

## Goals / Non-Goals

### Goals

- 偵測與自動移動全程唯讀：只讀 git 與 repo 內檔案，不寫 repo、不送 HERDR method。
- 自動與手動的優先權規則可以寫成 `cockpit-core` 的純函式狀態轉移，不依賴時間、不做 IO，可單元測試。
- 沒用到這個功能的 Repo Project 行為完全不變：沒有對應、沒有 `openspec/`、對不上 change。

### Non-Goals

設計層補充，範圍見 proposal「非目標」。

- 不監聽檔案系統事件，改用輪詢。理由見 D5。
- 不呼叫 `openspec` CLI。理由見 D6。
- 不支援同一個 Stage 對應多個 OpenSpec 階段。

## Decisions

### D1 每站階段存成與 `stages` 對齊的平行陣列

做法：`RepoProjectDef` 加 `phases: Vec<Option<OpenSpecPhase>>`，長度恆等於 `stages`。
`OpenSpecPhase` 為 `plan | implement | review | complete`。

- 驗證：長度相同；非空值互不重複（同一個階段最多對應一個 Stage）。
- 編輯 stages 時，每一列直接帶自己的 `phase`。改名、重排、刪站都由送出的整份清單決定，不必另做 remap。

否決的做法：

- 把 `stages` 改成結構體陣列：前後端與投影全都吃 `Vec<String>`，改動面太大。
- 以站名為 key 的 map：改名時要多一道 remap，而且「依名稱推預設」正是 memory
  「implicit-default-from-current-definition-drifts-on-edit」記過的漂移來源。

### D2 同步狀態放在獨立的 map，不擴充 `TaskProgress`

做法：`DomainState` 新增兩份資料。

- `repo_sync: HashMap<ProjectId, HashMap<TaskId, TaskSync>>`，持久化。
  - `TaskSync { mode: Auto | Manual, applied: Option<Observation> }`。
  - `Observation { change: String, phase: OpenSpecPhase, checked: u32, total: u32 }` 即「進度指紋」。
- `openspec_obs: HashMap<(RuntimeId, PaneId), Observation>`，不持久化，是最新一輪偵測結果，用於投影顯示。
  寫法比照 `pane_repos`。

理由：`TaskProgress` 有約 30 處 struct literal，`apply_op` 與 `drop_untouched_initial` 都依賴它的相等比較。
把同步狀態分開，進度操作規則一行不改。

- 清除規則與 `repo_progress` 一致：pane 消失時一起清。
- **同步狀態需要進度項目**：`apply_openspec` 建立或更新某 task 的 `TaskSync` 時，若該 task 沒有 `repo_progress` 項目，
  一律先補一筆初始進度（第一個 stage、`mark = none`）。沒有進度項目時，「目前 stage」視為第一個 stage。
  有 `TaskSync` 的 task 不算「未動過」：`drop_untouched_initial` 不得撤回有 `TaskSync` 的 task 的進度項目。
  理由：狀態檔把 `sync` 放在 task 項目底下，進度項目被撤回會連同 `sync` 一起消失，手動優先在重啟後失效。

### D3 自動移動的狀態轉移（純函式，在寫入鎖內執行）

`DomainState::apply_openspec(project, task, obs: Option<&Observation>)`：

1. `obs` 為 `None`（對不上 change 或偵測失敗）→ 不改任何東西。
2. task 的 `mark` 不是 `none` → 不改任何東西，`applied` 也不更新。之後清除標記時，下一輪會因 `obs != applied` 而重新套用。
3. `obs == applied` → 不改任何東西。手動優先就是靠這一條維持。
4. 其他情況：
   - `applied = obs`，`mode = Auto`。
   - 若該 task 沒有 `repo_progress` 項目，先補一筆初始進度（第一個 stage、`mark = none`），「目前 stage」即第一個 stage（見 D2）。
   - 找出對應 `obs.phase` 的 Stage。找到、通過「在 `stages` 內」的驗證、且與目前不同，就直接把 `progress.stage` 設為它。
     不經 `apply_op`，因為自動移動可以跨站。
   - 找不到對應的 Stage，或目標就是目前（含第一個）stage，就只更新 `applied` 與 `mode`，卡片不動；進度項目仍照上一點補齊。

手動標記：

- `ProgressService` 的人工 `advance`／`retreat`，以及 agent 的兩個推進入口成功後，把該 task 的 `mode` 設為 `Manual`。
  - **推進與標記必須在同一個 `write` 閉包內完成，只落檔一次**：閉包內先 `apply_progress`，成功才標 `Manual`，兩者一起提交。
    不得先推進、落檔、再另開一次寫入去標記，否則兩次寫入之間，偵測的同步可能搶先把卡片拉回去。
  - `agent_advance_for_pane` 的 task 在鎖內選出後，就地標記該 task，不在鎖外重選。
- `applied` 不變，所以偵測結果再變一次就會回到第 4 條，恢復自動。
- 若該 task 還沒有 `TaskSync`，就建立 `{ Manual, applied: 目前偵測結果 }`。理由：使用者在偵測到某個結果之後才手動，
  該結果不應再把卡片拉回去。
- `complete`／`fail`／`clear` 不改 `mode`。

Domain 層分不出呼叫者，所以「手動」的判定放在 `ProgressService` 各入口。這與現有 `declare_active` 的做法一致。

階段對應被修改時（PATCH stages），該 project 所有 `mode = Auto` 的 task 把 `applied` 清成 `None`，下一輪依新對應重新套用。
`Manual` 的 task 不動。

否決的做法：

- 每次手動操作都把自動停用，直到使用者手動恢復：使用者 2026-10-10 選了「下一次變化即恢復」。
- 以時間判斷手動是否過期：不可測，而且與使用者選定的語意不同。

### D4 狀態檔升為 v4，舊版本載入時一次性補預設對應

- `STATE_FILE_VERSION = 4`。系統寫出的狀態檔一律為 v4。版本檢查接受 1 到 4。
- `StateRepoProject` 加 `phases`：與 `stages` 對齊的陣列，值為 `"plan" | "implement" | "review" | "complete" | null`。
- `StateTask` 加選填欄位 `sync: { mode, applied }`，只有 Repo Project 的 task 會寫。
- **補預設對應只發生在讀到版本 1 到 3 的檔案時**：
  - 站名完全等於某個預設名的，補上對應：規劃／Plan→plan、實作／Implement→implement、審查／Review→review、完成／Complete→complete。
  - 補完若有重複，後出現的改成 `null`。
  - 補完就是明確資料，下次寫檔落成 v4。v4 檔裡的 `null` 一律尊重，不再推導。
- **載入時 stage 不存在而退回第一站的 task**：`mode` 為 auto 的，`applied` 一併清為 `None`（`applied` 記的是「上次套用到哪一站」，
  卡片被退回後已對不起來；留著會讓「偵測結果等於 `applied`」的比較永遠成立、卡片停在第一站不再自癒）；manual 保留不動。
- 升版而非「v3 加選填欄位」的理由：舊版程式讀到不認得的欄位會回 Parse 錯誤；讀到 v4 會回「不支援的版本」，
  對使用者的訊息比較清楚。兩種寫法對降版的結果相同，都是拒絕啟動。這延續 v2→v3 的先例（ADR-0008 Consequences）。

### D5 偵測工作 `OpenSpecSync`：10 秒輪詢，沿用 `RepoResolver` 的防護

新模組 `cockpit/src/openspec_sync.rs`，在 `app.rs` 組裝並納入 `shutdown_all`。每 10 秒跑一輪：

1. 從最新投影與 `pane_repos` 取出所有 Repo Project task 的 pane，依 worktree 根目錄分組。
   `PaneRepo` 要新增欄位 `root`（worktree 根目錄的主機路徑字串）：`RepoIdentity` 已經查了 `--show-toplevel`，只是 `classify` 把它丟了。
   - Windows：取 git `--show-toplevel` 的輸出，正斜線轉反斜線，大小寫保留（不套用 repo key 的小寫化）。
   - WSL：POSIX 路徑經既有的 `wsl_host_path` 轉為 `\\wsl.localhost\<distro>\...`；轉不出來則 `root` 為 `None`，該 pane 不偵測。
   - 這會改到 `cockpit-core` 的 `PaneRepo` 與所有 struct literal。
2. 每個 worktree 先過防護：runtime 未連線，或是 WSL 路徑但發行版沒在跑，就跳過，本輪不送任何結果。
   這直接重用 `RunningDistros`。
3. 以新的 `CurrentBranch` 查詢取分支，再掃 `openspec/changes/`（D6），得到該 worktree 每個 pane 的 `Option<Observation>`。
4. 經新 sink trait `OpenSpecSink::submit(Vec<((RuntimeId, PaneId), Option<Observation>)>)` 送進
   `ProgressService::sync_openspec`。
   - 在一次 `transact` 裡更新 `openspec_obs`，並對每個 task 跑 D3。
   - 內容與上一輪相同就不送，去重比照 `RepoResolver.last_sent`。

輪詢而非監聽檔案事件的理由：

- `\\wsl.localhost` 上的檔案監聽不可靠，`ReadDirectoryChangesW` 對 9P 共享不保證送事件。
- 輪詢每輪的成本很低：每個 worktree 一次 `git symbolic-ref` 加讀幾個小檔。

10 秒的取捨：OpenSpec 階段變化以分鐘計，10 秒內反映已足夠；WSL worktree 每 10 秒多開一次 `wsl.exe` 也可以接受。

### D6 change 對應與階段判定直接讀檔，不呼叫 `openspec` CLI

理由：

- 使用者機器上不保證有 `openspec`，WSL 端更不一定有。
- 呼叫 CLI 是每個 worktree 每輪再多一支子程序。
- 判定只需要目錄名與 checkbox 數。

規則：

- 進行中的 change：`openspec/changes/` 下的直接子目錄，`archive` 除外。
- 對應順序：
  1. 分支名最後一段（最後一個 `/` 之後）等於某個進行中 change 名 → 該 change。
  2. 否則，`openspec/changes/archive/` 下有目錄名為 `YYYY-MM-DD-<該段>` → 該 change，階段為 complete。
     同一個 slug 有多個日期的目錄時，取日期字串最大者（字典序即時間序）。
  3. 否則，進行中的 change 恰好一個 → 該 change。
  4. 否則（含 detached HEAD 且進行中不是恰好一個、沒有 `openspec/changes/`）→ `None`。
- 階段判定，`checked`／`total` 取自 `tasks.md`：
  - 不存在、`total == 0` 或 `checked == 0` → plan
  - `0 < checked < total` → implement
  - `checked == total > 0` → review
  - archive 對應 → complete，`checked`／`total` 取 archive 目錄裡的 `tasks.md`
- checkbox 認法：行首空白之後是 `-`、`*` 或 `+`，接空白、`[ ]` 或 `[x]`／`[X]`。其他寫法不計。
- `tasks.md` 以 `cockpit_files::read_capped` 讀，上限 1 MiB。超過、讀取錯誤或非 UTF-8 都視為「本輪無法判斷」，
  該 worktree 送 `None`。`openspec/changes/` 或 `archive/` 存在但讀取錯誤，同樣視為無法判斷（D10-4）。

### D7 新增 `CurrentBranch` sealed 查詢

指令為 `git symbolic-ref -q HEAD`（不用 `--short`），輸出上限小。程式自行去掉 `refs/heads/` 前綴。

- exit 0 且輸出以 `refs/heads/` 開頭 → `Some(去掉前綴的分支名)`
- exit 0 但不是 `refs/heads/` 開頭 → `None`
- exit 1 且無輸出 → detached HEAD → `None`
- 其他 → 錯誤

不用 `--short` 的理由：`--short` 會把 `refs/heads/x` 縮成 `x`，但存在同名 tag 時 git 可能縮成帶歧義的 `heads/x`，
導致拿到錯的分支名。自己去前綴沒有這個問題。

比照 `VerifyCommit`／`RepoIdentity` 的模式，補 argv 單元測試與 `real_git.rs`。

### D8 投影與 API 形狀

- `ProjectedProject` 加 `stage_phases`：與 `stages` 對齊，值為階段字串或 `null`。只有 Repo Project 有，手寫 project 為空陣列。
- `ProjectedTask` 加 `sync`：值為 `null` 或 `{ change, phase, checked, total, mode }`。
  - 取自 `openspec_obs`（當下偵測）加 `repo_sync.mode`。
  - 當下對不上 change 時為 `null`，即使 `repo_sync` 有舊紀錄也一樣，避免顯示過時的 change。
  - 有偵測結果但沒有 `TaskSync` 時（例如帶著標記），`mode` 以 `auto` 呈現。
- `POST /api/repo-projects` 的本體加選填 `phases`，與 `stages` 對齊。省略等同全部 `null`。
  前端「加入」一律帶預設四站的對應。
- `PATCH /api/repo-projects/<pid>` 的每列 stage 加選填 `phase`，省略等同 `null`。
  前端資產與後端同版內嵌，`?v=` 綁版本，不會有舊前端送出省略的本體。

### D9 畫面

- 卡片上有 `sync` 時顯示一行小標示：change 名稱、`checked/total`，以及「自動」或「手動」。手動用較淡的樣式。
- 「編輯 stage」對話框每列多一個下拉選單：不對應、規劃、實作、審查、完成。
  - 選到已被他列使用的階段時，他列改回「不對應」，維持唯一。
  - 過期檢查要涵蓋 `stage_phases`。
- 外觀依 memory「frontend-appearance-reviewed-by-frontend-design-skill」過 frontend-design 審核。

### D10 撰寫規格時補定的細節

以下各點在撰寫 delta specs 時補定，規格以此為準：

1. **錯誤碼**：
   - 本體型別不對（例如 `phases` 不是陣列）→ `invalid_body`。
   - 長度不符、值不是四個階段字串之一、非 `null` 值重複 → `invalid_stages`。
   - 未知的階段字串不可落到反序列化錯誤的 `invalid_body`：先以字串接收，再驗證。
2. **重新套用的觸發條件**：對每個 phase，比較「修改前擁有它的 stage，經本次 `from` 對應後的新名稱」與「修改後擁有它的 stage 名稱」；
   任一 phase 不同，才清掉 `Auto` task 的 `applied`。`from: null` 的新列視為新身分（不等於任何舊 stage）。
   因此只改 project 名稱、stages 改名但 phase 跟著走、或只重排，都不觸發；某 phase 換了擁有者（含改由新列擁有、或原本有擁有者而改為沒有）才觸發。
   某 phase 修改前的擁有者被刪除（沒有新名稱），修改後也沒有任何 stage 擁有它時，兩邊都沒有擁有者，視為不變。
3. **v4 載入**：
   - `repo_projects` 必須存在。
   - 每個 Repo Project 必須有 `phases`，長度等於 `stages`，非 `null` 值不重複。
   - 違反以上任一條視為損毀，啟動失敗，與既有 Repo Project 定義不合規則的處理相同。
   - 版本 1 到 3 出現 `phases` 或 `sync` 欄位，由既有的「無法解析」條款處理，啟動失敗。
     實作上 `check_version_shape` 改為 v3／v4 分流：`phases` 在 v3 缺席才合法，在 v4 必填。
   - 手寫 `projects` 底下的 task 帶 `sync` 視為損毀，啟動失敗（`sync` 只屬於 `repo_projects`）。
4. **偵測失敗**：目前分支查詢出錯，或 `openspec/changes/`、`openspec/changes/archive/` 存在但讀不了，該 worktree 本輪為 `None`。
   目錄不存在不是失敗（沒有 `openspec/changes/` 為 `None` 的正常結果；沒有 `archive/` 只是 archive 對應不成立）。
   處理方式與 `tasks.md` 讀取失敗相同。
5. **archive 目錄裡沒有 `tasks.md`**：`checked`／`total` 為 0/0，階段為 complete。
6. **被防護跳過的 worktree**：只有被跳過的 worktree 沿用上一輪的偵測結果，其他 worktree 照常送出。
   每輪送出的是「所有仍在 Repo Project 中的 pane」的完整對照表；不在表中的 pane，其 `openspec_obs` 移除。
   去重比較的是整張表。Cockpit 剛啟動、被防護跳過的 worktree 尚無上一輪結果時，其 pane 沒有偵測結果（`sync` 為 `null`），直到防護解除。
7. **手動入口建立 `TaskSync` 時，當下沒有偵測結果**：`applied = None`。之後第一次出現偵測結果會自動移動並轉回自動。
   這是刻意的：建立 change 是新資訊，等同「OpenSpec 進度有變化」。
8. **D3 第 4 條一律設 `mode = Auto`**：即使找不到對應的 Stage，或卡片已在目標 Stage，也一樣。
9. **`tasks.md` 大小**：大於 1 MiB（1,048,576 位元組）才算超限，剛好 1 MiB 可以讀。

## Risks / Trade-offs

- [分支名不含 change 名、又同時有多個進行中 change] → 卡片顯示為無同步，維持手動。這是使用者在 brainstorming 已接受的退路。
- [主 worktree 停在 `main`，只有一個進行中 change，但那個 change 其實是別的 worktree 在做] → 主 worktree 的 pane 會誤對上它。
  影響只是卡片位置與標示，可用手動覆蓋，下次變化又會跟上。README 說明「分支名對上 change 最準」。
- [偵測輪詢讓 WSL 背景多開 `wsl.exe`] → 只對發行版正在跑的 worktree 查，與 `RepoResolver` 同一道防護。
  驗收要確認發行版停止時不會被開機。
- [探測與查詢之間的時間窗] → 每個 WSL worktree 在 git 查詢前一刻重新探測（task 4.7），時間窗縮到毫秒級，但仍存在：
  發行版剛好在探測之後、查詢之前被停止，`wsl.exe -d` 會把它開機；經 `\\wsl.localhost` 讀檔同樣會喚醒發行版。
  完全消除需要不經 `wsl.exe -d` 與 UNC 的機制，超出本 change 的範圍。
  探測本身逾時或失敗時，本輪其餘 WSL worktree 不再探測、一律跳過，一輪最多只等一次探測上限，下一輪重試。
- [`\\wsl.localhost` 讀檔卡在 9P] → 每次讀檔最多等 5 秒，逾時該 worktree 本輪無結果；前一次讀檔未返回前不再派新的，
  避免 blocking 執行緒堆積。行程結束時 tokio runtime 最多再等 3 秒就放手，卡住的執行緒不拖住關機（task 4.7）。
- [自動移動把使用者剛手動放好的卡片拉走] → 只有偵測結果真的變化時才會發生，這正是使用者選定的語意。
  卡片標示自動或手動，讓使用者看得出原因。
- [降版拒絕啟動] → CHANGELOG 與 README 註明。做法沿用 v3 先例：降版前刪除或改名狀態檔。

## Migration Plan

- 升級：首次載入 v3 檔時補預設對應（D4）。下次有寫入時落成 v4。沒有任何 Repo Project 的檔案也照樣升版號。
- 回滾：v0.1.5 以前的版本讀 v4 會拒絕啟動。降版前刪除或改名 `cockpit.state.json`，會失去進度與 Repo Project。
  與 v3 先例相同。

## Open Questions

（無）
