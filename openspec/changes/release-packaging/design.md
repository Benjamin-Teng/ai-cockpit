# design：release-packaging

## Context

- 發行物只需要 `cockpit` 套件的兩個執行檔：前端資產全部以 `include_str!`／`include_bytes!`／`include_dir!` 內嵌
  （`cockpit/src/http.rs`、`cockpit/src/vendor.rs`），`cockpit/README.md`「單一執行檔」節已說明複製到任何目錄都能跑；
  `cockpit-launch` 啟動同目錄的 `cockpit.exe`。設定檔是選用的（`cockpit-config`「設定檔位置與零設定模式」）。
- 原始碼流程的 `scripts/install-desktop.ps1` 裝到 `%LOCALAPPDATA%\ai-cockpit\bin\`，零設定時捷徑工作目錄為
  `%LOCALAPPDATA%\ai-cockpit\`（`desktop-launch`「桌面捷徑安裝腳本」）。
- 各 crate 的 `version` 各自寫死為 `0.1.0`，沒有 `[workspace.package] version`；程式不讀 `CARGO_PKG_VERSION`。
- 宣傳頁 `site/` 由 `pages.yml` 原樣發布，沒有建置步驟，`site.js` 目前沒有任何對外請求；英文寫在 HTML，繁中在
  `site/i18n.js`，以 `data-i18n` 鍵對應。
- 本機沒有 Inno Setup；GitHub `windows-latest`（Windows Server 2025）映像預裝 Inno Setup 6.7.1（`iscc` 在 PATH）與 Rust
  stable（msvc、clippy、rustfmt）。查證來源與日期見本檔「外部精確資訊」。
- 單人 repo、不走 PR（使用者的分支慣例）；本 change 與進行中的 `ui-language` 平行，`README*`、`docs/handover.md` 等
  兩邊都會改的檔案留到 `ui-language` 併回 main 之後才動。

## Goals / Non-Goals

**Goals:**

- 安裝檔的測試在 GitHub runner 的乾淨環境跑，不碰使用者電腦上既有的安裝與捷徑。
- 發新版只需要：改 crate 版本、寫 `CHANGELOG.md` 段落、推 tag；宣傳頁不必跟著改。

**Non-Goals:**

- 不把版本號收斂成 workspace 單一來源（會動到六個 `Cargo.toml`，與 `ui-language` 衝突；由 workflow 的一致性檢查兜住）。
- 不提供 `PrivilegesRequiredOverridesAllowed`（不讓使用者改成所有使用者安裝）。

## Decisions

### D1 安裝位置：`{autopf}\AI Agent Cockpit`，資料目錄沿用 `%LOCALAPPDATA%\ai-cockpit\`

`PrivilegesRequired=lowest` 時 `{autopf}` 對映 `{userpf}`，即 `%LOCALAPPDATA%\Programs\`，是 Windows 上 per-user 程式的慣例
位置，與「設定 > 應用程式」顯示一致。程式與資料分開：解除安裝刪程式目錄即可，不必區分哪些是使用者檔案。資料目錄沿用安裝
腳本零設定時的 `%LOCALAPPDATA%\ai-cockpit\`，兩條安裝路線的使用者把 `cockpit.toml` 放同一處就會被讀到（工作目錄的
`cockpit.toml` 規則），捷徑因此不需要 `--config`。

- 替代：裝到 `%LOCALAPPDATA%\ai-cockpit\bin\`（與腳本相同）。否決：程式與資料混在同一棵樹，解除安裝要逐檔判斷；而且會和
  使用者以腳本裝的那份互相覆寫，腳本版捷徑帶的 `--config` 也會被蓋掉。
- 後果：曾用腳本安裝的人改用安裝檔時，桌面同名捷徑會被安裝檔取代（不再帶 `--config`），舊的 `ai-cockpit\bin\` 不會被刪。
  README 說明把 `cockpit.toml` 連同 `cockpit.state.json` 移到資料目錄、刪掉舊 `bin\`：狀態檔預設在設定檔目錄
  （`cockpit/src/config.rs` 的 `resolve_state_path`），只搬設定檔會無聲地從空白看板開始。

### D2 執行中偵測：以 `[Code]` 查詢程序路徑，不用 Restart Manager、不加 AppMutex

規格要求執行中「停止並提示，不強制結束」，與 `install-desktop.ps1` 一致。Inno 的 `CloseApplications`（Restart Manager）會
提議關閉程式，且官方文件只描述安裝時的行為，解除安裝時沒有保證。`AppMutex` 需要 `cockpit` 程式自己建立具名 mutex，要改
`cockpit/src`（與 `ui-language` 衝突，也為了打包改程式行為）。

做法：`CloseApplications=no`；`[Code]` 以 WMI（`Win32_Process`，比對 `ExecutablePath` 是否在 `{app}` 下且檔名為兩個執行檔
之一）判斷，安裝在 `PrepareToInstall` 回傳提示字串中止，解除安裝在 `InitializeUninstall` 以 `SuppressibleMsgBox`（受
`/SUPPRESSMSGBOXES` 控制；一般 `MsgBox` 不受控，靜默模式會卡在訊息框直到 CI 逾時）提示後回 `False`。靜默模式下同樣中止。
結束碼：安裝中止為 7（Preparing to Install 判定無法繼續，jrsoftware.org/ishelp/topic_setupexitcodes.htm）；解除安裝中止為 1
（Inno 6.7.1 `Projects/Src/Setup.Uninstall.pas`：預設 1，只有移除成功才設 0）。冒煙測試據此斷言，預發布演練實證。
Inno 會還原內嵌檔案的時間戳，所以「沒覆寫」不能靠修改時間判斷，改看結束碼與 log 中有沒有檔案複製紀錄。
WMI 查詢本身失敗時視同「無法確認」而中止（spec 已要求）。安裝檔是 32 位元程序，`ExecutablePath` 對 64 位元程序是否可取得
由冒煙測試的「執行中重跑安裝檔」項目實證。

- 替代：`[UninstallRun]` 跑 `taskkill`。否決：違反「不強制結束」。

### D3 版本：tag 為觸發，crate 版本為準，workflow 比對一致性

workflow 從 `GITHUB_REF_NAME` 解析 `vX.Y.Z[-pre]`，以 `cargo metadata --no-deps` 取 `cockpit` 套件版本比對，不符就失敗
（fail closed）。版本以 `iscc -dAppVersion=<X.Y.Z>`（ISPP 命令列定義）傳入 `.iss`，用於 `AppVersion`、
`VersionInfoVersion`、`OutputBaseFilename`。手動觸發時版本取 crate 版本。

- 替代：版本只存在 tag、建置時注入（多人 repo 的慣例）。否決：本 repo 單人、手動 bump 版本；crate 版本不改就會出現安裝檔
  寫 0.2.0、程式內部仍 0.1.0 的分歧。

### D4 預發布 tag 即演練

`workflow_dispatch` 只能觸發位於預設分支上的 workflow 檔，所以在併回 main 前無法手動演練。改以預發布 tag
（`v0.1.0-rc.N`，在 `release-packaging` 分支上打）跑完整流程，最後停在預發布草稿，連 release job 的權限、說明擷取、上傳都
一起驗到；人工檢查後刪掉草稿與 tag。併回 main 之後，`workflow_dispatch` 提供不建 release 的建置演練。

### D5 workflow 結構

- 觸發：`push: tags: ["v*"]`、`workflow_dispatch`。
- `build` job（`windows-latest`，`contents: read`，`timeout-minutes: 60`）：checkout → 版本解析與比對 → fmt／clippy／test →
  `cargo build --release -p cockpit --bins` → 組 staging 目錄（兩個 exe、`cockpit.example.toml`、`LICENSE`）→ 壓 zip → `iscc`
  建安裝檔 → 冒煙測試（D6）→ `SHA256SUMS.txt` → `actions/upload-artifact`。
- `release` job（`needs: build`，只在 push tag 事件（手動觸發選 tag 也不跑），`ubuntu-latest`，`contents: write`，`timeout-minutes: 15`，job 層 `env` 設
  `GH_TOKEN: ${{ github.token }}`、`GH_REPO: ${{ github.repository }}`）：checkout（讀 `CHANGELOG.md`）→ 下載產物並 `sha256sum -c` →
  從 `CHANGELOG.md` 擷取該版段落（擷取不到、或正式版標題仍是 `Unreleased` 就失敗）→ 已有同 tag release 就失敗 → `gh release create <tag> <三個檔> --draft --verify-tag --title <tag>
  --notes-file <段落>`（預發布 tag 加 `--prerelease`）→ 以 `gh release view --json assets` 確認三個資產名稱都在 → 正式版
  `gh release edit <tag> --draft=false --latest`。
- `concurrency: release-${{ github.ref }}`，`cancel-in-progress: false`。
- Rust 版本由 repo 根的 `rust-toolchain.toml` 釘住（含 clippy、rustfmt），workflow 先 `rustup toolchain install --no-self-update` 再建置；
  不用 runner 預裝的 stable。rc.1 演練時 runner 是 1.98.1、本機 1.97.1，新 lint `chunks_exact_to_as_chunks` 只在 CI 出現而讓
  Clippy 失敗；釘版後本機、CI 與其他 session 同版，升版是有意識的一個 commit（改檔、修新 lint、跑完整 gate）。`rustc -V` 印在
  log 以利追溯。clippy／test／build 都加 `--locked`：發布的執行檔一定用提交的 `Cargo.lock` 建置，改版本忘了更新 lock 就失敗。
- 正式 tag 的 CHANGELOG 段落存在且標題不是 `Unreleased`，在 build job 的版本解析步驟就檢查（不必等 60 分鐘的建置跑完才失敗）。
- release job 的 `GH_TOKEN` 只設在三個呼叫 `gh` 的 step。
  以 `Swatinem/rust-cache` 快取 `target/`。action 一律釘到完整版號（沿用 `pages.yml` 的寫法）。
- markdownlint 不在 release 流程跑：它不影響產出物，且本地 gate 已涵蓋。

### D6 冒煙測試在 runner 上跑真的安裝與解除安裝

runner 是用完即丟的乾淨 Windows，以 `/VERYSILENT /SUPPRESSMSGBOXES /NORESTART` 安裝。順序與檢查：

1. **安裝**（預設選項）：檔案、資料目錄、開始功能表與桌面捷徑（以 `WScript.Shell` 讀 `.lnk` 的 `TargetPath`、
   `WorkingDirectory`、`Arguments`）、`HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\<AppId>_is1` 的
   `DisplayName`、`DisplayVersion`、`InstallLocation`。程式目錄在 `%LOCALAPPDATA%\Programs\` 下且登錄在 HKCU，證明即使
   runner 以管理員身分執行仍是 per-user 安裝；「一般使用者不出現 UAC」在 runner 上無法直接觀察，由
   `PrivilegesRequired=lowest` 保證。
2. **沒有殘留程序**：確認沒有任何 `cockpit*.exe` 在執行（防止 `[Run]` 漏了 `skipifsilent`，讓啟動器的後端佔住 7770 而使下一步
   假 PASS）。
3. **啟動**：以資料目錄為工作目錄背景啟動已安裝的 `cockpit.exe`，輪詢 `GET http://127.0.0.1:7770/` 到 200（逾時 30 秒失敗），
   並確認佔住 7770 的正是這個程序。
