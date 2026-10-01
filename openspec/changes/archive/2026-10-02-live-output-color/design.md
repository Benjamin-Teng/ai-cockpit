# design：live-output-color

## Context

動機見 `proposal.md`「Why」，行為以本 change 的 delta spec 為準。以下只列影響做法的現況：

- **後端**：`cockpit-core/src/runtime.rs` 的 `PaneOutput { format, text, truncated }` 欄位全為 `pub`，`OutputFormat` 只有 `Text`；
  建構點有 `cockpit-herdr/src/runtime.rs`（`read_output`，送 `format: Some(ReadFormat::Text)`、`source: Recent`、
  `lines: Some(max_lines)`，值來自 `cockpit/src/http.rs` 的 `OUTPUT_MAX_LINES` = 200）、`cockpit/examples/ui_preview.rs`（`Html` 模式與
  `growing_output`）、`cockpit/tests/output_endpoint.rs`、`cockpit-core/tests/common/mod.rs`、`cockpit-core/tests/types.rs`。直接讀欄位的
  還有 `cockpit/src/http.rs` 的 `OutputBody`、`cockpit-core/tests/runtime.rs`、`cockpit-herdr/tests/read_output.rs`。
- **HERDR `ansi` 格式**（`docs/research/2026-10-02/ansi-probe.md`）：兩端只見 SGR、樣式不跨行、`\r\n` 換行；去掉控制序列與 `\r` 後
  與 `format=text` 只差行尾空白與結尾換行。背景色、真彩色、游標定位未曾在真機出現，但 HERDR 版本會變，解析器不能假設它們不出現。
- **前端**：`output.js` 的 `writeText` 以 `text === lastRenderedText` 去重、`preEl.textContent = text` 寫入；寫入前量是否貼底、寫入後補捲。
  過期由 `setStale()` 在 `#output` 加 `.is-stale`，CSS 把 `.output-text` 文字改 `--text-dim`。
- **CSS 色票規則**：`style.css` 只允許 10 個色票與由它們推導的 `color-mix`，`docs/research/2026-09-23/visual-check.js` 機器檢查。
- **依賴方向**（ADR-0003）：`cockpit-core` 不依賴任何 `cockpit-*` 與 `herdr-client`；`cockpit` 不直接依賴 `herdr-client`，但依賴
  `cockpit-herdr`。

## Goals / Non-Goals

**Goals:**

- 「片段串接等於 `text`」由型別保證，不靠每個建構點自律。
- 解析器的所有分支都能用 Rust 單元測試覆蓋；前端只做查表與建節點，不解析任何控制序列。
- 沒有樣式的輸出，畫面行為與改版前完全相同（既有驗收腳本以 `textContent` 比對的段落不需修改）。

**Non-Goals:**

- 不重建終端機畫面格（游標定位、清除畫面只丟棄，不模擬）。
- 不在前端提供「切換純文字／彩色」的選項。
- 不為了與 HERDR `format=text` 逐位元組相同而剪行尾空白（見 D3）。

## Decisions

### D1 解析放在 `cockpit-herdr`，用 `vte` crate

`cockpit-herdr` 新增 `ansi` 模組（`pub`，供 `ui_preview` 共用），以 `vte` 0.15 的 `Parser::advance` 走過 HERDR 回傳字串的位元組，
實作 `vte::Perform`：

- `print(c)`：附加到目前樣式的片段；`'\x7f'`（DEL）丟棄——vte 在 Ground 狀態把 DEL 交給 `print`，不是 `execute`。
- `execute(b)`：`0x0A` 附加 `\n`、`0x09` 附加 `\t`，其餘（含 `\r`、其他 C0 與 vte 交來的 C1 U+0080–U+009F）丟棄。
- `csi_dispatch`：只處理 `action == 'm'`、沒有 intermediates、`ignore == false` 的序列（帶 `?`、`>` 等前綴的私有序列 vte 會放進
  intermediates，例如 `ESC[>4;2m`，不是 SGR）；其餘 CSI、`osc_dispatch`、`esc_dispatch`、`hook`／`put`／`unhook` 全部不動作。
  `ignore == true`（參數超過 vte 的 32 個上限，子參數也計入；或 intermediates 超過 2 個）整段略過。
- 結尾停在序列中間的位元組：`advance` 結束後 parser 停在非 Ground 狀態，不會 `print` 任何東西，自然丟棄。
- 已知限制（後端審查 4.3 F1–F3 實證，接受不改）：vte 以位元組驅動、只認 7-bit 引導序列。8-bit C1 引導字元（U+009B CSI、
  U+009D OSC）只丟引導字元本身、其後參數字元會成為可見文字；DCS 內文若含 UTF-8 延續位元組 0x9C 會被當成 ST 提早結束；ESC 後接
  非 ASCII 字元會連帶吃掉下一個字元。HERDR 從畫面格重新輸出、實測只含 7-bit SGR（`docs/research/2026-10-02/ansi-probe.md`），
  這些輸入不會出現；影響僅是畸形輸入多或少幾個可見字元。

