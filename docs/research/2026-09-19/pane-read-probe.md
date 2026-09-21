# `pane.read` 真機探測（change 3 `live-output` 設計前 spike）

> 日期：2026-09-19　|　工具：`herdr-client/examples/probe_pane_read.rs`（走 `Client::request(PaneReadRequest)`）
> 對象：WSL 端 HERDR 0.8.2（拋棄式測試 pane，內容可控、完整讀取）；Windows 端 HERDR 0.9.0
> （使用中的 pane，`--metadata-only`，未讀出任何畫面文字）。
> 性質：量測紀錄。數字是當天這台機器的觀察值，不是規格；重跑指令見文末。

## 結論（影響設計的五件事）

1. **`revision` 在兩個版本都恆為 0，不能拿來判斷內容有沒有變。** 設計文件 §12「以 `revision` 比對後才推送」
   的前提不成立，去重必須改用內容比對（雜湊或字串相等）。
2. **讀取很便宜。** WSL 端單次約 42 ms、Windows 端約 1.2 ms，且與回應大小無關（37 B 到 6 KB 同速）。
   ADR-0002 估的「每次 0.1–0.3 秒」與 handover 的「數百 ms」都高估；每秒 1–2 次的輪詢不需要方案 B。
3. **既有 `PaneReadResult` 型別對兩個版本的真機回應都能反序列化**，協定層不用改。
4. **`ansi` 格式是 HERDR 從畫面格重新輸出的正規化結果**：只看到 SGR（顏色／粗體）序列與 `\r\n` 換行，
   沒有游標移動。體積約為純文字的 1.2–2.6 倍。
5. **工作中的 agent pane 幾乎每次讀取內容都不同**（spinner／計時器），內容去重對它無效；
   去重只在 pane 靜止時省流量。頻寬上限要用輪詢間隔控制，不能指望去重。

## 1. 延遲與大小

| 端 | 情境 | 次數 | elapsed_ms（min／median／p95／max） | text_bytes |
|---|---|---|---|---|
| WSL 0.8.2 | 靜止 shell，`visible` | 10 | 42.3／43.8／46.7／46.7 | 37 |
| WSL 0.8.2 | 每秒印一行，`visible`，間隔 300 ms | 30 | 39.1／41.2／43.2／43.3 | 141–206 |
| WSL 0.8.2 | `recent --lines 100000`（被 server 截到 1000 行），`ansi` | 1 | 42.8 | 6089 |
| Windows 0.9.0 | 工作中的 claude pane，`visible` | 20 | 1.0／1.2／1.6／44.8 | 3834–4163 |
| Windows 0.9.0 | 同上，`visible`＋`ansi` | 3 | 0.8–1.5 | 4826–4828 |
| Windows 0.9.0 | 靜止 shell，`visible` | 5 | 1.0–1.8 | 679 |

- 全部約 100 次 `pane.read` 請求 0 失敗。Windows 端 20 次中有 1 次 44.8 ms 的離群值（named pipe 忙碌重試的量級，
  `herdr-client` 以 50 ms 為單位重試），其餘都在 2 ms 內。
- `elapsed_ms` 含開連線（WSL 端含啟動 `wsl.exe`＋`nc`）。
- **證據等級**：本節與第 2 節的數字抄自探測工具當場的 stdout，沒有另存原始 TSV；Windows 端依約定不留任何
  內容 dump。第 3、4 節（WSL 端）有原始 dump，已由獨立 subagent 逐項對照（行數、`cmp`、escape 序列清單）。
  要複核本節數字只能重跑。

## 2. `revision` 行為

| 端 | 觀察 | 內容變、`revision` 沒變 |
|---|---|---|
| WSL 0.8.2 | 30 次讀取中內容變了 9 次（每秒多一行），`revision` 全程 0 | 9／9 |
| Windows 0.9.0 | 20 次讀取內容次次不同，`revision` 全程 0 | 19／19 |

- 反向情況（`revision` 變、內容沒變）0 次——因為 `revision` 從沒變過。
- `tab.create` 回應裡 `root_pane` 物件也有一個 `revision` 欄位，值同樣是 0。
- 靜止 pane 連讀內容雜湊完全一致（WSL 10／10、Windows 5／5）：沒有游標閃爍之類的雜訊，內容比對可用。

## 3. `source` 與 `lines` 的語意（WSL 0.8.2，測試 pane 39 列 × 32 欄）

| 參數 | 回應行數 | `truncated` | 說明 |
|---|---|---|---|
| `visible` | 39（畫面列數；尾端空白列會被修掉，靜止時只回 2 行） | false | 目前一屏 |
| `recent`（不給 `lines`） | 80 | true | 預設取最後 80 行 |
| `recent --lines 5`／`40`／`100`／`1000` | 5／40／100／1000 | true | 取最後 N 行 |
| `recent --lines 100000` | 1000 | true | **server 端上限 1000 行** |
| `recent_unwrapped --lines 100000` | 999 | true | 把因欄寬折行的邏輯行接回去 |
| `detection` | 與 `visible` 相同（雜湊一致） | false | 本次看不出差異 |

