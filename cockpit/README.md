# cockpit

AI Agent Cockpit 的服務本體：載入設定、接上每個 HERDR runtime、把所有 runtime 的狀態合併投影成
一張圖，再用內嵌的單頁 dashboard（HTTP ＋ WebSocket）呈現。對 HERDR 全程唯讀，從不送 prompt、
不改任何 pane。

- 畫面與 JSON 的形狀：`docs/superpowers/specs/2026-09-13-cockpit-mvp-design.md` §6.4、§8
- 連線行為與型別：`herdr-client/README.md`
- 程序生命週期的決定：`openspec/changes/attach-herdr-runtimes/design.md` D16

## 啟動

```bash
# 零設定模式（工作目錄沒有 cockpit.toml 時）
cargo run -p cockpit

# 指定設定檔
cargo run -p cockpit -- --config cockpit.toml

# 調整日誌等級（預設 info，只寫 stderr）
RUST_LOG=debug cargo run -p cockpit
```

啟動成功會印出一行 `dashboard 已啟動：http://127.0.0.1:7770/（Ctrl-C 結束）`，用瀏覽器開那個網址
即可。Ctrl-C 會優雅結束：先停掉 HTTP 伺服器，再讓每個驅動器釋放事件流與子程序。

`--config` 指的檔案不存在、設定驗證不過、或 `server.listen` 綁不上（port 被占用）時，程序會印出
路徑或位址加原因後**非零結束**，不會自動換 port：

```text
$ cargo run -p cockpit -- --config missing.toml
Error: 設定檔不存在：D:\projects\ai-cockpit\missing.toml
```

## 設定檔

來源依序是：`--config <path>` → 工作目錄的 `cockpit.toml` → 零設定模式。

- **零設定模式**：沒有 `--config`、工作目錄也沒有 `cockpit.toml` 時，等同一筆 `id = "local"`、
  `kind = "herdr"`、自動找本機 HERDR socket 的 runtime，其餘全部預設值
  （`127.0.0.1:7770`、`resnapshot_secs = 30`、`wsl_probe_secs = 60`）。
- **範例**：repo 根的 `cockpit.example.toml`，含 `[server]`、`[polling]` 與 Windows／WSL 兩筆
  runtime 的寫法。裡面的路徑用 `<user>` 佔位，複製後要換成自己的使用者名稱。
- `cockpit.toml` 已列入 `.gitignore`（`/cockpit.toml`），不會進 repo——它會帶著真實的使用者名稱
  與 socket 路徑。

```bash
cp cockpit.example.toml cockpit.toml   # 然後把 <user> 換掉
cargo run -p cockpit
```

## Pipeline 設定（change 2 `pipeline-projection`）

`[[project]]`（零到多筆）與 `[state]` 都是選填區段，省略時行為與 1b 完全相同（不讀不寫狀態檔）。

```toml
[[project]]
id = "cockpit"
name = "AI Cockpit"
stages = ["Spec", "Build", "Review", "Done"]   # 陣列順序即 Stage 順序，字串同時是識別與顯示名稱

[[project.workstream]]
id = "planning"                                 # 沒有 binding：只用來分組畫面上的欄

[[project.workstream]]
id = "backend"
binding = { runtime = "wsl", workspace = "ai-cockpit", pane_label = "backend", agent = "claude" }

[[project.task]]
id = "spec"
title = "寫 spec"
workstream = "planning"
stage = "Spec"

[[project.task]]
id = "impl"
title = "實作"
workstream = "backend"
stage = "Build"
depends_on = ["spec"]

[state]
path = "cockpit.state.json"   # 選填；相對路徑相對於設定檔目錄解析，省略時預設同一個檔名
```

- `id` 須符合 `^[A-Za-z0-9_-]{1,64}$`，同層不重複；`name`／`title` 省略時預設等於 `id`；
  `binding.runtime` 必須是設定檔中某一筆 `[[runtime]]` 的 `id`。以上區段與欄位之外的未知欄位視為
  錯誤（啟動失敗）。完整範例見 repo 根的 `cockpit.example.toml`（已含一份可直接跑的示範 project）。
