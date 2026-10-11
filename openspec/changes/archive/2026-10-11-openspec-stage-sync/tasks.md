# tasks：openspec-stage-sync

> 執行路徑：SDD｜理由：
>
> - 改動橫跨 `cockpit-git`、`cockpit-core`、`cockpit` 後端與前端四塊，各自可以獨立驗收。
> - 涉及狀態檔升版（v4）、寫入服務單一鎖內的自動與手動優先權，以及新背景工作的並發與 WSL 防護（design D3～D5）。
> - 後面的 task 建在前面之上，錯了返工貴。
>
> **審查**：diff 審查優先以 Codex `adversarial-review`（全域 `CLAUDE.md` 路徑②）。Codex 無法使用時，依專案 memory
> `opus-review-counts-as-codex-review` 改派 Opus 5.5 subagent。分兩批：
>
> - 第 4 節（後端）完成後審一次（4.7）。
> - 收尾前審整支分支（7.2）。
>
> findings 與處理方式記在 `sdd-ledger.md`。Codex findings 先實測重現才採信。
>
> **授權**：使用者 2026-10-10 授權其餘設計與實作由 Claude 全權決定，拿不定的跟 Codex 商量，最後回報決定清單。
> **不併回 `main`、不發版**，等使用者確認。

**工作樹**：`D:\projects\ai-cockpit`，分支 `feat/openspec-stage-sync`。

## 通則（每個 task 都適用）

- **品質 gate**：
  - 每個 task 結尾跑 `cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test --workspace`。
  - 有改 `cockpit/examples/` 時，加跑 `cargo test -p cockpit --example ui_preview`。
  - 有改 `.md` 時，在 repo 根跑 `markdownlint-cli2 "**/*.md"`，並核對 `Linting: N files` 的 N 不為 0。
  - 有改 `openspec/` 時，跑 `openspec validate --all`。
  - gate 單獨跑、看結束碼，不接管線（專案 memory `gate-exit-code-swallowed-by-pipe`）。輸出貼進回報。
- **TDD**：實作者載入 `superpowers:test-driven-development`，新行為先寫會失敗的測試，回報附 red → green 證據。
  測試名稱盡量對應 spec scenario。
- **跑腳本前先重建**：改完 `cockpit/assets/` 一定先跑 `cargo build -p cockpit --example ui_preview`，再跑腳本
  （專案 memory `acceptance-scripts-run-stale-embedded-assets`）。
- **既有腳本**指 1.1 寫進 `sdd-ledger.md` 的清單。
  - 每個改前端或投影的 task 結束時，既有腳本必須全綠。
  - 只能改被本 task 打壞、且 spec 已改變的斷言，不得放寬 spec 沒改變的斷言。
  - 腳本修改與產品修改分開 commit，commit 訊息列出被改的斷言與對應 scenario。
  - 腳本不可並行，一律前景跑。
- **port**：跑腳本前用 `netstat -ano` 確認腳本用到的 port 沒人在用。
  - 使用者自己的 Cockpit 用 7770，另見專案 memory `acceptance-scripts-fixed-ports-collide-across-sessions`。
  - **不是自己開的程序只能回報、不准砍**（專案 memory `harness-pid-checks-misreport-on-windows`）。
- **暫存 git repo** 一律建在 repo 外（`%TEMP%` 下）。建立後先驗證 `git rev-parse --show-toplevel` 是暫存目錄本身
  （專案 memory `git-in-subdir-falls-through-to-enclosing-repo`）。commit 一律帶 `-c user.name=… -c user.email=…`。
- **HERDR 唯讀**：不新增任何 HERDR method（AGENTS.md）。
- **WSL 防護**：任何背景 git 或讀檔，碰到 WSL 路徑前都要先過 `RunningDistros` 檢查。
  相關專案 memory：`guard-checks-owner-but-side-effect-follows-data`、`wsl-exe-argument-passing-pitfalls`。
- **介面文字**一律放進 `cockpit/assets/app/i18n.js` 中英兩份字典。新的後端錯誤 code 與 `Message` 變體都要有 `msg.<code>`。
- **註解與勾選**：程式碼註解引用 task 寫成 `openspec-stage-sync task N.M`。`tasks.md` 由控制端統一勾。
- **提交**：一次只有一個 subagent 改工作樹。commit 只 `git add <具體路徑>`。不 push。

