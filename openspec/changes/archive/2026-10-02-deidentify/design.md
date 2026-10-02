# design：deidentify

## Context

動機見 `proposal.md`「Why」。2026-10-02 唯讀盤點與規劃驗證（只計數、以代號記錄；U＝Windows 使用者名稱、W＝WSL 使用者名稱、H＝主機名稱、
E＝作者 email、R1–R3＝三個私人 repo 名稱）：

- **目前檔案**（1878 個追蹤檔）：U 5 檔；W 7 檔，主要是 `/home/W/` 路徑段（含 3 個 Rust 測試／範例、`herdr-client/README.md`、設計文件），
  另有 archive `2026-09-15-attach-herdr-runtimes/tasks.md` 的一行 grep 指令含 `W@` 與 `/home/W\b` 兩種形式；R1–R3 共 3 檔；H、E 0 檔。
  二進位檔位元組 0 命中。
- **截圖**：18 張追蹤中的 PNG 逐張看過，`docs/research/2026-10-01/ui-fixes-live-{connected,disconnected,reconnected}.png` 三張的右欄
  runtime 清單露出 R1–R3，其餘乾淨。
- **git 歷史**（可達：44 commit、2163 blob）：U 涉及 7 路徑／10 blob；W 以「前後不接英數字」比對共 24 處（`/home/W/` 18 blob、`/home/W\` 3、`W@` 3）；
  R1–R3 共 6 路徑；含已刪除的 `openspec/changes/attach-herdr-runtimes/tasks.md` 與 `docs/handover.md` 舊版。舊截圖 `progress-700.png`（含完整 U）
  與 `progress-1536.png`（含 U 前綴）的舊 blob 只能看圖判讀。commit 訊息 0 命中；作者與 committer 全為本名＋E。
- **不可達但仍在本機的物件**：reflog 撐住約 575 個 commit（含 H 與其他形式的 W），另有 dangling 物件；`.git/lost-found/` 有 9 個明文 blob。
  都不會被推送；filter-repo 結束時的 gc 會清掉 reflog 撐住的部分，lost-found 要手動刪。
- **ref 實況**：`refs/heads/` 2 個；`refs/codex/turn-diffs/checkpoints/*` 2 個**直接指向含 U、W 的舊 tree**；沒有 tag、stash、notes。
- 金鑰／token 樣式：目前與歷史 0 命中。
- **Git Bash 的 MSYS 路徑轉換會改寫以 `/` 開頭的參數**（`git grep -F "/home/W/"` 會漏掉），比對一律在 Node 內進行，不把敏感字串當成 shell 參數。
- **git-filter-repo 2.47.0 行為**（以 `--help`、原始碼與 repo 外合成 repo 實測）：
  - `--replace-text` 的 literal 比對分大小寫，`regex:` 前綴走 Python bytes regex（可用 `(?i)`）；所有 literal 先於 regex 套用；`#` 開頭的行不是註解；
    檔首 BOM 會讓第一條規則失效；前 8KB 含 NUL 的 blob（PNG）整個跳過，不會破壞圖片。
  - `--mailmap` 的單欄寫法 `New <new>` 只比對已是新 email 的 commit，等於無效；要用 `New <new> <舊 email>`，會同時改 author 與 committer。
  - `--blob-callback` 可依 `blob.original_id` 換 `blob.data`（拿不到檔名）。
  - 非 fresh clone 需 `--force`；`--force` 也跳過「多個 worktree」與「工作樹乾淨」檢查。
  - 結束時**自動**執行 `git reset --hard`、`git reflog expire --expire=now --all`、`git gc --prune=now`——舊物件立即消失，驗證失敗只能從備份還原。
  - 指向 tree 的 ref 只會警告 `Unexpected object of type tree, skipping`，原封不動、舊 tree 仍可達。
  - `.git/filter-repo/commit-map`：首行 `old new`，之後每行 40 碼舊→新；commit 訊息中 7–40 碼的舊編號會自動改寫（保持長度），檔案內容中的不會。
  - `--dry-run`／`--debug` 會在 `.git/filter-repo/` 留下含完整舊內容的 `fast-export.original`。

## Goals / Non-Goals

**Goals:**

- 推送後，從任何追蹤中的檔案、任何歷史版本、commit 作者／committer 欄位，都找不到 U、W、H、E、R1–R3。
- 改寫歷史前有可完整還原的備份，並先在副本上演練；改寫後 HEAD 的 tree 與改寫前相同。
- 留下以後推送前可重複執行的檢查。

**Non-Goals:**

- 不處理 repo 外的本機狀態（Claude、Codex 的本機紀錄檔等）：不在 repo 內、不會被推送。
- 不建立 remote、不推送。

## Decisions

### D1 替換對照

| 原字串 | 換成 | 規則 |
|---|---|---|
| U（含 `C:\Users\U`、JSON 跳脫形式、named pipe 名稱中的 U） | `<user>` | 不分大小寫；U 在歷史中只出現在路徑與指令中，單一規則即涵蓋各種跳脫形式 |
| W | `<user>` | 不分大小寫，且**前後都不接英數字或底線**（`(?<![A-Za-z0-9_])W(?![A-Za-z0-9_])`）；W 只有 3 個字元，這條界線擋掉一般單字中的子字串，已實測可達歷史中命中恰為 24 處敏感出現、0 誤命中 |
| R1、R2、R3 | `repo-a`、`repo-b`、`repo-c` | 不分大小寫，字串先跳脫 |

不列入（使用者 2026-10-02 於 A 段審查後決定保留）：WSL 日常工作 repo 名稱 `quant-dev`（git review 真機探測紀錄）、`shioaji`
（早期驗收紀錄的 workstream id、設計概念圖的範例專案標題；公開 SDK 名稱）。

目前檔案（A 段）與歷史（B 段）用同一組規則。替換後人工讀過每個改到的段落，確認語意仍通（例如「`repo-a` 的 pane」）。Rust 程式見 D2：
A 段在 HEAD 上另行改寫；歷史中的舊版程式由 B 段規則替換即可（只影響歷史，不需可編譯）。

### D2 Rust 測試與範例的 WSL socket 預設值

- `cockpit/tests/config.rs` 的兩處是純解析測試的輸入字串，改為 `/home/user/.config/herdr/herdr.sock`，斷言同步改，測試語意不變。
- `herdr-client/tests/real_herdr.rs` 的 `wsl_socket()` 與 `herdr-client/examples/spike4_no_window.rs` 的預設值是給真機測試用的：環境變數
  `HERDR_CLIENT_TEST_WSL_SOCKET` 沒給時，改為執行 `wsl.exe -d <distro> --exec printenv HOME`（distro 沿用既有 `HERDR_CLIENT_TEST_WSL_DISTRO`
  的取法），以 `<HOME>/.config/herdr/herdr.sock` 為預設；取不到時以明確訊息失敗。用 `--exec` 不經 shell（見 memory「wsl.exe 的 `--` 會經 shell」）。
  文件註解與 `herdr-client/README.md` 的預設值欄改寫成「由 WSL 的 `$HOME` 推得」。
- 選這個而非「改成必填」：保留使用者在本機直接跑 opt-in 真機測試的便利，又不把個人路徑寫進 repo。

### D3 三張真機截圖遮罩

以 headless Chrome 載入 PNG 到 canvas，在 R1–R3 出現的區塊（runtime 卡的 workspace 標題與 cwd 列）以面板底色 `--bg-base`（`#101a2a`）
填滿矩形後輸出覆蓋原檔；矩形座標由看圖決定、寫在腳本內。不加新依賴。遮罩後逐張看圖確認，並在 `ui-fixes-live.md` 註明「截圖已遮罩私人 repo 名稱」。

### D4 防再犯檢查腳本

`docs/research/2026-10-02/deid-check.js`（附 `deid-check.md`），兩種模式共用同一套比對：

- **詞表**：U＝`os.userInfo().username`、H＝`os.hostname()`、W＝`wsl.exe -d <distro> --exec whoami`（WSL 不可用時略過並提示）、
  E＝`git config --global user.email`（另讀 repo 層級設定；改寫後 repo 層級會是 noreply，所以一定要讀 global），以及 repo 根目錄的本機詞表
  `.deid-terms`（每行一詞，已加入 `.gitignore`；放 R1–R3、以及改寫前的 E 以防 global 設定日後變更）。noreply、`anthropic.com`、`example`／`.invalid`
  網域的 email 不列入。
- **比對**：每個物件以位元組讀入，對 UTF-8 與 UTF-16LE 兩種編碼不分大小寫比對；W 用 D1 的「前後不接英數字」規則，其他詞做子字串比對。
- **檔案模式**（預設；`--rev <commit>` 可指定 commit，預設 HEAD）：以 `git ls-tree -r` ＋ `git cat-file --batch` 讀該 commit 的所有檔案，不需 checkout。
  A 段驗收用這個模式。
- **歷史模式**（`--history`）：`git cat-file --batch-all-objects --batch` 掃全部物件（blob、commit、tag，含不可達），commit 物件的作者／committer
  與訊息一併比對；另以 `git for-each-ref` 列出 ref，非 `refs/heads/*` 的 ref 一律列為警告。B 段驗證用這個模式。
- **輸出**只列「物件或檔案路徑：類別代號：次數」，**絕不印出詞本身**；有命中時 exit 1。
- PNG 只能比對位元組；畫面上的文字要另外逐張看圖，`deid-check.md` 寫明這一點與「真機截圖不進 repo」的規則。

### D5 改寫歷史的步驟（不可逆）

前提：A 段（D1–D4）已在 feature 分支完成、審查通過、併回 `main`；`deid-check.js`（檔案模式）對 `main` 為 0 命中；`git status --porcelain` 沒有追蹤中的改動；
`git worktree list` 只有本 repo。

1. **備份（兩份）**：
   - 把整個 repo 目錄連同 `.git` 複製到 repo 外（排除可重建的 `target/`），還原時整份拷回——保留 reflog、repo 層級設定、`info/exclude`、hooks、
     `lost-found` 與未追蹤檔（如 `cockpit.toml`、`.deid-terms`）。
   - `git clone --mirror --no-local` 到 repo 外作為第二份獨立備份（`--no-local` 不與原 repo 以 hardlink 共用物件），在其中 `git fsck --full`，比對
     `git for-each-ref` 數量與原 repo 相同。
   - 記下原 `main` 的 commit 編號與 `main^{tree}` 編號。
2. **演練**：`git clone --no-local` 一份到 repo 外，在副本上執行第 3–4 步（fresh clone 不需 `--force`），以第 5 步的驗證全過為準；不過就修正替換表或
   mailmap 再演練，不碰原 repo。
3. **刪除 Codex 參照**（原 repo）：`git update-ref -d` 逐一刪除 `refs/codex/turn-diffs/checkpoints/*`——filter-repo 不處理指向 tree 的 ref，不刪就會留下可達的舊 tree。
4. **filter-repo**（原 repo，`--force`；不得用 `--dry-run`／`--debug`）一次完成：
   - `--replace-text <repo 外的替換表>`：D1 的三類規則，寫成 `regex:(?i)…==>…`；檔案為無 BOM 的 UTF-8、不放 `#` 註解行。
   - `--mailmap <repo 外的 mailmap>`：一行 `Benjamin-Teng <68319994+Benjamin-Teng@users.noreply.github.com> <E>`（只以舊 email 比對）。
   - `--blob-callback`：依盤點得到的舊 blob 編號，把含 U 的 `progress-700.png`、`progress-1536.png` 舊版，以及 `ui-fixes-live-*.png` 的所有歷史版本，
     換成 A 段後乾淨版本的位元組。
   - 結束時 filter-repo 自動 reset、清 reflog、gc。替換表與 mailmap 保留到 2.6 通過後才刪。
5. **驗證**：
   - `deid-check.js --history` 0 命中（全部物件含 commit 作者、committer 與訊息）；`git for-each-ref` 只剩 `refs/heads/*`。
   - 新 `main^{tree}` 與改寫前記下的編號相同（A 段已清乾淨，歷史改寫不應改變 HEAD 的內容）。
   - commit 數與改寫前相同；`git log --format='%an <%ae>|%cn <%ce>'` 只剩新身分一種。
   - 全 gate 與既有腳本在新 HEAD 全綠。
6. **收尾**：刪除 `.git/lost-found/` 與 `.git/filter-repo/` 以外的殘留暫存；確認 `git count-objects -v` 的 loose 物件與 unreachable 物件為 0；repo 層級
   `git config user.name "Benjamin-Teng"`、`git config user.email "68319994+Benjamin-Teng@users.noreply.github.com"`；把 `.git/filter-repo/commit-map`
   存成 `docs/research/2026-10-02/commit-map.txt` 後 commit，再跑一次 `deid-check.js --history`。
7. **還原方式**（若第 5 步失敗）：刪掉原 repo 目錄，把第 1 步的整份目錄備份拷回原位置；mirror 備份留作第二道保險。

### D6 審查

使用者 2026-10-02 指示：本 change 的 Codex 審查由 Opus 5.5 取代。A 段（D1–D4 的 diff）審一次；B 段不是 diff，改由 D5 第 5 步的全歷史實測把關，
並請 Opus 獨立重跑 `deid-check.js --history`、作者欄位與 ref 檢查。

## Risks / Trade-offs

- [改寫後 commit 編號全部改變，文件中引用的舊編號失效] → D5 第 6 步保存對照表；限制：對照表只收可達 commit，文件中引用的、已被 squash 掉的
  feature 分支短編號查不到——寫進交接手冊。
- [替換破壞程式語意] → A 段跑全 gate 與既有腳本；HEAD 上的 Rust 只動 D2 列的三個檔；歷史中的舊程式不需可編譯。
- [截圖遮罩不完整] → 逐張看圖；D4 腳本無法檢查畫面文字，這點寫進用法說明。
- [W 的規則誤換或漏換] → 規則已在可達歷史實測；D5 第 2 步先在副本演練，`deid-check.js --history` 用同一規則複驗。
- [filter-repo 自動清掉舊物件，驗證失敗無法就地回復] → 先演練（D5 第 2 步）；原 repo 失敗時以整份目錄備份還原（D5 第 7 步）。
- [本機殘留含舊資料] → 整份目錄備份與 mirror 備份都保有完整舊資料，備份的保存與刪除由使用者決定。

## Migration Plan

見 D5。沒有 remote，改寫不影響其他人。

## Open Questions

（無）
