# 交接：下一段任務

> **建立日期**：2026-09-14　|　**上一段做完的事**：change 1a `herdr-client` 全部完成、Codex
> review 通過、squash 併回 main（`fd5f5d3`）並 archive（`e543d77`），主規格 4 份已在
> `openspec/specs/`。下一步是 change 1b `attach-herdr-runtimes` 的 propose。
> **性質**：接手用文件，會過期，每段重寫。
> 為什麼做看 `docs/cockpit-spec.md`、怎麼做看 `~/.claude/CLAUDE.md`（本 repo 精簡版在
> `AGENTS.md`）、待辦看 `openspec/changes/herdr-client/tasks.md`。

## 0. 三十秒版本

1. 下一步：`/opsx:propose attach-herdr-runtimes`（change 1b）。propose 前先讀第 2 節的
   前置工作（設計文件 §2.3 表格要補兩個欄位）與第 3 節「1b 要沿用的 1a 事實」。沒有時效性
   任務。
2. 環境注意：Codex review gate 已開（回合內改過 code 就強制 review；Codex 額度用盡時回合會
   卡住，重置時間看錯誤訊息）；WSL 端測試 server 已停，1b 真機測試前用第 1 節指令重啟。

## 1. 現在的狀態

- 已產出（`herdr-client/`，已在 main）：`src/connector/`（`NamedPipeConnector`／
  `UnixSocketConnector`／`ChildStdioConnector`、`ConnectError`）、`src/types/`（observer
  子集型別、`AgentStatus`、事件 payload、`pane.read` 型別）、`src/client/`
  （`Client::request`／`Client::subscribe`、sealed `Request`、`EventStream`、
  `IncomingEvent`、`RequestError`／`StreamError`）、`src/testing/`（feature
  `test-support` 的 `FakeHerdr`）、`examples/`（`spike4_no_window.rs`、
  `spike5_wsl_probe.rs`、`capture_events.rs`）、`tests/`（`transport.rs`、`types.rs`、
  `events.rs`、`contract.rs`、`fake_herdr.rs`、`request.rs`、`subscribe.rs`、
  `child_bridge.rs`、`real_herdr.rs` 真機 `#[ignore]`）。公開 API 與內部機制細節見
  `herdr-client/README.md`，不在這裡重複。
- 可用指令（可直接複製執行）：
  - crate gate：`cargo fmt --check && cargo clippy -p herdr-client --all-targets -- -D warnings && cargo test -p herdr-client`
  - 全 workspace gate：`cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test --workspace && markdownlint-cli2 "**/*.md"`（後者在 repo 根跑）
  - 真機測試（唯讀）：`cargo test -p herdr-client --test real_herdr -- --ignored --test-threads=1`
  - 真機測試（含 WSL 寫入案例）：`HERDR_CLIENT_TEST_ALLOW_WSL_WRITES=1 cargo test -p herdr-client --test real_herdr -- --ignored --test-threads=1`
  - 事件擷取：`cargo run -p herdr-client --example capture_events -- --seconds 60 --out target/capture`
  - WSL 端測試 server（headless，非使用者日常那個）啟動：`wsl.exe -d Ubuntu-24.04 -e bash -lc "setsid -f ~/.local/bin/herdr server >/tmp/herdr-server.log 2>&1 </dev/null"`
  - OpenSpec 進度：`openspec status --change herdr-client`
  - Codex scoped review：`node "C:/Users/<user>/.claude/plugins/cache/openai-codex/codex/1.0.6/scripts/codex-companion.mjs" adversarial-review "<focus 說明>"`（**沒有 `--help`**，見第 4 節）
- 測試與 gate：以 `cargo test --workspace` 當場輸出為準，不要抄本檔舊數字。main 上最後
  一次是 181 passed／0 failed／7 ignored（真機）＋fmt／`clippy -D warnings`／
  markdownlint 皆 0 issue／`openspec validate herdr-client` valid；本輪修正後這些數字
  會變。
- 版控：main 領先 openspec init 的 `a39354b` 兩個 commit：`fd5f5d3`（squash 併入 change 1a
  的 6 個 commit）、`e543d77`（archive 與 4 份主規格）；feature 分支已刪；沒有 remote。SDD
  ledger（每次 review、修正輪次、全部裁決與使用者決定）永久存於
  `openspec/changes/archive/2026-09-14-herdr-client/sdd-ledger.md`；本機 `.superpowers/sdd/`
  已刪。

## 2. 立刻要做：change 1b `attach-herdr-runtimes` 的 propose

前置（小、但要先做，否則 1b 的 spec 會抄到過時的表）：

