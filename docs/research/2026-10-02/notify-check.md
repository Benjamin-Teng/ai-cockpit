# notify-check：桌面通知與通知設定面板驗收

驗收 OpenSpec change `desktop-launch-notify` 的前端通知行為（desktop-launch-notify task 3.2；design D7、D8、D9、D10）。
腳本：`docs/research/2026-10-02/notify-check.js`。寫法比照 `docs/research/2026-10-01/ui-fixes-check.js`：
raw CDP over WebSocket、headless Chrome（自己的 user-data-dir）、自己 spawn `ui_preview`、依 PID 收尾。

權威是 spec：`openspec/changes/desktop-launch-notify/specs/desktop-notifications/spec.md`（「通知事件」「通知呈現」
「通知設定」全部 scenario）與 `specs/cockpit-dashboard/spec.md`（`/app/notify.js` 路由、「鈴鐺按鈕跨重畫保留焦點」
「鈴鐺不影響畫面操作的錯誤訊息」）。

## 用法

在 repo 根執行，先建置（前端資源內嵌在執行檔，改過 `cockpit/assets/` 一定要重建）：

```bash
cargo build -p cockpit --example ui_preview
node docs/research/2026-10-02/notify-check.js              # 全部（N、M、S、T 四段，約 80 秒）
node docs/research/2026-10-02/notify-check.js --only=S     # 只跑某幾段（逗號分隔，例如 --only=N,M）
node docs/research/2026-10-02/notify-check.js --screenshots              # 全部斷言之後再截圖
node docs/research/2026-10-02/notify-check.js --only=none --screenshots  # 只截圖
```

- 只需要 Node 22 與 Chrome（預設 `C:\Program Files\Google\Chrome\Application\chrome.exe`，可用環境變數
  `COCKPIT_CHROME` 指定）。
- 埠預設 7770，被占用就往上找空埠；Chrome 的 CDP 埠從 18810 起。三段依序各起一個 `ui_preview`（同一個埠），
  本機儲存因此跨段共用，每段開頭會清空。
- 每條斷言印 `PASS`／`FAIL`。「前置：」開頭的是環境、fixture 與既有行為的前提（例如 ui_preview 起來了、
  按「Failed」會出現錯誤訊息），不是本 change 的新行為；結尾分別列出新行為斷言與前置斷言的通過／失敗數，
  最後一行 `RESULT: PASS` 或 `RESULT: FAIL (N)`，有失敗時結束碼為 2。
- 收尾只依 PID 終止自己 spawn 的 `ui_preview.exe` 與 `chrome.exe` 並刪暫存 user-data-dir。**驗收腳本不可並行跑**，
  跑之前先確認 7770 沒有別人在用。
- 預設不產生截圖；加 `--screenshots` 才截圖（見下節）。

## 截圖（`--screenshots`，task 3.4）

斷言段跑完後（或搭配 `--only=none` 單獨執行），另起一個 `ui_preview`（同樣設一小時後才套用的無害轉換，停掉 `wJ:p1` 的輪替），
以記錄器把通知權限設為**尚未決定**（`default`）後載入，用真實滑鼠點鈴鐺開啟設定面板，
在 1536×1024 與 700×900 兩種 viewport 各截一張，存到 `docs/research/2026-10-02/`：

- `notify-panel-1536.png`
- `notify-panel-700.png`

選「尚未決定」的理由：面板同時顯示權限說明（「通知權限：尚未決定」）、「允許通知」按鈕與四個預設開關
（`agent blocked`、`task failed` 開；`agent done`、`task completed` 關），是各權限狀態中資訊最完整的一種；
已允許、已封鎖、不支援三種只是少了按鈕或換說明文字，由 S12 至 S14 以文字斷言涵蓋。

