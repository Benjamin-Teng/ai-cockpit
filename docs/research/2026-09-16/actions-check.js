// Task 5.3 驗收：畫面操作（spec cockpit-dashboard「畫面操作」四個情境＋節點按鈕顯示規則；
// design D9）。
//
// 兩段：
//
// A. 真的 ui_preview（`COCKPIT_PREVIEW_PUSH_MS=100`：每 100 ms 推送一份新 version；寫入路由
//    只記錄請求並回 204，記錄行印在 stdout，格式 `write-request <METHOD> <PATH> <BODY>`）。
//    透過真正的 `/ws`＋channel.js 重畫，驗：
//    - 按鈕顯示規則：逐一比對 /api/state 每個 task 的 mark／stage 與畫面上的按鈕集合；
//      workstream 列首「改綁」一律有、「取消改綁」只在 source 為 override 時有。
//      direction-01-visual task 3.1：Factory Floor 一次只畫選定的一個 Project（design D6），
//      這裡逐 Project 點左欄切換後再比對，不是像過去那樣一次 dump 出全部 Project 的節點。
//    - 情境「推進按鈕」：按 be-1 的「推進」→ 恰好收到一個 `POST /api/projects/cockpit/tasks/be-1/advance`。
//    - 情境「改綁模式跨重畫保留」：按 be 的「改綁」→ 等 version 至少前進 2 且確認 DOM 真的被
//      換掉 → 提示、「取消」、connected runtime 未 exited pane 的「綁定到這裡」都還在 → 按
//      wJ:p3 那列 → 收到 `PUT .../workstreams/be/override`，本體 `{"runtime":"win","pane_id":"wJ:p3"}`
//      → 離開改綁模式。另驗「取消」離開且不送請求（frontend 屬於 Project p，先切過去）、
//      「取消改綁」送 `DELETE`（qa 屬於 Project cockpit，切回去）。
//    - 情境「頻繁重畫時按鈕仍有效」：連按 10 個不同 task 的「Completed」——cockpit 專案畫面上
//      全部 7 個，再點左欄切到 Project p 繼續按剩下 3 個（task 3.1 fix round 1／Codex
//      finding：曾經被誤改成只按 7 個，判定為既有驗收覆蓋率被削弱，改回跨 Project 湊足 10 個）
//      ，每次按下與放開之間刻意間隔 150 ms（> 100 ms 推送週期，保證按下與放開落在不同 DOM
//      元素上——正是 design D9 說 `click` 會遺失的情況）→ 恰好收到 10 個對應的
//      `POST .../complete`。
//
//    點擊一律用 CDP `Input.dispatchMouseEvent`（真的滑鼠事件，會產生 pointerdown／mouseup／
//    click），不是在頁面裡呼叫 `element.click()`。
//
// B. 極簡 harness（node http server 服務真正的 style.css／render.js／actions.js，不載
//    channel.js，投影以 CDP 呼叫 `window.onState(...)` 餵入）：寫入端點回應可控，驗：
//    - 情境「推進按鈕」後半：送出後畫面不自行移動節點；新投影到達後節點出現在下一站的欄。
//    - 情境「被拒絕時顯示原因」：回 409 `{"error":"已有標記"}` → 錯誤訊息含「已有標記」→ 再
//      餵兩份新投影仍在 → 按「關閉」後消失。
//    - 請求失敗（連線被切斷）也顯示錯誤；下一次操作清除舊錯誤。
//    - 鍵盤：focus 按鈕後送 Enter（`detail === 0` 的 click）會送出請求。
//
// 用法（repo 根，需先 `cargo build -p cockpit --example ui_preview`）：
//   node docs/research/2026-09-16/actions-check.js
// 清理：只終止本腳本自己 spawn 的 ui_preview.exe／chrome.exe（依 PID），沿用
// factory-floor-check.js 的 killTree／tasklist 收尾判準；埠被占用就往上找空埠。
const http = require('node:http');
const os = require('node:os');
const { spawn, spawnSync } = require('node:child_process');
const path = require('node:path');
const fs = require('node:fs');

