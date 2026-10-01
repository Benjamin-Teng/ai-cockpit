# design：ui-fixes

## Context

動機見 `proposal.md`「Why」，行為以本 change 的 delta spec 為準。以下只列影響做法的現況：

- **焦點還原**：`actions.js` 的委派在 `pointerdown` 呼叫 `preventDefault()`，接著同步 `repaint()`；`render.js` 的
  `restoreFocus` 在 `replaceChildren` 後以程式 `focus({ preventScroll: true })` 放回焦點。2026-10-01 在 headless Chrome 154
  實測：pane 列、左欄專案、所有 task 按鈕被滑鼠點過後 `:focus-visible` 為 true（外框殘留），焦點來源是 `restoreFocus`；
  原生 click 的分頁按鈕不受影響。成因是 Chrome 的啟發式——這次滑鼠操作沒有讓任何元素被滑鼠聚焦，之後的程式焦點就算
  `:focus-visible`。同一實測確認 `FocusOptions.focusVisible: false` 在 Chrome 154 有效，而鍵盤 Tab 後的程式焦點仍為 true。
  全部焦點樣式本來就是 `:focus-visible`，沒有純 `:focus` 規則。
- **通道斷線**：`window.onChannel` 只更新頂列 `data-channel-state`、燈號 title 與底列；右欄 `.runtime-conn-*` 與中欄狀態色
  不受通道影響。
- **綁定解析**：`BindingResolution::RuntimeDisconnected` 與 `ProjectedBinding::RuntimeDisconnected` 只有 `runtime`；前端以
  `binding.source === "override"` 決定是否顯示「取消改綁」。後端取消覆蓋不看連線狀態。
- **git 執行**：本機 git 以 `git -C <path>` 直接執行、不清環境；只有 WSL 分支前綴 `env LC_ALL=C`。refs 解析在 `peeled` 非空時
  直接用剝開後的 oid，不檢查物件型別。
- **撞號判定**：`cockpit/src/agent.rs` 只走投影的 workspace→tab→pane 樹收集「擁有此 pane id 的 runtime」。

## Goals / Non-Goals

**Goals:**

- 鍵盤使用者的焦點提示一點都不退步；只拿掉滑鼠路徑上的外框。
- 斷線轉暗用單一機制涵蓋右欄與中欄，跨整頁重畫仍正確。
- 每個邊界修補都有能重現原缺陷的測試（先紅後綠）。

**Non-Goals:**

- 不重構焦點還原或重畫架構；只在既有焦點還原呼叫點加條件，並為 Git 詳情重建補上焦點還原（D7）。
- 不調整頂列燈號既有的斷線呈現。
- 不為 WSL 端 git 加環境清理（理由見 Risks）。
- 使用者以 `ref=` 明確選取剝開後不是 commit 的 tag 時，維持現行行為（不在本 change 定義專用錯誤）；本 change 只處理預設起點。

## Decisions

### D1 滑鼠焦點：記錄最後一次輸入方式，程式焦點帶 `focusVisible: false`

前端模組層保存「最後一次輸入方式」：`pointerdown` 設為 pointer，`keydown` 設為 keyboard，兩者都掛在 `document` 的 capture
階段——必須早於 `#app` 上 `actions.js` 的 `pointerdown` 委派（它會同步重畫），否則第一次點擊的重畫讀到的仍是舊值。所有**程式還原焦點**
的呼叫點（`render.js` 的 `restoreFocus`、Git 詳情重建、`output.js` 的 `restoreFocusAfterDeselect`）改經一個共用 helper：
最後輸入為 pointer 時帶 `focusVisible: false`，否則維持現狀。前端各檔是獨立 IIFE，helper 與輸入方式狀態放在載入順序最前的
`output.js`（`index.html` 中 `output.js` → … → `render.js` → `actions.js`），以全域掛出（比照既有 `window.onChannel` 的跨檔作法），
其他檔呼叫時取用。由 `/ws` 推送觸發的重畫沿用同一判斷——使用者正在用滑鼠就不出框。

