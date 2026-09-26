# live-output Specification

## Purpose

定義 Live Output：把使用者選定的那個 pane 的畫面輸出以純文字顯示在 Cockpit。涵蓋讀取端點（路徑、回應、
錯誤對應、本機同源檢查）與畫面面板的行為（選取、輪詢、失敗呈現、常駐於版面中的顯示）。Live Output 是
HERDR pane 輸出的投影，不是 terminal，不接受輸入。證據：`docs/cockpit-spec.md` §12 與 Scenario E、
`CONTEXT.md`「Live Output」、`docs/research/2026-09-19/pane-read-probe.md`。

## Requirements

### Requirement: 輸出讀取端點

系統必須提供 `GET /api/runtimes/<runtime>/panes/<pane>/output`：每次請求即時向該 runtime 讀一次該 pane 的
輸出（最多最近 200 行），不快取、不在請求之間保留任何與選取有關的狀態。成功回 200，本體為 JSON 物件，
欄位為 `runtime`、`pane_id`（與路徑相同）、`format`（目前固定為 `"text"`）、`text`（純文字，不含終端機
控制序列）、`truncated`（布林，表示還有更早的輸出未回傳）。這個端點的所有回應（含 400、403、404、405、503、504）都必須帶
`Cache-Control: no-store` 與 `X-Content-Type-Options: nosniff`。路徑中的 `<runtime>` 或 `<pane>` 片段解碼後不是合法 UTF-8 時回 400，且不得對 runtime 發出讀取；
`<runtime>` 不是設定中的 runtime id、或該 pane 不存在時回 404；
runtime 無法連線或讀取失敗時回 503；單次讀取超過 5 秒回 504。所有非 200 回應本體沿用 `{"error": "<原因>"}`。
端點只接受 `GET`，不改變任何狀態；其他 method（含 `HEAD`）一律回 405，且不得對 runtime 發出讀取。

#### Scenario: 讀到輸出

- **GIVEN** runtime `win` 的 pane `w1:p1` 目前畫面有三行文字
- **WHEN** `GET /api/runtimes/win/panes/w1:p1/output`
- **THEN** 回 200，`format` 為 `text`，`text` 為那三行，`truncated` 為 `false`，回應帶 `Cache-Control: no-store`
  與 `X-Content-Type-Options: nosniff`

#### Scenario: 還有更早的輸出

- **GIVEN** 該 pane 的輸出超過 200 行
- **WHEN** 請求輸出
- **THEN** 回 200，`text` 為最後 200 行，`truncated` 為 `true`

#### Scenario: 不認識的 runtime

- **WHEN** `GET /api/runtimes/nope/panes/w1:p1/output`
- **THEN** 回 404，本體 `error` 指出 runtime 不存在，且沒有對任何 runtime 發出讀取

#### Scenario: pane 不存在

- **GIVEN** runtime `win` 沒有 pane `w1:p99`
- **WHEN** 請求其輸出
- **THEN** 回 404，本體 `error` 指出 pane 不存在

#### Scenario: runtime 斷線

- **GIVEN** runtime `wsl` 目前連不上
- **WHEN** 請求其任一 pane 的輸出
- **THEN** 回 503，本體 `error` 含原因

#### Scenario: 讀取逾時

- **GIVEN** runtime 對讀取遲遲不回應
- **WHEN** 請求輸出
- **THEN** 在 5 秒後回 504，服務不因此卡住其他請求

#### Scenario: 路徑不合法

- **WHEN** `GET /api/runtimes/win/panes/%FF/output`
- **THEN** 回 400，本體為 `{"error": "<原因>"}`（不反射請求送來的位元組），帶 `Cache-Control: no-store` 與
  `X-Content-Type-Options: nosniff`，沒有對 runtime 發出讀取

#### Scenario: 不接受其他 method

- **WHEN** 分別以 `POST` 與 `HEAD` 請求 `/api/runtimes/win/panes/w1:p1/output`
- **THEN** 兩者都回 405，沒有對 runtime 發出讀取；`POST` 的回應本體為 `{"error": "<原因>"}`（`HEAD` 依 HTTP 沒有本體），
  兩者都帶 `Cache-Control: no-store` 與 `X-Content-Type-Options: nosniff`

#### Scenario: 錯誤回應也不可快取

- **GIVEN** runtime `win` 沒有 pane `w1:p99`
- **WHEN** 請求其輸出得到 404
- **THEN** 該回應帶 `Cache-Control: no-store` 與 `X-Content-Type-Options: nosniff`

### Requirement: 輸出端點只接受本機同源請求

系統必須對輸出讀取端點套用與 `pipeline-progress`「寫入端點只接受本機同源請求」相同的檢查：`Host` 標頭必須是
`127.0.0.1:<port>`、`localhost:<port>` 或 `[::1]:<port>`（`<port>` 為服務實際監聽的埠）；若帶 `Origin`
標頭，其值必須是 `http://` 加上同一個 `Host` 值；`Host` 或 `Origin` 重複出現視為不合格。不符合時回 403，
且不得對 runtime 發出讀取。沒有 `Origin` 標頭但 `Host` 合格的請求（同源 `GET`、命令列工具）接受。

