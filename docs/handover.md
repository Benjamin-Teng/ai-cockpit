# 交接：下一段任務

> **建立日期**：2026-10-11　|　**上一段做完的事**：change `openspec-stage-sync`（卡片依 OpenSpec 進度自動移動）實作與審查完成，
> 在分支 `feat/openspec-stage-sync`，**未併回 `main`、未發版、未 push**，等使用者確認。
> **性質**：接手用文件，會過期，每段重寫。
> 為什麼做看 `docs/cockpit-spec.md`（北極星）與各 change 的 proposal；怎麼做看 `~/.claude/CLAUDE.md`（本 repo 精簡版在
> `AGENTS.md`）；規格看 `openspec/specs/`；本功能的決策與審查紀錄看 `openspec/changes/openspec-stage-sync/`
> （`design.md`、`sdd-ledger.md`）與 `docs/adr/0009-openspec-stage-sync.md`。

## 0. 三十秒版本

1. **active change = `openspec-stage-sync`**，在分支 `feat/openspec-stage-sync`（領先 `main` 一批 commit，`main` 沒動、仍是 v0.1.5 時的狀態）。
   實作、逐 task 審查、Codex 與最終審查、真機冒煙都做完。**未併回、未發版、未 push**，等使用者確認。進度一律現場跑
   `openspec status --change openspec-stage-sync`，不要信任何文件裡的 task 進度。
2. **這一段做了什麼**：Repo Project 的卡片會依 repo 內的 OpenSpec 進度自動移動（唯讀：只讀 git 與 repo 內檔案，不呼叫 `openspec`
   指令、不對 HERDR 寫入）。pane 對應 change 先看分支名，對不上退回 archive 同名或「只有一個進行中的 change」；階段由 `tasks.md`
   勾選判定（規劃／實作／審查／完成）；每個 Stage 可設定對應哪個階段。**手動暫時優先**：人按推進或退回後卡片停住，偵測結果「下一次改變」
   才恢復自動。不自動貼 Completed／Failed。**狀態檔升到 v4**：v0.1.5 以前的版本讀到會拒絕啟動，降版只能刪檔。
3. **待使用者決定**：
   1. 是否併回 `main` 並發版。下一版版號未定（CHANGELOG 目前在 `## [Unreleased]`，發版流程見第 2 節）。
   2. 設計審核交付的問題：卡片的「手動」標示要不要加非顏色記號（虛線框或符號；現行 design D9 是一行文字，採用要改 D9）。
   3. 第二期「按鈕直接對 AI 下指令」（推進＝送訊息讓 AI 接受建議、Failed＝停止 agent）：需要 HERDR 寫入 method，會推翻 ADR-0001。
      使用者 2026-10-10 選了先做唯讀這期，之後再決定；要做就另開 change 與新 ADR。
4. repo **已公開**；**每次推送前**跑去識別化檢查（第 1 節指令，0 命中才推）；**歷史不再改寫**。跑 `deid-check.js` 可能把 WSL 開機（第 4 節）。
5. **驗收腳本用固定 port，同時只能有一個 session 跑**。本機常駐的無關監聽：7778（ASUS Armoury Crate）、7679（Google Drive）、
   7680（Windows 服務）——都不是本專案的，不可砍。

## 1. 現在的狀態

- **版本與版控**：GitHub `releases/latest` = **v0.1.5**（2026-10-08）。`main` 最新是 v0.1.5 之後的 handover commit；本機與遠端
  `main` 一致。`feat/openspec-stage-sync` **只在本機**，沒有遠端分支。併回前先 `git log main..HEAD` 看範圍。
