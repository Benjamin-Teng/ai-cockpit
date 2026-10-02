<!-- markdownlint-disable-next-line MD041 -->
[English](README.md) | [繁體中文](README.zh-TW.md)

![AI Agent Cockpit：四條 agent 工作流以點陣訊號依序經過 Plan、Build、Review](docs/assets/readme-banner.svg)

# AI Agent Cockpit

本機執行、對 [HERDR](https://herdr.dev) 唯讀的 AI coding agent 儀表板。
Cockpit 把 HERDR 的 pane 變成即時的工廠樓層：每個 agent 負責哪條工作流、走到哪個階段、
哪一個正在等你。

**網站：** <https://benjamin-teng.github.io/ai-cockpit/>

![Cockpit 儀表板（範例資料）：專案清單、工作流與階段交錯的 Factory Floor、HERDR runtime 卡片](docs/research/2026-10-01/progress-1536.png)

## 功能

- **Factory Floor。** 每個專案是一條由階段組成的流水線，各工作流並排前進。task 卡片標出
  正在執行、卡住還是失敗；推進、退回或標記都只要按一下。
- **Windows 與 WSL 合成一張全貌。** Cockpit 同時連上多個 HERDR runtime 並合併顯示，每個都有
  自己的連線燈號。
- **即時輸出，保留顏色。** 隨時讀任何 pane 最近的輸出，顏色照原樣呈現。它只是檢視，不是
  終端機，你在這裡的操作不會傳到 agent。
- **檔案、差異與 git 歷史。** 瀏覽 pane 正在處理的 repo：Markdown、PDF、HTML、並排差異和
  commit 圖，全部唯讀。
- **agent 自己回報進度。** agent 在自己的 pane 裡發一個 HTTP 請求，就能宣告手上的 task 並
  往下推進。Cockpit 不靠猜。目前只支援 Windows 端的 pane。
- **桌面通知。** agent 卡住或 task 失敗時跳出通知，其他事件也能自行開啟。Cockpit 視窗開著或
  最小化、且不在最前面時才會通知。點通知就直接打開那個 pane。
- **像桌面 App 一樣開啟。** 點 Windows 桌面捷徑就在背景啟動 Cockpit，並開在獨立視窗裡；
  關掉視窗，Cockpit 跟著結束。

## 設計上就是唯讀

Cockpit 是旁觀者：不能在 pane 裡打字、不能對 agent 下指令，也不能停掉 HERDR。

- 能對 HERDR 呼叫的只有 `session.snapshot`、`pane.read`、`events.subscribe` 三個，由
  `herdr-client` 的 sealed trait 在編譯期限定。
- 儀表板只聽 loopback（預設 `127.0.0.1:7770`）。改變進度、讀 pane 輸出、瀏覽檔案、查 git 的端點會
  檢查 `Host` 與 `Origin` 標頭；即時狀態（`/api/state` 與 `/ws`）目前還沒檢查，所以 Cockpit
  執行時，你瀏覽器裡開著的網頁讀得到儀表板狀態。
- 進度只在你按下按鈕、或 agent 自己回報時才會改變。Cockpit 不寫你的 repo，也不寫 HERDR；它自己的
  檔案是 `cockpit.state.json`，用桌面啟動器時另有 `cockpit.log`。

## 開始使用

Cockpit 用 Rust（edition 2024）從原始碼建置。測試過的平台是 Windows，也支援 WSL 裡的
HERDR。桌面啟動器需要 Chrome 或 Edge；git 相關畫面需要 `PATH` 上有 `git`。測試過的版本是 Windows 的 HERDR 0.9.0-preview 與 WSL 的
HERDR 0.8.2。

```bash
git clone https://github.com/Benjamin-Teng/ai-cockpit.git
cd ai-cockpit
cargo run -p cockpit
```

打開 <http://127.0.0.1:7770/>。沒有設定檔時，Cockpit 會自己找到本機的 HERDR。

要描述你的專案，先複製範例設定檔。執行前換掉裡面的 `<user>` 佔位字，再列出 HERDR runtime、
每個專案的階段，以及每條工作流在哪個 pane 裡跑。Cockpit 會讀工作目錄裡的 `cockpit.toml`，
或 `--config` 指定的檔案。

```powershell
Copy-Item cockpit.example.toml cockpit.toml
cargo run -p cockpit
```

### 桌面捷徑（Windows，可略過）

```powershell
pwsh scripts/install-desktop.ps1
```

會建置 release 版、安裝到 `%LOCALAPPDATA%\ai-cockpit\bin`，並在桌面放一個
**AI Agent Cockpit** 捷徑。這個模式下，關掉最後一個視窗約 10 秒後 Cockpit 就會結束，之後
送來的進度回報會遺失。agent 還在工作時，請把視窗最小化而不是關掉。

### 還沒有 HERDR？

```bash
cargo run -p cockpit --example ui_preview
```

會在 <http://127.0.0.1:7770/> 用範例資料打開儀表板。

## 讓 agent 回報進度

在 agent 自己的 HERDR pane 裡執行（限 Windows 端的 runtime）：

```bash
# 列出綁定到這個 pane 的 task
curl -s http://127.0.0.1:7770/api/agent/tasks -H "X-Herdr-Pane-Id: $HERDR_PANE_ID"
# 宣告正在做的 task
curl -i -X POST http://127.0.0.1:7770/api/agent/projects/cockpit/tasks/impl/start   -H "X-Herdr-Pane-Id: $HERDR_PANE_ID"
# 完成這一站，推進到下一站
curl -i -X POST http://127.0.0.1:7770/api/agent/projects/cockpit/tasks/impl/advance -H "X-Herdr-Pane-Id: $HERDR_PANE_ID"
```

`cockpit` 與 `impl` 是範例設定檔裡的專案與 task id，請換成你自己的。
PowerShell 要用 `curl.exe` 與 `$env:HERDR_PANE_ID`。把 task 標成完成或失敗留給你決定。
完整 API，以及可以直接貼進專案 `AGENTS.md` 的說明，在 [`cockpit/README.md`](cockpit/README.md)。

## 架構

Cargo workspace：

| Crate | 角色 |
|---|---|
| `herdr-client` | HERDR 協定 client 與傳輸層（named pipe、Unix socket、WSL 用的子程序 stdio）；只含觀察用的 method |
| `cockpit-core` | runtime 與 domain 模型、每個 runtime 的 driver、投影成儀表板狀態；不含 HERDR 型別 |
| `cockpit-herdr` | 把 HERDR 翻譯成 core 型別，是唯一同時看得到兩邊的 crate |
| `cockpit-files` | 檔案瀏覽：根目錄白名單與安全路徑 |
| `cockpit-git` | 封閉的唯讀 git 查詢層 |
| `cockpit` | 主程式：axum HTTP 與 WebSocket、內嵌的單頁介面，以及 `cockpit-launch` 桌面啟動器 |

設計決策在 [`docs/adr/`](docs/adr/)，詞彙表在 [`CONTEXT.md`](CONTEXT.md)，各能力的規格在
[`openspec/specs/`](openspec/specs/)。
