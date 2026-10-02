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

`cockpit/Cargo.toml` 設了 `default-run = "cockpit"`：套件除了 `cockpit` 還有第二個執行檔
`cockpit-launch`（見下方「桌面啟動器」），沒有這個設定時 `cargo run -p cockpit` 會因為有多個執行檔
而失敗。

啟動成功會印出一行 `dashboard 已啟動：http://127.0.0.1:7770/（Ctrl-C 結束）`，用瀏覽器開那個網址
即可。Ctrl-C 會優雅結束：先停掉 HTTP 伺服器，再讓每個驅動器釋放事件流與子程序。

`--config` 指的檔案不存在、設定驗證不過、或 `server.listen` 綁不上（port 被占用）時，程序會印出
路徑或位址加原因後**非零結束**，不會自動換 port：

```text
$ cargo run -p cockpit -- --config missing.toml
Error: 設定檔不存在：D:\projects\ai-cockpit\missing.toml
```

### `--exit-when-idle`

```bash
cargo run -p cockpit -- --config cockpit.toml --exit-when-idle
```

帶此旗標時，後端記錄目前開著的 `/ws` 連線數，並在下列任一情況走與 Ctrl-C 相同的正常關閉流程、
結束碼 0：

- 開始監聽後 60 秒內從未有任何 `/ws` 連線。
- 曾經有過連線，目前連線數為 0，且距離「最近一次連線數降為 0」與「之後最近一次 `GET /` 或
  `GET /api/state`」兩者中較晚者已滿 10 秒。計時中有新連線就取消（重新整理頁面不會被誤殺）。

不帶旗標時行為與過去完全相同，不因連線數結束。旗標可與 `--config` 併用、順序不拘；重複給旗標視為錯誤。
這個模式是桌面啟動器（下方）「關掉視窗就結束」的基礎，規則細節見
`openspec/specs/desktop-launch/spec.md`。

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
- **狀態檔 v2（change 6 `progress-model`）**：每個 project 多一個 `active` 欄位（workstream id → 目前
  task id，見下一節），檔案 `version` 為 2。升級直接換新版即可：舊的 v1 狀態檔照常讀取（視為沒有任何
  目前 task），第一次寫入時存成 v2。**回退**：舊版 cockpit 讀到 v2 會因版本不支援而啟動失敗；回退前先停掉
  cockpit，手動把 `version` 改回 `1`、刪掉每個 project 底下的 `active` 欄位，進度與覆蓋不受影響，再換回舊版。
- **回滾注意**：`[[project]]`／`[state]` 是 change 2 新增的區段，設定檔解析一律 `deny_unknown_fields`
  （未知欄位＝啟動失敗）。換回沒有這兩個功能的舊版 `cockpit.exe` 前，要先把 `cockpit.toml` 裡的
  `[[project]]` 與 `[state]` 區段整段移除，不然舊版會直接啟動失敗；狀態檔可以留著不動，舊版本來就
  不會去讀它。

## 寫入 API（change 2 `pipeline-projection`）

進度與畫面覆蓋改由 HTTP 寫入端點操作，畫面上按對應按鈕即會送出；下列是等效的 `curl` 範例。
**寫入端點只接受本機同源請求**：`Host` 標頭必須是 `127.0.0.1:<port>`、`localhost:<port>`、
`[::1]:<port>` 三者之一（`<port>` 是服務實際監聽的埠，即 `listen` 或它綁定後真正拿到的埠），有
`Origin` 標頭時其值必須逐字等於 `http://` 加上同一個 `Host`；不符合一律 403、不改任何狀態。
同一套檢查也套在 `GET /api/state` 與 `GET /ws`（change `ws-source-check`）：外站網頁的 WebSocket 連線與 DNS rebinding
的讀取都會被 403 擋下，`/ws` 不升級；被拒的請求不算連線，也不延長 `--exit-when-idle` 的閒置期限。
下面範例對本機打 `127.0.0.1:7770`，`curl` 依網址自動送出對應的 `Host` 標頭，仍明寫出來方便對照：

