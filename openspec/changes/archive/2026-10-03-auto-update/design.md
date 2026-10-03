# design：auto-update

## Context

- 背景研究（2026-10-03，對話中由 subagent 查一手來源）：Microsoft Learn 明載 EV 憑證不再繞過 SmartScreen，信譽要靠下載量累積；
  Azure Artifact Signing 個人只開放美國、加拿大；SignPath Foundation 免費但要過審、Certum 開源憑證約 €49–69 且 CI 自動化只有
  非官方做法。Shioaji Pro v0.1.51 的 Windows 安裝檔 `Get-AuthenticodeSignature` 為 `NotSigned`，靠 Tauri updater 讓警告只在
  首次安裝出現。結論：先做自動更新，簽章另議。
- 現況：`cockpit-launch`（`cockpit/src/bin/cockpit-launch.rs`）只做 I/O 串接，可測邏輯在 `cockpit/src/launch.rs`；訊息語言
  走 `LaunchText` 與 `text()`。啟動器一開始就清掉標準 handle 的繼承旗標（`stop_inheriting_std_handles`）。`cockpit` crate
  目前沒有 HTTP client、TLS、SHA-256 相依（`reqwest` 只經測試鏈進 `Cargo.lock`）。
- 契約（打包發佈 session 2026-10-03 提供，見 `openspec/specs/release-distribution/` 與 archive
  `2026-10-03-release-packaging/design.md` D2、D6）：資產為 `ai-cockpit-<X.Y.Z>-x64-setup.exe`、`ai-cockpit-<X.Y.Z>-x64.zip`、
  `SHA256SUMS.txt`（依檔名排序、`<64 位小寫 hex>␠␠<檔名>`、LF、結尾換行、無 BOM）；tag `vX.Y.Z` 等於 `cockpit` crate 版本；
  安裝檔執行中中止的結束碼為 7；`[Run]` 的完成頁啟動帶 `skipifsilent`。v0.1.0 已改標預發布，`releases/latest` 目前回 404。

## Goals / Non-Goals

**Goals：** 已安裝的使用者在啟動時得知有新版、一鍵更新、裝完自動重開；更新下載不經瀏覽器，因而不帶「從網路下載」標記；
任何失敗都退回照常啟動目前版本。

**Non-Goals：** 見 proposal「非目標」。

## Decisions

### D1 檢查點：啟動器偵測到後端沒在執行之後、啟動後端之前

後端在執行時，安裝檔會因 `cockpit.exe` 執行中而中止（D2 of release-packaging）；要更新就得結束後端，而後端可能正被使用者的
瀏覽器視窗使用。只在「後端沒在執行」時檢查，就不必關閉任何正在使用的東西，也維持「不強制結束」的原則。代價是使用者一直
開著視窗就一直不會被詢問；後端有 `--exit-when-idle`，視窗關掉後 10 秒就結束，下次從捷徑開啟時即可檢查。

- 替代：後端或畫面顯示「有新版」橫幅。否決：要改後端與前端、跨 crate，且仍要結束後端才能裝；留作後續。

### D2 「由安裝檔安裝」以程式目錄有 `unins000.exe` 判定

Inno Setup 會把解除安裝程式 `unins000.exe` 放在程式目錄。zip、`install-desktop.ps1` 與 `cargo build` 的目錄都沒有它。
比讀 HKCU Uninstall 機碼簡單（不需登錄 API、不需 `windows-sys`），也直接回答「這個目錄是不是安裝檔管理的」。

- 替代：比對 `HKCU\...\Uninstall\BenjaminTeng.AIAgentCockpit_is1` 的 `InstallLocation`。否決：要加登錄存取，換來的精確度
  在本情境沒有差別。

### D3 相依：`ureq`（rustls）加 `sha2`，不用現成更新框架

