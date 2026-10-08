# 交接：下一段任務

> **建立日期**：2026-10-08　|　**上一段做完的事**：change `repo-projects`（畫面加入 git repo 成為 Project、agent 免帶 id 推進）
> 在分支 `feat/repo-projects` 實作完成並通過全部審查與 gate，**尚未併回 `main`、尚未 archive、尚未發版**。使用者 2026-10-08
> 授權 Claude 全權決定設計與實作，回來後要看決定清單（第 2 節）再決定是否併回。審查由 Opus 5.5 擔任（本專案視同 Codex）。
> **性質**：接手用文件，會過期，每段重寫。
> 為什麼做看 `docs/cockpit-spec.md`（北極星）與各 change 的 proposal；怎麼做看 `~/.claude/CLAUDE.md`（本 repo 精簡版在
> `AGENTS.md`）；規格看 `openspec/specs/` 與 `openspec/changes/repo-projects/`。

## 0. 三十秒版本

1. **等使用者確認 `feat/repo-projects`**：看第 2 節的決定清單，同意後照第 2 節的指令 squash 併回 `main`、推送、archive。
   **併回前不要開別的分支改同一批檔案**（`cockpit/src/progress_service.rs`、`render.js`、`actions.js`、`i18n.js` 改動很大）。
2. **狀態檔格式升到 v3**：併回並發版後，v0.1.3 以前的版本讀到新狀態檔會拒絕啟動（CHANGELOG 已寫）。使用者桌面捷徑的
   安裝（`install-desktop.ps1`）若換成本分支建置，舊版就回不去，要先備份 `D:\projects\ai-cockpit\cockpit.state.json`。
3. 其餘候選工作（沿用上一版）：自動更新加獨立簽章、修 `abort_await_is_bounded` 的時序前提（第 3 節）；還沒問到的兩件事：
   通知真機確認、啟動器模式下關窗後 agent 回報遺失可否接受（第 5 節）。
4. repo **已公開**；**每次推送前**跑去識別化檢查（第 1 節指令，0 命中才推）；**歷史不再改寫**。
5. **驗收腳本用固定 port，同時只能有一個 session 跑**。本段新增 `repo-projects-check.js` 用 7950／CDP 19610。本機常駐的無關監聽：
   7778（ASUS Armoury Crate）、7679（Google Drive）、7680（Windows 服務）——都不是本專案的，不可砍。

## 1. 現在的狀態

- **版本**：GitHub `releases/latest` = **v0.1.3**（2026-10-07）。`main` 從 `2e896f3` 起沒有變動。
- **分支 `feat/repo-projects`**（本機，未推送）：自 `main` 起 70 多個 commit，最後一段是最終修正波（`04132c4`～`6b88a39`）與本檔。
  change 目錄 `openspec/changes/repo-projects/`：proposal、design（D1～D9）、9 份 delta spec、tasks（全部勾選）、`sdd-ledger.md`
  （基線、git 查證、全部審查紀錄、收尾驗證、最終修正）。
- **功能摘要**（細節看 design 與 `cockpit/README.md`）：
  - 左欄 Project 分頁多「偵測到的 repo」區：Cockpit 對已連線 runtime 的每個 pane cwd 跑
    `git rev-parse --path-format=absolute --git-common-dir --git-dir --show-toplevel`（`cockpit/src/repo_resolver.rs`），同一個 repo 的
    所有 worktree 歸成一個。按「加入」→ Repo Project：每個 pane 一條工作線、每條一張卡、預設四個 stage；「⋯」可改名、編輯 stage、移除。
  - 定義與進度存狀態檔 **v3** 的 `repo_projects`（`cockpit/src/progress.rs`），與手寫 `[[project]]` 的進度分開；零設定模式的狀態檔在
    `%LOCALAPPDATA%\ai-cockpit\cockpit.state.json`。
  - `POST /api/agent/advance`：不帶 id，從 `X-Herdr-Pane-Id` 找到唯一一張卡推進（候選在寫入鎖內依 Domain 計算）。
  - pane 從 HERDR 消失才清進度，而且要等 runtime **沉降重拿完成**（`ConnectionState::Connected.settled`）；已 exited 不清。
  - 查 WSL 路徑前先 `wsl.exe --list --running --quiet` 探測（5 秒逾時、結果沿用 60 秒），發行版沒在跑就不查（不會把它開機）。
- **使用者電腦**：
  - 桌面捷徑「AI Agent Cockpit」→ `install-desktop.ps1` 安裝的 0.1.3（`main` 建置），**不含本分支**。
  - **Ubuntu-24.04 被本段的 WSL 實測開機後沒有關**（最終修正波 I2，`cargo test -p cockpit-git -- --ignored wsl_repo_identity`）。
    使用者若要關，自己 `wsl --terminate Ubuntu-24.04`；Claude 不要代關（可能有使用者的 pane 在跑）。
  - 本段期間 7770 曾短暫出現一支不是本 session 開的 `ui_preview.exe`（已自行結束），推測是其他 session；跑腳本前一律先查 port。