- [ ] 設計文件 `docs/superpowers/specs/2026-09-13-cockpit-mvp-design.md` §2.3 payload 表補
  `pane_moved` 的四個選填欄位（`created_workspace`、`created_tab`、`closed_workspace_id`、
  `closed_tab_id`）與 `pane_agent_detected` 的 `final_status`；證據：兩份 schema
  （`herdr-client/tests/fixtures/schema-p22.json`、`schema-p20.json`）與 1a 的 spec
  `openspec/specs/herdr-observer-types/spec.md`。改完 `markdownlint-cli2 "**/*.md"` 0 issue。
- [ ] 重讀設計文件 §4.2（連線迴圈六階段、ReopenStatus、WSL 探測）、§6（`RuntimeSnapshot`／
  `RuntimeEvent`／`RuntimeStore`／`ProjectedState`）、§7（HERDR 事件對照）、§8（三個 crate、
  axum、WS、PWA）、§10.2（Scenario A／B／F），確認章節號仍準確。

propose：`/opsx:propose attach-herdr-runtimes`。判讀 checklist：

- [ ] proposal 有「非目標」並指向設計文件章節；HERDR 行為論斷引用 §2 或 `docs/research`
  （`openspec/config.yaml` 的 rules）。
- [ ] design 只寫 1b 特有取捨，並明確引用 1a 的公開 API（`herdr-client/README.md`）與第 3 節
  列的事實（D12 後綴、L 訂閱舊事件、idle／Done 跳動、UTF-16LE、`ConnectionAborted` 的
  reason 字串）。
- [ ] tasks 每個附驗收指令或測試名、結尾跑 crate gate；三個新 crate 依 ADR-0003 依賴方向
  （`cockpit-core` 不得依賴 `herdr-client`）。
- [ ] `openspec validate attach-herdr-runtimes` 無 ERROR（strict 的 SHALL/MUST 警告不算）。
- [ ] propose 產完就停；apply 要另起，而且 **apply 的第一個動作是載入
  `superpowers:subagent-driven-development`**（memory：`apply-phase-must-run-through-sdd-skill`），
  之後逐 task fresh 實作者＋Codex review，不平行派工。

## 3. 接著要做：change 1b apply 與真機驗收

- apply 走 SDD（見第 2 節最後一條）；每個 task 一次 Codex review；review gate 已開，額度用盡
  時改 fresh subagent 對抗式審查並在本檔註明（本段 1a 用過一次）。
- 真機驗收（設計文件 §10.2 Scenario A／B／F）前：用第 1 節指令重啟 WSL 端測試 server；
  spike 4 的目視複驗（HERDR pane 內啟動 cockpit.exe 不出現新視窗）併在 Scenario A 做。
- 1a 留下、由使用者決定不做的事：spike 5(a) 重驗（需 `wsl.exe --shutdown`，會關
  docker-desktop）；若 1b 期間剛好會 shutdown，順手用
  `cargo run -p herdr-client --example spike5_wsl_probe` 補驗並記進 spike 紀錄。

change 1b 要直接沿用、不用重新推導的 change 1a 產出：

- `Connector`／`Client`／observer 型別／`FakeHerdr`（本 crate 公開 API，見
  `herdr-client/README.md`）。
- design D12：`pane.agent_status_changed` 訂閱探測失敗的 error 回應 `id` 帶
  `:sub:<n>:probe` 後綴，client 端已處理成 `Remote`，1b 不用再處理一次。
- **待查證項（spike 3 附帶觀察，未深究）**：WSL 0.8.2 的 L 訂閱剛建立連線瞬間會推送
  數分鐘前已建立並關閉的舊 tab 事件，且有「建立 tab N 才補推 tab N-2 的 closed」的
  一代延遲模式；§4.2「authoritative snapshot 前的事件一律丟棄」可以容忍，但 1b 真機
  驗收要確認 Windows 0.9.0 端是否也有這個行為。
- `report_agent state=idle` 可能呈現 `Idle` 或 `Done`（`AgentStatus::Done` 語意是
  「已 idle 且未被看過」，headless 測試 server 沒有真正 UI「看過」，兩者都可能出現）
  ——1b 若要基於狀態變化判斷，不能假設 idle 一定對應單一值。
- `wsl.exe --list` 系列指令輸出預設 UTF-16LE（含交錯 NUL byte）、exit code 恆
  `0`——1b 的 WSL 探測要用 `from_utf16_lossy` 解碼、看清單是否為空來判斷，不要看
  exit code（同一條坑也記在第 4 節）。
- 設計文件 §2.3 payload 對照表要補兩個欄位：`pane_moved` 的 4 個選填欄位（含
  `closed_tab_id`）、`pane_agent_detected` 的 `final_status`——型別已依 schema 建了，
  只是文件表格沒列，1b propose 時一併補。

