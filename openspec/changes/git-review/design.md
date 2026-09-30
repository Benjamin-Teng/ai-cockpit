# design：git-review

## Context

動機見 `proposal.md`；行為契約見本 change 的 `specs/`。本 change 的細節由使用者 2026-09-28 授權 Claude 決定，第二意見交
Codex 審核。現況中與本設計有關的限制：

- 5a 的根目錄以 `Root { path, is_git }` 表示（`cockpit-files/src/root.rs`），`path` 永遠是主機路徑；WSL 的根目錄是
  `\\wsl.localhost\<distro>\...` 形式，distro 與 POSIX 路徑沒有另外存，要從 UNC 路徑拆回（`cockpit/src/files.rs` 的
  `vscode_uri` 已這樣做）。允許清單由 `authorize_root` 在每個請求當下依最新投影推算（同檔）。
- Windows 端 git 對 WSL repo 會報 dubious ownership（`docs/research/2026-09-27/file-review-probe.md` §3）。
- 2026-09-28 實測（brainstorming 期間，Windows git 2.50.1、WSL git 2.43.0、Ubuntu-24.04）：
  - 對同一個 WSL repo，`wsl.exe` 在 WSL 端執行 git 每次約 66–122 ms（其中啟動 `wsl.exe` 約 65–78 ms），Windows git 加
    `-c safe.directory` 經 `\\wsl.localhost` 讀取約 100–210 ms；Windows 本機 repo 約 27–83 ms。
  - `wsl.exe -d <distro> -- <cmd> <args>` 會經 Linux 預設 shell 重新解析引數（`'$HOME'` 被展開）；`--exec` 不經 shell。
  - `wsl.exe --cd <不存在的路徑>` 靜默退回 `/` 且 exit 0。
  - 經 `wsl.exe --exec` 轉呼叫的 Linux 程式 stdout 是原本的 UTF-8（只有 `wsl.exe` 自己的輸出是 UTF-16）。
- 前端分頁區目前只有 Live Output 與檔案分頁兩種，沒有 kind 欄位；本機儲存鍵 `cockpit.fileTabs` 格式 `v:1`
  （`cockpit/assets/app/files.js`）。
- `ui_preview` 的 fixture repo 只有空的 `.git/` 資料夾，不是真的 git repo（`cockpit/examples/ui_preview.rs`
  `setup_review_fixture`）。
- 十色 token 與四階字級規則（`cockpit/assets/app/style.css`、`cockpit-dashboard`「Direction 01 視覺語彙」）照舊適用於
  diff 與 Graph 的 UI；diff 的內容文字屬 Cockpit 自己的 UI（不是 5a 例外清單裡的「檔案內容」），同樣受規則約束。

## Goals / Non-Goals

**Goals：**

- 「只讀」由編譯器與固定引數保證：能執行的 git 查詢種類在 `cockpit-git` 內封閉，HTTP 層拿不到任意 git 引數的入口。
- 不放大權限：git 一律以 repo 所在環境的使用者身分執行（WSL repo 在 WSL 內），不為了讀取而關掉 git 的 ownership 檢查。
- 不干擾 agent：cockpit 的讀取不拿 index 鎖、不寫任何檔案，agent 同時 `git commit` 不會因 cockpit 失敗。
- 可單元測試：解析、左右並排對齊、Graph 排版都是純函式，不需要 git 也能測。

**Non-Goals（設計層面）：**

- 不快取 git 結果；每個請求即時執行 git（輪詢只對顯示中的內容，見 D8）。
- 不做 Graph 的虛擬捲動；以分批載入與 5000 筆上限控制 DOM 大小。
- 不支援 SHA-256 以外的新雜湊格式；hash 只接受 40 或 64 個十六進位字元。

## Decisions

### D1 新 crate `cockpit-git`：封閉查詢＋執行器＋純解析

