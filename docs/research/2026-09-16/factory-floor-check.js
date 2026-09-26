// Task 5.2 驗收：Factory Floor 網格位置與 Project 上下順序（spec cockpit-dashboard
// 「Factory Floor」「Scenario D 的畫面」「兩個 Project」「未知 status 不破壞畫面」）。
//
// Fix round 1（Codex review：「驗收證據不足」，控制端視為 Important）：round 0 只斷言
// 「格子裡有沒有任意 task-node」「running 節點總數」「Project 順序」，沒有逐項比對每個
// task 節點的狀態 class／文字、沒有涵蓋 blocked／ready／failed／未知 status、沒有涵蓋
// bound(override)／unbound 兩種綁定、也沒有斷言 warning 內容。這一版把這些全部補上：
//   - `cockpit/tests/fixtures/projected-state.json` 的第一個 project（"cockpit"）補了
//     `qa`（bound + override，測「改綁」標示）、`release`（unbound）兩條 workstream，
//     以及 `qa-1`（blocked）、`release-1`（ready）、`ops-2`（failed）三個 task——連同原本
//     的 running／pending／completed，`StageStatus` 六個已知值現在都有真實樣本。Scenario D
//     的三個 workstream／task 完全沒動。
//   - 「未知 status」無法塞進這份 fixture：`ProjectedTask.status` 的 Rust 型別是
//     `StageStatus`，一個沒有 `#[serde(other)]` catch-all 的封閉 enum，寫一個列舉值以外的
//     字串會讓 `serde_json::from_str::<ProjectedState>` 直接失敗——`cockpit/tests/fixture.rs`
//     與 `ui_preview.rs` 啟動時都會 panic，不是「這裡懶得補」而是型別系統本來就不允許。
//     這個情境只能繞過 Rust 型別系統，直接對 `render.js` 的公開入口 `window.onState` 餵一個
//     手寫的 JS 物件（不經過 Rust 序列化），做法見下面的 `scenarioUnknownStatus`。
//
// Fix round 2（Codex review：「驗收證據不足」，Important）：round 1 的 `assertTaskNode` 只
// 比對 class 名稱、狀態文字、標題文字，沒有量測實際顏色——如果 `style.css` 的 CSS 變數遺失
// 或兩個顏色互換（例如 `.task-status-failed` 誤接到 `--stage-completed`），class 名稱與
// 文字都還是對的，round 1 的斷言會全部 PASS，但畫面顏色是錯的。這一版新增：
//   - `EXPECTED_COLORS`／`hexToRgb`：六種已知 status＋`unknown` 在 `style.css` 定義的實際
//     hex 顏色（承載顏色的屬性是 `.task-status-*` 規則的 `background` shorthand，只給顏色
//     值等同只設 `background-color`），換算成 `getComputedStyle` 會回傳的 `rgb(r, g, b)`
//     格式。
//   - `scenarioGridAndCoverage` 內新增一段：dump-dom 是序列化文字，量不到 computed style，
//     另外開一個「真的」headless Chrome（`--remote-debugging-port`，不是 `--dump-dom`）連到
//     同一個 ui_preview，用 CDP 對六種已知 status 的節點各自 `getComputedStyle(...)
//     .backgroundColor`，逐一比對 RGB 值。
//   - `scenarioUnknownStatus` 的 harness 補上 `<link rel="stylesheet" href="/app/style.css">`
//     （round 1 版本沒有載入樣式表，`getComputedStyle` 只會量到瀏覽器預設值）並且新增
//     `/app/style.css` 路由，對「對照組 running 節點」與「未知 status 節點」都補上顏色比對。
//
// direction-01-visual task 3.2（Factory Floor 換皮）：節點改成 --surface 底＋左緣狀態色條＋
// aria-hidden 的狀態符號 span＋status 文字，標題 span 多了 title 屬性，bound 的綁定摘要拆成
// runtime／分隔／pane 三個子 span。依 tasks.md 3.2 把「逐字 HTML 字串斷言」與「顏色斷言」改成
// 以 DOM 結構與屬性判斷（全部改在即時 CDP session 裡讀 DOM，不再對 --dump-dom 的序列化字串做
// regex）：
//   - 六種已知 status 的 class／status 文字／標題：原本 regex 比對 `class="task-title">標題<`
//     等片段，改成讀 `.task-node` 的 classList、`.task-status-label`／`.task-title` 的
//     textContent，另外斷言 `.task-status-symbol` 存在且 aria-hidden（spec「狀態不只靠顏色」）。
//   - 六種已知 status 的實際顏色：節點不再是狀態色實底（spec「節點以表面色為底、左緣一條狀態
//     色條……狀態文字與色條使用狀態色」），原本讀 backgroundColor，改成讀左緣色條
//     （borderLeftColor）與 status 文字（.task-status-label 的 color），並斷言節點底色是
//     --surface。EXPECTED_COLORS 的值不變，仍能抓到「兩個狀態色互換」。
//   - 綁定摘要：原本 regex 比對 `class="ff-binding-text">文字<`，改成讀 .ff-binding-text 的
//     textContent（bound 時是三個子 span 串起來的「runtime / pane」全文）與 .ff-binding-badge。
//   - 未知 status（情境二）：原本 `html.includes('class="task-node task-status-unknown"
//     title="whatever"')` 等逐字片段，改成讀節點的 className、title 屬性、status 文字與標題；
//     顏色從 backgroundColor 改成 status 文字色＋虛線外框（spec「以次要文字色與虛線外框顯示
//     原字串」），對照組 running 改讀色條與 status 文字色。
// --dump-dom 仍用來驗 warning 內容、左欄 Project 順序、只畫一張 Factory Floor、.projects 在
// .runtime-cards 之前（這幾條不受本 task 影響，逐字斷言維持原樣）。
//
// 做法：直接執行 `cargo build -p cockpit --example ui_preview` 產生的執行檔
// （target/debug/examples/ui_preview.exe），不透過 `cargo run` 啟動——`cargo run` 會多一層
// wrapper 行程，實際監聽 port 的是它的子行程，PID 追蹤與收尾容易對不上；直接執行 build
// 好的執行檔可以拿到單一、穩定的 PID 做收尾。這跟文件裡寫的
// `cargo run -p cockpit --example ui_preview` 啟動的是同一支程式、同一份 fixture，只是省了
// cargo 那層 wrapper。
//
// headless Chrome 用 `--dump-dom --virtual-time-budget=<ms>`：讓 Chrome 在收到 index.html
// 的 load 事件後，繼續跑一段虛擬時間（WebSocket 連線、收到第一份投影、render.js 整頁重畫
// 都在這段時間內完成），時間到才把最終 DOM 序列化成 HTML 字串輸出到 stdout。（格子／列首
// 內容的比對在 direction-01-visual task 3.2 改到即時 CDP session 讀 DOM，見檔頭說明。）
//
// `scenarioUnknownStatus` 不透過 ui_preview／dump-dom，改用 CDP（跟 task 5.1
// channel-backoff-check.js 同一套手法）：起一個只服務兩個檔案（harness html＋真正的
// render.js 檔案內容）的極簡 http server，headless Chrome 開起來後用 CDP
// `Runtime.evaluate` 直接呼叫 `window.onState(syntheticState)`（render.js 唯一暴露在
// window 上、會觸發整頁重畫的入口），再讀 DOM 斷言（task 3.2 前是讀 `#app` 的 innerHTML）。
//
// 用法（repo 根，需先 `cargo build -p cockpit --example ui_preview`）：
//   node docs/research/2026-09-16/factory-floor-check.js
// 清理：只終止本腳本自己 spawn 的 ui_preview.exe／chrome.exe（依 PID），不依映像名稱全域
// taskkill；沿用 task 5.1 channel-backoff-check.js 的 killTree／tasklist 收尾判準
// （taskkill 對多行程行程樹可能回報非 0，用 tasklist 覆核主行程是否真的不在了才算數）。
const http = require('node:http');
const os = require('node:os');
const { spawn, spawnSync } = require('node:child_process');
const path = require('node:path');
const fs = require('node:fs');

