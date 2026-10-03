# tasks：release-packaging

> 執行路徑：直接 apply（不走 SDD）｜理由：不改任何 Rust 程式，改動集中在打包腳本、workflow 與宣傳頁，任務線性；驗收靠
> 預發布演練在 runner 上的實跑結果。品質 gate 與審查（本專案 Opus 視同 Codex）照常。

通則：在 worktree `D:\projects\ai-cockpit-release`（分支 `release-packaging`）作業，不動主工作目錄。不改 `.rs`，所以每個 task
不需跑 cargo gate（release workflow 本身會跑）；改 `.md` 在 repo 根跑 `markdownlint-cli2 "**/*.md"`。不在本機執行安裝檔或
`packaging/smoke-test.ps1`（使用者電腦已有腳本安裝與同名捷徑）。推送分支與 tag 前先跑
`node docs/research/2026-10-02/deid-check.js && node docs/research/2026-10-02/deid-check.js --history`（0 命中），並取得使用者同意。

## 1. 授權與版本說明

- [x] 1.1 新增 `LICENSE`（MIT 全文，著作權人 `Benjamin-Teng`，年份 2026）與 `CHANGELOG.md`（Keep a Changelog，含
  `## [0.1.0] - Unreleased` 段落，摘要 MVP 至今的使用者可見功能）；驗收＝`markdownlint-cli2 "**/*.md"` 0 issues

## 2. 安裝檔

- [x] 2.1 `packaging/ai-cockpit.iss`：依 design D1、D2、D7、D9 與 spec「Windows 安裝檔」「解除安裝」撰寫（`AppId` 用固定的
  非 GUID 字串、版本以 `-dAppVersion=` 傳入、缺少時編譯失敗、檔案來源目錄以 `-dStageDir=` 傳入）；驗收＝檔案自我檢查清單
  逐項對照 spec 條目（實際編譯在 3.2 演練）
- [x] 2.2 `packaging/smoke-test.ps1`：依 design D6 實作安裝→檢查→啟動→執行中重跑安裝檔預期中止→執行中解除安裝預期中止→
  解除安裝→檢查，加上 zip 與安裝結果的 SHA-256 比對；沒有 `CI` 環境變數時拒跑；每項檢查失敗時印出期望值與實際值並以非 0 結束；
  驗收＝`pwsh -NoProfile -Command "[System.Management.Automation.Language.Parser]::ParseFile(...)"` 無語法錯誤，且不帶 `CI` 執行時拒跑

## 3. Release workflow

- [x] 3.1 查 `actions/upload-artifact`、`actions/download-artifact` 最新版號（各 repo releases 頁），寫進 design「外部精確資訊」；
  `.github/workflows/release.yml` 依 design D3、D5 實作，build job 先印 `rustc -V`、`iscc` 版本與 Inno `Languages\` 目錄內容；
  驗收＝`actionlint` 若本機可用則 0 error，否則以 Opus subagent 對照 design D5 逐項檢查
- [x] 3.2 預發布演練：使用者同意後推送分支與 `v0.1.0-rc.1`，跑到 build 與 release 兩個 job 全綠；失敗時依
  systematic debugging 找根因修正後遞增 rc 重打（舊 rc 的草稿與 tag 刪除）；依 log 決定 D7 繁中語言檔的處理並補上；
  驗收＝`gh run view` 兩個 job 成功、`gh release view v0.1.0-rc.N` 為預發布草稿且三個資產齊全、冒煙測試 log 每項檢查都有
  PASS；下載安裝檔與 zip 核對 `SHA256SUMS.txt`；確認 D2 兩條中止路徑的結束碼與 design 記載相符；完成後刪除演練草稿與 rc tag
  （本地與遠端）
- [x] 3.3 版本不一致演練：在同一 commit 推 `v0.0.9-rc.1`（與 crate 版本 `0.1.0` 不符）；驗收＝build job 在版本比對步驟失敗、
  訊息指出兩個版本、`gh release list` 沒有新草稿；之後刪除該 tag（本地與遠端）

## 4. 宣傳頁

- [x] 4.1 `site/`：依 spec「宣傳頁下載區塊」與 design D8 新增 `#download` 區塊（`#start` 之前）、頁首導覽與首屏主要按鈕
  連到它，`site.js` 取最新 release，`site/i18n.js` 補齊繁中鍵（鍵與 `data-i18n` 一一對應），`#start` 的開場改成「下載安裝檔或
  從原始碼建置」；驗收＝以 headless Playwright 開本機靜態伺服器上的 `site/`，分別攔截 API 回應為 (a) 一筆含兩個資產的
  release、(b) 404、(c) 網路錯誤、(d) release 存在但資產檔名不符預期，檢查版本號、日期與按鈕網址符合 spec 情境，並在中英兩種語言下截圖確認；腳本放
  `docs/research/2026-10-03/`
- [x] 4.2 宣傳頁外觀過 `frontend-design` skill 的設計審核（專案 memory），只調整新增的下載區塊；驗收＝審核意見已處理或說明不採理由

## 5. 等 `ui-language` 併回 main 之後

- [x] 5.1 rebase 到 main（`ui-language` 已 squash 併入），衝突逐檔處理；驗收＝`git log --oneline main..` 只有本 change 的 commit，
  主工作目錄不受影響
- [x] 5.2 `README.md`、`README.zh-TW.md`「開始使用」改成先下載安裝檔（含 SmartScreen 說明、資料目錄放 `cockpit.toml`、從腳本安裝
  遷移的說明），原始碼建置保留為第二條路；`cockpit/README.md` 安裝腳本節補一句與安裝檔的關係；`CHANGELOG.md` 補介面語言切換一條；驗收＝`markdownlint-cli2` 0 issues
- [x] 5.3 完整 gate：`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test --workspace`（確認 rebase 後的
  main 程式仍綠）、`markdownlint-cli2 "**/*.md"`；再推 `v0.1.0-rc.N` 演練一次確認安裝檔包含介面語言功能（冒煙測試綠）；
  驗收＝輸出附在回報中
- [x] 5.4 審查：以 Opus 5.5 subagent 對整支分支 diff 做對抗式審查（本專案 Opus 視同 Codex），findings 實測後才採信並修正；
  驗收＝審查結論與處理記錄
- [x] 5.5 `docs/handover.md` 重寫本段（發版流程、版本 bump 步驟、未簽章、演練方式）；驗收＝`markdownlint-cli2` 0 issues

## 6. 發布 v0.1.0

- [x] 6.1 使用者同意後 squash 併回 main 並推送，確認 Pages 部署成功、宣傳頁在尚無 release 時下載按鈕連到 Releases 頁；驗收＝
  `gh run list --workflow pages.yml` 最新一筆成功
- [x] 6.2 在 main 手動觸發 release workflow（`gh workflow run release.yml --ref main`）；驗收＝build job 成功、產物可下載、
  `gh release list` 沒有新增 release
- [ ] 6.3 `CHANGELOG.md` 填上發布日期並提交，推 tag `v0.1.0`；驗收＝`gh release view v0.1.0` 為公開最新版、三個資產齊全，
  宣傳頁顯示 `v0.1.0` 與日期、下載按鈕下載到的安裝檔 SHA-256 與 `SHA256SUMS.txt` 相符
- [ ] 6.4 archive 本 change（`/opsx:archive`）