現成框架都不支援「下載 Inno 安裝檔並以參數執行」：`self_update` 只替換執行檔（會繞過安裝檔的捷徑與解除安裝登錄）、
`axoupdater` 綁 cargo-dist 安裝收據、`velopack` 要整套換成自家打包。需求只有「GET 一個 JSON、下載兩個檔、算 SHA-256」，
自寫約兩百行即可。HTTP 用 `ureq`（同步、無 async runtime，啟動器本來就是同步程式），TLS 用 rustls（不依賴系統
schannel 行為差異）；SHA-256 用 `sha2`。已加入 `cockpit/Cargo.toml`：`ureq = "3.4.2"`（預設 feature，TLS 後端為
rustls，沒有 native-tls／openssl）、`sha2 = "0.11.0"`；版號、feature 與來源見「外部精確資訊」。

- 執行檔大小（`cargo build --release -p cockpit --bins`，2026-10-03）：task 1.2 加相依時 `cockpit-launch.exe` 為
  1,453,056 位元組、`cockpit.exe` 為 16,672,256 位元組（尚未有程式碼使用新 crate，連結器不放入，差異 0）。
  Unit 3（tasks 3.1–3.3，啟動器實際使用 `ureq`、rustls、`sha2`）完成後：`cockpit-launch.exe` 3,603,456 位元組
  （增加 2,150,400，約 2.1 MB，主要是 rustls、ring 與內建根憑證）；`cockpit.exe` 16,667,136 位元組（後端不使用這些
  crate，大小不變，差異 −5,120 來自 lib 的其他程式碼變動）。最終審查修正波（F2 每程序暫存子資料夾、F3 `https_only`、
  F5 `MB_SETFOREGROUND`；根憑證仍為內建 `webpki-roots`）後：`cockpit-launch.exe` 3,609,600 位元組（較 Unit 3 增加
  6,144）、`cockpit.exe` 16,983,040 位元組。
- 依賴樹增量（`cargo tree -p cockpit -e normal`）：`ureq`、`ureq-proto`、`rustls 0.23`（ring 加密供應者）、
  `rustls-webpki`、`webpki-roots`（內建 Mozilla 根憑證）、`flate2`（預設的 `gzip`）、`sha2` 與其 RustCrypto 相依；
  `native-tls`、`openssl`、`schannel`、`aws-lc-rs` 皆不在樹中。
- 根憑證為內建的 `webpki-roots`，不讀 Windows 憑證存放區：有 TLS 檢查（自簽根）的公司網路會憑證驗證失敗，依本設計
  一律靜默略過、照常啟動。若日後要支援，改開 `ureq` 的 `platform-verifier` feature（新增 `rustls-platform-verifier`
  相依）；目前不採用，見下一點。
- 最終審查修正波評估過改用平台驗證器（Risks「根憑證生命週期」），查證結果見「外部精確資訊」的 `ureq` 平台驗證器一項：
  設定方式明確、Windows 不需額外系統相依，但 Windows 實作每次建立憑證鏈都做線上撤銷檢查、URL 取得逾時 10 秒，且在
  `ureq` 的整體逾時之外阻塞；在擋掉 OCSP／CRL 端點的網路上，查詢會超過 spec 的 3 秒上限。另有 Windows 的 open issue
  指出 TLS 檢查用的自簽根仍驗證失敗，「支援公司網路 TLS 檢查」的好處也不成立。結論（控制端裁決）：維持內建
  `webpki-roots`，不開 `platform-verifier`；根憑證過期風險的處理見 Risks「根憑證生命週期」。
- 預設 `Agent` 的行為與本設計相關者：`http_status_as_error` 預設為真（404、403 會變成 `Err`，實作要把它們分流處理，
  不可當成網路錯誤）；`https_only` 預設為假，本設計在沒有 D7 覆寫變數時設為真（連轉址在內只允許 `https://`），有覆寫時
  維持假讓測試能用 `http://`；跟隨最多 10 次轉址；未設
  `User-Agent` 時預設送 `ureq/<版本>`，D6 要明確覆寫。

- 替代：呼叫 Windows 內建 `curl.exe` 與 `certutil -hashfile`。否決：解析外部程式輸出、錯誤處理與測試都更脆弱，且要再處理
  子程序視窗與 handle 繼承。

### D4 模組切分：純邏輯放 `cockpit/src/update.rs`，I/O 留在啟動器