const REPO = path.resolve(__dirname, '..', '..', '..');
const UI_PREVIEW_EXE = path.join(REPO, 'target', 'debug', 'examples', 'ui_preview.exe');
const RENDER_JS_PATH = path.join(REPO, 'cockpit', 'assets', 'app', 'render.js');
const STYLE_CSS_PATH = path.join(REPO, 'cockpit', 'assets', 'app', 'style.css');
const CHROME =
  process.env.COCKPIT_CHROME || 'C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe';
const SCREENSHOT_PATH = path.join(__dirname, 'task-5.2-scenario-d.png');

// fix round 2（Codex review：class／文字對了不代表 CSS 顏色真的對——變數名寫對、值接錯照樣
// 全部 PASS）：六種已知 status＋unknown 各自在 style.css 定義的實際顏色（hex），承載顏色的
// CSS 屬性是 `background`（shorthand，只給顏色值＝只設 `background-color`，見
// `cockpit/assets/app/style.css` 的 `.task-status-*` 規則），所以斷言讀
// `getComputedStyle(node).backgroundColor`。
//
// direction-01-visual task 2.2 fix round 1（控制端 Ruling R20，採 Codex high／medium）：
// spec `cockpit-dashboard`「Direction 01 視覺語彙」與 design D4「對象／狀態→顏色與符號」
// task 對照表一直都要求畫面只用 10 個核心色彩 token；2.2 首輪把舊 GitHub 深色主題色盤
// （--status-*／--stage-*）留給後續 task，被 Codex 與設計審核判定為未落實「把既有規則改為
// 取用 token」，R20 裁決 2.2 本輪就要收斂。下面六個值全部換成 design D4 對照表指定的核心
// token（running→--accent、blocked→--warn、ready→--text、pending／unknown→--text-dim、
// failed→--bad、completed→--ok），是同一個 spec scenario（Factory Floor Requirement）
// 底下、承載色值的 token 換了，不是新增或放寬斷言。
// direction-01-visual task 3.2：承載狀態色的屬性從節點 background 改成左緣色條
// （border-left-color）與 status 文字（.task-status-label 的 color），值不變；節點底色一律
// 是 SURFACE_COLOR。
const SURFACE_COLOR = '#142338'; // --surface
const EXPECTED_COLORS = {
  running: '#63d5e8', // --accent
  blocked: '#e9bc73', // --warn
  ready: '#e5edf3', // --text
  pending: '#a3b7c9', // --text-dim（design D4：pending／unknown 同一個 token）
  failed: '#f47279', // --bad
  completed: '#39d5ac', // --ok（刻意不是 --status-done／pane done 的顏色，spec 明文兩者不得同色）
  unknown: '#a3b7c9', // --text-dim（task-status-unknown fallback，跟 pending 同一個值）
};

function hexToRgb(hex) {
  const n = parseInt(hex.slice(1), 16);
  return `rgb(${(n >> 16) & 255}, ${(n >> 8) & 255}, ${n & 255})`;
}

const failures = [];
function check(cond, label) {
  console.log(`${cond ? 'ok  ' : 'FAIL'} ${label}`);
  if (!cond) failures.push(label);
}
const log = (s) => console.log(`[${new Date().toISOString()}] ${s}`);
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

function spawnTracked(cmd, args, label, opts) {
  const child = spawn(cmd, args, { stdio: 'ignore', windowsHide: true, ...(opts || {}) });
  child.on('error', (e) => check(false, `${label} spawn error：${e.message}`));
  return child;
}

