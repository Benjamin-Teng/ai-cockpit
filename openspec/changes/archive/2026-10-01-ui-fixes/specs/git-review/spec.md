# git-review（delta）

## MODIFIED Requirements

### Requirement: git 讀取的安全邊界

系統必須只以「讀取」的方式使用 git：只執行 status、列 refs、列 commit、讀 commit 內容、比較兩個版本、找共同祖先、讀取某版本
的檔案內容這幾類查詢，且能執行的查詢種類在程式中是封閉清單，HTTP 請求無法讓服務執行清單以外的 git 子命令或加入任意引數。
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

### Requirement: refs 端點

系統必須提供 `GET /api/git/<runtime>/<root_id>/refs`：回 200 與 JSON 物件，欄位為 `head`（`oid`：HEAD 的 hash 或 null；`ref`：
HEAD 指向的完整 refname，分離 HEAD 時為 null）與 `refs`（陣列，每筆 `name`（完整 refname）、`short`（短名稱）、`kind`
（`branch`、`remote`、`tag` 之一）、`oid`（指向的 commit hash；附註 tag 取其指向的 commit；tag 剝開〔peel〕後指向的物件
不是 commit〔例如 tree 或 blob〕時，該 tag 仍照常列出，`oid` 為剝開後那個物件的 hash）、`commit`（布林：`oid` 剝開後的
物件是否可作為 commit 起點：剝開後為 tree 或 blob 時為 `false`，其餘為 `true`））。只列 `refs/heads/`、
`refs/remotes/`、`refs/tags/` 下的 ref，不列 `refs/stash` 與遠端的 `HEAD` 符號 ref。

#### Scenario: 分支、遠端與 tag

- **GIVEN** repo 有本地分支 `main`、`feat/x`，遠端追蹤分支 `origin/main`，附註 tag `v0.1`，以及一筆 stash
- **WHEN** 請求 refs
- **THEN** `refs` 含這四個 ref，`kind` 分別為 `branch`、`branch`、`remote`、`tag`；`v0.1` 的 `oid` 為它指向的 commit；沒有任何
  `refs/stash` 項目；`head.ref` 為 `refs/heads/main`

#### Scenario: tag 指向非 commit 的物件

- **GIVEN** repo 有一個輕量 tag `tree-tag` 指向某個 tree 物件
- **WHEN** 請求 refs
- **THEN** 回 200，`refs` 仍含 `tree-tag`（`kind` 為 `tag`），`oid` 為該 tree 的 hash，`commit` 為 `false`；其他 ref 的
  `commit` 為 `true`

### Requirement: commit 清單端點

系統必須提供 `GET /api/git/<runtime>/<root_id>/log`，參數為 `offset`（預設 0）、`limit`（預設 200，最大 200）、可重複的
`ref`（完整 refname）或可重複的 `tip`（commit hash），`offset + limit` 不得超過 5000，`ref` 與 `tip` 不得同時出現（違反皆回
400 `bad_request`）。起點：有 `tip` 時為這些 commit；有 `ref` 時為這些 ref 目前指向的 commit（每個 `ref` 必須逐字等於目前
refs 清單中的某個 `name`，否則 404 `ref_unknown`）；都沒有時為 refs 端點列出的全部 ref 與 HEAD，但 `commit` 為 `false` 的 ref（剝開後指向 tree 或 blob，例如指向 tree 的 tag）不當起點
（它仍在 refs 清單中，也不使整個請求失敗）。commit 依日期排序（任何 commit
都排在它所有子 commit 之後）。回 200 與 JSON 物件：`tips`（本次使用的起點 hash 陣列）、`rows`（第 `offset` 到
`offset + limit - 1` 筆，每筆 `oid`、`parents`、`author`、`email`、`time`（Unix 秒）、`subject`、`graph`）、`has_more`（之後
是否還有 commit，受 5000 筆上限約束）。`graph` 描述該列在 commit 圖中的排版：`col`（節點所在欄，從 0 起）、`color`
（0–5 的顏色編號）、`lines`（該列要畫的線段陣列，每段 `from`、`to`（欄）、`half`（`top` 為從列頂到節點高度，`bottom` 為從節點
高度到列底）、`color`）。排版必須前綴穩定：同一組 `tips` 下，第 i 列的 `graph` 與 `offset` 無關。每個 commit 與它每個 parent
（若 parent 也在已載入的列中）之間必須有連續的線段相連；沒有合併與分岔的第一父鏈保持在同一欄。

#### Scenario: 分批載入一致

- **GIVEN** repo 有 260 個 commit
- **WHEN** 請求 `limit=200`，再以回傳的 `tips` 請求 `offset=200&limit=200`，另外以同一組 `tips` 請求 `offset=0&limit=200`
  後又請求 `offset=150&limit=100`
- **THEN** 第一次 `has_more` 為 `true`、第二次回 60 列且 `has_more` 為 `false`；兩種分法中相同 `oid` 的列，`graph` 完全相同

#### Scenario: 分支與合併的排版