4. **執行中**：重跑安裝檔預期結束碼 7 且 log 沒有檔案複製紀錄；解除安裝預期結束碼非 0；兩者之後檔案雜湊、捷徑、登錄不變，
   `cockpit.exe` 仍在執行（D2）。之後停掉 `cockpit.exe`。
5. **覆蓋更新**：再跑一次安裝檔，程式目錄不變、Uninstall 機碼仍只有一筆。
6. **解除安裝（資料目錄為空）**：Inno 的解除安裝程式會在 TEMP 產生副本執行實際移除。PowerShell 7 的 `Start-Process -Wait`
   會等整個程序樹，另外仍輪詢到程式目錄、捷徑、登錄都消失（期限 60 秒）才判定；結束碼為 0；資料目錄仍在。
7. **不建桌面捷徑**：以 `/MERGETASKS="!desktopicon"` 安裝，只有開始功能表捷徑。
8. **解除安裝（資料目錄有 `cockpit.toml`）**：放一份 `cockpit.toml` 後解除安裝，同第 6 步輪詢；資料目錄與 `cockpit.toml` 仍在。
9. **zip**：根目錄恰好四個預期檔案；兩個 exe 的 SHA-256 與第 1 步裝出的相同。腳本放 `packaging/smoke-test.ps1`，本機也能對任意安裝檔執行（會動到
執行者自己的安裝，所以只在 runner 或拋棄式環境跑，腳本開頭檢查 `CI` 環境變數，沒有就拒跑，可用參數明確覆寫）。