兩條例外（修正波 1 起，比照 Chrome 原生 `:focus-visible` 判定）：(1) 帶 Alt、Ctrl 或 Meta 的 `keydown`（含單按這些鍵）不改變
「最近一次輸入方式」——修飾鍵不算鍵盤導覽，Shift 與一般按鍵才算；(2) 不移動焦點的滑鼠操作（例如按住捲軸）不應讓鍵盤使用者失去焦點
提示，做法是重畫前記下舊元素是否匹配 `:focus-visible`、是則還原時沿用；會移動或重設焦點的滑鼠點擊（在畫面元素上觸發）一律不呈現。

- 替代方案：CSS `[data-input="pointer"] :focus-visible { outline: none }`。否決：它連「滑鼠點擊後按 Tab」的瀏覽器原生外框也遮掉，
  直到下次 keydown 清旗標前都靠時序；且狀態存在 DOM 屬性上，要額外保證跨重畫不丟。
- 替代方案：無條件 `focusVisible: false`。否決：實測鍵盤 Enter 觸發的重畫會失去外框，違反鍵盤焦點要求。
- 不支援 `focusVisible` 的瀏覽器會忽略該選項，退回現行行為（外框殘留），不會更糟。

### D2 斷線轉暗：通道狀態掛在 `#app` 根節點，在範圍內覆寫狀態色 token

`onChannel` 把通道狀態寫到 `#app` 根節點本身的 `data-channel-state`。重畫用 `replaceChildren` 只換子節點，根節點屬性跨重畫保留；
仍在模組變數保存一份，`paint()` 結束時寫回，避免任何路徑重建根節點時遺失。

CSS 在 `#app:not([data-channel-state="connected"])` 範圍內把 spec 列舉的元素轉為 `--text-dim`，與頂列既有呈現一致。現況只有
task 節點經 `--task-status-color` token；右欄 `.runtime-conn-*`、`.agent-dot-*`、`.status-*`，`running` 節點外框的
`var(--accent)`、列首 `.ff-undeclared`、左欄 `.project-count-*` 都直接寫色值。做法：在斷線範圍內逐一列出這些選擇器覆寫
`color`／`background`／`border-color`／`box-shadow`（集中在 `style.css` 一個區塊，註明對應 spec 條文），不另造一套 token；
task 節點則在範圍內覆寫既有的 `--task-status-color`（`.task-status-*` 與 failed／blocked 的外框都吃這個 token）。
runtime 卡的「最後已知」比照頂列 `runtime-lamp-stale` 的作法用手足節點，連線狀態節點的 `textContent` 維持等於連線狀態字串
（既有腳本以此精確比對）。按鈕（含「取消改綁」）不轉暗，斷線時仍可操作。

- 替代方案：逐元素在 JS 加 class。否決：要動每個 render 函式、重畫時逐一維護；CSS 範圍選擇器只依賴根節點一個屬性。
- 替代方案：把所有狀態色改經 token 再於範圍內覆寫 token。否決：牽動整份 `style.css` 的狀態色寫法，超出修補範圍。
- 替代方案：整區 `filter: grayscale()`／降低不透明度。否決：連可操作的按鈕一起變灰，看起來像停用，而斷線時「取消改綁」仍可按。
- 外觀細節（對比、狀態文字可讀性）依 `frontend-design` skill 的設計審核，以設計文件與 spec 為優先。

### D3 `RuntimeDisconnected` 帶 `source`

`BindingResolution::RuntimeDisconnected` 與 `ProjectedBinding::RuntimeDisconnected` 加 `source: BindingSource`：覆蓋造成的斷線為
`Override`，自動解析路徑為 `Auto`。JSON 序列化沿用其他狀態的 `source` 欄位寫法，欄位為必填（不加 `serde(default)`），
`cockpit/tests/fixtures/projected-state.json` 中既有的 `runtime_disconnected` 一併補 `"source": "auto"`（該檔同時餵
`ui_preview`）。前端既有 `source === "override"` 判斷自動生效；
另在斷線狀態也顯示「改綁」徽章。取消覆蓋時清除目前 task 的既有行為（change 6）不變。

### D4 refs 取得物件型別，非 commit 不當起點

