# tasks：git-review

> 執行路徑：SDD｜理由：約 20 個 task，一個 session 做不完；跨新 crate `cockpit-git`、`cockpit` 後端、前端三塊，各有可獨立驗收的工作；
> 含安全邊界（子程序執行、引數注入、repo 設定觸發的外部程式、index 鎖）與並發（子程序上限、逾時、輪詢），屬高風險；前端分頁一般化
> 會改寫既有驗收腳本的前提，後面的 task 疊在前面之上，需要逐 task 的 Codex review。
>
> **審查紀錄**：本 change 全部 task **未經 Codex 審查**；使用者 2026-09-30 決定不等 Codex 週限額，改由 Sonnet 5.5 subagent 分 6 批驗收
> （含 5.4 的整支分支審查），findings 與處理見 `sdd-ledger.md`「Sonnet 驗收」節。勾選以此為準。

通則（每個 task 都適用）：

- 結尾跑 `cargo fmt --check`、`cargo clippy -p <本 task 改到的 crate> --all-targets -- -D warnings`、`cargo test -p <同上>`；
  有改 `cockpit/examples/ui_preview.rs` 時加跑 `cargo test -p cockpit --example ui_preview`；有改 `.md` 時在 repo 根跑
  `markdownlint-cli2 "**/*.md"`（核對 `Linting: N files` 不為 0）；有改 `openspec/` 時跑 `openspec validate --all`。輸出貼進回報。
- Rust 的 task 以 `superpowers:test-driven-development` 進行，回報附 red→green 證據。
- 前端資源以內嵌方式編進執行檔：改完 `cockpit/assets/` 一定先 `cargo build -p cockpit --example ui_preview` 再跑任何腳本。
- **「既有腳本」**＝`docs/research/2026-09-15/reconnect-check.js`、`whatever-check.js`、`docs/research/2026-09-16/actions-check.js`、
  `channel-backoff-check.js`、`factory-floor-check.js`、`docs/research/2026-09-19/live-output-check.js`、
  `docs/research/2026-09-23/visual-check.js`、`docs/research/2026-09-27/files-check.js`。**每個改前端或 `ui_preview` 的 task 結束時全部必須
  全綠**：只改被本 task 打壞、且 spec 已改變的斷言；腳本修改與產品修改分開 commit，commit 訊息逐條列出被改的斷言與對應的 spec
  scenario；不得放寬 spec 沒有改變的斷言。驗收腳本不可並行跑（共用暫存目錄）。
- `factory-floor-check.js` 會覆寫 `docs/research/2026-09-16/task-5.2-scenario-d.png`，跑完 `git checkout --` 還原。
- 跑腳本前確認 7770 沒有人在用；**不是自己開的程序只能回報、不准砍**；跑完依 PID 收尾，確認 7770 與 CDP port 沒有 LISTEN。
- 對 HERDR 只送唯讀請求；Windows 端不得 `herdr server stop`（`AGENTS.md`）。
- **對任何既有 git repo 只讀**：測試與實測只能在自己建立的暫存 repo 內寫入（`git init`、commit、改檔）；對本 repo 與 WSL 內既有 repo
  只執行 design D2 的唯讀查詢。不得修改任何 git 全域／系統設定。
- 呼叫 `wsl.exe` 一律 `--exec`，不用 `--` 與 `--cd`（design D2）。
- 程式碼註解若要引用 task，寫成 `git-review task N.M`。`tasks.md` 由控制端統一勾。
- 每個改外觀的 task（4.x）結束後，由控制端做設計審核（1536／1100／700 三寬 viewport 截圖＋CSS diff，對照 design.md 與
  `docs/direction-01-visual-design.md`），並依 memory 過 frontend-design 審核；有 finding 才回頭修正。

## 1. 基線與實測

- [x] 1.1 記錄基線：在分支 `feat/git-review` 跑 `cargo test --workspace`、`cargo test -p cockpit --example ui_preview`、既有腳本，
  把 passed／failed／ignored 數字與各腳本結果寫進 SDD ledger（`openspec/changes/git-review/sdd-ledger.md`）；驗收＝ledger 有這些數字，
  failed 為 0、腳本全綠