`cockpit-git` 對外提供：查詢型別與其輸出型別、執行器 `GitRunner`、執行目標 `GitTarget`、左右並排對齊、Graph 排版、
版本與路徑的值型別。它不知道 HTTP、HERDR、投影與 runtime 設定；依賴 `tokio`（process、io-util、time、sync）、`serde`。
`cockpit` 依賴它；`cockpit-core`、`cockpit-herdr`、`herdr-client`、`cockpit-files` 都不依賴它，它也不依賴它們。新增
`docs/adr/0007-cockpit-git-crate.md` 記錄：這條依賴邊界、5a「伺服器不執行任何程式」在本 capability 改為「只執行封閉清單內
的 git 唯讀查詢」、以及 D2 的執行方式與理由。

- **查詢封閉**：`pub trait GitQuery: private::Sealed { type Output; }`，寫法照 `herdr-client/src/client/request.rs`
  （`pub(crate) mod private { pub trait Sealed {} }`），附 `compile_fail` doctest 證明 crate 外無法實作。每個查詢型別在
  crate 內決定完整的 git 子命令與引數（呼叫端只能提供已驗證的值型別：`Oid`、`RepoPath`、`RefName`、`Side`），並附輸出解析器。
  查詢清單：`Status`、`Refs`、`Log`、`CommitInfo`、`ChangedFiles`、`FileDiff`、`MergeBase`、`BlobSize`、`Blob`、`VerifyCommit`、`BlobId`、`BlobHead`。
- **替代方案**：gitoxide（`gix` 0.88）不需要子程序，但讀 WSL repo 一樣要走 `\\wsl.localhost`（慢）、diff 與 rename 偵測要自己組、
  依賴樹大；直接在 `cockpit` 寫 `Command::new("git")`（沒有編譯期封閉，安全邊界散在 handler 裡）。都不採用。

### D2 執行方式：依根目錄形式選 Windows git 或 WSL 端 git

- `GitTarget::Native { path }` → `git -C <path> <固定前綴> <子命令…>`。
- `GitTarget::Wsl { distro, posix }` → `wsl.exe -d <distro> --exec env LC_ALL=C git -C <posix> <固定前綴> <子命令…>`。
  一律 `--exec`（不經 shell），不用 `--`、不用 `--cd`（見 Context 的兩個實測）。`env` 是 `--exec` 直接執行的程式，同樣不經
  shell，用來設定 `LC_ALL=C`，讓錯誤訊息固定為英文以便分類（D6）。Native 以子程序環境變數設 `LC_ALL=C`。
