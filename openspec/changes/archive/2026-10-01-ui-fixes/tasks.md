# tasks：ui-fixes

> 執行路徑：SDD｜理由：跨 `cockpit-core`、`cockpit-git`、`cockpit` 後端與前端四塊，約 17 個可獨立驗收的 task；投影 JSON 擴充與
> 環境變數清理屬對外格式與執行邊界，前端焦點與斷線呈現牽動多支既有驗收腳本的斷言，錯了返工貴。
>
> **審查**：Codex adversarial-review 依階段批次跑，不逐 task——第 3 節完成後審 core＋後端（3.7），第 4 節完成後審前端（4.9），
> 5.3 審整支分支。Codex 週限額 2026-10-04 10:55 才恢復；審查 task 到點前不得以其他審查取代，除非使用者另行決定。等待期間可
> 繼續做後續 task（不 squash、不併回），恢復後依序補審；審查 findings 若推翻已做的後續 task，照 receiving-code-review 返工並記入 ledger。
> findings 與處理記在 `sdd-ledger.md`。

通則（每個 task 都適用）：

- 結尾跑 `cargo fmt --check`、`cargo clippy -p <本 task 改到的 crate> --all-targets -- -D warnings`、`cargo test -p <同上>`；
  有改 `cockpit/examples/ui_preview.rs` 時加跑 `cargo test -p cockpit --example ui_preview`；有改 `.md` 時在 repo 根跑
  `markdownlint-cli2 "**/*.md"`（核對 `Linting: N files` 不為 0）；有改 `openspec/` 時跑 `openspec validate --all`。輸出貼進回報。
- Rust 的 task 以 `superpowers:test-driven-development` 進行，回報附 red→green 證據；前端 task 的新斷言先在未修的程式上跑出紅，再修。
- 前端資源以內嵌方式編進執行檔：改完 `cockpit/assets/` 一定先 `cargo build -p cockpit --example ui_preview` 再跑任何腳本。
- **「既有腳本」**＝`docs/research/2026-09-15/reconnect-check.js`、`whatever-check.js`、`docs/research/2026-09-16/actions-check.js`、
  `channel-backoff-check.js`、`factory-floor-check.js`、`docs/research/2026-09-19/live-output-check.js`、
  `docs/research/2026-09-23/visual-check.js`、`docs/research/2026-09-27/files-check.js`、`docs/research/2026-09-28/git-check.js`、
  `docs/research/2026-10-01/progress-check.js`。**每個改前端或 `ui_preview` 的 task 結束時全部必須全綠**：只改被本 task 打壞、
  且 spec 已改變的斷言；腳本修改與產品修改分開 commit，commit 訊息逐條列出被改的斷言與對應的 spec scenario；不得放寬 spec 沒有改變的
  斷言。驗收腳本不可並行跑（共用暫存目錄），一律前景跑。
- 本 change 的新前端斷言集中在新腳本 `docs/research/2026-10-01/ui-fixes-check.js`（寫法比照 `progress-check.js`，raw CDP＋
  `Input.dispatchMouseEvent`／`Input.dispatchKeyEvent` 產生真實輸入，附 `ui-fixes-check.md` 用法），由 4.2 建立、後續 task 擴充；
  Git 頁的新斷言可改放 `git-check.js`。
- `factory-floor-check.js` 會覆寫 `docs/research/2026-09-16/task-5.2-scenario-d.png`，跑完 `git checkout --` 還原。
- 跑腳本前確認 7770 沒有人在用；**不是自己開的程序只能回報、不准砍**；跑完依 PID 收尾，確認 7770 與 CDP port 沒有 LISTEN。
- 對 HERDR 只送唯讀請求；Windows 端不得 `herdr server stop`（`AGENTS.md`）。
- 程式碼註解若要引用 task，寫成 `ui-fixes task N.M`。`tasks.md` 由控制端統一勾。
- 多個 subagent 不同時改同一個工作樹；commit 只 `git add <具體路徑>`。

## 1. 基線

- [x] 1.1 記錄基線：在分支 `feat/ui-fixes` 跑 `cargo test --workspace`、`cargo test -p cockpit --example ui_preview`、既有腳本，把
  passed／failed／ignored 數字與各腳本結果寫進 `openspec/changes/ui-fixes/sdd-ledger.md`；驗收＝ledger 有這些數字，failed 為 0、腳本全綠
  （既有 flaky `abort_await_is_bounded`、`visual-check.js` DF1 收尾若失敗，重跑一次並記錄）

## 2. `cockpit-core`

- [x] 2.1 斷線綁定帶來源（spec `runtime-binding`「解析結果種類」、`state-projection`「Project 投影」、design D3）：
  `BindingResolution::RuntimeDisconnected` 與 `ProjectedBinding::RuntimeDisconnected` 加必填 `source`；覆蓋斷線為 `override`、
  自動解析斷線為 `auto`；驗收＝`cockpit-core` 測試涵蓋兩份 delta 中斷線相關的全部 scenario；更新受影響的測試
  （`cockpit-core/tests/projection_projects.rs` 精確 JSON、`domain_types.rs`、`domain_binding.rs`、`domain_status.rs`、
  `cockpit/tests/app.rs`）與 fixture `cockpit/tests/fixtures/projected-state.json`（兩筆 `runtime_disconnected` 補 `"source": "auto"`）；
  `cargo test --workspace` 與 `cargo test -p cockpit --example ui_preview` 全綠

