# tasks：direction-01-visual

> 執行路徑：SDD｜理由：16 個 task，一個 session 做不完；產品改動雖集中在 `cockpit/assets/`，但六支既有驗收腳本（含 2900 行的 `live-output-check.js`）與新驗收腳本要逐步跟著改，後面的 task（各區換皮、面板常駐）疊在外框與 token 之上，錯了返工貴；需要逐 task 的 Codex review 才放心往下疊。

通則（每個 task 都適用）：

- 結尾跑 `cargo fmt --check`、`cargo clippy -p cockpit --all-targets -- -D warnings`、`cargo test -p cockpit`；有改
  `cockpit/examples/ui_preview.rs` 時加跑 `cargo test -p cockpit --example ui_preview`；有改 `.md` 時在 repo 根跑
  `markdownlint-cli2 "**/*.md"`（核對 `Linting: N files` 不為 0）。輸出貼進回報。
- 前端資源以 `include_str!` 編進執行檔：改完 `cockpit/assets/` 一定先 `cargo build -p cockpit --example ui_preview`
  再跑任何腳本，否則腳本跑的是舊前端。
- **「既有六支腳本」**＝對 `ui_preview` 跑的 `docs/research/2026-09-15/reconnect-check.js`、`whatever-check.js`、
  `docs/research/2026-09-16/actions-check.js`、`channel-backoff-check.js`、`factory-floor-check.js`、
  `docs/research/2026-09-19/live-output-check.js`。**每個產品 task 結束時六支必須全綠**：只改被本 task 的改動打壞、且 spec
  已改變的斷言；腳本修改與產品修改分開 commit，commit 訊息逐條列出被改的斷言與對應的 spec scenario。不得放寬 spec 沒有改變的斷言；
  測試手法的前提被版面推翻時（例如以 `window.scrollTo` 捲動整頁），要改寫手法讓斷言保有辨識力，不得讓它變成恆真。
- **行為未變的 scenario 由既有腳本守住**（`visual-check.js` 不重做）：「連上即斷不歸零退避」「壞訊息不中斷」→
  `channel-backoff-check.js`；「通道重連」→ `reconnect-check.js`；「未知狀態不破壞畫面」→ `whatever-check.js`；「重畫不丟鍵盤焦點」
  「推進按鈕」「改綁模式跨重畫保留」等畫面操作 → `actions-check.js`；「從 workstream 選」「未綁定的 workstream 沒有入口」
  「改綁模式期間不改變選取」「鍵盤焦點跨重畫保留」「選取跨重畫保留」（pane）與「輪詢與顯示」「失敗與消失的呈現」→
  `live-output-check.js`。所以六支全綠即代表這些 scenario 在每個 task 後仍成立。
- `docs/research/2026-09-19/live-output-real-check.js` 要接 WSL 測試 server，不列入每 task 必跑；其受影響的斷言在 4.2 改寫，
  5.4 視環境實跑（指令見 `docs/handover.md` §1）。
- 同名 scenario 分屬兩個 capability（「選取跨重畫保留」：Project／pane；「未知狀態不破壞畫面」：pane agent 狀態 vs
  「未知 status 不破壞畫面」：task），`visual-check.js` 的段落名稱一律加 capability 前綴（例如 `dashboard/`、`live-output/`）。
- 跑腳本前清掉殘留的 `ui_preview`／headless Chrome（`factory-floor-check.js` 的 `--dump-dom` 對殘留程序敏感）；跑完依 PID 收尾，
  確認 7770 與 CDP port 沒有 LISTEN。
- 程式碼註解若要引用 task，寫成 `direction-01-visual task N.M`。`tasks.md` 由控制端統一勾。
- 每個改外觀的 task（2.1、2.2、2.3、3.1、3.2、3.3、3.4、4.1、4.2）結束後，由控制端的設計審核（1536／1100／700 三寬
  截圖＋CSS diff，對照 design.md 與本檔各 task 的驗收敘述）確認沒有偏離已定案的視覺語彙；有 finding 才回頭修正對應 task。

## 1. 基線與驗收鷹架

