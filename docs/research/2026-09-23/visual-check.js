// visual-check.js：direction-01-visual 驗收鷹架（task 1.2）。headless Chrome＋CDP，寫法沿用
// docs/research/2026-09-19/live-output-check.js 的啟動、收尾與段落代號寫法（CDP class、
// startChrome／startPreview、killTree／tasklist 收尾判準、`node visual-check.js X,Y` 只跑指定
// 段落）。對 `ui_preview` 逐條驗兩份 delta spec 的 scenario（openspec/changes/direction-01-visual/
// specs/cockpit-dashboard/spec.md、specs/live-output/spec.md，以 HEAD 為準——含
// 2026-09-23 兩項中途裁決：Live Output「關閉」改名「取消選取」；固定一屏（不整頁捲動）的條件
// 改為寬 ≥1200 **且**高 ≥720，寬 ≥1200 但高 <720 時仍三欄但允許整頁捲動，新增的「寬但矮的視窗」
// scenario 留給 task 2.1 補段，本檔不搶做），定位區塊一律依 design D2 的 data-region
// （`topbar`／`projects`／`banner`／`floor`／`runtimes`／`events`／`statusbar`／`output`）
// ——這些屬性現在還不存在（task 2.1 才加），所以大多數 scenario 段現在會 RED，這是 design D9
// 「驗收做法」與本檔的預期狀態，不是腳本寫錯。
//
// fix round 1（2026-09-23，依控制端裁決處理 Codex F1–F3／F5–F7 與規格對照檢查 Critical／
// Important／兩個 Minor；F4 只採「用 preview 請求紀錄驗證輸出請求」半，文字斷言維持「取消
// 選取」；F8 只採「檔頭映射表列到既有腳本的段」半，不逐 scenario 拆檔）、fix round 2（同日，
// 處理 fix round 1 複審的 4 條：finalSweep 誤殺風險、F4／F7 未完成、F6 在 V3 漏掉的
// data-region）與 fix round 3（同日，控制端裁決 R14：finalSweep 的行程所有權模型換設計，
// 不再用 PID＋事後身分比對，改成「還握著 ChildProcess 物件、且還沒觀察到 exit 事件」）：
// 詳見 `.superpowers/sdd/tasks/task-1.2-report.md` 的對應章節。
//
// 段落清單（Ruling R1，控制端裁決，逐字核對 tasks.md 每個 task 的驗收清單）：
//   S1：state 注入工具自我測試（design D9：保存原 window.onState、換成空函數擋推送、以原函數
//       畫特製投影；fix round 1／F2：擋推送的證明改成計數至少攔到兩次真推送，不是固定等待）。
//   S2：對比計算工具自我測試（計算後文字色 vs 最近一層不透明背景，WCAG 公式，含半透明疊層
//       compositing；fix round 1／F1：另外驗證前景 rgba alpha 與 CSS opacity 鏈的合成）。
//   S3：命中測試工具自我測試（可見範圍中心點 elementFromPoint，含「被別的元素蓋住」的辨識力）。
//   S4：finalSweep 行程所有權自我測試（fix round 2 初版：用兩個探針證明「身分核對不符就不
//       終止」；fix round 3／R14：整個換成「還沒觀察到 exit 就是自己人」模型，三個情境——
//       (a) 活著的 child 會被終止、(b) 已經 exit 的 child 不會觸發任何終止命令（用
//       TASKKILL_PID_CALLS 的次數斷言）、(c) 收尾失敗（沒觀察到 exit）時仍留在追蹤集合；
//       fix round 4 加 (d) 父行程已 exit 但 port 仍 LISTENING 時保留追蹤，finalSweep 只警告、
//       taskkill 0 次）。
//   S5：「第一份真投影已經畫出」判準自我測試（task 2.3 fix round 1／Ruling R22／Codex C2）：
//       用 `Page.addScriptToEvaluateOnNewDocument` 在 render.js 賦值 `window.onState` 之前
//       裝一個攔截器，讓第一次真正呼叫延遲約 1.2 秒，證明延遲期間舊條件
//       （`.runtime-cards` 存在）會被靜態占位騙過而提早成立、新條件（`waitForFirstProjection`：
//       頂列出現 `[data-runtime]` 節點且 `#version` 等於 `/api/state` 的 version）不會，且新
//       條件最終真的等到延遲結束才通過（量測耗時 ≥900ms）。
//   S6：命令列段落代號驗證自我測試（task 5.4 final review／Codex F4）：parseSegmentArg() 對合法、
//       拼錯、大小寫不符、空字串與只有逗號的參數各驗一次，並實際用拼錯的代號與空字串跑一次本檔，
//       斷言非零結束、不印 RESULT: PASS、不走到清殘留。
//   V1：dashboard/桌面寬度不整頁捲動＋dashboard/長名稱不溢出＋dashboard/網格過寬時不撐破頁面
//       （1536×1024，高度 1024 已經 ≥720，不受新高度門檻影響；task 2.1）。「Live Output 在
//       視窗內可見」子斷言原依 Ruling R2 印 PENDING(4.1)；direction-01-visual task 4.1 轉成真斷言（空狀態
//       整塊在視窗內、高度 ≥240px、空狀態文字在面板內）。direction-01-visual task 3.3：
//       pane 標題／cwd 的 PENDING(3.3) 拿掉、一律硬失敗（VISUAL_CHECK_STRICT_3_3 不再有作用）；
//       加驗右欄兩個內層捲動容器沒有橫向捲動、長 cwd 單行省略或換行且 title 帶全文（附否定對照）。
//   V2：dashboard/中等寬度（1100×900；task 2.1；fix round 1／F6：改驗嚴格的
//       bottom≤top 排序＋兩兩區塊不重疊，不是單純比 top）。
//   V3：dashboard/窄視窗單欄（700×900；task 2.1；fix round 1／F6 初版：加驗單欄寬度、垂直
//       順序、兩兩區塊不重疊；fix round 2／F6 未完成：改用跟 V1 一致的 8 個 data-region 逐一
//       查（原本用 class 代理選擇器漏掉 projects／output／banner／statusbar），banner 不存在
//       時用 log() 說明跳過、不計入 ok 或 FAIL。section 開頭先 `window.liveOutput.select(...)`
//       選定一個 pane（同 V2），量的是「有選取」時面板在單欄版面裡的位置（task 4.1 起面板常駐，
//       沒選取時也量得到空狀態，這裡維持量有選取的版面）。
//   V4：dashboard/寬但矮的視窗（1280×650；task 2.1 新增段落，1.2 未做——design D9 原文：
//       「新增的『寬但矮的視窗』scenario 留給 task 2.1 補段」）：驗 design D3「`≥1200px` 但高度
//       `<720px`」情形——仍三欄、取消固定一屏高度、允許整頁捲動、Factory Floor 最小高度
//       240px、Live Output 改固定高度 `clamp(320px, 50vh, 560px)` 內部捲動，跟桌面寬度固定一屏
//       （V1）與 760–1199px 兩欄（沒有專屬段代號，靠 V2／design review 涵蓋）兩種情形區分開。
//   G1：dashboard/Scenario D 的畫面＋dashboard/未知 status 不破壞畫面＋dashboard/狀態不只靠顏色
//       （Factory Floor；task 3.2；fix round 1／規格檢查 Important：從舊版 G1 拆出，不再跟
//       task 2.2 的驗收項混在同一段代號）。direction-01-visual task 3.2：加驗節點換皮與 3.2 帶入
//       事項（子斷言標 [G1/node]／[G1/button]／[G1/glow]／[G1/title]／[G1/tick]／[G1/sticky]／
//       [G1/binding]／[G1/unknown]，各附否定對照），詳見段落開頭註解。
//   G2：dashboard/running 節點沒有動畫（Factory Floor；task 2.2；fix round 1 新拆出的段代號，
//       可獨立於 G1 選跑）。fix round 2／設計複審 N1：加驗節點上每顆按鈕的文字對實際背景
//       對比 ≥4.5:1。fix round 3／設計複審 P1：加驗 running 節點按鈕 :focus-visible 時，外框色
//       對節點底色對比 ≥3:1。這兩條原本靠 style.css 的暫時規則（.task-node .action-button 改用
//       --bg-deep）過關；direction-01-visual task 3.2 節點換成 --surface 底、刪掉暫時規則後，
//       兩條斷言保留不動（brief：3.2 驗收要列按鈕對比），改由 .action-button 的中性樣式本身通過。
//   P1：dashboard/切換Project＋dashboard/選取跨重畫保留＋dashboard/鍵盤切換與焦點保留＋
//       dashboard/切換Project不清除錯誤也不離開改綁模式＋dashboard/各狀態數量＋dashboard/沒有
//       Project＋dashboard/兩個Project（task 3.1；fix round 1／F2：「選取跨重畫保留」「鍵盤
//       切換與焦點保留」改成觀察 #version 真的變化至少兩次、每次變化後立刻斷言，不是固定
//       sleep）。
//   R1：dashboard/兩個runtime的畫面（task 2.3／3.3；子斷言標 topbar／statusbar／runtimes，
//       Ruling R4；fix round 1／F7 初版：topbar 逐一斷言 win／wsl 的 id、狀態文字與對應 token
//       色，statusbar 明確斷言確切 version 與通道狀態文字；fix round 2／F7 未完成：狀態文字
//       改成精確比對（優先讀專用節點，找不到才退回整顆燈號文字），不是 indexOf 子字串
//       ——原本的 indexOf('connected') 對 "disconnected" 也會誤判成 true）。direction-01-visual
//       task 3.3：runtimes 子斷言加驗 design D4 agent 對照表（[R1/agent]：fixture 三列＋注入五種
//       已知狀態與 whatever 各一列，符號形狀／顏色／8px／aria-hidden、文字與文字色）與 D11 右欄
//       層次（[R1/frame]／[R1/workspace]／[R1/tab]／[R1/pane]／[R1/connection]／[R1/events]，
//       以及改綁模式的 [R1/pane-rebind]），每一項各附一個注入違規樣式的否定對照。
//       task 3.3 fix round 1：[R1/frame] 反轉為「區塊一層框、runtime 之間上方分隔線、不畫四邊卡框」
//       （使用者 2026-09-25 裁決）；新增 [R1/align]（事件欄位與連線明細值欄對齊）、[R1/focus]（HERDR
//       焦點以「作用中」文字標記、不用底色）、[R1/cwd]（只差最後一段的 cwd 畫面可見文字不同）、
//       [R1/tabgap]（相鄰 tab 間距）；[R1/connection] 加驗 dd 為 --text-dim。V1 另加 R34（1100／700 寬
//       最近事件參與整頁捲動、內層不縱向捲動）。
//   CH1：task 2.3 fix round 1／2／3 回歸（Codex C1／使用者決定 I2／M3／Codex C3／設計審核
//       N1／N2／Ruling R26）：(a) Codex C1——onChannel('disconnected') 後呼叫
//       window.repaint()（模擬非 WS 觸發的重畫），底列應該仍是 disconnected、叉形狀還在；
//       (b) 使用者決定 I2（fix round 2 依 Codex fix round 1 review／N3 收斂顏色）——通道非
//       connected 期間，頂列燈號「四個」子節點（符號、id、「最後已知」、狀態文字）逐一斷言
//       都是 --text-dim，repaint 之後再驗一次，恢復 connected 後還原；(c) 使用者決定
//       M3——連線三態（connected 實心圓／connecting 空心圓／disconnected 叉）的計算樣式可以
//       互相區分；(d) Codex C3／Ruling R26（fix round 3，換設計拿掉「+N」徽章與 JS 量測，
//       改純 CSS：燈號永遠全部顯示，空間不夠先縮 id，再不夠橫向捲動）——固定一屏
//       （1200×720）、5 個 60 字元 id 的 runtime（其中一個 disconnected），三個情境
//       （connected；只呼叫 onChannel('disconnected') 不 repaint；視窗 1200→1536→1200
//       來回縮放）都驗證：每顆燈號存在、不是 display:none；符號與狀態文字捲得到（實際把
//       `.topbar-runtimes` 捲到那顆燈號的位置再量，不是只看目前視窗座標）；溢出時
//       computed overflow-x 是 auto 或 scroll；頂列高度等於 token；斷線的 runtime 燈號
//       必定存在（R2-1：不會因為排不下就被藏起來）；否定對照證明「捲得到」不是恆真（暫時
//       改成 overflow-x: hidden 應該讓至少一顆燈號不在可視範圍內）；另外注入兩個同前綴長
//       id 的 runtime，驗證空間足夠時不被 ellipsis 截斷、彼此分得出來（N2）；390 寬長 id
//       通道斷線時，產品名與每顆燈號都不重疊（R2-2）。fix round 4（Codex r3 medium／設計
//       複審 R3-1、R3-2、R3-3）：(e) 1536×1024、fixture 的兩個 runtime——燈號列沒有水平
//       溢出、沒有捲軸軌道（offsetHeight＝clientHeight），頂列高度＝--shell-topbar-h＝45；
//       每顆燈號 id 文字右緣到下一個可見文字（狀態文字）的距離 ≤16px（R3-1）；兩個否定
//       對照（overflow-x: scroll 必須讓「無捲軸」判準失敗、`.runtime-lamp-id { flex: 1 1
//       auto }` 必須讓間距判準失敗）。(d) 的三個溢出情境另外驗：捲軸帶不與任何燈號重疊、
//       每顆 id 的框寬 ≥ min(10ch, 文字自然寬)（R3-3）。fix round 5（Codex r4／Ruling R27）：
//       token 改驗「＝頂列行高＋24＋2」的推導值；(f) 文字放大 200%（根字級＋--fs-* 加倍）與
//       較寬字型（Courier New＋字距）× 兩種畫面，驗產品名與燈號完整在頂列內、頂列＝token、
//       不與其他區塊重疊、燈號不互相重疊、id ≥ min(10ch, 文字寬)、捲得到；否定對照放回寫死的
//       220px 燈號下限與 45px 頂列高度必須失敗。截斷偵測改成橫向或縱向溢出（line-clamp）。
//   D1：dashboard/done不使用成功色（task 3.3；fix round 1／F5：改讀 `--ok` token，未定義時退回
//       spec 色值 `#39D5AC`；只比較狀態文字的計算色；`doneColor !== completedColor` 直接比較，
//       不用背景差異當退路）。
//       task 3.3：加驗 [D1/symbol]——done 的點是 2px 冰青空心環、文字 --text，點與文字都不是成功色（附否定對照）。
//   U1：dashboard/未知狀態不破壞畫面（pane agent_status；task 3.3，Ruling R1）：whatever 列以
//       --text-dim 顯示原字串與虛線環、其他列不變、兩行排版不變、超長未知字串與超長 workspace
//       label 不讓右欄橫向捲動也不擠掉彙總狀態；兩個否定對照。DOM 層（class 白名單）仍由
//       whatever-check.js 守。
//   RM1：dashboard/減少動態（task 2.2；CDP Emulation.setEmulatedMedia）。
//   FN1：dashboard/不為字體發出網路請求（task 2.2）。
//   TK1：dashboard/唯一色彩與字級 token 契約（task 2.2 fix round 1；控制端 Ruling R20，
//       採 Codex high／medium；fix round 2 依 Codex medium 強化）。純靜態檢查，不啟動
//       preview／Chrome、不加任何新套件：宣告層級解析 cockpit/assets/app/style.css 原始碼，
//       斷言 10 個核心色彩 token 在 :root 內各恰好宣告一次且值與 spec 相符；其餘宣告不得
//       出現 hex／rgb()／rgba()／hsl()／hwb()／lab()／lch()／oklab()／oklch()／color()
//       或具名顏色；font-size 只能是 var(--fs-title|panel|dense|meta)。六個否定對照（hex／
//       rgb／hsl／具名顏色／重複核心宣告／非 token 字級）證明偵測器有牙齒。
//   CT1：dashboard/文字對比（task 3.4／4.2／5.2；子斷言標 banner／stale／all，Ruling R4）。
//       task 3.4：banner 子斷言加驗「左緣條家族」——左緣不被圓角彎成括號形
//       （borderTopLeftRadius／borderBottomLeftRadius 皆為 0px）、除顏色與框寬外形狀一致
//       （padding／圓角／display 等逐項比對），各附否定對照。
//       task 3.4 fix round 1（Codex medium／規格對照 Minor／設計 I1／M2；控制端 Ruling R36）：
//       (a) 底色／左框／其餘三邊框線改成兩則提示各自獨立比對精確 token 值（--surface 底、2px
//       solid --bad／--accent 左框、其餘三邊 1px solid --line），不再只驗「兩者彼此相等」
//       （原本兩者一起改成同一個錯誤值也會通過）；按鈕改用完整 D4 動作按鈕 probe（透明底、
//       四邊 1px --text-dim、文字 --text-dim）；新增三個否定對照（只改 error 底色、兩者一起
//       改左框寬度、按鈕加狀態色背景）。(b) 新增 banner-wrap 子斷言：1200×720 與 700×900 下，
//       錯誤文字折行時提示按鈕仍維持單行高度、寬度大於高度（不被壓成直排兩行），附否定對照
//       （還原成 fix 之前的預設 flex 行為——flex-shrink: 1＋white-space: normal 一起還原，
//       只拿掉 nowrap 不會重現，因為 flex-shrink: 0 本身已經讓按鈕不被擠壓——必須轉紅）；
//       同時驗證錯誤提示的 aria-hidden ✕ 符號存在、改綁提示不加。
//   LO1：live-output/沒有選取時顯示空狀態＋live-output/點pane列＋live-output/取消選取＋
//       live-output/面板打開時仍可操作頁面下方的內容＋live-output/鍵盤選定（task 4.1；fix
//       round 1／F4 半初版：空狀態與取消選取都額外用 preview.requests 驗證「不發輸出請求」
//       「取消後跨過一個輪詢週期請求不再增加」；按鈕文字斷言維持「取消選取」，已對齊 HEAD
//       spec；fix round 2／F4 未完成：空狀態改成直接斷言觀察期結束後總數是 0，不是用「載入
//       後的請求數」當基準——原本的基準寫法測不到「預載時已經偷發一筆」這種違規）。
//       direction-01-visual task 4.1：空狀態逐字文案、「畫出來」改用 getClientRects() 判斷（附否定對照）、
//       [LO1/fill] 空狀態與有選取時面板同高且內容撐滿（2.1 設計 M6，附否定對照）、點另一個 pane 列一律實測、
//       取消選取後面板不殘留上一個選取的文字。V1 的 R17 矩陣另外在 0／2 則提示時量空狀態高度＝有選取高度。
//       task 4.1 fix round 1：[LO1/window]（內容框透明無框、面板兩態皆 --bg-deep）、[LO1/mono]（標題等寬
//       14px／600）、[LO1/lh]（輸出行高 1.4）、[LO1/align]（fix round 2 改為空狀態／標題列／輸出內容的 content-box 起點對齊，附否定對照）、[LO1/focus]
//       （鍵盤 Enter 取消後焦點回到原 pane 列、否定對照、列消失時 Space 取消後退回 #output）。
//   CL1：清理——style.css 的每一條選擇器至少在一種畫面狀態下對得到元素（direction-01-visual task 5.1；
//       tasks.md「以 visual-check.js 走訪 DOM 列出實際用到的 class 對照」）。依序走訪預設投影、截斷、
//       過期＋原因、pane 已不存在、空狀態、點選 pane 列、兩則提示＋改綁、通道三種非 connected 字串、
//       防禦狀態（五種 agent 狀態＋未知、兩個 tab、connecting／未知連線、protocol 警告、未知 task
//       status）、沒有 Project、四種視窗；用 CSSOM 逐條選擇器（去掉 :hover／:focus-visible 等互動偽類
//       與偽元素）querySelector，斷言沒有任何一條從頭到尾對不到元素，並印出 DOM 出現過的全部 class；
//       否定對照：注入的 <style> 只抓出沒人用的兩條。fix round 1（Codex medium）：外層 @media／@supports
//       條件跟著遞迴，當下不成立就不算命中；每個狀態都在四種視窗取樣，另加 prefers-reduced-motion 取樣；
//       否定對照加「元素存在但唯一規則在不成立的 @media／@supports 裡」兩條，必須判定為死規則。
//   DF1：前面 task 帶到 5.1 的延後項目（direction-01-visual task 5.1），子斷言 [DF1/topbar]（R28：寬
//       ≥1200 不論高度頂列固定 token 高度；1200×719 四種情境）、[DF1/statusbar]（底列高度由字級推導）、
//       [DF1/conn]（連線符號隨字級等比放大、形狀可分）、[DF1/scrollbar]（每個內層捲動容器明設
//       scrollbar-width: thin）、[DF1/pretty]（連線明細值欄 text-wrap: pretty）、[DF1/cwd]（沒有前段不
//       產生空 span）、[DF1/focus]（#output 聚焦時切角焦點框不斷開，截圖像素連通判定）、[DF1/pin]
//       （提示行出現時貼底的內容仍貼底、往上捲不被拉回），各附否定對照。fix round 1：[DF1/topbar] 長 id
//       情境加驗燈號列 overflow-x 為 auto／scroll、真的溢出、捲到兩端首尾燈號完整可見（否定對照只改
//       overflow-x: hidden）；[DF1/pretty] 加最近事件 detail、專案警告、Live Output 失敗原因；新增
//       [DF1/radius]（每個區塊直角）；[DF1/pin] 加 markGone（pane 已不存在）路徑。
//   FR1：task 5.4 final review 修正波（Codex final F1–F3，Ruling R43）。[FR1/scroll-switch]：兩個都能
//       捲動的 Project，同一個 Project 的 UI 重畫與新投影重畫保留 Floor 捲動位置、切換 Project 後
//       新 Project 從 0 開始（切回原本的也是 0）。[FR1/focus-edge]：三種視窗下真實 Tab／Shift+Tab
//       走過 Floor 每一站，焦點框（border box 外推 outline-offset＋outline 寬）四邊都在捲動容器
//       可見範圍內（扣掉 sticky 欄首／列首）；鍵盤焦點留在 Floor 裡時重畫不把畫面拉回；兩個否定
//       對照（擋掉 render.js focusin 補捲、拿掉 scroll-padding 的焦點框餘裕）。[FR1/text200]：
//       文字放大 200%（同 CH1 (f)）× 700／1100／1536 寬 × 兩個 Project，每顆節點與列首按鈕都在
//       所屬框內；否定對照還原成不可斷行。
//   FT1／FT2／FT3：file-review task 3.5 新增，對應 openspec/changes/file-review/specs/cockpit-dashboard/
//       spec.md 的三個 scenario（先寫測試：file-review 4.x 前端落地前預期 RED，main() 結尾的段落彙總會把
//       這三段與既有段落分開列）。前端契約（#files／#review、role="tab"、data-path、data-viewer 等）
//       與 docs/research/2026-09-27/files-check.js 檔頭「前端契約」C1–C6 同一份，定位規則見下方
//       FILES_CONTRACT_JS（兩個腳本不共用模組，改契約時兩邊一起改）。
//     FT1：dashboard/分頁很多不撐破頁面（1280 寬、20 個 60 字元檔名的檔案分頁；檔案只寫在 ui_preview
//          的暫存副本）：重畫後分頁列內部橫向捲動、頁面沒有橫向捲軸、中欄（Factory Floor）寬度不變且
//          分頁區不超出中欄。
//     FT2：dashboard/頻繁重畫不影響檔案分頁（COCKPIT_PREVIEW_PUSH_MS=100、long.md 往下捲）：3 秒後分頁區、
//          分頁列、分頁、tabpanel、檢視器與其下所有內容子節點都沒被換掉，捲動容器仍是原本那一個且在頁面上，
//          捲動位置（讀目前的捲動容器）與目前分頁不變。fix round 1（Codex finding 2）：加內容子節點與捲動容器的
//          比對；偵測器正負對照先在合成 DOM 上跑（重設檢視器 innerHTML 必須轉紅），前端落地後另在真頁面跑一次。
//     FT3：dashboard/Markdown檢視遵守色彩與對比（README.md：標題、段落、連結、表格、行內程式碼、程式碼
//          區塊）：Markdown 內容區每個含文字的元素對比 ≥4.5:1（沿用 textContrast），文字色、實際背景色與
//          元素自身不透明背景色都是 10 個色彩 token 之一；排除 PDF canvas 與 iframe 內容（design D11）；
//          附否定對照（注入 #ff0000 文字必須被抓到）。
//
// Ruling R3（1.2 的 RED 放寬）：已經在目前前端就滿足的 scenario 段——目前判斷會出現在 V3、D1、
// FN1（各自理由見段落內註解），各自在段落內附一個「否定對照」自我測試：在頁面裡注入一個刻意
// 違規的合成節點／請求，證明偵測器抓得到，才把該段判定為綠燈；其餘段落預期在目前前端下有真的
// RED 斷言（見各段落內的比對說明）。V2 在 fix round 1 之前 report 誤植為 RED，實測其實也是
// 巧合綠燈，已修正（見 report「fix round 1」）。
//
// 通則點名由既有六支腳本守住、不在本檔重做的 scenario（逐條列到哪支腳本的哪一段，
// fix round 1／F8 後半）：
//   - 「連上即斷不歸零退避」「壞訊息不中斷」→ docs/research/2026-09-15/channel-backoff-check.js
//     （整支腳本單一流程，沒有分段代號）。
//   - 「通道重連」→ docs/research/2026-09-15/reconnect-check.js（整支腳本單一流程，沒有分段
//     代號）。
//   - 「未知狀態不破壞畫面」（pane agent_status，跟本檔 G1 的「未知 status 不破壞畫面」是
//     cockpit-dashboard 兩個不同 Requirement 的同名／近似名 scenario，見 brief）→
//     docs/research/2026-09-15/whatever-check.js（整支腳本單一流程，沒有分段代號）；task 3.3 換皮後
//     的畫面層另由本檔 U1 守（Ruling R1 把它列為 3.3 驗收段）。
//   - 「推進按鈕」「改綁模式跨重畫保留」→ docs/research/2026-09-16/actions-check.js 的 A 段
//     （`=== A. ui_preview（COCKPIT_PREVIEW_PUSH_MS=100）===`）。「重畫不丟鍵盤焦點」沒有
//     單獨情境標題，分散驗在 A 段「頻繁重畫時按鈕仍有效」（連按 10 個不同 task 按鈕跨重畫）與
//     B 段（`=== B. harness ===`）鍵盤觸發＋409 錯誤跨重畫保留的焦點檢查。
//   - 「從 workstream 選」「未綁定的 workstream 沒有入口」「改綁模式期間不改變選取」→
//     docs/research/2026-09-19/live-output-check.js 的 C 段
//     （`=== C. 選取 UI：點 pane 列／從 workstream 選／未綁定沒入口／取消選取／回歸檢查 ===`）。
//   - 「選取跨重畫保留」（pane，跟本檔 P1 的「選取跨重畫保留」是 cockpit-dashboard／
//     live-output 兩個 capability 的同名 scenario，見 brief）→ live-output-check.js 的 D 段
//     （`=== D. COCKPIT_PREVIEW_PUSH_MS=100：選取跨重畫保留／頻繁重畫不影響面板 ===`）。
//   - 「鍵盤焦點跨重畫保留」→ live-output-check.js 的 R 段
//     （`=== R. COCKPIT_PREVIEW_PUSH_MS=100：整頁重畫不得丟失鍵盤焦點 ===`）。
//   - 「輪詢與顯示」（沒有選取就不請求、請求不堆積、內容不被當成 HTML、截斷提示、內容跟上、
//     舊回應不蓋掉新選取、往上捲不被拉回、停在底部會跟著走）→ live-output-check.js 的
//     A／B／F／G／H／I 段。
//   - 「失敗與消失的呈現」（pane 被關掉、端點回 404、runtime 斷線後恢復）→
//     live-output-check.js 的 J／K／L 段。
//
// 用法（repo 根，需先 `cargo build -p cockpit --example ui_preview`）：
//   node docs/research/2026-09-23/visual-check.js        # 全部段落
//   node docs/research/2026-09-23/visual-check.js V1,G1   # 只跑指定段落（代號拼錯或空＝非零結束，見 S6）
//
// 清理：main() 開頭先清掉殘留的 ui_preview.exe（依 image name，這是本專案專用的測試執行檔，
// 唯一可能的誤殺對象就是別的視窗手動跑的同一支 ui_preview.exe）與殘留的 headless Chrome（只殺
// user-data-dir 含 `cockpit-chrome-` 字樣的 chrome.exe process，不動使用者平常在用的瀏覽器）；
// 每段自己的 preview／chrome 用 killTree／tasklist 判準收尾（同 live-output-check.js）。
const os = require('node:os');
const { spawn, spawnSync } = require('node:child_process');
const path = require('node:path');
const fs = require('node:fs');
const net = require('node:net');

const REPO = path.resolve(__dirname, '..', '..', '..');
const UI_PREVIEW_EXE = path.join(REPO, 'target', 'debug', 'examples', 'ui_preview.exe');
const FIXTURE_PATH = path.join(REPO, 'cockpit', 'tests', 'fixtures', 'projected-state.json');
const CHROME =
  process.env.COCKPIT_CHROME || 'C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe';

// 大多數段落不需要頻繁重畫，用一個很長的推送間隔讓 ui_preview 的「每個推送輪替一個 pane 狀態」
// 背景行為在段落執行期間（遠小於 10 分鐘）實質靜止，避免跟腳本自己讀到的畫面互相干擾；需要
// 頻繁重畫的段落（P1 的「選取跨重畫保留」「鍵盤切換與焦點保留」）另外覆寫成短間隔。
const STABLE_PUSH_MS = '600000';

// design D4 色票（docs/direction-01-visual-design.md／spec cockpit-dashboard「Direction 01
// 視覺語彙」）。用 rgb() 字串比對是因為 getComputedStyle 一律回傳正規化過的 rgb()／rgba()，
// 不會回傳原始 hex；G1／R1／D1 共用同一份，避免各段各自手key hex→rgb 換算，容易手誤。
const SPEC_COLORS = {
  bgDeep: 'rgb(9, 19, 32)', // #091320
  bgBase: 'rgb(16, 26, 42)', // #101A2A
  surface: 'rgb(20, 35, 56)', // #142338
  text: 'rgb(229, 237, 243)', // #E5EDF3
  textDim: 'rgb(163, 183, 201)', // #A3B7C9
  accent: 'rgb(99, 213, 232)', // #63D5E8
  line: 'rgb(41, 66, 88)', // #294258
  ok: 'rgb(57, 213, 172)', // #39D5AC（design D4 的 --ok token 尚未落地時的 spec 色值退回）
  warn: 'rgb(233, 188, 115)', // #E9BC73
  bad: 'rgb(244, 114, 121)', // #F47279
};

// 逼 banner 折行用的長錯誤文字（原本只在 V1 用；task 3.4 fix round 1／設計 I1 hoist 成模組層級，
// CT1 的「banner-wrap」子段共用同一份，不重複維護一份幾乎一樣的長字串）。
const LONG_ERROR_TEXT =
  '模擬一段很長的錯誤訊息，用來逼中欄在較窄的寬度下把改綁提示與錯誤提示都折成至少兩行，' +
  '藉此驗證 banner 在固定一屏、可用高度最緊繃的情況下仍然完整顯示、不會被裁切、也不會把 ' +
  'Live Output 的份額往下拉，這段文字刻意寫得夠長，確保每一個測試尺寸下都真的會換行。';

// 命令列段落代號（task 5.4 final review／Codex F4）：原本直接拿去過濾，拼錯的代號會讓所有段落
// 都被跳過、最後照樣印 RESULT: PASS（假綠燈）。改成 main() 一開始先用 parseSegmentArg() 驗證：
// 每個代號都要在 PARTS 裡、至少選中一段；不合格就以非零狀態結束，不清殘留、不啟動任何行程。
// 沒給參數＝全部段落。ONLY 在驗證通過後才設定（null＝全部）。
const SEGMENT_ARG = process.argv[2];
let ONLY = null;
function shouldRun(code) {
  return !ONLY || ONLY.includes(code);
}
function parseSegmentArg(arg, knownCodes) {
  if (arg === undefined) return { ok: true, codes: null };
  const codes = String(arg)
    .split(',')
    .map((s) => s.trim())
    .filter((s) => s !== '');
  if (codes.length === 0) {
    return { ok: false, message: `沒有選中任何段落（參數 ${JSON.stringify(arg)}）；可用代號：${knownCodes.join(',')}` };
  }
  const unknown = codes.filter((c) => !knownCodes.includes(c));
  if (unknown.length > 0) {
    return { ok: false, message: `未知的段落代號：${unknown.join(',')}；可用代號：${knownCodes.join(',')}` };
  }
  return { ok: true, codes };
}

const failures = [];
function check(cond, label) {
  console.log(`${cond ? 'ok  ' : 'FAIL'} ${label}`);
  if (!cond) failures.push(label);
  return cond;
}
// PENDING 子斷言（Ruling R2）：不得印 PASS，也不計入 failures（代價：這條子斷言在 2.1～4.1
// 之間不受保護，由控制端 final review 對照）。
function pending(label, taskRef) {
  console.log(`PEND ${label}（PENDING(${taskRef})）`);
}
const log = (s) => console.log(`[${new Date().toISOString()}] ${s}`);
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

// ---------------------------------------------------------------------------
// 行程管理（同 live-output-check.js 的判準）
// ---------------------------------------------------------------------------

function pidStillRunning(pid) {
  const r = spawnSync('tasklist', ['/FI', `PID eq ${pid}`, '/NH'], { encoding: 'utf8' });
  return typeof r.stdout === 'string' && r.stdout.includes(String(pid));
}

// fix round 3／R14 item 5(b)：所有「用 PID 送終止命令」的路徑都經過這裡，記一筆到
// TASKKILL_PID_CALLS——S4 自我測試用這個陣列的長度變化，直接斷言「已經 exit 的 child 不會
// 觸發任何終止命令」，不是只看行為結果（行程還活著／死了）這種間接推論。只增不減，記憶體成本
// 可忽略（整支腳本一次執行最多幾十筆）。
const TASKKILL_PID_CALLS = [];
function taskkillPid(pid) {
  TASKKILL_PID_CALLS.push(pid);
  return spawnSync('taskkill', ['/PID', String(pid), '/T', '/F'], { encoding: 'utf8' });
}

// fix round 3：只有在還沒觀察到 Node 的 'exit' 事件（exitCode 與 signalCode 皆為 null）時才
// 送終止——這正是「這個 PID 現在仍然保證是我們自己 spawn 的那個行程」的唯一依據（見
// finalSweep() 上方大段引用來源的說明）；exitCode／signalCode 任一個非 null 就代表 Node 已經
// 觀察到這個行程結束，這個時間點之後這個 PID 數字在 Windows 上可能已經（或即將）被別的行程
// 拿去用，不該再對它送任何終止命令。
function killTree(child, label) {
  if (!child || child.exitCode !== null || child.signalCode !== null) return;
  taskkillPid(child.pid);
  check(!pidStillRunning(child.pid), `${label} PID ${child.pid} 已終止（tasklist 查無此 PID）`);
}

function isPortListening(port) {
  const r = spawnSync('netstat', ['-ano'], { encoding: 'utf8' });
  const needle = `127.0.0.1:${port} `;
  return (r.stdout || '')
    .split('\n')
    .some((line) => line.includes(needle) && line.includes('LISTENING'));
}

function pickPort(start, avoid = []) {
  let port = start;
  while (isPortListening(port) || avoid.includes(port)) port += 1;
  return port;
}

// 跑腳本前清掉殘留程序（brief 通則）：只殺這支測試專用的 ui_preview.exe，與 user-data-dir 帶
// `cockpit-chrome-` 字樣（本檔＋既有腳本共用的命名慣例）的 headless Chrome，不動使用者平常在
// 用的瀏覽器或其他無關行程。
function killLeftovers() {
  log('清掉殘留的 ui_preview.exe（若有）...');
  spawnSync('taskkill', ['/IM', 'ui_preview.exe', '/F', '/T'], { encoding: 'utf8' });
  log('清掉殘留的 headless Chrome（user-data-dir 含 cockpit-chrome- 字樣，若有）...');
  const ps = spawnSync(
    'powershell',
    [
      '-NoProfile',
      '-NonInteractive',
      '-Command',
      "Get-CimInstance Win32_Process -Filter \"Name='chrome.exe'\" | " +
        "Where-Object { $_.CommandLine -like '*cockpit-chrome-*' } | " +
        'ForEach-Object { Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue }',
    ],
    { encoding: 'utf8' }
  );
  if (ps.error) {
    log(`（PowerShell 清理殘留 Chrome 失敗，忽略：${ps.error.message}）`);
  }
}

// fix round 1／Codex F3：每個 preview／chrome 行程一 spawn 成功（不等 readiness）就登記，
// main() 結尾再做一次總清查——就算某個段落的 startChrome／startPreview 在 readiness 失敗時
// 自己的收尾漏了什麼（或未來改動不小心繞過那段收尾邏輯），這裡仍是最後一道防線，不會讓行程
// 一路活到腳本結束後才被使用者發現。
//
// fix round 2／Codex：上一版用「PID＋事後用 PowerShell 核對 image name／command line」判斷
// 能不能終止，核對通過後到真正送出 taskkill 之間仍有競態視窗，而且同一路徑啟動的
// ui_preview.exe 每次身分完全相同（沒有參數可以區分「哪一次」），核對本質上分辨不出來。
//
// fix round 3（控制端裁決 R14，換設計，不再補 PID 身分比對）：所有權唯一依據改成「手上還握著
// 那個 Node ChildProcess 物件、且還沒觀察到它的 'exit' 事件」——只要 `child.exitCode` 與
// `child.signalCode` 皆為 `null`，這個 PID 數字保證還是我們自己 spawn 的那個行程，可以安全
// 終止；一旦觀察到 exit（兩者任一變成非 null），就絕不再對這個數字送任何終止命令，因為 Windows
// 隨時可能把它重新配給別的程序。
//
// 這個保證鏈的三段一手來源（2026-09-24 查證）：
//   1. Node.js 官方文件（child_process）：`subprocess.exitCode`／`subprocess.signalCode` 在
//      子行程還在跑時皆為 `null`；'exit' 事件送出的當下才會被設定（兩者恰有一個非 null）。
//      https://nodejs.org/api/child_process.html#event-exit
//      https://nodejs.org/api/child_process.html#subprocessexitcode
//   2. libuv 原始碼（Windows 實作，src/win/process.c）：`CreateProcess` 回傳的 process
//      HANDLE 存在 `uv_process_t.process_handle`，從 spawn 起就保持開啟，直到偵測到行程
//      結束、執行 `uv_process_proc_exit`（也就是驅動 Node 送出 'exit' 事件的那個時間點）才
//      呼叫 `CloseHandle` 關閉——換句話說，Node 還沒送出 'exit' 事件之前，libuv 手上那個
//      Windows process handle 一定還開著。
//      https://github.com/libuv/libuv/blob/v1.x/src/win/process.c
//   3. Microsoft（Raymond Chen，官方 Microsoft DevBlogs「The Old New Thing」，2011-01-07，
//      "When does a process ID become available for reuse?"）：一個 process 物件（連同它的
//      PID）要等「行程已經結束」**且**「所有指向它的 handle 都已經關閉」兩個條件同時成立，
//      核心才會把這個物件銷毀、PID 數字才會被釋放供重用；只要還有人握著開著的 handle，PID
//      就不會被別的行程拿去用。
//      https://devblogs.microsoft.com/oldnewthing/20110107-00/?p=11803
// 串起來：`child.exitCode === null && child.signalCode === null` ⟹ Node 還沒送出 'exit'
// ⟹（來源 2）libuv 還沒執行 `uv_process_proc_exit`、那個 CreateProcess handle 還開著
// ⟹（來源 3）核心保證這個 PID 數字沒有被重新配給別的行程。三段都是一手來源（Node 官方文件、
// libuv 專案原始碼、Microsoft 官方工程部落格），不是待查證的推論。
const SPAWNED_CHILDREN = []; // { child, label, port }
function trackChild(child, label, port) {
  SPAWNED_CHILDREN.push({ child, label, port });
}
// fix round 3：只有「已經觀察到這個 child 的 exit」才可以移除追蹤——收尾（taskkill／port
// 檢查）失敗時必須保留，讓 finalSweep() 還有機會重試（Codex 這輪額外指出的 medium finding：
// fix round 2 的版本不管收尾成不成功都無條件 untrack，等於真正殘留的行程也會被從清單移除，
// 最後一道防線形同虛設）。
function untrackChild(child) {
  const idx = SPAWNED_CHILDREN.findIndex((e) => e.child === child);
  if (idx !== -1) SPAWNED_CHILDREN.splice(idx, 1);
}
function hasObservedExit(child) {
  return child.exitCode !== null || child.signalCode !== null;
}
// 等到觀察到 child 的 'exit' 事件、或逾時；回傳是否真的觀察到 exit。呼叫端據此決定要不要
// untrackChild()——沒觀察到就不能移除追蹤。
function waitForChildExit(child, timeoutMs) {
  if (hasObservedExit(child)) return Promise.resolve(true);
  return new Promise((resolve) => {
    const timer = setTimeout(() => {
      child.removeListener('exit', onExit);
      resolve(hasObservedExit(child)); // 逾時前也可能剛好觀察到，保險再查一次。
    }, timeoutMs);
    function onExit() {
      clearTimeout(timer);
      resolve(true);
    }
    child.once('exit', onExit);
  });
}
// fix round 4／Codex r3 medium：四個收尾路徑（startChrome 的 cleanupOnFailure、stopChrome、
// startPreview 的失敗路徑、stopPreview）共用的「送出終止之後」判斷。等 exit、查 port，回傳兩者
// 讓呼叫端各自用原本的文字做 check()。untrack 的條件由這裡統一決定：只有「已觀察到 exit」
// 且「port 不再 LISTENING」才移除。fix round 3 只看 exited，父行程已 exit、孫行程還占著 port
// 時照樣 untrack，finalSweep() 的殘留警告分支就永遠走不到。已 exit 但 port 仍 LISTENING 時
// 保留項目：finalSweep() 只會警告，不會對任何 PID 送終止（R14 第 4 點）。
async function settleTrackedChild(child, port, label, timeoutMs = 5000) {
  const exited = await waitForChildExit(child, timeoutMs);
  const portListening = port !== undefined && isPortListening(port);
  if (exited && !portListening) {
    untrackChild(child);
  } else if (!exited) {
    log(`${label}：沒有觀察到子行程 exit 事件，保留在追蹤集合，留給 finalSweep() 重試`);
  } else {
    log(`${label}：子行程已 exit，但 port ${port} 仍在 LISTENING，保留在追蹤集合，留給 finalSweep() 警告（不會終止任何 PID）`);
  }
  return { exited, portListening };
}
// assertZeroKilled=false 供 S4 自我測試用：main() 結尾呼叫時維持預設 true。
function finalSweep(assertZeroKilled = true) {
  // 不清空——只有真正處理掉的（已觀察到 exit、或這裡補送終止過）才移除；還沒觀察到 exit 且
  // 這裡也還沒補送終止成功的極端情況（理論上不會發生：只要送過 taskkill /F，行程就會結束並
  // 觸發 exit 事件）不會留下無限增長的清單，因為每一輪都會對「還沒 exit」的項目送終止。
  const entries = SPAWNED_CHILDREN.slice();
  let killed = 0;
  let residualWarnings = 0;
  for (const { child, label, port } of entries) {
    if (hasObservedExit(child)) {
      // 已經觀察到 exit：這是正常結案，不需要再對這個 PID 做任何事（那個數字可能早就被
      // Windows 拿去用了）。用「port 是否仍在 LISTEN」當作「有沒有漏網孫行程」的間接跡象——
      // 偵測到的話不對任何 PID 動手（我們沒有那個孫行程的 ChildProcess handle，沒有前面
      // 那條保證鏈可用，貿然用 PID 動手就是 fix round 2 被抓到的同一種風險），只印警告、留給
      // 使用者人工排查。
      if (port !== undefined && isPortListening(port)) {
        residualWarnings += 1;
        log(`最終清查警告：${label}（父行程 PID ${child.pid}）已經觀察到 exit，但 port ${port} 仍在 LISTENING——可能有漏網的孫行程，不會嘗試終止（沒有這個行程的 handle，無法比照上面的保證鏈確認 PID 沒被重用），需要人工排查（netstat -ano 找出目前占用 port ${port} 的 PID）`);
      }
      untrackChild(child);
      continue;
    }
    // 還沒觀察到 exit：依上方引用來源，這個 PID 保證仍然是我們自己 spawn 的那個行程，可以
    // 安全終止。
    killed += 1;
    log(`最終清查：${label}（PID ${child.pid}）尚未觀察到 exit 事件，補送終止`);
    taskkillPid(child.pid);
    // 不在這裡同步等 exit 事件（finalSweep 本身不是 async，S4 需要同步可測的行為）；
    // untrackChild 留給下一次有人呼叫 finalSweep（或這裡本來就是 main() 最後一次呼叫，不會
    // 再有下一次）——taskkill /F 對一個真的還活著、身分保證正確的行程幾乎不會失敗，這裡不做
    // 過度工程化的重試迴圈。
  }
  if (assertZeroKilled) {
    check(
      killed === 0,
      `最終清查：所有曾經 spawn 過、且各自段落收尾時沒有觀察到 exit 就從追蹤集合移除的行程，應該是 0 個需要補送終止（實際 ${killed} 個）`
    );
    check(
      residualWarnings === 0,
      `最終清查：不應該有「已觀察到 exit 但 port 仍在 LISTEN」的殘留跡象（實際 ${residualWarnings} 個，詳見上面的警告 log；這種情況不會強制終止任何 PID，需人工排查）`
    );
  } else {
    log(`最終清查（S4 自我測試模式，不對數量斷言）：補送終止 ${killed} 個、殘留跡象警告 ${residualWarnings} 個`);
  }
  return { total: entries.length, killed, residualWarnings };
}

// ---------------------------------------------------------------------------
// CDP（同 live-output-check.js；click／pressKey 逐字沿用，含 fix round 1 的 settle delay 與
// R17 的輪詢重試，理由見該檔案檔頭「fix round 1 的根因調查」）
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
    const r = await this.send('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true });
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

    const base = { x: rect.x, y: rect.y, button: 'left', clickCount: 1 };
    await this.send('Input.dispatchMouseEvent', { type: 'mouseMoved', x: rect.x, y: rect.y });
    await this.send('Input.dispatchMouseEvent', { type: 'mousePressed', ...base });
    if (holdMs > 0) await sleep(holdMs);
    await this.send('Input.dispatchMouseEvent', { type: 'mouseReleased', ...base });
    return true;
  }
  async pressKey(key, code, windowsVirtualKeyCode, text) {
    await this.send('Input.dispatchKeyEvent', { type: 'keyDown', key, code, windowsVirtualKeyCode, text });
    await this.send('Input.dispatchKeyEvent', { type: 'keyUp', key, code, windowsVirtualKeyCode });
  }
}

async function startChrome(cdpPort, url, label, windowSize = '1536,1024') {
  const udd = fs.mkdtempSync(path.join(os.tmpdir(), `cockpit-chrome-${label}-`));
  const chrome = spawn(
    CHROME,
    [
      '--headless=new',
      '--disable-gpu',
      '--no-first-run',
      `--remote-debugging-port=${cdpPort}`,
      '--remote-allow-origins=*',
      `--user-data-dir=${udd}`,
      `--window-size=${windowSize}`,
      url,
    ],
    { stdio: 'ignore', windowsHide: true }
  );
  chrome.on('error', (e) => check(false, `${label} spawn error：${e.message}`));
  // fix round 1／Codex F3：從 spawn 成功那一刻起就登記，不等 readiness 通過——readiness 失敗
  // 時 chrome handle 還沒回傳給呼叫端，呼叫端的 finally 摸不到它，若這裡不自己清掉就會一路
  // 留到 main() 最後的 finalSweep() 才補殺（見下方 cleanupOnFailure 與 finalSweep）。
  // fix round 3：登記的是 ChildProcess 物件本身（trackChild），不是 PID＋事後核對的身分
  // 資訊——所有權依據見 finalSweep() 上方大段引用來源的說明。
  trackChild(chrome, `${label}（chrome）`, cdpPort);

  // readiness 失敗時的收尾：按已知 PID 終止、確認消失、清暫存目錄，再把原因往外拋——不是留給
  // 呼叫端的 finally（那時候這個函式根本還沒回傳 handle）。
  async function cleanupOnFailure(reason) {
    log(`${label} 啟動失敗（${reason}），收尾中：終止 PID ${chrome.pid}、清暫存目錄`);
    killTree(chrome, `${label}（啟動失敗收尾）`);
    // untrack 條件統一在 settleTrackedChild()（fix round 3／4，見該函式註解）。
    const { exited, portListening } = await settleTrackedChild(chrome, cdpPort, label);
    check(exited, `${label} 啟動失敗收尾：應該觀察到子行程的 exit 事件（PID ${chrome.pid}）`);
    check(!portListening, `${label} 啟動失敗收尾後 CDP port ${cdpPort} 應該不再有 LISTENING 的行程`);
    try {
      fs.rmSync(udd, { recursive: true, force: true });
    } catch (e) {
      check(false, `${label} 啟動失敗收尾時清理暫存目錄失敗：${e.message}`);
    }
  }

  let page = null;
  try {
    for (let i = 0; i < 100 && !page; i++) {
      try {
        const r = await fetch(`http://127.0.0.1:${cdpPort}/json/list`);
        page = (await r.json()).find((t) => t.type === 'page' && t.url.startsWith(url));
      } catch {
        // CDP endpoint 還沒起來。
      }
      if (!page) await sleep(200);
    }
  } catch (e) {
    await cleanupOnFailure(`尋找 page target 時發生例外：${e.message}`);
    throw e;
  }
  if (!page) {
    await cleanupOnFailure('page target not found（輪詢逾時）');
    throw new Error(`${label}: page target not found`);
  }

  let ws;
  try {
    ws = new WebSocket(page.webSocketDebuggerUrl);
    await new Promise((res, rej) => {
      ws.onopen = res;
      ws.onerror = rej;
    });
  } catch (e) {
    await cleanupOnFailure(`WebSocket 連線失敗：${e && e.message ? e.message : e}`);
    throw e instanceof Error ? e : new Error(String(e));
  }
  return { chrome, udd, ws, cdp: new CDP(ws), cdpPort };
}

async function stopChrome(handle, label) {
  if (!handle) return;
  try {
    handle.ws.close();
  } catch {
    // 已斷線。
  }
  killTree(handle.chrome, label);
  const { exited, portListening } = await settleTrackedChild(handle.chrome, handle.cdpPort, label);
  check(exited, `${label} 應該觀察到子行程的 exit 事件（PID ${handle.chrome.pid}）`);
  check(!portListening, `${label} 的 CDP port ${handle.cdpPort} 應該不再有 LISTENING 的行程`);
  try {
    fs.rmSync(handle.udd, { recursive: true, force: true });
  } catch (e) {
    check(false, `清理暫存目錄失敗：${e.message}`);
  }
}

async function startPreview(envOverrides, label) {
  if (!fs.existsSync(UI_PREVIEW_EXE)) {
    throw new Error(`找不到 ${UI_PREVIEW_EXE}，請先跑 cargo build -p cockpit --example ui_preview`);
  }
  const port = pickPort(7830);
  const requests = [];
  const writeRequests = [];
  const fixturePaths = { reviewRepo: null };
  const server = spawn(UI_PREVIEW_EXE, [], {
    stdio: ['ignore', 'pipe', 'ignore'],
    windowsHide: true,
    env: {
      ...process.env,
      COCKPIT_PREVIEW_LISTEN: `127.0.0.1:${port}`,
      COCKPIT_PREVIEW_PUSH_MS: STABLE_PUSH_MS,
      ...envOverrides,
    },
  });
  server.on('error', (e) => check(false, `${label} spawn error：${e.message}`));
  // fix round 1／Codex F3：同 startChrome，readiness 失敗前就先登記，失敗時自己清乾淨再拋錯。
  // fix round 3：登記 ChildProcess 物件本身，理由同 startChrome（見 finalSweep() 上方的
  // 所有權保證鏈說明）。
  trackChild(server, `${label}（preview）`, port);
  let buffer = '';
  server.stdout.setEncoding('utf8');
  server.stdout.on('data', (chunk) => {
    buffer += chunk;
    let nl;
    while ((nl = buffer.indexOf('\n')) !== -1) {
      const line = buffer.slice(0, nl).replace(/\r$/, '');
      buffer = buffer.slice(nl + 1);
      const m = /^output-request (\S+) (\S+)$/.exec(line);
      if (m) requests.push({ runtime: m[1], pane: m[2], at: Date.now() });
      const w = /^write-request (\S+) (\S+) ?(.*)$/.exec(line);
      if (w) writeRequests.push({ method: w[1], path: w[2], body: w[3] });
      // file-review task 3.5：記下 ui_preview 暫存副本的路徑（FT1 要在副本裡建檔；stopPreview 用它清目錄）。
      const rr = /^review-repo: (.+)$/.exec(line);
      if (rr) fixturePaths.reviewRepo = rr[1].trim();
    }
  });

  let up = false;
  for (let i = 0; i < 50 && !up; i++) {
    try {
      up = (await fetch(`http://127.0.0.1:${port}/api/state`)).ok;
    } catch {
      // 還沒起來。
    }
    if (!up) await sleep(200);
  }
  check(up, `${label} 應該在 10 秒內開始回應（port ${port}）`);
  if (!up) {
    log(`${label} 啟動失敗，收尾中：終止 PID ${server.pid}`);
    killTree(server, `${label}（啟動失敗收尾）`);
    const { exited, portListening } = await settleTrackedChild(server, port, label);
    check(exited, `${label} 啟動失敗收尾：應該觀察到子行程的 exit 事件（PID ${server.pid}）`);
    check(!portListening, `${label} 啟動失敗收尾後 port ${port} 應該不再有 LISTENING 的行程`);
    throw new Error(`${label} 沒有起來`);
  }
  return { server, port, requests, writeRequests, fixturePaths };
}

async function stopPreview(preview, label) {
  if (!preview) return;
  killTree(preview.server, label);
  const { exited, portListening } = await settleTrackedChild(preview.server, preview.port, label);
  check(exited, `${label} 應該觀察到子行程的 exit 事件（PID ${preview.server.pid}）`);
  check(!portListening, `port ${preview.port}（${label}）應該不再有 LISTENING 的行程`);
  // file-review task 3.4 起 ui_preview 會把 review-repo fixture 複製到 %TEMP%\cockpit-ui-preview-*；
  // taskkill /F 結束時它沒有機會自己刪（task 3.4 report「暫存目錄生命週期」），這裡代刪（file-review
  // task 3.5）。只記 log、不加斷言，既有段落的斷言不變。
  const reviewRepo = preview.fixturePaths && preview.fixturePaths.reviewRepo;
  if (reviewRepo && path.basename(path.dirname(reviewRepo)).startsWith('cockpit-ui-preview-')) {
    const tempRoot = path.dirname(reviewRepo);
    for (let i = 0; i < 20 && fs.existsSync(tempRoot); i++) {
      try {
        fs.rmSync(tempRoot, { recursive: true, force: true });
      } catch {
        // Windows 偶爾 EBUSY／EPERM（檔案還被剛結束的行程鎖著），稍後重試。
      }
      if (fs.existsSync(tempRoot)) await sleep(250);
    }
    if (fs.existsSync(tempRoot)) log(`${label}：ui_preview 暫存目錄刪不掉（${tempRoot}），下次 ui_preview 啟動時會清`);
  }
}

// ---------------------------------------------------------------------------
// waitForFirstProjection：「第一份真投影已經畫出」的判準（Ruling R22，2.3 fix round 1，取代
// 本檔原本全檔通用的 `!!document.querySelector('.runtime-cards')`）。
//
// Codex C2（task 2.3 review）：`.runtime-cards` 在 index.html 的靜態占位內容裡從頁面一載入
// 就存在（`<div class="runtime-cards"></div>` 是空殼，見 index.html），renderState() 真的
// 用收到的投影跑過一次才會把 runtime 卡片塞進去——但「這個 class 存在」本身在真投影抵達前就
// 已經成立，拿它當判準在 WebSocket 較慢（例如系統忙碌、多個 headless Chrome 同時啟動）時會
// 提前通過：腳本讀到的其實是空殼占位，不是真投影，足以解釋 task 2.3 report 記錄的間歇性
// topbar 斷言 FAIL（R22：不採 spec-check「無已知因果」，因為這個機制本身具體可驗、也與
// 「15 次獨立重跑未重現」不衝突——沒重現只代表沒踩到那個時間窗，不代表這個機制不存在）。
//
// 新判準改成兩個條件同時成立：(a) 頂列出現至少一個 `[data-runtime]` 燈號節點（direction-01
// -visual task 2.3；`renderRuntimeLamp()` 只在 `renderState()` 真的跑過一次才會畫出，靜態
// 占位的 `.topbar-runtimes` 是空 div）；(b) `#version` 的文字精確等於當下 `/api/state` 回傳
// 的 version（加上 `v` 前綴）——靜態占位的 `#version` 是空字串，不可能等於任何合法 version。
// 兩者都只有真投影畫過才會成立，任一項單獨都不夠（例如 (a) 沒有 (b) 理論上可能是上一輪投影的
// 殘留 DOM，雖然目前實作不會發生，但雙重條件比單一條件更保守）。
//
// `/api/state` 在每次輪詢當下才重新 fetch（不是進入這個函式時 fetch 一次存起來）：本檔部分
// 段落用短推送間隔（例如 S1 的 COCKPIT_PREVIEW_PUSH_MS=200、P1 的 100），version 在等待期間
// 可能一直往前跑，若只在函式開頭 fetch 一次會有跟頁面實際版本對不上的競態視窗；改成每次輪詢
// 都用「幾乎同一時刻」fetch 到的 /api/state 版本去跟頁面比對，把這個競態視窗縮到最小。
async function waitForFirstProjection(cdp, previewPort, label) {
  const finalLabel =
    label || '第一份真投影已畫出（topbar 出現 [data-runtime] 節點且 #version 等於 /api/state 的 version，不是靜態占位）';
  const timeoutMs = 5000;
  const start = Date.now();
  while (Date.now() - start < timeoutMs) {
    const pageState = await cdp.eval(`(() => {
      var lamp = document.querySelector('[data-region="topbar"] [data-runtime]');
      var v = document.getElementById('version');
      return { hasLamp: !!lamp, versionText: v ? v.textContent.trim() : null };
    })()`);
    if (pageState.hasLamp && pageState.versionText) {
      let expected = null;
      try {
        expected = await fetch(`http://127.0.0.1:${previewPort}/api/state`).then((r) => r.json());
      } catch {
        // preview 暫時沒回應（極少見），這一輪當作沒對上，下一輪重試。
      }
      if (expected && pageState.versionText === 'v' + expected.version) {
        check(true, finalLabel);
        return true;
      }
    }
    await sleep(50);
  }
  check(false, `逾時（${timeoutMs} ms）：${finalLabel}`);
  return false;
}

// ---------------------------------------------------------------------------
// design D9 狀態注入工具：保存原 window.onState、換成空函數擋推送、以原函數畫特製投影。
// ---------------------------------------------------------------------------

// fix round 1／Codex F2：預設的擋推送函數是完全靜默的空函數，證明不了「真的攔到了幾次」；
// countBlocked=true 時改成會計數的空函數（`window.__cockpitBlockedPushCount` 遞增，仍然不
// 呼叫原本的 onState，一樣達到「擋推送」的效果），S1 用它輪詢「真的攔到至少兩次」而不是固定
// 睡多久。其餘呼叫點（V1/G1/P1 的邊界資料注入）不需要這個計數，維持預設 false 不受影響。
async function injectState(cdp, state, options) {
  const countBlocked = !!(options && options.countBlocked);
  const json = JSON.stringify(state);
  const blockerExpr = countBlocked
    ? 'function () { window.__cockpitBlockedPushCount = (window.__cockpitBlockedPushCount || 0) + 1; }'
    : 'function () {}';
  return cdp.eval(`(function () {
    if (!window.__cockpitOrigOnState) {
      window.__cockpitOrigOnState = window.onState;
    }
    window.onState = ${blockerExpr}; // 擋掉之後所有真的 /ws 推送（countBlocked 時順便計數）。
    if (typeof window.__cockpitOrigOnState !== 'function') {
      return { ok: false, reason: 'window.onState 在保存當下不是函數' };
    }
    window.__cockpitOrigOnState(${json});
    return { ok: true };
  })()`);
}

// fix round 1／Codex F2：用「真的觀察到 #version 變化」取代固定 sleep 來證明「這段時間內發生
// 了幾次重畫」，並在每次觀察到變化時立刻呼叫 perRepaintCheck()（內部自己 check()）——不是只在
// 等待結束後看最後一眼。P1 的「選取跨重畫保留」「鍵盤切換與焦點保留」共用這個實作。
async function waitForRepaintsAssertingEachTime(cdp, minRepaints, timeoutMs, perRepaintCheck, summaryLabel) {
  const readVersion = () =>
    cdp.eval("(() => { var v = document.getElementById('version'); return v ? v.textContent : null; })()");
  let lastVersion = await readVersion();
  let repaints = 0;
  const start = Date.now();
  while (Date.now() - start < timeoutMs && repaints < minRepaints) {
    await sleep(50);
    const v = await readVersion();
    if (v !== null && v !== lastVersion) {
      repaints += 1;
      lastVersion = v;
      await perRepaintCheck(repaints, v);
    }
  }
  check(
    repaints >= minRepaints,
    `${summaryLabel}：應該在 ${timeoutMs} ms 內觀察到至少 ${minRepaints} 次重畫（#version 變化）（實際觀察到 ${repaints} 次）`
  );
  return repaints;
}

// ---------------------------------------------------------------------------
// 對比計算工具＋命中測試工具（安裝到頁面成 window.__cockpitVisualTools；S2／S3 自我測試、CT1、
// LO1 的「面板打開時仍可操作頁面下方的內容」共用同一份實作，不同腳本重複貼一份容易兩邊漂移）。
// ---------------------------------------------------------------------------

const TOOLS_JS = `
(function () {
  function parseColor(str) {
    if (!str || str === 'transparent') return { r: 0, g: 0, b: 0, a: 0 };
    var m = str.match(/rgba?\\(([^)]+)\\)/);
    if (!m) return { r: 0, g: 0, b: 0, a: 0 };
    var parts = m[1].split(',').map(function (s) { return parseFloat(s.trim()); });
    return { r: parts[0] || 0, g: parts[1] || 0, b: parts[2] || 0, a: parts.length > 3 ? parts[3] : 1 };
  }
  function channel(c) {
    c = c / 255;
    return c <= 0.03928 ? c / 12.92 : Math.pow((c + 0.055) / 1.055, 2.4);
  }
  function relLuminance(c) {
    return 0.2126 * channel(c.r) + 0.7152 * channel(c.g) + 0.0722 * channel(c.b);
  }
  function contrastRatio(c1, c2) {
    var l1 = relLuminance(c1) + 0.05;
    var l2 = relLuminance(c2) + 0.05;
    return l1 > l2 ? l1 / l2 : l2 / l1;
  }
  function compositeOver(top, base) {
    // base 視為不透明（a === 1）；標準 alpha-over 公式（brief：「最近一層不透明背景」，中間的
    // 半透明層要疊回去，不能只看最近一層本身的顏色）。
    return {
      r: top.r * top.a + base.r * (1 - top.a),
      g: top.g * top.a + base.g * (1 - top.a),
      b: top.b * top.a + base.b * (1 - top.a),
      a: 1,
    };
  }
  // 從 startEl 本身開始往上找最近一層不透明背景，把中間所有半透明背景層依繪製順序（外層先畫）
  // 疊回去。
  function effectiveBackground(startEl) {
    var layers = [];
    var node = startEl;
    while (node) {
      var cs = getComputedStyle(node);
      var bg = parseColor(cs.backgroundColor);
      if (bg.a > 0) {
        layers.push(bg);
        if (bg.a >= 0.999) break;
      }
      node = node.parentElement;
    }
    if (layers.length === 0) {
      // 理論上不會發生（html/body 一定有背景色）；退回白色讓比對明顯失敗，不拋例外。
      return { r: 255, g: 255, b: 255, a: 1 };
    }
    layers.reverse(); // 由最外層（最先畫）到最靠近 startEl。
    var result = layers[0].a >= 0.999 ? layers[0] : compositeOver(layers[0], { r: 255, g: 255, b: 255, a: 1 });
    for (var i = 1; i < layers.length; i += 1) {
      result = compositeOver(layers[i], result);
    }
    return result;
  }
  // fix round 1／Codex F1：CSS opacity（元素自己或任一祖先）會讓「這個元素連同它的子孫」整組
  // 一起以該透明度跟它外層的背景疊色——不是只影響 background-color 本身。現行
  // ".output-panel.is-stale .output-text { opacity: 0.55 }" 正是這種用法：.output-text
  // 自己有不透明的 background-color（--bg-inset），若只看 backgroundColor／color 字面值算
  // 對比，會完全略過這層 0.55，把過期文字誤判成跟正常文字一樣的對比。做法：從 el 往上走訪整條
  // 祖先鏈，任何節點的 computed opacity < 1 時，把「目前疊好的顏色」以該 opacity 當作 alpha
  // 跟「那個節點外層（父節點）的有效背景」做一次 alpha-over，逐層往外套用（元素自己與每個祖先
  // 各自的 opacity 都要套，不能只套一次）。
  function applyOpacityChain(el, startColor) {
    var current = startColor;
    var node = el;
    while (node) {
      var op = parseFloat(getComputedStyle(node).opacity);
      if (!Number.isNaN(op) && op < 1) {
        var parent = node.parentElement;
        var backdrop = parent ? effectiveBackground(parent) : { r: 255, g: 255, b: 255, a: 1 };
        current = compositeOver({ r: current.r, g: current.g, b: current.b, a: op }, backdrop);
      }
      node = node.parentElement;
    }
    return current;
  }
  function textContrast(el) {
    var cs = getComputedStyle(el);
    var rawFg = parseColor(cs.color);
    var localBg = effectiveBackground(el); // el 自己這層的不透明背景，先不管 opacity 鏈。
    // 前景本身的 alpha（fix round 1／Codex F1：color: rgba(...) 的 alpha 之前完全被忽略，
    // 直接拿 r/g/b 去跟背景比較）：alpha < 1 時先跟 el 自己的本地背景疊一次，得到「不看 opacity
    // 鏈時，這段文字實際看起來的顏色」。
    var fgBeforeOpacity = rawFg.a < 0.999 ? compositeOver(rawFg, localBg) : rawFg;
    var fg = applyOpacityChain(el, fgBeforeOpacity);
    var bg = applyOpacityChain(el, localBg);
    return { ratio: contrastRatio(fg, bg), fg: fg, bg: bg };
  }
  function hasOwnText(el) {
    for (var i = 0; i < el.childNodes.length; i += 1) {
      var n = el.childNodes[i];
      if (n.nodeType === 3 && n.textContent.trim().length > 0) return true;
    }
    return false;
  }
  function isVisible(el) {
    var cs = getComputedStyle(el);
    if (cs.display === 'none' || cs.visibility === 'hidden') return false;
    if (el.offsetWidth === 0 && el.offsetHeight === 0) return false;
    return true;
  }
  // 走訪 root 底下每個「自己直接含有非空白文字」的元素，算出對比；跳過不可見的元素。
  function walkTextContrast(rootSelector) {
    var root = rootSelector ? document.querySelector(rootSelector) : document.body;
    if (!root) return [];
    var out = [];
    var all = root.querySelectorAll('*');
    for (var i = 0; i < all.length; i += 1) {
      var el = all[i];
      if (!hasOwnText(el) || !isVisible(el)) continue;
      var r = textContrast(el);
      out.push({
        tag: el.tagName,
        cls: el.className ? String(el.className) : '',
        text: el.textContent.trim().slice(0, 40),
        ratio: r.ratio,
        fg: r.fg,
        bg: r.bg,
      });
    }
    return out;
  }
  // 命中測試（spec live-output「面板打開時仍可操作頁面下方的內容」）：取可見範圍中心點，
  // document.elementFromPoint 命中的是不是自己或子孫。
  //
  // fix round 1／規格對照 S3（Minor-3）：「可見範圍」原本只跟 viewport 取交集，沒有排除
  // 「被祖先自己的 overflow: auto/hidden 裁掉」的座標——design D3「各區內部捲動」（task 2.1）
  // 讓 Factory Floor／runtime 清單／最近事件各自有自己的內層捲動容器（design 審核 I2），內容
  // 比容器高時，後段元素的幾何座標仍落在「視窗」範圍內，卻已經被自己的捲動容器裁掉、實際不
  // 可見；沒有排除這種情況會讓 LO1 的按鈕命中測試把「純粹被自己的捲動容器捲出去」誤判成
  // 「被 Live Output 面板蓋住」。改成從元素本身的 rect 開始，往上走訪每一層祖先，只要祖先的
  // overflow-x／overflow-y 不是 visible 就用它的 rect 跟目前的交集框再取一次交集，最後才跟
  // 視窗（最外層的裁切框）取交集——跟 live-output-check.js E 段 fix round 1（Codex C2）用的是
  // 同一套邏輯，兩邊分別實作是因為兩個腳本不共用模組，不是刻意寫兩份不同的規則。
  function visibleBox(n) {
    var r = n.getBoundingClientRect();
    var left = r.left;
    var top = r.top;
    var right = r.right;
    var bottom = r.bottom;
    var node = n.parentElement;
    while (node) {
      var cs = getComputedStyle(node);
      if (cs.overflowX !== 'visible' || cs.overflowY !== 'visible') {
        var cr = node.getBoundingClientRect();
        left = Math.max(left, cr.left);
        top = Math.max(top, cr.top);
        right = Math.min(right, cr.right);
        bottom = Math.min(bottom, cr.bottom);
      }
      node = node.parentElement;
    }
    left = Math.max(left, 0);
    top = Math.max(top, 0);
    right = Math.min(right, window.innerWidth);
    bottom = Math.min(bottom, window.innerHeight);
    return { left: left, top: top, right: right, bottom: bottom };
  }

  function hitCenter(selector) {
    var n = typeof selector === 'string' ? document.querySelector(selector) : selector;
    if (!n) return null;
    var box = visibleBox(n);
    if (box.right <= box.left || box.bottom <= box.top) {
      return { visible: false };
    }
    var cx = (box.left + box.right) / 2;
    var cy = (box.top + box.bottom) / 2;
    var hit = document.elementFromPoint(cx, cy);
    return {
      visible: true,
      cx: cx,
      cy: cy,
      selfHit: !!hit && (hit === n || n.contains(hit)),
      hitDesc: hit ? hit.tagName + (hit.id ? '#' + hit.id : '') + (hit.className ? '.' + String(hit.className).split(' ').join('.') : '') : null,
    };
  }
  window.__cockpitVisualTools = {
    parseColor: parseColor,
    relLuminance: relLuminance,
    contrastRatio: contrastRatio,
    compositeOver: compositeOver,
    effectiveBackground: effectiveBackground,
    applyOpacityChain: applyOpacityChain,
    textContrast: textContrast,
    walkTextContrast: walkTextContrast,
    hitCenter: hitCenter,
  };
  return true;
})();
`;

async function installTools(cdp) {
  const ok = await cdp.eval(TOOLS_JS);
  check(ok === true, '安裝 window.__cockpitVisualTools（對比／命中測試工具）成功');
}

// ---------------------------------------------------------------------------
// fix round 1／Codex F6：V2／V3 共用的版面幾何工具——一次取多個區塊的 getBoundingClientRect，
// Node 端（不是頁面內）用純粹的矩形交集判斷「兩兩不重疊」，比只比較 top 嚴謹：允許左右並排
// （X 不重疊即可），但不允許任何實際面積重疊，也不允許「上面那塊還沒真的結束，下面那塊就開始」
// 這種只看 top 通不過、只看某個角也算過的邊界情況。data-region 現在還不存在，先用既有 class
// 當代理選擇器；task 2.1 加上 data-region 後這裡的 selectorMap 要跟著換。
//
// task 2.1 實作時的裁決：V2 的 selectorMap 維持不動，不換成 data-region。Factory Floor 外層
// 沿用舊 class="projects"（factory-floor-check.js 逐字比對 `class="projects"` 在
// `class="runtime-cards"` 之前，D1「保留 DOM 身分」），render.js 在同一個節點上*另外*加
// `data-region="floor"`——`.projects`（class）跟 `[data-region="floor"]`（屬性）兩個選擇器都
// 指向這個節點，V2 的 `.projects` 繼續有效，不必改。新加的左欄（design D2「projects」＝左欄，
// 跟這個舊 class 撞名但是不同節點）改用 `[data-region="projects"]` 定位，V3／V4 用的正是這個
// 屬性選擇器，不會跟 Factory Floor 混淆。
// ---------------------------------------------------------------------------

async function getRects(cdp, selectorMap) {
  const entries = Object.entries(selectorMap);
  const parts = entries.map(
    ([label, sel]) =>
      `${JSON.stringify(label)}: (() => {
        var n = document.querySelector(${JSON.stringify(sel)});
        if (!n) return null;
        var cs = getComputedStyle(n);
        if (cs.display === 'none' || cs.visibility === 'hidden') return null;
        var r = n.getBoundingClientRect();
        if (r.width === 0 && r.height === 0) return null;
        return { left: r.left, right: r.right, top: r.top, bottom: r.bottom, width: r.width, height: r.height };
      })()`
  );
  return cdp.eval(`({ ${parts.join(', ')} })`);
}

// ---------------------------------------------------------------------------
// fix round 4／控制端 Ruling R18：長名稱包含檢查的頁面內工具（字串，嵌進 cdp.eval 的函式本體；
// 不得含反引號）。判準只有一條：捲動歸零後，目標元素的水平範圍 [left,right] 落在所屬
// data-region 未裁切 rect 的 [left,right] 內，沒有任何捲動例外。
//   - r18ResetScroll()：所有元素（含整頁 scrollingElement）的 scrollLeft／scrollTop 歸零。
//   - extentOf(el)：元素框；元素自己 overflow-x 為 visible 時再與其內容的 Range 框取聯集——
//     不換行、不省略的長字串，元素框可能仍在區域內、文字卻畫到框外，只量元素框會漏掉；元素
//     自己裁切（overflow hidden／clip，例如單行省略）時，畫得出來的範圍就是元素框。
//   - r18Measure(el, regionSelector)：缺元素、缺 region、元素不在該 region 裡、display:none／
//     visibility:hidden、零尺寸都回報為各自的旗標且 contained=false。paneRowRight：元素若在
//     .pane-row 裡，回報該列 grid 欄位排版的右緣（列框左緣＋border＋padding＋解析後的欄寬總和與
//     gap；.pane-row 的欄位總寬大於列框時，欄位會畫到列框外，列框本身不會變寬。刻意不用
//     scrollWidth——被 relative 位移的子元素會撐大 scrollWidth、替自己作證，這正是 R18 要排除的
//     情況；欄寬是排版結果，不受子元素位移影響。供 Node 端判斷 3.3 已知特徵）。
// ---------------------------------------------------------------------------
const R18_MEASURE_JS = `
  function r18ResetScroll() {
    var all = document.querySelectorAll('*');
    for (var i = 0; i < all.length; i += 1) {
      if (all[i].scrollLeft !== 0) all[i].scrollLeft = 0;
      if (all[i].scrollTop !== 0) all[i].scrollTop = 0;
    }
    if (document.scrollingElement) {
      document.scrollingElement.scrollLeft = 0;
      document.scrollingElement.scrollTop = 0;
    }
  }
  function r18Round(v) { return Math.round(v * 100) / 100; }
  function r18GridContentRight(row) {
    var rr = row.getBoundingClientRect();
    var rcs = getComputedStyle(row);
    var tracks = rcs.gridTemplateColumns.split(' ').map(parseFloat).filter(function (v) { return !isNaN(v); });
    var gap = parseFloat(rcs.columnGap) || 0;
    var sum = 0;
    for (var k = 0; k < tracks.length; k += 1) sum += tracks[k];
    if (tracks.length > 1) sum += gap * (tracks.length - 1);
    var start = rr.left + (parseFloat(rcs.borderLeftWidth) || 0) + (parseFloat(rcs.paddingLeft) || 0);
    return Math.max(rr.right, start + sum);
  }
  function r18Measure(el, regionSelector) {
    if (!el) return { exists: false, contained: false };
    var region = document.querySelector(regionSelector);
    if (!region) return { exists: true, regionMissing: true, contained: false };
    if (!region.contains(el)) return { exists: true, notInRegion: true, contained: false };
    var cs = getComputedStyle(el);
    if (cs.display === 'none' || cs.visibility === 'hidden') return { exists: true, hidden: true, contained: false };
    var b = el.getBoundingClientRect();
    var r = region.getBoundingClientRect();
    if (b.width <= 0.5 || b.height <= 0.5) return { exists: true, zeroSize: true, contained: false };
    var left = b.left;
    var right = b.right;
    if (cs.overflowX === 'visible') {
      var range = document.createRange();
      range.selectNodeContents(el);
      var t = range.getBoundingClientRect();
      if (t.width > 0 || t.height > 0) {
        left = Math.min(left, t.left);
        right = Math.max(right, t.right);
      }
    }
    var row = el.closest('.pane-row');
    return {
      exists: true,
      contained: left >= r.left - 0.5 && right <= r.right + 0.5,
      text: el.textContent.slice(0, 24),
      ext: { left: r18Round(left), right: r18Round(right) },
      box: { left: r18Round(b.left), right: r18Round(b.right) },
      region: { left: r18Round(r.left), right: r18Round(r.right) },
      paneRowRight: row ? r18Round(r18GridContentRight(row)) : null,
    };
  }
  function r18MeasureAll(targetSelector, regionSelector) {
    var region = document.querySelector(regionSelector);
    if (!region) return [{ exists: true, regionMissing: true, contained: false }];
    var list = document.querySelectorAll(targetSelector);
    var out = [];
    for (var i = 0; i < list.length; i += 1) out.push(r18Measure(list[i], regionSelector));
    return out;
  }
`;

// 兩個矩形是否有實際面積重疊（容忍 0.5px 的次像素誤差，避免相鄰邊界因為捨入誤差被誤判成重疊）。
function rectsOverlap(a, b) {
  return a.left < b.right - 0.5 && b.left < a.right - 0.5 && a.top < b.bottom - 0.5 && b.top < a.bottom - 0.5;
}

// fix round 2：由一組 label 產生所有兩兩不重複配對，取代手 key 清單——V2 的 fix round 1 就
// 漏過 ['floor','output'] 這一組（手動列表容易漏），用程式產生從結構上排除這種遺漏。
function allPairs(labels) {
  const out = [];
  for (let i = 0; i < labels.length; i += 1) {
    for (let j = i + 1; j < labels.length; j += 1) {
      out.push([labels[i], labels[j]]);
    }
  }
  return out;
}

function checkNoOverlap(rects, pairs, prefix) {
  for (const [labelA, labelB] of pairs) {
    const a = rects[labelA];
    const b = rects[labelB];
    if (!a || !b) {
      check(false, `${prefix}: ${labelA} 與 ${labelB} 至少有一個不存在／不可見，無法驗證不重疊`);
      continue;
    }
    check(
      !rectsOverlap(a, b),
      `${prefix}: ${labelA}（${JSON.stringify(a)}）與 ${labelB}（${JSON.stringify(b)}）不應該視覺重疊`
    );
  }
}

// I1／Codex medium #2（task 2.3 fix round 2）：V3／V4 上一輪的「sticky 替代斷言」只確認
// statusbar 的 rect 落在頁首視窗內——position: fixed、absolute，甚至捲動後就消失的錯誤實作
// 都能通過（Codex 原文）。改成完整驗證：
//   (1) 計算樣式 position 真的是 sticky（不是只看座標）；
//   (2) 頁首（scrollY=0）時 bottom 貼齊視窗底；
//   (3) 實際往下捲動一段距離（刻意不捲到文件最底——捲到底的話任何定位方式的最後一個元素都會
//       自然出現在視窗底部附近，測不出差異）之後，bottom 仍然貼齊視窗底；
//   (4) DOM 順序：statusbar 仍是 #app 底下最後一個 [data-region]（sticky 不改變文件結構）。
// 最後兩個否定對照，直接暫時覆寫真正的 statusbar 元素（inline style 特異度比樣式表規則高，
// 可以蓋掉 CSS 的 position: sticky，覆寫完立刻還原，不影響後續斷言）：
//   (a) 改成 position: fixed，(1) 的計算樣式檢查要抓到「不是 sticky」；
//   (b) 改成 position: static，捲到跟 (3) 同一個（不到文件底的）位置，bottom 不應該貼底，
//       (3) 的邏輯要抓到「捲動後沒有真的貼底」。
async function checkStickyStatusbar(cdp, label) {
  const initial = await cdp.eval(`(() => {
    var el = document.querySelector('[data-region="statusbar"]');
    var cs = getComputedStyle(el);
    var rect = el.getBoundingClientRect();
    var regions = Array.prototype.slice.call(document.querySelectorAll('#app [data-region]'));
    return {
      position: cs.position,
      bottom: rect.bottom,
      innerHeight: window.innerHeight,
      isLastRegion: regions.length > 0 && regions[regions.length - 1] === el,
      regionOrder: regions.map(function (r) { return r.getAttribute('data-region'); }),
      scrollHeight: document.documentElement.scrollHeight,
    };
  })()`);
  check(initial.position === 'sticky', `${label}：statusbar 的計算樣式 position 應該是 sticky（實際 ${initial.position}）`);
  check(
    Math.abs(initial.bottom - initial.innerHeight) < 1,
    `${label}：頁首（scrollY=0）時 statusbar 的 bottom 應該貼齊視窗底（實際 bottom=${initial.bottom}, innerHeight=${initial.innerHeight}）`
  );
  check(
    initial.isLastRegion,
    `${label}：statusbar 應該仍是 #app 底下最後一個 [data-region]（DOM 順序；實際 ${JSON.stringify(initial.regionOrder)}）`
  );

  const scrollable = initial.scrollHeight - initial.innerHeight;
  check(
    scrollable > 60,
    `${label}：頁面應該有足夠的可捲動高度才能有意義地驗證「捲動後仍貼底」（不能只捲到文件最底，那樣任何定位方式都會自然貼底；實際可捲動 ${scrollable}px）`
  );
  // 門檻取一個「離頁首夠遠、離文件最底也夠遠」的中間值：V3（700×900）與 V4（1280×650）實測
  // 可捲動高度分別約 900+px 與 212px，用固定 300px／100px 這種寫死的數字任一邊都會踩雷
  // （太大在矮頁面會逼近甚至超過可捲動範圍，太小在高頁面測不出明顯差異）；改成跟可捲動總量
  // 成比例並夾在 [20, 150] 之間。
  const scrollTarget = Math.min(150, Math.max(20, scrollable - 40));

  await cdp.eval(`window.scrollTo(0, ${scrollTarget}); true`);
  await sleep(150);
  const afterScroll = await cdp.eval(`(() => {
    var el = document.querySelector('[data-region="statusbar"]');
    var rect = el.getBoundingClientRect();
    return { bottom: rect.bottom, innerHeight: window.innerHeight, scrollY: window.scrollY };
  })()`);
  check(
    afterScroll.scrollY > 0,
    `${label}：捲動後 window.scrollY 應該大於 0（實際 ${afterScroll.scrollY}，確認真的捲動過，不是恆真）`
  );
  check(
    Math.abs(afterScroll.bottom - afterScroll.innerHeight) < 1,
    `${label}：捲動到 ${scrollTarget}px（刻意不到文件最底）之後，statusbar 的 bottom 仍應該貼齊視窗底（實際 bottom=${afterScroll.bottom}, innerHeight=${afterScroll.innerHeight}）`
  );

  const fixedPosition = await cdp.eval(`(() => {
    var el = document.querySelector('[data-region="statusbar"]');
    var original = el.style.position;
    el.style.position = 'fixed';
    var positionNow = getComputedStyle(el).position;
    el.style.position = original;
    return positionNow;
  })()`);
  check(
    fixedPosition !== 'sticky',
    `否定對照：${label} 把 statusbar 暫時改成 position: fixed 後，計算樣式不應該是 sticky（實際 ${fixedPosition}）——證明「position === sticky」這條斷言不是恆真`
  );

  const staticAfterScroll = await cdp.eval(`(() => {
    var el = document.querySelector('[data-region="statusbar"]');
    var original = el.style.position;
    el.style.position = 'static';
    window.scrollTo(0, ${scrollTarget});
    var rect = el.getBoundingClientRect();
    var result = { bottom: rect.bottom, innerHeight: window.innerHeight };
    el.style.position = original;
    window.scrollTo(0, ${scrollTarget});
    return result;
  })()`);
  check(
    Math.abs(staticAfterScroll.bottom - staticAfterScroll.innerHeight) > 5,
    `否定對照：${label} 把 statusbar 暫時改成 position: static 後捲動到同一個位置，bottom 不應該貼齊視窗底（實際 bottom=${staticAfterScroll.bottom}, innerHeight=${staticAfterScroll.innerHeight}）——證明「捲動後仍貼底」這條斷言不是恆真`
  );

  await cdp.eval('window.scrollTo(0, 0); true');
  await sleep(50);
}

// ---------------------------------------------------------------------------
// crafted state（design D9：邊界資料用注入，不改 ui_preview 共用的 fixture）
// ---------------------------------------------------------------------------

function loadFixture() {
  return JSON.parse(fs.readFileSync(FIXTURE_PATH, 'utf8'));
}

// V1：兩個 runtime 各 5 個 pane、30 筆最近事件、一個有 10 個 stages 的 Project、一個 200 字元的
// workstream 名稱與一個 200 字元的 pane cwd，一次滿足 V1 段的三個 GIVEN。
function craftWideState() {
  const s = loadFixture();
  function makePane(id, i) {
    return {
      id,
      agent: i % 2 === 0 ? 'claude' : null,
      agent_status: ['working', 'idle', 'blocked', 'done', 'unknown'][i % 5],
      title: 'pane ' + id,
      cwd: 'C:\\\\x\\\\' + id,
      label: null,
      focused: false,
      exited: false,
      updated_at: '2026-09-23T00:00:00Z',
    };
  }
  function makeRuntime(id) {
    const panes = [];
    for (let i = 0; i < 5; i += 1) panes.push(makePane(id + ':p' + i, i));
    return {
      id,
      kind: 'herdr',
      endpoint: 'tcp://127.0.0.1:0',
      connection: {
        state: 'connected',
        since: '2026-09-23T00:00:00Z',
        server_version: '0.9.0-preview.3',
        protocol: 22,
        last_snapshot_at: '2026-09-23T00:00:00Z',
        protocol_warning: null,
      },
      focused: { workspace_id: null, tab_id: null, pane_id: null },
      workspaces: [
        {
          id: id + ':w',
          label: null,
          number: 1,
          agent_status: 'working',
          focused: false,
          tabs: [{ id: id + ':t', number: 1, agent_status: 'working', focused: false, panes }],
        },
      ],
    };
  }
  s.version = 1;
  s.runtimes = [makeRuntime('rA'), makeRuntime('rB')];
  const longName = 'W'.repeat(200);
  const longCwd = 'C:\\\\' + 'x'.repeat(196);
  const stages = [];
  for (let i = 1; i <= 10; i += 1) stages.push('Stage' + i);
  s.projects = [
    {
      id: 'wide',
      name: 'Wide Project',
      stages,
      warnings: [],
      workstreams: [
        {
          id: 'ws-long',
          name: longName,
          binding: {
            state: 'bound',
            runtime: 'rA',
            pane_id: 'rA:p0',
            source: 'auto',
            agent: 'claude',
            agent_status: 'working',
          },
        },
      ],
      tasks: [
        {
          id: 't1',
          title: 'Task',
          workstream: 'ws-long',
          stage: 'Stage10',
          mark: 'none',
          status: 'running',
          depends_on: [],
        },
      ],
    },
  ];
  s.runtimes[0].workspaces[0].tabs[0].panes[0].cwd = longCwd;
  s.recent_events = [];
  for (let i = 0; i < 30; i += 1) {
    s.recent_events.push({
      at: '2026-09-23T00:00:0' + (i % 10) + 'Z',
      runtime: 'rA',
      kind: 'pane.agent_status_changed',
      pane_id: 'rA:p0',
      detail: 'e' + i,
    });
  }
  return s;
}

function craftNoProjectsState() {
  const s = loadFixture();
  s.version = 2;
  s.projects = [];
  return s;
}

// fix round 1／Codex finding（1）：模擬「已選定的 Project（p）暫時從投影中消失」——只留
// cockpit，供「消失又出現時選取不會自動跳回去」的回歸測試使用（design D6：「選定的 Project
// 已不在最新投影中時改為選定第一個」是正式的狀態改變，不是暫時的顯示 fallback）。
function craftOnlyCockpitState() {
  const s = loadFixture();
  s.version = 90;
  s.projects = s.projects.filter((p) => p.id === 'cockpit');
  return s;
}

// design 審核 M2：200 字中文 Project 名稱（沒有空白可斷行，逼出 overflow-wrap:
// anywhere＋line-clamp 都要生效），驗左欄項目名稱最多顯示 2 行、不撐破欄寬、完整名稱放
// title。
function craftLongProjectNameState() {
  const s = loadFixture();
  s.version = 91;
  const project = s.projects.find((p) => p.id === 'cockpit');
  project.name = '測'.repeat(200);
  return s;
}

// 設計審核 N1（task 3.1 fix round 2）：12 個 Project（含三種 200 字名稱，同審核自己用的量測
// 資料）——左欄清單高度超過內容能撐開的視窗高度，才會在頁面頂端觸發「左欄蓋住底列通道狀態」
// 這個退步（1100×900 下約 9 個以上 Project 才會觸發，2 個 Project 的 fixture 量不到）。
function craftManyProjectsState() {
  const s = loadFixture();
  s.version = 92;
  const template = s.projects[0];
  const projects = [];
  for (let i = 0; i < 12; i += 1) {
    const clone = JSON.parse(JSON.stringify(template));
    clone.id = 'proj-' + i;
    clone.name = i < 3 ? '測'.repeat(200) : 'Project ' + i;
    projects.push(clone);
  }
  s.projects = projects;
  return s;
}

function craftUnknownTaskStatusState() {
  const s = loadFixture();
  s.version = 3;
  // cockpit 專案的 be-1（原本 running）改成未知字串，其餘節點（ops-1 completed、qa-1
  // blocked……）保持原樣，供「其他節點正常」比對。
  const project = s.projects.find((p) => p.id === 'cockpit');
  const task = project.tasks.find((t) => t.id === 'be-1');
  task.status = 'whatever';
  return s;
}

// G1／[G1/sticky]（direction-01-visual task 3.2）：cockpit 專案改成 10 個 stage、再加兩條
// workstream（共 7 條），讓 Factory Floor 的內層捲動容器在 1536×1024 下兩個方向都溢出；其餘
// task 沿用 fixture（stage 名稱 Spec／Implement／Review 保留在前三欄）。
function craftStickyFloorState() {
  const s = loadFixture();
  s.version = 73;
  const project = s.projects.find((p) => p.id === 'cockpit');
  project.stages = ['Spec', 'Implement', 'Review', 'S4', 'S5', 'S6', 'S7', 'S8', 'S9', 'S10'];
  project.workstreams.push(
    { id: 'x1', name: 'Extra 1', binding: { state: 'none' } },
    { id: 'x2', name: 'Extra 2', binding: { state: 'none' } }
  );
  project.tasks.push(
    { id: 'x1-1', title: '橫向捲動用', workstream: 'x1', stage: 'S6', mark: 'none', status: 'running', depends_on: [] },
    { id: 'x2-1', title: '最後一欄', workstream: 'x2', stage: 'S10', mark: 'none', status: 'ready', depends_on: [] }
  );
  return s;
}

function craftStatusCountState() {
  const s = loadFixture();
  s.version = 4;
  // spec 例子：p1 有 2 個 running、1 個 completed、1 個 failed、1 則 warning（fixture 的
  // cockpit 專案已經有 1 則 warning；把 qa-1 從 blocked 改成 running，湊成 2 個 running，
  // ops-1／ops-2 原本就是 1 個 completed／1 個 failed，不用動）。
  const project = s.projects.find((p) => p.id === 'cockpit');
  const qa1 = project.tasks.find((t) => t.id === 'qa-1');
  qa1.status = 'running';
  return s;
}

// regression (d)（Codex C3，task 2.3 fix round 1；fix round 3／設計複審 R2-1 追加：其中一個
// runtime 是 disconnected）：5 個 runtime、每個 id 長 60 字元——設定檔沒有限制 runtime 數量或
// id 長度上限，這是「合法輸入」不是刻意構造的極端值。沿用第一個 runtime 的
// connection／workspaces 當範本（workspaces 清空，這個回歸只關心頂列高度，不需要 pane
// 資料），只換 id；第一個 runtime（最容易被排擠到捲動範圍外的那個）額外改成 disconnected，
// 對照設計複審 R2-1「斷線的 runtime 被排擠出去、頂列只剩綠燈」的原始情境——R26 換成純 CSS
// 橫向捲動之後，這個 runtime 的燈號應該還在 DOM 裡（只是可能需要捲動才看得到），不會被
// 藏起來。
function craftManyLongRuntimesState() {
  const s = loadFixture();
  s.version = 12345;
  const template = s.runtimes[0];
  s.runtimes = [];
  for (let i = 0; i < 5; i += 1) {
    const clone = JSON.parse(JSON.stringify(template));
    clone.id = ('runtime-' + i + '-').padEnd(60, 'x').slice(0, 60);
    clone.workspaces = [];
    if (i === 0) {
      clone.connection = { state: 'disconnected', reason: 'R2-1 regression', retry_in_secs: 5 };
    }
    s.runtimes.push(clone);
  }
  return s;
}

// R2-2（設計審核，task 2.3 fix round 2；fix round 3 補段）：390 寬、一個長一點的 id（沿用
// 設計複審原始重現案例 "wsl-ubuntu"）、通道斷線——驗證產品名與燈號不再互相重疊。
function craftLongIdNarrowState() {
  const s = loadFixture();
  s.version = 12347;
  const wsl = s.runtimes.find((r) => r.id === 'wsl');
  wsl.id = 'wsl-ubuntu-24-04-lts-long-name';
  return s;
}

// regression (d)-N2（設計審核，task 2.3 fix round 2）：兩個「同前綴」的長 id——上一輪
// `.runtime-lamp-id { max-width: 12ch; }` 會把兩者都截成一樣（例如 "lab-server-01" 與
// "lab-server-02" 都截成 "lab-server-…"，讀不出差異，見設計複審截圖 idTrunc）。這裡刻意只放
// 兩個 runtime、id 長度中等（24～26 字元，不是 60 字元）——在 1200px 寬、扣掉產品名之後還有
// 上千 px 空間，兩顆燈號應該放得下、完整顯示、彼此分得出來，驗證 N2「空間足夠時完整顯示」
// 這個訴求，跟 craftManyLongRuntimesState() 驗證的「真的放不下、整顆收進 +N」是互補的兩種
// 情境（N1／N2 分別對應「放不下」與「放得下但不該亂截」）。
function craftSamePrefixRuntimesState() {
  const s = loadFixture();
  s.version = 12346;
  const template = s.runtimes[0];
  const ids = ['lab-server-0001-primary', 'lab-server-0001-secondary'];
  s.runtimes = ids.map((id) => {
    const clone = JSON.parse(JSON.stringify(template));
    clone.id = id;
    clone.workspaces = [];
    return clone;
  });
  return s;
}

// ---------------------------------------------------------------------------
// S1：state 注入工具自我測試
// ---------------------------------------------------------------------------

async function partSelfTestInjection() {
  log('=== S1. 自我測試：state 注入工具（design D9）===');
  let preview = null;
  let chrome = null;
  try {
    // 短推送間隔：若「擋推送」失效，真的推送會很快把注入的內容蓋掉，讓這個自我測試有辨識力。
    preview = await startPreview({ COCKPIT_PREVIEW_PUSH_MS: '200' }, 'preview-S1');
    const url = `http://127.0.0.1:${preview.port}/`;
    chrome = await startChrome(pickPort(19000, [preview.port]), url, 'chrome-S1');
    const { cdp } = chrome;
    await cdp.waitFor(
      "typeof window.onState === 'function'",
      5000,
      '真正的 window.onState（render.js 掛上去的那個）已經就緒'
    );
    // `#app-name` 在 index.html 的靜態占位內容裡就存在（見 index.html「首次占位內容」），第一則
    // /ws 訊息抵達前就已經在畫面上，不能拿來當「真投影已經畫出」的判準；`.runtime-cards` 同樣
    // 是靜態占位的一部分（Ruling R22／Codex C2，2.3 fix round 1：`<div class="runtime-cards">`
    // 是空殼、頁面一載入就存在，不是可靠訊號），一律改用 `waitForFirstProjection()`（頂列出現
    // `[data-runtime]` 節點且 `#version` 等於 `/api/state` 的 version）。
    await waitForFirstProjection(cdp, preview.port);

    const marker = 'INJECT-MARKER-' + Date.now();
    const state = loadFixture();
    state.version = 999;
    state.projects = [
      {
        id: 'inject-marker',
        name: marker,
        stages: ['S'],
        warnings: [],
        workstreams: [],
        tasks: [],
      },
    ];
    // fix round 1／Codex F2：countBlocked=true 讓擋推送函數順便計數，之後用輪詢「真的攔到
    // 至少兩次」取代固定睡多久——固定睡 1.5 秒只能證明「這段時間內畫面沒被真投影蓋掉」，證明
    // 不了「真的有推送發生但被擋住」（萬一 WebSocket 已經斷線、或伺服器沒有照節奏推送，同樣會
    // 讓 marker 保留，卻不是因為擋推送生效）。
    const result = await injectState(cdp, state, { countBlocked: true });
    check(result && result.ok === true, `注入工具回報成功（實際 ${JSON.stringify(result)}）`);

    const markerVisible = await cdp.eval(
      `document.body.textContent.indexOf(${JSON.stringify(marker)}) !== -1`
    );
    check(markerVisible === true, '注入後畫面立刻反映特製投影（找得到 marker 文字）— 證明「以原函數畫特製投影」有效');

    // 「擋推送」的辨識力：輪詢直到 window.__cockpitBlockedPushCount 至少數到 2（推送間隔
    // 200 ms，逾時給 3 秒，理論上足夠攔到至少 15 次），確認注入之後真的收到了 ≥2 次會被擋住
    // 的真推送，不是靠運氣沒有新推送；每次計數增加後都立刻確認 marker 仍在（不是只在等待結束
    // 後看最後一眼）。
    let lastCount = 0;
    const start = Date.now();
    while (Date.now() - start < 3000 && lastCount < 2) {
      await sleep(50);
      const count = await cdp.eval('window.__cockpitBlockedPushCount || 0');
      if (count > lastCount) {
        lastCount = count;
        const stillVisible = await cdp.eval(
          `document.body.textContent.indexOf(${JSON.stringify(marker)}) !== -1`
        );
        check(
          stillVisible === true,
          `第 ${count} 次真推送被攔下之後 marker 仍在（證明這次攔截真的沒有讓真投影蓋掉畫面）`
        );
      }
    }
    check(
      lastCount >= 2,
      `3 秒內應該攔到至少 2 次真推送（推送間隔 200 ms；實際攔到 ${lastCount} 次）— 證明「換成空函數擋推送」真的在攔截真投影，不是巧合沒有新推送`
    );
  } finally {
    await stopChrome(chrome, 'chrome-S1');
    await stopPreview(preview, 'preview-S1');
  }
}

// ---------------------------------------------------------------------------
// S2：對比計算工具自我測試
// ---------------------------------------------------------------------------

async function partSelfTestContrast() {
  log('=== S2. 自我測試：對比計算工具 ===');
  let preview = null;
  let chrome = null;
  try {
    preview = await startPreview({}, 'preview-S2');
    const url = `http://127.0.0.1:${preview.port}/`;
    chrome = await startChrome(pickPort(19010, [preview.port]), url, 'chrome-S2');
    const { cdp } = chrome;
    await waitForFirstProjection(cdp, preview.port);
    await installTools(cdp);

    // 已知配對：白字黑底（約 21:1，應該過 4.5:1）／深灰字淺灰底（刻意選一組低於 4.5:1 的
    // 配對）；用真正的 DOM 節點（不是手算），確保工具真的在讀 getComputedStyle，不是抄公式
    // 之後回傳固定值。
    const known = await cdp.eval(`(() => {
      var wrap = document.createElement('div');
      wrap.id = '__ct_wrap';
      wrap.style.position = 'fixed';
      wrap.style.left = '-9999px';
      wrap.innerHTML =
        '<div id="__ct_good" style="background:#000000;color:#ffffff;">good</div>' +
        '<div id="__ct_bad" style="background:#777777;color:#888888;">bad</div>';
      document.body.appendChild(wrap);
      var good = window.__cockpitVisualTools.textContrast(document.getElementById('__ct_good'));
      var bad = window.__cockpitVisualTools.textContrast(document.getElementById('__ct_bad'));
      document.body.removeChild(wrap);
      return { good: good.ratio, bad: bad.ratio };
    })()`);
    check(
      known.good >= 20.5 && known.good <= 21.5,
      `白字黑底的對比應該接近 21:1（實際 ${known.good}）`
    );
    check(
      known.good >= 4.5,
      `白字黑底應該過 4.5:1 門檻（實際 ${known.good}）— 正例`
    );
    check(
      known.bad < 4.5,
      `深灰字淺灰底應該低於 4.5:1 門檻（實際 ${known.bad}）— 反例，證明工具抓得到不合格的配對`
    );

    // 半透明疊層 compositing：中間插一層 50% 透明的疊色，效果應該等於手算的疊合值，不是只看
    // 最近一層本身（brief：計算「最近一層不透明背景」時要把半透明層疊回去）。
    const composited = await cdp.eval(`(() => {
      var wrap = document.createElement('div');
      wrap.id = '__ct_wrap2';
      wrap.style.position = 'fixed';
      wrap.style.left = '-9999px';
      // 底層不透明黑，中間層 50% 白疊上去（有效背景應該變成約 rgb(128,128,128)），文字白色。
      wrap.innerHTML =
        '<div style="background:#000000;">' +
        '<div style="background:rgba(255,255,255,0.5);">' +
        '<span id="__ct_layered" style="color:#ffffff;">x</span>' +
        '</div></div>';
      document.body.appendChild(wrap);
      var bg = window.__cockpitVisualTools.effectiveBackground(document.getElementById('__ct_layered'));
      document.body.removeChild(wrap);
      return bg;
    })()`);
    check(
      Math.abs(composited.r - 128) <= 2 && Math.abs(composited.g - 128) <= 2 && Math.abs(composited.b - 128) <= 2,
      `50% 白疊在黑底上的有效背景應該接近 rgb(128,128,128)（實際 ${JSON.stringify(composited)}）— 證明工具真的在 composite 半透明層，不是只看最近一層`
    );

    // fix round 1／Codex F1 反例 1：CSS `opacity`（元素自己，不是 background-color 的
    // alpha）。外層不透明黑底，內層 `opacity: 0.35`＋自己不透明的白字——舊版工具只看
    // backgroundColor／color 字面值，會讀到「白字黑底」＝21:1（PASS）；正確算法要把 0.35
    // 疊回外層背景，結果應該明顯低於 4.5:1（FAIL）。兩個數字都印出來，直接證明「不看 opacity
    // 會誤判成通過」這件事本身成立，不是憑空捏造的疑慮。
    const opacityCase = await cdp.eval(`(() => {
      var wrap = document.createElement('div');
      wrap.id = '__ct_wrap3';
      wrap.style.position = 'fixed';
      wrap.style.left = '-9999px';
      wrap.style.background = '#000000';
      wrap.innerHTML = '<div id="__ct_op" style="opacity:0.35;"><span style="color:#ffffff;">x</span></div>';
      document.body.appendChild(wrap);
      var span = wrap.querySelector('span');
      var naiveRatio = window.__cockpitVisualTools.contrastRatio(
        window.__cockpitVisualTools.parseColor(getComputedStyle(span).color),
        window.__cockpitVisualTools.effectiveBackground(span)
      );
      var corrected = window.__cockpitVisualTools.textContrast(span);
      document.body.removeChild(wrap);
      return { naiveRatio: naiveRatio, correctedRatio: corrected.ratio, correctedFg: corrected.fg };
    })()`);
    check(
      opacityCase.naiveRatio >= 20.5,
      `opacity 反例：只看 backgroundColor／color 字面值（忽略 CSS opacity）算出的對比應該還是白字黑底的 21:1（實際 ${opacityCase.naiveRatio}）— 證明「舊算法會誤判成通過」這個疑慮成立`
    );
    check(
      opacityCase.correctedRatio < 4.5,
      `opacity 反例：套用 element 自己的 CSS opacity:0.35 疊回外層背景之後，真正的對比應該掉到 4.5:1 以下（實際 ${opacityCase.correctedRatio}，疊色後前景 ${JSON.stringify(opacityCase.correctedFg)}）— 證明修正後的工具抓得到 opacity 造成的視覺降對比`
    );

    // fix round 1／Codex F1 反例 2：文字自己的 `color: rgba(...)` alpha（不是元素的 CSS
    // opacity）。同樣先印出「忽略 alpha」的舊算法會誤判成通過，再證明修正後的工具正確地把
    // alpha 疊進背景、抓到真正偏低的對比。
    const fgAlphaCase = await cdp.eval(`(() => {
      var wrap = document.createElement('div');
      wrap.id = '__ct_wrap4';
      wrap.style.position = 'fixed';
      wrap.style.left = '-9999px';
      wrap.style.background = '#000000';
      wrap.innerHTML = '<span id="__ct_fga" style="color:rgba(255,255,255,0.3);">x</span>';
      document.body.appendChild(wrap);
      var span = document.getElementById('__ct_fga');
      var rawParsed = window.__cockpitVisualTools.parseColor(getComputedStyle(span).color);
      var naiveRatio = window.__cockpitVisualTools.contrastRatio(
        { r: rawParsed.r, g: rawParsed.g, b: rawParsed.b, a: 1 }, // 舊算法：直接拿 r/g/b，當作不透明
        window.__cockpitVisualTools.effectiveBackground(span)
      );
      var corrected = window.__cockpitVisualTools.textContrast(span);
      document.body.removeChild(wrap);
      return { rawAlpha: rawParsed.a, naiveRatio: naiveRatio, correctedRatio: corrected.ratio, correctedFg: corrected.fg };
    })()`);
    check(
      Math.abs(fgAlphaCase.rawAlpha - 0.3) < 0.01,
      `前景 alpha 反例：getComputedStyle 讀回的 color 應該保留 alpha=0.3（實際 ${fgAlphaCase.rawAlpha}）`
    );
    check(
      fgAlphaCase.naiveRatio >= 20.5,
      `前景 alpha 反例：忽略 rgba 的 alpha、只拿 r/g/b 當不透明色算出的對比應該還是 21:1（實際 ${fgAlphaCase.naiveRatio}）— 證明「舊算法會誤判成通過」這個疑慮成立`
    );
    check(
      fgAlphaCase.correctedRatio < 4.5,
      `前景 alpha 反例：把 rgba(255,255,255,0.3) 的 alpha 疊回背景之後，真正的對比應該掉到 4.5:1 以下（實際 ${fgAlphaCase.correctedRatio}，疊色後前景 ${JSON.stringify(fgAlphaCase.correctedFg)}）— 證明修正後的工具抓得到前景半透明造成的視覺降對比`
    );
  } finally {
    await stopChrome(chrome, 'chrome-S2');
    await stopPreview(preview, 'preview-S2');
  }
}

// ---------------------------------------------------------------------------
// S3：命中測試工具自我測試
// ---------------------------------------------------------------------------

async function partSelfTestHitTest() {
  log('=== S3. 自我測試：命中測試工具 ===');
  let preview = null;
  let chrome = null;
  try {
    preview = await startPreview({}, 'preview-S3');
    const url = `http://127.0.0.1:${preview.port}/`;
    chrome = await startChrome(pickPort(19020, [preview.port]), url, 'chrome-S3');
    const { cdp } = chrome;
    await waitForFirstProjection(cdp, preview.port);
    await installTools(cdp);

    const result = await cdp.eval(`(() => {
      var target = document.createElement('div');
      target.id = '__ht_target';
      target.textContent = 't';
      target.style.position = 'fixed';
      target.style.left = '100px';
      target.style.top = '100px';
      target.style.width = '80px';
      target.style.height = '30px';
      document.body.appendChild(target);
      var clear = window.__cockpitVisualTools.hitCenter('#__ht_target');

      var blocker = document.createElement('div');
      blocker.id = '__ht_blocker';
      blocker.style.position = 'fixed';
      blocker.style.left = '0';
      blocker.style.top = '0';
      blocker.style.width = '400px';
      blocker.style.height = '400px';
      blocker.style.background = 'red';
      document.body.appendChild(blocker);
      var blocked = window.__cockpitVisualTools.hitCenter('#__ht_target');

      document.body.removeChild(blocker);
      document.body.removeChild(target);
      return { clear, blocked };
    })()`);
    check(
      result.clear && result.clear.visible === true && result.clear.selfHit === true,
      `沒有遮蔽物時命中測試應該回報 selfHit=true（實際 ${JSON.stringify(result.clear)}）— 正例`
    );
    check(
      result.blocked && result.blocked.visible === true && result.blocked.selfHit === false,
      `被更高層的元素蓋住時命中測試應該回報 selfHit=false（實際 ${JSON.stringify(result.blocked)}）— 反例，證明工具抓得到「被蓋住」`
    );
  } finally {
    await stopChrome(chrome, 'chrome-S3');
    await stopPreview(preview, 'preview-S3');
  }
}

// ---------------------------------------------------------------------------
// S4：自我測試：finalSweep 的行程所有權模型（fix round 3／控制端裁決 R14：不再用 PID＋事後
// 身分比對，改成「還握著 ChildProcess 物件、且還沒觀察到 exit 事件」這個唯一依據；見
// finalSweep() 上方的三段一手來源）。不需要 chrome／preview——finalSweep() 是純粹操作
// SPAWNED_CHILDREN 的 Node 端函式，用真的、完全掌控存活時間的 node 子行程直接測。三個情境
// （R14 item 5）：(a) 活著的 child 會被終止；(b) 已經 exit 的 child 不會觸發任何終止命令
// （用 TASKKILL_PID_CALLS 的次數斷言，不是只看行為結果）；(c) 收尾失敗（沒觀察到 exit）時
// 仍留在追蹤集合。
// ---------------------------------------------------------------------------

async function partSelfTestFinalSweepOwnership() {
  log('=== S4. 自我測試：finalSweep 的行程所有權模型（(a) 終止活著的／(b) 不動已 exit 的／(c) 收尾失敗仍保留追蹤／(d) 已 exit 但 port 仍 LISTEN 只警告）===');

  // --- (a) 活著的 child 會被 finalSweep 終止 ---
  const aliveProbe = spawn(process.execPath, ['-e', 'setTimeout(() => {}, 8000)'], {
    stdio: 'ignore',
    windowsHide: true,
  });
  await sleep(300); // 讓行程真的起來，pidStillRunning() 才讀得到。
  check(pidStillRunning(aliveProbe.pid), `(a) 存活探針（PID ${aliveProbe.pid}）應該處於存活狀態，才能有意義地驗證 finalSweep 會終止它`);
  check(!hasObservedExit(aliveProbe), `(a) 存活探針一開始不應該觀察到 exit（實際 exitCode=${aliveProbe.exitCode}, signalCode=${aliveProbe.signalCode}）`);
  trackChild(aliveProbe, 'S4(a)-存活探針', undefined);

  const taskkillCountBeforeA = TASKKILL_PID_CALLS.length;
  const resultA = finalSweep(false); // assertZeroKilled=false：這裡故意留一個活探針讓它補殺。
  check(resultA.killed === 1, `(a) finalSweep 應該補送終止剛好 1 個（存活探針）（實際 killed=${resultA.killed}）`);
  check(
    TASKKILL_PID_CALLS.length === taskkillCountBeforeA + 1 &&
      TASKKILL_PID_CALLS[TASKKILL_PID_CALLS.length - 1] === aliveProbe.pid,
    `(a) finalSweep 應該對存活探針的 PID 送出剛好 1 次 taskkill（實際新增 ${TASKKILL_PID_CALLS.length - taskkillCountBeforeA} 次）`
  );
  const aExited = await waitForChildExit(aliveProbe, 5000);
  check(aExited, `(a) 存活探針被 finalSweep 終止後應該觀察到 exit 事件`);
  check(!pidStillRunning(aliveProbe.pid), `(a) 存活探針被 finalSweep 終止後應該不再存活（tasklist 查無此 PID）`);
  if (aExited) untrackChild(aliveProbe);

  // --- (b) 已經 exit 的 child 不會觸發任何終止命令 ---
  const exitedProbe = spawn(process.execPath, ['-e', '0'], { stdio: 'ignore', windowsHide: true }); // 立刻結束。
  const exitedForReal = await waitForChildExit(exitedProbe, 5000);
  check(exitedForReal, `(b) 探針應該很快就自然結束、觀察到 exit 事件（PID ${exitedProbe.pid}）`);
  check(
    hasObservedExit(exitedProbe),
    `(b) 已 exit 的探針 exitCode／signalCode 應該至少一個非 null（實際 exitCode=${exitedProbe.exitCode}, signalCode=${exitedProbe.signalCode}）`
  );
  trackChild(exitedProbe, 'S4(b)-已exit探針', undefined);

  const taskkillCountBeforeB = TASKKILL_PID_CALLS.length;
  const resultB = finalSweep(false);
  check(resultB.killed === 0, `(b) finalSweep 對已經 exit 的探針不應該送出任何終止命令（killed 應該是 0，實際 ${resultB.killed}）`);
  check(
    TASKKILL_PID_CALLS.length === taskkillCountBeforeB,
    `(b) finalSweep 處理已 exit 的探針時，taskkill 呼叫次數不應該增加（實際新增 ${TASKKILL_PID_CALLS.length - taskkillCountBeforeB} 次）— 直接證明「已觀察到 exit 就絕不送終止命令」，不是只看行為結果的間接推論`
  );
  // finalSweep() 對已觀察到 exit 的項目一律自己 untrackChild（見該函式實作），這裡不用再做。

  // --- (c) 收尾失敗（沒觀察到 exit）時仍留在追蹤集合，不會被提早 untrack ---
  const cProbe = spawn(process.execPath, ['-e', 'setTimeout(() => {}, 8000)'], {
    stdio: 'ignore',
    windowsHide: true,
  });
  await sleep(300);
  check(pidStillRunning(cProbe.pid), `(c) 探針應該處於存活狀態`);
  trackChild(cProbe, 'S4(c)-模擬收尾失敗探針', undefined);
  // 模擬「收尾失敗」：不送終止命令，直接用逾時 0 ms 去等 exit——探針還活著、不可能在這個窗口內
  // 結束，waitForChildExit() 一定回傳 false，忠實重現 stopChrome／stopPreview 遇到「送出終止
  // 後仍未觀察到 exit」時的同一種狀態（判斷依據就是這個回傳值，不是另外模擬一套邏輯）。
  const cExitedWithinShortWindow = await waitForChildExit(cProbe, 0);
  check(cExitedWithinShortWindow === false, `(c) 模擬收尾失敗：極短逾時內不應該觀察到 exit（探針還活著）（實際 ${cExitedWithinShortWindow}）`);
  // 比照 stopChrome／stopPreview 的判斷式：沒觀察到 exit 就不能 untrack。
  const stillTrackedAfterFailure = SPAWNED_CHILDREN.some((e) => e.child === cProbe);
  check(stillTrackedAfterFailure, `(c) 收尾失敗（沒觀察到 exit）時，探針應該仍然留在追蹤集合（不會被提早 untrack，留給 finalSweep() 重試）`);

  // 測試收尾：把 (c) 的探針真正清乾淨，不留殘留。
  killTree(cProbe, 'S4(c) 測試收尾');
  const cExited = await waitForChildExit(cProbe, 5000);
  check(cExited, `測試收尾：(c) 探針真正終止後應該觀察到 exit 事件`);
  if (cExited) untrackChild(cProbe);
  check(!pidStillRunning(cProbe.pid), `測試收尾：(c) 探針應該已經不再存活`);

  // --- (d) 父行程已 exit、但 port 仍在 LISTEN（fix round 4／Codex r3 medium）---
  // 模擬「漏網孫行程占著 port」：探針是真的 node 子行程、自然結束；port 由本行程自己開的
  // net server 占住（完全掌控，測完關掉），走的是四個收尾路徑共用的 settleTrackedChild()，
  // 不是另外模擬一套判斷。期望：保留在追蹤集合 → finalSweep 警告 1 次、taskkill 0 次。
  const dPort = pickPort(7990);
  const holder = net.createServer();
  await new Promise((res, rej) => {
    holder.once('error', rej);
    holder.listen(dPort, '127.0.0.1', res);
  });
  try {
    check(isPortListening(dPort), `(d) 占 port 的 helper 應該讓 port ${dPort} 處於 LISTENING`);
    const dProbe = spawn(process.execPath, ['-e', '0'], { stdio: 'ignore', windowsHide: true });
    trackChild(dProbe, 'S4(d)-父已exit但port仍LISTEN探針', dPort);
    const dSettle = await settleTrackedChild(dProbe, dPort, 'S4(d)');
    check(dSettle.exited, `(d) 探針應該自然結束、觀察到 exit 事件（PID ${dProbe.pid}）`);
    check(dSettle.portListening, `(d) 收尾判斷應該看到 port ${dPort} 仍在 LISTENING（實際 ${dSettle.portListening}）`);
    check(
      SPAWNED_CHILDREN.some((e) => e.child === dProbe),
      `(d) 父行程已 exit 但 port 仍 LISTENING 時，應該保留在追蹤集合，留給 finalSweep() 警告`
    );
    const taskkillCountBeforeD = TASKKILL_PID_CALLS.length;
    const resultD = finalSweep(false);
    check(resultD.residualWarnings === 1, `(d) finalSweep 應該對這個項目印出 1 次殘留警告（實際 ${resultD.residualWarnings}）`);
    check(resultD.killed === 0, `(d) finalSweep 不應該補送任何終止（實際 killed=${resultD.killed}）`);
    check(
      TASKKILL_PID_CALLS.length === taskkillCountBeforeD,
      `(d) finalSweep 處理「已 exit、port 仍 LISTEN」時 taskkill 呼叫次數應該是 0（實際新增 ${TASKKILL_PID_CALLS.length - taskkillCountBeforeD} 次）`
    );
    untrackChild(dProbe); // 測試收尾：無論上面斷言結果如何都不留在集合裡。
  } finally {
    await new Promise((res) => holder.close(res));
  }
  check(!isPortListening(dPort), `測試收尾：(d) helper 關閉後 port ${dPort} 應該不再 LISTENING`);
}

// ---------------------------------------------------------------------------
// S5：「第一份真投影已經畫出」判準自我測試（task 2.3 fix round 1；Ruling R22／Codex C2）
// ---------------------------------------------------------------------------

// 攔截器：在 render.js 把 `window.onState` 賦值成真正的重畫函式之前（透過
// `Page.addScriptToEvaluateOnNewDocument`，在這份文件任何腳本執行前就注入），用
// `Object.defineProperty` 的 setter 攔截這次賦值，把「第一次真正被呼叫」包一層
// `setTimeout` 延遲——channel.js 收到 /ws 第一則訊息時仍然會呼叫 `window.onState(state)`
// （這一步本身沒有被擋，跟 S1 的「擋推送」不同），只是真正跑 `renderState()`／
// `replaceChildren` 那一刻被延後，模擬「WebSocket 訊息抵達了，但畫面還沒重畫完」這個窗口。
const DELAY_ONSTATE_SCRIPT = `
(function () {
  var real;
  var first = true;
  var DELAY_MS = 1200;
  Object.defineProperty(window, 'onState', {
    configurable: true,
    get: function () { return real; },
    set: function (fn) {
      real = function (state) {
        if (first) {
          first = false;
          setTimeout(function () { fn(state); }, DELAY_MS);
        } else {
          fn(state);
        }
      };
    },
  });
})();
`;

async function partSelfTestFirstProjectionWait() {
  log('=== S5. 自我測試：「第一份真投影已經畫出」判準的負對照（Ruling R22／Codex C2）===');
  let preview = null;
  let chrome = null;
  try {
    preview = await startPreview({}, 'preview-S5');
    chrome = await startChrome(pickPort(19005, [preview.port]), 'about:blank', 'chrome-S5');
    const { cdp } = chrome;

    // fix（本段自查）：計時起點必須是 Page.navigate 之前，不能等到 waitForFirstProjection()
    // 呼叫前才開始算——攔截器的 1.2 秒延遲是從「channel.js 第一次呼叫 window.onState」算起，
    // 這件事在下面的 `.topbar` waitFor 與 duringDelay 的 CDP round-trip 期間就可能已經過了
    // 大半（CDP 通訊本身有實際的毫秒級耗時），只量最後一段會低估、誤判成「新條件沒有真的等」。
    //
    // fix（本段自查，第二個坑）：`Page.addScriptToEvaluateOnNewDocument` 沒有先送
    // `Page.enable` 的話，注入的腳本不會真的在下一次導覽時執行——本檔其餘地方（例如 FN1）都
    // 只呼叫 `Page.navigate` 沒呼叫過 `Page.addScriptToEvaluateOnNewDocument`，所以沒有踩過
    // 這個坑；用一支獨立 debug 腳本（在頁面上設一個 `window.__marker` 字串常數，跟
    // `window.onState` 的攔截邏輯完全無關）重現：沒有 `Page.enable` 時，導覽後
    // `window.__marker` 是 `undefined`；加上 `Page.enable` 後變成注入的字串——證實問題出在
    // CDP 呼叫順序，不是攔截器邏輯本身（那段邏輯从一開始就沒被執行過，S5 一開始的假 GREEN
    // 其實是「攔截器從未安裝、真正的 window.onState 從未被延遲」造成的巧合，不是負對照真的
    // 通過）。
    const flowStart = Date.now();
    await cdp.send('Page.enable', {});
    await cdp.send('Page.addScriptToEvaluateOnNewDocument', { source: DELAY_ONSTATE_SCRIPT });
    const url = `http://127.0.0.1:${preview.port}/`;
    await cdp.send('Page.navigate', { url });

    // 等到靜態占位真的解析完成（`.topbar` 這個框存在）——這一步不依賴任何 WS 活動，純粹是
    // HTML 解析＋render.js 賦值 window.onState（被攔截器包住）完成。這是負對照要卡住的時間
    // 窗：channel.js 已經連上 /ws、收到第一則訊息、呼叫了「被包住」的 window.onState，但真正
    // 的 renderState() 還沒執行（攔截器的 setTimeout 還沒到）。
    await cdp.waitFor("!!document.querySelector('.topbar')", 5000, '靜態占位已經解析完成（.topbar 存在）');

    const duringDelay = await cdp.eval(`(() => {
      var v = document.getElementById('version');
      return {
        oldConditionTrue: !!document.querySelector('.runtime-cards'),
        hasLamp: !!document.querySelector('[data-region="topbar"] [data-runtime]'),
        versionText: v ? v.textContent : null,
      };
    })()`);
    check(
      duringDelay.oldConditionTrue === true,
      `否定對照：延遲首份 onState 期間，舊條件（!!document.querySelector('.runtime-cards')）應該提早（錯誤地）成立（實際 ${duringDelay.oldConditionTrue}）——證明舊條件確實會被 index.html 的靜態占位騙過，不是本檔誤判`
    );
    check(
      duringDelay.hasLamp === false,
      `延遲期間，新條件的「頂列出現 [data-runtime] 節點」應該還沒成立（實際 ${duringDelay.hasLamp}）`
    );
    check(
      !duringDelay.versionText,
      `延遲期間，#version 應該還是靜態占位的空字串（實際 ${JSON.stringify(duringDelay.versionText)}）`
    );

    // 新條件：正確的行為是「等到延遲結束、真投影真的畫出來才通過」——用「從 Page.navigate 算起
    // 的總耗時」佐證，不是只量 waitForFirstProjection() 這次呼叫本身的耗時（那樣會把前面
    // `.topbar` waitFor 與 duringDelay eval 的 CDP round-trip 時間漏算，低估總延遲、誤判成
    // 「新條件沒有真的等」——本段第一版就踩過這個坑，見上方 flowStart 註解）。
    await waitForFirstProjection(cdp, preview.port, '延遲後新條件應該正確等到真投影畫出');
    const totalElapsedMs = Date.now() - flowStart;
    check(
      totalElapsedMs >= 900,
      `從 Page.navigate 到新條件通過的總耗時應該真的涵蓋攔截器延遲的 1.2 秒（容許 CDP round-trip 誤差，門檻取 900ms；實際 ${totalElapsedMs}ms）——低於這個值代表新條件可能被某種殘留狀態騙過，沒有真的卡住延遲窗口`
    );
  } finally {
    await stopChrome(chrome, 'chrome-S5');
    await stopPreview(preview, 'preview-S5');
  }
}

// ---------------------------------------------------------------------------
// V1：dashboard/桌面寬度不整頁捲動＋長名稱不溢出＋網格過寬時不撐破頁面（1536×1024；task 2.1）
// ---------------------------------------------------------------------------

async function partViewportDesktop() {
  log('=== V1. 1536x1024：桌面寬度不整頁捲動／長名稱不溢出／網格過寬時不撐破頁面 ===');
  let preview = null;
  let chrome = null;
  try {
    preview = await startPreview({}, 'preview-V1');
    const url = `http://127.0.0.1:${preview.port}/`;
    chrome = await startChrome(pickPort(19030, [preview.port]), url, 'chrome-V1', '1536,1024');
    const { cdp } = chrome;
    await waitForFirstProjection(cdp, preview.port);
    await installTools(cdp);

    const wide = craftWideState();
    const result = await injectState(cdp, wide);
    check(result && result.ok === true, '注入含兩個 runtime（各 5 pane）／10 stages／200 字元名稱／30 筆事件的特製投影');
    await cdp.waitFor(
      "document.body.textContent.indexOf('Wide Project') !== -1",
      5000,
      '特製投影已經畫出（找得到 Wide Project）'
    );

    // --- 桌面寬度不整頁捲動（spec cockpit-dashboard「版面與窄視窗」）---
    const scroll = await cdp.eval(`(() => ({
      scrollHeight: document.documentElement.scrollHeight,
      innerHeight: window.innerHeight,
      scrollWidth: document.documentElement.scrollWidth,
      innerWidth: window.innerWidth,
    }))()`);
    check(
      scroll.scrollHeight <= scroll.innerHeight,
      `視窗 ≥1200px 時頁面不整頁捲動：document.documentElement.scrollHeight（${scroll.scrollHeight}）不應該大於視窗高度（${scroll.innerHeight}）— design D2／D3 的 grid 外框（task 2.1）尚未實作，目前預期 FAIL`
    );

    // 頂列／左欄／Factory Floor／runtime 卡／最近事件／底列都在視窗內可見（design D2
    // data-region）。
    const regions = ['topbar', 'projects', 'floor', 'runtimes', 'events', 'statusbar'];
    for (const region of regions) {
      const info = await cdp.eval(`(() => {
        var n = document.querySelector('[data-region="${region}"]');
        if (!n) return null;
        var r = n.getBoundingClientRect();
        return { top: r.top, bottom: r.bottom, left: r.left, right: r.right };
      })()`);
      check(
        info !== null &&
          info.top >= 0 &&
          info.bottom <= scroll.innerHeight &&
          info.left >= 0 &&
          info.right <= scroll.innerWidth,
        `data-region="${region}" 存在且整塊落在視窗內（design D2；目前 data-region 尚未實作，預期 FAIL；實際 ${JSON.stringify(info)}）`
      );
    }

    // Live Output 在視窗內可見（Ruling R2 的 PENDING(4.1) 在 direction-01-visual task 4.1 轉成真
    // 斷言）：這裡沒有選取任何 pane，面板以常駐空狀態出現（design D7），整塊落在視窗內、有實際
    // 尺寸（不是 display:none 的 0×0），空狀態文字本身也畫在面板範圍內。
    const outputInfo = await cdp.eval(`(() => {
      var n = document.querySelector('[data-region="output"]');
      if (!n) return null;
      var r = n.getBoundingClientRect();
      var e = n.querySelector('.output-empty');
      var er = e && e.getClientRects().length > 0 ? e.getBoundingClientRect() : null;
      return {
        top: r.top, bottom: r.bottom, left: r.left, right: r.right, width: r.width, height: r.height,
        display: getComputedStyle(n).display,
        empty: er ? { top: er.top, bottom: er.bottom, left: er.left, right: er.right } : null,
      };
    })()`);
    check(
      outputInfo !== null &&
        outputInfo.display !== 'none' &&
        outputInfo.width > 0 &&
        outputInfo.height >= 239.5 &&
        outputInfo.top >= 0 &&
        outputInfo.bottom <= scroll.innerHeight &&
        outputInfo.left >= 0 &&
        outputInfo.right <= scroll.innerWidth,
      `dashboard/桌面寬度不整頁捲動：Live Output（沒有選取、空狀態）整塊落在視窗內且高度 ≥240px（實際 ${JSON.stringify(outputInfo)}）`
    );
    check(
      outputInfo !== null &&
        outputInfo.empty !== null &&
        outputInfo.empty.top >= outputInfo.top &&
        outputInfo.empty.bottom <= outputInfo.bottom &&
        outputInfo.empty.left >= outputInfo.left &&
        outputInfo.empty.right <= outputInfo.right,
      `dashboard/桌面寬度不整頁捲動：Live Output 空狀態文字畫在面板範圍內（實際 ${JSON.stringify(outputInfo && outputInfo.empty)}）`
    );

    // --- 長名稱不溢出（spec「版面與窄視窗」情境「長名稱不溢出」：「兩者都沒有超出所屬區域的
    // 邊界，頁面也沒有出現橫向捲軸」）。
    //
    // fix round 4／控制端 Ruling R18（換設計）：round 1～3 的 fits()／withinRegion()／viaScroll
    // 例外連續三輪被判恆真——「元素落在某個捲動祖先的 scrollWidth 範圍內就算可捲達」會被溢出
    // 元素自己撐大的 scrollWidth 作證（Codex r3），整段刪掉，改成最簡判準：
    //   1. 先把所有捲動容器（含整頁）的 scrollLeft／scrollTop 歸零；
    //   2. 只比水平方向：目標文字元素的未裁切範圍 [left,right] 必須落在所屬 data-region 未裁切
    //      rect 的 [left,right] 內，不設任何捲動例外（R18_MEASURE_JS 的 extentOf()：元素自己
    //      overflow-x 為 visible 時，範圍＝元素框與其文字 Range 框的聯集——不換行也不省略的
    //      長字串會畫出自己的框，只看元素框量不到；元素自己裁切時〔省略〕範圍就是元素框）；
    //   3. 目標＝workstream 名稱（Factory Floor 首欄）、pane 標題、pane cwd，全部逐一量；
    //      Factory Floor 的 stage 欄（spec 允許區內橫向捲動）不列入。
    // 缺元素、缺 region、元素不在所屬 region 裡、display:none／visibility:hidden、零尺寸一律硬
    // 失敗。pane 標題／cwd 目前因右欄 .pane-row 的欄位總寬大於 300px 右欄而超出 runtimes 右緣
    // （3.3 改 pane 兩行排列才會落回區域內）：只有符合這個已知特徵（見 isKnownPaneRowOverflow）
    // 才印 PENDING(3.3)；VISUAL_CHECK_STRICT_3_3=1 時一律硬失敗（3.3 的驗收 gate）。
    // direction-01-visual task 3.3 完成後：pane 兩行排列已落地，這個 PENDING 不再有存在理由，
    // 一律嚴格（硬失敗）；VISUAL_CHECK_STRICT_3_3 環境變數不再有作用，設不設結果相同。
    const strict33 = true;
    // 已知的 3.3 特徵（只看水平）：元素與 region 都存在且可見、不是零尺寸、元素在 .pane-row 裡、
    // 左緣沒有越過 region 左緣、右緣越過 region 右緣，而且它所在的 .pane-row 排版內容的右緣也越過
    // region 右緣（整列 grid 欄位總寬超過右欄），且元素右緣沒有越過該列欄位排版的右緣（不是元素自己
    // 被位移）。其餘任何失敗（含 Factory Floor 的
    // workstream 名稱）都不是這個特徵。
    function isKnownPaneRowOverflow(res) {
      if (!res || res.exists !== true || res.contained === true) return false;
      if (res.regionMissing || res.notInRegion || res.hidden || res.zeroSize) return false;
      if (!res.ext || !res.region || res.paneRowRight === null || res.paneRowRight === undefined) return false;
      return (
        res.ext.left >= res.region.left - 0.5 &&
        res.ext.right > res.region.right + 0.5 &&
        res.paneRowRight > res.region.right + 0.5 &&
        res.ext.right <= res.paneRowRight + 0.5
      );
    }
    const containment = await cdp.eval(`(() => {
      ${R18_MEASURE_JS}
      r18ResetScroll();
      return {
        ws: r18MeasureAll('.ff-ws-name', '[data-region="floor"]'),
        title: r18MeasureAll('.pane-title', '[data-region="runtimes"]'),
        cwd: r18MeasureAll('.pane-cwd', '[data-region="runtimes"]'),
        wsText: (document.querySelector('.ff-ws-name') || { textContent: '' }).textContent.length,
        wsTitleOk: (function () { var n = document.querySelector('.ff-ws-name'); return !!n && n.title === n.textContent; })(),
        cwdLong: (function () {
          var list = document.querySelectorAll('.pane-cwd');
          for (var i = 0; i < list.length; i += 1) {
            if (list[i].textContent.length >= 190) return { len: list[i].textContent.length, titleOk: list[i].title === list[i].textContent };
          }
          return null;
        })(),
        pageScrollWidth: document.documentElement.scrollWidth,
        innerWidth: window.innerWidth,
      };
    })()`);
    check(
      containment.wsText === 200 && containment.wsTitleOk === true,
      `特製投影的 200 字元 workstream 名稱存在，且 title 帶完整內容（實際長度 ${containment.wsText}，titleOk=${containment.wsTitleOk}）`
    );
    check(
      containment.cwdLong !== null && containment.cwdLong.titleOk === true,
      `特製投影的長 pane cwd 存在，且 title 帶完整內容（實際 ${JSON.stringify(containment.cwdLong)}）`
    );
    check(
      containment.pageScrollWidth <= containment.innerWidth,
      `長名稱不應該讓頁面出現橫向捲軸（scrollWidth ${containment.pageScrollWidth} vs innerWidth ${containment.innerWidth}）`
    );
    function reportGroup(key, label, allowPending33) {
      const list = containment[key];
      if (!Array.isArray(list) || list.length === 0) {
        check(false, `R18 ${label}：找不到任何目標元素或所屬 region（實際 ${JSON.stringify(list)}）`);
        return;
      }
      const bad = list.filter((r) => r.contained !== true);
      if (bad.length === 0) {
        check(true, `R18 ${label}：${list.length} 個目標的水平範圍都落在所屬 data-region 內（捲動歸零、無捲動例外）`);
        return;
      }
      const allKnown = allowPending33 && bad.every((r) => isKnownPaneRowOverflow(r));
      if (allKnown && !strict33) {
        pending(`dashboard/長名稱不溢出：${label} 的水平範圍落在所屬 data-region 內`, '3.3');
        log(
          `PENDING 原因（${label}）：${bad.length}/${list.length} 個超出 runtimes 右緣，全部符合已知的 .pane-row 超寬特徵（右欄 300px 放不下 pane 列的欄位總寬，3.3 改兩行排列後應轉綠）；第一個：${JSON.stringify(bad[0])}`
        );
        return;
      }
      check(
        false,
        `R18 ${label}：${bad.length}/${list.length} 個目標的水平範圍超出所屬 data-region（strict33=${strict33}，全部符合 3.3 已知特徵=${allKnown}；第一個 ${JSON.stringify(bad[0])}）`
      );
    }
    reportGroup('ws', 'workstream 名稱（Factory Floor 首欄）', false);
    reportGroup('title', 'pane 標題', true);
    reportGroup('cwd', 'pane cwd', true);

    // direction-01-visual task 3.3（2.1 fix round 3 carry／R18 代價）：上面的 R18 量法只看文字元素
    // 是否落在所屬 region 內，右欄內層自己若能橫向捲動（內容被撐寬、捲得到），「落在 region 內」
    // 仍可能成立。另外直接斷言：捲動歸零後，右欄的兩個內層捲動容器都沒有橫向溢出
    // （scrollWidth ≤ clientWidth）；長 cwd 以單行省略或換行呈現（兩者之一），title 帶完整內容。
    const rightInner = await cdp.eval(`(() => {
      ${R18_MEASURE_JS}
      r18ResetScroll();
      function sz(sel) { var n = document.querySelector(sel); return n ? { sw: n.scrollWidth, cw: n.clientWidth } : null; }
      var longCwd = null;
      document.querySelectorAll('.pane-cwd').forEach(function (n) {
        if (longCwd === null && n.textContent.length >= 190) {
          var cs = getComputedStyle(n);
          // task 3.3 fix round 1（設計審核 I1）：cwd 拆成前段／尾段兩個 span 各自省略，省略判準
          // 改看「元素本身、或它的每一個子 span」都是單行省略且被 cwd 格子裁切（overflow hidden）。
          function ell(x) { var c = getComputedStyle(x); return c.whiteSpace === 'nowrap' && c.overflowX === 'hidden' && c.textOverflow === 'ellipsis'; }
          var kids = Array.prototype.slice.call(n.children);
          longCwd = {
            ellipsis: ell(n) || (kids.length > 0 && cs.overflowX === 'hidden' && kids.every(ell)),
            wraps: cs.whiteSpace !== 'nowrap' && (cs.overflowWrap === 'anywhere' || cs.wordBreak === 'break-all'),
            titleOk: n.getAttribute('title') === n.textContent,
          };
        }
      });
      return { cards: sz('[data-region="runtimes"] > .runtime-cards'), events: sz('[data-region="events"] > .recent-events-list'), longCwd: longCwd };
    })()`);
    for (const key of ['cards', 'events']) {
      const v = rightInner[key];
      check(
        !!v && v.sw <= v.cw,
        `右欄內層（${key === 'cards' ? '[data-region="runtimes"] > .runtime-cards' : '[data-region="events"] > .recent-events-list'}）沒有橫向捲動：scrollWidth ${v && v.sw} ≤ clientWidth ${v && v.cw}（長名稱／30 筆事件的特製投影）`
      );
    }
    check(
      !!rightInner.longCwd && (rightInner.longCwd.ellipsis || rightInner.longCwd.wraps) && rightInner.longCwd.titleOk,
      `長 cwd 以單行省略或換行呈現、title 帶完整內容（實際 ${JSON.stringify(rightInner.longCwd)}）`
    );
    // 否定對照：右欄內層塞一段 2000px 不換行文字，scrollWidth ≤ clientWidth 必須轉紅。
    const rightInnerNeg = await cdp.eval(`(() => {
      var out = {};
      [['cards', '[data-region="runtimes"] > .runtime-cards'], ['events', '[data-region="events"] > .recent-events-list']].forEach(function (pair) {
        var sc = document.querySelector(pair[1]);
        if (!sc) { out[pair[0]] = null; return; }
        var host = sc.querySelector('.pane-row, .event-row') || sc;
        var probe = document.createElement('span');
        probe.style.cssText = 'display:inline-block;width:2000px;white-space:nowrap;';
        probe.textContent = 'x';
        host.appendChild(probe);
        out[pair[0]] = { sw: sc.scrollWidth, cw: sc.clientWidth };
        probe.remove();
      });
      return out;
    })()`);
    check(
      !!rightInnerNeg.cards && rightInnerNeg.cards.sw > rightInnerNeg.cards.cw && !!rightInnerNeg.events && rightInnerNeg.events.sw > rightInnerNeg.events.cw,
      `否定對照：右欄兩個內層各塞 2000px 不換行文字時，scrollWidth > clientWidth（實際 ${JSON.stringify(rightInnerNeg)}）`
    );

    // R18 否定／正面對照：全部在同一個 eval 裡注入、量測、還原，不污染後續量測。
    const r18Controls = await cdp.eval(`(() => {
      ${R18_MEASURE_JS}
      var ws = document.querySelector('.ff-ws-name');
      var floor = document.querySelector('[data-region="floor"]');
      var sc = document.querySelector('[data-region="floor"] > .projects');
      if (!ws || !floor || !sc) return null;
      var prev = ws.getAttribute('style');
      function restore() { if (prev === null) ws.removeAttribute('style'); else ws.setAttribute('style', prev); }

      // (a) position: relative 水平位移、而且撐大捲動容器的 scrollWidth——正是 round 3 的
      // viaScroll 例外會放行的情況（元素落在被自己撐大的捲動範圍內）。
      r18ResetScroll();
      var swBefore = sc.scrollWidth;
      ws.style.position = 'relative';
      ws.style.left = (swBefore + 200) + 'px';
      var swAfter = sc.scrollWidth;
      var b = ws.getBoundingClientRect();
      var s = sc.getBoundingClientRect();
      var l = b.left - s.left + sc.scrollLeft;
      var oldViaScrollWouldPass = l >= -1 && l + b.width <= sc.scrollWidth + 1;
      var shifted = r18Measure(ws, '[data-region="floor"]');
      restore();

      // (b) 200 字元、不換行、不省略（overflow 可見）：元素框本身仍在 region 內，只有文字畫出框外。
      r18ResetScroll();
      ws.style.whiteSpace = 'nowrap';
      ws.style.overflowWrap = 'normal';
      ws.style.wordBreak = 'normal';
      ws.style.overflow = 'visible';
      ws.style.textOverflow = 'clip';
      var nowrap = r18Measure(ws, '[data-region="floor"]');
      restore();

      // (c) 正面對照：同樣不換行，但單行省略（overflow hidden＋ellipsis，spec／D11 允許的呈現）
      // 必須判定在區域內，證明新判準沒有變相禁止省略。
      r18ResetScroll();
      ws.style.whiteSpace = 'nowrap';
      ws.style.overflow = 'hidden';
      ws.style.textOverflow = 'ellipsis';
      var ellipsis = r18Measure(ws, '[data-region="floor"]');
      restore();

      // (d) 分類函式對照用：cwd 往左位移到 runtimes 左緣之外（不是已知的整列超寬特徵）。
      var cwd = document.querySelector('.pane-cwd');
      var cwdPrev = cwd ? cwd.getAttribute('style') : null;
      var leftShift = null;
      if (cwd) {
        r18ResetScroll();
        cwd.style.position = 'relative';
        cwd.style.left = '-2000px';
        leftShift = r18Measure(cwd, '[data-region="runtimes"]');
        if (cwdPrev === null) cwd.removeAttribute('style'); else cwd.setAttribute('style', cwdPrev);
      }
      // (e) 同上，但往右位移 2000px：仍在 .pane-row 裡、右緣越過 runtimes，卻超出該列欄位排版，
      // 不是整列超寬特徵，必須硬失敗、不得 PENDING。
      var rightShift = null;
      if (cwd) {
        r18ResetScroll();
        cwd.style.position = 'relative';
        cwd.style.left = '2000px';
        rightShift = r18Measure(cwd, '[data-region="runtimes"]');
        if (cwdPrev === null) cwd.removeAttribute('style'); else cwd.setAttribute('style', cwdPrev);
      }
      r18ResetScroll();
      return { swBefore: swBefore, swAfter: swAfter, oldViaScrollWouldPass: oldViaScrollWouldPass, shifted: shifted, nowrap: nowrap, ellipsis: ellipsis, leftShift: leftShift, rightShift: rightShift };
    })()`);
    check(
      r18Controls !== null && r18Controls.swAfter > r18Controls.swBefore && r18Controls.oldViaScrollWouldPass === true,
      `R18 否定對照 (a) 前提：position: relative 位移真的撐大了 .projects 的 scrollWidth，且落在 round 3 viaScroll 例外會放行的範圍內（實際 ${JSON.stringify(r18Controls && { swBefore: r18Controls.swBefore, swAfter: r18Controls.swAfter, oldViaScrollWouldPass: r18Controls.oldViaScrollWouldPass })}）`
    );
    check(
      r18Controls !== null && r18Controls.shifted.exists === true && r18Controls.shifted.contained === false && !isKnownPaneRowOverflow(r18Controls.shifted),
      `R18 否定對照 (a)：workstream 名稱被 relative 位移到 region 右緣外時必須判定溢出、且不得落入 PENDING 特徵（實際 ${JSON.stringify(r18Controls && r18Controls.shifted)}）`
    );
    check(
      r18Controls !== null && r18Controls.nowrap.box.right <= r18Controls.nowrap.region.right + 0.5 && r18Controls.nowrap.contained === false,
      `R18 否定對照 (b)：200 字元不換行、不省略的名稱（元素框仍在 region 內、文字畫出框外）必須判定溢出（實際 ${JSON.stringify(r18Controls && r18Controls.nowrap)}）`
    );
    check(
      r18Controls !== null && r18Controls.ellipsis.contained === true,
      `R18 正面對照 (c)：單行省略（nowrap＋overflow hidden＋ellipsis）是 spec 允許的呈現，必須判定在區域內（實際 ${JSON.stringify(r18Controls && r18Controls.ellipsis)}）`
    );
    check(
      isKnownPaneRowOverflow(null) === false &&
        isKnownPaneRowOverflow({ exists: false, contained: false }) === false &&
        isKnownPaneRowOverflow({ exists: true, regionMissing: true, contained: false }) === false &&
        isKnownPaneRowOverflow({ exists: true, hidden: true, contained: false }) === false &&
        isKnownPaneRowOverflow({ exists: true, zeroSize: true, contained: false }) === false,
      'R18 PENDING 收窄：缺元素、缺 region、隱藏、零尺寸一律不得落入 3.3 已知特徵（必須硬失敗）'
    );
    check(
      r18Controls !== null && r18Controls.leftShift !== null && r18Controls.leftShift.contained === false && isKnownPaneRowOverflow(r18Controls.leftShift) === false,
      `R18 PENDING 收窄：cwd 往左位移出 runtimes（非整列超寬特徵）必須硬失敗、不得 PENDING（實際 ${JSON.stringify(r18Controls && r18Controls.leftShift)}）`
    );
    check(
      r18Controls !== null && r18Controls.rightShift !== null && r18Controls.rightShift.contained === false && isKnownPaneRowOverflow(r18Controls.rightShift) === false,
      `R18 PENDING 收窄：cwd 被 relative 往右位移 2000px（越過所在列的欄位排版）必須硬失敗、不得 PENDING（實際 ${JSON.stringify(r18Controls && r18Controls.rightShift)}）`
    );

    // --- 網格過寬時不撐破頁面（spec cockpit-dashboard「Factory Floor」情境「網格過寬時不撐破
    // 頁面」）：10 個 stages，Factory Floor 區域內可橫向捲動看到最後一欄，頁面本身不出現橫向
    // 捲軸（已經在上面驗過 scrollWidth），這裡另外驗「區域內可橫向捲動看到最後一欄」。
    // fix round 2／I2（設計複審 r1）：捲動目標從 `.factory-floor` 改成
    // `[data-region="floor"] > .projects`——`.factory-floor` 的 `overflow-x: auto` 已經被
    // fix round 2 蓋掉（改成 `overflow-x: visible`），因為它會讓 `.ff-stage-header` 往上找到
    // 的第一個捲動祖先變成它自己（縱向永遠不會捲動），攔在真正雙向捲動的 `.projects` 之前；
    // Factory Floor 的橫向捲動現在跟縱向捲動一樣，統一交給 `.projects` 處理。 */
    const gridScroll = await cdp.eval(`(() => {
      var grid = document.querySelector('.factory-floor');
      var scrollContainer = document.querySelector('[data-region="floor"] > .projects');
      if (!grid || !scrollContainer) return null;
      var before = scrollContainer.scrollLeft;
      scrollContainer.scrollLeft = scrollContainer.scrollWidth;
      var after = scrollContainer.scrollLeft;
      var headers = document.querySelectorAll('.ff-stage-header');
      return {
        before,
        after,
        canScroll: scrollContainer.scrollWidth > scrollContainer.clientWidth,
        stageHeaderCount: headers.length,
      };
    })()`);
    check(
      gridScroll !== null && gridScroll.canScroll === true && gridScroll.after > gridScroll.before,
      `Factory Floor 區域內應該可以橫向捲動看到最後一欄（實際 ${JSON.stringify(gridScroll)}）`
    );
    check(
      gridScroll !== null && gridScroll.stageHeaderCount === 10,
      `10 個 stages 應該畫出 10 個欄標頭（實際 ${gridScroll ? gridScroll.stageHeaderCount : 'n/a'}）`
    );

    // fix round 3／N3（設計複審 r1＋控制端指示 4）：網格已捲到最後一欄（gridScroll 那段剛把
    // scrollLeft 設成 scrollWidth），量最右邊的 stage 欄首是否落在 `.project` 面板自己的
    // rect 內——round 2 讓 `.factory-floor` 變成 overflow-x: visible 後，橫向溢出改由
    // `.projects` 承接，但 `.project` 面板本身沒有跟著網格變寬，格子會畫到面板外、面板的
    // 標題與列首整段被捲出畫面（見 task-2.1-design-review-r2.md N3）。修法（fix round 3 第二版）是
    // render.js 以算術給面板 min-width（網格各軌道下限總和＋內距），所以捲到底後最右欄首
    // 應該還是落在面板的 rect 內。
    const panelContainment = await cdp.eval(`(() => {
      var headers = document.querySelectorAll('.ff-stage-header');
      var lastHeader = headers[headers.length - 1];
      var panel = document.querySelector('[data-region="floor"] > .projects > .project');
      if (!lastHeader || !panel) return null;
      var h = lastHeader.getBoundingClientRect();
      var p = panel.getBoundingClientRect();
      return {
        header: { left: h.left, right: h.right, top: h.top, bottom: h.bottom },
        panel: { left: p.left, right: p.right, top: p.top, bottom: p.bottom },
        contained: h.left >= p.left - 1 && h.right <= p.right + 1,
      };
    })()`);
    check(
      panelContainment !== null && panelContainment.contained === true,
      `N3：捲到最後一欄後，最右邊的 stage 欄首（.ff-stage-header）應該落在 .project 面板自己的 rect 內，不應該畫到面板外（實際 ${JSON.stringify(panelContainment)}）`
    );

    // fix round 4／N3 回歸（Codex r3：render.js 的算術 min-width 把 minmax(160px, auto) 的首欄
    // 當 160，長 binding ID 會撐寬首欄）：注入長 runtime／pane ID（override 綁定，含「改綁」
    // 徽章），10 與 3 個 stage，各在 1536／700 寬量：
    //   - 網格（.factory-floor）不溢出自己（scrollWidth ≤ clientWidth），捲到最右後最後一欄欄首
    //     與網格右緣都在 .project 面板內（面板寬度的算術下限仍然成立）；
    //   - 首欄寬度 ≤ 200px（首欄軌道有上限，長 ID 不再吃掉 stage 欄的空間；task 3.2 fix round 1
    //     設計審核 M1 把上限從 240 降到 200，斷言跟著收緊）；
    //   - binding 文字（R18 同一個水平範圍量法）不超出所在列首（.ff-row-header），也不畫進
    //     第一個 stage 欄；外層 .ff-binding 的 title 帶完整的「runtime / pane」全文（title 不放在
    //     .ff-binding-text 本身，factory-floor-check.js 逐字比對它的 DOM 形狀）。
    const LONG_RUNTIME_ID = 'runtime-' + 'R'.repeat(40);
    const LONG_PANE_ID = 'pane-' + 'P'.repeat(60);
    const n3Results = [];
    for (const stageCount of [10, 3]) {
      const n3State = craftWideState();
      n3State.version = 50 + stageCount;
      const n3Project = n3State.projects[0];
      n3Project.stages = n3Project.stages.slice(0, stageCount);
      n3Project.tasks[0].stage = n3Project.stages[stageCount - 1];
      n3Project.workstreams[0].name = 'short ws';
      n3Project.workstreams[0].binding = {
        state: 'bound',
        runtime: LONG_RUNTIME_ID,
        pane_id: LONG_PANE_ID,
        source: 'override',
        agent: 'claude',
        agent_status: 'working',
      };
      const n3Inject = await injectState(cdp, n3State);
      check(n3Inject && n3Inject.ok === true, `N3 回歸：注入長 runtime／pane ID、${stageCount} 個 stage 的特製投影`);
      for (const [vw, vh] of [[1536, 1024], [700, 900]]) {
        await cdp.send('Emulation.setDeviceMetricsOverride', { width: vw, height: vh, deviceScaleFactor: 1, mobile: false });
        await sleep(150);
        const m = await cdp.eval(`(() => {
          ${R18_MEASURE_JS}
          r18ResetScroll();
          var grid = document.querySelector('.factory-floor');
          var panel = document.querySelector('[data-region="floor"] > .projects > .project');
          var sc = document.querySelector('[data-region="floor"] > .projects');
          var header = document.querySelector('.ff-row-header');
          var text = document.querySelector('.ff-binding-text');
          var stages = document.querySelectorAll('.ff-stage-header');
          if (!grid || !panel || !sc || !header || !text || stages.length === 0) return null;
          var textExt = r18Measure(text, '[data-region="floor"]');
          var hdr = header.getBoundingClientRect();
          var firstStage = stages[0].getBoundingClientRect();
          var col1 = parseFloat(getComputedStyle(grid).gridTemplateColumns.split(' ')[0]);
          sc.scrollLeft = sc.scrollWidth;
          var g = grid.getBoundingClientRect();
          var p = panel.getBoundingClientRect();
          var last = stages[stages.length - 1].getBoundingClientRect();
          var out = {
            textExt: textExt.ext,
            textTitle: text.parentElement && text.parentElement.classList.contains('ff-binding') ? text.parentElement.title : null,
            headerRight: Math.round(hdr.right * 100) / 100,
            firstStageLeft: Math.round(firstStage.left * 100) / 100,
            col1: col1,
            gridSW: grid.scrollWidth,
            gridCW: grid.clientWidth,
            gridRight: Math.round(g.right * 100) / 100,
            lastStageRight: Math.round(last.right * 100) / 100,
            panelRight: Math.round(p.right * 100) / 100,
          };
          r18ResetScroll();
          return out;
        })()`);
        n3Results.push({ stageCount, vw, m });
        const tag = `N3 回歸（長 binding ID、${stageCount} stage、${vw}x${vh}）`;
        check(m !== null, `${tag}：找得到網格、面板、列首、binding 文字與欄首`);
        if (m === null) continue;
        check(
          m.gridSW <= m.gridCW + 1 && m.gridRight <= m.panelRight + 1 && m.lastStageRight <= m.panelRight + 1,
          `${tag}：網格不溢出、捲到最右後網格右緣與最後一欄欄首都在 .project 面板內（實際 ${JSON.stringify(m)}）`
        );
        check(m.col1 <= 200.5, `${tag}：首欄寬度 ≤ 200px（實際 ${m.col1}）`);
        check(
          m.textExt && m.textExt.right <= m.headerRight + 0.5 && m.textExt.right <= m.firstStageLeft + 0.5,
          `${tag}：binding 文字的水平範圍不超出列首、不畫進第一個 stage 欄（實際 ${JSON.stringify(m)}）`
        );
        check(
          m.textTitle === LONG_RUNTIME_ID + ' / ' + LONG_PANE_ID,
          `${tag}：binding 文字外層 .ff-binding 的 title 帶完整全文（實際 ${JSON.stringify(m.textTitle)}）`
        );
      }
    }
    // task 3.3 fix round 1（Ruling R34）：整頁捲動的版面（寬 <1200，或寬 ≥1200 但高 <720）裡
    // 最近事件不設高度上限、參與整頁捲動（design D3「取消固定高度、整頁捲動」）——30 筆事件時
    // events 的內層清單自己不縱向捲動（scrollHeight ≤ clientHeight）。
    const evState = craftWideState();
    evState.version = 77;
    const evInj = await injectState(cdp, evState);
    check(evInj && evInj.ok === true, 'R34：注入 30 筆事件的特製投影');
    const evScroll = {};
    for (const [vw, vh] of [[1100, 900], [700, 900]]) {
      await cdp.send('Emulation.setDeviceMetricsOverride', { width: vw, height: vh, deviceScaleFactor: 1, mobile: false });
      await sleep(150);
      evScroll[vw] = await cdp.eval(`(() => {
        var l = document.querySelector('[data-region="events"] > .recent-events-list');
        var out = { rows: l ? l.querySelectorAll('.event-row').length : 0, sh: l ? l.scrollHeight : null, ch: l ? l.clientHeight : null };
        var st = document.createElement('style');
        st.textContent = '[data-region="events"] { max-height: 200px !important; }';
        document.head.appendChild(st);
        out.negSh = l ? l.scrollHeight : null;
        out.negCh = l ? l.clientHeight : null;
        st.remove();
        return out;
      })()`);
      const m = evScroll[vw];
      check(
        m.rows === 30 && m.sh !== null && m.sh <= m.ch + 0.5,
        `R34（${vw}x${vh}）：最近事件參與整頁捲動，內層清單沒有自己的縱向捲動（scrollHeight ${m.sh} ≤ clientHeight ${m.ch}，${m.rows} 筆）`
      );
      check(
        m.negSh !== null && m.negSh > m.negCh + 0.5,
        `否定對照 R34（${vw}x${vh}）：區塊加回 200px 高度上限時，內層清單 scrollHeight ${m.negSh} > clientHeight ${m.negCh}`
      );
    }
    await cdp.send('Emulation.clearDeviceMetricsOverride', {});
    await sleep(150);
    log(`N3 長 binding ID 回歸量測：${JSON.stringify(n3Results)}`);

    // I2 結構斷言（.ff-stage-header 的捲動祖先）搬到下面用預設 fixture 的 preview 量
    // （fix round 3：craftWideState 的「Wide Project」只有 1 個 workstream，N3 讓
    // `.project` 改成 width: max-content 之後，task 節點的按鈕不再被窄欄位逼著換行，
    // 單一 workstream 的內容量縮到跟 `.projects` 的 clientHeight 一樣高，縱向不再溢出；
    // 預設 fixture 兩個 Project、九個 workstream 疊在一起，內容量穩定遠超過一屏，不會受
    // N3 影響）。
  } finally {
    await stopChrome(chrome, 'chrome-V1');
    await stopPreview(preview, 'preview-V1');
  }

  // ---------------------------------------------------------------------------
  // C1／I1（task 2.1 fix round 1，Codex＋design 審核）：banner 出現時只占中欄、只從 Factory
  // Floor 的份額扣，不動 Live Output 的份額。用預設 fixture（不是 craftWideState）真的觸發
  // rebind 與 error 兩則提示（改綁模式 GET 不到就用真的按鈕點；error 用
  // COCKPIT_PREVIEW_WRITE_RULES 讓一次寫入立刻回 409），量三個狀態（無提示／一則／兩則）下
  // Factory Floor、Live Output、左欄、右欄的高度，用獨立的 preview／chrome（跟上面
  // craftWideState 的那組分開，避免互相干擾）。
  // ---------------------------------------------------------------------------
  let bannerPreview = null;
  let bannerChrome = null;
  try {
    bannerPreview = await startPreview(
      { COCKPIT_PREVIEW_WRITE_RULES: '/api/projects/cockpit/tasks/be-1/fail=0:409' },
      'preview-V1-banner'
    );
    const bannerUrl = `http://127.0.0.1:${bannerPreview.port}/`;
    bannerChrome = await startChrome(pickPort(19031, [bannerPreview.port]), bannerUrl, 'chrome-V1-banner', '1536,1024');
    const { cdp: bcdp } = bannerChrome;
    await waitForFirstProjection(bcdp, bannerPreview.port);
    // 先選定一個 pane，量「有選取」時的高度（task 4.1 起面板常駐，沒選取也量得到空狀態；空狀態
    // 與有選取同高由 R17 矩陣與 LO1 [LO1/fill] 另外驗）。
    await bcdp.eval("window.liveOutput.select('win', 'wJ:p1'); true");
    await sleep(200);

    async function measureBanner(label) {
      const rects = await getRects(bcdp, {
        floor: '[data-region="floor"]',
        output: '[data-region="output"]',
        projects: '[data-region="projects"]',
        runtimes: '[data-region="runtimes"]',
        banner: '[data-region="banner"]',
      });
      log(`banner 幾何（${label}）：${JSON.stringify(rects)}`);
      return rects;
    }

    const before = await measureBanner('無提示');
    check(before.banner === null, `無提示時 data-region="banner" 不應該存在（實際 ${JSON.stringify(before.banner)}）`);

    // --- I2 結構斷言（fix round 2／設計複審 r1；「不用實作 sticky 本身」，只驗基準對不對；
    // fix round 3：改用這裡的預設 fixture，理由見上面搬移時留的註解）：欄首
    // （.ff-stage-header）往上找到的第一個捲動祖先必須就是 Factory Floor 的內層容器
    // （[data-region="floor"] > .projects），3.2 之後在欄首上做 position: sticky; top: 0
    // 才會真的黏對容器；這個容器在內容夠多時要 scrollHeight > clientHeight，證明它真的是
    // 縱向捲動層，不是名義上的。 ---
    const scrollAncestor = await bcdp.eval(`(() => {
      var header = document.querySelector('.ff-stage-header');
      var floorInner = document.querySelector('[data-region="floor"] > .projects');
      if (!header || !floorInner) return null;
      var node = header.parentElement;
      var found = null;
      while (node) {
        var cs = getComputedStyle(node);
        if (cs.overflowX !== 'visible' || cs.overflowY !== 'visible') {
          found = node;
          break;
        }
        node = node.parentElement;
      }
      return {
        isFloorInner: found === floorInner,
        foundDesc: found ? found.tagName + (found.className ? '.' + String(found.className).trim().split(/\\s+/).join('.') : '') : null,
        scrollHeight: floorInner.scrollHeight,
        clientHeight: floorInner.clientHeight,
      };
    })()`);
    check(
      scrollAncestor !== null && scrollAncestor.isFloorInner === true,
      `.ff-stage-header 往上第一個捲動祖先應該就是 Factory Floor 的內層容器（[data-region="floor"] > .projects），3.2 的 sticky 欄首才有正確基準（實際找到 ${scrollAncestor && scrollAncestor.foundDesc}）`
    );
    check(
      scrollAncestor !== null && scrollAncestor.scrollHeight > scrollAncestor.clientHeight,
      `Factory Floor 的內層容器內容夠多時應該真的縱向溢出（scrollHeight > clientHeight，證明它是真正的捲動層，不是名義上的；實際 scrollHeight=${scrollAncestor && scrollAncestor.scrollHeight}，clientHeight=${scrollAncestor && scrollAncestor.clientHeight}）`
    );

    // --- fix round 4：內層捲動位置跨重畫保留（2.1 的迴歸守門）。2.1 把 Factory Floor／
    // runtime 清單／最近事件改成內層捲動容器（I2），它們在 #app 底下、每次整頁重畫都換成新節點；
    // 沒有保留的話，使用者捲到一半，下一份推送就把畫面拉回頂端（actions-check.js「頻繁重畫時
    // 按鈕仍有效」因此間歇漏送 POST）。捲動後呼叫 window.repaint()（同 actions.js 的同步重畫
    // 路徑，DOM 真的被換掉）再量：節點是新的、捲動位置不變。 ---
    const scrollKeep = await bcdp.eval(`(() => {
      var sel = '[data-region="floor"] > .projects';
      var before = document.querySelector(sel);
      if (!before || before.scrollHeight <= before.clientHeight) return null;
      before.scrollTop = 150;
      var want = before.scrollTop;
      window.repaint();
      var after = document.querySelector(sel);
      return { replaced: after !== before, want: want, got: after ? after.scrollTop : null };
    })()`);
    check(
      scrollKeep !== null && scrollKeep.replaced === true && scrollKeep.want > 0 && Math.abs(scrollKeep.got - scrollKeep.want) < 1,
      `Factory Floor 內層容器的捲動位置在整頁重畫（節點被換掉）後應該保留（實際 ${JSON.stringify(scrollKeep)}）`
    );
    await bcdp.eval(`(() => { var n = document.querySelector('[data-region="floor"] > .projects'); if (n) n.scrollTop = 0; return true; })()`);

    // --- 進入改綁模式：只有 rebind 提示 ---
    await bcdp.click('[data-action="rebind"][data-project="cockpit"][data-workstream="be"]');
    await bcdp.waitFor('!!document.querySelector(\'[data-region="banner"]\')', 2000, '進入改綁模式，banner 出現');
    const withRebind = await measureBanner('僅 rebind');
    check(
      !!withRebind.banner && !!before.floor && Math.abs(withRebind.banner.left - before.floor.left) <= 1 && Math.abs(withRebind.banner.right - before.floor.right) <= 1,
      `banner 應該只占中欄，左右緣要跟 Factory Floor 對齊（I1；banner=${JSON.stringify(withRebind.banner)}，floor=${JSON.stringify(before.floor)}）`
    );
    check(
      !!withRebind.output && !!before.output && Math.abs(withRebind.output.height - before.output.height) <= 2,
      `banner 出現（僅 rebind）時 Live Output 的高度不應該改變（I1；無提示=${before.output && before.output.height}，有 rebind=${withRebind.output && withRebind.output.height}）`
    );
    check(
      !!withRebind.floor && !!before.floor && withRebind.floor.height < before.floor.height,
      `banner 出現（僅 rebind）時 Factory Floor 的高度應該變矮（份額被 banner 吃掉；無提示=${before.floor && before.floor.height}，有 rebind=${withRebind.floor && withRebind.floor.height}）`
    );
    check(
      !!withRebind.floor && withRebind.floor.height >= 240 - 0.5,
      `僅一則提示時 Factory Floor 仍應該 ≥ 240px（實際 ${withRebind.floor && withRebind.floor.height}）`
    );

    // --- 觸發 error（COCKPIT_PREVIEW_WRITE_RULES 讓這次寫入立刻回 409）：rebind＋error 兩則 ---
    await bcdp.click('[data-action="fail"][data-project="cockpit"][data-task="be-1"]');
    await bcdp.waitFor(
      "document.querySelectorAll('[data-region=\"banner\"] .action-banner').length === 2",
      2000,
      '兩則提示（rebind＋error）都出現'
    );
    const withBoth = await measureBanner('rebind＋error 兩則');
    check(
      !!withBoth.banner && !!before.floor && Math.abs(withBoth.banner.left - before.floor.left) <= 1 && Math.abs(withBoth.banner.right - before.floor.right) <= 1,
      `兩則提示同時出現時 banner 仍然應該只占中欄（實際 banner=${JSON.stringify(withBoth.banner)}，floor=${JSON.stringify(before.floor)}）`
    );
    check(
      !!withBoth.output && !!before.output && Math.abs(withBoth.output.height - before.output.height) <= 2,
      `兩則提示同時出現時 Live Output 的高度仍不應該改變（I1；無提示=${before.output && before.output.height}，兩則=${withBoth.output && withBoth.output.height}）`
    );
    check(
      !!withBoth.floor && !!withRebind.floor && withBoth.floor.height <= withRebind.floor.height,
      `兩則提示應該比一則提示佔用更多 banner 高度、讓 Factory Floor 更矮或至少不變高（一則=${withRebind.floor && withRebind.floor.height}，兩則=${withBoth.floor && withBoth.floor.height}）`
    );
    // 1536×1024 下可用高度充裕，兩則提示（約 84px）預期仍不會讓 Factory Floor 跌破
    // 240px——這裡照樣斷言，跌破的邊界情境（1280 寬、720 高）另外用專屬視窗量、且明確允許
    // 「做不到就寫明原因」（Ruling R15）。
    check(
      !!withBoth.floor && withBoth.floor.height >= 240 - 0.5,
      `1536×1024 下兩則提示同時出現時 Factory Floor 仍應該 ≥ 240px（可用高度充裕；實際 ${withBoth.floor && withBoth.floor.height}）`
    );
  } finally {
    await stopChrome(bannerChrome, 'chrome-V1-banner');
    await stopPreview(bannerPreview, 'preview-V1-banner');
  }

  // ---------------------------------------------------------------------------
  // N1／R17（task 2.1 fix round 3，控制端裁決 Ruling R17，見 progress.md）：固定一屏（寬
  // ≥1200 且高 ≥720）高度分配的優先序——banner 永遠完整顯示、不設 max-height、不得裁切；
  // Live Output ≥240px 且高度不隨 banner 出現與否改變（style.css 把 r2／r3 兩列改成沒有
  // minmax 的固定 calc()，Factory Floor 用裸 1fr 當唯一的彈性／犧牲軌道）；三者無法同時滿足
  // 時由 Factory Floor 讓出、降到 240px 以下並在內部捲動。這裡取代原本只測單一邊界
  // （1280×720）的 I1 邊界區塊，跑指定的完整測試矩陣：寬 {1200,1280,1536} × 高
  // {720,761,1024}（固定一屏 9 格）外加 700×900（單欄），每格量 0／1／2 則提示（2 則時把
  // 錯誤文字覆寫成夠長、會折行的中文字串，逼出 banner 兩行內容，不只依賴預設 409 錯誤文字
  // 在某些寬度下才會折行）。用同一組 preview／chrome，靠
  // Emulation.setDeviceMetricsOverride 換尺寸（不重啟瀏覽器，也不用 --window-size 與
  // innerHeight 之間約 99px 的落差去換算——這個 CDP 方法直接把 viewport 設成指定值，
  // innerWidth/innerHeight 就是傳入的 width/height，量到的每一格都精準）。
  // `LONG_ERROR_TEXT` 是模組層級常數（見檔案開頭），CT1 的「banner-wrap」子段共用同一份。
  // ---------------------------------------------------------------------------
  const MATRIX_SIZES = [
    [1200, 720],
    [1200, 761],
    [1200, 1024],
    [1280, 720],
    [1280, 761],
    [1280, 1024],
    [1536, 720],
    [1536, 761],
    [1536, 1024],
    [700, 900],
  ];
  let matrixPreview = null;
  let matrixChrome = null;
  const matrixResults = [];
  try {
    matrixPreview = await startPreview(
      { COCKPIT_PREVIEW_WRITE_RULES: '/api/projects/cockpit/tasks/be-1/fail=0:409' },
      'preview-V1-matrix'
    );
    const matrixUrl = `http://127.0.0.1:${matrixPreview.port}/`;
    matrixChrome = await startChrome(pickPort(19032, [matrixPreview.port]), matrixUrl, 'chrome-V1-matrix', '1536,1024');
    const { cdp: mcdp } = matrixChrome;
    await waitForFirstProjection(mcdp, matrixPreview.port, '第一份投影已畫出（matrix；R22 動態條件）');
    // 選定一個 pane，量「有選取」時的高度（R17 原本就是量這個狀態）；direction-01-visual task 4.1
    // 之後面板常駐，另外在每格 0／2 則提示時用 measureOutputEmpty() 切回空狀態量一次，斷言
    // 空狀態與有選取時面板高度相同（2.1 設計 M6：輸出窗撐滿區塊、不是內容多高就多高）。
    await mcdp.eval("window.liveOutput.select('win', 'wJ:p1'); true");
    await sleep(200);

    async function measureOutputEmpty() {
      await mcdp.eval('window.liveOutput.clear(); true');
      await sleep(50);
      const r = await mcdp.eval(`(() => {
        var n = document.querySelector('[data-region="output"]');
        var e = document.querySelector('#output .output-empty');
        return {
          isOpen: n ? n.classList.contains('is-open') : null,
          height: n ? n.getBoundingClientRect().height : null,
          emptyShown: !!e && e.getClientRects().length > 0,
        };
      })()`);
      await mcdp.eval("window.liveOutput.select('win', 'wJ:p1'); true");
      await sleep(50);
      return r;
    }

    async function setViewport(width, height) {
      await mcdp.send('Emulation.setDeviceMetricsOverride', {
        width,
        height,
        deviceScaleFactor: 1,
        mobile: false,
      });
      await sleep(150);
      const actual = await mcdp.eval('({ w: window.innerWidth, h: window.innerHeight })');
      check(
        actual.w === width && actual.h === height,
        `Emulation.setDeviceMetricsOverride 應該把 innerWidth/innerHeight 精準設成 ${width}x${height}（實際 ${JSON.stringify(actual)}）`
      );
    }

    async function measureCell(width, height, noticeCount) {
      const rects = await getRects(mcdp, {
        floor: '[data-region="floor"]',
        output: '[data-region="output"]',
        // file-review task 4.1：spec cockpit-dashboard「版面與窄視窗」把中欄下半部從 Live Output 改成
        // 分頁區（Live Output 是其中第一個分頁），R17 的「≥240px」改量分頁區。
        review: '[data-region="review"]',
        banner: '[data-region="banner"]',
        statusbar: '[data-region="statusbar"]',
        topbar: '[data-region="topbar"]',
      });
      const bannerOverflow = await mcdp.eval(
        '(() => { var b = document.querySelector(\'[data-region="banner"]\'); return b ? { scrollHeight: b.scrollHeight, clientHeight: b.clientHeight } : null; })()'
      );
      // fix round 4／N4：外框本身是否溢出、底列是否還在視窗內、Factory Floor 內層捲動容器的
      // 內容量（Floor 讓出到 240 以下時，內容要能在內層捲到）。
      const shell = await mcdp.eval(`(() => {
        var s = document.querySelector('.shell');
        var inner = document.querySelector('[data-region="floor"] > .projects');
        return {
          scrollHeight: s ? s.scrollHeight : null,
          clientHeight: s ? s.clientHeight : null,
          innerHeight: window.innerHeight,
          floorInnerScrollHeight: inner ? inner.scrollHeight : null,
          floorInnerClientHeight: inner ? inner.clientHeight : null,
        };
      })()`);
      return { width, height, noticeCount, rects, bannerOverflow, shell };
    }

    for (const [width, height] of MATRIX_SIZES) {
      await setViewport(width, height);
      const isFixedLayout = width >= 1200 && height >= 720;

      // 每格開始前重置成 0 則提示：rebind-cancel／error-dismiss 都是「一般畫面操作」，會先
      // 清掉 ui.error（同 spec cockpit-dashboard「畫面操作」與既有 R1 段落的觀察），這裡分別
      // 判斷元素是否存在再點，不對不存在的元素點擊（否則會誤印一條無意義的 FAIL）。
      const resetState = await mcdp.eval(
        "({ hasRebind: !!document.querySelector('.rebind-banner'), hasError: !!document.querySelector('.error-banner') })"
      );
      if (resetState.hasError) await mcdp.click('[data-action="error-dismiss"]');
      if (resetState.hasRebind) await mcdp.click('[data-action="rebind-cancel"]');
      await mcdp.waitFor(
        "document.querySelectorAll('[data-region=\"banner\"] .action-banner').length === 0",
        2000,
        `${width}x${height}：重置回 0 則提示`
      );

      const m0 = await measureCell(width, height, 0);
      m0.outputEmpty = await measureOutputEmpty();
      matrixResults.push(m0);
      check(
        m0.rects.banner === null,
        `${width}x${height}、0 則提示：不應該有 data-region="banner"（實際 ${JSON.stringify(m0.rects.banner)}）`
      );

      await mcdp.click('[data-action="rebind"][data-project="cockpit"][data-workstream="be"]');
      await mcdp.waitFor(
        "document.querySelectorAll('[data-region=\"banner\"] .action-banner').length === 1",
        2000,
        `${width}x${height}：只有 rebind 一則提示`
      );
      const m1 = await measureCell(width, height, 1);
      matrixResults.push(m1);
      check(
        m1.bannerOverflow !== null && m1.bannerOverflow.scrollHeight <= m1.bannerOverflow.clientHeight + 0.5,
        `${width}x${height}、1 則提示：banner 不得被裁切（scrollHeight ≤ clientHeight；實際 ${JSON.stringify(m1.bannerOverflow)}）`
      );

      await mcdp.click('[data-action="fail"][data-project="cockpit"][data-task="be-1"]');
      await mcdp.waitFor(
        "document.querySelectorAll('[data-region=\"banner\"] .action-banner').length === 2",
        2000,
        `${width}x${height}：兩則提示都出現`
      );
      // 把錯誤文字覆寫成夠長、會折行的字串——不是每個尺寸單靠固定的 409 錯誤文字都會折行，
      // 直接覆寫確保每一格都測到「長文字折行」這個 R17 明訂要覆蓋的情境。
      await mcdp.eval(`(() => {
        var t = document.querySelector('.error-banner .action-banner-text');
        if (t) t.textContent = ${JSON.stringify(LONG_ERROR_TEXT)};
        return !!t;
      })()`);
      await sleep(50);
      const m2 = await measureCell(width, height, 2);
      m2.outputEmpty = await measureOutputEmpty();
      matrixResults.push(m2);
      check(
        m2.bannerOverflow !== null && m2.bannerOverflow.scrollHeight <= m2.bannerOverflow.clientHeight + 0.5,
        `${width}x${height}、2 則提示（含折行長文字）：banner 不得被裁切（scrollHeight ≤ clientHeight；實際 ${JSON.stringify(m2.bannerOverflow)}）`
      );

      if (isFixedLayout) {
        const heights = [m0, m1, m2].map((m) => m.rects.output && m.rects.output.height);
        const allPresent = heights.every((h) => typeof h === 'number');
        const allEqual = allPresent && heights.every((h) => Math.abs(h - heights[0]) < 0.5);
        const reviewHeights = [m0, m1, m2].map((m) => m.rects.review && m.rects.review.height);
        check(
          reviewHeights.every((h) => typeof h === 'number' && h >= 239.5),
          `${width}x${height}（固定一屏）：下半部分頁區高度應該 ≥240px（實際 0/1/2 則提示 ${JSON.stringify(reviewHeights)}）`
        );
        check(
          allEqual,
          `${width}x${height}（固定一屏）：Live Output 高度不應該隨 banner 出現與否改變（0/1/2 則提示實際 ${JSON.stringify(heights)}）`
        );
        // direction-01-visual task 4.1（2.1 設計 M6）：空狀態時面板一樣佔滿該區——高度與有選取時
        // 相同（0／2 則提示各量一次）。
        for (const m of [m0, m2]) {
          const e = m.outputEmpty;
          check(
            !!e && e.emptyShown === true && e.isOpen === false && !!m.rects.output &&
              typeof e.height === 'number' && Math.abs(e.height - m.rects.output.height) < 0.5,
            `${width}x${height}、${m.noticeCount} 則提示（固定一屏）：空狀態時 Live Output 高度應該與有選取時相同（有選取 ${m.rects.output && m.rects.output.height}，空狀態 ${JSON.stringify(e)}）`
          );
        }

        // fix round 4／N4（設計複審 r3）：頂列與底列維持自然高度，缺口只能由 Factory Floor
        // 吸收——頂列高度在 0／1／2 則提示下不變、底列下緣在視窗內、.shell 本身不溢出、
        // Live Output 下緣在視窗內。
        const topbarHeights = [m0, m1, m2].map((m) => m.rects.topbar && m.rects.topbar.height);
        check(
          topbarHeights.every((h) => typeof h === 'number' && Math.abs(h - topbarHeights[0]) < 0.5),
          `${width}x${height}（固定一屏）：頂列高度不應該隨 banner 出現與否改變（0/1/2 則提示實際 ${JSON.stringify(topbarHeights)}）`
        );
        for (const m of [m0, m1, m2]) {
          const sb = m.rects.statusbar;
          check(
            !!sb && sb.bottom <= m.shell.innerHeight + 0.5,
            `${width}x${height}、${m.noticeCount} 則提示（固定一屏）：底列下緣應該在視窗內（statusbar.bottom=${sb && sb.bottom}，innerHeight=${m.shell.innerHeight}）`
          );
          check(
            m.shell.scrollHeight !== null && m.shell.scrollHeight <= m.shell.clientHeight,
            `${width}x${height}、${m.noticeCount} 則提示（固定一屏）：.shell 不應該溢出（scrollHeight=${m.shell.scrollHeight}，clientHeight=${m.shell.clientHeight}）`
          );
          check(
            !!m.rects.output && m.rects.output.bottom <= m.shell.innerHeight + 0.5,
            `${width}x${height}、${m.noticeCount} 則提示（固定一屏）：Live Output 下緣應該在視窗內（output.bottom=${m.rects.output && m.rects.output.bottom}）`
          );
        }
      }

      for (const m of [m0, m1, m2]) {
        const floorH = m.rects.floor && m.rects.floor.height;
        if (isFixedLayout && typeof floorH === 'number' && floorH < 239.5) {
          log(
            `R17：${width}x${height}、${m.noticeCount} 則提示時 Factory Floor 降到 240px 以下（實際 ${floorH.toFixed(2)}px）——依裁決，Factory Floor 是三者無法同時滿足時唯一允許讓出的一方，這裡不算 FAIL，report 會列出這個實測值`
          );
          // 讓出時內容改在 Factory Floor 的內層捲動容器裡捲（R17「在內部捲動」）。
          check(
            m.shell.floorInnerClientHeight > 0 && m.shell.floorInnerScrollHeight > m.shell.floorInnerClientHeight,
            `${width}x${height}、${m.noticeCount} 則提示：Factory Floor 讓出到 240 以下時，內容應該在內層容器捲動（scrollHeight=${m.shell.floorInnerScrollHeight}，clientHeight=${m.shell.floorInnerClientHeight}）`
          );
        }
      }
    }
  } finally {
    await stopChrome(matrixChrome, 'chrome-V1-matrix');
    await stopPreview(matrixPreview, 'preview-V1-matrix');
  }
  log(`N1／R17 測試矩陣完整結果（供 report 引用）：${JSON.stringify(matrixResults)}`);

  // ---------------------------------------------------------------------------
  // N1（設計審核，task 3.1 fix round 2；控制端 Ruling R30）：Project 多時，sticky 左欄在
  // 頁面頂端（scrollY=0）不得蓋住 sticky 底列的通道狀態——底列 z-index 固定比左欄高（防線
  // 一），左欄 max-height 依頁面頂端時的實際位置算（不是依「已經貼住 top:12」的位置算，防線
  // 二）。5 種指定寬高各測一次：scrollY=0（最容易觸發的位置，見設計審核）與捲動之後
  // （scrollY=max，順便驗 fix round 1 遺留的「捲到底時左欄底緣跟底列頂緣疊在一起」也一併
  // 解掉）。
  // ---------------------------------------------------------------------------
  let n1Preview = null;
  let n1Chrome = null;
  try {
    n1Preview = await startPreview({}, 'preview-V1-n1');
    const n1Url = `http://127.0.0.1:${n1Preview.port}/`;
    n1Chrome = await startChrome(pickPort(19033, [n1Preview.port]), n1Url, 'chrome-V1-n1', '1100,900');
    const { cdp: n1cdp } = n1Chrome;
    await waitForFirstProjection(n1cdp, n1Preview.port);

    const manyResult = await injectState(n1cdp, craftManyProjectsState());
    check(manyResult && manyResult.ok === true, 'N1：注入 12 個 Project 的特製投影');
    await sleep(300);

    async function measureN1(width, height, scrollLabel) {
      const info = await n1cdp.eval(`(() => {
        var badge = document.getElementById('channel-status');
        var badgeRect = badge ? badge.getBoundingClientRect() : null;
        var hitEl = badgeRect
          ? document.elementFromPoint(badgeRect.left + badgeRect.width / 2, badgeRect.top + badgeRect.height / 2)
          : null;
        var proj = document.querySelector('[data-region="projects"]');
        var projRect = proj ? proj.getBoundingClientRect() : null;
        var statusbar = document.querySelector('[data-region="statusbar"]');
        var statusbarRect = statusbar ? statusbar.getBoundingClientRect() : null;
        return {
          hitIsChannelStatus: !!badge && !!hitEl && badge.contains(hitEl),
          hitTag: hitEl ? hitEl.tagName + '.' + hitEl.className : null,
          projBottom: projRect ? projRect.bottom : null,
          statusbarTop: statusbarRect ? statusbarRect.top : null,
        };
      })()`);
      check(
        info.hitIsChannelStatus === true,
        `N1：${width}x${height}（${scrollLabel}）底列「cockpit 服務」通道狀態的中心點應該命中 #channel-status 本身（實際命中 ${info.hitTag}）`
      );
      check(
        info.projBottom !== null && info.statusbarTop !== null && info.projBottom <= info.statusbarTop + 0.5,
        `N1：${width}x${height}（${scrollLabel}）左欄底緣應該 ≤ 底列頂緣（projBottom=${info.projBottom}, statusbarTop=${info.statusbarTop}）`
      );
    }

    const n1Sizes = [
      [760, 900],
      [1100, 900],
      [1199, 900],
      [1280, 650],
      [1536, 700],
    ];
    for (const [width, height] of n1Sizes) {
      await n1cdp.send('Emulation.setDeviceMetricsOverride', { width, height, deviceScaleFactor: 1, mobile: false });
      await sleep(200);
      await n1cdp.eval('window.scrollTo(0, 0); true');
      await sleep(150);
      await measureN1(width, height, 'scrollY=0');

      await n1cdp.eval('window.scrollTo(0, document.documentElement.scrollHeight); true');
      await sleep(200);
      await measureN1(width, height, '捲動之後');
    }
  } finally {
    await stopChrome(n1Chrome, 'chrome-V1-n1');
    await stopPreview(n1Preview, 'preview-V1-n1');
  }
}

// ---------------------------------------------------------------------------
// V2：dashboard/中等寬度（1100；task 2.1）
// ---------------------------------------------------------------------------

async function partViewportMedium() {
  log('=== V2. 1100 寬：中等寬度 ===');
  let preview = null;
  let chrome = null;
  try {
    preview = await startPreview({}, 'preview-V2');
    const url = `http://127.0.0.1:${preview.port}/`;
    chrome = await startChrome(pickPort(19040, [preview.port]), url, 'chrome-V2', '1100,900');
    const { cdp } = chrome;
    await waitForFirstProjection(cdp, preview.port);
    await installTools(cdp);

    // 選一個 pane，量「有選取」時「runtime 卡在 Factory Floor 與 Live Output 之下」（task 4.1
    // 起面板常駐，空狀態也佔同一個區塊；這裡固定量有選取的版面）。
    await cdp.eval("window.liveOutput.select('win', 'wJ:p1'); true");
    await sleep(200);

    // fix round 1／Codex F6：不是只比較 top（那樣兩個區塊只要「開始的位置」符合順序、即使
    // 中段實際重疊也會通過），改成：(1) 嚴格的 bottom ≤ top 排序（前一塊必須完整結束，後一塊
    // 才能開始）；(2) 任兩個主要區塊的矩形不應該有實際面積重疊（允許左右並排，只要不重疊）。
    const rects = await getRects(cdp, {
      topbar: '.topbar',
      floor: '.projects',
      // file-review task 4.1：spec「中等寬度」改成「runtime 卡位於 Factory Floor 與下半部分頁區的下方」——
      // 量整個分頁區（#review，含分頁列），不再只量 Live Output 面板。
      review: '#review',
      runtimes: '.runtime-cards',
      events: '.recent-events',
    });
    check(
      !!rects.floor && !!rects.runtimes && rects.runtimes.top >= rects.floor.bottom - 0.5,
      `runtime 卡應該完整在 Factory Floor 下方（floor.bottom=${rects.floor && rects.floor.bottom}, runtimes.top=${rects.runtimes && rects.runtimes.top}）`
    );
    check(
      !!rects.review && !!rects.runtimes && rects.runtimes.top >= rects.review.bottom - 0.5,
      `runtime 卡應該完整在下半部分頁區下方（review.bottom=${rects.review && rects.review.bottom}, runtimes.top=${rects.runtimes && rects.runtimes.top}）`
    );
    check(
      !!rects.floor && !!rects.review && rects.review.top >= rects.floor.bottom - 0.5,
      `下半部分頁區應該完整在 Factory Floor 下方（floor.bottom=${rects.floor && rects.floor.bottom}, review.top=${rects.review && rects.review.top}）`
    );
    // fix round 2：改用 allPairs() 產生完整配對，不再手 key（V2 的 fix round 1 就手漏過
    // ['floor','output'] 這一組，程式產生從結構上排除這種遺漏）。
    checkNoOverlap(rects, allPairs(['topbar', 'floor', 'review', 'runtimes', 'events']), '中等寬度不重疊');

    // 「頁面沒有橫向捲軸、可整頁捲動」：用自我測試證明過的偵測器（同 V1）；目前沒有任何
    // @media 斷點，這條在現行前端很可能碰巧成立（沒有任何跨欄 grid），所以額外做一次否定對
    // 照：注入一個刻意撐寬的節點，證明偵測器抓得到（Ruling R3）。
    const scroll = await cdp.eval(`(() => ({
      scrollWidth: document.documentElement.scrollWidth,
      innerWidth: window.innerWidth,
      bodyOverflowY: getComputedStyle(document.body).overflowY,
      htmlOverflowY: getComputedStyle(document.documentElement).overflowY,
    }))()`);
    const noScroll = scroll.scrollWidth <= scroll.innerWidth;
    check(noScroll, `1100 寬時頁面不應該有橫向捲軸（scrollWidth ${scroll.scrollWidth} vs innerWidth ${scroll.innerWidth}）`);
    check(
      scroll.bodyOverflowY !== 'hidden' && scroll.htmlOverflowY !== 'hidden',
      `1100 寬時頁面應該可以整頁捲動（body／html 的 overflow-y 不應該是 hidden；實際 body=${scroll.bodyOverflowY} html=${scroll.htmlOverflowY}）`
    );
    if (noScroll) {
      const negControl = await cdp.eval(`(() => {
        var wide = document.createElement('div');
        wide.id = '__hscroll_probe';
        wide.style.width = (window.innerWidth + 500) + 'px';
        wide.style.height = '1px';
        document.body.appendChild(wide);
        var overflowed = document.documentElement.scrollWidth > window.innerWidth;
        document.body.removeChild(wide);
        return overflowed;
      })()`);
      check(
        negControl === true,
        `否定對照：注入一個明顯超寬的節點後，偵測器應該能抓到橫向捲軸出現（實際 ${negControl}）— 證明上面「沒有橫向捲軸」不是偵測器失靈`
      );
    }

    // M1（設計審核，task 3.1 fix round 1）：760–1199px（兩欄）下左欄 sticky 的 top 應該是
    // 12px（沿用 .shell 的面板間距 token），不是貼齊視窗最上緣的 0——審核截圖
    // fixture-1100x900-scroll333.png 指出 top: 0 時左欄上框線直接貼在 y=0，跟其他面板永遠
    // 和視窗邊緣留 12px 間距的樣子不一致。
    // file-review task 4.1：spec cockpit-dashboard「版面與窄視窗」改成「左欄頂端為『Project』『檔案』
    // 兩個分頁，其下顯示目前分頁的內容」——左欄最上面是分頁列（#files），Project 清單在分頁列下方。
    // 「左欄貼在 top: 12px」改驗左欄頂端的 #files；Project 清單的 sticky top＝12px＋分頁列高度＋8px
    // 間距（緊接在分頁列下方一起貼住，不被分頁列蓋住、也不跟分頁列分開捲）。
    const projectsStickyAt1100 = await cdp.eval(`(() => {
      var el = document.querySelector('[data-region="projects"]');
      var cs = getComputedStyle(el);
      var files = document.getElementById('files');
      var fcs = files ? getComputedStyle(files) : null;
      var tablist = files ? files.querySelector('[role="tablist"]') : null;
      return {
        position: cs.position,
        top: cs.top,
        alignSelf: cs.alignSelf,
        filesPosition: fcs ? fcs.position : null,
        filesTop: fcs ? fcs.top : null,
        tabStripHeight: tablist ? tablist.getBoundingClientRect().height : null,
      };
    })()`);
    check(
      projectsStickyAt1100.position === 'sticky' && projectsStickyAt1100.filesPosition === 'sticky',
      `1100 寬（兩欄）時左欄（分頁列 #files 與 Project 清單）應該是 position: sticky（實際 ${JSON.stringify(projectsStickyAt1100)}）`
    );
    check(
      projectsStickyAt1100.filesTop === '12px',
      `1100 寬（兩欄）時左欄頂端（分頁列 #files）sticky 的 top 應該是 12px（設計審核 M1；實際 ${projectsStickyAt1100.filesTop}）`
    );
    check(
      typeof projectsStickyAt1100.tabStripHeight === 'number' &&
        projectsStickyAt1100.tabStripHeight > 0 &&
        Math.abs(parseFloat(projectsStickyAt1100.top) - (12 + projectsStickyAt1100.tabStripHeight + 8)) < 0.5,
      `1100 寬（兩欄）時 Project 清單 sticky 的 top 應該是 12px＋分頁列高度＋8px（實際 ${JSON.stringify(projectsStickyAt1100)}）`
    );
  } finally {
    await stopChrome(chrome, 'chrome-V2');
    await stopPreview(preview, 'preview-V2');
  }
}

// ---------------------------------------------------------------------------
// V3：dashboard/窄視窗單欄（700；task 2.1）
// ---------------------------------------------------------------------------

async function partViewportNarrow() {
  log('=== V3. 700 寬：窄視窗單欄 ===');
  let preview = null;
  let chrome = null;
  try {
    preview = await startPreview({}, 'preview-V3');
    const url = `http://127.0.0.1:${preview.port}/`;
    chrome = await startChrome(pickPort(19050, [preview.port]), url, 'chrome-V3', '700,900');
    const { cdp } = chrome;
    await waitForFirstProjection(cdp, preview.port);

    // 先選定一個 pane，量「有選取」時（標題＋內容框）面板在單欄版面的位置與順序。task 4.1 起
    // 面板常駐、沒選取時顯示空狀態，選取不再是「讓面板出現」的前提，只是固定量測的狀態。
    // 跟 V2（同一份 fixture、同一顆 pane）做法一致。
    await cdp.eval("window.liveOutput.select('win', 'wJ:p1'); true");
    await sleep(200);

    // fix round 2／Codex F6 未完成：原本用 class 當代理選擇器（`.projects` 一個 class 混雜了
    // 未來 design D2 的「projects」左欄與「floor」Factory Floor 兩個不同區塊，且完全沒驗
    // banner／output／statusbar），漏掉的區塊等於完全沒鑑別力。改成跟 V1 一致，直接用 design
    // D2 的 8 個 data-region 名字逐一查（`[data-region="X"]`）——這些屬性現在還不存在，所以
    // 大多數會是 null，誠實反映「還沒做」，不是刻意放寬。banner 是條件式存在（GIVEN 沒有錯誤／
    // 改綁時本來就不該出現），不存在時用 log() 說明原因跳過、不算 check()（不記 PASS 也不記
    // FAIL）。
    const REGION_SELECTORS = {
      topbar: '[data-region="topbar"]',
      projects: '[data-region="projects"]',
      banner: '[data-region="banner"]',
      floor: '[data-region="floor"]',
      output: '[data-region="output"]',
      runtimes: '[data-region="runtimes"]',
      events: '[data-region="events"]',
      statusbar: '[data-region="statusbar"]',
    };
    const rects = await getRects(cdp, REGION_SELECTORS);
    const innerWidthForRects = await cdp.eval('window.innerWidth');
    const REQUIRED_REGIONS = ['topbar', 'projects', 'floor', 'output', 'runtimes', 'events', 'statusbar'];
    for (const label of REQUIRED_REGIONS) {
      const r = rects[label];
      check(r !== null, `窄視窗單欄：data-region="${label}" 應該存在且可見（design D2；目前多數尚未實作，預期 FAIL）`);
      if (r) {
        check(
          r.width >= innerWidthForRects * 0.7,
          `窄視窗單欄：data-region="${label}" 的寬度應該接近視窗寬度（單欄排列，門檻取視窗寬度的 70%；實際 ${r.width} / 視窗 ${innerWidthForRects}）`
        );
      }
    }
    if (rects.banner === null) {
      log('窄視窗單欄：data-region="banner" 不存在——GIVEN 沒有錯誤訊息／改綁提示，這個區塊本來就不該出現，跳過寬度／順序／重疊檢查（不計入 ok 或 FAIL）');
    } else {
      check(
        rects.banner.width >= innerWidthForRects * 0.7,
        `窄視窗單欄：data-region="banner" 存在時寬度應該接近視窗寬度（實際 ${rects.banner.width} / 視窗 ${innerWidthForRects}）`
      );
    }

    // 垂直順序（M7，task 2.1 fix round 1，design 審核＋控制端確認、不需使用者裁決）：
    // topbar → projects →（banner，若存在）→ floor → runtimes → output → events，由上而下、
    // 嚴格 bottom ≤ top。banner 緊貼 Factory Floor 上方（跟三欄版「中欄上方」語意一致）；
    // runtime 在 Live Output 之前（單欄是「清單→明細」關係，760–1199 兩欄仍是 spec 規定的
    // output 在 runtimes 之前，不受這條約束）。
    //
    // fix round 1（design 審核 I1，task 2.3）：statusbar 不再排進這條文件順序鏈——它現在是
    // `position: sticky; bottom: 0`（見 style.css），視覺上永遠貼在視窗底部，
    // getBoundingClientRect() 回傳的座標是「相對目前視窗」的浮動位置，不是文件順序位置，跟
    // events 比較 bottom≤top 恆假（events 深埋在文件裡、statusbar 浮在視窗底部）。sticky 的
    // 「永遠在第一屏可見」改用下面獨立的正面斷言驗（regression e），不是恆真也不是誤判成
    // FAIL。
    const order = [
      'topbar',
      'projects',
      ...(rects.banner !== null ? ['banner'] : []),
      'floor',
      'runtimes',
      'output',
      'events',
    ];
    for (let i = 0; i < order.length - 1; i += 1) {
      const a = rects[order[i]];
      const b = rects[order[i + 1]];
      check(
        !!a && !!b && b.top >= a.bottom - 0.5,
        `窄視窗單欄：${order[i]} 應該完整在 ${order[i + 1]} 上方（${order[i]}.bottom=${a && a.bottom}, ${order[i + 1]}.top=${b && b.top}）`
      );
    }

    // 不重疊：所有存在的區塊兩兩都不應該有實際面積重疊（用 allPairs() 產生完整組合，不手key
    // 清單——V2 的 fix round 1 就漏過一組 ['floor','output']，這裡直接用程式產生避免重蹈）。
    // fix round 1（I1）：statusbar 排除在外，理由同上——sticky 元素本來就會視覺蓋在文件流內容
    // 上方，「不重疊」對它不是有意義的斷言（若它真的完全不重疊任何東西，代表 sticky 根本沒有
    // 生效，那種情況由下面的 regression e 抓）。
    const overlapLabels = REQUIRED_REGIONS.filter((label) => label !== 'statusbar').concat(
      rects.banner !== null ? ['banner'] : []
    );
    checkNoOverlap(rects, allPairs(overlapLabels), '窄視窗單欄不重疊');

    // regression e（控制端裁決，task 2.3 fix round 1；fix round 2／Codex medium #2 收緊）：
    // 700×900（整頁捲動版面）下，底列應該用真正的 position: sticky 貼在視窗底部——不是只看
    // 頁首的座標（上一輪的寫法，position: fixed／absolute 或捲動後消失的錯誤實作都能矇混過
    // 關），完整驗證見 checkStickyStatusbar()：計算樣式、頁首與捲動後的貼底、DOM 順序、
    // 兩個否定對照。
    await checkStickyStatusbar(cdp, '700×900');

    // I1（設計審核，task 3.1 fix round 1）：< 760px 單欄不做左欄 sticky——單欄時左欄和其他
    // 區塊同寬，貼頂會蓋住下面捲過去的內容（審核截圖 fixture-700x900-scroll303.png／
    // many-700x900-scroll421.png）。這裡直接讀 computed position，不是恆真：否定對照見下方
    // （單欄底列的 sticky 已經在 checkStickyStatusbar() 裡用同一套「改成別的 position 就會
    // 不一樣」的手法驗過一次，這裡不重複整套否定對照，只確認左欄本身不是 sticky）。
    const projectsPosAt700 = await cdp.eval(
      "getComputedStyle(document.querySelector('[data-region=\"projects\"]')).position"
    );
    check(
      projectsPosAt700 !== 'sticky',
      `700 寬（單欄）時左欄不應該是 position: sticky（design 審核 I1；實際 ${projectsPosAt700}）`
    );

    const info = await cdp.eval(`(() => ({
      scrollWidth: document.documentElement.scrollWidth,
      innerWidth: window.innerWidth,
      bodyOverflowY: getComputedStyle(document.body).overflowY,
      htmlOverflowY: getComputedStyle(document.documentElement).overflowY,
    }))()`);
    const noScroll = info.scrollWidth <= info.innerWidth;
    check(noScroll, `700 寬時頁面不應該有橫向捲軸（scrollWidth ${info.scrollWidth} vs innerWidth ${info.innerWidth}）`);
    check(
      info.bodyOverflowY !== 'hidden' && info.htmlOverflowY !== 'hidden',
      `700 寬時頁面應該可以整頁捲動（body／html 的 overflow-y 不應該是 hidden；實際 body=${info.bodyOverflowY} html=${info.htmlOverflowY}）`
    );
    if (noScroll) {
      const negControl = await cdp.eval(`(() => {
        var wide = document.createElement('div');
        wide.id = '__hscroll_probe';
        wide.style.width = (window.innerWidth + 500) + 'px';
        wide.style.height = '1px';
        document.body.appendChild(wide);
        var overflowed = document.documentElement.scrollWidth > window.innerWidth;
        document.body.removeChild(wide);
        return overflowed;
      })()`);
      check(
        negControl === true,
        `否定對照：注入超寬節點後應該能偵測到橫向捲軸（實際 ${negControl}）— 證明「沒有橫向捲軸」不是偵測器失靈`
      );
    }
  } finally {
    await stopChrome(chrome, 'chrome-V3');
    await stopPreview(preview, 'preview-V3');
  }
}

// ---------------------------------------------------------------------------
// V4：dashboard/寬但矮的視窗（1280x650；task 2.1 新增段落——design D9「新增的『寬但矮的視窗』
// scenario 留給 task 2.1 補段，本檔不搶做」）。驗 design D3「≥1200px 但高度 <720px」的情形：
// 仍三欄（跟桌面寬度一樣的欄數），但取消固定一屏高度、允許整頁捲動；Factory Floor 最小高度仍
// 240px；Live Output 改固定高度 clamp(320px, 50vh, 560px)、內部捲動（跟 760–1199px 同規則，
// 不是桌面固定一屏那種跟 Factory Floor 按比例分配）。
// ---------------------------------------------------------------------------

async function partViewportWideShort() {
  log('=== V4. 1280x650：寬但矮的視窗 ===');
  let preview = null;
  let chrome = null;
  try {
    preview = await startPreview({}, 'preview-V4');
    const url = `http://127.0.0.1:${preview.port}/`;
    chrome = await startChrome(pickPort(19035, [preview.port]), url, 'chrome-V4', '1280,650');
    const { cdp } = chrome;
    await waitForFirstProjection(cdp, preview.port);

    // 跟 V2／V3 一樣先選定一個 pane，量「有選取」時的版面（task 4.1 起面板常駐，面板高度由版面
    // 決定、空狀態與有選取同高，見 V1 R17 矩陣與 LO1 [LO1/fill]）。
    await cdp.eval("window.liveOutput.select('win', 'wJ:p1'); true");
    await sleep(200);

    const rects = await getRects(cdp, {
      topbar: '[data-region="topbar"]',
      projects: '[data-region="projects"]',
      floor: '[data-region="floor"]',
      output: '[data-region="output"]',
      // file-review task 4.1：spec「寬但矮的視窗」的高度下限改量下半部分頁區（見下方 clamp 斷言）。
      review: '[data-region="review"]',
      runtimes: '[data-region="runtimes"]',
      events: '[data-region="events"]',
      statusbar: '[data-region="statusbar"]',
    });
    for (const label of ['topbar', 'projects', 'floor', 'output', 'review', 'runtimes', 'events', 'statusbar']) {
      check(rects[label] !== null, `寬但矮的視窗：data-region="${label}" 應該存在且可見（實際 ${JSON.stringify(rects[label])}）`);
    }

    // --- 仍是三欄（跟桌面寬度一樣，不是 760–1199px 的兩欄）：runtimes 應該在 floor／output
    // 右側，不是下方 ---
    check(
      !!rects.floor && !!rects.runtimes && rects.runtimes.left >= rects.floor.right - 0.5,
      `寬但矮的視窗應該仍是三欄（runtimes 在 floor 右側，不是下方；floor.right=${rects.floor && rects.floor.right}, runtimes.left=${rects.runtimes && rects.runtimes.left}）`
    );
    // fix round 1（design 審核 I1，task 2.3）：statusbar 排除在不重疊檢查之外，理由同 V3——
    // 它現在是 position: sticky; bottom: 0（style.css），視覺上貼在視窗底部，getBoundingClientRect
    // 回傳的是浮動位置而不是文件順序位置，跟文件流裡的其他區塊比較「不重疊」恆假。sticky 的
    // 「永遠在第一屏可見」改用下面獨立的正面斷言驗（regression e）。
    checkNoOverlap(
      rects,
      allPairs(['topbar', 'projects', 'floor', 'output', 'runtimes', 'events']),
      '寬但矮的視窗不重疊'
    );

    // regression e（控制端裁決，task 2.3 fix round 1；fix round 2／Codex medium #2 收緊，
    // 理由同 V3）：1280×650（整頁捲動版面）下，完整驗證底列的 sticky 行為，見
    // checkStickyStatusbar()。
    await checkStickyStatusbar(cdp, '1280×650');

    // --- 取消固定一屏高度、允許整頁捲動（跟 V1 桌面固定一屏相反）---
    const scroll = await cdp.eval(`(() => ({
      scrollHeight: document.documentElement.scrollHeight,
      innerHeight: window.innerHeight,
      scrollWidth: document.documentElement.scrollWidth,
      innerWidth: window.innerWidth,
      bodyOverflowY: getComputedStyle(document.body).overflowY,
      htmlOverflowY: getComputedStyle(document.documentElement).overflowY,
      shellOverflowY: getComputedStyle(document.querySelector('.shell')).overflowY,
    }))()`);
    check(
      scroll.bodyOverflowY !== 'hidden' && scroll.htmlOverflowY !== 'hidden' && scroll.shellOverflowY !== 'hidden',
      `寬但矮的視窗應該允許整頁捲動（body／html／.shell 的 overflow-y 都不應該是 hidden；實際 ${JSON.stringify(scroll)}）`
    );
    const noHScroll = scroll.scrollWidth <= scroll.innerWidth;
    check(noHScroll, `寬但矮的視窗不應該有橫向捲軸（scrollWidth ${scroll.scrollWidth} vs innerWidth ${scroll.innerWidth}）`);
    if (noHScroll) {
      const negControl = await cdp.eval(`(() => {
        var wide = document.createElement('div');
        wide.id = '__hscroll_probe';
        wide.style.width = (window.innerWidth + 500) + 'px';
        wide.style.height = '1px';
        document.body.appendChild(wide);
        var overflowed = document.documentElement.scrollWidth > window.innerWidth;
        document.body.removeChild(wide);
        return overflowed;
      })()`);
      check(negControl === true, `否定對照：注入超寬節點後應該能偵測到橫向捲軸（實際 ${negControl}）— 證明「沒有橫向捲軸」不是偵測器失靈`);
    }

    // --- Factory Floor 最小高度 240px ---
    check(
      !!rects.floor && rects.floor.height >= 235,
      `寬但矮的視窗：Factory Floor 最小高度應該約 240px（實際 ${rects.floor && rects.floor.height}）`
    );

    // --- Live Output 固定高度 clamp(320px, 50vh, 560px)：跟桌面固定一屏（V1）那種按
    // Factory Floor/Live Output 45% 動態分配不是同一條規則——這裡另外驗證它「不是」跟著剩餘
    // 空間按比例變化，而是落在 clamp 的區間裡。--window-size 要求的高度（650）跟
    // window.innerHeight 之間有 headless Chrome 自己保留的一段落差（實測約 99px，猜測是
    // headless 模式模擬的視窗外框），不能直接拿 650 算 50vh，改用 window.innerHeight 現場算。
    const viewportInnerHeight = await cdp.eval('window.innerHeight');
    const expectedOutputHeight = Math.min(560, Math.max(320, viewportInnerHeight * 0.5));
    // file-review task 4.1：spec「寬但矮的視窗」改為「下半部分頁區高度不小於 320px」——clamp 改驗分頁區
    // （#review），Live Output 是其中第一個分頁的內容。
    check(
      !!rects.review && Math.abs(rects.review.height - expectedOutputHeight) <= 5,
      `寬但矮的視窗：下半部分頁區高度應該是 clamp(320px, 50vh, 560px)（innerHeight=${viewportInnerHeight}，預期 ${expectedOutputHeight}，實際 ${rects.review && rects.review.height}）`
    );

    // --- M9（使用者 2026-09-24 裁決）：Factory Floor 加高度上限「視窗高度－頂列－提示列－
    // 200px」（下限仍 240px），讓 Live Output 的開頭落在第一屏內（top < innerHeight），不必先
    // 捲過 Factory Floor 才看得到。預設 fixture 有兩個 Project 疊在一起，內容夠多，足以驗證
    // 這條上限真的生效（沒有上限時 fix round 1 之前實測 Factory Floor 高度是 930px，Live
    // Output 完全落在第一屏外）。 ---
    check(
      !!rects.floor && rects.floor.height <= viewportInnerHeight,
      `寬但矮的視窗：Factory Floor 應該有高度上限，不能比視窗本身還高（design M9；實際 floor 高度=${rects.floor && rects.floor.height}，innerHeight=${viewportInnerHeight}）`
    );
    check(
      !!rects.output && rects.output.top < viewportInnerHeight,
      `寬但矮的視窗：Live Output 的頂端應該落在第一屏內（design M9，使用者 2026-09-24 裁決；實際 top=${rects.output && rects.output.top}，innerHeight=${viewportInnerHeight}）`
    );
  } finally {
    await stopChrome(chrome, 'chrome-V4');
    await stopPreview(preview, 'preview-V4');
  }
}

// ---------------------------------------------------------------------------
// G1：dashboard/Scenario D 的畫面＋未知 status 不破壞畫面＋狀態不只靠顏色（Factory Floor；
// task 3.2）。fix round 1／規格檢查 Important：從舊版 G1 拆出「running 節點沒有動畫」
// （task 2.2，見下面的 G2），兩個 task 各自能只跑自己範圍的段代號驗收，不用從混在一起的輸出裡
// 人工挑行。
// direction-01-visual task 3.2 實作時改寫／補強（子斷言以 [G1/xxx] 標籤區分，Ruling R4 慣例）：
//   - task 3.1 起中上區域只畫選定的 Project，Scenario D（Project `p`）要先點左欄切過去才看得到
//     （原本直接找 `.project[data-project="p"]`，3.1 之後恆找不到）；「狀態不只靠顏色」與
//     「未知 status」用預設選定的 cockpit，Scenario D 驗完切回 cockpit 再注入。
//   - Scenario D 的「狀態文字為品牌強調色」改讀狀態文字本身（.task-status-label）的計算色——
//     節點改成 --surface 底＋--text 標題之後，節點外層的 color 是標題色，不是狀態文字色；另外
//     加驗四邊 2px --accent 外框與柔光（design D4「2px 外框＋靜止柔光」）。
//   - 下面每一組新斷言都附否定對照（在頁面上暫時造出違規，證明偵測器抓得到，隨即還原）：
//     [G1/node] 六種已知 status 的符號、色條、狀態色、固定寬度與對比；[G1/button] 節點按鈕
//     是中性第三層級樣式；[G1/glow] 全畫面只有 running 節點發光；[G1/title] 主標題在不捲動的
//     標題列、20/600，標題列沒有 2.2 的刻度骨架，.project 不再畫第二層框；[G1/tick] 刻度是
//     stage 欄首上緣的 DOM 元素，依 stage 資料標出 running；[G1/sticky] 10 個 stage 雙向捲動時
//     列首／欄首／左上角格固定不動、不透明、蓋在格子上方，刻度跟欄位對齊；[G1/binding]
//     runtime 名稱很長時只有 runtime 被省略、pane id 完整可見；[G1/unknown] 未知 status 的
//     虛線外框與次要文字色，且和 pending 分得開。
// ---------------------------------------------------------------------------

// design D4 task 列：status → 狀態色（SPEC_COLORS 的鍵）與符號（ready 的 ♢ 見 design D4
// 「狀態符號」3.2 更新；running 是 ▶ 接 U+FE0E）。
const G1_STATUS_SPEC = {
  running: { color: 'accent', symbol: '▶︎' },
  blocked: { color: 'warn', symbol: '‖' },
  ready: { color: 'text', symbol: '♢' },
  pending: { color: 'textDim', symbol: '○' },
  failed: { color: 'bad', symbol: '✕' },
  completed: { color: 'ok', symbol: '✓' },
};

// 頁面內的節點檢查器：回傳這個節點的量測值與「違反了哪些條件」的清單。正式斷言與否定對照
// 共用同一個函式，否定對照才證明得了正式斷言有辨識力。
const G1_NODE_PROBE_JS = `
  window.__g1ProbeNode = function (node, expect) {
    var tools = window.__cockpitVisualTools;
    var problems = [];
    var symbol = node.querySelector('.task-status-symbol');
    var label = node.querySelector('.task-status-label');
    var title = node.querySelector('.task-title');
    var cs = getComputedStyle(node);
    var out = { problems: problems };
    if (!symbol) { problems.push('沒有 .task-status-symbol'); return out; }
    if (!label) { problems.push('沒有 .task-status-label'); return out; }
    if (!title) { problems.push('沒有 .task-title'); return out; }
    var scs = getComputedStyle(symbol);
    var lcs = getComputedStyle(label);
    out.symbolText = symbol.textContent;
    out.labelText = label.textContent;
    out.symbolColor = scs.color;
    out.labelColor = lcs.color;
    out.stripeColor = cs.borderLeftColor;
    out.stripeWidth = parseFloat(cs.borderLeftWidth);
    out.nodeBg = cs.backgroundColor;
    out.symbolWidth = symbol.getBoundingClientRect().width;
    out.symbolFontSize = parseFloat(scs.fontSize);
    if (symbol.getAttribute('aria-hidden') !== 'true') problems.push('符號沒有 aria-hidden="true"');
    if (symbol.textContent !== expect.symbol) problems.push('符號應該是 ' + JSON.stringify(expect.symbol) + '（實際 ' + JSON.stringify(symbol.textContent) + '）');
    if (getComputedStyle(symbol, '::before').content !== 'none' || getComputedStyle(symbol, '::after').content !== 'none') problems.push('符號不得用 CSS content 畫');
    if (label.textContent !== expect.status) problems.push('status 文字應該是 ' + expect.status + '（實際 ' + JSON.stringify(label.textContent) + '）');
    if (!(symbol.compareDocumentPosition(label) & Node.DOCUMENT_POSITION_FOLLOWING)) problems.push('符號應該在 status 文字前面');
    var tr = title.getBoundingClientRect();
    var sr = symbol.getBoundingClientRect();
    var actions = node.querySelector('.task-actions');
    if (sr.top < tr.bottom - 0.5) problems.push('符號＋狀態應該在標題下方');
    if (actions && actions.getBoundingClientRect().top < sr.bottom - 0.5) problems.push('按鈕應該在符號＋狀態下方');
    if (scs.display !== 'inline-block' && scs.display !== 'block') problems.push('符號 display 應該是 inline-block（實際 ' + scs.display + '）');
    if (scs.textAlign !== 'center') problems.push('符號 text-align 應該是 center（實際 ' + scs.textAlign + '）');
    if (Math.abs(out.symbolWidth - 1.25 * out.symbolFontSize) > 0.5) problems.push('符號寬度應該是 1.25em（' + (1.25 * out.symbolFontSize) + 'px，實際 ' + out.symbolWidth + '）');
    if (scs.color !== expect.color) problems.push('符號顏色應該是 ' + expect.color + '（實際 ' + scs.color + '）');
    if (lcs.color !== expect.color) problems.push('status 文字顏色應該是 ' + expect.color + '（實際 ' + lcs.color + '）');
    if (cs.borderLeftColor !== expect.color) problems.push('左緣色條應該是 ' + expect.color + '（實際 ' + cs.borderLeftColor + '）');
    if (!(out.stripeWidth >= 3)) problems.push('左緣色條寬度應該 ≥3px（實際 ' + cs.borderLeftWidth + '）');
    if (cs.backgroundColor !== expect.surface) problems.push('節點底色應該是 --surface（實際 ' + cs.backgroundColor + '）');
    var stripe = tools.parseColor(cs.borderLeftColor);
    var outside = tools.effectiveBackground(node.parentElement);
    out.stripeVsSurface = tools.contrastRatio(stripe, tools.parseColor(cs.backgroundColor));
    out.stripeVsOutside = tools.contrastRatio(stripe, outside);
    if (!(out.stripeVsSurface >= 3)) problems.push('色條對節點底色 <3:1（' + out.stripeVsSurface.toFixed(2) + '）');
    if (!(out.stripeVsOutside >= 3)) problems.push('色條對格子底色 <3:1（' + out.stripeVsOutside.toFixed(2) + '）');
    out.labelContrast = tools.textContrast(label).ratio;
    out.symbolContrast = tools.textContrast(symbol).ratio;
    out.titleContrast = tools.textContrast(title).ratio;
    if (!(out.labelContrast >= 4.5)) problems.push('status 文字對比 <4.5:1（' + out.labelContrast.toFixed(2) + '）');
    if (!(out.titleContrast >= 4.5)) problems.push('標題對比 <4.5:1（' + out.titleContrast.toFixed(2) + '）');
    return out;
  };
`;

async function partFactoryFloor() {
  log('=== G1. Factory Floor（task 3.2）：Scenario D 的畫面／未知 status 不破壞畫面／狀態不只靠顏色 ===');
  let preview = null;
  let chrome = null;
  try {
    preview = await startPreview({}, 'preview-G1');
    const url = `http://127.0.0.1:${preview.port}/`;
    chrome = await startChrome(pickPort(19060, [preview.port]), url, 'chrome-G1');
    const { cdp } = chrome;
    await waitForFirstProjection(cdp, preview.port);
    await installTools(cdp);
    await cdp.eval(`(() => { ${G1_NODE_PROBE_JS} return true; })()`);
    const expectFor = (status) => ({
      status,
      symbol: G1_STATUS_SPEC[status].symbol,
      color: SPEC_COLORS[G1_STATUS_SPEC[status].color],
      surface: SPEC_COLORS.surface,
    });

    // --- [G1/node] 狀態不只靠顏色（spec「Direction 01 視覺語彙」）＋節點換皮（design D4）：預設
    // 選定的 cockpit 已經有六種已知 status 各至少一個（be-1 running、qa-1 blocked、release-1
    // ready、docs-1 pending、ops-2 failed、ops-1 completed），不需要注入。一定要在「未知
    // status」注入之前做：那段注入會把 be-1 改成 whatever。
    const knownCells = [
      ['running', 'be', 'Implement'],
      ['blocked', 'qa', 'Implement'],
      ['ready', 'release', 'Spec'],
      ['pending', 'docs', 'Spec'],
      ['failed', 'ops', 'Spec'],
      ['completed', 'ops', 'Review'],
    ];
    const probes = await cdp.eval(`(() => {
      var cases = ${JSON.stringify(knownCells.map(([status, ws, stage]) => ({ ws, stage, expect: expectFor(status) })))};
      return cases.map(function (c) {
        var node = document.querySelector('.project[data-project="cockpit"] .ff-cell[data-workstream="' + c.ws + '"][data-stage="' + c.stage + '"] .task-node');
        if (!node) return { status: c.expect.status, found: false };
        var r = window.__g1ProbeNode(node, c.expect);
        r.status = c.expect.status;
        r.found = true;
        r.readText = node.querySelector('.task-status-symbol').textContent + node.querySelector('.task-status-label').textContent;
        return r;
      });
    })()`);
    const readTexts = new Set();
    for (const p of probes) {
      check(p.found === true, `[G1/node] 找到 ${p.status} 的 task 節點`);
      if (!p.found) continue;
      check(
        p.problems.length === 0,
        `[G1/node] ${p.status} 節點：aria-hidden 符號 ${JSON.stringify(p.symbolText)}＋status 文字、符號 1.25em 置中、符號／文字／色條同一個狀態色、--surface 底、色條 ≥3:1、文字 ≥4.5:1、由上往下標題→狀態→按鈕（問題：${JSON.stringify(p.problems)}；量測：色條對節點 ${p.stripeVsSurface && p.stripeVsSurface.toFixed(2)}、對格子 ${p.stripeVsOutside && p.stripeVsOutside.toFixed(2)}、status 文字 ${p.labelContrast && p.labelContrast.toFixed(2)}、標題 ${p.titleContrast && p.titleContrast.toFixed(2)}）`
      );
      readTexts.add(p.readText);
    }
    check(
      readTexts.size === probes.filter((p) => p.found).length,
      `[G1/node] 只讀文字（符號＋status 字串）六種狀態彼此都不同（灰階下也分得出；實際 ${JSON.stringify([...readTexts])}）`
    );
    // 否定對照：同一個檢查器對「拿掉 aria-hidden」「換掉符號」「色條換成別的 token」「符號寬度
    // 被改掉」的節點都要回報問題。
    const nodeNeg = await cdp.eval(`(() => {
      var node = document.querySelector('.project[data-project="cockpit"] .ff-cell[data-workstream="ops"][data-stage="Spec"] .task-node');
      var expect = ${JSON.stringify(expectFor('failed'))};
      var sym = node.querySelector('.task-status-symbol');
      var out = {};
      sym.removeAttribute('aria-hidden');
      out.ariaHidden = window.__g1ProbeNode(node, expect).problems.length;
      sym.setAttribute('aria-hidden', 'true');
      var orig = sym.textContent;
      sym.textContent = '';
      out.noSymbol = window.__g1ProbeNode(node, expect).problems.length;
      sym.textContent = orig;
      node.style.borderLeftColor = 'var(--ok)';
      out.stripe = window.__g1ProbeNode(node, expect).problems.length;
      node.style.borderLeftColor = '';
      sym.style.width = 'auto';
      out.width = window.__g1ProbeNode(node, expect).problems.length;
      sym.style.width = '';
      out.clean = window.__g1ProbeNode(node, expect).problems.length;
      return out;
    })()`);
    check(
      nodeNeg.ariaHidden > 0 && nodeNeg.noSymbol > 0 && nodeNeg.stripe > 0 && nodeNeg.width > 0 && nodeNeg.clean === 0,
      `[G1/node] 否定對照：拿掉 aria-hidden／清空符號／色條換成 --ok／符號寬度改 auto 都要被抓到，還原後回到 0 個問題（實際 ${JSON.stringify(nodeNeg)}）`
    );

    // --- [G1/button] 節點上的動作按鈕一律是 D4「動作按鈕」第三層級中性樣式：透明底、
    // --text-dim 文字與四邊外框，不用任何狀態色（設計審核檢查清單「沒有綠色或紅色按鈕」）。
    const BUTTON_PROBE = `function (btn) {
      var cs = getComputedStyle(btn);
      var dim = ${JSON.stringify(SPEC_COLORS.textDim)};
      var bad = [];
      if (cs.color !== dim) bad.push('color ' + cs.color);
      ['Top', 'Right', 'Bottom', 'Left'].forEach(function (s) { if (cs['border' + s + 'Color'] !== dim) bad.push('border' + s + ' ' + cs['border' + s + 'Color']); });
      if (cs.backgroundColor !== 'rgba(0, 0, 0, 0)') bad.push('background ' + cs.backgroundColor);
      return bad;
    }`;
    const buttons = await cdp.eval(`(() => {
      var probe = ${BUTTON_PROBE};
      var list = document.querySelectorAll('.task-node .action-button');
      var bad = [];
      for (var i = 0; i < list.length; i += 1) {
        var b = probe(list[i]);
        if (b.length) bad.push(list[i].textContent + ': ' + b.join(', '));
      }
      var victim = document.querySelector('.task-status-failed .action-button');
      victim.style.color = 'var(--bad)';
      var neg = probe(victim).length;
      victim.style.color = '';
      return { count: list.length, bad: bad, neg: neg };
    })()`);
    check(buttons.count > 0 && buttons.bad.length === 0, `[G1/button] 全部 ${buttons.count} 顆節點按鈕都是透明底＋--text-dim 文字與外框（違規：${JSON.stringify(buttons.bad)}）`);
    check(buttons.neg > 0, `[G1/button] 否定對照：把 failed 節點的按鈕文字暫時改成 --bad 必須被抓到（實際 ${buttons.neg} 個問題）`);

    // --- [G1/glow] 全畫面只有 running 節點發光（design D4「冰青的形狀分工」：柔光只給
    // running）：走訪 document 裡每個元素的計算 box-shadow。
    const GLOW_SCAN = `function () {
      var all = document.querySelectorAll('*');
      var glowing = [];
      for (var i = 0; i < all.length; i += 1) {
        if (getComputedStyle(all[i]).boxShadow !== 'none') glowing.push(all[i]);
      }
      var stray = glowing.filter(function (e) { return !e.classList.contains('task-status-running'); });
      var running = document.querySelectorAll('.task-node.task-status-running');
      var unlit = Array.prototype.filter.call(running, function (e) { return getComputedStyle(e).boxShadow === 'none'; });
      return { glowing: glowing.length, stray: stray.map(function (e) { return e.className; }), running: running.length, unlit: unlit.length };
    }`;
    const glow = await cdp.eval(`(() => {
      var scan = ${GLOW_SCAN};
      var real = scan();
      var victim = document.querySelector('.task-node.task-status-ready');
      victim.style.boxShadow = '0 0 8px var(--accent)';
      var neg = scan();
      victim.style.boxShadow = '';
      return { real: real, neg: neg };
    })()`);
    check(
      glow.real.running > 0 && glow.real.unlit === 0 && glow.real.stray.length === 0,
      `[G1/glow] 全畫面發光（box-shadow 不是 none）的元素只有 running 節點，且每個 running 節點都有柔光（實際 ${JSON.stringify(glow.real)}）`
    );
    check(glow.neg.stray.length > 0, `[G1/glow] 否定對照：暫時讓 ready 節點發光必須被抓到（實際 ${JSON.stringify(glow.neg)}）`);
    // --- [G1/emphasis] task 3.2 fix round 1（設計審核 I1／Ruling R31）：failed、blocked 除了 4px
    // 色條，四邊再加 1px 狀態色細框、status 文字 600，框對節點底色 ≥3:1（必要圖形）；形狀跟 running
    // 分得開（1px、不發光 vs 2px 冰青＋柔光）。其他非 running 狀態維持 --line 細框、400。
    const EMPHASIS_PROBE = `function (node) {
      var tools = window.__cockpitVisualTools;
      var cs = getComputedStyle(node);
      var label = node.querySelector('.task-status-label');
      var sides = ['Top', 'Right', 'Bottom'];
      var status = tools.parseColor(getComputedStyle(label).color);
      return {
        framed: sides.every(function (s) { return parseFloat(cs['border' + s + 'Width']) >= 1 && cs['border' + s + 'Style'] === 'solid' && cs['border' + s + 'Color'] === getComputedStyle(label).color; }),
        widths: sides.map(function (s) { return cs['border' + s + 'Width']; }).join(' '),
        frameVsSurface: tools.contrastRatio(tools.parseColor(cs.borderTopColor), tools.parseColor(cs.backgroundColor)),
        weight: getComputedStyle(label).fontWeight,
        glow: cs.boxShadow,
        statusColor: status,
      };
    }`;
    const emphasis = await cdp.eval(`(() => {
      var probe = ${EMPHASIS_PROBE};
      function at(ws, stage) { return document.querySelector('.project[data-project="cockpit"] .ff-cell[data-workstream="' + ws + '"][data-stage="' + stage + '"] .task-node'); }
      var failed = at('ops', 'Spec'), blocked = at('qa', 'Implement'), ready = at('release', 'Spec'), completed = at('ops', 'Review'), pending = at('docs', 'Spec');
      var out = { failed: probe(failed), blocked: probe(blocked), ready: probe(ready), completed: probe(completed), pending: probe(pending) };
      failed.style.borderColor = 'var(--line)';
      out.negFrame = probe(failed);
      failed.style.borderColor = '';
      failed.querySelector('.task-status-label').style.fontWeight = '400';
      out.negWeight = probe(failed);
      failed.querySelector('.task-status-label').style.fontWeight = '';
      return out;
    })()`);
    for (const st of ['failed', 'blocked']) {
      const e = emphasis[st];
      check(
        e.framed && e.widths === '1px 1px 1px' && e.frameVsSurface >= 3 && e.weight === '600' && e.glow === 'none',
        `[G1/emphasis] ${st} 節點：四邊 1px 實線狀態色細框（對 --surface ≥3:1）、status 文字 600、不發光（實際 ${JSON.stringify(e)}）`
      );
    }
    for (const st of ['ready', 'completed', 'pending']) {
      const e = emphasis[st];
      check(!e.framed && e.weight === '400', `[G1/emphasis] ${st} 節點不加狀態色細框、status 文字 400（強調只給 failed／blocked；實際 ${JSON.stringify(e)}）`);
    }
    check(
      !emphasis.negFrame.framed && emphasis.negWeight.weight !== '600',
      `[G1/emphasis] 否定對照：failed 的細框改回 --line、status 文字改回 400 都必須被抓到（實際 frame=${JSON.stringify(emphasis.negFrame)} weight=${emphasis.negWeight.weight}）`
    );

    // --- [G1/title] 主標題與框：選定 Project 的 name 是 20/600 主標題（design D10），放在外框
    // 裡、內層捲動容器之外（捲動時不跟著走）；標題列不再有 2.2 的刻度骨架；.project 不再畫
    // 第二層框（2.2 設計審核 F7）。
    const TITLE_PROBE = `function () {
      var region = document.querySelector('[data-region="floor"]');
      var name = region && region.querySelector('.project-name');
      var sc = region && region.querySelector('.projects');
      var panel = sc && sc.querySelector('.project');
      if (!name || !sc || !panel) return null;
      var ncs = getComputedStyle(name);
      var pcs = getComputedStyle(panel);
      return {
        text: name.textContent,
        fontSize: ncs.fontSize,
        fontWeight: ncs.fontWeight,
        color: ncs.color,
        insideScroller: sc.contains(name),
        regionBgImage: getComputedStyle(region).backgroundImage,
        panelBorder: [pcs.borderTopWidth, pcs.borderRightWidth, pcs.borderBottomWidth, pcs.borderLeftWidth].join(' '),
        panelBg: pcs.backgroundColor,
      };
    }`;
    const title = await cdp.eval(`(${TITLE_PROBE})()`);
    check(title !== null, '[G1/title] 找得到 Factory Floor 外框裡的 .project-name、.projects 與 .project');
    if (title !== null) {
      check(title.text === 'AI Cockpit', `[G1/title] 標題是目前選定 Project（cockpit）的 name（實際 ${JSON.stringify(title.text)}）`);
      check(
        title.fontSize === '20px' && title.fontWeight === '600' && title.color === SPEC_COLORS.text,
        `[G1/title] 主標題 20px／600／--text（design D10；實際 ${title.fontSize}／${title.fontWeight}／${title.color}）`
      );
      check(title.insideScroller === false, '[G1/title] 標題在內層捲動容器 .projects 之外（捲動網格時標題不跟著走）');
      check(
        title.regionBgImage.indexOf('repeating-linear-gradient') === -1,
        `[G1/title] 外框背景不再有 2.2 標題列右側的刻度骨架（repeating-linear-gradient；實際 ${title.regionBgImage.slice(0, 120)}…）`
      );
      check(
        title.panelBorder === '0px 0px 0px 0px' && title.panelBg === 'rgba(0, 0, 0, 0)',
        `[G1/title] .project 不再畫第二層框（框線 0、透明底；實際 ${title.panelBorder}／${title.panelBg}）`
      );
    }
    const titleNeg = await cdp.eval(`(() => {
      var name = document.querySelector('[data-region="floor"] .project-name');
      var sc = document.querySelector('[data-region="floor"] .projects');
      var parent = name.parentElement;
      var next = name.nextSibling;
      sc.insertBefore(name, sc.firstChild);
      var r = (${TITLE_PROBE})();
      parent.insertBefore(name, next);
      return r ? r.insideScroller : null;
    })()`);
    check(titleNeg === true, `[G1/title] 否定對照：把標題暫時搬進 .projects 必須被判定為「在捲動容器內」（實際 ${titleNeg}）`);

    // --- [G1/tick] 刻度（design D8，使用者 2026-09-24 裁決）：每個 stage 欄首上緣一個 DOM 元素
    // （aria-hidden），不是偽元素；cockpit 只有 Implement 有 running task（be-1），只有那一格
    // 較長、用 --text；其他格不用冰青。否定對照放在下面「未知 status」注入之後（be-1 改成
    // whatever，cockpit 就沒有 running 了，Implement 那格必須跟著變回一般刻度——證明刻度是依
    // stage 資料畫的，不是寫死）。
    const TICK_PROBE = `function () {
      var headers = document.querySelectorAll('[data-region="floor"] .ff-stage-header');
      return Array.prototype.map.call(headers, function (h) {
        var ticks = h.querySelectorAll('.ff-stage-tick');
        var t = ticks[0];
        var hr = h.getBoundingClientRect();
        var tr = t ? t.getBoundingClientRect() : null;
        return {
          stage: h.getAttribute('data-stage'),
          tickCount: ticks.length,
          ariaHidden: t ? t.getAttribute('aria-hidden') : null,
          running: h.classList.contains('ff-stage-running'),
          height: tr ? tr.height : null,
          color: t ? getComputedStyle(t).backgroundColor : null,
          topGap: tr ? tr.top - hr.top : null,
          centerOffset: tr ? (tr.left + tr.width / 2) - (hr.left + hr.width / 2) : null,
          beforeContent: getComputedStyle(h, '::before').content,
        };
      });
    }`;
    const ticks = await cdp.eval(`(${TICK_PROBE})()`);
    const tickSummary = JSON.stringify(ticks);
    check(ticks.length === 3, `[G1/tick] cockpit 有 3 個 stage 欄首（實際 ${ticks.length}）`);
    check(
      ticks.every((t) => t.tickCount === 1 && t.ariaHidden === 'true' && t.beforeContent === 'none'),
      `[G1/tick] 每個欄首恰好一個 aria-hidden 的 .ff-stage-tick DOM 元素（不是偽元素；實際 ${tickSummary}）`
    );
    check(
      ticks.every((t) => t.topGap !== null && t.topGap <= 1.5 && Math.abs(t.centerOffset) <= 1),
      `[G1/tick] 刻度貼在欄首上緣（距上緣 ≤1.5px，含 1px 基線）、水平置中（實際 ${tickSummary}）`
    );
    const runTick = ticks.find((t) => t.stage === 'Implement');
    const idleTicks = ticks.filter((t) => t.stage !== 'Implement');
    check(
      runTick && runTick.running === true && idleTicks.every((t) => t.running === false),
      `[G1/tick] 只有 Implement（有 running task be-1）標成 ff-stage-running（實際 ${tickSummary}）`
    );
    check(
      runTick && runTick.color === SPEC_COLORS.text && idleTicks.every((t) => runTick.height > t.height && t.color !== SPEC_COLORS.accent),
      `[G1/tick] running 那格刻度較長且用 --text，其他格較短、都不用冰青（實際 ${tickSummary}）`
    );

    // --- [G1/binding] 2.1 設計審核 r4 Minor：runtime 名稱很長時只有 runtime 被省略，pane id
    // 永遠完整可見。注入 be 的綁定 runtime 為 60 字元長 id（pane 維持 wJ:p1）。
    const bindState = loadFixture();
    bindState.version = 71;
    const LONG_RT = 'runtime-' + 'R'.repeat(52);
    const beWs = bindState.projects.find((p) => p.id === 'cockpit').workstreams.find((w) => w.id === 'be');
    beWs.binding = { ...beWs.binding, runtime: LONG_RT };
    const bindInject = await injectState(cdp, bindState);
    check(bindInject && bindInject.ok === true, '[G1/binding] 注入 be 綁定到 60 字元長 runtime id 的特製投影');
    const BINDING_PROBE = `function () {
      var row = document.querySelector('.ff-row-header[data-workstream="be"]');
      var text = row && row.querySelector('.ff-binding-text');
      var rt = row && row.querySelector('.ff-binding-runtime');
      var pane = row && row.querySelector('.ff-binding-pane');
      if (!row || !text || !rt || !pane) return null;
      var rr = row.getBoundingClientRect();
      var tr = text.getBoundingClientRect();
      var pr = pane.getBoundingClientRect();
      var clip = getComputedStyle(text).overflow !== 'visible' ? tr.right : rr.right;
      return {
        text: text.textContent,
        paneText: pane.textContent,
        paneFont: getComputedStyle(pane).fontFamily,
        runtimeTruncated: rt.scrollWidth > rt.clientWidth + 1,
        paneVisible: pr.width > 0 && pr.left >= rr.left - 0.5 && pr.right <= Math.min(clip, rr.right) + 0.5,
        pane: { left: pr.left, right: pr.right }, clip: clip, row: { left: rr.left, right: rr.right },
      };
    }`;
    const binding = await cdp.eval(`(${BINDING_PROBE})()`);
    check(binding !== null, '[G1/binding] be 列首有 .ff-binding-text／.ff-binding-runtime／.ff-binding-pane');
    if (binding !== null) {
      check(binding.text === LONG_RT + ' / wJ:p1', `[G1/binding] .ff-binding-text 的文字仍是「runtime / pane」全文（實際 ${JSON.stringify(binding.text)}）`);
      check(binding.runtimeTruncated === true, `[G1/binding] 長 runtime 名稱被單行省略（實際 ${JSON.stringify(binding)}）`);
      check(binding.paneVisible === true && binding.paneText === 'wJ:p1', `[G1/binding] pane id 完整落在列首內、沒被省略號吃掉（實際 ${JSON.stringify(binding)}）`);
      check(binding.paneFont.indexOf('Cascadia Mono') !== -1, `[G1/binding] pane id 用等寬字（design D11；實際 ${binding.paneFont}）`);
    }
    // 否定對照：套回舊做法（整串 runtime / pane 同一個單行省略的 span）必須判定 pane id 不可見。
    const bindingNeg = await cdp.eval(`(() => {
      var text = document.querySelector('.ff-row-header[data-workstream="be"] .ff-binding-text');
      text.style.cssText = 'display:block;white-space:nowrap;overflow:hidden;text-overflow:ellipsis;min-width:0';
      var r = (${BINDING_PROBE})();
      text.style.cssText = '';
      return r;
    })()`);
    check(bindingNeg !== null && bindingNeg.paneVisible === false, `[G1/binding] 否定對照：整串單行省略時必須判定 pane id 被吃掉（實際 ${JSON.stringify(bindingNeg)}）`);
    // 還原成真推送的內容（同一個 fixture 版本），後面的 Scenario D 用正常投影。
    await cdp.eval('window.onState = window.__cockpitOrigOnState; true');

    // --- Scenario D 的畫面：fixture 的 project `p`（Scenario D Demo）已經逐字符合 GIVEN
    // （stages Plan/Implement/Test；backend/frontend/tests 各一個 running task，分別在
    // Implement/Plan/Test），不需要注入；task 3.1 起要先點左欄切過去。
    await cdp.eval('window.__cockpitOrigOnState(' + JSON.stringify({ ...loadFixture(), version: 72 }) + '); true');
    await cdp.click('[data-action="select-project"][data-project="p"]');
    await cdp.waitFor('!!document.querySelector(\'.project[data-project="p"]\')', 3000, '點左欄 p 之後 Factory Floor 顯示 Scenario D');
    const scenarioD = await cdp.eval(`(() => {
      var proj = document.querySelector('.project[data-project="p"]');
      if (!proj) return null;
      var headers = proj.querySelectorAll('.ff-stage-header');
      var rows = proj.querySelectorAll('.ff-row-header');
      function cellNode(ws, stage) {
        var cell = proj.querySelector('.ff-cell[data-workstream="' + ws + '"][data-stage="' + stage + '"]');
        var node = cell ? cell.querySelector('.task-node') : null;
        if (!node) return null;
        var cs = getComputedStyle(node);
        var label = node.querySelector('.task-status-label');
        return {
          status: label ? label.textContent : null,
          running: node.classList.contains('task-status-running'),
          labelColor: label ? getComputedStyle(label).color : null,
          borderWidths: [cs.borderTopWidth, cs.borderRightWidth, cs.borderBottomWidth].join(' '),
          borderColors: [cs.borderTopColor, cs.borderRightColor, cs.borderBottomColor, cs.borderLeftColor],
          boxShadow: cs.boxShadow,
        };
      }
      var nodes = proj.querySelectorAll('.task-node');
      return {
        stageCount: headers.length,
        rowCount: rows.length,
        nodeCount: nodes.length,
        title: (document.querySelector('[data-region="floor"] .project-name') || {}).textContent,
        backendImplement: cellNode('backend', 'Implement'),
        frontendPlan: cellNode('frontend', 'Plan'),
        testsTest: cellNode('tests', 'Test'),
      };
    })()`);
    check(scenarioD !== null, '找到 project `p` 的 Factory Floor 區塊（.project[data-project="p"]）');
    if (scenarioD !== null) {
      check(scenarioD.stageCount === 3, `Scenario D 應該有 3 欄（實際 ${scenarioD.stageCount}）`);
      check(scenarioD.rowCount === 3, `Scenario D 應該有 3 列（實際 ${scenarioD.rowCount}）`);
      check(scenarioD.nodeCount === 3, `Scenario D 應該恰好 3 個 task 節點（實際 ${scenarioD.nodeCount}）`);
      check(scenarioD.title === 'Scenario D Demo', `[G1/title] 切到 p 後主標題換成 p 的 name（實際 ${JSON.stringify(scenarioD.title)}）`);
      for (const [label, cell] of [
        ['backend×Implement', scenarioD.backendImplement],
        ['frontend×Plan', scenarioD.frontendPlan],
        ['tests×Test', scenarioD.testsTest],
      ]) {
        check(cell !== null && cell.running === true && cell.status === 'running', `${label} 應該有一個 running 的 task 節點（實際 ${JSON.stringify(cell)}）`);
        if (cell !== null) {
          check(
            cell.labelColor === SPEC_COLORS.accent,
            `${label} 的狀態文字應該是品牌強調色 #63D5E8（design D4；實際 ${cell.labelColor}）`
          );
          check(
            cell.borderWidths === '2px 2px 2px' && cell.borderColors.every((c) => c === SPEC_COLORS.accent) && cell.boxShadow !== 'none',
            `${label} 應該有強調外框：四邊 --accent、上右下 2px（左緣是同色 4px 色條）加靜止柔光（design D4；實際 ${JSON.stringify(cell)}）`
          );
        }
      }
    }
    const pTicks = await cdp.eval(`(${TICK_PROBE})()`);
    check(
      pTicks.length === 3 && pTicks.every((t) => t.running === true),
      `[G1/tick] Scenario D 三個 stage 都有 running task，三格刻度都是 running（實際 ${JSON.stringify(pTicks)}）`
    );
    await cdp.click('[data-action="select-project"][data-project="cockpit"]');
    await cdp.waitFor('!!document.querySelector(\'.project[data-project="cockpit"]\')', 3000, '切回 cockpit');

    // --- 未知 status 不破壞畫面（Factory Floor task status；注入 whatever）---
    const unknownState = craftUnknownTaskStatusState();
    const injectResult = await injectState(cdp, unknownState);
    check(injectResult && injectResult.ok === true, '注入 be-1.status = "whatever" 的特製投影');
    await cdp.waitFor(
      "document.body.textContent.indexOf('whatever') !== -1",
      5000,
      '未知 status 文字已經畫出'
    );
    const unknown = await cdp.eval(`(() => {
      var cell = document.querySelector('.project[data-project="cockpit"] .ff-cell[data-workstream="be"][data-stage="Implement"]');
      var node = cell ? cell.querySelector('.task-node') : null;
      var other = document.querySelector('.project[data-project="cockpit"] .ff-cell[data-workstream="ops"][data-stage="Review"] .task-node');
      var pending = document.querySelector('.project[data-project="cockpit"] .ff-cell[data-workstream="docs"][data-stage="Spec"] .task-node');
      function info(n) {
        if (!n) return null;
        var cs = getComputedStyle(n);
        var label = n.querySelector('.task-status-label');
        var sym = n.querySelector('.task-status-symbol');
        var t = n.querySelector('.task-title');
        return {
          text: n.textContent,
          title: n.getAttribute('title'),
          color: cs.color,
          labelText: label ? label.textContent : null,
          labelColor: label ? getComputedStyle(label).color : null,
          symbol: sym ? sym.textContent : null,
          symbolHidden: sym ? sym.getAttribute('aria-hidden') : null,
          titleColor: t ? getComputedStyle(t).color : null,
          borderStyle: cs.borderStyle,
          borderColors: [cs.borderTopColor, cs.borderRightColor, cs.borderBottomColor, cs.borderLeftColor],
          borderWidths: [cs.borderTopWidth, cs.borderRightWidth, cs.borderBottomWidth, cs.borderLeftWidth].map(parseFloat),
          labelContrast: label ? window.__cockpitVisualTools.textContrast(label).ratio : null,
          borderVsSurface: window.__cockpitVisualTools.contrastRatio(window.__cockpitVisualTools.parseColor(cs.borderTopColor), window.__cockpitVisualTools.parseColor(cs.backgroundColor)),
        };
      }
      return { unknown: info(node), other: info(other), pending: info(pending) };
    })()`);
    const u = unknown.unknown;
    check(u !== null && u.labelText === 'whatever' && u.title === 'whatever', `未知 status 節點應該顯示原字串 whatever（status 文字與 title 屬性；實際 ${JSON.stringify(u)}）`);
    check(
      u !== null && u.color === SPEC_COLORS.textDim && u.labelColor === SPEC_COLORS.textDim,
      `未知 status 節點應該用次要文字色 #A3B7C9（節點與 status 文字；design D4；實際 ${u ? u.color + '／' + u.labelColor : 'n/a'}）`
    );
    check(
      u !== null && u.borderStyle === 'dashed' && u.borderColors.every((c) => c === SPEC_COLORS.textDim) && u.borderVsSurface >= 3,
      `未知 status 節點應該有 --text-dim 虛線外框（四邊，對節點底色 ≥3:1；design D4；實際 ${u ? JSON.stringify({ s: u.borderStyle, c: u.borderColors, r: u.borderVsSurface }) : 'n/a'}）`
    );
    // task 3.2 fix round 1（Codex (1)）：只驗 style 與顏色時，四邊寬度被改成 0（外框完全看不見）
    // 仍會通過。補驗四邊寬度 > 0、左側狀態條 ≥3px（與 [G1/node] 同一個設計下限），否定對照：暫時把
    // border-width 設成 0 必須判定失敗。
    const unknownFrameOk = (x) => x !== null && x.borderWidths.slice(0, 3).every((w) => w > 0) && x.borderWidths[3] >= 3;
    check(unknownFrameOk(u), `[G1/unknown] 未知 status 虛線外框四邊寬度 > 0、左側狀態條 ≥3px（實際 ${u && JSON.stringify(u.borderWidths)}）`);
    const unknownZero = await cdp.eval(`(() => {
      var n = document.querySelector('.project[data-project="cockpit"] .ff-cell[data-workstream="be"][data-stage="Implement"] .task-node');
      n.style.borderWidth = '0';
      var cs = getComputedStyle(n);
      var r = { borderStyle: cs.borderStyle, borderWidths: [cs.borderTopWidth, cs.borderRightWidth, cs.borderBottomWidth, cs.borderLeftWidth].map(parseFloat) };
      n.style.borderWidth = '';
      return r;
    })()`);
    check(
      unknownZero.borderStyle === 'dashed' && !unknownFrameOk(unknownZero),
      `[G1/unknown] 否定對照：border-width 設成 0 時 style 仍是 dashed，但寬度檢查必須判定失敗（實際 ${JSON.stringify(unknownZero)}）`
    );
    check(u !== null && u.symbol === '?' && u.symbolHidden === 'true', `[G1/unknown] 未知 status 的符號是 aria-hidden 的 ?（design D4；實際 ${u ? u.symbol : 'n/a'}）`);
    check(u !== null && u.labelContrast >= 4.5, `[G1/unknown] 未知 status 文字對比 ≥4.5:1（實際 ${u && u.labelContrast}）`);
    const pd = unknown.pending;
    check(
      pd !== null && u !== null && pd.borderStyle !== 'dashed' && pd.titleColor !== u.titleColor && pd.symbol !== u.symbol,
      `[G1/unknown] 未知 status 和 pending 分得開：pending 不是虛線框、標題亮度不同、符號不同（實際 pending=${JSON.stringify(pd)}）`
    );
    check(
      unknown.other !== null && unknown.other.text.indexOf('completed') !== -1,
      `其他節點（ops-1，completed）應該正常顯示、不受影響（實際 ${JSON.stringify(unknown.other)}）`
    );
    // [G1/tick] 否定對照：be-1 改成 whatever 之後 cockpit 沒有 running task，Implement 必須變回
    // 一般刻度。
    const ticksAfter = await cdp.eval(`(${TICK_PROBE})()`);
    check(
      ticksAfter.length === 3 && ticksAfter.every((t) => t.running === false) && runTick && ticksAfter.every((t) => t.height < runTick.height),
      `[G1/tick] 否定對照：沒有 running task 時三格都是一般刻度（刻度依 stage 資料畫，不是寫死；實際 ${JSON.stringify(ticksAfter)}）`
    );

    // --- [G1/sticky] design D3：10 個 stage、7 條 workstream（兩個方向都溢出），把 .projects
    // 捲到中段，列首貼在捲動容器左緣、欄首貼在上緣、左上角格兩個方向都貼住；三者都是不透明
    // --bg-base、列首右緣 1px --line；命中測試證明它們蓋在捲過去的格子上方；刻度仍跟欄位對齊。
    const stickyState = craftStickyFloorState();
    const stickyInject = await injectState(cdp, stickyState);
    check(stickyInject && stickyInject.ok === true, '[G1/sticky] 注入 10 stage／7 workstream 的特製投影');
    const STICKY_PROBE = `function () {
      var sc = document.querySelector('[data-region="floor"] > .projects');
      var corner = sc.querySelector('.ff-corner');
      var sr = sc.getBoundingClientRect();
      var inner = { left: sr.left + sc.clientLeft, top: sr.top + sc.clientTop };
      var rows = Array.prototype.filter.call(sc.querySelectorAll('.ff-row-header'), function (h) {
        var r = h.getBoundingClientRect(); return r.bottom > inner.top + 60 && r.top < sr.bottom - 10;
      });
      var heads = Array.prototype.filter.call(sc.querySelectorAll('.ff-stage-header'), function (h) {
        var r = h.getBoundingClientRect(); return r.left > inner.left + 250 && r.right < sr.right - 10;
      });
      function hitInside(el) {
        var r = el.getBoundingClientRect();
        var x = r.left + Math.min(r.width / 2, 40), y = r.top + r.height / 2;
        var hit = document.elementFromPoint(x, y);
        return !!hit && el.contains(hit);
      }
      function box(el) {
        var cs = getComputedStyle(el);
        var r = el.getBoundingClientRect();
        return { position: cs.position, bg: cs.backgroundColor, left: r.left, top: r.top, right: r.right };
      }
      var row = rows[0], head = heads[0];
      var tickAlign = heads.map(function (h) {
        var stage = h.getAttribute('data-stage');
        var cell = sc.querySelector('.ff-cell[data-stage="' + stage + '"]');
        var t = h.querySelector('.ff-stage-tick').getBoundingClientRect();
        var c = cell.getBoundingClientRect();
        var hr = h.getBoundingClientRect();
        var cx = t.left + t.width / 2;
        return { stage: stage, ok: Math.abs(hr.left - c.left) <= 1 && Math.abs(hr.right - c.right) <= 1 && cx > c.left && cx < c.right };
      });
      return {
        scrollLeft: sc.scrollLeft, scrollTop: sc.scrollTop, inner: inner,
        corner: box(corner), row: row ? box(row) : null, head: head ? box(head) : null,
        rowBorderRight: row ? getComputedStyle(row).borderRightWidth + ' ' + getComputedStyle(row).borderRightColor : null,
        rowHit: row ? hitInside(row) : null, headHit: head ? hitInside(head) : null, cornerHit: hitInside(corner),
        tickAlign: tickAlign,
      };
    }`;
    const sticky = await cdp.eval(`(() => {
      var sc = document.querySelector('[data-region="floor"] > .projects');
      sc.scrollLeft = Math.round((sc.scrollWidth - sc.clientWidth) / 2);
      sc.scrollTop = Math.round((sc.scrollHeight - sc.clientHeight) / 2);
      return { sw: sc.scrollWidth, cw: sc.clientWidth, sh: sc.scrollHeight, ch: sc.clientHeight, probe: (${STICKY_PROBE})() };
    })()`);
    const sp = sticky.probe;
    check(sticky.sw > sticky.cw && sticky.sh > sticky.ch && sp.scrollLeft > 0 && sp.scrollTop > 0, `[G1/sticky] .projects 兩個方向都溢出且已捲到中段（實際 ${JSON.stringify({ sw: sticky.sw, cw: sticky.cw, sh: sticky.sh, ch: sticky.ch, l: sp.scrollLeft, t: sp.scrollTop })}）`);
    check(sp.row !== null && sp.head !== null, '[G1/sticky] 捲動後可視範圍內找得到列首與欄首');
    if (sp.row !== null && sp.head !== null) {
      check(
        sp.row.position === 'sticky' && Math.abs(sp.row.left - sp.inner.left) <= 1,
        `[G1/sticky] 橫向捲動後列首仍貼在 .projects 左緣（position: sticky; left: 0；實際 ${JSON.stringify({ row: sp.row, inner: sp.inner })}）`
      );
      check(
        sp.head.position === 'sticky' && Math.abs(sp.head.top - sp.inner.top) <= 1,
        `[G1/sticky] 縱向捲動後欄首仍貼在 .projects 上緣（position: sticky; top: 0；實際 ${JSON.stringify({ head: sp.head, inner: sp.inner })}）`
      );
      check(
        Math.abs(sp.corner.left - sp.inner.left) <= 1 && Math.abs(sp.corner.top - sp.inner.top) <= 1,
        `[G1/sticky] 左上角格兩個方向都貼住（實際 ${JSON.stringify({ corner: sp.corner, inner: sp.inner })}）`
      );
      check(
        [sp.row.bg, sp.head.bg, sp.corner.bg].every((c) => c === SPEC_COLORS.bgBase),
        `[G1/sticky] 列首、欄首、左上角格都是不透明 --bg-base（實際 ${[sp.row.bg, sp.head.bg, sp.corner.bg].join('／')}）`
      );
      check(sp.rowBorderRight === `1px ${SPEC_COLORS.line}`, `[G1/sticky] 列首右緣 1px --line（實際 ${sp.rowBorderRight}）`);
      check(sp.rowHit === true && sp.headHit === true && sp.cornerHit === true, `[G1/sticky] 命中測試：列首、欄首、左上角格蓋在捲過去的格子上方（實際 row=${sp.rowHit} head=${sp.headHit} corner=${sp.cornerHit}）`);
      check(
        sp.tickAlign.length >= 2 && sp.tickAlign.every((t) => t.ok),
        `[G1/sticky] 橫向捲動後每個可見欄首與下方同一 stage 的格子左右對齊、刻度落在該欄範圍內（實際 ${JSON.stringify(sp.tickAlign)}）`
      );
    }
    // 否定對照：列首與欄首改回 position: static（同一個捲動位置）必須判定沒有貼住。
    const stickyNeg = await cdp.eval(`(() => {
      var style = document.createElement('style');
      style.textContent = '.ff-row-header, .ff-stage-header, .ff-corner { position: static !important; }';
      document.head.appendChild(style);
      var r = (${STICKY_PROBE})();
      style.remove();
      return r;
    })()`);
    check(
      stickyNeg.row !== null && Math.abs(stickyNeg.row.left - stickyNeg.inner.left) > 1,
      `[G1/sticky] 否定對照：列首改成 static 後同一捲動位置不應該再貼在左緣（實際 ${JSON.stringify(stickyNeg.row)}）`
    );
    check(
      stickyNeg.head !== null && Math.abs(stickyNeg.head.top - stickyNeg.inner.top) > 1,
      `[G1/sticky] 否定對照：欄首改成 static 後同一捲動位置不應該再貼在上緣（實際 ${JSON.stringify(stickyNeg.head)}）`
    );
    // sticky 列首／欄首不得蓋住被捲進視野的目標：scrollIntoView（block／inline 都是 start，
    // 最壞情況——目標被捲到捲動容器的左上角）之後，按鈕中心點的命中測試仍是按鈕本身
    // （.projects 的 scroll-padding 讓開欄首高度與列首寬度；鍵盤焦點與 cdp.click 的
    // scrollIntoView 都靠這個）。否定對照：scroll-padding 歸零必須讓命中測試失敗。
    const SCROLL_HIT = `function () {
      var btn = document.querySelector('[data-action="advance"][data-project="cockpit"][data-task="qa-1"]');
      if (!btn) return null;
      btn.scrollIntoView({ block: 'start', inline: 'start' });
      var r = btn.getBoundingClientRect();
      var hit = document.elementFromPoint(r.left + r.width / 2, r.top + r.height / 2);
      return { hitSelf: !!hit && btn.contains(hit), hit: hit ? hit.className : null };
    }`;
    const scrollHit = await cdp.eval(`(${SCROLL_HIT})()`);
    const scrollHitNeg = await cdp.eval(`(() => {
      var sc = document.querySelector('[data-region="floor"] > .projects');
      var saved = sc.style.cssText;
      sc.style.scrollPadding = '0px';
      sc.scrollLeft = 0; sc.scrollTop = 0;
      var r = (${SCROLL_HIT})();
      sc.style.cssText = saved;
      return r;
    })()`);
    check(scrollHit !== null && scrollHit.hitSelf === true, `[G1/sticky] scrollIntoView 到左上角後，qa-1 的「推進」按鈕沒有被列首／欄首蓋住（實際 ${JSON.stringify(scrollHit)}）`);
    check(scrollHitNeg !== null && scrollHitNeg.hitSelf === false, `[G1/sticky] 否定對照：scroll-padding 歸零時同一個按鈕必須被列首或欄首蓋住（實際 ${JSON.stringify(scrollHitNeg)}）`);
    // task 3.2 fix round 1（Codex (2)）：上面只驗了程式化的 scrollIntoView。真實鍵盤路徑：先用
    // scrollLeft／scrollTop 把 qa-1 的「推進」按鈕放到 sticky 列首（或欄首）底下——幾何上仍在
    // .projects 的 scrollport 內，只是被 sticky 蓋住——再用 CDP 送 Tab／Shift+Tab（或呼叫
    // focus()）把焦點移到它。斷言：activeElement 是它、中心點命中它自己、整顆按鈕落在「扣掉
    // 列首欄與欄首列之後」的可視矩形內。否定對照：scroll-padding 歸零後同一流程至少有一種情境
    // 會停在 sticky 底下（證明這組斷言能失敗）。
    const KBD_SETUP = `function (mode, from) {
      var sc = document.querySelector('[data-region="floor"] > .projects');
      var target = document.querySelector('[data-action="advance"][data-project="cockpit"][data-task="qa-1"]');
      var buttons = Array.prototype.slice.call(document.querySelectorAll('#app button'));
      var idx = buttons.indexOf(target);
      var start = from === 'prev' ? buttons[idx - 1] : from === 'next' ? buttons[idx + 1] : null;
      if (start) start.focus({ preventScroll: true }); else if (document.activeElement) document.activeElement.blur();
      var corner = sc.querySelector('.ff-corner').getBoundingClientRect();
      var sr = sc.getBoundingClientRect();
      var inner = { left: sr.left + sc.clientLeft, top: sr.top + sc.clientTop };
      var r = target.getBoundingClientRect();
      var contentLeft = r.left - inner.left + sc.scrollLeft;
      var contentTop = r.top - inner.top + sc.scrollTop;
      var headerW = corner.right - inner.left;
      var headerH = corner.bottom - inner.top;
      if (mode === 'under-row-header') {
        sc.scrollLeft = contentLeft - 20;
        sc.scrollTop = contentTop - headerH - 40;
      } else {
        sc.scrollLeft = contentLeft - headerW - 40;
        sc.scrollTop = contentTop - 10;
      }
      var r2 = target.getBoundingClientRect();
      var cov = document.elementFromPoint(r2.left + r2.width / 2, r2.top + r2.height / 2);
      return {
        started: start ? document.activeElement === start : true,
        covered: !cov || !target.contains(cov),
        inScrollport: r2.left >= inner.left && r2.top >= inner.top && r2.right <= inner.left + sc.clientWidth && r2.bottom <= inner.top + sc.clientHeight,
      };
    }`;
    const KBD_CHECK = `function () {
      var sc = document.querySelector('[data-region="floor"] > .projects');
      var target = document.querySelector('[data-action="advance"][data-project="cockpit"][data-task="qa-1"]');
      var corner = sc.querySelector('.ff-corner').getBoundingClientRect();
      var sr = sc.getBoundingClientRect();
      var inner = { left: sr.left + sc.clientLeft, top: sr.top + sc.clientTop };
      var vis = { left: corner.right, top: corner.bottom, right: inner.left + sc.clientWidth, bottom: inner.top + sc.clientHeight };
      var a = document.activeElement;
      var r = target.getBoundingClientRect();
      var hit = document.elementFromPoint(r.left + r.width / 2, r.top + r.height / 2);
      return {
        focused: a === target,
        hitSelf: !!hit && target.contains(hit),
        inside: r.left >= vis.left - 0.5 && r.top >= vis.top - 0.5 && r.right <= vis.right + 0.5 && r.bottom <= vis.bottom + 0.5,
        rect: { l: r.left, t: r.top, r: r.right, b: r.bottom }, vis: vis,
      };
    }`;
    async function keyboardCase(mode, how) {
      const from = how === 'Tab' ? 'prev' : how === 'Shift+Tab' ? 'next' : null;
      const setup = await cdp.eval(`(${KBD_SETUP})(${JSON.stringify(mode)}, ${JSON.stringify(from)})`);
      if (how === 'focus()') {
        await cdp.eval(`document.querySelector('[data-action="advance"][data-project="cockpit"][data-task="qa-1"]').focus(); true`);
      } else {
        const modifiers = how === 'Shift+Tab' ? 8 : 0;
        await cdp.send('Input.dispatchKeyEvent', { type: 'keyDown', key: 'Tab', code: 'Tab', windowsVirtualKeyCode: 9, modifiers });
        await cdp.send('Input.dispatchKeyEvent', { type: 'keyUp', key: 'Tab', code: 'Tab', windowsVirtualKeyCode: 9, modifiers });
      }
      await sleep(100);
      const after = await cdp.eval(`(${KBD_CHECK})()`);
      return { mode, how, setup, after };
    }
    const kbdResults = [];
    for (const mode of ['under-row-header', 'under-stage-header']) {
      for (const how of ['Tab', 'Shift+Tab', 'focus()']) {
        const r = await keyboardCase(mode, how);
        kbdResults.push(r);
        check(
          r.setup.started && r.setup.covered && r.setup.inScrollport,
          `[G1/sticky-kbd] 前置（${mode}／${how}）：起點焦點就位，qa-1「推進」在 scrollport 內但被 sticky 蓋住（實際 ${JSON.stringify(r.setup)}）`
        );
        check(
          r.after.focused && r.after.hitSelf && r.after.inside,
          `[G1/sticky-kbd] ${how} 把焦點移到被${mode === 'under-row-header' ? '列首' : '欄首'}蓋住的按鈕後，它完整落在扣掉 sticky 區域的可視範圍內、中心點命中自己（實際 ${JSON.stringify(r.after)}）`
        );
      }
    }
    // 實測（Chrome）：Tab／Shift+Tab／focus() 的「捲到看得見」都遵守 .projects 的 scroll-padding，
    // 產品不需要另外加 focusin 修正。否定對照：scroll-padding 歸零之後，同一組流程至少一種情境焦點
    // 停在 sticky 底下。
    const kbdNeg = [];
    await cdp.eval(`(() => {
      var sc = document.querySelector('[data-region="floor"] > .projects');
      window.__g1SavedScCss = sc.style.cssText;
      sc.style.scrollPadding = '0px';
      return true;
    })()`);
    for (const mode of ['under-row-header', 'under-stage-header']) {
      for (const how of ['Tab', 'Shift+Tab', 'focus()']) kbdNeg.push(await keyboardCase(mode, how));
    }
    await cdp.eval(`(() => {
      var sc = document.querySelector('[data-region="floor"] > .projects');
      sc.style.cssText = window.__g1SavedScCss;
      return true;
    })()`);
    check(
      kbdNeg.some((r) => r.after.focused && !(r.after.hitSelf && r.after.inside)),
      `[G1/sticky-kbd] 否定對照：scroll-padding 歸零後，至少一種情境焦點停在 sticky 底下（實際 ${JSON.stringify(kbdNeg.map((r) => ({ m: r.mode, h: r.how, f: r.after.focused, hit: r.after.hitSelf, in: r.after.inside })))}）`
    );
  } finally {
    await stopChrome(chrome, 'chrome-G1');
    await stopPreview(preview, 'preview-G1');
  }
}

// ---------------------------------------------------------------------------
// G2：dashboard/running 節點沒有動畫（Factory Floor 本文；task 2.2）。fix round 1／規格檢查
// Important：從舊版 G1 拆出，可以獨立於 G1（task 3.2 的範圍）選跑，`node visual-check.js G2`
// 就能只驗 task 2.2 這一條，不用等 task 3.2 的符號／顏色斷言一起跑完再從一堆輸出裡挑行。
// 不牽涉 prefers-reduced-motion（那是 Direction 01 視覺語彙的「減少動態」，見 RM1）。
// ---------------------------------------------------------------------------

async function partFactoryFloorAnimation() {
  log('=== G2. Factory Floor（task 2.2）：running 節點沒有動畫＋節點按鈕對比 ===');
  let preview = null;
  let chrome = null;
  try {
    preview = await startPreview({}, 'preview-G2');
    const url = `http://127.0.0.1:${preview.port}/`;
    chrome = await startChrome(pickPort(19065, [preview.port]), url, 'chrome-G2');
    const { cdp } = chrome;
    await waitForFirstProjection(cdp, preview.port);

    const anim = await cdp.eval(`(() => {
      var node = document.querySelector('.task-status-running');
      if (!node) return null;
      var cs = getComputedStyle(node);
      var before = getComputedStyle(node, '::before');
      var after = getComputedStyle(node, '::after');
      return { animationName: cs.animationName, beforeAnim: before.animationName, afterAnim: after.animationName };
    })()`);
    check(anim !== null, '找到一個 running 的 task 節點（.task-status-running）');
    if (anim !== null) {
      check(anim.animationName === 'none', `running 節點的 animation-name 應該是 none（實際 ${anim.animationName}）`);
      check(anim.beforeAnim === 'none', `running 節點的 ::before animation-name 應該是 none（實際 ${anim.beforeAnim}）`);
      check(anim.afterAnim === 'none', `running 節點的 ::after animation-name 應該是 none（實際 ${anim.afterAnim}）`);
    }

    // fix round 2／設計複審 N1（Important，2.2 現在補救）：Factory Floor 節點底色（fix
    // round 1）改用核心 token 後全是中高亮度色，.action-button 依 D4 用 --text-dim 文字／
    // 外框，跟節點底色幾乎同亮度——pending 節點的按鈕量到 1.00:1（完全隱形）。style.css 當時
    // 補了一條暫時規則（.task-node .action-button 改用 --bg-deep），direction-01-visual task
    // 3.2 節點換成 --surface 底後已刪除，斷言保留：這裡逐一走訪目前投影裡每個節點上的按鈕，斷言文字對其「實際背景」（節點底色，不
    // 是按鈕自己的 transparent 背景——window.__cockpitVisualTools.textContrast 會自動往上找
    // 最近一層不透明背景）的對比都 ≥4.5:1，防止之後有人改動節點底色或按鈕樣式時
    // 讓按鈕再度看不見。
    await installTools(cdp);
    const buttonContrasts = await cdp.eval(`(() => {
      var buttons = document.querySelectorAll('.task-node .action-button');
      var out = [];
      for (var i = 0; i < buttons.length; i += 1) {
        var btn = buttons[i];
        var r = window.__cockpitVisualTools.textContrast(btn);
        var node = btn.closest('.task-node');
        out.push({
          label: btn.textContent.trim(),
          nodeStatus: node ? Array.prototype.find.call(node.classList, function (c) { return c.indexOf('task-status-') === 0; }) : null,
          ratio: r.ratio,
        });
      }
      return out;
    })()`);
    check(buttonContrasts.length > 0, `找到至少一個節點上的按鈕可以量對比（實際 ${buttonContrasts.length} 顆）`);
    for (const b of buttonContrasts) {
      check(
        b.ratio >= 4.5,
        `節點按鈕「${b.label}」（${b.nodeStatus}）文字對實際背景的對比應該 ≥4.5:1（實際 ${b.ratio.toFixed(2)}:1）`
      );
    }

    // fix round 3／設計複審（上一輪就存在的問題，這輪量測才發現，P1）：running 節點底色是
    // --accent，:focus-visible 的焦點外框（design D4「焦點」全域規則）也是 --accent，兩者
    // 對比 1:1——鍵盤把焦點移到 running 節點按鈕上時，外框看不見。style.css 補了
    // `.task-node .action-button:focus-visible { outline-color: var(--bg-deep); }`（同一段
    // 暫時規則，task 3.2 換皮時已刪除，節點底色改成 --surface 後冰青外框本身就 ≥3:1），這裡驗證聚焦後外框色對節點底色的對比 ≥3:1（WCAG
    // 「必要圖形」的最低門檻，design D4「Direction 01 視覺語彙」同一個數字）。
    const runningFocusContrast = await cdp.eval(`(() => {
      var node = document.querySelector('.task-status-running');
      if (!node) return null;
      var btn = node.querySelector('.action-button');
      if (!btn) return null;
      btn.focus();
      var tools = window.__cockpitVisualTools;
      var nodeBg = tools.effectiveBackground(node);
      var outlineColor = tools.parseColor(getComputedStyle(btn).outlineColor);
      return {
        focused: document.activeElement === btn,
        focusVisible: btn.matches(':focus-visible'),
        outlineStyle: getComputedStyle(btn).outlineStyle,
        outlineWidth: getComputedStyle(btn).outlineWidth,
        ratio: tools.contrastRatio(outlineColor, nodeBg),
        outlineColor: getComputedStyle(btn).outlineColor,
        nodeBg: getComputedStyle(node).backgroundColor,
      };
    })()`);
    check(runningFocusContrast !== null, '找到 running 節點上的按鈕可以測焦點外框對比');
    if (runningFocusContrast !== null) {
      check(runningFocusContrast.focused === true, 'running 節點的按鈕應該成功取得鍵盤焦點（document.activeElement）');
      // direction-01-visual task 3.2：暫時規則刪除後，沒有 :focus-visible 時 outline-color 會退回
      // currentColor（--text-dim），對 --surface 也 ≥3:1——只量顏色會變成恆真。另外斷言按鈕真的
      // 處於 :focus-visible、外框是 2px solid，量到的才是焦點外框本身。
      check(
        runningFocusContrast.focusVisible === true && runningFocusContrast.outlineStyle === 'solid' && runningFocusContrast.outlineWidth === '2px',
        `running 節點按鈕聚焦後應該符合 :focus-visible 並畫出 2px solid 外框（實際 ${JSON.stringify(runningFocusContrast)}）`
      );
      check(
        runningFocusContrast.ratio >= 3,
        `running 節點按鈕 :focus-visible 時，外框色（${runningFocusContrast.outlineColor}）對節點底色（${runningFocusContrast.nodeBg}）的對比應該 ≥3:1（實際 ${runningFocusContrast.ratio.toFixed(2)}:1）`
      );
    }
  } finally {
    await stopChrome(chrome, 'chrome-G2');
    await stopPreview(preview, 'preview-G2');
  }
}

// ---------------------------------------------------------------------------
// P1：dashboard/切換Project＋選取跨重畫保留＋鍵盤切換與焦點保留＋切換Project不清除錯誤也不
// 離開改綁模式＋各狀態數量＋沒有Project＋兩個Project（task 3.1）
// ---------------------------------------------------------------------------

async function partProjectSwitching() {
  log('=== P1. Project 切換：切換／持久化／鍵盤／不清錯誤不離開改綁／各狀態數量／沒有Project／兩個Project ===');
  let preview = null;
  let chrome = null;
  try {
    // fix round 1／Codex finding（3）：延遲從 100ms 拉長到 1500ms，給「in-flight 寫入失敗時
    // 切換 Project」情境足夠的時間窗——cdp.click() 本身有 scrollIntoView 後的 100ms 停頓＋
    // 重試邏輯，兩次連續 click()（先按「Failed」、緊接著切換 Project）之間可能就消耗了
    // 100–300ms，100ms 的延遲不夠可靠地保證「切換發生在回應抵達之前」；1500ms 對照既有的
    // 「切換 Project 不清除錯誤也不離開改綁模式」情境（等錯誤訊息用 3000ms 逾時）仍然充裕。
    preview = await startPreview(
      {
        COCKPIT_PREVIEW_PUSH_MS: '100',
        COCKPIT_PREVIEW_WRITE_RULES: '/api/projects/cockpit/tasks/be-2/fail=1500:409',
      },
      'preview-P1'
    );
    const url = `http://127.0.0.1:${preview.port}/`;
    chrome = await startChrome(pickPort(19070, [preview.port]), url, 'chrome-P1');
    const { cdp } = chrome;
    await waitForFirstProjection(cdp, preview.port);

    // --- 兩個 Project（GIVEN 尚未選定任何 Project；fixture 已有 cockpit／p 兩個 Project）---
    const twoProjects = await cdp.eval(`(() => {
      var floors = document.querySelectorAll('.factory-floor');
      var projectSections = document.querySelectorAll('.project');
      var leftColumn = document.querySelector('[data-region="projects"]');
      return {
        floorCount: floors.length,
        projectSectionCount: projectSections.length,
        firstProjectShown: projectSections.length > 0 ? projectSections[0].getAttribute('data-project') : null,
        hasLeftColumn: !!leftColumn,
      };
    })()`);
    check(
      twoProjects.floorCount === 1,
      `中上區域應該只有一張 Factory Floor（spec「兩個 Project」：另一個的網格不在畫面上、改由左欄切換；實際 ${twoProjects.floorCount} 張）`
    );
    check(
      twoProjects.firstProjectShown === 'cockpit',
      `未選定過時應該預設顯示第一個 Project（cockpit；實際 ${twoProjects.firstProjectShown}）`
    );
    check(
      twoProjects.hasLeftColumn === true,
      '應該有左欄（data-region="projects"）列出 Project 供切換（design D2／D6）'
    );

    // --- 切換 Project（點左欄的 p）---
    const clicked = await cdp.click('[data-action="select-project"][data-project="p"]');
    if (clicked) {
      await cdp.waitFor(
        '!!document.querySelector(\'.project[data-project="p"]\')',
        2000,
        '點選後 Factory Floor 顯示 p（Scenario D Demo）'
      );
    } else {
      check(false, '沒有找到可點的 Project 項目（data-action="select-project"；design D6 迴歸）');
    }

    // --- 選取跨重畫保留（dashboard；此時推送間隔 100ms）---
    // fix round 1／Codex F2：不是固定睡 3 秒才看最後一眼，改成觀察 #version 真的變化至少兩次
    // （證明期間真的發生了規格要求的整頁重畫，不是伺服器剛好沒推送），每次變化後立刻斷言
    // Factory Floor 是否仍顯示 p。
    await waitForRepaintsAssertingEachTime(
      cdp,
      2,
      4000,
      async (n, v) => {
        const stillP = await cdp.eval('!!document.querySelector(\'.project[data-project="p"]\')');
        check(stillP, `選取跨重畫保留：第 ${n} 次觀察到重畫（version=${v}）後 Factory Floor 仍應該顯示 p`);
      },
      '選取跨重畫保留'
    );

    // --- 鍵盤切換與焦點保留（焦點在 cockpit 項目上，按 Enter 應該切回去）---
    const focusable = await cdp.eval(`(() => {
      var n = document.querySelector('[data-action="select-project"][data-project="cockpit"]');
      if (!n) return false;
      n.focus();
      return document.activeElement === n;
    })()`);
    if (focusable) {
      // fix round 1／Codex F2：同上，觀察 #version 真的變化取代固定睡 2 秒，每次變化後立刻
      // 確認焦點仍在 cockpit 項目上。
      await waitForRepaintsAssertingEachTime(
        cdp,
        2,
        4000,
        async (n, v) => {
          const stillFocused = await cdp.eval(
            '!!(document.activeElement && document.activeElement.dataset && document.activeElement.dataset.project === "cockpit" && document.activeElement.dataset.action === "select-project")'
          );
          check(stillFocused, `鍵盤切換與焦點保留：第 ${n} 次觀察到重畫（version=${v}）後焦點仍應該在 cockpit 項目上`);
        },
        '鍵盤切換與焦點保留'
      );
      await cdp.pressKey('Enter', 'Enter', 13, '\r');
      await cdp.waitFor(
        '!!document.querySelector(\'.project[data-project="cockpit"]\') && document.querySelectorAll(\'.factory-floor\').length === 1',
        2000,
        'Enter 後 Factory Floor 改顯示 cockpit'
      );
    } else {
      check(false, '找不到可聚焦的 cockpit Project 項目（design D6 迴歸）');
    }

    // --- 切換 Project 不清除錯誤也不離開改綁模式 ---
    // 順序很重要：先進改綁模式，再觸發會失敗的寫入（be-2 的「Failed」按鈕，preview 已設
    // write-rule 回 409）——actions.js 的 perform() 對任何一般「畫面操作」（含 rebind 本身）
    // 都會先把 ui.error 清成 null（spec cockpit-dashboard「畫面操作」：「清掉上一次的錯誤訊息」；
    // 這是現行已知、正確的行為，不是 bug）。若順序反過來（先觸發錯誤、再點「改綁」），「改綁」
    // 這個動作本身就會把剛顯示的錯誤清掉，根本走不到「切換 Project」這一步就已經沒有錯誤可驗。
    await cdp.click('[data-action="rebind"][data-project="cockpit"][data-workstream="ops"]');
    await cdp.waitFor("!!document.querySelector('.rebind-banner')", 2000, '進入改綁模式（目標 ops）');
    await cdp.click('[data-action="fail"][data-project="cockpit"][data-task="be-2"]');
    await cdp.waitFor("!!document.querySelector('.error-banner')", 3000, '改綁模式期間觸發操作錯誤訊息（be-2 fail 409）');
    check(
      await cdp.eval("!!document.querySelector('.rebind-banner')"),
      '觸發錯誤後仍在改綁模式（rebind-banner 仍在，回歸檢查）'
    );
    const switchedDuring = await cdp.click('[data-action="select-project"][data-project="p"]');
    if (!switchedDuring) {
      check(false, '切換 Project 的入口不存在，無法實際驗證「切換後錯誤與改綁是否保留」（design D6 迴歸）');
    }
    await sleep(300);
    const stillThere = await cdp.eval(`(() => ({
      hasError: !!document.querySelector('.error-banner'),
      hasRebind: !!document.querySelector('.rebind-banner'),
    }))()`);
    check(
      stillThere.hasError === true,
      `切換 Project 後錯誤訊息仍應該在（實際 ${JSON.stringify(stillThere)}）`
    );
    check(
      stillThere.hasRebind === true,
      `切換 Project 後改綁提示仍應該在（實際 ${JSON.stringify(stillThere)}）`
    );
    // 收尾（不是斷言，下面的情境會重新進入改綁模式，這裡只是為了不留上一段的改綁模式在頁面
    // 上）：rebind-cancel 本身也是一般「畫面操作」，會先清掉 ui.error（跟上面「進改綁模式時
    // 一般操作會清錯誤」同一個機制）——error-banner 這時多半已經因此消失，只有還在時才需要再
    // 點一次 error-dismiss，不然找不到元素會誤印一條無意義的 FAIL。
    await cdp.click('[data-action="rebind-cancel"]');
    if (await cdp.eval("!!document.querySelector('.error-banner')")) {
      await cdp.click('[data-action="error-dismiss"]');
    }
    await cdp.click('[data-action="select-project"][data-project="cockpit"]');
    await cdp.waitFor(
      '!!document.querySelector(\'.project[data-project="cockpit"]\')',
      2000,
      '切回 cockpit（上一段結束後）'
    );

    // --- in-flight 寫入失敗時切換 Project（fix round 1／Codex finding 3）---
    // 上一段「切換 Project 不清除錯誤也不離開改綁模式」是在錯誤訊息已經顯示之後才切換
    // Project，只證明了「切換不會清除已經顯示的錯誤」；沒有證明 select-project 真的不會讓
    // `latestOp` 往前推、把還在路上（尚未回應）的失敗悄悄吞掉（design D6；R19）——即使
    // select-project 錯誤地遞增了 `latestOp`，上一段的斷言方式一樣會通過，因為它完全沒有碰到
    // `showError()` 那個「op !== latestOp 就忽略」的分支。改成：先進改綁模式、按下「Failed」
    // 觸發一筆延遲 1500ms 才回 409 的寫入，**不等回應**、立刻切換 Project，等延遲的回應真的
    // 抵達之後再斷言：錯誤訊息出現（代表 `showError()` 判斷 op === latestOp，`select-project`
    // 沒有偷偷把 `latestOp` 推走）、改綁模式仍在。
    log('--- 情境「in-flight 寫入失敗時切換 Project」（送出後、回應抵達前就切換，驗證 select-project 沒有遞增 latestOp）---');
    await cdp.click('[data-action="rebind"][data-project="cockpit"][data-workstream="ops"]');
    await cdp.waitFor("!!document.querySelector('.rebind-banner')", 2000, '再次進入改綁模式（in-flight 情境）');
    await cdp.click('[data-action="fail"][data-project="cockpit"][data-task="be-2"]');
    // 刻意不等待、立刻切換——這是這個情境要驗的關鍵時序（切換發生在延遲的 409 回應抵達之前）。
    const switchedInFlight = await cdp.click('[data-action="select-project"][data-project="p"]');
    check(switchedInFlight, '應該能在寫入回應抵達前就切換 Project（找到左欄的 p 項目）');
    await cdp.waitFor(
      '!!document.querySelector(\'.project[data-project="p"]\')',
      2000,
      '切換當下 Factory Floor 應該已經顯示 p（不等寫入回應）'
    );
    await cdp.waitFor(
      "!!document.querySelector('.error-banner')",
      3000,
      '延遲的 409 回應抵達後，切換 Project 之後仍應該顯示錯誤訊息（select-project 沒有遞增 latestOp 把它吞掉）'
    );
    const afterInFlight = await cdp.eval(`(() => ({
      hasRebind: !!document.querySelector('.rebind-banner'),
      errorText: (document.querySelector('.error-banner') || {}).textContent || null,
    }))()`);
    check(
      afterInFlight.hasRebind === true,
      `in-flight 失敗的回應抵達之後，改綁模式仍應該在（實際 ${JSON.stringify(afterInFlight)}）`
    );
    check(
      typeof afterInFlight.errorText === 'string' && afterInFlight.errorText.indexOf('409') !== -1,
      `錯誤訊息應該含 HTTP 409（實際 ${JSON.stringify(afterInFlight.errorText)}）`
    );
    // 收尾，同上一段。
    await cdp.click('[data-action="rebind-cancel"]');
    if (await cdp.eval("!!document.querySelector('.error-banner')")) {
      await cdp.click('[data-action="error-dismiss"]');
    }
    await cdp.click('[data-action="select-project"][data-project="cockpit"]');
    await cdp.waitFor(
      '!!document.querySelector(\'.project[data-project="cockpit"]\')',
      2000,
      '切回 cockpit（供後續各狀態數量／沒有 Project 情境使用）'
    );

    // --- 各狀態數量（注入 crafted state：2 個 running、1 個 completed、1 個 failed、1 則
    // warning）---
    const countState = craftStatusCountState();
    const injectResult = await injectState(cdp, countState);
    check(injectResult && injectResult.ok === true, '注入各狀態數量的特製投影');
    await sleep(300);
    const counts = await cdp.eval(`(() => {
      var col = document.querySelector('[data-region="projects"]');
      if (!col) return null;
      var text = col.textContent;
      return {
        text,
        hasRunning2: /running[^0-9]*2/.test(text),
        hasCompleted1: /completed[^0-9]*1/.test(text),
        hasFailed1: /failed[^0-9]*1/.test(text),
        hasWarning: text.indexOf('warning') !== -1 || /\\u8b66\\u544a/.test(text),
        noComplete: text.indexOf('完成') === -1,
      };
    })()`);
    check(counts !== null, '找到左欄（data-region="projects"）以核對各狀態數量');
    if (counts !== null) {
      check(counts.hasRunning2, `左欄 cockpit 項目應該顯示 running 2（實際文字片段見上）`);
      check(counts.hasCompleted1, `左欄 cockpit 項目應該顯示 completed 1`);
      check(counts.hasFailed1, `左欄 cockpit 項目應該顯示 failed 1`);
      check(counts.hasWarning, `左欄 cockpit 項目應該顯示 1 則 warning`);
      check(counts.noComplete, `左欄不應該出現「完成」字樣（實際文字 ${JSON.stringify(counts.text).slice(0, 200)}）`);
    }

    // --- 選定的 Project 消失又出現，選取不應該自動跳回去（fix round 1／Codex finding 1）---
    // render.js 原本只在 renderState() 內部 fallback 到第一個 Project 讓畫面正確，沒有把
    // actions.js 那份持久狀態（ui.selectedProject）一併改掉：選 p → p 暫時消失（畫面正確
    // fallback 顯示 cockpit）→ p 又出現時，殘留的舊 ID 會讓畫面在使用者沒有任何操作的情況下
    // 自己跳回 p。spec「選定的 Project 已不在最新投影中時改為選定第一個」是正式的狀態改變，
    // 不是暫時的顯示 fallback，這裡驗證 p 重新出現後選取「仍是」cockpit，不會自動跳回去。
    log('--- 情境「選定的 Project 消失又出現，選取不應該自動跳回去」---');
    const selectedP = await cdp.click('[data-action="select-project"][data-project="p"]');
    check(selectedP, '應該能點左欄的 p（重新選定，供這個情境使用）');
    await cdp.waitFor('!!document.querySelector(\'.project[data-project="p"]\')', 2000, '選定 p 之後 Factory Floor 顯示 p');

    const disappearResult = await injectState(cdp, craftOnlyCockpitState());
    check(disappearResult && disappearResult.ok === true, '注入只有 cockpit（p 暫時消失）的特製投影');
    await sleep(300);
    check(
      await cdp.eval('!!document.querySelector(\'.project[data-project="cockpit"]\')'),
      'p 消失後，Factory Floor 應該 fallback 顯示 cockpit'
    );

    const reappearResult = await injectState(cdp, loadFixture());
    check(reappearResult && reappearResult.ok === true, '注入 p 重新出現（回到原始 fixture）的特製投影');
    await sleep(300);
    const afterReappear = await cdp.eval(`(() => ({
      shownProject: (document.querySelector('.project') || {}).dataset
        ? document.querySelector('.project').dataset.project
        : null,
      floorCount: document.querySelectorAll('.factory-floor').length,
    }))()`);
    check(
      afterReappear.shownProject === 'cockpit',
      `p 重新出現之後，Factory Floor 仍應該顯示 cockpit（選取沒有自動跳回 p；實際 ${JSON.stringify(afterReappear)}）`
    );

    // --- 選定 p → 投影變空 → 投影恢復成 [cockpit, p]，應該重新選定第一個（fix round 2／
    // Codex finding）---
    // fix round 1 的回寫只在 resolveSelectedProject() 找得到「其他可以 fallback 的
    // Project」時才生效（resolvedForPersist !== null）；投影 projects 剛好是空陣列時
    // resolveSelectedProject() 回傳 null，被那個條件擋下、完全不回寫，actions.js 仍記著
    // 舊的 p。下一份投影若恢復成 [cockpit, p]，殘留的舊 ID p 這次「真的在投影裡」，會被
    // resolveSelectedProject() 判定成「還是選定的」而直接顯示 p——不是「未選定過時預設第一
    // 個」的 cockpit。這裡驗證選取被正確正規化成 null、投影恢復後重新選出 cockpit。
    log('--- 情境「選定 p → 投影變空 → 投影恢復，重新選定第一個」（fix round 2／Codex finding）---');
    const selectedPAgain = await cdp.click('[data-action="select-project"][data-project="p"]');
    check(selectedPAgain, '應該能點左欄的 p（重新選定，供這個情境使用）');
    await cdp.waitFor('!!document.querySelector(\'.project[data-project="p"]\')', 2000, '選定 p 之後 Factory Floor 顯示 p');

    const emptiedResult = await injectState(cdp, craftNoProjectsState());
    check(emptiedResult && emptiedResult.ok === true, '注入沒有 Project 的特製投影（p 連同 cockpit 一起消失）');
    await sleep(300);
    check(
      await cdp.eval('!!document.querySelector(\'.projects-empty-state\')'),
      '投影變空之後，左欄應該顯示沒有 Project 的空狀態'
    );

    const restoredResult = await injectState(cdp, loadFixture());
    check(restoredResult && restoredResult.ok === true, '注入投影恢復成 [cockpit, p] 的特製投影');
    await sleep(300);
    const afterRestore = await cdp.eval(`(() => ({
      shownProject: (document.querySelector('.project') || {}).dataset
        ? document.querySelector('.project').dataset.project
        : null,
      floorCount: document.querySelectorAll('.factory-floor').length,
    }))()`);
    check(
      afterRestore.shownProject === 'cockpit',
      `投影從空陣列恢復成 [cockpit, p] 之後，應該重新選定第一個 cockpit，不是殘留的舊選取 p（實際 ${JSON.stringify(afterRestore)}）`
    );

    // --- Project 名稱 200 字：左欄項目最多 2 行、不撐破欄寬、完整名稱放 title（設計審核 M2）---
    log('--- 情境「Project 名稱 200 字：左欄最多 2 行、有 title」---');
    const longNameResult = await injectState(cdp, craftLongProjectNameState());
    check(longNameResult && longNameResult.ok === true, '注入 200 字 Project 名稱的特製投影');
    await sleep(300);
    const longName = '測'.repeat(200);
    const nameMetrics = await cdp.eval(`(() => {
      var item = document.querySelector('[data-action="select-project"][data-project="cockpit"]');
      var nameEl = item ? item.querySelector('.project-item-name') : null;
      if (!nameEl) return null;
      var cs = getComputedStyle(nameEl);
      var lineHeight = parseFloat(cs.lineHeight);
      var rect = nameEl.getBoundingClientRect();
      var region = document.querySelector('[data-region="projects"]');
      return {
        title: nameEl.title,
        textLength: nameEl.textContent.length,
        rectHeight: rect.height,
        lineHeight,
        lineCount: isNaN(lineHeight) || lineHeight <= 0 ? null : rect.height / lineHeight,
        regionScrollWidth: region ? region.scrollWidth : null,
        regionClientWidth: region ? region.clientWidth : null,
      };
    })()`);
    check(nameMetrics !== null, '應該找到左欄 cockpit 項目的名稱節點（.project-item-name）');
    if (nameMetrics !== null) {
      check(
        nameMetrics.title === longName,
        `名稱節點的 title 應該是完整的 200 字名稱（實際長度 ${nameMetrics.title ? nameMetrics.title.length : 0}）`
      );
      check(
        nameMetrics.lineCount !== null,
        `應該量得到 line-height（style.css 明確設定，不是瀏覽器預設的 normal，才量得出行數；實際 lineHeight=${nameMetrics.lineHeight}）`
      );
      check(
        nameMetrics.lineCount !== null && nameMetrics.lineCount <= 2.2,
        `設計審核 M2：名稱最多顯示 2 行（允許量測誤差；實際約 ${nameMetrics.lineCount} 行，height=${nameMetrics.rectHeight}, lineHeight=${nameMetrics.lineHeight}）`
      );
      check(
        nameMetrics.regionScrollWidth === null || nameMetrics.regionScrollWidth <= nameMetrics.regionClientWidth + 1,
        `200 字名稱不應該撐破左欄欄寬（scrollWidth=${nameMetrics.regionScrollWidth}, clientWidth=${nameMetrics.regionClientWidth}）`
      );
    }

    // --- 沒有 Project（注入空 projects）---
    const emptyResult = await injectState(cdp, craftNoProjectsState());
    check(emptyResult && emptyResult.ok === true, '注入沒有 Project 的特製投影');
    await sleep(300);
    // fix round 1／設計審核 M4：左欄那則原本重複了 Factory Floor 那句幾乎一樣的說明（兩段
    // 上下或左右相鄰時像同一句話說兩次，「在這裡看到」也沒有受詞），審核建議左欄只寫狀態、
    // 完整的指向動作說明留給 Factory Floor 那句 D11 逐字定案文案。這裡改成精確字串比對（同
    // GONE_TEXT／TRUNCATED_TEXT 的做法）。
    const PROJECTS_EMPTY_TEXT = '沒有 Project';
    const FLOOR_EMPTY_TEXT =
      '在 cockpit.toml 加入 [[project]] 區段即可在這裡看到 Factory Floor，加入後需要重啟 cockpit';
    const empty = await cdp.eval(`(() => {
      var col = document.querySelector('[data-region="projects"]');
      var floor = document.querySelector('[data-region="floor"]');
      var runtimes = document.querySelector('[data-region="runtimes"]');
      return {
        colText: col ? col.textContent : null,
        floorText: floor ? floor.textContent : null,
        runtimesStillThere: runtimes ? runtimes.textContent.length > 0 : null,
      };
    })()`);
    empty.leftColumnEmptyState = empty.colText === PROJECTS_EMPTY_TEXT;
    empty.floorEmptyState = empty.floorText === FLOOR_EMPTY_TEXT;
    check(
      empty.leftColumnEmptyState === true,
      `左欄應該顯示沒有 Project 的空狀態逐字文案「${PROJECTS_EMPTY_TEXT}」（實際 ${JSON.stringify(empty)}）`
    );
    check(
      empty.floorEmptyState === true,
      `Factory Floor 區域應該顯示沒有 Project 的空狀態逐字文案（design D11；「${FLOOR_EMPTY_TEXT}」；實際 ${JSON.stringify(empty)}）`
    );
    check(
      empty.runtimesStillThere === true,
      '沒有 Project 時 runtime 卡應該正常顯示（spec「沒有 Project」：頁面其他部分正常）'
    );
  } finally {
    await stopChrome(chrome, 'chrome-P1');
    await stopPreview(preview, 'preview-P1');
  }
}

// ---------------------------------------------------------------------------
// R1：dashboard/兩個runtime的畫面（task 2.3／3.3；子斷言標 topbar／statusbar／runtimes，
// Ruling R4）
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// direction-01-visual task 3.3：右欄換皮的量測工具（R1 runtimes 子斷言、D1、U1、V1 共用）。
// design D4 agent 對照表：working 冰青實心點、idle 次要文字色（實心點）、blocked 警示色（實心
// 點）、done 主要文字色＋冰青空心點（2px 實線環）、未知次要文字色＋斷線環（虛線或點線，跟
// idle 的實心點在形狀上分開——2.2 設計複審觀察 idle 與 unknown 同色，carry 8）。點是 CSS 畫的
// 8px 圓（border-radius 50%），不用 ●／○ 字元，符號節點 aria-hidden、沒有文字、沒有偽元素
// content。狀態文字沿用 .status 節點，文字精確等於投影的 agent_status 字串。
// ---------------------------------------------------------------------------
const AGENT_SPEC = {
  working: { text: 'accent', shape: 'solid', color: 'accent' },
  idle: { text: 'textDim', shape: 'solid', color: 'textDim' },
  blocked: { text: 'warn', shape: 'solid', color: 'warn' },
  done: { text: 'text', shape: 'ring', color: 'accent' },
  unknown: { text: 'textDim', shape: 'broken', color: 'textDim' },
};

const AGENT_MEASURE_JS = `
  function agentMeasure(scope) {
    if (!scope) return null;
    var st = scope.querySelector('.status');
    var dot = scope.querySelector('.agent-dot');
    if (!st || !dot) return { missing: true, hasStatus: !!st, hasDot: !!dot };
    var d = getComputedStyle(dot);
    var s = getComputedStyle(st);
    var bg = d.backgroundColor;
    var filled = bg !== 'rgba(0, 0, 0, 0)' && bg !== 'transparent';
    var sides = ['Top', 'Right', 'Bottom', 'Left'];
    var widths = sides.map(function (k) { return parseFloat(d['border' + k + 'Width']) || 0; });
    var styles = sides.map(function (k) { return d['border' + k + 'Style']; });
    var colors = sides.map(function (k) { return d['border' + k + 'Color']; });
    var uniform = widths.every(function (w) { return w === widths[0]; }) &&
      styles.every(function (x) { return x === styles[0]; }) &&
      colors.every(function (x) { return x === colors[0]; });
    var shape = 'none';
    var color = null;
    if (filled) { shape = 'solid'; color = bg; }
    else if (uniform && widths[0] >= 2 && styles[0] === 'solid') { shape = 'ring'; color = colors[0]; }
    else if (uniform && widths[0] >= 1 && (styles[0] === 'dashed' || styles[0] === 'dotted')) { shape = 'broken'; color = colors[0]; }
    var r = dot.getBoundingClientRect();
    return {
      text: st.textContent,
      statusClass: st.className,
      statusTitle: st.getAttribute('title'),
      textColor: s.color,
      fontFamily: s.fontFamily,
      shape: shape,
      color: color,
      borderWidth: widths[0],
      w: Math.round(r.width * 100) / 100,
      h: Math.round(r.height * 100) / 100,
      radius: d.borderTopLeftRadius,
      ariaHidden: dot.getAttribute('aria-hidden'),
      dotText: dot.textContent,
      pseudo: [getComputedStyle(dot, '::before').content, getComputedStyle(dot, '::after').content,
        getComputedStyle(st, '::before').content, getComputedStyle(st, '::after').content],
      dotVisible: d.display !== 'none' && d.visibility !== 'hidden' && r.width > 0,
    };
  }
`;

// 回傳不符合的理由（空陣列＝符合）。spec 以 status 字串為鍵；未知字串一律比對 unknown 那一列。
function agentMismatch(m, status) {
  const spec = AGENT_SPEC[status] || AGENT_SPEC.unknown;
  const why = [];
  if (!m || m.missing) return ['找不到 .status 或 .agent-dot'];
  if (m.text !== status) why.push(`狀態文字 ${JSON.stringify(m.text)} ≠ ${JSON.stringify(status)}`);
  if (m.textColor !== SPEC_COLORS[spec.text]) why.push(`狀態文字色 ${m.textColor} ≠ ${spec.text}`);
  if (m.shape !== spec.shape) why.push(`點形狀 ${m.shape} ≠ ${spec.shape}`);
  if (m.color !== SPEC_COLORS[spec.color]) why.push(`點顏色 ${m.color} ≠ ${spec.color}`);
  if (Math.abs(m.w - 8) > 0.6 || Math.abs(m.h - 8) > 0.6) why.push(`點尺寸 ${m.w}×${m.h} ≠ 8×8`);
  if (m.radius !== '50%') why.push(`點 border-radius ${m.radius} ≠ 50%`);
  if (m.ariaHidden !== 'true') why.push('點沒有 aria-hidden="true"');
  if (m.dotText !== '') why.push(`點節點含文字 ${JSON.stringify(m.dotText)}（應該是 CSS 畫的圓，不用字元）`);
  if (!m.pseudo.every((c) => c === 'none' || c === 'normal')) why.push(`偽元素有 content ${JSON.stringify(m.pseudo)}`);
  if (!m.dotVisible) why.push('點不可見');
  return why;
}

// 右欄結構量測（D11：runtime 卡只有本身一層框、workspace 一行標題列＋上方 1px --line、tab
// 縮排 8px 次標題 --text-dim 12px 不加框、pane 列兩行 grid、連線明細兩欄定義列表、最近事件
// 只顯示時間）。每一項回傳 { ok, detail }，讓否定對照可以只看單一項轉紅。
const RIGHT_COLUMN_MEASURE_JS = `
  function rcIsMono(ff) { return /cascadia mono|consolas|monospace/i.test(ff); }
  function rcBorders(el) {
    var cs = getComputedStyle(el);
    return ['Top', 'Right', 'Bottom', 'Left'].map(function (k) { return parseFloat(cs['border' + k + 'Width']) || 0; });
  }
  // cwd 保留尾段（設計審核 I1；task 3.3 fix round 1）：.pane-cwd 內拆成前段（.pane-cwd-head，單行
  // 省略、先縮）與尾段（.pane-cwd-tail，最後一段路徑，完整可見；尾段本身比格子還寬時才省略），
  // 兩段串起來＝完整路徑＝title。回傳 visible：尾段完整可見時就是尾段文字。
  function rcCwdInfo(n) {
    if (!n) return { ok: false, missing: true };
    var cs = getComputedStyle(n);
    var full = n.textContent;
    if (full === '\\u2014') return { ok: cs.color === ${JSON.stringify(SPEC_COLORS.textDim)}, dash: true };
    var head = n.querySelector(':scope > .pane-cwd-head');
    var tail = n.querySelector(':scope > .pane-cwd-tail');
    if (!head || !tail) return { ok: false, noSplit: true };
    var hcs = getComputedStyle(head), tcs = getComputedStyle(tail);
    var cr = n.getBoundingClientRect(), tr = tail.getBoundingClientRect();
    var tailFull = tail.scrollWidth <= tail.clientWidth + 0.5 && tr.right <= cr.right + 0.5 && tr.left >= cr.left - 0.5 && tr.width > 0;
    var tailTooWide = head.getBoundingClientRect().width <= 0.5 && tcs.textOverflow === 'ellipsis' && tcs.overflowX === 'hidden';
    var ok = n.getAttribute('title') === full && head.textContent + tail.textContent === full &&
      hcs.whiteSpace === 'nowrap' && hcs.overflowX === 'hidden' && hcs.textOverflow === 'ellipsis' &&
      [cs.color, hcs.color, tcs.color].every(function (c) { return c === ${JSON.stringify(SPEC_COLORS.textDim)}; }) &&
      (tailFull || tailTooWide) && cr.width > 0;
    return { ok: ok, tailFull: tailFull, tailTooWide: tailTooWide, visible: tailFull ? tail.textContent : null, head: head.textContent, tail: tail.textContent };
  }
  function rcMeasureRightColumn() {
    var out = {};
    var region = document.querySelector('[data-region="runtimes"]');
    var cards = Array.prototype.slice.call(document.querySelectorAll('[data-region="runtimes"] .runtime-card'));
    // 右欄一層框（使用者 2026-09-25 裁決，design D11 更新；task 3.3 fix round 1）：區塊本身四邊
    // 1px --line 框＋--bg-base 底（跟左欄、最近事件一致）；runtime 之間只用上方 1px --line 分隔線
    // （第一個沒有），不畫四邊卡框；卡內 workspace／tab／pane 列／連線明細也不畫四邊框。
    var regionB = region ? rcBorders(region) : null;
    var rcs0 = region ? getComputedStyle(region) : null;
    var regionOk = !!region && regionB.every(function (w) { return w === 1; }) && rcs0.borderTopStyle === 'solid' &&
      rcs0.borderTopColor === ${JSON.stringify(SPEC_COLORS.line)} && rcs0.backgroundColor === ${JSON.stringify(SPEC_COLORS.bgBase)};
    var cardBad = cards.filter(function (c, i) {
      var b = rcBorders(c);
      var cs = getComputedStyle(c);
      if (b[1] !== 0 || b[2] !== 0 || b[3] !== 0) return true;
      if (i === 0) return b[0] !== 0;
      return !(b[0] === 1 && cs.borderTopStyle === 'solid' && cs.borderTopColor === ${JSON.stringify(SPEC_COLORS.line)});
    }).length;
    var boxedInside = Array.prototype.slice.call(document.querySelectorAll('.runtime-card, .runtime-card .workspace, .runtime-card .tab, .runtime-card .pane-row, .runtime-card .connection'))
      .filter(function (n) { var b = rcBorders(n); return b[0] > 0 && b[1] > 0 && b[2] > 0; }).length;
    out.frame = {
      ok: regionOk && cards.length > 0 && cardBad === 0 && boxedInside === 0,
      detail: { regionBorders: regionB, regionBg: rcs0 && rcs0.backgroundColor, cards: cards.length, cardsWithBadBorders: cardBad, boxedInside: boxedInside },
    };
    // workspace：一行標題列、上方 1px --line、其餘三邊無框。
    var wsBad = [];
    document.querySelectorAll('.runtime-card .workspace').forEach(function (w) {
      var cs = getComputedStyle(w);
      var b = rcBorders(w);
      var header = w.querySelector(':scope > .workspace-header');
      var headerOneLine = false;
      if (header) {
        // 一行＝標題列的子節點（label、#number、彙總狀態）彼此在垂直方向都重疊（同一行）。
        var rects = Array.prototype.map.call(header.children, function (c) { return c.getBoundingClientRect(); });
        var maxTop = Math.max.apply(null, rects.map(function (r) { return r.top; }));
        var minBottom = Math.min.apply(null, rects.map(function (r) { return r.bottom; }));
        headerOneLine = rects.length >= 3 && maxTop < minBottom;
      }
      // 彙總狀態（符號＋文字）不得被長 label 擠到省略或裁切：label 才是該縮的那一個。
      var hst = header ? header.querySelector('.agent-state .status') : null;
      var statusFull = !!hst && hst.getBoundingClientRect().width > 0 && hst.scrollWidth <= hst.clientWidth + 0.5 &&
        header.querySelector('.agent-state').getBoundingClientRect().right <= header.getBoundingClientRect().right + 0.5;
      if (!(b[0] === 1 && cs.borderTopStyle === 'solid' && cs.borderTopColor === ${JSON.stringify(SPEC_COLORS.line)} && b[1] === 0 && b[2] === 0 && b[3] === 0 && headerOneLine && statusFull)) {
        wsBad.push({ borders: b, style: cs.borderTopStyle, color: cs.borderTopColor, headerOneLine: headerOneLine, statusFull: statusFull });
      }
    });
    out.workspace = { ok: document.querySelectorAll('.runtime-card .workspace').length > 0 && wsBad.length === 0, detail: wsBad };
    // tab：次標題縮排 8px（相對 workspace 標題列左緣）、--text-dim、12px、不加框。
    var tabBad = [];
    document.querySelectorAll('.runtime-card .tab').forEach(function (t) {
      var th = t.querySelector(':scope > .tab-header');
      var wh = t.closest('.workspace') ? t.closest('.workspace').querySelector(':scope > .workspace-header') : null;
      if (!th || !wh) { tabBad.push({ missing: true }); return; }
      var cs = getComputedStyle(th);
      var indent = th.getBoundingClientRect().left + (parseFloat(cs.paddingLeft) || 0) - (wh.getBoundingClientRect().left + (parseFloat(getComputedStyle(wh).paddingLeft) || 0));
      var num = th.querySelector('.tab-number');
      var ncs = num ? getComputedStyle(num) : null;
      var b = rcBorders(t).concat(rcBorders(th));
      if (!(Math.abs(indent - 8) <= 0.5 && ncs && ncs.color === ${JSON.stringify(SPEC_COLORS.textDim)} && ncs.fontSize === '12px' && b.every(function (w) { return w === 0; }))) {
        tabBad.push({ indent: Math.round(indent * 100) / 100, color: ncs && ncs.color, fontSize: ncs && ncs.fontSize, borders: b });
      }
    });
    out.tab = { ok: document.querySelectorAll('.runtime-card .tab').length > 0 && tabBad.length === 0, detail: tabBad };
    // pane 列兩行：DOM 順序不變（id、agent、狀態、標題、cwd）；第一行狀態（符號＋文字）→agent→
    // pane id（等寬、靠右）；第二行標題→cwd，兩者 --text-dim、單行省略、title 帶全文。
    var paneBad = [];
    var rows = document.querySelectorAll('[data-region="runtimes"] .pane-row');
    rows.forEach(function (row) {
      var order = Array.prototype.map.call(row.children, function (c) { return c.className.split(' ')[0]; });
      var expected = ['pane-id', 'pane-agent', 'agent-state', 'pane-title', 'pane-cwd'];
      var orderOk = expected.every(function (c, i) { return order[i] === c; });
      function rr(sel) { var n = row.querySelector(sel); return n ? n.getBoundingClientRect() : null; }
      var st = rr('.agent-state'), ag = rr('.pane-agent'), id = rr('.pane-id'), ti = rr('.pane-title'), cw = rr('.pane-cwd');
      if (!st || !ag || !id || !ti || !cw) { paneBad.push({ pane: row.dataset.pane, missing: true, order: order }); return; }
      var rcs = getComputedStyle(row);
      var rrow = row.getBoundingClientRect();
      var contentRight = rrow.right - (parseFloat(rcs.borderRightWidth) || 0) - (parseFloat(rcs.paddingRight) || 0);
      var line1Bottom = Math.max(st.bottom, ag.bottom, id.bottom);
      var line1Top = Math.min(st.top, ag.top, id.top);
      var line2Top = Math.min(ti.top, cw.top);
      var bind = row.querySelector('[data-action="bind-here"]');
      var idNode = row.querySelector('.pane-id');
      var textsOk = (function () {
        var n = row.querySelector('.pane-title');
        var cs = getComputedStyle(n);
        var full = n.textContent;
        return cs.color === ${JSON.stringify(SPEC_COLORS.textDim)} && cs.whiteSpace === 'nowrap' && cs.textOverflow === 'ellipsis' &&
          cs.overflowX === 'hidden' && (full === '\\u2014' || n.getAttribute('title') === full);
      })() && rcCwdInfo(row.querySelector('.pane-cwd')).ok;
      var res = {
        display: rcs.display,
        orderOk: orderOk,
        twoLines: line2Top >= line1Bottom - 0.5 && ti.top < rrow.bottom && line1Top >= rrow.top - 0.5,
        line1Order: st.right <= ag.left + 0.5 && ag.right <= id.left + 0.5,
        line2Order: ti.right <= cw.left + 0.5,
        idRight: Math.abs(id.right - contentRight) <= 1,
        // 改綁模式的「綁定到這裡」在第三行，不跟兩行內容同列。
        bindBelow: !bind || bind.getBoundingClientRect().top >= Math.max(ti.bottom, cw.bottom) - 0.5,
        idMono: rcIsMono(getComputedStyle(idNode).fontFamily),
        textsOk: textsOk,
      };
      if (!(res.display === 'grid' && res.orderOk && res.twoLines && res.line1Order && res.line2Order && res.idRight && res.bindBelow && res.idMono && res.textsOk)) {
        res.pane = row.dataset.pane;
        res.order = order;
        paneBad.push(res);
      }
    });
    out.pane = { ok: rows.length > 0 && paneBad.length === 0, detail: { rows: rows.length, bad: paneBad } };
    // 連線明細：兩欄定義列表（dt --text-dim 標籤、dd 等寬值，同一列左右排），不用中點串接。
    var connBad = [];
    cards.forEach(function (c) {
      var dl = c.querySelector('dl.connection-details');
      if (!dl) { connBad.push({ card: (c.querySelector('.runtime-id') || {}).textContent || null, noDl: true }); return; }
      var dts = dl.querySelectorAll(':scope > dt');
      var dds = dl.querySelectorAll(':scope > dd');
      var pairsOk = dts.length > 0 && dts.length === dds.length;
      var rowsOk = pairsOk && Array.prototype.every.call(dts, function (dt, i) {
        var a = dt.getBoundingClientRect(), b = dds[i].getBoundingClientRect();
        return Math.abs(a.top - b.top) <= 1 && a.right <= b.left + 0.5 &&
          getComputedStyle(dt).color === ${JSON.stringify(SPEC_COLORS.textDim)} && rcIsMono(getComputedStyle(dds[i]).fontFamily) &&
          getComputedStyle(dds[i]).color === ${JSON.stringify(SPEC_COLORS.textDim)} &&
          !rcIsMono(getComputedStyle(dt).fontFamily);
      });
      var middot = /[\\u00b7\\u2022\\u30fb]/.test((c.querySelector('.connection') || c).textContent);
      if (!(pairsOk && rowsOk && !middot)) connBad.push({ card: (c.querySelector('.runtime-id') || {}).textContent || null, dt: dts.length, dd: dds.length, rowsOk: rowsOk, middot: middot });
    });
    out.connection = { ok: cards.length > 0 && connBad.length === 0, detail: connBad };
    // 最近事件：at 只顯示時間（保留 Z），完整字串在 title；時間與 id 用等寬字。
    var evBad = [];
    var evRows = document.querySelectorAll('[data-region="events"] .event-row');
    evRows.forEach(function (row, i) {
      var at = row.querySelector('.event-at');
      var subj = row.querySelector('.event-subject');
      if (!at || !subj) { evBad.push({ i: i, missing: true }); return; }
      var full = at.getAttribute('title') || '';
      var ok = /^\\d{2}:\\d{2}:\\d{2}(\\.\\d+)?Z$/.test(at.textContent) && full.length > at.textContent.length &&
        full.slice(-at.textContent.length) === at.textContent && full.indexOf('T') === full.length - at.textContent.length - 1 &&
        rcIsMono(getComputedStyle(at).fontFamily) && (subj.textContent === '' || rcIsMono(getComputedStyle(subj).fontFamily));
      if (!ok) evBad.push({ i: i, text: at.textContent, title: full, atFont: getComputedStyle(at).fontFamily, subjFont: getComputedStyle(subj).fontFamily });
    });
    out.events = { ok: evRows.length > 0 && evBad.length === 0, detail: { rows: evRows.length, bad: evBad.slice(0, 3) } };
    // 欄位對齊（設計審核 M1；fix round 1）：所有事件的 kind 欄與 subject 欄左緣相同；每張 runtime
    // 的連線明細值欄（dd）左緣相同。
    function lefts(sel) { return Array.prototype.map.call(document.querySelectorAll(sel), function (n) { return Math.round(n.getBoundingClientRect().left * 100) / 100; }); }
    function same(a) { return a.length > 0 && a.every(function (x) { return Math.abs(x - a[0]) <= 0.5; }); }
    var kindL = lefts('[data-region="events"] .event-kind');
    var subjL = lefts('[data-region="events"] .event-subject');
    var ddL = cards.map(function (c) { var d = c.querySelector('dl.connection-details > dd'); return d ? Math.round(d.getBoundingClientRect().left * 100) / 100 : null; });
    out.align = {
      ok: kindL.length >= 2 && same(kindL) && same(subjL) && ddL.length >= 2 && ddL.every(function (x) { return x !== null; }) && same(ddL),
      detail: { kind: kindL, subject: subjL, dd: ddL },
    };
    // HERDR 焦點（設計審核 M3；fix round 1）：pane 列不用底色，改用「作用中」文字標記
    // （--text-dim 12px、DOM 文字、有 title），只標 .focused 的那一列。
    var focusBad = [];
    var plainBg = null;
    rows.forEach(function (row) {
      if (plainBg === null && !row.classList.contains('focused') && !row.classList.contains('selected') && !row.classList.contains('bind-target')) {
        plainBg = getComputedStyle(row).backgroundColor;
      }
    });
    var focusedCount = 0;
    rows.forEach(function (row) {
      var mark = row.querySelector(':scope > .pane-herdr-focus');
      if (row.classList.contains('focused')) {
        focusedCount += 1;
        var mcs = mark ? getComputedStyle(mark) : null;
        var bg = getComputedStyle(row).backgroundColor;
        var plainish = row.classList.contains('selected') || row.classList.contains('bind-target') || bg === 'rgba(0, 0, 0, 0)' || bg === plainBg;
        var okRow = !!mark && mark.textContent === '\u4f5c\u7528\u4e2d' && !!mark.getAttribute('title') && mcs.color === ${JSON.stringify(SPEC_COLORS.textDim)} &&
          mcs.fontSize === '12px' && mark.getBoundingClientRect().width > 0 && plainish;
        if (!okRow) focusBad.push({ pane: row.dataset.pane, mark: mark ? mark.textContent : null, color: mcs && mcs.color, bg: bg, plainBg: plainBg });
      } else if (mark) {
        focusBad.push({ pane: row.dataset.pane, unexpectedMark: true });
      }
    });
    out.focus = { ok: focusBad.length === 0, detail: { focused: focusedCount, bad: focusBad } };
    // tab 之間的間距（設計審核 M2；fix round 1）：相鄰 tab 的間距比 pane 列之間多 ≥8px。只有一個 tab
    // 時量不到（na），由注入兩個 tab 的投影驗。
    var tabs = Array.prototype.slice.call(document.querySelectorAll('.runtime-card .tab'));
    var gaps = [];
    tabs.forEach(function (t) {
      var prev = t.previousElementSibling;
      if (prev && prev.classList.contains('tab')) gaps.push(Math.round((t.getBoundingClientRect().top - prev.getBoundingClientRect().bottom) * 100) / 100);
    });
    var paneGap = tabs.length ? parseFloat(getComputedStyle(tabs[0]).rowGap) || 0 : 0;
    out.tabgap = { ok: gaps.length > 0 && gaps.every(function (g) { return g >= paneGap + 8 - 0.5; }), na: gaps.length === 0, detail: { gaps: gaps, paneGap: paneGap } };
    return out;
  }
`;

const RIGHT_COLUMN_KEYS = ['frame', 'workspace', 'tab', 'pane', 'connection', 'events', 'align', 'focus'];
const RIGHT_COLUMN_LABELS = {
  frame: '右欄一層框（區塊四邊 1px --line＋--bg-base；runtime 之間上方 1px --line 分隔線、第一個沒有；卡與卡內 workspace／tab／pane 列／連線明細不畫四邊框）',
  workspace: 'workspace 一行標題列（label、#number、彙總狀態）＋上方 1px --line，其餘三邊無框',
  tab: 'tab 次標題縮排 8px、--text-dim 12px、不加框',
  pane: 'pane 列兩行 grid（DOM 順序不變；第一行 狀態→agent→pane id 等寬靠右；第二行 標題→cwd，--text-dim 單行省略、title 帶全文；cwd 省略前段、尾段完整可見）',
  connection: '連線明細是兩欄定義列表（dt --text-dim 標籤、dd --text-dim 等寬值、同列左右排），沒有中點串接',
  events: '最近事件 at 只顯示時間（保留 Z）、完整字串在 title，時間與 id 用等寬字',
  align: '相鄰列同一種欄位對齊（事件 kind／subject 欄左緣相同、各 runtime 連線明細值欄左緣相同）',
  focus: 'HERDR 焦點只以「作用中」文字標記（--text-dim 12px、有 title）標在 .focused 的 pane 列，不用底色',
};

async function partTwoRuntimes() {
  log('=== R1. 兩個 runtime 的畫面（topbar／statusbar／runtimes 子斷言）===');
  let preview = null;
  let chrome = null;
  try {
    preview = await startPreview({}, 'preview-R1');
    const url = `http://127.0.0.1:${preview.port}/`;
    chrome = await startChrome(pickPort(19080, [preview.port]), url, 'chrome-R1');
    const { cdp } = chrome;
    await waitForFirstProjection(cdp, preview.port);
    // fixture 本身已經是 GIVEN：win（connected）與 wsl（disconnected，附原因）。

    // --- topbar 子斷言（task 2.3）：頂列出現兩個連線燈號，fix round 1／Codex F7 改成逐一
    // 斷言 win／wsl 各自的 id、狀態文字與對應 token 色，不是只數數量。
    // fix round 2／Codex F7 未完成：原本用 text.indexOf('connected') 判定，"disconnected"
    // 這個字串本身就包含 "connected" 子字串，wsl 燈號文字若誤植成「win disconnected」之類
    // 也會被判定通過——substring 比對本質上分不出這兩種狀態。改成：優先找一個跟 id 文字分開
    // 的專用狀態節點（`[data-conn-state]`，本檔對 design 的期望約定，供 task 2.3 實作參考；
    // design 目前沒有強制這個屬性名，只是「id 與狀態要能分開讀」這件事本身是精確比對的前提），
    // 讀 trim() 後的文字跟預期值「完整字串相等」；找不到專用節點時退回整顆燈號的文字，一樣用
    // 完整字串相等比對（不會因為子字串巧合誤判，只是因為 id 文字混在一起而預期 FAIL，這是誠實
    // 反映現況，不是遷就子字串比對）。 ---
    const topbarDetail = await cdp.eval(`(() => {
      var region = document.querySelector('[data-region="topbar"]');
      function badgeInfo(id) {
        var n = region ? region.querySelector('[data-runtime="' + id + '"]') : null;
        if (!n) return null;
        var cs = getComputedStyle(n);
        var stateNode = n.querySelector('[data-conn-state]');
        return {
          fullText: n.textContent,
          stateText: (stateNode ? stateNode.textContent : n.textContent).trim(),
          hasDedicatedStateNode: !!stateNode,
          color: cs.color,
        };
      }
      return { hasRegion: !!region, win: badgeInfo('win'), wsl: badgeInfo('wsl') };
    })()`);
    check(topbarDetail.hasRegion === true, 'topbar: 應該有 data-region="topbar"（design D2；目前尚未實作，預期 FAIL）');
    check(topbarDetail.win !== null, 'topbar: 應該找到 win 的連線燈號（[data-region="topbar"] [data-runtime="win"]）');
    if (topbarDetail.win !== null) {
      check(topbarDetail.win.fullText.indexOf('win') !== -1, `topbar: win 燈號文字應該含有 runtime id "win"（實際 ${JSON.stringify(topbarDetail.win.fullText)}）`);
      check(
        topbarDetail.win.stateText === 'connected',
        `topbar: win 的狀態文字應該精確等於 "connected"（實際 ${JSON.stringify(topbarDetail.win.stateText)}；${topbarDetail.win.hasDedicatedStateNode ? '讀自專用節點 [data-conn-state]' : '目前沒有專用狀態節點，退回整顆燈號文字（id 與狀態混在一起，本身就不精確，預期 FAIL）'}）`
      );
      check(topbarDetail.win.color === SPEC_COLORS.ok, `topbar: win（connected）燈號應該用成功色 token（design D4 #39D5AC；實際 ${topbarDetail.win.color}）`);
    }
    check(topbarDetail.wsl !== null, 'topbar: 應該找到 wsl 的連線燈號（[data-region="topbar"] [data-runtime="wsl"]）');
    if (topbarDetail.wsl !== null) {
      check(topbarDetail.wsl.fullText.indexOf('wsl') !== -1, `topbar: wsl 燈號文字應該含有 runtime id "wsl"（實際 ${JSON.stringify(topbarDetail.wsl.fullText)}）`);
      check(
        topbarDetail.wsl.stateText === 'disconnected',
        `topbar: wsl 的狀態文字應該精確等於 "disconnected"（實際 ${JSON.stringify(topbarDetail.wsl.stateText)}；${topbarDetail.wsl.hasDedicatedStateNode ? '讀自專用節點 [data-conn-state]' : '目前沒有專用狀態節點，退回整顆燈號文字，預期 FAIL'}）`
      );
      check(topbarDetail.wsl.color === SPEC_COLORS.bad, `topbar: wsl（disconnected）燈號應該用失敗色 token（design D4 #F47279；實際 ${topbarDetail.wsl.color}）`);
    }

    // 否定對照（fix round 2／Codex）：直接證明「indexOf('connected') 對 'disconnected' 會誤判
    // 為 true」這件事成立，再證明新的精確比對不會被同一個子字串騙過。
    const negControlSubstring = await cdp.eval(`(() => {
      var text = 'disconnected';
      return {
        oldBuggyIndexOf: text.indexOf('connected') !== -1,
        newExactMatch: text.trim() === 'connected',
      };
    })()`);
    check(
      negControlSubstring.oldBuggyIndexOf === true,
      `否定對照：舊寫法（indexOf('connected')）對文字 "disconnected" 應該誤判為 true（實際 ${negControlSubstring.oldBuggyIndexOf}）— 證明子字串比對確實有這個假陽性風險，不是憑空疑慮`
    );
    check(
      negControlSubstring.newExactMatch === false,
      `否定對照：新寫法（精確比對 === 'connected'）對文字 "disconnected" 應該正確判定為 false（實際 ${negControlSubstring.newExactMatch}）— 證明修正後的比對不會被子字串騙過`
    );

    // --- statusbar 子斷言（task 2.3）：底列顯示通道狀態與 version，fix round 1／Codex F7
    // 改成明確斷言 hasVersion／hasChannel（不是算出來卻沒 check），並核對投影的確切 version
    // 值（跟 preview 的 /api/state 比對，不是只驗證「有數字」）。
    // fix round 2／Codex F7 未完成：原本的 alternation `/connected|connecting|disconnected/`
    // 只證明「三者之一有出現」，沒有綁定「這個情境（頁面剛連上）預期是 connected」這個具體值——
    // 就算通道其實顯示 disconnected，這條也會誤判通過。改成跟 topbar 一樣：優先找專用節點
    // （`[data-channel-state]`），精確比對預期值 "connected"。 ---
    const expectedState = await fetch(`http://127.0.0.1:${preview.port}/api/state`).then((r) => r.json());
    const statusbar = await cdp.eval(`(() => {
      var region = document.querySelector('[data-region="statusbar"]');
      var channelNode = region ? region.querySelector('[data-channel-state]') : null;
      return {
        hasRegion: !!region,
        text: region ? region.textContent : null,
        channelStateText: (channelNode ? channelNode.textContent : (region ? region.textContent : '')).trim(),
        hasDedicatedChannelNode: !!channelNode,
      };
    })()`);
    check(statusbar.hasRegion === true, 'statusbar: 應該有 data-region="statusbar"（design D2；目前 version／channel 都在頂列，預期 FAIL）');
    const versionRe = new RegExp('v' + expectedState.version + '(\\D|$)');
    check(
      statusbar.hasRegion === true && statusbar.text !== null && versionRe.test(statusbar.text),
      `statusbar: 應該顯示確切的 version（期望 v${expectedState.version}；statusbar 文字 ${JSON.stringify(statusbar.text)}）`
    );
    check(
      statusbar.hasRegion === true && statusbar.channelStateText === 'connected',
      `statusbar: 通道狀態文字應該精確等於 "connected"（GIVEN 頁面剛連上；實際 ${JSON.stringify(statusbar.channelStateText)}；${statusbar.hasDedicatedChannelNode ? '讀自專用節點 [data-channel-state]' : '目前沒有專用節點，退回整個 statusbar 文字，預期 FAIL'}）`
    );

    // --- runtimes 子斷言（task 3.3）：右欄出現兩張卡，wsl 卡顯示原因，pane 狀態符號＋文字＋
    // 色彩對應狀態 ---
    const runtimes = await cdp.eval(`(() => {
      var cards = document.querySelectorAll('.runtime-card');
      var wslCard = null;
      cards.forEach(function (c) {
        if (c.textContent.indexOf('wsl') !== -1) wslCard = c;
      });
      var wJp1 = document.querySelector('.pane-row[data-runtime="win"][data-pane="wJ:p1"]');
      var symbol = wJp1 ? wJp1.querySelector('[aria-hidden="true"]') : null;
      return {
        cardCount: cards.length,
        wslReason: wslCard ? /reason|原因|未啟動/.test(wslCard.textContent) : false,
        paneHasSymbol: !!symbol,
      };
    })()`);
    check(runtimes.cardCount === 2, `runtimes: 右欄應該出現兩張 runtime 卡（實際 ${runtimes.cardCount}）`);
    check(runtimes.wslReason === true, `runtimes: wsl 卡應該顯示斷線原因（已經在現行前端顯示，見 render.js renderConnection；實際 ${runtimes.wslReason}）`);
    check(
      runtimes.paneHasSymbol === true,
      `runtimes: pane 的 agent 狀態應該有符號＋文字＋色彩（design D4「符號放在 DOM 文字裡」；目前尚未實作，預期 FAIL）`
    );

    // direction-01-visual task 3.3：上面那條只驗「有 aria-hidden 節點」，不足以證明「符號、文字與
    // 色彩對應狀態」（spec 本情境 THEN）。逐列對 design D4 agent 對照表（AGENT_SPEC）：fixture 的
    // 三個 pane（working／done／unknown），再注入五種已知狀態＋一個未知字串各一列。
    const fixture = loadFixture();
    const fixturePanes = fixture.runtimes[0].workspaces[0].tabs[0].panes;
    const fixtureAgents = await cdp.eval(`(() => {
      ${AGENT_MEASURE_JS}
      var ids = ${JSON.stringify(fixturePanes.map((p) => p.id))};
      return ids.map(function (id) { return agentMeasure(document.querySelector('.pane-row[data-runtime="win"][data-pane="' + id + '"]')); });
    })()`);
    fixturePanes.forEach((p, i) => {
      const why = agentMismatch(fixtureAgents[i], p.agent_status);
      check(why.length === 0, `runtimes [R1/agent]：${p.id}（${p.agent_status}）的符號、文字與色彩對應 design D4（${why.join('；') || 'ok'}；實際 ${JSON.stringify(fixtureAgents[i])}）`);
    });

    // 右欄結構（D11），fixture 畫面。
    const rc = await cdp.eval(`(() => { ${RIGHT_COLUMN_MEASURE_JS} return rcMeasureRightColumn(); })()`);
    for (const key of RIGHT_COLUMN_KEYS) {
      check(rc[key] && rc[key].ok === true, `runtimes [R1/${key}]：${RIGHT_COLUMN_LABELS[key]}（實際 ${JSON.stringify(rc[key] && rc[key].detail)}）`);
    }
    // 否定對照：逐項注入一條違規樣式，只看那一項會轉紅（證明每一項都有辨識力），量完立刻移除。
    const RC_NEGATIVE = {
      frame: '[data-region="runtimes"] .runtime-card { border: 1px solid var(--line) !important; }',
      workspace: '.runtime-card .workspace { border: 1px solid var(--line) !important; }',
      tab: '.runtime-card .tab > .tab-header { padding-left: 0 !important; margin-left: 0 !important; } .runtime-card .tab { padding-left: 0 !important; margin-left: 0 !important; }',
      pane: '[data-region="runtimes"] .pane-row { display: flex !important; }',
      connection: '.connection-details > dd { font-family: serif !important; }',
      events: '[data-region="events"] .event-at { font-family: serif !important; }',
      align: '.recent-events-list { display: flex !important; flex-direction: column !important; } .event-row { grid-template-columns: max-content fit-content(40%) minmax(0, 1fr) !important; } .connection-details { grid-template-columns: max-content minmax(0, 1fr) !important; }',
      focus: '.pane-row.focused { background: var(--focused-tint) !important; }',
    };
    for (const key of RIGHT_COLUMN_KEYS) {
      const neg = await cdp.eval(`(() => {
        ${RIGHT_COLUMN_MEASURE_JS}
        var st = document.createElement('style');
        st.textContent = ${JSON.stringify(RC_NEGATIVE[key])};
        document.head.appendChild(st);
        var m = rcMeasureRightColumn();
        st.remove();
        return m[${JSON.stringify(key)}];
      })()`);
      check(neg && neg.ok === false, `否定對照 [R1/${key}]：注入 ${RC_NEGATIVE[key]} 後這一項必須判定不符（實際 ok=${neg && neg.ok}）`);
    }
    // 事件時間的格式判準本身：完整 ISO 字串（未截成時間）必須判定不符。
    check(
      !/^\d{2}:\d{2}:\d{2}(\.\d+)?Z$/.test('2026-09-15T01:59:30Z') && /^\d{2}:\d{2}:\d{2}(\.\d+)?Z$/.test('01:59:30Z'),
      '否定對照 [R1/events]：時間格式判準拒絕完整 ISO 字串、接受 01:59:30Z'
    );
    // 否定對照 [R1/connection] 第二條：dd 值改回 --text（M4 之前的樣子）時也要判定不符。
    const ddNeg = await cdp.eval(`(() => {
      ${RIGHT_COLUMN_MEASURE_JS}
      var st = document.createElement('style');
      st.textContent = '.connection-details > dd { color: var(--text) !important; }';
      document.head.appendChild(st);
      var m = rcMeasureRightColumn();
      st.remove();
      return m.connection.ok;
    })()`);
    check(ddNeg === false, `否定對照 [R1/connection]：連線明細值改成 --text 時判定不符（實際 ok=${ddNeg}）`);

    // cwd 保留尾段（設計審核 I1；task 3.3 fix round 1）：fixture 的 wJ:p1 與 wJ:p3 只在最後一段不同
    // （…\ai-cockpit 對 …\ai-cockpit\sub），畫面上可見的文字必須不同——尾段完整可見且兩者不同。
    const cwdPair = await cdp.eval(`(() => {
      ${RIGHT_COLUMN_MEASURE_JS}
      function info(id) { return rcCwdInfo(document.querySelector('.pane-row[data-pane="' + id + '"] .pane-cwd')); }
      var out = { a: info('wJ:p1'), b: info('wJ:p3') };
      // 否定對照：放回「單一 span 從尾端省略」的效果——前段不縮、尾段可縮，尾段會被擠出或截斷。
      var st = document.createElement('style');
      st.textContent = '.pane-cwd-head { flex: 0 0 auto !important; } .pane-cwd-tail { flex: 0 1 auto !important; min-width: 0 !important; }';
      document.head.appendChild(st);
      out.negA = info('wJ:p1');
      out.negB = info('wJ:p3');
      st.remove();
      return out;
    })()`);
    check(
      cwdPair.a.ok && cwdPair.b.ok && cwdPair.a.visible !== null && cwdPair.b.visible !== null && cwdPair.a.visible !== cwdPair.b.visible,
      `runtimes [R1/cwd]：兩個只在最後一段不同的 cwd，畫面上可見的尾段完整且不同（實際 ${JSON.stringify({ a: cwdPair.a, b: cwdPair.b })}）`
    );
    check(
      !(cwdPair.negA.visible !== null && cwdPair.negB.visible !== null && cwdPair.negA.visible !== cwdPair.negB.visible),
      `否定對照 [R1/cwd]：前段不縮、尾段可縮時，至少一列的尾段不再完整可見（實際 ${JSON.stringify({ a: cwdPair.negA, b: cwdPair.negB })}）`
    );

    // 改綁模式（「綁定到這裡」）：可綁定的列多一顆按鈕，兩行排版仍成立、按鈕在第三行，agent 名稱
    // 不被擠掉（實作初版把按鈕放在第四欄時，agent 欄被壓成 0 寬——截圖發現，這裡守住）。
    await cdp.click('[data-action="rebind"][data-project="cockpit"][data-workstream="be"]');
    await cdp.waitFor("document.querySelectorAll('[data-action=\"bind-here\"]').length > 0", 3000, 'R1：進入改綁模式（出現「綁定到這裡」）');
    const rebindLayout = await cdp.eval(`(() => {
      ${RIGHT_COLUMN_MEASURE_JS}
      var m = rcMeasureRightColumn();
      var agents = Array.prototype.map.call(document.querySelectorAll('.pane-row.bind-target .pane-agent'), function (n) {
        return { text: n.textContent, w: Math.round(n.getBoundingClientRect().width * 100) / 100, sw: n.scrollWidth };
      });
      return { pane: m.pane, agents: agents };
    })()`);
    check(rebindLayout.pane.ok === true, `runtimes [R1/pane-rebind]：改綁模式下每一列仍是兩行排版、「綁定到這裡」在第三行（實際 ${JSON.stringify(rebindLayout.pane.detail)}）`);
    check(
      rebindLayout.agents.length > 0 && rebindLayout.agents.every((a) => a.w >= a.sw - 0.5 && a.w > 0),
      `runtimes [R1/pane-rebind]：改綁模式下 agent 名稱完整可見、沒有被擠成省略或 0 寬（實際 ${JSON.stringify(rebindLayout.agents)}）`
    );
    // 否定對照：放回初版「按鈕在第四欄」的排法，兩條都必須轉紅。
    const rebindNeg = await cdp.eval(`(() => {
      ${RIGHT_COLUMN_MEASURE_JS}
      var st = document.createElement('style');
      st.textContent = '.pane-row.bind-target { grid-template-columns: fit-content(60%) minmax(0, 1fr) fit-content(45%) auto !important; grid-template-areas: "state agent id bind" "title title cwd bind" !important; }';
      document.head.appendChild(st);
      var m = rcMeasureRightColumn();
      var agents = Array.prototype.map.call(document.querySelectorAll('.pane-row.bind-target .pane-agent'), function (n) {
        return { w: n.getBoundingClientRect().width, sw: n.scrollWidth };
      });
      st.remove();
      return { paneOk: m.pane.ok, agentsOk: agents.length > 0 && agents.every(function (a) { return a.w >= a.sw - 0.5 && a.w > 0; }) };
    })()`);
    check(
      rebindNeg.paneOk === false && rebindNeg.agentsOk === false,
      `否定對照 [R1/pane-rebind]：按鈕放回第四欄時，第三行與 agent 完整可見兩項都判定不符（實際 ${JSON.stringify(rebindNeg)}）`
    );
    await cdp.click('[data-action="rebind-cancel"]');
    await cdp.waitFor("document.querySelectorAll('[data-action=\"bind-here\"]').length === 0", 3000, 'R1：離開改綁模式');

    // 五種已知 agent 狀態＋未知字串各一列（D4 整張對照表）。
    const allStates = loadFixture();
    allStates.version = 4242;
    const ALL_AGENT = ['working', 'idle', 'blocked', 'done', 'unknown', 'whatever'];
    const basePane = allStates.runtimes[0].workspaces[0].tabs[0].panes[0];
    allStates.runtimes[0].workspaces[0].tabs[0].panes = ALL_AGENT.map((st, i) => ({
      ...basePane,
      id: 'wJ:s' + i,
      agent_status: st,
      title: 'state ' + st,
      focused: false,
      exited: false,
    }));
    // 第二個 tab（設計審核 M2）：驗相鄰 tab 的間距。
    const baseTab = allStates.runtimes[0].workspaces[0].tabs[0];
    allStates.runtimes[0].workspaces[0].tabs.push({
      ...baseTab,
      id: 'wJ:t2',
      number: 2,
      focused: false,
      panes: [{ ...basePane, id: 'wJ:t2p1', agent_status: 'idle', title: 'tab2 pane', focused: false, exited: false }],
    });
    const inj = await injectState(cdp, allStates);
    check(inj && inj.ok === true, 'runtimes [R1/agent]：注入五種已知 agent 狀態＋未知字串 whatever 各一列');
    const allMeasured = await cdp.eval(`(() => {
      ${AGENT_MEASURE_JS}
      return ${JSON.stringify(ALL_AGENT)}.map(function (st, i) { return agentMeasure(document.querySelector('.pane-row[data-pane="wJ:s' + i + '"]')); });
    })()`);
    ALL_AGENT.forEach((st, i) => {
      const why = agentMismatch(allMeasured[i], st);
      check(why.length === 0, `runtimes [R1/agent]：${st} 列的符號、文字與色彩對應 design D4（${why.join('；') || 'ok'}；實際 ${JSON.stringify(allMeasured[i])}）`);
    });
    const tabGap = await cdp.eval(`(() => {
      ${RIGHT_COLUMN_MEASURE_JS}
      var out = { real: rcMeasureRightColumn().tabgap };
      var st = document.createElement('style');
      st.textContent = '.tab + .tab { margin-top: 0 !important; }';
      document.head.appendChild(st);
      out.neg = rcMeasureRightColumn().tabgap;
      st.remove();
      return out;
    })()`);
    check(tabGap.real.ok === true, `runtimes [R1/tabgap]：相鄰 tab 的間距比 pane 列之間多 ≥8px（實際 ${JSON.stringify(tabGap.real)}）`);
    check(tabGap.neg.ok === false && tabGap.neg.na === false, `否定對照 [R1/tabgap]：拿掉 tab 間距時判定不符（實際 ${JSON.stringify(tabGap.neg)}）`);
    // 五種已知狀態的「點形狀＋點顏色＋文字色」兩兩不同（idle 與 unknown 同為 --text-dim，必須靠形狀分開）。
    const sigs = ALL_AGENT.slice(0, 5).map((st, i) => st + '=' + (allMeasured[i] ? [allMeasured[i].shape, allMeasured[i].color, allMeasured[i].textColor].join('|') : 'null'));
    const sigSet = new Set(sigs.map((x) => x.split('=')[1]));
    check(sigSet.size === 5, `runtimes [R1/agent]：五種已知狀態的「點形狀＋點顏色＋文字色」兩兩可區分（實際 ${JSON.stringify(sigs)}）`);
    // 否定對照：把 done 的點改成實心冰青（跟 working 撞形）、unknown 的點改成實心（跟 idle 撞形），
    // agentMismatch 必須抓到。
    const agentNeg = await cdp.eval(`(() => {
      ${AGENT_MEASURE_JS}
      var st = document.createElement('style');
      st.textContent = '.pane-row[data-pane="wJ:s3"] .agent-dot { background: var(--accent) !important; } .pane-row[data-pane="wJ:s4"] .agent-dot { background: var(--text-dim) !important; border-style: solid !important; }';
      document.head.appendChild(st);
      var out = { done: agentMeasure(document.querySelector('.pane-row[data-pane="wJ:s3"]')), unknown: agentMeasure(document.querySelector('.pane-row[data-pane="wJ:s4"]')) };
      st.remove();
      return out;
    })()`);
    check(agentMismatch(agentNeg.done, 'done').length > 0, `否定對照 [R1/agent]：done 的點改成實心冰青時必須判定不符（實際 ${JSON.stringify(agentNeg.done)}）`);
    check(agentMismatch(agentNeg.unknown, 'unknown').length > 0, `否定對照 [R1/agent]：unknown 的點改成實心（跟 idle 撞形）時必須判定不符（實際 ${JSON.stringify(agentNeg.unknown)}）`);
  } finally {
    await stopChrome(chrome, 'chrome-R1');
    await stopPreview(preview, 'preview-R1');
  }
}

// ---------------------------------------------------------------------------
// CH1：task 2.3 fix round 1／fix round 2 回歸——Codex C1（通道狀態持久化）／使用者決定 I2
// （「最後已知」，fix round 2 收斂顏色為 --text-dim，見 (b)）／M3（三態形狀）／Codex C3
// （固定一屏頂列高度不受 runtime 數量與 id 長度影響）／設計審核 N1／N2（fix round 2：燈號
// 放不下時整顆收進「+N」徽章、放得下時完整顯示不截斷，見 (d)）
// ---------------------------------------------------------------------------

// task 2.3 fix round 5（Ruling R27）：固定一屏的 id 改成「可斷行文字＋line-clamp 只顯示一行」
// 之後，被截斷的 id 是多出來的行被藏起來（scrollHeight > clientHeight），不再是橫向溢出
// （scrollWidth 等於 clientWidth）；文字的自然寬度也不能再用 Range 量（換行後 Range 只到框寬）。
// 以下兩個頁面內函式給 CH1 共用：idNaturalWidth 用離屏、同字型同字級同字距、不換行的 span
// 量整串 id 的寬；idIsTruncated 橫向與縱向溢出都算。
const ID_METRIC_HELPERS_JS = `
  function idNaturalWidth(idEl) {
    var cs = getComputedStyle(idEl);
    var probe = document.createElement('span');
    probe.style.cssText = 'position:absolute;left:-9999px;top:0;white-space:nowrap;';
    probe.style.fontFamily = cs.fontFamily;
    probe.style.fontSize = cs.fontSize;
    probe.style.letterSpacing = cs.letterSpacing;
    probe.textContent = idEl.textContent;
    document.body.appendChild(probe);
    var w = probe.getBoundingClientRect().width;
    probe.remove();
    return w;
  }
  function tenChOf(el) {
    var cs = getComputedStyle(el);
    var probe = document.createElement('span');
    probe.style.cssText = 'position:absolute;left:-9999px;top:0;display:inline-block;width:10ch;';
    probe.style.fontFamily = cs.fontFamily;
    probe.style.fontSize = cs.fontSize;
    document.body.appendChild(probe);
    var w = probe.getBoundingClientRect().width;
    probe.remove();
    return w;
  }
  function idIsTruncated(idEl) {
    return idEl.scrollWidth > idEl.clientWidth + 0.5 || idEl.scrollHeight > idEl.clientHeight + 0.5;
  }
`;

async function partChannelFixRound1Regressions() {
  log('=== CH1. 通道狀態持久化＋「最後已知」＋三態形狀＋固定一屏頂列高度與溢出處理（task 2.3 fix round 1／2）===');
  let preview = null;
  let chrome = null;
  try {
    preview = await startPreview({}, 'preview-CH1');
    const url = `http://127.0.0.1:${preview.port}/`;
    chrome = await startChrome(pickPort(19200, [preview.port]), url, 'chrome-CH1');
    const { cdp } = chrome;
    await waitForFirstProjection(cdp, preview.port);

    // --- (e) task 2.3 fix round 4（Codex r3 medium／設計複審 R3-1、R3-2）：一般情況（1536×1024、
    // fixture 的兩個短 id runtime、通道 connected）——燈號列放得下就不能有捲軸軌道（上一輪
    // overflow-x: scroll 常駐 15px 空軌道，把頂列撐到 57px），頂列高度等於 token 且 token 是
    // 45；id 只佔文字本身的寬，id 文字與狀態文字之間不能被拉開（上一輪 id 被拉到約 125px，
    // win 與 connected 之間空約 104px，眼睛會把 connected 跟下一顆的 id 配成一組）。
    // 間距量的是「id 文字的可見右緣」（文字 Range 右緣與 id 框右緣取小者）到下一個可見子節點
    // （通道 connected 時「最後已知」隱藏，下一個就是狀態文字）的左緣，不是 id 框的右緣——
    // 框被拉寬時框右緣仍緊貼狀態文字（gap 6px），只量框會恆真。 ---
    await cdp.send('Emulation.setDeviceMetricsOverride', { width: 1536, height: 1024, deviceScaleFactor: 1, mobile: false });
    await sleep(150);
    const MEASURE_NORMAL_TOPBAR_JS = `(() => {
      var container = document.querySelector('[data-region="topbar"] .topbar-runtimes');
      var topbar = document.querySelector('[data-region="topbar"]');
      var shell = document.querySelector('.shell');
      var lamps = Array.prototype.slice.call(container.querySelectorAll('.runtime-lamp[data-runtime]'));
      var topbarCs = getComputedStyle(topbar);
      return {
        viewport: innerWidth + 'x' + innerHeight,
        tokenPx: parseFloat(getComputedStyle(shell).getPropertyValue('--shell-topbar-h')),
        // fix round 5／R27：token 由「頂列行高（px）＋上下 padding 各 12＋上下框線各 1」推導。
        expectedTokenPx: parseFloat(topbarCs.lineHeight) + 2 * 12 + 2,
        topbarHeight: topbar.getBoundingClientRect().height,
        scrollWidth: container.scrollWidth,
        clientWidth: container.clientWidth,
        offsetHeight: container.offsetHeight,
        clientHeight: container.clientHeight,
        gaps: lamps.map(function (lamp) {
          var idEl = lamp.querySelector('.runtime-lamp-id');
          var range = document.createRange();
          range.selectNodeContents(idEl);
          var textRight = Math.min(range.getBoundingClientRect().right, idEl.getBoundingClientRect().right);
          var next = idEl.nextElementSibling;
          while (next && getComputedStyle(next).display === 'none') next = next.nextElementSibling;
          return {
            runtimeId: lamp.getAttribute('data-runtime'),
            nextClass: next ? next.className : null,
            gap: next ? next.getBoundingClientRect().left - textRight : null,
          };
        }),
      };
    })()`;
    function noScrollbarVerdict(m) {
      return m.scrollWidth <= m.clientWidth && m.offsetHeight === m.clientHeight;
    }
    function gapVerdict(m) {
      return m.gaps.length > 0 && m.gaps.every((g) => g.gap !== null && g.gap <= 16);
    }
    const normal = await cdp.eval(MEASURE_NORMAL_TOPBAR_JS);
    check(
      normal.viewport === '1536x1024' && normal.gaps.length === 2,
      `(e) GIVEN：1536×1024、fixture 的兩個 runtime（實際 ${normal.viewport}、${normal.gaps.length} 顆燈號）`
    );
    check(
      noScrollbarVerdict(normal),
      `(e) 一般情況燈號列沒有水平溢出、沒有捲軸軌道（scrollWidth=${normal.scrollWidth} ≤ clientWidth=${normal.clientWidth}；offsetHeight=${normal.offsetHeight} 應等於 clientHeight=${normal.clientHeight}）`
    );
    check(
      Math.abs(normal.tokenPx - normal.expectedTokenPx) < 0.5 && Math.abs(normal.topbarHeight - normal.tokenPx) < 0.5,
      `(e) 頂列高度應該等於 --shell-topbar-h，且 token＝行高＋24＋2（fix round 5／R27：由字級推導；token=${normal.tokenPx}px，推導值 ${normal.expectedTokenPx}px，實際 ${normal.topbarHeight}px）`
    );
    check(
      normal.tokenPx >= 44 && normal.tokenPx <= 47,
      `(e) 預設字級下頂列維持矮版（fix round 4 為 45px，允許行高推導帶來的 ±2px；實際 token=${normal.tokenPx}px）`
    );
    for (const g of normal.gaps) {
      check(
        g.gap !== null && g.gap <= 16,
        `(e) R3-1：runtime=${g.runtimeId} 的 id 文字右緣到下一個可見文字（${g.nextClass}）距離應該 ≤16px（實際 ${g.gap === null ? null : g.gap.toFixed(1)}px）`
      );
    }

    // 否定對照：兩個上一輪真實出現過的寫法，各自必須讓對應判準失敗——證明判準不是恆真。
    async function measureWithInjectedCss(cssText) {
      await cdp.eval(`(() => {
        var s = document.createElement('style');
        s.id = '__ch1_neg_control';
        s.textContent = ${JSON.stringify(cssText)};
        document.head.appendChild(s);
        return true;
      })()`);
      await sleep(50);
      const m = await cdp.eval(MEASURE_NORMAL_TOPBAR_JS);
      await cdp.eval("(() => { var s = document.getElementById('__ch1_neg_control'); if (s) s.remove(); return true; })()");
      await sleep(50);
      return m;
    }
    const negScroll = await measureWithInjectedCss(
      '@media (min-width: 1200px) and (min-height: 720px) { [data-region="topbar"] .topbar-runtimes { overflow-x: scroll; } }'
    );
    check(
      noScrollbarVerdict(negScroll) === false,
      `(e) 否定對照：燈號列改成 overflow-x: scroll 後，「沒有捲軸軌道」判準必須失敗（實際 offsetHeight=${negScroll.offsetHeight}、clientHeight=${negScroll.clientHeight}）`
    );
    // fix round 5：燈號不再有寫死的 min-width，燈號本身沒有多餘寬度，id 單獨改 flex-grow 1 也長不
    // 出空白；R3-1 的實際成因是「燈號有寫死下限＋id 會長」兩者同時成立，否定對照把兩者一起放回去
    // （fix round 3 的 min-width: 200px＋flex: 1 1 auto）。
    const negGrow = await measureWithInjectedCss(
      '@media (min-width: 1200px) and (min-height: 720px) { [data-region="topbar"] .runtime-lamp { min-width: 200px; } [data-region="topbar"] .runtime-lamp-id { flex: 1 1 auto; } }'
    );
    check(
      gapVerdict(negGrow) === false,
      `(e) 否定對照：燈號 min-width: 200px＋.runtime-lamp-id flex: 1 1 auto（fix round 3 寫法）後，間距判準必須失敗（實際間距 ${JSON.stringify(negGrow.gaps.map((g) => g.gap === null ? null : +g.gap.toFixed(1)))}）`
    );
    const normalAfterNeg = await cdp.eval(MEASURE_NORMAL_TOPBAR_JS);
    check(
      noScrollbarVerdict(normalAfterNeg) && gapVerdict(normalAfterNeg),
      '(e) 否定對照移除注入樣式後，頂列回到無捲軸、間距 ≤16px 的狀態'
    );

    // --- (a) Codex C1：onChannel('disconnected') 之後觸發一次 repaint，底列仍是
    // disconnected、且叉形狀（M3）還在——這是回歸測試的核心：舊實作
    // renderChannelIndicator() 每次整頁重畫都寫死 "connected"，window.repaint()（不是
    // window.onState 觸發，例如使用者操作）會把 onChannel 剛設定的紅色蓋掉。 ---
    await cdp.eval("window.onChannel('disconnected'); true");
    const beforeRepaint = await cdp.eval(`(() => {
      var el = document.getElementById('channel-status');
      var text = el ? el.querySelector('.channel-status-text') : null;
      return text ? text.textContent.trim() : null;
    })()`);
    check(
      beforeRepaint === 'disconnected',
      `(a) GIVEN：onChannel('disconnected') 後 #channel-status 文字應該是 disconnected（實際 ${JSON.stringify(beforeRepaint)}）`
    );

    await cdp.eval("window.repaint(); true");
    await sleep(100);
    const afterRepaint = await cdp.eval(`(() => {
      var el = document.getElementById('channel-status');
      var text = el ? el.querySelector('.channel-status-text') : null;
      var dot = el ? el.querySelector('.channel-status-dot') : null;
      var cs = dot ? getComputedStyle(dot) : null;
      return {
        text: text ? text.textContent.trim() : null,
        backgroundImage: cs ? cs.backgroundImage : null,
      };
    })()`);
    check(
      afterRepaint.text === 'disconnected',
      `(a) Codex C1 回歸：onChannel('disconnected') 後呼叫 window.repaint()（非 WS 觸發的重畫），底列應該仍是 disconnected（實際 ${JSON.stringify(afterRepaint.text)}）——舊實作會被錯誤畫回 connected`
    );
    check(
      !!afterRepaint.backgroundImage && afterRepaint.backgroundImage.indexOf('linear-gradient') !== -1,
      `(a) repaint 後叉形狀（M3）應該還在（.channel-status-dot 的 background-image 應含 linear-gradient；實際 ${afterRepaint.backgroundImage}）`
    );

    // 恢復 connected，當 (b) 的 GIVEN 基準。
    await cdp.eval("window.onChannel('connected'); true");
    await sleep(50);

    // --- (b) 使用者決定 I2／Codex fix round 1 review（N3，控制端裁決採方案 (a)）：通道非
    // connected 時，頂列燈號「四個」子節點（符號、id、「最後已知」、狀態文字）全部改用
    // --text-dim，不是只看外層 .runtime-lamp 的 computed color——上一輪只讀父層，沒讀
    // .runtime-lamp-stale 自己的 color，讓它殘留 --warn（connecting 的顏色）沒被測到，
    // Codex fix round 1 review 抓到這個漏洞。恢復 connected 後全部還原。斷線後另外觸發一次
    // window.repaint()，四個子節點的顏色要再驗一次——確認不是只有 window.onChannel 當下的
    // DOM 補丁對，repaint() 之後 renderTopbar() 用 latestChannelState 重新產生的輸出也要對
    // （這正是 (a) Codex C1 的同一類回歸：任何會重新產生 DOM 的路徑都要維持正確）。win 這顆
    // 燈號自己的連線狀態（HERDR）本身是 connected（fixture GIVEN），色彩差異因此只可能來自
    // 通道（browser↔cockpit）狀態。 ---
    async function readLampColors() {
      return cdp.eval(`(() => {
        var lamp = document.querySelector('[data-region="topbar"] [data-runtime="win"]');
        if (!lamp) return null;
        var dot = lamp.querySelector('.conn-symbol');
        var idEl = lamp.querySelector('.runtime-lamp-id');
        var stale = lamp.querySelector('.runtime-lamp-stale');
        var state = lamp.querySelector('[data-conn-state]');
        return {
          lamp: getComputedStyle(lamp).color,
          dot: dot ? getComputedStyle(dot).color : null,
          id: idEl ? getComputedStyle(idEl).color : null,
          stale: stale ? getComputedStyle(stale).color : null,
          state: state ? getComputedStyle(state).color : null,
          staleVisible: stale ? getComputedStyle(stale).display !== 'none' : null,
          fullText: lamp.textContent,
        };
      })()`);
    }
    function checkAllFourTextDim(colors, contextLabel) {
      check(
        !!colors && colors.lamp === SPEC_COLORS.textDim,
        `(b) ${contextLabel}：.runtime-lamp（外層）顏色應該是 --text-dim（實際 ${colors && colors.lamp}）`
      );
      check(
        !!colors && colors.dot === SPEC_COLORS.textDim,
        `(b) ${contextLabel}：符號（.conn-symbol）顏色應該是 --text-dim（實際 ${colors && colors.dot}）`
      );
      check(
        !!colors && colors.id === SPEC_COLORS.textDim,
        `(b) ${contextLabel}：id（.runtime-lamp-id）顏色應該是 --text-dim（實際 ${colors && colors.id}）`
      );
      check(
        !!colors && colors.stale === SPEC_COLORS.textDim,
        `(b) ${contextLabel}：「最後已知」（.runtime-lamp-stale）顏色應該是 --text-dim，不是 --warn（Codex fix round 1 review／N3；實際 ${colors && colors.stale}）`
      );
      check(
        !!colors && colors.state === SPEC_COLORS.textDim,
        `(b) ${contextLabel}：狀態文字（[data-conn-state]）顏色應該是 --text-dim（實際 ${colors && colors.state}）`
      );
    }

    const connectedBaseline = await readLampColors();
    check(
      !!connectedBaseline && connectedBaseline.lamp === SPEC_COLORS.ok,
      `(b) GIVEN（通道 connected）：win 燈號顏色應該是成功色（實際 ${connectedBaseline && connectedBaseline.lamp}）`
    );
    check(
      !!connectedBaseline && connectedBaseline.staleVisible === false,
      `(b) GIVEN（通道 connected）：燈號不應該顯示「最後已知」（實際 ${connectedBaseline && connectedBaseline.staleVisible}）`
    );

    await cdp.eval("window.onChannel('disconnected'); true");
    const duringOutage = await readLampColors();
    checkAllFourTextDim(duringOutage, '通道斷線期間（onChannel 剛更新，尚未 repaint）');
    check(
      !!duringOutage && duringOutage.staleVisible === true,
      `(b) 通道斷線期間，「最後已知」應該顯示（實際 ${duringOutage && duringOutage.staleVisible}）`
    );
    check(
      !!duringOutage && !!duringOutage.fullText && duringOutage.fullText.indexOf('最後已知') !== -1,
      `(b) 通道斷線期間，燈號文字應該含「最後已知」（實際 ${duringOutage && JSON.stringify(duringOutage.fullText)}）`
    );

    // Codex 回歸：斷線後再 repaint 一次，確認 renderTopbar() 重新產生的輸出，四個子節點的
    // 顏色依然正確。
    await cdp.eval("window.repaint(); true");
    await sleep(100);
    const duringOutageAfterRepaint = await readLampColors();
    checkAllFourTextDim(duringOutageAfterRepaint, '通道斷線期間、repaint 之後');
    check(
      !!duringOutageAfterRepaint && duringOutageAfterRepaint.staleVisible === true,
      `(b) repaint 之後，「最後已知」應該仍然顯示（實際 ${duringOutageAfterRepaint && duringOutageAfterRepaint.staleVisible}）`
    );

    await cdp.eval("window.onChannel('connected'); true");
    const afterRestore = await readLampColors();
    check(
      !!afterRestore && afterRestore.lamp === SPEC_COLORS.ok,
      `(b) 通道恢復 connected 後，win 燈號顏色應該還原成成功色（實際 ${afterRestore && afterRestore.lamp}）`
    );
    check(
      !!afterRestore && afterRestore.staleVisible === false,
      `(b) 通道恢復 connected 後，「最後已知」應該還原成隱藏（實際 ${afterRestore && afterRestore.staleVisible}）`
    );

    // --- (c) 使用者決定 M3：連線三態（connected／connecting／disconnected）的計算樣式應該
    // 可以互相區分——不只是文字不同，符號本身也不同形狀（灰階／餘光也能辨識）。用三個離屏的
    // .conn-symbol 節點量 computed style，不依賴目前頁面實際處於哪個狀態。 ---
    const shapes = await cdp.eval(`(() => {
      function shapeOf(el) {
        var cs = getComputedStyle(el);
        return {
          borderRadius: cs.borderRadius,
          borderWidth: cs.borderTopWidth,
          hasGradient: cs.backgroundImage !== 'none' && cs.backgroundImage.indexOf('gradient') !== -1,
        };
      }
      var wrap = document.createElement('div');
      wrap.style.position = 'absolute';
      wrap.style.left = '-9999px';
      wrap.innerHTML =
        '<span class="runtime-lamp runtime-lamp-connected"><span class="runtime-lamp-dot conn-symbol"></span></span>' +
        '<span class="runtime-lamp runtime-lamp-connecting"><span class="runtime-lamp-dot conn-symbol"></span></span>' +
        '<span class="runtime-lamp runtime-lamp-disconnected"><span class="runtime-lamp-dot conn-symbol"></span></span>';
      document.body.appendChild(wrap);
      var dots = wrap.querySelectorAll('.conn-symbol');
      var result = {
        connected: shapeOf(dots[0]),
        connecting: shapeOf(dots[1]),
        disconnected: shapeOf(dots[2]),
      };
      wrap.remove();
      return result;
    })()`);
    check(
      shapes.connected.hasGradient === false && shapes.connected.borderWidth === '0px',
      `(c) connected 應該是實心圓（無 background 漸層、無邊框；實際 ${JSON.stringify(shapes.connected)}）`
    );
    check(
      shapes.connecting.hasGradient === false && shapes.connecting.borderWidth !== '0px',
      `(c) connecting 應該是空心圓（無 background 漸層、有邊框；實際 ${JSON.stringify(shapes.connecting)}）`
    );
    check(
      shapes.disconnected.hasGradient === true,
      `(c) disconnected 應該是用 background 漸層畫的叉（實際 ${JSON.stringify(shapes.disconnected)}）`
    );
    check(
      shapes.connected.borderWidth !== shapes.connecting.borderWidth,
      `(c) connected 與 connecting 的邊框寬度應該不同，計算樣式可以區分兩者（實際 connected=${shapes.connected.borderWidth}, connecting=${shapes.connecting.borderWidth}）`
    );

    // --- (d) Codex C3／R26（換設計：拿掉「+N」徽章與 JS 量測，改純 CSS：燈號永遠全部顯示，
    // 空間不夠先縮 id，再不夠橫向捲動）：固定一屏（1200×720）、5 個 runtime、每個 id 長 60
    // 字元（其中一個是 disconnected，對照設計複審 R2-1 的原始情境），三個情境——(a) 通道
    // 正常；(b) 只呼叫 window.onChannel('disconnected')、不 repaint（Codex 抓到上一輪的
    // adjustTopbarRuntimeOverflow() 只在 paint() 後跑，這個路徑會漏算；R26 的純 CSS 方案
    // 天生沒有這個問題——data-channel-state 屬性一改，CSS 立刻重新算版面，不需要任何 JS
    // 觸發）；(c) 視窗 1200→1536→1200 來回縮放後（Codex 抓到上一輪沒有 resize listener；
    // 同理，純 CSS 方案不需要）。三個情境都驗證同一組不變量。 ---
    await cdp.send('Emulation.setDeviceMetricsOverride', { width: 1200, height: 720, deviceScaleFactor: 1, mobile: false });
    await sleep(150);
    const viewportActual = await cdp.eval('({ w: window.innerWidth, h: window.innerHeight })');
    check(
      viewportActual.w === 1200 && viewportActual.h === 720,
      `(d) Emulation.setDeviceMetricsOverride 應該把 innerWidth/innerHeight 精準設成 1200x720（實際 ${JSON.stringify(viewportActual)}）`
    );

    const manyLongInject = await injectState(cdp, craftManyLongRuntimesState());
    check(manyLongInject && manyLongInject.ok === true, '(d) 注入 5 個長 id runtime（含一個 disconnected）的特製投影');
    const expectedRuntimeIds = [0, 1, 2, 3, 4].map((i) => ('runtime-' + i + '-').padEnd(60, 'x').slice(0, 60));
    const disconnectedRuntimeId = expectedRuntimeIds[0];

    // 逐一讀取「每顆預期的燈號」：存不存在／display／是否完整落在可捲動範圍內／捲過去之後
    // 符號與狀態文字是否真的進到可視範圍——不是只看「目前視窗座標」，是實際把
    // `.topbar-runtimes` 捲到那顆燈號的位置再量，才是真正驗證「捲得到」而不是「剛好在視窗
    // 內」。捲完之後把 scrollLeft 還原，不影響下一顆燈號的量測基準。
    async function readLampsFull() {
      return cdp.eval(`(() => {
        ${ID_METRIC_HELPERS_JS}
        var container = document.querySelector('[data-region="topbar"] .topbar-runtimes');
        var topbar = document.querySelector('[data-region="topbar"]');
        var shell = document.querySelector('.shell');
        var tokenPx = parseFloat(getComputedStyle(shell).getPropertyValue('--shell-topbar-h'));
        var cs = getComputedStyle(container);
        var overflowing = container.scrollWidth > container.clientWidth + 0.5;
        var containerRect = container.getBoundingClientRect();
        var originalScrollLeft = container.scrollLeft;
        var maxScrollLeft = Math.max(0, container.scrollWidth - container.clientWidth);

        // fix round 4：捲軸帶＝容器 padding box 底下、clientHeight 以外那一截（沒有捲軸時
        // 高度 0）。10ch 用 id 自己的等寬字型量（離屏 span，同字型同字級）。
        var scrollbarBand = null;
        if (container.offsetHeight > container.clientHeight) {
          var bandTop = containerRect.top + container.clientTop + container.clientHeight;
          scrollbarBand = { top: bandTop, bottom: containerRect.bottom - parseFloat(cs.borderBottomWidth) };
        }
        var anyId = container.querySelector('.runtime-lamp-id');
        var tenChPx = null;
        if (anyId) {
          var idCs = getComputedStyle(anyId);
          var chProbe = document.createElement('span');
          chProbe.style.cssText = 'position:absolute;left:-9999px;top:0;display:inline-block;width:10ch;';
          chProbe.style.fontFamily = idCs.fontFamily;
          chProbe.style.fontSize = idCs.fontSize;
          document.body.appendChild(chProbe);
          tenChPx = chProbe.getBoundingClientRect().width;
          chProbe.remove();
        }

        var expectedIds = ${JSON.stringify(expectedRuntimeIds)};
        var lamps = expectedIds.map(function (rid) {
          var lamp = container.querySelector('[data-runtime="' + CSS.escape(rid) + '"]');
          if (!lamp) {
            return { runtimeId: rid, exists: false };
          }
          var lampCs = getComputedStyle(lamp);
          var dot = lamp.querySelector('.conn-symbol');
          var stateEl = lamp.querySelector('[data-conn-state]');
          var rectBefore = lamp.getBoundingClientRect();
          var idEl = lamp.querySelector('.runtime-lamp-id');
          var idTextWidth = idNaturalWidth(idEl);
          var idBoxWidth = idEl.getBoundingClientRect().width;
          // fix round 5：id 第一行文字的可見右緣到下一個可見子節點的距離（R3-1 在縮小狀態的版本：
          // 第一行若斷在連字號，框裡會留一大段空白）。
          var idRange = document.createRange();
          idRange.selectNodeContents(idEl);
          var firstLine = idRange.getClientRects()[0];
          var idVisibleRight = firstLine ? Math.min(firstLine.right, idEl.getBoundingClientRect().right) : idEl.getBoundingClientRect().left;
          var nextEl = idEl.nextElementSibling;
          while (nextEl && getComputedStyle(nextEl).display === 'none') nextEl = nextEl.nextElementSibling;
          var idGap = nextEl ? nextEl.getBoundingClientRect().left - idVisibleRight : null;
          var idTruncated = idIsTruncated(idEl);
          var overlapsScrollbar = !!scrollbarBand && rectBefore.bottom > scrollbarBand.top + 0.5 && rectBefore.top < scrollbarBand.bottom - 0.5;
          var contentLeft = rectBefore.left - containerRect.left + container.scrollLeft;
          var contentRight = contentLeft + rectBefore.width;
          var target = Math.max(0, Math.min(maxScrollLeft, contentLeft));
          container.scrollLeft = target;
          var cRectNow = container.getBoundingClientRect();
          var dotRect = dot ? dot.getBoundingClientRect() : null;
          var stateRect = stateEl ? stateEl.getBoundingClientRect() : null;
          var dotReachable = !!dotRect && dotRect.left >= cRectNow.left - 0.5 && dotRect.right <= cRectNow.right + 0.5;
          var stateReachable = !!stateRect && stateRect.left >= cRectNow.left - 0.5 && stateRect.right <= cRectNow.right + 0.5;
          return {
            runtimeId: rid,
            exists: true,
            display: lampCs.display,
            withinScrollRange: contentLeft >= -0.5 && contentRight <= container.scrollWidth + 0.5,
            dotReachable: dotReachable,
            stateReachable: stateReachable,
            lampTop: rectBefore.top,
            lampBottom: rectBefore.bottom,
            overlapsScrollbar: overlapsScrollbar,
            idTextWidth: idTextWidth,
            idBoxWidth: idBoxWidth,
            idTruncated: idTruncated,
            idGap: idGap,
          };
        });

        container.scrollLeft = originalScrollLeft;

        return {
          lampCount: container.querySelectorAll('.runtime-lamp[data-runtime]').length,
          overflowX: cs.overflowX,
          overflowing: overflowing,
          topbarHeight: topbar.getBoundingClientRect().height,
          tokenPx: tokenPx,
          scrollbarBand: scrollbarBand,
          tenChPx: tenChPx,
          containerTop: containerRect.top,
          containerBottom: containerRect.bottom,
          topbarTop: topbar.getBoundingClientRect().top,
          topbarBottom: topbar.getBoundingClientRect().bottom,
          lamps: lamps,
        };
      })()`);
    }

    function checkFullLampState(result, contextLabel) {
      check(
        result.lampCount === expectedRuntimeIds.length,
        `(d) ${contextLabel}：燈號數量應該是 ${expectedRuntimeIds.length}（實際 ${result.lampCount}）——每顆燈號都存在，沒有被整顆移除`
      );
      for (const lamp of result.lamps) {
        check(lamp.exists === true, `(d) ${contextLabel}：runtime=${lamp.runtimeId} 的燈號應該存在（沒有被移除）`);
        if (!lamp.exists) continue;
        check(
          lamp.display !== 'none',
          `(d) ${contextLabel}：runtime=${lamp.runtimeId} 的燈號不應該是 display: none（實際 ${lamp.display}）`
        );
        check(
          lamp.withinScrollRange === true,
          `(d) ${contextLabel}：runtime=${lamp.runtimeId} 的燈號應該完整落在容器的可捲動範圍內（不是被推到 scrollWidth 之外；實際 withinScrollRange=${lamp.withinScrollRange}）`
        );
        check(
          lamp.dotReachable === true,
          `(d) ${contextLabel}：runtime=${lamp.runtimeId} 的符號捲過去之後應該進到可視範圍內（實際 ${lamp.dotReachable}）`
        );
        check(
          lamp.stateReachable === true,
          `(d) ${contextLabel}：runtime=${lamp.runtimeId} 的狀態文字捲過去之後應該進到可視範圍內（實際 ${lamp.stateReachable}）`
        );
        // fix round 4：捲軸帶不得蓋到燈號（設計複審警告：只改 overflow-x: auto 不改 padding，
        // 捲軸會蓋住燈號下半部）；id 至少保有 min(10ch, 文字自然寬)（R3-3）。
        check(
          lamp.overlapsScrollbar === false,
          `(d) ${contextLabel}：runtime=${lamp.runtimeId} 的燈號不應該與捲軸帶重疊（燈號 y ${lamp.lampTop}～${lamp.lampBottom}，捲軸帶 ${JSON.stringify(result.scrollbarBand)}）`
        );
        // 截斷偵測器的正對照（fix round 5）：60 字元 id 在 1200 寬一定放不下，偵測器必須報
        // 截斷——(d)-N2 的「沒被截斷」判準靠同一個偵測器，這條證明它在 line-clamp 下不是恆 false。
        check(
          lamp.idTruncated === true,
          `(d) ${contextLabel}：runtime=${lamp.runtimeId} 的 60 字元 id 應該被偵測為截斷（偵測器正對照；實際 ${lamp.idTruncated}）`
        );
        check(
          lamp.idGap !== null && lamp.idGap <= 16,
          `(d) ${contextLabel}：R3-1：runtime=${lamp.runtimeId} 的 id 第一行文字到下一個可見文字距離應該 ≤16px（實際 ${lamp.idGap === null ? null : lamp.idGap.toFixed(1)}px）`
        );
        const idFloor = Math.min(result.tenChPx, lamp.idTextWidth);
        check(
          typeof result.tenChPx === 'number' && lamp.idBoxWidth >= idFloor - 0.5,
          `(d) ${contextLabel}：R3-3：runtime=${lamp.runtimeId} 的 id 寬度應該 ≥ min(10ch=${result.tenChPx}px, 文字寬 ${lamp.idTextWidth.toFixed(1)}px)（實際 ${lamp.idBoxWidth.toFixed(1)}px）`
        );
      }
      if (result.overflowing) {
        check(
          result.overflowX === 'auto' || result.overflowX === 'scroll',
          `(d) ${contextLabel}：燈號列溢出時，computed overflow-x 必須是 auto 或 scroll（實際 ${result.overflowX}）`
        );
        check(
          result.scrollbarBand !== null,
          `(d) ${contextLabel}：燈號列溢出時應該出現可見的橫向捲軸（不是無聲裁切；實際捲軸帶 ${JSON.stringify(result.scrollbarBand)}）`
        );
      }
      // fix round 4：燈號列（含捲軸）必須落在頂列框內——頂列寫死高度後，若燈號列自己被捲軸
      // 撐高，會溢出頂列框、畫到下方區塊上，頂列高度卻照樣等於 token，上面那條量不到。
      check(
        result.containerTop >= result.topbarTop - 0.5 && result.containerBottom <= result.topbarBottom + 0.5,
        `(d) ${contextLabel}：燈號列（含捲軸）應該落在頂列框內（燈號列 y ${result.containerTop}～${result.containerBottom}，頂列 y ${result.topbarTop}～${result.topbarBottom}）`
      );
      check(
        Math.abs(result.topbarHeight - result.tokenPx) < 0.5,
        `(d) ${contextLabel}：頂列實際高度應該等於 --shell-topbar-h（token=${result.tokenPx}px，實際 ${result.topbarHeight}px）`
      );
      const disconnectedLamp = result.lamps.find((l) => l.runtimeId === disconnectedRuntimeId);
      check(
        !!disconnectedLamp && disconnectedLamp.exists === true,
        `(d) ${contextLabel}：斷線的 runtime（${disconnectedRuntimeId}）的燈號必定存在（R2-1：不可以因為排不下就被藏起來）`
      );
    }

    // (a) connected。
    const resultA = await readLampsFull();
    checkFullLampState(resultA, '(a) 通道正常');

    // (b) 只呼叫 window.onChannel('disconnected')，不呼叫 window.repaint()——這是 Codex 抓到
    // 上一輪漏掉的真實事件路徑（通道真的斷線時，channel.js 就是這樣呼叫的，不會有任何
    // repaint）。
    await cdp.eval("window.onChannel('disconnected'); true");
    const resultB = await readLampsFull();
    checkFullLampState(resultB, "(b) window.onChannel('disconnected')，不 repaint");
    await cdp.eval("window.onChannel('connected'); true");

    // (c) 視窗 1200→1536→1200 來回縮放（不觸發任何重畫或 onChannel）——驗證純 CSS 版面本身
    // 就會跟著視窗大小即時重算，不需要 resize listener。
    await cdp.send('Emulation.setDeviceMetricsOverride', { width: 1536, height: 1024, deviceScaleFactor: 1, mobile: false });
    await sleep(150);
    await cdp.send('Emulation.setDeviceMetricsOverride', { width: 1200, height: 720, deviceScaleFactor: 1, mobile: false });
    await sleep(150);
    const resultC = await readLampsFull();
    checkFullLampState(resultC, '(c) 視窗 1200→1536→1200 來回縮放後');

    // 否定對照：把燈號列暫時改成 overflow-x: hidden（不能捲動），至少要有一顆燈號的符號或
    // 狀態文字不在可視範圍內——證明上面「捲得到」這幾條斷言不是恆真，偵測器真的有牙齒。
    // fix round 5：燈號下限改由內容推導後，通道正常時 5 顆 60 字元 id 縮到 id 約 105px 就放得下、
    // 不溢出；否定對照要在真的溢出的狀態（通道斷線、燈號多了「最後已知」）下做，做完還原。
    await cdp.eval("window.onChannel('disconnected'); true");
    await sleep(50);
    const negControl = await cdp.eval(`(() => {
      var container = document.querySelector('[data-region="topbar"] .topbar-runtimes');
      var original = container.style.overflowX;
      container.style.overflowX = 'hidden';
      container.scrollLeft = 0;
      var containerRect = container.getBoundingClientRect();
      var lamps = Array.prototype.slice.call(container.querySelectorAll('.runtime-lamp[data-runtime]'));
      var reachable = lamps.map(function (lamp) {
        var dot = lamp.querySelector('.conn-symbol');
        var stateEl = lamp.querySelector('[data-conn-state]');
        var dotRect = dot.getBoundingClientRect();
        var stateRect = stateEl.getBoundingClientRect();
        var dotOk = dotRect.left >= containerRect.left - 0.5 && dotRect.right <= containerRect.right + 0.5;
        var stateOk = stateRect.left >= containerRect.left - 0.5 && stateRect.right <= containerRect.right + 0.5;
        return dotOk && stateOk;
      });
      container.style.overflowX = original;
      return { anyUnreachable: reachable.some(function (ok) { return !ok; }) };
    })()`);
    await cdp.eval("window.onChannel('connected'); true");
    check(
      negControl.anyUnreachable === true,
      `否定對照：把燈號列暫時改成 overflow-x: hidden 後，至少應該有一顆燈號的符號或狀態文字不在可視範圍內（實際 anyUnreachable=${negControl.anyUnreachable}）——證明「捲得到」這幾條斷言不是恆真`
    );

    // N2（設計審核 fix round 2，這裡沿用）：兩個同前綴的中等長度 id，空間足夠時應該完整顯示、
    // 彼此分得出來——用 idIsTruncated（fix round 5：橫向或縱向溢出，line-clamp 之後截斷是多出來
    // 的行被藏起來；正對照見 (d) 的「60 字元 id 應該被偵測為截斷」）判斷有沒有被截斷，不是比對
    // textContent（textContent 一律是完整原字串，CSS ellipsis 只影響畫面呈現，不影響
    // textContent，比 textContent 測不出「有沒有被截斷」）。
    const prefixInject = await injectState(cdp, craftSamePrefixRuntimesState());
    check(prefixInject && prefixInject.ok === true, '(d)-N2 注入兩個同前綴長 id runtime 的特製投影');
    const prefixState = await cdp.eval(`(() => {
      ${ID_METRIC_HELPERS_JS}
      var lamps = Array.prototype.slice.call(document.querySelectorAll('[data-region="topbar"] .runtime-lamp[data-runtime]'));
      return lamps.map(function (l) {
        var idEl = l.querySelector('.runtime-lamp-id');
        return {
          runtimeId: l.getAttribute('data-runtime'),
          idTextContent: idEl.textContent,
          idTruncated: idIsTruncated(idEl),
        };
      });
    })()`);
    check(prefixState.length === 2, `(d)-N2 應該有兩顆燈號（實際 ${prefixState.length}）`);
    for (const p of prefixState) {
      check(
        p.idTruncated === false,
        `(d)-N2 空間足夠時，id 不應該被視覺截斷（runtime=${p.runtimeId}，實際 idTruncated=${p.idTruncated}）`
      );
    }
    if (prefixState.length === 2) {
      check(
        prefixState[0].idTextContent !== prefixState[1].idTextContent,
        `(d)-N2 兩個同前綴的 id 應該可以彼此區分（實際 ${JSON.stringify(prefixState[0].idTextContent)} vs ${JSON.stringify(prefixState[1].idTextContent)}）`
      );
    }

    // --- (f) task 2.3 fix round 5（Codex r4 兩條 medium／Ruling R27）：文字放大與較寬字型下，頂列
    // 不得裁切或溢出，id 不得被壓到 10ch 以下。兩種擾動 × 兩種畫面：
    //   擾動 1「文字放大 200%」：注入 html { font-size: 200% }，並把 :root 的 --fs-* 四個字級
    //     token 各乘 2。只注入前者不會放大任何字——style.css 的字級 token 是 px（R20 定案），根字級
    //     只影響 rem；瀏覽器的最小字級與「只放大文字」會直接放大 px 字級，這裡用 token 加倍模擬，
    //     並先斷言產品名的計算字級真的變成兩倍（前提斷言，避免情境本身無效而恆綠）。
    //   擾動 2「較寬字型」：頂列注入較寬的 fallback 字型（Courier New）＋字距 0.3em——ch 只量
    //     「0」字的前進寬，不含字距，所以「最後已知」、狀態文字與 id 文字都變寬，10ch 不變。
    //   畫面 A：1536×1024、fixture 兩個 runtime、通道 connected。
    //   畫面 B：1200×720、5 個 60 字元 id、onChannel('disconnected') 不重畫（最寬的燈號）。
    // 每個組合都驗：產品名完整在頂列框內；每顆燈號垂直完整在頂列框內、也在燈號列的可視高度內
    // （不被捲軸蓋、不被 overflow-y 裁）；頂列高度＝token；其他區塊不與頂列重疊；每顆燈號的
    // 子節點都在燈號框內、燈號彼此不重疊；每顆 id 框寬 ≥ min(10ch, 文字自然寬)；溢出時捲軸可見、
    // 每顆燈號捲得到。否定對照：把 fix round 4 的寫死值放回去（燈號 min-width: 220px、頂列
    // height: 45px），擾動 1 的畫面 B 必須判定失敗。
    const MEASURE_TOPBAR_INVARIANTS_JS = `(() => {
      ${ID_METRIC_HELPERS_JS}
      function rect(el) { var b = el.getBoundingClientRect(); return { left: b.left, top: b.top, right: b.right, bottom: b.bottom, width: b.width, height: b.height }; }
      var topbar = document.querySelector('[data-region="topbar"]');
      var container = topbar.querySelector('.topbar-runtimes');
      var appName = topbar.querySelector('.app-name');
      var shell = document.querySelector('.shell');
      var tb = rect(topbar);
      var cRect = rect(container);
      var bandTop = cRect.top + container.clientTop;
      var bandBottom = bandTop + container.clientHeight;
      var hasScrollbar = container.offsetHeight > container.clientHeight;
      var maxScrollLeft = Math.max(0, container.scrollWidth - container.clientWidth);
      var originalScrollLeft = container.scrollLeft;
      var others = Array.prototype.slice.call(document.querySelectorAll('[data-region]'))
        .filter(function (el) { return el !== topbar && !topbar.contains(el); })
        .map(function (el) { var r = rect(el); return { region: el.getAttribute('data-region'), r: r }; })
        .filter(function (o) { return o.r.width > 0 && o.r.height > 0; });
      var lampEls = Array.prototype.slice.call(container.querySelectorAll('.runtime-lamp[data-runtime]'));
      var lamps = lampEls.map(function (lamp) {
        var lr = rect(lamp);
        var idEl = lamp.querySelector('.runtime-lamp-id');
        var childrenInside = Array.prototype.slice.call(lamp.children)
          .filter(function (c) { return getComputedStyle(c).display !== 'none'; })
          .every(function (c) { var r = rect(c); return r.left >= lr.left - 0.5 && r.right <= lr.right + 0.5; });
        var contentLeft = lr.left - cRect.left + container.scrollLeft;
        container.scrollLeft = Math.max(0, Math.min(maxScrollLeft, contentLeft));
        var cNow = rect(container);
        var dotR = rect(lamp.querySelector('.conn-symbol'));
        var stateR = rect(lamp.querySelector('[data-conn-state]'));
        var reachable = dotR.left >= cNow.left - 0.5 && dotR.right <= cNow.right + 0.5 && stateR.left >= cNow.left - 0.5 && stateR.right <= cNow.right + 0.5;
        container.scrollLeft = originalScrollLeft;
        return {
          runtimeId: lamp.getAttribute('data-runtime').slice(0, 16),
          contentLeft: contentLeft,
          contentRight: contentLeft + lr.width,
          top: lr.top,
          bottom: lr.bottom,
          childrenInside: childrenInside,
          reachable: reachable,
          idBoxWidth: rect(idEl).width,
          idNatural: idNaturalWidth(idEl),
          tenCh: tenChOf(idEl),
        };
      });
      return {
        appNameFontPx: parseFloat(getComputedStyle(appName).fontSize),
        stateTextWidth: lampEls.length ? rect(lampEls[0].querySelector('[data-conn-state]')).width : null,
        topbar: tb,
        appName: rect(appName),
        tokenPx: parseFloat(getComputedStyle(shell).getPropertyValue('--shell-topbar-h')),
        bandTop: bandTop,
        bandBottom: bandBottom,
        overflowing: container.scrollWidth > container.clientWidth + 0.5,
        hasScrollbar: hasScrollbar,
        others: others,
        lamps: lamps,
      };
    })()`;
    // 回傳違反清單（空陣列＝全部成立），主情境逐條 check，否定對照只要求「至少一條」。
    function topbarInvariantViolations(m) {
      const v = [];
      const inside = (r, box) => r.left >= box.left - 0.5 && r.right <= box.right + 0.5 && r.top >= box.top - 0.5 && r.bottom <= box.bottom + 0.5;
      if (!inside(m.appName, m.topbar)) v.push(`產品名不在頂列框內（產品名 ${JSON.stringify(m.appName)}，頂列 ${JSON.stringify(m.topbar)}）`);
      if (Math.abs(m.topbar.height - m.tokenPx) >= 0.5) v.push(`頂列高度 ${m.topbar.height}px ≠ token ${m.tokenPx}px`);
      for (const o of m.others) {
        if (rectsOverlap(o.r, m.topbar)) v.push(`區塊 ${o.region} 與頂列重疊（${JSON.stringify(o.r)}）`);
      }
      if (m.overflowing && !m.hasScrollbar) v.push('燈號列溢出卻沒有可見捲軸');
      const sorted = m.lamps.slice().sort((a, b) => a.contentLeft - b.contentLeft);
      for (let i = 0; i < sorted.length; i += 1) {
        const l = sorted[i];
        if (l.top < m.topbar.top - 0.5 || l.bottom > m.topbar.bottom + 0.5) v.push(`燈號 ${l.runtimeId} 垂直超出頂列框（y ${l.top}～${l.bottom}）`);
        if (l.top < m.bandTop - 0.5 || l.bottom > m.bandBottom + 0.5) v.push(`燈號 ${l.runtimeId} 超出燈號列可視高度（被裁或被捲軸蓋；y ${l.top}～${l.bottom}，可視 ${m.bandTop}～${m.bandBottom}）`);
        if (!l.childrenInside) v.push(`燈號 ${l.runtimeId} 的子節點溢出燈號框（會與鄰居重疊）`);
        if (i > 0 && l.contentLeft < sorted[i - 1].contentRight - 0.5) v.push(`燈號 ${l.runtimeId} 與前一顆重疊`);
        if (!l.reachable) v.push(`燈號 ${l.runtimeId} 捲不到（符號或狀態文字無法進入可視範圍）`);
        const floor = Math.min(l.tenCh, l.idNatural);
        if (l.idBoxWidth < floor - 0.5) v.push(`燈號 ${l.runtimeId} 的 id 寬 ${l.idBoxWidth.toFixed(1)}px < min(10ch=${l.tenCh.toFixed(1)}, 文字寬 ${l.idNatural.toFixed(1)})`);
      }
      return v;
    }
    async function injectStyle(id, cssText) {
      await cdp.eval(`(() => { var s = document.getElementById(${JSON.stringify(id)}); if (!s) { s = document.createElement('style'); s.id = ${JSON.stringify(id)}; document.head.appendChild(s); } s.textContent = ${JSON.stringify(cssText)}; return true; })()`);
      await sleep(100);
    }
    async function removeStyle(id) {
      await cdp.eval(`(() => { var s = document.getElementById(${JSON.stringify(id)}); if (s) s.remove(); return true; })()`);
      await sleep(100);
    }
    const baseFs = await cdp.eval(`(() => {
      var cs = getComputedStyle(document.documentElement);
      return ['--fs-title', '--fs-panel', '--fs-dense', '--fs-meta'].map(function (n) { return [n, parseFloat(cs.getPropertyValue(n))]; });
    })()`);
    const textScaleCss =
      'html { font-size: 200%; } :root { ' + baseFs.map(([n, px]) => `${n}: ${px * 2}px;`).join(' ') + ' }';
    const wideFontCss = '[data-region="topbar"] { font-family: "Courier New", monospace; letter-spacing: 0.3em; }';
    const disturbances = [
      { tag: '文字放大 200%', css: textScaleCss, isTextScale: true },
      { tag: '較寬字型（Courier New＋字距 0.3em）', css: wideFontCss, isTextScale: false },
    ];
    async function setScreenA() {
      await cdp.send('Emulation.setDeviceMetricsOverride', { width: 1536, height: 1024, deviceScaleFactor: 1, mobile: false });
      await sleep(150);
      await cdp.eval("window.onChannel('connected'); true");
      const r = await injectState(cdp, loadFixture());
      check(r && r.ok === true, '(f) 畫面 A：注入 fixture（兩個 runtime）');
      await sleep(100);
    }
    async function setScreenB() {
      await cdp.send('Emulation.setDeviceMetricsOverride', { width: 1200, height: 720, deviceScaleFactor: 1, mobile: false });
      await sleep(150);
      const r = await injectState(cdp, craftManyLongRuntimesState());
      check(r && r.ok === true, '(f) 畫面 B：注入 5 個 60 字元 id');
      await cdp.eval("window.onChannel('disconnected'); true");
      await sleep(100);
    }
    const screens = [
      { tag: '畫面 A 1536×1024 兩 runtime', set: setScreenA, runNegatives: false },
      { tag: '畫面 B 1200×720 五長 id 斷線不重畫', set: setScreenB, runNegatives: true },
    ];
    for (const scr of screens) {
      await scr.set();
      const baseline = await cdp.eval(MEASURE_TOPBAR_INVARIANTS_JS);
      for (const d of disturbances) {
        await injectStyle('__ch1_f_disturb', d.css);
        const m = await cdp.eval(MEASURE_TOPBAR_INVARIANTS_JS);
        const label = `(f) ${d.tag}／${scr.tag}`;
        if (d.isTextScale) {
          check(
            Math.abs(m.appNameFontPx - baseline.appNameFontPx * 2) < 0.5,
            `${label}：前提——產品名的計算字級應該變成兩倍（基準 ${baseline.appNameFontPx}px，實際 ${m.appNameFontPx}px）`
          );
        } else {
          check(
            m.stateTextWidth > baseline.stateTextWidth + 5,
            `${label}：前提——狀態文字應該明顯變寬（基準 ${baseline.stateTextWidth}px，實際 ${m.stateTextWidth}px）`
          );
        }
        const violations = topbarInvariantViolations(m);
        check(
          violations.length === 0,
          `${label}：頂列不變量全部成立（token=${m.tokenPx}px、頂列高 ${m.topbar.height}px、溢出=${m.overflowing}、捲軸=${m.hasScrollbar}；違反 ${violations.length} 條${violations.length ? '：' + violations.slice(0, 4).join('；') : ''}）`
        );
        if (d.isTextScale && scr.runNegatives) {
          const negatives = [
            { tag: '燈號 min-width: 220px（fix round 4 的寫死下限）', css: '@media (min-width: 1200px) and (min-height: 720px) { .runtime-lamp { min-width: 220px; } }' },
            { tag: '頂列 height: 45px（寫死高度）', css: '@media (min-width: 1200px) and (min-height: 720px) { [data-region="topbar"] { height: 45px; } }' },
          ];
          for (const n of negatives) {
            await injectStyle('__ch1_f_negative', n.css);
            const nm = await cdp.eval(MEASURE_TOPBAR_INVARIANTS_JS);
            const nv = topbarInvariantViolations(nm);
            check(
              nv.length > 0,
              `${label}：否定對照「${n.tag}」必須讓不變量判定失敗（實際違反 ${nv.length} 條${nv.length ? '：' + nv.slice(0, 2).join('；') : ''}）`
            );
            await removeStyle('__ch1_f_negative');
          }
        }
        await removeStyle('__ch1_f_disturb');
      }
    }
    await cdp.eval("window.onChannel('connected'); true");

    // R2-2（設計審核，fix round 2；fix round 3 補段）：390 寬、長 id、通道斷線時，產品名與
    // 每顆燈號都不應該重疊——這個斷點用基本規則（.runtime-lamp 允許內部換行），不是固定一屏
    // 的橫向捲動規則。
    await cdp.send('Emulation.setDeviceMetricsOverride', { width: 390, height: 900, deviceScaleFactor: 1, mobile: false });
    await sleep(150);
    const narrowInject = await injectState(cdp, craftLongIdNarrowState());
    check(narrowInject && narrowInject.ok === true, '(d)-R2-2 注入 390 寬長 id 情境的特製投影');
    await cdp.eval("window.onChannel('disconnected'); true");
    await sleep(50);
    const narrowRects = await cdp.eval(`(() => {
      var appName = document.querySelector('.app-name');
      var lamps = Array.prototype.slice.call(document.querySelectorAll('[data-region="topbar"] .runtime-lamp[data-runtime]'));
      return {
        appNameRect: appName.getBoundingClientRect(),
        lampRects: lamps.map(function (l) {
          var r = l.getBoundingClientRect();
          return { runtimeId: l.getAttribute('data-runtime'), left: r.left, top: r.top, right: r.right, bottom: r.bottom, width: r.width, height: r.height };
        }),
      };
    })()`);
    for (const lampRect of narrowRects.lampRects) {
      check(
        !rectsOverlap(narrowRects.appNameRect, lampRect),
        `(d)-R2-2 390 寬、通道斷線：產品名不應該與燈號（runtime=${lampRect.runtimeId}）重疊（app-name=${JSON.stringify(narrowRects.appNameRect)}，lamp=${JSON.stringify(lampRect)}）`
      );
    }
  } finally {
    await stopChrome(chrome, 'chrome-CH1');
    await stopPreview(preview, 'preview-CH1');
  }
}

// ---------------------------------------------------------------------------
// D1：dashboard/done不使用成功色（task 3.3）
// ---------------------------------------------------------------------------

async function partDoneColor() {
  log('=== D1. done 不使用成功色 ===');
  let preview = null;
  let chrome = null;
  try {
    preview = await startPreview({}, 'preview-D1');
    const url = `http://127.0.0.1:${preview.port}/`;
    chrome = await startChrome(pickPort(19090, [preview.port]), url, 'chrome-D1');
    const { cdp } = chrome;
    await waitForFirstProjection(cdp, preview.port);
    // fixture 的 wJ:p2 agent_status 是 done、ops-1 task status 是 completed，不需要注入。

    // fix round 1／Codex F5：從 --ok token 讀（task 2.2 才會定案落地），讀不到（現行 style.css
    // 還沒有這個變數）就退回 spec 色值 #39D5AC（SPEC_COLORS.ok）——不再用 --conn-connected
    // 這個舊代理值（那是「連線成功」的語意，不是「task/agent 完成」的正式成功色 token）。用一個
    // 真的 DOM 節點讀 getComputedStyle 而不是直接比對 CSS 變數字串，確保拿到的是瀏覽器正規化
    // 過的顏色格式，能跟其他 getComputedStyle 讀回的顏色直接字串比較。
    const successColor = await cdp.eval(`(() => {
      var raw = getComputedStyle(document.documentElement).getPropertyValue('--ok').trim();
      if (!raw) return null;
      var probe = document.createElement('span');
      probe.style.color = raw;
      document.body.appendChild(probe);
      var resolved = getComputedStyle(probe).color;
      document.body.removeChild(probe);
      return resolved;
    })()`);
    const effectiveSuccessColor = successColor || SPEC_COLORS.ok;
    log(`D1：成功色比對基準 = ${effectiveSuccessColor}（${successColor ? '讀自 --ok token' : '--ok 尚未定義，退回 spec 色值 #39D5AC'}）`);

    const real = await cdp.eval(`(() => {
      var doneBadge = document.querySelector('.pane-row[data-pane="wJ:p2"] .status');
      var completedLabel = document.querySelector('.project[data-project="cockpit"] .ff-cell[data-workstream="ops"][data-stage="Review"] .task-node .task-status-label');
      var pageText = document.body.textContent;
      return {
        doneText: doneBadge ? doneBadge.textContent : null,
        // fix round 1／Codex F5：只比較「狀態文字的計算色」（前景 color），不看背景——背景不同
        // 不能拿來當「文字色沒有違規」的退路。
        doneColor: doneBadge ? getComputedStyle(doneBadge).color : null,
        completedColor: completedLabel ? getComputedStyle(completedLabel).color : null,
        hasWanCheng: pageText.indexOf('\\u5b8c\\u6210') !== -1,
      };
    })()`);
    check(real.doneText === 'done', `pane 應該顯示 done 文字（實際 ${JSON.stringify(real.doneText)}）`);
    check(real.hasWanCheng === false, `頁面任何位置不應該出現「完成」字樣（實際含有＝${real.hasWanCheng}）`);
    check(
      real.doneColor !== null && real.doneColor !== effectiveSuccessColor,
      `pane done 狀態文字的計算色不應該等於成功色（實際 ${real.doneColor}；成功色基準 ${effectiveSuccessColor}）`
    );
    // fix round 1／Codex F5：直接要求 doneColor !== completedColor，不再用
    // 「color 不同 OR background 不同」這種只要背景剛好不同就會通過的寬鬆條件。
    check(
      real.completedColor !== null && real.doneColor !== real.completedColor,
      `pane done 與 task completed 的狀態文字計算色不應該相同（實際 done=${real.doneColor}，completed=${real.completedColor}）`
    );

    // direction-01-visual task 3.3（design D4 agent 對照表）：done 是「主要文字色＋冰青空心點」——
    // 文字色要是 --text，符號是 2px 冰青實線環（不是實心、不是成功色）。成功色也不得出現在點上。
    const doneAgent = await cdp.eval(`(() => {
      ${AGENT_MEASURE_JS}
      return agentMeasure(document.querySelector('.pane-row[data-pane="wJ:p2"]'));
    })()`);
    const doneWhy = agentMismatch(doneAgent, 'done');
    check(doneWhy.length === 0, `[D1/symbol] pane done 用主要文字色＋冰青空心點（design D4；${doneWhy.join('；') || 'ok'}；實際 ${JSON.stringify(doneAgent)}）`);
    check(
      !!doneAgent && !doneAgent.missing && doneAgent.color !== effectiveSuccessColor && doneAgent.textColor !== effectiveSuccessColor,
      `[D1/symbol] pane done 的點與文字都不使用成功色（實際 點=${doneAgent && doneAgent.color}，文字=${doneAgent && doneAgent.textColor}）`
    );
    // 否定對照：把真正的 done 點改成成功色環，上面兩條都必須抓到。
    const doneNeg = await cdp.eval(`(() => {
      ${AGENT_MEASURE_JS}
      var st = document.createElement('style');
      st.textContent = '.pane-row[data-pane="wJ:p2"] .agent-dot { border-color: var(--ok) !important; }';
      document.head.appendChild(st);
      var m = agentMeasure(document.querySelector('.pane-row[data-pane="wJ:p2"]'));
      st.remove();
      return m;
    })()`);
    check(
      !!doneNeg && doneNeg.color === effectiveSuccessColor && agentMismatch(doneNeg, 'done').length > 0,
      `否定對照 [D1/symbol]：done 點改成成功色環時，量到的點色＝成功色且 agentMismatch 判定不符（實際 ${JSON.stringify(doneNeg)}）`
    );

    // 否定對照（Ruling R3）：上面幾條在現行前端很可能碰巧全過（done 用藍、completed 用紫，
    // 只是兩個互不相干的既有顏色選擇，不是刻意避開成功色）。fix round 1／規格檢查 Minor：
    // 改成套在跟真正 doneBadge「同構」的節點上（同樣的 class="status status-done"，走同一條
    // getComputedStyle 讀取路徑），不是隨手一個 `<span>`——直接證明「如果 done 真的被改成套用
    // 成功色，這條斷言會抓到」，不是只驗證顏色字串格式讀回一致這種間接證明。
    const negControl = await cdp.eval(`(() => {
      var realBadge = document.querySelector('.pane-row[data-pane="wJ:p2"] .status');
      if (!realBadge) return null;
      var probe = realBadge.cloneNode(true); // 同構：完全複製真正 doneBadge 的標籤與 class。
      probe.id = '__done_color_probe';
      probe.style.color = ${JSON.stringify(effectiveSuccessColor)}; // inline style 覆蓋掉 class 的顏色。
      probe.style.position = 'fixed';
      probe.style.left = '-9999px';
      document.body.appendChild(probe);
      var color = getComputedStyle(probe).color;
      document.body.removeChild(probe);
      return color;
    })()`);
    check(
      negControl !== null && negControl === effectiveSuccessColor,
      `否定對照：把一個跟真正 doneBadge 同構（同樣的 class="status status-done"）的節點顏色強制改成成功色，計算樣式讀到的確實是成功色（實際 ${negControl}）— 直接證明「done 真的被改成套用成功色時，doneColor !== effectiveSuccessColor 這條斷言會 FAIL」，不是只驗證字串格式`
    );

    // 頁面文字掃描的辨識力：注入含「完成」的臨時文字節點，證明掃描抓得到。
    const wanChengProbe = await cdp.eval(`(() => {
      var probe = document.createElement('span');
      probe.id = '__wancheng_probe';
      probe.textContent = '\\u5b8c\\u6210';
      document.body.appendChild(probe);
      var found = document.body.textContent.indexOf('\\u5b8c\\u6210') !== -1;
      document.body.removeChild(probe);
      return found;
    })()`);
    check(
      wanChengProbe === true,
      `否定對照：注入含「完成」的節點後，頁面文字掃描應該抓得到（實際 ${wanChengProbe}）— 證明上面「不出現完成字樣」不是偵測器失靈`
    );
  } finally {
    await stopChrome(chrome, 'chrome-D1');
    await stopPreview(preview, 'preview-D1');
  }
}

// ---------------------------------------------------------------------------
// U1：dashboard/未知狀態不破壞畫面（pane agent_status；direction-01-visual task 3.3，Ruling R1
// 把這個 scenario 列為 3.3 的 visual-check 驗收段）。whatever-check.js 守的是 DOM 層（class
// 白名單、原字串、title），這段守換皮後的「畫面」：該列以次要文字色顯示原字串、符號是跟 idle
// 實心點分得開的斷線環；其他列的符號、文字與色彩不受影響；兩行排版不變；超長未知字串也不讓右欄
// 出現橫向捲動。
// ---------------------------------------------------------------------------
async function partUnknownAgentStatus() {
  log('=== U1. dashboard/未知狀態不破壞畫面（pane agent_status）===');
  let preview = null;
  let chrome = null;
  try {
    preview = await startPreview({}, 'preview-U1');
    const url = `http://127.0.0.1:${preview.port}/`;
    chrome = await startChrome(pickPort(19160, [preview.port]), url, 'chrome-U1');
    const { cdp } = chrome;
    await waitForFirstProjection(cdp, preview.port);

    const OTHER = ['wJ:p2', 'wJ:p3'];
    const measureRows = `(() => {
      ${AGENT_MEASURE_JS}
      ${RIGHT_COLUMN_MEASURE_JS}
      var out = {};
      ['wJ:p1', 'wJ:p2', 'wJ:p3'].forEach(function (id) { out[id] = agentMeasure(document.querySelector('.pane-row[data-runtime="win"][data-pane="' + id + '"]')); });
      var sc = document.querySelector('[data-region="runtimes"] > .runtime-cards');
      out.rows = document.querySelectorAll('[data-region="runtimes"] .pane-row').length;
      var rcAll = rcMeasureRightColumn();
      out.layout = rcAll.pane;
      out.wsLayout = rcAll.workspace;
      out.hScroll = sc ? { sw: sc.scrollWidth, cw: sc.clientWidth } : null;
      out.badClass = !!document.querySelector('[class*="status-whatever"], [class*="status-future"]');
      return out;
    })()`;
    const before = await cdp.eval(measureRows);

    const fixture = loadFixture();
    fixture.version = 7001;
    fixture.runtimes[0].workspaces[0].tabs[0].panes[0].agent_status = 'whatever';
    const inj = await injectState(cdp, fixture);
    check(inj && inj.ok === true, 'U1：注入 wJ:p1 的 agent_status＝whatever 的特製投影');
    const after = await cdp.eval(measureRows);

    const target = after['wJ:p1'];
    check(
      !!target && !target.missing && target.text === 'whatever' && target.statusTitle === 'whatever',
      `U1：whatever 列顯示原字串，title 也帶原字串（實際 ${JSON.stringify(target && { text: target.text, title: target.statusTitle })}）`
    );
    check(
      !!target && /(^|\s)status-unknown(\s|$)/.test(target.statusClass) && after.badClass === false,
      `U1：whatever 列落在 status-unknown，沒有未經白名單的 status-whatever class（實際 class=${target && target.statusClass}，badClass=${after.badClass}）`
    );
    check(!!target && target.textColor === SPEC_COLORS.textDim, `U1：whatever 列的狀態文字用次要文字色 --text-dim（實際 ${target && target.textColor}）`);
    const whyTarget = agentMismatch(target, 'whatever');
    check(whyTarget.length === 0, `U1：whatever 列的符號是 --text-dim 斷線環（跟 idle 的實心點分得開；${whyTarget.join('；') || 'ok'}）`);
    for (const id of OTHER) {
      const a = before[id];
      const b = after[id];
      const same = !!a && !!b && a.text === b.text && a.textColor === b.textColor && a.shape === b.shape && a.color === b.color;
      const status = id === 'wJ:p2' ? 'done' : 'unknown';
      check(
        same && agentMismatch(b, status).length === 0,
        `U1：其他列正常——${id}（${status}）注入前後的符號、文字與色彩一致且符合 D4（前 ${JSON.stringify(a && [a.text, a.textColor, a.shape, a.color])}，後 ${JSON.stringify(b && [b.text, b.textColor, b.shape, b.color])}）`
      );
    }
    check(after.rows === 3, `U1：pane 列數不變（實際 ${after.rows}）`);
    check(after.layout && after.layout.ok === true, `U1：注入後每一列仍是兩行排版（實際 ${JSON.stringify(after.layout && after.layout.detail)}）`);
    check(
      !!after.hScroll && after.hScroll.sw <= after.hScroll.cw,
      `U1：右欄內層沒有橫向捲動（scrollWidth ${after.hScroll && after.hScroll.sw} ≤ clientWidth ${after.hScroll && after.hScroll.cw}）`
    );

    // 超長的未知字串（例如日後協定新增的長狀態名）：原字串仍在文字與 title 裡，右欄不出現橫向捲動。
    const longStatus = 'future-status-' + 'x'.repeat(120);
    const longState = loadFixture();
    longState.version = 7002;
    longState.runtimes[0].workspaces[0].tabs[0].panes[0].agent_status = longStatus;
    // 同時放一個超長的 workspace label：標題列的彙總狀態不得被它擠掉。
    longState.runtimes[0].workspaces[0].label = 'workspace-' + 'z'.repeat(120);
    const inj2 = await injectState(cdp, longState);
    check(inj2 && inj2.ok === true, 'U1：注入 134 字元未知 agent_status 的特製投影');
    const longM = await cdp.eval(measureRows);
    check(
      !!longM['wJ:p1'] && longM['wJ:p1'].text === longStatus && longM['wJ:p1'].statusTitle === longStatus,
      `U1：超長未知字串仍以原字串呈現、title 帶全文（實際長度 ${longM['wJ:p1'] && typeof longM['wJ:p1'].text === 'string' ? longM['wJ:p1'].text.length : null}）`
    );
    check(
      !!longM.hScroll && longM.hScroll.sw <= longM.hScroll.cw && longM.layout.ok === true,
      `U1：超長未知字串不讓右欄內層橫向捲動、兩行排版不變（實際 hScroll=${JSON.stringify(longM.hScroll)}，layout=${JSON.stringify(longM.layout && longM.layout.detail)}）`
    );
    check(
      !!longM.wsLayout && longM.wsLayout.ok === true,
      `U1：超長 workspace label 不擠掉標題列的彙總狀態（符號＋文字完整可見）（實際 ${JSON.stringify(longM.wsLayout && longM.wsLayout.detail)}）`
    );
    const whyLong = agentMismatch(longM['wJ:p1'], longStatus);
    check(whyLong.length === 0, `U1：超長未知字串列的符號與文字色同樣是 --text-dim 斷線環（${whyLong.join('；') || 'ok'}）`);

    // 否定對照 (a)：未知狀態文字被改成 --text 時，次要文字色斷言要抓到。
    // 否定對照 (b)：列內塞一段 2000px 不換行文字時，橫向捲動偵測要抓到。
    const neg = await cdp.eval(`(() => {
      ${AGENT_MEASURE_JS}
      var st = document.createElement('style');
      st.textContent = '.pane-row .status-unknown { color: var(--text) !important; }';
      document.head.appendChild(st);
      var colored = agentMeasure(document.querySelector('.pane-row[data-pane="wJ:p1"]'));
      st.remove();
      var row = document.querySelector('.pane-row[data-pane="wJ:p1"]');
      var sc = document.querySelector('[data-region="runtimes"] > .runtime-cards');
      var probe = document.createElement('span');
      probe.style.cssText = 'display:inline-block;width:2000px;white-space:nowrap;';
      probe.textContent = 'x';
      row.appendChild(probe);
      var wide = { sw: sc.scrollWidth, cw: sc.clientWidth };
      probe.remove();
      return { color: colored && colored.textColor, wide: wide };
    })()`);
    check(neg.color !== SPEC_COLORS.textDim, `否定對照 (a)：未知狀態文字改成 --text 時，量到的顏色不是 --text-dim（實際 ${neg.color}）`);
    check(neg.wide.sw > neg.wide.cw, `否定對照 (b)：列內塞 2000px 不換行文字時，右欄內層 scrollWidth 大於 clientWidth（實際 ${JSON.stringify(neg.wide)}）`);
  } finally {
    await stopChrome(chrome, 'chrome-U1');
    await stopPreview(preview, 'preview-U1');
  }
}

// ---------------------------------------------------------------------------
// RM1：dashboard/減少動態（task 2.2；CDP Emulation.setEmulatedMedia）
// ---------------------------------------------------------------------------

async function partReducedMotion() {
  log('=== RM1. 減少動態（prefers-reduced-motion: reduce）===');
  let preview = null;
  let chrome = null;
  try {
    preview = await startPreview({}, 'preview-RM1');
    const url = `http://127.0.0.1:${preview.port}/`;
    chrome = await startChrome(pickPort(19100, [preview.port]), url, 'chrome-RM1');
    const { cdp } = chrome;
    await waitForFirstProjection(cdp, preview.port);

    await cdp.send('Emulation.setEmulatedMedia', {
      features: [{ name: 'prefers-reduced-motion', value: 'reduce' }],
    });
    const matches = await cdp.eval("window.matchMedia('(prefers-reduced-motion: reduce)').matches");
    check(matches === true, 'CDP 模擬 prefers-reduced-motion: reduce 生效（matchMedia 為 true）');

    // 滑過與聚焦一個按鈕，再走訪全頁所有元素的 animation-name／transition-duration。
    await cdp.eval(`(() => {
      var btn = document.querySelector('.action-button');
      if (btn) { btn.focus(); }
      return true;
    })()`);
    const result = await cdp.eval(`(() => {
      var all = document.querySelectorAll('*');
      var bad = [];
      for (var i = 0; i < all.length; i += 1) {
        var el = all[i];
        var cs = getComputedStyle(el);
        if (cs.animationName !== 'none' || cs.transitionDuration !== '0s') {
          bad.push({
            tag: el.tagName,
            cls: el.className ? String(el.className) : '',
            animationName: cs.animationName,
            transitionDuration: cs.transitionDuration,
          });
          if (bad.length >= 5) break;
        }
      }
      return { total: all.length, badCount: bad.length, bad: bad };
    })()`);
    check(
      result.badCount === 0,
      `prefers-reduced-motion: reduce 時所有元素的 animation-name 應該是 none、transition-duration 應該是 0s（前 5 個違規：${JSON.stringify(result.bad)}）`
    );
  } finally {
    await stopChrome(chrome, 'chrome-RM1');
    await stopPreview(preview, 'preview-RM1');
  }
}

// ---------------------------------------------------------------------------
// FN1：dashboard/不為字體發出網路請求（task 2.2）
// ---------------------------------------------------------------------------

async function partNoFontRequests() {
  log('=== FN1. 不為字體發出網路請求 ===');
  let preview = null;
  let chrome = null;
  try {
    preview = await startPreview({}, 'preview-FN1');
    const blankUrl = 'about:blank';
    chrome = await startChrome(pickPort(19110, [preview.port]), blankUrl, 'chrome-FN1');
    const { cdp } = chrome;

    const requests = [];
    await cdp.send('Network.enable');
    cdp.onEvent('Network.requestWillBeSent', (params) => {
      requests.push(params.request.url);
    });

    const url = `http://127.0.0.1:${preview.port}/`;
    await cdp.send('Page.navigate', { url });
    await waitForFirstProjection(cdp, preview.port, '導覽到 / 並等待首份投影畫完（R22 動態條件）');
    await sleep(500); // 讓非同步的請求（若有）有機會被記錄到。

    const origin = `http://127.0.0.1:${preview.port}`;
    const external = requests.filter((u) => !u.startsWith(origin) && !u.startsWith('data:'));
    check(
      external.length === 0,
      `頁面發出的請求應該只有本服務自己的路徑，不應該有外部網域的請求（實際外部請求：${JSON.stringify(external)}；全部請求：${JSON.stringify(requests)}）`
    );

    // 否定對照（Ruling R3）：目前確實沒有任何外部字體請求，這條大概率是「真的綠燈」而不是
    // 偵測器失靈；用注入一個外部字體請求來證明 Network 監控真的抓得到跨網域請求
    // ——requestWillBeSent 在請求發起當下就觸發，不需要目的地真的可連通，headless 沙箱不需要
    // 對外網路也能完成這個自我測試。
    const before = requests.length;
    await cdp.eval(`(() => {
      var link = document.createElement('link');
      link.rel = 'stylesheet';
      link.href = 'https://visual-check-selftest.invalid/font.css';
      document.head.appendChild(link);
      return true;
    })()`);
    await sleep(300);
    const injected = requests.slice(before).some((u) => u.indexOf('visual-check-selftest.invalid') !== -1);
    check(
      injected === true,
      `否定對照：注入一個外部字體請求（visual-check-selftest.invalid）後，Network 監控應該記錄到它（實際請求 ${JSON.stringify(requests.slice(before))}）— 證明上面「沒有外部請求」不是監控失靈`
    );
  } finally {
    await stopChrome(chrome, 'chrome-FN1');
    await stopPreview(preview, 'preview-FN1');
  }
}

// ---------------------------------------------------------------------------
// TK1：dashboard/唯一色彩與字級 token 契約（task 2.2 fix round 1；控制端 Ruling R20，
// 採 Codex high／medium；fix round 2 依 Codex medium 強化；fix round 3 依控制端 Ruling
// R21「TK1 定型」再強化，不再逐條補洞——這是同類 finding 第三輪，一次補齊到「規格上的
// 有限集合」為止）。純靜態檢查，不啟動 preview／Chrome，也不加任何新套件（含 CSS
// parser——brief 明文禁止）：直接讀 cockpit/assets/app/style.css 原始碼，去掉
// /* ... */ 註解後，用宣告層級（`selector { name: value; }`）的正規表示式加允許清單斷言：
//   (a) 全部待檢內容先轉小寫（Ruling R21(a)）：色彩函式關鍵字與具名顏色比對因此天生不分
//       大小寫，不必額外掛 /i／逐一大小寫排列組合。
//   (b) 10 個核心色彩 token 在**全檔案所有 :root 區塊合併後**各恰好出現一次，值與 spec
//       色值逐字相符（Ruling R21(c)：fix round 2 只解析「第一個」:root，檔尾另外加一個新
//       的 `:root { --ok: var(--bad); }` 會被忽略——Codex r2 medium 抓到）。
//   (c) 任何非 :root 的選擇器裡如果重新宣告核心 token 名稱（不管值是什麼、合不合法），
//       一律 FAIL——核心 token 只准在 :root 定義（Ruling R21(c) 後半）。
//   (d) 除了 :root 裡那 10 筆核心宣告本身，檔案任何地方都不得出現被禁止的色彩語法：hex、
//       rgb()／rgba()、hsl()／hwb()／lab()／lch()／oklab()／oklch()／color()（色彩函式
//       黑名單比對前已轉小寫，Ruling R21(d)），或 CSS Color Module Level 4 的完整具名
//       顏色（148 個，Ruling R21(b)，見下方 CSS4_NAMED_COLORS 與來源）。這條同時堵住
//       「定義一個新的間接 custom property 存壞值、再用 var() 轉手引用」的繞過——壞值
//       宣告在它自己出現的那一行就會被抓到，不需要另外做 var() 依賴解析。
//   (e) 每一條 font-size 宣告的值只能是 var(--fs-title)／var(--fs-panel)／
//       var(--fs-dense)／var(--fs-meta) 四者之一；FONT_SIZE_KEYWORD_EXCEPTIONS 列出允許的
//       關鍵字例外（目前是空集合）。
// 十個負對照（Ruling R3 慣例；fix round 3 依 Codex r2 medium 補 HSL()／OKLCH()／
// rebeccapurple／第二個 :root 覆寫四個，另補一個「非 :root 選擇器重宣告核心 token」）：
// hex、rgb、hsl（大寫 HSL 測大小寫不分）、oklch（大寫 OKLCH）、具名顏色 red、
// rebeccapurple（CSS4 才新增、最容易漏掉的那個）、同一 :root 內重複核心宣告、**另一個**
// `:root {}` 區塊覆寫核心宣告、非 :root 選擇器重新宣告核心 token、非 token 字級，偵測器
// 都必須回報 FAIL。
// fix round 4（Codex r3 medium，使用者 2026-09-24 裁決修這一條後關閉 TK1）：宣告解析改成
// 「在 `;` 或區塊結尾結束」，並清掉同一類結構假設（引號字串裡的 `{`／`}`／`;`、括號裡的
// `;`、名稱含數字的 custom property、CSS nesting 父層宣告、字級檢查的獨立 `;` regex）。
// 負對照再加七個：Codex 的三個無尾端分號案例（第二個 :root、非 :root 重宣告核心 token、
// rebeccapurple），以及無分號 font-size、`--c1: red`、巢狀父層 `color: red`、字串內含
// `;}` 後接 `color: red`，同樣都必須回報 FAIL。
// ---------------------------------------------------------------------------

const STYLE_CSS_PATH = path.join(REPO, 'cockpit', 'assets', 'app', 'style.css');
// 核心 10 個色彩 token：名稱→design D4 指定的 hex 值（跟上面 SPEC_COLORS 的 rgb() 版本
// 是同一組色票；這裡要用 hex 才能跟原始碼裡的宣告逐字比對）。
const CORE_TOKEN_HEX = {
  '--bg-deep': '#091320',
  '--bg-base': '#101a2a',
  '--surface': '#142338',
  '--text': '#e5edf3',
  '--text-dim': '#a3b7c9',
  '--accent': '#63d5e8',
  '--line': '#294258',
  '--ok': '#39d5ac',
  '--warn': '#e9bc73',
  '--bad': '#f47279',
};
const CORE_TOKEN_NAMES = Object.keys(CORE_TOKEN_HEX);
const FONT_SIZE_KEYWORD_EXCEPTIONS = [];

// CSS Color 4 的其他色域函式，都要跟 hex／rgb()／rgba() 一樣被擋。用純字串 indexOf 比對
// （不是 regex），對照前文字已經整段轉小寫（Ruling R21(a)(d)），這裡的關鍵字本身也全小寫，
// 天生不分大小寫，不必掛 /i。`color(` 特別要確認不會誤判 `color-mix(`——`color-mix(` 在
// "color" 後面接的是 "-mix(" 不是 "("，兩者不是同一個子字串，indexOf('color(') 不會命中
// `color-mix(`（fix round 2 已用 node -e 手動驗證＋TK1 對真實 style.css 是 0 違規佐證）。
const DISALLOWED_COLOR_FUNCTIONS = ['hsl(', 'hwb(', 'lab(', 'lch(', 'oklab(', 'oklch(', 'color('];
// fix round 3（Ruling R21(b)）：CSS Color Module Level 4 §Named Colors 的完整具名顏色
// 清單，固定 148 個（147 個傳統具名色＋rebeccapurple）。來源：
// https://www.w3.org/TR/css-color-4/#named-colors（2026-09-24 以 WebFetch 查證逐字抄錄，
// 用 node 計數確認剛好 148 個）。全部已是小寫，對照前文字也已轉小寫，天生不分大小寫。
// 刻意不含 `transparent`／`currentcolor`——CSS 規格裡這兩個不在「具名顏色」表格內，是另外
// 兩個獨立關鍵字：`transparent` 沒有色相可言（不會讓畫面出現非 10 色的顏色），
// `currentcolor` 繼承的是同一個元素或祖先的 `color`，而 `color` 屬性本身仍然受這整套
// 檢查約束（如果 color 被設成非法值，會在它自己那條宣告上被抓到）——這兩個關鍵字因此
// 不需要、也不應該被視為繞過管道，明列為允許。
const CSS4_NAMED_COLORS = [
  'aliceblue', 'antiquewhite', 'aqua', 'aquamarine', 'azure', 'beige', 'bisque', 'black',
  'blanchedalmond', 'blue', 'blueviolet', 'brown', 'burlywood', 'cadetblue', 'chartreuse',
  'chocolate', 'coral', 'cornflowerblue', 'cornsilk', 'crimson', 'cyan', 'darkblue', 'darkcyan',
  'darkgoldenrod', 'darkgray', 'darkgreen', 'darkgrey', 'darkkhaki', 'darkmagenta',
  'darkolivegreen', 'darkorange', 'darkorchid', 'darkred', 'darksalmon', 'darkseagreen',
  'darkslateblue', 'darkslategray', 'darkslategrey', 'darkturquoise', 'darkviolet', 'deeppink',
  'deepskyblue', 'dimgray', 'dimgrey', 'dodgerblue', 'firebrick', 'floralwhite', 'forestgreen',
  'fuchsia', 'gainsboro', 'ghostwhite', 'gold', 'goldenrod', 'gray', 'green', 'greenyellow',
  'grey', 'honeydew', 'hotpink', 'indianred', 'indigo', 'ivory', 'khaki', 'lavender',
  'lavenderblush', 'lawngreen', 'lemonchiffon', 'lightblue', 'lightcoral', 'lightcyan',
  'lightgoldenrodyellow', 'lightgray', 'lightgreen', 'lightgrey', 'lightpink', 'lightsalmon',
  'lightseagreen', 'lightskyblue', 'lightslategray', 'lightslategrey', 'lightsteelblue',
  'lightyellow', 'lime', 'limegreen', 'linen', 'magenta', 'maroon', 'mediumaquamarine',
  'mediumblue', 'mediumorchid', 'mediumpurple', 'mediumseagreen', 'mediumslateblue',
  'mediumspringgreen', 'mediumturquoise', 'mediumvioletred', 'midnightblue', 'mintcream',
  'mistyrose', 'moccasin', 'navajowhite', 'navy', 'oldlace', 'olive', 'olivedrab', 'orange',
  'orangered', 'orchid', 'palegoldenrod', 'palegreen', 'paleturquoise', 'palevioletred',
  'papayawhip', 'peachpuff', 'peru', 'pink', 'plum', 'powderblue', 'purple', 'rebeccapurple',
  'red', 'rosybrown', 'royalblue', 'saddlebrown', 'salmon', 'sandybrown', 'seagreen',
  'seashell', 'sienna', 'silver', 'skyblue', 'slateblue', 'slategray', 'slategrey', 'snow',
  'springgreen', 'steelblue', 'tan', 'teal', 'thistle', 'tomato', 'turquoise', 'violet',
  'wheat', 'white', 'whitesmoke', 'yellow', 'yellowgreen',
];
const NAMED_COLOR_RE = new RegExp(`\\b(${CSS4_NAMED_COLORS.join('|')})\\b`);
const HEX_RE = /#[0-9a-f]{3,8}\b/;
const RGB_RE = /\brgba?\(/;

function stripCssComments(text) {
  return text.replace(/\/\*[\s\S]*?\*\//g, '');
}

// 掃一段文字（通常是某條宣告的 value）有沒有任何被禁止的色彩語法，回傳違規原因陣列
// （可能不只一個），空陣列代表乾淨。Ruling R21(a)：先整段轉小寫，下面所有比對天生不分
// 大小寫（HEX_RE／RGB_RE／DISALLOWED_COLOR_FUNCTIONS／NAMED_COLOR_RE 都已經是小寫）。
function findColorSyntaxViolations(text) {
  const lower = text.toLowerCase();
  const found = [];
  if (HEX_RE.test(lower)) found.push('hex 色碼');
  if (RGB_RE.test(lower)) found.push('rgb()／rgba()');
  for (const fn of DISALLOWED_COLOR_FUNCTIONS) {
    if (lower.indexOf(fn) !== -1) found.push(fn);
  }
  const namedMatch = lower.match(NAMED_COLOR_RE);
  if (namedMatch) found.push(`具名顏色 "${namedMatch[1]}"`);
  return found;
}

// fix round 3（Ruling R21(c)）：解析全檔案所有規則區塊，不是只找「第一個 :root」——用
// 大括號深度配對。這仍然是逐字元掃描，不是引入 CSS parser 套件（brief 明文禁止）。
// fix round 4（Codex r3 medium 同一類假設一併清掉）：
//   - 引號字串裡的 `{`／`}`／`;` 不算結構字元（例如 `content: "}"`、
//     `url("data:image/svg+xml;utf8,...")`），反斜線跳脫的字元原樣保留。
//   - 每個區塊只保留「自己這一層」的文字（body）：巢狀子規則的「選擇器＋整個區塊」會從
//     父層 body 剔除，所以 CSS nesting（`.a { color: red; .b { ... } }`）裡 `.a` 自己的宣告
//     不會再因為「它包著巢狀大括號」被整段跳過（fix round 3 只收葉規則，那段宣告會漏檢）；
//     @media 這種容器規則的 body 只剩空白，不影響結果。
//   - 子規則選擇器的起點＝父層最後一個 `;`／`{`／`}` 之後（括號內的 `;` 不算），所以巢狀
//     子規則的選擇器不會把父層前面的宣告吃進去。
// bodyEnd＝這個規則收尾 `}` 的字元位置——injectIntoRoot() 靠這個精確位置插入，不用對 body
// 字串做 indexOf() 反查，避免撞到雷同文字插錯地方。
function extractRules(text) {
  const rules = [];
  const stack = [{ selector: null, body: '', segStart: 0 }];
  let quote = null;
  let parenDepth = 0;
  for (let i = 0; i < text.length; i += 1) {
    const ch = text[i];
    const top = stack[stack.length - 1];
    if (quote) {
      top.body += ch;
      if (ch === '\\' && i + 1 < text.length) {
        i += 1;
        top.body += text[i];
      } else if (ch === quote) {
        quote = null;
      }
      continue;
    }
    if (ch === '"' || ch === "'") {
      quote = ch;
      top.body += ch;
    } else if (ch === '{') {
      const selector = top.body.slice(top.segStart).trim();
      top.body = top.body.slice(0, top.segStart);
      stack.push({ selector, body: '', segStart: 0 });
      parenDepth = 0;
    } else if (ch === '}') {
      if (stack.length > 1) {
        const done = stack.pop();
        rules.push({ selector: done.selector, body: done.body, bodyEnd: i });
      }
      const parent = stack[stack.length - 1];
      parent.segStart = parent.body.length;
      parenDepth = 0;
    } else {
      top.body += ch;
      if (ch === '(') parenDepth += 1;
      else if (ch === ')' && parenDepth > 0) parenDepth -= 1;
      else if (ch === ';' && parenDepth === 0) top.segStart = top.body.length;
    }
  }
  return rules;
}

// `:root` 可能跟其他選擇器逗號並列（本檔目前沒有這樣寫，保守處理）：只要其中一個是 :root
// 就算 root 規則的一部分，核心 token 若寫在這種區塊也要納入全域計數。
function isRootSelector(selector) {
  return selector
    .split(',')
    .map((s) => s.trim())
    .includes(':root');
}

// 宣告層級解析（不是完整 CSS parser——brief 明文禁止加套件）：回傳依出現順序排列的
// { name, value } 陣列。fix round 4（Codex r3 medium）：fix round 3 用
// `/([a-zA-Z-]+)\s*:\s*([^;]+);/g`，只收「以 `;` 結尾」的宣告，但 CSS 允許區塊最後一筆
// 宣告省略分號（`:root { --ok: var(--bad) }`），那筆會整個消失、繞過所有檢查；同一條 regex
// 的名稱字元類也不含數字與底線，`--c1: red;` 這種 custom property 同樣整筆漏掉。改成：
// 宣告在 `;` **或區塊結尾**結束——以括號深度 0、引號字串外的 `;` 切段，每段取第一個 `:`
// 前為名稱、後為值（值原樣保留 `!important` 等尾綴，交給後面的檢查自己判斷）；沒有 `:`
// 的片段瀏覽器本來就會丟棄，這裡也略過。一般屬性名稱不分大小寫，轉小寫；custom property
// （`--` 開頭）名稱依規格區分大小寫，保留原樣。
function extractDeclarations(blockText) {
  const chunks = [];
  let current = '';
  let quote = null;
  let parenDepth = 0;
  for (let i = 0; i < blockText.length; i += 1) {
    const ch = blockText[i];
    if (quote) {
      current += ch;
      if (ch === '\\' && i + 1 < blockText.length) {
        i += 1;
        current += blockText[i];
      } else if (ch === quote) {
        quote = null;
      }
      continue;
    }
    if (ch === '"' || ch === "'") quote = ch;
    else if (ch === '(') parenDepth += 1;
    else if (ch === ')' && parenDepth > 0) parenDepth -= 1;
    if (ch === ';' && parenDepth === 0) {
      chunks.push(current);
      current = '';
    } else {
      current += ch;
    }
  }
  chunks.push(current);
  const decls = [];
  for (const chunk of chunks) {
    const colon = chunk.indexOf(':');
    if (colon === -1) continue;
    const rawName = chunk.slice(0, colon).trim();
    if (rawName === '') continue;
    const name = rawName.startsWith('--') ? rawName : rawName.toLowerCase();
    decls.push({ name, value: chunk.slice(colon + 1).trim() });
  }
  return decls;
}

// S4 負對照專用：把一行額外宣告插進「既有第一個 :root 區塊內部」（緊接在它的收尾 `}`
// 之前）——用來測「同一個 :root 裡重複宣告」這個情境；「另一個獨立 :root {} 區塊覆寫」
// 的負對照不需要這個函式，直接把新 :root 規則接在檔尾字串即可，因為 checkColorContract()
// 已經改成解析全檔所有 :root 區塊（fix round 3／Ruling R21(c)），不會像 fix round 2 之前
// 那樣被忽略。回傳值已經是去掉註解的文字，直接餵給 checkColorContract()／
// checkFontSizeTokens() 即可。
function injectIntoRoot(cssText, extraDeclText) {
  const stripped = stripCssComments(cssText);
  const rules = extractRules(stripped);
  const first = rules.find((r) => isRootSelector(r.selector));
  if (!first) throw new Error('injectIntoRoot：找不到 :root 區塊');
  return `${stripped.slice(0, first.bodyEnd)}\n  ${extraDeclText}\n${stripped.slice(first.bodyEnd)}`;
}

function checkColorContract(cssText) {
  const stripped = stripCssComments(cssText);
  const rules = extractRules(stripped);
  const violations = [];

  const rootRules = rules.filter((r) => isRootSelector(r.selector));
  const otherRules = rules.filter((r) => !isRootSelector(r.selector));
  if (rootRules.length === 0) return { ok: false, violations: ['找不到任何 :root 區塊'] };

  // (b) 把「全檔案所有 :root 區塊」的宣告攤平成同一份清單再計數（Ruling R21(c)：不是只看
  // 第一個 :root——這正是 fix round 2「第二個 :root 覆寫」繞過的根因）。
  const rootDecls = [];
  for (const r of rootRules) rootDecls.push(...extractDeclarations(r.body));

  for (const name of CORE_TOKEN_NAMES) {
    const matches = rootDecls.filter((d) => d.name === name);
    if (matches.length !== 1) {
      violations.push(
        `:root（全檔案，含所有 :root 區塊）裡的 ${name} 應該恰好宣告一次（實際 ${matches.length} 次：${JSON.stringify(matches.map((m) => m.value))}）`
      );
      continue;
    }
    const actual = matches[0].value.trim().toLowerCase();
    const expected = CORE_TOKEN_HEX[name].toLowerCase();
    if (actual !== expected) {
      violations.push(`:root 裡的 ${name} 應該是 ${expected}（實際 ${actual}）`);
    }
  }

  // :root 內非核心 token 的宣告、以及核心 token 名稱的第 2 筆以後重複宣告，都要掃色彩語法
  // （第 1 筆已經在上面驗證過值，不必重複掃）。
  const seenCoreCount = {};
  for (const d of rootDecls) {
    const isCore = CORE_TOKEN_NAMES.includes(d.name);
    if (isCore) {
      seenCoreCount[d.name] = (seenCoreCount[d.name] || 0) + 1;
      if (seenCoreCount[d.name] === 1) continue;
      const reasons = findColorSyntaxViolations(d.value);
      for (const r of reasons) {
        violations.push(`:root 的 ${d.name} 第 ${seenCoreCount[d.name]} 筆重複宣告出現${r}（值：${d.value}）`);
      }
      continue;
    }
    const reasons = findColorSyntaxViolations(d.value);
    for (const r of reasons) violations.push(`:root 的 ${d.name}（非核心 token）宣告出現${r}（值：${d.value}）`);
  }

  // (c)(d)：:root 以外的每個選擇器——重新宣告核心 token 名稱一律 FAIL（不管值合不合法），
  // 其餘宣告掃色彩語法。
  for (const r of otherRules) {
    const decls = extractDeclarations(r.body);
    for (const d of decls) {
      if (CORE_TOKEN_NAMES.includes(d.name)) {
        violations.push(
          `選擇器 "${r.selector}" 不得重新宣告核心 token ${d.name}（值：${d.value}）——核心 token 只准在 :root 定義`
        );
      }
      const reasons = findColorSyntaxViolations(d.value);
      for (const rr of reasons) violations.push(`選擇器 "${r.selector}" 的 ${d.name} 宣告出現${rr}（值：${d.value}）`);
    }
  }

  return { ok: violations.length === 0, violations };
}

// fix round 4：改走跟色彩檢查同一套 extractRules()＋extractDeclarations()——fix round 3
// 以前是對整份文字跑 `/font-size\s*:\s*([^;]+);/g`，跟 Codex r3 medium 同一個「宣告一定以
// `;` 結尾」的假設：檔尾 `.x { font-size: 15px }` 會漏掉。
function checkFontSizeTokens(cssText) {
  const stripped = stripCssComments(cssText);
  const violations = [];
  for (const r of extractRules(stripped)) {
    for (const d of extractDeclarations(r.body)) {
      if (d.name !== 'font-size') continue;
      const isToken = /^var\(--fs-(title|panel|dense|meta)\)$/.test(d.value);
      const isException = FONT_SIZE_KEYWORD_EXCEPTIONS.includes(d.value);
      if (!isToken && !isException) violations.push(d.value);
    }
  }
  return { ok: violations.length === 0, violations };
}

async function partTokenContract() {
  log('=== TK1. 唯一色彩與字級 token 契約（style.css 靜態檢查，不需要瀏覽器）===');
  const cssText = fs.readFileSync(STYLE_CSS_PATH, 'utf8');

  const colorResult = checkColorContract(cssText);
  check(
    colorResult.ok,
    `style.css 的 10 個核心色彩 token 應該各自恰好宣告一次、值與 spec 相符，且其餘宣告不得出現任何被禁止的色彩語法（實際違規：${JSON.stringify(colorResult.violations)}）`
  );

  const fontResult = checkFontSizeTokens(cssText);
  check(
    fontResult.ok,
    `style.css 的 font-size 只能是 var(--fs-title|panel|dense|meta)（實際違規值：${JSON.stringify(fontResult.violations)}）`
  );

  // 否定對照（Ruling R3；fix round 2 補齊 hsl／具名顏色／重複核心宣告；fix round 3 依
  // Codex r2 medium 與控制端 Ruling R21 再補 OKLCH（大寫）／rebeccapurple／另一個獨立
  // :root 覆寫／非 :root 選擇器重宣告核心 token 四個，HSL 改用大寫測大小寫不分）：每個都
  // 注入到（複製的）CSS 文字尾端，偵測器必須回報對應違規。
  const injectedHex = `${cssText}\n.visual-check-selftest { color: #123456; }\n`;
  const injectedHexResult = checkColorContract(injectedHex);
  check(
    injectedHexResult.ok === false && injectedHexResult.violations.some((v) => v.indexOf('hex') !== -1),
    `否定對照：注入 color: #123456 後偵測器應該回報 hex 違規（實際 ${JSON.stringify(injectedHexResult)}）`
  );

  const injectedRgb = `${cssText}\n.visual-check-selftest { color: rgb(1, 2, 3); }\n`;
  const injectedRgbResult = checkColorContract(injectedRgb);
  check(
    injectedRgbResult.ok === false && injectedRgbResult.violations.some((v) => v.indexOf('rgb') !== -1),
    `否定對照：注入 color: rgb(1,2,3) 後偵測器應該回報 rgb 違規（實際 ${JSON.stringify(injectedRgbResult)}）`
  );

  // fix round 3／Codex r2 medium：大寫 HSL(...)——舊版 indexOf 區分大小寫，這個會漏網。
  const injectedHsl = `${cssText}\n.visual-check-selftest { color: HSL(0, 100%, 50%); }\n`;
  const injectedHslResult = checkColorContract(injectedHsl);
  check(
    injectedHslResult.ok === false && injectedHslResult.violations.some((v) => v.indexOf('hsl(') !== -1),
    `否定對照：注入 color: HSL(0,100%,50%)（大寫）後偵測器應該回報 hsl() 違規（實際 ${JSON.stringify(injectedHslResult)}）`
  );

  // fix round 3／Codex r2 medium：大寫 OKLCH(...)，CSS Color 4 的色域函式，舊版完全沒查。
  const injectedOklch = `${cssText}\n.visual-check-selftest { color: OKLCH(0.7 0.15 200); }\n`;
  const injectedOklchResult = checkColorContract(injectedOklch);
  check(
    injectedOklchResult.ok === false && injectedOklchResult.violations.some((v) => v.indexOf('oklch(') !== -1),
    `否定對照：注入 color: OKLCH(0.7 0.15 200)（大寫）後偵測器應該回報 oklch() 違規（實際 ${JSON.stringify(injectedOklchResult)}）`
  );

  const injectedNamed = `${cssText}\n.visual-check-selftest { color: red; }\n`;
  const injectedNamedResult = checkColorContract(injectedNamed);
  check(
    injectedNamedResult.ok === false && injectedNamedResult.violations.some((v) => v.indexOf('具名顏色') !== -1),
    `否定對照：注入 color: red 後偵測器應該回報具名顏色違規（實際 ${JSON.stringify(injectedNamedResult)}）`
  );

  // fix round 3／Codex r2 medium：rebeccapurple——CSS Color 4 才新增的具名色，舊版「常見色」
  // 清單沒有收，最容易漏掉的那個，現在改用完整 148 色清單必須抓到。
  const injectedRebecca = `${cssText}\n.visual-check-selftest { color: rebeccapurple; }\n`;
  const injectedRebeccaResult = checkColorContract(injectedRebecca);
  check(
    injectedRebeccaResult.ok === false &&
      injectedRebeccaResult.violations.some((v) => v.indexOf('rebeccapurple') !== -1),
    `否定對照：注入 color: rebeccapurple 後偵測器應該回報具名顏色違規（實際 ${JSON.stringify(injectedRebeccaResult)}）`
  );

  // 同一個 :root 區塊內重複宣告。
  const injectedDup = injectIntoRoot(cssText, '--ok: red;');
  const injectedDupResult = checkColorContract(injectedDup);
  check(
    injectedDupResult.ok === false &&
      injectedDupResult.violations.some((v) => v.indexOf('--ok') !== -1 && v.indexOf('恰好宣告一次') !== -1),
    `否定對照：注入第二筆 --ok: red（同一個 :root 內）後偵測器應該回報「應該恰好宣告一次」違規（實際 ${JSON.stringify(injectedDupResult)}）`
  );

  // fix round 3／Codex r2 medium（Ruling R21(c) 核心動機）：另一個**獨立**的 :root {} 區塊
  // 覆寫核心 token，值本身合法（var(--bad) 不含任何被禁字面值）——fix round 2 的
  // findRootBlock() 只認第一個 :root，這個案例會被完全忽略；fix round 3 改成解析全檔所有
  // :root 區塊、全域計數，必須抓到「--ok 出現兩次」。
  const injectedSecondRoot = `${cssText}\n:root { --ok: var(--bad); }\n`;
  const injectedSecondRootResult = checkColorContract(injectedSecondRoot);
  check(
    injectedSecondRootResult.ok === false &&
      injectedSecondRootResult.violations.some((v) => v.indexOf('--ok') !== -1 && v.indexOf('恰好宣告一次') !== -1),
    `否定對照：另加一個獨立 :root { --ok: var(--bad); } 後偵測器應該回報「應該恰好宣告一次」違規（實際 ${JSON.stringify(injectedSecondRootResult)}）`
  );

  // 額外負對照（非 Codex／控制端明文要求，但直接對應 Ruling R21(c) 後半「任何選擇器裡
  // 重新宣告核心 token 名稱一律 FAIL」這條新規則本身，補這個測試避免它沒被驗證過）：在
  // 非 :root 的選擇器裡重新宣告一個核心 token 名稱，值合法也要 FAIL。
  const injectedRedeclare = `${cssText}\n.visual-check-selftest { --bg-deep: #091320; }\n`;
  const injectedRedeclareResult = checkColorContract(injectedRedeclare);
  check(
    injectedRedeclareResult.ok === false &&
      injectedRedeclareResult.violations.some((v) => v.indexOf('--bg-deep') !== -1 && v.indexOf('不得重新宣告核心 token') !== -1),
    `否定對照：非 :root 選擇器重新宣告 --bg-deep（值合法）後偵測器應該回報「不得重新宣告核心 token」違規（實際 ${JSON.stringify(injectedRedeclareResult)}）`
  );

  const injectedFontSize = `${cssText}\n.visual-check-selftest { font-size: 15px; }\n`;
  const injectedFontResult = checkFontSizeTokens(injectedFontSize);
  check(
    injectedFontResult.ok === false && injectedFontResult.violations.includes('15px'),
    `否定對照：注入 font-size: 15px 後偵測器應該回報違規（實際 ${JSON.stringify(injectedFontResult)}）`
  );

  // fix round 4／Codex r3 medium：區塊最後一筆宣告省略分號（合法 CSS）。fix round 3 的
  // extractDeclarations() 只收以 `;` 結尾的宣告，下面三個 Codex 原案例都回傳 ok:true。
  const noSemiSecondRoot = checkColorContract(`${cssText}\n:root { --ok: var(--bad) }\n`);
  check(
    noSemiSecondRoot.ok === false &&
      noSemiSecondRoot.violations.some((v) => v.indexOf('--ok') !== -1 && v.indexOf('恰好宣告一次') !== -1),
    `否定對照：另加獨立 :root { --ok: var(--bad) }（無尾端分號）後偵測器應該回報「應該恰好宣告一次」違規（實際 ${JSON.stringify(noSemiSecondRoot)}）`
  );

  const noSemiRedeclare = checkColorContract(`${cssText}\n.visual-check-selftest { --ok: var(--bad) }\n`);
  check(
    noSemiRedeclare.ok === false &&
      noSemiRedeclare.violations.some((v) => v.indexOf('--ok') !== -1 && v.indexOf('不得重新宣告核心 token') !== -1),
    `否定對照：非 :root 選擇器 { --ok: var(--bad) }（無尾端分號）後偵測器應該回報「不得重新宣告核心 token」違規（實際 ${JSON.stringify(noSemiRedeclare)}）`
  );

  const noSemiNamed = checkColorContract(`${cssText}\n.visual-check-selftest { color: rebeccapurple }\n`);
  check(
    noSemiNamed.ok === false && noSemiNamed.violations.some((v) => v.indexOf('rebeccapurple') !== -1),
    `否定對照：{ color: rebeccapurple }（無尾端分號）後偵測器應該回報具名顏色違規（實際 ${JSON.stringify(noSemiNamed)}）`
  );

  // fix round 4：同一個「宣告一定以 `;` 結尾／結構單純」假設的其他出口，一併補負對照。
  // (1) 字級檢查原本是另一條獨立 regex，同樣要求 `;`——檔尾無分號的 font-size 會漏掉。
  const noSemiFont = checkFontSizeTokens(`${cssText}\n.visual-check-selftest { font-size: 15px }`);
  check(
    noSemiFont.ok === false && noSemiFont.violations.includes('15px'),
    `否定對照：檔尾 { font-size: 15px }（無尾端分號）後偵測器應該回報違規（實際 ${JSON.stringify(noSemiFont)}）`
  );
  // (2) 名稱含數字的 custom property：舊 regex 名稱字元類是 [a-zA-Z-]，整筆漏掉（真實
  //     style.css 的 `--shell-banner-1` 在 fix round 3 就是這樣沒被掃到的）。
  const digitName = checkColorContract(`${cssText}\n.visual-check-selftest { --c1: red; }\n`);
  check(
    digitName.ok === false && digitName.violations.some((v) => v.indexOf('--c1') !== -1 && v.indexOf('具名顏色') !== -1),
    `否定對照：{ --c1: red; }（名稱含數字）後偵測器應該回報具名顏色違規（實際 ${JSON.stringify(digitName)}）`
  );
  // (3) CSS nesting：父規則自己的宣告不能因為它包著子規則就被跳過。
  const nested = checkColorContract(`${cssText}\n.visual-check-selftest { color: red; .x { color: var(--text); } }\n`);
  check(
    nested.ok === false &&
      nested.violations.some((v) => v.indexOf('".visual-check-selftest"') !== -1 && v.indexOf('具名顏色') !== -1),
    `否定對照：巢狀規則的父層 { color: red; .x {...} } 後偵測器應該回報具名顏色違規（實際 ${JSON.stringify(nested)}）`
  );
  // (4) 引號字串裡的 `;` 與 `}` 不是結構字元，不得把後面的宣告切斷或吞掉。
  const inString = checkColorContract(`${cssText}\n.visual-check-selftest { content: "a;}b"; color: red }\n`);
  check(
    inString.ok === false && inString.violations.some((v) => v.indexOf('color') !== -1 && v.indexOf('具名顏色') !== -1),
    `否定對照：content: "a;}b" 之後的 color: red 應該被回報具名顏色違規（實際 ${JSON.stringify(inString)}）`
  );
}

// ---------------------------------------------------------------------------
// CT1：dashboard/文字對比（task 3.4／4.2／5.2；子斷言標 banner／stale／all，Ruling R4）
// ---------------------------------------------------------------------------

async function partTextContrast() {
  log('=== CT1. 文字對比（banner／stale／all）===');

  // --- banner 子斷言（task 3.4）：錯誤與改綁提示的文字對比 ---
  {
    let preview = null;
    let chrome = null;
    try {
      preview = await startPreview(
        { COCKPIT_PREVIEW_WRITE_RULES: '/api/projects/cockpit/tasks/be-2/fail=100:409' },
        'preview-CT1-banner'
      );
      const url = `http://127.0.0.1:${preview.port}/`;
      chrome = await startChrome(pickPort(19120, [preview.port]), url, 'chrome-CT1-banner');
      const { cdp } = chrome;
      await waitForFirstProjection(cdp, preview.port);
      await installTools(cdp);

      // 順序：先進改綁模式，再觸發錯誤（同 P1「切換 Project 不清除錯誤也不離開改綁模式」段
      // 的註解——actions.js 的 perform() 對任何一般「畫面操作」都會先清掉 ui.error，反過來做
      // 會讓「改綁」這個動作自己把剛顯示的錯誤清掉，兩個 banner 就湊不到一起同時存在）。
      await cdp.click('[data-action="rebind"][data-project="cockpit"][data-workstream="ops"]');
      await cdp.waitFor("!!document.querySelector('.rebind-banner')", 2000, '進入改綁模式');
      await cdp.click('[data-action="fail"][data-project="cockpit"][data-task="be-2"]');
      await cdp.waitFor("!!document.querySelector('.error-banner')", 3000, '改綁模式期間出現錯誤提示');

      const ratios = await cdp.eval(`(() => {
        var out = [];
        var err = document.querySelector('.error-banner .action-banner-text');
        var rebind = document.querySelector('.rebind-banner .action-banner-text');
        if (err) out.push({ label: 'error-banner', ratio: window.__cockpitVisualTools.textContrast(err).ratio });
        if (rebind) out.push({ label: 'rebind-banner', ratio: window.__cockpitVisualTools.textContrast(rebind).ratio });
        return out;
      })()`);
      check(ratios.length === 2, `banner: 找到錯誤與改綁提示的文字節點（實際 ${ratios.length}）`);
      for (const r of ratios) {
        check(r.ratio >= 4.5, `banner: ${r.label} 的文字對比應該 ≥ 4.5:1（實際 ${r.ratio.toFixed(2)}）`);
      }

      // --- 左緣條家族（task 3.4；前面 task 帶入事項 2／3）：錯誤與改綁提示同一套形狀，只有
      // 顏色不同；左緣條保持直線（不被圓角彎成括號）；提示裡的按鈕仍是 D4 第三層級中性樣式、
      // 不用任何狀態色。每一項都附一個注入違規樣式的否定對照，量完立刻還原。
      //
      // fix round 1（Codex medium／規格對照 Minor／控制端 Ruling R36）：原本底色與其餘三邊框線
      // 只驗「兩者彼此相等」，兩者一起改成同一個錯誤值（例如底色一起變回 --bg-base、左框寬度
      // 一起變成 1px）也會通過，不是真的釘住「精確等於 --surface／2px --bad 或 --accent／1px
      // --line」。改成兩則各自獨立比對精確 token 值；按鈕改用完整 D4 動作按鈕 probe（同其他段落
      // 的 [G1/button] 做法：透明底、四邊 1px --text-dim、文字 --text-dim），不只挑 color 與
      // borderTopColor 兩項。新增三個否定對照，分別對應 Codex 點名的三種「兩者一起改仍會誤判
      // 通過」風險：(a) 只改 error-banner 的底色、(b) 兩者一起改左框寬度、(c) 按鈕加狀態色背景。
      const family = await cdp.eval(`(() => {
        // 純幾何形狀（不含顏色／邊框寬度，那些下面用精確 token 值個別釘住）：兩則提示逐項應該
        // 相同。
        function shape(el) {
          var cs = getComputedStyle(el);
          return {
            borderTopLeftRadius: cs.borderTopLeftRadius,
            borderTopRightRadius: cs.borderTopRightRadius,
            borderBottomRightRadius: cs.borderBottomRightRadius,
            borderBottomLeftRadius: cs.borderBottomLeftRadius,
            padding: cs.padding,
            display: cs.display,
            alignItems: cs.alignItems,
            justifyContent: cs.justifyContent,
            gap: cs.gap,
          };
        }
        var err = document.querySelector('.error-banner');
        var rebind = document.querySelector('.rebind-banner');
        var errShape = shape(err);
        var rebindShape = shape(rebind);
        var shapeKeys = Object.keys(errShape);
        var shapeDiff = shapeKeys.filter(function (k) { return errShape[k] !== rebindShape[k]; });
        // 否定對照：暫時把 rebind-banner 的 padding 改掉，證明「同一套形狀」偵測器抓得到差異。
        rebind.style.padding = '20px';
        var negShapeDiff = shapeKeys.filter(function (k) { return errShape[k] !== shape(rebind)[k]; });
        rebind.style.padding = '';

        var straight = {
          error: errShape.borderTopLeftRadius === '0px' && errShape.borderBottomLeftRadius === '0px',
          rebind: rebindShape.borderTopLeftRadius === '0px' && rebindShape.borderBottomLeftRadius === '0px',
        };
        // 否定對照：暫時給 error-banner 圓角，證明「左緣條保持直線」偵測器抓得到。
        err.style.borderRadius = '6px';
        var negStraight = getComputedStyle(err).borderTopLeftRadius !== '0px';
        err.style.borderRadius = '';

        // 精確值：底色都是 --surface；左框都是 2px solid（錯誤 --bad、改綁 --accent）；其餘三邊
        // 都是 1px solid --line。兩則各自獨立比對，不是「跟對方一樣」——這樣兩者同時被改成同一個
        // 錯誤值時，每一則都會各自被抓到。
        var surface = ${JSON.stringify(SPEC_COLORS.surface)};
        var line = ${JSON.stringify(SPEC_COLORS.line)};
        var bad = ${JSON.stringify(SPEC_COLORS.bad)};
        var accent = ${JSON.stringify(SPEC_COLORS.accent)};
        function exactShape(el, sideColor) {
          var cs = getComputedStyle(el);
          var problems = [];
          if (cs.color !== sideColor) problems.push('color ' + cs.color);
          if (cs.backgroundColor !== surface) problems.push('background ' + cs.backgroundColor);
          if (!(cs.borderLeftWidth === '2px' && cs.borderLeftStyle === 'solid' && cs.borderLeftColor === sideColor)) {
            problems.push('borderLeft ' + cs.borderLeftWidth + ' ' + cs.borderLeftStyle + ' ' + cs.borderLeftColor);
          }
          ['Top', 'Right', 'Bottom'].forEach(function (s) {
            if (!(cs['border' + s + 'Width'] === '1px' && cs['border' + s + 'Style'] === 'solid' && cs['border' + s + 'Color'] === line)) {
              problems.push('border' + s + ' ' + cs['border' + s + 'Width'] + ' ' + cs['border' + s + 'Style'] + ' ' + cs['border' + s + 'Color']);
            }
          });
          return problems;
        }
        var exact = { error: exactShape(err, bad), rebind: exactShape(rebind, accent) };

        // 否定對照 (a)：只把 error-banner 的底色改錯（不是 --surface），必須被抓到。
        err.style.backgroundColor = 'var(--bg-base)';
        var negBackground = exactShape(err, bad).some(function (p) { return p.indexOf('background') === 0; });
        err.style.backgroundColor = '';

        // 否定對照 (b)：兩者「一起」把左框寬度改成同一個錯誤值（1px）——只驗「兩者相等」抓不到
        // 這種回歸，精確值比對必須兩邊都轉紅。
        err.style.borderLeftWidth = '1px';
        rebind.style.borderLeftWidth = '1px';
        var negLeftWidthBoth = {
          error: exactShape(err, bad).some(function (p) { return p.indexOf('borderLeft') === 0; }),
          rebind: exactShape(rebind, accent).some(function (p) { return p.indexOf('borderLeft') === 0; }),
        };
        err.style.borderLeftWidth = '';
        rebind.style.borderLeftWidth = '';

        // 按鈕：完整 D4 動作按鈕 probe（同其他段落 [G1/button] 的做法）——透明底、四邊 1px
        // --text-dim、文字 --text-dim，不是只挑 color 與 borderTopColor 兩項。
        var dim = ${JSON.stringify(SPEC_COLORS.textDim)};
        function probeButton(btn) {
          var bcs = getComputedStyle(btn);
          var problems = [];
          if (bcs.color !== dim) problems.push('color ' + bcs.color);
          ['Top', 'Right', 'Bottom', 'Left'].forEach(function (s) {
            if (bcs['border' + s + 'Color'] !== dim) problems.push('border' + s + 'Color ' + bcs['border' + s + 'Color']);
            if (bcs['border' + s + 'Width'] !== '1px') problems.push('border' + s + 'Width ' + bcs['border' + s + 'Width']);
          });
          if (bcs.backgroundColor !== 'rgba(0, 0, 0, 0)') problems.push('background ' + bcs.backgroundColor);
          return problems;
        }
        var errBtn = err.querySelector('.action-button');
        var rebindBtn = rebind.querySelector('.action-button');
        var buttons = { error: probeButton(errBtn), rebind: probeButton(rebindBtn) };
        // 否定對照 (c)：暫時給 error-banner 按鈕加狀態色背景，必須被抓到。
        errBtn.style.backgroundColor = 'var(--bad)';
        var negButtonBackground = probeButton(errBtn).some(function (p) { return p.indexOf('background') === 0; });
        errBtn.style.backgroundColor = '';

        return {
          shapeDiff: shapeDiff,
          negShapeDiff: negShapeDiff,
          straight: straight,
          negStraight: negStraight,
          exact: exact,
          negBackground: negBackground,
          negLeftWidthBoth: negLeftWidthBoth,
          buttons: buttons,
          negButtonBackground: negButtonBackground,
        };
      })()`);
      check(
        family.straight.error === true && family.straight.rebind === true,
        `banner: 兩則提示的左緣不得被圓角彎成括號形（實際 ${JSON.stringify(family.straight)}）`
      );
      check(
        family.negStraight === true,
        `banner: 否定對照——暫時給 error-banner 圓角後，左緣直線偵測器必須判定不再是 0px（實際 ${family.negStraight}）`
      );
      check(
        family.shapeDiff.length === 0,
        `banner: 錯誤與改綁提示除顏色與框寬外形狀應該一致（差異欄位 ${JSON.stringify(family.shapeDiff)}）`
      );
      check(
        family.negShapeDiff.length > 0,
        `banner: 否定對照——暫時改掉 rebind-banner 的 padding 後，形狀比對必須抓到差異（實際 ${JSON.stringify(family.negShapeDiff)}）`
      );
      check(
        family.exact.error.length === 0,
        `banner: 錯誤提示應該精確是「--surface 底＋2px solid --bad 左框＋其餘三邊 1px solid --line＋--bad 文字」（違規：${JSON.stringify(family.exact.error)}）`
      );
      check(
        family.exact.rebind.length === 0,
        `banner: 改綁提示應該精確是「--surface 底＋2px solid --accent 左框＋其餘三邊 1px solid --line＋--accent 文字」（違規：${JSON.stringify(family.exact.rebind)}）`
      );
      check(
        family.negBackground === true,
        `banner: 否定對照——暫時把 error-banner 底色改成 --bg-base 後，精確底色比對必須抓到（實際 ${family.negBackground}）`
      );
      check(
        family.negLeftWidthBoth.error === true && family.negLeftWidthBoth.rebind === true,
        `banner: 否定對照——兩則提示的左框寬度一起改成 1px 後，精確值比對必須兩邊都抓到（實際 ${JSON.stringify(family.negLeftWidthBoth)}）`
      );
      check(
        family.buttons.error.length === 0 && family.buttons.rebind.length === 0,
        `banner: 兩則提示裡的按鈕都應該是完整的 D4 中性樣式（透明底、四邊 1px --text-dim、文字 --text-dim；違規：${JSON.stringify(family.buttons)}）`
      );
      check(
        family.negButtonBackground === true,
        `banner: 否定對照——暫時給 error-banner 按鈕加狀態色背景後，按鈕 probe 必須抓到（實際 ${family.negButtonBackground}）`
      );
    } finally {
      await stopChrome(chrome, 'chrome-CT1-banner');
      await stopPreview(preview, 'preview-CT1-banner');
    }
  }

  // --- banner-wrap 子斷言（task 3.4 fix round 1；設計 I1／M2；控制端 Ruling R36；fix round 2／
  // Codex medium：docs/research/2026-09-23/visual-check.js:7244-7287 舊版丟棄 `!!t`、只量測
  // error-dismiss、沒有證明文字真的折成多行且完整可見，negative control 同時改 white-space 與
  // flex-shrink 兩個屬性，抓不到「只改一個屬性」的回歸）：1200×720（固定一屏最緊繃）與 700×900
  // （單欄）兩種寬度下：
  //   1. 兩則提示的文字節點都必須存在（`!!t` 不再被丟棄）。
  //   2. 用 Range.getClientRects() 證明文字真的折成 ≥2 行（不是只看按鈕沒被擠壓就推論文字有
  //      折行——舊版完全沒量文字本身）。
  //   3. 文字沒有被裁切（scrollHeight ≤ clientHeight）也沒有水平溢出（scrollWidth ≤
  //      clientWidth）。
  //   4. 錯誤「關閉」（error-dismiss）與改綁「取消」（rebind-cancel）兩顆按鈕都要維持單行高度、
  //      寬度大於高度、按鈕內部不溢出——同一條 CSS 規則（`.action-banner > .action-button`／
  //      `.action-banner-text`）涵蓋兩個 banner，只驗 error-dismiss 會漏掉 rebind-cancel 的
  //      回歸（Codex 原文點名）。
  //   5. 三個獨立 negative control，各自只改一個屬性：
  //      (a) 文字節點被迫 `white-space: nowrap`（模擬「被裁切／不換行」的回歸）→ 折行行數必須
  //          偵測到掉回 1 行、且出現水平溢出。
  //      (b) 只還原按鈕的 `white-space`（保留 `flex-shrink: 0`）。
  //      (c) 只還原按鈕的 `flex-shrink`（保留 `white-space: nowrap`）。
  //      經實測（見 fix round 2 報告），(b)／(c) 單獨還原都不會讓按鈕被壓成直排——`flex-shrink:
  //      0` 已經讓按鈕在 flex 版面裡拿住自然內容寬度，`.action-banner-text` 的
  //      `min-width: 0`／`overflow-wrap: anywhere` 讓文字吸收掉幾乎全部必要的收縮量，兩個按鈕
  //      屬性都不是唯一防線；真正的 negative control 需要同時撤銷「按鈕的兩個屬性」或撤銷
  //      「文字的 `min-width: 0`」才會重現擠壓。誠實記錄這個結果，不假造「(b)／(c) 必定 FAIL」。
  //      仍然各自獨立量測、印出實際數字，供之後回歸參考。 ---
  {
    let preview = null;
    let chrome = null;
    try {
      preview = await startPreview(
        { COCKPIT_PREVIEW_WRITE_RULES: '/api/projects/cockpit/tasks/be-2/fail=100:409' },
        'preview-CT1-banner-wrap'
      );
      const url = `http://127.0.0.1:${preview.port}/`;
      chrome = await startChrome(pickPort(19125, [preview.port]), url, 'chrome-CT1-banner-wrap', '1536,1024');
      const { cdp } = chrome;
      await waitForFirstProjection(cdp, preview.port);

      async function setViewport(width, height) {
        await cdp.send('Emulation.setDeviceMetricsOverride', { width, height, deviceScaleFactor: 1, mobile: false });
        await sleep(150);
        const actual = await cdp.eval('({ w: window.innerWidth, h: window.innerHeight })');
        check(
          actual.w === width && actual.h === height,
          `banner-wrap：viewport 應該精準設成 ${width}x${height}（實際 ${JSON.stringify(actual)}）`
        );
      }

      for (const [width, height] of [
        [1200, 720],
        [700, 900],
      ]) {
        await setViewport(width, height);

        await cdp.click('[data-action="rebind"][data-project="cockpit"][data-workstream="ops"]');
        await cdp.waitFor("!!document.querySelector('.rebind-banner')", 2000, `banner-wrap ${width}x${height}：進入改綁模式`);
        await cdp.click('[data-action="fail"][data-project="cockpit"][data-task="be-2"]');
        await cdp.waitFor("!!document.querySelector('.error-banner')", 3000, `banner-wrap ${width}x${height}：出現錯誤提示`);

        // 錯誤提示的 ✕ 符號（設計 M2）：aria-hidden，改綁提示不加。
        const symbols = await cdp.eval(`(() => {
          var errSym = document.querySelector('.error-banner .action-banner-symbol');
          var rebindSym = document.querySelector('.rebind-banner .action-banner-symbol');
          return {
            errText: errSym ? errSym.textContent : null,
            errAriaHidden: errSym ? errSym.getAttribute('aria-hidden') : null,
            rebindHasSymbol: !!rebindSym,
          };
        })()`);
        check(
          symbols.errText === '✕' && symbols.errAriaHidden === 'true',
          `banner-wrap ${width}x${height}：錯誤提示應該有 aria-hidden 的 ✕ 符號（實際 ${JSON.stringify(symbols)}）`
        );
        check(
          symbols.rebindHasSymbol === false,
          `banner-wrap ${width}x${height}：改綁提示不應該加符號（實際 ${symbols.rebindHasSymbol}）`
        );

        // 折行前的單行高度當基準（預設 409 文字在多數尺寸下一行放得下，當場量、不寫死數字）；
        // 兩顆按鈕都量，覆蓋 Codex medium 點名「只測 error-dismiss、漏了同一條規則保護的
        // rebind-cancel」。
        const before = await cdp.eval(`(() => {
          function rect(sel) {
            var el = document.querySelector(sel);
            var r = el.getBoundingClientRect();
            return { height: r.height, width: r.width };
          }
          return {
            errBtn: rect('.error-banner [data-action="error-dismiss"]'),
            rebindBtn: rect('.rebind-banner [data-action="rebind-cancel"]'),
          };
        })()`);

        // 覆寫成長文字逼折行（同 V1 矩陣段的 LONG_ERROR_TEXT，模組層級共用同一份）：兩則提示的
        // 文字都覆寫——同一條 CSS 規則涵蓋兩個 banner，只逼錯誤提示折行測不到改綁提示的回歸。
        // 不再丟棄 `!!t`（Codex medium 原文點名的第一個問題），改成斷言兩個文字節點都存在。
        const overwrite = await cdp.eval(`(() => {
          var et = document.querySelector('.error-banner .action-banner-text');
          var rt = document.querySelector('.rebind-banner .action-banner-text');
          if (et) et.textContent = ${JSON.stringify(LONG_ERROR_TEXT)};
          if (rt) rt.textContent = ${JSON.stringify(LONG_ERROR_TEXT)};
          return { hasErrText: !!et, hasRebindText: !!rt };
        })()`);
        check(
          overwrite.hasErrText === true,
          `banner-wrap ${width}x${height}：錯誤提示的文字節點應該存在（用來覆寫成長文字逼折行）`
        );
        check(
          overwrite.hasRebindText === true,
          `banner-wrap ${width}x${height}：改綁提示的文字節點應該存在（用來覆寫成長文字逼折行）`
        );
        await sleep(50);

        // 用 Range.getClientRects() 直接證明文字折成 ≥2 行——比「按鈕沒被擠壓」更直接的證據
        // （Codex medium 原文點名：舊版完全沒量文字本身，只測按鈕尺寸）。同時驗證文字沒有被裁切
        // （scrollHeight ≤ clientHeight）也沒有水平溢出（scrollWidth ≤ clientWidth）。
        const textMeasureFn = `
          function measureText(sel) {
            var el = document.querySelector(sel);
            if (!el) return null;
            var range = document.createRange();
            range.selectNodeContents(el);
            var rects = Array.from(range.getClientRects());
            var lineTops = Array.from(new Set(rects.map((r) => Math.round(r.top))));
            return {
              lineCount: lineTops.length,
              scrollWidth: el.scrollWidth,
              clientWidth: el.clientWidth,
              scrollHeight: el.scrollHeight,
              clientHeight: el.clientHeight,
            };
          }
        `;
        const textMetrics = await cdp.eval(`(() => {
          ${textMeasureFn}
          return {
            err: measureText('.error-banner .action-banner-text'),
            rebind: measureText('.rebind-banner .action-banner-text'),
          };
        })()`);

        for (const [label, m] of [
          ['錯誤提示', textMetrics.err],
          ['改綁提示', textMetrics.rebind],
        ]) {
          check(
            m.lineCount >= 2,
            `banner-wrap ${width}x${height}：${label}的長文字應該真的折成至少兩行（實際 ${JSON.stringify(m)}）`
          );
          check(
            m.scrollHeight <= m.clientHeight + 0.5,
            `banner-wrap ${width}x${height}：${label}折行後的文字不應該被裁切（實際 ${JSON.stringify(m)}）`
          );
          check(
            m.scrollWidth <= m.clientWidth + 0.5,
            `banner-wrap ${width}x${height}：${label}折行後的文字不應該水平溢出（實際 ${JSON.stringify(m)}）`
          );
        }

        // 否定對照 (a)：只把錯誤提示的文字節點強制 white-space: nowrap（模擬「被裁切／不換行」
        // 的回歸），折行行數必須掉回 1 行、且出現水平溢出，證明上面三條斷言有辨識力。
        const negNowrapText = await cdp.eval(`(() => {
          ${textMeasureFn}
          var el = document.querySelector('.error-banner .action-banner-text');
          el.style.whiteSpace = 'nowrap';
          var m = measureText('.error-banner .action-banner-text');
          el.style.whiteSpace = '';
          return m;
        })()`);
        check(
          negNowrapText.lineCount === 1 && negNowrapText.scrollWidth > negNowrapText.clientWidth + 0.5,
          `banner-wrap ${width}x${height}：否定對照——強制錯誤提示文字 white-space: nowrap 後，折行行數必須掉回 1 行且出現水平溢出（實際 ${JSON.stringify(negNowrapText)}）`
        );

        // 折行後量兩顆按鈕，並在 error-dismiss 上做否定對照（rebind-cancel 用同一條 CSS 規則，
        // 機制相同，不重複做否定對照）。
        const after = await cdp.eval(`(() => {
          function measure(sel) {
            var eb = document.querySelector(sel);
            var er = eb.getBoundingClientRect();
            return { height: er.height, width: er.width, scrollWidth: eb.scrollWidth, clientWidth: eb.clientWidth };
          }
          var out = {
            errBtn: measure('.error-banner [data-action="error-dismiss"]'),
            rebindBtn: measure('.rebind-banner [data-action="rebind-cancel"]'),
          };

          var eb = document.querySelector('.error-banner [data-action="error-dismiss"]');

          // 否定對照 (b)：只還原按鈕的 white-space（保留 flex-shrink: 0）。
          eb.style.whiteSpace = 'normal';
          out.negWhiteSpaceOnly = measure('.error-banner [data-action="error-dismiss"]');
          eb.style.whiteSpace = '';

          // 否定對照 (c)：只還原按鈕的 flex-shrink（保留 white-space: nowrap）。
          eb.style.flexShrink = '1';
          out.negFlexShrinkOnly = measure('.error-banner [data-action="error-dismiss"]');
          eb.style.flexShrink = '';

          // 否定對照（fix round 1 既有寫法）：兩個屬性一起還原，證明擠壓現象確實可重現。
          eb.style.whiteSpace = 'normal';
          eb.style.flexShrink = '1';
          out.negBothReverted = measure('.error-banner [data-action="error-dismiss"]');
          eb.style.whiteSpace = '';
          eb.style.flexShrink = '';

          out.restored = measure('.error-banner [data-action="error-dismiss"]');
          return out;
        })()`);

        for (const [label, btnBefore, btnAfter] of [
          ['關閉 error-dismiss', before.errBtn, after.errBtn],
          ['取消 rebind-cancel', before.rebindBtn, after.rebindBtn],
        ]) {
          check(
            Math.abs(btnAfter.height - btnBefore.height) < 1,
            `banner-wrap ${width}x${height}：折行後「${label}」按鈕高度應該等於折行前的單行高度（折行前 ${btnBefore.height}、折行後 ${btnAfter.height}）`
          );
          check(
            btnAfter.width > btnAfter.height,
            `banner-wrap ${width}x${height}：折行後「${label}」按鈕寬度應該大於高度（實際 ${JSON.stringify(btnAfter)}）`
          );
          check(
            btnAfter.scrollWidth <= btnAfter.clientWidth + 0.5,
            `banner-wrap ${width}x${height}：折行後「${label}」按鈕不應該內部溢出（實際 ${JSON.stringify(btnAfter)}）`
          );
        }

        // 誠實記錄 (b)／(c) 單獨還原的實測結果（fix round 2，回應「若 (b)／(c) 各自不會讓按鈕
        // 壓扁，如實回報量測結果，不要假造」）：兩者實測都不會讓按鈕被壓成直排。原因是
        // `flex-shrink: 0` 讓按鈕在 flex 分配收縮量時完全不參與（跟 white-space 無關），而
        // `.action-banner-text` 自己的 `min-width: 0`／`overflow-wrap: anywhere` 讓文字願意
        // 吸收掉幾乎全部必要的收縮量，即使按鈕的 flex-shrink 恢復成 1，它分配到的收縮量也趨近
        // 於 0（下面 negFlexShrinkOnly 實測跟正常值完全相同）。也就是說：按鈕的兩個屬性各自都
        // 不是「唯一防線」，真正扛住擠壓的是「文字端的 min-width: 0」加上「按鈕端的
        // flex-shrink: 0」共同生效；只撤銷按鈕其中一個屬性都不足以重現 fix 之前的回歸，必須
        // 兩個一起撤銷（negBothReverted）才會重現。這裡仍然對 (b)／(c) 各自斷言「維持正常」，
        // 當之後的重構打破這個力學關係時，這兩條會先變紅。
        check(
          Math.abs(after.negWhiteSpaceOnly.height - after.errBtn.height) < 1,
          `banner-wrap ${width}x${height}：否定對照 (b)——只還原按鈕的 white-space（保留 flex-shrink: 0）不會讓按鈕被壓成直排，flex-shrink: 0 本身已經足夠（實際 還原後=${after.negWhiteSpaceOnly.height}、正常=${after.errBtn.height}）`
        );
        check(
          Math.abs(after.negFlexShrinkOnly.height - after.errBtn.height) < 1,
          `banner-wrap ${width}x${height}：否定對照 (c)——只還原按鈕的 flex-shrink（保留 white-space: nowrap）不會讓按鈕被壓成直排，.action-banner-text 的 min-width: 0 已經吸收掉幾乎全部收縮量（實際 還原後=${after.negFlexShrinkOnly.height}、正常=${after.errBtn.height}）`
        );
        check(
          after.negBothReverted.height > after.errBtn.height + 1,
          `banner-wrap ${width}x${height}：否定對照——同時還原按鈕的 white-space 與 flex-shrink 後必須被壓成直排、高度變高（唯一能重現 fix 之前回歸的組合；實際 還原後=${after.negBothReverted.height}、正常=${after.errBtn.height}）`
        );
        check(
          Math.abs(after.restored.height - after.errBtn.height) < 1,
          `banner-wrap ${width}x${height}：還原按鈕樣式後高度應該回到正常值（實際 ${after.restored.height}）`
        );

        // N1／N2（設計複審 r1 的兩個 Minor，fix round 2 順手修）：文字折成多行時，✕ 應該對齊
        // 第一行而不是整段文字置中；符號跟文字之間的間距要收到跟 Factory Floor 節點
        // （`.task-status-symbol`／`.task-state` 的 2px gap）一致的量級，不能沿用
        // `.action-banner` 給按鈕留的 12px gap。
        const n1n2 = await cdp.eval(`(() => {
          function firstLineCenterY(sel) {
            var el = document.querySelector(sel);
            var range = document.createRange();
            range.selectNodeContents(el);
            var rects = Array.from(range.getClientRects());
            var top = rects[0];
            return top ? (top.top + top.bottom) / 2 : null;
          }
          var errSym = document.querySelector('.error-banner .action-banner-symbol');
          var symRect = errSym.getBoundingClientRect();
          var symCenterY = (symRect.top + symRect.bottom) / 2;
          var errFirstLineY = firstLineCenterY('.error-banner .action-banner-text');
          var errTextRect = document.querySelector('.error-banner .action-banner-text').getBoundingClientRect();
          var rebindTextRect = document.querySelector('.rebind-banner .action-banner-text').getBoundingClientRect();
          var out = {
            diffY: Math.abs(symCenterY - errFirstLineY),
            symToTextGap: errTextRect.left - symRect.right,
            xDiff: errTextRect.left - rebindTextRect.left,
            symWidth: symRect.width,
          };
          // 否定對照：暫時把符號蓋回 align-items 繼承的置中（align-self: center），對齊差距
          // 必須大於原本的門檻，證明上面的 diffY 斷言有辨識力。
          errSym.style.alignSelf = 'center';
          var symRectNeg = errSym.getBoundingClientRect();
          var symCenterYNeg = (symRectNeg.top + symRectNeg.bottom) / 2;
          out.negAlignDiffY = Math.abs(symCenterYNeg - errFirstLineY);
          errSym.style.alignSelf = '';
          // 否定對照：暫時拿掉符號的負 margin，間距必須變回原本的 12px gap，證明 symToTextGap
          // 斷言有辨識力。
          errSym.style.marginRight = '0px';
          var errTextRectNeg = document.querySelector('.error-banner .action-banner-text').getBoundingClientRect();
          out.negGap = errTextRectNeg.left - errSym.getBoundingClientRect().right;
          errSym.style.marginRight = '';
          return out;
        })()`);
        check(
          n1n2.diffY <= 1,
          `banner-wrap ${width}x${height}：折行後 ✕ 符號的垂直中心應該對齊文字第一行的垂直中心（實際差距 ${n1n2.diffY}px）`
        );
        check(
          n1n2.negAlignDiffY > n1n2.diffY + 1,
          `banner-wrap ${width}x${height}：否定對照——暫時把 ✕ 符號的 align-self 改回 center（繼承 .action-banner 的置中）後，對齊差距必須變大，證明上面的斷言有辨識力（實際 還原後=${n1n2.negAlignDiffY}、正常=${n1n2.diffY}）`
        );
        check(
          n1n2.symToTextGap >= 2 && n1n2.symToTextGap <= 6,
          `banner-wrap ${width}x${height}：✕ 符號跟文字之間的間距應該收到跟 Factory Floor 節點一致的量級（2–6px；實際 ${n1n2.symToTextGap}px）`
        );
        check(
          n1n2.negGap > n1n2.symToTextGap + 4,
          `banner-wrap ${width}x${height}：否定對照——暫時拿掉符號的負 margin 後，間距必須變回原本的 12px gap，證明上面的斷言有辨識力（實際 還原後=${n1n2.negGap}、正常=${n1n2.symToTextGap}）`
        );

        // 重置回 0 則提示，準備下一個尺寸（先判斷存在再點，避免點到已消失的元素）。
        const resetState = await cdp.eval(
          "({ hasError: !!document.querySelector('.error-banner'), hasRebind: !!document.querySelector('.rebind-banner') })"
        );
        if (resetState.hasError) await cdp.click('[data-action="error-dismiss"]');
        if (resetState.hasRebind) await cdp.click('[data-action="rebind-cancel"]');
        await cdp.waitFor(
          "document.querySelectorAll('[data-region=\"banner\"] .action-banner').length === 0",
          2000,
          `banner-wrap ${width}x${height}：重置回 0 則提示`
        );
      }
    } finally {
      await stopChrome(chrome, 'chrome-CT1-banner-wrap');
      await stopPreview(preview, 'preview-CT1-banner-wrap');
    }
  }

  // --- stale 子斷言（task 4.2）：Live Output 過期標示的文字對比 ---
  {
    let preview = null;
    let chrome = null;
    try {
      preview = await startPreview(
        { COCKPIT_PREVIEW_OUTPUT_MODES: 'wJ:p1=fail:3' },
        'preview-CT1-stale'
      );
      const url = `http://127.0.0.1:${preview.port}/`;
      chrome = await startChrome(pickPort(19130, [preview.port]), url, 'chrome-CT1-stale');
      const { cdp } = chrome;
      await waitForFirstProjection(cdp, preview.port);
      await installTools(cdp);

      await cdp.eval("window.liveOutput.select('win', 'wJ:p1'); true");
      await cdp.waitFor("document.getElementById('output').classList.contains('is-stale')", 5000, '面板進入過期狀態（fail:3）');

      const ratios = await cdp.eval(`(() => {
        var out = [];
        var text = document.querySelector('.output-text');
        var reason = document.querySelector('.output-error-reason');
        if (text) out.push({ label: 'output-text（過期）', ratio: window.__cockpitVisualTools.textContrast(text).ratio });
        if (reason && !reason.hidden) out.push({ label: 'output-error-reason', ratio: window.__cockpitVisualTools.textContrast(reason).ratio });
        return out;
      })()`);
      check(ratios.length >= 1, `stale: 找到過期狀態下的文字節點（實際 ${ratios.length}）`);
      for (const r of ratios) {
        check(r.ratio >= 4.5, `stale: ${r.label} 的文字對比應該 ≥ 4.5:1（現行 opacity 做法會降低對比，design D7 要求改掉；實際 ${r.ratio.toFixed(2)}）`);
      }
    } finally {
      await stopChrome(chrome, 'chrome-CT1-stale');
      await stopPreview(preview, 'preview-CT1-stale');
    }
  }

  // --- all 子斷言（task 5.2）：走訪整頁（含各種 task status、agent 狀態、連線狀態）---
  {
    let preview = null;
    let chrome = null;
    try {
      preview = await startPreview({}, 'preview-CT1-all');
      const url = `http://127.0.0.1:${preview.port}/`;
      chrome = await startChrome(pickPort(19140, [preview.port]), url, 'chrome-CT1-all');
      const { cdp } = chrome;
      await waitForFirstProjection(cdp, preview.port);
      await installTools(cdp);

      const rows = await cdp.eval('window.__cockpitVisualTools.walkTextContrast(null)');
      check(Array.isArray(rows) && rows.length > 0, `all: 走訪到含文字的元素（實際 ${Array.isArray(rows) ? rows.length : 'n/a'} 個）`);
      const belowThreshold = rows.filter((r) => r.ratio < 4.5);
      check(
        belowThreshold.length === 0,
        `all: 每一組文字對比都應該 ≥ 4.5:1（實際低於門檻 ${belowThreshold.length} / ${rows.length} 個；前 5 個：${JSON.stringify(belowThreshold.slice(0, 5).map((r) => ({ tag: r.tag, cls: r.cls, text: r.text, ratio: r.ratio.toFixed(2) })))}）`
      );
    } finally {
      await stopChrome(chrome, 'chrome-CT1-all');
      await stopPreview(preview, 'preview-CT1-all');
    }
  }

  // --- diff 子斷言（git-review task 4.3；spec git-review「diff 分頁」；design D7 的底色推導）：
  // change 列（左 `.diff-row-del`、右 `.diff-row-add`）與 add／blank 列（暫存副本裡在
  // docs/design.md 插入一行；左側 `.diff-row-blank`）裡的文字對比都要 ≥4.5:1。`walkTextContrast`
  // 讀 `effectiveBackground()`（合成 `color-mix()` 之後瀏覽器算出的實際 rgb），不需要另外手算
  // color-mix 的合成結果。 ---
  {
    let preview = null;
    let chrome = null;
    try {
      preview = await startPreview({}, 'preview-CT1-diff');
      const reviewRepo = await ftWaitReviewRepo(preview);
      const url = `http://127.0.0.1:${preview.port}/`;
      chrome = await startChrome(pickPort(19150, [preview.port]), url, 'chrome-CT1-diff');
      const { cdp } = chrome;
      await waitForFirstProjection(cdp, preview.port);
      await installTools(cdp);

      const original = fs.readFileSync(path.join(reviewRepo, 'docs', 'design.md'), 'utf8');
      const lines = original.split('\n');
      lines.splice(Math.floor(lines.length / 2), 0, 'CT1：diff 對比檢查插入的一行（供驗 add／blank 列）');
      fs.writeFileSync(path.join(reviewRepo, 'docs', 'design.md'), lines.join('\n'));

      await cdp.click('.pane-row[data-runtime="win"][data-pane="wJ:p4"]');
      await cdp.waitFor('!!document.querySelector(\'.pane-row[data-pane="wJ:p4"].selected\')', 5000, 'diff 對比：選定 win/wJ:p4');
      await cdp.click('#files-tab-changes');
      await cdp.waitFor("document.getElementById('files-tab-changes').getAttribute('aria-selected') === 'true'", 3000, 'diff 對比：切到左欄「變更」分頁');
      await cdp.waitFor('!!document.querySelector(\'#changes-panel .changes-row[title="history/unstaged-change.txt"]\')', 5000, 'diff 對比：「變更」面板列出 history/unstaged-change.txt');
      await cdp.click('#changes-panel .changes-row[title="history/unstaged-change.txt"]');
      await cdp.waitFor(
        '(() => { var t = document.querySelector(\'#review [role="tab"][data-diff-path="history/unstaged-change.txt"]\'); return !!t && t.getAttribute(\'aria-selected\') === \'true\'; })()',
        5000,
        'diff 對比：開啟 history/unstaged-change.txt 的 diff 分頁（change 列）'
      );
      const panelId1 = await cdp.eval('document.querySelector(\'#review [role="tab"][data-diff-path="history/unstaged-change.txt"]\').getAttribute(\'aria-controls\')');
      await cdp.waitFor(`!!document.getElementById(${JSON.stringify(panelId1)}).querySelector('.diff-row-del')`, 5000, 'diff 對比：change 列的內容載入完成');

      const rows1 = await cdp.eval(`window.__cockpitVisualTools.walkTextContrast(${JSON.stringify('#' + panelId1)})`);
      check(Array.isArray(rows1) && rows1.length > 0, `diff（change 列）：走訪到含文字的元素（實際 ${Array.isArray(rows1) ? rows1.length : 'n/a'} 個）`);
      const bad1 = rows1.filter((r) => r.ratio < 4.5);
      check(bad1.length === 0, `diff（change 列）：每一組文字對比都應該 ≥ 4.5:1（實際低於門檻 ${bad1.length} / ${rows1.length} 個；${JSON.stringify(bad1.slice(0, 5))}）`);

      await cdp.click('#files-tab-changes');
      await cdp.waitFor("document.getElementById('files-tab-changes').getAttribute('aria-selected') === 'true'", 3000, 'diff 對比：切回左欄「變更」分頁（準備開 docs/design.md）');
      await cdp.waitFor('!!document.querySelector(\'#changes-panel .changes-row[title="docs/design.md"]\')', 5000, 'diff 對比：「變更」面板列出 docs/design.md');
      await cdp.click('#changes-panel .changes-row[title="docs/design.md"]');
      await cdp.waitFor(
        '(() => { var t = document.querySelector(\'#review [role="tab"][data-diff-path="docs/design.md"]\'); return !!t && t.getAttribute(\'aria-selected\') === \'true\'; })()',
        5000,
        'diff 對比：開啟 docs/design.md 的 diff 分頁（add／blank 列）'
      );
      const panelId2 = await cdp.eval('document.querySelector(\'#review [role="tab"][data-diff-path="docs/design.md"]\').getAttribute(\'aria-controls\')');
      await cdp.waitFor(`!!document.getElementById(${JSON.stringify(panelId2)}).querySelector('.diff-row-add')`, 5000, 'diff 對比：add 列的內容載入完成');

      const rows2 = await cdp.eval(`window.__cockpitVisualTools.walkTextContrast(${JSON.stringify('#' + panelId2)})`);
      check(Array.isArray(rows2) && rows2.length > 0, `diff（add／blank 列）：走訪到含文字的元素（實際 ${Array.isArray(rows2) ? rows2.length : 'n/a'} 個）`);
      const bad2 = rows2.filter((r) => r.ratio < 4.5);
      check(bad2.length === 0, `diff（add／blank 列）：每一組文字對比都應該 ≥ 4.5:1（實際低於門檻 ${bad2.length} / ${rows2.length} 個；${JSON.stringify(bad2.slice(0, 5))}）`);
    } finally {
      await stopChrome(chrome, 'chrome-CT1-diff');
      await stopPreview(preview, 'preview-CT1-diff');
    }
  }

  // --- graph 子斷言（git-review task 4.4；spec git-review「Git Graph 分頁」；design D8 的 ref
  // 標籤與 HEAD 節點樣式）：預設（不篩選）開啟 review-repo 的 Git Graph 分頁，第一批 200 列裡就
  // 同時含三種 ref 標籤（本地分支／遠端分支／tag）與 HEAD 指向的分支強調樣式（見 task 4.4 報告
  // 「開啟並分批載入」段的實測：review-repo fixture 的 origin/main、v0.1.0 tag 都落在第一批範圍
  // 內），走一次就能覆蓋全部畫面狀態，不需要另外操作。SVG 的線段／節點沒有文字節點，
  // `walkTextContrast` 自然只會量到 ref 標籤與 commit 列文字。 ---
  {
    let preview = null;
    let chrome = null;
    try {
      preview = await startPreview({}, 'preview-CT1-graph');
      const url = `http://127.0.0.1:${preview.port}/`;
      chrome = await startChrome(pickPort(19155, [preview.port]), url, 'chrome-CT1-graph');
      const { cdp } = chrome;
      await waitForFirstProjection(cdp, preview.port);
      await installTools(cdp);

      await cdp.click('.pane-row[data-runtime="win"][data-pane="wJ:p4"]');
      await cdp.waitFor('!!document.querySelector(\'.pane-row[data-pane="wJ:p4"].selected\')', 5000, 'graph 對比：選定 win/wJ:p4');
      await cdp.click('#files-tab-changes');
      await cdp.waitFor("document.getElementById('files-tab-changes').getAttribute('aria-selected') === 'true'", 3000, 'graph 對比：切到左欄「變更」分頁');
      await cdp.waitFor('!!document.querySelector(\'#changes-panel [data-action="open-git-graph"]\')', 5000, 'graph 對比：「Git Graph」按鈕出現');
      await cdp.click('#changes-panel [data-action="open-git-graph"]');
      await cdp.waitFor(
        '(() => { var t = document.querySelector(\'#review [role="tab"][data-graph-root]\'); return !!t && t.getAttribute(\'aria-selected\') === \'true\'; })()',
        5000,
        'graph 對比：Git Graph 分頁出現並成為目前分頁'
      );
      const panelId = await cdp.eval('document.getElementById(document.querySelector(\'#review [role="tab"][data-graph-root]\').getAttribute(\'aria-controls\')).id');
      await cdp.waitFor(
        `(() => { var p = document.getElementById(${JSON.stringify(panelId)}); var kinds = new Set(Array.from(p.querySelectorAll('.graph-ref-badge')).map(function (b) { return b.getAttribute('data-kind'); })); return p.querySelectorAll('.graph-row').length >= 200 && kinds.has('branch') && kinds.has('remote') && kinds.has('tag') && p.querySelectorAll('.graph-ref-badge.is-head-branch').length > 0; })()`,
        5000,
        'graph 對比：第一批載入完成，三種 ref 標籤與 HEAD 指向的分支標籤都已畫出'
      );

      const rows = await cdp.eval(`window.__cockpitVisualTools.walkTextContrast(${JSON.stringify('#' + panelId)})`);
      check(Array.isArray(rows) && rows.length > 0, `graph: 走訪到含文字的元素（實際 ${Array.isArray(rows) ? rows.length : 'n/a'} 個）`);
      const bad = rows.filter((r) => r.ratio < 4.5);
      check(bad.length === 0, `graph: 每一組文字對比都應該 ≥ 4.5:1（實際低於門檻 ${bad.length} / ${rows.length} 個；${JSON.stringify(bad.slice(0, 5))}）`);
    } finally {
      await stopChrome(chrome, 'chrome-CT1-graph');
      await stopPreview(preview, 'preview-CT1-graph');
    }
  }
}

// ---------------------------------------------------------------------------
// LO1：live-output/沒有選取時顯示空狀態＋點pane列＋取消選取＋面板打開時仍可操作頁面下方的內容
// ＋鍵盤選定（task 4.1）
// ---------------------------------------------------------------------------

// direction-01-visual task 4.1：LO1 共用的面板狀態量測。design D7 逐字空狀態文案；「畫出來」＝
// getClientRects() 非空且 visibility 不是 hidden。contentBox 是面板扣掉框線與內距後的內容區，
// 用來驗 2.1 設計 M6「輸出窗撐滿 data-region="output" 區塊；空狀態時也一樣佔滿該區」。
const LO1_EMPTY_TEXT = '還沒選 pane。點 runtime 清單裡的任一列，或按 Factory Floor 列首的「看輸出」。';
const LO1_PANEL_STATE_FN = `
  function panelState() {
    function shown(n) { return !!n && n.getClientRects().length > 0 && getComputedStyle(n).visibility !== 'hidden'; }
    function rect(n) { if (!shown(n)) return null; var r = n.getBoundingClientRect(); return { top: r.top, bottom: r.bottom, left: r.left, right: r.right, height: r.height }; }
    var out = document.getElementById('output');
    var empty = out ? out.querySelector('.output-empty') : null;
    var contentBox = null;
    if (shown(out)) {
      var r = out.getBoundingClientRect();
      var cs = getComputedStyle(out);
      contentBox = {
        top: r.top + parseFloat(cs.borderTopWidth) + parseFloat(cs.paddingTop),
        bottom: r.bottom - parseFloat(cs.borderBottomWidth) - parseFloat(cs.paddingBottom),
      };
    }
    return {
      panelShown: shown(out),
      isOpen: !!out && out.classList.contains('is-open'),
      panel: rect(out),
      contentBox: contentBox,
      emptyShown: shown(empty),
      emptyText: empty ? empty.textContent : null,
      emptyRect: rect(empty),
      closeShown: shown(out && out.querySelector('.output-close')),
      titleShown: shown(out && out.querySelector('.output-title')),
      titleText: out && out.querySelector('.output-title') ? out.querySelector('.output-title').textContent : null,
      textShown: shown(out && out.querySelector('.output-text')),
      textRect: rect(out && out.querySelector('.output-text')),
      // task 4.1 fix round 1：觀察窗底色／框線（I1）、標題等寬（I2）、行高（M1）、首行對齊（M2）。
      panelBg: out ? getComputedStyle(out).backgroundColor : null,
      emptyBg: empty ? getComputedStyle(empty).backgroundColor : null,
      // fix round 2（Codex r1 medium）：對齊改比 content-box 起點（邊框＋內距之內的左上角，
      // 含 relative 位移），不再比 Range glyph rect（受字族、字型度量與 DPR 影響）。
      emptyContent: contentOrigin(empty),
      headerContent: contentOrigin(out && out.querySelector('.output-header')),
      textContent: contentOrigin(out && out.querySelector('.output-text')),
      textStyle: styleOf(out && out.querySelector('.output-text')),
      titleStyle: styleOf(out && out.querySelector('.output-title')),
      monoFamily: monoFamily(),
    };
    function contentOrigin(n) {
      if (!shown(n)) return null;
      var r = n.getBoundingClientRect();
      var cs = getComputedStyle(n);
      return {
        top: r.top + parseFloat(cs.borderTopWidth) + parseFloat(cs.paddingTop),
        left: r.left + parseFloat(cs.borderLeftWidth) + parseFloat(cs.paddingLeft),
      };
    }
    function styleOf(n) {
      if (!n) return null;
      var cs = getComputedStyle(n);
      return {
        backgroundColor: cs.backgroundColor,
        borderWidths: [cs.borderTopWidth, cs.borderRightWidth, cs.borderBottomWidth, cs.borderLeftWidth],
        borderRadius: cs.borderTopLeftRadius + ' ' + cs.borderTopRightRadius + ' ' + cs.borderBottomRightRadius + ' ' + cs.borderBottomLeftRadius,
        fontFamily: cs.fontFamily,
        fontSize: cs.fontSize,
        fontWeight: cs.fontWeight,
        lineHeight: cs.lineHeight,
      };
    }
    function monoFamily() {
      var p = document.createElement('span');
      p.style.fontFamily = 'var(--font-mono)';
      document.body.appendChild(p);
      var f = getComputedStyle(p).fontFamily;
      p.remove();
      return f;
    }
  }`;

// task 4.1 fix round 2（Codex r1 medium／設計 N1）：空狀態、標題列、輸出內容的 content-box 起點
// 對齊判準。left：三者兩兩差 <0.5px——以極差（最大 left − 最小 left）判定，fix round 3（Codex r2
// medium）：原本只比空狀態↔標題列、空狀態↔輸出內容，標題列 +0.49／輸出內容 −0.49 會通過，但兩者
// 彼此差 0.98px；top：空狀態與標題列差 <0.5px。
function lo1Align(emptyO, headerO, textO) {
  if (!emptyO || !headerO || !textO) return { ok: false, reason: 'missing', emptyO, headerO, textO };
  const lefts = [emptyO.left, headerO.left, textO.left];
  const leftSpan = Math.max(...lefts) - Math.min(...lefts);
  const dLeftHeader = Math.abs(emptyO.left - headerO.left);
  const dLeftText = Math.abs(emptyO.left - textO.left);
  const dTop = Math.abs(emptyO.top - headerO.top);
  return {
    ok: leftSpan < 0.5 && dTop < 0.5,
    leftSpan,
    dLeftHeader,
    dLeftText,
    dTop,
  };
}

// task 4.1 fix round 1：目前鍵盤焦點落在哪裡（pane 列以 data-runtime／data-pane 辨認，重畫後是新節點）。
const LO1_ACTIVE_ELEMENT_JS = `(() => {
  var a = document.activeElement;
  return {
    tag: a ? a.tagName : null,
    isBody: a === document.body,
    isOutput: a === document.getElementById('output'),
    isPaneRow: !!a && !!a.classList && a.classList.contains('pane-row'),
    runtime: a && a.getAttribute ? a.getAttribute('data-runtime') : null,
    pane: a && a.getAttribute ? a.getAttribute('data-pane') : null,
    connected: !!a && a.isConnected,
  };
})()`;

async function partLiveOutputPanel() {
  log('=== LO1. Live Output 常駐面板：空狀態／點列／取消選取／下方內容可操作／鍵盤選定 ===');
  let preview = null;
  let chrome = null;
  try {
    preview = await startPreview({}, 'preview-LO1');
    const url = `http://127.0.0.1:${preview.port}/`;
    chrome = await startChrome(pickPort(19150, [preview.port]), url, 'chrome-LO1', '1536,1024');
    const { cdp } = chrome;
    await waitForFirstProjection(cdp, preview.port);
    await installTools(cdp);

    // --- 沒有選取時顯示空狀態（spec live-output「沒有選取時顯示空狀態」）---
    // direction-01-visual task 4.1：「畫出來」一律用 getClientRects() 非空判斷（祖先
    // display:none 時也是空），不看 `.hidden` 屬性——spec 要的是畫面上沒有「取消選取」，不管它是
    // 用 hidden 屬性還是 CSS 收起。空狀態文案逐字比對 design D7。
    const empty = await cdp.eval(`(() => {
      ${LO1_PANEL_STATE_FN}
      return panelState();
    })()`);
    // fix round 2：[LO1/align] 否定對照 (a) 用——空狀態加內距時的 content-box 起點（量完即移除）。
    const emptyPadded = await cdp.eval(`(() => {
      ${LO1_PANEL_STATE_FN}
      var s = document.createElement('style');
      s.textContent = '#output .output-empty { padding: 4px 0 0 6px !important; }';
      document.head.appendChild(s);
      var st = panelState();
      s.remove();
      return st;
    })()`);
    check(
      empty.panelShown === true && empty.isOpen === false,
      `Live Output 面板應該常駐可見（即使沒有選取；design D7），且沒有 .is-open（實際 ${JSON.stringify(empty)}）`
    );
    check(
      empty.emptyShown === true && empty.emptyText === LO1_EMPTY_TEXT,
      `沒有選取時顯示空狀態文案（逐字「${LO1_EMPTY_TEXT}」；實際 ${JSON.stringify(empty.emptyText)}，shown=${empty.emptyShown}）`
    );
    check(
      empty.closeShown === false && empty.titleShown === false && empty.textShown === false,
      `沒有選取時不應該畫出「取消選取」、標題與內容框（實際 close=${empty.closeShown} title=${empty.titleShown} text=${empty.textShown}）`
    );
    // 否定對照：把 .output-close 強制畫出來時，上面的判準必須判定「有畫出」，證明它不是恆假。
    const closeNeg = await cdp.eval(`(() => {
      ${LO1_PANEL_STATE_FN}
      var s = document.createElement('style');
      s.textContent = '#output .output-close, #output .output-header { display: inline-block !important; }';
      document.head.appendChild(s);
      var shownWhenForced = panelState().closeShown;
      s.remove();
      return { shownWhenForced: shownWhenForced, shownAfter: panelState().closeShown };
    })()`);
    check(
      closeNeg.shownWhenForced === true && closeNeg.shownAfter === false,
      `否定對照：強制畫出 .output-close 時「畫出來」判準應該轉為 true、移除後回到 false（實際 ${JSON.stringify(closeNeg)}）`
    );
    check(
      !!empty.emptyRect && !!empty.contentBox &&
        Math.abs(empty.emptyRect.top - empty.contentBox.top) <= 1 &&
        Math.abs(empty.emptyRect.bottom - empty.contentBox.bottom) <= 1,
      `[LO1/fill] 空狀態佔滿面板內容區（M6；emptyRect=${JSON.stringify(empty.emptyRect)}，contentBox=${JSON.stringify(empty.contentBox)}）`
    );
    check(
      !!empty.panel && empty.panel.height >= 239.5,
      `[LO1/fill] 1536×1024 空狀態時面板高度 ≥240px（design D3；實際 ${empty.panel && empty.panel.height}）`
    );
    // fix round 2／Codex（F4 未完成）：原本用「載入後的請求數」當基準、只驗證「沒有增加」——
    // 若錯誤實作在頁面載入當下就已經偷發一筆 output-request，那筆會被算進基準值，之後「沒有
    // 增加」照樣通過，等於完全測不到「預載時發過一次」這種違規。改成觀察期結束後直接斷言總數
    // 是 0（spec live-output「沒有選取時顯示空狀態」：「頁面沒有發出輸出請求」，沒有「一開始
    // 除外」這種例外）。
    await sleep(1500);
    check(
      preview.requests.length === 0,
      `沒有選取時（含頁面載入後的整段觀察期）完全不應該有任何 output-request（實際 ${preview.requests.length} 筆：${JSON.stringify(preview.requests)}）`
    );
    // 否定對照：手動模擬「預載時已經偷發一筆 output-request」（直接塞一筆假資料進
    // preview.requests，不透過真的網路請求——這裡要驗證的是斷言邏輯本身，不是重新驗證
    // output.js），證明「總數必須是 0」這條斷言真的會抓到這種情況，不是只驗證「沒有增加」。
    const fakePreloadEntry = { runtime: 'win', pane: 'wJ:p1', at: Date.now(), __fakeForNegControl: true };
    preview.requests.push(fakePreloadEntry);
    const wouldFailWithPreload = preview.requests.length !== 0;
    preview.requests.pop(); // 清掉假資料，不污染後面「點 pane 列」要看到的真實請求紀錄。
    check(
      wouldFailWithPreload === true,
      `否定對照：模擬「預載時已經偷發一筆 output-request」後，「總數必須是 0」這條斷言應該判定不通過（實際 wouldFailWithPreload=${wouldFailWithPreload}）— 證明上面的斷言真的看總數，不是只看「有沒有比某個基準值增加」`
    );

    // --- 點 pane 列（spec「點 pane 列」）---
    const clicked = await cdp.click('.pane-row[data-runtime="win"][data-pane="wJ:p1"]');
    check(clicked, '找得到並點到 wJ:p1 的 pane 列');
    if (clicked) {
      await cdp.waitFor(
        '(() => { var r = document.querySelector(\'.pane-row[data-pane="wJ:p1"]\'); return !!r && r.classList.contains("selected"); })()',
        2000,
        '該列出現選定標示'
      );
      const title = await cdp.eval("document.querySelector('.output-title') ? document.querySelector('.output-title').textContent : null");
      check(title === 'win / wJ:p1', `面板標題應該顯示 win / wJ:p1（實際 ${JSON.stringify(title)}）`);
      // direction-01-visual task 4.1：由空狀態改為有選取——空狀態收起，標題、內容框、「取消選取」
      // 畫出來，面板仍是同一個區塊、同樣高度（M6：輸出窗撐滿區塊，不隨內容多寡改變）。
      await cdp.waitFor(
        "(() => { var t = document.querySelector('#output .output-text'); return !!t && t.textContent.length > 0; })()",
        5000,
        '面板開始顯示該 pane 的輸出'
      );
      const opened = await cdp.eval(`(() => {
        ${LO1_PANEL_STATE_FN}
        return panelState();
      })()`);
      check(
        opened.isOpen === true && opened.emptyShown === false && opened.closeShown === true &&
          opened.titleShown === true && opened.textShown === true,
        `點 pane 列後面板由空狀態改為有選取（.is-open、空狀態收起、標題／內容框／「取消選取」畫出來；實際 ${JSON.stringify(opened)}）`
      );
      check(
        !!opened.panel && !!empty.panel && Math.abs(opened.panel.height - empty.panel.height) < 0.5,
        `[LO1/fill] 有選取與空狀態時面板高度相同（空狀態 ${empty.panel && empty.panel.height}，有選取 ${opened.panel && opened.panel.height}）`
      );
      check(
        !!opened.textRect && !!opened.contentBox && Math.abs(opened.textRect.bottom - opened.contentBox.bottom) <= 1,
        `[LO1/fill] 有選取時內容框撐到面板內容區底部（M6；textRect.bottom=${opened.textRect && opened.textRect.bottom}，contentBox.bottom=${opened.contentBox && opened.contentBox.bottom}）`
      );
      // 否定對照：內容框改回 max-height: 40vh（改版前的寫法）且內容很少時，就不會撐到底——
      // 用極短內容＋ flex:none 模擬「內容多高就多高」，判準必須轉紅。
      const fillNeg = await cdp.eval(`(() => {
        ${LO1_PANEL_STATE_FN}
        var s = document.createElement('style');
        s.textContent = '#output .output-text { flex: none !important; height: 3em !important; }';
        document.head.appendChild(s);
        var st = panelState();
        s.remove();
        return { gap: st.textRect && st.contentBox ? st.contentBox.bottom - st.textRect.bottom : null };
      })()`);
      check(
        fillNeg.gap !== null && fillNeg.gap > 1,
        `[LO1/fill] 否定對照：內容框不撐滿時判準應該看得出底部缺口（實際 gap=${fillNeg.gap}）`
      );

      // --- task 4.1 fix round 1（設計審核 I1／I2／M1／M2）---
      // I1：design D8「Live Output 底色用 --bg-deep，像一扇嵌進面板的窗」——內容框撐滿之後不得再是
      // --surface 卡片：背景透明、四邊無框線、無圓角；面板本身在兩種狀態都是 --bg-deep，空狀態也不自帶底色。
      const ts = opened.textStyle;
      check(
        !!ts && ts.backgroundColor === 'rgba(0, 0, 0, 0)' && ts.borderWidths.every((w) => w === '0px') && ts.borderRadius === '0px 0px 0px 0px',
        `[LO1/window] 有選取時 .output-text 背景透明、無框線、無圓角（design D8；實際 ${JSON.stringify(ts)}）`
      );
      check(
        opened.panelBg === SPEC_COLORS.bgDeep && empty.panelBg === SPEC_COLORS.bgDeep && empty.emptyBg === 'rgba(0, 0, 0, 0)',
        `[LO1/window] 面板在空狀態與有選取時都是 --bg-deep、空狀態不自帶底色（實際 空狀態 ${empty.panelBg}／${empty.emptyBg}，有選取 ${opened.panelBg}）`
      );
      // I2：spec cockpit-dashboard「時間、id、數值與 Live Output 使用等寬字體」＋D11——標題的 runtime／pane
      // id 用 --font-mono；字級字重維持面板標題 14px／600（D10）。
      const tt = opened.titleStyle;
      check(
        !!tt && tt.fontFamily === opened.monoFamily && tt.fontSize === '14px' && tt.fontWeight === '600',
        `[LO1/mono] 標題 win / wJ:p1 用 --font-mono、14px／600（實際 ${JSON.stringify(tt)}，--font-mono＝${opened.monoFamily}）`
      );
      // M1：D10「行高……Live Output 1.4」（12px × 1.4 ＝ 16.8px）。
      check(
        !!ts && ts.fontSize === '12px' && Math.abs(parseFloat(ts.lineHeight) - 16.8) < 0.05,
        `[LO1/lh] 輸出內容行高 1.4（12px → 16.8px；實際 ${ts && ts.fontSize}／${ts && ts.lineHeight}）`
      );
      // M2＋fix round 2（Codex r1 medium／設計 N1）：空狀態、標題列、輸出內容三者的 content-box
      // 起點對齊（layout invariant，與字型無關）：left 三者兩兩差 <0.5px；top 比空狀態與標題列
      // （兩者都是面板內容區的第一個東西）差 <0.5px。輸出內容在標題列下方，只比 left。
      const alignResult = lo1Align(empty.emptyContent, opened.headerContent, opened.textContent);
      check(
        alignResult.ok,
        `[LO1/align] 空狀態／標題列／輸出內容的 content-box 起點對齊（left 差 <0.5px、空狀態與標題列 top 差 <0.5px；實際 ${JSON.stringify(alignResult)}）`
      );
      // 否定對照（三種錯位各自必須判定不通過）：(a) 空狀態加左內距（空狀態時量）、(b) 輸出內容
      // 保留左內距、(c) 標題列 relative 位移。
      const alignNeg = await cdp.eval(`(() => {
        ${LO1_PANEL_STATE_FN}
        function withStyle(css, fn) {
          var s = document.createElement('style');
          s.textContent = css;
          document.head.appendChild(s);
          try { return fn(); } finally { s.remove(); }
        }
        return {
          textPadded: withStyle('#output .output-text { padding-left: 8px !important; }', function () { return panelState(); }),
          headerShifted: withStyle('#output .output-header { position: relative !important; top: 3px !important; left: 3px !important; }', function () { return panelState(); }),
          // fix round 3（Codex r2 medium）：標題列 +0.49px、輸出內容 −0.49px——各自對空狀態都 <0.5px，
          // 但兩者彼此差約 0.98px；「三者兩兩 <0.5px」必須判定不通過。
          splitShift: withStyle('#output .output-header { position: relative !important; left: 0.49px !important; } #output .output-text { position: relative !important; left: -0.49px !important; }', function () { return panelState(); }),
        };
      })()`);
      const negText = lo1Align(empty.emptyContent, alignNeg.textPadded.headerContent, alignNeg.textPadded.textContent);
      const negHeader = lo1Align(empty.emptyContent, alignNeg.headerShifted.headerContent, alignNeg.headerShifted.textContent);
      const negEmpty = lo1Align(emptyPadded.emptyContent, opened.headerContent, opened.textContent);
      const negSplit = lo1Align(empty.emptyContent, alignNeg.splitShift.headerContent, alignNeg.splitShift.textContent);
      check(
        negSplit.ok === false,
        `[LO1/align] 否定對照：標題列 +0.49px、輸出內容 −0.49px（彼此差約 0.98px）必須判定不通過（實際 ${JSON.stringify(negSplit)}）`
      );
      check(
        negText.ok === false && negHeader.ok === false && negEmpty.ok === false,
        `[LO1/align] 否定對照：輸出內容加左內距、標題列 relative 位移、空狀態加內距都必須判定不通過（實際 text=${JSON.stringify(negText)} header=${JSON.stringify(negHeader)} empty=${JSON.stringify(negEmpty)}）`
      );
    }

    // --- 面板打開時仍可操作頁面下方的內容（命中測試工具）---
    const hitResults = await cdp.eval(`(() => {
      var out = [];
      document.querySelectorAll('.pane-row[data-action="select-pane"]').forEach(function (n, i) {
        out.push(Object.assign({ kind: 'pane-row#' + i }, window.__cockpitVisualTools.hitCenter(n)));
      });
      document.querySelectorAll('.action-button').forEach(function (n, i) {
        out.push(Object.assign({ kind: 'action-button#' + i }, window.__cockpitVisualTools.hitCenter(n)));
      });
      document.querySelectorAll('[data-action="select-project"]').forEach(function (n, i) {
        out.push(Object.assign({ kind: 'project-item#' + i }, window.__cockpitVisualTools.hitCenter(n)));
      });
      return out;
    })()`);
    const projectItems = hitResults.filter((r) => r.kind.startsWith('project-item'));
    check(
      projectItems.length > 0,
      `應該至少有一個可測的 Project 項目（design D6 的 select-project 尚未實作，預期 FAIL，0 個）`
    );
    const visibleTargets = hitResults.filter((r) => r.visible !== false);
    check(visibleTargets.length > 0, `應該至少有一個可見的命中測試目標（實際 ${visibleTargets.length}）`);
    const blocked = visibleTargets.filter((r) => r.selfHit === false);
    check(
      blocked.length === 0,
      `每個可選 pane 列／畫面操作按鈕／Project 項目的可見範圍中心點應該命中自己（不是 Live Output 面板）；被蓋住的：${JSON.stringify(blocked.map((r) => r.kind))}`
    );
    // spec「點另一個 pane 列即改為選取該 pane」：direction-01-visual task 4.1 起一律實測（原本只在
    // 有元素被蓋住時才點），並斷言選取與標題真的換過去——也讓下面「鍵盤選定」wJ:p1 的 Enter 是從
    // 另一個選取切回來，不是對已選定的列按 Enter（那樣標題本來就相符、斷言恆真）。
    const anotherPane = await cdp.click('.pane-row[data-runtime="win"][data-pane="wJ:p3"]');
    check(anotherPane, '面板有選取時點另一個 pane 列（wJ:p3）');
    if (anotherPane) {
      await cdp.waitFor(
        "(() => { var t = document.querySelector('.output-title'); var r = document.querySelector('.pane-row[data-pane=\"wJ:p3\"]'); var o = document.querySelector('.pane-row[data-pane=\"wJ:p1\"]'); return !!t && t.textContent === 'win / wJ:p3' && !!r && r.classList.contains('selected') && !!o && !o.classList.contains('selected'); })()",
        2000,
        '點另一個 pane 列即改為選取該 pane（標題 win / wJ:p3、標示換到 wJ:p3）'
      );
    }

    // --- 鍵盤選定（spec「鍵盤選定」）---
    await cdp.eval("document.querySelector('.pane-row[data-runtime=\"win\"][data-pane=\"wJ:p2\"]') ? void 0 : void 0; true");
    const focusable = await cdp.eval(`(() => {
      var n = document.querySelector('.pane-row[data-runtime="win"][data-pane="wJ:p1"]');
      if (!n) return false;
      n.focus();
      return document.activeElement === n;
    })()`);
    check(focusable === true, 'wJ:p1 的 pane 列可以取得鍵盤焦點');
    if (focusable) {
      await cdp.pressKey('Enter', 'Enter', 13, '\r');
      await cdp.waitFor(
        "document.querySelector('.output-title') && document.querySelector('.output-title').textContent === 'win / wJ:p1'",
        2000,
        'Enter 選定該 pane，面板顯示其 runtime 與 pane id'
      );
    }

    // --- 取消選取（spec「取消選取」）---
    // 控制端裁決（2026-09-23）：面板上原本叫「關閉」的按鈕改名為「取消選取」。定位一律用結構
    // （`.output-close` class／`#output` 內的位置），不依賴按鈕文字；文字斷言比對新名稱「取消選取」。
    const closeBtn = await cdp.eval("!!document.querySelector('.output-close')");
    if (closeBtn) {
      const closeBtnText = await cdp.eval("document.querySelector('.output-close').textContent");
      check(
        closeBtnText === '取消選取',
        `面板上的按鈕文字應該是「取消選取」（不是「關閉」；design 改名，spec live-output「取消選取」；實際 ${JSON.stringify(closeBtnText)}）`
      );
      // task 4.1 fix round 1（Codex medium／設計 M3）：以鍵盤觸發「取消選取」（CDP 真按鍵，不是
      // element.click()），之後焦點要回到剛取消的那一列 pane（重畫後的新節點，以 data-runtime／
      // data-pane 辨認），不是掉到 body。
      const closeFocused = await cdp.eval(
        "(() => { var b = document.querySelector('.output-close'); b.focus(); return document.activeElement === b; })()"
      );
      check(closeFocused, '「取消選取」按鈕可以取得鍵盤焦點');
      await cdp.pressKey('Enter', 'Enter', 13, '\r');
      await sleep(300);
      const focusAfterCancel = await cdp.eval(LO1_ACTIVE_ELEMENT_JS);
      check(
        focusAfterCancel.isPaneRow && focusAfterCancel.runtime === 'win' && focusAfterCancel.pane === 'wJ:p1' && focusAfterCancel.connected,
        `[LO1/focus] 以 Enter 按「取消選取」後焦點回到剛取消的 wJ:p1 pane 列（實際 ${JSON.stringify(focusAfterCancel)}）`
      );
      const afterClose = await cdp.eval(`(() => {
        ${LO1_PANEL_STATE_FN}
        var st = panelState();
        st.anySelected = document.querySelectorAll('.pane-row.selected').length;
        st.panelText = document.getElementById('output') ? document.getElementById('output').textContent : null;
        return st;
      })()`);
      check(
        afterClose.anySelected === 0,
        `按「取消選取」後不應該有任何 pane 列有選定標示（實際 ${afterClose.anySelected}）`
      );
      check(
        afterClose.panelShown === true &&
          afterClose.isOpen === false &&
          afterClose.emptyShown === true &&
          afterClose.emptyText === LO1_EMPTY_TEXT &&
          afterClose.closeShown === false &&
          afterClose.titleShown === false &&
          afterClose.textShown === false,
        `按「取消選取」後面板應該回到常駐的空狀態（不是整個收起；design D7；實際 ${JSON.stringify(afterClose)}）`
      );
      check(
        typeof afterClose.panelText === 'string' && afterClose.panelText.indexOf('wJ:p1') === -1,
        `回到空狀態後面板裡不應該殘留上一個選取的標題或內容（textContent 不含 wJ:p1；實際 ${JSON.stringify(afterClose.panelText)}）`
      );
      check(
        !!afterClose.panel && !!empty.panel && Math.abs(afterClose.panel.height - empty.panel.height) < 0.5,
        `[LO1/fill] 取消選取後面板高度與一開始的空狀態相同（${empty.panel && empty.panel.height} vs ${afterClose.panel && afterClose.panel.height}）`
      );
      // fix round 1／Codex F4（採半）：取消選取後，跨過至少一個輪詢週期不應該再有新的
      // output-request——用 preview 的請求紀錄實測，不是只看畫面。按「取消選取」前一刻可能
      // 已經有一個輪詢請求飛出去、還沒落地，屬於正常現象（output.js 沒有 AbortController
      // 取消已發出的請求；這裡量的是「取消後有沒有再發『新』的」）：先等一個輪詢間隔讓這種
      // 已飛出的請求落地、記下這時的筆數，再開一段乾淨的觀察視窗。
      await sleep(1200);
      const wJp1RequestsAfterFlight = preview.requests.filter((r) => r.pane === 'wJ:p1').length;
      await sleep(1500);
      const wJp1RequestsAfterWindow = preview.requests.filter((r) => r.pane === 'wJ:p1').length;
      check(
        wJp1RequestsAfterWindow === wJp1RequestsAfterFlight,
        `取消選取後（排除取消當下已在飛的請求），跨過一個以上輪詢週期不應該再有新的 wJ:p1 output-request（實際從 ${wJp1RequestsAfterFlight} 筆增加到 ${wJp1RequestsAfterWindow} 筆）`
      );

      // [LO1/focus] 否定對照：把焦點交接拿掉（暫時把 HTMLElement.prototype.focus 換成空函數，
      // 在「取消選取」按鈕已經聚焦之後才換，所以只擋得到產品取消後的那次交接）——同一個判準必須
      // 判定不通過（焦點掉到 body）。
      await cdp.click('.pane-row[data-runtime="win"][data-pane="wJ:p1"]');
      await cdp.waitFor(
        "(() => { var t = document.querySelector('.output-title'); return !!t && t.textContent === 'win / wJ:p1'; })()",
        2000,
        '否定對照前重新選定 wJ:p1'
      );
      await cdp.eval(
        "(() => { var b = document.querySelector('.output-close'); b.focus(); window.__origFocus = HTMLElement.prototype.focus; HTMLElement.prototype.focus = function () {}; return true; })()"
      );
      await cdp.pressKey('Enter', 'Enter', 13, '\r');
      await sleep(300);
      const focusNeg = await cdp.eval(LO1_ACTIVE_ELEMENT_JS);
      await cdp.eval('HTMLElement.prototype.focus = window.__origFocus; true');
      check(
        !(focusNeg.isPaneRow && focusNeg.runtime === 'win' && focusNeg.pane === 'wJ:p1' && focusNeg.connected),
        `[LO1/focus] 否定對照：拿掉焦點交接時，「焦點回到 pane 列」判準必須不通過（實際 ${JSON.stringify(focusNeg)}）`
      );

      // [LO1/focus] fallback：取消的那一列已不在畫面上（pane 從投影消失 → 「pane 已不存在」）時，
      // 以 Space 按「取消選取」，焦點退回 #output 面板本身（tabindex="-1"），不掉到 body。
      // 這裡用 design D9 的注入工具把 wJ:p3 從投影拿掉，之後推送一律被擋住，所以放在本段最後。
      await cdp.click('.pane-row[data-runtime="win"][data-pane="wJ:p3"]');
      await cdp.waitFor(
        "(() => { var t = document.querySelector('.output-title'); return !!t && t.textContent === 'win / wJ:p3'; })()",
        2000,
        'fallback 前選定 wJ:p3'
      );
      const vanished = loadFixture();
      for (const rt of vanished.runtimes) {
        for (const ws of rt.workspaces || []) {
          for (const tab of ws.tabs || []) {
            tab.panes = (tab.panes || []).filter((p) => !(rt.id === 'win' && p.id === 'wJ:p3'));
          }
        }
      }
      const injected = await injectState(cdp, vanished);
      check(injected && injected.ok === true, 'fallback：注入不含 wJ:p3 的投影');
      await cdp.waitFor(
        "(() => { var g = document.querySelector('.output-gone-notice'); return !!g && !g.hidden && !document.querySelector('.pane-row[data-pane=\"wJ:p3\"]'); })()",
        3000,
        'fallback：wJ:p3 列已消失、面板顯示「pane 已不存在」'
      );
      const closeFocused2 = await cdp.eval(
        "(() => { var b = document.querySelector('.output-close'); b.focus(); return document.activeElement === b; })()"
      );
      check(closeFocused2, 'fallback：「取消選取」按鈕仍在畫面上且可以取得鍵盤焦點');
      await cdp.pressKey(' ', 'Space', 32, ' ');
      await sleep(300);
      const focusFallback = await cdp.eval(LO1_ACTIVE_ELEMENT_JS);
      check(
        focusFallback.isOutput === true,
        `[LO1/focus] fallback：列不存在時以 Space 取消選取後焦點落在 #output 面板（實際 ${JSON.stringify(focusFallback)}）`
      );
      check(
        (await cdp.eval(`(() => { ${LO1_PANEL_STATE_FN} var s = panelState(); return s.emptyShown && !s.isOpen; })()`)) === true,
        'fallback：取消選取後面板回到空狀態'
      );
    } else {
      check(false, '找不到「取消選取」按鈕（結構定位 .output-close）可以測「取消選取」');
    }
  } finally {
    await stopChrome(chrome, 'chrome-LO1');
    await stopPreview(preview, 'preview-LO1');
  }
}

// ---------------------------------------------------------------------------
// CL1：清理——style.css 的每一條選擇器，至少在一種畫面狀態下對得到元素（direction-01-visual
// task 5.1）。tasks.md 5.1：「刪除 style.css 中已無元素使用的舊規則（以 visual-check.js 走訪
// DOM 列出實際用到的 class 對照）」。只走訪一種畫面會誤刪只在特定狀態出現的規則，所以依序
// 走訪：預設投影、輸出截斷、過期＋失敗原因、pane 已不存在、空狀態、兩則提示＋改綁模式、通道
// disconnected／connecting／未知字串、防禦狀態（五種已知 agent 狀態＋未知字串、兩個 tab、
// runtime connecting 與未知連線字串、未知 task status）、沒有 Project，再把預設投影放回去
// 走 700／1100／1280×650／1536 四種視窗。每個狀態都在頁面內用 CSSOM 讀 style.css（只看
// href 結尾 /app/style.css 那一份，不含腳本注入的 <style>）的每一條樣式規則（含 @media 內層），
// 把選擇器清單在頂層逗號切開，拿掉只在使用者互動時才成立的偽類（:hover／:focus-visible／
// :focus／:active）與偽元素（::before／::after），用 querySelector 看當下 DOM 有沒有元素對得上。
// 斷言：全部狀態走完後，沒有任何一條選擇器從頭到尾對不到元素（對不到＝死規則，應該刪）。
// 否定對照：注入一份含「沒人用的 class」與「有人用的 class」的 <style>，同一套判定必須只抓出
// 前者。另外印出走訪期間 DOM 出現過的全部 class（report 的 class 清單來源）。
// fix round 1（Codex medium）：「對得到」＝選擇器命中元素**且**所有外層 @media／@supports 在當下成立
// （matchMedia／CSS.supports）；每個狀態都在 1536×1024、1280×650、1100×900、700×900 各取樣一次。
// ---------------------------------------------------------------------------

// 頁面內收集器（字串形式的函式，呼叫端以 `(${CL1_COLLECT_FN})(tag, storeName, sheetId)` 執行）。
// storeName：累積結果的 window 屬性名（window 上的變數不會被整頁重畫清掉，可以跨狀態累積）；
// sheetId：null＝style.css，否則只看 id 相符的 <style>（否定對照用）。
const CL1_COLLECT_FN = `function (stateTag, storeName, sheetId) {
  var store = window[storeName] || (window[storeName] = { selectors: {}, classes: {}, states: [] });
  store.states.push(stateTag);
  function splitTopLevel(text) {
    var parts = [], depth = 0, cur = '';
    for (var i = 0; i < text.length; i += 1) {
      var ch = text.charAt(i);
      if (ch === '(' || ch === '[') depth += 1;
      else if (ch === ')' || ch === ']') depth -= 1;
      if (ch === ',' && depth === 0) { parts.push(cur.trim()); cur = ''; } else { cur += ch; }
    }
    if (cur.trim() !== '') parts.push(cur.trim());
    return parts;
  }
  function strip(sel) {
    var s = sel.replace(/::?(?:hover|focus-visible|focus-within|focus|active|before|after)(?![-\\w(])/g, '').trim();
    if (s === '') return '*';
    if (/[>+~]$/.test(s)) s += ' *';
    return s;
  }
  var sheets = Array.prototype.slice.call(document.styleSheets).filter(function (sh) {
    if (sheetId) return !!sh.ownerNode && sh.ownerNode.id === sheetId;
    return !!sh.href && /\\/app\\/style\\.css$/.test(sh.href);
  });
  var seenNow = 0, matchedNow = 0;
  // fix round 1（Codex medium）：外層 @media／@supports 的條件跟著遞迴往下帶——當下條件不成立的
  // 規則，就算選擇器對得到元素，這個狀態也不算「用得到」（規則根本不會套用）。只有選擇器命中且
  // 所有外層條件在當下成立，才記進 matchedIn。
  function conditionHolds(rule) {
    if (typeof CSSMediaRule !== 'undefined' && rule instanceof CSSMediaRule) return window.matchMedia(rule.media.mediaText).matches;
    if (typeof CSSSupportsRule !== 'undefined' && rule instanceof CSSSupportsRule) return CSS.supports(rule.conditionText);
    return true;
  }
  function walk(rules, context, active) {
    for (var i = 0; i < rules.length; i += 1) {
      var rule = rules[i];
      if (rule.type === 1) {
        var list = splitTopLevel(rule.selectorText);
        for (var j = 0; j < list.length; j += 1) {
          var key = context + ' ｜ ' + list[j];
          var entry = store.selectors[key] || (store.selectors[key] = { context: context, selector: list[j], probe: strip(list[j]), matchedIn: [], error: null });
          seenNow += 1;
          var hit = false;
          try { hit = document.querySelector(entry.probe) !== null; } catch (e) { entry.error = String(e && e.message || e); }
          if (hit && active) {
            matchedNow += 1;
            if (entry.matchedIn.indexOf(stateTag) === -1) entry.matchedIn.push(stateTag);
          }
        }
      } else if (rule.cssRules) {
        var cond = rule.conditionText || (rule.media && rule.media.mediaText) || '';
        walk(rule.cssRules, (context ? context + ' ' : '') + '@' + cond, active && conditionHolds(rule));
      }
    }
  }
  sheets.forEach(function (sh) { walk(sh.cssRules, '', true); });
  Array.prototype.forEach.call(document.querySelectorAll('*'), function (el) {
    el.classList.forEach(function (c) { store.classes[c] = true; });
  });
  return { sheets: sheets.length, seenNow: seenNow, matchedNow: matchedNow };
}`;

const CL1_REPORT_FN = `function (storeName) {
  var store = window[storeName];
  if (!store) return null;
  var entries = Object.keys(store.selectors).map(function (k) { return store.selectors[k]; });
  return {
    total: entries.length,
    unmatched: entries.filter(function (e) { return e.matchedIn.length === 0; }).map(function (e) { return { context: e.context, selector: e.selector, probe: e.probe, error: e.error }; }),
    errors: entries.filter(function (e) { return e.error !== null; }).map(function (e) { return e.selector + '：' + e.error; }),
    classes: Object.keys(store.classes).sort(),
    states: store.states,
  };
}`;

// CL1 防禦狀態：五種已知 agent 狀態＋未知字串（working／done／unknown 來自 fixture，補 idle、
// blocked、whatever）、同一 workspace 兩個 tab（`.tab + .tab`）、runtime connecting 與未知連線
// 字串（`-unknown` 系列是「未知不破壞畫面」的防禦規則，render.js 的 connStateClass() 對任何
// 未知字串都會產生）、未知 task status（`.task-status-unknown`／`.project-count-unknown`）。
function craftCl1DefensiveState() {
  const s = loadFixture();
  s.version = 5101;
  const win = s.runtimes.find((r) => r.id === 'win');
  const tab = win.workspaces[0].tabs[0];
  const template = tab.panes[0];
  ['idle', 'blocked', 'whatever'].forEach((status, i) => {
    const p = JSON.parse(JSON.stringify(template));
    p.id = 'wJ:c' + i;
    p.agent_status = status;
    p.focused = false;
    tab.panes.push(p);
  });
  const tab2 = JSON.parse(JSON.stringify(tab));
  tab2.id = 'wJ:t2';
  tab2.number = 2;
  tab2.focused = false;
  tab2.panes = tab2.panes.slice(0, 1).map((p) => ({ ...p, id: 'wJ:t2p1', focused: false }));
  win.workspaces[0].tabs.push(tab2);
  const connecting = JSON.parse(JSON.stringify(s.runtimes.find((r) => r.id === 'wsl')));
  connecting.id = 'lab';
  connecting.connection = { state: 'connecting' };
  const weird = JSON.parse(JSON.stringify(connecting));
  weird.id = 'odd';
  weird.connection = { state: 'cl1-unknown-state' };
  s.runtimes.push(connecting, weird);
  // protocol 版本不相符的警告（fixture 的 protocol_warning 是 null，.protocol-warning 只在有警告時才畫）。
  win.connection.protocol_warning = 'CL1：HERDR protocol 與 cockpit 預期的版本不同';
  const project = s.projects.find((p) => p.id === 'cockpit');
  project.tasks.find((t) => t.id === 'be-2').status = 'whatever';
  // progress-model task 4.3：列首「工作中・未宣告 task」（.ff-undeclared）只在 activity_undeclared 為
  // true 時才畫；cockpit 專案是預設選定、fixture 沒有 true 的 workstream，這裡設一個讓該規則有元素可對。
  project.workstreams.find((w) => w.id === 'docs').activity_undeclared = true;
  return s;
}

async function partCssInventory() {
  log('=== CL1. 清理：style.css 每一條選擇器至少在一種畫面狀態下對得到元素（direction-01-visual task 5.1）===');
  let preview = null;
  let chrome = null;
  try {
    preview = await startPreview(
      {
        COCKPIT_PREVIEW_WRITE_RULES: '/api/projects/cockpit/tasks/be-1/fail=0:409',
        // wJ:p1 一律 503（過期＋失敗原因）；wJ:p3 維持預設 long（截斷提示）；wJ:p2 預設 404（pane 已不存在）。
        // live-output-color task 5.2：wJ:p5 回 ansi 樣本（上色片段與過期覆寫規則才有元素可對）。
        COCKPIT_PREVIEW_OUTPUT_MODES: 'wJ:p1=fail:1000000;wJ:p5=ansi',
      },
      'preview-CL1'
    );
    const url = `http://127.0.0.1:${preview.port}/`;
    chrome = await startChrome(pickPort(19140, [preview.port]), url, 'chrome-CL1', '1536,1024');
    const { cdp } = chrome;
    await waitForFirstProjection(cdp, preview.port);
    await cdp.send('Emulation.setDeviceMetricsOverride', { width: 1536, height: 1024, deviceScaleFactor: 1, mobile: false });
    await sleep(150);

    // fix round 1：規則要「選擇器命中且外層 @media 成立」才算用得到，所以每個畫面狀態都在四種視窗
    // （固定一屏、寬但矮、兩欄、單欄）各取樣一次——例如兩則提示只在 1536 取樣的話，寬矮版面裡
    // `.shell:has([data-region="banner"]) [data-region="floor"]` 就永遠不會被算到。取樣完回到
    // 1536×1024，後續操作的版面不變。
    const CL1_VIEWPORTS = [[1536, 1024], [1280, 650], [1100, 900], [700, 900]];
    const collectIn = async (tag, storeName, sheetId) => {
      let last = null;
      for (const [w, h] of CL1_VIEWPORTS) {
        await cdp.send('Emulation.setDeviceMetricsOverride', { width: w, height: h, deviceScaleFactor: 1, mobile: false });
        await sleep(120);
        last = await cdp.eval(`(${CL1_COLLECT_FN})(${JSON.stringify(tag + ' @' + w + '×' + h)}, ${JSON.stringify(storeName)}, ${JSON.stringify(sheetId)})`);
      }
      await cdp.send('Emulation.setDeviceMetricsOverride', { width: 1536, height: 1024, deviceScaleFactor: 1, mobile: false });
      await sleep(120);
      return last;
    };
    const collect = async (tag) => {
      const r = await collectIn(tag, '__cl1', null);
      check(!!r && r.sheets === 1 && r.seenNow > 0, `[CL1] ${tag}：讀到 style.css 的規則（sheets=${r && r.sheets}，700×900 取樣對到 ${r && r.matchedNow}／${r && r.seenNow} 條選擇器）`);
    };

    await collect('預設投影 1536×1024（沒有選取）');

    await cdp.eval("window.liveOutput.select('win', 'wJ:p3'); true");
    await cdp.waitFor("(() => { var n = document.querySelector('#output .output-truncated-notice'); return !!n && !n.hidden; })()", 5000, '[CL1] 選 wJ:p3：截斷提示出現');
    await collect('輸出（截斷提示）');

    await cdp.eval("window.liveOutput.select('win', 'wJ:p1'); true");
    await cdp.waitFor("(() => { var o = document.getElementById('output'); var r = o.querySelector('.output-error-reason'); return o.classList.contains('is-stale') && !!r && !r.hidden; })()", 5000, '[CL1] 選 wJ:p1（503）：過期＋失敗原因出現');
    await collect('輸出過期＋失敗原因');

    // live-output-color task 5.2（spec live-output「輸出依樣式上色」「失敗與消失的呈現」）：`ansi-*` 規則
    // 與 `.output-panel.is-stale .output-text span` 只在輸出有上色片段時才有元素——先選 wJ:p5（ansi 樣本）
    // 走一次上色畫面，再把 /output 請求換成 503，走一次「過期＋上色片段」，最後還原 fetch。
    await cdp.eval("window.liveOutput.select('win', 'wJ:p5'); true");
    await cdp.waitFor("!!document.querySelector('#output .output-text span.ansi-fg')", 5000, '[CL1] 選 wJ:p5（ansi）：出現上色片段');
    await collect('輸出（上色片段）');
    await cdp.eval(`(() => {
      if (!window.__cl1OrigFetch) window.__cl1OrigFetch = window.fetch;
      window.fetch = function (input, init) {
        var u = typeof input === 'string' ? input : input.url;
        if (u.indexOf('/output') !== -1) {
          return Promise.resolve(new Response(JSON.stringify({ error: 'CL1 模擬暫時失敗' }), { status: 503, headers: { 'content-type': 'application/json' } }));
        }
        return window.__cl1OrigFetch.apply(this, arguments);
      };
      return true;
    })()`);
    await cdp.waitFor("(() => { var o = document.getElementById('output'); return o.classList.contains('is-stale') && !!o.querySelector('.output-text span.ansi-fg'); })()", 5000, '[CL1] 模擬 503：過期且上色片段仍在');
    await collect('輸出過期＋上色片段');
    await cdp.eval("(() => { if (window.__cl1OrigFetch) { window.fetch = window.__cl1OrigFetch; } return true; })()");

    await cdp.eval("window.liveOutput.select('win', 'wJ:p2'); true");
    await cdp.waitFor("(() => { var n = document.querySelector('#output .output-gone-notice'); return !!n && !n.hidden; })()", 5000, '[CL1] 選 wJ:p2（404）：pane 已不存在出現');
    await collect('pane 已不存在');

    await cdp.eval('window.liveOutput.clear(); true');
    await sleep(100);
    await collect('Live Output 空狀態');

    // 從 pane 列點選（actions.js 的選取狀態，.pane-row.selected；liveOutput.select() 只動面板，不標列）。
    await cdp.click('.pane-row.selectable[data-pane="wJ:p3"]');
    await cdp.waitFor("!!document.querySelector('.pane-row.selected[data-pane=\"wJ:p3\"]')", 3000, '[CL1] 點 wJ:p3 列：列標為 selected');
    await collect('點選 pane 列');
    await cdp.eval('window.liveOutput.clear(); true');

    // file-review task 4.2：左欄「檔案」分頁的檔案樹（spec file-review「左欄檔案樹」新增的畫面狀態）——
    // 樹列、展開的資料夾、資料夾讀取失敗（暫存副本裡先建 cl1-gone/，樹列出後刪掉再展開 → not_found）、
    // 根目錄查詢失敗（wJ:p1 的 cwd 在 fixture 裡不存在 → no_root）。只寫 ui_preview 的暫存副本。
    for (let i = 0; i < 40 && !preview.fixturePaths.reviewRepo; i++) await sleep(50);
    const cl1Repo = preview.fixturePaths.reviewRepo;
    check(!!cl1Repo, `[CL1] 讀到 ui_preview 印出的 review-repo 暫存路徑（${cl1Repo}）`);
    const cl1Gone = path.join(cl1Repo, 'cl1-gone');
    fs.mkdirSync(cl1Gone, { recursive: true });
    await cdp.eval("window.liveOutput.select('win', 'wJ:p4'); true");
    await cdp.click('#files-tab-files');
    await cdp.waitFor("!!document.querySelector('#files [role=\"treeitem\"][title=\"cl1-gone\"]')", 5000, '[CL1] 左欄檔案樹列出 review-repo 第一層');
    fs.rmSync(cl1Gone, { recursive: true, force: true });
    await cdp.click('#files [role="treeitem"][title="src"]');
    await cdp.waitFor("!!document.querySelector('#files [role=\"treeitem\"][title=\"src/main.rs\"]')", 5000, '[CL1] 展開 src');
    await cdp.click('#files [role="treeitem"][title="cl1-gone"]');
    await cdp.waitFor("!!document.querySelector('#files .tree-note[data-tone=\"warn\"]')", 5000, '[CL1] 展開已刪除的 cl1-gone：顯示讀取失敗原因');
    await collect('左欄檔案樹（展開、資料夾讀取失敗）');
    // file-review task 4.3：中欄下半部的檔案分頁（spec file-review「檔案分頁」新增的畫面狀態）——開 README.md
    // （分頁、工具列、「在 VS Code 開啟」、檢視器容器）、開一個樹列出後才在暫存副本刪掉的 cl1-gone.md（讀取失敗
    // 原因）。之後選 wJ:p1 時分頁區自動切回 Live Output（spec live-output「選定一個 pane」），檔案分頁留著。
    await cdp.click('#files [role="treeitem"][title="README.md"]');
    await cdp.waitFor("(() => { var t = document.querySelector('#review [role=\"tab\"][data-path=\"README.md\"]'); var p = t && document.getElementById(t.getAttribute('aria-controls')); var a = p && p.querySelector('a'); return !!a && !a.hidden; })()", 5000, '[CL1] 開 README.md 檔案分頁：工具列的「在 VS Code 開啟」出現');
    await collect('檔案分頁（README.md）');
    // file-review task 4.4：檔案檢視器（spec file-review「檔案檢視器」新增的畫面狀態）——note.txt（純文字）、
    // page.html（HTML iframe）、bin.dat（不支援預覽），以及暫存副本裡的 cl1-big.txt（超過 2 MiB：「檔案太大，
    // 無法預覽」）與 cl1-md.md（README.md 沒有的 Markdown 元素：h3–h6、有序清單、引用、分隔線、點了不動作的
    // 連結）。只寫 ui_preview 的暫存副本；各分頁留著（非目前分頁的 tabpanel 只是 hidden，節點仍在）。
    fs.writeFileSync(
      path.join(cl1Repo, 'cl1-md.md'),
      '# cl1-md\n\n### h3\n\n#### h4\n\n##### h5\n\n###### h6\n\n1. 一\n2. 二\n\n> 引用\n\n---\n\n[跳出根目錄](../../outside.md)\n'
    );
    fs.writeFileSync(path.join(cl1Repo, 'cl1-big.txt'), 'x'.repeat(2 * 1024 * 1024 + 1));
    await cdp.click('#files .files-refresh');
    await cdp.waitFor("!!document.querySelector('#files [role=\"treeitem\"][title=\"cl1-big.txt\"]') && !!document.querySelector('#files [role=\"treeitem\"][title=\"cl1-md.md\"]')", 5000, '[CL1] 重新整理後檔案樹列出 cl1-big.txt 與 cl1-md.md');
    for (const [file, viewer] of [['note.txt', 'text'], ['page.html', 'html'], ['bin.dat', 'unsupported'], ['cl1-big.txt', 'text'], ['cl1-md.md', 'markdown']]) {
      await cdp.click(`#files [role="treeitem"][title="${file}"]`);
      await cdp.waitFor(
        `(() => { var t = document.querySelector('#review [role="tab"][data-path="${file}"]'); var p = t && document.getElementById(t.getAttribute('aria-controls')); return !!p && !p.hidden && !!p.querySelector('[data-viewer="${viewer}"]'); })()`,
        5000,
        `[CL1] 開 ${file}：檔案分頁以 ${viewer} 檢視器顯示`
      );
    }
    await collect('檔案檢視器（純文字、HTML、不支援預覽、檔案太大、Markdown 其他元素）');
    // file-review task 4.5：PDF 檢視器（spec file-review「檔案檢視器」的 pdf）——report.pdf（工具列、頁框、canvas；
    // 停在第 1 頁時「上一頁」為 aria-disabled、預設「符合寬度」為 aria-pressed）與暫存副本裡的 cl1-broken.pdf
    // （「PDF 無法解析」，沿用 .viewer-note）。只寫 ui_preview 的暫存副本；兩個分頁都留著。
    fs.writeFileSync(path.join(cl1Repo, 'cl1-broken.pdf'), 'CL1：這不是 PDF，只是改名的文字檔\n');
    await cdp.click('#files .files-refresh');
    await cdp.waitFor("!!document.querySelector('#files [role=\"treeitem\"][title=\"cl1-broken.pdf\"]')", 5000, '[CL1] 重新整理後檔案樹列出 cl1-broken.pdf');
    await cdp.click('#files [role="treeitem"][title="report.pdf"]');
    await cdp.waitFor(
      "(() => { var t = document.querySelector('#review [role=\"tab\"][data-path=\"report.pdf\"]'); var p = t && document.getElementById(t.getAttribute('aria-controls')); var s = p && p.querySelector('[data-viewer=\"pdf\"] .pdf-page-status'); return !!s && s.textContent === '1 / 3' && !!p.querySelector('.pdf-page > canvas'); })()",
      10000,
      '[CL1] 開 report.pdf：PDF 檢視器顯示「1 / 3」與每頁的 canvas'
    );
    await cdp.click('#files [role="treeitem"][title="cl1-broken.pdf"]');
    await cdp.waitFor(
      "(() => { var t = document.querySelector('#review [role=\"tab\"][data-path=\"cl1-broken.pdf\"]'); var p = t && document.getElementById(t.getAttribute('aria-controls')); return !!p && !p.hidden && !!p.querySelector('.viewer-note[data-viewer=\"pdf\"]'); })()",
      10000,
      '[CL1] 開 cl1-broken.pdf：顯示「PDF 無法解析」'
    );
    await collect('PDF 檢視器（report.pdf、無法解析）');
    // file-review task 4.6：自動更新的過期標示（spec file-review「自動更新」：讀取失敗時保留最後一次的內容並標為
    // 過期）——暫存副本裡建 cl1-stale.md 與 cl1-stale.txt，各自開啟、畫出內容後刪掉，等自動更新把該分頁標為過期
    // （.file-panel.is-stale）。先標過期的 Markdown 分頁切走後仍留著標示（非目前分頁不查詢），兩個一起取樣。
    // 只寫 ui_preview 的暫存副本。
    fs.writeFileSync(path.join(cl1Repo, 'cl1-stale.md'), '# cl1-stale\n\nCL1：刪掉後標為過期的 Markdown。\n');
    fs.writeFileSync(path.join(cl1Repo, 'cl1-stale.txt'), 'CL1：刪掉後標為過期的純文字\n');
    await cdp.click('#files .files-refresh');
    await cdp.waitFor("!!document.querySelector('#files [role=\"treeitem\"][title=\"cl1-stale.md\"]') && !!document.querySelector('#files [role=\"treeitem\"][title=\"cl1-stale.txt\"]')", 5000, '[CL1] 重新整理後檔案樹列出 cl1-stale.md 與 cl1-stale.txt');
    for (const [file, viewer] of [['cl1-stale.md', 'markdown'], ['cl1-stale.txt', 'text']]) {
      const panelJs = `(() => { var t = document.querySelector('#review [role="tab"][data-path="${file}"]'); return t && document.getElementById(t.getAttribute('aria-controls')); })()`;
      await cdp.click(`#files [role="treeitem"][title="${file}"]`);
      await cdp.waitFor(`(() => { var p = ${panelJs}; return !!p && !p.hidden && !!p.querySelector('[data-viewer="${viewer}"]'); })()`, 5000, `[CL1] 開 ${file}：檔案分頁以 ${viewer} 檢視器顯示`);
      fs.rmSync(path.join(cl1Repo, file), { force: true });
      await cdp.waitFor(`(() => { var p = ${panelJs}; return !!p && p.classList.contains('is-stale'); })()`, 5000, `[CL1] 刪掉 ${file}：自動更新把分頁標為過期`);
    }
    await collect('檔案分頁過期（Markdown、純文字）');
    fs.writeFileSync(path.join(cl1Repo, 'cl1-gone.md'), '# cl1-gone\n');
    await cdp.click('#files .files-refresh');
    await cdp.waitFor("!!document.querySelector('#files [role=\"treeitem\"][title=\"cl1-gone.md\"]')", 5000, '[CL1] 重新整理後檔案樹列出 cl1-gone.md');
    fs.rmSync(path.join(cl1Repo, 'cl1-gone.md'), { force: true });
    await cdp.click('#files [role="treeitem"][title="cl1-gone.md"]');
    await cdp.waitFor("(() => { var t = document.querySelector('#review [role=\"tab\"][data-path=\"cl1-gone.md\"]'); var p = t && document.getElementById(t.getAttribute('aria-controls')); var s = p && p.querySelector('[data-tone=\"warn\"]'); return !!s && !s.hidden; })()", 5000, '[CL1] 開已刪除的 cl1-gone.md：檔案分頁顯示讀取失敗原因');
    await collect('檔案分頁（讀取失敗）');

    // git-review task 4.2：左欄「變更」分頁（spec git-review「左欄變更分頁」新增的畫面狀態；spec
    // file-review「左欄檔案樹」MODIFIED 的第三個分頁）——分支資訊、已暫存／變更／未追蹤三組（task 3.1
    // fixture 的 review-repo 工作區既有狀態，合併衝突放在 other-repo，這裡不驗）、點列開 diff 分頁、
    // 「Git Graph」按鈕開 graph 分頁。只讀既有工作區狀態，不另外寫暫存副本。
    await cdp.click('#files-tab-changes');
    await cdp.waitFor("document.getElementById('files-tab-changes').getAttribute('aria-selected') === 'true'", 3000, '[CL1] 切到左欄「變更」分頁');
    await cdp.waitFor('!!document.querySelector(\'#changes-panel .changes-row[title="history/staged-change.txt"]\')', 5000, '[CL1] 「變更」面板列出 history/staged-change.txt（已暫存組）');
    await cdp.click('#changes-panel .changes-row[title="history/unstaged-change.txt"]');
    await cdp.waitFor(
      '(() => { var t = document.querySelector(\'#review [role="tab"][data-diff-path="history/unstaged-change.txt"]\'); return !!t && t.getAttribute(\'aria-selected\') === \'true\'; })()',
      5000,
      '[CL1] 點「變更」列開啟並選定 diff 分頁'
    );
    await collect('左欄變更分頁（清單、diff 分頁）');

    // git-review task 4.3：diff 分頁內容的其餘畫面狀態（spec git-review「diff 分頁」；design D7 新增
    // 的選擇器 `.diff-grid`／`.diff-num`／`.diff-text`／`.diff-gap`／`.diff-row-del`／
    // `.diff-row-add`／`.diff-row-blank`／`.diff-toolbar` 系列）——上面已經走過 change 列（左
    // `.diff-row-del`、右 `.diff-row-add`，非 blank）；這裡補兩種還沒出現過的列：deleted-in-
    // worktree.txt 的 delete 列（右側 `.diff-row-blank`）與暫存副本裡在 docs/design.md 中間插入
    // 一行造成的 add 列（左側 `.diff-row-blank`）＋`.diff-gap`（前後都留有足夠未變更的行）。
    await cdp.click('#files-tab-changes');
    await cdp.waitFor("document.getElementById('files-tab-changes').getAttribute('aria-selected') === 'true'", 3000, '[CL1] 切回左欄「變更」分頁（準備開 deleted-in-worktree.txt 的 diff）');
    await cdp.click('#changes-panel .changes-row[title="history/deleted-in-worktree.txt"]');
    await cdp.waitFor(
      '(() => { var t = document.querySelector(\'#review [role="tab"][data-diff-path="history/deleted-in-worktree.txt"]\'); return !!t && t.getAttribute(\'aria-selected\') === \'true\'; })()',
      5000,
      '[CL1] 點「變更」列開啟 deleted-in-worktree.txt 的 diff 分頁'
    );
    await cdp.waitFor(
      '(() => { var t = document.querySelector(\'#review [role="tab"][data-diff-path="history/deleted-in-worktree.txt"]\'); var p = t && document.getElementById(t.getAttribute(\'aria-controls\')); return !!p && !!p.querySelector(\'.diff-row-blank\'); })()',
      5000,
      '[CL1] deleted-in-worktree.txt 的 diff 內容含右側空白（.diff-row-blank）'
    );
    await collect('diff 分頁（delete 列＋右側空白）');

    const cl1DesignLines = fs.readFileSync(path.join(cl1Repo, 'docs', 'design.md'), 'utf8').split('\n');
    cl1DesignLines.splice(Math.floor(cl1DesignLines.length / 2), 0, 'CL1：diff add 列與 gap 列（只在暫存副本插入，不動 repo 內的 fixture）');
    fs.writeFileSync(path.join(cl1Repo, 'docs', 'design.md'), cl1DesignLines.join('\n'));
    await cdp.click('#files-tab-changes');
    await cdp.waitFor("document.getElementById('files-tab-changes').getAttribute('aria-selected') === 'true'", 3000, '[CL1] 切回左欄「變更」分頁（準備開 docs/design.md 的 diff）');
    await cdp.waitFor('!!document.querySelector(\'#changes-panel .changes-row[title="docs/design.md"]\')', 5000, '[CL1] 「變更」面板列出 docs/design.md（輪詢抓到暫存副本的修改）');
    await cdp.click('#changes-panel .changes-row[title="docs/design.md"]');
    await cdp.waitFor(
      '(() => { var t = document.querySelector(\'#review [role="tab"][data-diff-path="docs/design.md"]\'); return !!t && t.getAttribute(\'aria-selected\') === \'true\'; })()',
      5000,
      '[CL1] 點「變更」列開啟 docs/design.md 的 diff 分頁'
    );
    await cdp.waitFor(
      '(() => { var t = document.querySelector(\'#review [role="tab"][data-diff-path="docs/design.md"]\'); var p = t && document.getElementById(t.getAttribute(\'aria-controls\')); return !!p && !!p.querySelector(\'.diff-gap\') && !!p.querySelector(\'.diff-row-add\'); })()',
      5000,
      '[CL1] docs/design.md 的 diff 內容含 gap 列（.diff-gap）與新增列（.diff-row-add）'
    );
    await collect('diff 分頁（add 列＋gap 列）');

    // git-review task 4.4：Git Graph 分頁內容（design D8／D9；spec「Git Graph 分頁」新增的選擇器
    // `.graph-row`／`.graph-svg`／`.graph-line`／`.graph-node-head-ring`／`.graph-message`／
    // `.graph-refs`／`.graph-ref-badge` 系列／`.graph-subject`／`.graph-author`／`.graph-time`／
    // `.graph-hash`／`.graph-load-more`／`.graph-search`／`.graph-filter` 系列）——不篩選開啟時
    // review-repo 的第一批 200 列就同時含三種 ref 標籤與 HEAD 節點外框（見 task 4.4 報告「開啟並
    // 分批載入」段的實測），這裡再走選取（`.is-selected`）、搜尋標示（`.is-search-hit`）、分支
    // 篩選 popover（`.graph-filter-popover`／`.graph-filter-group`／`.graph-filter-group-title`／
    // `.graph-filter-item`／`.graph-filter-actions`）三個還沒出現過的畫面狀態。git-review task 4.5：
    // 選取一個 commit 後自動展開詳情（`.commit-detail` 系列，見下方）。
    await cdp.click('#files-tab-changes');
    await cdp.waitFor("document.getElementById('files-tab-changes').getAttribute('aria-selected') === 'true'", 3000, '[CL1] 切回左欄「變更」分頁（準備點 Git Graph）');
    await cdp.click('#changes-panel [data-action="open-git-graph"]');
    await cdp.waitFor(
      '(() => { var t = document.querySelector(\'#review [role="tab"][data-graph-root]\'); return !!t && t.getAttribute(\'aria-selected\') === \'true\'; })()',
      5000,
      '[CL1] 按「Git Graph」開啟並選定 Git Graph 分頁'
    );
    const cl1GraphPanelId = await cdp.eval('document.getElementById(document.querySelector(\'#review [role="tab"][data-graph-root]\').getAttribute(\'aria-controls\')).id');
    await cdp.waitFor(
      `(() => { var p = document.getElementById(${JSON.stringify(cl1GraphPanelId)}); var kinds = new Set(Array.from(p.querySelectorAll('.graph-ref-badge')).map(function (b) { return b.getAttribute('data-kind'); })); return p.querySelectorAll('.graph-row').length >= 200 && kinds.has('branch') && kinds.has('remote') && kinds.has('tag') && p.querySelectorAll('.graph-node-head-ring').length > 0; })()`,
      5000,
      '[CL1] Git Graph 第一批載入完成，三種 ref 標籤與 HEAD 節點外框都已畫出'
    );
    await cdp.eval(`(() => { document.getElementById(${JSON.stringify(cl1GraphPanelId)}).querySelector('.graph-row').click(); return true; })()`);
    await cdp.waitFor(
      `!!document.getElementById(${JSON.stringify(cl1GraphPanelId)}).querySelector('.graph-row.is-selected')`,
      5000,
      '[CL1] 點第一列：選取樣式出現'
    );
    // git-review task 4.5：commit 詳情與比較（design 控制端裁決；spec「commit 詳情與比較」新增的
    // 選擇器 `.commit-detail`／`.commit-detail-row`／`.commit-detail-label`／`.commit-detail-hash`／
    // `.commit-detail-message`／`.commit-detail-actions`／`.commit-detail-files`／
    // `.commit-detail-file-row`）——點第一列（HEAD）選取後就會自動展開詳情，這裡等它出現。parent
    // 「不在已載入範圍」的純文字分支沒有專屬選擇器（git.js commitParentNode() 的既有裁決：這個狀態
    // 極難穩定重現——捲到最後一列本身就會觸發自動載入下一批，parent 反而變成已載入——改用行內樣式，
    // 不受 CL1 死規則限制）。
    await cdp.waitFor(
      `!!document.getElementById(${JSON.stringify(cl1GraphPanelId)}).querySelector('.commit-detail')`,
      5000,
      '[CL1] 點第一列後 commit 詳情自動展開'
    );
    await collect('左欄變更分頁（Git Graph 分頁，清單與選取，commit 詳情）');

    await cdp.eval(`(() => {
      var input = document.getElementById(${JSON.stringify(cl1GraphPanelId)}).querySelector('.graph-search-input');
      input.value = 'main';
      input.dispatchEvent(new Event('input', { bubbles: true }));
      return true;
    })()`);
    await cdp.waitFor(
      `document.getElementById(${JSON.stringify(cl1GraphPanelId)}).querySelector('.graph-search-count').textContent.indexOf('共') !== -1`,
      5000,
      '[CL1] 搜尋框輸入 main：顯示比對到的總筆數'
    );
    await cdp.eval(`(() => {
      var input = document.getElementById(${JSON.stringify(cl1GraphPanelId)}).querySelector('.graph-search-input');
      input.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
      return true;
    })()`);
    await cdp.waitFor(
      `!!document.getElementById(${JSON.stringify(cl1GraphPanelId)}).querySelector('.graph-row.is-search-hit')`,
      5000,
      '[CL1] 按 Enter 跳到第一筆：搜尋標示出現'
    );
    await collect('左欄變更分頁（Git Graph 分頁，搜尋標示）');

    await cdp.click(`#${cl1GraphPanelId} [data-action="graph-filter-toggle"]`);
    await cdp.waitFor(
      `(() => { var pop = document.getElementById(${JSON.stringify(cl1GraphPanelId)}).querySelector('.graph-filter-popover'); return !!pop && !pop.hidden; })()`,
      5000,
      '[CL1] 點「分支篩選」按鈕：popover 打開，三組（本地分支／遠端分支／tag）都畫出'
    );
    await collect('左欄變更分頁（Git Graph 分頁，分支篩選 popover）');
    await cdp.eval(`(() => { document.activeElement.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true })); return true; })()`);
    await cdp.waitFor(
      `(() => { var pop = document.getElementById(${JSON.stringify(cl1GraphPanelId)}).querySelector('.graph-filter-popover'); return !!pop && pop.hidden; })()`,
      5000,
      '[CL1] Esc 關閉分支篩選 popover'
    );
    await cdp.click('#files-tab-files');
    await cdp.waitFor("document.getElementById('files-tab-files').getAttribute('aria-selected') === 'true'", 3000, '[CL1] 切回左欄「檔案」分頁');

    await cdp.eval("window.liveOutput.select('win', 'wJ:p1'); true");
    await cdp.waitFor("(() => { var s = document.querySelector('#files .files-status[data-tone=\"warn\"]'); return !!s && !s.hidden; })()", 5000, '[CL1] 選 wJ:p1：根目錄查詢失敗原因出現');
    await collect('左欄檔案樹（根目錄查詢失敗）');
    await cdp.eval('window.liveOutput.clear(); true');
    await cdp.click('#files-tab-projects');
    await cdp.waitFor("document.getElementById('files-tab-projects').getAttribute('aria-selected') === 'true'", 3000, '[CL1] 切回左欄 Project 分頁');

    await cdp.click('[data-action="rebind"][data-project="cockpit"][data-workstream="be"]');
    await cdp.click('[data-action="fail"][data-project="cockpit"][data-task="be-1"]');
    await cdp.waitFor(
      "document.querySelectorAll('[data-region=\"banner\"] .action-banner').length === 2",
      3000,
      '[CL1] 改綁模式＋錯誤提示兩則都出現'
    );
    await collect('兩則提示＋改綁模式');
    await cdp.click('[data-action="error-dismiss"]');
    if (await cdp.eval("!!document.querySelector('[data-action=\"rebind-cancel\"]')")) {
      await cdp.click('[data-action="rebind-cancel"]');
    }
    await cdp.waitFor("document.querySelectorAll('[data-region=\"banner\"] .action-banner').length === 0", 3000, '[CL1] 兩則提示收掉');

    for (const channel of ['disconnected', 'connecting', 'cl1-unknown-channel']) {
      await cdp.eval(`window.onChannel(${JSON.stringify(channel)}); true`);
      await sleep(50);
      await collect(`通道 ${channel}`);
    }
    await cdp.eval("window.onChannel('connected'); true");

    const defensive = await injectState(cdp, craftCl1DefensiveState());
    check(defensive && defensive.ok === true, '[CL1] 注入防禦狀態（五種 agent 狀態＋未知、兩個 tab、connecting／未知連線、未知 task status）');
    await sleep(100);
    await collect('防禦狀態');
    // ui-fixes task 4.3：style.css 新增 `#app:not([data-channel-state="connected"]) …` 區塊（通道斷線的
    // 最後已知呈現），其中 .ff-undeclared、.agent-dot-blocked 只有在「通道非 connected＋畫面有未宣告提示／
    // blocked pane」同時成立時才對得到元素；上面的通道段用的是預設投影（兩者都沒有），防禦狀態又是
    // connected。補一個「防禦狀態＋通道 disconnected」的取樣，規則仍須對得到元素（死規則斷言不放寬）。
    await cdp.eval("window.onChannel('disconnected'); true");
    await sleep(50);
    await collect('防禦狀態＋通道 disconnected');
    await cdp.eval("window.onChannel('connected'); true");

    const noProjects = await injectState(cdp, craftNoProjectsState());
    check(noProjects && noProjects.ok === true, '[CL1] 注入沒有 Project 的投影');
    await sleep(100);
    await collect('沒有 Project');

    const back = await injectState(cdp, loadFixture());
    check(back && back.ok === true, '[CL1] 放回預設投影');
    await cdp.eval("window.liveOutput.select('win', 'wJ:p3'); true");
    await collect('預設投影（有選取）');
    // prefers-reduced-motion 的防護規則（task 2.2）只在使用者要求減少動態時生效。
    await cdp.send('Emulation.setEmulatedMedia', { features: [{ name: 'prefers-reduced-motion', value: 'reduce' }] });
    await collect('prefers-reduced-motion: reduce');
    await cdp.send('Emulation.setEmulatedMedia', { features: [] });

    const report = await cdp.eval(`(${CL1_REPORT_FN})('__cl1')`);
    check(!!report && report.total > 100, `[CL1] 累積的選擇器數量合理（實際 ${report && report.total} 條，走訪 ${report && report.states.length} 個狀態）`);
    check(!!report && report.errors.length === 0, `[CL1] 每條選擇器都能被 querySelector 解析（解析失敗：${JSON.stringify(report && report.errors)}）`);
    check(
      !!report && report.unmatched.length === 0,
      `[CL1] style.css 沒有死規則：每條選擇器至少在一種狀態下對得到元素（對不到 ${report && report.unmatched.length} 條：${JSON.stringify(report && report.unmatched.map((u) => (u.context ? u.context + ' ' : '') + u.selector))}）`
    );
    log(`CL1：走訪期間 DOM 出現過的 class（${report ? report.classes.length : 0} 個）：${JSON.stringify(report && report.classes)}`);

    // 否定對照：同一套判定對一份注入的 <style> 跑，必須只抓出沒人用的那條。
    await cdp.eval(`(() => {
      var s = document.createElement('style');
      s.id = '__cl1_neg';
      s.textContent = '.cl1-never-used-probe { color: inherit; } .pane-row:hover, .cl1-never-used-probe > .pane-row { color: inherit; } @media (min-width: 1px) { .pane-row::before { color: inherit; } } @media (max-width: 400px) { .pane-row { color: inherit; } } @supports (display: cl1-bogus) { .pane-row { color: inherit; } }';
      document.head.appendChild(s);
      return true;
    })()`);
    await collectIn('否定對照', '__cl1neg', '__cl1_neg');
    const neg = await cdp.eval(`(${CL1_REPORT_FN})('__cl1neg')`);
    await cdp.eval("(() => { var s = document.getElementById('__cl1_neg'); if (s) s.remove(); return true; })()");
    const negUnmatched = neg ? neg.unmatched.map((u) => (u.context ? u.context + ' ' : '') + u.selector).sort() : null;
    // fix round 1（Codex medium）：.pane-row 在四種視窗都存在，但放在「任何取樣視窗都不成立」的
    // @media (max-width: 400px) 與不支援的 @supports 裡，規則永遠不套用，必須被判定為死規則；
    // 同一個選擇器放在永遠成立的 @media (min-width: 1px) 裡則不算。
    const negExpected = ['.cl1-never-used-probe', '.cl1-never-used-probe > .pane-row', '@(display: cl1-bogus) .pane-row', '@(max-width: 400px) .pane-row'];
    check(
      !!neg && neg.total === 6 && JSON.stringify(negUnmatched) === JSON.stringify(negExpected),
      `[CL1] 否定對照：抓出兩條沒人用的選擇器，以及元素存在但外層 @media／@supports 在每個取樣視窗都不成立的兩條；:hover／::before 拿掉後對得到、且在成立的 @media 裡的 .pane-row 不算（實際 total=${neg && neg.total}，unmatched=${JSON.stringify(negUnmatched)}）`
    );
  } finally {
    await stopChrome(chrome, 'chrome-CL1');
    await stopPreview(preview, 'preview-CL1');
  }
}

// ---------------------------------------------------------------------------
// DF1：前面 task 帶到 5.1 的延後項目（direction-01-visual task 5.1 brief「必做 1–7」與「看情況 8」）。
// 子斷言以 [DF1/xxx] 區分，每一組都附否定對照（在頁面上暫時造出違規，證明偵測器抓得到）：
//   [DF1/topbar]（R28）：寬 ≥1200 不論高度，頂列一律固定 token 高度、不換行、燈號列在頂列內橫向
//     捲動——1200×719（整頁捲動的寬矮版面）下預設投影、文字放大 200%、五個 60 字元 id＋通道
//     disconnected、兩者疊加四種情境，都驗「實際頂列高＝--shell-topbar-h（Factory Floor 高度上限
//     扣除用的值）」、產品名與燈號在頂列框內、Factory Floor ≤ 上限、Live Output 開頭在第一屏。
//   [DF1/statusbar]：底列高度由字級推導（行高 × --fs-meta＋通道徽章上下內距＋底列上下內距＋上框
//     線），--shell-statusbar-h 用同一個算式；三種版面 × 預設／文字放大 200% 都驗實際高＝token、
//     內容不被裁、固定一屏時底列在視窗內。
//   [DF1/conn]：連線符號（.conn-symbol）隨字級等比放大——頂列燈號、底列通道、右欄 runtime 卡三處，
//     文字放大 200% 時寬高都是原本的兩倍；三種形狀（實心圓／空心圓／叉）放大後仍可分。
//   [DF1/scrollbar]（3.2 設計 M3、R32）：scrollbar-width 不繼承——四種版面下，頁面裡每一個
//     overflow 為 auto／scroll 的內層捲動容器 computed scrollbar-width 都是 thin，且點名的六個
//     容器（render.js SCROLL_KEEP_SELECTORS 的四個、頂列燈號列、Live Output 內容框）都有被量到。
//   [DF1/pretty]（3.3 N1）：連線明細值欄 text-wrap-style: pretty。
//   [DF1/cwd]（3.3 N2）：cwd 沒有前段（沒有路徑分隔符）時不產生空的前段 span，列高與第二行基線
//     跟有前段的列一致。
//   [DF1/focus]（4.1 N2）：取消選取後焦點退到 #output（tabindex=-1）時，內縮焦點框在兩個切角不斷開
//     ——截圖（clip.scale=4）後在切角 16×16 CSS px 範圍內，從上緣（下緣）焦點框走到左緣（右緣）
//     焦點框，路徑上的像素都要接近冰青（8 連通）。
//   [DF1/pin]（4.2 觀察）：寬 ≥760 時，截斷提示或過期原因行出現讓內容框變矮，原本貼底的內容仍然
//     貼底（最後一行不被切半）；使用者往上捲時不被拉回。
// ---------------------------------------------------------------------------

// PNG 解碼（只為 [DF1/focus] 讀截圖像素；Node 內建 zlib，不加套件）：支援 Chrome 截圖會產生的
// 8-bit、非交錯、RGB（color type 2）或 RGBA（color type 6）。
function decodePng(buf) {
  const zlib = require('node:zlib');
  let pos = 8;
  let width = 0;
  let height = 0;
  let colorType = 0;
  const idat = [];
  while (pos < buf.length) {
    const len = buf.readUInt32BE(pos);
    const type = buf.toString('ascii', pos + 4, pos + 8);
    const data = buf.subarray(pos + 8, pos + 8 + len);
    if (type === 'IHDR') {
      width = data.readUInt32BE(0);
      height = data.readUInt32BE(4);
      if (data[8] !== 8 || data[12] !== 0) throw new Error(`decodePng：只支援 8-bit 非交錯（bitDepth=${data[8]}，interlace=${data[12]}）`);
      colorType = data[9];
    } else if (type === 'IDAT') {
      idat.push(data);
    } else if (type === 'IEND') {
      break;
    }
    pos += 12 + len;
  }
  const bpp = colorType === 6 ? 4 : colorType === 2 ? 3 : 0;
  if (!bpp) throw new Error(`decodePng：不支援的 color type ${colorType}`);
  const raw = zlib.inflateSync(Buffer.concat(idat));
  const stride = width * bpp;
  const out = Buffer.alloc(width * height * 4);
  let prev = Buffer.alloc(stride);
  for (let y = 0; y < height; y += 1) {
    const filter = raw[y * (stride + 1)];
    const line = Buffer.from(raw.subarray(y * (stride + 1) + 1, (y + 1) * (stride + 1)));
    for (let i = 0; i < stride; i += 1) {
      const a = i >= bpp ? line[i - bpp] : 0;
      const b = prev[i];
      const c = i >= bpp ? prev[i - bpp] : 0;
      let v = line[i];
      if (filter === 1) v += a;
      else if (filter === 2) v += b;
      else if (filter === 3) v += Math.floor((a + b) / 2);
      else if (filter === 4) {
        const p = a + b - c;
        const pa = Math.abs(p - a);
        const pb = Math.abs(p - b);
        const pc = Math.abs(p - c);
        v += pa <= pb && pa <= pc ? a : pb <= pc ? b : c;
      }
      line[i] = v & 0xff;
    }
    for (let x = 0; x < width; x += 1) {
      out[(y * width + x) * 4] = line[x * bpp];
      out[(y * width + x) * 4 + 1] = line[x * bpp + 1];
      out[(y * width + x) * 4 + 2] = line[x * bpp + 2];
      out[(y * width + x) * 4 + 3] = bpp === 4 ? line[x * bpp + 3] : 255;
    }
    prev = line;
  }
  return { width, height, data: out };
}

// 切角焦點框連通判定：img 是切角附近的截圖（已翻轉成「左上角是切角」的方向），ring 像素＝在
// 「背景→冰青」這條色彩線段上投影 ≥0.4（--line 本身只有 0.27，半覆蓋的反鋸齒像素約 0.5）。
// 起點＝最右一欄、上方 band 像素高度內的冰青像素（上緣焦點框），終點＝最下一列、左方 band 內的
// 冰青像素（左緣焦點框）；8 連通 BFS 走得到才算「不斷開」。
function cornerRingConnected(img, flip, band) {
  const accent = [99, 213, 232];
  const bg = [9, 19, 32];
  const d = accent.map((v, i) => v - bg[i]);
  const dd = d.reduce((s, v) => s + v * v, 0);
  const W = img.width;
  const H = img.height;
  const at = (x, y) => {
    const sx = flip ? W - 1 - x : x;
    const sy = flip ? H - 1 - y : y;
    const k = (sy * W + sx) * 4;
    const t = ((img.data[k] - bg[0]) * d[0] + (img.data[k + 1] - bg[1]) * d[1] + (img.data[k + 2] - bg[2]) * d[2]) / dd;
    return t >= 0.4;
  };
  const starts = [];
  for (let y = 0; y < band; y += 1) if (at(W - 1, y)) starts.push([W - 1, y]);
  let targets = 0;
  for (let x = 0; x < band; x += 1) if (at(x, H - 1)) targets += 1;
  const seen = new Uint8Array(W * H);
  const queue = starts.slice();
  for (const [x, y] of starts) seen[y * W + x] = 1;
  let reached = false;
  while (queue.length && !reached) {
    const [x, y] = queue.shift();
    if (y === H - 1 && x < band) reached = true;
    for (let dy = -1; dy <= 1; dy += 1) {
      for (let dx = -1; dx <= 1; dx += 1) {
        const nx = x + dx;
        const ny = y + dy;
        if (nx < 0 || ny < 0 || nx >= W || ny >= H || seen[ny * W + nx]) continue;
        if (!at(nx, ny)) continue;
        seen[ny * W + nx] = 1;
        queue.push([nx, ny]);
      }
    }
  }
  return { starts: starts.length, targets, reached };
}

async function partDeferredItems() {
  log('=== DF1. 前面 task 帶到 5.1 的延後項目（direction-01-visual task 5.1）===');
  let preview = null;
  let chrome = null;
  try {
    preview = await startPreview({}, 'preview-DF1');
    const url = `http://127.0.0.1:${preview.port}/`;
    chrome = await startChrome(pickPort(19150, [preview.port]), url, 'chrome-DF1', '1536,1024');
    const { cdp } = chrome;
    await waitForFirstProjection(cdp, preview.port);

    async function setViewport(width, height, dsf = 1) {
      await cdp.send('Emulation.setDeviceMetricsOverride', { width, height, deviceScaleFactor: dsf, mobile: false });
      await sleep(150);
      await cdp.eval('window.scrollTo(0, 0); true');
      const actual = await cdp.eval('({ w: window.innerWidth, h: window.innerHeight })');
      check(actual.w === width && actual.h === height, `[DF1] 視窗精準設成 ${width}x${height}（實際 ${JSON.stringify(actual)}）`);
    }
    async function injectStyle(id, cssText) {
      await cdp.eval(`(() => { var s = document.getElementById(${JSON.stringify(id)}); if (!s) { s = document.createElement('style'); s.id = ${JSON.stringify(id)}; document.head.appendChild(s); } s.textContent = ${JSON.stringify(cssText)}; return true; })()`);
      await sleep(100);
    }
    async function removeStyle(id) {
      await cdp.eval(`(() => { var s = document.getElementById(${JSON.stringify(id)}); if (s) s.remove(); return true; })()`);
      await sleep(100);
    }
    // 文字放大 200%（同 CH1 (f)）：根字級 200%＋四個 --fs-* token 加倍（token 是 px，只改根字級
    // 不會放大任何字；瀏覽器「只放大文字」會直接放大 px 字級，這裡用 token 加倍模擬）。
    const baseFs = await cdp.eval(`(() => {
      var cs = getComputedStyle(document.documentElement);
      return ['--fs-title', '--fs-panel', '--fs-dense', '--fs-meta'].map(function (n) { return [n, parseFloat(cs.getPropertyValue(n))]; });
    })()`);
    const textScaleCss =
      'html { font-size: 200%; } :root { ' + baseFs.map(([n, px]) => `${n}: ${px * 2}px;`).join(' ') + ' }';

    // --- [DF1/topbar]（R28）---
    const TOPBAR_FIT_JS = `(() => {
      var shell = document.querySelector('.shell');
      var tb = document.querySelector('[data-region="topbar"]');
      var r = tb.getBoundingClientRect();
      var inside = function (el) { var b = el.getBoundingClientRect(); return b.top >= r.top - 0.5 && b.bottom <= r.bottom + 0.5; };
      var lamps = Array.prototype.slice.call(tb.querySelectorAll('.runtime-lamp[data-runtime]'));
      var out = document.querySelector('[data-region="output"]').getBoundingClientRect();
      var floor = document.querySelector('[data-region="floor"]').getBoundingClientRect();
      // fix round 1（Codex medium）：燈號列真的能橫向捲、捲到兩端時首尾燈號完整進入可視範圍
      // （overflow-x: hidden 會無聲裁切，只看垂直邊界抓不到）。
      var strip = tb.querySelector('.topbar-runtimes');
      var sr = strip.getBoundingClientRect();
      var portL = sr.left + strip.clientLeft;
      var portR = portL + strip.clientWidth;
      var fullyIn = function (el) { var b = el.getBoundingClientRect(); return b.left >= portL - 0.5 && b.right <= portR + 0.5; };
      var origLeft = strip.scrollLeft;
      strip.scrollLeft = 0;
      var firstIn = lamps.length > 0 && fullyIn(lamps[0]);
      strip.scrollLeft = strip.scrollWidth;
      var lastIn = lamps.length > 0 && fullyIn(lamps[lamps.length - 1]);
      strip.scrollLeft = origLeft;
      return {
        stripOverflowX: getComputedStyle(strip).overflowX,
        stripScrollWidth: strip.scrollWidth,
        stripClientWidth: strip.clientWidth,
        firstLampIn: firstIn,
        lastLampIn: lastIn,
        tokenPx: parseFloat(getComputedStyle(shell).getPropertyValue('--shell-topbar-h')),
        topbarH: r.height,
        appInside: inside(tb.querySelector('.app-name')),
        lampsInside: lamps.length > 0 && lamps.every(inside),
        lampCount: lamps.length,
        appFontPx: parseFloat(getComputedStyle(tb.querySelector('.app-name')).fontSize),
        outputTop: out.top,
        floorH: floor.height,
        innerHeight: window.innerHeight,
        shellOverflowY: getComputedStyle(shell).overflowY,
      };
    })()`;
    function topbarViolations(m) {
      const v = [];
      if (Math.abs(m.topbarH - m.tokenPx) >= 0.5) v.push(`頂列實際高 ${m.topbarH}px ≠ --shell-topbar-h ${m.tokenPx}px`);
      if (!m.appInside) v.push('產品名不在頂列框內');
      if (!m.lampsInside) v.push('燈號不在頂列框內');
      const floorCap = Math.max(240, m.innerHeight - m.tokenPx - 200);
      if (m.floorH > floorCap + 0.5) v.push(`Factory Floor 高 ${m.floorH}px 超過上限 ${floorCap}px`);
      if (!(m.outputTop < m.innerHeight)) v.push(`Live Output 開頭 ${m.outputTop}px 不在第一屏（innerHeight ${m.innerHeight}）`);
      return v;
    }
    // 長 id 壓力情境才一定溢出：燈號列 overflow-x 必須是 auto／scroll、真的有橫向溢出，捲到 0 時
    // 第一顆、捲到最右時最後一顆完整在可視範圍內。
    function topbarScrollViolations(m) {
      const v = [];
      if (!/^(auto|scroll)$/.test(m.stripOverflowX)) v.push(`燈號列 overflow-x 是 ${m.stripOverflowX}（應為 auto／scroll）`);
      if (!(m.stripScrollWidth > m.stripClientWidth + 0.5)) v.push(`燈號列沒有橫向溢出（scrollWidth ${m.stripScrollWidth}，clientWidth ${m.stripClientWidth}）`);
      if (!m.firstLampIn) v.push('捲到最左時第一顆燈號不完整在可視範圍內');
      if (!m.lastLampIn) v.push('捲到最右時最後一顆燈號不完整在可視範圍內');
      return v;
    }
    await setViewport(1200, 719);
    const topbarContexts = [
      { tag: '預設投影', state: loadFixture, channel: 'connected', scale: false },
      { tag: '預設投影＋文字放大 200%', state: loadFixture, channel: 'connected', scale: true },
      { tag: '五個 60 字元 id＋通道 disconnected', state: craftManyLongRuntimesState, channel: 'disconnected', scale: false },
      { tag: '五個 60 字元 id＋通道 disconnected＋文字放大 200%', state: craftManyLongRuntimesState, channel: 'disconnected', scale: true },
    ];
    for (const c of topbarContexts) {
      const r = await injectState(cdp, c.state());
      await cdp.eval(`window.onChannel(${JSON.stringify(c.channel)}); true`);
      if (c.scale) await injectStyle('__df1_scale', textScaleCss);
      await cdp.eval('window.scrollTo(0, 0); true');
      await sleep(100);
      const m = await cdp.eval(TOPBAR_FIT_JS);
      const v = topbarViolations(m);
      check(
        r && r.ok === true && m.shellOverflowY !== 'hidden' && m.lampCount > 0,
        `[DF1/topbar] 1200×719／${c.tag}：前提——整頁捲動的寬矮版面、有燈號（.shell overflow-y=${m.shellOverflowY}，燈號 ${m.lampCount} 顆，產品名 ${m.appFontPx}px）`
      );
      check(
        v.length === 0,
        `[DF1/topbar] 1200×719／${c.tag}：頂列高＝token、產品名與燈號在框內、Factory Floor ≤ 上限、Live Output 開頭在第一屏（token=${m.tokenPx}，頂列=${m.topbarH}，floor=${m.floorH}，output.top=${m.outputTop}；違反：${JSON.stringify(v)}）`
      );
      if (c.state === craftManyLongRuntimesState) {
        const hv = topbarScrollViolations(m);
        check(
          hv.length === 0,
          `[DF1/topbar] 1200×719／${c.tag}：燈號列可橫向捲動，捲到兩端首尾燈號都完整可見（overflow-x=${m.stripOverflowX}，scrollWidth=${m.stripScrollWidth}，clientWidth=${m.stripClientWidth}；違反：${JSON.stringify(hv)}）`
        );
        await injectStyle('__df1_neg', '.topbar-runtimes { overflow-x: hidden !important; }');
        const hm = await cdp.eval(TOPBAR_FIT_JS);
        const hnv = topbarScrollViolations(hm);
        check(hnv.length > 0, `[DF1/topbar] 否定對照（${c.tag}）：只把燈號列改成 overflow-x: hidden 必須判定失敗（實際違反 ${JSON.stringify(hnv)}）`);
        await removeStyle('__df1_neg');
      }
      if (c.tag === '五個 60 字元 id＋通道 disconnected') {
        await injectStyle(
          '__df1_neg',
          '[data-region="topbar"] { height: auto !important; padding-top: 12px !important; padding-bottom: 12px !important; } .topbar-runtimes { flex-wrap: wrap !important; overflow: visible !important; } .runtime-lamp { flex-wrap: wrap !important; }'
        );
        const nm = await cdp.eval(TOPBAR_FIT_JS);
        const nv = topbarViolations(nm);
        check(nv.length > 0, `[DF1/topbar] 否定對照：頂列改回可換行、自然高度時必須判定失敗（實際違反 ${JSON.stringify(nv)}）`);
        await removeStyle('__df1_neg');
      }
      if (c.scale) await removeStyle('__df1_scale');
    }
    await cdp.eval("window.onChannel('connected'); true");
    const fixtureBack = await injectState(cdp, loadFixture());
    check(fixtureBack && fixtureBack.ok === true, '[DF1] 放回預設投影');

    // --- [DF1/statusbar] ---
    const STATUSBAR_JS = `(() => {
      var shell = document.querySelector('.shell');
      var sb = document.querySelector('[data-region="statusbar"]');
      var cs = getComputedStyle(sb);
      var r = sb.getBoundingClientRect();
      var badge = sb.querySelector('#channel-status');
      var bcs = getComputedStyle(badge);
      var parts = ['.statusbar-channel', '#channel-status', '.channel-status-text', '.statusbar-channel-label', '#version'].map(function (sel) {
        var n = sb.querySelector(sel);
        var b = n.getBoundingClientRect();
        return { sel: sel, top: b.top, bottom: b.bottom };
      });
      var innerTop = r.top + parseFloat(cs.borderTopWidth) + parseFloat(cs.paddingTop) - 0.5;
      var innerBottom = r.bottom - parseFloat(cs.borderBottomWidth) - parseFloat(cs.paddingBottom) + 0.5;
      return {
        tokenPx: parseFloat(getComputedStyle(shell).getPropertyValue('--shell-statusbar-h')),
        height: r.height,
        bottom: r.bottom,
        innerHeight: window.innerHeight,
        lineHeightPx: parseFloat(cs.lineHeight),
        fontPx: parseFloat(cs.fontSize),
        derivedPx: parseFloat(cs.lineHeight) + parseFloat(bcs.paddingTop) + parseFloat(bcs.paddingBottom) + parseFloat(cs.paddingTop) + parseFloat(cs.paddingBottom) + parseFloat(cs.borderTopWidth) + parseFloat(cs.borderBottomWidth),
        inside: parts.every(function (p) { return p.top >= innerTop && p.bottom <= innerBottom; }),
        parts: parts,
        clipped: sb.scrollHeight > sb.clientHeight + 0.5,
        fixed: getComputedStyle(shell).overflowY === 'hidden',
      };
    })()`;
    function statusbarViolations(m) {
      const v = [];
      if (!(m.lineHeightPx > 0)) v.push(`底列行高沒有明寫（computed line-height 不是數值）`);
      if (Math.abs(m.height - m.tokenPx) >= 0.5) v.push(`底列實際高 ${m.height}px ≠ --shell-statusbar-h ${m.tokenPx}px`);
      if (!(Math.abs(m.tokenPx - m.derivedPx) < 0.5)) v.push(`token ${m.tokenPx}px ≠ 行高＋徽章上下內距＋底列上下內距＋框線 ${m.derivedPx}px`);
      if (!m.inside) v.push(`底列內容超出內容框：${JSON.stringify(m.parts)}`);
      if (m.clipped) v.push('底列內容被裁（scrollHeight > clientHeight）');
      if (m.fixed && m.bottom > m.innerHeight + 0.5) v.push(`固定一屏時底列下緣 ${m.bottom} 超出視窗 ${m.innerHeight}`);
      return v;
    }
    const statusbarBase = {};
    for (const [w, h] of [[1536, 1024], [1280, 650], [700, 900]]) {
      await setViewport(w, h);
      for (const scale of [false, true]) {
        if (scale) await injectStyle('__df1_scale', textScaleCss);
        const m = await cdp.eval(STATUSBAR_JS);
        const v = statusbarViolations(m);
        const tag = `${w}×${h}${scale ? '／文字放大 200%' : ''}`;
        if (!scale) statusbarBase[`${w}x${h}`] = m.tokenPx;
        check(
          v.length === 0,
          `[DF1/statusbar] ${tag}：底列高由字級推導、實際高＝token、內容不被裁（token=${m.tokenPx}，實際=${m.height}，推導=${m.derivedPx}，字級=${m.fontPx}；違反：${JSON.stringify(v)}）`
        );
        if (scale) {
          const base = statusbarBase[`${w}x${h}`];
          check(m.tokenPx > base + 10, `[DF1/statusbar] ${tag}：token 隨字級變大（預設 ${base}px，放大後 ${m.tokenPx}px）`);
          if (w === 1536) {
            await injectStyle('__df1_neg', '[data-region="statusbar"] { height: 32px !important; min-height: 0 !important; }');
            const nm = await cdp.eval(STATUSBAR_JS);
            const nv = statusbarViolations(nm);
            check(nv.length > 0, `[DF1/statusbar] 否定對照：文字放大時底列寫死 32px 必須判定失敗（實際違反 ${JSON.stringify(nv)}）`);
            await removeStyle('__df1_neg');
          }
          await removeStyle('__df1_scale');
        }
      }
    }

    // --- [DF1/conn]：連線符號隨字級等比放大，三種形狀仍可分 ---
    await setViewport(1536, 1024);
    const connState = loadFixture();
    connState.version = 5102;
    const lab = JSON.parse(JSON.stringify(connState.runtimes.find((r) => r.id === 'wsl')));
    lab.id = 'lab';
    lab.connection = { state: 'connecting' };
    connState.runtimes.push(lab);
    const connInject = await injectState(cdp, connState);
    check(connInject && connInject.ok === true, '[DF1/conn] 注入 connected／disconnected／connecting 三個 runtime');
    const CONN_JS = `(() => {
      function shape(el) {
        var cs = getComputedStyle(el);
        var b = el.getBoundingClientRect();
        var kind = cs.backgroundImage.indexOf('gradient') !== -1 ? 'cross'
          : parseFloat(cs.borderTopWidth) > 0 && cs.backgroundColor === 'rgba(0, 0, 0, 0)' && parseFloat(cs.borderTopLeftRadius) > 0 ? 'ring'
          : parseFloat(cs.borderTopWidth) === 0 && cs.backgroundColor !== 'rgba(0, 0, 0, 0)' && parseFloat(cs.borderTopLeftRadius) > 0 ? 'disc'
          : 'other';
        return { w: b.width, h: b.height, kind: kind, border: parseFloat(cs.borderTopWidth), fontPx: parseFloat(cs.fontSize) };
      }
      var out = {};
      ['connected', 'connecting', 'disconnected'].forEach(function (st) {
        var lamp = document.querySelector('[data-region="topbar"] .runtime-lamp-' + st + ' .conn-symbol');
        var card = document.querySelector('[data-region="runtimes"] .runtime-conn-' + st + ' .conn-symbol');
        out['topbar-' + st] = lamp ? shape(lamp) : null;
        out['runtime-' + st] = card ? shape(card) : null;
      });
      var ch = document.querySelector('#channel-status .conn-symbol');
      out['statusbar-' + document.getElementById('channel-status').className.replace(/.*channel-/, '')] = ch ? shape(ch) : null;
      return out;
    })()`;
    const EXPECTED_KIND = { connected: 'disc', connecting: 'ring', disconnected: 'cross' };
    async function measureConn() {
      const all = {};
      for (const ch of ['connected', 'connecting', 'disconnected']) {
        await cdp.eval(`window.onChannel(${JSON.stringify(ch)}); true`);
        await sleep(50);
        Object.assign(all, await cdp.eval(CONN_JS));
      }
      await cdp.eval("window.onChannel('connected'); true");
      return all;
    }
    const connBase = await measureConn();
    await injectStyle('__df1_scale', textScaleCss);
    const connScaled = await measureConn();
    const connProblems = [];
    for (const key of Object.keys(connBase)) {
      const b = connBase[key];
      const s = connScaled[key];
      if (!b || !s) {
        connProblems.push(`${key} 找不到符號`);
        continue;
      }
      const state = key.replace(/^[a-z]+-/, '');
      if (Math.abs(b.w - (b.fontPx * 8) / 12) > 0.25 || Math.abs(b.h - b.w) > 0.25) connProblems.push(`${key} 預設尺寸應為 8/12em（${(b.fontPx * 8) / 12}px）的正方形（實際 ${b.w}×${b.h}）`);
      if (Math.abs(s.w - 2 * b.w) > 0.5 || Math.abs(s.h - 2 * b.h) > 0.5) connProblems.push(`${key} 放大後應為兩倍（預設 ${b.w}×${b.h}，放大 ${s.w}×${s.h}）`);
      if (b.kind !== EXPECTED_KIND[state] || s.kind !== EXPECTED_KIND[state]) connProblems.push(`${key} 形狀應為 ${EXPECTED_KIND[state]}（預設 ${b.kind}，放大 ${s.kind}）`);
      if (state === 'connecting' && !(s.border > b.border)) connProblems.push(`${key} 空心圓的環寬應隨字級變粗（預設 ${b.border}，放大 ${s.border}）`);
    }
    check(
      Object.keys(connBase).length === 9 && connProblems.length === 0,
      `[DF1/conn] 頂列／右欄／底列三處 × 三種連線狀態的符號隨字級等比放大、形狀仍可分（量到 ${Object.keys(connBase).length} 組；問題：${JSON.stringify(connProblems)}）`
    );
    await injectStyle('__df1_neg', '.conn-symbol { width: 8px !important; height: 8px !important; }');
    const connNeg = await measureConn();
    const negFixed = Object.keys(connNeg).filter((k) => connNeg[k] && Math.abs(connNeg[k].w - 2 * connBase[k].w) > 0.5);
    check(negFixed.length === 9, `[DF1/conn] 否定對照：符號寫死 8px 時放大 200% 必須被判定不是兩倍（抓到 ${negFixed.length}／9）`);
    await removeStyle('__df1_neg');
    await removeStyle('__df1_scale');
    const connBack = await injectState(cdp, loadFixture());
    check(connBack && connBack.ok === true, '[DF1] 放回預設投影');

    // --- [DF1/scrollbar]：每個內層捲動容器都明設 scrollbar-width: thin ---
    await cdp.eval("window.liveOutput.select('win', 'wJ:p3'); true");
    const SCROLLERS_JS = `(() => {
      var named = {
        '[data-region="floor"] > .projects': 0,
        '[data-region="runtimes"] > .runtime-cards': 0,
        '[data-region="events"] > .recent-events-list': 0,
        '[data-region="projects"]': 0,
        '.topbar-runtimes': 0,
        '#output .output-text': 0,
      };
      var bad = [];
      var count = 0;
      Array.prototype.forEach.call(document.querySelectorAll('body *'), function (el) {
        var cs = getComputedStyle(el);
        var scrolls = /^(auto|scroll)$/.test(cs.overflowX) || /^(auto|scroll)$/.test(cs.overflowY);
        if (!scrolls || cs.display === 'none') return;
        count += 1;
        var name = el.id ? '#' + el.id : (el.getAttribute('data-region') ? '[data-region=' + el.getAttribute('data-region') + ']' : '.' + Array.prototype.join.call(el.classList, '.'));
        if (cs.scrollbarWidth !== 'thin') bad.push(name + '：' + cs.scrollbarWidth);
        Object.keys(named).forEach(function (sel) { if (el.matches(sel)) named[sel] += 1; });
      });
      return { count: count, bad: bad, named: named };
    })()`;
    const namedSeen = {};
    for (const [w, h] of [[1536, 1024], [1280, 650], [1100, 900], [700, 900]]) {
      await setViewport(w, h);
      const m = await cdp.eval(SCROLLERS_JS);
      for (const [sel, n] of Object.entries(m.named)) namedSeen[sel] = (namedSeen[sel] || 0) + n;
      check(
        m.count > 0 && m.bad.length === 0,
        `[DF1/scrollbar] ${w}×${h}：每個 overflow 為 auto／scroll 的內層捲動容器 scrollbar-width 都是 thin（量到 ${m.count} 個；不是 thin：${JSON.stringify(m.bad)}）`
      );
      if (w === 1536) {
        await injectStyle('__df1_neg', '[data-region="runtimes"] > .runtime-cards { scrollbar-width: auto !important; }');
        const nm = await cdp.eval(SCROLLERS_JS);
        check(nm.bad.length === 1, `[DF1/scrollbar] 否定對照：單獨把 .runtime-cards 改回 auto 必須恰好抓到一個（實際 ${JSON.stringify(nm.bad)}）`);
        await removeStyle('__df1_neg');
      }
    }
    const missingNamed = Object.keys(namedSeen).filter((k) => namedSeen[k] === 0);
    check(missingNamed.length === 0, `[DF1/scrollbar] 點名的六個內層捲動容器都有被量到（沒量到：${JSON.stringify(missingNamed)}；次數 ${JSON.stringify(namedSeen)}）`);
    await cdp.eval('window.liveOutput.clear(); true');

    // --- [DF1/pretty]：會折行的中文長句（連線明細值欄；fix round 1／設計 M1 加最近事件 detail、
    // 專案警告、Live Output 失敗原因）text-wrap-style: pretty ---
    await setViewport(1536, 1024);
    const PRETTY_TARGETS = {
      '連線明細值欄': '[data-region="runtimes"] .connection-details > dd',
      '最近事件 detail': '[data-region="events"] .event-detail',
      '專案警告': '[data-region="floor"] .project-warnings > li',
      'Live Output 失敗原因': '#output .output-error-reason',
    };
    const PRETTY_JS = `(() => {
      var targets = ${JSON.stringify(PRETTY_TARGETS)};
      var out = {};
      Object.keys(targets).forEach(function (k) {
        out[k] = Array.prototype.map.call(document.querySelectorAll(targets[k]), function (el) {
          return getComputedStyle(el).textWrapStyle || getComputedStyle(el).getPropertyValue('text-wrap-style');
        });
      });
      return out;
    })()`;
    const pretty = await cdp.eval(PRETTY_JS);
    for (const k of Object.keys(PRETTY_TARGETS)) {
      check(
        pretty[k].length > 0 && pretty[k].every((v) => v === 'pretty'),
        `[DF1/pretty] ${k}（${PRETTY_TARGETS[k]}）text-wrap-style 都是 pretty（實際 ${JSON.stringify(pretty[k])}）`
      );
    }
    await injectStyle('__df1_neg', Object.values(PRETTY_TARGETS).join(', ') + ' { text-wrap: wrap !important; }');
    const prettyNeg = await cdp.eval(PRETTY_JS);
    const prettyNegCaught = Object.keys(PRETTY_TARGETS).filter((k) => prettyNeg[k].some((v) => v !== 'pretty'));
    check(
      prettyNegCaught.length === Object.keys(PRETTY_TARGETS).length,
      `[DF1/pretty] 否定對照：四種節點改回 text-wrap: wrap 都必須被抓到（抓到 ${JSON.stringify(prettyNegCaught)}）`
    );
    await removeStyle('__df1_neg');

    // --- [DF1/radius]（fix round 1／設計 M2）：每個區塊都是直角——頂列原本是唯一的 6px 圓角。 ---
    const RADIUS_JS = `(() => Array.prototype.map.call(document.querySelectorAll('[data-region]'), function (el) {
      var cs = getComputedStyle(el);
      return { region: el.getAttribute('data-region'), radii: [cs.borderTopLeftRadius, cs.borderTopRightRadius, cs.borderBottomRightRadius, cs.borderBottomLeftRadius] };
    }).filter(function (r) { return r.radii.some(function (v) { return v !== '0px'; }); }))()`;
    const rounded = await cdp.eval(RADIUS_JS);
    check(rounded.length === 0, `[DF1/radius] 每個 data-region 區塊四角都是 0（有圓角的：${JSON.stringify(rounded)}）`);
    await injectStyle('__df1_neg', '[data-region="topbar"] { border-radius: 6px !important; }');
    const roundedNeg = await cdp.eval(RADIUS_JS);
    check(
      roundedNeg.length === 1 && roundedNeg[0].region === 'topbar',
      `[DF1/radius] 否定對照：頂列加回 6px 圓角必須恰好抓到頂列（實際 ${JSON.stringify(roundedNeg)}）`
    );
    await removeStyle('__df1_neg');

    // --- [DF1/cwd]：cwd 沒有前段時不產生空 span，列高與第二行基線跟有前段的列一致 ---
    const cwdState = loadFixture();
    cwdState.version = 5103;
    const cwdPanes = cwdState.runtimes.find((r) => r.id === 'win').workspaces[0].tabs[0].panes;
    cwdPanes.find((p) => p.id === 'wJ:p1').cwd = 'D:\\x\\ai-cockpit';
    cwdPanes.find((p) => p.id === 'wJ:p3').cwd = 'ai-cockpit';
    const cwdInject = await injectState(cdp, cwdState);
    check(cwdInject && cwdInject.ok === true, '[DF1/cwd] 注入一列有前段（D:\\x\\ai-cockpit）、一列沒有前段（ai-cockpit）的 cwd');
    const CWD_JS = `(() => {
      function info(id) {
        var row = document.querySelector('.pane-row[data-pane="' + id + '"]');
        var cwd = row.querySelector('.pane-cwd');
        var title = row.querySelector('.pane-title');
        var rr = row.getBoundingClientRect();
        var tail = cwd.querySelector('.pane-cwd-tail');
        return {
          rowH: rr.height,
          children: Array.prototype.map.call(cwd.children, function (c) { return { cls: c.className, text: c.textContent }; }),
          text: cwd.textContent,
          title: cwd.getAttribute('title'),
          titleBottom: title.getBoundingClientRect().bottom - rr.top,
          tailBottom: tail ? tail.getBoundingClientRect().bottom - rr.top : null,
        };
      }
      return { withHead: info('wJ:p1'), noHead: info('wJ:p3') };
    })()`;
    const cwd = await cdp.eval(CWD_JS);
    const emptySpans = cwd.noHead.children.filter((c) => c.text === '');
    check(
      emptySpans.length === 0 && cwd.noHead.children.every((c) => c.cls !== 'pane-cwd-head') && cwd.noHead.text === 'ai-cockpit' && cwd.noHead.title === 'ai-cockpit',
      `[DF1/cwd] 沒有前段的 cwd 不產生空 span（子節點 ${JSON.stringify(cwd.noHead.children)}，文字 ${JSON.stringify(cwd.noHead.text)}，title ${JSON.stringify(cwd.noHead.title)}）`
    );
    check(
      cwd.withHead.children.length === 2 && cwd.withHead.text === 'D:\\x\\ai-cockpit',
      `[DF1/cwd] 有前段的 cwd 仍拆成前段＋尾段（子節點 ${JSON.stringify(cwd.withHead.children)}）`
    );
    check(
      Math.abs(cwd.noHead.rowH - cwd.withHead.rowH) < 0.5 && cwd.noHead.tailBottom !== null && Math.abs(cwd.noHead.tailBottom - cwd.withHead.tailBottom) < 0.5,
      `[DF1/cwd] 兩列列高與 cwd 尾段底緣一致（有前段 ${cwd.withHead.rowH}／${cwd.withHead.tailBottom}，沒有前段 ${cwd.noHead.rowH}／${cwd.noHead.tailBottom}）`
    );
    // 否定對照：在沒有前段的那列補一個空的前段 span（等於修正前的 DOM），列高或尾段底緣必須不同。
    const cwdNeg = await cdp.eval(`(() => {
      var cwd = document.querySelector('.pane-row[data-pane="wJ:p3"] .pane-cwd');
      var head = document.createElement('span');
      head.className = 'pane-cwd-head';
      cwd.insertBefore(head, cwd.firstChild);
      var r = ${CWD_JS};
      head.remove();
      return r;
    })()`);
    check(
      Math.abs(cwdNeg.noHead.rowH - cwdNeg.withHead.rowH) >= 0.5 || Math.abs(cwdNeg.noHead.tailBottom - cwdNeg.withHead.tailBottom) >= 0.5,
      `[DF1/cwd] 否定對照：補回空的前段 span 時列高或尾段底緣必須不同（列高 ${cwdNeg.withHead.rowH}／${cwdNeg.noHead.rowH}，尾段底緣 ${cwdNeg.withHead.tailBottom}／${cwdNeg.noHead.tailBottom}）`
    );
    const cwdBack = await injectState(cdp, loadFixture());
    check(cwdBack && cwdBack.ok === true, '[DF1] 放回預設投影');

    // --- [DF1/focus]：#output 聚焦時，內縮焦點框在兩個切角不斷開 ---
    await setViewport(1536, 1024);
    await cdp.send('Emulation.setFocusEmulationEnabled', { enabled: true });
    await cdp.eval('window.liveOutput.clear(); true');
    await cdp.pressKey('Shift', 'ShiftLeft', 16);
    const focusInfo = await cdp.eval(`(() => {
      var o = document.getElementById('output');
      o.focus();
      var r = o.getBoundingClientRect();
      return { active: document.activeElement === o, visible: o.matches(':focus-visible'), left: r.left, top: r.top, right: r.right, bottom: r.bottom, outline: getComputedStyle(o).outlineColor + ' ' + getComputedStyle(o).outlineWidth + ' ' + getComputedStyle(o).outlineOffset };
    })()`);
    check(focusInfo.active && focusInfo.visible, `[DF1/focus] 前提：#output 取得焦點且符合 :focus-visible（${JSON.stringify(focusInfo)}）`);
    async function captureCorner(x, y) {
      const shot = await cdp.send('Page.captureScreenshot', { format: 'png', clip: { x, y, width: 16, height: 16, scale: 4 } });
      return decodePng(Buffer.from(shot.result.data, 'base64'));
    }
    async function cornersConnected() {
      const tl = cornerRingConnected(await captureCorner(focusInfo.left, focusInfo.top), false, 8);
      const br = cornerRingConnected(await captureCorner(focusInfo.right - 16, focusInfo.bottom - 16), true, 8);
      return { tl, br };
    }
    const corners = await cornersConnected();
    check(
      corners.tl.starts > 0 && corners.tl.targets > 0 && corners.br.starts > 0 && corners.br.targets > 0,
      `[DF1/focus] 前提：截圖裡上緣／左緣（下緣／右緣）焦點框都量得到冰青像素（${JSON.stringify(corners)}）`
    );
    check(
      corners.tl.reached && corners.br.reached,
      `[DF1/focus] 左上與右下切角處焦點框不斷開（冰青像素 8 連通；${JSON.stringify(corners)}）`
    );
    await injectStyle(
      '__df1_neg',
      '[data-region="output"]:focus-visible { background: linear-gradient(to bottom right, transparent 50%, var(--line) 50% calc(50% + 1px), transparent calc(50% + 1px)) top left / 10px 10px no-repeat border-box, linear-gradient(to top left, transparent 50%, var(--line) 50% calc(50% + 1px), transparent calc(50% + 1px)) bottom right / 10px 10px no-repeat border-box, var(--bg-deep) !important; }'
    );
    const cornersNeg = await cornersConnected();
    check(
      !cornersNeg.tl.reached && !cornersNeg.br.reached,
      `[DF1/focus] 否定對照：切角補線維持 1px --line 時兩個切角都必須判定斷開（${JSON.stringify(cornersNeg)}）`
    );
    await removeStyle('__df1_neg');
    await cdp.eval('document.activeElement && document.activeElement.blur(); true');
    await cdp.send('Emulation.setFocusEmulationEnabled', { enabled: false });

    // --- [DF1/pin]：寬 ≥760 時提示行出現讓內容框變矮，原本貼底的內容仍貼底；往上捲不被拉回 ---
    await setViewport(1536, 1024);
    const GAP_JS = `(() => { var p = document.querySelector('#output .output-text'); return p ? Math.round((p.scrollHeight - p.scrollTop - p.clientHeight) * 100) / 100 : null; })()`;
    await cdp.eval("window.liveOutput.select('win', 'wJ:p3'); true");
    await cdp.waitFor(
      "(() => { var n = document.querySelector('#output .output-truncated-notice'); var p = document.querySelector('#output .output-text'); return !!n && !n.hidden && !!p && p.scrollHeight > p.clientHeight; })()",
      5000,
      '[DF1/pin] 選 wJ:p3（long）：內容超過一屏且截斷提示出現'
    );
    const gapAfterTruncated = await cdp.eval(GAP_JS);
    check(gapAfterTruncated !== null && gapAfterTruncated <= 1, `[DF1/pin] 截斷提示出現後內容仍貼底（距底 ${gapAfterTruncated}px）`);
    await cdp.eval("(() => { var p = document.querySelector('#output .output-text'); p.scrollTop = p.scrollHeight; return true; })()");
    await sleep(1300);
    const gapBeforeFail = await cdp.eval(GAP_JS);
    check(gapBeforeFail !== null && gapBeforeFail <= 1, `[DF1/pin] 前提：失敗前內容貼底（距底 ${gapBeforeFail}px）`);
    const FAIL_FETCH_JS = `(() => {
      if (!window.__df1OrigFetch) window.__df1OrigFetch = window.fetch;
      window.fetch = function (input, init) {
        var u = typeof input === 'string' ? input : input.url;
        if (u.indexOf('/output') !== -1) {
          return Promise.resolve(new Response(JSON.stringify({ error: 'DF1 模擬暫時失敗' }), { status: 503, headers: { 'content-type': 'application/json' } }));
        }
        return window.__df1OrigFetch.apply(this, arguments);
      };
      return true;
    })()`;
    const RESTORE_FETCH_JS = '(() => { if (window.__df1OrigFetch) { window.fetch = window.__df1OrigFetch; } return true; })()';
    const STALE_SHOWN_JS = "(() => { var o = document.getElementById('output'); var r = o.querySelector('.output-error-reason'); return o.classList.contains('is-stale') && !!r && !r.hidden; })()";
    await cdp.eval(FAIL_FETCH_JS);
    await cdp.waitFor(STALE_SHOWN_JS, 4000, '[DF1/pin] 模擬 503：過期原因行出現');
    const gapAfterStale = await cdp.eval(GAP_JS);
    check(gapAfterStale !== null && gapAfterStale <= 1, `[DF1/pin] 過期原因行出現後內容仍貼底、最後一行不被切（距底 ${gapAfterStale}px）`);
    // 往上捲的使用者不被拉回：先恢復、捲到頂端，再失敗一次。
    await cdp.eval(RESTORE_FETCH_JS);
    await cdp.waitFor("!document.getElementById('output').classList.contains('is-stale')", 4000, '[DF1/pin] 恢復成功：過期標示消失');
    await cdp.eval("(() => { var p = document.querySelector('#output .output-text'); p.scrollTop = 0; return true; })()");
    await cdp.eval(FAIL_FETCH_JS);
    await cdp.waitFor(STALE_SHOWN_JS, 4000, '[DF1/pin] 再次模擬 503：過期原因行出現');
    const topAfterStale = await cdp.eval("document.querySelector('#output .output-text').scrollTop");
    check(topAfterStale === 0, `[DF1/pin] 使用者往上捲時，過期原因行出現不把內容拉回底部（scrollTop=${topAfterStale}）`);
    await cdp.eval(RESTORE_FETCH_JS);
    // fix round 1（規格對照 Minor）：markGone 路徑——貼底時「pane 已不存在」出現，內容仍貼底。
    await cdp.waitFor("!document.getElementById('output').classList.contains('is-stale')", 4000, '[DF1/pin] 恢復成功：過期標示消失（markGone 前）');
    await cdp.eval("(() => { var p = document.querySelector('#output .output-text'); p.scrollTop = p.scrollHeight; return true; })()");
    await sleep(1300);
    const gapBeforeGone = await cdp.eval(GAP_JS);
    check(gapBeforeGone !== null && gapBeforeGone <= 1, `[DF1/pin] 前提：pane 消失前內容貼底（距底 ${gapBeforeGone}px）`);
    await cdp.eval(`(() => {
      if (!window.__df1OrigFetch) window.__df1OrigFetch = window.fetch;
      window.fetch = function (input, init) {
        var u = typeof input === 'string' ? input : input.url;
        if (u.indexOf('/output') !== -1) {
          return Promise.resolve(new Response(JSON.stringify({ error: 'pane not found' }), { status: 404, headers: { 'content-type': 'application/json' } }));
        }
        return window.__df1OrigFetch.apply(this, arguments);
      };
      return true;
    })()`);
    await cdp.waitFor(
      "(() => { var n = document.querySelector('#output .output-gone-notice'); return !!n && !n.hidden; })()",
      4000,
      '[DF1/pin] 模擬 404：「pane 已不存在」出現'
    );
    const gapAfterGone = await cdp.eval(GAP_JS);
    check(gapAfterGone !== null && gapAfterGone <= 1, `[DF1/pin] 「pane 已不存在」出現後內容仍貼底、最後一行不被切（距底 ${gapAfterGone}px）`);
    await cdp.eval(RESTORE_FETCH_JS);
    await cdp.eval('window.liveOutput.clear(); true');
  } finally {
    await stopChrome(chrome, 'chrome-DF1');
    await stopPreview(preview, 'preview-DF1');
  }
}

// ---------------------------------------------------------------------------
// S6：命令列段落代號驗證自我測試（task 5.4 final review／Codex F4）
// ---------------------------------------------------------------------------

async function partSelfTestSegmentArg() {
  log('=== S6. 命令列段落代號驗證：拼錯或空的代號以非零狀態結束，不印 PASS ===');
  const known = PARTS.map(([code]) => code);
  // 函式層：合法的代號（正對照）與各種不合法的參數。
  const cases = [
    { arg: undefined, ok: true, codes: null },
    { arg: 'V1,G1', ok: true, codes: ['V1', 'G1'] },
    { arg: ' TK1 ', ok: true, codes: ['TK1'] },
    { arg: 'V1X', ok: false, mention: 'V1X' },
    { arg: 'V1,V1X', ok: false, mention: 'V1X' },
    { arg: 'v1', ok: false, mention: 'v1' },
    { arg: '', ok: false },
    { arg: ',', ok: false },
  ];
  for (const c of cases) {
    const r = parseSegmentArg(c.arg, known);
    const good =
      r.ok === c.ok &&
      (c.ok ? JSON.stringify(r.codes) === JSON.stringify(c.codes) : typeof r.message === 'string' && (!c.mention || r.message.includes(c.mention)));
    check(good, `[S6] parseSegmentArg(${JSON.stringify(c.arg)}) → ${c.ok ? '通過' : '拒絕'}（實際 ${JSON.stringify(r)}）`);
  }
  // 行程層：真的用拼錯的代號與空字串跑一次本檔，必須非零結束、不印 RESULT: PASS、印出錯的代號，
  // 而且在驗證失敗時就結束（沒有走到清殘留那一步）。
  for (const arg of ['V1X', '']) {
    const r = spawnSync(process.execPath, [__filename, arg], { encoding: 'utf8', timeout: 30000, windowsHide: true });
    const out = `${r.stdout || ''}${r.stderr || ''}`;
    check(
      r.status !== 0 && r.status !== null && !out.includes('RESULT: PASS') && out.includes('RESULT: FAIL') &&
        (arg === '' || out.includes(arg)) && !out.includes('清掉殘留'),
      `[S6] node visual-check.js ${JSON.stringify(arg)}：非零結束、不印 PASS、指出問題、不清殘留（exit ${r.status}；輸出 ${JSON.stringify(out.trim().slice(0, 200))}）`
    );
  }
}

// ---------------------------------------------------------------------------
// FR1：direction-01-visual task 5.4 final review 修正波（Codex final F1–F3，控制端 Ruling R43）
// ---------------------------------------------------------------------------

// [FR1/scroll-switch]：cockpit 與 p 兩個 Project 都改成 10 個 stage＋額外 4 條 workstream，
// 1536×1024 下兩張 Factory Floor 各自都能兩個方向捲動——前提斷言「p 也捲得動」，否則「切到 p
// 後捲動位置是 0」可能只是瀏覽器把超出範圍的值夾回 0，斷言沒有牙齒。
function craftTwoScrollableProjectsState() {
  const s = loadFixture();
  s.version = 131;
  for (const project of s.projects) {
    const extra = [];
    for (let i = project.stages.length + 1; i <= 10; i += 1) extra.push('S' + i);
    project.stages = project.stages.concat(extra);
    for (let i = 1; i <= 4; i += 1) {
      const wsId = project.id + '-x' + i;
      project.workstreams.push({ id: wsId, name: 'Extra ' + i, binding: { state: 'none' } });
      project.tasks.push({
        id: wsId + '-t',
        title: '捲動用 ' + i,
        workstream: wsId,
        stage: 'S10',
        mark: 'none',
        status: 'ready',
        depends_on: [],
      });
    }
  }
  return s;
}

// Factory Floor 內層捲動容器的捲動位置與它目前畫的是哪個 Project。
const FR1_FLOOR_SCROLL_JS = `(() => {
  var n = document.querySelector('[data-region="floor"] > .projects');
  if (!n) return null;
  var p = n.querySelector(':scope > .project');
  return {
    top: n.scrollTop,
    left: n.scrollLeft,
    maxTop: n.scrollHeight - n.clientHeight,
    maxLeft: n.scrollWidth - n.clientWidth,
    project: p ? p.getAttribute('data-project') : null,
  };
})()`;

// [FR1/focus-edge]：目前的焦點元素若在 Factory Floor 捲動容器內，量它的焦點框（border box
// 往外推 outline-offset＋outline-width）是否四邊都落在容器的可見範圍內。可見範圍＝容器的
// client 區（扣掉框線與捲軸），再扣掉浮在上面的 sticky 欄首（每個元素都會被它蓋）與 sticky
// 列首（只有格子裡的元素會被它蓋；列首本身就是 sticky left）。
const FR1_RING_JS = `(() => {
  var c = document.querySelector('[data-region="floor"] > .projects');
  var a = document.activeElement;
  if (!c || !a || !c.contains(a)) return null;
  var cs = getComputedStyle(a);
  var ow = parseFloat(cs.outlineWidth) || 0;
  var oo = parseFloat(cs.outlineOffset) || 0;
  var e = cs.outlineStyle === 'none' ? 0 : ow + oo;
  var r = a.getBoundingClientRect();
  var ring = { l: r.left - e, t: r.top - e, r: r.right + e, b: r.bottom + e };
  var cr = c.getBoundingClientRect();
  var box = { l: cr.left + c.clientLeft, t: cr.top + c.clientTop };
  box.r = box.l + c.clientWidth;
  box.b = box.t + c.clientHeight;
  var corner = c.querySelector('.ff-corner');
  if (corner) {
    var k = corner.getBoundingClientRect();
    box.t = Math.max(box.t, k.bottom);
    if (!a.closest('.ff-row-header')) box.l = Math.max(box.l, k.right);
  }
  var tol = 0.5;
  var clipped = [];
  if (ring.l < box.l - tol) clipped.push('left ' + (box.l - ring.l).toFixed(1));
  if (ring.t < box.t - tol) clipped.push('top ' + (box.t - ring.t).toFixed(1));
  if (ring.r > box.r + tol) clipped.push('right ' + (ring.r - box.r).toFixed(1));
  if (ring.b > box.b + tol) clipped.push('bottom ' + (ring.b - box.b).toFixed(1));
  var label = (a.getAttribute('data-action') || a.tagName) + ':' + (a.getAttribute('data-task') || a.getAttribute('data-workstream') || '');
  return { label: label, outline: cs.outlineStyle, extent: e, clipped: clipped, scroll: [Math.round(c.scrollLeft), Math.round(c.scrollWidth - c.clientWidth), Math.round(c.scrollTop), Math.round(c.scrollHeight - c.clientHeight)], ring: [ring.l, ring.t, ring.r, ring.b].map(function (v) { return +v.toFixed(1); }), box: [box.l, box.t, box.r, box.b].map(function (v) { return +v.toFixed(1); }) };
})()`;

// [FR1/text200]：每顆 task 節點上的按鈕（以及列首的按鈕）框是否完整落在所屬節點（列首）框內。
const FR1_BUTTON_FIT_JS = `(() => {
  var out = { total: 0, bad: [] };
  var tol = 0.5;
  var buttons = document.querySelectorAll('[data-region="floor"] .task-node .action-button, [data-region="floor"] .ff-row-header .action-button');
  for (var i = 0; i < buttons.length; i += 1) {
    var b = buttons[i];
    var owner = b.closest('.task-node') || b.closest('.ff-row-header');
    var r = b.getBoundingClientRect();
    var o = owner.getBoundingClientRect();
    out.total += 1;
    var over = Math.max(o.left - r.left, o.top - r.top, r.right - o.right, r.bottom - o.bottom);
    if (over > tol) {
      out.bad.push({ text: b.textContent.trim(), owner: owner.className.split(' ')[0], task: b.getAttribute('data-task'), over: +over.toFixed(1) });
    }
  }
  return out;
})()`;

async function partFinalReviewFixes() {
  log('=== FR1. final review 修正：切換 Project 捲動位置不外洩／Floor 邊界焦點框完整可見／文字放大 200% 按鈕不出節點 ===');
  let preview = null;
  let chrome = null;
  try {
    preview = await startPreview({}, 'preview-FR1');
    const url = `http://127.0.0.1:${preview.port}/`;
    chrome = await startChrome(pickPort(19170, [preview.port]), url, 'chrome-FR1', '1536,1024');
    const { cdp } = chrome;
    await waitForFirstProjection(cdp, preview.port);

    async function setViewport(width, height) {
      await cdp.send('Emulation.setDeviceMetricsOverride', { width, height, deviceScaleFactor: 1, mobile: false });
      await sleep(150);
      await cdp.eval('window.scrollTo(0, 0); true');
      const actual = await cdp.eval('({ w: window.innerWidth, h: window.innerHeight })');
      check(actual.w === width && actual.h === height, `[FR1] 視窗精準設成 ${width}x${height}（實際 ${JSON.stringify(actual)}）`);
    }
    async function injectStyle(id, cssText) {
      await cdp.eval(`(() => { var s = document.getElementById(${JSON.stringify(id)}); if (!s) { s = document.createElement('style'); s.id = ${JSON.stringify(id)}; document.head.appendChild(s); } s.textContent = ${JSON.stringify(cssText)}; return true; })()`);
      await sleep(100);
    }
    async function removeStyle(id) {
      await cdp.eval(`(() => { var s = document.getElementById(${JSON.stringify(id)}); if (s) s.remove(); return true; })()`);
      await sleep(100);
    }
    async function selectProject(id, tag) {
      await cdp.click(`[data-action="select-project"][data-project="${id}"]`);
      return cdp.waitFor(
        `!!document.querySelector('[data-region="floor"] > .projects > .project[data-project="${id}"]')`,
        2000,
        `${tag} Factory Floor 改畫 ${id}`
      );
    }
    async function setFloorScroll(top, left) {
      await cdp.eval(`(() => { var n = document.querySelector('[data-region="floor"] > .projects'); n.scrollTop = ${top}; n.scrollLeft = ${left}; return true; })()`);
      await sleep(50);
      return cdp.eval(FR1_FLOOR_SCROLL_JS);
    }

    // --- [FR1/scroll-switch]（Codex final F1）：捲動位置保留（Ruling R19）只適用於同一個 Project
    // 的重畫；切換 Project 時新 Project 的 Factory Floor 從 0 開始（規則：切換即歸零，切回原本
    // 的 Project 也是 0——不另外記每個 Project 的位置）。 ---
    await setViewport(1536, 1024);
    const twoInject = await injectState(cdp, craftTwoScrollableProjectsState());
    check(twoInject && twoInject.ok === true, '[FR1/scroll-switch] 注入兩個都能捲動的 Project');
    await selectProject('cockpit', '[FR1/scroll-switch]');
    const c0 = await setFloorScroll(120, 240);
    check(
      c0 && c0.project === 'cockpit' && c0.top > 0 && c0.left > 0,
      `[FR1/scroll-switch] 前提：cockpit 的 Factory Floor 捲到非 0（實際 ${JSON.stringify(c0)}）`
    );
    await cdp.eval('window.repaint(); true');
    const c1 = await cdp.eval(FR1_FLOOR_SCROLL_JS);
    check(
      c1 && c1.project === 'cockpit' && c1.top === c0.top && c1.left === c0.left,
      `[FR1/scroll-switch] 同一個 Project 的 UI 重畫（repaint）保留捲動位置（前 ${JSON.stringify(c0)}，後 ${JSON.stringify(c1)}）`
    );
    const reInject = await injectState(cdp, craftTwoScrollableProjectsState());
    const c2 = await cdp.eval(FR1_FLOOR_SCROLL_JS);
    check(
      reInject && reInject.ok === true && c2 && c2.project === 'cockpit' && c2.top === c0.top && c2.left === c0.left,
      `[FR1/scroll-switch] 同一個 Project 收到新投影重畫後保留捲動位置（R19；前 ${JSON.stringify(c0)}，後 ${JSON.stringify(c2)}）`
    );
    await selectProject('p', '[FR1/scroll-switch]');
    const p0 = await cdp.eval(FR1_FLOOR_SCROLL_JS);
    check(
      p0 && p0.project === 'p' && p0.maxTop > 0 && p0.maxLeft > 0,
      `[FR1/scroll-switch] 前提：p 的 Factory Floor 兩個方向都捲得動（實際 ${JSON.stringify(p0)}）`
    );
    check(
      p0 && p0.top === 0 && p0.left === 0,
      `[FR1/scroll-switch] 切到 p 後 Factory Floor 從 0 開始，不沿用 cockpit 的捲動位置（實際 ${JSON.stringify(p0)}，cockpit 當時 ${JSON.stringify(c0)}）`
    );
    const p1 = await setFloorScroll(60, 150);
    await cdp.eval('window.repaint(); true');
    const p2 = await cdp.eval(FR1_FLOOR_SCROLL_JS);
    check(
      p1 && p1.top > 0 && p1.left > 0 && p2 && p2.project === 'p' && p2.top === p1.top && p2.left === p1.left,
      `[FR1/scroll-switch] p 自己的捲動位置在重畫後保留（前 ${JSON.stringify(p1)}，後 ${JSON.stringify(p2)}）`
    );
    await selectProject('cockpit', '[FR1/scroll-switch]');
    const c3 = await cdp.eval(FR1_FLOOR_SCROLL_JS);
    check(
      c3 && c3.project === 'cockpit' && c3.top === 0 && c3.left === 0,
      `[FR1/scroll-switch] 切回 cockpit 後同樣從 0 開始（切換即歸零規則；實際 ${JSON.stringify(c3)}）`
    );

    // --- [FR1/focus-edge]（Codex final F2＝5.2 M1）：真實 Tab／Shift+Tab 走過 Factory Floor 的
    // 每一個可聚焦元素，每一站的焦點框四邊都要完整落在捲動容器的可見範圍內。 ---
    async function tabWalk(tag) {
      // 起點：左欄最後一個 Project 項目（DOM 順序緊接在後的就是 Factory Floor）。
      await setFloorScroll(0, 0);
      await cdp.eval(`(() => { var items = document.querySelectorAll('[data-action="select-project"]'); items[items.length - 1].focus(); return true; })()`);
      const stations = [];
      let entered = false;
      for (let i = 0; i < 300; i += 1) {
        await cdp.send('Input.dispatchKeyEvent', { type: 'keyDown', key: 'Tab', code: 'Tab', windowsVirtualKeyCode: 9 });
        await cdp.send('Input.dispatchKeyEvent', { type: 'keyUp', key: 'Tab', code: 'Tab', windowsVirtualKeyCode: 9 });
        const m = await cdp.eval(FR1_RING_JS);
        if (m === null) {
          if (entered) break;
          continue;
        }
        entered = true;
        stations.push({ dir: 'Tab', ...m });
      }
      // 往回：從 Floor 之後的第一個元素 Shift+Tab 走回左欄。
      let enteredBack = false;
      for (let i = 0; i < 300; i += 1) {
        await cdp.send('Input.dispatchKeyEvent', { type: 'keyDown', key: 'Tab', code: 'Tab', windowsVirtualKeyCode: 9, modifiers: 8 });
        await cdp.send('Input.dispatchKeyEvent', { type: 'keyUp', key: 'Tab', code: 'Tab', windowsVirtualKeyCode: 9, modifiers: 8 });
        const m = await cdp.eval(FR1_RING_JS);
        if (m === null) {
          if (enteredBack) break;
          continue;
        }
        enteredBack = true;
        stations.push({ dir: 'Shift+Tab', ...m });
      }
      log(`${tag}：Tab／Shift+Tab 共 ${stations.length} 站`);
      return stations;
    }
    const EDGE_CASES = [
      { tag: '1536×1024 10 stage', w: 1536, h: 1024, state: craftStickyFloorState },
      { tag: '1280×650 預設投影', w: 1280, h: 650, state: loadFixture },
      { tag: '700×900 10 stage', w: 700, h: 900, state: craftStickyFloorState },
    ];
    for (const ec of EDGE_CASES) {
      await setViewport(ec.w, ec.h);
      await injectState(cdp, ec.state());
      await selectProject('cockpit', `[FR1/focus-edge] ${ec.tag}`);
      const stations = await tabWalk(`[FR1/focus-edge] ${ec.tag}`);
      const withRing = stations.filter((s) => s.outline !== 'none' && s.extent > 0);
      check(
        stations.length >= 10 && withRing.length === stations.length,
        `[FR1/focus-edge] ${ec.tag} 前提：Tab 路徑走過 Floor 內至少 10 站、每站都有外推焦點框（實際 ${stations.length} 站、有框 ${withRing.length}）`
      );
      const bad = stations.filter((s) => s.clipped.length > 0);
      check(
        bad.length === 0,
        `[FR1/focus-edge] ${ec.tag}：每一站焦點框四邊都完整可見（被裁／被蓋 ${bad.length} 站：${JSON.stringify(bad.slice(0, 6))}）`
      );
    }
    // 重畫不把畫面拉回焦點（render.js focusin 補捲只處理鍵盤移動焦點，不處理 paint() 自己還原
    // 焦點）：鍵盤焦點留在 Floor 裡的按鈕上、把 Floor 捲開到看不到它，UI 重畫與新投影重畫後捲動
    // 位置都不變、焦點仍在同一顆按鈕上。
    await setViewport(1536, 1024);
    await injectState(cdp, craftStickyFloorState());
    await selectProject('cockpit', '[FR1/focus-edge] 重畫不拉回');
    await tabWalk('[FR1/focus-edge] 重畫不拉回（先走一遍讓焦點停在 Floor 之後）');
    await cdp.eval(`(() => { var items = document.querySelectorAll('[data-action="select-project"]'); items[items.length - 1].focus(); return true; })()`);
    await cdp.send('Input.dispatchKeyEvent', { type: 'keyDown', key: 'Tab', code: 'Tab', windowsVirtualKeyCode: 9 });
    await cdp.send('Input.dispatchKeyEvent', { type: 'keyUp', key: 'Tab', code: 'Tab', windowsVirtualKeyCode: 9 });
    const kbdFocus = await cdp.eval(FR1_RING_JS);
    const awayScroll = await setFloorScroll(9999, 9999);
    const focusGone = await cdp.eval(`(() => {
      var c = document.querySelector('[data-region="floor"] > .projects');
      var r = document.activeElement.getBoundingClientRect();
      var b = c.getBoundingClientRect();
      return r.bottom < b.top || r.top > b.bottom || r.right < b.left || r.left > b.right;
    })()`);
    await cdp.eval('window.repaint(); true');
    const afterUiRepaint = await cdp.eval(FR1_FLOOR_SCROLL_JS);
    await injectState(cdp, craftStickyFloorState());
    const afterStateRepaint = await cdp.eval(FR1_FLOOR_SCROLL_JS);
    const focusAfter = await cdp.eval(FR1_RING_JS);
    check(
      kbdFocus !== null && awayScroll.top > 0 && focusGone === true,
      `[FR1/focus-edge] 重畫不拉回 前提：鍵盤焦點在 Floor 內（${kbdFocus && kbdFocus.label}）且 Floor 已捲到看不到它（${JSON.stringify(awayScroll)}）`
    );
    check(
      afterUiRepaint.top === awayScroll.top && afterUiRepaint.left === awayScroll.left &&
        afterStateRepaint.top === awayScroll.top && afterStateRepaint.left === awayScroll.left &&
        focusAfter !== null && kbdFocus !== null && focusAfter.label === kbdFocus.label,
      `[FR1/focus-edge] 重畫不拉回：UI 重畫與新投影重畫後 Floor 捲動位置不變、焦點仍在 ${kbdFocus && kbdFocus.label}（捲開 ${JSON.stringify(awayScroll)}；UI 重畫後 ${JSON.stringify(afterUiRepaint)}；新投影後 ${JSON.stringify(afterStateRepaint)}；焦點 ${focusAfter && focusAfter.label}）`
    );

    // 否定對照 1：擋掉 render.js 的 focusin 補捲（頁面層級 capture 階段攔下 focusin、不讓它
    // 傳到 #app），同一條 Tab 路徑必須抓到 Chrome 橫向不捲造成的被裁站。
    await cdp.eval(`(() => { window.__fr1BlockFocusin = function (e) { e.stopPropagation(); }; window.addEventListener('focusin', window.__fr1BlockFocusin, true); return true; })()`);
    const negHandler = await tabWalk('[FR1/focus-edge] 否定對照 1（擋掉 focusin 補捲）');
    await cdp.eval(`(() => { window.removeEventListener('focusin', window.__fr1BlockFocusin, true); return true; })()`);
    const negHandlerBad = negHandler.filter((s) => s.clipped.length > 0);
    check(
      negHandlerBad.length > 0,
      `[FR1/focus-edge] 否定對照 1：擋掉 focusin 補捲後必須抓到被裁的站（抓到 ${negHandlerBad.length}／${negHandler.length}：${JSON.stringify(negHandlerBad.slice(0, 3))}）`
    );
    // 否定對照 2：拿掉捲動容器四邊為焦點框預留的 scroll-padding（上緣退回只讓開欄首、左緣退回
    // 只讓開列首上限），同一條 Tab 路徑必須抓到被裁／被蓋的站。
    await injectStyle(
      'fr1-neg-edge',
      '[data-region="floor"] > .projects { scroll-padding: calc(2.3 * var(--fs-meta) + 12px) 0 0 200px !important; }'
    );
    const negStations = await tabWalk('[FR1/focus-edge] 否定對照 2');
    await removeStyle('fr1-neg-edge');
    const negBad = negStations.filter((s) => s.clipped.length > 0);
    check(
      negBad.length > 0,
      `[FR1/focus-edge] 否定對照 2：拿掉焦點框預留的 scroll-padding 後必須抓到被裁／被蓋的站（抓到 ${negBad.length}／${negStations.length}：${JSON.stringify(negBad.slice(0, 3))}）`
    );

    // --- [FR1/text200]（Codex final F3＝5.2 M4 前半）：只放大文字 200%（同 CH1 (f)／DF1：根字級
    // 200%＋四個 --fs-* token 加倍）時，每顆按鈕都完整留在自己的節點（列首）裡。 ---
    const baseFs = await cdp.eval(`(() => {
      var cs = getComputedStyle(document.documentElement);
      return ['--fs-title', '--fs-panel', '--fs-dense', '--fs-meta'].map(function (n) { return [n, parseFloat(cs.getPropertyValue(n))]; });
    })()`);
    const textScaleCss =
      'html { font-size: 200%; } :root { ' + baseFs.map(([n, px]) => `${n}: ${px * 2}px;`).join(' ') + ' }';
    const metaBefore = await cdp.eval("parseFloat(getComputedStyle(document.querySelector('.task-node .action-button')).fontSize)");
    await injectState(cdp, loadFixture());
    const TEXT_CASES = [
      { w: 700, h: 900 },
      { w: 1100, h: 900 },
      { w: 1536, h: 1024 },
    ];
    for (const { w, h } of TEXT_CASES) {
      await setViewport(w, h);
      for (const pid of ['cockpit', 'p']) {
        await selectProject(pid, `[FR1/text200] ${w}×${h}`);
        await injectStyle('fr1-text', textScaleCss);
        const metaAfter = await cdp.eval("parseFloat(getComputedStyle(document.querySelector('.task-node .action-button')).fontSize)");
        check(
          metaAfter === metaBefore * 2,
          `[FR1/text200] ${w}×${h} ${pid} 前提：按鈕字級真的放大成兩倍（${metaBefore} → ${metaAfter}）`
        );
        const fit = await cdp.eval(FR1_BUTTON_FIT_JS);
        check(
          fit.total > 0 && fit.bad.length === 0,
          `[FR1/text200] ${w}×${h} ${pid}：文字放大 200% 時 ${fit.total} 顆按鈕都完整在所屬節點／列首內（超出：${JSON.stringify(fit.bad)}）`
        );
        await removeStyle('fr1-text');
      }
    }
    // 否定對照：700×900、cockpit、文字放大 200%，把節點按鈕還原成「不可斷行、不設寬度上限」，
    // 必須抓到溢出節點的按鈕（Codex／5.2 M4 量到的 Completed）。
    await setViewport(700, 900);
    await selectProject('cockpit', '[FR1/text200] 否定對照');
    await injectStyle('fr1-text', textScaleCss);
    await injectStyle(
      'fr1-neg-text',
      '[data-region="floor"] .task-node .action-button { max-width: none !important; white-space: nowrap !important; overflow-wrap: normal !important; }'
    );
    const negFit = await cdp.eval(FR1_BUTTON_FIT_JS);
    check(
      negFit.bad.length > 0,
      `[FR1/text200] 否定對照：按鈕還原成不可斷行時必須抓到溢出節點的按鈕（抓到 ${JSON.stringify(negFit.bad)}）`
    );
    await removeStyle('fr1-neg-text');
    await removeStyle('fr1-text');
  } finally {
    await stopChrome(chrome, 'chrome-FR1');
    await stopPreview(preview, 'preview-FR1');
  }
}

// ---------------------------------------------------------------------------
// FT1–FT3：file-review 的 cockpit-dashboard delta（file-review task 3.5；先寫測試，4.x 前預期 RED）
// ---------------------------------------------------------------------------

// 前端契約的定位規則（與 docs/research/2026-09-27/files-check.js 的 pageHelpers() 同一份契約 C1–C6，
// 這裡只取 FT1–FT3 用得到的部分；改契約時兩邊一起改）。
function filesContractHelpers() {
  const txt = (el) => {
    if (!el) return '';
    let t = el.innerText;
    if (typeof t !== 'string') t = el.textContent || '';
    return t.replace(/\s+/g, ' ').trim();
  };
  const visible = (el) => {
    if (!el || !el.isConnected) return false;
    const cs = getComputedStyle(el);
    if (cs.display === 'none' || cs.visibility === 'hidden') return false;
    return el.getClientRects().length > 0;
  };
  const filesRoot = () => document.getElementById('files');
  const reviewRoot = () => document.getElementById('review');
  const leftTablist = () => (filesRoot() ? filesRoot().querySelector('[role="tablist"]') : null);
  const leftTab = (name) => (leftTablist() ? Array.from(leftTablist().querySelectorAll('[role="tab"]')).find((t) => txt(t) === name) || null : null);
  const tree = () => (filesRoot() ? filesRoot().querySelector('[role="tree"]') : null);
  const row = (p) => (tree() ? Array.from(tree().querySelectorAll('[role="treeitem"]')).find((r) => r.getAttribute('title') === p) || null : null);
  const reviewTablist = () => (reviewRoot() ? reviewRoot().querySelector('[role="tablist"]') : null);
  const reviewTabs = () => (reviewTablist() ? Array.from(reviewTablist().querySelectorAll('[role="tab"]')) : []);
  const fileTab = (p) => reviewTabs().find((t) => t.getAttribute('data-path') === p) || null;
  const selectedTab = () => reviewTabs().find((t) => t.getAttribute('aria-selected') === 'true') || null;
  const panelOf = (t) => {
    if (!t) return null;
    const id = t.getAttribute('aria-controls');
    const p = id ? document.getElementById(id) : null;
    return p && p.getAttribute('role') === 'tabpanel' ? p : null;
  };
  const currentPanel = () => panelOf(selectedTab());
  const viewer = () => (currentPanel() ? currentPanel().querySelector('[data-viewer]') : null);
  const scrollable = (n) => {
    const cs = getComputedStyle(n);
    return (cs.overflowY === 'auto' || cs.overflowY === 'scroll') && n.scrollHeight > n.clientHeight + 1;
  };
  const contentScroller = () => {
    const p = currentPanel();
    if (!p) return null;
    for (let n = viewer() || p; n; n = n.parentElement) {
      if (scrollable(n)) return n;
      if (n === p) break;
    }
    for (const n of p.querySelectorAll('*')) if (scrollable(n)) return n;
    return null;
  };
  const contract = () => ({
    files: !!filesRoot(),
    leftTab檔案: !!leftTab('檔案'),
    tree: !!tree(),
    review: !!reviewRoot(),
    reviewTablist: !!reviewTablist(),
    tabs: reviewTabs().map((t) => ({ label: txt(t), path: t.getAttribute('data-path'), selected: t.getAttribute('aria-selected') })),
  });
  window.__fcv = { txt, visible, leftTab, row, reviewRoot, reviewTablist, reviewTabs, fileTab, selectedTab, currentPanel, viewer, contentScroller, contract };
  return true;
}
const FILES_CONTRACT_JS = `(${filesContractHelpers.toString()})()`;

function runFn(cdp, fn, ...args) {
  return cdp.eval(`(${fn.toString()})(${args.map((a) => JSON.stringify(a)).join(',')})`);
}
async function pollFn(cdp, fn, args, timeoutMs) {
  const start = Date.now();
  for (;;) {
    const v = await runFn(cdp, fn, ...args).catch(() => false);
    if (v) return v;
    if (Date.now() - start >= timeoutMs) return false;
    await sleep(100);
  }
}
// 前置條件不成立：記一條 FAIL 後中止本段（main() 認得 ftAbort，不再多記一條「段中止」）。
function ftNeed(cond, label) {
  if (!check(cond, label)) {
    const e = new Error(label);
    e.ftAbort = true;
    throw e;
  }
}
// 把 finder 回傳的元素標上 data-fc-click，再用既有的 cdp.click() 點（#files／#review 不在整頁重畫範圍，
// 標記不會被洗掉）。
async function ftClick(cdp, finderSrc, args, label) {
  const marked = await cdp.eval(`(() => {
    document.querySelectorAll('[data-fc-click]').forEach((n) => n.removeAttribute('data-fc-click'));
    const el = (${finderSrc})(${args.map((a) => JSON.stringify(a)).join(',')});
    if (!el) return false;
    el.setAttribute('data-fc-click', '1');
    return true;
  })()`);
  ftNeed(marked, `找得到要點的元素：${label}`);
  ftNeed(await cdp.click('[data-fc-click="1"]'), `點 ${label}`);
}
async function ftContractDump(cdp) {
  return JSON.stringify(await runFn(cdp, () => window.__fcv.contract()).catch((e) => ({ error: e.message })));
}

// 選定 wJ:p4（根目錄 review-repo 暫存副本）→ 左欄「檔案」→ 逐一在檔案樹點開 relPaths。
async function ftOpenFiles(cdp, relPaths) {
  const paneSel = '.pane-row[data-runtime="win"][data-pane="wJ:p4"]';
  ftNeed(await cdp.click(paneSel), '點 pane 列 win/wJ:p4');
  ftNeed(!!(await pollFn(cdp, (s) => !!document.querySelector(s) && document.querySelector(s).classList.contains('selected'), [paneSel], 5000)), 'wJ:p4 出現選定標示');
  const hasLeft = await pollFn(cdp, () => !!window.__fcv.leftTab('檔案'), [], 5000);
  ftNeed(!!hasLeft, `找不到左欄分頁「檔案」（files-check.js 前端契約 C1：#files 內 role="tablist" 的 role="tab"）；目前 DOM：${await ftContractDump(cdp)}`);
  await ftClick(cdp, '(n) => window.__fcv.leftTab(n)', ['檔案'], '左欄分頁「檔案」');
  for (const rel of relPaths) {
    const parts = rel.split('/');
    for (let i = 1; i < parts.length; i++) {
      const dir = parts.slice(0, i).join('/');
      ftNeed(!!(await pollFn(cdp, (p) => !!window.__fcv.row(p), [dir], 5000)), `檔案樹有資料夾列 ${dir}（契約 C2）`);
      if ((await runFn(cdp, (p) => window.__fcv.row(p).getAttribute('aria-expanded'), dir)) !== 'true') {
        await ftClick(cdp, '(p) => window.__fcv.row(p)', [dir], `資料夾列 ${dir}`);
        ftNeed(!!(await pollFn(cdp, (p) => window.__fcv.row(p).getAttribute('aria-expanded') === 'true', [dir], 5000)), `資料夾列 ${dir} 展開`);
      }
    }
    const rowOk = await pollFn(cdp, (p) => !!window.__fcv.row(p) && window.__fcv.visible(window.__fcv.row(p)), [rel], 5000);
    ftNeed(!!rowOk, `檔案樹有可見的列 title="${rel}"（契約 C2）；目前 DOM：${await ftContractDump(cdp)}`);
    await ftClick(cdp, '(p) => window.__fcv.row(p)', [rel], `檔案列 ${rel}`);
    const opened = await pollFn(cdp, (p) => !!window.__fcv.fileTab(p) && window.__fcv.fileTab(p).getAttribute('aria-selected') === 'true', [rel], 5000);
    ftNeed(!!opened, `${rel} 的檔案分頁出現並成為目前分頁（契約 C3）；目前 DOM：${await ftContractDump(cdp)}`);
  }
}

async function ftWaitReviewRepo(preview) {
  for (let i = 0; i < 40 && !preview.fixturePaths.reviewRepo; i++) await sleep(50);
  ftNeed(!!preview.fixturePaths.reviewRepo, `讀到 ui_preview 印出的 review-repo 暫存路徑（${preview.fixturePaths.reviewRepo}）`);
  return preview.fixturePaths.reviewRepo;
}

// FT1：GIVEN 視窗寬 1280，已打開 20 個檔名各長 60 個字元的檔案分頁 WHEN 重畫 THEN 分頁列在內部橫向捲動，
// 頁面沒有橫向捲軸，中欄寬度不變。
async function partManyFileTabs() {
  log('=== FT1. dashboard/分頁很多不撐破頁面（1280 寬、20 個 60 字元檔名的檔案分頁）===');
  let preview = null;
  let chrome = null;
  try {
    preview = await startPreview({}, 'preview-FT1');
    const reviewRepo = await ftWaitReviewRepo(preview);
    const names = [];
    for (let i = 1; i <= 20; i++) {
      const base = `tab-${String(i).padStart(2, '0')}-`;
      const name = `${base}${'x'.repeat(60 - base.length - 3)}.md`;
      fs.writeFileSync(path.join(reviewRepo, name), `# ${name}\n\nFT1 檔案（只在 ui_preview 暫存副本內）。\n`);
      names.push(name);
    }
    check(names.every((n) => n.length === 60), '[FT1] 20 個檔名各長 60 個字元（只寫在 ui_preview 暫存副本）');
    chrome = await startChrome(pickPort(19410, [preview.port]), `http://127.0.0.1:${preview.port}/`, 'chrome-FT1', '1280,1024');
    const { cdp } = chrome;
    await waitForFirstProjection(cdp, preview.port);
    await cdp.eval(FILES_CONTRACT_JS);
    const floor0 = await runFn(cdp, () => document.querySelector('[data-region="floor"]').getBoundingClientRect().width);
    await ftOpenFiles(cdp, names);
    await cdp.eval('window.repaint(); true');
    await sleep(300);
    const m = await runFn(cdp, () => {
      const tl = window.__fcv.reviewTablist();
      const cs = getComputedStyle(tl);
      const floor = document.querySelector('[data-region="floor"]').getBoundingClientRect();
      const review = document.getElementById('review').getBoundingClientRect();
      const de = document.documentElement;
      return {
        tabs: window.__fcv.reviewTabs().length,
        overflowX: cs.overflowX,
        tlScroll: tl.scrollWidth,
        tlClient: tl.clientWidth,
        docScroll: de.scrollWidth,
        docClient: de.clientWidth,
        floor: { left: floor.left, right: floor.right, width: floor.width },
        review: { left: review.left, right: review.right },
      };
    });
    log(`[FT1] 量測：${JSON.stringify(m)}`);
    check(m.tabs === 21, `[FT1] 分頁列有 Live Output＋20 個檔案分頁（實際 ${m.tabs}）`);
    check((m.overflowX === 'auto' || m.overflowX === 'scroll') && m.tlScroll > m.tlClient + 1, `[FT1] 分頁列在內部橫向捲動（overflow-x ${m.overflowX}，scrollWidth ${m.tlScroll} > clientWidth ${m.tlClient}）`);
    check(m.docScroll <= m.docClient, `[FT1] 頁面沒有橫向捲軸（documentElement scrollWidth ${m.docScroll} ≤ clientWidth ${m.docClient}）`);
    check(Math.abs(m.floor.width - floor0) <= 0.5, `[FT1] 中欄寬度不變（Factory Floor 寬 ${floor0} → ${m.floor.width}）`);
    check(m.review.left >= m.floor.left - 1 && m.review.right <= m.floor.right + 1, `[FT1] 分頁區沒有超出中欄（#review ${m.review.left}–${m.review.right}，中欄 ${m.floor.left}–${m.floor.right}）`);
  } finally {
    await stopChrome(chrome, 'chrome-FT1');
    await stopPreview(preview, 'preview-FT1');
  }
}

// FT2 的快照與比對（file-review task 3.5 fix round 1，Codex finding 2）：除了分頁區外殼（#review、分頁列、各分頁、
// tabpanel、檢視器元素），還保存檢視器底下**所有子孫節點**（Markdown 內容本身）逐一比 identity；捲動容器要是
// 「目前」的 contentScroller() 且仍在頁面上，捲動位置讀目前的捲動容器，不是讀保存下來、可能已脫離頁面的舊節點。
// finderKey 為 null 時用 window.__fcv；偵測器自我測試用合成 DOM 時傳入另一個同介面的全域物件名稱
// （reviewRoot／reviewTablist／reviewTabs／currentPanel／viewer／contentScroller／selectedTab）。
function ft2Snapshot(finderKey) {
  const F = finderKey ? window[finderKey] : window.__fcv;
  const sc = F.contentScroller();
  const viewer = F.viewer();
  if (!sc || !viewer) return { error: `找不到內容捲動容器或檢視器（契約 C6；scroller ${!!sc}、viewer ${!!viewer}）` };
  window.__ft2 = {
    review: F.reviewRoot(),
    tablist: F.reviewTablist(),
    tabs: F.reviewTabs(),
    panel: F.currentPanel(),
    viewer,
    content: Array.from(viewer.querySelectorAll('*')),
    sc,
    scrollTop: sc.scrollTop,
    selected: F.selectedTab(),
  };
  return { scrollTop: sc.scrollTop, content: window.__ft2.content.length };
}
function ft2Compare(finderKey) {
  const F = finderKey ? window[finderKey] : window.__fcv;
  const k = window.__ft2;
  const tabs = F.reviewTabs();
  const viewer = F.viewer();
  const content = viewer ? Array.from(viewer.querySelectorAll('*')) : [];
  const sc = F.contentScroller();
  return {
    review: F.reviewRoot() === k.review && !!k.review && k.review.isConnected,
    tablist: F.reviewTablist() === k.tablist,
    tabs: tabs.length === k.tabs.length && tabs.every((t, i) => t === k.tabs[i]),
    panel: F.currentPanel() === k.panel && k.panel.isConnected,
    viewer: viewer === k.viewer && k.viewer.isConnected,
    content: content.length === k.content.length && content.every((n, i) => n === k.content[i]),
    contentNodes: content.length,
    scSame: sc === k.sc && k.sc.isConnected,
    scrollTop: sc ? sc.scrollTop : null,
    expect: k.scrollTop,
    selected: F.selectedTab() === k.selected,
  };
}
// 合成 DOM 上的偵測器正負對照（真的 Markdown 檢視器由 file-review 4.4 才實作；這段在目前前端就跑得到）：
// (a) 什麼都不動→全部相同；(b) 只把檢視器的 innerHTML 重設成同樣內容→content 與 scSame（捲動容器在檢視器內）
// 必須轉紅，外殼（tabpanel、檢視器元素）仍相同——證明只驗外殼抓不到這種替換。
function ft2SyntheticDetectorProbe() {
  const host = document.createElement('div');
  host.style.cssText = 'position:fixed;left:0;top:0;width:300px;';
  host.innerHTML =
    '<div data-fc-review><div role="tablist"><div role="tab" aria-selected="true" aria-controls="ft2-syn-p">x.md</div></div>' +
    '<div role="tabpanel" id="ft2-syn-p"><div data-viewer="markdown"><div data-fc-sc style="height:60px;overflow:auto">' +
    '<h1>x</h1>' + '<p>段落</p>'.repeat(30) + '</div></div></div></div>';
  document.body.appendChild(host);
  const q = (s) => host.querySelector(s);
  window.__ft2Synth = {
    reviewRoot: () => q('[data-fc-review]'),
    reviewTablist: () => q('[role="tablist"]'),
    reviewTabs: () => Array.from(host.querySelectorAll('[role="tab"]')),
    currentPanel: () => q('[role="tabpanel"]'),
    viewer: () => q('[data-viewer]'),
    contentScroller: () => q('[data-fc-sc]'),
    selectedTab: () => q('[role="tab"]'),
  };
  q('[data-fc-sc]').scrollTop = 200;
  const out = {};
  ft2Snapshot('__ft2Synth');
  out.untouched = ft2Compare('__ft2Synth');
  const v = q('[data-viewer]');
  v.innerHTML = v.innerHTML;
  q('[data-fc-sc]').scrollTop = 200;
  out.innerReplaced = ft2Compare('__ft2Synth');
  host.remove();
  delete window.__ft2Synth;
  return out;
}
const FT2_JS = `window.ft2Snapshot = ${ft2Snapshot.toString()}; window.ft2Compare = ${ft2Compare.toString()}; true`;

// FT2：GIVEN 已打開一個 Markdown 檔案分頁且往下捲動，投影每 100 ms 推送 WHEN 經過 3 秒 THEN 分頁區與檔案內容的
// DOM 節點沒有被換掉，捲動位置不變，目前分頁不變。
async function partRepaintKeepsFileTab() {
  log('=== FT2. dashboard/頻繁重畫不影響檔案分頁（COCKPIT_PREVIEW_PUSH_MS=100，long.md 往下捲）===');
  let preview = null;
  let chrome = null;
  try {
    preview = await startPreview({ COCKPIT_PREVIEW_PUSH_MS: '100' }, 'preview-FT2');
    chrome = await startChrome(pickPort(19420, [preview.port]), `http://127.0.0.1:${preview.port}/`, 'chrome-FT2');
    const { cdp } = chrome;
    await waitForFirstProjection(cdp, preview.port);
    await cdp.eval(FILES_CONTRACT_JS);
    await cdp.eval(FT2_JS);
    const syn = await runFn(cdp, ft2SyntheticDetectorProbe);
    const u = syn.untouched;
    check(
      u.review && u.tablist && u.tabs && u.panel && u.viewer && u.content && u.scSame && u.selected && u.scrollTop === u.expect,
      `[FT2] 偵測器正對照（合成 DOM）：什麼都不動時全部判定相同（${JSON.stringify(u)}）`
    );
    const r = syn.innerReplaced;
    check(
      r.panel && r.viewer && !r.content && !r.scSame,
      `[FT2] 偵測器負對照（合成 DOM）：只重設檢視器 innerHTML 時外殼仍相同、但內容子節點與捲動容器被判定為換掉（${JSON.stringify(r)}）`
    );
    await ftOpenFiles(cdp, ['long.md']);
    ftNeed(!!(await pollFn(cdp, () => !!window.__fcv.viewer() && window.__fcv.txt(window.__fcv.viewer()).includes('long.md'), [], 5000)), '[FT2] long.md 分頁顯示 Markdown 內容（契約 C6：data-viewer）');
    const setup = await runFn(cdp, () => {
      const sc = window.__fcv.contentScroller();
      if (!sc) return { error: '找不到內容捲動容器（契約 C6）' };
      sc.scrollTop = Math.round((sc.scrollHeight - sc.clientHeight) / 2);
      return window.ft2Snapshot(null);
    });
    ftNeed(!setup.error && setup.scrollTop > 0 && setup.content > 0, `[FT2] 前置：內容往下捲動並記下內容子節點（${JSON.stringify(setup)}）`);
    let repaints = 0;
    let last = await cdp.eval("document.getElementById('version').textContent");
    const start = Date.now();
    while (Date.now() - start < 3000) {
      await sleep(100);
      const v = await cdp.eval("document.getElementById('version').textContent");
      if (v !== last) {
        repaints += 1;
        last = v;
      }
    }
    check(repaints >= 10, `[FT2] 3 秒內真的發生了多次整頁重畫（#version 變化 ${repaints} 次）`);
    const after = await runFn(cdp, () => window.ft2Compare(null));
    check(after.review && after.tablist && after.tabs, `[FT2] 分頁區、分頁列與各分頁的 DOM 節點沒有被換掉（${JSON.stringify(after)}）`);
    check(after.panel && after.viewer && after.content, `[FT2] 檔案內容（tabpanel、檢視器與其下 ${after.contentNodes} 個內容節點）的 DOM 節點沒有被換掉`);
    check(after.scSame, '[FT2] 捲動容器仍是原本那一個且仍在頁面上');
    check(after.scrollTop !== null && Math.abs(after.scrollTop - after.expect) <= 1, `[FT2] 捲動位置不變（讀目前的捲動容器：${after.expect} → ${after.scrollTop}）`);
    check(after.selected, '[FT2] 目前分頁不變');
    // 真頁面負對照（前端落地後才跑得到）：只把檢視器的 innerHTML 重設成同樣內容，比對必須轉紅。
    const neg = await runFn(cdp, () => {
      window.ft2Snapshot(null);
      const v = window.__fcv.viewer();
      v.innerHTML = v.innerHTML;
      return window.ft2Compare(null);
    });
    check(!neg.content, `[FT2] 真頁面負對照：重設檢視器 innerHTML 後偵測器判定內容節點被換掉（${JSON.stringify(neg)}）`);
  } finally {
    await stopChrome(chrome, 'chrome-FT2');
    await stopPreview(preview, 'preview-FT2');
  }
}

// FT3：GIVEN 已打開含標題、段落、連結、表格、行內程式碼與程式碼區塊的 Markdown 分頁（README.md）WHEN 對內容區每個
// 含文字的元素計算對比並檢查文字色與背景色 THEN 每一組對比 ≥4.5:1，所有顏色都來自 10 個色彩 token。
// 對比沿用 __cockpitVisualTools.textContrast（含半透明疊層與 opacity 鏈）；「背景色」檢查兩件事：實際背景
// （effectiveBackground 合成結果）與元素自己的不透明背景色都要是 token。排除 PDF canvas 與 iframe（design D11）。
function measureMarkdownColors(tokens, rootSelector) {
  const T = window.__cockpitVisualTools;
  const root = rootSelector ? document.querySelector(rootSelector) : window.__fcv.viewer();
  const set = new Set(tokens);
  const rgb = (c) => `rgb(${Math.round(c.r)}, ${Math.round(c.g)}, ${Math.round(c.b)})`;
  const ownText = (el) => Array.from(el.childNodes).some((n) => n.nodeType === 3 && n.textContent.trim().length > 0);
  const items = [];
  for (const el of [root, ...root.querySelectorAll('*')]) {
    if (el.closest('canvas, iframe')) continue;
    if (!ownText(el) || !window.__fcv.visible(el)) continue;
    const cs = getComputedStyle(el);
    const tc = T.textContrast(el);
    const own = T.parseColor(cs.backgroundColor);
    const eff = rgb(T.effectiveBackground(el));
    items.push({
      tag: el.tagName,
      text: el.textContent.trim().slice(0, 30),
      ratio: Math.round(tc.ratio * 100) / 100,
      color: cs.color,
      colorOk: set.has(cs.color),
      bg: eff,
      bgOk: set.has(eff),
      ownBgOk: own.a === 0 || (own.a >= 0.999 && set.has(rgb(own))),
    });
  }
  return items;
}

async function partMarkdownColors() {
  log('=== FT3. dashboard/Markdown檢視遵守色彩與對比（README.md 內容區）===');
  let preview = null;
  let chrome = null;
  try {
    preview = await startPreview({}, 'preview-FT3');
    chrome = await startChrome(pickPort(19430, [preview.port]), `http://127.0.0.1:${preview.port}/`, 'chrome-FT3');
    const { cdp } = chrome;
    await waitForFirstProjection(cdp, preview.port);
    await installTools(cdp);
    await cdp.eval(FILES_CONTRACT_JS);
    await ftOpenFiles(cdp, ['README.md']);
    const ready = await pollFn(cdp, () => {
      const v = window.__fcv.viewer();
      return !!v && v.getAttribute('data-viewer') === 'markdown' && !!v.querySelector('table');
    }, [], 5000);
    ftNeed(!!ready, `[FT3] README.md 分頁以 Markdown 檢視器顯示（契約 C6：data-viewer="markdown"）；目前 DOM：${await ftContractDump(cdp)}`);
    const has = await runFn(cdp, () => {
      const v = window.__fcv.viewer();
      return {
        heading: !!v.querySelector('h1,h2,h3,h4,h5,h6'),
        p: !!v.querySelector('p'),
        a: !!v.querySelector('a'),
        table: !!v.querySelector('table'),
        inlineCode: Array.from(v.querySelectorAll('code')).some((c) => !c.closest('pre')),
        pre: !!v.querySelector('pre'),
      };
    });
    check(Object.values(has).every(Boolean), `[FT3] 內容區含標題、段落、連結、表格、行內程式碼與程式碼區塊（${JSON.stringify(has)}）`);
    const tokens = Object.values(SPEC_COLORS);
    const items = await runFn(cdp, measureMarkdownColors, tokens, null);
    check(items.length > 0, `[FT3] 內容區有含文字的元素可檢查（${items.length} 個）`);
    const lowContrast = items.filter((i) => i.ratio < 4.5);
    const badColor = items.filter((i) => !i.colorOk || !i.bgOk || !i.ownBgOk);
    check(lowContrast.length === 0, `[FT3] 每一組對比皆不低於 4.5:1（不合格 ${JSON.stringify(lowContrast.slice(0, 8))}）`);
    check(badColor.length === 0, `[FT3] 文字色與背景色都來自 10 個色彩 token（不合格 ${JSON.stringify(badColor.slice(0, 8))}）`);
    // 否定對照（Ruling R3 慣例）：注入一個 #ff0000 文字的節點，偵測器必須抓到它的文字色不是 token。
    const neg = await runFn(cdp, (tks) => {
      const v = window.__fcv.viewer();
      const span = document.createElement('span');
      span.id = 'ft3-negative';
      span.style.color = '#ff0000';
      span.textContent = '否定對照';
      v.appendChild(span);
      return tks.length;
    }, tokens);
    const negItems = await runFn(cdp, measureMarkdownColors, tokens, '#ft3-negative');
    await cdp.eval("document.getElementById('ft3-negative').remove(); true");
    check(neg === 10 && negItems.length === 1 && negItems[0].colorOk === false, `[FT3] 否定對照：#ff0000 文字被判定為不是 token（${JSON.stringify(negItems)}）`);
  } finally {
    await stopChrome(chrome, 'chrome-FT3');
    await stopPreview(preview, 'preview-FT3');
  }
}

// FT4：GIVEN 視窗寬 700，已打開一個含 300 個字元長行的 diff 分頁 WHEN 顯示該分頁 THEN 長行在所屬欄內
// 折行，頁面沒有橫向捲軸（spec cockpit-dashboard「diff 與 Git Graph 不撐破頁面」的 diff 部分；
// git-review task 4.3）；Git Graph 部分（task 4.4）在同一支 preview／chrome 裡接著做：用
// `git commit-tree`＋`update-ref`（不碰工作區／索引，安全機制同 git-check.js 的 `gitTemp()`）在
// review-repo 暫存副本疊 16 個各自獨立的分支（同一個共同祖先），不篩選開啟 Git Graph 時這些分支的
// tip commit 會是最新的 16 列，同時佔滿 16 條車道（design D8「進行中的車道」：分岔前每一列都有
// 15 條「與本列節點無關」的車道穿過＋1 條本列新節點，恰好 16 條），驗「Graph 在分頁內容區內部捲動，
// 頁面沒有橫向捲軸」。互動（選 pane、切「變更」分頁、點列開 diff／Git Graph）在預設寬度（1536）
// 完成——`cdp.click()` 用 `scrollIntoView({block:'start'})`，700 寬的單欄＋整頁捲動版面下 pane 列
// 可能被 sticky 底列擋住點不到（同 git-check.js「窄視窗不橫向捲動」段的既有教訓）；只在量測當下才
// 切到 700 寬。
function ft4VerifyTempRepoToplevel(repoDir) {
  const r = spawnSync('git', ['-C', repoDir, 'rev-parse', '--show-toplevel'], { encoding: 'utf8' });
  const top = (r.stdout || '').trim();
  if (r.status !== 0 || path.resolve(top).toLowerCase() !== path.resolve(repoDir).toLowerCase()) {
    throw new Error(`FT4 安全檢查失敗：git rev-parse --show-toplevel（${JSON.stringify(top)}）與預期的暫存副本路徑（${repoDir}）不符，拒絕寫入`);
  }
}
function ft4GitTemp(repoDir, args, env) {
  ft4VerifyTempRepoToplevel(repoDir);
  const r = spawnSync('git', ['-C', repoDir, ...args], { encoding: 'utf8', env: env || process.env });
  if (r.status !== 0) {
    throw new Error(`FT4：git ${args.join(' ')} 於 ${repoDir} 失敗：${r.stderr}`);
  }
  return (r.stdout || '').trim();
}
function ft4CreateManyLanes(repoDir, count) {
  const base = ft4GitTemp(repoDir, ['rev-parse', 'HEAD']);
  const tree = ft4GitTemp(repoDir, ['rev-parse', `${base}^{tree}`]);
  const nowSec = Math.floor(Date.now() / 1000);
  for (let i = 1; i <= count; i += 1) {
    const env = { ...process.env, GIT_AUTHOR_NAME: 'ft4', GIT_AUTHOR_EMAIL: 'ft4@invalid', GIT_COMMITTER_NAME: 'ft4', GIT_COMMITTER_EMAIL: 'ft4@invalid', GIT_AUTHOR_DATE: `${nowSec + i} +0000`, GIT_COMMITTER_DATE: `${nowSec + i} +0000` };
    const newOid = ft4GitTemp(repoDir, ['-c', 'user.name=ft4', '-c', 'user.email=ft4@invalid', '-c', 'commit.gpgsign=false', 'commit-tree', tree, '-p', base, '-m', `ft4-lane-${i}`], env);
    ft4GitTemp(repoDir, ['update-ref', `refs/heads/ft4-lane-${i}`, newOid]);
  }
}
async function partDiffNoOverflow() {
  log('=== FT4. dashboard/diff 與 Git Graph 不撐破頁面（視窗寬 700）===');
  let preview = null;
  let chrome = null;
  try {
    preview = await startPreview({}, 'preview-FT4');
    const reviewRepo = await ftWaitReviewRepo(preview);
    const longLine = 'x'.repeat(300);
    fs.writeFileSync(path.join(reviewRepo, 'history', 'ft4-longline.md'), `${longLine}\n`);
    ft4CreateManyLanes(reviewRepo, 16);

    chrome = await startChrome(pickPort(19440, [preview.port]), `http://127.0.0.1:${preview.port}/`, 'chrome-FT4');
    const { cdp } = chrome;
    await waitForFirstProjection(cdp, preview.port);

    ftNeed(await cdp.click('.pane-row[data-runtime="win"][data-pane="wJ:p4"]'), '[FT4] 點 pane 列 win/wJ:p4');
    ftNeed(await cdp.waitFor('!!document.querySelector(\'.pane-row[data-pane="wJ:p4"].selected\')', 5000, '[FT4] wJ:p4 出現選定標示'), '[FT4] 選定標示');
    ftNeed(await cdp.click('#files-tab-changes'), '[FT4] 點左欄分頁「變更」');
    await cdp.waitFor("document.getElementById('files-tab-changes').getAttribute('aria-selected') === 'true'", 3000, '[FT4] 左欄「變更」分頁成為目前分頁');
    await cdp.waitFor('!!document.querySelector(\'#changes-panel .changes-row[title="history/ft4-longline.md"]\')', 5000, '[FT4] 「變更」面板列出 history/ft4-longline.md（未追蹤）');
    ftNeed(await cdp.click('#changes-panel .changes-row[title="history/ft4-longline.md"]'), '[FT4] 點「變更」清單的 history/ft4-longline.md 列');
    await cdp.waitFor(
      '(() => { var t = document.querySelector(\'#review [role="tab"][data-diff-path="history/ft4-longline.md"]\'); return !!t && t.getAttribute(\'aria-selected\') === \'true\'; })()',
      5000,
      '[FT4] diff 分頁出現並成為目前分頁'
    );
    await cdp.waitFor(
      '(() => { var t = document.querySelector(\'#review [role="tab"][data-diff-path="history/ft4-longline.md"]\'); var p = t && document.getElementById(t.getAttribute(\'aria-controls\')); return !!p && !!p.querySelector(\'.diff-text\'); })()',
      5000,
      '[FT4] diff 內容載入完成'
    );

    await cdp.send('Emulation.setDeviceMetricsOverride', { width: 700, height: 900, deviceScaleFactor: 1, mobile: false });
    await sleep(300);

    const measured = await cdp.eval(`(() => {
      var t = document.querySelector('#review [role="tab"][data-diff-path="history/ft4-longline.md"]');
      var p = t && document.getElementById(t.getAttribute('aria-controls'));
      var cells = p ? Array.from(p.querySelectorAll('.diff-text')) : [];
      var cell = cells.find(function (c) { return c.textContent.length >= 300; });
      var lineHeight = cell ? parseFloat(getComputedStyle(cell).lineHeight) : null;
      var cellHeight = cell ? cell.getBoundingClientRect().height : null;
      return {
        cellFound: !!cell,
        lineHeight: lineHeight,
        cellHeight: cellHeight,
        wrapped: !!cell && !!lineHeight && cellHeight > lineHeight * 1.5,
        docScrollWidth: document.documentElement.scrollWidth,
        docClientWidth: document.documentElement.clientWidth,
      };
    })()`);
    check(measured.cellFound, `[FT4] 找到含 300 字元長行的 .diff-text 儲存格（實際 ${JSON.stringify(measured)}）`);
    check(measured.wrapped, `[FT4] 長行在所屬欄內折行（渲染高度 ${measured.cellHeight} > 1.5 倍行高 ${measured.lineHeight}）`);
    check(measured.docScrollWidth <= measured.docClientWidth + 1, `[FT4] 頁面沒有橫向捲軸（documentElement scrollWidth ${measured.docScrollWidth} ≤ clientWidth ${measured.docClientWidth}）`);

    // --- Git Graph 部分：回到預設寬度開分頁（同上，避免 700 寬單欄＋整頁捲動下 pane 列被 sticky
    // 底列擋住），只在量測當下切到 700 寬。---
    await cdp.send('Emulation.clearDeviceMetricsOverride', {});
    await sleep(200);
    ftNeed(await cdp.click('#files-tab-changes'), '[FT4] 點左欄分頁「變更」（準備開 Git Graph）');
    await cdp.waitFor("document.getElementById('files-tab-changes').getAttribute('aria-selected') === 'true'", 3000, '[FT4] 左欄「變更」分頁成為目前分頁');
    await cdp.waitFor('!!document.querySelector(\'#changes-panel [data-action="open-git-graph"]\')', 5000, '[FT4] 「Git Graph」按鈕出現');
    ftNeed(await cdp.click('#changes-panel [data-action="open-git-graph"]'), '[FT4] 點「Git Graph」按鈕');
    await cdp.waitFor(
      '(() => { var t = document.querySelector(\'#review [role="tab"][data-graph-root]\'); return !!t && t.getAttribute(\'aria-selected\') === \'true\'; })()',
      5000,
      '[FT4] Git Graph 分頁出現並成為目前分頁'
    );
    await cdp.waitFor(
      "(() => { var t = document.querySelector('#review [role=\"tab\"][data-graph-root]'); var p = document.getElementById(t.getAttribute('aria-controls')); var subjects = Array.from(p.querySelectorAll('.graph-subject')).slice(0, 16).map(function (s) { return s.textContent; }); return subjects.filter(function (s) { return /^ft4-lane-/.test(s); }).length >= 16; })()",
      5000,
      '[FT4] 16 條車道的 commit 都在最前面 16 列（8 條以上並行車道的前提成立）'
    );

    await cdp.send('Emulation.setDeviceMetricsOverride', { width: 700, height: 900, deviceScaleFactor: 1, mobile: false });
    await sleep(300);

    const graphMeasured = await cdp.eval(`(() => {
      var t = document.querySelector('#review [role="tab"][data-graph-root]');
      var p = document.getElementById(t.getAttribute('aria-controls'));
      var scroller = p.querySelector('.graph-scroll');
      var svgs = Array.from(p.querySelectorAll('.graph-svg')).slice(0, 16);
      var maxSvgWidth = svgs.reduce(function (m, s) { return Math.max(m, Number(s.getAttribute('width')) || 0); }, 0);
      return {
        maxSvgWidth: maxSvgWidth,
        scrollerScrollWidth: scroller ? scroller.scrollWidth : null,
        scrollerClientWidth: scroller ? scroller.clientWidth : null,
        scrollerCanScrollInternally: !!scroller && scroller.scrollWidth > scroller.clientWidth,
        docScrollWidth: document.documentElement.scrollWidth,
        docClientWidth: document.documentElement.clientWidth,
      };
    })()`);
    check(graphMeasured.maxSvgWidth >= 16 * 18, `[FT4] 車道 SVG 寬度反映至少 16 條車道（實際 ${graphMeasured.maxSvgWidth}px）`);
    check(graphMeasured.scrollerCanScrollInternally, `[FT4] Graph 在分頁內容區內部捲動（.graph-scroll 的 scrollWidth ${graphMeasured.scrollerScrollWidth} > clientWidth ${graphMeasured.scrollerClientWidth}）`);
    check(graphMeasured.docScrollWidth <= graphMeasured.docClientWidth + 1, `[FT4] 頁面沒有橫向捲軸（documentElement scrollWidth ${graphMeasured.docScrollWidth} ≤ clientWidth ${graphMeasured.docClientWidth}）`);

    await cdp.send('Emulation.clearDeviceMetricsOverride', {});
  } finally {
    await stopChrome(chrome, 'chrome-FT4');
    await stopPreview(preview, 'preview-FT4');
  }
}

// ---------------------------------------------------------------------------
// main
// ---------------------------------------------------------------------------

const PARTS = [
  ['S1', partSelfTestInjection],
  ['S2', partSelfTestContrast],
  ['S3', partSelfTestHitTest],
  ['S4', partSelfTestFinalSweepOwnership],
  ['S5', partSelfTestFirstProjectionWait],
  ['S6', partSelfTestSegmentArg],
  ['V1', partViewportDesktop],
  ['V2', partViewportMedium],
  ['V3', partViewportNarrow],
  ['V4', partViewportWideShort],
  ['G1', partFactoryFloor],
  ['G2', partFactoryFloorAnimation],
  ['P1', partProjectSwitching],
  ['R1', partTwoRuntimes],
  ['CH1', partChannelFixRound1Regressions],
  ['D1', partDoneColor],
  ['U1', partUnknownAgentStatus],
  ['RM1', partReducedMotion],
  ['FN1', partNoFontRequests],
  ['TK1', partTokenContract],
  ['CT1', partTextContrast],
  ['LO1', partLiveOutputPanel],
  ['CL1', partCssInventory],
  ['DF1', partDeferredItems],
  ['FR1', partFinalReviewFixes],
  // file-review task 3.5（cockpit-dashboard delta；4.x 前端落地前預期 RED，見 FILE_REVIEW_RED_CODES）。
  ['FT1', partManyFileTabs],
  ['FT2', partRepaintKeepsFileTab],
  ['FT3', partMarkdownColors],
  ['FT4', partDiffNoOverflow],
];

// file-review task 3.5 新增、先寫測試的段落（彙總時與既有段落分開計數）。
const FILE_REVIEW_RED_CODES = ['FT1', 'FT2', 'FT3'];

async function main() {
  const segments = parseSegmentArg(SEGMENT_ARG, PARTS.map(([code]) => code));
  if (!segments.ok) {
    console.error(`FAIL ${segments.message}`);
    console.log('RESULT: FAIL (段落代號)');
    process.exitCode = 2;
    return;
  }
  ONLY = segments.codes;
  if (!fs.existsSync(CHROME)) {
    throw new Error(`找不到 Chrome：${CHROME}（可用環境變數 COCKPIT_CHROME 指定路徑）`);
  }
  killLeftovers();
  const segmentFailCounts = [];
  for (const [code, fn] of PARTS) {
    if (!shouldRun(code)) continue;
    const before = failures.length;
    try {
      await fn();
    } catch (e) {
      // FT1–FT3 的前置條件不成立時已經記過一條 FAIL（ftNeed），不再多記「段中止」。
      if (e && e.ftAbort) log(`${code}：前置條件不成立，本段中止`);
      else check(false, `${code} 段中止：${e.message}`);
    }
    segmentFailCounts.push([code, failures.length - before]);
  }
  const beforeFinal = failures.length;
  check(!isPortListening(7770), '結束後 port 7770（ui_preview 預設埠）沒有 LISTENING 的行程');
  finalSweep(); // fix round 1／Codex F3：最後一道防線，見 finalSweep() 上方註解。
  // 段落彙總（file-review task 3.5）：讓控制端一眼分辨「既有段落是否全綠」與「新增的 FT1–FT3 是否依預期 RED」。
  console.log('=== 段落彙總 ===');
  for (const [code, n] of segmentFailCounts) {
    const note = FILE_REVIEW_RED_CODES.includes(code) ? '（file-review task 3.5 新增；4.x 前端落地前預期 RED）' : '';
    console.log(`${(n === 0 ? 'PASS' : `FAIL(${n})`).padEnd(9)} ${code}${note}`);
  }
  const existingFails = segmentFailCounts.filter(([c]) => !FILE_REVIEW_RED_CODES.includes(c)).reduce((sum, [, n]) => sum + n, 0);
  const newFails = segmentFailCounts.filter(([c]) => FILE_REVIEW_RED_CODES.includes(c)).reduce((sum, [, n]) => sum + n, 0);
  console.log(`既有段落 FAIL 數：${existingFails}；FT1–FT3 FAIL 數：${newFails}；收尾 FAIL 數：${failures.length - beforeFinal}`);
  if (failures.length) {
    console.log(`RESULT: FAIL (${failures.length})`);
    process.exitCode = 2;
  } else {
    console.log('RESULT: PASS');
  }
}

main().catch((e) => {
  console.error('FAIL', e);
  process.exitCode = 1;
});
