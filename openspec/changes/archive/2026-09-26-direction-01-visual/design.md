# design：direction-01-visual

## Context

動機與範圍見 `proposal.md`；行為見本 change 的兩份 delta spec。這裡只寫實作取捨。

現況（2026-09-23 調查）：

- `render.js` 的 `renderState(state, ui)` 是純函數，`paint()` 以 `replaceChildren` 整頁重畫 `#app`；`#output`（Live Output）
  是 `#app` 的兄弟節點，不在重畫範圍內（設計文件 §8.3「Factory Floor 與互動」；change 3 design D8）。
- 事件在 `#app` 以委派處理、以 `pointerdown` 觸發並 `preventDefault`，重畫後依 `data-action`＋全部 `data-*` 還原焦點
  （change 3）。`channel.js` 每次收到訊息都重新查 `window.onState`。
- `style.css` 只有一組 `:root` 變數、沒有任何 `@media`、`running` 有無條件的持續脈動；`.factory-floor` 的欄數由
  `render.js` 以 inline style 設定。
- `cockpit/tests/http.rs` 對前端**原始碼字串**有斷言：`style.css` 含 `.status-working`、`render.js` 含
  `window.onState = function` 與 `function renderState`、`actions.js` 含 `addEventListener("pointerdown"` 與
  `event.detail === 0`、`index.html` 三個 script 標籤的順序、`render.js` 不得出現「完成」。
- 既有驗收腳本（`factory-floor-check.js`、`live-output-check.js`、`live-output-real-check.js`）除了 selector 與 `data-action`，還以 `getComputedStyle` 比對背景色、opacity 等視覺值。

## Goals / Non-Goals

**Goals:**

- 版面、色彩、狀態呈現達到 delta spec，且 change 1b～3 的互動行為（按鈕、改綁、選取、焦點還原、輸出輪詢）不退化。
- 色彩 token 一次定案，change 5（.md 瀏覽）、change 6（上色）直接沿用。

**Non-Goals:**

- 不重構 `render.js`／`actions.js` 的模組邊界；不改 `pointerdown`＋焦點還原機制。
- 不為 Live Output 上色（change 6）；本 change 只定義 SGR 256 色之後要對應的 token 集合（即本 change 的色票），不做對照表。

## Decisions

### D1. 保留 DOM 身分，只換外框與樣式（使用者選定做法 1）

`data-action` 名稱與既有 class（`.task-node`、`.ff-cell`、`.ff-row-header`、`.pane-row`、`.selected`、`.bind-target`、
`.exited`、`.status-*`、`.task-status-*`、`.error-banner`、`.rebind-banner`、`#output` 與 `.output-*`）一律保留；新增的
結構以新 class 表達。

- 替代方案：重寫畫面並重新命名（結構最乾淨，但 2900 行的 `live-output-check.js` 近乎重寫，change 3 以 32 條裁決穩定下來
  的焦點／點擊邊界要重驗）；導入 CSS 框架（新依賴、表達不了設計文件的框線語彙）。皆不採。
- 後果：驗收腳本主要改視覺斷言；`http.rs` 的原始碼字串斷言多數不必動，實作時逐條核對。

### D2. `#app` 以 `display: contents` 參與外框 grid

外框是 `index.html` 裡一個新的 `.shell` grid 容器，裡面是 `#app` 與 `#output`。`#app` 設 `display: contents`，所以
`renderState` 畫出的各區塊（頂列、左欄、錯誤／改綁提示、Factory Floor、右欄 runtime、最近事件、底列）直接成為
`.shell` 的 grid item，以 `grid-area` 就位；`#output` 同樣是 grid item，佔中下區。

- 為什麼：Live Output 在版面上夾在 Factory Floor 與右欄之間，但在 DOM 上必須留在 `#app` 之外才不被整頁重畫。
  `display: contents` 讓「DOM 歸屬」與「版面位置」分開，`paint()`、事件委派（事件沿 DOM 冒泡，與版面無關）、焦點還原
  都不用改。
- 替代方案：把 `#app` 拆成多個重畫根節點（要改 `paint()`、委派與焦點還原的範圍，正是 D1 想避免的）；把 `#output`
  搬進 `#app` 並在重畫時搬回來（違反「面板 DOM 節點不被換掉」）。皆不採。