去識別化：作法與 `output-color-check.js --screenshots` 相同。ui_preview 的 fixture 把 pane cwd 放在真實 `%TEMP%` 底下，
畫面右欄會出現含使用者名稱的路徑。截圖前在頁面裝 `MutationObserver`，持續把文字節點中 `Users\` 之後的路徑段與
真實使用者名稱、主機名稱換成 `<user>`（名稱只在執行時由 `os.userInfo().username`、`os.hostname()` 取得，不寫進 repo）；
每次截圖前再斷言 `document.body.textContent` 不含這兩個字串，命中就不截。截完要逐張看圖，並跑
`node docs/research/2026-10-02/deid-check.js`（檔案模式）確認 0 命中。截圖斷言列為「前置：」類。

## 測試替身

以 CDP `Page.addScriptToEvaluateOnNewDocument` 在每次載入前注入（只對 `http:` 頁面）：

- `window.Notification` 換成記錄器：建構時記下 `title`、`body`、`tag`、`renotify`、當下權限與前景狀態到
  `window.__notifs`；靜態 `permission` 可切換 `granted`／`default`／`denied`；`requestPermission()` 記次數
  （`window.__permRequests`），約 50 ms 後以設定值解析並更新 `permission`；實例支援 `onclick`、`addEventListener("click")`
  與 `close()`。「不支援」時直接刪掉 `window.Notification`。
- `document.hasFocus()` 可控（預設 `false`，所以通知會發出）；`document.visibilityState` 可覆寫成 `hidden`。
- `window.focus()` 換成計次（`window.__focusCalls`），用來判斷「帶到前景」。
- 「本機儲存不可用」時，存取 `window.localStorage` 本身丟 `SecurityError`。
- 包一層 `WebSocket`，記下每條 `/ws` 連線第一則訊息中 `cockpit/ops-2` 的 status（M9 的前提檢查）。

設定存在 localStorage 鍵 `__cockpitNotifyTest`（與產品的 `cockpit.notify.v1` 分開），重新載入後保留。
作業系統的通知無法自動化點選，「點通知」是在頁面內呼叫記錄器實例的 click。

## DOM 約定（task 3.3 實作要對齊）

腳本只依下列約定找元素；其餘外觀由實作決定（frontend-design 審核）。

| 對象 | 約定 | 依據 |
|---|---|---|
| 鈴鐺 | `#app` 頂列（`[data-region="topbar"]`）內的 `<button data-action="notify-settings">` | design D8 |
| 設定面板 | `id="notify-panel"`，在 `body` 底下、`#app` 之外；「開啟」＝沒有 `hidden`、`display` 不是 `none`、有尺寸 | design D8 |
| 四個開關 | 面板內帶 `data-notify-kind="blocked\|done\|failed\|completed"` 的可聚焦元素，DOM 順序同左；`<input type="checkbox">`（讀 `checked`）或 `role="switch"`（讀 `aria-checked`），Space 切換 | spec「通知設定」 |
| 開關名稱 | 可及名稱（`<label>`、`aria-label`、`aria-labelledby` 或外層 `<label>`）含 `agent blocked` 等英文標籤 | spec「通知設定」 |
| 允許按鈕 | 面板內文字恰為「允許通知」的 `<button>` | spec「通知設定」 |
| 權限文字 | 已允許含「已允許」；已封鎖含「網站設定」；不支援含「不支援」 | spec「通知設定」 |
| 模組 | `window.cockpitNotify` 有 `observe(state)`、`togglePanel()`、`diff(prev, next)`、`toNotifications(events, settings)` | design D7 |
| `toNotifications` | 回傳陣列，每項有 `title`、`body`、`tag`（`target` 不檢查）；`settings` 形如 `{"blocked":true,"done":false,"failed":true,"completed":false}` | design D7 |
| 本機儲存 | 鍵 `cockpit.notify.v1`，值為上列四鍵的 JSON | design D7 |

合併通知的內文只檢查「4 個 task 標題中恰好出現 3 個」且以「…」結尾，不限定排列與分隔方式。

## 分段與對應 scenario

### N 段（ui_preview 一）

