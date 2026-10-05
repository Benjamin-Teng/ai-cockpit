# design：app-icon

## Context

動機見 proposal.md「Why」。現況：

- `cockpit` crate 有兩個 bin（`cockpit`、`cockpit-launch`），沒有 `build.rs`，執行檔不含任何 Windows 資源。
- 網頁圖示由 `cockpit/src/http.rs` 的 `/icons/{file}` 以 `include_bytes!` 提供兩張 PNG（192、512），`cockpit/tests/pwa.rs`、
  `tests/http.rs` 驗 PNG 簽章、尺寸與 IHDR CRC。PNG 由 change 1b 的 `cockpit/assets/gen-icons.py`（純 stdlib 畫圓與斜線）產生。
- 安裝檔 `packaging/ai-cockpit.iss` 已設 `UninstallDisplayIcon={app}\cockpit-launch.exe`，捷徑目標是 `cockpit-launch.exe`；兩者
  在執行檔帶圖示後自然顯示新圖示，`.iss` 只需補安裝檔自身的 `SetupIconFile`。
- 介面 `index.html` 沒有 `rel="icon"`；桌面通知（`notify.js`）建立時只帶 `body`、`tag`、`renotify`。
- 圖示設計：使用者選定提案 B（`docs/research/2026-10-04/icon-concepts/b-horizon.svg`），視覺語言依設計文件 §8.3 與
  `cockpit/assets/app/style.css` 的色票。

## Goals / Non-Goals

**Goals:**

- 一份向量母檔 → 所有圖示產物，產生器可重跑；建置期只讀已提交的 `.ico`，不依賴瀏覽器。
- 16 px 仍可辨識（工作列小圖示、檔案總管清單）。
- 發布流程（`release.yml`）不需要新增步驟即可帶上圖示。

**Non-Goals:**

- 不改 `/icons/{file}` 路由與 manifest 結構（只換 PNG 內容）；不新增 `/favicon.ico` 路由。
- 不做安裝精靈的 `WizardImageFile`／`WizardSmallImageFile`（精靈內的側邊與右上角插圖）。

## Decisions

### D1 產生器：headless Chrome 渲染＋Node 組 `.ico`，不加任何套件

`packaging/icon/gen-icon.js`（Node，只用內建模組）以 headless Chrome（`--headless=new --dump-dom`）載入一個本機 HTML，
頁內把 SVG 畫進各尺寸的 `<canvas>`，把 PNG（`toDataURL`）與原始 RGBA（`getImageData`）以 base64 寫進 DOM；Node 讀回後：

- 寫出網頁用 PNG：`cockpit/assets/icons/icon-192.png`、`icon-512.png`、`site/img/icon-192.png`。
- 組 `packaging/icon/app.ico`：16、20、24、32、40、48、64 用 32 位元 BGRA DIB（含 AND mask），256 用 PNG 壓縮項目。
  小尺寸用 DIB 是 Windows 圖示的慣例相容做法（舊版 API 與部分工具不讀 PNG 項目）；256 用 PNG 控制檔案大小。

替代方案：`sharp`／`png-to-ico` 等 npm 套件（要新增相依，違反「先問、少相依」），Python Pillow（同理）；改寫舊的
`gen-icons.py` 自己畫（無法渲染 SVG，母檔與產物會分家）。Chrome 是本機與驗收腳本既有的工具。

Chrome 版本不同時抗鋸齒可能有像素級差異，所以產物提交進 repo、建置只讀產物；重跑產生器只在改母檔時。

### D2 小尺寸用簡化母檔

`packaging/icon/app-icon.svg` 為主母檔（即提案 B 定稿），`app-icon-small.svg` 為 16–24 px 用的簡化版：拿掉地平線白線、
放大圓盤、機身符號只留加粗的提示字元，讓 16 px 下仍是「圓＋上下分色＋琥珀記號」。產生器依尺寸選母檔（≤24 用 small）。是否真的需要
small 版在實作時以 16、24 px 實際像素截圖判斷；不需要就只留一份母檔，spec 允許兩者。

### D3 執行檔資源：`build.rs`＋`winresource`

`cockpit/build.rs` 在 `CARGO_CFG_TARGET_OS == "windows"` 時：`WindowsResource::new()` → `set_icon("../packaging/icon/app.ico")`
→ `set("ProductName", "AI Agent Cockpit")`、`set("FileDescription", "AI Agent Cockpit")`、
`set("LegalCopyright", "Copyright (c) 2026 Benjamin-Teng")` → `compile()`；`compile()` 失敗即 panic 讓建置失敗
（發布的執行檔不得默默少了圖示）。`cargo:rerun-if-changed` 指向 `build.rs` 與 `.ico`。版本欄位用 winresource 預設
（取 `CARGO_PKG_VERSION`），所以 tag 版本＝crate 版本＝執行檔版本，不必另外同步。

依查證（task 1.1），MSVC 工具鏈下 winresource 以 `rc.exe` 編譯並用 `cargo:rustc-link-arg` 連結，會套到本 crate 所有
連結產物（兩個 bin、tests、examples）；tests 與 examples 多帶圖示不影響行為。判斷目標平台必須用環境變數，不能用
`#[cfg(target_os)]`（build script 在 host 上編譯）。

替代方案：`embed-resource`（只編資源檔，版本資訊要自己寫 `VERSIONINFO`）；使用者選 winresource。

### D4 安裝檔

