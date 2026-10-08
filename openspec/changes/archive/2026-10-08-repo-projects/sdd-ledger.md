# sdd-ledger：repo-projects

## 1. 基線

task 1.1 記錄，給後續 task 當回歸基準。

- 日期：2026-10-08
- HEAD：`006c1bc`（`feat/repo-projects`，工作樹乾淨）
- 工具鏈：`markdownlint-cli2 v0.23.2`；跑前 `cargo build -p cockpit --example ui_preview` 與 `cargo build -p cockpit --examples` 皆 exit 0
- `cargo test --workspace`：exit 0；68 個測試套件合計 **1304 passed／0 failed／13 ignored**
- `cargo test -p cockpit --example ui_preview`：exit 0；**62 passed／0 failed／0 ignored**
- 環境：跑腳本前 `netstat -ano` 確認 7770～7999、19000 一帶與 9222 沒有監聽；唯一相關的監聽是 `127.0.0.1:7778`
  （ASUS `ArmouryCrateControlInterface.exe`，與本專案無關，未動）。
- 偶發項目：本輪 15 支腳本與 cargo test 全部一次通過，沒有任何偶發失敗，沒有重跑。

### 既有腳本清單

出處：`docs/handover.md` 第 1 節「可用指令」，15 支（file-split-view 收尾全綠的集合）。`idle-exit-check.js` 測啟動器，不列入。
一律前景、一次一支、repo 根目錄 `node <路徑>`；`reconnect-check.js` 需先 `cargo build -p cockpit --examples`。

| 腳本 | 結果 | 備註 |
| --- | --- | --- |
| `docs/research/2026-09-15/reconnect-check.js` | PASS（exit 0） | 14 ok |
| `docs/research/2026-09-15/whatever-check.js` | PASS（exit 0） | 9 ok |
| `docs/research/2026-09-16/actions-check.js` | PASS（exit 0） | 86 ok |
| `docs/research/2026-09-16/channel-backoff-check.js` | PASS（exit 0） | 19 ok |
| `docs/research/2026-09-16/factory-floor-check.js` | PASS（exit 0） | 131 ok；會覆寫 `task-5.2-scenario-d.png`，已 `git checkout` 還原 |
| `docs/research/2026-09-19/live-output-check.js` | PASS（exit 0） | 345 ok |
| `docs/research/2026-09-23/visual-check.js` | PASS（exit 0） | 1769 ok；既有段落／FT1–FT3／收尾 FAIL 皆 0 |
| `docs/research/2026-09-27/files-check.js` | PASS（exit 0） | 578 ok；自我測試 2/2、scenario 0/27 FAIL、收尾衛生 PASS |
| `docs/research/2026-09-28/git-check.js` | PASS（exit 0） | 770 ok；收尾衛生 PASS |
| `docs/research/2026-10-01/progress-check.js` | PASS（exit 0） | 61 ok |
| `docs/research/2026-10-01/ui-fixes-check.js` | PASS（exit 0） | 349 ok |
| `docs/research/2026-10-02/output-color-check.js` | PASS（exit 0） | 89 PASS |
| `docs/research/2026-10-02/notify-check.js` | PASS（exit 0） | 新行為 191／前置 62 斷言，FAIL 0 |
| `docs/research/2026-10-03/i18n-check.js` | PASS（exit 0） | 新行為 948／前置 512 斷言，FAIL 0；重拍 `i18n-en-*.png` 15 張，已 `git checkout -- docs/research/2026-10-03` 還原 |
| `docs/research/2026-10-04/split-check.js` | PASS（exit 0） | 段落 47/47 PASS、收尾衛生 PASS；約 7 分鐘 |

腳本跑完後 `git status` 乾淨（除本檔外沒有修改），截圖皆已還原。

## 2. task 2.1：git rev-parse 查證

task 2.1 記錄（鐵則 3：精確字串先查一手來源）。

- 日期：2026-10-08
- 來源：<https://git-scm.com/docs/git-rev-parse>
- 本機 git：Windows git 2.50.1

官方文件逐字引文：

