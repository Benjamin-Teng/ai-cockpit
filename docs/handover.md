# 交接：下一段任務

> **建立日期**：2026-10-07　|　**上一段做完的事**：v0.1.2（change `app-icon` 應用程式圖示＋執行檔版本資訊、底列改顯示程式版本、
> 推送即跑的 CI）與 v0.1.3（change `file-split-view` 檔案分頁並排，session ai-cockpit-3f 實作）兩版發布；v0.1.1 → v0.1.2
> 自動更新第一次在真機從 GitHub 更新成功；修掉讓 v0.1.2 首次發版失敗的測試時序問題。審查由 Opus 5.5 擔任（本專案視同 Codex）。
> **性質**：接手用文件，會過期，每段重寫。
> 為什麼做看 `docs/cockpit-spec.md`（北極星）與各 change 的 proposal；怎麼做看 `~/.claude/CLAUDE.md`（本 repo 精簡版在
> `AGENTS.md`）；規格看 `openspec/specs/`。

## 0. 三十秒版本

1. **使用者排定的候選工作（新 session 討論，尚未開 change）**：
   - **新增 project 不必手寫 `cockpit.toml`**（使用者 2026-10-04 提出：「對一般使用者太困難」）——第 2 節。
   - **自動更新加獨立簽章**（使用者 2026-10-06：「列入」）——第 3 節。
   - **修 `cockpit/tests/app.rs::abort_await_is_bounded` 的時序前提**（使用者 2026-10-06 交辦）——第 3 節。
2. **還沒問到的兩件事（沿用前幾段）**：① 通知真機確認（archive `2026-10-02-desktop-launch-notify` 未勾的 task 4.2）；② 啟動器
   模式下關窗後 agent 回報會遺失，可不可以接受（第 5 節第一列）。
3. repo **已公開** `https://github.com/Benjamin-Teng/ai-cockpit`；**每次推送前**跑去識別化檢查（第 1 節指令，0 命中才推）；**歷史不再改寫**。
4. **驗收腳本用固定 port，同時只能有一個 session 跑**（7770、7830 起一段、split-check 7910／CDP 19510）。多個 session 並行開發時先講好誰在用，
   撞 port 的 FAIL 看起來像偶發（memory `acceptance-scripts-fixed-ports-collide-across-sessions`）。
5. **發新版**：改 `cockpit/Cargo.toml` 的 `version`、`cargo update -p webpki-roots`、`cargo check`，連同 `Cargo.lock` 提交 → `CHANGELOG.md`
   的 `## [Unreleased]`（目前沒有這段，開發新功能時先在最上面建）改 `## [X.Y.Z] - Unreleased` → 推 `vX.Y.Z-rc.N` 演練 → 綠了改成日期、推 `vX.Y.Z`（推正式 tag 與刪 rc 前問使用者）→
   `gh release delete vX.Y.Z-rc.N --yes --cleanup-tag`。正式 tag 的 build 撞到偶發測試：`gh run rerun <id> --failed`，**不用刪 tag**。
6. **凍結契約**（改了已安裝的客戶端就全部無法自動更新）：資產名稱 `ai-cockpit-<X.Y.Z>-x64-setup.exe`／`-x64.zip`／`SHA256SUMS.txt`、
   `SHA256SUMS.txt` 格式、安裝檔參數 `/SILENT /SUPPRESSMSGBOXES /NORESTART /NOCANCEL /COCKPITUPDATE=1 /LOG=`、程式目錄的 `unins000.exe`。
   規格 `openspec/specs/release-distribution/`「自動更新客戶端契約」。

## 1. 現在的狀態

- **版本**：GitHub `releases/latest` = **v0.1.3**（2026-10-07）；v0.1.2（2026-10-05）、v0.1.1（2026-10-04）為一般 release，v0.1.0 為預發布。
  tag 只有 `v0.1.0`～`v0.1.3`（rc 草稿與 tag 都已刪）。
- **`main`**（`origin/main` 追蹤中）：本段 commit 由舊到新 `0f89f66`（CI）、`b7be30e`（app-icon）、`8bd9109`（升版 0.1.2）、`8afd835`（底列版本號）、
  `7f6ce36`（0.1.2 日期）、`44732ed`（測試時序修正）、`e1bbaa2`（archive app-icon）、`c3e25a1`／`fae506d`／`03b6c57`（file-split-view、archive、0.1.3）、本檔。
  worktree 只剩 `D:\projects\ai-cockpit`；本機與遠端都只有 `main`（已併回的分支經使用者同意於 2026-10-07 刪除）。
