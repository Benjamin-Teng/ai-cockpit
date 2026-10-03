# proposal：release-packaging

## Why

目前要用 Cockpit 只能從原始碼建置：裝 Rust 工具鏈、`cargo build`，再用 `scripts/install-desktop.ps1` 建捷徑。宣傳頁
（`site/`）與 README 的「開始使用」也全是原始碼流程，沒有可下載的東西。Tauri 評估的結論是不包 Tauri、維持
`cockpit-launch` 加本機網頁（ADR-0005 2026-10-02 補充段），所以發行形式就是把現有兩個執行檔包成 Windows 安裝檔。
使用者 2026-10-03 定案：Inno Setup 安裝精靈加 zip、推 tag 時由 GitHub Actions 建置並建 release、MIT 授權、第一版
`v0.1.0`，排在介面語言功能（change `ui-language`）之後發布，讓安裝檔包含它。

## What Changes

- 新增 Inno Setup 腳本（`packaging/`）：安裝精靈 `setup.exe`，裝到使用者資料夾、不需要管理員權限；建桌面與開始功能表
  捷徑「AI Agent Cockpit」；可從 Windows「設定 > 應用程式」解除安裝。安裝檔不需要原始碼與 Rust 工具鏈。
- 另附免安裝 zip：兩個執行檔、`cockpit.example.toml`、`LICENSE`。
- 新增 release workflow（`.github/workflows/release.yml`）：推 `vX.Y.Z` tag 時在 Windows runner 跑品質 gate、建置、打包、
  安裝與解除安裝冒煙測試，再建 GitHub Release 並上傳安裝檔、zip 與 SHA-256 檢查碼；tag 版本與 `cockpit` crate 版本不一致
  就失敗。另可手動觸發只建置不發布的演練。
- 新增 `LICENSE`（MIT）與 `CHANGELOG.md`（release 說明的來源）。
- 宣傳頁新增「下載」區塊：顯示最新 release 的版本號與日期、安裝檔與 zip 下載按鈕，版本資訊在瀏覽時向 GitHub 取得，發新版
  不必改網頁；取不到時退回 Releases 頁連結。中英雙語。
- README 與 `README.zh-TW.md` 的「開始使用」改成先下載安裝檔，原始碼建置與安裝腳本保留為第二條路（等 `ui-language`
  併回 main 後才動，避免衝突）。

## 非目標

- 不做程式碼簽章：沒有憑證，第一次執行會出現 SmartScreen 警告，宣傳頁與 README 說明如何繼續。
- 不做自動更新：使用者自行下載新版覆蓋安裝。
- 不支援 Windows 以外的平台安裝檔：啟動器只有 Windows 版（設計文件 §8 程式本體、`desktop-launch` 規格）。
- 不在安裝檔附帶 HERDR，也不安裝或設定 WSL：HERDR 是外部前置條件（設計文件 §2.1 本機環境）。
- 不改 `scripts/install-desktop.ps1` 與 `desktop-launch` 的需求：原始碼流程維持原樣。
- 不改 `cockpit` 程式行為（版本號不顯示在儀表板上）。

## Capabilities

### New Capabilities

- `release-distribution`：Windows 安裝檔與 zip 的內容與安裝／解除安裝行為、release workflow 的觸發與產出物、版本一致性
  檢查、宣傳頁下載區塊。

### Modified Capabilities

（無）

## Impact

- 新增：`packaging/`（Inno Setup 腳本）、`.github/workflows/release.yml`、`LICENSE`、`CHANGELOG.md`。
- 修改：`site/index.html`、`site/i18n.js`、`site/site.js`、`site/site.css`（下載區塊）、`README.md`、`README.zh-TW.md`、
  `docs/handover.md`。
- 外部：GitHub Releases、GitHub Pages（`pages.yml` 不變，`site/**` 變動即部署）、GitHub REST API（宣傳頁在瀏覽器端讀
  最新 release）。
- 相依工具：GitHub Windows runner 上的 Inno Setup 6（release 時使用，不進 Cargo 相依）。
