# file-review Specification

## Purpose

定義 Cockpit 的檔案瀏覽與 Review：由選定 pane 推算檔案根目錄、以唯讀端點在安全邊界內列目錄與讀檔、左欄檔案樹、
中欄下半部的檔案分頁與 md／pdf／html／純文字檢視器、自動更新、分頁還原、「在 VS Code 開啟」與檔案 icon。
證據：`docs/research/2026-09-27/file-review-probe.md`、設計文件 §2.5（pane `cwd`）。

## Requirements

### Requirement: 檔案根目錄與允許清單

系統必須由 pane 的 `cwd` 推算「檔案根目錄」：先把 `cwd` 轉成服務所在主機可讀的路徑——runtime 設定為 WSL 時，POSIX 路徑
`/a/b` 轉為 `\\wsl.localhost\<distro>\a\b`（`<distro>` 取自該 runtime 的設定），其他 runtime 原樣使用——再從該路徑
（含）逐層往上找名為 `.git` 的資料夾或檔案，找到時以其所在目錄為根目錄（`is_git` 為真），到檔案系統頂端都找不到時以
`cwd` 本身為根目錄（`is_git` 為假）。`cwd` 為 null、或轉換後的路徑不存在時，該 pane 沒有根目錄。

「允許清單」是處理每個檔案請求當下，由最新投影中所有 runtime 的所有 pane（含 exited）各自推算出的根目錄集合
（以 runtime 區分）。檔案端點只能讀取允許清單內的根目錄；不在清單內的根目錄一律視為不可用，即使先前曾經可用。

#### Scenario: 往上找到 git repo 根目錄

- **GIVEN** runtime `win` 的 pane `w1:p1` 的 `cwd` 為 `D:\repo\src\deep`，`D:\repo\.git` 是資料夾
- **WHEN** 推算 `w1:p1` 的根目錄
- **THEN** 根目錄為 `D:\repo`，`is_git` 為真

#### Scenario: worktree 的 .git 是檔案

- **GIVEN** pane 的 `cwd` 為 `D:\wt`，`D:\wt\.git` 是檔案
- **WHEN** 推算根目錄
- **THEN** 根目錄為 `D:\wt`，`is_git` 為真

#### Scenario: 不在 git repo 內

- **GIVEN** pane 的 `cwd` 為 `D:\notes`，其本身與所有上層都沒有 `.git`
- **WHEN** 推算根目錄
- **THEN** 根目錄為 `D:\notes`，`is_git` 為假

#### Scenario: WSL 路徑轉換

- **GIVEN** runtime `wsl` 設定的 distro 為 `Ubuntu-24.04`，其 pane 的 `cwd` 為 `/home/u/repo`
- **WHEN** 推算根目錄
- **THEN** 以 `\\wsl.localhost\Ubuntu-24.04\home\u\repo` 為起點往上尋找

#### Scenario: pane 關掉後根目錄不可用

- **GIVEN** 根目錄 `D:\repo` 只由 pane `w1:p1` 推算得出，且曾成功讀取其中的檔案
- **WHEN** 最新投影中已沒有 `w1:p1`，之後再請求 `D:\repo` 下的檔案
- **THEN** 回應為根目錄不可用（見「檔案端點的共同規則」）

### Requirement: 檔案端點的共同規則