`update.rs`（`cockpit` lib，無網路、無檔案 I/O）提供：版本解析與比較（只接受 `X.Y.Z` 三段十進位）、從 latest release 的 JSON
判定有無新版（`tag_name` 與兩個資產名稱）、`SHA256SUMS.txt` 的解析與比對、24 小時節流判定（輸入現在時間與紀錄）、固定
下載網址組裝、更新檢查紀錄的序列化，以及 ureq agent 的設定（`http_agent` 只建立設定、不連線；放在 lib 讓單元測試能驗證
`https_only`）。啟動器 bin 負責網路、檔案、訊息框、spawn。新增的訊息文字加進 `LaunchText`，中英
兩版，與既有啟動器訊息同一套語言判定。依賴方向不變（`cockpit` 內部，不碰 `cockpit-core`，ADR-0003）。

### D5 更新檢查紀錄：資料目錄的 `cockpit.update.json`

放在 `cockpit.log` 所在目錄（`launch::log_path` 的同一規則：有設定檔時為其目錄，零設定時為工作目錄；安裝版即
`%LOCALAPPDATA%\ai-cockpit\`）。內容 `{"checked_at": <Unix 秒>, "result": "<結果與原因>"}`，每次發出查詢前寫入
`checked_at`，結果出來後覆寫 `result`。不寫進 `cockpit.log`，因為該檔每次啟動後端都被覆寫。讀不懂、不存在或
`checked_at` 在未來都視為「已滿 24 小時」（防止時鐘被調回後永遠不檢查）。

### D6 查詢與下載的網址、逾時與上限

- 查詢：`GET https://api.github.com/repos/Benjamin-Teng/ai-cockpit/releases/latest`，帶 `User-Agent: ai-cockpit/<版本>`
  與 `Accept: application/vnd.github+json`，整體逾時 3 秒（spec）。
- 下載：不採用 API 回應的 `browser_download_url`，而以固定格式
  `https://github.com/Benjamin-Teng/ai-cockpit/releases/download/v<版本>/<檔名>` 組出（spec），讓下載來源永遠是本 repo 的
  release，即使 API 回應被竄改也無法把下載導到其他網域；GitHub 會轉址到其資產主機，`ureq` 跟隨轉址。
- 下載逾時：`SHA256SUMS.txt` 15 秒、安裝檔 300 秒；安裝檔以串流寫檔、超過 200 MB 即中止。
- 暫存位置：每個啟動器程序一個子資料夾 `%TEMP%\ai-cockpit-update\<pid>\`，下載前清空（同 pid 的舊資料夾屬於已結束的
  程序）；安裝檔與 `setup.log`（D8）都在其中。同時開兩個啟動器（雙擊）時各用各的，不會清掉或覆寫對方的下載。
- 舊子資料夾清理：已安裝版每次進入更新步驟（後端沒在執行時，不論是否在 24 小時內、是否設了 `COCKPIT_NO_UPDATE_CHECK`）
  先嘗試刪除 `ai-cockpit-update` 下其他子資料夾，刪不掉（被占用）就留著、不顯示訊息，下次再試。所以更新後由安裝檔重新
  啟動的那次就會清掉上次下載的安裝檔。下載中的安裝檔以不含 `FILE_SHARE_DELETE` 的共用模式開啟，另一個啟動器的清理刪不掉
  它；正在執行的安裝檔同樣刪不掉。代價：更新失敗時的 `setup.log` 只留到下次後端沒在執行時的啟動。
- `SHA256SUMS.txt` 的格式是已發出客戶端的契約，由 CI 斷言（D12）。

### D7 測試入口：兩個網址覆寫變數與一個答案變數，只供自動驗收

- `COCKPIT_UPDATE_API_URL`：取代查詢網址；`COCKPIT_UPDATE_DOWNLOAD_BASE`：取代
  `https://github.com/Benjamin-Teng/ai-cockpit/releases/download`。任一有值時 agent 允許 `http://`，讓測試用本機 axum
  假伺服器；兩者都沒設時 agent 設 `https_only(true)`（`update::https_only`、`update::http_agent`）。
