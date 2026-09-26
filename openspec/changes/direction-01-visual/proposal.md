# direction-01-visual（change 4）

## Why

MVP §20 的 13 個項目已全數完成，但畫面仍是 change 1b 的樣式語彙：Task 節點整塊亮色、所有區塊由上往下堆疊、
Live Output 是貼在視窗底部的條狀面板、`running` 有持續脈動，與 2026-09-15 選定的 Direction 01「航太指揮席」
方向（`docs/direction-01-visual-design.md`）差距很大，也不符合該文件對對比、減少動態與窄視窗的要求。
接下來的 change（.md 瀏覽、Live Output 上色）都要建立在同一套色彩與版面上，所以先把視覺系統定下來。

**依據的分工（使用者 2026-09-21 裁決，原樣記錄）：** 實作視覺改版時，**視覺設計完全參照
`docs/direction-01-visual-design.md`**（配色、字體、關鍵語彙、元件語彙）；概念圖
`docs/cockpit-dashboard-direction-01-concept.png` **只用來標示功能區的位置與排列**，不作為配色、字體、間距或
元件外觀的依據——兩者有出入時以文件為準，不從點陣圖取色或臨摹像素。概念圖上有、但產品還沒有的功能區
（Add Agent、篩選、Flow／Details、Files Changed、Test Results、Token Usage、底部輸入區等）只是排列示意，
不在本 change 實作。

## What Changes

- **三欄版面**（使用者於 brainstorming 選定版面 A）：頂列（產品名稱＋每個 runtime 一個連線燈號）、左欄 Project、
  中上 Factory Floor、中下 Live Output、右欄 runtime 與 pane 清單疊最近事件、底列（version、通道狀態）；視窗寬度至少
  1200 且高度至少 720 CSS px 時固定一屏、不整頁捲動，各面板內部捲動；未達此門檻（含寬 ≥1200 但高 <720）時取消固定
  高度、允許整頁捲動；窄視窗依寬度重排（右欄移到下方 → 單欄可整頁捲動）。
- **Project 切換**：左欄列出所有 Project（名稱、warnings 數、各 status 的 task 數），點選切換中間 Factory Floor
  顯示的 Project，預設第一個。**取代**原本「每個 Project 一塊、上下排列、不做切換選單」的呈現。
- **Direction 01 視覺語彙**：文件色票定為共用色彩 token；Task 節點改為深藍表面＋左側狀態條＋「符號＋文字」狀態
  （使用者選定樣式 A）；pane 的 agent 狀態、runtime 連線狀態同樣以符號＋文字＋色彩呈現；`done` 不用成功綠；
  所有文字對比 ≥ 4.5:1、必要圖形 ≥ 3:1；系統字體、等寬數字；4 px 間距系統；只有 Factory Floor（切角＋帶資訊的刻度）
  與 Live Output（只切角）有外緣裝飾，其餘面板只用框線。
- **動態收斂**：拿掉 `running` 的持續脈動；會被整頁重畫的元素不加任何動畫（否則每次重畫重播）；支援
  `prefers-reduced-motion`。
- **Live Output 面板常駐**：沒有選取時顯示空狀態（提示選一個 pane），「取消選取」改為回到空狀態；面板位於版面內，
  不再覆蓋其他內容。輪詢、丟棄舊回應、純文字、捲動跟隨、過期標示等既有行為不變。
- 既有 headless Chrome 驗收腳本（對 `ui_preview` 的六支與 WSL 真機的 `live-output-real-check.js`）隨新 DOM 與新視覺更新；
  新增一支視覺驗收腳本。
- 順手：`.gitignore` 加 `.superpowers/`；`manifest.webmanifest` 的深色改用新色票；`docs/direction-01-visual-design.md`
  狀態改為已套用。

## 非目標

- **不動後端、HERDR 與對外介面**：投影 JSON、HTTP／WebSocket 路由、寫入端點、HERDR 唯讀約束全部不變
  （設計文件 §6.4、§8.1；ADR-0003）。本 change 不新增前端資源檔（`cockpit-dashboard`「路由與內嵌資源」列出的
  檔案清單不變）。
- **不做 Live Output 上色**（`format=ansi`、SGR 轉換器）與 **M2「斷線期間沒有取消改綁按鈕」**：兩者都要改 Rust
  後端（`OutputFormat` 只有 `Text`、`ProjectedBinding::RuntimeDisconnected` 不帶 `source`），排在之後的 change 6。
- **不做 .md 文件瀏覽**：需要 Project 目錄設定、後端檔案讀取端點與 Markdown 渲染，排在 change 5；本 change
  不預先放置空的「文件」分頁或清單。
- **不實作概念圖上產品沒有的功能區**（見 Why 的裁決）；不改寫 Stage／Workstream 模型，Factory Floor 仍是二維網格
  （設計文件 §8.3「Factory Floor 與互動」）。
- 不做淺色主題、不下載網路字體、不加即時時鐘。
- 不改 `pointerdown` 觸發與焦點還原的機制（設計文件 §8.3 末段；change 3 design）。

## Capabilities

### New Capabilities

（無）

### Modified Capabilities

- `cockpit-dashboard`：「畫面整頁重畫」的版面與狀態呈現改為三欄與 Direction 01 語彙；「Factory Floor」由上下排列
  改為左欄 Project 切換、節點色彩與動態強調改寫；新增「Direction 01 視覺語彙」「版面與窄視窗」「Project 切換」
  三條需求。
- `live-output`：「選定一個 pane」的面板改為常駐（空狀態、取消選取回到空狀態、不再覆蓋頁面內容）。

## Impact

- 前端：`cockpit/assets/app/style.css`（整份重寫）、`render.js`（外框、左欄、頂列／底列、狀態符號）、
  `actions.js`（`select-project` 操作與 UI 狀態）、`output.js`（常駐與空狀態）、`index.html`（外框容器）、
  `manifest.webmanifest`（色碼）。
- 預覽與測試：`ui_preview` 與其 fixture 不需要改（fixture 已有兩個 Project）；`cockpit/tests/http.rs` 中斷言前端原始碼字串的
  測試需逐條核對（例如 `.status-working` class、`render.js` 不得出現「完成」）。
- 驗收腳本：`docs/research/2026-09-15/reconnect-check.js`、`whatever-check.js`、`docs/research/2026-09-16/actions-check.js`、
  `channel-backoff-check.js`、`factory-floor-check.js`、`docs/research/2026-09-19/live-output-check.js`、`live-output-real-check.js`，
  新增一支視覺驗收腳本。
- 文件：`docs/direction-01-visual-design.md`、`cockpit/README.md`（畫面說明若有）、`docs/handover.md`、`.gitignore`。
- 依賴：不新增任何套件。
