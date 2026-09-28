# change file-review 前置探測（2026-09-26～27）

> 性質：propose 前的一手查證紀錄，供 `openspec/changes/file-review/` 引用。主機名與使用者名稱以 `<USER>`、`<HOST>` 代替。
> 全程對 HERDR 只送唯讀請求；未修改任何 git 設定。

## 1. pane `cwd` 的格式

| runtime | 實測值（去識別化） | 來源 |
|---|---|---|
| Windows 端 HERDR | `D:\projects\<repo>`（原生 Windows 路徑、反斜線） | 2026-09-27 以 `herdr api snapshot`（唯讀）實測 |
| WSL 端 HERDR | `/home/<USER>/<repo>`（POSIX 路徑） | 2026-09-13 spike 留下的 raw snapshot（未進版控）；本次探測時 WSL 端 HERDR 沒在跑，**未重新實測** |

- `cwd` 在 schema 中是 `Option<String>`，不帶「這是哪一種路徑」的型別資訊（`herdr-client/src/types/snapshot.rs`
  的 `PaneInfo`）。判斷要靠該 pane 所屬 runtime 的設定（`[[runtime]]` 有沒有 `wsl = { distro, ... }`）。
- pane 的 `cwd` 改變沒有任何事件，只能靠定期 snapshot 更新（設計文件 §2.3）。

## 2. Windows 程序讀 WSL 路徑

- `Test-Path '\\wsl.localhost\<distro>\home\<USER>\<repo>'` → `True`，`Get-ChildItem` 正常列出（含 `.git`）。
- `\\wsl$\<distro>\...` 寫法等效。
- 副作用：存取 `\\wsl.localhost\` 會讓 WSL VM 由 Stopped 轉為 Running（WSL 本身行為；不會啟動 HERDR）。
- 從 UNC 路徑往上找 `.git` 可行：`Test-Path '<root>\.git'` 回 `True`，且是資料夾。

## 3. Windows 端 git 對 WSL repo

- `git version 2.50.1.windows.1`。
- `git -C '\\wsl.localhost\<distro>\home\<USER>\<repo>' rev-parse --show-toplevel` → exit 128，
  `fatal: detected dubious ownership in repository`。
- 結論：**Windows 端不能直接對 WSL repo 跑 git 指令**（除非改 `safe.directory`，本次未改）。
  change file-review 的 `.gitignore` 過濾因此改用純 Rust 的 `ignore` crate；git 讀取層（diff、Git Graph）留給後續 change 另外設計。

## 4. 外部套件與資源（查證日 2026-09-26）

| 項目 | 版本 | 授權 | 重點 | 來源 |
|---|---|---|---|---|
| Material Icon Theme VSIX | 5.38.1 | MIT | `extension/icons/` 1251 個 SVG、約 3.3 MB；對照檔 `extension/dist/material-icons.json` 約 440 KB（`file`→`file.svg`、`folder`→`folder.svg`、`folderExpanded`→`folder-open.svg`）；repo 原始碼裡沒有 `folder.svg`／`file.svg`，是建置時產生 | <https://open-vsx.org/api/PKief/material-icon-theme/5.38.1/file/PKief.material-icon-theme-5.38.1.vsix> |
| pdfjs-dist | 6.3.289 | Apache-2.0 | `build/pdf.min.mjs` 約 459 KB、`build/pdf.worker.min.mjs` 約 1.27 MB；`cmaps/` 約 1.5 MB、`standard_fonts/` 約 0.8 MB（CJK 顯示需要 `cMapUrl`，官方參數＋社群佐證）；`isEvalSupported` 已移除（CVE-2024-4367 修正於 4.2.67，GHSA-wgrm-67xf-hhpq）；可用 `<script type="module">` 直接載入 | <https://registry.npmjs.org/pdfjs-dist/latest>、<https://github.com/mozilla/pdf.js> |
| comrak | 0.55.0 | BSD-2-Clause | `extension.table`／`tasklist`／`strikethrough`／`autolink`；`render.r#unsafe` 預設 `false`（原始 HTML 與危險連結被清掉） | <https://crates.io/crates/comrak> |
| ignore | 0.4.33 | Unlicense OR MIT | `WalkBuilder::max_depth(Some(1))` 只列一層；`require_git(false)` 才會在非 git 目錄套用 `.gitignore`（預設值官方文件未明寫，待實測）；`hidden(false)` 才會列出點開頭的檔案 | <https://docs.rs/ignore/latest/ignore/struct.WalkBuilder.html> |
| include_dir | 0.7.4 | MIT | 直接依賴 1 個（`include_dir_macros`） | <https://crates.io/crates/include_dir> |

