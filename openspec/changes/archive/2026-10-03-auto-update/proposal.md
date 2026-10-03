# proposal：auto-update

## Why

安裝檔沒有程式碼簽章，每個新版本第一次執行都會跳 Windows SmartScreen 警告。研究結論是便宜的簽章方案都無法讓第一次
安裝免警告（台灣個人無法用 Azure Artifact Signing；SignPath、Certum 仍要累積信譽）；同類產品 Shioaji Pro 的 Windows
安裝檔同樣未簽章，靠程式內自動更新讓警告只出現在首次安裝——程式自己下載的檔案沒有「從網路下載」標記，SmartScreen
不檢查。使用者 2026-10-03 決定自動更新隨下一版（v0.1.1）一起發布；v0.1.0 已改標預發布，`releases/latest` 目前回 404。

## What Changes

- 已用安裝檔安裝的 `cockpit-launch`，在後端沒在執行、準備啟動後端之前，向 GitHub 查最新正式版；有較新版時以訊息框詢問
  是否更新。同意後下載該版安裝檔、以同一 release 的 `SHA256SUMS.txt` 驗證 SHA-256，啟動安裝檔的更新模式後結束自己。
- 檢查每 24 小時最多一次；網路錯誤、查無正式版（404）、速率限制等一律不打擾使用者，照常啟動現有版本。
- 安裝檔新增「更新模式」（自訂命令列參數）：等候執行中的 Cockpit 結束（有上限）再安裝，裝完以資料目錄為工作目錄重新
  啟動 `cockpit-launch`。一般安裝與一般靜默安裝的行為不變。
- 免安裝 zip、`install-desktop.ps1` 安裝與開發建置不檢查更新、不連網；使用者可用環境變數完全關閉檢查。
- 冒煙測試加入更新模式的情境；`cockpit` crate 新增 HTTPS client 與 SHA-256 相依。

## Capabilities

### New Capabilities

- `auto-update`：啟動器的更新檢查、詢問、下載與驗證、交棒給安裝檔的完整行為，以及不檢查的情況。

### Modified Capabilities

- `desktop-launch`：「啟動器」的步驟在偵測到後端沒在執行之後、背景啟動之前插入更新檢查。
- `release-distribution`：「Windows 安裝檔」新增更新模式（等候程式結束、裝完重新啟動），「執行中不覆寫」在更新模式下改為
  等候後再判定；「推版本 tag 時發布 release」的冒煙測試涵蓋更新模式。

## 非目標

- 程式碼簽章（Authenticode）與 Microsoft Store：研究結論見本 change 的 design「背景」，不在本 change 處理。
- 獨立於 GitHub 帳號的更新檔簽章（例如 minisign）：本 change 只做 HTTPS 加同 release 的 SHA-256 驗證，帳號被盜的威脅列為
  已知風險，見 design「Risks」。
- 後端執行中（瀏覽器視窗開著）時提示更新、背景下載、差異更新、回滾到舊版、更新頻道（預發布）。
- 免安裝 zip 的自動替換。
- 後端 `cockpit` 與畫面不參與更新（設計文件 §8 的 HTTP 路由與畫面不變）。

## Impact

- 程式：`cockpit/src/bin/cockpit-launch.rs`（I/O 串接）、`cockpit` crate 新增更新的純邏輯模組；`cockpit/Cargo.toml` 新增
  HTTPS client 與 SHA-256 相依（版號實作時查證）。
- 打包：`packaging/ai-cockpit.iss`（更新模式的等候與重新啟動）、`packaging/smoke-test.ps1`（新增情境）；`release.yml` 的
  資產命名與 `SHA256SUMS.txt` 格式成為更新器依賴的契約，不改。
- 文件：README（中英）說明自動更新與如何關閉提示的行為、`CHANGELOG.md` 的下一版段落。
- 協作：版本號升到 0.1.1、rc 演練與正式發布由打包發佈 session 負責。
