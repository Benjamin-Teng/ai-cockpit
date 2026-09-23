# tasks：live-output

> 執行路徑：SDD｜理由：25 個 task 橫跨 4 個 crate 加前端與真機驗收；含安全邊界（pane 內容的來源檢查）、並發（同 runtime 讀取排隊）與對外端點；前端 task 逐層疊在輪詢核心之上，錯了返工貴。

設計前 spike 已完成並 commit（`docs/research/2026-09-19/pane-read-probe.md`、
`herdr-client/examples/probe_pane_read.rs`），不再列為 task。每個 task 結尾都要跑所改 crate 的
`cargo fmt --check`、`cargo clippy -p <crate> --all-targets -- -D warnings`、`cargo test -p <crate>`，
改 `.md` 要跑 `markdownlint-cli2 "**/*.md"`（repo 根）；輸出貼進回報。平行派工時各自只 `git add` 自己的檔，
`tasks.md` 由控制端統一勾。程式碼註解若要引用 task，寫成 `live-output task N.M`——repo 內既有的
「task 4.1」「task 4.2」註解指的是 change 2，不要混用裸編號。

## 1. 基線與真機合約

- [x] 1.1 記錄基線：在分支 `change-3-live-output` 跑 `cargo test --workspace`，把 passed／failed／ignored 數字寫進 SDD ledger；驗收＝ledger 有這三個數字且 failed 為 0
- [x] 1.2 擷取真機 `pane.read` 回應 fixture：啟動 WSL 測試 server，以 `HERDR_CLIENT_TEST_ALLOW_WSL_WRITES=1` 建立 label 為 `live_output_fixture` 的測試 tab、對其 root pane `pane.send_text` 送 `printf 'alpha\nbeta\n'`，用 `wsl.exe -d <distro> -e nc -U <socket>` 送一筆 `source=recent`、`format=text`、`lines=200` 的 `pane.read` 並把原始回應行存成 `herdr-client/tests/fixtures/pane-read-p<protocol>.json`（比照既有 `snapshot-p20.json` 的命名；`<protocol>` 取同一個 server 的 `session.snapshot` 回報值）（存檔前把 `text` 中的主機名與使用者名稱替換成 `host`／`user`），然後 `tab.close` 並停掉 WSL 測試 server；驗收＝fixture 檔存在、`text` 含 `alpha` 與 `beta`、關閉後以 `probe_pane_read --wsl … --list` 確認測試 pane 已消失
- [x] 1.3 `herdr-client` 合約測試：在 `tests/contract.rs` 加 `parses_real_pane_read_response`，把 1.2 的 fixture（檔名見 1.2）解析為 `PaneReadResultEnvelope`，斷言 `text` 含 `alpha`／`beta`、`source`＝`recent`、`format`＝`text`、`truncated`＝`false`；並在 `PaneReadResult::revision` 的 doc 註解寫明「已測版本恆為 0，呼叫端不得用它判斷內容是否改變」與研究筆記路徑（spec `herdr-observer-types`）；驗收＝`cargo test -p herdr-client parses_real_pane_read_response` 通過

## 2. `cockpit-core`：抽象

- [x] 2.1 在 `cockpit-core/src/runtime.rs` 新增 `OutputFormat`（目前只有 `Text`，serde 序列化為 `"text"`）、`PaneOutput { format, text, truncated }`（不含任何 revision 欄位）、`RuntimeError::PaneNotFound`（帶 pane id；`retry_after()` 對它回 `None`），並從 `lib.rs` 匯出；驗收＝新單元測試 `pane_output_serializes_format_as_text`、`pane_not_found_has_no_retry_after` 通過
- [x] 2.2 `AgentRuntime` 加 `async fn read_output(&self, pane: &PaneId, max_lines: u32) -> Result<PaneOutput, RuntimeError>`（不給預設實作），同一個 task 內補齊所有實作者使 workspace 可編譯：`cockpit-core/tests/common` 的 `FakeRuntime`（可腳本化：指定 pane 回指定文字，其餘回 `PaneNotFound`）與 `HerdrRuntime`（先做成功路徑：`source=recent`、`format=text`、`lines=max_lines`、不送 `strip_ansi`，回應的 `text`／`truncated` 原樣交回）；驗收＝`cargo test --workspace` 全綠，且新測試 `fake_runtime_read_output`（spec `runtime-model`「假 runtime 回應輸出」）與 `read_output_does_not_bump_projection_version`（「讀取輸出不動狀態庫」）通過