`COCKPIT_PREVIEW_TRANSITIONS`（毫秒從 ui_preview 啟動起算）：300 `wJ:p3=blocked`（載入前）、6000 `wJ:p1=blocked`、
8000 `wJ:p4=done`＋`cockpit/be-1=completed`、10000／10500 `wJ:p3` working→blocked、12000／12500 `wJ:p1` working→blocked、
14000 `wJ:p5=blocked`；另設 `COCKPIT_PREVIEW_VANISH_PANE=wJ:p5=15500`。

| 代號 | 斷言 | 對應 |
|---|---|---|
| N1 | 第一份狀態已有 blocked pane（`wJ:p3`）與 failed task（fixture 的 `ops-2`），載入後 1.5 秒 0 則通知（且模組已載入） | 第一份狀態不通知 |
| N2 | `wJ:p1` working→blocked：恰好一則（持續 blocked 不重發），標題「agent 卡住」、內文「win / wJ:p1（Backend、Undeclared）」、tag `cockpit:blocked:win/wJ:p1`、`renotify: true` | agent 卡住、通知內容（多個綁定去重：`Backend` 有兩個 workstream） |
| N3 | `wJ:p4` working→done、`be-1` running→completed：0 則 | 預設關閉的類別 |
| N4 | `wJ:p3`：內文「win / wJ:p3（QA）」（`p/frontend` 是 ambiguous，不算綁定） | 通知內容 |
| N5 | `wJ:p1` 再次 blocked：同 tag 的新通知、`renotify: true` | 通知呈現（同標籤取代仍提醒） |
| N6 | 無綁定 pane `wJ:p5`：內文「win / wJ:p5」 | 通知內容 |
| N7 | 點 `wJ:p1` 通知：`window.focus()` 一次、`close()`、右欄 `wJ:p1` 列 `.selected`、Live Output 標題 `win / wJ:p1`，背景重畫後仍在 | 點通知帶出 pane |
| N8 | `wJ:p5` 已從投影消失後點它的通知：帶到前景、關閉，選取不變 | 通知呈現（pane 已不在最新狀態） |
| N9 | 改綁模式中、選定 `wJ:p1`，點 `wJ:p3` 通知：帶到前景、關閉，選取仍是 `wJ:p1`、仍在改綁模式 | 改綁模式中點通知 |
| N10 | `cockpitActions.selectPane('win', 'wJ:p2')`（fixture 中 exited、pane 列不可點）回 `false`，選取仍是 `wJ:p1`（修正波 3.6 M3） | 通知呈現（點通知＝點 pane 列；exited 的列不可點） |
| N11 | 700×500、`wJ:p3` 列捲到視窗外時點它的通知：選定後該列完整出現在視窗內（容許 1px 次像素；修正波 3.6 M8） | 點通知帶出 pane（右欄選定標示要看得到） |

spec 的 scenario 以 `wJ:p2`、`backend` 舉例；fixture 中 `wJ:p2` 是 exited（不產生事件，見 P7），所以改用
`wJ:p1`（綁定 `Backend`、`Undeclared`）與 `wJ:p3`（綁定 `QA`）。

### M 段（ui_preview 二，再重啟一次）

轉換：8000 `wJ:p1=blocked`、10000 `wJ:p3=blocked`、12000 四個 task（`cockpit/be-1`、`docs-1`、`release-1`、`be-2`）同時 failed、
14000 `p/backend-1=completed`、16000 `wJ:p4=done`、18000 `p/frontend-1=failed`、19000 `cockpit/ops-2=running`。

