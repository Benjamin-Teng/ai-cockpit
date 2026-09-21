# Live Output 真機驗收（live-output task 6.1）

> 日期：2026-09-20　|　對象：WSL 端 HERDR 0.8.2（拋棄式測試 tab，`workspace_id=wD`）、真的
> `target/debug/cockpit.exe`（設定只含一筆 `wsl` runtime，無 `[[project]]`）、headless Chrome。
> 腳本：`docs/research/2026-09-19/live-output-real-check.js`（`node` 執行）。
> 性質：一次性真機驗收紀錄；環境事實與觀察值不是規格，重跑指令見文末。

## 結論

腳本全數 PASS，連續跑兩次（修正一處腳本斷言後）皆 0 FAIL。spec `live-output`「輪詢與顯示」
「內容跟上」（3 秒門檻）、「失敗與消失的呈現」三個情境（pane 被關掉／端點回 404／runtime
斷線後恢復）、「輸出讀取端點」「pane 不存在」（404 而非 503）、R18 追加的「runtime 斷線後
恢復」完整端到端 GIVEN，都在真的 WSL HERDR、真的 `cockpit`、真的瀏覽器上驗證通過。

R18 觀察到的是 **(a) 同一 pane 仍在**：WSL 測試 server 重啟後，同一個 pane id
（兩次跑分別是 `wD:p3D`、`wD:p3J`）仍然存在，Cockpit 的過期標示與失敗原因在恢復後消失、
輪詢繼續反映真實內容。

## 環境與版本

- WSL distro：`Ubuntu-24.04`；HERDR `0.8.2`（protocol `20`）；socket
  `/home/<user>/.config/herdr/herdr.sock`；workspace `wD`（label `~/work`）。
- `cockpit`：分支 `change-3-live-output`，commit `7b28ab5`（跑之前的 `HEAD`，腳本本身這次
  commit 未改動任何 `cockpit`／`cockpit-herdr` 原始碼）；`cargo build -p cockpit`（debug，
  `target/debug/cockpit.exe`）。
- `herdr-client`：`cargo build -p herdr-client --example probe_pane_read`
  （`target/debug/examples/probe_pane_read.exe`）。
- Windows 端 HERDR 0.9.0 全程未連線、未送任何請求（腳本只對 WSL 端啟停與寫入）。
- 驗收用設定檔（系統暫存目錄，跑完即刪，內容逐字如下，`<port>` 為腳本挑到的空埠 `7792`）：

  ```toml
  [server]
  listen = "127.0.0.1:7792"

  [[runtime]]
  id = "wsl"
  kind = "herdr"
  wsl = { distro = "Ubuntu-24.04", socket = "/home/<user>/.config/herdr/herdr.sock" }
  ```

- 使用者自己的 `cockpit.toml`（repo 根、gitignored）全程未被讀取或修改；使用者的
  `127.0.0.1:7770` 全程未被觸碰。

## 執行方式

```bash
cargo build -p cockpit
cargo build -p herdr-client --example probe_pane_read
HERDR_CLIENT_TEST_ALLOW_WSL_WRITES=1 node docs/research/2026-09-19/live-output-real-check.js
```

腳本一開頭檢查 `HERDR_CLIENT_TEST_ALLOW_WSL_WRITES=1`（比照
`docs/research/2026-09-15/live-state-check.py` 的自我把關），沒有這個環境變數就直接結束、
不做任何寫入。

