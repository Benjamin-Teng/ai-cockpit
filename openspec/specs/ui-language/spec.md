# ui-language Specification

## Purpose

定義儀表板介面語言（繁體中文或英文）的決定與切換、介面文字由同一份字典提供的涵蓋範圍、後端訊息以代碼加參數交給前端翻譯的契約，
以及桌面啟動器訊息框依 Windows 顯示語言選擇語言。決策背景見 change `ui-language` 的 proposal 與 design。

## Requirements

### Requirement: 介面語言的決定

系統必須在儀表板任何介面文字繪製之前決定介面語言，結果只有繁體中文（`zh`）與英文（`en`）兩種：`localStorage` 的
`cockpit.lang` 為 `zh` 或 `en` 時照它；否則瀏覽器第一順位語言（`navigator.languages[0]`，沒有時用 `navigator.language`）
不分大小寫符合 `zh`、`zh-Hant`、`zh-Hans`，或其後再接 `-TW`、`-HK`、`-MO`、`-CN` 之一時為 `zh`；否則時區（`Intl` 解析結果）
是 `Asia/Taipei`、`Asia/Hong_Kong`、`Asia/Macau`、`Asia/Macao`、`Asia/Shanghai`、`Asia/Chongqing`、`Asia/Chungking`、
`Asia/Harbin`、`Asia/Urumqi`、`Asia/Kashgar`、`PRC`、`ROC`、`Hongkong` 之一時為 `zh`；其餘為 `en`。`localStorage` 或 `Intl`
無法使用時視為沒有該項資訊。`<html>` 的 `lang` 屬性必須設為 `zh-Hant` 或 `en`。

#### Scenario: 台灣使用者預設繁中

- **GIVEN** 沒有 `cockpit.lang`，瀏覽器語言為 `en-US`，時區為 `Asia/Taipei`
- **WHEN** 開啟儀表板
- **THEN** 介面為繁體中文，`<html lang="zh-Hant">`

#### Scenario: 其他地區預設英文

- **GIVEN** 沒有 `cockpit.lang`，瀏覽器語言為 `ja-JP`，時區為 `Asia/Tokyo`
- **WHEN** 開啟儀表板
- **THEN** 介面為英文，`<html lang="en">`

#### Scenario: 只看第一順位語言

- **GIVEN** 沒有 `cockpit.lang`，瀏覽器語言依序為 `en-US`、`zh-TW`，時區為 `America/New_York`
- **WHEN** 開啟儀表板
- **THEN** 介面為英文

#### Scenario: 手動選擇優先

- **GIVEN** `cockpit.lang` 為 `en`，時區為 `Asia/Taipei`
- **WHEN** 開啟儀表板
- **THEN** 介面為英文

### Requirement: 語言切換按鈕

頂列必須在通知設定（鈴鐺）按鈕旁提供語言切換按鈕：目前為 `zh` 時按鈕文字為 `EN`、`lang="en"`、無障礙名稱為
`Switch to English`；目前為 `en` 時按鈕文字為 `中文`、`lang="zh-Hant"`、無障礙名稱為 `切換為繁體中文`。按下後必須把另一種語言
寫入 `cockpit.lang` 並重新載入頁面。其他開著同一個 Cockpit 的視窗收到 `cockpit.lang` 的 `storage` 事件時，必須同樣重新載入。
重新載入後開著的檔案與 git 分頁、通知設定照既有機制還原。

#### Scenario: 切換為英文

- **GIVEN** 介面為繁中
- **WHEN** 按下語言切換按鈕
- **THEN** `cockpit.lang` 變為 `en`，頁面重新載入後介面為英文，按鈕文字為 `中文`

#### Scenario: 其他視窗同步

- **GIVEN** 兩個視窗都開著儀表板，介面為英文
- **WHEN** 在其中一個視窗按下語言切換按鈕
- **THEN** 另一個視窗也重新載入並顯示繁中

### Requirement: 介面文字涵蓋範圍