## 3. `cockpit-herdr`：以 `pane.read` 實作

- [x] 3.1 參數與回應：新增 `cockpit-herdr/tests/read_output.rs`，用假 HERDR `with_method_response("pane.read", …)` 驗 spec `herdr-runtime-session`「送出的參數」（檢查假 HERDR 收到的 request 行：`pane_id`、`source`、`format`、`lines`，且沒有 `strip_ansi` 鍵）與「回應原樣交回」；驗收＝`cargo test -p cockpit-herdr --test read_output` 這兩個測試通過
- [x] 3.2 錯誤對應：`pane_not_found` → `RuntimeError::PaneNotFound`；其他錯誤碼、端點不存在、回應無法解析 → `RuntimeError::Failed(原因)`，不做 WSL 探測、不重試；驗收＝`read_output.rs` 的 `pane_not_found_maps_to_pane_not_found`、`other_remote_error_is_failed_with_code`、`unreachable_endpoint_is_failed` 通過
- [x] 3.3 同 runtime 不並發：`HerdrRuntime` 加一把只序列化 `read_output` 彼此的 `tokio::sync::Mutex`（不與 `snapshot`／`subscribe` 互斥）。假 HERDR 若還不能「延遲回應」與「回報同時進行中的連線數上限」，在 `herdr-client/src/testing/` 擴充（例如 `MethodResponse` 加延遲變體、`FakeHerdr` 記錄 per-method 最大並發數）；測試用真實時間，不用 `start_paused`；驗收＝`read_output_is_serialized_per_runtime`（4 筆並發、每筆延遲 200 ms、最大並發為 1、全部成功）通過，且 `cargo test -p herdr-client` 仍全綠
- [x] 3.4 method 集合與事件流：更新既有的 method 集合測試（實際位於 `cockpit-herdr/tests/reopen.rs`，非 `session.rs`）對應 spec「假 HERDR 收到的 method 集合」（不呼叫 `read_output` 時仍只有兩個 method）並新增「讀取輸出只多一種 method」；新增「不干擾事件流」測試（事件流建立後讀 10 次輸出，沒有重開訂閱、事件照收）；驗收＝這三個測試通過

## 4. `cockpit`：輸出端點

- [x] 4.1 `AppState` 加 `runtimes: Arc<HashMap<RuntimeId, Arc<dyn AgentRuntime>>>`；`app.rs` 建 runtime 時 clone 一份 `Arc` 進表、原本那份照舊交給 `driver::run`；所有既有建構 `AppState` 的地方（測試、`ui_preview`）補上空表；確認 `shutdown_all` 等收尾路徑不因多一份 `Arc` 而卡住；驗收＝`cargo test -p cockpit` 全綠，且新測試 `app_state_holds_every_configured_runtime` 通過
- [x] 4.2 新增 `GET /api/runtimes/{runtime}/panes/{pane}/output` handler：以常數 200 呼叫 `read_output`，外包 5 秒 `tokio::time::timeout`；依 design D6 對應 200／404／503／504，非 200 本體沿用 `error_response` 的 `{"error": …}`；200 帶 `Cache-Control: no-store`、`X-Content-Type-Options: nosniff`、`application/json`；pane id 含冒號要能解碼；驗收＝`cockpit/tests/` 新檔 `output_endpoint.rs` 以假 runtime 涵蓋 spec `live-output`「輸出讀取端點」七個情境（504 用 `start_paused`；「不認識的 runtime」要斷言假 runtime 的讀取次數為 0）全部通過
- [x] 4.3 以 `route_layer` 把 `source_check` 掛到輸出路由，並更新 `source_check.rs` 檔頭註解（不再是「只套在兩個寫入路由」）；驗收＝`output_endpoint.rs` 涵蓋 spec「輸出端點只接受本機同源請求」三個情境，其中兩個 403 情境要斷言假 runtime 的讀取次數為 0；另加一個 `Host` 重複出現回 403 的測試
- [x] 4.4 路由清單：`/app/output.js` 納入內嵌資源與 content-type 測試（spec `cockpit-dashboard`「路由與 content-type」）；此 task 先放一個只有檔頭註解的 `output.js`；驗收＝既有路由測試更新後通過

