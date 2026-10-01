# Live Output 依樣式上色真機驗收（change 8 `live-output-color` task 6.2）

> 日期：2026-10-02　|　對象：Windows 端 HERDR 真實 pane `wW:p1`（Claude Code，驗收期間為 `idle`）、
> 真的 `target/debug/cockpit.exe`。
> 性質：一次性真機驗收紀錄；數字是當天這台機器的觀察值，不是規格。對 HERDR 全程唯讀
> （只有 `cockpit.exe` 這個觀察者與 `probe_pane_read` 的 `pane.read`），未執行 `herdr server stop`。
> 本紀錄只含中繼資料（計數、長度、類別），不含任何畫面原文；真機截圖依控制端裁決不進 repo。

## 結論

四項都成立，沒有發現與 spec 或設計 D3 不符之處。

| 項目 | 結果 |
|---|---|
| a 有色片段呈現 | 通過：端點回 107 個片段、其中 53 個帶樣式；面板有 53 個樣式 `span`，逐一對照 spec 對照表，檢查 53 個、不符 0 個 |
| b `textContent` 等於端點 `text` | 通過：頁面自己收到的最後一份回應，第 1 次比對就相符（1707 字等於 1707 字）；之後連續 8 次獨立比對 8 次相符 |
| c 過期轉暗 | 通過：回 503 後 `#output.is-stale`，53 個樣式 `span` 計算後文字色全等於 `--text-dim`、全無背景；停止攔截後標示消失、非暗色 `span` 回到 24 個（與過期前相同） |
| d 端點 `text` 與 HERDR `format=text` | 通過：5 次背靠背成對取樣，每次都是 70 行逐字相同、2 行僅行尾空白不同、0 行內容不同，另加結尾換行差異 |

### 未能驗證

1. **背景色、粗體、斜體、底線、反白沒有在真機上出現**：這個 pane 當下的輸出只有前景色（綠 20 段、紅 2 段、亮黃 1 段、白 1 段）
   與無前景色的 `dim`（29 段）。這些樣式由 `docs/research/2026-10-02/output-color-check.js` 在 `ui_preview` 的 64 格全組合與
   16 色樣本上覆蓋，沒有真機證據。Windows 端不可寫入，無法自造內容（同 `ansi-probe.md`）。
2. **工作中（畫面持續變化）的 pane 未量**：驗收期間 `wW:p1` 為 `idle`，d 的 5 次取樣內容完全一致，所以「背靠背成對」
   在這裡不會被畫面變動干擾，但也沒有量到「變動中兩次讀取不同步」的情況。
3. **只量一個 pane、一個 runtime**。

## 環境

- HERDR：`herdr 0.9.2-preview.2026-09-29-8e78f929d8f0`（client）；runtime 卡讀到 server
  `0.9.0-preview.2026-09-08-62431dbd033b`、protocol 22，連線狀態 `connected`。
- cockpit：分支 `feat/live-output-color`、基底 commit `b945a71`，`cargo build -p cockpit` 後直接執行
  `target\debug\cockpit.exe --config <臨時設定檔>`，監聽 `127.0.0.1:7792`（啟動前確認無 LISTEN；7778 與 9012–9014 是別的程序，未動）。
- 瀏覽器：Chrome 154.0.8037.58，`--headless=new`，viewport 1536 x 1000，raw CDP（寫法比照
  `docs/research/2026-10-02/output-color-check.js`），Node v22.19.0。