- **可用指令**（repo 根目錄）：
  - 全 gate：`cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test --workspace && cargo test -p cockpit --example ui_preview && markdownlint-cli2 "**/*.md" && openspec validate --all`
  - 推送前：`node docs/research/2026-10-02/deid-check.js && node docs/research/2026-10-02/deid-check.js --history`
  - 預覽：`cargo run -p cockpit --example ui_preview`（`127.0.0.1:7770`；fixture 已含 Repo Project「Demo App」與兩個偵測到的 repo）。
  - 驗收腳本（改了 `cockpit/assets/` 先 `cargo build -p cockpit --example ui_preview`；不可並行、一次一支）：上一版的 15 支
    （清單在 `openspec/changes/repo-projects/sdd-ledger.md` 第 1 節）＋本段新增 `docs/research/2026-10-08/repo-projects-check.js`
    （28 段，用法 `repo-projects-check.md`）。跑完還原 `docs/research/2026-09-16/task-5.2-scenario-d.png`；i18n 截圖在本段已更新為
    含偵測區的版本，之後外觀沒改就 `git checkout -- docs/research/2026-10-03`。
  - WSL 端 git 實測（會開機發行版，非必要不跑）：`cargo test -p cockpit-git --test real_git -- --ignored wsl_repo_identity`
- **測試數字**（2026-10-08，最終修正第二波 `d4590fd` 後；當場跑為準）：workspace 1518 passed／0 failed／17 ignored；ui_preview 82；
  markdownlint 220 files 0 issues；`openspec validate --all` 25 passed；16 支驗收腳本全綠；去識別化兩種模式無命中。

## 2. 立刻要做：使用者確認 `feat/repo-projects` 後併回

**先給使用者看的決定清單**（完整清單在 `openspec/changes/repo-projects/sdd-ledger.md` 第 8 節；重點如下，
完整版在本 session 最後的回報）：

- 產品面：Project＝git repo（含所有 worktree）；按一下加入；一個 pane 一條工作線、每條一張卡；預設 stage「規劃、實作、審查、完成」；
  第一版只做加入／移除／改名／編輯 stage；手寫 `cockpit.toml` 繼續支援、撞名時手寫優先。
- 交使用者決定、這次**沒做**的：設計審核 F8（worktree 標註要不要加可見的「worktree」字樣，目前只有資料夾名＋title）、
  F9（主要動作按鈕要不要比「取消」亮一級，會改設計文件）。

**使用者同意後的步驟**：

```bash
git switch main && git merge --squash feat/repo-projects
git commit   # 訊息：feat: Repo Project（change repo-projects）＋ Co-Authored-By
node docs/research/2026-10-02/deid-check.js && node docs/research/2026-10-02/deid-check.js --history
git push origin main   # 推送後看 CI（ci.yml）綠
openspec archive repo-projects   # 主規格合併後跑 openspec validate --all；archive 新 capability 的 Purpose 段落要有空行（MD022）
git branch -D feat/repo-projects   # 刪分支要使用者明確指示
```

（專案 memory 的候選工作已在本段改成只剩更新簽章：`pending-update-signing`。）發新版照
第 0 節舊流程（`cockpit/Cargo.toml` 版本、`CHANGELOG.md` 的 `## [Unreleased]` 改版號、rc 演練）。

## 3. 接著要做

### 3.1 自動更新加獨立簽章（使用者 2026-10-06：「列入」）

**機制**：現在只驗「HTTPS＋同 release 的 `SHA256SUMS.txt`」，擋不住 release 本身被換掉（GitHub 帳號或 token 被盜、CI 被入侵時攻擊者
可同時上傳惡意安裝檔與對得上的雜湊檔）。獨立簽章是私鑰不放 GitHub、公鑰編進執行檔。要先白話說明後問使用者：私鑰放哪（GitHub
secrets vs 本機離線簽）、遺失或外洩的換鑰方案、已發出的版本升到第一個帶公鑰版本那一跳仍只靠 SHA-256、簽章檔怎麼發布（凍結契約之外）。
工具名稱與格式（例如 minisign）動手前查一手來源。

### 3.2 修 `abort_await_is_bounded` 的時序前提

`cockpit/tests/app.rs` 的前提檢查「blocking task 在放行前確實收不掉」在 CI 高負載時不成立（CI run 37217313821）；本段 4.6 修正期間
本機也偶發一次（先 `release_tx.send` 才檢查 `is_finished`，中間有競態）。修法：前提改成「等到條件成立」而不是假設立刻成立；
修完推分支看 CI。

## 4. 這一段踩過的坑

（**不會報錯的錯誤**加粗；機制都已寫進專案 memory。）

- **「沒紀錄就用預設」而預設取自當下定義**：編輯 stage 時只遷移有紀錄的卡片，新加入的卡片全部靜默跳到新的首 stage
  （memory `implicit-default-from-current-definition-drifts-on-edit`）。