- 訊息框：沿用 `COCKPIT_LAUNCH_DIALOG_FILE`（有值時訊息寫檔、不跳框）。詢問框在此模式下的答案取
  `COCKPIT_LAUNCH_UPDATE_ANSWER=yes|no`，未設為 `no`（自動驗收不可能意外觸發安裝）。
- 這些變數能把更新導向其他來源；但能改使用者環境變數的人本來就能直接替換程式目錄（per-user 安裝、使用者可寫），不構成新的
  攻擊面。README 不宣傳這些變數。

### D8 交棒：`/SILENT` 加自訂參數，啟動器立即結束

啟動器以 `setup.exe /SILENT /SUPPRESSMSGBOXES /NORESTART /NOCANCEL /COCKPITUPDATE=1 /LOG="%TEMP%\ai-cockpit-update\<pid>\setup.log"`
啟動安裝檔，標準輸入、輸出、錯誤皆 `Stdio::null()`（啟動器開頭已清掉自身 handle 的繼承旗標），不等待，隨即以 0 結束。

- `/SILENT` 而非 `/VERYSILENT`：顯示進度視窗，使用者看得到「正在更新」，不會以為點了沒反應。
- `/SUPPRESSMSGBOXES`：更新模式逾時的中止訊息不卡住；逾時代表有人在 30 秒內又開了 Cockpit，現有版本照常可用，24 小時後再詢問，
  原因留在 `setup.log`。
- 下載的安裝檔沒有 Zone.Identifier（`ureq` 寫檔不經 Attachment Manager），SmartScreen 不檢查；以自動測試斷言下載檔沒有
  `Zone.Identifier` 資料流佐證，並在端到端演練以肉眼確認沒有警告。

### D9 安裝檔更新模式：`{param:COCKPITUPDATE|0}`

- `[Code]` 新增 `IsUpdateMode()`：`ExpandConstant('{param:COCKPITUPDATE|0}') = '1'`。
- `PrepareToInstall`：更新模式時每秒以 `CockpitRunState` 檢查一次，最多 30 次；得到 0 即繼續，期限到時沿用原本的中止訊息
  （結束碼 7）。非更新模式不變。等待用 `[Code]` 的 `Sleep(1000)`（`procedure Sleep(const Milliseconds: Cardinal);`，Inno
  Setup 官方 Support Functions 的 Other functions；6.7.1 原始碼 `Setup.ScriptFunc.pas` 有註冊，查證見「外部精確資訊」），
  不需要替代做法。
- `[Run]` 新增一筆：`Filename: "{app}\cockpit-launch.exe"; WorkingDir: "{#DataDir}"; Flags: nowait skipifnotsilent;
  Check: IsUpdateMode`。原本帶 `skipifsilent` 的完成頁項目不動，所以一般靜默安裝仍不啟動（冒煙測試第 2 步繼續成立）。
- 重新啟動的啟動器不帶引數：安裝版捷徑本來就不帶引數；自訂 `--config` 捷徑的使用者更新後要從自己的捷徑再開一次，可接受。
- 更新模式不改選項：Inno 預設 `UsePreviousTasks=yes`，靜默模式沿用上次的桌面捷徑選擇。

### D10 冒煙測試新增兩個情境

在既有「覆蓋更新」之後、解除安裝之前：

1. **等候後安裝並重新啟動**：以資料目錄為工作目錄背景啟動已安裝的 `cockpit.exe`，另起計時器 5 秒後停止它；同時以
   啟動器交棒的同一組參數（D8：`/SILENT /SUPPRESSMSGBOXES /NORESTART /NOCANCEL /COCKPITUPDATE=1`，`/LOG=` 指到含空白
   的路徑、整個引數加雙引號，與 Rust `Command::arg` 相同）執行安裝檔。斷言結束碼 0、log 有檔案複製紀錄、30 秒內
   `http://127.0.0.1:7770/api/state` 回應為 Cockpit（重新啟動的啟動器拉起了後端），之後停掉相關程序。重新啟動的啟動器會開
   瀏覽器，所以整個腳本設定假的 `COCKPIT_BROWSER`（選定 `%SystemRoot%\System32\whoami.exe`：收到不認得的 `--app=<網址>`
   時印錯誤並以 1 立即結束，不讀標準輸入）與 `COCKPIT_LAUNCH_DIALOG_FILE`，並設 `COCKPIT_NO_UPDATE_CHECK=1` 避免冒煙測試
   對外連網。
