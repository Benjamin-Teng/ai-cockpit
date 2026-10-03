# tasks：auto-update

> 執行路徑：SDD｜理由：跨 Rust 純邏輯、啟動器 I/O、Inno `[Code]`、PowerShell 冒煙測試四塊可各自驗收的工作；有完整性驗證
> （安全邊界）與交棒等候（程序生命週期）兩個高風險面；後面的整合建在前面的純邏輯上（design D11）。

基線：開工前在分支 `feat/auto-update`（自最新 `origin/main`）記錄 `cargo test --workspace` 的通過數，之後每個 Rust task 結尾
都跑 `cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test --workspace`，改 `.md` 跑
`markdownlint-cli2 "**/*.md"`，0 error 才算完成。審查依專案 memory 由 Opus subagent 進行（視同 Codex review）。

## 1. 查證與相依

- [x] 1.1 查證 design「外部精確資訊」各項（`ureq`、`sha2` 最新版號與 feature、`ureq` 預設 TLS 後端與代理環境變數支援；GitHub
  `releases/latest` 行為、`User-Agent`、匿名速率限制；Inno 6.7.1 的 `{param:}`、`skipifnotsilent`、`/SILENT`、
  `/SUPPRESSMSGBOXES`、`/NOCANCEL`、`/LOG=`、`[Code]` 的 `Sleep`），逐項附一手來源 URL 與查證日期寫回 design。驗收＝design
  該節沒有「待查證」字樣，或剩下的項目註明查不到的原因與替代做法。
- [x] 1.2 `cockpit/Cargo.toml` 加入 `ureq`（rustls）與 `sha2`，附註解說明用途與 design D3。驗收＝`cargo tree -p cockpit -e normal`
  看得到兩者且沒有引入 native-tls／openssl；`cargo build --release -p cockpit --bins` 前後 `cockpit-launch.exe` 大小差異記入
  design D3；gate 全過。

## 2. 純邏輯（`cockpit/src/update.rs`，TDD）

- [x] 2.1 版本解析與比較：只接受 `X.Y.Z`（三段十進位、無前導 `v` 以外的字元、無後綴），`tag_name` 去 `v` 後解析；比較依主、次、
  修訂。驗收＝單元測試涵蓋 `v0.2.0>0.1.1`、`v0.1.10>0.1.9`、相同、較舊、`v0.2.0-rc.1`／`0.2`／`v0.2.0.1`／空字串／非數字被拒。
- [x] 2.2 latest release JSON 判定：輸入 HTTP 狀態碼與本體、目前版本，輸出「有新版（版本）」或「沒有新版（原因）」。驗收＝單元測試
  涵蓋 spec「判定有無新版」每一種情況（200 有新版、404、403、429、500、非 JSON、缺 `tag_name`、版本不大於、缺任一資產）。
- [x] 2.3 `SHA256SUMS.txt` 解析與比對：找檔名恰相符的行、64 位十六進位、不分大小寫比對；同名多行、格式錯誤、找不到皆為錯誤。
  驗收＝單元測試含 release.yml 實際格式的樣本（LF、結尾換行）與 CRLF、BOM、多一個空白、檔名為子字串等反例。
- [x] 2.4 更新檢查紀錄與節流：`cockpit.update.json` 的讀寫格式（design D5）與「是否已滿 24 小時」判定（不存在、讀不懂、時間在未來
  皆為已滿）。驗收＝單元測試涵蓋邊界（剛好 24 小時、差 1 秒、未來時間）。
- [x] 2.5 網址組裝與測試覆寫：查詢網址、固定下載網址（design D6），`COCKPIT_UPDATE_API_URL`／`COCKPIT_UPDATE_DOWNLOAD_BASE`
  覆寫（design D7）；未覆寫時只產生 `https://` 網址。驗收＝單元測試（環境以注入的 lookup 函式提供，同 `launch::plan` 做法）。
- [x] 2.6 訊息文字：詢問框、下載失敗、雜湊不符、啟動安裝檔失敗等加入 `LaunchText`，中英兩版，含新舊版本號與「將繼續使用目前
  版本」說明。驗收＝單元測試確認兩種語言都有、版本號有代入；既有語言測試仍過。

## 3. 啟動器串接（`cockpit/src/bin/cockpit-launch.rs`）

