# desktop-launch Specification

## Purpose

定義「一個捷徑就開好 Cockpit」：Windows 上無主控台視窗的啟動器與建立捷徑的安裝腳本，以及隨畫面關閉而結束的後端模式（`--exit-when-idle`，不限平台）。
決策背景見 ADR-0005 與 change `desktop-launch-notify` 的 proposal。

## Requirements

### Requirement: 啟動器

系統必須提供 `cockpit-launch` 執行檔，在 Windows 上以圖形子系統執行（啟動時不出現主控台視窗），依序執行：

1. **引數與設定**：命令列只接受 `--config <path>`（或 `--config=<path>`），依 `cockpit-config`「設定檔位置與零設定模式」的同一規則決定設定，取得監聽位址；
   其他引數、設定錯誤、監聽埠為 0 時，以錯誤訊息框結束。
2. **瀏覽器**：依序選第一個存在的執行檔——環境變數 `COCKPIT_BROWSER`、Google Chrome、Microsoft Edge（各自依標準安裝位置尋找）；都找不到時以錯誤訊息框結束
   （此時尚未啟動任何後端）。
3. **偵測**：對 `http://<監聽位址>/api/state` 發 `GET`（連線與讀取各有逾時，總計不超過 3 秒），回應 200 且本體是含 `version` 與 `runtimes` 欄位的 JSON 物件，
   即視為 Cockpit 已在執行，跳到第 6 步；連得上但不是 Cockpit 的回應（或不回 HTTP）視為埠被占用，以錯誤訊息框結束；連不上則進入第 4 步。
4. **更新檢查**：依 `auto-update` 規格檢查並處理更新。交給安裝檔時啟動器在此以 0 結束，不執行後續步驟；其餘情況（不需檢查、
   沒有新版、使用者選稍後、下載或驗證失敗）進入第 5 步。
5. **背景啟動**：以同目錄的 `cockpit` 執行檔在背景啟動後端（Windows 上不建立主控台視窗、標準輸入為空），工作目錄沿用啟動器的工作目錄，引數為
   `--config <設定檔絕對路徑>`（有設定檔時）與 `--exit-when-idle`；後端的標準輸出與標準錯誤寫到設定檔所在目錄的 `cockpit.log`（零設定模式時寫到工作目錄），
   每次啟動覆寫。之後每 200 毫秒以第 3 步的方式檢查是否就緒，最多 15 秒。後端在就緒前結束時，先再偵測一次：若已有 Cockpit 在執行（例如同時點了兩次捷徑、
   另一個啟動器先啟動成功），照常進入第 6 步；否則以錯誤訊息框結束，內容含 `cockpit.log` 的完整路徑與其最後 20 行。15 秒仍未就緒也以同樣的訊息框結束。
6. **開啟視窗**：以 `--app=http://<監聽位址>/` 啟動瀏覽器（不等待其結束），之後啟動器以 0 結束。

非 Windows 平台編譯時，啟動器只印出「僅支援 Windows」的說明並以非 0 結束。

#### Scenario: 已在執行時只開視窗

- **GIVEN** 監聽位址上已有 Cockpit 在執行
- **WHEN** 執行 `cockpit-launch`
- **THEN** 沒有啟動新的 `cockpit` 程序，並以 `--app=http://<監聽位址>/` 開啟瀏覽器

#### Scenario: 沒在執行時啟動後端

- **GIVEN** 監聽位址上沒有任何程式
- **WHEN** 在工作目錄 `D:\work` 執行 `cockpit-launch --config cockpit.toml`
- **THEN** 以 `--config D:\work\cockpit.toml --exit-when-idle` 在背景啟動 `cockpit`，`D:\work\cockpit.log` 收到其輸出，就緒後開啟瀏覽器

#### Scenario: 埠被其他程式占用

