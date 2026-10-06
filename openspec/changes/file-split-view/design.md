# file-split-view 設計

## Context

動機見 proposal.md「Why」；行為合約見 `specs/file-review/spec.md`。以下只寫本 change 特有的現況與取捨。分頁區的
既有架構來自 change 5a（`openspec/changes/archive/2026-09-28-file-review/design.md` D6）與 5b
（`2026-10-01-git-review`），畫面整體版面見設計文件 §8.3。

現況（皆在 `cockpit/assets/app/files.js`，行號為 2026-10-04 `main` 的位置，實作時以函式名為準）：

- **分頁框架**：
  - `reviewTabs`（各分頁的 kind 物件）與 `currentReviewTabId`（**單一**目前分頁）。
  - `selectTab()` 對離開的分頁呼叫 `kind.deactivate`、對進入的分頁呼叫 `kind.activate`，再更新 `hidden`／`aria-selected`
    並呼叫 `persistTabs()`。
  - Live Output 不在 `reviewTabs` 裡，tab 物件為 null。
- **檔案分頁的輪詢**：
  - 全域只有一條（`poll = {gen, timer, controller}`），`pollMeta()` 以 `currentTab() === ft` 為繼續條件。
  - `FILE_KIND.deactivate` 會呼叫 `abandonRead()` 與 `stopPolling()`。
- **以 `panel.hidden` 判斷可見**：`applyPendingAnchor` 與內容區的 scroll listener 都靠這個（捲動位置在可見時記進
  `ft.scroll`）。
- **檢視器可以有多個實例**：PDF 狀態以 host 為鍵存在 WeakMap，工具列欄位屬於各分頁自己；PDF 已有 `ResizeObserver`，
  在符合寬度模式下會隨容器改寬重排。
- **git 類分頁的輪詢**（`git.js` 的 `diffPoll`、`graphRefsPoll`、`revPoll`）也是單例，前提是「同一時間只有一個分頁可見」。
- **版面**：
  - 分頁區 `#review` 底下直接是分頁列與各 `.review-panel`（Live Output 的面板寫死在 `index.html`，檔案分頁的面板由
    files.js 附加在後）。
  - 切換分頁只靠 `hidden` 屬性，`#review` 本身是 flex column。
- **整頁重畫**（render.js `paint()`）只換 `#app`，`#review` 是它的兄弟節點，不受影響。

## Goals / Non-Goals

**Goals:**

- 把「可見」與「目前」拆成兩個概念：可見集合可以有 1～3 個檔案分頁，目前分頁（焦點）恆為一個。既有依賴「目前分頁」的
  語意（`aria-selected`、鍵盤、持久化的 `current`）保持不變。
- 檔案分頁的生命週期（讀取、輪詢、捲動記錄）改以「是否可見」驅動。
- 沒有並排時，行為與 DOM 結果和現在一致，既有驗收腳本不需改斷言。

**Non-Goals:**

- 不改 `git.js` 的單例輪詢。git 類分頁不會參與並排，所以「最多一個可見」的前提對它們仍成立。
- 不搬動任何面板的 DOM 位置（理由見 D4）。
- 不改後端與 `ui_preview` 的 Rust 程式碼；fixture 只在需要時補檔案。

## Decisions

### D1：狀態模型＝目前分頁＋並排組合＋推導出的可見集合

`currentReviewTabId` 保留原意：目前分頁，也就是焦點。另外新增兩個模組變數：

- `splitTabs`：檔案 kind 物件的陣列，長度為 0 或 2～3，順序就是欄位順序。
- `splitFocus`：並排組合中最後一個焦點分頁。目前分頁離開並排組合時（例如選了 Live Output），靠它在恢復並排時知道焦點欄。

可見集合由單一純函式 `visibleTabs()` 推導，判斷順序如下：

1. 目前分頁在 `splitTabs` 裡，而且視窗寬度至少 760 → 可見集合是 `splitTabs`。
2. 其他情況 → 可見集合是 `[目前分頁]`。Live Output 時是 `[null]`，不含任何檔案分頁。

**為什麼不把 `currentReviewTabId` 改成陣列**：`aria-selected`、roving tabindex、關閉按鈕的 Tab 順序、持久化的 `current`，
都假設只有一個目前分頁。改成陣列要動的地方多，又沒有好處。另外記一份「可見集合」狀態也不划算：它和前兩者重複，容易不同步。
推導函式加上「舊集合／新集合比對」就夠了（D2）。

