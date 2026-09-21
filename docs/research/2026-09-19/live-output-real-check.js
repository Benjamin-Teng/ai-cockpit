// Live Output 真機驗收（live-output task 6.1；spec live-output「輪詢與顯示」「失敗與消失的
// 呈現」「輸出讀取端點」；design D8／D9；brief `.superpowers/sdd/tasks/task-6.1-brief.md`）。
//
// 跟 `docs/research/2026-09-19/live-output-check.js`（假 runtime、`ui_preview`）不同：這支腳本
// 對真的 WSL 測試 server（HERDR 0.8.2）、真的 `cockpit`（`target/debug/cockpit.exe`，設定含一筆
// `wsl` runtime）、真的 headless Chrome 端到端跑一次。CDP 手法（`CDP` class、`click` 的等待與
// settle delay、`killTree`／`isPortListening` 收尾判準）沿用 live-output-check.js；WSL 端 JSON-RPC
// 手法（`nc -U`、tab.create／pane.send_text／tab.close 的參數與回應形狀）沿用
// `docs/research/2026-09-15/live-state-check.py`／`acceptance_common.py`（`wsl_rpc`）與
// `docs/research/2026-09-19/pane-read-probe.md` 第 5、7 節的真機事實。
//
// 操作邊界（硬性，task-6.1-brief.md／task-final-fix-brief.md F1／task-final-fix2-brief.md
// finding A／C／task-script-fix-brief.md finding 1／2／3／task-script-fix2-brief.md finding
// A／B（round 2）／task-script-fix3-brief.md（round 3，換路）：只對 WSL 測試 server 做寫入與
// 啟停；絕對不對 Windows 端 HERDR 送任何寫入、`herdr server stop` 或 signal。腳本啟動時第一件事
// 就是檢查 `HERDR_CLIENT_TEST_ALLOW_WSL_WRITES=1`（finding 3：這個檢查搬到選擇要跑 `main()`
// 還是 `--verify-ownership-mismatch` 之前的共用入口 `assertWslWriteOptIn()`，不是各模式各自
// 檢查——沒有這個環境變數，任何模式都直接結束、不對 WSL 送出任何請求，同 `live-state-check.py`
// 的自我把關）。
//
// **換路（round 3）：本次執行使用專屬的 socket 路徑，不再與任何人共用**——round 1／2 連續四輪
// Codex review 都在「腳本與開發者手動啟動的 server 共用同一個預設 socket 路徑」這件事上找到新的
// 競態（啟動前怎麼判斷有沒有別人、收尾會不會停到別人的、殘留 socket 算不算有人、慢啟動的 server
// 被誤判為殘留——最新一個：`wslServerAlreadyRunning()` 先做一次 process 檢查，再做最長 3 秒的
// socket probe，這兩次獨立檢查之間若開發者剛好啟動了 server，窗口內就會誤判成「沒有人在跑」而
// 啟動第二個 server 接管同一 socket）。round 3 的裁決是不再逐一補這些競態，改成每次執行用自己
// 專屬的路徑（`/tmp/cockpit-accept-<run id>.sock`，透過 `HERDR_SOCKET_PATH` 環境變數告訴
// `herdr server` 綁定這個路徑；真機驗證見 `.superpowers/sdd/tasks/task-script-fix3-report.md`
// 「第一步」：`HERDR_SOCKET_PATH` 確實被 HERDR 0.8.2 採用、預設路徑完全不受影響、一個用專屬路徑
// 一個用預設路徑的兩個 server 可以並存且互不可見）——共用的資源沒了，這一整類競態從根上不存在：
//   - **不再需要「啟動前拒絕執行」檢查**：專屬路徑上不可能有別人的 server，`wslServerAlreadyRunning()`
//     （process 檢查＋socket probe）與只看「機器上有沒有任何 herdr 行程」的 `wslServerProcessRunning()`
//     `listHerdrServerPids()` 都已移除。取代它的是 `ensureDedicatedSocketPathIsFree()`：啟動前
//     確認專屬 socket 路徑不存在（存在＝run id 碰撞，機率上是十六進位 6 碼、16.7M 分之一，或上次
//     異常殘留）——碰到就 `regenerateRunId()` 換一個重試一次，仍然存在才報錯，不猜測、不覆蓋。
//     開發者手動開著的 WSL HERDR server 現在可以與這支腳本的整趟驗收全程並存，腳本完全不查、
//     不碰它。
//   - **`verifyOwnership()` 改成只認「這個 PID 自己」**：不再比對「機器上目前有哪些 herdr 行程」
//     （那份清單現在可能包含開發者自己的 server，機器上有別的 herdr 行程完全合法，不該讓 owned
//     判定失敗）——改成單看 pidfile 記下的這個 PID 的 `/proc/<pid>/comm` 是否確實是 `herdr`、
//     且本次執行的專屬 socket 是否連得上，兩個條件都只問「我們自己這一份」。
//   - **收尾會刪除自己的專屬 socket 檔與 pidfile**：round 2（finding B）當時因為「殘留的 socket
//     檔是共用路徑，刪除可能誤刪別人剛綁定的新 socket」所以整支腳本刻意不刪任何 socket 檔；
//     round 3 的路徑只屬於本次執行、刪除不可能影響任何人，因此改回收尾時刪除（純 argv `rm -f`，
//     刪之前確認路徑符合 `/tmp/cockpit-accept-` 前綴與既有白名單，斷言式防禦）。收尾後的驗證是
//     「自己的 PID 已不在、自己的專屬 socket 不存在」，**不**斷言「機器上沒有任何 herdr 程序」
//     （那台機器上可能還有開發者自己的 server，不關這支腳本的事）。
//   - **R18 步驟（中途停掉再重啟 server）重啟時沿用同一個專屬路徑**（Cockpit 的設定本來就指向
//     它）；重啟前不需要重做「拒絕執行」檢查（原因同上，專屬路徑上不可能有別人），重啟後照舊
//     重新確立所有權憑證（新的 PID／starttime，見下方說明）。
//
// **所有權憑證＝PID＋starttime＋comm，不是只有 PID**（finding A；round 2 補強；round 3 沿用
// 不變）：啟動時讓 WSL 端把「即將成為 herdr server 的那個程序」的 PID 寫進本次執行專屬的 pidfile
// （`/tmp/cockpit-accept-<run id>.pid`）再 `exec`（`startWslServer()`；`exec` 保證 pidfile 裡的
// PID 就是 server 本身，不是啟動它的殼），腳本讀回 pidfile 得到 `ownedPid`；`waitForWslServerUp()`
// 的回傳值一定會被檢查，成功後還要確認「pidfile 記下的這個 PID 的 `/proc/<pid>/comm` 確實是
// `herdr`、且本次執行的專屬 socket 連得上」才算 owned（見 `verifyOwnership()`，round 3 改成只看
// 這個 PID 自己，不再比對機器上的 herdr 行程清單），再多讀一次這個 PID 的 starttime／comm
// （`/proc/<pid>/stat`／`/proc/<pid>/comm`，見 `getProcessIdentity()`）當作完整憑證
// （`establishOwnershipIdentity()`；實測見 report：`exec` 之後 `pgrep -x herdr` 恆為單一行、
// 與 pidfile 相符，herdr 不會再 fork）。**finding 2**：這一整串驗證的回傳值真的會被用來擋
// 流程——初次啟動與 R18 重啟都是「PID 讀取、PID 層 owned、starttime／comm 讀取全部成功才把值
// 指派給 `ownedPid`／`ownedIdentity` 並繼續，否則立刻 throw」（訊息明講「偵測到不是本次啟動的
// server，未接管、未停止」），不會再出現「印了 FAIL 但流程照跑」的情況。
//
// **停止機制＝在單一 wsl.exe 呼叫內「核對身分 → 送 SIGTERM → 等待 → 必要時核對後補 SIGKILL」**
// （finding A round 2；round 3 沿用不變，只是收尾多一步刪除自己的 socket 檔與 pidfile，見上方
// 「換路」段落）：`killAndWaitPid()` 把「核對身分（PID＋starttime＋comm 三者皆符合才算數）→ 送
// `SIGTERM` → 每 0.2 秒重新核對身分並等待 → 逾時（10 秒）前再核對一次身分、符合才補送
// `SIGKILL`」全部收進**這一個函式、單一** `wsl.exe -e bash -c` 呼叫；PID／starttime／comm 一律
// 先在 node 端過白名單驗證，再用位置參數（`$1`／`$2`／`$3`／`$4`）傳給內層腳本，不做字串插值。
// 核對身分若在送 `SIGTERM` 之前就沒通過（行程本來就不在，或存在但身分不符）→ **不送出任何
// signal**；輪詢過程中身分核對失敗（不論是行程真的消失、還是 PID 被重用給別的行程）→ 立刻停止
// 等待、**不再送出任何 signal**——這兩種情況一律視為「原程序已經不在了」，統一不動手。
// `stopWslServerIfOwned()` 因此只需要在 node 端判斷 `pid`／`identity` 是否非空（純本地判斷，
// 不是對 WSL 送出的「比對用」獨立請求）就可以呼叫 `killAndWaitPid()`，並依它回傳的結果碼
// （`GONE_BEFORE_TERM`／`MISMATCH_BEFORE_TERM`／`GONE`／`KILLED_AFTER_TIMEOUT`／
// `STILL_ALIVE_AFTER_KILL`）決定要印什麼訊息。`--verify-ownership-mismatch` 是「不相符不誤停」
// 這條規則的專用驗證模式（finding A 驗證 (iii)：啟動一個真的 server、對同一個真實 PID 餵一個
// 不相符的 starttime，確認 stop 邏輯拒絕動手、印警告、真實行程完全沒被動過，見檔案後段
// `verifyOwnershipMismatchDoesNotStop()`；驗完不會自動收尾，需要驗證者自己手動停掉——round 3
// 之後這個模式跟主流程一樣使用專屬路徑，手動收尾要對記下的 PID 送 SIGTERM，不是全域
// `herdr server stop`，見腳本結尾印出的收尾指示；finding 3：這個模式現在也受
// `assertWslWriteOptIn()` 這個共用入口保護，不會繞過寫入 opt-in）。
//
// distro 由環境變數 `COCKPIT_ACCEPT_WSL_DISTRO`（必填）提供（F1；不寫死在腳本裡，避免帶著特定
// 機器的使用者名稱），一經解析就先過白名單驗證（finding C；`validateDistro()`）才會被用在任何
// 指令上——不合格直接印清楚訊息、非 0 結束、不做任何事，避免含空白或 shell metacharacter 的值
// 讓判定失真，或被 `$(...)`、分號等在 WSL 內執行任意命令。**socket 不再由環境變數提供**（round
// 3 移除 `COCKPIT_ACCEPT_WSL_SOCKET` 與「由 `$HOME` 推得預設 socket」的邏輯——那個推得的路徑正是
// 造成一整類 TOCTOU 的共用資源本身）：改成每次執行自動產生專屬路徑（見上方「換路」段落），同樣
// 過一次既有的白名單驗證（`validateSocket()`，斷言式防禦深度，這個值不是外部輸入，理論上一定
// 通過）。通過驗證後仍然優先走獨立 argv（例如連線探測用 `wsl.exe -d <distro> -e nc -U <socket>`，
// 不組 bash 字串），不需要 `bash -lc` 展開變數的地方就不給它機會展開。
//
// 用法（repo 根，需先 `cargo build -p cockpit` 產生 `target/debug/cockpit.exe`）：
//   HERDR_CLIENT_TEST_ALLOW_WSL_WRITES=1 COCKPIT_ACCEPT_WSL_DISTRO=Ubuntu-24.04 \
//     node docs/research/2026-09-19/live-output-real-check.js
//   # finding A 驗證 (iii) 專用模式（見上方「所有權憑證＝PID＋starttime＋comm」段落）：
//   #   HERDR_CLIENT_TEST_ALLOW_WSL_WRITES=1 COCKPIT_ACCEPT_WSL_DISTRO=Ubuntu-24.04 \
//   #     node docs/research/2026-09-19/live-output-real-check.js --verify-ownership-mismatch
//
// 流程（對應 brief「驗收步驟」1–8）：
//   0.（round 3 換路：不再需要「拒絕執行」檢查——專屬 socket 路徑不可能有別人的 server，見上方
//      「換路」段落）啟動前只做一個極簡防呆：`ensureDedicatedSocketPathIsFree()` 確認本次執行的
//      專屬 socket 路徑目前不存在（存在就換一個 run id 重試一次，仍存在才報錯、非 0 結束）。
//   1. 啟動 WSL 測試 server（finding A：pidfile＋`exec`，讀回 `ownedPid` 並確立完整所有權憑證
//      `ownedIdentity`；round 3：連帶帶上 `HERDR_SOCKET_PATH` 讓 server 綁定本次執行的專屬路徑）
//      → tab.create（label `live_output_acceptance_<本次執行的隨機 run id>`，F1：run id 避免跟
//      其他次執行或使用者自己的 tab 撞名，也是 pidfile／socket 檔名的一部分）→ pane.send_text
//      送「每秒印一行 tick」的迴圈。
//   2. 啟動 `cockpit`（驗收用設定，temp 目錄、非預設埠；`wsl = { distro, socket }` 指向本次執行
//      的專屬路徑）→ headless Chrome 開頁面 → 等測試 pane 的列出現 → 點該列。
//   3.「內容跟上」：取樣面板最大 tick N，任何連續 3 秒視窗內都至少前進一次（沿用
//      live-output-check.js `findStalls()` 的判準與理由，改比對 "tick N" 而非 "line N"）。
//   4. 端點直接驗：`curl -H "Host: 127.0.0.1:<port>"` 回 200 且 `text` 含 `tick`；
//      `Host: evil.example:<port>` 回 403（spec「輸出端點只接受本機同源請求」）。
//   5.（Ruling R18）runtime 斷線後恢復：停 WSL 測試 server（finding A round 2：在單一 wsl.exe
//      呼叫內核對 PID＋starttime＋comm 身分、通過才送 `SIGTERM` 並等待，不是分兩次獨立呼叫比對
//      再停）→ 面板標過期＋顯示原因（503，`error` 欄位）、持續重試 → 重啟 WSL 測試 server（round
//      3：沿用同一個專屬路徑，不需要重做拒絕檢查；finding A：重新寫 pidfile、讀回新的
//      `ownedPid` 並重新確立 `ownedIdentity`——重啟後是全新的程序，starttime 一定不同，不能沿用
//      重啟前的憑證）→ 如實記錄觀察到 (a) 同一 pane 仍在（過期標示與原因消失、內容更新）還是
//      (b) pane 已不存在（404 →「pane 已不存在」）；若要繼續後面步驟要重新建一個測試 tab（brief
//      步驟 5 備註）。
//   6.「pane 被關掉」：`tab.close` → 面板顯示「pane 已不存在」、停止輪詢（CDP Network 事件判定
//      5 秒內沒有新的 `/output` 請求，沿用 live-output-check.js N 段的 `Network.requestWillBeSent`
//      過濾寫法）。
//   7. 不存在的 pane：`probe_pane_read --wsl … --count 1 --metadata-only` 的 stderr 含
//      `pane_not_found`；`curl` 對 Cockpit 端點確認回 404（不是 503）。
//   8. 收尾（`finally`，無論成功失敗都做）：先逐一直接關掉本次執行明確 `tab.create` 過的 tab
//      （含 R18 重啟後重建的那個，F1），再以「label 相符本次 run id 且不在啟動時 baseline 內」
//      的條件掃一次殘留 tab 兜底（F1：baseline 是啟動 WSL 測試 server 後、建任何 tab 前先拿的
//      既有 tab id 集合，避免誤關不是這次執行建立的 tab）；然後停 WSL 測試 server（finding A
//      round 2：只有目前在跑的 PID 且身分核對通過才停，`wslServerStarted` 旗標仍用來判斷「這次
//      執行有沒有啟動過」，但實際下不下手改由身分核對決定；下手的動作是在單一 wsl.exe 呼叫內
//      對 `ownedPid` 送 `SIGTERM`／必要時 `SIGKILL`）、依 PID 收掉 `cockpit.exe`／Chrome、確認
//      埠不再 LISTEN、刪暫存設定檔；最後刪除本次執行專屬的 pidfile 與 socket 檔（round 3：這兩
//      個路徑只屬於本次執行，刪除不會影響任何人，取代 round 2「完全不刪除任何 socket 檔」的
//      做法——round 2 那個做法是因為當時的 socket 路徑是共用的，刪除有誤刪別人 socket 的風險；
//      round 3 的路徑不共用，這個風險不存在）。
//
// 這支腳本操作真的 WSL HERDR，跑一次即結束（不像 live-output-check.js 那樣可重複跑驗回歸），
// 沒有 A–N 那種可選段落旗標；`--verify-ownership-mismatch` 是唯一的例外分支，見上方說明。
"use strict";