- **目標怎麼選**：看根目錄的主機路徑，不看 runtime。路徑以 `\\wsl.localhost\<distro>\` 或 `\\wsl$\<distro>\`（不分大小寫）開頭
  → `Wsl`，POSIX 路徑為其餘各段以 `/` 串接並加前導 `/`；其餘 → `Native`。所以 Windows runtime 的 pane 若 `cd` 到 UNC 路徑，
  也走 WSL 端 git。distro 名稱只接受 `[A-Za-z0-9._-]`，POSIX 各段沿用 5a 的 `is_plain_segment` 檢查。
- **固定前綴**（每個查詢都帶）：`--no-pager --no-optional-locks --literal-pathspecs -c core.fsmonitor=false
  -c core.quotepath=false -c color.ui=false -c log.showSignature=false -c gc.auto=0 -c diff.suppressBlankEmpty=false`；diff 類查詢另加
  `--no-ext-diff --no-textconv --no-color`。`--no-optional-locks` 讓 `git status` 不刷新、不寫 index（官方文件：等同
  `GIT_OPTIONAL_LOCKS=0`），避免與 agent 的 `git commit` 搶 `index.lock`；`--literal-pathspecs` 讓 `:(glob)` 這類
  pathspec 語法失效，路徑一律字面解讀；`diff.suppressBlankEmpty=false` 擋掉 repo 設為 true 時空白 context 行變成 `""`（非 `" "`）而被解析器漏列、行號錯位。版本參數之後、路徑之前一律放 `--`；版本參數前放 `--end-of-options`（git 支援的
  子命令）。每個旗標在兩個版本的 git 上都要實測可用（task 1.2），不可用的旗標改用等價寫法並記在實測文件。
- Windows 上啟動子程序帶 `CREATE_NO_WINDOW`（同 `cockpit-herdr/src/probe.rs`），`kill_on_drop(true)`，stdin 為 null。
- **已知且接受**：repo 若設定 clean filter（例如 git-lfs），`git status`／工作區 diff 會執行它。WSL repo 在 WSL 內以擁有者本人
  身分執行、Windows repo 由 Windows 使用者本人擁有，等同使用者自己在該 pane 下 `git status`，不放大權限；而 fsmonitor、
  外部 diff、textconv、簽章驗證這些可由 repo 設定觸發、又不影響讀取正確性的外部程式一律關掉。Windows git 對「擁有者不是目前
  使用者」的 repo 仍會拒絕（dubious ownership），本 change **不繞過**，只把它分類成可讀的錯誤。
- **替代方案**：Windows git 一律加 `-c safe.directory=<路徑>`（實測可用且只有一條路徑，但慢，而且等於讓 WSL 內任何 repo 的
  設定能以 Windows 使用者身分執行程式，是跨邊界的權限放大），不採用。

### D3 執行器：逾時、輸出上限、並行上限

- 同時最多 4 支 git 子程序（`tokio::sync::Semaphore`），排隊時間計入逾時。單次查詢逾時 10 秒（含排隊），逾時即終止子程序並回
  `git_timeout`。
- stdout 邊讀邊數位元組（同 5a `read_capped` 的理由：避免先看大小再整份讀的競態）；各查詢的上限：`Status` 4 MiB、`Log` 8 MiB、
  `ChangedFiles` 4 MiB、`FileDiff` 8 MiB、`Blob` 50 MiB（同 5a 原始內容），其餘 1 MiB。超過上限時終止子程序；`Status` 與
  `ChangedFiles` 改回傳已完整解析的前段並標 `truncated: true`（NUL 分隔的紀錄可以安全截斷在最後一個完整紀錄），其他查詢回
  `too_large`。stderr 最多保留 8 KiB，只寫進服務日誌，不放進回應本體。
- `Blob` 先以 `BlobSize`（`cat-file -s`）確認大小，超過 50 MiB 直接回 `too_large`，不啟動讀取。

### D4 git 子命令與解析

| 查詢 | git 子命令（不含固定前綴） | 解析重點 |
|---|---|---|
| `Status` | `status --porcelain=v2 -z --branch --untracked-files=all` | `#` 標頭取分支、上游、ahead／behind；`1`／`2`／`u`／`?` 紀錄；`2`（改名）多一個 NUL 分隔的原路徑 |
| `Refs` | `for-each-ref --format=<refname、objectname、*objectname、symref 以 %00 分隔> refs/heads refs/remotes refs/tags` ＋ `rev-parse --verify -q HEAD`、`symbolic-ref -q HEAD` | 附註 tag 以 `*objectname`（剝皮後的 commit）為準；`refs/remotes/*/HEAD` 的 symref 略過 |
| `Log` | `log --date-order -z --format=%H%x00%P%x00%an%x00%ae%x00%at%x00%s -n <上限> --end-of-options <tips…>` | 每筆固定 6 欄，欄與筆都以 NUL 分隔，依欄數切分 |
| `CommitInfo` | `show -s -z --format=%H%x00%P%x00%an%x00%ae%x00%at%x00%cn%x00%ce%x00%ct%x00%B --end-of-options <oid>` | `%B` 放最後，可含換行 |
| `ChangedFiles` | 依兩側組合（D5）：`diff-files`（INDEX→WORKTREE）／`diff --cached`／`diff <a> <b>`／`diff-tree -r --root <b>`（`diff-index <a>` 僅 FileDiff 用），分兩次呼叫：一次加 `-z -M --name-status`、一次加 `-z -M --numstat` | 以路徑合併：狀態字母、路徑、改名原路徑、增刪行數（二進位檔為 null） |
| `FileDiff` | 同上但輸出 patch：`-M -U3 -- <old_path> <path>`；plumbing 子命令（`diff-tree`／`diff-files`／`diff-index`）預設輸出 raw，要另加 `-p` | 見 D7 |
| `MergeBase` | `merge-base --end-of-options <a> <b>` | 無共同祖先時 exit 1 → `no_merge_base` |
| `BlobSize`／`Blob` | `cat-file -s <oid>:<path>`、`cat-file blob <oid>:<path>`（暫存區為 `:<path>`） | 物件不存在 → `not_found_in_rev` |
| `VerifyCommit` | `rev-parse --verify -q --end-of-options <oid>^{commit}` | 不存在 → `rev_unknown` |
| `BlobId` | `rev-parse --verify -q --end-of-options <oid>:<path>`（暫存區為 `:<path>`） | 回傳檔案內容的物件 hash，供 `meta` 的 `blob` 欄位（不讀取內容）；不存在 → `not_found_in_rev`（task 3.3 後新增，Ruling R8） |
| `BlobHead` | 同 `Blob`，但 stdout 上限 8192 位元組且可截斷（讀滿即終止子程序） | 供 `meta` 依 5a 規則以前 8192 位元組分類 `viewer`，不整份讀取（Ruling R10） |

