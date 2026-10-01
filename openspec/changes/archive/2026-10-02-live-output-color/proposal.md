# proposal：live-output-color

## Why

Live Output 目前一律以單色純文字呈現（`read_output` 固定 `format=text`），agent 畫面上原本靠顏色傳達的資訊全部消失：
錯誤紅字、成功綠字、警告黃字、變暗的次要提示、diff 的增刪底色。使用者要回到 HERDR 視窗才看得出重點。
HERDR `pane.read` 已提供 `format=ansi`（設計文件 §2.6），且 2026-10-02 在 Windows 0.9.2 與 WSL 0.8.2 實測：只含 SGR 樣式序列、
樣式不跨行、去掉控制序列後與 `format=text` 只差行尾空白與結尾換行（`docs/research/2026-10-02/ansi-probe.md`）。
這是路線上「推 remote 前去識別化」之前的最後一個功能 change（`docs/handover.md` 第 5 節）。

## What Changes

- **讀取改用 `format=ansi`，在後端解析**：`cockpit-herdr` 以 `vte` crate 解析 SGR，產生帶樣式的片段。
  - 非 SGR 的控制序列（游標移動、OSC、DCS 等）與 `\r` 等控制字元一律丟棄。
  - 256 色與真彩色歸到 16 色之一。
- **輸出端點多一個 `segments` 欄位**：每段有文字，以及前景色、背景色（16 色名稱之一）與粗體、變暗、斜體、底線、反白。
  - `text` 仍是純文字，`format` 仍是 `"text"`。
  - **不變式**：所有片段文字依序串接等於 `text`。
- **`text` 的來源改變**：由 ansi 解析得出，不再是 HERDR 的 `format=text`。差異只在行尾空白與結尾換行（實測），畫面上看不出來。
- **面板依片段上色**：
  - 16 色對應既有色票（紅→`--bad`、綠→`--ok`、黃→`--warn`、藍／青→`--accent`、洋紅→`--graph-lane-4`、白→`--text`、黑／灰→`--text-dim`）。
  - 背景色與反白畫成 20% 淡底（片段同時有前景色時背景降為 14%，維持 4.5:1 對比）；沒有前景色的變暗改用 `--text-dim`。
  - 一律以 `textContent` 寫入，仍不解讀 HTML。
- **去重改比對片段**：只有顏色改變也會重畫；完全相同才不重寫。
- **過期時全部轉暗**：輸出標為過期時，有色片段一律改為 `--text-dim`、淡底移除。

## 非目標

- **把 Live Output 變成 terminal**：不處理游標定位、不重建畫面格、不接受輸入（設計文件 §12 與 `CONTEXT.md`「Live Output」：
  輸出投影，不是 terminal）。
- **新增色票或終端機專用調色盤**：使用者 2026-10-02 選定「對應既有色票」，部分顏色共用同一色票（藍與青）。
- **背景色以實心呈現、反白對調前景與背景**：在深色主題下對比不足，改為淡底（使用者 2026-10-02 選定）。
- **輪詢節奏、讀取行數、截斷提示**：不變（`live-output`「輪詢與顯示」）。
- **WSL 端以外的 runtime 種類、HERDR 以外的 runtime**：不在範圍；`AgentRuntime::read_output` 介面只多回傳片段。
- **推 remote 前的去識別化**：另開 change（`docs/handover.md` 第 4 節）。

## Capabilities

### New Capabilities

（無）

### Modified Capabilities

- `live-output`：讀取端點回應多 `segments` 並定義不變式；面板依片段上色、去重比對片段；過期時有色片段轉暗。
- `herdr-runtime-session`：「讀取 pane 輸出」改送 `format=ansi`，回應文字轉成純文字與片段後交回。
- `runtime-model`：「AgentRuntime 抽象」的讀取輸出多回傳片段，片段不變式由輸出型別保證。

## Impact

- `cockpit-core`：`runtime.rs`（`PaneOutput` 新增片段、以建構函式維持不變式；新增 16 色與片段樣式型別）。
- `cockpit-herdr`：新增 ansi 解析模組；`runtime.rs` 的 `read_output` 改送 `format=ansi`。新依賴 `vte` 0.15（Apache-2.0 OR MIT，
  預設 feature 只帶 `arrayvec`、`memchr`）。
- `cockpit`：`http.rs`（`OutputBody` 帶出 `segments`）；`assets/app/output.js`（逐段建節點、去重）、`style.css`（ansi class）；
  `examples/ui_preview.rs`（ansi 輸出模式）；`tests/output_endpoint.rs`。
- 測試：`cockpit-core/tests/`（`common/mod.rs`、`types.rs`、`runtime.rs`）、`cockpit-herdr/tests/read_output.rs` 改用建構函式與讀取函式。
- 驗收：新腳本 `docs/research/2026-10-02/output-color-check.js`（上色、對比、去重、過期轉暗）；既有腳本全數重跑，
  `docs/research/2026-09-19/live-output-real-check.js` 在 WSL 測試 server 上重跑。
- 文件：設計文件決策表「ANSI parsing crate」列、`CONTEXT.md`「Live Output」定義的「純文字」。