系統必須讓本 capability 的所有端點（根目錄查詢、列目錄、中繼資料、Markdown 渲染、原始內容）遵守：只接受 `GET`，其他
method（含 `HEAD`）回 405；套用與 `live-output`「輸出端點只接受本機同源請求」相同的 `Host`／`Origin` 檢查，不符合時回
403；所有回應（含錯誤）帶 `Cache-Control: no-store` 與 `X-Content-Type-Options: nosniff`；不寫入、不建立、不刪除任何檔案，
不執行任何外部程式。檔案端點以根目錄查詢回傳的 `root_id` 指定根目錄，以根目錄內的相對路徑（`/` 分隔、逐段 URL 編碼）
指定目標；目標解析為實體路徑（展開符號連結與 junction）後必須位於根目錄的實體路徑之內，否則回 403。允許清單與上述界限是
唯一的存取控制：列目錄時隱藏 `.git` 與依 `.gitignore` 過濾只影響「列出哪些項目」，不禁止以相對路徑直接讀取這些檔案。
非 200 回應的本體為 `{"error": "<中文原因>", "code": "<代碼>"}`，代碼與狀態碼的對應為：`Host`／`Origin` 檢查不合格 → 403
`forbidden_source`；method 不是 `GET` → 405 `method_not_allowed`（`HEAD` 依 HTTP 沒有本體）；路徑片段不是合法 UTF-8、
`root_id` 無法解碼、或相對路徑含空片段、`.` 或 `..` 片段，或任一片段（解碼後）含 `/`、`\`、`:`、NUL 字元、以 `.` 或空白
結尾、或（去掉副檔名後、不分大小寫）是 Windows 保留裝置名（`CON`、`PRN`、`AUX`、`NUL`、`COM1`–`COM9`、`LPT1`–`LPT9`、
`CONIN$`、`CONOUT$`）→ 400 `bad_request`；runtime 不是設定中的 id → 404 `runtime_unknown`；pane 不在最新投影中 → 404
`pane_unknown`；pane 沒有根目錄 → 404 `no_root`；根目錄不在允許清單 → 404 `root_unavailable`；目標跳出根目錄 → 403
`path_outside_root`；目標不存在 → 404 `not_found`；目標種類不符（例如對資料夾要內容、對檔案要列目錄）→ 400 `wrong_kind`；
超過大小上限 → 413 `too_large`；對不是 Markdown 的檔案要求渲染 → 415 `not_markdown`；讀取時發生其他 I/O 錯誤 → 500
`io_error`。錯誤本體不得包含請求路徑中的片段原文。

#### Scenario: 用 .. 跳出根目錄

- **WHEN** 以某個可用的 `root_id` 分別請求相對路徑 `docs/../../secret.txt`、`..%2F..%2Fsecret.txt`、`C:%5Cx`、`CON.txt`、`a.`
- **THEN** 皆回 400，`code` 為 `bad_request`，沒有開啟任何檔案

#### Scenario: 符號連結指向根目錄外

- **GIVEN** 根目錄內的 `link.txt` 是指向根目錄外 `D:\outside\secret.txt` 的符號連結
- **WHEN** 請求 `link.txt` 的原始內容
- **THEN** 回 403，`code` 為 `path_outside_root`，本體不含 `secret.txt` 的內容

#### Scenario: 根目錄不在允許清單

- **WHEN** 以一個自行把 `C:\Windows` 編成 `root_id` 的值請求列目錄
- **THEN** 回 404，`code` 為 `root_unavailable`

#### Scenario: DNS rebinding 被拒

- **WHEN** 以 `Host: evil.example:7770` 請求任一檔案端點
- **THEN** 回 403，`code` 為 `forbidden_source`，沒有讀取任何檔案

#### Scenario: 不接受其他 method

- **WHEN** 以 `POST` 與 `HEAD` 請求原始內容端點
- **THEN** 兩者都回 405，帶 `Cache-Control: no-store` 與 `X-Content-Type-Options: nosniff`；`POST` 的本體 `code` 為
  `method_not_allowed`

#### Scenario: 被隱藏的檔案仍可直接讀取

- **GIVEN** 根目錄的 `.gitignore` 含 `.env`，根目錄有 `.env`
- **WHEN** 列出根目錄，並請求 `.env` 的中繼資料
- **THEN** 列表中沒有 `.env`；中繼資料回 200

### Requirement: 根目錄查詢端點

系統必須提供 `GET /api/runtimes/<runtime>/panes/<pane>/root`：回 200 與 JSON 物件，欄位為 `runtime`、`pane_id`、
`root_id`（字串，之後所有檔案端點以它指定根目錄；同一個根目錄在任何時候得到相同的值；只含 URL 路徑片段可直接使用的字元）、
`root_path`（根目錄的主機路徑，供顯示）、`cwd_path`（pane 回報的原始 `cwd`）、`name`（根目錄最後一段名稱）、`is_git`（布林）。

#### Scenario: 查到根目錄

- **GIVEN** runtime `win` 的 pane `w1:p1` 的 `cwd` 為 `D:\repo\src`，`D:\repo\.git` 存在
- **WHEN** `GET /api/runtimes/win/panes/w1:p1/root`
- **THEN** 回 200，`root_path` 為 `D:\repo`，`name` 為 `repo`，`is_git` 為 `true`

#### Scenario: pane 沒有 cwd

- **GIVEN** pane `w1:p2` 的 `cwd` 為 null
- **WHEN** 查詢其根目錄
- **THEN** 回 404，`code` 為 `no_root`

### Requirement: 列目錄端點

系統必須提供 `GET /api/files/<runtime>/<root_id>/list`（根目錄本身）與 `GET /api/files/<runtime>/<root_id>/list/<相對路徑>`：
只列出該目錄的直接子項目（不遞迴），回 200 與 JSON 物件 `{"entries": [...], "omitted": <數字>, "skipped": <數字>}`。
每筆 entry 含 `name`、`kind`（`"dir"` 或 `"file"`）、`icon`（收合時的 icon 檔名）與 `icon_open`（資料夾展開時的 icon
檔名；檔案為 null），icon 依「檔案 icon」。過濾規則只取根目錄以內：根目錄與其下各層（到被列出的目錄為止）的 `.gitignore`，
以及根目錄是 git repo 時的 `.git/info/exclude`；根目錄以上各層的 `.gitignore` 與使用者的全域忽略設定不套用；根目錄不是 git repo
時同樣套用根目錄以內的 `.gitignore`。名為 `.git` 的項目一律不列出；其他以 `.` 開頭的項目照常列出。排序為資料夾在前、檔案在後，
各自依名稱不分大小寫排序。一個目錄超過 5000 筆時，回傳排序後的前 5000 筆，`omitted` 為未回傳的筆數；名稱不是合法 UTF-8 的
項目不列出，`skipped` 為其筆數。

#### Scenario: 依 .gitignore 過濾

- **GIVEN** 根目錄有 `.gitignore`（內容 `target/`）、資料夾 `target`、`src`、`.github`、`.git` 與檔案 `README.md`、`.env.example`
- **WHEN** 列出根目錄
- **THEN** `entries` 依序為 `.github`、`src`（資料夾）與 `.env.example`、`.gitignore`、`README.md`（檔案），
  沒有 `target` 與 `.git`

#### Scenario: 大目錄截斷

- **GIVEN** 某資料夾有 5003 個檔案
- **WHEN** 列出該資料夾
- **THEN** `entries` 有 5000 筆，`omitted` 為 3

#### Scenario: 根目錄以上的 .gitignore 不生效

- **GIVEN** 根目錄為 `D:\outer\repo`（不是 git repo），`D:\outer\.gitignore` 含 `*.md`，根目錄有 `README.md`
- **WHEN** 列出根目錄
- **THEN** `entries` 含 `README.md`

### Requirement: 中繼資料端點

系統必須提供 `GET /api/files/<runtime>/<root_id>/meta/<相對路徑>`：回 200 與 JSON 物件，欄位為 `size`（位元組）、
`modified_ms`（最後修改時間，Unix epoch 毫秒）、`viewer`、`icon`（依「檔案 icon」的檔案規則）、`vscode_uri`（依「在 VS Code
開啟」，不適用時為 null）。`viewer` 依副檔名（不分大小寫）與內容決定：`.md`、`.markdown`
→ `"markdown"`；`.pdf` → `"pdf"`；`.html`、`.htm` → `"html"`；其他檔案若前 8192 位元組不含 NUL 且為合法 UTF-8（容許結尾
被截斷的多位元組字元）→ `"text"`；其餘 → `"unsupported"`。此端點只讀取判斷所需的前 8192 位元組，不受大小上限限制。

#### Scenario: 分類

- **GIVEN** 根目錄有 `a.MD`、`b.pdf`、`c.htm`、`d.toml`、`e.png`（含 NUL 位元組）
- **WHEN** 分別查詢中繼資料
- **THEN** `viewer` 依序為 `markdown`、`pdf`、`html`、`text`、`unsupported`

#### Scenario: 修改後中繼資料改變

- **GIVEN** 已查詢過 `README.md` 的中繼資料
- **WHEN** 內容被改寫後再查詢
- **THEN** `size` 或 `modified_ms` 至少一項與先前不同

### Requirement: Markdown 渲染端點

系統必須提供 `GET /api/files/<runtime>/<root_id>/render/<相對路徑>`：對 `viewer` 為 `markdown` 的檔案回 200，本體為
`text/html; charset=utf-8` 的 HTML 片段（不含 `<html>`、`<head>`、`<body>`），並帶 `Content-Security-Policy: sandbox`。支援
GitHub 風格的表格、任務清單、刪除線與自動連結；每個標題帶可供錨點連結的 `id`，且一律以 `md-` 開頭（避免與頁面既有元素的 `id`
相同）。原始 HTML 區塊與行內 HTML 不得原樣輸出；`javascript:`、`vbscript:`、
`data:`（圖片以外）等危險連結不得出現在輸出的 `href` 或 `src` 中。檔案超過 2 MiB 回 413 `too_large`；內容不是合法
UTF-8 時以替代字元處理，不回錯誤。

#### Scenario: GFM 元素

- **GIVEN** `doc.md` 含一個表格、一個 `- [x] 完成項` 任務清單與 `~~刪除~~`
- **WHEN** 請求渲染
- **THEN** 輸出含 `table` 元素、一個已勾選的 checkbox 與 `del` 元素

#### Scenario: 夾帶的 HTML 與危險連結被清掉

- **GIVEN** `evil.md` 含 `<script>window.pwned=1</script>`、`<img src=x onerror=alert(1)>` 與 `[x](javascript:alert(1))`
- **WHEN** 請求渲染
- **THEN** 輸出不含 `<script`、`onerror` 與 `javascript:`

### Requirement: 原始內容端點

系統必須提供 `GET /api/files/<runtime>/<root_id>/raw/<相對路徑>`：回 200 與檔案原始位元組。`Content-Type` 依副檔名（不分
大小寫）：`.html`／`.htm` → 回應的位元組是合法 UTF-8 時為 `text/html; charset=utf-8`，不是合法 UTF-8 時為不帶 charset 的
`text/html`（由檔案內的 `<meta charset>` 或 BOM 決定編碼）、`.pdf` → `application/pdf`、`.svg` → `image/svg+xml`、
`.png` → `image/png`、`.jpg`／`.jpeg` → `image/jpeg`、`.gif` → `image/gif`、`.webp` → `image/webp`、`.css` → `text/css`、
`.md`／`.markdown`／`.txt` 與其他 `viewer` 為 `text` 的檔案 → `text/plain; charset=utf-8`，其餘 → `application/octet-stream`。
所有原始內容回應都必須帶 `Content-Security-Policy: sandbox`（使直接開啟該網址時，HTML 或 SVG 中的腳本不在 Cockpit 的來源下
執行）。檔案超過 50 MiB 回 413 `too_large`。

#### Scenario: HTML 帶 sandbox

- **GIVEN** `page.html` 的內容是 UTF-8
- **WHEN** 請求 `page.html` 的原始內容
- **THEN** 回 200，`Content-Type` 為 `text/html; charset=utf-8`，帶 `Content-Security-Policy: sandbox` 與
  `X-Content-Type-Options: nosniff`

#### Scenario: 沒有宣告編碼的 UTF-8 HTML

- **GIVEN** `nometa.html` 以 UTF-8 寫成、含中文「檔案瀏覽」，檔內沒有 `<meta charset>`，也沒有 BOM
- **WHEN** 請求其原始內容，並在檔案分頁中開啟它
- **THEN** 原始內容回應的 `Content-Type` 為 `text/html; charset=utf-8`；檔案分頁的 HTML 檢視器顯示「檔案瀏覽」字樣，不是亂碼

#### Scenario: 非 UTF-8 的 HTML

- **GIVEN** `old.html` 以 Big5 編碼寫成、含中文，並宣告 `<meta charset="big5">`
- **WHEN** 請求其原始內容
- **THEN** 回 200，`Content-Type` 為不帶 charset 的 `text/html`，回應本體與檔案位元組完全相同

#### Scenario: 太大

- **GIVEN** `big.pdf` 為 60 MiB
- **WHEN** 請求其原始內容
- **THEN** 回 413，`code` 為 `too_large`

### Requirement: 左欄檔案樹

系統必須在左欄頂端提供「Project」「檔案」「變更」三個分頁（預設「Project」；「變更」分頁的內容見 `git-review`「左欄變更
分頁」），三者為同一個 `role="tablist"`，可用方向鍵移動、Enter／Space 選定。「檔案」分頁顯示目前選定 pane（`live-output`
「選定一個 pane」）的根目錄的檔案樹：頂端顯示根目錄 `name`、runtime `id` 與「重新整理」按鈕；根目錄的子項目在切到此分頁
或根目錄改變時讀取，資料夾在展開時才讀取該層；每一列顯示 icon 與名稱，完整相對路徑放在 `title`。點資料夾列（或以鍵盤
Enter／Space）切換展開與收合，列帶 `aria-expanded`；點檔案列開啟或切換到該檔案的分頁（見「檔案分頁」）。「重新整理」
重新讀取根目錄與所有已展開的資料夾。沒有選定 pane 時顯示「先在 Factory Floor 或 runtime 清單選一個 pane」；選定的 pane
沒有根目錄或查詢失敗時顯示原因。展開狀態依根目錄分別保留：換到另一個根目錄再換回來時，原本展開的資料夾仍展開。整頁
重畫（`cockpit-dashboard`「畫面整頁重畫」）不得改變檔案樹的內容、展開狀態、內部捲動位置與鍵盤焦點。`omitted` 或 `skipped`
大於 0 時，在該資料夾末端顯示「還有 N 項未顯示」。

#### Scenario: 左欄三個分頁

- **WHEN** 載入頁面
- **THEN** 左欄頂端依序為「Project」「檔案」「變更」三個分頁，目前為「Project」；在分頁上按右方向鍵兩次再按 Enter，目前分頁為「變更」

#### Scenario: 切到檔案分頁

- **GIVEN** 已選定 pane `w1:p1`，其根目錄為 `repo`
- **WHEN** 點左欄的「檔案」分頁
- **THEN** 顯示 `repo` 的檔案樹，第一層為根目錄的子項目

#### Scenario: 沒有選定 pane

- **GIVEN** 沒有選定任何 pane，也沒有打開任何檔案分頁
- **WHEN** 切到「檔案」分頁
- **THEN** 顯示提示選一個 pane 的空狀態，頁面沒有發出任何檔案端點的請求

#### Scenario: 展開狀態跨根目錄保留

- **GIVEN** 選定 pane `w1:p1`（根目錄 `repo`）並展開了 `src`
- **WHEN** 改選根目錄不同的 pane `w2:p1`，再改選回 `w1:p1`
- **THEN** `repo` 的檔案樹中 `src` 仍為展開

#### Scenario: 重畫不影響檔案樹

- **GIVEN** 檔案樹已展開 `src` 並往下捲動，焦點在某個檔案列上，投影每 100 ms 推送一份新的 version
- **WHEN** 經過 3 秒
- **THEN** 檔案樹的 DOM 節點沒有被換掉，展開狀態、捲動位置與焦點都不變

### Requirement: 檔案分頁

系統必須把中欄下半部做成分頁區（`cockpit-dashboard`「版面與窄視窗」）：第一個分頁固定為 Live Output、不可關閉；其後每個
打開的檔案一個分頁，以及 `git-review` 定義的 diff、Git Graph、某版本檔案分頁（各自的身分、標題與內容見該 capability），
全部依開啟順序排列、共用下列規則。檔案分頁顯示 icon、檔名與關閉按鈕，完整相對路徑與根目錄名稱放在 `title`。檔案分頁以
runtime、`root_id` 與相對路徑識別：開啟已打開的檔案時切換到既有分頁，不新增（其他種類的分頁同理，以各自的身分判斷）。
同一時間恰有一個目前分頁；檔案並排顯示時，目前分頁為焦點欄的分頁，並排中的選定、開檔與關閉規則見「檔案並排」。沒有並排
顯示時，關閉目前分頁改為顯示其右側的分頁，沒有右側時顯示左側的分頁。分頁列為 `role="tablist"`，可用方向鍵在分頁間移動、
Enter／Space 選定；關閉按鈕可用鍵盤操作。分頁過多時分頁列在內部橫向捲動，頁面不得出現橫向捲軸。分頁不屬於任何 Project：
切換 Project 不改變已打開的分頁與並排組合。每個檔案分頁的內容區上方有一列工具列，顯示相對路徑、最後一次成功讀取的時間與
「在 VS Code 開啟」（見該需求）。整頁重畫不得改變分頁區的內容、捲動位置與並排狀態。

#### Scenario: 開檔新增分頁

- **GIVEN** 下半部只有 Live Output 分頁
- **WHEN** 在檔案樹點 `README.md`
- **THEN** 出現 `README.md` 分頁並成為目前分頁，Live Output 分頁仍在第一個

#### Scenario: 重複開啟不新增

- **GIVEN** 已打開 `README.md` 與 `docs/a.md` 兩個分頁，目前為 `docs/a.md`
- **WHEN** 再點檔案樹的 `README.md`
- **THEN** 分頁數不變，目前分頁改為 `README.md`

#### Scenario: 關閉目前分頁

- **GIVEN** 分頁依序為 Live Output、`a.md`、`b.md`、`c.md`，目前為 `b.md`，沒有並排
- **WHEN** 關閉 `b.md`
- **THEN** 目前分頁為 `c.md`

#### Scenario: 切換 Project 不影響分頁

- **GIVEN** 已打開 `README.md` 分頁
- **WHEN** 點左欄 Project 分頁中的另一個 Project
- **THEN** `README.md` 分頁仍在、仍為目前分頁

#### Scenario: 切換 Project 不影響並排

- **GIVEN** 已打開 `README.md` 與 `docs/a.md` 並排，焦點欄為 `docs/a.md`
- **WHEN** 點左欄 Project 分頁中的另一個 Project
- **THEN** 兩個分頁仍在並排，焦點欄仍為 `docs/a.md`

#### Scenario: 整頁重畫不影響並排

- **GIVEN** `README.md` 與 `docs/a.md` 並排，焦點欄為 `docs/a.md`，兩欄都已往下捲到中段
- **WHEN** 投影每 100 ms 推送一份新的 version，持續 3 秒
- **THEN** 仍為兩欄並排、焦點欄仍為 `docs/a.md`，兩欄捲動位置不變，面板的 DOM 節點沒有被換掉

### Requirement: 檔案檢視器

系統必須依中繼資料的 `viewer` 顯示檔案分頁的內容：

- `markdown`：顯示 Markdown 渲染端點的輸出，排版遵守 `cockpit-dashboard`「Direction 01 視覺語彙」。連結中指向根目錄內
  檔案的相對路徑，點選後在分頁區開啟該檔案（可帶 `#錨點`）；只有 `#錨點` 的連結捲動到同頁的對應標題；`http:`／`https:`
  連結在新的瀏覽器分頁開啟（`rel="noopener noreferrer"`）；其他連結不動作。圖片只載入根目錄內的相對路徑（改為讀取原始內容
  端點）；絕對網址（含 `http:`、`https:`、`//` 開頭）或解析後跳出根目錄的圖片不得發出任何請求，改顯示其替代文字。連結與
  圖片的改寫必須在內容放入頁面之前完成（不得先插入再改寫，否則瀏覽器已對原始 `src` 發出請求）。