const os = require("node:os");
const crypto = require("node:crypto");
const { spawn, spawnSync } = require("node:child_process");
const path = require("node:path");
const fs = require("node:fs");

const REPO = path.resolve(__dirname, "..", "..", "..");
const COCKPIT_EXE = path.join(REPO, "target", "debug", "cockpit.exe");
const PROBE_PANE_READ_EXE = path.join(REPO, "target", "debug", "examples", "probe_pane_read.exe");
const CHROME =
  process.env.COCKPIT_CHROME || "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe";

// F1：distro 不寫死在腳本裡（避免帶著特定機器的使用者名稱），由 `resolveWslEndpoint()` 在
// `main()` 一開頭、送出任何 WSL 請求之前解析並指派這個變數；本檔其餘函式在被呼叫當下讀取這個
// 變數（不是在函式定義當下），所以先解析再呼叫都正確。
let WSL_DISTRO = null;
const WORKSPACE_ID = "wD";
const RUNTIME_ID = "wsl";
// F1／finding A／換路（round 3）：本次執行專屬的 run id（十六進位）——`TAB_LABEL`／
// `WSL_PID_FILE`／`WSL_SOCKET` 都由它組出，三者共用同一個 run id 沒有語意耦合，純粹省一次
// 隨機數。正常情況下模組載入時算一次、全程不變；只有 `ensureDedicatedSocketPathIsFree()`
// 偵測到專屬 socket 路徑啟動前就已經存在時（run id 碰撞，機率上是十六進位 6 碼、16.7M 分之一，
// 或上次異常殘留），才會呼叫 `regenerateRunId()` 換一個，三者一起重新產生——這四個變數因此是
// `let` 而不是 `const`；正常路徑下等同常數。
let RUN_ID = crypto.randomBytes(3).toString("hex");
// F1：label 加上本次執行的隨機 run id，收尾的兜底清理只認這個 label（見 `listTabs()` 呼叫端），
// 避免跟其他次執行、或使用者自己剛好也叫這個名字的 tab 混在一起。
let TAB_LABEL = `live_output_acceptance_${RUN_ID}`;
// finding A：`startWslServer()` 讓 WSL 端把「即將成為 herdr server 的那個程序」的 PID 寫進這個
// 檔案再 `exec`（`exec` 保證寫進去的 PID 就是 server 本身，不是啟動它的殼）；腳本讀回這個檔案
// 得到 `ownedPid`，往後所有停止判斷都靠比對「目前在跑的 PID 是否等於這個值」，不是布林旗標。
let WSL_PID_FILE = `/tmp/cockpit-accept-${RUN_ID}.pid`;
// 換路（round 3）：本次執行專屬的 socket 路徑，取代原本「環境變數 `COCKPIT_ACCEPT_WSL_SOCKET`
// 選填、沒給就用 `wsl.exe -e bash -lc 'echo $HOME'` 推得 `$HOME/.config/herdr/herdr.sock`」的
// 做法——那個做法算出來的路徑是所有次執行、以及開發者手動啟動的 server 共用的同一個預設路徑，
// 是這一整類 TOCTOU 的根源。這個路徑跟 `WSL_PID_FILE` 一樣只含 run id（十六進位）與固定
// 前後綴，不是外部輸入；`resolveWslEndpoint()` 仍會呼叫既有的白名單 `validateSocket()` 驗一次
// （斷言式防禦深度，理論上一定通過）。啟動 server 時透過 `HERDR_SOCKET_PATH` 環境變數告訴
// `herdr server` 綁定這個路徑（真機驗證見 report「第一步」：`HERDR_SOCKET_PATH` 確實被採用、
// 預設路徑完全不受影響、兩個不同路徑的 server 可以並存且互不可見）。
let WSL_SOCKET = `/tmp/cockpit-accept-${RUN_ID}.sock`;
// 換路（round 3）：`ensureDedicatedSocketPathIsFree()` 偵測到專屬路徑碰撞時呼叫這個函式重新
// 產生 run id 及其衍生的三個路徑／label；呼叫時機保證在任何 WSL 請求送出、任何 tab 建立之前，
// 不會有「舊值已經被用過」的殘留狀態需要處理。`validateSocket()` 定義在本檔後段，但函式宣告會
// 整個被提升（hoist），這裡呼叫時模組已經完整載入過一次，不會有引用未定義的問題。
function regenerateRunId() {
  RUN_ID = crypto.randomBytes(3).toString("hex");
  TAB_LABEL = `live_output_acceptance_${RUN_ID}`;
  WSL_PID_FILE = `/tmp/cockpit-accept-${RUN_ID}.pid`;
  WSL_SOCKET = `/tmp/cockpit-accept-${RUN_ID}.sock`;
  validateSocket(WSL_SOCKET, "換過 run id 後重新產生的專屬 socket 路徑");
}
const LOOP_SCRIPT = 'for i in $(seq 1 600); do echo "tick $i"; sleep 1; done\n';
// finding A 驗證 (iii) 的除錯掛勾：`--verify-ownership-mismatch` 模式底下才會用到（見檔案後段
// `verifyOwnershipMismatchDoesNotStop()`），保留成有文件的測試掛勾，不是一次性程式碼——之後要
// 回歸驗證「所有權不相符不誤停」這條規則，直接重跑這個模式即可，不需要再手動改程式碼。
// finding A（round 2）：改用一個不相符的 starttime（而不是不相符的 PID）——`/proc/<pid>/stat`
// 的 starttime 是系統開機後的 tick 數，"1" 只有在系統開機後極短時間內才可能是任何行程的真實
// starttime，跟這支腳本啟動的測試 server（不可能在系統剛開機那一刻啟動）保證不同。
const FAKE_STARTTIME_FOR_MISMATCH_TEST = "1";
// finding 1：`killAndWaitPid()` 送出 SIGTERM 後等待多久才升級成 SIGKILL；真機實測 herdr 通常在
// 140–250 ms 內結束，10 秒是刻意放寬的安全邊界（涵蓋比實測慢很多的情況），不是預期常態耗時。
const KILL_WAIT_TIMEOUT_SEC = 10;

const failures = [];
function check(cond, label) {
  console.log(`${cond ? "ok  " : "FAIL"} ${label}`);
  if (!cond) failures.push(label);
  return cond;
}
const log = (s) => console.log(`[${new Date().toISOString()}] ${s}`);
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

// ---------------------------------------------------------------------------
// 行程與埠工具（沿用 live-output-check.js）
// ---------------------------------------------------------------------------

function pidStillRunning(pid) {
  const r = spawnSync("tasklist", ["/FI", `PID eq ${pid}`, "/NH"], { encoding: "utf8" });
  return typeof r.stdout === "string" && r.stdout.includes(String(pid));
}

function killTree(child, label) {
  if (!child || child.exitCode !== null) return;
  spawnSync("taskkill", ["/PID", String(child.pid), "/T", "/F"], { encoding: "utf8" });
  check(!pidStillRunning(child.pid), `${label} PID ${child.pid} 已終止（tasklist 查無此 PID）`);
}

function isPortListening(port) {
  const r = spawnSync("netstat", ["-ano"], { encoding: "utf8" });
  const needle = `127.0.0.1:${port} `;
  return (r.stdout || "")
    .split("\n")
    .some((line) => line.includes(needle) && line.includes("LISTENING"));
}

function pickPort(start, avoid = []) {
  let port = start;
  while (isPortListening(port) || avoid.includes(port)) port += 1;
  return port;
}

// ---------------------------------------------------------------------------
// CDP（沿用 live-output-check.js 的 CDP class：`click` 已含等元素出現、settle delay、送出前
// 重新核對座標的重試；這裡額外用得到的只有 `send`／`eval`／`waitFor`／`click`／`onEvent`）
// ---------------------------------------------------------------------------

