# visual-check.js 使用說明

> 日期：2026-09-26（direction-01-visual task 5.3）。對象：`docs/research/2026-09-23/visual-check.js`。
> 性質：腳本用法與段落代號參考文件，不是一次性驗收紀錄；內容依腳本檔頭註解與程式碼逐字整理。

## 用途

對 `cockpit --example ui_preview`（固定 fixture 起的 dashboard，不需要 HERDR）逐條驗證
`openspec/changes/direction-01-visual/specs/`（`cockpit-dashboard`、`live-output`）兩份 delta spec
的 scenario，以及本身的量測工具自我測試、CSS 靜態檢查（token 契約、選擇器清理）。headless
Chrome＋CDP，啟動與收尾寫法沿用 `docs/research/2026-09-19/live-output-check.js`。

## 事前準備

- **不需要 HERDR**：全程只對 `ui_preview` 操作，對 Windows／WSL 端 HERDR 皆不連線。
- **腳本不會自動 build**：先手動跑一次 `cargo build -p cockpit --example ui_preview`；找不到
  `target/debug/examples/ui_preview.exe` 時腳本會直接丟錯並在訊息裡印出這行指令，不會自己 build。
  `cockpit/assets/` 下的前端資源用 `include_str!`／`include_bytes!` 內嵌進執行檔，**每次改動這些
  檔案都要重新 build 才會反映在下一次跑腳本**。
- **需要 Chrome**：預設路徑 `C:\Program Files\Google\Chrome\Application\chrome.exe`，可用環境變數
  `COCKPIT_CHROME` 指到別的安裝位置。
- Node（沿用既有腳本慣例的 Node 22），不需要額外裝套件。

## 執行方式

```bash
cargo build -p cockpit --example ui_preview

node docs/research/2026-09-23/visual-check.js          # 全部段落
node docs/research/2026-09-23/visual-check.js V1,G1     # 只跑指定段落（逗號分隔，見下方代號表）
```

`TK1` 段是純靜態檢查（直接解析 `cockpit/assets/app/style.css` 原始碼），不需要 `ui_preview`／
Chrome，單獨挑 `TK1` 跑時最快。

## 環境變數

- `COCKPIT_CHROME`：Chrome 執行檔路徑，預設如上；沒有這個執行檔會在啟動時直接丟錯。
- `COCKPIT_PREVIEW_LISTEN`、`COCKPIT_PREVIEW_PUSH_MS`：腳本內部呼叫 `startPreview()` 時自動挑空埠
  （從 7830 起）並依段落需要覆寫推送間隔（多數段落用很長的間隔讓背景輪替靜止；`P1` 的兩個子案例另外
  覆寫成短間隔以觀察重畫），使用者不需要也不應該手動設定。
- `VISUAL_CHECK_STRICT_3_3`：direction-01-visual task 3.3 之前用來切換「印 PENDING」或「硬失敗」，
  task 3.3 之後兩種行為已統一為硬失敗，這個環境變數設或不設結果相同（程式碼註解裡的殘留說明，非
  目前有效的開關）。

## 段落代號對照表

| 代號 | 涵蓋內容（濃縮） | 對應 spec／task／裁決 |
|---|---|---|
| S1 | state 注入工具自我測試：擋掉背景推送、換上特製投影 | design D9 |
| S2 | 文字對比計算工具自我測試（WCAG 公式，含半透明疊層 compositing） | 供 CT1／D1／G1 共用的量測工具 |
| S3 | 命中測試工具自我測試（`elementFromPoint`，含「被別的元素蓋住」辨識） | 供 P1／LO1 等點擊斷言共用 |
| S4 | `finalSweep` 行程收尾所有權模型自我測試（還握著 ChildProcess 且未觀察到 exit 才可終止） | fix round 3／控制端 Ruling R14 |
| S5 | 「第一份真投影已畫出」判準的負對照 | task 2.3 fix round 1／Ruling R22／Codex C2 |
| S6 | 命令列段落代號驗證自我測試：拼錯或空的代號必須非零結束、不印 PASS | task 5.4 final review／Codex F4 |
| V1 | 桌面寬度（1536×1024）：不整頁捲動、長名稱不溢出、網格不撐破頁面、Live Output 可見、右欄不橫向捲動 | task 2.1／3.3／4.1 |
| V2 | 中等寬度（1100×900）：三欄嚴格排序（`bottom ≤ top`）與兩兩不重疊 | task 2.1 |
| V3 | 窄視窗單欄（700×900）：單欄依 8 個 `data-region` 逐一驗垂直順序與不重疊 | task 2.1 |
| V4 | 寬但矮的視窗（1280×650）：`≥1200px` 但高度 `<720px` 時仍三欄、取消固定一屏、允許整頁捲動 | task 2.1／design D3 |
| G1 | Factory Floor 換皮：Scenario D 畫面、未知 status 不破壞畫面、狀態不只靠顏色、節點換皮子斷言 | task 3.2 |
| G2 | Factory Floor `running` 節點沒有動畫＋節點按鈕對比（一般與 `:focus-visible`） | task 2.2 |
| P1 | Project 切換／選取跨重畫保留／鍵盤切換與焦點保留／不清錯誤不離開改綁／各狀態數量／沒有或兩個 Project | task 3.1 |
| R1 | 兩個 runtime 的畫面：topbar／statusbar／runtimes（agent 對照表、D11 右欄層次、改綁模式） | task 2.3／3.3 |
| CH1 | 通道非 connected 狀態持久化、「最後已知」、三態形狀可分、固定一屏頂列高度與溢出捲動處理 | task 2.3 fix round 1–5 |
| D1 | `done` 不使用成功色（含符號與文字色） | task 3.3 |
| U1 | pane `agent_status` 未知狀態不破壞畫面（右欄畫面層） | task 3.3 |
| RM1 | `prefers-reduced-motion: reduce` 時減少動態 | task 2.2 |
| FN1 | 不為字體發出網路請求 | task 2.2 |
| TK1 | 唯一色彩／字級 token 契約：純靜態解析 `style.css`，不啟動 preview／Chrome | task 2.2 fix round 1 |
| CT1 | 文字對比（banner／stale／all；含左緣條家族與 banner 折行時按鈕形狀） | task 3.4／4.2／5.2 |
| LO1 | Live Output 常駐面板：空狀態／點列／取消選取／下方內容可操作／鍵盤選定 | task 4.1 |
| CL1 | 清理：`style.css` 每一條選擇器至少在一種畫面狀態下對得到元素 | task 5.1 |
| DF1 | task 5.1 帶到後面的延後項目（topbar／statusbar／conn／scrollbar／pretty／cwd／focus／pin） | task 5.1 |
| FR1 | final review 修正：切換 Project 捲動位置歸零、Floor 邊界焦點框完整可見（真實 Tab 路徑）、文字放大 200% 按鈕不出節點 | task 5.4 final review／Codex F1–F3／Ruling R43 |