- [x] 1.2 git 旗標與輸出實測（design D2、D4）：在 Windows git 與 WSL git 上，於自建暫存 repo（WSL 端建在 `/tmp` 下，測完刪除）逐一實測並記錄
  指令與輸出摘要到新檔 `docs/research/2026-09-28/git-review-probe.md`：①固定前綴每個旗標可用，`-c core.fsmonitor=false` 能蓋過 repo 設定；
  ②`--end-of-options` 在 `log`、`show`、`merge-base`、`rev-parse` 可用；③`--literal-pathspecs` 下 `:(glob)*` 被當字面路徑；④porcelain v2 的
  `1`／`2`／`u`／`?` 紀錄實例（含改名、衝突、空白與中文檔名）；⑤`--name-status` 與 `--numstat` 能否同一次輸出及順序；⑥`diff --cached` 在
  還沒有 commit 的 repo、`diff-tree -r --root` 的輸出；⑦未知 hash、`cat-file` 路徑不存在、非 repo、dubious ownership 在 `LC_ALL=C` 下的
  exit code 與 stderr；⑧`wsl.exe -d <distro> --exec env LC_ALL=C git -C <不存在的路徑> status` 會報錯（不靜默退回）；⑨以 `git fast-import`
  建 260 個 commit 的耗時；⑩「讀取狀態不寫入 index」：touch 已追蹤檔後帶與不帶 `--no-optional-locks` 各跑一次 `status`，比對 `.git/index`
  的修改時間。任何一項與 design 不符時，停下回報控制端（控制端以 `/opsx:update` 修 design／spec 後再往下）；驗收＝probe 文件逐項有指令與
  輸出，`markdownlint-cli2` 0 issues

## 2. `cockpit-git` crate

- [x] 2.1 新 crate 骨架與封閉查詢（design D1）：workspace 加入 `cockpit-git`（依賴 `tokio`（process、io-util、time、sync）、`serde`）；
  新增 `docs/adr/0007-cockpit-git-crate.md`（design D1 的依賴邊界、改寫 5a「不執行程式」的範圍、D2 的執行方式與替代方案）；值型別 `Oid`、
  `Side`、`RepoPath`、`RefName` 與其驗證；sealed `GitQuery` 與 D4 表格中全部查詢型別的引數組裝；`GitTarget`（Native／Wsl）與由主機路徑
  選目標的純函式；驗收＝單元測試逐一斷言每個查詢在兩種目標下的**完整 argv**（含固定前綴、`--exec env LC_ALL=C git -C`、不含 `--cd` 與獨立
  的 `--`＋shell 形式、路徑前有 `--`）、`Oid` 拒絕大寫／長度不符／`-` 開頭、`RepoPath` 拒絕 5a 的全部非法片段、UNC 形式判斷
  （`\\wsl.localhost\`、`\\wsl$\`、大小寫、非 WSL UNC、非法 distro 名稱）；`compile_fail` doctest 證明 crate 外無法實作 `GitQuery`；
  `cargo tree -p cockpit-core` 不含 `cockpit-git`
- [x] 2.2 執行器（design D3）：子程序啟動（`CREATE_NO_WINDOW`、`kill_on_drop`、stdin null、Native 設 `LC_ALL=C`）、同時 4 支上限、含排隊的
  10 秒逾時並終止子程序、stdout 邊讀邊數的上限與可截斷查詢的前段解析、stderr 8 KiB 只進日誌、錯誤分類（無法啟動 → `Unavailable`、
  dubious ownership → `Untrusted`、逾時、其他非零）；逾時與上限以測試專用的假程式驗證（crate 內測試用建構子指定程式路徑，假程式以 cargo
  example 或 test helper 實作，不依賴 shell）；驗收＝測試涵蓋：逾時後子程序已結束、超過上限時終止並回 `TooLarge` 或截斷結果、第 5 支查詢
  等前面結束才啟動、找不到程式 → `Unavailable`、`LC_ALL=C` 下的 dubious ownership stderr → `Untrusted`，全綠
- [x] 2.3 解析器：`Status`（含 `#` 標頭、四種紀錄、改名原路徑、非 UTF-8 路徑計入 `skipped`、截斷在最後完整紀錄）、`Refs`（附註 tag 剝皮、
  略過遠端 `HEAD` symref 與 `refs/stash`）、`Log`、`CommitInfo`、`ChangedFiles`（依 1.2 ⑤ 的結論）；驗收＝以 1.2 記錄的真實輸出為 fixture
  的單元測試涵蓋 spec「各組變更」「合併衝突」「還沒有 commit 的 repo」「分支、遠端與 tag」「兩個 commit 之間」的解析部分，全綠
