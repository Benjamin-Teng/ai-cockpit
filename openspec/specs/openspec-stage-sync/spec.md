# openspec-stage-sync Specification

## Purpose

定義 Cockpit 如何唯讀地偵測 Repo Project 每個 pane 所在 worktree 的 OpenSpec 進度（對應到哪個 change、判定規劃／實作／審查／完成階段），
並據以自動移動 Factory Floor 上的卡片。涵蓋 change 對應與階段判定規則、輪詢與 WSL 防護、自動與手動的優先權規則，以及卡片同步狀態的持久化。
決策背景見 change `openspec-stage-sync` 的 proposal 與 design。

## Requirements

### Requirement: 偵測結果與 change 對應規則

系統必須為 Repo Project 的每個 pane，依它所在 worktree 的目前分支與該 worktree 的 `openspec/changes/` 目錄，判定該 pane 正在做的 OpenSpec change，
結果稱為「偵測結果」，內容為 change 名稱、OpenSpec 階段與勾選進度（`checked`、`total`，見「OpenSpec 階段判定」），或「無結果」。
同一個 worktree 的所有 pane 得到相同的偵測結果。「進行中的 change」指 `openspec/changes/` 下的直接子目錄，`archive` 目錄不算，非目錄的項目不算。
change 的對應依下列順序，取第一個成立者：

1. 目前分支名稱的最後一段（最後一個 `/` 之後；沒有 `/` 時為整個名稱）完全等於某個進行中的 change 名稱 → 該 change。
2. 否則，`openspec/changes/archive/` 下有目錄名稱為 `YYYY-MM-DD-<該最後一段>`（`YYYY-MM-DD` 為四位數年、兩位數月日，後接 `-` 與完整的最後一段）
   → 該 change，階段為 `complete`；同一個名稱有多個日期的 archive 目錄時，取日期字串最大者（即最新的日期）。
3. 否則，進行中的 change 恰好一個 → 該 change。
4. 否則 → 無結果。

HEAD 為 detached（沒有分支）、或 worktree 沒有 `openspec/changes/` 目錄時，第 1、2 條不成立；沒有 `openspec/changes/` 目錄時結果為無結果。
無結果的 pane 不顯示同步標示、不被自動移動，卡片維持手動操作（見「自動移動規則」）。

#### Scenario: 分支最後一段對上進行中的 change

- **GIVEN** 分支 `feat/foo`，`openspec/changes/` 下有進行中的 `foo` 與 `bar`
- **WHEN** 偵測
- **THEN** 對應 change `foo`

#### Scenario: 分支沒有斜線

- **GIVEN** 分支 `foo`，進行中的 change 有 `foo` 與 `bar`
- **WHEN** 偵測
- **THEN** 對應 change `foo`

#### Scenario: 取最後一段

- **GIVEN** 分支 `user/feat/foo`，進行中的 change 有 `foo` 與 `feat`
- **WHEN** 偵測
- **THEN** 對應 change `foo`（最後一個 `/` 之後的 `foo`，不是 `feat`）

#### Scenario: 對應已 archive 的 change

- **GIVEN** 分支 `feat/foo`，進行中的 change 只有 `bar`，`openspec/changes/archive/2026-10-08-foo/` 存在
- **WHEN** 偵測
- **THEN** 對應 change `foo`，階段為 `complete`（archive 對應優先於「只有一個進行中的 change」的退路）

#### Scenario: 進行中的 change 優先於 archive

- **GIVEN** 分支 `feat/foo`，進行中的 change 有 `foo`，archive 下也有 `2026-10-08-foo`
- **WHEN** 偵測
- **THEN** 對應進行中的 `foo`，階段依 `tasks.md` 判定，不是 `complete`

#### Scenario: archive 目錄名稱必須完整符合

- **GIVEN** 分支 `feat/foo`，進行中的 change 有 `bar` 與 `baz`，archive 下只有 `2026-10-08-foo-bar`、`foo`、`10-08-foo`
- **WHEN** 偵測
- **THEN** 無結果（三個目錄名稱都不是 `YYYY-MM-DD-foo`，進行中的 change 有兩個）

#### Scenario: 退路：只有一個進行中的 change

- **GIVEN** 分支 `main`，進行中的 change 恰好一個 `foo`
- **WHEN** 偵測
- **THEN** 對應 change `foo`

#### Scenario: 分支名稱對不上且有多個進行中的 change

- **GIVEN** 分支 `main`，進行中的 change 有 `foo` 與 `bar`，archive 下沒有 `main`
- **WHEN** 偵測
- **THEN** 無結果

#### Scenario: detached HEAD

- **GIVEN** worktree 為 detached HEAD；情況一進行中的 change 恰好一個 `foo`，情況二有 `foo` 與 `bar`
- **WHEN** 分別偵測
- **THEN** 情況一對應 `foo`，情況二無結果

