# change `pipeline-projection` 驗收紀錄

分支 `feat/pipeline-projection`。本檔依 task 進度逐節追加，做法沿用 `docs/research/2026-09-15/
change-1b-acceptance.md` 的慣例：自動化腳本只依 PID 清理自己 spawn 的程序、生命週期包在
`try/finally`、路徑與使用者名稱去識別化。

## `channel.js` 退避歸零時機與 `JSON.parse` 防護（task 5.1，D11 1b deferred minor）

spec `cockpit-dashboard`「連上即斷不歸零退避」「壞訊息不中斷」（節錄）：

- 斷線後以 1、2、4、8 秒退避（上限 8 秒）重連，且**只在重連後收到第一則訊息時才把退避歸零**
  （連上後立刻被關閉不歸零）。
- 收到無法解析為 JSON 的訊息時略過該則並在 console 記警告，通道不中斷。

### 修法前的狀態（bug）

`cockpit/assets/app/channel.js` 原本把 `backoffIndex = 0` 放在 `socket.onopen` 裡，也就是
「連上」就歸零，而不是「收到第一則訊息」才歸零——與 spec「連上即斷不歸零退避」矛盾：如果服務
接受 WebSocket 後立即關閉、什麼訊息都不送，原本的程式碼仍會在 `onopen` 那一刻把退避歸零，下一次
重連又會從 1 秒開始，永遠長不大。`onmessage` 也是裸的 `JSON.parse(event.data)`，沒有 try/catch，
解析失敗會丟出例外（瀏覽器只會印一則 `Uncaught SyntaxError`，不是 spec 要求的 `console.warn`
且沒有「略過該則、下一則繼續」的顯式保證）。

### 修法

把退避歸零移到 `onmessage`，且只在「這次連線第一次收到訊息」時歸零一次（每個 `connect()`
呼叫各自一個 `backoffResetPending` 旗標，不影響跨連線共用的 `backoffIndex`）；`JSON.parse`
包一層 `try/catch`，失敗就 `console.warn` 並 `return`（不呼叫 `onState`、不動用
`disconnectHandled`／不關閉連線）。

### Fix round 1（Codex adversarial review，兩條 finding）

1. **[high] 退避歸零放在 `JSON.parse` 之後**：round 1 的第一版把 `backoffResetPending` 的
   歸零判斷放在 `try { state = JSON.parse(...) }` **之後**，等於「解析成功才算收到第一則
   訊息」。但 spec 寫的是「收到第一則訊息」，沒有限定要解析成功——壞訊息也是「收到訊息」，
   一樣代表這次連線真的復原了。修法：把歸零判斷搬到 `onmessage` 的最前面，`JSON.parse`
   之前，對合法與不合法訊息一視同仁。
2. **[medium] 兩個 Rust 測試只驗證原始碼字串，擋不住錯誤實作**：`cockpit/tests/http.rs` 的
   `channel_js_backoff_resets_on_first_message_not_on_open`、
   `channel_js_skips_unparseable_messages_with_warning` 靠切函式主體字串位置比對
   `backoffIndex = 0`／`try`／`catch`／`console.warn` 是否出現，能被上面 finding 1 那種
   「字面上符合但語意錯」的實作矇混過去（歸零確實在 `onmessage` 裡、也確實在 `try/catch`
   之後——字串比對看不出「之後」是不是太晚）。裁決：**移除**這兩個測試，行為證據一律改由
   瀏覽器腳本（headless Chrome 觀察真實 `WebSocket` 行為）提供，不用 Rust 測試宣稱驗證
   JS 的執行語意。`embedded_assets_are_the_final_files` 仍保留原本「`/ws`、`onChannel`
   存在」這種單純的資源內嵌一致性檢查，不宣稱驗行為。

`cargo test -p cockpit --test http`（round 1 修完之後）：

