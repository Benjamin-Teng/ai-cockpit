# release-distribution（修改）

## ADDED Requirements

### Requirement: 應用程式圖示與版本資訊

系統必須讓發布的程式在 Windows 上以同一個應用程式圖示呈現，並帶可辨識的版本資訊：

- **執行檔**：`cockpit.exe` 與 `cockpit-launch.exe` 必須內嵌應用程式圖示（至少含 16、24、32、48、256 像素），以及版本資訊：
  產品名稱與檔案描述皆為「AI Agent Cockpit」，檔案版本與產品版本等於該次發布的版本號（`X.Y.Z`）。
- **捷徑與清單**：安裝建立的開始功能表與桌面捷徑、Windows 已安裝的應用程式清單，必須顯示這個圖示（取自 `cockpit-launch.exe`）。
- **安裝檔**：`ai-cockpit-<版本>-x64-setup.exe` 檔案本身與安裝精靈視窗必須使用同一個圖示。
- **網頁介面**：Cockpit 介面必須以同一圖示作為頁面圖示（favicon）與 manifest 圖示（192、512 像素），桌面通知必須帶這個圖示。
- **來源**：上述所有圖示必須由 repo 內的向量母檔產生（小尺寸可用同一設計的簡化版母檔），產物提交進 repo；建置執行檔不得
  需要瀏覽器或其他圖像工具。

#### Scenario: 安裝後執行檔帶版本資訊

- **GIVEN** 用版本 `X.Y.Z` 的安裝檔完成安裝
- **WHEN** 讀取程式目錄 `cockpit.exe` 與 `cockpit-launch.exe` 的檔案版本資訊
- **THEN** 兩者的產品名稱與檔案描述皆為「AI Agent Cockpit」，產品版本為 `X.Y.Z`

#### Scenario: 執行檔帶應用程式圖示

- **GIVEN** 以 `cargo build --release -p cockpit --bins` 建出的兩個執行檔
- **WHEN** 讀取執行檔的圖示資源
- **THEN** 兩者都含應用程式圖示，且與 `packaging/icon/app.ico` 相同（該檔含 16、24、32、48、256 像素的影像）

#### Scenario: 捷徑顯示應用程式圖示

- **GIVEN** 已用安裝檔完成安裝
- **WHEN** 查看開始功能表與桌面的「AI Agent Cockpit」捷徑
- **THEN** 捷徑顯示應用程式圖示，不是 Windows 預設的空白程式圖示

#### Scenario: 介面頁面圖示

- **GIVEN** Cockpit 後端執行中
- **WHEN** 瀏覽器載入 Cockpit 介面 `GET /`
- **THEN** 頁面宣告的圖示網址回 200 且內容為應用程式圖示的 PNG
