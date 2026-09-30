# file-review（delta）

## MODIFIED Requirements

### Requirement: 左欄檔案樹

系統必須在左欄頂端提供「Project」「檔案」「變更」三個分頁（預設「Project」；「變更」分頁的內容見 `git-review`「左欄變更
分頁」），三者為同一個 `role="tablist"`，可用方向鍵移動、Enter／Space 選定。「檔案」分頁顯示目前選定 pane（`live-output`
「選定一個 pane」）的根目錄的檔案樹：頂端顯示根目錄 `name`、runtime `id` 與「重新整理」按鈕；根目錄的子項目在切到此分頁
或根目錄改變時讀取，資料夾在展開時才讀取該層；每一列顯示 icon 與名稱，完整相對路徑放在 `title`。點資料夾列（或以鍵盤
Enter／Space）切換展開與收合，列帶 `aria-expanded`；點檔案列開啟或切換到該檔案的分頁（見「檔案分頁」）。「重新整理」
重新讀取根目錄與所有已展開的資料夾。沒有選定 pane 時顯示「先在 Factory Floor 或 runtime 清單選一個 pane」；選定的 pane
沒有根目錄或查詢失敗時顯示原因。展開狀態依根目錄分別保留：換到另一個根目錄再換回來時，原本展開的資料夾仍展開。整頁
重畫（`cockpit-dashboard`「畫面整頁重畫」）不得改變檔案樹的內容、展開狀態、內部捲動位置與鍵盤焦點。`omitted` 或 `skipped`
大於 0 時，在該資料夾末端顯示「還有 N 項未顯示」。

#### Scenario: 左欄三個分頁

- **WHEN** 載入頁面
- **THEN** 左欄頂端依序為「Project」「檔案」「變更」三個分頁，目前為「Project」；在分頁上按右方向鍵兩次再按 Enter，目前分頁為「變更」

#### Scenario: 切到檔案分頁

- **GIVEN** 已選定 pane `w1:p1`，其根目錄為 `repo`
- **WHEN** 點左欄的「檔案」分頁
- **THEN** 顯示 `repo` 的檔案樹，第一層為根目錄的子項目

#### Scenario: 沒有選定 pane

- **GIVEN** 沒有選定任何 pane，也沒有打開任何檔案分頁
- **WHEN** 切到「檔案」分頁
- **THEN** 顯示提示選一個 pane 的空狀態，頁面沒有發出任何檔案端點的請求

#### Scenario: 展開狀態跨根目錄保留

- **GIVEN** 選定 pane `w1:p1`（根目錄 `repo`）並展開了 `src`
- **WHEN** 改選根目錄不同的 pane `w2:p1`，再改選回 `w1:p1`
- **THEN** `repo` 的檔案樹中 `src` 仍為展開

#### Scenario: 重畫不影響檔案樹

- **GIVEN** 檔案樹已展開 `src` 並往下捲動，焦點在某個檔案列上，投影每 100 ms 推送一份新的 version
- **WHEN** 經過 3 秒
- **THEN** 檔案樹的 DOM 節點沒有被換掉，展開狀態、捲動位置與焦點都不變

### Requirement: 檔案分頁

系統必須把中欄下半部做成分頁區（`cockpit-dashboard`「版面與窄視窗」）：第一個分頁固定為 Live Output、不可關閉；其後每個
打開的檔案一個分頁，以及 `git-review` 定義的 diff、Git Graph、某版本檔案分頁（各自的身分、標題與內容見該 capability），
全部依開啟順序排列、共用下列規則。檔案分頁顯示 icon、檔名與關閉按鈕，完整相對路徑與根目錄名稱放在 `title`。檔案分頁以
runtime、`root_id` 與相對路徑識別：開啟已打開的檔案時切換到既有分頁，不新增（其他種類的分頁同理，以各自的身分判斷）。
關閉目前分頁時，改為顯示其右側的分頁，沒有右側時顯示左側的分頁。分頁列為 `role="tablist"`，可用方向鍵在分頁間移動、
Enter／Space 選定；關閉按鈕可用鍵盤操作。分頁過多時分頁列在內部橫向捲動，頁面不得出現橫向捲軸。分頁不屬於任何 Project：
切換 Project 不改變已打開的分頁。每個檔案分頁的內容區上方有一列工具列，顯示相對路徑、最後一次成功讀取的時間與「在 VS Code
開啟」（見該需求）。整頁重畫不得改變分頁區的內容與捲動位置。