```text
running 5 tests
test render_js_never_prints_completion_word ... ok
test unknown_path_is_404 ... ok
test embedded_assets_are_the_final_files ... ok
test routes_return_200_with_expected_content_types ... ok
test api_state_equals_current_projection_version ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

### 瀏覽器實測：`channel-backoff-check.js`

擴充 `docs/research/2026-09-15/reconnect-check.js` 的做法：只用 Node 22 內建 API（`http`、
`net` 隱含、`crypto`、`child_process`；讀 CDP 時當客戶端用內建 `WebSocket`），不引入任何 npm
套件。Node 22 沒有內建 WebSocket **server**，這裡用 `http` 模組的 `'upgrade'` 事件手寫最小握手
（`Sec-WebSocket-Accept = base64(sha1(client_key + GUID))`）與 server→client 文字訊框（server
端不需要 mask，RFC 6455 §5.1）。

腳本用一個自製的極簡 harness 頁面（不是完整 `index.html`／`render.js`），把 `window.onChannel`、
`window.onState`、`console.warn` 直接記到陣列供 CDP `Runtime.evaluate` 讀，理由：task 5.1 的
範圍只有 `channel.js` 本身，用完整的 app 畫面會被 `render.js` 的渲染邏輯干擾判讀，也不必要；
頁面載入的 `channel.js` 仍是即時讀 `cockpit/assets/app/channel.js` 的實際檔案內容
（`fs.readFileSync`），不是另外複製一份，確保測的是正式內嵌檔案。

情境一與情境二／三的測試 server 行為互斥（一個要「接受後立即關閉」、一個要「維持連線送
訊息」），所以腳本內建各自獨立的 harness server 設定，各自開一個乾淨的 headless Chrome
session（不同埠、各自 `try/finally`、只依 PID 收自己 spawn 的 Chrome，暫存 user-data-dir
刪除失敗算 FAIL；清理判準見下方 fix round 1 的說明）：

- **情境一（無訊息不重設）**：server 對每個 WebSocket upgrade 只完成握手（回 101）就立刻
  `socket.end()`，不送任何訊息；記錄每次連線到達的時間戳，算出連續兩次之間的秒差，驗證
  1、2、4、8、8 秒的完整序列。
- **情境二（壞訊息後正常訊息會重畫）**：server 完成握手後維持連線，500 ms 後送一則
  `{not json`，再 700 ms 後送一份合法 JSON（`{"version":9,"marker":"task-5.1-good-state"}`），
  之後讀 harness 頁記錄的 `onChannel`／`onState`／`console.warn` 陣列。
- **情境三（fix round 1 新增，退避升到 ≥4 秒後收到訊息應重設為約 1 秒）**：直接對應
  finding 1——只驗「無訊息不重設」測不出「歸零判斷放在 JSON.parse 之前還是之後」，因為那個
  情境根本沒有訊息。這裡讓 server 對前三次連線都立即關閉、不送訊息（退避依序升到 1、2、4
  秒），第四次連線送一則訊息後才關閉，驗證：a) 收訊息前退避確實已經升到約 4 秒（第三→第四次
  間隔）；b) 收訊息後下一次重連間隔回到約 1 秒、不是 8 秒（第四→第五次間隔）。跑兩輪，訊息
  分別是合法 JSON 與 `{not json`——兩輪都要重設成功，才證明重設點真的在 `JSON.parse` 之前，
  不因訊息能否解析而有差。

執行（repo 根）：

```text
node docs/research/2026-09-16/channel-backoff-check.js

[...] === 情境一：連上即斷，觀察退避間隔 ===
ok   觀察到至少 6 次連線嘗試（實際 6）
連線嘗試間隔（秒）：1.01, 2.00, 4.01, 8.01, 8.01
ok   間隔 #1 應約為 1s（實際 1.01s，容許 ±0.4s）
ok   間隔 #2 應約為 2s（實際 2.00s，容許 ±0.4s）
ok   間隔 #3 應約為 4s（實際 4.01s，容許 ±0.4s）
ok   間隔 #4 應約為 8s（實際 8.01s，容許 ±0.4s）
ok   間隔 #5 應約為 8s（實際 8.01s，容許 ±0.4s）
ok   chrome PID 43624 已終止（tasklist 查無此 PID）

[...] === 情境二：壞訊息不中斷 ===
events=[{"status":"connected"}]（全程只有一筆，未曾 disconnected）
warnings=["channel.js: 收到無法解析為 JSON 的訊息，已略過 SyntaxError: ..."]（1 筆）
states=[{"state":{"version":9,"marker":"task-5.1-good-state"}}]（只有合法投影進 onState）
ok   通道狀態全程只有 connected、未曾 disconnected
ok   壞訊息應該觸發至少一次 console.warn（實際 1 次）
ok   壞訊息之後只有合法投影送進 onState、且內容正確
ok   chrome PID 7756 已終止（tasklist 查無此 PID）

[...] === 情境三（合法 JSON）：退避升到 ≥4 秒後收到訊息應重設為約 1 秒 ===
ok   觀察到至少 5 次連線嘗試（實際 5）
間隔（秒）：#1=1.02 #2=2.02 #3（升到 4s）=4.00 #4（收訊息後應回 1s）=1.02
ok   退避在收到訊息前應該已經升到約 4s（實際 4.00s）
ok   收到合法 JSON 訊息後，下一次重連間隔應該回到約 1s，不是 8s（實際 1.02s）
ok   chrome PID 66488 已終止（tasklist 查無此 PID）

[...] === 情境三（壞 JSON）：退避升到 ≥4 秒後收到訊息應重設為約 1 秒 ===
ok   觀察到至少 5 次連線嘗試（實際 5）
間隔（秒）：#1=1.01 #2=2.01 #3（升到 4s）=4.00 #4（收訊息後應回 1s）=1.00
ok   退避在收到訊息前應該已經升到約 4s（實際 4.00s）
ok   收到壞 JSON 訊息後，下一次重連間隔應該回到約 1s，不是 8s（實際 1.00s）
ok   chrome PID 50036 已終止（tasklist 查無此 PID）
RESULT: PASS
```

（完整時間戳已省略，只留判讀所需的行。清理判準：round 1 起改成 `taskkill` 之後另外用
`tasklist /FI "PID eq <pid>"` 查主行程是否真的不在了，而不是看 `taskkill` 自己的結束碼——
Chrome 是多行程架構，`/T` 殺行程樹時常有一兩支子行程剛好搶先退出或受保護，`taskkill` 會回報
非 0（甚至觀察到 255），但主行程確實已經終止；第一版只看 `taskkill` 結束碼，在這次改動中
遇到一次因此誤判為清理失敗的情況，改用 `tasklist` 覆核後穩定重現 `RESULT: PASS`。）

執行後以 `tasklist | grep -i chrome` 確認沒有殘留的 chrome.exe；`netstat -ano` 對應埠沒有
殘留的 `LISTENING`。

### 結論

- 情境一（無訊息不重設）實測間隔 1.01 / 2.00 / 4.01 / 8.01 / 8.01 秒，符合 spec「1、2、4、8、
  8 秒遞增、不回到 1 秒」。
- 情境二（壞訊息後正常訊息會重畫）實測壞訊息觸發一次 `console.warn`、通道狀態全程維持
  `connected`（未曾變成 `disconnected`）、只有合法投影送進 `window.onState` 且內容正確，符合
  spec「第一則被略過、console 有警告，第二則正常重畫，通道狀態維持已連線」。
- 情境三（fix round 1 新增）兩輪都證實：退避升到約 4 秒後，收到一則訊息（不論合法或不合法
  JSON）都會讓下一次重連間隔回到約 1 秒——直接證明歸零判斷確實在 `JSON.parse` 之前，不是只有
  解析成功的訊息才算數，回應 Codex finding 1。
- 行為證據全部來自這支瀏覽器腳本，觀察的是真實 headless Chrome 執行 `channel.js`（讀正式內嵌
  檔案，不是複製版本）之後的實際 `WebSocket` 行為，不依賴任何對原始碼字串的比對。

### Gate 證據

```text
cargo fmt --check                                      → exit 0
cargo clippy -p cockpit --all-targets -- -D warnings   → Finished, 0 warning
cargo test -p cockpit                                  → app 12、config 46、fixture 1、http 5、
                                                          progress_file 14、progress_service 11、
                                                          pwa 3、real_attach 0（1 ignored，需真機
                                                          HERDR）、runtimes 4、ws 4，共 100 passed /
                                                          0 failed / 1 ignored
```

`app.rs` 是另一名實作者平行進行中的 task 3.3（`progress_service`）測試，本次一併跑過確認沒有
互相破壞，但不屬於本節驗收範圍。

## Factory Floor 網格與 Scenario D（task 5.2）

spec `cockpit-dashboard`「Factory Floor」「Scenario D 的畫面」「兩個 Project」（節錄）：

- 每個 Project 一塊區域，上下排列、畫在 runtime 卡之前；區域內是網格，欄為 `stages`、列為
  workstream；每列列首顯示 workstream 名稱與綁定摘要（`bound`／`unbound`／`ambiguous`／
  `runtime_disconnected`／`none` 五種文字）；每個 task 以節點出現在所屬 workstream 列與目前
  `stage` 欄交會的格子，節點顯示 `title` 與 `status`，六色（`running` 綠、`blocked` 琥珀、
  `ready` 灰、`pending` 暗灰、`failed` 紅、`completed` 紫，`completed` 不得用 pane `done` 的
  藍色），`running` 另有動態強調；未知 `status` 暗灰顯示原字串。
- Scenario D：stages `Plan`／`Implement`／`Test`，workstream `backend`／`frontend`／`tests`
  各一個 `running` task，分別在 `Implement`／`Plan`／`Test`——網格應為三列三欄，三個綠色節點
  分別落在 (backend, Implement)、(frontend, Plan)、(tests, Test)。

### 實作

`cockpit/assets/app/render.js`：`renderState` 改為 `renderState(state, ui)`（`ui` 選填，本
task 不讀取任何欄位——改綁模式／錯誤訊息／`data-action` 按鈕屬於 task 5.3 的 `actions.js`
範圍，design D9 明講「可先輸出屬性但不處理互動」，屬於允許而非要求；task 原文對 5.2 的敘述
也沒有提到按鈕，所以這裡刻意不畫任何按鈕，避免出現使用者按了沒反應的死按鈕）。新增
`renderProject`／`renderFactoryFloor`／`renderWorkstreamRowHeader`／`renderFactoryCell`／
`renderTaskNode`／`renderBindingSummary`，畫在 `renderTopbar` 之後、`runtime-cards` 之前，依
`state.projects` 順序逐一 `appendChild`。網格用 CSS Grid，欄數（`stages.length + 1`）由 JS
算好寫進 `factory-floor` 的 inline `grid-template-columns`（欄數隨 Project 而變，寫死在
`style.css` 沒有意義）；每格帶 `data-workstream`／`data-stage`，每個 task 節點帶
`task-status-<status>` class，這些屬性單純是結構標記與可測試性，不代表任何互動。

`cockpit/assets/app/style.css`：新增 6 個 `--stage-*` CSS 變數，`running`／`blocked`／
`ready` 直接複用既有的 `--status-working`／`--status-blocked`／`--status-idle`；`pending`
複用 `--status-unknown`（沿用既有「暗灰」語彙，未知 task status 的 fallback class 也用同一個
值）；`failed`／`completed` 是本 change 新增顏色（`completed` 用 `#a371f7` 紫，不是
`--status-done` 的藍）。`running` 節點另加 `@keyframes task-running-pulse` 的 box-shadow
脈動作為動態強調。

`cockpit/tests/fixtures/projected-state.json`：`projects` 從一個擴充為兩個——沿用既有
`cockpit` project，新增第二個 project `p`（`name: "Scenario D Demo"`），內容直接對應 spec
「Scenario D 的畫面」情境：stages `Plan`／`Implement`／`Test`，`backend`／`frontend`／
`tests` 三條 workstream 各一個 `running` task，分別在 `Implement`／`Plan`／`Test`。三條
workstream 的綁定刻意選不同的五種狀態之三（`bound`／`ambiguous`／`runtime_disconnected`），
補足人眼檢查時的視覺變化，不是行為要求。`examples/ui_preview.rs` 本身不用改——它是
「fixture 驅動」（`include_str!` 讀同一份 fixture），加 project 只需要改 fixture 檔案。

`cockpit/tests/fixture.rs`：`fixture_projected_state_deserializes` 原本斷言
`state.projects.len() == 1`，先改成 `== 2`（RED：fixture 還沒加第二個 project，斷言失敗），
確認失敗訊息符合預期後才把上面的 fixture 改動加上去（GREEN）；另外加一段斷言驗證第二個
project 的 `id`／`stages`／三個 task 的 `(workstream, stage)` 位置／全部 `status ==
StageStatus::Running`。

### TDD 證據：`cockpit/tests/fixture.rs`

RED（只改斷言、fixture 還是舊的一個 project）：

```text
cargo test -p cockpit --test fixture

thread 'fixture_projected_state_deserializes' panicked at cockpit\tests\fixture.rs:64:5:
assertion `left == right` failed: fixture 應該恰好有兩個 project
  left: 1
 right: 2
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out
```

GREEN（fixture 加上 Scenario D project 之後）：

```text
cargo test -p cockpit --test fixture

running 1 test
test fixture_projected_state_deserializes ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

### 瀏覽器實測：`factory-floor-check.js`

不透過 `cargo run`（會多一層 wrapper 行程，子行程才是真正監聽 port 的那個，PID 追蹤容易對不
上），改直接執行 `cargo build -p cockpit --example ui_preview` 產生的
`target/debug/examples/ui_preview.exe`——同一支程式、同一份 fixture，只是省了 cargo 那層
wrapper，方便腳本用單一 PID 收尾。headless Chrome 用 `--dump-dom --virtual-time-budget=4000`：
讓 Chrome 在收到 `index.html` 的 load 事件後，繼續跑一段虛擬時間（WebSocket 連線、收到第一份
投影、`render.js` 整頁重畫都在這段時間內完成），時間到才把最終 DOM 序列化輸出，不用另外起
CDP 連線輪詢。取格子內容用簡單的括號深度計數（`extractCellHtml`），不用正規表達式硬吃到下一個
`</div>`——cell 裡可能巢狀一層 task-node 的 `</div>`，正規表達式會提早收尾。埠沿用 task 5.1
的做法：先探測 7770 是否已被占用，占用就往上加一（brief 要求）；本次實測 7770 空閒。

執行（repo 根，需先 `cargo build -p cockpit --example ui_preview`）：

```text
node docs/research/2026-09-16/factory-floor-check.js

ok   ui_preview 應該在 10 秒內開始回應 /api/state（port 7770）
ok   --dump-dom 應該成功結束（實際 exit 0）
ok   dump-dom 輸出應該含 Factory Floor 網格
[...] === 斷言 1：Scenario D 網格位置 ===
ok   (backend, Plan) 不應該有 task 節點（實際沒有）
ok   (backend, Implement) 應該有 task 節點（實際有）
ok   (backend, Test) 不應該有 task 節點（實際沒有）
ok   (frontend, Plan) 應該有 task 節點（實際有）
ok   (frontend, Implement) 不應該有 task 節點（實際沒有）
ok   (frontend, Test) 不應該有 task 節點（實際沒有）
ok   (tests, Plan) 不應該有 task 節點（實際沒有）
ok   (tests, Implement) 不應該有 task 節點（實際沒有）
ok   (tests, Test) 應該有 task 節點（實際有）
ok   running 節點總數應該是 4（cockpit 專案 1 個 + Scenario D 3 個，實際 4）
[...] === 斷言 2：兩個 Project 上下順序 ===
ok   應該恰好有兩個 data-project（實際 2：["cockpit","p"]）
ok   Project 順序應該是 cockpit 在上、p（Scenario D）在下（實際 ["cockpit","p"]）
ok   Factory Floor（.projects）應該整塊畫在 runtime 卡（.runtime-cards）之前
ok   cockpit 專案應該在 p（Scenario D）之上，兩者都在 runtime 卡之前
ok   --screenshot 應該成功結束（實際 exit 0）
ok   截圖應該存在：D:\projects\ai-cockpit\docs\research\2026-09-16\task-5.2-scenario-d.png
ok   ui_preview PID 65600 已終止（tasklist 查無此 PID）
ok   port 7770 應該不再有 LISTENING 的行程
RESULT: PASS
```

截圖：`docs/research/2026-09-16/task-5.2-scenario-d.png`——人眼核對兩塊 Factory Floor（`AI
Cockpit`、`Scenario D Demo`）上下排列、running 綠／pending 暗灰／completed 紫（非 pane done
的藍）、Scenario D 三個綠色 RUNNING 節點分別在 (Backend, Implement)、(Frontend, Plan)、
(Tests, Test)，畫面色彩與版面符合 spec 敘述。

執行後以 `tasklist | grep -i "cockpit\|cargo"` 確認沒有殘留行程；`netstat -ano` 對應埠沒有
殘留的 `LISTENING`（腳本自身的收尾斷言已覆核，這裡另外手動覆核一次）。

### 結論

- Scenario D 的三個 task 節點精確落在 (backend, Implement)、(frontend, Plan)、
  (tests, Test)，其餘六格皆無節點——不只是「有畫出來」，而是排除了畫錯格子的可能。
- 兩個 Project（`cockpit`、Scenario D 的 `p`）依 `state.projects` 順序上下排列，整塊都在
  runtime 卡之前，符合 spec Scenario「兩個 Project」。
- `completed` 節點用紫色（`#a371f7`），與 pane `done` 的藍色（`--status-done` /
  `#58a6ff`）不同；`render_js_never_prints_completion_word`（既有測試）確認 render.js 全檔
  沒有「完成」字樣。
- 未知 task status 的處理沿用既有檔案「未知值不壞畫面」的原則（`task-status-unknown`
  class＋`title` 屬性顯示原字串），與既有 `renderState` 對未知 `agent_status` 的處理手法
  一致；fixture 目前沒有這個情境的樣本，行為由程式碼結構（`taskStatusClass` 的
  `indexOf` 檢查＋`default` 分支）保證，未另外造假資料測試——task 原文只要求「未知 status
  暗灰」，沒有要求額外測試情境，且 6 個已知狀態的 fallback 路徑與既有 pane 狀態的
  fallback 路徑完全同構，風險低。

### Gate 證據

```text
cargo fmt --check                                      → exit 0
cargo clippy -p cockpit --all-targets -- -D warnings   → Finished, 0 warning
cargo test -p cockpit                                  → app 14、config 46、fixture 1、http 5、
                                                          progress_file 14、progress_service 11、
                                                          pwa 3、real_attach 0（1 ignored，需真機
                                                          HERDR）、runtimes 4、ws 4，共 100 passed /
                                                          0 failed / 1 ignored
```

`app.rs`（14 tests）是另一名實作者平行進行中的 task 3.4 修正，本次一併跑過確認沒有互相破壞，
不屬於本節驗收範圍。

### Fix round 1（Codex review：驗收證據不足）

Codex 靜態確認 render.js／style.css 的實作正確，但指出 round 0 的
`docs/research/2026-09-16/factory-floor-check.js:166-216` 只檢查「格子裡有沒有任意
task-node」「running 節點總數」「Project 順序」，沒有逐項比對每個 task 節點的狀態
class／文字，也沒有涵蓋 `blocked`／`ready`／`failed`／未知 status、`bound(override)`／
`unbound` 兩種綁定、warning 內容——控制端列為 Important，等同上一節自我檢查寫的
concern 2。

**修法**：

1. `cockpit/tests/fixtures/projected-state.json` 的第一個 project（`"cockpit"`）補兩條
   workstream：`qa`（`bound` + `source: "override"`，binding 指到既有的
   `win`／`wJ:p3`／`codex`／`unknown`，跟 runtime 那邊的真實 pane 資料一致，不是憑空編）、
   `release`（`unbound`）；補三個 task：`qa-1`（`blocked`）、`release-1`（`ready`）、
   `ops-2`（`failed`，掛在既有 `ops` workstream 的另一個 stage）。連同原本的
   running／pending／completed，`StageStatus` 六個已知值現在全部有真實樣本；
   `binding.state` 五種之中的四種（`bound(auto)`／`runtime_disconnected`／`none`／
   `bound(override)`／`unbound`）在這個 project 內，第五種 `ambiguous` 沿用第二個
   project（Scenario D 的 `frontend` workstream）既有的樣本，沒有重複造一份。Scenario D
   的三條 workstream／三個 task 完全沒動，`examples/ui_preview.rs` 不用改（fixture 驅動）。
2. **「未知 status」技術上無法塞進這份 fixture**：`ProjectedTask.status` 的 Rust 型別是
   `StageStatus`，一個沒有 `#[serde(other)]` catch-all 的封閉 enum（`cockpit-core/src/
   domain/status.rs`），寫一個列舉值以外的字串會讓 `serde_json::from_str::
   <ProjectedState>` 直接失敗——`cockpit/tests/fixture.rs` 的 `expect(...)` 與
   `ui_preview.rs` 啟動時的 `expect(...)` 都會 panic，不是偷懶不補而是型別系統本來就不
   允許。這個情境改用 CDP 直接呼叫 `render.js` 唯一暴露在 `window` 上、會觸發整頁重畫的
   入口 `window.onState(syntheticState)`，餵一個手寫、沒經過 Rust 序列化的 JS 物件（同格
   放一個已知 `running` 的 task 當對照組，驗證「其他節點正常」）。
3. `cockpit/tests/fixture.rs` 同步更新斷言（`workstreams.len() == 5`、`tasks.len() ==
   6`、新增 workstream／task 的 binding／status 斷言、warning 內容的精確字串比對），先
   改斷言見紅、fixture 加上內容後轉綠（TDD，見下方證據）。
4. `docs/research/2026-09-16/factory-floor-check.js` 大幅擴充：
   - `extractCellHtml`／`extractRowHeaderHtml` 統一收斂成 `extractBalancedDiv`（依括號
     深度計數取配對的 `</div>`，不怕巢狀）。
   - 新增 `assertTaskNode(html, ws, stage, status, title)`：逐項比對節點的
     `class="task-node task-status-<status>"`、狀態文字、標題文字——不再只看「有沒有
     節點」。六個已知 status 各跑一次，加上 Scenario D 原本的三個 running 節點也補上
     逐項比對（round 0 只驗證位置，沒驗證 class／文字）。
   - `render.js` 的 `renderWorkstreamRowHeader` 補上 `data-workstream` 屬性（結構標記，
     不是互動屬性——跟 `ff-cell` 的 `data-workstream`／`data-stage` 同一個性質）：round 0
     沒有這個屬性時，只能用 workstream 顯示名稱找列首，但 cockpit 專案的 `be` 與
     Scenario D 的 `backend` 顯示名稱都是「Backend」，名稱查找會抓到錯的那一列；加上
     `data-workstream` 才能用 id 精確定位。
   - 新增 `assertBinding(html, ws, expectedText, expectBadge)`：逐項比對
     `.ff-binding-text` 的精確文字，以及 `source: "override"` 時的「改綁」
     `.ff-binding-badge` 是否出現——五種綁定摘要文字＋override 標示全部跑過。
   - 新增 warning 內容的精確字串比對（`<li class="project-warning">...</li>` 完整比對）。
   - 新增 `scenarioUnknownStatus()`：獨立起一個只服務 harness html＋真正
     `cockpit/assets/app/render.js` 檔案內容的極簡 http server（跟 task 5.1
     `channel-backoff-check.js` 同一套手法，不引入任何 npm 套件），headless Chrome 開起來
     後用 CDP `Runtime.evaluate` 呼叫 `window.onState(syntheticState)`，讀 `#app` 的
     `innerHTML` 斷言：未知 status 節點的 class 是 `task-status-unknown`、`title` 屬性是
     原字串、文字顯示原字串；同格的已知 `running` 節點正常渲染、不受影響；這格恰好兩個
     節點（沒有因為未知值整格消失或多長出東西）。
     - 實作過程踩過一個坑：一開始呼叫 `window.onState(...)` 後立刻讀 `#app`，斷言全部
       落空（`#app` 是空字串）——`attachCdp` 只等「page target 出現在 `/json/list`」，不
       等 `render.js` 這顆 `<script>` 真的執行完，此時 `window.onState` 還沒掛上去，呼叫
       它相當於呼叫一個 `undefined`，靜默失敗。修法：呼叫前先輪詢
       `typeof window.onState === 'function'` 到 `true`（上限 5 秒），並且改用
       `cdp.send('Runtime.evaluate', ...)` 直接檢查回傳是否帶 `exceptionDetails`，避免
       同類錯誤再次靜默過關。

### TDD 證據：`cockpit/tests/fixture.rs`（fix round 1）

RED（只改斷言、fixture 還沒加 `qa`／`release`／`qa-1`／`release-1`／`ops-2`）：

```text
cargo test -p cockpit --test fixture

thread 'fixture_projected_state_deserializes' panicked at cockpit\tests\fixture.rs:78:5:
assertion `left == right` failed: fixture 應該有五條 workstream（be／docs／ops／qa／release）
  left: 3
 right: 5
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out
```

GREEN（fixture 補上內容之後）：

```text
cargo test -p cockpit --test fixture

running 1 test
test fixture_projected_state_deserializes ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

### 瀏覽器實測（fix round 1）：`factory-floor-check.js` 完整輸出

```text
node docs/research/2026-09-16/factory-floor-check.js

=== 情境一：Factory Floor 網格位置、六種已知 status、五種綁定摘要（含 override）、Project 順序、warning ===
ok   ui_preview 應該在 10 秒內開始回應 /api/state（port 7770）
ok   --dump-dom 應該成功結束（實際 exit 0）
ok   dump-dom 輸出應該含 Factory Floor 網格
--- Scenario D 網格位置 ---
（9 格逐一比對，皆 ok；backend/Implement、frontend/Plan、tests/Test 三個 running 節點另外
  逐項比對 class／文字／標題，皆 ok）
--- 六種已知 status（class／文字逐項比對）---
ok   (be, Implement) 節點應該有 class "task-status-running"
ok   (docs, Spec) 節點應該有 class "task-status-pending"
ok   (ops, Review) 節點應該有 class "task-status-completed"
ok   (qa, Implement) 節點應該有 class "task-status-blocked"
ok   (release, Spec) 節點應該有 class "task-status-ready"
ok   (ops, Spec) 節點應該有 class "task-status-failed"
ok   running 節點總數應該是 4（cockpit 專案 1 個 + Scenario D 3 個，實際 4）
--- 五種綁定摘要文字（含 override）---
ok   workstream be 的綁定摘要應該是 "win / wJ:p1"（不應該有改綁標示）
ok   workstream docs 的綁定摘要應該是 "runtime 未連線"
ok   workstream ops 的綁定摘要應該是 "無綁定"
ok   workstream qa 的綁定摘要應該是 "win / wJ:p3"（應該顯示「改綁」標示，實際有）
ok   workstream release 的綁定摘要應該是 "未綁定"
ok   workstream frontend 的綁定摘要應該是 "歧義（2）"
--- warning 內容 ---
ok   project「cockpit」應該顯示指定內容的 warning
--- 兩個 Project 上下順序 ---
ok   應該恰好有兩個 data-project（實際 2：["cockpit","p"]）
ok   Project 順序應該是 cockpit 在上、p（Scenario D）在下
ok   Factory Floor（.projects）應該整塊畫在 runtime 卡（.runtime-cards）之前
ok   cockpit 專案應該在 p（Scenario D）之上，兩者都在 runtime 卡之前
ok   --screenshot 應該成功結束（實際 exit 0）
ok   截圖應該存在：docs/research/2026-09-16/task-5.2-scenario-d.png
ok   ui_preview PID 31416 已終止（tasklist 查無此 PID）
ok   port 7770 應該不再有 LISTENING 的行程
=== 情境二：未知 task status 不破壞畫面（直接呼叫 window.onState，繞過 Rust 型別系統）===
ok   render.js 應該在 5 秒內把 window.onState 掛好
ok   window.onState 呼叫不應該丟例外
ok   #app 應該有內容（重畫沒有整頁壞掉）
ok   應該找到格子 (ws1, Stage1)
ok   同格內已知 status（running）的節點應該正常渲染
ok   未知 status 節點應該落 class "task-status-unknown" 並帶 title="whatever"
ok   未知 status 節點的文字應該保留原字串 "whatever"
ok   未知 status 節點的標題應該正常顯示
ok   這格應該恰好有兩個 task 節點（實際 2）
ok   chrome-unknown-status PID 56696 已終止（tasklist 查無此 PID）
RESULT: PASS
```

截圖 `docs/research/2026-09-16/task-5.2-scenario-d.png` 已重新產生：`cockpit` 專案現在
六種狀態色都看得到（running 綠、pending 灰、completed 紫、blocked 琥珀、ready 亮灰、
failed 紅），`qa` 列的「改綁」徽章清楚可見；Scenario D 區塊未受影響。

### Fix round 1 結論

- 六種已知 task status 的 class／文字、五種綁定摘要文字（含 override 標示）、warning
  內容，現在逐項斷言精確值，不再只是「有沒有畫出東西」的存在性檢查。
- 未知 status 的處理改成真的用瀏覽器跑過（CDP 直呼 `window.onState`），不再只靠「程式
  碼結構跟已知路徑同構」的推論佐證；驗證了 spec「未知 status 不破壞畫面」的兩個要求都
  成立：該節點暗灰顯示原字串、其他節點正常。
- Scenario D 的三個節點位置、兩個 Project 上下順序維持 round 0 已驗證的結論，這次額外
  補上這三個節點各自的 class／文字逐項比對。

### Gate 證據（fix round 1）

```text
cargo fmt --check                                      → exit 0（先前一版有格式問題，
                                                          cargo fmt -p cockpit 修過）
cargo clippy -p cockpit --all-targets -- -D warnings   → Finished, 0 warning
cargo test -p cockpit                                  → app 14、config 46、fixture 1、http 5、
                                                          pipeline_api 16、progress_file 14、
                                                          progress_service 11、pwa 3、
                                                          real_attach 0（1 ignored，需真機
                                                          HERDR）、runtimes 4、ws 4，共 118
                                                          passed / 0 failed / 1 ignored
```

`app.rs`（14 tests）、`pipeline_api.rs`（16 tests）是另一名實作者平行進行中的 task 4.1
（`http.rs`／`app.rs`／`tests/pipeline_api.rs`／`actions.js`）留下的測試，本次一併跑過
確認沒有互相破壞，不屬於本節驗收範圍，我沒有修改那些檔案。

### Fix round 2（Codex review：class／文字對了不代表顏色真的對）

複審結論：其餘項目 ADDRESSED、沒有新破壞，剩一條 Important——`factory-floor-check.js` 的
`assertTaskNode` 只比對 `class` 名稱、狀態文字、標題文字，沒有量測實際顏色。舉例：如果
`style.css` 的 `.task-status-failed` 手滑接到 `--stage-completed`（顏色互換），`class`
名稱仍是 `task-status-failed`、文字仍是 `failed`，round 1 的斷言會全部 PASS，但畫面上
這個節點其實是紫色不是紅色——CSS 變數遺失或互換這種錯誤，class／文字斷言完全擋不住。

**修法**：

1. 新增 `EXPECTED_COLORS`（六種已知 status＋`unknown` 在 `style.css` 定義的實際 hex
   顏色）與 `hexToRgb`（換算成 `getComputedStyle` 會回傳的 `rgb(r, g, b)` 格式）。承載
   顏色的 CSS 屬性確認是 `background`（shorthand，`.task-status-*` 規則只給顏色值，等同
   只設 `background-color`），所以斷言讀 `getComputedStyle(node).backgroundColor`。
2. `scenarioGridAndCoverage` 新增一段：dump-dom 的輸出是序列化文字，量不到 computed
   style，另外開一個「真的」headless Chrome（`--remote-debugging-port`，不是
   `--dump-dom`）連到同一個 ui_preview，用 CDP 對 `cockpit` 專案六個已知 status 的節點
   （`be`／`docs`／`ops`／`qa`／`release`／`ops`-Spec）各自
   `getComputedStyle(...).backgroundColor`，逐一比對 RGB 值。**保留**既有的 class／文字
   斷言，顏色斷言是新增不是取代。
3. `scenarioUnknownStatus` 的 harness 補上 `<link rel="stylesheet"
   href="/app/style.css">` 與 `/app/style.css` 路由——round 1 版本沒有載入樣式表，
   `getComputedStyle` 只會量到瀏覽器預設值（沒有顏色），沒辦法驗證「未知 status 真的是
   暗灰」；補上之後對「對照組 running 節點」與「未知 status 節點」都加上顏色比對。

### 瀏覽器實測（fix round 2）：新增的顏色斷言

```text
node docs/research/2026-09-16/factory-floor-check.js
...
--- 六種已知 status 的實際顏色（getComputedStyle）---
ok   即時頁面應該在 5 秒內畫出全部 9 個 task 節點
ok   (be, Implement) 節點（running）的 background-color 應該是 rgb(63, 185, 80)（style.css #3fb950，實際 rgb(63, 185, 80)）
ok   (docs, Spec) 節點（pending）的 background-color 應該是 rgb(72, 79, 88)（style.css #484f58，實際 rgb(72, 79, 88)）
ok   (ops, Review) 節點（completed）的 background-color 應該是 rgb(163, 113, 247)（style.css #a371f7，實際 rgb(163, 113, 247)）
ok   (qa, Implement) 節點（blocked）的 background-color 應該是 rgb(210, 153, 34)（style.css #d29922，實際 rgb(210, 153, 34)）
ok   (release, Spec) 節點（ready）的 background-color 應該是 rgb(139, 148, 158)（style.css #8b949e，實際 rgb(139, 148, 158)）
ok   (ops, Spec) 節點（failed）的 background-color 應該是 rgb(248, 81, 73)（style.css #f85149，實際 rgb(248, 81, 73)）
ok   chrome-colors PID 65056 已終止（tasklist 查無此 PID）
...
=== 情境二：未知 task status 不破壞畫面（直接呼叫 window.onState，繞過 Rust 型別系統）===
...
ok   對照組 running 節點的 background-color 應該是 rgb(63, 185, 80)（實際 rgb(63, 185, 80)）
ok   未知 status 節點的 background-color 應該是 rgb(72, 79, 88)（style.css #484f58，實際 rgb(72, 79, 88)）
ok   chrome-unknown-status PID 66492 已終止（tasklist 查無此 PID）
RESULT: PASS
```

七個節點（六種已知 status＋unknown）的實際 `background-color` 全部與 `style.css` 定義的
RGB 值精確相符，其餘 round 0／round 1 已驗證的斷言（結構位置、class、文字、綁定摘要、
warning、Project 順序）全部保留並重跑一次，一併 PASS。截圖已重新產生，內容與 round 1
一致（畫面本身沒有改動，這次改的是驗收腳本）。

### Gate 證據（fix round 2）

```text
cargo fmt --check                                      → exit 0
cargo clippy -p cockpit --all-targets -- -D warnings   → Finished, 0 warning
cargo test -p cockpit                                  → 119 passed / 0 failed / 1 ignored
                                                          （app 15、config 46、fixture 1、
                                                          http 5、pipeline_api 16、
                                                          progress_file 14、
                                                          progress_service 11、pwa 3、
                                                          real_attach 0(1 ignored)、
                                                          runtimes 4、ws 4）
markdownlint-cli2 "**/*.md"（repo 根）                  → Linting: 58 files → Summary: 0 issues
```

本輪只改了 `docs/research/2026-09-16/factory-floor-check.js` 這個 JS 檔案，沒有動任何
`.rs`／fixture／`render.js`／`style.css`，Rust gate 數字變化（`app.rs` 從 14→15）純粹是
另一名實作者平行進行中的 task 4.1 留下的未 commit 修改，跑過確認沒有互相破壞，不屬於本節
驗收範圍。

## 畫面操作（task 5.3）

驗 spec `cockpit-dashboard`「畫面操作」四個情境與節點按鈕顯示規則；design D9。

### 實作

- `cockpit/assets/app/actions.js`：持有 UI 狀態（改綁目標 workstream、最近一次操作的錯誤
  訊息），以 `window.cockpitActions.uiSnapshot()` 提供給 render.js；在 `#app` 根節點以
  `pointerdown` 委派（只收主要按鍵），鍵盤另收 `event.detail === 0` 的 `click`；按鈕送
  `POST .../tasks/<task>/<op>`、`PUT`／`DELETE .../workstreams/<ws>/override`（PUT 本體
  `{"runtime","pane_id"}`）。非 2xx 或請求失敗時錯誤訊息含 HTTP 狀態碼與回應本體的
  `error`，保留到下一次操作或按「關閉」；成功不改畫面，等 `/ws` 推送（改綁成功才離開
  改綁模式）。
- `render.js`：`renderState(state, ui)` 開始讀 `ui`——頂列下方的錯誤訊息與改綁提示
  （含「取消」）、task 節點按鈕（規則見 spec）、workstream 列首「改綁」／「取消改綁」、
  改綁模式下 connected runtime 未 exited pane 列的「綁定到這裡」。只輸出 `<button>` 與
  `data-*` 屬性，不綁 listener。`window.onState` 記住最新投影，`window.repaint` 供
  actions.js 在 UI 狀態改變後立即重畫。
- `index.html`：依序載入 render.js → actions.js → channel.js。`style.css`：按鈕、提示列
  只用既有配色變數。
- `examples/ui_preview.rs`：寫入端點改由外層 router 接手，只在 stdout 印
  `write-request <METHOD> <PATH> <BODY>` 並回 204（正式 dashboard router 當
  `fallback_service`；`merge` 遇到同路徑同方法會 panic）；`COCKPIT_PREVIEW_PUSH_MS=100`
  切成每 100 ms 推送一份的模式（預設 2000）。
- fixture 補 `be-2`、`docs-2`、`qa-2`（都是 `mark: none`，後兩個在最後一站），讓畫面上有
  10 個「Completed」可連按，也有「最後一站不顯示推進」的樣本。

### TDD 證據

RED（實作前）：

```text
cargo test -p cockpit --test fixture
  fixture_projected_state_deserializes ... FAILED
  fixture 應該有九個 task（be-1／docs-1／ops-1／qa-1／release-1／ops-2／be-2／docs-2／qa-2）
cargo test -p cockpit --test http
  index_loads_actions_js_between_render_and_channel ... FAILED
  index.html 應該引用 <script src="/app/actions.js"></script>
node docs/research/2026-09-16/actions-check.js
  RESULT: FAIL (47)   （找不到任何 data-action 按鈕、沒有 PUT／POST 被記錄）
```

GREEN：兩個 Rust 測試通過（見 Gate），瀏覽器腳本 `RESULT: PASS`。

突變驗證（證明「頻繁重畫」情境真的擋得住 design D9 說的問題）：把 actions.js 的
`pointerdown` 委派改成只收 `click`（滑鼠 click），重 build ui_preview 後重跑：

```text
FAIL 應該恰好收到 10 個對應的 POST（實際 0 個：[]）
RESULT: FAIL (1)
```

按下與放開間隔 150 ms、推送週期 100 ms 時，10 次 `click` 全部遺失；改回 `pointerdown`
即 10／10。突變已還原。

### 瀏覽器實測：`actions-check.js`

A 段跑真的 ui_preview（`COCKPIT_PREVIEW_PUSH_MS=100`，經由真正的 `/ws`＋channel.js
重畫），點擊一律用 CDP `Input.dispatchMouseEvent`；B 段是 node harness 服務真正的
style.css／render.js／actions.js、以 `window.onState` 餵投影、寫入端點回應可控（409、
切斷連線）。節錄（另有 20 行逐 task／workstream 的按鈕集合比對全部 ok，未列出）：

```text
node docs/research/2026-09-16/actions-check.js
=== A. ui_preview（COCKPIT_PREVIEW_PUSH_MS=100）===
ok   100 ms 推送模式：350 ms 內 version 應該至少前進 2（7 → 10）
ok   ui_preview 的 POST 寫入路由應該回 204（實際 204）
ok   ui_preview 的寫入路徑 GET 應該回 405（實際 405）
ok   按鈕文字應該照 spec（實際 {"advance":"推進","complete":"Completed","fail":"Failed","clear":"清除標記","rebind":"改綁","overrideClear":"取消改綁"}）
ok   不在改綁模式時不應該出現「綁定到這裡」
--- 情境「推進按鈕」（送出路徑）---
ok   應該恰好收到一個 POST /api/projects/cockpit/tasks/be-1/advance
--- 情境「改綁模式跨重畫保留」---
ok   改綁模式期間收到兩份新投影
ok   舊的提示節點已不在文件中（整頁重畫真的發生過）
ok   改綁提示應該指出目標 workstream Backend
ok   改綁提示裡應該有「取消」
ok   只有 connected runtime（win）未 exited 的 pane（wJ:p1、wJ:p3）有「綁定到這裡」
ok   應該恰好收到 PUT /api/projects/cockpit/workstreams/be/override 本體 {"runtime":"win","pane_id":"wJ:p3"}
ok   覆蓋成功後離開改綁模式
ok   按「取消」離開改綁模式
ok   按「取消」不送任何請求（實際 []）
ok   應該恰好收到 DELETE /api/projects/cockpit/workstreams/qa/override
--- 情境「頻繁重畫時按鈕仍有效」（10 個不同 task，按住 150 ms）---
ok   畫面上至少有 10 個「Completed」（實際 10）
ok   連按期間持續重畫（version 37 → 54，至少前進 10）
ok   應該恰好收到 10 個對應的 POST（實際 10 個）
ok   ui_preview PID <pid> 已終止（tasklist 查無此 PID）
ok   port 7770 應該不再有 LISTENING 的行程
=== B. harness（409、請求失敗、鍵盤、畫面不自行修改狀態）===
ok   204 之後、新投影到達前，t1 仍在 Plan（畫面不自行修改狀態）
ok   新投影到達後 t1 出現在下一站 Implement
--- 情境「被拒絕時顯示原因」---
ok   錯誤訊息含「已有標記」（實際 "操作失敗（HTTP 409，POST /api/projects/p/tasks/t1/complete）：已有標記關閉"）
ok   再重畫兩次後錯誤訊息仍在（version v4）
ok   按「關閉」後錯誤訊息消失
ok   關閉後的重畫不會把錯誤訊息帶回來
ok   請求失敗後出現錯誤訊息
ok   下一次操作（成功）清除舊錯誤訊息
ok   按 Enter 送出一次 POST advance
ok   mark 為 completed 時只顯示「清除標記」（實際 ["clear"]）
ok   chrome-harness PID <pid> 已終止（tasklist 查無此 PID）
RESULT: PASS
```

合計 68 行 ok、0 FAIL。錯誤訊息文字尾端的「關閉」是同一列的關閉按鈕文字（`textContent`
含按鈕）。

### 對 task 5.2 腳本的影響

`factory-floor-check.js` 原本以 `/data-project="([^"]*)"/g` 數 Project 區塊；5.3 起操作
按鈕也帶 `data-project`（design D9），改成比對 `class="project" data-project="..."`。改後
重跑 `RESULT: PASS`（105 行 ok）；腳本會重寫 `task-5.2-scenario-d.png`，已還原成 5.2 的
版本，不隨本 task 提交。

### Gate 證據

```text
cargo fmt --check                                      → exit 0
cargo clippy -p cockpit --all-targets -- -D warnings   → Finished, 0 warning
cargo test -p cockpit                                  → 121 passed / 0 failed / 1 ignored
                                                          （app 15、config 46、fixture 1、
                                                          http 6、pipeline_api 16、
                                                          progress_file 14、
                                                          progress_service 11、pwa 3、
                                                          lib 1、real_attach 0(1 ignored)、
                                                          runtimes 4、ws 4）
markdownlint-cli2 "**/*.md"（repo 根）                  → Linting: 58 files → Summary: 0 issues
```

### Fix round 1（Codex review：過期的錯誤訊息）

Finding（Important）：actions.js 只在送出時清 `ui.error`，任何 fetch 結束都能無條件寫入
錯誤。請求 A 慢、B 後送先成功、A 之後才回 409 時，畫面會顯示已過期的 A 錯誤，違反「最近
一次操作的錯誤訊息」。

**修法**：每次操作配遞增序號 `latestOp`，`send` 帶著自己的序號；失敗時只有序號仍等於
`latestOp` 才更新 `ui.error`。`ui_preview` 加 `COCKPIT_PREVIEW_WRITE_RULES`
（`<PATH>=<延遲毫秒>:<狀態碼>`，`;` 分隔），符合的請求照樣記錄，延遲後回指定狀態碼
（非 2xx 附 `{"error": ...}`），其他路徑仍立即 204。`actions-check.js` A 段以
`/api/projects/cockpit/tasks/be-1/fail=1500:409` 啟動，新增情境：先按 A（be-1「Failed」，
慢且 409）再按 B（release-1「推進」，立即 204），等 A 回來後斷言沒有錯誤訊息；另有對照組
只按 A，斷言 409 錯誤照常出現，證明規則確實生效。

RED（修 actions.js 前，ui_preview 規則與腳本已就位）：

```text
ok   伺服器依序收到 A、B（實際 ["POST /api/projects/cockpit/tasks/be-1/fail","POST /api/projects/cockpit/tasks/release-1/advance"]）
FAIL A 在 B 之後才回 409，畫面不應該顯示 A 的過期錯誤（實際："操作失敗（HTTP 409，POST /api/projects/cockpit/tasks/be-1/fail）：ui_preview 模擬回應 409關閉"）
RESULT: FAIL (1)
```

GREEN（修後）：

```text
--- fix round 1：慢的舊操作失敗不蓋掉較新的成功操作 ---
ok   伺服器依序收到 A、B
ok   A 在 B 之後才回 409，畫面不應該顯示 A 的過期錯誤（實際：null）
--- fix round 1 對照組：慢操作本身就是最近一次操作時照常顯示錯誤 ---
ok   最近一次操作 A 的 409 錯誤訊息出現（含本體 error）
ok   關閉對照組的錯誤訊息
RESULT: PASS
```

全部 72 行 ok、0 FAIL；原本四個情境（推進路徑、改綁跨重畫後 `PUT`、100 ms 推送下 10 個
`POST`、409 跨重畫保留）一起重跑，都還是 PASS。

Gate（fix round 1）：

```text
cargo fmt --check -p cockpit                           → exit 0
cargo clippy -p cockpit --all-targets -- -D warnings   → Finished, 0 warning
cargo test -p cockpit                                  → 127 passed / 0 failed / 1 ignored
                                                          （含平行 task 4.2 已提交的測試）
markdownlint-cli2 "**/*.md"（repo 根）                  → Linting: 58 files → Summary: 0 issues
```

## 真機 Scenario C／D（WSL 端，task 7.2）

環境：Windows 端 release 版 `cockpit.exe`（本分支 HEAD 當場 `cargo build --release -p cockpit`）；WSL
`Ubuntu-24.04` 的 headless 測試 server（HERDR 0.8.2、protocol 20），跑前先停再以 `setsid -f` 重啟、
`herdr status server --json` 確認 `running`。**Windows 端 HERDR 完全不碰**：暫存設定只含一筆 WSL
runtime，所有寫入都是對 WSL 測試 server 的 JSON-RPC，並需 `HERDR_CLIENT_TEST_ALLOW_WSL_WRITES=1`。

### 腳本：`pipeline-check.py`

沿用 `docs/research/2026-09-15/acceptance_common.py`（`/api/state` 輪詢、7770 已有服務就拒絕啟動、
依 PID 停 cockpit、`wsl.exe -e nc -U` 送 JSON-RPC、cp950 主控台處理）。流程：

1. WSL 端 `workspace.create`（label `cockpit-7-2`）→ `pane.split` 三次 → `pane.rename` 為
   `backend`／`frontend`／`tests`／`extra`（`extra` 不符合任何 binding，留作覆蓋目標）。
2. 暫存目錄寫設定與狀態檔路徑：一個 project `p`（stages `Plan`／`Implement`／`Test`）、三條 workstream
   各以 `{ runtime = "wsl", workspace = "cockpit-7-2", pane_label = "<名稱>" }` 綁定、四個 task
   （`be1`@Implement、`fe1`@Plan、`qa1`@Test、`be2`@Plan 且 `depends_on = ["be1"]`），以
   `--config` 啟動 release 版 cockpit。
3. Scenario C／D：對三個 pane 送 `pane.report_agent`（`working`），輪詢到對應 task `running` 並記錄秒數；
   逐欄位比對 Scenario C 的 binding JSON、Scenario D 的三個 stage。
4. HTTP 寫入（帶 `Host: 127.0.0.1:7770`）：推進 `fe1`、以同源 `Origin` 標 `be1` Completed（驗
   version 遞增、`be2` 依賴解除）；`qa1` 在最後一站推進 → 409、跨站 `Origin` → 403、不存在的 task →
   404，三者都不遞增 version；讀狀態檔核對內容。
5. `PUT` 把 `tests` 覆蓋到 `extra` pane → 依 PID 停 cockpit → 同設定重啟 → 進度、覆蓋、warnings 保留。
6. `pane.close` 覆蓋指向的 `extra` pane → 投影與狀態檔的覆蓋被刪、`tests` 回到自動解析。
7. 觀察：再 `pane.split` 一次，看新 pane id 是否重用剛關掉的 id。

清理包在 `finally`：`workspace.close` 本腳本的 workspace（另以 snapshot 掃 label 補漏並斷言無殘留）、
依 PID 停 cockpit、刪暫存目錄。輸出只含 id、狀態、秒數。「穩定成立」判定用 `settle()`：條件成立
且連續 2–3 秒不變才算，期間每次倒退都記錄時刻（坑：Drift 或重播可能讓投影短暫倒退）。

### 執行輸出

```text
HERDR_CLIENT_TEST_ALLOW_WSL_WRITES=1 uv run --no-project python docs/research/2026-09-16/pipeline-check.py

ok   WSL 端測試 server 在跑（herdr status server --json）
WSL server version=0.8.2 protocol=20
ok   pane.rename wE:p1 → backend
ok   pane.rename wE:p3 → frontend
ok   pane.rename wE:p4 → tests
ok   pane.rename wE:p5 → extra
workspace wE panes: {'backend': 'wE:p1', 'frontend': 'wE:p3', 'tests': 'wE:p4', 'extra': 'wE:p5'}
OBSERVE startup binding: first_bound=30.44s regressed_at=[] stable_from=30.44s
ok   啟動後 30.44s 起 WSL connected 且三條 workstream 穩定 bound(auto) 到各自 pane（首次 30.44s，倒退 0 次）
ok   report 前 be1／fe1／qa1 皆 ready（pane 無 agent）
ok   be2 依賴未完成的 be1 → pending
ok   尚未有寫入操作 → 狀態檔不存在（不立即建立）
ok   pane.report_agent(wE:p1, working) 回 ok
ok   pane.report_agent(wE:p3, working) 回 ok
ok   pane.report_agent(wE:p4, working) 回 ok
ok   be1 變 running，距 report_agent 0.22s
ok   fe1 變 running，距 report_agent 0.14s
ok   qa1 變 running，距 report_agent 0.20s
running latency (s): {'be1': 0.22, 'fe1': 0.14, 'qa1': 0.2}
ok   Scenario C：backend binding JSON 逐欄位相符（實際 {'state': 'bound', 'runtime': 'wsl', 'pane_id': 'wE:p1', 'source': 'auto', 'agent': 'claude', 'agent_status': 'working'}）
ok   Scenario D：三個 running task 各自在 Implement／Plan／Test（實際 {'be1': 'Implement', 'fe1': 'Plan', 'qa1': 'Test'}）
ok   Runtime working 不改 mark
ok   be2 綁定 pane working 但依賴未完成 → 仍 pending
ok   POST fe1/advance → 204（預期 204）
ok   advance 後 0.15s 內 version>30 且 fe1 在 Implement（實際 version 31）
ok   POST be1/complete（Host＋同源 Origin）→ 204（預期 204）
ok   complete 後 0.15s 內 version>31、be1 mark=completed
ok   be1 status=completed、stage 仍 Implement（done 不是 Completed，標記才是）
ok   依賴 be1 完成後 be2（backend 綁定 working）→ running（實際 running）
ok   POST qa1/advance（已在最後一站）→ 409 {'error': '已是最後一個 Stage'}
ok   跨站 Origin 的 POST qa1/complete → 403（預期 403）
ok   POST 不存在的 task → 404（預期 404）
ok   409／403／404 不遞增 version（前 32，1 s 後 32）
ok   qa1 未被改動
ok   狀態檔 version=1、be1={'stage': 'Implement', 'mark': 'completed'}、fe1 stage=Implement（實際 {'version': 1, 'projects': {'p': {'tasks': {'qa1': {'stage': 'Test', 'mark': 'none'}, 'be2': {'stage': 'Plan', 'mark': 'none'}, 'be1': {'stage': 'Implement', 'mark': 'completed'}, 'fe1': {'stage': 'Implement', 'mark': 'none'}}, 'overrides': {}}}}）
ok   PUT tests/override → wE:p5：204（預期 204）
ok   覆蓋生效：tests bound(override) → wE:p5（0.10s）
ok   覆蓋到沒有 agent 的 pane → qa1 ready（實際 ready）
ok   狀態檔含 tests 的覆蓋
ok   cockpit 已停止（API 不再回應）
--- cockpit 已停止，重新啟動 ---
ok   重啟後 0.53s 內 connected 且 tests 覆蓋仍在（進度來自狀態檔）
ok   重啟後進度保留（實際 {'be1': ('Implement', 'completed'), 'fe1': ('Implement', 'none'), 'qa1': ('Test', 'none'), 'be2': ('Plan', 'none')}）
ok   重啟後 tests 覆蓋仍指向 extra pane
ok   重啟後 project warnings 為空（實際 []）
OBSERVE restart running: first=29.81s regressed_at=[] stable_from=29.81s（自上一步起算）
ok   重啟後 HERDR 仍報 working 的 pane 讓 be2／fe1 穩定回到 running（首次 29.81s、穩定起點 29.81s、倒退 0 次）
ok   pane.close wE:p5 回 ok
ok   關 pane 後 0.12s 內覆蓋失效：投影 tests 不再是 override、狀態檔 overrides 不含 tests
  覆蓋失效當下 tests binding = {'state': 'bound', 'runtime': 'wsl', 'pane_id': 'wE:p4', 'source': 'auto', 'agent': 'claude', 'agent_status': 'working'}
OBSERVE after close auto-resolve: first=0.01s regressed_at=[] stable_from=0.01s（自覆蓋失效起算）
ok   覆蓋失效後 tests 穩定回到自動解析 bound(auto) → wE:p4（首次 0.01s、穩定起點 0.01s）
ok   回到自動解析後 qa1（tests pane working）→ running（實際 running）
drift events after close: 0
OBSERVE pane id reuse: closed=wE:p5 new_split=wE:p6 reused=False
ok   新 split 的 pane 不會復活已刪除的覆蓋
cleanup: workspace.close wE → ok
ok   cleanup：WSL 端沒有殘留 label=cockpit-7-2 的 workspace（殘留：[]）
ok   cockpit 已停止（API 不再回應）
ok   暫存設定／狀態檔目錄已刪除
RESULT: PASS
```

執行後 `netstat -ano` 的 7770 沒有 `LISTENING`、`tasklist` 沒有 `cockpit.exe`；WSL 測試 server 已停
（`herdr status server --json` 為 `not_running`）。

### 秒數摘要

| 項目 | 秒數 |
|---|---|
| `report_agent(working)` → task `running`（be1／fe1／qa1） | 0.22／0.14／0.20 |
| 推進、標 Completed → 投影反映且 version 遞增 | 0.15／0.15 |
| 設覆蓋 → 投影 `bound(override)` | 0.10 |
| 重啟 → connected 且進度與覆蓋在投影中 | 0.53 |
| 關覆蓋指向的 pane → 投影與狀態檔的覆蓋被刪 | 0.12 |
| **啟動 → 三條 workstream 穩定 `bound(auto)`** | **30.44**（見下方發現） |
| **重啟 → 仍報 working 的 pane 讓 task 回到 `running`** | **29.81**（見下方發現） |

### 發現：WSL 0.8.2 新訂閱會重播歷史事件，投影在連上後倒退最多一個定期重拿週期

第一輪執行（腳本當時對重啟後的 `running` 只等 10 秒）得到 `RESULT: FAIL (1)`：「重啟後 10.12s 內 HERDR
仍報 working 的 pane 讓 be2／fe1 回到 running」失敗；啟動後的 binding 也要 30.36 秒才全部 `bound`。
兩個數字都接近 `resnapshot_secs = 30`，於是另做兩個診斷（診斷腳本不進 repo）：

1. **cockpit 投影時間線**（`RUST_LOG=debug`，每 0.25 秒記錄 WSL runtime 內本 workspace 的 pane label／
   agent_status 與三條 binding 狀態）。啟動前直接 `herdr api snapshot` 確認四個 pane 的 label 都已是
   新名稱。cockpit 啟動後（節錄、整理格式）：

   ```text
   [start] t+0.52  bind={backend: bound, frontend: unbound, tests: unbound}  label: wE:p1=backend wE:p3=null wE:p4=null wE:p5=extra
   [start] t+0.79  bind={全部 unbound}                                       label: wE:p2=Explorer，其餘全為 null
   [start] t+1.30  bind={tests: bound，其餘 unbound}                          label: wE:p4=tests wE:p5=extra wE:p1=null wE:p3=null
   [start] t+30.38 bind={全部 bound}                                         label: 四個全部正確
   ```

   重啟時（`report_agent working` 之後、HERDR snapshot 顯示三個 pane 皆 `working`）同樣：t+0.52 時
   `wE:p1` 還是 `working`，t+0.78 起四個 pane 全變 `unknown`、label 變 null，直到 t+30.40 定期重拿
   snapshot 後才回到 `working` 與正確 label。debug 日誌只有「snapshot 前丟棄的事件數 discarded=8」，
   沒有 Drift（重播的事件都指向仍存在的 pane id，不會觸發 Drift 重拿）。

2. **直接擷取 L 訂閱原始事件**（不經 cockpit，`wsl.exe` 內以 `nc -U` 送 `events.subscribe`，14 種生命
   週期事件，閒置 4 秒）：
   - 剛重啟、還沒寫入任何東西：只有 `subscription_started` 與既有 4 個 pane 的 `pane_updated`。
   - 建 workspace、split、rename、`report_agent working` 之後再訂閱：收到 23 筆事件，依序是
     `workspace_created`、`tab_created`、`pane_created wE:p1`（無 label、`unknown`）、`pane_focused`、
     `pane_agent_detected`、`layout_updated`、`pane_created wE:p2`……、`pane_updated wE:p1`（label
     `backend`、status `unknown`）、`pane_updated wE:p2`（label `Explorer`）、`pane_updated wE:p2`（label
     `Sidebar`）……——也就是**從 server 啟動以來的整段事件歷史**，且帶的是當時的舊值（`report_agent`
     之後 `wE:p1` 的 agent 狀態仍是 `unknown`）。
   - 立刻再訂閱一次：同一段歷史又完整重播一次（26 筆，後段多了幾筆 Sidebar 的 `pane_updated`）。

失效鏈：cockpit 連上（或重連）→ 開 L 訂閱 → HERDR 0.8.2 開始重播整段歷史 → 驅動器拿到 authoritative
snapshot（正確）後，只丟掉 snapshot 回應**之前**到達的事件；經 `wsl.exe`＋`nc` 傳輸，歷史事件有一大段在
snapshot **之後**才到，被當成新事件套用 → pane 的 label、agent_status 被舊值蓋掉 → binding 變
`unbound`、綁定 task 從 `running` 掉成 `ready` → 直到下一次定期重拿（30 秒）才恢復。事件都指向仍存在
的 id，不觸發 Drift，所以不會提早重拿。

判別證據：連上後約 1 秒內 pane label 變 null 或 Sidebar 變 `Explorer`；恢復時間貼齊 `resnapshot_secs`。

這與設計文件 §2.3「L 訂閱建立瞬間的補推：WSL 0.8.2 會在 authoritative snapshot 前推送少量剛發生過的
事件（實測 2 筆），已由丟棄規則吸收」的前提不符：實際是**整段歷史**，數量隨 server 存活期間的操作量
增長，且會延續到 snapshot 之後。handover §4「多輪 tab 開關後會累積補推」很可能是同一個機制。影響範圍：
WSL runtime 每次啟動／重連後最多 `resnapshot_secs` 秒內，Scenario C／D 的投影可能是錯的（binding
`unbound`、task `ready`）；Windows 0.9.0 依 task 4.4 結論不補推，推測不受影響（本次未驗）。**本 task
不改產品碼**，交控制端裁決。

腳本最終版把這兩處改成 `settle()` 輪詢到穩定成立（上限 60 秒）並印出首次成立、倒退時刻與穩定起點；
上面第二輪的 `RESULT: PASS` 是在「接受最多一個重拿週期的收斂時間」這個前提下成立的，秒數摘要表中的兩列
粗體就是這個代價。第二輪兩處都顯示 `regressed_at=[]`、首次成立即 30 秒左右，是因為重播發生在啟動後約
1 秒內、`settle()` 看到的第一個「全部成立」已經是重拿之後。

### 觀察：pane id 不重用

關掉 `wE:p5` 之後再 `pane.split`，新 pane 為 `wE:p6`（`reused=False`）；兩輪執行結果相同。design
Risks「pane id 被重用導致覆蓋綁到錯的 pane」在 WSL 0.8.2 這個順序下沒有發生；另外覆蓋在 pane 關閉後
0.12 秒內就被刪除，新 pane 不會復活覆蓋。

### 結論

- Scenario C：綁定 pane `working` 後 0.14–0.22 秒內 task `running`，binding JSON 逐欄位符合 spec 形狀
  （`runtime` 為 `wsl`）。
- Scenario D：三條平行 workstream 同時 `running`，目前 Stage 分別為 `Implement`／`Plan`／`Test`。
- HTTP 推進／標記：204 後 version 遞增、依賴解除；409／403／404 不改狀態也不遞增 version；狀態檔形狀
  與內容符合 spec。
- 重啟後進度、覆蓋保留，warnings 為空。
- 關覆蓋指向的 pane：0.12 秒內投影與狀態檔的覆蓋都被刪，回到 `bound(auto)`。
- **疑慮**：WSL 0.8.2 在新訂閱時重播整段事件歷史，會讓 cockpit 在啟動／重連後約 30 秒內顯示錯的綁定與
  task 狀態（見上方「發現」）。腳本在等收斂的前提下 PASS。

### 6.6 修正後嚴格時限重跑：3 秒版（`RESULT: FAIL (1)`，保留作紀錄）

上方「發現」促成 task 6.6「驅動器連線後沉降重拿」（commit `0bb3e10`＋`7d4becd`：進入 Connected 後事件流
靜默 1 秒再取 snapshot，最晚第 5 秒）。tasks.md 7.2 改為嚴格時限：啟動與重啟後 3 秒內對應 task 為
`running`，不得依賴 30 秒定期重拿，並觀察建立／關閉 pane 後是否倒退。本節是含 6.6 的 release build
（`cargo build --release -p cockpit`）、每次執行前重啟 WSL 測試 server 後的結果。

腳本改動：

- `strict_hold()`：自事件時刻起 3 秒內條件成立，之後 10 秒每 0.1 秒取樣都成立才 PASS；倒退逐筆記錄。
  時限內沒成立就印完整時間線（每次投影變化的 version、task status、binding），並繼續記錄到實際穩定為止，
  只作診斷，判定仍 FAIL。`settle()` 保留但不再用於判定。
- 啟動情境：`tests` pane 在 cockpit 啟動**前**就 report working，驗「啟動後 3 秒內三條 bound(auto)、qa1
  running、be1／fe1 ready」且 10 秒不倒退（直接對著重播會蓋掉的欄位）。
- 重啟情境：3 秒內 be2／fe1 running、tests 覆蓋仍在，10 秒不倒退。
- 關 pane、建 pane 之後：3 秒內 tests 為 bound(auto)、be2／fe1／qa1 running，10 秒不倒退（S 訂閱重開
  是否也重播）。

#### 執行輸出（第二輪，時間線另列）

```text
HERDR_CLIENT_TEST_ALLOW_WSL_WRITES=1 uv run --no-project python docs/research/2026-09-16/pipeline-check.py

ok   WSL 端測試 server 在跑（herdr status server --json）
WSL server version=0.8.2 protocol=20
ok   pane.rename wE:p1 → backend
ok   pane.rename wE:p3 → frontend
ok   pane.rename wE:p4 → tests
ok   pane.rename wE:p5 → extra
workspace wE panes: {'backend': 'wE:p1', 'frontend': 'wE:p3', 'tests': 'wE:p4', 'extra': 'wE:p5'}
ok   啟動前 pane.report_agent(wE:p4, working) 回 ok
OBSERVE 啟動：三條 bound(auto)、qa1 running、be1／fe1 ready: first=2.30s hold=10s regressions=0
ok   啟動：三條 bound(auto)、qa1 running、be1／fe1 ready：2.30s 內成立（≤3s），之後 10s 無倒退（倒退樣本 0）
ok   report 前 be1／fe1 皆 ready（pane 無 agent）
ok   be2 依賴未完成的 be1 → pending
ok   尚未有寫入操作 → 狀態檔不存在（不立即建立）
ok   pane.report_agent(wE:p1, working) 回 ok
ok   pane.report_agent(wE:p3, working) 回 ok
ok   be1 變 running，距 report_agent 0.15s（≤3s）
ok   fe1 變 running，距 report_agent 0.20s（≤3s）
OBSERVE report 後三個 task running 持續: first=0.01s hold=10s regressions=0
ok   report 後三個 task running 持續：0.01s 內成立（≤1s），之後 10s 無倒退（倒退樣本 0）
ok   Scenario C：backend binding JSON 逐欄位相符（實際 {'state': 'bound', 'runtime': 'wsl', 'pane_id': 'wE:p1', 'source': 'auto', 'agent': 'claude', 'agent_status': 'working'}）
ok   Scenario D：三個 running task 各自在 Implement／Plan／Test（實際 {'be1': 'Implement', 'fe1': 'Plan', 'qa1': 'Test'}）
ok   Runtime working 不改 mark
ok   be2 綁定 pane working 但依賴未完成 → 仍 pending
ok   POST fe1/advance → 204（預期 204）
ok   advance 後 0.13s 內 version>24 且 fe1 在 Implement（實際 version 25）
ok   POST be1/complete（Host＋同源 Origin）→ 204（預期 204）
ok   complete 後 0.12s 內 version>25、be1 mark=completed
ok   be1 status=completed、stage 仍 Implement（done 不是 Completed，標記才是）
ok   依賴 be1 完成後 be2（backend 綁定 working）→ running（實際 running）
ok   POST qa1/advance（已在最後一站）→ 409 {'error': '已是最後一個 Stage'}
ok   跨站 Origin 的 POST qa1/complete → 403（預期 403）
ok   POST 不存在的 task → 404（預期 404）
ok   409／403／404 不遞增 version（前 26，1 s 後 26）
ok   qa1 未被改動
ok   狀態檔 version=1、be1={'stage': 'Implement', 'mark': 'completed'}、fe1 stage=Implement（實際略）
ok   PUT tests/override → wE:p5：204（預期 204）
ok   覆蓋生效：tests bound(override) → wE:p5（0.12s）
ok   覆蓋到沒有 agent 的 pane → qa1 ready（實際 ready）
ok   狀態檔含 tests 的覆蓋
ok   cockpit 已停止（API 不再回應）
--- cockpit 已停止，重新啟動 ---
ok   重啟後 0.52s 內 connected 且 tests 覆蓋仍在（進度來自狀態檔）
ok   重啟後進度保留（實際 {'be1': ('Implement', 'completed'), 'fe1': ('Implement', 'none'), 'qa1': ('Test', 'none'), 'be2': ('Plan', 'none')}）
ok   重啟後 tests 覆蓋仍指向 extra pane
ok   重啟後 project warnings 為空（實際 []）
TIMELINE 重啟：be2／fe1 running、tests 覆蓋仍在（未在 3s 內成立；診斷：4.45s 起成立並穩定）:
FAIL 重啟：be2／fe1 running、tests 覆蓋仍在：3s 內成立（實際 4.45s 才成立）
ok   pane.close wE:p5 回 ok
ok   關 pane 後 0.12s 內覆蓋失效：投影 tests 不再是 override、狀態檔 overrides 不含 tests
OBSERVE 關 pane 後：tests 回 bound(auto)，be2／fe1／qa1 running 不倒退: first=0.20s hold=10s regressions=0
ok   關 pane 後：tests 回 bound(auto)，be2／fe1／qa1 running 不倒退：0.20s 內成立（≤3s），之後 10s 無倒退（倒退樣本 0）
ok   回到自動解析後 qa1（tests pane working）→ running（實際 running）
drift events after close: 0
OBSERVE pane id reuse: closed=wE:p5 new_split=wE:p6 reused=False
OBSERVE 建立 pane 後：tests 仍 bound(auto)，be2／fe1／qa1 running 不倒退: first=0.07s hold=10s regressions=0
ok   建立 pane 後：tests 仍 bound(auto)，be2／fe1／qa1 running 不倒退：0.07s 內成立（≤3s），之後 10s 無倒退（倒退樣本 0）
ok   新 split 的 pane 不會復活已刪除的覆蓋
cleanup: workspace.close wE → ok
ok   cleanup：WSL 端沒有殘留 label=cockpit-7-2 的 workspace（殘留：[]）
ok   cockpit 已停止（API 不再回應）
ok   暫存設定／狀態檔目錄已刪除
RESULT: FAIL (1)
```

第一輪（同一 build、同樣重啟 server）另有一條腳本自身的量測錯誤（qa1 的延遲從啟動算到 10 秒觀察窗結束，
印成 12.97 s；已修，改由啟動情境的 `strict_hold` 判定），其餘結果相同：啟動 2.29 s PASS、重啟 FAIL
（時限內未成立，診斷約 4.1 s 才成立）、關 pane 0.22 s 與建 pane 0.07 s 後 10 秒無倒退。**重啟情境兩輪都
FAIL**，可重現。

#### 時間線：重啟（FAIL）

t 為距重啟 cockpit 的秒數；binding 欄位為 `state/pane/source/agent_status`（節錄，省略未變的 be1）。

```text
t+0.53 v=4  be2=running fe1=ready  backend=bound/wE:p1/auto/working   frontend=unbound  tests=bound/wE:p5/override/unknown
t+1.00 v=9  be2=ready   fe1=ready  backend=bound/wE:p1/auto/unknown   frontend=unbound  tests=bound/wE:p5/override/unknown
t+1.12 … t+2.43  v=10→23，約每 0.12 s version+1，狀態維持上一行（重播事件持續套用）
t+3.25 v=24 同上
t+3.36 v=25 同上
t+4.45 v=26 be2=running fe1=running backend=bound/wE:p1/auto/working frontend=bound/wE:p3/auto/working tests=bound/wE:p5/override/unknown
```

讀法：t+0.53 的初始 snapshot 是對的（backend 的 pane 為 working），但 frontend 已經 `unbound`（label 被
最早一批重播事件蓋掉）；t+1.00 起 backend 的 agent_status 也被重播蓋成 `unknown`，be2 掉成 `ready`。
重播事件一路到 t+3.36 仍在到達（version 持續遞增），所以 6.6 的「靜默 1 秒」要到約 t+4.4 才滿足，沉降重拿
在 t+4.45 把投影拉回正確值，之後穩定。

#### 時間線：啟動（PASS，但時限內曾顯示錯的 binding）

```text
t+0.53 v=4  qa1=ready   backend=bound/wE:p1/auto/unknown  frontend=unbound  tests=unbound
t+1.09 v=10 qa1=ready   backend=bound/wE:p1/auto/unknown  frontend=unbound  tests=bound/wE:p4/auto/unknown
t+2.30 v=12 qa1=running backend=bound/wE:p1/auto/unknown  frontend=bound/wE:p3/auto/unknown  tests=bound/wE:p4/auto/working
```

啟動時 WSL server 的歷史較短，重播約 t+1.2 結束，靜默 1 秒後 t+2.30 沉降重拿，所以在 3 秒內成立、之後
10 秒無倒退。但 t+0.53～t+2.30 之間 frontend `unbound`、tests 的 agent_status 為 `unknown`（qa1 `ready`），
也就是 6.6 之後錯誤窗口從約 30 秒縮到約 2 秒，並未消失。

#### 判讀

- **6.6 有效但不足以滿足 3 秒時限**：錯誤窗口的長度＝重播持續時間＋靜默 1 秒（上限 5 秒）。重播長度隨
  WSL server 存活期間累積的事件量增加（本腳本到重啟時已做過 workspace／split／rename／report／HTTP 等
  操作，重播持續約 3.4 秒），所以重啟情境穩定落在約 4.1–4.45 秒。依 6.6 的上限 5 秒，最壞情況也可能到
  5 秒。
- **S 訂閱重開沒有觀察到重播造成的倒退**：關 pane（0.20 s 成立）與建 pane（0.07 s 成立）之後各 10 秒、
  約 100 次取樣都沒有倒退，也沒有 Drift 事件。
- pane id 仍不重用（`wE:p5` 關閉後新 pane 為 `wE:p6`）。
- 本 task 不改產品碼。控制端裁決：3 秒與 spec 的沉降上限 5 秒不一致，時限改為 6 秒（見下一節）。

#### 秒數摘要（6.6 後）

| 項目 | 第一輪 | 第二輪 | 判定 |
|---|---|---|---|
| 啟動 → 三條 bound(auto)＋qa1 running（其後 10 s 不倒退） | 2.29 | 2.30 | PASS |
| report working → be1／fe1 running | 0.20／0.14 | 0.15／0.20 | PASS |
| 重啟 → be2／fe1 running（診斷值） | 約 4.1 | 4.45 | **FAIL** |
| 關 pane → tests bound(auto)、其餘 running（其後 10 s 不倒退） | 0.22 | 0.20 | PASS |
| 建 pane → 同上 | 0.07 | 0.07 | PASS |

收尾：兩輪都依 PID 停掉 cockpit、刪暫存目錄、關掉 WSL 端 workspace；7770 無 `LISTENING`；WSL 測試 server
在最後一輪之後停掉（`not_running`）。

### 門檻調整為 6 秒後重跑（`RESULT: PASS`）

**調整理由**：上一節的 3 秒是寫 tasks.md 時自訂的數字，和已核准的 spec 不一致。`runtime-driver` delta
spec「連線後沉降重拿」寫的是「進入 `Connected` 後事件流靜默滿 1 秒時再取得一次 snapshot……若 5 秒內事件流
始終沒有靜默滿 1 秒，必須在第 5 秒取得」，也就是正確投影的保證上限是 5 秒。控制端裁決時限改為**啟動與重啟
後 6 秒內 `running`（spec 上限 5 秒＋1 秒量測餘裕：秒數從 spawn cockpit 起算，比進入 `Connected` 早約 0.5
秒），且之後 10 秒不倒退**，tasks.md 7.2 已同步修改。上一節 3 秒版的 FAIL 與時間線保留作紀錄。

腳本改動只有門檻：`CONNECT_DEADLINE = 6.0` 用於啟動與重啟兩處 `strict_hold`；report working、關 pane、
建 pane 仍是 3 秒（這三處本來就不經過連線沉降）。release build 與上一節相同（含 6.6），跑前重啟 WSL 測試
server。

#### 執行輸出（節錄；`pane.rename`、狀態檔內容行與成立前時間線省略）

```text
HERDR_CLIENT_TEST_ALLOW_WSL_WRITES=1 uv run --no-project python docs/research/2026-09-16/pipeline-check.py

ok   WSL 端測試 server 在跑（herdr status server --json）
WSL server version=0.8.2 protocol=20
workspace wE panes: {'backend': 'wE:p1', 'frontend': 'wE:p3', 'tests': 'wE:p4', 'extra': 'wE:p5'}
ok   啟動前 pane.report_agent(wE:p4, working) 回 ok
OBSERVE 啟動：三條 bound(auto)、qa1 running、be1／fe1 ready: first=2.30s hold=10s regressions=0
ok   啟動：三條 bound(auto)、qa1 running、be1／fe1 ready：2.30s 內成立（≤6s），之後 10s 無倒退（倒退樣本 0）
ok   report 前 be1／fe1 皆 ready（pane 無 agent）
ok   be2 依賴未完成的 be1 → pending
ok   尚未有寫入操作 → 狀態檔不存在（不立即建立）
ok   pane.report_agent(wE:p1, working) 回 ok
ok   pane.report_agent(wE:p3, working) 回 ok
ok   be1 變 running，距 report_agent 0.14s（≤3s）
ok   fe1 變 running，距 report_agent 0.14s（≤3s）
OBSERVE report 後三個 task running 持續: first=0.01s hold=10s regressions=0
ok   report 後三個 task running 持續：0.01s 內成立（≤1s），之後 10s 無倒退（倒退樣本 0）
ok   Scenario C：backend binding JSON 逐欄位相符（實際 {'state': 'bound', 'runtime': 'wsl', 'pane_id': 'wE:p1', 'source': 'auto', 'agent': 'claude', 'agent_status': 'working'}）
ok   Scenario D：三個 running task 各自在 Implement／Plan／Test（實際 {'be1': 'Implement', 'fe1': 'Plan', 'qa1': 'Test'}）
ok   Runtime working 不改 mark
ok   be2 綁定 pane working 但依賴未完成 → 仍 pending
ok   POST fe1/advance → 204（預期 204）
ok   advance 後 0.15s 內 version>23 且 fe1 在 Implement（實際 version 24）
ok   POST be1/complete（Host＋同源 Origin）→ 204（預期 204）
ok   complete 後 0.13s 內 version>24、be1 mark=completed
ok   be1 status=completed、stage 仍 Implement（done 不是 Completed，標記才是）
ok   依賴 be1 完成後 be2（backend 綁定 working）→ running（實際 running）
ok   POST qa1/advance（已在最後一站）→ 409 {'error': '已是最後一個 Stage'}
ok   跨站 Origin 的 POST qa1/complete → 403（預期 403）
ok   POST 不存在的 task → 404（預期 404）
ok   409／403／404 不遞增 version（前 25，1 s 後 25）
ok   qa1 未被改動
ok   PUT tests/override → wE:p5：204（預期 204）
ok   覆蓋生效：tests bound(override) → wE:p5（0.12s）
ok   覆蓋到沒有 agent 的 pane → qa1 ready（實際 ready）
ok   狀態檔含 tests 的覆蓋
ok   cockpit 已停止（API 不再回應）
--- cockpit 已停止，重新啟動 ---
ok   重啟後 0.51s 內 connected 且 tests 覆蓋仍在（進度來自狀態檔）
ok   重啟後進度保留（實際 {'be1': ('Implement', 'completed'), 'fe1': ('Implement', 'none'), 'qa1': ('Test', 'none'), 'be2': ('Plan', 'none')}）
ok   重啟後 tests 覆蓋仍指向 extra pane
ok   重啟後 project warnings 為空（實際 []）
OBSERVE 重啟：be2／fe1 running、tests 覆蓋仍在: first=4.20s hold=10s regressions=0
ok   重啟：be2／fe1 running、tests 覆蓋仍在：4.20s 內成立（≤6s），之後 10s 無倒退（倒退樣本 0）
ok   pane.close wE:p5 回 ok
ok   關 pane 後 0.12s 內覆蓋失效：投影 tests 不再是 override、狀態檔 overrides 不含 tests
OBSERVE 關 pane 後：tests 回 bound(auto)，be2／fe1／qa1 running 不倒退: first=0.20s hold=10s regressions=0
ok   關 pane 後：tests 回 bound(auto)，be2／fe1／qa1 running 不倒退：0.20s 內成立（≤3s），之後 10s 無倒退（倒退樣本 0）
ok   回到自動解析後 qa1（tests pane working）→ running（實際 running）
drift events after close: 0
OBSERVE pane id reuse: closed=wE:p5 new_split=wE:p6 reused=False
OBSERVE 建立 pane 後：tests 仍 bound(auto)，be2／fe1／qa1 running 不倒退: first=0.08s hold=10s regressions=0
ok   建立 pane 後：tests 仍 bound(auto)，be2／fe1／qa1 running 不倒退：0.08s 內成立（≤3s），之後 10s 無倒退（倒退樣本 0）
ok   新 split 的 pane 不會復活已刪除的覆蓋
cleanup: workspace.close wE → ok
ok   cleanup：WSL 端沒有殘留 label=cockpit-7-2 的 workspace（殘留：[]）
ok   cockpit 已停止（API 不再回應）
ok   暫存設定／狀態檔目錄已刪除
RESULT: PASS
```

#### 時間線：重啟（6 秒版，PASS）

```text
t+0.52 v=4  be2=running fe1=ready  backend=bound/wE:p1/auto/working  frontend=unbound  tests=bound/wE:p5/override/unknown
t+0.65 v=6  be2=ready   fe1=ready  backend=bound/wE:p1/auto/unknown  frontend=unbound  tests=bound/wE:p5/override/unknown
t+0.87 … t+3.16  v=7→25，狀態維持上一行（重播事件持續套用）
t+4.20 v=26 be2=running fe1=running backend=bound/wE:p1/auto/working frontend=bound/wE:p3/auto/working tests=bound/wE:p5/override/unknown
```

與 3 秒版的形狀相同：重播到約 t+3.2 結束，靜默 1 秒後沉降重拿在 t+4.20 把投影拉回正確值，之後 10 秒
（約 100 次取樣）無倒退。

#### 秒數摘要（6 秒版）

| 項目 | 秒數 | 門檻 | 判定 |
|---|---|---|---|
| 啟動 → 三條 bound(auto)＋qa1 running，其後 10 s 不倒退 | 2.30 | 6 | PASS |
| report working → be1／fe1 running | 0.14／0.14 | 3 | PASS |
| 推進／標 Completed → 投影反映、version 遞增 | 0.15／0.13 | 3 | PASS |
| 設覆蓋 → bound(override) | 0.12 | 3 | PASS |
| 重啟 → be2／fe1 running、覆蓋仍在，其後 10 s 不倒退 | 4.20 | 6 | PASS |
| 關 pane → 覆蓋失效、tests bound(auto)、其餘 running，其後 10 s 不倒退 | 0.12／0.20 | 3 | PASS |
| 建 pane → 同上 | 0.08 | 3 | PASS |

#### task 7.2 最終結論

- Scenario C／D 在 WSL 真機通過：綁定 pane `working` 後約 0.14 秒 task `running`，binding JSON 逐欄位符合，
  三條平行 workstream 各自停在 `Implement`／`Plan`／`Test`；HTTP 推進／標記、來源檢查、重啟後進度與覆蓋
  保留、關 pane 使覆蓋失效都符合 spec。
- **WSL HERDR 0.8.2：cockpit 啟動或重連後，最多約 5 秒（spec 沉降上限）畫面不準確**：新訂閱會重播整段
  事件歷史，重播期間 binding 可能顯示 `unbound`、綁定 task 顯示 `ready`；6.6 的沉降重拿在事件靜默 1 秒後
  （最晚第 5 秒）拉回正確值，之後穩定。本次實測啟動 2.30 秒、重啟 4.20 秒；重播長度隨 server 存活期間的
  事件量增加。
- **Windows HERDR 0.9.0：未見重播**——控制端於 2026-09-17 以唯讀
  `cargo run -p herdr-client --example capture_events -- --seconds 5` 訂閱長時間使用中的 server，只收到 2 筆
  帶當前值的 `pane_updated`，沒有歷史事件（task 4.4 的 20 秒擷取結果相同）；本 task 為了不碰 Windows 端，
  沒有在 Windows 端重跑這個腳本。
- S 訂閱因 pane 集合變化而重開時（關 pane、建 pane）沒有觀察到重播造成的倒退；pane id 不重用。

收尾：cockpit 依 PID 停止，7770 沒有 `LISTENING`；暫存目錄已刪；WSL 端 workspace 無殘留；WSL 測試
server 已停（`not_running`）。

## 全 workspace gate（task 7.1）

在 commit `14959a3`（最終審查修正波之後）於 repo 根執行，輸出摘要：

```text
## cargo fmt --check
exit=0
## cargo clippy --all-targets -- -D warnings
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.22s
exit=0
## cargo test --workspace
exit=0
passed=494 failed=0 ignored=10
## markdownlint-cli2
Linting: 58 files
Summary: 0 issues in 0 files
## openspec validate --all
Totals: 12 passed, 0 failed (12 items)
```

`passed`／`failed`／`ignored` 為各測試執行檔 `test result:` 行加總；10 個 ignored 是需要真機 HERDR 的測試。
`openspec validate --all` 另有 SHALL/MUST 與 requirement 過長的 WARNING／INFO，不算失敗。

## 使用者手動驗收（task 7.3，2026-09-19）

設定：在本機 `cockpit.toml`（gitignored）加一個 project `cockpit`，stages `Spec／Build／Review／Done`，
兩條 workstream 以 Windows runtime 的 workspace 標籤＋`agent = "claude"` 綁到兩個真實 agent pane
（`ai-cockpit` 的 `wN:p1`、`repo-c` 的 `wQ:p1`），三個 task：`t1`（main／Build）、
`t2`（main／Spec，`depends_on = ["t1"]`）、`t3`（shioaji／Review）。以 debug build 執行。

使用者親自確認（回報「沒問題」）：

- Factory Floor 網格顯示正確：兩列 workstream、四欄 stage，三個節點各在所屬列與目前 stage 的格子；
  綁定摘要為已綁定；`t1` 隨綁定 pane 的 agent 狀態呈現 `running`，`t2` 因依賴未完成為 `pending`，
  `t3` 綁定的 agent 為 `done`／`idle` 時為 `ready`（HERDR 的 `done` 沒有被推成 `completed`）。
- 「推進」按鈕：把 `t2` 一路推進到 `Done`，畫面依 `/ws` 推送更新。
- 「改綁」與「取消改綁」：目視無問題。

控制端代為執行的部分（使用者委託）：

- **重啟保留**：先以寫入端點設 `shioaji` 覆蓋到 `wN:p1`（`PUT` → 204）並把 `t1` 標 `completed`
  （`POST …/complete` → 204），確認 `t1` 標記使 `t2` 的 `pending` 解除、`t3` 依覆蓋跟隨 `wN:p1` 狀態；
  停止程序（`taskkill /PID`）後以同一份設定與狀態檔重新啟動，重啟後逐欄比對：`t1` Build／`completed`、
  `t2` Done、`t3` Review、`shioaji` 覆蓋仍為 `wN:p1` 且 `source` 為 `override`、`warnings` 為空，
  狀態檔內容與重啟前相同。投影 `version` 由 790 歸零重算為 15，屬新程序的預期行為（version 只在單次執行內遞增）。
- 驗收後以 `DELETE …/override` 與 `POST …/clear` 還原，狀態檔回到三個 task 皆 `mark: none`、`overrides: {}`。

未在本次手動驗收涵蓋、已由自動化涵蓋的項目：

- 「標 Failed」與「清除標記」的目視：清除標記已由控制端以端點驗過（204、狀態檔回到 `none`）；
  兩者的畫面行為由 `actions-check.js`（task 5.3）與 `pipeline_api` 測試涵蓋。
- 覆蓋失效（關掉被覆蓋指向的 pane）：未在 Windows 端做，因當時覆蓋指向的是正在使用的 agent pane；
  已由 task 7.2 的 WSL 真機腳本自動驗過（關 pane 後 0.20 秒失效，投影與狀態檔兩邊都刪除）。
