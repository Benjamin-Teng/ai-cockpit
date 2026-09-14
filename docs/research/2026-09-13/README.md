# HERDR 查證證據（2026-09-13）

設計文件 §2 的每個技術論斷都應能在這個資料夾找到依據。這裡的檔案是**原始輸出與
研究報告**，不是規格；規格在 `docs/superpowers/specs/`。

| 檔案 | 來源 | 怎麼重新產生 |
|---|---|---|
| `herdr-schema-windows-0.9.0-preview-protocol22.json` | Windows 端 `herdr api schema --json --output <path>` | 同左；版本變了就換檔名 |
| `herdr-schema-wsl-0.8.2-protocol20.json` | WSL 端 `~/.local/bin/herdr api schema --json > <path>`（0.8.2 不接受與 `--output` 併用） | 同左 |
| `herdr-schema-findings.txt` | sonnet 研究 agent 對 Windows schema 的萃取報告 | 一次性產物，schema 改版時重做 |
| `herdr-source-findings.txt` | sonnet 研究 agent 對 HERDR 原始碼（commit `bafbc0949dd996cf7fd0848c8965e254348cc11e`）的引用報告 | 一次性產物；行號以該 commit 為準 |
| `herdr-schema-compare.sh` | 兩版 schema 的 jq 比對腳本，在 WSL 內執行 | 腳本內的路徑指向產生當時的 session 暫存目錄，重跑前改成本資料夾的路徑 |
| `herdr-schema-compare-output.txt` | 上述腳本輸出 | 同上 |
| `herdr-status-server-windows.txt` | Windows 端 `herdr status server` | 同左 |
| `local-checks.txt` | 本機 Rust 工具鏈版本、WSL 發行版、netcat 版本、snapshot 位元組數、`wsl --list --running` 不喚醒虛擬機的實測 | 檔內每段都附指令 |

報告內的檔案路徑指向產生當時的 session 暫存目錄（`...\scratchpad\herdr-research\`），
對應關係：`schema.json` ＝ 本資料夾的 Windows schema；`schema_dump_full.txt` 是該 schema
的逐字展開版，未保存，行號引用可用 Windows schema 的 JSON 路徑重新定位。

## change 1a spike 追加（2026-09-13）

- `change-1a-spikes.md`：設計文件 §11 五個 spike 的執行紀錄與 go/no-go。
- `deidentify-fixture.py`：把真機 snapshot 回應去識別化成 `herdr-client/tests/fixtures/` 用的 fixture。
- 兩份 schema 的複本在 `herdr-client/tests/fixtures/schema-p22.json`、`schema-p20.json`，供合約測試使用；重新匯出時兩處都要更新。