function pidStillRunning(pid) {
  const r = spawnSync('tasklist', ['/FI', `PID eq ${pid}`, '/NH'], { encoding: 'utf8' });
  return typeof r.stdout === 'string' && r.stdout.includes(String(pid));
}

function killTree(child, label) {
  if (!child || child.exitCode !== null) return;
  spawnSync('taskkill', ['/PID', String(child.pid), '/T', '/F'], { encoding: 'utf8' });
  // 見檔頭註解：taskkill 對行程樹可能回報非 0，用 tasklist 查主行程 PID 還在不在才是
  // 真正的清理判準。
  check(!pidStillRunning(child.pid), `${label} PID ${child.pid} 已終止（tasklist 查無此 PID）`);
}

// 7770 是 ui_preview 的預設埠；task 5.2 brief 要求「7770 若被占用改用別的埠」，這裡直接
// 探測，遇到已在 LISTENING 的埠就往上加一。
function isPortListening(port) {
  const r = spawnSync('netstat', ['-ano'], { encoding: 'utf8' });
  const needle = `127.0.0.1:${port} `;
  return (r.stdout || '')
    .split('\n')
    .some((line) => line.includes(needle) && line.includes('LISTENING'));
}

function pickPort(start) {
  let port = start;
  while (isPortListening(port)) {
    port += 1;
  }
  return port;
}

async function waitForServer(port, deadlineMs) {
  const start = Date.now();
  while (Date.now() - start < deadlineMs) {
    try {
      const r = await fetch(`http://127.0.0.1:${port}/api/state`);
      if (r.ok) return true;
    } catch {
      // 伺服器還沒起來，繼續等。
    }
    await sleep(200);
  }
  return false;
}

// direction-01-visual task 3.2：原本在這裡的 extractBalancedDiv／extractCellHtml／
// extractRowHeaderHtml／assertTaskNode／assertBinding（對 --dump-dom 字串做括號計數與 regex
// 逐字比對）已移除，同樣的比對改在即時 CDP session 裡讀 DOM，見 scenarioGridAndCoverage 的
// assertLiveTaskNode／assertLiveBinding。

// 一個 task 節點的 DOM 結構與屬性（兩個情境共用）：classList、status 文字、標題、狀態符號
// span（aria-hidden）、左緣色條色、status 文字色、節點底色、外框樣式、title 屬性。
const NODE_INFO_JS = `function (node) {
  if (!node) return null;
  var cs = getComputedStyle(node);
  var label = node.querySelector('.task-status-label');
  var title = node.querySelector('.task-title');
  var symbol = node.querySelector('.task-status-symbol');
  return {
    className: node.className,
    titleAttr: node.getAttribute('title'),
    statusText: label ? label.textContent : null,
    titleText: title ? title.textContent : null,
    symbolText: symbol ? symbol.textContent : null,
    symbolHidden: symbol ? symbol.getAttribute('aria-hidden') : null,
    stripeColor: cs.borderLeftColor,
    labelColor: label ? getComputedStyle(label).color : null,
    background: cs.backgroundColor,
    borderStyle: cs.borderStyle,
    borderTopColor: cs.borderTopColor,
    borderWidths: [cs.borderTopWidth, cs.borderRightWidth, cs.borderBottomWidth, cs.borderLeftWidth].map(parseFloat),
  };
}`;