> **後續變更（final fix wave F1，2026-09-21）**：這次跑（2026-09-20）的腳本把 distro／socket
> 寫死在檔案內；F1 之後改成必須另外提供環境變數 `COCKPIT_ACCEPT_WSL_DISTRO`（沒給 socket 會
> 用 `wsl.exe -d <distro> -e bash -lc 'echo $HOME'` 推得 `$HOME/.config/herdr/herdr.sock`，
> 不用另外設 `COCKPIT_ACCEPT_WSL_SOCKET`），並在啟動前多一道「WSL 測試 server 已經在跑就拒絕
> 執行」的檢查。上面這份指令是這次跑當時的樣子，重跑見文末「重跑」一節的最新用法。
>
> **後續變更（final fix round 2 finding A／C，2026-09-21）**：下面「1. 啟動 WSL 測試 server」
> 貼的 `wsl.exe -d Ubuntu-24.04 -e bash -lc "setsid -f ~/.local/bin/herdr server ..."`
> 是這次跑（2026-09-20）當時 `startWslServer()` 的樣子。finding A 之後改成寫入 PID 憑證的
> 版本：內層多一個 `bash -c 'echo $$ > <pidfile>; exec "$HOME/.local/bin/herdr" server ...'`，
> 讓腳本讀回的 PID（`ownedPid`）保證就是 server 本身，往後每次停止前都重新比對「目前在跑的
> `herdr` PID 是否還是 `ownedPid`」才執行 `herdr server stop`，不再只靠一個布林旗標；
> finding C 同時把 `COCKPIT_ACCEPT_WSL_DISTRO`／`COCKPIT_ACCEPT_WSL_SOCKET` 兩個值都加上白
> 名單驗證，並把原本組進 `bash -lc` 字串的 socket 存在檢查（`wslSocketExists()`）改成純
> argv（`wsl.exe -d <distro> -e test -S <socket>`）。這次跑（2026-09-20）記錄的每一步關鍵
> 輸出仍然真實有效（行為本身沒變，只有「怎麼啟停、怎麼確認所有權、怎麼防 shell 注入」這幾件
> 事的做法變了）；新一輪的真機驗證證據（含 `ownedPid` 比對、所有權不相符不誤停、shell 注入
> 防護）見 `.superpowers/sdd/tasks/task-final-fix2-report.md`，不重複貼在這份文件裡。
>
> **後續變更（whole-branch review finding 1／2／3，2026-09-21）**：上一輪（finding A）的
> `stopWslServerIfOwned()` 雖然會先用一次獨立的 `pgrep` 呼叫比對 `ownedPid`，但比對之後是
> 透過**另一個**獨立呼叫執行全域 `herdr server stop`——若 owned server 剛好在這兩次呼叫之間
> 退出、另一個 server 隨即用同一個 socket 起來，全域 stop 會誤殺後者（finding 1，TOCTOU）。
> 真機實測 WSL HERDR 0.8.2 對 `SIGTERM` 的反應（3/3 trial）：行程在約 140–250 ms 內結束、
> 不留孤兒行程，只留下殘留的 socket 檔（下一次啟動不受影響）；因此改成 `killAndWaitPid()`——
> **單一** `wsl.exe -e bash -c` 呼叫裡完成「對這一個 PID 送 `SIGTERM`、輪詢等待它結束、逾時
> 再對同一個 PID 補 `SIGKILL`」，PID 用位置參數傳遞、不做字串插值；確認結束後只有在偵測不到
> 任何 herdr 行程時才清掉殘留 socket 檔。全檔不再有任何一處呼叫全域 `herdr server stop`。
> 另外兩個 finding：**finding 2** 是初次啟動與 R18 重啟時 `verifyOwnership()` 的回傳值先前
> 只拿去印 FAIL，沒有真的擋住流程繼續——現在改成沒通過就立刻 throw、`ownedPid` 保持
> `null`，收尾不會誤動任何 server。**finding 3** 是 `HERDR_CLIENT_TEST_ALLOW_WSL_WRITES=1`
> 這道寫入 opt-in 檢查先前只存在 `main()` 裡，帶 `--verify-ownership-mismatch` 執行會跳過
> 這道檢查——現在搬到選擇要跑哪個模式「之前」的共用入口 `assertWslWriteOptIn()`，兩個模式都
> 逃不掉。三個 finding 的真機驗證證據（SIGTERM 行為實測、正常路徑 0 FAIL、拒絕已在跑、所有權
> 不相符不誤停、不帶 opt-in 兩個模式都拒絕、shell 注入回歸）見
> `.superpowers/sdd/tasks/task-script-fix-report.md`，不重複貼在這份文件裡。
>
> **後續變更（複審 finding A／B round 2，2026-09-21）**：上一輪（finding 1）的
> `stopWslServerIfOwned()` 雖然已經不再呼叫全域 `herdr server stop`，但停止前仍然是先用
> **一次獨立的** `pgrep` invocation 比對 `ownedPid`，比對通過之後才呼叫**另一個獨立**
> invocation 的 `killAndWaitPid()` 送 `SIGTERM`——這兩次獨立 invocation 之間仍有窄縫，原程序
> 若剛好在這段窗口退出、PID 被系統重用給別的行程，第二個 invocation 送出的訊號就會打在無關的
> 行程上（finding A round 2，PID 重用）。修法：所有權憑證改成「PID＋starttime＋comm」
> （`/proc/<pid>/stat` 第 22 欄、`/proc/<pid>/comm`），且「核對身分 → 送 `SIGTERM` → 輪詢核對
> → 必要時核對後補 `SIGKILL`」全部收進 `killAndWaitPid()` 內的**單一** `wsl.exe -e bash -c`
> 呼叫，PID／starttime／comm 都用位置參數傳入、先過白名單驗證。另外一個 finding（finding B
> round 2）：先前版本在確認程序結束後，只要偵測不到任何 herdr 行程就會清掉殘留的 socket
> 檔——這個判斷本身仍是 `wslSocketExists()`／`listHerdrServerPids()`／`rm -f` 三個獨立
> invocation，外來 HERDR 可以在中間的窄縫啟動並綁定新 socket，接著被第三個 invocation 誤刪
> （TOCTOU）。真機實測（HERDR 0.8.2）：對測試 server 送 `SIGTERM` 之後**完全不刪除**殘留的
> socket 檔，連續 3 次重新啟動 HERDR server 都在 1.5 秒內成功、且都能立刻連線成功——殘留的
> socket 檔完全不妨礙下一次啟動。因此腳本**不再刪除任何 socket 檔**；連帶地，「已在跑」的
> 拒絕判準與「已停止」的驗證都不再用「socket 檔存在／不存在」當依據，改成「有沒有任何 herdr
> 行程在跑」＋「socket 是否真的連得上（純 argv `nc -U` 送一筆 JSON-RPC，短逾時內是否收到
> 回應）」。兩個 finding 的真機驗證證據（正常路徑 0 FAIL＋ownedPid／starttime／每次停止的結果
> 標記、連跑兩次不因殘留 socket 誤拒、身分不符不誤停、不帶 opt-in 兩個模式都拒絕、shell 注入
> 回歸）見 `.superpowers/sdd/tasks/task-script-fix2-report.md`，不重複貼在這份文件裡。
>
> **後續變更（round 3，換路，2026-09-21）**：round 1／2 連續四輪 Codex review 都在「腳本與開發者
> 手動啟動的 server 共用同一個預設 socket 路徑」這件事上找到新的競態（啟動前怎麼判斷有沒有
> 別人、收尾會不會停到別人的、殘留 socket 算不算有人、慢啟動的 server 被誤判為殘留）。控制端
> 裁決不再逐一補，改成換路：**每次執行使用本次執行專屬的 socket 路徑**（`/tmp/cockpit-accept-
> <run id>.sock`，透過 `HERDR_SOCKET_PATH` 環境變數告訴 `herdr server` 綁定這個路徑；真機驗證
> 見 `.superpowers/sdd/tasks/task-script-fix3-report.md`「第一步」：`HERDR_SOCKET_PATH` 確實被
> HERDR 0.8.2 採用、預設路徑完全不受影響、一個用專屬路徑一個用預設路徑的兩個 server 可以並存
> 且互不可見）。移除了環境變數 `COCKPIT_ACCEPT_WSL_SOCKET`（與由 `$HOME` 推得預設 socket 的
> 邏輯）——distro 仍由 `COCKPIT_ACCEPT_WSL_DISTRO` 提供，socket 不再是外部輸入。共用資源沒了，
> 上面幾輪逐一補的競態從根上不存在：不再需要「啟動前拒絕執行」檢查（只保留極簡防呆——專屬路徑
> 啟動前必須不存在，撞到就換一個 run id 重試一次）；`verifyOwnership()` 改成只認「這個 PID
> 自己」（讀 `/proc/<pid>/comm` 是否為 `herdr`、專屬 socket 是否連得上），不再比對「機器上有
> 哪些 herdr 行程」；收尾改回**會**刪除自己的專屬 socket 檔與 pidfile（round 2 因為路徑共用
> 才刻意不刪，round 3 的路徑只屬於本次執行，刪除不影響任何人）；R18 重啟沿用同一個專屬路徑，
> 不需要重做拒絕檢查。所有權憑證（PID＋starttime＋comm）與單一 invocation 的停止機制沿用不變。
> 真機驗證（正常路徑 0 FAIL、與手動啟動的 server 並存且互不可見／互不影響、身分不符不誤停、
> 不帶 opt-in 兩個模式都拒絕、shell 注入回歸改測 distro、連跑兩次互不影響）見
> `.superpowers/sdd/tasks/task-script-fix3-report.md`，不重複貼在這份文件裡。
>
> **後續變更（round 4，2026-09-21）**：round 3 換路時漏了一個呼叫點——`listTabs()` 仍用
> `herdr api snapshot` CLI、沒帶 `HERDR_SOCKET_PATH`，查到的是預設路徑的 server（沒開時靜默
> 拿到空清單，有開時拿別人 server 的 tab 清單做兜底清理決策）。改成經 `wslRpc()` 對專屬 socket
> 送 `session.snapshot`，讀不到就拋錯、不回空清單；並在 log 印出 baseline tab 清單。真機驗證
> 兩個情境都 `RESULT: PASS`：(1) 預設 server 未啟動——baseline 拿到專屬 server 的 2 個 tab
> （修正前這裡是空清單）；(2) 手動啟動預設路徑 server 並在上面多建一個標記 tab（預設 server 共
> 3 個 tab）——腳本的 baseline 仍只有專屬 server 的 2 個，不含標記 tab；腳本跑完後預設 server
> 的 PID 與 3 個 tab 都原封不動。