### D2：生命週期改由「可見集合的變化」驅動

所有會改變可見集合的入口，都在改完狀態後呼叫同一個 `applyVisibility()`。這些入口包括：選定分頁、開檔、按並排鈕、
關閉分頁、還原、視窗寬度跨過 760。`applyVisibility()` 依序做三件事：

1. 用 `visibleTabs()` 算出新集合，和上一次的集合比對。
2. 對**離開**集合的分頁呼叫 `kind.deactivate`，再設 `hidden`。
3. 對**進入**集合的分頁先清掉 `hidden`，再呼叫 `kind.activate`。

兩邊都有的分頁什麼都不做，所以「替換焦點欄時其他欄不重新讀取」由這個結構保證，不靠逐案判斷。

`kind.activate`／`deactivate` 這兩個介面名稱不變，意思從「成為／不再是目前分頁」改成「變成可見／不可見」。git 類分頁
永遠不會進入並排組合，它們可見就等於是目前分頁，所以這兩種意思對它們完全相同，`git.js` 不必修改。

現在的 `selectTab()` 拆成兩步：先依「檔案並排」的規則改狀態（目前分頁、`splitTabs`、`splitFocus`），再呼叫
`applyVisibility()`，最後呼叫 `persistTabs()`。`aria-selected`、目前分頁標示與欄位標記，在 `applyVisibility()` 裡和
`hidden` 一起更新，而且排在 `kind.activate` 之前。原因是 files-check 的契約 C3 要求先更新 `aria-selected`，再開始輪詢
（file-split-view task 2.2 實作時確認）。從檔案樹開檔、點 md 相對連結開檔、
還原，原本就都經過 `selectTab()`，所以替換規則只需寫在一個地方。

改寫時要注意兩個既有細節：

- **Live Output 的回呼包裝**：現行 `selectTab()` 把 `hidden`／`aria-selected` 的更新包在 `liveOutput.tabHidden(apply)`／
  `tabShown(apply)` 的回呼裡，讓 output.js 能在面板藏起來或顯示的前後處理自己的狀態。`applyVisibility()` 在 Live Output
  進出可見集合時，必須維持同樣的包裝與呼叫順序。
- **error 分頁的重試**：`openFile()` 對「已經是目前分頁而且處於 error」的分頁會立即重試讀取。這個條件改成「已經可見而且
  處於 error」。否則並排中點選一個可見、但不是焦點欄的 error 欄時，只會切換焦點、不會重試。

**替代方案**：保留 activate/deactivate 只綁目前分頁，另為並排欄加一組 show/hide 掛鉤。這樣同一個分頁會有兩條生命週期
路徑，切換焦點時容易重複讀取或漏停輪詢，所以不採用。

### D3：檔案輪詢從全域單例改為每個分頁各一條

把 `poll = {gen, timer, controller}` 搬進每個檔案 kind 物件（`ft.poll`），`pollMeta(ft)` 的繼續條件從
`currentTab() === ft` 改成「`ft` 在可見集合中，而且沒有被關閉」。舊回應靠各分頁自己的 `gen` 丟棄。`activate` 時立即查詢
一次，`deactivate` 時中止進行中的請求（`AbortController`）並停掉計時器。

查詢負擔：最多 3 欄，每欄每 2 秒一個只回中繼資料的 `GET`，內容只在 `size`／`modified_ms` 變了才重讀，負擔可以忽略。

### D4：版面用 CSS grid，不搬動面板的 DOM

`#review` 加上 `data-split="2"` 或 `"3"` 時改成 grid。現行樣式的選擇器是 `[data-region="review"]`（flex column、
`gap: 8px`），新規則照用同一個選擇器：

- 分頁列 `grid-column: 1 / -1`。
- 其餘列用 `repeat(n, minmax(0, 1fr))` 等寬切欄，`minmax(0, …)` 讓長內容在欄內捲動，不會撐寬中欄。
- 並排中的各面板用 CSS `order`，依它在 `splitTabs` 的索引決定左右順序；其他面板維持 `hidden`。
- 沒有並排時移除 `data-split`，回到現在的 flex column，DOM 結果與現在相同。

**為什麼不搬 DOM**：把 `<iframe>`（`html` 檢視器）移到別的位置，瀏覽器會重新載入它。PDF 的 canvas 與捲動狀態也會被打斷。
只改 CSS `order` 與 grid，任何面板都不會被移動。