async function scenarioGridAndCoverage() {
  log('=== 情境一：Factory Floor 網格位置、六種已知 status、五種綁定摘要（含 override）、Project 順序、warning ===');
  const port = pickPort(7770);
  if (port !== 7770) {
    log(`7770 已被占用，改用埠 ${port}`);
  }

  let server = null;
  try {
    if (!fs.existsSync(UI_PREVIEW_EXE)) {
      throw new Error(
        `找不到 ${UI_PREVIEW_EXE}，請先跑 cargo build -p cockpit --example ui_preview`
      );
    }
    server = spawnTracked(UI_PREVIEW_EXE, [], 'ui_preview', {
      env: { ...process.env, COCKPIT_PREVIEW_LISTEN: `127.0.0.1:${port}` },
    });
    const up = await waitForServer(port, 10000);
    check(up, `ui_preview 應該在 10 秒內開始回應 /api/state（port ${port}）`);
    if (!up) {
      throw new Error('ui_preview 沒有如期起來，中止後續斷言');
    }

    const dump = spawnSync(
      CHROME,
      [
        '--headless=new',
        '--disable-gpu',
        '--no-first-run',
        '--virtual-time-budget=4000',
        '--dump-dom',
        `http://127.0.0.1:${port}/`,
      ],
      { encoding: 'utf8', maxBuffer: 32 * 1024 * 1024 }
    );
    check(dump.status === 0, `--dump-dom 應該成功結束（實際 exit ${dump.status}）`);
    const html = dump.stdout || '';
    check(html.includes('factory-floor'), 'dump-dom 輸出應該含 Factory Floor 網格');

    // direction-01-visual task 3.1：中上區域改成只顯示目前選定的 Project（spec「兩個
    // Project」：「中上區域只有一張 Factory Floor……另一個的網格不在畫面上，改由左欄切換」）；
    // `--dump-dom` 是單次靜態快照、不能點擊切換，所以這裡只能驗到未選定過時預設顯示的第一個
    // Project（cockpit）。Scenario D（Project `p`）的網格位置與 workstream `frontend` 的
    // 「歧義」綁定摘要改到下面「六種已知 status 的實際顏色」那段共用的即時 CDP session 裡，
    // 先點左欄切到 `p` 再驗（見下方「Scenario D 網格位置（即時 CDP，切換 Project 後）」）。

    // 六種已知 status 逐項比對 class／文字（fix round 1）：direction-01-visual task 3.2 起改在
    // 下面的即時 CDP session 以 DOM 結構判斷（assertLiveTaskNode），這裡只留 running 節點總數。
    const runningNodes = (html.match(/task-status-running/g) || []).length;
    check(
      runningNodes === 1,
      `未選定過時預設顯示 cockpit，畫面上應該只有 cockpit 專案的 1 個 running 節點（Scenario D 的 3 個屬於另一個 Project、要切換過去才看得到，實際 ${runningNodes}）`
    );

    // --- 六種已知 status 的實際顏色（fix round 2）---
    // class／文字對了不代表 CSS 顏色真的對：CSS 變數遺失或互換仍會讓上面兩段全部 PASS。
    // dump-dom 的輸出是序列化文字，量不到 getComputedStyle，這裡另外開一個「真的」headless
    // Chrome（帶 --remote-debugging-port，不是 --dump-dom）連到同一個 ui_preview，用 CDP
    // 讀 getComputedStyle(node).backgroundColor，跟 style.css 定義的 RGB 值逐一比對。
    log('--- 六種已知 status 的實際顏色（getComputedStyle）---');
    const colorCdpPort = pickPort(18795);
    let colorChrome = null;
    let colorWs = null;
    let colorUdd = null;
    try {
      colorUdd = fs.mkdtempSync(path.join(os.tmpdir(), 'cockpit-chrome-colors-'));
      colorChrome = spawnTracked(
        CHROME,
        [
          '--headless=new',
          '--disable-gpu',
          '--no-first-run',
          `--remote-debugging-port=${colorCdpPort}`,
          '--remote-allow-origins=*',
          `--user-data-dir=${colorUdd}`,
          '--window-size=1400,1600',
          `http://127.0.0.1:${port}/`,
        ],
        'chrome-colors'
      );
      const attached = await attachCdp(colorCdpPort, `http://127.0.0.1:${port}/`);
      colorWs = attached.ws;
      const cdp = attached.cdp;

      let ready = false;
      for (let i = 0; i < 50 && !ready; i++) {
        ready = await cdp.eval("document.querySelectorAll('.task-node').length >= 9");
        if (!ready) await sleep(100);
      }
      check(ready, '即時頁面應該在 5 秒內畫出全部 9 個 task 節點');

      // 逐項比對一個 task 節點的狀態 class、狀態文字、標題文字（fix round 1 的核心訴求：不能只
      // 斷言「有沒有節點」，要斷言「這個節點是不是這個狀態」）。direction-01-visual task 3.2：
      // 原本對 --dump-dom 字串 regex 比對 `class="task-node task-status-X"`、
      // `class="task-status-label">X<`、`class="task-title">標題<`，改成讀 DOM：classList 恰好是
      // task-node＋task-status-X、.task-status-label／.task-title 的 textContent 完全相等，另外
      // 斷言狀態符號 span 存在且 aria-hidden（spec「狀態不只靠顏色」：task status 另有符號）。
      async function assertLiveTaskNode(ws, stage, status, title) {
        const cellSel = `.ff-cell[data-workstream="${ws}"][data-stage="${stage}"]`;
        const info = await cdp.eval(`(() => {
          const cell = document.querySelector(${JSON.stringify(cellSel)});
          return (${NODE_INFO_JS})(cell ? cell.querySelector('.task-node') : null);
        })()`);
        check(info !== null, `(${ws}, ${stage}) 應該有 task 節點可供比對`);
        if (info === null) return null;
        check(
          info.className === `task-node task-status-${status}`,
          `(${ws}, ${stage}) 節點的 class 應該恰好是 "task-node task-status-${status}"（實際 ${JSON.stringify(info.className)}）`
        );
        check(info.statusText === status, `(${ws}, ${stage}) 節點的狀態文字應該是 "${status}"（實際 ${JSON.stringify(info.statusText)}）`);
        check(info.titleText === title, `(${ws}, ${stage}) 節點的標題應該是 "${title}"（實際 ${JSON.stringify(info.titleText)}）`);
        check(
          info.symbolText !== null && info.symbolText !== '' && info.symbolHidden === 'true',
          `(${ws}, ${stage}) 節點應該有 aria-hidden 的狀態符號 span（實際 ${JSON.stringify({ t: info.symbolText, h: info.symbolHidden })}）`
        );
        return info;
      }

      // 六種已知 status：class／文字（原 dump-dom 版 assertTaskNode）＋實際顏色（fix round 2；
      // task 3.2 起讀色條與 status 文字色，節點底色是 --surface）。
      log('--- 六種已知 status（class／文字／符號逐項比對＋getComputedStyle 顏色）---');
      const knownCells = [
        ['be', 'Implement', 'running', '投影擴充'],
        ['docs', 'Spec', 'pending', 'README'],
        ['ops', 'Review', 'completed', '部署腳本'],
        ['qa', 'Implement', 'blocked', '程式碼審查'],
        ['release', 'Spec', 'ready', '發布準備'],
        ['ops', 'Spec', 'failed', '上線檢查'],
      ];
      for (const [ws, stage, status, title] of knownCells) {
        const info = await assertLiveTaskNode(ws, stage, status, title);
        if (info === null) continue;
        const expected = hexToRgb(EXPECTED_COLORS[status]);
        check(
          info.stripeColor === expected,
          `(${ws}, ${stage}) 節點（${status}）的左緣色條 border-left-color 應該是 ${expected}（style.css ${EXPECTED_COLORS[status]}，實際 ${info.stripeColor}）`
        );
        check(
          info.labelColor === expected,
          `(${ws}, ${stage}) 節點（${status}）的 status 文字顏色應該是 ${expected}（實際 ${info.labelColor}）`
        );
        check(
          info.background === hexToRgb(SURFACE_COLOR),
          `(${ws}, ${stage}) 節點（${status}）的底色應該是 --surface ${hexToRgb(SURFACE_COLOR)}（實際 ${info.background}）`
        );
      }

      // --- 綁定摘要文字（含 override 標示）逐項比對（fix round 1）---
      // direction-01-visual task 3.2：原本對 dump-dom 字串 regex 比對
      // `class="ff-binding-text">文字<` 與 `class="ff-binding-badge">改綁<`；bound 的摘要拆成
      // runtime／分隔／pane 三個子 span 後，改成讀 .ff-binding-text 的 textContent（全文）與
      // .ff-binding-badge 的 textContent。frontend（歧義／2）屬於 Project p，在下面切換過去後驗。
      log('--- 五種綁定摘要文字（含 override）---');
      async function assertLiveBinding(ws, expectedText, expectBadge) {
        const b = await cdp.eval(`(() => {
          const row = document.querySelector('.ff-row-header[data-workstream="${ws}"]');
          if (!row) return null;
          const text = row.querySelector('.ff-binding-text');
          const badge = row.querySelector('.ff-binding-badge');
          return { text: text ? text.textContent : null, badge: badge ? badge.textContent : null };
        })()`);
        check(b !== null, `應該找到 workstream 列首 (data-workstream="${ws}")`);
        if (b === null) return;
        check(b.text === expectedText, `workstream ${ws} 的綁定摘要應該是 "${expectedText}"（實際 ${JSON.stringify(b.text)}）`);
        const hasBadge = b.badge === '改綁';
        check(
          hasBadge === expectBadge && (expectBadge || b.badge === null),
          `workstream ${ws} ${expectBadge ? '應該' : '不應該'} 顯示「改綁」標示（實際 ${JSON.stringify(b.badge)}）`
        );
      }
      await assertLiveBinding('be', 'win / wJ:p1', false);
      await assertLiveBinding('docs', 'runtime 未連線', false);
      await assertLiveBinding('ops', '無綁定', false);
      await assertLiveBinding('qa', 'win / wJ:p3', true);
      await assertLiveBinding('release', '未綁定', false);

      // --- Scenario D 網格位置（即時 CDP，切換 Project 後；direction-01-visual task 3.1）---
      // `--dump-dom` 是單次靜態快照、無法點擊，Scenario D（Project `p`）的網格改在這個「真的」
      // headless Chrome 上，先點左欄切到 `p`（design D6：data-action="select-project"），
      // 等 Factory Floor 真的換成 `p` 的網格後再驗三個節點的（列, 欄）與其餘六格沒有節點，
      // 跟原本 dump-dom 版本比對的內容完全對應，只是換了取值方式（cdp.eval 讀即時 DOM，不是
      // 序列化字串）。同一個 session 沿用同一份 fixture（cockpit／p 兩個 Project），不需要
      // 另外注入投影。
      log('--- Scenario D 網格位置（即時 CDP，切換 Project 後）---');
      await cdp.eval('document.querySelector(\'[data-action="select-project"][data-project="p"]\').click(); true');
      let switchedToP = false;
      for (let i = 0; i < 50 && !switchedToP; i++) {
        switchedToP = await cdp.eval('!!document.querySelector(\'.project[data-project="p"]\')');
        if (!switchedToP) await sleep(100);
      }
      check(switchedToP, '點左欄的 p 之後，Factory Floor 應該在 5 秒內換成 Scenario D 的網格');

      const scenarioDCells = [
        ['backend', 'Plan', false],
        ['backend', 'Implement', true],
        ['backend', 'Test', false],
        ['frontend', 'Plan', true],
        ['frontend', 'Implement', false],
        ['frontend', 'Test', false],
        ['tests', 'Plan', false],
        ['tests', 'Implement', false],
        ['tests', 'Test', true],
      ];
      for (const [ws, stage, expectNode] of scenarioDCells) {
        const cellSel = `.ff-cell[data-workstream="${ws}"][data-stage="${stage}"]`;
        const cellInfo = await cdp.eval(`(() => {
          const cell = document.querySelector(${JSON.stringify(cellSel)});
          if (!cell) return null;
          return { hasNode: !!cell.querySelector('.task-node') };
        })()`);
        check(cellInfo !== null, `應該找到格子 (${ws}, ${stage})`);
        const hasNode = !!cellInfo && cellInfo.hasNode;
        check(
          hasNode === expectNode,
          `(${ws}, ${stage}) ${expectNode ? '應該有' : '不應該有'} task 節點（實際${hasNode ? '有' : '沒有'}）`
        );
      }

      await assertLiveTaskNode('backend', 'Implement', 'running', '後端實作');
      await assertLiveTaskNode('frontend', 'Plan', 'running', '前端規劃');
      await assertLiveTaskNode('tests', 'Test', 'running', '測試執行');

      const runningNodesInP = await cdp.eval("document.querySelectorAll('.task-status-running').length");
      check(
        runningNodesInP === 3,
        `切到 p 之後，畫面上應該只有 Scenario D 的 3 個 running 節點（cockpit 的已經不在畫面上，實際 ${runningNodesInP}）`
      );

      // frontend 的「歧義（2）」綁定摘要（原本 dump-dom 版本的 assertBinding('frontend', ...)，
      // 同理搬到這裡：frontend 屬於 Project p，切換過去才看得到）。
      const frontendBinding = await cdp.eval(`(() => {
        const row = document.querySelector('.ff-row-header[data-workstream="frontend"]');
        const text = row ? row.querySelector('.ff-binding-text') : null;
        return {
          text: text ? text.textContent : null,
          hasBadge: !!(row && row.querySelector('.ff-binding-badge')),
        };
      })()`);
      check(
        frontendBinding.text === '歧義（2）',
        `workstream frontend 的綁定摘要應該是 "歧義（2）"（實際 ${JSON.stringify(frontendBinding.text)}）`
      );
      check(frontendBinding.hasBadge === false, 'workstream frontend 不應該顯示「改綁」標示');
    } finally {
      try {
        if (colorWs) colorWs.close();
      } catch {
        // 已經斷線，忽略。
      }
      killTree(colorChrome, 'chrome-colors');
      await sleep(300);
      if (colorUdd) {
        try {
          fs.rmSync(colorUdd, { recursive: true, force: true });
        } catch (e) {
          check(false, `清理暫存目錄失敗：${e.message}`);
        }
      }
    }

    // 五種綁定摘要文字（含 override 標示）：direction-01-visual task 3.2 起改在上面的即時 CDP
    // session 以 DOM 判斷（assertLiveBinding）；frontend（歧義／2）屬於 Project p，在切換過去
    // 之後驗（task 3.1）。

    // --- warning 內容（fix round 1）---
    log('--- warning 內容 ---');
    check(
      html.includes(
        '<li class="project-warning">task docs-1 的 stage "Draft" 已不在 stages，退回起始 stage</li>'
      ),
      'project「cockpit」應該顯示指定內容的 warning'
    );

    // --- 左欄 Project 順序＋Factory Floor 只顯示選定的一個（direction-01-visual task 3.1；
    // 原本這裡驗「兩個 Project 上下順序」，spec「兩個 Project」情境把行為改成「中上區域只有
    // 一張 Factory Floor……另一個的網格不在畫面上，改由左欄切換」，斷言跟著改：左欄仍然依
    // state.projects 順序列出兩個 Project（cockpit 在上、p 在下），但 Factory Floor
    // （.project，data-project）在這份未選定過的快照裡應該恰好只有一個，且是 cockpit）---
    log('--- 左欄 Project 順序＋Factory Floor 只顯示選定的一個 ---');
    // 左欄項目用 data-action="select-project" 搭 data-project 指認（design D6），跟 Factory
    // Floor 面板本身、節點操作按鈕的 data-project 不會混淆（後兩者的 data-action 分別是
    // undefined／advance 等，不是 select-project）。
    const projectItemMatches = [...html.matchAll(/data-action="select-project" data-project="([^"]*)"/g)].map(
      (m) => m[1]
    );
    check(
      projectItemMatches.length === 2,
      `左欄應該恰好有兩個 Project 項目（實際 ${projectItemMatches.length}：${JSON.stringify(projectItemMatches)}）`
    );
    check(
      projectItemMatches[0] === 'cockpit' && projectItemMatches[1] === 'p',
      `左欄 Project 順序應該是 cockpit 在上、p（Scenario D）在下（實際 ${JSON.stringify(projectItemMatches)}）`
    );
    // task 5.3 起操作按鈕也帶 data-project（design D9），這裡只算 Factory Floor 面板本身
    // （class="project"，不是 class="project-item"）。
    const projectPanelMatches = [...html.matchAll(/class="project" data-project="([^"]*)"/g)].map((m) => m[1]);
    check(
      projectPanelMatches.length === 1 && projectPanelMatches[0] === 'cockpit',
      `spec「兩個 Project」：未選定過時 Factory Floor 應該只畫出第一個 Project（cockpit）一張（實際 ${JSON.stringify(projectPanelMatches)}）`
    );
    const projectsIdx = html.indexOf('class="projects"');
    const runtimeCardsIdx = html.indexOf('class="runtime-cards"');
    check(
      projectsIdx !== -1 && runtimeCardsIdx !== -1 && projectsIdx < runtimeCardsIdx,
      'Factory Floor（.projects）應該整塊畫在 runtime 卡（.runtime-cards）之前'
    );

    // --- 附帶：截圖存研究目錄，供人眼核對視覺 ---
    const shot = spawnSync(
      CHROME,
      [
        '--headless=new',
        '--disable-gpu',
        '--no-first-run',
        '--virtual-time-budget=4000',
        '--window-size=1400,1600',
        `--screenshot=${SCREENSHOT_PATH}`,
        `http://127.0.0.1:${port}/`,
      ],
      { encoding: 'utf8' }
    );
    check(shot.status === 0, `--screenshot 應該成功結束（實際 exit ${shot.status}）`);
    check(fs.existsSync(SCREENSHOT_PATH), `截圖應該存在：${SCREENSHOT_PATH}`);
  } finally {
    killTree(server, 'ui_preview');
    await sleep(300);
    check(!isPortListening(port), `port ${port} 應該不再有 LISTENING 的行程`);
  }
}