- `text`：以等寬字體、保留換行與空白顯示原始內容並附行號；內容一律以純文字呈現，不得被瀏覽器解讀為 HTML。檔案超過
  2 MiB 時不讀取內容，改顯示「檔案太大，無法預覽」。
- `html`：以 iframe 載入原始內容端點；iframe 必須帶 `sandbox` 屬性且不含 `allow-scripts` 與 `allow-same-origin`。
  同一個根目錄內以相對路徑引用的樣式表與圖片必須能載入。
- `pdf`：逐頁顯示所有頁面（連續捲動），工具列有「目前頁／總頁數」、上一頁、下一頁、放大、縮小、符合寬度；含中文的 PDF
  必須正確顯示中文字。無法解析時顯示「PDF 無法解析」。
- `unsupported`：顯示「不支援預覽」與檔案大小。

讀取失敗時分頁不消失，改顯示原因（依錯誤 `code` 對應的中文說明，不以 HTTP 狀態碼或 API 路徑開頭）。

#### Scenario: md 相對連結在分頁區開啟

- **GIVEN** `README.md` 含 `[設計](docs/design.md#決策)`
- **WHEN** 在其分頁點這個連結
- **THEN** 開啟（或切換到）`docs/design.md` 分頁，並捲動到 `決策` 標題