#### Scenario: 開檔新增分頁

- **GIVEN** 下半部只有 Live Output 分頁
- **WHEN** 在檔案樹點 `README.md`
- **THEN** 出現 `README.md` 分頁並成為目前分頁，Live Output 分頁仍在第一個

#### Scenario: 重複開啟不新增

- **GIVEN** 已打開 `README.md` 與 `docs/a.md` 兩個分頁，目前為 `docs/a.md`
- **WHEN** 再點檔案樹的 `README.md`
- **THEN** 分頁數不變，目前分頁改為 `README.md`

#### Scenario: 關閉目前分頁

- **GIVEN** 分頁依序為 Live Output、`a.md`、`b.md`、`c.md`，目前為 `b.md`
- **WHEN** 關閉 `b.md`
- **THEN** 目前分頁為 `c.md`

#### Scenario: 切換 Project 不影響分頁

- **GIVEN** 已打開 `README.md` 分頁
- **WHEN** 點左欄 Project 分頁中的另一個 Project
- **THEN** `README.md` 分頁仍在、仍為目前分頁

### Requirement: 分頁還原

系統必須把已打開的分頁（依分頁順序；檔案分頁存 runtime、`root_id`、相對路徑、根目錄名稱，`git-review` 定義的分頁存其身分
所需的欄位與根目錄名稱）、目前分頁與左欄目前分頁（「Project」「檔案」「變更」之一）存到瀏覽器本機儲存，並在載入頁面時還原。
本 change 之前的儲存格式（只有檔案分頁）必須照常還原為檔案分頁。還原的分頁所屬根目錄不可用時，分頁仍保留並顯示「這個根目錄目前沒有任何 pane，無法讀取」，依自動更新的
節奏重試，恢復後正常顯示。瀏覽器本機儲存不可用或內容損毀時，以沒有已打開分頁的狀態開始，頁面其餘功能正常。Live Output 的
pane 選取不還原（沿用 `live-output`「選定一個 pane」）。

#### Scenario: 重新整理後還原

- **GIVEN** 已打開 `README.md` 與 `docs/a.md`，目前分頁為 `docs/a.md`，左欄目前為「檔案」
- **WHEN** 重新整理頁面
- **THEN** 兩個分頁依原順序還原，目前分頁為 `docs/a.md` 且顯示其內容，左欄為「檔案」分頁

#### Scenario: 儲存內容損毀

- **GIVEN** 瀏覽器本機儲存中對應的值不是合法 JSON
- **WHEN** 載入頁面
- **THEN** 下半部只有 Live Output 分頁，頁面其餘部分正常，console 有警告

#### Scenario: 還原 git 分頁與變更分頁

- **GIVEN** 已打開 `README.md`、`src/a.rs` 的 diff 分頁（變更）與 Git Graph 分頁，目前分頁為 Git Graph，左欄目前為「變更」
- **WHEN** 重新整理頁面
- **THEN** 三個分頁依原順序還原，目前分頁為 Git Graph 且顯示其內容，左欄為「變更」分頁

#### Scenario: 舊格式照常還原

- **GIVEN** 瀏覽器本機儲存中是本 change 之前的格式，記錄了 `README.md` 與 `docs/a.md` 兩個檔案分頁
- **WHEN** 載入頁面
- **THEN** 兩個檔案分頁依原順序還原並顯示內容
