# ui-language（delta）

## MODIFIED Requirements

### Requirement: 後端訊息代碼

前端原文顯示的後端訊息必須同時提供穩定的代碼與參數，讓前端依介面語言翻譯；原本的繁中文字欄位保留不變。錯誤本體為
`{"error": <繁中文字>, "code": <代碼>, "params": {<名稱>: <字串>}}`（沒有參數時 `params` 可省略），適用於進度寫入、綁定覆蓋、
agent 端點、Live Output 端點與 Repo Project 管理端點（`POST /api/repo-projects`、`PATCH`／`DELETE /api/repo-projects/<pid>`，
見 `repo-projects`）的錯誤（例外：綁定覆蓋本體不是合法 JSON、Live Output 路徑不是合法 UTF-8 這兩個 400 只有 `error`，
儀表板不會送出這兩種請求）。Repo Project 管理端點的每個錯誤代碼（`repo_not_detected`、`repo_already_added`、`unknown_project`、
`not_repo_project`、`invalid_body`、`invalid_name`、`invalid_stages`、`persist_failed`）以及固定 pane 工作線的 `not_overridable`、
免帶 id 推進的 `no_task_for_pane`、`ambiguous_task` 在繁中與英文字典都必須有對應訊息。投影中的連線原因、HERDR protocol 警告、project 警告
（含 Repo Project 與手寫 project 的 id 撞名警告，代碼 `repo_project_id_conflict`、參數為該 id），以及內容是 Cockpit 自己訊息的事件 `detail`（例如 drift 的原因），必須在原文欄位旁提供
`{"code": <代碼>, "params": {…}}` 形式的對應欄位；HERDR 或作業系統回傳、無法歸類的原文以代碼 `raw` 搭配參數 `text` 表示。
介面為英文時，前端遇到字典裡有的代碼以字典範本加參數顯示，遇到沒有的代碼或欄位不存在時顯示原文；介面為繁中時一律顯示原文
（原文即繁中，字典的繁中範本與原文相同，原文另可能帶範本沒有的細節）。

#### Scenario: 進度被拒以代碼翻譯

- **GIVEN** 介面為英文，task 已在最後一個 stage
- **WHEN** 按下推進
- **THEN** 回應本體的 `code` 為 `already_last_stage`、`error` 仍為「已是最後一個 Stage」，錯誤 banner 以英文顯示原因

#### Scenario: WSL 未啟動的連線原因

- **GIVEN** 介面為英文，WSL 發行版 `Ubuntu-24.04` 未啟動
- **WHEN** 檢視 `wsl` runtime 卡片
- **THEN** 原因以英文顯示並包含 `Ubuntu-24.04`；投影的原文欄位仍為「WSL 發行版 Ubuntu-24.04 未啟動」

#### Scenario: 未知代碼退回原文

- **GIVEN** 投影的連線原因代碼為字典沒有的值
- **WHEN** 畫面繪製該 runtime 卡片
- **THEN** 顯示原文欄位的文字，不顯示代碼本身

#### Scenario: Repo Project 管理端點的錯誤以代碼翻譯

- **GIVEN** 介面為英文，加入請求被拒絕，回應本體 `code` 為 `invalid_name`、`error` 為繁中原因
- **WHEN** 錯誤 banner 顯示
- **THEN** 以字典的英文訊息顯示；回應本體的 `error` 仍為繁中；介面為繁中時顯示 `error` 原文

#### Scenario: id 撞名警告以代碼翻譯

- **GIVEN** 介面為英文，手寫 project `app` 的 `warnings` 有一則撞名警告，對應欄位為 `{"code":"repo_project_id_conflict","params":{"id":"app"}}`
- **WHEN** 畫面繪製該 Project 的警告
- **THEN** 以字典的英文範本加參數 `app` 顯示；介面為繁中時顯示原文欄位
