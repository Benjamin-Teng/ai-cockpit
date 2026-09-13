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