```bash
# 推進（POST，五種操作 advance / retreat / complete / fail / clear 擇一，不需要本體）
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
`<op>` 不是五值之一 → 404；操作被拒絕（已是最後一站、已有標記、覆蓋的 runtime 未連線或 pane 已
exited）→ 409，本體 `{"error": "<原因>"}`；`PUT` 本體不是含 `runtime`／`pane_id` 兩個字串欄位的
JSON 物件 → 400；狀態檔寫入失敗 → 500，本體同樣是 `{"error": "<原因>"}`，記憶體中的進度維持操作
前的值（不會半套生效）。

**注意（畫面按鈕會用同一套規則回 403／500，不是只有 `curl` 才會踩到）：**

- **`Host` 比對是逐字比對，不是「任何 loopback 位址」都算**：只接受 `127.0.0.1:<port>`、
  `localhost:<port>`、`[::1]:<port>` 這三種寫法加上服務實際監聽的埠（見
  `cockpit/src/source_check.rs`）。所以 `[server] listen` 的位址只能是 `127.0.0.1` 或 `::1`、
  埠不得為 80，設成其他 loopback 位址（例如 `127.0.0.2`）或 80 埠會在啟動時就以明確訊息失敗
  （埠 0 供測試用，允許）：否則瀏覽器送出的 `Host: 127.0.0.2:<port>` 不在三種寫法裡，80 埠時瀏覽器與
  curl 又會把預設埠從 `Host` 省略，整個儀表板（`/ws`、`/api/state`）與桌面啟動器的偵測都會 403。
  用自訂 hosts 別名開啟頁面同理：頁面載得到，但 `/ws` 被拒、畫面停在連線中，請改用 `127.0.0.1`、
  `localhost` 或 `[::1]`。
- **`[state] path` 的父目錄必須事先存在**：cockpit 只會建立狀態檔本身（`.tmp` 再 `rename`），
  不會幫你建立目錄（寫檔見 `cockpit/src/progress_service.rs` 的 `write_atomically`）。如果 `path` 指到一個
  父目錄不存在的位置，每一次寫入（包含畫面按鈕）都會在寫 `.tmp` 這一步失敗，回 500，且永遠不會
  自己修好——請先手動建立好該目錄。

**推進可以退回**：畫面上 task 節點的「退回」按鈕（等效 `.../advance` 換成 `.../retreat`）把 task
移回上一個 stage；標記為 `none` 且不在第一個 stage 才接受，否則 409。`pointerdown` 事件委派比
`click` 容易誤觸（見設計文件 §8.3），誤按推進就用退回復原。退回不影響目前 task（見下一節）。

**手動改狀態檔前必須先停止 cockpit，改完才能再啟動**：cockpit 執行中對狀態檔的任何寫入——不只是
按按鈕，也包含背景任務刪除失效覆蓋（design D3；覆蓋指到的 pane 不存在或已 exited 時自動觸發）——
都會覆寫整份狀態檔（`.tmp` 寫好再 `rename` 取代），所以只要程序還在跑，任何時間點的下一次寫入都會
連同你手改的內容一起蓋掉。正確順序是：先 Ctrl-C 停掉這次執行 → 改 `cockpit.state.json` → 再重新
`cargo run -p cockpit` 或執行檔啟動，讓它在下次寫入前先把你手改的內容讀進記憶體。

## agent 回報進度（change 6 `progress-model`）

畫面上一條 workstream 的 task 只有「目前 task」會隨綁定 pane 的 agent 狀態變成 `running`／`blocked`，其餘
task 維持 `ready`。哪個 task 是目前 task 由 agent 自己宣告：agent 在自己所在的 HERDR pane 內呼叫下列
端點（Cockpit 不猜）。寫入只改 Cockpit 自己的狀態，對 HERDR 完全唯讀。沒宣告時，若 agent 正在 working，
workstream 列首會顯示「工作中・未宣告 task」。

**注意（桌面啟動器）**：用桌面捷徑啟動時後端帶 `--exit-when-idle`，最後一個 Cockpit 視窗關閉約 10 秒後後端就結束。
之後 agent 送的進度回報（以及其他寫入請求）連不上後端，連線失敗、不會被保留或重送，`curl -s` 也不會顯示任何錯誤，
進度就此遺失。agent 回報期間請保持 Cockpit 視窗開著（可以最小化），或改手動執行不帶 `--exit-when-idle` 的 `cockpit`。

| 端點 | 說明 |
|---|---|
| `GET /api/agent/tasks` | 列出綁定到這個 pane 的 workstream 與其 task（含 `id`、`stage`、`next_stage`、`mark`、`status`、`active_task`） |
| `POST /api/agent/projects/<project>/tasks/<task>/start` | 宣告這個 task 為目前 task（204） |
| `POST /api/agent/projects/<project>/tasks/<task>/advance` | 把 task 推進到下一個 stage，並設為目前 task（204） |

所有請求都要帶標頭 `X-Herdr-Pane-Id`，值取 HERDR 在 pane 內提供的 `HERDR_PANE_ID` 環境變數
（例如 `wW:p1`）。同樣只接受本機同源請求（見上一節）。agent 不能標 Completed／Failed、清除標記或退回，
那些是人的操作：`<操作>` 不是 `start`／`advance` 一律 404。

| 狀態碼 | 意義 |
|---|---|
| 204 | 成功（沒有本體） |
| 400 `missing_pane_id` | 沒帶 `X-Herdr-Pane-Id`，或值是空白 |
| 403 `pane_not_bound` | 該 task 所屬的 workstream 沒有綁定到這個 pane |
| 403 `forbidden_source` | 不是本機同源請求（`Host`／`Origin` 不符） |
| 404 | project／task 不存在，或 `<操作>` 不是 `start`／`advance` |
| 409 | 被拒絕：task 已有標記（`start`），或已是最後一個 stage／已有標記（`advance`）；本體 `{"error": "<原因>"}` |
| 500 | 狀態檔寫入失敗，記憶體不變 |

**只認非 WSL runtime 的 pane**：綁定到經由 WSL 連線的 runtime 的 pane 不算，同一個 pane id 在兩個以上
Windows runtime 都有綁定時也不算，這兩種情況 `GET` 回空的 `workstreams`、`POST` 回 403。目前 WSL 內的
agent 連不到這組 API：WSL2 預設 NAT 網路下，WSL 裡的 `127.0.0.1` 是 WSL 自己，不是 Windows，Cockpit
只監聽 Windows 的 `127.0.0.1`。

PowerShell（務必用 `curl.exe`，裸的 `curl` 在 PowerShell 是 `Invoke-WebRequest` 的別名）：

```powershell
# 先查自己的 task id
curl.exe -s http://127.0.0.1:7770/api/agent/tasks -H "X-Herdr-Pane-Id: $env:HERDR_PANE_ID"
# 宣告開始做某個 task
curl.exe -i -X POST http://127.0.0.1:7770/api/agent/projects/cockpit/tasks/impl/start `
  -H "X-Herdr-Pane-Id: $env:HERDR_PANE_ID"
