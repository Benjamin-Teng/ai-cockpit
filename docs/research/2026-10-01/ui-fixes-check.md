# ui-fixes-check：畫面與操作修補驗收

OpenSpec change `ui-fixes` 的前端驗收腳本，各 task 逐步擴充。啟動自己的 `ui_preview` 與 headless Chrome，
以 CDP 驅動，結尾輸出 `RESULT: PASS` 或 `RESULT: FAIL (N)`。輸入一律用 `Input.dispatchMouseEvent`、
`Input.dispatchKeyEvent` 產生真實事件，不用 `el.click()`／`el.focus()`，因為 `:focus-visible` 取決於瀏覽器
對最近一次真實輸入的判斷。

## 用法

在 repo 根執行，需先 `cargo build -p cockpit --example ui_preview`（前端資源內嵌進執行檔）：

```bash
node docs/research/2026-10-01/ui-fixes-check.js [--screenshots]
```

`--screenshots` 另產生斷線狀態 1536／1100／700 寬的設計審核截圖（`ui-fixes-disconnected-<寬>.png`，與本檔同目錄）。

- 預設用 7770 埠（被占用就往上找空埠），CDP 用 18800 起。
- 只終止本腳本自己 spawn 的 `ui_preview.exe`、`chrome.exe`（依 PID），結束時確認埠不再 LISTEN。
- 不可與其他驗收腳本並行（共用暫存目錄與埠）。
- 每段開頭都重新載入頁面：「第一次點擊」要在新載入頁面後量，此時還沒有任何輸入記錄，是最容易漏的情況。

## 段落

| 代號 | 內容 | 對應 spec scenario |
|---|---|---|
| F1 | 新載入後第一次以滑鼠點 pane 列：重畫當下與其後 1 秒（投影每 100 ms 推送、整頁重畫數次）焦點都在代表同一 pane 的新 pane 列上，且不匹配 `:focus-visible` | cockpit-dashboard「滑鼠點擊後重畫不呈現焦點外框」 |
| F2 | 同上，點左欄專案 | 同上 |
| F3 | 同上，點 task 按鈕（「推進」） | 同上 |
| F4 | 點 pane 列開啟面板後點「取消選取」：焦點交接到 pane 列（或面板）、沒有掉到 `body`，且不匹配 `:focus-visible`（`output.js` 的焦點交接也經共用 helper） | 同上（焦點交接路徑） |
| F5 | 以 Tab 把焦點移到某 task 的「推進」按鈕（外框可見）後按 Enter：服務收到該 task 的 `advance` 請求，重畫數次後焦點仍在同一個按鈕上且匹配 `:focus-visible` | 「鍵盤觸發的重畫保留焦點外框」 |
| F6 | 以滑鼠點 pane 列（外框不可見）後按 Tab：新焦點元素匹配 `:focus-visible`，之後的背景重畫不洗掉外框 | 「滑鼠點擊之後按 Tab 外框照常出現」 |
| F7 | 以滑鼠點 pane 列（外框不可見）後單按 Alt、Ctrl（CDP 真實按鍵）：修飾鍵不算鍵盤輸入，背景重畫數次後焦點仍在同一個 pane 列且不匹配 `:focus-visible`（修正波 1 F-I1） | 「修飾鍵不改變最近一次輸入方式」 |
| F8 | 以 Tab 把焦點移到「推進」按鈕（外框可見）後，對不移動焦點的位置送真實滑鼠按壓（優先按在有 classic scrollbar 的可捲容器的捲軸上，找不到才改按 `mousedown` 被取消的非可聚焦元素；斷言 document 收到 pointerdown 且焦點沒動）：背景重畫數次後焦點仍在同一按鈕且匹配 `:focus-visible`（修正波 1 F-M1） | 「不移動焦點的滑鼠操作不洗掉焦點外框」 |
| D1 | 通道斷線（`window.onChannel('disconnected')` 模擬，同 `visual-check.js` CH1）：以 `getComputedStyle` 逐區比對 `--text-dim`：runtime 卡連線狀態（符號＋文字）、卡內 pane／workspace／tab 的 agent 狀態（符號＋文字）、task 節點（色條、狀態文字與符號、running／failed／blocked 外框、running 柔光）、列首「未宣告 task」提示、左欄 project 狀態計數；runtime 卡「最後已知」是連線狀態節點的手足、狀態節點 `textContent` 仍等於狀態字串；按鈕外觀與連線時完全相同；恢復後各處還原（含連線中基準：狀態色確實不是 `--text-dim`，斷言才不是恆真） | cockpit-dashboard「通道斷線時 runtime 卡顯示最後已知」「通道斷線時中欄與 pane 列的狀態色轉暗」 |
| D2 | 斷線中以真滑鼠點 pane 列觸發整頁重畫，以及其後 1 秒的背景重畫：右欄、pane 列、task 節點、未宣告提示、project 計數仍為最後已知；根節點 `data-channel-state` 被移除後 `paint()` 寫回；通道 `connecting` 亦同 | 「斷線中整頁重畫後仍為最後已知」 |
| D3 | 真的停掉 `ui_preview` 造成通道斷線（不靠模擬）、再重啟：斷線中轉暗、重連後還原 | 同 D1／「通道重連」 |
| B1 | 覆蓋造成的斷線（`cockpit/ovr`，`source: override`）：綁定摘要顯示「runtime 未連線」並有「改綁」徽章（`.ff-binding-badge`），列首同時有「改綁」按鈕與「取消改綁」按鈕 | cockpit-dashboard「覆蓋造成的斷線仍顯示改綁標示」「覆蓋造成的斷線可取消改綁」 |
| B2 | 自動綁定的斷線（`cockpit/docs`、`p/tests`，`source: auto`）：沒有徽章、沒有「取消改綁」，但有「改綁」按鈕 | 「覆蓋造成的斷線仍顯示改綁標示」「自動綁定的斷線沒有取消改綁」 |
| B3 | 停在上述狀態整頁重畫（`window.repaint()`、點 pane 列、其後背景重畫）後 B1、B2 仍成立 | 同上（整頁重畫不丟狀態） |
| B4 | 真滑鼠按 `ovr` 的「取消改綁」：服務恰好收到 `DELETE /api/projects/cockpit/workstreams/ovr/override` | 「覆蓋造成的斷線可取消改綁」 |

B1 的徽章與列首的「改綁」按鈕是兩個不同元素：徽章在綁定摘要內（`span`），按鈕在列首操作區（`button[data-action="rebind"]`），
腳本分開量測。

F5 的「對應按鈕」：`ui_preview` 的 domain 是靜態 fixture，按「推進」後 stage 不變，所以重畫後同一顆按鈕仍在，
身分（`data-action`、`data-project`、`data-task`）應完全相同。

## 成因與修法（供對照）

`actions.js` 在 `#app` 的 `pointerdown` 委派呼叫 `preventDefault()` 並同步重畫，這次滑鼠操作沒有讓任何元素被滑鼠聚焦，
Chrome 因此把之後的程式焦點算作 `:focus-visible`。修法（design D1）：`output.js` 記錄最後輸入方式（`document` capture 階段
的 `pointerdown`／`keydown`），經 `window.cockpitFocus.focus(el)` 還原焦點；最後輸入為滑鼠時帶 `focusVisible: false`。
