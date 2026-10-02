# live-output Specification

## Purpose

定義 Live Output：把使用者選定的那個 pane 的畫面輸出依 HERDR 的樣式上色顯示在 Cockpit（內容仍是不含控制序列的純文字）。涵蓋讀取端點（路徑、回應、
錯誤對應、本機同源檢查）與畫面面板的行為（選取、輪詢、失敗呈現、作為下半部第一個分頁的顯示）。Live Output 是
HERDR pane 輸出的投影，不是 terminal，不接受輸入。證據：`docs/cockpit-spec.md` §12 與 Scenario E、
`CONTEXT.md`「Live Output」、`docs/research/2026-09-19/pane-read-probe.md`。

## Requirements

### Requirement: 輸出讀取端點

系統必須提供 `GET /api/runtimes/<runtime>/panes/<pane>/output`：每次請求即時向該 runtime 讀一次該 pane 的
輸出（最多最近 200 行），不快取、不在請求之間保留任何與選取有關的狀態。成功回 200，本體為 JSON 物件，
欄位為 `runtime`、`pane_id`（與路徑相同）、`format`（目前固定為 `"text"`，表示 `text` 為純文字）、`text`（純文字，不含終端機
控制序列）、`segments`（陣列，見下）、`truncated`（布林，表示還有更早的輸出未回傳）。`text` 與 `segments` 依「輸出樣式轉換」由
runtime 的帶樣式輸出產生。`segments` 的每個元素帶 `text`（非空字串）與樣式欄位：`fg`、`bg` 為 16 色名稱之一，未指定時省略；
`bold`、`dim`、`italic`、`underline`、`reverse` 只在為 `true` 時出現。所有片段的 `text` 依序串接必須等於回應的 `text`；
相鄰兩段的樣式必須不同；`text` 為空字串時 `segments` 為空陣列。這個端點的所有回應（含 400、403、404、405、503、504）都必須帶
`Cache-Control: no-store` 與 `X-Content-Type-Options: nosniff`。路徑中的 `<runtime>` 或 `<pane>` 片段解碼後不是合法 UTF-8 時回 400，且不得對 runtime 發出讀取；
`<runtime>` 不是設定中的 runtime id、或該 pane 不存在時回 404；
runtime 無法連線或讀取失敗時回 503；單次讀取超過 5 秒回 504。所有非 200 回應本體沿用 `{"error": "<原因>"}`。
端點只接受 `GET`，不改變任何狀態；其他 method（含 `HEAD`）一律回 405，且不得對 runtime 發出讀取。

#### Scenario: 讀到輸出

- **GIVEN** runtime `win` 的 pane `w1:p1` 目前畫面有三行文字，第二行是紅色的 `error`
- **WHEN** `GET /api/runtimes/win/panes/w1:p1/output`
- **THEN** 回 200，`format` 為 `text`，`text` 為那三行（不含控制序列），`segments` 串接後等於 `text`，其中 `error` 所在的片段帶
  `fg: red`，`truncated` 為 `false`，回應帶 `Cache-Control: no-store` 與 `X-Content-Type-Options: nosniff`

#### Scenario: 沒有樣式的輸出

- **GIVEN** pane 畫面只有一行沒有任何樣式的文字 `hello`
- **WHEN** 請求輸出
- **THEN** `segments` 為一段 `{"text": "hello"}`，不帶任何樣式欄位

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
被選定的 pane 列必須有可辨識的標示；再選另一個 pane 即取代原選取。Live Output 面板是中欄下半部分頁區的第一個分頁（見
`cockpit-dashboard`「版面與窄視窗」、`file-review`「檔案分頁」），該分頁為目前分頁時面板可見：沒有選取時顯示空狀態，提示
使用者選一個 pane；有選取時顯示該 pane 的 runtime 與 pane id 與輸出；面板上有「取消選取」可取消選取，取消後回到空狀態。
以任何方式選定 pane（點列、「看輸出」、鍵盤）時，分頁區必須切換到 Live Output 分頁；取消選取不切換分頁。選取只存在於
該瀏覽器頁面（不送到服務、不持久化，重新整理後清空），整頁重畫不得清除選取。改綁模式期間 pane 列不可點選
（不呈現可點選的樣式，點列的任何區域都不改變選取），列上的「綁定到這裡」仍是送出覆蓋；既有的選取與面板在改綁模式
期間保留，離開改綁模式後 pane 列恢復可點選。Live Output 面板在版面中佔有自己的區域，不得覆蓋頁面上其餘的內容；
有選取時，頁面上其餘的操作（選另一個 pane、「畫面操作」的所有按鈕、Project 切換）必須仍可直接操作。pane 列必須能以
鍵盤聚焦並以 Enter 或 Space 選定；整頁重畫不得讓鍵盤焦點離開原本聚焦的那個 pane 列（見 `cockpit-dashboard`「畫面整頁重畫」）。

