# output-color-check：Live Output 依樣式上色驗收

驗收 OpenSpec change `live-output-color` 的前端行為（spec `live-output`）。腳本：
`docs/research/2026-10-02/output-color-check.js`（live-output-color task 5.3 建立）。
寫法比照 `docs/research/2026-10-01/ui-fixes-check.js`：raw CDP over WebSocket、headless Chrome、
自己 spawn `ui_preview`、依 PID 收尾。

## 用法

在 repo 根執行，先建置（前端資源內嵌在執行檔，改過 `cockpit/assets/` 一定要重建）：

```bash
cargo build -p cockpit --example ui_preview
node docs/research/2026-10-02/output-color-check.js                # 只跑斷言
node docs/research/2026-10-02/output-color-check.js --screenshots  # 斷言後再截圖
```

- 只需要 Node 與 Chrome（預設 `C:\Program Files\Google\Chrome\Application\chrome.exe`，可用環境變數
  `COCKPIT_CHROME` 指定）。
- 自己啟動 `ui_preview`，環境變數 `COCKPIT_PREVIEW_OUTPUT_MODES=wJ:p1=ansi;wJ:p3=ansi-flip`：`wJ:p1`
  固定回 `ansi_sample` 的 116 個有樣式片段（含 16 色前景 `[fg16=<色>]` 與 16 色背景 `[bg16=<色>]` 各 16 格），`wJ:p3` 每次讀取在紅、綠之間切換同一段文字 `status`。
  樣本標籤清單見 `cockpit/examples/ui_preview.rs` 的 `ansi_sample` 文件註解。
- 埠預設 7770，被占用就往上找空埠；Chrome 的 CDP 埠從 18810 起。
- 每條斷言印 `PASS`／`FAIL`，最後印 `RESULT: PASS` 或 `RESULT: FAIL (N)`，有失敗時結束碼為 2。
- Chrome 啟動後等不到 CDP target 或 WebSocket 時，腳本會先收掉自己開的 chrome 與暫存 user-data-dir 再中止。
- 收尾只依 PID 終止自己 spawn 的 `ui_preview.exe` 與 `chrome.exe`，並確認沒有殘留使用該
  user-data-dir 的 chrome、7770 與 CDP 埠沒有 LISTEN。**驗收腳本不可並行跑**，跑之前先確認 7770 沒有別人在用。
- `--screenshots`（task 5.4）：全部斷言跑完後選 `ansi` pane，把 viewport 設成 1536×1024、1100×900、
  700×900，內容框捲到頂（有色格線在最前面）後各截一張，存 `docs/research/2026-10-02/output-color-<寬>.png`
  （設計審核素材）。不帶旗標不產生任何檔案。