`.iss` 的 `[Setup]` 加 `SetupIconFile=icon\app.ico`（相對於 `.iss` 所在的 `packaging/`）。`release.yml` 的 `iscc` 呼叫不變。

### D5 驗證方式

- 冒煙測試（`smoke-test.ps1`，CI 上）：安裝後讀兩個執行檔的 `VersionInfo`，斷言 `ProductName`／`FileDescription` 為
  「AI Agent Cockpit」、`ProductVersion` 與 `FileVersion` 等於 `-Version`；捷徑的 `IconLocation` 未另外指定（取自目標）；以 `shell32!ExtractIconExW(path, -1, …)` 取圖示數量，斷言
  兩個執行檔與安裝檔都 ≥ 1。後者是「有沒有圖示」的直接證據，不只靠版本資訊推論。
- Rust 測試：`index.html` 宣告 `rel="icon"` 指向 `/icons/icon-192.png`（既有 `pwa.rs` 已驗 PNG 尺寸）。
- 人工：本機 `install-desktop.ps1` 重裝後看桌面捷徑與工作列；v0.1.2 發布後的自動更新真機驗證（Migration Plan）。

### D6 介面

`index.html` 加 `<link rel="icon" type="image/png" href="/icons/icon-192.png">`；`notify.js` 建立通知時加
`icon: "/icons/icon-192.png"`。宣傳頁沿用 `site/img/icon-192.png` 檔名，只換內容。

## Risks / Trade-offs

- [本機或 CI 沒有 Windows SDK 的 `rc.exe` → Windows 建置失敗] → 本機與 `windows-latest` 已確認有；README 的原始碼建置
  段補一句需求。故意不降級成「沒有就略過」，避免發布出沒圖示的版本。
- [Windows 圖示快取讓捷徑仍顯示舊的空白圖示] → 人工驗證時以新安裝或 `ie4uinit.exe -show` 重新整理；自動驗證不靠畫面。
- [Chrome 升版造成重跑產生器的產物有像素差] → 產物提交、只在改母檔時重跑，diff 中可見。
- [改了母檔卻沒重跑產生器，產物與母檔分家，建置與 CI 仍綠] → 流程規範：改母檔與重跑產生器同一個 commit（母檔與
  產生器檔頭都有寫）。不做雜湊防呆：工作目錄的 SVG 換行依平台不同，要正規化才能比對，成本高於收益（審查 Minor 6）。
- [小尺寸辨識度] → D2 以實際像素截圖判斷，提交前附 16／24／32 px 截圖給使用者確認。

## Migration Plan

1. 實作、gate、審查後 squash 併回 `main`；本機以 `install-desktop.ps1` 重裝確認捷徑圖示。
2. 升版 0.1.2（`cockpit/Cargo.toml`、`Cargo.lock`、`CHANGELOG.md`），推 `v0.1.2-rc.1` 演練，CI 冒煙測試全綠後推 `v0.1.2`。
3. 自動更新真機驗證：v0.1.2 公開後，在使用者電腦安裝 GitHub 上的 v0.1.1（先備份桌面捷徑），從捷徑啟動 → 應詢問更新到
   0.1.2 → 按「是」→ 重新開啟後執行檔版本為 0.1.2、捷徑換成新圖示。先裝 v0.1.1 再發 v0.1.2 的順序不可顛倒：v0.1.1
   在 v0.1.2 公開前啟動會記下「沒有新版」並節流 24 小時。
4. 回退：圖示問題不影響功能；有問題發 v0.1.3 修正。

## 外部精確資訊（查證日期 2026-10-04）

- `winresource` 最新穩定版 0.1.31（crates.io API `https://crates.io/api/v1/crates/winresource`，2026-03-16 更新）；
  預設 feature `toml`；公開 API `new`、`set`、`set_icon`、`compile` 等與預設 `FileVersion`／`ProductVersion`
  取 `CARGO_PKG_VERSION`、`ProductName`／`FileDescription` 取套件名，均讀自 crate 原始碼 `lib.rs`
  （`https://static.crates.io/crates/winresource/winresource-0.1.31.crate`）；README 說明以 `CARGO_CFG_TARGET_OS` 判斷目標。
- 本機 Windows SDK `rc.exe`：`C:\Program Files (x86)\Windows Kits\10\bin\10.0.26100.0\x64\rc.exe`。
- Inno Setup `SetupIconFile`（<https://jrsoftware.org/ishelp/topic_setup_setupiconfile.htm>，2026-10-04 讀取）：檔案相對於
  安裝腳本的來源目錄解析；官方建議至少含 16、32、48、64、256；**文件沒有提到 PNG 壓縮項目**。查不到明文支援，改以實證：
  6.1 的 rc 演練中由冒煙測試對安裝檔做 `ExtractIconExW` 圖示數量檢查，並人工看安裝檔圖示；不成立時 256 改用 DIB。
- `ExtractIconExW`（<https://learn.microsoft.com/en-us/windows/win32/api/shellapi/nf-shellapi-extracticonexw>，2026-10-04
  讀取）：`nIconIndex` 為 -1 且 `phiconLarge`、`phiconSmall` 皆為 NULL 時回傳檔案內圖示總數；執行檔或 DLL 回傳
  `RT_GROUP_ICON` 資源數。Shell32.dll。錯誤時回傳 `UINT_MAX`（文件舉的例子是非 -1 的情況），所以冒煙測試以「≥ 1 且不等於
  `UINT_MAX`」判定。
