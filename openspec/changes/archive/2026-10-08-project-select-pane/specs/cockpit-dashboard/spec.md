# cockpit-dashboard（delta）

## MODIFIED Requirements

### Requirement: Project 切換

系統必須在左欄依 `projects` 順序列出每個 Project：顯示 `name`、`warnings` 數量（有時）與各 task `status` 的數量
（以 `status` 字串標示，不使用「完成」字樣）。使用者點選或以鍵盤（Enter 或 Space）選定某個 Project 後，Factory Floor
改為顯示該 Project；目前選定的項目必須有可辨識的標示。未選定過時預設選定第一個 Project；選定的 Project 已不在最新投影中時
改為選定第一個。選取只存在於該瀏覽器頁面（不送到服務、不持久化，重新整理後回到預設），整頁重畫不得清除選取。選定 Project
不是「畫面操作」：不得清除最近一次操作的錯誤訊息、不得影響進行中的寫入請求的結果呈現、不得離開改綁模式。投影沒有任何
Project 時，左欄與 Factory Floor 區域顯示沒有 Project 的空狀態，頁面其他部分正常；Factory Floor 的空狀態文字指向左欄
「偵測到的 repo」區（說明在那裡加入 repo 即可看到 Factory Floor），不得出現需要重啟 Cockpit 的字樣。

使用者選定 Project 時（點選、鍵盤，或加入成功後畫面自動選定新 Project），系統必須同時選定該 Project 的一個 pane，效果等同按該
工作線的「看輸出」（檔案、變更與 Live Output 改為該 pane，分頁區切到 Live Output）。挑選規則依序為：①已綁定（`binding.state`
為 `bound`）且綁定的 agent 狀態為 working 的工作線，依畫面順序取第一條；②否則第一條已綁定的工作線；③都沒有就不改變目前
選定的 pane。因此而選定 pane 時，若右欄 runtime 卡片清單本身是有界的捲動區（三欄且高度足夠的版面），系統必須只在該區內捲動，使
該 pane 的列落在可見位置；整頁捲動的版面（窄視窗或高度不足）不捲動，任何情況都不得捲動整個頁面。頁面載入時的預設選定 Project（未選定過時
選第一個、選定的 Project 離開投影時改選第一個）不得自動選定 pane，避免一打開頁面就被切到 Live Output。改綁模式中選定 Project 也不得自動選定 pane（改綁模式下
pane 列用於指定改綁目標，不是選定 pane）。選定 pane 不反過來切換 Project。

左欄 Project 分頁在 Project 清單之外必須有「偵測到的 repo」區，依投影 `detected_repos` 的順序列出每個 repo：顯示 `name`、
`pane_count` 與「加入」鈕；沒有偵測到的 repo 時該區不列任何項目、沒有「加入」鈕。按「加入」送出 `POST /api/repo-projects`，
本體為 `{"repo": <該項的 repo>, "stages": <預設 stages>}`（不帶 `name`）；預設 stages 依介面語言：繁中為 `規劃`、`實作`、
`審查`、`完成`，英文為 `Plan`、`Implement`、`Review`、`Complete`。

`kind` 為 `repo` 的 Project 項目必須有「⋯」選單（手寫的 Project 沒有），選單項目為「改名」「編輯 stage」「移除」：

- 「改名」讓使用者輸入新名稱，送出 `PATCH /api/repo-projects/<pid>`，本體 `{"name": <新名稱>}`。
- 「編輯 stage」開啟對話框，依序列出該 Project 目前的 stages，使用者可改名、新增、刪除、調整順序；按儲存送出
  `PATCH /api/repo-projects/<pid>`，本體 `{"stages": [{"name": <名稱>, "from": <原名稱 或 null>}, ...]}`，順序即對話框中的
  順序，原有項目的 `from` 為它原本的名稱，新增項目的 `from` 為 `null`；按取消不送出請求。
- 「移除」先顯示含 Project 名稱的確認，確認後送出 `DELETE /api/repo-projects/<pid>`；取消不送出請求。

加入、改名、編輯 stage、移除都是「畫面操作」：失敗（回非 2xx 或請求失敗）時依「畫面操作」顯示錯誤訊息（含回應本體的 `error`，
介面為英文時依 `code` 顯示英文），成功後畫面不自行修改狀態，一律等 `/ws` 推送的新投影重畫。加入成功（回 201 與新 Project 的 `id`）後，畫面在含該 `id` 的投影到達時自動選定這個新 Project（只選取一次，之後
使用者可自由切換；投影到達前使用者已手動選定其他 Project 時不自動切換）。同一個 repo 的加入請求進行中時，該 repo 的「加入」
鈕呈現忙碌且不再送出請求，直到請求失敗或該 repo 離開偵測區（避免連點造成第二筆請求以「已加入」失敗）。對話框與確認在開啟期間收到新投影並
整頁重畫時必須保留（含已輸入的內容與焦點）。所有名稱（Project、repo、stage、workstream）一律以文字節點呈現，不以 HTML 插入。