#### Scenario: 外部圖片不載入

- **GIVEN** `README.md` 含 `![logo](https://example.com/logo.png)` 與 `![圖](docs/pic.png)`
- **WHEN** 開啟其分頁
- **THEN** 頁面沒有對 `example.com` 發出任何請求，該處顯示替代文字 `logo`；`docs/pic.png` 經原始內容端點載入

#### Scenario: HTML 內的腳本不執行

- **GIVEN** `page.html` 含 `<script>parent.postMessage('ran','*')</script>` 與同目錄 `style.css` 的 `<link>`
- **WHEN** 開啟其分頁
- **THEN** 頁面沒有收到 `ran` 訊息；iframe 的 `sandbox` 屬性不含 `allow-scripts`；`style.css` 有被請求且成功

#### Scenario: 中文 PDF

- **GIVEN** `report.pdf` 共 3 頁，第 1 頁含中文標題
- **WHEN** 開啟其分頁
- **THEN** 工具列顯示「1 / 3」，3 頁都有畫出內容，第 1 頁的中文標題可辨識（非空白、非方框）

#### Scenario: 純文字不被解讀

- **GIVEN** `note.txt` 含 `<b>x</b>`
- **WHEN** 開啟其分頁
- **THEN** 字樣原樣顯示，內容區沒有 `b` 元素

