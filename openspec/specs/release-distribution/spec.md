# release-distribution Specification

## Purpose

定義 Cockpit 對外發行的形式：Windows 安裝檔與免安裝 zip 的內容與安裝／解除安裝行為、推版本 tag 時自動建置並發布 GitHub
Release 的流程與版本一致性檢查，以及宣傳頁的下載區塊。決策背景見 ADR-0005 與 change `release-packaging` 的 proposal。

## Requirements

### Requirement: Windows 安裝檔

系統必須提供 Windows x64 安裝精靈，檔名為 `ai-cockpit-<版本>-x64-setup.exe`（`<版本>` 為不含 `v` 的版本號，例如
`0.1.0`），不需要原始碼、Rust 工具鏈或管理員權限即可安裝：

- **安裝範圍**：只裝給目前使用者，不提供改成所有使用者的選項；程式目錄預設為 `%LOCALAPPDATA%\Programs\AI Agent Cockpit\`。
- **內容**：程式目錄放 `cockpit.exe`、`cockpit-launch.exe`、`cockpit.example.toml` 與授權文字；兩個執行檔是同一次發布流程從該
  tag 的原始碼以 `cargo build --release -p cockpit --bins` 建出的。
- **資料目錄**：建立 `%LOCALAPPDATA%\ai-cockpit\`（已存在則沿用），作為捷徑與完成頁啟動的工作目錄；放在這裡的 `cockpit.toml` 依
  `cockpit-config` 的設定檔位置規則被讀到，沒有時以零設定模式執行，`cockpit.log` 也寫在這裡。
- **捷徑**：開始功能表建立「AI Agent Cockpit」捷徑；桌面捷徑同名，是預設勾選、可取消的選項。兩者目標皆為程式目錄的
  `cockpit-launch.exe`，不帶引數，工作目錄為資料目錄。
- **完成頁**：提供預設勾選的「啟動 AI Agent Cockpit」選項，以資料目錄為工作目錄啟動 `cockpit-launch.exe`；靜默安裝時不啟動
  （更新模式除外）。
- **登錄**：出現在 Windows 已安裝的應用程式清單，顯示名稱「AI Agent Cockpit」與版本號，可從清單解除安裝。
- **更新**：對已安裝的電腦再執行安裝檔（同版或較新版）即為覆蓋更新，沿用原程式目錄，清單中仍只有一筆。
- **執行中不覆寫**：程式目錄中的 `cockpit.exe` 或 `cockpit-launch.exe` 正在執行，或無法確認是否在執行時，安裝必須在複製
  任何檔案前停止並提示先關閉 Cockpit，不得覆寫或強制結束執行中的程式；靜默安裝時不顯示提示、以非 0 結束碼結束。更新模式
  下先等候最多 30 秒讓兩者自行結束，期限到時仍在執行（或無法確認）才依本條停止。
- **更新模式**：以命令列參數 `/COCKPITUPDATE=1` 加靜默參數啟動時為更新模式（供 `auto-update` 使用）。選項沿用上次安裝（例如
  是否建桌面捷徑）；安裝成功後以資料目錄為工作目錄、不帶引數啟動 `cockpit-launch.exe`。沒有這個參數時，一般安裝與一般靜默
  安裝的行為與本條其他項目相同。
- **權限**：以一般使用者執行時不出現 UAC 提權要求（以管理員身分執行時同樣只裝給目前使用者）。

#### Scenario: 一般安裝

- **GIVEN** 一台沒裝過 Cockpit 的 Windows x64 電腦，以一般使用者身分登入
- **WHEN** 執行安裝檔並全部採用預設值
- **THEN** 過程中不要求管理員權限；程式目錄有兩個執行檔、`cockpit.example.toml` 與授權文字；資料目錄存在；開始功能表與
  桌面各有一個「AI Agent Cockpit」捷徑，指向程式目錄的 `cockpit-launch.exe`、工作目錄為資料目錄

#### Scenario: 裝好的程式可以啟動

- **GIVEN** 已用安裝檔完成安裝，資料目錄沒有 `cockpit.toml`
- **WHEN** 以資料目錄為工作目錄執行程式目錄的 `cockpit.exe`
- **THEN** 後端以零設定模式啟動，`http://127.0.0.1:7770/` 回 200

#### Scenario: 不建桌面捷徑

- **WHEN** 安裝時取消勾選桌面捷徑
- **THEN** 只建立開始功能表捷徑

#### Scenario: 覆蓋更新

- **GIVEN** 已安裝
- **WHEN** 再執行一次安裝檔
- **THEN** 程式目錄不變、檔案為安裝檔內的版本，已安裝應用程式清單中仍只有一筆 Cockpit

#### Scenario: 執行中不覆寫

- **GIVEN** 程式目錄的 `cockpit.exe` 正在執行
- **WHEN** 執行安裝檔（非更新模式）
- **THEN** 安裝在複製檔案前停止並提示先關閉 Cockpit（靜默安裝時不提示、以非 0 結束），程式目錄的檔案未被修改，執行中的
  程式未被結束