- `--path-format`：「Controls the behavior of certain other options. If specified as absolute, the paths printed by those
  options will be absolute and canonical. If specified as relative, the paths will be relative to the current working
  directory if that is possible. The default is option specific.」另：「This option may be specified multiple times and
  affects only the arguments that follow it on the command line, either to the end of the command line or the next
  instance of this option.」→ 必須放在三個路徑旗標之前。
- `--git-dir`：「Show `$GIT_DIR` if defined. Otherwise show the path to the .git directory. The path shown, when relative,
  is relative to the current working directory.」另：「If `$GIT_DIR` is not defined and the current directory is not
  detected to lie in a Git repository or work tree print a message to stderr and exit with nonzero status.」
- `--git-common-dir`：「Show `$GIT_COMMON_DIR` if defined, else `$GIT_DIR`.」
- `--show-toplevel`：「Show the (by default, absolute) path of the top-level directory of the working tree. If there is no
  working tree, report an error.」

文件未明文的部分與實測：

- **多旗標的輸出順序**：文件未明文。以 Windows git 2.50.1 實測確認依引數順序輸出三行（共同目錄、自己的 git 目錄、
  工作樹根目錄），主 worktree、linked worktree、submodule 皆驗證（`cockpit-git/tests/real_git.rs` 的
  `repo_identity_*` 整合測試）。
- **裸 repo 與 `.git` 目錄內**：stdout 先印出前兩行，再因 `--show-toplevel` 無工作樹印 `fatal: this operation must be
  run in a work tree`，exit 128。因此非零結束時不可解析 stdout。
- **一般目錄、不存在的目錄**：同為 exit 128（`not a git repository`、`cannot change to`）。
- **只有 exit 128 視為「不是 repo」**：git 的 `fatal:` 一律 128。其他非零結束碼與執行期 I/O 錯誤（`exit_code` 為 `None`）
  視為暫時性錯誤。WSL 找不到 git 時 `wsl.exe --exec env ... git` 由 `env` 回 127（2026-10-08 實測 `env: ... No such file
  or directory`，exit 127），若把所有非零都當「不是 repo」會把這種環境問題永久誤歸類。
- 未實測：WSL 端 git 2.43 的 `rev-parse` 輸出（task 4.3 處理 WSL 路徑轉換時再驗）。

## 3. task 4.1：會被改寫的既有測試

task 4.1 記錄（brief：改寫既有測試前先列進 ledger）。每一項都是 spec 已改變的斷言，改寫時保留原意。

| 測試 | 原斷言 | 改寫後 | 對應 spec |
| --- | --- | --- | --- |
| `cockpit/tests/app.rs` `no_projects_creates_no_state_file` | 沒有 project 時不建寫入服務、不建檔 | 寫入服務照建；沒有被接受的操作就不建檔（原意） | `pipeline-config`「狀態檔位置」「沒有 project 就不碰狀態檔」 |
| `cockpit/tests/config.rs` `no_project_means_no_state_path` | 設定檔沒有 project 時 `state_path` 為 `None` | 仍解出設定檔目錄下的 `cockpit.state.json`（啟動時要讀，可能含 Repo Project） | `pipeline-config`「狀態檔位置」「沒有 project 仍讀取既有狀態檔」 |
| `cockpit/tests/config.rs` `zero_config_has_no_state_path` | 零設定模式 `state_path` 為 `None` | 沒有 `LOCALAPPDATA` 時仍為 `None`；另有新測試驗有 `LOCALAPPDATA` 時的位置 | `pipeline-config`「零設定模式的位置」「沒有 LOCALAPPDATA」 |
| `cockpit/tests/progress_file.rs` `unsupported_version_fails_with_path` | `version: 3` 不支援 | 改用 `version: 4`（3 已是合法版本） | `pipeline-progress`「不支援的版本」 |
| `cockpit/tests/progress_file.rs` `written_state_is_v2_and_round_trips_with_active` | 寫出的 `version` 為 2 | 寫出的 `version` 為 3 | `pipeline-progress`「狀態檔格式與持久化」 |
| `cockpit/tests/progress_file.rs` `v1_file_is_rewritten_as_v2_with_empty_active` | v1 經操作後寫成 2 | 寫成 3 | `pipeline-progress`「讀取 v1 舊檔」 |
| `cockpit/tests/app.rs` `restart_keeps_progress_and_override` | 狀態檔 `version` 為 2 | 為 3 | `pipeline-progress`「狀態檔格式與持久化」 |
| `cockpit/tests/agent_api.rs` `start_declares_active_task` | 狀態檔 `version` 為 2 | 為 3 | `pipeline-progress`「狀態檔格式與持久化」 |