- 行數以 Rust `str::lines().count()` 計（結尾沒有換行時比 `wc -l` 多 1；`recent_unwrapped` 那列 `wc -l` 是 998）。
- `truncated=true` 的意思是「上面還有更早的歷史沒回傳」，不是錯誤。
- `lines` 從畫面格最底端往上數，**空白列也算**：畫面下半是空的時候，`recent --lines 5` 回 0 bytes。
  要「最後 N 行有字的輸出」不能只靠 `lines`，得多取再自己修掉尾端空白。
- 印了 3000 行後 `recent` 最多拿得到最後 1000 行；沒有 offset，更早的拿不到。
- Windows 端工作中的 claude pane：`recent --lines 1000` 只回 60 行（與 `visible` 同量級）——全螢幕 TUI
  用 alternate screen，沒有 scrollback 可拿。**對 agent pane 而言 `visible` 與 `recent` 幾乎等價。**

## 4. `ansi` 格式樣貌（WSL 0.8.2）

`cat -v` 節錄（`^[` 是 ESC、`^M` 是 CR）：

```text
^[[0m^[[1m^[[38;5;2muser@host^[[0m:^[[0m^[[1m^[[38;5;4m~/work^[[0m^M
^[[0m^[[38;5;2mline 1^[[0m^M
^[[0m^[[38;5;3mline 2^[[0m^M
```

- 原始輸出是 `ESC[32m`，回來變成 `ESC[38;5;2m`：HERDR 是從畫面格重新編碼，不是轉送原始位元組。
- 每行以 `ESC[0m` 開頭、以 `\r\n` 結尾；只出現 SGR 序列（`0`、`1`、`38;5;N`）。
- **未驗**：真彩色（`38;2;r;g;b`）、背景色、底線／反白、全螢幕 TUI 畫面的 `ansi` 輸出（Windows 端依約定沒讀內容）。
- `format=text` 不給 `strip_ansi` 時回應沒有任何控制序列。`strip_ansi=false` 搭配 `format=text` 的行為**未驗**
  （唯一一次嘗試剛好讀到空內容）。

## 5. 不存在的 pane

Windows 0.9.0，對不存在的 pane id 送 `pane.read`（唯讀、無副作用），0.6 ms 回錯誤：

```text
remote error pane_not_found: pane wQ:pZZZ not found (response id 1)
```

錯誤碼是 `pane_not_found`，與 change 1a spike 對 `events.subscribe` 觀察到的同名
（`docs/research/2026-09-13/change-1a-spikes.md`）。

WSL 0.8.2（2026-09-19 補驗，擷取 fixture 後對剛關掉的測試 pane 讀一次）回同一個錯誤碼：

```text
remote error pane_not_found: pane wD:p31 not found (response id 1)
```

兩個版本一致，change 3 可以把 `pane_not_found` 對應成「pane 不存在」。

## 6. 對 change 3 設計的含意

- 去重鍵：內容雜湊（或直接比字串），不是 `revision`。`revision` 照樣帶進型別，但不依賴它。
- 輪詢間隔：兩端都撐得起每秒一次；WSL 端每次 42 ms 主要是 `wsl.exe` 啟動成本，仍遠低於 ADR-0002 的換方案門檻。
- 只讀「被選取的 pane」時，Windows 端每秒一次約 4–5 KB／次，推給瀏覽器的量級可接受，不必做行級 diff。
- named pipe 並發仍要小心（handover 第 4 節：3 條並發會 `ERROR_PIPE_BUSY`）：輸出輪詢要序列化，
  不要與 snapshot／訂閱重開搶同一時間窗。

## 7. 過程中的寫入（僅 WSL 測試 server，使用者 2026-09-19 同意）

`tab.create`（label `probe_pane_read`）→ `pane.send_text` 兩次（彩色迴圈、`seq 1 3000`）→ `tab.close`。
關閉後 `--list` 確認測試 pane 已消失（Sidebar pane 照例換了 id），WSL 測試 server 已停。Windows 端只呼叫
`session.snapshot` 與 `pane.read`。

## 重跑

```bash
# WSL 端（先啟動測試 server，見 docs/handover.md §1；Git Bash 要 MSYS_NO_PATHCONV=1）
cargo run -p herdr-client --example probe_pane_read -- \
  --wsl Ubuntu-24.04 /home/<user>/.config/herdr/herdr.sock --pane <id> \
  --source visible --count 30 --interval-ms 300 --dump-dir <dir>

# Windows 端（不輸出畫面文字）
cargo run -p herdr-client --example probe_pane_read -- --list
cargo run -p herdr-client --example probe_pane_read -- --pane <id> --count 20 --interval-ms 500 --metadata-only
```

升版 HERDR 後值得重跑第 2 節：若新版 `revision` 開始遞增，可改回以它做第一層去重。
