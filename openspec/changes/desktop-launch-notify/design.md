# design：desktop-launch-notify

## Context

動機與範圍見 `proposal.md`；行為以兩份新 capability 的 spec 為準。現況（2026-10-02 盤點）：

- `cockpit` 是主控台子系統執行檔；命令列只接受 `--config`（`cockpit/src/config.rs`）；`app::run` 以 `tokio::signal::ctrl_c()` 為關閉訊號傳給
  `run_with_shutdown`，其內 `axum::serve(...).with_graceful_shutdown(...)` 後跑 `shutdown_all`（`cockpit/src/app.rs`）。綁定失敗時回帶 `無法綁定 <listen>` 的錯誤。
- `/ws`（`cockpit/src/http.rs` 的 `ws_handler`／`handle_socket`）每條連線各自 clone `watch::Receiver`，沒有連線計數；`/ws` 不做來源檢查。
- 前端：`render.js` 的 `window.onState` 存 `latestState` 後 `paint()`，`paint()` 以 `replaceChildren` 重建整個 `#app`（頂列在內）；只有 `files.js` 用
  `localStorage`；沒有任何 `Notification`、`visibilitychange`、`document.hasFocus` 用法。`window.liveOutput.select(runtime, paneId)` 可選定 pane。
- 載入順序（`index.html`）：output.js、viewers.js、git.js、files.js、render.js、actions.js、channel.js。
- `ui_preview` 的推送迴圈只會循環改第一個 pane 的 `agent_status`（working→idle→blocked），不能排程指定 pane 或 task 的轉換。
- 子程序一律帶 `CREATE_NO_WINDOW`（0x08000000）的既有寫法在 `cockpit-git/src/runner.rs`、`cockpit-herdr/src/probe.rs`、`herdr-client/src/connector/child_stdio.rs`。
- 查證（2026-10-02）：Chromium 的 `--app=<URL>` 以「application 模式」開啟（Chromium Command Line Switches 清單）；Edge 為 Chromium 核心、同一參數。
  Rust 的 `#![windows_subsystem = "windows"]` 使程式不附著、不建立主控台（Rust Reference「windows_subsystem」，可用值 `"console"`／`"windows"`）。

## Goals / Non-Goals

**Goals:** 一個捷徑就開好、關窗即結束、不閃黑窗；通知判斷是可單獨測試的純函式；不改變手動執行 `cockpit` 的既有行為；不新增第三方依賴。

**Non-Goals:** 見 proposal「非目標」。

## Decisions

### D1 啟動器放在 `cockpit` 套件的第二個 bin

`cockpit/src/bin/cockpit-launch.rs`，檔首 `#![windows_subsystem = "windows"]`（只影響這個 bin）。可測的邏輯放 `cockpit/src/launch.rs`（lib 模組、`pub`）：
解析引數、由設定得出網址、判斷「是否為 Cockpit 的回應」、瀏覽器候選清單（以「檔案是否存在」的函式注入）、組後端引數與 log 路徑。bin 只做 I/O 串接。
`cockpit/Cargo.toml` 加 `default-run = "cockpit"`，否則第二個 bin 會讓既有文件中的 `cargo run -p cockpit` 失效。
設定解析沿用 `config.rs` 既有函式，不重寫規則。選這個而非獨立 crate：兩者共用設定解析，獨立 crate 要把 `cockpit` 的 config 抽出來，改動更大。

### D2 偵測與就緒判斷用 `GET /api/state`

不新增健康檢查端點：`/api/state` 已存在、唯讀、回整張圖，檢查「JSON 物件含 `version` 與 `runtimes`」即可分辨 Cockpit 與其他服務。以標準函式庫
`TcpStream::connect_timeout`（1 秒）連線，送 `HTTP/1.0` 請求並帶 `Connection: close`（`Host` 用監聽位址，符合來源檢查），讀到 EOF 或達 `Content-Length`；
讀取逾時 2 秒、本體最多 8 MiB。`/api/state` 的本體是一次寫出的字串、帶 `Content-Length`，不會是 chunked。不為此加 HTTP client 依賴。

### D3 背景啟動後端