- 截圖去識別化（作法同 `ui-fixes-check.js`）：`ui_preview` 的 fixture 把 pane cwd 放在真實 `%TEMP%` 底下，畫面會出現
  `C:\Users\<使用者名稱>\…`。截圖前在頁面裝 MutationObserver，把文字節點中 `Users\` 之後的路徑段與真實使用者名稱、
  主機名稱換成 `<user>`（持續掃描，因為背景重畫會重建節點）；名稱執行時由 `os.userInfo()`／`os.hostname()` 取得，不寫進
  repo。每張截圖前斷言 `document.body.textContent` 不含兩者，命中就不截。commit 前仍要逐張看圖，並 grep PNG
  位元組確認沒有真名。

## 驗證方式

- 一律比 Chrome 的計算後樣式（`getComputedStyle`），不比 class 名稱：預期色從頁面的 `--bad`、`--ok`
  等色票取得（以探針元素套 `var(--…)` 讀計算色；`--graph-lane-4` 是 oklab 混色，非 rgb／`color(srgb)`
  的寫法交給 canvas 換算），不手 key hex。
- 容差：色彩每通道 ±2／255，不透明度 ±0.01。
- 以片段文字定位：在 `.output-text` 以 TreeWalker 找文字等於標籤的文字節點，取其父元素的計算樣式；
  找不到完全相等的節點時退而取包含該標籤的文字節點的父元素（失敗訊息能看出實際值，不是只有「找不到」）。
  但有樣式的標籤**必須**找得到完全相等的節點（A0 逐一斷言），片段被合併或切開時會失敗；只有無樣式的
  `[fg=none bg=none]`（與其後換行同屬一個未上色片段）不在此限。
- 對比：把片段的計算後背景（淡底）以其不透明度疊到內容框**實際量到的底色**（沿 `.output-text` 的祖先鏈收集背景層、
  合成到第一個不透明背景為止；背景透明就是該底色），再與文字色算 WCAG 對比，要求每個片段 >= 4.5:1。底色不寫死
  任何色票，版面日後改了面板底色，量到的值會跟著變。

## 各段與 spec scenario 對照

| 代號 | 斷言 | spec scenario |
|---|---|---|
| A0 | 64 格前景×背景全組合、8 個反白、16 種前景（30–37、90–97，含 `black` 與全部 `bright_*`）、16 種背景（40–47、100–107）、其餘 13 個片段（含 `[bg=cyan]`）的計算後文字色、底色、字重、斜體、底線逐一對照 spec 對照表；有樣式的片段必須是文字完全相等的獨立節點 | 輸出依樣式上色（全部 scenario 的總表；前景色／背景色對照含 `bright_*` 與 `black`） |
| A1 | `fg red` 文字為 `--bad`；`bg green` 底為 `--ok` 20% 且文字 `--text` | 紅字與綠底 |
| A2 | `fg red`＋`bg white` 底為 `--text` 14%、文字 `--bad`（對照：無前景的 `bg white` 為 20%） | 有色字疊在淡底上 |
| A3 | `fg red`＋`bg white`＋反白：底為 `--bad` 20%、文字 `--bad`；只帶反白：底為 `--text` 20% | 反白優先於背景 |
| A4 | `bg black` 沒有背景；`bg bright_black` 為 `--text-dim` 20% | 黑與亮黑背景 |
| A5 | 粗體字重 700、斜體、底線，文字色皆 `--text` | 粗體、斜體與底線 |
| A6 | 只帶 `dim`：`--text-dim`；`dim` 加 `fg red`：維持 `--bad`（用攔截回應補樣本沒有的組合） | 變暗的預設色文字 |
| A7 | `window.pwned` 未設定、面板沒有 `script`／`b` 元素、字樣原樣出現；`<script>` 字樣為 `--bad`、`<b>x</b>` 字重 700 | 上色後內容仍不被當成 HTML |
| A8 | 以 CDP `Fetch` 攔截輸出請求，回含 `fg`／`bg` 為 `orange; background: red` 的片段：以預設文字色呈現、無背景，面板內沒有元素的 class 或 style 含該字串（對照組 `fg red` 仍為 `--bad`，確認攔截內容真的被畫出）；另斷言這幾個片段是獨立節點，且（含 A7 之後的整個 ansi 樣本）內容框內沒有任何元素帶 `style` 屬性（design D7「不使用 `style` 屬性」） | 不認得的顏色名稱 |
| A9 | 117 項（116 個有樣式片段加無樣式的 `[fg=none bg=none]`）的文字色對實測底色合成後 >= 4.5:1；另斷言量測集合涵蓋 7 種色票文字色與至少 8 種底色，避免全預設色時對比恆過的空轉 | 對比 |
| A10 | 256 色 196 為 `--bad`、真彩色 `#d77757` 為 `--warn`、`cyan` 為 `--accent`（前端畫出後端歸色結果） | 前景對照表 |
| T1 | 面板 `textContent` 等於同一端點回應的 `text`；`segments` 串接等於 `text` 且 >= 80 段 | 輪詢與顯示（`textContent` 必須等於回應的 `text`） |
| M1 | 以 `Fetch` 攔截輸出請求，回 200 但本體為 `null`、`[]`、`7`、`"x"`（不是 JSON 物件）：前端仍在輪詢（8 秒內至少 3 次後續請求）、面板內容維持原樣不被清空，之後回合法本體即恢復顯示 | 輪詢與顯示（5.6 F2：`null` 曾使輪詢永久停止） |
| X1 | 以 `Fetch` 攔截：`segments` 含 `text` 非字串的元素使串接不等於 `text`、或串接與 `text` 完全不同：面板 `textContent` 仍等於回應 `text`（退回單一無樣式片段、無 span）；對照組：串接等於 `text`（含略過非字串元素後仍相等）時照片段上色 | 輪詢與顯示（`textContent` 必須等於回應的 `text`；design D7 防禦） |
| P1 | 靜態 `ansi` pane 上以 MutationObserver 觀察 >= 3 次輪詢：內容框無任何 childList／characterData／attributes 變動，內容框與子節點仍是原節點 | 相同內容不重寫 |
| P2 | `ansi-flip` pane：`textContent` 一律是 `status` 加換行，但計算後顏色在 `--bad` 與 `--ok` 間來回，兩色相隔 <= 3 秒、5 秒內至少切換 3 次 | 只有顏色改變也會重畫 |
| S1 | 503 前 `fg red`／`bg green` 片段有色；以 `Fetch` 回 503 `{"error":"x"}`：面板標為過期，兩片段與全部片段文字為 `--text-dim` 且無背景，且是獨立片段節點；放行恢復後回到 `--bad` 文字與 `--ok` 20% 淡底 | 過期時有色內容一併轉暗 |

S1 檢查「獨立片段節點」是因為既有的 `.output-panel.is-stale .output-text` 本來就會把整個內容框轉成
`--text-dim`；沒有這個條件，「全部轉暗」在尚未上色的前端也會成立。

## 紅綠證據（task 5.3 與 5.2 的順序）

task 5.3 排在 5.2（前端實作）之前執行，所以第一次在尚未上色的 HEAD 上跑，**依賴新行為的斷言必須為紅**
（上色、淡底、反白、粗斜底線、變暗、對比涵蓋、只換色也重畫、過期前後的有色狀態），既有行為本來就綠
（`textContent` 等於 `text`、相同內容不重寫、HTML 不被解讀、不認得顏色名稱不外漏、`bg black` 無背景）。
5.2 完成後本腳本應全綠。