焦點欄外框：在焦點欄的面板加 `data-split-focus` 屬性，用既有的強調色 token 畫外框。不另加顏色，遵守 `cockpit-dashboard`
「Direction 01 視覺語彙」的十個 token。

### D5：窄視窗靠 `matchMedia`，不只靠 CSS

用 `matchMedia("(min-width: 760px)")` 的 `change` 事件觸發 `applyVisibility()`。只用 media query 把多餘的欄藏起來不夠，
因為藏起來的欄仍會被當成可見而繼續輪詢，違反「不可見的分頁不得查詢」。760 和 `style.css` 現有的單欄斷點是同一個數值。

### D6：焦點欄的切換用 pointerdown，不呼叫 focus()

在 `#review` 上以 capture 階段監聽 `pointerdown`。目標在某個並排面板內、而且不是焦點欄時，把目前分頁改成它，再更新
標記與持久化。整個過程不改鍵盤焦點：不呼叫 `focus()`，也不 `preventDefault()`。

理由：使用者是想在那一欄裡捲動、選字或點連結，搶走焦點或擋掉預設動作，都會破壞這些操作。此外，滑鼠操作之後由程式呼叫
`focus()` 會被 Chrome 判成 `:focus-visible`，留下多餘的外框（專案 memory
`programmatic-focus-after-pointer-counts-as-focus-visible`）。

在非焦點欄裡點 md 相對連結時，`pointerdown` 先讓那一欄成為焦點欄，接著的 `click` 開檔，就替換那一欄。這和 spec 的
「替換焦點欄」一致，使用者看到的是「在哪一欄點的連結，就在哪一欄開」。

**行為界定**：`pointerdown` 不分按鍵與裝置，所以右鍵、中鍵與觸控的按下也算 spec 的「按下滑鼠」，同樣切換焦點欄。
只靠鍵盤（Tab 鍵）把焦點移到父文件裡的元素，例如 md 欄的連結，不會切換焦點欄，因為沒有監聽 `focusin`，spec 也只要求滑鼠。
html 欄的 iframe 是例外，見下。

**html 欄（iframe）**：html 檢視器的內容在 sandbox iframe 裡（不允許腳本）。iframe 內的 `pointerdown` 不會冒泡到父文件，
上面的監聽收不到。做法分兩條路，都不需要在 iframe 裡放腳本，也不碰 sandbox：

- **焦點從父文件進入 iframe**：父文件的 `window` 會收到 `blur`，之後 `document.activeElement` 變成那個 `<iframe>`。是非焦點欄
  的 iframe 就切換焦點欄（file-split-view task 3.4 修正第 1 輪）。
- **焦點從一個 iframe 直接移到另一個 iframe**：父文件收不到 `blur`、`focus`、`focusin`、`focusout` 任何事件，只有
  `activeElement` 默默改變（task 3.4 審查以探針實測）。所以只在「並排中，而且 `activeElement` 是某個並排欄的 iframe」時，
  以約 200 ms 的週期輪詢 `activeElement`，條件不成立就立刻停。平常沒有計時器在跑（task 3.4 修正第 2 輪）。

後果與取捨：

- 用 Tab 鍵把焦點移進 iframe 也會切換焦點欄，超出 spec「在該欄內按下滑鼠」的字面。瀏覽器分不出焦點是怎麼進 iframe 的，
  控制端接受這個結果，spec 不改。
- 已知缺口：焦點所在的 iframe 被換掉時（html 檔更新後檢視器換新的 iframe），父文件收不到事件，輪詢會停止，之後在 iframe
  之間直接移動焦點就不會切換焦點欄。在父文件內任意按一下就恢復。不另外修。
- 若這兩條路證明不可行（例如切到別的應用程式時誤切焦點欄），退路是在 spec 為 html 欄列例外。

### D7：並排鈕、Ctrl＋點選與欄位標記

並排鈕是一個 `<button>`，放在 `.review-tab` 裡、關閉鈕左側：

- `aria-pressed` 反映是否在並排組合中。
- 停用條件照 spec「加入」：沒有並排組合，而且目前分頁不是這個分頁以外的檔案分頁。所以沒有並排時，目前分頁自己的並排鈕
  一定是停用狀態。停用時設 `aria-disabled="true"`，並在 `title` 說明原因。用 `aria-disabled` 而不是 `disabled`，滑鼠移上去
  才看得到 `title`。
- 照關閉鈕現有的規則，只有目前分頁的並排鈕 `tabindex="0"`，其餘為 `-1`。鍵盤使用者要加入並排，主要靠方向鍵移到目標分頁
  再按 Ctrl＋Enter（spec「鍵盤加入並排」）。