## 4. 審查紀錄（task 2.1～4.6，後端）

審查者一律為 Opus 5.5 subagent（專案 memory `opus-review-counts-as-codex-review`）。每個 task 一次 task 審查，修正後做範圍限定的
複審；4.6 為後端跨 task 審查。

| task | 結論 | findings 與處理 |
|---|---|---|
| 2.1 | 修 1 輪後通過 | Important：查證結果未寫入 ledger（派工措辭造成）→ 補第 2 節。Minor：只有 exit 128 算「不是 repo」→ 採納，改 design D1 |
| 3.1 | 修 1 輪後通過 | Important：`ProjectDef` 沒有種類標記，撞名時會判錯 → 加 `repo: Option<RepoKey>`，`refresh_projects` 自行過濾。Minor：固定 pane 的重驗依據誤用 `Auto` → 新增 `BindingBasis::Pinned` |
| 3.2 | 通過 | Minor：讀寫兩端的種類判斷要收斂成單一 helper → 帶進 4.1（`progress_for`／`progress_for_mut`） |
| 4.1 | 通過 | Minor：零設定建資料夾那行組裝無測試 → 4.2 補端對端測試 |
| 4.2 | 修 1 輪後通過 | **Critical**：改 stages 時沒有進度紀錄的 task 被靜默移到新的第一個 stage → 先依 `pane_repos` 以舊首 stage 補紀錄再重新對應；記入 memory `implicit-default-from-current-definition-drifts-on-edit`。Minor：`stage_for` 索引、移除時 overrides／active 殘留 → 修 |
| 4.3 | 通過，3 個 Minor 主動修 | M3：一輪中途 WSL 斷線仍可能執行 `wsl.exe` → 每次查詢前重新確認連線。M2：寫檔持續失敗時 log 洗版 → `FailureLog`。M5：補「從未連上」測試 |
| 4.4 | 修 1 輪後通過 | Important：免帶 id 推進以落後的投影選候選，剛 `start` 後立刻推進會推錯 task → 候選移入寫入鎖依 Domain 計算；補進 memory `check-against-lagging-projection-misses-fresh-writes` |
| 4.5 | 通過 | Minor：假 POST 驗證比正式端點寬 → 帶進 5.1 |
| 4.6 | 修 2 波後通過 | Important：runtime 一連上就清除，首份 snapshot 不完整時會不可逆刪進度 → `Connected.settled`，沉降重拿完成後才清；spec／design 同步。Minor：移除時 warnings 殘留、PATCH 先查 pid、拒絕零寬／雙向格式字元（放行 ZWJ／ZWNJ、擋 U+061C）、id 長度上限 → 全修 |

延後的 Minor 與所有裁決見 `docs/handover.md`（收尾時整理）。

## 5. 前端審查與設計審核（task 5.1～6.2）

| task | 結論 | findings 與處理 |
|---|---|---|
| 5.1 | 修 1 輪後通過 | Important：連點「加入」時成功與 409 錯誤並存 → 進行中標記（`aria-disabled`＋`aria-busy`，跨重畫保留）。Minor：投影到達前手動選定優先（spec 同步）、注入投影形狀、補斷言、`http.rs` 只公開一個解析函式 |
| 5.2 | 修 2 輪後通過 | Minor（主動修）：移除後焦點落點、未知對話框種類不再預設送 DELETE、進行中取消停用、送出前比對最新 stages、沒有 Project 時偵測區標題的焦點跨重畫保留 |
| 5.3 | 通過 | `source: pane` 的比較逐處確認歸邊正確 |
| 6.1 | 無必修；建議 F1～F6 全實作，補修 1 輪 | F1 加入中文字、F2 標題外框、F3 改名標題含名稱、F4 出錯列 `aria-invalid`、F5「⋯」展開狀態的 hover、F6 pane 數字型；補修 label-in-name 等。F7 worktree 標註改為「名稱下方」（spec 同步）；F8 不加「worktree」字樣；F9 主要按鈕加亮會改設計文件，交使用者 |
| 6.2 | 獨立核對後修 1 輪 | curl 範例 id 大小寫錯誤、WSL pane 在免帶 id 端點回 404、PowerShell 範例、用詞統一 |

