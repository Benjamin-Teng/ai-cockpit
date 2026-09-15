# attach-herdr-runtimes（change 1b）

## Why

change 1a 交付的 `herdr-client` 只是協定層：能對 HERDR 拿一次 snapshot、收一條事件流，但沒有
狀態、沒有連線迴圈、沒有畫面，使用者仍然看不到任何 agent。change 1b 把它變成第一個可以跑的
觀測台：同時連上 Windows 端與 WSL 端兩個 HERDR、在記憶體維持投影狀態、在本機瀏覽器即時顯示
每個 pane 的 agent 狀態。對應 `docs/cockpit-spec.md` §20 必做項 4、5、8、10、13 與 12 的
agent 層視覺（Working／Blocked／Idle／Done／Unknown），驗收為 §26 的 Scenario A Attach、
B Live state、F Reconnect（設計文件 §1、§10.2）。

現在做的理由：1a 已 archive、主規格 4 份就位；1b 建立的 `cockpit-core` 與 `cockpit` 是
change 2（pipeline 投影）與 change 3（live output）直接往上疊的底座，形狀在這裡定下來。

## What Changes

- **新 crate `cockpit-core`**（只懂 Cockpit，不依賴 `herdr-client`，ADR-0003）：Runtime 層
  模型（`RuntimeId`、Cockpit 自己的 `AgentStatus`、`Workspace`／`Tab`／`Pane`／`Agent`／
  `Focused`、`RuntimeSnapshot`、`RuntimeEvent` 含 `Drift`／`Noted`、`ConnectionState`）、
  `AgentRuntime` trait、`RuntimeStore`（`replace`／`apply`，Drift 判定）、`ProjectedState`
  投影（遞增 `version`、內容不變不遞增、50 ms 合併、最近事件 ring buffer）、以及一個與
  runtime 種類無關的**連線驅動器**（訂閱先開好 → authoritative snapshot 整份替換 → 串流套用；
  Drift 與定期重拿 snapshot；退避）。
- **新 crate `cockpit-herdr`**：`HerdrRuntime` 以 `herdr-client` 的 `Client`／`Connector`
  實作 `AgentRuntime`：WSL 探測、seed snapshot、L（24 種生命週期）與 S（每 pane
  `pane.agent_status_changed`；沒有全域狀態訂閱，設計文件 §2.3）兩條訂閱合併成一條事件流、
  pane 集合改變時重開 S（ReopenStatus，§4.2）、HERDR snapshot／事件翻成 core 型別（§7.2 對照
  表）、protocol 版本落差警告；並提供由設定組出 runtime 的工廠，讓 `cockpit` 不直接依賴
  `herdr-client`（ADR-0003 的依賴圖）。
- **新 crate `cockpit`**（bin，產出 `cockpit.exe`）：TOML 設定檔與零設定模式、每個 runtime
  一個驅動器 task、axum HTTP ＋ WebSocket 只綁 `127.0.0.1`、內嵌靜態網頁（`index.html`、
  `app/channel.js`、`app/render.js`、`app/style.css`）、PWA manifest 與圖示、`/api/state`
  除錯端點。畫面每次收到整張圖就整頁重畫，沒有任何可點的互動。
- 根 `Cargo.toml` workspace members 加入三個 crate；`herdr-client` 的 `test-support`
  假 HERDR 以 dev-dependency 供 `cockpit-herdr` 迴圈測試重用（必要時只做加法擴充 builder）。
- 真機驗收：設計文件 §10.2 Scenario A／B／F；併入 1a 遺留的 spike 4 目視複驗（從 HERDR pane
  啟動 `cockpit.exe` 時不出現新視窗）與「WSL 0.8.2 的 L 訂閱剛建立時補推舊事件」在 Windows
  0.9.0 端是否也發生的查證（一手觀察：`docs/research/2026-09-13/change-1a-spikes.md` spike 3
  「額外觀察」；`docs/handover.md` §3 列為待查證）。
- 不含 **BREAKING**：`herdr-client` 公開 API 不變。

## Capabilities

### New Capabilities

- `runtime-model`：`cockpit-core` 的 Runtime 層型別、`AgentRuntime` trait、`RuntimeStore` 的
  `replace`／`apply` 語意與 Drift 判定（設計文件 §6.1–§6.3）。
- `runtime-driver`：與 runtime 種類無關的連線生命週期：訂閱先開、authoritative snapshot 之前
  的事件一律丟棄、Drift 與定期重拿 snapshot、退避序列與探測類錯誤的固定間隔、
  `ConnectionState` 的轉換（設計文件 §4.2、§7.1 中與 HERDR 無關的部分）。
- `state-projection`：`ProjectedState` 的 JSON 形狀、`version` 只在內容改變時遞增、50 ms
  合併廣播、最近事件 50 筆（設計文件 §6.3、§6.4、ADR-0004）。
- `herdr-runtime-translation`：HERDR snapshot 與 26 種事件翻成 core 型別的規則，含未知值歸
  `Unknown`、`released` 清 agent、payload 不足即 `Drift`、`Done` 不代表完成（設計文件 §7.2、
  §9）。
- `herdr-runtime-session`：`HerdrRuntime` 對 HERDR 的連線順序（Probe → seed snapshot → L →
  S）、ReopenStatus 重疊重開、WSL 探測與 UTF-16LE 解碼、protocol 版本警告（設計文件 §4.2、
  §7.1、ADR-0002）。
