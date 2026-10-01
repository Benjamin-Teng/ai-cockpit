# proposal：progress-model

## Why

進度目前只能靠人在畫面上按按鈕推進，agent 做到哪裡 Cockpit 不知道；活動狀態又只到 workstream 層級——綁定的 pane
在 `working` 時，同一 workstream 所有未標記的 task 一起亮 `running`，看不出 agent 實際在做哪一個。推錯了也沒有「退回」
可以修正。使用者 2026-09-26 定案：讓 agent 呼叫 Cockpit 的 API 回報，寫入 Cockpit 自己的狀態檔，不推翻對 HERDR 唯讀
（ADR-0001），「HERDR 的 `done` 不是 task 完成」保留。

## What Changes

- 新增進度操作「退回」（`retreat`）：只有人能從畫面操作；規則與推進對稱——標記必須為 `none`，第一站不能再退。
- 每條 workstream 新增「目前 task」：agent 宣告「我在做 task X」後記下；task 被標 Completed／Failed、或該 workstream 的
  覆蓋被設定／取消／因失效移除時自動清除；宣告別的 task 直接換掉。
- **BREAKING（顯示行為）**：StageStatus 推導改以 task 為單位——綁定 pane 為 `working`／`blocked` 時，只有該 workstream
  的「目前 task」呈 `running`／`blocked`，其餘 task 呈 `ready`。沒有目前 task 時全部 `ready`，workstream 投影另帶
  「agent 工作中但未宣告 task」旗標，畫面在列首顯示提示。
- 新增 agent 用的 HTTP 端點（`/api/agent/...`）：agent 以 header 帶自己的 `HERDR_PANE_ID` 表明身分，可以查詢自己綁定的
  task、宣告目前 task、推進 task。只能動「綁定到這個 pane 的 workstream」底下的 task。只認非 WSL runtime 的 pane。
- 狀態檔升為 `version: 2`，多一個每 project 的 `active` 欄位；`version: 1` 舊檔照常讀取。
- `cockpit/README.md` 加一節給 agent 的使用說明（PowerShell 與 bash 的 curl 範例），可貼進各專案的 `AGENTS.md`。

## 非目標

- WSL 內的 agent 回報：WSL2 預設 NAT 網路下 WSL 的 `127.0.0.1` 連不到 Windows 端 Cockpit，而 Cockpit 只聽 loopback
  （設計文件 §8.1 的寫入面安全前提）。延後，見 design「Risks」。
- agent 標 Completed／Failed、清除標記、退回：留給人（使用者 2026-10-01 選定）。
- 身分驗證（token）：本機單人情境，pane 身分只防誤操作、不防惡意本機程式（與設計文件 §8.1 現行信任模型一致）。
- Cockpit 定時 prompt agent 回報：`docs/cockpit-spec.md` 明文排除（LLM polling 不當主要 telemetry）；本 change 只提供
  agent 主動呼叫的入口。
- MCP server、`cockpit` 命令列子指令：先用 curl，日後需要再包。
- 寫入 HERDR（metadata、report_agent 等）：對 HERDR 維持完全唯讀（設計文件 §12、ADR-0001）。

## Capabilities

### New Capabilities

- `agent-reporting`: agent 以 pane 身分呼叫的 HTTP 端點——查詢自己綁定的 task、宣告目前 task、推進 task，以及身分判定與錯誤回應。

### Modified Capabilities

- `pipeline-domain`: 新增「退回」操作與 `retreat` 拒絕原因；新增「目前 task」requirement；StageStatus 推導改以目前 task 為準。
- `pipeline-progress`: 進度寫入端點接受 `retreat`；狀態檔格式升 v2（`active`）；載入容錯涵蓋 v1 與 `active` 的無效項目。
- `state-projection`: Project 投影的 workstream 多 `active_task` 與 `activity_undeclared` 兩欄。
- `cockpit-dashboard`: 畫面操作多「退回」；Factory Floor 列首顯示「未宣告 task」提示。

## Impact

- `cockpit-core`：`domain/progress.rs`（`ProgressOp::Retreat`、目前 task 的操作）、`domain/rejection.rs`、`domain/state.rs`
  （每 project 的 active 對應）、`domain/status.rs`（`derive_status` 多一個「是否為目前 task」輸入）、`projection.rs`。
- `cockpit`：`progress.rs`（狀態檔 v2）、`progress_service.rs`（新交易：宣告、agent 推進、覆蓋變更時清 active）、
  `http.rs`（`retreat`、`/api/agent/...` 路由與 pane 身分判定）、`cockpit/assets/app/`（退回按鈕、未宣告提示）、
  `cockpit/examples/ui_preview.rs`（情境）、`cockpit/README.md`。
- 測試：既有 `cockpit/tests/pipeline_api.rs`、`progress_file.rs`、`progress_service.rs` 擴充；驗收腳本中依賴「整條 workstream
  一起 running」的斷言需依新 spec 調整。
- 回退：舊版 Cockpit 讀到 `version: 2` 的狀態檔會啟動失敗（見 design「Migration Plan」）。
- 無新外部依賴。