// ---------------------------------------------------------------------------
// 情境二：未知 task status 不破壞畫面（spec「未知 status 不破壞畫面」）。
// 無法透過 ui_preview／fixture 走這個情境（見檔頭註解：StageStatus 是封閉 enum），改用
// CDP 直接呼叫 window.onState(syntheticState)——render.js 唯一暴露在 window 上、會觸發
// renderState 整頁重畫的入口，跳過 Rust 型別系統與 channel.js，只測 render.js 本身對
// 「陌生 status 字串」的容錯（暗灰 class＋title 屬性保留原字串），同時確認同格另一個已知
// status 的節點正常，不受影響。
// ---------------------------------------------------------------------------
class CDP {
  constructor(ws) {
    this.ws = ws;
    this.id = 0;
    this.pending = new Map();
    ws.onmessage = (e) => {
      const m = JSON.parse(e.data);
      if (m.id && this.pending.has(m.id)) {
        this.pending.get(m.id)(m);
        this.pending.delete(m.id);
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
  async eval(expression) {
    const r = await this.send('Runtime.evaluate', { expression, returnByValue: true });
    return r.result && r.result.result ? r.result.result.value : undefined;
  }
}

async function attachCdp(cdpPort, pageUrlPrefix) {
  let page = null;
  for (let i = 0; i < 100 && !page; i++) {
    try {
      const r = await fetch(`http://127.0.0.1:${cdpPort}/json/list`);
      page = (await r.json()).find((t) => t.type === 'page' && t.url.startsWith(pageUrlPrefix));
    } catch {
      // Chrome 的 CDP endpoint 還沒起來，繼續等。
    }
    if (!page) await sleep(200);
  }
  if (!page) throw new Error('page target not found');
  const ws = new WebSocket(page.webSocketDebuggerUrl);
  await new Promise((res, rej) => {
    ws.onopen = res;
    ws.onerror = rej;
  });
  return { ws, cdp: new CDP(ws) };
}

// fix round 2：加 style.css 連結——round 1 版本沒有載入樣式表，getComputedStyle 量到的只會
// 是瀏覽器預設值（沒有顏色），沒辦法驗證「未知 status 真的是暗灰」。
const HARNESS_HTML = `<!doctype html>
<html><head><meta charset="utf-8"><title>render.js unknown-status harness</title>
<link rel="stylesheet" href="/app/style.css">
</head>
<body>
<div id="app"></div>
<script src="/app/render.js"></script>
</body></html>`;

// 兩個 task 同格：t-known（running，對照組，證明「其他節點正常」）與 t-unknown
// （status 是協定目前不存在的字串，模擬「未來協定加的新值」）。
const SYNTHETIC_STATE = {
  version: 1,
  generated_at: '2026-09-16T00:00:00Z',
  runtimes: [],
  projects: [
    {
      id: 'unknown-status-demo',
      name: 'Unknown Status Demo',
      stages: ['Stage1'],
      warnings: [],
      workstreams: [{ id: 'ws1', name: 'WS1', binding: { state: 'none' } }],
      tasks: [
        {
          id: 't-known',
          title: '已知狀態對照',
          workstream: 'ws1',
          stage: 'Stage1',
          mark: 'none',
          status: 'running',
          depends_on: [],
        },
        {
          id: 't-unknown',
          title: '未知狀態',
          workstream: 'ws1',
          stage: 'Stage1',
          mark: 'none',
          status: 'whatever',
          depends_on: [],
        },
      ],
    },
  ],
  recent_events: [],
};

async function scenarioUnknownStatus() {
  log('=== 情境二：未知 task status 不破壞畫面（直接呼叫 window.onState，繞過 Rust 型別系統）===');
  const PORT = 18790;
  const CDP_PORT = 18791;
  const server = http.createServer((req, res) => {
    if (req.url === '/') {
      res.writeHead(200, { 'Content-Type': 'text/html; charset=utf-8' });
      res.end(HARNESS_HTML);
      return;
    }
    if (req.url === '/app/render.js') {
      res.writeHead(200, { 'Content-Type': 'text/javascript; charset=utf-8' });
      res.end(fs.readFileSync(RENDER_JS_PATH, 'utf8'));
      return;
    }
    if (req.url === '/app/style.css') {
      res.writeHead(200, { 'Content-Type': 'text/css; charset=utf-8' });
      res.end(fs.readFileSync(STYLE_CSS_PATH, 'utf8'));
      return;
    }
    res.writeHead(404);
    res.end();
  });

  let chrome = null;
  let udd = null;
  let ws = null;
  try {
    await new Promise((res) => server.listen(PORT, '127.0.0.1', res));
    log(`harness server up on :${PORT}`);
    udd = fs.mkdtempSync(path.join(os.tmpdir(), 'cockpit-chrome-unknown-status-'));
    chrome = spawnTracked(
      CHROME,
      [
        '--headless=new',
        '--disable-gpu',
        '--no-first-run',
        `--remote-debugging-port=${CDP_PORT}`,
        '--remote-allow-origins=*',
        `--user-data-dir=${udd}`,
        '--window-size=1024,768',
        `http://127.0.0.1:${PORT}/`,
      ],
      'chrome-unknown-status'
    );
    const attached = await attachCdp(CDP_PORT, `http://127.0.0.1:${PORT}/`);
    ws = attached.ws;
    const cdp = attached.cdp;

    // attachCdp 只等「page target 出現在 /json/list」，不等 render.js 這顆 <script> 真的
    // 執行完——這裡額外輪詢到 window.onState 掛上去為止，避免在它還沒定義時就呼叫。
    let ready = false;
    for (let i = 0; i < 50 && !ready; i++) {
      ready = await cdp.eval("typeof window.onState === 'function'");
      if (!ready) await sleep(100);
    }
    check(ready, 'render.js 應該在 5 秒內把 window.onState 掛好');

    // window.onState 是 render.js 唯一暴露的、會觸發 renderState 整頁重畫的入口；直接餵一個
    // 沒經過 Rust 型別系統的 JS 物件，模擬「協定加了目前版本不認得的新 status 值」。
    const evalResult = await cdp.send('Runtime.evaluate', {
      expression: `window.onState(${JSON.stringify(SYNTHETIC_STATE)})`,
    });
    check(
      !evalResult.result || !evalResult.result.exceptionDetails,
      `window.onState 呼叫不應該丟例外（實際：${JSON.stringify(evalResult.result && evalResult.result.exceptionDetails)}）`
    );
    const appLen = await cdp.eval("document.getElementById('app').innerHTML.length");
    check(typeof appLen === 'number' && appLen > 0, '#app 應該有內容（重畫沒有整頁壞掉）');

    // direction-01-visual task 3.2：原本把 #app 的 innerHTML 取出來、用括號計數切出格子再做
    // 逐字 regex（`class="task-node task-status-running"`、`class="task-title">已知狀態對照<`、
    // `class="task-node task-status-unknown" title="whatever"`、`class="task-status-label">
    // whatever<`、`class="task-title">未知狀態<`、`class="task-node` 的出現次數）；節點多了狀態
    // 符號 span、標題 span 多了 title 屬性後，改成直接讀 DOM 的 className／屬性／textContent，
    // 比對的內容一一對應。
    const cellInfo = await cdp.eval(`(() => {
      const cell = document.querySelector('.ff-cell[data-workstream="ws1"][data-stage="Stage1"]');
      if (!cell) return null;
      const info = ${NODE_INFO_JS};
      const nodes = cell.querySelectorAll('.task-node');
      return {
        nodeCount: nodes.length,
        known: info(cell.querySelector('.task-node.task-status-running')),
        unknown: info(cell.querySelector('.task-node.task-status-unknown')),
      };
    })()`);
    check(cellInfo !== null, '應該找到格子 (ws1, Stage1)');
    const known = cellInfo ? cellInfo.known : null;
    const unknown = cellInfo ? cellInfo.unknown : null;

    // 對照組：已知 status 的節點正常（沒有因為隔壁的未知 status 節點而壞掉）。
    check(
      known !== null && known.className === 'task-node task-status-running' && known.titleText === '已知狀態對照',
      `同格內已知 status（running）的節點應該正常渲染（實際 ${JSON.stringify(known)}）`
    );

    // 未知 status：暗灰 class（task-status-unknown）＋ title 屬性保留原字串＋文字顯示原字串。
    check(
      unknown !== null && unknown.className === 'task-node task-status-unknown' && unknown.titleAttr === 'whatever',
      `未知 status 節點應該落 class "task-status-unknown" 並帶 title="whatever"（實際 ${JSON.stringify(unknown)}）`
    );
    check(unknown !== null && unknown.statusText === 'whatever', '未知 status 節點的文字應該保留原字串 "whatever"');
    check(unknown !== null && unknown.titleText === '未知狀態', '未知 status 節點的標題應該正常顯示');

    check(cellInfo !== null && cellInfo.nodeCount === 2, `這格應該恰好有兩個 task 節點（實際 ${cellInfo && cellInfo.nodeCount}）`);

    // fix round 2：實際顏色（不只是 class 名稱對了）。harness 現在有載入真正的 style.css
    // （見 HARNESS_HTML 的變更），才量得到有意義的計算樣式。direction-01-visual task 3.2：節點
    // 不再是狀態色實底（spec「節點以表面色為底、左緣一條狀態色條」；「未知 status……以次要
    // 文字色與虛線外框顯示原字串」），原本讀兩個節點的 backgroundColor，改成讀左緣色條與
    // status 文字色，未知 status 另外斷言虛線外框與外框色。
    const runningColor = hexToRgb(EXPECTED_COLORS.running);
    check(
      known !== null && known.stripeColor === runningColor && known.labelColor === runningColor,
      `對照組 running 節點的色條與 status 文字應該是 ${runningColor}（實際 ${known && known.stripeColor}／${known && known.labelColor}）`
    );
    const unknownColor = hexToRgb(EXPECTED_COLORS.unknown);
    check(
      unknown !== null && unknown.labelColor === unknownColor && unknown.stripeColor === unknownColor,
      `未知 status 節點的 status 文字與色條應該是次要文字色 ${unknownColor}（style.css ${EXPECTED_COLORS.unknown}，實際 ${unknown && unknown.labelColor}／${unknown && unknown.stripeColor}）`
    );
    check(
      unknown !== null && unknown.borderStyle === 'dashed' && unknown.borderTopColor === unknownColor,
      `未知 status 節點應該有 ${unknownColor} 虛線外框（實際 ${unknown && unknown.borderStyle}／${unknown && unknown.borderTopColor}）`
    );
    // task 3.2 fix round 1（Codex (1)）：只驗 borderStyle 與顏色時，四邊寬度被改成 0（虛線外框
    // 完全看不見）仍會通過。補驗四邊寬度 > 0、左側狀態條 ≥ STRIPE_MIN_PX；否定對照：同一個節點
    // 暫時設 border-width: 0，同一個判準必須回報失敗。
    const STRIPE_MIN_PX = 3;
    const frameVisible = (x) =>
      x !== null && x.borderWidths.slice(0, 3).every((w) => w > 0) && x.borderWidths[3] >= STRIPE_MIN_PX;
    check(
      frameVisible(unknown),
      `未知 status 節點的虛線外框四邊寬度應該 > 0、左側狀態條 ≥${STRIPE_MIN_PX}px（實際 ${unknown && JSON.stringify(unknown.borderWidths)}）`
    );
    const zeroWidth = await cdp.eval(`(() => {
      const n = document.querySelector('.ff-cell[data-workstream="ws1"][data-stage="Stage1"] .task-node.task-status-unknown');
      n.style.borderWidth = '0';
      const r = (${NODE_INFO_JS})(n);
      n.style.borderWidth = '';
      return r;
    })()`);
    check(
      zeroWidth !== null && zeroWidth.borderStyle === 'dashed' && !frameVisible(zeroWidth),
      `否定對照：未知 status 節點 border-width 設成 0 時 style 仍是 dashed，寬度判準必須回報失敗（實際 ${JSON.stringify(zeroWidth && zeroWidth.borderWidths)}）`
    );
    check(
      unknown !== null && unknown.background === hexToRgb(SURFACE_COLOR) && known !== null && known.background === hexToRgb(SURFACE_COLOR),
      `兩個節點的底色都應該是 --surface ${hexToRgb(SURFACE_COLOR)}（實際 ${known && known.background}／${unknown && unknown.background}）`
    );
  } finally {
    try {
      if (ws) ws.close();
    } catch {
      // 已經斷線，忽略。
    }
    killTree(chrome, 'chrome-unknown-status');
    await sleep(500); // 讓已終止行程釋放暫存目錄檔案控制代碼，避免刪除時 EBUSY。
    await new Promise((res) => server.close(res));
    if (udd) {
      try {
        fs.rmSync(udd, { recursive: true, force: true });
      } catch (e) {
        check(false, `清理暫存目錄失敗：${e.message}`);
      }
    }
  }
}

async function main() {
  if (!fs.existsSync(CHROME)) {
    throw new Error(`找不到 Chrome：${CHROME}（可用環境變數 COCKPIT_CHROME 指定路徑）`);
  }
  await scenarioGridAndCoverage();
  await scenarioUnknownStatus();

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