- [x] 2.4 單檔 diff（design D7）：unified patch 解析（標頭、權限、改名、二進位、子模組、`\ No newline at end of file`）、左右並排對齊、
  `gap` 行數、`version` 雜湊、未追蹤檔案的全新增列與二進位判斷（純函式，讀檔由呼叫端傳入位元組）；驗收＝單元測試涵蓋 spec「左右配對」
  「未追蹤檔案」「二進位檔」與刪除多於新增、新增多於刪除、檔案開頭／結尾的變更、無結尾換行、只有權限改變，全綠
- [x] 2.5 Graph 排版（design D8）：車道演算法、6 色配置、`lines` 輸出；驗收＝單元測試涵蓋線性歷史、spec「分支與合併的排版」、octopus
  merge、多個根 commit、parent 不在已載入範圍、以及**前綴穩定**的性質測試（對多個隨機產生的 DAG，逐一比對「前 N 列單獨排版」與「整份排版的
  前 N 列」完全相同），全綠
- [x] 2.6 真實 git 的整合測試（`cockpit-git/tests/`）：以 Windows git 在暫存目錄建 repo，跑全部查詢並比對結果；spec「repo 設定的外部程式
  不被執行」（標記檔腳本以 Windows 可執行的形式撰寫）與「讀取狀態不寫入 index」；另加 `#[ignore]` 的 WSL 版（環境變數
  `COCKPIT_GIT_TEST_WSL_DISTRO` 指定 distro，於 WSL `/tmp` 建暫存 repo 並在結束時刪除），涵蓋 spec「WSL repo 在 WSL 內執行且引數不經
  shell」；找不到 git 時測試失敗而非略過（design 風險）；驗收＝`cargo test -p cockpit-git` 全綠，並手動跑一次 WSL 版
  （`cargo test -p cockpit-git -- --ignored`）把輸出貼進回報

## 3. `cockpit` 後端與預覽

- [x] 3.1 `ui_preview` 的 git fixture（design D10）：以本機 git 建立真正的 repo 取代空 `.git/`，內容涵蓋 design D10 的清單與 260 個以上 commit
  （`fast-import`）、固定作者與時間；合併衝突 repo 的放法依 design D10；找不到 git 時啟動失敗並印出原因；驗收＝連續啟動兩次得到相同的
  HEAD hash、`cargo test -p cockpit --example ui_preview` 全綠、既有腳本全綠（`files-check.js` 若因 fixture 內容變動而失敗，只能改 fixture
  帶來的預期值，並在 commit 訊息列出）
- [x] 3.2 git 端點（一）：路由前綴、共同規則（design D6：參數先驗證、`authorize_root`、`not_git`、目標選擇、錯誤分類與本體、標頭、405）、
  `status`、`refs`、`log`（含 `ref` 比對、`tip`、`offset`／`limit` 驗證、只回傳要求的列）、`commit`、`changes`、`merge-base`；驗收＝新測試檔
  `cockpit/tests/git_endpoint.rs`（暫存 repo＋假 runtime 狀態，同 `files_endpoint.rs` 的做法）涵蓋 spec「git 端點的共同規則」全部 scenario、
  「狀態端點」「refs 端點」「commit 清單端點」「commit 詳情端點」「變更檔案清單端點」「共同祖先端點」的全部 scenario 與「擁有者不符不繞過」
  （以錯誤分類的單元測試＋端點層的對應測試涵蓋，不在本機製造不同擁有者的 repo），全綠；「沒有啟動任何子程序」以執行器的測試計數器斷言
- [x] 3.3 git 端點（二）：`diff`（含未追蹤檔案經 5a 路徑界限與上限讀取）、`meta`／`blob`／`render`（重用 5a 的原始內容回應、viewer 分類、
  Markdown 渲染）；`cockpit/README.md` 補「git 唯讀讀取」一節（端點、安全邊界、執行方式、需要安裝 git）；驗收＝`git_endpoint.rs` 涵蓋
  spec「單檔 diff 端點」「某版本的檔案內容端點」全部 scenario，`http.rs` 涵蓋 cockpit-dashboard「路由與 content-type」（`/app/git.js` 先以
  空殼檔案內嵌），全綠

## 4. 前端

- [x] 4.1 分頁一般化與本機儲存 `v:2`（design D9）：`files.js` 的分頁改為帶 `kind` 的物件、各 kind 的內容／輪詢／標題交由模組提供、`v:1`
  相容讀取；新增 `/app/git.js` 骨架；驗收＝`files-check.js` 既有段全綠（檔案分頁行為不變），新增並通過 file-review「舊格式照常還原」段
