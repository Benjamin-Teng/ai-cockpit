# git-review（change 5b）

## Why

change 5a 讓使用者能就地看 agent 產出的檔案，但看不到「agent 改了什麼」：哪些檔案還沒 commit、每個檔案改了哪幾行、
各 agent 的分支現在長什麼樣子。這些資訊今天得切到 VS Code 或終端機跑 git 才看得到。5a 已經知道每個 pane 所屬的 repo 根目錄
（`openspec/specs/file-review/spec.md`「檔案根目錄與允許清單」），差一層能安全讀取 git 資料的機制，以及 diff 與 Git Graph 的
畫面。2026-09-27 brainstorming 把原 change 5 拆成 5a（已完成）與本 change 5b；2026-09-28 brainstorming 決定本 change 的範圍，
使用者並授權 Claude 決定其餘細節，第二意見交 Codex 審核。

## What Changes

- **git 唯讀讀取層（新 crate `cockpit-git`）**：以 sealed trait 在編譯期限定只能執行的 git 查詢種類（status、refs、log、
  commit 內容、兩版本之間的檔案清單與單檔 diff、merge-base、讀取某版本的檔案內容）。Windows repo 以 Windows 的 `git` 執行，
  WSL repo 以 `wsl.exe -d <distro> --exec git -C <POSIX 路徑> …` 在 WSL 端執行，不修改任何 git 設定、不動 `safe.directory`
  （`docs/research/2026-09-27/file-review-probe.md` §3 的 dubious ownership 問題以此解決）。每次呼叫固定帶唯讀保護
  （不取 optional lock、不執行 fsmonitor／外部 diff／textconv、pathspec 一律字面解讀），有執行逾時、輸出上限與同時執行數上限。
- **git 端點**：新增 `GET /api/git/<runtime>/<root_id>/…` 一組唯讀端點（狀態、refs、commit 清單含圖形排版、commit 詳情、
  兩版本之間的變更檔案清單、單檔左右並排 diff、某版本的原始內容與 Markdown 渲染）。授權沿用 5a 的允許清單與本機同源檢查，
  且只對 `is_git` 為真的根目錄開放。
- **左欄「變更」分頁**：左欄頂端由「Project／檔案」改為「Project／檔案／變更」三個分頁。「變更」分頁列出目前選定 pane 所屬
  repo 的分支名稱與未 commit 的檔案，分「合併衝突」「已暫存」「變更」「未追蹤」四組，顯示時每 2 秒更新；點檔案開啟 diff 分頁。
- **diff 分頁（左右並排）**：舊版在左、新版在右，附行號，長行折行，變更區塊之間以「省略 N 行」分隔；新增／刪除以 `--ok`／
  `--bad` 推導的底色呈現。工具列可開啟該檔目前版本的檔案分頁、在 VS Code 開啟、或看某一側版本的內容。任一側為工作區或暫存區時，
  顯示中每 2 秒更新。
- **Git Graph 分頁**：每個 repo 一個分頁，畫出本地與遠端分支、tag 的 commit 圖（commit 依日期排序、分批載入），可依分支篩選、
  以訊息／作者／hash 搜尋已載入的 commit 並跳轉；點 commit 顯示詳情與變更檔案清單；選兩個 commit 可比較（直接比較或自分岔點起）；
  可複製 commit hash 與分支名稱。refs 改變時提示重新載入。
- **某版本的檔案分頁**：從 commit 詳情或 diff 開啟某個 commit 當時的檔案，沿用 5a 的四種檢視器。
- **分頁還原**：瀏覽器本機儲存格式升版，還原 diff、Git Graph 與某版本的分頁；舊格式的檔案分頁自動沿用。
- **驗收用 fixture**：`ui_preview` 的假 repo 改為真正以 `git init` 建立、含分支／合併／改名／二進位檔／衝突等情境的 repo。

## Capabilities

### New Capabilities

- `git-review`：git 唯讀讀取層的安全邊界、git 端點、左欄「變更」分頁、左右並排 diff 分頁、Git Graph 分頁、某版本的檔案分頁。

### Modified Capabilities

- `file-review`：「左欄檔案樹」的左欄分頁由兩個變三個；「檔案分頁」的分頁區除了檔案分頁外還容納 `git-review` 定義的分頁；
  「分頁還原」涵蓋新的分頁種類並相容舊格式。
- `cockpit-dashboard`：「路由與內嵌資源」加入 git 端點與新前端檔；「版面與窄視窗」的左欄分頁描述改為三個。

## 非目標

- **不做任何寫入 repo 的操作**：沒有 checkout、開分支、merge、rebase、cherry-pick、reset、tag、stage／unstage、commit、
  fetch、push；也不讀取 stash（Git Graph 以 `--branches --remotes --tags HEAD` 取代 `--all`，不含 `refs/stash`）。
- **不動 HERDR 唯讀約束**：只沿用 5a 已經取得的 pane `cwd` 與根目錄，不新增任何 HERDR method（設計文件 §2、ADR-0001）。
- **不修改使用者的 git 設定**：不寫 `safe.directory`、不改全域或系統設定；Windows 端 git 不直接讀 WSL repo。
- 不做單欄（unified）diff、行內字詞層級的差異標示、語法上色、展開省略的上下文、blame、單檔歷史、子模組內容 diff、
  LFS 內容還原（LFS 檔案顯示指標檔本身）。
- 不改投影 JSON 與 WebSocket 推送（設計文件 §6.4、ADR-0004）：git 資料一律由前端向獨立端點查詢。
- 不在 `cockpit.toml` 新增設定（設計文件 §8.2 的設定結構不變）。
- 不做 VS Code extension host（沿用 5a 的判斷）。

## Impact

- 新 crate `cockpit-git`（依賴方向：`cockpit` → `cockpit-git`；`cockpit-git` 不依賴任何 `cockpit-*` 與 `herdr-client`，
  ADR-0003 的邊界不變），新 ADR `docs/adr/0007-cockpit-git-crate.md`。
- `cockpit` 後端：新路由與 handler（`cockpit/src/git.rs`），沿用 `cockpit/src/files.rs` 的根目錄授權、路徑界限與原始內容回應規則；
  `cockpit/README.md` 補端點說明。
- 前端：新 `/app/git.js`；`files.js` 的左欄分頁、分頁區資料結構與本機儲存格式改版；`style.css` 新增 diff 與 Graph 樣式
  （顏色一律由十色 token 推導）。
- 驗收：`cockpit/examples/ui_preview.rs` 以 `git` 建立 fixture repo（執行測試與預覽的機器需要安裝 git）；新驗收腳本
  `docs/research/2026-09-28/git-check.js`；`files-check.js`、`visual-check.js` 依 spec 變更調整。
- 執行環境：Cockpit 服務會啟動 `git` 與 `wsl.exe` 子程序（只做唯讀查詢）。