const REPO = path.resolve(__dirname, '..', '..', '..');
const UI_PREVIEW_EXE = path.join(REPO, 'target', 'debug', 'examples', 'ui_preview.exe');
const ASSETS = path.join(REPO, 'cockpit', 'assets', 'app');
const SLOW_REJECT_PATH = '/api/projects/cockpit/tasks/be-1/fail';
const SLOW_REJECT_DELAY_MS = 1500;
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
      '--headless=new', '--lang=zh-TW',
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
// A. ui_preview（100 ms 推送）
// ---------------------------------------------------------------------------

// 依 spec「畫面操作」算出某 task 節點應有的按鈕（data-action 集合，排序後比較）。
function expectedTaskActions(project, task) {
  const first = project.stages[0];
  const last = project.stages[project.stages.length - 1];
  const actions = [];
  if (task.mark === 'none') {
    // progress-model task 4.2：spec「畫面操作」新增「退回」（mark 為 none 且不在第一個 stage）。
    if (task.stage !== first) actions.push('retreat');
    if (task.stage !== last) actions.push('advance');
    actions.push('complete', 'fail');
  } else {
    actions.push('clear');
  }
  return actions.sort();
}

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
        // fix round 1：A（be-1 的 Failed）慢 1500 ms 且回 409，供「過期錯誤」情境使用；
        // 其他情境都不按這顆按鈕。
        COCKPIT_PREVIEW_WRITE_RULES: `${SLOW_REJECT_PATH}=${SLOW_REJECT_DELAY_MS}:409`,
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

    const s1 = await (await fetch(`http://127.0.0.1:${port}/api/state`)).json();
    await sleep(350);
    const s2 = await (await fetch(`http://127.0.0.1:${port}/api/state`)).json();
    check(
      s2.version >= s1.version + 2,
      `100 ms 推送模式：350 ms 內 version 應該至少前進 2（${s1.version} → ${s2.version}）`
    );

    // 寫入路由只記錄、回 204；GET 同路徑不被接受。
    const probe = await fetch(`http://127.0.0.1:${port}/api/projects/x/tasks/y/advance`, {
      method: 'POST',
    });
    check(probe.status === 204, `ui_preview 的 POST 寫入路由應該回 204（實際 ${probe.status}）`);
    const probeGet = await fetch(`http://127.0.0.1:${port}/api/projects/x/tasks/y/advance`);
    check(probeGet.status === 405, `ui_preview 的寫入路徑 GET 應該回 405（實際 ${probeGet.status}）`);
    await sleep(200);
    check(
      requests.length === 1 && requests[0].method === 'POST' && requests[0].path === '/api/projects/x/tasks/y/advance',
      `ui_preview 應該在 stdout 記錄那一筆 POST（實際 ${JSON.stringify(requests)}）`
    );
    requests.length = 0;

    const url = `http://127.0.0.1:${port}/`;
    chrome = await startChrome(cdpPort, url, 'preview');
    const { cdp } = chrome;
    // cockpit 是未選定過時的預設 Project（spec cockpit-dashboard「Project 切換」；design
    // D6；direction-01-visual task 3.1：中上區域只顯示選定的一個 Project），fixture 的
    // cockpit 專案有 9 個 task，這裡先等它畫出來再往下走；project p（Scenario D）的 3 個節點
    // 要切過去才看得到，見下面「按鈕顯示規則」逐 Project 迴圈。
    await cdp.waitFor("document.querySelectorAll('.task-node').length >= 9", 5000, '畫出 cockpit 專案全部 task 節點');

    // --- 按鈕顯示規則 ---
    // direction-01-visual task 3.1：Factory Floor 一次只畫選定的一個 Project，原本「一次
    // dump 出全部 12 個 task 節點、跨兩個 Project 一起核對」的做法不成立了——這裡改成逐
    // Project 迴圈：每個 Project 各自點左欄切過去（design D6：data-action="select-project"）、
    // 等它的 Factory Floor 真的畫出來，再讀當下 DOM 核對「只讀得到這個 Project 自己的按鈕」。
    // cockpit 是預設選定，這裡仍明確點一次（select-project 冪等、不是「畫面操作」，不影響
    // 後面任何情境的前置狀態，見 design D6）。
    log('--- 按鈕顯示規則（對照 /api/state，逐 Project 切換後比對）---');
    const state = await (await fetch(`http://127.0.0.1:${port}/api/state`)).json();
    let cockpitLabels = null;
    for (const project of state.projects) {
      await cdp.click(`[data-action="select-project"][data-project="${project.id}"]`);
      await cdp.waitFor(
        `!!document.querySelector('.project[data-project="${project.id}"]')`,
        3000,
        `切到 Project ${project.id}`
      );
      await cdp.waitFor(
        `document.querySelectorAll('.task-node').length >= ${project.tasks.length}`,
        3000,
        `Project ${project.id} 的 task 節點全部畫出`
      );
      const domActions = await cdp.eval(`(() => {
        const out = {};
        document.querySelectorAll('[data-action][data-task]').forEach((b) => {
          const k = b.dataset.project + '/' + b.dataset.task;
          (out[k] = out[k] || []).push(b.dataset.action);
        });
        const ws = {};
        document.querySelectorAll('[data-action][data-workstream]').forEach((b) => {
          const k = b.dataset.project + '/' + b.dataset.workstream;
          (ws[k] = ws[k] || []).push(b.dataset.action);
        });
        return { tasks: out, ws };
      })()`);
      for (const task of project.tasks) {
        const key = `${project.id}/${task.id}`;
        const actual = (domActions.tasks[key] || []).slice().sort();
        const expected = expectedTaskActions(project, task);
        check(
          JSON.stringify(actual) === JSON.stringify(expected),
          `task ${key}（stage ${task.stage}、mark ${task.mark}）按鈕應該是 ${JSON.stringify(expected)}（實際 ${JSON.stringify(actual)}）`
        );
      }
      for (const w of project.workstreams) {
        const key = `${project.id}/${w.id}`;
        const actual = (domActions.ws[key] || []).slice().sort();
        // repo-projects task 5.3：source 為 pane（Repo Project 固定 pane 的工作線）列首沒有「改綁」與「取消改綁」
        //（spec cockpit-dashboard「畫面操作」）；其餘維持舊規則。
        const expected =
          w.binding.source === 'pane' ? [] : w.binding.source === 'override' ? ['override-clear', 'rebind'] : ['rebind'];
        check(
          JSON.stringify(actual) === JSON.stringify(expected),
          `workstream ${key}（binding ${w.binding.state}${w.binding.source ? '/' + w.binding.source : ''}）按鈕應該是 ${JSON.stringify(expected)}（實際 ${JSON.stringify(actual)}）`
        );
      }
      if (project.id === 'cockpit') {
        // 按鈕文字（spec）只需要驗一次，be-1／ops-1／be／qa 都是 cockpit 專案的節點，趁 cockpit
        // 還選定著的這一輪順便讀（下面還有其他 Project 要切，讀完才切走）。
        cockpitLabels = await cdp.eval(`(() => {
          const t = (sel) => { const n = document.querySelector(sel); return n ? n.textContent : null; };
          return {
            advance: t('[data-action="advance"][data-task="be-1"]'),
            complete: t('[data-action="complete"][data-task="be-1"]'),
            fail: t('[data-action="fail"][data-task="be-1"]'),
            clear: t('[data-action="clear"][data-task="ops-1"]'),
            rebind: t('[data-action="rebind"][data-workstream="be"]'),
            overrideClear: t('[data-action="override-clear"][data-workstream="qa"]'),
          };
        })()`);
      }
    }
    check(
      JSON.stringify(cockpitLabels) ===
        JSON.stringify({
          advance: '推進',
          complete: 'Completed',
          fail: 'Failed',
          clear: '清除標記',
          rebind: '改綁',
          overrideClear: '取消改綁',
        }),
      `按鈕文字應該照 spec（實際 ${JSON.stringify(cockpitLabels)}）`
    );
    check(
      (await cdp.eval("document.querySelectorAll('[data-action=\"bind-here\"]').length")) === 0,
      '不在改綁模式時不應該出現「綁定到這裡」'
    );

    // 上面的逐 Project 迴圈以 state.projects 的順序（fixture：cockpit、p）跑完，最後一個是
    // p——後面「推進按鈕」「改綁模式跨重畫保留」等情境都預期 cockpit 是目前選定的 Project，
    // 這裡切回去（select-project 不是「畫面操作」，不影響任何進行中的狀態，見 design D6）。
    await cdp.click('[data-action="select-project"][data-project="cockpit"]');
    await cdp.waitFor(
      "!!document.querySelector('.project[data-project=\"cockpit\"]')",
      3000,
      '切回 cockpit 供後續情境使用'
    );

    // --- 情境：推進按鈕 ---
    log('--- 情境「推進按鈕」（送出路徑）---');
    await cdp.click('[data-action="advance"][data-project="cockpit"][data-task="be-1"]');
    await sleep(500);
    check(
      requests.length === 1 &&
        requests[0].method === 'POST' &&
        requests[0].path === '/api/projects/cockpit/tasks/be-1/advance',
      `應該恰好收到一個 POST /api/projects/cockpit/tasks/be-1/advance（實際 ${JSON.stringify(requests)}）`
    );
    requests.length = 0;

    // --- 情境：改綁模式跨重畫保留 ---
    log('--- 情境「改綁模式跨重畫保留」---');
    await cdp.click('[data-action="rebind"][data-project="cockpit"][data-workstream="be"]');
    await cdp.waitFor("!!document.querySelector('.rebind-banner')", 2000, '進入改綁模式出現提示');
    const v0 = await cdp.eval("Number(document.getElementById('version').getAttribute('data-state-version'))");
    await cdp.eval("window.__oldBanner = document.querySelector('.rebind-banner'); true");
    await cdp.waitFor(
      `Number(document.getElementById('version').getAttribute('data-state-version')) >= ${v0 + 2}`,
      3000,
      '改綁模式期間收到兩份新投影'
    );
    const v1 = await cdp.eval("Number(document.getElementById('version').getAttribute('data-state-version'))");
    check(v1 >= v0 + 2, `期間至少重畫兩次（version ${v0} → ${v1}）`);
    check(
      await cdp.eval("!document.contains(window.__oldBanner)"),
      '舊的提示節點已不在文件中（整頁重畫真的發生過）'
    );
    const mode = await cdp.eval(`(() => {
      const banner = document.querySelector('.rebind-banner');
      const bind = [...document.querySelectorAll('[data-action="bind-here"]')].map((b) => b.dataset.runtime + '/' + b.dataset.pane + ':' + b.textContent);
      return {
        banner: banner ? banner.textContent : null,
        cancel: !!document.querySelector('.rebind-banner [data-action="rebind-cancel"]'),
        bind,
      };
    })()`);
    check(
      typeof mode.banner === 'string' && mode.banner.includes('Backend'),
      `改綁提示應該指出目標 workstream Backend（實際 ${JSON.stringify(mode.banner)}）`
    );
    check(mode.cancel, '改綁提示裡應該有「取消」');
    // file-review task 3.4（design D12）：fixture 多了兩個未 exited 的 pane（wJ:p4／wJ:p5，掛在
    // 新增的 tab wJ:t2，供瀏覽器驗收腳本測檔案瀏覽——必須是未 exited 才選得到，見
    // ui_preview.rs 的 add_review_fixture_panes 文件），DOM 順序（tab wJ:t1 在前、wJ:t2 在後）
    // 下它們也會出現在「綁定到這裡」候選清單裡；候選清單本身仍是「只有 connected runtime 未
    // exited 的 pane」這條規則算出來的，只是筆數隨 fixture 資料變動。
    check(
      JSON.stringify(mode.bind) ===
        JSON.stringify([
          'win/wJ:p1:綁定到這裡',
          'win/wJ:p3:綁定到這裡',
          'win/wJ:p4:綁定到這裡',
          'win/wJ:p5:綁定到這裡',
          // repo-projects task 4.5：Repo Project `demo-app` 綁定的兩個未 exited pane（同在 tab wJ:t2）。
          'win/wJ:p6:綁定到這裡',
          'win/wJ:p7:綁定到這裡',
        ]),
      `只有 connected runtime（win）未 exited 的 pane（wJ:p1、wJ:p3、wJ:p4、wJ:p5、wJ:p6、wJ:p7）有「綁定到這裡」（實際 ${JSON.stringify(mode.bind)}）`
    );
    check(requests.length === 0, `進入改綁模式本身不送請求（實際 ${JSON.stringify(requests)}）`);
    await cdp.click('[data-action="bind-here"][data-runtime="win"][data-pane="wJ:p3"]');
    await sleep(500);
    let putBody = null;
    try {
      putBody = requests.length === 1 ? JSON.parse(requests[0].body) : null;
    } catch {
      putBody = null;
    }
    check(
      requests.length === 1 &&
        requests[0].method === 'PUT' &&
        requests[0].path === '/api/projects/cockpit/workstreams/be/override' &&
        putBody !== null &&
        putBody.runtime === 'win' &&
        putBody.pane_id === 'wJ:p3' &&
        Object.keys(putBody).length === 2,
      `應該恰好收到 PUT /api/projects/cockpit/workstreams/be/override 本體 {"runtime":"win","pane_id":"wJ:p3"}（實際 ${JSON.stringify(requests)}）`
    );
    requests.length = 0;
    await cdp.waitFor(
      "!document.querySelector('.rebind-banner') && document.querySelectorAll('[data-action=\"bind-here\"]').length === 0",
      2000,
      '覆蓋成功後離開改綁模式'
    );

    log('--- 改綁模式「取消」---');
    // workstream frontend 屬於 Project p（direction-01-visual task 3.1：Factory Floor 一次
    // 只畫選定的一個 Project），先切過去它的「改綁」按鈕才存在於畫面上（design D6：
    // select-project 不是「畫面操作」，不影響即將進入的改綁模式）。
    await cdp.click('[data-action="select-project"][data-project="p"]');
    await cdp.waitFor("!!document.querySelector('.project[data-project=\"p\"]')", 3000, '切到 Project p');
    await cdp.click('[data-action="rebind"][data-project="p"][data-workstream="frontend"]');
    await cdp.waitFor("!!document.querySelector('.rebind-banner')", 2000, '再次進入改綁模式');
    await cdp.click('.rebind-banner [data-action="rebind-cancel"]');
    await cdp.waitFor("!document.querySelector('.rebind-banner')", 2000, '按「取消」離開改綁模式');
    await sleep(300);
    check(requests.length === 0, `按「取消」不送任何請求（實際 ${JSON.stringify(requests)}）`);

    log('--- 「取消改綁」---');
    // workstream qa 屬於 Project cockpit，切回去（同上，select-project 不影響進行中狀態）。
    await cdp.click('[data-action="select-project"][data-project="cockpit"]');
    await cdp.waitFor("!!document.querySelector('.project[data-project=\"cockpit\"]')", 3000, '切回 Project cockpit');
    await cdp.click('[data-action="override-clear"][data-project="cockpit"][data-workstream="qa"]');
    await sleep(500);
    check(
      requests.length === 1 &&
        requests[0].method === 'DELETE' &&
        requests[0].path === '/api/projects/cockpit/workstreams/qa/override',
      `應該恰好收到 DELETE /api/projects/cockpit/workstreams/qa/override（實際 ${JSON.stringify(requests)}）`
    );
    requests.length = 0;

    // --- 情境：頻繁重畫時按鈕仍有效 ---
    // fix round 1／Codex finding（2）：direction-01-visual task 3.1 把「連按 10 個不同
    // task」誤改成只按 cockpit 專案畫面上找得到的 7 個，被 Codex 判定為既有驗收覆蓋率被削弱
    // ——原始 spec 要求的是「同一個 100 ms 推送週期內連續按壓 10 個不同按鈕，每次按下與放開
    // 落在不同 DOM 元素時仍然送得出去」（design D9），10 這個數字本身雖然不是 spec 逐字規定，
    // 但既有驗收一直用 10 顆按鈕的壓力測試涵蓋，7 顆會讓「第 8～10 次操作遺失」這種回歸在測試
    // 裡量不到，即使那不是 spec 逐字要求也不該無聲少驗。改回 10：先操作 cockpit 專案畫面上
    // 全部 7 個「Completed」，再點左欄切到 Project p（design D6：select-project 不影響任何
    // 進行中狀態），操作 p 專案的 3 個「Completed」（backend-1／frontend-1／tests-1，p 的
    // 全部 task 都是 mark === "none"），合計 10 個不同 task，跨兩個 Project 一起驗「按下與
    // 放開落在不同 DOM 元素仍送得出去」在 Project 切換之後依然成立。
    log('--- 情境「頻繁重畫時按鈕仍有效」（連按 10 個不同 task：cockpit 7 個＋切到 p 再按 3 個，按住 150 ms）---');
    const cockpitTargets = await cdp.eval(
      "[...document.querySelectorAll('[data-action=\"complete\"]')].map((b) => [b.dataset.project, b.dataset.task])"
    );
    check(
      cockpitTargets.length === 7,
      `cockpit 專案畫面上應該有 7 個「Completed」（實際 ${cockpitTargets.length}：${JSON.stringify(cockpitTargets)}）`
    );
    const vBefore = await cdp.eval("Number(document.getElementById('version').getAttribute('data-state-version'))");
    for (const [project, task] of cockpitTargets) {
      await cdp.click(`[data-action="complete"][data-project="${project}"][data-task="${task}"]`, 150);
    }
    await cdp.click('[data-action="select-project"][data-project="p"]');
    await cdp.waitFor("!!document.querySelector('.project[data-project=\"p\"]')", 3000, '切到 Project p 繼續按');
    const pTargets = await cdp.eval(
      "[...document.querySelectorAll('[data-action=\"complete\"]')].map((b) => [b.dataset.project, b.dataset.task])"
    );
    check(
      pTargets.length === 4,
      `p 專案畫面上應該有 4 個「Completed」（實際 ${pTargets.length}：${JSON.stringify(pTargets)}）`
    );
    for (const [project, task] of pTargets) {
      await cdp.click(`[data-action="complete"][data-project="${project}"][data-task="${task}"]`, 150);
    }
    // progress-model task 4.1：ui_preview 在 p 末尾新增 undeclared-1（mark=none，供「退回」驗收），
    // p 的「Completed」從 3 個變 4 個、合計從 10 個變 11 個（仍涵蓋原本 10 個以上不同 task）。
    const chosen = cockpitTargets.concat(pTargets);
    check(chosen.length === 11, `合計應該連按 11 個不同 task（實際 ${chosen.length}）`);
    const vAfter = await cdp.eval("Number(document.getElementById('version').getAttribute('data-state-version'))");
    check(
      vAfter - vBefore >= 10,
      `連按期間持續重畫（version ${vBefore} → ${vAfter}，至少前進 10）`
    );
    await sleep(800);
    const expectedPaths = chosen.map(([p, t]) => `POST /api/projects/${p}/tasks/${t}/complete`).sort();
    const actualPaths = requests.map((r) => `${r.method} ${r.path}`).sort();
    check(
      JSON.stringify(actualPaths) === JSON.stringify(expectedPaths),
      `應該恰好收到 10 個對應的 POST（實際 ${actualPaths.length} 個：${JSON.stringify(actualPaths)}）`
    );
    check(
      (await cdp.eval("document.querySelectorAll('.error-banner').length")) === 0,
      '204 成功後不顯示錯誤訊息'
    );

    // 下面「fix round 1：過期的錯誤不得蓋掉較新的操作」情境用的是 cockpit 專案的 be-1／
    // release-1，切回去（select-project 不是「畫面操作」，不影響任何進行中狀態，見 design D6）。
    await cdp.click('[data-action="select-project"][data-project="cockpit"]');
    await cdp.waitFor(
      "!!document.querySelector('.project[data-project=\"cockpit\"]')",
      3000,
      '切回 cockpit 供後續情境使用'
    );

    // --- fix round 1：過期的錯誤不得蓋掉較新的操作 ---
    // A＝be-1「Failed」（ui_preview 延遲 1500 ms 回 409），B＝release-1「推進」（立即 204）。
    // 先按 A 再按 B：B 是最近一次操作且成功，A 之後才回 409，畫面不得顯示 A 的錯誤。
    log('--- fix round 1：慢的舊操作失敗不蓋掉較新的成功操作 ---');
    requests.length = 0;
    await cdp.click('[data-action="fail"][data-project="cockpit"][data-task="be-1"]');
    await cdp.click('[data-action="advance"][data-project="cockpit"][data-task="release-1"]');
    await sleep(SLOW_REJECT_DELAY_MS + 1000);
    const order = requests.map((r) => `${r.method} ${r.path}`);
    check(
      JSON.stringify(order) ===
        JSON.stringify([`POST ${SLOW_REJECT_PATH}`, 'POST /api/projects/cockpit/tasks/release-1/advance']),
      `伺服器依序收到 A、B（實際 ${JSON.stringify(order)}）`
    );
    check(
      (await cdp.eval("document.querySelectorAll('.error-banner').length")) === 0,
      `A 在 B 之後才回 409，畫面不應該顯示 A 的過期錯誤（實際：${JSON.stringify(await cdp.eval("(() => { const b = document.querySelector('.error-banner'); return b ? b.textContent : null; })()"))}）`
    );

    // 對照組：只按 A（它就是最近一次操作），1500 ms 後的 409 要顯示——證明上面的「沒有錯誤」
    // 不是因為 409 規則沒生效。
    log('--- fix round 1 對照組：慢操作本身就是最近一次操作時照常顯示錯誤 ---');
    requests.length = 0;
    await cdp.click('[data-action="fail"][data-project="cockpit"][data-task="be-1"]');
    await cdp.waitFor(
      "(() => { const b = document.querySelector('.error-banner'); return !!b && b.textContent.includes('HTTP 409') && b.textContent.includes('ui_preview 模擬回應 409'); })()",
      SLOW_REJECT_DELAY_MS + 2000,
      '最近一次操作 A 的 409 錯誤訊息出現（含本體 error）'
    );
    await cdp.click('.error-banner [data-action="error-dismiss"]');
    await cdp.waitFor("!document.querySelector('.error-banner')", 2000, '關閉對照組的錯誤訊息');
  } finally {
    await stopChrome(chrome, 'chrome-preview');
    killTree(server, 'ui_preview');
    await sleep(300);
    check(!isPortListening(port), `port ${port} 應該不再有 LISTENING 的行程`);
  }
}