- `display: contents` 的已知無障礙問題只發生在本身有語意角色的元素上；`#app` 是無語意的 `div`，不受影響。
- `renderState` 目前回傳單一 `<div class="page">`，`paint()` 以 `appEl.replaceChildren(renderState(...))` 放入；改為回傳
  `DocumentFragment`（`replaceChildren` 會展開 fragment；它不接受陣列），呼叫點維持單一參數。`index.html` 裡 `#app` 的首次占位
  內容（首份投影到達前顯示）也要改成與新外框相容。
- 每個區塊的根元素帶 `data-region`：`topbar`、`projects`、`banner`、`floor`、`runtimes`、`events`、`statusbar`
  （`#output` 另帶 `data-region="output"`）。這是驗收腳本定位區塊的唯一依據，之後改 class 不影響腳本。

### D3. 斷點與捲動

- `≥ 1200px` **且** `≥ 720px` 高：`.shell` 高 `100dvh`、`overflow: hidden`；三欄（左欄約 220px、中欄 `minmax(0, 1fr)`、
  右欄約 300px）；每個區塊 `min-height: 0` 且內部 `overflow: auto`；中欄上下按 Factory Floor 約 55%、Live Output 約
  45% 分配，兩者最小高度都約 240px；錯誤或改綁提示出現時，從 Factory Floor 的份額扣（不動 Live Output 的份額）。
- `≥ 1200px` 但高度 `< 720px`：仍維持三欄，取消固定高度、`.shell` 允許整頁捲動；Factory Floor 最小高度維持 240px；
  Live Output 改為固定高度 `clamp(320px, 50vh, 560px)`、內部捲動（其有效最小高度即下限 320px），避免和 Factory Floor
  互搶剩餘高度。**Factory Floor 另加高度上限**（使用者 2026-09-24 裁決）：約「視窗高度－頂列－提示列－200px」（下限仍是
  240px），超過的內容在 Factory Floor 的內層捲動容器（`.projects`）捲動；沒有這個上限時 Factory Floor 會無限長高，
  把 Live Output 整個推到第一屏外面，這個情形本來就允許整頁捲動，但至少要讓使用者不用先捲過 Factory Floor 才看得到
  Live Output 的開頭。實作上跟 Factory Floor 共用同一個 grid 列的 runtime 清單（右欄）也要加同一個上限，否則這一列的
  auto 高度仍然會被 runtime 清單自己的自然高度撐高，Factory Floor 的上限形同虛設（task 2.1 fix round 1 實測過）。
- `760–1199px`：兩欄（左欄＋中欄），runtime 與最近事件移到中欄下方；取消固定高度、允許整頁捲動；Live Output 同上
  改為固定高度 `clamp(320px, 50vh, 560px)`、內部捲動。
- `< 760px`：單欄、取消固定高度，整頁捲動。
- 長字串：名稱、標題、cwd 用 `overflow-wrap: anywhere` 或單行省略，並把完整內容放進 `title` 屬性。
- Factory Floor 欄數仍由 `render.js` 以 inline `grid-template-columns` 設定（D1：不搬這段邏輯），外層區塊負責橫向捲動；
  橫向捲動時列首（`position: sticky; left: 0`）與 stage 欄首（`position: sticky; top: 0`）固定不動，兩者底色不透明
  （`--bg-base`），左上角格兩個方向都 sticky，列首右緣加 1px `--line`，讓人看出它浮在內容上方。

### D4. 色彩 token 與狀態對應

`:root` 以用途命名 10 個 token（`--bg-deep`、`--bg-base`、`--surface`、`--text`、`--text-dim`、`--accent`、`--line`、
`--ok`、`--warn`、`--bad`），值取設計文件色彩表；其他顏色一律由這 10 個推導（例如柔光用 `--accent` 加透明度）。
2026-09-23 實算：所有文字色在三種底色上的對比最低為 `--bad` 對 `--surface` 的 5.67:1；`--line` 只有約 1.6:1，所以
必要圖形（未知狀態的虛線外框等）改用 `--text-dim`。