每個段落開頭都會印一行 `=== <代號>. <段落標題> ===`（例如 `=== V1. 1536x1024：桌面寬度不整頁捲動…
===`），要在大量輸出裡定位某一段時可以搜尋這個字串。

## 輸出格式

- `ok  <說明>` / `FAIL <說明>`：每一個 `check()` 斷言一行；`FAIL` 會被記進失敗清單。
- `PEND <說明>（PENDING(<task>)）`：屬於 Ruling R2 的暫緩子斷言，不算 PASS 也不計入失敗，代表這條
  要等到指定 task 才會補成真斷言。
- `[<timestamp>] <說明>`：`log()` 印的過程訊息（啟動／收尾／等待中等），不是斷言。
- 全部段落跑完後固定印 `RESULT: PASS` 或 `RESULT: FAIL (<n>)`；後者同時把 `process.exitCode` 設成
  `2`。腳本本身未預期的例外（`main()` 外層 `catch`）會印 `FAIL <錯誤>` 並把 `exitCode` 設成 `1`。
- 只跑部分段落（`node visual-check.js V1,G1`）時，`RESULT` 只反映有跑到的段落。
- 段落代號拼錯（例如 `V1X`、`v1`）、參數是空字串或只有逗號時，腳本在清殘留與啟動任何行程之前就印
  `FAIL 未知的段落代號…`／`FAIL 沒有選中任何段落…` 與 `RESULT: FAIL (段落代號)`，`exitCode` 設成 `2`
  （task 5.4 final review 之前會把所有段落跳過、照樣印 `RESULT: PASS`）。

## 清理行為

- 開跑前 `killLeftovers()`：用 image name `ui_preview.exe` 收掉殘留的測試執行檔，並用 PowerShell 找
  `user-data-dir` 含 `cockpit-chrome-` 字樣的 `chrome.exe` 收掉殘留的 headless Chrome——只認這兩種
  身分特徵，不動使用者平常在用的瀏覽器或其他無關行程。
- 每個段落自己的 `startPreview`／`startChrome` 一 spawn 成功就登記到 `SPAWNED_CHILDREN`，段落結束
  時透過 `stopPreview`／`stopChrome` 收尾；收尾依據是「還握著那個 Node `ChildProcess` 物件、且還沒
  觀察到它的 `exit` 事件」（`child.exitCode`／`child.signalCode` 皆為 `null`），不是 PID＋事後身分比對
  ——後者在同路徑重複啟動、身分完全相同的情況下分辨不出「這一次」是誰（fix round 3／控制端 Ruling
  R14 換設計的原因，三段一手來源見程式碼中 `finalSweep()` 上方的長註解）。
- `main()` 結尾一律跑 `finalSweep()` 當最後一道防線：對照 `SPAWNED_CHILDREN` 清單，逐一比照同樣的
  「未觀察到 exit 才終止」規則收尾；另外斷言預設埠 7770（`ui_preview` 的預設監聽埠）在腳本結束後沒有
  任何行程在 LISTENING。

## 已知陷阱

- **改了 `cockpit/assets/` 沒重新 build 就跑腳本**：畫面看到的仍是舊版前端，斷言結果不可信。腳本只
  檢查執行檔存不存在，不會幫你判斷它是不是最新的，也不會自動重 build。
- **段落裡大量「否定對照」是設計的一部分，不是腳本壞掉**：多數段落會在頁面裡臨時注入一個刻意違規的
  節點／樣式／請求，證明偵測器抓得到才把該段判定為綠燈；跑起來看到腳本主動製造「錯誤畫面」是預期
  行為（Ruling R3）。
- **`TK1` 與 `S6` 不啟動瀏覽器**：`TK1` 純文字解析 `style.css`，跟其他段落混跑不影響彼此，但單獨
  執行最快，適合改完 CSS token 後先跑這段；`S6` 只驗命令列參數。
- **只跑子集時 `RESULT` 不代表整體驗收狀態**：`node visual-check.js V1` 只反映 V1 是否綠燈，不代表
  其餘段落也是綠燈；要看完整驗收狀態要跑全部段落（不帶參數）。
- **`VISUAL_CHECK_STRICT_3_3`／部分段落內建的 PENDING 子斷言**：`PEND` 開頭的行不會讓 `RESULT`
  變成 FAIL，逐段確認驗收狀態時不能只看 `RESULT: PASS`，要留意有沒有夾雜 `PEND` 行（代表還有未落地
  的子斷言，需對照該行標的 `PENDING(<task>)` 決定要不要在意）。