- `[state] path` 給了才用，未給時預設為設定檔目錄下的 `cockpit.state.json`；零設定模式（沒有
  `--config` 也沒有 `cockpit.toml`）下 `projects` 恆為空，不會有任何 project，也就不讀寫狀態檔。
- 狀態檔（`cockpit.state.json` 或 `[state] path` 指到的檔案）已列入 `.gitignore`，不會進 repo——
  它帶著會頻繁變動的進度與覆蓋，不是設定。停用某條 pipeline 只要刪掉對應的 `[[project]]` 區段，
  下次啟動時舊狀態檔中對不到設定檔的 project／task／workstream 會被忽略並記一則 warn。
- **單一實例**：狀態檔沒有鎖，也沒有多實例協調機制。同一份狀態檔只能給一個 cockpit 行程用；兩個
  行程指到同一個檔案時，後寫入的那個會覆蓋先寫入的，不會合併。
- **回滾注意**：`[[project]]`／`[state]` 是 change 2 新增的區段，設定檔解析一律 `deny_unknown_fields`
  （未知欄位＝啟動失敗）。換回沒有這兩個功能的舊版 `cockpit.exe` 前，要先把 `cockpit.toml` 裡的
  `[[project]]` 與 `[state]` 區段整段移除，不然舊版會直接啟動失敗；狀態檔可以留著不動，舊版本來就
  不會去讀它。

## 寫入 API（change 2 `pipeline-projection`）

進度與畫面覆蓋改由 HTTP 寫入端點操作，畫面上按對應按鈕即會送出；下列是等效的 `curl` 範例。
**寫入端點只接受本機同源請求**：`Host` 標頭必須是 `127.0.0.1:<port>`、`localhost:<port>`、
`[::1]:<port>` 三者之一（`<port>` 是服務實際監聽的埠，即 `listen` 或它綁定後真正拿到的埠），有
`Origin` 標頭時其值必須逐字等於 `http://` 加上同一個 `Host`；不符合一律 403、不改任何狀態。
下面範例對本機打 `127.0.0.1:7770`，`curl` 依網址自動送出對應的 `Host` 標頭，仍明寫出來方便對照：

```bash
# 推進（POST，四種操作 advance / complete / fail / clear 三選一，不需要本體）
curl -i -X POST http://127.0.0.1:7770/api/projects/cockpit/tasks/impl/advance \
  -H 'Host: 127.0.0.1:7770'

# 設定畫面覆蓋（PUT，本體含 runtime、pane_id 兩個字串欄位）
curl -i -X PUT http://127.0.0.1:7770/api/projects/cockpit/workstreams/backend/override \
  -H 'Host: 127.0.0.1:7770' \
  -H 'Content-Type: application/json' \
  -d '{"runtime":"wsl","pane_id":"w1:p3"}'

# 取消畫面覆蓋（DELETE；覆蓋本來就不存在也回 204）
curl -i -X DELETE http://127.0.0.1:7770/api/projects/cockpit/workstreams/backend/override \
  -H 'Host: 127.0.0.1:7770'
```

狀態碼：成功 204（不回投影本體，畫面等 `/ws` 推送）；project／task／workstream 不存在，或
`<op>` 不是四值之一 → 404；操作被拒絕（已是最後一站、已有標記、覆蓋的 runtime 未連線或 pane 已
exited）→ 409，本體 `{"error": "<原因>"}`；`PUT` 本體不是含 `runtime`／`pane_id` 兩個字串欄位的
JSON 物件 → 400；狀態檔寫入失敗 → 500，本體同樣是 `{"error": "<原因>"}`，記憶體中的進度維持操作
前的值（不會半套生效）。

**注意（畫面按鈕會用同一套規則回 403／500，不是只有 `curl` 才會踩到）：**

- **`Host` 比對是逐字比對，不是「任何 loopback 位址」都算**：只接受 `127.0.0.1:<port>`、
  `localhost:<port>`、`[::1]:<port>` 這三種寫法加上服務實際監聽的埠（見
  `cockpit/src/source_check.rs`）。如果把 `[server] listen` 設成其他 loopback 位址（例如
  `127.0.0.2`），瀏覽器送出的 `Host: 127.0.0.2:<port>` 不在這三種寫法裡，畫面上的按鈕一律回
  403。同理，**不要把 `listen` 設成 80 埠**：瀏覽器與 curl 都會把預設埠從 `Host` 省略（即使網址
  明寫 `:80`，送出的仍是 `Host: 127.0.0.1`），對不上 `127.0.0.1:80`，寫入一律回 403，沒有網址
  寫法能繞過。
