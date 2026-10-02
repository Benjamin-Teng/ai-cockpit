# proposal：deidentify

## Why

repo 準備第一次推上 remote，但目前檔案、截圖與 git 歷史裡帶有使用者的個人資訊：Windows 與 WSL 的使用者名稱（早期研究紀錄、
測試與範例的預設 socket 路徑）、使用者其他私人 repo 的名稱（真機驗收紀錄與三張真機截圖），以及歷史中兩張帶出使用者名稱的舊截圖；
全部 commit 的作者欄位是本名與私人信箱。推上去之後任何人都能從檔案或歷史翻出來。2026-10-02 盤點結果（只計數）見本 change 的
design「Context」。使用者 2026-10-02 決定：私人 repo 名稱一併換成代號、推 remote 前改寫歷史（先完整備份）、作者改為 GitHub 帳號與
noreply 信箱、刪除 Codex 外掛的兩個內部參照。路線上這是推 remote 前的最後一步（`docs/handover.md` 第 5 節）。

## What Changes

- **目前檔案**：使用者名稱（Windows、WSL）換成 `<user>`；三個私人 repo 名稱換成固定代號 `repo-a`、`repo-b`、`repo-c`。
- **測試與範例的 WSL socket 預設值**：不再寫死含使用者名稱的路徑，改為執行時向 WSL 取得 `$HOME` 推得；純解析用的測試字串改用中性路徑。
- **三張真機截圖**（change 7 `ui-fixes-live-*.png`）：以面板底色蓋掉露出私人 repo 名稱的區塊。
- **防再犯檢查腳本**：推送前執行，以執行時取得的使用者名稱、主機名稱、git 作者 email，以及不進 git 的本機詞表，檢查所有追蹤中的檔案
  （含 PNG 等二進位檔的位元組）。
- **改寫 git 歷史**（不可逆，執行前再向使用者確認）：對全部歷史套用同樣的文字替換、把含個人資訊的舊截圖換成乾淨版本、作者與 committer
  改為 `Benjamin-Teng <68319994+Benjamin-Teng@users.noreply.github.com>`、刪除 `refs/codex/turn-diffs/checkpoints/*`；改寫前以
  `git clone --mirror` 完整備份到 repo 外；改寫後保存「舊 commit 編號 → 新編號」對照表，讓文件中引用的舊編號仍可查。
- 本 repo 的 git 作者設定改為上述新身分。

## 非目標

- **產品行為**：不改任何功能、端點或畫面；規格不變（`skip_specs`）。
- **重拍真機截圖**：真機當時的狀態無法重現，只遮罩不重拍。
- **建立 remote 與推送**：本 change 只讓 repo 處於可推送狀態；建立 GitHub repo、推送與公開與否由使用者另行操作或指示。
- **設計文件的內容修訂**：`docs/superpowers/specs/2026-09-13-cockpit-mvp-design.md` 只做字串替換（§2 的 WSL socket 範例路徑），不改設計。

## Capabilities

### New Capabilities

（無）

### Modified Capabilities

（無；本 change 不改變任何規格層級的行為，`.openspec.yaml` 設 `skip_specs: true`）

## Impact

- 文件：`docs/research/2026-09-13/`（四個檔）、`docs/research/2026-09-16/pipeline-projection-acceptance.md`、`docs/research/2026-10-01/agent-report-live.md`、`ui-fixes-live.md`、
  設計文件、`herdr-client/README.md`、archive `2026-09-15-attach-herdr-runtimes/tasks.md`。
- 程式（只動測試與範例）：`cockpit/tests/config.rs`、`herdr-client/tests/real_herdr.rs`、`herdr-client/examples/spike4_no_window.rs`。
- 截圖：`docs/research/2026-10-01/ui-fixes-live-{connected,disconnected,reconnected}.png`。
- 新增：防再犯檢查腳本（`docs/research/2026-10-02/deid-check.js` 與用法說明）；`.gitignore` 加本機詞表。
- git：全部 commit 編號改變；作者欄位改變；`refs/codex/*` 刪除。新工具 `git-filter-repo` 2.47.0（已以 `uv tool` 安裝，不進專案依賴）。
