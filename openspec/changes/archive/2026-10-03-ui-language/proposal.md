# proposal：ui-language

## Why

Cockpit 的介面文字全部寫死成繁體中文，後端也有約 45 條訊息由前端原文顯示（進度被拒原因、寫入錯誤、Live Output 錯誤、
連線原因、project 警告），桌面啟動器的錯誤訊息框同樣只有中文。repo 已公開、宣傳頁與 README 預設英文，即將發布 v0.1.0；
英文使用者打開儀表板卻全是中文。使用者 2026-10-03 決定讓介面語言可選中文或英文，並選定：預設語言跟宣傳頁同一套規則、
頂列獨立切換按鈕、後端訊息改成代碼由前端翻譯、啟動器跟隨 Windows 顯示語言。

## What Changes

- 新增前端字典 `cockpit/assets/app/i18n.js`（繁中與英文各一份），所有介面字串改由它提供；拼接句改成帶具名佔位符的完整句子。
- 語言決定：手動選過就照選擇（`localStorage` 的 `cockpit.lang`）；否則第一順位瀏覽器語言是台港澳中的中文、或時區在台港澳中
  → 繁中，其他 → 英文。
- 頂列鈴鐺旁新增語言切換按鈕；按下後記住選擇並重新載入頁面，其他開著的 Cockpit 視窗跟著切換。
- 後端原文顯示的訊息改為同時回傳「代碼＋參數」：寫入與 agent 端點的錯誤本體加 `params`（`code` 已有的沿用），Live Output 錯誤、
  連線原因、HERDR protocol 警告、project 警告在投影裡加對應的代碼欄位。原本的中文文字保留（命令列與日誌用）；HERDR 自己回的
  英文錯誤照原文顯示。
- 桌面啟動器的訊息框依 Windows 顯示語言選繁中或英文。
- 已是英文的產品詞彙（running、blocked、Completed、Failed、Factory Floor、Live Output、HERDR 狀態值等）兩種語言都照原樣顯示。

## 非目標

- 不翻譯終端機與日誌訊息（`dashboard 已啟動` 等）、設定檔驗證錯誤、`tracing` 輸出。
- 不翻譯使用者資料（project、task、workstream 名稱、檔案內容、pane 輸出）與 HERDR／git 回傳的原文。
- 不做執行期熱切換（切換一律重新載入頁面）；不支援簡體中文或其他語言。

## Capabilities

### New Capabilities

- `ui-language`：介面語言的決定、切換、字典涵蓋範圍、後端訊息代碼契約、啟動器訊息框語言。

### Modified Capabilities

- `cockpit-dashboard`：需求「路由與內嵌資源」的 `/app/<檔名>` 清單與路由 scenario 加入 `i18n.js`（content-type 為
  JavaScript）；其餘需求不變。後端錯誤本體與投影只新增欄位，既有欄位與既有需求的行為不變。

## Impact

- 前端：`cockpit/assets/index.html` 與 `cockpit/assets/app/` 下全部 8 個腳本，新增 `i18n.js`；`cockpit/src/http.rs` 的資產白名單。
- 後端：`cockpit-core`（`Rejection` 代碼、投影的訊息代碼欄位）、`cockpit-herdr`（連線原因與 protocol 警告的代碼）、
  `cockpit`（寫入、agent、Live Output 端點錯誤本體、project 警告、啟動器訊息框）。
- 測試：Rust 新增代碼與語言判斷測試；新增驗收腳本 `docs/research/2026-10-03/i18n-check.js`；既有 14 支驗收腳本在繁中預設下照常。
- 文件：`cockpit/README.md`、`README.md`、`README.zh-TW.md`、`docs/handover.md`。