- **`[state] path` 的父目錄必須事先存在**：cockpit 只會建立狀態檔本身（`.tmp` 再 `rename`），
  不會幫你建立目錄（寫檔見 `cockpit/src/progress_service.rs` 的 `write_atomically`）。如果 `path` 指到一個
  父目錄不存在的位置，每一次寫入（包含畫面按鈕）都會在寫 `.tmp` 這一步失敗，回 500，且永遠不會
  自己修好——請先手動建立好該目錄。

**推進沒有反悔按鈕**：畫面上「Completed」「Failed」都能再按「清除標記」復原，但「推進」把 task
移到下一個 stage 後沒有對應的「退回」操作；`pointerdown` 事件委派又比 `click` 容易誤觸（見設計文件
§8.3），誤按只能直接手動改狀態檔（`cockpit.state.json` 裡對應 task 的 `stage` 欄位）。

**手動改狀態檔前必須先停止 cockpit，改完才能再啟動**：cockpit 執行中對狀態檔的任何寫入——不只是
按按鈕，也包含背景任務刪除失效覆蓋（design D3；覆蓋指到的 pane 不存在或已 exited 時自動觸發）——
都會覆寫整份狀態檔（`.tmp` 寫好再 `rename` 取代），所以只要程序還在跑，任何時間點的下一次寫入都會
連同你手改的內容一起蓋掉。正確順序是：先 Ctrl-C 停掉這次執行 → 改 `cockpit.state.json` → 再重新
`cargo run -p cockpit` 或執行檔啟動，讓它在下次寫入前先把你手改的內容讀進記憶體。

## 輸出讀取 API（change 3 `live-output`）

`GET /api/runtimes/<runtime>/panes/<pane>/output` 對指定 pane 即時讀一次目前的畫面輸出（最多最近
200 行），不快取、不在請求之間保留任何與選取有關的狀態。畫面上點 runtime 卡裡任一未 exited 的
pane 列，或 Factory Floor 中已綁定 workstream 列首的「看輸出」，即會開啟 Live Output 面板並開始
每秒輪詢一次；面板的「關閉」可取消選取。改綁模式期間 pane 列不可點選（不呈現可點選樣式），既有的
選取與面板維持不變，離開改綁模式後才恢復可點選。

成功回 200，本體為 JSON：`runtime`、`pane_id`（與路徑相同）、`format`（目前固定為 `"text"`）、
`text`（純文字，不含終端機控制序列）、`truncated`（`true` 表示還有更早的輸出未回傳）。下面範例對
本機打 `127.0.0.1:7770`，`curl` 依網址自動送出對應的 `Host` 標頭，仍明寫出來方便對照：

```bash
# 讀輸出（GET，只讀不寫，每次都是即時讀一次）
curl -i http://127.0.0.1:7770/api/runtimes/win/panes/w1:p1/output \
  -H 'Host: 127.0.0.1:7770'
```

狀態碼：路徑中的 `<runtime>` 或 `<pane>` 片段解碼後不是合法 UTF-8 → 400；`<runtime>` 不是設定中的
runtime id、或該 pane 不存在 → 404；runtime 無法連線或讀取失敗 → 503；單次讀取超過 5 秒 → 504；
`GET` 以外的 method（含 `HEAD`）→ 405。這個端點**所有**回應（含 400／403／404／405／503／504）都帶
`Cache-Control: no-store` 與 `X-Content-Type-Options: nosniff`；非 200 回應本體一律
`{"error": "<原因>"}`。

**與寫入端點同一套本機同源檢查**：`Host` 必須是 `127.0.0.1:<port>`、`localhost:<port>`、
`[::1]:<port>` 三者之一（`<port>` 是服務實際監聽的埠），帶 `Origin` 時其值必須是 `http://` 加同一個
`Host`；不符合回 403，且不會對 runtime 發出讀取。