## 每一步的指令與關鍵輸出

腳本內部全部用 Node 的 `child_process.spawnSync`／`spawn` 直接呼叫下列原生程式（不經過
Git Bash，不需要 `MSYS_NO_PATHCONV`）；下面按 brief 的步驟編號列出對應的操作與關鍵輸出。

### 1. 啟動 WSL 測試 server → 建測試 tab → 送迴圈

```text
wsl.exe -d Ubuntu-24.04 -e bash -lc "setsid -f ~/.local/bin/herdr server >/tmp/herdr-server.log 2>&1 </dev/null"
（等 socket 出現＋session.snapshot 有回應）
tab.create → tab_id=wD:t1C pane_id=wD:p3D
pane.send_text → {"id":"st","result":{"type":"ok"}}
```

### 2. 啟動 cockpit → headless Chrome → 等測試 pane 出現 → 點該列

```text
cockpit 應該在 20 秒內開始回應（port 7792） → ok
runtime wsl 在 30 秒內變成 connected（/api/state） → ok
測試 pane wD:p3D 的列出現在畫面上 → ok
點選之後該列出現選定標示 → ok
面板標題應該顯示 wsl / wD:p3D → ok
面板出現含 tick 的內容 → ok
```

### 3.「內容跟上」（3 秒門檻，觀察 10 秒）