- [x] 1.1 記錄基線：在分支 `feat/direction-01-visual` 跑 `cargo test --workspace`、`cargo test -p cockpit --example ui_preview` 與既有六支腳本，把 passed／failed／ignored 數字與六支腳本的結果寫進 SDD ledger；驗收＝ledger 有這些數字，failed 為 0、六支腳本全綠
- [x] 1.2 新增 `docs/research/2026-09-23/visual-check.js`（headless Chrome＋CDP，沿用 `live-output-check.js` 的啟動、收尾與段落代號寫法，可只跑指定段落）：實作 design D9 的狀態注入工具（保存原 `window.onState`、換成空函數擋推送、以原函數畫特製投影）、對比計算工具（計算後文字色 vs 最近一層不透明背景）、命中測試工具，並為兩份 delta spec 的每個 scenario 各寫一段（以 `data-region` 定位區塊，見 design D2）；驗收＝對目前的前端執行時，工具自我測試段通過、各 scenario 段依預期失敗（RED），且腳本結束後沒有殘留程序

## 2. 外框、token 與頂列／底列

- [x] 2.1 外框：`index.html` 加 `.shell` grid 容器包住 `#app` 與 `#output`；`#app` 設 `display: contents`；`renderState` 的頂層輸出改為各區塊的平鋪 `DocumentFragment` 並帶 `data-region`，`paint()` 的 `replaceChildren` 呼叫點跟著改（design D2；`replaceChildren` 不接受陣列）；`index.html` 中 `#app` 的首次占位內容改成與新外框相容（首份投影到達前不跑版）；依 design D3 實作四種情形與各區內部捲動：`≥1200` 且 `≥720` 高固定一屏；`≥1200` 但 `<720` 高與 `760–1199` 寬皆取消固定高度、改整頁捲動（`style.css` 內新增，舊樣式暫留）；`live-output-check.js` 中以 `window.scrollTo` 捲動整頁的兩處手法（面板遮擋段約第 1011–1057 行、焦點不捲動段約第 2500–2560 行）依 design D9 改寫——前者改為命中測試、後者改為捲動 pane 列所在區塊的內部容器；驗收＝`visual-check.js` 的「桌面寬度不整頁捲動」「寬但矮的視窗」「中等寬度」「窄視窗單欄」「網格過寬時不撐破頁面」「長名稱不溢出」「重畫不重置區塊內部捲動位置」（V1；固定一屏下把 Factory Floor 與右欄內部捲動容器都捲到非 0 位置，整頁重畫後兩者捲動位置不變、頁面本身也未捲動）段通過，`cockpit/tests/http.rs` 全綠，既有六支腳本全綠
- [x] 2.2 視覺 token 與基礎：`style.css` 開頭定義 design D4 的 10 個色彩 token 與 D10 的 `--fs-*` 字級 token，並把既有規則改為取用 token；系統字體採 D11 的無襯線與等寬堆疊（含中文 UI 字型）與等寬數字；`:focus-visible` 2px `--accent` 外框；移除所有 `animation`／`transition` 並加 `prefers-reduced-motion` 保護段（design D5）；Factory Floor 與 Live Output 的切角（design D8，僅此兩面板；左欄與右欄改為 1px `--line` 框線；Factory Floor 的
  刻度改由 task 3.2 在 stage 欄首上緣實作，見 design D8）；`manifest.webmanifest` 的 `background_color`／`theme_color` 改為 `#091320`／`#101A2A`；驗收＝`visual-check.js` 的「減少動態」「不為字體發出網路請求」「running 節點沒有動畫」段通過，`http.rs` 的 manifest 測試全綠，既有六支腳本全綠
- [x] 2.3 頂列與底列：頂列顯示產品名稱（依 D10 降為面板標題級字級）與每個 runtime 的連線燈號（符號＋`id`＋狀態文字，色彩依 design
  D4；連線三態改用三種形狀＋顏色不變：`connected` 實心圓、`connecting` 空心圓、`disconnected` 叉，皆用 CSS 畫，頂列燈號與
  底列通道狀態共用同一套形狀——使用者 2026-09-24 裁決）；**頁面與 cockpit 服務的通道不是 `connected`（`disconnected` 或
  `connecting`）時，頂列每個 runtime 燈號的形狀維持最後已知連線狀態，但顏色一律改為 `--text-dim`、狀態文字前加「最後已知」，
  通道恢復 `connected` 後依 runtime 實際連線狀態還原顏色（使用者 2026-09-24 裁決，design D4）**；底列的通道狀態依 D4 加上
  「cockpit 服務」標籤並沿用連線三色與三種形狀，顯示 `version`；`window.onChannel` 仍能在不重畫的情況下更新通道狀態；
  驗收＝`visual-check.js` 的「兩個 runtime 的畫面」段中頂列與底列的斷言通過、「通道斷線時 runtime 燈號標示為最後已知」段
  通過；手動以停止／重啟 `ui_preview` 確認底列通道狀態先斷線後恢復（spec「通道重連」），並觀察頂列燈號在斷線期間確實改為
  `--text-dim`＋「最後已知」、恢復後還原，結果寫進回報

