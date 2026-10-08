# repo-projects

## Why

零設定模式只能看 HERDR 的 pane，要用 Factory Floor 必須手寫 `cockpit.toml` 的 `[[project]]`（stages、workstream 的
`binding`、task），而且要知道 workspace label、pane label 這些一字不差的字串，改完還要關窗重開。使用者 2026-10-04 提出
這對一般使用者太困難（`docs/handover.md` 第 2 節）。2026-10-07～08 的 brainstorming 定案：Project 就是一個 git repo，
在畫面上按一下加入，之後 repo 裡的每個 pane 自動成為一條工作線。

## What Changes

- **Repo Project**：新的一種 Project，身分是一個 git repo（以 git 的共同 `.git` 目錄判定，同一個 repo 的所有 worktree
  算同一個）。由畫面加入，不寫在 `cockpit.toml`。
- **偵測到的 repo**：左欄 Project 分頁新增一區，列出目前 pane 的 cwd 所屬、但尚未加入的 git repo，各附「加入」鈕。
- **工作線由 pane 推導**：Repo Project 裡，cwd 落在該 repo（任一 worktree、任一 workspace、任一 runtime）的每個
  未 exited 的 pane 自動成為一條 workstream，綁定固定是那個 pane，每條 workstream 恰有一張 task。pane 從 HERDR 消失（關掉），
  那條 workstream 與它的 task 進度一起移除。pane 的 cwd 變動在下一次重新抓取 snapshot 後反映。
- **畫面操作**：加入（預設四個 stage，依介面語言命名）、移除、改顯示名稱、編輯 stage（改名、新增、刪除、排序），
  全部立即生效，不必重開。
- **agent 免帶 id 推進**：新增 `POST /api/agent/advance`，從 `X-Herdr-Pane-Id` 找到綁定該 pane 的唯一一張 task 並推進。
  舊的帶 id 端點不變。
- **狀態檔第 3 版**：`cockpit.state.json` 加入 Repo Project 的定義與進度，升為 `version: 3`；v1、v2 照常讀取並在下次
  寫入時升級。**BREAKING（僅降版）**：v0.1.3 以前的程式讀到 v3 檔會拒絕啟動。
- **零設定模式也有狀態檔**：沒有設定檔時，狀態檔放 `%LOCALAPPDATA%\ai-cockpit\cockpit.state.json`。
- **空狀態文字**：拿掉「需要重啟」，改指向「偵測到的 repo」。

## 非目標

- **不改手寫 `[[project]]`**：格式、驗證、綁定規則照舊（設計文件 §8.2；`pipeline-config`、`runtime-binding`）。
  手寫的與畫面加入的並列；id 撞名時手寫的優先。
- **不在畫面上編輯手寫的 project**，也不把畫面加入的 project 寫回 `cockpit.toml`。
- **一條工作線只有一張 task**：不做多張 task、task 之間的依賴、手動新增或刪除 task。需要的人繼續用手寫設定。
- **pane 關掉不保留紀錄**：不做「已結束」列，也不在同一個 worktree 重開 pane 時接回舊進度。
- **不追蹤 pane 搬移**：HERDR 的 `pane_moved` 會改變 pane id，搬移後視為舊 pane 消失、新 pane 出現。
- **不做 repo 黑名單或隱藏**：偵測到的 repo 全部列出。
- **不顯示分支名稱**：linked worktree 的工作線只標 worktree 資料夾名稱。
- **不新增任何 HERDR method**（設計文件 §2、ADR-0001）：pane 的 cwd 來自既有 snapshot；repo 身分用 git 查詢，經既有的
  `cockpit-git` 允許清單。
- **不熱載入 `cockpit.toml`**：手寫設定仍只在啟動時讀一次。

## Capabilities

### New Capabilities

- `repo-projects`：偵測 pane 所屬的 git repo、偵測到的 repo 清單、加入／移除／改名／編輯 stage 的端點與驗證、
  由 pane 推導 workstream 與 task、pane 消失時清除進度、Repo Project 與手寫 project 的 id 規則。

### Modified Capabilities

- `pipeline-config`：「狀態檔位置」改為零設定模式也有狀態檔、Project 清單為空時仍可能讀寫狀態檔。
- `pipeline-progress`：「狀態檔格式與持久化」升為 v3、加入 Repo Project 區段；「狀態檔載入與容錯」接受 v1～v3。
- `agent-reporting`：新增「免帶 id 推進」需求。
- `state-projection`：「Project 投影」加入 Project 種類與偵測到的 repo 清單。
- `cockpit-dashboard`：「Project 切換」加入偵測到的 repo 區、加入鈕與 Repo Project 的管理選單，空狀態文字改寫；
  「畫面操作」固定 pane 的工作線不顯示改綁；新增 worktree 標註。
- `runtime-binding`：「解析結果種類」加入 `source: pane`；「畫面覆蓋」排除固定 pane 的 workstream。
- `pipeline-domain`：「目前 task」加入 Repo Project 的規則（標記為 `none` 的 task 就是目前 task）。
- `ui-language`：「後端訊息代碼」的適用範圍加入 Repo Project 管理端點。

## Impact

- `cockpit-git`：新增一個查詢（`rev-parse --path-format=absolute --git-common-dir --git-dir --show-toplevel`）。
- `cockpit-core`：Domain 加入 Repo Project 定義與 pane→repo 對應；投影推導 workstream／task、輸出偵測到的 repo；
  新的 `Message` 變體。不依賴其他 crate 的限制不變（ADR-0003）。
- `cockpit`：repo 偵測背景工作、寫入服務改為永遠存在並接手 project 定義的變更、狀態檔 v3、零設定的狀態檔位置、
  新 HTTP 端點（加入、改、刪、agent 推進）套用既有來源檢查。
- 前端：`cockpit/assets/app/` 的左欄 Project 分頁、對話框、`i18n.js` 兩種語言。
- 文件：`CONTEXT.md`（Repo Project、偵測到的 repo）、`cockpit/README.md` 與 `README.md`（agent 免帶 id 推進）、
  新 ADR（工作線由 pane 推導、定義存狀態檔）、`CHANGELOG.md`。
- 驗收：新增 `docs/research/<日期>/repo-projects-check.js`；既有驗收腳本全數回歸。