```text
取樣點數：39；最大 tick 序列：[1,1,1,1,1,2,2,2,2,3,3,3,3,4,4,4,4,6,6,6,6,7,7,7,7,8,8,8,8,9,9,9,9,10,10,10,10,11,11]
任何 >=3 秒的觀察視窗內面板最大 tick 都應該至少前進一次（實際違規 0 個）→ ok
```

判準與 `docs/research/2026-09-19/live-output-check.js` 的 `findStalls()` 相同（沿用其理由：
pane 每秒確定性地多印一行，因此「任何 >=3 秒視窗內最大值至少前進一次」等價於「每一行在產生後
3 秒內出現在面板」）。

### 4. 端點直接驗（curl，Host 檢查）

```text
$ curl -H "Host: 127.0.0.1:7792" http://127.0.0.1:7792/api/runtimes/wsl/panes/wD%3Ap3D/output
→ 200，text 含 "tick"（節錄 {"runtime":"wsl","pane_id":"wD:p3D","format":"text","text":"for i in $(seq 1 600); do echo \"\ntick $i\"; ...）

$ curl -H "Host: evil.example:7792" http://127.0.0.1:7792/api/runtimes/wsl/panes/wD%3Ap3D/output
→ 403
```

### 5.（Ruling R18）runtime 斷線後恢復

```text
>>> 停 WSL 測試 server <<<
WSL 測試 server 在 15 秒內停止（socket 消失） → ok
停 server 後面板在時限內標為過期（is-stale） → ok
過期期間 opacity 應該不是 1（實際 0.55） → ok
過期期間顯示失敗原因（.output-error-reason 可見） → ok
失敗原因文字："讀取 pane 輸出失敗：connect error: HERDR server not running: bridge exited
  (exit code: 1): nc: /home/<user>/.config/herdr/herdr.sock: No such file or directory"
503（runtime 斷線）不是「pane 已不存在」，gone 提示應該仍隱藏 → ok
斷線期間應該依節奏繼續重試（5 秒內 >=2 次 /output 請求，實際 4 次） → ok

>>> 重啟 WSL 測試 server <<<
重啟後 WSL 測試 server 在 30 秒內恢復 → ok
逾時（30000 ms）：重啟後應該恢復成 (a) 或顯示 (b)「pane 已不存在」 → ok
R18 觀察結果：(a) 同一 pane 仍在，恢復

(a) 恢復後 gone 提示應該仍隱藏 → ok
(a) 恢復後失敗原因應該消失 → ok
(a) 恢復後 opacity 應該變回 1（實際 1） → ok
(a) 恢復後面板應該已經讀到真的內容（非空） → ok
```