## 3. 各區換皮與 Project 切換

- [x] 3.1 Project 切換：`actions.js` 加 `selectedProject` 與 `select-project`（不遞增 `latestOp`、不清 `ui.error`，design D6）；左欄依 `projects` 順序列出 Project（名稱、warnings 數、各 `status` 數量依 D11 的計數 chip 規則：數量為 0 的不顯示、固定依「需要注意」程度排序 `failed`／`blocked`／`running`／`ready`／`pending`／`completed`／未知、每項為「符號＋`status`＋數字」的小 chip、不用中點串接，不出現「完成」）、選定標示、沒有 Project 的空狀態；`renderState` 依選取畫 Factory Floor，找不到就用第一個；`ui_preview` 共用的 fixture（`cockpit/tests/fixtures/projected-state.json`）已有兩個 Project（`cockpit`、`p`），直接用來驗切換，不改 fixture；驗收＝`visual-check.js` 的「切換 Project」「選取跨重畫保留」「鍵盤切換與焦點保留」「切換 Project 不清除錯誤也不離開改綁模式」「各狀態數量」「沒有 Project」「兩個 Project」段通過，`http.rs` 的「完成」禁字測試仍通過，既有六支腳本全綠
- [x] 3.2 Factory Floor 換皮：節點改為 `--surface` 底＋左側狀態色條＋`aria-hidden` 的狀態符號（依 D4 固定寬度、▶ 接 U+FE0E）＋`status` 文字（design D4 對照表）；`running` 靜止強調（2px `--accent` 外框＋柔光，僅 running 有柔光）；未知 status 用 `--text-dim` 與虛線外框；節點上的動作按鈕改為 D4 的第三層級中性樣式，不用任何狀態色；標題列顯示目前選定 Project 的 `name`（依 D10 為 20/600 主標題）；切角依 D8 只套用在 Factory Floor（task 2.2 已完成）；**刻度改在每個 stage 欄首上緣以 DOM 元素實作**（design D8，
  使用者 2026-09-24 裁決，取代原本標題列右側偽元素做法），跟著 stage 欄一起橫向捲動、永遠對齊，標題列右側不再放
  刻度；每個 stage 一格並與下方欄位對齊，接 stage 資料判斷該 stage 是否有 running task，有的話該格刻度較長且用
  `--text`（不用冰青）；橫向捲動時列首 `position: sticky; left: 0`、stage 欄首 `position: sticky; top: 0`，兩者不透明底色、列首右緣 1px `--line`（design D3）；驗收＝`visual-check.js` 的「Scenario D 的畫面」「未知 status 不破壞畫面」「狀態不只靠顏色」段通過，`factory-floor-check.js` 依本 task 改寫顏色斷言與逐字 HTML 字串斷言（節點多了狀態符號 `span`，例如「未知 status」段的 `html.includes(...)`，改成以 DOM 結構與屬性判斷）後全綠，既有六支腳本全綠
- [x] 3.3 右欄換皮：依 D11 的層次——右欄區塊一層框、runtime 之間以分隔線區隔（使用者 2026-09-25 裁決），workspace 用一行標題列（label、`#number`、彙總狀態）加上方 1px `--line` 分隔，tab 用縮排 8px 次標題（`--text-dim` 12px、不加框）；pane 列排成兩行（CSS grid 定位、不改 DOM 順序）：第一行狀態符號（依 D4 CSS 畫的 8px 圓）＋狀態文字＋agent＋pane id（等寬、靠右），第二行標題與 cwd（`--text-dim`、單行省略，完整內容放 `title`）；連線明細改為兩欄定義列表（左 `--text-dim` 標籤、右等寬值），不用中點串接；agent 狀態改為符號＋文字＋色彩（design D4，`done` 為 `--text` 加 `--accent` 空心點）；保留 `.status-*` class（`http.rs` 斷言 `.status-working`）；`.selected`／`.bind-target`／`.exited` 的樣式改用 token；最近事件換皮並使用等寬字體，`at` 只顯示時間部分（保留 `Z`，完整字串放 `title`）；驗收＝`visual-check.js` 的「兩個 runtime 的畫面」（右欄部分）「done 不使用成功色」「未知狀態不破壞畫面」段通過，既有六支腳本全綠
- [x] 3.4 錯誤與改綁提示：`.error-banner`／`.rebind-banner` 放在中欄上方 `data-region="banner"`，錯誤用 `--bad`、改綁提示用 `--accent`，文字對比合格；驗收＝`actions-check.js` 全綠，`visual-check.js` 在錯誤與改綁狀態下的「文字對比」子段通過