## 1. 基線

- [x] 1.1 記錄基線與既有腳本清單。
  - 腳本清單取 `openspec/changes/archive/2026-10-08-repo-projects/sdd-ledger.md` 第 1 節的 15 支，加
    `docs/research/2026-10-08/repo-projects-check.js`，共 16 支（以 `docs/handover.md` 第 1 節「可用指令」核對）。
    測啟動器的 `idle-exit-check.js` 不列入。
  - 跑 `cargo test --workspace`、`cargo test -p cockpit --example ui_preview` 與清單中全部腳本。
  - 把清單、passed／failed／ignored 數字與各腳本結果寫進 `openspec/changes/openspec-stage-sync/sdd-ledger.md`。
  - 驗收：ledger 有清單與數字，failed 為 0、腳本全綠。已知偶發項目重跑一次並記錄。

## 2. 目前分支查詢（`cockpit-git`）

- [x] 2.1 新增 sealed 查詢 `CurrentBranch`：`symbolic-ref -q HEAD`，自行去掉 `refs/heads/` 前綴（design D7；spec `git-review`）。
  - 動手前對 git 官方文件（git-scm.com `git-symbolic-ref`）確認 `-q` 的語意與 detached 時的結束碼（`-q` 在 detached 時安靜地以 exit 1 結束）。
    查證結果與日期寫進 ledger（鐵則 3）。
  - 照 `RepoIdentity`／`VerifyCommit` 的模式：query struct、parse、`lib.rs` 匯出、argv 斷言測試。
  - parse 測試涵蓋：正常分支名（`refs/heads/main` → `main`）、含 `/` 的分支名、CRLF 結尾、輸出不是 `refs/heads/` 開頭為 `None`、
    detached（exit 1 無輸出）為 `None`、其他錯誤。
  - `real_git.rs` 在 `%TEMP%` 建 repo 實測：一般分支、`feat/x` 分支、與分支同名的 tag（結果仍為不帶 `heads/` 的分支名）、detached HEAD。
  - WSL 端的 `real_git` 測試為 `#[ignore]`（會開機發行版，非必要不跑）；手動跑的結果記在 ledger。
  - 驗收：新測試先紅後綠；`cargo test -p cockpit-git` 通過。

## 3. Domain 與投影（`cockpit-core`）

- [x] 3.1 每站 OpenSpec 階段對應（design D1；spec `repo-projects` 的加入、修改 stages、輸入驗證）。
  - 新增 `OpenSpecPhase`。`RepoProjectDef.phases` 與 `stages` 對齊。
  - 驗證長度相同、非空值唯一。`apply_stage_edits` 帶每列 phase。
  - 驗收：單元測試涵蓋對齊、重複拒絕、改名、重排、刪站後對應正確；既有 repo 測試照常綠。
- [x] 3.2 同步狀態與自動移動轉移（design D2、D3；spec `openspec-stage-sync` 的自動移動與手動優先、同步狀態持久化）。
  - `Observation`、`TaskSync`、`repo_sync`（持久）、`openspec_obs`（不持久）。
  - `DomainState::apply_openspec` 依 D3 的四條規則實作。
  - 手動標記的 helper：建立或改為 `Manual`；沒有 `TaskSync` 時以目前偵測結果建立。
  - 修改對應時，清掉 `Auto` task 的 `applied`。
  - 建立或更新 `TaskSync` 時，task 沒有進度項目就補一筆初始進度（第一個 stage、`mark = none`），沒有進度項目時「目前 stage」視為第一個 stage；
    `drop_untouched_initial` 不得撤回有 `TaskSync` 的 task 的進度項目（design D2、D3）。
  - pane 消失時進度項目與同步狀態一起清除。
  - 自動移動寫入的 stage 必須在 `stages` 內，不可觸發 `apply_op` 的 panic。
  - 驗收：每條 D3 規則各有測試，含「已標記不動且不更新 applied → 清除標記後下一輪套用」「手動後同結果不動、結果變化恢復自動」
    「對應不存在只更新 applied」「修改對應後 Auto 重套、Manual 不動」，以及：
    - 找不到對應 stage 時仍建立進度項目，且 pane 消失時一起清除。
    - 目標等於第一個 stage 時不移動，但仍補進度項目與同步狀態，`drop_untouched_initial` 不會撤回。
    - 「階段對應是否改變」的判準（design D10-2）：改名且 phase 跟著走不清 applied、只重排不清、phase 換擁有者才清。