class CDP {
  constructor(ws) {
    this.ws = ws;
    this.id = 0;
    this.pending = new Map();
    this.eventHandlers = [];
    ws.onmessage = (e) => {
      const m = JSON.parse(e.data);
      if (m.id && this.pending.has(m.id)) {
        this.pending.get(m.id)(m);
        this.pending.delete(m.id);
        return;
      }
      if (m.method) {
        for (const { method, handler } of this.eventHandlers) {
          if (method === m.method) handler(m.params);
        }
      }
    };
  }
  send(method, params = {}) {
    const id = ++this.id;
    return new Promise((res) => {
      this.pending.set(id, res);
      this.ws.send(JSON.stringify({ id, method, params }));
    });
  }
  onEvent(method, handler) {
    this.eventHandlers.push({ method, handler });
  }
  async eval(expression) {
    const r = await this.send("Runtime.evaluate", { expression, returnByValue: true });
    if (r.result && r.result.exceptionDetails) {
      throw new Error(`頁面內例外：${JSON.stringify(r.result.exceptionDetails)}`);
    }
    return r.result && r.result.result ? r.result.result.value : undefined;
  }
  async waitFor(expression, timeoutMs, label) {
    const start = Date.now();
    while (Date.now() - start < timeoutMs) {
      if (await this.eval(expression)) {
        check(true, label);
        return true;
      }
      await sleep(50);
    }
    check(false, `逾時（${timeoutMs} ms）：${label}`);
    return false;
  }
  async click(selector, holdMs = 0) {
    const SETTLE_AFTER_SCROLL_MS = 100;
    const FIND_TIMEOUT_MS = 5000;
    const FIND_POLL_MS = 50;
    const RECHECK_RETRIES = 5;
    const RECHECK_INTERVAL_MS = 100;
    const sel = JSON.stringify(selector);

    const findStart = Date.now();
    let rect = null;
    while (Date.now() - findStart < FIND_TIMEOUT_MS) {
      rect = await this.eval(
        `(() => { const n = document.querySelector(${sel});
          if (!n) return null; n.scrollIntoView({block: 'start'});
          const r = n.getBoundingClientRect(); return {x: r.left + r.width / 2, y: r.top + r.height / 2}; })()`
      );
      if (rect) break;
      await sleep(FIND_POLL_MS);
    }
    if (!rect) {
      check(false, `找不到可點的元素：${selector}（輪詢 ${FIND_TIMEOUT_MS} ms 仍找不到）`);
      return false;
    }

    await sleep(SETTLE_AFTER_SCROLL_MS);

    for (let attempt = 1; ; attempt += 1) {
      const hit = await this.eval(
        `(() => { const n = document.querySelector(${sel}); if (!n) return null;
          const el = document.elementFromPoint(${rect.x}, ${rect.y});
          return !!el && n.contains(el); })()`
      );
      if (hit) break;
      if (attempt >= RECHECK_RETRIES) {
        check(false, `座標上的元素不是目標本身或其子孫（重試 ${RECHECK_RETRIES} 次仍不吻合）：${selector}`);
        return false;
      }
      await sleep(RECHECK_INTERVAL_MS);
      const refreshed = await this.eval(
        `(() => { const n = document.querySelector(${sel}); if (!n) return null;
          const r = n.getBoundingClientRect(); return {x: r.left + r.width / 2, y: r.top + r.height / 2}; })()`
      );
      if (!refreshed) {
        check(false, `找不到可點的元素：${selector}（重新取座標時元素已經消失）`);
        return false;
      }
      rect = refreshed;
    }

    const base = { x: rect.x, y: rect.y, button: "left", clickCount: 1 };
    await this.send("Input.dispatchMouseEvent", { type: "mouseMoved", x: rect.x, y: rect.y });
    await this.send("Input.dispatchMouseEvent", { type: "mousePressed", ...base });
    if (holdMs > 0) await sleep(holdMs);
    await this.send("Input.dispatchMouseEvent", { type: "mouseReleased", ...base });
    return true;
  }
}

async function startChrome(cdpPort, url, label, windowSize = "1400,1600") {
  const udd = fs.mkdtempSync(path.join(os.tmpdir(), `cockpit-chrome-${label}-`));
  const chrome = spawn(
    CHROME,
    [
      "--headless=new",
      "--disable-gpu",
      "--no-first-run",
      `--remote-debugging-port=${cdpPort}`,
      "--remote-allow-origins=*",
      `--user-data-dir=${udd}`,
      `--window-size=${windowSize}`,
      url,
    ],
    { stdio: "ignore", windowsHide: true }
  );
  chrome.on("error", (e) => check(false, `${label} spawn error：${e.message}`));
  let page = null;
  for (let i = 0; i < 100 && !page; i++) {
    try {
      const r = await fetch(`http://127.0.0.1:${cdpPort}/json/list`);
      page = (await r.json()).find((t) => t.type === "page" && t.url.startsWith(url));
    } catch {
      // CDP endpoint 還沒起來。
    }
    if (!page) await sleep(200);
  }
  if (!page) throw new Error(`${label}: page target not found`);
  const ws = new WebSocket(page.webSocketDebuggerUrl);
  await new Promise((res, rej) => {
    ws.onopen = res;
    ws.onerror = rej;
  });
  return { chrome, udd, ws, cdp: new CDP(ws) };
}

async function stopChrome(handle, label) {
  if (!handle) return;
  try {
    handle.ws.close();
  } catch {
    // 已斷線。
  }
  killTree(handle.chrome, label);
  await sleep(500);
  try {
    fs.rmSync(handle.udd, { recursive: true, force: true });
  } catch (e) {
    check(false, `清理暫存目錄失敗：${e.message}`);
  }
}

// ---------------------------------------------------------------------------
// WSL 測試 server：啟停、JSON-RPC（沿用 acceptance_common.py `wsl_rpc`／`wsl_bash` 的手法與
// docs/handover.md §1 的指令，逐字對照過）
// ---------------------------------------------------------------------------

function wslBash(script, timeoutMs = 30000) {
  return spawnSync("wsl.exe", ["-d", WSL_DISTRO, "-e", "bash", "-lc", script], {
    encoding: "utf8",
    timeout: timeoutMs,
  });
}

// 一條連線一個 method（handover 第 4 節、brief「對 WSL 送 JSON-RPC」）：server 回應後關連線，
// nc 自行結束，不需要額外的逾時處理來讓連線收尾。這裡的 `WSL_SOCKET` 是純 argv（陣列元素），
// 不經過任何 shell，不是 finding C 的問題點。
function wslRpc(id, method, params, timeoutMs = 15000) {
  const line = JSON.stringify({ id, method, params }) + "\n";
  const r = spawnSync("wsl.exe", ["-d", WSL_DISTRO, "-e", "nc", "-U", WSL_SOCKET], {
    input: line,
    encoding: "utf8",
    timeout: timeoutMs,
  });
  if (r.error) throw new Error(`wsl_rpc ${method} spawn error：${r.error.message}`);
  const out = (r.stdout || "").trim().split("\n").filter(Boolean);
  if (out.length === 0) {
    throw new Error(`wsl_rpc ${method} 沒有回應（stderr: ${(r.stderr || "").slice(0, 300)}）`);
  }
  return JSON.parse(out[0]);
}

// finding C：distro／socket 的白名單——只允許不會被任何 shell 特殊解讀的字元。socket 必須是
// 絕對路徑；兩者都禁止空白與 shell metacharacter（`;`、`$`、`` ` ``、`|`、`&`、`(`、`)` 等，
// 白名單寫法本身就排除了它們，不需要另外列黑名單）。這兩個值一經解析（`resolveWslEndpoint()`）
// 就先驗證過才會被用在任何指令或設定檔裡。
const DISTRO_PATTERN = /^[A-Za-z0-9._-]+$/;
const SOCKET_PATTERN = /^\/[A-Za-z0-9._/-]+$/;

function validateDistro(distro) {
  if (!DISTRO_PATTERN.test(distro)) {
    console.error(
      `COCKPIT_ACCEPT_WSL_DISTRO 格式不合法（實際值 ${JSON.stringify(distro)}）：只能包含英數字、` +
        `點、底線、連字號（白名單 ${DISTRO_PATTERN}）；不合格直接拒絕，不嘗試任何指令。`
    );
    process.exit(2);
  }
}

function validateSocket(socket, source) {
  if (!SOCKET_PATTERN.test(socket)) {
    console.error(
      `${source} 格式不合法（實際值 ${JSON.stringify(socket)}）：socket 必須是絕對路徑，且只能` +
        `包含英數字、點、底線、連字號、斜線（白名單 ${SOCKET_PATTERN}）；不得含空白或 shell` +
        "特殊字元。不合格直接拒絕，不嘗試任何指令（finding C：避免這個值被插進 shell 字串時" +
        "讓判定失真，或執行任意命令）。"
    );
    process.exit(2);
  }
}

// F1／換路（round 3）：解析 distro 並指派模組層級的 `WSL_DISTRO`；必須在送出任何 WSL 請求之前
// 呼叫一次（`main()`／`verifyOwnershipMismatchDoesNotStop()` 最先做的事之一，早於
// `ensureDedicatedSocketPathIsFree()`）。`COCKPIT_ACCEPT_WSL_DISTRO` 沒給就直接報錯結束——不
// 悄悄落入某台機器的預設值。finding C：這個值先過白名單驗證（`validateDistro()`）才會被指派，
// 之後本檔任何函式讀到的 `WSL_DISTRO` 保證已經驗證過。`WSL_SOCKET` 不再由這裡處理——它已經在
// 模組載入時由本次執行的 run id 產生（見上方常數區），這裡只是斷言式地再驗一次白名單（防禦
// 深度，這個值不是外部輸入，理論上一定通過）。
function resolveWslEndpoint() {
  const distro = process.env.COCKPIT_ACCEPT_WSL_DISTRO;
  if (!distro) {
    console.error(
      "缺少環境變數 COCKPIT_ACCEPT_WSL_DISTRO：請設定成要用來驗收的 WSL distro 名稱" +
        "（例如 COCKPIT_ACCEPT_WSL_DISTRO=Ubuntu-24.04）再重跑。"
    );
    process.exit(2);
  }
  validateDistro(distro);
  WSL_DISTRO = distro;
  validateSocket(WSL_SOCKET, "本次執行自動產生的專屬 socket 路徑");
}

// 換路（round 3）：純 argv 檢查一個路徑是否存在（不論檔案類型；`test -e`，不是 `test -S`——
// 用來偵測「這個路徑上有沒有任何東西」，不限定一定要是 socket）。只有 `ensureDedicatedSocketPathIsFree()`
// 呼叫，不經過任何 shell 字串。
function wslPathExists(path) {
  const r = spawnSync("wsl.exe", ["-d", WSL_DISTRO, "-e", "test", "-e", path], {
    encoding: "utf8",
    timeout: 5000,
  });
  return r.status === 0;
}

// 換路（round 3）：取代舊版「啟動前拒絕執行」檢查（`wslServerAlreadyRunning()`：process 檢查＋
// socket probe，兩次獨立呼叫之間有窄縫）。專屬路徑不可能有別人的 server，所以不再需要判斷
// 「有沒有別人」；唯一需要防的是本次執行自己的 run id 剛好撞到殘留檔（機率上是十六進位 6 碼、
// 16.7M 分之一，或上一次異常中止留下的殘留）。偵測到路徑已存在就換一個 run id 重試一次；重試後
// 還在，代表情況不單純（例如權限問題，或極端情況下的重複撞號），直接報錯中止，不猜測、不覆蓋
// 別人可能留在那個路徑上的東西。
function ensureDedicatedSocketPathIsFree() {
  if (!wslPathExists(WSL_SOCKET)) return;
  log(
    `專屬 socket 路徑 ${WSL_SOCKET} 在啟動前就已經存在（run id 碰撞，或上次異常中止留下的殘留）` +
      "，換一個 run id 重試一次。"
  );
  regenerateRunId();
  if (wslPathExists(WSL_SOCKET)) {
    throw new Error(
      `換過 run id 後，專屬 socket 路徑 ${WSL_SOCKET} 仍然存在，中止（這不是共用路徑本該有的` +
        "競態，需要人工檢查 WSL /tmp 底下為什麼會有這個檔案，不猜測、不覆蓋）。"
    );
  }
  log(`換過 run id，本次執行改用：pidfile=${WSL_PID_FILE} socket=${WSL_SOCKET}`);
}

// finding A：`setsid -f` 之後接一個內層 `bash -c '<單引號字串>'`——單引號字串裡的 `$0`／`$1`／
// `$HOME` 不會被外層 `bash -lc` 展開（單引號內沒有變數展開），只會在 `setsid -f` detach 出去
// 之後、由那個內層 `bash -c` 自己展開。`echo $$ > pidfile` 先把內層 bash 自己的 PID 寫進
// pidfile，接著 `exec env HERDR_SOCKET_PATH=... "$HOME/.local/bin/herdr" server ...` 讓這個
// bash 程序就地換成 herdr server 本身（`exec` 不會產生新的 PID）——因此 pidfile 裡的 PID 保證
// 就是 server 本身的 PID，不是啟動它的殼。實測（見 report）：`exec` 之後 `pgrep -x herdr` 恆為
// 單一行、PID 與 pidfile 相符，herdr 不會再 fork 出子行程。
//
// 換路（round 3）：`HERDR_SOCKET_PATH` 放在 `exec` 之前（已知坑：放在 `exec` 之後才接的話，
// 接到的是 herdr 自己的環境，不是我們要傳給它的值——這裡是在 `exec` 同一行內用 `env NAME=value
// command` 的形式帶過去，不是分兩步設定再 exec）。`WSL_PID_FILE`／`WSL_SOCKET` 都用位置參數
// （外層 `$1`／`$2`，轉給內層 bash -c 變成它自己的 `$0`／`$1`）傳入，不做字串插值——即使兩者
// 本身是本次執行自己產生的 run id 組出來的、不是外部輸入，這裡仍比照 brief 的既定做法，避免
// 巢狀 bash -c 字串插值本身成為未來維護時的錯誤示範。傳給 `wsl.exe` 的 argv 因此比 `wslBash()`
// 多兩個尾端參數（`start-wsl-server` 只是佔用 `$0` 的位置，內容本身不使用），這裡直接呼叫
// `spawnSync()` 而不是共用的 `wslBash()`（那個 helper 的簽名不支援附加額外 argv）。
//
// 內層 `bash -c '...'` 結尾額外加 `</dev/null >/dev/null 2>&1`（實測踩到的環境坑，見 report）：
// 沒有這段重導向時，這整條 `setsid -f bash -c '...'` 大約三、四次會有一次完全沒有留下任何
// 行程（`wsl.exe` 回 exit code 0、沒有任何 stderr，但 `pgrep -x herdr`／pidfile 都是空的，
// 像什麼都沒發生過）——重現後拆解發現問題出在內層 `bash -c` 繼承了外層 `bash -lc` 的
// stdin／stdout／stderr（來自呼叫 `wsl.exe` 的那個行程，這裡是 Node 的 `spawnSync`），
// 與 `setsid -f` 的 fork＋detach 之間有競態：外層 `wsl.exe` 行程結束、那些繼承來的 I/O
// 控制代碼被收回時，如果剛好發生在內層 `bash -c` 完成 `setsid()` 之前，內層行程就會連著被
// 一起收掉。幫內層 `bash -c` 自己的 stdin／stdout／stderr 明確接上 `/dev/null`（而不是等到
// `exec` 之後才幫 herdr 接）之後，同樣的操作連續跑數十次（含單獨的 `sleep` 最小重現與完整
// pidfile＋`exec herdr` 版本）沒有再出現這個問題。這跟 Node 或 Git Bash 呼叫 `wsl.exe`無關
// （兩種呼叫方式都重現過同樣的競態），是 `setsid -f` 搭配巢狀 `bash -c` 這個組合本身的坑。
function startWslServer() {
  const script =
    "setsid -f bash -c 'echo $$ > \"$0\"; " +
    'exec env HERDR_SOCKET_PATH="$1" "$HOME/.local/bin/herdr" server ' +
    ">/tmp/herdr-server.log 2>&1 </dev/null' " +
    '"$1" "$2" </dev/null >/dev/null 2>&1';
  spawnSync(
    "wsl.exe",
    ["-d", WSL_DISTRO, "-e", "bash", "-lc", script, "start-wsl-server", WSL_PID_FILE, WSL_SOCKET],
    { encoding: "utf8", timeout: 30000 }
  );
}