`std::process::Command`：程式為 `current_exe()` 同目錄的 `cockpit.exe`；引數為解析後的設定檔**絕對路徑**（`--config`，有設定檔時）＋`--exit-when-idle`；
不另設工作目錄（沿用啟動器的）；Windows 上 `creation_flags(CREATE_NO_WINDOW)`（不與 `DETACHED_PROCESS` 併用，孫程序 `wsl.exe`／git 因而繼承隱藏主控台、
不閃窗）；stdin 為 `Stdio::null()`，stdout／stderr 都導向 `cockpit.log`（`File::create` 覆寫）。啟動後輪詢 `try_wait` 偵測提早結束，提早結束時先重新偵測一次
（同時點兩次捷徑時另一個後端可能已就緒）。log 位置：有設定檔時為其所在目錄，否則為工作目錄。`cockpit.log` 加入 `.gitignore`。

`cockpit.log` 不得含 ANSI 控制碼：tracing-subscriber 預設輸出 ANSI 色碼、不偵測是否為終端機。`main.rs` 改為
`.with_ansi(std::io::stderr().is_terminal())`，導向檔案時自動不帶色碼（手動在主控台執行時照舊有色）。

### D4 瀏覽器

候選順序：`COCKPIT_BROWSER`（存在才用）→ `%ProgramFiles%`、`%ProgramFiles(x86)%`、`%LOCALAPPDATA%` 下的 `Google\Chrome\Application\chrome.exe` →
`%ProgramFiles(x86)%`、`%ProgramFiles%` 下的 `Microsoft\Edge\Application\msedge.exe`。不查登錄檔（KISS；標準安裝位置已涵蓋）。以
`--app=http://<監聽位址>/` 啟動、不等待其結束；不指定 `--user-data-dir`，沿用使用者的瀏覽器設定檔，通知權限因此能跨次保留。

### D5 錯誤訊息框

以 `#[cfg(windows)] #[link(name = "user32")] unsafe extern "system" { fn MessageBoxW(...) }` 直接宣告（edition 2024 的 extern 區塊必須標 `unsafe`），
呼叫處包 `unsafe {}`；UTF-16 字串、`MB_OK | MB_ICONERROR`。不加 `windows-sys` 依賴：只用一個函式。訊息含原因與（若有）`cockpit.log` 路徑和最後 20 行。
**測試入口**：環境變數 `COCKPIT_LAUNCH_DIALOG_FILE` 有值時，不顯示訊息框，改把訊息以 UTF-8 附加寫入該檔（訊息框會阻塞，自動驗收需要這個入口）。

### D6 閒置結束

`config.rs` 接受 `--exit-when-idle`（布林；重複給為錯誤，`--config` 重複時維持既有「最後一個為準」）。`http.rs` 的 `/ws` 處理在升級成功時遞增、結束時遞減一個連線計數，
透過一個 `watch::Sender<usize>`（放進 `AppState`）發布目前連線數；遞減以 drop guard 確保任何結束路徑都會執行。`GET /` 與 `GET /api/state` 的處理另外
更新一個「最近活動時間」（同樣放進 `AppState`），供閒置計時延長期限（spec「閒置自動結束」）。監看端以「每次收到 `changed()` 通知就重新評估」的方式運作，
不以「等到值大於 0」判斷，避免漏掉 0→1→0 的短暫變化。`AppState` 新增欄位會動到既有的 struct literal（含 `ui_preview.rs` 與數個測試檔），以預設值補上。`app.rs` 新增閒置監看 future：參數 `IdlePolicy { startup_grace, idle_grace }`（正式值 60 秒、10 秒；
測試注入短值），規則依 spec；觸發時完成關閉訊號，與 Ctrl-C 以 `select` 合併後交給既有的 `run_with_shutdown`。不帶旗標時不建立監看，行為不變。
選 `watch<usize>` 而非 `AtomicUsize`＋輪詢：監看端能精確地在「降為 0」的那一刻開始計時，也能在計時中被新連線打斷。

### D7 前端通知模組 `notify.js`

