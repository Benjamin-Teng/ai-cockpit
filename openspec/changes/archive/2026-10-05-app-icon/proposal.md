# proposal：app-icon

## Why

`cockpit.exe` 與 `cockpit-launch.exe` 沒有內嵌 Windows 圖示，安裝後的桌面捷徑、開始功能表、工作列、安裝檔與「已安裝的應用程式」
清單都顯示 Windows 預設的空白程式圖示；網頁 manifest 用的 `icon-192.png`／`icon-512.png` 是 change 1b 的暫用圖（白圓加綠色
斜線），配色與現在的介面不一致。使用者在桌面上第一眼看到的就是圖示，v0.1.1 已發布、有自動更新，這是讓一般使用者認得出
產品的最小一步，也正好以 v0.1.2 實際驗證 v0.1.1 的自動更新。

## What Changes

- 新圖示採使用者 2026-10-04 從五個提案中選定的「姿態儀」（`docs/research/2026-10-04/icon-concepts/b-horizon.svg`）：深色
  圓形儀表、青色天空與深藍地面以地平線分開並略為傾斜，中央琥珀色的機身符號以終端機提示字元 `›` 構成；色票取自介面
  `style.css`。
- 新增一份 SVG 母檔與產生器：從母檔產生 Windows `.ico`（多尺寸）與網頁用 PNG，產物提交進 repo，建置時不需要瀏覽器；
  取代舊的 `cockpit/assets/gen-icons.py`。
- `cockpit.exe`、`cockpit-launch.exe` 內嵌圖示與版本資訊（產品名稱「AI Agent Cockpit」、版本取自 `cockpit` crate）；捷徑、
  工作列、檔案總管、「已安裝的應用程式」清單（`UninstallDisplayIcon` 已指向 `cockpit-launch.exe`）因此顯示新圖示。
- 安裝檔（`ai-cockpit-<版本>-x64-setup.exe`）本身與安裝精靈視窗使用新圖示。
- 介面網頁：manifest 的兩張 PNG 換新圖，`index.html` 加 favicon，桌面通知帶圖示。宣傳頁 favicon 換新圖。
- 冒煙測試檢查安裝後兩個執行檔的版本資訊（產品名稱與版本號），作為資源確實內嵌的證據。
- 以 v0.1.2 發布；發布後在 v0.1.1 安裝上確認自動更新詢問並完成更新（取代先前以假伺服器演練的那一段，這次是真的
  GitHub 最新版）。

## 非目標

- 介面版面、配色或元件外觀不變（設計文件 §8.3 畫面）；只換圖示資產。
- 程式碼簽章、系統匣圖示（Tauri 已確認不包，ADR-0005 補充段）、macOS／Linux 圖示格式。
- 依 agent 狀態動態變化的工作列圖示或徽章（overlay icon）。
- `install-desktop.ps1` 建的桌面捷徑不另外指定圖示：它指向同一支 `cockpit-launch.exe`，重新安裝後自然帶上內嵌圖示。

## Capabilities

### New Capabilities

（無）

### Modified Capabilities

- `release-distribution`：新增需求「應用程式圖示與版本資訊」——安裝檔與兩個執行檔必須帶應用程式圖示與版本資訊。

## Impact

- 建置：`cockpit/build.rs`（新增）、`cockpit/Cargo.toml` 新增建置期相依 `winresource`（使用者 2026-10-04 同意）；Windows
  建置需要 Windows SDK 的 `rc.exe`（本機與 GitHub `windows-latest` runner 皆有）。非 Windows 目標不嵌入。
- 資產：`packaging/icon/`（SVG 母檔、產生器、`.ico`）、`cockpit/assets/icons/icon-192.png`／`icon-512.png`、`site/img/`
  的 favicon；刪除 `cockpit/assets/gen-icons.py`。
- 打包：`packaging/ai-cockpit.iss`（`SetupIconFile`）、`packaging/smoke-test.ps1`（版本資訊檢查）。
- 介面：`cockpit/assets/index.html`（favicon）、`cockpit/assets/app/notify.js`（通知圖示）；後端 `/icons/{file}` 路由不變。
- 文件：`CHANGELOG.md` 0.1.2 段落；圖示提案留在 `docs/research/2026-10-04/icon-concepts/`。