**安全性**：pane 畫面內容可能含任何被那個 pane 印出來的東西（密碼、token 等）。來源檢查擋的是
`Host` 不合法（DNS rebinding）與帶了不同源 `Origin` 的請求；**沒有 `Origin` 標頭的請求會放行**——
跨站頁面（例如 `<img src=…>`）仍可能不帶 `Origin` 觸發一次真實讀取，但瀏覽器同源政策不讓它讀到回應
內容，回應本體只有本機同源頁面與本機命令列（如 `curl`）讀得到。端點唯讀、無副作用，被觸發也沒有
傷害；所有回應一律帶 `Cache-Control: no-store`，不會被任何地方快取。

## 單一執行檔

所有靜態資源（HTML、JS、CSS、manifest、PNG 圖示）都用 `include_str!`／`include_bytes!` 內嵌進
執行檔，執行期不讀檔案系統。所以 `target/release/cockpit.exe` 複製到任何目錄都能直接跑：

```bash
cargo build --release -p cockpit
# 把 target/release/cockpit.exe 複製到任何目錄後執行，/ 與 /icons/icon-192.png 一樣是 200
```

## 畫面預覽（不需要 HERDR）

`examples/ui_preview.rs` 用固定 fixture（`tests/fixtures/projected-state.json`）起一個真的
dashboard，之後每 2 秒輪替一個 pane 的狀態，供人眼與瀏覽器工具檢查：

```bash
cargo run -p cockpit --example ui_preview
# 預設 127.0.0.1:7770；COCKPIT_PREVIEW_LISTEN=127.0.0.1:8080 可改
```

## PWA 安裝

dashboard 提供 `manifest.webmanifest` 與 192／512 圖示，沒有 service worker。在 Chrome 開
`http://127.0.0.1:7770/` 之後，從網址列右側的安裝圖示或右上角選單的「安裝」把它裝成獨立視窗的
應用程式。

## 真機測試

`tests/real_attach.rs` 全部標 `#[ignore]`，需要 Windows 端 HERDR 正在跑
（`herdr status server` 顯示 `status: running`）：

```bash
cargo test -p cockpit --test real_attach -- --ignored --test-threads=1
```

它用零設定模式在程序內起一份完整的 cockpit，等 runtime 變成 `connected`，再比對 `/api/state` 的
pane 總數與 `herdr api snapshot` 的 `result.snapshot.panes` 筆數。

Scenario A／F 還會用到 WSL 端的測試 server（headless，不是使用者日常那個）：

```bash
wsl.exe -d Ubuntu-24.04 -e bash -lc "setsid -f ~/.local/bin/herdr server >/tmp/herdr-server.log 2>&1 </dev/null"
```

**禁令**：真機測試不得執行 `herdr server stop`，那會殺掉所有 pane。Windows 端的斷線只用假 server
驗，真機斷線重連只在 WSL 端做（設計文件 §10.1）。

## 驗收腳本

`docs/research/2026-09-15/` 有 change 1b 的畫面驗收紀錄與兩支可重跑的 Node 22 腳本
（`whatever-check.js`、`reconnect-check.js`），會自己啟動 `ui_preview` 並用 headless Chrome
斷言 DOM 與重連行為。

## 驗收紀錄

`docs/research/2026-09-15/change-1b-acceptance.md` 是 change 1b 真機驗收（設計文件 §10.2
Scenario A／B／F）與 task 4.4 L 訂閱補推查證的完整紀錄，含截圖與去識別化輸出。三支可重跑的
Python 腳本（`uv run --no-project python docs/research/2026-09-15/<script>.py`，全程對 HERDR
唯讀或依禁令只碰 WSL 端）：

- `attach-check.py`：Scenario A，驗兩側 runtime 皆 `connected`、pane／workspace 清單與各自
  `herdr api snapshot` 一致。
- `live-state-check.py`：Scenario B，WSL 端用 `HERDR_CLIENT_TEST_ALLOW_WSL_WRITES=1` 的寫入
  鷹架製造狀態變化，驗證即時反映到 `/api/state`。
- `reconnect-real-check.py`：Scenario F，只對 WSL 端 headless 測試 server 做斷線重連，驗
  `connected → disconnected → connected` 與內容一致。

## 品質 gate

```bash
cargo fmt --check && cargo clippy -p cockpit --all-targets -- -D warnings && cargo test -p cockpit
```