// finding A（round 2）：白名單——所有權憑證的三個欄位（PID、starttime、comm）在被組進任何 WSL
// 端指令之前，一律先在 node 端驗證過。PID／starttime 是 `/proc` 裡的數字欄位，comm 是
// `/proc/<pid>/comm` 的內容（HERDR 應恆為 `herdr`），三者都用位置參數傳遞、不做字串插值，這裡的
// 白名單是防禦深度，不是唯一防線。
const PID_PATTERN = /^[0-9]+$/;
const STARTTIME_PATTERN = /^[0-9]+$/;
const COMM_PATTERN = /^[A-Za-z0-9._-]+$/;

// finding A（round 2）：讀回一個 PID 目前的「身分」——`/proc/<pid>/comm`（行程名稱）與
// `/proc/<pid>/stat` 第 22 欄（`starttime`，開機後的 tick 數，只要行程還是同一個就不會變，
// PID 被系統重用給別的行程之後幾乎必然不同）。`comm` 欄在 `stat` 裡用括號包住、可能含空白或
// 括號本身，所以不直接切 `stat` 的欄位取 comm，改用 `/proc/<pid>/comm` 這個獨立檔案（單純一行，
// 沒有這個問題）；`starttime` 則取 `stat` 最後一個 `)` 之後、以空白切開的第 20 個欄位（brief：
// 整行的第 22 欄，扣掉 pid／comm 兩欄之後即為第 20 欄）。單一 `wsl.exe -e bash -c` 呼叫，PID 用
// 位置參數傳入；讀不到（行程不存在、沒有讀取權限）回傳 `null`。
function getProcessIdentity(pid) {
  if (!PID_PATTERN.test(String(pid))) {
    throw new Error(`getProcessIdentity：pid 不是純數字（${JSON.stringify(pid)}），拒絕查詢`);
  }
  const script =
    'pid="$1"; ' +
    'if [ ! -r "/proc/$pid/stat" ]; then exit 1; fi; ' +
    'stat_content=$(cat "/proc/$pid/stat" 2>/dev/null) || exit 1; ' +
    'after="${stat_content##*)}"; ' +
    'comm=$(cat "/proc/$pid/comm" 2>/dev/null) || exit 1; ' +
    'set -- $after; starttime="${20}"; ' +
    'printf "%s\\t%s\\n" "$comm" "$starttime"';
  const r = spawnSync(
    "wsl.exe",
    ["-d", WSL_DISTRO, "-e", "bash", "-c", script, "get-identity", String(pid)],
    { encoding: "utf8", timeout: 5000 }
  );
  if (r.status !== 0) return null;
  const line = (r.stdout || "").trim();
  const [comm, starttime] = line.split("\t");
  if (!comm || !starttime || !COMM_PATTERN.test(comm) || !STARTTIME_PATTERN.test(starttime)) {
    return null;
  }
  return { comm, starttime };
}

// finding A（round 2）：修正上一輪的殘留問題——`stopWslServerIfOwned()` 先前用一次獨立的
// `pgrep` invocation 比對 `ownedPid`，比對之後才呼叫這個函式送出 `SIGTERM`／輪詢／必要時
// `SIGKILL`；兩次獨立 invocation 之間仍有窄縫：原程序若剛好在這個窗口退出、PID 被系統重用給
// 別的行程，第二個 invocation 送出的訊號就會打在無關的行程上。修法：所有權憑證改成
// 「PID＋starttime＋comm」，且「核對身分 → 送 SIGTERM → 輪詢核對 → 必要時核對後補 SIGKILL」
// 全部收進**這一個**函式、**單一** `wsl.exe -e bash -c` 呼叫，PID／starttime／comm 都用位置
// 參數傳入內層腳本（`$1`／`$2`／`$3`／`$4`），呼叫前三者都先過白名單驗證。內層腳本的
// `check_identity()`：`/proc/$pid` 不存在或讀取失敗 → 回傳 2（視為「本來就不在」）；
// comm／starttime 其中之一與預期不符 → 回傳 1（視為「身分不符」，可能是 PID 被重用）；兩者皆
// 符合 → 回傳 0。四個階段：
//   1. 送 `SIGTERM` 之前先核對一次——不符合就直接回報，**不送任何 signal**。
//   2. 核對通過才送 `SIGTERM`，之後每 0.2 秒重新核對一次身分——身分不符或行程消失都代表「原
//      程序已經不在了」，立刻停止等待，**不再送任何 signal**（不論是不是我們自己的 SIGTERM
//      造成的）。
//   3. 逾時（`timeoutSec`）之後、真的要補 `SIGKILL` 之前，再核對一次身分，符合才送；核對當下
//      發現已經不在了就直接視為已結束，不補送 `SIGKILL`。
//   4. 送出 `SIGKILL` 之後再核對一次確認結果。
// stdout 用互斥的標記回報四種以上的結果，呼叫端（`stopWslServerIfOwned()`）依標記決定 log 內容：
// `GONE_BEFORE_TERM`（本來就不在，未送 signal）、`MISMATCH_BEFORE_TERM`（身分不符，未送
// signal）、`GONE`（送出 SIGTERM 後確認已結束，即「已由本次停止」）、`KILLED_AFTER_TIMEOUT`
// （逾時後核對身分通過才補送 SIGKILL 並確認已結束）、`STILL_ALIVE_AFTER_KILL`（SIGKILL 之後
// 仍存活，異常情況）。
function killAndWaitPid(pid, identity, timeoutSec) {
  if (!PID_PATTERN.test(String(pid))) {
    throw new Error(`killAndWaitPid：pid 不是純數字（${JSON.stringify(pid)}），拒絕送出 kill`);
  }
  if (!identity || !STARTTIME_PATTERN.test(String(identity.starttime))) {
    throw new Error(`killAndWaitPid：starttime 不是純數字（${JSON.stringify(identity && identity.starttime)}），拒絕送出 kill`);
  }
  if (!COMM_PATTERN.test(identity.comm)) {
    throw new Error(`killAndWaitPid：comm 不符合白名單（${JSON.stringify(identity.comm)}），拒絕送出 kill`);
  }
  const script =
    'pid="$1"; exp_starttime="$2"; exp_comm="$3"; timeout_s="$4"; ' +
    "check_identity() { " +
    'if [ ! -r "/proc/$pid/stat" ]; then return 2; fi; ' +
    'stat_content=$(cat "/proc/$pid/stat" 2>/dev/null) || return 2; ' +
    'after="${stat_content##*)}"; ' +
    'comm=$(cat "/proc/$pid/comm" 2>/dev/null) || return 2; ' +
    'set -- $after; starttime="${20}"; ' +
    'if [ "$comm" != "$exp_comm" ] || [ "$starttime" != "$exp_starttime" ]; then return 1; fi; ' +
    "return 0; " +
    "}; " +
    "check_identity; rc=$?; " +
    'if [ "$rc" -eq 2 ]; then echo GONE_BEFORE_TERM; exit 0; fi; ' +
    'if [ "$rc" -eq 1 ]; then echo MISMATCH_BEFORE_TERM; exit 0; fi; ' +
    'kill -TERM "$pid" 2>/dev/null; ' +
    'max_iters=$((timeout_s * 5)); i=0; ' +
    "while :; do " +
    "check_identity; rc=$?; " +
    'if [ "$rc" -ne 0 ]; then echo GONE; exit 0; fi; ' +
    'if [ "$i" -ge "$max_iters" ]; then break; fi; ' +
    "sleep 0.2; i=$((i + 1)); " +
    "done; " +
    "check_identity; rc=$?; " +
    'if [ "$rc" -ne 0 ]; then echo GONE; exit 0; fi; ' +
    'kill -KILL "$pid" 2>/dev/null; ' +
    "sleep 0.3; " +
    "check_identity; rc=$?; " +
    'if [ "$rc" -ne 0 ]; then echo KILLED_AFTER_TIMEOUT; else echo STILL_ALIVE_AFTER_KILL; fi';
  const r = spawnSync(
    "wsl.exe",
    [
      "-d",
      WSL_DISTRO,
      "-e",
      "bash",
      "-c",
      script,
      "kill-and-wait",
      String(pid),
      String(identity.starttime),
      identity.comm,
      String(timeoutSec),
    ],
    { encoding: "utf8", timeout: (timeoutSec + 10) * 1000 }
  );
  if (r.error) throw new Error(`killAndWaitPid(${pid}) spawn error：${r.error.message}`);
  const out = (r.stdout || "").trim();
  const code = out.split("\n").pop();
  return { code };
}

// 換路（round 3）：連線探測改用純 argv（`wsl.exe -d <distro> -e nc -U <socket>`，不組 bash
// 字串）送一筆真的 JSON-RPC `session.snapshot`，短逾時內收到**任何**可解析的 JSON-RPC 回應
// （不論是成功結果還是錯誤）就代表「真的連得上」——有一個活的 server 在另一端回應；socket 檔
// 不存在、沒有任何行程在監聽（ECONNREFUSED）、或逾時完全沒有回應，都回傳 false。`WSL_SOCKET`
// 現在永遠是本次執行的專屬路徑，所以這個函式問的一律是「我們自己這一份還連得上嗎」，不是「機器
// 上有沒有人在用共用路徑」（那個問題已經隨著換路不存在了，見檔頭「換路」段落）。
function wslSocketConnectable(timeoutMs = 3000) {
  const line = JSON.stringify({ id: "alive-probe", method: "session.snapshot", params: {} }) + "\n";
  const r = spawnSync("wsl.exe", ["-d", WSL_DISTRO, "-e", "nc", "-U", WSL_SOCKET], {
    input: line,
    encoding: "utf8",
    timeout: timeoutMs,
  });
  const out = (r.stdout || "").trim();
  if (!out) return false;
  try {
    JSON.parse(out.split("\n")[0]);
    return true;
  } catch {
    return false;
  }
}

// finding A：讀回 `startWslServer()` 寫進 `WSL_PID_FILE` 的 PID（純 argv，`cat` 是獨立執行檔，
// 不經過 shell）。檔案不存在、讀取失敗、或內容不是純數字都回傳 `null`（呼叫端用 `check()` 把
// 這個情況變成明確的 FAIL，不會把 `null` 誤當成一個合法 PID 去比對）。
function readOwnedPid() {
  const r = spawnSync("wsl.exe", ["-d", WSL_DISTRO, "-e", "cat", WSL_PID_FILE], {
    encoding: "utf8",
    timeout: 5000,
  });
  if (r.status !== 0) return null;
  const pid = (r.stdout || "").trim();
  return /^[0-9]+$/.test(pid) ? pid : null;
}

// 換路（round 3）：「owned」的定義改成只問這個 PID 自己——不再比對「機器上目前有哪些名稱為
// herdr 的行程」（那份清單現在可能包含開發者自己另外開著的 server，機器上有別的 herdr 行程
// 完全合法，不該讓 owned 判定失敗；舊版 `listHerdrServerPids()`／`pgrep -x herdr` 的機器層級
// 清單比對已移除）。兩個條件都只問我們自己這一份：(1) pidfile 記下的這個 PID 的
// `/proc/<pid>/comm` 是否確實是 `herdr`（用 `getProcessIdentity()` 讀，不需要另外的清單比對）；
// (2) 本次執行的專屬 socket 是否連得上（`wslSocketConnectable()`，因為路徑專屬，連得上就代表
// 是我們自己剛啟動的這個，不可能是別人）。`check()` 把結果印成明確的 PASS／FAIL 並記進
// `failures`（brief 驗證 (i)「輸出中看得到 ownedPid 與每次停止前的 PID 比對結果」）。這裡只確立
// PID 這一層；完整的所有權憑證（PID＋starttime＋comm）由 `establishOwnershipIdentity()` 在這
// 一層通過之後再補上。
function verifyOwnership(pid, label) {
  const identity = pid !== null ? getProcessIdentity(pid) : null;
  const commIsHerdr = identity !== null && identity.comm === "herdr";
  const socketOk = wslSocketConnectable();
  const owned = pid !== null && commIsHerdr && socketOk;
  check(
    owned,
    `${label}（pidfile PID=${JSON.stringify(pid)}，該 PID 的 comm=` +
      `${identity ? JSON.stringify(identity.comm) : "null"}，專屬 socket 連得上=${socketOk}）`
  );
  return owned;
}