- 新檔，載入順序放在 `render.js` 之前（`render.js` 會呼叫它）。對外 `window.cockpitNotify = { observe(state), togglePanel(), diff, toNotifications }`（鈴鐺本身由 `renderTopbar` 產生，見 D8）；內部分成
  **純函式**（`diff(prev, next) → events`、`toNotifications(events, settings) → [{title, body, tag, target}]`，無 DOM、可在驗收腳本中直接呼叫驗證）與
  **副作用**（權限、`new Notification`、onclick）。
- `render.js` 的 `window.onState` 在 `paint()` 之後呼叫 `window.cockpitNotify.observe(state)`（`window.cockpitNotify` 不存在時略過，讓只載入 render.js 的
  既有驗收不受影響）；通道重連不重設基準（模組保留上一份）。
- 建立通知時帶 `renotify: true`，同標籤取代時仍重新提醒；實際行為在 notify-check 中以記錄器驗證參數，真機確認一次。
- 前景判斷：`document.visibilityState === "visible" && document.hasFocus()` 時不發。
- 設定存 `localStorage` 鍵 `cockpit.notify.v1`，值 `{"blocked":true,"done":false,"failed":true,"completed":false}`；讀取時逐鍵驗證型別，缺漏或損毀用預設。
- 識別標籤：`cockpit:<類別>:<runtime>/<pane>` 或 `cockpit:<類別>:<project>/<task>`；合併通知標籤 `cockpit:summary`。
- onclick：`window.focus()`；pane 事件呼叫新增的 `window.cockpitActions.selectPane(runtime, paneId)`——在 `actions.js` 內走與點 pane 列（`select-pane`）
  相同的路徑（設定 `ui.selected`、`liveOutput.select`、`repaint`），改綁模式中或 pane 不在最新狀態時不做事；之後 `notification.close()`。只呼叫
  `liveOutput.select` 不夠：右欄選定標示由 `actions.js` 的 `ui.selected` 決定。

### D8 鈴鐺與設定面板

鈴鐺按鈕由 `renderTopbar` 產生（在 `#app` 內、每次重畫重建，狀態不存在 DOM 上），帶 `data-action="notify-settings"`：焦點還原（`render.js`）只認
`data-action` 元素，這樣鍵盤停在鈴鐺上時重畫後焦點能回到新的鈴鐺。`actions.js` 的 `perform()` 在最前面比照 `select-project` 提早處理 `notify-settings`
（呼叫 `window.cockpitNotify.togglePanel()`），不遞增 `latestOp`、不清除 `ui.error`。「點面板外關閉」的判定排除鈴鐺本身，否則開啟的那次點擊會立刻關掉面板。設定面板是 `body` 底下、`#app` 之外的獨立節點，由
`notify.js` 建立與管理，因此整頁重畫不影響它（見 memory「整頁重畫會丟掉只存在 DOM 上的狀態」）。外觀沿用既有色票與按鈕樣式（`style.css`），由
frontend-design 審核模式評一次。

修正波（3.6 審查後）補的互動細節：面板位置一律限制在視窗內，開啟時鈴鐺不在畫面內就先捲進來，開著時遇到捲動、縮放或重畫重新定位（不採「捲動就關閉」，
避免讀到一半被關掉）；焦點以程式管理而不搬 DOM——鈴鐺按 Tab 進入面板、第一個開關 Shift+Tab 回鈴鐺、最後一個元素按 Tab 關閉面板並回鈴鐺；鈴鐺帶
`aria-expanded` 並在開啟時呈現按下狀態；Esc 已被其他元素 `preventDefault` 時不處理。

### D9 `ui_preview` 排程轉換

新增環境變數 `COCKPIT_PREVIEW_TRANSITIONS`：`;` 分隔的 `<毫秒>:pane:<runtime>/<pane>=<agent_status>` 或 `<毫秒>:task:<project>/<task>=<status>`；
推送迴圈到時直接改投影中該 pane 的 `agent_status`（並同步所有 `binding.state=bound` 且指向該 pane 的 workstream 的 `binding.agent_status`）或該 task 的
`status`（`completed`／`failed` 時一併設 `mark`，其他值時 `mark` 設為 `none`），並遞增 `version`。**設了此變數時停止既有的循環 pane**（否則 `wJ:p1` 每 3 次推送
變成 `blocked`，會干擾通知驗收）。fixture 中 `wJ:p2` 是 `exited`，驗收用其他 pane。fixture 是直接建好的投影、
沒有推導邏輯，所以只改投影欄位，不模擬推導。既有循環 pane 維持原行為，驗收腳本選其他 pane 以免干擾。

