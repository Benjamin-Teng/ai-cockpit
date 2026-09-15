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