- `--name-status` 與 `--numstat` 不能在同一次呼叫同時輸出（task 1.2 實測：兩版本 git 都只印 `--name-status`），所以
  `ChangedFiles` 固定分兩次呼叫再以路徑合併（`docs/research/2026-09-28/git-review-probe.md` ⑤）。兩次呼叫之間工作區可能改變，
  合併時以 `--name-status` 為準，`--numstat` 缺的路徑增刪行數為 null。
- 路徑不是合法 UTF-8 的紀錄不回傳（前端網址無法表示），計入 `skipped`，與 5a 列目錄一致。作者、訊息等顯示用文字以有損方式
  轉成 UTF-8。
- Git Graph 不用 `--all`：未篩選時 tips 為 `Refs` 列出的全部 refs 的 commit 加上 `HEAD`，所以不含 `refs/stash` 與其他
  非分支命名空間。

### D5 版本與兩側組合

- 前端送來的版本（side）只有四種：`Oid`（40 或 64 個小寫十六進位字元，執行前以 `VerifyCommit` 確認是 commit）、`INDEX`、
  `WORKTREE`、`EMPTY`。`HEAD` 與分支名稱都由前端先從 `refs`／`status` 的結果換成 hash 再送；Graph 的分支篩選則送完整 refname，
  伺服器以當下 `Refs` 的結果逐字比對，不在清單內就回 `ref_unknown`，比對通過才換成 hash 放進 `Log` 的 tips。
  因此使用者可控的字串永遠不會以「版本語法」交給 git 解讀（`HEAD~1`、`@{-1}`、`:/訊息` 這類語法都進不來）。
- 允許的組合（`from` → `to`）：`Oid`→`INDEX`（已暫存；repo 還沒有任何 commit 時用 `EMPTY`→`INDEX`）、`INDEX`→`WORKTREE`（變更）、
  `EMPTY`→`WORKTREE`（未追蹤檔案，僅 `FileDiff`，不經 git，見 D7）、`Oid`→`WORKTREE`（合併衝突中的檔案，僅 `FileDiff`；未合併檔案的 `INDEX`→`WORKTREE` 會得到 `diff --cc` 三方格式，
  改以 HEAD 為左側，解析器遇到 `diff --cc` 回 `unmerged_path`，Ruling R11）、`Oid`→`Oid`（commit 詳情與兩 commit 比較）、
  `EMPTY`→`Oid`（根 commit）。其他組合回 `bad_request`。
