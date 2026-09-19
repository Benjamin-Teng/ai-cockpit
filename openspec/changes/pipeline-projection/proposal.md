# pipeline-projection（change 2）

## Why

Change 1b 讓 Cockpit 看得到每個 HERDR pane 的 agent 狀態，但畫面只回答「有幾個 agent 在跑」，回答不了
「哪條工作線卡在哪個階段」——這是 `docs/cockpit-spec.md` §11 所說 Cockpit 與一般多 terminal 管理器的
核心差異。使用者的 HERDR 用法是 workspace＝專案、tab／pane＝平行工作線，而「現在在哪個階段」只存在於
與 agent 的對話裡，HERDR 看不出來（設計文件 §2.3、§2.4：HERDR 只有五值 `AgentStatus`，沒有任務或階段
概念，`done` 也不是完成），所以 Stage 資訊必須由 Cockpit 自己管理。這是 MVP 第二片（設計文件 §1 表：
spec §20 的 6、7、9 與 12 的 domain 層 Failed／Completed；驗收 Scenario C、D）。

## What Changes

- `cockpit.toml` 新增 `[[project]]`：每個 Project 列出線性的 stages、workstreams（含 binding 穩定特徵）、
  tasks（所屬 workstream、起始 stage、可選 `depends_on`）；另可用 `[state] path` 指定狀態檔位置。結構只
  來自 TOML，畫面不改結構。
- `cockpit-core` 新增 Domain 層（Project、Pipeline、Stage、Workstream、Task、Dependency、RuntimeBinding、
  StageStatus），不引用任何 HERDR 型別。
- **RuntimeBinding 以穩定特徵自動解析**：每條 workstream 以 runtime id＋workspace 標籤，再加可選的
  pane 標籤、cwd 子路徑、agent 種類匹配當下的 pane；pane 換 id 也能重新對上；對不到或對到多個時標示
  「未綁定／歧義」。畫面可臨時指定 pane 覆蓋，覆蓋存狀態檔、該 pane 消失即失效（設計文件 §12「不綁
  pane id」）。
- **StageStatus 推導**：人工標記的 Completed／Failed 優先；有 `depends_on` 的 task 在依賴未 Completed 前
  為 Pending；否則依綁定 agent 為 Running（working）、Blocked（blocked）、Ready（其他）。Runtime 層的
  `done`／`idle` 不推任何 Domain 完成狀態。
- **Cockpit 自己的寫入 API 與狀態檔**：「推進到下一 Stage」「標 Completed」「標 Failed」「清除標記」、
  綁定覆蓋與取消覆蓋，經 HTTP 寫入並持久化到狀態檔（重啟不掉）；寫入端點檢查來源，只接受本機同源
  請求。對 HERDR 仍完全唯讀（ADR-0001 不變）。
- `/api/state` 與 `/ws` 的投影加頂層 `projects[]`，既有欄位不變；Domain 變動同樣走「內容有變才遞增
  version」。
- 畫面新增每個 Project 一塊 Factory Floor 網格（橫軸 Stage、縱軸 Workstream、Task 節點放在目前 Stage 的
  格子），多個 Project 上下分區；**畫面第一次有可點的互動**（進度按鈕、改綁）。
- 帶進 change 1b 的 deferred minor：Drift 重拿期間有事件套用時再拿一次（有上限）、固定重試間隔下限、
  `channel.js` 退避歸零時機與壞訊息防護、`Shutdown.aborts` 只增不減、停止時驅動器逾時的整體上限，以及
  幾項測試缺口逐條補測或裁決。

## Capabilities

### New Capabilities

- `pipeline-config`：`[[project]]` 與 `[state]` 的 TOML 結構、預設值與驗證（錯誤以 `project.<id>.…` 識別）。
- `pipeline-domain`：Project／Stage／Workstream／Task 的進度模型、進度操作的合法轉移、StageStatus 推導規則。
- `runtime-binding`：workstream 綁定以穩定特徵解析成 pane 的規則、解析結果種類、畫面覆蓋的生命週期。
- `pipeline-progress`：進度與綁定覆蓋的 HTTP 寫入端點、來源檢查、狀態檔的讀寫與容錯。

