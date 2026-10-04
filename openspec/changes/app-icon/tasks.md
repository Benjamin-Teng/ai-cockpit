# tasks：app-icon

> 執行路徑：直接 `opsx:apply`｜理由：小而線性（產生器 → 資產 → 建置嵌入 → 安裝檔與冒煙測試 → 發版），每步依賴上一步、
> 沒有可平行的獨立模組；唯一高風險面是建置期新相依，在 3.1 以 gate 與實際執行檔檢查收斂。

基線：開工前在分支 `app-icon`（自最新 `main`）記錄 `cargo test --workspace` 通過數。每個改 Rust 的 task 結尾跑
`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test --workspace`；改 `.md` 跑
`markdownlint-cli2 "**/*.md"`；0 error 才算完成。審查依專案 memory 由 Opus subagent 進行（視同 Codex review）。

## 1. 查證

- [x] 1.1 查證 design「外部精確資訊」的兩項待查證（Inno 6.7.1 `SetupIconFile` 與 PNG 壓縮圖示項目、`ExtractIconExW` 的
  `nIconIndex = -1`），附一手來源 URL 與日期寫回 design。驗收＝該節沒有「待查證」，或註明查不到的原因與替代做法。

## 2. 母檔與產生器

- [x] 2.1 `packaging/icon/app-icon.svg`（提案 B 定稿）與 `packaging/icon/gen-icon.js`（design D1）：產出 `app.ico`（16、20、
  24、32、40、48、64 為 DIB，256 為 PNG）、`cockpit/assets/icons/icon-192.png`、`icon-512.png`、`site/img/icon-192.png`；
  刪除 `cockpit/assets/gen-icons.py`。檔頭寫用法與 Chrome 路徑的覆寫方式。驗收＝產生器 exit 0；以獨立腳本解析 `app.ico`
  的目錄項目，尺寸清單與格式符合上述；`cargo test -p cockpit --test pwa --test http` 通過（PNG 尺寸與 CRC）。
- [x] 2.2 小尺寸判斷（design D2）：把 16、24、32 px 的實際像素放大截圖給使用者看；需要時加 `app-icon-small.svg` 並重跑
  產生器。驗收＝使用者確認小尺寸可接受（回覆記入本 task）。
  使用者 2026-10-04 選簡化版（16–24 px 用 `app-icon-small.svg`）；比較圖
  `docs/research/2026-10-04/icon-concepts/small-sizes-main.png`、`small-sizes-simplified.png`。

## 3. 執行檔資源

- [x] 3.1 `cockpit/Cargo.toml` 加 `[build-dependencies] winresource = "0.1.31"`，新增 `cockpit/build.rs`（design D3）。
  驗收＝`cargo build --release -p cockpit --bins` 後，PowerShell 讀兩個執行檔的 `VersionInfo`：`ProductName`、
  `FileDescription` 為「AI Agent Cockpit」、`ProductVersion` 等於 crate 版本；`ExtractIconExW(path, -1, …)` ≥ 1；
  `cargo tree -p cockpit -e build` 看得到 winresource；三項 Rust gate 全過。

## 4. 安裝檔、冒煙測試、介面

- [x] 4.1 `packaging/ai-cockpit.iss` 加 `SetupIconFile=icon\app.ico`；`packaging/smoke-test.ps1` 加 design D5 的版本資訊與圖示
  數量檢查（兩個執行檔＋安裝檔）。驗收＝推 `app-icon` 分支後以 `gh workflow run release.yml --ref app-icon` 無法觸發（只認
  預設分支），改在 6.1 rc 演練驗；本 task 先以 PowerShell 語法檢查（`[scriptblock]::Create((Get-Content -Raw ...))`）通過。
- [x] 4.2 `cockpit/assets/index.html` 加 favicon、`notify.js` 通知帶 `icon`（design D6）；Rust 測試新增「`index.html` 宣告
  `rel="icon"` 指向 `/icons/icon-192.png`」。驗收＝新測試先紅後綠；`cargo build -p cockpit --example ui_preview` 後
  `cargo test -p cockpit --example ui_preview` 通過；三項 Rust gate 全過。

## 5. 文件與收尾

- [x] 5.1 `CHANGELOG.md` 新增 `## [Unreleased]` 的 Changed 段落（新圖示、執行檔版本資訊）；README（中英）原始碼建置段補
  「Windows 建置需要 Windows SDK（`rc.exe`）」。驗收＝markdownlint 0 error；中英內容一致。
- [x] 5.2 全 gate 與審查：handover 第 1 節「全 gate」整串附輸出；整支分支交 Opus subagent 依 spec 與 design 做 diff 審查，
  findings 實測後處理。驗收＝審查無未處理的 Critical／Important；本機 `install-desktop.ps1` 重裝後桌面捷徑顯示新圖示
  （使用者目視確認）。

## 6. 發布 v0.1.2 與自動更新真機驗證

- [ ] 6.1 squash 併回 `main` 並推送；升版 0.1.2（`cockpit/Cargo.toml`、`Cargo.lock`、`CHANGELOG.md` 改 `## [0.1.2] - Unreleased`）
  後推 `v0.1.2-rc.1`。驗收＝release workflow 全綠、冒煙測試 log 有本 change 新增的檢查且 PASS。
- [ ] 6.2 填 CHANGELOG 日期、推 `v0.1.2`（推 tag 前問使用者）；`gh release delete v0.1.2-rc.1 --cleanup-tag`（刪 tag 前問使用者）。
  驗收＝`releases/latest` 為 v0.1.2、三個資產齊全。
- [ ] 6.3 自動更新真機驗證（design Migration Plan 第 3 步，需使用者操作）：備份桌面捷徑 → 安裝 GitHub 上的 v0.1.1 → 從捷徑
  啟動 → 詢問更新到 0.1.2 → 按「是」→ 重新開啟。驗收＝重開後程式目錄 `cockpit-launch.exe` 的 `ProductVersion` 為 0.1.2、
  使用者目視捷徑為新圖示；結果記入 `docs/research/2026-10-04/auto-update-e2e.md`；收尾還原捷徑。