**真機觀察（不是產品缺陷，是這次測試環境的行為，見下一節「腳本修正紀錄」）**：WSL 測試
server 重啟後，pane id 雖然保留（走到 (a) 分支），但原本執行中的 tick 迴圈行程被中斷、pane
內容重置成一個新的 shell prompt（例如 `"user@host:~/work$\n"`，一個非空的新提示字串）。這是
`herdr server stop`／重啟本身對底層 shell 行程的影響，Cockpit 正確地把讀到的新內容顯示出來
（過期標示與原因清除、`text` 更新）。為了在這個真機行為下仍然驗到 spec 用字「內容更新」，
腳本改成對同一個 pane 重新送一次迴圈指令，驗輪詢確實把新內容反映出來：

```text
--- (a) 對同一個 pane 重新送一次 tick 迴圈，驗證恢復後輪詢確實反映真實內容 ---
(a) 對同一個 pane 重新送迴圈後，面板在 10 秒內出現含 tick 的內容
  （證明恢復後輪詢確實反映真實 pane 內容） → ok
```

之後照 brief 備註「重啟後若要繼續後面步驟，需要重新建測試 tab」，重新 `tab.create`
（`wD:t1D`／`wD:p3F`）供步驟 6、7 使用。

### 6.「pane 被關掉」

```text
tab.close(wD:t1D) 回 ok → ok
關掉 tab 後面板顯示「pane 已不存在」 → ok
「pane 已不存在」文字應該逐字一致（實際 "pane 已不存在"） → ok
顯示「pane 已不存在」之後 5 秒內不應該再有新的 /output 請求（CDP Network.requestWillBeSent
  過濾 /panes/.+/output，實際 0 → 0） → ok
```

### 7. 不存在的 pane

```text
$ probe_pane_read --wsl Ubuntu-24.04 /home/<user>/.config/herdr/herdr.sock --pane wD:p3F --count 1 --metadata-only
stderr: iter 1 失敗: remote error pane_not_found: pane wD:p3F not found (response id 1)
```

stderr 含錯誤碼 `pane_not_found`，與
`docs/research/2026-09-19/pane-read-probe.md` 第 5 節記錄的 WSL 0.8.2 與 Windows 0.9.0
一致——WSL 0.8.2 這次真機重跑（本 task 是第二次對 WSL 0.8.2 驗這件事）再次確認同一錯誤碼，
不需要修 `cockpit-herdr` 的錯誤對應或 spec。

```text
$ curl -H "Host: 127.0.0.1:7792" http://127.0.0.1:7792/api/runtimes/wsl/panes/wD%3Ap3F/output
→ 404（不是 503），body {"error":"pane 不存在：wD:p3F"}
```

### 8. 收尾

```text
chrome-real PID 87200 已終止（tasklist 查無此 PID） → ok
cockpit PID 42924 已終止（tasklist 查無此 PID） → ok
port 7792（cockpit）應該不再有 LISTENING 的行程 → ok
收尾 tab.close wD:t1C → ok（起始建的第一個測試 tab；R18 重啟後建的第二個測試 tab 在步驟 6
  已經關掉，收尾階段重新列一次帶 live_output_acceptance 標記的 tab，兩個都不殘留）
收尾：WSL 測試 server 在 15 秒內停止（socket 消失） → ok
暫存設定目錄已刪除 → ok
```

收尾後另外人工確認（見文末「收尾後環境確認」）：WSL socket 消失、埠不再 LISTEN、沒有殘留
`cockpit.exe`／`chrome.exe`、`git status --short` 只有這兩個新檔案。

## 腳本修正紀錄（R18 斷言，不是產品缺陷）

第一次跑（修正前）在步驟 5 的最後一個斷言 FAIL：

```text
FAIL (a) 恢復後內容應該更新為真正的 tick 輸出（實際節錄一個新的 shell prompt，例如 "user@host:~/work$\n"）
RESULT: FAIL (1)
```

判斷：其餘 22 個斷言（含過期標示、原因顯示、恢復後 opacity／gone／reason 全部正確清除）都
`ok`，唯獨這一條假設「恢復後內容一定含 tick」。真正原因是 WSL 測試 server 重啟殺掉了原本執行
中的 shell 行程（見上一節「真機觀察」），不是 Cockpit 沒有正確輪詢或顯示——Cockpit 忠實反映
了它讀到的（重置後的）內容。這是腳本斷言對測試環境的假設錯誤，不是產品缺陷，因此修腳本
（改成對同一個 pane 重新送一次迴圈指令、驗輪詢確實反映新內容），不動
`cockpit`／`cockpit-herdr` 原始碼。修正後連續兩次整支腳本跑（含這次修正驗證的那次）皆
`RESULT: PASS`，見下一節。