- **本機設定**：`.claude/settings.local.json`（由使用者全域 gitignore `~/.config/git/ignore` 忽略，repo 的 `.gitignore` 沒有這條，換機器要留意）只允許 `git push origin main` 與 `git push origin v*`（使用者授權）。
- **使用者電腦**：
  - 桌面捷徑「AI Agent Cockpit」→ `%LOCALAPPDATA%\ai-cockpit\bin\cockpit-launch.exe --config "D:\projects\ai-cockpit\cockpit.toml"`（`install-desktop.ps1`
    的安裝，**不會自動更新**；2026-10-07 已更新為 `main` 建的 0.1.3）。改了程式要換成最新：關 Cockpit 等 10 秒後
    `pwsh -File scripts\install-desktop.ps1`。
  - **安裝檔版本目前沒裝**（自動更新驗證後已解除安裝）。所以「v0.1.2 → v0.1.3 第二次真實派送」要觀察的話得先裝 v0.1.2 再從捷徑啟動。
  - 使用者自己的 `cockpit.toml` 是 `listen = "127.0.0.1:7770"`，開著時會擋到驗收腳本（第 0 節第 4 點）。
- **應用程式圖示**（archive `2026-10-05-app-icon`；規格 `release-distribution`「應用程式圖示與版本資訊」）：使用者從五個提案選「姿態儀」
  （提案與比較圖在 `docs/research/2026-10-04/icon-concepts/`）。母檔 `packaging/icon/app-icon.svg`、16–24 px 簡化版 `app-icon-small.svg`；
  `node packaging/icon/gen-icon.js` 以 headless Chrome 產生 `packaging/icon/app.ico` 與網頁 PNG（`cockpit/assets/icons/`、`site/img/icon-192.png`），
  產物提交、**改母檔要同一個 commit 重跑產生器**（沒有自動防呆）。`cockpit/build.rs`＋建置期相依 `winresource` 把圖示與版本資訊
  （ProductName／FileDescription「AI Agent Cockpit」、版本＝crate 版本）嵌進兩個執行檔，Windows 建置需要 Windows SDK 的 `rc.exe`；
  `.iss` 的 `SetupIconFile`；冒煙測試以 32 px 逐像素比對 `app.ico`（Inno 會原樣搬入 `.ico`，見 issrc `is-6_7_1` `UpdateIconsAndStyle`）。
- **底列版本號**：右下角顯示程式版本（`v` 加 crate 版本）。`index.html` 的 `<meta name="cockpit-version" content="__COCKPIT_VERSION__">` 由
  後端 `GET /` 送出前替換（`cockpit/src/http.rs` 的 `INDEX_HTML`）。投影的遞增 version 不顯示，放在 `#version` 的 `data-state-version`
  屬性——**驗收腳本以它判斷重畫**，寫新腳本照抄，不要讀 `#version` 的文字。
- **檔案並排**（archive `2026-10-07-file-split-view`；規格 `file-review`「檔案並排」等）：最多 3 個檔案分頁等寬並排，焦點欄替換規則、
  窄於 760 px 只顯示焦點欄、每個可見檔案分頁各自定時更新、並排組合存本機儲存。審查、17 條裁決、偶發紀錄、延後清單在該 archive
  的 `sdd-ledger.md` §4。真機驗收 `docs/research/2026-10-04/split-live.md`（Windows HERDR、2560 寬，6 項 PASS）。
- **CI／CD**：`.github/workflows/ci.yml`——推送任何分支或對 `main` 開 PR 時跑 Windows 上的 fmt／clippy／`cargo test --workspace`／
  ui_preview 範例測試（clippy、test、ui_preview 帶 `--locked`）與 Ubuntu 上的 markdownlint、`openspec validate`（版本與本機一致，升版兩邊
  一起改）；也可手動觸發（`workflow_dispatch`），推 tag 不觸發。`release.yml`
  推 tag 發版、`pages.yml` 推 `site/**` 發宣傳頁。本段沒有啟用 branch protection（單人 repo、直接 squash 推 `main`）。