- [x] 3.3 投影（design D8；spec `state-projection`）。
  - `ProjectedProject.stage_phases`、`ProjectedTask.sync`。當下對不上 change 時為 `null`。
  - 有偵測結果但沒有 `TaskSync` 時，`mode` 以 `auto` 呈現。
  - 驗收：投影測試涵蓋上述 scenario，並含「偵測結果改變時投影 `version` 遞增、沒變時不遞增」；「version 只在內容改變時遞增」的既有測試照常綠。

## 4. 後端（`cockpit`）

- [x] 4.1 狀態檔 v4（design D4；spec `pipeline-progress` 的格式與載入）。
  - v4 讀寫 `phases` 與 task 的 `sync`。版本檢查接受 1 到 4，系統寫出一律 v4。
  - 讀 v1 到 v3 時依預設站名（繁中與英文）補對應，重複的後者為 `null`。v4 的 `null` 一律尊重。
  - 會被改寫的既有測試（斷言寫出 `version: 3`、v4 不支援等）先列進 ledger 再動，改寫時保留原意。
  - `check_version_shape` 改為 v3／v4 分流：`phases` 在 v3 缺席才合法、在 v4 必填。
  - 驗收：測試涵蓋 v3 升 v4 補對應、v4 往返不變、`sync` 往返、不合規則的 `phases` 啟動失敗、v5 不支援，以及：
    - v3 檔含 `phases` 或 `sync` → 啟動失敗。
    - 手寫 `projects` 底下的 task 帶 `sync` → 視為損毀、啟動失敗。
- [x] 4.2 OpenSpec 偵測純邏輯（design D6；spec `openspec-stage-sync` 的偵測規則）。
  - 新模組 `cockpit/src/openspec_sync.rs` 的純函式部分：給定 worktree 根目錄與分支（`Option<String>`），回傳 `Option<Observation>`。
  - 實作 change 對應的四步順序（archive 同 slug 有多個日期時取日期字串最大者）、checkbox 認法、`tasks.md` 1 MiB 上限，
    以及 `tasks.md`、`openspec/changes/`、`archive/` 的讀取錯誤視為「無法判斷」。
  - 以 `%TEMP%` 下的暫存目錄造 `openspec/changes/` 結構測試，不用真 git。
  - 驗收：測試涵蓋分支命中、archive 命中、單一進行中退路、多個進行中對不上、沒有 `openspec/`、detached、
    `tasks.md` 缺檔、0 勾、部分勾、全勾、`*`／`+`／大寫 X、超過上限（含剛好 1 MiB 可讀）、非 UTF-8、
    archive 同 slug 多個日期取最大者、`openspec/changes/` 或 `archive/` 讀取錯誤為無法判斷。
- [x] 4.3 寫入服務（design D3、D8；spec `openspec-stage-sync`、`repo-projects`）。
  - `ProgressService::sync_openspec`：一次 `transact` 內更新 `openspec_obs`，並對每個 task 套用轉移。
  - 人工 `advance`／`retreat` 與 agent 的兩個推進入口，成功後標記手動：`apply_progress` 與標 Manual 必須在同一個 `write` 閉包內，只落檔一次；
    `agent_advance_for_pane` 的 task 在鎖內選出後就地標記（design D3）。
  - `remove_repo_project` 一併清除該 project 的 `repo_sync`。
  - 加入時的 `phases`；修改 stages 的每列 `phase`，修改對應後重套 Auto task。
  - 驗收：整合測試涵蓋自動移動落檔、手動後重啟仍為手動、agent 推進算手動、complete／fail／clear 不改 mode、
    並發的同步與人工寫入不遺失，以及：
    - 推進與同步並發不遺失、不被拉回（只會是「先同步後推進」或「先推進後同步」兩種結果，不會出現推進失效的手動狀態）。
    - 被拒絕的推進不改 mode。
    - 手寫 project 推進不產生 `sync`。
    - 偵測結果沒造成改變時不改寫狀態檔。
    - 最新偵測結果（`openspec_obs`）不寫入狀態檔。
    - 移除 Repo Project 後同步狀態一併消失。
