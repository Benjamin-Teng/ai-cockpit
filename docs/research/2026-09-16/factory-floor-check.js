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
// 做法：直接執行 `cargo build -p cockpit --example ui_preview` 產生的執行檔
// （target/debug/examples/ui_preview.exe），不透過 `cargo run` 啟動——`cargo run` 會多一層
// wrapper 行程，實際監聽 port 的是它的子行程，PID 追蹤與收尾容易對不上；直接執行 build
// 好的執行檔可以拿到單一、穩定的 PID 做收尾。這跟文件裡寫的
// `cargo run -p cockpit --example ui_preview` 啟動的是同一支程式、同一份 fixture，只是省了
// cargo 那層 wrapper。
//
// headless Chrome 用 `--dump-dom --virtual-time-budget=<ms>`：讓 Chrome 在收到 index.html
// 的 load 事件後，繼續跑一段虛擬時間（WebSocket 連線、收到第一份投影、render.js 整頁重畫
// 都在這段時間內完成），時間到才把最終 DOM 序列化成 HTML 字串輸出到 stdout。取格子／列首
// 內容用簡單的括號深度計數（`extractBalancedDiv`），不用正規表達式硬吃到下一個
// `</div>`——cell 裡可能巢狀一層 task-node 的 `</div>`，正規表達式會提早收尾判斷錯。
//
// `scenarioUnknownStatus` 不透過 ui_preview／dump-dom，改用 CDP（跟 task 5.1
// channel-backoff-check.js 同一套手法）：起一個只服務兩個檔案（harness html＋真正的
// render.js 檔案內容）的極簡 http server，headless Chrome 開起來後用 CDP
// `Runtime.evaluate` 直接呼叫 `window.onState(syntheticState)`（render.js 唯一暴露在
// window 上、會觸發整頁重畫的入口），再讀 `#app` 的 innerHTML 斷言。
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
const EXPECTED_COLORS = {
  running: '#3fb950', // --status-working
  blocked: '#d29922', // --status-blocked
  ready: '#8b949e', // --status-idle
  pending: '#484f58', // --status-unknown（pending 沿用既有「暗灰」語彙）
  failed: '#f85149', // --stage-failed（本 change 新增）
  completed: '#a371f7', // --stage-completed（本 change 新增，刻意不是 --status-done 的藍）
  unknown: '#484f58', // --status-unknown（task-status-unknown fallback，跟 pending 同一個值）
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
const escapeRe = (s) => s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');

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

// 找 `openTag`（例如 `<div class="ff-cell" ...>`）開頭、跟它配對的 `</div>` 之間的內容：
// 不用正規表達式硬吃到下一個 `</div>`（裡面可能巢狀一層子 div），改用簡單的深度計數，掃到
// 與開頭配對的那個 `</div>` 才停。
function extractBalancedDiv(html, openTag) {
  const start = html.indexOf(openTag);
  if (start === -1) return null;
  let i = start + openTag.length;
  let depth = 1;
  while (depth > 0 && i < html.length) {
    const nextOpen = html.indexOf('<div', i);
    const nextClose = html.indexOf('</div>', i);
    if (nextClose === -1) return null;
    if (nextOpen !== -1 && nextOpen < nextClose) {
      depth += 1;
      i = nextOpen + 4;
    } else {
      depth -= 1;
      i = nextClose + 6;
    }
  }
  return html.slice(start + openTag.length, i - 6);
}

function extractCellHtml(html, ws, stage) {
  return extractBalancedDiv(html, `<div class="ff-cell" data-workstream="${ws}" data-stage="${stage}">`);
}

function extractRowHeaderHtml(html, ws) {
  return extractBalancedDiv(html, `<div class="ff-row-header" data-workstream="${ws}">`);
}

// 逐項比對一個 task 節點的狀態 class、狀態文字、標題文字（fix round 1 的核心訴求：不能只
// 斷言「有沒有節點」，要斷言「這個節點是不是這個狀態」）。
function assertTaskNode(html, ws, stage, status, title) {
  const cell = extractCellHtml(html, ws, stage);
  check(cell !== null, `應該找到格子 (${ws}, ${stage})`);
  if (cell === null) return;
  check(
    cell.includes(`class="task-node task-status-${status}"`),
    `(${ws}, ${stage}) 節點應該有 class "task-status-${status}"（實際片段：${cell.slice(0, 120)}）`
  );
  check(
    new RegExp(`class="task-status-label">${escapeRe(status)}<`).test(cell),
    `(${ws}, ${stage}) 節點的狀態文字應該是 "${status}"`
  );
  check(
    new RegExp(`class="task-title">${escapeRe(title)}<`).test(cell),
    `(${ws}, ${stage}) 節點的標題應該是 "${title}"`
  );
}

// 逐項比對一條 workstream 列首的綁定摘要文字、以及 override 時的「改綁」標示。
function assertBinding(html, ws, expectedText, expectBadge) {
  const row = extractRowHeaderHtml(html, ws);
  check(row !== null, `應該找到 workstream 列首 (data-workstream="${ws}")`);
  if (row === null) return;
  check(
    new RegExp(`class="ff-binding-text">${escapeRe(expectedText)}<`).test(row),
    `workstream ${ws} 的綁定摘要應該是 "${expectedText}"（實際片段：${row.slice(0, 160)}）`
  );
  const hasBadge = row.includes('class="ff-binding-badge">改綁<');
  check(
    hasBadge === expectBadge,
    `workstream ${ws} ${expectBadge ? '應該' : '不應該'} 顯示「改綁」標示（實際${hasBadge ? '有' : '沒有'}）`
  );
}

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

    // --- Scenario D：三個節點位於正確（列, 欄），其餘六格沒有節點 ---
    log('--- Scenario D 網格位置 ---');
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
      const cell = extractCellHtml(html, ws, stage);
      check(cell !== null, `應該找到格子 (${ws}, ${stage})`);
      const hasNode = !!cell && cell.includes('class="task-node');
      check(
        hasNode === expectNode,
        `(${ws}, ${stage}) ${expectNode ? '應該有' : '不應該有'} task 節點（實際${hasNode ? '有' : '沒有'}）`
      );
    }
    assertTaskNode(html, 'backend', 'Implement', 'running', '後端實作');
    assertTaskNode(html, 'frontend', 'Plan', 'running', '前端規劃');
    assertTaskNode(html, 'tests', 'Test', 'running', '測試執行');

    // --- 六種已知 status 逐項比對 class／文字（fix round 1）---
    log('--- 六種已知 status（class／文字逐項比對）---');
    assertTaskNode(html, 'be', 'Implement', 'running', '投影擴充');
    assertTaskNode(html, 'docs', 'Spec', 'pending', 'README');
    assertTaskNode(html, 'ops', 'Review', 'completed', '部署腳本');
    assertTaskNode(html, 'qa', 'Implement', 'blocked', '程式碼審查');
    assertTaskNode(html, 'release', 'Spec', 'ready', '發布準備');
    assertTaskNode(html, 'ops', 'Spec', 'failed', '上線檢查');
    const runningNodes = (html.match(/task-status-running/g) || []).length;
    check(
      runningNodes === 4,
      `running 節點總數應該是 4（cockpit 專案 1 個 + Scenario D 3 個，實際 ${runningNodes}）`
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

      const knownCells = [
        ['be', 'Implement', 'running'],
        ['docs', 'Spec', 'pending'],
        ['ops', 'Review', 'completed'],
        ['qa', 'Implement', 'blocked'],
        ['release', 'Spec', 'ready'],
        ['ops', 'Spec', 'failed'],
      ];
      for (const [ws, stage, status] of knownCells) {
        const selector = `.ff-cell[data-workstream="${ws}"][data-stage="${stage}"] .task-node`;
        const bg = await cdp.eval(
          `getComputedStyle(document.querySelector('${selector}')).backgroundColor`
        );
        const expected = hexToRgb(EXPECTED_COLORS[status]);
        check(
          bg === expected,
          `(${ws}, ${stage}) 節點（${status}）的 background-color 應該是 ${expected}（style.css ${EXPECTED_COLORS[status]}，實際 ${bg}）`
        );
      }
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

    // --- 五種綁定摘要文字（含 override 標示）逐項比對（fix round 1）---
    log('--- 五種綁定摘要文字（含 override）---');
    assertBinding(html, 'be', 'win / wJ:p1', false);
    assertBinding(html, 'docs', 'runtime 未連線', false);
    assertBinding(html, 'ops', '無綁定', false);
    assertBinding(html, 'qa', 'win / wJ:p3', true);
    assertBinding(html, 'release', '未綁定', false);
    assertBinding(html, 'frontend', '歧義（2）', false);

    // --- warning 內容（fix round 1）---
    log('--- warning 內容 ---');
    check(
      html.includes(
        '<li class="project-warning">task docs-1 的 stage "Draft" 已不在 stages，退回起始 stage</li>'
      ),
      'project「cockpit」應該顯示指定內容的 warning'
    );

    // --- 兩個 Project 上下順序在 runtime 卡之前 ---
    log('--- 兩個 Project 上下順序 ---');
    // task 5.3 起操作按鈕也帶 data-project（design D9），這裡只算 Project 區塊本身。
    const projectMatches = [...html.matchAll(/class="project" data-project="([^"]*)"/g)].map((m) => m[1]);
    check(
      projectMatches.length === 2,
      `應該恰好有兩個 data-project（實際 ${projectMatches.length}：${JSON.stringify(projectMatches)}）`
    );
    check(
      projectMatches[0] === 'cockpit' && projectMatches[1] === 'p',
      `Project 順序應該是 cockpit 在上、p（Scenario D）在下（實際 ${JSON.stringify(projectMatches)}）`
    );
    const projectsIdx = html.indexOf('class="projects"');
    const runtimeCardsIdx = html.indexOf('class="runtime-cards"');
    check(
      projectsIdx !== -1 && runtimeCardsIdx !== -1 && projectsIdx < runtimeCardsIdx,
      'Factory Floor（.projects）應該整塊畫在 runtime 卡（.runtime-cards）之前'
    );
    const cockpitIdx = html.indexOf('class="project" data-project="cockpit"');
    const pIdx = html.indexOf('class="project" data-project="p"');
    check(
      cockpitIdx !== -1 && pIdx !== -1 && cockpitIdx < pIdx && pIdx < runtimeCardsIdx,
      'cockpit 專案應該在 p（Scenario D）之上，兩者都在 runtime 卡之前'
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
    const appHtml = await cdp.eval("document.getElementById('app').innerHTML");
    check(typeof appHtml === 'string' && appHtml.length > 0, '#app 應該有內容（重畫沒有整頁壞掉）');

    const cell = extractCellHtml(appHtml, 'ws1', 'Stage1');
    check(cell !== null, '應該找到格子 (ws1, Stage1)');
    const html = cell || '';

    // 對照組：已知 status 的節點正常（沒有因為隔壁的未知 status 節點而壞掉）。
    check(
      html.includes('class="task-node task-status-running"') &&
        /class="task-title">已知狀態對照</.test(html),
      '同格內已知 status（running）的節點應該正常渲染'
    );

    // 未知 status：暗灰 class（task-status-unknown）＋ title 屬性保留原字串＋文字顯示原字串。
    check(
      html.includes('class="task-node task-status-unknown" title="whatever"'),
      `未知 status 節點應該落 class "task-status-unknown" 並帶 title="whatever"（實際片段：${html.slice(0, 300)}）`
    );
    check(
      /class="task-status-label">whatever</.test(html),
      '未知 status 節點的文字應該保留原字串 "whatever"'
    );
    check(
      /class="task-title">未知狀態</.test(html),
      '未知 status 節點的標題應該正常顯示'
    );

    const nodeCount = (html.match(/class="task-node/g) || []).length;
    check(nodeCount === 2, `這格應該恰好有兩個 task 節點（實際 ${nodeCount}）`);

    // fix round 2：實際顏色（不只是 class 名稱對了）。harness 現在有載入真正的 style.css
    // （見 HARNESS_HTML 的變更），才量得到有意義的 backgroundColor。
    const runningBg = await cdp.eval(
      "getComputedStyle(document.querySelector('.task-node.task-status-running')).backgroundColor"
    );
    check(
      runningBg === hexToRgb(EXPECTED_COLORS.running),
      `對照組 running 節點的 background-color 應該是 ${hexToRgb(EXPECTED_COLORS.running)}（實際 ${runningBg}）`
    );
    const unknownBg = await cdp.eval(
      "getComputedStyle(document.querySelector('.task-node.task-status-unknown')).backgroundColor"
    );
    check(
      unknownBg === hexToRgb(EXPECTED_COLORS.unknown),
      `未知 status 節點的 background-color 應該是 ${hexToRgb(EXPECTED_COLORS.unknown)}（style.css ${EXPECTED_COLORS.unknown}，實際 ${unknownBg}）`
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