SGR 參數以 `Params::iter()` 取得，每個元素是 `&[u16]`：長度 1 是分號寫法的單一參數，長度大於 1 是冒號子參數。
`38`／`48` 的分號寫法要往後吃 `5;N` 或 `2;R;G;B`；冒號寫法在同一個元素內（`[38,5,N]`、`[38,2,R,G,B]`，或帶色彩空間 id 的
`[38,2,id,R,G,B]`；vte 把空子參數 `38:2::R:G:B` 給成 `[38,2,0,R,G,B]`）。分號寫法吃掉幾個參數依 spec「輸出樣式轉換」：
子類型 `5` 吃至多 1 個、`2` 吃至多 3 個、其他只吃子類型本身；不完整或數值超出範圍時只讓該色彩設定不生效，迭代從下一個未消耗的
參數繼續。`38`／`48` 以外帶子參數的元素（例如 `4:3`）整個略過。

選 `vte` 而非：

- **前端 JS 解析**（brainstorming 方案 B，使用者 2026-10-02 選 A）：解析錯誤是「不報錯、只畫錯」，放在測試能力最強的 Rust。
- **自寫狀態機**：OSC 的 BEL／ST 兩種結尾、DCS、被打斷的序列都要處理，vte 是 alacritty 長期使用的實作。
- **`anstyle-parse`**：源自 vte 的同一個狀態機；vte 是上游，預設 feature 只帶 `arrayvec`、`memchr`。

### D2 片段型別放在 `cockpit-core`，`PaneOutput` 以建構函式維持不變式

`cockpit-core/src/runtime.rs` 新增：

- `AnsiColor`：16 個變體，`serde(rename_all = "snake_case")`（`red`、`bright_red`…）。
- `SegmentStyle`：`fg`、`bg: Option<AnsiColor>`，`bold`、`dim`、`italic`、`underline`、`reverse: bool`；`Default` 為全空；
  序列化時 `None` 與 `false` 省略。
- `OutputSegment { text: String, style: SegmentStyle }`，序列化時 `style` 以 `#[serde(flatten)]` 攤平。

`PaneOutput` 的欄位改為私有，新增 `segments`，只能經兩個建構函式產生：

- `PaneOutput::plain(text, truncated)`：無樣式，`segments` 為單一段（`text` 為空時為空陣列）。給測試替身與純文字的假 runtime。
- `PaneOutput::from_segments(segments, truncated)`：丟棄空片段、合併相鄰同樣式片段，`text` 由片段串接而得。

另提供 `format()`、`text()`、`segments()`、`truncated()` 讀取。`OutputFormat` 維持只有 `Text`（語意改為「`text` 欄位是純文字」），
spec 的 `format` 欄位不變。

選這個而非「`text` 與 `segments` 都是 `pub` 欄位」：測試替身與 `ui_preview` 共五個建構點，任何一處手寫不一致的兩個欄位，
前端 `textContent` 就會與 `text` 不符，而且不會報錯。

### D3 `text` 直接取自 ansi 解析結果，不剪行尾空白

實測差異只有行尾空白與結尾換行（`ansi-probe.md` 結論 3）。剪掉行尾空白就得連帶剪掉帶背景色的空白格，diff 行的淡底會被截短；
而畫面上看不出行尾空白，`<pre>` 也不會因為少一個結尾換行而少一行。代價：從面板複製出的文字可能多出行尾空白；以 HERDR
`format=text` 為準比對的工具會看到差異（`live-output-real-check.js` 由 task 檢查）。

### D4 256 色與真彩色以色相歸色，而非找最近的 xterm 調色盤

規則見 spec「輸出樣式轉換」。最近色（RGB 歐氏距離對 xterm 16 色）會把低彩度、但人眼明顯有色的顏色歸成灰：例如 Claude Code
的橘色 `#d77757` 離 `bright_black` 最近，畫面上會變成 `--text-dim`。色相歸色把它歸到 `yellow`（`--warn` 是琥珀色，最接近）。
有彩的歸色只輸出非 `bright_` 名稱（前景兩者對應同一色票，沒有差別）；無彩的會輸出 `black`、`bright_black`、`white`，
因為 `black` 與 `bright_black` 當背景時不同（前者不畫、後者畫 `--text-dim` 淡底）。代價：區間界線是人為的，界線附近的顏色可能和終端機看起來不同；
真機目前不出現真彩色（HERDR 已先換成 16 色），影響有限。

### D5 讀取參數

`read_output` 改送 `format: Some(ReadFormat::Ansi)`，其餘不變。回應的 `format` 欄位不檢查：若某版 HERDR 忽略參數仍回純文字，
解析結果就是一段無樣式的片段，行為等同改版前。

### D6 端點序列化

`OutputBody` 多借用 `segments: &[OutputSegment]`，其他欄位改用讀取函式。片段 JSON 例：`{"text":"error ","fg":"red","bold":true}`。

### D7 前端：查表建節點，片段字串比對去重

`output.js` 的 `writeText` 改為 `writeOutput(payload)`：