#### Scenario: 更新模式等候程式結束

- **GIVEN** 已安裝，程式目錄的 `cockpit-launch.exe` 正在執行，並在 5 秒後自行結束
- **WHEN** 以更新模式執行安裝檔
- **THEN** 安裝檔等到程式結束後完成安裝（結束碼 0），之後以資料目錄為工作目錄啟動了 `cockpit-launch.exe`

#### Scenario: 更新模式逾時

- **GIVEN** 已安裝，程式目錄的 `cockpit.exe` 正在執行且超過 30 秒都沒有結束
- **WHEN** 以更新模式執行安裝檔
- **THEN** 安裝在複製檔案前停止、以非 0 結束，程式目錄的檔案未被修改，執行中的程式未被結束，也沒有啟動 `cockpit-launch.exe`

#### Scenario: 一般靜默安裝不啟動

- **WHEN** 不帶 `/COCKPITUPDATE=1` 以 `/VERYSILENT` 安裝
- **THEN** 安裝完成後沒有任何 `cockpit-launch.exe` 或 `cockpit.exe` 在執行

### Requirement: 解除安裝

系統必須能從 Windows 已安裝的應用程式清單或程式目錄的解除安裝程式移除 Cockpit：刪除程式目錄中安裝檔放入的檔案、
開始功能表與桌面捷徑，以及已安裝應用程式清單中的項目；資料目錄 `%LOCALAPPDATA%\ai-cockpit\` 不論是否為空都必須保留，
其中的設定、狀態與 log 不動。`cockpit.exe` 或 `cockpit-launch.exe` 正在執行，或無法確認是否在執行時，解除安裝必須在移除
任何檔案前停止並提示先關閉 Cockpit；靜默解除安裝時不顯示提示。

#### Scenario: 資料目錄為空也保留

- **GIVEN** 已安裝，資料目錄是空的
- **WHEN** 解除安裝
- **THEN** 程式目錄與捷徑消失，資料目錄仍在

#### Scenario: 解除安裝保留使用者資料

- **GIVEN** 已安裝，且資料目錄有使用者放的 `cockpit.toml`
- **WHEN** 解除安裝
- **THEN** 程式目錄的檔案與兩個捷徑消失、已安裝應用程式清單沒有 Cockpit，資料目錄與其中的 `cockpit.toml` 仍在

#### Scenario: 執行中不解除安裝

- **GIVEN** 程式目錄的 `cockpit.exe` 正在執行
- **WHEN** 解除安裝
- **THEN** 解除安裝停止並提示先關閉 Cockpit，檔案與捷徑都未被移除

### Requirement: 免安裝 zip

系統必須在每個 release 一併提供 `ai-cockpit-<版本>-x64.zip`，根目錄直接放 `cockpit.exe`、`cockpit-launch.exe`、
`cockpit.example.toml` 與授權文字（不包一層資料夾），不含其他檔案；兩個執行檔與同一 release 安裝檔內的相同。

#### Scenario: zip 內容

- **WHEN** 解開某個 release 的 zip
- **THEN** 根目錄恰好是上述四個檔案，兩個執行檔的 SHA-256 與同版安裝檔裝出的檔案相同

### Requirement: 推版本 tag 時發布 release

系統必須在推送 `v<主>.<次>.<修訂>` 形式的 tag（可帶 `-` 開頭的預發布後綴，例如 `v0.1.0-rc.1`）時自動執行發布流程：

1. **版本一致性**：tag 去掉 `v` 與預發布後綴後的版本必須等於 `cockpit` crate 的版本，否則流程失敗、不建 release。
2. **品質 gate**：`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test --workspace` 全部通過才繼續。
3. **建置與打包**：建出安裝檔、zip 與 `SHA256SUMS.txt`（列出前兩者的 SHA-256）。
4. **冒煙測試**：在建置環境靜默安裝、驗證「Windows 安裝檔」的安裝結果與「裝好的程式可以啟動」，驗證更新模式的「等候程式結束」
   與「逾時」兩種結果，再靜默解除安裝並驗證「解除安裝」的移除結果；任一不符則流程失敗、不建 release。
5. **release 說明**：取自 `CHANGELOG.md` 中標題以 `## [<版本>]` 開頭的段落（`<版本>` 為去掉預發布後綴的版本，所以
   `v0.1.0-rc.1` 取 `0.1.0` 段落）；找不到該段落時流程失敗。
6. **發布**：先建草稿 release 並上傳三個產出物，確認三個都在後，不帶預發布後綴的 tag 才轉為正式公開並成為最新版；帶後綴的
   tag 停在標示為預發布的草稿，供人工檢查後刪除。

另必須能手動觸發同一流程的第 1 到 4 步（版本取 `cockpit` crate 的版本），只產出可下載的建置產物，不建 release。

#### Scenario: 正式版