#### Scenario: 點 pane 列

- **GIVEN** 畫面有 runtime `win` 的 pane `w1:p1`，目前沒有選取
- **WHEN** 點該 pane 列
- **THEN** 該列出現選定標示，Live Output 面板由空狀態改為標題顯示 `win` 與 `w1:p1`，頁面開始請求該 pane 的輸出

#### Scenario: 選定 pane 時切回 Live Output 分頁

- **GIVEN** 已選定 `w1:p1`，目前分頁是某個檔案分頁
- **WHEN** 點 `w1:p2` 列
- **THEN** 目前分頁改為 Live Output，面板顯示 `w1:p2`，檔案分頁仍在分頁列上

#### Scenario: 沒有選取時顯示空狀態

- **GIVEN** 目前沒有選取，也沒有還原任何檔案分頁
- **WHEN** 開啟 `/` 並等待首份投影畫完
- **THEN** Live Output 分頁為目前分頁，面板可見，顯示提示選一個 pane 的空狀態，沒有「取消選取」，頁面沒有發出輸出請求

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

系統必須在有選取且 Live Output 分頁為目前分頁期間，由頁面向輸出讀取端點輪詢：前一次請求結束（成功、失敗或逾時）後 1 秒
才發下一次，同一時間至多一個進行中的輸出請求；沒有選取、或目前分頁不是 Live Output 時不得發出新的輸出請求；切回 Live Output
分頁且有選取時立即請求一次。換選取或取消選取之後才回來的舊回應必須丟棄，不得改變面板。回應的 `segments`（含每段文字與樣式）
與面板目前內容相同時不得重寫面板；只有樣式不同、文字相同時必須重畫。面板內容一律以純文字呈現（等寬字體、保留換行與空白），
樣式依「輸出依樣式上色」，內容中的任何 HTML 或 script 字樣都不得被瀏覽器解讀；面板的 `textContent` 必須等於回應的 `text`。
面板捲動位置在最底端時，新內容到達後維持在最底端；使用者已往上捲時，新內容到達不得改變捲動位置。切到其他分頁再切回 Live Output
分頁時，切走前停在最底端則切回後仍在最底端（含切回後立即到達的新內容），否則維持切走前的捲動位置。`truncated` 為 `true` 時
面板頂端顯示「更早的輸出未顯示」。Live Output 分頁為目前分頁時，pane 內容改變後面板必須在 3 秒內反映。

#### Scenario: 內容跟上

- **GIVEN** 已選定一個每秒多印一行的 pane
- **WHEN** 該 pane 印出新的一行
- **THEN** 面板在 3 秒內出現該行

#### Scenario: 沒有選取就不請求

- **GIVEN** 沒有選取
- **WHEN** 觀察 10 秒
- **THEN** 服務沒有收到任何輸出端點的請求

#### Scenario: 檔案分頁期間不請求輸出

- **GIVEN** 已選定一個 pane，之後切到某個檔案分頁
- **WHEN** 觀察 10 秒後切回 Live Output 分頁
- **THEN** 切走後至多再完成一個先前已發出的請求，之後直到切回前服務沒有收到輸出請求；切回後立即收到一個輸出請求

#### Scenario: 切回時保持貼底

- **GIVEN** 已選定一個每秒多印一行的 pane，面板停在最底端
- **WHEN** 切到某個檔案分頁 5 秒後切回 Live Output 分頁
- **THEN** 切回後面板在最底端，並在 3 秒內出現切走期間印出的行

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

#### Scenario: 相同內容不重寫