## 4. 這一段踩過的坑（不要再推導一次）

（本段新增）

- **`wsl.exe -e bash -lc '… &'` 這種背景寫法啟動的程序，會在那次 `wsl.exe -e` 的起始
  程序結束時被 WSL 一起收掉**（`nohup`＋`&`＋`disown` 都擋不住；實測 `herdr status server`
  下一秒就 not running）：WSL 終止的是該次呼叫的整個 session 程序群組。正解：
  `setsid -f ~/.local/bin/herdr server >/tmp/herdr-server.log 2>&1 </dev/null`，讓它另起
  session 才撐得過。
- **Git Bash 對 `wsl.exe` 傳 `/home/...` 路徑也會做 MSYS 路徑轉換**，不只是先前記錄
  的 `/mnt/c/...` 案例——spike 2 實測：不設 `MSYS_NO_PATHCONV=1` 直接跑會把
  `/home/<user>/...` 轉成 `C:/Program Files/Git/home/<user>/...` 導致找不到檔案；但
  `tokio::process::Command` 不經過 MSYS 這層轉譯，只有在 Git Bash 手動測試指令時才
  需要加這個環境變數。
- **named pipe／unix socket 上「server 不回就關連線」，對 client 端只是乾淨 EOF，不是
  I/O 錯誤**（**不會報錯的錯誤**）：只有明確呼叫 `disconnect()`（Windows）才會讓對端
  看到 `Err`。client 要表達「回應前被中斷」，得自己判斷「EOF 但還沒讀到完整回應」再
  合成錯誤，不能指望底層連線本身會報錯。
- **AF_UNIX 沒有 RST 語意，`Abort`（非正常中斷）在 unix 上會降級成普通 EOF**：
  `FakeHerdr` 測試腳本的 `Step::Abort` 在 unix 平台效果等同 `Step::Close`，測試要
  分平台斷言，不能假設兩個平台行為一致。
- **tokio 的 `ClientOptions::open` 對含冒號的 named pipe 名稱可以直接用**（不需跳脫
  或改名），但 3 條並發連線會遇到 `ERROR_PIPE_BUSY`（231）——50ms 重試、1 秒上限的
  邏輯是必要的，spike 1 實測 3 條並發真的遇到 2 次。
- **WSL 0.8.2 的 `herdr pane report-agent`／`release-agent` CLI，只要給超過一個具名
  flag 就回 `unknown option`**（`--help` 顯示的多 flag 用法與實際可解析的引數不符）：
  不要浪費時間排列組合旗標順序，直接用 `herdr api schema` 查到的底層 JSON-RPC method
  （`pane.report_agent`／`pane.clear_agent_authority`）對 socket 送，效果相同。
- **`codex-companion.mjs adversarial-review --help` 不會印說明，會直接開一個 review
  job**：這個腳本沒有標準 `--help` 出口，任何字串都被當成 focus 參數；本段有一次
  誤觸，只能將錯就錯把那次結果拿來用，別再嘗試用 `--help` 探路。
