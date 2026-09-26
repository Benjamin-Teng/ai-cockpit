# live-output（delta）

## MODIFIED Requirements

### Requirement: 選定一個 pane

系統必須讓使用者在畫面上選定至多一個 pane 作為 Live Output 的對象：runtime 卡中每個未 exited 的 pane 列可點選；
Factory Floor 中 `binding` 為 `bound` 的 workstream 列首顯示「看輸出」，按下即選定它綁定的那個 pane。
被選定的 pane 列必須有可辨識的標示；再選另一個 pane 即取代原選取。Live Output 面板常駐於版面中（見
`cockpit-dashboard`「版面與窄視窗」）：沒有選取時顯示空狀態，提示使用者選一個 pane；有選取時顯示該 pane 的
runtime 與 pane id 與輸出；面板上有「取消選取」可取消選取，取消後回到空狀態。選取只存在於
該瀏覽器頁面（不送到服務、不持久化，重新整理後清空），整頁重畫不得清除選取。改綁模式期間 pane 列不可點選
（不呈現可點選的樣式，點列的任何區域都不改變選取），列上的「綁定到這裡」仍是送出覆蓋；既有的選取與面板在改綁模式
期間保留，離開改綁模式後 pane 列恢復可點選。Live Output 面板在版面中佔有自己的區域，不得覆蓋頁面上其餘的內容；
有選取時，頁面上其餘的操作（選另一個 pane、「畫面操作」的所有按鈕、Project 切換）必須仍可直接操作。pane 列必須能以
鍵盤聚焦並以 Enter 或 Space 選定；整頁重畫不得讓鍵盤焦點離開原本聚焦的那個 pane 列（見 `cockpit-dashboard`「畫面整頁重畫」）。

#### Scenario: 點 pane 列

- **GIVEN** 畫面有 runtime `win` 的 pane `w1:p1`，目前沒有選取
- **WHEN** 點該 pane 列
- **THEN** 該列出現選定標示，Live Output 面板由空狀態改為標題顯示 `win` 與 `w1:p1`，頁面開始請求該 pane 的輸出

#### Scenario: 沒有選取時顯示空狀態

- **GIVEN** 目前沒有選取
- **WHEN** 開啟 `/` 並等待首份投影畫完
- **THEN** Live Output 面板可見，顯示提示選一個 pane 的空狀態，沒有「取消選取」，頁面沒有發出輸出請求

#### Scenario: 從 workstream 選

- **GIVEN** workstream `backend` 的 `binding` 為 `bound`，指向 `win` 的 `w1:p2`
- **WHEN** 按 `backend` 列首的「看輸出」
- **THEN** 選定 `win` 的 `w1:p2`

#### Scenario: 未綁定的 workstream 沒有入口

- **GIVEN** workstream `tests` 的 `binding` 為 `unbound`
- **WHEN** 重畫
- **THEN** `tests` 列首沒有「看輸出」

#### Scenario: 選取跨重畫保留

- **GIVEN** 已選定 `w1:p1`
- **WHEN** 期間收到兩份新投影並整頁重畫
- **THEN** `w1:p1` 列仍有選定標示，面板內容與捲動位置不變

#### Scenario: 改綁模式期間不改變選取

- **GIVEN** 已選定 `w1:p1` 且面板顯示其輸出，之後按某 workstream 的「改綁」進入改綁模式
- **WHEN** 先點 `w1:p2` 列上「綁定到這裡」以外的區域，再按該列的「綁定到這裡」
- **THEN** 第一次點擊不改變選取；第二次點擊送出覆蓋請求；全程選取仍是 `w1:p1`、面板仍顯示其輸出

#### Scenario: 面板打開時仍可操作頁面下方的內容

- **GIVEN** 視窗 1536×1024，已選定一個 pane 且面板顯示其輸出
- **WHEN** 對每個可選的 pane 列、每個「畫面操作」按鈕與每個 Project 項目，取其可見範圍中心點做命中測試
- **THEN** 命中的都是該元素本身（或其子元素），不是 Live Output 面板；點另一個 pane 列即改為選取該 pane

#### Scenario: 鍵盤選定

- **GIVEN** 焦點在一個未 exited 的 pane 列上
- **WHEN** 按 Enter
- **THEN** 選定該 pane，面板顯示其 runtime 與 pane id

#### Scenario: 鍵盤焦點跨重畫保留

- **GIVEN** 焦點在未 exited 的 pane 列 `w1:p2` 上
- **WHEN** 期間收到新投影並整頁重畫（該列的 DOM 節點被換成新的），之後按 Enter
- **THEN** 重畫後焦點仍在 `w1:p2` 列上；按 Enter 選定 `w1:p2`，面板顯示其 runtime 與 pane id

#### Scenario: 取消選取

- **GIVEN** 已選定 `w1:p1`
- **WHEN** 按面板的「取消選取」
- **THEN** 面板回到空狀態、選定標示消失，頁面不再請求輸出
