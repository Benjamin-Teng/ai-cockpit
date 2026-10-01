// progress-model task 4.2／4.3 驗收：退回按鈕（spec cockpit-dashboard「畫面操作」的「退回按鈕」）與
// 工作中・未宣告 task 的列首提示（「Factory Floor」的「未宣告 task 的提示」）。
// 各段代號與對應 scenario 見同目錄 progress-check.md。
//
// 兩段：
//
// A. 真的 ui_preview（`COCKPIT_PREVIEW_PUSH_MS=100`，寫入路由只記錄、回 204，stdout 印
//    `write-request <METHOD> <PATH> <BODY>`）。ui_preview 的 domain 是靜態 fixture：按「退回」
//    之後 stage 不會改變，所以 A 段只斷言「送出的請求」與「畫面不自行改狀態」，
//    「新投影後節點移到上一站」交給 B 段（harness 直接餵新投影）。
// B. 極簡 harness（node http server 服務真正的 style.css／render.js／actions.js，不載
//    channel.js，投影以 CDP 呼叫 `window.onState(...)` 餵入）：用 spec 原文的情境
//    （stages Plan／Build，t1 在 Build 無標記、t2 在 Plan、t3 在 Build 標 failed）。
//
// 用法（repo 根，需先 `cargo build -p cockpit --example ui_preview`）：
//   node docs/research/2026-10-01/progress-check.js
// 清理：只終止本腳本自己 spawn 的 ui_preview.exe／chrome.exe（依 PID），埠被占用就往上找空埠。
const http = require('node:http');
const os = require('node:os');
const { spawn, spawnSync } = require('node:child_process');
const path = require('node:path');
const fs = require('node:fs');

const REPO = path.resolve(__dirname, '..', '..', '..');
const UI_PREVIEW_EXE = path.join(REPO, 'target', 'debug', 'examples', 'ui_preview.exe');
const ASSETS = path.join(REPO, 'cockpit', 'assets', 'app');
const CHROME =
  process.env.COCKPIT_CHROME || 'C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe';

const failures = [];
function check(cond, label) {
  console.log(`${cond ? 'ok  ' : 'FAIL'} ${label}`);
  if (!cond) failures.push(label);
}
const log = (s) => console.log(`[${new Date().toISOString()}] ${s}`);
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

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
  // 以真的滑鼠事件點 selector 指到的元素中心；holdMs 是按下與放開之間的間隔。
  async click(selector, holdMs = 0) {
    const rect = await this.eval(
      `(() => { const n = document.querySelector(${JSON.stringify(selector)});
        if (!n) return null; n.scrollIntoView({block: 'center'});
        const r = n.getBoundingClientRect(); return {x: r.left + r.width / 2, y: r.top + r.height / 2}; })()`
    );
    if (!rect) {
      check(false, `找不到可點的元素：${selector}`);
      return false;
    }
    const base = { x: rect.x, y: rect.y, button: 'left', clickCount: 1 };
    await this.send('Input.dispatchMouseEvent', { type: 'mouseMoved', x: rect.x, y: rect.y });
    await this.send('Input.dispatchMouseEvent', { type: 'mousePressed', ...base });
    if (holdMs > 0) await sleep(holdMs);
    await this.send('Input.dispatchMouseEvent', { type: 'mouseReleased', ...base });
    return true;
  }
}