- 臨時設定檔放在 `%TEMP%\cockpit-output-color-live\`：一個 `win` runtime（`kind = "herdr"`）、一個 project、一個 workstream
  綁定 `wW:p1` 所在 workspace、一個 task、狀態檔用相對路徑。設定與狀態檔驗收後已刪除。

## 方法

選 pane 是真實的滑鼠點擊（`Input.dispatchMouseEvent` 點 `.pane-row[data-pane="wW:p1"]`）。**點擊之前**在頁面裝一層 `window.fetch`
包裝，保存頁面自己收到的 `/output` JSON（只留最近 5 份，只在頁面內存取，不輸出內容）。以下比對都是一次同步的頁面內求值：
先確認 `.output-text.textContent` 等於「最後一份回應的 `text`」，相等才做逐片段對照，避免輪詢中途換了內容造成誤判。

### a 有色片段呈現

最後一份回應與面板（同一時刻）：

| 項目 | 數量 |
|---|---|
| 片段總數 | 107（53 個帶樣式、54 個純文字） |
| 回應中 `fg` 名稱 | `green` 20、`red` 2、`bright_yellow` 1、`white` 1 |
| 回應中 `bg`、`bold`、`italic`、`underline`、`reverse` | 0 |
| 回應中 `dim`（皆無前景色） | 29 |
| 面板樣式 `span` | 53（與帶樣式片段數相同） |
| 面板 class 家族 | `ansi-fg` 24（`ansi-fg-green` 20、`ansi-fg-red` 2、`ansi-fg-yellow` 1、`ansi-fg-white` 1）、`ansi-dim` 29 |

逐片段對照（DOM 子節點依序對回應片段，文字長度與內容須一致才算對齊）：對齊不符 0 個；53 個帶樣式片段的計算後文字色、
背景色（預期皆無）、字重、斜體、底線都符合 spec「輸出依樣式上色」對照表（綠→`--ok`、紅→`--bad`、亮黃→`--warn`、白→`--text`、
只帶 `dim`→`--text-dim`，預期色票以頁面上的 `var(--…)` 實際量測，不手 key），不符 0 個。無樣式片段沒有帶 class 的 `span`。

### b `textContent` 等於端點 `text`

- 重試次數：第 1 次比對即相符（比對歷程 `[true]`）。
- 之後再每 0.5 秒獨立比對 8 次，8 次相符。
- 該份回應：`text` 1707 字、面板 `textContent` 1707 字、`truncated` 為 `false`、片段的 `text` 依序串接等於 `text`。

### c 過期轉暗

用 CDP `Fetch` 攔截 `/output` 請求，回 503 `{"error":"x"}`：

| 階段 | `#output.is-stale` | 樣式 `span` | 非 `--text-dim` 文字色 | 有背景 |
|---|---|---|---|---|
| 過期前 | 否 | 53 | 24 | 0 |
| 503 攔截中（先等 `is-stale` 出現，再停 3 秒） | 是 | 53 | **0** | **0** |
| 停止攔截、`is-stale` 消失後 1.5 秒 | 否 | 53 | 24 | 0 |

「過期前的 24」就是前景色的 24 個片段；過期期間全部轉成 `--text-dim`，恢復後回到相同的 24 個。

### d 端點 `text` 與 HERDR `format=text`

每次取樣：先由 Node 直接請求端點 `/output`，立即以
`probe_pane_read --pane wW:p1 --source recent --lines 200 --format text --count 1 --dump-dir <%TEMP% 下目錄>` 讀 HERDR，
兩者相隔數十毫秒。比對方式同 `ansi-probe.md`：兩邊各以 `\n` 切行，逐行分類為「逐字相同」「只差行尾空白」「內容不同」，
再看兩邊結尾是否有換行。

5 次取樣結果完全相同（pane 閒置）：

| 項目 | 端點 `text` | HERDR `format=text` |
|---|---|---|
| 字元數 | 1707 | 1706 |
| 行數（不含結尾換行造成的空尾段） | 72 | 72 |
| 結尾換行 | 無 | 有 |
| 行尾空白字元總數 | 2（集中在 2 行） | 0 |

逐行：70 行逐字相同、2 行只差行尾空白（端點較長）、**0 行內容不同**。字元數差 = 端點多 2 個行尾空白、HERDR 多 1 個結尾換行，
正好相抵 1。端點回應的片段串接等於其 `text`（5 次皆是）。這與設計 D3 一致：端點 `text` 與 HERDR `format=text`
只差行尾空白與結尾換行；方向與 `ansi-probe.md` 的觀察相同（`ansi` 衍生的 `text` 保留部分行尾空白，HERDR 的 `text` 剪掉）。

## 去識別化與截圖

- 沒有把畫面原文、使用者名稱、主機名稱寫進本紀錄；只用 pane id。
- 截圖前在頁面裝 `MutationObserver` 遮罩使用者名稱與主機名稱，並確認頁面 `textContent` 不含兩者（命中 0）。
  截圖放在 `%TEMP%\cockpit-output-color-live\shot.png` 供控制端檢視，**不進 repo**，由控制端看完後刪除。

## 收尾

- 依 Windows PID 終止自己啟動的 `cockpit.exe`（PID 46840）；headless Chrome 由腳本依 PID 終止並確認沒有殘留使用其
  user-data-dir 的 `chrome.exe`，暫存 profile 已清除。
- `:7792` 與 CDP 埠 `18810` 均無 LISTEN。
- 臨時設定檔、狀態檔、日誌、5 次（外加 1 次方向確認）的 dump 目錄、驗收腳本與腳本回報 JSON 都已刪除，
  `%TEMP%\cockpit-output-color-live\` 只留 `shot.png`。
- 驗收腳本是一次性暫存腳本（沿用 `output-color-check.js` 的 CDP 與攔截寫法），不進 repo。