- **GIVEN** 監聽位址上有一個不是 Cockpit 的 HTTP 服務
- **WHEN** 執行 `cockpit-launch`
- **THEN** 跳出訊息框說明埠被占用，沒有啟動 `cockpit`，也沒有開啟瀏覽器

#### Scenario: 後端啟動失敗

- **GIVEN** 設定檔本身合法，但其狀態檔內容損毀，使 `cockpit` 啟動後立即結束
- **WHEN** 執行 `cockpit-launch`
- **THEN** 跳出訊息框，內容含 `cockpit.log` 路徑與其最後數行（含錯誤原因），沒有開啟瀏覽器

#### Scenario: 同時點兩次捷徑

- **GIVEN** 監聽位址上沒有任何程式
- **WHEN** 幾乎同時執行兩次 `cockpit-launch`
- **THEN** 只有一個 `cockpit` 持續執行，兩個啟動器都開啟了瀏覽器視窗，沒有任何錯誤訊息框

#### Scenario: 瀏覽器選擇順序

- **GIVEN** 沒有設定 `COCKPIT_BROWSER`，Chrome 未安裝、Edge 已安裝
- **WHEN** 啟動器要開啟視窗
- **THEN** 使用 Edge

#### Scenario: 交給安裝檔後不啟動後端

- **GIVEN** 監聽位址上沒有任何程式，更新檢查判定有新版且使用者選擇更新、安裝檔驗證通過
- **WHEN** 啟動器啟動安裝檔
- **THEN** 啟動器以 0 結束，沒有啟動 `cockpit`，也沒有開啟瀏覽器

### Requirement: 閒置自動結束

`cockpit` 必須接受命令列旗標 `--exit-when-idle`（可與 `--config` 併用、順序不拘；重複給視為錯誤）。帶此旗標時，後端記錄目前開著的 `/ws` 連線數，並在下列
任一情況走與 Ctrl-C 相同的正常關閉流程結束、結束碼 0：

- **從未連線**：自開始監聽起 60 秒內從未有任何 `/ws` 連線（期間其他 HTTP 請求不延長這 60 秒）。
- **閒置**：曾經有過連線，目前連線數為 0，且距離「最近一次連線數降為 0」與「之後最近一次 `GET /` 請求或通過來源檢查的 `GET /api/state` 請求」兩者中較晚者已滿 10 秒。
  計時中若有新的 `/ws` 連線即取消；之後再降為 0 時重新計時。

`GET /` 與通過來源檢查的 `GET /api/state` 在閒置計時中延長期限，是為了讓「啟動器偵測到 Cockpit 仍在執行、接著開啟視窗」這段時間內後端不會剛好結束。
被來源檢查拒絕的 `/ws` 與 `GET /api/state`（`cockpit-dashboard`「狀態端點只接受本機同源請求」）不算連線、不延長期限。
其他 HTTP 請求不影響連線數與計時。不帶此旗標時，行為與過去完全相同（不因連線數結束）。

後端結束後，`agent-reporting` 的回報端點與 `pipeline-progress` 的寫入端點都無法連線，在此期間送出的請求會連線失敗、不會被保留或重送；
這是「關視窗即結束」（使用者 2026-10-02 的決定）的直接後果，啟動器模式下 agent 的進度回報只在 Cockpit 視窗開著時有效。「結束」指開始正常關閉流程；關閉流程本身所需的時間依既有規則。

#### Scenario: 最後一個畫面關閉後結束

- **GIVEN** `cockpit --exit-when-idle` 執行中，有一個 `/ws` 連線
- **WHEN** 該連線關閉，之後 10 秒內沒有新連線、也沒有 `GET /` 或通過來源檢查的 `GET /api/state`
- **THEN** 後端在約 10 秒時開始正常關閉並結束，結束碼 0

#### Scenario: 重新整理不會被誤殺

- **GIVEN** `cockpit --exit-when-idle` 執行中，有一個 `/ws` 連線
- **WHEN** 該連線關閉，3 秒後又有新的 `/ws` 連線
- **THEN** 後端持續執行