- [x] 4.4 背景偵測工作（design D5；spec `openspec-stage-sync` 的偵測時機與防護）。
  - `PaneRepo` 新增欄位 `root`（worktree 根目錄的主機路徑字串）；會改到 `cockpit-core` 的 `PaneRepo` 與所有 struct literal。
    - Windows：git `--show-toplevel` 輸出的正斜線轉反斜線，大小寫保留（不套 repo key 的小寫化）。
    - WSL：POSIX 路徑經既有的 `wsl_host_path` 轉為 `\\wsl.localhost\<distro>\...`；轉不出來則 `root` 為 `None`，該 pane 不偵測。
  - `OpenSpecSync`：10 秒輪詢；依 worktree 分組；未連線 runtime 或 WSL 發行版未在執行時跳過；
    經 `CurrentBranch` 與 4.2 的純邏輯取結果；內容與上一輪相同不送；以 sink trait 送進寫入服務。
  - 在 `app.rs` 組裝，納入 `shutdown_all`。
  - 查詢、讀檔、`RunningDistros` 以 trait 注入。測試用假實作與 tokio 暫停時間，涵蓋：
    - 正常送出
    - 不變不送
    - 未連線不查
    - 發行版未在執行時不查、不呼叫任何 git 或讀檔
    - 查詢錯誤送 `None`
    - 同一 worktree 多個 pane 共用一次查詢
    - 手寫 project 的 pane 不偵測；`root` 為 `None` 的 pane 不偵測
    - 剛啟動且被防護跳過的 worktree 沒有偵測結果（`sync` 為 `null`）
  - 唯讀驗證：測試以暫存 repo 跑一輪偵測，之後 repo 內檔案內容與 mtime 不變、沒有 `index.lock`。  - 驗收：上述測試通過；`app.rs` 接上正式實作。
- [x] 4.5 HTTP 端點本體（design D8；spec `repo-projects`）。
  - `POST /api/repo-projects` 的選填 `phases`；`PATCH` 每列選填 `phase`。兩者都維持 `deny_unknown_fields`。
  - 錯誤碼只用既有的 `invalid_stages`／`invalid_body`（design D10-1）：型別不對回 `invalid_body`；長度不符、階段字串不合法、重複回 `invalid_stages`。
    階段字串先以字串接收再驗證，未知字串不可落到反序列化錯誤的 `invalid_body`。不新增 code。
  - 驗收：`cockpit/tests/repo_project_api.rs` 涵蓋上列 scenario；對帳測試通過。
- [x] 4.6 `ui_preview` fixture。
  - `demo-app` 的 stages 帶 `stage_phases`。
  - 兩張 task 各帶一種 `sync`：一張 `auto` 實作中 3/8、一張 `manual` 審查。
  - 假路由記錄 `phases`／`phase` 欄位。fixture 字串不得含使用者名稱。
  - 驗收：`cargo test -p cockpit --example ui_preview` 通過；既有腳本全綠。
- [x] 4.7 後端 diff 審查（Codex，退路為 Opus）。
  - 範圍：`main..feat/openspec-stage-sync`。
  - focus：狀態檔升版與補對應、寫入鎖內的優先權轉移、背景工作的 WSL 防護與並發、端點驗證。
  - findings 依 `superpowers:receiving-code-review` 處理，先實測重現才採信。
  - 驗收：ledger 記錄結論段與每個 finding 的處理。

## 5. 前端（`cockpit/assets/app/`）

- [x] 5.1 卡片同步標示（design D9；spec `cockpit-dashboard` 的 Factory Floor）。
  - 有 `sync` 時顯示 change 名稱、`checked/total` 與「自動」或「手動」，手動用較淡樣式。名稱一律以 `textContent` 呈現。
  - i18n 鍵中英同步。
  - 新增驗收腳本 `docs/research/2026-10-10/stage-sync-check.js` 與說明 `stage-sync-check.md`。
    - 寫法比照 `docs/research/2026-10-08/repo-projects-check.js`。
    - 使用新的固定 port，先 `netstat` 確認不與既有腳本重複。
    - 斷言手動標示的計算顏色與背景的對比不低於 4.5:1。
  - 驗收：新斷言先紅後綠；既有腳本全綠。