### D7 安裝精靈細節：完成頁工作目錄、資料目錄、語言

`[Run]` 的完成頁啟動項目明確設 `WorkingDir` 為資料目錄（預設是程式目錄：讀不到資料目錄的 `cockpit.toml`，
`cockpit.log` 也會寫進程式目錄、解除安裝時殘留）。資料目錄以 `[Dirs]` 建立並加 `uninsneveruninstall`（預設會在解除安裝時刪除
空目錄）。

英文（`compiler:Default.isl`）與繁體中文，依系統語言自動選。rc.1 演練的 log 顯示 runner 的 Inno Setup 6.7.1 `Languages\`
沒有繁中，所以 `packaging/ChineseTraditional.isl` 取自 Inno 原始碼 `jrsoftware/issrc` tag `is-6_7_1` 的
`Files/Languages/Unofficial/`（blob `b8a50d595fe5dda3308fcbf27c4ce8ffddb2c9cc`，與下載檔 `git hash-object --no-filters`
相符）；Inno 主版本升級時一併換成對應 tag 的檔案。「執行中」提示的兩則自訂訊息另有繁中版。

### D8 宣傳頁取版本：瀏覽器端呼叫 GitHub REST API

`site.js` 在載入時 `fetch("https://api.github.com/repos/Benjamin-Teng/ai-cockpit/releases/latest")`（文件明載支援任何來源的
CORS，未認證每 IP 每小時 60 次，`latest` 排除草稿與預發布）。從 `assets` 依檔名規則（`-x64-setup.exe`、`-x64.zip` 結尾）
找下載網址，填入按鈕與版本／日期；任何失敗（非 2xx、例外、找不到資產）保留 HTML 裡預設的 Releases 頁連結，只在主控台記錄。
動態文字（版本號、日期）放在沒有 `data-i18n` 的元素，避免和 `site.js` 的 `data-en` 原文快取互相覆寫；日期以 `YYYY-MM-DD`
顯示，不隨語言變。

- 替代一：用 `releases/latest/download/<固定檔名>` 永久網址。否決：檔名就不能帶版本號，而且頁面仍需另取版本號。
- 替代二：release workflow 發布後改寫 `site/` 並推回 main。否決：workflow 要有推 main 的權限，且每次發版多一個 commit。

### D9 授權與 CHANGELOG

`LICENSE` 用 MIT 標準全文，著作權人 `Benjamin-Teng`。`CHANGELOG.md` 採 Keep a Changelog 格式，段落標題
`## [X.Y.Z] - <日期>`；發布前日期寫 `Unreleased`，推正式 tag 前改成 `YYYY-MM-DD`。workflow 只比對 `## [X.Y.Z]` 前綴（不管日期），
擷取到下一個 `## [` 之前；預發布 tag 用去掉後綴的版本找段落，所以演練時日期未定也擷取得到。安裝檔與 zip 內的授權文字為 `LICENSE` 的副本
（安裝檔另以 `LicenseFile` 在精靈中顯示）。