### D10 驗收

- Rust：`launch.rs` 純函式單元測試；`--exit-when-idle` 解析；閒置監看以 `tokio::time::pause` 的單元測試涵蓋 spec 五個 scenario；`/ws` 計數的整合測試。
- 新腳本 `docs/research/2026-10-02/notify-check.js`：以 CDP `Page.addScriptToEvaluateOnNewDocument` 替換 `window.Notification` 為記錄器（記下 title／body／tag、
  可模擬 permission 三種值）並可覆寫 `document.hasFocus`；以 `ui_preview` 的 `COCKPIT_PREVIEW_TRANSITIONS` 產生轉換，逐條驗 spec 的 scenario；另直接呼叫
  `cockpitNotify` 的純函式驗合併與內容；設定面板的鍵盤操作、Esc 關閉、重畫不關閉。
- 新腳本 `docs/research/2026-10-02/idle-exit-check.js`：以臨時設定（runtime 指向不存在的 socket，不碰 HERDR）啟動 `cockpit --exit-when-idle`，用 Node 內建
  `WebSocket` 連上再關閉，量測約 10 秒後開始關閉（總時間上限要預留 `shutdown_all` 最多約 11 秒）、結束碼 0；另驗重連打斷計時與 `GET /api/state` 延長期限。
  60 秒的「從未連線」由 Rust 單元測試涵蓋。
- 啟動器：`launch.rs` 單元測試＋真機一次（用安裝腳本安裝到真實位置、從捷徑開啟、確認視窗出現、關閉後後端結束），紀錄只寫中繼資料。

## Risks / Trade-offs

- [`--app` 視窗的通知會被歸在 Chrome／Edge 名下] → 可接受；通知內容寫明 Cockpit。
- [使用者用一般分頁也開著 Cockpit 時，關掉 `--app` 視窗後端不會結束] → 符合「沒人看才結束」的語意。
- [瀏覽器裝在非標準位置] → `COCKPIT_BROWSER` 覆寫；找不到時訊息框說明。
- [重新整理時間超過 10 秒（例如載入很慢）會被結束] → 頁面載入只需數百毫秒；10 秒餘裕足夠。
- [通知權限被使用者封鎖] → 面板說明如何解除；不報錯。
- [啟動器模式下視窗關閉約 10 秒後後端結束，之後 agent 的進度回報與寫入請求連線失敗、靜默遺失（`curl -s` 看不到錯誤）] → 整支分支審查 I1 指出；
  Claude 依授權裁決維持「關視窗即結束」並寫明於 spec 與 README，交使用者重新評估（替代方案：後端常駐＋系統匣、或 agent 端回報失敗時重試）。手動執行
  `cockpit`（不帶旗標）不受影響。
- [多個 Cockpit 視窗同時開著] → 通知設定以 `storage` 事件跨視窗同步；同一事件會在每個視窗各發一次通知（同 tag 會被系統合併成一則）。
- [task 的 failed／completed 目前只由人工標記產生，這兩類通知價值有限] → 使用者明確要求四類都做；spec 寫明現況；預設只開 failed。
- [同時點兩次捷徑時兩個後端競爭綁定] → 輸的那個綁定失敗結束，其啟動器重新偵測後照常開窗（spec「同時點兩次捷徑」）。
- [舊後端仍在關閉時重新啟動，`File::create` 截斷仍被寫入的 `cockpit.log`] → Rust std 在 Windows 預設共用讀寫刪除，不會開檔失敗，最壞是 log 中出現一段 NUL；可接受。
- [Chrome 記憶體節省或 Edge 睡眠分頁可能凍結最小化的 `--app` 視窗，使 `/ws` 斷線、後端閒置結束、通知中斷] → 待真機觀察；若發生再評估（例如延長閒置期限）。
- [`/ws` 不檢查來源，任何網頁都能連上使後端不結束] → 既有暴露面（推送內容只有投影），本 change 不擴大；記錄供日後評估。

## Open Questions

（無）