| 對象 | 狀態 → 顏色與符號 |
|---|---|
| task | `pending` `--text-dim` ○、`ready` `--text` ♢、`running` `--accent` ▶（2px 外框＋靜止柔光）、`blocked` `--warn` ‖、`failed` `--bad` ✕、`completed` `--ok` ✓、未知 `--text-dim` ?（虛線外框） |
| agent | `working` `--accent` 實心點、`idle` `--text-dim`、`blocked` `--warn`、`done` `--text` 加 `--accent` 空心點、未知 `--text-dim` |
| 連線 | `connected` `--ok` 實心圓、`connecting` `--warn` 空心圓、`disconnected` `--bad` 叉（三種形狀皆 CSS 畫，顏色不變；使用者 2026-09-24 裁決：取代純顏色區分，灰階下 `--ok`／`--warn` 亮度比僅 1.05:1；頂列 runtime 燈號與底列通道狀態共用同一套形狀） |

符號放在 DOM 文字裡（`aria-hidden="true"` 的 `span`），不用 CSS `content`，讓「只讀文字也分得出狀態」可以直接驗。
狀態文字沿用投影的英文 `status` 字串；左欄的數量也用 `status` 字串，以符合「不出現『完成』」與 `http.rs` 的禁字測試。

**冰青的形狀分工**：冰青（`--accent`）只標「此刻正在工作」，其餘元素改用形狀或亮度差區分，不與 running 搶份量。

- `running`（task）：唯一同時有 2px 外框**加**柔光的元素；柔光只給 running，其他地方一律不加 `box-shadow` 發光。
- `working`（pane）：只有實心點和狀態文字用冰青，整列不加外框。
- 選定（Project、pane）：底色改為 `--surface`，左緣加 2px 冰青條，不畫四邊外框——和 running 的四邊框加發光在形狀上分開。
- 焦點：2px 冰青外框，外推 `outline-offset: 2px`，和元素本體之間留一道底色縫；已選定又正聚焦時，左緣條與外推框都看得到，
  彼此不會蓋掉。
- 改綁提示：只有左緣條和文字用冰青，底色維持 `--surface`，不做冰青實底。
- 刻度、切角補線：用 `--line`（純裝飾）或 `--text-dim`，**不用冰青**。例外（使用者 2026-09-25 裁決）：面板本身取得
  鍵盤焦點時（例如取消選取後焦點退回 `#output`），切角補線是焦點框的一段，改用 `--accent`、與焦點框同寬，讓焦點框
  在切角處不斷開。

**動作按鈕**：Task 節點與列首上的所有動作按鈕（「推進」「Completed」「Failed」「看輸出」「改綁」「取消改綁」
「綁定到這裡」「取消」等）一律用同一種第三層級樣式：透明底、1px `--text-dim` 外框（`--line` 只有 1.6:1，看不出是
按鈕）、`--text-dim` 文字、12px；hover 時文字和外框改成 `--text`。**按鈕不得使用任何狀態色**，冰青也不行。按鈕固定
排在節點底部，和標題、狀態之間留 8px，讓「標題 → 狀態 → 操作」的讀序由上往下固定；節點最小寬度 140px 放不下三顆
按鈕時允許換行，不縮小字級。

**狀態符號**：符號 `span` 固定寬度（`1.25em`、`text-align: center`、`display: inline-block`），和狀態文字用同一個
顏色；▶（U+25B6）後面接 U+FE0E（文字呈現選擇字元），避免被部分字型畫成彩色 emoji，3.2 截圖確認後如仍不穩再換符號並
回頭更新本節。agent 的實心點與空心點改用 CSS 畫的 8px 圓（`border-radius: 50%`，空心用 2px 邊框），不用 `●`／`○`
字元，避免和 task 的 `pending` `○` 混淆。

3.2 截圖確認的結果：

- `ready` 原本用 ◇（U+25C7），它在 Segoe UI 12px 下墨跡只有約 7×6px，其他符號約 10×9px，看起來小一半，
  改用 ♢（U+2662，約 8×9px，非 emoji 碼位）。
- ▶＋U+FE0E 畫出來是單色文字字形，不換。

