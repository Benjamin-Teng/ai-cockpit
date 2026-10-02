# tasks：deidentify

> 執行路徑：直接 `opsx:apply`（不上 SDD）｜理由：步驟線性、互相依賴（A 段清理 → 併回 → B 段改寫歷史 → 驗證），可獨立派工的 task 少；
> 產品行為不變。A 段的機械性替換與腳本可派 subagent，B 段由控制端親自執行並逐步驗證。
>
> **審查**：使用者 2026-10-02 指示 Codex 審查由 Opus 5.5 取代。A 段 diff 審一次（1.6）；B 段由全歷史實測把關，並請 Opus 獨立重跑一次（2.6）。
> **B 段不可逆**：2.1 是停止點，未得使用者明確同意不得執行 2.2 之後的任何步驟。

通則：

- 敏感字串一律在執行時取得（`os.userInfo().username`、`os.hostname()`、`wsl.exe -d Ubuntu-24.04 --exec whoami`、`git log --format=%ae`），
  私人 repo 名稱放 repo 根的本機詞表 `.deid-terms`（不進 git）；**任何 commit、報告、對話輸出都不得出現它們的實際值**，一律用代號
  （U、W、H、E、R1–R3，定義見 design「Context」）。
- 比對與替換在 Node 內讀檔進行，不把以 `/` 開頭的敏感字串當成 Git Bash 的命令列參數（MSYS 路徑轉換會改寫它）。
- 改 `.md` 跑 `markdownlint-cli2 "**/*.md"`（核對 `Linting: N files` 不為 0）；改 Rust 跑 `cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、
  `cargo test --workspace`；有改 `openspec/` 跑 `openspec validate --all`。
- 截圖改動後逐張用影像工具看圖。

## 1. A 段：清理目前檔案（feature 分支 `feat/deidentify`）

- [x] 1.1 文字替換（design D1）：`docs/research/2026-09-13/` 四檔、`docs/research/2026-09-16/pipeline-projection-acceptance.md`、
  `docs/research/2026-10-01/agent-report-live.md`、`ui-fixes-live.md`、`docs/superpowers/specs/2026-09-13-cockpit-mvp-design.md`、
  `herdr-client/README.md`、`openspec/changes/archive/2026-09-15-attach-herdr-runtimes/tasks.md`，以及盤點列出的其餘含 W 路徑段的文件。驗收：人工讀過每個改到的段落；
  markdownlint 0 issues；`openspec validate --all` 通過
- [x] 1.2 Rust 測試與範例（design D2）：`cockpit/tests/config.rs` 改中性路徑；`herdr-client/tests/real_herdr.rs` 的 `wsl_socket()` 與
  `herdr-client/examples/spike4_no_window.rs` 改為由 WSL `$HOME` 推得預設 socket。驗收：`cargo test -p cockpit --test config` 通過；
  `HERDR_CLIENT_TEST_WSL_SOCKET` 未設時，`spike4_no_window` 印出的預設路徑等於 WSL 的 `$HOME/.config/herdr/herdr.sock`（只比對是否相等、不印路徑）；
  全 Rust gate 通過
- [x] 1.3 三張真機截圖遮罩（design D3）。驗收：逐張看圖確認 R1–R3 不可見、其餘畫面未被誤蓋；`ui-fixes-live.md` 註明已遮罩
- [x] 1.4 防再犯檢查腳本（design D4，檔案模式＋`--rev`＋`--history`）：`docs/research/2026-10-02/deid-check.js`、`deid-check.md`；`.gitignore` 加
  `.deid-terms`；建立本機 `.deid-terms`（R1–R3 與改寫前的 E）。驗收：`--rev <1.1 之前的 commit>` 跑出命中且類別與檔數與盤點一致（含 archive
  tasks.md 的 `W@` 與 `/home/W` 接反斜線兩種形式）；檔案模式對現行 HEAD 0 命中、exit 0；`--history` 在改寫前會命中（作者欄位為 E、舊 blob），
  記錄命中類別供 2.5 對照；輸出不含任何詞的實際值
- [x] 1.5 全 gate：Rust gate、`cargo test -p cockpit --example ui_preview`、markdownlint、`openspec validate --all`、12 支既有驗收腳本（清單見 `docs/handover.md`
  第 1 節）、`deid-check.js`（檔案模式）0 命中
- [x] 1.6 A 段審查（Opus 5.5 取代 Codex）：範圍 `main..HEAD`；findings 實測後才改
- [x] 1.7 A 段 squash 併回 `main`（已授權），刪除 `feat/deidentify` 分支；`main` 上 `deid-check.js`（檔案模式）0 命中

## 2. B 段：改寫歷史（不可逆，design D5）

- [x] 2.1 **停止點**：向使用者報告 A 段結果、備份位置與 2.2–2.7 的確切步驟，取得明確同意後才繼續
- [x] 2.2 前提與備份（design D5 前提、第 1 步）：`git status --porcelain` 無追蹤中改動、`git worktree list` 只有本 repo；整個 repo 目錄（排除 `target/`）
  複製到 repo 外；`git clone --mirror --no-local` 到 repo 外並 `git fsck --full`、ref 數相同；記下原 `main` 與 `main^{tree}` 編號
- [x] 2.3 演練（D5 第 2 步）：`git clone --no-local` 的副本上執行刪 ref＋filter-repo，跑 2.5 的全部驗證；不過就修正替換表或 mailmap 重演練，不碰原 repo
- [x] 2.4 原 repo 執行（D5 第 3–4 步）：刪 `refs/codex/turn-diffs/checkpoints/*` → `git filter-repo --force`（替換表、mailmap、blob-callback；不用
  `--dry-run`／`--debug`）
- [x] 2.5 驗證（D5 第 5 步）：`deid-check.js --history` 0 命中；`git for-each-ref` 只剩 `refs/heads/*`；新 `main^{tree}` 等於改寫前；commit 數不變；
  作者／committer 只剩新身分；全 gate 與 12 支既有腳本全綠。任一項失敗即依 D5 第 7 步從整份目錄備份還原
- [x] 2.6 Opus 獨立驗證：不給替換表，由它自行跑 `deid-check.js --history`、讀物件比對、檢查作者欄位與 ref；通過後才刪 repo 外的替換表與 mailmap
- [x] 2.7 收尾（D5 第 6 步）：刪 `.git/lost-found/`；loose／unreachable 物件 0；repo 層級 `git config user.name`／`user.email` 改為新身分；`commit-map`
  存成 `docs/research/2026-10-02/commit-map.txt` 並 commit，再跑一次 `deid-check.js --history`

## 3. 收尾

- [x] 3.1 archive 本 change；重寫 `docs/handover.md`（含舊編號對照說明、備份位置與「備份保存或刪除由使用者決定」、推送前跑 `deid-check.js`）；markdownlint 0 issues