### Requirement: 自動更新

系統必須對每個可見的檔案分頁各自定時查詢中繼資料。可見的檔案分頁是指：沒有並排顯示時的目前分頁（若它是檔案分頁）；
並排顯示時，並排組合中實際顯示的各分頁。同一個分頁的前一次查詢結束（成功或失敗）後 2 秒才發下一次，同一個分頁同一時間
至多一個進行中的查詢；檔案分頁從不可見變成可見時立即查詢一次。沒有任何可見的檔案分頁時（例如目前分頁是 Live Output，
或沒有打開任何檔案分頁），不得為檔案分頁發出中繼資料查詢；不可見的檔案分頁（含窄視窗下未顯示的並排欄）不得查詢。
`size` 或 `modified_ms` 與上次顯示時不同時，重新讀取並重畫內容；檔案改變後內容必須在 3 秒內反映。重畫時 `markdown` 與 `text`
維持原本的捲動位置（超出新內容長度時取最大值），`pdf` 維持原本的頁碼（超出新總頁數時取最後一頁）。查詢或讀取失敗時，保留
最後一次的內容並以 `live-output`「失敗與消失的呈現」相同的過期標示呈現，顯示原因，並依原節奏繼續查詢；恢復後過期標示消失。
分頁變成不可見或被關閉之後才回來的舊回應必須丟棄。

#### Scenario: 改檔後更新並保住捲動

- **GIVEN** `long.md` 的分頁為目前分頁，已往下捲到中段
- **WHEN** 在檔案末端加一段文字
- **THEN** 3 秒內新段落出現在內容中，捲動位置不變

#### Scenario: Live Output 分頁時不查詢

- **GIVEN** 已打開兩個檔案分頁，目前分頁是 Live Output
- **WHEN** 觀察 10 秒
- **THEN** 服務沒有收到任何中繼資料查詢

#### Scenario: 檔案被刪掉後又出現

- **GIVEN** `plan.md` 的分頁為目前分頁
- **WHEN** 檔案被刪除，5 秒後以新內容重新建立
- **THEN** 刪除期間內容標為過期並顯示「檔案已不存在」；重新建立後 3 秒內顯示新內容、過期標示消失

#### Scenario: 並排中的非焦點欄也更新

- **GIVEN** `a.md`（第 1 欄）與 `b.md`（第 2 欄、焦點欄）並排
- **WHEN** 在 `a.md` 末端加一段文字
- **THEN** 3 秒內第 1 欄出現新段落，焦點欄仍為 `b.md`

#### Scenario: 只查詢可見的檔案分頁

- **GIVEN** 已打開 `a.md`、`b.md`、`c.md`，其中 `a.md` 與 `b.md` 並排，視窗寬 1280
- **WHEN** 觀察 10 秒，再把視窗寬度改為 700、焦點欄為 `b.md`，再觀察 10 秒
- **THEN** 前 10 秒服務只收到 `a.md` 與 `b.md` 的中繼資料查詢；後 10 秒只收到 `b.md` 的；`c.md` 始終沒有被查詢

### Requirement: 分頁還原

系統必須把下列狀態存到瀏覽器本機儲存，並在載入頁面時還原：已打開的分頁（依分頁順序；檔案分頁存 runtime、`root_id`、
相對路徑、根目錄名稱，`git-review` 定義的分頁存其身分所需的欄位與根目錄名稱）、目前分頁、並排組合（依欄位順序）與焦點欄、
左欄目前分頁（「Project」「檔案」「變更」之一）。較早的儲存格式（只有檔案分頁的格式，以及沒有並排組合的格式）必須照常還原，
視為沒有並排組合。儲存的並排組合不合法時，忽略並排組合，其餘狀態照常還原。不合法包括：少於 2 個或多於 3 個分頁；有重複；
指到非檔案分頁；指到不存在的分頁，或指到還原時因資料不合法而被略過的分頁。儲存的焦點欄不在並排組合中時，改用並排組合的
第一欄；目前分頁在並排組合中時，焦點欄一律為目前分頁。還原的分頁所屬根目錄不可用時，分頁仍保留並顯示「這個根目錄目前沒有任何 pane，無法讀取」，依自動更新的
節奏重試，恢復後正常顯示。瀏覽器本機儲存不可用或內容損毀時，以沒有已打開分頁的狀態開始，頁面其餘功能正常。Live Output 的
pane 選取不還原（沿用 `live-output`「選定一個 pane」）。

