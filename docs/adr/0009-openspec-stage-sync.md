# ADR-0009 卡片進度取自 repo 內的 OpenSpec 檔案，自動移動與手動操作靠「偵測結果有沒有變」切換

- Status: Accepted
- Date: 2026-10-11

## Context

Repo Project 的卡片（ADR-0008）只會因人工按鈕或 agent 主動呼叫 `/api/agent/advance` 而移動。
使用者希望 Factory Floor 自動反映 AI 目前做到哪個階段，不必每階段手動按「推進」。
使用者的 AI 工作大多在有 `openspec/` 的 repo 裡跑，change 目錄與 `tasks.md` 的勾選本身就是可讀的進度訊號。

這帶來幾個要先定下來的問題：

1. 進度從哪裡來？pane 輸出、LLM、`openspec` CLI、還是 repo 內的檔案？
2. 自動移動和使用者手動放好的位置衝突時，聽誰的？
3. 「這張卡片是自動還是手動」、「上次套用了什麼」存在哪裡？
4. 背景輪詢碰到 WSL 路徑時，怎麼不為了偵測把使用者停掉的發行版開機？

不變的前提：Cockpit 對 HERDR 完全唯讀、不新增任何 HERDR method（ADR-0001）；
`cockpit-core` 不做 IO、不依賴其他 `cockpit-*`（ADR-0003）；
標記 Completed／Failed 一律由人來貼（change `progress-model`），自動移動不改標記。

## Decision

細節見 change `openspec-stage-sync` 的 `design.md`（D2～D6、D10），這裡只記決策與理由。

### 進度來源是 repo 內的檔案，由背景工作每 10 秒輪詢（design D5、D6）

- 背景工作 `OpenSpecSync` 對每個 Repo Project pane 所在的 worktree，取目前分支（`cockpit-git` 新增的
  sealed 查詢 `CurrentBranch`，`git symbolic-ref -q HEAD`，自行去掉 `refs/heads/`），再直接讀
  `openspec/changes/` 與 `tasks.md`，判定 change 與 OpenSpec 階段：
  - 對應 change：分支名最後一段等於進行中的 change 名；否則 archive 下有 `YYYY-MM-DD-<該段>`；
    否則進行中的 change 恰好一個；否則無結果。
  - 階段：沒有勾選是規劃、部分勾選是實作、全部勾選是審查、已 archive 是完成。
- 偵測全程唯讀，只讀 git 的分支與 repo 內的小檔案（`tasks.md` 上限 1 MiB）。
  讀不了、超限、非 UTF-8、分支查詢出錯，一律視為「本輪無法判斷」：不移動卡片、不更新已保存的同步狀態。
- 每個 Stage 可設定對應一個 OpenSpec 階段（定義上的 `phases`，與 `stages` 逐項對齊，同一階段最多對應一站），
  偵測結果決定卡片該去哪一站。

### 自動與手動的優先權：偵測結果與「上次套用的」不同才動卡片（design D3）

- 每張 Task 另存 `TaskSync { mode: Auto | Manual, applied: Option<Observation> }`。
  `Observation` 是進度指紋：change 名稱、階段、`checked`、`total`。
- 純函式 `apply_openspec` 在寫入鎖內執行，規則依序判斷：
  1. 沒有偵測結果，不動。
  2. 卡片帶 Completed／Failed 標記，不動，且不更新 `applied`。清除標記後，下一輪因結果與 `applied` 不同而套用。
  3. 偵測結果等於 `applied`，不動。**手動優先就是靠這一條維持**。
  4. 其他情況：`applied` 設為偵測結果、`mode` 設為 `Auto`，卡片移到對應的 Stage。
     找不到對應的 Stage 或已在該站，就只更新 `applied` 與 `mode`。這一步可以跨站，不經 `apply_op`。
- 人工推進、退回，以及 agent 的兩個推進入口成功後，在**同一個寫入閉包內**把該 Task 標為 `Manual`、`applied` 不變（尚無同步狀態時，`applied` 取該 pane 當下的偵測結果，可為無），
  只落檔一次。分兩次寫入會讓偵測的同步在兩次之間搶先把卡片拉回去。
  Domain 層分不出呼叫者，所以「手動」的判定放在 `ProgressService` 各入口，與既有的 `declare_active` 一致。