// finding A（round 2）：所有權憑證＝PID＋starttime＋comm，不是只有 PID。`verifyOwnership()`
// 通過之後（PID 這一層核對相符），這裡再多讀一次這個 PID 目前的 starttime／comm
// （`getProcessIdentity()`）當作往後每次停止前核對身分的基準值；讀不到（行程消失、權限問題）
// 一律視為建立所有權失敗，呼叫端要把這個結果當成跟 `verifyOwnership()` 回 false 一樣嚴重的
// 情況處理（立刻 throw，不把 `null` 憑證交給 `ownedIdentity`）。
function establishOwnershipIdentity(pid, label) {
  const owned = verifyOwnership(pid, label);
  if (!owned) return null;
  const identity = getProcessIdentity(pid);
  check(
    identity !== null,
    `${label}：讀到所有權憑證（starttime／comm，供之後每次停止前核對身分）` +
      `（實際 ${JSON.stringify(identity)}）`
  );
  return identity;
}

// finding A（round 2）：移除上一輪殘留的「先用一次獨立的 pgrep invocation 比對，再呼叫另一個
// 獨立 invocation 執行停止」——那正是 finding A 這一輪指出的 TOCTOU（比對與送 signal 是兩次
// 獨立 `wsl.exe` invocation，中間仍有窄縫）。現在「核對身分 → 送 SIGTERM → 輪詢核對 → 必要時
// 核對後補 SIGKILL」全部收進 `killAndWaitPid()` 內的**單一** invocation；這裡只做 node 端本地
// 判斷（`pid`／`identity` 是否非空），不再對 WSL 送出任何「比對用」的獨立請求。這是本檔唯一會
// 呼叫 `killAndWaitPid()` 的地方，不會有任何路徑繞過它直接送 signal。確認結束之後這裡**不**
// 處理 socket 檔／pidfile 的刪除——那是本次執行專屬路徑的清理工作，換路（round 3）之後統一由
// `main()` 收尾階段在確認停止之後一次做（見 `main()` 的 `finally` 區塊），不是這個函式的職責。
function stopWslServerIfOwned(pid, identity, context) {
  if (!pid || !identity) {
    log(`${context}：ownedPid／所有權憑證為空，跳過停止（不確定是不是自己的 server，寧可不停）。`);
    return false;
  }
  log(
    `${context}：對 PID ${pid}（comm=${identity.comm} starttime=${identity.starttime}）在同一次 ` +
      "wsl.exe 呼叫內完成「核對身分 → 送 SIGTERM → 等待，必要時逾時後核對身分再補 SIGKILL」。"
  );
  const { code } = killAndWaitPid(pid, identity, KILL_WAIT_TIMEOUT_SEC);
  switch (code) {
    case "GONE_BEFORE_TERM":
      log(`${context}：PID ${pid} 在核對身分當下就已經不存在（本來就不在），未送出任何 signal。`);
      return false;
    case "MISMATCH_BEFORE_TERM":
      log(
        `警告：${context}時 PID ${pid} 存在，但 comm／starttime 與記錄的所有權憑證不符（可能已被` +
          "系統重用給別的行程)，未送出任何 signal（避免誤停不屬於這次執行的行程）。"
      );
      return false;
    case "GONE":
      check(true, `${context}：PID ${pid} 送出 SIGTERM 後（身分核對通過才送）確認已結束`);
      return true;
    case "KILLED_AFTER_TIMEOUT":
      check(true, `${context}：PID ${pid} 逾時未回應 SIGTERM，核對身分後才補送 SIGKILL 並確認已結束`);
      return true;
    case "STILL_ALIVE_AFTER_KILL":
      check(false, `${context}：PID ${pid} 送出 SIGTERM／SIGKILL 後仍然存活（異常，需要人工介入）`);
      return false;
    default:
      check(false, `${context}：killAndWaitPid() 回傳未知的結果碼 ${JSON.stringify(code)}`);
      return false;
  }
}

async function waitForWslServerUp(timeoutMs, label) {
  const start = Date.now();
  while (Date.now() - start < timeoutMs) {
    try {
      const resp = wslRpc("ready-check", "session.snapshot", {}, 8000);
      if (resp && resp.result) {
        check(true, label);
        return true;
      }
    } catch {
      // server 剛起來，socket 還沒真的能回應（或路徑還沒建立）；繼續等。
    }
    await sleep(500);
  }
  check(false, `逾時（${timeoutMs} ms）：${label}`);
  return false;
}

// 換路（round 3）：不再看「機器上有沒有任何 herdr 行程」（那份清單可能包含開發者自己的
// server，不關這支腳本的事，見檔頭「換路」段落）。只問「我們自己的專屬 socket 還連得上嗎」——
// 因為路徑專屬，連不上就代表我們自己啟動的這個真的停了（不可能有別人在這個路徑上頂替）。
async function waitForWslServerDown(timeoutMs, label) {
  const start = Date.now();
  while (Date.now() - start < timeoutMs) {
    if (!wslSocketConnectable()) {
      check(true, label);
      return true;
    }
    await sleep(300);
  }
  check(false, `逾時（${timeoutMs} ms）：${label}`);
  return false;
}

// 建測試 tab＋送迴圈；回傳 { tabId, paneId }。
function createTestTabAndLoop() {
  const created = wslRpc("tc", "tab.create", {
    workspace_id: WORKSPACE_ID,
    label: TAB_LABEL,
    focus: false,
  });
  if (!created.result) {
    throw new Error(`tab.create 沒有 result：${JSON.stringify(created)}`);
  }
  const tabId = created.result.tab.tab_id;
  const paneId = created.result.root_pane.pane_id;
  log(`tab.create → tab_id=${tabId} pane_id=${paneId}`);
  const sent = wslRpc("st", "pane.send_text", { pane_id: paneId, text: LOOP_SCRIPT });
  check(
    sent.result && sent.result.type === "ok",
    `pane.send_text 送出 tick 迴圈到 ${paneId}（實際 ${JSON.stringify(sent)}）`
  );
  return { tabId, paneId };
}

function closeTab(tabId) {
  const resp = wslRpc("tclose", "tab.close", { tab_id: tabId });
  return resp.result && resp.result.type === "ok";
}

// F1：列出目前所有 tab（`{tab_id, label}`）。兩個用途共用同一份原始清單：(1) 啟動 server 後、
// 建任何 tab 前拿一次當 baseline；(2) 收尾時拿一次，篩「label 相符本次 run id 且不在 baseline
// 內」的 tab 做兜底清理（見 `main()`）。
//
// round 4：一律經 `wslRpc()` 走本次執行專屬的 socket。原本用 `herdr api snapshot` CLI，沒帶
// `HERDR_SOCKET_PATH`，查到的是預設路徑的 server——預設 server 沒開時靜默拿到空清單，有開時
// 則是拿別人 server 的 tab 清單替專屬 server 做清理決策，兩者都違反換路（round 3）的隔離。
// 讀不到或格式不對就拋錯、不回空清單：baseline 拿不到要中止，收尾的呼叫端本來就包在 try 裡。
function listTabs() {
  const resp = wslRpc("tabs", "session.snapshot", {});
  const tabs = resp && resp.result && resp.result.snapshot && resp.result.snapshot.tabs;
  if (!Array.isArray(tabs)) {
    throw new Error(
      `session.snapshot 回應沒有 tabs 陣列：${JSON.stringify(resp).slice(0, 300)}`
    );
  }
  return tabs;
}

// ---------------------------------------------------------------------------
// curl（帶自訂 Host 標頭，驗 source_check 與端點行為；沿用 handover.md 既有的 curl 用法）
// ---------------------------------------------------------------------------

const CURL_SCRATCH = fs.mkdtempSync(path.join(os.tmpdir(), "cockpit-live-output-real-curl-"));

function curlGet(url, hostHeader) {
  const bodyFile = path.join(CURL_SCRATCH, `body-${Date.now()}-${Math.random().toString(16).slice(2)}.txt`);
  const r = spawnSync(
    "curl",
    ["-s", "-D", "-", "-o", bodyFile, "-w", "\n%{http_code}", "-H", `Host: ${hostHeader}`, url],
    { encoding: "utf8", timeout: 15000 }
  );
  const status = Number((r.stdout || "").trim().split("\n").pop());
  let body = "";
  try {
    body = fs.readFileSync(bodyFile, "utf8");
  } catch {
    // 沒寫出檔案（curl 失敗）。
  }
  try {
    fs.unlinkSync(bodyFile);
  } catch {
    // 已經不存在。
  }
  return { status, body, headers: r.stdout || "", stderr: r.stderr || "" };
}

// ---------------------------------------------------------------------------
// cockpit：驗收用設定檔、起停
// ---------------------------------------------------------------------------

// finding C 第 3 點：`validateDistro()`／`validateSocket()` 的白名單已經排除了會讓 TOML 基本字串
// （`"..."`）逃逸或壞掉的字元（雙引號、反斜線；白名單也不含控制字元）；這裡是斷言式的第二層
// 防禦——理論上不會觸發，真的觸發代表白名單漏放行了什麼，直接丟例外中止，不要悄悄寫出格式錯誤
// 或可能逃逸的 TOML。
function assertSafeForTomlBasicString(value, label) {
  if (/["\\]/.test(value)) {
    throw new Error(
      `${label} 含 TOML 基本字串不允許直接內插的字元（雙引號或反斜線，實際值 ${JSON.stringify(value)}）：` +
        "理論上已經被白名單驗證擋下，這裡是斷言式的第二層防禦，不應該走到這裡。"
    );
  }
}

function writeCockpitConfig(port) {
  assertSafeForTomlBasicString(WSL_DISTRO, "WSL_DISTRO");
  assertSafeForTomlBasicString(WSL_SOCKET, "WSL_SOCKET");
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "cockpit-live-output-real-config-"));
  const configPath = path.join(dir, "cockpit.toml");
  const content = [
    "[server]",
    `listen = "127.0.0.1:${port}"`,
    "",
    "[[runtime]]",
    `id = "${RUNTIME_ID}"`,
    'kind = "herdr"',
    `wsl = { distro = "${WSL_DISTRO}", socket = "${WSL_SOCKET}" }`,
    "",
  ].join("\n");
  fs.writeFileSync(configPath, content, "utf8");
  return { dir, configPath };
}

async function startCockpit(configPath, port) {
  if (!fs.existsSync(COCKPIT_EXE)) {
    throw new Error(`找不到 ${COCKPIT_EXE}，請先跑 cargo build -p cockpit`);
  }
  const child = spawn(COCKPIT_EXE, ["--config", configPath], {
    stdio: ["ignore", "ignore", "pipe"],
    windowsHide: true,
    cwd: REPO,
  });
  child.on("error", (e) => check(false, `cockpit spawn error：${e.message}`));
  let stderrTail = "";
  child.stderr.setEncoding("utf8");
  child.stderr.on("data", (chunk) => {
    stderrTail = (stderrTail + chunk).slice(-4000);
  });
  let up = false;
  for (let i = 0; i < 100 && !up && child.exitCode === null; i++) {
    try {
      up = (await fetch(`http://127.0.0.1:${port}/api/state`)).ok;
    } catch {
      // 還沒起來。
    }
    if (!up) await sleep(200);
  }
  check(up, `cockpit 應該在 20 秒內開始回應（port ${port}；若 FAIL 看下面 stderr 節錄）`);
  if (!up) {
    log(`cockpit stderr 節錄：\n${stderrTail}`);
    throw new Error("cockpit 沒有起來");
  }
  return { child, port, stderrTailRef: () => stderrTail };
}

async function waitForRuntimeConnected(port, timeoutMs, label) {
  const start = Date.now();
  while (Date.now() - start < timeoutMs) {
    try {
      const r = await fetch(`http://127.0.0.1:${port}/api/state`);
      if (r.ok) {
        const state = await r.json();
        const rt = (state.runtimes || []).find((x) => x.id === RUNTIME_ID);
        if (rt && rt.connection && rt.connection.state === "connected") {
          check(true, label);
          return state;
        }
      }
    } catch {
      // 還在啟動或暫時連不上。
    }
    await sleep(300);
  }
  check(false, `逾時（${timeoutMs} ms）：${label}`);
  return null;
}

async function fetchApiState(port) {
  const r = await fetch(`http://127.0.0.1:${port}/api/state`);
  return r.ok ? r.json() : null;
}

// ---------------------------------------------------------------------------
// probe_pane_read（唯讀，只送 session.snapshot／pane.read；驗 stderr 的 pane_not_found）
// ---------------------------------------------------------------------------