- **GIVEN** 面板正顯示某份帶樣式的輸出
- **WHEN** 下一次回應的 `segments` 與目前完全相同
- **THEN** 面板內容節點沒有被替換或修改

#### Scenario: 只有顏色改變也會重畫

- **GIVEN** 面板正顯示一段 `fg: red` 的 `status`
- **WHEN** 下一次回應同一位置改為 `fg: green` 的 `status`，`text` 不變
- **THEN** 面板在 3 秒內改以 `--ok` 顯示 `status`

### Requirement: 失敗與消失的呈現

系統必須在輸出讀不到時如實呈現，不顯示過期內容而不加標示：端點回 404、或被選定的 pane 已不在最新投影中時，
面板顯示「pane 已不存在」並停止輪詢（保留最後一份內容並標為過期）；端點回 503、504、其他非 2xx，或請求本身
失敗時，面板保留最後一份內容並標為過期、顯示原因（含回應本體的 `error`），並繼續依輪詢節奏重試；重試成功
後過期標示與原因消失。標為過期期間，面板內所有文字（含有色片段）一律以 `--text-dim` 呈現，背景色與反白的淡底不畫；
過期標示消失後恢復各片段的樣式。

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

#### Scenario: 過期時有色內容一併轉暗

- **GIVEN** 面板顯示含 `fg: red` 片段與 `bg: green` 片段的輸出
- **WHEN** 輸出請求回 503，之後恢復回 200
- **THEN** 503 期間兩個片段的計算後文字顏色都等於 `--text-dim`、`bg: green` 片段沒有背景色；恢復後兩個片段回到 `--bad` 文字與
  `--ok` 20% 淡底

### Requirement: 輸出樣式轉換

系統必須以 runtime 提供的帶樣式輸出（終端機 SGR 序列）產生輸出讀取端點的 `text` 與 `segments`，轉換規則如下：

- 可見字元、換行與 tab 保留；`\r` 與其他控制字元（C0、DEL、C1 即 U+0080–U+009F）丟棄。
- SGR 以外的控制序列（游標移動、清除畫面、OSC、DCS、帶 `?`／`>` 等私有前綴的 CSI 等）整段丟棄，不得留下任何字元；輸出結尾
  不完整的序列也丟棄；參數（冒號子參數也計入）超過 32 個的 SGR 整段略過。
- SGR 參數依序套用，不認得的參數略過，不影響同一序列中的其他參數；以冒號帶子參數的參數，除 `38`／`48`／`58` 外一律略過；`58`（底線顏色）依下方 `38`／`48` 的規則吃掉參數，但不產生任何樣式：
  - `0`（或空參數）：重設所有樣式。
  - `1` 粗體、`2` 變暗、`3` 斜體、`4` 底線、`7` 反白；`22` 取消粗體與變暗、`23` 取消斜體、`24` 取消底線、`27` 取消反白。
  - 前景色 `30`–`37`、`90`–`97`，`39` 回預設；背景色 `40`–`47`、`100`–`107`，`49` 回預設。
  - `38`／`48` 後接 `5;N`（256 色）或 `2;R;G;B`（真彩色）設定前景／背景色，分號與冒號兩種分隔寫法皆接受（冒號寫法可在 `2`
    之後帶一個色彩空間 id，例如 `38:2::R:G:B`）。分號寫法吃掉的參數個數：子類型為 `5` 時吃 `5` 與其後至多 1 個、為 `2` 時吃 `2`
    與其後至多 3 個、其他子類型只吃子類型本身；吃掉的參數不再當成獨立參數解讀。參數不完整或任一數值超出 0–255 時，該色彩設定
    不生效，其後未被吃掉的參數照常套用。
- 16 色的名稱為 `black`、`red`、`green`、`yellow`、`blue`、`magenta`、`cyan`、`white` 與各自加 `bright_` 前綴的版本；
  `30`–`37`／`40`–`47` 對應前八個，`90`–`97`／`100`–`107` 對應 `bright_` 版本。