設計審核截圖（114 張，含使用者名稱，**不進 repo**）：`%TEMP%\repo-projects-design\shots\`；報告 `%TEMP%\repo-projects-design\design-review.md`。

## 6. 收尾驗證（task 7.1）

task 7.1 記錄：全 gate 與全部驗收腳本，只驗證與記錄，不改產品程式。

- 日期：2026-10-08
- HEAD：`aae7d86`（`feat/repo-projects`，跑前工作樹乾淨）
- 環境：每支腳本跑前 `netstat -ano` 確認 127.0.0.1 的 7700～7999、9222、9333、18xxx、19xxx 沒有監聽；唯一相關的監聽仍是
  `127.0.0.1:7778`（ASUS `ArmouryCrateControlInterface.exe`，與本專案無關，未動）。一次一支、前景跑，沒有任何程序被砍。
- 偶發項目：全部 16 支腳本與所有 gate 一次通過，**沒有任何偶發失敗，沒有重跑**。

### 6.1 Gate

每個指令單獨跑，不接管線，逐一看結束碼。

| 指令 | 結束碼 | 結果 |
| --- | --- | --- |
| `cargo fmt --check` | 0 | 無差異 |
| `cargo clippy --all-targets -- -D warnings` | 0 | 0 warning |
| `cargo test --workspace` | 0 | 75 個測試套件合計 **1510 passed／0 failed／13 ignored** |
| `cargo build -p cockpit --example ui_preview` | 0 | 成功 |
| `cargo test -p cockpit --example ui_preview` | 0 | **81 passed／0 failed／0 ignored** |
| `cargo build -p cockpit --examples`（`reconnect-check.js` 前置） | 0 | 成功 |
| `markdownlint-cli2 "**/*.md"` | 0 | `Linting: 219 files`、`Summary: 0 issues in 0 files` |
| `openspec validate --all` | 0 | 25 passed／0 failed（25 items）；只有各 spec 既有的「Requirement should contain SHALL or MUST」WARNING（中文 requirement 標題，非錯誤） |

與基線（第 1 節，task 1.1）相比：workspace 1304 → 1510 passed，ui_preview 62 → 81 passed，markdownlint 204 → 219 files，
`openspec validate` 24 → 25 items（新增 `repo-projects` 的 delta），failed／ignored 沒有變。

### 6.2 驗收腳本

一律 repo 根目錄 `node <路徑>`；`reconnect-check.js` 帶 `--examples`。下表「斷言」欄的數字為腳本輸出中 `^ok`／`^PASS` 行數，
計法與第 1 節基線的記法不一定相同，只供對照量級。

| 腳本 | 結果 | 重跑紀錄 |
| --- | --- | --- |
| `docs/research/2026-09-15/reconnect-check.js` | PASS（exit 0）；14 ok；約 14 秒 | 無 |
| `docs/research/2026-09-15/whatever-check.js` | PASS（exit 0）；9 ok | 無 |
| `docs/research/2026-09-16/actions-check.js` | PASS（exit 0）；92 ok | 無 |
| `docs/research/2026-09-16/channel-backoff-check.js` | PASS（exit 0）；19 ok | 無 |
| `docs/research/2026-09-16/factory-floor-check.js` | PASS（exit 0）；131 ok | 無；覆寫的 `task-5.2-scenario-d.png` 已 `git checkout` 還原 |
| `docs/research/2026-09-19/live-output-check.js` | PASS（exit 0）；345 ok | 無 |
| `docs/research/2026-09-23/visual-check.js` | PASS（exit 0）；1784 ok；既有段落／FT1–FT3／收尾 FAIL 皆 0 | 無；比基線多 15 條，來自 task 5.1／5.2／5.3 對本腳本新增的斷言（空狀態、CL1 取樣 Repo Project 選單與管理對話框） |
| `docs/research/2026-09-27/files-check.js` | PASS（exit 0）；578 ok；自我測試 2/2、scenario 0/27 FAIL、收尾衛生 PASS | 無 |
| `docs/research/2026-09-28/git-check.js` | PASS（exit 0）；770 ok；收尾衛生 PASS | 無 |
| `docs/research/2026-10-01/progress-check.js` | PASS（exit 0）；64 ok | 無 |
| `docs/research/2026-10-01/ui-fixes-check.js` | PASS（exit 0）；349 ok | 無 |
| `docs/research/2026-10-02/output-color-check.js` | PASS（exit 0）；89 PASS | 無 |
| `docs/research/2026-10-02/notify-check.js` | PASS（exit 0）；新行為 191／前置 62 斷言，FAIL 0 | 無 |
| `docs/research/2026-10-03/i18n-check.js` | PASS（exit 0）；新行為 948／前置 512 斷言，FAIL 0 | 無；重拍 15 張 `i18n-en-*.png`，**刻意未還原**（見 6.4） |
| `docs/research/2026-10-04/split-check.js` | PASS（exit 0）；段落 47/47 PASS、收尾衛生 PASS；約 7.6 分鐘 | 無 |
| `docs/research/2026-10-08/repo-projects-check.js` | PASS（exit 0）；段落 28/28 PASS、收尾衛生 PASS；約 3.5 分鐘 | 無 |

跑完 `netstat` 沒有殘留的監聽（只剩無關的 7778）。

### 6.3 去識別化

- `node docs/research/2026-10-02/deid-check.js`：exit 0；模式「檔案」（commit `aae7d86db931`）、掃描 2043 個檔案、類別 U W H E T1 T2 T3 T4，**命中：無**。
- `node docs/research/2026-10-02/deid-check.js --history`：exit 0；模式「歷史（全部物件）」、8 個 ref、掃描 5503 個物件，**命中：無**。

兩項都是掃已提交的內容；工作樹中未提交的新 i18n 截圖不在掃描範圍，需逐張看圖（見 6.4）。

### 6.4 i18n 截圖變動

`i18n-check.js` 重拍的 15 張全部與已提交版本不同（`git status --short docs/research/2026-10-03` 皆為 modified 狀態），**保留新截圖、未 commit、未還原**，
交控制端逐張看圖做去識別化後決定是否提交（本 change 改了左欄外觀：偵測到的 repo 區、Repo Project）：

- `i18n-en-changes-700.png`、`i18n-en-changes-1100.png`、`i18n-en-changes-1536.png`
- `i18n-en-default-700.png`、`i18n-en-default-1100.png`、`i18n-en-default-1536.png`
- `i18n-en-file-700.png`、`i18n-en-file-1100.png`、`i18n-en-file-1536.png`
- `i18n-en-graph-700.png`、`i18n-en-graph-1100.png`、`i18n-en-graph-1536.png`
- `i18n-en-notify-700.png`、`i18n-en-notify-1100.png`、`i18n-en-notify-1536.png`

除這 15 張外，工作樹沒有其他未提交的變更（`factory-floor-check.js` 的截圖已還原）。

## 7. 最終修正（task 7.2／7.3）

整支分支最終審查（`.superpowers/sdd/tasks-repo-projects/final-review.md`）與真機冒煙
（`docs/research/2026-10-08/repo-projects-live.md`「發現」）的修正波。

### 7.1 WSL 端 `RepoIdentity` 實測（最終審查 I2）

補第 2 節「未實測：WSL 端 git 2.43 的 `rev-parse` 輸出」。

- 日期：2026-10-08；distro `Ubuntu-24.04`（跑前 `Stopped`，測試讓它開機；控制端授權，未關閉）
- WSL git：`git version 2.43.0`（`wsl.exe -d Ubuntu-24.04 --exec git --version`）
- 測試：`cockpit-git/tests/real_git.rs` 的 4 個 `#[ignore]` 測試 `wsl_repo_identity_*`，暫存目錄由 WSL 的 `mktemp -d` 建在
  `/tmp` 下，每個 fixture 先驗 `--show-toplevel` 是暫存目錄本身，測完 `rm -rf`（事後以 `ls -d` 確認三個目錄都已不存在）