**Ctrl＋點選**：分頁的選定是由 `wireTablist()` 呼叫 `onActivate(tab)`，而 `wireTablist()` 是左欄分頁列與分頁區共用的。
現在的 click 委派（`files.js` 處理關閉鈕的那一個）不處理選定，`onActivate` 也拿不到 event。做法是讓 `wireTablist()`
把 event 當第二個參數傳給 `onActivate(tab, event)`：左欄的呼叫端忽略它，分頁區的呼叫端讀 `event.ctrlKey`。並排鈕停用
或分頁不是檔案分頁時，照一般選定處理（spec「並排鈕」）。

**Ctrl＋Enter**：放在分頁列既有的 keydown 處理（和 Delete 關閉分頁同一處），而且必須 `preventDefault()`。因為 `<button>`
上的 Enter 本來就會轉成 click，不擋的話會同時多跑一次一般選定。

欄位標記：`.review-tab` 加 `data-split-col="1|2|3"`，以 CSS 畫出數字徽章。另外放兩段帶 `hidden` 屬性的文字：
「並排第 N 欄」與分頁的路徑說明（與 `title` 相同）。再用 `aria-describedby` 依序連到分頁按鈕。

- **用 `hidden`，不用 visually-hidden**：依 accname 1.2，被 `aria-describedby` 直接參照的隱藏節點，仍會計入可及描述。
  visually-hidden 的節點則會以靜態文字留在 `tablist` 裡，閱讀模式會多唸一次。
- **路徑也要連**：有了 `aria-describedby`，瀏覽器就不再拿 `title` 當說明。不一併連上路徑，並排兩個同名檔時，
  螢幕閱讀器使用者會分不出是哪一個檔案。這兩點是 file-split-view task 3.3 審查時定案的。

### D8：持久化：v2 加可選欄位，不升版號

`cockpit.fileTabs` 維持 `v: 2`，加兩個可選欄位：

- `split`：並排組合，存成「寫入時 `tabs` 陣列」的索引。
- `splitFocus`：焦點欄，也是索引。

沒有並排組合時這兩個欄位不寫。

**索引一律指「儲存的 `tabs` 陣列」的位置，不指還原後的位置。** `restoreTabs()` 逐筆還原時，若某筆資料不合法就略過它。
如果用還原後的位置對照，後面每一筆都會往前位移，並排就會靜默指到錯的分頁。所以還原時要建一張「儲存位置 → 還原出的
分頁」對照表，被略過的位置沒有對應。寫入時同理：`serializeTabList()` 會略過 `serialize()` 回傳 null 的分頁，索引要在
過濾之後計算。

載入規則：

- v1 照舊解析。
- v2 沒有 `split` 時視為沒有並排組合。
- `split` 不合法時，忽略並排組合，其他狀態照常還原，並在 console 寫一則警告。不合法包括：長度不在 2～3、索引越界、
  指到被略過的位置、指到非檔案 kind、有重複。
- `splitFocus` 不在並排組合中時，改用第一欄；目前分頁在並排組合中時，`splitFocus` 一律改成目前分頁（spec「分頁還原」）。

**為什麼不升到 v3**：現行 `isStoredState()` 遇到不認得的 `v` 就整份放棄。升版號的話，回滾到舊版會讓使用者重開時一個
分頁都沒有。加可選欄位的話，舊版的形狀檢查不看多出來的欄位，回滾只會失去並排，分頁照常還原。

**為什麼用索引**：`tabs` 陣列本來就是有序的，用索引不必重複存一份檔案身分（runtime、`root_id`、路徑）。

### D9：移出與關閉時焦點欄怎麼決定

移出（再按一次並排鈕或關閉）時，依序處理：

1. 先記下移出前是否在並排中（目前分頁在 `splitTabs` 裡）。
2. 移出的若是 `splitFocus`，它改成右側欄，沒有右側就改成左側欄。
3. 移出後只剩一個分頁時，清空 `splitTabs` 與 `splitFocus`。
4. 只有第 1 步記下「在並排中」時才改目前分頁：改成新的 `splitFocus`；組合已解除的話，改成剩下的那個分頁。不在並排中
   （例如目前是 Live Output）時，目前分頁不動，所以在看 Live Output 時關掉並排成員，不會被切走。

關閉並排組合中的分頁時，先依上面的規則處理，再走既有的關閉流程。原本「關閉目前分頁就改顯示分頁列右側分頁」的規則，
只用在被關閉的分頁不在並排組合中的情況。