#### Scenario: 切換 Project

- **GIVEN** 投影有 Project `p1`、`p2`
- **WHEN** 開啟 `/`，之後點左欄的 `p2`
- **THEN** 開啟時左欄 `p1` 在上、`p2` 在下，`p1` 有選定標示、Factory Floor 顯示 `p1`；點選後 `p2` 有選定標示、
  Factory Floor 顯示 `p2`

#### Scenario: 選取跨重畫保留

- **GIVEN** 已選定 `p2`
- **WHEN** 期間收到兩份新投影並整頁重畫
- **THEN** 仍選定 `p2`、Factory Floor 仍顯示 `p2`

#### Scenario: 鍵盤切換與焦點保留

- **GIVEN** 焦點在左欄 `p2` 項目上，投影每 100 ms 推送一份新的 version
- **WHEN** 經過 2 秒後按 Enter
- **THEN** 這段期間焦點始終在 `p2` 項目上，按 Enter 後 Factory Floor 顯示 `p2`

#### Scenario: 切換 Project 不清除錯誤也不離開改綁模式

- **GIVEN** 頁面正顯示一則操作錯誤訊息，且已按 `p1` 某 workstream 的「改綁」進入改綁模式
- **WHEN** 點左欄的 `p2`
- **THEN** 錯誤訊息仍在，改綁提示與各 pane 列的「綁定到這裡」仍在；選定的 pane 不變（即使 `p2` 有已綁定的工作線）

#### Scenario: 選定 Project 時優先選 working 的已綁定工作線

- **GIVEN** `p2` 有三條工作線依序為：已綁定且 agent 為 idle 的 `w1`、已綁定且 agent 為 working 的 `w2`、未綁定的 `w3`
- **WHEN** 點左欄的 `p2`
- **THEN** 選定的 pane 為 `w2` 綁定的 pane，檔案、變更與 Live Output 顯示該 pane，分頁區切到 Live Output

#### Scenario: 沒有 working 時選第一條已綁定工作線

- **GIVEN** `p2` 有三條工作線依序為：未綁定的 `w1`、已綁定且 agent 為 idle 的 `w2`、已綁定且 agent 為 blocked 的 `w3`
- **WHEN** 以鍵盤（Enter）選定左欄的 `p2`
- **THEN** 選定的 pane 為 `w2` 綁定的 pane

#### Scenario: 全部未綁定時選定的 pane 不變

- **GIVEN** 目前選定的 pane 為 `pX`，`p2` 的工作線全部未綁定
- **WHEN** 點左欄的 `p2`
- **THEN** Factory Floor 顯示 `p2`，選定的 pane 仍為 `pX`，分頁區的目前分頁不變

#### Scenario: 加入後自動選定新 Project 也選其 pane

- **GIVEN** 已選定 Project `h`，`detected_repos` 有 repo `app`，`app` 有一條已綁定的工作線
- **WHEN** 按 `app` 的「加入」，服務回 201，含 `app` 的新投影到達
- **THEN** 左欄選定 `app`，並選定該已綁定工作線的 pane（等同按「看輸出」）

#### Scenario: 頁面載入不自動選 pane

- **GIVEN** 第一個 Project `p1` 有已綁定的工作線
- **WHEN** 開啟 `/`
- **THEN** 預設選定 `p1`，但沒有因此選定 pane，分頁區的目前分頁維持載入時的狀態

#### Scenario: 右欄捲動到選定的 pane

- **GIVEN** 右欄 runtime 卡片很長，`p2` 的已綁定 pane 列在可視範圍之外
- **WHEN** 點左欄的 `p2`
- **THEN** 右欄捲動，使該 pane 的列落在可見位置

#### Scenario: 整頁捲動的版面不捲動頁面

- **GIVEN** 視窗寬 900、高 800（版面為整頁捲動），頁面捲在最上方
- **WHEN** 點左欄有已綁定工作線的 Project
- **THEN** 該 pane 被選定，頁面捲動位置不變

#### Scenario: 各狀態數量

- **GIVEN** `p1` 有 2 個 `running`、1 個 `completed`、1 個 `failed` 的 task，並有 1 則 warning
- **WHEN** 重畫
- **THEN** 左欄 `p1` 項目顯示 `running` 2、`completed` 1、`failed` 1 與 1 則 warning，不出現「完成」字樣

#### Scenario: 沒有 Project

- **GIVEN** 投影的 `projects` 為空
- **WHEN** 重畫
- **THEN** 左欄與 Factory Floor 區域顯示沒有 Project 的空狀態，runtime 卡與 Live Output 正常

#### Scenario: 空狀態指向偵測到的 repo

- **GIVEN** 投影的 `projects` 為空，`detected_repos` 有 repo `app`
- **WHEN** 重畫（繁中與英文各一次）
- **THEN** Factory Floor 的空狀態文字指向「偵測到的 repo」區，不含「需要重啟」（英文為 `Restart`）字樣；左欄「偵測到的 repo」區列出 `app`
  與「加入」鈕