function runProbePaneReadNotFound(paneId) {
  if (!fs.existsSync(PROBE_PANE_READ_EXE)) {
    throw new Error(
      `找不到 ${PROBE_PANE_READ_EXE}，請先跑 cargo build -p herdr-client --example probe_pane_read`
    );
  }
  const r = spawnSync(
    PROBE_PANE_READ_EXE,
    ["--wsl", WSL_DISTRO, WSL_SOCKET, "--pane", paneId, "--count", "1", "--metadata-only"],
    { encoding: "utf8", timeout: 15000 }
  );
  return { stdout: r.stdout || "", stderr: r.stderr || "", status: r.status };
}

// ---------------------------------------------------------------------------
// 面板文字解析
// ---------------------------------------------------------------------------

function parseMaxTick(text) {
  const re = /tick (\d+)/g;
  let max = -1;
  let m;
  while ((m = re.exec(text || ""))) {
    const n = Number(m[1]);
    if (n > max) max = n;
  }
  return max;
}

async function sampleMaxTick(cdp, durationMs, intervalMs) {
  const start = Date.now();
  const samples = [];
  while (Date.now() - start < durationMs) {
    const text = await cdp.eval("document.querySelector('.output-text').textContent");
    samples.push({ t: Date.now(), max: parseMaxTick(text) });
    await sleep(intervalMs);
  }
  return samples;
}

// 「內容跟上」的等價判準：沿用 live-output-check.js `findStalls()` 的理由（見該檔第 F 段上方
// 註解）——pane 每秒確定性地多印一行，因此「每一行在產生後 3 秒內出現在面板」等價於「任何長度
// 達 windowMs（3000）的觀察視窗內，面板顯示的最大行號至少前進一次」。
function findStalls(samples, windowMs) {
  const violations = [];
  for (let i = 0; i < samples.length; i += 1) {
    let j = -1;
    for (let k = i + 1; k < samples.length; k += 1) {
      if (samples[k].t - samples[i].t >= windowMs) {
        j = k;
        break;
      }
    }
    if (j === -1) continue;
    if (samples[j].max <= samples[i].max) {
      violations.push({ fromT: samples[i].t, fromMax: samples[i].max, toT: samples[j].t, toMax: samples[j].max });
    }
  }
  return violations;
}

// ---------------------------------------------------------------------------
// 主流程
// ---------------------------------------------------------------------------

let r18Observation = null; // main() 設定，供最後的摘要輸出使用（brief 要求如實記錄 (a)／(b)）。

