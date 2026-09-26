// Live Output 面板驗收（spec live-output「輪詢與顯示」「失敗與消失的呈現」；design D8）。
// 寫法比照 docs/research/2026-09-16/actions-check.js：以 node 執行、自己啟動 ui_preview 與
// headless Chrome、收尾時依 PID 收掉兩者，Windows 主控台編碼注意事項同該檔（純 ASCII／CJK
// 走 console.log，不特別處理編碼）。
//
// 腳本分段，每段印 PASS／FAIL 與情境名，最後以非 0 結束碼表示有失敗；task 5.3–5.5 會往後
// 追加更多段落，不動第一段。
//
// 第二段（live-output task 5.3；fix round 1 補三個情境＋R15）：選取 UI 接上之後，改成真的點
// 畫面（CDP Input.dispatchMouseEvent，比照 docs/research/2026-09-16/actions-check.js，不是在
// 頁面裡呼叫 element.click()）。驗 spec live-output「選定一個 pane」的情境：點 pane 列、從
// workstream 選、未綁定的 workstream 沒有入口、選取跨重畫保留、取消選取、改綁模式期間不改變
// 選取（R15：pane 列在改綁模式期間整列不可點選）、面板打開時仍可操作頁面下方的內容、鍵盤選定
// ——以及 cockpit-dashboard「頻繁重畫不影響 Live Output 面板」，外加 exited 的 pane 不可選這個
// 回歸檢查。
//   - C 段：預設推送間隔（2000 ms，不開 COCKPIT_PREVIEW_PUSH_MS），涵蓋點選、workstream
//     選取、未綁定沒入口、exited 不可選、改綁模式期間不改變選取（含 R15 的 cursor 檢查）、
//     鍵盤選定、取消選取。標示用 getComputedStyle 比對（不只比 class），比照 brief 的要求。
//   - D 段：COCKPIT_PREVIEW_PUSH_MS=100，涵蓋「選取跨重畫保留」與「頻繁重畫不影響面板」。
//     比照 actions-check.js「改綁模式跨重畫保留」的作法——記下重畫前的舊節點（#app 底下的
//     選定列），之後斷言它已經不在文件裡（證明 #app 真的被 replaceChildren 換過），同時
//     #output（面板）節點原封不動、捲動位置不變、選定標示仍在同一個 pane 上。
//   - E 段（fix round 1 新增）：面板打開時仍可操作頁面下方的內容——用較矮的視窗
//     （--window-size 高度縮小）讓另一個可選 pane 列原本被面板遮住，捲到面板上方後點它，驗
//     選取確實改變。
//
// fix round 1 的根因調查（控制端 findings 第 1 點）：原本以為「面板打開時 CDP 點不到畫面
// 下半部」是面板（position: sticky; bottom: 0）真的擋住了目標，改成「把該情境挪到面板關閉時
// 做」規避。用 systematic debugging 重新查證（過程見 fix round 1 report）：
//   (1) document.elementsFromPoint(cx, cy) 在失敗座標上查出的完整疊層完全沒有面板或其子元素
//       ——面板在視覺上根本沒蓋住那個像素，直接推翻「面板擋住目標」的假設。
//   (2) 把導致原本失敗的完整點擊序列（選 pane → 從 workstream 選 → 進改綁模式 → 點
//       「綁定到這裡」）抽成獨立的最小重現腳本，反覆跑：不管 COCKPIT_PREVIEW_PUSH_MS 是預設
//       2000 還是拉到 60000（幾乎不會推送），也不管「改綁模式期間 pane 列還留著
//       tabindex／data-action」（模擬修正前）或拿掉（模擬 R15 修正後），失敗率都落在同一個
//       區間（20 次裡 1～2 次），跟面板開關、下半部與否、DOM 巢狀結構都不相關。
//   (3) 在 CDP.click 的 scrollIntoView 之後、送出 Input.dispatchMouseEvent 之前插一段
//       settle delay（80 ms），32 次連續嘗試 0 失敗；不插的對照組 20 次裡失敗 1 次。
// 結論：**這是驗收腳本本身的環境問題，不是產品缺陷**——headless Chrome 在同一個 Runtime.evaluate
// 呼叫裡執行 scrollIntoView 後，若緊接著送 Input.dispatchMouseEvent，合成器（compositor）偶爾
// 還沒吃到新的捲動位置就先處理了滑鼠事件，導致命中失敗；跟本專案的 CSS／DOM 結構無關（真實
// 使用者用真的滑鼠操作不會有這個問題，滑鼠移動與點擊之間天生就有這段 settle 時間）。修法：
// CDP.click 固定在 scrollIntoView 之後等 100 ms 再送滑鼠事件。
//
// 第三段（live-output task 5.4）：spec「輪詢與顯示」剩下四個情境——「內容跟上」（3 秒門檻）、
// 「舊回應不蓋掉新選取」、「往上捲不被拉回」、「停在底部會跟著走」。
//   - F：預設模式（wJ:p1 ticker）。取樣面板最大「line N」隨時間變化，驗「任何 3 秒視窗內都
//     至少前進一次」——這個判準跟 spec「pane 內容改變後 3 秒內反映」等價的理由見 findStalls()
//     上方註解。
//   - G：wJ:p1 覆寫 delay:2500（A），wJ:p3 保持預設 long（B，內容數字範圍跟 A 不重疊）。選
//     A → 在 A 的延遲回應回來前改選 B → 等超過 A 的延遲。用 MutationObserver 把面板標題與
//     內容的每一次變化都推進 window.__history，斷言改選之後的整段歷史（不只看最後一眼）都
//     沒有出現過 A 的內容——才抓得到「中途閃現」。
//   - H：預設模式（wJ:p3 long）。驗「往上捲不被拉回」（中段 scrollTop，等至少 2 次內容更新，
//     scrollTop 不變）。
//   - I：預設模式（wJ:p1 ticker）＋較矮視窗（強迫提早超過一屏）。驗「停在底部會跟著走」
//     （捲到底，等至少 2 次內容更新，仍貼底且 scrollTop 確實變大）。**刻意不跟 H 共用 `long`
//     pane**：`long` 的內容固定顯示最後 200 行、`scrollHeight` 從一開始就不再變化，瀏覽器
//     對「換成等高內容」預設就不會動 `scrollTop`，用它驗「貼底跟著走」會沒有辨識力（即使
//     output.js 完全不做「貼底才跟著捲」，`long` 下 `scrollTop`／`gap` 也會巧合地維持不變）；
//     `ticker` 的內容在測試這幾秒內遠低於 200 行截斷門檻、`scrollHeight` 真的在長大，才測得
//     出「新內容到達後主動捲到底」這個行為，詳見 partScrollBehaviorStickToBottom() 上方註解。
//   - H／I 都用 waitForContentChanges() 以面板文字實際改變兩次為準，不是等固定秒數。
//
// 第四段（live-output task 5.5）：spec「失敗與消失的呈現」三個情境，外加一個補充的邊界情境。
//   - J：COCKPIT_PREVIEW_VANISH_PANE=wJ:p1=3000（wJ:p1 先維持預設 ticker，選取後真的讀到內容
//     才讓它從投影消失）＋COCKPIT_PREVIEW_PUSH_MS=200（縮短推送間隔，消失儘快反映到
//     setKnownPanes）。驗「pane 被關掉」：面板顯示「pane 已不存在」、內容保留但標為過期
//     （direction-01-visual task 4.2／design D7：內容文字 --text-dim、面板左緣 --warn 色條、
//     標題列「過期」文字，見 readStaleSignals()／checkMarkedStale()）、頁面不再請求輸出
//     （5 秒內 output-request 不再增加）。
//   - K：COCKPIT_PREVIEW_OUTPUT_MODES=wJ:p1=notfound（仍在投影中、可點選，但讀取一律
//     404）＋額外疊上 COCKPIT_PREVIEW_VANISH_PANE=wJ:p1=4000（晚到的第二個「pane 已不存在」
//     觸發）。驗「端點回 404」，並用晚到的 VANISH 驗兩條「pane 已不存在」路徑的競態：先到的
//     （404）生效後，晚到的（投影消失）不得重新啟動輪詢或覆蓋已經顯示的訊息——這個觀察視窗
//     天然涵蓋 4 秒的 VANISH 門檻，不需要另外加一段等待。
//   - L：COCKPIT_PREVIEW_OUTPUT_MODES=wJ:p1=fail:3。驗「runtime 斷線後恢復」：503 期間標為
//     過期並顯示原因（回應本體的 error 欄位）、持續依節奏重試；第 4 次讀取恢復後過期標示與
//     原因消失、內容更新為真正的輸出。
//   - M（補充，非 spec 情境之一）：替換 window.fetch 精確控制「成功 → 503 → 內容逐字相同的
//     成功」三次回應，驗「相同內容不重寫 `<pre>`」的最佳化不會連帶略過清除過期標示——這個
//     邊界用 ui_preview 真實的 fail:k 無法乾淨測到（fail:k 一律從第一次讀取就開始算失敗，
//     沒有「先前已有內容」這個狀態，見 L 段落上方註解），改用替換 fetch 換取精確的回應時序，
//     仍然是走真的瀏覽器 DOM／CSS（getComputedStyle），不是比對原始碼字串。
//
// 用法：`node live-output-check.js` 跑全部段落；`node live-output-check.js F,G,H,I` 只跑指定
// 段落（大小寫比對字母；RED 證據用這個旗標縮短單一情境的驗證時間，不需要每次都跑全部）。
//
// 第一段（live-output task 5.2）：選取 UI 尚未接上（那是 5.3），直接呼叫
// design D8 定義的 window.liveOutput.select(runtime, paneId) / clear()。驗 spec
// 「輪詢與顯示」的四個情境：
//   - 「沒有選取就不請求」：開頁面後 10 秒內 preview 的 stdout 沒有任何 output-request 行。
//   - 「請求不堆積」：pane 設 delay:3000，select 後觀察 10 秒，用 stdout 記錄行的時間間隔
//     （約 4 秒＝3 秒延遲＋1 秒輪詢間隔）佐證任一時刻至多一個進行中請求——選這種方式而不是
//     數 CDP Network 事件，因為 output.js 的輪詢核心本身是「同一時間只可能有一個 fetch()
//     在飛」的結構（select／換選取時若已有請求在飛，不會另外起一個，而是等它結束後才補
//     發），用請求出現的時間間隔就足以佐證，不需要額外掛 Network domain。
//   - 「內容不被當成 HTML」：pane 設 html，斷言兩段字樣原樣出現在面板文字中、window.pwned
//     為 undefined、面板內沒有 b 元素。
//   - 「截斷提示」：pane 設 long（wJ:p3 預設），面板頂端出現「更早的輸出未顯示」。task 4.2 fix
//     round 1（Codex medium (1)／設計審核 I1／Ruling R40）在 A 段加驗：截斷提示改用 --text-dim
//     （不再是 --warn）、面板 gap 8px（M2），並疊一次假 503 驗過期原因排在截斷提示之前、兩者顏色
//     分得開。
//
// 第五段（task 4.2 fix round 1；Codex medium (2)／設計審核 M1；控制端 Ruling R41）：
//   - W：`partHelperSelfTest()`——純邏輯自我測試（不需要瀏覽器），證明收緊後的
//     `assessMarkedStale()`／`assessNotStale()` 對錯誤值有辨識力，不是「只要不是某個特定字串就
//     一律算過」的鬆散負向判斷；每個收緊過的判斷都附一個否定對照。
//   - X：`partLongTitleCloseButtonSingleLine()`——長標題（疊上「過期」標籤）時「取消選取」不應該
//     被壓縮成兩行（設計審核 M1）。
//
// 用法（repo 根，需先 `cargo build -p cockpit --example ui_preview`）：
//   node docs/research/2026-09-19/live-output-check.js
// 清理：只終止本腳本自己 spawn 的 ui_preview.exe／chrome.exe（依 PID），沿用
// actions-check.js 的 killTree／tasklist 收尾判準；埠被占用就往上找空埠。
const os = require('node:os');
const { spawn, spawnSync } = require('node:child_process');
const path = require('node:path');
const fs = require('node:fs');

const REPO = path.resolve(__dirname, '..', '..', '..');
const UI_PREVIEW_EXE = path.join(REPO, 'target', 'debug', 'examples', 'ui_preview.exe');
const CHROME =
  process.env.COCKPIT_CHROME || 'C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe';

// 選跑指定段落（`node live-output-check.js F,G,H`）；不帶參數跑全部。task 5.4 加的（見檔頭
// 用法說明），純粹加速 RED 證據的取得，不影響不帶參數時的完整跑法。
const ONLY = process.argv[2] ? process.argv[2].split(',').map((s) => s.trim()) : null;
function shouldRun(letter) {
  return !ONLY || ONLY.includes(letter);
}

const failures = [];
function check(cond, label) {
  console.log(`${cond ? 'ok  ' : 'FAIL'} ${label}`);
  if (!cond) failures.push(label);
}
const log = (s) => console.log(`[${new Date().toISOString()}] ${s}`);
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

// direction-01-visual task 4.1（design D7；live-output delta spec「沒有選取時顯示空狀態」「取消
// 選取」）：面板改為常駐，`getComputedStyle(#output).display` 不再區分「有選取／沒選取」
// ——原本的 `display !== 'none'`（面板打開）在新版恆真、`display === 'none'`（面板收起）恆假，
// 直接留著會讓前者失去辨識力。改用兩個判準，都量「實際有沒有畫出來」（getClientRects 非空，
// 祖先 display:none 時也是空），不是只看 class：
//   - PANEL_OPEN_JS：面板畫出來、帶 `.is-open`（D7：保留為「有選取」的意義）、內容框
//     `.output-text` 與「取消選取」按鈕畫出來、空狀態沒畫出來。
//   - PANEL_EMPTY_JS：面板畫出來、沒有 `.is-open`、空狀態逐字文案畫出來、「取消選取」／標題／
//     內容框都沒畫出來。
// 兩者互斥，腳本裡原本「面板打開」的斷言換成前者、「面板收起」換成後者。
const EMPTY_STATE_TEXT = '還沒選 pane。點 runtime 清單裡的任一列，或按 Factory Floor 列首的「看輸出」。';
const PANEL_SHOWN_FN = `function (n) { return !!n && n.getClientRects().length > 0 && getComputedStyle(n).visibility !== 'hidden'; }`;
const PANEL_OPEN_JS = `(() => {
  var shown = ${PANEL_SHOWN_FN};
  var out = document.getElementById('output');
  return shown(out) && out.classList.contains('is-open') &&
    shown(document.querySelector('#output .output-text')) &&
    shown(document.querySelector('#output .output-close')) &&
    !shown(document.querySelector('#output .output-empty'));
})()`;
const PANEL_EMPTY_JS = `(() => {
  var shown = ${PANEL_SHOWN_FN};
  var out = document.getElementById('output');
  var empty = document.querySelector('#output .output-empty');
  return shown(out) && !out.classList.contains('is-open') &&
    shown(empty) && empty.textContent === ${JSON.stringify(EMPTY_STATE_TEXT)} &&
    !shown(document.querySelector('#output .output-close')) &&
    !shown(document.querySelector('#output .output-title')) &&
    !shown(document.querySelector('#output .output-text'));
})()`;

function pidStillRunning(pid) {
  const r = spawnSync('tasklist', ['/FI', `PID eq ${pid}`, '/NH'], { encoding: 'utf8' });
  return typeof r.stdout === 'string' && r.stdout.includes(String(pid));
}