## 3. 後端（`cockpit-git`、`cockpit`）

- [x] 3.1 refs 標示 commit、非 commit tag 不當起點（spec `git-review`「refs 端點」「commit 清單端點」、design D4）：
  `cockpit-git/src/query.rs` 的 `REFS_FORMAT` 加型別欄位（連同 `query.rs` 內的 argv 斷言），`cockpit-git/src/refs.rs` 解析
  （欄位數檢查隨之更新）、refs 回應每筆加 `commit`；`cockpit/src/git.rs` 組預設起點時只收 `commit` 為真者；
  驗收＝`cockpit-git` 單元測試涵蓋「tag 指向 tree 仍列在 refs、`oid` 為剝開後物件、`commit` 為 false，其餘 ref 為 true」；
  `cockpit-git/tests/real_git.rs` 或 `cockpit/tests/git_endpoint.rs` 以真實 repo（建立指向 tree 的 tag）驗證 refs 回應與預設 log 成功且
  `tips` 不含該 oid；refs 精確 JSON 斷言隨之更新
- [x] 3.2 git 執行環境（spec `git-review`「git 讀取的安全邊界」、design D5）：`cockpit-git/src/runner.rs` 建立 `Command` 時一律清除 repo
  區域變數、設 `LC_ALL=C`；驗收＝design D5 的三個測試（清單涵蓋 `--local-env-vars` 輸出；`as_std().get_envs()` 斷言移除與
  `LC_ALL=C`；呼叫端行程帶 `GIT_DIR` 指向另一 repo 時查詢結果仍是目標 repo——獨立整合測試檔、建 runtime 前設環境，見 D5）；
  ③先紅後綠；既有 WSL argv 斷言不變
- [x] 3.3 撞號涵蓋 `Bound` 孤兒（spec `agent-reporting`「pane 身分判定」、design D8）：`cockpit/src/agent.rs`；驗收＝`agent.rs` 內單元測試
  以手工組的 `ProjectedState` 涵蓋「孤兒 pane 與另一個 runtime 撞號」（讀取得空集合）；`cockpit/tests/agent_api.rs` 新增「斷線 runtime
  最後已知的 pane 仍參與撞號」（現行已成立，作為回歸）；寫入被拒的 scenario 若無法在整合層造出孤兒，以單元測試證明判定結果為空、
  並在回報說明寫入路徑共用同一判定；既有撞號測試仍綠
- [x] 3.4 v1 `active` 判別（spec `pipeline-progress`「狀態檔載入與容錯」、design D9）：`cockpit/src/progress.rs`（型別與檢查）、
  `cockpit/src/progress_service.rs`（寫出處）；驗收＝測試涵蓋 v1 帶
  `"active": null`（先紅後綠）與 `"active": {}`、v2 帶 `"active": null`（兩者為回歸）皆啟動失敗且訊息含路徑；既有 v1／v2 測試仍綠
- [x] 3.5 競態測試改名（design D10）：`cockpit/tests/agent_api.rs` 的 `stale_pane_start_right_after_rebind_is_403` 改名並改文件註解，
  如實描述它驗的範圍，並點名 `cockpit/tests/progress_service.rs` 的 `declare_active_rejects_when_binding_basis_no_longer_holds`、
  `agent_advance_rejects_when_binding_basis_no_longer_holds`；驗收＝`cargo test -p cockpit` 全綠，grep 證明這兩個測試名稱存在
- [x] 3.6 ledger 記錄後端完成狀態：`cargo test --workspace` 數字寫進 ledger；驗收＝ledger 有數字、failed 為 0
- [x] 3.7 Codex 審查（core＋後端）：2026-10-04 10:55 後跑 `codex-companion.mjs adversarial-review --base main`（前景、`--wait`），
  focus 為第 2、3 節；依 `superpowers:receiving-code-review` 實測驗證後才改；findings 與處理寫進 ledger；確認輸出有最後的
  「# Codex Adversarial Review」結論段

## 4. 前端與預覽

- [x] 4.1 `ui_preview` 情境：新增一條以覆蓋綁到斷線 runtime（`wsl`）的 workstream，使投影出現 `runtime_disconnected`＋`source: override`；
  驗收＝`cargo test -p cockpit --example ui_preview` 全綠、既有腳本全綠（若有斷言因新增的 workstream 而失敗，只能依通則調整）
- [x] 4.2 滑鼠焦點（spec `cockpit-dashboard`「畫面整頁重畫」焦點相關 scenario、design D1）：最後輸入方式（`document` capture）＋共用
  焦點還原 helper（放在 `output.js` 以全域掛出，見 D1），`render.js` 的 `restoreFocus` 與 `output.js` 的 `restoreFocusAfterDeselect` 改經 helper；建立 `ui-fixes-check.js`，
  涵蓋「滑鼠點擊後重畫不呈現焦點外框」（新載入頁面後的**第一次**點擊，pane 列、左欄專案、task 按鈕各一，含其後 1 秒內的背景重畫）、
  「鍵盤觸發的重畫保留焦點外框」「滑鼠點擊之後按 Tab 外框照常出現」；驗收＝新腳本先在未修程式上紅、修後綠，既有腳本全綠
  （`visual-check.js` 的鍵盤焦點斷言不得放寬）