- **偵測結果改變才恢復自動**：`applied` 沒變，所以偵測結果再變一次（勾選數變了、階段變了、change 變了）就落入第 4 條，
  卡片被拉回對應站、`mode` 轉回 `Auto`。清除標記後也一樣。
- **階段對應被修改，或標記被清除，或 pane 歸類改變時，服務層對所有展開中的 Repo Project task 全表重套**
  （`reapply_openspec_all`，以記憶體裡最新的偵測結果呼叫 `apply_openspec`）。規則 3 保證重套是冪等的，
  所以背景工作內容不變不送出時，「清除標記後，若進度與標記前套用的不同，就會依最新進度移動；相同則卡片不動」「改對應後 Auto 卡片移動」仍會發生。
  修改階段對應時，只有真的有階段換了擁有者（經 `from` 對應比較）才把 `Auto` task 的 `applied` 清成 `None`；
  `Manual` 的 task 不動；只改名稱、改名但階段跟著走、只重排都不觸發。

### 同步狀態獨立存放，`openspec_obs` 是不持久化的最新偵測結果（design D2）

- `DomainState.repo_sync` 持久化每張 Task 的 `TaskSync`；`DomainState.openspec_obs` 不持久化，是最新一輪
  各 pane 的偵測結果，用於投影顯示與重套，寫法比照 `pane_repos`。`TaskProgress` 一個欄位都不加：
  它有散布在程式與測試各處的 struct literal，`apply_op` 與 `drop_untouched_initial` 又依賴它的相等比較。
- **`TaskSync` 一定伴隨進度項目**：建立同步狀態時，沒有進度項目的 Task 先補一筆初始進度；
  有 `TaskSync` 的 Task 不算「未動過」，不會被撤回。狀態檔把 `sync` 放在 task 底下，進度項目消失會連同 `sync`
  一起消失，手動優先在重啟後就失效了。
- **`openspec_obs` 不變式**：只含目前展開中的 Repo Project task 的 pane，且每筆都屬於該 pane **目前**的歸類
  （repo、worktree、根目錄）。背景工作每筆結果附上偵測當下的歸類，寫入鎖內與 Domain 現況比對，不符就當無結果；
  `set_pane_repos` 在歸類改變時先丟棄該 pane 的結果；改變展開範圍的寫入路徑在重套前先 retain。
  否則舊 worktree 或舊 repo 的結果會被套到新卡片並落檔。
  背景工作的去重依據是 Domain 現有的 `openspec_obs`，不是自己記的「上次送出」，否則歸類 A→B→A 會讓 Domain
  丟掉結果而送出端認為已送過，造成靜默且不自癒的遺失。
- 清除規則與 `repo_progress` 一致：pane 消失時一起清。

### 狀態檔升為 v4，舊檔載入時一次性補預設對應（design D4）

- `version: 4`。`repo_projects.<id>` 多一個必填的 `phases`（與 `stages` 對齊，值為四個階段字串或 `null`，非 `null` 不重複），
  Repo Project 的 task 多一個選填的 `sync: { mode, applied }`。版本檢查接受 1 到 4。
- 讀版本 1 到 3 的檔案時，站名完全等於預設名（規劃／Plan、實作／Implement、審查／Review、完成／Complete）的 Stage
  補上對應，補完就是明確資料；v4 檔裡的 `null` 一律尊重，不再依名稱推導。
  「依當下的站名推預設」正是會在改名、重排時漂移的來源，所以只做一次。
- 版本 1 到 3 出現 `phases` 或 `sync`、v4 缺 `phases` 或長度不符、手寫 project 的 task 帶 `sync`，都視為損毀，啟動失敗。
- 升版而非「v3 加選填欄位」：舊版程式讀到不認得的欄位會回 Parse 錯誤，讀到 v4 則回「不支援的版本」，訊息較清楚。
  兩種寫法對降版的結果相同，都是拒絕啟動，延續 ADR-0008 的 v2→v3 先例。

### WSL 防護與殘餘風險（design D5、D10-6、Risks）

