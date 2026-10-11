# git-review（delta）

## MODIFIED Requirements

### Requirement: git 讀取的安全邊界

系統必須只以「讀取」的方式使用 git：只執行 status、列 refs、列 commit、讀 commit 內容、比較兩個版本、找共同祖先、讀取某版本
的檔案內容、取得工作樹目前所在的分支名稱這幾類查詢，且能執行的查詢種類在程式中是封閉清單，HTTP 請求無法讓服務執行清單以外的
git 子命令或加入任意引數。「目前分支」查詢只供 Cockpit 內部的 OpenSpec 進度偵測使用（見 `openspec-stage-sync`），不對應任何
HTTP 端點。「目前分支」查詢在 HEAD 指向某個分支時回傳該分支名稱（含 `/`，例如 `feat/foo`）；HEAD 是 detached（沒有指向任何分支）時
回傳「沒有分支」，這不是錯誤；HEAD 指向的不是 `refs/heads/` 底下的 ref 時同樣回傳「沒有分支」；分支名稱以 HEAD 的完整 ref 去掉
`refs/heads/` 前綴取得，不使用縮寫形式（存在同名 tag 時縮寫形式可能帶歧義）；其他失敗（不是 repo、git 無法執行、逾時、擁有者不符等）為錯誤，不回傳分支名稱。
每次執行 git 都必須：不取得 optional lock、不寫入 index 或 repo 內任何檔案；不執行 repo 設定所指定的 fsmonitor、外部 diff 程式、
textconv 與簽章驗證程式；把路徑一律當字面路徑（不解讀 pathspec 語法）；不修改任何 git 設定（含 `safe.directory`）。
根目錄位於 WSL（主機路徑以 `\\wsl.localhost\<distro>\` 或 `\\wsl$\<distro>\` 開頭）時，git 必須在該 distro 內以
`wsl.exe --exec` 執行（不經 shell，引數不被重新解讀），並以 git 自己的旗標指定 repo 目錄；其他根目錄以 Windows 的 git 執行。
每次執行 git 時，不得受 Cockpit 自身行程所繼承、會改變 git 對 repo 位置判定的環境變數影響（含 `GIT_DIR`、`GIT_WORK_TREE`、
`GIT_INDEX_FILE` 等），一律查詢目標 project 的 repo；本機的 git 與 WSL 內的 git 都以固定語系（`LC_ALL=C`）執行，使錯誤判定
（例如擁有者不符）不依賴使用者的語系設定。
git 拒絕讀取「擁有者不是目前使用者」的 repo 時，系統必須回報錯誤而不得繞過該檢查。同一時間執行中的 git 子程序不超過 4 個；
單次查詢（含排隊）超過 10 秒即終止子程序。

#### Scenario: repo 設定的外部程式不被執行

- **GIVEN** 一個 git repo 的 `.git/config` 設定 `core.fsmonitor`、`diff.external`、一個 textconv 驅動（並以 `.gitattributes`
  套用到 `*.txt`）、`log.showSignature = true` 與 `gpg.program`，每個設定都指向一支執行時會建立標記檔的腳本；repo 有已修改的
  `a.txt`
- **WHEN** 依序請求該 repo 的狀態、refs、commit 清單、commit 詳情、變更檔案清單、`a.txt` 的 diff 與 `a.txt` 某版本的原始內容
- **THEN** 全部回 200，標記檔都不存在

#### Scenario: 讀取狀態不寫入 index

- **GIVEN** 一個 git repo，某個已追蹤檔案的修改時間被更新但內容不變（一般的 `git status` 會因此改寫 index）
- **WHEN** 請求狀態
- **THEN** 回 200，`.git/index` 的內容與修改時間都不變，repo 內沒有出現 `index.lock`

#### Scenario: 工作區側的 diff 不改寫 index

- **GIVEN** 一個 git repo，已追蹤檔案 `stale.txt` 的修改時間被更新但內容不變，另一個已追蹤檔案 `real.txt` 內容真的被修改
- **WHEN** 請求「暫存區→工作區」的變更檔案清單、`stale.txt` 與 `real.txt` 各自的 diff，以及以 HEAD 為左側、工作區為右側的兩者 diff
- **THEN** 全部回 200；變更檔案清單只有 `real.txt`、沒有 `stale.txt`；`stale.txt` 的 diff 沒有任何列；`real.txt` 的 diff 有修改列；
  `.git/index` 的內容與修改時間都不變，repo 內沒有出現 `index.lock`

#### Scenario: WSL repo 在 WSL 內執行且引數不經 shell

- **GIVEN** WSL runtime 的 pane `cwd` 位於 WSL repo 內，repo 有一個名為 `$(touch pwned).md` 的未追蹤檔案
- **WHEN** 請求狀態，再請求該檔案的 diff
- **THEN** 兩者回 200，狀態中的路徑原樣為 `$(touch pwned).md`；WSL 內沒有出現 `pwned` 檔案；Windows 端的 git 設定沒有任何改變

#### Scenario: 繼承的 repo 定位環境變數不影響查詢

- **GIVEN** Cockpit 以帶 `GIT_DIR=<另一個 repo 的 .git>`（另可帶 `GIT_WORK_TREE`、`GIT_INDEX_FILE`）的環境啟動；目標
  project 的 repo 在 `main` 分支、有未暫存修改的 `b.rs`，另一個 repo 在 `other` 分支且沒有任何變更
- **WHEN** 請求目標 repo 的狀態與 refs
- **THEN** 回 200；`branch.head` 為 `main`、`entries` 含 `b.rs`、refs 為目標 repo 的 refs，與 `GIT_DIR` 指向的 repo 無關；
  兩個 repo 的 `.git/index` 內容與修改時間都不變

#### Scenario: 擁有者不符的判定不受使用者語系影響

- **GIVEN** 使用者語系為非英文（例如 `LANG=zh_TW.UTF-8`）；以 Windows 的 git 或 WSL 內的 git 讀取時會回報擁有者不符的 repo
- **WHEN** 請求狀態
- **THEN** 回 502，`code` 為 `git_untrusted`（不是 `git_failed`）

#### Scenario: 擁有者不符不繞過

- **GIVEN** 以 Windows 的 git 讀取時會回報 dubious ownership 的 repo
- **WHEN** 請求狀態
- **THEN** 回 502，`code` 為 `git_untrusted`；git 的全域與系統設定沒有被修改

#### Scenario: 目前分支查詢回傳分支名稱

- **GIVEN** 一個 git repo，HEAD 指向分支 `feat/openspec-stage-sync`
- **WHEN** 執行「目前分支」查詢
- **THEN** 回傳分支名稱 `feat/openspec-stage-sync`（完整名稱，不截斷）

#### Scenario: detached HEAD 沒有分支但不是錯誤

- **GIVEN** 一個 git repo，HEAD 指向某個 commit 而不是分支（detached HEAD）
- **WHEN** 執行「目前分支」查詢
- **THEN** 回傳「沒有分支」，不是錯誤

#### Scenario: 目前分支查詢失敗為錯誤

- **GIVEN** 目標目錄不是 git repo（或 git 回報擁有者不符）
- **WHEN** 執行「目前分支」查詢
- **THEN** 回傳錯誤，不回傳分支名稱，且沒有修改任何 git 設定

#### Scenario: 目前分支查詢遵守讀取邊界

- **GIVEN** 一個 git repo，某個已追蹤檔案的修改時間被更新但內容不變；repo 位於 WSL 發行版內
- **WHEN** 執行「目前分支」查詢
- **THEN** 查詢在該 distro 內以 `wsl.exe --exec` 執行；`.git/index` 的內容與修改時間都不變，repo 內沒有出現 `index.lock`

#### Scenario: 與 tag 同名的分支

- **GIVEN** 一個 git repo，HEAD 指向分支 `feat/foo`，且存在一個同名的 tag `feat/foo`
- **WHEN** 執行「目前分支」查詢
- **THEN** 回傳 `feat/foo`（不是 `heads/feat/foo`）

#### Scenario: HEAD 指向非 refs/heads 的 ref

- **GIVEN** 一個 git repo，HEAD 是指向 `refs/remotes/origin/main` 的符號參照（非 `refs/heads/` 底下）
- **WHEN** 執行「目前分支」查詢
- **THEN** 回傳「沒有分支」，不是錯誤