for-each-ref 的 format 加 `%(objecttype)` 與 `%(*objecttype)`。refs 回應每筆多一個布林 `commit`：取最終型別（`*objecttype`
非空用它，否則用 `objecttype`），**是 `tree` 或 `blob` 才為 `false`，其餘（`commit`、`tag`）為 `true`**。理由：`%(*objecttype)` 在
舊版 git 只剝一層（修正波 1 實測：git 2.43 對巢狀附註 tag 回 `tag`；git 2.50.1 才剝到底），不能假設一定剝到 commit；`git log` 對
tree 起點本來就直接忽略（不報錯），所以「無法確定時當作可用」無害，反之判成 `false` 會讓只經由該 tag 才到得了的 commit 從預設
Graph 靜默消失。後端組預設起點只收 `commit` 為真者；前端 `computeExpectedTips`
（自行複算後端起點規則以偵測分支變更）同樣只收 `commit` 為真者——兩邊不一致會讓「分支已變更」永久誤報。非 commit 的 ref 仍出現在
refs 清單與篩選選單，顯示不變。refs 回應多一個欄位屬相容擴充。

- 替代方案：前端不複算起點，改由後端在 log 回應附上起點清單供比對。否決：仍需後端判定型別，且要改端點形狀；`commit` 欄位讓前端
  照抄同一規則即可。（`git log` 遇到 tree／blob 起點在 git 2.43 與 2.50.1 都直接忽略、不會失敗，所以排除不是為了避免失敗。）

### D5 本機 git 清 repo 區域環境變數並設 `LC_ALL=C`

本機執行前 `env_remove` git 自己定義的 repo 區域變數清單——以 `git rev-parse --local-env-vars` 為準（2026-10-01 在 git 2.50.1 實測
15 個：`GIT_ALTERNATE_OBJECT_DIRECTORIES`、`GIT_CONFIG`、`GIT_CONFIG_PARAMETERS`、`GIT_CONFIG_COUNT`、`GIT_OBJECT_DIRECTORY`、
`GIT_DIR`、`GIT_WORK_TREE`、`GIT_IMPLICIT_WORK_TREE`、`GIT_GRAFT_FILE`、`GIT_INDEX_FILE`、`GIT_NO_REPLACE_OBJECTS`、
`GIT_REPLACE_REF_BASE`、`GIT_PREFIX`、`GIT_SHALLOW_FILE`、`GIT_COMMON_DIR`），並 `env("LC_ALL", "C")`。

清理與 `LC_ALL=C` 在 `runner.rs` 建立 `Command` 處一律套用，不區分本機與 WSL：對 `wsl.exe` 移除這些變數、設 `LC_ALL` 都無害，
WSL 端既有的 `env LC_ALL=C` 前綴與 argv 不變。`git rev-parse --local-env-vars` 不含 `GIT_NAMESPACE`、`GIT_CEILING_DIRECTORIES` 等，
本 change 不處理（前者只影響 ref 命名空間、後者只影響 repo 探索，Cockpit 都以 `-C <root>` 指定）。

清單寫死在程式內，另加測試：①本機有 git 時跑 `git rev-parse --local-env-vars`，斷言寫死清單涵蓋其全部輸出（git 升級新增變數時
測試會紅）；②以 `Command::as_std().get_envs()` 斷言建出的命令移除了清單中每個變數、`LC_ALL` 為 `C`（語系 scenario 的驗收）；
③端到端：呼叫 runner 的行程本身帶 `GIT_DIR` 指向另一個 repo 時，查詢結果仍是目標 repo。因為要改的是呼叫端行程的環境（runner
建的子行程會被清掉，在那裡設等於沒測），這個測試獨立成一個只含它的整合測試檔，以 `#[test]` 在建立 tokio runtime 之前
`unsafe { std::env::set_var(...) }`（edition 2024 要求 unsafe；單一測試的獨立執行檔沒有並行執行緒），再手動建 runtime 跑查詢。

- 替代方案：每次查詢前先跑 `--local-env-vars`。否決：每次查詢多一次 spawn。
- 替代方案：`env_clear()` 後只帶必要變數。否決：Windows 上 git 需要 `PATH`、`SystemRoot`、`HOME` 等，清單難以窮舉。

### D6 Graph 搜尋：高亮與捲動分開

命中列高亮函式加「是否捲動」參數；只有使用者移到下一筆／上一筆（含 Enter）傳 true。背景分批載入後重算命中只更新高亮與計數。

### D7 檔案清單截斷提示、詳情焦點保留

- commit 詳情與兩點比較的檔案清單讀後端已有的 `truncated`，沿用變更清單提示的文案與樣式（變更清單的提示是虛擬清單裡的 note row，
  詳情與比較清單不是虛擬清單，元素需另建）。