2. **逾時**：背景啟動 `cockpit.exe` 不停止，以更新模式執行安裝檔。斷言結束碼 7、耗時至少 30 秒、log 沒有檔案複製紀錄、
   沒有 `cockpit-launch.exe` 被啟動，之後停掉 `cockpit.exe`。

### D11 執行路徑：SDD

跨 Rust（純邏輯、啟動器 I/O）、Inno `[Code]`、PowerShell 冒煙測試三種技術；有網路與完整性驗證（安全邊界）與程序生命週期
（交棒、等候）兩個高風險面，後面的整合測試建在前面的純邏輯上。依 `openspec-workflow`「執行路徑分流」走 SDD。

### D12 已發出客戶端的契約由 CI 守

啟動器發出後無法修改，它依賴的四項發布產物性質（資產名稱、`SHA256SUMS.txt` 格式、更新模式參數、`unins000.exe`）寫成
spec `release-distribution`「自動更新客戶端契約」。`release.yml` 在 build 的 Checksums 步驟寫出 `SHA256SUMS.txt` 後讀回
位元組斷言：無 BOM、無 CR、以 LF 結尾、每行符合 `^[0-9a-f]{64}  [^ *].*$`（區分大小寫；`[^ *]` 比最終審查提的
`[^ ]` 多擋檔名前的 `*` 二進位標記，對應契約第 2 項），不符即失敗；release job 的 Verify checksums 對下載回來的檔再以
bash 驗一次同樣規則。手動觸發只跑 build job，所以前者是每次手動 run 都會執行的那一道。更新模式參數由冒煙測試 5a、5b
以同一組參數執行涵蓋；`unins000.exe` 由冒煙測試第 1 步新增的檢查涵蓋。

## Risks / Trade-offs

- [GitHub 帳號或 release 被竄改時，攻擊者可同時換掉安裝檔與 `SHA256SUMS.txt`] → 本 change 只擋傳輸損毀與中間人；獨立簽章
  （minisign 等，公鑰編入執行檔、私鑰不放 GitHub）列為後續。
- [已安裝 v0.1.0 的使用者沒有更新器] → v0.1.0 為預發布，CHANGELOG 與 README 說明需手動安裝一次 v0.1.1。
- [公司網路走系統代理（WinINet）時 `ureq` 可能連不出去] → 失敗一律靜默、照常啟動。查證結論（`ureq` 3.4.2，2026-10-03）：
  預設 `Agent` 會依序讀 `ALL_PROXY`、`HTTPS_PROXY`、`HTTP_PROXY`（含小寫變體）環境變數，並讀 `NO_PROXY`；**不**讀 Windows
  「網際網路選項」的系統代理（那要開 `win-system-proxy` feature，會多一個 `winreg` 相依，本 change 不啟用）。所以只設系統
  代理、沒設環境變數的環境會直接連線並（若被擋）逾時失敗，行為即上述靜默略過，最多多等 3 秒；README 疑難排解可說明改設
  `HTTPS_PROXY`。
- [多人共用 NAT 對外 IP 時 60 次／小時的匿名速率限制] → 每台電腦 24 小時最多一次，403／429 靜默略過。
- [啟動器結束與安裝檔的 WMI 偵測之間的競態] → 更新模式最多等 30 秒，啟動器在 spawn 後立即結束，正常情況 1 秒內即不在執行。
- [防毒軟體在安裝檔寫入或執行時延遲或攔截] → 失敗時安裝檔不會執行、啟動器照常開啟目前版本（下載、驗證、啟動安裝檔任一步
  失敗都顯示錯誤後照常啟動）。