- **使用者電腦**：桌面捷徑「AI Agent Cockpit」→ `%LOCALAPPDATA%\ai-cockpit\bin\cockpit-launch.exe --config "D:\projects\ai-cockpit\cockpit.toml"`，
  是 **v0.1.5 的 release 建置**（`install-desktop.ps1`，不會自動更新）。狀態檔在 `D:\projects\ai-cockpit\cockpit.state.json`（`.gitignore` 忽略）。
  - **踩降版地雷的路徑**：若用本分支的建置去跑使用者的真狀態檔，第一次偵測就會把它升成 v4，之後桌面捷徑的 v0.1.5 會拒絕啟動。
    真機冒煙是用 repo 外的臨時設定與狀態檔做的，**使用者的真狀態檔沒被動過**。
  - 要換成新建置：關 Cockpit 等 10 秒後 `pwsh -File scripts\install-desktop.ps1`（release 建置會印 MSVC「正在建立程式庫 .lib／.exp」，是例行訊息）。
  - 安裝檔版本沒裝。Ubuntu-24.04 可能被驗證工具開機而沒關，使用者要關自己 `wsl --terminate Ubuntu-24.04`；Claude 不要代關。
- **功能摘要**（細節看 `cockpit/README.md`「依 OpenSpec 進度自動移動卡片」、change 的 `design.md`；主規格 `openspec/specs/`
  會在 archive 時更新）：
  - 偵測：`cockpit/src/openspec_sync.rs` 每約 10 秒對 Repo Project 的每個 pane 取 worktree 目前分支（`cockpit-git` 新增的唯讀
    `CurrentBranch`，`symbolic-ref -q HEAD`）並讀 `openspec/changes/`。分支以最後一段對 change 名稱；對不上退回 archive 同名（多個日期取最大）
    或該 worktree 唯一進行中的 change；都不是就不顯示標示、維持手動。
  - 移動規則：核心在 `cockpit-core` 的 `domain/state.rs`（`apply_openspec`）。卡片有 `sync`（mode 自動／手動、上次套用的偵測結果 `applied`）；
    偵測結果與 `applied` 不同才移動；Completed／Failed 卡片不動。手動入口（推進、退回、agent 回報）與標記手動放在同一個寫入閉包。
  - 狀態檔 **v4**（`cockpit/src/progress.rs`）：Repo Project 多 `phases`，task 多選填 `sync`。v1–3 載入時，站名剛好是預設四站名（繁中或英文）的
    一次性補上對應；v3 檔帶 `phases`／`sync` 視為損毀。
  - 前端：卡片顯示 change 名稱、勾選數、自動／手動；「編輯 stage」對話框每列有階段下拉（附說明文字）。
  - 防護：WSL 路徑先 `wsl.exe --list --running --quiet` 探測，發行版沒在跑就不查；每個 WSL root 在 git 查詢前與讀檔前各重新探測一次，單輪內探測失敗即斷路。
    讀檔包逾時，同一 root 前一次讀檔未返回不派新的；`main.rs` 手動建 runtime 並以 `shutdown_timeout` 收尾，卡住的 9P 不拖住關機。
  - 上一版（`repo-projects`、`project-select-pane`）的功能不變：左欄「偵測到的 repo」、`POST /api/agent/advance` 免帶 id、選 project 自動選 pane、
    pane 消失要等 runtime 沉降重拿完成才清進度。
- **可用指令**（repo 根目錄）：
  - 全 gate：`cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test --workspace && cargo test -p cockpit --example ui_preview && markdownlint-cli2 "**/*.md" && openspec validate --all`
    （各指令單獨看結束碼，不要接管線，見 memory `gate-exit-code-swallowed-by-pipe`）
  - 推送前：`node docs/research/2026-10-02/deid-check.js && node docs/research/2026-10-02/deid-check.js --history`
  - 預覽：`cargo run -p cockpit --example ui_preview`（`127.0.0.1:7770`；fixture 含 Repo Project 與偵測到的 repo，已加同步標示的卡片）。
  - 驗收腳本（改了 `cockpit/assets/` 先 `cargo build -p cockpit --example ui_preview`；不可並行、一次一支）：清單在
    `openspec/changes/openspec-stage-sync/sdd-ledger.md` 第 7.1 節（17 支，含本段新增的 `docs/research/2026-10-10/stage-sync-check.js`）。
    跑完還原被重拍的截圖：`git checkout -- docs/research/2026-09-16 docs/research/2026-10-03`。
  - 看 change 進度：`openspec status --change openspec-stage-sync`
  - WSL 端 git 實測（會開機發行版，非必要不跑）：`cargo test -p cockpit-git --test real_git -- --ignored wsl_repo_identity`