- **工作區側一律用 plumbing（Ruling R13）**：`INDEX`→`WORKTREE` 用 `diff-files`、`Oid`→`WORKTREE` 用 `diff-index <oid>`（不加 `--cached`）。
  原因：porcelain `git diff` 遇到工作區檔案 stat 變舊（例如只 touch）會刷新 stat cache 並**重寫 `.git/index`**（取得 `index.lock`），
  `--no-optional-locks` 擋不住（該旗標只管 `git status` 這類「可選」的刷新）；diff 分頁每 2 秒輪詢，會跟 agent 的 `git add`／`commit`
  搶 `index.lock`。`diff-files`／`diff-index` 不刷新 stat、不寫 index（Windows git 2.50.1 實測）。代價有二：預設輸出 raw，`FileDiff` 要加 `-p`；
  不刷新 stat，所以「只 touch、內容沒變」的檔案在 `--name-status` 誤報 `M`，但 `--numstat` 會真的比對內容而不列出它（純權限變更仍列為
  `0\t0\t<path>`）——`ChangedFiles` 解析器對 `diff-files` 丟掉「狀態 `M` 且不在 numstat 輸出中」的路徑（numstat 被截斷時不濾）。
  `diff --cached` 與 `diff-tree` 本來就不碰工作區，不受影響。
- commit 詳情的檔案清單與第一個父 commit 比較（merge commit 也一樣，畫面註明「與第一個父 commit 比較」）；根 commit 與 `EMPTY` 比較。
- 「自分岔點起」比較：前端先查 `merge-base`，再以 `merge-base`→`to` 呼叫一般比較。

### D6 端點與錯誤對應

路由前綴 `/api/git/<runtime>/<root_id>`，全部 `GET`，沿用 5a 的 `source_check`、405 fallback、`Cache-Control: no-store`、
`nosniff` 與錯誤本體格式。流程：解析參數（在啟動任何子程序之前完成所有格式驗證）→ `authorize_root`（5a）→ 根目錄
`is_git` 為假回 `not_git` → 依根目錄選 `GitTarget` → 執行查詢。端點清單與回應欄位以 spec 為準。

- 路徑參數：`status`、`refs`、`log`、`changes`、`merge-base` 無路徑；`diff` 的 `path`／`old_path` 放在 query string（逐字解碼一次，
  套用 5a 相對路徑的片段規則）；`blob`、`render`、`meta` 以 `/<rev>/<相對路徑>` 放在網址路徑，讓 HTML 與 Markdown 的相對
  子資源自然解析到同一個版本（與 5a 原始內容端點同理）。query string 一律從原始 URI 自行解析（同 5a「axum 會先解碼」的坑），
  `log` 的多個 `ref`／`tip` 以重複參數表示。
- 錯誤分類（stderr 在 `LC_ALL=C` 下判讀）：子程序無法啟動（找不到 `git` 或 `wsl.exe`）→ 503 `git_unavailable`；stderr 含
  `detected dubious ownership` → 502 `git_untrusted`；逾時 → 504 `git_timeout`；其他非零結束 → 502 `git_failed`。
- `blob` 的回應規則與 5a 原始內容端點相同（依副檔名給 content-type、HTML 依內容決定 charset、帶 `Content-Security-Policy:
  sandbox`），實作直接重用 `cockpit/src/files.rs` 的回應組裝函式；`render` 重用 `cockpit-files` 的 Markdown 渲染；`meta` 重用
  5a 的 `viewer` 分類。

### D7 單檔 diff 與左右並排對齊