- [x] 4.3 斷線轉暗（spec `cockpit-dashboard`「畫面整頁重畫」通道斷線相關 scenario、design D2）：通道狀態掛 `#app`、`paint()` 寫回；
  `style.css` 一個集中區塊覆寫 design D2 列出的選擇器；runtime 卡「最後已知」用手足節點；`ui-fixes-check.js` 涵蓋 runtime 卡、pane 列、
  task 節點、列首「未宣告 task」提示、左欄 project 計數的計算顏色、按鈕不轉暗、恢復後還原，以及「斷線中整頁重畫後仍為最後已知」；
  控制端做設計審核（1536／1100／700 三寬 viewport 斷線截圖，對照 `docs/direction-01-visual-design.md`）並過 frontend-design 審核；
  驗收＝新腳本與既有腳本全綠、審核 findings 已處理
- [x] 4.4 斷線覆蓋的改綁標示與取消（spec `cockpit-dashboard`「Factory Floor」「畫面操作」覆蓋斷線相關 scenario、design D3）：
  `render.js` 的綁定摘要在 `runtime_disconnected` 且 `source` 為 `override` 時顯示「改綁」徽章；`ui-fixes-check.js` 涵蓋：覆蓋斷線有徽章、
  「取消改綁」可按且送出的請求正確；自動斷線列首沒有「改綁」徽章、也沒有「取消改綁」；停在該狀態整頁重畫後仍正確；
  驗收＝新腳本與既有腳本全綠
- [x] 4.5 Graph 搜尋不拉動捲動（spec `git-review`「Git Graph 分頁」「搜尋命中後背景載入不拉動捲動」、design D6）：`git.js`；
  `git-check.js` 依該 scenario 新增斷言（先 Enter 跳到前段命中、捲到底觸發下一批、載入後 `scrollTop` 不變、i 不變），並保留「按 Enter
  仍會捲到下一筆命中」；驗收＝新斷言先紅後綠、既有腳本全綠
- [x] 4.6 非 commit tag 不誤報分支變更（spec `git-review`「有非 commit tag 時不誤報分支變更」、design D4）：`git.js` 的
  `computeExpectedTips` 只收 `commit` 為真的 ref；`git-check.js` 的 fixture repo 加一個指向 tree 的 tag，斷言 5 秒內不出現「分支已變更」，
  既有「分支變更提示」斷言仍綠；驗收＝新斷言先紅後綠（依賴 3.1）
- [x] 4.7 檔案清單截斷提示（spec `git-review`「commit 詳情與比較」、design D7）：詳情與比較清單讀 `truncated` 並顯示既有提示；
  `ui_preview` 或 `git-check.js` 的 fixture 需能產生截斷（必要時以小上限的測試 repo），斷言提示出現與否；驗收＝既有腳本全綠
- [x] 4.8 詳情重建焦點保留（spec `git-review`「commit 詳情與比較」焦點 scenario、design D7）：新增重建前記焦點身分、重建後經 4.2 的
  helper 還原；`git-check.js` 新增鍵盤路徑（焦點回到對應元素且外框可見）與滑鼠路徑（焦點回到對應元素、無外框）；驗收＝既有腳本全綠
- [x] 4.9 Codex 審查（前端）：同 3.7，focus 為 `cockpit/assets/`、`ui_preview.rs` 與腳本

## 5. 真機驗收與收尾

- [x] 5.1 真機驗收：以臨時設定檔（放在 repo 外）啟動 Cockpit，接 Windows 端 HERDR；在 Chrome（headless 以外的實機觀察可由使用者確認）
  確認：滑鼠點 pane 列不殘留外框、Tab 操作有外框；**停止 Cockpit 行程本身**（不是 HERDR）造成通道斷線，右欄與中欄轉暗並標「最後已知」，
  重啟 Cockpit 後還原；Git 頁在本 repo 搜尋後往下捲不被拉回。結果與截圖路徑寫進 `docs/research/2026-10-01/ui-fixes-live.md`；
  驗收＝該檔存在、markdownlint 0 issues
- [x] 5.2 全 gate：`cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test --workspace && cargo test -p cockpit --example ui_preview && markdownlint-cli2 "**/*.md" && openspec validate --all`
  與全部腳本（既有＋`ui-fixes-check.js`）；驗收＝輸出貼進 ledger，全綠
- [x] 5.3 Codex 審查（整支分支）：`adversarial-review --base main`，處理方式同 3.7；驗收＝ledger 記錄結論段與每個 finding 的處理
- [x] 5.4 收尾：squash 併回 `main`、archive change、重寫 `docs/handover.md`（依 `~/.claude/guides/handover-template.md`）；
  驗收＝`openspec validate --all` 通過、`git status` 乾淨