## 4. Live Output 面板常駐

- [x] 4.1 常駐與空狀態：`output.js` 面板骨架一開始即可見，沒有選取時顯示空狀態文字「還沒選 pane。點 runtime 清單裡的任一列，或按 Factory Floor 列首的「看輸出」。」（design D7）、隱藏「取消選取」與內容區；「取消選取」回到空狀態；`.is-open` 維持「有選取」的意義；面板佔中下區、不覆蓋其他區塊（design D7）；驗收＝`visual-check.js` 的「沒有選取時顯示空狀態」「點 pane 列」「取消選取」「面板打開時仍可操作頁面下方的內容」「鍵盤選定」段通過；`live-output-check.js` 依本 task 改寫「沒選取時面板不存在／收起」與按鈕改名（「關閉」→「取消選取」）相關斷言為空狀態後全綠（含 spec「沒有選取就不請求」「頻繁重畫不影響 Live Output 面板」對應段），既有六支腳本全綠
- [x] 4.2 過期標示改版：拿掉 `.is-stale` 的 opacity，改為內容文字 `--text-dim`、面板左緣 `--warn` 色條、標題列「過期」文字、原因訊息 `--warn`（design D7）；「pane 已不存在」與截斷提示換皮；驗收＝`live-output-check.js` 中 opacity 斷言改為比對上述計算值後全綠（含「pane 被關掉」「端點回 404」「runtime 斷線後恢復」對應段），`visual-check.js` 在過期狀態下的「文字對比」子段通過；`live-output-real-check.js` 對 `.output-text` opacity 的兩處斷言（約第 1261、1357 行）同步改為比對新的過期標示（本 task 只改不跑，5.4 視環境實跑）

## 5. 收斂與驗收

- [x] 5.1 清理：刪除 `style.css` 中已無元素使用的舊規則（以 `visual-check.js` 走訪 DOM 列出實際用到的 class 對照），確認沒有殘留 `animation`／`@keyframes`；驗收＝`grep -c "@keyframes" cockpit/assets/app/style.css` 為 0，`visual-check.js` 全部段落通過，既有六支腳本全綠
- [x] 5.2 全面視覺驗收：`visual-check.js` 全跑（含完整「文字對比」段：各種 task status、agent 狀態、連線狀態、warnings、錯誤、過期），並在 1536×1024、1100、700 三種寬度各存一張截圖到 scratch（不進 repo）；驗收＝全部段落通過，截圖路徑寫進回報，交由使用者目視驗收
- [x] 5.3 文件：`.gitignore` 加 `.superpowers/`；`docs/direction-01-visual-design.md` 狀態改為「已套用到正式介面（change direction-01-visual）」；`cockpit/README.md` 若有畫面描述則同步；新增 `docs/research/2026-09-23/visual-check.md` 說明腳本用法與段落代號；驗收＝`markdownlint-cli2 "**/*.md"` 0 issues
- [x] 5.4 全 gate 與整支分支 review：repo 根跑 `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test --workspace && cargo test -p cockpit --example ui_preview && markdownlint-cli2 "**/*.md" && openspec validate --all`，既有六支腳本與 `visual-check.js` 全跑；以 `codex-companion.mjs adversarial-review --wait --base main` 做整支分支審查，findings 先重現再處理；WSL 測試環境可用時另跑 `live-output-real-check.js`（不可用則在 ledger 註明未跑與原因）；驗收＝gate 輸出全綠、六支既有腳本與 `visual-check.js` 全綠、Codex 結論與處理紀錄寫進 SDD ledger（確認 log 沒有 `usage limit`／`Turn failed`）
- [x] 5.5 交接：依 `~/.claude/guides/handover-template.md` 整份重寫 `docs/handover.md`（active change、下一段為 change 5 .md 瀏覽、本段踩過的坑）；驗收＝`markdownlint-cli2 "**/*.md"` 0 issues，使用者確認目視驗收結果後才標記完成