- **自動更新**（規格 `auto-update`）：只有安裝檔版本會更新；從捷徑啟動且 Cockpit 沒在跑時最多每 24 小時查 `releases/latest`，問過使用者
  才下載、以同 release 的 `SHA256SUMS.txt` 驗證、交棒給安裝檔更新模式。**真機驗證通過**（2026-10-05，v0.1.1 → v0.1.2：無 SmartScreen、
  無 UAC、自動重開；`docs/research/2026-10-04/auto-update-e2e.md`）。**只驗 SHA-256、沒有獨立簽章**（第 3 節）。
- **發版、安裝檔、宣傳頁、介面語言、桌面啟動、去識別化**：做法與前一版本檔第 1 節相同（`git show 7e5af42:docs/handover.md`），規格在
  `openspec/specs/` 對應目錄。安裝檔不能在本機測（`smoke-test.ps1` 沒有 `CI` 會拒跑）。
- **可用指令**（repo 根目錄）：
  - 全 gate：`cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test --workspace && cargo test -p cockpit --example ui_preview && markdownlint-cli2 "**/*.md" && openspec validate --all`
  - 推送前：`node docs/research/2026-10-02/deid-check.js && node docs/research/2026-10-02/deid-check.js --history`
  - 預覽：`cargo run -p cockpit --example ui_preview`（`127.0.0.1:7770`）。
  - 驗收腳本（改了 `cockpit/assets/` 先 `cargo build -p cockpit --example ui_preview`；`reconnect-check.js` 要 `--examples`；不可並行、
    一次一支）：`docs/research/2026-09-15/reconnect-check.js`、`whatever-check.js`、`docs/research/2026-09-16/actions-check.js`、
    `channel-backoff-check.js`、`factory-floor-check.js`、`docs/research/2026-09-19/live-output-check.js`、`docs/research/2026-09-23/visual-check.js`、
    `docs/research/2026-09-27/files-check.js`、`docs/research/2026-09-28/git-check.js`、`docs/research/2026-10-01/progress-check.js`、`ui-fixes-check.js`、
    `docs/research/2026-10-02/output-color-check.js`、`notify-check.js`、`docs/research/2026-10-03/i18n-check.js`，本段新增
    `docs/research/2026-10-04/split-check.js`（47 段、每輪約 6～7 分鐘，用法 `split-check.md`）——以上 15 支是 file-split-view 收尾全綠的集合。
    另有 `docs/research/2026-10-02/idle-exit-check.js`（測啟動器、不走 ui_preview，不在上述集合）。跑完要還原會被覆寫的截圖：
    `factory-floor-check.js` → `git checkout -- docs/research/2026-09-16/task-5.2-scenario-d.png`；`i18n-check.js` 預設含第 5 段、重拍
    `i18n-en-*.png` → `git checkout -- docs/research/2026-10-03`（真的改了外觀才保留，並跑 deid-check、逐張看圖）。
    多數腳本可帶段落代號只跑一段（例如 `node docs/research/2026-09-23/visual-check.js S1,DF1`），偶發失敗先單段重跑。
  - 圖示：`node packaging/icon/gen-icon.js`（Chrome 路徑可用 `COCKPIT_ICON_CHROME` 指定）。
- **測試數字**（2026-10-07，file-split-view 收尾時；內容等同 `c3e25a1`，當時的分支 HEAD `a1e0cd3` 已 squash、日後可能查不到；當場跑為準）：
  workspace 1304 passed／0 failed／13 ignored；ui_preview 62；markdownlint 204 files 0 issues；`openspec validate --all` 24 passed；
  `visual-check.js` 1769 ok；上列 15 支驗收腳本全綠（`sdd-ledger.md` §5.1）。release 冒煙測試 104 PASS（v0.1.2 的 rc 與正式 release run）。

## 2. 立刻要做：新增 project 不必手寫 `cockpit.toml`（新 session）