## 5. 前端：Live Output 面板

- [x] 5.1 `ui_preview` 加腳本化假 `AgentRuntime`（只有 `read_output` 有內容：預設每秒多一行；環境變數可切換為固定延遲、連續 503 後恢復、404、含 `<script>window.pwned=1</script>` 與 `<b>x</b>` 的文字、超過一屏且 `truncated=true` 的長文），掛上與正式服務相同的輸出 handler，並在 stdout 以 `output-request <runtime> <pane>` 記錄每次請求；驗收＝`cargo run -p cockpit --example ui_preview` 啟動後 `curl -H "Host: 127.0.0.1:<port>" http://127.0.0.1:<port>/api/runtimes/win/panes/<pane>/output` 回 200 且 stdout 有對應記錄行
- [x] 5.2 面板骨架與輪詢核心：`index.html` 在 `#app` 旁加常駐 `<section id="output">`；`output.js` 實作選取後的輪詢（`setTimeout` 串接、同時至多一個進行中請求、世代序號丟棄舊回應、文字相同不重寫、`textContent` 寫入 `<pre>`、貼底才跟捲、`truncated` 提示、「關閉」）；`style.css` 加面板樣式（等寬字體、沿用現有語彙）；驗收＝以 CDP 腳本 `docs/research/2026-09-19/live-output-check.js` 第一段驗 spec `live-output`「輪詢與顯示」的「沒有選取就不請求」「請求不堆積」「內容不被當成 HTML」「截斷提示」（此時選取 UI 尚未接上，腳本直接呼叫 design D8 定義的 `window.liveOutput.select(runtime, paneId)`／`clear()`；`output.js` 不自己持有「誰被選」以外的選取 UI 狀態）
- [x] 5.3 選取互動：`actions.js` 的 `ui` 加 `selected`；pane 列 `data-action="select-pane"`（exited 的不可選）、被選列的標示、workstream 列首「看輸出」（`data-action="select-bound-pane"`，只在 `binding` 為 `bound` 時出現）；`ui.selected` 改變時同步呼叫 `liveOutput.select`／`clear`，面板「關閉」與「pane 已不存在」回呼清掉 `ui.selected`；改綁模式下「綁定到這裡」不得觸發選取；`render.js` 每次重畫後以 `liveOutput.setKnownPanes(...)` 交出投影中的 pane 集合；驗收＝CDP 腳本第二段驗 spec「選定一個 pane」五個情境與 `cockpit-dashboard`「頻繁重畫不影響 Live Output 面板」（`COCKPIT_PREVIEW_PUSH_MS=100`，比對面板 DOM 節點同一性與 `scrollTop`；標示比 `getComputedStyle` 不只比 class），且既有 `actions-check.js`、`factory-floor-check.js` 重跑仍通過
- [x] 5.4 顯示時序與捲動：CDP 腳本第三段驗「內容跟上」（3 秒內）、「舊回應不蓋掉新選取」（假 runtime 對 `p1` 延遲 2 秒）、「往上捲不被拉回」「停在底部會跟著走」；驗收＝腳本該段全數 PASS
- [x] 5.5 失敗與消失：面板的過期標示與原因顯示、404 與「pane 已不在投影」停止輪詢、503／504／請求失敗保留文字並持續重試、恢復後清除標示；驗收＝CDP 腳本第四段驗 spec「失敗與消失的呈現」三個情境（「pane 被關掉」以 preview 的投影切換模擬；停止輪詢以 stdout 的 `output-request` 記錄行在 5 秒內不再增加判定）

