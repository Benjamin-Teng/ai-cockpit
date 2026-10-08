# pipeline-config Specification

## Purpose

定義 `cockpit.toml` 中描述 Project 結構（stages、workstreams 與其 binding、tasks）與狀態檔位置的區段、
預設值與驗證規則；結構只來自設定檔，畫面與寫入 API 不能改結構。證據：設計文件 §8.2、§12。

## Requirements

### Requirement: Project 區段結構

系統必須解析零到多個 `[[project]]`，每筆含：`id`（必填）、`name`（選填，預設等於 `id`）、`stages`
（必填、非空字串陣列，陣列順序即 Stage 的線性順序，字串同時是 Stage 的識別與顯示名稱）、
`[[project.workstream]]`（零到多筆）、`[[project.task]]`（零到多筆）。每筆 workstream 含 `id`（必填）、
`name`（選填，預設 `id`）、`binding`（選填表）。`binding` 含 `runtime`（必填）、`workspace`（必填，
workspace 標籤）、`pane_label`、`cwd`、`agent`（三者選填）。每筆 task 含 `id`（必填）、`title`（選填，
預設 `id`）、`workstream`（必填）、`stage`（選填，預設 `stages` 第一個）、`depends_on`（選填，同一
Project 內 task `id` 的陣列，預設空）。project、workstream、task 在投影與畫面上依設定檔中的順序排列。
這些區段與欄位之外的未知欄位視為錯誤。

#### Scenario: 最小 Project

- **GIVEN** 設定檔含一筆 `[[project]]`，`id = "p"`、`stages = ["Spec", "Build"]`，一筆 workstream
  `id = "be"`（無 binding），一筆 task `id = "t1"`、`workstream = "be"`
- **WHEN** 載入設定
- **THEN** 得到 Project `p`，名稱為 `p`；task `t1` 的起始 stage 為 `Spec`、`depends_on` 為空；workstream
  `be` 沒有 binding

#### Scenario: 完整 binding

- **GIVEN** workstream 的 `binding = { runtime = "wsl", workspace = "ai-cockpit", pane_label = "backend", cwd = "worktrees/backend", agent = "claude" }`
- **WHEN** 載入設定
- **THEN** 該 workstream 的 binding 五個欄位皆為所給值

#### Scenario: 沒有 project 區段

- **GIVEN** 設定檔只有 `[server]` 與 `[[runtime]]`
- **WHEN** 載入設定
- **THEN** 載入成功、Project 清單為空

#### Scenario: 未知欄位

- **GIVEN** 某筆 task 多了 `owner = "x"`
- **WHEN** 載入設定
- **THEN** 啟動失敗，訊息含 `owner`

### Requirement: Project 區段驗證

系統必須在下列情況啟動失敗，並以「`project.<project id>`」開頭的路徑指出位置與原因（workstream 的問題
為 `project.<pid>.workstream.<wid>.<欄位>`、task 的問題為 `project.<pid>.task.<tid>.<欄位>`；該筆
`id` 本身缺漏或非法時改以序號識別，例如 `project.<pid>.task[2]`）：project、workstream、task 的 `id`
不符合 `^[A-Za-z0-9_-]{1,64}$`；project `id` 在設定檔內重複；workstream 或 task 的 `id` 在同一 Project
內重複；`stages` 為空、含空字串或重複；task 的 `workstream` 不存在於同一 Project；task 的 `stage` 不在
`stages`；`depends_on` 指向同一 Project 不存在的 task、指向自己、或依賴關係形成環；binding 的 `runtime`
不是設定檔中任何一筆 `[[runtime]]` 的 `id`；binding 的 `workspace` 為空字串；`pane_label`、`cwd`、`agent`
給了但為空字串。

#### Scenario: task 指向不存在的 workstream

- **GIVEN** Project `p` 的 task `t1` 寫 `workstream = "fe"`，但 `p` 沒有 workstream `fe`
- **WHEN** 載入設定
- **THEN** 啟動失敗，訊息含 `project.p.task.t1.workstream` 與 `fe`

#### Scenario: 依賴成環

- **GIVEN** task `a` 的 `depends_on = ["b"]`、task `b` 的 `depends_on = ["a"]`
- **WHEN** 載入設定
- **THEN** 啟動失敗，訊息含 `depends_on` 以及 `a` 與 `b`

#### Scenario: binding 指向未設定的 runtime

- **GIVEN** 設定檔的 runtime 只有 `win`，某 workstream `binding.runtime = "wsl"`
- **WHEN** 載入設定
- **THEN** 啟動失敗，訊息含 `project.p.workstream.be.binding.runtime` 與 `wsl`

#### Scenario: id 含非法字元

