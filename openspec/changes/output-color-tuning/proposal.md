# proposal：output-color-tuning

## Why

change 8 `live-output-color` 的整支分支審查（archive `2026-10-02-live-output-color/sdd-ledger.md` Task 7.3 M1）指出：256 色與真彩色歸到 16 色時，
「最大與最小分量差小於 64 即視為無彩」的門檻太高，Claude Code 深色主題的 diff 底色（新增行、刪除行與其淡化版）會被歸成灰色——一旦 HERDR
改傳 256 色或真彩色，diff 會不報錯地畫錯。另外 5.5 外觀審核發現 12px 等寬字下粗體 600 與一般字差距小。使用者 2026-10-02 授權這兩項由 Claude 決定。
目前 HERDR 實測只傳 16 色（`docs/research/2026-10-02/ansi-probe.md`），改門檻對現況畫面沒有影響，是預防性修正。

## What Changes

- **無彩門檻**：由「差 < 64」改為「差 < 16，或差 < 最大分量的十分之一」。以本機 Claude Code 2.x 四套主題（深色、淺色、各自的色盲友善版）共 24 個 diff 色實算：
  舊規則 10 個歸成灰或白，新規則 23 個歸到正確的紅、綠、藍（剩下淺色色盲版的極淡粉紅 `rgb(255,233,233)` 仍歸白）；256 色灰階 24 階結果不變，
  色立方 216 色中 24 個帶色調的顏色由灰或白改歸有色。
- **粗體字重**：600 改為 700。

## 非目標

- 不改色相區間、無彩時的明度分界、色票對應、淡底比例（spec「輸出樣式轉換」「輸出依樣式上色」其餘條文不變）。
- 不改 HERDR 讀取方式或解析器其他規則（設計見 archive `2026-10-02-live-output-color/design.md` D1、D4）。

## Capabilities

### New Capabilities

（無）

### Modified Capabilities

- `live-output`：「輸出樣式轉換」的無彩門檻；「輸出依樣式上色」的粗體字重。

## Impact

- `cockpit-herdr/src/ansi.rs`（歸色函式）與 `cockpit-herdr/tests/ansi.rs`（界線測試）。
- `cockpit/assets/app/style.css`（`.ansi-bold`）。
- `docs/research/2026-10-02/output-color-check.js`（粗體斷言）；`ui_preview` 的 ansi 樣本若有依舊門檻預期的標籤需同步。
- 截圖 `docs/research/2026-10-02/output-color-*.png` 重拍。