#### Scenario: 列出偵測到的 repo

- **GIVEN** 投影的 `detected_repos` 為 `app`（`pane_count` 2）與 `lib`（`pane_count` 1），`projects` 有手寫的 `h`
- **WHEN** 重畫
- **THEN** 左欄 Project 分頁在 `h` 之外有「偵測到的 repo」區，依序列出 `app`（含數量 2）與 `lib`，各有「加入」鈕

#### Scenario: 按加入送出預設 stages

- **GIVEN** 介面為繁中，`detected_repos` 有 repo `app`（repo key `d:\work\app\.git`）
- **WHEN** 按 `app` 的「加入」
- **THEN** 服務收到 `POST /api/repo-projects`，本體 `{"repo":"d:\\work\\app\\.git","stages":["規劃","實作","審查","完成"]}`
  （沒有 `name`）；新投影到達後 `app` 從該區消失並出現在 Project 清單。介面為英文時 `stages` 為 `["Plan","Implement","Review","Complete"]`

#### Scenario: 加入成功後自動選定新 Project

- **GIVEN** 已選定 Project `h`，`detected_repos` 有 repo `app`
- **WHEN** 按 `app` 的「加入」，服務回 201 `{"id":"app"}`，稍後含 `app` 的新投影到達
- **THEN** 左欄選定 `app`，Factory Floor 顯示 `app`；之後點回 `h` 不會再被自動切走

#### Scenario: 投影到達前手動切換則不自動選定

- **GIVEN** 已選定 Project `h`，`detected_repos` 有 repo `app`
- **WHEN** 按 `app` 的「加入」，服務回 201 `{"id":"app"}`；含 `app` 的投影到達前，使用者點選 Project `h2`
- **THEN** 投影到達後仍選定 `h2`

#### Scenario: 連點加入只送出一次

- **GIVEN** `detected_repos` 有 repo `app`
- **WHEN** 快速連按兩次 `app` 的「加入」
- **THEN** 只送出一筆 `POST /api/repo-projects`，不顯示錯誤；請求進行中該鈕呈現忙碌

#### Scenario: 加入被拒絕時顯示原因

- **GIVEN** 服務對加入請求回 409，`code` 為 `repo_already_added`
- **WHEN** 該操作完成
- **THEN** 頁面顯示含原因的錯誤訊息（英文介面以字典的英文訊息），之後的重畫不清除它

#### Scenario: 只有 Repo Project 有選單

- **GIVEN** 投影有手寫的 `h`（`kind` 為 `config`）與 Repo Project `app`（`kind` 為 `repo`）
- **WHEN** 重畫
- **THEN** `app` 項目有「⋯」選單，選單含「改名」「編輯 stage」「移除」；`h` 項目沒有

#### Scenario: 改名

- **WHEN** 從 `app` 的「⋯」選單選「改名」，輸入 `App 前端` 並送出
- **THEN** 服務收到 `PATCH /api/repo-projects/app`，本體 `{"name":"App 前端"}`；新投影到達後左欄與 Factory Floor 標題顯示 `App 前端`

#### Scenario: 編輯 stage

- **GIVEN** `app` 的 stages 為 `Plan`、`Implement`、`Review`、`Done`
- **WHEN** 開啟「編輯 stage」對話框，把 `Implement` 改名為 `Build`、刪除 `Review`、在最後新增 `Ship`，按儲存
- **THEN** 服務收到 `PATCH /api/repo-projects/app`，本體 `{"stages":[{"name":"Plan","from":"Plan"},{"name":"Build","from":"Implement"},{"name":"Done","from":"Done"},{"name":"Ship","from":null}]}`

#### Scenario: 取消編輯與取消移除不送請求

- **WHEN** 開啟「編輯 stage」對話框後按取消；再從選單選「移除」，在確認中按取消
- **THEN** 服務沒有收到任何 `PATCH` 或 `DELETE` 請求

#### Scenario: 移除要先確認

- **WHEN** 從 `app` 的「⋯」選單選「移除」
- **THEN** 先顯示含 `app` 名稱的確認，尚未送出請求；確認後服務收到 `DELETE /api/repo-projects/app`，新投影到達後 `app` 從
  Project 清單消失；若它原是選定的 Project，改為選定第一個

#### Scenario: 對話框跨重畫保留

- **GIVEN** 「編輯 stage」對話框開著，已改了一個 stage 名稱但尚未儲存
- **WHEN** 期間收到兩份新投影並整頁重畫
- **THEN** 對話框仍在，已輸入的內容與焦點保留

#### Scenario: 名稱不以 HTML 解讀

- **GIVEN** 某 Repo Project 的名稱為 `<img src=x onerror=alert(1)>`，其 stage 名稱與 pane label 也含 HTML 字元
- **WHEN** 重畫
- **THEN** 這些名稱在左欄、Factory Floor 與對話框中以原樣文字顯示，沒有建立任何 `<img>` 元素，也沒有執行指令碼