## Risks / Trade-offs

- [未簽章，SmartScreen 警告與部分防毒誤判] → 宣傳頁與 README 說明「其他資訊 → 仍要執行」；提供 `SHA256SUMS.txt` 供核對。
- [runner 映像更新換掉 Inno Setup 主版本（例如 7.x 的預設值變動）] → log 印 `iscc` 版本；`.iss` 只用 6.x 與 7.x 皆支援的指令，
  主版本變動時以預發布演練確認。
- [測試在 runner 上與本機行為不同（逾時、路徑）而失敗] → 第一次預發布演練就會現形；依 systematic debugging 找根因，不放寬
  或跳過測試。
- [WMI 在受限環境不可用] → 查詢失敗時視為「無法確認」並中止，提示使用者先關閉 Cockpit 後再試；不在不確定時覆寫。
- [同一台電腦上另一位使用者開著 Cockpit] → 一般權限讀不到他人程序的 `ExecutablePath`，判為「無法確認」而擋住安裝與解除安裝。
  接受：fail-closed 是刻意的，情境罕見；請對方關閉 Cockpit 即可。
- [GitHub API 速率限制（每 IP 每小時 60 次）] → 失敗時退回 Releases 頁連結，功能不中斷。
- [使用者電腦已有腳本安裝的同名桌面捷徑] → D1 後果段；README 說明遷移。