- [x] 5.2 「編輯 stage」對話框與加入（design D8、D9；spec `cockpit-dashboard`）。
  - 每列加階段下拉。選到已被他列使用的階段時，他列改回「不對應」。
  - 送出本體每列帶 `phase`；過期檢查涵蓋 `stage_phases`。
  - 跨整頁重畫保留下拉選擇（專案 memory `full-repaint-discards-state-held-only-in-dom`）。
  - 「加入」送出預設四站與對應的 `phases`。
  - 驗收：`stage-sync-check.js` 加斷言先紅後綠，涵蓋請求本體、唯一性、跨重畫保留、加入的本體；既有腳本全綠。
- [x] 5.3 設計審核（專案 memory `frontend-appearance-reviewed-by-frontend-design-skill`）。
  - 用 `ui_preview` 拍 1536、1100、700 三種寬度的卡片標示與對話框截圖，對照 `docs/direction-01-visual-design.md`，
    過 frontend-design 審核。
  - 截圖放 repo 外；要進 repo 的逐張看圖去識別化（專案 memory `deidentification-must-inspect-images-not-just-grep`）。
  - 會動到設計文件的建議交使用者決定。
  - 驗收：findings 已處理或記錄待使用者決定，ledger 有截圖路徑。

## 6. 文件

- [x] 6.1 文件。
  - `CONTEXT.md`：新增 OpenSpec 階段、同步狀態（自動／手動）詞條。
  - 新 ADR `docs/adr/0009-openspec-stage-sync.md`：進度來源取自 repo 檔案、自動與手動的優先權、狀態檔 v4（design D3、D4、D6）。
  - `cockpit/README.md`：功能說明、分支名對上 change 最準、10 秒反映、降版限制。
  - `CHANGELOG.md` 的 `## [Unreleased]`：註明狀態檔 v4 與降版限制。
  - 驗收：`markdownlint-cli2 "**/*.md"` 0 error，N 不為 0。

## 7. 收尾

- [x] 7.1 全 gate 與全部腳本，各指令單獨跑、逐一看結束碼。
  - gate 指令：
    - `cargo fmt --check`
    - `cargo clippy --all-targets -- -D warnings`
    - `cargo test --workspace`
    - `cargo test -p cockpit --example ui_preview`
    - `markdownlint-cli2 "**/*.md"`
    - `openspec validate --all`
  - 腳本：既有腳本加 `stage-sync-check.js`。
  - 去識別化：`node docs/research/2026-10-02/deid-check.js`。
  - 驗收：輸出貼進 ledger，全綠。
- [x] 7.2 整支分支 diff 審查：範圍 `main..feat/openspec-stage-sync` 全部 diff，處理方式同 4.7。
  驗收：ledger 記錄結論段與每個 finding 的處理。
- [x] 7.3 真機冒煙（Claude 執行，唯讀）。
  - 用 repo 外的臨時設定檔與狀態檔、未被佔用的 port 啟動 Cockpit，接 Windows 端 HERDR。
  - 以本 repo（有 `openspec/`）的真 pane 驗證：卡片對上 `openspec-stage-sync`，階段與 `tasks.md` 勾選一致。
  - 在 repo 外的暫存 repo 改 `tasks.md` 勾選，10 秒內卡片移動；手動推進後停住，再勾一項恢復自動。
  - WSL 防護驗證：前提是使用者的 WSL 發行版本來就沒在執行才驗；停止狀態下 Cockpit 跑 30 秒以上，用 `wsl.exe --list --running` 確認發行版仍未被開機。
    若發行版正在執行，記錄「未驗」與原因，不得為此關掉使用者的 WSL。
  - 結束後只關自己開的程序。
  - 結果寫進 `docs/research/2026-10-10/stage-sync-live.md`，截圖不進 repo。
  - 驗收：該檔存在、markdownlint 0 error。
- [x] 7.4 交接：重寫 `docs/handover.md`（依 `~/.claude/guides/handover-template.md`），寫明分支未併回、等使用者確認。
  驗收：markdownlint 0 error；`git status` 乾淨。