- Git 詳情目前沒有任何重建後的焦點還原，這是新增：重建前若焦點在詳情區內，記下身分（詳情內元素多半沒有 `data-action`，
  需另加穩定的身分屬性，例如檔案路徑或 ref 名稱＋元素種類），重建後找回對應元素，經 D1 的 helper 還原；找不到時聚焦詳情區容器，
  不落到 `body`。

### D8 撞號判定涵蓋綁定

收集「擁有此 pane id 的 runtime」時，除了投影 pane 樹，併入「投影中有 workstream 以 `Bound` 指向此 runtime 的此 pane id」者。
WSL runtime 仍先排除（change 6 既有規則）。只用同一份最新投影，不另查 Domain。

範圍說明：runtime 斷線不會清空它的 pane 樹（`RuntimeStore` 只在收到 snapshot 時整份取代），所以斷線 runtime 最後已知的 pane
本來就參與判定；斷線時綁定投影為 `runtime_disconnected`、不帶 pane id，也無從併入。本修補補的是「`Bound` 但 pane 不在投影樹」
的孤兒（store 註明理論上不發生），屬防禦性修補。測試在 `agent.rs` 內以手工組的 `ProjectedState` 做單元測試，不靠 store 事件造孤兒。

### D9 v1 狀態檔 `active` 以「欄位存在」判斷

`StateProject.active: Option<BTreeMap<…>>` 分不出 `null` 與缺席（serde 對 Option 欄位把兩者都收成 `None`）。形狀檢查在型別化的
`StateFile`／`StateProject` 上（不是 `serde_json::Value`），改用雙層 `Option<Option<…>>`，並同時加 `#[serde(default, deserialize_with = …)]`
——只加 `deserialize_with` 時缺席會報 `missing field`，把合法 v1 舊檔誤判為損毀（已實測）。結果：缺席＝`None`、`null`＝`Some(None)`、
有值＝`Some(Some(_))`。v1 檢查改為「`active` 不是 `None` 即損毀」；v2 檢查改為「`active` 不是 `Some(Some(_))` 即損毀」，使 v2 的
`"active": null` 維持失敗。型別改動連帶 `progress.rs` 的讀取處與 `progress_service.rs` 寫出處（`Some(active)` 變 `Some(Some(active))`）。
錯誤訊息比照既有損毀處理。
`"active": {}` 在 v1 現行已會失敗，只有 `null` 會先紅；`{}` 與 v2 `null` 作為回歸測試。

### D10 競態測試：改名與註解，不加掛鉤

`stale_pane_start_right_after_rebind_is_403` 分不出鎖內重驗與投影已更新兩條路。要確定性地走鎖內路徑，得在正式程式加暫停投影的
測試掛鉤，只為一個測試不值得（鎖內重驗已由 service 層 basis 測試確定性覆蓋）。改測試名稱與文件註解，如實寫它驗的是「改綁後舊 pane
的 start 不會留下目前 task」，並在註解指向 service 層測試。

## Risks / Trade-offs

- [`focusVisible` 屬 Chromium 系支援] → 目標瀏覽器是 Chrome（日後 Tauri 在 Windows 為 WebView2，同為 Chromium）；不支援時退回現行行為。
- [斷線轉暗改動多個驗收腳本的通道段斷言（`visual-check.js` R1／I2 等）] → 只改 spec 已改變的斷言，腳本修改與產品修改分開 commit，
  逐條列出。驗收必含「斷線中觸發整頁重畫」段（例如斷線時點 pane 列），確認轉暗不被重畫洗掉。
- [WSL 端 git 未清 Windows 繼承的 repo 變數] → Windows 環境變數只有列在 `WSLENV` 才會傳進 WSL，預設不傳；不為極端設定加 `env -u`。
- [覆寫狀態色 token 可能漏掉寫死色值的元素] → 驗收時在斷線狀態逐區檢查計算後的顏色，不靠目測。
- [投影 JSON 多 `source` 欄位、refs 回應多 `commit` 欄位] → 相容擴充，消費者只有本 repo 前端；精確 JSON 斷言的測試與 fixture 隨之更新。

## Migration Plan

無資料遷移。狀態檔格式不變（v1 帶 `"active": null` 本就不合規格，現在改為啟動失敗並指出路徑，使用者刪掉該鍵即可）。