- 解析 unified patch：`diff --git` 標頭、`old mode`／`new mode`、`rename from/to`、`Binary files … differ`、`Subproject commit`、
  `@@ -a,b +c,d @@` 與 `\ No newline at end of file`。
- 對齊（純函式）：在每個 hunk 內，連續的刪除區塊與緊接的新增區塊逐列配對，多出的一側對面補空白列；上下文列兩側同列。hunk 之間、
  第一個 hunk 之前與最後一個 hunk 之後若有未顯示的行，插入「省略 N 行」列（N 由 hunk 標頭與檔案總行數推得；檔案總行數未知時
  只在 hunk 之間插入）。
- 回應帶 `version`：patch 原始位元組的 64 位元雜湊（十六進位），前端輪詢時比對，相同就不重畫。
- 未追蹤檔案（`EMPTY`→`WORKTREE`）不經 git：伺服器以 5a 的相對路徑界限與有上限讀取讀檔（上限 8 MiB），前 8000 位元組含 NUL
  視為二進位（同 git 的判斷），否則每一行都是新增列。
- 特殊結果：二進位（`binary: true`）、只有權限改變（`mode_only: true`）、子模組（`submodule: true`）、兩側相同（`rows` 為空）
  都以欄位表示，前端顯示對應文字。

### D8 Graph 排版與分批載入

- 排版是純函式：輸入依 `--date-order` 排好的 commit 清單（每筆 hash 與 parents），輸出每一列的節點欄位、顏色編號，以及該列
  上半段與下半段的連線（起訖欄位與顏色）。演算法採「進行中的車道」：每條車道記住它在等哪個 hash；遇到 commit 時，等待它的第一條
  車道成為節點欄位，其他等待它的車道在此匯入並釋放；節點車道改等第一個 parent；其餘 parent 若已有車道在等就連過去，否則配置
  最左邊的空車道。顏色在車道開始時依序配置（6 色循環），沿同一條第一父鏈延續。
- **前綴穩定**：第 i 列的排版只取決於前 i 列，所以分批載入時伺服器對「前 N＋200 筆」重算後只回傳新的 200 列，與前面已顯示
  的列一致。為了讓重算看到同一份歷史，第一批回應附上 `tips`（當時解析出的 hash 清單），之後的批次一律帶 `tip` 參數，不再
  從 refs 解析——期間 agent 新增 commit 或移動分支都不會打亂已載入的列。上限 5000 筆。
- refs 變化的偵測：Graph 分頁為目前分頁時每 2 秒查 `refs`，與載入時的 tips 不同就顯示「分支已變更」與「重新載入」按鈕，不自動重載
  （避免捲動位置與選取被打斷）。
- 6 個車道顏色由十色 token 推導：`--accent`、`--ok`、`--warn`、`--bad`，以及 `color-mix(in oklab, var(--accent), var(--bad))`、
  `color-mix(in oklab, var(--ok), var(--warn))`；每列以一個內嵌 SVG 繪製（列高固定），`HEAD` 的節點以實心加外框標示。

### D9 前端結構

- 新檔 `/app/git.js`（`window.cockpitGit`），負責「變更」面板、diff 分頁、Graph 分頁、某版本分頁的內容與輪詢；`files.js` 保留
  分頁區框架（分頁列、切換、關閉、還原、鍵盤操作），把分頁一般化為帶 `kind` 的物件（`file`、`diff`、`graph`、`rev`），各 kind
  的內容建立、輪詢啟停、標題與 `title` 文字交給對應模組。這些 DOM 一律在 `#app` 之外，不受整頁重畫影響（沿用 5a D6）。
- 分頁身分：`file` 為 runtime＋`root_id`＋路徑；`diff` 為 runtime＋`root_id`＋`from`＋`to`＋路徑＋原路徑；`graph` 為
  runtime＋`root_id`（每個 repo 一個）；`rev` 為 runtime＋`root_id`＋版本＋路徑。
