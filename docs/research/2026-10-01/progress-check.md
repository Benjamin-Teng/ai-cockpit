# progress-check：退回按鈕與未宣告 task 提示驗收

progress-model task 4.2、4.3 的驗收腳本。啟動自己的 `ui_preview` 與 headless Chrome，以 CDP 驅動，
結尾輸出 `RESULT: PASS` 或 `RESULT: FAIL (N)`。

## 用法

在 repo 根執行，需先 `cargo build -p cockpit --example ui_preview`（前端資源內嵌進執行檔）：

```bash
node docs/research/2026-10-01/progress-check.js
```

- 預設用 7770 埠（被占用就往上找空埠），CDP 用 18800 起。
- 只終止本腳本自己 spawn 的 `ui_preview.exe`、`chrome.exe`（依 PID），結束時確認埠不再 LISTEN。
- 不可與其他驗收腳本並行（共用暫存目錄與埠）。

設計審核截圖（Project `p` 在寬 1536、1100、700）由同目錄的腳本產生：

```bash
node docs/research/2026-10-01/progress-screenshots.js
```

## 段落

| 代號 | 內容 | 對應 spec scenario |
|---|---|---|
| A1 | ui_preview：逐 Project 對照 `/api/state`，每個 task 的「退回」有無必須等於「mark 為 none 且不在第一個 stage」；並確認情境涵蓋第一站無標記、有標記、可退回三種 | cockpit-dashboard「退回按鈕」 |
| A2 | ui_preview：`p/undeclared` 列首有「工作中・未宣告 task」，其他列沒有；整頁重畫（version 前進且 DOM 節點被換掉）後仍一致；提示及其偽元素 `animation-name` 為 `none` | cockpit-dashboard「未宣告 task 的提示」 |
| A3 | ui_preview：按 `p/undeclared-1` 的「退回」，服務收到恰好一個 `POST /api/projects/p/tasks/undeclared-1/retreat`；停在此狀態再收到新投影重畫後，按鈕與提示仍在 | 「退回按鈕」、「未宣告 task 的提示」 |
| B1 | harness：spec 原文情境（stages `Plan`、`Build`；`t1` 在 `Build` 無標記、`t2` 在 `Plan`、`t3` 在 `Build` 標 failed）。只有 `t1` 有「退回」；按鈕順序為退回、Completed、Failed；按下後收到 `POST .../t1/retreat`；新投影到達前 `t1` 仍在 `Build`，到達後出現在 `Plan` 欄 | 「退回按鈕」 |
| B2 | harness：`backend` 為 true、`frontend` 為 false；連續兩份新投影重畫後結果相同；改成 false 提示消失、再改回 true 回來（提示完全由投影決定，不是殘留的 DOM 狀態）；欄位缺漏時不顯示 | 「未宣告 task 的提示」 |

## 為什麼 A 段不斷言按退回後的節點位置

`ui_preview` 的 domain 來自靜態 fixture，寫入端點只記錄請求並回 204，不會真的改變 stage。
所以 A 段只斷言「送出的請求 URL」與「畫面不自行改狀態」（節點仍在 `Implement`）；
「新投影到達後節點出現在上一站」由 B1 直接餵新投影驗證。