// finding 3：寫入 opt-in 檢查已經搬到模組頂層的 `assertWslWriteOptIn()`（在挑選要跑 `main()`
// 還是 `--verify-ownership-mismatch` 之前，兩種模式共用同一個入口），這裡不再重複檢查——
// `main()` 被呼叫到的時候，已經保證 `HERDR_CLIENT_TEST_ALLOW_WSL_WRITES=1` 成立。
async function main() {

  // F1：distro 必須在送出任何 WSL 請求之前解析好。換路（round 3）：不再需要「拒絕執行」檢查
  // （專屬 socket 路徑不可能有別人的 server，見檔頭「換路」段落）——取代它的是
  // `ensureDedicatedSocketPathIsFree()`，這裡什麼都還沒建立（沒有 tab、沒有啟動 cockpit），
  // 碰到極端的 run id 碰撞而重試失敗會直接 throw（頂層 `.catch()` 會印例外並非 0 結束）。
  resolveWslEndpoint();
  log(`WSL 端點：distro=${WSL_DISTRO} socket=${WSL_SOCKET}`);
  ensureDedicatedSocketPathIsFree();

  log("=== live-output task 6.1：Scenario E 真機驗收（WSL 測試 server、真 cockpit、headless Chrome）===");

  const port = pickPort(7792);
  log(`使用埠 ${port}（127.0.0.1，避開已佔用的埠）`);

  let wslServerStarted = false;
  // finding A：目前這次執行「擁有」的 herdr server PID（來自 pidfile，`exec` 之後就是 server
  // 本身的 PID）；`null` 代表目前沒有已驗證的所有權，停止邏輯遇到 `null` 一律不停（見
  // `stopWslServerIfOwned()`）。
  let ownedPid = null;
  // finding A（round 2）：`ownedPid` 的完整憑證，多記 starttime／comm（見
  // `establishOwnershipIdentity()`）；跟 `ownedPid` 一起變化——確立所有權時一起設，停止確認
  // 成功後一起清空為 `null`。停止邏輯遇到 `null` 一律不停。
  let ownedIdentity = null;
  let currentTabId = null;
  let currentPaneId = null;
  let closedPaneId = null; // 步驟 6 關掉的那個 pane，供步驟 7 用
  let cockpit = null;
  let chrome = null;
  let configHandle = null;
  // F1：本次執行明確 tab.create 過的所有 tab id（含 R18 重啟後重建的），收尾時逐一直接關閉。
  const createdTabIds = [];
  // F1：啟動 WSL 測試 server 後、建任何 tab 前的既有 tab id 集合；收尾兜底掃描只認「label 相符
  // 本次 run id 且不在這個集合內」的 tab，避免誤關不是這次執行建立的 tab。
  let baselineTabIds = [];

  try {
    // --- 步驟 1：啟動 WSL 測試 server → 建測試 tab → 送迴圈 ---
    log("--- 步驟 1：啟動 WSL 測試 server ---");
    startWslServer();
    wslServerStarted = true;
    const upOk = await waitForWslServerUp(
      30000,
      "WSL 測試 server 在 30 秒內起來（socket 出現且 session.snapshot 有回應）"
    );
    if (!upOk) {
      throw new Error("WSL 測試 server 沒有在時限內起來，中止（見上面的 FAIL，收尾仍會照跑）");
    }
    // finding A：啟動後才算數——讀回 pidfile 的 PID，並確認「這個 PID 確實是 herdr、且本次執行
    // 的專屬 socket 連得上」（owned，round 3 改成只看這個 PID 自己，見 `verifyOwnership()`）；
    // `waitForWslServerUp()` 的回傳值已經被檢查過，不是被忽略的死值。
    // finding 2：`verifyOwnership()` 的回傳值不能只印出來就算了——只有 `readOwnedPid()` 非 null
    // 且 `verifyOwnership()` 回 true 才把值指派給模組層級的 `ownedPid` 並繼續；任何一個條件不
    // 成立就立刻 throw，`ownedPid` 保持 `null`（收尾時 `stopWslServerIfOwned(null, ...)` 會直接
    // 跳過，不會對任何 server 動手）。finding A（round 2）：`verifyOwnership()` 通過只確立 PID
    // 這一層，`establishOwnershipIdentity()` 再多讀一次 starttime／comm 當完整憑證——這一步也失敗
    // 就跟 PID 這層沒過一樣嚴重，一律 throw。
    const startPid = readOwnedPid();
    check(startPid !== null, `讀到 pidfile（${WSL_PID_FILE}）記下的 PID（實際 ${JSON.stringify(startPid)}）`);
    const startIdentity =
      startPid !== null &&
      establishOwnershipIdentity(startPid, "啟動後確認 pidfile 記下的 PID 是 herdr、且專屬 socket 連得上（owned）");
    if (!startIdentity) {
      throw new Error(
        "finding 2：偵測到不是本次啟動的 server，未接管、未停止（啟動後 pidfile 讀取、所有權驗證或" +
          `身分憑證讀取未通過，pidfile PID=${JSON.stringify(startPid)}）。中止，ownedPid 仍為 null，` +
          "收尾不會對任何 server 動手。"
      );
    }
    ownedPid = startPid;
    ownedIdentity = startIdentity;
    log(`ownedPid=${ownedPid}（comm=${ownedIdentity.comm} starttime=${ownedIdentity.starttime}）`);
    baselineTabIds = listTabs().map((t) => t.tab_id);
    log(`baseline tab（專屬 socket）：${baselineTabIds.length} 個 ${JSON.stringify(baselineTabIds)}`);

    log(`--- 建測試 tab（label ${TAB_LABEL}）並送每秒印一行的迴圈 ---`);
    const created = createTestTabAndLoop();
    currentTabId = created.tabId;
    currentPaneId = created.paneId;
    createdTabIds.push(currentTabId);

    // --- 步驟 2：啟動 cockpit → headless Chrome → 等測試 pane 出現 → 點該列 ---
    log("--- 步驟 2：啟動 cockpit（驗收用設定） ---");
    configHandle = writeCockpitConfig(port);
    cockpit = await startCockpit(configHandle.configPath, port);

    await waitForRuntimeConnected(
      port,
      30000,
      `runtime ${RUNTIME_ID} 在 30 秒內變成 connected（/api/state）`
    );

    const url = `http://127.0.0.1:${port}/`;
    chrome = await startChrome(pickPort(18960, [port]), url, "chrome-real");
    const { cdp } = chrome;
    await cdp.waitFor(
      "typeof window.liveOutput === 'object' && typeof window.liveOutput.select === 'function'",
      10000,
      "output.js 載入完成，window.liveOutput 就緒"
    );

    const paneSel = `.pane-row[data-runtime="${RUNTIME_ID}"][data-pane="${currentPaneId}"]`;
    // WSL 0.8.2 對新 events.subscribe 重播歷史（handover 第 4 節）：連線後沉降最多約 5 s 投影
    // 可能不準；等到投影裡真的看得到測試 pane（含沉降時間）再開始點選，逾時給寬（30 s）。
    await cdp.waitFor(
      `!!document.querySelector(${JSON.stringify(paneSel)})`,
      30000,
      `測試 pane ${currentPaneId} 的列出現在畫面上`
    );
    await cdp.click(paneSel);
    await cdp.waitFor(
      `(() => { const r = document.querySelector(${JSON.stringify(paneSel)}); return !!r && r.classList.contains('selected'); })()`,
      5000,
      "點選之後該列出現選定標示"
    );
    const title = await cdp.eval("document.querySelector('.output-title').textContent");
    check(
      title === `${RUNTIME_ID} / ${currentPaneId}`,
      `面板標題應該顯示 ${RUNTIME_ID} / ${currentPaneId}（實際 ${JSON.stringify(title)}）`
    );
    await cdp.waitFor(
      "(() => { const t = document.querySelector('.output-text'); return !!t && t.textContent.indexOf('tick') !== -1; })()",
      10000,
      "面板出現含 tick 的內容"
    );

    // --- 步驟 3：內容跟上（3 秒門檻，觀察 >= 10 秒） ---
    log("--- 步驟 3：內容跟上（觀察 10 秒，任何連續 3 秒視窗內最大 tick N 至少前進一次）---");
    const samples = await sampleMaxTick(cdp, 10000, 250);
    log(`取樣點數：${samples.length}；最大 tick 序列：${JSON.stringify(samples.map((s) => s.max))}`);
    const violations = findStalls(samples, 3000);
    check(
      violations.length === 0,
      `任何 >=3 秒的觀察視窗內面板最大 tick 都應該至少前進一次（實際違規 ${violations.length} 個：${JSON.stringify(violations)}）`
    );

    // --- 步驟 4：端點直接驗（curl） ---
    log("--- 步驟 4：端點直接驗（curl，Host 檢查）---");
    const outputPath = `/api/runtimes/${encodeURIComponent(RUNTIME_ID)}/panes/${encodeURIComponent(currentPaneId)}/output`;
    const okResp = curlGet(`http://127.0.0.1:${port}${outputPath}`, `127.0.0.1:${port}`);
    check(okResp.status === 200, `合法 Host 應該回 200（實際 ${okResp.status}）`);
    check(
      okResp.body.indexOf("tick") !== -1,
      `回應本體 text 應該含 tick（實際節錄 ${JSON.stringify(okResp.body.slice(0, 200))}）`
    );
    const evilResp = curlGet(`http://127.0.0.1:${port}${outputPath}`, `evil.example:${port}`);
    check(evilResp.status === 403, `Host: evil.example:${port} 應該回 403（實際 ${evilResp.status}）`);

    // --- 步驟 5（R18）：runtime 斷線後恢復 ---
    log("--- 步驟 5（R18）：runtime 斷線後恢復 ---");
    log(">>> 停 WSL 測試 server <<<");
    // finding A（round 2）：停止動作本身（核對身分 → 送 SIGTERM → 等待）已經收進
    // `stopWslServerIfOwned()` 內單一 wsl.exe 呼叫，這裡不再另外做一次獨立的比對（符合 brief
    // 驗證 (i)「輸出中看得到 ownedPid、starttime、每次停止的結果標記」）。
    const stopAttempted = stopWslServerIfOwned(ownedPid, ownedIdentity, "R18 中途停 server 前");
    check(stopAttempted, `R18 中途停 server 前身分核對通過，對 PID ${ownedPid} 送 SIGTERM 並確認已結束`);
    // F1／finding A：旗標要跟著「這次執行實際啟停的狀態」走——`stopWslServerIfOwned()` 回傳
    // `true` 就代表 `killAndWaitPid()` 已經在同一次呼叫裡確認這個 PID 結束了（`GONE`／
    // `KILLED_AFTER_TIMEOUT`），`ownedPid`／`ownedIdentity` 這時就該清空，不等外部再做一次
    // 檢查——重啟後會重新讀一次新的 PID／身分，不該讓收尾誤以為舊憑證還算數。
    if (stopAttempted) {
      wslServerStarted = false;
      ownedPid = null;
      ownedIdentity = null;
    }
    // finding B（round 2）：不再斷言「socket 消失」——這裡改成額外確認「沒有任何 herdr 行程、
    // socket 也連不上」，作為這次停止的環境層驗證（不影響上面 `ownedPid` 的清空邏輯）。
    await waitForWslServerDown(15000, "WSL 測試 server 在 15 秒內停止（沒有 herdr 程序、socket 連不上）");

    await cdp.waitFor(
      "document.getElementById('output').classList.contains('is-stale')",
      15000,
      "停 server 後面板在時限內標為過期（is-stale）"
    );
    const staleOpacity = await cdp.eval(
      "getComputedStyle(document.querySelector('.output-text')).opacity"
    );
    check(staleOpacity !== "1", `過期期間 opacity 應該不是 1（實際 ${staleOpacity}）`);
    await cdp.waitFor(
      "!document.querySelector('.output-error-reason').hidden",
      5000,
      "過期期間顯示失敗原因（.output-error-reason 可見）"
    );
    const reasonText = await cdp.eval("document.querySelector('.output-error-reason').textContent");
    check(
      typeof reasonText === "string" && reasonText.length > 0,
      `失敗原因文字應該非空（實際 ${JSON.stringify(reasonText)}）`
    );
    log(`503 期間顯示的原因文字：${JSON.stringify(reasonText)}`);
    check(
      (await cdp.eval("document.querySelector('.output-gone-notice').hidden")) === true,
      "503（runtime 斷線）不是「pane 已不存在」，gone 提示應該仍隱藏"
    );

    // 持續重試：用 Network 事件在 5 秒觀察窗內數 /output 請求次數，>=2 次代表仍在依節奏輪詢。
    await cdp.send("Network.enable");
    const retryRequests = [];
    cdp.onEvent("Network.requestWillBeSent", (p) => {
      if (/\/panes\/.+\/output(\?|$)/.test(p.request.url)) retryRequests.push(p.timestamp);
    });
    await sleep(5000);
    check(
      retryRequests.length >= 2,
      `斷線期間應該依節奏繼續重試（5 秒內 >=2 次 /output 請求，實際 ${retryRequests.length} 次）`
    );

    log(">>> 重啟 WSL 測試 server <<<");
    // 換路（round 3）：不再需要重啟前的「拒絕執行」檢查——沿用同一個專屬 socket 路徑
    // （`WSL_SOCKET` 沒變，Cockpit 的設定本來就指向它），這個路徑不可能有別人的 server（見檔頭
    // 「換路」段落）。舊版 finding A 第 2 點的重做拒絕檢查已經移除。
    startWslServer();
    wslServerStarted = true;
    const restartUpOk = await waitForWslServerUp(
      30000,
      "重啟後 WSL 測試 server 在 30 秒內恢復（socket 出現且 session.snapshot 有回應）"
    );
    if (!restartUpOk) {
      throw new Error("重啟後 WSL 測試 server 沒有在時限內恢復，中止（見上面的 FAIL，收尾仍會照跑）");
    }
    // finding A：重啟後是全新的程序，pidfile 也是重新寫的（同一個 `WSL_PID_FILE` 路徑，內容
    // 換成新的 PID）——重新讀回並重新驗證 owned，不能沿用重啟前的 `ownedPid`。
    // finding 2：跟初次啟動同一套規則——`readOwnedPid()` 非 null 且 `establishOwnershipIdentity()`
    // 回傳非 null 才把值指派給 `ownedPid`／`ownedIdentity` 並繼續，否則立刻 throw（兩者這時仍是
    // 重啟前停下來後設的 `null`，收尾不會誤動任何 server）。finding A（round 2）：重啟後的 PID
    // 幾乎必然是新的 starttime，重新讀一次憑證，不能沿用重啟前的 `ownedIdentity`。
    const restartPid = readOwnedPid();
    check(
      restartPid !== null,
      `重啟後讀到 pidfile（${WSL_PID_FILE}）記下的 PID（實際 ${JSON.stringify(restartPid)}）`
    );
    const restartIdentity =
      restartPid !== null &&
      establishOwnershipIdentity(restartPid, "重啟後確認 pidfile 記下的 PID 是 herdr、且專屬 socket 連得上（owned）");
    if (!restartIdentity) {
      throw new Error(
        "finding 2：偵測到不是本次啟動的 server，未接管、未停止（重啟後 pidfile 讀取、所有權驗證或" +
          `身分憑證讀取未通過，pidfile PID=${JSON.stringify(restartPid)}）。中止，ownedPid 仍為 null，` +
          "收尾不會對任何 server 動手。"
      );
    }
    ownedPid = restartPid;
    ownedIdentity = restartIdentity;
    log(`重啟後 ownedPid=${ownedPid}（comm=${ownedIdentity.comm} starttime=${ownedIdentity.starttime}）`);

    // 觀察窗：面板要嘛恢復（(a) 同一 pane 仍在），要嘛顯示「pane 已不存在」（(b)）。兩者都是
    // 合法結果，逾時放寬（30 s，涵蓋重連退避＋WSL 端沉降時間）。
    const R18_TIMEOUT_MS = 30000;
    const r18Start = Date.now();
    let r18Outcome = null;
    while (Date.now() - r18Start < R18_TIMEOUT_MS && r18Outcome === null) {
      const staleNow = await cdp.eval("document.getElementById('output').classList.contains('is-stale')");
      const goneHidden = await cdp.eval("document.querySelector('.output-gone-notice').hidden");
      if (goneHidden === false) {
        r18Outcome = "b";
        break;
      }
      if (staleNow === false) {
        r18Outcome = "a";
        break;
      }
      await sleep(300);
    }
    check(r18Outcome !== null, `逾時（${R18_TIMEOUT_MS} ms）：重啟後應該恢復成 (a) 或顯示 (b)「pane 已不存在」`);
    r18Observation = r18Outcome;
    log(`R18 觀察結果：${r18Outcome === "a" ? "(a) 同一 pane 仍在，恢復" : r18Outcome === "b" ? "(b) pane 已不存在" : "未判定"}`);

    if (r18Outcome === "a") {
      const goneHiddenAfter = await cdp.eval("document.querySelector('.output-gone-notice').hidden");
      check(goneHiddenAfter === true, "(a) 恢復後 gone 提示應該仍隱藏");
      const reasonHiddenAfter = await cdp.eval("document.querySelector('.output-error-reason').hidden");
      check(reasonHiddenAfter === true, "(a) 恢復後失敗原因應該消失");
      const opacityAfter = await cdp.eval(
        "getComputedStyle(document.querySelector('.output-text')).opacity"
      );
      check(opacityAfter === "1", `(a) 恢復後 opacity 應該變回 1（實際 ${opacityAfter}）`);
      const textAfter = await cdp.eval("document.querySelector('.output-text').textContent");
      check(
        typeof textAfter === "string" && textAfter.length > 0,
        `(a) 恢復後面板應該已經讀到真的內容（非空；實際長度 ${textAfter ? textAfter.length : "n/a"}）`
      );
      // 真機觀察（詳見 acceptance.md）：WSL 測試 server 重啟後，pane id 雖然保留（走到這個 (a)
      // 分支），但原本執行中的 tick 迴圈行程被中斷、pane 內容重置成一個新的 shell prompt——這是
      // WSL 測試 server 重啟本身的行為，不是 Cockpit 的缺陷（run 1 實測：opacity／gone／reason
      // 三者都正確恢復，只是 textAfter 是空 prompt，不含 tick，直接斷言含 tick 會誤判成失敗）。
      // 改用「對同一個 pane 重新送一次迴圈指令，確認輪詢會把新內容顯示出來」驗證 spec 用字
      // 「內容更新」：不是假設舊迴圈還在跑，而是主動證明恢復後的輪詢確實反映當下真實的 pane 內容。
      log("--- (a) 對同一個 pane 重新送一次 tick 迴圈，驗證恢復後輪詢確實反映真實內容 ---");
      wslRpc("resend", "pane.send_text", { pane_id: currentPaneId, text: LOOP_SCRIPT });
      await cdp.waitFor(
        "(() => { const t = document.querySelector('.output-text'); return !!t && t.textContent.indexOf('tick') !== -1; })()",
        10000,
        "(a) 對同一個 pane 重新送迴圈後，面板在 10 秒內出現含 tick 的內容（證明恢復後輪詢確實反映真實 pane 內容）"
      );
    } else if (r18Outcome === "b") {
      const goneText = await cdp.eval("document.querySelector('.output-gone-notice').textContent");
      check(goneText === "pane 已不存在", `(b)「pane 已不存在」文字應該逐字一致（實際 ${JSON.stringify(goneText)}）`);
    }

    // brief 備註：重啟後若要繼續後面步驟，需要重新建測試 tab（不管 (a)／(b)，用一個乾淨的新
    // pane 走步驟 6／7，避免舊 pane 在 (a) 分支下仍存在、混淆「pane 被關掉」的驗證）。
    log("--- 重啟後重新建一個測試 tab，供步驟 6／7 使用 ---");
    await cdp.eval("window.liveOutput.clear(); true");
    const recreated = createTestTabAndLoop();
    currentTabId = recreated.tabId;
    currentPaneId = recreated.paneId;
    createdTabIds.push(currentTabId);

    const paneSel2 = `.pane-row[data-runtime="${RUNTIME_ID}"][data-pane="${currentPaneId}"]`;
    await cdp.waitFor(
      `!!document.querySelector(${JSON.stringify(paneSel2)})`,
      30000,
      `重建後測試 pane ${currentPaneId} 的列出現在畫面上`
    );
    await cdp.click(paneSel2);
    await cdp.waitFor(
      `(() => { const t = document.querySelector('.output-text'); return !!t && t.textContent.indexOf('tick') !== -1; })()`,
      10000,
      "重新選取後面板出現含 tick 的內容"
    );

    // --- 步驟 6：pane 被關掉 ---
    log("--- 步驟 6：pane 被關掉（tab.close）---");
    const outputReqsBeforeClose = [];
    cdp.onEvent("Network.requestWillBeSent", (p) => {
      if (/\/panes\/.+\/output(\?|$)/.test(p.request.url)) outputReqsBeforeClose.push(p.timestamp);
    });
    check(closeTab(currentTabId), `tab.close(${currentTabId}) 回 ok`);
    closedPaneId = currentPaneId;

    await cdp.waitFor(
      "!document.querySelector('.output-gone-notice').hidden",
      15000,
      "關掉 tab 後面板顯示「pane 已不存在」"
    );
    const goneTextClose = await cdp.eval("document.querySelector('.output-gone-notice').textContent");
    check(
      goneTextClose === "pane 已不存在",
      `「pane 已不存在」文字應該逐字一致（實際 ${JSON.stringify(goneTextClose)}）`
    );

    // 停止輪詢：先讓已經在飛的請求落地（1.5 s），清空計數，再開一段 5 秒的乾淨觀察視窗。
    await sleep(1500);
    const countBeforeWindow = outputReqsBeforeClose.length;
    await sleep(5000);
    const countAfterWindow = outputReqsBeforeClose.length;
    check(
      countAfterWindow === countBeforeWindow,
      `顯示「pane 已不存在」之後 5 秒內不應該再有新的 /output 請求（實際 ${countBeforeWindow} → ${countAfterWindow}）`
    );

    // --- 步驟 7：不存在的 pane ---
    log("--- 步驟 7：不存在的 pane（probe_pane_read 唯讀 + curl 端點）---");
    const probe = runProbePaneReadNotFound(closedPaneId);
    const stderrHasCode = probe.stderr.indexOf("pane_not_found") !== -1;
    check(
      stderrHasCode,
      `probe_pane_read 的 stderr 應該含錯誤碼 pane_not_found（實際 stderr 節錄 ${JSON.stringify(probe.stderr.slice(0, 400))}）`
    );
    log(`probe_pane_read stderr：${probe.stderr.trim()}`);

    const notFoundPath = `/api/runtimes/${encodeURIComponent(RUNTIME_ID)}/panes/${encodeURIComponent(closedPaneId)}/output`;
    const notFoundResp = curlGet(`http://127.0.0.1:${port}${notFoundPath}`, `127.0.0.1:${port}`);
    check(
      notFoundResp.status === 404,
      `已關掉的 pane 應該回 404（不是 503；實際 ${notFoundResp.status}，body 節錄 ${JSON.stringify(notFoundResp.body.slice(0, 200))}）`
    );

    return failures.length === 0 ? 0 : 1;
  } finally {
    // --- 步驟 8：收尾（無論成功失敗都做）---
    log("--- 步驟 8：收尾 ---");

    await stopChrome(chrome, "chrome-real");

    if (cockpit && cockpit.child) {
      killTree(cockpit.child, "cockpit");
      await sleep(300);
      check(!isPortListening(port), `port ${port}（cockpit）應該不再有 LISTENING 的行程`);
    }

    // F1：關掉本次執行建立過的 tab，分兩層——(1) 直接關本次明確 tab.create 過的每一個
    // （`createdTabIds`，含 R18 重啟後重建的那個；其中步驟 6 已經主動關過的那個再關一次預期會
    // 失敗，忽略即可）；(2) 兜底掃一次「label 相符本次 run id 且不在啟動時 baseline 內」的殘留
    // tab，只補 (1) 沒抓到的（例如中途失敗、還沒走到 createdTabIds.push 那一行就死掉的情況），
    // 兩層都不會動到不屬於這次執行的 tab（baseline 排除法＋label 帶隨機 run id）。
    if (wslServerStarted) {
      try {
        // finding B（round 2）：這裡要問的是「server 還連得上嗎（能不能送 tab.close RPC）」，
        // 不是「socket 檔存不存在」——改用 `wslSocketConnectable()`。
        if (wslSocketConnectable()) {
          for (const tid of createdTabIds) {
            try {
              const ok = closeTab(tid);
              log(`收尾 tab.close（本次建立）${tid} → ${ok ? "ok" : "已經不在或失敗，略過"}`);
            } catch (e) {
              log(`收尾 tab.close（本次建立）${tid} 例外（可能已經關過）：${e.message}`);
            }
          }

          const baselineSet = new Set(baselineTabIds);
          const strays = listTabs()
            .filter((t) => t.label === TAB_LABEL && !baselineSet.has(t.tab_id))
            .map((t) => t.tab_id);
          for (const tid of strays) {
            try {
              const ok = closeTab(tid);
              log(`收尾 tab.close（兜底：label 相符且不在 baseline）${tid} → ${ok ? "ok" : "失敗"}`);
            } catch (e) {
              log(`收尾 tab.close（兜底）${tid} 例外：${e.message}`);
            }
          }
        }
      } catch (e) {
        log(`收尾列出殘留 tab 時例外：${e.message}`);
      }

      // finding A（round 2）：收尾停止一樣交給 `stopWslServerIfOwned()` 在單一 wsl.exe 呼叫內
      // 核對身分（PID＋starttime＋comm）→ 送 SIGTERM → 等待；不相符（或 ownedPid／ownedIdentity
      // 已經是 null）就不停、印警告，絕不用 `wslServerStarted` 這個布林旗標單獨授權動手。
      const stoppedAtCleanup = stopWslServerIfOwned(ownedPid, ownedIdentity, "收尾");
      check(
        stoppedAtCleanup,
        `收尾身分核對通過，對 PID ${ownedPid} 送 SIGTERM 並確認已結束`
      );
      wslServerStarted = false;
      if (stoppedAtCleanup) {
        // 換路（round 3）：不再斷言「沒有 herdr 程序」——改斷言「我們自己的專屬 socket 連不上」
        // （見 `waitForWslServerDown()`）。
        await waitForWslServerDown(15000, "收尾：WSL 測試 server 在 15 秒內停止（專屬 socket 連不上）");
      }
    }

    // 換路（round 3）：刪除本次執行專屬的 pidfile 與 socket 檔（純 argv，`rm -f` 對不存在的檔案
    // 不會報錯；用 `wsl.exe -e rm -f <path>`，不經過 bash）——round 2（finding B）當時因為這個
    // 路徑是共用的，刪除可能誤刪別人剛綁定的新 socket，所以整支腳本刻意不刪任何 socket 檔；
    // round 3 的這兩個路徑只屬於本次執行，刪除不會影響任何人。刪之前再確認一次路徑符合
    // `/tmp/cockpit-accept-` 前綴與既有白名單字元集（斷言式第二層防禦——理論上一定成立，因為
    // 兩者都是模組載入時由 RUN_ID 產生，不是外部輸入）；不符合就跳過刪除並記錄，不冒然對不
    // 認得的路徑下手。不論最後有沒有真的執行過 stop 都嘗試刪，避免留下過期檔案佔用 `/tmp`。
    if (WSL_DISTRO) {
      const OWN_RUN_PATH_PREFIX = "/tmp/cockpit-accept-";
      for (const p of [WSL_PID_FILE, WSL_SOCKET]) {
        if (!p.startsWith(OWN_RUN_PATH_PREFIX) || !SOCKET_PATTERN.test(p)) {
          log(`收尾：跳過刪除 ${p}——不符合本次執行專屬路徑的前綴／白名單（不應該發生，防禦性檢查）。`);
          continue;
        }
        try {
          spawnSync("wsl.exe", ["-d", WSL_DISTRO, "-e", "rm", "-f", p], {
            encoding: "utf8",
            timeout: 5000,
          });
        } catch {
          // 收尾動作，失敗不影響整體結果；下次執行用的是不同 run id 的檔名，不會撞名。
        }
      }
      check(
        !wslPathExists(WSL_SOCKET),
        `收尾：本次執行專屬的 socket 檔 ${WSL_SOCKET} 已刪除（不存在）`
      );
    }

    if (configHandle) {
      try {
        fs.rmSync(configHandle.dir, { recursive: true, force: true });
        check(!fs.existsSync(configHandle.dir), "暫存設定目錄已刪除");
      } catch (e) {
        check(false, `刪除暫存設定目錄失敗：${e.message}`);
      }
    }
    try {
      fs.rmSync(CURL_SCRATCH, { recursive: true, force: true });
    } catch {
      // 忽略。
    }
  }
}