function killTree(child, label) {
  if (!child || child.exitCode !== null) return;
  spawnSync('taskkill', ['/PID', String(child.pid), '/T', '/F'], { encoding: 'utf8' });
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

// 給 R22（G4 fix wave round 2 Finding A）Network 事件重疊判定用：intervals 是
// [{ start, end }]（CDP `timestamp`，同一個 CDP session 內單調遞增的秒數，不是 wall clock），
// 算出任一時刻「已開始但尚未結束」的區間數上限。時間點相同時視為「先結束才開始」（不計
// 重疊）——用事件本身的 timestamp 排序，不依賴 WS 訊息實際送達 JS 的先後順序：新請求的
// requestWillBeSent 與舊請求的 loadingFailed 有可能在同一個事件迴圈內先後送達 JS，但只要
// 兩者的 timestamp 顯示不重疊，就不該被判定成堆積（也就是本節開頭 brief 要求的「容許以時間戳
// 比較判定，不要因為事件到達順序而假 FAIL」）。
function computeMaxConcurrentIntervals(intervals) {
  const events = [];
  for (const iv of intervals) {
    events.push({ t: iv.start, type: 'start' });
    events.push({ t: iv.end, type: 'end' });
  }
  events.sort((a, b) => a.t - b.t || (a.type === 'end' ? -1 : 1));
  let current = 0;
  let max = 0;
  for (const e of events) {
    if (e.type === 'start') {
      current += 1;
      if (current > max) max = current;
    } else {
      current -= 1;
    }
  }
  return max;
}

class CDP {
  constructor(ws) {
    this.ws = ws;
    this.id = 0;
    this.pending = new Map();
    // 不帶 id 的通知（CDP 事件，例如 Network.* 系列）：{ method, handler } 陣列，見
    // onEvent()（G4 fix wave round 2 Finding A／R22：N 段要驗 Network 事件的請求生命週期）。
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
  // 註冊一個 CDP 事件（不帶 id 的通知）的 handler；同一個 method 可以掛多個，依註冊順序依序
  // 呼叫。目前只有 N 段用它訂閱 Network.* 事件。
  onEvent(method, handler) {
    this.eventHandlers.push({ method, handler });
  }
  async eval(expression) {
    const r = await this.send('Runtime.evaluate', { expression, returnByValue: true });
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
  // 以真的滑鼠事件點 selector 指到的元素中心；holdMs 是按下與放開之間的間隔（task 5.3：
  // 比照 actions-check.js，不用 element.click()，選取也是走 #app 的 pointerdown 委派）。
  //
  // scrollIntoView 之後固定等 SETTLE_AFTER_SCROLL_MS 才送滑鼠事件（fix round 1 根因調查的
  // 結論，見檔頭「fix round 1 的根因調查」）：headless Chrome 在同一個 Runtime.evaluate 呼叫裡
  // 呼叫 scrollIntoView 後，若緊接著在下一個 CDP 呼叫送 Input.dispatchMouseEvent，合成器偶爾
  // 還沒吃到新的捲動位置就處理了滑鼠事件，導致命中失敗（跟頁面內容、CSS、sticky 元素都無關，
  // 純粹是「捲動」與「合成滑鼠事件」這兩個動作之間的時序問題）。沒有這段 delay 時實測落在
  // 5～10% 的失敗率（20 次裡 1～2 次）；加上這段 delay 後 32 次連續嘗試 0 失敗。
  //
  // [Ruling R17] 兩段額外的等待/重試（task 5.4 report 記錄過整支腳本偶爾在 D／E 段出現
  // 「找不到可點的元素：…[data-action="select-bound-pane"]」或「逾時：選取 wJ:p3…」；本檔案
  // 這次改動前實測重現：連續跑 8 次 `node live-output-check.js E`，第 7、8 次都是同一種
  // FAIL——`找不到可點的元素：.ff-row-header[data-workstream="be"] [data-action="select-
  // bound-pane"]`，緊接著「逾時（2000 ms）：選定 wJ:p1，面板打開」）：
  //   1. 找元素本身改成輪詢直到出現或逾時（FIND_TIMEOUT_MS）才 FAIL，不是查一次
  //      `querySelector` 回 `null` 就直接判定找不到——根因是頁面剛連上 `/ws`、第一份投影還
  //      沒畫出來（或正好卡在兩次重畫之間），不是 scrollIntoView 與合成滑鼠事件之間的時序
  //      縫隙（那個已由上面的 settle delay 處理）。
  //   2. 取得座標、settle 之後、送出滑鼠事件之前，再用 `elementFromPoint` 確認該座標上的
  //      元素仍是目標本身或其子孫——`COCKPIT_PREVIEW_PUSH_MS=100` 這類頻繁重畫的段落，
  //      settle 這段時間內頁面可能已經重新佈局，原本算好的座標可能已經指向別的位置；不吻合
  //      就重新查一次目標目前的座標，最多重試 RECHECK_RETRIES 次，不是整段重跑。
  async click(selector, holdMs = 0) {
    const SETTLE_AFTER_SCROLL_MS = 100;
    const FIND_TIMEOUT_MS = 5000;
    const FIND_POLL_MS = 50;
    const RECHECK_RETRIES = 5;
    const RECHECK_INTERVAL_MS = 100;
    const sel = JSON.stringify(selector);

    // (1) 等元素出現（含 scrollIntoView 之後量座標），逾時才判定找不到。
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

    // (2) 送出事件前再確認座標仍落在目標（或其子孫）身上；不吻合就重新查座標重試。
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
  // 以 Input.dispatchKeyEvent 送一個「按下＋放開」的按鍵（task 5.3 fix round 1「鍵盤選定」
  // 情境；windowsVirtualKeyCode 沿用 actions-check.js 的 Enter 按法）。
  async pressKey(key, code, windowsVirtualKeyCode, text) {
    await this.send('Input.dispatchKeyEvent', { type: 'keyDown', key, code, windowsVirtualKeyCode, text });
    await this.send('Input.dispatchKeyEvent', { type: 'keyUp', key, code, windowsVirtualKeyCode });
  }
}

async function startChrome(cdpPort, url, label, windowSize = '1400,1600') {
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
  let page = null;
  for (let i = 0; i < 100 && !page; i++) {
    try {
      const r = await fetch(`http://127.0.0.1:${cdpPort}/json/list`);
      page = (await r.json()).find((t) => t.type === 'page' && t.url.startsWith(url));
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

// 啟動 ui_preview，回傳 { server, port, requests, writeRequests }：requests 是持續累積的
// { runtime, pane, at } 陣列（解析 stdout 的 `output-request <runtime> <pane>` 行）；
// writeRequests 是 { method, path, body } 陣列（解析 `write-request <METHOD> <PATH> <BODY>`
// 行，task 5.3 的「bind-here 不觸發選取」回歸檢查需要確認覆蓋請求真的送出）。
async function startPreview(envOverrides, label) {
  if (!fs.existsSync(UI_PREVIEW_EXE)) {
    throw new Error(`找不到 ${UI_PREVIEW_EXE}，請先跑 cargo build -p cockpit --example ui_preview`);
  }
  const port = pickPort(7793);
  const requests = [];
  const writeRequests = [];
  const server = spawn(UI_PREVIEW_EXE, [], {
    stdio: ['ignore', 'pipe', 'ignore'],
    windowsHide: true,
    env: {
      ...process.env,
      COCKPIT_PREVIEW_LISTEN: `127.0.0.1:${port}`,
      ...envOverrides,
    },
  });
  server.on('error', (e) => check(false, `${label} spawn error：${e.message}`));
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
  if (!up) throw new Error(`${label} 沒有起來`);
  return { server, port, requests, writeRequests };
}

// ---------------------------------------------------------------------------
// 第一段情境（live-output task 5.2）
// ---------------------------------------------------------------------------

// A：預設模式（wJ:p1=ticker、wJ:p3=long、wJ:p2=notfound）。驗「沒有選取就不請求」與
// 「截斷提示」（wJ:p3 的 long 模式一開始就 truncated=true，不需要任何覆寫）。
async function partDefaultModes() {
  log('=== A. 預設模式：沒有選取就不請求 ／ 截斷提示 ===');
  let preview = null;
  let chrome = null;
  try {
    preview = await startPreview({}, 'preview-A');
    const url = `http://127.0.0.1:${preview.port}/`;
    chrome = await startChrome(pickPort(18900, [preview.port]), url, 'chrome-A');
    const { cdp } = chrome;
    await cdp.waitFor(
      "typeof window.liveOutput === 'object' && typeof window.liveOutput.select === 'function'",
      5000,
      'output.js 載入完成，window.liveOutput 就緒'
    );

    // --- 情境：沒有選取就不請求 ---
    log('--- 情境「沒有選取就不請求」（觀察 10 秒）---');
    // direction-01-visual task 4.1：「沒有選取時面板收起」改為常駐空狀態（design D7；spec
    // live-output「沒有選取時顯示空狀態」）。
    check(
      await cdp.eval(PANEL_EMPTY_JS),
      '沒有選取時面板常駐並顯示空狀態（逐字文案、沒有「取消選取」、沒有標題與內容框）'
    );
    await sleep(10000);
    check(
      preview.requests.length === 0,
      `10 秒內服務不應該收到任何輸出端點的請求（實際 ${JSON.stringify(preview.requests)}）`
    );

    // --- 情境：截斷提示 ---
    log('--- 情境「截斷提示」（select wJ:p3，long 模式）---');
    await cdp.eval("window.liveOutput.select('win', 'wJ:p3'); true");
    check(await cdp.eval(PANEL_OPEN_JS), '選取後面板由空狀態改為顯示標題、內容框與「取消選取」');
    await cdp.waitFor(
      "(() => { const n = document.querySelector('.output-truncated-notice'); return !!n && !n.hidden; })()",
      5000,
      '截斷提示變成可見'
    );
    const noticeText = await cdp.eval(
      "document.querySelector('.output-truncated-notice').textContent"
    );
    check(
      noticeText === '更早的輸出未顯示',
      `截斷提示文字應該逐字是「更早的輸出未顯示」（實際 ${JSON.stringify(noticeText)}）`
    );
    const noticeIsAbovePre = await cdp.eval(`(() => {
      const notice = document.querySelector('.output-truncated-notice');
      const pre = document.querySelector('.output-text');
      if (!notice || !pre) return false;
      return notice.compareDocumentPosition(pre) & Node.DOCUMENT_POSITION_FOLLOWING ? true : false;
    })()`);
    check(noticeIsAbovePre, '截斷提示在面板內容（<pre>）之前（「面板頂端」）');
    const preText = await cdp.eval("document.querySelector('.output-text').textContent");
    check(
      typeof preText === 'string' && preText.length > 0,
      `long 模式應該已經寫入內容（實際長度 ${preText ? preText.length : 'n/a'}）`
    );

    // task 4.2 fix round 1（Codex medium (1)／設計審核 I1／控制端 Ruling R40）：截斷提示是常駐
    // 資訊（真機 `read_output` 固定用 `ReadSource::Recent`，`truncated` 幾乎恆為 true），改用
    // --text-dim，不再跟過期色條／標籤／原因訊息一樣用 --warn。
    const truncatedColorNormal = await cdp.eval(
      "getComputedStyle(document.querySelector('.output-truncated-notice')).color"
    );
    check(
      truncatedColorNormal === TEXT_DIM_RGB,
      `截斷提示應該是 --text-dim（常駐資訊，不是警示；design 審核 I1／Ruling R40；實際 ${truncatedColorNormal}）`
    );

    // 設計審核 M2／Ruling R41：面板 gap 6px → 8px，貼底捲動時被捲掉一半的行不再看起來緊貼提示。
    const panelGap = await cdp.eval("getComputedStyle(document.getElementById('output')).rowGap");
    check(panelGap === '8px', `面板內距 gap 應該是 8px（設計審核 M2／Ruling R41；實際 ${panelGap}）`);

    // 設計審核 I1／Ruling R40：截斷提示常駐時疊加一次假的 503，驗過期原因排在截斷提示之前、
    // 兩者顏色分得開（原因 --warn、截斷 --text-dim）。截斷提示這時仍應該可見（不是被過期蓋掉），
    // 也順便讓既有的 checkMarkedStale() 驗到「is-stale 期間、截斷提示與過期原因同時存在」這個
    // 組合沒有互相干擾。
    log('--- 截斷提示常駐期間疊加一次假的 503，驗過期原因排在截斷提示之前（設計審核 I1）---');
    await cdp.eval(`(() => {
      window.__origFetchA = window.fetch;
      var used = false;
      window.fetch = function (input, init) {
        if (!used) {
          used = true;
          return Promise.resolve({
            ok: false,
            status: 503,
            text: () => Promise.resolve(JSON.stringify({ error: 'fix round 1 假 503：驗原因排在截斷提示之前' })),
          });
        }
        return window.__origFetchA(input, init);
      };
      true;
    })()`);
    await cdp.waitFor(
      "(() => { const n = document.querySelector('.output-error-reason'); return !!n && !n.hidden; })()",
      5000,
      '假 503 之後顯示過期原因'
    );
    const orderSignals = await readStaleSignals(cdp);
    checkMarkedStale(orderSignals, '截斷提示常駐期間疊加 503');
    check(
      !(await cdp.eval("document.querySelector('.output-truncated-notice').hidden")),
      '截斷提示應該仍然可見（過期不應該蓋掉常駐的截斷提示）'
    );
    const order = await cdp.eval(`(() => {
      const reason = document.querySelector('.output-error-reason');
      const notice = document.querySelector('.output-truncated-notice');
      if (!reason || !notice) return null;
      return {
        reasonBeforeNoticeInDom: !!(reason.compareDocumentPosition(notice) & Node.DOCUMENT_POSITION_FOLLOWING),
        reasonTop: reason.getBoundingClientRect().top,
        noticeTop: notice.getBoundingClientRect().top,
      };
    })()`);
    check(
      !!order && order.reasonBeforeNoticeInDom && order.reasonTop < order.noticeTop,
      `過期原因應該排在截斷提示之前、視覺位置在它上方（設計審核 I1／Ruling R40；實際 ${JSON.stringify(order)}）`
    );
    await cdp.eval('window.fetch = window.__origFetchA; true');

    await cdp.eval('window.liveOutput.clear(); true');
  } finally {
    await stopChrome(chrome, 'chrome-A');
    if (preview) {
      killTree(preview.server, 'preview-A');
      await sleep(300);
      check(!isPortListening(preview.port), `port ${preview.port}（preview-A）應該不再有 LISTENING 的行程`);
    }
  }
}

// B：wJ:p1 覆寫為 delay:3000、wJ:p2 覆寫為 html。驗「請求不堆積」與「內容不被當成 HTML」。
async function partOverriddenModes() {
  log('=== B. wJ:p1=delay:3000、wJ:p2=html：請求不堆積 ／ 內容不被當成 HTML ===');
  let preview = null;
  let chrome = null;
  try {
    preview = await startPreview(
      { COCKPIT_PREVIEW_OUTPUT_MODES: 'wJ:p1=delay:3000;wJ:p2=html' },
      'preview-B'
    );
    const url = `http://127.0.0.1:${preview.port}/`;
    chrome = await startChrome(pickPort(18910, [preview.port]), url, 'chrome-B');
    const { cdp } = chrome;
    await cdp.waitFor(
      "typeof window.liveOutput === 'object' && typeof window.liveOutput.select === 'function'",
      5000,
      'output.js 載入完成，window.liveOutput 就緒'
    );

    // --- 情境：請求不堆積 ---
    log('--- 情境「請求不堆積」（select wJ:p1，delay:3000，觀察 10 秒）---');
    preview.requests.length = 0;
    await cdp.eval("window.liveOutput.select('win', 'wJ:p1'); true");
    await sleep(10500);
    const p1Requests = preview.requests.filter((r) => r.pane === 'wJ:p1');
    check(
      p1Requests.length >= 2,
      `10.5 秒內至少應該看到 2 筆 wJ:p1 的 output-request（實際 ${p1Requests.length}：${JSON.stringify(p1Requests.map((r) => r.at))}）`
    );
    let minGap = Infinity;
    const gaps = [];
    for (let i = 1; i < p1Requests.length; i += 1) {
      const gap = p1Requests[i].at - p1Requests[i - 1].at;
      gaps.push(gap);
      minGap = Math.min(minGap, gap);
    }
    check(
      gaps.length > 0 && minGap >= 2500,
      `連續兩筆請求的間隔應該接近「3 秒延遲 + 1 秒輪詢間隔」（≥ 2500 ms 佐證沒有堆積，實際間隔 ${JSON.stringify(gaps)}）`
    );

    // --- 情境：內容不被當成 HTML ---
    log('--- 情境「內容不被當成 HTML」（select wJ:p2，html 模式）---');
    await cdp.eval("window.liveOutput.select('win', 'wJ:p2'); true");
    await cdp.waitFor(
      "(() => { const t = document.querySelector('.output-text'); return !!t && t.textContent.indexOf('before') !== -1; })()",
      5000,
      'html 模式的內容已經寫入面板'
    );
    const htmlCheck = await cdp.eval(`(() => {
      const text = document.querySelector('.output-text').textContent;
      return {
        text,
        hasScriptTag: text.indexOf('<script>window.pwned=1</script>') !== -1,
        hasBoldTag: text.indexOf('<b>x</b>') !== -1,
        pwned: typeof window.pwned,
        bElementCount: document.querySelectorAll('#output b').length,
      };
    })()`);
    check(
      htmlCheck.hasScriptTag,
      `面板文字應該原樣含有 <script>window.pwned=1</script>（實際 ${JSON.stringify(htmlCheck.text)}）`
    );
    check(
      htmlCheck.hasBoldTag,
      `面板文字應該原樣含有 <b>x</b>（實際 ${JSON.stringify(htmlCheck.text)}）`
    );
    check(
      htmlCheck.pwned === 'undefined',
      `window.pwned 不應該被設定（實際 typeof 為 ${htmlCheck.pwned}）`
    );
    check(
      htmlCheck.bElementCount === 0,
      `面板內不應該有 b 元素（實際 ${htmlCheck.bElementCount} 個）`
    );
    await cdp.eval('window.liveOutput.clear(); true');
  } finally {
    await stopChrome(chrome, 'chrome-B');
    if (preview) {
      killTree(preview.server, 'preview-B');
      await sleep(300);
      check(!isPortListening(preview.port), `port ${preview.port}（preview-B）應該不再有 LISTENING 的行程`);
    }
  }
}

// ---------------------------------------------------------------------------
// 第二段情境（live-output task 5.3）：選取 UI
// ---------------------------------------------------------------------------

// C：預設模式、預設推送間隔（2000 ms）。驗「點 pane 列」「從 workstream 選」「未綁定的
// workstream 沒有入口」「取消選取」，加兩個回歸：exited 的 pane 不可選、改綁模式下「綁定到
// 這裡」不觸發選取（但仍送出覆蓋）。fixture：project cockpit 的 workstream be 綁 wJ:p1、qa
// 綁 wJ:p3（override）、release 是 unbound（task 5.1 report）。
async function partSelectionBasics() {
  log('=== C. 選取 UI：點 pane 列／從 workstream 選／未綁定沒入口／取消選取／回歸檢查 ===');
  let preview = null;
  let chrome = null;
  try {
    preview = await startPreview({}, 'preview-C');
    const url = `http://127.0.0.1:${preview.port}/`;
    chrome = await startChrome(pickPort(18920, [preview.port]), url, 'chrome-C');
    const { cdp } = chrome;
    await cdp.waitFor(
      "typeof window.liveOutput === 'object' && typeof window.liveOutput.select === 'function'",
      5000,
      'output.js 載入完成，window.liveOutput 就緒'
    );
    await cdp.waitFor(
      "document.querySelectorAll('.pane-row[data-action=\"select-pane\"]').length >= 2",
      5000,
      '畫出可選的 pane 列'
    );

    // --- 情境：未綁定的 workstream 沒有入口 ---
    log('--- 情境「未綁定的 workstream 沒有入口」（release；對照組 be／qa 都有）---');
    const entryPoints = await cdp.eval(`(() => ({
      be: !!document.querySelector('.ff-row-header[data-workstream="be"] [data-action="select-bound-pane"]'),
      qa: !!document.querySelector('.ff-row-header[data-workstream="qa"] [data-action="select-bound-pane"]'),
      release: !!document.querySelector('.ff-row-header[data-workstream="release"] [data-action="select-bound-pane"]'),
    }))()`);
    check(entryPoints.be === true, '已綁定的 be 列首有「看輸出」');
    check(entryPoints.qa === true, '已綁定的 qa 列首有「看輸出」');
    check(entryPoints.release === false, '未綁定的 release 列首沒有「看輸出」');

    // --- 回歸：exited 的 pane 不可選（wJ:p2） ---
    log('--- 回歸「exited 的 pane 不可選」（wJ:p2）---');
    const exitedState = await cdp.eval(`(() => {
      const row = document.querySelector('.pane-row.exited');
      if (!row) return null;
      return { hasAction: row.hasAttribute('data-action'), selectable: row.classList.contains('selectable'), tabIndex: row.tabIndex };
    })()`);
    check(
      exitedState !== null &&
        exitedState.hasAction === false &&
        exitedState.selectable === false &&
        exitedState.tabIndex !== 0,
      `exited 的 pane 列不應該有 data-action／selectable／可聚焦（實際 ${JSON.stringify(exitedState)}）`
    );

    // --- 情境：點 pane 列 ---
    log('--- 情境「點 pane 列」（wJ:p1）---');
    await cdp.click('.pane-row[data-runtime="win"][data-pane="wJ:p1"]');
    await cdp.waitFor(
      "(() => { const r = document.querySelector('.pane-row[data-pane=\"wJ:p1\"]'); return !!r && r.classList.contains('selected'); })()",
      2000,
      '該列出現選定標示（.selected）'
    );
    const title1 = await cdp.eval("document.querySelector('.output-title').textContent");
    check(title1 === 'win / wJ:p1', `面板標題應該顯示 win / wJ:p1（實際 ${JSON.stringify(title1)}）`);
    check(await cdp.eval(PANEL_OPEN_JS), '面板由空狀態改為有選取（標題、內容框、「取消選取」）');
    await cdp.waitFor(
      "(() => { const t = document.querySelector('.output-text'); return !!t && t.textContent.length > 0; })()",
      5000,
      '頁面開始請求該 pane 的輸出（面板已有內容）'
    );
    check(
      preview.requests.some((r) => r.runtime === 'win' && r.pane === 'wJ:p1'),
      `preview 應該收到 wJ:p1 的 output-request（實際 ${JSON.stringify(preview.requests)}）`
    );

    // --- 標示要用 getComputedStyle 比對，不只比 class（brief） ---
    const styleDiff = await cdp.eval(`(() => {
      const selected = document.querySelector('.pane-row[data-pane="wJ:p1"]');
      const other = document.querySelector('.pane-row[data-pane="wJ:p3"]');
      if (!selected || !other) return null;
      const a = getComputedStyle(selected);
      const b = getComputedStyle(other);
      return {
        selectedBg: a.backgroundColor,
        otherBg: b.backgroundColor,
        selectedBorder: a.borderLeftColor,
        otherBorder: b.borderLeftColor,
      };
    })()`);
    check(
      styleDiff !== null &&
        (styleDiff.selectedBg !== styleDiff.otherBg || styleDiff.selectedBorder !== styleDiff.otherBorder),
      `被選定的列應該有可辨識的樣式差異（getComputedStyle，實際 ${JSON.stringify(styleDiff)}）`
    );

    // --- 情境：改綁模式期間不改變選取（fix round 1 新增；R15：pane 列改綁期間整列不可點選）---
    // GIVEN 已選定 wJ:p1 且面板打開（上面「點 pane 列」情境剛做完）；按 be 的「改綁」進入改綁
    // 模式；WHEN 先點 wJ:p3 列上「綁定到這裡」以外的區域，THEN 選取不變；再按該列的「綁定到
    // 這裡」，THEN 送出覆蓋請求、選取仍是 wJ:p1、面板仍打開。另驗改綁模式期間 pane 列沒有可
    // 點選的樣式（cursor 不是 pointer），離開改綁模式後恢復。
    log('--- 情境「改綁模式期間不改變選取」（R15：pane 列改綁期間不可點選）---');
    await cdp.click('[data-action="rebind"][data-project="cockpit"][data-workstream="be"]');
    await cdp.waitFor("!!document.querySelector('.rebind-banner')", 2000, '進入改綁模式（目標 be）');

    const duringRebind = await cdp.eval(`(() => {
      const row = document.querySelector('.pane-row[data-runtime="win"][data-pane="wJ:p3"]');
      return {
        hasDataAction: row.hasAttribute('data-action'),
        selectableClass: row.classList.contains('selectable'),
        cursor: getComputedStyle(row).cursor,
        tabIndex: row.tabIndex,
      };
    })()`);
    check(
      duringRebind.hasDataAction === false && duringRebind.selectableClass === false && duringRebind.tabIndex !== 0,
      `改綁模式期間 wJ:p3 列不應該有 data-action／selectable／可聚焦（R15，實際 ${JSON.stringify(duringRebind)}）`
    );
    check(
      duringRebind.cursor !== 'pointer',
      `改綁模式期間 pane 列不應該呈現可點選的樣式（cursor 不是 pointer，實際 ${JSON.stringify(duringRebind.cursor)}）`
    );

    // 先點「綁定到這裡」以外的區域（pane-title span）——R15 之後這一列根本沒有 data-action，
    // 點它不會有任何 data-action 元素被 closest() 命中，選取應該完全不變。
    await cdp.click('.pane-row[data-runtime="win"][data-pane="wJ:p3"] .pane-title');
    await sleep(300);
    const afterNonButtonClick = await cdp.eval(`(() => ({
      title: document.querySelector('.output-title').textContent,
      p1Selected: document.querySelector('.pane-row[data-pane="wJ:p1"]').classList.contains('selected'),
      p3Selected: document.querySelector('.pane-row[data-pane="wJ:p3"]').classList.contains('selected'),
      panelOpen: ${PANEL_OPEN_JS},
    }))()`);
    check(
      afterNonButtonClick.title === 'win / wJ:p1' &&
        afterNonButtonClick.p1Selected === true &&
        afterNonButtonClick.p3Selected === false &&
        afterNonButtonClick.panelOpen === true,
      `點「綁定到這裡」以外的區域不應該改變選取（仍是 wJ:p1、面板仍打開；實際 ${JSON.stringify(afterNonButtonClick)}）`
    );

    // 再按該列的「綁定到這裡」：送出覆蓋請求，選取仍不變。
    preview.writeRequests.length = 0;
    await cdp.click('[data-action="bind-here"][data-runtime="win"][data-pane="wJ:p3"]');
    await sleep(500);
    check(
      preview.writeRequests.length === 1 &&
        preview.writeRequests[0].method === 'PUT' &&
        preview.writeRequests[0].path === '/api/projects/cockpit/workstreams/be/override',
      `應該收到 PUT /api/projects/cockpit/workstreams/be/override（實際 ${JSON.stringify(preview.writeRequests)}）`
    );
    const afterBindHere = await cdp.eval(`(() => ({
      title: document.querySelector('.output-title').textContent,
      p1Selected: document.querySelector('.pane-row[data-pane="wJ:p1"]').classList.contains('selected'),
      p3Selected: document.querySelector('.pane-row[data-pane="wJ:p3"]').classList.contains('selected'),
      panelOpen: ${PANEL_OPEN_JS},
    }))()`);
    check(
      afterBindHere.title === 'win / wJ:p1' &&
        afterBindHere.p1Selected === true &&
        afterBindHere.p3Selected === false &&
        afterBindHere.panelOpen === true,
      `按「綁定到這裡」送出覆蓋後選取仍應該是 wJ:p1、面板仍打開（實際 ${JSON.stringify(afterBindHere)}）`
    );
    await cdp.waitFor("!document.querySelector('.rebind-banner')", 2000, '覆蓋成功後離開改綁模式');

    const afterRebindExit = await cdp.eval(`(() => {
      const row = document.querySelector('.pane-row[data-runtime="win"][data-pane="wJ:p3"]');
      return { selectableClass: row.classList.contains('selectable'), cursor: getComputedStyle(row).cursor };
    })()`);
    check(
      afterRebindExit.selectableClass === true && afterRebindExit.cursor === 'pointer',
      `離開改綁模式後 pane 列應該恢復可點選（實際 ${JSON.stringify(afterRebindExit)}）`
    );

    // --- 情境：從 workstream 選（qa 綁 wJ:p3，取代原選取） ---
    log('--- 情境「從 workstream 選」（qa 綁 wJ:p3，按「看輸出」）---');
    await cdp.click('.ff-row-header[data-workstream="qa"] [data-action="select-bound-pane"]');
    await cdp.waitFor(
      "document.querySelector('.output-title').textContent === 'win / wJ:p3'",
      2000,
      '面板標題改成 win / wJ:p3（取代原選取）'
    );
    const marks = await cdp.eval(`(() => ({
      p1: document.querySelector('.pane-row[data-pane="wJ:p1"]').classList.contains('selected'),
      p3: document.querySelector('.pane-row[data-pane="wJ:p3"]').classList.contains('selected'),
    }))()`);
    check(marks.p1 === false, '改選 wJ:p3 後，wJ:p1 列的選定標示應該消失');
    check(marks.p3 === true, 'wJ:p3 列出現選定標示');

    // --- 情境：鍵盤選定（fix round 1 新增）---
    // GIVEN 焦點在一個未 exited 的 pane 列上；WHEN 按 Enter；THEN 選定該 pane，面板打開。
    // 目前選取是 wJ:p3；先用 Enter 把 wJ:p1 的列聚焦後選定，再用 Space 選回 wJ:p3，驗兩個鍵都
    // 有效。
    log('--- 情境「鍵盤選定」（Enter／Space）---');
    await cdp.eval(
      "document.querySelector('.pane-row[data-runtime=\"win\"][data-pane=\"wJ:p1\"]').focus(); true"
    );
    check(
      await cdp.eval(
        "document.activeElement && document.activeElement.dataset.pane === 'wJ:p1'"
      ),
      'wJ:p1 的 pane 列可以取得焦點'
    );
    await cdp.pressKey('Enter', 'Enter', 13, '\r');
    await cdp.waitFor(
      "document.querySelector('.output-title').textContent === 'win / wJ:p1'",
      2000,
      'Enter 選定 wJ:p1，面板標題更新'
    );
    check(await cdp.eval(PANEL_OPEN_JS), 'Enter 選定後面板顯示有選取的內容（不是空狀態）');
    check(
      await cdp.eval(
        "document.querySelector('.pane-row[data-pane=\"wJ:p1\"]').classList.contains('selected')"
      ),
      'Enter 選定後該列出現選定標示'
    );

    await cdp.eval(
      "document.querySelector('.pane-row[data-runtime=\"win\"][data-pane=\"wJ:p3\"]').focus(); true"
    );
    await cdp.pressKey(' ', 'Space', 32, ' ');
    await cdp.waitFor(
      "document.querySelector('.output-title').textContent === 'win / wJ:p3'",
      2000,
      'Space 選定 wJ:p3，面板標題更新'
    );
    check(
      await cdp.eval(
        "document.querySelector('.pane-row[data-pane=\"wJ:p3\"]').classList.contains('selected') && !document.querySelector('.pane-row[data-pane=\"wJ:p1\"]').classList.contains('selected')"
      ),
      'Space 選定後 wJ:p3 有標示、wJ:p1 標示消失'
    );

    // exited 的 pane 列不可聚焦：tabIndex 不是 0（前面「exited 的 pane 不可選」情境已經斷言過
    // 沒有 data-action／selectable／tabIndex===0，這裡不重複）。

    // --- 情境：取消選取 ---
    // direction-01-visual task 4.1：按鈕改名「取消選取」（live-output delta spec），按下後回到
    // 常駐空狀態，不再是「面板收起」（design D7）。
    log('--- 情境「取消選取」（按面板「取消選取」）---');
    check(
      (await cdp.eval("document.querySelector('.output-close').textContent")) === '取消選取',
      '面板上的按鈕文字是「取消選取」'
    );
    await cdp.eval("document.querySelector('.output-close').click(); true");
    await cdp.waitFor(PANEL_EMPTY_JS, 2000, '面板回到空狀態');
    check(
      (await cdp.eval("document.querySelectorAll('.pane-row.selected').length")) === 0,
      '取消選取後沒有任何 pane 列有選定標示'
    );
    // 按「取消選取」前一刻剛好有一個輪詢請求已經送出、還沒到達伺服器，屬於正常現象，不代表
    // 取消之後又發了新請求。先等超過一個輪詢間隔（1 秒）讓這種已飛出的請求落地、清空記錄，再開
    // 一段乾淨的觀察視窗才是「取消之後有沒有『新』請求」的正確驗法。
    await sleep(1500);
    preview.requests.length = 0;
    await sleep(2500);
    check(
      preview.requests.length === 0,
      `取消選取後（排除取消當下已在飛的請求）不應該再有新的輸出請求（實際 ${JSON.stringify(preview.requests)}）`
    );
  } finally {
    await stopChrome(chrome, 'chrome-C');
    if (preview) {
      killTree(preview.server, 'preview-C');
      await sleep(300);
      check(!isPortListening(preview.port), `port ${preview.port}（preview-C）應該不再有 LISTENING 的行程`);
    }
  }
}

// D：COCKPIT_PREVIEW_PUSH_MS=100。驗「選取跨重畫保留」與「頻繁重畫不影響 Live Output 面板」：
// 比照 actions-check.js「改綁模式跨重畫保留」的作法，記下重畫前 #app 底下的舊選定列節點，之後
// 斷言它已經不在文件裡（證明 #app 真的被 replaceChildren 換過），同時 #output（面板）節點
// 原封不動、捲動位置不變、選定標示仍在同一個 pane 上。
async function partFrequentRepaint() {
  log('=== D. COCKPIT_PREVIEW_PUSH_MS=100：選取跨重畫保留／頻繁重畫不影響面板 ===');
  let preview = null;
  let chrome = null;
  try {
    preview = await startPreview({ COCKPIT_PREVIEW_PUSH_MS: '100' }, 'preview-D');
    const url = `http://127.0.0.1:${preview.port}/`;
    chrome = await startChrome(pickPort(18930, [preview.port]), url, 'chrome-D');
    const { cdp } = chrome;
    await cdp.waitFor(
      "typeof window.liveOutput === 'object' && typeof window.liveOutput.select === 'function'",
      5000,
      'output.js 載入完成，window.liveOutput 就緒'
    );

    await cdp.click('.pane-row[data-runtime="win"][data-pane="wJ:p3"]');
    await cdp.waitFor(
      "(() => { const t = document.querySelector('.output-text'); return !!t && t.textContent.length > 0; })()",
      5000,
      '選取 wJ:p3（long 模式）後面板已有內容'
    );

    // 模擬使用者已往上捲：long 模式一開始就 300+ 行，內容應該已經超過一屏。捲到「非零、非
    // 貼底」的中段值（scrollHeight 的三分之一），不是 0——fix round 1 findings：先前拿 0 當
    // before 基準值沒有辨識力（0 剛好也是很多「沒真的往上捲」的錯誤實作會呈現的值），改成一個
    // 具體、非零、非貼底的中段值，並在斷言前印出來，之後 3 秒才比對是否真的維持在這個值。
    const scrollSetup = await cdp.eval(`(() => {
      const pre = document.querySelector('.output-text');
      const overflowed = pre.scrollHeight > pre.clientHeight;
      const mid = Math.round((pre.scrollHeight - pre.clientHeight) / 3);
      pre.scrollTop = mid;
      const maxScrollTop = pre.scrollHeight - pre.clientHeight;
      return {
        overflowed,
        scrollHeight: pre.scrollHeight,
        clientHeight: pre.clientHeight,
        maxScrollTop,
        appliedScrollTop: pre.scrollTop,
      };
    })()`);
    check(scrollSetup.overflowed === true, 'long 模式的內容應該超過一屏（可捲動，才能驗「已往上捲」）');
    check(
      scrollSetup.appliedScrollTop > 0 && scrollSetup.appliedScrollTop < scrollSetup.maxScrollTop,
      `捲動位置應該落在非零、非貼底的中段（實際 ${JSON.stringify(scrollSetup)}）`
    );

    const before = await cdp.eval(`(() => {
      window.__outputNode = document.getElementById('output');
      window.__oldSelectedRow = document.querySelector('.pane-row.selected');
      return {
        version: Number(document.getElementById('version').textContent.slice(1)),
        scrollTop: document.querySelector('.output-text').scrollTop,
        markedOutput: !!window.__outputNode,
        markedRow: !!window.__oldSelectedRow,
      };
    })()`);
    check(
      before.markedOutput && before.markedRow && before.scrollTop > 0,
      `記下面板節點與目前選定列的節點供之後比對，捲動位置應該是非零的中段值（實際 ${JSON.stringify(before)}）`
    );

    await sleep(3000);

    const after = await cdp.eval(`(() => {
      const row = document.querySelector('.pane-row[data-pane="wJ:p3"]');
      return {
        version: Number(document.getElementById('version').textContent.slice(1)),
        scrollTop: document.querySelector('.output-text').scrollTop,
        sameOutputNode: document.getElementById('output') === window.__outputNode,
        oldRowGone: !document.contains(window.__oldSelectedRow),
        stillSelected: !!row && row.classList.contains('selected'),
        title: document.querySelector('.output-title').textContent,
      };
    })()`);

    check(
      after.version >= before.version + 20,
      `3 秒內（100 ms 推送）version 應該至少前進 20（${before.version} → ${after.version}）`
    );
    check(after.oldRowGone, '舊的選定列節點已經不在文件中（#app 真的被 replaceChildren 換過）');
    check(after.stillSelected, '重畫後仍然是同一個 pane（wJ:p3）有選定標示');
    check(after.title === 'win / wJ:p3', `重畫後面板標題仍是 win / wJ:p3（實際 ${JSON.stringify(after.title)}）`);
    check(after.sameOutputNode, '面板（#output）的 DOM 節點沒有被換掉');
    check(
      after.scrollTop === before.scrollTop && after.scrollTop > 0,
      `面板捲動位置不變且非零（before ${before.scrollTop} → after ${after.scrollTop}）`
    );

    await cdp.eval('window.liveOutput.clear(); true');
  } finally {
    await stopChrome(chrome, 'chrome-D');
    if (preview) {
      killTree(preview.server, 'preview-D');
      await sleep(300);
      check(!isPortListening(preview.port), `port ${preview.port}（preview-D）應該不再有 LISTENING 的行程`);
    }
  }
}

// E（fix round 1 新增；[Ruling R16] 收緊 GIVEN、補「畫面操作」按鈕仍可操作的斷言）：面板打開
// 時仍可操作頁面下方的內容。用比較矮的視窗（--window-size=1400,800）讓 Factory Floor
// （兩個 Project）撐滿視窗，runtime 卡的 pane 列自然落在下面一屏，選取後面板打開（最高佔視窗
// 下方 45vh）也不會讓它變得「看得到又點得到」——驗收重點是：不需要碰程式碼，光靠 CDP.click
// 既有的 scrollIntoView 行為，把原本在畫面外的 pane 列捲到面板上方後仍然點得到、選取確實改變。
//
// [Ruling R16] GIVEN 收緊：原本用 `belowFold || overlapsPanel` 判定，控制端複檢指出實測走的
// 是 `belowFold`（目標整個在視窗外），沒有真的驗到 spec 寫的「目前被面板遮住」——加大視窗高度
// 到能讓 wJ:p3 列一開始就落在視窗內但仍與面板 rect 重疊都不會發生，因為目前視窗高度
// （--window-size=1400,800）配合 Factory Floor 的內容量，wJ:p3 列此時整個在視窗下緣之外
// （見下方 GIVEN 印出的 targetRect.top >= innerHeight）。改法：選定後先程式化捲動到「wJ:p3
// 列的中心點落在面板 rect 中間」這個位置，直接斷言 `overlapsPanel` 為真，並用
// `elementFromPoint` 在目標中心點確認最上層元素是面板本身（`closest('#output')` 非空）而不是
// 目標，才是真的「被面板遮住」；再執行 WHEN（`CDP.click` 內建的 scrollIntoView 把它捲到面板
// 上方後點擊）。
//
// [Ruling R16] 補一個斷言：面板打開時，頁面上其餘「畫面操作」的按鈕仍可操作——按 be-1 節點的
// 「推進」，preview stdout 應該收到對應的 `write-request POST /api/projects/cockpit/tasks/
// be-1/advance`（task id `be-1` 沿用 docs/research/2026-09-16/actions-check.js 既有 fixture）。
async function partPanelOverlap() {
  log('=== E. 面板打開時仍可操作頁面下方的內容（命中測試；direction-01-visual task 2.1 改寫）===');
  let preview = null;
  let chrome = null;
  try {
    preview = await startPreview({}, 'preview-E');
    const url = `http://127.0.0.1:${preview.port}/`;
    // direction-01-visual task 2.1 fix round 1／Codex C2：spec live-output「面板打開時仍可
    // 操作頁面下方的內容」GIVEN 明定「視窗 1536×1024」，不是原本的 1400,800——用回 spec 的
    // 精確視窗尺寸，也是桌面固定一屏（design D3）跟 V1 用的同一個尺寸。
    chrome = await startChrome(pickPort(18940, [preview.port]), url, 'chrome-E', '1536,1024');
    const { cdp } = chrome;
    await cdp.waitFor(
      "typeof window.liveOutput === 'object' && typeof window.liveOutput.select === 'function'",
      5000,
      'output.js 載入完成，window.liveOutput 就緒'
    );

    // 用 Factory Floor 裡 be 的「看輸出」選定 wJ:p1：這顆按鈕在第一個 Project 區塊的最上方，
    // 選取之後畫面仍停在頁面最上緣，不會意外把 runtime 卡捲進視窗。
    await cdp.click('.ff-row-header[data-workstream="be"] [data-action="select-bound-pane"]');
    await cdp.waitFor(
      "document.querySelector('.output-title').textContent === 'win / wJ:p1'",
      2000,
      '選定 wJ:p1，面板打開'
    );
    check(
      await cdp.eval(PANEL_OPEN_JS),
      '面板應該是有選取的狀態（標題、內容框、「取消選取」，不是空狀態）'
    );

    // --- GIVEN／WHEN（direction-01-visual task 2.1 fix round 1／Codex C2）：spec 原文逐字是
    // 「對每個可選的 pane 列、每個『畫面操作』按鈕與每個 Project 項目，取其可見範圍中心點做
    // 命中測試」——不是只驗一筆事件列。改成真的列舉當下所有可見的目標，對每一個都先斷言「有
    // 非零的可見交集」（precondition，避免「反正本來就看不到」的假通過），再在可見範圍中心
    // `elementFromPoint`，斷言命中的是自己或子孫，不是 `#output` 面板。命中斷言前完全不呼叫
    // `cdp.click`（它內建 `scrollIntoView`，Codex 的原始 finding 正是點出這一點：click helper
    // 會把元素捲到看得到的地方，可能剛好把「命中測試當下沒被面板蓋住」這件事變得恆真，掩蓋
    // 「本來可見範圍中心其實被蓋住」的真違規）。「可見範圍」跟 visual-check.js 的
    // `hitCenter()`（fix round 1／規格對照 S3）用同一套邏輯：從元素本身的 rect 開始，往上走
    // 訪每一層祖先，只要祖先的 `overflow-x`／`overflow-y` 不是 `visible` 就用它的 rect 跟目前
    // 的交集框再取一次交集（不只是跟視窗取交集）——Factory Floor／runtime 清單／最近事件現在
    // 各自有自己的內層捲動容器（design 審核 I2），沒被選到的內容會被自己的捲動容器裁掉，不能
    // 只跟整個視窗比。
    const hitTest = await cdp.eval(`(() => {
      function visibleBox(n) {
        var r = n.getBoundingClientRect();
        var left = r.left, top = r.top, right = r.right, bottom = r.bottom;
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
      function describe(el) {
        if (!el) return null;
        return el.tagName + (el.id ? '#' + el.id : '') + (el.className ? '.' + String(el.className).trim().split(/\\s+/).join('.') : '');
      }
      var targets = [];
      document.querySelectorAll('.pane-row[data-action="select-pane"]').forEach(function (n, i) {
        targets.push({ kind: 'pane-row#' + i + ' (' + n.dataset.runtime + '/' + n.dataset.pane + ')', el: n });
      });
      document.querySelectorAll('.action-button').forEach(function (n, i) {
        targets.push({ kind: 'action-button#' + i + ' (' + (n.dataset.action || '') + ')', el: n });
      });
      document.querySelectorAll('[data-action="select-project"]').forEach(function (n, i) {
        targets.push({ kind: 'project-item#' + i, el: n });
      });
      var results = targets.map(function (t) {
        var box = visibleBox(t.el);
        var visible = box.right > box.left && box.bottom > box.top;
        if (!visible) {
          return { kind: t.kind, visible: false, selfHit: null, box: box };
        }
        var cx = (box.left + box.right) / 2;
        var cy = (box.top + box.bottom) / 2;
        var hit = document.elementFromPoint(cx, cy);
        return {
          kind: t.kind,
          visible: true,
          box: box,
          selfHit: !!hit && (hit === t.el || t.el.contains(hit)),
          hitIsPanel: !!hit && !!hit.closest('#output'),
          hitDesc: describe(hit),
        };
      });
      return {
        total: results.length,
        visibleCount: results.filter(function (r) { return r.visible; }).length,
        blocked: results.filter(function (r) { return r.visible && r.selfHit === false; }),
        blockedByPanel: results.filter(function (r) { return r.visible && r.selfHit === false && r.hitIsPanel; }),
        results: results,
      };
    })()`);
    log(`GIVEN／WHEN（命中測試，逐一列舉；direction-01-visual task 2.1 fix round 1）：共 ${hitTest.total} 個目標，${hitTest.visibleCount} 個有非零可見交集；${JSON.stringify(hitTest.results)}`);
    check(
      hitTest.total > 0 && hitTest.visibleCount > 0,
      `應該至少找得到一個可選 pane 列／畫面操作按鈕（precondition，不然下面的命中測試沒有對象；實際 total=${hitTest.total} visible=${hitTest.visibleCount}）`
    );
    check(
      hitTest.blocked.length === 0,
      `每個可選 pane 列／畫面操作按鈕／Project 項目的可見範圍中心點都應該命中自己（不是別的元素，尤其不是 Live Output 面板）；被蓋住的（共 ${hitTest.blocked.length} 個）：${JSON.stringify(hitTest.blocked)}`
    );

    // --- THEN：點另一個 pane 列即改為選取該 pane（spec 原文；用右欄 wJ:p3，上面的命中測試已經
    // 證明它在命中斷言階段沒有被蓋住，這裡才用 cdp.click——click 內建的 scrollIntoView 不影響
    // 已經做完的命中測試）---
    await cdp.click('.pane-row[data-runtime="win"][data-pane="wJ:p3"]');
    await cdp.waitFor(
      "document.querySelector('.output-title').textContent === 'win / wJ:p3'",
      2000,
      '點擊 wJ:p3，選取改變、面板標題更新'
    );
    check(
      (await cdp.eval(
        "document.querySelector('.pane-row[data-pane=\"wJ:p3\"]').classList.contains('selected')"
      )),
      'wJ:p3 列出現選定標示'
    );
    check(
      await cdp.eval(PANEL_OPEN_JS),
      '面板仍然是有選取的狀態（不是空狀態）'
    );

    // --- [Ruling R16] 面板打開時，頁面上其餘「畫面操作」的按鈕仍可操作（按 be-1 的「推進」）---
    preview.writeRequests.length = 0;
    await cdp.click('[data-action="advance"][data-project="cockpit"][data-task="be-1"]');
    await sleep(500);
    check(
      preview.writeRequests.some(
        (r) => r.method === 'POST' && r.path === '/api/projects/cockpit/tasks/be-1/advance'
      ),
      `面板打開時仍應該能操作「推進」按鈕（應該收到 POST /api/projects/cockpit/tasks/be-1/advance，實際 ${JSON.stringify(preview.writeRequests)}）`
    );
  } finally {
    await stopChrome(chrome, 'chrome-E');
    if (preview) {
      killTree(preview.server, 'preview-E');
      await sleep(300);
      check(!isPortListening(preview.port), `port ${preview.port}（preview-E）應該不再有 LISTENING 的行程`);
    }
  }
}

// ---------------------------------------------------------------------------
// 第三段情境（live-output task 5.4）：顯示時序與捲動
// ---------------------------------------------------------------------------

// 解析面板文字裡最大的「line N」（Growing 模式的內容固定是這個格式，見 ui_preview.rs 的
// growing_output）；找不到任何 "line N" 回 -1。
function parseMaxLine(text) {
  const re = /line (\d+)/g;
  let max = -1;
  let m;
  while ((m = re.exec(text || ''))) {
    const n = Number(m[1]);
    if (n > max) max = n;
  }
  return max;
}

// 在 durationMs 內每隔 intervalMs 取樣一次面板最大行號，回傳 {t, max} 陣列（t 為取樣當下的
// Date.now()，不是固定間隔——取樣本身有 CDP 往返延遲，用實際時間戳記比對才準）。
async function sampleMaxLine(cdp, durationMs, intervalMs) {
  const start = Date.now();
  const samples = [];
  while (Date.now() - start < durationMs) {
    const text = await cdp.eval("document.querySelector('.output-text').textContent");
    samples.push({ t: Date.now(), max: parseMaxLine(text) });
    await sleep(intervalMs);
  }
  return samples;
}

// 「內容跟上」的等價判準：ui_preview 的 Growing 模式用 started.elapsed().as_secs() 算總行數
// ——內容精確地每 1000 ms 前進一行，是伺服器端的確定性行為，不是機率性的。因此「每一行在產生
// 後 3 秒內出現在面板」等價於「任何長度達 windowMs（3000）的觀察視窗內，面板顯示的最大行號
// 至少前進一次」：
//   - 若某個 >=3000 ms 的視窗完全沒有前進，視窗開始那一刻已經產生的那一行，到視窗結束時仍未
//     顯示，落後時間 >= windowMs = 3000 ms，直接違反 spec 的 3 秒門檻。
//   - 反之若 3 秒門檻成立，視窗內任何一行最晚在產生後 3 秒內顯示；由於伺服器每 1 秒就產生
//     新的一行，一個 >=3000 ms 的視窗裡一定包含「視窗開始前產生、視窗開始後 3 秒內應該顯示」
//     的行，該行必定會在視窗結束前顯示，最大行號因而前進。
// 對每個取樣點 i，找第一個時間差 >= windowMs 的取樣點 j；若該區間內 max 沒有前進即為一個違規
// （尾端不足一個完整視窗的取樣點會被跳過，不算違規，只是觀察期已經結束）。
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
      violations.push({
        fromT: samples[i].t,
        fromMax: samples[i].max,
        toT: samples[j].t,
        toMax: samples[j].max,
      });
    }
  }
  return violations;
}

// 等待面板文字「實際改變」count 次（不是等固定秒數）；逾時回 timedOut: true。
async function waitForContentChanges(cdp, count, timeoutMs) {
  const start = Date.now();
  let prev = await cdp.eval("document.querySelector('.output-text').textContent");
  let changes = 0;
  while (Date.now() - start < timeoutMs && changes < count) {
    await sleep(200);
    const cur = await cdp.eval("document.querySelector('.output-text').textContent");
    if (cur !== prev) {
      changes += 1;
      prev = cur;
    }
  }
  return { changes, timedOut: changes < count };
}

// A（delay 模式，start_lines=0）的內容特徵：測試期間總行數遠小於 200，不會被截斷，第一行
// 永遠是「line 1」；B（wJ:p3 預設 long，start_lines=300）不管什麼時候讀，畫面上最小的行號都
// 遠大於 1（見 ui_preview.rs growing_output：first_line = total_lines.saturating_sub(200) + 1，
// total_lines >= 301 時 first_line >= 102）。用「整行剛好是 line 1」而不是子字串比對，避免誤配
// 「line 15」「line 100」這種含有 "line 1" 子字串的行。
const A_CONTENT_PATTERN = /(^|\n)line 1(\n|$)/;

// F：預設模式（wJ:p1=ticker）。驗「內容跟上」（3 秒門檻）。
async function partContentCatchesUp() {
  log('=== F. 內容跟上（3 秒門檻，wJ:p1 ticker）===');
  let preview = null;
  let chrome = null;
  try {
    preview = await startPreview({}, 'preview-F');
    const url = `http://127.0.0.1:${preview.port}/`;
    chrome = await startChrome(pickPort(18950, [preview.port]), url, 'chrome-F');
    const { cdp } = chrome;
    await cdp.waitFor(
      "typeof window.liveOutput === 'object' && typeof window.liveOutput.select === 'function'",
      5000,
      'output.js 載入完成，window.liveOutput 就緒'
    );
    await cdp.eval("window.liveOutput.select('win', 'wJ:p1'); true");
    await cdp.waitFor(
      "(() => { const t = document.querySelector('.output-text'); return !!t && t.textContent.length > 0; })()",
      5000,
      '選取後面板已有初始內容'
    );

    log('--- 取樣 8 秒：面板最大行號隨時間變化 ---');
    const samples = await sampleMaxLine(cdp, 8000, 250);
    log(`取樣點數：${samples.length}；最大行號序列：${JSON.stringify(samples.map((s) => s.max))}`);
    const violations = findStalls(samples, 3000);
    check(
      violations.length === 0,
      `任何 >=3 秒的觀察視窗內面板最大行號都應該至少前進一次（實際違規 ${violations.length} 個：${JSON.stringify(violations)}）`
    );
    await cdp.eval('window.liveOutput.clear(); true');
  } finally {
    await stopChrome(chrome, 'chrome-F');
    if (preview) {
      killTree(preview.server, 'preview-F');
      await sleep(300);
      check(!isPortListening(preview.port), `port ${preview.port}（preview-F）應該不再有 LISTENING 的行程`);
    }
  }
}

// G：wJ:p1 覆寫為 delay:2500（A），wJ:p3 保持預設 long（B，立即回應）。驗「舊回應不蓋掉新
// 選取」：選 A → 在 A 的延遲回應回來前改選 B → 等超過 A 的延遲 → 斷言改選之後的整段歷史（不
// 只看最後一眼）都沒有出現過 A 的內容，標題全程是 B。
//
// round 2 Finding B 註記：這裡走真端點、原生 fetch，A 的請求在改選當下就被 abort——它真正在
// 驗的是「abort 之後 A 的回應不會出現」這條路徑（跟 N 段是同一種佐證），onPollSettled 開頭的
// `gen !== generation` 世代序號比對在這個情境下多半根本不會被執行到（reject 分支在
// `controller.signal.aborted` 為真時直接 return，不呼叫 onPollSettled）。世代序號檢查本身
// （abort 之外、回應仍然「成功」落地時的那道防線）改由 Q 段（忽略 AbortSignal 的假 fetch）
// 獨立驗證。G 段的斷言不需要改，繼續作為「abort 路徑」與使用者可觀察行為（舊回應不能污染新
// 選取）的迴歸測試。
async function partOldResponseNotOverwrite() {
  log('=== G. wJ:p1=delay:2500：舊回應不蓋掉新選取 ===');
  let preview = null;
  let chrome = null;
  try {
    preview = await startPreview({ COCKPIT_PREVIEW_OUTPUT_MODES: 'wJ:p1=delay:2500' }, 'preview-G');
    const url = `http://127.0.0.1:${preview.port}/`;
    chrome = await startChrome(pickPort(18960, [preview.port]), url, 'chrome-G');
    const { cdp } = chrome;
    await cdp.waitFor(
      "typeof window.liveOutput === 'object' && typeof window.liveOutput.select === 'function'",
      5000,
      'output.js 載入完成，window.liveOutput 就緒'
    );

    // 掛 MutationObserver：面板標題與內容任何一次變化都推進 window.__history（{t, title,
    // text}），才能抓到「中途閃現」，不能只在最後看一眼。
    await cdp.eval(`(() => {
      window.__history = [];
      const titleEl = document.querySelector('.output-title');
      const preEl = document.querySelector('.output-text');
      const push = () => window.__history.push({ t: Date.now(), title: titleEl.textContent, text: preEl.textContent });
      const observer = new MutationObserver(push);
      observer.observe(document.getElementById('output'), { childList: true, characterData: true, subtree: true });
      push();
      true;
    })()`);

    log('--- 選 A（wJ:p1，delay:2500）---');
    await cdp.eval("window.liveOutput.select('win', 'wJ:p1'); true");
    await sleep(400); // 遠小於 2500 ms，確保 A 的請求已送出、還在飛（尚未回應）。
    check(
      preview.requests.some((r) => r.pane === 'wJ:p1'),
      'GIVEN：A（wJ:p1）的輸出請求應該已經送出'
    );
    const switchAt = Date.now();
    log('--- 在 A 回應回來前改選 B（wJ:p3，long，立即回應）---');
    await cdp.eval("window.liveOutput.select('win', 'wJ:p3'); true");
    await cdp.waitFor(
      "document.querySelector('.output-title').textContent === 'win / wJ:p3'",
      2000,
      '標題立刻更新為 win / wJ:p3'
    );

    // 等超過 A 的延遲（2500 ms）＋安全邊界，確保 A 的回應一定已經落地（不論被丟棄或誤套用）。
    await sleep(3200);

    const history = await cdp.eval('window.__history');
    log(`歷史筆數：${history.length}`);
    const postSwitch = history.filter((h) => h.t >= switchAt);
    const leaked = postSwitch.filter((h) => A_CONTENT_PATTERN.test(h.text));
    check(
      leaked.length === 0,
      `改選 B 之後的歷史裡不應該出現 A 的內容（實際 ${leaked.length}／${postSwitch.length} 筆含 A 內容：${JSON.stringify(leaked)}）`
    );
    check(
      postSwitch.length > 0 && postSwitch.every((h) => h.title === 'win / wJ:p3'),
      `改選 B 之後的歷史裡標題應該全程是 win / wJ:p3（實際 ${JSON.stringify(postSwitch.map((h) => h.title))}）`
    );

    const finalTitle = await cdp.eval("document.querySelector('.output-title').textContent");
    const finalText = await cdp.eval("document.querySelector('.output-text').textContent");
    check(
      finalTitle === 'win / wJ:p3' && !A_CONTENT_PATTERN.test(finalText),
      `最終面板應該是 B 的內容（實際標題 ${JSON.stringify(finalTitle)}，含 A 內容＝${A_CONTENT_PATTERN.test(finalText)}）`
    );
  } finally {
    await stopChrome(chrome, 'chrome-G');
    if (preview) {
      killTree(preview.server, 'preview-G');
      await sleep(300);
      check(!isPortListening(preview.port), `port ${preview.port}（preview-G）應該不再有 LISTENING 的行程`);
    }
  }
}

// H：預設模式（wJ:p3=long）。驗「往上捲不被拉回」：`long` 一開始就 300+ 行（truncated=true，
// 固定顯示最後 200 行），立刻就超過一屏，不用等內容長大——用來驗「已經在中段的捲動位置不會被
// 拉走」很合適且快。
async function partScrollBehaviorUpNotPulledBack() {
  log('=== H. 往上捲不被拉回（wJ:p3 long）===');
  let preview = null;
  let chrome = null;
  try {
    preview = await startPreview({}, 'preview-H');
    const url = `http://127.0.0.1:${preview.port}/`;
    chrome = await startChrome(pickPort(18970, [preview.port]), url, 'chrome-H');
    const { cdp } = chrome;
    await cdp.waitFor(
      "typeof window.liveOutput === 'object' && typeof window.liveOutput.select === 'function'",
      5000,
      'output.js 載入完成，window.liveOutput 就緒'
    );
    await cdp.eval("window.liveOutput.select('win', 'wJ:p3'); true");
    await cdp.waitFor(
      "(() => { const t = document.querySelector('.output-text'); return !!t && t.textContent.length > 0; })()",
      5000,
      '選取後面板已有初始內容（long 模式）'
    );

    log('--- 情境「往上捲不被拉回」（中段 scrollTop，等至少 2 次內容更新）---');
    const scrollSetup = await cdp.eval(`(() => {
      const pre = document.querySelector('.output-text');
      const overflowed = pre.scrollHeight > pre.clientHeight;
      const mid = Math.round((pre.scrollHeight - pre.clientHeight) / 3);
      pre.scrollTop = mid;
      return { overflowed, appliedScrollTop: pre.scrollTop, maxScrollTop: pre.scrollHeight - pre.clientHeight };
    })()`);
    check(scrollSetup.overflowed === true, 'long 模式的內容應該超過一屏（可捲動，才能驗「已往上捲」）');
    check(
      scrollSetup.appliedScrollTop > 0 && scrollSetup.appliedScrollTop < scrollSetup.maxScrollTop,
      `捲動位置應該落在非零、非貼底的中段（實際 ${JSON.stringify(scrollSetup)}）`
    );
    const beforeUp = scrollSetup.appliedScrollTop;
    const upResult = await waitForContentChanges(cdp, 2, 10000);
    check(!upResult.timedOut, `等待內容至少更新 2 次不應該逾時（實際 ${upResult.changes} 次）`);
    const afterUpScrollTop = await cdp.eval("document.querySelector('.output-text').scrollTop");
    check(
      afterUpScrollTop === beforeUp,
      `至少 2 次內容更新後，捲動位置應該不變（before ${beforeUp} → after ${afterUpScrollTop}）`
    );

    await cdp.eval('window.liveOutput.clear(); true');
  } finally {
    await stopChrome(chrome, 'chrome-H');
    if (preview) {
      killTree(preview.server, 'preview-H');
      await sleep(300);
      check(!isPortListening(preview.port), `port ${preview.port}（preview-H）應該不再有 LISTENING 的行程`);
    }
  }
}

// I：`ticker`（wJ:p1，預設）＋較矮的視窗（`--window-size=1400,500`，`.output-text` 的
// `max-height: 40vh` 讓它在這個視窗高度下只要 8~10 行內容就會超過一屏）。驗「停在底部會跟著
// 走」——刻意不用 `long`：`long` 的內容固定顯示最後 200 行、總高度（`scrollHeight`）從一開始
// 就不再變化（見 ui_preview.rs growing_output：`truncated=true` 之後每次回應永遠正好 200
// 行），瀏覽器對「textContent 換成等高的新內容」預設就不會動 `scrollTop`，所以就算 output.js
// 完全不做「貼底才跟著捲」這個邏輯，`long` 模式下 `scrollTop` 也會巧合地維持不變、`gap`
// 巧合地維持 0——這個情境的驗收就會沒有辨識力（改壞 output.js 也測不出來）。`ticker` 在測試
// 這幾秒內總行數遠低於 200 行截斷門檻，內容持續淨增加、`scrollHeight` 真的在長大，才能真正驗
// 到「新內容到達後主動捲到底」這個行為，也才能讓 `scrollTop` 有意義地「確實變大」。
async function partScrollBehaviorStickToBottom() {
  log('=== I. 停在底部會跟著走（wJ:p1 ticker，較矮視窗強迫提早超過一屏）===');
  let preview = null;
  let chrome = null;
  try {
    preview = await startPreview({}, 'preview-I');
    const url = `http://127.0.0.1:${preview.port}/`;
    chrome = await startChrome(pickPort(18980, [preview.port]), url, 'chrome-I', '1400,500');
    const { cdp } = chrome;
    await cdp.waitFor(
      "typeof window.liveOutput === 'object' && typeof window.liveOutput.select === 'function'",
      5000,
      'output.js 載入完成，window.liveOutput 就緒'
    );
    await cdp.eval("window.liveOutput.select('win', 'wJ:p1'); true");
    await cdp.waitFor(
      "(() => { const t = document.querySelector('.output-text'); return !!t && t.textContent.length > 0; })()",
      5000,
      '選取後面板已有初始內容（ticker 模式）'
    );

    log('--- 等待內容超過一屏（較矮視窗，ticker 每秒多一行）---');
    const overflowed = await cdp.waitFor(
      "(() => { const pre = document.querySelector('.output-text'); return pre.scrollHeight > pre.clientHeight; })()",
      20000,
      '內容應該在 20 秒內超過一屏（可捲動）'
    );
    if (!overflowed) return;

    log('--- 情境「停在底部會跟著走」（捲到底，等至少 2 次內容更新）---');
    const bottomSetup = await cdp.eval(`(() => {
      const pre = document.querySelector('.output-text');
      pre.scrollTop = pre.scrollHeight;
      return { scrollTop: pre.scrollTop, scrollHeight: pre.scrollHeight, gap: pre.scrollHeight - pre.scrollTop - pre.clientHeight };
    })()`);
    check(bottomSetup.gap < 6, `捲到底後應該貼底（gap < 6px，實際 ${bottomSetup.gap}）`);
    const beforeBottomScrollTop = bottomSetup.scrollTop;
    const bottomResult = await waitForContentChanges(cdp, 2, 10000);
    check(!bottomResult.timedOut, `等待內容至少更新 2 次不應該逾時（實際 ${bottomResult.changes} 次）`);
    const afterBottom = await cdp.eval(`(() => {
      const pre = document.querySelector('.output-text');
      return { scrollTop: pre.scrollTop, scrollHeight: pre.scrollHeight, gap: pre.scrollHeight - pre.scrollTop - pre.clientHeight };
    })()`);
    check(afterBottom.gap < 6, `更新後仍應該貼底（gap < 6px，實際 ${afterBottom.gap}）`);
    check(
      afterBottom.scrollHeight > bottomSetup.scrollHeight,
      `內容應該真的長大了（scrollHeight before ${bottomSetup.scrollHeight} → after ${afterBottom.scrollHeight}），驗「跟著走」才有意義`
    );
    check(
      afterBottom.scrollTop > beforeBottomScrollTop,
      `貼底時新內容到達後 scrollTop 應該變大（確實跟著走；before ${beforeBottomScrollTop} → after ${afterBottom.scrollTop}）`
    );

    await cdp.eval('window.liveOutput.clear(); true');
  } finally {
    await stopChrome(chrome, 'chrome-I');
    if (preview) {
      killTree(preview.server, 'preview-I');
      await sleep(300);
      check(!isPortListening(preview.port), `port ${preview.port}（preview-I）應該不再有 LISTENING 的行程`);
    }
  }
}

// ---------------------------------------------------------------------------
// 第四段情境（live-output task 5.5）：失敗與消失的呈現
// ---------------------------------------------------------------------------

// 「停止輪詢」的判定（brief 精確值逐字要求）：某個 pane 的 output-request 記錄行在 5 秒內
// 不再增加。
async function assertPollingStopped(preview, pane, label) {
  const before = preview.requests.filter((r) => r.pane === pane).length;
  await sleep(5000);
  const after = preview.requests.filter((r) => r.pane === pane).length;
  check(after === before, `${label}（5 秒內 output-request 不應該再增加，實際 ${before} → ${after}）`);
  return after;
}

// 過期標示的計算值（direction-01-visual task 4.2；design D7）：J／K／L／M 四段原本各自比對
// `.output-text` 的 opacity（現行 `.is-stale` 做法），D7 拿掉 opacity（會把文字對比拉到 4.5:1
// 以下），改成幾個獨立的計算值——內容文字色（正常 `--text`／過期 `--text-dim`）、面板左緣
// `--warn` 色條（inset box-shadow，正常是 `none`）、標題列「過期」文字（`.output-stale-label`
// 的 `hidden`／文字）、`is-stale` class 本身、可見的原因訊息或「pane 已不存在」提示的顏色。
// 這裡一次讀完，取代原本單一的 opacity 讀取，J／K／L／M 每一處原本「有 opacity 差異」
// 「opacity 變回正常值」的比對都各自拆成獨立斷言，不放寬——見下面各段落。
//
// task 4.2 fix round 1（Codex medium (2)；控制端 Ruling R41）：round 0 的 `checkNotStale()` 只驗
// `textColor !== TEXT_DIM_RGB`——任何錯誤色甚至 `null` 都會判定「不是過期」而通過，也完全沒用到
// `readStaleSignals()` 已經讀出來的 `isStale` 欄位；`checkMarkedStale()` 也沒有驗過可見原因訊息
// 或「pane 已不存在」提示本身的顏色。改成 `assessNotStale()`／`assessMarkedStale()`：回傳「哪裡
// 不對」的問題清單（純函式，不呼叫 `check()`），`checkNotStale()`／`checkMarkedStale()` 只是薄
// 包裝；純邏輯、不需要瀏覽器的自我測試見下方 `partHelperSelfTest()`（W 段）。
const TEXT_RGB = 'rgb(229, 237, 243)'; // --text: #e5edf3（正常內容文字色）
const TEXT_DIM_RGB = 'rgb(163, 183, 201)'; // --text-dim: #a3b7c9（過期內容文字色；fix round 1 起也是截斷提示的顏色）
const WARN_RGB = 'rgb(233, 188, 115)'; // --warn: #e9bc73（面板左緣色條／可見的原因訊息）
const BAD_RGB = 'rgb(244, 114, 121)'; // --bad: #f47279（「pane 已不存在」提示，fix round 1 未變）

async function readStaleSignals(cdp) {
  return cdp.eval(`(() => {
    var out = document.getElementById('output');
    var text = document.querySelector('.output-text');
    var label = document.querySelector('.output-stale-label');
    var reason = document.querySelector('.output-error-reason');
    var gone = document.querySelector('.output-gone-notice');
    return {
      isStale: !!out && out.classList.contains('is-stale'),
      textColor: text ? getComputedStyle(text).color : null,
      panelBoxShadow: out ? getComputedStyle(out).boxShadow : null,
      labelHidden: label ? label.hidden : null,
      labelText: label ? label.textContent : null,
      reasonHidden: reason ? reason.hidden : null,
      reasonColor: reason ? getComputedStyle(reason).color : null,
      goneHidden: gone ? gone.hidden : null,
      goneColor: gone ? getComputedStyle(gone).color : null,
    };
  })()`);
}

// 過期態應該成立的一切（fix round 1 收緊）：`is-stale` class 本身存在；內容文字精確等於
// `--text-dim`；面板左緣精確有 `--warn` 的 inset box-shadow；標題列「過期」標籤可見且文字正確；
// 「可見的原因訊息或『pane 已不存在』提示，其中之一必須可見，且顏色精確等於該狀態的 token」
// （Codex medium (2)：「過期態……讀可見原因訊息的 computed color 驗證為 --warn（gone 情境則驗
// gone 提示）」）——兩者都隱藏（過期卻沒有任何說明）或顏色錯誤都算問題。
function assessMarkedStale(signals) {
  const problems = [];
  if (signals.isStale !== true) problems.push(`isStale 應該是 true，實際 ${signals.isStale}`);
  if (signals.textColor !== TEXT_DIM_RGB) problems.push(`textColor 應該是 --text-dim（${TEXT_DIM_RGB}），實際 ${signals.textColor}`);
  if (
    !(
      signals.panelBoxShadow &&
      signals.panelBoxShadow !== 'none' &&
      signals.panelBoxShadow.indexOf(WARN_RGB) !== -1 &&
      signals.panelBoxShadow.indexOf('inset') !== -1
    )
  ) {
    problems.push(`panelBoxShadow 應該含 --warn 的 inset 色條，實際 ${signals.panelBoxShadow}`);
  }
  if (!(signals.labelHidden === false && signals.labelText === '過期')) {
    problems.push(`標題列「過期」標籤應該可見，實際 hidden=${signals.labelHidden} text=${JSON.stringify(signals.labelText)}`);
  }
  if (signals.reasonHidden === false) {
    if (signals.reasonColor !== WARN_RGB) {
      problems.push(`可見的原因訊息應該是 --warn（${WARN_RGB}），實際 ${signals.reasonColor}`);
    }
  } else if (signals.goneHidden === false) {
    if (signals.goneColor !== BAD_RGB) {
      problems.push(`「pane 已不存在」提示應該是 --bad（${BAD_RGB}），實際 ${signals.goneColor}`);
    }
  } else {
    problems.push('過期時原因訊息與「pane 已不存在」提示應該至少有一個可見，實際兩者都隱藏');
  }
  return problems;
}

// 正常／恢復態應該成立的一切（fix round 1 收緊）：`is-stale` class 不存在；內容文字精確等於
// `--text`（不是「不等於 --text-dim」這種任何值都能通過的負向判斷——Codex medium (2) 抓到的
// 缺口本身）；面板左緣沒有色條；標題列「過期」標籤隱藏。
function assessNotStale(signals) {
  const problems = [];
  if (signals.isStale !== false) problems.push(`isStale 應該是 false，實際 ${signals.isStale}`);
  if (signals.textColor !== TEXT_RGB) problems.push(`textColor 應該是 --text（${TEXT_RGB}），實際 ${signals.textColor}`);
  if (signals.panelBoxShadow !== 'none') problems.push(`panelBoxShadow 應該是 none，實際 ${signals.panelBoxShadow}`);
  if (signals.labelHidden !== true) problems.push(`標題列「過期」標籤應該隱藏，實際 hidden=${signals.labelHidden}`);
  return problems;
}

function checkMarkedStale(signals, label) {
  const problems = assessMarkedStale(signals);
  check(problems.length === 0, `${label}：應該標為過期（${problems.length === 0 ? '通過' : problems.join('；')}）`);
}

function checkNotStale(signals, label) {
  const problems = assessNotStale(signals);
  check(problems.length === 0, `${label}：不應該標為過期（${problems.length === 0 ? '通過' : problems.join('；')}）`);
}

// W（fix round 1；Codex medium (2)）：純邏輯自我測試，不需要瀏覽器——證明 assessMarkedStale／
// assessNotStale 對錯誤值有辨識力，不是「只要不是某個特定字串就一律算過」的鬆散負向判斷。round 0
// 的 checkNotStale() 只驗 `textColor !== TEXT_DIM_RGB`：任何錯誤色、甚至 null，都會被誤判成
// 「不是過期」而通過；checkMarkedStale() 完全沒有讀 isStale、也沒有驗過可見原因訊息／gone 提示
// 的顏色。這裡對每個收緊過的判斷各給一個否定對照，逐一證明新版能抓到 round 0 版本會放過的錯誤。
function partHelperSelfTest() {
  log('=== W. 自我測試：assessNotStale／assessMarkedStale 對錯誤值有辨識力 ===');

  const goodNormal = { isStale: false, textColor: TEXT_RGB, panelBoxShadow: 'none', labelHidden: true, labelText: '' };
  check(assessNotStale(goodNormal).length === 0, '合法的正常 signals 應該通過 assessNotStale（0 個問題）');

  const wrongColorNormal = { ...goodNormal, textColor: 'rgb(1, 2, 3)' };
  check(
    assessNotStale(wrongColorNormal).some((p) => p.indexOf('textColor') !== -1),
    '否定對照：內容文字色跑掉、但不是 --text-dim（例如 rgb(1,2,3)）時，assessNotStale 應該抓到——round 0 的 `!== TEXT_DIM_RGB` 對這個值會誤判成正常'
  );

  const nullColorNormal = { ...goodNormal, textColor: null };
  check(
    assessNotStale(nullColorNormal).some((p) => p.indexOf('textColor') !== -1),
    '否定對照：textColor 為 null 時 assessNotStale 應該抓到——round 0 的 `!== TEXT_DIM_RGB` 對 null 一樣會誤判成正常'
  );

  const staleClassNotCleared = { ...goodNormal, isStale: true };
  check(
    assessNotStale(staleClassNotCleared).some((p) => p.indexOf('isStale') !== -1),
    '否定對照：is-stale class 沒有拿掉時 assessNotStale 應該抓到——round 0 完全沒有用到 isStale 欄位'
  );

  const goodStaleWithReason = {
    isStale: true,
    textColor: TEXT_DIM_RGB,
    panelBoxShadow: `${WARN_RGB} 2px 0px 0px 0px inset`,
    labelHidden: false,
    labelText: '過期',
    reasonHidden: false,
    reasonColor: WARN_RGB,
    goneHidden: true,
    goneColor: null,
  };
  check(assessMarkedStale(goodStaleWithReason).length === 0, '合法的過期（503，顯示原因）signals 應該通過 assessMarkedStale（0 個問題）');

  const wrongReasonColor = { ...goodStaleWithReason, reasonColor: 'rgb(9, 9, 9)' };
  check(
    assessMarkedStale(wrongReasonColor).some((p) => p.indexOf('原因訊息') !== -1),
    '否定對照：可見原因訊息顏色不是 --warn 時 assessMarkedStale 應該抓到——round 0 完全不讀這個欄位'
  );

  const isStaleFalseButRestLooksStale = { ...goodStaleWithReason, isStale: false };
  check(
    assessMarkedStale(isStaleFalseButRestLooksStale).some((p) => p.indexOf('isStale') !== -1),
    '否定對照：is-stale class 沒有加上時 assessMarkedStale 應該抓到——round 0 完全沒有用到 isStale 欄位'
  );

  const goodStaleGone = {
    isStale: true,
    textColor: TEXT_DIM_RGB,
    panelBoxShadow: `${WARN_RGB} 2px 0px 0px 0px inset`,
    labelHidden: false,
    labelText: '過期',
    reasonHidden: true,
    reasonColor: null,
    goneHidden: false,
    goneColor: BAD_RGB,
  };
  check(assessMarkedStale(goodStaleGone).length === 0, '合法的過期（gone）signals 應該通過 assessMarkedStale（0 個問題）');

  const wrongGoneColor = { ...goodStaleGone, goneColor: WARN_RGB };
  check(
    assessMarkedStale(wrongGoneColor).some((p) => p.indexOf('pane 已不存在') !== -1),
    '否定對照：「pane 已不存在」提示顏色不是 --bad 時 assessMarkedStale 應該抓到'
  );

  const neitherVisible = { ...goodStaleWithReason, reasonHidden: true, goneHidden: true };
  check(
    assessMarkedStale(neitherVisible).some((p) => p.indexOf('兩者都隱藏') !== -1),
    '否定對照：原因訊息與 gone 提示都隱藏時（過期卻沒有任何說明）assessMarkedStale 應該抓到'
  );
}

// J：wJ:p1 維持預設 ticker（先讀得到內容），3 秒後從推送投影中消失（COCKPIT_PREVIEW_PUSH_MS
// 縮到 200 ms，讓消失儘快反映）。驗 spec 情境「pane 被關掉」。
async function partPaneVanishes() {
  log('=== J. COCKPIT_PREVIEW_VANISH_PANE=wJ:p1=3000：pane 被關掉 ===');
  let preview = null;
  let chrome = null;
  try {
    preview = await startPreview(
      { COCKPIT_PREVIEW_PUSH_MS: '200', COCKPIT_PREVIEW_VANISH_PANE: 'wJ:p1=3000' },
      'preview-J'
    );
    const url = `http://127.0.0.1:${preview.port}/`;
    chrome = await startChrome(pickPort(18990, [preview.port]), url, 'chrome-J');
    const { cdp } = chrome;
    await cdp.waitFor(
      "typeof window.liveOutput === 'object' && typeof window.liveOutput.select === 'function'",
      5000,
      'output.js 載入完成，window.liveOutput 就緒'
    );

    await cdp.eval("window.liveOutput.select('win', 'wJ:p1'); true");
    await cdp.waitFor(
      "(() => { const t = document.querySelector('.output-text'); return !!t && t.textContent.length > 0; })()",
      5000,
      'GIVEN：選取後面板已有內容（消失之前）'
    );
    const beforeSignals = await readStaleSignals(cdp);
    checkNotStale(beforeSignals, 'GIVEN：選取後、消失之前');

    log('--- 等待 wJ:p1 從投影中消失（3 秒後，推送間隔 200 ms）---');
    await cdp.waitFor(
      "(() => { const n = document.querySelector('.output-gone-notice'); return !!n && !n.hidden; })()",
      6000,
      '面板顯示「pane 已不存在」'
    );
    const goneText = await cdp.eval("document.querySelector('.output-gone-notice').textContent");
    check(
      goneText === 'pane 已不存在',
      `「pane 已不存在」文字應該逐字一致（實際 ${JSON.stringify(goneText)}）`
    );

    const afterSignals = await readStaleSignals(cdp);
    checkMarkedStale(afterSignals, 'pane 被關掉之後');

    // 「保留最後一份文字」不是跟消失之前很早的一個快照逐字相同（ticker 在真正消失之前仍然
    // 健康、內容持續在長大，兩個時間點的內容本來就會不一樣，不是缺陷）；驗的是「gone 之後
    // 內容不再變化」——先記下 gone 當下的內容，過完 assertPollingStopped 的 5 秒觀察視窗
    // （頁面已經停止輪詢）再讀一次，應該逐字相同、且非空。
    const contentAtGone = await cdp.eval("document.querySelector('.output-text').textContent");
    check(contentAtGone.length > 0, `gone 當下內容不應該是空的（實際長度 ${contentAtGone.length}）`);

    await assertPollingStopped(preview, 'wJ:p1', '頁面不應該再請求輸出');

    const contentAfterStopped = await cdp.eval("document.querySelector('.output-text').textContent");
    check(
      contentAfterStopped === contentAtGone,
      `停止輪詢後內容應該維持凍結、不再變化（gone 當下 ${JSON.stringify(contentAtGone)} → 5 秒後 ${JSON.stringify(contentAfterStopped)}）`
    );
  } finally {
    await stopChrome(chrome, 'chrome-J');
    if (preview) {
      killTree(preview.server, 'preview-J');
      await sleep(300);
      check(!isPortListening(preview.port), `port ${preview.port}（preview-J）應該不再有 LISTENING 的行程`);
    }
  }
}

// K：wJ:p1 覆寫為 notfound（仍在投影中、可點選，但讀取一律 404），額外疊上
// COCKPIT_PREVIEW_VANISH_PANE=wJ:p1=4000（晚到的第二個「pane 已不存在」觸發）。驗 spec 情境
// 「端點回 404」，順便驗兩條「pane 已不存在」路徑的競態（見檔頭第四段說明）。
async function partOutputEndpoint404() {
  log('=== K. wJ:p1=notfound（+ VANISH 晚到）：端點回 404 ===');
  let preview = null;
  let chrome = null;
  try {
    preview = await startPreview(
      {
        COCKPIT_PREVIEW_OUTPUT_MODES: 'wJ:p1=notfound',
        COCKPIT_PREVIEW_PUSH_MS: '200',
        COCKPIT_PREVIEW_VANISH_PANE: 'wJ:p1=4000',
      },
      'preview-K'
    );
    const url = `http://127.0.0.1:${preview.port}/`;
    chrome = await startChrome(pickPort(19000, [preview.port]), url, 'chrome-K');
    const { cdp } = chrome;
    await cdp.waitFor(
      "typeof window.liveOutput === 'object' && typeof window.liveOutput.select === 'function'",
      5000,
      'output.js 載入完成，window.liveOutput 就緒'
    );

    await cdp.eval("window.liveOutput.select('win', 'wJ:p1'); true");
    await cdp.waitFor(
      "(() => { const n = document.querySelector('.output-gone-notice'); return !!n && !n.hidden; })()",
      3000,
      '端點回 404 後面板顯示「pane 已不存在」'
    );
    const goneText = await cdp.eval("document.querySelector('.output-gone-notice').textContent");
    check(
      goneText === 'pane 已不存在',
      `「pane 已不存在」文字應該逐字一致（實際 ${JSON.stringify(goneText)}）`
    );
    const titleAfterGone = await cdp.eval("document.querySelector('.output-title').textContent");
    check(
      titleAfterGone === 'win / wJ:p1',
      `面板標題應該仍顯示 win / wJ:p1（實際 ${JSON.stringify(titleAfterGone)}）`
    );
    const signalsAfterGone = await readStaleSignals(cdp);
    checkMarkedStale(signalsAfterGone, '端點回 404 之後');

    // 這個 5 秒觀察視窗會涵蓋 VANISH_PANE 的 4 秒門檻（見上方檔頭「第四段」說明），順便驗證
    // 「投影消失」這個晚到的觸發沒有讓輪詢重新啟動。
    await assertPollingStopped(preview, 'wJ:p1', '停止輪詢後不應該再請求輸出');

    const goneTextAfterVanishWindow = await cdp.eval(
      "document.querySelector('.output-gone-notice').textContent"
    );
    const goneHiddenAfterVanishWindow = await cdp.eval(
      "document.querySelector('.output-gone-notice').hidden"
    );
    check(
      goneHiddenAfterVanishWindow === false && goneTextAfterVanishWindow === 'pane 已不存在',
      `晚到的「投影消失」觸發不應該改變已經顯示的訊息（實際 hidden=${goneHiddenAfterVanishWindow}，文字=${JSON.stringify(goneTextAfterVanishWindow)}）`
    );
  } finally {
    await stopChrome(chrome, 'chrome-K');
    if (preview) {
      killTree(preview.server, 'preview-K');
      await sleep(300);
      check(!isPortListening(preview.port), `port ${preview.port}（preview-K）應該不再有 LISTENING 的行程`);
    }
  }
}

// L：wJ:p1 覆寫為 fail:3（前 3 次讀取回 503，第 4 次起恢復成 ticker）。驗 spec 情境
// 「runtime 斷線後恢復」。
//
// 注：ui_preview 的 FailThenRecover 一律從「第一次讀取」就開始算失敗次數（見 ui_preview.rs
// read_output），沒有辦法讓同一個 pane 先成功一次再開始失敗——面板在失敗期間顯示的是「還沒有
// 任何內容」而不是「先前的內容」，跟真實情境（先前已經看到內容，runtime 才斷線）不完全一樣，
// 但完整覆蓋 spec THEN 要求的每一句：過期標示／顯示原因／持續重試／恢復後清除／內容更新。
// 「相同內容不重寫 <pre> 時過期標示仍要消失」這個更窄的邊界情境改在 M 段用假 fetch 直接驗
// （見該段落上方註解）。
async function partRuntimeRecovers() {
  log('=== L. wJ:p1=fail:3：runtime 斷線後恢復 ===');
  let preview = null;
  let chrome = null;
  try {
    preview = await startPreview({ COCKPIT_PREVIEW_OUTPUT_MODES: 'wJ:p1=fail:3' }, 'preview-L');
    const url = `http://127.0.0.1:${preview.port}/`;
    chrome = await startChrome(pickPort(19010, [preview.port]), url, 'chrome-L');
    const { cdp } = chrome;
    await cdp.waitFor(
      "typeof window.liveOutput === 'object' && typeof window.liveOutput.select === 'function'",
      5000,
      'output.js 載入完成，window.liveOutput 就緒'
    );

    await cdp.eval("window.liveOutput.select('win', 'wJ:p1'); true");

    log('--- 503 期間：過期標示／顯示原因／持續重試 ---');
    await cdp.waitFor(
      "(() => { const n = document.querySelector('.output-error-reason'); return !!n && !n.hidden; })()",
      3000,
      '503 後面板顯示失敗原因'
    );
    const reasonText = await cdp.eval("document.querySelector('.output-error-reason').textContent");
    check(
      /^ui_preview 模擬斷線/.test(reasonText),
      `原因文字應該取自回應本體的 error 欄位（實際 ${JSON.stringify(reasonText)}）`
    );
    const staleSignals = await readStaleSignals(cdp);
    checkMarkedStale(staleSignals, '503 期間');
    check(
      await cdp.eval("document.querySelector('.output-gone-notice').hidden"),
      '503 不是「pane 已不存在」，不應該顯示 gone 提示'
    );

    const requestsBeforeRetry = preview.requests.filter((r) => r.pane === 'wJ:p1').length;
    await sleep(2200);
    const requestsAfterRetry = preview.requests.filter((r) => r.pane === 'wJ:p1').length;
    check(
      requestsAfterRetry > requestsBeforeRetry,
      `503 期間應該依節奏繼續重試（實際 ${requestsBeforeRetry} → ${requestsAfterRetry}）`
    );

    log('--- 恢復（第 4 次讀取成功）：過期標示與原因消失、內容更新 ---');
    await cdp.waitFor(
      "(() => { const n = document.querySelector('.output-error-reason'); return !!n && n.hidden; })()",
      8000,
      '恢復後過期原因消失'
    );
    const recoveredSignals = await readStaleSignals(cdp);
    checkNotStale(recoveredSignals, '恢復後（L 段）');
    const recoveredText = await cdp.eval("document.querySelector('.output-text').textContent");
    check(
      /line \d+/.test(recoveredText),
      `恢復後內容應該更新為真正的輸出（實際 ${JSON.stringify(recoveredText)}）`
    );
  } finally {
    await stopChrome(chrome, 'chrome-L');
    if (preview) {
      killTree(preview.server, 'preview-L');
      await sleep(300);
      check(!isPortListening(preview.port), `port ${preview.port}（preview-L）應該不再有 LISTENING 的行程`);
    }
  }
}

// M（補充，非 spec 情境之一，是 brief「精確值」的一個更窄邊界）：直接在頁面裡替換
// window.fetch，精確控制三次回應（成功 → 503 → 內容逐字相同的成功），驗證「相同內容不重寫
// <pre>」的最佳化（writeText 提早 return）不會連帶略過清除過期標示。這個邊界用 ui_preview
// 真實的 fail:k 模式無法乾淨測到（見 L 段落上方註解），改用替換 fetch 精確控制回應序列——仍然
// 是走真的瀏覽器 DOM／CSS（getComputedStyle），不是比對原始碼字串。
//
// 選 wJ:p1（預設 ticker，真的存在於投影裡）而不是虛構的 pane id：preview 仍然按預設間隔
// （2000 ms）推送真實投影，`render.js` 的 `paint()` 每次都會呼叫
// `window.liveOutput.setKnownPanes(...)`；選一個真的存在的 pane 才不會被這個背景推送誤判成
// 「pane 已不存在」而觸發 markGone()（踩過的坑：一開始選了不存在的 'fake' pane id，2 秒後
// 背景推送的 setKnownPanes 判定它不在投影裡，呼叫 markGone() 把過期標示／原因整個蓋過去，
// 跟這個情境要測的「fetch 回應序列」完全無關）。fetch 被完全接管，實際打到 preview 的只有
// WebSocket 那個真實的投影推送，output 端點的請求則完全走假 fetch，不會真的碰到伺服器。
async function partSameContentClearsStale() {
  log('=== M. 相同內容恢復也要清除過期標示（替換 window.fetch 精確控制回應序列）===');
  let preview = null;
  let chrome = null;
  try {
    preview = await startPreview({}, 'preview-M');
    const url = `http://127.0.0.1:${preview.port}/`;
    chrome = await startChrome(pickPort(19020, [preview.port]), url, 'chrome-M');
    const { cdp } = chrome;
    await cdp.waitFor(
      "typeof window.liveOutput === 'object' && typeof window.liveOutput.select === 'function'",
      5000,
      'output.js 載入完成，window.liveOutput 就緒'
    );

    await cdp.eval(`(() => {
      window.__fetchCalls = 0;
      const responses = [
        { ok: true, status: 200, body: JSON.stringify({ runtime: 'win', pane_id: 'wJ:p1', format: 'text', text: 'stable content', truncated: false }) },
        { ok: false, status: 503, body: JSON.stringify({ error: 'boom' }) },
        { ok: true, status: 200, body: JSON.stringify({ runtime: 'win', pane_id: 'wJ:p1', format: 'text', text: 'stable content', truncated: false }) },
      ];
      window.fetch = () => {
        const idx = Math.min(window.__fetchCalls, responses.length - 1);
        const r = responses[idx];
        window.__fetchCalls += 1;
        return Promise.resolve({ ok: r.ok, status: r.status, text: () => Promise.resolve(r.body) });
      };
      window.liveOutput.select('win', 'wJ:p1');
      true;
    })()`);

    await cdp.waitFor('window.__fetchCalls >= 1', 3000, '第一次請求（成功）已送出');
    await cdp.waitFor(
      "document.querySelector('.output-text').textContent === 'stable content'",
      3000,
      '第一次成功後內容顯示「stable content」'
    );
    const normalSignals = await readStaleSignals(cdp);
    checkNotStale(normalSignals, '第一次成功後（M 段，正常）');

    await cdp.waitFor('window.__fetchCalls >= 2', 3000, '第二次請求（503）已送出');
    await cdp.waitFor(
      "(() => { const n = document.querySelector('.output-error-reason'); return !!n && !n.hidden; })()",
      3000,
      '503 後顯示過期原因'
    );
    const staleSignals = await readStaleSignals(cdp);
    checkMarkedStale(staleSignals, '503 後（M 段）');
    const contentDuringStale = await cdp.eval("document.querySelector('.output-text').textContent");
    check(
      contentDuringStale === 'stable content',
      `503 期間應該保留最後一份文字（實際 ${JSON.stringify(contentDuringStale)}）`
    );

    // 不額外等「第三次請求已送出」（__fetchCalls >= 3）這個中繼條件：直接等最終可觀察的狀態
    // （原因節點重新隱藏）即可，逾時值放寬一點（8 秒）吸收 CDP 往返與排程的正常抖動。
    await cdp.waitFor(
      "(() => { const n = document.querySelector('.output-error-reason'); return !!n && n.hidden; })()",
      8000,
      '恢復後過期原因消失（即使內容跟之前一樣）'
    );
    const recoveredSignals = await readStaleSignals(cdp);
    checkNotStale(recoveredSignals, '恢復後（M 段，即使內容跟之前一樣）');
    const finalText = await cdp.eval("document.querySelector('.output-text').textContent");
    check(
      finalText === 'stable content',
      `內容應該仍是「stable content」（沒有被意外清空或改變，實際 ${JSON.stringify(finalText)}）`
    );
  } finally {
    await stopChrome(chrome, 'chrome-M');
    if (preview) {
      killTree(preview.server, 'preview-M');
      await sleep(300);
      check(!isPortListening(preview.port), `port ${preview.port}（preview-M）應該不再有 LISTENING 的行程`);
    }
  }
}

// 第五段（G4 fix wave，Finding 1 [R20]；round 2 補強 Finding A／B [R22]）：切換 pane 不再被
// 舊請求擋住——output.js 加 AbortController，select()／clear()／markGone() abort 進行中的
// 請求並（select 時）立即對新選取發請求；abort 造成的 reject 不算失敗；加前端逾時（6 秒，略
// 大於服務端 5 秒逾時）。
//   - N：wJ:p1=delay:4500（小於服務端 5 秒逾時、真端點、原生 fetch）。選 wJ:p1，約 0.5 秒後
//     改選 wJ:p3（立即回應）——驗「wJ:p3 的內容在改選後 3 秒內出現」（門檻與 spec「輪詢與
//     顯示」相同）；之後等過 wJ:p1 原本的 4500 ms 延遲，用 MutationObserver（比照 G 段手法）
//     確認它的回應（若到達）沒有改變面板、沒有觸發過期標示——每一筆歷史除了比對標題，也明確
//     拒絕 wJ:p1 的內容特徵（`A_CONTENT_PATTERN`），不能只看標題（round 2 Finding B：
//     `applySuccess()` 只更新文字不改標題，若舊回應真的被誤套用到目前面板，標題仍會是
//     wJ:p3，只有文字混進 wJ:p1 的內容，只比標題測不出來）。RED：修之前 wJ:p3 要等 wJ:p1 的
//     回應落地（約 4.5 秒）才出現，3 秒的 waitFor 逾時。
//     round 2 Finding A（R22）另外掛 CDP Network domain：用 `requestWillBeSent`／
//     `loadingFinished`／`loadingFailed` 追蹤輸出端點請求的完整生命週期，驗兩件事——(1) 舊的
//     wJ:p1 請求真的在網路層被取消（`loadingFailed` 的 `canceled === true`），不是只在 JS 層
//     被丟棄、底層連線繼續掛著；(2) 用事件的 CDP timestamp（不是 WS 訊息送達 JS 的先後順序）
//     算重疊，任一時刻「已開始但尚未結束」的輸出端點請求數不超過一個——`computeMaxConcurrent
//     Intervals()` 對時間點相同的 start／end 視為「先結束才開始」，避免新請求的
//     `requestWillBeSent` 與舊請求的 `loadingFailed` 剛好在同一個事件迴圈內先後送達 JS 時被
//     誤判成堆積。這組斷言的前提（也是它要驗證的事）：output.js「同一時間至多一個進行中請求」
//     這個不變量，建立在瀏覽器原生 `fetch` 真的遵守 `AbortSignal`（在網路層取消請求）之上——
//     這是產品實際執行的環境，O 段那種忽略 `AbortSignal` 的假 `fetch` 只是模擬「請求卡住」的
//     測試手法，不是受支援的執行環境（見 O 段開頭註解）。
//   - O：直接替換 window.fetch（比照 M 段手法），讓第一個請求回傳一個永遠不 settle、且完全
//     不理會 init.signal 的 Promise——驗前端逾時判定不依賴底層 fetch 是否真的因為 abort() 而
//     reject。約 6 秒後面板應該標為過期並顯示逾時原因，之後依節奏重試（第二個假回應成功 →
//     過期標示與原因消失、內容更新）。RED：修之前沒有任何前端逾時機制，面板永遠停在「已選取
//     但沒有內容」，等不到過期標示，8 秒的 waitFor 逾時。**round 2 Finding A（R22）定位澄清**：
//     這裡的假 fetch 是刻意忽略 `AbortSignal` 的測試替身，模擬「底層請求卡住不動」這種情境
//     （用來單獨驗證前端逾時判定不依賴 fetch 真的 reject），不是宣稱 output.js 支援或需要在
//     一個忽略 abort 的執行環境下維持「至多一個進行中請求」——那個不變量的驗證在 N 段，前提是
//     原生 fetch（見 N 段）。
async function partSwitchNotBlockedBySlowPane() {
  log('=== N. wJ:p1=delay:4500、wJ:p3 立即：慢 pane 不擋住改選（R20／R22 Network 事件佐證）===');
  let preview = null;
  let chrome = null;
  try {
    preview = await startPreview({ COCKPIT_PREVIEW_OUTPUT_MODES: 'wJ:p1=delay:4500' }, 'preview-N');
    const url = `http://127.0.0.1:${preview.port}/`;
    chrome = await startChrome(pickPort(19040, [preview.port]), url, 'chrome-N');
    const { cdp } = chrome;
    await cdp.waitFor(
      "typeof window.liveOutput === 'object' && typeof window.liveOutput.select === 'function'",
      5000,
      'output.js 載入完成，window.liveOutput 就緒'
    );

    // [Ruling R22] 掛 Network domain，追蹤輸出端點請求的完整生命週期（見上方檔頭註解）。
    await cdp.send('Network.enable');
    const outputRequests = new Map(); // requestId -> { url, startTs, endTs, endType, canceled }
    cdp.onEvent('Network.requestWillBeSent', (p) => {
      if (!/\/panes\/.+\/output(\?|$)/.test(p.request.url)) return;
      outputRequests.set(p.requestId, {
        url: p.request.url,
        startTs: p.timestamp,
        endTs: null,
        endType: null,
        canceled: null,
      });
    });
    cdp.onEvent('Network.loadingFinished', (p) => {
      const r = outputRequests.get(p.requestId);
      if (!r) return;
      r.endTs = p.timestamp;
      r.endType = 'finished';
    });
    cdp.onEvent('Network.loadingFailed', (p) => {
      const r = outputRequests.get(p.requestId);
      if (!r) return;
      r.endTs = p.timestamp;
      r.endType = 'failed';
      r.canceled = p.canceled === true;
    });

    log('--- 選 wJ:p1（delay:4500）---');
    await cdp.eval("window.liveOutput.select('win', 'wJ:p1'); true");
    await sleep(500);
    check(
      preview.requests.some((r) => r.pane === 'wJ:p1'),
      'GIVEN：wJ:p1 的輸出請求應該已經送出、還在飛（遠小於 4500 ms，尚未回應）'
    );

    // 掛 MutationObserver（比照 G 段手法）：等過 wJ:p1 原本的延遲之後，確認它的回應（若到達）
    // 完全沒有改變過面板——不能只在最後看一眼，中途閃現也算違規。
    await cdp.eval(`(() => {
      window.__historyN = [];
      const titleEl = document.querySelector('.output-title');
      const preEl = document.querySelector('.output-text');
      const push = () => window.__historyN.push({ t: Date.now(), title: titleEl.textContent, text: preEl.textContent });
      const observer = new MutationObserver(push);
      observer.observe(document.getElementById('output'), { childList: true, characterData: true, subtree: true });
      true;
    })()`);

    const switchAt = Date.now();
    log('--- 0.5 秒後改選 wJ:p3（立即回應）---');
    await cdp.eval("window.liveOutput.select('win', 'wJ:p3'); true");

    await cdp.waitFor(
      "(() => { const t = document.querySelector('.output-text'); return !!t && t.textContent.length > 0; })()",
      3000,
      '改選後 3 秒內 wJ:p3 的內容應該出現'
    );
    const elapsed = Date.now() - switchAt;
    check(elapsed < 3000, `wJ:p3 的內容應該在改選後 3 秒內出現（實際 ${elapsed} ms）`);
    check(
      (await cdp.eval("document.querySelector('.output-title').textContent")) === 'win / wJ:p3',
      '面板標題應該是 win / wJ:p3'
    );
    check(
      !(await cdp.eval("document.getElementById('output').classList.contains('is-stale')")),
      '改選之後面板不應該標為過期（abort 造成的 reject 不算失敗）'
    );
    check(
      (await cdp.eval("document.querySelector('.output-error-reason').hidden")) === true,
      '改選之後不應該顯示失敗原因（abort 造成的 reject 不算失敗）'
    );

    // 等過 wJ:p1 原本 4500 ms 的延遲＋安全邊界，確認它（若到達）沒有留下任何痕跡。
    await sleep(4700);
    const historyN = await cdp.eval('window.__historyN');
    // round 2 Finding B：不能只看 title——applySuccess() 只更新文字、不改標題，若 wJ:p1 的
    // 回應真的被誤套用到目前面板，標題仍會是 win / wJ:p3、只有 text 混進 wJ:p1 的內容特徵。
    // 每一筆都要同時拒絕「標題不是 wJ:p3」與「文字含 wJ:p1 的內容特徵」兩種洩漏方式。
    const leakedN = historyN.filter((h) => h.title !== 'win / wJ:p3' || A_CONTENT_PATTERN.test(h.text));
    check(
      leakedN.length === 0,
      `wJ:p1 的延遲回應到達後不應該改變面板（實際 ${JSON.stringify(leakedN)}）`
    );
    check(
      !(await cdp.eval("document.getElementById('output').classList.contains('is-stale')")),
      'wJ:p1 的回應到達後面板仍不應該標為過期'
    );

    // [Ruling R22 Finding A] 用 Network 事件佐證：abort 真的在網路層結束了舊請求，且任一時刻
    // 最多一個進行中的輸出端點請求。
    const records = Array.from(outputRequests.values());
    check(
      records.length >= 2,
      `應該至少觀察到 2 筆輸出端點請求（wJ:p1 舊的、wJ:p3 新的；實際 ${records.length} 筆：${JSON.stringify(records.map((r) => r.url))}）`
    );
    const unfinished = records.filter((r) => r.endTs === null);
    check(
      unfinished.length === 0,
      `所有觀察到的輸出端點請求都應該有結束事件（finished 或 failed）（實際仍未結束：${JSON.stringify(unfinished)}）`
    );
    const encodedP1 = encodeURIComponent('wJ:p1');
    const p1Records = records.filter((r) => r.url.includes(encodedP1));
    check(
      p1Records.length >= 1,
      `應該觀察到至少一筆 wJ:p1 的輸出端點請求（實際 ${JSON.stringify(records.map((r) => r.url))}）`
    );
    check(
      p1Records.every((r) => r.endType === 'failed' && r.canceled === true),
      `wJ:p1 的請求應該因為 abort 在網路層被取消（loadingFailed canceled:true）（實際 ${JSON.stringify(p1Records)}）`
    );
    const maxConcurrent = computeMaxConcurrentIntervals(records.map((r) => ({ start: r.startTs, end: r.endTs })));
    check(
      maxConcurrent <= 1,
      `任一時刻進行中的輸出端點請求數應該 ≤ 1（依 Network 事件 timestamp 算出的重疊數，實際 ${maxConcurrent}；` +
        `${JSON.stringify(records.map((r) => ({ url: r.url, startTs: r.startTs, endTs: r.endTs, endType: r.endType })))}）`
    );
  } finally {
    await stopChrome(chrome, 'chrome-N');
    if (preview) {
      killTree(preview.server, 'preview-N');
      await sleep(300);
      check(!isPortListening(preview.port), `port ${preview.port}（preview-N）應該不再有 LISTENING 的行程`);
    }
  }
}

// O：直接替換 window.fetch（比照 M 段手法），第一個回應永遠不 settle 且不理會 init.signal，驗
// 前端逾時判定不依賴底層 fetch 是否真的因為 abort() 而 reject（brief「精確值」：這個判定要獨立
// 於 fetch 本身的行為）。**round 2 Finding A（R22）定位澄清**：這是模擬「底層請求卡住」的測試
// 替身，不是宣稱 output.js 支援忽略 AbortSignal 的執行環境——見上方 N 段開頭註解與 output.js
// 檔頭的對應說明。
async function partFrontendTimeoutDoesNotFreeze() {
  log('=== O. 永久 pending 的 fetch：前端逾時不凍結輪詢（R20）===');
  let preview = null;
  let chrome = null;
  try {
    preview = await startPreview({}, 'preview-O');
    const url = `http://127.0.0.1:${preview.port}/`;
    chrome = await startChrome(pickPort(19060, [preview.port]), url, 'chrome-O');
    const { cdp } = chrome;
    await cdp.waitFor(
      "typeof window.liveOutput === 'object' && typeof window.liveOutput.select === 'function'",
      5000,
      'output.js 載入完成，window.liveOutput 就緒'
    );

    await cdp.eval(`(() => {
      window.__fetchCalls = 0;
      window.fetch = () => {
        window.__fetchCalls += 1;
        if (window.__fetchCalls === 1) {
          return new Promise(() => {});
        }
        return Promise.resolve({
          ok: true,
          status: 200,
          text: () => Promise.resolve(JSON.stringify({
            runtime: 'win', pane_id: 'wJ:p1', format: 'text', text: 'recovered', truncated: false
          })),
        });
      };
      window.liveOutput.select('win', 'wJ:p1');
      true;
    })()`);

    await cdp.waitFor('window.__fetchCalls >= 1', 3000, '第一次請求（永不 settle）已送出');
    const selectAt = Date.now();

    await cdp.waitFor(
      "(() => { const n = document.querySelector('.output-error-reason'); return !!n && !n.hidden; })()",
      8000,
      '約 6 秒後面板應該顯示逾時原因'
    );
    const elapsed = Date.now() - selectAt;
    check(elapsed >= 5500, `逾時判定應該在接近 6 秒才觸發，不應該提早（實際 ${elapsed} ms）`);

    const reasonText = await cdp.eval("document.querySelector('.output-error-reason').textContent");
    check(
      typeof reasonText === 'string' && reasonText.length > 0,
      `逾時原因文字不應該是空字串（實際 ${JSON.stringify(reasonText)}）`
    );
    check(
      await cdp.eval("document.getElementById('output').classList.contains('is-stale')"),
      '逾時後面板應該標為過期'
    );

    log('--- 之後依節奏重試（第二個假回應成功）---');
    await cdp.waitFor('window.__fetchCalls >= 2', 4000, '逾時後應該依節奏重試（第二次請求送出）');
    await cdp.waitFor(
      "(() => { const n = document.querySelector('.output-error-reason'); return !!n && n.hidden; })()",
      4000,
      '重試成功後過期原因應該消失'
    );
    check(
      !(await cdp.eval("document.getElementById('output').classList.contains('is-stale')")),
      '重試成功後面板不應該再標為過期'
    );
    check(
      (await cdp.eval("document.querySelector('.output-text').textContent")) === 'recovered',
      '重試成功後內容應該更新'
    );
  } finally {
    await stopChrome(chrome, 'chrome-O');
    if (preview) {
      killTree(preview.server, 'preview-O');
      await sleep(300);
      check(!isPortListening(preview.port), `port ${preview.port}（preview-O）應該不再有 LISTENING 的行程`);
    }
  }
}

// Q（round 2 Finding B；round 3 改成確定性控制，Codex medium finding）：重建 generation
// guard（onPollSettled 開頭的 `gen !== generation` discard 分支）的辨識力。R20 之後，G／N 兩段
// 的「舊回應不蓋掉新選取」主要是靠 abort——舊請求被 abort 之後根本不會走到 onPollSettled 的
// 世代比對（見 runPoll() 的 reject 分支：`controller.signal.aborted` 為真時直接 return，不呼叫
// onPollSettled）——世代序號檢查本身因此失去被獨立驗到的機會。這裡用一個「忽略 AbortSignal」的
// 假 window.fetch（跟 O 段同一種測試手法：刻意讓 abort() 對這個假 Promise 的結局沒有影響，逼
// 流程真的走到世代比對，而不是被 abort 分支攔下）。
//
// round 3：原本 A 的「何時落地」只靠固定 2500 ms 的 timer＋事後固定 sleep 3200 ms 判定順序，
// 兩者都是計時器碰運氣——測試本身既沒有記錄、也沒有等待 A 的 Promise 真的 resolve，更沒有
// 保證 resolve 發生在改選 B 之後；負載重時 timer 可能提早或延遲觸發，移除 generation guard 之後
// 仍可能巧合 PASS，RED 辨識力不可靠（Codex 原文見 fix3 brief）。改成「A 何時落地」由測試端明確
// 控制，不設 timer：
//   1. pane A（wJ:p1）的假 fetch 不設 timer，resolver 存到 window.__qResolveA；pane B
//      （wJ:p3）立即成功、內容特徵（GEN-GUARD-B-CONTENT）跟 A（GEN-GUARD-A-CONTENT）明確可
//      區分。
//   2. 選 A → 等到假 fetch 確實已經被以 A 的 URL 呼叫過（window.__qACalled，逾時才 FAIL）→
//      改選 B → 等到面板已經顯示 B 的內容（條件等待，不是固定 sleep）→ 記錄這個時間點
//      （performance.now()，瀏覽器行程內部）→ 此時才由測試呼叫 window.__qResolveA(...)，同時
//      記錄呼叫當下的時間點。兩個時間點都是同一個瀏覽器行程內的 performance.now()，不跨行程
//      比較時間戳（上一輪 Q 段 flake 的教訓，見 report「Q 段 flake 與修法」）。
//   3. A 的 Promise resolve 之後，output.js 還要 await response.text() 才進
//      onPollSettled：假 Response 的 text() 被呼叫時設 window.__qABodyRead = true；等它變真
//      之後，再讓頁面自己排一個 50 ms 的 setTimeout 標記（window.__qSettleDone），確保
//      response.text() 之後的 .then（complete() → onPollSettled()）已經跑完，才做斷言。
//   4. 斷言：改選 B 之後的整段文字歷史（MutationObserver，沿用 G／N 段手法）從未出現 A 的內容
//      特徵；標題全程是 B；最終面板是 B；面板沒有過期標示、沒有失敗原因；A 的 resolver 確實是
//      在 B 內容顯示之後才被呼叫。
async function partGenerationGuardStillWorks() {
  log(
    '=== Q. 忽略 AbortSignal 的假 fetch，A 何時落地由測試端明確控制：重建 generation guard 的辨識力（round 3）==='
  );
  let preview = null;
  let chrome = null;
  try {
    preview = await startPreview({}, 'preview-Q');
    const url = `http://127.0.0.1:${preview.port}/`;
    chrome = await startChrome(pickPort(19100, [preview.port]), url, 'chrome-Q');
    const { cdp } = chrome;
    await cdp.waitFor(
      "typeof window.liveOutput === 'object' && typeof window.liveOutput.select === 'function'",
      5000,
      'output.js 載入完成，window.liveOutput 就緒'
    );

    const encodedP1 = encodeURIComponent('wJ:p1');
    await cdp.eval(`(() => {
      window.__history = [];
      const titleEl = document.querySelector('.output-title');
      const preEl = document.querySelector('.output-text');
      const push = () => window.__history.push({ t: Date.now(), title: titleEl.textContent, text: preEl.textContent });
      const observer = new MutationObserver(push);
      observer.observe(document.getElementById('output'), { childList: true, characterData: true, subtree: true });
      push();

      window.__qACalled = false;
      window.__qABodyRead = false;
      window.__qResolveA = null;
      window.fetch = (u) => {
        const isA = u.indexOf(${JSON.stringify(encodedP1)}) !== -1;
        const body = JSON.stringify({
          runtime: 'win',
          pane_id: isA ? 'wJ:p1' : 'wJ:p3',
          format: 'text',
          text: isA ? 'GEN-GUARD-A-CONTENT' : 'GEN-GUARD-B-CONTENT',
          truncated: false,
        });
        const resp = {
          ok: true,
          status: 200,
          text: () => {
            if (isA) window.__qABodyRead = true;
            return Promise.resolve(body);
          },
        };
        if (isA) {
          // 忽略第二個參數（init.signal）：即使 abort() 被呼叫，這個 Promise 也不會自動
          // resolve——resolver 存到 window.__qResolveA，由測試端在確認 B 已經顯示之後才明確
          // 呼叫，不設 timer（round 3：逼流程真的走到世代比對，而不是被 abort 分支攔下，也不是
          // 靠 timer 碰運氣）。
          window.__qACalled = true;
          return new Promise((resolve) => {
            window.__qResolveA = () => resolve(resp);
          });
        }
        return Promise.resolve(resp);
      };
      true;
    })()`);

    log('--- 選 A（wJ:p1）---');
    await cdp.eval("window.liveOutput.select('win', 'wJ:p1'); true");
    await cdp.waitFor('window.__qACalled === true', 3000, 'GIVEN：A（wJ:p1）的假 fetch 應該已經被呼叫過');

    // 「改選 B 之後」的分界：改選前先記下歷史目前的長度，之後只看這個索引之後新增的紀錄——
    // 不用跨行程時間戳比較（上一輪 Q 段 flake 的教訓，見 report）。
    const markerIndex = await cdp.eval('window.__history.length');

    log('--- 改選 B（wJ:p3，立即成功）---');
    await cdp.eval("window.liveOutput.select('win', 'wJ:p3'); true");
    await cdp.waitFor(
      "(() => { const t = document.querySelector('.output-text'); return !!t && t.textContent === 'GEN-GUARD-B-CONTENT'; })()",
      3000,
      'B 的內容應該很快出現'
    );
    await cdp.eval('window.__qBShownAt = performance.now(); true');

    log('--- 此時才讓 A 落地（測試端明確呼叫 resolver）---');
    await cdp.eval('window.__qAResolvedAt = performance.now(); window.__qResolveA(); true');

    await cdp.waitFor(
      'window.__qABodyRead === true',
      3000,
      'A 的假 Response.text() 應該已經被呼叫過（resolver 已經讓 Promise resolve）'
    );

    // 再排一個 macrotask，確保 response.text() 之後的 .then（complete() → onPollSettled()）
    // 已經跑完，才做斷言——text() 被呼叫只保證 Promise 鏈走到那一步，不保證 onPollSettled 已經
    // 執行完畢（brief 第 3 點）。
    await cdp.eval('window.__qSettleDone = false; setTimeout(() => { window.__qSettleDone = true; }, 50); true');
    await cdp.waitFor(
      'window.__qSettleDone === true',
      2000,
      'A 落地後應該讓 output.js 的 onPollSettled 有機會跑完（50 ms macrotask）'
    );

    const history = await cdp.eval('window.__history');
    const postSwitch = history.slice(markerIndex);
    const leaked = postSwitch.filter((h) => h.text.indexOf('GEN-GUARD-A-CONTENT') !== -1);
    check(
      leaked.length === 0,
      `改選 B 之後的歷史裡不應該出現 A 的內容（A 的假 fetch 忽略 abort、由測試端明確在 B 顯示之後才讓它落地；實際 ${leaked.length}／${postSwitch.length} 筆含 A 內容：${JSON.stringify(leaked)}）`
    );
    check(
      postSwitch.length > 0 && postSwitch.every((h) => h.title === 'win / wJ:p3'),
      `改選 B 之後的歷史裡標題應該全程是 win / wJ:p3（實際 ${JSON.stringify(postSwitch.map((h) => h.title))}）`
    );
    const finalTitle = await cdp.eval("document.querySelector('.output-title').textContent");
    const finalText = await cdp.eval("document.querySelector('.output-text').textContent");
    check(
      finalTitle === 'win / wJ:p3' && finalText === 'GEN-GUARD-B-CONTENT',
      `最終面板應該是 B 的內容（實際標題 ${JSON.stringify(finalTitle)}，內容 ${JSON.stringify(finalText)}）`
    );
    check(
      !(await cdp.eval("document.getElementById('output').classList.contains('is-stale')")),
      'A 落地之後面板不應該標為過期（generation guard 在世代比對就丟棄，不當成一次失敗處理）'
    );
    check(
      (await cdp.eval("document.querySelector('.output-error-reason').hidden")) === true,
      'A 落地之後不應該顯示失敗原因（generation guard 丟棄的回應不算一次失敗）'
    );

    const times = await cdp.eval('({ resolvedAt: window.__qAResolvedAt, shownAt: window.__qBShownAt })');
    check(
      times.resolvedAt >= times.shownAt,
      `A 的 resolver 確實應該在 B 內容顯示之後才被呼叫（同一瀏覽器行程內 performance.now()，不跨行程比較；實際 resolvedAt=${times.resolvedAt}、shownAt=${times.shownAt}）`
    );
  } finally {
    await stopChrome(chrome, 'chrome-Q');
    if (preview) {
      killTree(preview.server, 'preview-Q');
      await sleep(300);
      check(!isPortListening(preview.port), `port ${preview.port}（preview-Q）應該不再有 LISTENING 的行程`);
    }
  }
}

// 第六段（G4 fix wave，Finding 2 [R19]）：選取不是「畫面操作」——actions.js 的
// select-pane／select-bound-pane 不得遞增 latestOp、不得清 ui.error（否則會把正在飛、稍後才會
// 失敗的寫入請求的錯誤訊息悄悄吞掉）。
//   - P：`COCKPIT_PREVIEW_WRITE_RULES` 讓 be-1 的「Failed」延遲 1500 ms 後回 409。按下「Failed」
//     後立刻點一個 pane 列（選取）：驗選取本身照常生效、延遲的 409 回來後錯誤訊息仍然正常出現
//     （含回應本體的 error 字串）；接著再選另一個 pane：驗既有的錯誤訊息不被清掉。RED：修之前
//     選取會把 latestOp 往前推，延遲的 409 回來時 op 已經不是 latestOp，`showError` 因此忽略，
//     錯誤訊息永遠不出現；已顯示的錯誤訊息也會被下一次選取立刻清掉。（已在前一個 commit 修好，
//     這裡的 P 段沿用不動。）
async function partSelectionDoesNotSwallowWriteError() {
  log('=== P. 選取不吞掉寫入失敗／選取不清掉既有錯誤（R19）===');
  const FAIL_PATH = '/api/projects/cockpit/tasks/be-1/fail';
  const FAIL_DELAY_MS = 1500;
  let preview = null;
  let chrome = null;
  try {
    preview = await startPreview(
      { COCKPIT_PREVIEW_WRITE_RULES: `${FAIL_PATH}=${FAIL_DELAY_MS}:409` },
      'preview-P'
    );
    const url = `http://127.0.0.1:${preview.port}/`;
    chrome = await startChrome(pickPort(19080, [preview.port]), url, 'chrome-P');
    const { cdp } = chrome;
    // direction-01-visual task 3.1：Factory Floor 一次只畫選定的一個 Project（design D6），
    // cockpit 是未選定過時的預設 Project（fixture 順序 cockpit、p），這個情境全程只用到
    // cockpit 的 task／pane，改成等 cockpit 專案的 9 個 task 節點全部畫出（不再假設 12 個，
    // 那是舊行為「兩個 Project 疊在一起畫」才有的總數）。
    await cdp.waitFor("document.querySelectorAll('.task-node').length >= 9", 5000, '畫出 cockpit 專案全部 task 節點');
    await cdp.waitFor(
      "typeof window.liveOutput === 'object' && typeof window.liveOutput.select === 'function'",
      5000,
      'output.js 載入完成，window.liveOutput 就緒'
    );

    log('--- 情境「選取不吞掉寫入失敗」---');
    await cdp.click('[data-action="fail"][data-project="cockpit"][data-task="be-1"]');
    await cdp.click('.pane-row[data-runtime="win"][data-pane="wJ:p1"]');
    check(
      (await cdp.eval("document.querySelectorAll('.error-banner').length")) === 0,
      '選取當下（延遲的 409 還沒回來）不應該提早出現錯誤訊息'
    );
    check(
      await cdp.eval(
        "(() => { const r = document.querySelector('.pane-row[data-pane=\"wJ:p1\"]'); return !!r && r.classList.contains('selected'); })()"
      ),
      '選取本身應該正常生效（wJ:p1 出現選定標示）'
    );
    await cdp.waitFor(
      "(() => { const b = document.querySelector('.error-banner'); return !!b && b.textContent.includes('409'); })()",
      FAIL_DELAY_MS + 2000,
      '延遲的 409 回來後應該顯示錯誤訊息（含 HTTP 409），沒有被選取動作悄悄吞掉'
    );
    const errText = await cdp.eval("document.querySelector('.error-banner').textContent");
    check(
      errText.includes('ui_preview 模擬回應 409'),
      `錯誤訊息應該含回應本體的 error 字串（實際 ${JSON.stringify(errText)}）`
    );
    check(
      await cdp.eval(
        "(() => { const r = document.querySelector('.pane-row[data-pane=\"wJ:p1\"]'); return !!r && r.classList.contains('selected'); })()"
      ),
      '錯誤訊息出現之後，選取仍然是 wJ:p1（選取沒有被寫入失敗影響）'
    );

    log('--- 情境「選取不清掉既有錯誤」---');
    const beforeSelect = await cdp.eval("document.querySelector('.error-banner').textContent");
    await cdp.click('.pane-row[data-runtime="win"][data-pane="wJ:p3"]');
    await sleep(500);
    const afterSelect = await cdp.eval(
      "(() => { const b = document.querySelector('.error-banner'); return b ? b.textContent : null; })()"
    );
    check(
      afterSelect === beforeSelect,
      `選取另一個 pane 之後，既有的錯誤訊息仍應該保留、內容不變（before=${JSON.stringify(beforeSelect)} after=${JSON.stringify(afterSelect)}）`
    );
    check(
      await cdp.eval(
        "(() => { const r = document.querySelector('.pane-row[data-pane=\"wJ:p3\"]'); return !!r && r.classList.contains('selected'); })()"
      ),
      '選取確實生效（wJ:p3 出現選定標示）'
    );
  } finally {
    await stopChrome(chrome, 'chrome-P');
    if (preview) {
      killTree(preview.server, 'preview-P');
      await sleep(300);
      check(!isPortListening(preview.port), `port ${preview.port}（preview-P）應該不再有 LISTENING 的行程`);
    }
  }
}

// 第六段（live-output task focus-fix；同時解掉專案已知的使用性問題 M3）：整頁重畫不得丟失
// 鍵盤焦點。spec cockpit-dashboard「畫面整頁重畫」本文最後兩句、spec live-output「選定一個
// pane」本文與情境「鍵盤選定」「鍵盤焦點跨重畫保留」。render.js 的 paint() 在 replaceChildren
// 前後各比對一次焦點所在元素的身分（data-action 加上該元素當下全部的 data-*），把焦點還原到
// 新樹裡代表同一個對象的元素上；詳見 render.js captureFocusIdentity／findByFocusIdentity／
// restoreFocus 上方註解。
//   - R：COCKPIT_PREVIEW_PUSH_MS=100（頻繁重畫，秒級測試時間內就能驗到「重畫後」），較矮視窗
//     （1400,800，逼頁面本身可捲動，用來驗「不捲動頁面」）。依序驗四個情境（brief 逐字要求）：
//       1. 鍵盤焦點跨重畫保留：focus wJ:p1 的 pane 列 → 等 version 前進 ≥5 且舊節點
//          isConnected 變 false（證明真的被 replaceChildren 換過）→ activeElement 是代表同一
//          個 pane 的新節點 → 送 Enter → 選定 wJ:p1、面板打開。再對 wJ:p3 送 Space 驗一次
//          （同樣先驗過重畫、節點已換過，才送鍵）。
//       2. 重畫不丟鍵盤焦點：focus task be-1 的「推進」按鈕（ui_preview 的寫入端點只記錄、不
//          改投影，見檔頭 startPreview 上方說明——按鈕本身不會因為送出請求而從畫面消失、也不會
//          因為投影沒變而不重畫，version 仍在推送迴圈裡持續前進）→ 2 秒內（100 ms 推送）每
//          200 ms 取樣 activeElement 的身分，全程都應該是 { action: 'advance', project:
//          'cockpit', task: 'be-1' } → 送 Enter → preview stdout 應該恰好收到一筆對應的
//          write-request。
//       3. 焦點在面板內不被搶：上一步送出 Enter 之前，wJ:p3 仍是選取、面板打開（情境 1 的
//          Space 留下的狀態）；focus 面板的「取消選取」按鈕（#output 不在 #app 底下，理論上完全不
//          受重畫影響，design D8）→ 等數次重畫（version 前進 ≥5）→ activeElement 仍是同一個
//          「取消選取」節點（用 === 比對節點參照，不只是選擇器命中，才能證明節點真的沒被換掉）。
//       4. 不捲動頁面：把頁面捲到非零位置 → focus wJ:p1 的 pane 列（測試腳本自己這次
//          `.focus()` 呼叫沒有帶 `preventScroll`，瀏覽器可能把它捲進視窗，這是測試腳本本身的
//          一次性行為，不是要驗的事——所以基準值改在這次 focus **之後**才拍）→ 等數次重畫
//          （version 前進 ≥5，期間 render.js 的 restoreFocus() 因為焦點仍在同一列，會重複呼叫
//          `focus({ preventScroll: true })`）→ `window.scrollY` 不再變化，才是驗
//          `preventScroll: true` 真的生效。
//   - S：COCKPIT_PREVIEW_VANISH_PANE=wJ:p1=3000＋COCKPIT_PREVIEW_PUSH_MS=200（比照 J 段，讓
//     消失儘快反映）。驗「對象消失時不亂跳」：focus wJ:p1 的 pane 列 → 等它從投影消失（列的
//     data-pane="wJ:p1" 選擇器找不到元素）→ 斷言此時鍵盤焦點不在任何 pane 列上（掉回 body 可
//     接受，brief 邊界）。
async function partFocusPreservedAcrossRepaint() {
  log('=== R. COCKPIT_PREVIEW_PUSH_MS=100：整頁重畫不得丟失鍵盤焦點 ===');
  let preview = null;
  let chrome = null;
  try {
    preview = await startPreview({ COCKPIT_PREVIEW_PUSH_MS: '100' }, 'preview-R');
    const url = `http://127.0.0.1:${preview.port}/`;
    // direction-01-visual task 2.1：1400,900（不是原本的 1400,800）——headless Chrome 的
    // window.innerHeight 比 --window-size 要求的高度少约 99px（headless 視窗外框的模擬），
    // 1400,800 量出來的 innerHeight 只有約 701px，落在 design D3「≥1200px 但 <720px 高」那個
    // 仍會整頁捲動的情形，跟這裡「不捲動頁面」情境要驗的固定一屏（≥1200 且 ≥720 高，
    // `.shell` overflow: hidden，頁面本身不能捲動）對不上。1400,900 量出來約 801px，穩穩落在
    // 固定一屏區間。
    chrome = await startChrome(pickPort(19110, [preview.port]), url, 'chrome-R', '1400,900');
    const { cdp } = chrome;
    await cdp.waitFor(
      "typeof window.liveOutput === 'object' && typeof window.liveOutput.select === 'function'",
      5000,
      'output.js 載入完成，window.liveOutput 就緒'
    );
    await cdp.waitFor(
      "document.querySelectorAll('.pane-row[data-action=\"select-pane\"]').length >= 2",
      5000,
      '畫出可選的 pane 列'
    );

    // --- 情境「鍵盤焦點跨重畫保留」（Enter，wJ:p1）---
    log('--- 情境「鍵盤焦點跨重畫保留」（Enter，wJ:p1）---');
    await cdp.eval(
      "window.__oldNode = document.querySelector('.pane-row[data-runtime=\"win\"][data-pane=\"wJ:p1\"]'); window.__oldNode.focus(); true"
    );
    check(
      await cdp.eval(
        "document.activeElement === window.__oldNode && document.activeElement.dataset.pane === 'wJ:p1'"
      ),
      'wJ:p1 的 pane 列可以取得焦點'
    );
    const baselineVersion1 = await cdp.eval("Number(document.getElementById('version').textContent.slice(1))");
    await cdp.waitFor(
      `Number(document.getElementById('version').textContent.slice(1)) - ${baselineVersion1} >= 5`,
      5000,
      'focus 之後應該經過至少 5 次重畫（version 前進 ≥5）'
    );
    const afterRepaint1 = await cdp.eval(`(() => {
      const active = document.activeElement;
      return {
        oldConnected: window.__oldNode.isConnected,
        sameNode: active === window.__oldNode,
        activeRuntime: active && active.dataset ? active.dataset.runtime : null,
        activePane: active && active.dataset ? active.dataset.pane : null,
      };
    })()`);
    check(
      afterRepaint1.oldConnected === false,
      `舊的聚焦節點應該已經不在文件中（isConnected === false，證明真的被 replaceChildren 換過，實際 ${JSON.stringify(afterRepaint1)}）`
    );
    check(
      afterRepaint1.sameNode === false &&
        afterRepaint1.activeRuntime === 'win' &&
        afterRepaint1.activePane === 'wJ:p1',
      `重畫後鍵盤焦點應該落在代表同一個 pane（wJ:p1）的新節點上（實際 ${JSON.stringify(afterRepaint1)}）`
    );
    await cdp.pressKey('Enter', 'Enter', 13, '\r');
    await cdp.waitFor(
      "document.querySelector('.output-title').textContent === 'win / wJ:p1'",
      2000,
      '按 Enter 應該選定 wJ:p1，面板標題更新'
    );
    check(
      await cdp.eval(PANEL_OPEN_JS),
      'Enter 選定後面板顯示有選取的內容（不是空狀態）'
    );

    // --- 再對另一個 pane（wJ:p3）用 Space 驗一次，同樣先驗過重畫、節點已換過 ---
    log('--- 情境「鍵盤焦點跨重畫保留」（Space，wJ:p3）---');
    await cdp.eval(
      "window.__oldNode2 = document.querySelector('.pane-row[data-runtime=\"win\"][data-pane=\"wJ:p3\"]'); window.__oldNode2.focus(); true"
    );
    check(
      await cdp.eval("document.activeElement === window.__oldNode2"),
      'wJ:p3 的 pane 列可以取得焦點'
    );
    const baselineVersion2 = await cdp.eval("Number(document.getElementById('version').textContent.slice(1))");
    await cdp.waitFor(
      `Number(document.getElementById('version').textContent.slice(1)) - ${baselineVersion2} >= 5`,
      5000,
      'focus wJ:p3 之後應該經過至少 5 次重畫（version 前進 ≥5）'
    );
    const afterRepaint2 = await cdp.eval(`(() => {
      const active = document.activeElement;
      return {
        oldConnected: window.__oldNode2.isConnected,
        sameNode: active === window.__oldNode2,
        activeRuntime: active && active.dataset ? active.dataset.runtime : null,
        activePane: active && active.dataset ? active.dataset.pane : null,
      };
    })()`);
    check(
      afterRepaint2.oldConnected === false && afterRepaint2.sameNode === false &&
        afterRepaint2.activeRuntime === 'win' && afterRepaint2.activePane === 'wJ:p3',
      `重畫後鍵盤焦點應該落在代表同一個 pane（wJ:p3）的新節點上（實際 ${JSON.stringify(afterRepaint2)}）`
    );
    await cdp.pressKey(' ', 'Space', 32, ' ');
    await cdp.waitFor(
      "document.querySelector('.output-title').textContent === 'win / wJ:p3'",
      2000,
      '按 Space 應該選定 wJ:p3，面板標題更新'
    );
    check(
      await cdp.eval(
        "document.querySelector('.pane-row[data-pane=\"wJ:p3\"]').classList.contains('selected') && !document.querySelector('.pane-row[data-pane=\"wJ:p1\"]').classList.contains('selected')"
      ),
      'Space 選定後 wJ:p3 有選定標示、wJ:p1 標示消失'
    );

    // --- 情境「重畫不丟鍵盤焦點」（task be-1 的「推進」按鈕）---
    log('--- 情境「重畫不丟鍵盤焦點」（task be-1「推進」，2 秒取樣＋Enter）---');
    await cdp.eval(
      "document.querySelector('[data-action=\"advance\"][data-project=\"cockpit\"][data-task=\"be-1\"]').focus(); true"
    );
    check(
      await cdp.eval(
        "(() => { const a = document.activeElement; return !!a && a.dataset.action === 'advance' && a.dataset.project === 'cockpit' && a.dataset.task === 'be-1'; })()"
      ),
      'be-1 的「推進」按鈕可以取得焦點'
    );
    const identitySamples = [];
    const sampleStart = Date.now();
    while (Date.now() - sampleStart < 2000) {
      const sample = await cdp.eval(
        "(() => { const a = document.activeElement; return a && a.dataset ? { action: a.dataset.action, project: a.dataset.project, task: a.dataset.task } : null; })()"
      );
      identitySamples.push(sample);
      await sleep(200);
    }
    const allAdvanceBe1 = identitySamples.every(
      (s) => s && s.action === 'advance' && s.project === 'cockpit' && s.task === 'be-1'
    );
    check(
      allAdvanceBe1,
      `2 秒內（100 ms 推送）鍵盤焦點應該全程停留在 be-1 的「推進」按鈕上（實際取樣 ${JSON.stringify(identitySamples)}）`
    );
    preview.writeRequests.length = 0;
    await cdp.pressKey('Enter', 'Enter', 13, '\r');
    await sleep(500);
    const advanceWrites = preview.writeRequests.filter(
      (r) => r.method === 'POST' && r.path === '/api/projects/cockpit/tasks/be-1/advance'
    );
    check(
      advanceWrites.length === 1,
      `送出 Enter 後應該恰好收到一筆 POST /api/projects/cockpit/tasks/be-1/advance（實際 ${JSON.stringify(preview.writeRequests)}）`
    );

    // --- 情境「焦點在面板內不被搶」（面板「取消選取」按鈕；wJ:p3 仍是選取、面板仍打開）---
    log('--- 情境「焦點在面板內不被搶」（面板「取消選取」按鈕）---');
    check(
      await cdp.eval(PANEL_OPEN_JS),
      'GIVEN：面板仍是打開的（wJ:p3 仍是選取）'
    );
    await cdp.eval("window.__closeButtonRef = document.querySelector('.output-close'); window.__closeButtonRef.focus(); true");
    check(
      await cdp.eval("document.activeElement === window.__closeButtonRef"),
      '面板「取消選取」按鈕可以取得焦點'
    );
    const baselineVersion3 = await cdp.eval("Number(document.getElementById('version').textContent.slice(1))");
    await cdp.waitFor(
      `Number(document.getElementById('version').textContent.slice(1)) - ${baselineVersion3} >= 5`,
      5000,
      '面板「取消選取」按鈕聚焦後應該經過至少 5 次重畫（version 前進 ≥5）'
    );
    check(
      await cdp.eval("document.activeElement === window.__closeButtonRef"),
      '重畫後鍵盤焦點仍在面板「取消選取」按鈕上（#output 不在 #app 底下，不受重畫影響；用 === 比對同一個節點）'
    );

    await cdp.eval("document.querySelector('.output-close').click(); true");

    // --- 情境「不捲動頁面」（fix round 1 Finding 3：先驗過「拿掉 preventScroll 也要 FAIL」，
    // 證明舊寫法有辨識力；direction-01-visual task 2.1／design D9 改寫：桌面寬度（≥1200 且
    // ≥720 高）下 `.shell` 現在是 `height: 100dvh; overflow: hidden`（design D3「固定一屏」），
    // 整頁（`window.scrollTo` 作用的 document）本身不再捲動，原本「把目標捲出視窗外」的手法
    // 對 document 已經沒有效果；pane 列所在的右欄改成捲動它。
    // fix round 1（task 2.1 fix round 1／design 審核 I2）：真正捲動的容器換成 `.runtime-cards`
    // ——`[data-region="runtimes"]` 現在是不捲動的框（`overflow: hidden`），`.runtime-cards`
    // 才是內層唯一的捲動容器（跟 D8 的切角／3.2 的 sticky 掛在「框」這一層是同一個改動）。
    // 手法不變：先 focus 目標 → 把「仍保持焦點」的目標捲出**容器**的可視範圍外（不是隨便捲到
    // 某個位置，是明確捲到目標的 getBoundingClientRect 完全落在容器可視範圍之外，precondition
    // 必須先成立，見 brief）→ 這時才拍容器捲動基準值 → 等到確認舊節點 isConnected === false
    // 的重畫 → 斷言焦點落在新節點上，且容器的 scrollTop 沒有變。若拿掉 preventScroll，重畫時
    // 的 focus() 會把這個「已經在容器可視範圍外」的新節點捲回可視範圍內（`Element.focus()`
    // 的 preventScroll 涵蓋所有會被捲動的捲動容器祖先，不只 document——MDN／規格：預設會捲動
    // 「捲進可視範圍」，preventScroll 就是關掉這個行為），容器的 scrollTop 一定會變，這樣才是
    // 真的測到 preventScroll 的效果。
    log('--- 情境「不捲動頁面」（focus 目標、捲動 .runtime-cards 的內部容器把它推出可視範圍才拍基準，等數次重畫，內部捲動位置不變）---');
    // fixture 只有 3 個 pane，容器（右欄，min-height 240px 起跳）內容通常撐不滿自己的
    // grid-area，天生沒什麼可捲的空間（maxScroll 太小，不夠把 wJ:p1 這種偏上方的列推出可視
    // 範圍）。注入一條「選擇器規則」（不是 inline style）暫時把這個區塊夾窄，強迫它出現真的
    // 捲得動的 overflow——這裡故意不用 `container.style.maxHeight = ...` 這種 inline style：
    // repaint 會 `replaceChildren` 整個換掉 `.runtime-cards` 這個節點本身，inline
    // style 掛在舊節點上，換掉之後新節點不會帶著它，maxScroll 又縮回天生的小範圍，後面「重畫後
    // scrollTop 有沒有變」會變成拿一個已經被重置的容器比較、失去辨識力（實測驗證過：這樣寫
    // 即使拿掉 render.js 的 `preventScroll: true`，這裡照樣印 PASS）。改用 `<style>` 規則掛在
    // `document.head`（不在 `#app` 底下，不受 `replaceChildren` 影響）：規則跟著 CSS 選擇器走，
    // 每次重畫換上的新節點只要還是 `.runtime-cards` 就會自動套用，maxScroll 才能在
    // 整個情境（focus → 捲動 → 等重畫 → 比對）裡維持一致。
    await cdp.eval(`(() => {
      const style = document.createElement('style');
      style.textContent = '[data-region="runtimes"] > .runtime-cards { max-height: 120px !important; }';
      document.head.appendChild(style);
      return true;
    })()`);
    await cdp.eval(
      "window.__oldNodeF3 = document.querySelector('.pane-row[data-runtime=\"win\"][data-pane=\"wJ:p1\"]'); window.__oldNodeF3.focus(); true"
    );
    check(
      await cdp.eval("document.activeElement === window.__oldNodeF3"),
      'wJ:p1 的 pane 列可以取得焦點'
    );
    const scrollSetup = await cdp.eval(`(() => {
      const row = window.__oldNodeF3;
      const container = document.querySelector('[data-region="runtimes"] > .runtime-cards');
      const rect = row.getBoundingClientRect();
      const cRect = container.getBoundingClientRect();
      const rowTopInContainer = rect.top - cRect.top + container.scrollTop;
      const maxScroll = Math.max(0, container.scrollHeight - container.clientHeight);
      // 一律捲到底（scrollTop = maxScroll，不是「挑一個方向」）：wJ:p1 天生就在內容偏上方
      // （rowTopInContainer 遠小於捲到底之後的可視範圍起點），捲到底一定會把它推到可視範圍
      // 上方看不到；同時這樣 scrollTop 一定會從 0 變成一個非 0 的值，後面「重畫後 scrollTop
      // 沒有變」才是在比對一個真的移動過的位置，不是「本來就是 0、現在還是 0」這種沒有辨識力
      // 的巧合（fix round 1 Finding 3 的教訓——見上方大註解——同一個陷阱換了捲動對象要重新
      // 檢查一次：先前用「maxScroll 太小，scrollTop 停在 0」的寫法量出來 scrollTopApplied 是
      // 0，preventScroll 拿掉也測不出差異，已經現場驗證過、改掉）。
      const newScrollTop = maxScroll;
      container.scrollTop = newScrollTop;
      const rectAfter = row.getBoundingClientRect();
      const cRectAfter = container.getBoundingClientRect();
      return {
        rowTopInContainer,
        maxScroll,
        newScrollTop,
        scrollTopApplied: container.scrollTop,
        offscreen: rectAfter.bottom < cRectAfter.top || rectAfter.top > cRectAfter.bottom,
        stillFocused: document.activeElement === row,
      };
    })()`);
    check(scrollSetup.stillFocused === true, '把目標捲出容器可視範圍的過程中，焦點應該仍在目標上（尚未重畫）');
    check(
      scrollSetup.offscreen === true,
      `目標應該已經整個捲出 .runtime-cards 容器的可視範圍外（實際 ${JSON.stringify(scrollSetup)}）`
    );

    const baselineVersionF3 = await cdp.eval("Number(document.getElementById('version').textContent.slice(1))");
    await cdp.waitFor(
      `Number(document.getElementById('version').textContent.slice(1)) - ${baselineVersionF3} >= 5`,
      5000,
      '目標捲出容器可視範圍後應該再經過至少 5 次重畫（version 前進 ≥5）'
    );

    const afterF3 = await cdp.eval(`(() => {
      const active = document.activeElement;
      const container = document.querySelector('[data-region="runtimes"] > .runtime-cards');
      return {
        oldConnected: window.__oldNodeF3.isConnected,
        sameNode: active === window.__oldNodeF3,
        activeRuntime: active && active.dataset ? active.dataset.runtime : null,
        activePane: active && active.dataset ? active.dataset.pane : null,
        containerScrollTop: container ? container.scrollTop : null,
      };
    })()`);
    check(
      afterF3.oldConnected === false,
      `舊的聚焦節點應該已經不在文件中（isConnected === false，證明真的被 replaceChildren 換過，實際 ${JSON.stringify(afterF3)}）`
    );
    check(
      afterF3.sameNode === false && afterF3.activeRuntime === 'win' && afterF3.activePane === 'wJ:p1',
      `重畫後鍵盤焦點應該落在代表同一個 pane（wJ:p1）的新節點上（實際 ${JSON.stringify(afterF3)}）`
    );
    // direction-01-visual task 2.1 fix round 4：比對基準改回 scrollSetup.scrollTopApplied。
    // round 1 的前提是「.runtime-cards 在 #app 底下、每次 replaceChildren 都換成新節點，新節點的
    // scrollTop 一定是 0」，所以把預期值訂為 0；fix round 4 發現這個前提本身就是 2.1 的產品迴歸
    // ——內層捲動容器每次推送都被拉回頂端（actions-check.js「頻繁重畫時按鈕仍有效」因此間歇漏送
    // POST），render.js 的 paint() 改成重畫前記下、重畫後寫回內層捲動位置。前提被推翻後，這裡
    // 回到本段標題原本要驗的事：等數次重畫之後，容器的 scrollTop 仍是捲出目標時設的值（目標仍在
    // 可視範圍外）。辨識力：拿掉 render.js restoreFocus() 的 preventScroll 時，重畫後的 focus()
    // 會把容器捲回去顯示目標，scrollTop 必然改變（fix round 4 現場驗證過，數字見 task 2.1 report）。
    check(
      afterF3.containerScrollTop === scrollSetup.scrollTopApplied && scrollSetup.scrollTopApplied > 0,
      `數次重畫之後 .runtime-cards 的捲動位置應該不變（捲出目標時設為 ${scrollSetup.scrollTopApplied}）——內層捲動位置跨重畫保留，且 focus({ preventScroll: true }) 不得把容器捲去顯示目標（實際 ${afterF3.containerScrollTop}）`
    );
  } finally {
    await stopChrome(chrome, 'chrome-R');
    if (preview) {
      killTree(preview.server, 'preview-R');
      await sleep(300);
      check(!isPortListening(preview.port), `port ${preview.port}（preview-R）應該不再有 LISTENING 的行程`);
    }
  }
}

// S：COCKPIT_PREVIEW_VANISH_PANE=wJ:p1=3000＋COCKPIT_PREVIEW_PUSH_MS=200（比照 J 段）。驗
// 「對象消失時不亂跳」：目標從投影裡真的消失時，焦點不得誤落在其他 pane 列上（掉回 body 可
// 接受，brief 邊界情境）。
async function partFocusNotStolenByVanishedTarget() {
  log('=== S. COCKPIT_PREVIEW_VANISH_PANE=wJ:p1=3000：對象消失時鍵盤焦點不亂跳 ===');
  let preview = null;
  let chrome = null;
  try {
    preview = await startPreview(
      { COCKPIT_PREVIEW_PUSH_MS: '200', COCKPIT_PREVIEW_VANISH_PANE: 'wJ:p1=3000' },
      'preview-S'
    );
    const url = `http://127.0.0.1:${preview.port}/`;
    chrome = await startChrome(pickPort(19120, [preview.port]), url, 'chrome-S');
    const { cdp } = chrome;
    await cdp.waitFor(
      "typeof window.liveOutput === 'object' && typeof window.liveOutput.select === 'function'",
      5000,
      'output.js 載入完成，window.liveOutput 就緒'
    );
    await cdp.waitFor(
      "!!document.querySelector('.pane-row[data-runtime=\"win\"][data-pane=\"wJ:p1\"]')",
      5000,
      '畫出 wJ:p1 的 pane 列'
    );

    await cdp.eval(
      "document.querySelector('.pane-row[data-runtime=\"win\"][data-pane=\"wJ:p1\"]').focus(); true"
    );
    check(
      await cdp.eval("document.activeElement && document.activeElement.dataset.pane === 'wJ:p1'"),
      'GIVEN：wJ:p1 的 pane 列可以取得焦點'
    );

    log('--- 等待 wJ:p1 從投影中消失（3 秒後，推送間隔 200 ms）---');
    await cdp.waitFor(
      "!document.querySelector('.pane-row[data-runtime=\"win\"][data-pane=\"wJ:p1\"]')",
      6000,
      'wJ:p1 的 pane 列應該從投影中消失'
    );

    const afterVanish = await cdp.eval(`(() => {
      const active = document.activeElement;
      return {
        tag: active ? active.tagName : null,
        isPaneRow: !!active && !!active.classList && active.classList.contains('pane-row'),
        isBody: active === document.body,
      };
    })()`);
    check(
      afterVanish.isPaneRow === false,
      `目標消失後鍵盤焦點不應該落在任何其他 pane 列上（掉回 body 可接受，實際 ${JSON.stringify(afterVanish)}）`
    );
  } finally {
    await stopChrome(chrome, 'chrome-S');
    if (preview) {
      killTree(preview.server, 'preview-S');
      await sleep(300);
      check(!isPortListening(preview.port), `port ${preview.port}（preview-S）應該不再有 LISTENING 的行程`);
    }
  }
}

// ---------------------------------------------------------------------------
// fix round 1（Codex 對 af7aec4 提的 3 個 finding）
// ---------------------------------------------------------------------------
//
// T：Finding 2 [medium]——兩個 workstream 綁同一個 pane 時，「看輸出」按鈕的身分只看
// action＋runtime＋pane 會完全相同，焦點還原遇到重複身分取文件順序第一個，焦點原在後一列時
// 重畫後會錯誤跳到第一列。render.js 的修法：「看輸出」另外帶 `project`／`source-workstream`
// （刻意不叫 `workstream`，避免混進 actions-check.js 既有 `[data-action][data-workstream]`
// 的按鈕集合斷言）。
//
// fixture（`projected-state.json`）裡 `be` 綁 `win`/`wJ:p1`、`qa` 綁 `win`/`wJ:p3`，兩個
// workstream 沒有天生綁同一個 pane 的情況；ui_preview 的寫入端點只記錄、不改投影（見檔頭
// `startPreview` 上方說明），走「改綁 → 綁定到這裡」的真實 UI 流程不會讓投影真的變成兩個
// workstream 綁同一個 pane。改用最小做法：直接呼叫 `window.onState(...)`（比照
// `factory-floor-check.js`「情境二」繞過 Rust 型別系統、直接餵合成投影的既有手法）把從
// `/api/state` 抓到的真實投影深拷貝一份、把 `qa` 的 `binding` 覆寫成跟 `be` 一樣指向
// `win`/`wJ:p1`，人為做出碰撞情境；`COCKPIT_PREVIEW_PUSH_MS` 設一個很長的值（10 分鐘），避免
// 背景推送迴圈在測試視窗內用「乾淨」的原始投影蓋掉這份合成投影。
async function partWorkstreamFocusIdentityUnique() {
  log('=== T. Finding 2：兩個 workstream 綁同一 pane 時「看輸出」焦點身分不碰撞 ===');
  let preview = null;
  let chrome = null;
  try {
    preview = await startPreview({ COCKPIT_PREVIEW_PUSH_MS: '600000' }, 'preview-T');
    const url = `http://127.0.0.1:${preview.port}/`;
    chrome = await startChrome(pickPort(19130, [preview.port]), url, 'chrome-T');
    const { cdp } = chrome;
    await cdp.waitFor(
      "typeof window.liveOutput === 'object' && typeof window.liveOutput.select === 'function'",
      5000,
      'output.js 載入完成，window.liveOutput 就緒'
    );
    await cdp.waitFor(
      "!!document.querySelector('.ff-row-header[data-workstream=\"be\"] [data-action=\"select-bound-pane\"]')",
      5000,
      "GIVEN：be 的「看輸出」已畫出（原始投影）"
    );

    // --- 合成「qa 也綁到 win/wJ:p1」的投影，直接餵給 window.onState ---
    const stateResp = await fetch(`http://127.0.0.1:${preview.port}/api/state`);
    const state = await stateResp.json();
    const project = state.projects.find((p) => p.id === 'cockpit');
    const be = project.workstreams.find((w) => w.id === 'be');
    const qa = project.workstreams.find((w) => w.id === 'qa');
    check(
      be.binding.state === 'bound' && be.binding.runtime === 'win' && be.binding.pane_id === 'wJ:p1',
      `GIVEN：fixture 的 be 應該綁 win/wJ:p1（實際 ${JSON.stringify(be.binding)}）`
    );
    qa.binding = {
      state: 'bound',
      runtime: 'win',
      pane_id: 'wJ:p1',
      source: 'override',
      agent: be.binding.agent,
      agent_status: be.binding.agent_status,
    };
    await cdp.eval(`window.onState(${JSON.stringify(state)}); true`);
    await cdp.waitFor(
      "!!document.querySelector('.ff-row-header[data-workstream=\"qa\"] [data-action=\"select-bound-pane\"]')",
      5000,
      '合成投影套用後 qa 也畫出「看輸出」'
    );

    const collision = await cdp.eval(`(() => {
      const beBtn = document.querySelector('.ff-row-header[data-workstream="be"] [data-action="select-bound-pane"]');
      const qaBtn = document.querySelector('.ff-row-header[data-workstream="qa"] [data-action="select-bound-pane"]');
      return {
        beRuntime: beBtn && beBtn.dataset.runtime,
        bePane: beBtn && beBtn.dataset.pane,
        qaRuntime: qaBtn && qaBtn.dataset.runtime,
        qaPane: qaBtn && qaBtn.dataset.pane,
        beSourceWorkstream: beBtn && beBtn.dataset.sourceWorkstream,
        qaSourceWorkstream: qaBtn && qaBtn.dataset.sourceWorkstream,
      };
    })()`);
    check(
      collision.beRuntime === 'win' &&
        collision.bePane === 'wJ:p1' &&
        collision.qaRuntime === 'win' &&
        collision.qaPane === 'wJ:p1',
      `GIVEN：be／qa 的「看輸出」現在應該指向同一個 runtime＋pane（碰撞前提，實際 ${JSON.stringify(collision)}）`
    );
    check(
      collision.beSourceWorkstream === 'be' && collision.qaSourceWorkstream === 'qa',
      `be／qa 的「看輸出」應該分別帶不同的 data-source-workstream（身分不碰撞，實際 ${JSON.stringify(collision)}）`
    );

    // --- focus 第二顆（qa 的）「看輸出」，等一次由合成投影再次觸發的重畫，斷言焦點還在 qa 上 ---
    await cdp.eval(
      "window.__oldNodeT = document.querySelector('.ff-row-header[data-workstream=\"qa\"] [data-action=\"select-bound-pane\"]'); window.__oldNodeT.focus(); true"
    );
    check(
      await cdp.eval(
        "document.activeElement === window.__oldNodeT && document.activeElement.dataset.sourceWorkstream === 'qa'"
      ),
      'qa 的「看輸出」可以取得焦點'
    );

    await cdp.eval(`window.onState(${JSON.stringify(state)}); true`);

    const after = await cdp.eval(`(() => {
      const active = document.activeElement;
      return {
        oldConnected: window.__oldNodeT.isConnected,
        sameNode: active === window.__oldNodeT,
        activeAction: active && active.dataset ? active.dataset.action : null,
        activeRuntime: active && active.dataset ? active.dataset.runtime : null,
        activePane: active && active.dataset ? active.dataset.pane : null,
        activeSourceWorkstream: active && active.dataset ? active.dataset.sourceWorkstream : null,
      };
    })()`);
    check(
      after.oldConnected === false,
      `舊節點應該已不在文件中（isConnected === false，證明真的被 replaceChildren 換過，實際 ${JSON.stringify(after)}）`
    );
    check(
      after.sameNode === false &&
        after.activeAction === 'select-bound-pane' &&
        after.activeRuntime === 'win' &&
        after.activePane === 'wJ:p1' &&
        after.activeSourceWorkstream === 'qa',
      `重畫後鍵盤焦點應該落在 qa 的「看輸出」新節點上，不是身分碰撞的 be（實際 ${JSON.stringify(after)}）`
    );
  } finally {
    await stopChrome(chrome, 'chrome-T');
    if (preview) {
      killTree(preview.server, 'preview-T');
      await sleep(300);
      check(!isPortListening(preview.port), `port ${preview.port}（preview-T）應該不再有 LISTENING 的行程`);
    }
  }
}

// U：Finding 1 [high]——actions.js 在 pointerdown 內同步 perform()→同步 repaint()；瀏覽器對
// 被按元素的預設聚焦發生在 pointerdown dispatch **之後**，所以 paint() 開始時
// document.activeElement 仍是先前聚焦的元素 A，重畫後被錯誤還原到新的 A；真正被按的 B（此時
// 已被 replaceChildren 換掉）沒有機會被聚焦，使用者接著按 Enter 可能重複觸發 A。
//
// render.js 的修法：actions.js 的 pointerdown 委派在呼叫 perform() 之前，把「即將被按下的
// 目標」交給 render.js（`window.cockpitFocusHint.setPendingTarget`），只對**這一次**同步觸發
// 的 paint() 有效——paint() 開始時優先讀這個待還原目標（讀到就用它的身分，不看
// document.activeElement；沒讀到才照舊看 document.activeElement），讀完不管有沒有用到都立刻
// 清空；pointerdown 的委派本身在 perform() 之後也會再清一次（雙重保險：萬一某個 action 沒有
// 觸發同步 repaint，也不會把這次的目標留到下一次重畫）。鍵盤路徑（keydown Enter／Space、
// <button> 原生 click）本來就已經是「焦點已經在目標上才觸發同步 repaint」，不需要這個 hint。
//
// 驗收：focus task be-1 的「推進」（A）→ 用 CDP.click 真的滑鼠按 task qa-1 的「推進」（B，
// 兩者都是 mark:none、非最後一個 stage，兩個不同 task）→ B 的操作應該有發生（一筆
// write-request）、鍵盤焦點的身分應該是 B（qa-1 的「推進」）而不是 A → 再送 Enter，斷言沒有
// 多出一筆 A 的 write-request。
async function partPointerdownFocusesRealTarget() {
  log('=== U. Finding 1：滑鼠按下另一個操作時，鍵盤焦點應該還原到真正被按的目標 ===');
  let preview = null;
  let chrome = null;
  try {
    preview = await startPreview({}, 'preview-U');
    const url = `http://127.0.0.1:${preview.port}/`;
    chrome = await startChrome(pickPort(19140, [preview.port]), url, 'chrome-U');
    const { cdp } = chrome;
    await cdp.waitFor(
      "typeof window.liveOutput === 'object' && typeof window.liveOutput.select === 'function'",
      5000,
      'output.js 載入完成，window.liveOutput 就緒'
    );
    await cdp.waitFor(
      "!!document.querySelector('[data-action=\"advance\"][data-project=\"cockpit\"][data-task=\"be-1\"]') && !!document.querySelector('[data-action=\"advance\"][data-project=\"cockpit\"][data-task=\"qa-1\"]')",
      5000,
      'GIVEN：be-1／qa-1 的「推進」按鈕都已畫出'
    );

    // --- GIVEN：焦點在 A（be-1 的「推進」）---
    await cdp.eval(
      "document.querySelector('[data-action=\"advance\"][data-project=\"cockpit\"][data-task=\"be-1\"]').focus(); true"
    );
    check(
      await cdp.eval(
        "(() => { const a = document.activeElement; return !!a && a.dataset.action === 'advance' && a.dataset.task === 'be-1'; })()"
      ),
      'GIVEN：be-1 的「推進」（A）可以取得焦點'
    );

    // --- WHEN：真的用滑鼠按 B（qa-1 的「推進」，跟 A 不是同一個元素）---
    preview.writeRequests.length = 0;
    await cdp.click('[data-action="advance"][data-project="cockpit"][data-task="qa-1"]');
    await sleep(500);

    // --- THEN：B 的操作真的送出、焦點身分是 B 不是 A ---
    const qa1Writes = preview.writeRequests.filter(
      (r) => r.method === 'POST' && r.path === '/api/projects/cockpit/tasks/qa-1/advance'
    );
    check(
      qa1Writes.length === 1,
      `按下 B（qa-1「推進」）後應該恰好收到一筆 POST /api/projects/cockpit/tasks/qa-1/advance（實際 ${JSON.stringify(preview.writeRequests)}）`
    );
    const afterClick = await cdp.eval(`(() => {
      const active = document.activeElement;
      return {
        action: active && active.dataset ? active.dataset.action : null,
        project: active && active.dataset ? active.dataset.project : null,
        task: active && active.dataset ? active.dataset.task : null,
      };
    })()`);
    check(
      afterClick.action === 'advance' && afterClick.project === 'cockpit' && afterClick.task === 'qa-1',
      `按下 B 之後鍵盤焦點的身分應該是 B（qa-1 的「推進」），不是 A（be-1，實際 ${JSON.stringify(afterClick)}）`
    );

    // --- 再送 Enter：不該多出一筆 A（be-1）的 write-request；應該是 B（qa-1）再送一筆 ---
    preview.writeRequests.length = 0;
    await cdp.pressKey('Enter', 'Enter', 13, '\r');
    await sleep(500);
    const be1WritesAfterEnter = preview.writeRequests.filter(
      (r) => r.method === 'POST' && r.path === '/api/projects/cockpit/tasks/be-1/advance'
    );
    check(
      be1WritesAfterEnter.length === 0,
      `送出 Enter 後不應該多出一筆 A（be-1）的 write-request（實際 ${JSON.stringify(preview.writeRequests)}）`
    );
    const qa1WritesAfterEnter = preview.writeRequests.filter(
      (r) => r.method === 'POST' && r.path === '/api/projects/cockpit/tasks/qa-1/advance'
    );
    check(
      qa1WritesAfterEnter.length === 1,
      `送出 Enter 後應該是焦點所在的 B（qa-1）再送一筆 write-request（實際 ${JSON.stringify(preview.writeRequests)}）`
    );
  } finally {
    await stopChrome(chrome, 'chrome-U');
    if (preview) {
      killTree(preview.server, 'preview-U');
      await sleep(300);
      check(!isPortListening(preview.port), `port ${preview.port}（preview-U）應該不再有 LISTENING 的行程`);
    }
  }
}

// V：fix round 2 Finding [medium]——actions.js 的 pointerdown 委派在 perform(target) 前後各設
// 一次「待還原目標」（見 window.cockpitFocusHint.setPendingTarget 上方註解），但清除
// （setPendingTarget(null)）原本寫在 perform(target) 呼叫之後，沒有 try/finally：perform() 的
// 同步呼叫鏈包含 window.liveOutput.select、repaint 與 DOM rendering，任一處拋錯都會跳過清除，
// 讓這個待還原目標殘留到下一次重畫——即使那次重畫由完全不同的原因（例如 /ws 推送新投影）觸發，
// 也會優先用這個殘留身分覆蓋掉當下真正的 document.activeElement，把焦點錯誤跳到早已按下失敗的
// 那個舊目標上。
//
// 修法：actions.js 的 pointerdown 委派改成 `try { perform(target); } finally {
// setPendingTarget(null); }`，例外照常往外拋（不吞）。
//
// 驗收：把 window.liveOutput.select 換成一個會同步丟例外的函式（perform() 處理 select-pane 時
// 會同步呼叫到它）→ focus A（task be-1 的「推進」）→ 用 CDP.click 真的按 pane 列 B（wJ:p1，
// select-pane，perform() 因為 window.liveOutput.select 拋錯而中止，這次同步呼叫鏈裡沒有走到
// repaint()，所以這次 pointerdown 本身不會觸發重畫；preventDefault() 已經在呼叫 perform() 之前
// 執行，瀏覽器不會替 B 補上原生聚焦，這裡的重點只是讓「待還原目標＝B」在沒有 finally 時卡在
// pendingFocusTarget 裡出不去）→ 還原 window.liveOutput.select → 模擬使用者把焦點移到第三個
// 元素 C（task qa-1 的「推進」，跟 A、B 都不同，選 C 而不是退回 A：即使當下真正的
// document.activeElement 明確是 C，沒有 finally 時殘留的 B 仍會在下一次重畫時把焦點搶走，比只
// 驗「退回 A」更能排除「其實是巧合退回原焦點」的疑慮）→ 直接呼叫 window.onState(現有投影)
// 觸發一次不依賴背景推送時機、由腳本完全控制的真實重畫（比照 T 段手法；COCKPIT_PREVIEW_PUSH_MS
// 設成 10 分鐘，避免背景推送在腳本控制的時間窗內意外提前消費掉殘留目標）→ 斷言重畫後鍵盤焦點的
// 身分是 C，不是殘留的 B。頁面會有一個未捕捉的例外印在 console（brief 預期內：pointerdown 委派
// 沒有包住 perform() 的呼叫，例外原本就會往外拋），透過 Input.dispatchMouseEvent（不經過
// Runtime.evaluate）觸發點擊，不會被 cdp.eval 的 exceptionDetails 檢查攔下，腳本不會因此誤判
// 失敗。
async function partPendingFocusTargetClearedOnPerformError() {
  log('=== V. fix round 2：perform() 拋錯不得殘留待還原焦點目標 ===');
  let preview = null;
  let chrome = null;
  try {
    preview = await startPreview({ COCKPIT_PREVIEW_PUSH_MS: '600000' }, 'preview-V');
    const url = `http://127.0.0.1:${preview.port}/`;
    chrome = await startChrome(pickPort(19150, [preview.port]), url, 'chrome-V');
    const { cdp } = chrome;
    await cdp.waitFor(
      "typeof window.liveOutput === 'object' && typeof window.liveOutput.select === 'function'",
      5000,
      'output.js 載入完成，window.liveOutput 就緒'
    );
    await cdp.waitFor(
      "!!document.querySelector('[data-action=\"advance\"][data-project=\"cockpit\"][data-task=\"be-1\"]') && !!document.querySelector('.pane-row[data-runtime=\"win\"][data-pane=\"wJ:p1\"]') && !!document.querySelector('[data-action=\"advance\"][data-project=\"cockpit\"][data-task=\"qa-1\"]')",
      5000,
      'GIVEN：be-1「推進」、wJ:p1 pane 列、qa-1「推進」都已畫出'
    );

    // --- 把 window.liveOutput.select 換成會同步丟例外的函式；perform() 處理 select-pane
    //     （B＝wJ:p1 pane 列）時會同步呼叫到它，模擬 perform() 內部拋錯。
    await cdp.eval(
      "window.__origSelectV = window.liveOutput.select; window.liveOutput.select = function () { throw new Error('注入的測試例外：模擬 perform() 拋錯（task focus-fix2 V 段）'); }; true"
    );

    // --- GIVEN：焦點在 A（be-1 的「推進」）---
    await cdp.eval(
      "document.querySelector('[data-action=\"advance\"][data-project=\"cockpit\"][data-task=\"be-1\"]').focus(); true"
    );
    check(
      await cdp.eval(
        "(() => { const a = document.activeElement; return !!a && a.dataset.action === 'advance' && a.dataset.task === 'be-1'; })()"
      ),
      'GIVEN：be-1 的「推進」（A）可以取得焦點'
    );

    // --- WHEN：真的用滑鼠按 B（wJ:p1 pane 列）；perform() 因為注入的例外而中止，這次
    //     pointerdown 本身不會觸發同步 repaint。---
    await cdp.click('.pane-row[data-runtime="win"][data-pane="wJ:p1"]');
    await sleep(200);

    // --- 還原 window.liveOutput.select ---
    await cdp.eval('window.liveOutput.select = window.__origSelectV; delete window.__origSelectV; true');

    // --- 模擬使用者把焦點移到第三個元素 C（qa-1 的「推進」，跟 A、B 都不同）---
    await cdp.eval(
      "window.__oldNodeV = document.querySelector('[data-action=\"advance\"][data-project=\"cockpit\"][data-task=\"qa-1\"]'); window.__oldNodeV.focus(); true"
    );
    check(
      await cdp.eval(
        "document.activeElement === window.__oldNodeV && document.activeElement.dataset.task === 'qa-1'"
      ),
      'C（qa-1 的「推進」）可以取得焦點'
    );

    // --- 觸發一次由投影更新造成的真實重畫（比照 T 段：直接把目前投影餵回 window.onState，
    //     不等背景推送——COCKPIT_PREVIEW_PUSH_MS 設成 10 分鐘，讓這次重畫的時機完全由腳本
    //     控制，不會被背景推送提早消費掉殘留的 pending target，或晚於腳本預期出現）---
    const state = await (await fetch(`http://127.0.0.1:${preview.port}/api/state`)).json();
    await cdp.eval(`window.onState(${JSON.stringify(state)}); true`);

    const after = await cdp.eval(`(() => {
      const active = document.activeElement;
      return {
        oldConnected: window.__oldNodeV.isConnected,
        sameNode: active === window.__oldNodeV,
        activeAction: active && active.dataset ? active.dataset.action : null,
        activeProject: active && active.dataset ? active.dataset.project : null,
        activeTask: active && active.dataset ? active.dataset.task : null,
      };
    })()`);
    check(
      after.oldConnected === false,
      `舊節點應該已不在文件中（isConnected === false，證明真的被 replaceChildren 換過，實際 ${JSON.stringify(after)}）`
    );
    check(
      after.activeAction === 'advance' && after.activeProject === 'cockpit' && after.activeTask === 'qa-1',
      `重畫後鍵盤焦點應該落在重畫前聚焦的 C（qa-1 的「推進」）上，不是殘留的待還原目標 B（wJ:p1 pane 列，實際 ${JSON.stringify(after)}）`
    );
  } finally {
    await stopChrome(chrome, 'chrome-V');
    if (preview) {
      killTree(preview.server, 'preview-V');
      await sleep(300);
      check(!isPortListening(preview.port), `port ${preview.port}（preview-V）應該不再有 LISTENING 的行程`);
    }
  }
}

// X（fix round 1；設計審核 M1／控制端 Ruling R41）：長標題（runtime／pane id 都很長）疊上
// 「過期」標籤（同時觸發 is-stale，重現設計審核 `700x900-7-long-title-stale-panel.png` 的最壞
// 情況——過期標籤跟截斷提示一起佔用標題列的可縮空間）時，「取消選取」不應該被壓縮成兩行。
// 700 寬窄視窗（同設計審核用的寬度）。用不存在的 runtime／pane 直接 select()：真的請求會 404，
// 走 markGone() 進入 is-stale（不需要真的建立那麼長 id 的 pane）。
async function partLongTitleCloseButtonSingleLine() {
  log('=== X. fix round 1（設計審核 M1）：長標題時「取消選取」不應該被擠成兩行 ===');
  let preview = null;
  let chrome = null;
  try {
    preview = await startPreview({}, 'preview-X');
    const url = `http://127.0.0.1:${preview.port}/`;
    chrome = await startChrome(pickPort(19160, [preview.port]), url, 'chrome-X', '700,900');
    const { cdp } = chrome;
    await cdp.waitFor(
      "typeof window.liveOutput === 'object' && typeof window.liveOutput.select === 'function'",
      5000,
      'output.js 載入完成，window.liveOutput 就緒'
    );

    // 基準：短標題（真的存在的 pane）時「取消選取」的高度與 client rect 數（單行）。
    await cdp.eval("window.liveOutput.select('win', 'wJ:p1'); true");
    await cdp.waitFor(PANEL_OPEN_JS, 3000, '短標題選取後面板顯示');
    const baseline = await cdp.eval(`(() => {
      const btn = document.querySelector('.output-close');
      const r = btn.getBoundingClientRect();
      return { height: r.height, rects: btn.getClientRects().length };
    })()`);
    check(baseline.rects === 1, `基準（短標題）「取消選取」本來就應該是單行（實際 ${baseline.rects}）`);

    // 長標題＋不存在的 pane：404 → markGone() → is-stale，標題列同時多一個「過期」標籤（M1 的
    // 重現條件：截斷提示與過期標籤一起擠壓可縮空間，這裡疊過期標籤已經是比只有長標題更嚴苛的
    // 情況）。
    const longRuntime = 'a-very-long-runtime-name-that-keeps-going-xxxxxxxxxxxxxxxxxxxxxxxxxxxxxx';
    const longPane = 'w1:p-a-very-long-pane-id-that-also-keeps-going-yyyyyyyyyyyyyyyyyyyyyyyyyyyy';
    await cdp.eval(`window.liveOutput.select(${JSON.stringify(longRuntime)}, ${JSON.stringify(longPane)}); true`);
    await cdp.waitFor(
      "(() => { const n = document.querySelector('.output-gone-notice'); return !!n && !n.hidden; })()",
      5000,
      '不存在的 pane 應該 404 → 顯示「pane 已不存在」（同時進入 is-stale，標題列多一個「過期」標籤）'
    );
    const longTitleSignals = await readStaleSignals(cdp);
    checkMarkedStale(longTitleSignals, '長標題＋不存在的 pane（M1 重現條件）');

    const closeStyle = await cdp.eval(`(() => {
      const btn = document.querySelector('.output-close');
      const cs = getComputedStyle(btn);
      const r = btn.getBoundingClientRect();
      return {
        flexShrink: cs.flexShrink,
        whiteSpace: cs.whiteSpace,
        text: btn.textContent,
        height: r.height,
        rects: btn.getClientRects().length,
      };
    })()`);
    check(
      closeStyle.flexShrink === '0',
      `「取消選取」不應該被壓縮（flex-shrink 應該是 0；設計審核 M1；實際 ${closeStyle.flexShrink}）`
    );
    check(
      closeStyle.whiteSpace === 'nowrap',
      `「取消選取」不應該換行（white-space 應該是 nowrap；設計審核 M1；實際 ${closeStyle.whiteSpace}）`
    );
    check(
      closeStyle.text === '取消選取',
      `按鈕文字仍應該是完整的「取消選取」（實際 ${JSON.stringify(closeStyle.text)}）`
    );
    check(
      closeStyle.rects === 1,
      `長標題下「取消選取」的文字仍應該落在單一個 client rect 裡（沒有折成兩行；設計審核 M1；實際 ${closeStyle.rects}）`
    );
    check(
      Math.abs(closeStyle.height - baseline.height) < 0.5,
      `長標題下「取消選取」的高度應該跟短標題時相同（沒有變成兩行高；設計審核 M1；短標題 ${baseline.height}，長標題 ${closeStyle.height}）`
    );

    // 折行的可縮空間讓給標題（ellipsis），不是按鈕。
    const titleOverflow = await cdp.eval(`(() => {
      const t = document.querySelector('.output-title');
      return { scrollWidth: t.scrollWidth, clientWidth: t.clientWidth, textOverflow: getComputedStyle(t).textOverflow };
    })()`);
    check(
      titleOverflow.textOverflow === 'ellipsis' && titleOverflow.scrollWidth > titleOverflow.clientWidth,
      `長標題應該用 ellipsis 截斷（折行的可縮空間讓給標題，不是按鈕；實際 ${JSON.stringify(titleOverflow)}）`
    );

    await cdp.eval('window.liveOutput.clear(); true');
  } finally {
    await stopChrome(chrome, 'chrome-X');
    if (preview) {
      killTree(preview.server, 'preview-X');
      await sleep(300);
      check(!isPortListening(preview.port), `port ${preview.port}（preview-X）應該不再有 LISTENING 的行程`);
    }
  }
}

// letter → 段落函式；task 5.4 加 ONLY 篩選（見檔頭「用法」），逐一 try/catch 維持跟之前一樣
// 「一段中止不影響其他段落繼續跑」的行為，只是把原本重複的六段 try/catch 收成一個迴圈。
const PARTS = [
  ['A', partDefaultModes],
  ['B', partOverriddenModes],
  ['C', partSelectionBasics],
  ['D', partFrequentRepaint],
  ['E', partPanelOverlap],
  ['F', partContentCatchesUp],
  ['G', partOldResponseNotOverwrite],
  ['H', partScrollBehaviorUpNotPulledBack],
  ['I', partScrollBehaviorStickToBottom],
  ['J', partPaneVanishes],
  ['K', partOutputEndpoint404],
  ['L', partRuntimeRecovers],
  ['M', partSameContentClearsStale],
  ['N', partSwitchNotBlockedBySlowPane],
  ['O', partFrontendTimeoutDoesNotFreeze],
  ['P', partSelectionDoesNotSwallowWriteError],
  ['Q', partGenerationGuardStillWorks],
  ['R', partFocusPreservedAcrossRepaint],
  ['S', partFocusNotStolenByVanishedTarget],
  ['T', partWorkstreamFocusIdentityUnique],
  ['U', partPointerdownFocusesRealTarget],
  ['V', partPendingFocusTargetClearedOnPerformError],
  ['W', partHelperSelfTest],
  ['X', partLongTitleCloseButtonSingleLine],
];

async function main() {
  if (!fs.existsSync(CHROME)) {
    throw new Error(`找不到 Chrome：${CHROME}（可用環境變數 COCKPIT_CHROME 指定路徑）`);
  }
  for (const [letter, fn] of PARTS) {
    if (!shouldRun(letter)) continue;
    try {
      await fn();
    } catch (e) {
      check(false, `${letter} 段中止：${e.message}`);
    }
  }
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
