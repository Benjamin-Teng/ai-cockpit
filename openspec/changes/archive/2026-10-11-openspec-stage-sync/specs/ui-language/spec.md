# ui-language（delta）

## ADDED Requirements

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