#### Scenario: 重新整理後還原

- **GIVEN** 已打開 `README.md` 與 `docs/a.md`，目前分頁為 `docs/a.md`，左欄目前為「檔案」
- **WHEN** 重新整理頁面
- **THEN** 兩個分頁依原順序還原，目前分頁為 `docs/a.md` 且顯示其內容，左欄為「檔案」分頁

#### Scenario: 儲存內容損毀

- **GIVEN** 瀏覽器本機儲存中對應的值不是合法 JSON
- **WHEN** 載入頁面
- **THEN** 下半部只有 Live Output 分頁，頁面其餘部分正常，console 有警告

#### Scenario: 還原 git 分頁與變更分頁

- **GIVEN** 已打開 `README.md`、`src/a.rs` 的 diff 分頁（變更）與 Git Graph 分頁，目前分頁為 Git Graph，左欄目前為「變更」
- **WHEN** 重新整理頁面
- **THEN** 三個分頁依原順序還原，目前分頁為 Git Graph 且顯示其內容，左欄為「變更」分頁

#### Scenario: 舊格式照常還原

- **GIVEN** 瀏覽器本機儲存中是只有檔案分頁的舊格式，記錄了 `README.md` 與 `docs/a.md` 兩個檔案分頁
- **WHEN** 載入頁面
- **THEN** 兩個檔案分頁依原順序還原並顯示內容，沒有並排

#### Scenario: 還原並排

- **GIVEN** 已打開 `README.md`、`docs/a.md`、`docs/b.md`，其中 `docs/b.md` 與 `README.md` 依序並排，焦點欄為 `README.md`
- **WHEN** 重新整理頁面
- **THEN** 三個分頁依原順序還原；`docs/b.md`、`README.md` 依序兩欄並排並顯示內容，焦點欄為 `README.md`

#### Scenario: 選定 Live Output 時重新整理

- **GIVEN** `a.md` 與 `b.md` 並排，之後選定 Live Output 分頁
- **WHEN** 重新整理頁面
- **THEN** 目前分頁為 Live Output；之後點選 `a.md` 時，`a.md` 與 `b.md` 恢復並排

#### Scenario: 並排資料不合法時忽略

- **GIVEN** 瀏覽器本機儲存的並排組合屬於下列任一情況：只指到一個分頁、指到 4 個分頁、同一個分頁出現兩次、指到 Git Graph
  分頁、指到一筆因資料不合法而在還原時被略過的檔案分頁
- **WHEN** 載入頁面
- **THEN** 其餘可還原的分頁與目前分頁照常還原，沒有並排，console 有警告；被略過的分頁不會讓並排指到別的分頁

### Requirement: 在 VS Code 開啟

系統必須在每個檔案分頁的工具列提供「在 VS Code 開啟」連結，其網址由中繼資料端點的 `vscode_uri` 提供（由服務依 runtime 設定
產生）：非 WSL runtime 的檔案為 `vscode://file/<主機路徑>`（路徑中的 `\` 換成 `/`，各段 URL 編碼、保留 `/` 與磁碟代號的 `:`）；
WSL runtime 的檔案為 `vscode://vscode-remote/wsl+<distro><POSIX 路徑>:1`（POSIX 路徑各段 URL 編碼；結尾的 `:1` 行號
不可省略，否則 VS Code 把遠端路徑當成資料夾開啟，見 `file-review-probe.md` §5）。`vscode_uri` 為 null 時
不顯示此連結。點選不得讓 Cockpit 服務啟動任何程式。

#### Scenario: Windows 檔案

- **GIVEN** runtime `win` 的根目錄為 `D:\repo`，分頁為 `docs/a b.md`
- **WHEN** 查詢其中繼資料並檢查「在 VS Code 開啟」連結
- **THEN** `vscode_uri` 與連結 `href` 皆為 `vscode://file/D:/repo/docs/a%20b.md`

#### Scenario: WSL 檔案

- **GIVEN** runtime `wsl` 的 distro 為 `Ubuntu-24.04`，根目錄主機路徑為 `\\wsl.localhost\Ubuntu-24.04\home\u\repo`，分頁為 `a.md`
- **WHEN** 查詢其中繼資料並檢查連結
- **THEN** `vscode_uri` 與連結 `href` 皆為 `vscode://vscode-remote/wsl+Ubuntu-24.04/home/u/repo/a.md:1`

### Requirement: 檔案 icon

系統必須以 Material Icon Theme（MIT）的 icon 與其主題對照表，為檔案樹的每一列（列目錄端點的 `icon`／`icon_open`）與每個檔案
分頁（中繼資料端點的 `icon`；尚未取得中繼資料前顯示預設檔案 icon）指定 icon，icon 保留原本的顏色。
檔案依序比對：完整檔名（不分大小寫）→ 副檔名（由最長的多段副檔名往短比對，例如 `a.d.ts` 先比 `d.ts` 再比 `ts`）→ 預設檔案 icon。
資料夾依資料夾名稱（不分大小寫）比對，收合與展開各用對照表中對應的 icon，沒有對應時用預設的資料夾 icon。icon 為裝飾性圖片
（`alt=""`，不被輔助技術朗讀）。icon 檔與對照表內嵌在執行檔中，隨附其 MIT 授權聲明。

#### Scenario: 常見檔案

- **WHEN** 列出含 `README.md`、`Cargo.toml`、`report.pdf`、`index.html`、`unknown.zzz` 的目錄
- **THEN** `README.md` 使用對照表中 `readme.md` 對應的 icon（完整檔名優先於 `.md`），`report.pdf`、`index.html` 使用各自對照的 icon，
  `unknown.zzz` 使用預設檔案 icon