#### Scenario: 沒有 openspec 目錄

- **GIVEN** worktree 沒有 `openspec/changes/` 目錄
- **WHEN** 偵測
- **THEN** 無結果

#### Scenario: archive 不是 change，檔案不是 change

- **GIVEN** 分支 `feat/archive`，`openspec/changes/` 下有目錄 `archive`、一個檔案 `notes.md`，沒有其他進行中的 change
- **WHEN** 偵測
- **THEN** 無結果（`archive` 目錄與檔案都不算進行中的 change）

#### Scenario: 同一個 worktree 的 pane 結果相同

- **GIVEN** Repo Project 有兩個 pane 都在同一個 worktree，該 worktree 對應 change `foo`
- **WHEN** 偵測
- **THEN** 兩個 pane 的偵測結果相同

#### Scenario: 同名 change 有多個 archive 日期

- **GIVEN** 分支 `feat/foo`，進行中的 change 沒有 `foo`，archive 下有 `2026-09-01-foo`（`tasks.md` 為 3／8）與 `2026-10-08-foo`（`tasks.md` 為 8／8）
- **WHEN** 偵測
- **THEN** 對應 change `foo`，階段為 `complete`，`checked`／`total` 取自 `2026-10-08-foo` 的 `tasks.md`（8／8）

### Requirement: OpenSpec 階段判定

系統必須依對應到的 change 的 `tasks.md` 判定階段與勾選進度：`checked` 為已勾選的項目數，`total` 為所有 checkbox 項目數（已勾選加未勾選）。

- `tasks.md` 不存在、`total` 為 0、或 `checked` 為 0 → `plan`（規劃）。
- `0 < checked < total` → `implement`（實作）。
- `checked == total` 且 `total > 0` → `review`（審查）。
- change 是以 archive 目錄對應到的 → `complete`（完成），`checked`／`total` 取自 archive 目錄裡的 `tasks.md`（不存在時為 0／0）。

checkbox 項目的認法：一行的行首空白之後是 `-`、`*` 或 `+`，接著一個空白，再接 `[ ]`（未勾選）、`[x]` 或 `[X]`（已勾選）。其他寫法
（例如 `1. [x]`、`- []`、`-[x]`、`- [y]`、行首沒有項目符號的 `[x]`）不計入 `checked` 與 `total`。

#### Scenario: 沒有 tasks.md 是規劃

- **GIVEN** change `foo` 存在，沒有 `tasks.md`
- **WHEN** 判定階段
- **THEN** 階段為 `plan`，`checked`／`total` 為 0／0

#### Scenario: 沒有任何勾選是規劃

- **GIVEN** `tasks.md` 有 5 個未勾選項目
- **WHEN** 判定階段
- **THEN** 階段為 `plan`，`checked`／`total` 為 0／5

#### Scenario: 沒有 checkbox 是規劃

- **GIVEN** `tasks.md` 存在但沒有任何 checkbox 項目
- **WHEN** 判定階段
- **THEN** 階段為 `plan`，0／0

#### Scenario: 部分勾選是實作

- **GIVEN** `tasks.md` 有 8 個項目，其中 3 個已勾選
- **WHEN** 判定階段
- **THEN** 階段為 `implement`，3／8

#### Scenario: 全部勾選且尚未 archive 是審查

- **GIVEN** `tasks.md` 有 8 個項目、全部已勾選，change 仍在進行中
- **WHEN** 判定階段
- **THEN** 階段為 `review`，8／8

#### Scenario: 已 archive 是完成

- **GIVEN** change 以 `openspec/changes/archive/2026-10-08-foo/` 對應，其中 `tasks.md` 有 8 個項目、全部已勾選
- **WHEN** 判定階段
- **THEN** 階段為 `complete`，8／8

#### Scenario: checkbox 的認法

- **GIVEN** `tasks.md` 含這幾行：`- [x] a`、縮排兩格的 `* [ ] b`、`+ [X] c`、`1. [x] d`、`- [] e`、`-[x] f`、`- [y] g`、`[x] h`
- **WHEN** 判定階段
- **THEN** `checked` 為 2（`a`、`c`）、`total` 為 3（`a`、`b`、`c`），階段為 `implement`

### Requirement: tasks.md 的讀取上限與無法判斷

系統讀取 `tasks.md` 時，檔案大小上限為 1 MiB（大於 1,048,576 位元組才算超過，剛好 1 MiB 可以讀）。檔案超過上限、讀取發生錯誤、或內容不是合法 UTF-8 時，視為「本輪無法判斷」，
該 worktree 的所有 pane 本輪的偵測結果為無結果；檔案不存在則不是錯誤（見「OpenSpec 階段判定」）。`openspec/changes/` 或
`openspec/changes/archive/` 存在但讀取發生錯誤時，同樣為本輪無法判斷（`archive/` 只在需要判斷 archive 對應時才讀：分支最後一段已對上進行中的 change，或 HEAD 為 detached 時，
不讀 `archive/`，其讀取錯誤不影響結果）；這兩個目錄不存在則不是錯誤（沒有 `openspec/changes/` 的結果為無結果，
沒有 `archive/` 只是 archive 對應不成立）。查詢目前分支失敗（git 回報錯誤，不含 detached HEAD）
時同樣為本輪無法判斷。無法判斷時不得移動任何卡片、不得更新任何 task 已保存的同步狀態；下一輪重新判斷。

