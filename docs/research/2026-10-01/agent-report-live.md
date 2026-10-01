# agent 回報進度真機驗收（progress-model task 5.2）

> 日期：2026-10-01　|　對象：Windows 端 HERDR 真實 pane、真的 `target/debug/cockpit.exe`。
> 性質：一次性真機驗收紀錄；環境事實與觀察值不是規格。對 HERDR 全程唯讀，未執行 `herdr server stop`。

## 結論

README「agent 回報進度」一節的範例照抄可跑（PowerShell `curl.exe` 與 bash 各跑過 `start`；其餘用 bash），
所有狀態碼與 spec `agent-reporting` 一致：`GET` 列出綁定 task、`start` 與 `advance` 204、錯 pane 403
`pane_not_bound`、不帶標頭 400 `missing_pane_id`、動別人 workstream 的 task 403、`complete` 404、最後一站
`advance` 409、跨站 Origin 403 `forbidden_source`。狀態檔為 `version: 2` 含 `active`，重啟 cockpit 後目前 task
保留。README 不需修改。

**未能驗證的一項**：本 pane 的 `agent_status` 整段驗收期間恆為 `idle`（`herdr pane get` 連續取樣亦同），
所以「目前 task 呈 `running`」與「未宣告時 `activity_undeclared` 為 `true`」這兩個只在 working 時才出現的畫面，
真機上沒能出現；`/api/state` 如實記錄的是 `idle` 對應 `ready`、`activity_undeclared` 恆 `false`。這兩項由
單元與整合測試（`pipeline_api`、`agent_api`）與 `progress-check.js`（`ui_preview`）覆蓋，沒有偽造真機值。

## 環境

- HERDR：`herdr 0.9.2-preview.2026-09-29-8e78f929d8f0`（client）；running server `0.9.0-preview.2026-09-08`，
  private protocol 22，相容。
- 本 shell 所在 pane：`HERDR_PANE_ID=wW:p1`、`HERDR_WORKSPACE_ID=wW`；`herdr workspace list` 顯示 `wW`
  的 label 為 `ai-cockpit`；`herdr pane list` 顯示該 pane `agent` 為 `claude`、`cwd` 為
  `D:\projects\ai-cockpit`。另一個 workspace `wX`（label `repo-a`）的 pane `wX:p1` 也是 claude，用來當「別人」。
- cockpit：分支 `feat/progress-model`，`cargo build -p cockpit` 後直接執行 `target/debug/cockpit.exe`，
  監聽 `127.0.0.1:7791`（驗收前確認無人使用）。

## 臨時設定檔（放在 `%TEMP%\cockpit-progress-live\`，驗收後刪除）

```toml
[server]
listen = "127.0.0.1:7791"

[[runtime]]
id = "win"
kind = "herdr"

[[project]]
id = "live"
name = "Live"
stages = ["Plan", "Build", "Review"]

[[project.workstream]]
id = "mine"
binding = { runtime = "win", workspace = "ai-cockpit", agent = "claude" }

[[project.workstream]]
id = "other"
binding = { runtime = "win", workspace = "repo-a", agent = "claude" }

[[project.task]]
id = "t1"
title = "第一個"
workstream = "mine"
stage = "Plan"

[[project.task]]
id = "t2"
title = "第二個"
workstream = "mine"
stage = "Plan"

[[project.task]]
id = "o1"
title = "別人的"
workstream = "other"
stage = "Plan"

[state]
path = "state.json"
```

Windows 使用者目錄路徑在日誌中以 `<user>` 代替。啟動日誌：runtime `win` 為預設 named-pipe endpoint
（`C:\Users\<user>\AppData\Roaming\herdr\herdr.sock`），dashboard 啟動成功。

## 步驟與實際輸出

`/api/state` 摘要欄位格式：`WS <id> active=… undeclared=… <binding.state> <pane_id> <agent_status> <source>`、
`T <id> <stage> <mark> <status>`。

1. 初始狀態：`mine` 綁到 `wW:p1`（auto）、`other` 綁到 `wX:p1`，兩者 `idle`、`active=None`、
   `undeclared=False`；`t1`、`t2`、`o1` 皆 `Plan none ready`。
2. `GET /api/agent/tasks`（`X-Herdr-Pane-Id: wW:p1`）：200，`workstreams` 只有 `mine`，`active_task` 為
   `null`，`t1`／`t2` 的 `next_stage` 皆 `Build`；沒有 `other`。
3. PowerShell `curl.exe -i -X POST .../projects/live/tasks/t2/start -H "X-Herdr-Pane-Id: $env:HERDR_PANE_ID"`：
   `HTTP/1.1 204 No Content`。`state.json` 立即為 `"version": 2`，`projects.live.active` 為
   `{"mine": "t2"}`；`/api/state`：`mine active=t2`，三個 task 仍 `ready`（pane 為 `idle`）。
4. bash `.../t2/advance`：204；`t2` 變 `Build none ready`，`mine active=t2` 不變。
5. 錯 pane id（`X-Herdr-Pane-Id: wZ:p9`）`start t1`：403
   `{"error":"task t1 所屬的 workstream 沒有綁定到 pane wZ:p9","code":"pane_not_bound"}`。
6. 不帶標頭 `start`：400 `{"error":"缺少 X-Herdr-Pane-Id 標頭（值為 pane 內的 HERDR_PANE_ID）","code":"missing_pane_id"}`；
   `GET` 不帶標頭同為 400。
7. 用 `wW:p1` 對 `other` 的 `o1` 打 `start`：403 `pane_not_bound`（`o1 所屬的 workstream 沒有綁定到 pane wW:p1`），
   `other` 的 `active` 仍為 `None`。
8. `.../t2/complete`：404 `{"error":"agent 端點只提供 start、advance：complete"}`。
9. `t2` 再 `advance`（Build 到 Review）：204；再一次：409 `{"error":"已是最後一個 Stage"}`，`t2` 停在 `Review`。
10. `Origin: https://evil.example` 的 `start`：403 `{"error":"Origin 與 Host 不符","code":"forbidden_source"}`。
11. `start t1`：204，`mine active=t1`（取代 `t2`）。
12. 依 Windows PID 停掉 cockpit 再用同一份設定重啟：`mine active=t1` 保留，`t2` 仍在 `Review`；
    `state.json` 為 `version 2`，`active` 為 `{'mine': 't1'}`。

## 與 spec 的對照

- 只有目前 task 呈 `running`／`blocked`：真機上 agent 為 `idle`，所有 task 皆 `ready`，與 spec（其他情況皆
  `ready`）一致，但 working 時的 `running` 沒有在真機上看到（見結論）。
- 宣告前 `activity_undeclared`：pane 非 working，值為 `False`，符合「只在 working 或 blocked 且無目前 task 才為 true」。
- 目前 task 重啟保留、狀態檔 v2 含 `active`：符合。

## 收尾

依 Windows PID（`netstat` 取 LISTENING 的 PID）停掉自己啟動的兩次 cockpit，`:7791` 無 LISTEN 殘留；臨時目錄已刪；
`git status` 乾淨（驗收前後 repo 無其他變動）。