- **用落後的投影選「要寫哪一筆」**：免帶 id 推進在 `start` 後立刻呼叫會推進舊的卡；實作者曾以 `wait_state` 讓測試等投影而「修好」，
  等於讓測試遷就缺陷（memory `check-against-lagging-projection-misses-fresh-writes` 例證 2）。
- **防護檢查資料的擁有者，副作用卻由資料內容決定**：「runtime 已連線」擋不住 Windows pane 的 `\\wsl.localhost` cwd 讓 `wsl.exe` 開機
  （memory `guard-checks-owner-but-side-effect-follows-data`）。
- **HERDR 連上後的首份 snapshot 可能不完整**：據以清除會不可逆刪進度 → 等沉降重拿完成（`settled`）。
- **pane 歸類一變就寫檔會把 v2 狀態檔在沒有任何操作下升成 v3** → 比較序列化後的內容才寫。
- **Windows PowerShell 5.1 的 here-string 管線會在 JSON 前加 BOM**，`serde_json` 回 400 → 後端已容忍開頭一個 BOM。
- Windows git 對同一資料夾以不同大小寫 `cd` 進去會回不同大小寫的路徑 → Windows repo key 整串小寫；WSL 保留大小寫。
- `git rev-parse` 在裸 repo／`.git` 內會先印兩行才 exit 128 → 非零結束時不看 stdout；只有 128 算「不是 repo」（WSL 找不到 git 回 127）。
- 兩支腳本同時跑會互撞 port、FAIL 像偶發；實作 subagent 曾誤開兩份批次 → 派工時寫明「一次一支、絕不同時兩份」。
- 截圖裡的使用者名稱：i18n-check 會把路徑遮成 `<user>`，但設計審核與真機冒煙的截圖沒有遮 → 一律放 `%TEMP%`，不進 repo。
- 既有偶發（沿用）：見上一版本檔第 4 節（`git show 2e896f3:docs/handover.md`）；本段 7.1 全量回歸一次全綠。

## 5. 已定但未執行的決策

| 決策 | 狀態 |
|---|---|
| `feat/repo-projects` 併回、archive、發版 | **待使用者確認**（第 2 節） |
| 設計審核 F8（worktree 字樣）、F9（主要按鈕加亮） | **待使用者決定**；目前不做 |
| 同一 pane 同時被手寫自動綁定與 Repo Project 綁到時 `/api/agent/advance` 回 409 `ambiguous_task` | 依 design D7 字面，罕見；該 agent 改用帶 id 端點 |
| git dubious ownership 時 repo 不出現在偵測區、只記 debug | 已知；repo 沒出現時先檢查 `git config --global safe.directory` |
| Windows 與 WSL（`/mnt/d/...`）開同一個 repo 會列成兩個 | 已知限制，文件已寫 |
| WSL 探測把「在跑」也快取 60 秒：`wsl --shutdown` 後 60 秒內若 Windows pane 停在 `\\wsl.localhost\` 路徑且快取未命中，仍可能把發行版開機一次 | **待辦（小）**：只快取「沒在跑」，或 WSL runtime 斷線時清快取（`cockpit/src/repo_resolver.rs` 的 `distro_probe`）；最終修正第二波複審的 Minor，因機率低而停在這裡 |
| 已不在設定中的 runtime，其 Repo 進度載入時忽略、下次寫入不寫出 | 設計行為 |
| 「⋯」選單用 Tab 切換的按鈕組，不做方向鍵 ARIA menu | 設計審核建議不做（只有三項） |
| 啟動器模式下關窗後 agent 回報遺失（change 11 整支審查 I1） | **待使用者裁決**（沿用） |
| 通知的真機確認（change 11 task 4.2） | **待使用者操作**（沿用） |
| 程式碼簽章（Authenticode）、Windows 以外的安裝檔、Tauri | **不做**（沿用） |
| 更早的延後項 | 見上一版本檔第 5 節（`git show 2e896f3:docs/handover.md`） |

## 6. 之後的路

北極星是讓一個人同時盯多個 coding agent 的工作狀態。本段補上了「一般使用者上手」最大的缺口（不必手寫 `cockpit.toml`、agent 回報
一行通用指令）。剩下：Repo Project 只有一條線一張卡（多卡與依賴仍要手寫設定）、更新簽章與 Authenticode（SmartScreen 首次警告）。

## 版本紀錄

| 版本 | 日期 | 變更 |
|---|---|---|
| 1–42 | 2026-09-13～10-07 | 見 `git log -- docs/handover.md`（change 1a～file-split-view、v0.1.0～v0.1.3） |
| 43 | 2026-10-08 | change `repo-projects` 在 `feat/repo-projects` 完成（SDD、Opus 逐 task 審查＋後端跨 task＋整支分支審查、設計審核、真機冒煙），待使用者確認併回 |