- `vscode://file/{完整路徑}:{行}:{欄}` 為官方格式（<https://code.visualstudio.com/docs/configure/command-line>）。
  WSL 遠端官方只給 CLI 形式 `code --file-uri vscode-remote://wsl+<distro>/<path>`
  （<https://code.visualstudio.com/docs/remote/wsl>）；瀏覽器連結 `vscode://vscode-remote/wsl+<distro>/<path>` **未見於官方文件，待實測**。
- Chrome 內建 PDF viewer 在 sandbox iframe 內顯示空白（<https://github.com/whatwg/html/issues/3958>），因此 PDF 改用 pdf.js。
- `mhutchie/vscode-git-graph` 的 LICENSE 禁止發布、散布衍生作品：只能當功能與外觀參考，不取用程式碼。

## 5. 「在 VS Code 開啟」連結實測（2026-09-27，file-review task 1.2）

使用者請控制端代為實測。方法：以 Windows 的 `vscode:` protocol handler（登錄檔
`HKCR\vscode\shell\open\command` 為 `Code.exe --open-url -- "%1"`，與 Chrome 按下外部協定連結後交給系統的路徑相同）逐一開啟連結，
以 UI Automation 讀 VS Code 跳出的確認框文字，並對其中一個按「Yes」後截圖確認。沒有在 Chrome 內實際點擊（Chrome 另有自己的
「要開啟 Visual Studio Code 嗎？」確認，自動化工具無法操作）；Chrome 端留待 task 5.1 使用者目視驗收時點一次。

| 連結 | VS Code 確認框顯示要開啟的對象 | 結果 |
|---|---|---|
| `vscode://file/D:/projects/ai-cockpit/AGENTS.md` | `D:\projects\ai-cockpit\AGENTS.md`，「open this file or folder?」 | 路徑正確 |
| `vscode://file/C:/Users/<USER>/…/vscode%20probe/a%20b.md` | `C:\Users\<USER>\…\vscode probe\a b.md`，「file or folder」 | `%20` 正確解碼 |
| `vscode://vscode-remote/wsl+Ubuntu-24.04/home/<USER>/quant-dev/README.md` | `vscode-remote://wsl+ubuntu-24.04/home/<USER>/quant-dev/README.md`，「open this **folder**?」 | **被當成資料夾** |
| 同上加 `:1` 結尾 | `…/README.md:1`，「file or folder」 | 以檔案開啟 |
| `vscode://vscode-remote/wsl+Ubuntu-24.04/tmp/cockpit%20probe/a%20b.md:1` | `vscode-remote://wsl+ubuntu-24.04/tmp/cockpit probe/a b.md:1`，「file or folder」 | 按 Yes 後開出「WSL: Ubuntu-24.04」遠端視窗並顯示該檔內容（截圖確認） |

結論：

- Windows 檔案沿用官方格式 `vscode://file/<路徑>`，URL 編碼可用。
- **WSL 檔案的連結必須在結尾加 `:1`（行號）**，否則 VS Code 把遠端路徑當成資料夾開啟。未加行號時一律視為資料夾，是 VS Code
  URL handler 的行為（本節以確認框措辭實測佐證；原始碼位置未查證）。
- 每次點連結，VS Code 都會先跳出「An external application wants to open …」確認框（除非使用者自行勾選「Allow opening local
  paths without asking」）。這是 VS Code 的安全機制，Cockpit 不需處理。
- 依 task 1.2，spec「在 VS Code 開啟」的 WSL 格式改為結尾加 `:1`（見 change 的 spec delta）。

## 6. WSL 真機驗收（2026-09-28，file-review task 5.2）

性質：一次性真機驗收紀錄；環境事實與觀察值不是規格。design「Risks」第一條（WSL 經 `\wsl.localhost`（9P）讀檔時 Linux
符號連結可能由 WSL 端解開、Windows canonicalize 看不到它指向根外）的實測結論：**沒有洩漏**，所有指向 `/etc/passwd` 的連結在
`raw`／`meta`／`render` 三個端點都回 `403 path_outside_root`，本體不含 `/etc/passwd` 的任何內容。

### 環境

- 分支 `feat/file-review`，`HEAD` `03cc094`；`cargo build -p cockpit`（debug，`target/debug/cockpit.exe`）。
- WSL distro `Ubuntu-24.04`，HERDR `0.8.2`（protocol `20`）。開始前 distro 為 Stopped、`pgrep herdr` 無結果、使用者預設 socket
  `/home/<USER>/.config/herdr/herdr.sock` 不存在——沒有使用者自己的 HERDR 在跑。