#### Scenario: 資料夾展開

- **GIVEN** 根目錄有資料夾 `src`
- **WHEN** 列出根目錄
- **THEN** `src` 的 `icon` 與 `icon_open` 分別為對照表中 `src` 收合與展開的 icon，兩者不同

### Requirement: 檔案並排

系統必須允許最多 3 個檔案分頁同時並排顯示在分頁區的內容區，各欄等寬，每欄顯示該分頁原本的內容（含工具列）。只有檔案
分頁能加入並排：Live Output 與 `git-review` 定義的分頁不顯示並排鈕，也不得加入並排組合。

本需求的用語：

- **並排組合**：有順序的 2～3 個檔案分頁（欄位順序由左而右），或者「沒有並排組合」。並排組合記住其中一個分頁作為
  **焦點欄**；沒有並排組合時也就沒有焦點欄。
- **並排中**：目前分頁在並排組合裡。並排中時，目前分頁一定是焦點欄；以下「替換」「移出」所說的焦點欄，在並排中之外
  也照樣指這個記住的分頁。

規則：

- **並排鈕**：每個檔案分頁都有「並排」切換鈕，`aria-pressed` 反映該分頁是否在並排組合中。按住 Ctrl 點選分頁，或焦點在
  分頁上時按 Ctrl＋Enter，效果與按該分頁的並排鈕相同；但若該分頁不是檔案分頁、或其並排鈕為停用狀態，這兩種操作就等同
  一般的選定。並排鈕在分頁為目前分頁、或滑鼠移到分頁上時顯示；只有目前分頁的並排鈕在 Tab 順序中。
- **加入**：對不在並排組合中的檔案分頁 X 按並排時：
  - 沒有並排組合，且目前分頁是 X 以外的檔案分頁 Y：並排組合成為 Y、X 兩欄。
  - 沒有並排組合，且目前分頁不是 X 以外的檔案分頁（目前分頁就是 X 自己、是 Live Output，或是 `git-review` 定義的分頁）：
    X 的並排鈕為停用狀態，並以 `title` 說明需要先選另一個檔案分頁，按了不動作。
  - 已有並排組合且未滿 3 個：X 加到最右欄。
  - 已有並排組合且已滿 3 個：X 取代焦點欄的分頁，位置不變；被取代的分頁仍留在分頁列。
  - 只要 X 有加入並排組合，X 就成為焦點欄與目前分頁，進入並排中。
- **焦點欄的標示與切換**：並排中時，焦點欄外框以強調色標示，其分頁就是目前分頁（分頁列上 `aria-selected="true"` 的分頁）。
  以下操作會讓某一欄成為焦點欄：在該欄內按下滑鼠；選定並排組合中的分頁（點選，或方向鍵移動後按 Enter／Space）。
  並排組合中的每個分頁在分頁列上都帶欄位編號（1～3）標記，並有輔助技術讀得出的「並排第 N 欄」說明。
- **替換**：並排中時，選定一個不在並排組合中的檔案分頁，該分頁取代焦點欄的分頁，位置不變，並成為焦點欄。適用的操作
  包括：點選分頁、從檔案樹開檔、點 md 相對連結開檔，含因此新開的分頁。被取代的分頁仍留在分頁列。其他欄不得重新讀取內容，
  捲動位置不變。選定的分頁已在並排組合中時，只是讓它成為焦點欄，不替換。
- **移出**：對並排組合中的分頁按並排，或關閉該分頁，它就移出並排組合。
  - 移出的若是焦點欄，焦點欄改為其右側欄，沒有右側時為左側欄。
  - 並排組合只剩一個分頁時，並排組合解除。
  - 目前分頁只在並排中時才跟著改變：移出前是並排中，就改成新的焦點欄；並排組合因此解除時，改成剩下的那個分頁，以單欄
    顯示。移出前不是並排中，目前分頁不變。
  - 關閉並排組合中的分頁時，依上述規則決定目前分頁，不套用「檔案分頁」中「改為顯示右側分頁」的規則。
- **不在並排中**：選定 Live Output、`git-review` 定義的分頁，或不在並排組合中的檔案分頁（且不是並排中，所以不適用替換，
  例如從 Live Output 經檔案樹開啟另一個檔案）時，只以單欄顯示目前分頁，並排組合與其焦點欄保留。之後選定並排組合中的任一
  分頁，就回到並排中：整組恢復並排顯示，該分頁為焦點欄，各欄捲動位置與離開前相同。
- **窄視窗**：視窗寬度小於 760 CSS px 時，並排中仍適用上述所有規則，但只顯示焦點欄；寬度回到至少 760 CSS px 時恢復並排
  顯示。未顯示的欄視同不可見（見「自動更新」）。
- **版面**：並排不得讓頁面出現橫向捲軸，也不得改變中欄寬度；某欄內容超出欄寬時，在該欄內捲動或折行。

#### Scenario: 加入並排

- **GIVEN** 已打開 `README.md` 與 `docs/a.md` 兩個檔案分頁，目前為 `README.md`
- **WHEN** 按 `docs/a.md` 分頁的並排鈕
- **THEN** 內容區分成等寬兩欄，左為 `README.md`、右為 `docs/a.md`；`docs/a.md` 為焦點欄與目前分頁；分頁列上兩者分別
  標示 1、2，兩者的並排鈕 `aria-pressed="true"`

#### Scenario: Ctrl＋點選加入並排

- **GIVEN** 已打開 `README.md` 與 `docs/a.md`，目前為 `README.md`
- **WHEN** 按住 Ctrl 點選 `docs/a.md` 分頁
- **THEN** 結果與按 `docs/a.md` 的並排鈕相同

#### Scenario: 鍵盤加入並排

- **GIVEN** 已打開 `README.md` 與 `docs/a.md`，目前為 `README.md`，鍵盤焦點在分頁列的 `README.md` 上
- **WHEN** 按右方向鍵把焦點移到 `docs/a.md`，再按 Ctrl＋Enter
- **THEN** 兩者並排，`docs/a.md` 為焦點欄

