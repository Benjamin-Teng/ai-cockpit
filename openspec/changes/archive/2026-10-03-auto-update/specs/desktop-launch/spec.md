# desktop-launch（修改）

## MODIFIED Requirements

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
