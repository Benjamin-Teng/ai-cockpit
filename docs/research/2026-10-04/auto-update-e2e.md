# 自動更新端到端真機驗證（v0.1.1 發版前）

CI 的冒煙測試（88 項）驗不到兩件事：使用者桌面上「下載 → 驗證 → 交棒 → 靜默安裝 → 重新開啟」整段是否順利，以及
SmartScreen／UAC 有沒有跳出。這份是在使用者電腦上補驗的步驟。規格見 `openspec/specs/auto-update/`，
安裝檔契約見 `openspec/specs/release-distribution/`。

## 已確認（2026-10-03）

- **查詢與判定**：裝好 `v0.1.1-rc.1` 後以假伺服器啟動，啟動器送出 `GET /api/latest`（User-Agent `ai-cockpit/0.1.1`），
  `cockpit.update.json` 記為 `update available: 0.1.2`，並跳出更新詢問視窗。
- **不需提權**：`v0.1.1-rc.1` 安裝檔的 manifest 為 `requestedExecutionLevel level="asInvoker"`（`.iss` 為
  `PrivilegesRequired=lowest`），以 `CreateProcess` 啟動對一般使用者與管理員都不需要提權，不必另建一般使用者帳號實測。

## 結果（2026-10-04）：通過

以 `v0.1.1-rc.1` 安裝檔（SHA-256 `c9a6007a…4aad`，與草稿的 `SHA256SUMS.txt` 一致）照下列步驟執行：

