# deid-check：推送前的去識別化檢查

## 目的

這個 repo 推到 remote 之前，要確認追蹤中的檔案與 git 歷史裡沒有個人識別字串（使用者名稱、主機名稱、作者 email、私人 repo 名稱）。
`deid-check.js` 在執行時取得這些字串，掃描 git 物件的位元組，只回報「哪個檔案或物件、哪個類別、幾次」，**絕不印出字串本身**。
腳本與本文件也不含任何實際值，所以本身不會變成洩漏來源。

**每次推送前都要跑**（檔案模式至少一次；改寫過歷史或第一次推送前再跑歷史模式）。

## 用法

```bash
node docs/research/2026-10-02/deid-check.js [--rev <commit>] [--history] [--distro <name>]
```

| 選項 | 說明 |
|---|---|
| （無） | 檔案模式，檢查 `HEAD` 的所有追蹤檔案，不需 checkout |
| `--rev <commit>` | 檔案模式，改檢查指定 commit |
| `--history` | 歷史模式，掃描全部 git 物件，並檢查 ref；不可與 `--rev` 並用 |
| `--distro <name>` | 取得 W 用的 WSL distro，預設 `Ubuntu-24.04`，亦可用環境變數 `DEID_WSL_DISTRO` |

只需要 Node（內建模組，無額外依賴）與 `git`。可從 repo 內任一目錄執行。

exit code：`0` 無命中；`1` 有命中（歷史模式下，`refs/heads/`、`refs/remotes/`、`refs/tags/` 以外的 ref 也算）；`2` 用法或執行錯誤。

歷史模式的 ref 規則：`refs/heads/`、`refs/remotes/`、`refs/tags/` 是正常的（第一次推送後一定會有 remote 與 tag），不算命中；
其他命名空間（例如 `refs/codex/`、`refs/stash`、`refs/original/`）可能讓舊物件保持可達，算命中。
另外，**所有 ref 的名稱**（含 heads、remotes、tags）都會用同一套詞表比對，名稱含詞也算命中（lightweight tag 與分支名不存在於任何物件內，物件掃描看不到）。
任何 ref 可達的物件，以及不可達的物件，本來就都會被掃描，所以這項檢查只負責擋「不該存在或不該推送的 ref」。

### 輸出怎麼讀

- 檔案模式：每個有命中的檔案一行，`路徑<Tab>類別:次數`。
- 歷史模式：每個有命中的物件一行，`物件編號<Tab>類型<Tab>可達或不可達<Tab>首見路徑<Tab>類別:次數`；
  不可達物件沒有路徑（顯示 `-`）。類型有 `blob`、`tree`（只比對檔名）、`commit`（作者、committer 與訊息）、`tag`。
- 最後有摘要：各類別的不同檔案／物件數與出現次數；摘要前會列出「已載入類別」，
  **確認它包含你預期的全部類別**，否則某類詞沒取到會造成假的 0 命中。
- 路徑或 ref 名稱本身若含詞，會顯示為 `<名稱含詞，已隱藏>`。

## 類別代號

| 代號 | 來源 | 比對規則 |
|---|---|---|
| U | `os.userInfo().username`（Windows 使用者名稱） | 子字串 |
| H | `os.hostname()`（主機名稱），加上環境變數 `COMPUTERNAME`（NetBIOS 名稱，最長 15 字，常是完整名稱被截斷）；兩者不同才都納入，不分大小寫去重，若其中一個是另一個的子字串則只留較短者 | 子字串 |
| W | `wsl.exe -d <distro> --exec whoami`（WSL 使用者名稱）；WSL 不可用時略過並警告 | 前後都不是英數字或底線（W 很短，避免誤中一般單字） |
| E | `git config --global user.email` 與 repo 層級的 `git config user.email` | 子字串；`noreply`、`anthropic.com`、`example`、`.invalid` 網域的 email 不列入 |
| T1…Tn | repo 根目錄 `.deid-terms` 的每一行，依檔案順序編號 | 子字串 |

所有類別都不分大小寫，每個物件以位元組同時比對 UTF-8 與 UTF-16LE 兩種編碼。

E 一定要讀 global 設定：歷史改寫後 repo 層級的 email 會改成 noreply，不會再被當成要找的詞，所以改寫前的 email 也要放進 `.deid-terms`，
以免 global 設定日後變更就漏掉。

## 建立 `.deid-terms`

`.deid-terms` 是本機詞表，已列入 `.gitignore`，**不進 git**。格式：

- 每行一個詞；空行與 `#` 開頭的行忽略。編碼可為 UTF-8（有無 BOM 皆可）、UTF-16LE（BOM `FF FE`）或 UTF-16BE（BOM `FE FF`）；
  沒有 BOM 但含 NUL 位元組時視為 UTF-16LE（Windows PowerShell 5.1 的 `>`／`Out-File` 預設輸出）。
  解碼後若仍有 NUL 或 U+FFFD 的詞（例如存成 Big5），腳本會以 exit 2 中止，不會靜默載入亂碼詞。
- 放腳本無法自動取得的詞：私人 repo 名稱，以及改寫前的作者 email。
- 不要用 shell 的 `echo` 把含 `/` 開頭的詞寫進檔案或當成命令列參數（Git Bash 的 MSYS 路徑轉換會改寫它）；用編輯器或 Node 寫入。

換新機器或重新 clone 後，`.deid-terms` 不會跟著來，要重建；缺檔時腳本會警告「T 類別未檢查」。

## 限制

- **PNG 只比對位元組，畫面上的文字不會被發現**。每張新增或修改的截圖都要逐張看圖，確認沒有使用者名稱、路徑、私人 repo 名稱。
- **真機截圖不進 repo**（專案規則）：需要留證的真機畫面，先遮罩後逐張確認再追蹤。
- 只涵蓋這個 repo 的 git 物件；repo 外的本機狀態（工具的對話紀錄、備份目錄）不在範圍內。
- 歷史模式的 tree 只比對檔名，不比對二進位雜湊。
- 比對的是文字內容；經過編碼（base64、壓縮檔內部）的字串看不到。
- 大小寫不分的規則涵蓋單一字元的大小寫對應，不處理 Unicode 正規化差異。