## Migration Plan

1. 本分支完成 `packaging/`、workflow、`LICENSE`、`CHANGELOG.md`、`site/`，以 `v0.1.0-rc.N` 演練到全綠，刪除演練草稿與 tag。
2. `ui-language` squash 併回 main 後，本分支 rebase 到 main，再更新 README 與 handover，跑完整 gate 與審查。
3. 使用者同意後併回 main 並推送（`site/` 變動觸發 Pages 部署；此時尚無正式 release，下載按鈕退回 Releases 頁）。
4. 寫上 `CHANGELOG.md` 的發布日期，推 `v0.1.0`，確認 release 公開、三個產出物齊全、宣傳頁顯示 `v0.1.0`。
5. 回退：release 有問題不刪已公開的版本，修正後發 `v0.1.1`；宣傳頁問題以一般 commit 修正。

## 外部精確資訊（查證日期 2026-10-03）

- runner：`windows-latest` 為 Windows Server 2025，預裝 Inno Setup 6.7.1、`iscc` 在 PATH、Rust 1.98.1（stable msvc，含 clippy、
  rustfmt）——github.com/actions/runner-images（`images/windows/Windows2025-Readme.md`、`toolsets/toolset-2025.json`、
  `scripts/tests/ChocoPackages.Tests.ps1`、`scripts/build/Install-Rust.ps1`）。
- Inno Setup：`PrivilegesRequired=lowest`、`{autopf}`／`{autodesktop}`／`{autoprograms}` 於 non-admin 對映使用者層、
  `ArchitecturesAllowed`／`ArchitecturesInstallIn64BitMode=x64compatible`、`OutputBaseFilename` 勿取名 `setup`、`[Run]`
  `postinstall`、`AppMutex`、`CloseApplications`、`InitializeUninstall`——jrsoftware.org/ishelp 各 topic 頁；ISPP 命令列
  定義為 `-d<name>=<value>`／`--define=`（topic_isppcc.htm）。最新穩定版 7.1.0、6 系列 6.7.3（jrsoftware.org/isdl.php）。
- GitHub：`/releases/latest/download/<asset>` 固定連結（docs.github.com「Linking to releases」）；REST `releases/latest` 為
  最新非草稿、非預發布，支援任何來源 CORS，未認證每小時 60 次；`gh release create --draft --verify-tag --prerelease
  --notes-file`、`gh release edit --draft=false`（cli.github.com/manual）；建 release 需 `contents: write`。
- action 版號（各 repo `releases/latest`）：`actions/checkout` v7.0.1、`Swatinem/rust-cache` v2.9.2、`actions/upload-artifact` v7.0.1、
  `actions/download-artifact` v8.0.1。