- [x] 3.1 在偵測為「連不上」之後、背景啟動之前插入更新檢查：判定 `unins000.exe`（design D2）、`COCKPIT_NO_UPDATE_CHECK`、節流；
  發查詢前寫紀錄、查詢 3 秒逾時、結果寫回紀錄；沒有新版時照常往下。驗收＝整合測試（本機 axum 假伺服器，啟動器執行檔複製到
  暫存目錄並放假的 `unins000.exe`；一律設假的 `COCKPIT_BROWSER` 與 `COCKPIT_LAUNCH_DIALOG_FILE`，見專案 memory「測啟動器時…」）
  涵蓋：zip 目錄不連線、關閉變數不連線、24 小時內不連線、404 照常啟動、離線（連到未監聽的埠）3 秒內照常啟動。
- [x] 3.2 詢問與下載驗證：詢問框（測試模式讀 `COCKPIT_LAUNCH_UPDATE_ANSWER`，預設 `no`）；同意後清空暫存子資料夾、下載
  `SHA256SUMS.txt` 與安裝檔（逾時與 200 MB 上限依 design D6）、比對雜湊；失敗時刪檔、顯示錯誤、照常啟動。驗收＝整合測試涵蓋選
  「否」不下載、雜湊不符（檔案被刪、未執行、錯誤訊息寫入對話框檔、後端照常啟動）、下載 404、通過驗證時下載檔沒有
  `Zone.Identifier` 資料流（design D8）。
- [x] 3.3 交棒：以 design D8 的參數啟動安裝檔（`Stdio::null()`、不等待），隨即以 0 結束、不啟動後端與瀏覽器；啟動失敗時顯示錯誤並
  照常啟動。驗收＝整合測試以一個只記錄自身引數的測試用假安裝檔（測試內建置，不進安裝包），斷言引數完全相符、啟動器結束碼 0、
  監聽埠上沒有後端、假瀏覽器沒有被呼叫；呼叫端以擷取輸出方式執行啟動器時不會被假安裝檔卡住。

## 4. 安裝檔與冒煙測試

- [x] 4.1 `packaging/ai-cockpit.iss`：依 design D9 加 `IsUpdateMode`、`PrepareToInstall` 的等候迴圈、更新模式的 `[Run]`；檔頭
  註解補上本 change。驗收＝4.2 的冒煙測試在 CI 通過（本機沒有 Inno Setup）。
- [x] 4.2 `packaging/smoke-test.ps1`：依 design D10 新增兩個情境並設定假瀏覽器、對話框檔、`COCKPIT_NO_UPDATE_CHECK`；既有 65 項
  不刪不改語意。驗收＝在 `feat/auto-update` 分支手動觸發 `release.yml`（workflow_dispatch，只跑第 1 到 4 步），冒煙測試全部
  PASS，log 中可見兩個新情境的斷言輸出；把 run 網址記入本 task。
  <https://github.com/Benjamin-Teng/ai-cockpit/actions/runs/37117792312>；最終審查修正波後重跑（14fd251，含 SHA256SUMS 格式
  斷言與 unins000.exe 檢查，88 項 PASS）：<https://github.com/Benjamin-Teng/ai-cockpit/actions/runs/37120082183>

## 5. 文件

- [x] 5.1 `README.md`、`README.zh-TW.md`：說明自動更新何時詢問、需要網路、`COCKPIT_NO_UPDATE_CHECK=1` 可關閉、zip 版不自動更新、
  v0.1.0 使用者需手動安裝一次新版。`CHANGELOG.md` 新增 `## [Unreleased]` 段落描述本功能（升版時由打包發佈 session 改成版本號）。
  驗收＝markdownlint 0 error；中英內容一致（Opus subagent 對照）。

## 6. 收尾

- [x] 6.1 全 gate 與審查：四項 gate 附輸出；整支分支交 Opus subagent 依 spec 與 design 做 diff 審查，findings 經實測後處理。
  驗收＝審查結論無未處理的 Critical／Important。
- [ ] 6.2 交接：合併回 main 後通知打包發佈 session，附 commit、`release.yml` 手動 run 網址，並請其在 rc 演練時依 design「Migration
  Plan」第 2 步做端到端演練（需在使用者同意的機器上，因為會覆寫既有安裝）。`docs/handover.md` 記 active change。驗收＝對方回覆
  收到。