- **測試與 gate**：各指令數字以當場跑為準（併回前必須重跑全 gate）；7.1 與 7.2 修正後的紀錄在 ledger。

## 2. 發版流程（下次發版照做）

改 `cockpit/Cargo.toml` 的 `version`、`cargo update -p webpki-roots`、`cargo check`，連同 `Cargo.lock` 提交 → `CHANGELOG.md` 的
`## [Unreleased]` 改 `## [X.Y.Z] - Unreleased` → 推 `vX.Y.Z-rc.N` 演練（release workflow 允許 rc 標題是 Unreleased）→ 綠了把標題改成日期、
推 `vX.Y.Z`（正式 tag 的標題是 Unreleased 會在建置前失敗）→ `gh release delete vX.Y.Z-rc.N --yes --cleanup-tag`。推正式 tag 前問使用者。
正式 tag 的 build 撞到偶發測試：`gh run rerun <id> --failed`，不用刪 tag。凍結契約（資產名稱、`SHA256SUMS.txt` 格式、安裝檔參數、
`unins000.exe`）見 `openspec/specs/release-distribution/`。這版的 CHANGELOG 已寫明降版限制（v4 狀態檔），發版時保留。

## 3. 接著要做

### 3.1 使用者確認後：併回、archive、發版

順序：重跑全 gate 與 17 支腳本 → 使用者同意後 squash 併回 `main`（慣例見 `~/.claude/guides/git-branch-workflow.md`）→ **`/opsx:archive`
走獨立 commit**（archive 會把新 capability `openspec-stage-sync` 併進主規格；新 capability 的 Purpose 可能缺空行造成 MD022，見 memory
`gate-exit-code-swallowed-by-pipe`，archive 後單獨跑 markdownlint）→ 依第 2 節發版（問使用者版號）→ 發版後更新桌面版。
併回前提醒使用者降版限制（桌面 v0.1.5 讀不了 v4 狀態檔）。刪分支（本地）與 SDD 工作區 `.superpowers/sdd/tasks-openspec-stage-sync/` 要先問。

### 3.2 自動更新加獨立簽章（使用者 2026-10-06：「列入」，未開 change）

**機制**：現在只驗「HTTPS＋同 release 的 `SHA256SUMS.txt`」，擋不住 release 本身被換掉（GitHub 帳號或 token 被盜、CI 被入侵時攻擊者
可同時上傳惡意安裝檔與對得上的雜湊檔）。獨立簽章是私鑰不放 GitHub、公鑰編進執行檔。要先白話說明後問使用者：私鑰放哪（GitHub
secrets vs 本機離線簽）、遺失或外洩的換鑰方案、已發出的版本升到第一個帶公鑰版本那一跳仍只靠 SHA-256、簽章檔怎麼發布（凍結契約之外）。
工具名稱與格式（例如 minisign）動手前查一手來源。

### 3.3 修 `abort_await_is_bounded` 的時序前提

`cockpit/tests/app.rs` 的前提檢查「blocking task 在放行前確實收不掉」在 CI 高負載時不成立（CI run 37217313821）；本機也偶發過一次
（先 `release_tx.send` 才檢查 `is_finished`，中間有競態）。修法：前提改成「等到條件成立」而不是假設立刻成立；修完推分支看 CI。

## 4. 已知限制與留待之後

