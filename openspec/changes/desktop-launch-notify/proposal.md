# proposal：desktop-launch-notify

## Why

ADR-0005 決定 MVP 先用本機網頁＋PWA，「MVP 完成後依實際使用感受決定是否包成 Tauri」。使用者 2026-10-02 提出兩個實際需求：
**一個捷徑就開好**（現在要先手動啟動 `cockpit.exe`、再開瀏覽器）、**桌面通知**（agent 卡住或 task 失敗時不用一直盯著畫面）；
並決定關掉視窗就整個結束、通知四種事件都做且可開關（預設只開 agent blocked 與 task failed）。比較 Tauri 後選擇不包 Tauri：
兩個需求都能以「無主控台視窗的啟動器＋瀏覽器獨立視窗模式＋瀏覽器內建通知」達成，不增加工具鏈與打包，符合 ADR-0005 的理由。

## What Changes

- **新增 `cockpit-launch` 執行檔**（Windows 圖形子系統，不開主控台）：讀同一份設定取得監聽位址；Cockpit 已在跑就只開視窗，沒在跑就在背景啟動
  `cockpit`（記錄寫到設定檔旁的 `cockpit.log`）並等它就緒；以 Chrome（找不到時用 Edge）的 `--app=<網址>` 開獨立視窗；出錯以 Windows 訊息框說明。
- **`cockpit` 新增 `--exit-when-idle` 模式**：記錄 `/ws` 連線數；最後一個連線斷開後 10 秒內沒有新連線就走正常關閉流程結束；啟動後 60 秒內從未有連線也結束。
  不加這個參數時行為不變。
- **安裝腳本**：建置正式版、把兩個執行檔複製到 `%LOCALAPPDATA%\ai-cockpit\bin\`、在桌面建立捷徑（帶 `--config` 指向使用者的設定檔）；偵測到執行中的
  Cockpit 時停止並提示。
- **桌面通知**（純前端，瀏覽器內建 Notification）：比對前後兩份推送狀態，pane 的 agent 狀態變成 `blocked`／`done`、task 狀態變成 `failed`／`completed`
  時發通知；第一份狀態只當基準；同一次更新超過 3 件合併成一則；視窗正在前景時不發；點通知帶出視窗（pane 事件並選定該 pane）。
- **通知設定**：頂列鈴鐺按鈕開啟設定面板，四種事件各自開關（預設 blocked、failed 開，done、completed 關），存在瀏覽器本機儲存；面板顯示通知權限狀態並提供
  「允許通知」按鈕。

## 非目標

- **Tauri 桌面殼**：維持 ADR-0005，不包（理由見 Why）。系統匣圖示、開機自啟、視窗關閉後仍通知：使用者選擇關視窗即結束，不做。
- **自訂捷徑圖示**：需要另外產生 `.ico` 的工具，先用預設圖示。
- **macOS／Linux 的啟動器**：使用者環境是 Windows（Cockpit 以 Windows 為主機、WSL 為第二個 runtime）；非 Windows 編譯時啟動器只印說明。
- **後端推送通知、行動裝置推播、service worker**：不做；通知只在 Cockpit 視窗開著時發生（ADR-0005：不做 service worker）。

## Capabilities

### New Capabilities

- `desktop-launch`：啟動器的行為、`cockpit --exit-when-idle` 的閒置結束規則、安裝腳本。
- `desktop-notifications`：通知事件的判斷、合併與抑制規則，通知設定與權限介面。

### Modified Capabilities

- `cockpit-dashboard`：「路由與內嵌資源」加入 `/app/notify.js`；「畫面整頁重畫」的頂列內容、可點的互動清單、不屬於整頁重畫的範圍加入通知鈴鐺與設定面板。

## Impact

- `cockpit`：新增 `src/bin/cockpit-launch.rs`（與其可測的純函式模組）；`config.rs`（`--exit-when-idle`）；`app.rs`／`http.rs`（`/ws` 連線計數與閒置關閉）。
- 前端：新增 `cockpit/assets/app/notify.js`；`render.js`（頂列鈴鐺、狀態送進通知模組）、`actions.js`（鈴鐺動作、`selectPane`）、`index.html`（載入順序）、
  `style.css`（鈴鐺與面板）；`http.rs` 內嵌新檔。
- `cockpit/src/main.rs`（log 導向檔案時不輸出 ANSI 色碼）；`cockpit/Cargo.toml`（`default-run`）。
- `cockpit/examples/ui_preview.rs`：新增依時間排程改 pane agent 狀態與 task 狀態的環境變數，供驗收腳本產生轉換。
- 新增 `scripts/install-desktop.ps1`；`.gitignore` 加 `cockpit.log`。
- 驗收：新腳本（通知、閒置結束）；既有 12 支腳本全數重跑；真機實際用捷徑開一次。
- 無新的第三方依賴（訊息框直接呼叫 Windows `user32`）。