- 去重鍵 `JSON.stringify(segments)`，與上次相同就不動 DOM。200 行的字串比對成本可忽略。`showEmptyState()`、
  `resetPanelForSelection()` 原本重設 `lastRenderedText` 的地方改為重設這個鍵。
- 防禦：`payload.segments` 不是陣列時，視同 `[{ text: payload.text }]`（`text` 不是字串時視同空字串，比照現行對 `text` 的處理）；
  陣列中 `text` 不是字串的元素略過；`payload.text` 是字串、而可用片段串接後不等於它時，退回 `[{ text: payload.text }]`，使面板 `textContent` 恆等於回應的 `text`。
- 以 `DocumentFragment` 建內容：沒有任何 class 的片段寫成文字節點；其餘建 `<span>`，`textContent` 寫入，class 由固定對照表產生：
  - 前景：`ansi-fg` 加 `ansi-fg-<色系>`；背景：`ansi-bg` 加 `ansi-bg-<色系>`。色系是去掉 `bright_` 的名稱，`bright_black` 歸 `dim` 色系
    （背景也畫 `--text-dim` 淡底），`black` 前景歸 `dim` 色系、背景不加任何 class。
  - `ansi-bold`、`ansi-dim`、`ansi-italic`、`ansi-underline`、`ansi-reverse`。
  - 片段帶 `reverse` 時不加任何 `ansi-bg*` class：spec 規定反白優先於背景，而 `.ansi-bg.ansi-fg`（兩個 class）的特異度高於
    `.ansi-reverse`，靠 CSS 先後順序擋不住，由 JS 從源頭排除。
  - 對照表沒有的名稱不加 class；不使用 `style` 屬性。
- 最後 `preEl.replaceChildren(fragment)`；貼底判斷與補捲流程不變。

CSS（`style.css`，`.output-text` 規則之後）：

- `.ansi-fg-red { color: var(--bad) }` 等前景規則；`.ansi-dim:not(.ansi-fg) { color: var(--text-dim) }`。
- `.ansi-bg-red { --ansi-bg: var(--bad) }` 等；`.ansi-bg { background: color-mix(in srgb, var(--ansi-bg) 20%, transparent) }`；
  `.ansi-bg.ansi-fg` 改 14%。
- `.ansi-reverse { background: color-mix(in srgb, currentColor 20%, transparent) }`（帶反白的片段不會同時帶 `ansi-bg*`，見上）。
- 過期：`.output-panel.is-stale .output-text span { color: var(--text-dim); background: none; }`，特異度高於所有 `ansi-*` 規則。

14% 的由來：以 sRGB 混色對 `--bg-deep` 試算，20% 時「有色字疊在另一種顏色的淡底」最低 3.86:1（`--bad` 字疊 `--text` 淡底），
14% 時最低 4.69:1；沒有前景色（`--text`、`--text-dim`）的字在 20% 淡底上最低 5.21:1。實際值以 Chrome 計算後顏色驗收（`--graph-lane-4`
是 oklab 混色，與試算略有差異）。

### D8 `ui_preview` 加兩種輸出模式，走真正的解析器

`OutputMode` 新增 `Ansi`（固定樣本：16 種前景、各背景、反白、各樣式、256 色與真彩色、非 SGR 序列、上了色的 HTML／script 字樣）與
`AnsiFlip`（每次讀取在同一段文字的紅、綠之間切換，驗「只有顏色改變也重畫」）。兩者把 ansi 字串交給 `cockpit_herdr::ansi` 解析，
不手寫片段；既有模式（`Html` 與 `growing_output` 產生的各模式）改用 `PaneOutput::plain`。`Ansi` 樣本要涵蓋 spec「對比」
scenario 的全部組合：「無前景＋7 種前景色票」×「無背景＋7 種背景色票」，加上每種前景色票的反白，以及 `black` 背景。

### D9 審查

使用者 2026-10-02 指示：change 8 的 Codex 審查一律由 Opus 5.5 取代，跑完視同取代 Codex。

## Risks / Trade-offs

- [HERDR 日後輸出新的序列種類（例如 OSC 8 超連結）] → 整段丟棄、文字保留；解析器測試涵蓋 OSC 的 BEL 與 ST 兩種結尾。
- [回應約變兩倍大] → 200 行實測純文字 1–2 KB，加片段後仍是個位數 KB，每秒一次可忽略。
- [工作中的 pane 每秒內容都變，每次整段重建節點] → 改版前 `textContent` 也是整段替換；`replaceChildren` 一次完成，不逐節點插入。
- [選取文字在下一次重畫時消失] → 改版前即如此（`textContent` 替換），本 change 不改變。
- [歸色界線與終端機視覺不一致] → 見 D4；真機目前不出現真彩色。
- [`text` 與 HERDR `format=text` 不再逐位元相同] → 見 D3；task 重跑 `live-output-real-check.js`，若有逐字比對則改為比對解析後的語意。
- [`--graph-lane-4` 以 oklab 混色，試算用 sRGB] → 對比以 Chrome 計算值驗收，不以試算為準。

## Migration Plan

前後端同一個執行檔一起發佈，沒有相容性問題；回退即 revert。

## Open Questions

（無）