# 完成一站，推進到下一個 stage
curl.exe -i -X POST http://127.0.0.1:7770/api/agent/projects/cockpit/tasks/impl/advance `
  -H "X-Herdr-Pane-Id: $env:HERDR_PANE_ID"
```

bash：

```bash
curl -s http://127.0.0.1:7770/api/agent/tasks -H "X-Herdr-Pane-Id: $HERDR_PANE_ID"
curl -i -X POST http://127.0.0.1:7770/api/agent/projects/cockpit/tasks/impl/start   -H "X-Herdr-Pane-Id: $HERDR_PANE_ID"
curl -i -X POST http://127.0.0.1:7770/api/agent/projects/cockpit/tasks/impl/advance   -H "X-Herdr-Pane-Id: $HERDR_PANE_ID"
```

可直接貼進專案 `AGENTS.md` 的短文（埠與 project id 依自己的設定改）：

```markdown
## 回報進度給 Cockpit

你的 pane 綁在 Cockpit 的一條 workstream 上。請用 `curl.exe`（PowerShell）或 `curl`（bash）回報進度，
每次都帶標頭 `X-Herdr-Pane-Id: $HERDR_PANE_ID`，網址前綴 `http://127.0.0.1:7770`：

- 開始做某個 task 時：`POST /api/agent/projects/<project>/tasks/<task>/start`。
- 完成一個 stage 時：`POST /api/agent/projects/<project>/tasks/<task>/advance`。
- 不要嘗試標完成、失敗或退回，那是人的操作。
- 不知道自己的 task id 時，先 `GET /api/agent/tasks` 查。
```

## 輸出讀取 API（change 3 `live-output`）

`GET /api/runtimes/<runtime>/panes/<pane>/output` 對指定 pane 即時讀一次目前的畫面輸出（最多最近
200 行），不快取、不在請求之間保留任何與選取有關的狀態。Live Output 面板一開始就常駐（沒有選取
時顯示空狀態文字）；畫面上點 runtime 卡裡任一未 exited 的 pane 列，或 Factory Floor 中已綁定
workstream 列首的「看輸出」，即會選取該 pane 並開始每秒輪詢一次；面板的「取消選取」可退回空狀態。
改綁模式期間 pane 列不可點選（不呈現可點選樣式），既有的選取與面板維持不變，離開改綁模式後才恢復
可點選。

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

## 檔案瀏覽與 Review（change 5a `file-review`）

Cockpit 依每個 pane 的 `cwd` 推算「檔案根目錄」（往上找到的第一個 `.git`，找不到就用 `cwd` 本身），
提供一組唯讀端點在安全邊界內列目錄、渲染 Markdown 與讀取原始檔案內容，並在下半部分頁區顯示。詳細
行為契約見 `openspec/specs/file-review/spec.md`；本節只講服務端點與安全邊界。

### 端點

- `GET /api/runtimes/<runtime>/panes/<pane>/root`：查詢該 pane 的檔案根目錄，回傳 `root_id`（之後
  所有檔案端點用它指定根目錄）與 `root_path`、`name`、`is_git` 等顯示用欄位。
- `GET /api/files/<runtime>/<root_id>/list`（根目錄本身）與 `.../list/<相對路徑>`：列出一層目錄的
  直接子項目（不遞迴），依 `.gitignore` 過濾、隱藏 `.git`。
- `GET /api/files/<runtime>/<root_id>/meta/<相對路徑>`：檔案大小、修改時間、`viewer` 分類
  （`markdown`／`pdf`／`html`／`text`／`unsupported`）、icon、`vscode_uri`。
- `GET /api/files/<runtime>/<root_id>/render/<相對路徑>`：只對 Markdown 檔案有效，伺服器用 `comrak`
  渲染成 HTML 片段（GFM 表格／任務清單／刪除線／自動連結），原始 HTML 與危險連結一律被清掉。
- `GET /api/files/<runtime>/<root_id>/raw/<相對路徑>`：檔案原始位元組，`Content-Type` 依副檔名決定。
  `.html`／`.htm` 另看內容：整份回傳位元組是合法 UTF-8 時回 `text/html; charset=utf-8`（沒寫
  `<meta charset>` 的 UTF-8 報告在繁中 Windows 上才不會被當成 Big5 而顯示亂碼），不是時回不帶 charset
  的 `text/html`，交給檔內 `<meta charset>` 或 BOM 決定編碼。
- `GET /vendor/<路徑>`：內嵌的第三方前端資源（pdf.js、Material Icon Theme 的 icon 與對照表），公開
  靜態資源，不套用下方的來源檢查與允許清單。

非 200 回應本體固定為 `{"error": "<中文原因>", "code": "<代碼>"}`；完整代碼與狀態碼對照見
`openspec/specs/file-review/spec.md`「檔案端點的共同規則」。

### 安全邊界

- **允許清單＝目前所有 pane 的根目錄**：每個請求當下重新對最新投影中該 runtime 的每個 pane（含
  `exited`）推算根目錄，找到相符的才放行；不在清單內（含先前曾經可用、pane 已消失的根目錄）一律
  `404 root_unavailable`。不快取，沒有伺服器端 session 或 token。
- **相對路徑從原始 URI 取得，逐段檢查**：HTTP 層直接切原始請求路徑（仍是 percent-encoded 的字面
  字串），不使用 axum 已解碼過的 `Path` 擷取值——後者的一次性解碼會讓 `..%2F..` 這類多重編碼在到
  達邊界檢查前就被還原成字面的 `/`、`..`，形同繞過逐段檢查。空片段、`.`、`..`、含 `/`、`\`、`:`、
  NUL、以 `.` 或空白結尾、或是 Windows 保留裝置名（`CON`、`PRN`、`NUL`、`COM1`–`COM9`、`LPT1`–
  `LPT9` 等）一律 `400 bad_request`，在碰檔案系統之前就擋掉。
- **實體路徑必須在根目錄內**：根目錄與目標各自 `canonicalize` 後以 `Path::starts_with` 比對；
  **符號連結、junction、懸空連結（reparse point 指到根外或已不存在）一律視為跳出根目錄**，回
  `403 path_outside_root`，不區分「根外存在」與「根外不存在」（避免以錯誤碼種類探測根外檔案系統
  是否存在）。列目錄時的 `.gitignore` 過濾只影響「列出哪些項目」，不是存取控制——被過濾掉的檔案
  仍可用 `raw`／`meta` 直接讀到；規則檔本身（`.gitignore`、`.git`、`.git/info/exclude`）若是連結一律
  略過（同 git 對 `.gitignore` 是連結時的行為），祖先目錄被忽略時該層列表回空清單（非錯誤），不會
  重新套用其內規則。
- **WSL runtime 的 repo 內符號連結一律讀不到**：Windows 經 `\\wsl.localhost` 看到的 Linux 符號連結是無法
  跟隨的 reparse point（解不出目標、直接開檔也失敗），所以不論連結指向根目錄外或 repo 內部，都依上一條
  一律 `403 path_outside_root`；列目錄時這類連結一律顯示為檔案（Windows 端無法得知它指向資料夾），點開
  得到同一個錯誤。這是 fail-closed 的功能限制，不是安全問題（task 5.2 實測，見
  `docs/research/2026-09-27/file-review-probe.md` 第 6 節）。
- **只接受 `GET`**：其他 method（含 `HEAD`）一律 `405 method_not_allowed`，且不對檔案系統或 runtime
  發出任何存取。未命中任何本節路由形狀的請求（例如 `list/` 空尾、缺相對路徑、未知端點名）落到
  axum 預設的空本體 404，不帶 `no-store`／`nosniff` 標頭——這些不是本節定義的端點。
- **Host／Origin 檢查**：與寫入端點、輸出讀取端點同一套本機同源檢查（見上方「寫入 API」），不符合
  回 `403 forbidden_source`。
- **根目錄判定 fail-closed**：非 WSL runtime 的 pane，其 `cwd` 不是絕對路徑或含 `..` 時一律視為沒有
  根目錄（`404 no_root`），不嘗試猜測；請求路徑先解碼 `root_id`（400）再查 runtime 是否存在
  （404），順序固定。
- **回應標頭**：所有回應（含錯誤）帶 `Cache-Control: no-store` 與 `X-Content-Type-Options: nosniff`；
  `render`／`raw` 另帶 `Content-Security-Policy: sandbox`，讓直接開啟該網址時 HTML／SVG 內容裡的
  腳本不會在 Cockpit 的來源下執行（`source_check` 擋下的 403／405 本身不含檔案內容，不加這個標頭）。
  中繼資料的 `icon` 依請求字面檔名決定、原始內容的 `Content-Type` 依實體路徑（`canonicalize` 後）的
  副檔名決定，8.3 短檔名或連結情境下兩者可能不一致，只影響顯示，不影響安全邊界。
- **大小上限**：中繼資料端點只讀前 8192 位元組判斷 `viewer` 種類，不受上限限制；Markdown 渲染上限
  2 MiB（超過回 `413 too_large`）；原始內容上限 50 MiB。不支援 HTTP Range，pdf.js 以整檔讀取。
- **不寫入、不執行**：這組端點沒有任何寫入、建立、刪除檔案或執行外部程式的路徑，純讀取。

**已知風險與裁決（設計文件與控制端裁決已記錄、刻意不額外限制）**：

- 允許清單的範圍是「目前有 pane 的整個 repo」而不是單一檔案；若某個 shell pane 的 `cwd` 不在 git
  repo 內（例如停在使用者家目錄或 `C:\`），該 pane 的檔案根目錄就是 `cwd` 本身，會讓整個家目錄或
  整顆磁碟進入允許清單。這仍只對本機同源請求開放、且只讀，等同使用者在那個 pane 裡本來就能讀到的
  範圍，因此不另設限制；使用者若在意，避免讓 shell pane 停在家目錄或磁碟根目錄即可。
- PDF 檢視器把「檔案能讀到但 pdf.js 無法解析」視為成功讀取後的一種顯示內容（顯示「PDF 無法解析」），
  不算讀取失敗，不會加上過期標示；過期標示只套用在讀取本身失敗（檔案消失、逾時等）的情況。
- HTML 檢視器以 iframe 的 `src` 直接載入原始內容端點（不改寫使用者的 HTML，相對路徑的樣式表與圖片
  才載得到）。iframe 的 `load` 事件分不出 HTTP 成功與否、sandbox 文件的內容也讀不到，所以新版本先在
  隱藏的新 iframe 載入（網址加一個唯一的 `?_cv=<序號>`，服務端忽略 query），再從父頁的 Resource Timing
  取這次導覽的 `responseStatus`：是 200 才換掉舊 iframe；不是 200、取不到或 10 秒內沒有結果就丟掉新
  iframe、保留舊內容並標過期，同一個版本下一輪輪詢重試。狀態碼只能對應到大致的錯誤原因（例如 404 一律
  顯示「檔案已不存在」），取不到狀態碼（連線失敗等）時顯示「讀取時發生錯誤」。

### vendored 資源升版

`cockpit/assets/vendor/` 下的 pdfjs-dist 與 Material Icon Theme 版本、下載來源、SHA-256 與升版步驟
記在 `cockpit/assets/vendor/README.md`，升版時照該檔案的步驟重做。

## git 唯讀讀取（change 5b `git-review`）

Cockpit 在檔案根目錄是 git repo 時（`is_git` 為真），另外提供一組唯讀端點讀取 git 狀態、commit
歷史與檔案在不同版本的內容，供「變更」面板、diff 分頁、Git Graph 分頁與某版本檔案分頁使用。完整
行為契約見 `openspec/changes/git-review/specs/git-review/spec.md`（尚未 archive 前）與
`docs/adr/0007-cockpit-git-crate.md`（依賴邊界與執行方式的決策理由）；本節只講服務端點與安全邊界。

### 端點

路徑前綴皆為 `GET /api/git/<runtime>/<root_id>/…`：

- `status`：目前分支、暫存／未暫存／未追蹤／合併衝突的變更清單。
- `refs`：分支、遠端追蹤分支、tag 清單與 HEAD。
- `log`：commit 清單（分批載入、Graph 排版）。
- `commit/<hash>`：單一 commit 的詳情與變更檔案清單。
- `changes`：兩個版本之間的變更檔案清單。
- `diff`：單一檔案在兩個版本之間的左右並排差異（含未追蹤檔案）。
- `merge-base`：兩個 commit 的共同祖先。
- `meta/<rev>/<相對路徑>`、`blob/<rev>/<相對路徑>`、`render/<rev>/<相對路徑>`：某個版本（commit
  hash 或 `INDEX`）中檔案的中繼資料、原始內容、Markdown 渲染——不讀取工作區。

非 200 回應本體固定為 `{"error": "<中文原因>", "code": "<代碼>"}`；完整代碼與狀態碼對照見 spec
「git 端點的共同規則」。

### 安全邊界

- **執行 git 的查詢種類在程式中封閉**：能執行的子命令與引數由 `cockpit-git` crate 的 sealed
  `GitQuery` 在編譯期限定（清單見 `cockpit-git/src/query.rs`），HTTP 層完全沒有「自訂 argv」的
  入口；`cockpit` 這一側也沒有任何 `Command::new`，執行子程序一律經同一個
  `cockpit_git::GitRunner`（同時最多 4 支、逾時 10 秒）。
- **固定的唯讀前綴**：每次呼叫都帶 `--no-pager --no-optional-locks --literal-pathspecs` 等旗標，
  不取得 optional lock、不寫 index、不執行 repo 設定指定的 fsmonitor／外部 diff／textconv／簽章
  驗證程式，路徑一律當字面路徑（不解讀 pathspec 語法）。工作區側的 diff 用 plumbing
  （`diff-files`／`diff-index`）而不是 `git diff`：後者遇到 stat 變舊的檔案會重寫 `.git/index`，
  `--no-optional-locks` 擋不住。
- **WSL repo 在 WSL 內以 `--exec` 執行**：根目錄主機路徑以 `\\wsl.localhost\<distro>\` 開頭時，git
  改由 `wsl.exe -d <distro> --exec env LC_ALL=C git -C <posix repo 路徑> …` 在該 distro 內執行
  （不經 shell、引數不被重新解讀），其餘（含 Windows 端看到的其他 UNC 路徑）用 Windows 的 git。
- **不動 `safe.directory`**：git 對「擁有者不是目前使用者」的 repo（dubious ownership）回報的錯誤
  一律分類成 502 `git_untrusted` 並直接回報，不加 `-c safe.directory=…` 之類的旗標繞過這個檢查。
- **需要安裝 git**：Windows 端要能在 `PATH` 找到 `git`；根目錄在 WSL 時，該 distro 內也要能找到
  `git`（`wsl.exe --exec` 直接呼叫，不經登入 shell 的 `PATH` 設定）。找不到時回 503
  `git_unavailable`，`ui_preview` 範例在找不到本機 git 時啟動即失敗（不靜默降級）。
- **其餘規則沿用 5a**：`root_id` 與允許清單、相對路徑片段規則、路徑界限、`Host`／`Origin` 檢查、
  回應標頭、`GET`-only 皆與「檔案瀏覽與 Review」一節相同（`diff` 的未追蹤檔案分支與
  `meta`／`blob`／`render` 直接重用同一套 [`resolve`／`read_capped`]／viewer 分類／Markdown 渲染／
  原始內容 content-type 規則）。

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

## 桌面啟動器（Windows）

一個桌面捷徑就開好：捷徑指向 `cockpit-launch.exe`，它是 Windows 圖形子系統執行檔，啟動時不會出現
主控台視窗。決策背景見 `docs/adr/0005-browser-pwa-before-tauri.md` 的「2026-10-02 補充」。

### `cockpit-launch` 做什麼

只接受 `--config <path>`（或 `--config=<path>`），設定的決定規則與 `cockpit` 相同，由此取得監聽位址。依序：

1. 選瀏覽器：環境變數 `COCKPIT_BROWSER`（指向的檔案存在才用）→ Google Chrome → Microsoft Edge，
   各自在標準安裝位置尋找；都找不到時以訊息框說明（此時尚未啟動任何後端）。
2. 對 `http://<監聽位址>/api/state` 偵測：已有 Cockpit 在跑就只開視窗；連得上但不是 Cockpit
   （埠被其他程式占用）以訊息框說明。