- 指令：

```bash
COCKPIT_GIT_TEST_WSL_DISTRO=Ubuntu-24.04 cargo test -p cockpit-git --test real_git -- --ignored wsl_repo_identity --nocapture --test-threads=1
```

- 結果：exit 0，`4 passed; 0 failed; 0 ignored; 50 filtered out`。實際輸出（暫存目錄名為 `mktemp` 隨機產生）：

| 情境 | `common_dir` | `git_dir` | `toplevel` |
| --- | --- | --- | --- |
| 主 worktree 子目錄 `app/deep/er` | `/tmp/tmp.Vk2Ygzg3ux/app/.git` | `/tmp/tmp.Vk2Ygzg3ux/app/.git` | `/tmp/tmp.Vk2Ygzg3ux/app` |
| 主 worktree 根目錄 `app` | `/tmp/tmp.lXweuPa9At/app/.git` | `/tmp/tmp.lXweuPa9At/app/.git` | `/tmp/tmp.lXweuPa9At/app` |
| linked worktree `app-feat` | `/tmp/tmp.lXweuPa9At/app/.git` | `/tmp/tmp.lXweuPa9At/app/.git/worktrees/app-feat` | `/tmp/tmp.lXweuPa9At/app-feat` |
| submodule `super/lib` | `/tmp/tmp.1lcqNPQUII/super/.git/modules/lib` | `/tmp/tmp.1lcqNPQUII/super/.git/modules/lib` | `/tmp/tmp.1lcqNPQUII/super/lib` |
| 一般目錄 | — | — | — |