**現況**（2026-10-06 向使用者說明過）：零設定模式只能看 HERDR 的 pane；要用 Factory Floor 必須手寫 `cockpit.toml` 的 `[[project]]`
（`stages`、`[[project.workstream]]` 的 `binding = { runtime, workspace, pane_label, cwd, agent }`、`[[project.task]]` 的 stage 與
`depends_on`），格式見 `cockpit.example.toml`。設定只在啟動時讀一次（`cockpit/src/config.rs` 的 `load`，沒有熱載入），改完要關窗等
10 秒再開；寫錯會啟動失敗並以訊息框說明。設定檔位置依啟動方式不同：`install-desktop.ps1` 的捷徑帶 `--config`，安裝檔版本讀
`%LOCALAPPDATA%\ai-cockpit\cockpit.toml`；狀態檔 `cockpit.state.json` 預設跟著設定檔目錄。

**做法**：使用者明說要在新 session 討論，所以先走 brainstorming，不要直接 propose。要先問清楚的事：

- 一般使用者心中的「project」是什麼：一個 repo？一個 HERDR workspace？
- 想在畫面上手動新增（表單），還是希望 Cockpit 從 HERDR 現有的 workspace／pane 自動產生？
- stages 與 tasks 要不要一開始就有，還是先只有 project＋workstream、之後再加？
- 畫面寫回設定檔還是另存一份「使用者設定」？手寫的 `cockpit.toml` 要不要繼續支援、兩者衝突時誰優先？

**牽涉的硬性約束**：對 HERDR 完全唯讀（AGENTS.md）；寫入類 HTTP 端點要過來源檢查（規格 `cockpit-dashboard`、archive
`2026-10-03-ws-source-check`）；`cockpit-core` 不得依賴其他 crate（ADR-0003）；新介面字串要加進 `i18n.js` 兩種語言、後端訊息要加
`Message` 變體（對帳測試會擋）。

## 3. 接著要做

### 3.1 自動更新加獨立簽章

**機制**：現在只驗「HTTPS＋同 release 的 `SHA256SUMS.txt`」，擋得住傳輸損毀與中間人，擋不住 release 本身被換掉——GitHub 帳號或 token
被盜、CI 被入侵時，攻擊者可同時上傳惡意安裝檔與對得上的雜湊檔，所有已安裝的客戶端都會被問要不要更新（archive `2026-10-03-auto-update`
design Risks 第一條）。獨立簽章是私鑰不放 GitHub、公鑰編進執行檔，客戶端驗不過就不裝。

**要先跟使用者決定**（白話說明後再問取捨，memory `user-prefers-plain-language-before-tradeoffs`）：

- 私鑰放哪：放 GitHub secrets 讓 CI 自動簽，帳號被盜時攻擊者仍能觸發 CI 簽章，保護打折；本機離線簽，每次發版多一個手動步驟。
- 私鑰遺失或外洩的換鑰方案（客戶端只認編進去的公鑰）。
- 已發出的 v0.1.1～v0.1.3 不驗簽：升到第一個帶公鑰的版本那一次仍只靠 SHA-256，之後才受保護。
- 簽章檔怎麼發布（新增資產會動到凍結契約之外的部分，舊客戶端要能忽略它）。

工具名稱與格式（例如 minisign）動手前查一手來源（鐵則 3）。

### 3.2 修 `abort_await_is_bounded` 的時序前提

CI run 37217313821（commit `8afd835`）失敗在 `cockpit/tests/app.rs:1206` 的前提檢查「那個 blocking task 在放行前確實收不掉」——
CI 高負載時這個前提本身不成立。與本段修過的 `loop_integration`（memory `fake-server-registers-connection-before-first-line`）同一類：
**測試對時序的假設在 CI 慢機上被打破，本機幾乎重現不了**。修法要讓前提改成「等到條件成立」而不是假設它立刻成立；修完在分支推 CI
看結果，修改前後各記錄能否重現。

## 4. 這一段踩過的坑

（**不會報錯的錯誤**加粗。）

- **測試假伺服器在 accept 時就登記連線，早於收到第一行**：「看到第 N 條連線就讀它的 request」在 CI 負載下偶發 panic，曾讓 v0.1.2 首次
  發版 build 失敗（memory 有）。
- **驗收腳本的固定 port 跨 session 互撞**，零星 FAIL 像 flaky（memory 有）。**收尾只看 PID 會誤報殘留**（PID 重用或 taskkill 後還在結束），
  先用 `Get-CimInstance` 查建立時間，不是自己開的 PID 不可砍（memory 有）。**iframe 內的 pointerdown 不冒泡**，焦點在兩個 iframe 間移動時
  父文件收不到事件（memory 有）。