**連線配色也適用底列通道狀態**：底列的通道狀態沿用本節「連線」列的三色與燈號形狀；文字前面加一個獨立的 `span` 標籤
「cockpit 服務」（`#channel-status` 本身文字不變，腳本不受影響），和頂列的 runtime 連線燈號（指 cockpit 到 HERDR 的
連線）區分開，避免同一個「connected」字樣讓使用者分不出是哪一段連線斷了。

**通道斷線時頂列燈號降級為「最後已知」（使用者 2026-09-24 裁決）**：頁面與 cockpit 服務的通道（即 `#channel-status`
所示的通道）不是 `connected` 時，頁面已經不知道各 runtime 的真實連線狀態，不能繼續用 `connected` 對應的成功色顯示；此時
頂列每個 runtime 燈號的形狀仍維持該 runtime 最後已知連線狀態對應的形狀（實心圓／空心圓／叉），但顏色一律改為
`--text-dim`，狀態文字前加「最後已知」。通道恢復 `connected` 後，燈號才依 runtime 目前實際的連線狀態還原顏色。底列
通道狀態本身不受此規則影響——它顯示的就是這條通道自己的狀態。

### D5. 不使用動畫與過渡

整頁重畫每次都換掉節點，任何 `animation` 或進場效果都會每次重播，所以 `#app` 內**完全不用 `animation`**，滑過與聚焦也
**不加 `transition`**（瞬間切換）。這樣「減少動態」自然成立；樣式表仍加一段 `prefers-reduced-motion: reduce` 把
`animation`／`transition` 歸零，防止日後有人加回來。`running` 的強調只用靜止的外框與 `box-shadow`。

- 按下態：`actions.js` 在 `pointerdown` 上 `preventDefault`，瀏覽器預設的 `:active` 與點擊聚焦可能不生效（change 3 未驗），
  所以不設計依賴 `:active` 的按下態；回饋靠 `:hover` 變亮與 `:focus-visible` 的 2px `--accent` 外框。

### D6. Project 選取是 UI 狀態，比照 `select-pane`

`actions.js` 的 UI 狀態加 `selectedProject`，新操作 `data-action="select-project"`＋`data-project`。它和 `select-pane`
一樣**不是「畫面操作」**：不遞增 `latestOp`、不清 `ui.error`（handover §4：否則進行中的寫入會變 stale、其後的失敗被吞掉）。
`renderState` 依 `selectedProject` 找 Project，找不到（尚未選或已不在投影中）就用第一個。左欄項目是 `button`，帶
`data-action`／`data-project`，焦點還原自動涵蓋。

- 替代方案：用 URL hash 記住選取（重新整理後保留）。spec 要求「重新整理後回到預設」與 pane 選取一致，不採。

### D7. Live Output 面板常駐

`output.js` 的面板骨架一開始就可見：沒有選取時顯示空狀態文字、隱藏「取消選取」與內容區；選取後才顯示標題、內容與
「取消選取」。「取消選取」回到空狀態。`.is-open` 保留為「有選取」的意義（減少腳本改動）。

**空狀態文案**：「還沒選 pane。點 runtime 清單裡的任一列，或按 Factory Floor 列首的「看輸出」。」不提位置（760–1199
與 < 760 時 runtime 清單不在右側），直接用畫面上的按鈕名稱。

**過期標示改掉 opacity**：現行 `.is-stale` 以降低 opacity 表示過期，會讓文字對比掉到 4.5:1 以下，違反「Direction 01
視覺語彙」。改為：內容文字改用 `--text-dim`（對 `--bg-deep` 9.04:1）、面板左緣加 `--warn` 色條，並在標題列顯示「過期」
文字；原因訊息用 `--warn`。`live-output-check.js` 中比對 opacity 的斷言改為比對這些計算值。

### D8. 面板裝飾（僅 Factory Floor 與 Live Output）

只有兩個有主題意義的面板有裝飾，其餘面板不切角、不加刻度：

