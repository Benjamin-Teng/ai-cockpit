# file-review（delta）

## MODIFIED Requirements

### Requirement: 檔案分頁

系統必須把中欄下半部做成分頁區（`cockpit-dashboard`「版面與窄視窗」）：第一個分頁固定為 Live Output、不可關閉；其後每個
打開的檔案一個分頁，以及 `git-review` 定義的 diff、Git Graph、某版本檔案分頁（各自的身分、標題與內容見該 capability），
全部依開啟順序排列、共用下列規則。檔案分頁顯示 icon、檔名與關閉按鈕，完整相對路徑與根目錄名稱放在 `title`。檔案分頁以
runtime、`root_id` 與相對路徑識別：開啟已打開的檔案時切換到既有分頁，不新增（其他種類的分頁同理，以各自的身分判斷）。
同一時間恰有一個目前分頁；檔案並排顯示時，目前分頁為焦點欄的分頁，並排中的選定、開檔與關閉規則見「檔案並排」。沒有並排
顯示時，關閉目前分頁改為顯示其右側的分頁，沒有右側時顯示左側的分頁。分頁列為 `role="tablist"`，可用方向鍵在分頁間移動、
Enter／Space 選定；關閉按鈕可用鍵盤操作。分頁過多時分頁列在內部橫向捲動，頁面不得出現橫向捲軸。分頁不屬於任何 Project：
切換 Project 不關閉、不新增檔案分頁，也不改變並排組合；切到有可選 pane 的 Project 時，會因選定該 pane
（`cockpit-dashboard`「Project 切換」）而使分頁區切到 Live Output，選回原本的檔案分頁時內容與並排照舊。每個檔案分頁的內容區上方有一列工具列，顯示相對路徑、最後一次成功讀取的時間與
「在 VS Code 開啟」（見該需求）。整頁重畫不得改變分頁區的內容、捲動位置與並排狀態。

#### Scenario: 開檔新增分頁

- **GIVEN** 下半部只有 Live Output 分頁
- **WHEN** 在檔案樹點 `README.md`
- **THEN** 出現 `README.md` 分頁並成為目前分頁，Live Output 分頁仍在第一個

#### Scenario: 重複開啟不新增

- **GIVEN** 已打開 `README.md` 與 `docs/a.md` 兩個分頁，目前為 `docs/a.md`
- **WHEN** 再點檔案樹的 `README.md`
- **THEN** 分頁數不變，目前分頁改為 `README.md`

#### Scenario: 關閉目前分頁

- **GIVEN** 分頁依序為 Live Output、`a.md`、`b.md`、`c.md`，目前為 `b.md`，沒有並排
- **WHEN** 關閉 `b.md`
- **THEN** 目前分頁為 `c.md`

#### Scenario: 切換 Project 不影響分頁

- **GIVEN** 已打開 `README.md` 分頁，另一個 Project 沒有可選的 pane
- **WHEN** 點左欄 Project 分頁中的另一個 Project
- **THEN** `README.md` 分頁仍在、仍為目前分頁

#### Scenario: 切換 Project 不影響並排

- **GIVEN** 已打開 `README.md` 與 `docs/a.md` 並排，焦點欄為 `docs/a.md`，另一個 Project 沒有可選的 pane
- **WHEN** 點左欄 Project 分頁中的另一個 Project
- **THEN** 兩個分頁仍在並排，焦點欄仍為 `docs/a.md`

#### Scenario: 切到有可選 pane 的 Project

- **GIVEN** 已打開 `README.md` 分頁且為目前分頁，另一個 Project 有可選的 pane
- **WHEN** 點左欄 Project 分頁中的該 Project，再點 `README.md` 分頁
- **THEN** 點 Project 後分頁區切到 Live Output，`README.md` 分頁仍在、沒有新增或關閉任何檔案分頁；點 `README.md` 分頁後內容照舊

#### Scenario: 切到有可選 pane 的 Project 不改變並排

- **GIVEN** 已打開 `README.md` 與 `docs/a.md` 並排，焦點欄為 `docs/a.md`，另一個 Project 有可選的 pane
- **WHEN** 點左欄 Project 分頁中的該 Project，再選回 `docs/a.md` 分頁
- **THEN** 點 Project 後分頁區切到 Live Output，兩個檔案分頁仍在；選回後仍為兩欄並排，焦點欄為 `docs/a.md`，兩欄內容與捲動位置照舊

#### Scenario: 整頁重畫不影響並排

- **GIVEN** `README.md` 與 `docs/a.md` 並排，焦點欄為 `docs/a.md`，兩欄都已往下捲到中段
- **WHEN** 投影每 100 ms 推送一份新的 version，持續 3 秒
- **THEN** 仍為兩欄並排、焦點欄仍為 `docs/a.md`，兩欄捲動位置不變，面板的 DOM 節點沒有被換掉