async function startChrome(cdpPort, url, label) {
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
      '--window-size=1400,1600',
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

// ---------------------------------------------------------------------------
// 共用：未宣告提示的讀取
// ---------------------------------------------------------------------------

const UNDECLARED_TEXT = '工作中・未宣告 task';

// 回傳目前畫面上每個 workstream 列首是否含提示（以列首 textContent 判斷，不看 class）。
const readHeaders = `(() => {
  const out = {};
  document.querySelectorAll('.ff-row-header[data-workstream]').forEach((h) => {
    out[h.dataset.workstream] = h.textContent.includes(${JSON.stringify(UNDECLARED_TEXT)});
  });
  return out;
})()`;

// 提示元素與其偽元素的 animation-name／transition-property 都必須沒有動畫（spec：不得有動畫）。
const readAnimations = `(() => {
  const out = [];
  document.querySelectorAll('.ff-undeclared').forEach((n) => {
    for (const pseudo of [null, '::before', '::after']) {
      const cs = getComputedStyle(n, pseudo);
      out.push({ pseudo, animation: cs.animationName, transition: cs.transitionProperty });
    }
  });
  return out;
})()`;

function checkNoAnimation(anims, label) {
  check(anims.length > 0, `${label}：有找到提示元素可檢查動畫`);
  check(
    anims.every((a) => a.animation === 'none'),
    `${label}：提示與其偽元素的 animation-name 皆為 none（實際 ${JSON.stringify(anims.map((a) => a.animation))}）`
  );
}

const versionOf = "Number(document.getElementById('version').textContent.replace(/\\D/g, ''))";

// ---------------------------------------------------------------------------
// A. ui_preview（100 ms 推送）
// ---------------------------------------------------------------------------

async function partPreview() {
  log('=== A. ui_preview（COCKPIT_PREVIEW_PUSH_MS=100）===');
  if (!fs.existsSync(UI_PREVIEW_EXE)) {
    throw new Error(`找不到 ${UI_PREVIEW_EXE}，請先跑 cargo build -p cockpit --example ui_preview`);
  }
  const port = pickPort(7770);
  if (port !== 7770) log(`7770 已被占用，改用埠 ${port}`);
  const cdpPort = pickPort(18800, [port]);

  const requests = [];
  let server = null;
  let chrome = null;
  try {
    server = spawn(UI_PREVIEW_EXE, [], {
      stdio: ['ignore', 'pipe', 'ignore'],
      windowsHide: true,
      env: {
        ...process.env,
        COCKPIT_PREVIEW_LISTEN: `127.0.0.1:${port}`,
        COCKPIT_PREVIEW_PUSH_MS: '100',
      },
    });
    server.on('error', (e) => check(false, `ui_preview spawn error：${e.message}`));
    let buffer = '';
    server.stdout.setEncoding('utf8');
    server.stdout.on('data', (chunk) => {
      buffer += chunk;
      let nl;
      while ((nl = buffer.indexOf('\n')) !== -1) {
        const line = buffer.slice(0, nl).replace(/\r$/, '');
        buffer = buffer.slice(nl + 1);
        const m = /^write-request (\S+) (\S+) ?(.*)$/.exec(line);
        if (m) requests.push({ method: m[1], path: m[2], body: m[3] });
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
    check(up, `ui_preview 應該在 10 秒內開始回應（port ${port}）`);
    if (!up) throw new Error('ui_preview 沒有起來');

    const state = await (await fetch(`http://127.0.0.1:${port}/api/state`)).json();
    chrome = await startChrome(cdpPort, `http://127.0.0.1:${port}/`, 'preview');
    const { cdp } = chrome;
    await cdp.waitFor("document.querySelectorAll('.task-node').length >= 9", 5000, '畫出預設 Project 的 task 節點');

    // --- A1 退回按鈕顯示條件：逐 Project 對照 /api/state ---
    log('--- A1 退回按鈕顯示條件（對照 /api/state）---');
    let sawFirstStageNone = false;
    let sawMarked = false;
    let sawRetreat = false;
    for (const project of state.projects) {
      await cdp.click(`[data-action="select-project"][data-project="${project.id}"]`);
      await cdp.waitFor(
        `document.querySelectorAll('.project[data-project="${project.id}"] .task-node').length >= ${project.tasks.length}`,
        3000,
        `切到 Project ${project.id} 並畫出全部節點`
      );
      const domRetreat = await cdp.eval(`(() => {
        const out = {};
        document.querySelectorAll('.task-node').forEach((n) => {
          const id = n.querySelector('[data-task]').dataset.task;
          out[id] = !!n.querySelector('[data-action="retreat"]');
        });
        return out;
      })()`);
      for (const task of project.tasks) {
        const want = task.mark === 'none' && task.stage !== project.stages[0];
        check(
          domRetreat[task.id] === want,
          `${project.id}/${task.id}（stage ${task.stage}、mark ${task.mark}）「退回」應${want ? '有' : '沒有'}（實際 ${domRetreat[task.id]}）`
        );
        if (task.mark === 'none' && task.stage === project.stages[0]) sawFirstStageNone = true;
        if (task.mark !== 'none') sawMarked = true;
        if (want) sawRetreat = true;
      }
    }
    check(sawFirstStageNone, '情境涵蓋：存在「第一站且無標記」的 task（應無退回）');
    check(sawMarked, '情境涵蓋：存在「有標記」的 task（應無退回）');
    check(sawRetreat, '情境涵蓋：存在「可退回」的 task（應有退回）');

    // --- A2 未宣告提示：p/undeclared 有、其他列沒有；重畫後仍正確 ---
    log('--- A2 未宣告提示（ui_preview 的 p/undeclared）---');
    await cdp.click('[data-action="select-project"][data-project="p"]');
    await cdp.waitFor(
      "!!document.querySelector('.project[data-project=\"p\"] .ff-row-header[data-workstream=\"undeclared\"]')",
      3000,
      '切到 Project p 並畫出 undeclared 列'
    );
    const wantHeaders = {};
    for (const ws of state.projects.find((p) => p.id === 'p').workstreams) {
      wantHeaders[ws.id] = ws.activity_undeclared === true;
    }
    check(wantHeaders.undeclared === true, '/api/state 的 p/undeclared 為 activity_undeclared=true');
    check(
      Object.values(wantHeaders).filter((v) => v).length === 1,
      'p 只有 undeclared 一列 activity_undeclared=true，其他為 false'
    );
    const got1 = await cdp.eval(readHeaders);
    check(
      JSON.stringify(got1) === JSON.stringify(wantHeaders),
      `各列提示與投影一致（期望 ${JSON.stringify(wantHeaders)}；實際 ${JSON.stringify(got1)}）`
    );
    checkNoAnimation(await cdp.eval(readAnimations), 'A2');

    // 整頁重畫後仍正確：等 version 至少前進 2，並確認 DOM 節點真的被換掉。
    await cdp.eval("window.__marker = document.querySelector('.ff-row-header[data-workstream=\"undeclared\"]'); true");
    const v0 = await cdp.eval(versionOf);
    await cdp.waitFor(`${versionOf} >= ${v0 + 2}`, 3000, '收到至少兩份新投影');
    check(
      await cdp.eval("document.querySelector('.ff-row-header[data-workstream=\"undeclared\"]') !== window.__marker"),
      '整頁重畫確實換掉了列首 DOM 節點'
    );
    const got2 = await cdp.eval(readHeaders);
    check(
      JSON.stringify(got2) === JSON.stringify(wantHeaders),
      `重畫後各列提示仍與投影一致（實際 ${JSON.stringify(got2)}）`
    );
    checkNoAnimation(await cdp.eval(readAnimations), 'A2 重畫後');

    // cockpit Project 沒有任何未宣告提示。
    await cdp.click('[data-action="select-project"][data-project="cockpit"]');
    await cdp.waitFor("!!document.querySelector('.project[data-project=\"cockpit\"]')", 3000, '切到 cockpit');
    const gotC = await cdp.eval(readHeaders);
    check(
      Object.values(gotC).length > 0 && Object.values(gotC).every((v) => v === false),
      `cockpit 各列都沒有提示（實際 ${JSON.stringify(gotC)}）`
    );

    // --- A3 按退回：服務收到請求，畫面不自行改狀態；停在這個狀態再收到新投影重畫 ---
    log('--- A3 按「退回」（p/undeclared-1）---');
    await cdp.click('[data-action="select-project"][data-project="p"]');
    await cdp.waitFor(
      "!!document.querySelector('[data-action=\"retreat\"][data-task=\"undeclared-1\"]')",
      3000,
      '切回 p，undeclared-1 的「退回」在畫面上'
    );
    requests.length = 0;
    await cdp.click('[data-action="retreat"][data-project="p"][data-task="undeclared-1"]');
    await sleep(500);
    check(
      requests.length === 1 &&
        requests[0].method === 'POST' &&
        requests[0].path === '/api/projects/p/tasks/undeclared-1/retreat',
      `服務收到恰好一個 POST /api/projects/p/tasks/undeclared-1/retreat（實際 ${JSON.stringify(requests)}）`
    );
    const where = `(() => { const n = document.querySelector('[data-task="undeclared-1"]'); const c = n && n.closest('.ff-cell'); return c ? c.dataset.stage : null; })()`;
    const stageNow = await cdp.eval(where);
    check(stageNow === 'Implement', `按下後（ui_preview 不改 fixture）節點仍在 Implement，畫面不自行移動（實際 ${stageNow}）`);
    // 停在這個狀態再收到新投影整頁重畫：退回按鈕、提示都還在。
    const v1 = await cdp.eval(versionOf);
    await cdp.waitFor(`${versionOf} >= ${v1 + 2}`, 3000, '按下退回後又收到至少兩份新投影');
    check(
      await cdp.eval("!!document.querySelector('[data-action=\"retreat\"][data-task=\"undeclared-1\"]')"),
      '重畫後 undeclared-1 的「退回」仍在'
    );
    const got3 = await cdp.eval(readHeaders);
    check(
      got3.undeclared === true && got3.backend === false,
      `重畫後 undeclared 有提示、backend 沒有（實際 ${JSON.stringify(got3)}）`
    );
  } finally {
    await stopChrome(chrome, 'chrome-preview');
    killTree(server, 'ui_preview');
    await sleep(300);
    check(!isPortListening(port), `port ${port} 應該不再有 LISTENING 的行程`);
  }
}

// ---------------------------------------------------------------------------
// B. harness（直接餵投影，spec 原文情境）
// ---------------------------------------------------------------------------

const HARNESS_HTML = `<!doctype html>
<html><head><meta charset="utf-8"><title>progress harness</title>
<link rel="stylesheet" href="/app/style.css">
</head>
<body>
<div id="app"></div>
<script src="/app/render.js"></script>
<script src="/app/actions.js"></script>
</body></html>`;

// stages Plan／Build；t1 在 t1Stage 無標記、t2 在 Plan 無標記、t3 在 Build 標 failed。
// 兩個 workstream：backend（undeclared 由參數決定）、frontend（恆為 false）。
function harnessState(version, t1Stage, backendUndeclared) {
  const task = (id, stage, mark, ws) => ({
    id,
    title: id,
    workstream: ws,
    stage,
    mark,
    status: mark === 'none' ? 'ready' : mark,
    depends_on: [],
  });
  return {
    version,
    generated_at: '2026-10-01T00:00:00Z',
    runtimes: [],
    projects: [
      {
        id: 'p',
        name: 'Harness',
        stages: ['Plan', 'Build'],
        warnings: [],
        workstreams: [
          { id: 'backend', name: 'backend', binding: { state: 'none' }, active_task: null, activity_undeclared: backendUndeclared },
          { id: 'frontend', name: 'frontend', binding: { state: 'none' }, active_task: null, activity_undeclared: false },
        ],
        tasks: [
          task('t1', t1Stage, 'none', 'backend'),
          task('t2', 'Plan', 'none', 'frontend'),
          task('t3', 'Build', 'failed', 'frontend'),
        ],
      },
    ],
    recent_events: [],
  };
}

async function partHarness() {
  log('=== B. harness（退回按鈕 scenario、未宣告提示 scenario）===');
  const port = pickPort(18810);
  const cdpPort = pickPort(18811, [port]);
  const requests = [];
  const server = http.createServer((req, res) => {
    const files = {
      '/': ['text/html', HARNESS_HTML],
      '/app/style.css': ['text/css', null],
      '/app/render.js': ['text/javascript', null],
      '/app/actions.js': ['text/javascript', null],
    };
    if (req.method === 'GET' && files[req.url]) {
      const [type, inline] = files[req.url];
      res.writeHead(200, { 'Content-Type': `${type}; charset=utf-8` });
      res.end(inline !== null ? inline : fs.readFileSync(path.join(ASSETS, req.url.slice(5)), 'utf8'));
      return;
    }
    if (req.method === 'GET') {
      res.writeHead(404);
      res.end();
      return;
    }
    let body = '';
    req.on('data', (c) => (body += c));
    req.on('end', () => {
      requests.push({ method: req.method, path: req.url, body });
      res.writeHead(204);
      res.end();
    });
  });

  let chrome = null;
  try {
    await new Promise((res) => server.listen(port, '127.0.0.1', res));
    chrome = await startChrome(cdpPort, `http://127.0.0.1:${port}/`, 'harness');
    const { cdp } = chrome;
    await cdp.waitFor(
      "typeof window.onState === 'function' && document.readyState === 'complete'",
      5000,
      'render.js／actions.js 載入完成'
    );
    const feed = (s) => cdp.eval(`window.onState(${JSON.stringify(s)}); true`);
    const stageOf = (id) =>
      cdp.eval(
        `(() => { const n = document.querySelector('[data-task="${id}"]'); const c = n && n.closest('.ff-cell'); return c ? c.dataset.stage : null; })()`
      );
    const hasRetreat = (id) => cdp.eval(`!!document.querySelector('[data-action="retreat"][data-task="${id}"]')`);

    // --- B1 退回按鈕 scenario ---
    log('--- B1 情境「退回按鈕」---');
    await feed(harnessState(1, 'Build', true));
    check(await hasRetreat('t1'), 't1（Build、none）有「退回」');
    check(!(await hasRetreat('t2')), 't2（第一站 Plan、none）沒有「退回」');
    check(!(await hasRetreat('t3')), 't3（Build、failed）沒有「退回」');
    const t1Order = await cdp.eval("[...document.querySelectorAll('[data-task=\"t1\"]')].map((b) => b.dataset.action)");
    check(
      JSON.stringify(t1Order) === JSON.stringify(['retreat', 'complete', 'fail']),
      `t1 在最後一站：按鈕依序為退回、Completed、Failed（實際 ${JSON.stringify(t1Order)}）`
    );
    const t1Label = await cdp.eval("document.querySelector('[data-action=\"retreat\"][data-task=\"t1\"]').textContent");
    check(t1Label === '退回', `按鈕文案為「退回」（實際 ${JSON.stringify(t1Label)}）`);
    await cdp.click('[data-action="retreat"][data-task="t1"]');
    await sleep(400);
    check(
      requests.length === 1 && requests[0].method === 'POST' && requests[0].path === '/api/projects/p/tasks/t1/retreat',
      `服務收到 POST /api/projects/p/tasks/t1/retreat（實際 ${JSON.stringify(requests)}）`
    );
    check((await stageOf('t1')) === 'Build', '204 之後、新投影到達前，t1 仍在 Build（畫面不自行修改狀態）');
    await feed(harnessState(2, 'Plan', true));
    check((await stageOf('t1')) === 'Plan', '新投影到達後 t1 出現在 Plan 欄');
    check(!(await hasRetreat('t1')), 't1 回到第一站後不再有「退回」');
    requests.length = 0;

    // --- B2 未宣告提示 scenario ---
    log('--- B2 情境「未宣告 task 的提示」---');
    await feed(harnessState(3, 'Plan', true));
    const h1 = await cdp.eval(readHeaders);
    check(h1.backend === true && h1.frontend === false, `backend 有提示、frontend 沒有（實際 ${JSON.stringify(h1)}）`);
    checkNoAnimation(await cdp.eval(readAnimations), 'B2');
    await cdp.eval("window.__marker = document.querySelector('.ff-row-header[data-workstream=\"backend\"]'); true");
    await feed(harnessState(4, 'Plan', true));
    check(
      await cdp.eval("document.querySelector('.ff-row-header[data-workstream=\"backend\"]') !== window.__marker"),
      '第二份新投影確實整頁重畫（列首 DOM 節點被換掉）'
    );
    const h2 = await cdp.eval(readHeaders);
    check(
      h2.backend === true && h2.frontend === false,
      `停在此狀態再收到新投影重畫後結果相同（實際 ${JSON.stringify(h2)}）`
    );

    // 提示完全由投影決定：改成 false 就消失、再改回 true 就回來（不是 DOM 上殘留的狀態）。
    await feed(harnessState(5, 'Plan', false));
    const h3 = await cdp.eval(readHeaders);
    check(h3.backend === false && h3.frontend === false, `activity_undeclared 變 false 後提示消失（實際 ${JSON.stringify(h3)}）`);
    await feed(harnessState(6, 'Plan', true));
    const h4 = await cdp.eval(readHeaders);
    check(h4.backend === true && h4.frontend === false, `再變回 true 提示回來（實際 ${JSON.stringify(h4)}）`);
    // 欄位缺漏（舊版後端）不得顯示提示。
    const legacy = harnessState(7, 'Plan', true);
    delete legacy.projects[0].workstreams[0].activity_undeclared;
    await feed(legacy);
    const h5 = await cdp.eval(readHeaders);
    check(h5.backend === false, `欄位缺漏時不顯示提示（實際 ${JSON.stringify(h5)}）`);
  } finally {
    await stopChrome(chrome, 'chrome-harness');
    await new Promise((res) => server.close(res));
  }
}

async function main() {
  if (!fs.existsSync(CHROME)) {
    throw new Error(`找不到 Chrome：${CHROME}（可用環境變數 COCKPIT_CHROME 指定路徑）`);
  }
  try {
    await partPreview();
  } catch (e) {
    check(false, `A 段中止：${e.message}`);
  }
  try {
    await partHarness();
  } catch (e) {
    check(false, `B 段中止：${e.message}`);
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