## 6. 真機驗收

- [x] 6.1 Scenario E（WSL）：啟動 WSL 測試 server，以 `HERDR_CLIENT_TEST_ALLOW_WSL_WRITES=1` 建測試 tab、送「每秒印一行」的迴圈；啟動 `cockpit`（設定含 `wsl` runtime），用 CDP 腳本 `docs/research/2026-09-19/live-output-real-check.js` 點選該 pane，驗新行在 3 秒內出現在面板（門檻與 spec 相同，不得自訂更嚴）；再關掉該 tab，驗面板顯示「pane 已不存在」並停止請求；關 tab 之後、停 server 之前，另以 `probe_pane_read --wsl … --pane <剛關掉的 pane id> --count 1 --metadata-only` 直接對 WSL 真機讀一次不存在的 pane，確認 stderr 的錯誤碼是 `pane_not_found`（研究筆記第 5 節只驗了 Windows 0.9.0；若 WSL 0.8.2 的錯誤碼不同，停下來回報並修 `cockpit-herdr` 的錯誤對應與 spec，不得略過），並以 `curl` 對 Cockpit 端點確認該 pane 回 404 而不是 503；收尾關 tab、停 WSL 測試 server、確認沒有殘留 `cockpit.exe`；驗收＝腳本輸出全數 PASS，結果寫進 `docs/research/2026-09-19/live-output-acceptance.md`
- [x] 6.2 Windows 端手動驗收（使用者執行）：啟動 `cockpit`，點一個工作中的 agent pane，確認面板內容與 HERDR 畫面一致、每秒更新、往上捲不被拉回、點 workstream「看輸出」能選到綁定的 pane；另以 `curl -H "Host: evil.example:7770" …/output` 確認 403；驗收＝使用者回報通過，結果記入 6.1 的驗收文件

## 7. 文件

- [x] 7.1 ADR-0002：Consequences 補上實測數字（WSL 約 42 ms、Windows 約 1.2 ms、與大小無關）與「每秒一次的 Live Output 不需要方案 B」，連到研究筆記；設計文件 §12「Live Output 讀取模型」與 §13 #4 加註「`revision` 實測恆為 0，change 3 改由前端比對文字」並連到研究筆記與本 change；驗收＝`markdownlint-cli2 "**/*.md"` 0 issues
- [x] 7.2 `CONTEXT.md`「Live Output」詞條補上「由頁面輪詢、純文字、最多最近 200 行」；`README.md` 補輸出端點的用法（含 `curl` 要帶合法 `Host`）與安全說明（pane 內容只給本機同源）；驗收＝markdownlint 0 issues
- [x] 7.3 重寫 `docs/handover.md`（依 `~/.claude/guides/handover-template.md`）：change 3 狀態、「WSL 端每秒多次 `pane.read`」改為已結案、新增本段踩到的坑；驗收＝markdownlint 0 issues，且第 5 節表格不再有該未決列

## 8. 收尾

- [x] 8.1 全 gate：`cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test --workspace && markdownlint-cli2 "**/*.md" && openspec validate --all`；驗收＝全部 0 error，輸出貼進回報，測試數與 1.1 基線對照（只增不減）
- [x] 8.2 Codex adversarial review（`codex-companion.mjs adversarial-review --wait --base main`，focus 字串不放反引號；先確認 log 沒有 `usage limit`／`Turn failed`）；findings 逐條實測重現後才採信，依 `superpowers:receiving-code-review` 處理；驗收＝review 無未處理的成立 finding，處理紀錄寫進 SDD ledger