- [根憑證生命週期：已發出的啟動器凍結，內建 `webpki-roots` 的根憑證清單隨該版本固定] → 數年後 GitHub 若換到清單沒有的
  根憑證，舊版的檢查會 TLS 失敗並一律靜默略過，使用者只會「不再收到更新」而不知道。結論：維持內建 `webpki-roots`；
  發新版前執行 `cargo update -p webpki-roots`，讓每個新版帶最新的根清單。長期未更新的客戶端若遇到 GitHub 換到清單外的
  CA，會靜默查不到新版，需到 Releases 頁面手動下載。未改用 `ureq` 的 `platform-verifier`（讀 Windows 憑證存放區）的
  原因見 D3：Windows 實作的撤銷檢查最多阻塞 10 秒、不受 `ureq` 逾時控制，會違反 3 秒查詢上限
  （<https://github.com/rustls/rustls-platform-verifier/issues/237>，仍 open）；TLS 檢查用的自簽根仍驗證失敗
  （<https://github.com/rustls/rustls-platform-verifier/issues/138>），支援公司網路 TLS 檢查的好處也不成立。待 #237
  修好再評估。
- [驗證通過到執行安裝檔之間，檔案可能被替換（TOCTOU）] → Unit 3 審查結論：不另做處理。`%TEMP%` 只有同一使用者、SYSTEM、
  管理員可寫，能在這段空檔替換檔案的攻擊者，本來就能直接改 per-user 程式目錄或設 D7 的覆寫變數，沒有跨權限邊界。
- [第一次檢查會讓沒有網路時的啟動多等最多 3 秒] → 24 小時一次，可接受。

## Migration Plan

1. 本 change 合併後由打包發佈 session 升版 0.1.1、補 `CHANGELOG.md` 段落、跑 rc 演練與發布。
2. 端到端演練（rc 期間）：安裝本 change 建出的安裝檔，以 `COCKPIT_UPDATE_API_URL`／`COCKPIT_UPDATE_DOWNLOAD_BASE` 指向本機
   假伺服器提供較高版本號的安裝檔，從捷徑啟動並選「是」，確認重新開啟且沒有 SmartScreen 警告。
3. 回退：若 v0.1.1 的更新器有問題，發 v0.1.2 修正；更新器失敗一律照常啟動目前版本，不會讓程式無法使用。

## Open Questions

無阻擋實作的問題；外部精確資訊已於 task 1.1 查證，結果見下節。

## 外部精確資訊（task 1.1 查證，2026-10-03）

### Rust 相依

- `ureq` 最新穩定版 3.4.2（2026-09-13 發布，未撤回，MSRV 1.85；本機 cargo 1.97.1）。來源：
  <https://crates.io/api/v1/crates/ureq>，版本與 feature 表：<https://crates.io/api/v1/crates/ureq/3.4.2>。
- `ureq` 3.4.2 的 feature：`default = ["rustls", "gzip"]`；`rustls = ["rustls-no-provider", "_ring", "rustls-webpki-roots"]`
  （ring 加密供應者加內建 webpki-roots）；`native-tls`、`platform-verifier`、`socks-proxy`、`win-system-proxy`、`json`、
  `cookies`、`charset`、`brotli`、`multipart` 皆為選用。結論：預設 TLS 後端就是 rustls，不需指定 feature。來源：同上
  crates.io 版本 API；原始碼 `Cargo.toml`（<https://static.crates.io/crates/ureq/ureq-3.4.2.crate>）。
- `ureq` 代理：`Config::default()` 呼叫 `Proxy::try_from_env()`，依序讀 `ALL_PROXY`、`HTTPS_PROXY`、`HTTP_PROXY`（各含小寫），
  `NO_PROXY`／`no_proxy` 決定繞過的主機；Windows 系統代理只在 `win-system-proxy` feature 開啟時才讀（HKCU Internet
  Settings 的 `ProxyEnable`／`ProxyServer`）。來源：<https://docs.rs/ureq/3.4.2/ureq/> 的 Proxying 節；上述 crate 原始碼
  `src/lib.rs`、`src/proxy.rs`、`src/config.rs`。
- `sha2` 最新穩定版 0.11.0（2026-03-25 發布，MSRV 1.85）；feature `default = ["alloc", "oid"]`、選用 `zeroize`，本用途用
  預設即可。來源：<https://crates.io/api/v1/crates/sha2>、<https://crates.io/api/v1/crates/sha2/0.11.0>。