## 穩定性：連續兩次 PASS

修正後連續跑兩次（各自從乾淨環境開始，WSL server 全程未殘留）：

```text
RUN A: exit=0  RESULT: PASS（0 FAIL）  R18 觀察：(a) 同一 pane 仍在，恢復
RUN B: exit=0  RESULT: PASS（0 FAIL）  R18 觀察：(a) 同一 pane 仍在，恢復
```

兩次的 pane id 不同（各自新建的測試 tab：RUN A `wD:p3D`／R18 後重建的 `wD:p3F`，RUN B
`wD:p3J`／R18 後重建的 `wD:p3M`），行為一致。

## 未涵蓋的項目

- **只驗到 R18 的 (a) 分支**：這次兩次真機跑都是「WSL 測試 server 重啟後同一個 pane id 仍在」
  （(a)）。brief 承認的另一種合法結果 (b)「pane 已不存在」（測試 server 重啟後原 pane 真的消
  失、端點回 404）這次沒有觀察到——腳本的邏輯兩種都能正確判定與斷言（見程式碼
  `r18Outcome === "a" | "b"` 兩個分支），但目前沒有真機證據涵蓋 (b) 分支曾經被腳本正確跑過。
- **`ansi` 格式、真彩色、全螢幕 TUI 畫面**：這次沿用固定文字輸出（`echo "tick $i"`），沒有
  另外驗證含 ANSI 控制序列或全螢幕 agent 畫面的顯示（那屬於 task 6.2 使用者手動用真的 agent
  pane 驗收的範圍）。
- **多個瀏覽器分頁同時輪詢同一 runtime**：design D5「同一個 runtime 的輸出讀取排隊」已有
  `cockpit-herdr` 的單元測試涵蓋（假 HERDR），這次真機只開了一個瀏覽器分頁，沒有另外對真機
  驗證多分頁並發時的排隊行為。
- **Windows 端 HERDR 0.9.0**：這個 task 依操作邊界全程只碰 WSL 測試 server；Windows 端的
  Live Output 由 task 6.2 使用者手動驗收（見下一節）。

## 收尾後環境確認

```text
$ wsl.exe -d Ubuntu-24.04 -e bash -lc "test -S /home/<user>/.config/herdr/herdr.sock && echo SOCK_PRESENT || echo SOCK_ABSENT"
SOCK_ABSENT

$ netstat -ano | grep "127.0.0.1:7792.*LISTENING"
（無輸出，7792 未在 LISTEN）

$ tasklist | grep -i "cockpit.exe\|chrome.exe"
（無輸出，沒有殘留行程）

$ git status --short
?? docs/research/2026-09-19/live-output-real-check.js
?? docs/research/2026-09-19/live-output-acceptance.md
```

## 重跑

腳本目前的用法（round 3 換路之後，distro 由環境變數提供並通過白名單驗證；socket 不再是外部
輸入，每次執行自動產生本次執行專屬的路徑，透過 `HERDR_SOCKET_PATH` 讓 `herdr server` 綁定）：

```bash
cargo build -p cockpit
cargo build -p herdr-client --example probe_pane_read
HERDR_CLIENT_TEST_ALLOW_WSL_WRITES=1 COCKPIT_ACCEPT_WSL_DISTRO=Ubuntu-24.04 \
  node docs/research/2026-09-19/live-output-real-check.js
```

需要 HERDR 已安裝在 `~/.local/bin/herdr`、workspace `wD` 存在（`session.snapshot` 查得到）。
**不需要**跑之前先確認 WSL 測試 server 是停止狀態——本次執行使用專屬的 socket 路徑
（`/tmp/cockpit-accept-<run id>.sock`），跟開發者手動啟動的 WSL HERDR server（預設路徑）完全
不衝突，兩者可以整趟全程並存、互不可見（round 3 真機驗證：一個用專屬路徑一個用預設路徑的兩個
server 可以並存，見下方「後續變更」）。啟動前只做一個極簡防呆：專屬 socket 路徑必須不存在
（撞到就換一個 run id 重試一次，仍存在才報錯、非 0 結束，不做任何收尾）。
`COCKPIT_ACCEPT_WSL_DISTRO` 不符合白名單格式（只允許英數字、點、底線、連字號）會直接拒絕、
非 0 結束（finding C）；沒有設定 `HERDR_CLIENT_TEST_ALLOW_WSL_WRITES=1` 也會直接結束、不對
WSL 送出任何請求——這道檢查是 `main()` 與 `--verify-ownership-mismatch` 共用的入口，兩個模式
都逃不掉（finding 3）。停止 WSL 測試 server 時，實際下手的動作是在**單一** `wsl.exe` 呼叫內
核對 PID＋starttime＋comm 身分、通過才對這個 PID 送 `SIGTERM`（逾時再核對身分後補
`SIGKILL`），不是全域 `herdr server stop`（finding 1；身分核對機制見 finding A round 2）。
腳本收尾時**會刪除**自己的專屬 socket 檔與 pidfile（round 3；這兩個路徑只屬於本次執行，
刪除不會影響任何人——跟 round 2「完全不刪除任何 socket 檔」的做法不同，round 2 是因為當時
的路徑是共用的）。`COCKPIT_CHROME` 環境變數可覆寫 Chrome 路徑（預設
`C:\Program Files\Google\Chrome\Application\chrome.exe`）。

