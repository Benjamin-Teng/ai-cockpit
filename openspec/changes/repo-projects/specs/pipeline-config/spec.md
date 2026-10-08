# pipeline-config（delta）

## MODIFIED Requirements

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