#### Scenario: DNS rebinding 被拒

- **WHEN** 請求帶 `Host: evil.example:7770`
- **THEN** 回 403，沒有對 runtime 發出讀取

#### Scenario: 跨站請求被拒

- **WHEN** 請求帶 `Host: 127.0.0.1:7770`、`Origin: https://evil.example`
- **THEN** 回 403，沒有對 runtime 發出讀取

#### Scenario: 自家頁面與命令列可用

- **WHEN** 分別以只有 `Host: 127.0.0.1:7770`，與 `Host: localhost:7770`＋`Origin: http://localhost:7770`
  請求一個存在的 pane
- **THEN** 兩者都回 200

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

### Requirement: 輪詢與顯示

系統必須在有選取期間，由頁面向輸出讀取端點輪詢：前一次請求結束（成功、失敗或逾時）後 1 秒才發下一次，
同一時間至多一個進行中的輸出請求；沒有選取時不得發出輸出請求。換選取或取消選取之後才回來的舊回應必須丟棄，
不得改變面板。回應的 `text` 與面板目前內容相同時不得重寫面板。`text` 必須一律以純文字呈現（等寬字體、保留
換行與空白），內容中的任何 HTML 或 script 字樣都不得被瀏覽器解讀。面板捲動位置在最底端時，新內容到達後
維持在最底端；使用者已往上捲時，新內容到達不得改變捲動位置。`truncated` 為 `true` 時面板頂端顯示
「更早的輸出未顯示」。pane 內容改變後，面板必須在 3 秒內反映。

#### Scenario: 內容跟上

- **GIVEN** 已選定一個每秒多印一行的 pane
- **WHEN** 該 pane 印出新的一行
- **THEN** 面板在 3 秒內出現該行

#### Scenario: 沒有選取就不請求

- **GIVEN** 沒有選取
- **WHEN** 觀察 10 秒
- **THEN** 服務沒有收到任何輸出端點的請求

#### Scenario: 舊回應不蓋掉新選取

- **GIVEN** 已選定 `p1`，其輸出請求尚未回來
- **WHEN** 改選 `p2`，之後 `p1` 的回應才到達
- **THEN** 面板顯示 `p2` 的內容，`p1` 的回應被丟棄

#### Scenario: 請求不堆積

- **GIVEN** 輸出端點每次要 3 秒才回應
- **WHEN** 觀察 10 秒
- **THEN** 任一時刻進行中的輸出請求不超過一個

#### Scenario: 往上捲不被拉回

- **GIVEN** 面板內容超過一屏，使用者已往上捲
- **WHEN** 新內容到達
- **THEN** 捲動位置不變

#### Scenario: 停在底部會跟著走

- **GIVEN** 面板捲動位置在最底端
- **WHEN** 新內容到達
- **THEN** 捲動位置仍在最底端

#### Scenario: 內容不被當成 HTML

- **GIVEN** pane 輸出含 `<script>window.pwned=1</script>` 與 `<b>x</b>`
- **WHEN** 面板顯示該輸出
- **THEN** 兩段字樣原樣以文字出現，`window.pwned` 未被設定，面板內沒有 `b` 元素

#### Scenario: 截斷提示

- **GIVEN** 回應 `truncated` 為 `true`
- **WHEN** 面板顯示
- **THEN** 頂端出現「更早的輸出未顯示」

### Requirement: 失敗與消失的呈現

系統必須在輸出讀不到時如實呈現，不顯示過期內容而不加標示：端點回 404、或被選定的 pane 已不在最新投影中時，
面板顯示「pane 已不存在」並停止輪詢（保留最後一份文字並標為過期）；端點回 503、504、其他非 2xx，或請求本身
失敗時，面板保留最後一份文字並標為過期、顯示原因（含回應本體的 `error`），並繼續依輪詢節奏重試；重試成功
後過期標示與原因消失。

#### Scenario: pane 被關掉

- **GIVEN** 已選定 `w1:p1` 且面板有內容
- **WHEN** 新投影中已沒有 `w1:p1`
- **THEN** 面板顯示「pane 已不存在」，內容標為過期，頁面不再請求輸出

#### Scenario: 端點回 404

- **GIVEN** 已選定的 pane 在 runtime 端已不存在，但投影尚未更新
- **WHEN** 輸出請求回 404
- **THEN** 面板顯示「pane 已不存在」並停止輪詢

#### Scenario: runtime 斷線後恢復

- **GIVEN** 已選定 `wsl` 的某個 pane 且面板有內容
- **WHEN** 輸出請求連續回 503，之後恢復回 200
- **THEN** 503 期間內容標為過期並顯示原因、頁面持續重試；恢復後過期標示與原因消失、內容更新