另有一個獨立的驗證模式（`--verify-ownership-mismatch`；finding A 驗證 (iii) 專用，不跑主流程
的 7 個步驟，只驗「所有權不相符時不誤停」這條規則，見腳本檔頭與
`verifyOwnershipMismatchDoesNotStop()` 的說明）：

```bash
HERDR_CLIENT_TEST_ALLOW_WSL_WRITES=1 COCKPIT_ACCEPT_WSL_DISTRO=Ubuntu-24.04 \
  node docs/research/2026-09-19/live-output-real-check.js --verify-ownership-mismatch
```

這個模式驗完**不會**自動停掉它啟動的 WSL 測試 server（刻意如此，驗的就是「不該自動停」這件
事）；跑完要自己手動收尾——腳本執行結束時會印出具體指令（對記下的 PID 送 `SIGTERM`，不是全域
`herdr server stop`，因為 round 3 之後這個 server 綁在本次執行的專屬路徑，不是預設路徑）。

## Windows 端手動驗收（task 6.2）

> 2026-09-21 由 Claude 代為執行（使用者明示授權並預先開好 Chrome；以 Claude in Chrome 操作真實瀏覽器，真的滑鼠點擊與鍵盤事件）。
> 環境：Windows 端 HERDR 0.9.0-preview（protocol 22，使用中的真實工作階段）；分支 `change-3-live-output` 於 `22c3470`；
> `cargo run -p cockpit -- --config <暫存目錄的驗收用設定>`。驗收用設定沿用使用者 `cockpit.toml` 的 runtime 區段、改用埠 7793、
> 另加一個測試用 `[[project]]`（workstream `viewer` 綁一個不存在的 workspace → `unbound`；`nobinding` 無綁定）與放在暫存目錄的狀態檔；
> 使用者的 `cockpit.toml` 未被讀寫以外的方式觸碰（mtime 不變），repo 根沒有產生狀態檔。
> **對 Windows 端 HERDR 全程唯讀**（只有 Cockpit 自己的 `session.snapshot`／`events.subscribe`／`pane.read`，與探測工具的一次 `pane.read`）。
> **隱私**：面板顯示的是真實 pane 畫面；驗收只以 JavaScript 取長度、行數、雜湊是否改變、樣式、捲動位置等中繼資料，沒有把畫面文字讀出或存檔。

- [x] 啟動 `cockpit`，點一個工作中的 agent pane，確認面板內容與 HERDR 畫面一致、每秒更新
  - 點 `win / wN:p1`（agent `claude`、`working`）：面板打開、標題 `win / wN:p1`、該列出現選定標示（左邊框 `rgb(88, 166, 255)`，
    未選列為透明）；內容 61 行純文字、沒有 ESC 控制字元、`<pre>` 內沒有子元素、等寬字體、`white-space: pre`。
  - 9 秒內內容變了 5 次，間隔 993–2008 ms（最長 2.0 秒，spec 門檻 3 秒）。間隔約 2 秒而非 1 秒是因為該分頁當時
    `document.visibilityState === "hidden"`，Chrome 對背景分頁的計時器降頻（design D1 預期的行為）；前景分頁為 1 秒。
  - 與 HERDR 一致：對靜止的 shell pane `wN:p2`，面板文字 679 bytes、端點回應 679 bytes（兩者逐字相等）、
    `probe_pane_read --pane wN:p2 --source recent --format text --lines 200 --metadata-only` 直接讀 HERDR 也是 679 bytes。
  - 30 個 `/output` 請求（Resource Timing）沒有任何重疊，單次最長 18 ms。