- **rustdoc 的 `` ```compile_fail,E0xxx `` 附加的錯誤碼不會被校驗**：只驗證「這段
  程式碼確實編譯失敗」，不驗證失敗原因是不是那個碼；碼寫錯也不會被抓出來，只能自己
  實測核對。
- **HERDR 的 `config_dir()` 在 Windows 上也會先看 `XDG_CONFIG_HOME`，空字串也算
  「有設」**（原始碼 `src/config/io.rs:30-68`，commit `bafbc0949`）：先前假設「只有
  unix 吃 XDG」是錯的；完整規則已查證並寫進 `herdr-client/README.md`「本機預設
  socket 路徑」一節，不要重查一次。
- **`String::from_utf8(bytes).is_ok()` 對 UTF-16LE 輸出也會回 `Ok`**：NUL byte 本身
  是合法的單位元組 UTF-8 字元，ASCII 字元交錯 `00` 逐位元組看都合法，只是拼出來的
  字串是不可用內容。判斷「這段輸出是不是 UTF-16LE」不能靠 `from_utf8().is_ok()`，
  要先檢查是否含 NUL byte；`wsl.exe --list` 系列指令的 stdout 正是這個陷阱的真實
  案例（spike 5 實測）。
- **Codex 無法核實「某檔本輪未變」，因為整個 `herdr-client/` 都沒 commit**：review
  只能整份重看，不能只看 diff。這是「使用者尚未授權 commit」的直接代價（見第 5
  節），不是 Codex 本身的限制。

（舊版仍有效的坑，保留）

- **HERDR 沒有全域 agent 狀態訂閱**：`pane.agent_status_changed` 訂閱必填
  `pane_id`，`pane.updated` 只在 agent 名稱改變時發、狀態改變不發；要即時狀態就得每
  pane 各訂一筆，pane 集合變了要重開訂閱連線。spike 3 已在真機重新驗證這個結論成立。
- **pane id 在 `pane.moved` 後會變**：任何以 pane id 當長期鍵的設計都會壞。
- **HERDR Windows 端不是 TCP**：`herdr.sock` 內容 `<pid>:<納秒時間戳>` 看起來像 port
  加 token，實際 pid 就是 server 的 pid；真正的 endpoint 是 named pipe，名稱＝檔案
  完整路徑。
- **每條 API 連線只服務一個 method**：不要設計成一條連線多工。
- **`done` 不是完成**：是「idle 且未被看過」；本段進一步發現這個語意在 headless
  測試環境下會與 `idle` 互相跳動（見第 3 節）。
- **不得用 `herdr server stop` 測 Windows 端斷線**：會殺掉所有 pane。
- **markdownlint-cli2 要在 repo 根目錄跑**：核對輸出的 `Linting: N files` 不是 0。

## 5. 已定但未執行的決策

| 決策 | 狀態 |
|---|---|
| 每個 task 完成後各一個 commit（方便 Codex 用 git 基準做 diff review） | **待使用者決定**（尚未回覆）；目前全部 review 用 working-tree diff 代替 |
| spike 4 目視複驗（HERDR pane 內跑 `spike4_no_window` 與 `--with-window` 對照組） | 待使用者執行 |
| spike 5(a) 重驗需 `wsl.exe --shutdown` | 待使用者決定（會關掉 WSL 端 HERDR 與所有 pane，含測試 server） |
| 開 Codex review gate（`/codex:setup --enable-review-gate`） | 待使用者決定 |
| WSL 端測試 server（本段以 `setsid -f` 啟動、仍在跑）是否 `herdr server stop` | 待使用者決定 |
| f2a「為偵測缺席而建模 Cockpit 用不到的 required 欄位」（task 3.1–3.3 review finding） | **已否決**：違反 §5.2 observer 子集原則，D13 已限縮為「已建模的必填欄位」；代價是 HERDR 若少送 Cockpit 不用的欄位不會被察覺（本來就不影響功能） |
| 2.x／3.x 平行派工，違反 SDD「不可平行」原則 | 已定為一次性例外：檔案完全分離（`src/connector` vs `src/types`），第 4 組起不再平行 |
| （舊）計畫層走 OpenSpec | 已執行（`openspec init --tools claude,codex,agents`） |
| （舊）MVP 三個功能切片，第一片拆 1a／1b 兩個 change | 使用者已核可設計文件與此拆分 |
| （舊）Tauri | 決定 MVP 後再評估，不做 |

## 6. 之後的路

change 1b `attach-herdr-runtimes`（下一步 propose，見第 3 節）之後：change 2
`pipeline-projection`（domain、TOML pipeline 設定、binding、Factory Floor）、change 3
`live-output`（`pane.read` 以 revision 輪詢，供 change 1a 已建的 `PaneReadParams`／
`PaneReadResult` 型別使用；WSL 端可能觸發 ADR-0002 方案 B）。MVP 完成後再評估 Tauri
包裝。北極星與整體範圍見 `docs/cockpit-spec.md`。

## 版本紀錄

| 版本 | 日期 | 變更 |
|---|---|---|
| 1 | 2026-09-13 | 初版：brainstorming 完成、設計文件待審 |
| 2 | 2026-09-13 | 找碴審閱後：每 pane 狀態訂閱、change 1 拆 1a／1b、研究證據進 repo |
| 3 | 2026-09-14 | change 1a `herdr-client` 18 task 完成 17；SDD＋Codex 逐 task review 全程記錄於 `.superpowers/sdd/tasks/progress.md`；5.2 全 workspace gate 已全綠一次，Codex 全分支 review 回 3 Important＋1 low，修正進行中；記錄 spike 1–5 真機結論、D12／D13 兩個 review 觸發的設計修正、假 HERDR／子程序橋接測試機制、待使用者的 5 項決定 |
| 4 | 2026-09-14 | change 1a 全部完成：最終 review 修正波、Codex 額度用盡改替代審查、deferred minor 與待決事項 |
| 5 | 2026-09-14 | change 1a 併回 main 並 archive；下一步改為 1b propose；使用者五項決定的結果（commit 授權、只關 WSL server、spike 4 留 1b、開 review gate、全套收尾） |
