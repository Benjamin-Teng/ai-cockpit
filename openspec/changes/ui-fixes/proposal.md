# proposal：ui-fixes

## Why

前幾個 change 累積了一批「不會報錯、但畫面說謊或操作卡住」的瑕疵：服務斷線時右欄 runtime 卡與中欄狀態色仍像即時資料、
斷線期間找不到「取消改綁」、滑鼠點過按鈕後冰青焦點框一直殘留、Git 搜尋後往下捲會被背景載入拉回命中列。另有幾個已查明、
每項只需幾行的邊界洞（tag 指向非 commit 讓前端「分支已變更」偵測與後端起點規則不一致、繼承的 `GIT_DIR` 讓 git 讀錯 repo、
撞號判定漏數孤兒 pane 等）。
路線上這是去識別化與推 remote 前的最後一輪修補（`docs/handover.md` 第 4、5 節），使用者 2026-10-01 選定範圍。

## What Changes

- **斷線時畫面不再顯示成即時**：服務（瀏覽器與 Cockpit 之間的通道）不在 connected 時，右欄 runtime 卡、中欄 agent 狀態色、
  pane 列一律轉暗；runtime 卡的連線文字標「最後已知」。恢復連線即還原。
- **斷線期間可取消改綁**：Project 投影的 `runtime_disconnected` 綁定狀態多帶 `source`（`auto`／`override`）；畫面在覆蓋造成的
  斷線狀態下照常顯示「改綁」徽章與「取消改綁」。
- **滑鼠操作不殘留焦點框**：由滑鼠觸發的重畫，還原焦點時不呈現焦點外框；鍵盤觸發的維持現狀（外框保留）。
- **Git 頁**：
  - 搜尋後背景分批載入不再移動捲動位置。
  - 剝開後不是 commit 的 tag 不當 Graph 起點，也不讓「分支已變更」誤報（該 tag 仍列在 refs，refs 每筆標示是否為 commit）。
  - 執行 git 前清除會改變 repo 位置的繼承環境變數，本機 git 也設 `LC_ALL=C`。
  - commit 詳情與比較的檔案清單被截斷時顯示提示。
  - commit 詳情重建後焦點回到原元素，不掉到頁面最外層。
- **agent 回報**：
  - 撞號判定把「有 workstream 綁到該 pane id」的 runtime 算進去（孤兒 pane 不再漏判）。
  - `version: 1` 狀態檔出現 `"active": null` 視為損毀。
  - 改寫說明力不足的競態測試。

## 非目標

- **Live Output 上色**：另開 change 8。`format=ansi` 的控制序列只在 WSL 0.8.2 看過三種 SGR，Windows 0.9.0 未驗（設計文件 §2.6、
  `docs/research/2026-09-19/pane-read-probe.md`），混在修補包會拖住其他小修。
- **Graph 起點過多超過命令列上限**：需重新設計起點傳遞方式與分批前綴穩定性（archive `2026-10-01-git-review/design.md` D5、D8），延後。
- **refs 超過 1 MiB 的截斷與提示**、git 逾時只殺轉手程式（未證實）、其餘 5a／5b 延後清單：延後。
- **WSL 內的 agent 回報**與**以自訂 `command` 端點橋接 WSL 的 runtime 被當成 Windows 端**：前者牽涉只聽 loopback 的安全前提
  （設計文件 §8.1），後者牽涉設定檔的 runtime 描述（設計文件 §8.2），都需另行設計。
- **v2 狀態檔未知 project 缺 `active` 啟動失敗**：change 6 已裁決不修（archive `2026-10-01-progress-model/sdd-ledger.md`）。
- 新功能：本 change 只修正既有行為，不新增操作或端點。

## Capabilities

### New Capabilities

（無）

### Modified Capabilities

- `cockpit-dashboard`: 服務通道斷線時右欄與中欄的「最後已知」呈現；滑鼠觸發的焦點還原不呈現焦點外框；覆蓋造成的斷線狀態可取消改綁。
- `state-projection`: Project 投影 `runtime_disconnected` 綁定狀態多帶 `source`。
- `runtime-binding`: 綁定解析結果 `RuntimeDisconnected` 帶出來源（自動或覆蓋）。
- `git-review`: 非 commit tag 不當 Graph 起點；git 執行環境清理；搜尋分批載入不移動捲動；檔案清單截斷提示；詳情重建焦點保留。
- `agent-reporting`: 撞號判定涵蓋綁定到孤兒 pane 的 runtime。
- `pipeline-progress`: `version: 1` 狀態檔的 `"active": null` 視為損毀。

## Impact

- `cockpit-core`：`domain/binding.rs`（`BindingResolution::RuntimeDisconnected` 加 `source`）、`projection.rs`（`ProjectedBinding`）。
- `cockpit-git`：`query.rs`、`refs.rs`（物件型別）、`runner.rs`（環境清理與 `LC_ALL=C`）。
- `cockpit`：`agent.rs`（撞號）、`progress.rs`、`progress_service.rs`（v1 `active` 判別）、`git.rs`（Graph 起點）、
  `cockpit/assets/app/`（`render.js`、`output.js`、`git.js`、`style.css`）、`cockpit/examples/ui_preview.rs`（斷線覆蓋情境）。
- 投影 JSON 的 `runtime_disconnected` 多 `source`、refs 回應每筆多 `commit`（皆為相容擴充，前端是唯一消費者）；
  `cockpit/tests/fixtures/projected-state.json` 隨之更新。
- 測試：`cockpit-core/tests` 的綁定與投影測試（含精確 JSON 斷言）、`cockpit/tests/agent_api.rs`、`progress_file.rs`、
  `cockpit-git` 測試；驗收腳本 `visual-check.js`（通道斷線、焦點段）、`ui-fixes-check.js`（焦點、斷線轉暗、斷線覆蓋按鈕）、`git-check.js`（搜尋段）。
- 無新外部依賴。