本 change 最終審查（ledger「7.2 最終整支審查」）判定產品程式無必修缺陷，下列留待之後，都不在常見路徑、修法會牽動 spec：

- **暫時偵測失敗送 None，手動入口記下 `applied=None`，下一輪偵測恢復後卡片被拉回**：例如讀檔逾時那一輪手動推進，下一輪就被自動移回。
- **`set_pane_repos` 在狀態檔持續寫不進時整個不生效**（記憶體不變，pane 歸類停在舊的）。
- **WSL worktree 每 10 秒起 3 支 `wsl.exe`**（running 探測、git 查詢前後各一次重新探測）：發行版在跑時的固定成本。
- **首次偵測就把狀態檔升成 v4**，降版只能刪檔；可考慮升級前先備份 v3。
- WSL 的 `wsl.exe` 若每次都往 stderr 印警告，detached HEAD 會恆為錯誤（偵測不到分支）；`tasks.md` 被寫到一半時讀到會短暫判錯階段（下一輪自癒）。
- 殘餘風險（design Risks）：「剛停掉 WSL 的那一瞬間」偵測仍可能把它開回來；完全消除需要不經 `wsl.exe -d` 與 UNC 的機制。
- **真機沒驗到的**：cwd 在 `\\wsl.localhost\` 的 pane（WSL 防護的真正路徑）、分支對不上的後備、archive 後的 complete 階段——
  這三項只有腳本與 Rust 測試涵蓋（`docs/research/2026-10-10/stage-sync-live.md` 的「未驗項目」）。使用者哪天剛好有 WSL pane 時可補驗。
- 其餘 `minor (deferred)` 逐條在 ledger 各 task 條目（多為測試鑑別力與註解），要清理時依 ledger grep `minor (deferred)`。

## 5. 這一段踩過的坑

（**不會報錯的錯誤**加粗；機制都已寫進專案 memory。）

- **驗證工具本身會把 WSL 開機**：`deid-check.js` 為了取使用者名稱會跑 `wsl.exe -d`，把停止中的 Ubuntu-24.04 開起來（memory
  `verification-tools-can-boot-wsl`）。驗「Cockpit 不開 WSL」的真機冒煙要在跑任何驗證工具**之前**查 `wsl.exe --list --running`；
  工具跑完 WSL 變 running 不是 Cockpit 造成的。
- **驗收腳本收尾用 PID 判殘留會誤報（PID 重用、剛 taskkill 就判）**：已改成「PID＋建立時間」並在確認終止時自 OUR_PIDS 移除，
  `repo-projects-check.js`、`split-check.js`、`stage-sync-check.js` 都修了（memory `harness-pid-checks-misreport-on-windows`）；
  `split-check.js` 的「殘留 headless Chrome」偶發誤報屬同類，還沒修（候選：改短輪詢）。
- **背景工作偵測與寫入之間 pane 可能改歸類**：不比對歸類就會把舊 worktree 的結果套到新卡片並落檔。寫入服務在鎖內比對整個歸類
  （repo、worktree、root），不符當無結果（memory `check-against-lagging-projection-misses-fresh-writes`）。
- **用背景工作自己的 `last_sent` 去重會靜默且不自癒地丟結果**：歸類 A→B→A 時 domain 丟掉結果而 `last_sent` 不變。去重依據改為 domain 現有的 `openspec_obs`。
- **`git rev-parse --show-toplevel` 沒 canonicalize，CI（Windows 8.3 短名）上路徑比對失敗**：本機綠、CI 紅；偵測前先 canonicalize。
- **載入時 stage 不存在被重設為第一站，卻保留 auto 的 `applied`**：偵測不變就永久停錯站。載入重設 stage 且 mode 為 auto 時清 `applied`。
- **UNC 讀檔卡住會讓程式關不掉**：`#[tokio::main]` 的 runtime drop 會無限期等 `spawn_blocking`。讀檔包逾時、`main.rs` 用 `shutdown_timeout`。
- 診斷時 Git Bash 送 JSON 本體反斜線會被吃掉（回 400 `invalid_body`／404 `repo_not_detected`）：用 Node 寫檔再經標準輸入送。
- 沿用：Windows PowerShell 5.1 的 here-string 管線會在 JSON 前加 BOM（後端已容忍一個 BOM）；兩支腳本同時跑會互撞 port；
  截圖裡的使用者名稱（設計審核與真機冒煙的截圖放 `%TEMP%`，不進 repo）；其餘偶發見上一版本檔第 4 節（`git show 2e896f3:docs/handover.md`）。