| 代號 | 斷言 | 對應 |
|---|---|---|
| M1 | 滑鼠按鈴鐺開面板，鍵盤 Tab＋Space 開啟 done 與 completed | 開啟後的類別（前置）、變更立即生效 |
| M2 | `hasFocus()` 為真且可見時 `wJ:p1` 變 blocked：0 則 | 正在看畫面時不打擾 |
| M3 | 有焦點但 `visibilityState` 為 `hidden`（最小化）時照發 | 通知呈現（前景判斷） |
| M4 | 同一份狀態 4 個 task failed：只發一則，標題「Cockpit：4 件事需要注意」、列 3 件、以「…」結尾、tag `cockpit:summary`、`renotify: true` | 合併通知 |
| M5 | `p/backend-1` running→completed：「task completed」「Scenario D Demo：後端實作」、tag `cockpit:completed:p/backend-1` | 開啟後的類別、通知內容（task） |
| M6 | `wJ:p4` idle→done：「agent 停下等你看」「win / wJ:p4」 | 開啟後的類別（done） |
| M7 | `p/frontend-1` running→failed：「task failed」「Scenario D Demo：前端規劃」 | 通知事件（failed） |
| M8 | 點合併通知：帶到前景、關閉、不選定任何 pane | 通知呈現（點選） |
| M9 | 真的停掉 ui_preview 再重啟：重連後一則「task failed」「AI Cockpit：上線檢查」、tag `cockpit:failed:cockpit/ops-2` | 重連後補報 |

**M9 的做法與理由**：斷線前最後一份狀態中 `ops-2` 為 running（19000 ms 的轉換），重啟的 ui_preview 從 fixture
開始，`ops-2` 是 failed，所以重連後的第一份狀態一定帶著 failed，與重試時序無關。保留基準時是 running→failed，
發一則；若實作在重連時重設基準，這份只會當基準、不發，可以分辨。其餘欄位相對 fixture 的差異都是「變回非通知狀態」，
不產生事件。沒有採用「新 ui_preview 啟動後再套用轉換」：Windows 連到沒人在聽的 localhost 埠約 2 秒才失敗，
頁面的重試會卡在連線中、新服務一起來就連上，第一份訊息是轉換前的狀態，分辨不出兩種實作（初版實測如此）。
也沒有只呼叫 `cockpitNotify.diff`：那驗不到「通道重連時模組保留上一份」這個接線。

### S 段（ui_preview 三）與 P 段

ui_preview 設一條一小時後才套用的轉換（只為停掉 `wJ:p1` 的輪替），並以 `COCKPIT_PREVIEW_WRITE_RULES` 讓
`POST /api/projects/cockpit/tasks/be-1/fail` 立即回 409（產生錯誤訊息）、`POST /api/projects/cockpit/tasks/docs-1/fail`
1500 ms 後才回 409（S23 的進行中寫入）。