3. 沒在跑就在背景啟動同目錄的 `cockpit.exe`（帶絕對路徑的 `--config` 與 `--exit-when-idle`，
   不建立主控台視窗），並每 200 毫秒檢查一次是否就緒，最多 15 秒。
4. 以 `--app=http://<監聽位址>/` 開瀏覽器的獨立視窗後結束。不指定 `--user-data-dir`，沿用你的瀏覽器
   設定檔，通知權限因此能跨次保留。

最後一個視窗關閉滿 10 秒後，後端開始正常結束（`--exit-when-idle`；真機實測約 11 秒後行程消失）。

**後果：用捷徑啟動時，視窗關著就沒有後端。** 後端結束後，agent 回報進度的端點與寫入類請求都連不上，在此期間送出的
請求連線失敗、不會被保留或重送（`curl -s` 不會顯示任何錯誤）。agent 還在工作時請保持 Cockpit 視窗開著（可以最小化），
或改手動執行不帶 `--exit-when-idle` 的 `cockpit`。

後端的標準輸出與標準錯誤寫到設定檔所在目錄的 `cockpit.log`（零設定模式時寫到工作目錄），
每次啟動覆寫，內容不含 ANSI 色碼；該檔已列入 `.gitignore`。任何失敗（找不到瀏覽器、埠被占用、
後端啟動失敗或 15 秒內未就緒）都以 Windows 訊息框說明，後端啟動失敗時訊息含 `cockpit.log` 的路徑
與最後 20 行。非 Windows 平台編譯時，啟動器只印出「僅支援 Windows」並以非 0 結束。

