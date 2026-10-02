# 桌面啟動器真機驗收（change `desktop-launch-notify` task 4.1）

> 日期：2026-10-02　|　對象：Windows 11、使用者實際設定（repo 根的 `cockpit.toml`，監聽 `127.0.0.1:7770`）、使用者既有的 Chrome。
> 執行者：控制端（Claude）。只記中繼資料；真機畫面截圖不進 repo（見專案 memory「去識別化」）。

## 步驟與結果

| 步驟 | 結果 |
|---|---|
| `pwsh -File scripts\install-desktop.ps1`（預設參數） | 結束碼 0；兩個執行檔複製到 `%LOCALAPPDATA%\ai-cockpit\bin\`；桌面捷徑「AI Agent Cockpit」目標為 `cockpit-launch.exe`、引數 `--config "<repo>\cockpit.toml"`、工作目錄為 repo 根 |
| 從桌面捷徑啟動 | 約 1.5 秒內 `/api/state` 回 200；只有 1 個 `cockpit.exe`，來自安裝目錄；出現 1 個標題為「AI Agent Cockpit」的獨立視窗（併入使用者既有的 Chrome 程序）；過程中沒有主控台視窗 |
| `cockpit.log`（repo 根，已在 `.gitignore`） | 產生，內容沒有 ANSI 色碼 |
| 再點捷徑兩次（間隔 0.3 秒） | 視窗變成 3 個，`cockpit.exe` 仍只有 1 個（沒有重複啟動、沒有錯誤訊息框） |
| 只對標題恰為「AI Agent Cockpit」的 3 個視窗送 `WM_CLOSE` | 視窗全數關閉；約 11 秒後 `cockpit.exe` 結束、7770 不再監聽；使用者其他 Chrome 視窗與程序不受影響 |

## 未在真機驗證（需使用者操作）

- 通知權限：在設定面板按「允許通知」後瀏覽器跳出的授權詢問。
- 真實的 Windows 通知呈現、`renotify` 的重新提醒、點通知把視窗帶到前景並選定 pane。
- 視窗最小化一段時間後是否被瀏覽器的記憶體節省機制凍結（design Risks）。

以上行為已由 `docs/research/2026-10-02/notify-check.js`（以記錄器替換 `Notification`）在 headless Chrome 驗證邏輯與參數。

## 安裝狀態

安裝目錄與桌面捷徑保留，供使用者直接使用。更新：重跑安裝腳本（Cockpit 執行中時腳本會拒絕並提示先關閉）。