### Modified Capabilities

- `cockpit-config`：設定內容新增 `[[project]]` 與 `[state]` 兩個可識別的區段。
- `state-projection`：投影形狀加頂層 `projects[]`；Domain 變動也觸發投影與 version 遞增。
- `cockpit-dashboard`：路由加寫入端點；畫面新增 Factory Floor 與可點互動（取代「沒有任何可點的互動」）；
  `channel.js` 退避在收到第一則訊息後才歸零、壞訊息不中斷。
- `runtime-driver`：Drift 重拿期間有事件套用時替換後再重拿（上限 2 次）；固定重試間隔下限 1 秒。

## 非目標

- **不寫入 HERDR**：不送 prompt、不呼叫 `pane.report_metadata`，binding 真相只在 Cockpit 設定與狀態檔
  （設計文件 §12、§13 第 9、10 項；ADR-0001）。
- **不從 agent 狀態推論完成**：`done`／`idle` 不變成 Completed（設計文件 §2.4、§9；`CONTEXT.md` 禁用規則）。
- **不畫依賴箭頭、不做 DAG 視覺**：模型保留 `depends_on`，網格只有 Stage×Workstream（`docs/cockpit-spec.md`
  §10 的 DAG 視覺留給之後）。
- **不做 Stage 層級 DAG**：MVP 的 stages 是線性順序（`docs/cockpit-spec.md` §10「第一版視覺可以是線性」；
  `CONTEXT.md` Pipeline「第一版視覺可線性」）。
- **畫面不改結構**：新增／刪除 project、stage、workstream、task 只能改 TOML；也沒有「退回上一站」按鈕
  （`docs/cockpit-spec.md` §20「Pipeline config 可以先由 TOML 手動定義」；設計文件 §8.2 設定檔為結構來源）。
- **一條 workstream 只對一個 runtime 的一個 workspace**：多對多留 Phase 2（設計文件 §13 第 12 項）。
- **不建模 Worktree、Artifact**：worktree 靠 cwd 子路徑涵蓋（設計文件 §13 第 6 項）；Artifact 不在本 change。
- **不做 Live Output、Usage 面板**：Live Output 屬 change 3（設計文件 §1、§12）；Usage 無可靠來源不顯示（§12）。
- **不做歷史**：狀態檔只存目前進度，不存時間軸（設計文件 §13 第 8 項）。

## Impact

- **程式**：`cockpit-core` 新增 domain 模組與投影擴充、`StoreHandle` 接 domain 狀態、`driver.rs` 的 Drift
  重拿與間隔下限；`cockpit` 的 `config.rs`（`[[project]]`／`[state]`）、新的狀態檔讀寫、`http.rs` 寫入
  路由、`app.rs` 組裝與停止逾時、`assets/render.js`／`channel.js`／`style.css`、`examples/ui_preview.rs`
  fixture；`cockpit-herdr` 的 `Shutdown.aborts` 修剪。
- **API**：`/api/state`、`/ws` 加 `projects[]`（相容，只加欄位）；新增 `POST`／`PUT`／`DELETE` 寫入端點。
- **檔案**：新增狀態檔（預設 `cockpit.state.json`，與設定檔同目錄）；`cockpit.example.toml` 加範例；
  `.gitignore` 加狀態檔。
- **文件**：`CONTEXT.md` 的 Dependency、Task、RuntimeBinding 定義依本 change 更新；設計文件 §8.1、§8.2、§8.3、
  §10.2（Scenario C、D）回寫；`cockpit/README.md` 補 pipeline 設定與寫入 API。
- **依賴**：預計不新增 crate（`serde_json`、`toml`、`axum` 已有）；若實作需要，依鐵則先問使用者。
