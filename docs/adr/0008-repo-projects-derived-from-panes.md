# ADR-0008 Repo Project 的工作線由 pane 推導，定義與進度存進狀態檔

- Status: Accepted
- Date: 2026-10-08

## Context

手寫 `cockpit.toml` 的 `[[project]]` 對一般使用者太難：要知道 workspace 標籤、pane 標籤這些
一字不差的字串，改完還要關窗重開。`repo-projects` change 改成「Project 就是一個 git repo」，
在畫面上按「加入」即可，之後該 repo 裡的每個 pane 自動成為一條工作線。

這帶來三個要先定下來的問題：

1. 工作線要放在哪裡、怎麼隨 pane 進出？pane id 不該當設定鍵，同一個資料夾也可能開兩個 pane。
2. 畫面加入的 Project 定義與進度存在哪裡？
3. 怎麼判斷「兩個 pane 屬於同一個 repo」，包含 linked worktree？

另有兩條不變的前提：Cockpit 對 HERDR 完全唯讀、不新增任何 HERDR method（ADR-0001）；
`cockpit-core` 不做 IO、不依賴其他 `cockpit-*`（ADR-0003）。

## Decision

細節見 change `repo-projects` 的 `design.md`（D1、D3、D5），這裡只記決策與理由。

### 工作線在執行期由 pane 推導，並在寫入服務內展開成一般 `ProjectDef`（design D3）

- Domain 另存三份資料：Repo Project 定義（`repo_projects`）、它們的 task 進度（`repo_progress`，
  與手寫 project 的 `progress` 分開存放）、pane 所屬 repo 的對應（`pane_repos`，不持久化）。
- 寫入服務在每次定義或對應變動時，把每個 Repo Project 展開成一個一般的 `ProjectDef`：
  repo 相符的每個未 exited pane 各一條 workstream、一張 task，id 都是 `<runtime id>~<pane id>`，
  綁定是「固定 pane」綁定。展開結果放進 `DomainState.projects`，所以投影、進度操作、agent 端點的
  既有程式不必分辨兩種 Project。
- 名稱與排序在投影時從當下的 pane 資料取（pane label，空的用 agent 名稱，再空用 pane id），
  label 改了不必經過寫入服務。
- pane 從 HERDR 消失時，那條工作線與它的進度一起清除；清除檢查在寫入鎖內、依 `RuntimeStore`
  當下的連線狀態與 pane 樹判斷，runtime 未連線或尚未沉降時一律保留。已 exited 但仍在 pane 樹中的 pane，
  其工作線不再展開（從畫面消失），進度則保留到 pane 真正關閉才清。

### 定義與進度存進狀態檔 v3，不另開檔案（design D5）

`cockpit.state.json` 升為 `version: 3`，加 `repo_projects` 區段。定義與進度同在一個檔案，
才能一次原子寫入。零設定模式也有狀態檔：`%LOCALAPPDATA%\ai-cockpit\cockpit.state.json`。

### repo 身分用 git 的共同目錄（design D1）

對 pane 的 cwd 執行 `git rev-parse --path-format=absolute --git-common-dir --git-dir --show-toplevel`
（`cockpit-git` 新增的一個 sealed 查詢），以共同 `.git` 目錄的主機路徑為 repo key。
同一個 repo 的主 worktree 與所有 linked worktree 共用同一個共同目錄，所以歸為同一個 repo。
查詢由 `cockpit` 的背景工作 `RepoResolver` 執行，結果經寫入服務進 Domain。

## 被否決的做法

- **把偵測到的 pane 寫成固定的 workstream（設定檔式）**：同資料夾兩個 pane 會歧義，
  pane id 也不該當設定鍵。
- **在前端依 cwd 分組**：agent 回報要在後端找得到 workstream，前端分組做不到。
- **投影時才即時推導 workstream**：投影、進度操作、agent 端點都要各自加一套「這個 task 存在嗎」
  的判斷，分岔點多。
- **另開 `cockpit.projects.json` 存定義**：兩份檔案無法一起原子寫入，移除 project 時定義與進度
  可能對不上。
- **沿用 `cockpit-files` 的 `find_root` 判斷 repo**：它把 linked worktree 當成自己的根，
  同一個 repo 的 worktree 會被拆開。
- **直接讀 `.git` 檔案的 `gitdir:` 與 `commondir`**：不用啟動 git，但要自己處理相對路徑、
  `GIT_DIR` 類設定，以及 Windows 經 `\\wsl.localhost` 看 WSL 符號連結會失敗的問題。

## Consequences

- **降版限制**：v0.1.3 以前的版本讀到 v3 狀態檔會拒絕啟動。裝回舊版前要先刪除或改名
  `cockpit.state.json`（會失去進度與 Repo Project）。升級方向不受影響，v1、v2 檔照常讀取，
  並在下次寫入內容有變時升為 v3。
- **cwd 反映有延遲**：pane 的 cwd 改變沒有事件通知，靠定期重新抓取 snapshot
  （預設 `resnapshot_secs = 30`），最多約 30 秒後才反映；git 查詢結果另有快取。
- **Windows 與 WSL 看到同一個資料夾會是兩個 repo**：repo key 一個是 Windows 路徑、
  一個是 `\\wsl.localhost\<distro>\...`，不做跨環境合併。
- **一條工作線只有一張 task**，pane 關掉不保留紀錄，同一個 worktree 重開 pane 不會接回舊進度。
- **手寫 `[[project]]` 不變**，與 Repo Project 並列；id 撞名時手寫的優先，被隱藏的 Repo Project
  只能經 `/api/repo-projects/{pid}` 改名或移除。
- 寫入服務改為永遠建立，成為更多操作的單一入口；只有序列化內容改變時才寫檔，
  所以 pane 進出、cwd 改變只改記憶體，不會落檔。
- 對 HERDR 仍完全唯讀：pane 的 cwd 來自既有 snapshot，repo 身分經 `cockpit-git` 的允許清單查詢。