- **Factory Floor**（平行工作線）：左上／右下 10px 切角（`clip-path`）；標題列右側不再放刻度。**刻度改畫在每個
  stage 欄首的上緣，用 DOM 元素實作，跟著 stage 欄一起橫向捲動（使用者 2026-09-24 裁決，取代原本「標題列右側偽
  元素」做法）**：Factory Floor 橫向捲動時（見 D3）stage 欄會動、標題列不會，刻度畫在標題列上會隨捲動與欄位錯開，
  畫在 stage 欄首則永遠對齊；另外 task 2.2 已把標題列切角用的兩個偽元素拿去畫切角補線，沒有餘裕再疊一份刻度。刻度
  **帶資訊**：每個 stage 一格，與下方欄位對齊；有 running task 的 stage，該格刻度較長且用 `--text`（不用冰青，見
  D4「冰青的形狀分工」），成為一條看得出「工作進行到哪幾站」的迷你尺規。
- **Live Output**（觀察窗）：只有切角，不加刻度；底色用 `--bg-deep`，像一扇嵌進面板的窗。
- **左欄與右欄**：不切角、不加刻度，只用 1px `--line` 框線加底色層次表現邊界。

`clip-path` 會切掉斜角處的邊框，斜角用偽元素補一條 1px `--line` 線。Factory Floor 的刻度是 stage 欄首上緣的 DOM
元素、不是偽元素，接 stage 資料判斷該格是否有 running task；刻度本身也用 `--line` 或 `--text-dim`，不用冰青。內層
元件不重複切角；面板內距至少 12px，避免切到子元素的焦點外框與 `running` 柔光。

### D9. 驗收做法

- **既有腳本**（對 `ui_preview` 的六支，清單見 `tasks.md` 通則）跟著新 DOM 與視覺改：色彩斷言改比對 token 值；面板「沒選取時
  不存在」改為「顯示空狀態」；`live-output-check.js` 的 opacity 斷言依 D7 改寫；`factory-floor-check.js` 的逐字 HTML 字串斷言
  改為以 DOM 結構判斷。**測試手法被版面推翻的兩處要改寫、不是改斷言**：`live-output-check.js` 以 `window.scrollTo` 捲動整頁
  來製造「面板遮住下方內容」與「聚焦元素被捲出視窗」——桌面寬度下頁面不再捲動，前者改為 spec「面板打開時仍可操作頁面下方的
  內容」的命中測試，後者改為捲動 pane 列所在區塊（`data-region="runtimes"`）的內部容器，並先斷言目標確實被捲出可視範圍，
  避免變成恆真。WSL 真機腳本 `live-output-real-check.js` 的 opacity 斷言同步改寫。
- **新增 `docs/research/2026-09-23/visual-check.js`**（headless Chrome＋CDP，沿用既有腳本的啟動與收尾寫法），對 `ui_preview`
  逐條驗 delta spec 的 scenario：版面與三種寬度、Project 切換與焦點、對比（逐一走訪含文字的元素，以計算後的文字色與最近一層
  不透明背景計算）、無動畫與減少動態（CDP `Emulation.setEmulatedMedia`）、無外部請求、`done` 不是成功色。
- **邊界資料用注入，不改 `ui_preview`**：`channel.js` 每則訊息都重新查 `window.onState`，所以腳本可以先保存原函數、把
  `window.onState` 換成空函數擋掉後續推送，再以原函數畫出特製投影（0 個 Project、10 個 stages、200 字名稱、未知
  status）。`ui_preview` 共用的 fixture 已有兩個 Project（`cockpit`、`p`），真實推送下的切換驗收直接用它，不改 fixture。
- 截圖：驗收腳本在 1536×1024、1100、700 三種寬度存截圖到 scratch，交給使用者目視驗收（不進 repo）。
- 前端資源是 `include_str!` 編進執行檔：每次改 `cockpit/assets/` 後要重新 build `ui_preview` 才跑腳本（handover §4）。

### D10. 字級階層

主標題＝目前選定 Project 的 `name`（使用者 2026-09-24 裁決：讀成「儀表板」語意，最該被看見的是 Project，不是產品名）；
頂列的產品名降為與面板標題同級。字級收斂成四階（20／14／13／12px），字重只用 400 和 600：

| 用途 | 字級／字重 | 顏色 |
|---|---|---|
| Project 名（主標題） | 20/600 | `--text` |
| 面板標題（含降級後的產品名） | 14/600 | `--text` |
| 內容（表格或列表密集處可用 13） | 14/400 | `--text` |
| 輔助資訊與時間（含等寬字） | 12/400 | `--text-dim` |