| 代號 | 斷言 | 對應 |
|---|---|---|
| S0 | `GET /app/notify.js` 200、JavaScript；`index.html` 在 `render.js` 之前載入 | cockpit-dashboard「路由與 content-type」；design D7 |
| S1 | 鈴鐺是頂列的 `<button>`；Tab 到鈴鐺外框可見；投影每 100 ms 推送、1 秒後焦點仍在新的鈴鐺且外框可見 | 鈴鐺按鈕跨重畫保留焦點 |
| S2 | 按 Enter 開啟；面板在 `#app` 之外；四個開關順序；焦點到第一個開關且外框可見 | 鈴鐺按鈕跨重畫保留焦點、通知設定（開啟焦點、焦點外框） |
| S3 | 預設 blocked、failed 開；四個可及名稱；標籤順序；有中文說明；不出現「完成」；已允許時顯示「已允許」 | 預設值、通知設定（標籤） |
| S4 | 面板內可見文字對比 ≥ 4.5:1（文字色疊到祖先鏈實際背景）；`prefers-reduced-motion: reduce` 下沒有動畫與 transition | 通知設定（Direction 01 視覺語彙） |
| S5 | Space 開 done，立即寫入 `cockpit.notify.v1`；2 秒、≥ 10 份推送後面板節點未換、仍開啟、開關不變、焦點仍在 done | 重畫不關閉面板 |
| S6 | Esc 關閉，焦點回鈴鐺 | 通知設定（關閉與焦點） |
| S7 | 鍵盤再開（焦點到第一個開關），滑鼠再按鈴鐺關閉，焦點回鈴鐺 | 通知設定（再按一次鈴鐺） |
| S8 | 滑鼠開啟 300 ms 後仍開（點外面的判定排除鈴鐺）、焦點到第一個開關；點產品名稱關閉，焦點回鈴鐺 | 通知設定（點面板外關閉） |
| S9 | 關 blocked、開 done，本機儲存隨之改變；重新載入後面板顯示 blocked 關、done 開 | 設定保留 |
| S10 | 本機儲存為 `{not json` 或各鍵型別錯誤：預設值、無未捕捉例外 | 通知設定（內容損毀） |
| S11 | 存取 `localStorage` 丟例外：預設值、切換仍立即生效、無未捕捉例外 | 通知設定（本機儲存不可用） |
| S12 | 權限尚未決定：observe 兩份快照不發、不報錯、不主動請求；面板有「允許通知」，按下請求一次，允許後顯示已允許、按鈕消失 | 請求權限、通知呈現（權限不是已允許） |
| S13 | 權限已封鎖：不發、不請求；面板說明含「網站設定」、沒有允許按鈕 | 通知設定（已封鎖說明） |
| S14 | 不支援：observe 不報錯；面板說明含「不支援」 | 通知呈現、通知設定（不支援） |
| S15 | 對照組：權限已允許時同樣的 observe 發出一則（`win / wJ:p4`） | 通知呈現 |
| S16 | 按 be-1「Failed」得到錯誤訊息後，按鈴鐺開啟、再按關閉，錯誤訊息仍在（含其後背景重畫） | 鈴鐺不影響畫面操作的錯誤訊息 |
| S17 | 改綁模式中按鈴鐺開啟再關閉，仍在改綁模式 | cockpit-dashboard「畫面整頁重畫」（鈴鐺不改變進行中的操作狀態） |
| S18 | 鈴鐺 `aria-expanded`：關閉時 `false`、開啟時 `true`，面板開著時整頁重畫換掉鈴鐺後仍為 `true`，Esc 或點面板外關閉後回 `false`；開啟時框線為 `--accent`（按下狀態，關閉後恢復）；四個開關的 `accent-color` 為 `--accent`（修正波，3.5 採納項） | 通知設定（鈴鐺可用鍵盤聚焦與操作）、重畫不關閉面板 |
| S19 | 1536×1024 與 700×900 時鈴鐺約為正方形、高不超過 20px、在頂列垂直置中（修正波 3.6 M2） | 通知設定（鈴鐺外觀依 Direction 01） |
| S20 | 焦點所在元素的 keydown 處理常式已對 Esc `preventDefault`（模擬其他浮層自己處理 Esc）時面板不關；下一次未處理的 Esc 照常關閉、焦點回鈴鐺（修正波 3.6 M5） | 通知設定（按 Esc 關閉） |
| S21 | 面板的 Tab 順序接在鈴鐺之後：第一個開關按 Shift+Tab 回鈴鐺（面板仍開）、鈴鐺按 Tab 回第一個開關、最後一個可聚焦元素按 Tab 關閉面板並回鈴鐺（修正波 3.6 M9） | 通知設定（開啟焦點、關閉後焦點回鈴鐺） |
| S22 | 700×500、頁面往下捲 400 px 使鈴鐺在視窗外時以 Enter 開啟：面板與取得焦點的第一個開關都在視窗內；開著時捲到底仍在視窗內；捲回頂端後面板在鈴鐺下方（修正波 3.6 M4） | 通知設定（開啟時焦點移到第一個開關，須看得到） |
| S23 | 按 `docs-1` 的「Failed」（1500 ms 後才回 409），回應之前按鈴鐺開啟再關閉：稍後的錯誤訊息仍出現（修正波 3.6 M1；鈴鐺若遞增 `latestOp`，錯誤會被當成過期而吞掉） | cockpit-dashboard「畫面整頁重畫」（鈴鐺不改變進行中的操作狀態） |

P 段在頁面內以 `/api/state` 為底做前後兩份狀態，直接呼叫 `diff` 與 `toNotifications`：