// ---------------------------------------------------------------------------
// finding A 驗證 (iii) 專用：所有權不相符時 stop 邏輯不誤停
// ---------------------------------------------------------------------------
//
// `node live-output-real-check.js --verify-ownership-mismatch` 執行；獨立、輕量，不跑 main() 的
// 其餘 7 個步驟（不建測試 tab、不啟動 cockpit／Chrome）——只驗證 finding A（round 2）第 4 點這一
// 條規則本身：啟動一個真的 WSL 測試 server、確立正常情況下的所有權憑證（真實 PID＋starttime／
// comm），再餵一個**同一個真實 PID、但 starttime 不相符**的假憑證（brief 明講：用不相符的
// starttime，比上一輪「換成不存在的 PID」更貼近 finding A 真正要防的情境——PID 被重用之後，
// 新舊行程的 PID 相同但 starttime 一定不同），確認 `stopWslServerIfOwned()` 因為身分核對沒過而
// 拒絕動手、印警告、不送出任何 signal，並且真實的 server 行程真的還在跑、身分（starttime／
// comm）完全沒變過。保留成有文件的測試掛勾（不是一次性程式碼）：之後要回歸驗證這條規則，直接
// 重跑這個模式即可。finding 3：這個模式跟 `main()` 一樣，執行前都會先經過模組頂層的
// `assertWslWriteOptIn()`——沒有 `HERDR_CLIENT_TEST_ALLOW_WSL_WRITES=1` 連這個函式都不會被
// 呼叫到。
//
// 刻意**不**自動收尾（brief finding A 驗證 (iii)：驗完「你自己手動停掉 server」）——這支函式
// 驗的正是「不該自動停」這件事，讓它自己把剛啟動的 server 停掉會混淆這個示範本身要證明的東西；
// 收尾交給驗證者自己手動執行（指令見 report）。
async function verifyOwnershipMismatchDoesNotStop() {
  resolveWslEndpoint();
  log(`WSL 端點：distro=${WSL_DISTRO} socket=${WSL_SOCKET}`);
  ensureDedicatedSocketPathIsFree();

  log("--- 啟動 WSL 測試 server，準備驗證「所有權不相符不誤停」---");
  startWslServer();
  const upOk = await waitForWslServerUp(30000, "WSL 測試 server 在 30 秒內起來");
  if (!upOk) return 1;

  const realOwnedPid = readOwnedPid();
  check(realOwnedPid !== null, `讀到 pidfile 記下的真實 PID（實際 ${JSON.stringify(realOwnedPid)}）`);
  const realIdentity =
    realOwnedPid !== null &&
    establishOwnershipIdentity(
      realOwnedPid,
      "前提檢查：啟動後確認 pidfile 記下的 PID 是 herdr、且專屬 socket 連得上（owned，真實值）"
    );
  if (!realIdentity) {
    log(
      "前提檢查失敗：真實所有權（或身分憑證）比對本身就不成立，無法繼續驗證「不相符不誤停」——請" +
        "檢查 startWslServer()／readOwnedPid()／getProcessIdentity() 是否正確；請自己手動執行 " +
        "herdr server stop 收尾後再重跑。"
    );
    return 1;
  }

  // finding A（round 2）：只換 starttime（`FAKE_STARTTIME_FOR_MISMATCH_TEST`，一個真實系統開機
  // tick 數不可能剛好等於的極小值），comm 沿用真實值——這是「同一個 PID、但身分核對就是不會通過」
  // 的最貼近情境，直接命中 `killAndWaitPid()` 內 `check_identity()` 的 starttime 比對分支。
  const fakeIdentity = { comm: realIdentity.comm, starttime: FAKE_STARTTIME_FOR_MISMATCH_TEST };
  log(
    `[除錯掛勾] 對真實 PID=${realOwnedPid} 使用不相符的 starttime（真實值 ${realIdentity.starttime}` +
      ` 換成 ${fakeIdentity.starttime}），模擬「同一個 PID 但身分不符」`
  );
  const stopped = stopWslServerIfOwned(
    realOwnedPid,
    fakeIdentity,
    "驗證 (iii)：所有權不相符時嘗試 stop"
  );
  check(!stopped, "所有權不相符時 stopWslServerIfOwned() 應該回傳 false（沒有對任何 PID 送 SIGTERM）");

  const stillConnectable = wslSocketConnectable();
  check(stillConnectable, "所有權不相符不誤停之後，WSL 測試 server 應該仍然連得上（沒被停掉）");
  const identityAfter = getProcessIdentity(realOwnedPid);
  const identityUnchanged =
    identityAfter !== null &&
    identityAfter.comm === realIdentity.comm &&
    identityAfter.starttime === realIdentity.starttime;
  check(
    identityUnchanged,
    "所有權不相符不誤停之後，真實 PID 的身分（comm／starttime）應該完全沒變過（證明真的沒有收到" +
      `任何 signal；實際 ${JSON.stringify(identityAfter)}，原始 ${JSON.stringify(realIdentity)}）`
  );

  console.log(
    `\n驗證完成：所有權不相符時沒有誤停 WSL 測試 server（真實 PID=${realOwnedPid}，模擬的錯誤 ` +
      `starttime=${fakeIdentity.starttime}）。這支腳本不會自動收尾——請自己手動收掉剛剛啟動的測試 ` +
      `server（換路後這個 server 綁在本次執行的專屬 socket 路徑，不是預設路徑，全域 ` +
      "`herdr server stop` 不會生效）：對 WSL 執行 " +
      `\`wsl.exe -d ${WSL_DISTRO} -e kill -TERM ${realOwnedPid}\`（或用其他方式對這個 PID 送 ` +
      "SIGTERM），確認結束後再視需要手動刪除殘留檔：" +
      `\`wsl.exe -d ${WSL_DISTRO} -e rm -f ${WSL_PID_FILE} ${WSL_SOCKET}\`（這兩個路徑只屬於` +
      "本次執行，刪除不會影響任何人）。"
  );
  return stillConnectable && identityUnchanged && failures.length === 0 ? 0 : 1;
}

// finding 3：寫入 opt-in 檢查搬到這裡——選擇要跑哪個 entry point「之前」的共用入口，任何模式
// （目前是 `main()` 與 `--verify-ownership-mismatch`；以後新增的模式也要一律經過這裡）都逃不掉。
// 先前的版本只在 `main()` 裡檢查，`--verify-ownership-mismatch` 完全沒有這道防線，帶著這個旗標
// 執行就會直接對 WSL 端寫入、啟動 server，繞過了「沒有這個環境變數就不對 WSL 送出任何請求」的
// 保證。這裡 fail closed：沒有 `HERDR_CLIENT_TEST_ALLOW_WSL_WRITES=1` 就同步印出訊息並
// `process.exit(0)`，兩個 entry point 函式都不會被呼叫到，WSL 端不會收到任何請求。
function assertWslWriteOptIn() {
  if (process.env.HERDR_CLIENT_TEST_ALLOW_WSL_WRITES !== "1") {
    console.log(
      "未設定 HERDR_CLIENT_TEST_ALLOW_WSL_WRITES=1：不對 WSL 端寫入或啟停任何 HERDR server，直接" +
        "結束（唯讀，比照 live-state-check.py 的自我把關）。finding 3：這個檢查是所有模式共用的" +
        "入口，任何模式都不能繞過。"
    );
    process.exit(0);
  }
}
assertWslWriteOptIn();

const VERIFY_OWNERSHIP_MISMATCH_MODE = process.argv.includes("--verify-ownership-mismatch");
const entry = VERIFY_OWNERSHIP_MISMATCH_MODE ? verifyOwnershipMismatchDoesNotStop() : main();

entry
  .then((code) => {
    if (!VERIFY_OWNERSHIP_MISMATCH_MODE) {
      console.log(
        `\nR18 觀察：${r18Observation === "a" ? "(a) 同一 pane 仍在，恢復" : r18Observation === "b" ? "(b) pane 已不存在" : String(r18Observation)}`
      );
    }
    console.log(failures.length === 0 ? "RESULT: PASS" : `RESULT: FAIL (${failures.length})`);
    process.exit(code ?? (failures.length === 0 ? 0 : 1));
  })
  .catch((e) => {
    console.error("未預期的例外：", e);
    console.log(`RESULT: FAIL (${failures.length + 1})`);
    process.exit(1);
  });