環境變數：

- `COCKPIT_BROWSER`：瀏覽器執行檔的完整路徑，覆寫上述尋找順序。
- `COCKPIT_LAUNCH_DIALOG_FILE`：**測試用入口**。有值時不顯示訊息框（訊息框會阻塞），改把訊息以 UTF-8
  附加寫入該檔，供自動驗收讀取。一般使用不要設。

### 安裝腳本 `scripts/install-desktop.ps1`

```powershell
# PowerShell 7
pwsh scripts/install-desktop.ps1

# Windows PowerShell 5.1
powershell -ExecutionPolicy Bypass -File scripts\install-desktop.ps1 -Config D:\work\cockpit.toml
```

腳本以正式版建置 `cockpit` 套件的兩個執行檔，複製到安裝目錄，並在桌面建立「AI Agent Cockpit」捷徑
（目標為安裝目錄的 `cockpit-launch.exe`）。重複執行即為更新。參數：

| 參數 | 預設 | 說明 |
|---|---|---|
| `-Config` | repo 根目錄的 `cockpit.toml` | 設定檔。存在時捷徑帶 `--config "<絕對路徑>"`、工作目錄為設定檔所在目錄；不存在時警告並以零設定模式執行（捷徑不帶引數，工作目錄為 `%LOCALAPPDATA%\ai-cockpit\`） |
| `-InstallDir` | `%LOCALAPPDATA%\ai-cockpit\bin` | 執行檔安裝目錄 |
| `-ShortcutDir` | 使用者桌面 | 捷徑所在目錄；測試時可指到暫存目錄，以免動到真實捷徑 |

安裝目錄的 `cockpit.exe` 或 `cockpit-launch.exe` 正在執行時，腳本會在複製前停止、提示先關閉 Cockpit
並以非 0 結束，不會覆寫執行中的檔案（關掉視窗約 10 秒後後端自動結束）。

### 桌面通知

通知是純前端功能：瀏覽器比對前後兩份推送狀態，在 pane 的 agent 狀態變成 `blocked`／`done`、
task 狀態變成 `failed`／`completed` 時發通知；視窗在前景時不發。頂列的鈴鐺按鈕開啟設定面板，
四種事件各自開關（預設只開 blocked 與 failed）並顯示通知權限狀態。

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