- **GIVEN** `main` 上 A→B，從 B 分出 `feat` 有 C，`main` 上有 D，再以 E 合併 `feat` 回 `main`
- **WHEN** 請求 commit 清單
- **THEN** E 的 `parents` 為 D 與 C；E、D、B、A 的 `col` 相同；C 的 `col` 與它們不同；E 與 C、C 與 B 之間都有線段相連

#### Scenario: 依分支篩選

- **GIVEN** repo 有 `main` 與 `feat/x`，`feat/x` 有 2 個不在 `main` 上的 commit
- **WHEN** 以 `ref=refs/heads/main` 請求
- **THEN** `rows` 不含那 2 個 commit

#### Scenario: 不在清單中的 ref

- **WHEN** 以 `ref=refs/heads/does-not-exist` 與 `ref=HEAD` 分別請求
- **THEN** 皆回 404，`code` 為 `ref_unknown`

#### Scenario: 指向非 commit 物件的 tag 不當預設起點

- **GIVEN** repo 有分支 `main`（有 commit），以及輕量 tag `tree-tag` 指向某個 tree 物件
- **WHEN** 不帶 `ref` 與 `tip` 請求 commit 清單
- **THEN** 回 200；`tips` 只含 `main` 與 HEAD 對應的 commit，不含 `tree-tag` 指向的 tree；`rows` 為 `main` 上的 commit，
  與沒有 `tree-tag` 時相同；同一 repo 的 refs 仍列出 `tree-tag`

### Requirement: Git Graph 分頁

系統必須為每個 repo 提供一個 Git Graph 分頁（身分為 runtime 與 `root_id`），標題為「Git Graph · <根目錄名稱>」。內容為 commit
清單，每列依序顯示：commit 圖（依 `graph` 繪製節點與線段，6 種車道顏色皆由十色 token 推導；HEAD 所在 commit 的節點以不同樣式
標示）、指向該 commit 的 ref 標籤（本地分支、遠端分支、tag 各有可區分的樣式，HEAD 指向的分支另外標示）與訊息標題、作者、
時間（本地時區 `YYYY-MM-DD HH:mm`）、短 hash。開啟時載入前 200 筆；捲到底部（或按「載入更多」）時以第一批回傳的 `tips` 載入
下一批，直到 `has_more` 為假；達 5000 筆時在末端顯示「已達上限 5000 筆，可用分支篩選縮小範圍」。工具列提供：分支篩選（列出
refs 端點的全部 ref，依本地分支、遠端分支、tag 分組，可複選、可全選或清除；未選任何 ref 時等同全部；變更篩選後從頭重新載入）、
搜尋框（在已載入的 commit 中比對訊息標題、作者名稱、作者 email 與 hash 前綴，不分大小寫；顯示「第 i／共 n 筆」並可以 Enter／
Shift+Enter 或上下按鈕跳到下一筆／上一筆，跳到的列捲入可見範圍並標示；搜尋不改變清單的內容與排版；有搜尋命中時，載入新的一批 commit 不得改變捲動位置，只有使用者
明確移到下一筆／上一筆命中〔含在搜尋框按 Enter〕時才把命中列捲進視野）、「重新整理」（從頭重新
載入）。此分頁為目前分頁時，每次 refs 讀取結束 2 秒後再讀取一次；refs 指向的 commit 與載入時不同時，在工具列下方顯示「分支已
變更」與「重新載入」按鈕，不自動重新載入。commit 列可用滑鼠點選或以鍵盤（上下方向鍵移動、Enter／Space 選取）選取。整頁重畫與
refs 讀取不得改變此分頁的捲動位置與選取。

#### Scenario: 開啟並分批載入

- **GIVEN** repo 有 260 個 commit
- **WHEN** 在「變更」分頁按「Git Graph」，再捲到清單底部
- **THEN** 先顯示 200 列，捲到底部後共顯示 260 列，沒有重複的 commit，末端沒有「載入更多」

#### Scenario: 搜尋跳轉

- **GIVEN** Git Graph 已載入，有 3 個訊息含「fix」的 commit
- **WHEN** 在搜尋框輸入 `FIX` 並按兩次 Enter
- **THEN** 顯示「第 2／共 3 筆」，第二個相符的列在可見範圍內且被標示；清單列數不變

#### Scenario: 搜尋命中後背景載入不拉動捲動

- **GIVEN** repo 有 500 個 commit，Git Graph 已載入前 200 列；搜尋 `fix` 並按 Enter，跳到位於清單前段的第一筆命中；
  使用者接著往下捲到清單底部（離開該命中列）
- **WHEN** 因捲到底部而載入下一批 commit（命中數因此可能增加）
- **THEN** 載入完成後清單的捲動位置（`scrollTop`）與載入前相同，不被拉回目前命中列；「第 i／共 n 筆」的 i 不變、n 反映新的命中數；
  使用者再按 Enter 後，下一筆命中列才被捲入可見範圍並標示

#### Scenario: 有非 commit tag 時不誤報分支變更

- **GIVEN** repo 有一個指向 tree 物件的 tag，Git Graph 分頁為目前分頁，repo 沒有任何變動
- **WHEN** 經過 5 秒
- **THEN** 不出現「分支已變更」