#### Scenario: 超過 1 MiB

- **GIVEN** change `foo` 的 `tasks.md` 大小為 2 MiB
- **WHEN** 偵測
- **THEN** 該 worktree 的 pane 為無結果，卡片不移動，task 的 `sync` 為 `null`

#### Scenario: 非 UTF-8

- **GIVEN** `tasks.md` 含無效的 UTF-8 位元組
- **WHEN** 偵測
- **THEN** 無結果，卡片不移動

#### Scenario: 錯誤消失後接續

- **GIVEN** 上一輪因 `tasks.md` 讀取錯誤而無結果，該 task 已保存的 `applied` 為 `foo`／`implement`／3／8
- **WHEN** 下一輪讀取成功，結果仍是 `foo`／`implement`／3／8
- **THEN** 結果與 `applied` 相同，卡片不移動（見「自動移動規則」）

#### Scenario: 查詢分支失敗

- **GIVEN** worktree 的目前分支查詢回報錯誤
- **WHEN** 偵測
- **THEN** 該 worktree 本輪為無結果，不移動卡片

#### Scenario: 剛好 1 MiB 仍可判斷

- **GIVEN** change `foo` 的 `tasks.md` 大小剛好為 1,048,576 位元組且內容合法
- **WHEN** 偵測
- **THEN** 正常判定階段與勾選進度

#### Scenario: changes 或 archive 目錄讀取錯誤

- **GIVEN** 分支最後一段沒有對上進行中的 change，需要判斷 archive 對應，而 `openspec/changes/archive/` 存在但讀取時發生錯誤（例如權限不足）
- **WHEN** 偵測
- **THEN** 該 worktree 本輪為無結果，不移動卡片；`archive/` 不存在時則不是錯誤，只是 archive 對應不成立

#### Scenario: 不需要判斷 archive 對應時，archive 讀取錯誤不影響結果

- **GIVEN** 分支 `feat/foo` 對上進行中的 change `foo`，`openspec/changes/archive/` 存在但讀取時發生錯誤；或 HEAD 為 detached 且進行中的 change 恰好一個
- **WHEN** 偵測
- **THEN** 照常得到偵測結果（前者對應 `foo`，後者對應那唯一的 change），不因 `archive/` 的讀取錯誤而無結果

### Requirement: 偵測的時機與去重

系統必須在 Cockpit 執行期間持續偵測，約每 10 秒一輪：從最新的 Repo Project 投影取出所有 task 的 pane，依所在 worktree 分組（worktree 根目錄見
`repo-projects`「納入 repo 判定的 pane 與更新時機」），每個 worktree 只查詢一次，並把結果套用到該 worktree 的每個 pane。沒有 worktree 根目錄的 pane（見 `repo-projects`）不偵測，
視為沒有偵測結果。
偵測結果與上一輪相同時，不得造成任何狀態變動或投影變動（`version` 不遞增）。手寫 project 的 pane 不偵測。

#### Scenario: 約 10 秒內反映勾選變化

- **GIVEN** pane 的 worktree 對應 change `foo`，`tasks.md` 勾選 3 / 8，投影的 `sync.checked` 為 3
- **WHEN** 在 worktree 內把 `tasks.md` 再勾一項
- **THEN** 約 10 秒內投影的該 task `sync.checked` 變為 4

#### Scenario: 結果沒變不更新投影

- **GIVEN** 連續兩輪偵測結果相同
- **WHEN** 第二輪結束
- **THEN** 投影 `version` 沒有遞增，觀察者沒有收到新的一份

#### Scenario: 同一個 worktree 只查詢一次

- **GIVEN** 同一個 worktree 內有 3 個 pane
- **WHEN** 一輪偵測
- **THEN** 該 worktree 的目前分支只查詢一次，3 個 pane 都得到結果

#### Scenario: 手寫 project 不偵測

- **GIVEN** 手寫 project 的 workstream 綁定到某個 pane
- **WHEN** 偵測
- **THEN** 該 pane 不因手寫 project 而被偵測，手寫 project 的 task 沒有同步資訊

#### Scenario: 沒有 worktree 根目錄的 pane 不偵測

- **GIVEN** 某 Repo Project 的 pane 轉不出 worktree 根目錄
- **WHEN** 偵測
- **THEN** 不對該 pane 執行任何查詢，其 task 的 `sync` 為 `null`