- **GIVEN** 某 task `id = "a/b"`
- **WHEN** 載入設定
- **THEN** 啟動失敗，訊息含 `a/b`

#### Scenario: stage 重複

- **GIVEN** `stages = ["Spec", "Spec"]`
- **WHEN** 載入設定
- **THEN** 啟動失敗，訊息含 `project.p.stages` 與 `Spec`

### Requirement: 狀態檔位置

系統必須解析選填的 `[state] path`：給相對路徑時相對於設定檔所在目錄解析，給絕對路徑時照用；未給時為
設定檔所在目錄下的 `cockpit.state.json`。設定來源沒有檔案（零設定模式）時，狀態檔位於
`%LOCALAPPDATA%\ai-cockpit\cockpit.state.json`，該資料夾在第一次寫入時建立；環境變數 `LOCALAPPDATA` 不存在時沒有狀態檔，
狀態只存在記憶體並記 warn。設定來源是程式內嵌的設定（測試用）時沒有狀態檔，狀態同樣只存在記憶體。有狀態檔路徑時，系統
在啟動時讀取它（不論 Project 清單是否為空，因為檔案可能含畫面加入的 Repo Project，見 `repo-projects`）；只有被接受的
操作才寫入狀態檔，沒有任何被接受的操作時不建立檔案。`[state] path` 為空字串時啟動失敗，訊息含 `state.path`。

#### Scenario: 預設位置

- **GIVEN** 以 `--config D:/work/cockpit.toml` 載入、沒有 `[state]`、有一筆 project
- **WHEN** 決定狀態檔位置
- **THEN** 為 `D:/work/cockpit.state.json`

#### Scenario: 相對路徑相對於設定檔

- **GIVEN** 以 `--config D:/work/cockpit.toml` 載入、`[state] path = "state/progress.json"`
- **WHEN** 決定狀態檔位置
- **THEN** 為 `D:/work/state/progress.json`，不受工作目錄影響

#### Scenario: 零設定模式的位置

- **GIVEN** 沒有設定檔（零設定模式），`LOCALAPPDATA` 為 `C:\Users\u\AppData\Local`，該路徑下沒有 `ai-cockpit` 資料夾
- **WHEN** 系統啟動，之後加入一個 Repo Project
- **THEN** 啟動時不建立任何檔案或資料夾；加入被接受後建立 `C:\Users\u\AppData\Local\ai-cockpit\cockpit.state.json`，
  其中含該 Repo Project

#### Scenario: 零設定模式重啟後保留

- **GIVEN** 零設定模式下已加入 Repo Project 並產生進度
- **WHEN** 停止並重新啟動系統
- **THEN** 啟動時從 `%LOCALAPPDATA%\ai-cockpit\cockpit.state.json` 讀回 Repo Project 與進度

#### Scenario: 沒有 LOCALAPPDATA

- **GIVEN** 零設定模式，環境變數 `LOCALAPPDATA` 不存在
- **WHEN** 系統啟動並加入一個 Repo Project
- **THEN** 加入成功（回 201）並記 warn，不建立任何狀態檔；重啟後該 Repo Project 不在

#### Scenario: 內嵌設定沒有狀態檔

- **GIVEN** 以程式內嵌的設定啟動（測試用）
- **WHEN** 加入 Repo Project 或對手寫 project 做被接受的操作
- **THEN** 操作成功，不建立任何狀態檔

#### Scenario: 沒有 project 就不碰狀態檔

- **GIVEN** 設定檔沒有 `[[project]]`，設定檔目錄下沒有狀態檔
- **WHEN** 系統啟動並運作，期間沒有任何被接受的操作
- **THEN** 不建立任何狀態檔

#### Scenario: 零設定模式只開啟不操作

- **GIVEN** 沒有設定檔（零設定模式），`LOCALAPPDATA` 下沒有 `ai-cockpit` 資料夾，HERDR 有數個 pane 位於 git repo 內
- **WHEN** 系統啟動並運作，pane 進出、cwd 改變，`detected_repos` 隨之變動，但使用者沒有加入任何 Repo Project
- **THEN** 不建立任何檔案或資料夾

#### Scenario: 既有 v2 狀態檔不因開啟新版而被改寫

- **GIVEN** 設定檔旁的狀態檔為 `version: 2`
- **WHEN** 啟動並運作，沒有任何被接受的操作
- **THEN** 狀態檔仍是 `version: 2`，內容不變

#### Scenario: 沒有 project 仍讀取既有狀態檔

- **GIVEN** 設定檔沒有 `[[project]]`，設定檔目錄下的狀態檔含 Repo Project `app`
- **WHEN** 系統啟動
- **THEN** 讀回 `app`，投影的 `projects` 含它