#### Scenario: 沒有另一個檔案分頁時不並排

- **GIVEN** 只打開 `README.md` 一個檔案分頁，且它是目前分頁
- **WHEN** 按 `README.md` 的並排鈕
- **THEN** 仍以單欄顯示 `README.md`；該並排鈕為停用狀態，`title` 說明需要先選另一個檔案分頁

#### Scenario: 替換焦點欄不影響其他欄

- **GIVEN** `README.md`（第 1 欄）與 `docs/a.md`（第 2 欄、焦點欄）並排，`README.md` 已往下捲到中段，另已打開未並排的
  `docs/b.md`
- **WHEN** 點選 `docs/b.md` 分頁
- **THEN** 第 2 欄改為 `docs/b.md` 並為焦點欄；`docs/a.md` 分頁仍在分頁列；`README.md` 欄沒有重新讀取內容，捲動位置不變

#### Scenario: 從檔案樹開檔替換焦點欄

- **GIVEN** `README.md`（第 1 欄、焦點欄）與 `docs/a.md`（第 2 欄）並排
- **WHEN** 在檔案樹點選尚未打開的 `docs/c.md`
- **THEN** 新增 `docs/c.md` 分頁，它取代第 1 欄並為焦點欄；`docs/a.md` 仍在第 2 欄

#### Scenario: 在欄內點選切換焦點欄

- **GIVEN** `README.md`（第 1 欄）與 `docs/a.md`（第 2 欄、焦點欄）並排
- **WHEN** 在 `README.md` 欄的內容上按下滑鼠
- **THEN** `README.md` 欄成為焦點欄、目前分頁為 `README.md`，兩欄內容都沒有重新讀取

#### Scenario: 三欄已滿時替換焦點欄

- **GIVEN** `a.md`、`b.md`、`c.md` 並排，焦點欄為 `b.md`，另已打開 `d.md`
- **WHEN** 按 `d.md` 的並排鈕
- **THEN** 仍為三欄，依序為 `a.md`、`d.md`、`c.md`，`d.md` 為焦點欄；`b.md` 分頁仍在分頁列

#### Scenario: 移出焦點欄

- **GIVEN** `a.md`、`b.md`、`c.md` 並排，焦點欄為 `b.md`
- **WHEN** 按 `b.md` 的並排鈕
- **THEN** `a.md`、`c.md` 兩欄並排，`c.md` 為焦點欄；`b.md` 分頁仍在分頁列，並排鈕 `aria-pressed="false"`

#### Scenario: 關閉後只剩一個時解除並排

- **GIVEN** `a.md` 與 `b.md` 並排，焦點欄為 `b.md`
- **WHEN** 關閉 `b.md` 分頁
- **THEN** 解除並排，`a.md` 以單欄顯示並為目前分頁

#### Scenario: 不在並排中時關閉並排組合的成員

- **GIVEN** `a.md`、`b.md`、`c.md` 並排，焦點欄為 `b.md`，之後選定 Live Output
- **WHEN** 關閉 `b.md` 分頁，再點選 `a.md` 分頁
- **THEN** 關閉後目前分頁仍為 Live Output；點選 `a.md` 後 `a.md`、`c.md` 兩欄並排，`a.md` 為焦點欄

#### Scenario: 不在並排中時加入已滿的並排組合

- **GIVEN** `a.md`、`b.md`、`c.md` 並排，焦點欄為 `b.md`，之後選定 Live Output，另已打開 `d.md`
- **WHEN** 按 `d.md` 的並排鈕
- **THEN** 依序為 `a.md`、`d.md`、`c.md` 三欄並排顯示，`d.md` 為焦點欄與目前分頁

#### Scenario: 並排鈕停用時 Ctrl＋點選等同一般選定

- **GIVEN** 已打開 `README.md` 與 `docs/a.md`，目前為 `docs/a.md`，沒有並排組合
- **WHEN** 按住 Ctrl 點選 `docs/a.md` 分頁
- **THEN** 沒有形成並排，仍以單欄顯示 `docs/a.md`

#### Scenario: 窄視窗下仍替換焦點欄

- **GIVEN** 視窗寬 700，`a.md` 與 `b.md` 並排，焦點欄為 `b.md`，另已打開 `c.md`
- **WHEN** 點選 `c.md` 分頁，再把視窗寬度改為 1280
- **THEN** 寬 700 時只顯示 `c.md`；改為 1280 後依序為 `a.md`、`c.md` 兩欄並排，`c.md` 為焦點欄

#### Scenario: 切到 Live Output 後整組恢復

- **GIVEN** `a.md` 與 `b.md` 並排，兩欄都已往下捲到中段
- **WHEN** 點選 Live Output 分頁，再點選 `a.md` 分頁
- **THEN** 選定 Live Output 期間只顯示 Live Output；回到 `a.md` 後兩欄恢復並排，`a.md` 為焦點欄，兩欄捲動位置與離開前相同

#### Scenario: 非檔案分頁不能並排

- **GIVEN** 已打開 `a.md` 與 Git Graph 分頁，目前為 `a.md`
- **WHEN** 按住 Ctrl 點選 Git Graph 分頁
- **THEN** Git Graph 成為目前分頁並單獨顯示，沒有形成並排；Git Graph 與 Live Output 分頁上都沒有並排鈕

#### Scenario: 窄視窗只顯示焦點欄

- **GIVEN** 視窗寬 1280，`a.md` 與 `b.md` 並排，焦點欄為 `b.md`
- **WHEN** 把視窗寬度改為 700，再改回 1280
- **THEN** 寬 700 時只顯示 `b.md`，頁面沒有橫向捲軸；改回 1280 後恢復兩欄並排

#### Scenario: 三欄並排不撐破頁面

- **GIVEN** 視窗寬 1280，`a.md`、含 300 個字元長行的 `long.txt`、`report.pdf` 三欄並排
- **WHEN** 重畫
- **THEN** 頁面沒有橫向捲軸，中欄寬度與沒有並排時相同，`long.txt` 的長行在該欄內捲動