一般目錄：`git rev-parse --git-dir` 前提檢查 exit 128；`GitRunner` 的 `calls[0]` 為 `Failed { exit_code: Some(128) }`，
`RepoIdentity::parse` 回 `Ok(None)`。

結論：與 Windows git 2.50.1 相同——三行依引數順序、絕對路徑、沒有結尾斜線；linked worktree 的 git 目錄在
`<共同>/worktrees/<名稱>`；submodule 的兩者相同。`repo_resolver.rs` 的 `wsl_*` 單元測試以手寫字串假設的正是這個格式，
**不需要改產品程式**。

### 7.2 其餘修正

| 項目 | 處理 | 測試（RED → GREEN） |
| --- | --- | --- |
| I1 Windows pane 停在 WSL UNC 路徑會把發行版開機 | resolver 查詢目標是 WSL 時，先以 `RunningDistros`（正式實作跑不會開機的 `wsl.exe --list --running --quiet`，與 `cockpit-herdr` 共用 `probe::list_running`／`decode_list`）確認發行版在跑；每一輪最多探測一次、失敗視為沒在跑、名稱不分大小寫；沒在跑就不查、沿用上一輪歸類 | `cockpit/tests/repo_resolver.rs` 兩支 `windows_pane_on_wsl_path_*`：修前 2 次查詢／快取過期後重查而失敗，修後通過 |
| M1 agent 端點對 Repo Project pane 缺 HTTP 層測試 | `cockpit/tests/repo_project_api.rs` 補三支 `agent_*`，路徑用 `local~wJ%3Ap1` | 既有行為的特性測試，一次通過 |
| M2、M3 | 過時註解改正；測試模組 `use` 移到 fn 之前 | — |
| BOM（真機冒煙發現 1） | POST 與 PATCH 解析前剝開頭一個 UTF-8 BOM（`strip_utf8_bom`），`ui_preview` 共用 `parse_add_repo_project_body` | 修前 400、修後 201／204；兩個 BOM 仍 400；只還原 PATCH 的剝除時測試失敗（突變檢查） |
| `invalid_body` 文字（發現 3） | 繁中原文與 `i18n.js` 中英範本改為兩種方法都適用的說法；`i18n-check.js` MSG_CASES 跟上 | `i18n-check.js` 全綠 |
| README（發現 1、2） | PowerShell 區塊補 PATCH、說明 BOM 已被容忍、引號問題限 5.1 與 7.2 以前 | Windows PowerShell 5.1（`chcp 65001`）對 `ui_preview` 實跑 here-string POST：`curl --trace` 見本體以 `ef bb bf` 開頭，回 201 |