## Risks / Trade-offs

- **[每欄變窄，PDF 與長行排版變擠]**：PDF 已有 `ResizeObserver`，符合寬度模式下會重排。`text` 的長行與 Markdown 的
  表格、程式碼區塊原本就在區塊內橫向捲動，並排不會讓頁面出現橫向捲軸。驗收腳本另外測三欄並排加長行（spec「三欄並排不撐破頁面」）。
- **[漏掉某個依賴「目前分頁」的舊判斷]**：`currentTab()` 目前只在 `pollMeta()` 呼叫一處，但 `currentReviewTabId` 還有好幾處
  直接使用，`openFile()` 的 error 重試也隱含「目前分頁＝唯一可見」的前提（D2）。只要漏改一處，並排時就會把非焦點欄誤判為
  不可見。對策是 task 2.1 全面 grep `currentTab()`、`currentReviewTabId`、`panel.hidden`，逐一判斷原意是「焦點」還是「可見」，
  清單寫進 `sdd-ledger.md`。需要「可見」語意的改用 `isVisible(ft)`。
- **[`git.js` 的隱含前提]**：git 類分頁的單例輪詢依賴「同一時間最多一個可見」。只要 `splitTabs` 只收檔案 kind，這個前提
  就成立。在加入並排的入口驗 kind，並用單元或驗收測試保證 Ctrl＋點選 git 類分頁不會形成並排。
- **[`pointerdown` 改焦點欄可能與既有點擊行為衝突]**：例如 PDF 工具列按鈕、md 連結。D6 的做法不攔截預設行為，只多改了
  目前分頁，所以事件照常往下傳。驗收時涵蓋「在非焦點欄點 PDF 下一頁」「在非焦點欄點 md 連結」。
- **[空間取捨]**：每欄最小寬度沒有下限。中等寬度視窗（760～1199）下三欄會很窄，但 spec 定了固定等寬、不另設下限，
  使用者可以自己移出一欄。這是刻意的 KISS 取捨，之後真的不夠用再談可拖曳欄寬（proposal 非目標）。
- **[Tab 順序要與畫面左右一致，改用 `reading-flow` 解決；不支援的瀏覽器退回開啟順序]**：D4 只用 CSS `order` 排欄，不搬動 DOM
  （搬動 iframe 會重新載入），所以預設的 Tab 順序是面板在 DOM 中的順序，也就是開啟順序，可能跟畫面左右相反，牽涉 WCAG 2.4.3
  焦點順序。task 3.7 階段審查實測重現了這個現象，task 4.1 設計審核決定用 CSS `reading-flow: grid-order` 處理：並排時的
  `#review` 是 grid 容器、各欄面板有 `order`，這個值讓 Tab 順序與螢幕閱讀器的瀏覽順序照 `order` 走，不必搬 DOM。
  `split-check.js` 以真實的 Tab 按鍵驗證各欄被走到的順序與欄位編號一致。代價是 `reading-flow` 在 MDN 標為 Experimental，
  只有 Chromium 系（Chrome／Edge 137 起）支援，Firefox 與 Safari 不支援。不支援的瀏覽器會忽略這個屬性，退回開啟順序，
  行為與沒有加這條規則時相同，不會更糟；各欄內容彼此獨立，順序不同不影響操作。
  （查證：MDN `reading-flow` 頁面 <https://developer.mozilla.org/en-US/docs/Web/CSS/Reference/Properties/reading-flow> 與 mdn/browser-compat-data
  `css/properties/reading-flow.json`，2026-10-06。）

## Migration Plan

純前端改動，隨下一個版本的執行檔發布（前端資產內嵌在執行檔中）。不需要資料遷移，版號維持 v2（D8）。回滾到舊版時，
舊程式會忽略 `split`／`splitFocus`，分頁與目前分頁照常還原，只失去並排。實作時要以驗收腳本實際驗證這一點：寫入帶
`split` 的資料後，用 `main` 上的 `isStoredState()`／`restoreTabs()` 邏輯讀取，確認不會整份放棄。實作時比對的是固定的
回滾目標：tag `v0.1.2`，也就是本 change 之前的最後一個發行版。不用 `main` 或 merge-base，是因為本分支 squash 併回 `main`
之後兩者都會變成含並排的版本，斷言就成了自己比自己（task 5.2 最終審查 I-1；做法見 `split-check.md`「回滾相容」）。