#### Scenario: 啟動器偵測延長期限

- **GIVEN** `cockpit --exit-when-idle` 執行中，最後一個連線已關閉 8 秒
- **WHEN** 收到通過來源檢查的 `GET /api/state`（啟動器以 `Host: <監聽位址>` 偵測），之後 5 秒內有新的 `/ws` 連線
- **THEN** 後端持續執行

#### Scenario: 從未有人連上

- **GIVEN** `cockpit --exit-when-idle` 剛開始監聽
- **WHEN** 60 秒內沒有任何 `/ws` 連線（期間有 `GET /api/state` 也一樣）
- **THEN** 後端正常結束

#### Scenario: 被拒的 /ws 不讓後端保持執行

- **GIVEN** `cockpit --exit-when-idle` 執行中，唯一的 `/ws` 連線剛關閉
- **WHEN** 之後只有帶 `Origin: https://evil.example` 的 `/ws` 請求，以及 `Host: evil.example:<port>` 的 `GET /api/state` 請求（皆回 403）
- **THEN** 閒置計時不受影響，約 10 秒後後端照常結束

#### Scenario: 沒有旗標時不結束

- **GIVEN** `cockpit` 不帶 `--exit-when-idle` 執行
- **WHEN** 唯一的 `/ws` 連線關閉並經過 60 秒
- **THEN** 後端持續執行

### Requirement: 桌面捷徑安裝腳本

系統必須提供 `scripts/install-desktop.ps1`（PowerShell 7 執行，或 Windows PowerShell 5.1 以 `-ExecutionPolicy Bypass -File` 執行）：以正式版建置 `cockpit`
套件的兩個執行檔，複製到安裝目錄（預設 `%LOCALAPPDATA%\ai-cockpit\bin\`），並在使用者桌面（`[Environment]::GetFolderPath('Desktop')`，涵蓋桌面被重導向的情況）
建立名為「AI Agent Cockpit」的捷徑，目標為安裝目錄的 `cockpit-launch.exe`。參數 `-Config <path>` 指定設定檔（預設為 repo 根目錄的 `cockpit.toml`）：

- 設定檔存在時：捷徑引數為 `--config "<絕對路徑>"`，工作目錄為設定檔所在目錄。
- 不存在時：提示將以零設定模式執行；捷徑不帶 `--config`，工作目錄為 `%LOCALAPPDATA%\ai-cockpit\`（必要時建立）。

另提供 `-InstallDir` 與 `-ShortcutDir` 參數覆寫安裝目錄與捷徑位置（供測試使用）。安裝目錄中的 `cockpit.exe` 或 `cockpit-launch.exe` 正在執行時，腳本必須在
複製前停止、提示先關閉 Cockpit 並以非 0 結束，不得覆寫執行中的檔案。重複執行即為更新。

#### Scenario: 安裝並建立捷徑

- **GIVEN** repo 根目錄有 `cockpit.toml`，Cockpit 沒有在執行
- **WHEN** 執行 `scripts/install-desktop.ps1`
- **THEN** 安裝目錄有 `cockpit.exe` 與 `cockpit-launch.exe`，桌面捷徑目標為後者、引數為 `--config "<repo>\cockpit.toml"`、工作目錄為 repo 根目錄

#### Scenario: 零設定的捷徑

- **GIVEN** 指定的設定檔不存在
- **WHEN** 執行安裝腳本
- **THEN** 捷徑不帶 `--config`，工作目錄為 `%LOCALAPPDATA%\ai-cockpit\`

#### Scenario: 執行中不覆寫

- **GIVEN** 安裝目錄的 `cockpit.exe` 正在執行
- **WHEN** 執行 `scripts/install-desktop.ps1`
- **THEN** 腳本提示先關閉 Cockpit 並以非 0 結束，安裝目錄的檔案未被修改