- [x] 4.2 左欄「變更」分頁：三分頁 tablist 與鍵盤操作、分支資訊、四組清單、開啟 diff、Git Graph 按鈕、2 秒輪詢只在目前分頁、捲動與焦點
  保留、空狀態／非 git／截斷／過期標示；驗收＝新驗收腳本 `docs/research/2026-09-28/git-check.js`（寫法同 `files-check.js`，段落名稱加
  capability 前綴）新增並通過 git-review「顯示變更並開啟 diff」「新變更自動出現」「不是目前分頁時不讀取」與 file-review「左欄三個分頁」段，
  `visual-check.js` 依 cockpit-dashboard delta 更新左欄分頁描述的斷言，既有腳本全綠
- [x] 4.3 diff 分頁：左右並排格線、行號、底色（由 `--ok`／`--bad` 推導）、`gap` 列、折行、工具列四個動作、特殊結果文案、輪詢與 `version`
  比對、捲動保留、過期標示；驗收＝`git-check.js` 新增並通過「左右並排呈現」「改檔後更新」「窄視窗不橫向捲動」段，`visual-check.js` 新增並
  通過 cockpit-dashboard「diff 與 Git Graph 不撐破頁面」的 diff 部分與 diff 顏色的對比檢查，既有腳本全綠
- [x] 4.4 Git Graph 分頁：每列 SVG 繪製、ref 標籤、分批載入與上限提示、分支篩選、搜尋與跳轉、refs 輪詢與「分支已變更」提示、鍵盤選取、
  捲動與選取在重畫下保留；驗收＝`git-check.js` 新增並通過「開啟並分批載入」「搜尋跳轉」「分支變更提示」段，`visual-check.js` 通過
  「diff 與 Git Graph 不撐破頁面」的 Graph 部分，既有腳本全綠
- [x] 4.5 commit 詳情、比較與某版本檔案分頁：展開詳情、複製（hash 與 ref）、parents 跳轉、檔案清單開 diff、「看此版本」、比較基準與
  Ctrl／⌘＋點選、直接比較／自分岔點起、某版本分頁（沿用 `viewers.js`，ctx 改由呼叫端提供網址與相對連結處理，design D9）；驗收＝
  `git-check.js` 新增並通過「看 commit 的變更並開 diff」「比較兩個 commit」「複製 hash」「看舊版規格」「commit 版本不輪詢」段與
  file-review「還原 git 分頁與變更分頁」段，`files-check.js` 的檢視器段仍全綠，既有腳本全綠

## 5. 收斂與驗收

- [x] 5.1 全面驗收：`git-check.js`、`files-check.js`、`visual-check.js`（含完整文字對比段）與其餘既有腳本全跑，並在 1536×1024、1100、700 三種
  寬度各存「變更」分頁、diff 分頁、Git Graph（展開詳情）的 viewport 截圖到 scratch；驗收＝全部段落通過，截圖路徑寫進回報
- [x] 5.2 WSL 真機驗收：以正式服務（`cargo run -p cockpit`）連 WSL runtime，選一個 cwd 在 WSL 既有 repo 內的 pane，確認「變更」分頁、一個 diff、
  Git Graph 與一個 commit 詳情可用，且期間該 repo 的 `.git/index` 修改時間不變；結果寫進 `docs/research/2026-09-28/git-review-probe.md`。
  WSL runtime 不可用時在 ledger 註明未跑與原因；驗收＝probe 文件或 ledger 有紀錄
- [x] 5.3 文件：`docs/research/2026-09-28/git-check.md`（腳本用法與段落代號）、`docs/research/2026-09-23/visual-check.md` 補新段落、
  `CONTEXT.md` 補「diff 分頁」「Git Graph 分頁」「某版本檔案分頁」「變更分頁」詞條；驗收＝`markdownlint-cli2 "**/*.md"` 0 issues
- [x] 5.4 全 gate 與整支分支 review：repo 根跑 `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test --workspace &&
  cargo test -p cockpit --example ui_preview && markdownlint-cli2 "**/*.md" && openspec validate --all`，所有腳本全跑；以
  `codex-companion.mjs adversarial-review --wait --base main` 做整支分支審查，只採信 log 最後「# Codex Adversarial Review」段的結論，
  findings 先重現再處理；驗收＝gate 輸出全綠、腳本全綠、Codex 結論與處理紀錄寫進 SDD ledger
- [x] 5.5 交接：依 `~/.claude/guides/handover-template.md` 整份重寫 `docs/handover.md`（active change、待使用者目視驗收、本段踩過的坑）；
  驗收＝`markdownlint-cli2 "**/*.md"` 0 issues，使用者確認目視驗收結果後才標記完成