- 本機儲存格式升為 `v:2`：`tabs` 每筆帶 `kind`；讀到 `v:1` 時把每筆視為 `kind: "file"` 並照常還原，下次寫入即為 `v:2`；左欄值新增
  `changes`。
- 某版本分頁沿用 5a 檢視器：`viewers.js` 的 ctx 由呼叫端提供「原始內容網址」「渲染網址」與「相對連結如何開啟」，某版本分頁提供
  指向 `blob`／`render` 的網址，Markdown 內的相對連結開啟同一版本的某版本分頁。不輪詢（commit 的內容不會變；暫存區版本例外，
  以 2 秒輪詢 `meta` 的 `size` 與物件 hash）。
- 左欄「變更」面板在切到該分頁時與每次輪詢時讀 `status`，重畫只替換清單內容並保住捲動位置與焦點（memory：整頁重畫會丟掉只存在
  DOM 上的狀態）。

### D10 驗收用 fixture

- `ui_preview` 啟動時以本機 `git` 在暫存目錄建立真正的 repo（取代空 `.git/` 資料夾）：固定 `GIT_AUTHOR_DATE`／
  `GIT_COMMITTER_DATE`、`user.name`／`user.email`、`init.defaultBranch=main`、`commit.gpgsign=false`，使 hash 可重現。
  大量 commit（至少 260 筆，測分批載入）以一次 `git fast-import` 建立，不逐筆啟動 `git commit`。
- 內容至少涵蓋：兩條分支與一次 merge、附註 tag、一個假的遠端追蹤分支（`update-ref refs/remotes/origin/main`）、改名、刪除、
  二進位檔、中文檔名；工作區有已暫存修改、未暫存修改、未追蹤檔案、已刪除檔案。合併衝突放在 `other-repo`（進行中的 merge），
  前提是不改變既有驗收腳本依賴的 `other-repo` 行為；若會改變，另建第三個 repo 並加一個 pane。
- 找不到 `git` 時 `ui_preview` 啟動失敗並印出原因（fixture 是驗收的前提，不靜默降級）。

## Risks / Trade-offs

- [WSL distro 停止時第一次查詢要喚醒，超過 1 秒] → 逾時 10 秒足夠；前端顯示載入中，不當失敗。
- [`--untracked-files=all` 在未被 `.gitignore` 忽略的大型目錄（如 `node_modules`）輸出很大] → 4 MiB 上限＋`truncated` 標示，
  畫面顯示「變更過多，只列出前面一部分」。
- [clean filter 在 `git status` 時執行] → 見 D2「已知且接受」；ADR-0007 記錄。
- [每 2 秒一次 `wsl.exe` 啟動（約 70 ms）] → 只對顯示中的內容輪詢，同時最多 4 支子程序；Live Output 與檔案分頁的既有輪詢不受影響。
- [Graph 5000 筆上限] → 超過時顯示「已達上限」，建議以分支篩選縮小範圍。
- [diff 列很多（數萬行）時 DOM 很大] → 8 MiB patch 上限；超過顯示「差異過大，請在 VS Code 查看」。
- [左右並排在窄視窗每側只有一半寬] → 長行折行（不橫向捲動，避免兩側同步捲動的複雜度）。
- [`ui_preview` 與測試依賴本機安裝 git] → 開發機已安裝（Windows git 2.50.1）；`cockpit-git` 的整合測試在找不到 git 時失敗而不是略過，
  避免 gate 假綠。
- [git 的 porcelain 格式與旗標在不同版本的差異] → task 1.2 在兩個實際版本上實測所有旗標與輸出格式；解析器對未知的紀錄類型略過而非崩潰。

## Migration Plan

純新增功能，無資料遷移；本機儲存 `v:1` 自動沿用（D9）。回退＝還原本 change 的 commit，`v:2` 的儲存內容在舊版前端會被當成不認得的
版本而以空分頁開始（5a「儲存內容損毀」的既有行為）。