- 加入後 `cargo tree -p cockpit -e normal`：`ureq v3.4.2`、`sha2 v0.11.0`；`native-tls`、`openssl`、`openssl-sys`、`schannel`、
  `aws-lc-rs` 以 `cargo tree -i` 查詢皆回「did not match any packages」（不在樹中）。

- `ureq` 平台驗證器（最終審查修正波查證，2026-10-03；未採用）：feature 名 `platform-verifier =
  ["dep:rustls-platform-verifier"]`（`rustls-platform-verifier` `0.7.0` 起、`default-features = false`）；設定為
  `TlsConfig::builder().root_certs(RootCerts::PlatformVerifier)` 交給 `Agent::config_builder().tls_config(...)`，rustls
  後端以 `rustls_platform_verifier::Verifier::new(provider)` 當自訂驗證器。來源：`ureq` 3.4.2 原始碼 `Cargo.toml`、
  `src/lib.rs`（platform-verifier 節）、`src/tls/mod.rs`、`src/tls/rustls.rs`（<https://static.crates.io/crates/ureq/ureq-3.4.2.crate>）。
  Windows 實作（`rustls-platform-verifier` 最新穩定版 0.7.1，2026-09-24 發布，<https://crates.io/api/v1/crates/rustls-platform-verifier>；
  `src/verification/windows.rs`）以 `CERT_CHAIN_REVOCATION_CHECK_END_CERT`
  建鏈、`dwUrlRetrievalTimeout = 10 * 1000`，策略檢查卻設 `CERT_CHAIN_POLICY_IGNORE_ALL_REV_UNKNOWN_FLAGS`；上游 issue
  rustls/rustls-platform-verifier#237「Windows: revocation check blocks up to 10s with no effect on verification outcome」
  （2026-07-07 開、仍 open）。另有 Windows 相關 open issue #138（TLS 檢查用的自簽根裝在「本機電腦」存放區時仍
  `UnknownIssuer`）與 #200（rustup 經此驗證器在部分 Windows 機器 access violation）。README 寫明 Windows 不需額外設定。
  來源：<https://github.com/rustls/rustls-platform-verifier>、上述 issue 頁。

### GitHub REST API

- `GET /repos/{owner}/{repo}/releases/latest`：「The latest release is the most recent non-prerelease, non-draft release,
  sorted by the created_at attribute.」即排除草稿與預發布；沒有符合者回 404（文件的回應碼列有 404；實測本 repo 目前
  `releases/latest` 回 404，與 v0.1.0 為預發布一致）。來源：
  <https://docs.github.com/en/rest/releases/releases#get-the-latest-release>。
- `User-Agent` 必填：「All API requests must include a valid `User-Agent` header. Requests with no `User-Agent` header will
  be rejected. If you provide an invalid `User-Agent` header, you will receive a `403 Forbidden` response.」實測不帶時回 403。
  來源：<https://docs.github.com/en/rest/using-the-rest-api/getting-started-with-the-rest-api>。
- `Accept: application/vnd.github+json` 為多數端點的建議值（同上來源）。文件另列 `X-GitHub-Api-Version` 標頭（該頁寫
  `2026-03-10`）；本設計不送，採用 API 預設版本，只讀 `tag_name`，風險低。
- 匿名速率限制：「The primary rate limit for unauthenticated requests is 60 requests per hour.」依來源 IP 計算；超限回
  403 或 429，`x-ratelimit-remaining` 為 0，`x-ratelimit-reset` 為重置時間（UTC epoch 秒）。來源：
  <https://docs.github.com/en/rest/using-the-rest-api/rate-limits-for-the-rest-api>。

### Inno Setup（官方說明 <https://jrsoftware.org/ishelp/>；6.7.1 原始碼 tag `is-6_7_1`）

說明頁為目前發行版（官方下載頁最新為 6.7.3）的內容；下列項目皆為長期存在的功能，並以 6.7.1 原始碼交叉確認。