// ---------------------------------------------------------------------------
// B. harness（可控回應、直接餵投影）
// ---------------------------------------------------------------------------

const HARNESS_HTML = `<!doctype html>
<html><head><meta charset="utf-8"><title>actions.js harness</title>
<link rel="stylesheet" href="/app/style.css">
</head>
<body>
<div id="app"></div>
<script src="/app/i18n.js"></script>
<script src="/app/render.js"></script>
<script src="/app/actions.js"></script>
</body></html>`;

function harnessState(version, stage, mark) {
  return {
    version,
    generated_at: '2026-09-16T00:00:00Z',
    runtimes: [],
    projects: [
      {
        id: 'p',
        name: 'Harness',
        stages: ['Plan', 'Implement', 'Test'],
        warnings: [],
        workstreams: [{ id: 'ws', name: 'WS', binding: { state: 'none' } }],
        tasks: [
          {
            id: 't1',
            title: '任務一',
            workstream: 'ws',
            stage,
            mark,
            status: mark === 'none' ? 'ready' : mark,
            depends_on: [],
          },
        ],
      },
    ],
    recent_events: [],
  };
}

async function partHarness() {
  log('=== B. harness（409、請求失敗、鍵盤、畫面不自行修改狀態）===');
  const port = pickPort(18810);
  const cdpPort = pickPort(18811, [port]);
  const requests = [];
  const server = http.createServer((req, res) => {
    const files = {
      '/': ['text/html', HARNESS_HTML],
      '/app/style.css': ['text/css', null],
      '/app/i18n.js': ['text/javascript', null],
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
      // 例如瀏覽器自動要的 /favicon.ico：不是寫入請求，不記錄。
      res.writeHead(404);
      res.end();
      return;
    }
    let body = '';
    req.on('data', (c) => (body += c));
    req.on('end', () => {
      requests.push({ method: req.method, path: req.url, body });
      if (req.url.endsWith('/complete')) {
        res.writeHead(409, { 'Content-Type': 'application/json' });
        res.end('{"error":"已有標記"}');
      } else if (req.url.endsWith('/fail')) {
        req.socket.destroy();
      } else {
        res.writeHead(204);
        res.end();
      }
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
    const cellHasT1 = (stage) =>
      cdp.eval(
        `!!document.querySelector('.ff-cell[data-workstream="ws"][data-stage="${stage}"] [data-task="t1"]')`
      );

    await feed(harnessState(1, 'Plan', 'none'));

    // --- 推進：送出後不自行修改，新投影到達後出現在下一站 ---
    log('--- 情境「推進按鈕」後半（等新投影才移動）---');
    await cdp.click('[data-action="advance"][data-task="t1"]');
    await sleep(400);
    check(
      requests.length === 1 && requests[0].method === 'POST' && requests[0].path === '/api/projects/p/tasks/t1/advance',
      `harness 收到 POST /api/projects/p/tasks/t1/advance（實際 ${JSON.stringify(requests)}）`
    );
    check(await cellHasT1('Plan'), '204 之後、新投影到達前，t1 仍在 Plan（畫面不自行修改狀態）');
    await feed(harnessState(2, 'Implement', 'none'));
    check(await cellHasT1('Implement'), '新投影到達後 t1 出現在下一站 Implement');
    check(!(await cellHasT1('Plan')), '新投影到達後 Plan 不再有 t1');
    requests.length = 0;

    // --- 409 錯誤跨重畫保留 ---
    log('--- 情境「被拒絕時顯示原因」---');
    await cdp.click('[data-action="complete"][data-task="t1"]');
    await cdp.waitFor("!!document.querySelector('.error-banner')", 2000, '409 後出現錯誤訊息');
    const errText = await cdp.eval("document.querySelector('.error-banner').textContent");
    check(errText.includes('已有標記'), `錯誤訊息含「已有標記」（實際 ${JSON.stringify(errText)}）`);
    check(errText.includes('409'), `錯誤訊息帶 HTTP 狀態碼 409（實際 ${JSON.stringify(errText)}）`);
    await feed(harnessState(3, 'Implement', 'none'));
    await feed(harnessState(4, 'Implement', 'none'));
    const afterTwo = await cdp.eval(
      "(() => { const b = document.querySelector('.error-banner'); return b ? b.textContent : null; })()"
    );
    check(
      typeof afterTwo === 'string' && afterTwo.includes('已有標記'),
      `再重畫兩次後錯誤訊息仍在（實際 ${JSON.stringify(afterTwo)}；version ${await cdp.eval("document.getElementById('version').getAttribute('data-state-version')")}）`
    );
    await cdp.click('.error-banner [data-action="error-dismiss"]');
    await cdp.waitFor("!document.querySelector('.error-banner')", 2000, '按「關閉」後錯誤訊息消失');
    await feed(harnessState(5, 'Implement', 'none'));
    check(
      (await cdp.eval("document.querySelectorAll('.error-banner').length")) === 0,
      '關閉後的重畫不會把錯誤訊息帶回來'
    );

    // --- 請求失敗（連線被切斷）也顯示錯誤；下一次操作清除舊錯誤 ---
    log('--- 請求失敗、下一次操作清除 ---');
    await cdp.click('[data-action="fail"][data-task="t1"]');
    await cdp.waitFor("!!document.querySelector('.error-banner')", 3000, '請求失敗後出現錯誤訊息');
    await cdp.click('[data-action="advance"][data-task="t1"]');
    await cdp.waitFor("!document.querySelector('.error-banner')", 2000, '下一次操作（成功）清除舊錯誤訊息');
    requests.length = 0;

    // --- 鍵盤（detail === 0 的 click）---
    log('--- 鍵盤操作 ---');
    await cdp.eval("document.querySelector('[data-action=\"advance\"][data-task=\"t1\"]').focus(); true");
    check(
      await cdp.eval("document.activeElement && document.activeElement.dataset.action === 'advance'"),
      '推進按鈕取得焦點'
    );
    await cdp.send('Input.dispatchKeyEvent', {
      type: 'keyDown',
      key: 'Enter',
      code: 'Enter',
      windowsVirtualKeyCode: 13,
      text: '\r',
    });
    await cdp.send('Input.dispatchKeyEvent', { type: 'keyUp', key: 'Enter', code: 'Enter', windowsVirtualKeyCode: 13 });
    await sleep(400);
    check(
      requests.length === 1 && requests[0].path === '/api/projects/p/tasks/t1/advance',
      `按 Enter 送出一次 POST advance（實際 ${JSON.stringify(requests)}）`
    );
    requests.length = 0;

    // mark 不是 none：只顯示「清除標記」，按下送 clear。
    await feed(harnessState(6, 'Implement', 'completed'));
    const onlyClear = await cdp.eval(
      "[...document.querySelectorAll('[data-task=\"t1\"]')].map((b) => b.dataset.action)"
    );
    check(
      JSON.stringify(onlyClear) === JSON.stringify(['clear']),
      `mark 為 completed 時只顯示「清除標記」（實際 ${JSON.stringify(onlyClear)}）`
    );
    await cdp.click('[data-action="clear"][data-task="t1"]');
    await sleep(400);
    check(
      requests.length === 1 && requests[0].method === 'POST' && requests[0].path === '/api/projects/p/tasks/t1/clear',
      `「清除標記」送出 POST .../t1/clear（實際 ${JSON.stringify(requests)}）`
    );
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