#### Scenario: 分支變更提示

- **GIVEN** Git Graph 分頁為目前分頁
- **WHEN** 在 repo 新增一個 commit
- **THEN** 3 秒內出現「分支已變更」與「重新載入」按鈕，清單內容與捲動位置不變；按「重新載入」後新 commit 出現在第一列

### Requirement: commit 詳情與比較

系統必須在 Git Graph 選取一個 commit 時，於該列下方展開詳情：完整 hash（附「複製」按鈕）、作者與 email、時間、committer（與
作者不同時）、parents（短 hash，點選後捲到並選取該 commit；不在已載入範圍時顯示為純文字）、指向它的 ref（每個附「複製」按鈕，
複製短名稱）、完整訊息（保留換行），以及變更檔案清單（狀態字母、icon、路徑、改名時的原路徑、增刪行數；merge commit 另外註明
「與第一個父 commit 比較」；後端回報檔案清單被截斷〔`truncated` 為真〕時，在清單末端顯示「變更過多，只列出前面一部分」，commit
詳情與兩個 commit 之間的比較皆然）。點檔案列開啟該檔案的 diff 分頁（`compared_to` 或空內容 → 該 commit）；每列另有「看此版本」按鈕，
開啟該 commit 版本的某版本檔案分頁（刪除的檔案改開 `compared_to` 版本）。詳情中有「選為比較基準」按鈕；已有比較基準時選取另一個
commit（或以 Ctrl／⌘＋點選直接指定第二個 commit），詳情改為「比較 <基準短 hash> ↔ <目前短 hash>」，提供「直接比較」與「自分岔點
起」切換（後者以兩者的共同祖先為左側；沒有共同祖先時顯示「兩者沒有共同祖先」），並列出兩者之間的變更檔案，點選開啟對應的 diff
分頁。再點一次已選取的列收合詳情；按 Esc 取消比較基準。詳情區因重新載入而重建時，若焦點原本在詳情內的元素上，焦點必須回到重建後代表同一個對象的元素上，不得落到頁面
最外層；找不到對應元素時焦點才可以離開；焦點外框依最近一次輸入方式呈現（判定方式同 `cockpit-dashboard`「畫面整頁重畫」：滑鼠時
不呈現、鍵盤時照常呈現），唯一差異是：焦點元素在滑鼠操作前已呈現焦點外框時（例如先以鍵盤聚焦、再用滑鼠點同一顆按鈕），重建後照常
呈現，與瀏覽器原生行為一致。複製使用瀏覽器剪貼簿，成功或失敗都以文字提示回饋。

#### Scenario: 看 commit 的變更並開 diff

- **GIVEN** Git Graph 已載入
- **WHEN** 點某個修改了 `m.rs` 的 commit，再點詳情中的 `m.rs`
- **THEN** 詳情顯示完整 hash 與訊息並列出 `m.rs`；分頁區出現 `m.rs` 的 diff 分頁，左側為該 commit 的第一個 parent、右側為該 commit

#### Scenario: 比較兩個 commit

- **GIVEN** Git Graph 已載入
- **WHEN** 點 commit X、按「選為比較基準」，再點 commit Y
- **THEN** 詳情顯示「比較 X ↔ Y」與兩者之間的變更檔案；切到「自分岔點起」後，檔案清單改為兩者共同祖先與 Y 之間的變更

#### Scenario: 複製 hash

- **WHEN** 在詳情按完整 hash 旁的「複製」
- **THEN** 剪貼簿內容為該 commit 的 40 字元 hash，畫面顯示「已複製」

#### Scenario: commit 詳情的檔案清單被截斷

- **GIVEN** 某 commit 變更的檔案多到後端回報 `truncated` 為 `true`
- **WHEN** 點該 commit 展開詳情
- **THEN** 詳情的檔案清單末端顯示「變更過多，只列出前面一部分」；檔案數未被截斷的 commit 不顯示此提示

#### Scenario: 兩個 commit 比較的檔案清單被截斷

- **GIVEN** commit X 與 Y 之間的變更檔案多到後端回報 `truncated` 為 `true`
- **WHEN** 選 X 為比較基準再點 Y
- **THEN** 比較詳情的檔案清單末端顯示「變更過多，只列出前面一部分」

#### Scenario: 詳情重建後焦點留在對應元素

- **GIVEN** 兩個 commit 的比較詳情已顯示，以鍵盤 Tab 把焦點移到「自分岔點起」切換按鈕上（外框可見）
- **WHEN** 按 Enter 切換，比較結果載入完成、詳情區重建
- **THEN** 焦點在重建後代表同一個操作的切換按鈕上，不在 `body` 上，且該按鈕匹配 `:focus-visible`

#### Scenario: 滑鼠觸發的詳情重建不呈現焦點外框

- **GIVEN** 以滑鼠點了詳情中的「自分岔點起」切換按鈕
- **WHEN** 詳情因此重建
- **THEN** 焦點在重建後的對應切換按鈕上，不在 `body` 上，且該按鈕不匹配 `:focus-visible`、沒有焦點外框