儀表板的介面文字（`index.html` 的靜態文字與屬性、各腳本產生的文字、`title`、`aria-label`、`placeholder`、錯誤與空狀態訊息、
通知設定面板、桌面通知的標題與內文、`document.title`）都必須依介面語言顯示，並由同一份字典提供；繁中與英文兩份字典必須有完全
相同的鍵，同一個鍵兩種語言的具名佔位符集合必須相同。下列內容兩種語言都照原文顯示、不翻譯：使用者資料（project、stage、task、
workstream 名稱、檔案與 pane 內容、git 資料）、HERDR 與 git 回傳的原文、HERDR 的 agent 狀態值、task 狀態值、連線狀態值、事件
`kind` 與 HERDR 產生的 `detail`、`AI Agent Cockpit`、`Factory Floor`、`Live Output`、`Git Graph`、`Completed`／`Failed` 按鈕。介面為英文時，
畫面上不得出現任何繁中字典的字串。描述 HERDR `done` 的文字不得暗示 task 已完成：繁中不得使用「完成」，英文不得使用 `complete`、
`completed`、`finished`（`task completed` 這個通知種類名稱除外）。

#### Scenario: 英文介面沒有中文字典字串

- **GIVEN** 介面為英文，預覽模式的範例資料
- **WHEN** 依序開啟 Factory Floor、Live Output、檔案樹與各種檢視器、變更與 Git Graph、通知設定面板、錯誤 banner
- **THEN** 畫面文字與 `title`／`aria-label` 都不含任何繁中字典的字串；範例資料裡的中文 task 名稱照原樣顯示

#### Scenario: 字典鍵與佔位符一致

- **WHEN** 比對繁中與英文字典
- **THEN** 兩者鍵集合相同，每個鍵的具名佔位符集合相同

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

### Requirement: 桌面啟動器訊息框語言

桌面啟動器的訊息框必須依 Windows 使用者介面語言（`GetUserDefaultUILanguage`）選擇語言：主要語言為中文，且語言代碼為
繁體中文（台灣、香港、澳門或未指定地區的繁體）、簡體中文（中國或未指定地區的簡體）時用繁中，其餘一律英文。訊息框標題維持
`AI Agent Cockpit`。附在訊息後的記錄檔路徑與記錄檔內容照原文。

#### Scenario: 英文 Windows

- **GIVEN** Windows 顯示語言為英文（美國）
- **WHEN** 監聽埠被其他程式占用，啟動器顯示錯誤
- **THEN** 訊息框內文為英文

#### Scenario: 繁中 Windows

- **GIVEN** Windows 顯示語言為中文（台灣）
- **WHEN** 同上
- **THEN** 訊息框內文為繁中，與改動前相同

### Requirement: OpenSpec 同步相關介面文字

系統必須讓 OpenSpec 進度同步新增的介面文字依「介面文字涵蓋範圍」由同一份字典提供，繁中與英文兩份字典鍵集合相同、同鍵的具名佔位符集合相同：
卡片同步標示的「自動」「手動」文字（英文為 `Auto`、`Manual`）、「編輯 stage」對話框每列階段下拉的標籤與選項「不對應」「規劃」「實作」「審查」
「完成」（英文為 `None`、`Plan`、`Implement`、`Review`、`Complete`）、這些元素的 `title` 與 `aria-label`，以及階段對應在別處被改過而不送出時的錯誤
訊息。下列內容兩種語言都照原文顯示、不翻譯：change 名稱（使用者資料）與 `checked/total` 數字。階段對應不合法的錯誤使用既有代碼 `invalid_stages`
與 `invalid_body`（見「後端訊息代碼」），不新增代碼。介面為英文時，這些元素不得出現繁中字典的字串。

#### Scenario: 英文介面沒有繁中字串

- **GIVEN** 介面為英文，預覽模式的範例資料含已同步（自動與手動各一）的 task
- **WHEN** 檢視 Factory Floor 的同步標示並開啟「編輯 stage」對話框
- **THEN** 標示文字為 `Auto`／`Manual`，下拉選項為 `None`、`Plan`、`Implement`、`Review`、`Complete`；畫面文字與 `title`／`aria-label`
  都不含繁中字典的字串；範例資料裡的 change 名稱照原樣顯示

#### Scenario: 繁中介面

- **GIVEN** 介面為繁中
- **WHEN** 檢視同一組畫面
- **THEN** 標示文字為「自動」「手動」，下拉選項為「不對應」「規劃」「實作」「審查」「完成」

#### Scenario: 字典鍵與佔位符一致

- **WHEN** 比對繁中與英文字典
- **THEN** 新增的同步相關鍵在兩者都存在，每個鍵的具名佔位符集合相同