### Requirement: 偵測不得喚醒 WSL 或查詢未連線的 runtime

系統在每一輪偵測中，對每個 worktree 必須先檢查防護：該 worktree 的 pane 所屬 runtime 全都不是 `connected`，或 worktree 位於 WSL（主機路徑以 `\\wsl.localhost\<distro>\`
或 `\\wsl$\<distro>\` 開頭）而該發行版目前沒有在執行時，這個 worktree 本輪完全不查詢（不執行 git、不讀檔、不執行會啟動發行版的指令），因此不得因偵測而
啟動任何 WSL 發行版。防護本身所需的探測（`wsl.exe --list --running`）不在此限，它不會啟動發行版。
同一個 worktree 同時有已連線與未連線 runtime 的 pane 時，照常查詢（防護看的是路徑，不會因此啟動發行版），查詢結果只套用到已連線 runtime 的 pane，
未連線 runtime 的 pane 沿用上一輪的偵測結果。
每個 WSL worktree 必須在查詢前一刻重新探測發行版是否在執行，不沿用同一輪較早的探測結果（把探測與查詢之間的時間窗縮到最短）；
探測逾時或失敗時，本輪其餘 WSL worktree 一律跳過、不再探測（一輪最多只等一次探測上限），下一輪重試。
被跳過的 worktree 本輪不產生新的偵測結果：該 worktree 的 pane 沿用上一輪的偵測結果與已保存的同步狀態，卡片不移動。
Cockpit 剛啟動、被跳過的 worktree 尚無上一輪結果時，其 pane 沒有偵測結果（`sync` 為 `null`），直到防護解除。
發行版恢復執行、runtime 重新連上後，下一輪恢復偵測。

#### Scenario: WSL 發行版沒在執行

- **GIVEN** WSL 發行版 `Ubuntu` 已停止，Repo Project 有一個 pane 的 worktree 在 `\\wsl.localhost\Ubuntu\home\u\app`
- **WHEN** 經過數輪偵測
- **THEN** 沒有對該 worktree 執行任何 git 或檔案讀取，`Ubuntu` 仍是停止狀態；該 pane 的卡片與同步標示維持上一輪的樣子

#### Scenario: runtime 未連線

- **GIVEN** runtime `wsl` 為 `disconnected`，其 pane 的卡片有同步標示
- **WHEN** 偵測
- **THEN** 不查詢該 pane 的 worktree；同步標示與卡片位置不變

#### Scenario: 同一 worktree 混合已連線與未連線的 runtime

- **GIVEN** 同一個 Windows 路徑的 worktree 內，有 runtime `local`（`connected`）的 pane `a` 與 runtime `other`（`disconnected`）的 pane `b`，兩張卡片都有同步標示
- **WHEN** 偵測，且該 worktree 的 `tasks.md` 多勾了一項
- **THEN** 照常查詢該 worktree；`a` 的同步標示更新，`b` 沿用上一輪的偵測結果

#### Scenario: 查詢前重新探測

- **GIVEN** 發行版 `Ubuntu` 在一輪開始時正在執行，但在輪到某個 WSL worktree 查詢前已停止
- **WHEN** 該輪偵測
- **THEN** 該 worktree 在查詢前重新探測後被跳過，沒有執行 git 或讀檔

#### Scenario: 探測失敗時本輪其餘 WSL worktree 跳過

- **GIVEN** 兩個 WSL worktree，`wsl.exe --list --running` 探測逾時或失敗
- **WHEN** 偵測
- **THEN** 兩個 worktree 本輪都被跳過，探測只做一次、不逐個重試；下一輪重新探測；Windows 路徑的 worktree 不受影響

#### Scenario: 恢復後接續偵測

- **GIVEN** 發行版 `Ubuntu` 先前停止、期間使用者在 worktree 內完成了 change 的所有勾選
- **WHEN** 發行版重新執行，下一輪偵測
- **THEN** 該 pane 偵測到最新結果並依「自動移動規則」處理

#### Scenario: Windows 路徑的 worktree 不受 WSL 防護影響

- **GIVEN** runtime `local` 為 `connected`，worktree 在 `D:\work\app`，所有 WSL 發行版都停止
- **WHEN** 偵測
- **THEN** 照常查詢該 worktree

#### Scenario: 剛啟動時被跳過的 worktree 沒有偵測結果

- **GIVEN** Cockpit 剛啟動，WSL 發行版 `Ubuntu` 沒有在執行，Repo Project 有一個 pane 的 worktree 在 `\\wsl.localhost\Ubuntu\home\u\app`
- **WHEN** 經過數輪偵測
- **THEN** 該 pane 沒有偵測結果，task 的 `sync` 為 `null`，直到發行版執行且防護解除後的下一輪才有結果

### Requirement: 偵測全程唯讀

系統的 OpenSpec 進度偵測必須全程唯讀：只讀取 git 的目前分支與 repo 內的目錄與 `tasks.md`，不得在 repo 內建立、修改或刪除任何檔案（含 git 的
`index.lock`），不得送出任何 HERDR 寫入類 method（偵測不新增任何 HERDR method），不得解析 pane 的輸出、不得以 LLM 判斷階段。偵測不依賴 `openspec`
指令存在於使用者的機器上。

#### Scenario: repo 不被改動

- **GIVEN** 一個含 `openspec/changes/foo/tasks.md` 的 repo，記錄所有檔案的內容與修改時間
- **WHEN** 經過數輪偵測
- **THEN** repo 內所有檔案的內容與修改時間不變，沒有出現 `index.lock` 或新檔案

#### Scenario: 沒有送出 HERDR 寫入

- **GIVEN** 以紀錄請求的測試 server 作為 HERDR
- **WHEN** 經過數輪偵測並發生自動移動
- **THEN** server 只收到唯讀 method，沒有收到任何寫入類 method

#### Scenario: 機器上沒有 openspec 指令

- **GIVEN** 執行環境的 PATH 中沒有 `openspec`
- **WHEN** 偵測
- **THEN** 正常判定 change 與階段

### Requirement: 自動移動規則

系統必須在每次收到某個 Repo Project task 的偵測結果時，於進度寫入鎖內依該 task 當下的狀態（不依據可能落後的投影）依序判斷：

1. 偵測結果為無結果 → 不改變任何東西。
2. task 的標記不是 `none`（已標 Completed 或 Failed）→ 不改變任何東西，上次套用的偵測結果（`applied`）也不更新。
3. 偵測結果與該 task 上次套用的偵測結果（`applied`，從未套用過時為無）相同 → 不改變任何東西。
4. 其他情況：把 `applied` 更新為這次的偵測結果，並把 `mode` 設為自動（task 還沒有進度項目時，同時補一筆初始進度：第一個 stage、標記 `none`，
   此時 task 的「目前所在 stage」就是第一個 stage）；再找出對應偵測結果階段的 stage（見 `repo-projects`「Stage 對應 OpenSpec 階段」）：
   找到且與 task 目前所在的 stage 不同時，直接把 task 移到該 stage（可以跨越多站，不受推進／退回逐站的限制，也不受「最後一站不能推進」等規則限制）；
   找到且就是目前所在的 stage，或沒有 stage 對應該階段時，只更新 `applied` 與 `mode`，卡片不動。

偵測結果的比較包含 change 名稱、階段、`checked`、`total` 全部欄位，任一欄位改變即視為不同。自動移動不改變 task 的標記（含 archive 對應的 `complete` 階段也不
自動標 Completed）。自動移動與人工進度操作經同一把寫入鎖依序處理，不互相覆蓋。

#### Scenario: 新卡片第一次偵測就移到對應站

- **GIVEN** Repo Project 的 stages 為 `規劃`、`實作`、`審查`、`完成`，對應依序為 `plan`、`implement`、`review`、`complete`；新 pane 的 task 在 `規劃`、
  標記 `none`、沒有同步狀態
- **WHEN** 該 pane 的偵測結果為 `foo`／`implement`／3／8
- **THEN** task 移到 `實作`，`applied` 為 `foo`／`implement`／3／8，`mode` 為自動；投影的 `sync` 為 `{"change":"foo","phase":"implement","checked":3,"total":8,"mode":"auto"}`

#### Scenario: 勾選數變化但階段不變

- **GIVEN** task 在 `實作`、自動、`applied` 為 `foo`／`implement`／3／8
- **WHEN** 偵測結果變為 `foo`／`implement`／4／8
- **THEN** `applied` 更新為 4／8，task 仍在 `實作`

#### Scenario: 階段改變時跨站移動

- **GIVEN** task 在 `規劃`、自動、`applied` 為 `foo`／`plan`／0／5
- **WHEN** 一輪偵測內結果直接變為 `foo`／`review`／5／5
- **THEN** task 直接移到 `審查`（不經過 `實作`）

#### Scenario: 階段倒退時移回

- **GIVEN** task 在 `審查`、自動、`applied` 為 `foo`／`review`／5／5
- **WHEN** 偵測結果變為 `foo`／`implement`／4／5
- **THEN** task 移回 `實作`

#### Scenario: archive 移到完成站但不標 Completed

- **GIVEN** task 在 `審查`、自動、標記 `none`
- **WHEN** 偵測結果變為 `foo`／`complete`／8／8（change 已 archive）
- **THEN** task 移到對應 `complete` 的 stage，標記仍是 `none`

#### Scenario: 該階段沒有對應的 stage

- **GIVEN** stages 為 `Plan`、`Build`、`Done`，`stage_phases` 為 `["plan",null,"complete"]`；task 在 `Build`
- **WHEN** 偵測結果變為 `foo`／`implement`／3／8
- **THEN** task 仍在 `Build`；`applied` 更新為該結果，`mode` 為自動

#### Scenario: 目前所在 stage 就是對應的 stage

- **GIVEN** task 在 `實作`、`mode` 為手動、`applied` 為 `foo`／`implement`／3／8
- **WHEN** 偵測結果變為 `foo`／`implement`／4／8
- **THEN** task 仍在 `實作`，`applied` 更新為 4／8，`mode` 變為自動

#### Scenario: 無結果不改變任何東西

- **GIVEN** task 在 `實作`、自動、`applied` 為 `foo`／`implement`／3／8
- **WHEN** 偵測結果為無結果（例如分支切到對不上任何 change 的分支）
- **THEN** task 的所在 stage、`applied`、`mode` 都不變；投影的 `sync` 為 `null`

#### Scenario: 結果相同不改變

- **GIVEN** task 在 `審查`、`mode` 為手動、`applied` 為 `foo`／`implement`／3／8（使用者在偵測到實作後手動推進）
- **WHEN** 偵測結果仍是 `foo`／`implement`／3／8
- **THEN** task 仍在 `審查`，`mode` 仍為手動

#### Scenario: 判定以寫入鎖內的當下狀態為準

- **GIVEN** task 的標記為 `none`，偵測結果與 `applied` 不同；偵測結果送出的同時，該 task 被標為 Failed
- **WHEN** 兩者依序在寫入鎖內處理
- **THEN** 若 Failed 標記先生效，task 不被移動、`applied` 不更新；若偵測先生效，task 先被移動，之後標記為 Failed；兩種順序都不遺失任一方的變更

#### Scenario: 找不到對應 stage 時仍建立進度項目

- **GIVEN** stages 為 `Plan`、`Build`、`Done`，`stage_phases` 為 `["plan",null,"complete"]`；新 pane 的 task 還沒有進度項目與同步狀態
- **WHEN** 偵測結果為 `foo`／`implement`／3／8
- **THEN** task 在 `Plan`（第一個 stage）、標記 `none`；狀態檔中該 task 有 `{"stage":"Plan","mark":"none","sync":{"mode":"auto","applied":{...}}}`；
  之後 pane 消失時，進度與 `sync` 一併被清除

#### Scenario: 目標就是第一個 stage

- **GIVEN** 新 pane 的 task 還沒有進度項目，第一個 stage 對應 `plan`
- **WHEN** 偵測結果為 `foo`／`plan`／0／5
- **THEN** task 不移動（已在第一個 stage），但補上進度項目與同步狀態（`mode` 為自動、`applied` 為該結果），狀態檔因此寫出

### Requirement: 已標記的卡片不自動移動

系統必須讓標記為 Completed 或 Failed 的 Repo Project task 不被自動移動，且在標記期間不更新該 task 上次套用的偵測結果（`applied`）；
清除標記後，下一輪偵測結果若與 `applied` 不同，便依「自動移動規則」套用。系統不自動貼 Completed 或 Failed 標記（含 change archive 時），標記一律由使用者操作。

#### Scenario: 標記期間不移動

- **GIVEN** task 在 `實作`、標記 `completed`、`applied` 為 `foo`／`implement`／3／8
- **WHEN** 偵測結果變為 `foo`／`review`／8／8
- **THEN** task 仍在 `實作`，`applied` 仍為 3／8，標記仍是 `completed`；投影的 `sync` 顯示最新的 `foo`／`review`／8／8

#### Scenario: 清除標記後恢復自動

- **GIVEN** 同上，task 仍標記 `completed`，偵測結果為 `foo`／`review`／8／8
- **WHEN** 使用者清除標記，下一輪偵測
- **THEN** 偵測結果與 `applied` 不同，task 移到 `審查`，`applied` 更新為 8／8，`mode` 為自動

#### Scenario: archive 不貼標記

- **GIVEN** task 標記 `none`，change 被 archive，偵測結果變為 `complete`
- **WHEN** 自動移動完成
- **THEN** task 的標記仍是 `none`

### Requirement: 手動操作暫時優先

系統必須在下列「手動入口」成功（被接受並持久化，回 2xx）之後，把該 Repo Project task 的 `mode` 設為手動，`applied` 不變：

- 畫面的人工「推進」「退回」：`POST /api/projects/<project>/tasks/<task>/advance` 與 `.../retreat`。
- agent 的兩個推進端點：`POST /api/agent/projects/<project>/tasks/<task>/advance` 與 `POST /api/agent/advance`。

推進（或退回）與標記手動必須在同一次寫入內完成、只落檔一次：不會出現「已推進並落檔、尚未標記手動」的中間狀態，因此
並發的偵測同步不會在兩者之間把卡片拉回去，也不會遺失推進。agent 的推進端點在寫入鎖內選出 task 後，就地標記該 task。
被拒絕的請求（回 409 等）不改變 `mode`。task 還沒有同步狀態時，手動入口成功後建立同步狀態：`mode` 為手動，`applied` 為該 pane 當下的偵測結果
（沒有偵測結果時為無），使該結果不會再把卡片拉回去。`complete`（標 Completed）、`fail`（標 Failed）、`clear`（清除標記）不改變 `mode`。
手寫 project 的 task 沒有同步狀態，不受影響。手動之後卡片停在手動位置，下一次偵測結果與 `applied` 不同時才依「自動移動規則」恢復為自動。

#### Scenario: 人工推進後卡片留在手動位置

- **GIVEN** task 在 `實作`、自動、`applied` 為 `foo`／`implement`／3／8
- **WHEN** 使用者按「推進」，task 到 `審查`；之後偵測結果仍是 `foo`／`implement`／3／8
- **THEN** task 仍在 `審查`，`mode` 為手動（投影 `sync.mode` 為 `manual`）

#### Scenario: 偵測結果下一次變化時恢復自動

- **GIVEN** 同上，task 在 `審查`、手動
- **WHEN** 使用者在 worktree 內勾選了一項，偵測結果變為 `foo`／`implement`／4／8
- **THEN** task 移回對應 `implement` 的 `實作`，`mode` 為自動，`applied` 為 4／8

#### Scenario: 人工退回同樣標記手動

- **GIVEN** task 在 `審查`、自動
- **WHEN** 使用者按「退回」，task 到 `實作`
- **THEN** `mode` 為手動，`applied` 不變

#### Scenario: agent 的推進端點標記手動

- **GIVEN** task 為自動
- **WHEN** 分別以 `POST /api/agent/projects/<project>/tasks/<task>/advance` 與 `POST /api/agent/advance` 推進（各用一張自動的 task）
- **THEN** 兩者回 204 後，各自 task 的 `mode` 都是手動

#### Scenario: 被拒絕的推進不改變模式

- **GIVEN** task 在最後一個 stage、自動
- **WHEN** 使用者按「推進」，服務回 409
- **THEN** `mode` 仍是自動

#### Scenario: 標記與清除不改變模式

- **GIVEN** task A 為自動、task B 為手動
- **WHEN** 對 A 標 Completed 再清除，對 B 標 Failed 再清除
- **THEN** A 的 `mode` 仍是自動，B 的 `mode` 仍是手動

#### Scenario: 還沒有同步狀態時手動推進

- **GIVEN** 新 pane 的 task 還沒有同步狀態，偵測結果為 `foo`／`implement`／3／8，第一輪套用之前
- **WHEN** 使用者按「推進」
- **THEN** 建立同步狀態：`mode` 為手動，`applied` 為 `foo`／`implement`／3／8；下一輪偵測結果相同，卡片不被拉走

#### Scenario: 沒有偵測結果時手動推進

- **GIVEN** task 還沒有同步狀態，pane 的偵測結果為無結果
- **WHEN** 使用者按「推進」
- **THEN** 建立同步狀態：`mode` 為手動，`applied` 為無；之後偵測結果首次出現時，與 `applied` 不同而依「自動移動規則」套用

#### Scenario: 手寫 project 的推進不產生同步狀態

- **GIVEN** 手寫 project 的 task
- **WHEN** 使用者按「推進」
- **THEN** 該 task 沒有同步狀態，狀態檔中沒有 `sync`

#### Scenario: 推進與同步並發不遺失、不被拉回

- **GIVEN** task 在 `實作`、自動、`applied` 為 `foo`／`implement`／3／8；偵測結果同時變為 `foo`／`implement`／4／8
- **WHEN** 使用者按「推進」的同時，偵測同步送出新結果，兩者在寫入鎖內依序處理
- **THEN** 最終狀態只會是兩種之一：先同步後推進 → task 在 `審查`、手動、`applied` 為 4／8；先推進後同步 → 結果與 `applied` 不同而移回 `實作`、
  自動、`applied` 為 4／8。不會出現「在 `實作` 且手動」而使用者的推進沒有生效的狀態

#### Scenario: 推進只落檔一次

- **GIVEN** task 為自動
- **WHEN** 使用者按「推進」被接受
- **THEN** 該次推進與手動標記在同一次寫檔中提交（狀態檔不會有只含推進、不含手動標記的中間版本）

### Requirement: 階段對應被修改時重新套用

系統必須在 Repo Project 的階段對應因 `PATCH /api/repo-projects/<pid>` 而改變時（「改變」的判準見 `repo-projects`「修改 Repo Project 名稱與 stages」：
每個 phase 的擁有者換了才算，改名但 phase 跟著走或只重排不算），對該 Project 所有 `mode` 為自動的 task，把 `applied` 清為無，使下一輪偵測
依新對應重新套用；`mode` 為手動的 task 不變。未改變階段對應的 `PATCH`（例如只改名稱）不觸發重新套用。

#### Scenario: 自動的卡片依新對應移動

- **GIVEN** stages 為 `A`、`B`、`C`，`stage_phases` 為 `["plan","implement","review"]`；自動 task 在 `B`、偵測結果為 `foo`／`implement`／3／8
- **WHEN** `PATCH` 把對應改成 `["plan","review","implement"]`，之後下一輪偵測
- **THEN** task 移到 `C`，`applied` 為 `foo`／`implement`／3／8，`mode` 為自動

#### Scenario: 手動的卡片不動

- **GIVEN** 同上，但 task 為手動、在 `A`
- **WHEN** 同一個 `PATCH` 與下一輪偵測
- **THEN** task 仍在 `A`，`mode` 仍為手動，`applied` 不變

#### Scenario: 只改名稱不重新套用

- **GIVEN** 自動 task 在 `B`、`applied` 為 `foo`／`implement`／3／8
- **WHEN** `PATCH` 本體只有 `{"name":"x"}`，下一輪偵測結果不變
- **THEN** task 仍在 `B`，`applied` 不變

#### Scenario: 改名且 phase 不變不清 applied

- **GIVEN** 自動 task 的 `applied` 為 `foo`／`implement`／3／8
- **WHEN** `PATCH` 把擁有 `implement` 的 stage 改名，`phase` 仍是 `implement`
- **THEN** 該 task 的 `applied` 不被清除

#### Scenario: 只重排不清 applied

- **GIVEN** 自動 task 的 `applied` 不為無
- **WHEN** `PATCH` 只調整 stages 順序，每個 phase 仍由同一個 stage 擁有
- **THEN** 該 task 的 `applied` 不被清除

### Requirement: 卡片同步狀態的持久化

系統必須把每張 Repo Project task 的同步狀態（`mode`：自動或手動；`applied`：上一次套用的偵測結果或無）隨進度一併持久化（格式見
`pipeline-progress`「狀態檔格式與持久化」），使重啟 Cockpit 後手動優先仍然有效：重啟後第一輪偵測結果若與已保存的 `applied` 相同，卡片不移動、`mode` 不變；
不同時才依「自動移動規則」處理。只有真的套用過偵測結果、或被手動入口標記過的 task 才有同步狀態；從未被這兩者觸及的 task 沒有同步狀態。
有同步狀態的 task 一定有進度項目（建立同步狀態時若還沒有，一併補上初始進度：第一個 stage、標記 `none`），且不會被當成「從未動過」而撤回進度項目，
因為進度項目被撤回會連同 `sync` 一起消失。
同步狀態隨 pane 消失一起清除（見 `repo-projects`「Repo Project 進度的保存與清除」）。每一輪偵測的最新結果本身不持久化，重啟後由偵測重新取得。

#### Scenario: 重啟後手動優先仍有效

- **GIVEN** task 在 `審查`、`mode` 為手動、`applied` 為 `foo`／`implement`／3／8；停止並重新啟動 Cockpit
- **WHEN** 重啟後第一輪偵測結果仍是 `foo`／`implement`／3／8
- **THEN** task 仍在 `審查`，`mode` 為手動

#### Scenario: 重啟期間 OpenSpec 進度已變

- **GIVEN** 同上，Cockpit 關閉期間 worktree 內的勾選變成 4／8
- **WHEN** 重啟後第一輪偵測
- **THEN** 偵測結果與 `applied` 不同，task 移到 `實作`，`mode` 為自動

#### Scenario: 自動模式重啟後不重複移動

- **GIVEN** task 在 `實作`、自動、`applied` 為 `foo`／`implement`／3／8；重啟
- **WHEN** 重啟後偵測結果相同
- **THEN** task 仍在 `實作`，狀態檔內容不變

#### Scenario: 沒被觸及的 task 沒有同步狀態

- **GIVEN** pane 的 task 從未套用過偵測結果（例如一直無結果），也沒有被手動入口操作過
- **WHEN** 讀取狀態檔
- **THEN** 該 task 沒有 `sync` 紀錄

#### Scenario: pane 消失一起清除

- **GIVEN** task 有同步狀態
- **WHEN** 該 pane 被關閉且 runtime 已連線並完成沉降重拿
- **THEN** 狀態檔中該 task 的進度與 `sync` 都被移除

#### Scenario: 有同步狀態的 task 的進度項目不被撤回

- **GIVEN** task 剛被補上初始進度（第一個 stage、標記 `none`）與同步狀態，狀態與沒有任何人工操作的新 task 看起來相同
- **WHEN** 因其他原因寫出狀態檔（含重啟）
- **THEN** 該 task 的進度項目與 `sync` 都仍在狀態檔中