- 測試全部放在一次性目錄 `/tmp/cockpit-5.2-20260928041712/`（跑完整個刪除），沒有碰使用者既有的任何 WSL repo：
  - `repo/`：`git init`＋`README.md`＋`docs/inner.txt`，以及下列符號連結（`ln -s`）：

    | 連結 | 指向 | 性質 |
    |---|---|---|
    | `passwd-link` | `/etc/passwd` | 根外（brief 指定） |
    | `rel-passwd-link` | `../../../etc/passwd` | 根外，相對路徑跳出 |
    | `etc-dir` | `/etc` | 根外目錄 |
    | `inner-link` | `docs/inner.txt` | 根內檔案（對照） |
    | `docs-link` | `docs` | 根內目錄（對照） |

  - headless 測試 server：沿用 `docs/research/2026-09-19/live-output-acceptance.md` round 3 的「專屬 socket」做法，再加一層隔離——
    `HOME` 也指向一次性目錄，讓測試 server 不讀、不寫使用者的 `~/.config/herdr`（`session.json` mtime 前後不變）：

    ```bash
    wsl.exe -d Ubuntu-24.04 -e bash -c 'setsid -f bash -c '\''echo $$ > "$0/herdr.pid"; exec env HOME="$0/home" \
      HERDR_SOCKET_PATH="$0/herdr.sock" /home/<USER>/.local/bin/herdr server >"$0/herdr-server.log" 2>&1 </dev/null'\'' \
      "$0" </dev/null >/dev/null 2>&1' /tmp/cockpit-5.2-20260928041712
    # → pid 23，/proc/23/comm = herdr，environ 內 HERDR_SOCKET_PATH=/tmp/cockpit-5.2-20260928041712/herdr.sock
    ```

  - 在測試 server 上建 pane（唯一的寫入操作，只對測試 server，設 `HERDR_CLIENT_TEST_ALLOW_WSL_WRITES=1`）：

    ```text
    → {"id":"wc","method":"workspace.create","params":{"cwd":"/tmp/cockpit-5.2-20260928041712/repo","label":"cockpit_5_2_probe","focus":false}}
    ← root_pane: {"pane_id":"w1:p1", ..., "cwd":"/tmp/cockpit-5.2-20260928041712/repo", ...}
    ```

- `cockpit` 用暫存設定檔（系統暫存目錄，跑完即刪；repo 根的 `cockpit.toml` 未讀未改），`127.0.0.1:7770` 啟動前確認無人 LISTEN：

  ```toml
  [server]
  listen = "127.0.0.1:7770"

  [[runtime]]
  id = "wsl"
  kind = "herdr"
  wsl = { distro = "Ubuntu-24.04", socket = "/tmp/cockpit-5.2-20260928041712/herdr.sock" }
  ```

  `/api/state` 顯示 runtime `wsl` 為 `connected`（`server_version` `0.8.2`），pane `w1:p1` 的 `cwd` 為測試 repo。

以下 curl 一律帶 `-H "Host: 127.0.0.1:7770"`；`$U` ＝ `http://127.0.0.1:7770/api/files/wsl/<root_id>`（`root_id` 為下方 (a) 回傳值）。

### (a) 根目錄查詢

```text
$ curl -i -H "Host: 127.0.0.1:7770" http://127.0.0.1:7770/api/runtimes/wsl/panes/w1%3Ap1/root
HTTP/1.1 200 OK
cache-control: no-store
x-content-type-options: nosniff
{"runtime":"wsl","pane_id":"w1:p1","root_id":"5c5c77736c2e…7265706f",
 "root_path":"\\wsl.localhost\Ubuntu-24.04\tmp\cockpit-5.2-20260928041712\repo",
 "cwd_path":"/tmp/cockpit-5.2-20260928041712/repo","name":"repo","is_git":true}
```

### (b) 列根目錄

```text
$ curl $U/list
{"entries":[{"name":"docs","kind":"dir",…},{"name":"docs-link","kind":"file",…},{"name":"etc-dir","kind":"file",…},
 {"name":"inner-link","kind":"file",…},{"name":"passwd-link","kind":"file",…},{"name":"README.md","kind":"file","icon":"readme.svg",…},
 {"name":"rel-passwd-link","kind":"file",…}],"omitted":0,"skipped":0}   [HTTP 200]
```

含 `README.md`、不含 `.git`。**所有符號連結都會列出，且一律 `kind: "file"`**（包括指向目錄的 `etc-dir`、`docs-link`），原因見 (e) 的機制說明。

### (c) 讀 README.md 與改寫後的變化

```text
$ curl $U/render/README.md
<h1 id="md-probe-52">Probe 5.2…</h1>
<p>line one</p>                                          [HTTP 200]

$ curl -D - $U/raw/README.md
HTTP/1.1 200 OK
content-type: text/plain; charset=utf-8
content-security-policy: sandbox
# Probe 5.2 … line one                                   [22 bytes]

$ curl $U/meta/README.md          # 改寫前
{"size":22,"modified_ms":1790540232376,"viewer":"markdown",…}

$ wsl.exe -d Ubuntu-24.04 -e bash -c 'echo "line two (appended by task 5.2)" >> /tmp/cockpit-5.2-20260928041712/repo/README.md'
（WSL 端 stat：size=54 mtime=1790540359.053）

$ curl $U/meta/README.md          # 改寫後 1 秒內，連續 3 次皆同
{"size":54,"modified_ms":1790540359053,"viewer":"markdown",…}
```