### 7.3 Gate 與腳本

每個指令單獨跑、不接管線，逐一看結束碼。

| 指令 | 結束碼 | 結果 |
| --- | --- | --- |
| `cargo fmt --check` | 0 | 無差異 |
| `cargo clippy --all-targets -- -D warnings` | 0 | 0 warning |
| `cargo test --workspace` | 0 | 75 個測試套件合計 **1516 passed／0 failed／17 ignored**（比 6.1 多 6 支測試、4 支 `#[ignore]` 的 WSL 測試） |
| `cargo test -p cockpit --example ui_preview` | 0 | **82 passed／0 failed** |
| `markdownlint-cli2 "**/*.md"` | 0 | `Linting: 220 files`、`Summary: 0 issues in 0 files` |
| `openspec validate --all` | 0 | 25 passed／0 failed |
| `node docs/research/2026-10-08/repo-projects-check.js` | 0 | 段落 28/28 PASS、收尾衛生 PASS（約 3.6 分鐘） |
| `node docs/research/2026-10-03/i18n-check.js` | 0 | 新行為 948／前置 512 斷言，FAIL 0；重拍的 5 張 `i18n-en-*.png` 已 `git checkout` 還原 |

跑前、跑後 `netstat` 只有無關的 7680、7679、7778 在監聽；腳本前先 `cargo build -p cockpit --example ui_preview`。

### 7.x 最終修正第二波（`d4590fd`）

最終修正波複審發現修正本身新增的 Important：resolver 的 WSL 探測（`wsl.exe --list --running`）沒有逾時，`wsl.exe` 卡住會讓整個
resolver 永久停住。修正：resolver 側 5 秒逾時、`kill_on_drop`，逾時視為沒在跑；探測結果跨輪沿用 60 秒。複審通過。