中文不低於 12px；行高：中文內文 1.5、單行標籤 1.3、Live Output 1.4。這四階寫成 `--fs-*` token 放在 task 2.2 的
`:root` 區塊，之後各 task 只准取用這些 token，不得另開字級。

### D11. 字體堆疊與文案

- **無襯線堆疊**明列 Windows 中文 UI 字型：`-apple-system, "Segoe UI", "Microsoft JhengHei UI", sans-serif`
  （`Microsoft JhengHei UI` 已於本機 `C:\Windows\Fonts` 查證存在）。
- **等寬堆疊**：`"Cascadia Mono", Consolas, "Microsoft JhengHei UI", monospace`（`Cascadia Mono` 同樣已於本機
  `C:\Windows\Fonts` 查證存在）。等寬字只用在時間、id、數值與 Live Output 這四類，狀態文字、stage 名稱、按鈕一律用
  無襯線，避免整個畫面變成終端機風格。無襯線文字裡的數字（左欄計數）加 `font-variant-numeric: tabular-nums`。
- **右欄層次**：右欄 runtimes 區塊只有一層框（1px `--line`，跟左欄、最近事件一致），runtime 之間以上方 1px
  `--line` 分隔線區隔、不再各自畫四邊框（使用者 2026-09-25 裁決，取代原本的「runtime 卡只有本身一層框」）；
  workspace 用一行標題列（label、`#number`、彙總狀態）加上方 1px `--line` 分隔；tab 用縮排 8px 的次標題（`tab 1`、狀態），`--text-dim` 12px，不加框。pane 列排成兩行（CSS grid 定位、不改
  DOM 順序）：第一行狀態符號＋狀態文字、agent、pane id（等寬、靠右）；第二行標題與 cwd，`--text-dim` 並單行省略，
  完整內容放 `title`。連線明細用兩欄定義列表（左 `--text-dim` 標籤、右等寬值），不用中點串接。最近事件的 `at` 只
  顯示時間部分（例如 `01:59:30Z`，保留 `Z`、不轉時區），完整字串放 `title`。
- **左欄計數 chip**：數量為 0 的 status 不顯示；固定依「需要注意」的程度排序：`failed`、`blocked`、`running`、
  `ready`、`pending`、`completed`、未知；每項寫成「符號＋`status`＋數字」的小 chip、用該狀態的顏色，chip 之間用
  間距分隔，不用中點串接；`warnings` 用 `--warn`，放在名稱同一行右側。
- **沒有 Project 的空狀態文案**：Factory Floor 區域寫「在 `cockpit.toml` 加入 `[[project]]` 區段即可在這裡看到
  Factory Floor，加入後需要重啟 cockpit」（設定只在啟動時讀，見 `cockpit/src/config.rs:177` 的 `load`，沒有 watch，
  已查證）。語氣直述、指向動作、不道歉。

## Risks / Trade-offs

- [`display: contents` 下 grid item 由 `renderState` 的輸出結構決定，某區塊多包一層就會跑版] → `renderState` 的頂層輸出固定為
  各區塊的平鋪清單，`visual-check.js` 驗每個區塊的 grid 位置。
- [固定高度版面下，資料多時中欄上下兩區互相擠壓] → Factory Floor 與 Live Output 各給最小高度，其餘按比例分配；兩區各自內部捲動。
- [既有腳本改動量大，改腳本時不小心放寬斷言] → 腳本修改與產品修改分開 commit；review 時對照每條被改的斷言與 spec scenario。
- [Codex 在唯讀沙箱只能靜態推導] → findings 一律先以腳本或測試重現才採信（AGENTS.md）。
- [`pointerdown` 的 `preventDefault` 對 `:focus-visible` 與 `:hover` 的影響未驗] → `visual-check.js` 以鍵盤 Tab 驗焦點外框；
  滑鼠點擊後是否出現焦點外框不列為要求。

## Migration Plan

純前端資源，隨執行檔發佈；回復方式是還原這個 change 的 commit。沒有資料或設定遷移。