- [x] 往上捲不被拉回：內容可捲動（`maxScroll` 575），捲到非零中段 `scrollTop` 192，等內容實際變了 2 次後仍是 192。
- [x] 點 workstream「看輸出」能選到綁定的 pane：先按 `viewer` 的「改綁」→ 按 `wN:p2` 列的「綁定到這裡」（只寫 Cockpit 自己的暫存狀態檔）→
  `viewer` 變成 `bound`（`source: override`、`wN:p2`）、出現「看輸出」（`nobinding` 沒有）→ 按下後選取由 `wN:p1` 換成 `win / wN:p2`。
- [x] `curl -H "Host: evil.example:7793" …/output` → **403**；`Host: 127.0.0.1:7793`＋`Origin: https://evil.example` → **403**。
- [x] 捲回面板最底後新內容到達仍貼底（`scrollHeight - scrollTop - clientHeight` 前後皆約 0）；投影更新時面板不受影響：
  期間投影 version 60→62、`#app` 的子節點確實被換掉，而 `#output` 與 `<pre>` 節點是同一個、選定標示仍在。
  （註：這個 pane 是全螢幕 TUI，內容等高替換，「貼底跟著走」在這裡沒有辨識力；有辨識力的驗證在 `live-output-check.js` 的 I 段。）
- [x] 改綁模式期間 pane 列不可點選：兩列都沒有 `data-action`、不是 `.selectable`、`tabIndex` −1、`cursor: auto`；既有選取（`wN:p1`）與面板保留；
  點 `wN:p2` 列上按鈕以外的區域 → 選取不變、仍在改綁模式；按「綁定到這裡」→ 送出覆蓋、離開改綁模式、pane 列恢復 `cursor: pointer`。
- [x] 合法 `Host`（`127.0.0.1:7793`、`localhost:7793`）→ **200**，回應帶 `Cache-Control: no-store`、`X-Content-Type-Options: nosniff`、
  `application/json`，本體欄位 `runtime`／`pane_id`／`format`（`text`）／`text`／`truncated`。另驗：`HEAD` → 405（帶兩個標頭）、`POST` → 405
  `{"error":"這個端點只接受 GET"}`、不存在的 pane → 404、不認識的 runtime → 404、斷線中的 `wsl` → 503、路徑 `%FF` → 400。
- [ ] 在 HERDR 關掉被選的 pane → 「pane 已不存在」：**未在 Windows 端執行**。這需要對使用者正在使用的 Windows 端 HERDR 做關閉 pane 的操作，
  超出「Windows 端唯讀」的約束，Claude 沒有做。同一情境已由 task 6.1 在 WSL 真機驗過（`tab.close` → 面板顯示「pane 已不存在」、停止請求），
  也由 `live-output-check.js` 的 J／K 段在替身上驗過。使用者若要在 Windows 端親眼確認，手動關一個不要的 pane 觀察即可。
- 另驗「取消選取」：按面板「關閉」→ 面板收起、沒有任何選定列、之後 6 秒內 0 個新的 `/output` 請求。

### 發現：鍵盤焦點在整頁重畫時消失，影響「鍵盤選定」

pane 列可以聚焦（`role="button"`、`tabindex="0"`），聚焦後在同一個事件迴圈內送出 Enter 的 `keydown` 會選定該 pane——處理邏輯正確。
但**聚焦後約 5 秒，投影更新（version 147→149）觸發整頁重畫，被聚焦的那一列節點被換掉（`isConnected === false`），焦點掉回 `<body>`**；
此時再按 Enter 什麼都不會發生（實測：以真的鍵盤事件按 Enter，選取為空）。在這台機器上有工作中的 agent 時，投影大約每數秒就更新一次，
所以以 Tab 逐一移動焦點的鍵盤使用者實際上很難走到 pane 列並按下 Enter。

這是 `docs/handover.md` 第 5 節已知的 M3（「每次推送整頁重畫使鍵盤焦點消失」，原訂留待視覺改版），不是 change 3 引入的機制；但 change 3 在
spec `live-output`「選定一個 pane」新增了「pane 列必須能以鍵盤聚焦並以 Enter 或 Space 選定」與情境「鍵盤選定」，在真實的重畫頻率下這個要求只在
「聚焦後、下一次重畫前」的視窗內成立。`live-output-check.js` 的鍵盤情境是聚焦後數毫秒內就按鍵，所以沒有抓到。
可能的修法（未做，待使用者決定）：`render.js` 在 `replaceChildren` 前記下 `document.activeElement` 的識別（`data-action`＋`data-runtime`／`data-pane`／
`data-workstream` 等），重畫後找回對應的新節點並 `focus()`——這會一併解掉 M3。