- 256 色的 0–15 直接對應 16 色；16–255 先換成 RGB（16–231 為 6×6×6 色立方，各分量依序為 0、95、135、175、215、255；
  232–255 為灰階 8＋10×(N−232)），再與真彩色一樣歸到 16 色之一：
  - 最大分量與最小分量的差小於 16，或該差乘以 10 小於最大分量時，視為無彩：（最大分量＋最小分量）÷2 小於 48 為 `black`，小於 160 為
    `bright_black`，其餘為 `white`。
  - 其餘依色相（0–360°）歸色：小於 12° 或不小於 330° 為 `red`；12°–75° 為 `yellow`；75°–165° 為 `green`；
    165°–200° 為 `cyan`；200°–270° 為 `blue`；270°–330° 為 `magenta`（區間含下界不含上界）。

#### Scenario: 基本色與樣式

- **GIVEN** pane 輸出為 `ESC[1;31merror ESC[0mdone`
- **WHEN** 轉換
- **THEN** `text` 為 `error done`，`segments` 為兩段：`error`（含其後的空格）帶 `fg: red` 與 `bold`；`done` 不帶任何樣式欄位

#### Scenario: 非 SGR 序列與 CR 被丟棄

- **GIVEN** pane 輸出含 `ESC[2J`、`ESC[10;5H`、`ESC]0;title BEL`，以及每行結尾的 `\r\n`
- **WHEN** 轉換
- **THEN** `text` 不含上述任何序列的字元，也不含 `\r`；換行保留

#### Scenario: 256 色與真彩色歸色

- **GIVEN** pane 輸出依序以 `ESC[38;5;196m`、`ESC[38;5;244m`、`ESC[38;2;215;119;87m`、`ESC[48:5:21m` 設色
- **WHEN** 轉換
- **THEN** 依序得到前景 `red`、前景 `bright_black`、前景 `yellow`、背景 `blue`

#### Scenario: 深色主題 diff 底色不被歸成灰

- **GIVEN** pane 輸出依序以 `ESC[48;2;34;92;43m`、`ESC[48;2;122;41;54m`、`ESC[48;2;71;88;74m`、`ESC[48;2;105;72;77m` 設背景色
- **WHEN** 轉換
- **THEN** 依序得到背景 `green`、`red`、`green`、`red`

#### Scenario: 無彩門檻的界線

- **GIVEN** pane 輸出依序以 `ESC[38;2;128;128;143m`、`ESC[38;2;128;128;144m`、`ESC[38;2;226;226;251m`、`ESC[38;2;225;225;250m` 設前景色
- **WHEN** 轉換
- **THEN** 依序得到 `bright_black`（差 15，小於 16）、`blue`（差 16，且 10×16＝160 不小於最大分量 144）、`white`（差 25，10×25＝250
  小於最大分量 251）、`blue`（差 25，10×25＝250 不小於最大分量 250）

#### Scenario: 不認得的參數不影響其他參數

- **GIVEN** pane 輸出為 `ESC[5;32;99mok`
- **WHEN** 轉換
- **THEN** `ok` 帶 `fg: green`，沒有其他樣式

#### Scenario: 色彩參數吃掉的個數

- **GIVEN** pane 輸出依序為 `ESC[38;9;1ma`、`ESC[0;38;5;256;4mb`、`ESC[0;38;5mc`、`ESC[0;58;5;1;3md`
- **WHEN** 轉換
- **THEN** `a` 只帶 `bold`（`9` 被當成子類型吃掉、`1` 照常套用）；`b` 只帶 `underline`（`256` 超出範圍，色彩不生效）；`c` 沒有任何樣式；`d` 只帶 `italic`（`58;5;1` 整組被吃掉、不產生樣式）

#### Scenario: DEL 與 C1 控制字元被丟棄

- **GIVEN** pane 輸出為 `a`、DEL（U+007F）、`b`、U+0085、`c`
- **WHEN** 轉換
- **THEN** `text` 為 `abc`

#### Scenario: 結尾不完整的序列

- **GIVEN** pane 輸出以 `abc ESC[3` 結尾
- **WHEN** 轉換
- **THEN** `text` 以 `abc` 加一個空格結尾，不含 `[3` 或其他殘字

### Requirement: 輸出依樣式上色

系統必須依回應的 `segments` 在 Live Output 面板上呈現樣式，且所有顏色取自既有色票：