- 沿用 `RepoResolver` 的 `RunningDistros` 防護：worktree 的 pane 所屬 runtime 全都未連線，或根目錄在
  `\\wsl.localhost\<distro>\`（或 `\\wsl$\`）而該發行版沒在執行，這個 worktree 本輪完全不查詢，沿用上一輪結果。
  同一個 worktree 混合已連線與未連線 runtime 的 pane 時照常查詢，未連線的 pane 沿用上一輪。
- 每個 WSL worktree 在 git 查詢與讀檔**前一刻**重新探測（`wsl.exe --list --running`，這個指令不會啟動發行版）；
  探測逾時或失敗時，本輪其餘 WSL worktree 一律跳過，一輪最多只等一次探測上限。
- 每個 worktree 的讀檔最多等 5 秒，逾時該 worktree 本輪無結果；前一次讀檔未返回前不再派新的，
  避免卡在 9P 的 blocking 執行緒堆積。行程結束時 runtime 最多再等 3 秒就放手，卡住的執行緒不拖住關機。
- **殘餘風險（已知、接受）**：探測與查詢之間仍有毫秒級的時間窗。發行版剛好在探測之後、查詢之前被停止，
  `wsl.exe -d` 會把它開機；經 `\\wsl.localhost` 讀檔同樣會喚醒發行版。完全消除需要不經 `wsl.exe -d` 與 UNC 的機制，
  不在本 change 範圍。`RepoResolver` 有同性質、時間窗更長的既有風險。

## 被否決的做法

- **解析 pane 輸出判斷階段**：違反設計文件 §3、§17、§21，輸出格式因 agent 而異，脆弱。
- **用 LLM 判斷階段**：同上，且需要 token 與外部呼叫，結果不可重現。
- **呼叫 `openspec` CLI**：使用者機器不保證有 `openspec`（WSL 端更不一定），而且每個 worktree 每輪多一支子程序。
  判定只需要目錄名與 checkbox 數，直接讀檔就夠。
- **監聽檔案系統事件取代輪詢**：`\\wsl.localhost` 上的檔案監聽不可靠（`ReadDirectoryChangesW` 對 9P 共享不保證送事件）。
  輪詢成本很低：每個 worktree 一次 `git symbolic-ref` 加讀幾個小檔，OpenSpec 階段變化以分鐘計，10 秒內反映已足夠。
- **對 HERDR 寫入（送訊息讓 AI 接受建議、停止 agent 重來）**：違反 ADR-0001。按鈕仍只改 Cockpit 自己的狀態，
  「按鈕直接對 AI 下指令」留待之後另開 change 與 ADR。
- **每次手動操作就停用自動，直到使用者手動恢復**：使用者選定「下一次變化即恢復」。
- **以時間判斷手動是否過期**：不可測，語意也與使用者選定的不同。
- **把同步狀態塞進 `TaskProgress`**：改動面太大（見上）。
- **把 `stages` 改成結構體陣列、或以站名為 key 的 map 存階段對應**：前後端與投影都吃 `Vec<String>`；
  以站名為 key 改名時要多一道 remap，也是漂移來源。改為與 `stages` 對齊的平行陣列，編輯時每列帶自己的 `phase`。
- **archive 時自動貼 Completed 標記**：標記一律由人貼（change `progress-model`）。

## Consequences

- **降版限制**：v0.1.5 以前的版本讀到 v4 狀態檔會拒絕啟動。裝回舊版前要先刪除或改名 `cockpit.state.json`
  （會失去進度與已加入的 Repo Project）。升級方向不受影響，v1 到 v3 檔照常讀取，並在下次寫入內容有變時升為 v4。
- **對上 change 的準確度靠分支名**：分支名最後一段等於 change 名最準。分支名對不上又有多個進行中的 change，卡片沒有同步標示、
  維持手動。主 worktree 停在 `main` 且只有一個進行中的 change 時，會誤對上它（可能其實是別的 worktree 在做），
  影響只是卡片位置與標示，可手動覆蓋，下次偵測結果改變又會跟上。
- **自動移動會把使用者剛放好的卡片拉走**：只在偵測結果真的變化時發生，這是使用者選定的語意；
  卡片標示自動或手動，讓使用者看得出原因。
- **約 10 秒反映**：勾選變化最多約 10 秒後才出現在卡片上。
- **Windows 與 WSL 看到同一個資料夾仍是兩個 repo**（ADR-0008），各自偵測。
- 對沒用到這個功能的 Repo Project（沒有對應的 Stage、沒有 `openspec/`、對不上 change）行為完全不變。
- 對 HERDR 仍完全唯讀：偵測只讀 git 與 repo 內的檔案，不新增任何 HERDR method。