| 代號 | 內容 |
|---|---|
| P1 | 相同狀態 → 0 事件 |
| P2、P3 | `wJ:p1` working→blocked 的完整內容（綁定名稱去重）；blocked 關閉時 0 則 |
| P4、P5 | 只算 `binding.state=bound`（ambiguous 不算）；無綁定的內文 |
| P6、P7、P8 | 新出現的 pane、exited 的 pane、新出現的 task 都不產生事件 |
| P9 | pane 以 runtime＋pane id 識別；綁定只認 runtime 相符者 |
| P10、P11 | done 預設關閉；開啟後「agent 停下等你看」 |
| P12、P13、P14、P15 | completed 預設關閉、開啟後內容；failed 內容；failed→failed 不重複 |
| P16、P17、P18 | 3 件逐一；關閉的類別不計入件數；5 件合併成一則 |
| P19 | blocked→done 產生 done 事件、不產生 blocked 事件 |

### T 段（ui_preview 四，同源兩個分頁）

同一個 headless Chrome（共用 user-data-dir 與 localStorage）開兩個分頁甲、乙，兩頁都掛同一份注入腳本；啟動器會開出多個同源視窗，
這一段守住「變更立即生效」在多視窗下成立（5.3 審查 I2）。「乙目前生效的設定」以頁面內 `observe` 餵 `wJ:p4` working→done
兩份狀態、看有沒有發出通知來判定。

| 代號 | 斷言 | 對應 |
|---|---|---|
| T1 | 乙開著面板（預設值）、done 關閉時 working→done 不發通知；甲以滑鼠開啟 done，本機儲存為 done 開 | 通知設定（變更立即生效） |
| T2 | 乙開著的面板立即顯示 done 已勾且仍開著；乙之後的 working→done 發出「agent 停下等你看」 | 通知設定（變更立即生效） |
| T3 | 乙再關閉 failed：乙面板為 done 開、failed 關；本機儲存同時保有甲的 done 開與乙的 failed 關；甲開著的面板跟著顯示；甲之後的 done 仍發通知 | 通知設定（變更立即生效、設定保留） |

## 紅燈紀錄（2026-10-02，task 3.3 之前，HEAD `1c9ced5`）

前端尚未實作（沒有 `notify.js`、鈴鐺與面板）時跑出的結果：新行為斷言 156 條全部 FAIL，前置斷言 40 條全部 PASS，
`RESULT: FAIL (156)`。完整輸出存在 SDD 工作區（`.superpowers/`，不進 repo）。負向斷言（「不產生通知」「不報錯」）
都把「通知模組已載入」或「面板／鈴鐺確實操作過」併進同一條，避免在沒有實作時空洞通過。

## T 段紅燈紀錄（2026-10-02，HEAD `151abe5` 的產品）

`notify.js` 尚未監聽 `storage` 事件時跑 `--only=T`：新行為斷言 10 條中 5 條 FAIL（T2×2、T3×3：乙的面板與之後的通知仍是舊設定、
乙再改 failed 後本機儲存變成 `done:false`、甲的面板不同步），前置 8 條全 PASS，`RESULT: FAIL (5)`。

## 修正波紅燈紀錄（2026-10-02，HEAD `003e34d`）

N10、N11、S18–S23 加入後，在修正波改前端之前的實作上跑：新行為斷言 180 條中 17 條 FAIL（N10×2、N11、S18 的
`aria-expanded` 與按下狀態 6 條、S19 的 1536 寬、S20、S21×3、S22×3），前置 56 條全 PASS。S18 的 `accent-color`、
S19 的 700 寬與 S23 在當時的實作上已經成立（`accent-color` 在 task 3.3 就已加上；鈴鐺分支本來就不遞增 `latestOp`）。
S23 另以變異驗證：在 `actions.js` 的 `notify-settings` 分支加一行 `latestOp += 1` 後跑 `--only=S`，S23 轉為 FAIL。
完整輸出存在 SDD 工作區（`.superpowers/`，不進 repo）。