- `{param:ParamName|DefaultValue}`：「Embeds a command-line parameter value.」讀 `/ParamName=value` 形式的參數，不存在
  時用預設值。來源：<https://jrsoftware.org/ishelp/topic_consts.htm>；實作 `Setup.MainFunc.pas` 的 `ExpandParamConst`（比對
  `'/'+Param+'='`，不分大小寫）：
  <https://raw.githubusercontent.com/jrsoftware/issrc/is-6_7_1/Projects/Src/Setup.MainFunc.pas>。
- `[Run]` 的 `skipifnotsilent`：「Instructs Setup to skip this entry if Setup is not running (very) silent.」；`skipifsilent`
  相反；`nowait` 為不等待程序結束。來源：<https://jrsoftware.org/ishelp/topic_runsection.htm>；6.7.1 `Setup.MainFunc.pas`
  有 `roSkipIfNotSilent`。
- `/SILENT`、`/VERYSILENT`：「When Setup is silent the wizard and the background window are not displayed but the
  installation progress window is.」`/VERYSILENT` 連進度視窗也不顯示。`/SUPPRESSMSGBOXES`：「Only has an effect when
  combined with /SILENT or /VERYSILENT.」並列出各情境的預設答案（Abort/Retry 取 Abort 等）。`/NOCANCEL`：「Prevents the
  user from cancelling during the installation process, by disabling the Cancel button and ignoring clicks on the close
  button.」`/NORESTART`：不在安裝後重新啟動系統。`/LOG="filename"`：「Same as /LOG, except it allows you to specify a
  fixed path/filename to use for the log file. If a file with the specified name already exists it will be overwritten. If
  the file cannot be created, Setup will abort with an error message.」來源：
  <https://jrsoftware.org/ishelp/topic_setupcmdline.htm>。
- `PrepareToInstall` 回傳非空字串即停在「準備安裝」頁並以專屬結束碼結束（不使用 `/RESTARTEXITCODE`）。來源：
  <https://jrsoftware.org/ishelp/topic_scriptevents.htm>。
- `[Code]` 的 `Sleep`：存在，`procedure Sleep(const Milliseconds: Cardinal);`，「Suspends the execution of Setup or Uninstall
  for a specified interval.」列於 Support Functions 的 Other functions。來源：
  <https://jrsoftware.org/ishelp/topic_isxfunc_sleep.htm>、<https://jrsoftware.org/ishelp/topic_scriptfunctions.htm>；6.7.1
  `Setup.ScriptFunc.pas` 註冊 `'SLEEP'`：
  <https://raw.githubusercontent.com/jrsoftware/issrc/is-6_7_1/Projects/Src/Setup.ScriptFunc.pas>。

### 尚待後續實測的項目（非查證缺口）

- 更新模式下 `PrepareToInstall` 回傳錯誤字串時，`/SILENT /SUPPRESSMSGBOXES` 是否以結束碼 7 結束而不卡住：由 D10 冒煙測試
  情境 2 實測（既有冒煙測試已涵蓋 `/VERYSILENT` 的結束碼 7）。**實測結果（task 4.2，CI windows-latest、Inno Setup 6.7.1）**：
  以 D8 的完整參數（`/SILENT /SUPPRESSMSGBOXES /NORESTART /NOCANCEL /COCKPITUPDATE=1 "/LOG=<含空白路徑>"`）執行，
  等候 30 秒後以結束碼 7 結束、總耗時約 31 秒，沒有卡在訊息框。與原始碼一致：靜默安裝由
  `TWizardForm.ClickThroughPages`（`Setup.WizardForm.pas`）以可抑制的 `LoggedMsgBox` 顯示 PrepareToInstall 的錯誤後設
  `ecPrepareToInstallFailed`（7）並中止；`/SILENT` 與 `/VERYSILENT` 同走這條路。
- `/LOG=` 路徑含空白時整個引數加雙引號（`"/LOG=C:\a b\setup.log"`，Rust `Command::arg` 的形式）：**實測可用**，log 產生在該
  路徑（情境 1）。Inno 的 `GetParamStr`（`Shared.CommonFunc.pas`）在引數任何位置遇到 `"` 都只切換引號狀態並丟掉引號字元，
  所以與文件的 `/LOG="filename"` 形式等價，啟動器不必改。