- 前景色：`red`／`bright_red`→`--bad`；`green`／`bright_green`→`--ok`；`yellow`／`bright_yellow`→`--warn`；
  `blue`／`bright_blue`／`cyan`／`bright_cyan`→`--accent`；`magenta`／`bright_magenta`→`--graph-lane-4`；
  `white`／`bright_white`→`--text`；`black`／`bright_black`→`--text-dim`。
- 背景色：以上述對應色票的 20% 不透明度畫在該段文字後方；該段同時指定前景色時改用 14%（有色字疊在另一種顏色的淡底上，
  20% 會讓對比低於 4.5:1）；`black` 背景不畫。
- 反白：以該段文字本身顏色的 20% 不透明度畫在後方，文字顏色不變；同時有背景色時以反白為準。
- 粗體以字重 700 呈現；斜體、底線照字面呈現。
- 變暗：沒有前景色的片段改用 `--text-dim`；有前景色的片段維持其前景色。
- 每一段文字都必須以純文字寫入（不得解讀其中的 HTML），樣式只能來自固定的名稱對照表；回應中不在對照表內的顏色名稱視為未指定。
- 有色文字（含畫在淡底上的文字）與其實際背景的對比必須不低於 4.5:1（見 `cockpit-dashboard`「Direction 01 視覺語彙」）。

#### Scenario: 紅字與綠底

- **GIVEN** 回應有一段 `fg: red` 的 `FAIL`，與一段 `bg: green` 的 `+ added`
- **WHEN** 面板顯示
- **THEN** `FAIL` 的計算後文字顏色等於 `--bad`；`+ added` 的計算後背景色等於 `--ok` 的 20% 不透明度，文字顏色為 `--text`

#### Scenario: 有色字疊在淡底上

- **GIVEN** 回應有一段同時帶 `fg: red` 與 `bg: white` 的片段
- **WHEN** 面板顯示
- **THEN** 該段的計算後背景色等於 `--text` 的 14% 不透明度，文字顏色等於 `--bad`

#### Scenario: 反白優先於背景

- **GIVEN** 回應有一段同時帶 `fg: red`、`bg: white`、`reverse` 的片段，與一段只帶 `reverse` 的片段
- **WHEN** 面板顯示
- **THEN** 前者的計算後背景色等於 `--bad` 的 20% 不透明度、文字顏色等於 `--bad`；後者的計算後背景色等於 `--text` 的 20% 不透明度

#### Scenario: 黑與亮黑背景

- **GIVEN** 回應有一段 `bg: black` 與一段 `bg: bright_black` 的片段，兩者都沒有前景色
- **WHEN** 面板顯示
- **THEN** 前者沒有背景色；後者的計算後背景色等於 `--text-dim` 的 20% 不透明度

#### Scenario: 粗體、斜體與底線

- **GIVEN** 回應有分別只帶 `bold`、`italic`、`underline` 的三段
- **WHEN** 面板顯示
- **THEN** 三段的計算後樣式依序為字重 700、斜體、底線，文字顏色皆為 `--text`

#### Scenario: 變暗的預設色文字

- **GIVEN** 回應有一段只帶 `dim` 的片段
- **WHEN** 面板顯示
- **THEN** 該段的計算後文字顏色等於 `--text-dim`

#### Scenario: 上色後內容仍不被當成 HTML

- **GIVEN** 回應有一段 `fg: red` 的 `<script>window.pwned=1</script>`，與一段 `bold` 的 `<b>x</b>`
- **WHEN** 面板顯示
- **THEN** 兩段字樣原樣以文字出現，`window.pwned` 未被設定，面板內沒有 `b` 或 `script` 元素

#### Scenario: 不認得的顏色名稱

- **GIVEN** 回應有一段 `fg` 為 `"orange; background: red"` 的片段
- **WHEN** 面板顯示
- **THEN** 該段以預設文字顏色呈現，面板上沒有任何元素的 class 或 style 含該字串

#### Scenario: 對比

- **GIVEN** 面板顯示的輸出涵蓋「無前景色、7 種前景色票」與「無背景、7 種背景色票（`black` 背景除外）」的每一種組合，並涵蓋
  每種前景色票的反白
- **WHEN** 量測每段文字顏色與其實際背景（淡底合成到面板底色後）
- **THEN** 對比皆不低於 4.5:1