改寫後 `render`／`raw` 也立即反映新增的一行。`size`／`modified_ms` 經 9P 讀到的值與 WSL 端 `stat` 一致（毫秒精度），前端自動更新的
後端前提成立。本次沒有另開 headless Chrome 看前端的 3 秒內更新（非必要項，前端行為已由 task 4.6 與 `visual-check.js` 在替身上驗證）。

### (d) 「在 VS Code 開啟」連結

```text
"vscode_uri":"vscode://vscode-remote/wsl+Ubuntu-24.04/tmp/cockpit-5.2-20260928041712/repo/README.md:1"
```

符合 spec 的 WSL 格式（結尾 `:1`，見本文件第 5 節）。

### (e) 符號連結

```text
$ curl -D - $U/raw/passwd-link
HTTP/1.1 403 Forbidden
content-type: application/json
cache-control: no-store
x-content-type-options: nosniff
content-security-policy: sandbox
content-length: 66
{"error":"目標不在根目錄之內","code":"path_outside_root"}
```

| 請求 | 狀態 | `code` | 本體含 `root:x:0:0` |
|---|---|---|---|
| `raw/passwd-link` | 403 | `path_outside_root` | 否（66 bytes，只有錯誤 JSON） |
| `meta/passwd-link` | 403 | `path_outside_root` | 否 |
| `render/passwd-link` | 403 | `path_outside_root` | 否 |
| `raw/rel-passwd-link`、`render/rel-passwd-link` | 403 | `path_outside_root` | 否 |
| `raw/etc-dir/passwd`、`meta/etc-dir/passwd`、`list/etc-dir` | 403 | `path_outside_root` | 否 |
| `raw/inner-link`、`meta/inner-link`（根內對照） | 403 | `path_outside_root` | — |
| `list/docs-link`、`raw/docs-link/inner.txt`（根內對照） | 403 | `path_outside_root` | — |
| `raw/docs/inner.txt`、`list/docs`（不經連結的同一目標） | 200 | — | — |

另驗：`Host: evil.example:7770` 對 `raw/README.md` → `403 forbidden_source`。

**機制**（以 PowerShell 對同一批 UNC 路徑實測）：經 `\wsl.localhost` 看到的 Linux 符號連結是一個 reparse point
（`Attributes = ReparsePoint`，`LinkType`／`Target` 皆空，`[IO.File]::ResolveLinkTarget(…, $true)` 解不出目標），直接開檔
（`[IO.File]::ReadAllBytes`）失敗為「Could not find a part of the path」——**Windows 端根本無法跟隨這種連結，WSL 端也沒有替
Windows 把它解開**，design 擔心的「由 WSL 端直接解開、canonicalize 看不到根外」在 WSL 2／HERDR 0.8.2 這台機器上沒有發生。
於是 `fs::canonicalize` 失敗，落到 `cockpit-files/src/relpath.rs` 的 `classify_unresolved`：該段 `symlink_metadata` 看得到、
是 reparse point → 一律 `PathOutsideRoot`（控制端裁決 R11「一致拒絕優先於精準分類」）。也因此：

- **根內的符號連結同樣回 403、讀不到**（`inner-link`、`docs-link`）。這不是安全問題，而是 WSL repo 的功能限制：經 9P 的連結
  無法解析，只能一致拒絕。spec 沒有要求 WSL 根內連結可讀；若之後要支援，需要另外設計（例如改由 WSL 端解析），不在本 change 範圍。
- 列目錄時連結一律顯示為 `kind: "file"`（Windows 端無法得知它指向目錄），點開會得到 403。

### 收尾

```text
cockpit PID 62224：依 PID 停止 → Get-Process 查無此 PID；127.0.0.1:7770 LISTEN 數 0
測試 server：單一 wsl.exe 呼叫內核對 /proc/23/comm = herdr 且 environ 的 HERDR_SOCKET_PATH 為本次專屬路徑，通過後 kill -TERM 23 → TERMINATED
pgrep -a herdr → no herdr processes
rm -rf /tmp/cockpit-5.2-20260928041712 → DIR_REMOVED
/home/<USER>/.config/herdr/herdr.sock → DEFAULT_SOCK_ABSENT（與開始前相同）
/home/<USER>/.config/herdr/session.json mtime → Sep 26 08:45（未變）
```

全程沒有執行任何 `herdr server stop`；Windows 端 HERDR 未連線、未送任何請求；暫存設定檔已刪除。
