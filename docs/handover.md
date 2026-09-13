# 交接：下一段任務

> **建立日期**：2026-09-13　|　**上一段做完的事**：brainstorming 完成，設計文件 v2（已過找碴審閱）待使用者審
> **性質**：接手用文件，會過期，每段重寫。
> 為什麼做看 `docs/cockpit-spec.md`、怎麼做看 `~/.claude/CLAUDE.md` 與本 repo 的
> 設計文件、待辦看 `openspec/changes/<slug>/tasks.md`（尚未建立）。

## 0. 三十秒版本

1. 使用者要先審 `docs/superpowers/specs/2026-09-13-cockpit-mvp-design.md`；未點頭前
   不可 `openspec init`、不可寫程式。
2. 點頭後的第一步是 `openspec init --tools claude,codex,agents`，接著
   `/opsx:propose herdr-client`（不是直接做 attach）；tasks 第一批必須是設計文件 §11 的五個 spike。

## 1. 現在的狀態

- 已有：`docs/cockpit-spec.md`（概念）、設計文件 v2、`CONTEXT.md`、`docs/adr/0001`–`0005`、
  `docs/research/2026-09-13/`（兩份 HERDR schema、兩份研究報告、schema 比對腳本與輸出）。
  沒有任何程式碼、沒有 `openspec/`。
- 可用指令：`herdr api snapshot`（Windows 端目前狀態）、`herdr api schema --json`（匯出 schema）、
  `wsl.exe -e bash -lc '~/.local/bin/herdr status server'`（WSL 端狀態）、
  `markdownlint-cli2 "**/*.md"`（在 repo 根執行）。
- 測試與 gate：尚無測試。`.md` 2026-09-13 全 repo 0 issue。
- 版控：本機 git repo，main 分支，無 remote。

## 2. 立刻要做：使用者審閱設計文件 v2

看什麼：§1 切片（change 1 拆成 1a、1b 是審閱後的改動，使用者尚未確認）；§2.3 與 §4.2
「每 pane 訂閱狀態」的機制；§8.2 設定檔；§10.2 三個 Scenario 的通過條件。
使用者要求修改 → 改設計文件與對應 ADR，重跑 markdownlint，再請審。

## 3. 接著要做：`openspec init` 與 change 1a propose

- `openspec/config.yaml` 的 `context` 寫指標：設計文件、`CONTEXT.md`、`docs/adr/`、
  `docs/research/2026-09-13/`。
- change 1a `herdr-client`：spike（§11 五項）→ Connector 三實作 → Client（request／subscribe，
  含每 pane 訂閱型別）→ observer 子集型別 → contract test → 假 HERDR（client 層）。
- change 1b `attach-herdr-runtimes`：`cockpit-core`（模型、store、投影）→ `cockpit-herdr`
  （六階段連線迴圈、ReopenStatus、WSL 探測、假 HERDR 迴圈測試）→ `cockpit`（axum、WS、
  畫面、PWA）→ 真機驗收 A／B／F。
- 驗收方式：設計文件 §10。

## 4. 這一段踩過的坑（不要再推導一次）

- **HERDR 沒有全域 agent 狀態訂閱**：`pane.agent_status_changed` 訂閱必填 `pane_id`，
  `pane.updated` 只在 agent 名稱改變時發、狀態改變不發。要即時狀態就得每 pane 各訂一筆，
  pane 集合變了要重開訂閱連線。證據：`docs/research/2026-09-13/herdr-source-findings.txt`
  與設計文件 §2.3。
- **pane id 在 `pane.moved` 後會變**：任何以 pane id 當長期鍵的設計都會壞。
- **HERDR Windows 端不是 TCP**：`herdr.sock` 內容 `<pid>:<納秒時間戳>` 看起來像 port 加
  token，實際 pid 就是 server 的 pid；真正的 endpoint 是 named pipe，名稱＝檔案完整路徑。
- **每條 API 連線只服務一個 method**：不要設計成一條連線多工。
- **`done` 不是完成**：是「idle 且未被看過」。
- **Git Bash 呼叫 `wsl.exe` 傳 `/mnt/c/...` 路徑會被 MSYS 轉成 Windows 路徑**：
  前置 `MSYS_NO_PATHCONV=1`。
- **WSL 0.8.2 的 `herdr api schema` 不接受 `--json` 與 `--output` 併用**：用重導。
- **不得用 `herdr server stop` 測 Windows 端斷線**：會殺掉所有 pane。
- **markdownlint-cli2 要在 repo 根目錄跑**：核對輸出的 `Linting: N files` 不是 0。已於
  2026-09-13 以 `npm install -g markdownlint-cli2` 全域安裝（v0.23.2）；spec 第 27 節三行
  粗體前的 `markdownlint-disable-next-line MD036` 註解是刻意保留原作者寫法，不要移除。
- **研究產物要進 repo**：session 暫存會消失，已搬到 `docs/research/2026-09-13/`；之後的
  查證輸出照此模式存。

## 5. 已定但未執行的決策

| 決策 | 狀態 |
|---|---|
| 計畫層走 OpenSpec | 使用者已選，待設計審閱後執行 |
| MVP 三個功能切片，第一片拆 1a／1b 兩個 change | 拆分為審閱後的建議，待使用者確認 |
| 兩個 HERDR 第一天都接 | 已定；審閱者建議「Windows 先行」已否決，因使用者明確要兩端 |
| 五個 ADR | 已寫 |
| Tauri | 決定 MVP 後再評估，不做 |
| PWA 安裝條件、JSON Schema 驗證 crate | 已查證，結論在設計文件 §15 |

## 6. 之後的路

change 2 `pipeline-projection`（domain、TOML pipeline 設定、binding、Factory Floor）、
change 3 `live-output`（`pane.read` 以 revision 輪詢；WSL 端可能觸發 ADR-0002 方案 B）。
MVP 後評估 Tauri 包裝。

## 版本紀錄

| 版本 | 日期 | 變更 |
|---|---|---|
| 1 | 2026-09-13 | 初版：brainstorming 完成、設計文件待審 |
| 2 | 2026-09-13 | 找碴審閱後：每 pane 狀態訂閱、change 1 拆 1a／1b、研究證據進 repo |
