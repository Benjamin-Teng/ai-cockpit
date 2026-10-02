# ADR-0005 畫面先做本機網頁加 PWA，Tauri 等 MVP 之後

- Status: Accepted
- Date: 2026-09-13

## Context

spec §16 定的是 Rust 後端加本機網頁。使用者提出 Tauri 桌面殼是否更合適。比較過
Tauri、Electron、Dioxus、egui／iced／Slint 與「網頁加 PWA 安裝」：在 Rust 後端加
HTML 畫面這條路上，Tauri 是最合適的桌面殼；其他框架不是換語言就是換掉整個畫法。
MVP 要驗的風險全在 HERDR 側，Tauri 對此沒有幫助，卻多一套工具鏈與打包問題，且畫面
驗收在瀏覽器裡最方便。Tauri 程式沒有 console，啟動 `wsl.exe` 會閃黑窗。

## Decision

MVP 三個 change 用 axum 提供本機網頁，並附 PWA manifest 讓 Chrome 能裝成獨立視窗。
畫面端把「怎麼收到整張圖」隔離在 `app/channel.js`，換 Tauri 時只改此檔。子程序啟動
從第一天加 `CREATE_NO_WINDOW`。MVP 完成後依實際使用感受決定是否開一個 change 包成 Tauri。

## Consequences

- 立刻可用，零額外工具鏈。
- 沒有系統匣與開機自啟，後端要自己先啟動。
- 從 Chrome 選單安裝 PWA 不需要 service worker（Chrome 112 桌面版起；查證日期 2026-09-13，
  來源見設計文件 §15），所以不做 service worker。

## 2026-10-02 補充

MVP 完成後依實際使用感受重新評估（change `desktop-launch-notify`）。使用者實際用下來有兩個需求：
一個捷徑就開好（原本要先手動啟動 `cockpit.exe`、再開瀏覽器）、agent 卡住或 task 失敗時有桌面通知；
並決定關掉視窗就整個結束。

決定：**維持本機網頁的做法，不包 Tauri**。以下四件事達成同樣的效果：

- 無主控台視窗的啟動器 `cockpit-launch`：Cockpit 沒在跑就在背景啟動，再以瀏覽器的獨立視窗模式
  （`--app=<網址>`）開畫面。
- `cockpit --exit-when-idle`：最後一個畫面連線斷開並閒置一小段時間後，後端自己正常結束，
  做到「關視窗即結束」。
- 安裝腳本 `scripts/install-desktop.ps1`：建置、複製到使用者目錄並在桌面建立捷徑。
- 瀏覽器內建 Notification：純前端比對前後兩份推送狀態，發出四類事件的通知。

理由：兩個需求都不需要新的工具鏈與打包流程，也沒有動到本決定的核心（後端加網頁、畫面端
`app/channel.js` 隔離通道）；與當初不選 Tauri 的理由一致。系統匣圖示、開機自啟、視窗關閉後仍通知
目前沒有需求；日後若需要，Tauri 仍是可行的下一步，不因這次的決定而變難。
