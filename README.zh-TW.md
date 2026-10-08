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
- **英文或繁體中文介面。** 儀表板依瀏覽器語言與時區自動選一種，頂列的按鈕可以在兩種之間切換。

## 設計上就是唯讀

Cockpit 是旁觀者：不能在 pane 裡打字、不能對 agent 下指令，也不能停掉 HERDR。

- 能對 HERDR 呼叫的只有 `session.snapshot`、`pane.read`、`events.subscribe` 三個，由
  `herdr-client` 的 sealed trait 在編譯期限定。
- 儀表板只聽 loopback（預設 `127.0.0.1:7770`）。所有會回傳或改動你資料的端點，包括即時狀態
  （`/api/state` 與 `/ws`），都會檢查 `Host` 與 `Origin` 標頭，你瀏覽器裡開著的其他網頁讀不到儀表板。
- 進度只在你按下按鈕、或 agent 自己回報時才會改變。Cockpit 不寫你的 repo，也不寫 HERDR；它自己的
  檔案是 `cockpit.state.json`，用桌面啟動器時另有 `cockpit.log` 與 `cockpit.update.json`（上次檢查更新的時間與結果）。

## 開始使用

測試過的平台是 Windows，也支援 WSL 裡的 HERDR。桌面啟動器需要 Chrome 或 Edge；git 相關畫面需要
`PATH` 上有 `git`。測試過的版本是 Windows 的 HERDR 0.9.0-preview 與 WSL 的 HERDR 0.8.2。

### 安裝（Windows x64）