- **GIVEN** `cockpit` crate 版本為 `0.1.0`，`CHANGELOG.md` 有 `0.1.0` 段落
- **WHEN** 推送 tag `v0.1.0`
- **THEN** 產生公開的 release `v0.1.0`，標為最新版，附 `ai-cockpit-0.1.0-x64-setup.exe`、`ai-cockpit-0.1.0-x64.zip`、
  `SHA256SUMS.txt`，說明為 `CHANGELOG.md` 的 `0.1.0` 段落

#### Scenario: 預發布演練

- **WHEN** 推送 tag `v0.1.0-rc.1`
- **THEN** 產生標示為預發布的草稿 release，附同樣三個產出物（檔名中的版本為 `0.1.0`），不公開、不成為最新版

#### Scenario: 版本不一致

- **GIVEN** `cockpit` crate 版本為 `0.1.0`
- **WHEN** 推送 tag `v0.2.0`
- **THEN** 流程失敗並指出 tag 與 crate 版本不一致，沒有建立任何 release

#### Scenario: 冒煙測試失敗

- **WHEN** 安裝後缺少任一預期檔案或捷徑、更新模式沒有等候或沒有重新啟動、或解除安裝後仍殘留
- **THEN** 流程失敗，沒有建立任何 release

### Requirement: 宣傳頁下載區塊

宣傳頁必須有「下載」區塊，並可從頁首導覽與首屏的主要按鈕直接到達：

- **最新版資訊**：瀏覽時向 GitHub 取得最新的正式 release（不含草稿與預發布），顯示版本號與發布日期，下載按鈕直接連到
  該版的安裝檔與 zip。發布新版後不需修改或重新部署宣傳頁。
- **取不到時**：無法取得（網路錯誤、超過 API 速率限制、尚無 release、找不到預期檔名）時，不顯示版本號，下載按鈕改連 GitHub
  Releases 頁面，頁面其餘部分照常可用。
- **說明**：列出系統需求（Windows 10／11 x64、HERDR、`cockpit-launch` 需要 Chrome 或 Edge），說明未簽章安裝檔第一次執行時
  Windows SmartScreen 的提示與繼續方式，並保留從原始碼建置的連結。
- **雙語**：所有固定文字有英文與繁體中文，跟隨宣傳頁現有的語言切換。

#### Scenario: 顯示最新版

- **GIVEN** 最新的正式 release 是 `v0.1.0`
- **WHEN** 開啟宣傳頁
- **THEN** 下載區塊顯示 `v0.1.0` 與其發布日期，安裝檔按鈕連到 `ai-cockpit-0.1.0-x64-setup.exe` 的下載網址

#### Scenario: 取不到 release 資訊

- **GIVEN** GitHub API 回應錯誤或請求失敗
- **WHEN** 開啟宣傳頁
- **THEN** 下載區塊不顯示版本號，下載按鈕連到 GitHub Releases 頁面，主控台以外沒有錯誤訊息打斷頁面

#### Scenario: 切換語言

- **WHEN** 在宣傳頁切換成繁體中文
- **THEN** 下載區塊的標題、按鈕、系統需求與 SmartScreen 說明都換成繁體中文，版本號與日期維持顯示

### Requirement: 自動更新客戶端契約

已發出的 `cockpit-launch`（capability `auto-update`）依賴下列四項發布產物的性質。啟動器一經發出就無法修改，所以之後每個版本的
安裝檔與發布流程都必須維持這四項，不得變更：

1. **資產名稱**：安裝檔為 `ai-cockpit-<版本>-x64-setup.exe`、雜湊檔為 `SHA256SUMS.txt`（`<版本>` 為不含 `v` 的
   `<主>.<次>.<修訂>`），兩者都是 tag `v<版本>` 的 release 資產。
2. **`SHA256SUMS.txt` 格式**：每行為 `<64 位小寫十六進位>␠␠<檔名>`（雜湊與檔名之間恰好兩個空白），以 LF 換行、沒有 CR，
   檔案開頭沒有 BOM，檔名前沒有 `*`（二進位模式標記）。
3. **更新模式參數**：安裝檔接受 `/SILENT /SUPPRESSMSGBOXES /NORESTART /NOCANCEL /COCKPITUPDATE=1 /LOG=<路徑>`，並依
   「Windows 安裝檔」的更新模式運作。
4. **解除安裝程式**：安裝檔在程式目錄放 `unins000.exe`（啟動器以它判定「由安裝檔安裝」）。

發布流程必須在產生 `SHA256SUMS.txt` 後檢查第 2 項，不符即流程失敗；冒煙測試檢查第 3、4 項。

#### Scenario: SHA256SUMS 格式不符時發布流程失敗

- **GIVEN** 發布流程產生的 `SHA256SUMS.txt` 有一行不是 `<64 位小寫十六進位>␠␠<檔名>`，或含 CR，或開頭有 BOM
- **WHEN** 發布流程執行到雜湊檔檢查
- **THEN** 流程失敗並指出不符之處，沒有建立任何 release

#### Scenario: 安裝後有解除安裝程式

- **GIVEN** 一台沒裝過 Cockpit 的電腦
- **WHEN** 以安裝檔完成安裝
- **THEN** 程式目錄有 `unins000.exe`

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