- 20:52:53 啟動器送出 `GET /api/latest`（UA `ai-cockpit/0.1.1`），`cockpit.update.json` 記為 `update available: 0.1.2`，跳出詢問。
- 使用者按「是」：22:24:01 依序 `GET /dl/v0.1.2/SHA256SUMS.txt`、`GET /dl/v0.1.2/ai-cockpit-0.1.2-x64-setup.exe`（UA 同上）；
  22:24:04 新的 `cockpit.exe` 從 `%LOCALAPPDATA%\Programs\AI Agent Cockpit\` 啟動。
- 使用者觀察第 7 步四項皆符合預期：沒有 SmartScreen、沒有 UAC、安裝進度視窗自己關閉、Cockpit 自己重新開啟。
- 重新開啟後 `cockpit.update.json` 仍是 20:52:53 那筆（24 小時節流生效，重開時沒有再查）。
- 第 8 步的安裝檔 `setup.log` 看不到：重新開啟的啟動器依 auto-update design D6 清掉 `%TEMP%\ai-cockpit-update\` 下其他程序的
  子資料夾，舊 PID 資料夾連同 log 一起刪了（資料夾修改時間 22:24:04，內容為空）。這是設計行為；要保留 log 得在按「是」之後、
  Cockpit 重開之前另外複製。
- 演練中假伺服器曾因 agent 背景工作 30 分鐘時限被停掉，使用者按「是」前已重啟；長時間等待時背景工作要給較長時限。

## 真實更新 v0.1.1 → v0.1.2（2026-10-05）：通過

change `app-icon` task 6.3。這次不用假伺服器，啟動器直接查 GitHub 的 `releases/latest`（v0.1.2 公開後才安裝 v0.1.1，
避免 v0.1.1 先記下「沒有新版」而節流 24 小時）。

- 安裝：GitHub 上 v0.1.1 的安裝檔（SHA-256 `5ea4e3b0…e946`，與 v0.1.1 的 `SHA256SUMS.txt` 一致），`/VERYSILENT` 結束碼 0；
  安裝後 `cockpit-launch.exe` 沒有版本資訊（v0.1.1 尚未內嵌，作為更新前的對照）。刪除 `cockpit.update.json` 後由使用者
  雙擊桌面捷徑啟動。
- 使用者觀察：詢問更新到 0.1.2 → 按「是」→ 沒有 SmartScreen、沒有 UAC、安裝進度視窗自己關閉、Cockpit 自己重新開啟，
  底列顯示 `v0.1.2`，捷徑換成新圖示（使用者回報「一切正常」）。
- 佐證：`cockpit.update.json` 為 `update available: 0.1.2`（18:46:26 查詢）；18:46:54 新的 `cockpit.exe` 從
  `%LOCALAPPDATA%\Programs\AI Agent Cockpit\` 啟動；兩個執行檔 `ProductName`「AI Agent Cockpit」、`ProductVersion`
  0.1.2；`GET /` 帶 `<meta name="cockpit-version" content="0.1.2">`。執行檔的修改時間是建置時間（Inno 保留內嵌檔案
  的時間戳），不能用來判斷是否覆寫。

## 步驟：按「是」之後的整段

需要使用者在電腦前操作第 7 步。約 5–10 分鐘。指令在 repo 根目錄、PowerShell 7 執行。

1. **前置**：Cockpit 沒在執行（`Get-Process cockpit, cockpit-launch` 沒有結果；開著就關視窗等 10 秒）。
2. **備份桌面捷徑**（安裝檔會取代同名捷徑）：

   ```powershell
   $bk = Join-Path $env:TEMP 'cockpit-e2e-backup'; New-Item -ItemType Directory -Force $bk | Out-Null
   $lnk = Join-Path ([Environment]::GetFolderPath('Desktop')) 'AI Agent Cockpit.lnk'
   Copy-Item $lnk $bk -Force
   ```

3. **取得要先裝的版本**（rc 草稿或已發布的版本都可）：

   ```powershell
   gh release download v0.1.1-rc.1 -R Benjamin-Teng/ai-cockpit -D $env:TEMP\rc011 --clobber
   ```

4. **靜默安裝**：

   ```powershell
   $setup = "$env:TEMP\rc011\ai-cockpit-0.1.1-x64-setup.exe"
   Start-Process $setup -ArgumentList '/VERYSILENT','/SUPPRESSMSGBOXES','/NORESTART' -Wait
   ```

5. **啟動假更新伺服器**（另開一個視窗，驗完再關）：

   ```powershell
   node docs/research/2026-10-04/fake-update-server.js "$env:TEMP\rc011\ai-cockpit-0.1.1-x64-setup.exe"
   ```

6. **以捷徑同樣的方式啟動**（工作目錄＝資料目錄、不帶引數），並指向假伺服器、清掉 24 小時節流紀錄：

   ```powershell
   $env:COCKPIT_UPDATE_API_URL = 'http://127.0.0.1:7790/api/latest'
   $env:COCKPIT_UPDATE_DOWNLOAD_BASE = 'http://127.0.0.1:7790/dl'
   $data = "$env:LOCALAPPDATA\ai-cockpit"
   Remove-Item "$data\cockpit.update.json" -ErrorAction SilentlyContinue
   Start-Process "$env:LOCALAPPDATA\Programs\AI Agent Cockpit\cockpit-launch.exe" -WorkingDirectory $data
   ```

7. **使用者操作與觀察**：更新詢問視窗可能在其他視窗後面，從工作列找「AI Agent Cockpit」。按「是」，記下：

   | 觀察項目 | 預期 |
   |---|---|
   | SmartScreen「Windows 已保護您的電腦」 | 不出現 |
   | UAC「是否要允許此 App 對您的裝置進行變更」 | 不出現 |
   | 安裝進度視窗 | 短暫出現後自己關閉 |
   | Cockpit 視窗 | 約 30 秒內自己重新開啟 |

8. **佐證**：

   ```powershell
   Get-Content "$env:TEMP\cockpit-e2e\server.log"   # 應有 GET /dl/v0.1.2/SHA256SUMS.txt 與 setup.exe，UA 為 ai-cockpit/0.1.1
   Get-ChildItem "$env:TEMP\ai-cockpit-update" -Recurse -Filter setup.log |
     Sort-Object LastWriteTime | Select-Object -Last 1 | Get-Content -Tail 40   # 安裝檔 log：Update mode: waited N s、安裝完成
   Get-Process cockpit, cockpit-launch | Select-Object Id, StartTime, Path   # 程序路徑在 Programs\AI Agent Cockpit、啟動時間在按「是」之後
   Get-Content "$data\cockpit.update.json"
   ```

9. **收尾**：關 Cockpit 視窗等 10 秒、關掉假伺服器，再：

   ```powershell
   Start-Process "$env:LOCALAPPDATA\Programs\AI Agent Cockpit\unins000.exe" -ArgumentList '/VERYSILENT','/SUPPRESSMSGBOXES','/NORESTART' -Wait
   Copy-Item "$env:TEMP\cockpit-e2e-backup\AI Agent Cockpit.lnk" ([Environment]::GetFolderPath('Desktop')) -Force
   Remove-Item "$env:LOCALAPPDATA\ai-cockpit\cockpit.update.json", "$env:LOCALAPPDATA\ai-cockpit\cockpit.log" -ErrorAction SilentlyContinue
   Remove-Item Env:COCKPIT_UPDATE_API_URL, Env:COCKPIT_UPDATE_DOWNLOAD_BASE
   ```

   `%LOCALAPPDATA%\ai-cockpit\bin\`（`install-desktop.ps1` 的安裝）不要刪。

## 判讀

- 第 7 步四項都符合預期、第 8 步有下載紀錄與「重新啟動後的新程序」→ 驗證通過，結果補記在本檔，可發 v0.1.1。
- 按「是」後出現錯誤訊息框、仍開啟目前版本 → 下載或驗證失敗（規格「失敗時開啟現有版本」）；看 `server.log` 是否有下載請求、
  `cockpit.update.json` 的 `result`。
- 安裝進度視窗出現但 Cockpit 沒有重開 → 看安裝檔 log：`waited 30 s ... state 1` 表示 30 秒內程序沒結束（結束碼 7，不會重開舊版，
  已知延後項）；沒有這行表示 `[Run]` 的更新模式項目沒執行。
- 出現 SmartScreen → 下載的檔案被加上「從網路下載」標記，與 auto-update design 的前提不符，要回報給實作 session。