- `cockpit-config`：TOML 設定檔載入、`--config`、零設定模式、每筆 runtime 的
  `socket`／`wsl`／`command` 三選一與 `id` 唯一檢查、`polling` 預設值（設計文件 §8.2）。
- `cockpit-dashboard`：HTTP 路由、WebSocket 推送整張圖、只綁 loopback、內嵌靜態資源、PWA
  manifest、畫面呈現規則與斷線重連（設計文件 §8.1、§8.3、ADR-0004、ADR-0005）。

### Modified Capabilities

無。1a 的 `herdr-transport`、`herdr-request`、`herdr-event-subscription`、
`herdr-observer-types` 需求不變；`FakeHerdr` 屬 `test-support` 測試鷹架（1a design D7），
不在主規格內，擴充其 builder 不構成需求變更。實作中若發現 `herdr-client` 行為必須改，走
`/opsx:update` 補 delta spec，不在本 change 默默改。

## 非目標

| 非目標 | 對應設計文件章節 |
|---|---|
| Project／Pipeline／Stage／Workstream／Task 與 `RuntimeBinding`、Factory Floor | §1 表（change 2）、§12、`CONTEXT.md` Domain 層 |
| `pane.read` 與 Live Output 面板 | §2.6、§12（change 3） |
| 對 HERDR 送出 `session.snapshot`、`events.subscribe` 以外的任何 method | ADR-0001、§9 |
| 建模或顯示 `layouts`、worktree 細節、`tokens` 用量 | §6.1 註、§12、`CONTEXT.md` |
| 對畫面做增量（JSON patch）推送 | ADR-0004 Alternatives |
| 前端框架、Tauri、service worker、自動安裝提示 | §3 直接定案、ADR-0005、§8.3、§15 |
| 畫面上任何可點的互動；綁定 loopback 以外位址；任何認證 | §8.1、§8.3 |
| 事件歷史持久化（只留記憶體 50 筆） | §13 第 8 項 |
| 由 `AgentStatus` 推論任務完成（`Done` 不是 Completed） | §2.4、§9、`CONTEXT.md` 禁用與改稱 |
| 自寫 Linux relay（ADR-0002 方案 B）、更改 `herdr-client` 公開 API | ADR-0002 Alternatives、`herdr-client/README.md` |
| 重驗 1a 的 spike 1–3、5；spike 5(a) 的 `wsl.exe --shutdown` 重驗 | `docs/handover.md` §3、§5（使用者已決不做） |
| 深究 L 訂閱補推舊事件的機制（只在真機驗收確認 Windows 端是否也有，§4.2 的丟棄規則已可容忍） | `docs/handover.md` §3 |
| 以 `herdr server stop` 製造 Windows 端斷線 | §10.1 真機測試禁令、`AGENTS.md` |
| 定期 snapshot 以外的 cwd 變化偵測 | §2.3（HERDR 沒有 cwd 事件） |

## Impact

- **新檔案**：`cockpit-core/`、`cockpit-herdr/`、`cockpit/`（各含 `src/`、`tests/`；
  `cockpit/assets/` 放內嵌的靜態網頁與 PWA 圖示；`cockpit/README.md` 寫啟動、設定與真機驗收
  方式）、根 `Cargo.toml` members、`docs/research/<執行日期>/change-1b-acceptance.md`
  （Scenario A／B／F 與兩項 1a 遺留查證的證據）。
- **新依賴**（版本實作時查 crates.io 當時穩定版，不在此鎖定）：`axum`（開 `ws` feature）、
  `toml`、`tracing-subscriber`、`chrono`（`generated_at`、`since`、`updated_at` 等欄位要 RFC
  3339，std 沒有格式化能力），bin 另加 `anyhow`；`tokio`、`serde`、`serde_json`、`thiserror`、
  `tracing`、`async-trait` 沿用 1a 已在 `Cargo.lock` 的版本。dev：`tower` 與 `http-body-util`
  （對 axum `Router` 直接 `oneshot` 測 HTTP 路由，不開 port）、`tokio-tungstenite`（WebSocket
  客戶端測試）、`herdr-client` 開 `test-support`。刻意不引 `futures`：事件流以
  `tokio::sync::mpsc` 承載（見 design）。依賴圖照 ADR-0003：`cockpit` 只依賴 `cockpit-core` 與
  `cockpit-herdr`，不直接依賴 `herdr-client`。
- **真機影響**：全部唯讀，只送 `session.snapshot` 與 `events.subscribe`（設計文件 §2、
  `AGENTS.md`）。驗收前要用 `docs/handover.md` §1 的 `setsid -f` 指令重啟 WSL 端測試
  server；Scenario F 的斷線重連只在 WSL 端做，Windows 端斷線只用假 HERDR 驗（§10.1）。
- **對後續 change 的影響**：change 2 在 `cockpit-core` 加 Domain 型別與投影、在 `cockpit`
  加 Factory Floor 畫面；change 3 在 `AgentRuntime` 加 `read_output`。`RuntimeId`、
  `ProjectedState` JSON 頂層形狀、`channel.js` 的 `onState(state)` 介面在本 change 定下後
  由後續 change 沿用。
- **證據落差**：HERDR 行為論斷依設計文件 §2（查證日期 2026-09-13）與
  `docs/research/2026-09-13/change-1a-spikes.md`；其中「L 訂閱剛建立時補推舊事件」只在 WSL
  0.8.2 觀察過一次、未在 Windows 0.9.0 驗過，本 change 的真機驗收 task 負責定案並回寫設計
  文件 §2.3。