- **改了底列文字，9 支驗收腳本的「判斷重畫」全部失效**（讀 `textContent.slice(1)`、`replace(/\D/g,'')` 等不同寫法，`Number("v0.1.2")` 得 NaN 或
  恆為 12，等到逾時才 FAIL）→ 改 DOM 文字前，用多種寫法 grep 所有讀取者，不要只搜一種字串。
- **SVG 的 XML 註解裡出現 `--`（例如寫 CSS 變數名）整份 SVG 無效**，Chrome 只回「圖片載入失敗」不說原因 → 註解不寫兩個連字號。
- 只看「圖示數量 ≥ 1」分不出自己的圖示和 Inno 預設圖示 → 取 32 px 圖示逐像素比對 `app.ico`（冒煙測試做法）；對照組用 `cargo.exe`（0 個）、`node.exe`（不同）。
- **工作目錄的文字檔是 CRLF**，用 LF 字串比對錨點會找不到 → 比對前依檔案換行轉換，或改用 Edit 工具。Git Bash 裡用 `node -e "…"` 或 heredoc
  寫含反引號、反斜線的內容會被 shell 改寫（反引號被當指令替換、`\\` 變 `\`）→ 這類內容一律用 Edit／Write 工具。
- `gh release download` 在網速慢時兩個大檔停在 0 位元組像卡住 → `curl -L` 下載 `releases/download/<tag>/<asset>`（或帶 token 打 asset API）。
- agent 的背景工作預設 30 分鐘時限會砍掉長駐程序（例如假更新伺服器）；容器重啟也會砍掉背景 subagent → 長等待給長時限，重啟後重新派工。
- 執行檔的修改時間是建置時間（Inno 保留內嵌檔案時間戳），不能用來判斷自動更新是否覆寫 → 看 `VersionInfo` 與程序啟動時間。
- 重新開啟的啟動器會清掉 `%TEMP%\ai-cockpit-update\` 下其他 PID 的資料夾，安裝檔 `setup.log` 隨之消失（設計行為）→ 要保留得在 Cockpit 重開前複製。
- 既有偶發（單段重跑一次並記錄）：`git-check.js`「收尾衛生」、`factory-floor-check.js`、`live-output-check.js` 的 L／R／S 段收尾、
  `reconnect-check.js`（Chrome 20 秒內沒出現 page target）、`actions-check.js` 的 Enter 送出兩次 POST、`visual-check.js` 的 chrome-P1（原因未查）、
  `tests/app.rs::abort_await_is_bounded`（第 3.2 節）。

更早仍有效的坑：前一版本檔第 3 節（`git show 7e5af42:docs/handover.md`），再往前用 `git log --format=%h -- docs/handover.md` 逐版查。

## 5. 已定但未執行的決策

| 決策 | 狀態 |
|---|---|
| 啟動器模式下關窗後 agent 回報遺失（change 11 整支審查 I1） | Claude 依授權維持「關窗即結束」並文件化；**待使用者裁決**（替代：後端常駐＋系統匣，或 agent 端回報失敗重試） |
| 通知的真機確認（change 11 task 4.2） | **待使用者操作** |
| 新增 project 免手寫 toml、自動更新簽章、`abort_await_is_bounded` | **使用者已排定**，新 session 處理（第 2、3 節） |
| 刪除已併回的分支 | **已完成**（2026-10-07 使用者同意）。之後刪分支或 tag 仍會被權限擋，要使用者明確下指令 |
| 程式碼簽章（Authenticode）、Windows 以外的安裝檔、Tauri | **不做**；Tauri 使用者 2026-10-03 再次確認不包，有常駐／系統匣需求才重評 |
| 圖示：提案 B「姿態儀」、16–24 px 用簡化版 | **使用者 2026-10-04 決定** |
| 底列改顯示程式版本、併進 v0.1.2 | **使用者 2026-10-05 決定** |
| 母檔改了卻沒重跑產生器的防呆 | **不做**（換行依平台不同，雜湊比對成本高於收益；改為流程規範，archive `2026-10-05-app-icon` design Risks） |
| file-split-view：三欄不設最小欄寬；停用的並排鈕維持顯示 | **使用者 2026-10-06 裁決**（proposal 非目標） |
| file-split-view 延後項：鍵盤移出並排後焦點停在 tabindex=-1 按鈕、`reading-flow` 只有 Chrome／Edge 137+、重新整理後各欄捲動歸零、窄於 760 px 時 scrollIntoView 目標被狀態列蓋住（WCAG 2.4.11，既有問題，建議另開 change）、焦點 iframe 重建時漏一次切換 | **延後**（使用者已知；`2026-10-07-file-split-view/sdd-ledger.md` §4.6） |
| 授權 MIT；Claude 在 feature 分支 commit、收尾 squash 併回 `main` 並推送 | **已定／已授權** |
| 更早的延後項（change 11 m5／m6、3.6 M6、change 8 與更早）、`real_attach` 待真機實跑、v2 狀態檔缺 `active` 不修 | 見前一版本檔第 4 節 |

## 6. 之後的路

北極星是讓一個人同時盯多個 coding agent 的工作狀態（`docs/cockpit-spec.md`）。產品已能安裝、自動更新、看 pane、看進度、看檔案與
git 變更；最大的缺口是**一般使用者上手**——第 2 節的 project 設定，以及 agent 回報進度要靠 `curl` 指令（README「Let agents report
progress」）。發版面的缺口是第 3.1 節的更新簽章與 Authenticode（SmartScreen 首次警告）。

## 版本紀錄

| 版本 | 日期 | 變更 |
|---|---|---|
| 1–10 | 2026-09-13～15 | change 1a／1b |
| 11–14 | 2026-09-15～19 | change 2 |
| 15–17 | 2026-09-21～23 | change 3 `live-output` |
| 18–20 | 2026-09-23～26 | change 4 `direction-01-visual` |
| 21–23 | 2026-09-27～28 | change 5a `file-review` |
| 24–25 | 2026-09-28 | 小 change `html-charset` |
| 26–27 | 2026-09-29～10-01 | change 5b `git-review`（Sonnet 驗收、未經 Codex）併回並 archive |
| 28–29 | 2026-10-01 | change 6 `progress-model`（Opus 審查取代 Codex）併回並 archive |
| 30–31 | 2026-10-01 | change 7 `ui-fixes`（Opus 審查取代 Codex）併回並 archive |
| 32 | 2026-10-02 | change 8 `live-output-color` 併回並 archive |
| 33–34 | 2026-10-02 | change 9 `deidentify`（改寫 git 歷史）；建立 GitHub repo 並推送 |
| 35 | 2026-10-02 | repo 公開；change 10 `output-color-tuning` |
| 36 | 2026-10-02 | change 11 `desktop-launch-notify`（啟動器、關窗即結束、桌面通知）併回並 archive；兩份備份已刪除 |
| 37 | 2026-10-02 | 宣傳頁（GitHub Pages）、README 中英雙語與生成藝術橫幅；Opus 審查兩輪 |
| 38 | 2026-10-03 | change `ws-source-check`（m1）：`/api/state`、`/ws` 來源檢查，`listen` 收緊為 `127.0.0.1`／`::1`、埠不得為 80，`GET /` 防嵌入；Opus 審查兩輪 |
| 39 | 2026-10-03 | change `ui-language`（介面中英切換、後端訊息代碼、啟動器語言）SDD 完成，Opus 兩段階段審查＋整支審查；Tauri 確認不包；release 交給打包 session |
| 40 | 2026-10-03 | change `release-packaging`：Inno Setup 安裝檔＋zip、release workflow（rc 演練三輪後全綠、版本不一致演練如預期失敗）、宣傳頁下載區塊、MIT、CHANGELOG、釘 Rust 1.97.1 |
| 41 | 2026-10-03 | change `auto-update`（ai-cockpit-05 實作）併回並 archive；v0.1.0 改標預發布；v0.1.1 發版準備（版本、CHANGELOG、凍結契約） |
| 42 | 2026-10-07 | v0.1.1（真機驗證假伺服器）、CI（`ci.yml`）、change `app-icon`＋底列版本號＋測試時序修正 → v0.1.2，v0.1.1 → v0.1.2 真實自動更新通過；change `file-split-view`（ai-cockpit-3f）→ v0.1.3；兩個 session 的交接由本 session 統一重寫 |