已知殘留（列入 `docs/handover.md`）：「在跑」也快取 60 秒，使用者 `wsl --shutdown` 後 60 秒內，若某個 Windows pane 停在
`\wsl.localhost\` 路徑且快取剛好未命中，仍可能把發行版開機一次。建議之後只快取「沒在跑」，或在 WSL runtime 斷線時清快取。

## 8. 實作期間的裁決（Claude 依使用者 2026-10-08 授權）

格式：裁決 — 理由 — 若錯的代價。

- 3.1 的 workstream 名稱與排序交給 3.2（投影）— design D3 已改為投影時取名 — 若錯只是 3.1/3.2 邊界移動。
- 每 task 用 opus task reviewer，另保留 4.6、7.2 的跨 task 審查 — memory 規定 Opus 視同 Codex、CLAUDE.md 不許 sonnet 當 diff reviewer — 成本較高。
- 只有 exit 128 視為「不是 repo」，cwd 不存在也歸此類，其他非 0 為暫時錯誤 — 實作比 design 原文更正確（WSL 找不到 git 回 127）— 已改 design D1；若錯，最多 60 秒內重查一次
- 3.1 只跑 3 支既有腳本即可 — 投影只對固定 pane 的 unbound 多 source，既有 fixture 無固定 pane — 若錯，5.x／7.1 全量回歸會抓到
- Repo Project 標完成後 agent 仍 working 時顯示「未宣告」照 spec 字面保留 — 與手寫 project 一致、design Risks 已記；使用者清除標記即恢復 — 若錯，只是提示文字不精確
- 預設名稱不合規則（>64 字元等）時截斷而非回 invalid_name — 預設名稱來自資料夾名，使用者無從修正請求 — 若錯，只是名稱被截短，可再改名
- PATCH 的 from 逐字比對不 trim — stage 名稱保存時已 trim，前端送的是現有名稱 — 若錯，帶空白的 from 回 invalid_stages
- 加入時先判 repo_already_added 再判 repo_not_detected — 已加入但 pane 都關了時「已加入」較有資訊 — 若錯，只是錯誤碼不同
- set_pane_repos 與清除檢查分兩筆交易（同一把鎖）— 合併時清除落檔失敗會撤回歸類更新 — 若錯，兩筆之間有極短的不一致窗
- 編輯 stages 當下不在 pane_repos、且從無紀錄的 pane，日後歸回時落在新首 stage — 與新出現的 pane 無法區分，補救需讓 pane 進出落檔、違反 D3 — 若錯，該卡片顯示在新首 stage
- WSL 路徑含 Windows 無法表示字元 → 不歸類、60 秒重查；預設名稱取不到上一層 → 用最後一段 — spec 未定，皆保守 — 若錯，只影響罕見路徑的顯示
- PATCH 的 name/stages 為 null 視同省略 — serde 預設行為、spec 無 null scenario — 若錯，null 不回 invalid_body；已寫入 design D6
- whatever-check（pane 列數 5→7）、actions-check（改綁候選多 p6/p7）兩條因 fixture 規模改斷言 — 有 file-review task 3.4 先例；替代方案會動到 notify-check 逐字斷言或讓既有腳本永不見 Repo Project — 若錯，兩條斷言的數字需回改
- 同一 pane 同時被手寫自動綁定與 Repo Project 綁到時 /api/agent/advance 回 ambiguous_task — 符合 D7 字面，屬罕見設定 — 若錯，該 agent 需改用帶 id 端點；記入 handover
- 名稱放行 U+200C/U+200D、改擋 U+061C — emoji 與波斯／印度系文字需要，且不改變顯示方向 — 若錯，名稱中可能夾帶不可見的連接字元
- 併入的進行中重拿視同沉降完成 — 沿用 driver 既有沉降語意，比原本首份即清嚴格 — 若錯，HERDR 恢復極慢時仍可能誤清；7.3 真機觀察
- 加入後、投影到達前使用者手動選別的 Project 則不自動切換 — 使用者明確選擇優先 — 若錯，使用者需自己點新 Project；已改 spec
- 5.2 fix round 只重跑 4 支既有腳本 — 改動限焦點落點與對話框；7.1 收尾全量回歸 — 若錯，7.1 會抓到
- F7 worktree 標註維持在名稱下方，spec 改為「名稱下方」— 截圖顯示下方較易讀且不擠名稱 — 若錯，只是位置調整
- F8 不加可見的「worktree」字樣，維持 title 提示 — KISS、避免新視覺語彙 — 若錯，使用者可能看不出那是 worktree 名稱；列入最終報告供使用者改
- F9 主要動作按鈕加亮一級不做 — 會在設計文件新增按鈕層級，依 memory 交使用者決定
- I1 修法採「查 WSL 目標前以 wsl.exe --list --running 探測，發行版沒在跑就不查、沿用上次歸類」— 不需設定 WSL runtime 也正確 — 若錯，探測失敗時 WSL 路徑 pane 暫不歸類
- I2 允許為實測執行一次 WSL 測試（可能開機發行版）— 一次性、非背景、使用者可見影響小 — 若錯，只是多開一次 WSL
- 後端 JSON 本體容忍開頭 UTF-8 BOM — PowerShell 5.1 預設會加，照抄文件就失敗 — 若錯，只是多接受一種本體前綴
- 例外加一小波修正（探測 5 秒逾時＋kill_on_drop、探測結果跨輪快取 60 秒）— SDD 原則只一波，但已知卡死風險且修正極小；使用者授權全權決定 — 若錯，多一次審查成本
- 停止修正迴圈，上述 Minor 列入 handover 待辦（建議只快取「沒在跑」或 WSL runtime 斷線時清快取）— 需 shutdown 後 60 秒內恰好有 UNC pane 快取未命中，機率低 — 若錯，該情境會把發行版開機一次