## 6. 已定但未執行的決策

| 決策 | 狀態 |
|---|---|
| `feat/openspec-stage-sync` 併回、archive、發版 | **待使用者確認**（第 3.1 節） |
| 手動標示加非顏色記號（改 design D9） | **待使用者決定**；目前不做 |
| 第二期：按鈕直接對 AI 下指令（HERDR 寫入） | **待使用者決定**；需新 ADR 推翻 ADR-0001，目前不做 |
| 不自動貼 Completed／Failed，archive 也不貼 | 已定（承 change `progress-model`），不重議 |
| 手寫 `[[project]]` 不涵蓋自動同步（沒有 repo／worktree 對應） | 已定 |
| 不偵測 brainstorming 等尚未建立 change 的階段、不解析 pane 輸出、不用 LLM 判階段 | 已定，對不上 change 的卡片維持手動 |
| 設計審核 F8（worktree 標註加可見字樣）、F9（主要動作按鈕比「取消」亮一級） | **待使用者決定**；目前不做（沿用） |
| 同一 pane 被手寫自動綁定與 Repo Project 同時綁到時 `/api/agent/advance` 回 409 `ambiguous_task` | 依 design，罕見；該 agent 改用帶 id 端點（沿用） |
| git dubious ownership 時 repo 不出現在偵測區、只記 debug | 已知；repo 沒出現時先檢查 `git config --global safe.directory`（沿用） |
| Windows 與 WSL（`/mnt/d/...`）開同一個 repo 會列成兩個 | 已知限制，文件已寫（沿用） |
| WSL 探測把「在跑」也快取 60 秒（`cockpit/src/repo_resolver.rs` 的 `distro_probe`） | **待辦（小）**：`wsl --shutdown` 後 60 秒內若 Windows pane 停在 `\\wsl.localhost\` 路徑且快取未命中，仍可能把發行版開機一次（沿用） |
| 啟動器模式下關窗後 agent 回報遺失（change 11 整支審查 I1） | **待使用者裁決**（沿用） |
| 通知的真機確認（change 11 task 4.2） | **待使用者操作**（沿用） |
| 程式碼簽章（Authenticode）、Windows 以外的安裝檔、Tauri | **不做**（沿用） |
| 更早的延後項 | 見上一版本檔第 5 節（`git show 2e896f3:docs/handover.md`） |

## 7. 之後的路

北極星是讓一個人同時盯多個 coding agent 的工作狀態。本段讓卡片「自己跟著 AI 的進度走」，使用者不必每階段手動推進；
仍是唯讀。剩下：第二期的雙向互動（按鈕對 AI 下指令，需要 HERDR 寫入與新 ADR）、Repo Project 只有一條線一張卡（多卡與依賴仍要手寫設定）、
更新簽章與 Authenticode（SmartScreen 首次警告）。

## 版本紀錄

| 版本 | 日期 | 變更 |
|---|---|---|
| 1–45 | 2026-09-13～10-08 | 見 `git log -- docs/handover.md`（change 1a～project-select-pane、v0.1.0～v0.1.5） |
| 46 | 2026-10-11 | change `openspec-stage-sync` 在 `feat/openspec-stage-sync` 完成（SDD、逐 task 審查、Codex 與最終審查、設計審核、真機冒煙），未併回、未發版，待使用者確認 |
