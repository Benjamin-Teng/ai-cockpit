# `pane.read` 的 `ansi` 格式真機探測（change 8 `live-output-color` 設計前）

> 日期：2026-10-02　|　工具：`herdr-client/examples/probe_pane_read.rs`（`--dump-dir` 存到 repo 外的暫存目錄，
> 分析完即刪除）＋臨時 node 腳本（只統計控制序列種類、行長與空白種類，不輸出畫面內容）。
> 對象：Windows 端 HERDR 0.9.2（4 個閒置中的 Claude Code pane，只送 `session.snapshot`／`pane.read`）；
> WSL 端 HERDR 0.8.2（以專屬 socket 臨時啟動的測試 server，6 個還原的 shell pane，量完只停該行程）。
> 性質：量測紀錄。數字是當天這台機器的觀察值，不是規格。

## 結論（影響設計的三件事）

1. **Windows 0.9.2 的 `ansi` 只有 SGR，沒有其他控制序列。** 4 個 pane、visible 與 recent 兩種來源，
   全部控制序列都是 `ESC[…m`：參數只出現 `0`、`2`（dim）、`3`（italic）、`38;5;N`，N 只有 1、2、6、7、11
   （都在 16 色範圍內）。沒有背景色、真彩色、游標移動、OSC，也沒有冒號子參數。
2. **樣式不跨行。** 每行結尾都已回到預設樣式；換行是 `\r\n`。
3. **把 `ansi` 去掉控制序列與 `\r` 後，與同一時刻的 `format=text` 只差行尾空白與結尾換行**（兩端同一模式）：
   - `text` 結尾多一個 `\n`，`ansi` 最後一行沒有換行。
   - `ansi` 會保留部分行尾空白（Windows 每 pane 0～2 行；WSL 是提示字元 `$` 後的空格），`text` 會剪掉。
   - 其餘逐字相同。

## 未驗證

- 背景色、真彩色、反白、底線、全螢幕 TUI 的游標定位：本次樣本（閒置的 Claude Code、shell 提示字元）沒有出現。
  Windows 端不能寫入，無法自造測試內容；解析器對這些輸入採保守處理（見 change 8 design）。
- 工作中（畫面持續變化）的 pane：未量；`text` 與 `ansi` 分兩次讀，變動中的畫面無法逐字比對。

## 重跑

```sh
# Windows（唯讀）：列 pane，再對某個 pane 各讀一次兩種格式到 repo 外的暫存目錄
cargo run -p herdr-client --example probe_pane_read -- --list
cargo run -p herdr-client --example probe_pane_read -- \
    --pane <id> --source recent --lines 200 --format ansi --count 1 --dump-dir <repo 外的目錄>

# WSL：以專屬 socket 啟動測試 server（停止時只 kill 該 PID，不用全域 herdr server stop）
wsl.exe -d Ubuntu-24.04 --exec bash -c \
    'HERDR_SOCKET_PATH=/tmp/<專屬>.sock setsid -f "$HOME/.local/bin/herdr" server </dev/null >/tmp/<log> 2>&1'
MSYS_NO_PATHCONV=1 cargo run -p herdr-client --example probe_pane_read -- \
    --wsl Ubuntu-24.04 /tmp/<專屬>.sock --pane <id> --source recent --lines 200 --format ansi --count 1 --dump-dir <目錄>
```

dump 下來的是畫面原文，分析只輸出統計後立即刪除，不得進 repo。
