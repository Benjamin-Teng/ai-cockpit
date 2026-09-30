# ADR-0007 新增 `cockpit-git` crate，把「只讀 git」的安全邊界封在編譯期

- Status: Accepted
- Date: 2026-09-28

## Context

`git-review` change 要讓 Cockpit 讀取 agent 操作中的 git repo（狀態、refs、commit 清單、
diff、Git Graph），但**只能讀，不能改**——不能放大權限（讓伺服器以更高權限跑 git）、
不能被請求引數操縱成執行清單以外的 git 子命令、也不能干擾 agent 同時在跑的 `git commit`
（不可搶 `index.lock`）。這條邊界如果只靠 `cockpit` 裡的 handler 各自小心組 `Command::new
("git")` 的引數，安全性會散落在每個 handler 裡，review 只能一個個看；ADR-0006 已經示範過
「把安全邊界會用到的邏輯放進獨立的純邏輯 crate」這個模式（`cockpit-files`），本 change 比照
辦理。

file-review 的「伺服器不執行任何程式」原本是全域規則；本 change 把它改為**「只執行封閉清單
內的 git 唯讀查詢」**——例外只開給 design D4 列出的 10 種查詢，其餘所有子命令與引數組合
在編譯期就不存在對應的 Rust 型別可以送進執行器。

## Decision

```text
cockpit  →  cockpit-git
cockpit  →  cockpit-files
cockpit  →  cockpit-herdr  →  herdr-client
                           →  cockpit-core
cockpit  →  cockpit-core
```

- `cockpit-git`：純邏輯 crate，只依賴 `tokio`（`process`、`io-util`、`time`、`sync`）與
  `serde`。提供：查詢型別與其 argv 組裝（`Status`、`Refs`、`Log`、`CommitInfo`、
  `ChangedFiles`、`FileDiff`、`MergeBase`、`BlobSize`、`Blob`、`VerifyCommit`）、執行目標
  的選擇（`GitTarget`、`select_target`）、版本與路徑的值型別（`Oid`、`Side`、`RepoPath`、
  `RefName`）。不知道 HTTP、HERDR、投影與 runtime 設定。
- **查詢種類封閉**：`GitQuery` 是 sealed trait（`private::Sealed`，寫法照
  `herdr-client/src/client/request.rs`），crate 外無法實作，所以 HTTP 層永遠只能呼叫這
  10 個型別已經決定好的子命令與旗標，送不進任意引數。
- `cockpit-git` **不依賴** `cockpit-files`、`cockpit-core`、`cockpit-herdr`、
  `herdr-client`；它們也都不依賴 `cockpit-git`。`RepoPath` 的片段規則與 `cockpit-files`
  的 `RelPath`（file-review design D3）刻意重複——這是縱深防禦：`cockpit`（HTTP 層）已經
  用 5a 的 `RelPath` 驗過一次相對路徑，`cockpit-git` 在把路徑放進 git 的 argv 之前再驗一次，
  即使其中一層有漏洞，另一層仍擋得住。
- `cockpit`：唯一依賴 `cockpit-git` 的 crate，負責 HTTP 路由、`authorize_root`、把
  `Root`（`cockpit-files`）的主機路徑轉成 `GitTarget`、執行查詢並把結果對應成回應。

### D2 執行方式的理由與替代方案

- **採用**：依根目錄主機路徑選擇 `git -C <path> ...`（Windows 本機 git）或
  `wsl.exe -d <distro> --exec env LC_ALL=C git -C <posix> ...`（WSL 內的 git）。一律
  `--exec`，不經 shell，不用 `wsl.exe --cd`。task 1.2 的兩個實測陷阱（見
  `docs/research/2026-09-28/git-review-probe.md`）：
  1. `wsl.exe -d <distro> -- <cmd> <args>`（用 `--` 而非 `--exec`）會經 Linux 預設 shell
     重新解析引數，`'$HOME'` 這類字串會被展開——這正是要避免的「引數被重新解讀」風險，
     所以一律用 `--exec`。
  2. `wsl.exe --cd <不存在的路徑>` 會**靜默**退回 `/` 且 exit 0，不會報錯；改用
     `git -C <posix>` 本身在目錄不存在時會清楚報錯（exit 128，`fatal: cannot change to
     '...'`），不會被 `wsl.exe` 自己的任何隱藏退回邏輯蓋掉。
- **替代方案一：gitoxide（`gix`）**——不需要子程序，但讀 WSL repo一樣要經
  `\\wsl.localhost` 的 9P 檔案系統（慢，task 1.2 實測 wsl.exe 直接在 WSL 內執行反而更快：
  66–122 ms vs 經 `\\wsl.localhost` 讀取的 100–210 ms）；diff 與 rename 偵測要自己組，
  依賴樹也大。不採用。
- **替代方案二：Windows git 一律加 `-c safe.directory=<路徑>`**——task 1.2 實測可行且只
  需要一條路徑，但這等於讓 WSL 內任何 repo 的 `.git/config` 設定能以 Windows 使用者身分
  執行程式（跨信任邊界的權限放大：本來只有 repo 擁有者本人（WSL 內的使用者）能觸發這些
  設定驅動的外部程式，`safe.directory` 會讓 Windows 使用者也能觸發）。design 選擇
  **不繞過** dubious ownership 檢查，讀不到就回報 `git_untrusted`，不是本 change 的
  bug，是刻意的安全邊界。
- **替代方案三：直接在 `cockpit` 裡寫 `Command::new("git")`**——沒有編譯期封閉，安全邊界
  （固定前綴、`--end-of-options`、`--literal-pathspecs` 等）散在每個 handler 裡，review
  只能逐一檢查有沒有漏加；改用 sealed trait 後，漏加只可能發生在 `cockpit-git` 內的 10 個
  查詢型別，`cockpit` 完全無法繞過。不採用。

## Consequences

- `cockpit-git` 的 argv 組裝是純函式，單元測試不需要真的裝 git 或起子程序，就能斷言
  「這個查詢在這個目標下產生的完整 argv 就是這樣」；task 2.2（執行器）與 2.3（解析器）
  建在這些型別上，不用重新設計安全邊界。
- 多一份 `Cargo.toml` 與樣板；`cargo tree -p cockpit-core` 等指令可直接驗證 `cockpit-git`
  沒有被拉進不該依賴它的 crate。
- **已知且接受的殘留風險**（design Risks）：repo 若設定 clean filter（例如 git-lfs），
  `git status`／工作區 diff 仍會執行它；但這是 repo 擁有者本人（WSL 內的使用者、或
  Windows 使用者本人）在該 pane 下等同自己執行 `git status` 的正常後果，不是 Cockpit
  放大權限，且 fsmonitor、外部 diff、textconv、簽章驗證這些「可由 repo 設定觸發、又不影響
  讀取正確性」的外部程式已經由固定前綴逐一關掉。