從[最新版 release](https://github.com/Benjamin-Teng/ai-cockpit/releases/latest) 下載
`ai-cockpit-<版本>-x64-setup.exe` 並執行。它只裝給你這個帳號、不需要管理員權限，裝在
`%LOCALAPPDATA%\Programs\AI Agent Cockpit`，並在開始功能表與桌面（可取消勾選）放 **AI Agent Cockpit**
捷徑。要移除就到 Windows「**設定 > 應用程式**」解除安裝。不想安裝的話，同一個 release 的 zip 裡是同樣的兩個程式。

- **第一次執行**：安裝檔尚未做程式碼簽章，Windows SmartScreen 可能顯示「Windows 已保護您的電腦」，
  按「**其他資訊**」再按「**仍要執行**」。想先確認檔案，可拿同一個 release 附的 `SHA256SUMS.txt` 核對雜湊值。
- **設定檔**：捷徑以 `%LOCALAPPDATA%\ai-cockpit` 為工作目錄執行 Cockpit，把 `cockpit.toml` 放在這裡
  （可從安裝資料夾裡的 `cockpit.example.toml` 開始改）；沒有就以零設定執行。解除安裝會保留這個資料夾。
- **原本用 `install-desktop.ps1` 裝過？** 安裝檔會取代桌面上同名的捷徑，新捷徑不帶 `--config`。請把
  `cockpit.toml` 連同旁邊的 `cockpit.state.json`（Factory Floor 的進度）一起移到 `%LOCALAPPDATA%\ai-cockpit`。
  Cockpit 在設定檔旁邊找狀態檔，只搬設定檔會從空白看板開始。設定檔裡的 `[state] path` 若是相對路徑，
  請改成絕對路徑或把那個檔案一起搬。最後刪掉舊的 `%LOCALAPPDATA%\ai-cockpit\bin` 資料夾。

#### 更新

- **自動更新，僅限安裝檔版。** 用安裝檔安裝的話，從捷徑開啟 Cockpit、且後端當時沒在執行，就會去
  GitHub 查有沒有較新的正式版，每 24 小時最多查一次。有新版會先詢問你。選「是」，Cockpit 會下載安裝檔、
  用該 release 的 `SHA256SUMS.txt` 驗證、關閉、更新，然後自動重新開啟；下載或驗證失敗時會顯示錯誤，
  並開啟你現有的版本。選「否」，下次檢查時會再問。預發布版不會被提供。
- **SmartScreen 提示通常只在第一次安裝出現。** 這個提示跟著瀏覽器加在檔案上的「從網路下載」標記走。
  更新是 Cockpit 自己下載、不經過瀏覽器，檔案沒有這個標記。
- **會連網。** 每次檢查向 `api.github.com` 發一個請求。設定環境變數 `COCKPIT_NO_UPDATE_CHECK=1`
  可以完全關閉檢查。
- **不會自動更新的情況：** zip 版、用 `install-desktop.ps1` 安裝的、從原始碼建置的。請到
  [Releases 頁面](https://github.com/Benjamin-Teng/ai-cockpit/releases)手動下載新版。
- **從 v0.1.0 升級？** 這個預發布版沒有更新器，需要手動安裝一次新版，之後就會自動更新。
- **公司網路？** 網路若走系統代理或會檢查 TLS，Cockpit 可能連不到 GitHub。它找代理只讀
  `HTTPS_PROXY` 這類環境變數，也不使用 Windows 的憑證存放區。這時檢查會失敗而且不顯示任何訊息，
  Cockpit 照常開啟；請到 Releases 頁面手動更新。

### 從原始碼建置

需要 Rust（edition 2024）。在 Windows 上還需要 Windows SDK，Rust 使用的 Visual Studio C++ 建置工具會一起裝；
建置時用它的 `rc.exe` 把程式圖示嵌進執行檔。

```bash
git clone https://github.com/Benjamin-Teng/ai-cockpit.git
cd ai-cockpit
cargo run -p cockpit
```

打開 <http://127.0.0.1:7770/>。沒有設定檔時，Cockpit 會自己找到本機的 HERDR。

要追蹤某個 git repo，不需要設定檔：見[把 repo 加成專案](#把-repo-加成專案)。
要手動描述你的專案，先複製範例設定檔。執行前換掉裡面的 `<user>` 佔位字，再列出 HERDR runtime、
每個專案的階段，以及每條工作流在哪個 pane 裡跑。Cockpit 會讀工作目錄裡的 `cockpit.toml`，
或 `--config` 指定的檔案。手寫的 `[[project]]` 照常支援，和畫面上加入的專案並列；
兩者 id 相同時，手寫的優先。

```powershell
Copy-Item cockpit.example.toml cockpit.toml
cargo run -p cockpit
```

#### 桌面捷徑（Windows，可略過）

```powershell
pwsh scripts/install-desktop.ps1
```

會建置 release 版、安裝到 `%LOCALAPPDATA%\ai-cockpit\bin`，並在桌面放一個
**AI Agent Cockpit** 捷徑。這個模式下，關掉最後一個視窗約 10 秒後 Cockpit 就會結束，之後
送來的進度回報會遺失。agent 還在工作時，請把視窗最小化而不是關掉。

#### 還沒有 HERDR？

```bash
cargo run -p cockpit --example ui_preview
```

會在 <http://127.0.0.1:7770/> 用範例資料打開儀表板。

## 把 repo 加成專案

不用改 `cockpit.toml`、也不用重啟，直接在儀表板上加入 git repo：

1. 在 HERDR 裡於該 repo 內開一個 pane。
2. 左欄切到 **Project** 分頁，這個 repo 會列在「偵測到的 repo」。
3. 按「加入」。

之後，只要 pane 的工作目錄在這個 repo 裡（任一 worktree、任一 HERDR workspace），就自動成為
一條工作線，每條工作線各有一張 task 卡。新專案預設有四個 stage（規劃、實作、審查、完成）。
用專案的「⋯」選單可以改名、編輯 stage（改名、新增、刪除、排序）或移除，改完立即生效。位於
linked git worktree 的 pane 會標出 worktree 的資料夾名稱。關掉 pane 時，它的工作線與 task 進度
一起移除；pane 已 exited 但還留在 HERDR 裡時，它的工作線會從畫面消失，進度則保留到 pane 真正關閉才清除。

注意事項：

- pane 的工作目錄取自 HERDR 的 snapshot，Cockpit 預設每 30 秒重新抓一次。pane 內 `cd` 到另一個
  repo 後，最多約 30 秒才會歸到新的 repo。
- Cockpit 以 git 的共同目錄區分 repo，所以同一個 repo 的所有 worktree 算同一個專案。同一個
  資料夾若分別從 Windows 與 WSL（`/mnt/d/...`）開啟，會列成兩個 repo。
- 畫面上加入的專案存在 `cockpit.state.json`。沒有設定檔時，這個檔案是
  `%LOCALAPPDATA%\ai-cockpit\cockpit.state.json`。這個版本寫出狀態檔第 3 版，要回到舊版前請先看
  [CHANGELOG](CHANGELOG.md)。

## 讓 agent 回報進度

在 agent 自己的 HERDR pane 裡執行（限 Windows 端的 runtime）。在由 repo 加入的專案裡，一行就夠：
Cockpit 會依 pane ID 找到這個 pane 的 task，不必帶專案或 task id。

```bash
# 完成這一站，推進到下一站（不必帶專案或 task id）
curl -i -X POST http://127.0.0.1:7770/api/agent/advance -H "X-Herdr-Pane-Id: $HERDR_PANE_ID"
```

Cockpit 找不到唯一可推進的 task 時回 404 `no_task_for_pane`（pane 沒有綁定的工作線，或有多張 task 且沒有目前
task）或 409 `ambiguous_task`（有多張候選），這時請明確指定 task，或先 `start`。和其他
agent 端點一樣，WSL runtime 的 pane 不適用。手寫的專案也可以明確指定 task：

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
